//! Native operator checking consumes operands and checker dependencies first.

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, SpecialName, TypeFlags, TypeId,
    TypeView, UnionReduction,
};
use super::super::source::{Expression, ExpressionRoot, operators::BinaryRecipe};
use super::{Evaluator, contextual::CheckMode, facts::TypeFacts, relations::RelationKind};
use crate::analysis::SemanticFailureReason;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;
use crate::types::PrimitiveName;

mod truthy;

/// The relation `reportOperatorError` asks of unrelated operand bases.
type OperandRelation<E> = fn(&mut E, TypeId, TypeId, &Location) -> NativeResult<bool>;

impl Evaluator<'_, '_> {
    /// `checkNonNullAssertion` / `checkNonNullChain` always check their operand
    /// in Normal mode. The assertion removes nullability without the separate
    /// `checkNonNullExpression` diagnostic/recovery producer.
    pub(super) fn check_non_null_assertion_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let recipe = self.source.non_null_assertion_recipe(root)?;
        self.check_expression_completion(&recipe.operand, CheckMode::NORMAL)?
            .and_then(|left| {
                if !recipe.chain {
                    return self.non_nullable_type_completion(left, consumer);
                }
                self.optional_expression_type_completion(
                    left,
                    recipe.operand_optional_root,
                    recipe.operand_optional_chain,
                    &recipe.operand.location,
                )?
                .and_then(|non_optional| {
                    self.non_nullable_type_completion(non_optional, consumer)?
                        .and_then(|ty| {
                            self.propagate_optional_marker_completion(
                                ty,
                                non_optional != left,
                                recipe.outermost_chain,
                                consumer,
                            )
                        })
                })
            })
    }

    /// `checkConditionalExpression` checks both arms in the supplied mode,
    /// regardless of a literal condition, then performs subtype reduction.
    pub(super) fn check_conditional_expression(
        &mut self,
        root: ExpressionRoot,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let [condition, when_true, when_false] = self.source.conditional_expression_recipe(root)?;
        self.check_expression_completion(&condition, mode)?
            .and_then(|condition_type| {
                self.check_known_truthy_types_completion(
                    &condition,
                    condition_type,
                    when_true.root,
                    consumer,
                )?
                .and_then(|()| {
                    self.check_expression_completion(&when_true, mode)?
                        .and_then(|left| {
                            self.check_expression_completion(&when_false, mode)?
                                .and_then(|right| {
                                    self.union_type_reduced_completion(
                                        vec![left, right],
                                        UnionReduction::Subtype,
                                        None,
                                        None,
                                        consumer,
                                    )
                                })
                        })
                })
            })
    }

    /// `checkDeleteExpression` / `checkDeleteExpressionMustBeOptional`: an
    /// invalid reference is only a diagnostic. Actual symbol and raw property
    /// type acquisitions still run for valid access expressions.
    pub(super) fn check_delete_expression(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let (operand, access) = self.source.delete_operand(root)?;
        self.check_expression_completion(&operand, CheckMode::NORMAL)?
            .and_then(|_| {
                let mut sequence = NativeSequence::new();
                let result = (|| {
                    if matches!(
                        access.site.kind,
                        K::PropertyAccessExpression | K::ElementAccessExpression
                    ) && let Some(symbol) = self.reference_resolved_symbol_if_any(access)?
                    {
                        let symbol =
                            self.reference_export_symbol_of_value_if_exported(symbol, consumer)?;
                        let symbol = self.register_reference_symbol(symbol, consumer)?;
                        if !self.property_is_readonly(symbol, consumer)? {
                            let ty = sequence
                                .read(self.property_raw_type_completion(symbol, consumer))?;
                            if self.inputs.semantic_options().strict_null_checks
                                && !self.model.has_type_flags(
                                    ty,
                                    TypeFlags::ANY | TypeFlags::UNKNOWN | TypeFlags::NEVER,
                                )
                            {
                                if self.inputs.semantic_options().exact_optional_property_types {
                                    self.property_symbol_flags(symbol)?;
                                } else {
                                    sequence.read(self.type_facts_completion(
                                        ty,
                                        TypeFacts::ONLY_UNDEFINED,
                                        consumer,
                                    ))?;
                                }
                            }
                        }
                    }
                    Ok(self.model.primitive(PrimitiveName::Boolean))
                })();
                sequence.finish(result)
            })
    }

    /// `checkTypeOfExpression`: the operand is checked in Normal mode before
    /// returning the canonical initialized typeof union.
    pub(super) fn check_typeof_expression(&mut self, root: ExpressionRoot) -> NativeResult<TypeId> {
        let operand = self.source.typeof_operand(root)?;
        Ok(self
            .check_expression_completion(&operand, CheckMode::NORMAL)?
            .map(|_| self.model.typeof_type()))
    }

    /// Admission wrapper for pattern/flow callers. Expression recursion uses
    /// the completed native value and defers admission to its own boundary.
    pub(super) fn check_binary_expression(
        &mut self,
        root: ExpressionRoot,
        mode: CheckMode,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.check_binary_expression_completion(root, mode, consumer)?
            .admit()
    }

    /// `checkBinaryLikeExpression`: destructuring checks the right before its
    /// pattern; ordinary operands are checked left-to-right in the given mode.
    /// A recovered left value still consumes the right, while an interruption
    /// stops the operation. Equality suppresses relations only in TypeOnly mode.
    pub(super) fn check_binary_expression_completion(
        &mut self,
        root: ExpressionRoot,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let recipe = self.source.binary_recipe(root)?;
        if let Some(pattern) = recipe.destructuring {
            return self
                .check_expression_completion(&recipe.right, mode)?
                .and_then_value(|source| {
                    self.checked_destructuring_assignment(
                        pattern,
                        source,
                        mode,
                        recipe.right_is_this,
                    )
                });
        }
        self.check_expression_completion(&recipe.left, mode)?
            .and_then(|left| {
                self.check_expression_completion(&recipe.right, mode)?
                    .and_then(|right| {
                        self.check_binary_operands(&recipe, left, right, mode, consumer)
                    })
            })
    }

    fn check_binary_operands(
        &mut self,
        recipe: &BinaryRecipe,
        left: TypeId,
        right: TypeId,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let truthiness = if recipe.test_truthy {
            self.check_known_truthy_types_completion(
                &recipe.left,
                left,
                recipe.truthy_body,
                consumer,
            )?
        } else {
            NativeCompletion::clean(())
        };
        truthiness.and_then(|()| {
            let operator = recipe.operator;
            // checkTruthinessOfType and checkNullishCoalesceOperands only emit
            // syntax/type-flag diagnostics; neither changes these semantic types.
            let result = match operator {
                K::AsteriskToken
                | K::AsteriskEqualsToken
                | K::AsteriskAsteriskToken
                | K::AsteriskAsteriskEqualsToken
                | K::SlashToken
                | K::SlashEqualsToken
                | K::PercentToken
                | K::PercentEqualsToken
                | K::MinusToken
                | K::MinusEqualsToken
                | K::LessThanLessThanToken
                | K::LessThanLessThanEqualsToken
                | K::GreaterThanGreaterThanToken
                | K::GreaterThanGreaterThanEqualsToken
                | K::GreaterThanGreaterThanGreaterThanToken
                | K::GreaterThanGreaterThanGreaterThanEqualsToken
                | K::BarToken
                | K::BarEqualsToken
                | K::CaretToken
                | K::CaretEqualsToken
                | K::AmpersandToken
                | K::AmpersandEqualsToken => {
                    return self.check_arithmetic_binary(recipe, left, right, consumer);
                }
                K::PlusToken | K::PlusEqualsToken => {
                    return self.check_plus_binary(recipe, left, right, consumer);
                }
                K::LessThanToken
                | K::GreaterThanToken
                | K::LessThanEqualsToken
                | K::GreaterThanEqualsToken => {
                    let mut sequence = NativeSequence::new();
                    let result = (|| {
                        if sequence
                            .read(self.check_symbol_operands_completion(left, right, consumer))?
                        {
                            let left = sequence.read(
                                self.check_non_null_type_completion(left, &recipe.left.location),
                            )?;
                            let left = sequence.read(
                                self.base_literal_type_for_comparison_completion(left, consumer),
                            )?;
                            let right = sequence.read(
                                self.check_non_null_type_completion(right, &recipe.right.location),
                            )?;
                            let right = sequence.read(
                                self.base_literal_type_for_comparison_completion(right, consumer),
                            )?;
                            if !sequence.read(
                                self.relational_comparable_completion(left, right, consumer),
                            )? {
                                sequence.read(self.report_operator_error_completion(
                                    left,
                                    right,
                                    Some(Self::relational_comparable_completion),
                                    consumer,
                                ))?;
                            }
                        }
                        Ok(self.model.primitive(PrimitiveName::Boolean))
                    })();
                    return sequence.finish(result);
                }

                K::EqualsEqualsToken
                | K::ExclamationEqualsToken
                | K::EqualsEqualsEqualsToken
                | K::ExclamationEqualsEqualsToken => {
                    if !mode.contains(CheckMode::TYPE_ONLY) {
                        let mut sequence = NativeSequence::new();
                        let result = (|| {
                            sequence.read(self.check_nan_equality_completion(
                                &recipe.left,
                                &recipe.right,
                                consumer,
                            ))?;
                            if !sequence
                                .read(self.equality_comparable_completion(left, right, consumer))?
                            {
                                sequence.read(self.report_operator_error_completion(
                                    left,
                                    right,
                                    Some(Self::equality_comparable_completion),
                                    consumer,
                                ))?;
                            }
                            Ok(self.model.primitive(PrimitiveName::Boolean))
                        })();
                        return sequence.finish(result);
                    }
                    Ok(self.model.primitive(PrimitiveName::Boolean))
                }
                K::AmpersandAmpersandToken | K::AmpersandAmpersandEqualsToken => {
                    let mut sequence = NativeSequence::new();
                    let result = (|| {
                        let result = if sequence
                            .read(self.type_facts_completion(left, TypeFacts::TRUTHY, consumer))?
                            .contains(TypeFacts::TRUTHY)
                        {
                            let falsy_source = if self.inputs.semantic_options().strict_null_checks
                            {
                                left
                            } else {
                                sequence
                                    .read(self.base_type_of_literal_completion(right, consumer))?
                            };
                            let falsy =
                                sequence.read(self.extract_definitely_falsy_types_completion(
                                    falsy_source,
                                    consumer,
                                ))?;
                            sequence.read(self.union_type_ex_completion(
                                vec![falsy, right],
                                None,
                                None,
                                consumer,
                            ))?
                        } else {
                            left
                        };
                        if operator == K::AmpersandAmpersandEqualsToken {
                            sequence.read(self.check_assignment_operator_completion(
                                recipe, left, right, consumer,
                            ))?;
                        }
                        Ok(result)
                    })();
                    return sequence.finish(result);
                }
                K::BarBarToken | K::BarBarEqualsToken => {
                    let mut sequence = NativeSequence::new();
                    let result = (|| {
                        let result = if sequence
                            .read(self.type_facts_completion(left, TypeFacts::FALSY, consumer))?
                            .contains(TypeFacts::FALSY)
                        {
                            let retained = sequence.read(self.type_with_facts_completion(
                                left,
                                TypeFacts::TRUTHY,
                                consumer,
                            ))?;
                            let retained = sequence
                                .read(self.non_nullable_type_completion(retained, consumer))?;
                            sequence.read(self.union_type_reduced_completion(
                                vec![retained, right],
                                UnionReduction::Subtype,
                                None,
                                None,
                                consumer,
                            ))?
                        } else {
                            left
                        };
                        if operator == K::BarBarEqualsToken {
                            sequence.read(self.check_assignment_operator_completion(
                                recipe, left, right, consumer,
                            ))?;
                        }
                        Ok(result)
                    })();
                    return sequence.finish(result);
                }
                K::QuestionQuestionToken | K::QuestionQuestionEqualsToken => {
                    let mut sequence = NativeSequence::new();
                    let result = (|| {
                        let result = if sequence
                            .read(self.type_facts_completion(left, TypeFacts::NULLABLE, consumer))?
                            .contains(TypeFacts::NULLABLE)
                        {
                            let retained =
                                sequence.read(self.non_nullable_type_completion(left, consumer))?;
                            sequence.read(self.union_type_reduced_completion(
                                vec![retained, right],
                                UnionReduction::Subtype,
                                None,
                                None,
                                consumer,
                            ))?
                        } else {
                            left
                        };
                        if operator == K::QuestionQuestionEqualsToken {
                            sequence.read(self.check_assignment_operator_completion(
                                recipe, left, right, consumer,
                            ))?;
                        }
                        Ok(result)
                    })();
                    return sequence.finish(result);
                }
                K::EqualsToken => {
                    return self
                        .check_assignment_operator_completion(recipe, left, right, consumer)
                        .map(|assignment| assignment.map(|_| right));
                }
                K::InKeyword => {
                    return self.check_in_expression_completion(recipe, left, right, consumer);
                }
                K::CommaToken => Ok(right),
                _ => Err(self.operator_pending(
                    consumer,
                    "native assignment/in/instanceof checking is pending",
                )),
            };
            result.map(NativeCompletion::clean)
        })
    }

    /// `checkInExpression`: the two reporting relations are independent;
    /// false on the key still checks the receiver. A private left operand has
    /// already installed its lexical symbol; this operation only observes it.
    fn check_in_expression_completion(
        &mut self,
        recipe: &BinaryRecipe,
        left: TypeId,
        right: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        use super::relations::RelationReporting;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.is_silent_never(left) || self.model.is_silent_never(right) {
                return Ok(self.model.silent_never());
            }
            let left_node = recipe
                .left
                .root
                .ok_or_else(|| super::super::internal("in left operand has no actual node"))?;
            if left_node.site.kind == K::PrivateIdentifier {
                let declaration = self.source.declaration_from_expression_root(left_node)?;
                if self
                    .inputs
                    .semantic_options()
                    .private_access_needs_emit_helpers()
                {
                    sequence.read(self.check_external_emit_helpers_completion(
                        declaration,
                        super::emit_helpers::ExternalEmitHelpers::PRIVATE_IN,
                        &recipe.left.location,
                    ))?;
                }
                if self.reference_resolved_symbol_if_any(left_node)?.is_none()
                    && self.source.containing_class(declaration)?.is_some()
                {
                    let is_unchecked_js = self.is_unchecked_js_suggestion(
                        left_node,
                        None,
                        true,
                        &recipe.left.location,
                    )?;
                    sequence.read(self.report_nonexistent_property_completion(
                        declaration,
                        right,
                        is_unchecked_js,
                        &recipe.left.location,
                    ))?;
                }
            } else {
                let left = sequence
                    .read(self.check_non_null_type_completion(left, &recipe.left.location))?;
                let key = self.model.string_number_symbol_type();
                sequence.read(self.check_type_related_to_completion(
                    left,
                    key,
                    RelationKind::Assignable,
                    RelationReporting::Report.assignment_at(left_node),
                    consumer,
                ))?;
            }
            let right_node = recipe
                .right
                .root
                .ok_or_else(|| super::super::internal("in right operand has no actual node"))?;
            let nonnull = sequence
                .read(self.check_non_null_type_completion(right, &recipe.right.location))?;
            let object = self.model.primitive(PrimitiveName::Object);
            if sequence.read(self.check_type_related_to_completion(
                nonnull,
                object,
                RelationKind::Assignable,
                RelationReporting::Report.assignment_at(right_node),
                consumer,
            ))? && sequence
                .read(self.has_empty_object_intersection_completion(right, consumer))?
            {
                sequence.read(self.check_diagnostic_type_display_completion(right, consumer))?;
            }
            Ok(self.model.primitive(PrimitiveName::Boolean))
        })();
        sequence.finish(result)
    }

    /// `hasEmptyObjectIntersection` uses Some over unions and checks the
    /// special unknown-empty identity before an intersection's base constraint.
    fn has_empty_object_intersection_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let types = self
                .model
                .union_constituents(ty)
                .map_or_else(|| vec![ty], |types| types.to_vec());
            for ty in types {
                if ty == self.model.unknown_empty_object_type() {
                    return Ok(true);
                }
                if self.model.has_type_flags(ty, TypeFlags::INTERSECTION) {
                    let base = sequence
                        .read(self.base_constraint_completion(ty, consumer))?
                        .unwrap_or(ty);
                    if sequence
                        .read(self.is_empty_anonymous_object_type_completion(base, consumer))?
                    {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `initializeTypeChecker` builds `numberOrBigIntType` with the ordinary union
    /// factory. All arithmetic and unary callers share its canonical identity.
    pub(super) fn number_or_bigint_type_completion(
        &mut self,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let number = self.model.primitive(PrimitiveName::Number);
        let bigint = self.model.primitive(PrimitiveName::Bigint);
        self.union_type_ex_completion(vec![number, bigint], None, None, consumer)
    }

    /// `checkArithmeticOperandType`: only a failed numeric relation asks the
    /// optional promise/awaited relation, regardless of its diagnostic wording.
    pub(super) fn check_arithmetic_operand_type_completion(
        &mut self,
        ty: TypeId,
        await_valid: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let numeric = sequence.read(self.number_or_bigint_type_completion(consumer))?;
            if sequence.read(self.type_related_completion(
                ty,
                numeric,
                RelationKind::Assignable,
                consumer,
            ))? {
                return Ok(true);
            }
            if await_valid
                && let Some(awaited) =
                    sequence.read(self.awaited_type_of_promise_completion(ty, consumer))?
            {
                sequence.read(self.type_related_completion(
                    awaited,
                    numeric,
                    RelationKind::Assignable,
                    consumer,
                ))?;
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// Arithmetic branch of `checkBinaryLikeExpression`. Boolean bitwise
    /// diagnostics return before operand relations or compound assignment.
    fn check_arithmetic_binary(
        &mut self,
        recipe: &BinaryRecipe,
        left: TypeId,
        right: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.is_silent_never(left) || self.model.is_silent_never(right) {
                return Ok(self.model.silent_never());
            }
            let left =
                sequence.read(self.check_non_null_type_completion(left, &recipe.left.location))?;
            let right = sequence
                .read(self.check_non_null_type_completion(right, &recipe.right.location))?;
            if self.model.has_type_flags(left, TypeFlags::BOOLEAN_LIKE)
                && self.model.has_type_flags(right, TypeFlags::BOOLEAN_LIKE)
                && matches!(
                    recipe.operator,
                    K::BarToken
                        | K::BarEqualsToken
                        | K::CaretToken
                        | K::CaretEqualsToken
                        | K::AmpersandToken
                        | K::AmpersandEqualsToken
                )
            {
                return Ok(self.model.primitive(PrimitiveName::Number));
            }
            let left_ok = sequence.read(self.check_arithmetic_operand_type_completion(
                left,
                true,
                &recipe.left.location,
            ))?;
            let right_ok = sequence.read(self.check_arithmetic_operand_type_completion(
                right,
                true,
                &recipe.right.location,
            ))?;
            let result = if sequence.read(self.type_assignable_to_kind_completion(
                left,
                TypeFlags::ANY | TypeFlags::UNKNOWN,
                false,
                consumer,
            ))? && sequence.read(self.type_assignable_to_kind_completion(
                right,
                TypeFlags::ANY | TypeFlags::UNKNOWN,
                false,
                consumer,
            ))? || !self.model.maybe_type_of_kind(left, TypeFlags::BIGINT_LIKE)
                && !self.model.maybe_type_of_kind(right, TypeFlags::BIGINT_LIKE)
            {
                self.model.primitive(PrimitiveName::Number)
            } else if sequence.read(self.both_are_bigint_like_completion(left, right, consumer))? {
                if matches!(
                    recipe.operator,
                    K::GreaterThanGreaterThanGreaterThanToken
                        | K::GreaterThanGreaterThanGreaterThanEqualsToken
                ) {
                    sequence
                        .read(self.report_operator_error_completion(left, right, None, consumer))?;
                }
                self.model.primitive(PrimitiveName::Bigint)
            } else {
                sequence.read(self.report_operator_error_completion(
                    left,
                    right,
                    Some(Self::both_are_bigint_like_completion),
                    consumer,
                ))?;
                sequence.read(NativeCompletion::recovered(
                    self.model.native_error_type(),
                    super::super::invalid_at_location(
                        self.inputs,
                        consumer,
                        SemanticFailureReason::ErrorType,
                        "native arithmetic returns an error type for mixed numeric operands",
                    )
                    .into(),
                ))?
            };
            if left_ok && right_ok {
                sequence.read(
                    self.check_assignment_operator_completion(recipe, left, result, consumer),
                )?;
                if matches!(
                    recipe.operator,
                    K::LessThanLessThanToken
                        | K::LessThanLessThanEqualsToken
                        | K::GreaterThanGreaterThanToken
                        | K::GreaterThanGreaterThanEqualsToken
                        | K::GreaterThanGreaterThanGreaterThanToken
                        | K::GreaterThanGreaterThanGreaterThanEqualsToken
                ) {
                    let root = recipe.right.root.ok_or_else(|| {
                        super::super::internal("shift operand has no actual node")
                    })?;
                    sequence.read(self.evaluate_constant_completion(
                        root,
                        Some(root),
                        &recipe.right.location,
                    ))?;
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `bothAreBigIntLike` retains the native short-circuit ordering.
    fn both_are_bigint_like_completion(
        &mut self,
        left: TypeId,
        right: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            Ok(sequence.read(self.type_assignable_to_kind_completion(
                left,
                TypeFlags::BIGINT_LIKE,
                false,
                consumer,
            ))? && sequence.read(self.type_assignable_to_kind_completion(
                right,
                TypeFlags::BIGINT_LIKE,
                false,
                consumer,
            ))?)
        })();
        sequence.finish(result)
    }

    /// Plus's separate branch in `checkBinaryLikeExpression` preserves string
    /// coercion before non-null checking and uses strict kind comparisons.
    fn check_plus_binary(
        &mut self,
        recipe: &BinaryRecipe,
        mut left: TypeId,
        mut right: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.is_silent_never(left) || self.model.is_silent_never(right) {
                return Ok(self.model.silent_never());
            }
            if !sequence.read(self.type_assignable_to_kind_completion(
                left,
                TypeFlags::STRING_LIKE,
                false,
                consumer,
            ))? && !sequence.read(self.type_assignable_to_kind_completion(
                right,
                TypeFlags::STRING_LIKE,
                false,
                consumer,
            ))? {
                left = sequence
                    .read(self.check_non_null_type_completion(left, &recipe.left.location))?;
                right = sequence
                    .read(self.check_non_null_type_completion(right, &recipe.right.location))?;
            }
            let mut result = None;
            for (kind, primitive, either) in [
                (TypeFlags::NUMBER_LIKE, PrimitiveName::Number, false),
                (TypeFlags::BIGINT_LIKE, PrimitiveName::Bigint, false),
                (TypeFlags::STRING_LIKE, PrimitiveName::String, true),
            ] {
                let left_matches = sequence
                    .read(self.type_assignable_to_kind_completion(left, kind, true, consumer))?;
                let matches = if either {
                    left_matches
                        || sequence.read(
                            self.type_assignable_to_kind_completion(right, kind, true, consumer),
                        )?
                } else {
                    left_matches
                        && sequence.read(
                            self.type_assignable_to_kind_completion(right, kind, true, consumer),
                        )?
                };
                if matches {
                    result = Some(self.model.primitive(primitive));
                    break;
                }
            }
            if result.is_none()
                && (self.model.has_type_flags(left, TypeFlags::ANY)
                    || self.model.has_type_flags(right, TypeFlags::ANY))
            {
                result = Some(
                    if self.model.is_error_type(left) || self.model.is_error_type(right) {
                        self.model.native_error_type()
                    } else {
                        self.model.special(SpecialName::Any)
                    },
                );
            }
            if let Some(result) = result {
                if !sequence.read(self.check_symbol_operands_completion(left, right, consumer))? {
                    return Ok(result);
                }
                if recipe.operator == K::PlusEqualsToken {
                    sequence.read(
                        self.check_assignment_operator_completion(recipe, left, result, consumer),
                    )?;
                }
                return Ok(result);
            }
            // Plus produces genuine Any; only actual display dependencies add
            // recovery evidence. The diagnostic alone never marks this Any invalid.
            sequence.read(self.report_operator_error_completion(
                left,
                right,
                Some(Self::close_enough_for_plus_completion),
                consumer,
            ))?;
            Ok(self.model.special(SpecialName::Any))
        })();
        sequence.finish(result)
    }

    /// `checkBinaryLikeExpression` plus reporting callback uses ordered kind
    /// assignability, skipping the right question when the left is false.
    fn close_enough_for_plus_completion(
        &mut self,
        left: TypeId,
        right: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let kind = TypeFlags::NUMBER_LIKE
                | TypeFlags::BIGINT_LIKE
                | TypeFlags::STRING_LIKE
                | TypeFlags::ANY
                | TypeFlags::UNKNOWN;
            Ok(sequence
                .read(self.type_assignable_to_kind_completion(left, kind, false, consumer))?
                && sequence
                    .read(self.type_assignable_to_kind_completion(right, kind, false, consumer))?)
        })();
        sequence.finish(result)
    }

    /// `checkForDisallowedESSymbolOperand` visits the right constraint only if
    /// the left is not symbol-like. It emits no type-display diagnostic.
    fn check_symbol_operands_completion(
        &mut self,
        left: TypeId,
        right: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            Ok(!sequence.read(
                self.maybe_type_of_kind_considering_base_constraint_completion(
                    left,
                    TypeFlags::ES_SYMBOL_LIKE,
                    consumer,
                ),
            )? && !sequence.read(
                self.maybe_type_of_kind_considering_base_constraint_completion(
                    right,
                    TypeFlags::ES_SYMBOL_LIKE,
                    consumer,
                ),
            )?)
        })();
        sequence.finish(result)
    }

    /// `getBaseTypeOfLiteralTypeForComparison` deliberately maps enum literals
    /// to their primitive kind instead of acquiring the parent enum's type.
    fn base_literal_type_for_comparison_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        for (flags, primitive) in [
            (
                TypeFlags::STRING_LITERAL | TypeFlags::TEMPLATE_LITERAL | TypeFlags::STRING_MAPPING,
                PrimitiveName::String,
            ),
            (
                TypeFlags::NUMBER_LITERAL | TypeFlags::ENUM,
                PrimitiveName::Number,
            ),
            (TypeFlags::BIGINT_LITERAL, PrimitiveName::Bigint),
            (TypeFlags::BOOLEAN_LITERAL, PrimitiveName::Boolean),
        ] {
            if self.model.has_type_flags(ty, flags) {
                return Ok(NativeCompletion::clean(self.model.primitive(primitive)));
            }
        }
        if self.model.has_type_flags(ty, TypeFlags::UNION) {
            return self.map_type_completion(ty, consumer, |checker, ty| {
                checker.base_literal_type_for_comparison_completion(ty, consumer)
            });
        }
        Ok(NativeCompletion::clean(ty))
    }

    /// Relational operators distinguish numeric operands before applying the
    /// bidirectional Comparable relation. Equality has a separate nullable gate.
    fn relational_comparable_completion(
        &mut self,
        left: TypeId,
        right: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if self.model.has_type_flags(left, TypeFlags::ANY)
                || self.model.has_type_flags(right, TypeFlags::ANY)
            {
                return Ok(true);
            }
            let numeric = sequence.read(self.number_or_bigint_type_completion(consumer))?;
            let left_numeric = sequence.read(self.type_related_completion(
                left,
                numeric,
                RelationKind::Assignable,
                consumer,
            ))?;
            let right_numeric = sequence.read(self.type_related_completion(
                right,
                numeric,
                RelationKind::Assignable,
                consumer,
            ))?;
            Ok(left_numeric && right_numeric
                || !left_numeric
                    && !right_numeric
                    && (sequence.read(self.type_related_completion(
                        left,
                        right,
                        RelationKind::Comparable,
                        consumer,
                    ))? || sequence.read(self.type_related_completion(
                        right,
                        left,
                        RelationKind::Comparable,
                        consumer,
                    ))?))
        })();
        sequence.finish(result)
    }

    /// `checkAssignmentOperator`: compound dot writes acquire the setter
    /// before reference validation, then exact-optional heads and elaboration.
    fn check_assignment_operator_completion(
        &mut self,
        recipe: &BinaryRecipe,
        left: TypeId,
        right: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        use super::relations::RelationReporting;
        if !crate::syntax::is_assignment_operator(recipe.operator) {
            return Ok(NativeCompletion::clean(()));
        }
        let node = recipe
            .left
            .root
            .ok_or_else(|| super::super::internal("assignment left has no actual node"))?;
        // GetAssignmentDeclarationKind's exports suppression requires a JS
        // source file's CommonJS `exports`, which binding refuses.
        let dot = node.site.kind == K::PropertyAccessExpression;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let left = if recipe.operator != K::EqualsToken && dot {
                sequence.read(self.check_access_expression_completion(
                    node,
                    CheckMode::NORMAL,
                    true,
                ))?
            } else {
                left
            };
            if self.source.assignment_reference_is_valid(node)? {
                let mut error = RelationReporting::Report.assignment_at(node);
                if self.inputs.semantic_options().exact_optional_property_types
                    && dot
                    && self.model.maybe_type_of_kind(right, TypeFlags::UNDEFINED)
                {
                    let access = self.source.access_recipe(node)?;
                    let receiver = sequence.read(
                        self.type_of_expression_completion(
                            node.file,
                            access
                                .receiver
                                .root
                                .ok_or_else(|| {
                                    super::super::internal("access receiver has no actual node")
                                })?
                                .site,
                        ),
                    )?;
                    let super::super::source::AccessKey::Name { name, .. } = access.key else {
                        return Err(
                            super::super::internal("dot assignment lacks a named key").into()
                        );
                    };
                    let target = sequence.read(
                        self.type_of_property_of_type_completion(receiver, &name, consumer),
                    )?;
                    if self.exact_optional_property_mismatch(Some(right), target) {
                        error = RelationReporting::Report.exact_optional_assignment_at(node);
                    }
                }
                sequence.read(
                    self.check_type_related_to_and_optionally_elaborate_completion(
                        right,
                        left,
                        RelationKind::Assignable,
                        error,
                        recipe.right.root,
                        consumer,
                    ),
                )?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkBinaryLikeExpression` equality reporting asks the asymmetric
    /// `isTypeEqualityComparableTo` in forward then reverse order. Each direction
    /// skips its Comparable relation when its target is nullable.
    fn equality_comparable_completion(
        &mut self,
        left: TypeId,
        right: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            Ok(self.model.has_type_flags(right, TypeFlags::NULLABLE)
                || sequence.read(self.type_related_completion(
                    left,
                    right,
                    RelationKind::Comparable,
                    consumer,
                ))?
                || self.model.has_type_flags(left, TypeFlags::NULLABLE)
                || sequence.read(self.type_related_completion(
                    right,
                    left,
                    RelationKind::Comparable,
                    consumer,
                ))?)
        })();
        sequence.finish(result)
    }

    /// `reportOperatorError` / `getBaseTypesIfUnrelated`: a supplied relation
    /// consumes the ordered awaited and literal-base questions. Related bases
    /// retain the original display pair. Both effective operands are always
    /// serialized, including when no relation was supplied or await would work;
    /// those diagnostic dependencies can fail even though the operator's result
    /// type is otherwise known.
    fn report_operator_error_completion(
        &mut self,
        left: TypeId,
        right: TypeId,
        is_related: Option<OperandRelation<Self>>,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut effective = (left, right);
            if let Some(is_related) = is_related {
                let awaited_left =
                    sequence.read(self.awaited_type_no_alias_completion(left, consumer))?;
                let awaited_right =
                    sequence.read(self.awaited_type_no_alias_completion(right, consumer))?;
                let works_awaited = !(awaited_left == Some(left) && awaited_right == Some(right))
                    && if let (Some(left), Some(right)) = (awaited_left, awaited_right) {
                        sequence.read(is_related(self, left, right, consumer))?
                    } else {
                        false
                    };
                if !works_awaited {
                    let left_base =
                        sequence.read(self.base_type_of_literal_completion(left, consumer))?;
                    let right_base =
                        sequence.read(self.base_type_of_literal_completion(right, consumer))?;
                    if !sequence.read(is_related(self, left_base, right_base, consumer))? {
                        effective = (left_base, right_base);
                    }
                }
            }
            sequence.read(self.type_names_for_error_display_completion(
                effective.0,
                effective.1,
                consumer,
            ))?;
            Ok(())
        })();
        sequence.finish(result)
    }

    fn check_nan_equality_completion(
        &mut self,
        left: &Expression,
        right: &Expression,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for expression in [left, right] {
                let Some(root) = expression.root else {
                    continue;
                };
                let Some(root) = self.source.global_nan_candidate(root)? else {
                    continue;
                };
                let global = sequence.read(self.global_value_symbol_completion("NaN", consumer))?;
                if let Some(global) = global {
                    let resolved = sequence.read(
                        self.reference_resolved_identifier_symbol_completion(root, consumer),
                    )?;
                    let _same_symbol = self.reference_symbol_identity(global, consumer)?
                        == self.reference_symbol_identity(resolved, consumer)?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `extractDefinitelyFalsyTypes` / `getDefinitelyFalsyPartOfType` map
    /// native constituents without widening fresh false or nullable identities.
    fn extract_definitely_falsy_types_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.map_type_completion(ty, consumer, |checker, ty| {
            for (flags, primitive, value) in [
                (TypeFlags::STRING, PrimitiveName::String, ""),
                (TypeFlags::NUMBER, PrimitiveName::Number, "0"),
                (TypeFlags::BIGINT, PrimitiveName::Bigint, "0"),
            ] {
                if checker.model.has_type_flags(ty, flags) {
                    return Ok(NativeCompletion::clean(
                        checker.model.regular_literal_type(primitive, value),
                    ));
                }
            }
            if checker.model.has_type_flags(
                ty,
                TypeFlags::VOID
                    | TypeFlags::UNDEFINED
                    | TypeFlags::NULL
                    | TypeFlags::ANY
                    | TypeFlags::UNKNOWN,
            ) {
                return Ok(NativeCompletion::clean(ty));
            }
            if let TypeView::Literal { base, value } = checker.model.get(ty)? {
                let falsy = match base {
                    PrimitiveName::Boolean => value.equals_text("false"),
                    PrimitiveName::String => value.is_empty(),
                    PrimitiveName::Number => value.equals_text("0") || value.equals_text("-0"),
                    PrimitiveName::Bigint => value.equals_text("0") || value.equals_text("-0"),
                    _ => false,
                };
                if falsy {
                    return Ok(NativeCompletion::clean(ty));
                }
            }
            Ok(NativeCompletion::clean(
                checker.model.special(SpecialName::Never),
            ))
        })
    }

    fn operator_pending(&self, consumer: &Location, message: &'static str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }
}
