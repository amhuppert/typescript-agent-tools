//! Combined export-value/type symbols keep fresh identity and lazy declarations.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-combined-alias/cases.ts");
fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-combined-alias"),
    )
}
#[test]
fn combined_aliases_preserve_native_value_and_declared_questions() {
    let fixture = fixture();
    for profile in ["strict", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 13);
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
                    "expression" => TypeQuestion::Expression,
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
                    // Native expression selection normalizes a property-name
                    // token to its complete PropertyAccessExpression.
                    assert_eq!(
                        serde_json::to_value(&answer.target.location).unwrap(),
                        row["location"],
                        "{profile}/{name}"
                    );
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
fn separate_combined_factories_do_not_collapse_identical_origins() {
    use typescript_parser::source::SourcePosition;
    use typescript_parser::types::SelectionFailure;
    let fixture = fixture();
    let project = fixture.load_config("strict.json");
    let answer = project
        .type_of(TypeRequest::new(TypeQuery::Name {
            name: "SharedCombined".into(),
            file: None,
        }))
        .unwrap();
    let TypeAnswer::Unselected(answer) = answer else {
        panic!("fresh combined targets must stay distinct");
    };
    let SelectionFailure::Ambiguous { candidates } = answer.outcome else {
        panic!("both aliases must be found");
    };
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].declarations, candidates[1].declarations);
    assert_eq!(candidates[0].declarations.len(), 2);
    for candidate in candidates {
        assert_eq!(candidate.question, TypeQuestion::Value);
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
            panic!("combined candidate must replay");
        };
        assert_eq!(answer.outcome, TypeOutcome::Found);
        assert!(answer.issues.is_empty());
        assert_eq!(answer.description.summary, "new () => Point");
    }
}
