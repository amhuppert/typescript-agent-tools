//! Native comparison recovery keeps earlier properties before its depth guard.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::references::Location;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const CASES: [(&str, usize, &str, &str); 9] = [
    (
        "before99",
        99,
        "before",
        include_str!("../fixtures/types/native-relation-recovery/before99.ts"),
    ),
    (
        "after99",
        99,
        "after",
        include_str!("../fixtures/types/native-relation-recovery/after99.ts"),
    ),
    (
        "healthy99",
        99,
        "healthy",
        include_str!("../fixtures/types/native-relation-recovery/healthy99.ts"),
    ),
    (
        "before100",
        100,
        "before",
        include_str!("../fixtures/types/native-relation-recovery/before100.ts"),
    ),
    (
        "after100",
        100,
        "after",
        include_str!("../fixtures/types/native-relation-recovery/after100.ts"),
    ),
    (
        "healthy100",
        100,
        "healthy",
        include_str!("../fixtures/types/native-relation-recovery/healthy100.ts"),
    ),
    (
        "before101",
        101,
        "before",
        include_str!("../fixtures/types/native-relation-recovery/before101.ts"),
    ),
    (
        "after101",
        101,
        "after",
        include_str!("../fixtures/types/native-relation-recovery/after101.ts"),
    ),
    (
        "healthy101",
        101,
        "healthy",
        include_str!("../fixtures/types/native-relation-recovery/healthy101.ts"),
    ),
];

fn request(fixture: &Fixture, source: &str, token: &str, expand: bool) -> TypeRequest {
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(source, source.find(token).unwrap()),
    });
    request.expand = expand;
    request.depth = 2;
    request
}

#[test]
fn property_recovery_precedes_native_overflow_and_later_properties_stay_unconsumed() {
    for profile in ["strict", "loose", "exact"] {
        for (name, depth, order, source) in CASES {
            let config = serde_json::json!({
                "compilerOptions": {
                    "strict": profile != "loose",
                    "strictNullChecks": profile != "loose",
                    "exactOptionalPropertyTypes": profile == "exact",
                    "noLib": true,
                },
                "files": ["cases.ts"],
            })
            .to_string();
            let fixture = Fixture::new(&[("cases.ts", source), ("tsconfig.json", &config)]);
            let project = fixture.load_config("tsconfig.json");
            let overflow = depth >= 100;
            let missing = match order {
                "before" => Some("MissingBefore"),
                "after" if !overflow => Some("MissingAfter"),
                _ => None,
            };
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, source, "selected =", expand));
                if missing.is_none() && !overflow {
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}")
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found);
                    assert_eq!(answer.description.summary, "\"chosen\"");
                    assert!(answer.issues.is_empty());
                } else {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}")
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    if let Some(missing) = missing {
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{profile}/{name}: {error:?}"
                        );
                        assert_eq!(
                            error.position,
                            Some(position(source, source.find(missing).unwrap()))
                        );
                        assert_eq!(
                            error.causes.len(),
                            usize::from(overflow),
                            "{profile}/{name}: {error:?}"
                        );
                        if overflow {
                            let cause = &error.causes[0];
                            assert_eq!(
                                cause.reason,
                                SemanticFailureReason::RelationDepthLimit.into()
                            );
                            let argument = source.rfind("source)").unwrap();
                            assert_eq!(
                                cause.location,
                                Some(Location {
                                    file: "cases.ts".into(),
                                    start: position(source, argument),
                                    end: position(source, argument + "source".len()),
                                })
                            );
                        }
                    } else {
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::RelationDepthLimit.into(),
                            "{profile}/{name}: {error:?}"
                        );
                        assert!(
                            error.causes.is_empty(),
                            "a later property was skipped: {profile}/{name}: {error:?}"
                        );
                    }
                    assert!(error.omitted_causes.is_none());
                }
                let independent =
                    project.type_of(request(&fixture, source, "independent:", expand));
                let Ok(TypeAnswer::Selected(independent)) = independent else {
                    panic!("{profile}/{name}: {independent:?}")
                };
                assert_eq!(independent.outcome, TypeOutcome::Found);
                assert_eq!(independent.description.summary, "string");
                assert!(independent.issues.is_empty());
            }
        }
    }
}
