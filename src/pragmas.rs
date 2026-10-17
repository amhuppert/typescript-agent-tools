//! Pragmas and comment directives, read as TypeScript 7 reads them: in a
//! file's leading comments, triple-slash `<reference ... />` directives,
//! `// @ts-check` and `// @ts-nocheck`, and JSX pragmas; anywhere in a file,
//! the `@ts-ignore` and `@ts-expect-error` comment directives, and the
//! blank or comment-only lines across which a directive reaches. The parser
//! validates reference directives and project loading follows them; both
//! recognize them, and which reference each makes, here. The lexer owns
//! where comments and a leading `#!` line begin and end; this module owns
//! what a pragma or directive inside a comment is.

use crate::Span;
pub(crate) use crate::lexer::Comment;
use crate::lexer::{comments, line_break, shebang_end};
use crate::resources::{Control, Stop};

/// Space and tab: the only whitespace TypeScript 7 accepts between a pragma's
/// or comment directive's parts, and at the start of a blank line a
/// directive reaches across. A vertical tab, form feed, or no-break space
/// there ends the pragma, the directive, or the blank line.
fn pragma_space(c: char) -> bool {
    matches!(c, ' ' | '\t')
}

/// A tag or attribute name: ASCII letters and `-`, as in `no-default-lib`.
fn pragma_name(text: &str) -> usize {
    pragma_name_controlled(text).expect("uncontrolled pragma name")
}

fn pragma_name_controlled(text: &str) -> Result<usize, Stop> {
    Ok(text
        .find(|c: char| !(c.is_ascii_alphabetic() || c == '-'))
        .unwrap_or(text.len()))
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
    references_controlled(text, None).map(|result| result.expect("uncontrolled pragma scan"))
}

pub(crate) fn references_controlled<'s>(
    text: &'s str,
    control: Option<&'s Control>,
) -> impl Iterator<Item = Result<Reference<'s>, Stop>> + 's {
    let mut comments = None;
    let mut stopped = false;
    std::iter::from_fn(move || {
        if stopped {
            return None;
        }
        if comments.is_none() {
            let start = match crate::lexer::shebang_end_controlled(text, control) {
                Ok(start) => start,
                Err(stop) => {
                    stopped = true;
                    return Some(Err(stop));
                }
            };
            comments = Some(crate::lexer::comments_controlled(text, start, control));
        }
        loop {
            let comment = match comments.as_mut().expect("initialized comments").next()? {
                Ok(comment) => comment,
                Err(stop) => {
                    stopped = true;
                    return Some(Err(stop));
                }
            };
            match Reference::parse(comment, control) {
                Ok(Some(reference)) => return Some(Ok(reference)),
                Ok(None) => {}
                Err(stop) => {
                    stopped = true;
                    return Some(Err(stop));
                }
            }
        }
    })
}

impl<'s> Reference<'s> {
    /// `///`, spaces or tabs, then `<` and a tag named `reference` without
    /// case. Attributes follow as `name = "value"` or `name = 'value'`, with
    /// optional spaces or tabs around each part, until the first text that is
    /// not one; the rest of the comment, including any `/>`, is ignored.
    fn parse(comment: Comment<'s>, _control: Option<&Control>) -> Result<Option<Self>, Stop> {
        let Some(body) = comment.text.strip_prefix("///") else {
            return Ok(None);
        };
        let body = body.trim_start_matches(pragma_space);
        let Some(tag) = body.strip_prefix('<') else {
            return Ok(None);
        };
        let name = pragma_name_controlled(tag)?;
        if !tag[..name].eq_ignore_ascii_case("reference") {
            return Ok(None);
        }
        let mut attributes = Vec::new();
        let mut rest = &tag[name..];
        loop {
            let text = rest.trim_start_matches(pragma_space);
            let name = &text[..pragma_name_controlled(text)?];
            if name.is_empty() {
                break;
            }
            let tail = text[name.len()..].trim_start_matches(pragma_space);
            let Some(value) = tail.strip_prefix('=') else {
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
        Ok(Some(Self {
            comment,
            attributes,
        }))
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
    #[cfg(test)]
    pub(crate) fn attribute(&self, name: &str) -> Option<(&'s str, usize)> {
        self.attribute_controlled(name)
            .expect("uncontrolled pragma attribute")
    }

    pub(crate) fn attribute_controlled(
        &self,
        name: &str,
    ) -> Result<Option<(&'s str, usize)>, Stop> {
        for attribute in self.attributes.iter().rev() {
            if attribute.name.eq_ignore_ascii_case(name) {
                return Ok(Some((attribute.value, attribute.offset)));
            }
        }
        Ok(None)
    }

    /// Which reference this directive makes, by TypeScript 7's
    /// `processPragmasIntoFields`: `no-default-lib="true"` makes the parser
    /// ignore the whole directive; otherwise the first of `types`, `lib`, and
    /// `path` that is present decides, keeping its value as written, even
    /// empty; a directive with none of them is a syntax error. The parser
    /// validates directives and project loading follows them through this
    /// one rule; `no-default-lib` has no other effect, on library selection
    /// or anything else.
    pub(crate) fn directive(&self) -> Directive<'s> {
        self.directive_controlled()
            .expect("uncontrolled pragma directive")
    }

    pub(crate) fn directive_controlled(&self) -> Result<Directive<'s>, Stop> {
        if self
            .attribute_controlled("no-default-lib")?
            .is_some_and(|(value, _)| value == "true")
        {
            return Ok(Directive::Ignored);
        }
        if let Some((name, offset)) = self.attribute_controlled("types")? {
            let mode = match self.attribute_controlled("resolution-mode")? {
                None => TypesMode::Unset,
                Some(("import", _)) => TypesMode::Import,
                Some(("require", _)) => TypesMode::Require,
                Some(_) => TypesMode::Invalid,
            };
            return Ok(Directive::Types(name, offset, mode));
        }
        if let Some((name, offset)) = self.attribute_controlled("lib")? {
            return Ok(Directive::Lib(name, offset));
        }
        if let Some((name, offset)) = self.attribute_controlled("path")? {
            return Ok(Directive::Path(name, offset));
        }
        Ok(Directive::Invalid)
    }
}

/// The reference one `<reference ... />` directive makes; each value comes
/// with its byte offset in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Directive<'s> {
    /// `no-default-lib="true"`: nothing, whatever else the directive names.
    Ignored,
    /// `types="..."`: a type package.
    Types(&'s str, usize, TypesMode),
    /// `lib="..."`: a standard library.
    Lib(&'s str, usize),
    /// `path="..."`: a file relative to the containing file.
    Path(&'s str, usize),
    /// TypeScript's "Invalid 'reference' directive syntax".
    Invalid,
}

/// A `types` directive's `resolution-mode` attribute. Any value other than
/// `import` or `require` is a syntax error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypesMode {
    Unset,
    Import,
    Require,
    Invalid,
}

/// Whether a file asks to be checked, as TypeScript 7's `CheckJsDirective`
/// records it (`extractPragmas`, `processPragmasIntoFields`): among the
/// leading line comments, `//` or `///`, then spaces or tabs, then `@` and a
/// name of ASCII letters and `-`, matched without case, that is `ts-check`
/// (`Some(true)`) or `ts-nocheck` (`Some(false)`); the last one wins. Block
/// comments carry no such pragma, nor does a reference directive.
pub(crate) fn checks(text: &str) -> Option<bool> {
    let mut checks = None;
    for comment in leading_comments(text) {
        let Some(body) = comment.text.strip_prefix("//") else {
            continue;
        };
        let triple_slash = body.starts_with('/');
        let body = body
            .strip_prefix('/')
            .unwrap_or(body)
            .trim_start_matches(pragma_space);
        if triple_slash && body.starts_with('<') {
            continue;
        }
        let Some(pragma) = body.strip_prefix('@') else {
            continue;
        };
        let name = &pragma[..pragma_name(pragma)];
        if name.eq_ignore_ascii_case("ts-check") {
            checks = Some(true);
        } else if name.eq_ignore_ascii_case("ts-nocheck") {
            checks = Some(false);
        }
    }
    checks
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

/// Where each `@ts-ignore` and `@ts-expect-error` comment directive in
/// `text` begins, in source order, as TypeScript 7's scanner records them
/// (`processCommentDirective`, below): the start of a `//` comment, or the
/// start of a block comment's last line, whose line is the directive's.
/// The scanner finds them in every comment between tokens
/// (`crate::lexer::comment_spans`, around `literals`).
pub(crate) fn comment_directives(text: &str, literals: Vec<Span>) -> Vec<usize> {
    crate::lexer::comment_spans(text, literals)
        .filter_map(|comment| {
            let multiline = text[comment.start..].starts_with("/*");
            // A block comment's directive is on its last line: TypeScript's
            // scanner moves its start past each line break in the comment.
            let line = if multiline {
                text[comment.start..comment.end]
                    .char_indices()
                    .rfind(|&(_, c)| line_break(c))
                    .map_or(comment.start, |(offset, c)| {
                        comment.start + offset + c.len_utf8()
                    })
            } else {
                comment.start
            };
            comment_directive(text, line, comment.end, multiline).then_some(line)
        })
        .collect()
}

/// Port of TypeScript 7's `processCommentDirective`
/// (internal/scanner/scanner.go): whether the comment text from `start` to
/// `end` is a directive. For a `//` comment, `start` is the comment's
/// start: its `//` and any further slashes are skipped. For a block
/// comment, `start` is the start of its last line: spaces and tabs, then
/// any run of `/` and `*`, are skipped. Then spaces and tabs, and `@`
/// followed, case-sensitively, by `ts-expect-error` or `ts-ignore`, which
/// may continue (`@ts-ignored` is a directive). Only spaces and tabs
/// separate the parts (`pragma_space`).
fn comment_directive(text: &str, start: usize, end: usize, multiline: bool) -> bool {
    let bytes = text.as_bytes();
    let skip = |mut pos: usize, skipped: fn(u8) -> bool| {
        while pos < end && skipped(bytes[pos]) {
            pos += 1;
        }
        pos
    };
    let space = |byte| pragma_space(char::from(byte));
    let pos = if multiline {
        let pos = skip(start, space);
        skip(pos, |byte| matches!(byte, b'/' | b'*'))
    } else {
        skip(start + 2, |byte| byte == b'/')
    };
    let pos = skip(pos, space);
    if !(pos < end && bytes[pos] == b'@') {
        return false;
    }
    let rest = &text[pos + 1..];
    rest.starts_with("ts-expect-error") || rest.starts_with("ts-ignore")
}

/// Port of TypeScript 7's `isCommentOrBlankLine` (internal/compiler/
/// program.go), which decides how far up a diagnostic's preceding lines
/// `getDiagnosticsWithPrecedingDirectives` looks for a comment directive:
/// whether the line starting at `line_start` in `text` is, after spaces and
/// tabs only, empty or a `//` comment. Deliberately narrower than the
/// lexer's whitespace and line breaks: a no-break space, vertical tab, form
/// feed, or byte order mark makes the line neither, and only CR, LF, or the
/// end of the text ends an empty line, so an empty line that U+2028 or
/// U+2029 ends is not blank, although it is a line of its own.
pub(crate) fn comment_or_blank_line(text: &str, line_start: usize) -> bool {
    let rest = text[line_start..].trim_start_matches(pragma_space);
    rest.is_empty() || rest.starts_with(['\r', '\n']) || rest.starts_with("//")
}

/// The directive that governs each line's diagnostics: port of
/// TypeScript 7's `getDiagnosticsWithPrecedingDirectives`
/// (internal/compiler/program.go). From the line above a diagnostic's start
/// upward, the first line holding a directive's start (`comment_directives`;
/// with several, the last) suppresses it, and the search stops at the first
/// other line that `comment_or_blank_line` does not accept, a narrower test
/// than the lexer's whitespace and line breaks. TypeScript records which
/// directive each diagnostic used, which decides whether an
/// `@ts-expect-error` is unused; so this keeps the governing directive, not
/// only whether a line is covered. It propagates down the file in one pass
/// instead of searching upward from each diagnostic. Project loading decides
/// which binding diagnostics to report with it; editing checks that an edit
/// leaves every preserved line governed by the same directive.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct DirectiveCoverage {
    /// The start of the directive governing each zero-based line; empty
    /// without directives.
    governing: Vec<Option<usize>>,
}

impl DirectiveCoverage {
    /// The coverage of `source`, whose comment directives begin at
    /// `directives`.
    pub(crate) fn new(
        source: &crate::source::SourceText,
        directives: impl IntoIterator<Item = usize>,
    ) -> Self {
        let mut directives = directives.into_iter().peekable();
        if directives.peek().is_none() {
            return Self::default();
        }
        let mut directive = vec![None; source.line_count()];
        for start in directives {
            directive[source.line_index(start)] = Some(start);
        }
        let mut governing = vec![None; directive.len()];
        for line in 1..governing.len() {
            let above = line - 1;
            governing[line] = directive[above].or_else(|| {
                governing[above]
                    .filter(|_| comment_or_blank_line(source.as_str(), source.line_start(above)))
            });
        }
        Self { governing }
    }

    /// Whether diagnostics starting on the zero-based `line` are suppressed.
    pub(crate) fn covers_line(&self, line: usize) -> bool {
        self.governing(line).is_some()
    }

    /// Where the directive that suppresses diagnostics starting on the
    /// zero-based `line` begins, if one does.
    pub(crate) fn governing(&self, line: usize) -> Option<usize> {
        self.governing.get(line).copied().flatten()
    }
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
    fn one_rule_decides_which_reference_a_directive_makes() {
        let directives = |text| -> Vec<Directive<'_>> {
            references(text)
                .map(|reference| reference.directive())
                .collect()
        };
        let text = "/// <reference no-default-lib=\"true\" lib=\"es5\" />\n/// <reference path=\"p\" lib=\"l\" types=\"t\" />\n/// <reference path=\"p\" lib=\"\" />\n/// <reference path=\"\" />\n/// <reference types=\"t\" resolution-mode=\"require\" />\n/// <reference no-default-lib=\"false\" />\n";
        let at = |needle: &str, nth: usize| {
            text.match_indices(needle).nth(nth).unwrap().0 + needle.len()
        };
        assert_eq!(
            directives(text),
            [
                Directive::Ignored,
                Directive::Types("t", at("types=\"", 0), TypesMode::Unset),
                // `no-default-lib="` also ends in `lib="`.
                Directive::Lib("", at(" lib=\"", 2)),
                Directive::Path("", at("path=\"", 2)),
                Directive::Types("t", at("types=\"", 1), TypesMode::Require),
                Directive::Invalid,
            ]
        );
    }

    #[test]
    fn the_last_leading_check_pragma_decides() {
        for (text, expected) in [
            ("let a;", None),
            ("// @ts-nocheck\n", Some(false)),
            ("/// @ts-nocheck\n", Some(false)),
            ("//\t @TS-NoCheck: generated\n", Some(false)),
            ("// @ts-nocheck\n// @ts-check\n", Some(true)),
            ("// @ts-check\n// @ts-nocheck\n", Some(false)),
            (
                "#!/usr/bin/env node\n\n// lead\n// @ts-nocheck\n",
                Some(false),
            ),
            ("// @ts-nocheck-later\n", None),
            ("//// @ts-nocheck\n", None),
            ("/* @ts-nocheck */\n", None),
            ("/// <reference path=\"a\" /> @ts-nocheck\n", None),
            ("let a;\n// @ts-nocheck\n", None),
        ] {
            assert_eq!(checks(text), expected, "{text:?}");
        }
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
