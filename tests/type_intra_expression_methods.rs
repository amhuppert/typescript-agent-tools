//! Context-sensitive methods use their object-element context during native
//! intra-expression inference, before the consumer fixes a type parameter.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn contextual_method_inference_keeps_order_and_native_recovery() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-intra-expression-methods");
    let source = std::fs::read_to_string(root.join("cases.ts")).unwrap();
    for profile in ["strict", "loose", "nocheck"] {
        let config = std::fs::read_to_string(root.join(format!("{profile}.json"))).unwrap();
        let expected: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        let fixture = Fixture::new(&[("tsconfig.json", &config), ("cases.ts", &source)]);
        for reverse in [false, true] {
            let project = fixture.load();
            let mut rows: Vec<_> = expected.iter().collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let at = position(&source, source.find(&marker).unwrap() + marker.len());
                for expand in [false, true, false] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: at,
                        },
                        expand,
                        depth: 0,
                    });
                    if row["errorType"] == true {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{profile}/{name}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{profile}/{name}: {error:?}"
                        );
                        assert_eq!(
                            error.position,
                            Some(position(&source, source.find("missingValue").unwrap())),
                            "{profile}/{name}: {error:?}"
                        );
                    } else {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{profile}/{name}");
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{profile}/{name}"
                        );
                        assert!(answer.issues.is_empty(), "{profile}/{name}: {answer:?}");
                    }
                }
            }
        }
    }
}
