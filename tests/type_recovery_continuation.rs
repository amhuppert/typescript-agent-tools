//! Native values can continue checking while consumed recovery rejects a query.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-recovery-continuation/cases.ts");

#[test]
fn completed_arithmetic_checks_later_operands_and_retains_consumed_recovery() {
    check(&[
        "recoveredThenMissing",
        "recoveredThenHealthy",
        "healthyThenRecovered",
        "conditionalFirst",
        "conditionalLiteralTrue",
        "conditionalLiteralFalse",
        "conditionalHealthy",
        "typeofRecovered",
        "genuineAnyThenHealthy",
        "genuineAnyThenMissing",
        "unaryMinusRecovered",
        "unaryNotRecovered",
        "deleteRecovered",
        "voidDeferred",
    ]);
}

#[test]
fn recovered_members_check_later_native_dependencies() {
    check(&[
        "missingThenRecovered",
        "missingThenMissing",
        "conditionalSecond",
    ]);
}

#[test]
fn both_operator_displays_precede_recovery_and_compound_setter_checking() {
    check(&["bothDisplays", "bothDisplaysAndSetter"]);
}

#[test]
fn composite_signature_consumes_returns_after_unknown_signature_recovery() {
    check(&["compositeReturn"]);
}

fn check(names: &[&str]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-recovery-continuation/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-recovery-continuation/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-recovery-continuation/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-recovery-continuation/exact.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-recovery-continuation/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-recovery-continuation/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-recovery-continuation/expected-exact.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(rows.len(), 20);
        for reverse in [false, true] {
            let mut rows = rows.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                if !names.contains(&name) {
                    continue;
                }
                let marker = format!("/*#{name}*/");
                let selected = unique_offset(SOURCE, &marker) + marker.len();
                let initializer = selected + SOURCE[selected..].find(" = ").unwrap() + 3;
                let numeric = || initializer + SOURCE[initializer..].find("1 - 1n").unwrap();
                let missing = |name| {
                    (
                        SemanticFailureReason::UnresolvedSymbol,
                        unique_offset(SOURCE, name),
                    )
                };
                let missing_in = |declaration, name| {
                    let start = unique_offset(SOURCE, declaration);
                    let line = SOURCE[start..].lines().next().unwrap();
                    (
                        SemanticFailureReason::UnresolvedSymbol,
                        start + unique_offset(line, name),
                    )
                };
                let recovery = || (SemanticFailureReason::ErrorType, numeric());
                let failures = match name {
                    "genuineAnyThenHealthy" | "voidDeferred" => Vec::new(),
                    "genuineAnyThenMissing" => vec![missing("MissingLater")],
                    "recoveredThenMissing"
                    | "conditionalFirst"
                    | "conditionalLiteralTrue"
                    | "conditionalLiteralFalse" => {
                        vec![recovery(), missing("MissingLater")]
                    }
                    "missingThenRecovered" | "conditionalSecond" => {
                        vec![missing("MissingLater"), recovery()]
                    }
                    "missingThenMissing" => vec![missing("MissingFirst"), missing("MissingLater")],
                    "bothDisplays" => vec![
                        missing_in("declare const leftBrand:", "MissingLeftDisplay"),
                        missing_in("declare const leftBrand:", "MissingLeftDisplay"),
                        missing("MissingRightDisplay"),
                        missing("MissingRightDisplay"),
                        (SemanticFailureReason::ErrorType, initializer),
                    ],
                    "bothDisplaysAndSetter" => vec![
                        missing_in("declare const setter:", "MissingLeftDisplay"),
                        missing_in("declare const setter:", "MissingLeftDisplay"),
                        missing("MissingRightDisplay"),
                        missing("MissingRightDisplay"),
                        (SemanticFailureReason::ErrorType, initializer),
                        missing("MissingSetter"),
                    ],
                    "compositeReturn" => vec![
                        (SemanticFailureReason::ErrorType, initializer),
                        missing("MissingReturn"),
                    ],
                    _ => vec![recovery()],
                };
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, selected),
                    });
                    request.expand = expand;
                    request.depth = 2;
                    let answer = project.type_of(request);
                    if failures.is_empty() {
                        assert_eq!(row["errorType"], false);
                        let Ok(TypeAnswer::Selected(result)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}")
                        };
                        assert_eq!(result.outcome, TypeOutcome::Found);
                        assert_eq!(result.description.summary, row["summary"].as_str().unwrap());
                        assert!(result.issues.is_empty());
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}")
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(error.path.as_deref(), Some(path.as_path()));
                    assert_eq!(
                        error.reason,
                        failures[0].0.into(),
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, failures[0].1)),
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(
                        error.causes.len(),
                        failures.len() - 1,
                        "{profile}/{name}: {error:?}"
                    );
                    for (cause, (reason, offset)) in error.causes.iter().zip(&failures[1..]) {
                        assert_eq!(
                            cause.reason,
                            (*reason).into(),
                            "{profile}/{name}: {error:?}"
                        );
                        let location = cause
                            .location
                            .as_ref()
                            .expect("native dependency has an actual source");
                        assert_eq!(location.file, "cases.ts");
                        assert_eq!(
                            location.start,
                            position(SOURCE, *offset),
                            "{profile}/{name}: {error:?}"
                        );
                    }
                    assert!(error.omitted_causes.is_none());
                }
            }
        }
    }
}

fn unique_offset(source: &str, text: &str) -> usize {
    let mut matches = source.match_indices(text);
    let (offset, _) = matches
        .next()
        .unwrap_or_else(|| panic!("missing fixture text {text:?}"));
    assert!(matches.next().is_none(), "ambiguous fixture text {text:?}");
    offset
}

#[test]
fn final_query_bounds_ordered_causes_under_at_and_over_the_output_prefix() {
    let source = include_str!("../fixtures/types/native-recovery-cause-limit/cases.ts");
    let fixture = Fixture::new(&[
        ("cases.ts", source),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-recovery-cause-limit/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-recovery-cause-limit/tsconfig.json"),
        ),
    ]);
    let project = fixture.load_config("tsconfig.json");
    for count in [15usize, 16, 17] {
        let marker = format!("/*#causes{count}*/");
        let selected = source.find(&marker).unwrap() + marker.len();
        let initializer = selected + source[selected..].find(" = ").unwrap() + 3;
        let line = source[initializer..].lines().next().unwrap();
        let offsets: Vec<_> = line
            .match_indices("1 - 1n")
            .map(|(offset, _)| initializer + offset)
            .collect();
        assert_eq!(offsets.len(), count + 1);
        for expand in [false, true] {
            let mut request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(source, selected),
            });
            request.expand = expand;
            let answer = project.type_of(request);
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{count}/{expand}: {answer:?}")
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
            assert_eq!(error.position, Some(position(source, offsets[0])));
            assert_eq!(error.causes.len(), count.min(16));
            assert_eq!(
                error.omitted_causes,
                if count > 16 { Some(count - 16) } else { None }
            );
            for (cause, offset) in error.causes.iter().zip(&offsets[1..]) {
                assert_eq!(cause.reason, SemanticFailureReason::ErrorType.into());
                assert_eq!(
                    cause.location.as_ref().unwrap().start,
                    position(source, *offset)
                );
            }
        }
    }
}
