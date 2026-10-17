//! Native class-expression checks preserve immediate dependencies and deferred bodies.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-class-expression-checks/cases.ts");

#[test]
fn class_queries_check_immediate_dependencies_without_checking_deferred_members() {
    check(0);
}
#[test]
fn class_initialization_constraints_and_recovery_keep_native_option_gates() {
    check(1);
}
#[test]
fn class_heritage_implements_and_decorator_boundary_keep_their_native_prefix() {
    check(2);
}

fn check(group: u8) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-class-expression-checks"),
    );
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-class-expression-checks/expected-strict.json"),
        ),
        (
            "nocheck",
            include_str!("../fixtures/types/native-class-expression-checks/expected-nocheck.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-class-expression-checks/expected-loose.json"),
        ),
        (
            "helpers",
            include_str!("../fixtures/types/native-class-expression-checks/expected-helpers.json"),
        ),
        (
            "legacy",
            include_str!("../fixtures/types/native-class-expression-checks/expected-legacy.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 17);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let selected_group = match name {
                    "initialized"
                    | "initializationRecovery"
                    | "parameterRecovery"
                    | "heritageRecovery" => 1,
                    "heritage" | "implemented" | "decorated" => 2,
                    _ => 0,
                };
                if selected_group != group {
                    continue;
                }
                let recovered = match name {
                    "initializationRecovery" if profile != "loose" => {
                        Some("MissingInitializationType")
                    }
                    "parameterRecovery" => Some("MissingClassConstraint"),
                    "heritageRecovery" => Some("MissingClassBase"),
                    _ => None,
                };
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                assert_eq!(
                    row["codes"]
                        .as_array()
                        .unwrap()
                        .contains(&serde_json::json!(2304)),
                    recovered.is_some(),
                    "{profile}/{name}"
                );
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
                    let boundary = match name {
                        "decorated" if profile != "legacy" => Some((
                            SemanticFailureReason::DecoratorTypingRequired,
                            SOURCE.find("@decorate").unwrap(),
                        )),
                        "initialized" if profile != "loose" => Some((
                            SemanticFailureReason::FlowRequired,
                            SOURCE.find("class { static value: 99").unwrap(),
                        )),
                        _ => None,
                    };
                    if let Some((reason, start)) = boundary {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::UnsupportedSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(error.reason, reason.into(), "{label}: {error:?}");
                        assert_eq!(
                            error.path.as_deref(),
                            Some(fixture.path("cases.ts").as_path())
                        );
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, start)),
                            "{label}: {error:?}"
                        );
                        assert!(error.causes.is_empty(), "{label}: {error:?}");
                        assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                    } else if let Some(missing) = recovered {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        let start = SOURCE.find(missing).unwrap();
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.path.as_deref(),
                            Some(fixture.path("cases.ts").as_path())
                        );
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, start)),
                            "{label}: {error:?}"
                        );
                        assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                        for cause in &error.causes {
                            assert_eq!(
                                cause.reason,
                                SemanticFailureReason::UnresolvedSymbol.into(),
                                "{label}: {error:?}"
                            );
                            let location = cause.location.as_ref().unwrap();
                            assert_eq!(location.file, "cases.ts");
                            assert_eq!(
                                location.start,
                                position(SOURCE, start),
                                "{label}: {error:?}"
                            );
                            assert_eq!(
                                location.end,
                                position(SOURCE, start + missing.len()),
                                "{label}: {error:?}"
                            );
                        }
                    } else {
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
                    }
                }
            }
        }
    }
}
