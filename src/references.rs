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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceResult {
    /// The selected configuration file, in the canonical form of its
    /// directory, which every `Location::file` is relative to.
    pub project: PathBuf,
    /// Separate symbols, ordered by their earliest declaration.
    pub groups: Vec<ReferenceGroup>,
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
