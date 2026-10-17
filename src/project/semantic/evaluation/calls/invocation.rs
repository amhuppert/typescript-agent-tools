//! Shared native invocation classification and diagnostic dependencies.
use super::super::native_display::DiagnosticTypeFormat;
use super::*;

impl Evaluator<'_, '_> {
    /// `isUntypedFunctionCall` keeps genuine Any and the Function-assignable
    /// fallback distinct. A union without a common signature cannot enter the
    /// latter path, and native reduction precedes the global Function demand.
    pub(super) fn is_untyped_function_call_completion(
        &mut self,
        ty: TypeId,
        apparent: TypeId,
        call_count: usize,
        construct_count: usize,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if self.model.has_type_flags(ty, TypeFlags::ANY)
            || self.model.has_type_flags(apparent, TypeFlags::ANY)
                && self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER)
        {
            return Ok(NativeCompletion::clean(true));
        }
        if call_count != 0
            || construct_count != 0
            || self.model.has_type_flags(apparent, TypeFlags::UNION)
        {
            return Ok(NativeCompletion::clean(false));
        }
        self.reduced_type_completion(apparent, consumer)?
            .and_then(|reduced| {
                if self.model.has_type_flags(reduced, TypeFlags::NEVER) {
                    return Ok(NativeCompletion::clean(false));
                }
                self.global_object_type_completion("Function", consumer)?
                    .and_then(|function| {
                        self.type_related_completion(
                            ty,
                            function,
                            RelationKind::Assignable,
                            consumer,
                        )
                    })
            })
    }

    /// `invocationErrorDetails` / `invocationError` acquire awaited signatures
    /// and diagnostic type displays in native order, even though this query
    /// tool does not publish compiler diagnostic prose. Getter-head wording
    /// only inspects an existing symbol link and acquires no new dependency.
    pub(super) fn invocation_error_dependencies_completion(
        &mut self,
        apparent: TypeId,
        construct: bool,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(awaited) =
                sequence.read(self.awaited_type_completion(apparent, consumer))?
            {
                self.signatures_of_type(awaited, construct, consumer)?;
            }
            if let Some(types) = self
                .model
                .union_constituents(apparent)
                .map(|types| types.to_vec())
            {
                let mut has_signatures = false;
                let mut diagnostic = false;
                for constituent in types {
                    if !self
                        .signatures_of_type(constituent, construct, consumer)?
                        .is_empty()
                    {
                        has_signatures = true;
                        if diagnostic {
                            break;
                        }
                    } else {
                        if !diagnostic {
                            sequence.read(self.diagnostic_type_display_completion(
                                constituent,
                                None,
                                DiagnosticTypeFormat::DEFAULT,
                                consumer,
                            ))?;
                            sequence.read(self.diagnostic_type_display_completion(
                                apparent,
                                None,
                                DiagnosticTypeFormat::DEFAULT,
                                consumer,
                            ))?;
                            diagnostic = true;
                        }
                        if has_signatures {
                            break;
                        }
                    }
                }
                if !has_signatures {
                    sequence.read(self.diagnostic_type_display_completion(
                        apparent,
                        None,
                        DiagnosticTypeFormat::DEFAULT,
                        consumer,
                    ))?;
                    diagnostic = true;
                }
                if !diagnostic {
                    sequence.read(self.diagnostic_type_display_completion(
                        apparent,
                        None,
                        DiagnosticTypeFormat::DEFAULT,
                        consumer,
                    ))?;
                }
            } else {
                sequence.read(self.diagnostic_type_display_completion(
                    apparent,
                    None,
                    DiagnosticTypeFormat::DEFAULT,
                    consumer,
                ))?;
            }
            sequence.read(
                self.reference_invocation_error_recovery_completion(apparent, construct, consumer),
            )
        })();
        sequence.finish(result)
    }
}
