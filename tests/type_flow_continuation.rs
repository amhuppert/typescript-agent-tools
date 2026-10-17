//! Native flow return witnesses prove which erroneous operands are acquired.
//! Reverse requests exercise isolation; recovery order is asserted per query.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-flow-continuation/cases.ts");

#[test]
fn logical_assertion_and_switch_workers_continue_after_recovered_dependencies() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-flow-continuation"),
    );
    for profile in ["tsconfig", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 16);
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
                    .map(|name| missing_type(name.as_str().unwrap()))
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
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, offset)),
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
                        .map(|name| position(SOURCE, SOURCE.find(name).unwrap()))
                        .collect();
                    assert_eq!(
                        error.path.as_deref(),
                        Some(fixture.path("cases.ts").as_path())
                    );
                    assert_eq!(
                        error.position,
                        Some(expected[0]),
                        "{profile}/{name}: {error:?}"
                    );
                    let mut acquired = vec![0];
                    for cause in &error.causes {
                        assert_eq!(
                            cause.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{profile}/{name}: {error:?}"
                        );
                        let location = cause.location.as_ref().unwrap();
                        assert_eq!(location.file, "cases.ts");
                        let index = expected
                            .iter()
                            .position(|at| *at == location.start)
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

fn missing_type(name: &str) -> &'static str {
    match name {
        "andFirst" => "MissingAndFirst",
        "andSecond" => "MissingAndSecond",
        "orFirst" => "MissingOrFirst",
        "orSecond" => "MissingOrSecond",
        "orTrueFirst" => "MissingOrTrueFirst",
        "orTrueSecond" => "MissingOrTrueSecond",
        "andFalseFirst" => "MissingAndFalseFirst",
        "andFalseSecond" => "MissingAndFalseSecond",
        "nestedFirst" => "MissingNestedFirst",
        "nestedSecond" => "MissingNestedSecond",
        "assertionFirst" => "MissingAssertionFirst",
        "assertionSecond" => "MissingAssertionSecond",
        "assertionOrFirst" => "MissingAssertionOrFirst",
        "assertionOrSecond" => "MissingAssertionOrSecond",
        "caseFirst" => "MissingCaseFirst",
        "caseSecond" => "MissingCaseSecond",
        "defaultFirst" => "MissingDefaultFirst",
        "defaultSecond" => "MissingDefaultSecond",
        _ => panic!("unclassified native dependency {name}"),
    }
}
