//! Directional enum compatibility, cache reuse, and native enum consumers.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-enum-relations/cases.ts");
const PROFILES: [(&str, &str); 3] = [
    (
        "tsconfig",
        include_str!("../fixtures/types/native-enum-relations/expected-tsconfig.json"),
    ),
    (
        "loose",
        include_str!("../fixtures/types/native-enum-relations/expected-loose.json"),
    ),
    (
        "unchecked",
        include_str!("../fixtures/types/native-enum-relations/expected-unchecked.json"),
    ),
];

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-enum-relations"),
    )
}

fn selection(name: &str) -> usize {
    let marker = format!("/*#{name}*/");
    let mut matches = SOURCE.match_indices(&marker);
    let selected = matches.next().unwrap().0 + marker.len();
    assert!(matches.next().is_none(), "ambiguous {marker}");
    selected
}

fn query(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, selection(name)),
        },
        expand,
        depth: usize::from(expand),
    }
}

#[test]
fn enum_relations_and_consumers_match_pinned_native_variants() {
    let fixture = fixture();
    for (profile, baseline) in PROFILES {
        let project = fixture.load_config(&format!("{profile}.json"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 46);
        // Independent queries prove isolation. The comma expressions in
        // cachedSuccess/cachedFailure perform both calls in one evaluator.
        for reverse in [false, true] {
            let mut rows = rows.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                if matches!(
                    name,
                    "reportTwice"
                        | "normalValues"
                        | "normalMissing"
                        | "normalOpaque"
                        | "normalMissingTwice"
                        | "normalOpaqueTwice"
                        | "constNumber"
                        | "constDynamicString"
                ) {
                    continue;
                }
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert_eq!(row["codes"], serde_json::json!([]), "{profile}/{name}");
                // reportValues/reportMissing/reportOpaque deliberately keep
                // the native quick single-signature path: no applicability
                // or error-reporting work is consumed for their declarations.
                for expand in [false, true] {
                    let answer = project.type_of(query(&fixture, name, expand));
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
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
                    assert!(!result.description.summary_truncated, "{profile}/{name}");
                    assert!(result.issues.is_empty(), "{profile}/{name}: {result:?}");
                    if let Some(graph) = &result.description.expansion {
                        assert!(
                            graph
                                .nodes
                                .iter()
                                .all(|node| !matches!(node.data, TypeNodeData::Unsupported { .. })),
                            "{profile}/{name}: {graph:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn reported_enum_failures_retain_each_actual_call_consumer() {
    let fixture = fixture();
    for (profile, baseline) in PROFILES {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        let project = fixture.load_config(&format!("{profile}.json"));
        for (name, callee, count) in [
            ("reportTwice", "strictNumeric(", 2),
            ("normalValues", "strictNumeric(", 1),
            ("normalMissing", "strictNumeric(", 1),
            ("normalOpaque", "strictString(", 1),
            ("normalMissingTwice", "strictNumeric(", 2),
            ("normalOpaqueTwice", "strictString(", 2),
        ] {
            let start = selection(name);
            let end = start + SOURCE[start..].find(';').unwrap();
            let calls: Vec<_> = SOURCE[start..end]
                .match_indices(callee)
                .map(|(offset, _)| position(SOURCE, start + offset))
                .collect();
            assert_eq!(calls.len(), count, "{name}");
            let native = rows.iter().find(|row| row["name"] == name).unwrap();
            assert_eq!(
                native["codes"],
                serde_json::json!(vec![2345; count]),
                "{profile}/{name}"
            );
            assert_eq!(native["errorType"], false, "{profile}/{name}");
            assert!(native["demandFailures"].as_array().unwrap().is_empty());
            // Native reports preserve their return value. The product retains
            // each failed overload at its real call node; repeated reporting
            // tests the directional enum cache in this same evaluator.
            for expand in [false, true] {
                let answer = project.type_of(query(&fixture, name, expand));
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{profile}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    error.kind,
                    SemanticErrorKind::InvalidSemantic,
                    "{profile}/{name}: {error:?}"
                );
                assert_eq!(
                    error.reason,
                    SemanticFailureReason::OverloadRecovery.into(),
                    "{profile}/{name}: {error:?}"
                );
                assert_eq!(
                    error.path.as_deref(),
                    Some(fixture.path("cases.ts").canonicalize().unwrap().as_path())
                );
                assert_eq!(
                    error.position,
                    Some(calls[0]),
                    "{profile}/{name}: {error:?}"
                );
                assert_eq!(error.causes.len(), count - 1, "{profile}/{name}: {error:?}");
                for (cause, call) in error.causes.iter().zip(&calls[1..]) {
                    assert_eq!(cause.reason, SemanticFailureReason::OverloadRecovery.into());
                    let location = cause.location.as_ref().unwrap();
                    assert_eq!(location.file, "cases.ts");
                    assert_eq!(location.start, *call, "{profile}/{name}: {error:?}");
                }
                assert!(
                    error.omitted_causes.is_none(),
                    "{profile}/{name}: {error:?}"
                );
            }
        }
    }
}

#[test]
fn const_enum_index_requires_literal_syntax_after_checking_the_key() {
    let fixture = fixture();
    for (profile, baseline) in PROFILES {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        let project = fixture.load_config(&format!("{profile}.json"));
        for name in ["constNumber", "constDynamicString"] {
            let native = rows.iter().find(|row| row["name"] == name).unwrap();
            assert_eq!(native["errorType"], true);
            assert_eq!(native["codes"], serde_json::json!([2476]));
            let start = selection(name);
            let key = start + SOURCE[start..].find('[').unwrap() + 1;
            for expand in [false, true] {
                let answer = project.type_of(query(&fixture, name, expand));
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{profile}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    error.kind,
                    SemanticErrorKind::InvalidSemantic,
                    "{profile}/{name}: {error:?}"
                );
                assert_eq!(
                    error.reason,
                    SemanticFailureReason::ErrorType.into(),
                    "{profile}/{name}: {error:?}"
                );
                assert_eq!(
                    error.path.as_deref(),
                    Some(fixture.path("cases.ts").canonicalize().unwrap().as_path())
                );
                assert_eq!(
                    error.position,
                    Some(position(SOURCE, key)),
                    "{profile}/{name}: {error:?}"
                );
                assert!(error.causes.is_empty(), "{profile}/{name}: {error:?}");
                assert!(error.omitted_causes.is_none());
            }
        }
    }
}
