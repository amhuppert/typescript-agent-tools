//! Conditional factories continue after consumed native error values, preserve
//! branch gates, and retain recovery when a completed root is instantiated again.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

fn dependencies(name: &str, source: &str) -> Vec<(usize, SemanticFailureReason)> {
    use SemanticFailureReason::{ErrorType, UnresolvedSymbol};
    let tokens: &[&str] = match name {
        "directAny" => &["MissingDirectTrue", "MissingDirectFalse"],
        "genericAny" | "distributed" | "repeatedInstantiation" => {
            &["MissingGenericTrue", "MissingGenericFalse"]
        }
        "errorAlias" => &["MissingCheck", "MissingCheckTrue", "MissingCheckFalse"],
        "inference" => &["MissingInferSource", "MissingInferAfter"],
        "tailFalse" => &["MissingTailTrue"],
        "nestedAny" => &[
            "MissingNestedFirst",
            "MissingNestedSecond",
            "MissingNestedLast",
        ],
        "mappedLowerBound" => &["MissingMappedKey", "MissingMappedValue"],
        "bareCheck" => {
            return vec![
                (
                    source.find("number[\"missing\"] extends").unwrap(),
                    ErrorType,
                ),
                (source.find("MissingExtends").unwrap(), UnresolvedSymbol),
            ];
        }
        "bareExtends" => {
            return vec![
                (source.find("MissingBefore").unwrap(), UnresolvedSymbol),
                (
                    source.find("extends number[\"missing\"]").unwrap() + "extends ".len(),
                    ErrorType,
                ),
            ];
        }
        "trueOnly" | "falseOnly" | "healthyLowerBound" | "healthy" => &[],
        _ => panic!("unclassified native conditional row: {name}"),
    };
    tokens
        .iter()
        .map(|token| (source.find(token).unwrap(), UnresolvedSymbol))
        .collect()
}

#[test]
fn conditional_completion_preserves_native_branch_order_and_recovery() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-conditional-completion");
    let source = std::fs::read_to_string(root.join("cases.ts")).unwrap();
    for profile in ["strict", "nocheck"] {
        let config = std::fs::read_to_string(root.join(format!("{profile}.json"))).unwrap();
        let expected: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(expected.len(), 15);
        let fixture = Fixture::new(&[("tsconfig.json", &config), ("cases.ts", &source)]);
        for reverse in [false, true] {
            let project = fixture.load();
            let mut rows: Vec<_> = expected.iter().collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                assert_eq!(
                    row["demandFailures"],
                    serde_json::json!([]),
                    "{profile}/{name}"
                );
                let required = dependencies(name, &source);
                assert_eq!(
                    row["errors"].as_array().unwrap().is_empty(),
                    required.is_empty(),
                    "{profile}/{name}"
                );
                let marker = format!("/*#{name}*/");
                let at = position(&source, source.find(&marker).unwrap() + marker.len());
                for expand in [false, true, false] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: at,
                        },
                        expand,
                        depth: 0,
                    });
                    if let Some(((first, reason), later)) = required.split_first() {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(error.reason, (*reason).into(), "{label}: {error:?}");
                        assert_eq!(
                            error.path,
                            Some(fixture.path("cases.ts")),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.position,
                            Some(position(&source, *first)),
                            "{label}: {error:?}"
                        );
                        let mut remaining = error.causes.as_slice();
                        for &(offset, reason) in later {
                            let expected_at = position(&source, offset);
                            let found = remaining.iter().position(|cause| {
                                cause.location.as_ref().is_some_and(|location| {
                                    location.file == "cases.ts" && location.start == expected_at
                                }) && cause.reason == reason.into()
                            });
                            let Some(found) = found else {
                                panic!(
                                    "{label}: missing ordered dependency at {expected_at:?}: {error:?}"
                                );
                            };
                            remaining = &remaining[found + 1..];
                        }
                        // A bare native error operand stops before either branch.
                        // It differs from unresolved-alias Any, which can read both.
                        if name == "bareCheck" || name == "bareExtends" {
                            assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                            let forbidden = if name == "bareCheck" {
                                ["MissingUnreachedTrue", "MissingUnreachedFalse"]
                            } else {
                                ["MissingUnreachedThen", "MissingUnreachedElse"]
                            };
                            for token in forbidden {
                                let forbidden_at = position(&source, source.find(token).unwrap());
                                assert!(
                                    !error.causes.iter().any(|cause| cause
                                        .location
                                        .as_ref()
                                        .is_some_and(|location| location.start == forbidden_at)),
                                    "{label}: acquired skipped branch {token}: {error:?}"
                                );
                            }
                        }
                    } else {
                        assert_eq!(row["errorType"], false, "{label}");
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}");
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{label}"
                        );
                        assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                    }
                }
            }
        }
    }
}
