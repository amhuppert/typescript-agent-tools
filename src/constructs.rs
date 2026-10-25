//! Owned construction-site requests and results (Q5) from one Project
//! generation: the object literals whose contextual type constructs a selected
//! interface, class or type alias, and the properties each does not certainly
//! provide. Types, symbols and checker state stay private.
//!
//! Presence is the compiler's named-property model of the checked literal
//! type, not a statement about the runtime object (docs/constructs-design.md).

use crate::references::{Location, ReferenceGap};
use crate::types::{SelectionFailure, TypeKey, TypeQuery};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Q4's selection of a declared type, with the sites to report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructsRequest {
    pub query: TypeQuery,
    pub filters: ConstructsFilters,
}

impl ConstructsRequest {
    pub fn new(query: TypeQuery) -> Self {
        Self {
            query,
            filters: ConstructsFilters::default(),
        }
    }
}

/// Which sites a result reports. `exclude_tests` and `file` are the reference
/// filters' rules; `missing` keeps the sites at which a constructed
/// alternative declares that string key (any exact string, the empty one
/// included) and does not certainly provide it;
/// `all_literals` examines every literal rather than the default universe
/// (`LiteralUniverse`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructsFilters {
    pub exclude_tests: bool,
    pub file: Option<PathBuf>,
    pub missing: Option<String>,
    pub all_literals: bool,
}

/// A searched result, or honest absence/ambiguity of the selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConstructsAnswer {
    Selected(Box<ConstructsResult>),
    Unselected(ConstructsSelection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstructsOperation {
    Constructs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructsSelection {
    pub operation: ConstructsOperation,
    pub project: PathBuf,
    pub selection: TypeQuery,
    #[serde(flatten)]
    pub outcome: SelectionFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstructsOutcome {
    /// Every examined literal was decided.
    Complete,
    /// At least one examined literal is a gap.
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructsResult {
    pub operation: ConstructsOperation,
    pub outcome: ConstructsOutcome,
    pub project: PathBuf,
    pub selection: TypeQuery,
    pub target: ConstructsTarget,
    pub filters: ConstructsFilters,
    pub sites: Vec<ConstructionSite>,
    pub coverage: ConstructsCoverage,
}

/// A bounded compiler spelling of a type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub summary: String,
    pub summary_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructsTarget {
    #[serde(flatten)]
    pub summary: Summary,
    pub location: Location,
    pub alternatives: Vec<TargetAlternative>,
}

/// One object alternative of the target's declared type. `id` is local to
/// the result; `origin` is the declaration that writes the alternative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetAlternative {
    pub id: String,
    #[serde(flatten)]
    pub summary: Summary,
    pub origin: Option<Location>,
}

/// An object literal and the target alternatives it constructs. `location`
/// is the literal's whole span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionSite {
    pub location: Location,
    pub contextual_type: Summary,
    pub constructs: Vec<Construction>,
}

/// One constructed alternative, as instantiated at the site, and each of its
/// properties the literal does not certainly provide, in its property order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Construction {
    pub alternative: String,
    #[serde(flatten)]
    pub summary: Summary,
    pub lacks: Vec<Lack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lack {
    pub key: TypeKey,
    /// The alternative's own requirement: Optional or Partial, as
    /// `getUnmatchedProperties` tests it.
    pub optional: bool,
    pub presence: Presence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    /// The checked literal type has no such named property and no index
    /// signature that applies to its name.
    Absent,
    /// The literal type may provide it: it is optional or partial there, an
    /// index signature applies, or the type is `any`.
    Unknown,
}

/// `literals` counts the object literals in the files searched, `examined`
/// those of the `universe` whose construction question was asked, and
/// `sites` those reported before display cuts; `status` equals the outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructsCoverage {
    pub status: ConstructsOutcome,
    pub universe: LiteralUniverse,
    pub literals: usize,
    pub examined: usize,
    pub sites: usize,
    pub gaps: Vec<ReferenceGap>,
}

/// The literals a search examined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiteralUniverse {
    /// Each literal with a member naming a key some target alternative
    /// declares, or with no named member (`{}`, or only spreads and
    /// late-bound names). A literal whose named members all name other keys
    /// is not examined (docs/constructs.md says which constructions that can
    /// omit).
    TargetKeys,
    /// Every literal: requested, or because an alternative's index signature
    /// lets any key be a target key.
    All,
}
