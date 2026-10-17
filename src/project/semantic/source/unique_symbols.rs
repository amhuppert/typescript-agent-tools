//! The syntax-only gates of the native unique-symbol producers.

use super::{DeclarationIdentity, ExpressionRoot, Source};
use crate::project::semantic::model::DemandError;
use crate::{syntax, syntax_kind::SyntaxKind as K};

impl Source<'_, '_> {
    /// `getTypeFromTypeOperatorNode` tests the immediate operand kind, without
    /// checking its type. Parenthesized operands deliberately fail this gate.
    pub(in crate::project::semantic) fn unique_type_operand_is_symbol(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(root.file)?;
        let node = captured.ast.node(captured.node_at(root.site)?);
        Ok(syntax::id(node, "type")
            .is_some_and(|id| captured.ast.node(id).syntax_kind == K::SymbolKeyword))
    }

    /// `WalkUpParenthesizedTypes` / `WalkUpParenthesizedExpressions` start at
    /// the producer's parent. Only the matching parenthesized family is skipped.
    pub(in crate::project::semantic) fn es_symbol_parent(
        &mut self,
        root: ExpressionRoot,
        type_position: bool,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(root.file)?;
        let mut parent = captured.parents[captured.node_at(root.site)?.index()];
        let kind = if type_position {
            K::ParenthesizedType
        } else {
            K::ParenthesizedExpression
        };
        while let Some(id) = parent {
            if captured.ast.node(id).syntax_kind != kind {
                break;
            }
            parent = captured.parents[id.index()];
        }
        Ok(parent.map(|node| DeclarationIdentity::new(root.file, node)))
    }
}
