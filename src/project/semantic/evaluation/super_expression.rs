//! Native super-expression checking. Syntax legality and semantic base
//! acquisition remain separate; both ordinary checking and dotted-name
//! effects consume this same complete operation.

use super::super::model::{NativeCompletion, NativeResult, NativeSequence, SpecialName, TypeId};
use super::super::source::{DeclarationIdentity, ExpressionRoot, SuperExpressionContext};
use super::Evaluator;
use crate::analysis::SemanticFailureReason;
use crate::references::Location;

impl Evaluator<'_, '_> {
    /// `classDeclarationExtendsNull`: compare the actual base constructor
    /// identity with nullWideningType after acquiring the class declaration.
    pub(super) fn class_declaration_extends_null_completion(
        &mut self,
        class: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let symbol = self.reference_symbol_of_declaration(class)?;
        self.declared_reference_type_completion(symbol, consumer)?
            .and_then(|class_type| {
                self.base_constructor_type_completion(class_type, consumer)
                    .map(|base| base.map(|base| base == self.model.null_initializer()))
            })
    }

    /// `checkSuperExpression`: legal object members produce genuine Any;
    /// invalid usage and absent class bases produce actual errorType. The
    /// latter markers survive successful outer property/call continuation.
    pub(super) fn check_super_expression_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let (class, has_extends, before_super, call, static_, in_constructor_argument) =
                match self.source.super_expression_context(root)? {
                    SuperExpressionContext::Invalid => {
                        return sequence.read(self.super_error_completion(consumer));
                    }
                    SuperExpressionContext::ObjectLiteral => {
                        return Ok(self.model.special(SpecialName::Any));
                    }
                    SuperExpressionContext::Class {
                        class,
                        has_extends,
                        before_super,
                        call,
                        static_,
                        in_constructor_argument,
                    } => (
                        class,
                        has_extends,
                        before_super,
                        call,
                        static_,
                        in_constructor_argument,
                    ),
                };
            // checkThisBeforeSuper reads the class/base before its diagnostic
            // flow-order predicate. That predicate adds no type dependencies.
            if before_super && has_extends {
                sequence.read(self.class_declaration_extends_null_completion(class, consumer))?;
            }
            if !has_extends {
                return sequence.read(self.super_error_completion(consumer));
            }
            if sequence.read(self.class_declaration_extends_null_completion(class, consumer))? {
                return if call {
                    sequence.read(self.super_error_completion(consumer))
                } else {
                    Ok(self.model.null_initializer())
                };
            }
            let symbol = self.reference_symbol_of_declaration(class)?;
            let class_type =
                sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
            let bases = sequence.read(self.base_types_completion(class_type, consumer))?;
            let Some(&base) = bases.first() else {
                return sequence.read(self.super_error_completion(consumer));
            };
            if in_constructor_argument {
                return sequence.read(self.super_error_completion(consumer));
            }
            if static_ || call {
                // Native's ES2021-and-earlier static-initializer node flag is
                // consumed only by deferred Reflect spelling diagnostics;
                // neither publication nor that report acquires semantic types.
                sequence.read(self.base_constructor_type_completion(class_type, consumer))
            } else {
                let this = self.model.target_this_parameter(class_type)?;
                sequence
                    .read(self.get_type_with_this_argument_completion(base, this, false, consumer))
            }
        })();
        sequence.finish(result)
    }

    fn super_error_completion(&mut self, consumer: &Location) -> NativeResult<TypeId> {
        NativeCompletion::recovered(
            self.model.native_error_type(),
            super::super::invalid_at_location(
                self.inputs,
                consumer,
                SemanticFailureReason::ErrorType,
                "native super expression consumes errorType recovery",
            )
            .into(),
        )
    }
}
