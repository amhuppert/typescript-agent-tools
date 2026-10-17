//! Native body return inference shares expression checking, flow reachability,
//! factories and canonical function identities with the request's Evaluator.

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, SpecialName, TypeFlags, TypeId,
    UnionReduction,
};
use super::super::source::{ReturnBodyKind, ReturnBodyRecipe, ReturnExpressionRecipe};
use super::Evaluator;
use super::contextual::{CheckMode, ContextFlags};
use crate::analysis::SemanticFailureReason;
use std::collections::HashSet;

impl Evaluator<'_, '_> {
    /// `getReturnTypeFromBody`: infer from actual own return contributions and
    /// EndFlowNode, then apply the native contextual-unit and whole widening
    /// questions. Annotation precedence is owned by getReturnTypeOfSignature.
    pub(super) fn return_body_type_completion(
        &mut self,
        body: &ReturnBodyRecipe,
    ) -> NativeResult<TypeId> {
        self.return_body_type_ex_completion(body, CheckMode::NORMAL)
    }

    pub(super) fn return_body_type_ex_completion(
        &mut self,
        body: &ReturnBodyRecipe,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if body.flags.is_invalid() {
                return Err(super::super::internal(
                    "inferred return recipe has no native function body",
                )
                .into());
            }
            let returned = match &body.body {
                ReturnBodyKind::Expression(expression) => {
                    let mut returned =
                        sequence.read(self.check_expression_cached_ex_completion(
                            expression,
                            mode.without(CheckMode::SKIP_GENERIC_FUNCTIONS),
                        ))?;
                    if let Some(root) = expression.root
                        && sequence
                            .read(self.is_const_context_completion(root, &expression.location))?
                    {
                        returned = self.model.regular_literal(returned);
                    }
                    if body.flags.is_async() {
                        returned = sequence.read(self.check_awaited_type_completion(
                            returned,
                            false,
                            &body.location,
                        ))?;
                        returned = sequence
                            .read(self.unwrap_awaited_type_completion(returned, &body.location))?;
                    }
                    returned
                }
                ReturnBodyKind::Block(returns) => {
                    let (types, never_returning) = sequence
                        .read(self.aggregate_return_types_completion(body, returns, mode))?;
                    if body.flags.is_generator() {
                        return Err(self.return_body_pending(body, "native generator yield/next aggregation and iteration-global construction is pending"));
                    }
                    if never_returning {
                        if body.flags.is_async() {
                            let never = self.model.special(SpecialName::Never);
                            return sequence.read(
                                self.create_promise_return_type_completion(never, &body.location),
                            );
                        }
                        return Ok(self.model.special(SpecialName::Never));
                    }
                    if types.is_empty() {
                        let context = sequence.read(self.contextual_return_type_completion(
                            body.root,
                            ContextFlags::NONE,
                            &body.location,
                        ))?;
                        let context = match context {
                            Some(ty) => Some(sequence.read(self.unwrap_return_type_completion(
                                ty,
                                body.flags,
                                &body.location,
                            ))?),
                            None => None,
                        };
                        let returned = self.model.special(
                            if context.is_some_and(|ty| {
                                self.model.some_type_of_kind(ty, TypeFlags::UNDEFINED)
                            }) {
                                SpecialName::Undefined
                            } else {
                                SpecialName::Void
                            },
                        );
                        return if body.flags.is_async() {
                            sequence.read(
                                self.create_promise_return_type_completion(
                                    returned,
                                    &body.location,
                                ),
                            )
                        } else {
                            Ok(returned)
                        };
                    }
                    sequence.read(self.union_type_reduced_completion(
                        types,
                        UnionReduction::Subtype,
                        None,
                        None,
                        &body.location,
                    ))?
                }
            };
            sequence.read(self.report_return_widening_completion(body, returned))?;
            let returned = if self.model.has_type_flags(returned, TypeFlags::UNIT) {
                let contextual =
                    if let Some(signature) =
                        sequence.read(self.contextual_signature_for_function_completion(
                            body.root,
                            &body.location,
                        ))?
                    {
                        let recipe = self.source.signature_from_root(body.root)?;
                        if signature.id == sequence.read(self.signature_completion(&recipe))?.id {
                            Some(returned)
                        } else {
                            let ty =
                                sequence.read(self.get_return_type_of_signature_completion(
                                    &signature,
                                    &body.location,
                                ))?;
                            Some(
                                sequence.read(
                                    self.instantiate_contextual_type_completion(
                                        ty,
                                        Some(
                                            super::super::source::ExpressionRoot {
                                                file: body.root.file,
                                                site: body.root.site(),
                                            }
                                            .into(),
                                        ),
                                        ContextFlags::NONE,
                                        &body.location,
                                    ),
                                )?,
                            )
                        }
                    } else {
                        None
                    };
                let contextual = if body.flags.is_async() {
                    match contextual {
                        Some(ty) => sequence
                            .read(self.promised_type_of_promise_completion(ty, &body.location))?,
                        None => None,
                    }
                } else {
                    contextual
                };
                let widen = match contextual {
                    Some(context) => !sequence.read(self.literal_of_contextual_type_completion(
                        returned,
                        context,
                        &body.location,
                    ))?,
                    None => true,
                };
                let returned = if widen {
                    sequence.read(self.widened_literal_type_completion(returned, &body.location))?
                } else {
                    returned
                };
                self.model.regular_literal(returned)
            } else {
                returned
            };
            let returned = sequence.read(self.widened_type_completion(returned, &body.location))?;
            if body.flags.is_async() {
                return sequence
                    .read(self.create_promise_type_completion(returned, &body.location));
            }
            Ok(returned)
        })();
        sequence.finish(result)
    }

    /// `checkAndAggregateReturnExpressionTypes`: syntactically unreachable
    /// returns still contribute; nested functions were excluded by the sole
    /// ForEachReturnStatement acquisition owner. Identity deduplication and
    /// strict no-expression inclusion precede the Subtype union question.
    fn aggregate_return_types_completion(
        &mut self,
        body: &ReturnBodyRecipe,
        returns: &[ReturnExpressionRecipe],
        mode: CheckMode,
    ) -> NativeResult<(Vec<TypeId>, bool)> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(Vec<TypeId>, bool), DemandError> {
            let mut no_expression = sequence
                .read(self.function_has_implicit_return_completion(body.root, &body.location))?;
            let mut never_return = false;
            let mut types = Vec::new();
            let mut identities = HashSet::new();
            for returned in returns {
                let Some(expression) = &returned.expression else {
                    no_expression = true;
                    continue;
                };
                if let Some(callee) = &returned.bare_call_callee
                    && sequence.read(self.bare_self_return_completion(body, callee))?
                {
                    never_return = true;
                    continue;
                }
                let mut ty = sequence.read(self.check_expression_cached_ex_completion(
                    expression,
                    mode.without(CheckMode::SKIP_GENERIC_FUNCTIONS),
                ))?;
                if body.flags.is_async() {
                    ty = sequence.read(self.check_awaited_type_completion(
                        ty,
                        false,
                        &body.location,
                    ))?;
                    ty = sequence.read(self.unwrap_awaited_type_completion(ty, &body.location))?;
                }
                never_return |= self.model.has_type_flags(ty, TypeFlags::NEVER);
                if let Some(root) = expression.root
                    && sequence
                        .read(self.is_const_context_completion(root, &expression.location))?
                {
                    ty = self.model.regular_literal(ty);
                }
                // Const assertions and contextual-expression construction are
                // consumed by checkExpression. Unimplemented assertion recipes
                // fail there; they cannot enter this ordinary declaration family
                // as incorrectly fresh successful types.
                if identities.insert(ty) {
                    types.push(ty);
                }
            }
            if types.is_empty() && !no_expression && (never_return || body.may_return_never) {
                return Ok((types, true));
            }
            if self.inputs.semantic_options().strict_null_checks
                && !types.is_empty()
                && no_expression
            {
                let undefined = self.model.special(SpecialName::Undefined);
                if identities.insert(undefined) {
                    types.push(undefined);
                }
            }
            Ok((types, false))
        })();
        sequence.finish(result)
    }

    /// `functionHasImplicitReturn` uses the body's actual retained container,
    /// never the function name's outer declaration-flow attachment. Missing
    /// and ambiguous attachments are invariant failures, not guessed ends.
    pub(super) fn function_has_implicit_return_completion(
        &mut self,
        root: super::super::source::SignatureRoot,
        consumer: &crate::references::Location,
    ) -> NativeResult<bool> {
        let graph = self
            .inputs
            .bindings
            .flow_graph(root.file)
            .map_err(|error| {
                super::super::internal(&format!(
                    "return body has no retained flow graph: {error:?}"
                ))
            })?;
        let end = graph
            .container_at(root.site())
            .map_err(super::super::internal)?
            .end;
        match end {
            Some(end) => self.reachable_flow_completion(root.file, end, consumer),
            None => Ok(NativeCompletion::clean(false)),
        }
    }

    /// The bare-self-call branch of `checkAndAggregateReturnExpressionTypes`
    /// compares the cached callee Type.symbol with getMergedSymbol(fn.Symbol()).
    /// It neither acquires the function value type nor compares TypeIds. The
    /// raw function symbol owns the subsequent ValueDeclaration kind gate.
    fn bare_self_return_completion(
        &mut self,
        body: &ReturnBodyRecipe,
        callee: &super::super::source::Expression,
    ) -> NativeResult<bool> {
        self.check_expression_cached_ex_completion(callee, CheckMode::NORMAL)?
            .and_then(|callee_type| {
                let function = self
                    .reference_raw_symbol_of_node(super::super::source::ExpressionRoot {
                        file: body.root.file,
                        site: body.root.site(),
                    })?
                    .ok_or_else(|| {
                        super::super::internal("return-body function has no native symbol")
                    })?;
                let merged = self.get_merged_reference_symbol(function, &body.location)?;
                if self.type_symbol_identity(callee_type)?
                    != Some(self.reference_symbol_identity(merged, &body.location)?)
                {
                    return Ok(super::super::model::NativeCompletion::clean(false));
                }
                if let Some(super::ReferenceDeclaration::Captured(declaration)) =
                    self.reference_symbol_value_declaration(function, &body.location)?
                    && crate::syntax::is_function_expression_or_arrow_function(
                        self.source.declaration_kind(declaration)?,
                    )
                {
                    let root = callee.root.ok_or_else(|| {
                        super::super::internal("bare self-call identifier has no source identity")
                    })?;
                    return self.constant_reference_completion(root, &callee.location);
                }
                Ok(super::super::model::NativeCompletion::clean(true))
            })
    }

    fn return_body_pending(&self, body: &ReturnBodyRecipe, message: &str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            &body.location,
            SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }
}
