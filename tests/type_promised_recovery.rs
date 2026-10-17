//! Promise/awaited answers and consumed dependencies from the pinned compiler.
//! Repeated Project requests test isolation; they do not claim warm-cache proof.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-promised-recovery/cases.ts");

#[test]
fn direct_structural_and_overloaded_adoption_preserves_native_answers() {
    check(&[
        "directPromise",
        "nestedPromise",
        "structuralThen",
        "absentThen",
        "nonCallableThen",
        "anyThenType",
        "voidThisType",
        "matchingThisType",
        "noValueType",
        "overloadedThenType",
        "mixedThisType",
        "overloadedCallbackType",
        "invocationMissingAwait",
        "healthyAfter",
    ]);
}

#[test]
fn invalid_thenables_consume_error_type_only_at_checked_await() {
    check(&[
        "optionalThenType",
        "wrongThisType",
        "anyCallbackType",
        "noCallbackType",
        "selfRecursive",
        "mutualRecursive",
        "unionAllNil",
    ]);
}

#[test]
fn promise_children_nil_unions_and_checked_displays_keep_later_dependencies() {
    check(&[
        "hiddenThisTypes",
        "missingCallbackType",
        "missingFulfilledType",
        "unionNilThenMissing",
        "quickAwaitAssertion",
        "checkedAwaitAssertion",
        "checkedInvocationMissingAwait",
        "repeatedAwait",
    ]);
}

fn check(names: &[&str]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-promised-recovery/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-promised-recovery/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-promised-recovery/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-promised-recovery/exact.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, captured) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-promised-recovery/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-promised-recovery/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-promised-recovery/expected-exact.json"),
        ),
    ] {
        let baseline: Vec<serde_json::Value> = serde_json::from_str(captured).unwrap();
        assert_eq!(
            baseline
                .iter()
                .filter(|row| names.contains(&row["name"].as_str().unwrap()))
                .count(),
            names.len()
        );
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut rows = baseline.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                if !names.contains(&name) {
                    continue;
                }
                let expected = dependencies(name, profile);
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: marked('#', name),
                    });
                    request.expand = expand;
                    request.depth = 2;
                    let answer = project.type_of(request);
                    if expected.is_empty() {
                        assert_eq!(row["errorType"], false, "{profile}/{name}");
                        assert_eq!(row["codes"], serde_json::json!([]), "{profile}/{name}");
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(
                            answer.outcome,
                            TypeOutcome::Found,
                            "{profile}/{name}/{expand}: {answer:?}"
                        );
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{profile}/{name}/{expand}"
                        );
                        assert!(
                            answer.issues.is_empty(),
                            "{profile}/{name}/{expand}: {answer:?}"
                        );
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(error.path.as_deref(), Some(path.as_path()));
                    assert_eq!(
                        error.reason,
                        expected[0].0.into(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(
                        error.position,
                        Some(expected[0].1),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    let mut acquired = vec![0];
                    for cause in &error.causes {
                        let location = cause.location.as_ref().expect("located native dependency");
                        assert_eq!(location.file, "cases.ts");
                        let index = expected
                            .iter()
                            .position(|(reason, at)| {
                                cause.reason == (*reason).into() && location.start == *at
                            })
                            .unwrap_or_else(|| {
                                panic!("unexpected {profile}/{name}/{expand}: {error:?}")
                            });
                        if !acquired.contains(&index) {
                            acquired.push(index);
                        }
                    }
                    assert_eq!(
                        acquired,
                        (0..expected.len()).collect::<Vec<_>>(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert!(
                        error.omitted_causes.is_none(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                }
            }
        }
    }
}

fn dependencies(name: &str, profile: &str) -> Vec<(SemanticFailureReason, SourcePosition)> {
    use SemanticFailureReason::{ErrorType, UnresolvedSymbol};
    let missing = |site| (UnresolvedSymbol, marked('@', site));
    let await_error = || (ErrorType, awaited_expression(name));
    match name {
        "optionalThenType" if profile == "loose" => vec![],
        "optionalThenType" | "wrongThisType" | "anyCallbackType" | "noCallbackType"
        | "selfRecursive" | "mutualRecursive" | "unionAllNil" => vec![await_error()],
        // getAwaitedTypeNoAliasEx displays the original receiver before the
        // rejected this type, then checkAwaitedType consumes actual nil.
        "hiddenThisTypes" => vec![
            missing("receiverDisplay"),
            missing("thisDisplay"),
            await_error(),
        ],
        "missingCallbackType" => vec![missing("callback"), await_error()],
        "missingFulfilledType" | "repeatedAwait" => vec![missing("fulfilled")],
        // The first constituent's nil is filtered by mapType; only the final
        // checked-await boundary can consume nil as errorType. The later
        // constituent supplies a real unresolved type, so that boundary is nonnil.
        "unionNilThenMissing" => vec![missing("unionValue")],
        "quickAwaitAssertion" => vec![missing("quickValue")],
        // checkAssertion checks its operand before the annotation source node.
        "checkedAwaitAssertion" => vec![missing("checkedOperand"), missing("checkedValue")],
        "checkedInvocationMissingAwait" => vec![
            missing("invocationValue"),
            missing("invocationDisplay"),
            (ErrorType, declaration_expression(name, "invocationUnion()")),
        ],
        "directPromise"
        | "nestedPromise"
        | "structuralThen"
        | "absentThen"
        | "nonCallableThen"
        | "anyThenType"
        | "voidThisType"
        | "matchingThisType"
        | "noValueType"
        | "overloadedThenType"
        | "mixedThisType"
        | "overloadedCallbackType"
        | "invocationMissingAwait"
        | "healthyAfter" => vec![],
        _ => panic!("unclassified native promise root {name}"),
    }
}

fn marked(kind: char, name: &str) -> SourcePosition {
    position(SOURCE, marker_offset(kind, name))
}

fn marker_offset(kind: char, name: &str) -> usize {
    let marker = format!("/*{kind}{name}*/");
    let mut matches = SOURCE.match_indices(&marker);
    let (offset, _) = matches.next().unwrap_or_else(|| panic!("missing {marker}"));
    assert!(matches.next().is_none(), "ambiguous {marker}");
    offset + marker.len()
}

fn awaited_expression(name: &str) -> SourcePosition {
    declaration_expression(name, "await ")
}

fn declaration_expression(name: &str, expression: &str) -> SourcePosition {
    let offset = marker_offset('#', name);
    let declaration = SOURCE[offset..].split_once('\n').unwrap().0;
    position(
        SOURCE,
        offset
            + declaration
                .find(expression)
                .expect("checked expression root"),
    )
}
