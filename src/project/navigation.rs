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
use crate::references::{MemberSuggestion, ReferenceAnchor, UnsearchedMembers};
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

impl NameSite<'_> {
    /// The same captured Q2 candidate used by navigation's own outcomes.
    pub(super) fn candidate(&self) -> crate::navigation::Candidate {
        self.index.candidate(self.site, self.display)
    }
}

/// What a name selects: one site, or the outcome of no match or several.
pub(super) enum Named<'p> {
    One(NameSite<'p>),
    Other(ShowOutcome),
}

/// What loading captures of one input for navigation.
#[derive(Debug, Clone)]
pub(super) enum Captured {
    /// A TypeScript file's selection index.
    Index(Arc<Index>),
    /// A JSON module, which is not navigation input.
    Json,
}

impl Captured {
    /// The index of a TypeScript input, parsed into `ast` from `source`.
    pub(super) fn typescript(ast: &Ast<'_>, source: &Arc<SourceText>) -> Self {
        Self::Index(Arc::new(Index::new(ast, Arc::clone(source))))
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
    /// Matching Q2 member declarations omitted by a lexical name query. Reads
    /// captured selection indexes only, never disk or the semantic evaluator.
    pub(super) fn unsearched_members(&self, name: &str, only: Option<FileId>) -> UnsearchedMembers {
        let mut selectors = Vec::new();
        for (id, captured) in self.sources.ids().zip(&self.navigation.files) {
            if only.is_some_and(|only| only != id) {
                continue;
            }
            let Some(Captured::Index(index)) = captured else {
                continue;
            };
            let display = &self.sources.get(id).display;
            selectors.extend(
                index
                    .member_candidates(name, display)
                    .into_iter()
                    .map(|candidate| MemberSuggestion {
                        file: candidate.file.clone(),
                        selector: candidate.selector,
                        anchor: ReferenceAnchor {
                            file: candidate.file,
                            position: candidate.anchor,
                        },
                        // A semantic request names it (`reference_search`).
                        search: None,
                    }),
            );
        }
        // Stable ordering keeps the earliest declaration anchor when overloads,
        // merged sites, or getter/setter syntax share one display selector.
        selectors.sort_by(|a, b| {
            (&a.file, &a.selector, a.anchor.position).cmp(&(
                &b.file,
                &b.selector,
                b.anchor.position,
            ))
        });
        selectors.dedup_by(|a, b| {
            a.file == b.file
                && a.selector == b.selector
                && (a.selector.is_some() || a.anchor.position == b.anchor.position)
        });
        UnsearchedMembers {
            count: selectors.len(),
            selectors,
        }
    }

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
                    let selection = catalog
                        .select(&selector)
                        .map_err(|error| navigation::invalid_position(error, &file, &selector))?;
                    Ok::<_, AnalysisError>(navigation::outcome(display, catalog, &selection))
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
    /// A name that misses `only` lists the specifiers of its complete
    /// catalog that spell it, which only projecting it again gives, as for
    /// a position; a failure to project it fails the request.
    fn show_name(&self, name: &str, only: Option<FileId>) -> Result<ShowOutcome, AnalysisError> {
        Ok(match self.name_sites(name, only)? {
            Named::Other(ShowOutcome::NotFound { .. }) if let Some(file) = only => {
                ShowOutcome::NotFound {
                    specifiers: self.project_file(file, |catalog| catalog.specifiers(name))?,
                }
            }
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
        let mut matches = self.declaration_name_sites(name, only)?;
        Ok(match matches.len() {
            0 => Named::Other(ShowOutcome::NotFound {
                specifiers: Vec::new(),
            }),
            1 => Named::One(matches.remove(0)),
            _ => Named::Other(ShowOutcome::Ambiguous(
                matches.iter().map(NameSite::candidate).collect(),
            )),
        })
    }

    /// Raw selector sites before semantic identity/meaning normalization.
    /// Navigation decides its own syntax-site ambiguity; semantic selection
    /// can resolve these same sites before deciding its different question.
    pub(super) fn declaration_name_sites(
        &self,
        name: &str,
        only: Option<FileId>,
    ) -> Result<Vec<NameSite<'_>>, AnalysisError> {
        let files: Vec<(FileId, &Index)> = match only {
            Some(id) => vec![(id, self.index(id)?)],
            None => self
                .sources
                .ids()
                .zip(&self.navigation.files)
                .filter_map(|(id, captured)| match captured {
                    Some(Captured::Index(index)) => Some((id, index.as_ref())),
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
        Ok(matches)
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

#[cfg(test)]
mod tests {
    use super::Captured;
    use crate::analysis::{AnalysisError, InputErrorKind};
    use crate::navigation::{Index, Kind, ShowOutcome, ShowQuery, Specifier};
    use crate::project::{Project, ProjectRequest, ProjectResponse};
    use crate::source::{SourcePosition, SourceText};
    use std::path::PathBuf;
    use std::sync::Arc;

    /// A temporary directory, removed on drop, a failed assertion included.
    struct Removed(PathBuf);

    impl Drop for Removed {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A name that misses FILE lists FILE's specifiers from its captured
    /// text projected again. When that projection fails, the request fails
    /// with that input failure, the one `--show --at` gives, and never
    /// answers `not_found`. The fault, captured text that no longer parses,
    /// is made after loading, since loading captures only text that parses.
    #[test]
    fn a_failed_projection_of_file_fails_a_missed_name() {
        let directory =
            std::env::temp_dir().join(format!("tsparse-reprojection-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let directory = std::fs::canonicalize(&directory).unwrap();
        let _remove = Removed(directory.clone());
        std::fs::write(
            directory.join("tsconfig.json"),
            r#"{ "compilerOptions": { "noLib": true } }"#,
        )
        .unwrap();
        std::fs::write(directory.join("lib.ts"), "export const a = 1;\n").unwrap();
        let app = directory.join("app.ts");
        std::fs::write(&app, "import { a } from \"./lib\";\nexport const b = a;\n").unwrap();
        let mut project = Project::load(&directory.join("tsconfig.json")).unwrap();
        let show = |project: &Project, query: ShowQuery| match project
            .execute(ProjectRequest::Show(query))
        {
            Ok(ProjectResponse::Show(show)) => Ok(show.outcome),
            Ok(other) => panic!("{other:?}"),
            Err(error) => Err(error),
        };
        let missed = || ShowQuery::Name {
            name: "a".into(),
            file: Some(app.clone()),
        };
        let at = SourcePosition {
            line: 1,
            column: 10,
        };
        assert_eq!(
            show(&project, missed()).unwrap(),
            ShowOutcome::NotFound {
                specifiers: vec![Specifier {
                    kind: Kind::Import,
                    anchor: at,
                }],
            }
        );

        let id = project.searched_file(&app).unwrap();
        project.navigation.files[id.index()] = Some(Captured::Index(Arc::new(
            Index::without_sites(Arc::new(SourceText::new("import { a } from;\n".into()))),
        )));
        let failure = show(&project, missed()).unwrap_err();
        let AnalysisError::Input(input) = &failure else {
            panic!("{failure:?}");
        };
        assert_eq!(input.kind, InputErrorKind::Syntax, "{input:?}");
        assert!(
            input
                .message
                .starts_with("captured text no longer parses: "),
            "{input:?}"
        );
        assert_eq!(input.path.as_deref(), Some(app.as_path()));
        // `--show --at` projects the file the same way and fails alike.
        assert_eq!(
            show(
                &project,
                ShowQuery::At {
                    file: app.clone(),
                    position: at,
                }
            )
            .unwrap_err(),
            failure
        );
        // A miss across the project names no file, so it projects none.
        assert_eq!(
            show(
                &project,
                ShowQuery::Name {
                    name: "absent".into(),
                    file: None,
                }
            )
            .unwrap(),
            ShowOutcome::NotFound {
                specifiers: Vec::new()
            }
        );
    }
}
