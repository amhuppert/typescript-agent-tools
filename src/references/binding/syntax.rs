//! Field access over the final syntax tree. Only nodes reachable from the root
//! are visited, and the `assertClause` compatibility alias of `attributes` is
//! never treated as a second child.

use crate::{Ast, Node, NodeId, Value};

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

/// Text of an identifier's `escapedText` or a literal's `text`. Unpaired
/// surrogates in string literals are replaced.
pub(super) fn text(node: &Node<'_>) -> String {
    match node.field("escapedText").or_else(|| node.field("text")) {
        Some(Value::Text(text)) => text.to_string(),
        Some(Value::Utf16(units)) => String::from_utf16_lossy(units),
        _ => String::new(),
    }
}

/// Every child link of a node, in field order, excluding compatibility aliases.
pub(super) fn children<'a, 's>(node: &'a Node<'s>) -> impl Iterator<Item = NodeId> + use<'a, 's> {
    node.fields
        .iter()
        .filter(|(key, _)| *key != "assertClause")
        .flat_map(|(_, value)| match value {
            Value::Node(id) => std::slice::from_ref(id),
            Value::Nodes(ids) => ids.as_slice(),
            _ => &[],
        })
        .copied()
}
