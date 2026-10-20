//! Q3 selects a reference operation and proves replay using the same token
//! path. Q2 contributes declaration sites, never semantic identity or roles.

use super::super::evaluation::{Evaluator, ReferenceSymbol};
use super::super::source::ReferenceSite;
use crate::analysis::{AnalysisError, QueryError, QueryErrorKind};
use crate::project::binding::{
    NativeSymbolIdentity, ReferenceTokenFacts, ReferenceTokenRole, ReferenceTokenSource, SymbolId,
};
use crate::project::sources::FileId;
use crate::project::{Project, references};
use crate::references::{ReferenceBaseline, ReferenceCandidate, ReferenceQuery};
use std::collections::HashSet;

pub(in crate::project::semantic) enum ReferenceSelection {
    One(ReferenceSelected),
    Absent,
    Ambiguous(Vec<ReferenceCandidate>),
}

pub(in crate::project::semantic) enum ReferenceSelected {
    Lexical(ReferenceQuery),
    Member {
        site: ReferenceSite,
        symbol: ReferenceSymbol,
    },
}

#[derive(PartialEq, Eq, Hash)]
enum Identity {
    Lexical(Vec<SymbolId>),
    Member(NativeSymbolIdentity),
}

pub(in crate::project::semantic) fn select(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    query: &ReferenceQuery,
) -> Result<ReferenceSelection, AnalysisError> {
    match query {
        ReferenceQuery::Name { name, .. } if !name.contains('.') => Ok(ReferenceSelection::One(
            ReferenceSelected::Lexical(query.clone()),
        )),
        ReferenceQuery::At { file, position } => {
            let id = project.searched_file(file)?;
            let offset = project
                .sources
                .get(id)
                .text
                .offset(*position)
                .map_err(|error| QueryError::invalid_position(error, file, Some(*position)))?;
            Ok(ReferenceSelection::One(at(project, evaluator, id, offset)?))
        }
        ReferenceQuery::Name { name, file } => {
            let only = file
                .as_deref()
                .map(|file| project.searched_file(file))
                .transpose()?;
            let mut selected = Vec::new();
            let mut identities = HashSet::new();
            for declaration in project.declaration_name_sites(name, only)? {
                let candidate = declaration.candidate();
                let anchor = replay_offset(project, evaluator, declaration.file, candidate.anchor)?
                    .ok_or_else(|| QueryError {
                        kind: QueryErrorKind::UnsupportedTarget,
                        message: "the selected declaration has no reference token role".into(),
                        path: Some(project.sources.get(declaration.file).path.clone()),
                        position: Some(candidate.anchor),
                    })?;
                let seed = at(project, evaluator, declaration.file, anchor)?;
                let identity = identity(project, evaluator, &seed)?;
                if identities.insert(identity) {
                    selected.push((seed, candidate.selector));
                }
            }
            match selected.len() {
                0 => Ok(ReferenceSelection::Absent),
                1 => Ok(ReferenceSelection::One(selected.remove(0).0)),
                _ => {
                    let mut candidates = Vec::new();
                    for (seed, selector) in selected {
                        candidates.push(candidate(project, evaluator, &seed, selector)?);
                    }
                    Ok(ReferenceSelection::Ambiguous(candidates))
                }
            }
        }
    }
}

/// The offset whose position selection (`at`) searches the catalog
/// declaration anchored at `anchor` in `file`, as a selector naming it does,
/// or `None` when its name has no reference token. The one owner of that
/// mapping, for dotted selectors and for the members a bare name leaves
/// unsearched, which it answers for every suggested member. The token
/// inventory recorded the answer for those when it captured them: a
/// member's name selects itself, and a parameter property's name selects
/// the modifier token captured for it (`parameter_property_reference_anchor`),
/// so they need no syntax capture per request. Any other declaration, such
/// as a namespace, whose record denotes its innermost name, is answered from
/// its syntax (`reference_selector_anchor`).
pub(in crate::project::semantic) fn replay_offset(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    file: FileId,
    anchor: crate::source::SourcePosition,
) -> Result<Option<usize>, AnalysisError> {
    let offset = project.sources.get(file).text.offset(anchor).map_err(|_| {
        super::super::internal("captured reference selector anchor no longer addresses its source")
    })?;
    let Some(token) = project.bindings.reference_token_at(file, offset) else {
        return Ok(None);
    };
    let facts = project.bindings.reference_token_facts(token);
    if let ReferenceTokenSource::Syntax(syntax) = facts.source {
        match syntax.role {
            ReferenceTokenRole::MemberName => return Ok(Some(facts.selection_span.start)),
            ReferenceTokenRole::ParameterName => {
                let control = evaluator.checker_inputs().control;
                if let Some(modifier) = captured_modifier(project, control, &facts) {
                    return Ok(Some(modifier));
                }
            }
            _ => {}
        }
    }
    Ok(Some(evaluator.source().reference_selector_anchor(facts)?))
}

/// The modifier token the inventory captured for the parameter property
/// whose name is `name`: keyed by that name, in its file, with that name as
/// its occurrence. `None` leaves the question to the property's syntax.
fn captured_modifier(
    project: &Project,
    control: Option<&crate::resources::Control>,
    name: &ReferenceTokenFacts<'_>,
) -> Option<usize> {
    let Ok(key) = name.key else {
        return None;
    };
    project
        .bindings
        .reference_tokens(key, control)
        .iter()
        .map(|token| project.bindings.reference_token_facts(token))
        .find(|modifier| {
            modifier.file == name.file
                && matches!(
                    modifier.source,
                    ReferenceTokenSource::Syntax(syntax)
                        if syntax.role == ReferenceTokenRole::ParameterPropertyModifier
                            && syntax.occurrence_span == name.selection_span
                )
        })
        .map(|modifier| modifier.selection_span.start)
}

fn at(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    file: FileId,
    offset: usize,
) -> Result<ReferenceSelected, AnalysisError> {
    let input = project.sources.get(file);
    let lexical = || {
        ReferenceSelected::Lexical(ReferenceQuery::At {
            file: input.path.clone(),
            position: input.text.position(offset),
        })
    };
    let Some(token) = project.bindings.reference_token_at(file, offset) else {
        // Preserve the lexical operation's empty result for trivia positions.
        return Ok(lexical());
    };
    let site = evaluator
        .source()
        .reference_site(project.bindings.reference_token_facts(token))?;
    if references::has_lexical_target(project, file, offset)
        || !evaluator.source().reference_is_member_site(&site)?
    {
        return Ok(lexical());
    }
    let symbol = evaluator
        .reference_symbol_at_site(&site)?
        .ok_or_else(|| QueryError {
            kind: QueryErrorKind::UnresolvedTarget,
            message: format!("`{}` has no reference symbol at this site", site.key),
            path: Some(input.path.clone()),
            position: Some(site.selection.start),
        })?;
    let symbol = evaluator.combined_reference_symbol(symbol, &site)?;
    Ok(ReferenceSelected::Member { site, symbol })
}

fn identity(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    selected: &ReferenceSelected,
) -> Result<Identity, AnalysisError> {
    match selected {
        ReferenceSelected::Lexical(ReferenceQuery::At { file, position }) => {
            let file = project.searched_file(file)?;
            let offset = project
                .sources
                .get(file)
                .text
                .offset(*position)
                .map_err(|_| {
                    super::super::internal("selected lexical anchor is not in its captured source")
                })?;
            let mut symbols = references::lexical_symbols_at(project, file, offset)?;
            symbols.sort();
            symbols.dedup();
            if symbols.is_empty() {
                return Err(super::super::internal(
                    "catalog reference selector has no lexical symbol",
                ));
            }
            Ok(Identity::Lexical(symbols))
        }
        ReferenceSelected::Lexical(ReferenceQuery::Name { .. }) => Err(super::super::internal(
            "bare lexical name has no single selector identity",
        )),
        ReferenceSelected::Member { symbol, site } => Ok(Identity::Member(
            evaluator.reference_symbol_identity(*symbol, &site.selection)?,
        )),
    }
}

fn candidate(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    selected: &ReferenceSelected,
    selector: Option<String>,
) -> Result<ReferenceCandidate, AnalysisError> {
    let (file, position, question, declarations) = match selected {
        ReferenceSelected::Lexical(query @ ReferenceQuery::At { file, position }) => {
            let id = project.searched_file(file)?;
            let selection = references::selection(project, query)?;
            let result =
                references::lexical(project, query.clone(), Default::default(), selection)?;
            (
                id,
                *position,
                ReferenceBaseline::Lexical,
                result
                    .groups
                    .into_iter()
                    .flat_map(|group| group.declarations)
                    .collect(),
            )
        }
        ReferenceSelected::Lexical(ReferenceQuery::Name { .. }) => {
            return Err(super::super::internal(
                "bare lexical name cannot be an ambiguity candidate",
            ));
        }
        ReferenceSelected::Member { site, symbol } => {
            let file = project
                .sources
                .ids()
                .find(|&file| project.sources.get(file).display == site.selection.file)
                .ok_or_else(|| {
                    super::super::internal("reference selection lacks a captured file")
                })?;
            let declarations =
                evaluator.reference_symbol_named_declarations(*symbol, &site.selection)?;
            (
                file,
                site.selection.start,
                ReferenceBaseline::Member,
                declarations,
            )
        }
    };
    let offset = project
        .sources
        .get(file)
        .text
        .offset(position)
        .map_err(|_| super::super::internal("reference replay anchor lacks its captured offset"))?;
    let replay = at(project, evaluator, file, offset)?;
    if identity(project, evaluator, &replay)? != identity(project, evaluator, selected)? {
        return Err(super::super::internal(
            "reference candidate does not replay its own symbol and operation",
        ));
    }
    let file = project.sources.get(file).display.clone();
    Ok(ReferenceCandidate {
        selector,
        file: file.clone(),
        question,
        declarations,
        anchor: crate::references::ReferenceAnchor { file, position },
    })
}
