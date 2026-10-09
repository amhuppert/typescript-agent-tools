//! Source-preserving edits: select a declaration as `show` does, supply new
//! source, inspect the exact change, and apply precisely that change.
//!
//! - **E1** (`Operation::ReplaceBody`) replaces the complete body of one
//!   selected callable: a function, method, constructor, accessor, or arrow
//!   function, or a variable or property whose initializer is directly one.
//! - **E2** (`Operation::InsertAfter`) inserts complete statements right
//!   after the statement a selected declaration designates, in the same
//!   statement list.
//!
//! Preparation selects every edit's site in the original captured sources,
//! composes all edits to a file together, parses the result in its actual
//! file name, and checks that the syntax it preserves still corresponds and
//! that comment directives and JSDoc keep governing what they governed.
//! It returns a complete prepared batch or a diagnostic, never part of one.
//! Validation is syntax and edit-boundary validation: an edit is not type
//! checked. See `docs/editing-design.md`.
//!
//! ```no_run
//! use std::path::PathBuf;
//! use typescript_parser::editing::{self, Edit, EditRequest, Operation, Prepared};
//! use typescript_parser::navigation::ShowQuery;
//!
//! let request = EditRequest {
//!     edits: vec![Edit {
//!         operation: Operation::ReplaceBody,
//!         target: ShowQuery::Name {
//!             name: "createRunHost.start".into(),
//!             file: Some(PathBuf::from("src/host.ts")),
//!         },
//!         fragment: "{ return true; }".into(),
//!     }],
//! };
//! if let Prepared::Ready(batch) = editing::prepare(&request)? {
//!     print!("{}", batch.files()[0].proposed_text());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod apply;
mod compare;
mod compose;
mod diff;
mod file;
mod plan;
mod policy;
mod prepare;
mod seam;

use crate::analysis::{AnalysisError, InputError, InputErrorKind};
use crate::navigation::{Declaration, Selector, ShowOutcome, ShowQuery};
use crate::source::SourcePosition;
use crate::{Span, analysis};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use plan::{LoadError, LoadErrorKind};

/// What an edit does at its selected site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// E1: replace a callable's complete body. The fragment is the new body:
    /// a block, or, for an arrow function, an expression.
    ReplaceBody,
    /// E2: insert one or more complete statements after the statement the
    /// site designates.
    InsertAfter,
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReplaceBody => "replace_body",
            Self::InsertAfter => "insert_after",
        }
    }
}

/// One requested edit: an operation, the declaration it targets, selected
/// as `show` selects, and the new source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    pub operation: Operation,
    pub target: ShowQuery,
    /// The new source, exactly as supplied.
    pub fragment: String,
}

/// Edits prepared together against the same original sources. Each
/// selects its site in the originals, not in the results of the others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditRequest {
    pub edits: Vec<Edit>,
}

/// One change to a file's analyzed text, in its original coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// The replaced bytes of the original text; empty for an insertion.
    pub span: Span,
    pub start: SourcePosition,
    pub end: SourcePosition,
    /// The new text, including every generated separator and indentation.
    pub text: String,
}

/// One edit as prepared: its selected site, the fragment as supplied, and
/// the changes it makes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedEdit {
    /// The edit's position in its request.
    pub index: usize,
    pub operation: Operation,
    /// The selected declaration, as `show` describes it; its anchor selects
    /// it again.
    pub site: Declaration,
    pub fragment: String,
    pub changes: Vec<Change>,
}

/// One file's prepared text: its original and proposed analyzed text and
/// the edits that make the difference, with what preparing them again needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedText {
    /// The file analysis parsed, absolute: in a project, its canonical path.
    pub path: PathBuf,
    /// The file as results name it: in a project, relative to the
    /// configuration's directory; otherwise the absolute path.
    pub display: String,
    pub original: String,
    pub proposed: String,
    pub edits: Vec<PlannedEdit>,
}

/// Edits selected over one generation of a project: each edited file's
/// captured text and the edits to prepare in it. Selecting reads no file;
/// `bind` prepares the edits against the files on disk, which must still
/// hold the selected text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// The project's configuration.
    pub project: PathBuf,
    pub files: Vec<SelectedFile>,
}

/// One file of a selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedFile {
    /// The file's canonical path, which analysis parses it by.
    pub path: PathBuf,
    /// The file relative to the configuration's directory.
    pub display: String,
    /// The analyzed text the project captured.
    pub text: String,
    pub edits: Vec<SelectedEdit>,
}

/// One edit of a selected file: a selector within that file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedEdit {
    /// The edit's position in its request.
    pub index: usize,
    pub operation: Operation,
    pub selector: Selector,
    pub fragment: String,
}

/// An edit whose selection matched no site or several.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unselected {
    /// The edit's position in its request.
    pub edit: usize,
    /// The file the selection searched, as `Show::file`; absent when a name
    /// searched every file of a project.
    pub file: Option<String>,
    pub query: Selector,
    pub outcome: ShowOutcome,
}

/// Why requested edits cannot be made as requested. Nothing was changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejection {
    pub kind: RejectionKind,
    pub message: String,
    /// The edit concerned, by its position in the request.
    pub edit: Option<usize>,
    /// The file concerned, as results name it.
    pub file: Option<String>,
    /// Where in the original file, if the problem is in preserved text.
    pub position: Option<SourcePosition>,
    /// Where in the proposed text, with that line, for a problem there.
    pub proposed: Option<(SourcePosition, String)>,
}

/// Serialized as the variant name, the stable spelling tools compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionKind {
    /// The selected site does not support the operation.
    UnsupportedSite,
    /// Two edits' changes overlap, nest, repeat, or tie.
    Conflict,
    /// The proposed file does not parse.
    Syntax,
    /// The proposed file parses, but syntax the edit preserves changed or
    /// new syntax escaped its slot.
    Boundary,
    /// A comment directive or JSDoc comment would govern different code.
    Attachment,
    /// The proposed file exceeds the parser's nesting budget.
    ResourceLimit,
    /// A file changed after its proposal was prepared.
    Stale,
    /// Recorded edits, prepared again, give other text than was recorded.
    Replay,
}

/// The outcome of preparing edits: what was prepared, or why nothing was.
/// Selection misses and rejections are outcomes, as in `show`; inputs that
/// cannot be analyzed are `AnalysisError`s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome<T> {
    Ready(T),
    Unselected(Unselected),
    Rejected(Rejection),
}

/// Preparing edits of files on disk: an applicable batch.
pub type Prepared = Outcome<PreparedBatch>;

/// Selecting edits in a project: a selection, which `bind` prepares.
pub type Selected = Outcome<Selection>;

/// A prepared batch: the exact original and proposed bytes of each changed
/// file, its identity, and the edits that make the change. Only preparation
/// constructs one; it applies precisely these bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedBatch {
    project: Option<PathBuf>,
    files: Vec<PreparedFile>,
}

/// One file of a prepared batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedFile {
    captured: file::Captured,
    proposal: PreparedText,
    proposed: Vec<u8>,
}

impl PreparedBatch {
    /// The configuration of a project batch.
    pub fn project(&self) -> Option<&Path> {
        self.project.as_deref()
    }

    /// The file names the batch, and a plan saving it, records: the
    /// project's configuration, and each file's names in the request and the
    /// file they resolved to. A plan that decodes but cannot be loaded gives
    /// the same names (`LoadError::names`).
    pub fn names(&self) -> Vec<&Path> {
        let mut names: Vec<&Path> = self.project().into_iter().collect();
        for file in &self.files {
            let captured = &file.captured;
            names.push(&captured.path);
            names.extend(captured.aliases.iter().map(PathBuf::as_path));
            names.push(&captured.referent);
        }
        names
    }

    /// Every file the batch edits, in request order of first mention.
    pub fn files(&self) -> &[PreparedFile] {
        &self.files
    }

    /// Whether applying would change no file.
    pub fn is_empty(&self) -> bool {
        self.files.iter().all(|file| !file.changes())
    }
}

impl PreparedFile {
    /// The file as the request named it, absolute.
    pub fn path(&self) -> &Path {
        &self.captured.path
    }

    /// The file as results name it.
    pub fn display(&self) -> &str {
        &self.proposal.display
    }

    pub fn original_text(&self) -> &str {
        &self.proposal.original
    }

    pub fn proposed_text(&self) -> &str {
        &self.proposal.proposed
    }

    /// The exact bytes on disk when prepared, including any byte order mark.
    pub fn original_bytes(&self) -> &[u8] {
        self.captured.image()
    }

    /// The exact bytes applying writes.
    pub fn proposed_bytes(&self) -> &[u8] {
        &self.proposed
    }

    pub fn edits(&self) -> &[PlannedEdit] {
        &self.proposal.edits
    }

    /// Whether applying changes this file.
    pub fn changes(&self) -> bool {
        self.proposed != self.captured.image()
    }

    /// A unified diff of the file's original and proposed text, labeled
    /// with its display name: empty when it does not change. For reading
    /// only; applying writes `proposed_bytes`.
    pub fn diff(&self) -> String {
        diff::unified(
            &self.proposal.display,
            &self.proposal.original,
            &self.proposal.proposed,
        )
    }
}

/// What applying a batch did to the files on disk, independently of
/// whether it succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    /// The batch changes no file, so nothing was written.
    NoChange,
    /// A check, staging, or the first replacement failed: no file was
    /// replaced.
    Unchanged,
    /// Every file was replaced.
    Applied,
    /// Replacing failed after some file was replaced, and every replaced
    /// file was restored.
    RolledBack,
    /// Replacing failed partway, and some replaced file could not be
    /// restored; its original is kept beside it (`FileState`).
    RecoveryRequired,
}

/// What happened to one file of a batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    Unchanged,
    Replaced,
    Restored,
    /// The file could not be restored: what is at its path, and where the
    /// original is kept.
    RecoveryRequired {
        observed: String,
        original: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileOutcome {
    pub path: PathBuf,
    pub display: String,
    pub state: FileState,
}

/// Why applying failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyFailure {
    pub kind: ApplyFailureKind,
    pub message: String,
    pub path: PathBuf,
}

/// Serialized as the variant name, the stable spelling tools compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplyFailureKind {
    /// A file is absent, another file, or has other bytes or metadata than
    /// when the batch was prepared.
    Stale,
    /// A file could not be read, staged, or replaced.
    Io,
}

/// A temporary or backup file applying could not remove.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leftover {
    pub path: PathBuf,
    pub message: String,
}

/// The outcome of applying a batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Applied {
    pub effect: Effect,
    pub failure: Option<ApplyFailure>,
    /// Every file of the batch, in batch order.
    pub files: Vec<FileOutcome>,
    /// Resources that remain after the batch, whatever its effect.
    pub leftovers: Vec<Leftover>,
}

/// A step of applying, in install order of the changing files, at which a
/// test can inject a failure (`PreparedBatch::apply_with`).
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Before staging begins.
    Stage(usize),
    /// After the proposed bytes are staged, before the original is linked.
    Link(usize),
    Install(usize),
    Restore(usize),
    Cleanup(usize),
}

impl PreparedBatch {
    /// Apply exactly this batch's proposed bytes: check every file against
    /// its captured bytes and identity, stage every replacement, check again,
    /// replace the files in path order, and restore replaced files if a
    /// later replacement fails. Names are never selected again. Another
    /// writer must leave the files alone meanwhile; readers may see some
    /// files replaced before others.
    pub fn apply(&self) -> Applied {
        self.apply_with(&mut |_| Ok(()))
    }

    /// `apply`, with `hook` called before each step; an error from it stands
    /// for that step's failure. For deterministic failure tests.
    #[doc(hidden)]
    pub fn apply_with(&self, hook: &mut dyn FnMut(Step) -> std::io::Result<()>) -> Applied {
        apply::apply(&self.files, hook)
    }
}

/// Which of `names`, if any, names the file creating `output`, which must
/// not exist, would create: so a command's new result or plan never creates
/// a file it names, whatever the file system takes for one name (links,
/// case, Unicode forms). When a name does not exist, the file system
/// answers: `output` exists only for the question. An `output` that cannot
/// be created creates no file, and writing it reports why. A failure to
/// look up a name, or to remove `output` again, is an error that says so,
/// never an answer.
pub fn would_create<'a>(output: &Path, names: &[&'a Path]) -> std::io::Result<Option<&'a Path>> {
    file::would_create(output, names)
}

/// Prepare edits of explicitly named files, without a project: each edit
/// names its file, which is read once however many edits name it, and
/// parsed with the shared analysis policy.
pub fn prepare(request: &EditRequest) -> Result<Prepared, AnalysisError> {
    let mut files: Vec<Target<'_>> = Vec::new();
    for (index, edit) in request.edits.iter().enumerate() {
        let (path, selector) = match &edit.target {
            ShowQuery::Name {
                name,
                file: Some(file),
            } => (file, Selector::Name(name.clone())),
            ShowQuery::At { file, position } => (file, Selector::At(*position)),
            ShowQuery::Name { file: None, .. } => {
                return Err(InputError {
                    kind: InputErrorKind::Unsupported,
                    message: "an edit without a project must name its file".into(),
                    path: None,
                    position: None,
                }
                .into());
            }
        };
        let captured = file::capture(path).map_err(AnalysisError::Input)?;
        let request = prepare::Request {
            edit: index,
            operation: edit.operation,
            selector,
            fragment: &edit.fragment,
        };
        match files
            .iter_mut()
            .find(|target| target.captured.referent == captured.referent)
        {
            Some(target) => {
                target.captured.merge(captured)?;
                target.requests.push(request);
            }
            None => files.push(Target {
                display: captured.name().to_owned(),
                captured,
                requests: vec![request],
                expected: None,
            }),
        }
    }
    construct(None, files)
}

/// Prepare a project's selected edits against the files on disk: capture
/// each file here, require its analyzed text to be the text the project
/// selected in, and prepare its edits once, as named files are.
pub fn bind(selection: &Selection) -> Result<Prepared, AnalysisError> {
    let mut targets = Vec::with_capacity(selection.files.len());
    for file in &selection.files {
        let captured = file::capture(&file.path).map_err(AnalysisError::Input)?;
        if captured.text()? != file.text {
            return Ok(Prepared::Rejected(Rejection {
                kind: RejectionKind::Stale,
                message: "the file changed after the project read it; prepare the edit again"
                    .into(),
                edit: file.edits.first().map(|edit| edit.index),
                file: Some(file.display.clone()),
                position: None,
                proposed: None,
            }));
        }
        targets.push(Target {
            captured,
            display: file.display.clone(),
            requests: file
                .edits
                .iter()
                .map(|edit| prepare::Request {
                    edit: edit.index,
                    operation: edit.operation,
                    selector: edit.selector.clone(),
                    fragment: &edit.fragment,
                })
                .collect(),
            expected: None,
        });
    }
    construct(Some(selection.project.clone()), targets)
}

/// The edits to one captured file, with the text they must give when they
/// are prepared again.
struct Target<'r> {
    captured: file::Captured,
    display: String,
    requests: Vec<prepare::Request<'r>>,
    expected: Option<&'r str>,
}

impl<'r> Target<'r> {
    /// The edits of `proposal`, selected again at their anchors in
    /// `captured`, which must give `proposal.proposed`.
    fn replay(captured: file::Captured, proposal: &'r PreparedText) -> Self {
        Self {
            captured,
            display: proposal.display.clone(),
            requests: proposal
                .edits
                .iter()
                .map(|edit| prepare::Request {
                    edit: edit.index,
                    operation: edit.operation,
                    selector: Selector::At(edit.site.anchor),
                    fragment: &edit.fragment,
                })
                .collect(),
            expected: Some(&proposal.proposed),
        }
    }
}

/// The one construction of a prepared batch: prepare each target's edits
/// against its captured text, require replayed targets to give exactly
/// their recorded text, and refuse two targets that are one file.
fn construct(
    project: Option<PathBuf>,
    targets: Vec<Target<'_>>,
) -> Result<Prepared, AnalysisError> {
    for (index, target) in targets.iter().enumerate() {
        if let Some(other) = targets[..index]
            .iter()
            .find(|other| other.captured.referent == target.captured.referent)
        {
            return Ok(Prepared::Rejected(Rejection {
                kind: RejectionKind::Conflict,
                message: format!(
                    "{} and {} are the same file; edit it under one name",
                    other.display, target.display
                ),
                edit: None,
                file: Some(target.display.clone()),
                position: None,
                proposed: None,
            }));
        }
    }
    analysis::on_parsing_thread("tsparse-edit", |parser| {
        let mut proposals = Vec::with_capacity(targets.len());
        for target in &targets {
            let source = std::sync::Arc::new(crate::source::SourceText::new(
                target.captured.text()?.to_owned(),
            ));
            let proposal = match prepare::prepare(
                parser,
                target.captured.name(),
                &target.display,
                &source,
                &target.requests,
            ) {
                Ok(proposal) => proposal,
                Err(failure) => {
                    return failure.into_outcome(&target.captured.path, &target.display);
                }
            };
            if target
                .expected
                .is_some_and(|expected| expected != proposal.proposed)
            {
                return Ok(Prepared::Rejected(Rejection {
                    kind: RejectionKind::Replay,
                    message:
                        "preparing the recorded edits again gives other text than was recorded"
                            .into(),
                    edit: None,
                    file: Some(target.display.clone()),
                    position: None,
                    proposed: None,
                }));
            }
            proposals.push(proposal);
        }
        Ok(Prepared::Ready(PreparedBatch {
            project: project.clone(),
            files: targets
                .into_iter()
                .zip(proposals)
                .map(|(target, proposal)| {
                    let proposed = target.captured.image_of(&proposal.proposed);
                    PreparedFile {
                        captured: target.captured,
                        proposal,
                        proposed,
                    }
                })
                .collect(),
        }))
    })
    .unwrap_or_else(|error| {
        Err(InputError {
            kind: InputErrorKind::Io,
            message: format!("cannot start the parsing thread: {error}"),
            path: None,
            position: None,
        }
        .into())
    })
}
