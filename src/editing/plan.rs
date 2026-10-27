//! Saved plans: a prepared batch as an inspectable JSON file, and loading
//! one back. A plan records each file's original bytes, identity, analysis
//! name, edits with their sites' anchors and fragments, and proposed text.
//! Loading trusts none of it as validated: it prepares the recorded edits
//! again against the recorded originals, through the one construction of
//! a batch, and accepts the plan only if that gives the recorded text. It
//! reads no current file, fragment, or project; applying checks the files.
//! A plan from another build is refused rather than migrated.

use super::file::{Captured, Identity};
use super::{Prepared, PreparedBatch, PreparedText, Target};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

const FORMAT: &str = "tsparse-edit-plan";
const VERSION: u32 = 1;
/// The build that writes and reads plans, the service protocol's identity.
const BUILD: &str = env!("TSPARSE_SERVICE_BUILD");

#[derive(Serialize, Deserialize)]
struct Plan {
    format: String,
    version: u32,
    build: String,
    project: Option<PathBuf>,
    files: Vec<FileRecord>,
}

#[derive(Serialize, Deserialize)]
struct FileRecord {
    /// The other paths the request named the file by.
    #[serde(default)]
    aliases: Vec<PathBuf>,
    /// The regular file the analysis path resolved to.
    referent: PathBuf,
    identity: Identity,
    /// Whether the original bytes begin with a UTF-8 byte order mark, which
    /// is not part of the analyzed `original` text.
    byte_order_mark: bool,
    #[serde(flatten)]
    proposal: PreparedText,
}

/// Why a plan could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadError {
    pub kind: LoadErrorKind,
    pub message: String,
    pub path: PathBuf,
    /// The file names the plan records (`Plan::names`), or `None` when its
    /// text could not be read, or decoded as a plan, so which names it
    /// records is unknown: the names a command's outputs must not create.
    #[serde(skip)]
    pub names: Option<Vec<PathBuf>>,
}

impl Plan {
    /// The file names the plan records, as `PreparedBatch::names` gives
    /// them once loaded: the project's configuration, and each file's path,
    /// other names, and referent.
    fn names(&self) -> Vec<PathBuf> {
        let mut names: Vec<PathBuf> = self.project.iter().cloned().collect();
        for record in &self.files {
            names.push(record.proposal.path.clone());
            names.extend(record.aliases.iter().cloned());
            names.push(record.referent.clone());
        }
        names
    }
}

/// Serialized as the variant name, the stable spelling tools compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadErrorKind {
    /// The plan could not be read.
    Io,
    /// The plan is not a plan.
    Malformed,
    /// Another build or format wrote the plan; prepare it again.
    Incompatible,
    /// Preparing the recorded edits again does not give the recorded text.
    Replay,
    /// A recorded file is one editing refuses for its kind, such as a
    /// JavaScript file, which preparing it would refuse too; the error's
    /// path names that file.
    Unsupported,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for LoadError {}

impl PreparedBatch {
    /// Save this batch as a plan at `path`, which must not exist yet, so a
    /// plan never replaces a target or another input.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let plan = Plan {
            format: FORMAT.into(),
            version: VERSION,
            build: BUILD.into(),
            project: self.project.clone(),
            files: self
                .files
                .iter()
                .map(|file| FileRecord {
                    referent: file.captured.referent.clone(),
                    identity: file.captured.identity,
                    byte_order_mark: file.captured.byte_order_mark(),
                    aliases: file.captured.aliases.clone(),
                    proposal: file.proposal.clone(),
                })
                .collect(),
        };
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        let mut text = serde_json::to_string_pretty(&plan).map_err(std::io::Error::other)?;
        text.push('\n');
        output.write_all(text.as_bytes())?;
        output.sync_all()
    }

    /// Load the plan at `path` by preparing its recorded edits again.
    pub fn load(path: &Path) -> Result<Self, LoadError> {
        let text = std::fs::read_to_string(path).map_err(|io| LoadError {
            kind: LoadErrorKind::Io,
            message: io.to_string(),
            path: path.to_path_buf(),
            // A plan that does not exist records nothing.
            names: crate::input::is_absence(&io).then(Vec::new),
        })?;
        let plan: Plan = serde_json::from_str(&text).map_err(|json| LoadError {
            kind: LoadErrorKind::Malformed,
            message: format!("not an edit plan: {json}"),
            path: path.to_path_buf(),
            names: None,
        })?;
        let names = plan.names();
        let error = |kind, message: String| LoadError {
            kind,
            message,
            path: path.to_path_buf(),
            names: Some(names.clone()),
        };
        if plan.format != FORMAT || plan.version != VERSION || plan.build != BUILD {
            return Err(error(
                LoadErrorKind::Incompatible,
                "another build of tsparse saved this plan; prepare the edits again".into(),
            ));
        }
        let mut targets = Vec::with_capacity(plan.files.len());
        for record in &plan.files {
            let captured = Captured::recorded(
                record.proposal.path.clone(),
                record.aliases.clone(),
                record.referent.clone(),
                record.identity,
                crate::source::encode(record.byte_order_mark, &record.proposal.original),
            )
            .map_err(|refused| {
                if refused.file_kind {
                    // The product policy refuses the file as preparing it
                    // would, naming it.
                    LoadError {
                        kind: LoadErrorKind::Unsupported,
                        message: refused.error.message,
                        path: refused.error.path.unwrap_or_else(|| path.to_path_buf()),
                        names: Some(names.clone()),
                    }
                } else {
                    error(
                        LoadErrorKind::Malformed,
                        format!("a recorded file cannot be edited: {}", refused.error),
                    )
                }
            })?;
            targets.push(Target::replay(captured, &record.proposal));
        }
        let replay = |message: String| error(LoadErrorKind::Replay, message);
        match super::construct(plan.project.clone(), targets) {
            Ok(Prepared::Ready(batch)) => Ok(batch),
            Ok(Prepared::Rejected(rejection)) => Err(replay(rejection.message)),
            Ok(Prepared::Unselected(unselected)) => Err(replay(format!(
                "edit {}'s recorded site no longer selects one declaration",
                unselected.edit + 1
            ))),
            Err(analysis) => Err(replay(analysis.to_string())),
        }
    }
}
