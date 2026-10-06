//! Typed field access over syntax nodes. `Ast::walk` and `Ast::children`
//! traverse the final tree.

use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Node, NodeId, Value};

/// A punctuation or keyword token: TypeScript's `FirstPunctuation` through
/// `LastPunctuation` and `FirstKeyword` through `LastKeyword`. Such a leaf
/// names nothing, whatever syntax it appears in.
pub(super) fn is_token(kind: SyntaxKind) -> bool {
    let kind = kind.number();
    let range = |first: SyntaxKind, last: SyntaxKind| first.number()..=last.number();
    range(SyntaxKind::OpenBraceToken, SyntaxKind::CaretEqualsToken).contains(&kind)
        || range(SyntaxKind::BreakKeyword, SyntaxKind::DeferKeyword).contains(&kind)
}

pub(super) fn child<'a, 'src>(
    ast: &'a Ast<'src>,
    node: &Node<'src>,
    field: &str,
) -> Option<&'a Node<'src>> {
    id(node, field).map(|id| ast.node(id))
}

pub(super) fn id(node: &Node<'_>, field: &str) -> Option<NodeId> {
    match node.field(field) {
        Some(Value::Node(id)) => Some(*id),
        _ => None,
    }
}

pub(super) fn ids<'a>(node: &'a Node<'_>, field: &str) -> &'a [NodeId] {
    match node.field(field) {
        Some(Value::Nodes(ids)) => ids,
        _ => &[],
    }
}

pub(super) fn flag(node: &Node<'_>, field: &str) -> bool {
    matches!(node.field(field), Some(Value::Bool(true)))
}

pub(super) fn number(node: &Node<'_>, field: &str) -> Option<u32> {
    match node.field(field) {
        Some(Value::Number(value)) => Some(*value),
        _ => None,
    }
}

/// The exact text of an identifier's `escapedText` or a literal's `text`, or
/// empty for a node with neither. A string literal with an unpaired UTF-16
/// surrogate has no exact Rust form, and replacing the surrogate would make
/// distinct names equal, so it is an error holding display text for messages,
/// with each unpaired surrogate written as a `\uXXXX` escape.
pub(super) fn exact_text<'n>(node: &'n Node<'_>) -> Result<&'n str, String> {
    match node.field("escapedText").or_else(|| node.field("text")) {
        Some(Value::Text(text)) => Ok(text),
        Some(Value::Utf16(units)) => Err(char::decode_utf16(units.iter().copied())
            .map(|unit| match unit {
                Ok(c) => c.to_string(),
                Err(error) => format!("\\u{:04X}", error.unpaired_surrogate()),
            })
            .collect()),
        _ => Ok(""),
    }
}

/// Whether a node's exact text is `expected`.
pub(super) fn is_text(node: &Node<'_>, expected: &str) -> bool {
    exact_text(node) == Ok(expected)
}
