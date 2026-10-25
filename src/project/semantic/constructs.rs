//! Q5 construction sites: each object literal of the universe in the searched
//! files the filters admit, the target alternatives it constructs, and their
//! properties it does not certainly provide (docs/constructs-design.md). Each
//! literal's question is the evaluator's; this owner selects the target,
//! schedules the literals in exact path/position order, records gaps and
//! assembles the result.

use super::evaluation::Evaluator;
use super::evaluation::constructs::{ConstructionFacts, ConstructionTarget, TargetForm};
use super::gaps::Gaps;
use super::model::DemandError;
use super::presentation::bounded_summary;
use super::selection::{self, SelectedSeed, Selection};
use crate::analysis::{AnalysisError, QueryError, QueryErrorKind};
use crate::constructs::{
    Construction, ConstructionSite, ConstructsAnswer, ConstructsCoverage, ConstructsFilters,
    ConstructsOperation, ConstructsOutcome, ConstructsRequest, ConstructsResult,
    ConstructsSelection, ConstructsTarget, Lack, LiteralUniverse, TargetAlternative,
};
use crate::project::{Project, references};
use crate::references::{Location, ReferenceFilters};
use crate::types::{SelectionFailure, TypeKey, TypeQuery, TypeQuestion};
use std::path::PathBuf;

pub(super) fn search(
    project: &Project,
    request: &ConstructsRequest,
    evaluator: &mut Evaluator<'_, '_>,
) -> Result<ConstructsAnswer, AnalysisError> {
    let selected = match selection::select(project, evaluator, &request.query)? {
        Selection::One(selected) => selected,
        Selection::Absent => return Ok(unselected(project, request, Vec::new(), false)),
        Selection::Ambiguous(candidates) => {
            return Ok(unselected(project, request, candidates, true));
        }
    };
    let location = selected.target.location.clone();
    let symbol = match &selected.seed {
        SelectedSeed::ResolvedSymbol { symbol }
            if selected.target.kind == TypeQuestion::Declared =>
        {
            *symbol.value()
        }
        _ => {
            return Err(unsupported(
                project,
                &location,
                "select a class, interface or type alias",
            ));
        }
    };
    let declared = evaluator.establish(&selected)?;
    let target = match evaluator.construction_target(symbol, declared, &location)? {
        TargetForm::Supported(target) => target,
        TargetForm::Refused(reason) => {
            return Err(unsupported(
                project,
                &location,
                &format!("construction sites cannot be identified: {reason}"),
            ));
        }
    };
    let alternatives = alternatives(evaluator, &target, &location)?;
    if let Some(key) = &request.filters.missing {
        declared_key(project, evaluator, &target, key, &location)?;
    }
    let filters = references::Filters::resolve(
        project,
        &ReferenceFilters {
            calls: false,
            exclude_tests: request.filters.exclude_tests,
            file: request.filters.file.clone(),
        },
    )?;
    // A JSON module's literals have no contextual type: never constructions.
    let mut files: Vec<_> = project
        .sources
        .ids()
        .filter(|&file| {
            project.bindings.reference_file_is_searched(file)
                && !evaluator.checker_inputs().bindings.file_is_json(file)
                && filters.allows(project, file, false)
        })
        .collect();
    files.sort_by(|&a, &b| {
        project
            .sources
            .get(a)
            .display
            .cmp(&project.sources.get(b).display)
    });
    let universe = target.universe(request.filters.all_literals);
    let mut gaps = Gaps::new(evaluator.checker_inputs().control.cloned());
    let mut sites = Vec::new();
    let (mut literals, mut examined) = (0, 0);
    for file in files {
        // A file's syntax is captured only while its literals are examined,
        // unless an earlier demand holds it: every searched file is scanned,
        // and keeping them all would hold the whole program's trees at once.
        let held = evaluator.source().holds_syntax(file);
        for root in evaluator.source().object_literals(file)? {
            literals += 1;
            let in_universe = match universe {
                LiteralUniverse::All => Ok(true),
                LiteralUniverse::TargetKeys => evaluator.in_default_universe(root, &target),
            };
            if let Ok(false) = in_universe {
                continue;
            }
            examined += 1;
            let at = super::location(evaluator.checker_inputs(), root.file, root.site.span);
            match in_universe.and_then(|_| site(evaluator, &target, root, &at)) {
                Ok(Some(site)) => sites.push(site),
                Ok(None) => {}
                Err(failure) => gaps.record(at, failure)?,
            }
        }
        if !held {
            evaluator.source().release_syntax(file);
        }
    }
    if let Some(key) = &request.filters.missing {
        sites.retain(|site: &ConstructionSite| {
            site.constructs.iter().any(|construction| {
                construction
                    .lacks
                    .iter()
                    .any(|lack| matches!(&lack.key, TypeKey::String { name } if name == key))
            })
        });
    }
    let gaps = gaps.finish();
    let outcome = if gaps.is_empty() {
        ConstructsOutcome::Complete
    } else {
        ConstructsOutcome::Partial
    };
    Ok(ConstructsAnswer::Selected(Box::new(ConstructsResult {
        operation: ConstructsOperation::Constructs,
        outcome,
        project: project.config.clone(),
        selection: echo_selection(project, &request.query)?,
        target: ConstructsTarget {
            summary: bounded_summary(evaluator, declared, &location)?,
            location,
            alternatives,
        },
        filters: ConstructsFilters {
            exclude_tests: request.filters.exclude_tests,
            file: filters.echo(project).file,
            missing: request.filters.missing.clone(),
            all_literals: request.filters.all_literals,
        },
        coverage: ConstructsCoverage {
            status: outcome,
            universe,
            literals,
            examined,
            sites: sites.len(),
            gaps,
        },
        sites,
    })))
}

/// One examined literal: its constructed alternatives, spelled.
fn site(
    evaluator: &mut Evaluator<'_, '_>,
    target: &ConstructionTarget,
    root: super::source::ExpressionRoot,
    at: &Location,
) -> Result<Option<ConstructionSite>, DemandError> {
    let Some(ConstructionFacts {
        contextual,
        constructs,
    }) = evaluator.construction_at(root, target)?
    else {
        return Ok(None);
    };
    let mut spelled = Vec::new();
    for construction in constructs {
        spelled.push(Construction {
            alternative: alternative_id(construction.alternative),
            summary: bounded_summary(evaluator, construction.instance, at)?,
            lacks: construction
                .lacks
                .into_iter()
                .map(|lack| Lack {
                    key: lack.key,
                    optional: lack.optional,
                    presence: lack.presence,
                })
                .collect(),
        });
    }
    Ok(Some(ConstructionSite {
        location: crate::references::metadata::copy_location(at),
        contextual_type: bounded_summary(evaluator, contextual, at)?,
        constructs: spelled,
    }))
}

fn alternative_id(index: usize) -> String {
    format!("a{index}")
}

fn alternatives(
    evaluator: &mut Evaluator<'_, '_>,
    target: &ConstructionTarget,
    location: &Location,
) -> Result<Vec<TargetAlternative>, AnalysisError> {
    let mut alternatives = Vec::new();
    for (index, &alternative) in target.alternatives.iter().enumerate() {
        alternatives.push(TargetAlternative {
            id: alternative_id(index),
            summary: bounded_summary(evaluator, alternative, location)?,
            origin: evaluator.graph_label_origin(alternative, location)?,
        });
    }
    Ok(alternatives)
}

/// `--missing KEY` must name a string-keyed property some target alternative
/// declares; otherwise the request names nothing to find missing.
fn declared_key(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    target: &ConstructionTarget,
    key: &str,
    location: &Location,
) -> Result<(), AnalysisError> {
    let mut names = Vec::new();
    for &alternative in &target.alternatives {
        for name in evaluator.construction_keys(alternative, location)? {
            if name == key {
                return Ok(());
            }
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    let suggestion = crate::spelling::suggestion(
        key,
        names
            .iter()
            .enumerate()
            .map(|(index, name)| (index, name.clone())),
        |left, right| left.cmp(&right),
    );
    Err(QueryError {
        kind: QueryErrorKind::UnknownProperty,
        message: match suggestion {
            Some(index) => format!(
                "no alternative of the target declares the string-keyed property `{key}`; did you mean `{}`?",
                names[index]
            ),
            None => format!(
                "no alternative of the target declares the string-keyed property `{key}`"
            ),
        },
        path: Some(source_path(project, location)),
        position: Some(location.start),
    }
    .into())
}

fn unsupported(project: &Project, location: &Location, message: &str) -> AnalysisError {
    QueryError {
        kind: QueryErrorKind::UnsupportedTarget,
        message: message.into(),
        path: Some(source_path(project, location)),
        position: Some(location.start),
    }
    .into()
}

/// The path of the source a result location names, as query failures name
/// the files they locate.
fn source_path(project: &Project, location: &Location) -> PathBuf {
    project
        .sources
        .ids()
        .map(|file| project.sources.get(file))
        .find(|source| source.display == location.file)
        .map_or_else(
            || PathBuf::from(&location.file),
            |source| source.path.clone(),
        )
}

fn unselected(
    project: &Project,
    request: &ConstructsRequest,
    candidates: Vec<crate::types::TypeCandidate>,
    ambiguous: bool,
) -> ConstructsAnswer {
    ConstructsAnswer::Unselected(ConstructsSelection {
        operation: ConstructsOperation::Constructs,
        project: project.config.clone(),
        selection: request.query.clone(),
        outcome: if ambiguous {
            SelectionFailure::Ambiguous { candidates }
        } else {
            SelectionFailure::NotFound { candidates }
        },
    })
}

/// The selection with its file named as `Location::file` names it, as
/// reference results echo theirs.
fn echo_selection(project: &Project, query: &TypeQuery) -> Result<TypeQuery, AnalysisError> {
    let echo = |file: &std::path::Path| references::echoed_file(project, file);
    Ok(match query {
        TypeQuery::At { file, position } => TypeQuery::At {
            file: echo(file)?,
            position: *position,
        },
        TypeQuery::Name { name, file } => TypeQuery::Name {
            name: name.clone(),
            file: file.as_deref().map(echo).transpose()?,
        },
    })
}
