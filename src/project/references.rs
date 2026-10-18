//! Reference queries on a project snapshot: symbol selection by position
//! or name, and result grouping. Both entry points select symbols and share
//! one grouping operation.

use super::Project;
use super::binding::{self, Index, OccurrenceTarget, SymbolId, roles};
use crate::analysis::{AnalysisError, QueryError, QueryErrorKind};
use crate::references::{
    Location, NamedLocation, Occurrence, ReferenceBaseline, ReferenceCoverage, ReferenceFilters,
    ReferenceGroup, ReferenceOperation, ReferenceOutcome, ReferenceQuery, ReferenceResult, Role,
    UnsearchedMembers,
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

/// The existing lexical question, also used by filtered lexical requests.
/// Keeping it explicit preserves legacy Name semantics when semantic member
/// selection is added at the new request boundary.
pub(super) fn lexical(
    project: &Project,
    query: ReferenceQuery,
    filters: ReferenceFilters,
) -> Result<ReferenceResult, AnalysisError> {
    let index = &project.bindings.index;
    let mut symbols = Vec::new();
    let mut unsearched_members = UnsearchedMembers::default();
    match query {
        ReferenceQuery::At { file, position } => {
            let id = project.searched_file(&file)?;
            let text = &project.sources.get(id).text;
            let offset = text
                .offset(position)
                .map_err(|error| QueryError::invalid_position(error, &file, Some(position)))?;
            if let Some(occurrence) = index.occurrence_at(id, offset) {
                let path = &project.sources.get(id).path;
                symbols
                    .extend_from_slice(selected(index, occurrence, || (path.clone(), position))?);
            }
        }
        ReferenceQuery::Name { name, file } => {
            let only = file
                .as_deref()
                .map(|file| project.searched_file(file))
                .transpose()?;
            // This is an advisory about catalog entries, independent of the
            // report filters and of whether the lexical question finds groups.
            if !name.contains('.') {
                unsearched_members = project.unsearched_members(&name, only);
            }
            for &candidate in index.candidates(&name) {
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
    let mut groups: Vec<ReferenceGroup> = symbols
        .into_iter()
        .map(|symbol| group(project, index, symbol, filters))
        .collect();
    groups.sort_by(|a, b| first_location(a).cmp(&first_location(b)));
    Ok(ReferenceResult {
        operation: ReferenceOperation::References,
        outcome: ReferenceOutcome::Complete,
        project: project.config.clone(),
        groups,
        filters,
        coverage: ReferenceCoverage {
            baseline: ReferenceBaseline::Lexical,
            status: ReferenceOutcome::Complete,
            gaps: Vec::new(),
        },
        unsearched_members,
    })
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

fn group(
    project: &Project,
    index: &Index,
    symbol: SymbolId,
    filters: ReferenceFilters,
) -> ReferenceGroup {
    let declarations: Vec<NamedLocation> = index
        .sites(symbol)
        .iter()
        .map(|site| NamedLocation {
            name: site.name.to_string(),
            location: location(project, site.file, site.span),
        })
        .collect();
    let occurrences = index
        .occurrences_of(symbol)
        .iter()
        .filter(|indexed| {
            (!filters.calls || indexed.call)
                && (!filters.exclude_tests
                    || !is_test_path(&project.sources.get(indexed.occurrence.file).display))
        })
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
pub(super) fn is_test_path(path: &str) -> bool {
    let mut segments = path.split('/').peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_some() {
            if matches!(segment, "test" | "tests" | "__tests__") {
                return true;
            }
        } else {
            return [".ts", ".tsx", ".mts", ".cts"].iter().any(|extension| {
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
