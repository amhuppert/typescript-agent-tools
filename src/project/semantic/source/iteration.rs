use super::{ExpressionNode, Source, syntax};
use crate::project::semantic::model::DemandError;
use crate::syntax_kind::SyntaxKind;

impl Source<'_, '_> {
    /// `reportTypeNotIterableError` suggests await for the actual for-of RHS,
    /// not any descendant expression or synthetic error node.
    pub(in crate::project::semantic) fn iteration_error_is_for_of_expression(
        &mut self,
        error: ExpressionNode,
    ) -> Result<bool, DemandError> {
        let ExpressionNode::Captured(root) = error else {
            return Ok(false);
        };
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        let Some(parent) = captured.parents[node.index()] else {
            return Ok(false);
        };
        Ok(
            captured.ast.node(parent).syntax_kind == SyntaxKind::ForOfStatement
                && syntax::id(captured.ast.node(parent), "expression") == Some(node),
        )
    }
}
