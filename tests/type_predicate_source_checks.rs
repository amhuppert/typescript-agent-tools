//! `checkTypePredicate` keeps diagnostic checks and asserted-type acquisitions
//! separate. A declaration-only typeof query remains lazy.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-type-predicate-checks/cases.ts");

#[test]
fn predicate_source_checks_preserve_parent_gates_and_consumed_dependencies() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-type-predicate-checks"),
    );
    for (profile, baseline) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-type-predicate-checks/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-type-predicate-checks/expected-loose.json"),
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
                let recovered = name == "recoveredPredicate";
                assert_eq!(row["summary"], "any");
                assert_eq!(row["errorType"], false);
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                let error_reads = row["acquisitions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|event| {
                        event["operation"] == "getTypeFromTypeNode"
                            && event["event"] == "exit"
                            && event["errorType"] == true
                    })
                    .count();
                assert_eq!(
                    error_reads,
                    if recovered { 2 } else { 0 },
                    "{profile}/{name}"
                );
                assert_eq!(
                    row["codes"]
                        .as_array()
                        .unwrap()
                        .contains(&serde_json::json!(2304)),
                    recovered
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
                    if recovered {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        let start = SOURCE.find("MissingChecked").unwrap();
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
                        // Native reads the asserted type while constructing the
                        // predicate, then checks the same child from its cache.
                        assert_eq!(error.causes.len(), 1, "{label}: {error:?}");
                        assert!(error.omitted_causes.is_none());
                        for cause in &error.causes {
                            assert_eq!(
                                cause.reason,
                                SemanticFailureReason::UnresolvedSymbol.into()
                            );
                            let location = cause.location.as_ref().unwrap();
                            assert_eq!(location.file, "cases.ts");
                            assert_eq!(location.start, position(SOURCE, start));
                            assert_eq!(
                                location.end,
                                position(SOURCE, start + "MissingChecked".len())
                            );
                        }
                    } else {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}");
                        assert_eq!(answer.description.summary, "any", "{label}");
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
