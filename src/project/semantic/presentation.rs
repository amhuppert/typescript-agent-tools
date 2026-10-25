//! Finite descriptions from semantic facts. Type identity and construction
//! remain in the model; this owner controls display depth, IDs and byte limits.

use super::evaluation::{Evaluator, ReferenceDisplay};
use super::model::{
    AccessorDisplay, DemandError, DemandId, Index, MembersOfType, MembersView, Parameter,
    ParameterValue, Property, PropertyDisplay, PropertyDisplayDemand, PropertySerializationId,
    PropertyValueQuestion, ReferenceView, Signature, SignatureId, SpecialName, TupleElementFlags,
    TypeDemand, TypeId, TypeView,
};
use crate::analysis::{AnalysisError, SemanticError, SemanticErrorKind, SemanticResourceReason};
use crate::printer::{TypePosition as SummaryPosition, TypePrecedence as SummaryPrecedence};
use crate::references::Location;
use crate::types::*;
use std::collections::{HashMap, HashSet, VecDeque};

mod literals;
use literals::LiteralPresentation;

const MAX_NODES: usize = 2_000;
const MAX_BYTES: usize = 262_144;
const MAX_SUMMARY_BYTES: usize = 512;
// Summary nesting is independent of a request's graph expansion depth.
const MAX_SUMMARY_NESTING: usize = 32;

// These are the four structural collection fields in the public vocabulary.
// A deferred whole inventory cannot infer that any one of them is empty.
const MEMBER_COLLECTIONS: [(&str, TypeCollection); 4] = [
    ("properties", TypeCollection::Properties),
    ("callSignatures", TypeCollection::CallSignatures),
    ("constructSignatures", TypeCollection::ConstructSignatures),
    ("indexSignatures", TypeCollection::IndexSignatures),
];

pub(super) struct Presented {
    pub(super) description: TypeDescription,
    pub(super) issues: Vec<TypeIssue>,
}

/// Present one established root; unsupported child issues remain distinct from
/// display elision, and invalid/resource/internal child failures stay errors.
pub(super) fn present(
    evaluator: &mut Evaluator<'_, '_>,
    root: TypeId,
    request: &TypeRequest,
    consumer: &Location,
) -> Result<Presented, AnalysisError> {
    evaluator.describe(root).map_err(AnalysisError::from)?;
    let mut graph = Graph::new(evaluator, request.depth, consumer.clone());
    let summary = summary(
        graph.evaluator,
        root,
        &mut SummaryContext::new(consumer.clone(), graph.literals.clone()),
        &mut graph.issues,
    )?;
    let expansion = if request.expand {
        Some(graph.build(root, &summary)?)
    } else {
        None
    };
    let description = TypeDescription {
        summary: summary.text,
        summary_truncated: summary.truncated,
        expansion,
    };
    if graph.sizes.encoded_size(&description)? > MAX_BYTES {
        return Err(output_limit(
            "type description cannot fit its display byte budget",
        ));
    }
    Ok(Presented {
        description,
        issues: graph.issues.finish()?,
    })
}

/// Q5's bounded spelling of one established type: a Q4 summary's text and
/// byte bound. An unsupported detail it meets is that type's failure, located
/// at its own site; a display cannot stand in for it.
pub(super) fn bounded_summary(
    evaluator: &mut Evaluator<'_, '_>,
    id: TypeId,
    consumer: &Location,
) -> Result<crate::constructs::Summary, DemandError> {
    evaluator.describe(id)?;
    let mut issues = Issues::default();
    let summary = summary(
        evaluator,
        id,
        &mut SummaryContext::new(consumer.clone(), LiteralPresentation::default()),
        &mut issues,
    )?;
    if let Some(issue) = issues.ordered.into_iter().next() {
        return Err(super::unsupported_demand_at_location(
            evaluator.checker_inputs(),
            &issue.location,
            issue.reason,
            issue.message,
        ));
    }
    Ok(crate::constructs::Summary {
        summary: summary.text,
        summary_truncated: summary.truncated,
    })
}

/// Node-builder recursion and infer declarations have independent dynamic
/// scopes. Nested conditionals replace the infer list only for their extends
/// clause; all exit paths restore the enclosing context.
struct SummaryContext {
    consumer: Location,
    literals: LiteralPresentation,
    types: HashSet<TypeId>,
    infer_parameters: Vec<TypeId>,
    in_extends: bool,
    reverse_mapped_stack: Vec<super::model::PropertySymbolId>,
}

impl SummaryContext {
    fn new(consumer: Location, literals: LiteralPresentation) -> Self {
        Self {
            consumer,
            literals,
            types: HashSet::new(),
            infer_parameters: Vec::new(),
            in_extends: false,
            reverse_mapped_stack: Vec::new(),
        }
    }
}

struct Summary {
    infer_constraint: Option<bool>,
    emitted_precedence: Option<SummaryPrecedence>,
    text: String,
    truncated: bool,
}

/// Encountered semantic issues survive display rollback. A native failed
/// property question survives dependent graph/display wrappers; other failures
/// retain their exact deferred question. Origins and equal messages do not merge.
#[derive(Default)]
struct Issues {
    ids: HashSet<IssueIdentity>,
    ordered: Vec<TypeIssue>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum IssueIdentity {
    Child(DemandId),
    Members(MembersOfType),
    Property(PropertySerializationId),
    PropertyValue(PropertyValueQuestion),
    SignaturePredicate(SignatureId),
    SignatureParameters(SignatureId),
    ParameterName(super::model::PropertySymbolId),
    ParameterConstraint(TypeId),
    MappedDisplayModifiers(TypeId),
}

/// A semantic failure's identity travels with its located public issue. Graph
/// edges cannot substitute their wrapper question after canonicalization.
struct UnsupportedFailure {
    identity: IssueIdentity,
    issue: TypeIssue,
}

impl Issues {
    fn record_failure(
        &mut self,
        fallback: IssueIdentity,
        failure: DemandError,
    ) -> Result<TypeIssue, AnalysisError> {
        let failure = unsupported_failure(fallback, failure)?;
        self.record_identity(failure.identity, failure.issue.clone());
        Ok(failure.issue)
    }

    fn record_identity(&mut self, id: IssueIdentity, issue: TypeIssue) {
        if self.ids.insert(id) {
            self.ordered.push(issue);
        }
    }

    /// Public issue order is exact source site then wire reason. Reuse the
    /// vocabulary's serializer instead of defining a second spelling table;
    /// semantic demand and graph enumeration orders remain unchanged.
    fn finish(self) -> Result<Vec<TypeIssue>, AnalysisError> {
        let mut rows = self
            .ordered
            .into_iter()
            .map(|issue| {
                let reason = serde_json::to_string(&issue.reason)
                    .map_err(|_| super::internal("semantic issue reason serialization failed"))?;
                Ok((reason, issue))
            })
            .collect::<Result<Vec<_>, AnalysisError>>()?;
        rows.sort_by(|left, right| {
            left.1
                .location
                .cmp(&right.1.location)
                .then_with(|| left.0.cmp(&right.0))
        });
        Ok(rows.into_iter().map(|(_, issue)| issue).collect())
    }
}

fn unsupported_issue(mut failure: DemandError) -> Result<TypeIssue, AnalysisError> {
    if matches!(failure.error(), AnalysisError::Semantic(error) if error.permits_unsupported_leaf())
        && let Some(issue) = failure.take_issue()
    {
        return Ok(issue);
    }
    Err(failure.into())
}

fn unsupported_failure(
    fallback: IssueIdentity,
    failure: DemandError,
) -> Result<UnsupportedFailure, AnalysisError> {
    let identity = failure
        .failed_property
        .map_or(fallback, IssueIdentity::PropertyValue);
    Ok(UnsupportedFailure {
        identity,
        issue: unsupported_issue(failure)?,
    })
}

fn summary_demand(
    evaluator: &mut Evaluator<'_, '_>,
    demand: TypeDemand,
    position: SummaryPosition,
    active: &mut SummaryContext,
    issues: &mut Issues,
) -> Result<Summary, AnalysisError> {
    let result = evaluator.demand(demand);
    summary_demand_result(evaluator, demand, result, position, active, issues)
}

fn summary_demand_result(
    evaluator: &mut Evaluator<'_, '_>,
    demand: TypeDemand,
    result: Result<TypeId, DemandError>,
    position: SummaryPosition,
    active: &mut SummaryContext,
    issues: &mut Issues,
) -> Result<Summary, AnalysisError> {
    match result {
        Ok(id) => summary_at(evaluator, id, position, active, issues),
        Err(failure) => {
            let TypeDemand::Deferred(id) = demand else {
                return Err(failure.into());
            };
            issues.record_failure(IssueIdentity::Child(id), failure)?;
            Ok(Summary {
                infer_constraint: None,
                emitted_precedence: None,
                text: "unsupported".into(),
                truncated: false,
            })
        }
    }
}

/// Complete clauses are the unit of summary admission. Closing syntax and an
/// elision marker are reserved before adding a property; names stay whole.
fn summary(
    evaluator: &mut Evaluator<'_, '_>,
    id: TypeId,
    active: &mut SummaryContext,
    issues: &mut Issues,
) -> Result<Summary, AnalysisError> {
    if evaluator.is_enum_like(id) {
        let name = evaluator
            .enum_display_summary(id, &active.consumer)
            .map_err(AnalysisError::from)?;
        let text = String::from_utf8(name)
            .map_err(|_| super::internal("enum display has no exact scalar text"))?;
        return Ok(bounded_token(text));
    }
    let metadata = evaluator.metadata(id).map_err(AnalysisError::from)?;
    if let Some(alias) = metadata.alias {
        let Some(arguments) = alias
            .arguments
            .as_ref()
            .filter(|arguments| !arguments.is_empty())
        else {
            return Ok(bounded_token(alias.display_name.clone()));
        };
        if active.types.len() == MAX_SUMMARY_NESTING || !active.types.insert(id) {
            return Ok(Summary {
                infer_constraint: None,
                emitted_precedence: None,
                text: "…".into(),
                truncated: true,
            });
        }
        let mut parts = vec![SummaryPart::Text(format!("{}<", alias.display_name))];
        for (index, &argument) in arguments.iter().enumerate() {
            if index != 0 {
                parts.push(SummaryPart::Text(", ".into()));
            }
            parts.push(SummaryPart::Child(TypeDemand::Known(argument)));
        }
        parts.push(SummaryPart::Text(">".into()));
        let result = summary_parts(evaluator, parts, MAX_SUMMARY_BYTES, active, issues);
        active.types.remove(&id);
        return Ok(result?.unwrap_or(Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: "…".into(),
            truncated: true,
        }));
    }
    if let Some(origin) = metadata.summary_origin {
        return summary(evaluator, origin, active, issues);
    }
    if let Some((name, type_query)) =
        evaluator.anonymous_symbol_display(id, active.types.contains(&id), &active.consumer)?
    {
        let name = String::from_utf8(name)
            .map_err(|_| super::internal("native nominal type display has no exact scalar text"))?;
        let mut result = bounded_token(name);
        result.emitted_precedence = Some(if type_query {
            SummaryPrecedence::TypeOperator
        } else {
            SummaryPrecedence::NonArray
        });
        return Ok(result);
    }
    if let Some(name) = metadata.nominal_name {
        return Ok(bounded_token(name.to_string()));
    }
    if active.types.contains(&id) {
        if metadata.function_symbol.is_some() {
            // Native node-builder recursion is a finite printed clause,
            // distinct from a product display-cap elision.
            return Ok(Summary {
                infer_constraint: None,
                emitted_precedence: None,
                text: "...".into(),
                truncated: false,
            });
        }
        return Ok(Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: "…".into(),
            truncated: true,
        });
    }
    if active.types.len() == MAX_SUMMARY_NESTING {
        return Ok(Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: "…".into(),
            truncated: true,
        });
    }
    active.types.insert(id);
    let result = match evaluator.describe(id) {
        Ok(TypeView::Special(name)) => Ok(Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: name.text().into(),
            truncated: false,
        }),
        Ok(TypeView::Enum) => Err(super::internal(
            "enum summary must pass its native named-type owner",
        )),
        Ok(TypeView::UniqueSymbol(_)) => Ok(Summary {
            text: "unique symbol".into(),
            truncated: false,
            emitted_precedence: None,
            infer_constraint: None,
        }),
        Ok(TypeView::Primitive(name)) => Ok(Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: primitive_name(name).into(),
            truncated: false,
        }),
        Ok(TypeView::TypeParameter(parameter)) => {
            if active.infer_parameters.contains(&id) {
                let parts = vec![
                    SummaryPart::Text("infer ".into()),
                    SummaryPart::Text(parameter.copy_display_name()),
                    SummaryPart::InferConstraint(id),
                ];
                Ok(
                    summary_parts(evaluator, parts, MAX_SUMMARY_BYTES, active, issues)?.unwrap_or(
                        Summary {
                            infer_constraint: None,
                            emitted_precedence: None,
                            text: "…".into(),
                            truncated: true,
                        },
                    ),
                )
            } else {
                Ok(bounded_token(parameter.copy_display_name()))
            }
        }
        Ok(TypeView::Substitution(view)) => {
            let checkpoint = active.literals.checkpoint();
            let mut base = summary(evaluator, view.base, active, issues)?;
            if evaluator.display_no_infer_alias(id)? {
                base.text = format!("NoInfer<{}>", base.text);
                base.infer_constraint = None;
                if base.text.len() > MAX_SUMMARY_BYTES {
                    active.literals.rollback(checkpoint);
                    base = Summary {
                        infer_constraint: None,
                        emitted_precedence: None,
                        text: "…".into(),
                        truncated: true,
                    };
                }
            }
            Ok(base)
        }
        Ok(TypeView::Conditional(view)) => {
            let true_type = evaluator
                .conditional_branch_demand(id, true)
                .map_err(AnalysisError::from)?;
            let false_type = evaluator
                .conditional_branch_demand(id, false)
                .map_err(AnalysisError::from)?;
            let mut parts = vec![SummaryPart::TypeOperand {
                demand: TypeDemand::Known(view.check_type),
                position: SummaryPosition::Operand(SummaryPrecedence::Union),
            }];
            parts.push(SummaryPart::Text(" extends ".into()));
            parts.push(SummaryPart::InferExtends {
                ty: view.extends_type,
                parameters: evaluator.conditional_parameters(id)?,
            });
            parts.extend([
                SummaryPart::Text(" ? ".into()),
                SummaryPart::Child(true_type),
                SummaryPart::Text(" : ".into()),
                SummaryPart::Child(false_type),
            ]);
            Ok(
                summary_parts(evaluator, parts, MAX_SUMMARY_BYTES, active, issues)?.unwrap_or(
                    Summary {
                        infer_constraint: None,
                        emitted_precedence: None,
                        text: "…".into(),
                        truncated: true,
                    },
                ),
            )
        }
        Ok(TypeView::StringMapping(mapping)) => {
            let parts = vec![
                SummaryPart::Text(format!("{}<", mapping.kind.name())),
                SummaryPart::Child(TypeDemand::Known(mapping.target)),
                SummaryPart::Text(">".into()),
            ];
            Ok(
                summary_parts(evaluator, parts, MAX_SUMMARY_BYTES, active, issues)?.unwrap_or(
                    Summary {
                        infer_constraint: None,
                        emitted_precedence: None,
                        text: "…".into(),
                        truncated: true,
                    },
                ),
            )
        }
        Ok(TypeView::TemplateLiteral(template)) => {
            let mut parts = vec![SummaryPart::Text("`".into())];
            for (index, text) in template.texts.iter().enumerate() {
                let escaped = evaluator
                    .quoted_literal_value(text, '`', &active.consumer)
                    .map_err(AnalysisError::from)?;
                parts.push(SummaryPart::Text(escaped[1..escaped.len() - 1].into()));
                if let Some(&ty) = template.types.get(index) {
                    parts.push(SummaryPart::Text("${".into()));
                    parts.push(SummaryPart::Child(TypeDemand::Known(ty)));
                    parts.push(SummaryPart::Text("}".into()));
                }
            }
            parts.push(SummaryPart::Text("`".into()));
            Ok(
                summary_parts(evaluator, parts, MAX_SUMMARY_BYTES, active, issues)?.unwrap_or(
                    Summary {
                        infer_constraint: None,
                        emitted_precedence: None,
                        text: "…".into(),
                        truncated: true,
                    },
                ),
            )
        }
        Ok(TypeView::Index(index)) => {
            let parts = vec![
                SummaryPart::Text("keyof ".into()),
                SummaryPart::TypeOperand {
                    demand: TypeDemand::Known(index.target),
                    position: SummaryPosition::Operand(SummaryPrecedence::TypeOperator),
                },
            ];
            Ok(
                summary_parts(evaluator, parts, MAX_SUMMARY_BYTES, active, issues)?.unwrap_or(
                    Summary {
                        infer_constraint: None,
                        emitted_precedence: None,
                        text: "…".into(),
                        truncated: true,
                    },
                ),
            )
        }
        Ok(TypeView::IndexedAccess(access)) => {
            let mut parts = vec![SummaryPart::TypeOperand {
                demand: TypeDemand::Known(access.object_type),
                position: SummaryPosition::Operand(SummaryPrecedence::Postfix),
            }];
            parts.push(SummaryPart::Text("[".into()));
            parts.push(SummaryPart::Child(TypeDemand::Known(access.index_type)));
            parts.push(SummaryPart::Text("]".into()));
            Ok(
                summary_parts(evaluator, parts, MAX_SUMMARY_BYTES, active, issues)?.unwrap_or(
                    Summary {
                        infer_constraint: None,
                        emitted_precedence: None,
                        text: "…".into(),
                        truncated: true,
                    },
                ),
            )
        }
        Ok(TypeView::Literal { base, value }) => {
            let checkpoint = active.literals.checkpoint();
            let text = if base == PrimitiveName::String {
                active.literals.summary(&value)?
            } else if base == PrimitiveName::Bigint {
                format!(
                    "{}n",
                    evaluator
                        .literal_scalar_text(&value, &active.consumer)
                        .map_err(AnalysisError::from)?
                )
            } else {
                evaluator
                    .literal_scalar_text(&value, &active.consumer)
                    .map_err(AnalysisError::from)?
                    .to_owned()
            };
            let result = bounded_token(text);
            if result.truncated {
                active.literals.rollback(checkpoint);
            }
            Ok(result)
        }
        Ok(TypeView::Object(members)) => {
            summary_members(evaluator, &members, false, active, issues)
        }
        Ok(TypeView::LazyObject(anonymous)) => {
            if evaluator
                .mapped_uses_syntax(id, &active.consumer)
                .map_err(AnalysisError::from)?
            {
                summary_mapped(evaluator, id, active, issues)
            } else {
                let members = evaluator
                    .object_display_members(anonymous.members, &active.consumer)
                    .map_err(AnalysisError::from)?;
                let reverse = evaluator
                    .is_reverse_mapped_type(id)
                    .map_err(AnalysisError::from)?;
                summary_members(evaluator, &members, reverse, active, issues)
            }
        }
        Ok(TypeView::Reference(reference)) => {
            summary_reference(evaluator, &reference, active, issues)
        }
        Ok(TypeView::Union(alternatives) | TypeView::Intersection(alternatives)) => {
            let intersection = matches!(
                evaluator.describe(id).map_err(AnalysisError::from)?,
                TypeView::Intersection(_)
            );
            let operator = if intersection { " & " } else { " | " };
            let parts = if intersection {
                alternatives.to_vec()
            } else {
                evaluator
                    .formatted_union_types_completion(&alternatives, false, &active.consumer)
                    .and_then(|completed| completed.admit())
                    .map_err(AnalysisError::from)?
            };
            let mut text = String::new();
            let mut truncated = false;
            for (index, part) in parts.iter().enumerate() {
                let separator = if index == 0 { "" } else { operator };
                let remaining = parts.len() - index;
                let marker = format!("… {remaining} more …");
                let suffix = if remaining == 1 {
                    String::new()
                } else {
                    format!("{operator}… {} more …", remaining - 1)
                };
                if text.len() + separator.len() + 1 + suffix.len() > MAX_SUMMARY_BYTES {
                    text.push_str(separator);
                    text.push_str(&marker);
                    truncated = true;
                    break;
                }
                let checkpoint = active.literals.checkpoint();
                let child = summary_at(
                    evaluator,
                    *part,
                    SummaryPosition::Operand(SummaryPrecedence::TypeOperator),
                    active,
                    issues,
                )?;
                if text.len() + separator.len() + child.text.len() + suffix.len()
                    > MAX_SUMMARY_BYTES
                {
                    active.literals.rollback(checkpoint);
                    text.push_str(separator);
                    text.push_str(&marker);
                    truncated = true;
                    break;
                }
                text.push_str(separator);
                text.push_str(&child.text);
                truncated |= child.truncated;
            }
            Ok(Summary {
                infer_constraint: None,
                emitted_precedence: None,
                text,
                truncated,
            })
        }
        Err(failure) => Err(failure.into()),
    };
    active.types.remove(&id);
    result
}

/// Graph labels qualify every named declaration, so a later collision or
/// rolled-back sibling cannot change an earlier label. Node and omitted-edge
/// labels use this same bounded operation; the top-level summary stays native.
/// Keep the complete location suffix when it fits, eliding the native prefix
/// as a whole instead of cutting an identifier, type argument, or UTF-8 unit.
fn graph_summary(
    evaluator: &mut Evaluator<'_, '_>,
    id: TypeId,
    issues: &mut Issues,
    consumer: &Location,
    literals: &LiteralPresentation,
) -> Result<Summary, AnalysisError> {
    let checkpoint = literals.checkpoint();
    let mut value = summary(
        evaluator,
        id,
        &mut SummaryContext::new(consumer.clone(), literals.clone()),
        issues,
    )?;
    let Some(origin) = evaluator.graph_label_origin(id, consumer)? else {
        return Ok(value);
    };
    let suffix = format!(
        " ({}:{}:{})",
        origin.file, origin.start.line, origin.start.column
    );
    if value.text.len() + suffix.len() <= MAX_SUMMARY_BYTES {
        value.text.push_str(&suffix);
    } else {
        literals.rollback(checkpoint);
        value.text = if "…".len() + suffix.len() <= MAX_SUMMARY_BYTES {
            format!("…{suffix}")
        } else {
            "…".into()
        };
        value.truncated = true;
    }
    Ok(value)
}

fn summary_mapped(
    evaluator: &mut Evaluator<'_, '_>,
    id: TypeId,
    active: &mut SummaryContext,
    issues: &mut Issues,
) -> Result<Summary, AnalysisError> {
    use super::evaluation::mapped::MappedOperand;
    let header = evaluator
        .mapped_header(id, &active.consumer)
        .map_err(AnalysisError::from)?;
    let prefix = format!(
        "{{ {}[{} in {}",
        header.readonly,
        header.name,
        if header.keyof { "keyof " } else { "" }
    );
    let middle = format!("]{}: ", header.optional);
    let minimum = prefix.len() + middle.len() + 3 + 2 + if header.has_name { 5 } else { 0 };
    if minimum > MAX_SUMMARY_BYTES {
        return Ok(Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: "…".into(),
            truncated: true,
        });
    }
    // Once this complete clause is admitted, native loads its template before
    // rendering its constraint/name; the later display demand removes missing.
    let raw = evaluator
        .mapped_operand_demand(id, MappedOperand::Template, &active.consumer)
        .map_err(AnalysisError::from)?;
    let template_failed = match evaluator.demand(raw) {
        Ok(_) => false,
        Err(failure) => {
            let TypeDemand::Deferred(demand) = raw else {
                return Err(failure.into());
            };
            issues.record_failure(IssueIdentity::Child(demand), failure)?;
            true
        }
    };
    if !template_failed {
        if let Err(failure) = evaluator.mapped_display_needs_wrapper(id, false, &active.consumer) {
            issues.record_failure(IssueIdentity::MappedDisplayModifiers(id), failure)?;
        }
    }
    let constraint = evaluator
        .mapped_operand_demand(
            id,
            if header.keyof {
                MappedOperand::Modifiers
            } else {
                MappedOperand::Constraint
            },
            &active.consumer,
        )
        .map_err(AnalysisError::from)?;
    let mut parts = vec![SummaryPart::Text(prefix), SummaryPart::Child(constraint)];
    if header.has_name {
        let name = evaluator
            .mapped_operand_demand(id, MappedOperand::Name, &active.consumer)
            .map_err(AnalysisError::from)?;
        parts.extend([SummaryPart::Text(" as ".into()), SummaryPart::Child(name)]);
    }
    let template = if template_failed {
        raw
    } else {
        evaluator
            .mapped_operand_demand(id, MappedOperand::DisplayTemplate, &active.consumer)
            .map_err(AnalysisError::from)?
    };
    parts.extend([
        SummaryPart::Text(middle),
        SummaryPart::Child(template),
        SummaryPart::Text("; }".into()),
    ]);
    Ok(
        summary_parts(evaluator, parts, MAX_SUMMARY_BYTES, active, issues)?.unwrap_or(Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: "…".into(),
            truncated: true,
        }),
    )
}

fn bounded_token(text: String) -> Summary {
    if text.len() <= MAX_SUMMARY_BYTES {
        Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text,
            truncated: false,
        }
    } else {
        Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: "…".into(),
            truncated: true,
        }
    }
}

/// Printer `emitTypeNode`, `emitTypeNodeIn/OutsideExtends` and `emitReturnType`.
/// Infer precedence uses the constraint-presence fact acquired by this child,
/// so determining parentheses cannot repeat or force an elided semantic query.
fn summary_at(
    evaluator: &mut Evaluator<'_, '_>,
    id: TypeId,
    position: SummaryPosition,
    active: &mut SummaryContext,
    issues: &mut Issues,
) -> Result<Summary, AnalysisError> {
    let state = position.state(active.in_extends);
    let minimum = state.minimum;
    let in_extends = state.in_extends;
    let inferred = active.infer_parameters.contains(&id);
    let precedence = if inferred {
        None
    } else {
        Some(emitted_type_precedence(evaluator, id, active)?)
    };
    let parentheses = precedence.is_some_and(|precedence| precedence < minimum);
    let saved = active.in_extends;
    active.in_extends = in_extends && !parentheses;
    let result = summary(evaluator, id, active, issues);
    active.in_extends = saved;
    let mut result = result?;
    let precedence = match result.infer_constraint {
        Some(present) => crate::printer::infer_type_precedence(present),
        None => result
            .emitted_precedence
            .or(precedence)
            .unwrap_or(SummaryPrecedence::NonArray),
    };
    if state.parenthesizes(precedence, result.infer_constraint == Some(true)) {
        result.text = format!("({})", result.text);
        result.infer_constraint = None;
        result.emitted_precedence = Some(SummaryPrecedence::NonArray);
    }
    Ok(result)
}

enum SummaryPart {
    Text(String),
    Child(TypeDemand),
    Parameter(ParameterValue),
    ExpandedSignature {
        signature: Signature,
        construct: bool,
        arrow: bool,
        display_return: bool,
    },
    SignatureReturn {
        signature: Signature,
        arrow: bool,
    },
    SignatureReturnOperand {
        signature: Signature,
        position: SummaryPosition,
    },
    TypeOperand {
        demand: TypeDemand,
        position: SummaryPosition,
    },
    InferExtends {
        ty: TypeId,
        parameters: Vec<TypeId>,
    },
    InferConstraint(TypeId),
}

/// Admit complete syntax parts against a true lower bound before demanding
/// children. A clause that cannot fit is elided as a unit; encountered issues
/// remain owned by the request even when that clause is subsequently elided.
/// Its presentation references commit only with the complete accepted clause.
fn summary_parts(
    evaluator: &mut Evaluator<'_, '_>,
    parts: Vec<SummaryPart>,
    maximum: usize,
    active: &mut SummaryContext,
    issues: &mut Issues,
) -> Result<Option<Summary>, AnalysisError> {
    let checkpoint = active.literals.checkpoint();
    let result = (|| {
        let mut minimums = Vec::with_capacity(parts.len());
        let mut remaining = parts.iter().try_fold(0usize, |length, part| {
            let minimum = match part {
                SummaryPart::Parameter(parameter) => {
                    let name = evaluator
                        .summary_parameter_name(parameter, true, &active.consumer)
                        .map_err(AnalysisError::from)?;
                    name.map_or(0, |name| name.len())
                        + usize::from(parameter.rest) * 3
                        + usize::from(parameter.optional)
                        + 3 // colon, space, type
                }
                // A tuple rest may expand to no parameters. Only fixed signature
                // punctuation is a safe pre-acquisition lower bound here.
                SummaryPart::ExpandedSignature {
                    construct,
                    arrow,
                    display_return,
                    ..
                } => {
                    usize::from(*construct) * 4
                        + 2
                        + if *display_return {
                            if *arrow { 5 } else { 3 }
                        } else {
                            0
                        }
                }
                SummaryPart::Text(text) => text.len(),
                SummaryPart::Child(_)
                | SummaryPart::TypeOperand { .. }
                | SummaryPart::SignatureReturnOperand { .. }
                | SummaryPart::InferExtends { .. } => 1,
                SummaryPart::InferConstraint(_) => 0,
                SummaryPart::SignatureReturn { signature, .. } => {
                    signature_return_minimum(signature)?
                }
            };
            minimums.push(minimum);
            length
                .checked_add(minimum)
                .ok_or_else(|| output_limit("type summary metadata length overflowed"))
        })?;
        if remaining > maximum {
            return Ok(None);
        }
        let mut text = String::new();
        let mut truncated = false;
        let mut infer_constraint = None;
        for (part, minimum) in parts.into_iter().zip(minimums) {
            let value = match part {
                SummaryPart::Text(text) => {
                    remaining -= text.len();
                    Summary {
                        infer_constraint: None,
                        emitted_precedence: None,
                        text,
                        truncated: false,
                    }
                }
                SummaryPart::Child(demand) => {
                    remaining -= 1;
                    summary_demand(evaluator, demand, SummaryPosition::Outside, active, issues)?
                }
                SummaryPart::ExpandedSignature {
                    signature,
                    construct,
                    arrow,
                    display_return,
                } => {
                    remaining -= minimum;
                    let parts = match evaluator
                        .expanded_parameters_completion(&signature, true, &active.consumer)
                        .and_then(|value| value.admit())
                    {
                        Ok(parameters) => {
                            let expanded = parameters.first().ok_or_else(|| {
                                super::internal("native signature expansion has no list")
                            })?;
                            let parameters =
                                Evaluator::signature_display_parameters(&signature, expanded);
                            signature_parts_for_parameters(
                                &signature,
                                parameters,
                                construct,
                                arrow,
                                display_return,
                            )?
                        }
                        Err(failure) => {
                            issues.record_failure(
                                IssueIdentity::SignatureParameters(signature.id),
                                failure,
                            )?;
                            vec![SummaryPart::Text("unsupported".into())]
                        }
                    };
                    let Some(value) = summary_parts(
                        evaluator,
                        parts,
                        maximum - text.len() - remaining,
                        active,
                        issues,
                    )?
                    else {
                        return Ok(None);
                    };
                    value
                }
                SummaryPart::Parameter(parameter) => {
                    remaining -= minimum;
                    // Native symbolToParameterDeclaration serializes the type
                    // before cloning the name. Pattern spelling is never the
                    // parameter symbol's internal generated name.
                    let mut value = summary_demand(
                        evaluator,
                        parameter.display_type,
                        SummaryPosition::Outside,
                        active,
                        issues,
                    )?;
                    let name =
                        match evaluator.summary_parameter_name(&parameter, false, &active.consumer)
                        {
                            Ok(Some(name)) => name,
                            Ok(None) => {
                                return Err(super::internal("admitted parameter name is absent"));
                            }
                            Err(failure) => {
                                issues.record_failure(
                                    IssueIdentity::ParameterName(parameter.symbol),
                                    failure,
                                )?;
                                "unsupported".into()
                            }
                        };
                    value.text = format!(
                        "{}{}{}: {}",
                        if parameter.rest { "..." } else { "" },
                        name,
                        if parameter.optional { "?" } else { "" },
                        value.text
                    );
                    value
                }
                SummaryPart::TypeOperand { demand, position } => {
                    remaining -= 1;
                    summary_demand(evaluator, demand, position, active, issues)?
                }
                SummaryPart::SignatureReturnOperand {
                    signature,
                    position,
                } => {
                    remaining -= 1;
                    let result =
                        evaluator.get_return_type_of_signature(&signature, &active.consumer);
                    summary_demand_result(
                        evaluator,
                        signature.return_type,
                        result,
                        position,
                        active,
                        issues,
                    )?
                }
                SummaryPart::InferExtends { ty, parameters } => {
                    remaining -= 1;
                    let saved = std::mem::replace(&mut active.infer_parameters, parameters);
                    let value = summary_at(evaluator, ty, SummaryPosition::Extends, active, issues);
                    active.infer_parameters = saved;
                    value?
                }
                SummaryPart::InferConstraint(ty) => {
                    match evaluator.infer_constraint_for_display(ty) {
                        Ok(Some(constraint)) => {
                            infer_constraint = Some(true);
                            let mut value = summary_at(
                                evaluator,
                                constraint,
                                SummaryPosition::Extends,
                                active,
                                issues,
                            )?;
                            value.text = format!(" extends {}", value.text);
                            value
                        }
                        Ok(None) => {
                            infer_constraint = Some(false);
                            Summary {
                                infer_constraint: None,
                                emitted_precedence: None,
                                text: String::new(),
                                truncated: false,
                            }
                        }
                        Err(failure) => {
                            issues
                                .record_failure(IssueIdentity::ParameterConstraint(ty), failure)?;
                            infer_constraint = Some(true);
                            Summary {
                                infer_constraint: None,
                                emitted_precedence: None,
                                text: " extends unsupported".into(),
                                truncated: false,
                            }
                        }
                    }
                }
                SummaryPart::SignatureReturn { signature, arrow } => {
                    remaining -= signature_return_minimum(&signature)?;
                    let consumer = active.consumer.clone();
                    let parts = match evaluator.type_predicate_of_signature(&signature, &consumer) {
                        Ok(Some(predicate)) => predicate_parts(&signature, &predicate)?,
                        Ok(None) => vec![SummaryPart::SignatureReturnOperand {
                            signature: signature.clone(),
                            position: if arrow {
                                SummaryPosition::ArrowReturn
                            } else {
                                SummaryPosition::Outside
                            },
                        }],
                        Err(failure) => {
                            issues.record_failure(
                                IssueIdentity::SignaturePredicate(signature.id),
                                failure,
                            )?;
                            vec![SummaryPart::Text("unsupported".into())]
                        }
                    };
                    let Some(value) = summary_parts(
                        evaluator,
                        parts,
                        maximum - text.len() - remaining,
                        active,
                        issues,
                    )?
                    else {
                        return Ok(None);
                    };
                    value
                }
            };
            if text.len() + value.text.len() + remaining > maximum {
                return Ok(None);
            }
            text.push_str(&value.text);
            truncated |= value.truncated;
        }
        Ok(Some(Summary {
            infer_constraint,
            emitted_precedence: None,
            text,
            truncated,
        }))
    })();
    if !matches!(&result, Ok(Some(_))) {
        active.literals.rollback(checkpoint);
    }
    result
}

/// `typeReferenceToTypeNode`: an established Array target prints its actual
/// element; ordinary targets print their established name and actual args.
/// Neither display question resolves the structured member inventory.
fn summary_reference(
    evaluator: &mut Evaluator<'_, '_>,
    reference: &ReferenceView,
    active: &mut SummaryContext,
    issues: &mut Issues,
) -> Result<Summary, AnalysisError> {
    let mut parts = Vec::new();
    let arguments = evaluator
        .get_type_arguments(reference.ty)
        .map_err(AnalysisError::from)?;
    let display = evaluator
        .reference_display(reference.ty)
        .map_err(AnalysisError::from)?;
    let arguments = if display == ReferenceDisplay::Tuple {
        evaluator.tuple_arguments_for_display(reference.ty)
    } else {
        Ok(arguments)
    }
    .map_err(AnalysisError::from)?;
    match display {
        ReferenceDisplay::Array { readonly, .. } => {
            let element = array_element(evaluator, reference)?;
            if readonly {
                parts.push(SummaryPart::Text("readonly ".into()));
            }
            parts.push(SummaryPart::TypeOperand {
                demand: TypeDemand::Known(element),
                position: SummaryPosition::Operand(SummaryPrecedence::Postfix),
            });
            parts.push(SummaryPart::Text("[]".into()));
        }
        ReferenceDisplay::Ordinary => {
            let count = evaluator
                .reference_display_argument_count(reference.ty, &arguments)
                .map_err(AnalysisError::from)?;
            parts.push(SummaryPart::Text(reference.name.to_string()));
            if count != 0 {
                parts.push(SummaryPart::Text("<".into()));
                for (index, &argument) in arguments[..count].iter().enumerate() {
                    if index != 0 {
                        parts.push(SummaryPart::Text(", ".into()));
                    }
                    parts.push(SummaryPart::Child(TypeDemand::Known(argument)));
                }
                parts.push(SummaryPart::Text(">".into()));
            }
        }
        ReferenceDisplay::Tuple => {
            let tuple = reference
                .tuple
                .as_ref()
                .ok_or_else(|| super::internal("tuple reference has no target metadata"))?;
            parts.push(SummaryPart::Text(
                if tuple.readonly { "readonly [" } else { "[" }.into(),
            ));
            for (index, info) in tuple.elements.iter().enumerate() {
                if index != 0 {
                    parts.push(SummaryPart::Text(", ".into()));
                }
                let optional = info.flags.intersects(TupleElementFlags::OPTIONAL);
                let rest = info.flags.intersects(TupleElementFlags::REST);
                let variable = info.flags.intersects(TupleElementFlags::VARIABLE);
                if variable {
                    parts.push(SummaryPart::Text("...".into()));
                }
                if let Some(label) = &info.label {
                    let label = label
                        .name
                        .as_ref()
                        .map_err(|failure| AnalysisError::from(failure.clone()))?;
                    parts.push(SummaryPart::Text(format!(
                        "{label}{}: ",
                        if optional { "?" } else { "" }
                    )));
                }
                let postfix = rest || optional && info.label.is_none();
                parts.push(SummaryPart::TypeOperand {
                    demand: TypeDemand::Known(arguments[index]),
                    position: if postfix {
                        SummaryPosition::OutsideAt(SummaryPrecedence::Postfix)
                    } else {
                        SummaryPosition::Outside
                    },
                });
                if rest {
                    parts.push(SummaryPart::Text("[]".into()));
                }
                if optional && info.label.is_none() {
                    parts.push(SummaryPart::Text("?".into()));
                }
            }
            parts.push(SummaryPart::Text("]".into()));
        }
    }
    Ok(
        summary_parts(evaluator, parts, MAX_SUMMARY_BYTES, active, issues)?.unwrap_or(Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: "…".into(),
            truncated: true,
        }),
    )
}

fn array_element(
    evaluator: &mut Evaluator<'_, '_>,
    reference: &ReferenceView,
) -> Result<TypeId, AnalysisError> {
    let arguments = evaluator
        .get_type_arguments(reference.ty)
        .map_err(AnalysisError::from)?;
    let [element, ..] = arguments.as_ref() else {
        return Err(super::internal(
            "established Array reference has a different argument arity",
        ));
    };
    Ok(*element)
}

fn reference_kind(
    evaluator: &mut Evaluator<'_, '_>,
    reference: &ReferenceView,
    consumer: &Location,
) -> Result<TypeKind, DemandError> {
    match evaluator.reference_display(reference.ty)? {
        ReferenceDisplay::Array { .. } => Ok(TypeKind::Array),
        ReferenceDisplay::Tuple => Ok(TypeKind::Tuple),
        // Real callable/constructable shape is required graph metadata.
        // getSignaturesOfType resolves the native whole inventory, including
        // index dependencies; nominal Class/Interface flags cannot replace it.
        ReferenceDisplay::Ordinary => evaluator
            .members_at(reference.members, consumer)
            .map(|members| structured_kind(&members)),
    }
}

/// `signatureToSignatureDeclarationHelper` / `symbolToParameterDeclaration`:
/// print established parameter/signature facts. Annotation checking, optional
/// argument policy and predicate subject identity remain evaluator decisions.
fn signature_parts(
    signature: &Signature,
    construct: bool,
    arrow: bool,
) -> Result<Vec<SummaryPart>, AnalysisError> {
    signature_parts_with_return(signature, construct, arrow, true)
}

fn signature_parts_with_return(
    signature: &Signature,
    construct: bool,
    arrow: bool,
    display_return: bool,
) -> Result<Vec<SummaryPart>, AnalysisError> {
    if signature.flags.has_rest() {
        return Ok(vec![SummaryPart::ExpandedSignature {
            signature: signature.clone(),
            construct,
            arrow,
            display_return,
        }]);
    }
    signature_parts_for_parameters(
        signature,
        &signature.parameters,
        construct,
        arrow,
        display_return,
    )
}

fn signature_parts_for_parameters(
    signature: &Signature,
    parameters: &[ParameterValue],
    construct: bool,
    arrow: bool,
    display_return: bool,
) -> Result<Vec<SummaryPart>, AnalysisError> {
    let mut parts = Vec::new();
    let text = |value: &str| SummaryPart::Text(value.into());
    if construct {
        if arrow && signature.flags.has_abstract() {
            parts.push(text("abstract "));
        }
        parts.push(text("new "));
    }
    if !signature.type_parameters().is_empty() {
        parts.push(text("<"));
        for (index, parameter) in signature.type_parameters().iter().enumerate() {
            if index != 0 {
                parts.push(text(", "));
            }
            // `createModifiersFromModifierFlags` places Const before the
            // declared In/Out variance modifiers.
            if parameter.constant {
                parts.push(text("const "));
            }
            if let Some(variance) = &parameter.variance {
                parts.push(SummaryPart::Text(format!("{} ", variance.as_str())));
            }
            parts.push(SummaryPart::Text(parameter.copy_display_name()));
            if let Some(constraint) = parameter.constraint {
                parts.push(text(" extends "));
                parts.push(SummaryPart::Child(constraint));
            }
            if let Some(default) = parameter.default {
                parts.push(text(" = "));
                parts.push(SummaryPart::Child(default));
            }
        }
        parts.push(text(">"));
    }
    parts.push(text("("));
    for (index, parameter) in signature
        .this_parameter()
        .iter()
        .chain(parameters)
        .enumerate()
    {
        if index != 0 {
            parts.push(text(", "));
        }
        parts.push(SummaryPart::Parameter(parameter.clone()));
    }
    parts.push(text(if !display_return {
        ")"
    } else if arrow {
        ") => "
    } else {
        "): "
    }));
    if !display_return {
        return Ok(parts);
    }
    parts.push(SummaryPart::SignatureReturn {
        signature: signature.clone(),
        arrow,
    });
    Ok(parts)
}

fn signature_return_minimum(signature: &Signature) -> Result<usize, AnalysisError> {
    let Some(predicate) = &signature.predicate else {
        return Ok(1);
    };
    predicate_parts(signature, predicate)?
        .iter()
        .try_fold(0usize, |total, part| {
            total
                .checked_add(match part {
                    SummaryPart::Text(text) => text.len(),
                    SummaryPart::Child(_) => 1,
                    SummaryPart::SignatureReturn { .. }
                    | SummaryPart::Parameter(_)
                    | SummaryPart::ExpandedSignature { .. }
                    | SummaryPart::SignatureReturnOperand { .. }
                    | SummaryPart::TypeOperand { .. }
                    | SummaryPart::InferExtends { .. }
                    | SummaryPart::InferConstraint(_) => {
                        unreachable!("predicate parts contain no nested contextual clause")
                    }
                })
                .ok_or_else(|| output_limit("signature return metadata length overflowed"))
        })
}

fn predicate_parts(
    signature: &Signature,
    predicate: &super::model::Predicate,
) -> Result<Vec<SummaryPart>, AnalysisError> {
    let mut parts = Vec::new();
    let subject = match predicate.subject {
        TypePredicateSubject::This(_) => "this",
        TypePredicateSubject::ParameterIndex(index) => signature
            .parameters
            .get(index)
            .ok_or_else(|| super::internal("type predicate has an invalid parameter index"))?
            .name
            .as_str(),
    };
    if predicate.kind != TypePredicateKind::Is {
        parts.push(SummaryPart::Text("asserts ".into()));
    }
    parts.push(SummaryPart::Text(subject.into()));
    if predicate.kind != TypePredicateKind::Asserts {
        parts.push(SummaryPart::Text(" is ".into()));
        parts.push(SummaryPart::Child(predicate.value_type.ok_or_else(
            || super::internal("typed predicate has no established asserted type"),
        )?));
    }
    Ok(parts)
}

/// `createTypeNodeFromObjectType` / `createTypeNodesFromResolvedType`: native
/// arrow eligibility and summary clause order are separate from graph order.
/// Abstract lone constructors retain their native modifier; compound abstract
/// structure additionally requires native intersection normalization.
fn summary_members(
    evaluator: &mut Evaluator<'_, '_>,
    members: &MembersView,
    reverse: bool,
    active: &mut SummaryContext,
    issues: &mut Issues,
) -> Result<Summary, AnalysisError> {
    if let Some((signature, construct)) = arrow_signature(members) {
        return Ok(summary_parts(
            evaluator,
            signature_parts(signature, construct, true)?,
            MAX_SUMMARY_BYTES,
            active,
            issues,
        )?
        .unwrap_or_else(|| Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: "…".into(),
            truncated: true,
        }));
    }
    enum Clause<'a> {
        Parts(Vec<SummaryPart>),
        Property(&'a Property),
    }
    let mut clauses = Vec::new();
    for signature in members.call_signatures.iter() {
        clauses.push(Clause::Parts(signature_parts(signature, false, false)?));
    }
    for signature in members.construct_signatures.iter() {
        clauses.push(Clause::Parts(signature_parts(signature, true, false)?));
    }
    for index in members.index_signatures.iter() {
        clauses.push(Clause::Parts(vec![
            SummaryPart::Text(format!(
                "{}[{}: ",
                if index.readonly { "readonly " } else { "" },
                index.parameter_name
            )),
            SummaryPart::Child(index.key_type),
            SummaryPart::Text("]: ".into()),
            if reverse {
                SummaryPart::Text("...".into())
            } else {
                SummaryPart::Child(index.value_type)
            },
        ]));
    }
    for property in members.properties.iter() {
        clauses.push(Clause::Property(property));
    }
    if clauses.is_empty() {
        return Ok(Summary {
            infer_constraint: None,
            emitted_precedence: None,
            text: "{}".into(),
            truncated: false,
        });
    }
    let total = clauses.len();
    let mut text = String::from("{ ");
    let mut truncated = false;
    for (index, clause) in clauses.into_iter().enumerate() {
        let remaining = total - index;
        let marker = format!("… {remaining} more … }}");
        let suffix = if remaining == 1 {
            "}".to_string()
        } else {
            format!("… {} more … }}", remaining - 1)
        };
        let maximum = MAX_SUMMARY_BYTES.saturating_sub(text.len() + suffix.len());
        let mut reverse_property = None;
        let groups = match clause {
            Clause::Parts(parts) => vec![parts],
            Clause::Property(property) => {
                // A non-optional method can serialize to no clauses, so its
                // mode decision has a zero lower bound. Ordinary property and
                // accessor syntax always retains the complete exact key.
                let method = property.kind == TypePropertyKind::Method;
                let minimum = if method {
                    0
                } else {
                    match &property.key {
                        TypeKey::String { name } => name.len() + 4,
                        TypeKey::Private { .. } | TypeKey::Symbol { .. } => 4,
                    }
                };
                if minimum > maximum {
                    text.push_str(&marker);
                    truncated = true;
                    break;
                }
                // Established names refine the row lower bound without reading
                // property types or computed/unique name dependencies. A deferred
                // name uses only the syntax minimum until the row is admitted.
                if !method {
                    let preview = evaluator
                        .preview_property_name(property)
                        .map_err(AnalysisError::from)?;
                    if preview.is_some_and(|name| name.len() + 4 > maximum) {
                        text.push_str(&marker);
                        truncated = true;
                        break;
                    }
                }
                let placeholder = property
                    .symbol
                    .map(|symbol| {
                        evaluator.reverse_mapped_property_placeholder(
                            symbol,
                            &active.reverse_mapped_stack,
                        )
                    })
                    .transpose()
                    .map_err(AnalysisError::from)?
                    .unwrap_or(false);
                let parts = property_parts(evaluator, property, placeholder, issues)?;
                if !placeholder
                    && let Some(symbol) = property.symbol
                    && evaluator
                        .is_reverse_mapped_property(symbol)
                        .map_err(AnalysisError::from)?
                {
                    reverse_property = Some(symbol);
                }
                parts
            }
        };
        let mut parts = Vec::new();
        for mut group in groups {
            group.push(SummaryPart::Text("; ".into()));
            parts.extend(group);
        }
        if let Some(symbol) = reverse_property {
            active.reverse_mapped_stack.push(symbol);
        }
        let result = summary_parts(evaluator, parts, maximum, active, issues);
        if reverse_property.is_some() {
            active.reverse_mapped_stack.pop();
        }
        let Some(clause) = result? else {
            text.push_str(&marker);
            truncated = true;
            break;
        };
        text.push_str(&clause.text);
        truncated |= clause.truncated;
    }
    if !truncated || !text.ends_with('}') {
        text.push('}');
    }
    if text == "{ }" {
        text = "{}".into();
    }
    Ok(Summary {
        infer_constraint: None,
        emitted_precedence: None,
        text,
        truncated,
    })
}

/// Native mode choice is an evaluator operation, distinct from raw graph
/// read/write questions. This formatter receives only established clauses.
fn property_parts(
    evaluator: &mut Evaluator<'_, '_>,
    property: &Property,
    placeholder: bool,
    issues: &mut Issues,
) -> Result<Vec<Vec<SummaryPart>>, AnalysisError> {
    let (name, display) = match property.display {
        PropertyDisplayDemand::Value(value_type) => (
            super::evaluation::property_key_name(&property.key),
            PropertyDisplay::Plain { value_type },
        ),
        PropertyDisplayDemand::Native(id) => {
            let prepared =
                match evaluator.prepare_summary_property_completion(property, placeholder) {
                    Ok(prepared) => prepared,
                    Err(failure) => {
                        issues.record_failure(IssueIdentity::Property(id), failure)?;
                        return Ok(vec![vec![SummaryPart::Text(format!(
                            "{}: unsupported",
                            super::evaluation::property_key_name(&property.key)
                        ))]]);
                    }
                };
            let name = prepared.value().name().to_owned();
            if placeholder {
                if let Err(failure) = prepared.admit() {
                    issues.record_failure(IssueIdentity::Property(id), failure)?;
                }
                return Ok(vec![vec![SummaryPart::Text(format!(
                    "{}{}{}: ...",
                    if property.readonly { "readonly " } else { "" },
                    name,
                    if property.optional { "?" } else { "" }
                ))]]);
            }
            let display = match prepared
                .and_then(|prepared| evaluator.finish_summary_property_completion(prepared))
                .and_then(|display| display.admit())
            {
                Ok(display) => display,
                Err(failure) => {
                    issues.record_failure(IssueIdentity::Property(id), failure)?;
                    return Ok(vec![vec![SummaryPart::Text(format!(
                        "{name}: unsupported"
                    ))]]);
                }
            };
            (name, display)
        }
    };
    if placeholder {
        return Ok(vec![vec![SummaryPart::Text(format!(
            "{}{}{}: ...",
            if property.readonly { "readonly " } else { "" },
            name,
            if property.optional { "?" } else { "" }
        ))]]);
    }
    let clauses = match display {
        PropertyDisplay::Plain { value_type } => vec![vec![
            SummaryPart::Text(format!(
                "{}{}{}: ",
                if property.readonly { "readonly " } else { "" },
                name,
                if property.optional { "?" } else { "" }
            )),
            SummaryPart::Child(value_type),
        ]],
        PropertyDisplay::Methods { signatures } => signatures
            .iter()
            .map(|signature| {
                let mut parts = vec![SummaryPart::Text(format!(
                    "{}{}",
                    name,
                    if property.optional { "?" } else { "" }
                ))];
                parts.extend(signature_parts(signature, false, false)?);
                Ok(parts)
            })
            .collect::<Result<Vec<_>, AnalysisError>>()?,
        PropertyDisplay::Accessors(accessors) => {
            let AccessorDisplay { getter, setter } = *accessors;
            let mut clauses = Vec::new();
            if let Some(getter) = getter {
                let mut parts = vec![SummaryPart::Text(format!("get {name}"))];
                parts.extend(signature_parts(&getter, false, false)?);
                clauses.push(parts);
            }
            if let Some(setter) = setter {
                let mut parts = vec![SummaryPart::Text(format!("set {name}"))];
                parts.extend(signature_parts_with_return(&setter, false, false, false)?);
                clauses.push(parts);
            }
            clauses
        }
    };
    Ok(clauses)
}

/// Native `GetTypeNodePrecedence` for the type-node forms emitted by the
/// established model families. Alias and nominal references have reference
/// precedence; callability alone does not make them emitted arrow types.
fn emitted_type_precedence(
    evaluator: &mut Evaluator<'_, '_>,
    id: TypeId,
    active: &SummaryContext,
) -> Result<SummaryPrecedence, AnalysisError> {
    let metadata = evaluator.metadata(id).map_err(AnalysisError::from)?;
    if metadata.alias.is_some() || metadata.nominal_name.is_some() {
        return Ok(SummaryPrecedence::NonArray);
    }
    if let Some(origin) = metadata.summary_origin {
        return emitted_type_precedence(evaluator, origin, active);
    }
    // The emitted symbol node supplies its actual precedence after native
    // serialization (TypeQuery differs from ImportType). Do not resolve the
    // member inventory merely to preview these nominal candidates.
    if matches!(
        evaluator.describe(id)?,
        TypeView::Object(_) | TypeView::LazyObject(_)
    ) && evaluator
        .type_symbol_flags(id)?
        .is_some_and(|flags| flags.class || flags.enum_ || flags.value_module)
    {
        return Ok(SummaryPrecedence::NonArray);
    }
    if active.types.contains(&id) {
        return Ok(SummaryPrecedence::NonArray);
    }
    if active.types.len() == MAX_SUMMARY_NESTING {
        // The bounded summary emits its placeholder before asking this type's
        // children. Precedence cannot cause an unshown inventory demand.
        return Ok(SummaryPrecedence::NonArray);
    }
    Ok(match evaluator.describe(id).map_err(AnalysisError::from)? {
        TypeView::Union(_) => SummaryPrecedence::Union,
        TypeView::Conditional(_) => SummaryPrecedence::Conditional,
        TypeView::Substitution(view) => {
            if evaluator.display_no_infer_alias(id)? {
                SummaryPrecedence::NonArray
            } else {
                emitted_type_precedence(evaluator, view.base, active)?
            }
        }
        TypeView::Intersection(_) => SummaryPrecedence::Intersection,
        TypeView::IndexedAccess(_) => SummaryPrecedence::Postfix,
        TypeView::Index(_) => SummaryPrecedence::TypeOperator,
        TypeView::Object(members) => {
            if arrow_signature(&members).is_some() {
                SummaryPrecedence::Function
            } else {
                SummaryPrecedence::NonArray
            }
        }
        TypeView::LazyObject(anonymous) => {
            if evaluator
                .mapped_uses_syntax(id, &active.consumer)
                .map_err(AnalysisError::from)?
            {
                return Ok(SummaryPrecedence::NonArray);
            }
            let members = evaluator
                .object_display_members(anonymous.members, &active.consumer)
                .map_err(AnalysisError::from)?;
            if arrow_signature(&members).is_some() {
                SummaryPrecedence::Function
            } else {
                SummaryPrecedence::NonArray
            }
        }
        TypeView::Reference(reference) => match evaluator
            .reference_display(reference.ty)
            .map_err(AnalysisError::from)?
        {
            ReferenceDisplay::Array { readonly: true, .. } => SummaryPrecedence::TypeOperator,
            ReferenceDisplay::Array {
                readonly: false, ..
            } => SummaryPrecedence::Postfix,
            ReferenceDisplay::Ordinary => SummaryPrecedence::NonArray,
            ReferenceDisplay::Tuple => {
                if reference.tuple.as_ref().is_some_and(|tuple| tuple.readonly) {
                    SummaryPrecedence::TypeOperator
                } else {
                    SummaryPrecedence::NonArray
                }
            }
        },
        TypeView::Special(_)
        | TypeView::Primitive(_)
        | TypeView::Enum
        | TypeView::UniqueSymbol(_)
        | TypeView::Literal { .. }
        | TypeView::TypeParameter(_)
        | TypeView::TemplateLiteral(_)
        | TypeView::StringMapping(_) => SummaryPrecedence::NonArray,
    })
}

/// `createTypeNodeFromObjectType`'s lone-signature branch emits a function or
/// constructor type; other complete member shapes emit an object type literal.
fn arrow_signature(members: &MembersView) -> Option<(&Signature, bool)> {
    if !members.properties.is_empty() || !members.index_signatures.is_empty() {
        return None;
    }
    match (
        members.call_signatures.as_ref(),
        members.construct_signatures.as_ref(),
    ) {
        ([signature], []) => Some((signature, false)),
        ([], [signature]) => Some((signature, true)),
        _ => None,
    }
}

fn structured_kind(members: &MembersView) -> TypeKind {
    if members.call_signatures.is_empty() && members.construct_signatures.is_empty() {
        TypeKind::Object
    } else {
        TypeKind::Function
    }
}

fn structured_data(source: &MembersView, members: TypeMembers) -> TypeNodeData {
    if structured_kind(source) == TypeKind::Function {
        TypeNodeData::Function { members }
    } else {
        TypeNodeData::Object { members }
    }
}

fn primitive_name(name: PrimitiveName) -> &'static str {
    match name {
        PrimitiveName::String => "string",
        PrimitiveName::Number => "number",
        PrimitiveName::Boolean => "boolean",
        PrimitiveName::Bigint => "bigint",
        PrimitiveName::Symbol => "symbol",
        PrimitiveName::Object => "object",
    }
}

/// First discovery in this breadth-first queue has minimum root distance.
/// Private TypeId is identity; origins and display labels never merge types.
/// Every queued node already owns a byte reservation for its exact metadata
/// and all possible omission records. Collection admission spends only local
/// record deltas; a wide list never serializes its preceding prefix again.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Identity {
    Type(TypeId),
    Failure(IssueIdentity),
}

struct Graph<'e, 'p, 'w> {
    consumer: Location,
    evaluator: &'e mut Evaluator<'p, 'w>,
    depth: usize,
    ids: HashMap<Identity, usize>,
    resolved: HashMap<DemandId, TypeId>,
    failed: HashMap<DemandId, IssueIdentity>,
    queue: VecDeque<(TypeId, usize)>,
    nodes: Vec<TypeNode>,
    bytes: usize,
    sizes: WireSize,
    truncated: bool,
    literals: LiteralPresentation,
    issues: Issues,
}

#[derive(Clone, Copy)]
struct Checkpoint {
    nodes: usize,
    literals: usize,
    queued: usize,
    bytes: usize,
    truncated: bool,
}

impl<'e, 'p, 'w> Graph<'e, 'p, 'w> {
    fn new(evaluator: &'e mut Evaluator<'p, 'w>, depth: usize, consumer: Location) -> Self {
        let sizes = WireSize;
        Self {
            consumer,
            evaluator,
            depth,
            ids: HashMap::new(),
            resolved: HashMap::new(),
            failed: HashMap::new(),
            queue: VecDeque::new(),
            nodes: Vec::new(),
            bytes: 0,
            sizes,
            truncated: false,
            literals: LiteralPresentation::default(),
            issues: Issues::default(),
        }
    }

    fn build(
        &mut self,
        root: TypeId,
        root_summary: &Summary,
    ) -> Result<TypeExpansion, AnalysisError> {
        self.truncated = root_summary.truncated;
        self.bytes = self.sizes.encoded_size(&TypeDescription {
            summary: root_summary.text.clone(),
            summary_truncated: root_summary.truncated,
            expansion: Some(TypeExpansion {
                root: "t0".into(),
                nodes: Vec::new(),
                truncated: self.truncated,
            }),
        })?;
        self.reserve(root, 0)?;
        if self.bytes > MAX_BYTES {
            return Err(output_limit(
                "exact root type metadata cannot fit its display byte budget",
            ));
        }
        while let Some((id, distance)) = self.queue.pop_front() {
            let index = self.ids[&Identity::Type(id)];
            let mut node = self.nodes[index].clone();
            let mut fields = node
                .omitted
                .as_ref()
                .map(|omitted| omitted.fields.clone())
                .unwrap_or_default();
            if distance < self.depth {
                if let Some(arguments) = self
                    .evaluator
                    .graph_type_arguments(id)
                    .map_err(AnalysisError::from)?
                    .filter(|arguments| arguments.len() != 0)
                {
                    // Reference argument resolution is one native collection
                    // question. Every unresolved edge may reuse t0; admit that
                    // true lower bound before asking for the complete vector.
                    let preview = match arguments.known_arguments() {
                        Some(arguments) => self.preview_edges(arguments),
                        None => vec![TypeRef::Ref { r#ref: "t0".into() }; arguments.len()],
                    };
                    let minimum =
                        signed_size(
                            self.sizes
                                .optional_field_size("typeArguments", Some(&preview))?,
                        )? - signed_size(self.sizes.optional_field_size(
                            "omittedTypeArguments",
                            node.omitted_type_arguments.as_ref(),
                        )?)? + self.sizes.omission_delta(&fields, "typeArguments", true)?;
                    if !self.fits(minimum) {
                        self.mark_truncated();
                    } else {
                        let checkpoint = self.checkpoint();
                        let arguments = self
                            .evaluator
                            .resolve_graph_type_arguments(&arguments)
                            .map_err(AnalysisError::from)?;
                        let values = arguments
                            .iter()
                            .map(|&argument| self.edge(argument, distance + 1))
                            .collect::<Result<Vec<_>, _>>()?;
                        let delta =
                            signed_size(
                                self.sizes
                                    .optional_field_size("typeArguments", Some(&values))?,
                            )? - signed_size(self.sizes.optional_field_size(
                                "omittedTypeArguments",
                                node.omitted_type_arguments.as_ref(),
                            )?)? + self.sizes.omission_delta(&fields, "typeArguments", true)?;
                        if self.fits(delta) {
                            self.spend(delta)?;
                            fields.retain(|field| field != "typeArguments");
                            node.type_arguments = Some(values);
                            node.omitted_type_arguments = None;
                        } else {
                            self.rollback(checkpoint);
                            self.mark_truncated();
                        }
                    }
                }
                match self.evaluator.describe(id).map_err(AnalysisError::from)? {
                    TypeView::Object(members) => {
                        let displayed = self.members(&members, distance, &mut fields)?;
                        node.data = structured_data(&members, displayed);
                    }
                    TypeView::LazyObject(anonymous) => {
                        if self
                            .evaluator
                            .is_generic_mapped_type(id, &self.consumer)
                            .map_err(AnalysisError::from)?
                        {
                            let (operands, parameters) = self
                                .evaluator
                                .mapped_graph_parts(id, &self.consumer)
                                .map_err(AnalysisError::from)?;
                            node.data = self.operator(
                                TypeOperator::Mapped,
                                &operands,
                                &parameters,
                                distance,
                                &mut fields,
                            )?;
                        } else {
                            let members = self
                                .evaluator
                                .object_display_members(anonymous.members, &self.consumer)
                                .map_err(AnalysisError::from)?;
                            let displayed = self.members(&members, distance, &mut fields)?;
                            node.data = structured_data(&members, displayed);
                        }
                    }
                    TypeView::Union(alternatives) => {
                        node.data =
                            self.alternatives(&alternatives, false, distance, &mut fields)?
                    }
                    TypeView::Intersection(alternatives) => {
                        node.data = self.alternatives(&alternatives, true, distance, &mut fields)?
                    }
                    TypeView::IndexedAccess(access) => {
                        node.data = self.operator(
                            TypeOperator::IndexedAccess,
                            &[access.object_type, access.index_type].map(TypeDemand::Known),
                            &[],
                            distance,
                            &mut fields,
                        )?;
                    }
                    TypeView::StringMapping(mapping) => {
                        node.data = self.operator(
                            TypeOperator::StringMapping,
                            &[TypeDemand::Known(mapping.target)],
                            &[],
                            distance,
                            &mut fields,
                        )?;
                    }
                    TypeView::Conditional(view) => {
                        let parameters = self.evaluator.conditional_parameters(id)?;
                        let true_type = self.evaluator.conditional_branch_demand(id, true)?;
                        let false_type = self.evaluator.conditional_branch_demand(id, false)?;
                        node.data = self.operator(
                            TypeOperator::Conditional,
                            &[
                                TypeDemand::Known(view.check_type),
                                TypeDemand::Known(view.extends_type),
                                true_type,
                                false_type,
                            ],
                            &parameters,
                            distance,
                            &mut fields,
                        )?;
                    }
                    TypeView::Substitution(view) => {
                        node.data = self.operator(
                            TypeOperator::Substitution,
                            &[view.base, view.constraint].map(TypeDemand::Known),
                            &[],
                            distance,
                            &mut fields,
                        )?;
                    }
                    TypeView::TemplateLiteral(template) => {
                        let operands: Vec<_> = template
                            .types
                            .iter()
                            .copied()
                            .map(TypeDemand::Known)
                            .collect();
                        node.data = self.operator(
                            TypeOperator::TemplateLiteral,
                            &operands,
                            &[],
                            distance,
                            &mut fields,
                        )?;
                    }
                    TypeView::Index(index) => {
                        node.data = self.operator(
                            TypeOperator::Keyof,
                            &[TypeDemand::Known(index.target)],
                            &[],
                            distance,
                            &mut fields,
                        )?;
                    }
                    TypeView::TypeParameter(parameter) => {
                        node.data = TypeNodeData::TypeParameter {
                            name: parameter.copy_name(),
                            constraint: self.parameter_constraint_child(
                                id,
                                parameter.constraint,
                                distance + 1,
                                &mut fields,
                            )?,
                            default: self.optional_child(
                                "default",
                                parameter.default,
                                distance + 1,
                                &mut fields,
                            )?,
                        };
                    }
                    TypeView::Reference(reference) => {
                        node.data = match self
                            .evaluator
                            .reference_display(reference.ty)
                            .map_err(AnalysisError::from)?
                        {
                            ReferenceDisplay::Ordinary => {
                                let members = self
                                    .evaluator
                                    .members_at(reference.members, &self.consumer)
                                    .map_err(AnalysisError::from)?;
                                let displayed = self.members(&members, distance, &mut fields)?;
                                structured_data(&members, displayed)
                            }
                            ReferenceDisplay::Array { readonly, .. } => {
                                let TypeNodeData::Array {
                                    element: before, ..
                                } = &node.data
                                else {
                                    return Err(super::internal(
                                        "Array display skeleton has a different kind",
                                    ));
                                };
                                let element_type = array_element(self.evaluator, &reference)?;
                                let element = self.required_child(
                                    "element",
                                    TypeDemand::Known(element_type),
                                    before,
                                    distance + 1,
                                    &mut fields,
                                )?;
                                let members = self.reference_members(
                                    reference.members,
                                    distance,
                                    &mut fields,
                                )?;
                                TypeNodeData::Array {
                                    element,
                                    readonly,
                                    members,
                                }
                            }
                            ReferenceDisplay::Tuple => {
                                let elements =
                                    self.tuple_elements(&reference, distance, &mut fields)?;
                                let members = self.reference_members(
                                    reference.members,
                                    distance,
                                    &mut fields,
                                )?;
                                TypeNodeData::Tuple {
                                    omitted_elements: remaining(
                                        reference
                                            .tuple
                                            .as_ref()
                                            .expect("tuple target metadata")
                                            .elements
                                            .len(),
                                        elements.len(),
                                    ),
                                    elements,
                                    readonly: reference
                                        .tuple
                                        .as_ref()
                                        .expect("tuple target metadata")
                                        .readonly,
                                    members,
                                }
                            }
                        };
                    }
                    TypeView::Special(_)
                    | TypeView::Primitive(_)
                    | TypeView::Enum
                    | TypeView::UniqueSymbol(_)
                    | TypeView::Literal { .. } => {}
                }
            }
            if !fields.is_empty() {
                self.mark_truncated();
            }
            node.omitted = (!fields.is_empty()).then_some(TypeFieldsOmitted {
                reason: if distance == self.depth {
                    TypeOmissionReason::Depth
                } else {
                    TypeOmissionReason::Bytes
                },
                fields,
            });
            self.nodes[index] = node;
        }
        Ok(TypeExpansion {
            root: "t0".into(),
            nodes: std::mem::take(&mut self.nodes),
            truncated: self.truncated,
        })
    }

    fn reserve(&mut self, id: TypeId, distance: usize) -> Result<(), AnalysisError> {
        let index = self.nodes.len();
        let node = self.skeleton(id, distance, index)?;
        if node.summary_truncated || distance == self.depth && node.omitted.is_some() {
            self.mark_truncated();
        }
        self.bytes = self
            .bytes
            .checked_add(self.sizes.encoded_size(&node)? + usize::from(index != 0))
            .ok_or_else(|| output_limit("type display byte accounting overflowed"))?;
        self.nodes.push(node);
        self.ids.insert(Identity::Type(id), index);
        self.queue.push_back((id, distance));
        Ok(())
    }

    fn skeleton(
        &mut self,
        id: TypeId,
        distance: usize,
        index: usize,
    ) -> Result<TypeNode, AnalysisError> {
        let summary = graph_summary(
            self.evaluator,
            id,
            &mut self.issues,
            &self.consumer,
            &self.literals,
        )?;
        let mut fields = Vec::new();
        let metadata = self.evaluator.metadata(id).map_err(AnalysisError::from)?;
        let mut omitted_type_arguments = None;
        let type_arguments = self
            .evaluator
            .graph_type_arguments(id)
            .map_err(AnalysisError::from)?
            .and_then(|arguments| {
                if arguments.len() == 0 {
                    Some(Vec::new())
                } else {
                    fields.push("typeArguments".into());
                    omitted_type_arguments = Some(arguments.len());
                    None
                }
            });
        let data = match self.evaluator.describe(id).map_err(AnalysisError::from)? {
            TypeView::Special(name) => match name {
                SpecialName::Any => TypeNodeData::Any,
                SpecialName::Unknown => TypeNodeData::Unknown,
                SpecialName::Never => TypeNodeData::Never,
                SpecialName::Void => TypeNodeData::Void,
                SpecialName::Undefined => TypeNodeData::Undefined,
                SpecialName::Null => TypeNodeData::Null,
            },
            TypeView::Enum => TypeNodeData::Enum {
                origins: self
                    .evaluator
                    .type_symbol_origins(id, &self.consumer)
                    .map_err(AnalysisError::from)?,
            },
            TypeView::UniqueSymbol(_) => TypeNodeData::UniqueSymbol {
                origins: self
                    .evaluator
                    .unique_symbol_origins(id, &self.consumer)
                    .map_err(AnalysisError::from)?,
            },
            TypeView::Primitive(name) => TypeNodeData::Primitive { name },
            TypeView::Literal { base, value } => TypeNodeData::Literal {
                base,
                value: self
                    .literals
                    .value(&value, self.evaluator, &self.consumer)?,
            },
            TypeView::Object(members) => structured_data(
                &members,
                skeletal_members(&members, distance == self.depth, &mut fields),
            ),
            TypeView::LazyObject(anonymous) => {
                if self
                    .evaluator
                    .is_generic_mapped_type(id, &self.consumer)
                    .map_err(AnalysisError::from)?
                {
                    let header = self
                        .evaluator
                        .mapped_header(id, &self.consumer)
                        .map_err(AnalysisError::from)?;
                    operator_skeleton(
                        TypeOperator::Mapped,
                        1 + usize::from(header.has_name),
                        1,
                        &mut fields,
                    )
                } else {
                    let members = self
                        .evaluator
                        .object_display_members(anonymous.members, &self.consumer)
                        .map_err(AnalysisError::from)?;
                    structured_data(
                        &members,
                        skeletal_members(&members, distance == self.depth, &mut fields),
                    )
                }
            }
            TypeView::Union(alternatives) => {
                fields.push("alternatives".into());
                TypeNodeData::Union {
                    alternatives: Vec::new(),
                    omitted_alternatives: Some(alternatives.len()),
                }
            }
            TypeView::Intersection(alternatives) => {
                fields.push("alternatives".into());
                TypeNodeData::Intersection {
                    alternatives: Vec::new(),
                    omitted_alternatives: Some(alternatives.len()),
                }
            }
            TypeView::TypeParameter(parameter) => {
                if self
                    .evaluator
                    .parameter_may_have_constraint(id)
                    .map_err(AnalysisError::from)?
                {
                    fields.push("constraint".into());
                }
                if parameter.default.is_some() {
                    fields.push("default".into());
                }
                TypeNodeData::TypeParameter {
                    name: parameter.copy_name(),
                    constraint: None,
                    default: None,
                }
            }
            TypeView::IndexedAccess(_) => {
                operator_skeleton(TypeOperator::IndexedAccess, 2, 0, &mut fields)
            }
            TypeView::StringMapping(_) => {
                operator_skeleton(TypeOperator::StringMapping, 1, 0, &mut fields)
            }
            TypeView::Conditional(_) => operator_skeleton(
                TypeOperator::Conditional,
                4,
                self.evaluator.conditional_parameters(id)?.len(),
                &mut fields,
            ),
            TypeView::Substitution(_) => {
                operator_skeleton(TypeOperator::Substitution, 2, 0, &mut fields)
            }
            TypeView::TemplateLiteral(template) => operator_skeleton(
                TypeOperator::TemplateLiteral,
                template.types.len(),
                0,
                &mut fields,
            ),
            TypeView::Index(_) => operator_skeleton(TypeOperator::Keyof, 1, 0, &mut fields),
            TypeView::Reference(reference) => match self
                .evaluator
                .reference_display(reference.ty)
                .map_err(AnalysisError::from)?
            {
                ReferenceDisplay::Array { readonly, .. } => {
                    fields.push("element".into());
                    let element_type = array_element(self.evaluator, &reference)?;
                    let element = self.omitted_type(
                        element_type,
                        if distance == self.depth {
                            TypeOmissionReason::Depth
                        } else {
                            TypeOmissionReason::Bytes
                        },
                    )?;
                    TypeNodeData::Array {
                        element,
                        readonly,
                        members: deferred_members(&mut fields),
                    }
                }
                ReferenceDisplay::Ordinary => {
                    let members = self
                        .evaluator
                        .members_at(reference.members, &self.consumer)
                        .map_err(AnalysisError::from)?;
                    structured_data(
                        &members,
                        skeletal_members(&members, distance == self.depth, &mut fields),
                    )
                }
                ReferenceDisplay::Tuple => {
                    let tuple = reference.tuple.as_ref().expect("tuple target metadata");
                    if !tuple.elements.is_empty() {
                        fields.push("elements".into());
                    }
                    TypeNodeData::Tuple {
                        elements: Vec::new(),
                        omitted_elements: remaining(tuple.elements.len(), 0),
                        readonly: tuple.readonly,
                        members: deferred_members(&mut fields),
                    }
                }
            },
        };
        Ok(TypeNode {
            id: format!("t{index}"),
            summary: summary.text,
            summary_truncated: summary.truncated,
            alias: metadata
                .alias
                .as_ref()
                .map(|alias| alias.provenance.clone()),
            type_arguments,
            omitted_type_arguments,
            omitted: (!fields.is_empty()).then_some(TypeFieldsOmitted {
                reason: if distance == self.depth {
                    TypeOmissionReason::Depth
                } else {
                    TypeOmissionReason::Bytes
                },
                fields,
            }),
            data,
        })
    }

    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            nodes: self.nodes.len(),
            literals: self.literals.checkpoint(),
            queued: self.queue.len(),
            bytes: self.bytes,
            truncated: self.truncated,
        }
    }

    fn rollback(&mut self, checkpoint: Checkpoint) {
        self.literals.rollback(checkpoint.literals);
        self.ids.retain(|_, index| *index < checkpoint.nodes);
        self.nodes.truncate(checkpoint.nodes);
        self.queue.truncate(checkpoint.queued);
        self.bytes = checkpoint.bytes;
        self.truncated = checkpoint.truncated;
    }

    fn fits(&self, delta: isize) -> bool {
        self.bytes
            .checked_add_signed(delta)
            .is_some_and(|bytes| bytes <= MAX_BYTES)
    }

    fn spend(&mut self, delta: isize) -> Result<(), AnalysisError> {
        self.bytes = self
            .bytes
            .checked_add_signed(delta)
            .ok_or_else(|| super::internal("type display byte reservation became inconsistent"))?;
        Ok(())
    }

    fn mark_truncated(&mut self) {
        if !self.truncated {
            self.truncated = true;
            // Compact JSON's `true` is one byte shorter than `false`.
            self.bytes -= usize::from(self.bytes != 0);
        }
    }

    fn demand_identity(&self, demand: TypeDemand) -> Option<Identity> {
        match demand {
            TypeDemand::Known(id) => Some(Identity::Type(id)),
            TypeDemand::Deferred(id) => self
                .resolved
                .get(&id)
                .map(|id| Identity::Type(*id))
                .or_else(|| self.failed.get(&id).copied().map(Identity::Failure)),
        }
    }

    fn preview_demand(
        &self,
        demand: TypeDemand,
        pending: &mut HashMap<Identity, usize>,
    ) -> TypeRef {
        // An unresolved demand can resolve to any already displayed
        // type. `t0` is the true reference-size lower bound; predicting
        // a new identity here could reject a fitting shared child.
        let Some(identity) = self.demand_identity(demand) else {
            return TypeRef::Ref { r#ref: "t0".into() };
        };
        let index = self.ids.get(&identity).copied().unwrap_or_else(|| {
            let next = self.nodes.len() + pending.len();
            *pending.entry(identity).or_insert(next)
        });
        TypeRef::Ref {
            r#ref: format!("t{index}"),
        }
    }

    fn preview_edges(&self, demands: &[TypeDemand]) -> Vec<TypeRef> {
        let mut pending = HashMap::new();
        demands
            .iter()
            .map(|&demand| self.preview_demand(demand, &mut pending))
            .collect()
    }

    fn edge(&mut self, demand: TypeDemand, distance: usize) -> Result<TypeRef, AnalysisError> {
        let result = self.evaluator.demand(demand);
        self.edge_result(demand, result, distance)
    }

    fn edge_result(
        &mut self,
        demand: TypeDemand,
        result: Result<TypeId, DemandError>,
        distance: usize,
    ) -> Result<TypeRef, AnalysisError> {
        let id = match result {
            Ok(id) => {
                if let TypeDemand::Deferred(question) = demand {
                    self.resolved.insert(question, id);
                }
                id
            }
            Err(failure) => {
                let TypeDemand::Deferred(id) = demand else {
                    return Err(failure.into());
                };
                let failure = unsupported_failure(IssueIdentity::Child(id), failure)?;
                self.failed.insert(id, failure.identity);
                return self.failed_edge(failure, distance);
            }
        };
        if let Some(&index) = self.ids.get(&Identity::Type(id)) {
            return Ok(TypeRef::Ref {
                r#ref: format!("t{index}"),
            });
        }
        // Required real-kind metadata belongs to this admitted child demand.
        // Its native inventory can fail after the semantic type was established;
        // keep the exact inventory question separate from successful TypeId.
        let inventory = match self.evaluator.describe(id).map_err(AnalysisError::from)? {
            TypeView::Reference(reference)
                if self
                    .evaluator
                    .reference_display(reference.ty)
                    .map_err(AnalysisError::from)?
                    == ReferenceDisplay::Ordinary =>
            {
                Some(reference.members)
            }
            TypeView::LazyObject(anonymous) => {
                match self.evaluator.is_generic_mapped_type(id, &self.consumer) {
                    Ok(true) => None,
                    Ok(false) => Some(anonymous.members),
                    Err(error) => {
                        let failure =
                            unsupported_failure(IssueIdentity::Members(anonymous.members), error)?;
                        return self.failed_edge(failure, distance);
                    }
                }
            }
            _ => None,
        };
        if let Some(inventory) = inventory {
            if let Err(failure) = self
                .evaluator
                .object_display_members(inventory, &self.consumer)
            {
                let failure = unsupported_failure(IssueIdentity::Members(inventory), failure)?;
                return self.failed_edge(failure, distance);
            }
        }
        if distance > self.depth || self.nodes.len() == MAX_NODES {
            let reason = if distance > self.depth {
                TypeOmissionReason::Depth
            } else {
                TypeOmissionReason::Nodes
            };
            self.mark_truncated();
            return self.omitted_type(id, reason);
        }
        self.reserve(id, distance)?;
        Ok(TypeRef::Ref {
            r#ref: format!("t{}", self.ids[&Identity::Type(id)]),
        })
    }

    fn omitted_type(
        &mut self,
        id: TypeId,
        reason: TypeOmissionReason,
    ) -> Result<TypeRef, AnalysisError> {
        let kind = match self.evaluator.describe(id).map_err(AnalysisError::from)? {
            TypeView::Special(name) => match name {
                SpecialName::Any => TypeKind::Any,
                SpecialName::Unknown => TypeKind::Unknown,
                SpecialName::Never => TypeKind::Never,
                SpecialName::Void => TypeKind::Void,
                SpecialName::Undefined => TypeKind::Undefined,
                SpecialName::Null => TypeKind::Null,
            },
            TypeView::Primitive(_) => TypeKind::Primitive,
            TypeView::Enum => TypeKind::Enum,
            TypeView::UniqueSymbol(_) => TypeKind::UniqueSymbol,
            TypeView::Literal { .. } => TypeKind::Literal,
            TypeView::Object(members) => structured_kind(&members),
            TypeView::LazyObject(anonymous) => {
                match self.evaluator.is_generic_mapped_type(id, &self.consumer) {
                    Ok(true) => TypeKind::Operator,
                    result => match result.and_then(|_| {
                        self.evaluator
                            .object_display_members(anonymous.members, &self.consumer)
                    }) {
                        Ok(members) => structured_kind(&members),
                        Err(failure) => {
                            self.issues.record_failure(
                                IssueIdentity::Members(anonymous.members),
                                failure,
                            )?;
                            return Ok(TypeRef::Omitted {
                                omitted: reason,
                                kind: TypeKind::Unsupported,
                                summary: "unsupported".into(),
                                summary_truncated: false,
                            });
                        }
                    },
                }
            }
            TypeView::Union(_) => TypeKind::Union,
            TypeView::Intersection(_) => TypeKind::Intersection,
            TypeView::TypeParameter(_) => TypeKind::TypeParameter,
            TypeView::IndexedAccess(_) => TypeKind::Operator,
            TypeView::Conditional(_) | TypeView::Substitution(_) => TypeKind::Operator,
            TypeView::Index(_) | TypeView::TemplateLiteral(_) | TypeView::StringMapping(_) => {
                TypeKind::Operator
            }
            TypeView::Reference(reference) => {
                match reference_kind(self.evaluator, &reference, &self.consumer) {
                    Ok(kind) => kind,
                    Err(failure) => {
                        self.issues
                            .record_failure(IssueIdentity::Members(reference.members), failure)?;
                        return Ok(TypeRef::Omitted {
                            omitted: reason,
                            kind: TypeKind::Unsupported,
                            summary: "unsupported".into(),
                            summary_truncated: false,
                        });
                    }
                }
            }
        };
        let summary = graph_summary(
            self.evaluator,
            id,
            &mut self.issues,
            &self.consumer,
            &self.literals,
        )?;
        Ok(TypeRef::Omitted {
            omitted: reason,
            kind,
            summary: summary.text,
            summary_truncated: summary.truncated,
        })
    }

    fn failed_edge(
        &mut self,
        failure: UnsupportedFailure,
        distance: usize,
    ) -> Result<TypeRef, AnalysisError> {
        let UnsupportedFailure {
            identity: id,
            issue,
        } = failure;
        self.issues.record_identity(id, issue.clone());
        if let Some(&index) = self.ids.get(&Identity::Failure(id)) {
            return Ok(TypeRef::Ref {
                r#ref: format!("t{index}"),
            });
        }
        if distance > self.depth || self.nodes.len() == MAX_NODES {
            self.mark_truncated();
            return Ok(TypeRef::Omitted {
                omitted: if distance > self.depth {
                    TypeOmissionReason::Depth
                } else {
                    TypeOmissionReason::Nodes
                },
                kind: TypeKind::Unsupported,
                summary: "unsupported".into(),
                summary_truncated: false,
            });
        }
        let index = self.nodes.len();
        let node = TypeNode {
            id: format!("t{index}"),
            summary: "unsupported".into(),
            summary_truncated: false,
            alias: None,
            type_arguments: None,
            omitted_type_arguments: None,
            omitted: None,
            data: TypeNodeData::Unsupported {
                reason: issue.reason,
                location: issue.location,
                message: issue.message,
            },
        };
        self.bytes = self
            .bytes
            .checked_add(self.sizes.encoded_size(&node)? + usize::from(index != 0))
            .ok_or_else(|| output_limit("type display byte accounting overflowed"))?;
        self.nodes.push(node);
        self.ids.insert(Identity::Failure(id), index);
        Ok(TypeRef::Ref {
            r#ref: format!("t{index}"),
        })
    }

    fn admit<T: serde::Serialize>(
        &mut self,
        value: &T,
        field: &'static str,
        total: usize,
        shown: usize,
        fields: &mut Vec<String>,
        checkpoint: Checkpoint,
    ) -> Result<bool, AnalysisError> {
        let delta = self.sizes.item_delta(value, field, total, shown, fields)?;
        if !self.fits(delta) {
            self.rollback(checkpoint);
            self.mark_truncated();
            return Ok(false);
        }
        self.spend(delta)?;
        if total == shown + 1 {
            fields.retain(|name| name != field);
        }
        Ok(true)
    }

    /// The operator record has one owner for operand/descriptor order and
    /// exact omitted counts. Fixed operand vectors are admitted atomically;
    /// binder descriptors admit a stable prefix of complete records.
    fn operator(
        &mut self,
        operator: TypeOperator,
        operands: &[TypeDemand],
        parameters: &[TypeId],
        distance: usize,
        fields: &mut Vec<String>,
    ) -> Result<TypeNodeData, AnalysisError> {
        let displayed_operands = self.operator_operand_demands(operands, distance, fields)?;
        let displayed_parameters = self.operator_parameters(parameters, distance, fields)?;
        Ok(TypeNodeData::Operator {
            operator,
            omitted_operands: remaining(operands.len(), displayed_operands.len()),
            omitted_parameters: remaining(parameters.len(), displayed_parameters.len()),
            operands: displayed_operands,
            parameters: displayed_parameters,
        })
    }

    fn operator_operand_demands(
        &mut self,
        operands: &[TypeDemand],
        distance: usize,
        fields: &mut Vec<String>,
    ) -> Result<Vec<TypeRef>, AnalysisError> {
        if operands.is_empty() {
            return Ok(Vec::new());
        }
        let preview: Vec<_> = operands
            .iter()
            .map(|&demand| self.preview_demand(demand, &mut HashMap::new()))
            .collect();
        let delta = signed_size(self.sizes.encoded_size(&preview)?)?
            - 2
            - signed_size(
                self.sizes
                    .optional_field_size("omittedOperands", Some(&operands.len()))?,
            )?
            + self.sizes.omission_delta(fields, "operands", true)?;
        if !self.fits(delta) {
            self.mark_truncated();
            return Ok(Vec::new());
        }
        let checkpoint = self.checkpoint();
        let values = operands
            .iter()
            .map(|&demand| self.edge(demand, distance + 1))
            .collect::<Result<Vec<_>, _>>()?;
        let delta = signed_size(self.sizes.encoded_size(&values)?)?
            - 2
            - signed_size(
                self.sizes
                    .optional_field_size("omittedOperands", Some(&operands.len()))?,
            )?
            + self.sizes.omission_delta(fields, "operands", true)?;
        if !self.fits(delta) {
            self.rollback(checkpoint);
            self.mark_truncated();
            return Ok(Vec::new());
        }
        self.spend(delta)?;
        fields.retain(|field| field != "operands");
        Ok(values)
    }

    fn tuple_elements(
        &mut self,
        reference: &ReferenceView,
        distance: usize,
        fields: &mut Vec<String>,
    ) -> Result<Vec<TupleElement>, AnalysisError> {
        let tuple = reference
            .tuple
            .as_ref()
            .ok_or_else(|| super::internal("tuple graph lacks target metadata"))?;
        let established = self
            .evaluator
            .reference_arguments_for_graph(reference.ty)
            .map_err(AnalysisError::from)?
            .and_then(|arguments| arguments.established);
        let mut arguments = None;
        let mut displayed = Vec::new();
        for (index, info) in tuple.elements.iter().enumerate() {
            let mut element = TupleElement {
                value_type: match established
                    .as_ref()
                    .and_then(|arguments| arguments.get(index))
                {
                    Some(&ty) => self.preview_demand(TypeDemand::Known(ty), &mut HashMap::new()),
                    None => TypeRef::Ref { r#ref: "t0".into() },
                },
                label: info
                    .label
                    .as_ref()
                    .map(|label| {
                        label
                            .name
                            .as_ref()
                            .map(|name| name.to_string())
                            .map_err(|error| AnalysisError::from(error.clone()))
                    })
                    .transpose()?,
                optional: info.flags.intersects(TupleElementFlags::OPTIONAL),
                rest: info.flags.intersects(TupleElementFlags::REST),
                variadic: info.flags.intersects(TupleElementFlags::VARIADIC),
            };
            if !self.fits(self.sizes.item_delta(
                &element,
                "elements",
                tuple.elements.len(),
                displayed.len(),
                fields,
            )?) {
                self.mark_truncated();
                break;
            }
            let checkpoint = self.checkpoint();
            if arguments.is_none() {
                arguments = Some(
                    self.evaluator
                        .get_element_types(reference.ty)
                        .map_err(AnalysisError::from)?,
                );
            }
            let ty = *arguments
                .as_ref()
                .and_then(|arguments| arguments.get(index))
                .ok_or_else(|| {
                    super::internal("tuple argument count differs from its target element count")
                })?;
            element.value_type = self.edge(TypeDemand::Known(ty), distance + 1)?;
            if !self.admit(
                &element,
                "elements",
                tuple.elements.len(),
                displayed.len(),
                fields,
                checkpoint,
            )? {
                break;
            }
            displayed.push(element);
        }
        Ok(displayed)
    }

    /// Conditional/mapped binder descriptors are metadata, while their
    /// constraints and defaults are ordinary depth edges. Admit each complete
    /// record before acquiring either child, using canonical parameter types
    /// supplied by the operator owner instead of rediscovering syntax here.
    fn operator_parameters(
        &mut self,
        parameters: &[TypeId],
        distance: usize,
        fields: &mut Vec<String>,
    ) -> Result<Vec<TypeParameter>, AnalysisError> {
        let mut displayed = Vec::new();
        for &ty in parameters {
            let TypeView::TypeParameter(parameter) = self.evaluator.describe(ty)? else {
                return Err(super::internal("operator binder is not a type parameter"));
            };
            let inferred = parameter.constraint.is_none()
                && self.evaluator.parameter_may_have_constraint(ty)?;
            let mut pending = HashMap::new();
            let preview = parameter_record(&parameter, &mut |demand| {
                Ok(self.preview_demand(demand, &mut pending))
            })?;
            // An inferred constraint may resolve absent. Its true metadata
            // lower bound has no constraint field; final record admission
            // accounts for an acquired edge and rolls back if it cannot fit.
            if !self.fits(self.sizes.item_delta(
                &preview,
                "parameters",
                parameters.len(),
                displayed.len(),
                fields,
            )?) {
                self.mark_truncated();
                break;
            }
            let checkpoint = self.checkpoint();
            // An inferred constraint has no explicit deferred edge in the
            // descriptor. Acquire it before defaults, just as an explicit one.
            let constraint = if inferred {
                self.inferred_constraint_edge(ty, distance + 1)?
            } else {
                None
            };
            let mut record =
                parameter_record(&parameter, &mut |demand| self.edge(demand, distance + 1))?;
            if inferred {
                record.constraint = constraint;
            }
            if !self.admit(
                &record,
                "parameters",
                parameters.len(),
                displayed.len(),
                fields,
                checkpoint,
            )? {
                break;
            }
            displayed.push(record);
        }
        Ok(displayed)
    }

    fn alternatives(
        &mut self,
        alternatives: &[TypeId],
        intersection: bool,
        distance: usize,
        fields: &mut Vec<String>,
    ) -> Result<TypeNodeData, AnalysisError> {
        let mut displayed = Vec::new();
        for &id in alternatives {
            // A new reference cannot encode fewer bytes than this identity
            // preview. Reject it before demanding child metadata when even
            // that lower bound cannot fit the remaining collection budget.
            let index = self
                .ids
                .get(&Identity::Type(id))
                .copied()
                .unwrap_or(self.nodes.len());
            let preview = TypeRef::Ref {
                r#ref: format!("t{index}"),
            };
            if !self.fits(self.sizes.item_delta(
                &preview,
                "alternatives",
                alternatives.len(),
                displayed.len(),
                fields,
            )?) {
                self.mark_truncated();
                break;
            }
            let checkpoint = self.checkpoint();
            let reference = self.edge(TypeDemand::Known(id), distance + 1)?;
            if !self.admit(
                &reference,
                "alternatives",
                alternatives.len(),
                displayed.len(),
                fields,
                checkpoint,
            )? {
                break;
            }
            displayed.push(reference);
        }
        let omitted_alternatives = remaining(alternatives.len(), displayed.len());
        Ok(if intersection {
            TypeNodeData::Intersection {
                alternatives: displayed,
                omitted_alternatives,
            }
        } else {
            TypeNodeData::Union {
                alternatives: displayed,
                omitted_alternatives,
            }
        })
    }

    fn parameter_constraint_child(
        &mut self,
        ty: TypeId,
        known: Option<TypeDemand>,
        distance: usize,
        fields: &mut Vec<String>,
    ) -> Result<Option<TypeRef>, AnalysisError> {
        if known.is_some()
            || !self
                .evaluator
                .parameter_may_have_constraint(ty)
                .map_err(AnalysisError::from)?
        {
            return self.optional_child("constraint", known, distance, fields);
        }
        let preview = TypeRef::Ref { r#ref: "t0".into() };
        let minimum = signed_size(
            self.sizes
                .optional_field_size("constraint", Some(&preview))?,
        )? + self.sizes.omission_delta(fields, "constraint", true)?;
        if !self.fits(minimum) {
            self.mark_truncated();
            return Ok(None);
        }
        let checkpoint = self.checkpoint();
        let value = self.inferred_constraint_edge(ty, distance)?;
        let delta = signed_size(
            self.sizes
                .optional_field_size("constraint", value.as_ref())?,
        )? + self.sizes.omission_delta(fields, "constraint", true)?;
        if !self.fits(delta) {
            self.rollback(checkpoint);
            self.mark_truncated();
            return Ok(None);
        }
        self.spend(delta)?;
        fields.retain(|field| field != "constraint");
        Ok(value)
    }

    /// Ask an admitted inferred constraint once. The containing field or
    /// parameter record owns byte admission; this operation owns the edge and
    /// preserves the original failed property question through that wrapper.
    fn inferred_constraint_edge(
        &mut self,
        ty: TypeId,
        distance: usize,
    ) -> Result<Option<TypeRef>, AnalysisError> {
        let consumer = self.evaluator.parameter_constraint_location(ty)?.clone();
        match self.evaluator.parameter_constraint(ty, &consumer) {
            Ok(Some(constraint)) => self.edge(TypeDemand::Known(constraint), distance).map(Some),
            Ok(None) => Ok(None),
            Err(failure) => {
                let failure = unsupported_failure(IssueIdentity::ParameterConstraint(ty), failure)?;
                self.failed_edge(failure, distance).map(Some)
            }
        }
    }

    // Admit the exact optional-field metadata before the child question. A
    // demanded unsupported cause survives display rollback through Issues.
    fn optional_child(
        &mut self,
        field: &str,
        demand: Option<TypeDemand>,
        distance: usize,
        fields: &mut Vec<String>,
    ) -> Result<Option<TypeRef>, AnalysisError> {
        let Some(demand) = demand else {
            return Ok(None);
        };
        let preview = self.preview_demand(demand, &mut HashMap::new());
        let minimum = signed_size(self.sizes.optional_field_size(field, Some(&preview))?)?
            + self.sizes.omission_delta(fields, field, true)?;
        if !self.fits(minimum) {
            self.mark_truncated();
            return Ok(None);
        }
        let checkpoint = self.checkpoint();
        let value = self.edge(demand, distance)?;
        let delta = signed_size(self.sizes.optional_field_size(field, Some(&value))?)?
            + self.sizes.omission_delta(fields, field, true)?;
        if !self.fits(delta) {
            self.rollback(checkpoint);
            self.mark_truncated();
            return Ok(None);
        }
        self.spend(delta)?;
        fields.retain(|name| name != field);
        Ok(Some(value))
    }

    // Required edges already reserve a real omitted descriptor in the node
    // skeleton. Admission replaces that descriptor; it cannot account for the
    // field as though it were absent or demand the child before metadata fits.
    fn required_child(
        &mut self,
        field: &str,
        demand: TypeDemand,
        before: &TypeRef,
        distance: usize,
        fields: &mut Vec<String>,
    ) -> Result<TypeRef, AnalysisError> {
        let preview = self.preview_demand(demand, &mut HashMap::new());
        let delta =
            |sizes: &WireSize,
             value: &TypeRef,
             fields: &[String]|
             -> Result<isize, AnalysisError> {
                Ok(signed_size(sizes.encoded_size(value)?)?
                    - signed_size(sizes.encoded_size(before)?)?
                    + sizes.omission_delta(fields, field, true)?)
            };
        if !self.fits(delta(&self.sizes, &preview, fields)?) {
            self.mark_truncated();
            return Ok(before.clone());
        }
        let checkpoint = self.checkpoint();
        let value = self.edge(demand, distance)?;
        let delta = delta(&self.sizes, &value, fields)?;
        if !self.fits(delta) {
            self.rollback(checkpoint);
            self.mark_truncated();
            return Ok(before.clone());
        }
        self.spend(delta)?;
        fields.retain(|name| name != field);
        Ok(value)
    }

    /// A real reference owns one whole `resolveStructuredTypeMembers`
    /// question. Only its display header lower bound is inspected before that
    /// demand; exact counts and child records are admitted after it succeeds.
    fn reference_members(
        &mut self,
        question: MembersOfType,
        distance: usize,
        fields: &mut Vec<String>,
    ) -> Result<TypeMembers, AnalysisError> {
        let before = TypeMembers::default();
        let mut established_fields = without_member_fields(fields);
        let minimum = TypeMembers {
            properties: Some(Vec::new()),
            call_signatures: Some(Vec::new()),
            construct_signatures: Some(Vec::new()),
            index_signatures: Some(Vec::new()),
            ..TypeMembers::default()
        };
        if !self.fits(self.sizes.member_metadata_delta(
            &before,
            &minimum,
            fields,
            &established_fields,
        )?) {
            self.mark_truncated();
            return Ok(before);
        }
        let members = match self.evaluator.members_at(question, &self.consumer) {
            Ok(members) => members,
            Err(failure) => {
                let issue = self
                    .issues
                    .record_failure(IssueIdentity::Members(question), failure)?;
                let blocked = TypeMembers {
                    blocked_collections: Some(
                        MEMBER_COLLECTIONS
                            .iter()
                            .map(|(_, field)| TypeBlockedCollection {
                                field: *field,
                                issue: issue.clone(),
                            })
                            .collect(),
                    ),
                    ..TypeMembers::default()
                };
                let delta = self.sizes.member_metadata_delta(
                    &before,
                    &blocked,
                    fields,
                    &established_fields,
                )?;
                if !self.fits(delta) {
                    self.mark_truncated();
                    return Ok(before);
                }
                self.spend(delta)?;
                *fields = established_fields;
                return Ok(blocked);
            }
        };
        let skeleton = skeletal_members(&members, false, &mut established_fields);
        let delta =
            self.sizes
                .member_metadata_delta(&before, &skeleton, fields, &established_fields)?;
        if !self.fits(delta) {
            self.mark_truncated();
            return Ok(before);
        }
        self.spend(delta)?;
        *fields = established_fields;
        self.members(&members, distance, fields)
    }

    fn members(
        &mut self,
        members: &MembersView,
        distance: usize,
        fields: &mut Vec<String>,
    ) -> Result<TypeMembers, AnalysisError> {
        let mut result = skeletal_members(members, false, &mut Vec::new());
        let properties = result
            .properties
            .as_mut()
            .expect("an admitted collection has a prefix");
        for property in members.properties.iter() {
            // The native write question can resolve to the exact read identity,
            // so a possible write field is not a mandatory metadata lower bound.
            let preview = self.preview_edges(&[property.read_type]);

            let make = |read_type, write_type| TypeProperty {
                key: property.key_snapshot(),
                kind: property.kind,
                read_type,
                write_type,
                optional: property.optional,
                readonly: property.readonly,
                accessibility: property.accessibility,
                origins: property.origins_snapshot(),
            };
            let preview = make(preview[0].clone(), None);
            // Identity metadata alone can reject a field before its child is
            // demanded. A semantic failure actually encountered later remains
            // a failure, rather than becoming a display omission.
            if !self.fits(self.sizes.item_delta(
                &preview,
                "properties",
                members.properties.len(),
                properties.len(),
                fields,
            )?) {
                self.mark_truncated();
                break;
            }
            let checkpoint = self.checkpoint();
            let read = self.edge(property.read_type, distance + 1)?;
            let read_identity = self.demand_identity(property.read_type);
            let write = if property.readonly {
                None
            } else if let Some(write) = property.write_type {
                if write == property.read_type {
                    None
                } else {
                    let result = self.evaluator.demand(write);
                    match result {
                        Ok(id) if Some(Identity::Type(id)) == read_identity => None,
                        result => {
                            let edge = self.edge_result(write, result, distance + 1)?;
                            // A write may fail through the same original read
                            // question. Preserve its consumed issue while using
                            // one canonical failure edge, just as for a shared
                            // successful type; display text never decides this.
                            if self
                                .demand_identity(write)
                                .is_some_and(|identity| Some(identity) == read_identity)
                            {
                                None
                            } else {
                                Some(edge)
                            }
                        }
                    }
                }
            } else {
                None
            };
            let value = make(read, write);
            if !self.admit(
                &value,
                "properties",
                members.properties.len(),
                properties.len(),
                fields,
                checkpoint,
            )? {
                break;
            }
            properties.push(value);
        }
        result.omitted_properties = remaining(members.properties.len(), properties.len());
        for (field, collection, source, construct) in [
            (
                "callSignatures",
                TypeCollection::CallSignatures,
                &members.call_signatures,
                false,
            ),
            (
                "constructSignatures",
                TypeCollection::ConstructSignatures,
                &members.construct_signatures,
                true,
            ),
        ] {
            let before = result.clone();
            let before_fields = fields.clone();
            let collection_checkpoint = self.checkpoint();
            let mut values = Vec::new();
            let mut blocked = None;
            for signature in source.iter() {
                let mut pending = HashMap::new();

                let preview = signature_record(
                    signature,
                    self.evaluator.signature_origins_snapshot(signature),
                    |demand| Ok(self.preview_demand(demand.raw(), &mut pending)),
                )?;
                if !self.fits(self.sizes.item_delta(
                    &preview,
                    field,
                    source.len(),
                    values.len(),
                    fields,
                )?) {
                    self.mark_truncated();
                    break;
                }
                let checkpoint = self.checkpoint();
                let value = match self.signature(signature, distance + 1)? {
                    SignatureRecord::Ready(value) => value,
                    SignatureRecord::Blocked(issue) => {
                        blocked = Some(issue);
                        break;
                    }
                };
                if !self.admit(
                    &value,
                    field,
                    source.len(),
                    values.len(),
                    fields,
                    checkpoint,
                )? {
                    break;
                }
                values.push(value);
            }
            if let Some(issue) = blocked {
                self.rollback(collection_checkpoint);
                *fields = before_fields.clone();
                if construct {
                    result.construct_signatures = None;
                    result.omitted_construct_signatures = None;
                } else {
                    result.call_signatures = None;
                    result.omitted_call_signatures = None;
                }
                result
                    .blocked_collections
                    .get_or_insert_with(Vec::new)
                    .push(TypeBlockedCollection {
                        field: collection,
                        issue,
                    });
                fields.retain(|name| name != field);
                let delta =
                    self.sizes
                        .member_metadata_delta(&before, &result, &before_fields, fields)?;
                if self.fits(delta) {
                    self.spend(delta)?;
                } else {
                    result = before;
                    *fields = before_fields;
                    self.mark_truncated();
                }
            } else {
                let omitted = remaining(source.len(), values.len());
                if construct {
                    result.construct_signatures = Some(values);
                    result.omitted_construct_signatures = omitted;
                } else {
                    result.call_signatures = Some(values);
                    result.omitted_call_signatures = omitted;
                }
            }
        }
        let indexes = result
            .index_signatures
            .as_mut()
            .expect("an admitted collection has a prefix");
        for index in members.index_signatures.iter() {
            let mut pending = HashMap::new();
            let preview =
                index_record(
                    index,
                    |demand| Ok(self.preview_demand(demand, &mut pending)),
                )?;
            if !self.fits(self.sizes.item_delta(
                &preview,
                "indexSignatures",
                members.index_signatures.len(),
                indexes.len(),
                fields,
            )?) {
                self.mark_truncated();
                break;
            }
            let checkpoint = self.checkpoint();
            let value = index_record(index, |demand| self.edge(demand, distance + 1))?;
            if !self.admit(
                &value,
                "indexSignatures",
                members.index_signatures.len(),
                indexes.len(),
                fields,
                checkpoint,
            )? {
                break;
            }
            indexes.push(value);
        }
        result.omitted_index_signatures = remaining(members.index_signatures.len(), indexes.len());
        Ok(result)
    }

    fn signature(
        &mut self,
        signature: &Signature,
        distance: usize,
    ) -> Result<SignatureRecord, AnalysisError> {
        // The graph's required return edge precedes optional predicate metadata.
        // Its preview uses only established descriptor facts and makes no query.
        let mut unresolved = signature.clone();
        unresolved.predicate = None;

        let mut record = signature_record(
            &unresolved,
            self.evaluator.signature_origins_snapshot(&unresolved),
            |child| match child {
                SignatureChild::Type(demand) => self.edge(demand, distance),
                SignatureChild::Return(signature) => {
                    let result = self
                        .evaluator
                        .get_return_type_of_signature(signature, &self.consumer);
                    self.edge_result(signature.return_type, result, distance)
                }
            },
        )?;
        let consumer = self.consumer.clone();
        let predicate = match self
            .evaluator
            .type_predicate_of_signature(signature, &consumer)
        {
            Ok(predicate) => predicate,
            Err(failure) => {
                return Ok(SignatureRecord::Blocked(self.issues.record_failure(
                    IssueIdentity::SignaturePredicate(signature.id),
                    failure,
                )?));
            }
        };
        record.predicate = predicate
            .as_ref()
            .map(|predicate| {
                Ok::<_, AnalysisError>(TypePredicate {
                    kind: predicate.kind,
                    subject: predicate.subject,
                    value_type: predicate
                        .value_type
                        .map(|demand| self.edge(demand, distance))
                        .transpose()?,
                })
            })
            .transpose()?;
        Ok(SignatureRecord::Ready(record))
    }
}

enum SignatureRecord {
    Ready(TypeSignature),
    Blocked(TypeIssue),
}

fn parameter_record(
    parameter: &Parameter,
    reference: &mut impl FnMut(TypeDemand) -> Result<TypeRef, AnalysisError>,
) -> Result<TypeParameter, AnalysisError> {
    Ok(TypeParameter {
        name: parameter.copy_name(),
        constraint: parameter.constraint.map(&mut *reference).transpose()?,
        default: parameter.default.map(reference).transpose()?,
        variance: parameter
            .variance
            .map(|variance| variance.as_str().to_owned()),
    })
}

fn parameter_value_record(
    parameter: &ParameterValue,
    reference: &mut impl FnMut(TypeDemand) -> Result<TypeRef, AnalysisError>,
) -> Result<TypeParameterValue, AnalysisError> {
    let name = {
        let mut name = String::new();
        name.push_str(&parameter.name);
        name
    };
    Ok(TypeParameterValue {
        name,
        value_type: reference(parameter.value_type)?,
        optional: parameter.optional,
        rest: parameter.rest,
    })
}

/// The return child is a semantic operation, not an arbitrary demand ID.
/// Keeping its role in the record traversal lets previews stay query-free.
#[derive(Clone, Copy)]
enum SignatureChild<'a> {
    Type(TypeDemand),
    Return(&'a Signature),
}
impl SignatureChild<'_> {
    fn raw(self) -> TypeDemand {
        match self {
            Self::Type(demand) => demand,
            Self::Return(signature) => signature.return_type,
        }
    }
}

/// Preview and demanded records share one field/child traversal owner. A
/// preview only substitutes lower-bound references; it never checks a child.
fn signature_record<'a>(
    signature: &'a Signature,
    origins: Vec<Location>,
    mut reference: impl FnMut(SignatureChild<'a>) -> Result<TypeRef, AnalysisError>,
) -> Result<TypeSignature, AnalysisError> {
    let type_parameters = signature
        .type_parameters()
        .iter()
        .map(|parameter| {
            parameter_record(parameter, &mut |demand| {
                reference(SignatureChild::Type(demand))
            })
        })
        .collect::<Result<_, _>>()?;
    let this_parameter = signature
        .this_parameter()
        .as_ref()
        .map(|parameter| {
            parameter_value_record(parameter, &mut |demand| {
                reference(SignatureChild::Type(demand))
            })
        })
        .transpose()?;
    let mut parameters = Vec::new();
    {
        parameters.reserve(signature.parameters.len());
    }
    for parameter in signature.parameters.iter() {
        parameters.push(parameter_value_record(parameter, &mut |demand| {
            reference(SignatureChild::Type(demand))
        })?);
    }
    let return_type = reference(SignatureChild::Return(signature))?;
    let predicate = signature
        .predicate
        .as_ref()
        .map(|predicate| {
            Ok::<_, AnalysisError>(TypePredicate {
                kind: predicate.kind,
                subject: predicate.subject,
                value_type: predicate
                    .value_type
                    .map(|demand| reference(SignatureChild::Type(demand)))
                    .transpose()?,
            })
        })
        .transpose()?;
    Ok(TypeSignature {
        type_parameters,
        this_parameter,
        parameters,
        return_type,
        predicate,
        origins,
    })
}

fn index_record(
    index: &Index,
    mut reference: impl FnMut(TypeDemand) -> Result<TypeRef, AnalysisError>,
) -> Result<TypeIndex, AnalysisError> {
    Ok(TypeIndex {
        key_type: reference(index.key_type)?,
        value_type: reference(index.value_type)?,
        readonly: index.readonly,
    })
}

fn remaining(total: usize, shown: usize) -> Option<usize> {
    (total > shown).then_some(total - shown)
}

fn operator_skeleton(
    operator: TypeOperator,
    operands: usize,
    parameters: usize,
    fields: &mut Vec<String>,
) -> TypeNodeData {
    if operands != 0 {
        fields.push("operands".into());
    }
    if parameters != 0 {
        fields.push("parameters".into());
    }
    TypeNodeData::Operator {
        operator,
        operands: Vec::new(),
        parameters: Vec::new(),
        omitted_operands: remaining(operands, 0),
        omitted_parameters: remaining(parameters, 0),
    }
}

fn deferred_members(fields: &mut Vec<String>) -> TypeMembers {
    fields.extend(MEMBER_COLLECTIONS.iter().map(|(name, _)| (*name).into()));
    TypeMembers::default()
}

fn without_member_fields(fields: &[String]) -> Vec<String> {
    fields
        .iter()
        .filter(|name| {
            !MEMBER_COLLECTIONS
                .iter()
                .any(|(field, _)| name.as_str() == *field)
        })
        .cloned()
        .collect()
}

fn skeletal_members(
    members: &MembersView,
    at_depth: bool,
    fields: &mut Vec<String>,
) -> TypeMembers {
    for (name, count) in [
        ("properties", members.properties.len()),
        ("callSignatures", members.call_signatures.len()),
        ("constructSignatures", members.construct_signatures.len()),
        ("indexSignatures", members.index_signatures.len()),
    ] {
        if count != 0 {
            fields.push(name.into());
        }
    }
    TypeMembers {
        properties: (!at_depth || members.properties.is_empty()).then(Vec::new),
        call_signatures: (!at_depth || members.call_signatures.is_empty()).then(Vec::new),
        construct_signatures: (!at_depth || members.construct_signatures.is_empty()).then(Vec::new),
        index_signatures: (!at_depth || members.index_signatures.is_empty()).then(Vec::new),
        omitted_properties: (!at_depth)
            .then(|| remaining(members.properties.len(), 0))
            .flatten(),
        omitted_call_signatures: (!at_depth)
            .then(|| remaining(members.call_signatures.len(), 0))
            .flatten(),
        omitted_construct_signatures: (!at_depth)
            .then(|| remaining(members.construct_signatures.len(), 0))
            .flatten(),
        omitted_index_signatures: (!at_depth)
            .then(|| remaining(members.index_signatures.len(), 0))
            .flatten(),
        ..TypeMembers::default()
    }
}

fn signed_size(bytes: usize) -> Result<isize, AnalysisError> {
    isize::try_from(bytes)
        .map_err(|_| output_limit("type display size exceeds byte accounting capacity"))
}

fn output_limit(message: &str) -> AnalysisError {
    SemanticError {
        kind: SemanticErrorKind::OutputLimit,
        reason: SemanticResourceReason::OutputLimit.into(),
        message: message.into(),
        path: None,
        position: None,
        exact_location: None,
        unbound_recovery: None,
        causes: Vec::new(),
        omitted_causes: None,
        cause_kinds: Default::default(),
    }
    .into()
}

/// Exact public JSON size uses the wire serializer itself. Counting its writes
/// avoids allocating a discarded encoded copy. This is product display policy, not
/// a second implementation of compiler spelling or JSON escaping rules.
struct WireSize;
impl WireSize {
    /// Member fields flatten into a data record that already has required fields.
    /// A nonempty serialized member object therefore contributes its field bytes
    /// and a leading comma, while an empty object contributes no bytes.
    fn flattened_members_size(&self, members: &TypeMembers) -> Result<usize, AnalysisError> {
        let bytes = self.encoded_size(members)?;
        Ok(if bytes == 2 { 0 } else { bytes - 1 })
    }

    fn member_metadata_delta(
        &self,
        before: &TypeMembers,
        after: &TypeMembers,
        before_fields: &[String],
        after_fields: &[String],
    ) -> Result<isize, AnalysisError> {
        let omitted_size = |fields: &[String]| {
            let omitted = (!fields.is_empty()).then(|| TypeFieldsOmitted {
                reason: TypeOmissionReason::Bytes,
                fields: fields.to_vec(),
            });
            self.optional_field_size("omitted", omitted.as_ref())
        };
        Ok(signed_size(self.flattened_members_size(after)?)?
            - signed_size(self.flattened_members_size(before)?)?
            + signed_size(omitted_size(after_fields)?)?
            - signed_size(omitted_size(before_fields)?)?)
    }

    fn item_delta(
        &self,
        value: &impl serde::Serialize,
        field: &str,
        total: usize,
        shown: usize,
        fields: &[String],
    ) -> Result<isize, AnalysisError> {
        let count_field = match field {
            "properties" => Some("omittedProperties"),
            "callSignatures" => Some("omittedCallSignatures"),
            "constructSignatures" => Some("omittedConstructSignatures"),
            "indexSignatures" => Some("omittedIndexSignatures"),
            "alternatives" => Some("omittedAlternatives"),
            "elements" => Some("omittedElements"),
            "parameters" => Some("omittedParameters"),
            _ => return Err(super::internal("unknown type collection display field")),
        };
        let before = total - shown;
        let after = before - 1;
        let count_delta = if let Some(count_field) = count_field {
            signed_size(
                self.optional_field_size(count_field, remaining(total, shown + 1).as_ref())?,
            )? - signed_size(self.optional_field_size(count_field, Some(&before))?)?
        } else {
            0
        };
        Ok(
            signed_size(self.encoded_size(value)? + usize::from(shown != 0))?
                + count_delta
                + self.omission_delta(fields, field, after == 0)?,
        )
    }

    fn omission_delta(
        &self,
        fields: &[String],
        field: &str,
        removes: bool,
    ) -> Result<isize, AnalysisError> {
        if !removes {
            return Ok(0);
        }
        let before = TypeFieldsOmitted {
            reason: TypeOmissionReason::Bytes,
            fields: fields.to_vec(),
        };
        let after_fields: Vec<_> = fields
            .iter()
            .filter(|name| *name != field)
            .cloned()
            .collect();
        let after = (!after_fields.is_empty()).then_some(TypeFieldsOmitted {
            reason: TypeOmissionReason::Bytes,
            fields: after_fields,
        });
        Ok(
            signed_size(self.optional_field_size("omitted", after.as_ref())?)?
                - signed_size(self.optional_field_size("omitted", Some(&before))?)?,
        )
    }

    fn optional_field_size<T: serde::Serialize>(
        &self,
        name: &str,
        value: Option<&T>,
    ) -> Result<usize, AnalysisError> {
        value
            .map(|value| Ok(self.encoded_size(name)? + 2 + self.encoded_size(value)?))
            .transpose()
            .map(|bytes| bytes.unwrap_or(0))
    }

    fn encoded_size<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
    ) -> Result<usize, AnalysisError> {
        let mut sink = SizeSink { bytes: 0 };
        serde_json::to_writer(&mut sink, value)
            .map_err(|_| super::internal("cannot encode an established type description"))?;
        Ok(sink.bytes)
    }
}

struct SizeSink {
    bytes: usize,
}
impl std::io::Write for SizeSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self.bytes.checked_add(bytes.len()).ok_or_else(|| {
            std::io::Error::other("type display size exceeds accounting capacity")
        })?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
