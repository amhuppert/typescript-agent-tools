//! Pragmas in a file's leading comments, read as TypeScript 7 reads them:
//! triple-slash `<reference ... />` directives and JSX pragmas. The parser
//! validates reference directives and project loading follows them; both
//! recognize them here. The lexer owns where comments and a leading `#!`
//! line begin and end; this module owns what a pragma inside a comment is.

use crate::Span;
pub(crate) use crate::lexer::Comment;
use crate::lexer::{comments, line_break, shebang_end};

/// Space and tab: the only whitespace TypeScript 7 accepts between a pragma's
/// parts. A vertical tab, form feed, or no-break space there ends the pragma.
fn pragma_space(c: char) -> bool {
    matches!(c, ' ' | '\t')
}

/// A tag or attribute name: ASCII letters and `-`, as in `no-default-lib`.
fn pragma_name(text: &str) -> usize {
    text.find(|c: char| !(c.is_ascii_alphabetic() || c == '-'))
        .unwrap_or(text.len())
}

/// The comments before the first token, after a leading `#!` line.
pub(crate) fn leading_comments(text: &str) -> impl Iterator<Item = Comment<'_>> {
    comments(text, shebang_end(text))
}

/// A triple-slash `<reference ...>` directive in one line comment, with the
/// attributes it was written with.
#[derive(Debug, Clone)]
pub(crate) struct Reference<'s> {
    comment: Comment<'s>,
    attributes: Vec<Attribute<'s>>,
}

#[derive(Debug, Clone, Copy)]
struct Attribute<'s> {
    name: &'s str,
    value: &'s str,
    /// The value's byte offset in the file.
    offset: usize,
}

/// The reference directives among the leading comments, in source order.
pub(crate) fn references(text: &str) -> impl Iterator<Item = Reference<'_>> {
    leading_comments(text).filter_map(Reference::parse)
}

impl<'s> Reference<'s> {
    /// `///`, spaces or tabs, then `<` and a tag named `reference` without
    /// case. Attributes follow as `name = "value"` or `name = 'value'`, with
    /// optional spaces or tabs around each part, until the first text that is
    /// not one; the rest of the comment, including any `/>`, is ignored.
    fn parse(comment: Comment<'s>) -> Option<Self> {
        let tag = comment
            .text
            .strip_prefix("///")?
            .trim_start_matches(pragma_space)
            .strip_prefix('<')?;
        let name = pragma_name(tag);
        if !tag[..name].eq_ignore_ascii_case("reference") {
            return None;
        }
        let mut attributes = Vec::new();
        let mut rest = &tag[name..];
        loop {
            let text = rest.trim_start_matches(pragma_space);
            let name = &text[..pragma_name(text)];
            if name.is_empty() {
                break;
            }
            let Some(value) = text[name.len()..]
                .trim_start_matches(pragma_space)
                .strip_prefix('=')
            else {
                break;
            };
            let value = value.trim_start_matches(pragma_space);
            let Some(quote) = value.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
                break;
            };
            let Some(length) = value[1..].find(quote) else {
                break;
            };
            attributes.push(Attribute {
                name,
                value: &value[1..1 + length],
                offset: comment.start + comment.text.len() - value.len() + 1,
            });
            rest = &value[length + 2..];
        }
        Some(Self {
            comment,
            attributes,
        })
    }

    /// The directive's comment, for diagnostics.
    pub(crate) fn span(&self) -> Span {
        Span {
            start: self.comment.start,
            end: self.comment.start + self.comment.text.len(),
        }
    }

    /// The value of the last attribute named `name`, matched without case,
    /// with its byte offset in the file.
    pub(crate) fn attribute(&self, name: &str) -> Option<(&'s str, usize)> {
        self.attributes
            .iter()
            .rev()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
            .map(|attribute| (attribute.value, attribute.offset))
    }
}

/// JSX pragmas in a file's leading block comments; the last of each kind
/// wins.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct JsxPragmas {
    /// `@jsxImportSource package`.
    pub(crate) import_source: Option<String>,
    /// `@jsxRuntime classic` or `automatic`.
    pub(crate) runtime: Option<String>,
}

/// Each line of a leading block comment holds at most one pragma: its first
/// `@`, a name up to the next space or tab, matched without case, then an
/// argument after spaces or tabs, up to the next one.
pub(crate) fn jsx_pragmas(text: &str) -> JsxPragmas {
    let mut pragmas = JsxPragmas::default();
    let bodies = leading_comments(text)
        .filter_map(|comment| comment.text.strip_prefix("/*")?.strip_suffix("*/"));
    for line in bodies.flat_map(|body| body.split(line_break)) {
        let Some((_, pragma)) = line.split_once('@') else {
            continue;
        };
        let (name, rest) = pragma.split_at(pragma.find(pragma_space).unwrap_or(pragma.len()));
        let slot = if name.eq_ignore_ascii_case("jsxImportSource") {
            &mut pragmas.import_source
        } else if name.eq_ignore_ascii_case("jsxRuntime") {
            &mut pragmas.runtime
        } else {
            continue;
        };
        let rest = rest.trim_start_matches(pragma_space);
        let value = &rest[..rest.find(pragma_space).unwrap_or(rest.len())];
        if !value.is_empty() {
            *slot = Some(value.to_owned());
        }
    }
    pragmas
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(text: &str) -> Vec<&str> {
        references(text)
            .filter_map(|reference| reference.attribute("path"))
            .map(|(value, _)| value)
            .collect()
    }

    #[test]
    fn every_line_break_ends_a_line_comment() {
        for line_break in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
            let text = format!(
                "// lead{line_break}/// <reference path=\"./a.ts\" />{line_break}/// <reference path=\"./b.ts\" />{line_break}let x;"
            );
            assert_eq!(paths(&text), ["./a.ts", "./b.ts"], "{line_break:?}");
        }
    }

    #[test]
    fn leading_comments_stop_at_the_first_token() {
        let text = "#!/usr/bin/env node\u{2028}\u{feff}/* a */\n/// <reference path=\"x\" />\nconst a = 1;\n/// <reference path=\"late\" />\n";
        assert_eq!(paths(text), ["x"]);
        let reference = references(text).next().unwrap();
        let (value, offset) = reference.attribute("path").unwrap();
        assert_eq!(&text[offset..offset + value.len()], "x");
        assert_eq!(
            &text[reference.span().start..reference.span().end],
            "/// <reference path=\"x\" />"
        );
    }

    #[test]
    fn a_reference_tag_is_a_letter_run_and_needs_no_closing_slash() {
        assert!(paths("///<reference/>").is_empty());
        assert_eq!(paths("/// <reference path=\"open\""), ["open"]);
        assert!(paths("/// <amd-module path=\"x\" />").is_empty());
        assert!(paths("/// <referencex path=\"x\" />").is_empty());
        assert_eq!(paths("///<REFERENCE PATH='y'/>"), ["y"]);
        assert_eq!(paths("/// <reference path='a' Path='b' />"), ["b"]);
    }

    #[test]
    fn jsx_pragmas_read_leading_block_comments() {
        let pragmas = jsx_pragmas(
            "/* @jsxImportSource preact\u{2028} @jsxRuntime automatic */\n// @jsxRuntime classic\n",
        );
        assert_eq!(pragmas.import_source.as_deref(), Some("preact"));
        assert_eq!(pragmas.runtime.as_deref(), Some("automatic"));
    }
}
