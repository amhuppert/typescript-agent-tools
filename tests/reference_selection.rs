//! Q3's operation-specific declaration selectors, identity collapse and replay.
mod support;

use serde_json::{Value, json};
use support::{Fixture, position};
use typescript_parser::project::Project;
use typescript_parser::references::{ReferenceFilters, ReferenceQuery, ReferenceRequest};

const SOURCE: &str = "\
export class Overloaded {
  run(value: string): void;
  run(value: number): void;
  run(value: string | number): void {}
}
export class Both {
  static run(): void {}
  run(): void {}
}
export class Parameter { constructor(public run: number) {} }
export namespace Outer.Inner { export const run = 1; }
";

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true},"files":["cases.ts"]}"#,
        ),
    ])
}
fn search(project: &Project, query: ReferenceQuery) -> Value {
    serde_json::to_value(
        project
            .references(ReferenceRequest {
                query,
                filters: ReferenceFilters::default(),
            })
            .unwrap(),
    )
    .unwrap()
}
fn named(project: &Project, name: &str) -> Value {
    search(
        project,
        ReferenceQuery::Name {
            name: name.into(),
            file: None,
        },
    )
}

#[test]
fn overloads_collapse_but_static_and_instance_symbols_remain_ambiguous_and_replay() {
    let fixture = fixture();
    let project = Project::load(&fixture.path("tsconfig.json")).unwrap();
    let overload = named(&project, "Overloaded.run");
    assert_eq!(overload["outcome"], "complete", "{overload:#}");
    assert_eq!(overload["groups"].as_array().unwrap().len(), 1);
    assert_eq!(
        overload["groups"][0]["declarations"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let both = named(&project, "Both.run");
    assert_eq!(both["outcome"], "ambiguous", "{both:#}");
    let candidates = both["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 2);
    for candidate in candidates {
        assert_eq!(candidate["question"], "member");
        assert_eq!(candidate["selector"], "Both.run");
        let replay = search(
            &project,
            ReferenceQuery::At {
                file: fixture.path(candidate["anchor"]["file"].as_str().unwrap()),
                position: serde_json::from_value(candidate["anchor"].clone()).unwrap(),
            },
        );
        assert_eq!(replay["outcome"], "complete", "{replay:#}");
        assert_eq!(
            replay["groups"][0]["declarations"],
            candidate["declarations"]
        );
    }
}

#[test]
fn parameter_member_and_dotted_namespace_use_their_own_replay_roles() {
    let fixture = fixture();
    let project = Project::load(&fixture.path("tsconfig.json")).unwrap();
    let parameter = named(&project, "Parameter.run");
    assert_eq!(parameter["coverage"]["baseline"], "member", "{parameter:#}");
    let modifier = SOURCE.find("public run").unwrap();
    let replay = search(
        &project,
        ReferenceQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, modifier),
        },
    );
    assert_eq!(parameter, replay);
    let local = search(
        &project,
        ReferenceQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, modifier + "public ".len()),
        },
    );
    assert_eq!(local["coverage"]["baseline"], "lexical");
    let namespace = named(&project, "Outer.Inner");
    let inner = SOURCE.find("Outer.Inner").unwrap() + "Outer.".len();
    let replay = search(
        &project,
        ReferenceQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, inner),
        },
    );
    assert_eq!(namespace, replay);
    assert_eq!(namespace["coverage"]["baseline"], "lexical");
    assert_eq!(named(&project, "Absent.run")["candidates"], json!([]));
    assert_eq!(named(&project, "Absent.run")["outcome"], "not_found");
}

#[test]
fn filtered_search_retains_seed_origins_when_only_a_related_group_has_calls() {
    let source = "interface Base { run(): void }\ninterface Derived extends Base { run(): void }\ndeclare const derived: Derived;\nderived.run();\n";
    let fixture = Fixture::new(&[
        ("cases.ts", source),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true},"files":["cases.ts"]}"#,
        ),
    ]);
    let project = Project::load(&fixture.path("tsconfig.json")).unwrap();
    let answer = serde_json::to_value(
        project
            .references(ReferenceRequest {
                query: ReferenceQuery::Name {
                    name: "Base.run".into(),
                    file: None,
                },
                filters: ReferenceFilters {
                    calls: true,
                    exclude_tests: false,
                },
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(answer["outcome"], "complete", "{answer:#}");
    let groups = answer["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2, "{answer:#}");
    assert_eq!(groups[0]["declarations"][0]["location"]["start"]["line"], 1);
    assert_eq!(groups[0]["occurrences"], json!([]));
    assert_eq!(groups[1]["declarations"][0]["location"]["start"]["line"], 2);
    assert_eq!(groups[1]["occurrences"].as_array().unwrap().len(), 1);
    assert_eq!(groups[1]["occurrences"][0]["location"]["start"]["line"], 4);
}
