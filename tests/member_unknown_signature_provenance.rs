//! A native singleton return has no creation-site declaration provenance.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::SemanticFailureReason;
use typescript_parser::references::{
    Location, ReferenceAnswer, ReferenceFilters, ReferenceOutcome, ReferenceQuery, ReferenceRequest,
};

fn marked_offset(source: &str, marker: &str) -> usize {
    let marker = format!("/*#{marker}*/");
    source.find(&marker).unwrap() + marker.len()
}

fn marked_location(source: &str, marker: &str, text: &str) -> Location {
    let start = marked_offset(source, marker);
    assert!(source[start..].starts_with(text));
    Location {
        file: "cases.ts".to_owned(),
        start: position(source, start),
        end: position(source, start + text.len()),
    }
}

#[test]
fn singleton_recovery_is_located_at_each_consuming_call_after_a_signature_only_prelude() {
    let globals = include_str!("../fixtures/types/native-unknown-signature/globals.d.ts");
    for (variant, source) in [
        (
            "warm",
            include_str!("../fixtures/types/native-unknown-signature/warm.ts"),
        ),
        (
            "cold",
            include_str!("../fixtures/types/native-unknown-signature/cold.ts"),
        ),
        (
            "reordered",
            include_str!("../fixtures/types/native-unknown-signature/reordered.ts"),
        ),
        (
            "derived",
            include_str!("../fixtures/types/native-unknown-signature/derived.ts"),
        ),
    ] {
        let fixture = Fixture::new(&[
            ("cases.ts", source),
            ("globals.d.ts", globals),
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"strict":true,"noLib":true,"types":[]},"files":["globals.d.ts","cases.ts"]}"#,
            ),
            (
                "loose.json",
                r#"{"extends":"./tsconfig.json","compilerOptions":{"strict":false}}"#,
            ),
            (
                "exact.json",
                r#"{"extends":"./tsconfig.json","compilerOptions":{"exactOptionalPropertyTypes":true}}"#,
            ),
        ]);
        for config in ["tsconfig.json", "loose.json", "exact.json"] {
            let project = fixture.load_config(config);
            let answer = project
                .references(ReferenceRequest {
                    query: ReferenceQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(source, marked_offset(source, "seed")),
                    },
                    filters: ReferenceFilters::default(),
                })
                .unwrap();
            let ReferenceAnswer::Selected(result) = answer else {
                panic!("{variant}/{config}: {answer:?}");
            };
            assert_eq!(
                result.outcome,
                ReferenceOutcome::Partial,
                "{variant}/{config}: {result:?}"
            );
            assert_eq!(result.groups.len(), 1, "{variant}/{config}: {result:?}");
            assert_eq!(
                result.groups[0].occurrences.len(),
                2,
                "{variant}/{config}: {result:?}"
            );
            assert_eq!(
                result.coverage.gaps.len(),
                2,
                "{variant}/{config}: {result:?}"
            );
            for (gap, marker, call) in [
                (&result.coverage.gaps[0], "first", "firstCall"),
                (&result.coverage.gaps[1], "second", "secondCall"),
            ] {
                assert_eq!(
                    gap.location,
                    marked_location(source, marker, "probe"),
                    "{variant}/{config}"
                );
                assert_eq!(
                    gap.reason,
                    SemanticFailureReason::ErrorType,
                    "{variant}/{config}: {gap:?}"
                );
                let call_location = marked_location(
                    source,
                    call,
                    if variant == "derived" {
                        "mixed()"
                    } else {
                        "new NotConstructor()"
                    },
                );
                let expected = if variant == "derived" {
                    let start = source.find("mixed(healthy.").unwrap();
                    let end = source[start..].find(';').unwrap() + start;
                    let effects = Location {
                        file: "cases.ts".into(),
                        start: position(source, start),
                        end: position(source, end),
                    };
                    vec![
                        effects.clone(),
                        effects.clone(),
                        call_location.clone(),
                        effects.clone(),
                        effects,
                        call_location,
                    ]
                } else {
                    vec![call_location.clone(), call_location]
                };
                assert_eq!(
                    gap.causes.len(),
                    expected.len(),
                    "{variant}/{config}: {gap:?}"
                );
                for (cause, expected) in gap.causes.iter().zip(expected) {
                    assert_eq!(cause.reason, SemanticFailureReason::ErrorType);
                    assert_eq!(cause.location, Some(expected));
                }
                assert_eq!(gap.omitted_causes, None);
            }
        }
    }
}

#[test]
fn q4_preserves_effect_signature_recovery_even_when_the_native_root_is_healthy() {
    use typescript_parser::analysis::{AnalysisError, SemanticErrorKind};
    use typescript_parser::types::{TypeQuery, TypeRequest};
    let source = include_str!("../fixtures/types/native-unknown-signature/derived.ts");
    let fixture = Fixture::new(&[
        ("cases.ts", source),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-unknown-signature/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true,"types":[]},"files":["globals.d.ts","cases.ts"]}"#,
        ),
    ]);
    let project = fixture.load_config("tsconfig.json");
    let selected = marked_location(source, "selected", "mixed");
    let effects_call = source.find("mixed(healthy.").unwrap();
    for (expand, depth) in [(false, 1), (true, 0), (true, 1), (true, 2)] {
        let mut request = TypeRequest::new(TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: selected.start,
        });
        request.expand = expand;
        request.depth = depth;
        // The native root is Mixed<string>, but getEffectsSignature consumes
        // unknownSignature's return twice while computing its composite predicate.
        let answer = project.type_of(request);
        let Err(AnalysisError::Semantic(error)) = answer else {
            panic!("{expand}/{depth}: {answer:?}");
        };
        assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
        assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
        assert_eq!(error.position, Some(position(source, effects_call)));
        assert_eq!(error.causes.len(), 1);
        assert_eq!(error.causes[0].reason, error.reason);
        assert_eq!(
            error.causes[0].location.as_ref().unwrap().start,
            position(source, effects_call)
        );
    }

    let returned = marked_location(source, "returned", "returned");
    let initializer = source.find("const returned = ").unwrap() + "const returned = ".len();
    assert!(source[initializer..].starts_with("mixed()"));
    for expand in [false, true] {
        let mut request = TypeRequest::new(TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: returned.start,
        });
        request.expand = expand;
        let answer = project.type_of(request);
        let Err(AnalysisError::Semantic(error)) = answer else {
            panic!("returned/{expand}: {answer:?}")
        };
        assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
        assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
        assert_eq!(
            error.path.as_deref(),
            Some(fixture.path("cases.ts").as_path())
        );
        assert_eq!(error.position, Some(position(source, effects_call)));
        let expected = [
            effects_call,
            effects_call,
            effects_call,
            initializer,
            effects_call,
            effects_call,
        ];
        assert_eq!(error.causes.len(), expected.len(), "{error:?}");
        for (cause, offset) in error.causes.iter().zip(expected) {
            assert_eq!(cause.reason, SemanticFailureReason::ErrorType.into());
            assert_eq!(
                cause.location.as_ref().unwrap().start,
                position(source, offset)
            );
        }
        assert_eq!(error.omitted_causes, None);
    }
}
