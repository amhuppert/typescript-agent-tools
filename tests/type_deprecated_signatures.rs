//! Checked deprecated signatures consume the native diagnostic serializer;
//! ordinary and quick paths keep omitted parameter/default dependencies lazy.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-deprecated-signatures/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("native-deprecated-signatures/cases.ts", SOURCE),
        (
            "native-deprecated-signatures/globals.d.ts",
            include_str!("../fixtures/types/native-deprecated-signatures/globals.d.ts"),
        ),
        (
            "native-deprecated-signatures/tsconfig.json",
            include_str!("../fixtures/types/native-deprecated-signatures/tsconfig.json"),
        ),
        (
            "native-deprecated-signatures/loose.json",
            include_str!("../fixtures/types/native-deprecated-signatures/loose.json"),
        ),
        (
            "native-deprecated-signatures/exact.json",
            include_str!("../fixtures/types/native-deprecated-signatures/exact.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("native-deprecated-signatures/cases.ts"),
            position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
        },
        expand,
        depth: 1,
    }
}
#[test]
fn deprecated_checked_calls_retain_exact_hidden_dependency_failures() {
    let fixture = fixture();
    let path = fixture
        .path("native-deprecated-signatures/cases.ts")
        .canonicalize()
        .unwrap();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-deprecated-signatures/{config}.json"));
        for (marker, missing) in [
            ("deprecated", "MissingDeprecated"),
            ("deprecatedDefault", "MissingDeprecatedDefault"),
            ("deprecatedClass", "MissingDeprecatedClass"),
            ("deprecatedNew", "MissingDeprecatedNew"),
            ("overloadDeprecated", "MissingOverload"),
            ("secondTag", "MissingSecondTag"),
            ("tag", "MissingTag"),
        ] {
            let expected = position(SOURCE, SOURCE.find(missing).unwrap());
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                assert!(
                    matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref()==Some(path.as_path()) && error.position==Some(expected)),
                    "{config}/{marker}/{expand}: {answer:?}"
                );
            }
        }
    }
}
#[test]
fn negative_and_quick_deprecation_paths_remain_lazy() {
    let fixture = fixture();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-deprecated-signatures/{config}.json"));
        for (marker, expected) in [
            ("normal", "\"normal\""),
            ("normalDefault", "\"normalDefault\""),
            ("healthy", "\"healthy\""),
            ("notDeprecated", "\"notDeprecated\""),
            ("normalClass", "\"normalClass\""),
            ("overloadNormal", "\"number\""),
            ("inlineCode", "\"inlineCode\""),
            ("fencedCode", "\"fencedCode\""),
            ("hyphenTag", "\"hyphenTag\""),
            ("emailText", "\"emailText\""),
            ("nbspTag", "\"nbspTag\""),
            ("linkText", "\"linkText\""),
            ("quickNormal", "{ value: \"normal\"; }"),
            ("quickDeprecated", "{ value: \"deprecated\"; }"),
            ("quickDeprecatedClass", "DeprecatedClass"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}")
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}: {result:?}"
                );
                assert_eq!(result.description.summary, expected, "{config}/{marker}");
                assert!(result.issues.is_empty(), "{config}/{marker}: {result:?}");
            }
        }
    }
}

#[test]
fn nondeprecated_new_on_call_signature_keeps_native_profile_return_demand() {
    let fixture = fixture();
    let path = fixture
        .path("native-deprecated-signatures/cases.ts")
        .canonicalize()
        .unwrap();
    let expected = position(SOURCE, SOURCE.find("MissingNormalNew").unwrap());
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-deprecated-signatures/{config}.json"));
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, "normalNew", expand));
            if config == "loose" {
                assert!(
                    matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref()==Some(path.as_path()) && error.position==Some(expected)),
                    "{config}/{expand}: {answer:?}"
                );
            } else {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{expand}: {answer:?}")
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}: {result:?}");
                assert_eq!(result.description.summary, "any", "{config}");
                assert!(result.issues.is_empty(), "{config}: {result:?}");
            }
        }
    }
}
