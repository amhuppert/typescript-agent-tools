//! Error-type production does not skip later native operator dependencies.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-operator-recovery/cases.ts");

#[test]
fn mixed_numeric_recovery_preserves_later_setter_failures_and_genuine_any() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-operator-recovery/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-operator-recovery/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-operator-recovery/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-operator-recovery/exact.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-operator-recovery/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-operator-recovery/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-operator-recovery/expected-exact.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(rows.len(), 8);
        for reverse in [false, true] {
            let mut rows = rows.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let selected = SOURCE.find(&marker).unwrap() + marker.len();
                let initializer = selected + SOURCE[selected..].find(" = ").unwrap() + 3;
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, selected),
                    });
                    request.expand = expand;
                    request.depth = 1;
                    let answer = project.type_of(request);
                    if name == "genuineAnyPlus" {
                        assert_eq!(row["errorType"], false);
                        let Ok(TypeAnswer::Selected(result)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(result.outcome, TypeOutcome::Found);
                        assert_eq!(result.description.summary, "any");
                        assert!(result.issues.is_empty());
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
                    assert_eq!(error.path.as_deref(), Some(path.as_path()));
                    let only_setter = name == "validNumericSetter";
                    let branded = name.starts_with("branded");
                    let primary_offset = if branded {
                        SOURCE.find("DisplayMissing").unwrap()
                    } else if only_setter {
                        SOURCE.find("Missing").unwrap()
                    } else {
                        initializer
                    };
                    let reason = if only_setter || branded {
                        SemanticFailureReason::UnresolvedSymbol
                    } else {
                        SemanticFailureReason::ErrorType
                    };
                    assert_eq!(error.reason, reason.into(), "{profile}/{name}: {error:?}");
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, primary_offset)),
                        "{profile}/{name}: {error:?}"
                    );
                    let mut causes = Vec::new();
                    if branded {
                        // Symbol serialization and the subsequent type read
                        // each consume the same cached display annotation.
                        causes.push((SemanticFailureReason::UnresolvedSymbol, primary_offset));
                        causes.push((SemanticFailureReason::ErrorType, initializer));
                    }
                    if matches!(name, "mixedSetter" | "mixedShiftSetter") {
                        causes.push((
                            SemanticFailureReason::UnresolvedSymbol,
                            SOURCE.find("Missing").unwrap(),
                        ));
                    }
                    if name == "brandedMixedSetter" {
                        causes.push((
                            SemanticFailureReason::UnresolvedSymbol,
                            SOURCE.find("SetterMissing").unwrap(),
                        ));
                    }
                    assert_eq!(
                        error.causes.len(),
                        causes.len(),
                        "{profile}/{name}: {error:?}"
                    );
                    for (cause, (reason, offset)) in error.causes.iter().zip(causes) {
                        assert_eq!(cause.reason, reason.into(), "{profile}/{name}: {error:?}");
                        let location = cause
                            .location
                            .as_ref()
                            .expect("native dependency has an actual source site");
                        assert_eq!(location.file, "cases.ts");
                        assert_eq!(
                            location.start,
                            position(SOURCE, offset),
                            "{profile}/{name}: {error:?}"
                        );
                    }
                }
            }
        }
    }
}
