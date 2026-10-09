//! The binder's exact name text. Binding requires every name it records to
//! be exact; navigation's optional lookup keys have their own owner and
//! never use this conversion.

use crate::{Node, Value};

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
