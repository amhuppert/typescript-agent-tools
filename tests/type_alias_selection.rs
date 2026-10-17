//! Product alias questions use actual checker targets, including created JSON
//! properties. Native baselines own the declared/value and spelling controls.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-alias-selection/cases.ts");

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-alias-selection"),
    )
}

#[test]
fn all_alias_declaration_forms_follow_their_checker_target_and_question() {
    let fixture = fixture();
    for profile in ["strict", "verbatim"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 33);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered: Vec<_> = rows.iter().collect();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let at = position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len());
                assert_eq!(row["errorType"], false);
                assert_eq!(row["demandFailures"], serde_json::json!([]));
                let question = match row["kind"].as_str().unwrap() {
                    "value" => TypeQuestion::Value,
                    "declared" => TypeQuestion::Declared,
                    other => panic!("unexpected alias question: {other}"),
                };
                for expand in [false, true, false] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: at,
                        },
                        expand,
                        depth: 0,
                    });
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                    };
                    assert_eq!(
                        answer.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}: {answer:?}"
                    );
                    assert!(
                        answer.issues.is_empty(),
                        "{profile}/{name}: {:?}",
                        answer.issues
                    );
                    assert_eq!(answer.target.kind, question, "{profile}/{name}");
                    assert_eq!(answer.target.location.start, at, "{profile}/{name}");
                    let expected = row["summary"].as_str().unwrap().replace(
                        "<fixture>",
                        fixture.path("").to_str().unwrap().trim_end_matches('/'),
                    );
                    assert_eq!(answer.description.summary, expected, "{profile}/{name}");
                }
            }
        }
    }
}

#[test]
fn aliases_of_the_same_actual_target_collapse_before_named_selection() {
    let fixture = fixture();
    let project = fixture.load_config("strict.json");
    for (name, summary) in [("Shared", "typeof Widget"), ("SharedJson", "number[]")] {
        let answer = project.type_of(TypeRequest::new(TypeQuery::Name {
            name: name.into(),
            file: None,
        }));
        let Ok(TypeAnswer::Selected(answer)) = answer else {
            panic!("{name}: {answer:?}");
        };
        assert_eq!(answer.outcome, TypeOutcome::Found);
        assert_eq!(answer.target.kind, TypeQuestion::Value);
        assert_eq!(answer.description.summary, summary);
        assert!(answer.issues.is_empty());
        let replay = project
            .type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path(&answer.target.location.file),
                position: answer.target.location.start,
            }))
            .unwrap();
        let TypeAnswer::Selected(replay) = replay else {
            panic!("anchor must replay");
        };
        assert_eq!(replay.target, answer.target);
        assert_eq!(replay.description.summary, summary);
    }
}

#[test]
fn same_alias_target_with_different_questions_stays_ambiguous_and_replays() {
    use typescript_parser::source::SourcePosition;
    use typescript_parser::types::SelectionFailure;
    let fixture = fixture();
    let project = fixture.load_config("strict.json");
    let answer = project
        .type_of(TypeRequest::new(TypeQuery::Name {
            name: "Both".into(),
            file: None,
        }))
        .unwrap();
    let TypeAnswer::Unselected(answer) = answer else {
        panic!("different questions must remain ambiguous");
    };
    let SelectionFailure::Ambiguous { candidates } = answer.outcome else {
        panic!("both aliases must be found");
    };
    assert_eq!(candidates.len(), 2);
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.question)
            .collect::<Vec<_>>(),
        vec![TypeQuestion::Value, TypeQuestion::Declared]
    );
    assert_eq!(candidates[0].declarations, candidates[1].declarations);
    assert_eq!(candidates[0].declarations.len(), 1);
    for candidate in candidates {
        let answer = project
            .type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path(&candidate.anchor.file),
                position: SourcePosition {
                    line: candidate.anchor.line,
                    column: candidate.anchor.column,
                },
            }))
            .unwrap();
        let TypeAnswer::Selected(answer) = answer else {
            panic!("candidate anchor must select");
        };
        assert_eq!(answer.outcome, TypeOutcome::Found);
        assert_eq!(answer.target.kind, candidate.question);
        assert_eq!(
            answer.description.summary,
            if candidate.question == TypeQuestion::Value {
                "typeof Widget"
            } else {
                "Widget"
            }
        );
    }
}

#[test]
fn unknown_aliases_fail_before_preference_or_ambiguity_and_leave_healthy_queries_usable() {
    use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
    let source = include_str!("../fixtures/types/native-alias-selection/broken.ts");
    let fixture = fixture();
    for names in [
        ["missing", "cycle", "healthy"],
        ["healthy", "cycle", "missing"],
    ] {
        let project = fixture.load_config("broken.json");
        for name in names {
            let marker = format!("/*#{name}*/");
            let at = position(source, source.find(&marker).unwrap() + marker.len());
            for expand in [false, true] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("broken.ts"),
                        position: at,
                    },
                    expand,
                    depth: 0,
                });
                if name == "healthy" {
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{name}: {answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found);
                    assert!(answer.issues.is_empty());
                    assert_eq!(answer.description.summary, "\"fixed\"");
                    continue;
                }
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{name}: {answer:?}");
                };
                assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic, "{error:?}");
                assert_eq!(
                    error.reason,
                    SemanticFailureReason::ErrorType.into(),
                    "{error:?}"
                );
                assert_eq!(error.path, Some(fixture.path("broken.ts")), "{error:?}");
                assert_eq!(error.position, Some(at), "{error:?}");
            }
        }
    }
}
