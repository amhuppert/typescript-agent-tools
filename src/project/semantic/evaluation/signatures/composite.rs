//! Native signature composition retains lazy return and predicate questions.
//! Factories publish a new identity; they never modify an input's installed cache.

use super::*;
use crate::project::semantic::evaluation::relations::SignatureComparison;
use crate::project::semantic::model::UnionReduction;
use crate::types::{PrimitiveName, TypePredicateKind};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct CompositeSignature {
    pub(super) is_union: bool,
    pub(super) signatures: Arc<[Signature]>,
    pub(super) mapper: Option<MapperId>,
}

impl Evaluator<'_, '_> {
    /// Native createUnionOfSignaturesForOverloadFailure (checker9557).
    /// Candidates are already reordered and selected by the call owner. This
    /// factory reads this, positional, rest, and return children in native order,
    /// retaining completed recovery in the factory result. The fresh symbols and
    /// eager return contain the native values only; they do not replay this
    /// factory's earlier acquisitions when read later.
    pub(in crate::project::semantic::evaluation) fn create_union_of_signatures_for_overload_failure_completion(
        &mut self,
        candidates: &[Signature],
        consumer: &Location,
    ) -> NativeResult<Signature> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Signature, DemandError> {
            let first = candidates.first().ok_or_else(|| {
                super::super::super::internal("overload failure requires a candidate")
            })?;

            let this_sources: Vec<_> = candidates
                .iter()
                .filter_map(|signature| signature.this_parameter().as_ref())
                .collect();
            let this_parameter = if this_sources.is_empty() {
                None
            } else {
                let mut types = Vec::with_capacity(this_sources.len());
                for parameter in &this_sources {
                    // Native asks getTypeOfParameter, including declaration
                    // optionality. signature_this_type asks a different raw slot.
                    types.push(sequence.read(self.demand_completion(parameter.call_type))?);
                }
                let ty = sequence.read(self.union_type_reduced_completion(
                    types,
                    UnionReduction::Subtype,
                    None,
                    None,
                    consumer,
                ))?;
                Some(self.combined_parameter_for_overload_failure(&this_sources, ty, consumer)?)
            };

            // Pure raw counts, deliberately not tuple-expanded parameterCount or
            // resolved minArgumentCount. No rest-type read occurs here.
            let mut minimum = Self::signature_non_rest_parameter_count(first);
            let mut maximum = minimum;
            for signature in &candidates[1..] {
                let count = Self::signature_non_rest_parameter_count(signature);
                minimum = minimum.min(count);
                maximum = maximum.max(count);
            }
            let mut parameters = Vec::with_capacity(maximum);
            for position in 0..maximum {
                let mut sources = Vec::new();
                for signature in candidates {
                    let parameter = if signature.flags.has_rest() {
                        if position < Self::signature_non_rest_parameter_count(signature) {
                            signature.parameters.get(position)
                        } else {
                            signature.parameters.last()
                        }
                    } else {
                        signature.parameters.get(position)
                    };
                    if let Some(parameter) = parameter {
                        sources.push(parameter);
                    }
                }
                let mut types = Vec::new();
                for signature in candidates {
                    if let Some(ty) = sequence.read(
                        self.signature_type_at_position_completion(signature, position, consumer),
                    )? {
                        types.push(ty);
                    }
                }
                let ty = sequence.read(self.union_type_reduced_completion(
                    types,
                    UnionReduction::Subtype,
                    None,
                    None,
                    consumer,
                ))?;
                parameters
                    .push(self.combined_parameter_for_overload_failure(&sources, ty, consumer)?);
            }

            let mut rest_sources = Vec::new();
            for signature in candidates {
                if signature.flags.has_rest() {
                    rest_sources.push(signature.parameters.last().ok_or_else(|| {
                        super::super::super::internal("rest signature has no parameter")
                    })?);
                }
            }
            let has_rest = !rest_sources.is_empty();
            if has_rest {
                let mut types = Vec::new();
                for signature in candidates {
                    if let Some(ty) = sequence
                        .read(self.try_rest_type_of_signature_completion(signature, consumer))?
                    {
                        types.push(ty);
                    }
                }
                let element = sequence.read(self.union_type_reduced_completion(
                    types,
                    UnionReduction::Subtype,
                    None,
                    None,
                    consumer,
                ))?;
                let ty = sequence.read(self.array_type_completion(element, consumer))?;
                parameters.push(self.combined_parameter_for_overload_failure(
                    &rest_sources,
                    ty,
                    consumer,
                )?);
            }
            let has_literal_types = candidates
                .iter()
                .any(|signature| signature.flags.has_literal_types());

            let mut returns = Vec::with_capacity(candidates.len());
            for signature in candidates {
                returns
                    .push(sequence.read(self.raw_return_type_of_signature_completion(signature))?);
            }
            let return_type =
                sequence.read(self.intersection_type_completion(returns, consumer))?;
            let signature = self.model.signature(SignatureData {
                type_parameter_types: Vec::new().into(),
                type_parameters: Vec::new(),
                this_parameter,
                parameters: parameters.into(),
                return_type: TypeDemand::Known(return_type),
                predicate: None,
                origins: first.origins.clone().into(),
                minimum_arguments: minimum,
                flags: SignatureFlags::overload_failure(has_rest, has_literal_types),
            })?;
            self.copy_signature_declaration(first.id, signature.id)?;
            self.attach_signature_return(&signature, consumer.clone())?;
            Ok(signature)
        })();
        sequence.finish(result)
    }

    /// Native createCombinedSymbolForOverloadFailure selects the first actual
    /// source even if its positional type was absent. createSymbolWithType owns
    /// copied declarations, optionality, target links, and fresh symbol identity.
    fn combined_parameter_for_overload_failure(
        &mut self,
        sources: &[&ParameterValue],
        ty: TypeId,
        consumer: &Location,
    ) -> Result<ParameterValue, DemandError> {
        let source = sources.first().ok_or_else(|| {
            super::super::super::internal("combined overload parameter has no source")
        })?;
        self.parameter_with_type(source, ty, consumer)
    }
    /// `someSignature` applies its predicate to the immediate constituents
    /// of union composites, to ordinary signatures themselves, and to no
    /// signature for a non-union composite. It is not recursive flattening.
    pub(in crate::project::semantic::evaluation) fn some_abstract_signature(
        &self,
        signatures: &[Signature],
    ) -> bool {
        signatures.iter().any(
            |signature| match self.signatures.composites.get(&signature.id) {
                Some(composite) => {
                    composite.is_union
                        && composite
                            .signatures
                            .iter()
                            .any(|signature| signature.flags.has_abstract())
                }
                None => signature.flags.has_abstract(),
            },
        )
    }
    /// `createSymbolWithType(parameter,nil)` creates its unresolved raw
    /// property before contextual assignment reads the source parameter.
    pub(in crate::project::semantic::evaluation) fn unresolved_parameter_clone(
        &mut self,
        template: &ParameterValue,
        consumer: &Location,
    ) -> Result<ParameterValue, DemandError> {
        let symbol = self.create_property_with_type(template.symbol, None, false, consumer)?;
        let mut parameter = self.parameter_value_with_symbol(template, symbol, consumer);
        // createSymbolWithType copies only Readonly check flags. Real
        // declaration optional/rest syntax survives; synthetic checks do not.
        if template.declaration.is_none() {
            parameter.optional = false;
            parameter.rest = false;
        }
        Ok(parameter)
    }
    fn fresh_signature_demand(&mut self, recipe: DemandRecipe, consumer: &Location) -> TypeDemand {
        let id = self.register_demand(TypeDemandState {
            recipe,
            location: Some(consumer.clone()),
            state: None,
        });
        TypeDemand::Deferred(id)
    }

    /// `cloneSignature` preserves declaration, target, mapper and composite,
    /// but never carries an installed return or predicate into the fresh identity.
    pub(in crate::project::semantic::evaluation) fn clone_signature(
        &mut self,
        source: &Signature,
        consumer: &Location,
    ) -> Result<Signature, DemandError> {
        let return_type = self.fresh_signature_demand(DemandRecipe::SignatureReturn, consumer);
        let signature = self.model.signature(SignatureData {
            type_parameter_types: source.type_parameter_types().clone(),
            type_parameters: source.type_parameters().clone(),
            this_parameter: source.this_parameter().clone(),
            parameters: source.parameters.clone().into(),
            return_type,
            predicate: None,
            origins: source.origins.clone().into(),
            minimum_arguments: source.minimum_arguments,
            flags: source.flags.propagating(),
        })?;
        self.copy_signature_declaration(source.id, signature.id)?;
        if let Some(target) = self.signatures.instantiated.get(&source.id).cloned() {
            self.signatures.instantiated.insert(signature.id, target);
        }
        if let Some(composite) = self.signatures.composites.get(&source.id).cloned() {
            self.signatures.composites.insert(signature.id, composite);
        }
        self.attach_signature_return(&signature, consumer.clone())?;
        Ok(signature)
    }

    fn copy_signature_declaration(
        &mut self,
        source: SignatureId,
        target: SignatureId,
    ) -> Result<(), DemandError> {
        let declaration = self
            .signatures
            .declarations
            .get(&source)
            .copied()
            .ok_or_else(|| {
                super::super::super::internal("cloned signature has no declaration association")
            })?;
        self.signatures.declarations.insert(target, declaration);
        Ok(())
    }

    /// Native eager resolvedReturnType overrides occur while a fresh signature
    /// is private to its factory. The unused reverse lazy attachment is removed.
    pub(in crate::project::semantic::evaluation) fn install_signature_return(
        &mut self,
        signature: &mut Signature,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        let (previous, _) = self
            .signatures
            .returns
            .remove(&signature.id)
            .ok_or_else(|| {
                super::super::super::internal("fresh signature has no return association")
            })?;
        if let TypeDemand::Deferred(id) = previous {
            self.signatures.return_signatures.remove(&id);
        }
        signature.return_type = TypeDemand::Known(ty);
        self.attach_signature_return(signature, consumer.clone())
    }

    /// Native `unknownSignature` has an already installed errorType return.
    /// Collecting it succeeds; a consumer of that return retains the recovery.
    pub(in crate::project::semantic::evaluation) fn unknown_signature(
        &mut self,
    ) -> Result<Signature, DemandError> {
        if let Some(signature) = &self.signatures.unknown {
            return Ok(signature.clone());
        }
        let id = self.register_demand(TypeDemandState {
            recipe: DemandRecipe::SignatureReturn,
            location: None,
            state: Some(TypeState::Installed(NativeCompletion::recovered(
                self.model.native_error_type(),
                super::super::super::unbound_recovery(
                    crate::analysis::UnboundSemanticRecovery::UnknownSignatureReturn,
                ),
            )?)),
        });
        let return_type = TypeDemand::Deferred(id);
        let signature = self.model.signature(SignatureData {
            type_parameter_types: Vec::new().into(),
            type_parameters: Vec::new(),
            this_parameter: None,
            parameters: Vec::new().into(),
            return_type,
            predicate: None,
            origins: Vec::new().into(),
            minimum_arguments: 0,
            flags: SignatureFlags::declared(false, false, false, false),
        })?;
        self.attach_signature_declaration(signature.id, None)?;
        self.attach_signature_return(&signature, None)?;
        self.signatures.unknown = Some(signature.clone());
        Ok(signature)
    }

    /// `createUnionSignature` clears target/mapper after cloning, even when
    /// the selected input is itself an instantiated or composite signature.
    pub(in crate::project::semantic::evaluation) fn create_union_signature(
        &mut self,
        source: &Signature,
        signatures: Vec<Signature>,
        consumer: &Location,
    ) -> Result<Signature, DemandError> {
        let result = self.clone_signature(source, consumer)?;
        self.signatures.instantiated.remove(&result.id);
        self.signatures.composites.insert(
            result.id,
            CompositeSignature {
                is_union: true,
                signatures: signatures.into(),
                mapper: None,
            },
        );
        Ok(result)
    }

    /// `getReturnTypeOfSignature`: target substitution precedes composition;
    /// composite returns reduce subtypes before applying their optional mapper.
    pub(in crate::project::semantic::evaluation) fn synthetic_signature_return_completion(
        &mut self,
        signature: SignatureId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some((target, mapper)) = self.signatures.instantiated.get(&signature).cloned() {
            return self
                .raw_return_type_of_signature_completion(&target)?
                .and_then(|ty| self.instantiate_type_completion(ty, mapper, consumer));
        }
        if let Some(composite) = self.signatures.composites.get(&signature).cloned() {
            let mut types = NativeCompletion::clean(Vec::with_capacity(composite.signatures.len()));
            for signature in composite.signatures.iter() {
                types = types.and_then(|mut types| {
                    self.raw_return_type_of_signature_completion(signature)
                        .map(|value| {
                            value.map(|ty| {
                                types.push(ty);
                                types
                            })
                        })
                })?;
            }
            return types
                .and_then(|types| {
                    self.signature_composite_type_completion(
                        types,
                        composite.is_union,
                        UnionReduction::Subtype,
                        consumer,
                    )
                })?
                .and_then(|ty| match composite.mapper {
                    Some(mapper) => self.instantiate_type_completion(ty, mapper, consumer),
                    None => Ok(NativeCompletion::clean(ty)),
                });
        }
        let Some((root, recipe)) = self.signature_return_recipe(signature)? else {
            return Err(super::super::super::internal(
                "unresolved synthetic signature has no return computation",
            )
            .into());
        };
        self.check_signature_return_completion(root, &recipe)
    }

    pub(in crate::project::semantic::evaluation) fn signature_return_recipe(
        &self,
        signature: SignatureId,
    ) -> Result<Option<(SignatureRoot, SignatureReturnRecipe)>, DemandError> {
        let Some(root) = self.signature_declaration_root(signature)? else {
            return Ok(None);
        };
        let id = self
            .signature_demands
            .get(&(root, SignatureQuestion::Return))
            .ok_or_else(|| {
                super::super::super::internal("signature declaration has no return recipe")
            })?;
        match &self.demands[id.index()].recipe {
            DemandRecipe::Return { root, recipe } => Ok(Some((*root, recipe.clone()))),
            _ => Err(super::super::super::internal(
                "signature declaration return recipe has another role",
            )
            .into()),
        }
    }

    /// `isResolvingReturnTypeOfSignature` inspects composite children before
    /// its own installed property and cycle stack, including mapped composites.
    pub(in crate::project::semantic::evaluation) fn is_resolving_signature_return(
        &self,
        signature: &Signature,
    ) -> bool {
        self.signatures
            .composites
            .get(&signature.id)
            .is_some_and(|composite| {
                composite
                    .signatures
                    .iter()
                    .any(|child| self.is_resolving_signature_return(child))
            })
            || (!self
                .signatures
                .has_return_property(signature.id, &self.demands)
                && self
                    .resolution_cycle_start(resolution::Key::SignatureReturn(signature.id))
                    .is_some())
    }

    /// `getUnionOrIntersectionTypePredicate` ignores union false returns,
    /// rejects assertions/mismatched kinds, and deliberately does not map its result.
    pub(super) fn composite_signature_predicate_completion(
        &mut self,
        composite: &CompositeSignature,
        consumer: &Location,
    ) -> NativeResult<Option<Predicate>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut last: Option<Predicate> = None;
            let mut types = Vec::new();
            for signature in composite.signatures.iter() {
                if let Some(predicate) = sequence
                    .read(self.resolve_signature_predicate_completion(signature, consumer))?
                {
                    if predicate.kind != TypePredicateKind::Is
                        || last
                            .as_ref()
                            .is_some_and(|last| !predicate_kinds_match(last, &predicate))
                    {
                        return Ok(None);
                    }
                    let ty = predicate.value_type.ok_or_else(|| {
                        super::super::super::internal(
                            "ordinary type predicate has no asserted type",
                        )
                    })?;
                    types.push(sequence.read(self.demand_completion(ty))?);
                    last = Some(predicate);
                } else {
                    if !composite.is_union {
                        return Ok(None);
                    }
                    let ty =
                        sequence.read(self.raw_return_type_of_signature_completion(signature))?;
                    let false_type = self.model.literal(PrimitiveName::Boolean, "false".into());
                    if self.model.regular_literal(ty) != self.model.regular_literal(false_type) {
                        return Ok(None);
                    }
                }
            }
            let Some(mut result) = last else {
                return Ok(None);
            };
            result.value_type = Some(TypeDemand::Known(sequence.read(
                self.signature_composite_type_completion(
                    types,
                    composite.is_union,
                    UnionReduction::Literal,
                    consumer,
                ),
            )?));
            Ok(Some(result))
        })();
        sequence.finish(result)
    }

    pub(in crate::project::semantic::evaluation) fn signature_composite_type_completion(
        &mut self,
        types: Vec<TypeId>,
        is_union: bool,
        reduction: UnionReduction,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = if is_union {
            sequence
                .read(self.union_type_reduced_completion(types, reduction, None, None, consumer))
        } else {
            sequence.read(self.intersection_type_completion(types, consumer))
        };
        sequence.finish(result)
    }

    /// `createSymbolWithType` preserves the original parameter declaration's
    /// call/display transformations independently of its synthetic resolved type.
    pub(in crate::project::semantic::evaluation) fn parameter_with_type(
        &mut self,
        template: &ParameterValue,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<ParameterValue, DemandError> {
        let symbol = self.create_property_with_type(template.symbol, Some(ty), false, consumer)?;
        let mut parameter = self.parameter_value_with_symbol(template, symbol, consumer);
        // createSymbolWithType copies only Readonly check flags. Real
        // declaration optional/rest syntax survives; synthetic checks do not.
        if template.declaration.is_none() {
            parameter.optional = false;
            parameter.rest = false;
        }
        Ok(parameter)
    }

    pub(in crate::project::semantic::evaluation) fn parameter_value_with_symbol(
        &mut self,
        template: &ParameterValue,
        symbol: super::super::super::model::PropertySymbolId,
        consumer: &Location,
    ) -> ParameterValue {
        let value_type = self.register_property_symbol_demand(symbol, false, consumer);
        let call_type = if template.transforms.call_optional {
            self.fresh_signature_demand(
                DemandRecipe::ParameterCall {
                    read_type: value_type,
                },
                consumer,
            )
        } else {
            value_type
        };
        let display_type = if template.transforms.display_optional {
            self.fresh_signature_demand(
                DemandRecipe::ParameterDisplay {
                    read_type: value_type,
                },
                consumer,
            )
        } else {
            value_type
        };
        ParameterValue {
            symbol,
            value_type,
            call_type,
            display_type,
            ..template.clone()
        }
    }

    pub(in crate::project::semantic::evaluation) fn synthetic_parameter(
        &mut self,
        name: String,
        ty: TypeId,
        optional: bool,
        rest: bool,
        consumer: &Location,
    ) -> ParameterValue {
        let symbol = self.create_parameter_symbol(&name, optional, rest, Some(ty));
        self.synthetic_parameter_value(symbol, name, optional, rest, consumer)
    }

    /// Descriptor of an already-created native parameter. The raw demand
    /// always reads its actual Type slot, including contextually assigned types.
    pub(super) fn synthetic_parameter_value(
        &mut self,
        symbol: super::super::super::model::PropertySymbolId,
        name: String,
        optional: bool,
        rest: bool,
        consumer: &Location,
    ) -> ParameterValue {
        let value_type = self.register_property_symbol_demand(symbol, false, consumer);
        ParameterValue {
            symbol,
            name,
            declaration: None,
            value_type,
            call_type: value_type,
            display_type: value_type,
            optional,
            rest,
            transforms: Default::default(),
        }
    }

    /// `getParameterNameAtPosition` reads the raw rest type before selecting
    /// the tuple label owner. Ordinary parameter names are retained binder text.
    pub(in crate::project::semantic::evaluation) fn signature_parameter_name_completion(
        &mut self,
        signature: &Signature,
        position: usize,
        _consumer: &Location,
    ) -> NativeResult<String> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<String, DemandError> {
            let ordinary = Self::signature_non_rest_parameter_count(signature);
            if position < ordinary {
                return Ok(signature.parameters[position].name.clone());
            }
            let rest = signature.parameters.get(ordinary).ok_or_else(|| {
                super::super::super::internal("parameter name position has no native parameter")
            })?;
            let ty = sequence.read(self.demand_completion(rest.value_type))?;
            if let Some(tuple) = self.model.tuple_target(ty)? {
                let index = position - ordinary;
                let info = tuple.elements.get(index).ok_or_else(|| {
                    super::super::super::internal(
                        "tuple parameter name position exceeds established elements",
                    )
                })?;
                return self.tuple_parameter_element_name(info, Some(rest), index);
            }
            Ok(rest.name.clone())
        })();
        sequence.finish(result)
    }

    /// `findMatchingSignatures` requires exact generic matches only from the
    /// first list. Nongeneric matches try exact before partial subtype matching.
    fn matching_signatures_completion(
        &mut self,
        lists: &[Arc<[Signature]>],
        signature: &Signature,
        list_index: usize,
        consumer: &Location,
    ) -> NativeResult<Option<Vec<Signature>>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Vec<Signature>>, DemandError> {
            if !signature.type_parameter_types().is_empty() {
                if list_index > 0 {
                    return Ok(None);
                }
                for list in &lists[1..] {
                    if sequence
                        .read(self.find_matching_signature_completion(
                            list,
                            signature,
                            SignatureComparison::default(),
                            consumer,
                        ))?
                        .is_none()
                    {
                        return Ok(None);
                    }
                }
                return Ok(Some(vec![signature.clone()]));
            }
            let mut result: Vec<Signature> = Vec::new();
            for (index, list) in lists.iter().enumerate() {
                let matched = if index == list_index {
                    Some(signature.clone())
                } else {
                    let options = SignatureComparison {
                        ignore_return_types: true,
                        ..Default::default()
                    };
                    match sequence.read(
                        self.find_matching_signature_completion(list, signature, options, consumer),
                    )? {
                        Some(signature) => Some(signature),
                        None => sequence.read(self.find_matching_signature_completion(
                            list,
                            signature,
                            SignatureComparison {
                                partial_match: true,
                                ..options
                            },
                            consumer,
                        ))?,
                    }
                };
                let Some(matched) = matched else {
                    return Ok(None);
                };
                if !result.iter().any(|prior| prior.id == matched.id) {
                    result.push(matched);
                }
            }
            Ok(Some(result))
        })();
        sequence.finish(result)
    }

    /// `getUnionSignatures` first preserves compatible overloads, then combines
    /// parameter intersections only when at most one constituent is overloaded.
    pub(in crate::project::semantic::evaluation) fn union_signatures_completion(
        &mut self,
        lists: &[Arc<[Signature]>],
        consumer: &Location,
    ) -> NativeResult<Vec<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Signature>, DemandError> {
            let mut result = Vec::new();
            let mut overloaded_index = 0;
            let mut overloaded_count = 0;
            for (index, list) in lists.iter().enumerate() {
                if list.is_empty() {
                    return Ok(Vec::new());
                }
                if list.len() > 1 {
                    overloaded_index = index;
                    overloaded_count += 1;
                }
                for signature in list.iter() {
                    if !result.is_empty()
                        && sequence
                            .read(self.find_matching_signature_completion(
                                &result,
                                signature,
                                SignatureComparison {
                                    ignore_return_types: true,
                                    ..Default::default()
                                },
                                consumer,
                            ))?
                            .is_some()
                    {
                        continue;
                    }
                    if let Some(matches) = sequence.read(
                        self.matching_signatures_completion(lists, signature, index, consumer),
                    )? {
                        let mut combined = signature.clone();
                        if matches.len() > 1 {
                            let mut this = signature.this_parameter().clone();
                            if let Some(template) = matches
                                .iter()
                                .find_map(|signature| signature.this_parameter().as_ref())
                            {
                                let mut types = Vec::new();
                                for signature in &matches {
                                    if let Some(parameter) = signature.this_parameter() {
                                        types.push(
                                            sequence.read(
                                                self.demand_completion(parameter.value_type),
                                            )?,
                                        );
                                    }
                                }
                                let ty = sequence
                                    .read(self.intersection_type_completion(types, consumer))?;
                                this = Some(self.parameter_with_type(template, ty, consumer)?);
                            }
                            combined = self.create_union_signature(signature, matches, consumer)?;
                            combined.replace_factory_this(this)?;
                        }
                        result.push(combined);
                    }
                }
            }
            if result.is_empty() && overloaded_count <= 1 && !lists.is_empty() {
                result = lists[overloaded_index].to_vec();
                for list in lists.iter() {
                    if Arc::ptr_eq(list, &lists[overloaded_index]) {
                        continue;
                    }
                    let signature = &list[0];
                    if !signature.type_parameter_types().is_empty() {
                        let mut incompatible = false;
                        for candidate in &result {
                            if !candidate.type_parameter_types().is_empty()
                                && !sequence.read(
                                    self.compare_type_parameters_identical_completion(
                                        signature.type_parameter_types(),
                                        candidate.type_parameter_types(),
                                        consumer,
                                    ),
                                )?
                            {
                                incompatible = true;
                                break;
                            }
                        }
                        if incompatible {
                            return Ok(Vec::new());
                        }
                    }
                    result = result
                        .iter()
                        .map(|candidate| {
                            sequence.read(self.combine_member_signatures_completion(
                                candidate, signature, true, consumer,
                            ))
                        })
                        .collect::<Result<_, _>>()?;
                    if result.is_empty() {
                        break;
                    }
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `combineUnionOrIntersectionMemberSignatures` preserves native left
    /// declaration/defaults and the distinct flattening versus mapper rules.
    pub(in crate::project::semantic::evaluation) fn combine_member_signatures_completion(
        &mut self,
        left: &Signature,
        right: &Signature,
        is_union: bool,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Signature, DemandError> {
            let parameters_from = if left.type_parameter_types().is_empty() {
                right
            } else {
                left
            };
            let mapper = if !left.type_parameter_types().is_empty()
                && !right.type_parameter_types().is_empty()
            {
                Some(self.model.mapper(
                    right.type_parameter_types().to_vec(),
                    left.type_parameter_types().to_vec(),
                )?)
            } else {
                None
            };
            let parameters = sequence.read(self.combine_signature_parameters_completion(
                left, right, mapper, is_union, consumer,
            ))?;
            let rest = parameters.last().is_some_and(|parameter| parameter.rest);
            let this_parameter = match (left.this_parameter(), right.this_parameter()) {
                (None, right) => right.clone(),
                (left, None) => left.clone(),
                (Some(left), Some(right)) => {
                    let left_type = sequence.read(self.demand_completion(left.value_type))?;
                    let mut right_type = sequence.read(self.demand_completion(right.value_type))?;
                    if let Some(mapper) = mapper {
                        right_type = sequence
                            .read(self.instantiate_type_completion(right_type, mapper, consumer))?;
                    }
                    let ty = sequence.read(self.signature_composite_type_completion(
                        vec![left_type, right_type],
                        !is_union,
                        UnionReduction::Literal,
                        consumer,
                    ))?;
                    Some(self.parameter_with_type(left, ty, consumer)?)
                }
            };
            let return_type = self.fresh_signature_demand(DemandRecipe::SignatureReturn, consumer);
            let result = self.model.signature(SignatureData {
                type_parameter_types: parameters_from.type_parameter_types().clone(),
                type_parameters: parameters_from.type_parameters().clone(),
                this_parameter,
                parameters: parameters.into(),
                return_type,
                predicate: None,
                origins: left.origins.clone().into(),
                minimum_arguments: left.minimum_arguments.max(right.minimum_arguments),
                flags: left.flags.combined(right.flags, rest),
            })?;
            self.copy_signature_declaration(left.id, result.id)?;
            let previous = self.signatures.composites.get(&left.id).cloned();
            let mut signatures = match &previous {
                Some(previous) if previous.is_union => previous.signatures.to_vec(),
                _ => vec![left.clone()],
            };
            signatures.push(right.clone());
            let previous_mapper = previous
                .filter(|previous| previous.is_union == is_union)
                .and_then(|previous| previous.mapper);
            let mapper = match (previous_mapper, mapper) {
                (Some(left), Some(right)) => Some(self.model.combine_mappers(left, right)),
                (left, right) => right.or(left),
            };
            self.signatures.composites.insert(
                result.id,
                CompositeSignature {
                    is_union,
                    signatures: signatures.into(),
                    mapper,
                },
            );
            self.attach_signature_return(&result, consumer.clone())?;
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `combineUnionOrIntersectionParameters` uses positional/rest operations
    /// before optionality and name acquisition, preserving right-side mapping.
    fn combine_signature_parameters_completion(
        &mut self,
        left: &Signature,
        right: &Signature,
        mapper: Option<MapperId>,
        is_union: bool,
        consumer: &Location,
    ) -> NativeResult<Vec<ParameterValue>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<ParameterValue>, DemandError> {
            let left_count =
                sequence.read(self.signature_parameter_count_completion(left, consumer))?;
            let right_count =
                sequence.read(self.signature_parameter_count_completion(right, consumer))?;
            let (longest_count, longest, shorter) = if left_count >= right_count {
                (left_count, left, right)
            } else {
                (right_count, right, left)
            };
            let either_rest = sequence
                .read(self.signature_has_effective_rest_completion(left, consumer))?
                || sequence.read(self.signature_has_effective_rest_completion(right, consumer))?;
            let extra_rest = either_rest
                && !sequence
                    .read(self.signature_has_effective_rest_completion(longest, consumer))?;
            let mut result = Vec::with_capacity(longest_count + usize::from(extra_rest));
            for index in 0..longest_count {
                let mut longest_type = sequence
                    .read(self.signature_type_at_position_completion(longest, index, consumer))?
                    .ok_or_else(|| {
                        super::super::super::internal(
                            "longest signature parameter position is absent",
                        )
                    })?;
                if longest.id == right.id {
                    if let Some(mapper) = mapper {
                        longest_type = sequence.read(self.instantiate_type_completion(
                            longest_type,
                            mapper,
                            consumer,
                        ))?;
                    }
                }
                let mut shorter_type = sequence
                    .read(self.signature_type_at_position_completion(shorter, index, consumer))?
                    .unwrap_or_else(|| self.model.special(SpecialName::Unknown));
                if shorter.id == right.id {
                    if let Some(mapper) = mapper {
                        shorter_type = sequence.read(self.instantiate_type_completion(
                            shorter_type,
                            mapper,
                            consumer,
                        ))?;
                    }
                }
                let mut ty = sequence.read(self.signature_composite_type_completion(
                    vec![longest_type, shorter_type],
                    !is_union,
                    UnionReduction::Literal,
                    consumer,
                ))?;
                let rest = either_rest && !extra_rest && index == longest_count - 1;
                let optional = index
                    >= sequence.read(self.minimum_argument_count_completion(longest, consumer))?
                    && index
                        >= sequence
                            .read(self.minimum_argument_count_completion(shorter, consumer))?;
                let left_name = if index < left_count {
                    sequence
                        .read(self.signature_parameter_name_completion(left, index, consumer))?
                } else {
                    String::new()
                };
                let right_name = if index < right_count {
                    sequence
                        .read(self.signature_parameter_name_completion(right, index, consumer))?
                } else {
                    String::new()
                };
                let name = if left_name == right_name || right_name.is_empty() {
                    left_name
                } else if left_name.is_empty() {
                    right_name
                } else {
                    String::new()
                };
                let name = if name.is_empty() {
                    format!("arg{index}")
                } else {
                    name
                };
                if rest {
                    ty = sequence.read(self.array_type_completion(ty, consumer))?;
                }
                result.push(self.synthetic_parameter(name, ty, optional && !rest, rest, consumer));
            }
            if extra_rest {
                let element = sequence.read(self.signature_type_at_position_or_any_completion(
                    shorter,
                    longest_count,
                    consumer,
                ))?;
                let mut ty = sequence.read(self.array_type_completion(element, consumer))?;
                if shorter.id == right.id {
                    if let Some(mapper) = mapper {
                        ty = sequence
                            .read(self.instantiate_type_completion(ty, mapper, consumer))?;
                    }
                }
                result.push(self.synthetic_parameter("args".into(), ty, false, true, consumer));
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `appendSignatures` deduplicates with independent native Identity pairs.
    pub(in crate::project::semantic::evaluation) fn append_signatures_completion(
        &mut self,
        result: &mut Vec<Signature>,
        signatures: &[Signature],
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            for signature in signatures {
                if sequence
                    .read(self.find_matching_signature_completion(
                        result,
                        signature,
                        SignatureComparison::default(),
                        consumer,
                    ))?
                    .is_none()
                {
                    result.push(signature.clone());
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }
}
