//! Native keyword accessibility diagnostics preserve the property type.
//! The maintained compiler matrix covers the actual reporting branches; query
//! order below establishes request isolation, not a shared warm checker cache.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-modifier-access/cases.ts");

#[test]
fn private_protected_and_abstract_diagnostics_preserve_native_property_types() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-modifier-access"),
    );
    for (profile, expected) in [
        (
            "strict",
            include_str!("../fixtures/types/native-modifier-access/expected-strict.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-modifier-access/expected-loose.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(rows.len(), 28);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut rows = rows.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let offset = SOURCE.find(&marker).unwrap() + marker.len();
                // The compiler does report errors for invalid accessibility,
                // but this operation returns the actual property type rather
                // than its errorType or an invented Any recovery value.
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                let diagnostic = match name {
                    "privateDerived" | "privateOutside" | "privateStaticOutside" => Some(2341),
                    "protectedThroughBase" => Some(2446),
                    "compositeOneWrong"
                    | "protectedOutside"
                    | "protectedStaticOutside"
                    | "setter"
                    | "explicitThisStatic" => Some(2445),
                    "abstractPropertyConstructor" => Some(2715),
                    "superPublicField" => Some(2855),
                    "superAbstractMethod" => Some(2513),
                    _ => None,
                };
                let codes = row["codes"].as_array().unwrap();
                if let Some(code) = diagnostic {
                    assert!(
                        codes.iter().any(|actual| actual.as_u64() == Some(code)),
                        "{profile}/{name}: {row}"
                    );
                    assert!(
                        !row["displayEvents"].as_array().unwrap().is_empty(),
                        "{profile}/{name}"
                    );
                } else {
                    assert!(codes.is_empty(), "{profile}/{name}: {row}");
                }
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 2,
                    });
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}: {result:?}"
                    );
                    assert_eq!(
                        result.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}"
                    );
                    assert!(result.issues.is_empty(), "{profile}/{name}: {result:?}");
                    if expand {
                        let graph = result.description.expansion.as_ref().unwrap();
                        assert!(
                            graph
                                .nodes
                                .iter()
                                .all(|node| !matches!(node.data, TypeNodeData::Unsupported { .. })),
                            "{profile}/{name}: {graph:?}"
                        );
                    }
                }
            }
        }
    }
}
