//! Native alias-condition gates preserve the original reference across files.
//! Reverse requests establish isolation; sibling conditions prove inline unwind.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-alias-conditions/cases.ts");

#[test]
fn alias_conditions_keep_file_identity_constant_gates_and_inline_depth() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-alias-conditions"),
    );
    for profile in ["tsconfig", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
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
                let dependencies: Vec<_> = row["acquired"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|dependency| {
                        let file = dependency["file"].as_str().unwrap();
                        (file, missing_type(dependency["name"].as_str().unwrap()))
                    })
                    .collect();
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 1,
                    });
                    if dependencies.is_empty() && row["sameDeclared"] == false {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::UnsupportedSemantic,
                            "{profile}/{name}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::FlowRequired.into(),
                            "{profile}/{name}: {error:?}"
                        );
                        let start = if name == "readonlyReference" {
                            SOURCE[..offset].rfind("x.").unwrap()
                        } else {
                            offset
                        };
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, start)),
                            "{profile}/{name}: {error:?}"
                        );
                        assert_eq!(
                            error.path.as_deref(),
                            Some(fixture.path("cases.ts").as_path())
                        );
                        assert!(error.causes.is_empty(), "{profile}/{name}: {error:?}");
                        assert!(
                            error.omitted_causes.is_none(),
                            "{profile}/{name}: {error:?}"
                        );
                        continue;
                    }
                    if dependencies.is_empty() {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found);
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{profile}/{name}"
                        );
                        assert!(answer.issues.is_empty());
                        continue;
                    }
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
                        SemanticFailureReason::UnresolvedSymbol.into(),
                        "{profile}/{name}: {error:?}"
                    );
                    let expected: Vec<_> = dependencies
                        .iter()
                        .map(|&(file, missing)| {
                            let source = source(file);
                            (file, position(source, source.find(missing).unwrap()))
                        })
                        .collect();
                    assert_eq!(
                        error.path.as_deref(),
                        Some(fixture.path(expected[0].0).as_path())
                    );
                    assert_eq!(
                        error.position,
                        Some(expected[0].1),
                        "{profile}/{name}: {error:?}"
                    );
                    // The observer records presence. The only two-dependency
                    // order here follows native && left-then-right evaluation.
                    let mut acquired = vec![0];
                    for cause in &error.causes {
                        assert_eq!(
                            cause.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{profile}/{name}: {error:?}"
                        );
                        let location = cause.location.as_ref().unwrap();
                        let index = expected
                            .iter()
                            .position(|(file, at)| *file == location.file && *at == location.start)
                            .unwrap_or_else(|| panic!("unexpected {profile}/{name}: {error:?}"));
                        if !acquired.contains(&index) {
                            acquired.push(index);
                        }
                    }
                    assert_eq!(
                        acquired,
                        (0..expected.len()).collect::<Vec<_>>(),
                        "{profile}/{name}: {error:?}"
                    );
                    assert!(error.omitted_causes.is_none());
                }
            }
        }
    }
}

fn source(file: &str) -> &'static str {
    match file {
        "cases.ts" => SOURCE,
        "conditions.ts" => include_str!("../fixtures/types/native-alias-conditions/conditions.ts"),
        _ => panic!("unclassified native dependency file {file}"),
    }
}

fn missing_type(name: &str) -> &'static str {
    match name {
        "localRecovery" => "MissingLocal",
        "hiddenRecovery" => "MissingHidden",
        "crossRecovery" => "MissingCross",
        "siblingFirst" => "MissingSiblingFirst",
        "siblingSecond" => "MissingSiblingSecond",
        "depth5" => "MissingDepth5",
        "depth6" => "MissingDepth6",
        _ => panic!("unclassified native dependency {name}"),
    }
}
