//! Private-key literal identity participates in ordinary index applicability.
//! Expected outcomes come from the pinned API and same-checker native capture.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-generated-literal-keys/cases.ts");

#[test]
fn generated_private_keys_preserve_index_template_and_recovery_semantics() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-generated-literal-keys"),
    );
    for profile in ["tsconfig", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 12);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered: Vec<_> = rows.iter().collect();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let selected = SOURCE.find(&marker).unwrap() + marker.len();
                let own = (position(SOURCE, selected), SemanticFailureReason::ErrorType);
                // Native index-info creation reads the annotation before
                // applicability. A failed numeric index then also reports the
                // missing private property; string/template indexes apply.
                let expected = match name {
                    "stringIndex" | "privateSuffix" | "privatePrefix" => Vec::new(),
                    "recoveryString" | "recoveryNumber" | "recoveryTemplate" => {
                        let missing = match name {
                            "recoveryString" => "MissingStringIndex",
                            "recoveryNumber" => "MissingNumberIndex",
                            "recoveryTemplate" => "MissingTemplateIndex",
                            _ => unreachable!(),
                        };
                        let mut events = vec![(
                            position(SOURCE, SOURCE.find(missing).unwrap()),
                            SemanticFailureReason::UnresolvedSymbol,
                        )];
                        if name == "recoveryNumber" {
                            events.push(own);
                        }
                        events
                    }
                    _ => vec![own],
                };
                assert_eq!(row["errorType"], !expected.is_empty(), "{profile}/{name}");
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, selected),
                        },
                        expand,
                        depth: 2,
                    });
                    if expected.is_empty() {
                        let Ok(TypeAnswer::Selected(result)) = answer else {
                            panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                        };
                        assert_eq!(result.outcome, TypeOutcome::Found);
                        assert_eq!(result.description.summary, row["summary"].as_str().unwrap());
                        assert!(result.issues.is_empty());
                        if expand {
                            assert!(
                                result
                                    .description
                                    .expansion
                                    .as_ref()
                                    .unwrap()
                                    .nodes
                                    .iter()
                                    .all(|node| !matches!(
                                        node.data,
                                        TypeNodeData::Unsupported { .. }
                                    ))
                            );
                        }
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
                    assert_eq!(
                        error.reason,
                        expected[0].1.into(),
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(
                        error.position,
                        Some(expected[0].0),
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(
                        error.path.as_deref(),
                        Some(fixture.path("cases.ts").as_path())
                    );
                    let mut observed = vec![0];
                    for cause in &error.causes {
                        let location = cause.location.as_ref().expect("located native recovery");
                        assert_eq!(location.file, "cases.ts");
                        let index = expected
                            .iter()
                            .position(|&(at, reason)| {
                                location.start == at && cause.reason == reason.into()
                            })
                            .unwrap_or_else(|| panic!("unexpected {profile}/{name}: {error:?}"));
                        if !observed.contains(&index) {
                            observed.push(index);
                        }
                    }
                    if name == "recoveryNumber" {
                        // Both private-property inventory consumers replay the
                        // 13 cached per-name completions. The final ErrorType
                        // lies beyond the retained prefix; sparse recovery
                        // below independently proves that final event.
                        assert_eq!(observed, vec![0], "{profile}/{name}: {error:?}");
                        assert_eq!(error.causes.len(), 16, "{profile}/{name}: {error:?}");
                        assert_eq!(
                            error.omitted_causes,
                            Some(41),
                            "{profile}/{name}: {error:?}"
                        );
                    } else {
                        assert_eq!(
                            observed,
                            (0..expected.len()).collect::<Vec<_>>(),
                            "{profile}/{name}: {error:?}"
                        );
                        assert!(error.omitted_causes.is_none());
                    }
                    if expected == vec![own] {
                        assert!(error.causes.is_empty(), "{profile}/{name}: {error:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn recovered_index_display_continues_to_its_own_missing_property_error() {
    const SPARSE: &str = include_str!("../fixtures/types/native-generated-literal-keys/sparse.ts");
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-generated-literal-keys"),
    );
    let marker = "/*#recoveryNumber*/";
    let selected = position(SPARSE, SPARSE.find(marker).unwrap() + marker.len());
    let missing = position(SPARSE, SPARSE.find("MissingNumberIndex").unwrap());
    for profile in ["tsconfig", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-sparse-{profile}.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["errorType"], true);
        assert_eq!(rows[0]["codes"], serde_json::json!([2304, 2339]));
        let project = fixture.load_config(&format!("sparse-{profile}.json"));
        for expand in [false, true, true, false] {
            let answer = project.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("sparse.ts"),
                    position: selected,
                },
                expand,
                depth: 2,
            });
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{profile}/{expand}: {answer:?}");
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic, "{error:?}");
            assert_eq!(
                error.reason,
                SemanticFailureReason::UnresolvedSymbol.into(),
                "{error:?}"
            );
            assert_eq!(error.position, Some(missing));
            assert_eq!(
                error.path.as_deref(),
                Some(fixture.path("sparse.ts").as_path())
            );
            // Composite property/index reads (2), each of two private-name
            // candidates (4), the promise probe (2), object display (1), and
            // the cached suggestion inventory (4) retain 13 missing-type
            // acquisitions, followed by this access's own ErrorType.
            assert_eq!(error.causes.len(), 13, "{profile}/{expand}: {error:?}");
            assert!(error.omitted_causes.is_none(), "{error:?}");
            for (index, cause) in error.causes.iter().enumerate() {
                let location = cause.location.as_ref().expect("located native recovery");
                assert_eq!(location.file, "sparse.ts");
                let (at, reason) = if index == 12 {
                    (selected, SemanticFailureReason::ErrorType)
                } else {
                    (missing, SemanticFailureReason::UnresolvedSymbol)
                };
                assert_eq!(location.start, at, "{profile}/{expand}: {error:?}");
                assert_eq!(cause.reason, reason.into(), "{profile}/{expand}: {error:?}");
            }
        }
    }
}
