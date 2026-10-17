//! Default symbol serialization shares actual name facts with type headers.
//! Unlike typeToString, symbolToString has no final output-byte truncation.

use super::{DemandError, Evaluator, Location, PrimitiveName, RenderedTypeName, TypeView};
use crate::project::semantic::evaluation::{ReferenceDeclaration, ReferenceSymbol};
use crate::project::semantic::model::{NativeResult, NativeSequence, PropertySymbolId, TypeId};
use crate::syntax_kind::SyntaxKind;

impl Evaluator<'_, '_> {
    /// The standalone/header spelling question has the same initial-name,
    /// nil-enclosing context as the root of default symbolToString. Semantic
    /// serializers pass their existing Context to the shared worker instead.
    pub(in crate::project::semantic::evaluation) fn symbol_name_as_written(
        &mut self,
        symbol: impl Into<ReferenceSymbol>,
        consumer: &Location,
    ) -> Result<String, DemandError> {
        let mut context = super::Context::new(super::DiagnosticTypeFormat(1 << 24), None, false);
        self.symbol_name_as_written_in_context(symbol.into(), &mut context, consumer)
    }

    /// `getNameOfSymbolAsWritten`: remapping, the default export binding
    /// context, written declaration spelling and nameType share one owner.
    /// The name-map key is identity bookkeeping; it does not observe B order.
    pub(super) fn symbol_name_as_written_in_context(
        &mut self,
        symbol: ReferenceSymbol,
        context: &mut super::Context,
        consumer: &Location,
    ) -> Result<String, DemandError> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        let symbol = context
            .remapped_symbol_references
            .get(&identity)
            .copied()
            .unwrap_or(symbol);
        let facts = self.reference_symbol_name_facts(symbol, consumer)?;
        let declarations = self.reference_symbol_declarations(symbol, consumer)?;
        if facts.key.fixed_bytes() == Some(b"default".as_slice()) && !context.flags.has(1 << 14) {
            if !context.flags.has(1 << 24) || declarations.is_empty() {
                return Ok("default".into());
            }
            if let Some(enclosing) = context.enclosing {
                let declaration = match declarations[0] {
                    ReferenceDeclaration::Captured(declaration) => declaration,
                    ReferenceDeclaration::Json { .. } => {
                        return Err(self.diagnostic_dependency(
                            consumer,
                            "native default export has no captured binding context",
                        ));
                    }
                };
                let declaration_context = self
                    .source
                    .diagnostic_default_binding_context(declaration)?;
                let enclosing_context = match self.diagnostic_captured_enclosing(Some(enclosing))? {
                    Some(declaration) => self
                        .source
                        .diagnostic_default_binding_context(declaration)?,
                    None => None,
                };
                if declaration_context != enclosing_context {
                    return Ok("default".into());
                }
            }
        }
        for declaration in &declarations {
            let ReferenceDeclaration::Captured(declaration) = *declaration else {
                return Err(self.diagnostic_dependency(consumer,
                    "native JSON symbol declaration-name serialization requires its retained declaration"));
            };
            let Some((_, name)) = self.source.declaration_name_site(declaration)? else {
                continue;
            };
            if name.kind == SyntaxKind::ComputedPropertyName
                && !facts.late
                && let Some(ty) = facts.name_type
                && matches!(
                    self.model.get(ty)?,
                    TypeView::Literal {
                        base: PrimitiveName::String | PrimitiveName::Number,
                        ..
                    }
                )
                && let Some(name) = self.symbol_name_from_name_type(ty, context, consumer)?
            {
                return Ok(name);
            }
            return self
                .source
                .diagnostic_written_name(declaration)?
                .ok_or_else(|| {
                    super::super::super::internal("selected symbol declaration lost its name")
                        .into()
                });
        }
        if let Some(ReferenceDeclaration::Captured(declaration)) = declarations.first()
            && let Some((name, anonymous)) =
                self.source.diagnostic_nameless_symbol_name(*declaration)?
        {
            if anonymous && !context.encountered_error && !context.flags.has(1 << 17) {
                context.encountered_error = true;
            }
            return Ok(name);
        }
        if let Some(ty) = facts.name_type
            && let Some(name) = self.symbol_name_from_name_type(ty, context, consumer)?
        {
            return Ok(name);
        }
        let native = self.reference_symbol_name(symbol, consumer)?;
        let bytes = if native.first() == Some(&0xfe) {
            [b"__".as_slice(), &native[1..]].concat()
        } else {
            native
        };
        String::from_utf8(bytes).map_err(|_| {
            self.diagnostic_dependency(
                consumer,
                "native symbol display name is not representable as exact text",
            )
        })
    }

    fn symbol_name_from_name_type(
        &mut self,
        ty: TypeId,
        context: &mut super::Context,
        consumer: &Location,
    ) -> Result<Option<String>, DemandError> {
        let TypeView::Literal {
            base: PrimitiveName::String | PrimitiveName::Number,
            value,
        } = self.model.get(ty)?
        else {
            return if self
                .model
                .has_type_flags(ty, super::TypeFlags::UNIQUE_SYMBOL)
            {
                let symbol = self
                    .reference_symbol_of_type(ty, consumer)?
                    .ok_or_else(|| {
                        super::super::super::internal("unique name type has no symbol")
                    })?;
                Ok(Some(format!(
                    "[{}]",
                    self.symbol_name_as_written_in_context(symbol, context, consumer)?
                )))
            } else {
                Ok(None)
            };
        };
        let value = self.literal_scalar_text(&value, consumer)?;
        let numeric = crate::numbers::is_numeric_literal_name(value);
        let name = if !crate::lexer::is_identifier_text(value) && !numeric {
            crate::printer::quoted_text(value, '"')
        } else if numeric && value.starts_with('-') {
            format!("[{value}]")
        } else {
            value.to_string()
        };
        Ok((!name.is_empty()).then_some(name))
    }

    /// `symbolToString` -> `SymbolToNode` -> `symbolToExpression`. Native
    /// default flags have no enclosing declaration or FullyQualified flag,
    /// so lookupSymbolChain returns this raw symbol without resolving aliases.
    /// The default also omits WriteTypeParametersInQualifiedName/WriteComputedProps.
    pub(in crate::project::semantic::evaluation) fn diagnostic_symbol_display(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<RenderedTypeName, DemandError> {
        self.diagnostic_symbol_display_completion(symbol, consumer)?
            .admit()
    }

    pub(in crate::project::semantic::evaluation) fn diagnostic_symbol_display_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> super::NativeResult<RenderedTypeName> {
        Ok(self
            .raw_diagnostic_symbol_display_completion(symbol, consumer)
            .map_err(|failure| {
                super::super::super::observe_recovery(self.inputs, failure, consumer)
            })?
            .observe(|failure| {
                super::super::super::observe_recovery(self.inputs, failure, consumer)
            }))
    }

    pub(in crate::project::semantic::evaluation) fn raw_diagnostic_symbol_display_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> super::NativeResult<RenderedTypeName> {
        let name = self.symbol_name_as_written(symbol, consumer)?;
        if name.starts_with(['\'', '"'])
            && self.diagnostic_external_module_symbol(symbol, consumer)?
        {
            return Ok(self
                .diagnostic_module_specifier_completion(
                    symbol,
                    &super::Context::new(super::DiagnosticTypeFormat(0), None, false),
                    None,
                    consumer,
                )?
                .map(|specifier| {
                    RenderedTypeName(crate::printer::quoted_text(&specifier, '"').into_bytes())
                }));
        }
        Ok(super::NativeCompletion::clean(RenderedTypeName(
            name.into_bytes(),
        )))
    }

    /// `symbolToStringEx` with AllowAnyNodeKind and an actual enclosing scope.
    /// Accessibility consumes both successful and failing diagnostic names;
    /// the expression builder retains their shared chain/name acquisitions.
    pub(in crate::project::semantic::evaluation) fn diagnostic_symbol_display_at_completion(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: Option<super::super::symbol_accessibility::ScopeLocation>,
        meaning: super::super::SymbolMeaning,
        consumer: &Location,
    ) -> super::NativeResult<RenderedTypeName> {
        let captured = self.diagnostic_captured_enclosing(enclosing)?;
        let mut context = super::Context::new(super::DiagnosticTypeFormat(0), captured, false);
        context.enclosing = enclosing;
        Ok(self
            .symbol_expression_node(symbol, meaning, &mut context, consumer)?
            .map(|node| RenderedTypeName(node.print())))
    }

    pub(super) fn symbol_type_display(
        &mut self,
        symbol: ReferenceSymbol,
        meaning: super::super::SymbolMeaning,
        consumer: &Location,
    ) -> Result<super::SymbolTypeDisplay, DemandError> {
        let mut context = super::Context::new(super::DiagnosticTypeFormat::DEFAULT, None, false);
        let node = self
            .symbol_type_node(symbol, meaning, None, &mut context, consumer)?
            .admit()?;
        let type_query = matches!(node, super::Node::Prefix("typeof ", _));
        Ok(super::SymbolTypeDisplay {
            name: RenderedTypeName(node.print()),
            type_query,
        })
    }

    pub(super) fn diagnostic_external_module_symbol(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            if self.accessibility_external_declaration(declaration)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `getSymbolIfSameReference` preserves merge-before-resolution and the
    /// final merge separately. Native unknownSymbol is the same symbol sentinel
    /// on both sides; no consumed failure is converted to that sentinel.
    pub(in crate::project::semantic::evaluation) fn get_symbol_if_same_reference_completion(
        &mut self,
        first: ReferenceSymbol,
        second: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let left = self.get_merged_reference_symbol(first, consumer)?;
            let left = sequence.read(self.resolve_reference_symbol_completion(left, consumer))?;
            let left = left
                .map(|symbol| self.get_merged_reference_symbol(symbol, consumer))
                .transpose()?;
            let right = self.get_merged_reference_symbol(second, consumer)?;
            let right = sequence.read(self.resolve_reference_symbol_completion(right, consumer))?;
            let right = right
                .map(|symbol| self.get_merged_reference_symbol(symbol, consumer))
                .transpose()?;
            let same = match (left, right) {
                (Some(left), Some(right)) => {
                    self.reference_symbol_identity(left, consumer)?
                        == self.reference_symbol_identity(right, consumer)?
                }
                (None, None) => true,
                _ => false,
            };
            Ok(same.then_some(first))
        })();
        sequence.finish(result)
    }

    pub(in crate::project::semantic::evaluation) fn diagnostic_property_symbol_display(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<RenderedTypeName, DemandError> {
        self.diagnostic_symbol_display(ReferenceSymbol::from_property(symbol), consumer)
    }

    pub(in crate::project::semantic::evaluation) fn raw_diagnostic_property_symbol_display_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> super::NativeResult<RenderedTypeName> {
        self.raw_diagnostic_symbol_display_completion(
            ReferenceSymbol::from_property(symbol),
            consumer,
        )
    }
}

impl Evaluator<'_, '_> {
    /// `lookupSymbolChainWorker` owns the native qualification gate. A chain
    /// search receives the same context through parent recursion; only the
    /// outer call is required to return a nonempty chain.
    pub(super) fn diagnostic_symbol_chain_completion(
        &mut self,
        symbol: ReferenceSymbol,
        meaning: super::super::SymbolMeaning,
        context: &mut super::Context,
        yield_module: bool,
        consumer: &Location,
    ) -> NativeResult<Vec<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            self.diagnostic_track_symbol(symbol, context.enclosing, meaning, context, consumer)?;
            if !self
                .reference_symbol_flags(symbol, consumer)?
                .type_parameter
                && (context.enclosing.is_some() || context.flags.has(1 << 6))
            {
                sequence.read(self.diagnostic_symbol_chain_worker_completion(
                    symbol,
                    meaning,
                    context,
                    true,
                    yield_module,
                    consumer,
                ))
            } else {
                Ok(vec![symbol])
            }
        })();
        sequence.finish(result)
    }

    /// Native node-builder `getSymbolChain`: accessible lexical routes precede
    /// container acquisition, module-name sorting, and parent qualification.
    fn diagnostic_symbol_chain_worker_completion(
        &mut self,
        symbol: ReferenceSymbol,
        meaning: super::super::SymbolMeaning,
        context: &super::Context,
        end: bool,
        yield_module: bool,
        consumer: &Location,
    ) -> NativeResult<Vec<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut chain = sequence.read(self.accessible_symbol_chain_completion(
                symbol,
                context.enclosing,
                meaning,
                context.flags.has(1 << 7),
                consumer,
            ))?;
            let qualifier = if chain.len() > 1 {
                meaning.qualifier()
            } else {
                meaning
            };
            if chain.is_empty()
                || sequence.read(self.symbol_needs_qualification_completion(
                    chain[0],
                    context.enclosing,
                    qualifier,
                    consumer,
                ))?
            {
                let root = chain.first().copied().unwrap_or(symbol);
                let parents = sequence.read(self.symbol_containers_completion(
                    root,
                    context.enclosing,
                    meaning,
                    consumer,
                ))?;
                let mut pairs = Vec::with_capacity(parents.len());
                for parent in parents {
                    let specifier = if self.diagnostic_external_module_symbol(parent, consumer)? {
                        sequence.read(self.diagnostic_module_specifier_completion(
                            parent, context, None, consumer,
                        ))?
                    } else {
                        String::new()
                    };
                    pairs.push((parent, specifier));
                }
                let mut ordered = Vec::new();
                if pairs.len() > 1 {
                    for (parent, name) in &pairs {
                        let property = self.register_reference_symbol(*parent, consumer)?;
                        ordered.push((
                            *parent,
                            name.clone(),
                            self.property_native_symbol(property, consumer)?,
                        ));
                    }
                }
                crate::project::binding::try_sort_by(&mut ordered, &|left, right| {
                    if !left.1.is_empty() && !right.1.is_empty() {
                        let relative_left = crate::project::program::paths::is_relative(&left.1);
                        let relative_right = crate::project::program::paths::is_relative(&right.1);
                        return Ok(if relative_left == relative_right {
                            crate::project::program::paths::count_path_components(&left.1).cmp(
                                &crate::project::program::paths::count_path_components(&right.1),
                            )
                        } else {
                            relative_left.cmp(&relative_right)
                        });
                    }
                    self.model
                        .compare_native_symbols(Some(&left.2), Some(&right.2))
                        .map_err(|failure| self.generic_binding_failure(failure, consumer))
                })?;
                if !ordered.is_empty() {
                    pairs = ordered
                        .into_iter()
                        .map(|(parent, name, _)| (parent, name))
                        .collect();
                }
                for (parent, _) in pairs {
                    let mut parent_chain =
                        sequence.read(self.diagnostic_symbol_chain_worker_completion(
                            parent,
                            meaning.qualifier(),
                            context,
                            false,
                            yield_module,
                            consumer,
                        ))?;
                    if parent_chain.is_empty() {
                        continue;
                    }
                    if let Some(exported) =
                        self.reference_own_module_export(parent, "export=", consumer)?
                        && sequence
                            .read(self.get_symbol_if_same_reference_completion(
                                exported, symbol, consumer,
                            ))?
                            .is_some()
                    {
                        chain = parent_chain;
                        break;
                    }
                    if chain.is_empty() {
                        let alias = sequence
                            .read(self.alias_for_symbol_in_container_completion(
                                parent, symbol, consumer,
                            ))?
                            .unwrap_or(symbol);
                        chain.push(alias);
                    }
                    parent_chain.extend(chain);
                    chain = parent_chain;
                    break;
                }
            }
            if !chain.is_empty() {
                return Ok(chain);
            }
            let flags = self.reference_symbol_flags(symbol, consumer)?;
            if end || !(flags.type_literal || flags.object_literal) {
                if !end
                    && !yield_module
                    && self.diagnostic_external_module_symbol(symbol, consumer)?
                {
                    return Ok(Vec::new());
                }
                return Ok(vec![symbol]);
            }
            Ok(Vec::new())
        })();
        sequence.finish(result)
    }
}
