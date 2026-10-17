//! Missing aliases still resolve to native unknownSymbol after diagnostic-only
//! spelling/export traversal. Diagnostic suppression keeps the same target.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn missing_export_diagnostics_complete_before_unknown_alias_rejection() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-alias-diagnostics"),
    );
    let source = include_str!("../fixtures/types/native-alias-diagnostics/cases.ts");
    for profile in ["strict", "no-check"] {
        let baseline: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        let rows = baseline["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 20);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered: Vec<_> = rows.iter().collect();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let at = position(source, source.find(&marker).unwrap() + marker.len());
                for expand in [false, true, false] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: at,
                        },
                        expand,
                        depth: 0,
                    });
                    if row["unknown"] == false {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found);
                        assert!(answer.issues.is_empty());
                        assert_eq!(answer.description.summary, row["summary"].as_str().unwrap());
                    } else {
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
                            SemanticFailureReason::ErrorType.into(),
                            "{profile}/{name}: {error:?}"
                        );
                        assert_eq!(error.position, Some(at), "{profile}/{name}: {error:?}");
                        assert_eq!(
                            error.path,
                            Some(fixture.path("cases.ts")),
                            "{profile}/{name}: {error:?}"
                        );
                        assert!(error.causes.is_empty(), "{profile}/{name}: {error:?}");
                    }
                }
            }
        }
    }
}
