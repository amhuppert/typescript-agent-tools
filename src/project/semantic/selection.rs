//! A typed seed and question from the operation's own verified syntax role.
//! Canonical binding identity decides collapse; display origins never do.

pub(super) mod references;

use super::super::Project;
use super::super::binding::NativeSymbolIdentity;
use super::super::sources::FileId;
use super::evaluation::{Evaluator, ReferenceSymbol};
use super::model::{NativeCompletion, NativeSequence};
use super::source::{
    CallRoot, DeclarationKind, Expression, MemberRoot, SelectedSyntax, TypeExpression,
};
use crate::analysis::{AnalysisError, QueryError, QueryErrorKind, SemanticFailureReason};
use crate::references::NamedLocation;
use crate::types::{TypeAnchor, TypeCandidate, TypeQuery, TypeQuestion, TypeTarget};
use std::collections::HashSet;

pub(super) struct Selected {
    pub(super) seed: SelectedSeed,
    pub(super) target: TypeTarget,
    /// The fixed fresh-checking prelude; collecting it performs no checking.
    pub(super) checking_prelude: Vec<CallRoot>,
}

pub(super) enum SelectedSeed {
    ResolvedSymbol {
        symbol: NativeCompletion<ReferenceSymbol>,
    },
    PendingValue {
        root: MemberRoot,
        name: Option<String>,
        reason: SemanticFailureReason,
        message: String,
    },
    Type(TypeExpression),
    ValueAt(Expression),
    Expression(Expression),
}

impl SelectedSeed {
    fn identity(
        &self,
        evaluator: &Evaluator<'_, '_>,
        consumer: &crate::references::Location,
    ) -> Result<Option<SelectionIdentity>, AnalysisError> {
        let symbol = match *self {
            Self::ResolvedSymbol { ref symbol } => *symbol.value(),
            Self::PendingValue { root, .. } => {
                return Ok(Some(SelectionIdentity::MemberRoot(root)));
            }
            Self::Type(_) | Self::ValueAt(_) | Self::Expression(_) => return Ok(None),
        };
        Ok(Some(SelectionIdentity::Symbol(
            evaluator.reference_symbol_identity(symbol, consumer)?,
        )))
    }
}

#[derive(PartialEq, Eq, Hash)]
enum SelectionIdentity {
    Symbol(NativeSymbolIdentity),
    MemberRoot(MemberRoot),
}

pub(super) enum Selection {
    One(Box<Selected>),
    Absent,
    Ambiguous(Vec<TypeCandidate>),
}

pub(super) fn select(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    query: &TypeQuery,
) -> Result<Selection, AnalysisError> {
    match query {
        TypeQuery::At { file, position } => {
            let id = project.searched_file(file)?;
            let input = project.sources.get(id);
            let offset = input
                .text
                .offset(*position)
                .map_err(|error| QueryError::invalid_position(error, file, Some(*position)))?;
            Ok(match at(project, evaluator, id, offset)? {
                Some(selected) => Selection::One(Box::new(selected)),
                None => Selection::Absent,
            })
        }
        TypeQuery::Name { name, file } => {
            let only = file
                .as_deref()
                .map(|file| project.searched_file(file))
                .transpose()?;
            let mut sequence = NativeSequence::new();
            let result = (|| -> Result<Selection, super::model::DemandError> {
                let mut selected = Vec::new();
                let mut identities = HashSet::new();
                for site in project.declaration_name_sites(name, only)? {
                    let candidate = site.candidate();
                    let input = project.sources.get(site.file);
                    let offset = input.text.offset(candidate.anchor).map_err(|_| {
                        super::internal(
                            "captured declaration anchor no longer addresses its source",
                        )
                    })?;
                    let seed = at(project, evaluator, site.file, offset)?.ok_or_else(|| {
                        super::internal(
                            "captured declaration anchor has no final semantic syntax role",
                        )
                    })?;
                    if let SelectedSeed::ResolvedSymbol { symbol } = &seed.seed {
                        sequence.read(Ok(symbol.clone().map(|_| ())))?;
                    }
                    let identity = seed
                        .seed
                        .identity(evaluator, &seed.target.location)?
                        .ok_or_else(|| {
                            super::internal("catalog declaration selected an expression/type role")
                        })?;
                    if identities.insert((identity, seed.target.kind)) {
                        selected.push((seed, candidate.selector));
                    }
                }
                Ok(match selected.len() {
                    0 => Selection::Absent,
                    1 => Selection::One(Box::new(selected.remove(0).0)),
                    _ => Selection::Ambiguous(
                        selected
                            .into_iter()
                            .map(|(seed, selector)| candidate(evaluator, &seed, selector))
                            .collect::<Result<_, _>>()?,
                    ),
                })
            })();
            let completed = sequence.finish(result)?;
            let mut outcome = None;
            let evidence = completed.map(|selection| outcome = Some(selection));
            let mut outcome = outcome.expect("completion map installs its selected value");
            if let Selection::One(chosen) = &mut outcome {
                if let SelectedSeed::ResolvedSymbol { symbol } = &mut chosen.seed {
                    let value = *symbol.value();
                    *symbol = evidence.map(|_| value);
                } else {
                    evidence.admit()?;
                }
            } else {
                evidence.admit()?;
            }
            Ok(outcome)
        }
    }
}

fn at(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    file: FileId,
    offset: usize,
) -> Result<Option<Selected>, AnalysisError> {
    let Some(syntax) = evaluator.source().select(file, offset)? else {
        return Ok(None);
    };
    let checking_prelude = evaluator.source().enclosing_calls(file, offset)?;
    let (seed, token, kind) = match syntax {
        SelectedSyntax::Member {
            anchor,
            declaration,
        } => {
            let symbol = evaluator.reference_symbol_of_declaration_completion(declaration)?;
            (
                SelectedSeed::ResolvedSymbol { symbol },
                anchor,
                TypeQuestion::Value,
            )
        }
        SelectedSyntax::Alias {
            declaration,
            type_only,
            token,
        } => {
            let consumer = super::location(evaluator.checker_inputs(), file, token);
            let symbol = evaluator.selected_alias_target_completion(declaration, &consumer)?;
            let symbol = symbol.and_then_value(|symbol| {
                let flags = evaluator.reference_symbol_flags(symbol, &consumer)?;
                let kind = if flags.type_ && (type_only || !flags.value) {
                    TypeQuestion::Declared
                } else if flags.value {
                    TypeQuestion::Value
                } else {
                    return Err(super::failure_at_location(
                        evaluator.checker_inputs(),
                        &consumer,
                        SemanticFailureReason::UnsupportedSemanticForm,
                        "selected alias target has no declared or value type",
                    )
                    .into());
                };
                Ok((symbol, kind))
            })?;
            let kind = symbol.value().1;
            let symbol = symbol.map(|(symbol, _)| symbol);
            (SelectedSeed::ResolvedSymbol { symbol }, token, kind)
        }
        SelectedSyntax::Declaration {
            anchor,
            kind,
            declaration,
        } => {
            let symbol = evaluator.reference_symbol_of_declaration_completion(declaration)?;
            let question = match kind {
                DeclarationKind::Variable
                | DeclarationKind::Function
                | DeclarationKind::EnumMember => TypeQuestion::Value,
                DeclarationKind::TypeAlias
                | DeclarationKind::Class
                | DeclarationKind::Interface
                | DeclarationKind::Enum
                | DeclarationKind::TypeParameter => TypeQuestion::Declared,
            };
            (SelectedSeed::ResolvedSymbol { symbol }, anchor, question)
        }
        SelectedSyntax::Type(expression) => {
            let location = expression.location.clone();
            return Ok(Some(Selected {
                seed: SelectedSeed::Type(expression),
                target: TypeTarget {
                    location,
                    kind: TypeQuestion::Type,
                },
                checking_prelude,
            }));
        }
        SelectedSyntax::Expression(expression) => {
            let location = expression.location.clone();
            return Ok(Some(Selected {
                seed: SelectedSeed::Expression(expression),
                target: TypeTarget {
                    location,
                    kind: TypeQuestion::Expression,
                },
                checking_prelude,
            }));
        }
        SelectedSyntax::ValueAt(expression) => {
            let location = expression.location.clone();
            return Ok(Some(Selected {
                seed: SelectedSeed::ValueAt(expression),
                target: TypeTarget {
                    location,
                    kind: TypeQuestion::Value,
                },
                checking_prelude,
            }));
        }
        SelectedSyntax::PendingValue {
            root,
            name,
            reason,
            message,
        } => (
            SelectedSeed::PendingValue {
                root,
                name,
                reason,
                message,
            },
            root.span(),
            TypeQuestion::Value,
        ),
        SelectedSyntax::UnsupportedTarget { token } => {
            return Err(QueryError {
                kind: QueryErrorKind::UnsupportedTarget,
                message: "this syntax token has no supported type anchor".into(),
                path: Some(project.sources.get(file).path.clone()),
                position: Some(project.sources.get(file).text.position(token.start)),
            }
            .into());
        }
        SelectedSyntax::Pending { token } => {
            return Err(super::failure(
                evaluator.checker_inputs(),
                file,
                token,
                SemanticFailureReason::UnsupportedSemanticForm,
                "native semantic selection for this syntax role is pending",
            ));
        }
    };
    let location = super::location(evaluator.checker_inputs(), file, token);
    Ok(Some(Selected {
        seed,
        target: TypeTarget { location, kind },
        checking_prelude,
    }))
}

fn candidate(
    evaluator: &mut Evaluator<'_, '_>,
    selected: &Selected,
    selector: Option<String>,
) -> Result<TypeCandidate, AnalysisError> {
    let declarations = match selected.seed {
        SelectedSeed::ResolvedSymbol { ref symbol } => evaluator
            .reference_symbol_named_declarations(*symbol.value(), &selected.target.location)?,
        SelectedSeed::PendingValue {
            root,
            ref name,
            reason,
            ref message,
        } => {
            let name = name.as_ref().ok_or_else(|| {
                super::failure(
                    evaluator.checker_inputs(),
                    root.file,
                    root.span(),
                    reason,
                    message.clone(),
                )
            })?;
            vec![NamedLocation {
                name: name.clone(),
                location: super::location(evaluator.checker_inputs(), root.file, root.span()),
            }]
        }
        SelectedSeed::Type(_) | SelectedSeed::ValueAt(_) | SelectedSeed::Expression(_) => {
            return Err(super::internal(
                "catalog candidate has no declaration identity",
            ));
        }
    };
    let location = &selected.target.location;
    Ok(TypeCandidate {
        selector,
        file: location.file.clone(),
        question: selected.target.kind,
        declarations,
        anchor: TypeAnchor {
            file: location.file.clone(),
            line: location.start.line,
            column: location.start.column,
        },
    })
}
