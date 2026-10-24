//! Contextual element and property lookup preserve partial (native nil) maps.

use super::*;
use crate::project::binding::NativePropertyKey;
use crate::project::semantic::model::TupleElementFlags;

impl Evaluator<'_, '_> {
    /// `getContextualTypeForObjectLiteralElement`: the object's context is
    /// acquired before its actual declaration symbol or computed name type.
    pub(in crate::project::semantic::evaluation) fn contextual_object_element_type_completion(
        &mut self,
        root: ExpressionRoot,
        flags: ContextFlags,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.contextual_object_element(root)?;
            if recipe.method_in_with {
                return Ok(None);
            }
            if let Some(annotation) = recipe.annotation {
                return sequence
                    .read(self.check_type_completion(&annotation))
                    .map(Some);
            }
            let Some(contextual) = sequence.read(self.apparent_contextual_type_completion(
                recipe.object,
                flags,
                consumer,
            ))?
            else {
                return Ok(None);
            };
            let declaration = self.source.type_node_declaration(root)?;
            if sequence.read(self.has_bindable_declaration_name_completion(declaration))? {
                let raw =
                    sequence.read(self.reference_symbol_of_declaration_completion(declaration))?;
                let symbol = self.register_reference_symbol(raw, consumer)?;
                let key = self.reference_symbol_key(raw, consumer)?;
                let name_type = self.property_name_type(symbol)?;
                return sequence.read(self.contextual_property_type_by_key_completion(
                    contextual, &key, name_type, consumer,
                ));
            }
            if recipe.dynamic
                && let Some(expression) = recipe.computed
            {
                let name_type = sequence
                    .read(self.check_expression_completion(&expression, CheckMode::NORMAL))?;
                if let Some(name) = self.property_key_from_type(name_type)?
                    && let Some(ty) =
                        sequence.read(self.contextual_property_type_by_key_completion(
                            contextual, &name, None, consumer,
                        ))?
                {
                    return Ok(Some(ty));
                }
            }
            let Some(name) = recipe.name else {
                return Ok(None);
            };
            let name_type =
                sequence.read(self.literal_type_of_property_name_completion(root.file, name))?;
            sequence.read(self.map_type_optional_completion(
                contextual,
                true,
                consumer,
                |checker, ty| {
                    let mut sequence = NativeSequence::new();
                    let result = (|| {
                        let indices = sequence.read(
                            checker.index_infos_of_structured_type_completion(ty, consumer),
                        )?;
                        let index =
                            sequence.read(checker.find_applicable_index_info_completion(
                                &indices, name_type, consumer,
                            ))?;
                        index
                            .map(|index| sequence.read(checker.demand_completion(index.value_type)))
                            .transpose()
                    })();
                    sequence.finish(result)
                },
            ))
        })();
        sequence.finish(result)
    }

    /// `getContextualTypeForElementExpression`: fixed prefix, trailing fixed
    /// elements, then the no-reduction slice. Non-tuples ask a concrete numeric
    /// property before their iterated element type.
    pub(in crate::project::semantic::evaluation) fn contextual_element_type_completion(
        &mut self,
        contextual: Option<TypeId>,
        index: usize,
        length: Option<usize>,
        first_spread: Option<usize>,
        last_spread: Option<usize>,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(contextual) = contextual else {
                return Ok(None);
            };
            sequence.read(self.map_type_optional_completion(
                contextual,
                true,
                consumer,
                |checker, ty| {
                    let mut sequence = NativeSequence::new();
                    let result = (|| {
                        if let Some(tuple) = checker.model.tuple_target(ty)? {
                            if first_spread.is_none_or(|first| index < first)
                                && index < tuple.fixed_length
                            {
                                let arguments =
                                    sequence.read(checker.get_type_arguments_completion(ty))?;
                                return checker
                                    .model
                                    .remove_missing_type(
                                        arguments[index],
                                        tuple.elements[index]
                                            .flags
                                            .intersects(TupleElementFlags::OPTIONAL),
                                    )
                                    .map(Some)
                                    .map_err(|failure| failure.at(checker.inputs, consumer));
                            }
                            let offset = if let Some(length) = length
                                && last_spread.is_none_or(|last| index > last)
                            {
                                length - index
                            } else {
                                0
                            };
                            let fixed_end = if offset > 0
                                && tuple.combined_flags.intersects(TupleElementFlags::VARIABLE)
                            {
                                tuple.end_element_count(TupleElementFlags::FIXED)
                            } else {
                                0
                            };
                            if offset > 0 && offset <= fixed_end {
                                let arguments =
                                    sequence.read(checker.get_type_arguments_completion(ty))?;
                                return Ok(Some(
                                    arguments[checker.type_reference_arity(ty)? - offset],
                                ));
                            }
                            let tuple_index = first_spread
                                .map_or(tuple.fixed_length, |first| tuple.fixed_length.min(first));
                            let end_skip = match (length, last_spread) {
                                (Some(length), Some(last)) => fixed_end.min(length - last),
                                _ => fixed_end,
                            };
                            return sequence.read(
                                checker.get_element_type_of_slice_of_tuple_type_completion(
                                    ty,
                                    tuple_index,
                                    end_skip,
                                    false,
                                    true,
                                    consumer,
                                ),
                            );
                        }
                        if first_spread.is_none_or(|first| index < first)
                            && let Some(property) =
                                sequence.read(checker.contextual_property_type_completion(
                                    ty,
                                    &index.to_string(),
                                    None,
                                    consumer,
                                ))?
                        {
                            return Ok(Some(property));
                        }
                        sequence.read(checker.iterated_type_or_element_type_completion(
                            ty,
                            super::super::iteration::IterationUse::Element,
                            None,
                            false,
                            consumer,
                        ))
                    })();
                    sequence.finish(result)
                },
            ))
        })();
        sequence.finish(result)
    }

    /// `getTypeOfPropertyOfContextualTypeEx`: concrete intersection properties
    /// suppress all index candidates. Any contributes Unknown so it cannot
    /// erase contextual information from another intersection constituent.
    pub(in crate::project::semantic::evaluation) fn contextual_property_type_completion(
        &mut self,
        ty: TypeId,
        name: &str,
        name_type: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.contextual_property_type_by_key_completion(
            ty,
            &NativePropertyKey::fixed(name.as_bytes()),
            name_type,
            consumer,
        )
    }

    pub(in crate::project::semantic::evaluation) fn contextual_property_type_by_key_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        name_type: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.map_type_optional_completion(ty, true, consumer, |checker, ty| {
            let mut sequence = NativeSequence::new();
            let result = (|| {
                if let Some(constituents) = checker.model.intersection_constituents(ty) {
                    let mut types = Vec::new();
                    let mut index_candidates = Vec::new();
                    let mut ignore_indexes = false;
                    for &constituent in constituents.iter() {
                        if !checker.model.has_type_flags(constituent, TypeFlags::OBJECT) {
                            continue;
                        }
                        if sequence.read(
                            checker.is_generic_mapped_type_completion(constituent, consumer),
                        )? && sequence
                            .read(checker.mapped_name_kind_completion(constituent, consumer))?
                            != super::super::mapped::NameKind::Remapping
                        {
                            if let Some(value) =
                                sequence.read(checker.mapped_contextual_property_completion(
                                    constituent,
                                    name,
                                    name_type,
                                    consumer,
                                ))?
                            {
                                types.push(checker.contextual_property_constituent(value));
                            }
                            continue;
                        }
                        let Some(property) =
                            sequence.read(checker.concrete_contextual_property_completion(
                                constituent,
                                name,
                                consumer,
                            ))?
                        else {
                            if !ignore_indexes {
                                index_candidates.push(constituent);
                            }
                            continue;
                        };
                        ignore_indexes = true;
                        index_candidates.clear();
                        types.push(checker.contextual_property_constituent(property));
                    }
                    for candidate in index_candidates {
                        if let Some(index) =
                            sequence.read(checker.contextual_index_type_completion(
                                candidate, name, name_type, consumer,
                            ))?
                        {
                            types.push(checker.contextual_property_constituent(index));
                        }
                    }
                    return match types.as_slice() {
                        [] => Ok(None),
                        [ty] => Ok(Some(*ty)),
                        _ => sequence
                            .read(checker.intersection_type_completion(types, consumer))
                            .map(Some),
                    };
                }
                if !checker.model.has_type_flags(ty, TypeFlags::OBJECT) {
                    return Ok(None);
                }
                if sequence.read(checker.is_generic_mapped_type_completion(ty, consumer))?
                    && sequence.read(checker.mapped_name_kind_completion(ty, consumer))?
                        != super::super::mapped::NameKind::Remapping
                {
                    return sequence.read(
                        checker
                            .mapped_contextual_property_completion(ty, name, name_type, consumer),
                    );
                }
                if let Some(property) = sequence
                    .read(checker.concrete_contextual_property_completion(ty, name, consumer))?
                {
                    return Ok(Some(property));
                }
                sequence
                    .read(checker.contextual_index_type_completion(ty, name, name_type, consumer))
            })();
            sequence.finish(result)
        })
    }

    fn contextual_property_constituent(&mut self, ty: TypeId) -> TypeId {
        if self.model.has_type_flags(ty, TypeFlags::ANY) {
            self.model.special(SpecialName::Unknown)
        } else {
            ty
        }
    }

    /// `getTypeOfConcretePropertyOfContextualType` rejects only an actual
    /// circular mapped property before reading and removing optional Missing.
    fn concrete_contextual_property_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(property) =
                sequence.read(self.property_of_type_by_key_completion(ty, name, consumer))?
            else {
                return Ok(None);
            };
            let symbol = self.property_symbol(&property, consumer)?;
            if self.is_circular_mapped_property(symbol)? {
                return Ok(None);
            }
            sequence
                .read(self.non_missing_property_type_completion(symbol, consumer))
                .map(Some)
        })();
        sequence.finish(result)
    }

    /// `getTypeFromIndexInfosOfContextualType` gives tuple rest slices their
    /// native numeric-name priority and skips apparent-type reduction.
    fn contextual_index_type_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        name_type: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(tuple) = self.model.tuple_target(ty)?
                && name
                    .fixed_bytes()
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .and_then(crate::numbers::numeric_literal_name_value)
                    .is_some_and(|value| value >= 0.0)
                && let Some(rest) =
                    sequence.read(self.get_element_type_of_slice_of_tuple_type_completion(
                        ty,
                        tuple.fixed_length,
                        0,
                        false,
                        true,
                        consumer,
                    ))?
            {
                return Ok(Some(rest));
            }
            let key = match name_type {
                Some(ty) => ty,
                None => self.model.string_literal_for_key(name),
            };
            let indices =
                sequence.read(self.index_infos_of_structured_type_completion(ty, consumer))?;
            match sequence
                .read(self.find_applicable_index_info_completion(&indices, key, consumer))?
            {
                Some(index) => sequence
                    .read(self.demand_completion(index.value_type))
                    .map(Some),
                None => Ok(None),
            }
        })();
        sequence.finish(result)
    }
}
