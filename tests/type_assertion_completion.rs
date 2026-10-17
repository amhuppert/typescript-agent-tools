//! Measured assertion answers plus native checkAssertion/checkSatisfies order.
//! Request repetition tests isolation, not same-checker cache replay.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-assertion-completion/cases.ts");

#[test]
fn assertions_and_satisfies_preserve_native_values_after_diagnostic_only_branches() {
    check(false);
}

#[test]
fn recovered_operands_and_annotations_follow_each_native_operation_order() {
    check(true);
}

fn check(recovery: bool) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-assertion-completion/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-assertion-completion/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-assertion-completion/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-assertion-completion/exact.json"),
        ),
        (
            "erasable.json",
            include_str!("../fixtures/types/native-assertion-completion/erasable.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, baseline) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-assertion-completion/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-assertion-completion/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-assertion-completion/expected-exact.json"),
        ),
        (
            "erasable",
            include_str!("../fixtures/types/native-assertion-completion/expected-erasable.json"),
        ),
    ] {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), if profile == "erasable" { 1 } else { 11 });
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut rows = native.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let expected = dependencies(name);
                if recovery == expected.is_empty() {
                    continue;
                }
                assert_eq!(row["errorType"], recovery, "{profile}/{name}");
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: marked('#', name),
                        },
                        expand,
                        depth: 2,
                    });
                    if expected.is_empty() {
                        // TS1355/1360/1294 report diagnostics without replacing
                        // the native value or supplying semantic child failure.
                        let Ok(TypeAnswer::Selected(result)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(result.outcome, TypeOutcome::Found);
                        assert_eq!(
                            result.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{profile}/{name}/{expand}"
                        );
                        assert!(
                            result.issues.is_empty(),
                            "{profile}/{name}/{expand}: {result:?}"
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
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::UnresolvedSymbol.into(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(error.path.as_deref(), Some(path.as_path()));
                    assert_eq!(
                        error.position,
                        Some(expected[0]),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    let mut acquired = vec![0];
                    for cause in &error.causes {
                        assert_eq!(
                            cause.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                        let location = cause.location.as_ref().expect("actual dependency location");
                        assert_eq!(location.file, "cases.ts");
                        let index = expected
                            .iter()
                            .position(|at| *at == location.start)
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

fn dependencies(name: &str) -> Vec<SourcePosition> {
    let names: &[&str] = match name {
        // checker12256 acquires the operand before either annotation syntax.
        "asRecovery" => &["asOperand", "asTarget"],
        "angleRecovery" => &["angleOperand", "angleTarget"],
        "constRecovery" => &["constOperand"],
        // checker10715 checks type syntax before checking the expression.
        "satisfiesSourceFirst" => &["satisfiesTarget", "satisfiesOperand"],
        "satisfiesErrorTarget" => &["errorTarget", "errorOperand"],
        "asHealthy" | "angleHealthy" | "constLiteral" | "constInvalid" | "satisfiesFalse"
        | "healthyAfter" => &[],
        _ => panic!("unclassified native assertion row {name}"),
    };
    names.iter().map(|name| marked('@', name)).collect()
}

fn marked(kind: char, name: &str) -> SourcePosition {
    let marker = format!("/*{kind}{name}*/");
    let mut matches = SOURCE.match_indices(&marker);
    let (offset, _) = matches.next().unwrap_or_else(|| panic!("missing {marker}"));
    assert!(matches.next().is_none(), "ambiguous {marker}");
    position(SOURCE, offset + marker.len())
}
