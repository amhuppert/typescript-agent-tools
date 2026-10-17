//! `isSymbolAccessibleWorker` and its declaration visibility dependency.
//! Lexical chains are not sufficient: native visibility and its required
//! diagnostic symbol serializations remain part of the same question.
use super::*;
use crate::project::semantic::model::{NativeCompletion, NativeResult, NativeSequence};
use crate::project::semantic::source::symbol_visibility::{VisibilityFallback, VisibilityStep};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Accessibility {
    Accessible,
    NotAccessible,
    CannotBeNamed,
}

impl Evaluator<'_, '_> {
    /// `isDeclarationVisible` caches only completed native true/false slots.
    /// Synthetic declarations are not parse-tree nodes; ordinary created
    /// symbols instead retain their copied actual declaration identities.
    fn declaration_is_visible(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<bool, DemandError> {
        let mut path = Vec::new();
        let mut current = Some(declaration);
        let visible = loop {
            let Some(declaration) = current else {
                break false;
            };
            if let Some(&visible) = self.symbol_accessibility.visible.get(&declaration) {
                break visible;
            }
            path.push(declaration);
            let step = match declaration {
                ReferenceDeclaration::Captured(declaration) => {
                    self.source.declaration_visibility_step(declaration)?
                }
                ReferenceDeclaration::Json { declaration, .. } => VisibilityStep::Known(matches!(
                    declaration,
                    crate::project::binding::JsonDeclaration::SourceFile
                )),
            };
            match step {
                VisibilityStep::Known(visible) => break visible,
                VisibilityStep::Parent(parent) => {
                    current = parent.map(ReferenceDeclaration::Captured)
                }
            }
        };
        for declaration in path.into_iter().rev() {
            self.symbol_accessibility
                .visible
                .insert(declaration, visible);
        }
        Ok(visible)
    }

    /// `hasVisibleDeclarations` with shouldComputeAliasToMakeVisible=false.
    /// All actual declaration alternatives still run; the native callback is
    /// a no-op rather than a mutation of the declaration visibility cache.
    fn has_visible_declarations(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            let kind = self.reference_declaration_kind(declaration)?;
            if kind == K::Identifier || self.declaration_is_visible(declaration)? {
                continue;
            }
            let ReferenceDeclaration::Captured(declaration) = declaration else {
                return Ok(false);
            };
            let block_scoped = kind == K::BindingElement
                && self
                    .reference_symbol_flags(symbol, consumer)?
                    .block_scoped_variable;
            let mut visible = false;
            for alternative in self
                .source
                .declaration_visibility_fallbacks(declaration, block_scoped)?
            {
                match alternative {
                    VisibilityFallback::Visible => {
                        visible = true;
                        break;
                    }
                    VisibilityFallback::Parent(parent)
                        if self
                            .declaration_is_visible(ReferenceDeclaration::Captured(parent))? =>
                    {
                        visible = true;
                        break;
                    }
                    VisibilityFallback::Parent(_) => {}
                }
            }
            if !visible {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `getExternalModuleContainer` returns the actual symbol of the first
    /// source-file/ambient-module ancestor. JSON has no external-module
    /// indicator and therefore does not manufacture such an ancestor.
    pub(super) fn external_module_container_completion(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let ReferenceDeclaration::Captured(declaration) = declaration else {
                return Ok(None);
            };
            self.source
                .diagnostic_external_module_container(declaration)?
                .map(|declaration| {
                    sequence.read(self.reference_symbol_of_declaration_completion(declaration))
                })
                .transpose()
        })();
        sequence.finish(result)
    }

    /// `IsValueSymbolAccessible` asks the full result owner, including the
    /// diagnostic-name work required by a negative result before discarding
    /// those strings. A nonempty accessible chain alone is not visibility.
    pub(in crate::project::semantic::evaluation) fn is_value_symbol_accessible_completion(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: Option<ScopeLocation>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.is_symbol_accessible_completion(symbol, enclosing, SymbolMeaning::Value, consumer)
    }

    /// `IsSymbolAccessible(..., shouldComputeAliasesToMakeVisible=false)` is
    /// also the node-copy question for Type/Namespace names. It shares the
    /// full result computation, including negative-result diagnostic reads.
    pub(in crate::project::semantic::evaluation) fn is_symbol_accessible_completion(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: Option<ScopeLocation>,
        meaning: SymbolMeaning,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.symbol_accessible_completion(symbol, enclosing, meaning, consumer)
            .map(|value| value.map(|access| access == Accessibility::Accessible))
    }

    fn symbol_accessible_completion(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: Option<ScopeLocation>,
        meaning: SymbolMeaning,
        consumer: &Location,
    ) -> NativeResult<Accessibility> {
        let Some(enclosing) = enclosing else {
            return Ok(NativeCompletion::clean(Accessibility::Accessible));
        };
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(result) = sequence.read(self.any_symbol_accessible_completion(
                &[symbol],
                enclosing,
                symbol,
                meaning,
                consumer,
            ))? {
                return Ok(result);
            }
            let mut symbol_external = None;
            for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                if let Some(external) =
                    sequence.read(self.external_module_container_completion(declaration))?
                {
                    symbol_external = Some(external);
                    break;
                }
            }
            if let Some(symbol_external) = symbol_external {
                let enclosing_external =
                    match self.diagnostic_captured_enclosing(Some(enclosing))? {
                        Some(declaration) => {
                            sequence.read(self.external_module_container_completion(
                                ReferenceDeclaration::Captured(declaration),
                            ))?
                        }
                        None => None,
                    };
                let same = match enclosing_external {
                    Some(external) => {
                        self.reference_symbol_identity(external, consumer)?
                            == self.reference_symbol_identity(symbol_external, consumer)?
                    }
                    None => false,
                };
                if !same {
                    sequence.read(self.diagnostic_symbol_display_at_completion(
                        symbol,
                        Some(enclosing),
                        meaning,
                        consumer,
                    ))?;
                    sequence.read(
                        self.raw_diagnostic_symbol_display_completion(symbol_external, consumer),
                    )?;
                    return Ok(Accessibility::CannotBeNamed);
                }
            }
            sequence.read(self.diagnostic_symbol_display_at_completion(
                symbol,
                Some(enclosing),
                meaning,
                consumer,
            ))?;
            Ok(Accessibility::NotAccessible)
        })();
        sequence.finish(result)
    }

    /// `IsAnySymbolAccessible` with native shouldComputeAliases=false and
    /// allowModules=true, the exact flags used by IsValueSymbolAccessible.
    /// Container recursion preserves current meaning and the original symbol.
    fn any_symbol_accessible_completion(
        &mut self,
        symbols: &[ReferenceSymbol],
        enclosing: ScopeLocation,
        initial: ReferenceSymbol,
        meaning: SymbolMeaning,
        consumer: &Location,
    ) -> NativeResult<Option<Accessibility>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut had_chain = None;
            for &symbol in symbols {
                let chain = sequence.read(self.accessible_symbol_chain_completion(
                    symbol,
                    Some(enclosing),
                    meaning,
                    false,
                    consumer,
                ))?;
                if let Some(&first) = chain.first() {
                    had_chain = Some(symbol);
                    if self.has_visible_declarations(first, consumer)? {
                        return Ok(Some(Accessibility::Accessible));
                    }
                }
                for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                    if self.accessibility_external_declaration(declaration)? {
                        return Ok(Some(Accessibility::Accessible));
                    }
                }
                let containers = sequence.read(self.symbol_containers_completion(
                    symbol,
                    Some(enclosing),
                    meaning,
                    consumer,
                ))?;
                let next = if self.reference_symbol_identity(initial, consumer)?
                    == self.reference_symbol_identity(symbol, consumer)?
                {
                    meaning.qualifier()
                } else {
                    meaning
                };
                if let Some(result) = sequence.read(self.any_symbol_accessible_completion(
                    &containers,
                    enclosing,
                    initial,
                    next,
                    consumer,
                ))? {
                    return Ok(Some(result));
                }
            }
            if let Some(had_chain) = had_chain {
                if self.reference_symbol_identity(had_chain, consumer)?
                    != self.reference_symbol_identity(initial, consumer)?
                {
                    sequence.read(self.diagnostic_symbol_display_at_completion(
                        had_chain,
                        Some(enclosing),
                        SymbolMeaning::Namespace,
                        consumer,
                    ))?;
                }
                sequence.read(self.diagnostic_symbol_display_at_completion(
                    initial,
                    Some(enclosing),
                    meaning,
                    consumer,
                ))?;
                return Ok(Some(Accessibility::NotAccessible));
            }
            Ok(None)
        })();
        sequence.finish(result)
    }
}
