//! Typed field access over syntax nodes, shared by binding and declaration
//! projection. `Ast::walk` and `Ast::children` traverse the final tree.
//! Name text whose identity matters has its own owners: binding's exact
//! names and navigation's optional lookup keys.

use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Node, NodeId, Value};

/// A punctuation or keyword token: TypeScript's `FirstPunctuation` through
/// `LastPunctuation` and `FirstKeyword` through `LastKeyword`. Such a leaf
/// names nothing, whatever syntax it appears in.
pub(crate) fn is_token(kind: SyntaxKind) -> bool {
    let kind = kind.number();
    let range = |first: SyntaxKind, last: SyntaxKind| first.number()..=last.number();
    range(SyntaxKind::OpenBraceToken, SyntaxKind::CaretEqualsToken).contains(&kind)
        || range(SyntaxKind::BreakKeyword, SyntaxKind::DeferKeyword).contains(&kind)
}

pub(crate) fn child<'a, 'src>(
    ast: &'a Ast<'src>,
    node: &Node<'src>,
    field: &str,
) -> Option<&'a Node<'src>> {
    id(node, field).map(|id| ast.node(id))
}

pub(crate) fn id(node: &Node<'_>, field: &str) -> Option<NodeId> {
    match node.field(field) {
        Some(Value::Node(id)) => Some(*id),
        _ => None,
    }
}

pub(crate) fn ids<'a>(node: &'a Node<'_>, field: &str) -> &'a [NodeId] {
    match node.field(field) {
        Some(Value::Nodes(ids)) => ids,
        _ => &[],
    }
}

pub(crate) fn flag(node: &Node<'_>, field: &str) -> bool {
    matches!(node.field(field), Some(Value::Bool(true)))
}

pub(crate) fn number(node: &Node<'_>, field: &str) -> Option<u32> {
    match node.field(field) {
        Some(Value::Number(value)) => Some(*value),
        _ => None,
    }
}

/// Whether a declaration's `modifiers` include a `kind` token.
pub(crate) fn has_modifier<'s>(ast: &Ast<'s>, node: &Node<'s>, kind: SyntaxKind) -> bool {
    ids(node, "modifiers")
        .iter()
        .any(|&id| ast.node(id).syntax_kind == kind)
}

/// Port of TypeScript's `SkipParentheses` (internal/ast/utilities.go): the
/// expression inside any number of parentheses around `id`, looking down
/// through parenthesized expressions only, never through assertions or
/// other wrappers. Its upward counterpart, `WalkUpParenthesizedExpressions`,
/// answers a different question and is not this operation.
pub(crate) fn skip_parentheses(ast: &Ast<'_>, mut id: NodeId) -> NodeId {
    while ast.node(id).syntax_kind == SyntaxKind::ParenthesizedExpression {
        match self::id(ast.node(id), "expression") {
            Some(inner) => id = inner,
            None => break,
        }
    }
    id
}

/// Whether a node of `kind` collects as JSDoc the comments trailing on the
/// line before it, as well as those leading it: the first half of
/// TypeScript's `GetJSDocCommentRanges` (internal/parser/utilities.go), whose
/// comment collection is `crate::lexer::jsdoc_comments`.
pub(crate) fn trailing_jsdoc(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Parameter
            | SyntaxKind::TypeParameter
            | SyntaxKind::FunctionExpression
            | SyntaxKind::ArrowFunction
            | SyntaxKind::ParenthesizedExpression
            | SyntaxKind::VariableDeclaration
            | SyntaxKind::ExportSpecifier
    )
}
