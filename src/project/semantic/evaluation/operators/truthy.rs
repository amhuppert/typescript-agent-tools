//! Native truthiness diagnostics can acquire symbols and hidden display types.

use super::super::{Evaluator, ReferenceSymbol, facts::TypeFacts};
use crate::project::semantic::model::{
    DemandError, NativeResult, NativeSequence, TypeFlags, TypeId,
};
use crate::project::semantic::source::{Expression, ExpressionRoot, operators::SymbolUseReceiver};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    /// `checkTestingKnownTruthyCallableOrAwaitableOrEnumMemberType` and its
    /// two checking workers preserve the supplied type while Source retains
    /// each actual condition/location pair and native recheck decision.
    pub(super) fn check_known_truthy_types_completion(
        &mut self,
        expression: &Expression,
        supplied: TypeId,
        body: Option<ExpressionRoot>,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self.inputs.semantic_options().strict_null_checks {
                return Ok(());
            }
            let root = expression.root.ok_or_else(|| {
                crate::project::semantic::internal("truthy operand has no actual node")
            })?;
            for question in self.source.truthy_checks(root)? {
                let ty = if question.recheck {
                    sequence.read(self.check_expression_completion(
                        &question.expression,
                        super::super::contextual::CheckMode::NORMAL,
                    ))?
                } else {
                    supplied
                };
                if self.model.has_type_flags(ty, TypeFlags::ENUM_LITERAL)
                    && let Some(receiver) = question.receiver
                    && let Some(symbol) = self.reference_resolved_symbol_if_any(receiver)?
                    && self.reference_symbol_flags(symbol, consumer)?.enum_
                {
                    // The remaining native work only formats literal truthiness.
                    continue;
                }
                if !sequence
                    .read(self.type_facts_completion(ty, TypeFacts::TRUTHY, consumer))?
                    .contains(TypeFacts::TRUTHY)
                    || question.property_cast
                {
                    continue;
                }
                let signatures = sequence.read(self.call_signatures_completion(ty, consumer))?;
                let promise = sequence
                    .read(self.awaited_type_of_promise_completion(ty, consumer))?
                    .is_some();
                if signatures.is_empty() && !promise {
                    continue;
                }
                let tested = match question.tested_node {
                    Some(node) => sequence.read(self.reference_symbol_at_node_completion(
                        node,
                        &question.expression.location,
                    ))?,
                    None => None,
                };
                if tested.is_none() && !promise {
                    continue;
                }
                let mut used = false;
                if let Some(symbol) = tested {
                    used = sequence.read(self.symbol_used_in_binary_chain_completion(
                        question.condition,
                        symbol,
                        consumer,
                    ))?;
                    if !used && let Some(body) = body {
                        let tested_node = question.tested_node.ok_or_else(|| {
                            crate::project::semantic::internal("tested symbol lacks actual node")
                        })?;
                        used = sequence.read(self.symbol_used_in_condition_body_completion(
                            question.condition,
                            body,
                            tested_node,
                            symbol,
                            consumer,
                        ))?;
                    }
                }
                if !used && promise {
                    sequence.read(self.type_name_for_error_display_completion(ty, consumer))?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `isSymbolUsedInBinaryExpressionChain` tests identifier descendants of
    /// each right operand; a bare right-hand identifier is not visited.
    fn symbol_used_in_binary_chain_completion(
        &mut self,
        condition: ExpressionRoot,
        tested: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut current = self.source.symbol_use_parent(condition)?;
            while let Some(node) = current {
                let Some((right, parent)) = self.source.symbol_use_binary_step(node)? else {
                    break;
                };
                let mut walk = self.source.symbol_use_walk(right);
                while let Some(child) = self.source.next_symbol_use(&mut walk)? {
                    if child.site.kind == K::Identifier
                        && let Some(symbol) = sequence
                            .read(self.reference_symbol_at_node_completion(child, consumer))?
                        && self.same_symbol_identity(Some(symbol), Some(tested), consumer)?
                    {
                        return Ok(true);
                    }
                }
                current = parent;
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isSymbolUsedInConditionBody` distinguishes a simple tested name from
    /// a property that must be referenced on the same native receiver chain.
    fn symbol_used_in_condition_body_completion(
        &mut self,
        condition: ExpressionRoot,
        body: ExpressionRoot,
        tested_node: ExpressionRoot,
        tested: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let simple = condition.site.kind == K::Identifier
                || tested_node.site.kind == K::Identifier
                    && self
                        .source
                        .symbol_use_parent(tested_node)?
                        .is_some_and(|parent| parent.site.kind == K::BinaryExpression);
            let mut walk = self.source.symbol_use_walk(body);
            while let Some(child) = self.source.next_symbol_use(&mut walk)? {
                if child.site.kind != K::Identifier {
                    continue;
                }
                let Some(symbol) =
                    sequence.read(self.reference_symbol_at_node_completion(child, consumer))?
                else {
                    continue;
                };
                if !self.same_symbol_identity(Some(symbol), Some(tested), consumer)? {
                    continue;
                }
                if simple
                    || sequence.read(self.same_symbol_use_receiver_completion(
                        tested_node,
                        child,
                        consumer,
                    ))?
                {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    fn same_symbol_use_receiver_completion(
        &mut self,
        tested: ExpressionRoot,
        child: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut left = self.source.symbol_use_parent(tested)?;
            let mut right = self.source.symbol_use_parent(child)?;
            while let (Some(a), Some(b)) = (left, right) {
                match (
                    self.source.symbol_use_receiver(a)?,
                    self.source.symbol_use_receiver(b)?,
                ) {
                    (SymbolUseReceiver::Reference, SymbolUseReceiver::Reference)
                        if a.site.kind == b.site.kind =>
                    {
                        let a =
                            sequence.read(self.reference_symbol_at_node_completion(a, consumer))?;
                        let b =
                            sequence.read(self.reference_symbol_at_node_completion(b, consumer))?;
                        return self.same_symbol_identity(a, b, consumer);
                    }
                    (
                        SymbolUseReceiver::Property {
                            receiver: a,
                            name: an,
                        },
                        SymbolUseReceiver::Property {
                            receiver: b,
                            name: bn,
                        },
                    ) => {
                        let an = sequence
                            .read(self.reference_symbol_at_node_completion(an, consumer))?;
                        let bn = sequence
                            .read(self.reference_symbol_at_node_completion(bn, consumer))?;
                        if !self.same_symbol_identity(an, bn, consumer)? {
                            return Ok(false);
                        }
                        left = Some(a);
                        right = Some(b);
                    }
                    (SymbolUseReceiver::Call(a), SymbolUseReceiver::Call(b)) => {
                        left = Some(a);
                        right = Some(b);
                    }
                    _ => return Ok(false),
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// Native symbol equality includes nil==nil and compares actual identity
    /// across the bound and registered-arena representations of one symbol.
    fn same_symbol_identity(
        &self,
        left: Option<ReferenceSymbol>,
        right: Option<ReferenceSymbol>,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        match (left, right) {
            (Some(left), Some(right)) => Ok(self.reference_symbol_identity(left, consumer)?
                == self.reference_symbol_identity(right, consumer)?),
            (None, None) => Ok(true),
            _ => Ok(false),
        }
    }
}
