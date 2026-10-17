//! GetSymbolAtLocation's syntax branches share actual symbol identity with
//! identifier value reads. Expected ranges and types come from native queries.
mod support;
use serde_json::{Value, json};
use support::{Fixture, position};
use typescript_parser::references::{
    ReferenceAnswer, ReferenceBaseline, ReferenceFilters, ReferenceOutcome, ReferenceQuery,
    ReferenceRequest,
};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-symbol-location"),
    )
}

fn baseline(fixture: &Fixture, profile: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
    )
    .unwrap()
}

#[test]
fn symbol_location_and_actual_variable_reads_preserve_native_types() {
    let fixture = fixture();
    for profile in ["tsconfig", "loose", "exact"] {
        let native = baseline(&fixture, profile);
        let rows = native["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 11);
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
                assert_eq!(row["errorType"], false);
                assert_eq!(row["demandFailures"], json!([]));
                // Ordinary duplicate-symbol diagnostics in the late/raw
                // control do not introduce a native errorType acquisition.
                for expand in [false, true, false] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path(file),
                            position: at,
                        },
                        expand,
                        depth: 2,
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
                    assert_eq!(
                        answer.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}/{expand}/{reverse}"
                    );
                }
            }
        }
    }
}

#[test]
fn member_search_visits_binding_predicate_indexed_and_private_roles() {
    let fixture = fixture();
    let source = std::fs::read_to_string(fixture.path("cases.ts")).unwrap();
    for profile in ["tsconfig", "loose", "exact"] {
        let native = baseline(&fixture, profile);
        let rows = native["members"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered: Vec<_> = rows.iter().collect();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*@{name}*/");
                assert_eq!(row["demandFailures"], json!([]));
                let answer = project.references(ReferenceRequest {
                    query: ReferenceQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(&source, source.find(&marker).unwrap() + marker.len()),
                    },
                    filters: ReferenceFilters::default(),
                });
                let Ok(ReferenceAnswer::Selected(answer)) = answer else {
                    panic!("{profile}/{name}/{reverse}: {answer:?}");
                };
                assert_eq!(
                    answer.outcome,
                    ReferenceOutcome::Complete,
                    "{profile}/{name}: {answer:?}"
                );
                assert_eq!(answer.coverage.baseline, ReferenceBaseline::Member);
                assert!(
                    answer.coverage.gaps.is_empty(),
                    "{profile}/{name}: {answer:?}"
                );
                let actual: Vec<_> = answer.groups.iter().map(|group| json!({
                    "origins": group.declarations.iter().map(|declaration| &declaration.location).collect::<Vec<_>>(),
                    "occurrences": group.occurrences.iter().map(|occurrence| &occurrence.location).collect::<Vec<_>>(),
                })).collect();
                let expected: Vec<_> = row["groups"].as_array().unwrap().iter().map(|group| json!({
                    "origins": group["origins"].as_array().unwrap().iter().map(|origin| &origin["location"]).collect::<Vec<_>>(),
                    "occurrences": group["occurrences"],
                })).collect();
                assert_eq!(actual, expected, "{profile}/{name}/{reverse}");
            }
        }
    }
}
