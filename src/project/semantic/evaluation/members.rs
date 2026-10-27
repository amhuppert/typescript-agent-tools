//! Canonical ordinary member value operations within the shared Evaluator.

use super::super::model::MapperId;
use super::{Evaluator, TypeState, resolution};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::{
    BoundDeclaration, BoundMember, BoundMemberKind, BoundSymbol, BoundValueType,
};
use crate::project::semantic::model::{
    AccessorDisplay, DemandError, NativeCompletion, NativeResult, PropertyDisplay,
    PropertySerializationId, PropertySymbolId, SpecialName, TypeDemand, TypeId,
};
use crate::project::semantic::source::DeclarationReference;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use std::collections::HashMap;

pub(super) struct MemberValuesState {
    pub(super) symbols: super::property_symbols::PropertySymbols,
    pub(super) writes: HashMap<BoundSymbol, TypeState>,
    serialization_ids: HashMap<PropertySymbolId, PropertySerializationId>,
    serialization: Vec<Serialization>,
}

impl MemberValuesState {
    /// Request-local backing for the existing native cache questions. Their
    /// identities, child values and provisional publication stay operation-owned.
    pub(super) fn new() -> Self {
        Self {
            symbols: super::property_symbols::PropertySymbols::new(),
            writes: HashMap::new(),
            serialization_ids: HashMap::new(),
            serialization: Default::default(),
        }
    }
}

#[derive(Clone)]
struct Serialization {
    property_symbol: PropertySymbolId,
    consumer: Location,
}

struct SerializationQuestion {
    property_symbol: PropertySymbolId,
    readonly: bool,
    mapper: Option<MapperId>,
    consumer: Location,
}

/// One native value-symbol identity; copied declarations do not alter it.
#[derive(Clone, Copy)]
enum ValueSymbol {
    Bound(BoundSymbol),
    Created(PropertySymbolId),
}

enum ValueDeclaration {
    Variable(BoundDeclaration),
    Property(DeclarationReference),
    Json {
        file: crate::project::sources::FileId,
        declaration: crate::project::binding::JsonDeclaration,
    },
}

impl Evaluator<'_, '_> {
    /// `initializeChecker` publishes createArrayType(autoType), including its
    /// unique empty-object fallback marker. Observing this field does not
    /// repeat the global Array factory or consume its recovery.
    pub(super) fn automatic_array_type(
        &mut self,
        _consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.model.automatic_array().ok_or_else(|| {
            super::super::internal("autoArrayType identity was observed before initialization")
                .into()
        })
    }

    /// `convertAutoToAny` keeps scalar and array marker identities distinct.
    pub(super) fn convert_auto_to_any(
        &mut self,
        ty: TypeId,
        _consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        if self.model.is_automatic_array(ty) {
            return self.installed_any_array_type();
        }
        if self.model.is_automatic(ty) {
            return Ok(self.model.special(super::super::model::SpecialName::Any));
        }
        Ok(ty)
    }
    fn value_symbol_state(
        &self,
        symbol: ValueSymbol,
        writing: bool,
    ) -> Result<Option<TypeState>, DemandError> {
        match symbol {
            ValueSymbol::Bound(bound) => Ok(if writing {
                self.native_members.writes.get(&bound)
            } else {
                self.value_types.get(&bound)
            }
            .cloned()),
            ValueSymbol::Created(symbol) => self.property_value_state(symbol, writing),
        }
    }

    fn set_value_symbol_state(
        &mut self,
        symbol: ValueSymbol,
        writing: bool,
        state: TypeState,
    ) -> Result<(), DemandError> {
        match symbol {
            ValueSymbol::Bound(bound) => {
                if writing {
                    self.native_members.writes.insert(bound, state);
                } else {
                    self.value_types.insert(bound, state);
                }
                Ok(())
            }
            ValueSymbol::Created(symbol) => self.set_property_value_state(symbol, writing, state),
        }
    }

    fn push_value_resolution(
        &mut self,
        symbol: ValueSymbol,
        property: resolution::Property,
    ) -> bool {
        let key = match symbol {
            ValueSymbol::Bound(target) => resolution::Key::Symbol { target, property },
            ValueSymbol::Created(target) => resolution::Key::PropertySymbol { target, property },
        };
        self.push_resolution(key)
    }

    /// `getTypeOfAccessors` / `getWriteTypeOfAccessors` retain the actual
    /// declaration list before projecting getter/setter recipes.
    fn value_symbol_declarations(
        &mut self,
        symbol: ValueSymbol,
        consumer: &Location,
    ) -> Result<Vec<DeclarationReference>, DemandError> {
        match symbol {
            ValueSymbol::Bound(symbol) => self
                .source
                .bindings()
                .native_declarations(symbol)
                .map_err(|error| self.generic_binding_failure(error, consumer))
                .map(|declarations| {
                    (declarations.into_iter().map(DeclarationReference::Bound)).collect::<Vec<_>>()
                }),
            ValueSymbol::Created(symbol) => self
                .property_accessor_syntax_declarations(symbol, consumer)
                .map(|declarations| {
                    (declarations.into_iter().map(DeclarationReference::Captured))
                        .collect::<Vec<_>>()
                }),
        }
    }

    /// `isParameterOfContextSensitiveSignature` consumes the actual
    /// ValueDeclaration, including copied declarations on fresh symbol clones.
    pub(super) fn is_parameter_of_context_sensitive_signature(
        &mut self,
        symbol: super::ReferenceSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        let Some(super::ReferenceDeclaration::Captured(declaration)) =
            self.reference_symbol_value_declaration(symbol, consumer)?
        else {
            return Ok(false);
        };
        let Some(root) = self
            .source
            .parameter_declaration_context_sensitive_root(declaration)?
        else {
            return Ok(false);
        };
        self.source.expression_is_context_sensitive(root)
    }

    /// Native completion preserves a property installed during re-entry.
    /// A failed acquisition does not become a typeResolutionHasProperty barrier.
    fn finish_value_symbol(
        &mut self,
        symbol: ValueSymbol,
        result: NativeResult<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self
            .value_symbol_state(symbol, false)?
            .is_some_and(|state| state.has_type())
        {
            return result;
        }
        let result = result.and_then(|value| {
            value.and_then_value(|ty| {
                let cache = if let ValueSymbol::Created(created) = symbol {
                    let flags = self.property_symbol_flags(created)?;
                    !(flags.variable || flags.property)
                        || !self.is_parameter_of_context_sensitive_signature(
                            super::ReferenceSymbol::from_property(created),
                            consumer,
                        )?
                } else {
                    true
                };
                Ok((ty, cache))
            })
        });
        match result {
            Ok(value) => {
                let cache = value.value().1;
                let value = value.map(|(ty, _)| ty);
                if cache {
                    self.set_value_symbol_state(
                        symbol,
                        false,
                        TypeState::Installed(value.clone()),
                    )?;
                } else if let ValueSymbol::Created(created) = symbol {
                    self.clear_property_value_state(created, false)?;
                }
                Ok(value)
            }
            Err(error) => {
                self.set_value_symbol_state(symbol, false, TypeState::Failed(error.clone()))?;
                Err(error)
            }
        }
    }

    /// `getTypeOfSymbol` for a declaration-backed checker symbol consumes its
    /// own resolution/cache identity, including copied declaration recipes.
    pub(super) fn created_property_value_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.created_property_value_worker(symbol, consumer)
            .map(|value| {
                value.observe(|failure| {
                    failure.at_property(super::super::model::PropertyValueQuestion::Read(symbol))
                })
            })
            .map_err(|failure| {
                failure.at_property(super::super::model::PropertyValueQuestion::Read(symbol))
            })
    }

    fn created_property_value_worker(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let value_symbol = ValueSymbol::Created(symbol);
        match self.value_symbol_state(value_symbol, false)? {
            Some(TypeState::Installed(value)) => return Ok(value),
            Some(TypeState::Failed(error)) => return Err(error),
            Some(TypeState::Resolving) | None => {}
        }
        let result = match self.property_value_kind(symbol)? {
            Some(BoundMemberKind::Method) => {
                self.created_function_class_enum_module_type_completion(symbol, consumer)
            }
            Some(BoundMemberKind::Accessor) => self.accessor_read_value(value_symbol, consumer),
            Some(BoundMemberKind::Property) => {
                if self.property_symbol_flags(symbol)?.prototype {
                    let parent = self
                        .reference_parent_of_symbol(
                            super::ReferenceSymbol::from_property(symbol),
                            consumer,
                        )?
                        .ok_or_else(|| {
                            super::super::internal("prototype clone has no actual parent")
                        })?;
                    self.reference_prototype_property_type(parent, consumer)
                        .map(NativeCompletion::clean)
                } else {
                    let declaration = self
                        .property_reference_value_declaration(symbol, consumer)?
                        .ok_or_else(|| {
                            super::super::internal(
                                "variable/property clone has no ValueDeclaration",
                            )
                        })?;
                    let location = self.reference_declaration_location(declaration)?;
                    let declaration = match declaration {
                        super::ReferenceDeclaration::Captured(declaration) => {
                            ValueDeclaration::Property(declaration.into())
                        }
                        super::ReferenceDeclaration::Json { file, declaration } => {
                            ValueDeclaration::Json { file, declaration }
                        }
                    };
                    self.variable_or_property_value(value_symbol, declaration, &location)
                }
            }
            None => {
                let reference = super::ReferenceSymbol::from_property(symbol);
                let flags = self.reference_symbol_flags(reference, consumer)?;
                if flags.function || flags.class || flags.enum_ || flags.value_module {
                    self.created_function_class_enum_module_type_completion(symbol, consumer)
                } else if flags.enum_member {
                    self.declared_reference_type_completion(reference, consumer)
                } else if flags.alias {
                    self.alias_reference_value_type_completion(reference, consumer)
                } else {
                    NativeCompletion::recovered(
                        self.model.native_error_type(),
                        super::super::invalid_at_location(
                            self.inputs,
                            consumer,
                            SemanticFailureReason::ErrorType,
                            "native symbol value dispatch returns errorType",
                        )
                        .into(),
                    )
                }
            }
        };
        self.finish_value_symbol(value_symbol, result, consumer)
    }

    /// getTypeOfVariableOrParameterOrPropertyWorker shares actual ValueDeclaration
    /// dispatch and its Type frame between raw symbols and checker clones.
    pub(super) fn bound_variable_or_property_type_completion(
        &mut self,
        symbol: BoundSymbol,
        family: &BoundValueType,
    ) -> NativeResult<TypeId> {
        if let BoundValueType::Json { file, declaration } = *family {
            let consumer =
                self.reference_declaration_location(super::ReferenceDeclaration::Json {
                    file,
                    declaration,
                })?;
            return self.variable_or_property_value(
                ValueSymbol::Bound(symbol),
                ValueDeclaration::Json { file, declaration },
                &consumer,
            );
        }
        let (declaration, recipe) = match family {
            BoundValueType::Variable(declaration) => {
                (*declaration, ValueDeclaration::Variable(*declaration))
            }
            BoundValueType::Member(member) => (
                member.value_declaration,
                ValueDeclaration::Property(member.value_declaration.into()),
            ),
            _ => {
                return Err(super::super::internal(
                    "variable/property worker has another native family",
                )
                .into());
            }
        };
        let consumer = super::super::location(self.inputs, declaration.file, declaration.anchor);
        self.variable_or_property_value(ValueSymbol::Bound(symbol), recipe, &consumer)
    }

    fn variable_or_property_value(
        &mut self,
        symbol: ValueSymbol,
        declaration: ValueDeclaration,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        // Native JSON SourceFile checking precedes pushTypeResolution. A
        // cloned JSON PropertyAssignment instead uses the ordinary frame and
        // checks its copied ValueDeclaration's original initializer.
        if let ValueDeclaration::Json {
            file,
            declaration: crate::project::binding::JsonDeclaration::SourceFile,
        } = &declaration
        {
            return self.json_declaration_type_completion(
                *file,
                crate::project::binding::JsonDeclaration::SourceFile,
            );
        }
        let reference = match symbol {
            ValueSymbol::Bound(bound) => super::ReferenceSymbol::from_origin(
                crate::project::binding::ReadOrigin::Direct(bound),
            ),
            ValueSymbol::Created(created) => super::ReferenceSymbol::from_property(created),
        };
        // `ModuleExports`: a CommonJS module's own `module` or `exports`
        // (`declareCommonJSVariable`), whose type TypeScript makes of the
        // module, is refused at the file that declares it.
        if self
            .reference_symbol_flags(reference, consumer)?
            .module_exports
        {
            return Err(super::super::unsupported_demand_at_location(
                self.inputs,
                consumer,
                SemanticFailureReason::JavascriptSemanticsRequired,
                "a CommonJS module's `module` and `exports` have the module's JavaScript-only type",
            ));
        }
        if !self.push_value_resolution(symbol, resolution::Property::Type) {
            return self.report_symbol_circularity_completion(reference, consumer);
        }
        let result = (|| {
            self.set_value_symbol_state(symbol, false, TypeState::Resolving)?;
            match declaration {
                ValueDeclaration::Variable(declaration) => {
                    let declaration = self.source.declaration_identity(declaration)?;
                    self.widened_variable_like_type_completion(declaration)
                }
                ValueDeclaration::Property(declaration) => {
                    self.property_value_type_completion(declaration, consumer)
                }
                ValueDeclaration::Json { file, declaration } => {
                    self.json_declaration_type_completion(file, declaration)
                }
            }
        })();
        let resolved = self
            .resolution
            .pop()
            .ok_or_else(|| super::super::internal("missing value resolution frame"))?;
        // Native frame cleanup cannot replace an interrupted dependency
        // with circular recovery or launch another semantic read.
        if let Err(failure) = &result
            && failure.is_whole_operation_failure()
        {
            return Err(failure.clone());
        }
        if resolved {
            result
        } else {
            match result {
                Ok(value) => value
                    .and_then(|_| self.report_symbol_circularity_completion(reference, consumer)),
                // No actual native child result was established. Its error
                // remains an interruption, not permission to invent Any.
                Err(error) => Err(error),
            }
        }
    }

    pub(super) fn property_value_type_completion(
        &mut self,
        declaration: DeclarationReference,
        _consumer: &Location,
    ) -> NativeResult<TypeId> {
        let declaration = self.source.resolve_declaration_reference(declaration)?;
        match self.source.declaration_kind(declaration)? {
            SyntaxKind::PropertyAssignment | SyntaxKind::ShorthandPropertyAssignment => {
                let member = self.source.object_member_value_recipe(declaration)?;
                // The raw shorthand symbol worker uses inDestructuringPattern=true,
                // independent of its enclosing object's ordinary check mode.
                self.check_object_member_value_completion(
                    &member,
                    true,
                    super::contextual::CheckMode::NORMAL,
                    _consumer,
                )
            }
            _ => self.widened_variable_like_type_completion(declaration),
        }
    }

    fn accessor_circular(&self, consumer: &Location, annotation: Option<&Location>) -> DemandError {
        super::super::invalid_at_location(
            self.inputs,
            annotation.unwrap_or(consumer),
            SemanticFailureReason::CircularType,
            "native accessor resolution consumes its circular error/any recovery",
        )
        .into()
    }

    pub(super) fn accessor_read_type_completion(
        &mut self,
        symbol: BoundSymbol,
        member: &BoundMember,
    ) -> NativeResult<TypeId> {
        let consumer = super::super::location(
            self.inputs,
            member.value_declaration.file,
            member.value_declaration.anchor,
        );
        self.accessor_read_value(ValueSymbol::Bound(symbol), &consumer)
    }

    /// getTypeOfAccessors rejects a repeated Type push without installation;
    /// a marked pop installs recovery under the same actual symbol's cache.
    fn accessor_read_value(
        &mut self,
        symbol: ValueSymbol,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if !self.push_value_resolution(symbol, resolution::Property::Type) {
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                self.accessor_circular(consumer, None),
            );
        }
        let reference = match symbol {
            ValueSymbol::Bound(bound) => super::ReferenceSymbol::from_origin(
                crate::project::binding::ReadOrigin::Direct(bound),
            ),
            ValueSymbol::Created(created) => super::ReferenceSymbol::from_property(created),
        };
        let recipe = self
            .set_value_symbol_state(symbol, false, TypeState::Resolving)
            .and_then(|()| self.value_symbol_declarations(symbol, consumer))
            .and_then(|declarations| self.source.accessor_recipe(&declarations));
        let result = match &recipe {
            Ok(recipe) => (|| {
                for own in [&recipe.getter, &recipe.setter, &recipe.auto]
                    .into_iter()
                    .flatten()
                {
                    if let Some(annotation) = &own.annotation {
                        return self.check_type_completion(annotation);
                    }
                }
                if let Some(root) = recipe.getter.as_ref().and_then(|getter| getter.body) {
                    let body = self.source.return_body(root)?;
                    return self.return_body_type_completion(&body);
                }
                if let Some(auto) = &recipe.auto {
                    return self.widened_variable_like_type_completion(auto.declaration);
                }
                // Native evaluates SymbolToString even for suggestion-only
                // diagnostics. Private ambient members suppress this branch.
                for own in [&recipe.setter, &recipe.getter, &recipe.auto]
                    .into_iter()
                    .flatten()
                {
                    if !self.source.is_private_within_ambient(own.declaration)? {
                        let any = self.model.special(SpecialName::Any);
                        return self
                            .diagnostic_symbol_display_completion(reference, consumer)
                            .map(|value| value.map(|_| any));
                    }
                }
                Ok(NativeCompletion::clean(
                    self.model.special(SpecialName::Any),
                ))
            })(),
            Err(error) => Err(error.clone()),
        };
        let resolved = self
            .resolution
            .pop()
            .ok_or_else(|| super::super::internal("missing accessor Type resolution frame"))?;
        if result
            .as_ref()
            .is_err_and(DemandError::is_whole_operation_failure)
        {
            return result;
        }
        let result = result.and_then(|value| {
            if resolved {
                return Ok(value);
            }
            value.and_then(|_| {
                let recipe = recipe.as_ref().map_err(Clone::clone)?;
                let annotated = [&recipe.getter, &recipe.setter, &recipe.auto]
                    .into_iter()
                    .flatten()
                    .find_map(|own| own.annotation.as_ref());
                let diagnostic = annotated.is_some()
                    || recipe.getter.is_some() && self.inputs.semantic_options().no_implicit_any;
                let report = if diagnostic {
                    self.diagnostic_symbol_display_completion(reference, consumer)?
                        .map(|_| ())
                } else {
                    NativeCompletion::clean(())
                };
                report.and_then(|()| {
                    NativeCompletion::recovered(
                        self.model.special(SpecialName::Any),
                        self.accessor_circular(
                            consumer,
                            annotated.map(|annotation| &annotation.location),
                        ),
                    )
                })
            })
        });
        let value = result?;
        if let Some(TypeState::Installed(installed)) = self.value_symbol_state(symbol, false)? {
            return Ok(value.map(|_| *installed.value()));
        }
        self.set_value_symbol_state(symbol, false, TypeState::Installed(value.clone()))?;
        Ok(value)
    }

    /// `getWriteTypeOfSymbol`: ordinary Property removes Missing, Accessor
    /// asks a separate WriteType property, other admitted symbols use raw read.
    pub(super) fn write_type_completion(
        &mut self,
        symbol: BoundSymbol,
        family: &BoundValueType,
    ) -> NativeResult<TypeId> {
        let property = if matches!(
            family,
            BoundValueType::Member(_) | BoundValueType::Prototype { .. }
        ) {
            Some(self.register_property_symbol(symbol)?)
        } else {
            None
        };
        let result = self.write_type_worker(symbol, family);
        if let Some(property) = property {
            let question = super::super::model::PropertyValueQuestion::Write(property);
            result
                .map(|value| value.observe(|failure| failure.at_property(question)))
                .map_err(|failure| failure.at_property(question))
        } else {
            result
        }
    }

    fn write_type_worker(
        &mut self,
        symbol: BoundSymbol,
        family: &BoundValueType,
    ) -> NativeResult<TypeId> {
        match self.native_members.writes.get(&symbol) {
            Some(TypeState::Installed(value)) => return Ok(value.clone()),
            Some(TypeState::Failed(error)) => return Err(error.clone()),
            Some(TypeState::Resolving) | None => {}
        }
        let result = match family {
            BoundValueType::Member(member) if member.property => self
                .value_type_completion(symbol, family)?
                .and_then_value(|read| {
                    self.model
                        .remove_missing_type(read, member.optional)
                        .map_err(|failure| {
                            failure.at(
                                self.inputs,
                                &super::super::location(
                                    self.inputs,
                                    member.value_declaration.file,
                                    member.value_declaration.anchor,
                                ),
                            )
                        })
                }),
            BoundValueType::Member(member) if member.kind == BoundMemberKind::Accessor => {
                let consumer = super::super::location(
                    self.inputs,
                    member.value_declaration.file,
                    member.value_declaration.anchor,
                );
                self.accessor_write_value(ValueSymbol::Bound(symbol), &consumer)
            }
            _ => self.value_type_completion(symbol, family),
        };
        if !self
            .native_members
            .writes
            .get(&symbol)
            .is_some_and(TypeState::has_type)
        {
            self.native_members.writes.insert(
                symbol,
                match &result {
                    Ok(value) => TypeState::Installed(value.clone()),
                    Err(error) => TypeState::Failed(error.clone()),
                },
            );
        }
        result
    }

    /// `getWriteTypeOfAccessors` on a checker-created symbol owns its actual
    /// WriteType cache and resolution identity; copied declarations never
    /// authorize reading the source symbol's write slot.
    pub(super) fn created_accessor_write_type_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let value = ValueSymbol::Created(symbol);
        let result = match self.value_symbol_state(value, true)? {
            Some(TypeState::Installed(value)) => Ok(value),
            Some(TypeState::Failed(error)) => Err(error),
            Some(TypeState::Resolving) | None => self.accessor_write_value(value, consumer),
        };
        if !self
            .value_symbol_state(value, true)?
            .is_some_and(|state| state.has_type())
        {
            self.set_value_symbol_state(
                value,
                true,
                match &result {
                    Ok(value) => TypeState::Installed(value.clone()),
                    Err(error) => TypeState::Failed(error.clone()),
                },
            )?;
        }
        let question = super::super::model::PropertyValueQuestion::Write(symbol);
        result
            .map(|value| value.observe(|failure| failure.at_property(question)))
            .map_err(|failure| failure.at_property(question))
    }

    fn accessor_write_value(
        &mut self,
        symbol: ValueSymbol,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if !self.push_value_resolution(symbol, resolution::Property::WriteType) {
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                self.accessor_circular(consumer, None),
            );
        }
        let recipe = self
            .set_value_symbol_state(symbol, true, TypeState::Resolving)
            .and_then(|()| self.value_symbol_declarations(symbol, consumer))
            .and_then(|declarations| self.source.accessor_recipe(&declarations));
        let annotation = match &recipe {
            Ok(recipe) => recipe
                .setter
                .as_ref()
                .or(recipe.write_auto.as_ref())
                .and_then(|setter| setter.annotation.as_ref()),
            Err(_) => None,
        };
        let result = match &recipe {
            Ok(_) => match annotation {
                Some(annotation) => self
                    .check_type_completion(annotation)
                    .map(|value| value.map(Some)),
                None => Ok(NativeCompletion::clean(None)),
            },
            Err(error) => Err(error.clone()),
        };
        let resolved = self
            .resolution
            .pop()
            .ok_or_else(|| super::super::internal("missing accessor WriteType resolution frame"))?;
        if let Err(error) = &result
            && error.is_whole_operation_failure()
        {
            return Err(error.clone());
        }
        let result = result.and_then(|value| {
            if resolved {
                return Ok(value);
            }
            value.and_then(|_| {
                let reference = match symbol {
                    ValueSymbol::Bound(bound) => super::ReferenceSymbol::from_origin(
                        crate::project::binding::ReadOrigin::Direct(bound),
                    ),
                    ValueSymbol::Created(created) => super::ReferenceSymbol::from_property(created),
                };
                let report = if annotation.is_some() {
                    self.diagnostic_symbol_display_completion(reference, consumer)?
                        .map(|_| ())
                } else {
                    NativeCompletion::clean(())
                };
                report.and_then(|()| {
                    NativeCompletion::recovered(
                        Some(self.model.special(SpecialName::Any)),
                        self.accessor_circular(
                            consumer,
                            annotation.map(|annotation| &annotation.location),
                        ),
                    )
                })
            })
        });
        let value = result?;
        if let Some(TypeState::Installed(installed)) = self.value_symbol_state(symbol, true)? {
            return Ok(value.map(|_| *installed.value()));
        }
        // Native pops WriteType before fallback to the raw accessor read.
        let value = value.and_then(|write| match write {
            Some(ty) => Ok(NativeCompletion::clean(ty)),
            None => match symbol {
                ValueSymbol::Bound(bound) => {
                    let family = self
                        .source
                        .bindings()
                        .value_type(bound)
                        .map_err(|error| self.generic_binding_failure(error, consumer))?;
                    self.value_type_completion(bound, &family)
                }
                ValueSymbol::Created(created) => {
                    self.created_property_value_completion(created, consumer)
                }
            },
        })?;
        self.set_value_symbol_state(symbol, true, TypeState::Installed(value.clone()))?;
        Ok(value)
    }

    /// `addPropertyToElementList` has one question per actual property symbol.
    /// Instantiation metadata lives on that symbol, never on a second display
    /// mapper chain which callers would have to keep synchronized.
    pub(super) fn register_property_serialization(
        &mut self,
        property_symbol: PropertySymbolId,
        consumer: &Location,
    ) -> PropertySerializationId {
        if let Some(&id) = self.native_members.serialization_ids.get(&property_symbol) {
            return id;
        }
        let id = PropertySerializationId::new(self.native_members.serialization.len());
        self.native_members.serialization.push(Serialization {
            property_symbol,
            consumer: consumer.clone(),
        });
        self.native_members
            .serialization_ids
            .insert(property_symbol, id);
        id
    }

    fn serialization_question(
        &mut self,
        id: PropertySerializationId,
    ) -> Result<SerializationQuestion, DemandError> {
        let Serialization {
            property_symbol,
            consumer,
        } = self
            .native_members
            .serialization
            .get(id.index())
            .cloned()
            .ok_or_else(|| {
                super::super::internal("property serialization lacks its native association")
            })?;
        Ok(SerializationQuestion {
            property_symbol,
            readonly: self.property_is_readonly(property_symbol, &consumer)?,
            mapper: self.property_mapper(property_symbol)?,
            consumer,
        })
    }

    /// Retained consumer for the native property serialization question.
    /// Reading the frame must not acquire readonly or value facts for a preview.
    pub(super) fn property_serialization_consumer(
        &self,
        id: PropertySerializationId,
    ) -> Result<Location, DemandError> {
        self.native_members
            .serialization
            .get(id.index())
            .map(|question| question.consumer.clone())
            .ok_or_else(|| {
                super::super::internal("property serialization lacks its native association").into()
            })
    }

    /// Continuation of `addPropertyToElementList` after its single non-missing
    /// read and name acquisition. Both diagnostic and admitted Q4 preparation
    /// carry that actual type here; this operation never re-reads the property.
    pub(super) fn serialize_property_after_read_completion(
        &mut self,
        id: PropertySerializationId,
        read: TypeId,
    ) -> NativeResult<PropertyDisplay> {
        let question = self.serialization_question(id)?;
        let flags = self.property_symbol_flags(question.property_symbol)?;
        if flags.get_accessor || flags.set_accessor {
            return self
                .property_write_type_completion(question.property_symbol, &question.consumer)?
                .and_then_value(|write| {
                    // Native consumes both types before its error-type gate. A
                    // recovered accessor is displayed as a plain property.
                    if self.model.is_error_type(read) || self.model.is_error_type(write) {
                        return Ok(PropertyDisplay::Plain {
                            value_type: TypeDemand::Known(read),
                        });
                    }
                    let declarations = self.property_accessor_syntax_declarations(
                        question.property_symbol,
                        &question.consumer,
                    )?;
                    let mut property_declaration = None;
                    for &declaration in &declarations {
                        if self.source.declaration_kind(declaration)?
                            == SyntaxKind::PropertyDeclaration
                        {
                            property_declaration = Some(declaration);
                            break;
                        }
                    }
                    let class = self
                        .property_parent(question.property_symbol)?
                        .map(|parent| self.symbol_parent_is_class(parent))
                        .transpose()?
                        .unwrap_or(false);
                    if read != write || class && property_declaration.is_none() {
                        let getter = self.serialization_signature(
                            &question,
                            &declarations,
                            SyntaxKind::GetAccessor,
                        )?;
                        let setter = self.serialization_signature(
                            &question,
                            &declarations,
                            SyntaxKind::SetAccessor,
                        )?;
                        return Ok(PropertyDisplay::Accessors(Box::new(AccessorDisplay {
                            getter,
                            setter,
                        })));
                    }
                    if class
                        && let Some(declaration) = property_declaration
                        && self
                            .source
                            .declaration_has_modifier(declaration, SyntaxKind::AccessorKeyword)?
                    {
                        return self.serialize_auto_accessor(read, write, &question.consumer);
                    }
                    Ok(PropertyDisplay::Plain {
                        value_type: TypeDemand::Known(read),
                    })
                });
        }
        if (flags.method || flags.types.function)
            && self
                .properties_of_object_type(read, &question.consumer)?
                .is_empty()
            && !question.readonly
        {
            let callable = self.model.filter_undefined(read);
            let signatures = self.call_signatures(callable, &question.consumer)?;
            if !signatures.is_empty() || !flags.optional {
                return Ok(NativeCompletion::clean(PropertyDisplay::Methods {
                    signatures: signatures.into(),
                }));
            }
        }
        Ok(NativeCompletion::clean(PropertyDisplay::Plain {
            value_type: TypeDemand::Known(read),
        }))
    }

    /// `addPropertyToElementList` gives an auto-accessor two fresh synthetic
    /// signatures, with no declaration/mapper or optional parameter semantics.
    fn serialize_auto_accessor(
        &mut self,
        read: TypeId,
        write: TypeId,
        consumer: &Location,
    ) -> Result<PropertyDisplay, DemandError> {
        use super::super::model::{SignatureData, SignatureFlags};
        let getter = self.model.signature(SignatureData {
            type_parameter_types: Vec::new().into(),
            type_parameters: Vec::new(),
            this_parameter: None,
            parameters: Vec::new().into(),
            return_type: TypeDemand::Known(read),
            predicate: None,
            origins: Vec::new().into(),
            minimum_arguments: 0,
            flags: SignatureFlags::declared(false, false, false, false),
        })?;
        let parameter = self.synthetic_parameter("arg".into(), write, false, false, consumer);
        let void = self.model.special(SpecialName::Void);
        let setter = self.model.signature(SignatureData {
            type_parameter_types: Vec::new().into(),
            type_parameters: Vec::new(),
            this_parameter: None,
            parameters: vec![parameter].into(),
            return_type: TypeDemand::Known(void),
            predicate: None,
            origins: Vec::new().into(),
            minimum_arguments: 0,
            flags: SignatureFlags::declared(false, false, false, false),
        })?;
        Ok(PropertyDisplay::Accessors(Box::new(AccessorDisplay {
            getter: Some(getter),
            setter: Some(setter),
        })))
    }

    fn serialization_signature(
        &mut self,
        question: &SerializationQuestion,
        declarations: &[crate::project::semantic::source::DeclarationIdentity],
        kind: SyntaxKind,
    ) -> Result<Option<crate::project::semantic::model::Signature>, DemandError> {
        for &declaration in declarations {
            if self.source.declaration_kind(declaration)? == kind {
                let recipe = self.source.signature_from_declaration(declaration)?;
                let signature = self.signature(&recipe)?;
                return match question.mapper {
                    Some(mapper) => self
                        .instantiate_signature(&signature, mapper, &question.consumer)
                        .map(Some),
                    None => Ok(Some(signature)),
                };
            }
        }
        Ok(None)
    }
}
