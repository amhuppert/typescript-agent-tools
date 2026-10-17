//! Final accessor values and contextual signatures from independent native sessions.
use super::*;

#[test]
fn request_resource_accessor_declarations_native_values() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-accessor-declaration-resources/expected.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 32);
    for row in rows {
        let variant = &row["variant"];
        let width = variant["width"].as_u64().unwrap() as usize;
        let declarations = row["declarations"].as_array().unwrap();
        assert_eq!(declarations.len(), width);
        for (index, property) in declarations.iter().enumerate() {
            assert_eq!(property["name"], format!("p{index}"));
            let origins = property["origins"].as_array().unwrap();
            assert_eq!(origins.len(), 2);
            assert_eq!(origins[0]["kind"], "GetAccessor");
            assert_eq!(origins[1]["kind"], "SetAccessor");
        }
        verify(&row, &variant["configuration"]);
    }
}

#[test]
fn request_resource_accessor_declarations_native_signatures() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-accessor-declaration-resources/expected-signatures.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 12);
    for row in rows {
        verify(&row, &row["configuration"]);
    }
}

fn verify(row: &serde_json::Value, configuration: &serde_json::Value) {
    let label = format!(
        "{}/{}/{}/{}",
        row["variant"]["config"], row["profile"], row["mode"], row["key"]
    );
    let file = row["at"]["file"].as_str().unwrap();
    let source = row["source"].as_str().unwrap();
    let mut fixture = Fixture::new("let chosen: number = 1;");
    std::fs::write(
        fixture.directory.join("globals.d.ts"),
        include_str!(
            "../../../../fixtures/types/native-accessor-declaration-resources/globals.d.ts"
        ),
    )
    .unwrap();
    std::fs::write(fixture.directory.join(file), source).unwrap();
    std::fs::write(
        fixture.directory.join("tsconfig.json"),
        configuration.to_string(),
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
    let healthy_request = TypeRequest::new(TypeQuery::At {
        file: fixture.directory.join(file),
        position: text.position(source.find("healthy").unwrap()),
    });
    healthy(fixture.query_request(&healthy_request, Limits::default()));
}
