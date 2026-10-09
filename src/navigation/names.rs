//! Declared names: the exact optional lookup key and the display label of a
//! name node, and the labels TypeScript gives unnamed sites. Labels are
//! presentation; only `Name::key` is an identity.

use crate::syntax;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Value};

/// A declared name's label and its key, if the name has an exact one.
pub(super) struct Name {
    pub(super) label: String,
    pub(super) key: Option<String>,
}

/// TypeScript's `getTextOfName` (internal/ls/symbols.go) for the label, plus
/// the lookup key: identifiers and private names decode their escapes; a
/// string name is double-quoted and escaped, keyed by its exact value unless
/// that value has an unpaired surrogate; a numeric name is its normalized text.
/// Computed names are keyed by nothing, since the selection language has no
/// syntax for them; they keep TypeScript's label: a literal expression's
/// rendering, or else the source text.
pub(super) fn declared(ast: &Ast<'_>, name: NodeId) -> Name {
    let node = ast.node(name);
    match node.syntax_kind {
        SyntaxKind::Identifier | SyntaxKind::PrivateIdentifier => {
            let text = ast.identifier_name(name).unwrap_or_default().to_owned();
            Name {
                label: text.clone(),
                key: Some(text),
            }
        }
        SyntaxKind::StringLiteral => match node.field("text") {
            Some(Value::Text(text)) => Name {
                label: quoted(&cooked(node), '"'),
                key: Some(text.to_string()),
            },
            Some(Value::Utf16(_)) => Name {
                label: quoted(&cooked(node), '"'),
                key: None,
            },
            _ => source_label(ast, name),
        },
        SyntaxKind::NumericLiteral => match node.field("text") {
            Some(Value::Text(text)) => Name {
                label: text.to_string(),
                key: Some(text.to_string()),
            },
            _ => source_label(ast, name),
        },
        SyntaxKind::ComputedPropertyName => {
            let label = syntax::id(node, "expression")
                .and_then(|expression| literal_label(ast, expression))
                .unwrap_or_else(|| source_text(ast, name).to_owned());
            Name { label, key: None }
        }
        _ => source_label(ast, name),
    }
}

/// `getTextOfName` for a literal inside a computed name: TypeScript's
/// `IsStringOrNumericLiteralLike`.
fn literal_label(ast: &Ast<'_>, literal: NodeId) -> Option<String> {
    let node = ast.node(literal);
    let quote = match node.syntax_kind {
        SyntaxKind::NumericLiteral => {
            return match node.field("text") {
                Some(Value::Text(text)) => Some(text.to_string()),
                _ => None,
            };
        }
        SyntaxKind::StringLiteral => '"',
        SyntaxKind::NoSubstitutionTemplateLiteral => '`',
        _ => return None,
    };
    Some(quoted(&cooked(node), quote))
}

/// A string or template literal's value, keeping unpaired surrogates as
/// `Err(unit)`.
fn cooked(node: &crate::Node<'_>) -> Vec<Result<char, u16>> {
    match node.field("text") {
        Some(Value::Text(text)) => text.chars().map(Ok).collect(),
        Some(Value::Utf16(units)) => char::decode_utf16(units.iter().copied())
            .map(|unit| unit.map_err(|error| error.unpaired_surrogate()))
            .collect(),
        _ => Vec::new(),
    }
}

fn source_label(ast: &Ast<'_>, name: NodeId) -> Name {
    Name {
        label: source_text(ast, name).to_owned(),
        key: None,
    }
}

/// A node's source text without leading trivia (`scanner.GetTextOfNode`).
pub(super) fn source_text<'s>(ast: &Ast<'s>, id: NodeId) -> &'s str {
    let span = ast.token_span(id);
    &ast.source()[span.start..span.end]
}

/// TypeScript's `getUnnamedNodeLabel` label for an anonymous function: the
/// callee of a call it is passed to, with the call's string and template
/// arguments as written, as `name(args) callback`. The callee must be a name
/// or property-access chain (`getCallExpressionName`); other callees give
/// `None`. As in `cleanCallbackText`, line breaks are removed; unlike it,
/// nothing is truncated, a product rule shared by every label.
pub(super) fn callback(ast: &Ast<'_>, call: NodeId) -> Option<String> {
    let node = ast.node(call);
    let name = call_expression_name(ast, syntax::id(node, "expression")?);
    if name.is_empty() {
        return None;
    }
    let arguments: Vec<&str> = syntax::ids(node, "arguments")
        .iter()
        .filter(|&&argument| {
            matches!(
                ast.node(argument).syntax_kind,
                SyntaxKind::StringLiteral
                    | SyntaxKind::NoSubstitutionTemplateLiteral
                    | SyntaxKind::TemplateExpression
            )
        })
        .map(|&argument| source_text(ast, argument))
        .collect();
    let label = format!("{name}({}) callback", arguments.join(", "));
    Some(label.replace(crate::lexer::line_break, ""))
}

/// TypeScript's `getCallExpressionName`: an identifier or private name, or
/// a property access whose left part has such a name, joined with `.`, and
/// whose name part is one; any other left part, such as `this` or a call,
/// is dropped, leaving the names after it. TypeScript recurses down the
/// left parts and joins at each level; a member chain parses in a loop, so
/// its length is bounded only by the file, and this walks it in a loop and
/// assembles the name once.
fn call_expression_name(ast: &Ast<'_>, callee: NodeId) -> String {
    // The name part of each property access, outermost first.
    let mut parts: Vec<&str> = Vec::new();
    let mut current = callee;
    let innermost = loop {
        let node = ast.node(current);
        if node.syntax_kind != SyntaxKind::PropertyAccessExpression {
            break name_text(ast, current);
        }
        parts.push(syntax::id(node, "name").map_or("", |name| name_text(ast, name)));
        match syntax::id(node, "expression") {
            Some(left) => current = left,
            None => break "",
        }
    };
    let length = innermost.len() + parts.iter().map(|part| part.len() + 1).sum::<usize>();
    let mut name = String::with_capacity(length);
    name.push_str(innermost);
    for part in parts.into_iter().rev() {
        // `left + "." + right`, or `right` alone when `left` is empty.
        if !name.is_empty() {
            name.push('.');
        }
        name.push_str(part);
    }
    name
}

/// The text of an identifier or private name; empty for other syntax.
fn name_text<'a>(ast: &'a Ast<'_>, id: NodeId) -> &'a str {
    match ast.node(id).syntax_kind {
        SyntaxKind::Identifier | SyntaxKind::PrivateIdentifier => {
            ast.identifier_name(id).unwrap_or_default()
        }
        _ => "",
    }
}

/// `quote` + TypeScript's `printer.EscapeString(text, quote)` + `quote`, with
/// the language service's `NeverAsciiEscape`: backslashes, the quote,
/// control characters, CR, U+2028, U+2029, and U+0085 are escaped, LF too
/// except in a template, and each unpaired surrogate as `\uXXXX`, so distinct
/// names keep distinct labels. Other characters stay as written.
fn quoted(chars: &[Result<char, u16>], quote: char) -> String {
    let mut out = String::with_capacity(chars.len() + 2);
    out.push(quote);
    for (index, &c) in chars.iter().enumerate() {
        let next = chars.get(index + 1).and_then(|next| next.ok());
        let c = match c {
            Ok(c) => c,
            Err(unit) => {
                out.push_str(&format!("\\u{unit:04X}"));
                continue;
            }
        };
        match c {
            '\\' => out.push_str("\\\\"),
            '$' if quote == '`' && next == Some('{') => out.push_str("\\$"),
            '\r' if quote == '`' && next == Some('\n') => out.push_str("\\r\\n"),
            '\n' if quote == '`' && index > 0 && chars[index - 1] == Ok('\r') => {}
            '\n' if quote == '`' => out.push('\n'),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            '\t' => out.push_str("\\t"),
            '\u{000B}' => out.push_str("\\v"),
            '\u{000C}' => out.push_str("\\f"),
            '\u{0008}' => out.push_str("\\b"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\0' if next.is_some_and(|next| next.is_ascii_digit()) => out.push_str("\\x00"),
            '\0' => out.push_str("\\0"),
            '\u{2028}' | '\u{2029}' | '\u{0085}' => out.push_str(&format!("\\u{:04X}", c as u32)),
            c if c <= '\u{001F}' => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn escape(text: &str, quote: char) -> String {
        quoted(&text.chars().map(Ok).collect::<Vec<_>>(), quote)
    }

    #[test]
    fn escapes_follow_the_compilers_never_ascii_escape_printer() {
        assert_eq!(escape("a\"b'c\\", '"'), r#""a\"b'c\\""#);
        assert_eq!(
            escape("\t\u{b}\u{c}\u{8}\r\n\u{1}", '"'),
            r#""\t\v\f\b\r\n\u0001""#
        );
        assert_eq!(escape("\0x\u{0}1", '"'), r#""\0x\x001""#);
        assert_eq!(
            escape("\u{2028}\u{2029}\u{85}é😀", '"'),
            "\"\\u2028\\u2029\\u0085é😀\""
        );
        assert_eq!(escape("${a}`\n\r\n\r", '`'), "`\\${a}\\`\n\\r\\n\\r`");
        let lone = quoted(&[Err(0xD800), Ok('a'), Err(0xDC00)], '"');
        assert_eq!(lone, r#""\uD800a\uDC00""#);
    }
}
