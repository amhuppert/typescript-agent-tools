//! Reference queries on a project snapshot: symbol selection by position
//! or name, and result grouping. Both entry points select symbols and share
//! one grouping operation.

use super::Project;
use super::binding::{self, Index, SymbolId, Target, roles};
use crate::analysis::{AnalysisError, QueryError, QueryErrorKind};
use crate::references::{
    Location, NamedLocation, Occurrence, ReferenceGroup, ReferenceQuery, Role,
};
use crate::source::SourcePosition;
use std::path::PathBuf;

pub(super) fn find(
    project: &Project,
    query: ReferenceQuery,
) -> Result<Vec<ReferenceGroup>, AnalysisError> {
    let index = &project.index;
    let mut symbols = Vec::new();
    match query {
        ReferenceQuery::At { file, position } => {
            let id = project.searched_file(&file)?;
            let text = &project.sources.get(id).text;
            let offset = text
                .offset(position)
                .map_err(|error| QueryError::invalid_position(error, &file, Some(position)))?;
            let Some(occurrence) = index.occurrence_at(id, offset) else {
                return Ok(Vec::new());
            };
            let path = &project.sources.get(id).path;
            symbols.extend_from_slice(selected(index, occurrence, || (path.clone(), position))?);
        }
        ReferenceQuery::Name { name, file } => {
            let only = file
                .as_deref()
                .map(|file| project.searched_file(file))
                .transpose()?;
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
        .map(|symbol| group(project, index, symbol))
        .collect();
    groups.sort_by(|a, b| first_location(a).cmp(&first_location(b)));
    Ok(groups)
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
    occurrence: &'a binding::Occurrence,
    at: impl FnOnce() -> (PathBuf, SourcePosition),
) -> Result<&'a [SymbolId], QueryError> {
    let (kind, message) = match &occurrence.target {
        Target::Symbols(found) => return Ok(found),
        Target::NotReference(why) => (
            QueryErrorKind::UnsupportedTarget,
            format!(
                "`{}` is not a supported reference target ({why})",
                index.text(occurrence.name)
            ),
        ),
        Target::Unresolved(why) => (
            QueryErrorKind::UnresolvedTarget,
            format!("`{}`: {}", index.text(occurrence.name), index.text(*why)),
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

fn group(project: &Project, index: &Index, symbol: SymbolId) -> ReferenceGroup {
    let declarations: Vec<NamedLocation> = index
        .sites(symbol)
        .iter()
        .map(|site| NamedLocation {
            name: site.name.clone(),
            location: location(project, site.location),
        })
        .collect();
    let occurrences = index
        .occurrences_of(symbol)
        .iter()
        .map(|occurrence| Occurrence {
            name: index.text(occurrence.name).to_string(),
            location: location(project, occurrence.location),
            roles: role_list(occurrence.roles).collect(),
        })
        .collect();
    ReferenceGroup {
        declarations,
        occurrences,
    }
}

fn role_list(bits: u8) -> impl Iterator<Item = Role> {
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

fn location(project: &Project, location: binding::IndexedLocation) -> Location {
    let source = project.sources.get(location.file);
    Location {
        file: source.display.clone(),
        start: location.start,
        end: location.end,
    }
}
