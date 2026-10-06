//! On-demand nom scanner. Regex, templates and JSX are rescanned in the
//! grammatical context supplied by the parser, rather than guessed lexically.

use nom::{
    IResult, Parser as _,
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{anychar, char},
    combinator::recognize,
    sequence::{delimited, pair},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Eof,
    Word,
    Private,
    Number,
    String,
    TemplateHead,
    TemplateMiddle,
    TemplateTail,
    NoSubstitutionTemplate,
    Punct,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Token<'src> {
    pub kind: Kind,
    pub raw: &'src str,
    pub start: usize,
    pub full_start: usize,
    pub end: usize,
    pub newline: bool,
    pub error: Option<&'static str>,
}

#[derive(Clone, Copy)]
pub(crate) struct Lexer<'src> {
    pub src: &'src str,
    pub pos: usize,
}

pub(crate) fn line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}
pub(crate) fn whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\u{000B}'
            | '\u{000C}'
            | ' '
            | '\u{0085}'
            | '\u{00A0}'
            | '\u{FEFF}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200B}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    ) || line_break(c)
}

/// Where a leading `#!` line ends, or 0 without one. A byte order mark may
/// precede it: TypeScript removes the mark when it reads a file, so its
/// `getShebang` sees the `#!` at the start.
pub(crate) fn shebang_end(text: &str) -> usize {
    let start = if text.starts_with('\u{FEFF}') { 3 } else { 0 };
    if !text[start..].starts_with("#!") {
        return 0;
    }
    text[start..]
        .find(line_break)
        .map_or(text.len(), |length| start + length)
}

/// Where a comment starting at `pos` ends: a `//` comment before its line
/// break, a `/* */` comment after its `*/`. `None` when no comment starts at
/// `pos` or a block comment is unterminated.
pub(crate) fn comment_end(text: &str, pos: usize) -> Option<usize> {
    let rest = &text[pos..];
    if rest.starts_with("//") {
        Some(pos + rest.find(line_break).unwrap_or(rest.len()))
    } else if let Some(body) = rest.strip_prefix("/*") {
        Some(pos + body.find("*/")? + 4)
    } else {
        None
    }
}

/// One comment in trivia.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Comment<'s> {
    /// The comment, including its delimiters.
    pub(crate) text: &'s str,
    pub(crate) start: usize,
    /// The whitespace between the previous comment, or the scan's start, and
    /// this one.
    pub(crate) space_before: &'s str,
}

/// The comments in the trivia from `pos` until the next token, as
/// TypeScript's `iterateCommentRanges` finds them. Consumers apply their own
/// selection: which comments count, and which whitespace may separate them.
pub(crate) fn comments(text: &str, mut pos: usize) -> impl Iterator<Item = Comment<'_>> {
    std::iter::from_fn(move || {
        let rest = &text[pos..];
        let start = pos + rest.len() - rest.trim_start_matches(whitespace).len();
        let end = comment_end(text, start)?;
        let comment = Comment {
            text: &text[start..end],
            start,
            space_before: &text[pos..start],
        };
        pos = end;
        Some(comment)
    })
}

pub(crate) fn id_start(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphabetic() || c == '$' || c == '_'
    } else {
        unicode_id_start::is_id_start(c) && !crate::identifier_exclusions::start(c)
    }
}
pub(crate) fn id_continue(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphanumeric() || c == '$' || c == '_'
    } else {
        c == '\u{200C}'
            || c == '\u{200D}'
            || unicode_id_start::is_id_continue(c) && !crate::identifier_exclusions::part(c)
    }
}

fn unicode_escape(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        tag("\\u"),
        alt((
            delimited(
                char('{'),
                take_while1(|c: char| c.is_ascii_hexdigit()),
                char('}'),
            ),
            nom::bytes::complete::take_while_m_n(4, 4, |c: char| c.is_ascii_hexdigit()),
        )),
    ))
    .parse(input)
}

fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((
            recognize(nom::character::complete::satisfy(id_start)),
            unicode_escape,
        )),
        nom::multi::many0(alt((take_while1(id_continue), unicode_escape))),
    ))
    .parse(input)
}

fn quoted(input: &str, quote: char) -> IResult<&str, &str> {
    recognize(delimited(
        char(quote),
        nom::combinator::opt(nom::bytes::complete::escaped(
            take_while1(move |c| c != quote && c != '\\' && c != '\r' && c != '\n'),
            '\\',
            alt((tag("\r\n"), recognize(anychar))),
        )),
        char(quote),
    ))
    .parse(input)
}

impl<'src> Lexer<'src> {
    fn token(
        &mut self,
        kind: Kind,
        start: usize,
        end: usize,
        newline: bool,
        error: Option<&'static str>,
    ) -> Token<'src> {
        self.pos = end;
        Token {
            kind,
            raw: &self.src[start..end],
            start,
            full_start: start,
            end,
            newline,
            error,
        }
    }

    pub fn next(&mut self) -> Token<'src> {
        let full_start = self.pos;
        let mut token = self.scan_next();
        token.full_start = full_start;
        token
    }

    fn scan_next(&mut self) -> Token<'src> {
        let mut newline = false;
        loop {
            let rest = &self.src[self.pos..];
            let (tail, space) = take_while::<_, _, nom::error::Error<&str>>(whitespace)
                .parse(rest)
                .unwrap();
            newline |= space.chars().any(line_break);
            self.pos = self.src.len() - tail.len();
            if self.pos <= 3 && tail.starts_with("#!") && shebang_end(self.src) > self.pos {
                self.pos = shebang_end(self.src);
            } else if tail.starts_with("//") || tail.starts_with("/*") {
                if let Some(end) = comment_end(self.src, self.pos) {
                    newline |= self.src[self.pos..end].chars().any(line_break);
                    self.pos = end;
                } else {
                    let start = self.pos;
                    return self.token(
                        Kind::Eof,
                        start,
                        self.src.len(),
                        newline,
                        Some("Unterminated block comment"),
                    );
                }
            } else {
                break;
            }
        }
        let start = self.pos;
        let input = &self.src[start..];
        let Some(first) = input.chars().next() else {
            return self.token(Kind::Eof, start, start, newline, None);
        };
        if first == '`' {
            return self.template(start, false, newline);
        }
        if first == '"' || first == '\'' {
            return match quoted(input, first) {
                Ok((rest, _)) => self.token(
                    Kind::String,
                    start,
                    self.src.len() - rest.len(),
                    newline,
                    None,
                ),
                Err(_) => {
                    let end = input[1..]
                        .find(line_break)
                        .map(|i| start + i + 1)
                        .unwrap_or(self.src.len());
                    self.token(
                        Kind::String,
                        start,
                        end,
                        newline,
                        Some("Unterminated string literal"),
                    )
                }
            };
        }
        if id_start(first) || first == '\\' || first == '#' {
            let prefix = usize::from(first == '#');
            if let Ok((rest, _)) = identifier(&input[prefix..]) {
                return self.token(
                    if prefix == 1 {
                        Kind::Private
                    } else {
                        Kind::Word
                    },
                    start,
                    self.src.len() - rest.len(),
                    newline,
                    None,
                );
            }
        }
        if first.is_ascii_digit()
            || (first == '.' && input.as_bytes().get(1).is_some_and(u8::is_ascii_digit))
        {
            return self.number(start, newline);
        }
        // Longest match first. Greater-than tokens are split by type argument
        // parsing without rescanning or allocating another token.
        let candidates: &[&str] = match first {
            '>' => &[">>>=", ">>>", ">>=", ">>", ">="],
            '<' => &["<<=", "<<", "<="],
            '=' => &["===", "=>", "=="],
            '!' => &["!==", "!="],
            '*' => &["**=", "**", "*="],
            '&' => &["&&=", "&&", "&="],
            '|' => &["||=", "||", "|="],
            '?' => &["??=", "??", "?."],
            '.' => &["..."],
            '+' => &["++", "+="],
            '-' => &["--", "-="],
            '/' => &["/="],
            '%' => &["%="],
            '^' => &["^="],
            _ => &[],
        };
        for punct in candidates.iter().copied() {
            if input.starts_with(punct)
                && !(punct == "?." && input.as_bytes().get(2).is_some_and(u8::is_ascii_digit))
            {
                return self.token(Kind::Punct, start, start + punct.len(), newline, None);
            }
        }
        if "{}()[];:,.?~+-*/%<>=!&|^@".contains(first) {
            return self.token(Kind::Punct, start, start + first.len_utf8(), newline, None);
        }
        self.token(
            Kind::Unknown,
            start,
            start + first.len_utf8(),
            newline,
            Some("Invalid character"),
        )
    }

    fn number(&mut self, start: usize, newline: bool) -> Token<'src> {
        let input = &self.src[start..];
        let bytes = input.as_bytes();
        let mut pos;
        let mut error = None;
        let mut fractional = false;
        if bytes[0] == b'0'
            && bytes
                .get(1)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'_')
        {
            error =
                Some("Legacy octal literals and separators after a leading zero are not allowed");
        }
        if bytes[0] == b'0'
            && bytes
                .get(1)
                .is_some_and(|c| matches!(c, b'x' | b'X' | b'o' | b'O' | b'b' | b'B'))
        {
            let radix = match bytes[1] {
                b'x' | b'X' => 16,
                b'o' | b'O' => 8,
                _ => 2,
            };
            pos = 2;
            let (rest, digits) = take_while::<_, _, nom::error::Error<&str>>(|c: char| {
                c.is_digit(radix) || c == '_'
            })
            .parse(&input[pos..])
            .unwrap();
            pos = input.len() - rest.len();
            if !valid_digits(digits) {
                error = Some("Invalid digits or numeric separator");
            }
        } else {
            let (rest, digits) = take_while::<_, _, nom::error::Error<&str>>(|c: char| {
                c.is_ascii_digit() || c == '_'
            })
            .parse(input)
            .unwrap();
            pos = input.len() - rest.len();
            if !digits.is_empty() && !valid_digits(digits) {
                error = Some("Invalid numeric separator");
            }
            if bytes.get(pos) == Some(&b'.') {
                fractional = true;
                pos += 1;
                let (rest, digits) = take_while::<_, _, nom::error::Error<&str>>(|c: char| {
                    c.is_ascii_digit() || c == '_'
                })
                .parse(&input[pos..])
                .unwrap();
                pos = input.len() - rest.len();
                if !digits.is_empty() && !valid_digits(digits) {
                    error = Some("Invalid numeric separator");
                }
            }
            if bytes.get(pos).is_some_and(|c| *c == b'e' || *c == b'E') {
                fractional = true;
                pos += 1;
                if bytes.get(pos).is_some_and(|c| *c == b'+' || *c == b'-') {
                    pos += 1;
                }
                let (rest, digits) = take_while::<_, _, nom::error::Error<&str>>(|c: char| {
                    c.is_ascii_digit() || c == '_'
                })
                .parse(&input[pos..])
                .unwrap();
                pos = input.len() - rest.len();
                if !valid_digits(digits) {
                    error = Some("Invalid numeric exponent");
                }
            }
        }
        if bytes.get(pos) == Some(&b'n') {
            if fractional {
                error = Some("BigInt literal must be an integer");
            }
            pos += 1;
        }
        if input[pos..].chars().next().is_some_and(id_continue) {
            error = Some("Identifier cannot immediately follow a number");
        }
        self.token(Kind::Number, start, start + pos, newline, error)
    }

    pub fn template(&mut self, start: usize, continuation: bool, newline: bool) -> Token<'src> {
        let bytes = self.src.as_bytes();
        let mut pos = start + 1;
        while pos < bytes.len() {
            match bytes[pos] {
                b'\\' => {
                    pos += 1;
                    if pos < bytes.len() {
                        pos += self.src[pos..].chars().next().unwrap().len_utf8();
                    }
                }
                b'`' => {
                    return self.token(
                        if continuation {
                            Kind::TemplateTail
                        } else {
                            Kind::NoSubstitutionTemplate
                        },
                        start,
                        pos + 1,
                        newline,
                        None,
                    );
                }
                b'$' if bytes.get(pos + 1) == Some(&b'{') => {
                    return self.token(
                        if continuation {
                            Kind::TemplateMiddle
                        } else {
                            Kind::TemplateHead
                        },
                        start,
                        pos + 2,
                        newline,
                        None,
                    );
                }
                _ => pos += self.src[pos..].chars().next().unwrap().len_utf8(),
            }
        }
        self.token(
            Kind::TemplateTail,
            start,
            self.src.len(),
            newline,
            Some("Unterminated template literal"),
        )
    }

    pub fn regex(&mut self, start: usize) -> Token<'src> {
        let mut pos = start + 1;
        let bytes = self.src.as_bytes();
        let mut in_class = false;
        while pos < bytes.len() {
            let c = self.src[pos..].chars().next().unwrap();
            if line_break(c) {
                break;
            }
            match c {
                '\\' => {
                    pos += 1;
                    if pos < bytes.len() {
                        pos += self.src[pos..].chars().next().unwrap().len_utf8();
                    }
                }
                '[' => {
                    in_class = true;
                    pos += 1;
                }
                ']' => {
                    in_class = false;
                    pos += 1;
                }
                '/' if !in_class => {
                    pos += 1;
                    let (rest, _) = take_while::<_, _, nom::error::Error<&str>>(id_continue)
                        .parse(&self.src[pos..])
                        .unwrap();
                    return self.token(
                        Kind::String,
                        start,
                        self.src.len() - rest.len(),
                        false,
                        None,
                    );
                }
                _ => pos += c.len_utf8(),
            }
        }
        self.token(
            Kind::Unknown,
            start,
            pos,
            false,
            Some("Unterminated regular expression literal"),
        )
    }
}

fn valid_digits(digits: &str) -> bool {
    !digits.is_empty()
        && !digits.starts_with('_')
        && !digits.ends_with('_')
        && !digits.contains("__")
}

#[cfg(test)]
mod tests {
    #[test]
    fn identifiers_match_the_native_compiler_across_unicode() {
        #[derive(serde::Deserialize)]
        struct Ranges {
            start: Vec<[u32; 2]>,
            part: Vec<[u32; 2]>,
        }
        let golden: Ranges =
            serde_json::from_str(include_str!("../fixtures/identifier-ranges.json")).unwrap();
        let ranges = [&golden.start, &golden.part];
        let mut cursor = [0, 0];
        for cp in 0..0x110000 {
            let mut expected = 0;
            for bit in 0..2 {
                while cursor[bit] < ranges[bit].len() && ranges[bit][cursor[bit]][1] < cp {
                    cursor[bit] += 1;
                }
                if cursor[bit] < ranges[bit].len() && ranges[bit][cursor[bit]][0] <= cp {
                    expected |= 1 << bit;
                }
            }
            let actual = char::from_u32(cp).map_or(0, |c| {
                u8::from(super::id_start(c)) | (u8::from(super::id_continue(c)) << 1)
            });
            assert_eq!(actual, expected, "Identifier classification at U+{cp:04X}");
        }
    }
}
