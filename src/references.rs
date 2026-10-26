//! Project-wide reference queries and their results. `Project` answers
//! them from its immutable snapshot (`Project::find_references`), and
//! `ProjectSession` from a fresh generation
//! (`ProjectRequest::References`).
//!
//! ```no_run
//! use std::path::Path;
//! use typescript_parser::project::Project;
//! use typescript_parser::references::ReferenceQuery;
//!
//! let project = Project::load(Path::new("tsconfig.json"))?;
//! let result = project.find_references(ReferenceQuery::Name {
//!     name: "run".into(),
//!     file: None,
//! })?;
//! println!("{}", serde_json::to_string_pretty(&result)?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::source::SourcePosition;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Selects reference groups by token position or by exact declared name.
/// A result echoes it as its `selection`, tagged `at` or `name`, with its
/// file named as [`Location::file`] names files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceQuery {
    /// The identifier token containing `position` in `file`.
    At {
        file: PathBuf,
        position: SourcePosition,
    },
    /// Declarations, local imports, and exported aliases spelled `name`.
    /// `file` narrows candidates; references still span the project.
    Name { name: String, file: Option<PathBuf> },
}

/// Reporting filters; selection, loading and declaration origins are unaffected.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceFilters {
    pub calls: bool,
    pub exclude_tests: bool,
    /// Keep only occurrences in this searched file. A request names it as a
    /// query path names a file (`FileLookup::find`), and an absent or
    /// unsearched file fails the query as `OutsideProject`; a result echoes
    /// it as [`Location::file`] names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
}

/// A reference query with explicit reporting filters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceRequest {
    pub query: ReferenceQuery,
    pub filters: ReferenceFilters,
}

/// A selected search or an absent/ambiguous declaration selector. The flat
/// JSON documents are shared by the direct API, CLI and project service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ReferenceAnswer {
    Selected(ReferenceResult),
    Unselected(ReferenceSelection),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceSelection {
    pub operation: ReferenceOperation,
    pub project: PathBuf,
    /// As [`ReferenceResult::selection`] echoes it.
    pub selection: ReferenceQuery,
    #[serde(flatten)]
    pub outcome: ReferenceSelectionFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ReferenceSelectionFailure {
    NotFound { candidates: Vec<ReferenceCandidate> },
    Ambiguous { candidates: Vec<ReferenceCandidate> },
}

/// The anchor replays this same symbol and reference operation. It may differ
/// from Q2's display anchor, notably for parameter-property modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCandidate {
    pub selector: Option<String>,
    pub file: String,
    pub question: ReferenceBaseline,
    pub declarations: Vec<NamedLocation>,
    pub anchor: ReferenceAnchor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceResult {
    pub operation: ReferenceOperation,
    pub outcome: ReferenceOutcome,
    /// The selected configuration file, in the canonical form of its
    /// directory, which every `Location::file` is relative to.
    pub project: PathBuf,
    /// The request's selection, its file named as `Location::file` names
    /// files, so the echo is the same from any working directory.
    pub selection: ReferenceQuery,
    /// Separate symbols, ordered by their earliest declaration.
    pub groups: Vec<ReferenceGroup>,
    pub filters: ReferenceFilters,
    pub coverage: ReferenceCoverage,
    /// Catalog declarations omitted by lexical search. These are suggestions,
    /// not semantically resolved members or a complete inferred-member catalog.
    pub unsearched_members: UnsearchedMembers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceOperation {
    References,
}

/// The declarations a selection denotes, following import and export
/// aliases (`Project::definition`), or why nothing was selected. The flat
/// JSON documents are shared by the direct API, CLI and project service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DefinitionAnswer {
    Selected(DefinitionResult),
    Unselected(DefinitionSelection),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionResult {
    pub operation: DefinitionOperation,
    pub outcome: DefinitionOutcome,
    /// As [`ReferenceResult::project`] names it.
    pub project: PathBuf,
    /// As [`ReferenceResult::selection`] echoes it.
    pub selection: ReferenceQuery,
    /// One group per selected symbol, ordered by its first declaration.
    pub groups: Vec<DefinitionGroup>,
}

/// One selected symbol's declarations, as its reference group lists them
/// ([`ReferenceGroup::declarations`]): in location order, including merged
/// declarations outside the searched files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionGroup {
    pub declarations: Vec<NamedLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionOperation {
    Definition,
}

/// A definition result lists every selected symbol's declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionOutcome {
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionSelection {
    pub operation: DefinitionOperation,
    pub project: PathBuf,
    /// As [`ReferenceResult::selection`] echoes it.
    pub selection: ReferenceQuery,
    #[serde(flatten)]
    pub outcome: DefinitionSelectionFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum DefinitionSelectionFailure {
    /// The position is in no identifier and no member site, or no
    /// declaration, local import, or exported alias is spelled the name.
    NotFound,
    /// The position is a member site, whose member the reference selection
    /// selected: a reference query at the same position (`ReferenceQuery::At`)
    /// searches it and names its declarations. `name` is its decoded name.
    Member { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceOutcome {
    Complete,
    Partial,
}

/// The question whose candidate accounting the result describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceBaseline {
    Lexical,
    Member,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCoverage {
    pub baseline: ReferenceBaseline,
    pub status: ReferenceOutcome,
    pub gaps: Vec<ReferenceGap>,
}

/// A candidate whose semantic dependencies could not establish a reference.
/// Resource and internal failures abort a query; they are never coverage gaps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceGap {
    pub reason: ReferenceGapReason,
    pub location: Location,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub causes: Vec<ReferenceCause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_causes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCause {
    pub reason: ReferenceGapReason,
    /// Compiler global dependencies can have no declaration site. The gap's
    /// candidate location must not be substituted for an absent cause site.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    pub message: String,
}

/// The shared unsupported/invalid vocabulary. A lexical query produces no
/// semantic gaps and does not run semantic checking.
pub use crate::analysis::SemanticFailureReason as ReferenceGapReason;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsearchedMembers {
    pub count: usize,
    pub selectors: Vec<MemberSuggestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberSuggestion {
    pub file: String,
    /// Absent when the existing selector language cannot spell an ancestor
    /// key. The catalog anchor still identifies the declaration for Q2.
    pub selector: Option<String>,
    /// Q2's declaration anchor. A parameter-property name addresses its
    /// lexical parameter; a semantic member modifier anchor is a separate role.
    pub anchor: ReferenceAnchor,
    /// The position whose reference query (`ReferenceQuery::At`) searches
    /// this member, as a dotted selector naming it alone would: the anchor,
    /// or a parameter property's first property-producing modifier. Present
    /// in `Project::references` answers, which select semantically;
    /// `Project::find_references` makes no semantic request and leaves it
    /// out, as it does for a member whose name has no reference token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<ReferenceAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceAnchor {
    pub file: String,
    #[serde(flatten)]
    pub position: SourcePosition,
}

/// Every searched occurrence of one underlying symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceGroup {
    /// The symbol's declarations, including merged declarations outside the
    /// searched files.
    pub declarations: Vec<NamedLocation>,
    /// Declarations, uses, and import/export names in the searched files.
    pub occurrences: Vec<Occurrence>,
}

/// A source range in one of the project's files: `file` uses `/`
/// separators relative to the configuration directory; `end` is exclusive.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedLocation {
    pub name: String,
    pub location: Location,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occurrence {
    pub name: String,
    pub location: Location,
    pub roles: Vec<Role>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Declaration,
    Use,
    Import,
    Export,
}

pub(crate) mod metadata;
