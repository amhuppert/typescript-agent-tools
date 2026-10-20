mod support;

use serde_json::{Value, json};
use support::{QueryFailure, project};
use typescript_parser::analysis::QueryErrorKind;
use typescript_parser::navigation::{Kind, ShowOutcome, ShowQuery};
use typescript_parser::references::{ReferenceFilters, ReferenceQuery, ReferenceRequest};
use typescript_parser::source::SourcePosition;

fn named(loaded: &typescript_parser::project::Project, name: &str) -> Value {
    serde_json::to_value(
        loaded
            .references(ReferenceRequest {
                query: ReferenceQuery::Name {
                    name: name.into(),
                    file: None,
                },
                filters: ReferenceFilters::default(),
            })
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn exact_empty_member_names_have_advisories_with_unspellable_selectors() {
    let fixture = project(&[(
        "a.ts",
        "export const object = { '': 1 }; export class Host { '': number = 1; }",
    )]);
    let loaded = fixture.load();
    let result: Value = serde_json::to_value(
        loaded
            .references(ReferenceRequest {
                query: ReferenceQuery::Name {
                    name: String::new(),
                    file: None,
                },
                filters: ReferenceFilters::default(),
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["groups"], json!([]));
    assert_eq!(result["unsearchedMembers"]["count"], 2);
    let suggestions = result["unsearchedMembers"]["selectors"].as_array().unwrap();
    assert!(suggestions.iter().all(|entry| entry["selector"].is_null()));
    for entry in suggestions {
        let selected = loaded
            .show(ShowQuery::At {
                file: fixture.path("a.ts"),
                position: SourcePosition {
                    line: entry["anchor"]["line"].as_u64().unwrap() as usize,
                    column: entry["anchor"]["column"].as_u64().unwrap() as usize,
                },
            })
            .unwrap();
        let ShowOutcome::Found(found) = selected.outcome else {
            panic!("empty-key advisory anchor did not replay");
        };
        assert_eq!(found.declaration.key.as_deref(), Some(""));
        assert_eq!(found.declaration.selector, None);
    }
}

#[test]
fn decoded_keys_match_exactly_and_distinct_display_selectors_preserve_replay_roles() {
    let fixture = project(&[(
        "a.ts",
        concat!(
            "export const numerics = { 0x10: 1, 1.6e1: 2, '16': 3 };\n",
            "export class Host { '\\u0072un'() {} static run() {} Run() {} }\n",
            "export const nested = { 'a.b': { run() {} }, '': { run() {} } };\n",
            "export class Parameter { constructor(public run: string) { run; } }\n",
        ),
    )]);
    let loaded = fixture.load();
    let numeric = named(&loaded, "16");
    assert_eq!(numeric["unsearchedMembers"]["count"], 1);
    assert_eq!(
        numeric["unsearchedMembers"]["selectors"][0]["selector"],
        "numerics.16"
    );
    assert_eq!(named(&loaded, "0x10")["unsearchedMembers"]["count"], 0);
    let result = named(&loaded, "run");
    assert_eq!(result["groups"].as_array().unwrap().len(), 1);
    assert_eq!(result["unsearchedMembers"]["count"], 4);
    let suggestions = result["unsearchedMembers"]["selectors"].as_array().unwrap();
    let selectors: Vec<_> = suggestions
        .iter()
        .map(|entry| entry["selector"].as_str())
        .collect();
    assert_eq!(
        selectors,
        [None, None, Some("Host.run"), Some("Parameter.run")]
    );
    assert_eq!(named(&loaded, "Run")["unsearchedMembers"]["count"], 1);
    assert_eq!(named(&loaded, "RUN")["unsearchedMembers"]["count"], 0);
    for entry in suggestions {
        let anchor = &entry["anchor"];
        let position = SourcePosition {
            line: anchor["line"].as_u64().unwrap() as usize,
            column: anchor["column"].as_u64().unwrap() as usize,
        };
        let selected = loaded
            .show(ShowQuery::At {
                file: fixture.path("a.ts"),
                position,
            })
            .unwrap();
        let ShowOutcome::Found(found) = selected.outcome else {
            panic!("advisory anchor did not select its Q2 declaration");
        };
        assert_eq!(
            found.declaration.selector.as_deref(),
            entry["selector"].as_str()
        );
        if entry["selector"] == "Host.run" {
            // Display dedup preserves the earliest instance declaration,
            // although the later static member is a distinct semantic symbol.
            assert!(found.source.starts_with("'\\u0072un'"));
        }
        if entry["selector"] == "Parameter.run" {
            assert_eq!(found.declaration.kind, Kind::Property);
            let lexical = serde_json::to_value(
                loaded
                    .find_references(ReferenceQuery::At {
                        file: fixture.path("a.ts"),
                        position,
                    })
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(lexical["groups"], result["groups"]);
            assert_eq!(
                lexical["unsearchedMembers"],
                json!({"count": 0, "selectors": []})
            );
        }
    }
}

#[test]
fn private_astral_and_replacement_keys_do_not_merge_with_unkeyed_names() {
    let fixture = project(&[(
        "a.ts",
        concat!(
            "export const keys = { '😀': 1, '�': 2, '\\ud800': 3, ['computed']: 4 };\n",
            "export class Host { #run() {} }\n",
        ),
    )]);
    let loaded = fixture.load();
    for (name, selector) in [("😀", "keys.😀"), ("�", "keys.�"), ("#run", "Host.#run")] {
        let result = named(&loaded, name);
        assert_eq!(result["unsearchedMembers"]["count"], 1);
        assert_eq!(
            result["unsearchedMembers"]["selectors"][0]["selector"],
            selector
        );
    }
    // Q2 assigns no lookup key to a lone-surrogate or computed name. Their
    // presentation labels must not become identities for this advisory.
    for name in ["\\ud800", "computed"] {
        assert_eq!(named(&loaded, name)["unsearchedMembers"]["count"], 0);
    }
}

#[test]
fn advisory_anchors_follow_utf16_columns_and_all_typescript_line_breaks() {
    for newline in ["\r", "\n", "\r\n", "\u{2028}", "\u{2029}"] {
        for space in [" ", "\t", "\u{00a0}"] {
            for bom in ["", "\u{feff}"] {
                let before = format!("export class Host {{{space}/* 😀 */{space}");
                let text = format!("{bom}#! metadata{newline}{before}run() {{}} }}");
                let fixture = project(&[("a.ts", &text)]);
                let loaded = fixture.load();
                let result = named(&loaded, "run");
                assert_eq!(result["unsearchedMembers"]["count"], 1);
                let entry = &result["unsearchedMembers"]["selectors"][0];
                assert_eq!(entry["selector"], "Host.run");
                // All separators start the second line. The astral comment adds
                // two UTF-16 units, while each space-class variant adds one.
                let position = SourcePosition {
                    line: 2,
                    column: before.encode_utf16().count() + 1,
                };
                assert_eq!(entry["anchor"]["line"], position.line);
                assert_eq!(entry["anchor"]["column"], position.column);
                let selected = loaded
                    .show(ShowQuery::At {
                        file: fixture.path("a.ts"),
                        position,
                    })
                    .unwrap();
                let ShowOutcome::Found(found) = selected.outcome else {
                    panic!("advisory anchor did not replay");
                };
                assert_eq!(found.declaration.selector.as_deref(), Some("Host.run"));
            }
        }
    }
}

#[test]
fn lexical_target_failures_and_trivia_positions_do_not_become_member_metadata_successes() {
    let text = "export class Host { /* run */ run() {} }";
    let fixture = project(&[("a.ts", text)]);
    let loaded = fixture.load();
    let at = |column| ReferenceRequest {
        query: ReferenceQuery::At {
            file: fixture.path("a.ts"),
            position: SourcePosition { line: 1, column },
        },
        filters: ReferenceFilters {
            calls: true,
            exclude_tests: true,
            file: None,
        },
    };
    let member = loaded
        .find_references(at(text.rfind("run").unwrap() + 1).query)
        .query_error();
    assert_eq!(member.kind, QueryErrorKind::UnsupportedTarget);
    let member = serde_json::to_value(
        loaded
            .references(at(text.rfind("run").unwrap() + 1))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(member["coverage"]["baseline"], "member");
    assert_eq!(member["outcome"], "complete");
    assert_eq!(member["groups"].as_array().unwrap().len(), 1);
    assert_eq!(member["groups"][0]["occurrences"], json!([]));
    for column in [text.find("run").unwrap() + 1, text.find("/*").unwrap()] {
        let result = serde_json::to_value(loaded.references(at(column)).unwrap()).unwrap();
        assert_eq!(result["groups"], json!([]));
        assert_eq!(
            result["unsearchedMembers"],
            json!({"count":0,"selectors":[]})
        );
        assert_eq!(
            result["coverage"],
            json!({"baseline":"lexical","status":"complete","gaps":[]})
        );
    }
    let outside = loaded
        .references(ReferenceRequest {
            query: ReferenceQuery::Name {
                name: "run".into(),
                file: Some(fixture.path("outside.ts")),
            },
            filters: ReferenceFilters {
                calls: true,
                exclude_tests: true,
                file: None,
            },
        })
        .query_error();
    assert_eq!(outside.kind, QueryErrorKind::OutsideProject);
}

#[cfg(unix)]
#[test]
fn service_metadata_matches_direct_bytes_and_refreshes_without_stale_advisories() {
    let service = support::service::Service::new(&[
        (
            "seed.test.ts",
            "export function run() {} export class Hidden { run() {} }",
        ),
        ("src/use.ts", "import { run } from '../seed.test'; run();"),
        (
            "src/outside.ts",
            "import { run } from '../seed.test'; run();",
        ),
        ("z.ts", "export class Host { run() {} }"),
        ("keys.ts", "export const object = { '': 1, 0x10: 2 };"),
    ]);
    let arguments = [
        "--references",
        "--name",
        "run",
        "--calls",
        "--no-tests",
        "--json",
    ];
    let compare = |extra: &[&str]| {
        let mut remote_args = arguments.to_vec();
        remote_args.extend_from_slice(extra);
        let mut direct_args = remote_args.clone();
        direct_args.push("--direct");
        let direct = service.run(&direct_args);
        let remote = service.run(&remote_args);
        assert_eq!(remote.status.code(), direct.status.code());
        assert_eq!(remote.stdout, direct.stdout);
        assert_eq!(remote.stderr, direct.stderr);
        assert!(remote.stderr.is_empty());
        let document = serde_json::from_slice::<Value>(&remote.stdout).unwrap();
        if document["outcome"] == "error" {
            assert_eq!(remote.status.code(), Some(1));
        } else {
            assert_eq!(remote.status.code(), Some(0));
        }
        document
    };
    let initial = compare(&[]);
    assert_eq!(initial["unsearchedMembers"]["count"], 2);
    assert_eq!(
        initial["coverage"],
        json!({"baseline":"lexical","status":"complete","gaps":[]})
    );
    for (name, selector) in [("", None), ("16", Some("object.16"))] {
        let arguments = ["--references", "--name", name, "--json"];
        let direct = service.run(&[arguments.as_slice(), &["--direct"]].concat());
        let remote = service.run(&arguments);
        assert_eq!(remote.status.code(), Some(0));
        assert_eq!(remote.status.code(), direct.status.code());
        assert_eq!(remote.stdout, direct.stdout);
        assert_eq!(remote.stderr, direct.stderr);
        let document = support::service::json(remote);
        assert_eq!(document["groups"], json!([]));
        assert_eq!(document["unsearchedMembers"]["count"], 1);
        assert_eq!(
            document["unsearchedMembers"]["selectors"][0]["selector"].as_str(),
            selector
        );
    }
    let scoped = compare(&["seed.test.ts"]);
    assert_eq!(scoped["unsearchedMembers"]["count"], 1);
    assert_eq!(
        scoped["unsearchedMembers"]["selectors"][0]["selector"],
        "Hidden.run"
    );
    assert_eq!(
        scoped["groups"][0]["occurrences"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        scoped["groups"][0]["declarations"][0]["location"]["file"],
        "seed.test.ts"
    );
    let occurrence_files: Vec<_> = scoped["groups"][0]["occurrences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["location"]["file"].as_str().unwrap())
        .collect();
    assert_eq!(occurrence_files, ["src/outside.ts", "src/use.ts"]);
    let import_scoped = compare(&["src/use.ts"]);
    assert_eq!(import_scoped["unsearchedMembers"]["count"], 0);
    assert_eq!(
        import_scoped["groups"][0]["occurrences"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(service.generation(), 1);

    let source = service.fixture.path("z.ts");
    let modified = std::fs::metadata(&source).unwrap().modified().unwrap();
    std::fs::write(&source, "export class Host { sun() {} }").unwrap();
    std::fs::File::open(&source)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    assert_eq!(compare(&[])["unsearchedMembers"]["count"], 1);
    assert_eq!(service.generation(), 2);

    let config = service.fixture.path("tsconfig.json");
    let original = std::fs::read(&config).unwrap();
    std::fs::write(&config, r#"{"compilerOptions":{"strict":"invalid"}}"#).unwrap();
    let failed = compare(&[]);
    assert_eq!(failed["outcome"], "error");
    assert_eq!(failed["error"]["kind"], "Config");
    assert!(failed.get("groups").is_none());
    assert!(failed.get("unsearchedMembers").is_none());
    std::fs::write(&config, original).unwrap();
    assert_eq!(compare(&[])["unsearchedMembers"]["count"], 1);
    assert_eq!(service.generation(), 3);

    std::fs::write(&source, "export class").unwrap();
    let failed = compare(&[]);
    assert_eq!(failed["outcome"], "error");
    assert_eq!(failed["error"]["kind"], "Syntax");
    assert!(failed.get("unsearchedMembers").is_none());
    std::fs::write(&source, "export class Host { run() {} }").unwrap();
    assert_eq!(compare(&[])["unsearchedMembers"]["count"], 2);
    assert_eq!(service.generation(), 4);
}
