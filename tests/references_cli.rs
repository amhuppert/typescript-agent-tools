//! The `--references` command contract.
mod support;

use serde_json::{Value, json};
use std::process::{Command, Output, Stdio};
use support::{Fixture, project};

/// Cross-file aliases plus a parameter with the same spelling as an alias.
fn aliases() -> Fixture {
    project(&[
        ("src/api.ts", "export function run() {}\n"),
        (
            "src/barrel.ts",
            "export { run as execute } from \"./api\";\n",
        ),
        (
            "src/app.ts",
            "import { execute as start } from \"./barrel\";\nstart();\nfunction local(start: () => void) { start(); }\nlocal.name;\n",
        ),
        ("other/outside.ts", "export const outside = 1;\n"),
    ])
}

fn tsparse(fixture: &Fixture, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tsparse"));
    command.args(args);
    // These tests own CLI formatting/diagnostics; service lifecycle and
    // concurrent clients have dedicated teardown in reference_service.rs.
    if args.contains(&"--references") {
        command.arg("--direct");
    }
    command
        .current_dir(&fixture.root)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn location(file: &str, line: u32, start: u32, end: u32) -> Value {
    json!({
        "file": file,
        "start": { "line": line, "column": start },
        "end": { "line": line, "column": end },
    })
}

fn occurrence(name: &str, file: &str, line: u32, start: u32, roles: &[&str]) -> Value {
    json!({
        "name": name,
        "location": location(file, line, start, start + name.len() as u32),
        "roles": roles,
    })
}

#[test]
fn position_query_emits_the_aliased_symbol_group() {
    let fixture = aliases();
    let result = success(tsparse(
        &fixture,
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--at",
            "2:1",
            "src/app.ts",
            "--json",
        ],
    ));
    let project = std::fs::canonicalize(fixture.path("tsconfig.json")).unwrap();
    assert_eq!(
        result,
        json!({
            "operation": "references",
            "outcome": "complete",
            "project": project,
            "selection": { "at": { "file": "src/app.ts", "position": { "line": 2, "column": 1 } } },
            "filters": { "calls": false, "excludeTests": false },
            "coverage": { "baseline": "lexical", "status": "complete", "gaps": [] },
            "unsearchedMembers": { "count": 0, "selectors": [] },
            "groups": [{
                "declarations": [{ "name": "run", "location": location("src/api.ts", 1, 17, 20) }],
                "occurrences": [
                    occurrence("run", "src/api.ts", 1, 17, &["declaration", "export"]),
                    occurrence("execute", "src/app.ts", 1, 10, &["import"]),
                    occurrence("start", "src/app.ts", 1, 21, &["import"]),
                    occurrence("start", "src/app.ts", 2, 1, &["use"]),
                    occurrence("run", "src/barrel.ts", 1, 10, &["export"]),
                    occurrence("execute", "src/barrel.ts", 1, 17, &["export"]),
                ],
            }],
        })
    );
}

#[test]
fn name_queries_return_every_group_and_narrow_by_file() {
    let fixture = aliases();
    let groups = |args: &[&str]| -> Vec<Value> {
        let mut all = vec!["--references", "--project", "tsconfig.json", "--json"];
        all.extend_from_slice(args);
        let result = success(tsparse(&fixture, &all));
        result["groups"].as_array().unwrap().clone()
    };
    let start = groups(&["--name", "start"]);
    assert_eq!(start.len(), 2);
    assert_eq!(
        start[0]["declarations"][0]["location"]["file"],
        "src/api.ts"
    );
    assert_eq!(start[1]["declarations"][0]["name"], "start");
    assert_eq!(start[1]["occurrences"].as_array().unwrap().len(), 2);
    assert_eq!(groups(&["--name", "start", "src/app.ts"]).len(), 2);
    assert_eq!(groups(&["--name", "start", "src/api.ts"]).len(), 0);
    assert_eq!(
        groups(&["--name", "execute"]),
        groups(&["--at", "1:21", "src/app.ts"])
    );
    assert_eq!(groups(&["--name", "Start"]), Vec::<Value>::new());
    assert_eq!(groups(&["--at", "2:7", "src/app.ts"]), Vec::<Value>::new());
}

#[test]
fn pretty_and_output_file_keep_their_meanings() {
    let fixture = aliases();
    let output = tsparse(
        &fixture,
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "--pretty",
            "-o",
            "out.json",
        ],
    );
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let text = std::fs::read_to_string(fixture.path("out.json")).unwrap();
    assert!(text.starts_with("{\n  \"operation\": \"references\",\n  \"outcome\": \"complete\","));
    assert!(text.ends_with("}\n"));
    let compact = success(tsparse(
        &fixture,
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "--json",
        ],
    ));
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), compact);
}

#[test]
fn usage_and_io_errors_exit_2() {
    let fixture = aliases();
    for args in [
        &["--references", "--name", "run"][..],
        &["--references", "--project", "tsconfig.json"],
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "--at",
            "1:1",
            "src/app.ts",
        ],
        &["--references", "--project", "tsconfig.json", "--at", "1:1"],
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--at",
            "x:1",
            "src/app.ts",
        ],
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--at",
            "1:1",
            "-",
        ],
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "--check",
        ],
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "--tsx",
        ],
        &["--references", "--project", "missing.json", "--name", "run"],
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--at",
            "9:1",
            "src/app.ts",
        ],
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--at",
            "1:1",
            "missing.ts",
        ],
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "a.ts",
            "b.ts",
        ],
        &["--project", "tsconfig.json", "--name", "run"],
    ] {
        let output = tsparse(&fixture, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn analysis_failures_exit_1_with_located_diagnostics() {
    let fixture = aliases();
    let output = tsparse(
        &fixture,
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--at",
            "4:7",
            "src/app.ts",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let message = String::from_utf8(output.stderr).unwrap();
    assert!(message.contains("src/app.ts:4:7:"), "{message}");

    std::fs::write(
        fixture.path("src/broken.ts"),
        "import { x } from \"./missing\";\n",
    )
    .unwrap();
    let output = tsparse(
        &fixture,
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("broken.ts:1:19:")
    );
}

#[test]
fn a_leading_byte_order_mark_is_not_part_of_the_first_line() {
    // TypeScript's `decodeBytes` drops a leading UTF-8 byte order mark when
    // it reads a source or configuration file. For this `a.ts`, tsc 7.0.2
    // reports TS2322 at a.ts(1,14), the declaration of `a`, and TS2345 at
    // a.ts(1,70), its use; for the configuration, TS1136 at
    // tsconfig.json(1,24). Each holds with or without the mark.
    let line = "export const a: number = \"x\"; declare function g(s: string): void; g(a);\n";
    for prefix in ["", "\u{feff}"] {
        let fixture = project(&[
            ("a.ts", &format!("{prefix}{line}")),
            ("b.ts", "import { a } from \"./a\";\nconst b: string = a;\n"),
        ]);
        let expected = json!([{
            "declarations": [{ "name": "a", "location": location("a.ts", 1, 14, 15) }],
            "occurrences": [
                occurrence("a", "a.ts", 1, 14, &["declaration", "export"]),
                occurrence("a", "a.ts", 1, 70, &["use"]),
                occurrence("a", "b.ts", 1, 10, &["import"]),
                occurrence("a", "b.ts", 2, 19, &["use"]),
            ],
        }]);
        for at in ["1:14", "1:70"] {
            let result = success(tsparse(
                &fixture,
                &[
                    "--references",
                    "--project",
                    "tsconfig.json",
                    "--at",
                    at,
                    "a.ts",
                    "--json",
                ],
            ));
            assert_eq!(result["groups"], expected, "{prefix:?} {at}");
        }
        let result = success(tsparse(
            &fixture,
            &[
                "--references",
                "--project",
                "tsconfig.json",
                "--at",
                "1:15",
                "a.ts",
                "--json",
            ],
        ));
        assert_eq!(result["groups"], json!([]), "{prefix:?}");

        std::fs::write(
            fixture.path("tsconfig.json"),
            format!("{prefix}{{ \"compilerOptions\": {{ , }} }}"),
        )
        .unwrap();
        let output = tsparse(
            &fixture,
            &["--references", "--project", "tsconfig.json", "--name", "a"],
        );
        assert_eq!(output.status.code(), Some(1), "{prefix:?}");
        let message = String::from_utf8(output.stderr).unwrap();
        assert!(
            message.contains("tsconfig.json:1:24:"),
            "{prefix:?} {message}"
        );
    }
}

#[test]
fn a_member_chain_of_any_length_loads_and_labels_its_callback() {
    // The parser reads a member chain in a loop, as TypeScript's
    // `parseMemberExpressionRest` does, so no grammar budget bounds its
    // length. Loading indexes the callback's file for navigation, and its
    // label (`getCallExpressionName`) names the whole chain: neither may
    // recurse over the chain or rebuild its prefix per segment. A chain of
    // 600,000 accesses overflowed the 256 MiB parsing stack in unoptimized
    // builds, and took over 40 seconds in optimized ones.
    const SEGMENTS: usize = 600_000;
    let chain = format!("x{}", ".x".repeat(SEGMENTS));
    let fixture = project(&[(
        "a.ts",
        &format!("export const value = 1;\ndeclare const x: any;\n{chain}(() => {{}});\n"),
    )]);
    let result = success(tsparse(
        &fixture,
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
            "--json",
        ],
    ));
    assert_eq!(
        result["groups"],
        json!([{
            "declarations": [{ "name": "value", "location": location("a.ts", 1, 14, 19) }],
            "occurrences": [occurrence("value", "a.ts", 1, 14, &["declaration", "export"])],
        }])
    );

    // The complete projection, which outlines and shows use in file and
    // project mode alike, labels the callback with the whole chain.
    let outline = success(tsparse(&fixture, &["--outline", "--json", "a.ts"]));
    let entries = outline["entries"].as_array().unwrap();
    let labels: Vec<&str> = entries
        .iter()
        .map(|entry| entry["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["value", "x", &format!("{chain}() callback")]);
    let column = chain.len() + 2;
    assert_eq!(entries[2]["anchor"], json!({ "line": 3, "column": column }));
}
