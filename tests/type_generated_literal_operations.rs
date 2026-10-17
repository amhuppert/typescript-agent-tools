//! Generated literal operations are observed through ordinary final answers.
//! Native process-ID comparisons remain an explicit identity dependency.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-generated-literal-operations/cases.ts");

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-generated-literal-operations"),
    )
}

fn request(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{name}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
        },
        expand,
        depth: 2,
    }
}

#[test]
fn generated_concatenation_casing_and_template_capture_preserve_native_answers() {
    let fixture = fixture();
    for profile in ["strict", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 28);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered: Vec<_> = rows.iter().collect();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                if name == "identityConcreteDigits" {
                    continue;
                }
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert_eq!(row["codes"], serde_json::json!([]), "{profile}/{name}");
                assert_eq!(
                    row["demandFailures"],
                    serde_json::json!([]),
                    "{profile}/{name}"
                );
                for expand in [false, true] {
                    let answer = project.type_of(request(&fixture, name, expand));
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                    };
                    assert_eq!(result.outcome, TypeOutcome::Found);
                    assert_eq!(
                        result.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}/{expand}/{reverse}",
                    );
                    assert!(
                        result.issues.is_empty(),
                        "{profile}/{name}: {:?}",
                        result.issues
                    );
                    if expand {
                        assert!(
                            result
                                .description
                                .expansion
                                .as_ref()
                                .unwrap()
                                .nodes
                                .iter()
                                .all(|node| !matches!(node.data, TypeNodeData::Unsupported { .. }))
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn comparing_captured_identity_to_concrete_digits_remains_an_explicit_dependency() {
    let fixture = fixture();
    let parameter = SOURCE
        .find("static #field = (arg) => /*#identityConcreteDigits*/arg;")
        .unwrap()
        + "static #field = (".len();
    for profile in ["strict", "loose"] {
        let project = fixture.load_config(&format!("{profile}.json"));
        for expand in [false, true, true, false] {
            let answer = project.type_of(request(&fixture, "identityConcreteDigits", expand));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!(
                    "{profile}/{expand}: a hidden decimal spelling cannot prove inequality: {answer:?}"
                );
            };
            assert_eq!(
                error.kind,
                SemanticErrorKind::UnsupportedSemantic,
                "{error:?}"
            );
            assert_eq!(
                error.reason,
                SemanticFailureReason::UnrepresentableIdentity.into(),
                "{error:?}",
            );
            assert_eq!(error.path, Some(fixture.path("cases.ts")));
            assert_eq!(
                error.position,
                Some(position(SOURCE, parameter)),
                "{error:?}"
            );
            for (name, summary) in [
                ("sameIdentity", "\"same\""),
                ("identityNondigit", "\"nondigit\""),
            ] {
                let healthy = project.type_of(request(&fixture, name, expand));
                let Ok(TypeAnswer::Selected(result)) = healthy else {
                    panic!("{profile}/{name}/{expand}: {healthy:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, summary);
                assert!(result.issues.is_empty());
            }
        }
    }
}
