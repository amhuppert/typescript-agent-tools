//! Cloned signature constraints preserve completed absence and its failures.
//! Native diagnostic arrays prove acquisition, not their checker call order.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-cloned-constraint-recovery/cases.ts");

#[test]
fn cloned_circular_and_missing_constraints_keep_later_argument_failures() {
    check(&[
        "circularTyped",
        "circularInferred",
        "circularWithoutTailFailure",
        "mutualTyped",
        "missingTyped",
    ]);
}

#[test]
fn absent_and_instantiated_constraints_preserve_healthy_and_failed_arguments() {
    check(&[
        "unconstrainedLater",
        "mappedBoundLater",
        "unconstrainedHealthy",
        "mappedBoundHealthy",
        "healthyAfter",
    ]);
}

fn check(names: &[&str]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-cloned-constraint-recovery/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-cloned-constraint-recovery/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-cloned-constraint-recovery/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-cloned-constraint-recovery/exact.json"),
        ),
    ]);
    let file = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!(
                "../fixtures/types/native-cloned-constraint-recovery/expected-tsconfig.json"
            ),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-cloned-constraint-recovery/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-cloned-constraint-recovery/expected-exact.json"),
        ),
    ] {
        let baseline: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(baseline.len(), 10);
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
                assert_eq!(row["summary"], "number");
                assert_eq!(row["errorType"], false);
                let expected = dependencies(name);
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: marked('#', name),
                    });
                    request.expand = expand;
                    request.depth = 2;
                    let answer = project.type_of(request);
                    if expected.is_empty() {
                        assert_eq!(row["codes"], serde_json::json!([]));
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found);
                        assert_eq!(answer.description.summary, "number");
                        assert!(answer.issues.is_empty());
                        continue;
                    }
                    assert!(!row["codes"].as_array().unwrap().is_empty());
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(error.path.as_deref(), Some(file.as_path()));
                    assert_eq!(
                        error.reason,
                        expected[0].0.into(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(
                        error.position,
                        Some(marked('@', expected[0].1)),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    let mut acquired = vec![0];
                    for cause in &error.causes {
                        let location = cause
                            .location
                            .as_ref()
                            .expect("actual constraint/argument dependency location");
                        assert_eq!(location.file, "cases.ts");
                        let index = expected
                            .iter()
                            .position(|(reason, site)| {
                                cause.reason == (*reason).into()
                                    && location.start == marked('@', site)
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

fn dependencies(name: &str) -> Vec<(SemanticFailureReason, &'static str)> {
    use SemanticFailureReason::{CircularType, UnresolvedSymbol};
    match name {
        "circularTyped" | "circularInferred" => {
            vec![(CircularType, "circular"), (UnresolvedSymbol, "later")]
        }
        "circularWithoutTailFailure" => vec![(CircularType, "circular")],
        // The recursive U push supplies circularConstraintType before either
        // diagnostic pop. getNextBaseConstraint consumes that sentinel first;
        // V's later failed pop must not replace its original dependency.
        "mutualTyped" => vec![
            (CircularType, "mutualU"),
            (CircularType, "mutualV"),
            (UnresolvedSymbol, "later"),
        ],
        "missingTyped" => vec![(UnresolvedSymbol, "missing"), (UnresolvedSymbol, "later")],
        "unconstrainedLater" | "mappedBoundLater" => vec![(UnresolvedSymbol, "later")],
        "unconstrainedHealthy" | "mappedBoundHealthy" | "healthyAfter" => vec![],
        _ => panic!("unclassified native row {name}"),
    }
}

fn marked(kind: char, name: &str) -> typescript_parser::source::SourcePosition {
    let marker = format!("/*{kind}{name}*/");
    let mut sites = SOURCE.match_indices(&marker);
    let (offset, _) = sites.next().unwrap_or_else(|| panic!("missing {marker}"));
    assert!(sites.next().is_none(), "ambiguous {marker}");
    position(SOURCE, offset + marker.len())
}
