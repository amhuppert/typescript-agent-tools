//! The checker's GetTypeAtLocation operation, shared by semantic consumers.
//! Product selection and graph presentation do not supply checker context.

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, SpecialName, TypeId,
};
use super::super::source::{ExpressionRoot, NodeTypeForm};
use super::contextual::CheckMode;
use super::{Evaluator, ReferenceSymbol, TypeState};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::NativeSymbolIdentity;
use crate::references::Location;

#[derive(Default)]
pub(super) struct NodeTypesState {
    type_checked: std::collections::HashSet<super::super::source::DeclarationIdentity>,
    pub(super) alias_values: std::collections::HashMap<NativeSymbolIdentity, TypeState>,
    alias_declared: std::collections::HashMap<NativeSymbolIdentity, NativeResult<TypeId>>,
}

impl NodeTypesState {
    /// `NodeLinks.flags & TypeChecked`: publication precedes diagnostic work;
    /// an interrupted report does not clear the native flag.
    pub(super) fn mark_type_checked(
        &mut self,
        node: super::super::source::DeclarationIdentity,
    ) -> bool {
        self.type_checked.insert(node)
    }

    pub(super) fn alias_has_value(&self, identity: NativeSymbolIdentity) -> bool {
        self.alias_values
            .get(&identity)
            .is_some_and(TypeState::has_type)
    }
}

impl Evaluator<'_, '_> {
    /// Native `getTypeOfNode` / `GetTypeAtLocation`. Class heritage establishes
    /// its declaring class before type-node checking, and applies that class's
    /// this argument after the actual type/base question. Declaration queries
    /// use getSymbolOfDeclaration rather than an expression/name lookup.
    pub(super) fn type_of_node_completion(&mut self, root: ExpressionRoot) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let consumer = super::super::location(self.inputs, root.file, root.site.span);
            let recipe = self.source.node_type_recipe(root)?;
            let class = if let Some(declaration) = recipe.class {
                let symbol =
                    sequence.read(self.reference_symbol_of_declaration_completion(declaration))?;
                Some(sequence.read(self.declared_reference_type_completion(symbol, &consumer))?)
            } else {
                None
            };
            match recipe.form {
                NodeTypeForm::Error => sequence.read(NativeCompletion::recovered(
                    self.model.native_error_type(),
                    self.node_type_error(&consumer),
                )),
                NodeTypeForm::Type(expression) => {
                    let ty = sequence.read(self.check_type_completion(&expression))?;
                    if let Some(class) = class {
                        let this = self.model.target_this_parameter(class)?;
                        sequence.read(
                            self.get_type_with_this_argument_completion(ty, this, false, &consumer),
                        )
                    } else {
                        Ok(ty)
                    }
                }
                NodeTypeForm::Expression(expression) => {
                    sequence.read(self.regular_type_of_expression_completion(&expression))
                }
                NodeTypeForm::ClassBase => {
                    let class = class.ok_or_else(|| {
                        super::super::internal("class-base node has no class prelude")
                    })?;
                    let Some(&base) = sequence
                        .read(self.base_types_completion(class, &consumer))?
                        .first()
                    else {
                        return sequence.read(NativeCompletion::recovered(
                            self.model.native_error_type(),
                            self.node_type_error(&consumer),
                        ));
                    };
                    let this = self.model.target_this_parameter(class)?;
                    sequence.read(
                        self.get_type_with_this_argument_completion(base, this, false, &consumer),
                    )
                }
                NodeTypeForm::Declared(declaration) | NodeTypeForm::DeclaredName(declaration) => {
                    let symbol = sequence
                        .read(self.reference_symbol_of_declaration_completion(declaration))?;
                    sequence.read(self.declared_reference_type_completion(symbol, &consumer))
                }
                NodeTypeForm::VariableLike(declaration) => sequence
                    .read(self.variable_like_declaration_type_completion(
                        declaration,
                        true,
                        CheckMode::NORMAL,
                    ))?
                    .ok_or_else(|| self.node_type_error(&consumer)),
                NodeTypeForm::Value(declaration) => {
                    let symbol = sequence
                        .read(self.reference_symbol_of_declaration_completion(declaration))?;
                    sequence.read(self.value_reference_type_completion(symbol, &consumer))
                }
                NodeTypeForm::ValueName {
                    declaration,
                    import_property,
                } => {
                    let mut symbol = sequence
                        .read(self.reference_symbol_of_declaration_completion(declaration))?;
                    if import_property {
                        symbol =
                            sequence
                                .read(self.reference_immediate_aliased_symbol_completion(
                                    symbol, &consumer,
                                ))?
                                .ok_or_else(|| self.node_type_error(&consumer))?;
                    }
                    sequence.read(self.value_reference_type_completion(symbol, &consumer))
                }
                NodeTypeForm::ImportExportRhs => {
                    let symbol = sequence
                        .read(self.reference_entity_name_symbol_completion(root, &consumer))?
                        .ok_or_else(|| self.node_type_error(&consumer))?;
                    let ty = sequence
                        .read(self.declared_reference_type_completion(symbol, &consumer))?;
                    if !self.model.is_error_type(ty) {
                        Ok(ty)
                    } else {
                        sequence.read(self.value_reference_type_completion(symbol, &consumer))
                    }
                }
                // TypeScript 7.0.2 `checkMetaPropertyKeyword` unconditionally
                // returns errorType; the whole meta-property is a separate query.
                NodeTypeForm::MetaKeyword(meta) => sequence.read(NativeCompletion::recovered(
                    self.model.native_error_type(),
                    self.node_type_error(&super::super::location(
                        self.inputs,
                        meta.file,
                        meta.site.span,
                    )),
                )),
                NodeTypeForm::ImportAttributes => {
                    sequence.read(self.global_object_type_completion("ImportAttributes", &consumer))
                }
            }
        })();
        sequence.finish(result)
    }

    /// `getDeclaredTypeOfSymbol` preserves the native no-declared-flags
    /// sentinel. The caller decides how to use that completed errorType;
    /// import/export RHS queries, for example, then ask for the value type.
    /// Actual alias/type recovery remains evidence, independently of identity.
    pub(super) fn declared_reference_type(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.declared_reference_type_completion(symbol, consumer)?
            .observe(|failure| super::super::observe_recovery(self.inputs, failure, consumer))
            .admit()
    }

    pub(super) fn declared_reference_type_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.try_declared_reference_type_completion(symbol, consumer)
            .map(|result| result.map(|ty| ty.unwrap_or_else(|| self.model.native_error_type())))
    }

    /// `tryGetDeclaredTypeOfSymbol` owns flag precedence and only the native
    /// final nil branch. Unavailable declaration families and interrupted
    /// acquisitions cannot become that ordinary absence result.
    fn try_declared_reference_type_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        // unknownSymbol has no declared flags, but unlike an ordinary value
        // symbol it was established by alias-resolution recovery. Its native
        // nil result cannot erase that failed input when a type is demanded.
        if self.is_unknown_reference_symbol(symbol) {
            return NativeCompletion::recovered(
                None,
                super::super::unbound_recovery(
                    crate::analysis::UnboundSemanticRecovery::UnknownSymbolDeclaredType,
                ),
            );
        }
        if let Some(ty) = symbol
            .property()
            .and_then(|symbol| self.unresolved_alias_declared_type(symbol))
        {
            return Ok(NativeCompletion::clean(Some(ty)));
        }
        let flags = self
            .reference_symbol_native_flags(symbol, consumer)?
            .type_flags();
        if flags.class
            || flags.interface
            || flags.type_parameter
            || flags.enum_
            || flags.enum_member
            || flags.type_alias
        {
            let family = self.declared_reference_family(symbol, consumer)?;
            return self
                .declared_type_completion(symbol, family)
                .map(|ty| ty.map(Some));
        }
        if flags.alias {
            self.declared_alias_reference_type_completion(symbol, consumer)
                .map(|ty| ty.map(Some))
        } else {
            Ok(NativeCompletion::clean(None))
        }
    }

    /// `getTypeOfSymbol`: native value flags precede Alias, including headers
    /// with both meanings. Property symbols keep their created value caches.
    pub(super) fn value_reference_type(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.value_reference_type_completion(symbol, consumer)?
            .admit()
    }

    pub(super) fn value_reference_type_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let symbol = self.reference_current_header(symbol);
        if let Some(property) = symbol.property() {
            return self.property_raw_type_completion(property, consumer);
        }
        let bound = self
            .reference_raw_origin(symbol)?
            .ok_or_else(|| super::super::internal("reference symbol lacks native association"))?
            .symbol();
        let flags = self.reference_symbol_flags(symbol, consumer)?;
        if !flags.has_value_type() {
            if flags.alias {
                return self.alias_reference_value_type_completion(symbol, consumer);
            }
            // The actual getTypeOfSymbol fall-through has a native
            // errorType result even without a reported diagnostic.
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                super::super::unbound_recovery(
                    crate::analysis::UnboundSemanticRecovery::NonValueSymbol,
                ),
            );
        }
        let family = self
            .source
            .bindings()
            .value_type(bound)
            .map_err(|failure| self.generic_binding_failure(failure, consumer))?;
        self.value_type_completion(bound, &family)
    }

    /// `getDeclaredTypeOfAlias` has its own declared links and does not push a
    /// value TypeResolution frame. The resolver owns alias cycles.
    fn declared_alias_reference_type_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        if let Some(result) = self.node_types.alias_declared.get(&identity) {
            return result.clone();
        }
        let result = self
            .resolve_alias_target_completion(symbol, consumer)
            .and_then(|target| {
                target.and_then(|target| match target {
                    Some(target) => self.declared_reference_type_completion(target, consumer),
                    None => Err(super::super::internal(
                        "resolveAlias did not establish its target symbol",
                    )
                    .into()),
                })
            });
        self.node_types
            .alias_declared
            .insert(identity, result.clone());
        result
    }

    /// `getTypeOfAlias` resolves both the final alias target and its immediate
    /// declaration target under the native TypeResolution frame. Type-only
    /// syntax affects runtime use, not this value-type inspection question.
    pub(super) fn alias_reference_value_type_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        match self.node_types.alias_values.get(&identity) {
            Some(TypeState::Installed(value)) => return Ok(value.clone()),
            Some(TypeState::Failed(failure)) => return Err(failure.clone()),
            Some(TypeState::Resolving) | None => {}
        }
        if !self.push_resolution(super::resolution::Key::AliasType(identity)) {
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                self.node_type_error(consumer),
            );
        }
        let mut export = None;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let target = sequence.read(self.resolve_alias_target_completion(symbol, consumer))?;
            export = sequence.read(self.checker_alias_target_completion(symbol, consumer))?;
            if let Some(TypeState::Installed(value)) = self.node_types.alias_values.get(&identity) {
                return sequence.read(Ok(value.clone()));
            }
            match target {
                Some(target)
                    if sequence
                        .read(self.reference_combined_symbol_flags_completion(target, consumer))?
                        .value =>
                {
                    sequence.read(self.value_reference_type_completion(target, consumer))
                }
                _ => sequence.read(NativeCompletion::recovered(
                    self.model.native_error_type(),
                    self.node_type_error(consumer),
                )),
            }
        })();
        let result = sequence.finish(result);
        // getTypeOfAlias installs before pop and circularity diagnostics.
        if !self
            .node_types
            .alias_values
            .get(&identity)
            .is_some_and(TypeState::has_type)
        {
            self.node_types.alias_values.insert(
                identity,
                match &result {
                    Ok(value) => TypeState::Installed(value.clone()),
                    Err(failure) => TypeState::Failed(failure.clone()),
                },
            );
        }
        let popped = self
            .resolution
            .pop()
            .ok_or_else(|| super::super::internal("alias type resolution frame missing"))?;
        if popped {
            return result;
        }
        // No diagnostic acquisition follows an interrupted native operation.
        result.and_then(|value| {
            value.and_then(|ty| {
                let target = match export {
                    Some(target) => target,
                    None => symbol,
                };
                self.report_symbol_circularity_completion(target, consumer)
                    .map(|report| report.map(|_| ty))
            })
        })
    }

    /// `reportCircularityError` acquires diagnostic arguments at their native
    /// gate, then returns errorType for an annotated ValueDeclaration and
    /// anyType otherwise. The caller owns installation of that actual value.
    pub(super) fn report_symbol_circularity_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut annotated = false;
        let report = if let Some(declaration) =
            self.reference_symbol_value_declaration(symbol, consumer)?
        {
            let super::ReferenceDeclaration::Captured(declaration) = declaration else {
                return Err(self.access_pending(
                    consumer,
                    "JSON circularity declaration requires its native syntax",
                ));
            };
            let (annotation, diagnostic_initializer) =
                self.source.circularity_declaration(declaration)?;
            annotated = annotation;
            annotation || self.inputs.semantic_options().no_implicit_any && diagnostic_initializer
        } else {
            self.reference_symbol_flags(symbol, consumer)?.alias
                && !self
                    .reference_symbol_declarations(symbol, consumer)?
                    .is_empty()
        };
        let display = if report {
            self.diagnostic_symbol_display_completion(symbol, consumer)?
                .map(|_| ())
        } else {
            NativeCompletion::clean(())
        };
        display.and_then(|_| {
            let ty = if annotated {
                self.model.native_error_type()
            } else {
                self.model.special(SpecialName::Any)
            };
            NativeCompletion::recovered(
                ty,
                super::super::invalid_at_location(
                    self.inputs,
                    consumer,
                    SemanticFailureReason::CircularType,
                    "symbol type consumes native circular type-resolution recovery",
                )
                .into(),
            )
        })
    }

    pub(super) fn node_type_error(&self, consumer: &Location) -> DemandError {
        super::super::invalid_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::ErrorType,
            "native GetTypeAtLocation consumed an error type",
        )
        .into()
    }
}

impl Evaluator<'_, '_> {
    pub(super) fn type_of_node(&mut self, root: ExpressionRoot) -> Result<TypeId, DemandError> {
        self.type_of_node_completion(root)?.admit()
    }
}
