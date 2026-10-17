//! Mapped inference shares candidate contexts. The two reverse caches preserve
//! native nil semantics independently; failure is never a nil cache value.
use super::*;
use crate::project::semantic::model::{IndexFlags, MembersView, Metadata, TypeSymbol};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic::evaluation) struct ReverseKey {
    pub source: TypeId,
    pub target: TypeId,
    pub constraint: TypeId,
}
#[derive(Default)]
pub(super) struct ReverseState {
    homomorphic: HashMap<ReverseKey, NativeResult<Option<TypeId>>>,
    values: HashMap<ReverseKey, NativeResult<Option<TypeId>>>,
    types: HashMap<TypeId, ReverseKey>,
    sources: Vec<TypeId>,
    targets: Vec<TypeId>,
    expanding: u8,
}

impl Evaluator<'_, '_> {
    pub(in crate::project::semantic::evaluation) fn reverse_mapped_source(
        &self,
        ty: TypeId,
    ) -> Result<TypeId, DemandError> {
        self.inference
            .reverse
            .types
            .get(&ty)
            .map(|key| key.source)
            .ok_or_else(|| {
                super::super::super::internal("reverse mapped type lacks factory links").into()
            })
    }

    /// `instantiateReverseMappedType` preserves the original when either
    /// instantiated inner kind changes or homomorphic reverse inference is nil.
    pub(in crate::project::semantic::evaluation) fn instantiate_reverse_mapped_type_completion(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let key = *self.inference.reverse.types.get(&ty).ok_or_else(|| {
                super::super::super::internal("reverse mapped type lacks factory links")
            })?;
            let target =
                sequence.read(self.instantiate_type_completion(key.target, mapper, consumer))?;
            if !self
                .model
                .object_flags(target)?
                .intersects(ObjectFlags::MAPPED)
            {
                return Ok(ty);
            }
            let constraint = sequence.read(self.instantiate_type_completion(
                key.constraint,
                mapper,
                consumer,
            ))?;
            if !self.model.has_type_flags(constraint, TypeFlags::INDEX) {
                return Ok(ty);
            }
            let source =
                sequence.read(self.instantiate_type_completion(key.source, mapper, consumer))?;
            Ok(sequence
                .read(self.infer_type_for_homomorphic_mapped_type_completion(
                    source, target, constraint, consumer,
                ))?
                .unwrap_or(ty))
        })();
        sequence.finish(result)
    }

    /// `inferFromGenericMappedTypes` preserves constraint, template and optional
    /// paired name acquisition, in that order, in the active inference pass.
    pub(super) fn infer_from_generic_mapped_types_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let source_constraint =
                sequence.read(self.mapped_constraint_completion(source, consumer))?;
            let target_constraint =
                sequence.read(self.mapped_constraint_completion(target, consumer))?;
            sequence.read(self.infer_from_types_completion(
                source_constraint,
                target_constraint,
                pass,
                consumer,
            ))?;
            let source_template =
                sequence.read(self.mapped_template_completion(source, consumer))?;
            let target_template =
                sequence.read(self.mapped_template_completion(target, consumer))?;
            sequence.read(self.infer_from_types_completion(
                source_template,
                target_template,
                pass,
                consumer,
            ))?;
            let source_name = sequence.read(self.mapped_name_completion(source, consumer))?;
            let target_name = sequence.read(self.mapped_name_completion(target, consumer))?;
            if let (Some(source), Some(target)) = (source_name, target_name) {
                sequence.read(self.infer_from_types_completion(source, target, pass, consumer))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `inferToMappedType` makes every union/intersection inference, even after
    /// one succeeds. The fresh-checker query contract has no language-service
    /// edited-node block set (`isFromInferenceBlockedSource` is therefore false).
    pub(super) fn infer_to_mapped_type_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        constraint: TypeId,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if let Some(types) = self
                .model
                .union_constituents(constraint)
                .or_else(|| self.model.intersection_constituents(constraint))
            {
                let mut result = false;
                for &ty in types.iter() {
                    result = sequence.read(
                        self.infer_to_mapped_type_completion(source, target, ty, pass, consumer),
                    )? || result;
                }
                return Ok(result);
            }
            if let TypeView::Index(index) = self.model.get(constraint)? {
                let inference = self.inference.contexts[pass.context.index()]
                    .infos
                    .iter()
                    .find(|info| {
                        info.parameter == index.target
                            && self
                                .model
                                .has_type_flags(index.target, TypeFlags::TYPE_VARIABLE)
                    })
                    .filter(|info| !info.fixed)
                    .map(|info| info.parameter);
                if let Some(parameter) = inference {
                    if let Some(inferred) =
                        sequence.read(self.infer_type_for_homomorphic_mapped_type_completion(
                            source, target, constraint, consumer,
                        ))?
                    {
                        let priority = if self
                            .model
                            .object_flags(source)?
                            .intersects(ObjectFlags::NON_INFERRABLE_TYPE)
                        {
                            1 << 4
                        } else {
                            1 << 3
                        };
                        sequence.read(self.infer_with_priority_completion(
                            inferred,
                            parameter,
                            Priority(priority),
                            pass,
                            consumer,
                        ))?;
                    }
                }
                return Ok(true);
            }
            if self
                .model
                .has_type_flags(constraint, TypeFlags::TYPE_PARAMETER)
            {
                let flags = if self.has_binding_pattern_type(source) {
                    IndexFlags::NO_INDEX_SIGNATURES
                } else {
                    IndexFlags::NONE
                };
                let index = self.index_type_ex(source, flags, consumer)?;
                sequence.read(self.infer_with_priority_completion(
                    index,
                    constraint,
                    Priority(1 << 5),
                    pass,
                    consumer,
                ))?;
                if let Some(extended) = self.constraint_of_type(constraint, consumer)?
                    && sequence.read(self.infer_to_mapped_type_completion(
                        source, target, extended, pass, consumer,
                    ))?
                {
                    return Ok(true);
                }
                let mut values = Vec::new();
                for property in self.properties_of_type(source, consumer)? {
                    let symbol = self.property_symbol(&property, consumer)?;
                    values
                        .push(sequence.read(self.property_raw_type_completion(symbol, consumer))?);
                }
                for index in self.index_infos_of_type(source, consumer)? {
                    // The enumNumberIndexInfo sentinel has no admitted producer;
                    // ordinary indexes retain their actual raw value demand.
                    values.push(sequence.read(self.demand_completion(index.value_type))?);
                }
                let source =
                    sequence.read(self.union_type_ex_completion(values, None, None, consumer))?;
                let template = sequence.read(self.mapped_template_completion(target, consumer))?;
                sequence
                    .read(self.infer_from_types_completion(source, template, pass, consumer))?;
                return Ok(true);
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `inferTypeForHomomorphicMappedType`: an installed nil is retried, unlike
    /// the separate inferReverseMappedType value cache below.
    fn infer_type_for_homomorphic_mapped_type_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        constraint: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let key = ReverseKey {
            source,
            target,
            constraint,
        };
        if let Some(cached) = self.inference.reverse.homomorphic.get(&key) {
            if !matches!(cached, Ok(value) if value.value().is_none()) {
                return cached.clone();
            }
        }
        let result = self.create_reverse_mapped_type_completion(key, consumer);
        self.inference
            .reverse
            .homomorphic
            .insert(key, result.clone());
        result
    }

    /// `createReverseMappedType` asks eligibility before array/tuple shape and
    /// preserves the target's include-optional reversal on tuple flags.
    fn create_reverse_mapped_type_completion(
        &mut self,
        key: ReverseKey,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let string = self.model.primitive(crate::types::PrimitiveName::String);
            if self
                .index_info_of_type(key.source, string, consumer)?
                .is_none()
                && (self.properties_of_type(key.source, consumer)?.is_empty()
                    || !sequence
                        .read(self.is_partially_inferable_type_completion(key.source, consumer))?)
            {
                return Ok(None);
            }
            if self.is_array_type(key.source)? {
                let arguments = sequence.read(self.get_type_arguments_completion(key.source))?;
                let Some(element) = sequence.read(self.infer_reverse_mapped_type_completion(
                    arguments[0],
                    key.target,
                    key.constraint,
                    consumer,
                ))?
                else {
                    return Ok(None);
                };
                return if self.is_readonly_array_type(key.source)? {
                    self.readonly_array_type(element, consumer).map(Some)
                } else {
                    self.array_type(element, consumer).map(Some)
                };
            }
            if let Some(tuple) = self.model.tuple_target(key.source)? {
                let source_elements =
                    sequence.read(self.get_element_types_completion(key.source))?;
                let mut elements = Vec::new();
                let mut complete = true;
                for &ty in source_elements.iter() {
                    match sequence.read(self.infer_reverse_mapped_type_completion(
                        ty,
                        key.target,
                        key.constraint,
                        consumer,
                    ))? {
                        Some(ty) => elements.push(ty),
                        None => complete = false,
                    }
                }
                if !complete {
                    return Ok(None);
                }
                let mut infos = tuple.elements.to_vec();
                if self.mapped_recipe(key.target)?.optional > 0 {
                    for info in &mut infos {
                        if info.flags.intersects(TupleElementFlags::OPTIONAL) {
                            info.flags = TupleElementFlags::REQUIRED;
                        }
                    }
                }
                return sequence
                    .read(self.create_tuple_type_ex_completion(
                        elements,
                        infos,
                        tuple.readonly,
                        consumer,
                    ))
                    .map(Some);
            }
            let ty = self.model.anonymous_type_ex(
                TypeSymbol::Absent,
                Metadata::default(),
                ObjectFlags::ANONYMOUS | ObjectFlags::REVERSE_MAPPED,
                None,
            );
            self.register_reverse_mapped_object(ty, consumer);
            self.inference.reverse.types.insert(ty, key);
            Ok(Some(ty))
        })();
        sequence.finish(result)
    }

    /// `isPartiallyInferableType` short-circuits before every property/element
    /// read; a contextual function can coexist with an inferable sibling.
    pub(in crate::project::semantic::evaluation) fn is_partially_inferable_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let flags = self.model.object_flags(ty)?;
            if !flags.intersects(ObjectFlags::NON_INFERRABLE_TYPE) {
                return Ok(true);
            }
            if flags.intersects(ObjectFlags::OBJECT_LITERAL) {
                for property in self.properties_of_type(ty, consumer)? {
                    let symbol = self.property_symbol(&property, consumer)?;
                    let value =
                        sequence.read(self.property_raw_type_completion(symbol, consumer))?;
                    if sequence
                        .read(self.is_partially_inferable_type_completion(value, consumer))?
                    {
                        return Ok(true);
                    }
                }
            }
            if self.model.is_tuple_type(ty) {
                for &element in sequence.read(self.get_element_types_completion(ty))?.iter() {
                    if sequence
                        .read(self.is_partially_inferable_type_completion(element, consumer))?
                    {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `inferReverseMappedType`: a cached nil returns Unknown, while the first
    /// nil stays nil. Restore both active stacks and expanding flags on failure.
    pub(in crate::project::semantic::evaluation) fn infer_reverse_mapped_type(
        &mut self,
        source: TypeId,
        target: TypeId,
        constraint: TypeId,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.infer_reverse_mapped_type_completion(source, target, constraint, consumer)?
            .admit()
    }

    pub(in crate::project::semantic::evaluation) fn infer_reverse_mapped_type_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        constraint: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let key = ReverseKey {
            source,
            target,
            constraint,
        };
        if let Some(result) = self.inference.reverse.values.get(&key) {
            return result.clone().map(|ty| {
                ty.map(|ty| Some(ty.unwrap_or_else(|| self.model.special(SpecialName::Unknown))))
            });
        }
        let old_expanding = self.inference.reverse.expanding;
        self.inference.reverse.sources.push(source);
        self.inference.reverse.targets.push(target);
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let sources = self.inference.reverse.sources.clone();
            if sequence
                .read(self.deeply_nested_relation_type_completion(source, &sources, 2, consumer))?
            {
                self.inference.reverse.expanding |= 1;
            }
            let targets = self.inference.reverse.targets.clone();
            if sequence
                .read(self.deeply_nested_relation_type_completion(target, &targets, 2, consumer))?
            {
                self.inference.reverse.expanding |= 2;
            }
            if self.inference.reverse.expanding == 3 {
                return Ok(None);
            }
            let TypeView::Index(index) = self.model.get(constraint)? else {
                return Err(super::super::super::internal(
                    "reverse inference constraint is not an Index",
                )
                .into());
            };
            let parameter = sequence.read(self.mapped_parameter_completion(target, consumer))?;
            let variable = sequence.read(self.indexed_access_type_completion(
                index.target,
                parameter,
                consumer,
            ))?;
            let template = sequence.read(self.mapped_template_completion(target, consumer))?;
            let context = self.new_inference_context(vec![variable], None, None);
            sequence.read(self.infer_types_completion(
                context,
                source,
                template,
                Priority::NONE,
                consumer,
            ))?;
            let ty = sequence
                .read(self.type_from_inference_completion(context, 0, consumer))?
                .unwrap_or_else(|| self.model.special(SpecialName::Unknown));
            sequence
                .read(self.widened_type_completion(ty, consumer))
                .map(Some)
        })();
        self.inference.reverse.sources.pop();
        self.inference.reverse.targets.pop();
        self.inference.reverse.expanding = old_expanding;
        let result = sequence.finish(result);
        self.inference.reverse.values.insert(key, result.clone());
        result
    }

    /// `resolveReverseMappedTypeMembers` keeps source indexes before property
    /// filtering; symbols allocate before their source read and replacement map.
    pub(in crate::project::semantic::evaluation) fn reverse_mapped_members_completion(
        &mut self,
        ty: TypeId,
    ) -> NativeResult<MembersView> {
        let consumer = self.anonymous_consumer(ty)?;
        let mut table = None;
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<MembersView, DemandError> {
            let key = *self.inference.reverse.types.get(&ty).ok_or_else(|| {
                super::super::super::internal("reverse mapped type lacks factory links")
            })?;
            let consumer = self.anonymous_consumer(ty)?;
            let string = self.model.primitive(crate::types::PrimitiveName::String);
            let index =
                sequence.read(self.index_info_of_type_completion(key.source, string, &consumer))?;
            let modifiers = self.mapped_recipe(key.target)?;
            let readonly = modifiers.readonly <= 0;
            let optional = modifiers.optional <= 0;
            let mut indices = Vec::new();
            if let Some(index) = index {
                let value = sequence.read(self.demand_completion(index.value_type))?;
                let value = sequence
                    .read(self.infer_reverse_mapped_type_completion(
                        value,
                        key.target,
                        key.constraint,
                        &consumer,
                    ))?
                    .unwrap_or_else(|| self.model.special(SpecialName::Unknown));
                indices.push(self.model.index_info(
                    string,
                    value,
                    readonly && index.readonly,
                    None,
                    None,
                ));
            }
            let limited =
                sequence.read(self.limited_reverse_constraint_completion(key, &consumer))?;
            let mut properties = Vec::new();
            for property in
                sequence.read(self.properties_of_type_completion(key.source, &consumer))?
            {
                let source = self.property_symbol(&property, &consumer)?;
                if let Some(limited) = limited {
                    let name = sequence.read(self.literal_type_from_property_completion(
                        source,
                        TypeFlags::STRING_LITERAL
                            | TypeFlags::NUMBER_LITERAL
                            | TypeFlags::UNIQUE_SYMBOL,
                        false,
                        &consumer,
                    ))?;
                    if !sequence.read(self.type_related_completion(
                        name,
                        limited,
                        RelationKind::Assignable,
                        &consumer,
                    ))? {
                        continue;
                    }
                }
                let symbol = self.create_reverse_mapped_symbol(
                    source,
                    key.target,
                    key.constraint,
                    readonly,
                    optional,
                    &consumer,
                )?;
                let name = self.reference_symbol_key(
                    super::super::ReferenceSymbol::from_property(source),
                    &consumer,
                )?;
                properties.push((name, symbol));
            }
            let members = MembersView {
                properties: Arc::from([]),
                call_signatures: Arc::from([]),
                construct_signatures: Arc::from([]),
                index_signatures: indices.into(),
            };
            table = Some(super::super::structure::MemberTable::Symbols(
                properties.into(),
            ));
            Ok(members)
        })();
        let members = sequence.finish(result)?;
        self.set_structured_member_table_completion(ty, table, members, &consumer)
    }

    /// `getLimitedConstraint` uses a union's actual intersection origin, not
    /// the reduced constituents or displayed summary origin.
    fn limited_reverse_constraint_completion(
        &mut self,
        key: ReverseKey,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let constraint =
                sequence.read(self.mapped_constraint_completion(key.target, consumer))?;
            let origin = match self.model.get(constraint)? {
                TypeView::Union(_) => self.model.union_origin(constraint),
                TypeView::Intersection(_) => Some(constraint),
                _ => None,
            };
            let Some(origin) = origin else {
                return Ok(None);
            };
            let Some(types) = self.model.intersection_constituents(origin) else {
                return Ok(None);
            };
            let types = types
                .iter()
                .copied()
                .filter(|&ty| ty != key.constraint)
                .collect();
            let limited = sequence.read(self.intersection_type_completion(types, consumer))?;
            Ok((!self.model.has_type_flags(limited, TypeFlags::NEVER)).then_some(limited))
        })();
        sequence.finish(result)
    }

    /// `replaceIndexedAccess` maps the index to zero and the object to the
    /// singleton tuple through the ordinary tuple and instantiation factories.
    pub(in crate::project::semantic::evaluation) fn replace_indexed_access(
        &mut self,
        instantiable: TypeId,
        indexed: TypeId,
        replacement: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.replace_indexed_access_completion(instantiable, indexed, replacement, consumer)?
            .admit()
    }

    pub(in crate::project::semantic::evaluation) fn replace_indexed_access_completion(
        &mut self,
        instantiable: TypeId,
        indexed: TypeId,
        replacement: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let TypeView::IndexedAccess(access) = self.model.get(indexed)? else {
                return Err(super::super::super::internal(
                    "replaceIndexedAccess requires an indexed type",
                )
                .into());
            };
            let zero = self
                .model
                .literal(crate::types::PrimitiveName::Number, "0".into());
            let tuple = sequence.read(self.create_tuple_type_ex_completion(
                vec![replacement],
                vec![crate::project::semantic::model::TupleElementInfo {
                    flags: TupleElementFlags::REQUIRED,
                    label: None,
                }],
                false,
                consumer,
            ))?;
            let mapper = self.model.mapper(
                vec![access.index_type, access.object_type],
                vec![zero, tuple],
            )?;
            sequence.read(self.instantiate_type_completion(instantiable, mapper, consumer))
        })();
        sequence.finish(result)
    }
}
