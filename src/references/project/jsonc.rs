//! JSON with comments and trailing commas, as accepted in `tsconfig.json` and
//! `package.json`. Values keep the byte offset where they start so that
//! configuration diagnostics can point at them.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SyntaxError {
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
    Object(usize, Vec<(String, Value)>, String),
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
                let key = std::mem::take(key);
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

pub(super) fn parse(text: &str) -> Result<Value, SyntaxError> {
    let mut parser = Parser {
        text,
        pos: if text.starts_with('\u{feff}') { 3 } else { 0 },
    };
    let mut stack: Vec<Frame> = Vec::new();
    loop {
        parser.trivia()?;
        let start = parser.pos;
        if stack.len() == MAX_DEPTH && matches!(parser.peek(), Some(b'[' | b'{')) {
            return Err(parser.error("Value is nested too deeply"));
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
                let mut frame = Frame::Object(start, Vec::new(), String::new());
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
                if parser.eat(closer) {
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
            message: message.into(),
            offset: self.pos,
        }
    }

    /// Whitespace, line comments, and block comments.
    fn trivia(&mut self) -> Result<(), SyntaxError> {
        loop {
            let rest = &self.text[self.pos..];
            let Some(c) = rest.chars().next() else {
                return Ok(());
            };
            if c.is_whitespace() || c == '\u{feff}' {
                self.pos += c.len_utf8();
            } else if rest.starts_with("//") {
                self.pos += rest.find(['\n', '\r']).unwrap_or(rest.len());
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
        let name = self.string()?;
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
            Some(b'"') => Kind::String(self.string()?),
            Some(b'-' | b'0'..=b'9') => {
                let length = self.text[self.pos..]
                    .find(|c: char| !matches!(c, '0'..='9' | '-' | '+' | '.' | 'e' | 'E'))
                    .unwrap_or(self.text.len() - self.pos);
                if self.text[self.pos..self.pos + length]
                    .parse::<f64>()
                    .is_err()
                {
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

    fn string(&mut self) -> Result<String, SyntaxError> {
        let start = self.pos;
        self.pos += 1;
        let mut value = String::new();
        let mut pending_high: Option<u16> = None;
        loop {
            let Some(c) = self.text[self.pos..].chars().next() else {
                self.pos = start;
                return Err(self.error("Unterminated string literal"));
            };
            self.pos += c.len_utf8();
            let unit = match c {
                '"' => break,
                '\n' | '\r' => {
                    self.pos = start;
                    return Err(self.error("Unterminated string literal"));
                }
                '\\' => {
                    let Some(escape) = self.text[self.pos..].chars().next() else {
                        continue;
                    };
                    self.pos += escape.len_utf8();
                    match escape {
                        'n' => Some('\n'),
                        't' => Some('\t'),
                        'r' => Some('\r'),
                        'b' => Some('\u{8}'),
                        'f' => Some('\u{c}'),
                        'u' => {
                            let digits = self.text.get(self.pos..self.pos + 4);
                            let Some(code) =
                                digits.and_then(|digits| u16::from_str_radix(digits, 16).ok())
                            else {
                                return Err(self.error("Hexadecimal digit expected"));
                            };
                            self.pos += 4;
                            if let Some(high) = pending_high.take() {
                                if (0xdc00..0xe000).contains(&code) {
                                    value.extend(char::decode_utf16([high, code]).flatten());
                                    continue;
                                }
                                value.push(char::REPLACEMENT_CHARACTER);
                            }
                            if (0xd800..0xdc00).contains(&code) {
                                pending_high = Some(code);
                                continue;
                            }
                            Some(char::from_u32(code.into()).unwrap_or(char::REPLACEMENT_CHARACTER))
                        }
                        // Quotes, slashes, and other escapes denote themselves.
                        other => Some(other),
                    }
                }
                c => Some(c),
            };
            if pending_high.take().is_some() {
                value.push(char::REPLACEMENT_CHARACTER);
            }
            value.extend(unit);
        }
        if pending_high.is_some() {
            value.push(char::REPLACEMENT_CHARACTER);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_comments_trailing_commas_and_escapes() {
        let text = "\u{feff}// lead\n{ /* a */ \"a\": [1, -2.5e3, true, null,], \"b\": \"x\\u0041\\ud83d\\ude00\\\"\", \"a\": \"last\", }";
        let value = parse(text).unwrap();
        assert_eq!(value.get("a").and_then(Value::as_str), Some("last"));
        assert_eq!(value.get("b").and_then(Value::as_str), Some("xA😀\""));
        assert_eq!(value.entries().unwrap()[0].0, "a");
        assert_eq!(value.entries().unwrap().len(), 2);
        assert_eq!(value.get("b").unwrap().offset, text.find("\"x").unwrap());
    }

    #[test]
    fn bounds_nesting_depth() {
        let nested = |depth: usize| format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        assert!(parse(&nested(MAX_DEPTH)).is_ok());
        assert_eq!(parse(&nested(100_000)).unwrap_err().offset, MAX_DEPTH);
    }

    #[test]
    fn reports_offsets_of_errors() {
        assert_eq!(parse("{\"a\": }").unwrap_err().offset, 6);
        assert_eq!(parse("{\"a\" 1}").unwrap_err().offset, 5);
        assert_eq!(parse("[1 2]").unwrap_err().offset, 3);
        assert_eq!(parse("{} x").unwrap_err().offset, 3);
        assert_eq!(parse("\"abc").unwrap_err().offset, 0);
        assert_eq!(parse("/* open").unwrap_err().offset, 0);
    }
}
