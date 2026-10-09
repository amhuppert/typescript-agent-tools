//! TypeScript's JSON text: the one reader for `tsconfig.json` files and JSON
//! source modules. It ports `parseJSONText`, `validateJsonValue`, and
//! `validateJsonObjectLiteral` (`internal/parser/parser.go`), the
//! `ScriptKindJSON` path of `ParseSourceFile` that TypeScript 7 uses for
//! both. Values inside arrays and objects go through this parser's own
//! expression grammar, so tokens, escapes, numbers, comments, a leading `#!`
//! line, and object members follow the same owners as TypeScript files;
//! validation then accepts only JSON values, as the compiler's does.
//!
//! The result is lossless: every value and key keeps its span, keys keep
//! their exact cooked UTF-16 text, including unpaired surrogates, and every
//! repeated property stays in source order. Consumers apply their own
//! conversion policy above it: configuration keeps the last value of a
//! repeated key, while a JSON module accepts any root value.
//!
//! The first error this parser meets fails the read. That is TypeScript's
//! first diagnostic except where its error recovery parses on and its JSON
//! validation then reports an earlier node, or where its scanner reports an
//! error inside a token at the offending character rather than the token's
//! start (docs/references.md). Text nested more than
//! `MAX_JSON_DEPTH` containers deep is `JsonErrorKind::TooDeep`, a limit of
//! this tool rather than a property of the text: TypeScript has none.

use super::{Parser, Result};
use crate::lexer::{Kind, Lexer};
use crate::syntax_kind::SyntaxKind::*;
use crate::{Ast, NodeId, ParseOptions, Span, Value};

/// The deepest nesting of arrays and objects this tool reads. Dropping a
/// value recurses through its containers, and the parser spends a few
/// grammar entries per container, well within the analysis budget,
/// `ParseOptions::DEEP_MAX_DEPTH` (`crate::analysis::Parser::parse_json`).
pub(crate) const MAX_JSON_DEPTH: usize = 128;

/// A parsed JSON text.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JsonText {
    /// `None` for text that is empty or only trivia: TypeScript's source file
    /// then has no statement.
    pub(crate) root: Option<JsonValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JsonValue {
    /// The value's tokens, without leading trivia: a negative number
    /// includes its `-`, and a string its quotes.
    pub(crate) span: Span,
    pub(crate) kind: JsonKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JsonKind {
    Null,
    Bool(bool),
    /// Any numeric literal TypeScript accepts, such as `0x10` or `1_000`,
    /// optionally negated; its text is the value's span.
    Number,
    String(JsonString),
    Array(Vec<JsonValue>),
    /// Every property in source order, repeated keys included.
    Object(Vec<JsonProperty>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JsonProperty {
    pub(crate) key: JsonString,
    /// The key's string literal, including its quotes.
    pub(crate) key_span: Span,
    /// Where a `?` follows the key, which TypeScript's parser accepts and
    /// only configuration conversion rejects (TS8009).
    pub(crate) question: Option<usize>,
    pub(crate) value: JsonValue,
}

/// A string's cooked value. JavaScript strings may hold unpaired UTF-16
/// surrogates, which a Rust string cannot: those keep their code units, so
/// they equal no other text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum JsonString {
    Text(String),
    Utf16(Vec<u16>),
}

impl JsonString {
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Utf16(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsonErrorKind {
    /// A diagnostic TypeScript reports for the text.
    Syntax,
    /// Nesting beyond `MAX_JSON_DEPTH` or the parser's budget.
    TooDeep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsonError {
    pub(crate) kind: JsonErrorKind,
    pub(crate) message: String,
    pub(crate) offset: usize,
}

impl JsonValue {
    pub(crate) fn as_str(&self) -> Option<&str> {
        match &self.kind {
            JsonKind::String(text) => text.as_str(),
            _ => None,
        }
    }

    pub(crate) fn as_bool(&self) -> Option<bool> {
        match self.kind {
            JsonKind::Bool(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[JsonValue]> {
        match &self.kind {
            JsonKind::Array(items) => Some(items),
            _ => None,
        }
    }

    /// An object's properties, every occurrence in source order.
    pub(crate) fn properties(&self) -> Option<&[JsonProperty]> {
        match &self.kind {
            JsonKind::Object(properties) => Some(properties),
            _ => None,
        }
    }

    pub(crate) fn is_null(&self) -> bool {
        self.kind == JsonKind::Null
    }
}

impl JsonText {
    /// The spans of every string literal token, keys included, in source
    /// order: where `//` and `/*` do not begin comments.
    pub(crate) fn literal_spans(&self) -> Vec<Span> {
        let mut spans = Vec::new();
        let mut pending: Vec<&JsonValue> = self.root.iter().collect();
        while let Some(value) = pending.pop() {
            match &value.kind {
                JsonKind::String(_) => spans.push(value.span),
                JsonKind::Array(items) => pending.extend(items.iter()),
                JsonKind::Object(properties) => {
                    for property in properties {
                        spans.push(property.key_span);
                        pending.push(&property.value);
                    }
                }
                JsonKind::Null | JsonKind::Bool(_) | JsonKind::Number => {}
            }
        }
        spans.sort_by_key(|span| span.start);
        spans
    }
}

/// Read `text` as TypeScript reads JSON, within `max_depth` grammar
/// entries. Analysis reads through `crate::analysis::Parser::parse_json`,
/// which owns the budget and its thread.
pub(crate) fn parse_json_text(
    text: &str,
    max_depth: usize,
) -> std::result::Result<JsonText, JsonError> {
    let options = ParseOptions {
        jsx: false,
        file_name: String::new(),
        max_depth,
    };
    let mut parser = Parser::new(text, options);
    let root = match parser.json_root() {
        Ok(root) => root,
        Err(error) => {
            let kind = if parser.exhausted.is_some() {
                JsonErrorKind::TooDeep
            } else {
                JsonErrorKind::Syntax
            };
            return Err(JsonError {
                kind,
                message: if kind == JsonErrorKind::TooDeep {
                    too_deep()
                } else {
                    error.message
                },
                offset: error.span.start,
            });
        }
    };
    let ast = Ast {
        source: text,
        nodes: parser.nodes,
        root: NodeId(0),
        utf16_deltas: Vec::new(),
    };
    Ok(JsonText {
        root: root
            .map(|root| Converter { ast: &ast }.value(root))
            .transpose()?,
    })
}

fn too_deep() -> String {
    format!("JSON value is nested more than {MAX_JSON_DEPTH} levels deep")
}

impl Parser<'_> {
    /// `parseJSONText`: no value for empty text; otherwise an array, a
    /// keyword, a negative number, or a literal not followed by `:`, and
    /// anything else as an object literal, which must start with `{`. More
    /// tokens after the value are TypeScript's "Unexpected token".
    fn json_root(&mut self) -> Result<Option<NodeId>> {
        self.validate_pragmas()?;
        self.check_token()?;
        if self.current.kind == Kind::Eof {
            return Ok(None);
        }
        let numeric = |token: crate::lexer::Token<'_>| {
            token.kind == Kind::Number && !token.raw.ends_with('n')
        };
        let root = match self.current.kind {
            Kind::Punct if self.at("[") => self.primary_expression()?,
            Kind::Word if matches!(self.current.raw, "true" | "false" | "null") => {
                self.token_node()?
            }
            Kind::Punct
                if self.at("-") && numeric(self.peek()) && self.peek_second().raw != ":" =>
            {
                self.unary_expression(false, false)?
            }
            Kind::String if !self.peek_at(":") => self.literal()?,
            Kind::Number if numeric(self.current) && !self.peek_at(":") => self.literal()?,
            _ => self.object_literal()?,
        };
        self.check_token()?;
        if self.current.kind != Kind::Eof {
            return Err(self.error("Unexpected token"));
        }
        self.check_budget()?;
        Ok(Some(root))
    }
}

/// `validateJsonValue` over the parsed expression, building the lossless
/// value as it goes. Containers use an explicit stack.
struct Converter<'a, 'src> {
    ast: &'a Ast<'src>,
}

/// A container being built: its span, its remaining child expressions, and
/// what it has so far.
enum Frame {
    Array {
        span: Span,
        elements: std::vec::IntoIter<NodeId>,
        items: Vec<JsonValue>,
    },
    Object {
        span: Span,
        properties: std::vec::IntoIter<NodeId>,
        members: Vec<JsonProperty>,
        /// The key of the property whose value is being built.
        pending: Option<Key>,
    },
}

/// A property's key, read before its value.
struct Key {
    text: JsonString,
    span: Span,
    question: Option<usize>,
}

/// What one child expression contributes.
enum Step {
    Scalar(JsonValue),
    Open(Frame),
}

impl Converter<'_, '_> {
    fn value(&self, root: NodeId) -> std::result::Result<JsonValue, JsonError> {
        let mut stack: Vec<Frame> = Vec::new();
        let mut next = Some(root);
        loop {
            let mut complete = match next.take() {
                Some(id) => match self.step(id, stack.len())? {
                    Step::Scalar(value) => Some(value),
                    Step::Open(frame) => {
                        stack.push(frame);
                        None
                    }
                },
                None => None,
            };
            // Attach finished values and find the next child to read.
            loop {
                let Some(frame) = stack.last_mut() else {
                    return Ok(complete.expect("the root finishes last"));
                };
                if let Some(value) = complete.take() {
                    match frame {
                        Frame::Array { items, .. } => items.push(value),
                        Frame::Object {
                            members, pending, ..
                        } => {
                            let key = pending.take().expect("a property's value follows its key");
                            members.push(JsonProperty {
                                key: key.text,
                                key_span: key.span,
                                question: key.question,
                                value,
                            });
                        }
                    }
                }
                let child = match frame {
                    Frame::Array { elements, .. } => elements.next(),
                    Frame::Object {
                        properties,
                        pending,
                        ..
                    } => match properties.next() {
                        Some(property) => {
                            let (key, value) = self.property(property)?;
                            *pending = Some(key);
                            Some(value)
                        }
                        None => None,
                    },
                };
                match child {
                    Some(child) => {
                        next = Some(child);
                        break;
                    }
                    None => {
                        complete = Some(match stack.pop().expect("a frame is open") {
                            Frame::Array { span, items, .. } => JsonValue {
                                span,
                                kind: JsonKind::Array(items),
                            },
                            Frame::Object { span, members, .. } => JsonValue {
                                span,
                                kind: JsonKind::Object(members),
                            },
                        });
                    }
                }
            }
        }
    }

    /// One value expression at `depth` enclosing containers.
    fn step(&self, id: NodeId, depth: usize) -> std::result::Result<Step, JsonError> {
        let node = self.ast.node(id);
        let span = self.ast.token_span(id);
        let kind = match node.syntax_kind {
            TrueKeyword => JsonKind::Bool(true),
            FalseKeyword => JsonKind::Bool(false),
            NullKeyword => JsonKind::Null,
            NumericLiteral => JsonKind::Number,
            StringLiteral => JsonKind::String(self.string(id)?),
            PrefixUnaryExpression
                if matches!(node.field("operator"), Some(Value::Number(operator)) if *operator == MinusToken.number())
                    && matches!(node.field("operand"), Some(Value::Node(operand))
                        if self.ast.node(*operand).syntax_kind == NumericLiteral) =>
            {
                JsonKind::Number
            }
            ObjectLiteralExpression | ArrayLiteralExpression => {
                if depth == MAX_JSON_DEPTH {
                    return Err(JsonError {
                        kind: JsonErrorKind::TooDeep,
                        message: too_deep(),
                        offset: span.start,
                    });
                }
                let list = if node.syntax_kind == ArrayLiteralExpression {
                    "elements"
                } else {
                    "properties"
                };
                let Some(Value::Nodes(children)) = node.field(list) else {
                    unreachable!("literals list their members");
                };
                let children = children.clone();
                return Ok(Step::Open(if node.syntax_kind == ArrayLiteralExpression {
                    Frame::Array {
                        span,
                        elements: children.into_iter(),
                        items: Vec::new(),
                    }
                } else {
                    Frame::Object {
                        span,
                        properties: children.into_iter(),
                        members: Vec::new(),
                        pending: None,
                    }
                }));
            }
            _ => return Err(self.invalid(id, INVALID_VALUE)),
        };
        Ok(Step::Scalar(JsonValue { span, kind }))
    }

    /// `validateJsonObjectLiteral` for one member: a property assignment
    /// named by a double-quoted string. Its key, then its value expression.
    fn property(&self, id: NodeId) -> std::result::Result<(Key, NodeId), JsonError> {
        let node = self.ast.node(id);
        if node.syntax_kind != PropertyAssignment {
            return Err(self.invalid(id, "Property assignment expected"));
        }
        let child = |name: &str| match node.field(name) {
            Some(Value::Node(child)) => Some(*child),
            _ => None,
        };
        let name = child("name").expect("a property assignment has a name");
        if self.ast.node(name).syntax_kind != StringLiteral {
            return Err(self.invalid(name, DOUBLE_QUOTES));
        }
        let key = Key {
            text: self.string(name)?,
            span: self.ast.token_span(name),
            question: child("questionToken").map(|token| self.ast.token_span(token).start),
        };
        let value = child("initializer").expect("a property assignment has a value");
        Ok((key, value))
    }

    /// A string literal's cooked text, which must be double-quoted.
    fn string(&self, id: NodeId) -> std::result::Result<JsonString, JsonError> {
        let span = self.ast.token_span(id);
        if self.ast.source().as_bytes().get(span.start) != Some(&b'"') {
            return Err(self.invalid(id, DOUBLE_QUOTES));
        }
        Ok(match self.ast.node(id).field("text") {
            Some(Value::Text(text)) => JsonString::Text(text.clone().into_owned()),
            Some(Value::Utf16(units)) => JsonString::Utf16(units.clone()),
            _ => unreachable!("string literals have text"),
        })
    }

    /// A validation diagnostic at the node's first token, as
    /// `getErrorSpanForNode` places it.
    fn invalid(&self, id: NodeId, message: &str) -> JsonError {
        let node = self.ast.node(id);
        let offset = Lexer {
            src: self.ast.source(),
            pos: node.span.start,
        }
        .next()
        .start;
        JsonError {
            kind: JsonErrorKind::Syntax,
            message: message.into(),
            offset,
        }
    }
}

/// TypeScript's TS1327.
const DOUBLE_QUOTES: &str = "String literal with double quotes expected";
/// TypeScript's TS1328.
const INVALID_VALUE: &str = "Property value can only be string literal, numeric literal, 'true', 'false', 'null', object literal or array literal";

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> std::result::Result<JsonText, JsonError> {
        let text = text.to_owned();
        crate::analysis::on_parsing_thread("json-test", move |parser| parser.parse_json(&text))
            .unwrap()
    }

    fn accepts(text: &str) -> JsonText {
        read(text).unwrap_or_else(|error| panic!("{text:?}: {error:?}"))
    }

    fn rejects(text: &str) -> JsonError {
        match read(text) {
            Ok(value) => panic!("{text:?} was accepted: {value:?}"),
            Err(error) => error,
        }
    }

    #[test]
    fn keeps_spans_exact_keys_and_every_repeated_property() {
        let text = "{ \"a\": 1, \"\\ud800\": -0x1, \"a\": \"two\", \"\\u{1F600}\": [true, null] }";
        let parsed = accepts(text);
        let root = parsed.root.as_ref().unwrap();
        assert_eq!(
            root.span,
            Span {
                start: 0,
                end: text.len()
            }
        );
        let properties = root.properties().unwrap();
        let keys: Vec<&JsonString> = properties.iter().map(|p| &p.key).collect();
        assert_eq!(
            keys,
            [
                &JsonString::Text("a".into()),
                &JsonString::Utf16(vec![0xd800]),
                &JsonString::Text("a".into()),
                &JsonString::Text("😀".into()),
            ]
        );
        assert_eq!(
            &text[properties[1].key_span.start..properties[1].key_span.end],
            "\"\\ud800\""
        );
        let negative = &properties[1].value;
        assert_eq!(&text[negative.span.start..negative.span.end], "-0x1");
        assert_eq!(properties[2].value.as_str(), Some("two"));
        assert_eq!(
            parsed.literal_spans().len(),
            5,
            "four keys and one string value"
        );
    }

    #[test]
    fn any_root_value_or_none() {
        for text in ["", "  \n", "// c", "/* c */", "\u{feff}", "#!x", "#!x\n"] {
            assert_eq!(accepts(text).root, None, "{text:?}");
        }
        for (text, kind) in [
            ("[1, 2]", "array"),
            ("\"x\"", "string"),
            ("1", "number"),
            ("-1", "number"),
            ("- 1", "number"),
            (".5", "number"),
            ("true", "bool"),
            ("null", "null"),
        ] {
            let root = accepts(text).root.unwrap();
            let actual = match root.kind {
                JsonKind::Array(_) => "array",
                JsonKind::String(_) => "string",
                JsonKind::Number => "number",
                JsonKind::Bool(_) => "bool",
                JsonKind::Null => "null",
                JsonKind::Object(_) => "object",
            };
            assert_eq!(actual, kind, "{text:?}");
        }
    }

    /// Each case was read by TypeScript 7.0.2 as a JSON module's source
    /// (`fixtures/json-probes/parsing.json`): syntactic diagnostics or none.
    #[test]
    fn follows_the_compilers_grammar() {
        for text in [
            "// c\n{ /* x */ \"a\": [1, 2,], }",
            "\u{feff}{\"a\":1}",
            "#!x\n{\"a\":1}",
            "{\"a\": 0x10}",
            "{\"a\": 0b1, \"b\": 0o7, \"c\": 0B1, \"d\": 0X1F, \"e\": 0O7}",
            "{\"a\": 1_000}",
            "{\"a\": .5}",
            "{\"a\": 5.}",
            "{\"a\": -0x1}",
            "{\"a\": -.5}",
            "{\"a\": 1e3, \"b\": 1E+3, \"c\": 1.e-3}",
            "{\"a\": \"\\0\"}",
            "{\"a\": \"\\u{1F600}\"}",
            "{\"a\": \"\\x41\", \"b\": \"\\v\\'\\q\"}",
            "{\"\\ud800\": 1, \"\\udc00\": 2}",
            "{\"a\": \"x\\\ny\", \"b\": \"x\\\r\ny\", \"c\": \"x\\\u{2028}y\"}",
            "{\"a\": \"x\u{2028}y\u{2029}z\"}",
            "[1,]",
            "{\"a\": {\"b\": [1,],},}",
            "{\"a\"?: 1}",
            "{\"a\"!: 1}",
            "{public \"a\": 1}",
            "{async \"a\": 1}",
            "{\"a\": - 1}",
            "{\"a\": -/*c*/1}",
            "{\u{a0}\"a\":\u{a0}1}",
            "{\"a\":\u{feff}1}",
            "{\u{b}\"a\":\u{c}1}",
            "{\u{2028}\"a\":\u{2029}1}",
            "{// c\u{2028}\"a\": 1}",
            "{\"a\": 1, \"a\": \"two\"}",
        ] {
            accepts(text);
        }
        for text in [
            "{\"a\": 'x'}",
            "{'a': 1}",
            "{a: 1}",
            "{1: 1}",
            "{[\"a\"]: 1}",
            "{a}",
            "{\"a\"() {}}",
            "{...x}",
            "{get \"a\"() {return 1}}",
            "{\"a\": +1}",
            "{\"a\": NaN}",
            "{\"a\": undefined}",
            "{\"a\": 01}",
            "{\"a\": 00}",
            "{\"a\": 08}",
            "{\"a\": 09.5}",
            "{\"a\": 1n}",
            "{\"a\": 1e}",
            "{\"a\": .}",
            "{\"a\": 1..2}",
            "{\"a\": 1._5}",
            "{\"a\": 0_1}",
            "{\"a\": 1_}",
            "{\"a\": 1a}",
            "{\"a\": \"\\01\"}",
            "{\"a\": \"\\8\"}",
            "{\"a\": \"\\u{110000}\"}",
            "{\"a\": \"\\x4\"}",
            "{\"a\": \"x\ny\"}",
            "{\"a\": \"x\ry\"}",
            "{\"a\": 1",
            "{\"a\": 1} /* x",
            "{} {}",
            "'x'",
            "\"a\": 1",
            "-1: 2",
            "-x",
            "x",
            "[1,,2]",
            "[,]",
            "{,}",
            "{@x \"a\": 1}",
            "{\"a\": `x`}",
            "{\"a\": (1)}",
            "{\"a\": tru\\u0065}",
            "{\"a\": --1}",
            "{\"a\": - -1}",
            "{\"a\": 1 #}",
            "{\"a\": #x}",
            "{\"a\": /x/}",
            "{\"a\": 1 + 2}",
            "/// <reference types=\"x\" resolution-mode=\"bad\" />\n{}",
        ] {
            assert_eq!(rejects(text).kind, JsonErrorKind::Syntax, "{text:?}");
        }
    }

    #[test]
    fn reports_where_the_first_error_is() {
        let at = |text: &str| rejects(text).offset;
        assert_eq!(at("{\"a\": 'x'}"), 6);
        assert_eq!(at("{\"a\": 1, b: 2}"), 9);
        assert_eq!(at("[1, {\"a\": +1}]"), 10);
        assert_eq!(at("{} {}"), 3);
        assert_eq!(at("\"a\": 1"), 0);
        assert_eq!(at("{\"a\": 01}"), 6);
    }

    #[test]
    fn bounds_nesting_depth() {
        let arrays = |depth: usize| format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        let objects = |depth: usize| format!("{}1{}", "{\"a\":".repeat(depth), "}".repeat(depth));
        for nested in [arrays, objects] {
            assert!(read(&nested(MAX_JSON_DEPTH - 1)).is_ok());
            assert!(read(&nested(MAX_JSON_DEPTH)).is_ok());
            let over = rejects(&nested(MAX_JSON_DEPTH + 1));
            assert_eq!(over.kind, JsonErrorKind::TooDeep);
            // Past the parser's own budget, too.
            assert_eq!(rejects(&nested(10_000)).kind, JsonErrorKind::TooDeep);
        }
        let over = rejects(&arrays(MAX_JSON_DEPTH + 1));
        assert_eq!(over.offset, MAX_JSON_DEPTH);
    }

    #[test]
    fn reads_large_flat_values() {
        let items = vec!["1"; 200_000].join(",");
        let parsed = accepts(&format!("[{items}]"));
        assert_eq!(parsed.root.unwrap().as_array().unwrap().len(), 200_000);
        let members: Vec<String> = (0..50_000).map(|i| format!("\"k{i}\": {i}")).collect();
        let parsed = accepts(&format!("{{{}}}", members.join(",")));
        assert_eq!(parsed.root.unwrap().properties().unwrap().len(), 50_000);
    }
}
