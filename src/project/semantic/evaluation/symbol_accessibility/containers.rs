//! Native container discovery preserves raw symbol identities, export-table
//! acquisition, and the two distinct alternative-container cache policies.

use super::*;
use crate::project::semantic::model::TypeFlags;
use crate::project::semantic::source::reference_import_tracker::ImportNode;
use crate::project::semantic::source::symbol_accessibility::ContainerDeclaration;

impl Evaluator<'_, '_> {
    /// `getContainersOfSymbol` asks the late/merged parent owner first. Its
    /// declaration fallback checks assigned receivers only when native does.
    pub(in crate::project::semantic::evaluation) fn symbol_containers_completion(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: Option<ScopeLocation>,
        meaning: SymbolMeaning,
        consumer: &Location,
    ) -> NativeResult<Vec<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let parent =
                sequence.read(self.reference_parent_of_symbol_completion(symbol, consumer))?;
            if let Some(parent) = parent
                && !self
                    .reference_symbol_flags(symbol, consumer)?
                    .type_parameter
            {
                return sequence.read(self.with_alternative_symbol_containers_completion(
                    parent, symbol, enclosing, meaning, consumer,
                ));
            }
            let mut candidates = Vec::new();
            for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                let ReferenceDeclaration::Captured(declaration) = declaration else {
                    continue;
                };
                match self
                    .source
                    .accessibility_container_declaration(declaration)?
                {
                    ContainerDeclaration::None => {}
                    ContainerDeclaration::Parent {
                        parent,
                        module_block,
                    } => {
                        if self.source.diagnostic_external_module_declaration(parent)? {
                            let parent = sequence
                                .read(self.reference_symbol_of_declaration_completion(parent))?;
                            self.push_unique_container(&mut candidates, parent, consumer)?;
                            continue;
                        }
                        if let Some(module) = module_block {
                            let module = sequence
                                .read(self.reference_symbol_of_declaration_completion(module))?;
                            if let Some(resolved) =
                                sequence.read(self.reference_external_module_symbol_completion(
                                    module, false, consumer,
                                ))?
                                && self.reference_symbol_identity(resolved, consumer)?
                                    == self.reference_symbol_identity(symbol, consumer)?
                            {
                                self.push_unique_container(&mut candidates, module, consumer)?;
                            }
                        }
                    }
                    ContainerDeclaration::AssignedClass {
                        receiver,
                        module_exports,
                    } => {
                        if module_exports {
                            if let Some(module) =
                                self.reference_source_module_symbol(declaration.file)
                            {
                                self.push_unique_container(&mut candidates, module, consumer)?;
                            }
                            continue;
                        }
                        sequence.read(
                            self.check_expression_cached_completion(receiver.file, receiver.site),
                        )?;
                        if let Some(symbol) = self.reference_resolved_symbol_if_any(receiver)? {
                            self.push_unique_container(&mut candidates, symbol, consumer)?;
                        }
                    }
                }
            }
            let mut best = Vec::new();
            let mut alternatives = Vec::new();
            for container in candidates {
                if sequence
                    .read(
                        self.alias_for_symbol_in_container_completion(container, symbol, consumer),
                    )?
                    .is_none()
                {
                    continue;
                }
                let choices = sequence.read(self.with_alternative_symbol_containers_completion(
                    container, symbol, enclosing, meaning, consumer,
                ))?;
                let mut choices = choices.into_iter();
                if let Some(first) = choices.next() {
                    best.push(first);
                    alternatives.extend(choices);
                }
            }
            best.extend(alternatives);
            Ok(best)
        })();
        sequence.finish(result)
    }

    fn push_unique_container(
        &self,
        result: &mut Vec<ReferenceSymbol>,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        for existing in result.iter() {
            if self.reference_symbol_identity(*existing, consumer)? == identity {
                return Ok(());
            }
        }
        result.push(symbol);
        Ok(())
    }

    /// `getAliasForSymbolInContainer`: parent identity precedes raw export=,
    /// resolved exports, exact-name lookup, then the complete sorted scan.
    pub(in crate::project::semantic::evaluation) fn alias_for_symbol_in_container_completion(
        &mut self,
        container: ReferenceSymbol,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(parent) =
                sequence.read(self.reference_parent_of_symbol_completion(symbol, consumer))?
                && self.reference_symbol_identity(container, consumer)?
                    == self.reference_symbol_identity(parent, consumer)?
            {
                return Ok(Some(symbol));
            }
            if let Some(export) =
                self.reference_own_module_export(container, "export=", consumer)?
                && sequence
                    .read(self.get_symbol_if_same_reference_completion(export, symbol, consumer))?
                    .is_some()
            {
                return Ok(Some(container));
            }
            let exports = sequence
                .read(self.reference_exports_of_symbol_completion(container, consumer))?
                .unwrap_or_else(|| Arc::from([]));
            let key = self.reference_symbol_key(symbol, consumer)?;
            if let Some(quick) = exports
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, symbol)| *symbol)
                && sequence
                    .read(self.get_symbol_if_same_reference_completion(quick, symbol, consumer))?
                    .is_some()
            {
                return Ok(Some(quick));
            }
            let mut candidates = Vec::new();
            for (_, exported) in exports.iter() {
                if sequence
                    .read(
                        self.get_symbol_if_same_reference_completion(*exported, symbol, consumer),
                    )?
                    .is_some()
                {
                    candidates.push(vec![*exported]);
                }
            }
            self.sort_accessibility_chains(&mut candidates, consumer)?;
            Ok(candidates.first().map(|chain| chain[0]))
        })();
        sequence.finish(result)
    }

    /// `getWithAlternativeContainers` owns preference order and the late
    /// value/declared-type equality probe for instance-side namespace values.
    fn with_alternative_symbol_containers_completion(
        &mut self,
        container: ReferenceSymbol,
        symbol: ReferenceSymbol,
        enclosing: Option<ScopeLocation>,
        meaning: SymbolMeaning,
        consumer: &Location,
    ) -> NativeResult<Vec<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut additional = Vec::new();
            for declaration in self.reference_symbol_declarations(container, consumer)? {
                if let Some(file) =
                    sequence.read(self.file_symbol_if_export_equals_container_completion(
                        declaration,
                        container,
                        consumer,
                    ))?
                {
                    additional.push(file);
                }
            }
            let reexports = match enclosing {
                Some(enclosing) => sequence.read(
                    self.alternative_containing_modules_completion(symbol, enclosing, consumer),
                )?,
                None => Vec::new(),
            };
            let object_variable = sequence.read(
                self.object_literal_variable_container_completion(container, meaning, consumer),
            )?;
            let left = meaning.qualifier();
            let flags = self.reference_symbol_flags(container, consumer)?;
            if enclosing.is_some()
                && left.matches(flags)
                && !sequence
                    .read(self.accessible_symbol_chain_completion(
                        container,
                        enclosing,
                        SymbolMeaning::Namespace,
                        false,
                        consumer,
                    ))?
                    .is_empty()
            {
                let mut result = vec![container];
                result.extend(additional);
                result.extend(reexports);
                result.extend(object_variable);
                return Ok(result);
            }
            let mut variables = Vec::new();
            if meaning == SymbolMeaning::Value && !left.matches(flags) && flags.type_ {
                let declared =
                    sequence.read(self.declared_reference_type_completion(container, consumer))?;
                if self.model.has_type_flags(declared, TypeFlags::OBJECT) {
                    sequence.read(self.with_scope_tables_completion(
                        enclosing,
                        consumer,
                        |this, table| {
                            let mut sequence = NativeSequence::new();
                            let result = (|| {
                                let mut found = false;
                                for (_, candidate) in table.entries.iter() {
                                    if left
                                        .matches(this.reference_symbol_flags(*candidate, consumer)?)
                                    {
                                        let value =
                                            sequence.read(this.value_reference_type_completion(
                                                *candidate, consumer,
                                            ))?;
                                        let declared = sequence.read(
                                            this.declared_reference_type_completion(
                                                container, consumer,
                                            ),
                                        )?;
                                        if declared == value {
                                            variables.push(vec![*candidate]);
                                            found = true;
                                        }
                                    }
                                }
                                Ok(found.then_some(()))
                            })();
                            sequence.finish(result)
                        },
                    ))?;
                    self.sort_accessibility_chains(&mut variables, consumer)?;
                }
            }
            let mut result: Vec<_> = variables.into_iter().map(|chain| chain[0]).collect();
            result.extend(additional);
            result.push(container);
            result.extend(object_variable);
            result.extend(reexports);
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `getVariableDeclarationOfObjectLiteral` observes only the actual first
    /// declaration and its exact variable initializer/type-node relationship.
    fn object_literal_variable_container_completion(
        &mut self,
        symbol: ReferenceSymbol,
        meaning: SymbolMeaning,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if meaning != SymbolMeaning::Value {
                return Ok(None);
            }
            let declarations = self.reference_symbol_declarations(symbol, consumer)?;
            let Some(ReferenceDeclaration::Captured(first)) = declarations.first() else {
                return Ok(None);
            };
            let Some(variable) = self.source.accessibility_object_variable(*first)? else {
                return Ok(None);
            };
            sequence
                .read(self.reference_symbol_of_declaration_completion(variable))
                .map(Some)
        })();
        sequence.finish(result)
    }

    /// `getAlternativeContainingModules` caches nonempty per-file import
    /// results, but caches the program-wide fallback even when it is nil.
    /// Failures remain retained for either acquired operation.
    fn alternative_containing_modules_completion(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: ScopeLocation,
        consumer: &Location,
    ) -> NativeResult<Vec<ReferenceSymbol>> {
        let enclosing = self
            .diagnostic_captured_enclosing(Some(enclosing))?
            .ok_or_else(|| {
                super::super::super::internal("native display scope has no containing source file")
            })?;
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        let key = (identity, enclosing.file);
        if let Some(result) = self.symbol_accessibility.containers_by_file.get(&key) {
            return result.clone();
        }
        let mut sequence = NativeSequence::new();
        let local = (|| {
            let mut result = Vec::new();
            for (site, node) in self.source.reference_source_imports(enclosing.file)? {
                if matches!(node, ImportNode::Implicit(..)) {
                    continue;
                }
                let Some(module) = self.reference_module_symbol_ignore_errors(site, consumer)?
                else {
                    continue;
                };
                if sequence
                    .read(self.alias_for_symbol_in_container_completion(module, symbol, consumer))?
                    .is_some()
                {
                    result.push(module);
                }
            }
            Ok(result)
        })();
        let local = match sequence.finish(local) {
            Err(error) => {
                self.symbol_accessibility
                    .containers_by_file
                    .insert(key, Err(error.clone()));
                return Err(error);
            }
            Ok(result) if !result.value().is_empty() => {
                self.symbol_accessibility
                    .containers_by_file
                    .insert(key, Ok(result.clone()));
                return Ok(result);
            }
            Ok(empty) => empty,
        };
        // Native does not install empty per-file alternatives. Their acquired
        // recovery still precedes the separately cached program-wide result.
        local.and_then(|_| {
            if let Some(result) = self.symbol_accessibility.containers.get(&identity) {
                return result.clone();
            }
            let mut sequence = NativeSequence::new();
            let result = (|| {
                let mut result = Vec::new();
                let inputs = self.inputs;
                for file in inputs.sources.ids() {
                    if !self.inputs.bindings.reference_file_is_external(file) {
                        continue;
                    }
                    let module = self.reference_source_module_symbol(file).ok_or_else(|| {
                        super::super::super::internal(
                            "external source file has no native module symbol",
                        )
                    })?;
                    if sequence
                        .read(
                            self.alias_for_symbol_in_container_completion(module, symbol, consumer),
                        )?
                        .is_some()
                    {
                        result.push(module);
                    }
                }
                Ok(result)
            })();
            let result = sequence.finish(result);
            self.symbol_accessibility
                .containers
                .insert(identity, result.clone());
            result
        })
    }
}
