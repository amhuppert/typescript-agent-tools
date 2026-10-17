//! Owned type requests and bounded descriptions from one Project generation.
//!
//! Types, symbol identities, parser arenas and checker state stay private.
//! The graph is a finite presentation: its document-local IDs never identify
//! semantic types across requests, and display omissions stay distinct from
//! unsupported semantic details.

use crate::analysis::{QueryError, QueryErrorKind, SemanticFailureReason};
use crate::references::{Location, NamedLocation};
use crate::source::SourcePosition;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A declaration selector or an exact token position in the searched files.
/// A selector names declarations, not an expression path. Relative file
/// operands follow every Project operation's existing file lookup policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeQuery {
    Name {
        name: String,
        file: Option<PathBuf>,
    },
    At {
        file: PathBuf,
        position: SourcePosition,
    },
}

/// One type question with display choices; these never change semantic
/// admission or the parser's policy. Depth counts type edges, including zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRequest {
    pub query: TypeQuery,
    pub expand: bool,
    pub depth: usize,
}

/// Display policy shared by typed requests, the service and CLI parsing.
pub const MAX_TYPE_DEPTH: usize = 32;

impl TypeRequest {
    pub fn new(query: TypeQuery) -> Self {
        Self {
            query,
            expand: false,
            depth: 3,
        }
    }

    /// Validate operation options before selecting a target or demanding types.
    /// Display depth never changes parser or semantic admission policy.
    pub fn validate(&self) -> Result<(), QueryError> {
        if self.depth > MAX_TYPE_DEPTH {
            return Err(QueryError {
                kind: QueryErrorKind::InvalidRequest,
                message: format!(
                    "type expansion depth must be between 0 and {MAX_TYPE_DEPTH}, got {}",
                    self.depth
                ),
                path: None,
                position: None,
            });
        }
        Ok(())
    }
}

/// A selected type or honest absence/ambiguity. JSON is the corresponding
/// flat operation document, shared by direct, CLI and service presentation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TypeAnswer {
    Selected(TypeResult),
    Unselected(TypeSelection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeOperation {
    Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeSelection {
    pub operation: TypeOperation,
    pub project: PathBuf,
    pub selection: TypeQuery,
    #[serde(flatten)]
    pub outcome: SelectionFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SelectionFailure {
    NotFound { candidates: Vec<TypeCandidate> },
    Ambiguous { candidates: Vec<TypeCandidate> },
}

/// A semantic candidate, distinct from Q2's display candidate. Its own anchor
/// must replay the same role and question through this operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeCandidate {
    pub selector: Option<String>,
    pub file: String,
    pub question: TypeQuestion,
    pub declarations: Vec<NamedLocation>,
    pub anchor: TypeAnchor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeAnchor {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeQuestion {
    Declared,
    Value,
    Expression,
    Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeTarget {
    pub location: Location,
    pub kind: TypeQuestion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeOutcome {
    Found,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeResult {
    pub operation: TypeOperation,
    pub outcome: TypeOutcome,
    pub project: PathBuf,
    pub target: TypeTarget,
    #[serde(rename = "type")]
    pub description: TypeDescription,
    pub issues: Vec<TypeIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDescription {
    pub summary: String,
    pub summary_truncated: bool,
    pub expansion: Option<TypeExpansion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeIssue {
    /// Unsupported reasons only; invalid recovery fails the whole operation.
    pub reason: SemanticFailureReason,
    pub location: Location,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeExpansion {
    pub root: String,
    pub nodes: Vec<TypeNode>,
    /// Display elision only. Cycles/shared identities do not set this flag.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeNode {
    pub id: String,
    pub summary: String,
    pub summary_truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<TypeAlias>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<Vec<TypeRef>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_type_arguments: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted: Option<TypeFieldsOmitted>,
    #[serde(flatten)]
    pub data: TypeNodeData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeAlias {
    pub name: String,
    pub origins: Vec<Location>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeKind {
    Any,
    Unknown,
    Never,
    Void,
    Undefined,
    Null,
    Primitive,
    Literal,
    UniqueSymbol,
    Enum,
    TypeParameter,
    Object,
    Function,
    Array,
    Tuple,
    Union,
    Intersection,
    Operator,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrimitiveName {
    String,
    Number,
    Boolean,
    Bigint,
    Symbol,
    Object,
}

/// Exact literal payload. Ordinary scalar values keep their JSON string form;
/// native byte strings and generated identities use ordered, lossless parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TypeLiteralValue {
    Text(String),
    Symbolic { parts: Vec<TypeLiteralPart> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeLiteralPart {
    Text {
        value: String,
    },
    /// Lowercase hexadecimal encoding of the exact native bytes.
    Bytes {
        value: String,
    },
    /// A presentation-local reference; declaration origins are metadata only.
    Symbol {
        r#ref: String,
        origins: Vec<Location>,
    },
}

impl TypeLiteralValue {
    /// Returns scalar text only when the exact value has that representation.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Symbolic { .. } => None,
        }
    }
}

impl From<String> for TypeLiteralValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl From<&str> for TypeLiteralValue {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}
impl PartialEq<str> for TypeLiteralValue {
    fn eq(&self, other: &str) -> bool {
        matches!(self, Self::Text(value) if value == other)
    }
}
impl PartialEq<&str> for TypeLiteralValue {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeNodeData {
    Any,
    Unknown,
    Never,
    Void,
    Undefined,
    Null,
    Primitive {
        name: PrimitiveName,
    },
    Literal {
        base: PrimitiveName,
        value: TypeLiteralValue,
    },
    UniqueSymbol {
        origins: Vec<Location>,
    },
    Enum {
        origins: Vec<Location>,
    },
    TypeParameter {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        constraint: Option<TypeRef>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<TypeRef>,
    },
    Object {
        #[serde(flatten)]
        members: TypeMembers,
    },
    Function {
        #[serde(flatten)]
        members: TypeMembers,
    },
    Array {
        element: TypeRef,
        readonly: bool,
        #[serde(flatten)]
        members: TypeMembers,
    },
    Tuple {
        elements: Vec<TupleElement>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "omittedElements"
        )]
        omitted_elements: Option<usize>,
        readonly: bool,
        #[serde(flatten)]
        members: TypeMembers,
    },
    Union {
        alternatives: Vec<TypeRef>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "omittedAlternatives"
        )]
        omitted_alternatives: Option<usize>,
    },
    Intersection {
        alternatives: Vec<TypeRef>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "omittedAlternatives"
        )]
        omitted_alternatives: Option<usize>,
    },
    Operator {
        operator: TypeOperator,
        operands: Vec<TypeRef>,
        parameters: Vec<TypeParameter>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "omittedOperands"
        )]
        omitted_operands: Option<usize>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "omittedParameters"
        )]
        omitted_parameters: Option<usize>,
    },
    Unsupported {
        reason: SemanticFailureReason,
        location: Location,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TypeRef {
    Ref {
        r#ref: String,
    },
    Omitted {
        omitted: TypeOmissionReason,
        kind: TypeKind,
        summary: String,
        #[serde(rename = "summaryTruncated")]
        summary_truncated: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeOmissionReason {
    Depth,
    Nodes,
    Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeFieldsOmitted {
    pub reason: TypeOmissionReason,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeMembers {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<TypeProperty>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_signatures: Option<Vec<TypeSignature>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construct_signatures: Option<Vec<TypeSignature>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_signatures: Option<Vec<TypeIndex>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_collections: Option<Vec<TypeBlockedCollection>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_properties: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_call_signatures: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_construct_signatures: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_index_signatures: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeBlockedCollection {
    pub field: TypeCollection,
    pub issue: TypeIssue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TypeCollection {
    Properties,
    CallSignatures,
    ConstructSignatures,
    IndexSignatures,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeProperty {
    pub key: TypeKey,
    pub kind: TypePropertyKind,
    #[serde(rename = "type")]
    pub read_type: TypeRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_type: Option<TypeRef>,
    pub optional: bool,
    pub readonly: bool,
    pub accessibility: TypeAccessibility,
    pub origins: Vec<Location>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeKey {
    String {
        name: String,
    },
    Private {
        name: String,
        origins: Vec<Location>,
    },
    Symbol {
        name: String,
        origins: Vec<Location>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypePropertyKind {
    Property,
    Method,
    Accessor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeAccessibility {
    Public,
    Protected,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeSignature {
    pub type_parameters: Vec<TypeParameter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub this_parameter: Option<TypeParameterValue>,
    pub parameters: Vec<TypeParameterValue>,
    pub return_type: TypeRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<TypePredicate>,
    pub origins: Vec<Location>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeParameter {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint: Option<TypeRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<TypeRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variance: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeParameterValue {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: TypeRef,
    pub optional: bool,
    pub rest: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypePredicate {
    pub kind: TypePredicateKind,
    pub subject: TypePredicateSubject,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub value_type: Option<TypeRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypePredicateKind {
    Is,
    AssertsIs,
    Asserts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TypePredicateSubject {
    This(TypeThis),
    ParameterIndex(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeThis {
    This,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeIndex {
    pub key_type: TypeRef,
    pub value_type: TypeRef,
    pub readonly: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TupleElement {
    #[serde(rename = "type")]
    pub value_type: TypeRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub optional: bool,
    pub rest: bool,
    pub variadic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeOperator {
    Keyof,
    IndexedAccess,
    Conditional,
    Mapped,
    TemplateLiteral,
    StringMapping,
    Substitution,
}
