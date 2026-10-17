//! Template type spans retain cooked exact text and their actual type nodes.
use super::super::model::{DemandError, LiteralValue};
use super::{Expression, ExpressionRoot, Source, TypeExpression, expression, type_expression};
use crate::project::binding::flow::Site;

use crate::syntax;
use std::sync::Arc;

pub(in crate::project::semantic) struct TemplateTypeRecipe {
    pub(in crate::project::semantic) texts: Arc<[LiteralValue]>,
    pub(in crate::project::semantic) types: Vec<TypeExpression>,
    pub(in crate::project::semantic) children: Vec<ExpressionRoot>,
}
pub(in crate::project::semantic) struct TemplateExpressionRecipe {
    pub(in crate::project::semantic) texts: Arc<[LiteralValue]>,
    pub(in crate::project::semantic) expressions: Vec<Expression>,
    pub(in crate::project::semantic) tagged: bool,
}

/// Both `getTypeFromTemplateTypeNode` and `checkTemplateExpression` consume
/// the scanner's exact cooked bytes, before their distinct normalization steps.
fn cooked_text(
    captured: &super::CapturedSyntax<'_>,
    id: crate::NodeId,
) -> Result<LiteralValue, DemandError> {
    let ast = &captured.ast;
    let bytes = ast
        .cooked_literal(ast.node(id))
        .ok_or_else(|| super::super::internal("template token has no cooked payload"))?;
    Ok(captured.literal_value(bytes))
}

impl Source<'_, '_> {
    /// `checkTemplateExpression` retains ordered substitutions and the actual
    /// immediate tagged parent; no substitution is checked while projecting.
    pub(in crate::project::semantic) fn template_expression_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<TemplateExpressionRecipe, DemandError> {
        let inputs = self.inputs;

        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let node = ast.node(id);
        let head = syntax::id(node, "head")
            .ok_or_else(|| super::super::internal("template expression has no head"))?;
        let mut texts = Vec::new();
        texts.push(cooked_text(captured, head)?);
        let mut expressions = Vec::new();
        for &span in syntax::ids(node, "templateSpans") {
            let span = ast.node(span);
            let literal = syntax::id(span, "literal")
                .ok_or_else(|| super::super::internal("template span has no literal"))?;
            let operand = syntax::id(span, "expression")
                .ok_or_else(|| super::super::internal("template span has no expression"))?;
            texts.push(cooked_text(captured, literal)?);
            expressions.push(expression(inputs, root.file, captured, operand));
        }
        Ok(TemplateExpressionRecipe {
            texts: std::sync::Arc::from(texts),
            expressions,
            tagged: captured.parents[id.index()].is_some_and(|parent| {
                ast.node(parent).syntax_kind
                    == crate::syntax_kind::SyntaxKind::TaggedTemplateExpression
            }),
        })
    }

    /// `isTemplateLiteralContext` strips only enclosing parentheses and then
    /// tests actual element-access argument identity.
    pub(in crate::project::semantic) fn is_template_literal_context(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, DemandError> {
        use crate::syntax_kind::SyntaxKind as K;

        let captured = self.syntax(root.file)?;
        let mut id = captured.node_at(root.site)?;
        while let Some(parent) = captured.parents[id.index()] {
            let node = captured.ast.node(parent);
            if node.syntax_kind != K::ParenthesizedExpression {
                return Ok(node.syntax_kind == K::ElementAccessExpression
                    && syntax::id(node, "argumentExpression") == Some(id));
            }
            id = parent;
        }
        Ok(false)
    }

    /// `getTypeFromTemplateTypeNode` takes cooked head/tail payloads without
    /// redecoding source or substituting replacement characters for identity.
    pub(in crate::project::semantic) fn template_type_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<TemplateTypeRecipe, DemandError> {
        let inputs = self.inputs;

        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = ast.node(captured.node_at(root.site)?);
        let head = syntax::id(node, "head")
            .ok_or_else(|| super::super::internal("template type has no head"))?;
        let mut texts = Vec::new();
        texts.push(cooked_text(captured, head)?);
        let mut types = Vec::new();
        let mut children = Vec::new();
        for &span in syntax::ids(node, "templateSpans") {
            let span = ast.node(span);
            let literal = syntax::id(span, "literal")
                .ok_or_else(|| super::super::internal("template span has no literal"))?;
            let ty = syntax::id(span, "type")
                .ok_or_else(|| super::super::internal("template span has no type"))?;
            texts.push(cooked_text(captured, literal)?);
            types.push(type_expression(
                inputs,
                root.file,
                captured,
                &captured.parents,
                ty,
            ));
            children.push(ExpressionRoot {
                file: root.file,
                site: Site::new(ast, ty),
            });
        }
        Ok(TemplateTypeRecipe {
            texts: std::sync::Arc::from(texts),
            types,
            children,
        })
    }
}
