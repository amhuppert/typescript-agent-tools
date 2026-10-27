//! Native tuple targets, normalization and deferred reference arguments.

use super::super::model::KeyBuilder;
use super::super::model::{
    Alias, DeferredReference, DemandError, MapperId, MembersView, NativeCompletion, NativeResult,
    NativeSequence, ObjectFlags, SpecialName, TupleElementFlags as Flags, TupleElementInfo,
    TupleTarget, TypeFlags, TypeId, UnionReduction,
};
use super::super::source::{
    AliasResolutionRecipe, ArrayOrTupleRecipe, DeferredReferenceRecipe, TupleElementRecipe,
    TypeExpression,
};
use super::{Evaluator, resolution};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::NativeNodeIdentity;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::resources::{CheckedKey, CheckedMap};
use crate::types::PrimitiveName;
use std::collections::HashMap;
use std::sync::Arc;

/// `getTupleKey`: ordered element flags and actual label declarations, plus readonly.
struct TupleKey {
    elements: Vec<(Flags, Option<NativeNodeIdentity>)>,
    readonly: bool,
    fingerprint: u64,
}

impl TupleKey {
    fn new(elements: &[TupleElementInfo], readonly: bool) -> Self {
        let elements = {
            (elements.iter().map(|element| {
                (
                    element.flags,
                    element.label.as_ref().map(|label| label.declaration),
                )
            }))
            .collect::<Vec<_>>()
        };
        let fingerprint = {
            let mut hash = KeyBuilder::new();
            hash.fixed(&readonly);
            hash.fixed(&elements.len());
            for element in &elements {
                hash.fixed(element);
            }
            hash.finish()
        };
        Self {
            elements,
            readonly,
            fingerprint,
        }
    }
}

impl CheckedKey for TupleKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        if self.readonly != other.readonly || self.elements.len() != other.elements.len() {
            return false;
        }
        for (left, right) in self.elements.iter().zip(&other.elements) {
            if left != right {
                return false;
            }
        }
        true
    }
}

#[derive(Clone)]
pub(super) struct TupleAssociation {
    pub(super) members: MembersView,
    pub(super) consumer: Location,
}

#[derive(Clone)]
pub(super) struct DeferredArguments {
    pub(super) origin: TypeId,
    pub(super) recipe: DeferredReferenceRecipe,
    arguments: Vec<TypeExpression>,
    class_reference: bool,
    pub(super) consumer: Location,
}

#[derive(Default)]
pub(super) struct TuplesState {
    nodes: HashMap<(FileId, Site), NativeResult<TypeId>>,
    targets: CheckedMap<TupleKey, TypeId>,
    pub(super) associations: HashMap<TypeId, TupleAssociation>,
    pub(super) deferred: HashMap<TypeId, DeferredArguments>,
}

impl TuplesState {
    pub(super) fn new() -> Self {
        Self {
            targets: CheckedMap::new(),
            ..Self::default()
        }
    }
}

impl Evaluator<'_, '_> {
    /// `getTypeFromArrayOrTupleTypeNode`: target acquisition precedes alias
    /// deferral, which in turn precedes ordered element checking.
    pub(super) fn array_or_tuple_type_node_completion(
        &mut self,
        expression: &TypeExpression,
        recipe: &ArrayOrTupleRecipe,
    ) -> NativeResult<TypeId> {
        let key = (expression.file, recipe.site);
        if let Some(result) = self.tuples.nodes.get(&key) {
            return result.clone();
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let target = match &recipe.tuple {
                Some(elements) => sequence.read(self.get_tuple_target_type_completion(
                    elements,
                    recipe.readonly,
                    &expression.location,
                ))?,
                None => sequence
                    .read(self.array_target_completion(recipe.readonly, &expression.location))?,
            };
            // getTypeFromArrayOrTupleTypeNode returns before deferral or
            // element acquisition when the fixed global target is missing.
            if target == self.model.empty_generic_type() {
                return Ok(self.model.empty_object_type());
            }
            if !recipe.variadic && self.is_deferred_type_reference_node(&recipe.deferred, false)? {
                if recipe.elements.is_empty() {
                    return Ok(target);
                }
                return sequence.read(self.create_deferred_reference_completion(
                    target,
                    recipe.deferred.clone(),
                    recipe.elements.clone(),
                    false,
                    None,
                    None,
                    &expression.location,
                ));
            }
            let elements =
                sequence.read(self.type_arguments_from_nodes_completion(&recipe.elements))?;
            sequence.read(self.create_normalized_reference_ex_completion(
                target,
                elements,
                ObjectFlags::FROM_TYPE_NODE,
                &expression.location,
            ))
        })();
        let result = sequence.finish(result);
        self.tuples.nodes.insert(key, result.clone());
        result
    }

    /// `getTypeFromNamedTupleTypeNode` / `getTypeFromOptionalTypeNode` keep
    /// the child's actual recovery value through optionality and cache it.
    pub(super) fn tuple_element_type_node_completion(
        &mut self,
        expression: &TypeExpression,
        recipe: &TupleElementRecipe,
    ) -> NativeResult<TypeId> {
        let key = (expression.file, recipe.site);
        if recipe.cached
            && let Some(result) = self.tuples.nodes.get(&key)
        {
            return result.clone();
        }
        let result = self.check_type_completion(&recipe.value).and_then(|value| {
            value.and_then(|ty| {
                self.add_optionality_completion(ty, true, recipe.optional, &expression.location)
            })
        });
        if recipe.cached {
            self.tuples.nodes.insert(key, result.clone());
        }
        result
    }

    /// `getTypeFromTypeOperatorNode` caches readonly's delegated child value.
    pub(super) fn readonly_type_node_completion(
        &mut self,
        expression: &TypeExpression,
        site: Site,
        value: &TypeExpression,
    ) -> NativeResult<TypeId> {
        let key = (expression.file, site);
        if let Some(result) = self.tuples.nodes.get(&key) {
            return result.clone();
        }
        let result = self.check_type_completion(value);
        self.tuples.nodes.insert(key, result.clone());
        result
    }

    /// `isDeferredTypeReferenceNode` short-circuits direct aliases before
    /// asking any argument's separate mayResolveTypeAlias question.
    pub(super) fn is_deferred_type_reference_node(
        &mut self,
        recipe: &DeferredReferenceRecipe,
        has_defaults: bool,
    ) -> Result<bool, DemandError> {
        if recipe.alias.is_some() {
            return Ok(true);
        }
        if !recipe.resolved_by_alias {
            return Ok(false);
        }
        if has_defaults {
            return Ok(true);
        }
        for argument in &recipe.arguments {
            if self.may_resolve_type_alias(argument)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn may_resolve_type_alias(
        &mut self,
        recipe: &AliasResolutionRecipe,
    ) -> Result<bool, DemandError> {
        match recipe {
            AliasResolutionRecipe::No => Ok(false),
            AliasResolutionRecipe::Yes => Ok(true),
            AliasResolutionRecipe::Reference {
                file,
                anchor,
                location,
            } => {
                let (symbol, failure) = self
                    .reference_entity_at_completion(
                        *file,
                        *anchor,
                        true,
                        crate::project::binding::NativeLookupMeaning::TYPE,
                        false,
                        location,
                    )?
                    .admit()?;
                let symbol = symbol.ok_or_else(|| {
                    failure.unwrap_or_else(|| {
                        super::super::internal("type alias recipe has no entity symbol").into()
                    })
                })?;
                Ok(self
                    .reference_symbol_native_flags(symbol, location)?
                    .type_flags()
                    .type_alias)
            }
            AliasResolutionRecipe::Some(children) => {
                for child in children {
                    if self.may_resolve_type_alias(child)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            AliasResolutionRecipe::Unavailable(location) => {
                Err(super::super::unsupported_demand_at_location(
                    self.inputs,
                    location,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "native type-reference alias probe lacks its entity-name association",
                ))
            }
        }
    }

    /// `createDeferredTypeReference` allocates a fresh reference; native
    /// instantiation cache ownership is separate from this factory.
    #[expect(
        clippy::too_many_arguments,
        reason = "ports createDeferredTypeReference's parameters plus the request consumer"
    )]
    pub(super) fn create_deferred_reference_completion(
        &mut self,
        target: TypeId,
        recipe: DeferredReferenceRecipe,
        arguments: Vec<TypeExpression>,
        class_reference: bool,
        mapper: Option<MapperId>,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let alias = match alias {
                Some(alias) => Some(alias),
                None => {
                    let alias = recipe
                        .alias
                        .as_ref()
                        .map(|recipe| sequence.read(self.establish_alias_completion(recipe)))
                        .transpose()?;
                    match mapper {
                        Some(mapper) => sequence.read(
                            self.instantiate_type_alias_completion(alias, mapper, consumer),
                        )?,
                        None => alias,
                    }
                }
            };
            let ty = self.model.deferred_reference_type(
                target,
                DeferredReference {
                    node: recipe.node,
                    declaration: recipe.declaration,
                    mapper,
                },
                alias,
            )?;
            self.tuples.deferred.insert(
                ty,
                DeferredArguments {
                    origin: ty,
                    recipe,
                    arguments,
                    class_reference,
                    consumer: consumer.clone(),
                },
            );
            Ok(ty)
        })();
        sequence.finish(result)
    }

    pub(super) fn instantiate_deferred_reference_completion(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        alias: Option<Alias>,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let original = self.tuples.deferred.get(&ty).cloned().ok_or_else(|| {
                super::super::internal("deferred reference lacks its argument recipe")
            })?;
            let target = self
                .model
                .reference_target(ty)
                .expect("deferred reference target");
            let result = sequence.read(self.create_deferred_reference_completion(
                target,
                original.recipe,
                original.arguments,
                original.class_reference,
                Some(mapper),
                alias,
                &original.consumer,
            ))?;
            self.tuples
                .deferred
                .get_mut(&result)
                .expect("fresh deferred association")
                .origin = original.origin;
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `getTypeArguments` owns the native property stack and installs after
    /// popping, including the install-if-absent rule under reentrant mapping.
    pub(in crate::project::semantic) fn get_type_arguments(
        &mut self,
        ty: TypeId,
    ) -> Result<Arc<[TypeId]>, DemandError> {
        self.get_type_arguments_completion(ty)?.admit()
    }

    pub(in crate::project::semantic) fn get_type_arguments_completion(
        &mut self,
        ty: TypeId,
    ) -> NativeResult<Arc<[TypeId]>> {
        if let Some(result) = self.model.resolved_type_arguments_completion(ty) {
            return result;
        }
        let recipe = self.tuples.deferred.get(&ty).cloned().ok_or_else(|| {
            super::super::internal("pending type arguments lack their native node")
        })?;
        let target_type = self
            .model
            .reference_target(ty)
            .ok_or_else(|| super::super::internal("deferred arguments have no target"))?;
        let target = self.model.declared_target(target_type)?;
        let parameter_count = target.outer_parameters.len() + target.local_parameters.len();
        let inputs = self.inputs;
        let circular = || {
            super::super::invalid_at_location(
                inputs,
                &recipe.consumer,
                SemanticFailureReason::CircularType,
                "type arguments circularly reference themselves",
            )
            .into()
        };
        if !self.push_resolution(resolution::Key::TypeArguments(ty)) {
            return NativeCompletion::recovered(
                vec![self.model.native_error_type(); parameter_count].into(),
                circular(),
            );
        }
        let result = self
            .type_arguments_from_nodes_completion(&recipe.arguments)
            .and_then(|arguments| {
                arguments.and_then(|arguments| {
                    if recipe.class_reference {
                        self.fill_missing_type_arguments_completion(
                            arguments,
                            &target.local_parameters,
                            self.inputs
                                .in_javascript_file(recipe.recipe.declaration.file),
                            &recipe.consumer,
                        )
                        .map(|value| {
                            value.map(|arguments| {
                                let mut outer = target.outer_parameters.to_vec();
                                outer.extend(arguments);
                                outer
                            })
                        })
                    } else {
                        Ok(NativeCompletion::clean(arguments))
                    }
                })
            });
        let complete = self.resolution.pop().ok_or_else(|| {
            super::super::internal("type-argument resolution stack is unbalanced")
        })?;
        let arguments = match result {
            Ok(value) => value,
            Err(error) => return self.model.complete_type_arguments(ty, Err(error)),
        };
        if !complete {
            let recovery = arguments.and_then(|_| {
                NativeCompletion::recovered(
                    vec![self.model.native_error_type(); parameter_count].into(),
                    circular(),
                )
            });
            // Native publishes the actual error array before symbol display.
            let installed = self.model.complete_type_arguments(ty, recovery)?;
            return installed.and_then(|arguments| {
                match self.reference_symbol_of_type(target_type, &recipe.consumer)? {
                    Some(symbol) => self
                        .diagnostic_symbol_display_completion(symbol, &recipe.consumer)
                        .map(|display| display.map(|_| arguments)),
                    None => Ok(NativeCompletion::clean(arguments)),
                }
            });
        }
        if let Some(installed) = self.model.resolved_type_arguments_completion(ty) {
            return match installed {
                Ok(installed) => Ok(arguments.map(|_| installed.value().clone())),
                Err(error) => arguments.and_then(|_| Err(error)),
            };
        }
        let mapper = self
            .model
            .deferred_reference(ty)
            .expect("deferred header")
            .mapper;
        let result = arguments.and_then(|arguments| {
            let mut sequence = NativeSequence::new();
            let result = match mapper {
                Some(mapper) => arguments
                    .into_iter()
                    .map(|argument| {
                        sequence.read(self.instantiate_type_completion(
                            argument,
                            mapper,
                            &recipe.consumer,
                        ))
                    })
                    .collect::<Result<Arc<[_]>, _>>(),
                None => Ok(arguments.into()),
            };
            sequence.finish(result)
        });
        self.model.complete_type_arguments(ty, result)
    }

    /// `getTupleTargetType` has exact flags, readonly and label-node keys.
    fn get_tuple_target_type_completion(
        &mut self,
        elements: &[TupleElementInfo],
        readonly: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if elements.len() == 1 && elements[0].flags.intersects(Flags::REST) {
                return sequence.read(self.array_target_completion(readonly, consumer));
            }
            let key = TupleKey::new(elements, readonly);
            if let Some(&target) = self.tuples.targets.get(&key) {
                return Ok(target);
            }
            let minimum_length = elements
                .iter()
                .filter(|element| element.flags.intersects(Flags::REQUIRED | Flags::VARIADIC))
                .count();
            let mut parameters = Vec::with_capacity(elements.len());
            let mut properties = Vec::new();
            let mut combined_flags = Flags::NONE;
            for (index, element) in elements.iter().enumerate() {
                let parameter = self.model.synthetic_type_parameter();
                parameters.push(parameter);
                combined_flags = combined_flags | element.flags;
                if !combined_flags.intersects(Flags::VARIABLE) {
                    let symbol = self.create_known_property(
                        &index.to_string(),
                        element.flags.intersects(Flags::OPTIONAL),
                        readonly,
                        parameter,
                    );
                    properties.push(self.project_property_symbol(symbol, consumer)?);
                }
            }
            let fixed_length = properties.len();
            let length = if combined_flags.intersects(Flags::VARIABLE) {
                self.model.primitive(PrimitiveName::Number)
            } else {
                let mut literals = Vec::new();
                for length in minimum_length..=elements.len() {
                    literals.push(
                        self.model
                            .regular_literal_type(PrimitiveName::Number, length.to_string()),
                    );
                }
                sequence.read(self.union_type_ex_completion(literals, None, None, consumer))?
            };
            let symbol = self.create_known_property("length", false, readonly, length);
            properties.push(self.project_property_symbol(symbol, consumer)?);
            let target = self.model.tuple_target_type(
                parameters,
                TupleTarget {
                    elements: elements.into(),
                    minimum_length,
                    fixed_length,
                    combined_flags,
                    readonly,
                },
            )?;
            self.tuples.associations.insert(
                target,
                TupleAssociation {
                    members: MembersView {
                        properties: properties.into(),
                        call_signatures: Arc::from([]),
                        construct_signatures: Arc::from([]),
                        index_signatures: Arc::from([]),
                    },
                    consumer: consumer.clone(),
                },
            );
            self.tuples.targets.insert(key, target);
            Ok(target)
        })();
        sequence.finish(result)
    }

    pub(super) fn create_tuple_type_ex(
        &mut self,
        elements: Vec<TypeId>,
        infos: Vec<TupleElementInfo>,
        readonly: bool,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.create_tuple_type_ex_completion(elements, infos, readonly, consumer)?
            .admit()
    }

    /// `createTupleTypeEx` selects the target before normalized construction.
    pub(super) fn create_tuple_type_ex_completion(
        &mut self,
        elements: Vec<TypeId>,
        infos: Vec<TupleElementInfo>,
        readonly: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.get_tuple_target_type_completion(&infos, readonly, consumer)?
            .and_then(|target| {
                if target == self.model.empty_generic_type() {
                    return Ok(NativeCompletion::clean(self.model.empty_object_type()));
                }
                if elements.is_empty() {
                    return Ok(NativeCompletion::clean(target));
                }
                self.create_normalized_reference_completion(target, elements, consumer)
            })
    }

    pub(super) fn create_normalized_reference_completion(
        &mut self,
        target: TypeId,
        arguments: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.create_normalized_reference_ex_completion(
            target,
            arguments,
            ObjectFlags::NONE,
            consumer,
        )
    }

    fn create_normalized_reference_ex_completion(
        &mut self,
        target: TypeId,
        arguments: Vec<TypeId>,
        flags: ObjectFlags,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(tuple) = self.model.tuple_target(target)? else {
                return self.model.reference_ex(target, arguments, flags);
            };
            if !tuple.combined_flags.intersects(Flags::NON_REQUIRED) {
                return self.model.reference_ex(target, arguments, flags);
            }
            if tuple.combined_flags.intersects(Flags::VARIADIC) {
                for (index, info) in tuple.elements.iter().enumerate() {
                    let element = arguments[index];
                    if info.flags.intersects(Flags::VARIADIC)
                        && self
                            .model
                            .has_type_flags(element, TypeFlags::NEVER | TypeFlags::UNION)
                    {
                        let unknown = self.model.special(SpecialName::Unknown);
                        let check: Vec<_> = arguments
                            .iter()
                            .enumerate()
                            .map(|(index, &ty)| {
                                if tuple
                                    .elements
                                    .get(index)
                                    .is_some_and(|info| info.flags.intersects(Flags::VARIADIC))
                                {
                                    ty
                                } else {
                                    unknown
                                }
                            })
                            .collect();
                        if sequence
                            .read(self.check_cross_product_union_completion(&check, consumer))?
                            .is_some()
                        {
                            return sequence.read(self.map_type_completion(
                                element,
                                consumer,
                                |evaluator, value| {
                                    let mut replaced = arguments.clone();
                                    replaced[index] = value;
                                    evaluator.create_normalized_reference_ex_completion(
                                        target, replaced, flags, consumer,
                                    )
                                },
                            ));
                        }
                    }
                }
            }
            let Some((mut types, infos)) = sequence.read(self.normalize_tuple_completion(
                &arguments[..tuple.elements.len()],
                &tuple.elements,
                consumer,
            ))?
            else {
                return Ok(self.model.native_error_type());
            };
            if arguments.len() > tuple.elements.len() {
                types.push(arguments[tuple.elements.len()]);
            }
            let target = sequence.read(self.get_tuple_target_type_completion(
                &infos,
                tuple.readonly,
                consumer,
            ))?;
            if types.is_empty() {
                Ok(target)
            } else {
                self.model.reference_ex(target, types, flags)
            }
        })();
        sequence.finish(result)
    }

    /// `TupleNormalizer.normalize` owns expansion and the required/optional/
    /// rest layout. It never infers tuple structure from a printed type.
    fn normalize_tuple_completion(
        &mut self,
        types: &[TypeId],
        infos: &[TupleElementInfo],
        consumer: &Location,
    ) -> NativeResult<Option<(Vec<TypeId>, Vec<TupleElementInfo>)>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut normalized = Vec::new();
            let mut result_infos = Vec::new();
            let mut last_required = None;
            let mut first_rest = None;
            let mut last_optional_or_rest = None;
            for (&ty, info) in types.iter().zip(infos) {
                let additions = if info.flags.intersects(Flags::VARIADIC) {
                    if self.model.has_type_flags(ty, TypeFlags::ANY) {
                        vec![(
                            ty,
                            TupleElementInfo {
                                flags: Flags::REST,
                                label: info.label.clone(),
                            },
                        )]
                    } else if self
                        .model
                        .has_type_flags(ty, TypeFlags::INSTANTIABLE_NON_PRIMITIVE)
                        || self.is_generic_mapped_type(ty, consumer)?
                    {
                        vec![(ty, info.clone())]
                    } else if let Some(tuple) = self.model.tuple_target(ty)? {
                        let spread = sequence.read(self.get_element_types_completion(ty))?;
                        if spread.len() + normalized.len() >= 10_000 {
                            return sequence.read(NativeCompletion::recovered(
                                None,
                                super::super::invalid_at_location(
                                    self.inputs,
                                    consumer,
                                    SemanticFailureReason::TupleComplexityLimit,
                                    "type produces a tuple type that is too large to represent",
                                )
                                .into(),
                            ));
                        }
                        spread
                            .iter()
                            .copied()
                            .zip(tuple.elements.iter().cloned())
                            .collect()
                    } else {
                        let mut element = None;
                        if self.is_array_like_type(ty, consumer)? {
                            let number = self.model.primitive(PrimitiveName::Number);
                            element = sequence
                                .read(self.index_type_of_type_completion(ty, number, consumer))?;
                        }
                        let element = match element {
                            Some(element) => element,
                            None => sequence.read(NativeCompletion::recovered(
                                self.model.native_error_type(),
                                super::super::invalid_at_location(
                                    self.inputs,
                                    consumer,
                                    SemanticFailureReason::ErrorType,
                                    "variadic tuple element consumes native array-element recovery",
                                )
                                .into(),
                            ))?,
                        };
                        vec![(
                            element,
                            TupleElementInfo {
                                flags: Flags::REST,
                                label: info.label.clone(),
                            },
                        )]
                    }
                } else {
                    vec![(ty, info.clone())]
                };
                for (ty, info) in additions {
                    if info.flags.intersects(Flags::REQUIRED) {
                        last_required = Some(normalized.len());
                    }
                    if info.flags.intersects(Flags::REST) && first_rest.is_none() {
                        first_rest = Some(normalized.len());
                    }
                    if info.flags.intersects(Flags::OPTIONAL | Flags::REST) {
                        last_optional_or_rest = Some(normalized.len());
                    }
                    normalized.push(sequence.read(self.add_optionality_completion(
                        ty,
                        true,
                        info.flags.intersects(Flags::OPTIONAL),
                        consumer,
                    ))?);
                    result_infos.push(info);
                }
            }
            if let Some(last) = last_required {
                for info in &mut result_infos[..last] {
                    if info.flags.intersects(Flags::OPTIONAL) {
                        info.flags = Flags::REQUIRED;
                    }
                }
            }
            if let (Some(first), Some(last)) = (first_rest, last_optional_or_rest)
                && first < last
            {
                let mut merged = Vec::new();
                for index in first..=last {
                    let ty = if result_infos[index].flags.intersects(Flags::VARIADIC) {
                        let number_type = self.model.primitive(PrimitiveName::Number);
                        sequence.read(self.indexed_access_type_completion(
                            normalized[index],
                            number_type,
                            consumer,
                        ))?
                    } else {
                        normalized[index]
                    };
                    merged.push(ty);
                }
                normalized[first] =
                    sequence.read(self.union_type_ex_completion(merged, None, None, consumer))?;
                normalized.drain(first + 1..=last);
                result_infos.drain(first + 1..=last);
            }
            Ok(Some((normalized, result_infos)))
        })();
        sequence.finish(result)
    }

    /// NodeBuilder `typeReferenceToTypeNode` removes the exact Missing marker
    /// from optional tuple arguments before constructing any element nodes.
    /// Raw argument and graph-element questions keep their original identities.
    pub(in crate::project::semantic) fn tuple_arguments_for_display(
        &mut self,
        ty: TypeId,
    ) -> Result<Arc<[TypeId]>, DemandError> {
        self.get_type_arguments_completion(ty)?
            .and_then_value(|arguments| self.tuple_arguments_for_display_from(ty, &arguments))?
            .admit()
    }

    /// Continuation of `typeReferenceToTypeNode` after its one type-argument
    /// acquisition. Diagnostic serialization carries that acquisition's
    /// completion through this pure Missing-marker transformation.
    pub(super) fn tuple_arguments_for_display_from(
        &mut self,
        ty: TypeId,
        arguments: &[TypeId],
    ) -> Result<Arc<[TypeId]>, DemandError> {
        let tuple = self
            .model
            .tuple_target(ty)?
            .ok_or_else(|| super::super::internal("tuple display requires a tuple target"))?;
        let target = self
            .model
            .reference_target(ty)
            .expect("tuple reference target");
        let consumer = self
            .tuples
            .associations
            .get(&target)
            .ok_or_else(|| {
                super::super::internal("tuple display target lacks its factory association")
            })?
            .consumer
            .clone();
        arguments
            .iter()
            .enumerate()
            .map(|(index, &argument)| {
                let optional = tuple
                    .elements
                    .get(index)
                    .is_some_and(|element| element.flags.intersects(Flags::OPTIONAL));
                self.model
                    .remove_missing_type(argument, optional)
                    .map_err(|failure| failure.at(self.inputs, &consumer))
            })
            .collect()
    }

    pub(in crate::project::semantic) fn get_element_types(
        &mut self,
        ty: TypeId,
    ) -> Result<Arc<[TypeId]>, DemandError> {
        self.get_element_types_completion(ty)?.admit()
    }

    /// `getElementTypes` resolves arguments before excluding synthetic this.
    pub(super) fn get_element_types_completion(
        &mut self,
        ty: TypeId,
    ) -> NativeResult<Arc<[TypeId]>> {
        self.get_type_arguments_completion(ty)?
            .and_then_value(|arguments| {
                let arity = self.type_reference_arity(ty)?;
                Ok(if arguments.len() == arity {
                    arguments
                } else {
                    Arc::from(&arguments[..arity])
                })
            })
    }

    /// `getTypeReferenceArity` excludes synthetic this without acquiring any
    /// deferred reference arguments.
    pub(super) fn type_reference_arity(&self, ty: TypeId) -> Result<usize, DemandError> {
        let target = self
            .model
            .declared_target(self.model.reference_target(ty).ok_or_else(|| {
                super::super::internal("type reference arity requires an actual target")
            })?)?;
        Ok(target.outer_parameters.len() + target.local_parameters.len())
    }

    pub(super) fn get_element_type_of_slice_of_tuple_type_completion(
        &mut self,
        ty: TypeId,
        index: usize,
        end_skip: usize,
        writing: bool,
        no_reductions: bool,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let tuple = self
                .model
                .tuple_target(ty)?
                .ok_or_else(|| super::super::internal("tuple slice requires tuple target"))?;
            let length = tuple.elements.len().saturating_sub(end_skip);
            if index >= length {
                return Ok(None);
            }
            let arguments = sequence.read(self.get_type_arguments_completion(ty))?;
            let mut types = Vec::new();
            for i in index..length {
                types.push(if tuple.elements[i].flags.intersects(Flags::VARIADIC) {
                    sequence.read(self.tuple_numeric_access_completion(arguments[i], consumer))?
                } else {
                    arguments[i]
                });
            }
            if writing {
                sequence
                    .read(self.intersection_type_completion(types, consumer))
                    .map(Some)
            } else {
                sequence
                    .read(self.union_type_reduced_completion(
                        types,
                        if no_reductions {
                            UnionReduction::None
                        } else {
                            UnionReduction::Literal
                        },
                        None,
                        None,
                        consumer,
                    ))
                    .map(Some)
            }
        })();
        sequence.finish(result)
    }

    pub(super) fn get_rest_type_of_tuple_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let tuple = self
                .model
                .tuple_target(ty)?
                .ok_or_else(|| super::super::internal("tuple rest requires tuple target"))?;
            sequence.read(self.get_element_type_of_slice_of_tuple_type_completion(
                ty,
                tuple.fixed_length,
                0,
                false,
                false,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    pub(super) fn get_rest_array_type_of_tuple_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            match sequence.read(self.get_rest_type_of_tuple_type_completion(ty, consumer))? {
                Some(rest) => sequence
                    .read(self.array_type_completion(rest, consumer))
                    .map(Some),
                None => Ok(None),
            }
        })();
        sequence.finish(result)
    }

    pub(super) fn slice_tuple_type(
        &mut self,
        ty: TypeId,
        index: usize,
        end_skip: usize,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.slice_tuple_type_completion(ty, index, end_skip, consumer)?
            .admit()
    }

    /// `sliceTupleType` retains rest acquisition before empty/sliced factories.
    pub(super) fn slice_tuple_type_completion(
        &mut self,
        ty: TypeId,
        index: usize,
        end_skip: usize,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let tuple = self
            .model
            .tuple_target(ty)?
            .ok_or_else(|| super::super::internal("tuple slice requires tuple target"))?;
        let end = tuple.elements.len().saturating_sub(end_skip);
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if index > tuple.fixed_length
                && let Some(rest) = sequence
                    .read(self.get_rest_array_type_of_tuple_type_completion(ty, consumer))?
            {
                return Ok(rest);
            }
            if index > tuple.fixed_length || index >= end {
                return sequence.read(self.create_tuple_type_ex_completion(
                    Vec::new(),
                    Vec::new(),
                    false,
                    consumer,
                ));
            }
            let arguments = sequence.read(self.get_type_arguments_completion(ty))?;
            sequence.read(self.create_tuple_type_ex_completion(
                arguments[index..end].to_vec(),
                tuple.elements[index..end].to_vec(),
                false,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `getTupleElementTypeOutOfStartCount` retains each rest acquisition,
    /// including its optional absence, before the native missing union.
    pub(super) fn tuple_element_type_out_of_start_count_completion(
        &mut self,
        ty: TypeId,
        index: f64,
        undefined_like: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.map_type_completion(ty, consumer, |evaluator, ty| {
            evaluator
                .get_rest_type_of_tuple_type_completion(ty, consumer)?
                .and_then(|rest| {
                    let Some(rest) = rest else {
                        return Ok(NativeCompletion::clean(
                            evaluator.model.special(SpecialName::Undefined),
                        ));
                    };
                    let tuple = evaluator
                        .model
                        .tuple_target(ty)?
                        .expect("tuple tail target");
                    if let Some(undefined) = undefined_like
                        && index >= tuple.total_fixed_element_count() as f64
                    {
                        return evaluator.union_type_ex_completion(
                            vec![rest, undefined],
                            None,
                            None,
                            consumer,
                        );
                    }
                    Ok(NativeCompletion::clean(rest))
                })
        })
    }

    /// `getTupleBaseType` maps variadic parameters through actual T[number]
    /// before creating the one Array/ReadonlyArray base.
    pub(super) fn tuple_base_type_completion(
        &mut self,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let tuple = self.model.tuple_target(target)?.expect("tuple base target");
            let parameters = self.model.declared_target(target)?.local_parameters;
            let mut elements = Vec::new();
            for (&parameter, info) in parameters.iter().zip(tuple.elements.iter()) {
                elements.push(if info.flags.intersects(Flags::VARIADIC) {
                    sequence.read(self.tuple_numeric_access_completion(parameter, consumer))?
                } else {
                    parameter
                });
            }
            let element =
                sequence.read(self.union_type_ex_completion(elements, None, None, consumer))?;
            if tuple.readonly {
                sequence.read(self.readonly_array_type_completion(element, consumer))
            } else {
                sequence.read(self.array_type_completion(element, consumer))
            }
        })();
        sequence.finish(result)
    }

    fn tuple_numeric_access_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = {
            let number = self.model.primitive(PrimitiveName::Number);
            sequence.read(self.indexed_access_type_completion(ty, number, consumer))
        };
        sequence.finish(result)
    }

    /// `isArrayOrTupleType` compares array targets before inspecting tuple
    /// flags; neither question resolves deferred type arguments.
    pub(super) fn is_array_or_tuple_type(&mut self, ty: TypeId) -> Result<bool, DemandError> {
        self.is_array_or_tuple_type_completion(ty)?.admit()
    }

    pub(super) fn is_array_or_tuple_type_completion(&mut self, ty: TypeId) -> NativeResult<bool> {
        Ok(self
            .is_array_type_completion(ty)?
            .map(|array| array || self.model.is_tuple_type(ty)))
    }

    /// `getMutableArrayOrTupleType`: union origins are mapped by the shared
    /// mapper; the constraint fast path precedes tuple argument acquisition.
    pub(super) fn mutable_array_or_tuple_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        if self.model.has_type_flags(ty, TypeFlags::UNION) {
            return self.map_type(ty, consumer, |checker, child| {
                checker.mutable_array_or_tuple_type(child, consumer)
            });
        }
        if self.model.has_type_flags(ty, TypeFlags::ANY) {
            return Ok(ty);
        }
        let constrained = self.base_constraint(ty, consumer)?.unwrap_or(ty);
        if self.is_mutable_array_or_tuple(constrained)? {
            return Ok(ty);
        }
        if let Some(tuple) = self.model.tuple_target(ty)? {
            let elements = self.get_element_types(ty)?;
            return self.create_tuple_type_ex(
                elements.to_vec(),
                tuple.elements.to_vec(),
                false,
                consumer,
            );
        }
        self.create_tuple_type_ex(
            vec![ty],
            vec![TupleElementInfo {
                flags: Flags::VARIADIC,
                label: None,
            }],
            false,
            consumer,
        )
    }

    /// Product admission for the shared mutable array/tuple identity question.
    pub(super) fn is_mutable_array_or_tuple(&mut self, ty: TypeId) -> Result<bool, DemandError> {
        self.is_mutable_array_or_tuple_completion(ty)?.admit()
    }

    /// `isMutableArrayOrTuple` reads Array and guarded ReadonlyArray identity
    /// before the tuple's own readonly flag; it does not acquire arguments.
    pub(super) fn is_mutable_array_or_tuple_completion(
        &mut self,
        ty: TypeId,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if sequence.read(self.is_array_type_completion(ty))?
                && !sequence.read(self.is_readonly_array_type_completion(ty))?
            {
                return Ok(true);
            }
            Ok(self
                .model
                .tuple_target(ty)?
                .is_some_and(|tuple| !tuple.readonly))
        })();
        sequence.finish(result)
    }

    /// `isEmptyArrayLiteralType` uses the actual empty element intrinsic, not
    /// an ArrayLiteral flag or an arbitrary Never/Undefined lookalike.
    pub(super) fn is_empty_array_literal_type(&mut self, ty: TypeId) -> Result<bool, DemandError> {
        if !self.is_array_type(ty)? {
            return Ok(false);
        }
        let arguments = self.get_type_arguments(ty)?;
        Ok(arguments.first() == Some(&self.model.empty_array_element()))
    }
}
