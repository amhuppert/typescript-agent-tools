//! `this` contexts preserve native values even when diagnostics report usage.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn this_expressions_follow_native_container_and_contextual_type_rules() {
    let sources = [
        (
            "cases.ts",
            include_str!("../fixtures/types/native-this-expressions/cases.ts"),
        ),
        (
            "script.ts",
            include_str!("../fixtures/types/native-this-expressions/script.ts"),
        ),
    ];
    for (profile, captured) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-this-expressions/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-this-expressions/expected-loose.json"),
        ),
    ] {
        let fixture = Fixture::new(&[
            sources[0],
            sources[1],
            (
                "globals.d.ts",
                include_str!("../fixtures/types/native-this-expressions/globals.d.ts"),
            ),
            (
                "tsconfig.json",
                include_str!("../fixtures/types/native-this-expressions/tsconfig.json"),
            ),
            (
                "loose.json",
                include_str!("../fixtures/types/native-this-expressions/loose.json"),
            ),
        ]);
        let project = fixture.load_config(&format!("{profile}.json"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(captured).unwrap();
        assert_eq!(rows.len(), 23);
        for row in rows {
            let name = row["name"].as_str().unwrap();
            let file = row["file"].as_str().unwrap();
            let source = sources.iter().find(|(path, _)| *path == file).unwrap().1;
            let marker = format!("/*#{name}*/");
            let offset = source.find(&marker).unwrap() + marker.len();
            assert_eq!(row["errorType"], false, "{profile}/{name}");
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path(file),
                    position: position(source, offset),
                });
                request.expand = expand;
                request.depth = 0;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(answer)) = answer else {
                    panic!("{profile}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    answer.outcome,
                    TypeOutcome::Found,
                    "{profile}/{name}/{expand}: {answer:?}"
                );
                assert_eq!(
                    answer.description.summary,
                    row["summary"].as_str().unwrap(),
                    "{profile}/{name}/{expand}"
                );
                assert!(
                    answer.issues.is_empty(),
                    "{profile}/{name}/{expand}: {answer:?}"
                );
            }
        }
    }
}
