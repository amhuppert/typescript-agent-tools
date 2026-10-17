//! Contextual function signatures and return expressions share the native
//! signature comparison, annotation and resolution owners.

use super::*;
use crate::project::semantic::evaluation::relations::SignatureComparison;
use crate::project::semantic::model::{NativeResult, NativeSequence, Signature};
use crate::project::semantic::source::SignatureRoot;

impl Evaluator<'_, '_> {
    /// `getContextualTypeForReturnExpression` obtains the containing
    /// function's contextual return before asking generator/async unwrapping.
    pub(in crate::project::semantic::evaluation) fn contextual_return_expression_type_completion(
        &mut self,
        function: SignatureRoot,
        flags: ContextFlags,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let Some(ty) =
                sequence.read(self.contextual_return_type_completion(function, flags, consumer))?
            else {
                return Ok(None);
            };
            let (_, function_flags, _) = self.source.contextual_function_facts(function)?;
            if function_flags.is_generator() {
                return Err(self.contextual_return_dependency(consumer));
            }
            if function_flags.is_async() {
                let awaited = sequence.read(self.map_type_optional_completion(
                    ty,
                    false,
                    consumer,
                    |checker, ty| checker.awaited_type_no_alias_completion(ty, consumer),
                ))?;
                let Some(awaited) = awaited else {
                    return Ok(None);
                };
                let promise =
                    sequence.read(self.create_promise_like_type_completion(awaited, consumer))?;
                return sequence
                    .read(self.union_type_ex_completion(
                        vec![awaited, promise],
                        None,
                        None,
                        consumer,
                    ))
                    .map(Some);
            }
            Ok(Some(ty))
        })();
        sequence.finish(result)
    }

    /// `getContextualReturnType`: written annotation first, then an actual
    /// contextual signature whose return is not resolving, then the IIFE's
    /// context. Ordinary function declarations never acquire contextual calls.
    pub(in crate::project::semantic::evaluation) fn contextual_return_type_completion(
        &mut self,
        function: SignatureRoot,
        flags: ContextFlags,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let recipe = self.source.declaration_return_recipe(function)?;
            if let Some(ty) = sequence.read(self.return_type_from_annotation_completion(&recipe))? {
                return Ok(Some(ty));
            }
            if let Some(signature) = sequence
                .read(self.contextual_signature_for_function_completion(function, consumer))?
                && !self.is_resolving_signature_return(&signature)
            {
                let ty = sequence
                    .read(self.get_return_type_of_signature_completion(&signature, consumer))?;
                let (_, function_flags, _) = self.source.contextual_function_facts(function)?;
                if (function_flags.is_generator() || function_flags.is_async())
                    && !self.model.has_type_flags(ty, TypeFlags::NEVER)
                {
                    return sequence
                        .read(self.filter_type_completion(ty, |checker, candidate| {
                            if checker.model.has_type_flags(
                                candidate,
                                TypeFlags::ANY
                                    | TypeFlags::UNKNOWN
                                    | TypeFlags::VOID
                                    | TypeFlags::INSTANTIABLE_NON_PRIMITIVE,
                            ) {
                                return Ok(
                                    crate::project::semantic::model::NativeCompletion::clean(true),
                                );
                            }
                            if function_flags.is_generator() {
                                return Err(checker.contextual_return_dependency(consumer));
                            }
                            checker
                                .awaited_type_of_promise_completion(candidate, consumer)
                                .map(|awaited| awaited.map(|awaited| awaited.is_some()))
                        }))
                        .map(Some);
                }
                return Ok(Some(ty));
            }
            let (_, _, iife) = self.source.contextual_function_facts(function)?;
            match iife {
                Some(iife) => sequence.read(self.contextual_type_completion(iife, flags)),
                None => Ok(None),
            }
        })();
        sequence.finish(result)
    }

    fn contextual_return_dependency(&self, consumer: &Location) -> DemandError {
        super::super::super::unsupported_demand_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::UnsupportedSemanticForm,
            "native generator contextual return acquisition is pending",
        )
    }

    /// `unwrapReturnType`: async return contexts adopt through the no-alias
    /// worker. Only its actual nil result becomes errorType here.
    pub(in crate::project::semantic::evaluation) fn unwrap_return_type_completion(
        &mut self,
        ty: TypeId,
        flags: crate::syntax::FunctionFlags,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if flags.is_generator() {
            return Err(self.contextual_return_dependency(consumer));
        }
        if !flags.is_async() {
            return Ok(crate::project::semantic::model::NativeCompletion::clean(ty));
        }
        self.awaited_type_no_alias_completion(ty, consumer)?
            .and_then(|awaited| match awaited {
                Some(ty) => Ok(crate::project::semantic::model::NativeCompletion::clean(ty)),
                None => crate::project::semantic::model::NativeCompletion::recovered(
                    self.model.native_error_type(),
                    super::super::super::invalid_at_location(
                        self.inputs,
                        consumer,
                        SemanticFailureReason::ErrorType,
                        "native async return context consumes errorType for an invalid thenable",
                    )
                    .into(),
                ),
            })
    }

    /// `getContextualSignatureForFunctionLikeDeclaration` admits only actual
    /// function expressions, arrow functions and object-literal methods.
    pub(in crate::project::semantic::evaluation) fn contextual_signature_for_function_completion(
        &mut self,
        function: SignatureRoot,
        consumer: &Location,
    ) -> NativeResult<Option<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Signature>, DemandError> {
            let (contextual, _, _) = self.source.contextual_function_facts(function)?;
            if !contextual {
                return Ok(None);
            }
            sequence.read(self.contextual_signature_completion(
                ExpressionRoot {
                    file: function.file,
                    site: function.site(),
                },
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `getContextualSignature` ignores absent union constituent signatures;
    /// present signatures must be identical except for this and return types.
    pub(in crate::project::semantic::evaluation) fn contextual_signature(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> Result<Option<Signature>, DemandError> {
        self.contextual_signature_completion(root, consumer)?
            .admit()
    }

    pub(in crate::project::semantic::evaluation) fn contextual_signature_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<Option<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Signature>, DemandError> {
            let Some(ty) = sequence.read(self.apparent_contextual_type_completion(
                root,
                ContextFlags::SIGNATURE,
                consumer,
            ))?
            else {
                return Ok(None);
            };
            let Some(types) = self.model.union_constituents(ty) else {
                return sequence.read(self.contextual_call_signature(ty, root, consumer));
            };
            let mut signatures = Vec::new();
            for &ty in types.iter() {
                if let Some(signature) =
                    sequence.read(self.contextual_call_signature(ty, root, consumer))?
                {
                    if let Some(first) = signatures.first()
                        && !sequence.read(self.compare_signatures_identical_completion(
                            first,
                            &signature,
                            SignatureComparison {
                                partial_match: false,
                                ignore_this_types: true,
                                ignore_return_types: true,
                            },
                            consumer,
                        ))?
                    {
                        return Ok(None);
                    }
                    signatures.push(signature);
                }
            }
            match signatures.as_slice() {
                [] => Ok(None),
                [signature] => Ok(Some(signature.clone())),
                [first, ..] => self
                    .create_union_signature(&first.clone(), signatures, consumer)
                    .map(Some),
            }
        })();
        sequence.finish(result)
    }

    /// `getContextualCallSignature` / `isAritySmaller` filter actual call
    /// signatures by required syntax arity before optional intersection.
    fn contextual_call_signature(
        &mut self,
        ty: TypeId,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<Option<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Signature>, DemandError> {
            let signatures = sequence.read(self.call_signatures_completion(ty, consumer))?;
            let mut applicable = Vec::new();
            for signature in signatures {
                let required = self.source.contextual_function_parameter_count(root)?;
                if sequence
                    .read(self.signature_has_effective_rest_completion(&signature, consumer))?
                    || sequence
                        .read(self.signature_parameter_count_completion(&signature, consumer))?
                        as isize
                        >= required
                {
                    applicable.push(signature);
                }
            }
            if applicable.len() == 1 {
                return Ok(applicable.pop());
            }
            sequence.read(self.intersected_contextual_signatures(applicable, consumer))
        })();
        sequence.finish(result)
    }

    /// `getIntersectedSignatures` is enabled by noImplicitAny. Its native
    /// parameter-identity comparison precedes each intersection factory.
    fn intersected_contextual_signatures(
        &mut self,
        signatures: Vec<Signature>,
        consumer: &Location,
    ) -> NativeResult<Option<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Signature>, DemandError> {
            if !self.inputs.semantic_options().no_implicit_any {
                return Ok(None);
            }
            let mut combined: Option<Signature> = None;
            for signature in signatures {
                combined = Some(match combined {
                    None => signature,
                    Some(current) if current.id == signature.id => current,
                    Some(current) => {
                        if !sequence.read(self.compare_type_parameters_identical_completion(
                            current.type_parameter_types(),
                            signature.type_parameter_types(),
                            consumer,
                        ))? {
                            return Ok(None);
                        }
                        sequence.read(self.combine_member_signatures_completion(
                            &current, &signature, false, consumer,
                        ))?
                    }
                });
            }
            Ok(combined)
        })();
        sequence.finish(result)
    }
}
