//! Native JSDoc scanning is a separate scanner mode, with deliberately
//! narrower whitespace/newline tokens than ordinary TypeScript scanning.

use super::{Control, Kind, Lexer, Stop, Token, id_continue, id_start, line_break, whitespace};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CommentFlags {
    pub(crate) has_jsdoc: bool,
    pub(crate) deprecated: bool,
    pub(crate) see_or_link: bool,
}

/// Native `scanJSDocCommentForTags` / `hasJSDocTag`. These scanner hints do
/// not confirm parsed tags and do not apply comment attachment filtering.
pub(crate) fn preceding_comment_flags(
    text: &str,
    full_start: usize,
    token_start: usize,
    control: Option<&Control>,
) -> Result<CommentFlags, Stop> {
    let mut flags = CommentFlags::default();
    let start = full_start.max(super::shebang_end_controlled(text, control)?);
    for comment in super::comments_controlled(&text[..token_start], start, control) {
        let comment = comment?;
        if !comment.text.starts_with("/**") || comment.text.starts_with("/**/") {
            continue;
        }
        flags.has_jsdoc = true;
        let mut rest = comment.text;
        while let Some(at) = rest.find("@") {
            rest = &rest[at + 1..];
            flags.deprecated |= has_tag(rest, "deprecated");
            flags.see_or_link |= ["see", "link", "linkcode", "linkplain"]
                .iter()
                .any(|tag| has_tag(rest, tag));
            if flags.deprecated && flags.see_or_link {
                break;
            }
        }
    }
    Ok(flags)
}

fn has_tag(text: &str, name: &str) -> bool {
    text.strip_prefix(name).is_some_and(|tail| {
        tail.is_empty()
            || matches!(
                tail.as_bytes()[0],
                b' ' | b'\t' | b'\n' | b'\r' | b'}' | b'*'
            )
    })
}

fn work(_control: Option<&Control>) -> Result<(), Stop> {
    Ok(())
}

/// The same Unicode escape grammar as ordinary identifier scanning. A failed
/// grammar match leaves the backslash as an unknown JSDoc token.
fn escaped_character(text: &str, control: Option<&Control>) -> Result<Option<(char, usize)>, Stop> {
    let parsed = super::unicode_escape(text, control);
    if let Some(control) = control {
        control.poll()?;
    }
    let Ok((_, raw)) = parsed else {
        return Ok(None);
    };
    let digits = if raw.starts_with("\\u{") {
        &raw[3..raw.len() - 1]
    } else {
        &raw[2..]
    };
    Ok(u32::from_str_radix(digits, 16)
        .ok()
        .and_then(char::from_u32)
        .map(|ch| (ch, raw.len())))
}

impl<'s> Lexer<'s> {
    /// Native `ScanJSDocToken`. Only ASCII-leading whitespace starts a
    /// whitespace token; CR/LF alone produce NewLineTrivia. Unicode line
    /// characters remain unknown here, although the text scanner stops at them.
    pub(crate) fn next_jsdoc(&mut self, control: Option<&Control>) -> Result<Token<'s>, Stop> {
        work(control)?;
        let start = self.pos;
        let Some(ch) = self.src[start..].chars().next() else {
            return Ok(self.token(Kind::Eof, start, start, false, None));
        };
        self.pos += ch.len_utf8();
        let mut kind = Kind::Punct;
        let mut newline = false;
        match ch {
            '\t' | '\u{b}' | '\u{c}' | ' ' => {
                while let Some(next) = self.src[self.pos..].chars().next() {
                    work(control)?;
                    if !whitespace(next) || line_break(next) {
                        break;
                    }
                    self.pos += next.len_utf8();
                }
            }
            '\r' => {
                if self.src[self.pos..].starts_with('\n') {
                    self.pos += 1;
                }
                newline = true;
            }
            '\n' => newline = true,
            '@' | '*' | '{' | '}' | '[' | ']' | '(' | ')' | '<' | '>' | '=' | ',' | '.' | '`'
            | '#' => {}
            '\\' => {
                if let Some((decoded, len)) = escaped_character(&self.src[start..], control)?
                    && id_start(decoded)
                {
                    self.pos = start + len;
                    self.jsdoc_identifier_parts(control)?;
                    kind = Kind::Word;
                } else {
                    kind = Kind::Unknown;
                }
            }
            _ if id_start(ch) => {
                while let Some(next) = self.src[self.pos..].chars().next() {
                    work(control)?;
                    if !id_continue(next) && next != '-' {
                        break;
                    }
                    self.pos += next.len_utf8();
                }
                if self.src[self.pos..].starts_with('\\') {
                    self.jsdoc_identifier_parts(control)?;
                }
                kind = Kind::Word;
            }
            _ => kind = Kind::Unknown,
        }
        Ok(self.token(kind, start, self.pos, newline, None))
    }

    fn jsdoc_identifier_parts(&mut self, control: Option<&Control>) -> Result<(), Stop> {
        while let Some(ch) = self.src[self.pos..].chars().next() {
            work(control)?;
            if id_continue(ch) {
                self.pos += ch.len_utf8();
            } else if ch == '\\' {
                if let Some((decoded, len)) = escaped_character(&self.src[self.pos..], control)?
                    && id_continue(decoded)
                {
                    self.pos += len;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Native `ScanJSDocCommentTextToken`: @ starts a tag only after single
    /// line whitespace and before an identifier; backticks suppress that gate.
    pub(crate) fn next_jsdoc_text(
        &mut self,
        in_backticks: bool,
        control: Option<&Control>,
    ) -> Result<Token<'s>, Stop> {
        work(control)?;
        let start = self.pos;
        while let Some(ch) = self.src[self.pos..].chars().next() {
            work(control)?;
            if line_break(ch) || ch == '`' {
                break;
            }
            if !in_backticks {
                if ch == '{' {
                    break;
                }
                if ch == '@'
                    && self.src[..self.pos]
                        .chars()
                        .next_back()
                        .is_some_and(|previous| whitespace(previous) && !line_break(previous))
                    && self.src[self.pos + 1..]
                        .chars()
                        .next()
                        .is_some_and(id_start)
                {
                    break;
                }
            }
            self.pos += ch.len_utf8();
        }
        if self.pos == start {
            self.next_jsdoc(control)
        } else {
            Ok(self.token(Kind::Unknown, start, self.pos, false, None))
        }
    }

    /// Native `CanFollowJSDocAt` includes incomplete tags for code completion.
    pub(crate) fn can_follow_jsdoc_at(&self) -> bool {
        self.src[self.pos..]
            .chars()
            .next()
            .is_none_or(|ch| id_start(ch) || whitespace(ch))
    }
}
