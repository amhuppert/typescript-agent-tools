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
        command.arg("--references-direct");
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
        ],
    ));
    let project = std::fs::canonicalize(fixture.path("tsconfig.json")).unwrap();
    assert_eq!(
        result,
        json!({
            "project": project,
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
        let mut all = vec!["--references", "--project", "tsconfig.json"];
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
    assert!(text.starts_with("{\n  \"project\": "));
    assert!(text.ends_with("}\n"));
    let compact = success(tsparse(
        &fixture,
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
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
fn help_documents_reference_mode_and_its_limits() {
    let fixture = aliases();
    let output = tsparse(&fixture, &["--help"]);
    let text = String::from_utf8(output.stdout).unwrap();
    for needle in [
        "--references --project TSCONFIG",
        "--at LINE:COLUMN",
        "--name NAME",
        "Project references are not followed",
        "an explicit lib setting or lib reference",
        "In reference mode, exit 1",
    ] {
        assert!(text.contains(needle), "help lacks {needle:?}:\n{text}");
    }
}
