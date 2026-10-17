//! Native intrinsic identity survives ordinary binding, aliases and flow setup.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-intrinsic-undefined/cases.ts");
const CONSUMER: &str = include_str!("../fixtures/types/native-intrinsic-undefined/consumer.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        ("consumer.ts", CONSUMER),
        (
            "exports.ts",
            include_str!("../fixtures/types/native-intrinsic-undefined/exports.ts"),
        ),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-intrinsic-undefined/globals.d.ts"),
        ),
        (
            "global-type.ts",
            include_str!("../fixtures/types/native-intrinsic-undefined/global-type.ts"),
        ),
        (
            "global-value.ts",
            include_str!("../fixtures/types/native-intrinsic-undefined/global-value.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-intrinsic-undefined/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-intrinsic-undefined/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-intrinsic-undefined/exact.json"),
        ),
        (
            "global-type.json",
            include_str!("../fixtures/types/native-intrinsic-undefined/global-type.json"),
        ),
        (
            "global-value.json",
            include_str!("../fixtures/types/native-intrinsic-undefined/global-value.json"),
        ),
    ])
}
fn request(fixture: &Fixture, file: &str, source: &str, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path(file),
        position: position(source, source.find(&tag).unwrap() + tag.len()),
    });
    request.expand = expand;
    request
}
#[test]
fn intrinsic_reads_widening_and_shadow_symbols_keep_native_values() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        let loose = config == "loose.json";
        for (marker, expected) in [
            ("direct", "undefined"),
            ("constant", if loose { "any" } else { "undefined" }),
            ("automatic", "any"),
            ("parenthesized", "any"),
            ("queried", if loose { "any" } else { "undefined" }),
            ("shadow", "number"),
            ("shadowVariable", "number"),
            ("localTypeOnly", if loose { "any" } else { "undefined" }),
            ("declaredAlias", "1"),
        ] {
            let (file, source) = if marker == "declaredAlias" {
                ("consumer.ts", CONSUMER)
            } else {
                ("cases.ts", SOURCE)
            };
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, file, source, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(result.description.summary, expected, "{config}/{marker}");
                assert!(
                    result.issues.is_empty(),
                    "{config}/{marker}: {:?}",
                    result.issues
                );
            }
        }
    }
}
#[test]
fn intrinsic_automatic_initializer_keeps_the_existing_flow_boundary() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let answer = project.type_of(request(
                &fixture,
                "cases.ts",
                SOURCE,
                "automaticRead",
                expand,
            ));
            if config == "loose.json" {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, "any");
            } else {
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error))
                    if error.kind == SemanticErrorKind::UnsupportedSemantic
                    && error.reason == SemanticFailureReason::FlowRequired.into()),
                    "{config}: {answer:?}"
                );
            }
        }
    }
}
#[test]
fn global_entries_and_invalid_value_uses_do_not_turn_into_the_intrinsic() {
    let fixture = fixture();
    for (config, file, source, marker, healthy) in [
        (
            "global-type.json",
            "global-type.ts",
            include_str!("../fixtures/types/native-intrinsic-undefined/global-type.ts"),
            "globalType",
            false,
        ),
        (
            "global-value.json",
            "global-value.ts",
            include_str!("../fixtures/types/native-intrinsic-undefined/global-value.ts"),
            "globalValue",
            true,
        ),
        ("tsconfig.json", "cases.ts", SOURCE, "assignment", false),
        ("tsconfig.json", "consumer.ts", CONSUMER, "imported", false),
        ("loose.json", "cases.ts", SOURCE, "assignment", false),
        ("loose.json", "consumer.ts", CONSUMER, "imported", false),
        ("exact.json", "cases.ts", SOURCE, "assignment", false),
        ("exact.json", "consumer.ts", CONSUMER, "imported", false),
    ] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, file, source, marker, expand));
            if healthy {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, "string");
            } else {
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error))
                    if error.kind == SemanticErrorKind::InvalidSemantic),
                    "{config}/{marker}: {answer:?}"
                );
            }
        }
    }
}
