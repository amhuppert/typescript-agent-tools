//! Native ordinary Any fallback is consumed even when noImplicitAny is off;
//! an already-Any receiver and existing global properties remain healthy.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-global-this-missing/cases.ts");
#[test]
fn missing_global_this_access_keeps_consumed_any_separate_from_genuine_any() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-global-this-missing"),
    );
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-global-this-missing/expected-strict.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-global-this-missing/expected-loose.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 10);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut rows = rows.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let offset = SOURCE.find(&marker).unwrap() + marker.len();
                let fallback = !matches!(name, "actualValue" | "actualFunction" | "genuineAny");
                assert_eq!(row["errorType"], false);
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 2,
                    });
                    if fallback {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
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
                        assert_eq!(error.path.as_deref(), Some(path.as_path()));
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, offset)),
                            "{profile}/{name}: {error:?}"
                        );
                        assert!(error.causes.is_empty(), "{profile}/{name}: {error:?}");
                        assert!(error.omitted_causes.is_none());
                    } else {
                        let Ok(TypeAnswer::Selected(result)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(
                            result.outcome,
                            TypeOutcome::Found,
                            "{profile}/{name}: {result:?}"
                        );
                        assert_eq!(result.description.summary, row["summary"].as_str().unwrap());
                        assert!(result.issues.is_empty());
                        if expand {
                            let graph = result.description.expansion.as_ref().unwrap();
                            assert!(graph.nodes.iter().all(|node| !matches!(
                                node.data,
                                TypeNodeData::Unsupported { .. }
                            )));
                        }
                    }
                }
            }
        }
    }
}
