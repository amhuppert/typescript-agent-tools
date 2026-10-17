//! Native typeReferenceToTypeNode asks iterator globals after declaration
//! acquisition, even when the displayed generic has another name.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::Project;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn iterator_display_keeps_global_arity_rules_and_required_input_failures() {
    let sources = [
        (
            "tsconfig",
            "cases",
            include_str!("../fixtures/types/native-global-display/cases.ts"),
            include_str!("../fixtures/types/native-global-display/tsconfig.json"),
        ),
        (
            "one",
            "one",
            include_str!("../fixtures/types/native-global-display/one.ts"),
            include_str!("../fixtures/types/native-global-display/one.json"),
        ),
        (
            "three",
            "three",
            include_str!("../fixtures/types/native-global-display/three.ts"),
            include_str!("../fixtures/types/native-global-display/three.json"),
        ),
        (
            "this",
            "this",
            include_str!("../fixtures/types/native-global-display/this.ts"),
            include_str!("../fixtures/types/native-global-display/this.json"),
        ),
        (
            "plain",
            "plain",
            include_str!("../fixtures/types/native-global-display/plain.ts"),
            include_str!("../fixtures/types/native-global-display/plain.json"),
        ),
    ];
    let mut files = vec![(
        "globals.d.ts".to_owned(),
        include_str!("../fixtures/types/native-global-display/globals.d.ts"),
    )];
    for &(config, file, source, json) in &sources {
        files.push((format!("{file}.ts"), source));
        files.push((format!("{config}.json"), json));
    }
    let fixture = Fixture::new(
        &files
            .iter()
            .map(|(path, text)| (path.as_str(), *text))
            .collect::<Vec<_>>(),
    );
    for &(config, file, source, _) in &sources {
        let loaded =
            Project::load(&fixture.path(&format!("{config}.json"))).map_err(support::input_error);
        {
            // The compiler's isolated demand demonstrates the lookup schedule,
            // but this missing required module fails this tool's established
            // Project loading contract before a semantic query can run.
            let Err(error) = loaded else {
                panic!("missing required module loaded");
            };
            assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
            assert_eq!(error.path, Some(fixture.path(&format!("{file}.ts"))));
            assert_eq!(
                error.position,
                Some(position(source, source.find("\"./missing\"").unwrap()))
            );
        }
    }
    // The admitted twin supplies the actual dependency. Its unchanged arity
    // branches and final summaries are checked by the pinned native harness.
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-global-display/admitted"),
    );
    for &(config, file, source, _) in &sources {
        let project = fixture.load_config(&format!("{config}.json"));
        for (name, expected) in [
            ("ordinary", "Box<number>"),
            ("array", "number[]"),
            ("nongeneric", "{ value: number; }"),
        ] {
            for expand in [false, true] {
                let marker = format!("/*#{name}*/");
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path(&format!("{file}.ts")),
                        position: position(source, source.find(&marker).unwrap() + marker.len()),
                    },
                    expand,
                    depth: 1,
                });
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}/{expand}: {answer:?}")
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{name}/{expand}: {:?}",
                    result.issues
                );
                assert_eq!(
                    result.description.summary, expected,
                    "{config}/{name}/{expand}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{name}/{expand}: {:?}",
                    result.issues
                );
            }
        }
    }
}
