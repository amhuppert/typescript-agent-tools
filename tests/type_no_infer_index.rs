//! NoInfer index acquisition and ordinary conditional-flow substitutions share
//! the native keyof owner, while property values remain demand-driven.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn no_infer_indexes_keep_native_keys_inference_and_demand_boundaries() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-no-infer-index");
    let source = std::fs::read_to_string(root.join("cases.ts")).unwrap();
    let globals = std::fs::read_to_string(root.join("globals.d.ts")).unwrap();
    let strict = std::fs::read_to_string(root.join("strict.json")).unwrap();
    let loose = std::fs::read_to_string(root.join("loose.json")).unwrap();
    let nocheck = std::fs::read_to_string(root.join("nocheck.json")).unwrap();
    let fixture = Fixture::new(&[
        (
            "native-variable-patterns/globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        ("native-no-infer-index/globals.d.ts", &globals),
        ("native-no-infer-index/cases.ts", &source),
        ("native-no-infer-index/strict.json", &strict),
        ("native-no-infer-index/loose.json", &loose),
        ("native-no-infer-index/nocheck.json", &nocheck),
    ]);
    for profile in ["strict", "loose", "nocheck"] {
        let expected: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(expected.len(), 14);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("native-no-infer-index/{profile}.json"));
            let mut rows: Vec<_> = expected.iter().collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert_eq!(row["codes"], serde_json::json!([]), "{profile}/{name}");
                assert_eq!(row["errors"], serde_json::json!([]), "{profile}/{name}");
                let marker = format!("/*#{name}*/");
                let at = position(&source, source.find(&marker).unwrap() + marker.len());
                for expand in [false, true, false] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("native-no-infer-index/cases.ts"),
                            position: at,
                        },
                        expand,
                        depth: 1,
                    });
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{label}: {answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
                    assert_eq!(
                        answer.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{label}"
                    );
                    assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                    assert_eq!(
                        answer.description.expansion.is_some(),
                        expand,
                        "{label}: {answer:?}"
                    );
                }
            }
        }
    }
}
