//! Constant evaluation traverses syntax independently of checked types.
use super::super::{constants::ConstantValue, model::DemandError};
use super::{DeclarationIdentity, ExpressionRoot, Source};
use crate::project::binding::flow::Site;
use crate::{Value, syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) enum ConstantExpression {
    Literal(ConstantValue),
    Unary(K, ExpressionRoot),
    Binary(K, ExpressionRoot, ExpressionRoot),
    Template,
    Entity {
        name: Option<String>,
    },
    EnumElement {
        receiver: ExpressionRoot,
        name: ExpressionRoot,
    },
    None,
}

impl Source<'_, '_> {
    /// `NewEvaluator` skips parentheses only. Assertions deliberately break
    /// constant evaluation even when their checked type is a literal.
    pub(in crate::project::semantic) fn constant_expression(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<(ExpressionRoot, ConstantExpression), DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = syntax::skip_parentheses(ast, captured.node_at(root.site)?);
        let node = ast.node(id);
        let at = |id| ExpressionRoot {
            file: root.file,
            site: Site::new(ast, id),
        };
        let child = |name| {
            syntax::id(node, name)
                .map(at)
                .ok_or_else(|| super::super::internal("constant expression lacks operand"))
        };
        let form = match node.syntax_kind {
            K::StringLiteral | K::NoSubstitutionTemplateLiteral => {
                let text =
                    captured.literal_value(ast.cooked_literal(node).ok_or_else(|| {
                        super::super::internal("constant literal lacks cooked text")
                    })?);
                ConstantExpression::Literal(ConstantValue::String(text))
            }
            K::NumericLiteral => {
                let Some(Value::Text(text)) = node.field("text") else {
                    return Err(super::super::internal("constant number lacks scanner text").into());
                };
                ConstantExpression::Literal(ConstantValue::Number(
                    crate::numbers::number_from_string(text),
                ))
            }
            K::PrefixUnaryExpression => {
                let operator = syntax::unary_operator(node)
                    .ok_or_else(|| super::super::internal("constant unary lacks operator"))?;
                ConstantExpression::Unary(operator, child("operand")?)
            }
            K::BinaryExpression => {
                let operator = syntax::id(node, "operatorToken")
                    .ok_or_else(|| super::super::internal("constant binary lacks operator"))?;
                ConstantExpression::Binary(
                    ast.node(operator).syntax_kind,
                    child("left")?,
                    child("right")?,
                )
            }
            K::TemplateExpression => ConstantExpression::Template,
            K::Identifier => ConstantExpression::Entity {
                name: ast.identifier_name(id).map(str::to_owned),
            },
            K::PropertyAccessExpression
                if syntax::is_entity_name_expression(
                    ast,
                    syntax::id(node, "expression")
                        .ok_or_else(|| super::super::internal("constant access lacks receiver"))?,
                ) =>
            {
                ConstantExpression::Entity { name: None }
            }
            K::ElementAccessExpression => {
                let receiver = syntax::id(node, "expression")
                    .ok_or_else(|| super::super::internal("constant access lacks receiver"))?;
                let argument = syntax::id(node, "argumentExpression")
                    .ok_or_else(|| super::super::internal("constant access lacks key"))?;
                if syntax::is_entity_name_expression(ast, receiver)
                    && matches!(
                        ast.node(argument).syntax_kind,
                        K::StringLiteral | K::NoSubstitutionTemplateLiteral
                    )
                {
                    ConstantExpression::EnumElement {
                        receiver: at(receiver),
                        name: at(argument),
                    }
                } else {
                    ConstantExpression::None
                }
            }
            _ => ConstantExpression::None,
        };
        Ok((at(id), form))
    }

    /// Syntax projection for `getCombinedNodeFlagsCached`; the evaluator
    /// supplies the actual ValueDeclaration and handles its absence.
    pub(in crate::project::semantic) fn constant_declaration_flags(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<u32, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(syntax::combined_node_flags(
            &captured.ast,
            &captured.parents,
            declaration.node,
        ))
    }
}
