//! Native assertion null removal, chain marker propagation and recovery order.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-non-null-assertions/cases.ts");

#[test]
fn non_null_assertions_preserve_native_values_and_optional_chain_boundaries() {
    check(false);
}

#[test]
fn non_null_assertions_keep_operand_recovery_and_later_native_acquisitions() {
    check(true);
}

fn check(recovery: bool) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-non-null-assertions"),
    );
    let file = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, baseline) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-non-null-assertions/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-non-null-assertions/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-non-null-assertions/expected-exact.json"),
        ),
    ] {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), 11);
        let project = fixture.load_config(&format!("{profile}.json"));
        // Each query owns a fresh evaluator; reversal asserts isolation.
        for reverse in [false, true] {
            let mut rows = native.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                if recovery != matches!(name, "operandRecovery" | "continuation") {
                    continue;
                }
                assert_eq!(row["errorType"], recovery);
                let marker = format!("/*#{name}*/");
                let selected = SOURCE.find(&marker).unwrap() + marker.len();
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, selected),
                        },
                        expand,
                        depth: 3,
                    });
                    if !recovery {
                        let Ok(TypeAnswer::Selected(result)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(
                            result.outcome,
                            TypeOutcome::Found,
                            "{profile}/{name}: {result:?}"
                        );
                        assert_eq!(
                            result.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{profile}/{name}/{expand}"
                        );
                        assert!(result.issues.is_empty(), "{profile}/{name}: {result:?}");
                        if expand {
                            let graph = result.description.expansion.as_ref().unwrap();
                            assert!(
                                graph.nodes.iter().all(|node| !matches!(
                                    node.data,
                                    TypeNodeData::Unsupported { .. }
                                )),
                                "{profile}/{name}: {graph:?}"
                            );
                        }
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
                    assert_eq!(error.path.as_deref(), Some(file.as_path()));
                    let first = position(SOURCE, SOURCE.find("MissingOperand").unwrap());
                    let later = position(SOURCE, SOURCE.find("MissingLater").unwrap());
                    assert_eq!(error.position, Some(first), "{profile}/{name}: {error:?}");
                    let expected = if name == "continuation" {
                        vec![first, later]
                    } else {
                        vec![first]
                    };
                    let mut observed = vec![first];
                    for cause in &error.causes {
                        assert_eq!(
                            cause.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{profile}/{name}: {error:?}"
                        );
                        let at = cause.location.as_ref().unwrap();
                        assert_eq!(at.file, "cases.ts");
                        assert!(
                            expected.contains(&at.start),
                            "unexpected acquisition: {profile}/{name}: {error:?}"
                        );
                        if !observed.contains(&at.start) {
                            observed.push(at.start);
                        }
                    }
                    assert_eq!(observed, expected, "{profile}/{name}: {error:?}");
                    assert!(error.omitted_causes.is_none());
                }
            }
        }
    }
}
