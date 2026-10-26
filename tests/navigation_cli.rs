//! The `--outline` and `--show` command contract: request validity, text and
//! JSON formats, and the delivery of each outcome to stdout, stderr, `-o`, and
//! exit codes. Which declarations exist is covered by `navigation.rs`.
mod support;

use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use support::{EDGE_CASES, Fixture};

/// Containment follows TypeScript 7.0.2's document symbols for this shape
/// (`startEligible` and `run` inside the factory; the shorthand is a
/// documented difference); the text layout is this tool's contract.
const HOST: &str = "\
export function createRunHost(deps: Deps) {
  function startEligible() {
    return true;
  }
  return { startEligible, run() {} };
}
function overload(a: string): void;
function overload(a: number): void;
function overload(a: unknown) {}
type Deps = {};
";

fn fixture() -> Fixture {
    Fixture::new(&[
        ("host.ts", HOST),
        ("broken.ts", "const = 1;\n"),
        ("data.json", "{}\n"),
        ("empty.ts", ""),
    ])
}

fn tsparse(fixture: &Fixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
        .current_dir(&fixture.root)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn json_output(output: &Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn outline_text_shows_two_levels_and_counts_what_depth_omits() {
    let fixture = fixture();
    let file = fixture.path("host.ts");
    let output = tsparse(&fixture, &["--outline", "host.ts"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty());
    assert_eq!(
        text(&output.stdout),
        format!(
            "{}\n\
             1:17 function export function createRunHost(deps: Deps)\n\
             \x20 2:12 function function startEligible()\n\
             \x20 5:27 method run()\n\
             7:10 function function overload(a: string): void\n\
             8:10 function function overload(a: number): void\n\
             9:10 function function overload(a: unknown)\n\
             10:6 type type Deps = {{}}\n",
            file.display()
        )
    );
    let output = tsparse(&fixture, &["--outline", "--depth", "1", "host.ts"]);
    assert_eq!(output.status.code(), Some(0));
    let shown = text(&output.stdout);
    assert!(
        shown.contains(
            "1:17 function export function createRunHost(deps: Deps)\n  … 2 more children\n7:10"
        ),
        "{shown}"
    );
    let all = tsparse(&fixture, &["--outline", "--depth", "all", "host.ts"]);
    let two = tsparse(&fixture, &["--outline", "--depth", "2", "host.ts"]);
    assert_eq!(all.stdout, two.stdout);
}

#[test]
fn outline_json_nests_entries_and_reports_omitted_children() {
    let fixture = fixture();
    let file = fixture.path("host.ts");
    let json = json_output(
        &tsparse(
            &fixture,
            &["--outline", "--json", "--depth", "1", "host.ts"],
        ),
        0,
    );
    assert_eq!(json["operation"], "outline");
    assert_eq!(json["outcome"], "ok");
    assert_eq!(json["file"], file.to_str().unwrap());
    assert_eq!(
        json["entries"][0],
        json!({
            "label": "createRunHost",
            "key": "createRunHost",
            "selector": "createRunHost",
            "kind": "function",
            "anchor": { "line": 1, "column": 17 },
            "range": { "start": { "line": 1, "column": 1 }, "end": { "line": 6, "column": 2 } },
            "signature": "export function createRunHost(deps: Deps)",
            "children": [],
            "omitted_children": 2,
        })
    );
    let json = json_output(
        &tsparse(&fixture, &["--outline", "--json", "--pretty", "host.ts"]),
        0,
    );
    let children = &json["entries"][0]["children"];
    assert_eq!(children[1]["label"], "run");
    assert_eq!(children[1]["selector"], "createRunHost.run");
    assert!(children[1].get("omitted_children").is_none());
    assert_eq!(json["entries"].as_array().unwrap().len(), 5);
}

/// Text gives each import declaration one line, anchored at its first name,
/// whose `--at` shows the whole declaration; the catalog and JSON keep one
/// entry per imported name.
#[test]
fn text_outlines_give_each_import_declaration_one_line() {
    let fixture = Fixture::new(&[(
        "imports.ts",
        "import def, { named as alias, type Only } from \"./other\";\nimport * as ns from \"./other\";\nimport \"./side-effect\";\nimport { single } from \"./other\";\nimport Req = require(\"./other\");\nnamespace N {\n  import { a, b } from \"./other\";\n}\nexport { def, alias };\n",
    )]);
    let output = tsparse(&fixture, &["--outline", "imports.ts"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!(
            "{}\n\
             1:8 import def, alias, Only\n\
             2:13 import ns\n\
             4:10 import single\n\
             5:8 import import Req = require(\"./other\")\n\
             6:11 namespace namespace N\n\
             \x20 7:12 import a, b\n\
             9:10 export def\n\
             9:15 export alias\n",
            fixture.path("imports.ts").display()
        )
    );
    let json = json_output(
        &tsparse(&fixture, &["--outline", "--json", "imports.ts"]),
        0,
    );
    let labels: Vec<&str> = json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["label"].as_str().unwrap())
        .collect();
    assert_eq!(
        labels,
        [
            "def", "alias", "Only", "ns", "single", "Req", "N", "def", "alias"
        ]
    );
    let shown = tsparse(&fixture, &["--show", "--at", "1:8", "imports.ts"]);
    assert_eq!(
        text(&shown.stdout),
        format!(
            "{}:1:8 import def\n1 | import def, {{ named as alias, type Only }} from \"./other\";\n",
            fixture.path("imports.ts").display()
        )
    );
}

#[test]
fn a_long_import_line_names_its_first_bindings_and_counts_the_rest() {
    let names: Vec<String> = (1..=8).map(|n| format!("n{n}")).collect();
    let at_limit = format!("import {{ {} }} from \"./other\";\n", names.join(", "));
    let over_limit = format!(
        "import {{ {}, n9, n10 }} from \"./other\";\n",
        names.join(", ")
    );
    let fixture = Fixture::new(&[
        ("at.ts", at_limit.as_str()),
        ("over.ts", over_limit.as_str()),
    ]);
    let at = tsparse(&fixture, &["--outline", "at.ts"]);
    assert_eq!(
        text(&at.stdout),
        format!(
            "{}\n1:10 import {}\n",
            fixture.path("at.ts").display(),
            names.join(", ")
        )
    );
    let over = tsparse(&fixture, &["--outline", "over.ts"]);
    assert_eq!(
        text(&over.stdout),
        format!(
            "{}\n1:10 import {}, … 2 more\n",
            fixture.path("over.ts").display(),
            names.join(", ")
        )
    );
    // Every binding stays in JSON, with its own anchor.
    let json = json_output(&tsparse(&fixture, &["--outline", "--json", "over.ts"]), 0);
    assert_eq!(json["entries"].as_array().unwrap().len(), 10);
}

#[test]
fn an_empty_file_has_an_empty_outline() {
    let fixture = fixture();
    let output = tsparse(&fixture, &["--outline", "empty.ts"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        text(&output.stdout),
        format!("{}\n", fixture.path("empty.ts").display())
    );
    let json = json_output(&tsparse(&fixture, &["--outline", "--json", "empty.ts"]), 0);
    assert_eq!(json["entries"], json!([]));
}

#[test]
fn show_prints_the_exact_declaration_with_original_line_numbers() {
    let fixture = fixture();
    let file = fixture.path("host.ts");
    let output = tsparse(
        &fixture,
        &["--show", "--name", "createRunHost.startEligible", "host.ts"],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty());
    assert_eq!(
        text(&output.stdout),
        format!(
            "{}:2:12 function startEligible\n\
             2 | function startEligible() {{\n\
             3 |     return true;\n\
             4 |   }}\n",
            file.display()
        )
    );
    let json = json_output(
        &tsparse(&fixture, &["--show", "--json", "--name", "run", "host.ts"]),
        0,
    );
    assert_eq!(json["operation"], "show");
    assert_eq!(json["outcome"], "found");
    assert_eq!(json["file"], file.to_str().unwrap());
    assert_eq!(json["source"], "run() {}");
    assert_eq!(json["declaration"]["selector"], "createRunHost.run");
    assert_eq!(json["declaration"]["kind"], "method");
}

#[test]
fn ambiguity_lists_candidates_whose_anchors_select_one_declaration() {
    let fixture = fixture();
    let file = fixture.path("host.ts");
    let output = tsparse(&fixture, &["--show", "--name", "overload", "host.ts"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        text(&output.stderr),
        format!(
            "{0}: `overload` matches 3 declarations; select one with --at FILE:LINE:COLUMN\n\
             {0}:7:10 function overload (overload)\n\
             {0}:8:10 function overload (overload)\n\
             {0}:9:10 function overload (overload)\n",
            file.display()
        )
    );
    // A printed location is an `--at` operand on its own.
    let place = format!("{}:8:10", file.display());
    let shown = tsparse(&fixture, &["--show", "--at", &place]);
    assert_eq!(
        text(&shown.stdout),
        format!("{place} function overload\n8 | function overload(a: number): void;\n")
    );

    let json = json_output(
        &tsparse(
            &fixture,
            &["--show", "--json", "--name", "overload", "host.ts"],
        ),
        1,
    );
    assert_eq!(json["outcome"], "ambiguous");
    assert_eq!(json["query"], json!({ "name": "overload" }));
    let candidates = json["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 3);
    assert_eq!(
        candidates[1],
        json!({
            "file": file.to_str().unwrap(),
            "label": "overload",
            "kind": "function",
            "key": "overload",
            "selector": "overload",
            "anchor": { "line": 8, "column": 10 },
        })
    );
    // The returned file and anchor select exactly that candidate.
    let at = format!(
        "{}:{}",
        candidates[1]["anchor"]["line"], candidates[1]["anchor"]["column"]
    );
    let chosen = candidates[1]["file"].as_str().unwrap();
    let json = json_output(
        &tsparse(&fixture, &["--show", "--json", "--at", &at, chosen]),
        0,
    );
    assert_eq!(json["source"], "function overload(a: number): void;");
    assert_eq!(
        json["declaration"]["anchor"],
        json!({ "line": 8, "column": 10 })
    );
}

#[test]
fn missing_targets_exit_1_with_the_selector() {
    let fixture = fixture();
    let file = fixture.path("host.ts");
    let output = tsparse(&fixture, &["--show", "--name", "absent", "host.ts"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        text(&output.stderr),
        format!("{}: no declaration matches `absent`\n", file.display())
    );
    // `true` in `return true;` is neither an entry's target nor a
    // statement's first token.
    let output = tsparse(&fixture, &["--show", "--at", "3:12", "host.ts"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        text(&output.stderr),
        format!(
            "{}:3:12: no declaration is named or anchored at this position\n",
            file.display()
        )
    );
    let json = json_output(
        &tsparse(&fixture, &["--show", "--json", "--at", "3:12", "host.ts"]),
        1,
    );
    assert_eq!(
        json,
        json!({
            "operation": "show",
            "outcome": "not_found",
            "file": file.to_str().unwrap(),
            "query": { "at": { "line": 3, "column": 12 } },
        })
    );
    // Its `return` begins the statement, which it selects.
    let output = tsparse(&fixture, &["--show", "--at", "3:5", "host.ts"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        text(&output.stdout),
        format!(
            "{}:3:5 statement return true\n3 | return true;\n",
            file.display()
        )
    );
}

#[test]
fn analysis_failures_are_typed_outcomes_with_their_exit_codes() {
    let fixture = fixture();
    // (arguments, exit code, error kind)
    let cases: [(&[&str], i32, &str); 6] = [
        (&["--outline", "broken.ts"], 1, "Syntax"),
        (&["--show", "--name", "x", "broken.ts"], 1, "Syntax"),
        (&["--outline", "data.json"], 1, "Unsupported"),
        (&["--outline", "missing.ts"], 2, "Io"),
        (&["--show", "--at", "99:1", "host.ts"], 2, "InvalidPosition"),
        (&["--show", "--at", "1:99", "host.ts"], 2, "InvalidPosition"),
    ];
    for (args, code, kind) in cases {
        let output = tsparse(&fixture, args);
        assert_eq!(output.status.code(), Some(code), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");

        let mut json_args = args.to_vec();
        json_args.insert(1, "--json");
        let json = json_output(&tsparse(&fixture, &json_args), code);
        assert_eq!(json["outcome"], "error", "{args:?}");
        assert_eq!(json["error"]["kind"], kind, "{args:?}");
    }
    let output = tsparse(&fixture, &["--outline", "broken.ts"]);
    assert_eq!(
        text(&output.stderr),
        format!(
            "{}:1:7: Expected a binding identifier\n",
            fixture.path("broken.ts").display()
        )
    );
    let json = json_output(&tsparse(&fixture, &["--outline", "--json", "broken.ts"]), 1);
    assert_eq!(json["error"]["position"], json!({ "line": 1, "column": 7 }));
}

#[test]
fn output_files_receive_results_and_json_outcomes_but_never_text_failures() {
    let fixture = fixture();
    let out = fixture.path("out.txt");
    let output = tsparse(&fixture, &["--outline", "host.ts", "-o", "out.txt"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert!(
        std::fs::read_to_string(&out)
            .unwrap()
            .contains("createRunHost")
    );
    std::fs::remove_file(&out).unwrap();

    for args in [
        &["--show", "--name", "overload", "host.ts", "-o", "out.txt"][..],
        &["--show", "--name", "absent", "host.ts", "-o", "out.txt"],
        &["--outline", "broken.ts", "-o", "out.txt"],
        &["--outline", "missing.ts", "-o", "out.txt"],
    ] {
        let output = tsparse(&fixture, args);
        assert_ne!(output.status.code(), Some(0), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
        assert!(!out.exists(), "{args:?} created the output file");
    }

    for (args, code) in [
        (
            &[
                "--show", "--json", "--name", "overload", "host.ts", "-o", "out.txt",
            ][..],
            1,
        ),
        (&["--outline", "--json", "missing.ts", "-o", "out.txt"], 2),
    ] {
        let output = tsparse(&fixture, args);
        assert_eq!(output.status.code(), Some(code), "{args:?}");
        assert!(
            output.stdout.is_empty() && output.stderr.is_empty(),
            "{args:?}"
        );
        let json: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert!(matches!(
            json["outcome"].as_str(),
            Some("ambiguous" | "error")
        ));
        std::fs::remove_file(&out).unwrap();
    }

    let output = tsparse(
        &fixture,
        &["--outline", "host.ts", "-o", "missing-dir/out.txt"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty() && !output.stderr.is_empty());
}

#[test]
fn invalid_requests_are_usage_errors() {
    let fixture = fixture();
    for args in [
        &["--outline"][..],
        &["--outline", "host.ts", "broken.ts"],
        &["--outline", "-"],
        &["--outline", "--direct", "host.ts"],
        &["--outline", "--check", "host.ts"],
        &["--outline", "--tsx", "host.ts"],
        &["--outline", "--pretty", "host.ts"],
        &["--outline", "--name", "run", "host.ts"],
        &["--outline", "--at", "1:1", "host.ts"],
        &["--outline", "--depth", "0", "host.ts"],
        &["--outline", "--depth", "-1", "host.ts"],
        &["--outline", "--depth", "two", "host.ts"],
        &["--outline", "--depth"],
        &["--outline", "--show", "--name", "run", "host.ts"],
        &[
            "--outline",
            "--references",
            "--project",
            "tsconfig.json",
            "host.ts",
        ],
        &["--show", "host.ts"],
        &["--show", "--name", "run"],
        &["--show", "--at", "1:1"],
        &["--show", "--name", "run", "--at", "1:1", "host.ts"],
        &["--show", "--at", "x:1", "host.ts"],
        // A location names its file once: in `--at`, or as the operand.
        &["--show", "--at", "host.ts:1:17", "host.ts"],
        &["--show", "--at", ":1:17"],
        &["--show", "--at", "host.ts:1:x"],
        &["--show", "--at", "host.ts:0:17"],
        &["--show", "--depth", "2", "--name", "run", "host.ts"],
        // A value option given twice conflicts with itself.
        &["--show", "--name", "x", "--name", "run", "host.ts"],
        &["--show", "--at", "1:1", "--at", "1:17", "host.ts"],
        &["--outline", "--depth", "1", "--depth", "all", "host.ts"],
        &["--outline", "-o", "a.txt", "--output", "out.txt", "host.ts"],
        &["--name", "run", "host.ts"],
        &["--json", "host.ts"],
        &["--depth", "2", "host.ts"],
        &["--direct", "host.ts"],
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "--depth",
            "2",
        ],
    ] {
        let output = tsparse(&fixture, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(text(&output.stderr).starts_with("usage."), "{args:?}");
        assert!(!fixture.path("out.txt").exists(), "{args:?}");
    }
    // With `--json`, which show takes, a usage failure is one document on
    // stdout, and the `-o` the invalid request named is never written.
    let output = tsparse(
        &fixture,
        &[
            "--show", "--json", "--pretty", "--name", "run", "-", "-o", "out.txt",
        ],
    );
    assert!(text(&output.stdout).lines().count() > 1, "{output:?}");
    assert_eq!(
        json_output(&output, 2),
        json!({
            "operation": "show",
            "outcome": "error",
            "error": {
                "category": "usage",
                "code": "usage.operands",
                "message": "--show reads a named FILE, not stdin",
            },
        })
    );
    assert!(!fixture.path("out.txt").exists());
}

/// `--at FILE:LINE:COLUMN` stands for `--at LINE:COLUMN FILE`, read from
/// the right, so a file name may contain `:`, as a Windows path does.
#[test]
fn a_location_names_its_file_in_at_or_as_the_operand() {
    let fixture = fixture();
    let host = fixture.path("host.ts");
    let operand = tsparse(&fixture, &["--show", "--at", "2:12", "host.ts"]);
    for place in [
        "host.ts:2:12".to_owned(),
        format!("{}:2:12", host.display()),
    ] {
        let output = tsparse(&fixture, &["--show", "--at", &place]);
        assert_eq!(output.status.code(), Some(0), "{place}: {output:?}");
        assert_eq!(output.stdout, operand.stdout, "{place}");
    }
    let json = json_output(
        &tsparse(&fixture, &["--show", "--json", "--at", r"C:\dir\x.ts:3:4"]),
        2,
    );
    assert_eq!(json["error"]["kind"], "Io", "{json}");
    assert!(
        json["error"]["path"]
            .as_str()
            .unwrap()
            .ends_with(r"C:\dir\x.ts"),
        "{json}"
    );
    for (args, message) in [
        (
            &["--show", "--at", "host.ts:2:12", "host.ts"][..],
            "usage.operands: --at FILE:LINE:COLUMN names the file; omit the FILE operand",
        ),
        (
            &["--show", "--at", ":2:12"],
            "usage.invalid_value: --at expects LINE:COLUMN or FILE:LINE:COLUMN, got :2:12",
        ),
        (
            &["--show", "--at", "host.ts:0:12"],
            "usage.invalid_value: --at expects LINE:COLUMN or FILE:LINE:COLUMN, got host.ts:0:12",
        ),
        (
            &["--show", "--at", "2:12"],
            "usage.operands: --show --at needs a FILE operand or FILE:LINE:COLUMN",
        ),
    ] {
        let output = tsparse(&fixture, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert_eq!(text(&output.stderr), format!("{message}\n"), "{args:?}");
    }
}

#[test]
fn display_depth_never_changes_what_parses() {
    // 200 nested calls need more grammar entries than parse mode's default
    // budget of 256 but fit navigation's deep budget of 1,024.
    let source = format!(
        "{}x{};\nfunction after() {{}}\n",
        "f(".repeat(200),
        ")".repeat(200)
    );
    let fixture = Fixture::new(&[("deep.ts", &source)]);
    let parse = tsparse(&fixture, &["--check", "deep.ts"]);
    assert_eq!(
        parse.status.code(),
        Some(1),
        "parse mode keeps its default budget"
    );
    for depth in ["1", "2", "all"] {
        let output = tsparse(&fixture, &["--outline", "--depth", depth, "deep.ts"]);
        assert_eq!(output.status.code(), Some(0), "{depth}: {output:?}");
        assert!(text(&output.stdout).ends_with("2:10 function function after()\n"));
    }
}

#[test]
fn direct_answers_a_reference_query_in_this_process() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        ("app.ts", "export function run() {}\nrun();\n"),
    ]);
    let query = [
        "--references",
        "--project",
        "tsconfig.json",
        "--name",
        "run",
        "--json",
    ];
    let direct = tsparse(&fixture, &[&query[..], &["--direct"]].concat());
    assert_eq!(direct.status.code(), Some(0), "{direct:?}");
    let json: Value = serde_json::from_slice(&direct.stdout).unwrap();
    assert_eq!(
        json["groups"][0]["occurrences"].as_array().unwrap().len(),
        2
    );
    // Service actions take no query mode.
    let output = tsparse(
        &fixture,
        &[
            "--references-status",
            "--project",
            "tsconfig.json",
            "--direct",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn json_outcomes_whose_result_cannot_be_written_exit_2_on_stderr_without_a_document() {
    let fixture = fixture();
    for args in [
        &["--outline", "--json", "host.ts"][..],
        &["--show", "--json", "--name", "run", "host.ts"],
        &["--show", "--json", "--name", "overload", "host.ts"],
        &["--show", "--json", "--name", "absent", "host.ts"],
        &["--outline", "--json", "broken.ts"],
        &["--outline", "--json", "missing.ts"],
        &["--outline", "host.ts"],
    ] {
        let output = tsparse(&fixture, &[args, &["-o", "missing-dir/out.json"]].concat());
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        let message = text(&output.stderr);
        assert!(
            message.starts_with("write.output: missing-dir/out.json: "),
            "{args:?}: {message}"
        );
        assert_eq!(message.lines().count(), 1, "no second document: {message}");
        assert!(!fixture.path("missing-dir").exists());
    }
}

#[test]
fn text_failures_leave_an_existing_output_file_unchanged() {
    let fixture = fixture();
    let out = fixture.path("out.txt");
    for args in [
        &["--show", "--name", "overload", "host.ts"][..],
        &["--show", "--name", "absent", "host.ts"],
        &["--show", "--at", "99:1", "host.ts"],
        &["--outline", "broken.ts"],
        &["--outline", "data.json"],
        &["--outline", "missing.ts"],
    ] {
        std::fs::write(&out, "previous result\n").unwrap();
        let output = tsparse(&fixture, &[args, &["-o", "out.txt"]].concat());
        assert_ne!(output.status.code(), Some(0), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "previous result\n",
            "{args:?}"
        );
    }
    // A successful result replaces it.
    let output = tsparse(&fixture, &["--outline", "host.ts", "-o", "out.txt"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        std::fs::read_to_string(&out)
            .unwrap()
            .contains("createRunHost")
    );
}

#[test]
fn folded_case_extensions_follow_the_file_system_case_policy() {
    // Input classification folds case where the file system does, as
    // project loading does; the parse follows TypeScript, whose script kind
    // compares the extension case-insensitively, so `Upper.TSX` is TSX.
    let fixture = Fixture::new(&[("Upper.TSX", "export const view = <div>text</div>;\n")]);
    if !fixture.path("upper.tsx").exists() {
        println!(
            "SKIP folded_case_extensions_follow_the_file_system_case_policy: the file system is case-sensitive, so `.TSX` is unsupported input"
        );
        let output = tsparse(&fixture, &["--outline", "Upper.TSX"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        return;
    }
    let output = tsparse(&fixture, &["--outline", "Upper.TSX"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!(
            "{}\n1:14 variable export const view = <div>text</div>\n",
            fixture.path("Upper.TSX").display()
        )
    );
}

#[test]
fn text_show_numbers_the_lines_after_every_line_break() {
    // CR, LF, CRLF, U+2028, and U+2029 each end a line in the excerpt as in
    // the file, so the printed numbers match the outline's.
    let source =
        "// lead\r\nfunction f() {\r  a;\n  b;\r\n  c;\u{2028}  d;\u{2029}}\nconst after = 1;\n";
    let fixture = Fixture::new(&[("breaks.ts", source)]);
    let file = fixture.path("breaks.ts");
    let output = tsparse(&fixture, &["--show", "--name", "f", "breaks.ts"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!(
            "{}:2:10 function f\n\
             2 | function f() {{\n\
             3 |   a;\n\
             4 |   b;\n\
             5 |   c;\n\
             6 |   d;\n\
             7 | }}\n",
            file.display()
        )
    );
    let json = json_output(
        &tsparse(&fixture, &["--show", "--json", "--name", "f", "breaks.ts"]),
        0,
    );
    assert_eq!(
        json["source"],
        "function f() {\r  a;\n  b;\r\n  c;\u{2028}  d;\u{2029}}"
    );
    assert_eq!(
        json["declaration"]["range"],
        json!({ "start": { "line": 2, "column": 1 }, "end": { "line": 7, "column": 2 } })
    );
    let output = tsparse(&fixture, &["--outline", "breaks.ts"]);
    assert!(text(&output.stdout).ends_with("\n8:7 variable const after = 1\n"));
}

#[test]
fn selectors_with_empty_segments_select_nothing() {
    let fixture = fixture();
    for name in ["", ".", "createRunHost..run", "createRunHost.", ".run"] {
        let output = tsparse(&fixture, &["--show", "--name", name, "host.ts"]);
        assert_eq!(output.status.code(), Some(1), "{name:?}");
        assert!(output.stdout.is_empty());
        assert_eq!(
            text(&output.stderr),
            format!(
                "{}: no declaration matches `{name}`\n",
                fixture.path("host.ts").display()
            )
        );
        let json = json_output(
            &tsparse(&fixture, &["--show", "--json", "--name", name, "host.ts"]),
            1,
        );
        assert_eq!(json["outcome"], "not_found", "{name:?}");
        assert_eq!(json["query"], json!({ "name": name }));
    }
}

#[cfg(unix)]
#[test]
fn arguments_that_are_not_unicode_are_usage_errors() {
    use std::os::unix::ffi::OsStrExt;
    let fixture = fixture();
    let name = std::ffi::OsStr::from_bytes(b"\xff.ts");
    for operation in ["--outline", "--check"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .arg(operation)
            .arg(name)
            .current_dir(&fixture.root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        assert_eq!(
            text(&output.stderr),
            "usage.not_unicode: Arguments must be Unicode: \u{FFFD}.ts\n"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_working_directory_that_is_not_unicode_is_unsupported_input() {
    use std::os::unix::ffi::OsStrExt;
    // The absolute path of `x.ts` cannot be named exactly. JSON keeps exact
    // paths out of identity fields; only the message shows a lossy form.
    let fixture = fixture();
    let directory = fixture.root.join(std::ffi::OsStr::from_bytes(b"dir-\xff"));
    if let Err(error) = std::fs::create_dir(&directory) {
        // Apple file systems refuse names that are not UTF-8.
        if !cfg!(target_vendor = "apple") {
            panic!("{error}");
        }
        println!(
            "SKIP a_working_directory_that_is_not_unicode_is_unsupported_input: the file system refuses names that are not UTF-8 ({error})"
        );
        return;
    }
    std::fs::write(directory.join("x.ts"), "const x = 1;\n").unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args(args)
            .current_dir(&directory)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let output = run(&["--outline", "x.ts"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    let message = text(&output.stderr);
    assert!(
        message.contains("dir-\u{FFFD}/x.ts: the file name is not Unicode"),
        "{message}"
    );
    let json = json_output(&run(&["--outline", "--json", "x.ts"]), 1);
    assert_eq!(json["outcome"], "error");
    assert_eq!(json["file"], Value::Null);
    assert_eq!(json["error"]["kind"], "Unsupported");
    assert_eq!(json["error"]["path"], Value::Null);
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("dir-\u{FFFD}/x.ts: the file name is not Unicode")
    );
}

fn edge_cases() -> Fixture {
    Fixture::new(&[("edge-cases.test.ts", EDGE_CASES)])
}

#[test]
fn a_statement_s_first_token_selects_it_as_a_statement_site() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let name = file.to_str().unwrap();
    // Any position in the first token selects the statement.
    for at in ["10:3", "10:4"] {
        let output = tsparse(&fixture, &["--show", "--at", at, "edge-cases.test.ts"]);
        assert_eq!(output.status.code(), Some(0), "{at}: {output:?}");
        assert!(output.stderr.is_empty(), "{at}");
        assert_eq!(
            text(&output.stdout),
            format!(
                "{name}:10:3 statement it.skip(\"skipped one\", () => {{ … }})\n\
                 10 | it.skip(\"skipped one\", () => {{}});\n"
            ),
            "{at}"
        );
        let json = json_output(
            &tsparse(
                &fixture,
                &["--show", "--json", "--at", at, "edge-cases.test.ts"],
            ),
            0,
        );
        assert_eq!(
            json,
            json!({
                "declaration": {
                    "anchor": { "column": 3, "line": 10 },
                    "kind": "statement",
                    "label": "it.skip(\"skipped one\", () => { … })",
                    "range": {
                        "end": { "column": 36, "line": 10 },
                        "start": { "column": 3, "line": 10 },
                    },
                },
                "file": name,
                "operation": "show",
                "outcome": "found",
                "source": "it.skip(\"skipped one\", () => {});",
            }),
            "{at}"
        );
    }
    // The token's end, the `.`, is outside it.
    let output = tsparse(&fixture, &["--show", "--at", "10:5", "edge-cases.test.ts"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    assert_eq!(
        text(&output.stderr),
        format!("{name}:10:5: no declaration is named or anchored at this position\n")
    );
    // The hook, whose excerpt is its whole statement.
    let output = tsparse(&fixture, &["--show", "--at", "4:3", "edge-cases.test.ts"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!("{name}:4:3 statement beforeEach(() => {{ … }})\n4 | beforeEach(() => {{}});\n")
    );
    // A statement over several lines is excerpted whole, numbered as it is.
    let output = tsparse(&fixture, &["--show", "--at", &format!("{name}:6:3")]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!(
            "{name}:6:3 statement describe.each([[\"a\"], [\"b\"]])(\"case %s\", (x) => {{ … }})\n\
             6 | describe.each([[\"a\"], [\"b\"]])(\"case %s\", (x) => {{\n\
             7 |     it(`handles ${{\"x\"}}`, () => {{}});\n\
             8 |   }});\n"
        )
    );
}

#[test]
fn an_entry_s_position_keeps_selecting_the_entry() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let name = file.to_str().unwrap();
    // The `it.each` callback's anchor, the `(` of `(n)`.
    let output = tsparse(&fixture, &["--show", "--at", "5:30", "edge-cases.test.ts"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!("{name}:5:30 function <function>\n5 | (n) => {{ expect(n).toBeTruthy(); }}\n")
    );
    // A class's name selects the class; its keyword, the statement.
    let output = tsparse(
        &fixture,
        &["--show", "--json", "--at", "2:7", "edge-cases.test.ts"],
    );
    assert_eq!(json_output(&output, 0)["declaration"]["kind"], "class");
    let output = tsparse(
        &fixture,
        &["--show", "--json", "--at", "2:1", "edge-cases.test.ts"],
    );
    let json = json_output(&output, 0);
    assert_eq!(json["declaration"]["kind"], "statement");
    assert_eq!(json["declaration"]["label"], "class Widget { … }");
    assert_eq!(json["source"], "class Widget {}");
}

#[test]
fn an_import_keyword_selects_its_declaration_as_a_statement_site() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let name = file.to_str().unwrap();
    let output = tsparse(&fixture, &["--show", "--at", "1:1", "edge-cases.test.ts"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!(
            "{name}:1:1 statement import {{ describe, it, test, beforeEach, expect }} from \"vitest\"\n\
             1 | import {{ describe, it, test, beforeEach, expect }} from \"vitest\";\n"
        )
    );
    // A statement site has no key, selector, or signature.
    let json = json_output(
        &tsparse(
            &fixture,
            &["--show", "--json", "--at", "1:1", "edge-cases.test.ts"],
        ),
        0,
    );
    let declaration = json["declaration"].as_object().unwrap();
    let mut fields: Vec<&str> = declaration.keys().map(String::as_str).collect();
    fields.sort_unstable();
    assert_eq!(fields, ["anchor", "kind", "label", "range"]);
    // A specifier's name still selects the specifier, with the same excerpt.
    let specifier = json_output(
        &tsparse(
            &fixture,
            &["--show", "--json", "--at", "1:10", "edge-cases.test.ts"],
        ),
        0,
    );
    assert_eq!(specifier["declaration"]["kind"], "import");
    assert_eq!(specifier["source"], json["source"]);
}

#[test]
fn pinned_outlines_are_the_catalog_s_own_output() {
    // tools/navigation-conformance.test.mjs mutates these entries to prove
    // that the conformance comparison fails on regressions, so they must be
    // what the tool outputs: every entry, which it asks for with --limit all.
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/navigation");
    let mut pinned = 0;
    for entry in std::fs::read_dir(&fixtures).unwrap() {
        let path = entry.unwrap().path();
        let Some(source) = path
            .to_str()
            .and_then(|path| path.strip_suffix(".outline.json"))
        else {
            continue;
        };
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args([
                "--outline",
                "--json",
                "--depth",
                "all",
                "--limit",
                "all",
                source,
            ])
            .output()
            .unwrap();
        let expected: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            json_output(&output, 0)["entries"],
            expected["entries"],
            "regenerate with node tools/navigation-conformance.mjs --write-outlines {source}"
        );
        pinned += 1;
    }
    assert!(pinned >= 2, "{pinned} pinned outlines");
}
