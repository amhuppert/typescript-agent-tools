//! Native class checking distinguishes symbol-declaration inventories, base
//! constructor recovery, and type-reference recovery even when each selected
//! static property retains a healthy literal type in the compiler.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-class-heritage-boundaries/cases.ts");

#[test]
fn class_override_declaration_inventories_keep_native_exemptions() {
    check(0);
}
#[test]
fn class_constructor_boundaries_preserve_recovery_and_static_relations() {
    check(1);
}
#[test]
fn class_implements_type_reference_boundaries_preserve_native_recovery() {
    check(2);
}

fn check(group: u8) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-class-heritage-boundaries"),
    );
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-class-heritage-boundaries/expected-strict.json"),
        ),
        (
            "nocheck",
            include_str!(
                "../fixtures/types/native-class-heritage-boundaries/expected-nocheck.json"
            ),
        ),
        (
            "nodefine",
            include_str!(
                "../fixtures/types/native-class-heritage-boundaries/expected-nodefine.json"
            ),
        ),
        (
            "override",
            include_str!(
                "../fixtures/types/native-class-heritage-boundaries/expected-override.json"
            ),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 20);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let selected = match name {
                    "syntheticInterface"
                    | "syntheticConcrete"
                    | "ordinaryMerged"
                    | "ordinaryInterface"
                    | "mapped"
                    | "declaredOverride"
                    | "abstractOverride"
                    | "abstractImplementation"
                    | "abstractRedeclaration" => 0,
                    "staticIntersection" | "nullBase" | "invalidBase" | "arbitraryBase"
                    | "invalidReturn" => 1,
                    "nonentityImplements"
                    | "optionalImplements"
                    | "arityImplements"
                    | "primitiveImplements"
                    | "unionImplements"
                    | "intersectionImplements" => 2,
                    _ => panic!("unclassified native rule variant {name}"),
                };
                if selected != group {
                    continue;
                }
                let recovery = match name {
                    // checker.getBaseConstructorTypeOfClass installs errorType
                    // after diagnostic 2507. This property has no hook event;
                    // the explicit native assignment proves its recovery.
                    "invalidBase" => Some((
                        SemanticFailureReason::ErrorType,
                        "extends 42",
                        "extends ".len(),
                        "42",
                    )),
                    "nonentityImplements" => Some((
                        SemanticFailureReason::ErrorType,
                        "implements makeBase()",
                        "implements ".len(),
                        "makeBase()",
                    )),
                    "optionalImplements" => Some((
                        SemanticFailureReason::UnresolvedSymbol,
                        "implements holder?.Base",
                        "implements ".len(),
                        "holder",
                    )),
                    "arityImplements" => Some((
                        SemanticFailureReason::ErrorType,
                        "implements Contract<number>",
                        "implements ".len(),
                        "Contract<number>",
                    )),
                    _ => None,
                };
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                assert_eq!(
                    row["acquisitions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|event| event["event"] == "exit" && event["errorType"] == true),
                    matches!(
                        name,
                        "nonentityImplements" | "optionalImplements" | "arityImplements"
                    ),
                    "{profile}/{name}"
                );
                if name == "invalidBase" {
                    assert!(
                        row["codes"]
                            .as_array()
                            .unwrap()
                            .contains(&serde_json::json!(2507))
                    );
                }
                let marker = format!("/*#{name}*/");
                let offset = SOURCE.find(&marker).unwrap() + marker.len();
                for expand in [false, true] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 1,
                    });
                    if let Some((reason, needle, prefix, token)) = recovery {
                        let start = SOURCE.find(needle).unwrap() + prefix;
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(error.reason, reason.into(), "{label}: {error:?}");
                        assert_eq!(
                            error.path.as_deref(),
                            Some(fixture.path("cases.ts").as_path())
                        );
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, start)),
                            "{label}: {error:?}"
                        );
                        assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                        if name != "invalidBase" {
                            // checkTypeReferenceOrImport acquires the node,
                            // then checkClassLikeDeclaration reads its cached
                            // type again: one primary and one later recovery.
                            assert_eq!(error.causes.len(), 1, "{label}: {error:?}");
                        }
                        for cause in &error.causes {
                            assert_eq!(cause.reason, reason.into(), "{label}: {error:?}");
                            let location = cause.location.as_ref().unwrap();
                            assert_eq!(location.file, "cases.ts");
                            assert_eq!(
                                location.start,
                                position(SOURCE, start),
                                "{label}: {error:?}"
                            );
                            assert_eq!(
                                location.end,
                                position(SOURCE, start + token.len()),
                                "{label}: {error:?}"
                            );
                        }
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
