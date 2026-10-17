//! Broad computed class-member fallback results captured in independent native sessions.
use super::*;

fn cases() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-late-symbol-cache-fallback/expected.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 16);
    for row in rows {
        let variant = &row["variant"];
        let label = variant["config"].as_str().unwrap();
        let file = variant["file"].as_str().unwrap();
        let source = row["source"].as_str().unwrap();
        let mut fixture = Fixture::new("let chosen: number = 1;");
        for (name, contents) in [
            (
                "globals.d.ts",
                include_str!(
                    "../../../../fixtures/types/native-late-symbol-cache-fallback/globals.d.ts"
                ),
            ),
            (file, source),
        ] {
            std::fs::write(fixture.directory.join(name), contents).unwrap();
        }
        std::fs::write(
            fixture.directory.join("tsconfig.json"),
            variant["configuration"].to_string(),
        )
        .unwrap();
        fixture.project = Project::load(&fixture.directory.join("tsconfig.json")).unwrap();
        let request = TypeRequest::new(TypeQuery::At {
            file: fixture.directory.join(file),
            position: SourcePosition {
                line: row["at"]["line"].as_u64().unwrap() as usize,
                column: row["at"]["column"].as_u64().unwrap() as usize,
            },
        });
        let query = |limits| {
            worker_with_limits(&fixture.project, limits, |parser, control| {
                let answer = type_of_here(&fixture.project, &request, parser, control.clone())?;
                Ok((answer, control.usage()))
            })
        };
        let verify_answer = |answer: TypeAnswer| {
            let TypeAnswer::Selected(answer) = answer else {
                panic!("unselected {label}");
            };
            assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
            assert_eq!(
                answer.description.summary,
                row["answer"]["summary"].as_str().unwrap(),
                "{label}"
            );
            assert!(answer.issues.is_empty(), "{label}: {answer:?}");
        };
        let (answer, usage) = query(Limits::default()).unwrap();
        verify_answer(answer);
        assert_eq!(usage.depth, 0);

        let text = crate::source::SourceText::new(source.to_owned());
        let successor = TypeRequest::new(TypeQuery::At {
            file: fixture.directory.join(file),
            position: text.position(source.find("healthy").unwrap()),
        });
        healthy(fixture.query_request(&successor, Limits::default()));
    }
}

#[test]
fn request_resource_late_symbol_fallback_native_values() {
    cases();
}
