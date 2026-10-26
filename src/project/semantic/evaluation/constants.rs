//! Native source constant evaluation, independent of expression type caches.
use super::super::{
    constants::{ConstantResult, ConstantValue},
    model::{LiteralValue, NativeResult, NativeSequence},
    source::{ExpressionRoot, constants::ConstantExpression},
};
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::{references::Location, syntax_kind::SyntaxKind as K};

impl Evaluator<'_, '_> {
    /// `NewEvaluator` retains native left-to-right demands and metadata even
    /// when a binary operation has no constant value. Templates stop at their
    /// first absent constant, after checkTemplateExpression has checked all types.
    pub(super) fn evaluate_constant_completion(
        &mut self,
        root: ExpressionRoot,
        location: Option<ExpressionRoot>,
        consumer: &Location,
    ) -> NativeResult<ConstantResult> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let (root, expression) = self.source.constant_expression(root)?;
            match expression {
                ConstantExpression::Literal(value) => Ok(ConstantResult {
                    syntactically_string: matches!(value, ConstantValue::String(_)),
                    value: Some(value),
                    ..Default::default()
                }),
                ConstantExpression::Unary(operator, operand) => {
                    let result = sequence
                        .read(self.evaluate_constant_completion(operand, location, consumer))?;
                    let value = match result.value {
                        Some(ConstantValue::Number(number)) => match operator {
                            K::PlusToken => Some(number),
                            K::MinusToken => Some(-number),
                            K::TildeToken => {
                                Some((!crate::numbers::number_to_int32(number)) as f64)
                            }
                            _ => None,
                        }
                        .map(ConstantValue::Number),
                        _ => None,
                    };
                    Ok(ConstantResult {
                        value,
                        syntactically_string: false,
                        ..result
                    })
                }
                ConstantExpression::Binary(operator, left, right) => {
                    let left = sequence
                        .read(self.evaluate_constant_completion(left, location, consumer))?;
                    let right = sequence
                        .read(self.evaluate_constant_completion(right, location, consumer))?;
                    let syntactically_string = operator == K::PlusToken
                        && (left.syntactically_string || right.syntactically_string);
                    let other_files = left.other_files || right.other_files;
                    let external_references = left.external_references || right.external_references;
                    let value = match (left.value, right.value) {
                        (Some(ConstantValue::Number(left)), Some(ConstantValue::Number(right))) => {
                            use crate::numbers::number_to_int32 as int;
                            let shift = (int(right) as u32) & 31;
                            match operator {
                                K::BarToken => Some((int(left) | int(right)) as f64),
                                K::AmpersandToken => Some((int(left) & int(right)) as f64),
                                K::CaretToken => Some((int(left) ^ int(right)) as f64),
                                K::LessThanLessThanToken => {
                                    Some(int(left).wrapping_shl(shift) as f64)
                                }
                                K::GreaterThanGreaterThanToken => Some((int(left) >> shift) as f64),
                                K::GreaterThanGreaterThanGreaterThanToken => {
                                    Some(((int(left) as u32) >> shift) as f64)
                                }
                                K::AsteriskToken => Some(left * right),
                                K::SlashToken => Some(left / right),
                                K::PlusToken => Some(left + right),
                                K::MinusToken => Some(left - right),
                                K::PercentToken => Some(left % right),
                                K::AsteriskAsteriskToken => {
                                    Some(crate::numbers::number_exponentiate(left, right))
                                }
                                _ => None,
                            }
                            .map(ConstantValue::Number)
                        }
                        (Some(left), Some(right)) if operator == K::PlusToken => {
                            let text =
                                LiteralValue::concat([&left.into_text(), &right.into_text()]);
                            Some(ConstantValue::String(text))
                        }
                        _ => None,
                    };
                    Ok(ConstantResult {
                        value,
                        syntactically_string,
                        other_files,
                        external_references,
                    })
                }
                ConstantExpression::Template => {
                    let recipe = self.source.template_expression_recipe(root)?;
                    let mut result = ConstantResult {
                        syntactically_string: true,
                        ..Default::default()
                    };
                    let mut texts = vec![recipe.texts[0].clone()];
                    for (index, expression) in recipe.expressions.iter().enumerate() {
                        let operand = expression.root.ok_or_else(|| {
                            super::super::internal("template substitution has no node")
                        })?;
                        let span = sequence
                            .read(self.evaluate_constant_completion(operand, location, consumer))?;
                        let Some(value) = span.value else {
                            return Ok(ConstantResult {
                                syntactically_string: true,
                                ..Default::default()
                            });
                        };
                        texts.push(value.into_text());
                        texts.push(recipe.texts[index + 1].clone());
                        result.other_files |= span.other_files;
                        result.external_references |= span.external_references;
                    }
                    result.value = Some(ConstantValue::String(LiteralValue::concat(&texts)));
                    Ok(result)
                }
                ConstantExpression::Entity { name } => {
                    sequence.read(self.evaluate_constant_entity_completion(
                        root,
                        name.as_deref(),
                        location,
                        consumer,
                    ))
                }
                ConstantExpression::EnumElement { receiver, name } => {
                    let symbol = sequence
                        .read(self.resolve_value_entity_name_completion(receiver, consumer))?;
                    if let Some(symbol) = symbol
                        && self.reference_symbol_flags(symbol, consumer)?.enum_
                    {
                        // `evaluateEntity` reads ArgumentExpression.Text only
                        // after resolving an enum receiver. This is an identity
                        // demand at that argument, owned by the shared name rule.
                        let name = self.source.try_property_name_text(name)?.ok_or_else(|| {
                            super::super::internal("constant enum element lacks its literal name")
                        })?;
                        if let Some(member) =
                            self.reference_own_module_export(symbol, &name, consumer)?
                        {
                            return sequence.read(
                                self.evaluate_enum_member_completion(member, location, consumer),
                            );
                        }
                    }
                    Ok(ConstantResult::default())
                }
                ConstantExpression::None => Ok(ConstantResult::default()),
            }
        })();
        sequence.finish(result)
    }

    /// `resolveEntityName(Value, ignoreErrors=true, dontResolveAlias=false)`
    /// is shared by constant evaluation and const-assertion eligibility.
    pub(super) fn resolve_value_entity_name_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let anchor = self.source.reference_entity_name_anchor(root)?;
        Ok(self
            .reference_entity_at_completion(
                root.file,
                anchor,
                false,
                crate::project::binding::NativeLookupMeaning::VALUE,
                false,
                consumer,
            )?
            .map(|(symbol, _)| symbol))
    }

    /// `evaluateEntity`: Value-only resolution, special globals and enum members
    /// precede actual unannotated constant-variable initializer evaluation.
    fn evaluate_constant_entity_completion(
        &mut self,
        root: ExpressionRoot,
        name: Option<&str>,
        location: Option<ExpressionRoot>,
        consumer: &Location,
    ) -> NativeResult<ConstantResult> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(symbol) =
                sequence.read(self.resolve_value_entity_name_completion(root, consumer))?
            else {
                return Ok(ConstantResult::default());
            };
            if let Some(name @ ("Infinity" | "NaN")) = name {
                let global = sequence.read(self.global_symbol_completion(
                    name,
                    crate::project::binding::NativeLookupMeaning::VALUE,
                    consumer,
                ))?;
                if let Some(global) = global
                    && self.reference_symbol_identity(global, consumer)?
                        == self.reference_symbol_identity(symbol, consumer)?
                {
                    return Ok(ConstantResult {
                        value: Some(ConstantValue::Number(crate::numbers::number_from_string(
                            name,
                        ))),
                        ..Default::default()
                    });
                }
            }
            let flags = self.reference_symbol_flags(symbol, consumer)?;
            if flags.enum_member {
                return sequence
                    .read(self.evaluate_enum_member_completion(symbol, location, consumer));
            }
            if !self.is_constant_variable_symbol(symbol, consumer)? {
                return Ok(ConstantResult::default());
            }
            let value_declaration = self.reference_symbol_value_declaration(symbol, consumer)?;
            // getDeclarationNodeFlagsFromSymbol has no first-declaration fallback.
            let Some(ReferenceDeclaration::Captured(declaration)) = value_declaration else {
                return Ok(ConstantResult::default());
            };
            let facts = self.source.variable_facts(declaration)?;
            if facts.kind != K::VariableDeclaration || facts.annotation.is_some() {
                return Ok(ConstantResult::default());
            }
            let Some(initializer) = facts.initializer else {
                return Ok(ConstantResult::default());
            };
            let declaration_root = self.source.declaration_expression_root(declaration)?;
            if let Some(location) = location
                && (location == declaration_root
                    || !sequence.read(self.block_scoped_name_declared_before_use_completion(
                        declaration,
                        location,
                        consumer,
                    ))?)
            {
                return Ok(ConstantResult::default());
            }
            let initializer = initializer
                .root
                .ok_or_else(|| super::super::internal("constant initializer has no node"))?;
            let mut result = sequence.read(self.evaluate_constant_completion(
                initializer,
                Some(declaration_root),
                consumer,
            ))?;
            result.external_references = true;
            if location.is_some_and(|location| location.file != declaration.file) {
                result.syntactically_string = false;
                result.other_files = true;
            }
            Ok(result)
        })();
        sequence.finish(result)
    }
}
