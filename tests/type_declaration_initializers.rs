//! Final source-check/reporting outcomes from the native declaration matrix.
//! Diagnostic codes prove membership; ordering follows checker5769–5937.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-declaration-initializers/cases.ts");

#[test]
fn declaration_initializer_diagnostics_keep_actual_healthy_values() {
    check(false);
}

#[test]
fn declaration_initializer_children_and_reporting_preserve_ordered_recovery() {
    check(true);
}

fn check(recovery: bool) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-declaration-initializers/globals.d.ts"),
        ),
        (
            "protocol.d.ts",
            include_str!("../fixtures/types/native-declaration-initializers/protocol.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-declaration-initializers/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-declaration-initializers/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-declaration-initializers/exact.json"),
        ),
    ]);
    let file = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, baseline) in [
        (
            "tsconfig",
            include_str!(
                "../fixtures/types/native-declaration-initializers/expected-tsconfig.json"
            ),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-declaration-initializers/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-declaration-initializers/expected-exact.json"),
        ),
    ] {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), 25);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut rows = native.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let dependencies = dependencies(name, profile);
                let nullable_pattern = name == "emptyObjectNull" && profile != "loose";
                if recovery != (nullable_pattern || !dependencies.is_empty()) {
                    continue;
                }
                // The containing signature/any result is healthy in native;
                // consumed child error recovery still fails this Project query.
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                let checks = row["checks"].as_array().unwrap();
                assert_eq!(
                    checks.is_empty(),
                    name == "quickControl",
                    "{profile}/{name}: actual native worker entry"
                );
                if matches!(name, "secondaryMismatch" | "secondaryDisplay") {
                    assert!(
                        checks.iter().any(|check| check["primary"] == false),
                        "{profile}/{name}: actual secondary declaration"
                    );
                }
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: marked('#', name),
                        },
                        expand,
                        depth: 4,
                    });
                    if !recovery {
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
                        if expand {
                            let graph = result
                                .description
                                .expansion
                                .as_ref()
                                .expect("expanded query");
                            assert!(graph.nodes.iter().any(|node| node.id == graph.root));
                            assert!(
                                graph.nodes.iter().all(|node| !matches!(
                                    node.data,
                                    TypeNodeData::Unsupported { .. }
                                )),
                                "{profile}/{name}: {graph:?}"
                            );
                        } else {
                            assert!(result.description.expansion.is_none());
                        }
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(error.path.as_deref(), Some(file.as_path()));
                    if nullable_pattern {
                        // Native strict checkNonNullType(null, parameter) returns
                        // actual errorType before the later widened-object check.
                        let first = checks
                            .iter()
                            .find(|check| check["name"] == "variableLikeEntry:1")
                            .unwrap();
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::ErrorType.into(),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, first["pos"].as_u64().unwrap() as usize))
                        );
                        assert!(
                            error.causes.is_empty(),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                    } else {
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                        assert_eq!(
                            error.position,
                            Some(dependencies[0]),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                        let mut acquired = vec![0];
                        for cause in &error.causes {
                            assert_eq!(
                                cause.reason,
                                SemanticFailureReason::UnresolvedSymbol.into(),
                                "{profile}/{name}/{expand}: {error:?}"
                            );
                            let location =
                                cause.location.as_ref().expect("actual dependency location");
                            assert_eq!(location.file, "cases.ts");
                            let index = dependencies
                                .iter()
                                .position(|at| *at == location.start)
                                .unwrap_or_else(|| {
                                    panic!("unexpected {profile}/{name}/{expand}: {error:?}")
                                });
                            if !acquired.contains(&index) {
                                acquired.push(index);
                            }
                        }
                        assert_eq!(
                            acquired,
                            (0..dependencies.len()).collect::<Vec<_>>(),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                    }
                    assert!(
                        error.omitted_causes.is_none(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                }
            }
        }
    }
}

fn dependencies(name: &str, profile: &str) -> Vec<SourcePosition> {
    let markers: &[&str] = match name {
        "primaryRecovery" => &["primaryAnnotation", "primaryInitializer"],
        "primaryReportDisplay" => &["primaryDisplay"],
        "recoveryThenLater" => &[
            "firstAnnotation",
            "firstInitializer",
            "lastAnnotation",
            "lastInitializer",
        ],
        "nestedPattern" => &["nestedInitializer"],
        "computedRecovery" => &["computedName"],
        "bodylessAnnotation" => &["bodylessParameter", "bodylessReturn"],
        "literalMembers" => &["propertyFirst", "propertyLast"],
        "secondaryDisplay" => &["secondaryDisplay"],
        // Child binding checks precede the parent bodyless exit. Under strict
        // null checking they acquire the initializer in the parent type owner.
        "bodylessPatternSkip" if profile != "loose" => {
            return vec![position(
                SOURCE,
                SOURCE.find("MissingSkippedPattern").unwrap(),
            )];
        }
        "primaryValid"
        | "primaryMismatch"
        | "objectValid"
        | "objectMismatch"
        | "computedBinding"
        | "emptyObject"
        | "emptyObjectNull"
        | "emptyArray"
        | "omittedArray"
        | "restIdentifier"
        | "restPattern"
        | "bodylessSkip"
        | "bodylessPatternSkip"
        | "bodylessRenamed"
        | "secondaryMismatch"
        | "quickControl"
        | "healthyAfter" => &[],
        _ => panic!("unclassified declaration row {name}"),
    };
    markers.iter().map(|name| marked('@', name)).collect()
}

fn marked(kind: char, name: &str) -> SourcePosition {
    let marker = format!("/*{kind}{name}*/");
    let mut matches = SOURCE.match_indices(&marker);
    let (offset, _) = matches.next().unwrap_or_else(|| panic!("missing {marker}"));
    assert!(matches.next().is_none(), "ambiguous {marker}");
    position(SOURCE, offset + marker.len())
}
