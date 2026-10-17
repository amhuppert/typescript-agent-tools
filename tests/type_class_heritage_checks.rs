//! TS7.0.2 class checks acquire immediate heritage dependencies independently
//! of whole-file diagnostics and the deferred member-body queue.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-class-heritage-checks/cases.ts");

#[test]
fn class_base_accessibility_arguments_and_relations_keep_native_order() {
    check(0);
}
#[test]
fn class_override_kinds_and_initialization_keep_native_gates() {
    check(1);
}
#[test]
fn class_override_modifiers_keep_exact_names_and_option_policy() {
    check(2);
}
#[test]
fn class_implements_checks_continue_after_diagnostics_and_preserve_recovery() {
    check(3);
}
#[test]
fn class_mixins_share_the_constructor_inventory_and_parameter_rule() {
    check(4);
}

fn check(group: u8) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-class-heritage-checks"),
    );
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-class-heritage-checks/expected-strict.json"),
        ),
        (
            "nocheck",
            include_str!("../fixtures/types/native-class-heritage-checks/expected-nocheck.json"),
        ),
        (
            "nodefine",
            include_str!("../fixtures/types/native-class-heritage-checks/expected-nodefine.json"),
        ),
        (
            "override",
            include_str!("../fixtures/types/native-class-heritage-checks/expected-override.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 35);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let selected = match name {
                    "plain" | "privateWithin" | "privateOutside" | "genericGood" | "genericBad"
                    | "genericMissing" | "instanceMismatch" | "staticMismatch" | "classLike"
                    | "unequalReturn" => 0,
                    "inheritedAbstract"
                    | "implementedAbstract"
                    | "propertyToAccessor"
                    | "accessorToProperty"
                    | "methodToAccessor"
                    | "privateOverride"
                    | "uninitializedOverride"
                    | "definiteOverride"
                    | "initializedOverride" => 1,
                    "overrideGood" | "overrideMissing" | "overrideSuggested" | "overrideStatic"
                    | "overrideParameter" | "overrideDynamic" => 2,
                    "implementsGood" | "implementsMismatch" | "implementsAbsent"
                    | "implementsInvalid" | "implementsMissing" | "implementsGeneric"
                    | "implementsBroken" => 3,
                    "mixin" | "invalidMixin" | "abstractMixin" => 4,
                    _ => panic!("unclassified native rule variant {name}"),
                };
                if selected != group {
                    continue;
                }
                let missing = match name {
                    "genericMissing" => Some("MissingBaseArgument"),
                    "implementsInvalid" => Some("string"),
                    "implementsMissing" => Some("MissingContract"),
                    "implementsBroken" => Some("MissingContractType"),
                    _ => None,
                };
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                // Primitive heritage names report 2864 rather than 2304, but
                // resolveTypeReferenceName still installs unresolvedType.
                // Native acquisition events, not diagnostic codes or the
                // selected final literal, prove the semantic recovery.
                assert_eq!(
                    row["acquisitions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|event| event["event"] == "exit" && event["errorType"] == true),
                    missing.is_some(),
                    "{profile}/{name}"
                );
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
                    if name == "initializedOverride" {
                        // Native constructor flow is an explicit profile
                        // boundary; noCheck does not suppress this acquisition.
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        let start = SOURCE
                            .find("class extends PropertyBase { static value: 218")
                            .unwrap();
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::UnsupportedSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::FlowRequired.into(),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.path.as_deref(),
                            Some(fixture.path("cases.ts").as_path())
                        );
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, start)),
                            "{label}: {error:?}"
                        );
                        assert!(error.causes.is_empty(), "{label}: {error:?}");
                        assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                    } else if let Some(missing) = missing {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        // MissingContract is a prefix of MissingContractType;
                        // select the actual heritage use, not the earlier field.
                        let start = if matches!(missing, "MissingContract" | "string") {
                            SOURCE.find(&format!("implements {missing}")).unwrap()
                                + "implements ".len()
                        } else {
                            SOURCE.find(missing).unwrap()
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.path.as_deref(),
                            Some(fixture.path("cases.ts").as_path())
                        );
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, start)),
                            "{label}: {error:?}"
                        );
                        for cause in &error.causes {
                            assert_eq!(
                                cause.reason,
                                SemanticFailureReason::UnresolvedSymbol.into(),
                                "{label}: {error:?}"
                            );
                            let location = cause.location.as_ref().unwrap();
                            assert_eq!(location.file, "cases.ts");
                            assert_eq!(
                                location.start,
                                position(SOURCE, start),
                                "{label}: {error:?}"
                            );
                            assert_eq!(
                                location.end,
                                position(SOURCE, start + missing.len()),
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
