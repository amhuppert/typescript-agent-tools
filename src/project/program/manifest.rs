//! `package.json`, read as strict JSON, as TypeScript 7 reads it with Go's
//! `encoding/json` (`internal/packagejson`): comments, trailing commas, other
//! escapes or whitespace, raw control characters, and unpaired surrogates
//! make it malformed. This is a different dialect from the TypeScript JSON
//! text of configuration files and JSON modules (`crate::parser::json`).
//! Values keep the byte offset where they start so that diagnostics can
//! point at them.

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
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// Why a manifest could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Failure {
    /// The text is not strict JSON.
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
    /// The pending key of the property being read.
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
                let key = key.take().expect("a value follows its key");
                match entries.iter_mut().find(|(name, _)| *name == key) {
                    Some(entry) => entry.1 = value,
                    None => entries.push((key, value)),
                }
            }
        }
    }
}

/// Bounds nesting so that dropping, cloning, or comparing a value, which
/// recurse, stay shallow. The same limit as `crate::parser::json`.
const MAX_DEPTH: usize = crate::parser::json::MAX_JSON_DEPTH;

pub(super) fn parse(text: &str) -> Result<Value, SyntaxError> {
    let mut parser = Parser { text, pos: 0 };
    let mut stack: Vec<Frame> = Vec::new();
    loop {
        parser.trivia();
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
                parser.trivia();
                if parser.eat(b']') {
                    stack.pop().map(Frame::finish)
                } else {
                    None
                }
            }
            Some(b'{') => {
                parser.pos += 1;
                let mut frame = Frame::Object(start, Vec::new(), None);
                parser.trivia();
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
                parser.trivia();
                if parser.pos < text.len() {
                    return Err(parser.error("Unexpected content after the value"));
                }
                return Ok(value);
            };
            frame.push(value);
            parser.trivia();
            let closer = frame.closer();
            if parser.eat(b',') {
                parser.trivia();
                if let Frame::Object(..) = frame {
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

    /// JSON's four whitespace characters.
    fn trivia(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    /// A property name and its colon, stored as the frame's pending key.
    fn key(&mut self, frame: &mut Frame) -> Result<(), SyntaxError> {
        if self.peek() != Some(b'"') {
            return Err(self.error("Property name in double quotes expected"));
        }
        let name = self.string()?;
        self.trivia();
        if !self.eat(b':') {
            return Err(self.error("':' expected"));
        }
        if let Frame::Object(_, _, key) = frame {
            *key = Some(name);
        }
        Ok(())
    }

    fn scalar(&mut self) -> Result<Value, SyntaxError> {
        let offset = self.pos;
        let kind = match self.peek() {
            Some(b'"') => Kind::String(self.string()?),
            Some(b'-' | b'0'..=b'9') => {
                let length = self.text[self.pos..]
                    .find(|c: char| !matches!(c, '0'..='9' | '-' | '+' | '.' | 'e' | 'E'))
                    .unwrap_or(self.text.len() - self.pos);
                if !strict_number(&self.text[self.pos..self.pos + length]) {
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

    /// A double-quoted string, decoded as UTF-16 code units, which must form
    /// valid Unicode.
    fn string(&mut self) -> Result<String, SyntaxError> {
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
                c if u32::from(c) < 0x20 => {
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
        String::from_utf16(&units).map_err(|_| {
            self.pos = start;
            self.error("Unpaired surrogate in string")
        })
    }

    /// One escape after its backslash.
    fn escape(&mut self, units: &mut Vec<u16>) -> Result<(), SyntaxError> {
        let Some(escape) = self.text[self.pos..].chars().next() else {
            return Err(self.error("Unterminated string literal"));
        };
        self.pos += escape.len_utf8();
        let simple = match escape {
            '"' | '\\' | '/' => escape,
            'b' => '\u{8}',
            'f' => '\u{c}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'u' => {
                let digits = self
                    .text
                    .get(self.pos..self.pos + 4)
                    .filter(|digits| digits.bytes().all(|b| b.is_ascii_hexdigit()));
                let Some(code) = digits.and_then(|digits| u16::from_str_radix(digits, 16).ok())
                else {
                    return Err(self.error("Hexadecimal digit expected"));
                };
                self.pos += 4;
                units.push(code);
                return Ok(());
            }
            _ => {
                self.pos -= escape.len_utf8();
                return Err(self.error("Invalid escape sequence"));
            }
        };
        units.extend(simple.encode_utf16(&mut [0; 2]).iter());
        Ok(())
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

    #[test]
    fn a_manifest_is_strict_json() {
        let value = parse(
            "{ \"a\": [1, -0.5e+3, \"\\/\\u0041\\ud83d\\ude00\"], \"b\": true, \"a\": null }",
        )
        .unwrap();
        assert_eq!(value.get("a").unwrap().kind, Kind::Null);
        assert_eq!(value.entries().unwrap()[0].0, "a");
        assert_eq!(value.entries().unwrap().len(), 2);
        for text in [
            "{ \"a\": 1, }",
            "{ \"a\": [1,] }",
            "// c\n{}",
            "{ /* c */ }",
            "{ \"a\": \"\\x41\" }",
            "{ \"a\": \"\\u{41}\" }",
            "{ \"a\": \"\t\" }",
            "{\u{a0}}",
            "\u{feff}{}",
            "{ \"a\": 01 }",
            "{ \"a\": +1 }",
            "{ \"a\": .5 }",
            "{ \"a\": 1. }",
            "{ \"a\": 0x10 }",
            "{ \"a\": \"\\ud800\" }",
            "",
        ] {
            let error = parse(text).unwrap_err();
            assert_eq!(error.failure, Failure::Malformed, "{text:?}");
        }
    }

    #[test]
    fn bounds_nesting_depth() {
        let nested = |depth: usize| format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        assert!(parse(&nested(MAX_DEPTH)).is_ok());
        let error = parse(&nested(100_000)).unwrap_err();
        assert_eq!((error.failure, error.offset), (Failure::TooDeep, MAX_DEPTH));
    }

    #[test]
    fn reports_offsets_of_errors() {
        assert_eq!(parse("{\"a\": }").unwrap_err().offset, 6);
        assert_eq!(parse("{\"a\" 1}").unwrap_err().offset, 5);
        assert_eq!(parse("[1 2]").unwrap_err().offset, 3);
        assert_eq!(parse("{} x").unwrap_err().offset, 3);
        assert_eq!(parse("\"abc").unwrap_err().offset, 0);
    }
}
