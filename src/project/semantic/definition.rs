//! Go to definition: the declarations the reference selection selects. The
//! reference selection owner decides what a position or name selects
//! (`selection::references::select`), and the lexical selection the symbols
//! it denotes (`references::lexical_symbols`); this reads their
//! declarations from the index (`references::definition_groups`), so a
//! definition follows import and export aliases exactly as a reference
//! search's groups do.

use super::evaluation::Evaluator;
use super::selection::references::{ReferenceSelected, ReferenceSelection, select};
use crate::analysis::{AnalysisError, QueryError, QueryErrorKind};
use crate::project::{Project, references};
use crate::references::{
    DefinitionAnswer, DefinitionOperation, DefinitionOutcome, DefinitionResult,
    DefinitionSelection, DefinitionSelectionFailure, ReferenceQuery,
};

/// The declarations `query` selects. A position the reference selection
/// sends to its member path answers `member`, once that member is selected,
/// since a definition does not follow members; a position in no identifier,
/// or a name with no candidate, answers `not_found`. A dotted name, which
/// selects a member by containment, is refused.
pub(super) fn definition(
    project: &Project,
    query: &ReferenceQuery,
    evaluator: &mut Evaluator<'_, '_>,
) -> Result<DefinitionAnswer, AnalysisError> {
    if let ReferenceQuery::Name { name, .. } = query
        && name.contains('.')
    {
        return Err(QueryError {
            kind: QueryErrorKind::InvalidRequest,
            message: format!(
                "a definition name is one identifier, not {name}: a dotted name selects a member by containment, which a definition does not follow"
            ),
            path: None,
            position: None,
        }
        .into());
    }
    let selected = select(project, evaluator, query)?;
    let selection = references::selection(project, query)?;
    let unselected = |outcome| {
        Ok(DefinitionAnswer::Unselected(DefinitionSelection {
            operation: DefinitionOperation::Definition,
            project: project.config.clone(),
            selection: selection.clone(),
            outcome,
        }))
    };
    let symbols = match selected {
        ReferenceSelection::One(ReferenceSelected::Lexical(query)) => {
            references::lexical_symbols(project, &query)?
        }
        ReferenceSelection::One(ReferenceSelected::Member { site, .. }) => {
            return unselected(DefinitionSelectionFailure::Member { name: site.key });
        }
        // Only a dotted name selects by catalog, and none reaches here.
        ReferenceSelection::Absent | ReferenceSelection::Ambiguous(_) => {
            return Err(super::internal(
                "a position or bare name selected by catalog declaration",
            ));
        }
    };
    if symbols.is_empty() {
        return unselected(DefinitionSelectionFailure::NotFound);
    }
    Ok(DefinitionAnswer::Selected(DefinitionResult {
        operation: DefinitionOperation::Definition,
        outcome: DefinitionOutcome::Complete,
        project: project.config.clone(),
        selection,
        groups: references::definition_groups(project, symbols),
    }))
}
