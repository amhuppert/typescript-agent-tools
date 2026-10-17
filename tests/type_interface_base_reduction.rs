//! resolveBaseTypesOfInterface reduces before validating and continues after
//! acquired recovery; circular diagnostics observe the native partial base list.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-interface-base-reduction/cases.ts");

#[test]
fn interface_bases_reuse_complete_reduction_and_generic_eligibility() {
    check(0);
}
#[test]
fn interface_invalid_bases_remain_diagnostic_only() {
    check(1);
}
#[test]
fn interface_base_recovery_continues_in_actual_declaration_order() {
    check(2);
}
#[test]
fn interface_base_cycles_preserve_partial_native_state_and_display() {
    check(3);
}

fn check(group: u8) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-interface-base-reduction"),
    );
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-interface-base-reduction/expected-strict.json"),
        ),
        (
            "nocheck",
            include_str!("../fixtures/types/native-interface-base-reduction/expected-nocheck.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 25);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let selected = match name {
                    "valid" | "alias" | "generic" | "nested" | "empty" | "any" | "nonPrimitive"
                    | "propertyOwn" | "propertyNever" | "mapped" | "constrained"
                    | "reducedUnion" | "classDemand" => 0,
                    "primitive" | "union" | "discriminants" | "private" | "genericMapped"
                    | "unconstrained" => 1,
                    "errorThenValid" | "mergedRecovery" => 2,
                    "selfCycle" | "mutualCycle" | "intersectionCycle" | "defaultCycle" => 3,
                    _ => panic!("unclassified native base variant {name}"),
                };
                if selected != group {
                    continue;
                }
                assert_eq!(row["errorType"], false);
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                let expected = match name {
                    "errorThenValid" => vec![(
                        SemanticFailureReason::UnresolvedSymbol,
                        "MissingInterfaceBase",
                    )],
                    "mergedRecovery" => vec![
                        (SemanticFailureReason::UnresolvedSymbol, "MissingFirstBase"),
                        (SemanticFailureReason::UnresolvedSymbol, "MissingSecondBase"),
                    ],
                    "defaultCycle" => {
                        vec![(SemanticFailureReason::ErrorType, "DefaultDerived['prop']")]
                    }
                    _ => Vec::new(),
                };
                let native_errors: Vec<_> = row["acquisitions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|event| event["event"] == "exit" && event["errorType"] == true)
                    .collect();
                assert_eq!(native_errors.len(), expected.len());
                for (event, (_, token)) in native_errors.iter().zip(&expected) {
                    assert_eq!(event["operation"], "getTypeFromTypeNode");
                    let start = event["pos"].as_u64().unwrap() as usize;
                    let end = event["end"].as_u64().unwrap() as usize;
                    assert_eq!(SOURCE[start..end].trim_start(), *token);
                }
                let marker = format!("/*#{name}*/");
                let offset = SOURCE.find(&marker).unwrap() + marker.len();
                for expand in [false, true] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 1,
                    });
                    if expected.is_empty() {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{label}"
                        );
                        assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                        if let Some(graph) = answer.description.expansion {
                            assert!(
                                graph.nodes.iter().all(|node| !matches!(
                                    node.data,
                                    TypeNodeData::Unsupported { .. }
                                )),
                                "{label}: {graph:?}"
                            );
                        }
                    } else {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(error.reason, expected[0].0.into(), "{label}: {error:?}");
                        assert_eq!(
                            error.path.as_deref(),
                            Some(fixture.path("cases.ts").as_path())
                        );
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, SOURCE.find(expected[0].1).unwrap())),
                            "{label}: {error:?}"
                        );
                        assert_eq!(error.causes.len(), expected.len() - 1, "{label}: {error:?}");
                        assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                        for (cause, (reason, token)) in error.causes.iter().zip(&expected[1..]) {
                            assert_eq!(cause.reason, (*reason).into(), "{label}: {error:?}");
                            let start = SOURCE.find(token).unwrap();
                            let location = cause.location.as_ref().unwrap();
                            assert_eq!(location.file, "cases.ts");
                            assert_eq!(
                                location.start,
                                position(SOURCE, start),
                                "{label}: {error:?}"
                            );
                            assert_eq!(
                                location.end,
                                position(SOURCE, start + token.len()),
                                "{label}: {error:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
