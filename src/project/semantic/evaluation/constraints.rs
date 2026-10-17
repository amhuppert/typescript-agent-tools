//! Raw parameter constraints and flattened bases are separate native questions.
//! One owner retains their cache, resolution history and absence provenance.

use super::super::model::{
    DemandError, MapperId, NativeCompletion, NativeResult, NativeSequence, TypeDemand, TypeFlags,
    TypeId, TypeView,
};
use super::{Evaluator, resolution};
use crate::analysis::SemanticFailureReason;
use crate::references::Location;
use crate::resources::FixedMap;

use super::property_symbols::RecursionIdentity;

#[derive(Clone, Copy)]
struct NativeSafetyStop {
    depth: usize,
    repeated_identity: bool,
}

/// Native absence and the native safety approximation share the compiler's
/// noConstraintType identity, but not their provenance. Product resource
/// failures never enter this sentinel cache.
#[derive(Clone, Copy, Default)]
struct NoConstraint {
    safety_stop: Option<NativeSafetyStop>,
}

#[derive(Clone)]
enum BaseConstraint {
    Type(TypeId),
    None(NoConstraint),
    Circular(DemandError),
}

enum BaseState {
    Installed(NativeCompletion<BaseConstraint>),
    Failed(DemandError),
}

struct ParameterAssociation {
    symbol: super::ReferenceSymbol,
    raw_constraint: Option<RawConstraint>,
    location: Location,
}

/// The clone's native target question can complete with noConstraintType.
/// Keeping that optional value here avoids forcing it through TypeDemand's
/// nonoptional TypeId contract or introducing another constraint cache.
#[derive(Clone, Copy)]
enum RawConstraint {
    Annotation(TypeDemand),
    Target { target: TypeId, mapper: MapperId },
}

/// `getDeclaredTypeOfTypeParameter`, `getConstraintFromTypeParameter`,
/// `getResolvedBaseConstraint` and `getInferTypeParameters` keep distinct
/// questions. Only their fixed-identity table backing is shared here.
pub(super) struct ConstraintsState {
    parameters: FixedMap<TypeId, ParameterAssociation>,
    bases: FixedMap<TypeId, BaseState>,
    raw: FixedMap<TypeId, NativeResult<Option<TypeId>>>,
    inferred: FixedMap<TypeId, ()>,
    conditional_depth: usize,
}

impl ConstraintsState {
    pub(super) fn new() -> Self {
        Self {
            parameters: FixedMap::new(),
            bases: FixedMap::new(),
            raw: FixedMap::new(),
            inferred: FixedMap::new(),
            conditional_depth: 0,
        }
    }
    pub(super) fn register_inferred(&mut self, ty: TypeId) {
        self.inferred.insert(ty, ());
    }
    pub(super) fn register_parameter(
        &mut self,
        ty: TypeId,
        symbol: impl Into<super::ReferenceSymbol>,
        raw_constraint: Option<TypeDemand>,
        location: Location,
    ) {
        self.parameters.insert(
            ty,
            ParameterAssociation {
                symbol: symbol.into(),
                raw_constraint: raw_constraint.map(RawConstraint::Annotation),
                location,
            },
        );
    }

    /// `typeResolutionHasProperty(ResolvedBaseConstraint)` asks for a native
    /// installed property, including NoConstraint/CircularConstraint. A
    /// product failure before installation is not such a property.
    pub(super) fn has_property(&self, ty: TypeId) -> bool {
        matches!(self.bases.get(&ty), Some(BaseState::Installed(_)))
    }
}

impl Evaluator<'_, '_> {
    /// `maybeTypeOfKindConsideringBaseConstraint` asks the original flags
    /// before acquiring a constraint, preserving that lazy demand boundary.
    pub(super) fn maybe_type_of_kind_considering_base_constraint_completion(
        &mut self,
        ty: TypeId,
        kind: TypeFlags,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if self.model.maybe_type_of_kind(ty, kind) {
            return Ok(NativeCompletion::clean(true));
        }
        Ok(self
            .base_constraint_or_type_completion(ty, consumer)?
            .map(|base| self.model.maybe_type_of_kind(base, kind)))
    }
    pub(super) fn base_constraint_or_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.base_constraint_or_type_completion(ty, consumer)?
            .admit()
    }

    /// `getBaseConstraintOrType` preserves native absence separately from a
    /// consumed failure while supplying the original identity for no constraint.
    pub(super) fn base_constraint_or_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        Ok(self
            .base_constraint_completion(ty, consumer)?
            .map(|base| base.unwrap_or(ty)))
    }
    /// `reportErrorResults`'s fresh clone has a real nil constraint while its
    /// replacement is instantiated. Installation updates the shared raw slot;
    /// an aborted substitution poisons the escaped identity with its cause.
    pub(super) fn clone_with_reporting_constraint(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        let clone = self.model.clone_parameter(source)?;
        self.model.complete_parameter_clone(clone, None, None)?;
        self.register_parameter_clone(clone, source, None)?;
        let mapper = self.model.mapper(vec![source], vec![clone])?;
        match self.instantiate_type(target, mapper, consumer) {
            Ok(constraint) => {
                self.model
                    .install_parameter_clone_constraint(clone, constraint)?;
                self.constraints
                    .raw
                    .insert(clone, Ok(NativeCompletion::clean(Some(constraint))));
                Ok(clone)
            }
            Err(error) => {
                self.model.fail_parameter_clone(clone, error.clone())?;
                self.constraints.raw.insert(clone, Err(error.clone()));
                Err(error)
            }
        }
    }

    pub(super) fn has_non_circular_base_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.resolved_base_constraint(ty, &mut Vec::new(), consumer)?
            .and_then(|base| match base {
                BaseConstraint::Circular(error) => NativeCompletion::recovered(false, error),
                _ => Ok(NativeCompletion::clean(true)),
            })
    }
    /// `getConstraintFromTypeParameter(target, mapper)` preserves the original
    /// canonical symbol/constraint provenance on fresh native local clones.
    /// A mapped clone retains its target question even when the target has no
    /// written constraint. A reporting clone with no mapper starts with the
    /// actual empty raw slot, independently of its model target association.
    pub(super) fn register_parameter_clone(
        &mut self,
        clone: TypeId,
        target: TypeId,
        mapper: Option<MapperId>,
    ) -> Result<(), DemandError> {
        let original = self.constraints.parameters.get(&target).ok_or_else(|| {
            super::super::internal("cloned parameter has no canonical constraint origin")
        })?;
        let symbol = original.symbol;
        let location = original.location.clone();
        self.constraints.parameters.insert(
            clone,
            ParameterAssociation {
                symbol,
                raw_constraint: mapper.map(|mapper| RawConstraint::Target { target, mapper }),
                location,
            },
        );
        Ok(())
    }

    /// `getConstraintOfTypeParameter` first resolves the base's circular
    /// guard, then returns the original symbolic raw constraint. Flattened
    /// base identities must not replace that declared graph edge.
    pub(in crate::project::semantic) fn parameter_constraint(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.parameter_constraint_completion(ty, consumer)?.admit()
    }

    pub(in crate::project::semantic) fn parameter_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.has_non_circular_base_constraint_completion(ty, consumer)?
            .and_then(|noncircular| {
                if noncircular {
                    self.raw_constraint_completion(ty)
                } else {
                    Ok(NativeCompletion::clean(None))
                }
            })
    }

    /// `getConstraintOfType` keeps raw parameter, indexed and conditional
    /// constraints distinct from the recursively flattened base question.
    pub(super) fn constraint_of_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.constraint_of_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn constraint_of_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        match self.model.get(ty)? {
            TypeView::TypeParameter(_) => self.parameter_constraint_completion(ty, consumer),
            TypeView::IndexedAccess(access) => self
                .has_non_circular_base_constraint_completion(ty, consumer)?
                .and_then(|noncircular| {
                    if noncircular {
                        self.constraint_from_indexed_access_completion(access, consumer)
                    } else {
                        Ok(NativeCompletion::clean(None))
                    }
                }),
            TypeView::Conditional(_) => self
                .has_non_circular_base_constraint_completion(ty, consumer)?
                .and_then(|noncircular| {
                    if noncircular {
                        self.conditional_constraint_completion(ty, consumer)
                            .map(|value| value.map(Some))
                    } else {
                        Ok(NativeCompletion::clean(None))
                    }
                }),
            _ => self.base_constraint_completion(ty, consumer),
        }
    }

    /// `getBaseConstraintOfType` acquires bases only for native constrained
    /// flags. An ordinary structural target is its own internal resolved base
    /// but has no public base constraint. Its members are not demanded here.
    pub(super) fn base_constraint(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.base_constraint_completion(ty, consumer)?.admit()
    }

    pub(super) fn base_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        if !self.model.has_type_flags(
            ty,
            TypeFlags::INSTANTIABLE_NON_PRIMITIVE
                | TypeFlags::UNION
                | TypeFlags::INTERSECTION
                | TypeFlags::INDEX
                | TypeFlags::TEMPLATE_LITERAL
                | TypeFlags::STRING_MAPPING,
        ) && !self.model.is_generic_tuple_type(ty)?
        {
            return Ok(NativeCompletion::clean(None));
        }
        self.resolved_base_constraint(ty, &mut Vec::new(), consumer)?
            .and_then(BaseConstraint::consumed_completion)
    }

    /// `getConstraintFromTypeParameter` uses its distinct raw question cache.
    /// Synthetic this returns the actual target, without target member work.
    pub(super) fn raw_constraint_completion(&mut self, ty: TypeId) -> NativeResult<Option<TypeId>> {
        if let Some(result) = self.constraints.raw.get(&ty) {
            return result.clone();
        }
        let TypeView::TypeParameter(parameter) = self.model.get(ty)? else {
            return Err(super::super::internal("raw constraint has no parameter identity").into());
        };
        if parameter.is_this || self.model.is_synthetic_parameter(ty) {
            return match parameter.constraint {
                Some(demand) => Ok(self.demand_completion(demand)?.map(Some)),
                None => Ok(NativeCompletion::clean(None)),
            };
        }
        let association = self.constraints.parameters.get(&ty).ok_or_else(|| {
            super::super::internal("parameter has no canonical raw-constraint association")
        })?;
        let recipe = association.raw_constraint;
        let symbol = association.symbol;
        let location = association.location.clone();
        let result = if self.constraints.inferred.contains_key(&ty) {
            self.inferred_parameter_constraint_completion(ty, symbol, false, &location)
        } else {
            match recipe {
                Some(RawConstraint::Annotation(demand)) => {
                    self.demand_completion(demand).map(|value| value.map(Some))
                }
                Some(RawConstraint::Target { target, mapper }) => self
                    .parameter_constraint_completion(target, &location)
                    .and_then(|constraint| {
                        constraint.and_then(|constraint| match constraint {
                            Some(constraint) => self
                                .instantiate_type_completion(constraint, mapper, &location)
                                .map(|value| value.map(Some)),
                            None => Ok(NativeCompletion::clean(None)),
                        })
                    }),
                None => Ok(NativeCompletion::clean(None)),
            }
        };
        self.constraints.raw.insert(ty, result.clone());
        result
    }

    /// `getRestrictiveTypeParameter` observes the installed absence sentinel
    /// before the actual declaration constraint. It does not infer a constraint.
    pub(super) fn parameter_has_no_declared_constraint(
        &self,
        ty: TypeId,
    ) -> Result<bool, DemandError> {
        if let Some(result) = self.constraints.raw.get(&ty) {
            return match result {
                Ok(constraint) => Ok(constraint.value().is_none()),
                Err(error) => Err(error.clone()),
            };
        }
        let TypeView::TypeParameter(parameter) = self.model.get(ty)? else {
            return Ok(false);
        };
        Ok(parameter.constraint.is_none())
    }

    pub(super) fn register_restrictive_parameter(
        &mut self,
        clone: TypeId,
        _target: TypeId,
    ) -> Result<(), DemandError> {
        self.constraints
            .raw
            .insert(clone, Ok(NativeCompletion::clean(None)));
        Ok(())
    }

    /// Graph admission asks this nonforcing presence question before the raw
    /// optional constraint is demanded. Infer declarations may resolve absent.
    pub(in crate::project::semantic) fn parameter_may_have_constraint(
        &self,
        ty: TypeId,
    ) -> Result<bool, DemandError> {
        let TypeView::TypeParameter(parameter) = self.model.get(ty)? else {
            return Ok(false);
        };
        Ok(parameter.constraint.is_some() || self.constraints.inferred.contains_key(&ty))
    }
    pub(in crate::project::semantic) fn parameter_constraint_location(
        &self,
        ty: TypeId,
    ) -> Result<&Location, DemandError> {
        self.constraints
            .parameters
            .get(&ty)
            .map(|parameter| &parameter.location)
            .ok_or_else(|| {
                super::super::internal("parameter constraint has no declaration location").into()
            })
    }

    /// `getResolvedBaseConstraint`: push the property-aware native resolution
    /// frame before recursion identity admission, and always pop it before
    /// preserving either an installed native sentinel or a product failure.
    fn resolved_base_constraint(
        &mut self,
        ty: TypeId,
        stack: &mut Vec<RecursionIdentity>,
        consumer: &Location,
    ) -> NativeResult<BaseConstraint> {
        if !self.model.has_type_flags(
            ty,
            TypeFlags::TYPE_PARAMETER
                | TypeFlags::UNION
                | TypeFlags::INTERSECTION
                | TypeFlags::INDEXED_ACCESS
                | TypeFlags::INDEX
                | TypeFlags::OBJECT
                | TypeFlags::TEMPLATE_LITERAL
                | TypeFlags::STRING_MAPPING
                | TypeFlags::CONDITIONAL
                | TypeFlags::SUBSTITUTION,
        ) {
            return Ok(NativeCompletion::clean(BaseConstraint::Type(ty)));
        }
        if let Some(state) = self.constraints.bases.get(&ty) {
            return match state {
                BaseState::Installed(base) => Ok(base.clone()),
                BaseState::Failed(error) => Err(error.clone()),
            };
        }
        if !self.push_resolution(resolution::Key::BaseConstraint(ty)) {
            return Ok(NativeCompletion::clean(BaseConstraint::Circular(
                self.circular_constraint(ty, consumer),
            )));
        }
        let computed = (|| {
            let identity = self.recursion_identity(ty, consumer)?;
            let repeat = stack.contains(&identity);
            if stack.len() < 10 || stack.len() < 50 && !repeat {
                stack.push(identity);
                let result = self
                    .simplified_type_completion(ty, false, consumer)
                    .and_then(|simplified| {
                        simplified.and_then(|simplified| {
                            self.compute_base_constraint_completion(simplified, stack, consumer)
                        })
                    });
                stack.pop();
                result
            } else {
                Ok(NativeCompletion::clean(BaseConstraint::None(
                    NoConstraint {
                        safety_stop: Some(NativeSafetyStop {
                            depth: stack.len(),
                            repeated_identity: repeat,
                        }),
                    },
                )))
            }
        })();
        let non_circular = self
            .resolution
            .pop()
            .ok_or_else(|| super::super::internal("missing base-constraint resolution frame"))?;
        // `getResolvedBaseConstraint` can choose its native circular sentinel
        // only after semantic computation. Required frame cleanup does not
        // turn a product interruption into that successful native value.
        let result = match computed {
            Ok(value) if !non_circular => value.and_then(|_| {
                let mut sequence = NativeSequence::new();
                let result = (|| {
                    if self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER)
                        && self.constraint_declaration(ty, consumer)?.is_some()
                    {
                        sequence
                            .read(self.check_diagnostic_type_display_completion(ty, consumer))?;
                    }
                    Ok(BaseConstraint::Circular(
                        self.circular_constraint(ty, consumer),
                    ))
                })();
                sequence.finish(result)
            }),
            value => value,
        };
        self.constraints
            .bases
            .entry(ty)
            .or_insert_with(|| match &result {
                Ok(base) => BaseState::Installed(base.clone()),
                Err(error) => BaseState::Failed(error.clone()),
            });
        result
    }

    /// `computeBaseConstraint` / `getNextBaseConstraint` for established
    /// native parameter, union, intersection and ordinary structured types. Ordered
    /// children are all acquired even if an earlier one has no constraint.
    fn compute_base_constraint_completion(
        &mut self,
        ty: TypeId,
        stack: &mut Vec<RecursionIdentity>,
        consumer: &Location,
    ) -> NativeResult<BaseConstraint> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let TypeView::TypeParameter(parameter) = self.model.get(ty)? {
                let Some(raw) = sequence.read(self.raw_constraint_completion(ty))? else {
                    return Ok(BaseConstraint::None(NoConstraint::default()));
                };
                return if parameter.is_this {
                    Ok(BaseConstraint::Type(raw))
                } else {
                    sequence.read(self.next_base_constraint_completion(raw, stack, consumer))
                };
            }
            if let TypeView::Conditional(_) = self.model.get(ty)? {
                if self.constraints.conditional_depth >= 100 {
                    return Ok(BaseConstraint::None(NoConstraint {
                        safety_stop: Some(NativeSafetyStop {
                            depth: 100,
                            repeated_identity: false,
                        }),
                    }));
                }
                self.constraints.conditional_depth += 1;
                let constraint = self.conditional_constraint_completion(ty, consumer);
                self.constraints.conditional_depth -= 1;
                let constraint = sequence.read(constraint)?;
                return sequence
                    .read(self.next_base_constraint_completion(constraint, stack, consumer));
            }
            if let TypeView::Substitution(_) = self.model.get(ty)? {
                let intersection =
                    sequence.read(self.substitution_intersection_completion(ty, consumer))?;
                return sequence.read(self.next_base_constraint_completion(
                    intersection,
                    stack,
                    consumer,
                ));
            }
            // Native boolean is a Union despite its public Primitive display.
            if let Some(types) = self
                .model
                .union_constituents(ty)
                .or_else(|| self.model.intersection_constituents(ty))
            {
                let mut constraints = Vec::with_capacity(types.len());
                let mut different = false;
                let mut absence = NoConstraint::default();
                for &child in types.iter() {
                    match sequence
                        .read(self.next_base_constraint_completion(child, stack, consumer))?
                    {
                        BaseConstraint::Type(constraint) => {
                            different |= constraint != child;
                            constraints.push(constraint);
                        }
                        BaseConstraint::None(missing) => {
                            different = true;
                            if absence.safety_stop.is_none() {
                                absence = missing;
                            }
                        }
                        BaseConstraint::Circular(error) => return Err(error),
                    }
                }
                if !different {
                    return Ok(BaseConstraint::Type(ty));
                }
                if self.model.has_type_flags(ty, TypeFlags::INTERSECTION) && !constraints.is_empty()
                {
                    return sequence
                        .read(self.intersection_type_completion(constraints, consumer))
                        .map(BaseConstraint::Type);
                }
                if self.model.has_type_flags(ty, TypeFlags::UNION)
                    && constraints.len() == types.len()
                {
                    return sequence
                        .read(self.union_type_ex_completion(constraints, None, None, consumer))
                        .map(BaseConstraint::Type);
                }
                return Ok(BaseConstraint::None(absence));
            }
            if let TypeView::StringMapping(mapping) = self.model.get(ty)? {
                match sequence.read(self.next_base_constraint_completion(
                    mapping.target,
                    stack,
                    consumer,
                ))? {
                    BaseConstraint::Type(constraint) if constraint != mapping.target => {
                        return sequence
                            .read(self.string_mapping_type_completion(
                                mapping.symbol,
                                mapping.kind,
                                constraint,
                                consumer,
                            ))
                            .map(BaseConstraint::Type);
                    }
                    BaseConstraint::Circular(error) => return Err(error),
                    BaseConstraint::Type(_) | BaseConstraint::None(_) => {}
                }
                return Ok(BaseConstraint::Type(
                    self.model.primitive(crate::types::PrimitiveName::String),
                ));
            }
            if let TypeView::TemplateLiteral(template) = self.model.get(ty)? {
                let mut constraints = Vec::new();
                for &child in template.types.iter() {
                    match sequence
                        .read(self.next_base_constraint_completion(child, stack, consumer))?
                    {
                        BaseConstraint::Type(ty) => constraints.push(ty),
                        BaseConstraint::None(_) => {}
                        BaseConstraint::Circular(error) => return Err(error),
                    }
                }
                let ty = if constraints.len() == template.types.len() {
                    sequence.read(self.template_literal_type_completion(
                        template.texts,
                        constraints,
                        consumer,
                    ))?
                } else {
                    self.model.primitive(crate::types::PrimitiveName::String)
                };
                return Ok(BaseConstraint::Type(ty));
            }
            if let TypeView::Index(index) = self.model.get(ty)? {
                if sequence.read(self.is_generic_mapped_type_completion(index.target, consumer))?
                    && sequence
                        .read(self.mapped_name_completion(index.target, consumer))?
                        .is_some()
                    && self.mapped_recipe(index.target)?.keyof_operand.is_none()
                {
                    let keys = sequence.read(self.mapped_index_type_completion(
                        index.target,
                        super::super::model::IndexFlags::NONE,
                        consumer,
                    ))?;
                    return sequence
                        .read(self.next_base_constraint_completion(keys, stack, consumer));
                }
                return Ok(BaseConstraint::Type(self.model.string_number_symbol_type()));
            }
            if let TypeView::IndexedAccess(access) = self.model.get(ty)? {
                if sequence.read(self.is_mapped_generic_indexed_access_completion(
                    access.object_type,
                    access.index_type,
                    consumer,
                ))? {
                    let value = sequence.read(self.substitute_indexed_mapped_type_completion(
                        access.object_type,
                        access.index_type,
                        consumer,
                    ))?;
                    return sequence
                        .read(self.next_base_constraint_completion(value, stack, consumer));
                }
                let object = sequence.read(self.next_base_constraint_completion(
                    access.object_type,
                    stack,
                    consumer,
                ))?;
                let index = sequence.read(self.next_base_constraint_completion(
                    access.index_type,
                    stack,
                    consumer,
                ))?;
                return match (object, index) {
                    (BaseConstraint::Type(object), BaseConstraint::Type(index)) => {
                        match sequence.read(
                            self.indexed_access_without_node_optional_completion(
                                object,
                                index,
                                access.access_flags,
                                consumer,
                            ),
                        )? {
                            Some(ty) => sequence
                                .read(self.next_base_constraint_completion(ty, stack, consumer)),
                            None => Ok(BaseConstraint::None(NoConstraint::default())),
                        }
                    }
                    (BaseConstraint::Circular(error), _) | (_, BaseConstraint::Circular(error)) => {
                        Err(error)
                    }
                    (BaseConstraint::None(absence), _) | (_, BaseConstraint::None(absence)) => {
                        Ok(BaseConstraint::None(absence))
                    }
                };
            }
            if self.model.is_generic_tuple_type(ty)? {
                let tuple = self.model.tuple_target(ty)?.expect("generic tuple target");
                let elements = sequence.read(self.get_element_types_completion(ty))?;
                let mut substituted = Vec::with_capacity(elements.len());
                for (&element, info) in elements.iter().zip(tuple.elements.iter()) {
                    let mut next = element;
                    if self
                        .model
                        .has_type_flags(element, TypeFlags::TYPE_PARAMETER)
                        && info
                            .flags
                            .intersects(super::super::model::TupleElementFlags::VARIADIC)
                    {
                        match sequence
                            .read(self.next_base_constraint_completion(element, stack, consumer))?
                        {
                            BaseConstraint::Type(constraint)
                                if constraint != element
                                    && self
                                        .non_generic_array_or_tuple_constraint(constraint)? =>
                            {
                                next = constraint
                            }
                            BaseConstraint::Circular(error) => return Err(error),
                            BaseConstraint::Type(_) | BaseConstraint::None(_) => {}
                        }
                    }
                    substituted.push(next);
                }
                return sequence
                    .read(self.create_tuple_type_ex_completion(
                        substituted,
                        tuple.elements.to_vec(),
                        tuple.readonly,
                        consumer,
                    ))
                    .map(BaseConstraint::Type);
            }
            // Keep this branch exhaustive: adding an established symbolic type
            // must implement its native compute/simplification decision here,
            // never inherit an unchanged-type answer from a wildcard fallback.
            match self.model.get(ty)? {
                TypeView::Special(_)
                | TypeView::Primitive(_)
                | TypeView::Enum
                | TypeView::UniqueSymbol(_)
                | TypeView::Literal { .. }
                | TypeView::Object(_)
                | TypeView::LazyObject(_)
                | TypeView::Reference(_) => Ok(BaseConstraint::Type(ty)),
                TypeView::TypeParameter(_)
                | TypeView::Union(_)
                | TypeView::Intersection(_)
                | TypeView::IndexedAccess(_)
                | TypeView::TemplateLiteral(_)
                | TypeView::Conditional(_)
                | TypeView::Substitution(_)
                | TypeView::StringMapping(_)
                | TypeView::Index(_) => Err(super::super::internal(
                    "constrained type bypassed its complete base-computation branch",
                )
                .into()),
            }
        })();
        sequence.finish(result)
    }

    /// The generic-tuple computeBaseConstraint predicate uses everyType's
    /// union-only traversal and admits actual arrays or nonvariadic tuples.
    fn non_generic_array_or_tuple_constraint(&mut self, ty: TypeId) -> Result<bool, DemandError> {
        if let Some(types) = self.model.union_constituents(ty) {
            for &ty in types.iter() {
                if !self.non_generic_array_or_tuple_constraint(ty)? {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        Ok(self.is_array_type(ty)?
            || self.model.is_tuple_type(ty) && !self.model.is_generic_tuple_type(ty)?)
    }

    fn next_base_constraint_completion(
        &mut self,
        ty: TypeId,
        stack: &mut Vec<RecursionIdentity>,
        consumer: &Location,
    ) -> NativeResult<BaseConstraint> {
        self.resolved_base_constraint(ty, stack, consumer)?
            .and_then(|base| match base {
                // Native getNextBaseConstraint maps the circular sentinel to nil;
                // the completed absence still carries its consumed recovery.
                BaseConstraint::Circular(error) => NativeCompletion::recovered(
                    BaseConstraint::None(NoConstraint::default()),
                    error,
                ),
                base => Ok(NativeCompletion::clean(base)),
            })
    }

    fn circular_constraint(&self, ty: TypeId, consumer: &Location) -> DemandError {
        let location = self
            .constraints
            .parameters
            .get(&ty)
            .map(|parameter| &parameter.location)
            .unwrap_or(consumer);
        super::super::invalid_at_location(
            self.inputs,
            location,
            SemanticFailureReason::CircularType,
            "type parameter has a circular constraint",
        )
        .into()
    }
}

impl BaseConstraint {
    fn consumed_completion(self) -> NativeResult<Option<TypeId>> {
        match self {
            Self::Type(ty) => Ok(NativeCompletion::clean(Some(ty))),
            Self::None(absence) => {
                // Native safety-stop NoConstraint is the measured compiler
                // approximation, not exhausted product work/depth/resource.
                // Retain the exact branch witness in the installed cache.
                if let Some(stop) = absence.safety_stop {
                    debug_assert!(stop.depth >= 50 || stop.depth >= 10 && stop.repeated_identity);
                }
                Ok(NativeCompletion::clean(None))
            }
            Self::Circular(error) => NativeCompletion::recovered(None, error),
        }
    }
}
