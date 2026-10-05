//! Triple-slash reference directives in a file's leading comments, extracted
//! as TypeScript does: a `///` line comment opening an XML-like `<reference
//! ... />` tag, with quoted attributes found anywhere in that comment.

use super::super::binding::ResolutionMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Directive {
    /// `path="..."`: a file relative to the containing file.
    Path(String),
    /// `types="..."`: a type package, with an optional `resolution-mode`.
    Types(String, Option<ResolutionMode>),
    /// `lib="..."`: a standard library.
    Lib(String),
}

/// A directive and the byte offset of its value, for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Reference {
    pub(super) directive: Directive,
    pub(super) offset: usize,
}

/// JSX pragmas in a file's leading block comments; the last of each kind
/// wins.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct JsxPragmas {
    /// `@jsxImportSource package`.
    pub(super) import_source: Option<String>,
    /// `@jsxRuntime classic` or `automatic`.
    pub(super) runtime: Option<String>,
}

/// As in TypeScript, each line of a block comment holds at most one pragma:
/// its first `@name`, matched without case, with the first word after it as
/// the argument.
pub(super) fn jsx_pragmas(text: &str) -> JsxPragmas {
    let mut pragmas = JsxPragmas::default();
    let comments = leading_comments(text)
        .into_iter()
        .filter(|comment| comment.starts_with("/*"));
    for line in comments.flat_map(|comment| comment.split(['\n', '\r', '\u{2028}', '\u{2029}'])) {
        let Some((_, pragma)) = line.split_once('@') else {
            continue;
        };
        let (name, rest) = pragma
            .split_once(char::is_whitespace)
            .unwrap_or((pragma, ""));
        let slot = if name.eq_ignore_ascii_case("jsxImportSource") {
            &mut pragmas.import_source
        } else if name.eq_ignore_ascii_case("jsxRuntime") {
            &mut pragmas.runtime
        } else {
            continue;
        };
        if let Some(value) = rest.split_whitespace().next() {
            *slot = Some(value.to_owned());
        }
    }
    pragmas
}

/// The comments before a file's first token, after any shebang.
fn leading_comments(text: &str) -> Vec<&str> {
    let mut comments = Vec::new();
    let mut pos = 0;
    if text.starts_with("#!") {
        pos = text.find(['\n', '\r']).unwrap_or(text.len());
    }
    loop {
        let rest = &text[pos..];
        let trimmed = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        pos += rest.len() - trimmed.len();
        if trimmed.starts_with("//") {
            let end = trimmed.find(['\n', '\r']).unwrap_or(trimmed.len());
            comments.push(&trimmed[..end]);
            pos += end;
        } else if let Some(comment) = trimmed.strip_prefix("/*") {
            match comment.find("*/") {
                Some(end) => {
                    comments.push(&trimmed[..end + 4]);
                    pos += end + 4;
                }
                None => break,
            }
        } else {
            break;
        }
    }
    comments
}

pub(super) fn scan(text: &str) -> Vec<Reference> {
    let mut references = Vec::new();
    let mut pos = 0;
    if text.starts_with("#!") {
        pos = text.find(['\n', '\r']).unwrap_or(text.len());
    }
    loop {
        let rest = &text[pos..];
        let trimmed = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        pos += rest.len() - trimmed.len();
        if trimmed.starts_with("//") {
            let end = trimmed.find(['\n', '\r']).unwrap_or(trimmed.len());
            references.extend(reference(&trimmed[..end], pos));
            pos += end;
        } else if let Some(comment) = trimmed.strip_prefix("/*") {
            match comment.find("*/") {
                Some(end) => pos += end + 4,
                None => break,
            }
        } else {
            break;
        }
    }
    references
}

/// The directive in one line comment beginning at byte `start`.
fn reference(comment: &str, start: usize) -> Option<Reference> {
    let tag = comment.strip_prefix("///")?.trim_start();
    let tag = tag.strip_prefix('<')?;
    let name_end = tag.find(char::is_whitespace)?;
    if !tag[..name_end].eq_ignore_ascii_case("reference") || !tag[name_end..].contains("/>") {
        return None;
    }
    let argument = |name: &str| attribute(comment, name).filter(|(value, _)| !value.is_empty());
    if attribute(comment, "no-default-lib").is_some_and(|(value, _)| value == "true") {
        return None;
    }
    let (directive, offset) = if let Some((name, offset)) = argument("types") {
        let mode = match attribute(comment, "resolution-mode").map(|(value, _)| value) {
            Some("import") => Some(ResolutionMode::Import),
            Some("require") => Some(ResolutionMode::Require),
            _ => None,
        };
        (Directive::Types(name.into(), mode), offset)
    } else if let Some((name, offset)) = argument("lib") {
        (Directive::Lib(name.into()), offset)
    } else if let Some((path, offset)) = argument("path") {
        (Directive::Path(path.into()), offset)
    } else {
        return None;
    };
    Some(Reference {
        directive,
        offset: start + offset,
    })
}

/// The first `name = "value"` or `name = 'value'` preceded by whitespace,
/// matching the name case-insensitively, with the value's offset.
fn attribute<'a>(comment: &'a str, name: &str) -> Option<(&'a str, usize)> {
    let lower = comment.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(name) {
        let at = from + found;
        from = at + 1;
        if !comment[..at].ends_with(char::is_whitespace) {
            continue;
        }
        let after = &comment[at + name.len()..];
        let Some(after) = after.trim_start().strip_prefix('=') else {
            continue;
        };
        let value = after.trim_start();
        let Some(quote) = value.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let Some(length) = value[1..].find(quote) else {
            continue;
        };
        let offset = comment.len() - value.len() + 1;
        return Some((&value[1..1 + length], offset));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_reference_directives_from_leading_comments_only() {
        let text = "#!/usr/bin/env node\n/* lead */\n/// <reference path=\"./a.ts\" />\n///<REFERENCE types='pkg' resolution-mode=\"require\"/>\n/// <reference lib=\"es2015\" />\n/// <reference no-default-lib=\"true\" />\n/// <reference path=\"\" />\n/// <amd-module name=\"x\" />\n/// <reference path=\"open\"\nconst a = 1;\n/// <reference path=\"late.ts\" />\n";
        let references = scan(text);
        assert_eq!(
            references
                .iter()
                .map(|reference| reference.directive.clone())
                .collect::<Vec<_>>(),
            [
                Directive::Path("./a.ts".into()),
                Directive::Types("pkg".into(), Some(ResolutionMode::Require)),
                Directive::Lib("es2015".into()),
            ]
        );
        assert_eq!(
            &text[references[0].offset..references[0].offset + 6],
            "./a.ts"
        );
        assert_eq!(&text[references[1].offset..references[1].offset + 3], "pkg");
    }
}
