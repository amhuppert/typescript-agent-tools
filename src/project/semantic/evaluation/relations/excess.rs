//! Fresh object-literal excess checking and its diagnostic acquisitions.

use crate::project::semantic::model::{NativeResult, NativeSequence};

use super::{
    Argument, Evaluator, IntersectionState, Message, RecursionFlags, RelationInvocation,
    RelationKind, Ternary,
};
use crate::project::semantic::evaluation::native_display::DiagnosticTypeFormat;
use crate::project::semantic::evaluation::reference_symbols::ReferenceSymbol;
use crate::project::semantic::model::{
    DemandError, ObjectFlags, PropertySymbolId, SpecialName, TypeFlags, TypeId,
};
use crate::references::Location;

impl Evaluator<'_, '_> {
    /// `hasExcessProperties`: target eligibility precedes all source members;
    /// a union's discriminant reduction is shared with ordinary matching.
    pub(super) fn has_excess_properties_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if !sequence.read(self.is_excess_property_check_target_completion(target))?
                || !self.inputs.semantic_options().no_implicit_any
                    && self
                        .model
                        .object_flags(target)?
                        .intersects(ObjectFlags::JS_LITERAL)
            {
                return Ok(false);
            }
            let jsx = self
                .model
                .object_flags(source)?
                .intersects(ObjectFlags::JSX_ATTRIBUTES);
            if matches!(
                self.relations.session(session).kind,
                RelationKind::Assignable | RelationKind::Comparable
            ) {
                let object = self.global_object_identity(consumer)?;
                if sequence.read(self.type_subset_of_completion(object, target, consumer))?
                    || !jsx && self.is_empty_object_type(target, consumer)?
                {
                    return Ok(false);
                }
            }
            let mut reduced = target;
            let mut check_types = None;
            if self.model.has_type_flags(target, TypeFlags::UNION) {
                reduced = match self.matching_discriminant_type(
                    source,
                    target,
                    |checker, source, target| {
                        sequence
                            .read(checker.related_in_session_completion(
                                source,
                                target,
                                RecursionFlags::BOTH,
                                IntersectionState::NONE,
                                session.reporting(false),
                                consumer,
                            ))
                            .map(|result| result != Ternary::False)
                    },
                    consumer,
                )? {
                    Some(ty) => ty,
                    None => self.filter_primitives_if_non_primitive(target),
                };
                check_types = Some(
                    self.model
                        .union_constituents(reduced)
                        .map_or_else(|| vec![reduced], |types| types.to_vec()),
                );
            }
            for property in self.properties_of_type(source, consumer)? {
                let symbol = self.property_symbol(&property, consumer)?;
                let Some(declaration) =
                    self.property_reference_value_declaration(symbol, consumer)?
                else {
                    continue;
                };
                let Some(container) = self.type_symbol_value_declaration(source, consumer)? else {
                    continue;
                };
                if !self.reference_should_check_excess_property(declaration, container)? {
                    continue;
                }
                let name = self.property_lookup_key(symbol, consumer)?;
                if jsx && name.is_hyphenated_jsx_name() {
                    continue;
                }
                if !sequence
                    .read(self.is_known_property_completion(reduced, &name, jsx, consumer))?
                {
                    if session.report {
                        let error_target = sequence.read(
                            self.filter_type_completion(reduced, |checker, ty| {
                                checker.is_excess_property_check_target_completion(ty)
                            }),
                        )?;
                        let error_node =
                            self.relations.session(session).error_node.ok_or_else(|| {
                                super::super::super::internal(
                                    "excess-property reporting has no native error node",
                                )
                            })?;
                        let object = if self.source.is_jsx_relation_error(error_node)? {
                            None
                        } else {
                            self.type_symbol_first_declaration(source, consumer)?
                                .and_then(|declaration| match declaration {
                                    super::super::ReferenceDeclaration::Captured(declaration) => {
                                        Some(declaration)
                                    }
                                    super::super::ReferenceDeclaration::Json { .. } => None,
                                })
                        };
                        let facts = self.source.excess_property_diagnostic(
                            error_node,
                            match declaration {
                                super::super::ReferenceDeclaration::Captured(declaration) => {
                                    Some(declaration)
                                }
                                super::super::ReferenceDeclaration::Json { .. } => None,
                            },
                            object,
                        )?;
                        self.relations.session_mut(session).error_node = Some(facts.node);
                        if facts.jsx {
                            let name = sequence
                                .read(self.raw_diagnostic_property_symbol_display_completion(
                                    symbol, consumer,
                                ))?
                                .into_bytes();
                            let text = std::str::from_utf8(&name).map_err(|_| {
                                self.relation_pending(
                                    consumer,
                                    "JSX suggestion requires an exact native symbol name",
                                )
                            })?;
                            let suggestion =
                                sequence.read(self.suggested_property_symbol_completion(
                                    text,
                                    error_target,
                                    true,
                                    consumer,
                                ))?;
                            let target_name = sequence
                                .read(self.raw_diagnostic_type_display_completion(
                                    error_target,
                                    None,
                                    DiagnosticTypeFormat::DEFAULT,
                                    consumer,
                                ))?
                                .into_bytes();
                            let mut arguments =
                                vec![Argument::Text(name), Argument::Text(target_name)];
                            if let Some(suggestion) = suggestion {
                                arguments.push(Argument::Text(
                                    sequence
                                        .read(
                                            self.raw_diagnostic_property_symbol_display_completion(
                                                suggestion, consumer,
                                            ),
                                        )?
                                        .into_bytes(),
                                ));
                            }
                            self.relations.session_mut(session).error.report(
                                if suggestion.is_some() {
                                    Message::SuggestedProperty
                                } else {
                                    Message::PropertyDoesNotExist
                                },
                                arguments,
                            );
                        } else {
                            let suggestion = if let Some(name) = facts.suggestion_name {
                                sequence
                                    .read(self.suggested_property_symbol_completion(
                                        &name,
                                        error_target,
                                        false,
                                        consumer,
                                    ))?
                                    .map(|symbol| {
                                        let key = self.property_lookup_key(symbol, consumer)?;
                                        super::super::super::model::LiteralValue::from_property_key(
                                            &key,
                                        )
                                        .fixed_bytes()
                                        .map(<[u8]>::to_vec)
                                        .map_err(|error| {
                                            self.literal_value_failure(error, consumer)
                                        })
                                    })
                                    .transpose()?
                            } else {
                                None
                            };
                            let name = sequence
                                .read(self.raw_diagnostic_property_symbol_display_completion(
                                    symbol, consumer,
                                ))?
                                .into_bytes();
                            let target_name = sequence
                                .read(self.raw_diagnostic_type_display_completion(
                                    error_target,
                                    None,
                                    DiagnosticTypeFormat::DEFAULT,
                                    consumer,
                                ))?
                                .into_bytes();
                            let mut arguments =
                                vec![Argument::Text(name), Argument::Text(target_name)];
                            if let Some(suggestion) = &suggestion {
                                arguments.push(Argument::Text(suggestion.clone()));
                            }
                            self.relations.session_mut(session).error.report(
                                if suggestion.is_some() {
                                    Message::SuggestedExcessProperty
                                } else {
                                    Message::ExcessProperty
                                },
                                arguments,
                            );
                        }
                    }
                    return Ok(true);
                }
                if let Some(types) = &check_types {
                    let source_type =
                        sequence.read(self.property_raw_type_completion(symbol, consumer))?;
                    let target_type = sequence
                        .read(self.property_type_in_types_completion(types, &name, consumer))?;
                    if sequence.read(self.related_in_session_completion(
                        source_type,
                        target_type,
                        RecursionFlags::BOTH,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))? == Ternary::False
                    {
                        if session.report {
                            sequence.read(self.report_property_relation_completion(
                                Message::PropertyTypes,
                                symbol,
                                &[],
                                session,
                                consumer,
                            ))?;
                        }
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `filterPrimitivesIfContainsNonPrimitive` preserves the original union
    /// when no NonPrimitive constituent exists or filtering produces Never.
    fn filter_primitives_if_non_primitive(&mut self, ty: TypeId) -> TypeId {
        if self.model.maybe_type_of_kind(ty, TypeFlags::NON_PRIMITIVE) {
            let filtered = self.model.filter_type(ty, |model, part| {
                !model.has_type_flags(part, TypeFlags::PRIMITIVE)
            });
            if !self.model.has_type_flags(filtered, TypeFlags::NEVER) {
                return filtered;
            }
        }
        ty
    }

    /// `getTypeOfPropertyInTypes` / `getTypeOfPropertyInType`: apparent
    /// properties precede applicable indexes, with native Undefined absence.
    fn property_type_in_types_completion(
        &mut self,
        types: &[TypeId],
        name: &crate::project::binding::NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let mut results = Vec::new();
            for &ty in types {
                let ty = self.apparent_type(ty, consumer)?;
                let property = if self
                    .model
                    .has_type_flags(ty, TypeFlags::UNION | TypeFlags::INTERSECTION)
                {
                    self.property_of_composite_type_by_key(ty, name, false, consumer)?
                } else {
                    self.property_of_object_type_by_key(ty, name, consumer)?
                };
                let value = if let Some(property) = property {
                    let symbol = self.property_symbol(&property, consumer)?;
                    sequence.read(self.property_raw_type_completion(symbol, consumer))?
                } else if let Some(index) =
                    self.applicable_index_info_for_key(ty, name, consumer)?
                {
                    sequence.read(self.demand_completion(index.value_type))?
                } else {
                    self.model.special(SpecialName::Undefined)
                };
                results.push(value);
            }
            self.union_type_ex(results, None, None, consumer)
        })();
        sequence.finish(result)
    }

    /// `getSuggestionForNonexistentProperty` and its JSX variant share native
    /// SymbolName/Value filtering and compareSymbols tie order. JSX's two
    /// conventional names precede the general spelling operation.
    fn suggested_property_symbol_completion(
        &mut self,
        name: &str,
        ty: TypeId,
        jsx: bool,
        consumer: &Location,
    ) -> NativeResult<Option<PropertySymbolId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<PropertySymbolId>, DemandError> {
            let mut symbols = Vec::new();
            for property in sequence.read(self.properties_of_type_completion(ty, consumer))? {
                symbols.push(self.property_symbol(&property, consumer)?);
            }
            if jsx
                && let Some(wanted) = match name {
                    "for" => Some("htmlFor"),
                    "class" => Some("className"),
                    _ => None,
                }
            {
                for &symbol in &symbols {
                    if self.reference_symbol_display_name_bytes(
                        ReferenceSymbol::from_property(symbol),
                        consumer,
                    )? == wanted.as_bytes()
                    {
                        return Ok(Some(symbol));
                    }
                }
            }
            sequence
                .read(self.spelling_suggestion_for_properties_completion(name, symbols, consumer))
        })();
        sequence.finish(result)
    }
}
