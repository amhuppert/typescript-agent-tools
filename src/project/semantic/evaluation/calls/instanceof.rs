//! The native BinaryExpression invocation has its own receiver, argument and
//! default-signature policy; it shares only actual overload/cache operations.

use super::*;

impl Evaluator<'_, '_> {
    /// `resolveInstanceofExpression`: check the right operand without non-null
    /// conversion, prefer its actual hasInstance method, and otherwise test
    /// structured signatures before the Function relation.
    pub(super) fn resolve_instanceof_expression_completion(
        &mut self,
        recipe: &CallRecipe,
        mode: CheckMode,
    ) -> NativeResult<ResolvedSignature> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let right = sequence
                .read(self.check_expression_completion(&recipe.callee, CheckMode::NORMAL))?;
            if !self.model.has_type_flags(right, TypeFlags::ANY) {
                if let Some(method) = sequence.read(
                    self.symbol_has_instance_method_type_completion(right, &recipe.callee.location),
                )? {
                    let apparent =
                        sequence.read(self.apparent_type_completion(method, &recipe.location))?;
                    if self.model.is_error_type(apparent) {
                        return Ok(SignaturePreparation::Resolved(
                            ResolvedSignature::Signature(
                                sequence.read(self.resolve_error_call_completion(recipe))?,
                            ),
                        ));
                    }
                    let calls = sequence
                        .read(self.call_signatures_completion(apparent, &recipe.location))?;
                    let constructs = sequence
                        .read(self.construct_signatures_completion(apparent, &recipe.location))?;
                    if sequence.read(self.is_untyped_function_call_completion(
                        method,
                        apparent,
                        calls.len(),
                        constructs.len(),
                        &recipe.location,
                    ))? {
                        return Ok(SignaturePreparation::Resolved(
                            ResolvedSignature::Signature(
                                sequence.read(self.resolve_untyped_call_completion(recipe))?,
                            ),
                        ));
                    }
                    if !calls.is_empty() {
                        return Ok(SignaturePreparation::Call(PreparedCall {
                            signatures: calls,
                            chain: None,
                            mode,
                            construct_via_call: false,
                        }));
                    }
                } else if !sequence.read(
                    self.type_has_call_or_construct_signatures_completion(right, &recipe.location),
                )? {
                    let function = sequence
                        .read(self.global_object_type_completion("Function", &recipe.location))?;
                    if !sequence.read(self.type_related_completion(
                        right,
                        function,
                        RelationKind::Subtype,
                        &recipe.location,
                    ))? {
                        return Ok(SignaturePreparation::Resolved(
                            ResolvedSignature::Signature(
                                sequence.read(self.resolve_error_call_completion(recipe))?,
                            ),
                        ));
                    }
                }
            }
            Ok(SignaturePreparation::Resolved(
                ResolvedSignature::Signature(self.any_signature()?),
            ))
        })();
        let prepared = sequence.finish(result)?;
        self.resolve_prepared_signature_completion(recipe, prepared)
    }

    /// `typeHasCallOrConstructSignatures` asks structured members directly,
    /// without getSignaturesOfType's apparent/constraint conversion.
    pub(super) fn type_has_call_or_construct_signatures_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if !self.model.has_type_flags(
            ty,
            TypeFlags::OBJECT | TypeFlags::UNION | TypeFlags::INTERSECTION,
        ) {
            return Ok(NativeCompletion::clean(false));
        }
        Ok(self
            .structured_type_members_completion(ty, consumer)?
            .map(|members| {
                members.is_some_and(|members| {
                    !members.call_signatures.is_empty() || !members.construct_signatures.is_empty()
                })
            }))
    }
}
