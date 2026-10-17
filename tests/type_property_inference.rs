mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn inherited_property_types_and_automatic_flow_keep_distinct_outcomes() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-property-inference"),
    );
    let source = include_str!("../fixtures/types/native-property-inference/cases.ts");
    for config in [
        "strict.json",
        "implicit-any.json",
        "loose.json",
        "nocheck.json",
    ] {
        let project = fixture.load_config(config);
        for (offset, _) in source.match_indices("/*#") {
            let end = offset + source[offset..].find("*/").unwrap() + 2;
            let marker = &source[offset + 3..end - 2];
            for expand in [false, true] {
                let label = format!("{config}/{marker}/{expand}");
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(source, end),
                    },
                    expand,
                    depth: 1,
                });
                if config != "implicit-any.json"
                    && matches!(
                        marker,
                        "emptyConstructor"
                            | "assignedConstructor"
                            | "emptyStatic"
                            | "assignedStatic"
                            | "declareWithConstructor"
                            | "staticDeclareWithBlock"
                    )
                {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!(
                            "{label}: automatic inference must retain the F exclusion: {answer:?}"
                        );
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::UnsupportedSemantic,
                        "{label}"
                    );
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::FlowRequired.into(),
                        "{label}: {error:?}"
                    );
                    continue;
                }
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{label}: actual native base-property type required: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{label}: {result:?}");
                assert!(result.issues.is_empty(), "{label}: {result:?}");
                let expected = if config == "implicit-any.json" {
                    "any"
                } else {
                    match marker {
                        "inherited"
                        | "staticSameName"
                        | "instanceWithStaticBlock"
                        | "staticWithConstructor" => "number",
                        "optional" if config == "loose.json" => "number",
                        "optional" => "number | undefined",
                        _ => "any",
                    }
                };
                assert_eq!(result.description.summary, expected, "{label}");
            }
        }
    }
}

#[test]
fn missing_base_property_preserves_consumed_global_recovery() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-property-inference"),
    );
    let project = fixture.load_config("missing-globals.json");
    let source = include_str!("../fixtures/types/native-property-inference/cases.ts");
    for marker in ["missing", "inherited"] {
        let tag = format!("/*#{marker}*/");
        let answer = project.type_of(TypeRequest::new(TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(source, source.find(&tag).unwrap() + tag.len()),
        }));
        if marker == "missing" {
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("missing property must preserve the actual Object fallback: {answer:?}");
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
            assert_eq!(error.causes.len(), 1);
            assert_eq!(
                error.causes[0].reason,
                SemanticFailureReason::UnresolvedSymbol.into()
            );
            assert!(error.causes[0].message.contains("Object"));
        } else {
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("known base property must remain healthy after failure: {answer:?}");
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, "number");
            assert!(result.issues.is_empty());
        }
    }
}
