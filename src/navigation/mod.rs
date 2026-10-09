//! Declaration navigation: an outline of a file's declaration sites and the
//! exact source of one selected declaration, for one named file
//! (`outline_file`, `show_file`) or a project's searched files
//! (`Project::outline`, `Project::show`, and `ProjectRequest`).
//!
//! ```no_run
//! use std::path::Path;
//! use typescript_parser::navigation::{self, Selector, ShowOutcome};
//!
//! let outline = navigation::outline_file(Path::new("src/host.ts"))?;
//! for entry in &outline.entries {
//!     println!("{} {} {}", entry.depth, entry.kind, entry.label);
//! }
//! let shown = navigation::show_file(
//!     Path::new("src/host.ts"),
//!     &Selector::Name("createRunHost.start".into()),
//! )?;
//! if let ShowOutcome::Found(found) = shown.outcome {
//!     print!("{}", found.source);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Every operation uses one declaration catalog per file: entries are
//! syntax sites, not merged symbols, so overloads, merged interfaces, and two
//! blocks of one namespace are separate entries. Projection (`projection`)
//! owns which syntax is a declaration, its kind, name, containment, ranges, and
//! signature; the catalog (`catalog`) owns selection and pairs its records
//! with the captured source they describe; `file` owns the complete file
//! operation, from reading the file to its parsing policy. See
//! `docs/navigation.md` for the contract and its differences from TypeScript's
//! document symbols.

mod catalog;
mod file;
mod names;
mod projection;
mod signature;

pub(crate) use catalog::{Catalog, Index, SiteSyntax, Syntax, outcome};
pub use file::{outline_file, show_file};

use crate::analysis::{InputError, InputErrorKind, QueryError};
use crate::input::InputKind;
use crate::source::PositionError;
use crate::source::SourcePosition;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// The syntax family of a declaration site. Text and JSON output share this
/// one vocabulary; a function-valued variable is still a `Variable`, and a
/// class arrow field or object property is still a `Property`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Function,
    Variable,
    Class,
    Interface,
    Type,
    Enum,
    Namespace,
    /// `declare module "m"` and `declare global`.
    Module,
    Method,
    Accessor,
    Constructor,
    /// Call, construct, and index signatures.
    Signature,
    Property,
    EnumMember,
    Import,
    Export,
}

impl Kind {
    /// The spelling text and JSON output use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Variable => "variable",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Type => "type",
            Self::Enum => "enum",
            Self::Namespace => "namespace",
            Self::Module => "module",
            Self::Method => "method",
            Self::Accessor => "accessor",
            Self::Constructor => "constructor",
            Self::Signature => "signature",
            Self::Property => "property",
            Self::EnumMember => "enum_member",
            Self::Import => "import",
            Self::Export => "export",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A source range with one-based lines and UTF-16 columns; `end` is exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

/// One declaration site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declaration {
    /// Presentation: the declared name as written (identifiers decoded,
    /// string names quoted and escaped losslessly), or the label the catalog
    /// gives an unnamed site, such as `default` or `describe('x') callback`.
    /// Never truncated, and never a lookup identity.
    pub label: String,
    /// The exact declared name `--name` matches: a decoded identifier or
    /// private name, an exactly representable string name, or a numeric
    /// name's normalized text; a dotted namespace joins its segments. Absent
    /// for computed names, names with an unpaired surrogate, unnamed sites,
    /// a default import's name, and an import or export specifier that does
    /// not rename, which TypeScript's project-wide name search does not
    /// count as declarations; all remain selectable by position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// The qualified name selecting this site by containment: the keys of
    /// its keyed ancestors and its own key, joined with `.`, present exactly
    /// when `--name` can spell it, that is, when every segment is non-empty
    /// and contains no `.`. It may select other sites too, such as
    /// overloads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    pub kind: Kind,
    /// Where `--at` selects this site: its name token, the first segment of a
    /// dotted namespace, or the first token of an unnamed site.
    pub anchor: SourcePosition,
    /// The declaration's source, from its first token, including decorators
    /// and modifiers, through its last token. A variable binding's range is
    /// its variable statement, and an import or export specifier's range is
    /// its whole import or export declaration.
    pub range: SourceRange,
    /// The written header on one line, without inferred types: whitespace
    /// and comments between tokens collapse to one space, nested bodies and
    /// long text abbreviate with `…`, and tokens are never altered. Absent
    /// where the label says more, such as callbacks, bindings in a pattern,
    /// and import or export specifiers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Containment level: 1 for a declaration with no enclosing declaration.
    pub depth: usize,
}

/// A file's complete declaration hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outline {
    /// The base of every `file` in a project result: the selected
    /// configuration file, in the canonical form of its directory, as
    /// `ReferenceResult::project` names it. Absent for file navigation.
    pub project: Option<PathBuf>,
    /// The navigated file: in a project, its `/`-separated path relative to
    /// the configuration directory, as `references::Location::file` gives
    /// it; in file navigation, its absolute path.
    pub file: String,
    /// Every declaration in source order; each entry's children follow it
    /// with greater `depth`. Display depth is a presentation choice.
    pub entries: Vec<Declaration>,
}

/// What `show` selects: an exact name, optionally qualified by containment
/// (`a.b.c`), or the anchor or name token at a position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Selector {
    Name(String),
    At(SourcePosition),
}

/// What `show` selects in a project: declarations matching a name in every
/// searched file or in one of them, or the declaration named or anchored at
/// a position in one searched file. Relative paths use the current working
/// directory and are matched lexically, as reference queries match them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShowQuery {
    /// `Selector::Name` across the searched files, or only in `file`.
    Name { name: String, file: Option<PathBuf> },
    /// `Selector::At` in `file`.
    At {
        file: PathBuf,
        position: SourcePosition,
    },
}

/// A selection result. Ambiguity and absence are outcomes, not errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Show {
    /// The base of every file in a project result, as in `Outline::project`;
    /// absent for file navigation.
    pub project: Option<PathBuf>,
    /// The file the selection searched, in `Outline::file`'s form, or absent
    /// when a name searched every file of a project.
    pub file: Option<String>,
    pub query: Selector,
    pub outcome: ShowOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShowOutcome {
    /// Exactly one declaration matched.
    Found(Found),
    /// Several matched, ordered by file and then source position; none is
    /// chosen.
    Ambiguous(Vec<Candidate>),
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Found {
    /// The declaration's file, in `Outline::file`'s form.
    pub file: String,
    pub declaration: Declaration,
    /// The exact source slice of `declaration.range`.
    pub source: String,
}

impl Found {
    /// The source split at ECMAScript line breaks, each part with its original
    /// one-based line number and without its line break.
    pub fn lines(&self) -> impl Iterator<Item = (usize, &str)> {
        let text = self.source.as_str();
        let breaks: Vec<_> = crate::source::line_breaks(text).collect();
        let starts = std::iter::once(0).chain(breaks.iter().map(|line_break| line_break.end));
        let ends = breaks
            .iter()
            .map(|line_break| line_break.start)
            .chain(std::iter::once(text.len()));
        let first = self.declaration.range.start.line;
        let lines: Vec<(usize, &str)> = starts
            .zip(ends)
            .enumerate()
            .map(|(index, (start, end))| (first + index, &text[start..end]))
            .collect();
        lines.into_iter()
    }
}

/// An ambiguous match, with an anchor that selects it with `--at` in its
/// file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// The candidate's file, in `Outline::file`'s form.
    pub file: String,
    pub label: String,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    pub anchor: SourcePosition,
}

/// The failure for navigating a program input that is not TypeScript, such
/// as a JSON module, whether a file or a project names it.
pub(crate) fn not_navigation_input(kind: InputKind, path: &Path) -> InputError {
    InputError {
        kind: InputErrorKind::Unsupported,
        message: format!("{} files are not navigation input", kind.label()),
        path: Some(path.to_path_buf()),
        position: None,
    }
}

/// The failure for a selector's position that addresses no position in
/// `file`, named as the request named it.
pub(crate) fn invalid_position(
    error: PositionError,
    file: &Path,
    selector: &Selector,
) -> QueryError {
    let position = match selector {
        Selector::At(position) => Some(*position),
        Selector::Name(_) => None,
    };
    QueryError::invalid_position(error, file, position)
}
