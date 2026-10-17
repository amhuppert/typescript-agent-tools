//! The actual checked Promise factory consumes recovery even when it returns
//! native Unknown; quiet structural awaiting does not consume that fallback.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-checked-promise/cases.ts");
const KIND: &str = include_str!("../fixtures/types/native-checked-promise/promise-kind.d.ts");
const ARITY: &str = include_str!("../fixtures/types/native-checked-promise/promise-arity.d.ts");

#[test]
fn quiet_await_and_compatible_arrows_do_not_consume_checked_promise_recovery() {
    check(&["quiet", "compatible"]);
}

#[test]
fn arrow_error_elaboration_consumes_the_checked_global_after_quiet_lookup() {
    check(&["checked", "quietThenChecked"]);
}

fn check(names: &[&str]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-checked-promise/globals.d.ts"),
        ),
        (
            "promise-valid.d.ts",
            include_str!("../fixtures/types/native-checked-promise/promise-valid.d.ts"),
        ),
        ("promise-kind.d.ts", KIND),
        ("promise-arity.d.ts", ARITY),
        (
            "valid.json",
            include_str!("../fixtures/types/native-checked-promise/valid.json"),
        ),
        (
            "missing.json",
            include_str!("../fixtures/types/native-checked-promise/missing.json"),
        ),
        (
            "kind.json",
            include_str!("../fixtures/types/native-checked-promise/kind.json"),
        ),
        (
            "arity.json",
            include_str!("../fixtures/types/native-checked-promise/arity.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, baseline) in [
        (
            "valid",
            include_str!("../fixtures/types/native-checked-promise/expected-valid.json"),
        ),
        (
            "missing",
            include_str!("../fixtures/types/native-checked-promise/expected-missing.json"),
        ),
        (
            "kind",
            include_str!("../fixtures/types/native-checked-promise/expected-kind.json"),
        ),
        (
            "arity",
            include_str!("../fixtures/types/native-checked-promise/expected-arity.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), 4);
        // Reversed requests check isolation, not warmth. quietThenChecked
        // alone performs the quiet and checked questions in one evaluator.
        for reverse in [false, true] {
            let mut rows = native.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                if !names.contains(&name) {
                    continue;
                }
                let marker = format!("/*#{name}*/");
                let selected = SOURCE.find(&marker).unwrap() + marker.len();
                let factory = matches!(name, "checked" | "quietThenChecked");
                assert_eq!(row["errorType"], false);
                if factory {
                    assert_eq!(row["factory"]["emptyRecovery"], profile != "valid");
                } else {
                    assert!(row["factory"].is_null());
                }
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, selected),
                        },
                        expand,
                        depth: 2,
                    });
                    if !factory || profile == "valid" {
                        let Ok(TypeAnswer::Selected(result)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}")
                        };
                        assert_eq!(result.outcome, TypeOutcome::Found);
                        assert_eq!(result.description.summary, row["summary"].as_str().unwrap());
                        assert!(
                            result.issues.is_empty(),
                            "{profile}/{name}/{expand}: {result:?}"
                        );
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}")
                    };
                    assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic, "{error:?}");
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::ErrorType.into(),
                        "{error:?}"
                    );
                    assert_eq!(error.path.as_deref(), Some(path.as_path()));
                    // satisfies passes its actual expression as the consuming
                    // operation; the absent global has no invented declaration.
                    let consumer = selected
                        + SOURCE[selected..]
                            .find("(() => 1) satisfies (() => string)")
                            .unwrap();
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, consumer)),
                        "{error:?}"
                    );
                    assert_eq!(
                        error.message,
                        "global type 'Promise' dependency consumed compiler recovery"
                    );
                    assert_eq!(error.causes.len(), 1, "{error:?}");
                    let cause = &error.causes[0];
                    match profile {
                        "missing" => {
                            assert_eq!(
                                cause.reason,
                                SemanticFailureReason::UnresolvedSymbol.into()
                            );
                            assert!(cause.location.is_none());
                            assert_eq!(cause.message, "Cannot find global type 'Promise'.");
                        }
                        "kind" | "arity" => {
                            assert_eq!(cause.reason, SemanticFailureReason::ErrorType.into());
                            let (file, text, message) = if profile == "kind" {
                                (
                                    "promise-kind.d.ts",
                                    KIND,
                                    "Global type 'Promise' must be a class or interface type.",
                                )
                            } else {
                                (
                                    "promise-arity.d.ts",
                                    ARITY,
                                    "Global type 'Promise' must have 1 type parameter(s).",
                                )
                            };
                            let location =
                                cause.location.as_ref().expect("actual global declaration");
                            assert_eq!(location.file, file);
                            assert_eq!(
                                location.start,
                                position(text, text.find("Promise").unwrap())
                            );
                            assert_eq!(cause.message, message);
                        }
                        _ => unreachable!(),
                    }
                    assert!(error.omitted_causes.is_none(), "{error:?}");
                }
            }
        }
    }
}
