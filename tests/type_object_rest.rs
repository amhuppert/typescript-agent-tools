//! Object rest preserves native property symbols, omission and dependency order.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-object-rest/cases.ts");
const GENERIC: &str = include_str!("../fixtures/types/native-object-rest/generic.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "native-variable-patterns/globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        ("native-object-rest/cases.ts", SOURCE),
        ("native-object-rest/generic.ts", GENERIC),
        (
            "native-object-rest/omit.d.ts",
            include_str!("../fixtures/types/native-object-rest/omit.d.ts"),
        ),
        (
            "native-object-rest/omit-arity.d.ts",
            include_str!("../fixtures/types/native-object-rest/omit-arity.d.ts"),
        ),
        (
            "native-object-rest/omit-kind.d.ts",
            include_str!("../fixtures/types/native-object-rest/omit-kind.d.ts"),
        ),
        (
            "native-object-rest/omit-broken.d.ts",
            include_str!("../fixtures/types/native-object-rest/omit-broken.d.ts"),
        ),
        (
            "native-object-rest/omit-mapped.d.ts",
            include_str!("../fixtures/types/native-object-rest/omit-mapped.d.ts"),
        ),
        (
            "native-object-rest/tsconfig.json",
            include_str!("../fixtures/types/native-object-rest/tsconfig.json"),
        ),
        (
            "native-object-rest/loose.json",
            include_str!("../fixtures/types/native-object-rest/loose.json"),
        ),
        (
            "native-object-rest/exact.json",
            include_str!("../fixtures/types/native-object-rest/exact.json"),
        ),
        (
            "native-object-rest/generic-missing.json",
            include_str!("../fixtures/types/native-object-rest/generic-missing.json"),
        ),
        (
            "native-object-rest/generic-arity.json",
            include_str!("../fixtures/types/native-object-rest/generic-arity.json"),
        ),
        (
            "native-object-rest/generic-kind.json",
            include_str!("../fixtures/types/native-object-rest/generic-kind.json"),
        ),
        (
            "native-object-rest/generic-broken.json",
            include_str!("../fixtures/types/native-object-rest/generic-broken.json"),
        ),
        (
            "native-object-rest/generic-mapped.json",
            include_str!("../fixtures/types/native-object-rest/generic-mapped.json"),
        ),
    ])
}
fn request(fixture: &Fixture, file: &str, source: &str, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path(file),
            position: position(source, source.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 1,
    }
}
fn check(markers: &[&str]) {
    let fixture = fixture();
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-object-rest/expected.json"
    ))
    .unwrap();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-object-rest/{config}.json"));
        for row in rows.iter().filter(|row| {
            row["config"] == config && markers.contains(&row["name"].as_str().unwrap())
        }) {
            let marker = row["name"].as_str().unwrap();
            for expand in [false, true] {
                let answer = project.type_of(request(
                    &fixture,
                    "native-object-rest/cases.ts",
                    SOURCE,
                    marker,
                    expand,
                ));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}")
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}/{expand}: {:?}",
                    result.issues
                );
                assert_eq!(
                    result.description.summary,
                    row["summary"].as_str().unwrap(),
                    "{config}/{marker}/{expand}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{marker}/{expand}: {:?}",
                    result.issues
                );
            }
        }
    }
}
#[test]
fn spread_retains_accessors_methods_optional_flags_and_index_identity() {
    check(&["record", "class", "index"]);
}
#[test]
fn filtering_omission_and_union_mapping_preserve_native_results() {
    check(&["union", "nullable", "any", "empty", "excluded", "falsy"]);
}
#[test]
fn real_omit_alias_and_anonymous_outer_parameters_share_instantiation() {
    check(&[
        "noKeys",
        "generic",
        "parameter",
        "instantiated",
        "assignment",
    ]);
}
#[test]
fn copied_values_and_invalid_spread_sources_keep_their_original_failures() {
    let fixture = fixture();
    let file = fixture
        .path("native-object-rest/cases.ts")
        .canonicalize()
        .unwrap();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-object-rest/{config}.json"));
        for marker in ["copiedFailure", "unknown", "number", "truthy"] {
            let tag = format!("/*#{marker}*/");
            let offset = if marker == "copiedFailure" {
                SOURCE.find("Missing_copied").unwrap()
            } else {
                SOURCE[..SOURCE.find(&tag).unwrap()].rfind("...").unwrap()
            };
            let reason = if marker == "copiedFailure" {
                SemanticFailureReason::UnresolvedSymbol
            } else {
                SemanticFailureReason::ErrorType
            };
            for expand in [false, true] {
                let answer = project.type_of(request(
                    &fixture,
                    "native-object-rest/cases.ts",
                    SOURCE,
                    marker,
                    expand,
                ));
                assert!(
                    matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==reason.into() && error.path.as_deref()==Some(file.as_path()) && error.position==Some(position(SOURCE,offset))),
                    "{config}/{marker}/{expand}: {answer:?}"
                );
            }
        }
    }
}

#[test]
fn empty_omission_skips_global_alias_and_nonempty_omission_consumes_it() {
    let fixture = fixture();
    for kind in ["missing", "arity", "kind", "broken", "mapped"] {
        let project = fixture.load_config(&format!("native-object-rest/generic-{kind}.json"));
        for marker in ["noKeys", "omit"] {
            for expand in [false, true] {
                let answer = project.type_of(request(
                    &fixture,
                    "native-object-rest/generic.ts",
                    GENERIC,
                    marker,
                    expand,
                ));
                if marker == "noKeys" || kind == "mapped" {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{kind}/{marker}/{expand}: {answer:?}")
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{kind}/{marker}/{expand}: {:?}",
                        result.issues
                    );
                    assert_eq!(
                        result.description.summary,
                        if marker == "noKeys" {
                            "T"
                        } else {
                            "Omit<T, \"drop\">"
                        }
                    );
                    assert!(result.issues.is_empty());
                } else {
                    let reason = if kind == "broken" {
                        SemanticFailureReason::UnresolvedSymbol
                    } else {
                        SemanticFailureReason::ErrorType
                    };
                    assert!(
                        matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==reason.into()),
                        "{kind}/{marker}/{expand}: {answer:?}"
                    );
                    if kind == "broken" {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            unreachable!()
                        };
                        let source =
                            include_str!("../fixtures/types/native-object-rest/omit-broken.d.ts");
                        assert_eq!(
                            error.path.as_deref(),
                            Some(
                                fixture
                                    .path("native-object-rest/omit-broken.d.ts")
                                    .canonicalize()
                                    .unwrap()
                                    .as_path()
                            )
                        );
                        assert_eq!(
                            error.position,
                            Some(position(source, source.find("Missing_Omit").unwrap()))
                        );
                    }
                }
            }
        }
    }
}
