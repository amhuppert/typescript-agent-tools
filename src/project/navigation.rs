//! Navigation queries on a project snapshot: outlines and selections over
//! the searched files. Loading keeps a compact selection index of each
//! searched TypeScript file, built while its syntax tree is alive. A name
//! search, across the files or narrowed to one, reads only the indexes,
//! which describe a missing name or several matches; a single match, an
//! outline, and a position project the selected file's captured text again,
//! with the shared parsing capability, into its complete catalog. Queries
//! read no files. See `docs/navigation-performance.md` for why the snapshot
//! keeps indexes rather than complete catalogs.

use super::Project;
use super::sources::FileId;
use crate::Ast;
use crate::analysis::{AnalysisError, InputError, InputErrorKind};
use crate::input::InputKind;
use crate::navigation::{self, Catalog, Index, Outline, Selector, Show, ShowOutcome, ShowQuery};
use crate::source::SourceText;
use std::path::Path;
use std::sync::Arc;

/// One site a name selects in a searched file.
pub(super) struct NameSite<'p> {
    pub(super) display: &'p str,
    pub(super) file: FileId,
    index: &'p Index,
    site: usize,
}

/// What a name selects: one site, or the outcome of no match or several.
pub(super) enum Named<'p> {
    One(NameSite<'p>),
    Other(ShowOutcome),
}

/// What loading captures of one input for navigation.
#[derive(Debug)]
pub(super) enum Captured {
    /// A TypeScript file's selection index.
    Index(Index),
    /// A JSON module, which is not navigation input.
    Json,
}

impl Captured {
    /// The index of a TypeScript input, parsed into `ast` from `source`.
    pub(super) fn typescript(ast: &Ast<'_>, source: &Arc<SourceText>) -> Self {
        Self::Index(Index::new(ast, Arc::clone(source)))
    }
}

/// The navigation state of one generation: what loading captured of each
/// searched file, by `FileId`, and nothing for other inputs.
#[derive(Debug)]
pub(super) struct Navigation {
    files: Vec<Option<Captured>>,
}

impl Navigation {
    /// From each input's capture, by `FileId`, which loading keeps only for
    /// searched files.
    pub(super) fn new(files: Vec<Option<Captured>>) -> Self {
        Self { files }
    }
}

impl Project {
    /// The complete declaration outline of one searched file. Relative paths
    /// use the current working directory and are matched lexically; a path
    /// that is not Unicode names no file (`FileLookup::find`).
    pub fn outline(&self, file: &Path) -> Result<Outline, AnalysisError> {
        let id = self.searched_file(file)?;
        let entries = self.project_file(id, |catalog| catalog.declarations())?;
        Ok(Outline {
            project: Some(self.config.clone()),
            file: self.sources.get(id).display.clone(),
            entries,
        })
    }

    /// The declaration a query selects among the searched files, with its
    /// exact source, or every candidate when several match, ordered by file
    /// and then position. Relative paths use the current working directory
    /// and are matched lexically; a path that is not Unicode names no file
    /// (`FileLookup::find`).
    pub fn show(&self, query: ShowQuery) -> Result<Show, AnalysisError> {
        let (selector, scope, outcome) = match query {
            ShowQuery::Name { name, file } => {
                let only = file
                    .as_deref()
                    .map(|file| self.searched_file(file))
                    .transpose()?;
                let outcome = self.show_name(&name, only)?;
                let scope = only.map(|id| self.sources.get(id).display.clone());
                (Selector::Name(name), scope, outcome)
            }
            ShowQuery::At { file, position } => {
                let id = self.searched_file(&file)?;
                let display = self.sources.get(id).display.as_str();
                let selector = Selector::At(position);
                let outcome = self.project_file(id, |catalog| {
                    let records = catalog
                        .select(&selector)
                        .map_err(|error| navigation::invalid_position(error, &file, &selector))?;
                    Ok::<_, AnalysisError>(navigation::outcome(
                        records.into_iter().map(|record| (display, catalog, record)),
                    ))
                })??;
                (selector, Some(display.to_owned()), outcome)
            }
        };
        Ok(Show {
            project: Some(self.config.clone()),
            file: scope,
            query: selector,
            outcome,
        })
    }

    /// A name's matches in the searched files, or only in `only`: found by
    /// the indexes alone, which describe several matches as candidates, and
    /// the one match's file projected again for its source. Every name
    /// selection, in a project or a single file, follows the index's rule.
    fn show_name(&self, name: &str, only: Option<FileId>) -> Result<ShowOutcome, AnalysisError> {
        Ok(match self.name_sites(name, only)? {
            Named::One(NameSite {
                display,
                file,
                index,
                site,
            }) => {
                let record = index.record(site);
                self.project_file(file, |catalog| {
                    debug_assert_eq!(
                        catalog.candidate(record, display),
                        index.candidate(site, display),
                        "an index and the catalog projected again from its text agree"
                    );
                    ShowOutcome::Found(catalog.found(record, display))
                })?
            }
            Named::Other(outcome) => outcome,
        })
    }

    /// The one site a name selects in the searched files, or only in
    /// `only`, by the indexes alone; or the outcome of no match or several.
    pub(super) fn name_sites(
        &self,
        name: &str,
        only: Option<FileId>,
    ) -> Result<Named<'_>, AnalysisError> {
        let files: Vec<(FileId, &Index)> = match only {
            Some(id) => vec![(id, self.index(id)?)],
            None => self
                .sources
                .ids()
                .zip(&self.navigation.files)
                .filter_map(|(id, captured)| match captured {
                    Some(Captured::Index(index)) => Some((id, index)),
                    _ => None,
                })
                .collect(),
        };
        let mut matches = Vec::new();
        for (id, index) in files {
            let display = self.sources.get(id).display.as_str();
            let sites = index.select_name(name);
            matches.extend(sites.into_iter().map(|site| NameSite {
                display,
                file: id,
                index,
                site,
            }));
        }
        // Stable, so each file's matches stay in source order.
        matches.sort_by(|a, b| a.display.cmp(b.display));
        Ok(match matches.len() {
            0 => Named::Other(ShowOutcome::NotFound),
            1 => Named::One(matches.remove(0)),
            _ => Named::Other(ShowOutcome::Ambiguous(
                matches
                    .iter()
                    .map(|matched| matched.index.candidate(matched.site, matched.display))
                    .collect(),
            )),
        })
    }

    /// The captured index of a searched file, which is navigation input
    /// unless it is a JSON module.
    fn index(&self, id: FileId) -> Result<&Index, AnalysisError> {
        match &self.navigation.files[id.index()] {
            Some(Captured::Index(index)) => Ok(index),
            Some(Captured::Json) => Err(navigation::not_navigation_input(
                InputKind::Json,
                &self.sources.get(id).path,
            )
            .into()),
            None => unreachable!("loading captures every searched file"),
        }
    }

    /// Answer from the complete catalog of a searched file: its index's
    /// text projected again, on the parsing thread. Failures name the
    /// captured file by its canonical path, as other input failures do,
    /// whichever path the request named it by, so a request answers alike
    /// from any working directory and through the service.
    fn project_file<T: Send>(
        &self,
        id: FileId,
        answer: impl FnOnce(&Catalog) -> T + Send,
    ) -> Result<T, AnalysisError> {
        let path = &self.sources.get(id).path;
        let index = self.index(id)?;
        let name = path.to_str().expect("loading names its inputs with text");
        let failure = |kind, message: String, position| {
            AnalysisError::Input(InputError {
                kind,
                message,
                path: Some(path.clone()),
                position,
            })
        };
        crate::analysis::on_parsing_thread("tsparse-navigation", |parser| {
            let catalog = index.catalog(parser, name).map_err(|error| {
                // Loading parsed the same text with the same capability.
                let mut error = InputError::unparsable(error, index.source(), path);
                error.message = format!("captured text no longer parses: {}", error.message);
                AnalysisError::Input(error)
            })?;
            Ok(answer(&catalog))
        })
        .unwrap_or_else(|error| {
            Err(failure(
                InputErrorKind::Io,
                format!("cannot start the parsing thread: {error}"),
                None,
            ))
        })
    }
}
