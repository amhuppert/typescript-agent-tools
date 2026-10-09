//! What every complete analysis operation shares: project loading and
//! queries, file navigation, and any later reparse of captured project
//! text. They report failures as `AnalysisError`s, with positions in the
//! analyzed text (`crate::source::SourcePosition`), and parse with one
//! policy: the deep grammar budget, on a thread whose stack fits it, so
//! callers never choose a budget or a stack size, and display options such
//! as an outline depth cannot change which syntax an operation accepts.
//!
//! Parse mode keeps `ParseOptions::default()` on the caller's thread.

use crate::parser::json::{JsonError, JsonText};
use crate::source::{PositionError, SourcePosition, SourceText};
use crate::{Ast, Diagnostic, ParseOptions};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

/// The grammar budget of every analysis parse: generated declaration files
/// need several hundred grammar entries. `crate::DEEP_PARSE_STACK_BYTES`
/// fits it.
const BUDGET: usize = ParseOptions::DEEP_MAX_DEPTH;

/// The only means of parsing analyzed text. `on_parsing_thread` alone
/// creates one, on a thread whose stack fits `BUDGET`, and it parses only
/// with that budget, so no caller can pair the budget with another stack
/// or parse analyzed text with another budget. It is not `Sync`, so a
/// reference to it cannot leave its thread.
pub(crate) struct Parser {
    _thread: PhantomData<std::cell::Cell<()>>,
}

impl Parser {
    /// Parse `text`, the analyzed text of the file named `file_name`, with
    /// the options TypeScript derives from that name
    /// (`ParseOptions::for_file`).
    pub(crate) fn parse<'s>(&self, text: &'s str, file_name: &str) -> Result<Ast<'s>, Diagnostic> {
        crate::parse_with_options(
            text,
            ParseOptions {
                max_depth: BUDGET,
                ..ParseOptions::for_file(file_name)
            },
        )
    }

    /// Read `text` as TypeScript reads JSON, the one reader for
    /// configuration files and JSON modules (`parseJSONText`).
    pub(crate) fn parse_json(&self, text: &str) -> Result<JsonText, JsonError> {
        crate::parser::json::parse_json_text(text, BUDGET)
    }
}

/// Run `work` with the `Parser` on a dedicated thread with a stack of
/// `crate::DEEP_PARSE_STACK_BYTES`. Other recursion during project loading,
/// a package `imports` chain of at most `MAX_IMPORTS_CHAIN` lookups, needs
/// under 4 MiB and never runs during a parse. A panic resumes on the
/// caller's thread; failing to start the thread is an I/O error for the
/// caller's own error type.
pub(crate) fn on_parsing_thread<T: Send>(
    name: &str,
    work: impl FnOnce(&Parser) -> T + Send,
) -> std::io::Result<T> {
    std::thread::scope(|scope| {
        let thread = std::thread::Builder::new()
            .name(name.into())
            .stack_size(crate::DEEP_PARSE_STACK_BYTES)
            .spawn_scoped(scope, || {
                work(&Parser {
                    _thread: PhantomData,
                })
            })?;
        Ok(thread
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
    })
}

/// Inputs that could not be analyzed: a project that could not be
/// constructed, in which case no partial snapshot exists; a file that
/// navigation could not read or parse; or a file a request names by a path
/// that cannot name it exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputError {
    pub kind: InputErrorKind,
    pub message: String,
    pub path: Option<PathBuf>,
    pub position: Option<SourcePosition>,
}

/// Serialized as the variant name, the stable spelling tools compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputErrorKind {
    /// A file or configuration could not be read, or the system refused a
    /// resource analysis needs, such as its thread or the working directory
    /// a relative path needs.
    Io,
    /// Invalid configuration content or inheritance.
    Config,
    /// An analyzed input has a syntax error.
    Syntax,
    /// An analyzed input nests deeper than the parser's grammar budget
    /// (`ParseOptions::max_depth`): a limit of this tool, not invalid syntax.
    ResourceLimit,
    /// A required input, option, or construct is outside the supported
    /// profile, including a navigated file that is not TypeScript and a
    /// path that is not Unicode, which names no file exactly.
    Unsupported,
    /// Declarations that cannot share a name.
    Binding,
    /// A module that no file or ambient declaration provides.
    UnresolvedImport,
}

impl InputError {
    /// The failure of parsing the analyzed text `source` of the file at
    /// `path`: the one decision of which kind a parse failure is, for every
    /// operation that parses analyzed text. Exceeding the grammar budget is
    /// a `ResourceLimit`, kept apart from invalid syntax, which is `Syntax`.
    pub(crate) fn unparsable(error: Diagnostic, source: &SourceText, path: &Path) -> Self {
        Self {
            kind: if crate::parser::exhaustion(&error) {
                InputErrorKind::ResourceLimit
            } else {
                InputErrorKind::Syntax
            },
            position: Some(source.position(error.span.start)),
            message: error.message,
            path: Some(path.to_path_buf()),
        }
    }
}

/// A request that does not fit the analyzed inputs.
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
    /// The query file is not one of the project's searched files, including
    /// an empty path, which names no file.
    OutsideProject,
    /// A line/column outside the file or inside a surrogate pair.
    InvalidPosition,
    /// The selected identifier is a member, label, or other unsupported target.
    UnsupportedTarget,
    /// The selected identifier, or a name candidate, has no resolvable target.
    UnresolvedTarget,
}

impl QueryError {
    /// A position a request names in `file` that addresses no position in
    /// it, with the file as the request spelled it: every operation's
    /// `InvalidPosition`.
    pub(crate) fn invalid_position(
        error: PositionError,
        file: &Path,
        position: Option<SourcePosition>,
    ) -> Self {
        Self {
            kind: QueryErrorKind::InvalidPosition,
            message: error.to_string(),
            path: Some(file.to_path_buf()),
            position,
        }
    }
}

/// Why an analysis request failed: its inputs could not be analyzed, or the
/// request does not fit them. Every operation, whether on a project, a
/// session, or one navigated file, reports failures as these kinds, each
/// with its own location; they stay distinct until a caller decides the
/// response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisError {
    Input(InputError),
    Query(QueryError),
}

impl From<InputError> for AnalysisError {
    fn from(error: InputError) -> Self {
        Self::Input(error)
    }
}

impl From<QueryError> for AnalysisError {
    fn from(error: QueryError) -> Self {
        Self::Query(error)
    }
}

impl fmt::Display for InputError {
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

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => error.fmt(f),
            Self::Query(error) => error.fmt(f),
        }
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

impl std::error::Error for InputError {}
impl std::error::Error for QueryError {}
impl std::error::Error for AnalysisError {}
