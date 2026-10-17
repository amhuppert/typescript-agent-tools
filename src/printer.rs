//! The sole TypeScript literal-printing and escape policy. Navigation and
//! semantic summaries supply exact cooked values; neither redecodes source.

/// `GetTypeNodePrecedence` values for emitted type syntax. The caller owns
/// which syntax was produced; this owner handles its position in the printer.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TypePrecedence {
    Conditional = 1,
    Function = 2,
    Union = 3,
    Intersection = 4,
    TypeOperator = 5,
    Postfix = 6,
    NonArray = 7,
}

#[derive(Clone, Copy)]
pub(crate) enum TypePosition {
    Outside,
    OutsideAt(TypePrecedence),
    Operand(TypePrecedence),
    Extends,
    ArrowReturn,
}

pub(crate) struct TypePositionState {
    pub(crate) minimum: TypePrecedence,
    pub(crate) in_extends: bool,
    arrow_return: bool,
}
impl TypePosition {
    /// Printer `emitTypeNode`, `emitTypeNodeIn/OutsideExtends`, `emitReturnType`.
    pub(crate) fn state(self, parent_in_extends: bool) -> TypePositionState {
        let (minimum, in_extends) = match self {
            Self::Outside => (TypePrecedence::Conditional, false),
            Self::OutsideAt(minimum) => (minimum, false),
            Self::Operand(minimum) => (minimum, parent_in_extends),
            Self::Extends => (TypePrecedence::Conditional, true),
            Self::ArrowReturn => (TypePrecedence::Conditional, parent_in_extends),
        };
        TypePositionState {
            minimum: if in_extends && minimum <= TypePrecedence::Conditional {
                TypePrecedence::Function
            } else {
                minimum
            },
            in_extends,
            arrow_return: matches!(self, Self::ArrowReturn),
        }
    }
}
impl TypePositionState {
    pub(crate) fn parenthesizes(
        &self,
        precedence: TypePrecedence,
        constrained_infer: bool,
    ) -> bool {
        precedence < self.minimum || self.arrow_return && self.in_extends && constrained_infer
    }
}

/// `GetTypeNodePrecedence`'s InferType decision uses the emitted constraint,
/// which may differ from its semantic constraint after node-builder omission.
pub(crate) fn infer_type_precedence(has_constraint: bool) -> TypePrecedence {
    if has_constraint {
        TypePrecedence::Function
    } else {
        TypePrecedence::TypeOperator
    }
}

/// Quote exact Unicode text with the compiler literal-printing policy.
pub(crate) fn quoted_text(text: &str, quote: char) -> String {
    quoted_bytes(text.as_bytes(), quote)
}

/// `printer.escapeStringWorker` accepts exact native bytes. Its intentional
/// stray-byte escape is output only: a raw FE prints as `\uFFFD`, while an
/// actual U+FFFD rune prints unchanged under NeverAsciiEscape.
pub(crate) fn quoted_bytes(mut bytes: &[u8], quote: char) -> String {
    let mut chars = Vec::new();
    while !bytes.is_empty() {
        let (rune, width) = crate::strings::decode_js_string_rune(bytes);
        chars.push(
            if (0xd800..=0xdfff).contains(&rune) || rune == 0xfffd && width == 1 {
                Err(rune as u16)
            } else {
                Ok(char::from_u32(rune).expect("decoded non-surrogate scalar"))
            },
        );
        bytes = &bytes[width..];
    }
    quoted(&chars, quote)
}

/// `quote` + TypeScript's `printer.EscapeString(text, quote)` + `quote`, with
/// the language service's `NeverAsciiEscape`: backslashes, the quote,
/// control characters, CR, U+2028, U+2029, and U+0085 are escaped, LF too
/// except in a template, and each unpaired surrogate as `\uXXXX`, so distinct
/// names keep distinct labels. Other characters stay as written.
pub(crate) fn quoted(chars: &[Result<char, u16>], quote: char) -> String {
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
