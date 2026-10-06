//! Symbol selection by position or name, and result grouping. Both entry
//! points select symbols and share one grouping operation.

use super::binding::{self, Index, SymbolId, Target, roles};
use super::source::{FileId, PositionError};
use super::{
    Location, NamedLocation, Occurrence, QueryError, QueryErrorKind, ReferenceGroup,
    ReferenceProject, ReferenceQuery, Role, SourcePosition,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) fn find(
    project: &ReferenceProject,
    query: ReferenceQuery,
) -> Result<Vec<ReferenceGroup>, QueryError> {
    let index = &project.index;
    let mut symbols = Vec::new();
    match query {
        ReferenceQuery::At { file, position } => {
            let id = search_file(project, &file)?;
            let text = &project.sources.get(id).text;
            let offset = text.offset(position).map_err(|error| QueryError {
                kind: QueryErrorKind::InvalidPosition,
                message: match error {
                    PositionError::LineOutOfRange => "line is outside the file",
                    PositionError::ColumnOutOfRange => "column is outside the line",
                    PositionError::InsideSurrogatePair => "column is inside a surrogate pair",
                }
                .into(),
                path: Some(file.clone()),
                position: Some(position),
            })?;
            let Some(occurrence) = index.occurrence_at(id, offset) else {
                return Ok(Vec::new());
            };
            let path = &project.sources.get(id).path;
            symbols.extend_from_slice(selected(index, occurrence, || (path.clone(), position))?);
        }
        ReferenceQuery::Name { name, file } => {
            let only = file
                .as_deref()
                .map(|file| search_file(project, file))
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
    groups.sort_by(|a, b| {
        let first = |group: &ReferenceGroup| {
            group
                .declarations
                .first()
                .map(|declaration| declaration.location.clone())
                .or_else(|| group.occurrences.first().map(|o| o.location.clone()))
        };
        first(a).cmp(&first(b))
    });
    Ok(groups)
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

fn group(project: &ReferenceProject, index: &Index, symbol: SymbolId) -> ReferenceGroup {
    let mut declarations: Vec<NamedLocation> = index
        .sites(symbol)
        .iter()
        .map(|site| NamedLocation {
            name: site.name.clone(),
            location: location(project, site.file, site.span),
        })
        .collect();
    declarations.sort_by(|a, b| a.location.cmp(&b.location));
    declarations.dedup();
    let mut occurrences: BTreeMap<Location, Occurrence> = BTreeMap::new();
    for &reference in index.occurrences_of(symbol) {
        let occurrence = index.occurrence(reference);
        let location = location(project, reference.file, occurrence.span);
        let entry = occurrences
            .entry(location.clone())
            .or_insert_with(|| Occurrence {
                name: index.text(occurrence.name).to_string(),
                location,
                roles: Vec::new(),
            });
        for role in role_list(occurrence.roles) {
            if !entry.roles.contains(&role) {
                entry.roles.push(role);
            }
        }
    }
    let mut occurrences: Vec<Occurrence> = occurrences.into_values().collect();
    for occurrence in &mut occurrences {
        occurrence.roles.sort();
    }
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

fn location(project: &ReferenceProject, file: FileId, span: crate::Span) -> Location {
    let source = project.sources.get(file);
    Location {
        file: source.display.clone(),
        start: source.text.position(span.start),
        end: source.text.position(span.end),
    }
}

/// The searched file a query names, matched lexically without file access.
fn search_file(project: &ReferenceProject, file: &Path) -> Result<FileId, QueryError> {
    project.files.find(file).ok_or_else(|| QueryError {
        kind: QueryErrorKind::OutsideProject,
        message: "not one of the project's searched files".into(),
        path: Some(file.to_path_buf()),
        position: None,
    })
}
