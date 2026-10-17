//! A genuine Void result does not erase acquisitions in assertion reporting.
//! Native diagnostics without a supplied recovery value remain healthy.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, SemanticError, SemanticErrorKind, SemanticFailureReason,
};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-assertion-call-tail/cases.ts");
const TWIN: &str = include_str!("../fixtures/types/native-assertion-tail-predicate/cases.ts");

fn fixture(name: &str) -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types")
            .join(name),
    )
}

fn request(fixture: &Fixture, source: &str, name: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{name}*/");
    let offset = source.find(&marker).unwrap() + marker.len();
    assert_eq!(source.as_bytes()[offset], b')');
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(source, offset),
        },
        expand,
        depth: 2,
    }
}

fn healthy(answer: Result<TypeAnswer, AnalysisError>, summary: &str, expand: bool, label: &str) {
    let Ok(TypeAnswer::Selected(answer)) = answer else {
        panic!("{label}: {answer:?}");
    };
    assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
    assert_eq!(answer.description.summary, summary, "{label}");
    assert!(answer.issues.is_empty(), "{label}: {answer:?}");
    if expand {
        assert!(
            answer
                .description
                .expansion
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .all(|node| !matches!(node.data, TypeNodeData::Unsupported { .. })),
            "{label}: {answer:?}"
        );
    }
}

fn unresolved(
    answer: Result<TypeAnswer, AnalysisError>,
    fixture: &Fixture,
    source: &str,
    dependency: &str,
    label: &str,
) -> SemanticError {
    let Err(AnalysisError::Semantic(error)) = answer else {
        panic!("{label}: {answer:?}");
    };
    let at = source.find(dependency).unwrap();
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
        Some(fixture.path("cases.ts").canonicalize().unwrap().as_path())
    );
    assert_eq!(
        error.position,
        Some(position(source, at)),
        "{label}: {error:?}"
    );
    assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
    for cause in &error.causes {
        assert_eq!(
            cause.reason,
            SemanticFailureReason::UnresolvedSymbol.into(),
            "{label}: {error:?}"
        );
        let location = cause
            .location
            .as_ref()
            .expect("asserted types and arguments have source sites");
        assert_eq!(location.file, "cases.ts", "{label}: {error:?}");
        assert_eq!(location.start, position(source, at), "{label}: {error:?}");
        assert_eq!(
            location.end,
            position(source, at + dependency.len()),
            "{label}: {error:?}"
        );
    }
    *error
}

#[test]
fn assertion_target_reporting_preserves_native_result_and_consumed_dependencies() {
    let fixture = fixture("native-assertion-call-tail");
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-assertion-call-tail/expected-strict.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-assertion-call-tail/expected-loose.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 28);
        for reverse in [false, true] {
            // Requests own fresh evaluators. Reversing them proves isolation,
            // not a native cache hit within one checker operation.
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                let dependency = match name {
                    // Generic instantiation acquires these predicates before
                    // the tail even when the call is a returned expression.
                    "recoveredPredicate" | "recoveredNotStatement" => Some("MissingAssertedType"),
                    "recoveredArgument" => Some("missingAssertionArgument"),
                    _ => None,
                };
                let codes = row["codes"].as_array().unwrap();
                assert_eq!(
                    codes.contains(&serde_json::json!(2304)),
                    dependency.is_some()
                );
                let expected_reporting_code = match name {
                    "inferredTarget"
                    | "parenthesizedTarget"
                    | "inferredReceiver"
                    | "mappedOrigin"
                    | "inferredClassField"
                    | "privateField"
                    | "forOfInferred"
                    | "recoveredArgument" => Some(2775),
                    "optionalReceiver" if profile == "loose" => Some(2775),
                    "elementTarget" | "nonNullTarget" | "callTarget" => Some(2776),
                    _ => None,
                };
                assert_eq!(
                    codes.contains(&serde_json::json!(2775)),
                    expected_reporting_code == Some(2775)
                );
                assert_eq!(
                    codes.contains(&serde_json::json!(2776)),
                    expected_reporting_code == Some(2776)
                );
                for expand in [false, true] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(request(&fixture, SOURCE, name, expand));
                    if let Some(dependency) = dependency {
                        unresolved(answer, &fixture, SOURCE, dependency, &label);
                    } else {
                        healthy(answer, row["summary"].as_str().unwrap(), expand, &label);
                    }
                }
            }
        }
    }
}

#[test]
fn direct_statement_tail_alone_acquires_the_nongeneric_asserted_type() {
    let fixture = fixture("native-assertion-tail-predicate");
    let dependency = "MissingTailPredicate";
    let dependency_at = TWIN.find(dependency).unwrap();
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-assertion-tail-predicate/expected-strict.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-assertion-tail-predicate/expected-loose.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 2);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let statement = name == "statement";
                assert_eq!(row["summary"], "void");
                assert_eq!(row["errorType"], false);
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                assert_eq!(
                    row["codes"],
                    if statement {
                        serde_json::json!([2304])
                    } else {
                        serde_json::json!([])
                    }
                );
                let acquired = row["acquisitions"].as_array().unwrap().iter().any(|event| {
                    event["operation"] == "getTypeFromTypeNode"
                        && event["event"] == "exit"
                        && event["errorType"] == true
                        && event["pos"]
                            .as_u64()
                            .is_some_and(|pos| pos as usize <= dependency_at)
                        && event["end"].as_u64() == Some((dependency_at + dependency.len()) as u64)
                });
                assert_eq!(acquired, statement, "{profile}/{name}");
                for expand in [false, true] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(request(&fixture, TWIN, name, expand));
                    if statement {
                        let error = unresolved(answer, &fixture, TWIN, dependency, &label);
                        // checker8330 acquires the predicate, then getEffectsSignature
                        // reads it in Some(signatures) and once on the selected
                        // signature. The last two consume its completed cache.
                        assert_eq!(error.causes.len(), 2, "{label}: {error:?}");
                    } else {
                        healthy(answer, "void", expand, &label);
                    }
                }
            }
        }
    }
}
