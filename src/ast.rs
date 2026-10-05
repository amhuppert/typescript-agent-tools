//! An arena of syntax nodes. Child links are indices, so dropping an AST never
//! recursively walks the tree. Source text is borrowed rather than copied.

use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};
use std::borrow::Cow;

/// An arena-local link. Use it only with the `Ast` that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) usize);

impl NodeId {
    /// The node's index within its originating arena.
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    /// Inclusive UTF-8 byte offset.
    pub start: usize,
    /// Exclusive UTF-8 byte offset.
    pub end: usize,
}

#[derive(Debug, Clone)]
pub enum Value<'src> {
    Node(NodeId),
    Nodes(Vec<NodeId>),
    Text(Cow<'src, str>),
    /// JavaScript strings can contain unpaired UTF-16 surrogates. Use
    /// `Ast::write_json` for a lossless JSON encoding of these values.
    Utf16(Vec<u16>),
    Bool(bool),
    Number(u32),
}

impl<'src> From<NodeId> for Value<'src> {
    fn from(id: NodeId) -> Self {
        Self::Node(id)
    }
}
impl<'src> From<Vec<NodeId>> for Value<'src> {
    fn from(ids: Vec<NodeId>) -> Self {
        Self::Nodes(ids)
    }
}
impl<'src> From<&'src str> for Value<'src> {
    fn from(text: &'src str) -> Self {
        Self::Text(Cow::Borrowed(text))
    }
}
impl<'src> From<String> for Value<'src> {
    fn from(text: String) -> Self {
        Self::Text(Cow::Owned(text))
    }
}
impl<'src> From<bool> for Value<'src> {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

#[derive(Debug, Clone)]
pub struct Node<'src> {
    /// TypeScript syntax name, e.g. `BinaryExpression` or `ConditionalType`.
    pub kind: &'static str,
    pub span: Span,
    pub flags: u32,
    pub fields: Fields<'src>,
    pub(crate) syntax_kind: crate::syntax_kind::SyntaxKind,
    pub(crate) possible_top_level_await: bool,
}

/// Syntax fields, with one field stored inline to avoid allocating leaf nodes.
pub type Fields<'src> = smallvec::SmallVec<[(&'static str, Value<'src>); 1]>;

impl<'src> Node<'src> {
    pub fn field(&self, name: &str) -> Option<&Value<'src>> {
        self.fields
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at bytes {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for Diagnostic {}

impl Diagnostic {
    /// One-based line and UTF-16 column, including all ECMAScript line breaks.
    /// Pass the source that produced this diagnostic.
    ///
    /// # Panics
    /// Panics if the span start is not a UTF-8 boundary within `source`.
    pub fn line_column(&self, source: &str) -> (usize, usize) {
        let mut line = 1;
        let mut column = 1;
        let mut previous_cr = false;
        for c in source[..self.span.start].chars() {
            match c {
                '\n' if previous_cr => {}
                '\n' | '\r' | '\u{2028}' | '\u{2029}' => {
                    line += 1;
                    column = 1;
                }
                _ => column += c.len_utf16(),
            }
            previous_cr = c == '\r';
        }
        (line, column)
    }
}

/// An immutable syntax arena borrowing its original source. Child IDs and the
/// UTF-16 position index stay valid for the lifetime of this arena.
#[derive(Debug)]
pub struct Ast<'src> {
    pub(crate) source: &'src str,
    pub(crate) nodes: Vec<Node<'src>>,
    pub(crate) root: NodeId,
    pub(crate) utf16_deltas: Vec<(usize, usize)>,
}

impl<'src> Ast<'src> {
    pub fn source(&self) -> &'src str {
        self.source
    }
    /// All allocated nodes, including nodes superseded by module reparsing.
    /// Traverse child fields from `root()` to visit only the final syntax tree.
    pub fn nodes(&self) -> &[Node<'src>] {
        &self.nodes
    }
    pub fn root_id(&self) -> NodeId {
        self.root
    }
    /// Look up a link from this arena.
    ///
    /// # Panics
    /// Panics if the index is out of bounds. IDs from another arena are invalid
    /// even when their numeric index happens to be in bounds.
    pub fn node(&self, id: NodeId) -> &Node<'src> {
        &self.nodes[id.0]
    }
    pub fn root(&self) -> &Node<'src> {
        self.node(self.root)
    }
    /// Original node text, including its preceding trivia. `id` must belong to
    /// this arena, with the same lookup preconditions as `node()`.
    pub fn text(&self, id: NodeId) -> &'src str {
        let span = self.node(id).span;
        &self.source[span.start..span.end]
    }
    /// A JSON view of a link from this arena; see `node()` for ID preconditions.
    pub fn view(&self, id: NodeId) -> NodeView<'_, 'src> {
        NodeView { ast: self, id }
    }
    /// Convert a UTF-8 byte boundary in the original source to a UTF-16 offset.
    ///
    /// # Panics
    /// Panics if `byte_offset` is outside the source or splits a UTF-8 character.
    pub fn utf16_offset(&self, byte_offset: usize) -> usize {
        assert!(
            self.source.is_char_boundary(byte_offset),
            "source byte boundary"
        );
        let index = self
            .utf16_deltas
            .partition_point(|(byte, _)| *byte <= byte_offset);
        byte_offset
            - if index == 0 {
                0
            } else {
                self.utf16_deltas[index - 1].1
            }
    }
}

/// A nested, serializable view of an arena node.
pub struct NodeView<'ast, 'src> {
    pub(crate) ast: &'ast Ast<'src>,
    pub(crate) id: NodeId,
}

impl Serialize for Ast<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.view(self.root).serialize(serializer)
    }
}

impl Serialize for NodeView<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let node = self.ast.node(self.id);
        let mut map = serializer.serialize_map(Some(4 + node.fields.len()))?;
        map.serialize_entry("kind", &node.syntax_kind.number())?;
        map.serialize_entry("pos", &self.ast.utf16_offset(node.span.start))?;
        map.serialize_entry("end", &self.ast.utf16_offset(node.span.end))?;
        map.serialize_entry("flags", &node.flags)?;
        for (key, value) in &node.fields {
            match value {
                Value::Node(id) => map.serialize_entry(key, &self.ast.view(*id))?,
                Value::Nodes(ids) => map.serialize_entry(key, &NodeList { ast: self.ast, ids })?,
                Value::Text(text) => map.serialize_entry(key, text)?,
                Value::Utf16(text) => {
                    let text = String::from_utf16(text).map_err(serde::ser::Error::custom)?;
                    map.serialize_entry(key, &text)?;
                }
                Value::Bool(value) => map.serialize_entry(key, value)?,
                Value::Number(value) => map.serialize_entry(key, value)?,
            }
        }
        map.end()
    }
}

struct NodeList<'ast, 'src> {
    ast: &'ast Ast<'src>,
    ids: &'ast [NodeId],
}
impl Serialize for NodeList<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.ids.len()))?;
        for id in self.ids {
            seq.serialize_element(&self.ast.view(*id))?;
        }
        seq.end()
    }
}
