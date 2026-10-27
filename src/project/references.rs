//! Reference queries on a project snapshot: symbol selection by position
//! or name, and result grouping. Both entry points select symbols and share
//! one grouping operation.

use super::Project;
use super::binding::{self, Index, OccurrenceTarget, SymbolId, roles};
use crate::analysis::{AnalysisError, QueryError, QueryErrorKind};
use crate::references::{
    DefinitionGroup, Location, NamedLocation, Occurrence, ReferenceBaseline, ReferenceCoverage,
    ReferenceFilters, ReferenceGroup, ReferenceOperation, ReferenceOutcome, ReferenceQuery,
    ReferenceResult, Role, UnsearchedMembers,
};
use crate::source::SourcePosition;
use std::path::PathBuf;

/// Actual symbols from the existing lexical operation, used for canonical
/// dotted-selector collapse without interpreting its public declaration groups.
pub(super) fn lexical_symbols_at(
    project: &Project,
    file: super::sources::FileId,
    offset: usize,
) -> Result<Vec<SymbolId>, AnalysisError> {
    let Some(occurrence) = project.bindings.index.occurrence_at(file, offset) else {
        return Ok(Vec::new());
    };
    let source = project.sources.get(file);
    Ok(selected(&project.bindings.index, occurrence, || {
        (source.path.clone(), source.text.position(offset))
    })?
    .to_vec())
}

/// Existing qualified namespace resolution is a lexical question even though
/// its final segment has PropertyAccess syntax. An unresolved/member token is
/// not inferred to have a lexical symbol from its spelling.
pub(super) fn has_lexical_target(
    project: &Project,
    file: super::sources::FileId,
    offset: usize,
) -> bool {
    project
        .bindings
        .index
        .occurrence_at(file, offset)
        .is_some_and(|occurrence| matches!(occurrence.target, OccurrenceTarget::Symbols(_)))
}

/// The selection a result echoes: `query` with its file named as
/// `Location::file` names the searched file it selects, relative to the
/// configuration's directory with `/` separators, so the echo is the same
/// from any working directory and from the service. The one owner of that
/// form; a file that names no searched file fails as selecting would.
pub(super) fn selection(
    project: &Project,
    query: &ReferenceQuery,
) -> Result<ReferenceQuery, AnalysisError> {
    let echo = |file: &std::path::Path| echoed_file(project, file);
    Ok(match query {
        ReferenceQuery::At { file, position } => ReferenceQuery::At {
            file: echo(file)?,
            position: *position,
        },
        ReferenceQuery::Name { name, file } => ReferenceQuery::Name {
            name: name.clone(),
            file: file.as_deref().map(echo).transpose()?,
        },
    })
}

/// A request's file as its result echoes it: the searched file it names,
/// relative to the configuration's directory with `/` separators.
pub(super) fn echoed_file(
    project: &Project,
    file: &std::path::Path,
) -> Result<PathBuf, AnalysisError> {
    let id = project.searched_file(file)?;
    Ok(PathBuf::from(&project.sources.get(id).display))
}

/// A request's reporting filters, with `--file`'s path resolved to the
/// searched file it names through the file-identity owner
/// (`FileLookup::find`, which applies the project's case policy and
/// separators). The one owner of which occurrences a result reports, for
/// lexical and member searches alike.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Filters {
    calls: bool,
    exclude_tests: bool,
    file: Option<super::sources::FileId>,
}

impl Filters {
    /// `filters` resolved against `project`: a `file` that is no searched
    /// file fails as a FILE operand does (`OutsideProject`).
    pub(super) fn resolve(
        project: &Project,
        filters: &ReferenceFilters,
    ) -> Result<Self, AnalysisError> {
        Ok(Self {
            calls: filters.calls,
            exclude_tests: filters.exclude_tests,
            file: filters
                .file
                .as_deref()
                .map(|file| project.searched_file(file))
                .transpose()?,
        })
    }

    /// Whether an occurrence in `file`, a call target when `call`, is
    /// reported.
    pub(super) fn allows(
        &self,
        project: &Project,
        file: super::sources::FileId,
        call: bool,
    ) -> bool {
        (!self.calls || call)
            && (!self.exclude_tests || !is_test_path(&project.sources.get(file).display))
            && self.file.is_none_or(|only| only == file)
    }

    /// Whether any filter may remove an occurrence.
    pub(super) fn any(&self) -> bool {
        self.calls || self.exclude_tests || self.file.is_some()
    }

    /// The filters as a result echoes them, `file` named as
    /// `Location::file` names it.
    pub(super) fn echo(&self, project: &Project) -> ReferenceFilters {
        ReferenceFilters {
            calls: self.calls,
            exclude_tests: self.exclude_tests,
            file: self
                .file
                .map(|file| PathBuf::from(&project.sources.get(file).display)),
        }
    }
}

/// The existing lexical question, also used by filtered lexical requests.
/// Keeping it explicit preserves legacy Name semantics when semantic member
/// selection is added at the new request boundary. `selection` is what the
/// result echoes (`selection`), which differs from `query` where a member
/// selector resolved to a lexical seed.
pub(super) fn lexical(
    project: &Project,
    query: ReferenceQuery,
    filters: Filters,
    selection: ReferenceQuery,
) -> Result<ReferenceResult, AnalysisError> {
    let index = &project.bindings.index;
    let symbols = lexical_symbols(project, &query)?;
    // This is an advisory about catalog entries, independent of the
    // report filters and of whether the lexical question finds groups.
    let unsearched_members = match &query {
        ReferenceQuery::Name { name, file } if !name.contains('.') => {
            let only = file
                .as_deref()
                .map(|file| project.searched_file(file))
                .transpose()?;
            project.unsearched_members(name, only)
        }
        ReferenceQuery::Name { .. } | ReferenceQuery::At { .. } => UnsearchedMembers::default(),
    };
    let mut groups: Vec<ReferenceGroup> = symbols
        .into_iter()
        .map(|symbol| group(project, index, symbol, filters))
        .collect();
    groups.sort_by(|a, b| first_location(a).cmp(&first_location(b)));
    Ok(ReferenceResult {
        operation: ReferenceOperation::References,
        outcome: ReferenceOutcome::Complete,
        project: project.config.clone(),
        selection,
        groups,
        filters: filters.echo(project),
        coverage: ReferenceCoverage {
            baseline: ReferenceBaseline::Lexical,
            status: ReferenceOutcome::Complete,
            gaps: Vec::new(),
        },
        unsearched_members,
    })
}

/// The symbols a lexical selection denotes, sorted and deduplicated: those
/// of the identifier token containing a position, or of every declaration,
/// local import, and exported alias spelled exactly a name, which FILE
/// narrows to its own. A position in no identifier, or a name with no
/// candidate, selects none. The one owner of lexical symbol selection, for
/// reference searches and definitions alike.
pub(super) fn lexical_symbols(
    project: &Project,
    query: &ReferenceQuery,
) -> Result<Vec<SymbolId>, AnalysisError> {
    let index = &project.bindings.index;
    let mut symbols = Vec::new();
    match query {
        ReferenceQuery::At { file, position } => {
            let id = project.searched_file(file)?;
            let text = &project.sources.get(id).text;
            let offset = text
                .offset(*position)
                .map_err(|error| QueryError::invalid_position(error, file, Some(*position)))?;
            if let Some(occurrence) = index.occurrence_at(id, offset) {
                let path = &project.sources.get(id).path;
                symbols
                    .extend_from_slice(selected(index, occurrence, || (path.clone(), *position))?);
            }
        }
        ReferenceQuery::Name { name, file } => {
            let only = file
                .as_deref()
                .map(|file| project.searched_file(file))
                .transpose()?;
            for &candidate in index.candidates(name) {
                if only.is_some_and(|only| only != candidate.file) {
                    continue;
                }
                let occurrence = index.occurrence(candidate);
                let source = project.sources.get(candidate.file);
                symbols.extend_from_slice(selected(index, occurrence, || {
                    (
                        source.path.clone(),
                        source.text.position(occurrence.span.start),
                    )
                })?);
            }
        }
    }
    symbols.sort();
    symbols.dedup();
    Ok(symbols)
}

/// The declarations of `symbols`, one group per symbol, ordered by their
/// first declarations as reference groups are: each group lists exactly
/// the declarations the symbol's reference group does (`declarations`).
pub(super) fn definition_groups(project: &Project, symbols: Vec<SymbolId>) -> Vec<DefinitionGroup> {
    let index = &project.bindings.index;
    let mut groups: Vec<DefinitionGroup> = symbols
        .into_iter()
        .map(|symbol| DefinitionGroup {
            declarations: declarations(project, index, symbol),
        })
        .collect();
    fn first(group: &DefinitionGroup) -> Option<&Location> {
        group
            .declarations
            .first()
            .map(|declaration| &declaration.location)
    }
    groups.sort_by(|a, b| first(a).cmp(&first(b)));
    groups
}

fn first_location(group: &ReferenceGroup) -> Option<&Location> {
    group
        .declarations
        .first()
        .map(|declaration| &declaration.location)
        .or_else(|| group.occurrences.first().map(|o| &o.location))
}

/// The symbols a selected token denotes. Both entry points fail on a token
/// that is not a supported target or does not resolve, rather than letting
/// it vanish from an otherwise successful result.
fn selected<'a>(
    index: &Index,
    occurrence: binding::Occurrence<'a>,
    at: impl FnOnce() -> (PathBuf, SourcePosition),
) -> Result<&'a [SymbolId], QueryError> {
    let (kind, message) = match occurrence.target {
        OccurrenceTarget::Symbols(found) => return Ok(found),
        OccurrenceTarget::Unsearched => {
            unreachable!("dependency tokens never enter lexical selectors")
        }
        OccurrenceTarget::NotReference(why) => (
            QueryErrorKind::UnsupportedTarget,
            format!(
                "`{}` is not a supported reference target ({why})",
                index.text(occurrence.name)
            ),
        ),
        OccurrenceTarget::Unresolved(why) => (
            QueryErrorKind::UnresolvedTarget,
            format!("`{}`: {}", index.text(occurrence.name), why),
        ),
    };
    let (path, position) = at();
    Err(QueryError {
        kind,
        message,
        path: Some(path),
        position: Some(position),
    })
}

/// A symbol's declarations: its sites (`Index::sites`), in their order,
/// at public locations.
fn declarations(project: &Project, index: &Index, symbol: SymbolId) -> Vec<NamedLocation> {
    index
        .sites(symbol)
        .iter()
        .map(|site| NamedLocation {
            name: site.name.to_string(),
            location: location(project, site.file, site.span),
        })
        .collect()
}

fn group(project: &Project, index: &Index, symbol: SymbolId, filters: Filters) -> ReferenceGroup {
    let declarations = declarations(project, index, symbol);
    let occurrences = index
        .occurrences_of(symbol)
        .iter()
        .filter(|indexed| filters.allows(project, indexed.occurrence.file, indexed.call))
        .map(|indexed| {
            let occurrence = index.occurrence(indexed.occurrence);
            Occurrence {
                name: index.text(occurrence.name).to_string(),
                location: location(project, indexed.occurrence.file, occurrence.span),
                roles: role_list(indexed.roles).collect(),
            }
        })
        .collect();
    ReferenceGroup {
        declarations,
        occurrences,
    }
}

/// Product policy for `--no-tests`, applied to exact normalized project-relative
/// paths, independently of TypeScript configuration glob matching.
fn is_test_path(path: &str) -> bool {
    let mut segments = path.split('/').peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_some() {
            if matches!(segment, "test" | "tests" | "__tests__") {
                return true;
            }
        } else {
            return [".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"]
                .iter()
                .any(|extension| {
                    segment
                        .strip_suffix(extension)
                        .is_some_and(|base| base.ends_with(".test") || base.ends_with(".spec"))
                });
        }
    }
    false
}

pub(super) fn role_list(bits: u8) -> impl Iterator<Item = Role> {
    [
        (roles::DECLARATION, Role::Declaration),
        (roles::USE, Role::Use),
        (roles::IMPORT, Role::Import),
        (roles::EXPORT, Role::Export),
    ]
    .into_iter()
    .filter(move |(bit, _)| bits & bit != 0)
    .map(|(_, role)| role)
}

/// The public location of `span` in `file`; the index keeps offsets, and a
/// query converts only the locations it returns.
fn location(project: &Project, file: super::sources::FileId, span: crate::Span) -> Location {
    let source = project.sources.get(file);
    Location {
        file: source.display.clone(),
        start: source.text.position(span.start),
        end: source.text.position(span.end),
    }
}
