//! Native inference slots outlive the operation that collected candidates.
//! Dynamic mappers observe those same slots; they never freeze a substitute
//! table or manufacture a new relation after its captured invocation retires.
mod mapped;
mod templates;

use super::super::model::{
    DemandError, InferenceContextId, MapperId, NativeCompletion, NativeResult, NativeSequence,
    ObjectFlags, Signature, SpecialName, TupleElementFlags, TypeFlags, TypeId, TypeView,
    UnionReduction, VarianceFlags,
};
use super::super::source::ExpressionRoot;
use super::Evaluator;
use super::calls::CallInput;
use super::contextual::{CheckMode, ContextFlags, ContextNode};
use super::relations::{RelationComparer, RelationKind};
use crate::analysis::SemanticFailureReason;
use crate::references::Location;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Priority(i32);

enum InferenceAction {
    Objects,
    Conditional,
    Mapped,
}

impl Priority {
    const NONE: Self = Self(0);
    const RETURN: Self = Self(1 << 7);
    const NAKED: Self = Self(1);
    const SPECULATIVE_TUPLE: Self = Self(1 << 1);
    const SUBSTITUTE_SOURCE: Self = Self(1 << 2);
    const CONTRAVARIANT_CONDITIONAL: Self = Self(1 << 6);
    const CIRCULARITY: Self = Self(-1);
    const ALWAYS_STRICT: i32 = 1 << 10;
    const NO_CONSTRAINTS: i32 = 1 << 9;
    const MAX: Self = Self(1 << 11);
    const COMBINATION: i32 = (1 << 7) | (1 << 5) | (1 << 8);
}

#[derive(Clone)]
struct InferenceInfo {
    parameter: TypeId,
    candidates: Vec<(TypeId, usize)>,
    contra_candidates: Vec<TypeId>,
    inferred: Option<NativeResult<TypeId>>,
    priority: Priority,
    top_level: bool,
    fixed: bool,
    implied_arity: Option<usize>,
}

struct InferenceContext {
    infos: Vec<InferenceInfo>,
    signature: Option<Signature>,
    comparer: Option<RelationComparer>,
    fixing_mapper: MapperId,
    non_fixing_mapper: MapperId,
    no_default: bool,
    skipped_generic_function: bool,
    return_mapper: Option<MapperId>,
    outer_return_mapper: Option<MapperId>,
    intra_expression_sites: Vec<(ExpressionRoot, TypeId)>,
    failure: Option<DemandError>,
}

#[derive(Default)]
pub(super) struct InferenceContexts {
    contexts: Vec<InferenceContext>,
    reverse: mapped::ReverseState,
}

/// One `inferTypes` traversal. Candidate caches live in its context, while
/// priority, variance and traversal depth belong to this individual pass.
struct InferencePass {
    context: InferenceContextId,
    original_target: TypeId,
    priority: Priority,
    depth: usize,
    contravariant: bool,
    bivariant: bool,
    inference_priority: Priority,
    visited: HashMap<(TypeId, TypeId), Priority>,
    source_stack: Vec<TypeId>,
    target_stack: Vec<TypeId>,
    expanding: u8,
}

#[derive(Clone, Copy)]
enum InferenceMatch {
    Identity,
    IdentityOrBase,
    Close,
}

impl Evaluator<'_, '_> {
    /// The `getConditionalType` inference context shares native candidate
    /// slots and callback mappers; conditional evaluation owns no inference cache.
    pub(super) fn conditional_inference_mapper_completion(
        &mut self,
        parameters: Vec<TypeId>,
        source: Option<TypeId>,
        target: TypeId,
        mapper: Option<MapperId>,
        comparer: Option<RelationComparer>,
        consumer: &Location,
    ) -> NativeResult<MapperId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<MapperId, DemandError> {
            let context = self.new_inference_context(parameters, None, comparer);
            if let Some(mapper) = mapper {
                let nonfixing = self.inference.contexts[context.index()].non_fixing_mapper;
                self.inference.contexts[context.index()].non_fixing_mapper =
                    self.model.combine_mappers(nonfixing, mapper);
            }
            if let Some(source) = source
                && let Err(error) = sequence.read(self.infer_types_completion(
                    context,
                    source,
                    target,
                    Priority(Priority::NO_CONSTRAINTS | Priority::ALWAYS_STRICT),
                    consumer,
                ))
            {
                self.fail_inference_context(context, error.clone());
                return Err(error);
            }
            let fixing = self.inference.contexts[context.index()].fixing_mapper;
            Ok(match mapper {
                Some(mapper) => self.model.combine_mappers(fixing, mapper),
                None => fixing,
            })
        })();
        sequence.finish(result)
    }
    /// `newInferenceContextWorker` registers mapper identity before any
    /// candidate/default/constraint demand can call back into the context.
    fn new_inference_context(
        &mut self,
        parameters: Vec<TypeId>,
        signature: Option<&Signature>,
        comparer: Option<RelationComparer>,
    ) -> InferenceContextId {
        let infos = parameters
            .into_iter()
            .map(|parameter| InferenceInfo {
                parameter,
                candidates: Vec::new(),
                contra_candidates: Vec::new(),
                inferred: None,
                priority: Priority::MAX,
                top_level: true,
                fixed: false,
                implied_arity: None,
            })
            .collect();
        self.new_inference_context_worker(infos, signature.cloned(), comparer, false, None)
    }

    fn new_inference_context_worker(
        &mut self,
        infos: Vec<InferenceInfo>,
        signature: Option<Signature>,
        comparer: Option<RelationComparer>,
        no_default: bool,
        failure: Option<DemandError>,
    ) -> InferenceContextId {
        let id = InferenceContextId::new(self.inference.contexts.len());
        let fixing_mapper = self.model.inference_mapper(id, true);
        let non_fixing_mapper = self.model.inference_mapper(id, false);
        self.inference.contexts.push(InferenceContext {
            infos,
            signature,
            comparer,
            fixing_mapper,
            non_fixing_mapper,
            no_default,
            skipped_generic_function: false,
            return_mapper: None,
            outer_return_mapper: None,
            intra_expression_sites: Vec::new(),
            failure,
        });
        id
    }

    /// `cloneInferenceContext` / `cloneInferredPartOfContext` allocate fresh
    /// mapper identities around cloned slots. Return/site state is not cloned.
    fn clone_inference_context(
        &mut self,
        context: InferenceContextId,
        no_default: bool,
        inferred_only: bool,
    ) -> Option<InferenceContextId> {
        let original = &self.inference.contexts[context.index()];
        let infos: Vec<_> = original
            .infos
            .iter()
            .filter(|info| {
                !inferred_only || !info.candidates.is_empty() || !info.contra_candidates.is_empty()
            })
            .cloned()
            .collect();
        if inferred_only && infos.is_empty() {
            return None;
        }
        let signature = original.signature.clone();
        let comparer = original.comparer;
        let no_default = original.no_default || no_default;
        let failure = original.failure.clone();
        let skipped_generic_function = original.skipped_generic_function;
        let cloned =
            self.new_inference_context_worker(infos, signature, comparer, no_default, failure);
        self.inference.contexts[cloned.index()].skipped_generic_function = skipped_generic_function;
        Some(cloned)
    }

    /// `createOuterReturnMapper` snapshots all current slots only on the first
    /// request, then applies the independently collected return inferences first.
    fn outer_return_mapper(&mut self, context: InferenceContextId) -> MapperId {
        if let Some(mapper) = self.inference.contexts[context.index()].outer_return_mapper {
            return mapper;
        }
        let cloned = self
            .clone_inference_context(context, false, false)
            .expect("full inference clone exists");
        let mut mapper = self.inference.contexts[cloned.index()].fixing_mapper;
        if let Some(first) = self.inference.contexts[context.index()].return_mapper {
            mapper = self.model.merge_mappers(first, mapper);
        }
        self.inference.contexts[context.index()].outer_return_mapper = Some(mapper);
        mapper
    }

    pub(super) fn clear_intra_expression_sites(&mut self, context: InferenceContextId) {
        self.inference.contexts[context.index()]
            .intra_expression_sites
            .clear();
    }

    /// `inferFromAnnotatedParametersAndReturnTypes` contributes ordinary
    /// candidates to the same live context used by contextual parameter maps.
    pub(super) fn infer_contextual_annotation_completion(
        &mut self,
        context: InferenceContextId,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = sequence.read(self.infer_types_completion(
            context,
            source,
            target,
            Priority::NONE,
            consumer,
        ));
        sequence.finish(result)
    }

    /// Contextual function resolution observes the actual fixing/non-fixing
    /// mapper allocated with the context. A failed context stays failed.
    pub(super) fn inference_context_mapper(
        &self,
        context: InferenceContextId,
        nonfixing: bool,
    ) -> Result<MapperId, DemandError> {
        let context = &self.inference.contexts[context.index()];
        if let Some(error) = &context.failure {
            return Err(error.clone());
        }
        Ok(if nonfixing {
            context.non_fixing_mapper
        } else {
            context.fixing_mapper
        })
    }

    /// `addIntraExpressionInferenceSite` retains the actual expression node
    /// and checked source type for the context's later ordered inference pass.
    pub(super) fn add_intra_expression_inference_site(
        &mut self,
        context: InferenceContextId,
        root: ExpressionRoot,
        ty: TypeId,
    ) {
        self.inference.contexts[context.index()]
            .intra_expression_sites
            .push((root, ty));
    }

    /// `inferFromIntraExpressionSites` reads each current contextual type before
    /// fixation clears unfixed slots. Site lists are cleared only after the pass.
    fn infer_from_intra_expression_sites_completion(
        &mut self,
        context: InferenceContextId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let sites = self.inference.contexts[context.index()]
                .intra_expression_sites
                .clone();
            for (root, source) in sites {
                let target = if root.site.kind == crate::syntax_kind::SyntaxKind::MethodDeclaration
                {
                    sequence.read(self.contextual_object_element_type_completion(
                        root,
                        ContextFlags::NO_CONSTRAINTS,
                        consumer,
                    ))?
                } else {
                    sequence
                        .read(self.contextual_type_completion(root, ContextFlags::NO_CONSTRAINTS))?
                };
                if let Some(target) = target {
                    sequence.read(self.infer_types_completion(
                        context,
                        source,
                        target,
                        Priority::NONE,
                        consumer,
                    ))?;
                }
            }
            self.clear_intra_expression_sites(context);
            Ok(())
        })();
        sequence.finish(result)
    }

    pub(super) fn new_call_inference_context(
        &mut self,
        signature: &Signature,
    ) -> InferenceContextId {
        self.new_inference_context(
            signature.type_parameter_types().to_vec(),
            Some(signature),
            None,
        )
    }

    /// `inferTypeArguments` owns return-context acquisition before this and
    /// positional arguments. Slot/default/constraint work remains shared with
    /// contextual signature inference; call comparison uses its own comparer.
    pub(super) fn infer_call_type_arguments_completion(
        &mut self,
        input: &CallInput<'_>,
        signature: &Signature,
        context: InferenceContextId,
        mode: CheckMode,
    ) -> NativeResult<Vec<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = {
            let recipe = input.recipe;
            let result: Result<Vec<TypeId>, DemandError> = (|| {
                let consumer = &recipe.location;
                let mut skip_patterns = true;
                for &parameter in signature.type_parameter_types().iter() {
                    if sequence
                        .read(self.default_from_type_parameter_completion(parameter))?
                        .is_none()
                    {
                        skip_patterns = false;
                        break;
                    }
                }
                let root = ExpressionRoot {
                    file: recipe.root.file,
                    site: recipe.root.site(),
                };
                let flags = if skip_patterns {
                    ContextFlags::SKIP_BINDING_PATTERNS
                } else {
                    ContextFlags::NONE
                };
                if let Some(contextual) =
                    sequence.read(self.contextual_type_completion(root, flags))?
                {
                    let target = sequence
                        .read(self.get_return_type_of_signature_completion(signature, consumer))?;
                    if sequence
                        .read(self.could_contain_type_variables_completion(target, consumer))?
                    {
                        let outer = self.inference_context_for(root)?;
                        let binding_pattern = !skip_patterns
                            && sequence.read(self.contextual_type_completion(
                                root,
                                ContextFlags::SKIP_BINDING_PATTERNS,
                            ))? != Some(contextual);
                        if !binding_pattern {
                            let mut source = contextual;
                            if let Some(outer) = outer {
                                let cloned = self
                                    .clone_inference_context(outer, true, false)
                                    .expect("full inference clone exists");
                                let mapper = self.inference.contexts[cloned.index()].fixing_mapper;
                                source = sequence.read(
                                    self.instantiate_type_completion(source, mapper, consumer),
                                )?;
                            }
                            if let Some(contextual_signature) =
                                self.single_signature(source, false, false, consumer)?
                            {
                                if !contextual_signature.type_parameter_types().is_empty() {
                                    let instantiated = sequence.read(self.get_signature_instantiation_without_filling_completion(
                                    &contextual_signature,
                                    contextual_signature.type_parameter_types().to_vec(),
                                    consumer,
                                ))?;
                                    source = self.type_from_signature(&instantiated, consumer)?;
                                }
                            }
                            sequence.read(self.infer_types_completion(
                                context,
                                source,
                                target,
                                Priority::RETURN,
                                consumer,
                            ))?;
                        }
                        let returns = self.new_inference_context(
                            signature.type_parameter_types().to_vec(),
                            Some(signature),
                            None,
                        );
                        let return_result: Result<Option<MapperId>, DemandError> =
                            (|| {
                                let source = if let Some(outer) = outer {
                                    let mapper = self.outer_return_mapper(outer);
                                    sequence.read(self.instantiate_type_completion(
                                        contextual, mapper, consumer,
                                    ))?
                                } else {
                                    contextual
                                };
                                sequence.read(self.infer_types_completion(
                                    returns,
                                    source,
                                    target,
                                    Priority::NONE,
                                    consumer,
                                ))?;
                                Ok(self.clone_inference_context(returns, false, true).map(
                                    |cloned| self.inference.contexts[cloned.index()].fixing_mapper,
                                ))
                            })();
                        if let Err(error) = &return_result {
                            self.fail_inference_context(returns, error.clone());
                        }
                        self.inference.contexts[context.index()].return_mapper = return_result?;
                    }
                }
                let rest =
                    sequence.read(self.signature_non_array_rest_completion(signature, consumer))?;
                let count = if rest.is_some() {
                    input.arguments.len().min(
                        sequence
                            .read(self.signature_parameter_count_completion(signature, consumer))?
                            .checked_sub(1)
                            .ok_or_else(|| {
                                super::super::internal("native rest signature has no rest position")
                            })?,
                    )
                } else {
                    input.arguments.len()
                };
                if let Some(rest) =
                    rest.filter(|&rest| self.model.has_type_flags(rest, TypeFlags::TYPE_PARAMETER))
                {
                    if !input
                        .arguments
                        .iter()
                        .skip(count)
                        .any(|argument| argument.is_spread())
                    {
                        if let Some(info) = self.inference.contexts[context.index()]
                            .infos
                            .iter_mut()
                            .find(|info| info.parameter == rest)
                        {
                            info.implied_arity = Some(input.arguments.len() - count);
                        }
                    }
                }
                if let Some(this) = sequence.read(self.signature_this_type_completion(signature))? {
                    if sequence
                        .read(self.could_contain_type_variables_completion(this, consumer))?
                    {
                        let argument =
                            sequence.read(self.call_this_argument_type_completion(recipe))?;
                        sequence.read(self.infer_types_completion(
                            context,
                            argument,
                            this,
                            Priority::NONE,
                            consumer,
                        ))?;
                    }
                }
                for (index, argument) in input.arguments.iter().take(count).enumerate() {
                    if argument.is_omitted() {
                        continue;
                    }
                    let parameter =
                        sequence.read(self.signature_type_at_position_or_any_completion(
                            signature, index, consumer,
                        ))?;
                    if sequence
                        .read(self.could_contain_type_variables_completion(parameter, consumer))?
                    {
                        let ty = sequence.read(self.check_call_argument_completion(
                            argument,
                            parameter,
                            Some(context),
                            mode,
                        ))?;
                        sequence.read(self.infer_types_completion(
                            context,
                            ty,
                            parameter,
                            Priority::NONE,
                            consumer,
                        ))?;
                    }
                }
                if let Some(rest) = rest {
                    if sequence
                        .read(self.could_contain_type_variables_completion(rest, consumer))?
                    {
                        let spread = sequence.read(self.spread_argument_type_completion(
                            input,
                            count,
                            input.arguments.len(),
                            rest,
                            Some(context),
                            mode,
                        ))?;
                        sequence.read(self.infer_types_completion(
                            context,
                            spread,
                            rest,
                            Priority::NONE,
                            consumer,
                        ))?;
                    }
                }
                let mut arguments = Vec::new();
                for index in 0..self.inference.contexts[context.index()].infos.len() {
                    arguments.push(
                        sequence.read(self.inferred_type_completion(context, index, consumer))?,
                    );
                }
                Ok(arguments)
            })();
            if let Err(error) = &result {
                self.fail_inference_context(context, error.clone());
            }
            result
        };
        sequence.finish(result)
    }

    /// `skippedGenericFunction` records deferral only during inference.
    pub(super) fn record_skipped_generic_function(
        &mut self,
        root: impl Into<ContextNode>,
        mode: CheckMode,
    ) -> Result<(), DemandError> {
        if mode.contains(CheckMode::INFERENTIAL) {
            let context = self.inference_context_for(root)?.ok_or_else(|| {
                super::super::internal("inferential expression has no active inference context")
            })?;
            self.inference.contexts[context.index()].skipped_generic_function = true;
        }
        Ok(())
    }

    pub(super) fn inference_skipped_generic_function(&self, context: InferenceContextId) -> bool {
        self.inference.contexts[context.index()].skipped_generic_function
    }

    /// `instantiateContextualType` uses return-only inference for literals and
    /// the distinct non-fixing mapper for contextual signatures.
    pub(super) fn instantiate_contextual_type_completion(
        &mut self,
        contextual: TypeId,
        root: Option<ContextNode>,
        flags: ContextFlags,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if !self
                .model
                .maybe_type_of_kind(contextual, TypeFlags::INSTANTIABLE)
            {
                return Ok(contextual);
            }
            let Some(root) = root else {
                return Ok(contextual);
            };
            let Some(context) = self.inference_context_for(root)? else {
                return Ok(contextual);
            };
            if flags.contains(ContextFlags::SIGNATURE) {
                let mut has_candidates_or_default = false;
                for info in &self.inference.contexts[context.index()].infos {
                    if !info.candidates.is_empty() || !info.contra_candidates.is_empty() {
                        has_candidates_or_default = true;
                        break;
                    }
                    let TypeView::TypeParameter(parameter) = self.model.get(info.parameter)? else {
                        return Err(super::super::internal(
                            "inference slot has no native type parameter",
                        )
                        .into());
                    };
                    if parameter.default.is_some() {
                        has_candidates_or_default = true;
                        break;
                    }
                }
                if has_candidates_or_default {
                    let mapper = self.inference.contexts[context.index()].non_fixing_mapper;
                    let ty =
                        sequence.read(self.instantiate_instantiable_types_completion(
                            contextual, mapper, consumer,
                        ))?;
                    if !self
                        .model
                        .has_type_flags(ty, TypeFlags::ANY | TypeFlags::UNKNOWN)
                    {
                        return Ok(ty);
                    }
                }
            }
            if let Some(mapper) = self.inference.contexts[context.index()].return_mapper {
                let ty = sequence.read(
                    self.instantiate_instantiable_types_completion(contextual, mapper, consumer),
                )?;
                if !self
                    .model
                    .has_type_flags(ty, TypeFlags::ANY | TypeFlags::UNKNOWN)
                {
                    if let Some(types) = self.model.union_constituents(ty) {
                        let false_type = self
                            .model
                            .literal(crate::types::PrimitiveName::Boolean, "false".into());
                        let false_type = self.model.regular_literal(false_type);
                        let true_type = self
                            .model
                            .literal(crate::types::PrimitiveName::Boolean, "true".into());
                        let true_type = self.model.regular_literal(true_type);
                        if types.contains(&false_type) && types.contains(&true_type) {
                            return Ok(self.model.filter_type(ty, |_, part| {
                                part != false_type && part != true_type
                            }));
                        }
                    }
                    return Ok(ty);
                }
            }
            Ok(contextual)
        })();
        sequence.finish(result)
    }

    /// `instantiateInstantiableTypes` deliberately leaves objects untouched
    /// and uses UnionReductionNone for recursively instantiated union members.
    fn instantiate_instantiable_types_completion(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if self.model.has_type_flags(ty, TypeFlags::INSTANTIABLE) {
                return sequence.read(self.instantiate_type_completion(ty, mapper, consumer));
            }
            if let Some(types) = self.model.union_constituents(ty) {
                let mut instantiated = Vec::new();
                for &ty in types.iter() {
                    instantiated.push(sequence.read(
                        self.instantiate_instantiable_types_completion(ty, mapper, consumer),
                    )?);
                }
                return sequence.read(self.union_type_reduced_completion(
                    instantiated,
                    UnionReduction::None,
                    None,
                    None,
                    consumer,
                ));
            }
            if let Some(types) = self.model.intersection_constituents(ty) {
                let mut instantiated = Vec::new();
                for &ty in types.iter() {
                    instantiated.push(sequence.read(
                        self.instantiate_instantiable_types_completion(ty, mapper, consumer),
                    )?);
                }
                return sequence.read(self.intersection_type_completion(instantiated, consumer));
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    /// `instantiateTypeWithSingleGenericCallSignature` asks both signature
    /// kinds before contextual acquisition. SkipGenericFunctions records its
    /// actual wildcard result in the enclosing inference context.
    pub(super) fn instantiate_expression_signature(
        &mut self,
        node: Option<ContextNode>,
        ty: TypeId,
        mode: CheckMode,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.instantiate_expression_signature_completion(node, ty, mode, consumer)?
            .admit()
    }

    pub(super) fn instantiate_expression_signature_completion(
        &mut self,
        node: Option<ContextNode>,
        ty: TypeId,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if !mode.contains(CheckMode::INFERENTIAL | CheckMode::SKIP_GENERIC_FUNCTIONS) {
                return Ok(ty);
            }
            let call = self.single_signature(ty, false, true, consumer)?;
            let construct = self.single_signature(ty, true, true, consumer)?;
            let is_construct = call.is_none();
            let Some(signature) = call.or(construct) else {
                return Ok(ty);
            };
            if signature.type_parameter_types().is_empty() {
                return Ok(ty);
            }
            let Some(root) = node else {
                return Ok(ty);
            };
            let Some(contextual) = sequence.read(self.apparent_contextual_type_completion(
                root,
                ContextFlags::NO_CONSTRAINTS,
                consumer,
            ))?
            else {
                return Ok(ty);
            };
            let contextual = self.non_nullable_type(contextual, consumer)?;
            let Some(contextual_signature) =
                self.single_signature(contextual, is_construct, false, consumer)?
            else {
                return Ok(ty);
            };
            if !contextual_signature.type_parameter_types().is_empty() {
                return Ok(ty);
            }
            if mode.contains(CheckMode::SKIP_GENERIC_FUNCTIONS) {
                self.record_skipped_generic_function(root, mode)?;
                return Ok(self.model.any_function_type());
            }
            Err(self.inference_pending(
                consumer,
                "native higher-order contextual signature inference is pending",
            ))
        })();
        sequence.finish(result)
    }

    /// `instantiateSignatureInContextOf`: the contextual signature is the
    /// source of inference, and all slots resolve before the ordinary argument
    /// vector is published through the signature-instantiation owner.
    pub(super) fn instantiate_signature_in_context_of_completion(
        &mut self,
        signature: &Signature,
        contextual: &Signature,
        comparer: RelationComparer,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Signature, DemandError> {
            let parameters = sequence
                .read(self.get_type_parameters_for_mapper_completion(signature, consumer))?;
            let context = self.new_inference_context(parameters, Some(signature), Some(comparer));
            let result = (|| {
                sequence.read(self.signature_effective_rest_completion(contextual, consumer))?;
                sequence.read(self.apply_to_parameter_types_completion(
                    contextual,
                    signature,
                    consumer,
                    &mut |this, source, target| {
                        this.infer_types_completion(
                            context,
                            source,
                            target,
                            Priority::NONE,
                            consumer,
                        )
                    },
                ))?;
                sequence.read(self.apply_to_return_types_completion(
                    contextual,
                    signature,
                    consumer,
                    &mut |this, source, target| {
                        this.infer_types_completion(
                            context,
                            source,
                            target,
                            Priority::RETURN,
                            consumer,
                        )
                    },
                ))?;
                let mut arguments = Vec::new();
                for index in 0..self.inference.contexts[context.index()].infos.len() {
                    arguments.push(
                        sequence.read(self.inferred_type_completion(context, index, consumer))?,
                    );
                }
                sequence.read(
                    self.get_signature_instantiation_completion(signature, arguments, consumer),
                )
            })();
            if let Err(error) = &result {
                self.fail_inference_context(context, error.clone());
            }
            result
        })();
        sequence.finish(result)
    }

    /// Retain interruption only for work with no installed inferred value.
    /// `getInferredType`'s provisional publication is a native cache barrier:
    /// later non-fixing reads skip a suffix that the interrupted call attempted.
    /// Fixing mappers still acquire the context's unfinished site work first.
    fn fail_inference_context(&mut self, context: InferenceContextId, error: DemandError) {
        let context = &mut self.inference.contexts[context.index()];
        if context.failure.is_none() {
            context.failure = Some(error.clone());
        }
        for info in &mut context.infos {
            if info.inferred.is_none() {
                info.inferred = Some(Err(error.clone()));
            }
        }
    }

    /// `InferenceTypeMapper.Map` fixes only the actual matching slot. A
    /// non-fixing access can observe a provisional installed inferred type.
    /// The parameter scan stops at the first slot whose parameter is `ty`, just
    /// as the other Map implementations stop their source-table scans.
    pub(super) fn map_inference_parameter_completion(
        &mut self,
        ty: TypeId,
        context: InferenceContextId,
        fixing: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let Some(index) = self.inference.contexts[context.index()]
                .infos
                .iter()
                .position(|info| info.parameter == ty)
            else {
                return Ok(ty);
            };
            let result = (|| {
                if fixing && !self.inference.contexts[context.index()].infos[index].fixed {
                    if let Some(error) = &self.inference.contexts[context.index()].failure {
                        return Err(error.clone());
                    }
                    sequence.read(
                        self.infer_from_intra_expression_sites_completion(context, consumer),
                    )?;
                    self.clear_cached_inferences(context);
                    self.inference.contexts[context.index()].infos[index].fixed = true;
                }
                sequence.read(self.inferred_type_completion(context, index, consumer))
            })();
            if let Err(error) = &result {
                self.fail_inference_context(context, error.clone());
            }
            result
        })();
        sequence.finish(result)
    }

    /// `clearCachedInferences` preserves fixed slots. Active mapper caches are
    /// a separate native operation and are cleared only by inferred resolution.
    fn clear_cached_inferences(&mut self, context: InferenceContextId) {
        for info in &mut self.inference.contexts[context.index()].infos {
            if !info.fixed {
                info.inferred = None;
            }
        }
    }

    /// `applyToParameterTypes`: counts and both effective rest types precede
    /// raw this types and left-to-right positional inference pairs.
    fn apply_to_parameter_types_completion(
        &mut self,
        source: &Signature,
        target: &Signature,
        consumer: &Location,
        callback: &mut impl FnMut(&mut Self, TypeId, TypeId) -> NativeResult<()>,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let source_count =
                sequence.read(self.signature_parameter_count_completion(source, consumer))?;
            let target_count =
                sequence.read(self.signature_parameter_count_completion(target, consumer))?;
            let source_rest =
                sequence.read(self.signature_effective_rest_completion(source, consumer))?;
            let target_rest =
                sequence.read(self.signature_effective_rest_completion(target, consumer))?;
            let target_non_rest = target_count - usize::from(target_rest.is_some());
            let count = if source_rest.is_some() {
                target_non_rest
            } else {
                source_count.min(target_non_rest)
            };
            if let Some(source_this) = sequence.read(self.signature_this_type_completion(source))? {
                if let Some(target_this) =
                    sequence.read(self.signature_this_type_completion(target))?
                {
                    sequence.read(callback(self, source_this, target_this))?;
                }
            }
            for index in 0..count {
                let source_type = sequence.read(
                    self.signature_type_at_position_or_any_completion(source, index, consumer),
                )?;
                let target_type = sequence.read(
                    self.signature_type_at_position_or_any_completion(target, index, consumer),
                )?;
                sequence.read(callback(self, source_type, target_type))?;
            }
            if let Some(target_rest) = target_rest {
                let mut readonly = sequence.read(self.is_const_type_variable_completion(
                    target_rest,
                    0,
                    consumer,
                ))?;
                if readonly {
                    let types = self
                        .model
                        .union_constituents(target_rest)
                        .map(|types| types.to_vec())
                        .unwrap_or_else(|| vec![target_rest]);
                    for ty in types {
                        if sequence
                            .read(self.is_mutable_array_like_type_completion(ty, consumer))?
                        {
                            readonly = false;
                            break;
                        }
                    }
                }
                let source_rest =
                    sequence.read(self.signature_rest_type_at_position_completion(
                        source, count, readonly, consumer,
                    ))?;
                sequence.read(callback(self, source_rest, target_rest))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `applyToReturnTypes` asks the target predicate and return first. Source
    /// return inference occurs only when the target can contain variables.
    fn apply_to_return_types_completion(
        &mut self,
        source: &Signature,
        target: &Signature,
        consumer: &Location,
        callback: &mut impl FnMut(&mut Self, TypeId, TypeId) -> NativeResult<()>,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            if let Some(target_predicate) =
                sequence.read(self.type_predicate_of_signature_completion(target, consumer))?
            {
                if let Some(source_predicate) =
                    sequence.read(self.type_predicate_of_signature_completion(source, consumer))?
                {
                    if super::signatures::predicate_kinds_match(
                        &source_predicate,
                        &target_predicate,
                    ) {
                        if let (Some(source_type), Some(target_type)) =
                            (source_predicate.value_type, target_predicate.value_type)
                        {
                            let source_type = sequence.read(self.demand_completion(source_type))?;
                            let target_type = sequence.read(self.demand_completion(target_type))?;
                            sequence.read(callback(self, source_type, target_type))?;
                            return Ok(());
                        }
                    }
                }
            }
            let target_type =
                sequence.read(self.get_return_type_of_signature_completion(target, consumer))?;
            if sequence.read(self.could_contain_type_variables_completion(target_type, consumer))? {
                let source_type = sequence
                    .read(self.get_return_type_of_signature_completion(source, consumer))?;
                sequence.read(callback(self, source_type, target_type))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `inferTypes` gives each parameter/return pair its own traversal state;
    /// all passes update the same canonical inference slots.
    fn infer_types_completion(
        &mut self,
        context: InferenceContextId,
        source: TypeId,
        target: TypeId,
        priority: Priority,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = {
            let mut pass = InferencePass {
                context,
                original_target: target,
                priority,
                depth: 0,
                contravariant: false,
                bivariant: false,
                inference_priority: Priority::MAX,
                visited: HashMap::new(),
                source_stack: Vec::new(),
                target_stack: Vec::new(),
                expanding: 0,
            };
            sequence.read(self.infer_from_types_completion(source, target, &mut pass, consumer))
        };
        sequence.finish(result)
    }

    /// `inferFromTypes` first establishes target variable containment; actual
    /// type-variable candidates preserve priority and decreasing depth order.
    fn infer_from_types_completion(
        &mut self,
        mut source: TypeId,
        mut target: TypeId,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            if self.model.is_wildcard_type(source) {
                return Ok(());
            }
            if !sequence.read(self.could_contain_type_variables_completion(target, consumer))? {
                return Ok(());
            }
            let source_alias = self.model.metadata(source)?.alias;
            let target_alias = self.model.metadata(target)?.alias;
            if let (Some(source_alias), Some(target_alias)) = (source_alias, target_alias) {
                if source_alias.symbol == target_alias.symbol {
                    if source_alias
                        .arguments
                        .as_ref()
                        .is_some_and(|args| !args.is_empty())
                        || target_alias
                            .arguments
                            .as_ref()
                            .is_some_and(|args| !args.is_empty())
                    {
                        let parameters = sequence.read(
                            self.alias_parameters_completion(source_alias.symbol, consumer),
                        )?;
                        let source_arguments = sequence.read(
                            self.fill_missing_type_arguments_completion(
                                source_alias
                                    .arguments
                                    .as_ref()
                                    .map(|args| args.to_vec())
                                    .unwrap_or_default(),
                                &parameters,
                                consumer,
                            ),
                        )?;
                        let target_arguments = sequence.read(
                            self.fill_missing_type_arguments_completion(
                                target_alias
                                    .arguments
                                    .as_ref()
                                    .map(|args| args.to_vec())
                                    .unwrap_or_default(),
                                &parameters,
                                consumer,
                            ),
                        )?;
                        let variances = sequence
                            .read(self.alias_variances_completion(source_alias.symbol, consumer))?;
                        sequence.read(self.infer_from_type_arguments_completion(
                            &source_arguments,
                            &target_arguments,
                            &variances,
                            pass,
                            consumer,
                        ))?;
                    }
                    return Ok(());
                }
            }
            if source == target {
                if let Some(types) = self
                    .model
                    .union_constituents(target)
                    .or_else(|| self.model.intersection_constituents(target))
                {
                    for &ty in types.iter() {
                        sequence.read(self.infer_from_types_completion(ty, ty, pass, consumer))?;
                    }
                    return Ok(());
                }
            }
            if let Some(targets) = self.model.union_constituents(target) {
                let sources = self
                    .model
                    .union_constituents(source)
                    .map(|types| types.to_vec())
                    .unwrap_or_else(|| vec![source]);
                let (sources, targets) =
                    sequence.read(self.infer_from_matching_types_completion(
                        sources,
                        targets.to_vec(),
                        InferenceMatch::IdentityOrBase,
                        pass,
                        consumer,
                    ))?;
                let (sources, targets) =
                    sequence.read(self.infer_from_matching_types_completion(
                        sources,
                        targets,
                        InferenceMatch::Close,
                        pass,
                        consumer,
                    ))?;
                if targets.is_empty() {
                    return Ok(());
                }
                target =
                    sequence.read(self.union_type_ex_completion(targets, None, None, consumer))?;
                if sources.is_empty() {
                    return sequence.read(self.infer_with_priority_completion(
                        source,
                        target,
                        Priority::NAKED,
                        pass,
                        consumer,
                    ));
                }
                source =
                    sequence.read(self.union_type_ex_completion(sources, None, None, consumer))?;
            } else if let Some(targets) = self.model.intersection_constituents(target) {
                let mut simple = true;
                for &target in targets.iter() {
                    if !self.model.has_type_flags(target, TypeFlags::OBJECT)
                        || sequence
                            .read(self.is_generic_object_type_completion(target, consumer))?
                    {
                        simple = false;
                        break;
                    }
                }
                if !simple && !self.model.has_type_flags(source, TypeFlags::UNION) {
                    let sources = self
                        .model
                        .intersection_constituents(source)
                        .map(|types| types.to_vec())
                        .unwrap_or_else(|| vec![source]);
                    let (sources, targets) =
                        sequence.read(self.infer_from_matching_types_completion(
                            sources,
                            targets.to_vec(),
                            InferenceMatch::Identity,
                            pass,
                            consumer,
                        ))?;
                    if sources.is_empty() || targets.is_empty() {
                        return Ok(());
                    }
                    source = sequence.read(self.intersection_type_completion(sources, consumer))?;
                    target = sequence.read(self.intersection_type_completion(targets, consumer))?;
                }
            }
            if self
                .model
                .has_type_flags(target, TypeFlags::INDEXED_ACCESS | TypeFlags::SUBSTITUTION)
            {
                if self.model.is_no_infer_type(target) {
                    return Ok(());
                }
                target = sequence.read(self.actual_type_variable_completion(target, consumer))?;
            }
            if self.model.has_type_flags(target, TypeFlags::TYPE_VARIABLE) {
                let context = &self.inference.contexts[pass.context.index()];
                if let Some(index) = context
                    .infos
                    .iter()
                    .position(|info| info.parameter == target)
                {
                    if self
                        .model
                        .object_flags(source)?
                        .intersects(ObjectFlags::NON_INFERRABLE_TYPE)
                        || source == self.model.non_inferrable_any_type()
                    {
                        return Ok(());
                    }
                    let info = &mut self.inference.contexts[pass.context.index()].infos[index];
                    if !info.fixed {
                        let mut changed = false;
                        if pass.priority < info.priority {
                            info.candidates.clear();
                            info.contra_candidates.clear();
                            info.top_level = true;
                            info.priority = pass.priority;
                        }
                        if pass.priority == info.priority {
                            if pass.contravariant && !pass.bivariant {
                                if !info.contra_candidates.contains(&source) {
                                    info.contra_candidates.push(source);
                                    changed = true;
                                }
                            } else {
                                let old = info.candidates.iter().position(|&(ty, _)| ty == source);
                                if old.is_none_or(|index| info.candidates[index].1 < pass.depth) {
                                    if let Some(index) = old {
                                        info.candidates.remove(index);
                                    }
                                    let index = info
                                        .candidates
                                        .iter()
                                        .position(|&(_, depth)| depth < pass.depth)
                                        .unwrap_or(info.candidates.len());
                                    info.candidates.insert(index, (source, pass.depth));
                                    changed = true;
                                }
                            }
                        }
                        if changed {
                            self.clear_cached_inferences(pass.context);
                        }
                        if pass.priority.0 & Priority::RETURN.0 == 0
                            && self.inference.contexts[pass.context.index()].infos[index].top_level
                            && !sequence.read(self.type_parameter_at_top_level_completion(
                                pass.original_target,
                                target,
                                0,
                                consumer,
                            ))?
                        {
                            self.inference.contexts[pass.context.index()].infos[index].top_level =
                                false;
                            self.clear_cached_inferences(pass.context);
                        }
                    }
                    pass.inference_priority = pass.inference_priority.min(pass.priority);
                    return Ok(());
                }
                let simplified =
                    sequence.read(self.simplified_type_completion(target, false, consumer))?;
                if simplified != target {
                    sequence.read(
                        self.infer_from_types_completion(source, simplified, pass, consumer),
                    )?;
                } else if let TypeView::IndexedAccess(access) = self.model.get(target)? {
                    let index = sequence.read(self.simplified_type_completion(
                        access.index_type,
                        false,
                        consumer,
                    ))?;
                    if self.model.has_type_flags(index, TypeFlags::INSTANTIABLE) {
                        let object = sequence.read(self.simplified_type_completion(
                            access.object_type,
                            false,
                            consumer,
                        ))?;
                        if let Some(simplified) =
                            sequence.read(self.distribute_index_over_object_type_completion(
                                object, index, false, consumer,
                            ))?
                            && simplified != target
                        {
                            sequence.read(
                                self.infer_from_types_completion(
                                    source, simplified, pass, consumer,
                                ),
                            )?;
                        }
                    }
                }
            }
            if !(self.model.deferred_reference(source).is_some()
                && self.model.deferred_reference(target).is_some())
                && sequence.read(
                    self.infer_reference_arguments_completion(source, target, pass, consumer),
                )?
            {
                return Ok(());
            }
            if let (TypeView::StringMapping(source), TypeView::StringMapping(target)) =
                (self.model.get(source)?, self.model.get(target)?)
            {
                if source.symbol == target.symbol {
                    sequence.read(self.infer_from_types_completion(
                        source.target,
                        target.target,
                        pass,
                        consumer,
                    ))?;
                }
                return Ok(());
            }
            if let TypeView::Substitution(view) = self.model.get(source)? {
                sequence
                    .read(self.infer_from_types_completion(view.base, target, pass, consumer))?;
                let intersection =
                    sequence.read(self.substitution_intersection_completion(source, consumer))?;
                return sequence.read(self.infer_with_priority_completion(
                    intersection,
                    target,
                    Priority::SUBSTITUTE_SOURCE,
                    pass,
                    consumer,
                ));
            }
            if self.model.has_type_flags(target, TypeFlags::CONDITIONAL) {
                return sequence.read(self.infer_types_once_completion(
                    source,
                    target,
                    InferenceAction::Conditional,
                    pass,
                    consumer,
                ));
            }
            if let Some(types) = self
                .model
                .union_constituents(target)
                .or_else(|| self.model.intersection_constituents(target))
            {
                return sequence.read(self.infer_to_multiple_types_completion(
                    source,
                    &types,
                    self.model.type_flags(target),
                    pass,
                    consumer,
                ));
            }
            if let Some(types) = self.model.union_constituents(source) {
                for &ty in types.iter() {
                    sequence.read(self.infer_from_types_completion(ty, target, pass, consumer))?;
                }
                return Ok(());
            }
            if let TypeView::TemplateLiteral(template) = self.model.get(target)? {
                return sequence
                    .read(self.infer_to_template_completion(source, &template, pass, consumer));
            }
            let source = sequence.read(self.reduced_type_completion(source, consumer))?;
            if sequence.read(self.is_generic_mapped_type_completion(source, consumer))?
                && sequence.read(self.is_generic_mapped_type_completion(target, consumer))?
            {
                sequence.read(self.infer_types_once_completion(
                    source,
                    target,
                    InferenceAction::Mapped,
                    pass,
                    consumer,
                ))?;
            }
            let apparent = if pass.priority.0 & Priority::NO_CONSTRAINTS != 0
                && self
                    .model
                    .has_type_flags(source, TypeFlags::INTERSECTION | TypeFlags::INSTANTIABLE)
            {
                source
            } else {
                sequence.read(self.apparent_type_completion(source, consumer))?
            };
            if apparent != source
                && !self
                    .model
                    .has_type_flags(apparent, TypeFlags::OBJECT | TypeFlags::INTERSECTION)
            {
                return sequence
                    .read(self.infer_from_types_completion(apparent, target, pass, consumer));
            }
            if self
                .model
                .has_type_flags(apparent, TypeFlags::OBJECT | TypeFlags::INTERSECTION)
            {
                return sequence.read(self.infer_types_once_completion(
                    apparent,
                    target,
                    InferenceAction::Objects,
                    pass,
                    consumer,
                ));
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `inferFromMatchingTypes` uses target-major pair order and removes all
    /// matched identities only after every matching pair has inferred.
    fn infer_from_matching_types_completion(
        &mut self,
        mut sources: Vec<TypeId>,
        mut targets: Vec<TypeId>,
        matching: InferenceMatch,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<(Vec<TypeId>, Vec<TypeId>)> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(Vec<TypeId>, Vec<TypeId>), DemandError> {
            let mut matched_sources = Vec::new();
            let mut matched_targets = Vec::new();
            for &target in &targets {
                for &source in &sources {
                    let matches = match matching {
                        InferenceMatch::Identity => sequence.read(self.type_related_completion(
                            source,
                            target,
                            RelationKind::Identity,
                            consumer,
                        ))?,
                        InferenceMatch::IdentityOrBase => {
                            if self.model.is_missing_type(target) {
                                source == target
                            } else {
                                sequence.read(self.type_related_completion(
                                    source,
                                    target,
                                    RelationKind::Identity,
                                    consumer,
                                ))? || self.model.has_type_flags(target, TypeFlags::STRING)
                                    && self.model.has_type_flags(source, TypeFlags::STRING_LITERAL)
                                    || self.model.has_type_flags(target, TypeFlags::NUMBER)
                                        && self
                                            .model
                                            .has_type_flags(source, TypeFlags::NUMBER_LITERAL)
                            }
                        }
                        InferenceMatch::Close => {
                            let object_match = self.model.has_type_flags(source, TypeFlags::OBJECT)
                                && self.model.has_type_flags(target, TypeFlags::OBJECT)
                                && match self.type_symbol_identity(source)? {
                                    Some(symbol) => {
                                        self.type_symbol_identity(target)? == Some(symbol)
                                    }
                                    None => false,
                                };
                            if object_match {
                                true
                            } else {
                                match (
                                    self.model.metadata(source)?.alias,
                                    self.model.metadata(target)?.alias,
                                ) {
                                    (Some(source), Some(target)) => {
                                        source.symbol == target.symbol
                                            && source
                                                .arguments
                                                .as_ref()
                                                .is_some_and(|args| !args.is_empty())
                                    }
                                    _ => false,
                                }
                            }
                        }
                    };
                    if matches {
                        sequence.read(
                            self.infer_from_types_completion(source, target, pass, consumer),
                        )?;
                        if !matched_sources.contains(&source) {
                            matched_sources.push(source);
                        }
                        if !matched_targets.contains(&target) {
                            matched_targets.push(target);
                        }
                    }
                }
            }
            sources.retain(|ty| !matched_sources.contains(ty));
            targets.retain(|ty| !matched_targets.contains(ty));
            Ok((sources, targets))
        })();
        sequence.finish(result)
    }

    fn inference_has_parameter(&self, pass: &InferencePass, ty: TypeId) -> bool {
        self.inference.contexts[pass.context.index()]
            .infos
            .iter()
            .any(|info| info.parameter == ty)
    }

    /// `inferToMultipleTypes` records per-source matched priority before
    /// assigning the unmatched source union to a single naked variable.
    fn infer_to_multiple_types_completion(
        &mut self,
        source: TypeId,
        targets: &[TypeId],
        flags: TypeFlags,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let mut variable_count = 0;
            if flags.intersects(TypeFlags::UNION) {
                let sources = self
                    .model
                    .union_constituents(source)
                    .map(|types| types.to_vec())
                    .unwrap_or_else(|| vec![source]);
                let mut matched = vec![false; sources.len()];
                let mut circularity = false;
                let mut naked = None;
                for &target in targets {
                    if self.inference_has_parameter(pass, target) {
                        naked = Some(target);
                        variable_count += 1;
                    } else {
                        for (index, &source) in sources.iter().enumerate() {
                            let saved = pass.inference_priority;
                            pass.inference_priority = Priority::MAX;
                            let result = sequence.read(
                                self.infer_from_types_completion(source, target, pass, consumer),
                            );
                            matched[index] |= pass.inference_priority == pass.priority;
                            circularity |= pass.inference_priority == Priority::CIRCULARITY;
                            pass.inference_priority = pass.inference_priority.min(saved);
                            result?;
                        }
                    }
                }
                if variable_count == 0 {
                    let mut variable = None;
                    for &target in targets {
                        let Some(types) = self.model.intersection_constituents(target) else {
                            return Ok(());
                        };
                        let Some(&ty) = types
                            .iter()
                            .find(|&&ty| self.inference_has_parameter(pass, ty))
                        else {
                            return Ok(());
                        };
                        if variable.is_some_and(|variable| variable != ty) {
                            return Ok(());
                        }
                        variable = Some(ty);
                    }
                    if let Some(variable) = variable {
                        sequence.read(self.infer_with_priority_completion(
                            source,
                            variable,
                            Priority::NAKED,
                            pass,
                            consumer,
                        ))?;
                    }
                    return Ok(());
                }
                if variable_count == 1 && !circularity {
                    let unmatched: Vec<_> = sources
                        .into_iter()
                        .zip(matched)
                        .filter_map(|(ty, matched)| (!matched).then_some(ty))
                        .collect();
                    if !unmatched.is_empty() {
                        let source = sequence
                            .read(self.union_type_ex_completion(unmatched, None, None, consumer))?;
                        return sequence.read(self.infer_from_types_completion(
                            source,
                            naked.expect("one naked inference parameter"),
                            pass,
                            consumer,
                        ));
                    }
                }
            } else {
                for &target in targets {
                    if self.inference_has_parameter(pass, target) {
                        variable_count += 1;
                    } else {
                        sequence.read(
                            self.infer_from_types_completion(source, target, pass, consumer),
                        )?;
                    }
                }
            }
            if flags.intersects(TypeFlags::INTERSECTION) && variable_count == 1
                || !flags.intersects(TypeFlags::INTERSECTION) && variable_count > 0
            {
                for &target in targets {
                    if self.inference_has_parameter(pass, target) {
                        sequence.read(self.infer_with_priority_completion(
                            source,
                            target,
                            Priority::NAKED,
                            pass,
                            consumer,
                        ))?;
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `invokeOnce` records provisional Circularity before recursion, and
    /// combines observed priorities after restoring both traversal stacks.
    fn infer_types_once_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        action: InferenceAction,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let key = (source, target);
            if let Some(&status) = pass.visited.get(&key) {
                pass.inference_priority = pass.inference_priority.min(status);
                return Ok(());
            }
            pass.visited.insert(key, Priority::CIRCULARITY);
            let saved_priority = pass.inference_priority;
            pass.inference_priority = Priority::MAX;
            let saved_expanding = pass.expanding;
            pass.source_stack.push(source);
            pass.target_stack.push(target);
            let result = (|| {
                if sequence.read(self.deeply_nested_relation_type_completion(
                    source,
                    &pass.source_stack,
                    2,
                    consumer,
                ))? {
                    pass.expanding |= 1;
                }
                if sequence.read(self.deeply_nested_relation_type_completion(
                    target,
                    &pass.target_stack,
                    2,
                    consumer,
                ))? {
                    pass.expanding |= 2;
                }
                if pass.expanding == 3 {
                    pass.inference_priority = Priority::CIRCULARITY;
                    Ok(())
                } else {
                    match action {
                        InferenceAction::Objects => sequence.read(
                            self.infer_from_object_types_completion(source, target, pass, consumer),
                        ),
                        InferenceAction::Conditional => sequence.read(
                            self.infer_to_conditional_completion(source, target, pass, consumer),
                        ),
                        InferenceAction::Mapped => {
                            sequence.read(self.infer_from_generic_mapped_types_completion(
                                source, target, pass, consumer,
                            ))
                        }
                    }
                }
            })();
            pass.target_stack.pop();
            pass.source_stack.pop();
            pass.expanding = saved_expanding;
            pass.visited.insert(key, pass.inference_priority);
            pass.inference_priority = pass.inference_priority.min(saved_priority);
            result
        })();
        sequence.finish(result)
    }

    /// `inferToConditionalType` compares paired headers and branches in order;
    /// an ordinary source instead infers to both branches at native priority.
    fn infer_to_conditional_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let TypeView::Conditional(target_view) = self.model.get(target)? else {
                return Err(
                    super::super::internal("conditional inference lacks target header").into(),
                );
            };
            if let TypeView::Conditional(source_view) = self.model.get(source)? {
                sequence.read(self.infer_from_types_completion(
                    source_view.check_type,
                    target_view.check_type,
                    pass,
                    consumer,
                ))?;
                sequence.read(self.infer_from_types_completion(
                    source_view.extends_type,
                    target_view.extends_type,
                    pass,
                    consumer,
                ))?;
                let source_true =
                    sequence.read(self.conditional_true_type_completion(source, consumer))?;
                let target_true =
                    sequence.read(self.conditional_true_type_completion(target, consumer))?;
                sequence.read(self.infer_from_types_completion(
                    source_true,
                    target_true,
                    pass,
                    consumer,
                ))?;
                let source_false =
                    sequence.read(self.conditional_false_type_completion(source, consumer))?;
                let target_false =
                    sequence.read(self.conditional_false_type_completion(target, consumer))?;
                sequence.read(self.infer_from_types_completion(
                    source_false,
                    target_false,
                    pass,
                    consumer,
                ))
            } else {
                let true_type =
                    sequence.read(self.conditional_true_type_completion(target, consumer))?;
                let false_type =
                    sequence.read(self.conditional_false_type_completion(target, consumer))?;
                let priority = pass.priority;
                if pass.contravariant {
                    pass.priority.0 |= Priority::CONTRAVARIANT_CONDITIONAL.0;
                }
                let result = sequence.read(self.infer_to_multiple_types_completion(
                    source,
                    &[true_type, false_type],
                    self.model.type_flags(target),
                    pass,
                    consumer,
                ));
                pass.priority = priority;
                result
            }
        })();
        sequence.finish(result)
    }

    /// `inferFromObjectTypes` prunes only after native reference/mapped
    /// questions, then visits properties, call, construct and index inventories.
    fn infer_from_object_types_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            if sequence
                .read(self.infer_reference_arguments_completion(source, target, pass, consumer))?
            {
                return Ok(());
            }
            if sequence.read(self.is_generic_mapped_type_completion(source, consumer))?
                && sequence.read(self.is_generic_mapped_type_completion(target, consumer))?
            {
                sequence.read(
                    self.infer_from_generic_mapped_types_completion(source, target, pass, consumer),
                )?;
            }
            if self
                .model
                .object_flags(target)?
                .intersects(ObjectFlags::MAPPED)
                && self.mapped_recipe(target)?.name.is_none()
            {
                let constraint =
                    sequence.read(self.mapped_constraint_completion(target, consumer))?;
                if sequence.read(
                    self.infer_to_mapped_type_completion(
                        source, target, constraint, pass, consumer,
                    ),
                )? {
                    return Ok(());
                }
            }
            if sequence
                .read(self.types_definitely_unrelated_completion(source, target, consumer))?
            {
                return Ok(());
            }
            if sequence.read(self.is_array_or_tuple_type_completion(source))? {
                if self.model.is_tuple_type(target) {
                    return sequence
                        .read(self.infer_to_tuple_completion(source, target, pass, consumer));
                }
                if sequence.read(self.is_array_type_completion(target))? {
                    return sequence.read(
                        self.infer_from_index_types_completion(source, target, pass, consumer),
                    );
                }
            }
            for target_property in
                sequence.read(self.properties_of_object_type_completion(target, consumer))?
            {
                let target_symbol = self.property_symbol(&target_property, consumer)?;
                let name = self.property_lookup_key(target_symbol, consumer)?;
                if let Some(source_property) = sequence
                    .read(self.property_of_type_by_key_completion(source, &name, consumer))?
                {
                    let source_symbol = self.property_symbol(&source_property, consumer)?;
                    let source_type =
                        sequence.read(self.demand_completion(source_property.read_type))?;
                    let source_optional = self.property_symbol_flags(source_symbol)?.optional;
                    let source_type = self
                        .model
                        .remove_missing_type(source_type, source_optional)
                        .map_err(|error| error.at(self.inputs, consumer))?;
                    let target_type =
                        sequence.read(self.demand_completion(target_property.read_type))?;
                    let target_optional = self.property_symbol_flags(target_symbol)?.optional;
                    let target_type = self
                        .model
                        .remove_missing_type(target_type, target_optional)
                        .map_err(|error| error.at(self.inputs, consumer))?;
                    sequence.read(self.infer_from_types_completion(
                        source_type,
                        target_type,
                        pass,
                        consumer,
                    ))?;
                }
            }
            sequence.read(
                self.infer_from_signatures_completion(source, target, false, pass, consumer),
            )?;
            sequence.read(
                self.infer_from_signatures_completion(source, target, true, pass, consumer),
            )?;
            sequence.read(self.infer_from_index_types_completion(source, target, pass, consumer))
        })();
        sequence.finish(result)
    }

    /// Tuple branch of `inferFromObjectTypes`: target arguments precede source
    /// elements; prefix, middle and reversed suffix inferences share one pass.
    fn infer_to_tuple_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            use TupleElementFlags as Flags;
            let source_arity = self.type_reference_arity(source)?;
            let target_arity = self.type_reference_arity(target)?;
            let elements = sequence.read(self.get_type_arguments_completion(target))?;
            let tuple = self
                .model
                .tuple_target(target)?
                .expect("tuple inference target");
            let source_tuple = self.model.tuple_target(source)?;
            if let Some(source_tuple) = &source_tuple {
                let matching = source_arity == target_arity
                    && source_tuple.elements.iter().zip(tuple.elements.iter()).all(
                        |(source, target)| {
                            source.flags.intersects(Flags::REST)
                                == target.flags.intersects(Flags::REST)
                                && source.flags.intersects(Flags::VARIADIC)
                                    == target.flags.intersects(Flags::VARIADIC)
                        },
                    );
                if matching {
                    for index in 0..target_arity {
                        let arguments =
                            sequence.read(self.get_type_arguments_completion(source))?;
                        sequence.read(self.infer_from_types_completion(
                            arguments[index],
                            elements[index],
                            pass,
                            consumer,
                        ))?;
                    }
                    return Ok(());
                }
            }
            let (start, end) = source_tuple.as_ref().map_or((0, 0), |source| {
                (
                    source.fixed_length.min(tuple.fixed_length),
                    if tuple.combined_flags.intersects(Flags::VARIABLE) {
                        source
                            .end_element_count(Flags::FIXED)
                            .min(tuple.end_element_count(Flags::FIXED))
                    } else {
                        0
                    },
                )
            });
            for index in 0..start {
                let arguments = sequence.read(self.get_type_arguments_completion(source))?;
                sequence.read(self.infer_from_types_completion(
                    arguments[index],
                    elements[index],
                    pass,
                    consumer,
                ))?;
            }
            let source_is_rest = source_tuple.as_ref().is_none_or(|source| {
                source_arity.checked_sub(start + end) == Some(1)
                    && source.elements[start].flags.intersects(Flags::REST)
            });
            if source_is_rest {
                let rest = sequence.read(self.get_type_arguments_completion(source))?[start];
                for index in start..target_arity - end {
                    let source = if tuple.elements[index].flags.intersects(Flags::VARIADIC) {
                        sequence.read(self.array_type_completion(rest, consumer))?
                    } else {
                        rest
                    };
                    sequence.read(self.infer_from_types_completion(
                        source,
                        elements[index],
                        pass,
                        consumer,
                    ))?;
                }
            } else {
                let middle = target_arity - start - end;
                if middle == 2 {
                    let first = tuple.elements[start].flags;
                    let second = tuple.elements[start + 1].flags;
                    if first.intersects(Flags::VARIADIC) && second.intersects(Flags::VARIADIC) {
                        let implied = self.inference.contexts[pass.context.index()]
                            .infos
                            .iter()
                            .find(|info| info.parameter == elements[start])
                            .and_then(|info| info.implied_arity);
                        if let Some(implied) = implied {
                            let first = sequence.read(self.slice_tuple_type_completion(
                                source,
                                start,
                                (end + source_arity).saturating_sub(implied),
                                consumer,
                            ))?;
                            sequence.read(self.infer_from_types_completion(
                                first,
                                elements[start],
                                pass,
                                consumer,
                            ))?;
                            let second = sequence.read(self.slice_tuple_type_completion(
                                source,
                                start + implied,
                                end,
                                consumer,
                            ))?;
                            sequence.read(self.infer_from_types_completion(
                                second,
                                elements[start + 1],
                                pass,
                                consumer,
                            ))?;
                        }
                    } else if first.intersects(Flags::VARIADIC) && second.intersects(Flags::REST) {
                        let parameter = self.inference.contexts[pass.context.index()]
                            .infos
                            .iter()
                            .find(|info| info.parameter == elements[start])
                            .map(|info| info.parameter);
                        if let Some(parameter) = parameter
                            && let Some(constraint) = sequence
                                .read(self.base_constraint_completion(parameter, consumer))?
                            && let Some(constraint) = self.model.tuple_target(constraint)?
                            && !constraint.combined_flags.intersects(Flags::VARIABLE)
                        {
                            let implied = constraint.fixed_length;
                            let slice = sequence.read(self.slice_tuple_type_completion(
                                source,
                                start,
                                source_arity.saturating_sub(start + implied),
                                consumer,
                            ))?;
                            sequence.read(self.infer_from_types_completion(
                                slice,
                                elements[start],
                                pass,
                                consumer,
                            ))?;
                            if let Some(rest) = sequence.read(
                                self.get_element_type_of_slice_of_tuple_type_completion(
                                    source,
                                    start + implied,
                                    end,
                                    false,
                                    false,
                                    consumer,
                                ),
                            )? {
                                sequence.read(self.infer_from_types_completion(
                                    rest,
                                    elements[start + 1],
                                    pass,
                                    consumer,
                                ))?;
                            }
                        }
                    } else if first.intersects(Flags::REST) && second.intersects(Flags::VARIADIC) {
                        let parameter = self.inference.contexts[pass.context.index()]
                            .infos
                            .iter()
                            .find(|info| info.parameter == elements[start + 1])
                            .map(|info| info.parameter);
                        if let Some(parameter) = parameter
                            && let Some(constraint) = sequence
                                .read(self.base_constraint_completion(parameter, consumer))?
                            && let Some(constraint) = self.model.tuple_target(constraint)?
                            && !constraint.combined_flags.intersects(Flags::VARIABLE)
                        {
                            let implied = constraint.fixed_length;
                            let end_index =
                                source_arity.saturating_sub(tuple.end_element_count(Flags::FIXED));
                            if let Some(start_index) = end_index
                                .checked_sub(implied)
                                .filter(|&index| index >= start)
                            {
                                let arguments =
                                    sequence.read(self.get_type_arguments_completion(source))?;
                                let source_tuple = source_tuple
                                    .as_ref()
                                    .expect("source tuple after rest exclusion");
                                let trailing =
                                    sequence.read(self.create_tuple_type_ex_completion(
                                        arguments[start_index..end_index].to_vec(),
                                        source_tuple.elements[start_index..end_index].to_vec(),
                                        false,
                                        consumer,
                                    ))?;
                                if let Some(rest) = sequence.read(
                                    self.get_element_type_of_slice_of_tuple_type_completion(
                                        source,
                                        start,
                                        end + implied,
                                        false,
                                        false,
                                        consumer,
                                    ),
                                )? {
                                    sequence.read(self.infer_from_types_completion(
                                        rest,
                                        elements[start],
                                        pass,
                                        consumer,
                                    ))?;
                                }
                                sequence.read(self.infer_from_types_completion(
                                    trailing,
                                    elements[start + 1],
                                    pass,
                                    consumer,
                                ))?;
                            }
                        }
                    }
                } else if middle == 1 && tuple.elements[start].flags.intersects(Flags::VARIADIC) {
                    let priority = if tuple.elements[target_arity - 1]
                        .flags
                        .intersects(Flags::OPTIONAL)
                    {
                        Priority::SPECULATIVE_TUPLE
                    } else {
                        Priority::NONE
                    };
                    let slice = sequence
                        .read(self.slice_tuple_type_completion(source, start, end, consumer))?;
                    sequence.read(self.infer_with_priority_completion(
                        slice,
                        elements[start],
                        priority,
                        pass,
                        consumer,
                    ))?;
                } else if middle == 1 && tuple.elements[start].flags.intersects(Flags::REST) {
                    if let Some(rest) =
                        sequence.read(self.get_element_type_of_slice_of_tuple_type_completion(
                            source, start, end, false, false, consumer,
                        ))?
                    {
                        sequence.read(self.infer_from_types_completion(
                            rest,
                            elements[start],
                            pass,
                            consumer,
                        ))?;
                    }
                }
            }
            for index in 0..end {
                let arguments = sequence.read(self.get_type_arguments_completion(source))?;
                sequence.read(self.infer_from_types_completion(
                    arguments[source_arity - index - 1],
                    elements[target_arity - index - 1],
                    pass,
                    consumer,
                ))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    fn infer_reference_arguments_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if let (TypeView::Reference(source), TypeView::Reference(target)) =
                (self.model.get(source)?, self.model.get(target)?)
            {
                if source.is_reference
                    && target.is_reference
                    && (source.target == target.target
                        || sequence.read(self.is_array_type_completion(source.ty))?
                            && sequence.read(self.is_array_type_completion(target.ty))?)
                {
                    let source_arguments =
                        sequence.read(self.get_type_arguments_completion(source.ty))?;
                    let target_arguments =
                        sequence.read(self.get_type_arguments_completion(target.ty))?;
                    let variances = sequence
                        .read(self.reference_variances_completion(source.target, consumer))?;
                    sequence.read(self.infer_from_type_arguments_completion(
                        &source_arguments,
                        &target_arguments,
                        &variances,
                        pass,
                        consumer,
                    ))?;
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `inferFromTypeArguments` adds one candidate depth and flips only a
    /// measured pure contravariant parameter, restoring both on every exit.
    fn infer_from_type_arguments_completion(
        &mut self,
        source: &[TypeId],
        target: &[TypeId],
        variances: &[VarianceFlags],
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = {
            pass.depth += 1;
            let result = (|| {
                for (index, (&source, &target)) in source.iter().zip(target).enumerate() {
                    let flip = variances.get(index).is_some_and(|&variance| {
                        variance & VarianceFlags::VARIANCE_MASK == VarianceFlags::CONTRAVARIANT
                    });
                    if flip {
                        pass.contravariant = !pass.contravariant;
                    }
                    let result = sequence
                        .read(self.infer_from_types_completion(source, target, pass, consumer));
                    if flip {
                        pass.contravariant = !pass.contravariant;
                    }
                    result?;
                }
                Ok(())
            })();
            pass.depth -= 1;
            result
        };
        sequence.finish(result)
    }

    /// `typesDefinitelyUnrelated` short circuits its reverse unmatched-property
    /// question and only the first direction compares discriminants.
    fn types_definitely_unrelated_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if let Some(source) = self.model.tuple_target(source)?
                && let Some(target) = self.model.tuple_target(target)?
            {
                return Ok(!target
                    .combined_flags
                    .intersects(TupleElementFlags::VARIADIC)
                    && target.minimum_length > source.minimum_length
                    || !target
                        .combined_flags
                        .intersects(TupleElementFlags::VARIABLE)
                        && (source
                            .combined_flags
                            .intersects(TupleElementFlags::VARIABLE)
                            || target.fixed_length < source.fixed_length));
            }
            Ok(sequence
                .read(self.unmatched_property_completion(source, target, false, true, consumer))?
                .is_some()
                && sequence
                    .read(
                        self.unmatched_property_completion(target, source, false, false, consumer),
                    )?
                    .is_some())
        })();
        sequence.finish(result)
    }

    /// `inferFromSignatures` pairs overloads from the bottom while retaining
    /// target order. Source base constraints precede target erasure.
    fn infer_from_signatures_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        construct: bool,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let sources = if construct {
                sequence.read(self.construct_signatures_completion(source, consumer))?
            } else {
                sequence.read(self.call_signatures_completion(source, consumer))?
            };
            if sources.is_empty() {
                return Ok(());
            }
            let targets = if construct {
                sequence.read(self.construct_signatures_completion(target, consumer))?
            } else {
                sequence.read(self.call_signatures_completion(target, consumer))?
            };
            for (index, target) in targets.iter().enumerate() {
                let source_index = (sources.len() + index).saturating_sub(targets.len());
                let source = sequence
                    .read(self.base_signature_completion(&sources[source_index], consumer))?;
                let target = sequence.read(self.erased_signature_completion(target, consumer))?;
                sequence
                    .read(self.infer_from_signature_completion(&source, &target, pass, consumer))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `inferFromSignature`: method/constructor bivariance is sticky only
    /// through parameter traversal; returns restore the caller's variance.
    fn infer_from_signature_completion(
        &mut self,
        source: &Signature,
        target: &Signature,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            if !source.flags.is_non_inferrable() {
                let saved_bivariant = pass.bivariant;
                let kind = self
                    .signature_declaration_root(target.id)?
                    .map(|root| root.site().kind);
                pass.bivariant |= matches!(
                    kind,
                    Some(
                        crate::syntax_kind::SyntaxKind::MethodDeclaration
                            | crate::syntax_kind::SyntaxKind::MethodSignature
                            | crate::syntax_kind::SyntaxKind::Constructor
                    )
                );
                let result = sequence.read(self.apply_to_parameter_types_completion(
                    source,
                    target,
                    consumer,
                    &mut |this, source, target| {
                        let flip = this.inputs.semantic_options().strict_function_types
                            || pass.priority.0 & Priority::ALWAYS_STRICT != 0;
                        if flip {
                            pass.contravariant = !pass.contravariant;
                        }
                        let result =
                            this.infer_from_types_completion(source, target, pass, consumer);
                        if flip {
                            pass.contravariant = !pass.contravariant;
                        }
                        result
                    },
                ));
                pass.bivariant = saved_bivariant;
                result?;
            }
            sequence.read(self.apply_to_return_types_completion(
                source,
                target,
                consumer,
                &mut |this, source, target| {
                    this.infer_from_types_completion(source, target, pass, consumer)
                },
            ))
        })();
        sequence.finish(result)
    }

    /// `inferFromIndexTypes` first combines all applicable property/index
    /// values, then performs the separate applicable-index inference pass.
    fn infer_from_index_types_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let priority = if self
                .model
                .object_flags(source)?
                .intersects(ObjectFlags::MAPPED)
                && self
                    .model
                    .object_flags(target)?
                    .intersects(ObjectFlags::MAPPED)
            {
                Priority(1 << 3)
            } else {
                Priority::NONE
            };
            let indexes = sequence.read(self.index_infos_of_type_completion(target, consumer))?;
            if sequence.read(self.object_type_with_inferable_index_completion(source, consumer))? {
                for target in &indexes {
                    let target_key = sequence.read(self.demand_completion(target.key_type))?;
                    let mut candidates = Vec::new();
                    for property in
                        sequence.read(self.properties_of_type_completion(source, consumer))?
                    {
                        let symbol = self.property_symbol(&property, consumer)?;
                        let key = sequence.read(self.literal_type_from_property_completion(
                            symbol,
                            TypeFlags::STRING_LITERAL
                                | TypeFlags::NUMBER_LITERAL
                                | TypeFlags::UNIQUE_SYMBOL,
                            false,
                            consumer,
                        ))?;
                        if sequence.read(
                            self.is_applicable_index_type_completion(key, target_key, consumer),
                        )? {
                            let mut ty =
                                sequence.read(self.demand_completion(property.read_type))?;
                            if self.property_symbol_flags(symbol)?.optional {
                                ty = sequence.read(
                                    self.remove_missing_or_undefined_type_completion(ty, consumer),
                                )?;
                            }
                            candidates.push(ty);
                        }
                    }
                    for source in
                        sequence.read(self.index_infos_of_type_completion(source, consumer))?
                    {
                        let source_key = sequence.read(self.demand_completion(source.key_type))?;
                        if sequence.read(self.is_applicable_index_type_completion(
                            source_key, target_key, consumer,
                        ))? {
                            candidates
                                .push(sequence.read(self.demand_completion(source.value_type))?);
                        }
                    }
                    if !candidates.is_empty() {
                        let source = sequence.read(
                            self.union_type_ex_completion(candidates, None, None, consumer),
                        )?;
                        let target = sequence.read(self.demand_completion(target.value_type))?;
                        sequence.read(self.infer_with_priority_completion(
                            source, target, priority, pass, consumer,
                        ))?;
                    }
                }
            }
            for target in indexes {
                let target_key = sequence.read(self.demand_completion(target.key_type))?;
                if let Some(source) = sequence
                    .read(self.applicable_index_info_completion(source, target_key, consumer))?
                {
                    let source_type = sequence.read(self.demand_completion(source.value_type))?;
                    let target_type = sequence.read(self.demand_completion(target.value_type))?;
                    sequence.read(self.infer_with_priority_completion(
                        source_type,
                        target_type,
                        priority,
                        pass,
                        consumer,
                    ))?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    fn infer_with_priority_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        priority: Priority,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = {
            let saved = pass.priority;
            pass.priority.0 |= priority.0;
            let result =
                sequence.read(self.infer_from_types_completion(source, target, pass, consumer));
            pass.priority = saved;
            result
        };
        sequence.finish(result)
    }

    /// `getInferredType` installs its provisional value before constraint
    /// substitution can reenter a non-fixing mapper for the same slot.
    fn inferred_type_completion(
        &mut self,
        context: InferenceContextId,
        index: usize,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(result) = &self.inference.contexts[context.index()].infos[index].inferred {
            return result.clone();
        }
        if let Some(error) = &self.inference.contexts[context.index()].failure {
            return Err(error.clone());
        }
        let parameter = self.inference.contexts[context.index()].infos[index].parameter;
        if parameter == self.model.native_error_type() {
            return Ok(NativeCompletion::clean(parameter));
        }
        let candidates = self.inferred_candidate_completion(context, index, consumer);
        let result = match candidates {
            Ok(candidates) => {
                // Native AnyDefault is the JavaScript call branch. Project
                // admission is TS-only, so the absent inference is Unknown.
                let unknown = self.model.special(SpecialName::Unknown);
                let provisional = candidates
                    .clone()
                    .map(|(inferred, _)| inferred.unwrap_or(unknown));
                self.inference.contexts[context.index()].infos[index].inferred =
                    Some(Ok(provisional));
                candidates.and_then(|(inferred, fallback)| {
                    self.constrain_inferred_type_completion(
                        context, index, inferred, fallback, consumer,
                    )
                })
            }
            Err(error) => Err(error),
        };
        match &result {
            Ok(completed) => {
                if completed.value().1 {
                    self.inference.contexts[context.index()].infos[index].inferred =
                        Some(Ok(completed.clone().map(|(ty, _)| ty)));
                }
                self.clear_active_mapper_caches();
            }
            Err(error) => self.fail_inference_context(context, error.clone()),
        }
        result.map(|completed| completed.map(|(ty, _)| ty))
    }

    fn inferred_candidate_completion(
        &mut self,
        context: InferenceContextId,
        index: usize,
        consumer: &Location,
    ) -> NativeResult<(Option<TypeId>, Option<TypeId>)> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(Option<TypeId>, Option<TypeId>), DemandError> {
            let state = &self.inference.contexts[context.index()];
            let parameter = state.infos[index].parameter;
            let signature = state.signature.clone();
            let candidates: Vec<_> = state.infos[index]
                .candidates
                .iter()
                .map(|&(ty, _)| ty)
                .collect();
            let contra = state.infos[index].contra_candidates.clone();
            let priority = state.infos[index].priority;
            let mapper = state.non_fixing_mapper;
            let mut inferred = None;
            let mut fallback = None;
            if let Some(signature) = signature {
                let covariant = if candidates.is_empty() {
                    None
                } else {
                    Some(sequence.read(self.covariant_inference_completion(
                        context, index, &signature, candidates, consumer,
                    ))?)
                };
                let contravariant = if contra.is_empty() {
                    None
                } else {
                    Some(sequence.read(
                        self.contravariant_inference_completion(&contra, priority, consumer),
                    )?)
                };
                if covariant.is_some() || contravariant.is_some() {
                    let prefer = if let Some(covariant) = covariant {
                        if contravariant.is_none() {
                            true
                        } else if self
                            .model
                            .has_type_flags(covariant, TypeFlags::NEVER | TypeFlags::ANY)
                        {
                            false
                        } else {
                            let mut some = false;
                            for &ty in &contra {
                                if sequence.read(self.type_related_completion(
                                    covariant,
                                    ty,
                                    RelationKind::Assignable,
                                    consumer,
                                ))? {
                                    some = true;
                                    break;
                                }
                            }
                            if some {
                                for other in 0..self.inference.contexts[context.index()].infos.len()
                                {
                                    let other_parameter = self.inference.contexts[context.index()]
                                        .infos[other]
                                        .parameter;
                                    if other != index
                                        && sequence.read(self.parameter_constraint_completion(
                                            other_parameter,
                                            consumer,
                                        ))? != Some(parameter)
                                    {
                                        continue;
                                    }
                                    let types = self.inference.contexts[context.index()].infos
                                        [other]
                                        .candidates
                                        .clone();
                                    for (ty, _) in types {
                                        if !sequence.read(self.type_related_completion(
                                            ty,
                                            covariant,
                                            RelationKind::Assignable,
                                            consumer,
                                        ))? {
                                            some = false;
                                            break;
                                        }
                                    }
                                    if !some {
                                        break;
                                    }
                                }
                            }
                            some
                        }
                    } else {
                        false
                    };
                    if prefer {
                        inferred = covariant;
                        fallback = contravariant;
                    } else {
                        inferred = contravariant;
                        fallback = covariant;
                    }
                } else if self.inference.contexts[context.index()].no_default {
                    inferred = Some(self.model.silent_never());
                } else {
                    if let Some(default) =
                        sequence.read(self.default_from_type_parameter_completion(parameter))?
                    {
                        let forward = self.inference.contexts[context.index()].infos[index..]
                            .iter()
                            .map(|info| info.parameter)
                            .collect();
                        let unknown = self.model.special(SpecialName::Unknown);
                        let backreference = self.model.array_to_single_mapper(forward, unknown);
                        let merged = self.model.merge_mappers(backreference, mapper);
                        inferred =
                            Some(sequence.read(
                                self.instantiate_type_completion(default, merged, consumer),
                            )?);
                    }
                }
            } else {
                inferred =
                    sequence.read(self.type_from_inference_completion(context, index, consumer))?;
            }
            Ok((inferred, fallback))
        })();
        sequence.finish(result)
    }

    /// Constraint adjustment runs after the actual provisional property is
    /// published. Its completed evidence extends this invocation's prefix;
    /// installing the final result never rereads that prefix from the cache.
    /// The bool records native's final assignment: a nil constraint returns its
    /// invocation evidence but leaves the earlier provisional slot untouched.
    fn constrain_inferred_type_completion(
        &mut self,
        context: InferenceContextId,
        index: usize,
        mut inferred: Option<TypeId>,
        fallback: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<(TypeId, bool)> {
        let parameter = self.inference.contexts[context.index()].infos[index].parameter;
        let priority = self.inference.contexts[context.index()].infos[index].priority;
        let mapper = self.inference.contexts[context.index()].non_fixing_mapper;
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(TypeId, bool), DemandError> {
            let constraint =
                sequence.read(self.parameter_constraint_completion(parameter, consumer))?;
            let assign_final = constraint.is_some();
            if let Some(constraint) = constraint {
                let instantiated = sequence
                    .read(self.instantiate_type_completion(constraint, mapper, consumer))?;
                if let Some(candidate) = inferred {
                    let with_this = sequence.read(self.get_type_with_this_argument_completion(
                        instantiated,
                        Some(candidate),
                        false,
                        consumer,
                    ))?;
                    if !sequence.read(
                        self.inference_compare_completion(context, candidate, with_this, consumer),
                    )? {
                        inferred = None;
                        if priority == Priority::RETURN {
                            let filtered = sequence.read(self.map_type_completion(
                                candidate,
                                consumer,
                                |checker, ty| {
                                    Ok(checker
                                        .inference_compare_completion(
                                            context, ty, with_this, consumer,
                                        )?
                                        .map(|related| {
                                            if related {
                                                ty
                                            } else {
                                                checker.model.special(SpecialName::Never)
                                            }
                                        }))
                                },
                            ))?;
                            if !self.model.has_type_flags(filtered, TypeFlags::NEVER) {
                                inferred = Some(filtered);
                            }
                        }
                    }
                }
                if inferred.is_none() {
                    if let Some(fallback) = fallback {
                        let with_this =
                            sequence.read(self.get_type_with_this_argument_completion(
                                instantiated,
                                Some(fallback),
                                false,
                                consumer,
                            ))?;
                        if sequence.read(
                            self.inference_compare_completion(
                                context, fallback, with_this, consumer,
                            ),
                        )? {
                            inferred = Some(fallback);
                        }
                    }
                    if inferred.is_none() {
                        inferred = Some(instantiated);
                    }
                }
            }
            Ok((
                inferred.unwrap_or_else(|| self.model.special(SpecialName::Unknown)),
                assign_final,
            ))
        })();
        sequence.finish(result)
    }

    fn inference_compare_completion(
        &mut self,
        context: InferenceContextId,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = match self.inference.contexts[context.index()].comparer {
            Some(comparer) => sequence.read(
                self.inference_constraint_related_completion(source, target, comparer, consumer),
            ),
            None => sequence.read(self.type_related_completion(
                source,
                target,
                RelationKind::Assignable,
                consumer,
            )),
        };
        sequence.finish(result)
    }

    /// `getCovariantInference` keeps primitive/const constraints, top-level
    /// return occurrences and native union/common-supertype choices distinct.
    fn covariant_inference_completion(
        &mut self,
        context: InferenceContextId,
        index: usize,
        signature: &Signature,
        mut candidates: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if candidates.len() > 1 {
                let mut literals = Vec::new();
                let mut other = Vec::new();
                for ty in candidates {
                    if self
                        .model
                        .object_flags(ty)?
                        .intersects(ObjectFlags::OBJECT_LITERAL | ObjectFlags::ARRAY_LITERAL)
                    {
                        literals.push(ty);
                    } else {
                        other.push(ty);
                    }
                }
                if !literals.is_empty() {
                    other.push(sequence.read(self.inference_union_completion(literals, consumer))?);
                }
                candidates = other;
            }
            let state = &self.inference.contexts[context.index()].infos[index];
            let parameter = state.parameter;
            let priority = state.priority;
            let top_level = state.top_level;
            let fixed = state.fixed;
            let primitive = sequence
                .read(self.has_primitive_constraint_completion(parameter, consumer))?
                || sequence.read(self.is_const_type_variable_completion(parameter, 0, consumer))?;
            let widen = !primitive
                && top_level
                && (fixed
                    || !sequence.read(self.parameter_at_top_level_in_return_completion(
                        signature, parameter, consumer,
                    ))?);
            let mut widened = Vec::with_capacity(candidates.len());
            for ty in candidates {
                widened.push(if primitive {
                    self.model.regular_literal(ty)
                } else if widen {
                    sequence.read(self.widened_literal_type_completion(ty, consumer))?
                } else {
                    ty
                });
            }
            let candidates = widened;
            let inferred = if priority.0 & Priority::COMBINATION != 0 {
                sequence.read(self.inference_union_completion(candidates, consumer))?
            } else {
                sequence.read(self.common_supertype_completion(candidates, consumer))?
            };
            self.widened_type(inferred, consumer)
        })();
        sequence.finish(result)
    }

    fn contravariant_inference_completion(
        &mut self,
        candidates: &[TypeId],
        priority: Priority,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if priority.0 & Priority::COMBINATION != 0 {
                return sequence
                    .read(self.intersection_type_completion(candidates.to_vec(), consumer));
            }
            let mut subtype = candidates[0];
            for &ty in &candidates[1..] {
                if sequence.read(self.type_related_completion(
                    ty,
                    subtype,
                    RelationKind::Subtype,
                    consumer,
                ))? {
                    subtype = ty;
                }
            }
            Ok(subtype)
        })();
        sequence.finish(result)
    }

    /// `getTypeFromInference` selects covariance before contravariance and
    /// leaves the absence of candidates as native nil.
    fn type_from_inference_completion(
        &mut self,
        context: InferenceContextId,
        index: usize,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let info = &self.inference.contexts[context.index()].infos[index];
            let candidates = info
                .candidates
                .iter()
                .map(|&(ty, _)| ty)
                .collect::<Vec<_>>();
            if !candidates.is_empty() {
                return sequence
                    .read(self.inference_union_completion(candidates, consumer))
                    .map(Some);
            }
            let contra = info.contra_candidates.clone();
            if !contra.is_empty() {
                return sequence
                    .read(self.intersection_type_completion(contra, consumer))
                    .map(Some);
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    fn inference_union_completion(
        &mut self,
        types: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = sequence.read(self.union_type_reduced_completion(
            types,
            UnionReduction::Subtype,
            None,
            None,
            consumer,
        ));
        sequence.finish(result)
    }

    /// `getCommonSupertype` strips nullable constituents for candidate choice
    /// and restores the exact nullable flags after both native subtype passes.
    fn common_supertype_completion(
        &mut self,
        types: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if types.len() == 1 {
                return Ok(types[0]);
            }
            let mut primary = Vec::new();
            let mut had_null = false;
            let mut had_undefined = false;
            for &ty in &types {
                if self.inputs.semantic_options().strict_null_checks {
                    let parts = self
                        .model
                        .union_constituents(ty)
                        .map(|types| types.to_vec())
                        .unwrap_or_else(|| vec![ty]);
                    for part in parts {
                        if self.model.has_type_flags(part, TypeFlags::NULLABLE) {
                            had_null |= self.model.has_type_flags(part, TypeFlags::NULL);
                            had_undefined |= self.model.has_type_flags(part, TypeFlags::UNDEFINED);
                        }
                    }
                    primary.push(self.model.filter_type(ty, |model, part| {
                        !model.has_type_flags(part, TypeFlags::NULLABLE)
                    }));
                } else {
                    primary.push(ty);
                }
            }
            let mut same_base = true;
            let mut base = None;
            for &ty in &primary {
                if self.model.has_type_flags(ty, TypeFlags::NEVER) {
                    continue;
                }
                let current = self.base_literal_type(ty, consumer)?;
                if base.is_none() {
                    base = Some(current);
                }
                if current == ty || base != Some(current) {
                    same_base = false;
                    break;
                }
            }
            let supertype = if same_base {
                sequence.read(self.union_type_ex_completion(primary, None, None, consumer))?
            } else {
                let strict = sequence.read(self.leftmost_supertype_completion(
                    &primary,
                    RelationKind::StrictSubtype,
                    consumer,
                ))?;
                let mut every = true;
                for &ty in &primary {
                    if ty != strict
                        && !sequence.read(self.type_related_completion(
                            ty,
                            strict,
                            RelationKind::StrictSubtype,
                            consumer,
                        ))?
                    {
                        every = false;
                        break;
                    }
                }
                if every {
                    strict
                } else {
                    sequence.read(self.leftmost_supertype_completion(
                        &primary,
                        RelationKind::Subtype,
                        consumer,
                    ))?
                }
            };
            if !had_null && !had_undefined {
                return Ok(supertype);
            }
            let mut nullable = vec![supertype];
            if had_null {
                nullable.push(self.model.special(SpecialName::Null));
            }
            if had_undefined {
                nullable.push(self.model.special(SpecialName::Undefined));
            }
            sequence.read(self.union_type_ex_completion(nullable, None, None, consumer))
        })();
        sequence.finish(result)
    }

    fn leftmost_supertype_completion(
        &mut self,
        types: &[TypeId],
        relation: RelationKind,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let mut candidate = types[0];
            for &ty in &types[1..] {
                if sequence.read(self.type_related_completion(candidate, ty, relation, consumer))? {
                    candidate = ty;
                }
            }
            Ok(candidate)
        })();
        sequence.finish(result)
    }

    /// `hasPrimitiveConstraint` uses maybeTypeOfKind over a union's flags;
    /// this question is separate from whether the parameter itself is Const.
    fn has_primitive_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let Some(mut constraint) =
                sequence.read(self.parameter_constraint_completion(ty, consumer))?
            else {
                return Ok(false);
            };
            if self
                .model
                .has_type_flags(constraint, TypeFlags::CONDITIONAL)
            {
                constraint = sequence
                    .read(self.default_conditional_constraint_completion(constraint, consumer))?;
            }
            Ok(self.model.maybe_type_of_kind(
                constraint,
                TypeFlags::PRIMITIVE
                    | TypeFlags::INDEX
                    | TypeFlags::TEMPLATE_LITERAL
                    | TypeFlags::STRING_MAPPING,
            ))
        })();
        sequence.finish(result)
    }

    /// `isTypeParameterAtTopLevel` recurses composites without increasing the
    /// conditional depth, whose own true/false branches have a bound of three.
    fn type_parameter_at_top_level_completion(
        &mut self,
        ty: TypeId,
        parameter: TypeId,
        depth: usize,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if ty == parameter {
                return Ok(true);
            }
            if let Some(types) = self
                .model
                .union_constituents(ty)
                .or_else(|| self.model.intersection_constituents(ty))
            {
                for &ty in types.iter() {
                    if sequence.read(
                        self.type_parameter_at_top_level_completion(ty, parameter, depth, consumer),
                    )? {
                        return Ok(true);
                    }
                }
            }
            if depth < 3 && self.model.has_type_flags(ty, TypeFlags::CONDITIONAL) {
                let true_type =
                    sequence.read(self.conditional_true_type_completion(ty, consumer))?;
                if sequence.read(self.type_parameter_at_top_level_completion(
                    true_type,
                    parameter,
                    depth + 1,
                    consumer,
                ))? {
                    return Ok(true);
                }
                let false_type =
                    sequence.read(self.conditional_false_type_completion(ty, consumer))?;
                return sequence.read(self.type_parameter_at_top_level_completion(
                    false_type,
                    parameter,
                    depth + 1,
                    consumer,
                ));
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    fn parameter_at_top_level_in_return_completion(
        &mut self,
        signature: &Signature,
        parameter: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if let Some(predicate) = self.type_predicate_of_signature(signature, consumer)? {
                return match predicate.value_type {
                    Some(ty) => {
                        let ty = sequence.read(self.demand_completion(ty))?;
                        sequence.read(
                            self.type_parameter_at_top_level_completion(ty, parameter, 0, consumer),
                        )
                    }
                    None => Ok(false),
                };
            }
            let ty =
                sequence.read(self.get_return_type_of_signature_completion(signature, consumer))?;
            sequence.read(self.type_parameter_at_top_level_completion(ty, parameter, 0, consumer))
        })();
        sequence.finish(result)
    }

    /// `isConstTypeVariable` observes retained parameter modifiers, and shares
    /// the native five-level indexed/conditional bound across composite walks.
    pub(super) fn is_const_type_variable(
        &mut self,
        ty: TypeId,
        depth: usize,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_const_type_variable_completion(ty, depth, consumer)?
            .admit()
    }

    pub(super) fn is_const_type_variable_completion(
        &mut self,
        ty: TypeId,
        depth: usize,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if depth >= 5 {
                return Ok(false);
            }
            if let TypeView::TypeParameter(parameter) = self.model.get(ty)? {
                return Ok(parameter.constant);
            }
            if let Some(types) = self
                .model
                .union_constituents(ty)
                .or_else(|| self.model.intersection_constituents(ty))
            {
                for &ty in types.iter() {
                    if sequence.read(self.is_const_type_variable_completion(ty, depth, consumer))? {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            if let TypeView::IndexedAccess(access) = self.model.get(ty)? {
                return sequence.read(self.is_const_type_variable_completion(
                    access.object_type,
                    depth + 1,
                    consumer,
                ));
            }
            if let TypeView::Conditional(_) = self.model.get(ty)? {
                return match sequence.read(self.constraint_of_type_completion(ty, consumer))? {
                    Some(constraint) => sequence.read(self.is_const_type_variable_completion(
                        constraint,
                        depth + 1,
                        consumer,
                    )),
                    None => Ok(false),
                };
            }
            if let TypeView::Substitution(view) = self.model.get(ty)? {
                return sequence
                    .read(self.is_const_type_variable_completion(view.base, depth, consumer));
            }
            if self.model.object_flags(ty)?.intersects(ObjectFlags::MAPPED) {
                return match sequence
                    .read(self.homomorphic_type_variable_completion(ty, consumer))?
                {
                    Some(variable) => sequence
                        .read(self.is_const_type_variable_completion(variable, depth, consumer)),
                    None => Ok(false),
                };
            }
            if self.model.is_generic_tuple_type(ty)? {
                let elements = sequence.read(self.get_element_types_completion(ty))?;
                let tuple = self.model.tuple_target(ty)?.expect("generic tuple target");
                for (&element, info) in elements.iter().zip(tuple.elements.iter()) {
                    if info
                        .flags
                        .intersects(super::super::model::TupleElementFlags::VARIADIC)
                        && sequence.read(
                            self.is_const_type_variable_completion(element, depth, consumer),
                        )?
                    {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    fn inference_pending(&self, consumer: &Location, message: &str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }
}
