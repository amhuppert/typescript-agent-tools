//! The JSON of `tsconfig.json` and `package.json`, each read as TypeScript 7
//! reads it (`Dialect`). Values keep the byte offset where they start so
//! that configuration diagnostics can point at them.

use crate::lexer::{line_break, whitespace};

/// Which reader TypeScript 7 uses for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dialect {
    /// `tsconfig.json`, read with TypeScript's scanner: `//` and `/* */`
    /// comments, trailing commas, the lexer's whitespace and line breaks, and
    /// JavaScript string escapes such as `\x41` and `\u{1F600}`.
    Tsconfig,
    /// `package.json`, read as strict JSON. Comments, trailing commas, other
    /// escapes or whitespace, raw control characters, and unpaired surrogates
    /// make it malformed.
    Manifest,
}

/// A parsed value. Object keys are unique: a repeated key keeps its first
/// position and its last value, like a JavaScript object.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Value {
    pub(super) kind: Kind,
    pub(super) offset: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Kind {
    Null,
    Bool(bool),
    Number,
    String(String),
    /// A `tsconfig.json` string with an unpaired UTF-16 surrogate, which a
    /// Rust string cannot hold. It keeps its code units, so it equals no
    /// name and no other string, and it never becomes a path. A key with one
    /// cannot name an option, pattern, or field, so objects omit it.
    Utf16(Vec<u16>),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// Why a file could not be read as JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Failure {
    /// The text is not valid in the file's dialect.
    Malformed,
    /// Nesting passed this reader's limit: a limit of the tool, not a
    /// property of the file.
    TooDeep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SyntaxError {
    pub(super) failure: Failure,
    pub(super) message: String,
    pub(super) offset: usize,
}

impl Value {
    pub(super) fn get(&self, key: &str) -> Option<&Value> {
        self.entries()?
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// A property that is present and not `null`.
    pub(super) fn field(&self, key: &str) -> Option<&Value> {
        self.get(key).filter(|value| value.kind != Kind::Null)
    }

    pub(super) fn entries(&self) -> Option<&[(String, Value)]> {
        match &self.kind {
            Kind::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub(super) fn as_str(&self) -> Option<&str> {
        match &self.kind {
            Kind::String(text) => Some(text),
            _ => None,
        }
    }

    pub(super) fn as_bool(&self) -> Option<bool> {
        match self.kind {
            Kind::Bool(value) => Some(value),
            _ => None,
        }
    }

    pub(super) fn as_array(&self) -> Option<&[Value]> {
        match &self.kind {
            Kind::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// Containers under construction; nesting uses this explicit stack.
enum Frame {
    Array(usize, Vec<Value>),
    /// The pending key is `None` for a key that cannot name anything.
    Object(usize, Vec<(String, Value)>, Option<String>),
}

impl Frame {
    fn closer(&self) -> u8 {
        match self {
            Frame::Array(..) => b']',
            Frame::Object(..) => b'}',
        }
    }

    fn finish(self) -> Value {
        match self {
            Frame::Array(offset, items) => Value {
                kind: Kind::Array(items),
                offset,
            },
            Frame::Object(offset, entries, _) => Value {
                kind: Kind::Object(entries),
                offset,
            },
        }
    }

    fn push(&mut self, value: Value) {
        match self {
            Frame::Array(_, items) => items.push(value),
            Frame::Object(_, entries, key) => {
                let Some(key) = key.take() else {
                    return;
                };
                match entries.iter_mut().find(|(name, _)| *name == key) {
                    Some(entry) => entry.1 = value,
                    None => entries.push((key, value)),
                }
            }
        }
    }
}

/// Bounds nesting so that dropping, cloning, or comparing a value, which
/// recurse, stay shallow.
const MAX_DEPTH: usize = 128;

pub(super) fn parse(text: &str, dialect: Dialect) -> Result<Value, SyntaxError> {
    let mut parser = Parser {
        text,
        dialect,
        pos: if text.starts_with('\u{feff}') { 3 } else { 0 },
    };
    let mut stack: Vec<Frame> = Vec::new();
    loop {
        parser.trivia()?;
        let start = parser.pos;
        if stack.len() == MAX_DEPTH && matches!(parser.peek(), Some(b'[' | b'{')) {
            return Err(SyntaxError {
                failure: Failure::TooDeep,
                message: format!("Value is nested more than {MAX_DEPTH} levels deep"),
                offset: parser.pos,
            });
        }
        let mut complete = match parser.peek() {
            Some(b'[') => {
                parser.pos += 1;
                stack.push(Frame::Array(start, Vec::new()));
                parser.trivia()?;
                if parser.eat(b']') {
                    stack.pop().map(Frame::finish)
                } else {
                    None
                }
            }
            Some(b'{') => {
                parser.pos += 1;
                let mut frame = Frame::Object(start, Vec::new(), None);
                parser.trivia()?;
                if parser.eat(b'}') {
                    Some(frame.finish())
                } else {
                    parser.key(&mut frame)?;
                    stack.push(frame);
                    None
                }
            }
            _ => Some(parser.scalar()?),
        };
        while let Some(value) = complete.take() {
            let Some(frame) = stack.last_mut() else {
                parser.trivia()?;
                if parser.pos < text.len() {
                    return Err(parser.error("Unexpected content after the value"));
                }
                return Ok(value);
            };
            frame.push(value);
            parser.trivia()?;
            let closer = frame.closer();
            if parser.eat(b',') {
                parser.trivia()?;
                if dialect == Dialect::Tsconfig && parser.eat(closer) {
                    complete = stack.pop().map(Frame::finish);
                } else if let Frame::Object(..) = frame {
                    parser.key(frame)?;
                }
            } else if parser.eat(closer) {
                complete = stack.pop().map(Frame::finish);
            } else {
                return Err(parser.error(&format!("',' or '{}' expected", closer as char)));
            }
        }
    }
}

struct Parser<'a> {
    text: &'a str,
    dialect: Dialect,
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        let found = self.peek() == Some(byte);
        if found {
            self.pos += 1;
        }
        found
    }

    fn error(&self, message: &str) -> SyntaxError {
        SyntaxError {
            failure: Failure::Malformed,
            message: message.into(),
            offset: self.pos,
        }
    }

    /// Whitespace and, in `tsconfig.json`, comments.
    fn trivia(&mut self) -> Result<(), SyntaxError> {
        loop {
            let rest = &self.text[self.pos..];
            let Some(c) = rest.chars().next() else {
                return Ok(());
            };
            let space = match self.dialect {
                Dialect::Tsconfig => whitespace(c),
                Dialect::Manifest => matches!(c, ' ' | '\t' | '\n' | '\r'),
            };
            if space {
                self.pos += c.len_utf8();
            } else if self.dialect == Dialect::Manifest {
                return Ok(());
            } else if rest.starts_with("//") {
                self.pos += rest.find(line_break).unwrap_or(rest.len());
            } else if let Some(comment) = rest.strip_prefix("/*") {
                let Some(end) = comment.find("*/") else {
                    return Err(self.error("'*/' expected"));
                };
                self.pos += end + 4;
            } else {
                return Ok(());
            }
        }
    }

    /// A property name and its colon, stored as the frame's pending key.
    fn key(&mut self, frame: &mut Frame) -> Result<(), SyntaxError> {
        if self.peek() != Some(b'"') {
            return Err(self.error("Property name in double quotes expected"));
        }
        let name = match self.string()? {
            Kind::String(name) => Some(name),
            _ => None,
        };
        self.trivia()?;
        if !self.eat(b':') {
            return Err(self.error("':' expected"));
        }
        if let Frame::Object(_, _, key) = frame {
            *key = name;
        }
        Ok(())
    }

    fn scalar(&mut self) -> Result<Value, SyntaxError> {
        let offset = self.pos;
        let kind = match self.peek() {
            Some(b'"') => self.string()?,
            Some(b'-' | b'0'..=b'9') => {
                let length = self.text[self.pos..]
                    .find(|c: char| !matches!(c, '0'..='9' | '-' | '+' | '.' | 'e' | 'E'))
                    .unwrap_or(self.text.len() - self.pos);
                let number = &self.text[self.pos..self.pos + length];
                let valid = match self.dialect {
                    Dialect::Tsconfig => number.parse::<f64>().is_ok(),
                    Dialect::Manifest => strict_number(number),
                };
                if !valid {
                    return Err(self.error("Invalid number"));
                }
                self.pos += length;
                Kind::Number
            }
            _ => {
                let rest = &self.text[self.pos..];
                let (kind, length) = if rest.starts_with("true") {
                    (Kind::Bool(true), 4)
                } else if rest.starts_with("false") {
                    (Kind::Bool(false), 5)
                } else if rest.starts_with("null") {
                    (Kind::Null, 4)
                } else {
                    return Err(self.error("Value expected"));
                };
                self.pos += length;
                kind
            }
        };
        Ok(Value { kind, offset })
    }

    /// A double-quoted string, decoded as UTF-16 code units as JavaScript
    /// strings are.
    fn string(&mut self) -> Result<Kind, SyntaxError> {
        let start = self.pos;
        self.pos += 1;
        let mut units: Vec<u16> = Vec::new();
        loop {
            let Some(c) = self.text[self.pos..].chars().next() else {
                self.pos = start;
                return Err(self.error("Unterminated string literal"));
            };
            match c {
                '"' => {
                    self.pos += 1;
                    break;
                }
                '\n' | '\r' => {
                    self.pos = start;
                    return Err(self.error("Unterminated string literal"));
                }
                c if self.dialect == Dialect::Manifest && u32::from(c) < 0x20 => {
                    return Err(self.error("Control characters must be escaped"));
                }
                '\\' => {
                    self.pos += 1;
                    self.escape(&mut units)?;
                }
                c => {
                    self.pos += c.len_utf8();
                    units.extend(c.encode_utf16(&mut [0; 2]).iter());
                }
            }
        }
        match String::from_utf16(&units) {
            Ok(text) => Ok(Kind::String(text)),
            Err(_) if self.dialect == Dialect::Tsconfig => Ok(Kind::Utf16(units)),
            Err(_) => {
                self.pos = start;
                Err(self.error("Unpaired surrogate in string"))
            }
        }
    }

    /// One escape after its backslash.
    fn escape(&mut self, units: &mut Vec<u16>) -> Result<(), SyntaxError> {
        let Some(escape) = self.text[self.pos..].chars().next() else {
            return Err(self.error("Unterminated string literal"));
        };
        self.pos += escape.len_utf8();
        let simple = match escape {
            '"' | '\\' | '/' => Some(escape),
            'b' => Some('\u{8}'),
            'f' => Some('\u{c}'),
            'n' => Some('\n'),
            'r' => Some('\r'),
            't' => Some('\t'),
            'u' => None,
            _ if self.dialect == Dialect::Manifest => {
                self.pos -= escape.len_utf8();
                return Err(self.error("Invalid escape sequence"));
            }
            'v' => Some('\u{b}'),
            '0' => Some('\0'),
            'x' => {
                let code = self.hex(2)?;
                units.push(code as u16);
                return Ok(());
            }
            // A line continuation contributes nothing.
            '\r' => {
                self.eat(b'\n');
                return Ok(());
            }
            c if line_break(c) => return Ok(()),
            // Quotes and other escapes denote themselves.
            other => Some(other),
        };
        if let Some(c) = simple {
            units.extend(c.encode_utf16(&mut [0; 2]).iter());
            return Ok(());
        }
        if self.dialect == Dialect::Tsconfig && self.eat(b'{') {
            let digits = self.text[self.pos..]
                .find(|c: char| !c.is_ascii_hexdigit())
                .unwrap_or(self.text.len() - self.pos);
            let code = u32::from_str_radix(&self.text[self.pos..self.pos + digits], 16)
                .ok()
                .and_then(char::from_u32)
                .filter(|_| digits > 0);
            self.pos += digits;
            let (Some(c), true) = (code, self.eat(b'}')) else {
                return Err(self.error("Invalid Unicode escape sequence"));
            };
            units.extend(c.encode_utf16(&mut [0; 2]).iter());
            return Ok(());
        }
        let code = self.hex(4)?;
        units.push(code as u16);
        Ok(())
    }

    /// Exactly `count` hexadecimal digits.
    fn hex(&mut self, count: usize) -> Result<u32, SyntaxError> {
        let digits = self
            .text
            .get(self.pos..self.pos + count)
            .filter(|digits| digits.bytes().all(|b| b.is_ascii_hexdigit()));
        let Some(code) = digits.and_then(|digits| u32::from_str_radix(digits, 16).ok()) else {
            return Err(self.error("Hexadecimal digit expected"));
        };
        self.pos += count;
        Ok(code)
    }
}

/// RFC 8259's number grammar: an optional `-`, an integer without leading
/// zeros, then an optional fraction and exponent.
fn strict_number(text: &str) -> bool {
    let digits =
        |text: &str| text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let rest = text.strip_prefix('-').unwrap_or(text);
    let integer = digits(rest);
    if integer == 0 || integer > 1 && rest.starts_with('0') {
        return false;
    }
    let mut rest = &rest[integer..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let length = digits(fraction);
        if length == 0 {
            return false;
        }
        rest = &fraction[length..];
    }
    if let Some(exponent) = rest.strip_prefix(['e', 'E']) {
        let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        let length = digits(exponent);
        if length == 0 {
            return false;
        }
        rest = &exponent[length..];
    }
    rest.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tsconfig(text: &str) -> Result<Value, SyntaxError> {
        parse(text, Dialect::Tsconfig)
    }

    #[test]
    fn accepts_comments_trailing_commas_and_escapes() {
        let text = "\u{feff}// lead\n{ /* a */ \"a\": [1, -2.5e3, true, null,], \"b\": \"x\\u0041\\ud83d\\ude00\\\"\\x42\\u{43}\", \"a\": \"last\", }";
        let value = tsconfig(text).unwrap();
        assert_eq!(value.get("a").and_then(Value::as_str), Some("last"));
        assert_eq!(value.get("b").and_then(Value::as_str), Some("xA😀\"BC"));
        assert_eq!(value.entries().unwrap()[0].0, "a");
        assert_eq!(value.entries().unwrap().len(), 2);
        assert_eq!(value.get("b").unwrap().offset, text.find("\"x").unwrap());
    }

    #[test]
    fn keeps_unpaired_surrogates_apart() {
        let value =
            tsconfig("{ \"a\": \"\\ud800\", \"b\": \"\\ud801\", \"\\ud800\": 1, \"\\ud801\": 2 }")
                .unwrap();
        assert_eq!(value.get("a").unwrap().kind, Kind::Utf16(vec![0xd800]));
        assert_eq!(value.get("b").unwrap().kind, Kind::Utf16(vec![0xd801]));
        assert_eq!(value.entries().unwrap().len(), 2);
        assert!(parse("{ \"a\": \"\\ud800\" }", Dialect::Manifest).is_err());
    }

    #[test]
    fn a_manifest_is_strict_json() {
        let manifest = |text: &str| parse(text, Dialect::Manifest);
        assert!(manifest("\u{feff}{ \"a\": [1, -0.5e+3, \"\\/\\u0041\"], \"a\": null }").is_ok());
        for text in [
            "{ \"a\": 1, }",
            "{ \"a\": [1,] }",
            "// c\n{}",
            "{ /* c */ }",
            "{ \"a\": \"\\x41\" }",
            "{ \"a\": \"\t\" }",
            "{\u{a0}}",
            "{ \"a\": 01 }",
            "{ \"a\": +1 }",
            "{ \"a\": .5 }",
            "{ \"a\": 1. }",
        ] {
            let error = manifest(text).unwrap_err();
            assert_eq!(error.failure, Failure::Malformed, "{text:?}");
        }
    }

    #[test]
    fn line_comments_end_at_every_line_break() {
        for line_break in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
            let value = tsconfig(&format!("{{ // c{line_break}\"a\": 1 }}")).unwrap();
            assert!(value.get("a").is_some(), "{line_break:?}");
        }
    }

    #[test]
    fn bounds_nesting_depth() {
        let nested = |depth: usize| format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        assert!(tsconfig(&nested(MAX_DEPTH)).is_ok());
        let error = tsconfig(&nested(100_000)).unwrap_err();
        assert_eq!((error.failure, error.offset), (Failure::TooDeep, MAX_DEPTH));
    }

    #[test]
    fn reports_offsets_of_errors() {
        assert_eq!(tsconfig("{\"a\": }").unwrap_err().offset, 6);
        assert_eq!(tsconfig("{\"a\" 1}").unwrap_err().offset, 5);
        assert_eq!(tsconfig("[1 2]").unwrap_err().offset, 3);
        assert_eq!(tsconfig("{} x").unwrap_err().offset, 3);
        assert_eq!(tsconfig("\"abc").unwrap_err().offset, 0);
        assert_eq!(tsconfig("/* open").unwrap_err().offset, 0);
    }
}
