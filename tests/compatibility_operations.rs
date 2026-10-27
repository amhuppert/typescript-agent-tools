//! The navigation and construction operations share the configured program,
//! including the 5.6 resolver, JavaScript admission and package redirects.
//! Definition locations and contextual properties were checked with 5.6.3's
//! language service/checker on scratch projects (incremental disabled).
//! Statement selection and test outlines are syntax-only product contracts.
#![cfg(unix)]
mod support;

use serde_json::{Value, json};
use std::process::Output;
use support::service::Service;
use support::{BASE_OPTIONS, DIALECTS, Dialect, Fixture};

const MODEL: &str =
    "export interface Shape { id: number; label?: string }\nexport const value = 1;\n";
const TEST: &str = "import { Shape, value } from '@model';\nimport { extra } from 'helpers';\nexport const item: Shape = { id: value };\ndescribe('shape', () => {\n  it('builds', () => { extra(); });\n});\n";
const GLOBALS: &str = concat!(
    include_str!("../fixtures/types/constructs/globals.d.ts"),
    "declare function describe(name: string, body: () => void): void;\ndeclare function it(name: string, body: () => void): void;\n"
);

fn document(output: Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn definition(service: &Service, at: &str, file: &str, line: usize, column: usize) {
    let answer = document(service.both(&["--definition", "--at", at, "--json"]), 0);
    assert_eq!(answer["outcome"], "complete", "{answer}");
    assert_eq!(answer["groups"].as_array().unwrap().len(), 1, "{answer}");
    let declarations = answer["groups"][0]["declarations"].as_array().unwrap();
    assert_eq!(declarations.len(), 1, "{answer}");
    assert_eq!(declarations[0]["location"]["file"], file);
    assert_eq!(
        declarations[0]["location"]["start"],
        json!({"line":line,"column":column})
    );
}

#[test]
fn operations_follow_paths_and_base_url_under_5_6() {
    let config = format!(
        r#"{{"compilerOptions":{{{BASE_OPTIONS},"baseUrl":".","paths":{{"@model":["src/model"]}}}},"include":["src","helpers.ts"]}}"#
    );
    let service = Service::of(Fixture::with_dialect(
        Dialect::TypeScript56,
        &[
            ("tsconfig.json", &config),
            ("src/model.ts", MODEL),
            ("src/shape.test.ts", TEST),
            ("src/globals.d.ts", GLOBALS),
            ("helpers.ts", "export function extra() {}\n"),
            (
                "node_modules/helpers/index.d.ts",
                "export declare const extra: string;\n",
            ),
            ("statement.ts", "it('second', () => {});\n"),
        ],
    ));
    definition(&service, "src/shape.test.ts:1:17", "src/model.ts", 2, 14);
    definition(&service, "src/shape.test.ts:2:10", "helpers.ts", 1, 17);
    let sites = document(
        service.both(&["--constructs", "--name", "Shape", "--json"]),
        0,
    );
    assert_eq!(sites["outcome"], "complete", "{sites}");
    assert_eq!(sites["sites"].as_array().unwrap().len(), 1, "{sites}");
    assert_eq!(sites["sites"][0]["location"]["file"], "src/shape.test.ts");
    assert_eq!(
        sites["sites"][0]["constructs"][0]["lacks"],
        json!([
            {"key":{"kind":"string","name":"label"},"optional":true,"presence":"absent"}
        ])
    );
    // The hint is an executable definition command, not a module-name edit.
    let hint = document(
        service.both(&["--show", "--name", "value", "src/shape.test.ts", "--json"]),
        1,
    );
    let argv: Vec<&str> = hint["specifiers"][0]["next"]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert_eq!(
        &argv[..4],
        &["tsparse", "--definition", "--project", "tsconfig.json"]
    );
    let followed = document(
        service
            .command()
            .args(&argv[1..])
            .arg("--json")
            .output()
            .unwrap(),
        0,
    );
    assert_eq!(
        followed["groups"][0]["declarations"][0]["location"]["file"],
        "src/model.ts"
    );
    let outline = document(
        service
            .command()
            .args(["--test-outline", "src/shape.test.ts", "--json"])
            .output()
            .unwrap(),
        0,
    );
    assert_eq!(outline["entries"].as_array().unwrap().len(), 1, "{outline}");
    assert_eq!(outline["entries"][0]["call"], "describe");
    assert_eq!(outline["entries"][0]["children"][0]["call"], "it");
    let shown = document(
        service.both(&["--show", "--at", "src/shape.test.ts:5:3", "--json"]),
        0,
    );
    assert_eq!(shown["outcome"], "found", "{shown}");
    assert!(
        shown
            .to_string()
            .contains("it('builds', () => { extra(); });"),
        "{shown}"
    );
    let preview = service.both(&[
        "--insert-after",
        "--at",
        "src/shape.test.ts:5:3",
        "--from",
        "statement.ts",
    ]);
    assert_eq!(preview.status.code(), Some(0), "{preview:?}");
    assert_eq!(
        std::fs::read_to_string(service.fixture.path("src/shape.test.ts")).unwrap(),
        TEST
    );
}

#[test]
fn javascript_queries_answer_and_statement_edits_refuse_every_extension() {
    for dialect in DIALECTS {
        for extension in ["js", "jsx", "mjs", "cjs"] {
            let file = format!("src/script.{extension}");
            let js = "/** @type {import('./model').Shape} */\nexport const item = { id: 1 };\ndescribe('shape', () => {\n  it('builds', () => {});\n});\n";
            let app = format!(
                "import {{ item }} from './script.{extension}';\nexport const copy = item;\n"
            );
            let config = format!(
                r#"{{"compilerOptions":{{{BASE_OPTIONS},"allowJs":true,"noEmit":true}},"include":["src"]}}"#
            );
            let service = Service::of(Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    ("src/model.ts", MODEL),
                    ("src/app.ts", &app),
                    (&file, js),
                    ("src/globals.d.ts", GLOBALS),
                    ("statement.ts", "it('second', () => {});\n"),
                ],
            ));
            definition(&service, "src/app.ts:1:10", &file, 2, 14);
            let sites = document(
                service.both(&["--constructs", "--name", "Shape", "--json"]),
                0,
            );
            assert_eq!(
                sites["sites"].as_array().unwrap().len(),
                1,
                "{dialect:?} {file}: {sites}"
            );
            assert_eq!(sites["sites"][0]["location"]["file"], file);
            assert_eq!(
                sites["sites"][0]["constructs"][0]["lacks"][0]["key"]["name"],
                "label"
            );
            let outline = document(
                service
                    .command()
                    .args(["--test-outline", &file, "--json"])
                    .output()
                    .unwrap(),
                0,
            );
            assert_eq!(outline["entries"].as_array().unwrap().len(), 1, "{outline}");
            assert_eq!(outline["entries"][0]["call"], "describe");
            assert_eq!(outline["entries"][0]["children"][0]["call"], "it");
            let at = format!("{file}:4:3");
            let shown = document(service.both(&["--show", "--at", &at, "--json"]), 0);
            assert_eq!(shown["outcome"], "found");
            for project in [false, true] {
                let mut command = service.command();
                command.args([
                    "--insert-after",
                    "--at",
                    &at,
                    "--from",
                    "statement.ts",
                    "--json",
                ]);
                if project {
                    command.args(["--project", "tsconfig.json", "--direct"]);
                }
                let refused = document(command.output().unwrap(), 1);
                assert_eq!(refused["error"]["kind"], "Unsupported", "{refused}");
                assert!(
                    refused["error"]["message"]
                        .as_str()
                        .unwrap()
                        .contains("JavaScript files cannot be edited"),
                    "{refused}"
                );
            }
            assert_eq!(
                std::fs::read_to_string(service.fixture.path(&file)).unwrap(),
                js
            );
            // Capability refusals fail every project operation, but the
            // syntax-only file reader still lists and shows the test.
            std::fs::write(
                service.fixture.path(&file),
                format!(
                    "{js}\n/** @param {{number}} n */\nexport function refused(n) {{ return n; }}\n"
                ),
            )
            .unwrap();
            for args in [
                vec!["--definition", "--at", "src/app.ts:1:10"],
                vec!["--constructs", "--name", "Shape"],
                vec!["--show", "--at", &at],
            ] {
                let refused = document(service.both(&[args.as_slice(), &["--json"]].concat()), 1);
                assert_eq!(refused["error"]["kind"], "Unsupported", "{refused}");
                assert!(
                    refused["error"]["message"]
                        .as_str()
                        .unwrap()
                        .contains("unsupported JavaScript"),
                    "{refused}"
                );
            }
            let outline = document(
                service
                    .command()
                    .args(["--test-outline", &file, "--json"])
                    .output()
                    .unwrap(),
                0,
            );
            assert_eq!(outline["entries"][0]["children"][0]["name"], "'builds'");
        }
    }
}

#[test]
fn project_operations_share_the_ts1540_policy_but_file_navigation_is_syntax_only() {
    let source = "module Legacy { export interface Shape { id?: number } }\nconst item: Legacy.Shape = {};\ndescribe('legacy', () => {});\n";
    for dialect in DIALECTS {
        let service = Service::of(Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", &support::BASE_CONFIG),
                ("app.ts", source),
                ("globals.d.ts", GLOBALS),
            ],
        ));
        for args in [
            vec!["--definition", "--at", "app.ts:2:20"],
            vec!["--constructs", "--name", "Legacy.Shape"],
            vec!["--show", "--at", "app.ts:3:1"],
        ] {
            let code = if dialect == Dialect::TypeScript56 {
                0
            } else {
                1
            };
            let result = document(service.both(&[args.as_slice(), &["--json"]].concat()), code);
            if dialect == Dialect::TypeScript7 {
                assert!(
                    result["error"]["message"]
                        .as_str()
                        .unwrap()
                        .contains("namespace"),
                    "{result}"
                );
            } else {
                // Locations and the missing optional property come from 5.6.3.
                match args[0] {
                    "--definition" => {
                        assert_eq!(result["outcome"], "complete", "{result}");
                        assert_eq!(
                            result["groups"],
                            json!([{"declarations": [{
                                "name": "Shape",
                                "location": {
                                    "file": "app.ts",
                                    "start": {"line": 1, "column": 34},
                                    "end": {"line": 1, "column": 39}
                                }
                            }]}])
                        );
                    }
                    "--constructs" => {
                        assert_eq!(result["outcome"], "complete", "{result}");
                        assert_eq!(result["sites"].as_array().unwrap().len(), 1, "{result}");
                        assert_eq!(
                            result["sites"][0]["location"],
                            json!({
                                "file": "app.ts",
                                "start": {"line": 2, "column": 28},
                                "end": {"line": 2, "column": 30}
                            })
                        );
                        assert_eq!(
                            result["sites"][0]["constructs"][0]["lacks"],
                            json!([{
                                "key": {"kind": "string", "name": "id"},
                                "optional": true,
                                "presence": "absent"
                            }])
                        );
                    }
                    "--show" => {
                        assert_eq!(result["outcome"], "found", "{result}");
                        assert_eq!(result["declaration"]["kind"], "statement");
                        assert_eq!(
                            result["declaration"]["range"],
                            json!({
                                "start": {"line": 3, "column": 1},
                                "end": {"line": 3, "column": 30}
                            })
                        );
                        assert_eq!(result["source"], "describe('legacy', () => {});");
                    }
                    _ => unreachable!(),
                }
            }
        }
        let outline = document(
            service
                .command()
                .args(["--test-outline", "app.ts", "--json"])
                .output()
                .unwrap(),
            0,
        );
        assert_eq!(outline["entries"].as_array().unwrap().len(), 1);
        let shown = document(
            service
                .command()
                .args(["--show", "--at", "app.ts:3:1", "--json"])
                .output()
                .unwrap(),
            0,
        );
        assert_eq!(shown["outcome"], "found");
    }
}

#[test]
fn new_project_selections_refuse_a_redirect_path() {
    let config =
        format!(r#"{{"compilerOptions":{{{BASE_OPTIONS}}},"files":["app.ts","globals.d.ts"]}}"#);
    for dialect in DIALECTS {
        let service = Service::of(Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", &config),
                ("globals.d.ts", GLOBALS),
                (
                    "app.ts",
                    "import { Shape } from 'dup';\nimport { Other } from 'holder';\nexport const item: Shape = {};\nexport type Selected = Shape;\n",
                ),
                (
                    "node_modules/dup/package.json",
                    r#"{"name":"dup","version":"1.0.0","types":"index.d.ts"}"#,
                ),
                (
                    "node_modules/dup/index.d.ts",
                    "export interface Shape { id?: number }\n",
                ),
                (
                    "node_modules/holder/package.json",
                    r#"{"name":"holder","version":"1.0.0","types":"index.d.ts"}"#,
                ),
                (
                    "node_modules/holder/index.d.ts",
                    "export { Shape as Other } from 'dup';\n",
                ),
                (
                    "node_modules/holder/node_modules/dup/package.json",
                    r#"{"name":"dup","version":"1.0.0","types":"index.d.ts"}"#,
                ),
                (
                    "node_modules/holder/node_modules/dup/index.d.ts",
                    "export interface Shape { id?: number }\n",
                ),
            ],
        ));
        definition(
            &service,
            "app.ts:2:10",
            "node_modules/dup/index.d.ts",
            1,
            18,
        );
        let file = "node_modules/holder/node_modules/dup/index.d.ts";
        let at = format!("{file}:1:18");
        for args in [
            vec!["--definition", "--at", &at],
            vec!["--definition", "--name", "Shape", file],
            vec!["--constructs", "--at", &at],
            vec!["--constructs", "--name", "Shape", file],
            vec!["--constructs", "--name", "Selected", "--file", file],
            vec!["--show", "--at", &at],
        ] {
            let result = document(service.both(&[args.as_slice(), &["--json"]].concat()), 1);
            assert_eq!(result["error"]["kind"], "Unsupported", "{result}");
            assert!(
                result["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("'node_modules/dup/index.d.ts' instead"),
                "{result}"
            );
        }
    }
}
