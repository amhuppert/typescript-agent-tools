//! Final query admission preserves dependencies acquired by native reports.
mod support;
use std::collections::BTreeSet;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-missing-properties/cases.ts");

// The observer records actual returned error types. Diagnostic-code ordering
// is not an acquisition trace, and an ordinary type display is not an error.
fn demanded_missing_names(row: &serde_json::Value) -> BTreeSet<String> {
    row["displayAcquisitions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| {
            event["event"] == "exit"
                && event["operation"] == "getTypeFromTypeNode"
                && event["errorType"] == true
        })
        .flat_map(|event| {
            let start = event["pos"].as_u64().unwrap() as usize;
            let end = event["end"].as_u64().unwrap() as usize;
            SOURCE
                .get(start..end)
                .unwrap()
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .filter(|word| word.starts_with("Missing"))
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn missing_property_reports_preserve_native_branch_dependencies_and_outcomes() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-missing-properties"),
    );
    for profile in ["tsconfig", "loose"] {
        let expected =
            std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&expected).unwrap();
        assert_eq!(rows.len(), 34);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered: Vec<_> = rows.iter().collect();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let tag = format!("/*#{name}*/");
                let offset = SOURCE.find(&tag).unwrap() + tag.len();
                let missing = demanded_missing_names(row);
                for expand in [false, true] {
                    let request = TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 1,
                    };
                    let answer = project.type_of(request);
                    if row["errorType"] == false && missing.is_empty() {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{name}");
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{name}"
                        );
                        assert!(answer.issues.is_empty(), "{name}: {answer:?}");
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    if !missing.is_empty() {
                        // typeHasStaticProperty asks getTypeOfSymbol first.
                        // TypeLiteral/Interface symbols reach its native
                        // non-value errorType before these display children.
                        let primary = if matches!(
                            name,
                            "anonymousDisplay"
                                | "promiseSuggestion"
                                | "promiseMissing"
                                | "thenableRecovery"
                        ) {
                            SemanticFailureReason::ErrorType
                        } else {
                            SemanticFailureReason::UnresolvedSymbol
                        };
                        assert_eq!(error.reason, primary.into(), "{profile}/{name}: {error:?}");
                        for missing_name in &missing {
                            let expected = position(SOURCE, SOURCE.find(missing_name).unwrap());
                            assert!(
                                error.position == Some(expected)
                                    || error.causes.iter().any(|cause| {
                                        cause.reason
                                            == SemanticFailureReason::UnresolvedSymbol.into()
                                            && cause.location.as_ref().is_some_and(|location| {
                                                location.file == "cases.ts"
                                                    && location.start == expected
                                            })
                                    }),
                                "{profile}/{name}: actual native dependency {missing_name} absent from {error:?}"
                            );
                        }
                    } else if name.ends_with("Heritage") {
                        // getResolvedSymbol asks Value|ExportValue. Neither
                        // the interfaces nor the type-only namespace supply
                        // that meaning. The qualified receiver fails before
                        // property lookup; the later Interface-only diagnostic
                        // changes neither unknownSymbol nor this first cause.
                        let (lookup, start) = match name {
                            "interfaceHeritage" | "shadowedInterfaceHeritage" => {
                                ("BaseInterface", offset)
                            }
                            "qualifiedInterfaceHeritage" => {
                                let prefix = "class InvalidQualifiedExtends extends ";
                                let sites: Vec<_> = SOURCE.match_indices(prefix).collect();
                                assert_eq!(sites.len(), 1);
                                ("InterfaceSpace", sites[0].0 + prefix.len())
                            }
                            other => panic!("unclassified heritage control {other}"),
                        };
                        assert_eq!(&SOURCE[start..start + lookup.len()], lookup);
                        assert_eq!(SOURCE.match_indices(&tag).count(), 1);
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{profile}/{name}: {error:?}"
                        );
                        assert_eq!(
                            error.message,
                            format!("cannot find `{lookup}`"),
                            "{profile}/{name}"
                        );
                        assert_eq!(
                            error.path.as_deref(),
                            Some(fixture.path("cases.ts").as_path()),
                            "{profile}/{name}"
                        );
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, start)),
                            "{profile}/{name}: {error:?}"
                        );
                        // Qualified access checks its receiver again for index lookup;
                        // the cached unresolved result is a second acquisition.
                        if name == "qualifiedInterfaceHeritage" {
                            assert_eq!(error.causes.len(), 1, "{profile}/{name}: {error:?}");
                            assert_eq!(error.causes[0].reason, error.reason);
                            assert_eq!(
                                error.causes[0].location.as_ref().unwrap().start,
                                position(SOURCE, start)
                            );
                        } else {
                            assert!(error.causes.is_empty(), "{profile}/{name}: {error:?}");
                        }
                    } else {
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::ErrorType.into(),
                            "{profile}/{name}: {error:?}"
                        );
                        let property_start = position(SOURCE, offset);
                        let property_end =
                            position(SOURCE, offset + SOURCE[offset..].find(';').unwrap());
                        let is_property_error =
                            |location: &typescript_parser::references::Location| {
                                location.file == "cases.ts"
                                    && location.start == property_start
                                    && location.end == property_end
                            };
                        assert!(
                            error.position == Some(property_start)
                                || error.causes.iter().any(|cause| cause.reason
                                    == SemanticFailureReason::ErrorType.into()
                                    && cause.location.as_ref().is_some_and(is_property_error)),
                            "{profile}/{name}: absent-property error lost its exact name span: {error:?}"
                        );
                        if error.position != Some(property_start) {
                            // Source-free type recovery binds to the actual
                            // whole expression; the later report keeps its name.
                            let start = SOURCE[..offset].rfind('\n').map_or(0, |index| index + 1);
                            assert_eq!(
                                error.path.as_deref(),
                                Some(fixture.path("cases.ts").as_path()),
                                "{profile}/{name}"
                            );
                            assert_eq!(
                                error.position,
                                Some(position(SOURCE, start)),
                                "{profile}/{name}: {error:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
