//! Native getExportOfModule uses getExportsOfSymbol's late class/function
//! priority, while a type-only export-star link is acquired without value use.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn alias_export_selection_uses_actual_late_tables_and_nonforcing_star_links() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-module-export-selection"),
    );
    let source = std::fs::read_to_string(fixture.path("cases.cts")).unwrap();
    for profile in ["node20", "nodenext"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 12);
        // Invalid named-namespace stars and the later value use have separate
        // native diagnostics. This regression owns the seven healthy demands.
        let rows: Vec<_> = rows
            .iter()
            .filter(|row| row["codes"].as_array().unwrap().is_empty())
            .collect();
        assert_eq!(rows.len(), 7);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut rows = rows.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["errorType"], false);
                assert_eq!(row["demandFailures"], serde_json::json!([]));
                let marker = format!("/*#{name}*/");
                for expand in [false, true, false] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.cts"),
                            position: position(
                                &source,
                                source.find(&marker).unwrap() + marker.len(),
                            ),
                        },
                        expand,
                        depth: 0,
                    });
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}: {answer:?}");
                    };
                    assert_eq!(
                        answer.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}: {answer:?}"
                    );
                    assert_eq!(
                        answer.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}"
                    );
                    assert_eq!(
                        serde_json::to_value(&answer.target.location).unwrap(),
                        row["location"],
                        "{profile}/{name}"
                    );
                    assert!(answer.issues.is_empty(), "{profile}/{name}: {answer:?}");
                }
            }
        }
    }
}
