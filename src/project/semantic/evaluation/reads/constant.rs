//! Constant-reference identity is distinct from literal values and stable
//! element-index keys. It shares assignment marking and readonly-symbol rules.

use super::*;
use crate::project::semantic::evaluation::{
    ReferenceDeclaration, ReferenceSymbol as SemanticSymbol,
};
use crate::project::semantic::source::ExpressionRoot;

impl Evaluator<'_, '_> {
    /// `isConstantVariable`: only Variable symbols inspect the combined
    /// flags of their actual ValueDeclaration; aliases are never followed.
    pub(in crate::project::semantic::evaluation) fn is_constant_variable_symbol(
        &mut self,
        symbol: SemanticSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        Ok(self.reference_symbol_flags(symbol, consumer)?.variable
            && self.reference_declaration_node_flags(symbol, consumer)?
                & (crate::compiler::node::CONST | crate::compiler::node::USING)
                != 0)
    }

    /// `isConstantReference`: preserve the exact syntax gates, raw resolved
    /// symbol, and left-to-right short circuits. An access reads its already
    /// checked symbol slot; it never repeats property checking. Parentheses
    /// are deliberately not stripped by this operation.
    pub(in crate::project::semantic::evaluation) fn constant_reference_completion(
        &mut self,
        reference: impl Into<ReferenceNode>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut accesses = Vec::new();
        let mut receiver = reference.into();
        loop {
            let syntax = self.reference_syntax(receiver)?;
            if !matches!(
                syntax.kind,
                SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
            ) {
                break;
            }
            accesses.push(receiver);
            receiver = required(syntax.expression, "constant access has no receiver")?;
        }
        let mut result = self.constant_reference_base_completion(receiver, consumer)?;
        for access in accesses.into_iter().rev() {
            result = result.and_then(|constant| {
                if !constant {
                    return Ok(NativeCompletion::clean(false));
                }
                let Some(site) = access.captured_site() else {
                    // Native created access nodes have no installed resolvedSymbol.
                    return Ok(NativeCompletion::clean(false));
                };
                let Some(symbol) = self.reference_resolved_symbol_if_any(ExpressionRoot {
                    file: access.file,
                    site,
                })?
                else {
                    return Ok(NativeCompletion::clean(false));
                };
                let symbol = self.register_reference_symbol(symbol, consumer)?;
                self.property_is_readonly_completion(symbol, consumer)
            })?;
            if !*result.value() {
                break;
            }
        }
        Ok(result)
    }

    fn constant_reference_base_completion(
        &mut self,
        reference: ReferenceNode,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let syntax = self.reference_syntax(reference)?;
        match syntax.kind {
            SyntaxKind::ThisKeyword => Ok(NativeCompletion::clean(true)),
            SyntaxKind::Identifier => {
                if syntax.this_identifier && syntax.in_type_query {
                    return Ok(NativeCompletion::clean(false));
                }
                self.reference_resolved_identifier_symbol_completion(
                    ExpressionRoot {
                        file: reference.file,
                        site: reference.captured()?,
                    },
                    consumer,
                )?
                .and_then(|symbol| {
                    if self.is_constant_variable_symbol(symbol, consumer)? {
                        return Ok(NativeCompletion::clean(true));
                    }
                    if let Some(declaration) =
                        self.mutable_reference_declaration(symbol, consumer)?
                    {
                        return self
                            .is_symbol_assigned_completion(symbol, declaration, consumer)
                            .map(|value| value.map(|assigned| !assigned));
                    }
                    let value = self.reference_symbol_value_declaration(symbol, consumer)?;
                    let function = match value {
                        Some(ReferenceDeclaration::Captured(declaration)) => {
                            self.source.declaration_kind(declaration)?
                                == SyntaxKind::FunctionExpression
                        }
                        Some(ReferenceDeclaration::Json { .. }) | None => false,
                    };
                    Ok(NativeCompletion::clean(function))
                })
            }
            SyntaxKind::ObjectBindingPattern | SyntaxKind::ArrayBindingPattern => {
                let declaration = self
                    .source
                    .binding_pattern_root_declaration(ExpressionRoot {
                        file: reference.file,
                        site: reference.captured()?,
                    })?;
                let facts = self.source.variable_declaration_read_facts(declaration)?;
                if facts.root_kind == SyntaxKind::Parameter
                    || facts.root_kind == SyntaxKind::VariableDeclaration
                        && facts.parent_kind == SyntaxKind::CatchClause
                {
                    self.some_symbol_assigned_completion(declaration, consumer)
                        .map(|value| value.map(|assigned| !assigned))
                } else {
                    Ok(NativeCompletion::clean(
                        facts.root_kind == SyntaxKind::VariableDeclaration
                            && crate::syntax::is_var_const_like_flags(facts.combined_flags),
                    ))
                }
            }
            _ => Ok(NativeCompletion::clean(false)),
        }
    }
}
