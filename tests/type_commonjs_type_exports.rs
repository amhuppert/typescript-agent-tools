//! Native binder promotion changes the alias itself before checker lookup.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

#[test]
fn commonjs_type_exports_preserve_native_alias_and_value_selection() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-commonjs-type-exports"),
    );
    for profile in ["strict", "loose"] {
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
                let file = row["file"].as_str().unwrap();
                let source = std::fs::read_to_string(fixture.path(file)).unwrap();
                let marker = format!("/*#{name}*/");
                let at = position(&source, source.find(&marker).unwrap() + marker.len());
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert_eq!(row["codes"], serde_json::json!([]), "{profile}/{name}");
                assert_eq!(
                    row["demandFailures"],
                    serde_json::json!([]),
                    "{profile}/{name}"
                );
                let question = match row["kind"].as_str().unwrap() {
                    "value" => TypeQuestion::Value,
                    "declared" => TypeQuestion::Declared,
                    other => panic!("unexpected module-export question: {other}"),
                };
                for expand in [false, true, false] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path(file),
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
                    assert_eq!(
                        answer.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}"
                    );
                }
            }
        }
    }
}
