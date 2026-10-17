//! Decorator typing is deferred by the public profile. Class-expression
//! eligibility and immediate helper-prefix gates still follow TypeScript.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-class-decorator-prefix/cases.ts");
const AMBIENT: &str = include_str!("../fixtures/types/native-class-decorator-prefix/ambient.d.ts");

#[test]
fn class_decorator_prefix_respects_options_ambient_and_legacy_eligibility() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-class-decorator-prefix"),
    );
    for (profile, baseline) in [
        (
            "modern",
            include_str!("../fixtures/types/native-class-decorator-prefix/expected-modern.json"),
        ),
        (
            "nocheck",
            include_str!("../fixtures/types/native-class-decorator-prefix/expected-nocheck.json"),
        ),
        (
            "metadata",
            include_str!("../fixtures/types/native-class-decorator-prefix/expected-metadata.json"),
        ),
        (
            "verbatim",
            include_str!("../fixtures/types/native-class-decorator-prefix/expected-verbatim.json"),
        ),
        (
            "native",
            include_str!("../fixtures/types/native-class-decorator-prefix/expected-native.json"),
        ),
        (
            "legacy",
            include_str!("../fixtures/types/native-class-decorator-prefix/expected-legacy.json"),
        ),
        (
            "nohelpers",
            include_str!("../fixtures/types/native-class-decorator-prefix/expected-nohelpers.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 4);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let file = row["file"].as_str().unwrap();
                let source = if file == "ambient.d.ts" {
                    AMBIENT
                } else {
                    SOURCE
                };
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                assert!(
                    !row["acquisitions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|event| event["event"] == "exit" && event["errorType"] == true),
                    "{profile}/{name}"
                );
                // TS5052 excludes metadata without legacy decorators before
                // querying. With legacy decorators, ClassExpression is not a
                // checkDecorators target; modifier diagnostic 1206 is harmless.
                let legacy = matches!(profile, "legacy" | "metadata");
                assert_eq!(
                    row["codes"],
                    if legacy && name != "plain" {
                        serde_json::json!([1206])
                    } else {
                        serde_json::json!([])
                    },
                    "{profile}/{name}"
                );
                let marker = format!("/*#{name}*/");
                let offset = source.find(&marker).unwrap() + marker.len();
                for expand in [false, true] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path(file),
                            position: position(source, offset),
                        },
                        expand,
                        depth: 1,
                    });
                    if name != "plain" && !legacy {
                        let start = source
                            .find(if name == "named" {
                                "@decorate class NamedInner"
                            } else {
                                "@decorate class"
                            })
                            .unwrap();
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::UnsupportedSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::DecoratorTypingRequired.into(),
                            "{label}: {error:?}"
                        );
                        assert_eq!(error.path.as_deref(), Some(fixture.path(file).as_path()));
                        assert_eq!(
                            error.position,
                            Some(position(source, start)),
                            "{label}: {error:?}"
                        );
                        assert!(error.causes.is_empty(), "{label}: {error:?}");
                        assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                    } else {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{label}"
                        );
                        assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                        if let Some(graph) = answer.description.expansion {
                            assert!(
                                graph.nodes.iter().all(|node| !matches!(
                                    node.data,
                                    TypeNodeData::Unsupported { .. }
                                )),
                                "{label}: {graph:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
