//! Mapped declarations own lazy mapped-type links. Object instantiation,
//! member tables and symbol values retain their shared native owners.
use super::super::model::{
    DemandError, MapperId, Metadata, NativeCompletion, NativeResult, NativeSequence, ObjectFlags,
    SpecialName, TypeFlags, TypeId, TypeView,
};
use super::super::source::{ExpressionRoot, mapped::MappedRecipe};
use super::{Evaluator, relations::RelationKind};
use crate::references::Location;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NameKind {
    None,
    Filtering,
    Remapping,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Slot {
    Parameter,
    Constraint,
    Name,
    Template,
    Modifiers,
    Apparent,
}
// Six fixed native mapped-link questions; no mapped recipe participates.

pub(super) struct MappedState {
    nodes: crate::resources::FixedMap<ExpressionRoot, NativeCompletion<TypeId>>,
    declarations: crate::resources::FixedMap<TypeId, (ExpressionRoot, Arc<MappedRecipe>)>,
    slots: crate::resources::FixedMap<(TypeId, Slot), NativeCompletion<TypeId>>,
    contains_error: std::collections::HashSet<TypeId>,
    demands: crate::resources::FixedMap<(TypeId, MappedOperand), super::super::model::TypeDemand>,
}

impl Evaluator<'_, '_> {
    /// The complete modifier-preserving-wrapper condition in
    /// `createMappedTypeNodeFromType`. Callers have already acquired template
    /// and parameter; even default flags must ask the modifier source here.
    pub(in crate::project::semantic) fn mapped_display_needs_wrapper(
        &mut self,
        ty: TypeId,
        generate_names: bool,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        if self.mapped_recipe(ty)?.keyof_operand.is_some() {
            return Ok(false);
        }
        let modifiers = self.mapped_modifiers(ty, consumer)?;
        if self.model.has_type_flags(modifiers, TypeFlags::UNKNOWN) || !generate_names {
            return Ok(false);
        }
        let constraint = self.mapped_constraint(ty, consumer)?;
        if self
            .model
            .has_type_flags(constraint, TypeFlags::TYPE_PARAMETER)
        {
            let constraint = self.mapped_constraint(ty, consumer)?;
            if let Some(base) = self.parameter_constraint(constraint, consumer)?
                && self.model.has_type_flags(base, TypeFlags::INDEX)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `getMappedTargetWithSymbol` preserves the deepest written homomorphic
    /// target for the native recursion heuristic.
    pub(super) fn mapped_target_with_symbol_completion(
        &mut self,
        mut ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            loop {
                let flags = self.model.object_flags(ty)?;
                if !(flags.intersects(ObjectFlags::MAPPED)
                    && flags.intersects(ObjectFlags::INSTANTIATED))
                {
                    return Ok(ty);
                }
                let target = sequence.read(self.mapped_modifiers_completion(ty, consumer))?;
                let mut has_symbol = self.model.type_symbol(target).native()?.is_some();
                if !has_symbol && let Some(types) = self.model.intersection_constituents(target) {
                    for &child in types.iter() {
                        if self.model.type_symbol(child).native()?.is_some() {
                            has_symbol = true;
                            break;
                        }
                    }
                }
                if !has_symbol {
                    return Ok(ty);
                }
                ty = target;
            }
        })();
        sequence.finish(result)
    }

    /// `getApparentMappedTypeKeys` acquires modifier properties before indexes
    /// and substitutes each key through the actual mapped object's mapper.
    pub(super) fn apparent_mapped_type_keys_completion(
        &mut self,
        name: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let modifiers = sequence.read(self.mapped_modifiers_completion(target, consumer))?;
            let modifiers = sequence.read(self.apparent_type_completion(modifiers, consumer))?;
            let mut keys = Vec::new();
            sequence.read(self.for_each_mapped_key_completion(
                modifiers,
                false,
                consumer,
                |checker, key| {
                    let mut sequence = NativeSequence::new();
                    let result =
                        (|| -> Result<(), DemandError> {
                            let (_, mapper) = checker.object_target_mapper(target)?;
                            let parameter = sequence
                                .read(checker.mapped_parameter_completion(target, consumer))?;
                            let mapper = checker.append_type_mapping(mapper, parameter, key)?;
                            keys.push(sequence.read(
                                checker.instantiate_type_completion(name, mapper, consumer),
                            )?);
                            Ok(())
                        })();
                    sequence.finish(result)
                },
            ))?;
            sequence.read(self.union_type_ex_completion(keys, None, None, consumer))
        })();
        sequence.finish(result)
    }

    /// `checkMappedType`: grammar's extra-member diagnostic has no semantic
    /// child acquisition. Parameter, name and template source checks precede
    /// the mapped node and its reporting property-key assignability question.
    pub(super) fn check_mapped_type_completion(
        &mut self,
        root: ExpressionRoot,
    ) -> super::super::model::NativeResult<()> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let recipe = self.source.mapped_recipe(root)?;
            sequence.read(self.check_type_source_element_completion(recipe.parameter_root))?;
            for child in [&recipe.name, &recipe.template] {
                if let Some(root) = child.as_ref().and_then(|child| child.root) {
                    sequence.read(self.check_type_source_element_completion(root))?;
                }
            }
            let consumer = super::super::location(self.inputs, root.file, root.site.span);
            if recipe.template.is_none() {
                let declaration = self.source.declaration_from_expression_root(root)?;
                let any = self.model.special(SpecialName::Any);
                sequence.read(self.report_implicit_any_completion(declaration, any, &consumer))?;
            }
            let ty = sequence.read(self.mapped_type_node_completion(root, &consumer))?;
            let (key, node) = match sequence.read(self.mapped_name_completion(ty, &consumer))? {
                Some(name) => (name, recipe.name.as_ref().and_then(|name| name.root)),
                None => (
                    sequence.read(self.mapped_constraint_completion(ty, &consumer))?,
                    self.source
                        .type_parameter_check_children(recipe.parameter_root)?
                        .0,
                ),
            };
            let error = node
                .and_then(|node| super::relations::RelationReporting::Report.assignment_at(node));
            sequence.read(self.check_type_related_to_completion(
                key,
                self.model.string_number_symbol_type(),
                super::relations::RelationKind::Assignable,
                error,
                &consumer,
            ))?;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `getTypeFromMappedTypeNode` installs the allocated object before its
    /// eager constraint read. An interruption does not erase that native slot.
    pub(super) fn mapped_type_node_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(result) = self.mapped.nodes.get(&root) {
            return Ok(result.clone());
        }
        let recipe = Arc::new(self.source.mapped_recipe(root)?);
        let symbol = self.source.raw_symbol_of_declaration(recipe.declaration)?;
        let ty = self.model.anonymous_type_ex(
            self.type_symbol(symbol, consumer),
            Metadata::default(),
            ObjectFlags::MAPPED,
            None,
        );
        self.register_mapped_object(ty, recipe.declaration, consumer);
        self.mapped.declarations.insert(ty, (root, recipe.clone()));
        let alias = match recipe.alias.as_ref() {
            Some(alias) => self.establish_alias_completion(alias)?.map(Some),
            None => NativeCompletion::clean(None),
        };
        let completed = alias.and_then_value(|alias| {
            self.model.install_lazy_object_alias(ty, alias)?;
            Ok(ty)
        })?;
        self.mapped.nodes.insert(root, completed.clone());
        // The constraint is eager only on this invocation. The node slot was
        // installed before that read, so a warm node read must not replay it.
        completed.and_then(|ty| {
            self.mapped_constraint_completion(ty, consumer)
                .map(|value| value.map(|_| ty))
        })
    }

    pub(super) fn mapped_recipe(&self, ty: TypeId) -> Result<Arc<MappedRecipe>, DemandError> {
        self.mapped
            .declarations
            .get(&ty)
            .map(|(_, recipe)| recipe.clone())
            .ok_or_else(|| {
                super::super::internal("mapped type lacks its native declaration").into()
            })
    }

    /// `instantiateAnonymousType` copies mapped declaration and installs its
    /// fresh key parameter before the result can be observed by another owner.
    pub(super) fn clone_mapped_links(
        &mut self,
        result: TypeId,
        original: TypeId,
        parameter: TypeId,
    ) -> Result<(), DemandError> {
        let declaration = self
            .mapped
            .declarations
            .get(&original)
            .cloned()
            .ok_or_else(|| super::super::internal("mapped clone lacks its original declaration"))?;
        self.mapped.declarations.insert(result, declaration);
        self.mapped.slots.insert(
            (result, Slot::Parameter),
            NativeCompletion::clean(parameter),
        );
        Ok(())
    }

    /// The six native mapped links install only completed nonnil values.
    /// A syntactically absent name stays nil; a child interruption leaves the
    /// slot absent, while any reentrant installation remains intact.
    fn mapped_slot_completion(
        &mut self,
        ty: TypeId,
        slot: Slot,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        if let Some(result) = self.mapped.slots.get(&(ty, slot)) {
            return Ok(result.clone().map(Some));
        }
        let recipe = self.mapped_recipe(ty)?;
        let mapper = self.object_target_mapper(ty)?.1;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            Ok(match slot {
                Slot::Parameter => {
                    let symbol = sequence
                        .read(self.reference_symbol_of_declaration_completion(recipe.parameter))?;
                    Some(sequence.read(self.declared_reference_type_completion(symbol, consumer))?)
                }
                Slot::Constraint => {
                    let parameter =
                        sequence.read(self.mapped_parameter_completion(ty, consumer))?;
                    Some(
                        match sequence
                            .read(self.parameter_constraint_completion(parameter, consumer))?
                        {
                            Some(value) => value,
                            None => sequence.read(NativeCompletion::recovered(
                                self.model.native_error_type(),
                                super::super::invalid_at_location(
                                    self.inputs,
                                    consumer,
                                    crate::analysis::SemanticFailureReason::ErrorType,
                                    "native mapped key constraint consumes errorType",
                                )
                                .into(),
                            ))?,
                        },
                    )
                }
                Slot::Name => match &recipe.name {
                    Some(expression) => {
                        let value = sequence.read(self.check_type_completion(expression))?;
                        Some(match mapper {
                            Some(mapper) => sequence
                                .read(self.instantiate_type_completion(value, mapper, consumer))?,
                            None => value,
                        })
                    }
                    None => None,
                },
                Slot::Template => Some(if let Some(expression) = &recipe.template {
                    let value = sequence.read(self.check_type_completion(expression))?;
                    let value = sequence.read(self.add_optionality_completion(
                        value,
                        true,
                        recipe.optional > 0,
                        consumer,
                    ))?;
                    match mapper {
                        Some(mapper) => sequence
                            .read(self.instantiate_type_completion(value, mapper, consumer))?,
                        None => value,
                    }
                } else {
                    sequence.read(NativeCompletion::recovered(
                        self.model.native_error_type(),
                        super::super::invalid_at_location(
                            self.inputs,
                            consumer,
                            crate::analysis::SemanticFailureReason::ErrorType,
                            "native mapped value template consumes errorType",
                        )
                        .into(),
                    ))?
                }),
                Slot::Modifiers => {
                    let value = if let Some(operand) = &recipe.keyof_operand {
                        Some(sequence.read(self.check_type_completion(operand))?)
                    } else {
                        let root = self
                            .mapped
                            .declarations
                            .get(&ty)
                            .expect("established mapped declaration")
                            .0;
                        let original =
                            sequence.read(self.mapped_type_node_completion(root, consumer))?;
                        let constraint =
                            sequence.read(self.mapped_constraint_completion(original, consumer))?;
                        let constraint = if self
                            .model
                            .has_type_flags(constraint, TypeFlags::TYPE_PARAMETER)
                        {
                            sequence
                                .read(self.parameter_constraint_completion(constraint, consumer))?
                        } else {
                            Some(constraint)
                        };
                        match constraint.map(|ty| self.model.get(ty)).transpose()? {
                            Some(TypeView::Index(index)) => Some(index.target),
                            _ => None,
                        }
                    };
                    Some(match value {
                        Some(value) => match mapper {
                            Some(mapper) => sequence
                                .read(self.instantiate_type_completion(value, mapper, consumer))?,
                            None => value,
                        },
                        None => self.model.special(SpecialName::Unknown),
                    })
                }
                Slot::Apparent => Some(
                    sequence.read(self.compute_apparent_mapped_type_completion(ty, consumer))?,
                ),
            })
        })();
        let result = sequence.finish(result);
        if let Ok(value) = &result
            && let Some(ty_value) = *value.value()
        {
            self.mapped
                .slots
                .insert((ty, slot), value.clone().map(|_| ty_value));
        }
        result
    }

    fn required_mapped_slot_completion(
        &mut self,
        ty: TypeId,
        slot: Slot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.mapped_slot_completion(ty, slot, consumer)?
            .and_then_value(|value| {
                value.ok_or_else(|| super::super::internal("required mapped link is nil").into())
            })
    }

    pub(super) fn mapped_parameter(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.mapped_parameter_completion(ty, consumer)?.admit()
    }

    pub(super) fn mapped_parameter_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.required_mapped_slot_completion(ty, Slot::Parameter, consumer)
    }

    pub(super) fn mapped_constraint(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.mapped_constraint_completion(ty, consumer)?.admit()
    }

    pub(super) fn mapped_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.required_mapped_slot_completion(ty, Slot::Constraint, consumer)
    }

    pub(super) fn mapped_name(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.mapped_name_completion(ty, consumer)?.admit()
    }

    pub(super) fn mapped_name_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.mapped_slot_completion(ty, Slot::Name, consumer)
    }

    pub(super) fn mapped_template(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.mapped_template_completion(ty, consumer)?.admit()
    }

    pub(super) fn mapped_template_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.required_mapped_slot_completion(ty, Slot::Template, consumer)
    }

    pub(super) fn mapped_modifiers(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.mapped_modifiers_completion(ty, consumer)?.admit()
    }

    pub(super) fn mapped_modifiers_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.required_mapped_slot_completion(ty, Slot::Modifiers, consumer)
    }

    pub(super) fn apparent_mapped_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.required_mapped_slot_completion(ty, Slot::Apparent, consumer)
    }

    /// `getMappedTypeNameTypeKind` distinguishes syntactic absence from an
    /// identity/filtering `as` clause; only actual assignability selects it.
    pub(super) fn mapped_name_kind_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<NameKind> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<NameKind, DemandError> {
            let Some(name) = sequence.read(self.mapped_name_completion(ty, consumer))? else {
                return Ok(NameKind::None);
            };
            let parameter = sequence.read(self.mapped_parameter_completion(ty, consumer))?;
            Ok(
                if sequence.read(self.type_related_completion(
                    name,
                    parameter,
                    RelationKind::Assignable,
                    consumer,
                ))? {
                    NameKind::Filtering
                } else {
                    NameKind::Remapping
                },
            )
        })();
        sequence.finish(result)
    }

    /// `getHomomorphicTypeVariable` unwraps the actual variable behind keyof.
    pub(super) fn homomorphic_type_variable(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.homomorphic_type_variable_completion(ty, consumer)?
            .admit()
    }

    pub(super) fn homomorphic_type_variable_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let constraint = sequence.read(self.mapped_constraint_completion(ty, consumer))?;
            if let TypeView::Index(index) = self.model.get(constraint)? {
                let variable = self.actual_type_variable(index.target, consumer)?;
                if self
                    .model
                    .has_type_flags(variable, TypeFlags::TYPE_PARAMETER)
                {
                    return Ok(Some(variable));
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `appendTypeMapping` applies the existing Map operation, then the key
    /// substitution. Its merged mapper never instantiates the first result.
    pub(super) fn append_type_mapping(
        &mut self,
        mapper: Option<MapperId>,
        source: TypeId,
        target: TypeId,
    ) -> Result<MapperId, DemandError> {
        let local = self.model.mapper(vec![source], vec![target])?;
        Ok(match mapper {
            Some(mapper) => self.model.merge_mappers(mapper, local),
            None => local,
        })
    }
}

impl Evaluator<'_, '_> {
    /// `isArrayOrTupleOrIntersection` requires an actual intersection and
    /// recursively tests each constituent; an ordinary array is not sufficient.
    pub(super) fn is_array_or_tuple_or_intersection(
        &mut self,
        ty: TypeId,
    ) -> Result<bool, DemandError> {
        let Some(types) = self.model.intersection_constituents(ty) else {
            return Ok(false);
        };
        for &ty in types.iter() {
            if !self.is_array_or_tuple_type(ty)? && !self.is_array_or_tuple_or_intersection(ty)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `getResolvedApparentTypeOfMappedType` only materializes a homomorphic
    /// array/tuple constraint; other mapped objects retain their actual identity.
    fn compute_apparent_mapped_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let (target, mapper) = self.object_target_mapper(ty)?;
            if let Some(variable) =
                sequence.read(self.homomorphic_type_variable_completion(target, consumer))?
                && self.mapped_recipe(target)?.name.is_none()
            {
                let modifiers = sequence.read(self.mapped_modifiers_completion(ty, consumer))?;
                let base = if sequence
                    .read(self.is_generic_mapped_type_completion(modifiers, consumer))?
                {
                    Some(sequence.read(self.apparent_mapped_type_completion(modifiers, consumer))?)
                } else {
                    sequence.read(self.base_constraint_completion(modifiers, consumer))?
                };
                if let Some(base) = base {
                    let types = self
                        .model
                        .union_constituents(base)
                        .unwrap_or_else(|| vec![base].into());
                    let mut all = true;
                    for &ty in types.iter() {
                        if !self.is_array_or_tuple_type(ty)?
                            && !self.is_array_or_tuple_or_intersection(ty)?
                        {
                            all = false;
                            break;
                        }
                    }
                    if all {
                        let mapper = self.prepend_type_mapping(variable, base, mapper)?;
                        return sequence
                            .read(self.instantiate_type_completion(target, mapper, consumer));
                    }
                }
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    /// `instantiateMappedType` distributes only a changed homomorphic variable.
    /// A syntactically present `as` clause disables tuple/array specialization.
    pub(super) fn instantiate_mapped_type_completion(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        alias: Option<super::super::model::Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(variable) =
                sequence.read(self.homomorphic_type_variable_completion(ty, consumer))?
            {
                let mapped =
                    sequence.read(self.instantiate_type_completion(variable, mapper, consumer))?;
                if variable != mapped {
                    let mapped = sequence.read(self.reduced_type_completion(mapped, consumer))?;
                    return sequence.read(self.map_type_with_alias_completion(
                        mapped,
                        alias,
                        consumer,
                        |checker, constituent| {
                            checker.instantiate_mapped_constituent_completion(
                                constituent,
                                ty,
                                variable,
                                mapper,
                                consumer,
                            )
                        },
                    ));
                }
            }
            let constraint = sequence.read(self.mapped_constraint_completion(ty, consumer))?;
            if sequence.read(self.instantiate_type_completion(constraint, mapper, consumer))?
                == self.model.wildcard_type()
            {
                return Ok(self.model.wildcard_type());
            }
            sequence.read(self.instantiate_anonymous_type_completion(ty, mapper, alias, consumer))
        })();
        sequence.finish(result)
    }

    fn instantiate_mapped_constituent_completion(
        &mut self,
        source: TypeId,
        ty: TypeId,
        variable: TypeId,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self.model.has_type_flags(
                source,
                TypeFlags::ANY
                    | TypeFlags::UNKNOWN
                    | TypeFlags::INSTANTIABLE_NON_PRIMITIVE
                    | TypeFlags::OBJECT
                    | TypeFlags::INTERSECTION,
            ) || source == self.model.wildcard_type()
            {
                return Ok(source);
            }
            if self.mapped_recipe(ty)?.name.is_none() {
                let mut array = self.is_array_type(source)?;
                if !array
                    && self.model.has_type_flags(source, TypeFlags::ANY)
                    && self
                        .resolution_cycle_start(super::resolution::Key::BaseConstraint(variable))
                        .is_none()
                {
                    if let Some(constraint) =
                        sequence.read(self.parameter_constraint_completion(variable, consumer))?
                    {
                        array = true;
                        let types = self
                            .model
                            .union_constituents(constraint)
                            .unwrap_or_else(|| vec![constraint].into());
                        for &ty in types.iter() {
                            if !self.is_array_or_tuple_type(ty)? {
                                array = false;
                                break;
                            }
                        }
                    }
                }
                if array {
                    let mapper = self.prepend_type_mapping(variable, source, Some(mapper))?;
                    let key = self.model.primitive(crate::types::PrimitiveName::Number);
                    let element =
                        sequence.read(self.instantiate_mapped_template_completion(
                            ty, key, true, mapper, consumer,
                        ))?;
                    let readonly = self.is_readonly_array_type(source)?;
                    let readonly = modified(readonly, self.mapped_recipe(ty)?.readonly);
                    return if readonly {
                        self.readonly_array_type(element, consumer)
                    } else {
                        self.array_type(element, consumer)
                    };
                }
                if let Some(tuple) = self.model.tuple_target(source)? {
                    let fixed_mapper = if tuple.fixed_length != 0 {
                        self.prepend_type_mapping(variable, source, Some(mapper))?
                    } else {
                        mapper
                    };
                    let recipe = self.mapped_recipe(ty)?;
                    let elements = sequence.read(self.get_element_types_completion(source))?;
                    let mut infos = tuple.elements.to_vec();
                    let mut result = Vec::with_capacity(elements.len());
                    use super::super::model::TupleElementFlags as F;
                    for (i, &element) in elements.iter().enumerate() {
                        let flags = infos[i].flags;
                        let value = if i < tuple.fixed_length {
                            let key = self
                                .model
                                .literal(crate::types::PrimitiveName::String, i.to_string());
                            sequence.read(self.instantiate_mapped_template_completion(
                                ty,
                                key,
                                flags.intersects(F::OPTIONAL),
                                fixed_mapper,
                                consumer,
                            ))?
                        } else if flags.intersects(F::VARIADIC) {
                            let mapper =
                                self.prepend_type_mapping(variable, element, Some(mapper))?;
                            sequence.read(self.instantiate_type_completion(ty, mapper, consumer))?
                        } else {
                            let array = self.array_type(element, consumer)?;
                            let mapper =
                                self.prepend_type_mapping(variable, array, Some(mapper))?;
                            let array = sequence
                                .read(self.instantiate_type_completion(ty, mapper, consumer))?;
                            sequence
                                .read(self.array_element_type_completion(array))?
                                .unwrap_or_else(|| self.model.special(SpecialName::Unknown))
                        };
                        if recipe.optional > 0 && flags.intersects(F::REQUIRED) {
                            infos[i].flags = F::OPTIONAL;
                        } else if recipe.optional < 0 && flags.intersects(F::OPTIONAL) {
                            infos[i].flags = F::REQUIRED;
                        }
                        result.push(value);
                    }
                    return sequence.read(self.create_tuple_type_ex_completion(
                        result,
                        infos,
                        modified(tuple.readonly, recipe.readonly),
                        consumer,
                    ));
                }
                if self.is_array_or_tuple_or_intersection(source)? {
                    let types = self
                        .model
                        .intersection_constituents(source)
                        .expect("native intersection predicate");
                    let mut mapped = Vec::with_capacity(types.len());
                    for &source in types.iter() {
                        mapped.push(sequence.read(
                            self.instantiate_mapped_constituent_completion(
                                source, ty, variable, mapper, consumer,
                            ),
                        )?);
                    }
                    return sequence.read(self.intersection_type_completion(mapped, consumer));
                }
            }
            let mapper = self.prepend_type_mapping(variable, source, Some(mapper))?;
            sequence.read(self.instantiate_anonymous_type_completion(ty, mapper, None, consumer))
        })();
        sequence.finish(result)
    }

    /// `instantiateMappedTypeTemplate` applies the key after the enclosing
    /// mapper, then performs exactly the mapped optionality transformation.
    pub(super) fn instantiate_mapped_template_completion(
        &mut self,
        ty: TypeId,
        key: TypeId,
        optional: bool,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let parameter = sequence.read(self.mapped_parameter_completion(ty, consumer))?;
            let mapper = self.append_type_mapping(Some(mapper), parameter, key)?;
            let target = self.object_target_mapper(ty)?.0;
            let template = sequence.read(self.mapped_template_completion(target, consumer))?;
            let value =
                sequence.read(self.instantiate_type_completion(template, mapper, consumer))?;
            let recipe = self.mapped_recipe(ty)?;
            if self.inputs.semantic_options().strict_null_checks {
                if recipe.optional > 0
                    && !self
                        .model
                        .maybe_type_of_kind(value, TypeFlags::UNDEFINED | TypeFlags::VOID)
                {
                    return sequence.read(self.optional_type_completion(value, true, consumer));
                }
                if recipe.optional < 0 && optional {
                    return self.type_with_facts(
                        value,
                        super::facts::TypeFacts::typeof_test("undefined", false),
                        consumer,
                    );
                }
            }
            Ok(value)
        })();
        sequence.finish(result)
    }

    /// `getCombinedMappedTypeOptionality` consults modifier sources only when
    /// the current mapping preserves optionality. Intersections require unanimity.
    pub(super) fn combined_mapped_optionality(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<i8, DemandError> {
        self.combined_mapped_optionality_completion(ty, consumer)?
            .admit()
    }

    pub(super) fn combined_mapped_optionality_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<i8> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.object_flags(ty)?.intersects(ObjectFlags::MAPPED) {
                let optionality = self.mapped_recipe(ty)?.optional;
                if optionality != 0 {
                    return Ok(optionality);
                }
                let modifiers = sequence.read(self.mapped_modifiers_completion(ty, consumer))?;
                return sequence
                    .read(self.combined_mapped_optionality_completion(modifiers, consumer));
            }
            if let Some(types) = self.model.intersection_constituents(ty) {
                let optionality = sequence
                    .read(self.combined_mapped_optionality_completion(types[0], consumer))?;
                for &ty in &types[1..] {
                    if sequence.read(self.combined_mapped_optionality_completion(ty, consumer))?
                        != optionality
                    {
                        return Ok(0);
                    }
                }
                return Ok(optionality);
            }
            Ok(0)
        })();
        sequence.finish(result)
    }
}

fn modified(original: bool, modifier: i8) -> bool {
    match modifier {
        -1 => false,
        1 => true,
        _ => original,
    }
}

impl Evaluator<'_, '_> {
    /// `getLowerBoundOfKeyType` preserves the compiler's primitive & {} marker
    /// and uses no-reduction union mapping; these are not ordinary base constraints.
    pub(super) fn lower_bound_of_key_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            match self.model.get(ty)? {
                TypeView::Index(index) => {
                    let target =
                        sequence.read(self.apparent_type_completion(index.target, consumer))?;
                    if self.model.is_generic_tuple_type(target)? {
                        sequence.read(self.known_keys_of_tuple_type_completion(target, consumer))
                    } else {
                        sequence.read(self.index_type_ex_completion(
                            target,
                            super::super::model::IndexFlags::NONE,
                            consumer,
                        ))
                    }
                }
                TypeView::Conditional(view) => {
                    if self.conditional_is_distributive(ty)? {
                        let constraint = sequence.read(
                            self.lower_bound_of_key_type_completion(view.check_type, consumer),
                        )?;
                        if constraint != view.check_type {
                            let root_check = self.conditional_root_check(view.root);
                            let mapper =
                                self.prepend_type_mapping(root_check, constraint, view.mapper)?;
                            return sequence.read(self.conditional_instantiation_completion(
                                ty, mapper, false, None, consumer,
                            ));
                        }
                    }
                    Ok(ty)
                }
                _ if self.model.has_type_flags(ty, TypeFlags::UNION) => sequence.read(
                    self.map_type_no_reduction_completion(ty, consumer, |checker, ty| {
                        checker.lower_bound_of_key_type_completion(ty, consumer)
                    }),
                ),
                _ if self.model.has_type_flags(ty, TypeFlags::INTERSECTION) => {
                    let types = self
                        .model
                        .intersection_constituents(ty)
                        .expect("intersection view");
                    if types.len() == 2
                        && self.model.has_type_flags(
                            types[0],
                            TypeFlags::STRING | TypeFlags::NUMBER | TypeFlags::BIGINT,
                        )
                        && types[1] == self.model.empty_type_literal_type()
                    {
                        return Ok(ty);
                    }
                    let mut lower = Vec::with_capacity(types.len());
                    for &ty in types.iter() {
                        lower.push(
                            sequence.read(self.lower_bound_of_key_type_completion(ty, consumer))?,
                        );
                    }
                    // core.SameMap can preserve the slice, but getIntersectionType
                    // is still called on it by the native operation.
                    sequence.read(self.intersection_type_completion(lower, consumer))
                }
                _ => Ok(ty),
            }
        })();
        sequence.finish(result)
    }

    /// `forEachMappedTypePropertyKeyTypeAndIndexSignatureKeyType` acquires
    /// complete properties before index signatures and preserves iteration order.
    fn for_each_mapped_key_completion(
        &mut self,
        ty: TypeId,
        strings_only: bool,
        consumer: &Location,
        mut visit: impl FnMut(&mut Self, TypeId) -> NativeResult<()>,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let include =
                TypeFlags::STRING_LITERAL | TypeFlags::NUMBER_LITERAL | TypeFlags::UNIQUE_SYMBOL;
            for property in sequence.read(self.properties_of_type_completion(ty, consumer))? {
                let symbol = property.symbol.ok_or_else(|| {
                    super::super::internal("mapped key property lacks actual symbol")
                })?;
                let key = sequence.read(
                    self.literal_type_from_property_completion(symbol, include, false, consumer),
                )?;
                sequence.read(visit(self, key))?;
            }
            if self.model.has_type_flags(ty, TypeFlags::ANY) {
                let string = self.model.primitive(crate::types::PrimitiveName::String);
                sequence.read(visit(self, string))?;
            } else {
                for index in sequence.read(self.index_infos_of_type_completion(ty, consumer))? {
                    let key = sequence.read(self.demand_completion(index.key_type))?;
                    if !strings_only
                        || self
                            .model
                            .has_type_flags(key, TypeFlags::STRING | TypeFlags::TEMPLATE_LITERAL)
                    {
                        sequence.read(visit(self, key))?;
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `getIndexTypeForMappedType` maps key types without manufacturing value
    /// properties. Unchanged union constituents preserve the constraint's alias.
    pub(super) fn mapped_index_type_completion(
        &mut self,
        ty: TypeId,
        flags: super::super::model::IndexFlags,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            use super::super::model::IndexFlags;
            let parameter = sequence.read(self.mapped_parameter_completion(ty, consumer))?;
            let constraint = sequence.read(self.mapped_constraint_completion(ty, consumer))?;
            let (target, mapper) = self.object_target_mapper(ty)?;
            let name = sequence.read(self.mapped_name_completion(target, consumer))?;
            if name.is_none() && !flags.contains(IndexFlags::NO_INDEX_SIGNATURES) {
                return Ok(constraint);
            }
            let recipe = self.mapped_recipe(ty)?;
            let mut keys = Vec::new();
            let add = |checker: &mut Self, key| {
                let mut sequence = NativeSequence::new();
                let result = (|| {
                    let name = match name {
                        Some(name) => {
                            let mapper = checker.append_type_mapping(mapper, parameter, key)?;
                            sequence
                                .read(checker.instantiate_type_completion(name, mapper, consumer))?
                        }
                        None => key,
                    };
                    let string = checker.model.primitive(crate::types::PrimitiveName::String);
                    if name == string {
                        keys.push(checker.model.string_or_number_type());
                    } else {
                        keys.push(name);
                    }
                    Ok(())
                })();
                sequence.finish(result)
            };
            if sequence.read(self.is_generic_index_type_completion(constraint, consumer))? {
                if recipe.keyof_operand.is_some() {
                    return Ok(self.model.index_type_for_generic(ty, flags));
                }
                sequence.read(self.for_each_mapped_constituent_completion(constraint, add))?;
            } else if recipe.keyof_operand.is_some() {
                let modifiers = sequence.read(self.mapped_modifiers_completion(ty, consumer))?;
                let modifiers =
                    sequence.read(self.apparent_type_completion(modifiers, consumer))?;
                sequence.read(self.for_each_mapped_key_completion(
                    modifiers,
                    flags.contains(IndexFlags::STRINGS_ONLY),
                    consumer,
                    add,
                ))?;
            } else {
                let lower =
                    sequence.read(self.lower_bound_of_key_type_completion(constraint, consumer))?;
                sequence.read(self.for_each_mapped_constituent_completion(lower, add))?;
            }
            let mut result =
                sequence.read(self.union_type_ex_completion(keys, None, None, consumer))?;
            if flags.contains(IndexFlags::NO_INDEX_SIGNATURES) {
                result = self.model.filter_type(result, |model, ty| {
                    !model.has_type_flags(ty, TypeFlags::ANY | TypeFlags::STRING)
                });
            }
            if self.model.union_constituents(result).is_some()
                && self.model.union_constituents(result)
                    == self.model.union_constituents(constraint)
            {
                return Ok(constraint);
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    fn for_each_mapped_constituent_completion(
        &mut self,
        ty: TypeId,
        mut visit: impl FnMut(&mut Self, TypeId) -> NativeResult<()>,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let types = self
                .model
                .union_constituents(ty)
                .unwrap_or_else(|| vec![ty].into());
            for &ty in types.iter() {
                sequence.read(visit(self, ty))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }
}

impl Evaluator<'_, '_> {
    /// `isMappedTypeGenericIndexedAccess` is narrower than a mapped receiver.
    pub(super) fn is_mapped_generic_indexed_access_completion(
        &mut self,
        object: TypeId,
        index: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            Ok(self
                .model
                .object_flags(object)?
                .intersects(ObjectFlags::MAPPED)
                && !sequence.read(self.is_generic_mapped_type_completion(object, consumer))?
                && sequence.read(self.is_generic_index_type_completion(index, consumer))?
                && self.mapped_recipe(object)?.optional >= 0
                && self.mapped_recipe(object)?.name.is_none())
        })();
        sequence.finish(result)
    }

    /// `substituteIndexedMappedType` substitutes the key into the original
    /// template before acquiring optionality from the actual modifier source.
    pub(super) fn substitute_indexed_mapped_type_completion(
        &mut self,
        object: TypeId,
        index: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let parameter = sequence.read(self.mapped_parameter_completion(object, consumer))?;
            let (target, mapper) = self.object_target_mapper(object)?;
            // `substituteIndexedMappedType` explicitly composes these mappers:
            // unlike appendTypeMapping, it instantiates the first mapped result.
            let local = self.model.mapper(vec![parameter], vec![index])?;
            let mapper = match mapper {
                Some(mapper) => self.model.combine_mappers(mapper, local),
                None => local,
            };
            let template = sequence.read(self.mapped_template_completion(target, consumer))?;
            let result =
                sequence.read(self.instantiate_type_completion(template, mapper, consumer))?;
            let mut optional = self.mapped_recipe(object)?.optional > 0;
            if !optional {
                if sequence.read(self.is_generic_type_completion(object, consumer))? {
                    let modifiers =
                        sequence.read(self.mapped_modifiers_completion(object, consumer))?;
                    optional = sequence
                        .read(self.combined_mapped_optionality_completion(modifiers, consumer))?
                        > 0;
                } else if let Some(constraint) =
                    sequence.read(self.base_constraint_completion(index, consumer))?
                {
                    for property in
                        sequence.read(self.properties_of_type_completion(object, consumer))?
                    {
                        let symbol = property.symbol.ok_or_else(|| {
                            super::super::internal("mapped optional property lacks native symbol")
                        })?;
                        if self.property_symbol_flags(symbol)?.optional {
                            let key = sequence.read(self.literal_type_from_property_completion(
                                symbol,
                                TypeFlags::STRING_LITERAL
                                    | TypeFlags::NUMBER_LITERAL
                                    | TypeFlags::UNIQUE_SYMBOL,
                                false,
                                consumer,
                            ))?;
                            if sequence.read(self.type_related_completion(
                                key,
                                constraint,
                                RelationKind::Assignable,
                                consumer,
                            ))? {
                                optional = true;
                                break;
                            }
                        }
                    }
                }
            }
            sequence.read(self.add_optionality_completion(result, true, optional, consumer))
        })();
        sequence.finish(result)
    }
}

impl Evaluator<'_, '_> {
    /// Native direct MappedType.nameType observation does not initialize the
    /// lazy name slot. A cached failed acquisition remains a consumed failure.
    pub(super) fn mapped_established_name(
        &self,
        ty: TypeId,
    ) -> Result<Option<TypeId>, DemandError> {
        self.mapped_established_name_completion(ty)?.admit()
    }

    pub(super) fn mapped_established_name_completion(
        &self,
        ty: TypeId,
    ) -> NativeResult<Option<TypeId>> {
        Ok(match self.mapped.slots.get(&(ty, Slot::Name)) {
            Some(value) => value.clone().map(Some),
            None => NativeCompletion::clean(None),
        })
    }

    /// `getIndexedMappedTypeSubstitutedTypeOfContextualType` distinguishes an
    /// excluded key from failure and observes only an already established name.
    pub(super) fn mapped_contextual_property_completion(
        &mut self,
        ty: TypeId,
        name: &crate::project::binding::NativePropertyKey,
        name_type: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let key = match name_type {
                Some(ty) => ty,
                None => self.model.string_literal_for_key(name),
            };
            let constraint = sequence.read(self.mapped_constraint_completion(ty, consumer))?;
            if let Some(name) = sequence.read(self.mapped_established_name_completion(ty))?
                && sequence
                    .read(self.is_excluded_mapped_property_name_completion(name, key, consumer))?
            {
                return Ok(None);
            }
            if sequence
                .read(self.is_excluded_mapped_property_name_completion(constraint, key, consumer))?
            {
                return Ok(None);
            }
            let constraint = sequence
                .read(self.base_constraint_completion(constraint, consumer))?
                .unwrap_or(constraint);
            if !sequence.read(self.type_related_completion(
                key,
                constraint,
                RelationKind::Assignable,
                consumer,
            ))? {
                return Ok(None);
            }
            sequence
                .read(self.substitute_indexed_mapped_type_completion(ty, key, consumer))
                .map(Some)
        })();
        sequence.finish(result)
    }

    /// `isExcludedMappedPropertyName` short-circuits each actual conditional
    /// and intersection test while retaining completed recovery dependencies.
    fn is_excluded_mapped_property_name_completion(
        &mut self,
        ty: TypeId,
        key: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let TypeView::Conditional(view) = self.model.get(ty)? {
                let true_type =
                    sequence.read(self.conditional_true_type_completion(ty, consumer))?;
                let true_type = sequence.read(self.reduced_type_completion(true_type, consumer))?;
                if !self.model.has_type_flags(true_type, TypeFlags::NEVER) {
                    return Ok(false);
                }
                let false_type =
                    sequence.read(self.conditional_false_type_completion(ty, consumer))?;
                let false_type =
                    sequence.read(self.actual_type_variable_completion(false_type, consumer))?;
                if false_type
                    != sequence
                        .read(self.actual_type_variable_completion(view.check_type, consumer))?
                {
                    return Ok(false);
                }
                return sequence.read(self.type_related_completion(
                    key,
                    view.extends_type,
                    RelationKind::Assignable,
                    consumer,
                ));
            }
            if let Some(types) = self.model.intersection_constituents(ty) {
                for &ty in types.iter() {
                    if sequence
                        .read(self.is_excluded_mapped_property_name_completion(ty, key, consumer))?
                    {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }
}

struct MappedMembers {
    ty: TypeId,
    parameter: TypeId,
    mapper: Option<MapperId>,
    name: Option<TypeId>,
    template: TypeId,
    modifiers: TypeId,
    recipe: Arc<MappedRecipe>,
    link_declarations: bool,
    properties: Vec<(
        crate::project::binding::NativePropertyKey,
        super::super::model::PropertySymbolId,
    )>,
    indexes: Vec<super::super::model::Index>,
}

impl Evaluator<'_, '_> {
    pub(super) fn mark_mapped_contains_error(&mut self, ty: TypeId) {
        self.mapped.contains_error.insert(ty);
    }

    /// `resolveMappedTypeMembers` publishes nil members before any key/value
    /// acquisition, then installs one complete native raw symbol table.
    pub(super) fn mapped_members_completion(
        &mut self,
        ty: TypeId,
    ) -> NativeResult<super::super::model::MembersView> {
        let mut table = None;
        let consumer = self.anonymous_consumer(ty)?;
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<super::super::model::MembersView, DemandError> {
            use super::super::model::MembersView;
            let consumer = self.anonymous_consumer(ty)?;
            let empty = MembersView {
                properties: Arc::from([]),
                call_signatures: Arc::from([]),
                construct_signatures: Arc::from([]),
                index_signatures: Arc::from([]),
            };
            sequence.read(self.set_structured_member_table_completion(
                ty,
                None,
                NativeCompletion::clean(empty.clone()),
                &consumer,
            ))?;
            let parameter = sequence.read(self.mapped_parameter_completion(ty, &consumer))?;
            let constraint = sequence.read(self.mapped_constraint_completion(ty, &consumer))?;
            let (target, mapper) = self.object_target_mapper(ty)?;
            let name = sequence.read(self.mapped_name_completion(target, &consumer))?;
            let link_declarations = sequence
                .read(self.mapped_name_kind_completion(target, &consumer))?
                != NameKind::Remapping;
            let template = sequence.read(self.mapped_template_completion(target, &consumer))?;
            let modifiers = sequence.read(self.mapped_modifiers_completion(ty, &consumer))?;
            let modifiers = sequence.read(self.apparent_type_completion(modifiers, &consumer))?;
            let recipe = self.mapped_recipe(ty)?;
            let mut result = MappedMembers {
                ty,
                parameter,
                mapper,
                name,
                template,
                modifiers,
                recipe: recipe.clone(),
                link_declarations,
                properties: Vec::new(),
                indexes: Vec::new(),
            };
            if recipe.keyof_operand.is_some() {
                sequence.read(self.for_each_mapped_key_completion(
                    modifiers,
                    false,
                    &consumer,
                    |checker, key| checker.add_mapped_key_completion(&mut result, key, &consumer),
                ))?;
            } else {
                let lower = sequence
                    .read(self.lower_bound_of_key_type_completion(constraint, &consumer))?;
                sequence.read(
                    self.for_each_mapped_constituent_completion(lower, |checker, key| {
                        checker.add_mapped_key_completion(&mut result, key, &consumer)
                    }),
                )?;
            }
            let inventory = MembersView {
                index_signatures: result.indexes.into(),
                ..empty
            };
            table = Some(super::structure::MemberTable::Symbols(
                result.properties.into(),
            ));
            Ok(inventory)
        })();
        let members = sequence.finish(result)?;
        self.set_structured_member_table_completion(ty, table, members, &consumer)
    }

    fn add_mapped_key_completion(
        &mut self,
        result: &mut MappedMembers,
        key: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let name = match result.name {
                Some(name) => {
                    let mapper = self.append_type_mapping(result.mapper, result.parameter, key)?;
                    sequence.read(self.instantiate_type_completion(name, mapper, consumer))?
                }
                None => key,
            };
            sequence.read(
                self.for_each_mapped_constituent_completion(name, |checker, name| {
                    checker.add_mapped_name_completion(result, key, name, consumer)
                }),
            )
        })();
        sequence.finish(result)
    }

    fn add_mapped_name_completion(
        &mut self,
        result: &mut MappedMembers,
        key: TypeId,
        name_type: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            if let Some(name) = self.property_key_from_type(name_type)? {
                if let Some((_, existing)) = result.properties.iter().find(|(key, _)| key == &name)
                {
                    return sequence.read(
                        self.merge_mapped_symbol_key_completion(
                            *existing, name_type, key, consumer,
                        ),
                    );
                }
                let modifier = match self.property_key_from_type(key)? {
                    Some(name) => self
                        .property_of_type_by_key(result.modifiers, &name, consumer)?
                        .map(|property| self.property_symbol(&property, consumer))
                        .transpose()?,
                    None => None,
                };
                let modifier_optional = modifier
                    .map(|symbol| {
                        self.property_symbol_flags(symbol)
                            .map(|flags| flags.optional)
                    })
                    .transpose()?
                    .unwrap_or(false);
                let optional =
                    result.recipe.optional > 0 || result.recipe.optional == 0 && modifier_optional;
                let readonly = result.recipe.readonly > 0
                    || result.recipe.readonly == 0
                        && match modifier {
                            Some(symbol) => self.property_is_readonly(symbol, consumer)?,
                            None => false,
                        };
                let strip_optional = self.inputs.semantic_options().strict_null_checks
                    && !optional
                    && modifier_optional;
                let symbol = self.create_mapped_symbol(
                    name.clone(),
                    name_type,
                    key,
                    result.ty,
                    optional,
                    readonly,
                    strip_optional,
                    modifier,
                    result.link_declarations,
                    consumer,
                )?;
                result.properties.push((name, symbol));
            } else if sequence.read(self.is_valid_index_key_type_completion(name_type, consumer))?
                || self
                    .model
                    .has_type_flags(name_type, TypeFlags::ANY | TypeFlags::ENUM)
            {
                let index_key = if self
                    .model
                    .has_type_flags(name_type, TypeFlags::ANY | TypeFlags::STRING)
                {
                    self.model.primitive(crate::types::PrimitiveName::String)
                } else if self
                    .model
                    .has_type_flags(name_type, TypeFlags::NUMBER | TypeFlags::ENUM)
                {
                    self.model.primitive(crate::types::PrimitiveName::Number)
                } else {
                    name_type
                };
                let mapper = self.append_type_mapping(result.mapper, result.parameter, key)?;
                let value = sequence.read(self.instantiate_type_completion(
                    result.template,
                    mapper,
                    consumer,
                ))?;
                let modifiers = sequence.read(self.applicable_index_info_completion(
                    result.modifiers,
                    name_type,
                    consumer,
                ))?;
                let readonly = result.recipe.readonly > 0
                    || result.recipe.readonly == 0
                        && modifiers.as_ref().is_some_and(|index| index.readonly);
                let index = self
                    .model
                    .index_info(index_key, value, readonly, None, None);
                sequence.read(self.append_index_info_completion(
                    &mut result.indexes,
                    index,
                    true,
                    consumer,
                ))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }
}

impl Evaluator<'_, '_> {
    /// `createTypeNodeFromObjectType` selects mapped syntax before resolving
    /// members. Its binder-renaming/modifier-wrapper serializer is a distinct
    /// pending operation; generic mappings cannot masquerade as object literals.
    pub(in crate::project::semantic) fn object_display_members(
        &mut self,
        members: super::super::model::MembersOfType,
        consumer: &Location,
    ) -> Result<super::super::model::MembersView, DemandError> {
        let ty = members.type_id();
        self.check_object_display_kind(ty, consumer)?;
        self.members_at(members, consumer)
    }
    pub(super) fn check_object_display_kind(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        if self.mapped_uses_syntax(ty, consumer)? {
            return Err(super::super::internal(
                "mapped syntax requested through object member display",
            )
            .into());
        }
        Ok(())
    }
    /// `createTypeNodeFromObjectType` chooses symbolic mapped syntax before
    /// resolving an inventory, including a concrete map marked containsError.
    pub(in crate::project::semantic) fn mapped_uses_syntax(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        Ok(self.is_generic_mapped_type(ty, consumer)?
            || self.model.object_flags(ty)?.intersects(ObjectFlags::MAPPED)
                && self.mapped.contains_error.contains(&ty))
    }
}

pub(in crate::project::semantic) fn mapped_readonly_text(
    token: Option<crate::syntax_kind::SyntaxKind>,
) -> &'static str {
    use crate::syntax_kind::SyntaxKind as K;
    match token {
        Some(K::PlusToken) => "+readonly ",
        Some(K::MinusToken) => "-readonly ",
        Some(_) => "readonly ",
        None => "",
    }
}
pub(in crate::project::semantic) fn mapped_optional_text(
    token: Option<crate::syntax_kind::SyntaxKind>,
) -> &'static str {
    use crate::syntax_kind::SyntaxKind as K;
    match token {
        Some(K::PlusToken) => "+?",
        Some(K::MinusToken) => "-?",
        Some(_) => "?",
        None => "",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) enum MappedOperand {
    Constraint,
    Modifiers,
    Name,
    Template,
    DisplayTemplate,
}
// Five fixed lazy operand questions, independent of any expression or type graph.

pub(in crate::project::semantic) struct MappedHeader {
    pub parameter: TypeId,
    pub name: String,
    pub readonly: &'static str,
    pub optional: &'static str,
    pub keyof: bool,
    pub has_name: bool,
}
impl Evaluator<'_, '_> {
    /// Nonforcing mapped output header, after the caller's mandatory native
    /// genericity question. Template/name operand demands remain independent.
    pub(in crate::project::semantic) fn mapped_header(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<MappedHeader, DemandError> {
        let recipe = self.mapped_recipe(ty)?;
        let parameter = self.mapped_parameter(ty, consumer)?;
        let TypeView::TypeParameter(info) = self.model.get(parameter)? else {
            return Err(super::super::internal("mapped header has no type parameter").into());
        };
        Ok(MappedHeader {
            parameter,
            name: info.copy_display_name(),
            readonly: mapped_readonly_text(recipe.readonly_token),
            optional: mapped_optional_text(recipe.question_token),
            keyof: recipe.keyof_operand.is_some(),
            has_name: recipe.name.is_some(),
        })
    }

    pub(in crate::project::semantic) fn mapped_operand_demand(
        &mut self,
        ty: TypeId,
        operand: MappedOperand,
        consumer: &Location,
    ) -> Result<super::super::model::TypeDemand, DemandError> {
        if let Some(&demand) = self.mapped.demands.get(&(ty, operand)) {
            return Ok(demand);
        }
        let demand = self.register_generic_demand(super::generics::GenericDemand::MappedOperand {
            ty,
            operand,
            location: consumer.clone(),
        });
        self.mapped.demands.insert((ty, operand), demand);
        Ok(demand)
    }
    pub(super) fn mapped_operand(
        &mut self,
        ty: TypeId,
        operand: MappedOperand,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        match operand {
            MappedOperand::Constraint => self.mapped_constraint(ty, consumer),
            MappedOperand::Modifiers => self.mapped_modifiers(ty, consumer),
            MappedOperand::Name => self
                .mapped_name(ty, consumer)?
                .ok_or_else(|| super::super::internal("mapped name operand lacks syntax").into()),
            MappedOperand::Template => self.mapped_template(ty, consumer),
            MappedOperand::DisplayTemplate => {
                let template = self.mapped_template(ty, consumer)?;
                let optional = self.mapped_recipe(ty)?.optional > 0;
                self.model
                    .remove_missing_type(template, optional)
                    .map_err(|error| error.at(self.inputs, consumer))
            }
        }
    }
    pub(in crate::project::semantic) fn mapped_graph_parts(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<(Vec<super::super::model::TypeDemand>, Vec<TypeId>), DemandError> {
        let header = self.mapped_header(ty, consumer)?;
        let mut operands = Vec::new();
        if header.has_name {
            operands.push(self.mapped_operand_demand(ty, MappedOperand::Name, consumer)?);
        }
        operands.push(self.mapped_operand_demand(ty, MappedOperand::Template, consumer)?);
        Ok((operands, vec![header.parameter]))
    }
}

impl MappedState {
    /// Request-local backing for the existing native cache questions. Their
    /// identities, child values and provisional publication stay operation-owned.
    pub(super) fn new() -> Self {
        Self {
            nodes: crate::resources::FixedMap::new(),
            declarations: crate::resources::FixedMap::new(),
            slots: crate::resources::FixedMap::new(),
            contains_error: Default::default(),
            demands: crate::resources::FixedMap::new(),
        }
    }
}
