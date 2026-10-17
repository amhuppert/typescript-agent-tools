//! Actual SuperKeyword roots keep their enclosing calls/accesses. Native
//! diagnostics alone do not turn a genuine object Any or class base into
//! recovery. Reversed requests assert isolation, not warm-checker behavior.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-super-expression/cases.ts");

#[test]
fn super_expression_preserves_native_base_types_and_genuine_object_any() {
    check(false);
}

#[test]
fn super_expression_retains_illegal_use_and_missing_base_recovery() {
    check(true);
}

fn check(recovery: bool) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-super-expression"),
    );
    let file = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-super-expression/expected-strict.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-super-expression/expected-loose.json"),
        ),
        (
            "downlevel",
            include_str!("../fixtures/types/native-super-expression/expected-downlevel.json"),
        ),
    ] {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), 28);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut rows = native.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                if row["errorType"].as_bool().unwrap() != recovery {
                    continue;
                }
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let offset = SOURCE.find(&marker).unwrap() + marker.len();
                if name == "beforeCall" {
                    assert!(
                        row["codes"]
                            .as_array()
                            .unwrap()
                            .contains(&serde_json::json!(17011))
                    );
                }
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 2,
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
                    } else {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{profile}/{name}: {error:?}"
                        );
                        assert_eq!(error.path.as_deref(), Some(file.as_path()));
                        let (reason, start) = if name == "brokenBase" {
                            (
                                SemanticFailureReason::UnresolvedSymbol,
                                SOURCE.find("MissingSuperBase").unwrap(),
                            )
                        } else {
                            (SemanticFailureReason::ErrorType, offset)
                        };
                        assert_eq!(error.reason, reason.into(), "{profile}/{name}: {error:?}");
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, start)),
                            "{profile}/{name}: {error:?}"
                        );
                        assert!(
                            error.omitted_causes.is_none(),
                            "{profile}/{name}: {error:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn super_call_void_preserves_constructor_and_argument_acquisitions() {
    const CALLS: &str = include_str!("../fixtures/types/native-super-calls/cases.ts");
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/native-super-calls"),
    );
    let file = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-super-calls/expected-strict.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-super-calls/expected-loose.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 18);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let selected = CALLS.find(&marker).unwrap() + marker.len();
                let super_at = selected + CALLS[selected..].find("super(").unwrap();
                // checkCallExpression returns Void even after a recovered
                // base or argument. The product must retain those failures.
                assert_eq!(row["summary"], "void");
                assert_eq!(row["errorType"], false);
                let (reason, first, dependencies): (Option<SemanticFailureReason>, usize, &[&str]) =
                    match name {
                        "argumentRecovery" => (
                            Some(SemanticFailureReason::UnresolvedSymbol),
                            CALLS.find("missingPlainArgument").unwrap(),
                            &["missingPlainArgument"],
                        ),
                        "anyRecovery" => (
                            Some(SemanticFailureReason::UnresolvedSymbol),
                            CALLS.find("missingAnyArgument").unwrap(),
                            &["missingAnyArgument"],
                        ),
                        "noBase" => (
                            Some(SemanticFailureReason::ErrorType),
                            super_at,
                            &["missingNoBaseArgument"],
                        ),
                        "nullBase" => (
                            Some(SemanticFailureReason::ErrorType),
                            super_at,
                            &["missingNullArgument"],
                        ),
                        "appliedRecovery" => (
                            Some(SemanticFailureReason::UnresolvedSymbol),
                            CALLS.find("MissingApplied").unwrap(),
                            &["MissingApplied", "missingAppliedArgument"],
                        ),
                        "pairRecovery" => (
                            Some(SemanticFailureReason::UnresolvedSymbol),
                            CALLS.find("MissingPairFirst").unwrap(),
                            &[
                                "MissingPairFirst",
                                "MissingPairSecond",
                                "missingPairFirstArgument",
                                "missingPairSecondArgument",
                            ],
                        ),
                        "wrongArity" => (
                            Some(SemanticFailureReason::ErrorType),
                            CALLS.find("Generic<number, string>").unwrap(),
                            &["missingArityArgument"],
                        ),
                        "wrongFunctionArity" => (
                            Some(SemanticFailureReason::ErrorType),
                            super_at,
                            &["missingFunctionArityArgument"],
                        ),
                        "overloadRecovery" => {
                            (Some(SemanticFailureReason::OverloadRecovery), super_at, &[])
                        }
                        "baseRecovery" => (
                            Some(SemanticFailureReason::UnresolvedSymbol),
                            CALLS.find("MissingSuperConstructor").unwrap(),
                            &["MissingSuperConstructor", "missingBaseArgument"],
                        ),
                        _ => (None, 0, &[]),
                    };
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(CALLS, selected),
                        },
                        expand,
                        depth: 2,
                    });
                    let Some(reason) = reason else {
                        assert!(row["codes"].as_array().unwrap().is_empty());
                        let Ok(TypeAnswer::Selected(result)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(
                            result.outcome,
                            TypeOutcome::Found,
                            "{profile}/{name}: {result:?}"
                        );
                        assert_eq!(result.description.summary, "void");
                        assert!(result.issues.is_empty());
                        continue;
                    };
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(error.reason, reason.into(), "{profile}/{name}: {error:?}");
                    assert_eq!(error.path.as_deref(), Some(file.as_path()));
                    assert_eq!(
                        error.position,
                        Some(position(CALLS, first)),
                        "{profile}/{name}: {error:?}"
                    );
                    // Native checks extends arguments before call arguments,
                    // each in source order. Cache replay may repeat a site;
                    // diagnostic-code arrays do not establish this ordering.
                    let expected: Vec<_> = dependencies
                        .iter()
                        .map(|name| position(CALLS, CALLS.find(name).unwrap()))
                        .collect();
                    let mut observed = Vec::new();
                    if reason == SemanticFailureReason::UnresolvedSymbol {
                        observed.push(position(CALLS, first));
                    }
                    for cause in &error.causes {
                        let location = cause.location.as_ref().unwrap();
                        assert_eq!(location.file, "cases.ts");
                        if cause.reason != SemanticFailureReason::UnresolvedSymbol.into() {
                            assert!(
                                cause.reason == SemanticFailureReason::ErrorType.into()
                                    || name == "overloadRecovery"
                                        && cause.reason
                                            == SemanticFailureReason::OverloadRecovery.into(),
                                "{profile}/{name}: unexpected recovery reason {error:?}"
                            );
                            assert!(
                                location.start == position(CALLS, super_at)
                                    || location.start == position(CALLS, first)
                                    || expected.contains(&location.start),
                                "{profile}/{name}: unexpected recovery site {error:?}"
                            );
                            continue;
                        }
                        assert!(
                            expected.contains(&location.start),
                            "{profile}/{name}: unexpected acquisition {error:?}"
                        );
                        if !observed.contains(&location.start) {
                            observed.push(location.start);
                        }
                    }
                    assert_eq!(observed, expected, "{profile}/{name}: {error:?}");
                    assert!(
                        error.omitted_causes.is_none(),
                        "{profile}/{name}: {error:?}"
                    );
                }
            }
        }
    }
}
