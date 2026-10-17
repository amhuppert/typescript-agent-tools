//! `resolveUntypedCall` consumes every actual argument after recovery; the
//! error-call path then consumes unknownSignature's distinct return marker.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-recovery-calls/cases.ts");

fn unique_offset(text: &str) -> usize {
    let mut matches = SOURCE.match_indices(text);
    let first = matches
        .next()
        .unwrap_or_else(|| panic!("missing source selector {text}"));
    assert!(matches.next().is_none(), "ambiguous source selector {text}");
    first.0
}

#[test]
fn native_call_recovery_preserves_later_arguments_and_return_acquisition() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-recovery-calls/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-recovery-calls/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-recovery-calls/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-recovery-calls/exact.json"),
        ),
    ]);
    let names = [
        "untypedMany",
        "errorCallee",
        "uncallableDisplay",
        "returnedThenOperand",
        "newAny",
        "newError",
        "taggedAny",
        "taggedError",
        "healthyAny",
        "healthyAfter",
    ];
    for (profile, captured) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-recovery-calls/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-recovery-calls/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-recovery-calls/expected-exact.json"),
        ),
    ] {
        let native: Vec<serde_json::Value> = serde_json::from_str(captured).unwrap();
        assert_eq!(native.len(), names.len());
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut names = names.to_vec();
            if reverse {
                names.reverse();
            }
            for name in names {
                let native = native
                    .iter()
                    .find(|row| row["name"] == name)
                    .expect("maintained native query");
                let marker = format!("/*#{name}*/");
                let selected = unique_offset(&marker) + marker.len();
                let initializer = selected + SOURCE[selected..].find(" = ").unwrap() + 3;
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, selected),
                    });
                    request.expand = expand;
                    let answer = project.type_of(request);
                    if name.starts_with("healthy") {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}")
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found);
                        assert_eq!(
                            answer.description.summary,
                            native["summary"].as_str().unwrap()
                        );
                        assert_eq!(native["errorType"], false);
                        assert!(answer.issues.is_empty());
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}")
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    let missing =
                        |text| (SemanticFailureReason::UnresolvedSymbol, unique_offset(text));
                    let mut expected = Vec::new();
                    match name {
                        "returnedThenOperand" => expected.push(missing("MissingCallReturn")),
                        "uncallableDisplay" | "taggedError" => {
                            // Display acquires the property for serialization and type output.
                            expected.push(missing("MissingCalleeDisplay"));
                            expected.push(missing("MissingCalleeDisplay"));
                        }
                        _ => {
                            let arithmetic =
                                initializer + SOURCE[initializer..].find("1 - 1n").unwrap();
                            expected.push((SemanticFailureReason::ErrorType, arithmetic));
                            // `getQuickTypeOfExpression` checks the callee; if
                            // it has no sole signature, checked call resolution
                            // asks the callee again. Native emits both errors.
                            if matches!(name, "errorCallee" | "newError") {
                                expected.push((SemanticFailureReason::ErrorType, arithmetic));
                            }
                        }
                    }
                    expected.push(missing("MissingRightCall"));
                    if name != "returnedThenOperand" {
                        expected.push(missing("MissingTailCall"));
                    }
                    if matches!(
                        name,
                        "errorCallee" | "newError" | "uncallableDisplay" | "taggedError"
                    ) {
                        expected.push((SemanticFailureReason::ErrorType, initializer));
                    }
                    assert_eq!(
                        error.reason,
                        expected[0].0.into(),
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, expected[0].1)),
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(
                        error.causes.len(),
                        expected.len() - 1,
                        "{profile}/{name}: {error:?}"
                    );
                    for (cause, (reason, offset)) in error.causes.iter().zip(&expected[1..]) {
                        assert_eq!(
                            cause.reason,
                            (*reason).into(),
                            "{profile}/{name}: {error:?}"
                        );
                        let location = cause
                            .location
                            .as_ref()
                            .expect("call acquisition has an actual source site");
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
