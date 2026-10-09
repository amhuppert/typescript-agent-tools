//! One configured TypeScript project: an immutable snapshot (`Project`)
//! that answers reference and navigation requests, and a refreshable
//! session (`ProjectSession`) for long-lived callers, both through one
//! request type (`ProjectRequest`). Their failures are
//! `crate::analysis::AnalysisError`s.
//!
//! ```no_run
//! use std::path::Path;
//! use typescript_parser::project::{ProjectRequest, ProjectSession};
//! use typescript_parser::references::ReferenceQuery;
//!
//! let mut session = ProjectSession::new(Path::new("tsconfig.json"))?;
//! let response = session.execute(ProjectRequest::References(ReferenceQuery::Name {
//!     name: "run".into(),
//!     file: None,
//! }))?;
//! println!("{}", serde_json::to_string_pretty(&response)?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod binding;
mod editing;
mod navigation;
mod pattern;
mod program;
mod references;
mod session;
mod sources;

pub use session::ProjectSession;

use crate::analysis::{AnalysisError, InputError, InputErrorKind, QueryError, QueryErrorKind};
use crate::navigation::{Outline, Show, ShowQuery};
use crate::references::{ReferenceQuery, ReferenceResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Binding asks the loader's question of whether a module name is relative.
use program::is_relative_name;

/// An immutable snapshot of one configured project: its captured sources,
/// the reference index, and the declaration catalogs of its searched files.
///
/// Loading reads each participating file once and resolves the supported
/// declaration and module relationships. Requests perform no filesystem I/O
/// and never refresh: results describe the captured contents, not later
/// edits on disk. Scope tables, aliases, and internal IDs never escape this
/// object.
#[derive(Debug)]
pub struct Project {
    config: PathBuf,
    sources: sources::Sources,
    files: program::FileLookup,
    index: binding::Index,
    navigation: navigation::Navigation,
}

impl Project {
    /// Load the project selected by a `tsconfig.json`-style file. Nothing
    /// searches ancestor directories for another configuration. Parsing runs
    /// on the shared analysis thread (`crate::analysis`), whose deeper syntax
    /// budget than `ParseOptions::default()` fits generated declaration files.
    pub fn load(config: &Path) -> Result<Self, InputError> {
        Self::on_loading_thread(config, |parser| {
            Self::load_here(config, &program::disk::Disk::default(), parser)
        })
    }

    /// Loading parses with the shared analysis policy, on its thread.
    fn on_loading_thread<T: Send>(
        config: &Path,
        load: impl FnOnce(&crate::analysis::Parser) -> Result<T, InputError> + Send,
    ) -> Result<T, InputError> {
        crate::analysis::on_parsing_thread("tsparse-project-load", load).unwrap_or_else(|error| {
            Err(InputError {
                kind: InputErrorKind::Io,
                message: format!("cannot start the loading thread: {error}"),
                path: Some(config.to_path_buf()),
                position: None,
            })
        })
    }

    fn load_here(
        config: &Path,
        disk: &program::disk::Disk,
        parser: &crate::analysis::Parser,
    ) -> Result<Self, InputError> {
        let mut program = program::load(config, disk, parser)?;
        let index = binding::bind(&program)?;
        let files = program.file_lookup();
        let navigation = navigation::Navigation::new(
            program
                .files
                .iter_mut()
                .map(|file| file.navigation.take())
                .collect(),
        );
        Ok(Self {
            config: program.config,
            sources: program.sources,
            files,
            index,
            navigation,
        })
    }

    /// Answer one request from this snapshot. This is the one dispatch over
    /// the operations, shared by `ProjectSession::execute`; each operation
    /// is also available directly.
    pub fn execute(&self, request: ProjectRequest) -> Result<ProjectResponse, AnalysisError> {
        Ok(match request {
            ProjectRequest::References(query) => {
                ProjectResponse::References(self.find_references(query)?)
            }
            ProjectRequest::Outline { file } => ProjectResponse::Outline(self.outline(&file)?),
            ProjectRequest::Show(query) => ProjectResponse::Show(Box::new(self.show(query)?)),
            ProjectRequest::Edit(request) => ProjectResponse::Edit(Box::new(self.edit(request)?)),
        })
    }

    /// Find the reference groups selected by a source position or an exact
    /// declared name. Relative query paths use the current working directory
    /// and are matched lexically, without reading the filesystem; a path
    /// that is not Unicode names no file (`FileLookup::find`).
    pub fn find_references(&self, query: ReferenceQuery) -> Result<ReferenceResult, AnalysisError> {
        let groups = references::find(self, query)?;
        Ok(ReferenceResult {
            project: self.config.clone(),
            groups,
        })
    }

    /// The searched file a request names, matched lexically without file
    /// access: every operation's membership rule (`FileLookup::find`).
    fn searched_file(&self, file: &Path) -> Result<sources::FileId, AnalysisError> {
        self.files.find(file)?.ok_or_else(|| {
            QueryError {
                kind: QueryErrorKind::OutsideProject,
                message: "not one of the project's searched files".into(),
                path: Some(file.to_path_buf()),
                position: None,
            }
            .into()
        })
    }
}

/// One operation on a project, as `Project::execute` and
/// `ProjectSession::execute` take it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectRequest {
    References(ReferenceQuery),
    /// The outline of one searched file (`Project::outline`).
    Outline {
        file: PathBuf,
    },
    Show(ShowQuery),
    /// Select edits in the searched files (`Project::edit`), which
    /// `crate::editing::bind` prepares. Selecting reads and writes no file.
    Edit(crate::editing::EditRequest),
}

impl ProjectRequest {
    /// Make the request's file operands absolute against the working
    /// directory (`FileLookup::absolute`), so that a process with another
    /// working directory, such as the project's service, answers it as this
    /// one would; and return the operands as spelled, for the diagnostics
    /// that name them (`SpelledOperand::restore`). This is every
    /// operation's rule, for direct and service answers alike.
    pub fn make_absolute(&mut self) -> Result<SpelledOperand, AnalysisError> {
        let mut spelled = Vec::new();
        for file in self.operands() {
            let absolute = program::FileLookup::absolute(file)?;
            spelled.push((absolute.clone(), std::mem::replace(file, absolute)));
        }
        Ok(SpelledOperand(spelled))
    }

    /// The files a request names.
    fn operands(&mut self) -> Vec<&mut PathBuf> {
        fn show(query: &mut ShowQuery) -> Option<&mut PathBuf> {
            match query {
                ShowQuery::At { file, .. }
                | ShowQuery::Name {
                    file: Some(file), ..
                } => Some(file),
                ShowQuery::Name { file: None, .. } => None,
            }
        }
        match self {
            Self::References(
                ReferenceQuery::At { file, .. }
                | ReferenceQuery::Name {
                    file: Some(file), ..
                },
            )
            | Self::Outline { file } => vec![file],
            Self::References(ReferenceQuery::Name { file: None, .. }) => vec![],
            Self::Show(query) => show(query).into_iter().collect(),
            Self::Edit(request) => request
                .edits
                .iter_mut()
                .filter_map(|edit| show(&mut edit.target))
                .collect(),
        }
    }
}

/// A request's file operands as made absolute and as their caller spelled
/// them, before `ProjectRequest::make_absolute`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpelledOperand(Vec<(PathBuf, PathBuf)>);

impl SpelledOperand {
    /// Name an operand as spelled in a failure that names the request's
    /// file: `OutsideProject` and `InvalidPosition` (`QueryError::path`).
    /// Other failures name captured files by their canonical paths.
    pub fn restore(&self, error: &mut AnalysisError) {
        if let AnalysisError::Query(error) = error
            && matches!(
                error.kind,
                QueryErrorKind::OutsideProject | QueryErrorKind::InvalidPosition
            )
            && let Some((_, spelled)) = self
                .0
                .iter()
                .find(|(absolute, _)| error.path.as_ref() == Some(absolute))
        {
            error.path = Some(spelled.clone());
        }
    }
}

/// The result of a `ProjectRequest`, of the same operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectResponse {
    References(ReferenceResult),
    Outline(Outline),
    /// Boxed: a found declaration is far larger than the other responses.
    Show(Box<Show>),
    /// A selection, which `crate::editing::bind` prepares.
    Edit(Box<crate::editing::Selected>),
}
