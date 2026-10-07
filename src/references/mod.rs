//! Project-wide reference lookup for one configured TypeScript project.
//!
//! ```no_run
//! use std::path::Path;
//! use typescript_parser::references::{ReferenceProject, ReferenceQuery};
//!
//! let project = ReferenceProject::load(Path::new("tsconfig.json"))?;
//! let result = project.find_references(ReferenceQuery::Name {
//!     name: "run".into(),
//!     file: None,
//! })?;
//! println!("{}", serde_json::to_string_pretty(&result)?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod binding;
mod pattern;
mod project;
mod query;
mod session;
mod source;

pub use session::{ReferenceError, ReferenceSession};

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// An immutable reference index for one configured project.
///
/// Loading reads each participating file once and resolves the supported
/// declaration and module relationships. Queries perform no filesystem I/O.
/// Results describe those captured contents, not later edits on disk.
/// Scope tables, aliases, and internal IDs never escape this object.
#[derive(Debug)]
pub struct ReferenceProject {
    config: PathBuf,
    sources: source::Sources,
    files: project::FileLookup,
    index: binding::Index,
}

impl ReferenceProject {
    /// Load the project selected by a `tsconfig.json`-style file. Nothing
    /// searches ancestor directories for another configuration. Parsing runs
    /// on a dedicated thread whose stack fits a deeper syntax budget than
    /// `ParseOptions::default()`, for generated declaration files.
    pub fn load(config: &Path) -> Result<Self, ProjectError> {
        Self::on_loading_thread(config, || {
            Self::load_here(config, &project::disk::Disk::default())
        })
    }

    fn on_loading_thread<T: Send>(
        config: &Path,
        load: impl FnOnce() -> Result<T, ProjectError> + Send,
    ) -> Result<T, ProjectError> {
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("tsparse-references-load".into())
                .stack_size(project::LOAD_STACK_BYTES)
                .spawn_scoped(scope, load)
                .map_err(|error| ProjectError {
                    kind: ProjectErrorKind::Io,
                    message: format!("cannot start the loading thread: {error}"),
                    path: Some(config.to_path_buf()),
                    position: None,
                })?
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    }

    fn load_here(config: &Path, disk: &project::disk::Disk) -> Result<Self, ProjectError> {
        let program = project::load(config, disk)?;
        let index = binding::bind(&program)?;
        let files = program.file_lookup();
        Ok(Self {
            config: program.config,
            sources: program.sources,
            files,
            index,
        })
    }

    /// Find the reference groups selected by a source position or an exact
    /// declared name. Relative query paths use the current working directory
    /// and are matched lexically, without reading the filesystem.
    pub fn find_references(&self, query: ReferenceQuery) -> Result<ReferenceResult, QueryError> {
        let groups = query::find(self, query)?;
        Ok(ReferenceResult {
            project: self.config.clone(),
            groups,
        })
    }
}

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

/// One-based line and one-based UTF-16 column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
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

/// A source range. `file` uses `/` separators relative to the configuration
/// directory; `end` is exclusive.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Declaration,
    Use,
    Import,
    Export,
}

/// A project that could not be constructed. No partial index exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectError {
    pub kind: ProjectErrorKind,
    pub message: String,
    pub path: Option<PathBuf>,
    pub position: Option<SourcePosition>,
}

/// Serialized as the variant name, the stable spelling tools compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectErrorKind {
    /// A file or configuration could not be read, or the system refused a
    /// resource loading needs, such as its thread.
    Io,
    /// Invalid configuration content or inheritance.
    Config,
    /// A program input has a syntax error.
    Syntax,
    /// A required input, option, or construct is outside the supported profile.
    Unsupported,
    /// Declarations that cannot share a name.
    Binding,
    /// A module that no file or ambient declaration provides.
    UnresolvedImport,
}

/// A query that could not produce a reference result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryError {
    pub kind: QueryErrorKind,
    pub message: String,
    /// The file as the query spelled it for `OutsideProject` and
    /// `InvalidPosition`, which concern the query itself; otherwise the
    /// selected token's file as a canonical absolute path, whichever entry
    /// point selected it.
    pub path: Option<PathBuf>,
    pub position: Option<SourcePosition>,
}

/// Serialized as the variant name, the stable spelling tools compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryErrorKind {
    /// The query file is not one of the project's searched files, including a
    /// path that cannot be made absolute against the working directory.
    OutsideProject,
    /// A line/column outside the file or inside a surrogate pair.
    InvalidPosition,
    /// The selected identifier is a member, label, or other unsupported target.
    UnsupportedTarget,
    /// The selected identifier, or a name candidate, has no resolvable target.
    UnresolvedTarget,
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_location(f, self.path.as_deref(), self.position)?;
        f.write_str(&self.message)
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_location(f, self.path.as_deref(), self.position)?;
        f.write_str(&self.message)
    }
}

fn write_location(
    f: &mut fmt::Formatter<'_>,
    path: Option<&Path>,
    position: Option<SourcePosition>,
) -> fmt::Result {
    match (path, position) {
        (Some(path), Some(position)) => write!(
            f,
            "{}:{}:{}: ",
            path.display(),
            position.line,
            position.column
        ),
        (Some(path), None) => write!(f, "{}: ", path.display()),
        _ => Ok(()),
    }
}

impl std::error::Error for ProjectError {}
impl std::error::Error for QueryError {}
