//! Expression checking within the request-owned evaluator.

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, TypeFlags, TypeId, TypeView,
};
use super::super::source::{Expression, ExpressionForm};
use super::Evaluator;
use super::contextual::CheckMode;
use super::facts::TypeFacts;
use crate::analysis::SemanticFailureReason;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;
use crate::types::PrimitiveName;

type ExpressionKey = (
    crate::project::sources::FileId,
    crate::syntax_kind::SyntaxKind,
    usize,
    usize,
);
pub(super) type FlowExpressionCache =
    std::collections::HashMap<ExpressionKey, NativeCompletion<TypeId>>;

#[derive(Default)]
pub(super) struct ExpressionCache {
    pub(super) instantiations: std::collections::HashMap<
        (super::super::source::DeclarationIdentity, TypeId),
        NativeCompletion<TypeId>,
    >,
    pub(super) type_queries:
        std::collections::HashMap<super::super::source::ExpressionRoot, NativeCompletion<TypeId>>,
    normal: std::collections::HashMap<ExpressionKey, NativeResult<TypeId>>,
    context_free: std::collections::HashMap<ExpressionKey, NativeResult<TypeId>>,
    flow: FlowExpressionCache,
    regular_reference: Option<(crate::project::sources::FileId, crate::Span)>,
}

fn expression_key(
    file: crate::project::sources::FileId,
    site: crate::project::binding::flow::Site,
) -> ExpressionKey {
    (file, site.kind, site.span.start, site.span.end)
}

impl Evaluator<'_, '_> {
    /// The native loop and cached-expression owners hide the same transient
    /// `flowTypeCache`. An empty replacement is distinct from clearing a
    /// completed cache or reusing another reference's transient history.
    pub(super) fn suspend_flow_expression_cache(&mut self) -> FlowExpressionCache {
        std::mem::take(&mut self.expressions.flow)
    }

    pub(super) fn restore_flow_expression_cache(&mut self, saved: FlowExpressionCache) {
        self.expressions.flow = saved;
    }

    /// `getRegularTypeOfExpression`: the public expression question applies
    /// regular-literal normalization after the flow-only expression question.
    /// Only its actual identifier root can use that normalization for admission;
    /// nested value dependencies still ask their own raw type question.
    pub(super) fn regular_type_of_expression(
        &mut self,
        expression: &Expression,
    ) -> Result<TypeId, DemandError> {
        self.regular_type_of_expression_completion(expression)?
            .admit()
    }

    pub(super) fn regular_type_of_expression_completion(
        &mut self,
        expression: &Expression,
    ) -> NativeResult<TypeId> {
        let reference = match expression.form {
            ExpressionForm::Identifier { file, anchor } => Some((file, anchor)),
            _ => None,
        };
        let saved = std::mem::replace(&mut self.expressions.regular_reference, reference);
        let result = self.type_of_source_expression_completion(expression);
        self.expressions.regular_reference = saved;
        Ok(result?.map(|ty| self.model.regular_literal(ty)))
    }

    pub(super) fn is_regular_expression_reference(
        &self,
        file: crate::project::sources::FileId,
        anchor: crate::Span,
    ) -> bool {
        self.expressions.regular_reference == Some((file, anchor))
    }

    /// Consume the exact Source expression attachment rather than recovering
    /// a cache key from presentation origins or the flattened expression form.
    pub(super) fn type_of_source_expression(
        &mut self,
        expression: &Expression,
    ) -> Result<TypeId, DemandError> {
        self.type_of_source_expression_completion(expression)?
            .admit()
    }

    pub(super) fn type_of_source_expression_completion(
        &mut self,
        expression: &Expression,
    ) -> NativeResult<TypeId> {
        let Some(root) = expression.root else {
            return self.check_expression_completion(expression, CheckMode::NORMAL);
        };
        self.type_of_expression_recipe_completion(root.file, root.site, expression)
    }

    /// `getTypeOfInitializer`: use the installed native resolvedType if one
    /// exists, otherwise ask the distinct flow-only expression question.
    pub(super) fn type_of_initializer_completion(
        &mut self,
        file: crate::project::sources::FileId,
        site: crate::project::binding::flow::Site,
    ) -> NativeResult<TypeId> {
        if let Some(result) = self.expressions.normal.get(&expression_key(file, site)) {
            return result.clone();
        }
        self.type_of_expression_completion(file, site)
    }

    /// `getContextFreeTypeOfExpression` caches its own question. An actual
    /// Any context and SkipContextSensitive are installed for computation;
    /// the surrounding inference frame is not changed.
    pub(super) fn context_free_expression_type_completion(
        &mut self,
        root: super::super::source::ExpressionRoot,
    ) -> NativeResult<TypeId> {
        let key = expression_key(root.file, root.site);
        if let Some(result) = self.expressions.context_free.get(&key) {
            return result.clone();
        }
        let any = self.model.special(super::super::model::SpecialName::Any);
        self.with_contextual_type_override(root, any, |checker| {
            let expression = checker.source.expression_at(root.file, root.site)?;
            let result =
                checker.check_expression_completion(&expression, CheckMode::SKIP_CONTEXT_SENSITIVE);
            checker.expressions.context_free.insert(key, result.clone());
            result
        })
    }

    /// `checkExpressionCached` / `checkExpressionCachedEx` in Normal mode:
    /// an actual site owns its installed result; transient flow-expression
    /// cache history is hidden during computation and restored on every exit.
    /// The native active-loop stack is hidden with the transient cache; the
    /// completed loop cache remains available to restarted analysis.
    pub(super) fn check_expression_cached_completion(
        &mut self,
        file: crate::project::sources::FileId,
        site: crate::project::binding::flow::Site,
    ) -> NativeResult<TypeId> {
        let key = expression_key(file, site);
        if let Some(result) = self.expressions.normal.get(&key) {
            return result.clone();
        }
        let expression = self.source.expression_at(file, site)?;
        let saved_loops = std::mem::take(&mut self.reads.loops.active);
        let saved = self.suspend_flow_expression_cache();
        let result = self.check_expression_completion(&expression, CheckMode::NORMAL);
        self.restore_flow_expression_cache(saved);
        self.reads.loops.active = saved_loops;
        self.expressions.normal.insert(key, result.clone());
        result
    }

    /// `checkExpressionCachedEx` caches only Normal mode. Its complete owner
    /// restores loop and flow state before any product admission can fail.
    pub(super) fn check_expression_cached_ex_completion(
        &mut self,
        expression: &Expression,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        if mode != CheckMode::NORMAL {
            return self.check_expression_completion(expression, mode);
        }
        match expression.root {
            Some(root) => self.check_expression_cached_completion(root.file, root.site),
            None => self.check_expression_completion(expression, mode),
        }
    }

    /// `checkComputedPropertyName` installs the actual expression's normal
    /// resolvedType without checkExpressionCached's loop/cache suspension.
    /// Diagnostic rejection retains that installed type; only a consumed
    /// dependency or the explicit forbidden-in errorType becomes a failure.
    pub(super) fn computed_property_name_type(
        &mut self,
        expression: &Expression,
        forbidden_in: bool,
        consumer: &crate::references::Location,
    ) -> Result<TypeId, DemandError> {
        self.computed_property_name_type_completion(expression, forbidden_in, consumer)?
            .admit()
    }

    pub(super) fn computed_property_name_type_completion(
        &mut self,
        expression: &Expression,
        forbidden_in: bool,
        consumer: &crate::references::Location,
    ) -> NativeResult<TypeId> {
        let Some(root) = expression.root else {
            // A rootless failure recipe reports its original acquisition
            // failure. It cannot install a guessed syntax/cache association.
            return self.check_expression_completion(expression, CheckMode::NORMAL);
        };
        let key = expression_key(root.file, root.site);
        if let Some(result) = self.expressions.normal.get(&key) {
            return result.clone();
        }
        let result = if forbidden_in {
            NativeCompletion::recovered(
                self.model.native_error_type(),
                super::super::invalid_at_location(
                    self.inputs,
                    consumer,
                    SemanticFailureReason::ErrorType,
                    "native computed property name consumed forbidden-in errorType",
                )
                .into(),
            )
        } else {
            self.check_expression_completion(expression, CheckMode::NORMAL)
        };
        self.expressions.normal.insert(key, result.clone());
        let value = result?;
        if forbidden_in {
            return Ok(value);
        }
        value.and_then(|ty| {
            let mut sequence = super::super::model::NativeSequence::new();
            let result = (|| {
                let kind = super::super::model::TypeFlags::STRING_LIKE
                    | super::super::model::TypeFlags::NUMBER_LIKE
                    | super::super::model::TypeFlags::ES_SYMBOL_LIKE;
                if !self
                    .model
                    .has_type_flags(ty, super::super::model::TypeFlags::NULLABLE)
                    && !sequence
                        .read(self.type_assignable_to_kind_completion(ty, kind, false, consumer))?
                {
                    let target = self.model.string_number_symbol_type();
                    // The boolean diagnostic outcome does not change resolvedType.
                    sequence.read(self.type_related_completion(
                        ty,
                        target,
                        super::relations::RelationKind::Assignable,
                        consumer,
                    ))?;
                }
                Ok(ty)
            })();
            sequence.finish(result)
        })
    }

    /// `getTypeOfExpression`: quick types precede the actual flow-only cache.
    /// Only a computation that invoked getFlowTypeOfReference is installed.
    /// Contextual and generic expression-mode branches remain explicit demands
    /// in the expression/call owners until their full factories are admitted.
    pub(super) fn type_of_expression(
        &mut self,
        file: crate::project::sources::FileId,
        site: crate::project::binding::flow::Site,
    ) -> Result<TypeId, DemandError> {
        self.type_of_expression_completion(file, site)?.admit()
    }

    pub(super) fn type_of_expression_completion(
        &mut self,
        file: crate::project::sources::FileId,
        site: crate::project::binding::flow::Site,
    ) -> NativeResult<TypeId> {
        let expression = self.source.expression_at(file, site)?;
        self.type_of_expression_recipe_completion(file, site, &expression)
    }

    fn type_of_expression_recipe_completion(
        &mut self,
        file: crate::project::sources::FileId,
        site: crate::project::binding::flow::Site,
        expression: &Expression,
    ) -> NativeResult<TypeId> {
        self.type_of_expression_recipe_worker(file, site, expression)
            .map_err(|failure| {
                super::super::observe_recovery(self.inputs, failure, &expression.location)
            })
            .map(|value| {
                value.observe(|failure| {
                    super::super::observe_recovery(self.inputs, failure, &expression.location)
                })
            })
    }

    fn type_of_expression_recipe_worker(
        &mut self,
        file: crate::project::sources::FileId,
        site: crate::project::binding::flow::Site,
        expression: &Expression,
    ) -> NativeResult<TypeId> {
        self.quick_type_of_expression_completion(expression)?
            .and_then(|quick| {
                if let Some(ty) = quick {
                    return Ok(NativeCompletion::clean(ty));
                }
                let key = expression_key(file, site);
                if let Some(value) = self.expressions.flow.get(&key) {
                    return Ok(value.clone());
                }
                let start = self.reads.invocations;
                let result = self.check_expression_completion(expression, CheckMode::TYPE_ONLY)?;
                if start != self.reads.invocations {
                    self.expressions.flow.insert(key, result.clone());
                }
                Ok(result)
            })
    }

    /// `checkExpression`, `checkObjectLiteral` and `checkPropertyAssignment`.
    /// Ordinary property initializers demand their types before the object is
    /// established; this is an inference dependency, not graph expansion.
    pub(super) fn check_expression(
        &mut self,
        expression: &Expression,
    ) -> Result<TypeId, DemandError> {
        self.check_expression_ex(expression, CheckMode::NORMAL)
    }

    pub(super) fn check_expression_ex(
        &mut self,
        expression: &Expression,
        mode: CheckMode,
    ) -> Result<TypeId, DemandError> {
        self.check_expression_completion(expression, mode)?.admit()
    }

    /// `checkExpressionEx` keeps native completion separate from the product's
    /// admission result. Bind source-free evidence on this observation copy,
    /// including secondary recovery under an already located primary failure.
    pub(super) fn check_expression_completion(
        &mut self,
        expression: &Expression,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        self.check_expression_ex_worker(expression, mode)
            .map(|completion| {
                completion.observe(|failure| {
                    super::super::observe_recovery(self.inputs, failure, &expression.location)
                })
            })
            .map_err(|failure| {
                super::super::observe_recovery(self.inputs, failure, &expression.location)
            })
    }

    fn check_expression_ex_worker(
        &mut self,
        expression: &Expression,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        // `checkExpressionEx` resets the count while preserving the current
        // instantiation depth and active mapper frames.
        self.reset_instantiation_count();
        let completion = match &expression.form {
            ExpressionForm::This(root) => {
                self.check_this_expression_completion(*root, &expression.location)?
            }
            ExpressionForm::Super(root) => {
                self.check_super_expression_completion(*root, &expression.location)?
            }
            ExpressionForm::Instantiation(root) => {
                self.check_expression_with_type_arguments_completion(*root)?
            }
            ExpressionForm::Spread(root) => self.check_spread_expression_completion(*root, mode)?,
            ExpressionForm::Await(operand) => {
                self.check_await_expression_completion(operand, &expression.location)?
            }
            ExpressionForm::Template(root) => {
                self.check_template_expression_completion(*root, &expression.location)?
            }
            ExpressionForm::TypeOf(root) => self.check_typeof_expression(*root)?,
            ExpressionForm::Binary(root) => {
                self.check_binary_expression_completion(*root, mode, &expression.location)?
            }
            ExpressionForm::ConditionalValue(root) => {
                self.check_conditional_expression(*root, mode, &expression.location)?
            }
            ExpressionForm::Delete(root) => {
                self.check_delete_expression(*root, &expression.location)?
            }
            ExpressionForm::Array(root) => NativeCompletion::clean(self.check_array_literal(
                *root,
                mode,
                &expression.location,
            )?),
            ExpressionForm::Assertion(root) => {
                self.check_assertion_completion(*root, mode, &expression.location)?
            }
            ExpressionForm::NonNullAssertion(root) => {
                self.check_non_null_assertion_completion(*root, &expression.location)?
            }
            ExpressionForm::Satisfies(root) => {
                self.check_satisfies_expression_completion(*root, &expression.location)?
            }
            ExpressionForm::Omitted => NativeCompletion::clean(self.model.undefined_initializer()),
            ExpressionForm::Call(root) => self.check_call_expression_completion(*root, mode)?,
            ExpressionForm::Function(root) => {
                self.check_function_expression_completion(*root, mode, &expression.location)?
            }
            ExpressionForm::Class(root) => {
                self.check_class_expression_completion(*root, &expression.location)?
            }
            ExpressionForm::Access(root) => {
                self.check_access_expression_completion(*root, mode, false)?
            }
            ExpressionForm::Identifier { file, anchor } => {
                self.check_identifier_completion(expression, *file, *anchor, mode)?
            }
            ExpressionForm::PrivateIdentifier(root) => {
                self.check_private_identifier_expression_completion(*root, &expression.location)?
            }
            // TypeScript 7.0.2 checkRegularExpressionLiteral returns the
            // initialized globalRegExpType. Its grammar check only emits
            // diagnostics; it acquires no additional semantic dependency.
            ExpressionForm::RegularExpression => {
                self.global_object_type_completion("RegExp", &expression.location)?
            }
            ExpressionForm::Null => NativeCompletion::clean(self.model.null_initializer()),
            ExpressionForm::Void => NativeCompletion::clean(self.model.undefined_initializer()),
            ExpressionForm::Literal { base, value } => {
                if *base == PrimitiveName::String {
                    let regular = self.model.regular_string_literal_value(value.clone());
                    NativeCompletion::clean(self.model.fresh_literal(regular))
                } else {
                    let text = value
                        .scalar_text()
                        .map_err(|error| self.literal_value_failure(error, &expression.location))?;
                    let value = if *base == PrimitiveName::Bigint {
                        crate::numbers::bigint_literal_value(text).ok_or_else(|| {
                            super::super::internal(
                                "scanner-normalized bigint payload is not a valid integer",
                            )
                        })?
                    } else {
                        text.to_owned()
                    };
                    NativeCompletion::clean(self.model.literal(*base, value))
                }
            }
            ExpressionForm::Unary {
                operator,
                direct_literal,
                operand,
            } => self
                .check_expression_completion(operand, CheckMode::NORMAL)?
                .and_then(|operand_type| {
                    self.check_unary_expression_completion(
                        expression,
                        operand,
                        *operator,
                        *direct_literal,
                        operand_type,
                    )
                })?,
            ExpressionForm::Object(root) => NativeCompletion::clean(self.check_object_literal(
                *root,
                mode,
                &expression.location,
            )?),
            ExpressionForm::Pending { reason, message } => {
                return Err(super::super::unsupported_demand_at_location(
                    self.inputs,
                    &expression.location,
                    *reason,
                    message.clone(),
                ));
            }
        };
        completion.and_then(|ty| {
            self.instantiate_expression_signature_completion(
                expression.root.map(Into::into),
                ty,
                mode,
                &expression.location,
            )
        })
    }

    /// `checkSyntheticExpression` within `checkExpressionEx`: a synthetic
    /// spread is checked as its numeric indexed-access value. Its retained
    /// array type is a separate question used by getSpreadArgumentType.
    pub(super) fn check_synthetic_expression_completion(
        &mut self,
        node: super::contextual::ContextNode,
        ty: TypeId,
        spread: bool,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            self.reset_instantiation_count();
            let ty = if spread {
                let number = self.model.primitive(PrimitiveName::Number);
                sequence.read(self.indexed_access_type_completion(ty, number, consumer))?
            } else {
                ty
            };
            sequence.read(self.instantiate_expression_signature_completion(
                Some(node),
                ty,
                mode,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `checkPrefixUnaryExpression` / `checkPostfixUnaryExpression`: only direct numeric +/- and
    /// bigint - produce fresh literal shortcuts. All other operands use the
    /// established type, including parenthesized literals and nested unary forms.
    fn check_unary_expression_completion(
        &mut self,
        expression: &Expression,
        operand_expression: &Expression,
        operator: K,
        direct: Option<PrimitiveName>,
        operand: TypeId,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if self.model.is_silent_never(operand) {
                return Ok(self.model.silent_never());
            }
            if let Some(base) = direct
                && (matches!(operator, K::MinusToken)
                    || base == PrimitiveName::Number && matches!(operator, K::PlusToken))
            {
                let TypeView::Literal {
                    base: actual,
                    value,
                } = self.model.get(operand)?
                else {
                    return Err(super::super::internal(
                        "direct unary literal operand has a different established type",
                    )
                    .into());
                };
                if actual != base {
                    return Err(super::super::internal(
                        "direct unary literal operand has the wrong base",
                    )
                    .into());
                }
                let value = if matches!(operator, K::MinusToken) {
                    super::super::constants::negate_literal(
                        self.literal_scalar_text(&value, &operand_expression.location)?,
                    )
                } else {
                    self.literal_scalar_text(&value, &operand_expression.location)?
                        .to_owned()
                };
                return Ok(self.model.literal(base, value));
            }
            let consumer = &operand_expression.location;
            match operator {
                K::PlusToken | K::MinusToken | K::TildeToken => {
                    sequence.read(self.check_non_null_type_completion(operand, consumer))?;
                    sequence.read(
                        self.maybe_type_of_kind_considering_base_constraint_completion(
                            operand,
                            TypeFlags::ES_SYMBOL_LIKE,
                            consumer,
                        ),
                    )?;
                    if operator == K::PlusToken {
                        if sequence.read(
                            self.maybe_type_of_kind_considering_base_constraint_completion(
                                operand,
                                TypeFlags::BIGINT_LIKE,
                                consumer,
                            ),
                        )? {
                            let base = sequence
                                .read(self.base_type_of_literal_completion(operand, consumer))?;
                            sequence.read(
                                self.check_diagnostic_type_display_completion(base, consumer),
                            )?;
                        }
                        Ok(self.model.primitive(PrimitiveName::Number))
                    } else {
                        sequence.read(self.unary_result_type_completion(operand, consumer))
                    }
                }
                K::ExclamationToken => {
                    let truthy = sequence.read(self.truthiness_completion(expression, operand))?;
                    Ok(if let Some(truthy) = truthy {
                        self.model
                            .literal(PrimitiveName::Boolean, (!truthy).to_string())
                    } else {
                        self.model.primitive(PrimitiveName::Boolean)
                    })
                }
                K::PlusPlusToken | K::MinusMinusToken => {
                    let non_null =
                        sequence.read(self.check_non_null_type_completion(operand, consumer))?;
                    if sequence.read(
                        self.check_arithmetic_operand_type_completion(non_null, false, consumer),
                    )? {
                        let root = operand_expression.root.ok_or_else(|| {
                            super::super::internal("update operand has no actual node")
                        })?;
                        self.source.assignment_reference_is_valid(root)?;
                    }
                    sequence.read(self.unary_result_type_completion(operand, consumer))
                }
                _ => Err(
                    super::super::internal("unary recipe has an invalid native operator").into(),
                ),
            }
        })();
        sequence.finish(result)
    }

    /// `getUnaryResultType` asks constituent flags, not base constraints.
    /// A generic indexed operand therefore need not acquire its members to
    /// establish numeric coercion; unions use the same native kind owner.
    fn unary_result_type_completion(
        &mut self,
        operand: TypeId,
        consumer: &crate::references::Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if self
                .model
                .maybe_type_of_kind(operand, TypeFlags::BIGINT_LIKE)
            {
                if sequence.read(self.type_assignable_to_kind_completion(
                    operand,
                    TypeFlags::ANY | TypeFlags::UNKNOWN,
                    false,
                    consumer,
                ))? || self
                    .model
                    .maybe_type_of_kind(operand, TypeFlags::NUMBER_LIKE)
                {
                    return sequence.read(self.number_or_bigint_type_completion(consumer));
                }
                return Ok(self.model.primitive(PrimitiveName::Bigint));
            }
            Ok(self.model.primitive(PrimitiveName::Number))
        })();
        sequence.finish(result)
    }

    /// `checkPrefixUnaryExpression` asks the whole native facts operation with
    /// its Truthy/Falsy mask; all fact and lazy-dependency rules have one owner.
    fn truthiness_completion(
        &mut self,
        expression: &Expression,
        operand: TypeId,
    ) -> NativeResult<Option<bool>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<bool>, DemandError> {
            let facts = sequence.read(self.type_facts_completion(
                operand,
                TypeFacts::TRUTHY | TypeFacts::FALSY,
                &expression.location,
            ))?;
            Ok(if facts == TypeFacts::TRUTHY {
                Some(true)
            } else if facts == TypeFacts::FALSY {
                Some(false)
            } else {
                None
            })
        })();
        sequence.finish(result)
    }
}
