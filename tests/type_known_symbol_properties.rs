//! Native isKnownProperty's symbol-name compatibility affects overload choice;
//! its string-index exception does not change element-access index eligibility.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn known_symbol_properties_preserve_native_overload_and_element_access_results() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-known-symbol-properties");
    let source = std::fs::read_to_string(root.join("cases.ts")).unwrap();
    for profile in ["strict", "loose", "checked", "nocheck"] {
        let config = std::fs::read_to_string(root.join(format!("{profile}.json"))).unwrap();
        let expected: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(expected.len(), 15);
        let fixture = Fixture::new(&[("tsconfig.json", &config), ("cases.ts", &source)]);
        for reverse in [false, true] {
            let project = fixture.load();
            let mut rows: Vec<_> = expected.iter().collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                assert_eq!(
                    row["demandFailures"],
                    serde_json::json!([]),
                    "{profile}/{name}"
                );
                let marker = format!("/*#{name}*/");
                let at = position(&source, source.find(&marker).unwrap() + marker.len());
                for expand in [false, true, false] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: at,
                        },
                        expand,
                        depth: 0,
                    });
                    if row["errorType"] == true {
                        assert_eq!(name, "numberElementUnique", "{label}");
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::ErrorType.into(),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.path,
                            Some(fixture.path("cases.ts")),
                            "{label}: {error:?}"
                        );
                    } else {
                        // strings[key] emits native 2538 but returns the index
                        // value type; an ordinary diagnostic is not errorType.
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}");
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{label}"
                        );
                        assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                    }
                }
            }
        }
    }
}
