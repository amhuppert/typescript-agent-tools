mod support;

use serde_json::{Value, json};
use support::project;
use typescript_parser::navigation::{ShowOutcome, ShowQuery};
use typescript_parser::references::{ReferenceFilters, ReferenceQuery, ReferenceRequest};
use typescript_parser::source::SourcePosition;

#[test]
fn lexical_results_name_their_coverage_even_when_no_occurrences_survive() {
    let fixture = project(&[("a.test.ts", "export function run() {} run();")]);
    let project = fixture.load();
    for filters in [
        ReferenceFilters::default(),
        ReferenceFilters {
            calls: true,
            exclude_tests: true,
        },
    ] {
        let result = project
            .references(ReferenceRequest {
                query: ReferenceQuery::Name {
                    name: "run".into(),
                    file: None,
                },
                filters,
            })
            .unwrap();
        let document = serde_json::to_value(result).unwrap();
        assert_eq!(document["operation"], "references");
        assert_eq!(document["outcome"], "complete");
        assert_eq!(
            document["coverage"],
            json!({
                "baseline": "lexical", "status": "complete", "gaps": []
            })
        );
        assert_eq!(
            document["unsearchedMembers"],
            json!({"count": 0, "selectors": []})
        );
        assert_eq!(
            document["groups"][0]["declarations"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        if filters.exclude_tests {
            assert_eq!(document["groups"][0]["occurrences"], json!([]));
        }
    }
    let missing = serde_json::to_value(
        project
            .find_references(ReferenceQuery::Name {
                name: "absent".into(),
                file: None,
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(missing["outcome"], "complete");
    assert_eq!(missing["groups"], json!([]));
}

#[test]
fn member_advisory_uses_exact_catalog_selectors_and_deduplicates_display_entries() {
    let fixture = project(&[
        ("z.test.ts", "export class Zebra { run() {} }"),
        (
            "a.ts",
            concat!(
                "export function run() {}\n",
                "export class Host {\n",
                "  run(value: string): string;\n",
                "  run(value: number): number;\n",
                "  run(value: any) { return value; }\n",
                "}\n",
                "export interface Port { run(): void }\n",
                "export interface Port { run(value: string): void }\n",
                "export class Pair { get run() { return 1; } set run(value: number) {} }\n",
                // Native binding rejects a method/property collision; two
                // properties retain the advisory's duplicate-name case.
                "export const object = { run: 1, 'other-name': 1, ['run']: 2 };\n",
                "export enum Status { run }\n",
                "export class Parameter { constructor(public run: string) {} }\n",
                "export class Escaped { \\u0072un() {} }\n",
                "export class Different { Run() {} }\n",
                "export const nested = { 'a.b': { run() {} }, '': { run() {} } };\n",
            ),
        ),
    ]);
    let project = fixture.load();
    // A Project must answer from its own capture; the advisory never reparses disk.
    std::fs::write(
        fixture.path("a.ts"),
        "export class Replacement { run() {} }",
    )
    .unwrap();
    let query = |file| {
        project
            .references(ReferenceRequest {
                query: ReferenceQuery::Name {
                    name: "run".into(),
                    file,
                },
                filters: ReferenceFilters {
                    calls: true,
                    exclude_tests: true,
                },
            })
            .unwrap()
    };
    let document = serde_json::to_value(query(None)).unwrap();
    // The ordinary parameter remains a separate lexical group.
    assert_eq!(document["groups"].as_array().unwrap().len(), 2);
    assert_eq!(document["unsearchedMembers"]["count"], 10);
    let suggestions = document["unsearchedMembers"]["selectors"]
        .as_array()
        .unwrap();
    let entries: Vec<_> = suggestions
        .iter()
        .map(|entry| (entry["file"].as_str().unwrap(), entry["selector"].as_str()))
        .collect();
    assert_eq!(
        entries,
        [
            ("a.ts", None),
            ("a.ts", None),
            ("a.ts", Some("Escaped.run")),
            ("a.ts", Some("Host.run")),
            ("a.ts", Some("Pair.run")),
            ("a.ts", Some("Parameter.run")),
            ("a.ts", Some("Port.run")),
            ("a.ts", Some("Status.run")),
            ("a.ts", Some("object.run")),
            ("z.test.ts", Some("Zebra.run")),
        ]
    );
    assert_eq!(document["unsearchedMembers"]["count"], entries.len());
    for entry in suggestions {
        let file = entry["file"].as_str().unwrap();
        let anchor = &entry["anchor"];
        assert_eq!(anchor["file"], file);
        let selected = project
            .show(ShowQuery::At {
                file: fixture.path(file),
                position: SourcePosition {
                    line: anchor["line"].as_u64().unwrap() as usize,
                    column: anchor["column"].as_u64().unwrap() as usize,
                },
            })
            .unwrap();
        let ShowOutcome::Found(found) = selected.outcome else {
            panic!("advisory anchor selected {:?}", selected.outcome);
        };
        assert_eq!(
            found.declaration.selector.as_deref(),
            entry["selector"].as_str()
        );
    }
    let scoped = serde_json::to_value(query(Some(fixture.path("z.test.ts")))).unwrap();
    assert_eq!(scoped["groups"], json!([]));
    assert_eq!(scoped["unsearchedMembers"]["count"], 1);
    assert_eq!(
        scoped["unsearchedMembers"]["selectors"][0]["selector"],
        "Zebra.run"
    );
    let at = serde_json::to_value(
        project
            .find_references(ReferenceQuery::At {
                file: fixture.path("a.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 17,
                },
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        at["unsearchedMembers"],
        json!({"count": 0, "selectors": []})
    );
}

#[test]
fn an_unmatched_lexical_name_can_report_members_without_semantic_checking() {
    let fixture = project(&[(
        "a.ts",
        concat!(
            "export class Host { inspect() { return <div/>; } }\n",
            "export interface Port { inspect(): void }\n",
        ),
    )]);
    // JSX parses only in TSX mode; unrelated deferred bodies must stay harmless.
    std::fs::rename(fixture.path("a.ts"), fixture.path("a.tsx")).unwrap();
    let project = fixture.load();
    let result: Value = serde_json::to_value(
        project
            .find_references(ReferenceQuery::Name {
                name: "inspect".into(),
                file: None,
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["groups"], json!([]));
    assert_eq!(result["outcome"], "complete");
    assert_eq!(result["unsearchedMembers"]["count"], 2);
}
