//! The compiler's distinct optional-chain syntax questions share exact kinds,
//! flags and parent roles. Flow consumers do not reconstruct these decisions.

use super::id;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId};

/// TypeScript `IsOptionalChain`: the flag alone does not establish a chain.
pub(crate) fn is_optional_chain(ast: &Ast<'_>, node: NodeId) -> bool {
    let node = ast.node(node);
    node.flags & crate::compiler::node::OPTIONAL_CHAIN != 0
        && matches!(
            node.syntax_kind,
            SyntaxKind::PropertyAccessExpression
                | SyntaxKind::ElementAccessExpression
                | SyntaxKind::CallExpression
                | SyntaxKind::NonNullExpression
        )
}

/// TypeScript `IsOptionalChainRoot`: a NonNull chain is never a root.
pub(crate) fn is_optional_chain_root(ast: &Ast<'_>, node: NodeId) -> bool {
    is_optional_chain(ast, node)
        && ast.node(node).syntax_kind != SyntaxKind::NonNullExpression
        && id(ast.node(node), "questionDotToken").is_some()
}

/// TypeScript `IsOutermostOptionalChain`: new roots, non-chain parents and
/// arguments end the current expression; ordinary receiver continuation does not.
pub(crate) fn is_outermost_optional_chain(
    ast: &Ast<'_>,
    node: NodeId,
    parent: Option<NodeId>,
) -> bool {
    parent.is_none_or(|parent| {
        !is_optional_chain(ast, parent)
            || is_optional_chain_root(ast, parent)
            || id(ast.node(parent), "expression") != Some(node)
    })
}

/// TypeScript `IsExpressionOfOptionalChainRoot` asks about the receiver of a
/// root, rather than whether the receiver itself is an optional-chain root.
pub(crate) fn is_expression_of_optional_chain_root(
    ast: &Ast<'_>,
    node: NodeId,
    parent: Option<NodeId>,
) -> bool {
    parent.is_some_and(|parent| {
        is_optional_chain_root(ast, parent) && id(ast.node(parent), "expression") == Some(node)
    })
}
