//! Native resolved own tables distinguish name acquisition, symbol merging,
//! broad computed indexes and declaration value demand.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

fn source(file: &str) -> &'static str {
    match file {
        "names.ts" => include_str!("../fixtures/types/native-late-symbol-tables/names.ts"),
        "merges.ts" => include_str!("../fixtures/types/native-late-symbol-tables/merges.ts"),
        "indexes.ts" => include_str!("../fixtures/types/native-late-symbol-tables/indexes.ts"),
        "recovery.ts" => include_str!("../fixtures/types/native-late-symbol-tables/recovery.ts"),
        "owners.ts" => include_str!("../fixtures/types/native-late-symbol-tables/owners.ts"),
        "metadata.ts" => include_str!("../fixtures/types/native-late-symbol-tables/metadata.ts"),
        "meanings.ts" => include_str!("../fixtures/types/native-late-symbol-tables/meanings.ts"),
        other => panic!("unclassified source {other}"),
    }
}

#[test]
fn late_names_keep_exact_keys_owner_kinds_and_static_partition() {
    check("names.ts", false);
}

#[test]
fn rejected_name_syntax_does_not_create_members() {
    check("names.ts", true);
}

#[test]
fn late_and_early_symbols_keep_value_declarations_overloads_and_accessors() {
    check("merges.ts", false);
}

#[test]
fn broad_computed_keys_use_native_index_aggregation_and_explicit_precedence() {
    check("indexes.ts", false);
}

#[test]
fn recursive_tables_publish_early_members_and_keep_nil_recursion_recovery() {
    check("recovery.ts", false);
}

#[test]
fn computed_name_recovery_preserves_later_demands_and_lazy_values() {
    check("recovery.ts", true);
}

#[test]
fn module_and_declaration_owners_keep_late_symbol_acquisition_boundaries() {
    check("owners.ts", false);
}

#[test]
fn late_properties_and_indexes_preserve_optional_and_readonly_metadata() {
    check("metadata.ts", false);
}

#[test]
fn combined_late_value_and_namespace_type_meanings_keep_actual_created_identity() {
    check("meanings.ts", false);
}

fn check(file: &str, secondary: bool) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-late-symbol-tables"),
    );
    let (profiles, count) = match file {
        "owners.ts" => (
            [
                (
                    "owners",
                    include_str!(
                        "../fixtures/types/native-late-symbol-tables/expected-owners.json"
                    ),
                ),
                (
                    "owners-loose",
                    include_str!(
                        "../fixtures/types/native-late-symbol-tables/expected-owners-loose.json"
                    ),
                ),
            ],
            9,
        ),
        "metadata.ts" => (
            [
                (
                    "metadata",
                    include_str!(
                        "../fixtures/types/native-late-symbol-tables/expected-metadata.json"
                    ),
                ),
                (
                    "metadata-loose",
                    include_str!(
                        "../fixtures/types/native-late-symbol-tables/expected-metadata-loose.json"
                    ),
                ),
            ],
            7,
        ),
        "meanings.ts" => (
            [
                (
                    "meanings",
                    include_str!(
                        "../fixtures/types/native-late-symbol-tables/expected-meanings.json"
                    ),
                ),
                (
                    "meanings-loose",
                    include_str!(
                        "../fixtures/types/native-late-symbol-tables/expected-meanings-loose.json"
                    ),
                ),
            ],
            4,
        ),
        _ => (
            [
                (
                    "tsconfig",
                    include_str!(
                        "../fixtures/types/native-late-symbol-tables/expected-tsconfig.json"
                    ),
                ),
                (
                    "loose",
                    include_str!("../fixtures/types/native-late-symbol-tables/expected-loose.json"),
                ),
            ],
            42,
        ),
    };
    for (profile, captured) in profiles {
        let mut rows: Vec<serde_json::Value> = serde_json::from_str(captured).unwrap();
        assert_eq!(rows.len(), count);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            if reverse {
                rows.reverse();
            }
            for row in &rows {
                if row["file"] != file {
                    continue;
                }
                let name = row["name"].as_str().unwrap();
                let second_group = match file {
                    "names.ts" => name.starts_with("nonEntity") && name != "nonEntityUntouched",
                    "recovery.ts" => !name.starts_with("recursive"),
                    _ => false,
                };
                if second_group != secondary {
                    continue;
                }
                let source = source(file);
                let marker = format!("/*#{name}*/");
                let offset = source.find(&marker).unwrap() + marker.len();
                let dependencies = dependencies(name, profile, source, offset);
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                // Final native errorType alone is insufficient: a healthy
                // final property can have consumed an erroneous computed name.
                assert!(
                    !dependencies.is_empty() || row["errorType"] == false,
                    "{profile}/{name}"
                );
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
                    if dependencies.is_empty() {
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
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{label}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{label}: {error:?}"
                    );
                    assert_eq!(error.reason, dependencies[0].0.into(), "{label}: {error:?}");
                    assert_eq!(
                        error.path.as_deref(),
                        Some(fixture.path(file).as_path()),
                        "{label}: {error:?}"
                    );
                    assert_eq!(
                        error.position,
                        Some(position(source, dependencies[0].1)),
                        "{label}: {error:?}"
                    );
                    let mut acquired = vec![0];
                    for cause in &error.causes {
                        let location = cause
                            .location
                            .as_ref()
                            .expect("located native table dependency");
                        assert_eq!(location.file, file, "{label}: {error:?}");
                        let index = dependencies
                            .iter()
                            .position(|&(reason, start, end)| {
                                cause.reason == reason.into()
                                    && location.start == position(source, start)
                                    && location.end == position(source, end)
                            })
                            .unwrap_or_else(|| {
                                panic!("{label}: unexpected native dependency {error:?}")
                            });
                        if !acquired.contains(&index) {
                            acquired.push(index);
                        }
                    }
                    if name == "recursiveNilTable" {
                        // The native capture has two circular recoveries before
                        // the interface-value and absent-property errorTypes.
                        // Reacquiring cached completions preserves their evidence
                        // under the product contract; that circular prefix fills
                        // the public cap before the later native identities.
                        assert_eq!(acquired, vec![0], "{label}: {error:?}");
                        assert_eq!(error.causes.len(), 16, "{label}: {error:?}");
                        assert!(
                            error.omitted_causes.is_some_and(|count| count > 0),
                            "{label}: {error:?}"
                        );
                        continue;
                    }
                    assert_eq!(
                        acquired,
                        (0..dependencies.len()).collect::<Vec<_>>(),
                        "{label}: {error:?}"
                    );
                    assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                }
            }
        }
    }
}

fn dependencies(
    name: &str,
    profile: &str,
    source: &str,
    offset: usize,
) -> Vec<(SemanticFailureReason, usize, usize)> {
    use SemanticFailureReason::{CircularType, ErrorType, UnresolvedSymbol};
    let named = |reason, text: &str| {
        let start = source.find(text).unwrap();
        (reason, start, start + text.len())
    };
    let absent = || {
        let tail = &source[offset..];
        let start = offset + tail.find(" = ").unwrap() + 3;
        let end = start + source[start..].find(';').unwrap();
        let property = start + source[start..end].rfind('.').unwrap() + 1;
        // typeHasStaticProperty consumes the interface-only symbol's own
        // errorType before the actual missing-property recovery at its name.
        vec![(ErrorType, start, end), (ErrorType, property, end)]
    };
    match name {
        "nonEntityCallAbsent" | "nonEntityElementAbsent" | "nonEntityParenAbsent" => absent(),
        "nullableIndexFallback" if profile == "tsconfig" => absent(),
        "recursiveNilTable" => {
            let mut result = vec![named(CircularType, "emptySelfKey")];
            result.extend(absent());
            result
        }
        "nameRecovery" => vec![named(UnresolvedSymbol, "notDeclared")],
        "valueRecoveryDemanded" => vec![named(UnresolvedSymbol, "MissingAnnotation")],
        "accessorSourceRecovery" => vec![named(UnresolvedSymbol, "MissingGetterAnnotation")],
        // The error key is assignable to broad index types, so native index
        // aggregation subsequently reads the computed member's annotation.
        "nameTypeRecovery" => vec![
            named(UnresolvedSymbol, "MissingKeyType"),
            named(UnresolvedSymbol, "MissingSkippedAnnotation"),
        ],
        _ => Vec::new(),
    }
}
