//! `getTypeFromImportTypeNode`, its `getIdentifierChain` lookups and
//! `resolveImportSymbolType`: one installed result per actual ImportType
//! node, whose symbol links serve references and checks of that node.

use super::super::model::{DemandError, NativeCompletion, NativeResult, NativeSequence, TypeId};
use super::super::source::{ExpressionRoot, ImportTypeRecipe, TypeExpression};
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::{NativeLookupMeaning, NativePropertyKey};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct ImportTypesState {
    /// `typeNodeLinks.resolvedType` of each ImportType node.
    nodes: HashMap<ExpressionRoot, NativeResult<TypeId>>,
    /// `symbolNodeLinks.resolvedSymbol` this operation installs on each
    /// qualifier name, that name's parent, and the ImportType node.
    symbols: HashMap<ExpressionRoot, ReferenceSymbol>,
}

impl Evaluator<'_, '_> {
    /// `getTypeFromImportTypeNode` installs one result per actual node.
    pub(super) fn import_type_node_completion(
        &mut self,
        expression: &TypeExpression,
        root: ExpressionRoot,
    ) -> NativeResult<TypeId> {
        if let Some(result) = self.import_types.nodes.get(&root) {
            return result.clone();
        }
        let result = self.import_type_worker(expression, root);
        self.import_types.nodes.insert(root, result.clone());
        result
    }

    /// `getResolvedSymbolOrNil` of a qualifier name or the ImportType node,
    /// as `getTypeFromImportTypeNode` left it. The unknown symbol is nil.
    pub(super) fn import_type_symbol_completion(
        &mut self,
        import: &TypeExpression,
        name: ExpressionRoot,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        Ok(self.check_type_completion(import)?.map(|_| {
            self.import_types
                .symbols
                .get(&name)
                .copied()
                .filter(|&symbol| !self.is_unknown_reference_symbol(symbol))
        }))
    }

    /// The installed node link of an ImportType whose type was acquired.
    pub(super) fn resolved_import_type_symbol(
        &self,
        root: ExpressionRoot,
    ) -> Option<ReferenceSymbol> {
        self.import_types.symbols.get(&root).copied()
    }

    fn import_type_worker(
        &mut self,
        expression: &TypeExpression,
        root: ExpressionRoot,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.import_type_recipe(root)?;
            let consumer = &expression.location;
            let module = match &recipe.module {
                Ok(module) => *module,
                Err(argument) => {
                    // String_literal_expected: the argument names no module.
                    let unknown = self.unknown_reference_symbol();
                    self.import_types.symbols.insert(root, unknown);
                    return sequence.read(NativeCompletion::recovered(
                        self.model.native_error_type(),
                        super::super::invalid_at_location(
                            self.inputs,
                            argument,
                            SemanticFailureReason::ErrorType,
                            "an import type's argument must be a string literal",
                        )
                        .into(),
                    ));
                }
            };
            let target = if recipe.is_type_of {
                NativeLookupMeaning::VALUE
            } else {
                NativeLookupMeaning::TYPE
            };
            // resolveExternalModuleName: a module the loader could not
            // resolve fails at its specifier, as it does for an import.
            let inner = self.reference_module_symbol(module, consumer)?;
            let module_symbol = sequence
                .read(self.reference_external_module_symbol_completion(inner, false, consumer))?
                .ok_or_else(|| {
                    super::super::internal("resolveExternalModuleSymbol lost its module")
                })?;
            if recipe.qualifier.is_empty() {
                let flags = sequence.read(self.reference_symbol_flags_ex_completion(
                    module_symbol,
                    false,
                    false,
                    consumer,
                ))?;
                if target.matches(flags.native) {
                    return sequence.read(self.resolve_import_symbol_type_completion(
                        expression,
                        root,
                        &recipe,
                        module_symbol,
                    ));
                }
                let specifier = self.inputs.bindings.reference_module_specifier(module);
                let message = if recipe.is_type_of {
                    format!(
                        "module `{specifier}` does not refer to a value, but is used as a value here"
                    )
                } else {
                    format!(
                        "module `{specifier}` does not refer to a type, but is used as a type here"
                    )
                };
                let unknown = self.unknown_reference_symbol();
                self.import_types.symbols.insert(root, unknown);
                return sequence.read(NativeCompletion::recovered(
                    self.model.native_error_type(),
                    super::super::invalid_at_location(
                        self.inputs,
                        &recipe.site.location,
                        SemanticFailureReason::ErrorType,
                        message,
                    )
                    .into(),
                ));
            }
            let last = recipe.qualifier.len() - 1;
            let mut current = module_symbol;
            for (index, part) in recipe.qualifier.iter().enumerate() {
                let consumer = &part.location;
                let meaning = if index == last {
                    target
                } else {
                    NativeLookupMeaning::NAMESPACE
                };
                let resolved = sequence
                    .read(self.resolve_reference_symbol_completion(current, consumer))?
                    .ok_or_else(|| super::super::internal("resolveSymbol returned native nil"))?;
                let namespace = self.get_merged_reference_symbol(resolved, consumer)?;
                let next = if recipe.is_type_of {
                    // `typeof a.b.c` reads each name as a property of the
                    // previous value's type, as checkQualifiedName does.
                    let ty =
                        sequence.read(self.value_reference_type_completion(namespace, consumer))?;
                    sequence
                        .read(self.property_of_type_by_key_flags_completion(
                            ty,
                            &NativePropertyKey::fixed(part.name.as_bytes()),
                            false,
                            true,
                            consumer,
                        ))?
                        .map(|property| {
                            self.property_symbol(&property, consumer)
                                .map(ReferenceSymbol::from_property)
                        })
                        .transpose()?
                } else {
                    let found = sequence.read(
                        self.entity_export_completion(namespace, &part.name, meaning, consumer),
                    )?;
                    match found {
                        Some(found) => Some(found),
                        None => sequence.read(self.commonjs_type_exports_completion(
                            inner, &part.name, meaning, consumer,
                        ))?,
                    }
                };
                let Some(next) = next else {
                    // Namespace_0_has_no_exported_member_1 at this name.
                    let missing = self
                        .source
                        .bindings()
                        .missing_reference_at(part.root.file, part.root.site.span)
                        .map_err(|error| self.generic_binding_failure(error, consumer))?;
                    return sequence.read(NativeCompletion::recovered(
                        self.model.native_error_type(),
                        self.generic_binding_failure(missing, consumer),
                    ));
                };
                self.import_types.symbols.insert(part.root, next);
                self.import_types.symbols.insert(part.parent, next);
                current = next;
            }
            sequence.read(
                self.resolve_import_symbol_type_completion(expression, root, &recipe, current),
            )
        })();
        sequence.finish(result)
    }

    /// `resolveImportSymbolType`: the node's link is the resolved symbol. A
    /// value reads the unresolved symbol's type, which an alias caches as its
    /// own, then instantiates it with the node's type arguments.
    fn resolve_import_symbol_type_completion(
        &mut self,
        expression: &TypeExpression,
        root: ExpressionRoot,
        recipe: &ImportTypeRecipe,
        symbol: ReferenceSymbol,
    ) -> NativeResult<TypeId> {
        let consumer = &expression.location;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let resolved = sequence
                .read(self.resolve_reference_symbol_completion(symbol, consumer))?
                .ok_or_else(|| super::super::internal("resolveSymbol returned native nil"))?;
            self.import_types.symbols.insert(root, resolved);
            if !recipe.is_type_of && self.is_unknown_reference_symbol(resolved) {
                return sequence.read(NativeCompletion::recovered(
                    self.model.native_error_type(),
                    self.unknown_type_target_failure(consumer),
                ));
            }
            if recipe.is_type_of {
                let ty = sequence.read(self.value_reference_type_completion(symbol, consumer))?;
                return sequence
                    .read(self.instantiation_expression_type_completion(ty, &recipe.site));
            }
            sequence.read(self.type_reference_type_completion(
                expression,
                resolved,
                None,
                &recipe.type_arguments(),
                recipe.alias.as_ref(),
                None,
            ))
        })();
        sequence.finish(result)
    }

    /// The CommonJS fallback for typedefs exported beside a JavaScript
    /// `module.exports =` (`JSDeclarationKindModuleExports`): a name the
    /// `export=` target does not export is looked up among the exports of
    /// the `export=` alias's own parent, the module.
    fn commonjs_type_exports_completion(
        &mut self,
        inner: ReferenceSymbol,
        name: &str,
        meaning: NativeLookupMeaning,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<ReferenceSymbol>, DemandError> {
            let Some(immediate) = sequence
                .read(self.reference_external_module_symbol_completion(inner, true, consumer))?
            else {
                return Ok(None);
            };
            // Of the alias declarations an `export=` has, only binding's
            // `module.exports = name` is an assignment.
            let mut commonjs = false;
            for declaration in self.reference_symbol_declarations(immediate, consumer)? {
                if let ReferenceDeclaration::Captured(declaration) = declaration
                    && self.source.declaration_kind(declaration)? == SyntaxKind::BinaryExpression
                {
                    commonjs = true;
                }
            }
            if !commonjs {
                return Ok(None);
            }
            let Some(module) = self.reference_raw_parent_symbol(immediate, consumer)? else {
                return Ok(None);
            };
            sequence.read(self.entity_export_completion(module, name, meaning, consumer))
        })();
        sequence.finish(result)
    }
}
