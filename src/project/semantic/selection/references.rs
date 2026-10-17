//! Q3 selects a reference operation and proves replay using the same token
//! path. Q2 contributes declaration sites, never semantic identity or roles.

use super::super::evaluation::{Evaluator, ReferenceSymbol};
use super::super::source::ReferenceSite;
use crate::analysis::{AnalysisError, QueryError, QueryErrorKind};
use crate::project::binding::{NativeSymbolIdentity, SymbolId};
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
                let input = project.sources.get(declaration.file);
                let offset = input.text.offset(candidate.anchor).map_err(|_| {
                    super::super::internal(
                        "captured reference selector anchor no longer addresses its source",
                    )
                })?;
                let token = project
                    .bindings
                    .reference_token_at(declaration.file, offset)
                    .ok_or_else(|| QueryError {
                        kind: QueryErrorKind::UnsupportedTarget,
                        message: "the selected declaration has no reference token role".into(),
                        path: Some(input.path.clone()),
                        position: Some(candidate.anchor),
                    })?;
                let anchor = evaluator
                    .source()
                    .reference_selector_anchor(project.bindings.reference_token_facts(token))?;
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
            let result = references::lexical(project, query.clone(), Default::default())?;
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
