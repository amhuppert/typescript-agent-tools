//! Native normalized/origin filtering is observed through final Project answers.
//! Reversed requests prove isolation, not a same-evaluator warm-cache path.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-filter-origins/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-filter-origins/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-filter-origins/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-filter-origins/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-filter-origins/exact.json"),
        ),
    ])
}

#[test]
fn native_filter_origin_reuse_rebuild_and_discard_survive_final_projection() {
    check(false);
}

#[test]
fn actual_changed_flow_reads_keep_the_existing_flow_boundary() {
    check(true);
}

fn check(flow: bool) {
    let fixture = fixture();
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, baseline) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-filter-origins/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-filter-origins/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-filter-origins/expected-exact.json"),
        ),
    ] {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), 18);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut rows = native.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let flow_row = matches!(
                    name,
                    "equalityFilter"
                        | "discriminantFilter"
                        | "assignmentFilter"
                        | "switchFilter"
                        | "intersectionOriginFilter"
                );
                if flow != flow_row {
                    continue;
                }
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert_eq!(row["codes"], serde_json::json!([]), "{profile}/{name}");
                // Loose null-elision makes equality/default-switch reads retain
                // the original Pair identity. Other selected flow reads change it.
                let flow_required = flow_row
                    && !(profile == "loose" && matches!(name, "equalityFilter" | "switchFilter"));
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: marked(name),
                        },
                        expand,
                        depth: 4,
                    });
                    if flow_required {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::UnsupportedSemantic,
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::FlowRequired.into(),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                        assert_eq!(error.path.as_deref(), Some(path.as_path()));
                        assert_eq!(
                            error.position,
                            Some(marked(name)),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                        assert!(
                            error.causes.is_empty(),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                        assert!(
                            error.omitted_causes.is_none(),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                        continue;
                    }
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}/{expand}: {result:?}"
                    );
                    assert_eq!(
                        result.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}/{expand}"
                    );
                    assert!(
                        result.issues.is_empty(),
                        "{profile}/{name}/{expand}: {result:?}"
                    );
                    if !expand {
                        assert!(result.description.expansion.is_none());
                        continue;
                    }
                    let graph = result
                        .description
                        .expansion
                        .as_ref()
                        .expect("actual expanded query");
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .expect("graph root");
                    assert!(!graph.truncated, "{profile}/{name}: {graph:?}");
                    assert!(
                        graph
                            .nodes
                            .iter()
                            .all(|node| !matches!(node.data, TypeNodeData::Unsupported { .. })),
                        "{profile}/{name}: {graph:?}"
                    );
                    match name {
                        "never" => assert_eq!(root.data, TypeNodeData::Never),
                        "scalarRemoved" => assert_eq!(
                            root.data,
                            if profile == "loose" {
                                TypeNodeData::Undefined
                            } else {
                                TypeNodeData::Never
                            }
                        ),
                        "scalar" => assert_eq!(
                            root.data,
                            TypeNodeData::Primitive {
                                name: PrimitiveName::String
                            }
                        ),
                        _ => {
                            let TypeNodeData::Union {
                                alternatives,
                                omitted_alternatives,
                            } = &root.data
                            else {
                                panic!("{profile}/{name}: {root:?}");
                            };
                            assert!(alternatives.len() >= 2, "{profile}/{name}: {root:?}");
                            assert!(omitted_alternatives.is_none());
                            assert!(
                                graph.nodes.len() > 1,
                                "{profile}/{name}: expanded alternatives retained"
                            );
                        }
                    }
                }
            }
        }
    }
}

fn marked(name: &str) -> SourcePosition {
    let marker = format!("/*#{name}*/");
    let mut matches = SOURCE.match_indices(&marker);
    let (offset, _) = matches.next().unwrap_or_else(|| panic!("missing {marker}"));
    assert!(matches.next().is_none(), "ambiguous {marker}");
    position(SOURCE, offset + marker.len())
}
