//! Text and JSON layout of the same bounded type description. Semantic
//! selection, graph traversal and display budgets belong to the library.

use super::delivery::Delivery;
use std::{collections::HashMap, fmt::Write, io};
use typescript_parser::types::{
    SelectionFailure, TypeAnswer, TypeDescription, TypeKey, TypeMembers, TypeNodeData, TypeOutcome,
    TypeParameter, TypeRef, TypeResult, TypeSignature,
};

pub(crate) fn answer(answer: &TypeAnswer) -> Delivery<'_> {
    let code = match answer {
        TypeAnswer::Selected(result) => match result.outcome {
            TypeOutcome::Found => 0,
            TypeOutcome::Partial => 1,
        },
        TypeAnswer::Unselected(_) => 1,
    };
    Delivery {
        code,
        text_is_result: matches!(answer, TypeAnswer::Selected(_)),
        document: Box::new(move |writer, pretty| {
            if pretty {
                serde_json::to_writer_pretty(writer, answer)
            } else {
                serde_json::to_writer(writer, answer)
            }
            .map_err(io::Error::other)
        }),
        text: Some(Box::new(move || match answer {
            TypeAnswer::Selected(result) => text(result),
            TypeAnswer::Unselected(selection) => match &selection.outcome {
                SelectionFailure::NotFound { .. } => format!(
                    "{}: no type target matches the selection",
                    selection.project.display()
                ),
                SelectionFailure::Ambiguous { candidates } => {
                    let mut text = format!(
                        "{}: selection matches {} type targets; select one with --at FILE:LINE:COLUMN",
                        selection.project.display(),
                        candidates.len()
                    );
                    for candidate in candidates {
                        let anchor = &candidate.anchor;
                        write!(
                            text,
                            "\n  {}:{}:{} {}",
                            super::navigation::place(Some(&selection.project), &anchor.file),
                            anchor.line,
                            anchor.column,
                            candidate.selector.as_deref().unwrap_or("(position only)")
                        )
                        .unwrap();
                    }
                    text
                }
            },
        })),
    }
}

/// Render already admitted graph records in their document order. A graph ID
/// labels shared/cyclic edges; layout never follows them recursively or asks
/// the semantic model to fill an omitted field.
fn text(result: &TypeResult) -> String {
    let header = match result.outcome {
        TypeOutcome::Found => "TYPE",
        TypeOutcome::Partial => "PARTIAL TYPE — unsupported details",
    };
    let location = &result.target.location;
    let mut text = format!(
        "{header}\n{}:{}:{}\n{}",
        super::navigation::place(Some(&result.project), &location.file),
        location.start.line,
        location.start.column,
        result.description.summary
    );
    structure(&mut text, &result.description);
    for issue in &result.issues {
        write!(
            text,
            "\nUnsupported at {}:{}:{}: {}",
            super::navigation::place(Some(&result.project), &issue.location.file),
            issue.location.start.line,
            issue.location.start.column,
            issue.message
        )
        .unwrap();
    }
    text
}

fn structure(text: &mut String, description: &TypeDescription) {
    let Some(graph) = &description.expansion else {
        if description.summary_truncated {
            text.push_str("\nSummary omitted details.");
        }
        return;
    };
    let summaries: HashMap<_, _> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.summary.as_str()))
        .collect();
    for node in &graph.nodes {
        if node.id != graph.root {
            write!(text, "\n{}: {}", node.id, node.summary).unwrap();
        }
        if let Some(alias) = &node.alias {
            write!(text, "\n  alias {}", quoted(&alias.name)).unwrap();
        }
        if let Some(arguments) = &node.type_arguments {
            for argument in arguments {
                write!(
                    text,
                    "\n  type argument: {}",
                    reference(argument, &summaries)
                )
                .unwrap();
            }
        }
        match &node.data {
            TypeNodeData::Object { members } | TypeNodeData::Function { members } => {
                members_text(text, members, &summaries)
            }
            TypeNodeData::Array {
                element,
                readonly,
                members,
            } => {
                write!(
                    text,
                    "\n  {}element: {}",
                    if *readonly { "readonly " } else { "" },
                    reference(element, &summaries)
                )
                .unwrap();
                members_text(text, members, &summaries);
            }
            TypeNodeData::Tuple {
                elements,
                omitted_elements,
                readonly,
                members,
            } => {
                if *readonly {
                    text.push_str("\n  readonly tuple");
                }
                for (index, element) in elements.iter().enumerate() {
                    write!(
                        text,
                        "\n  element {index}{}{}{}{}: {}",
                        element
                            .label
                            .as_ref()
                            .map(|name| format!(" {}", quoted(name)))
                            .unwrap_or_default(),
                        if element.optional { " optional" } else { "" },
                        if element.rest { " rest" } else { "" },
                        if element.variadic { " variadic" } else { "" },
                        reference(&element.value_type, &summaries)
                    )
                    .unwrap();
                }
                omitted_count(text, "elements", *omitted_elements);
                members_text(text, members, &summaries);
            }
            TypeNodeData::Union {
                alternatives,
                omitted_alternatives,
            }
            | TypeNodeData::Intersection {
                alternatives,
                omitted_alternatives,
            } => {
                for alternative in alternatives {
                    write!(
                        text,
                        "\n  alternative: {}",
                        reference(alternative, &summaries)
                    )
                    .unwrap();
                }
                omitted_count(text, "alternatives", *omitted_alternatives);
            }
            TypeNodeData::TypeParameter {
                name,
                constraint,
                default,
            } => parameter_text(
                text,
                name,
                constraint.as_ref(),
                default.as_ref(),
                None,
                &summaries,
            ),
            TypeNodeData::Operator {
                operator,
                operands,
                parameters,
                omitted_operands,
                omitted_parameters,
            } => {
                write!(text, "\n  operator {operator:?}").unwrap();
                for operand in operands {
                    write!(text, "\n  operand: {}", reference(operand, &summaries)).unwrap();
                }
                for parameter in parameters {
                    type_parameter_text(text, parameter, &summaries);
                }
                omitted_count(text, "operands", *omitted_operands);
                omitted_count(text, "parameters", *omitted_parameters);
            }
            TypeNodeData::Unsupported { message, .. } => {
                write!(text, "\n  unsupported: {message}").unwrap();
            }
            TypeNodeData::Any
            | TypeNodeData::Unknown
            | TypeNodeData::Never
            | TypeNodeData::Void
            | TypeNodeData::Undefined
            | TypeNodeData::Null
            | TypeNodeData::Primitive { .. }
            | TypeNodeData::Literal { .. }
            | TypeNodeData::UniqueSymbol { .. }
            | TypeNodeData::Enum { .. } => {}
        }
        omitted_count(text, "type arguments", node.omitted_type_arguments);
        if let Some(omitted) = &node.omitted {
            write!(
                text,
                "\n  omitted ({:?}): {}",
                omitted.reason,
                omitted.fields.join(", ")
            )
            .unwrap();
        }
        if node.summary_truncated {
            text.push_str("\n  summary omitted details");
        }
    }
    if graph.truncated {
        text.push_str("\nDisplay omitted details; see the marked fields and edges.");
    }
}

fn quoted(text: &str) -> String {
    // JSON quotes preserve every exact key while escaping control characters.
    // Layout needs no second TypeScript identifier-character policy.
    serde_json::to_string(text).expect("serializing a string cannot fail")
}

fn reference(value: &TypeRef, summaries: &HashMap<&str, &str>) -> String {
    match value {
        TypeRef::Ref { r#ref: id } => format!(
            "{} ({id})",
            summaries
                .get(id.as_str())
                .expect("admitted graph edges address a node")
        ),
        TypeRef::Omitted {
            omitted, summary, ..
        } => format!("{summary} [{omitted:?} omitted]"),
    }
}

fn members_text(text: &mut String, members: &TypeMembers, summaries: &HashMap<&str, &str>) {
    if let Some(properties) = &members.properties {
        for property in properties {
            let key = match &property.key {
                TypeKey::String { name } => quoted(name),
                TypeKey::Private { name, .. } => format!("private {}", quoted(name)),
                TypeKey::Symbol { name, .. } => format!("symbol {}", quoted(name)),
            };
            write!(
                text,
                "\n  {}{}{}: {}",
                if property.readonly { "readonly " } else { "" },
                key,
                if property.optional { "?" } else { "" },
                reference(&property.read_type, summaries)
            )
            .unwrap();
            if let Some(write_type) = &property.write_type {
                write!(text, "\n    write: {}", reference(write_type, summaries)).unwrap();
            }
        }
    }
    for (label, signatures) in [
        ("call", &members.call_signatures),
        ("construct", &members.construct_signatures),
    ] {
        if let Some(signatures) = signatures {
            for signature in signatures {
                signature_text(text, label, signature, summaries);
            }
        }
    }
    if let Some(indexes) = &members.index_signatures {
        for index in indexes {
            write!(
                text,
                "\n  {}index [{}]: {}",
                if index.readonly { "readonly " } else { "" },
                reference(&index.key_type, summaries),
                reference(&index.value_type, summaries)
            )
            .unwrap();
        }
    }
    if let Some(blocked) = &members.blocked_collections {
        for collection in blocked {
            write!(
                text,
                "\n  blocked {:?}: {}",
                collection.field, collection.issue.message
            )
            .unwrap();
        }
    }
    for (field, count) in [
        ("properties", members.omitted_properties),
        ("call signatures", members.omitted_call_signatures),
        ("construct signatures", members.omitted_construct_signatures),
        ("index signatures", members.omitted_index_signatures),
    ] {
        omitted_count(text, field, count);
    }
}

fn omitted_count(text: &mut String, field: &str, count: Option<usize>) {
    if let Some(count) = count {
        write!(text, "\n  {count} more {field} omitted").unwrap();
    }
}

fn type_parameter_text(
    text: &mut String,
    parameter: &TypeParameter,
    summaries: &HashMap<&str, &str>,
) {
    parameter_text(
        text,
        &parameter.name,
        parameter.constraint.as_ref(),
        parameter.default.as_ref(),
        parameter.variance.as_deref(),
        summaries,
    );
}

fn parameter_text(
    text: &mut String,
    name: &str,
    constraint: Option<&TypeRef>,
    default: Option<&TypeRef>,
    variance: Option<&str>,
    summaries: &HashMap<&str, &str>,
) {
    write!(text, "\n  type parameter {}", quoted(name)).unwrap();
    if let Some(variance) = variance {
        write!(text, " {variance}").unwrap();
    }
    if let Some(constraint) = constraint {
        write!(text, " extends {}", reference(constraint, summaries)).unwrap();
    }
    if let Some(default) = default {
        write!(text, " = {}", reference(default, summaries)).unwrap();
    }
}

fn signature_text(
    text: &mut String,
    label: &str,
    signature: &TypeSignature,
    summaries: &HashMap<&str, &str>,
) {
    write!(
        text,
        "\n  {label} -> {}",
        reference(&signature.return_type, summaries)
    )
    .unwrap();
    for parameter in &signature.type_parameters {
        type_parameter_text(text, parameter, summaries);
    }
    if let Some(parameter) = &signature.this_parameter {
        write!(
            text,
            "\n    this: {}",
            reference(&parameter.value_type, summaries)
        )
        .unwrap();
    }
    for parameter in &signature.parameters {
        write!(
            text,
            "\n    {}{}{}: {}",
            if parameter.rest { "..." } else { "" },
            quoted(&parameter.name),
            if parameter.optional { "?" } else { "" },
            reference(&parameter.value_type, summaries)
        )
        .unwrap();
    }
    if let Some(predicate) = &signature.predicate {
        write!(
            text,
            "\n    predicate {:?} {:?}",
            predicate.kind, predicate.subject
        )
        .unwrap();
        if let Some(value_type) = &predicate.value_type {
            write!(text, ": {}", reference(value_type, summaries)).unwrap();
        }
    }
}
