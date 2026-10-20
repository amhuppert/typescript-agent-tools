//! A batch file is decoded and validated whole before anything is prepared:
//! a file that is not JSON fails with the one issue serde reports, at its
//! line and column; any other invalid batch reports every issue, each named
//! by its JSON path with zero-based indexes. Each failure is `input.batch`,
//! exit 2, the same through named files, a project loaded here, and the
//! project's service, none of which it reaches, and points to the help that
//! describes the format. A valid batch prepares as before.
mod support;

use serde_json::{Value, json};
use std::process::{Command, Output, Stdio};
use support::Fixture;
use support::service::Service;

const HOST: &str = "export function start() {\n  return 1;\n}\n";

fn fixture() -> Fixture {
    support::project(&[("host.ts", HOST), ("body.ts", "{ return 2; }")])
}

/// Run the binary in `fixture`, with a runtime directory inside it that
/// nothing creates unless a service is started.
fn tsparse(fixture: &Fixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
        .current_dir(&fixture.root)
        .env("TSPARSE_SERVICE_DIR", fixture.path("services"))
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The entries a batch can be prepared through: named files, a project
/// loaded in this process, and the project's service.
const ENTRIES: [&[&str]; 3] = [
    &[],
    &["--project", "tsconfig.json", "--direct"],
    &["--project", "tsconfig.json"],
];

/// The command every batch failure names, which prints the batch format.
const HELP: [&str; 4] = ["tsparse", "--edit-batch", "EDITS", "--help"];

/// The failure of the batch in `edits.json` holding `batch`, whose message
/// after the file name is `summary` and whose issues are `issues`: in text
/// on stderr and with `--json` as one document on stdout, exit 2, through
/// every entry, none of which starts a service. With `-o`, which the
/// batch's unknown file names could alias, it is one stderr diagnostic in
/// either format, and nothing is written.
fn refused(batch: &str, summary: &str, issues: &[(&str, &str)]) {
    let fixture = fixture();
    std::fs::write(fixture.path("edits.json"), batch).unwrap();
    let message = format!("edits.json: {summary}");
    let mut lines = String::new();
    for (path, issue) in issues {
        lines.push_str(&format!("  {path}: {issue}\n"));
    }
    let next = "  next: tsparse --edit-batch EDITS --help\n";
    let expected = json!({
        "operation": "edit",
        "outcome": "error",
        "effect": "none",
        "error": {
            "category": "input",
            "code": "input.batch",
            "message": message,
            "issues": issues
                .iter()
                .map(|(path, message)| json!({"path": path, "message": message}))
                .collect::<Vec<_>>(),
            "next": {"argv": HELP},
        },
    });
    for entry in ENTRIES {
        let args = [&["--edit-batch", "edits.json"][..], entry].concat();
        let output = tsparse(&fixture, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        assert_eq!(
            text(&output.stderr),
            format!("input.batch: {message}\n{lines}{next}"),
            "{args:?}"
        );

        let json = [&args[..], &["--json"]].concat();
        let output = tsparse(&fixture, &json);
        assert_eq!(output.status.code(), Some(2), "{json:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{json:?}: {output:?}");
        let stdout = text(&output.stdout);
        assert_eq!(stdout.lines().count(), 1, "{stdout}");
        let document: Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(document, expected, "{json:?}");

        for format in [&[][..], &["--json"]] {
            let to_file = [&args[..], format, &["-o", "out.json"]].concat();
            let output = tsparse(&fixture, &to_file);
            assert_eq!(output.status.code(), Some(2), "{to_file:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{to_file:?}: {output:?}");
            assert_eq!(
                text(&output.stderr),
                format!(
                    "input.batch: {message}; the result is not written to out.json\n{lines}  why: the batch's file names cannot be established, so the result's path could be one of them\n{next}"
                ),
                "{to_file:?}"
            );
            assert!(!fixture.path("out.json").exists(), "{to_file:?}");
        }
        assert!(!fixture.path("services").exists(), "{args:?}");
    }
}

#[test]
fn a_batch_with_several_invalid_edits_reports_every_issue() {
    refused(
        r#"{"edits": [
            {"operation": "replace_body", "name": "start", "at": "host.ts:1:17", "fragment": "{}"},
            {"operation": "replace_body", "name": "start", "file": "host.ts", "fragment": "{}", "extra": true},
            {"operation": "insert_after", "at": "host.ts:one", "from": "body.ts"}
        ]}"#,
        "3 edits are invalid",
        &[
            ("edits[0]", "give exactly one of name and at"),
            ("edits[1].extra", "unknown field"),
            (
                "edits[2].at",
                "expects LINE:COLUMN or FILE:LINE:COLUMN, got host.ts:one",
            ),
        ],
    );
}

#[test]
fn a_batch_that_is_not_json_reports_where_decoding_stopped() {
    // The issue is serde's, with the line and column it reports.
    let cause = serde_json::from_str::<Value>("{\"edits\": [\n  {\"operation\": }\n]}")
        .unwrap_err()
        .to_string();
    assert!(cause.ends_with("at line 2 column 17"), "{cause}");
    refused(
        "{\"edits\": [\n  {\"operation\": }\n]}",
        "not JSON",
        &[("$", &cause)],
    );
}

#[test]
fn an_empty_batch_is_an_issue_of_its_edits() {
    refused(
        r#"{"edits": []}"#,
        "not an edit batch",
        &[("edits", "the batch has no edits")],
    );
}

/// A batch, the summary after its file name, and its issues, each a path
/// and a message.
type Refusal = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

/// Every issue the validator distinguishes, each in a batch of its own, as
/// text through named files.
const ISSUES: &[Refusal] = &[
    // The document.
    (
        r#"7"#,
        "not an edit batch",
        &[("$", r#"expected {"edits": [...]}"#)],
    ),
    // The document as an array of its one field, edits.
    (
        r#"[[{"operation": "replace_body", "name": "start", "fragment": "{}"}], 1]"#,
        "not an edit batch",
        &[("$", r#"expected {"edits": [...]}"#)],
    ),
    (
        r#"[{"operation": "replace_body", "name": "start", "fragment": "{}"}]"#,
        "not an edit batch",
        &[("[0]", "expected an array of edits")],
    ),
    (
        "[[]]",
        "not an edit batch",
        &[("[0]", "the batch has no edits")],
    ),
    ("{}", "not an edit batch", &[("edits", "missing field")]),
    (
        r#"{"edits": null}"#,
        "not an edit batch",
        &[("edits", "expected an array of edits")],
    ),
    (
        r#"{"edits": {"operation": "replace_body"}}"#,
        "not an edit batch",
        &[("edits", "expected an array of edits")],
    ),
    (
        r#"{"edits": [], "edits": []}"#,
        "not an edit batch",
        &[
            ("edits", "the batch has no edits"),
            ("edits", "given more than once"),
        ],
    ),
    (
        r#"{"edit": [], "a b": 1}"#,
        "not an edit batch",
        &[
            ("edit", "unknown field"),
            (r#"["a b"]"#, "unknown field"),
            ("edits", "missing field"),
        ],
    ),
    // An edit's shape and members, in the order they are written; an edit
    // written as an array has its six fields in order.
    (
        r#"{"edits": [1, ["replace_body", "start", null, "host.ts", "{}"], ["replace_body", "start", null, "host.ts", "{}", null, null]]}"#,
        "3 edits are invalid",
        &[
            ("edits[0]", "expected an object"),
            ("edits[1]", "expected an object"),
            ("edits[2]", "expected an object"),
        ],
    ),
    (
        r#"[[["replace_body", "start", "1:1", "host.ts", "{}", null], ["rename", 1, null, null, null, null]]]"#,
        "2 edits are invalid",
        &[
            ("[0][0]", "give exactly one of name and at"),
            (
                "[0][1][0]",
                "unknown variant `rename`, expected `replace_body` or `insert_after`",
            ),
            ("[0][1][1]", "expected a string"),
            ("[0][1]", "give exactly one of fragment and from"),
        ],
    ),
    // An operation written as an object names it, with null.
    (
        r#"{"edits": [{"operation": {"replace_body": 1}, "name": "start", "fragment": "{}"}, {"operation": {"rename": null}, "name": "start", "fragment": "{}"}, {"operation": {}, "name": "start", "fragment": "{}"}, {"operation": {"replace_body": null, "replace_body": null}, "name": "start", "fragment": "{}"}]}"#,
        "4 edits are invalid",
        &[
            (
                "edits[0].operation",
                "invalid type: integer `1`, expected unit",
            ),
            (
                "edits[1].operation",
                "unknown variant `rename`, expected `replace_body` or `insert_after`",
            ),
            (
                "edits[2].operation",
                "expected a string, or one member naming the operation with null",
            ),
            (
                "edits[3].operation",
                "expected a string, or one member naming the operation with null",
            ),
        ],
    ),
    (
        r#"{"edits": [{"operation": "replace_body", "name": "start", "fragment": "{}", "name": "start", "fragment.x": 1}]}"#,
        "1 edit is invalid",
        &[
            ("edits[0].name", "given more than once"),
            (r#"edits[0]["fragment.x"]"#, "unknown field"),
        ],
    ),
    (
        r#"{"edits": [{"name": "start", "fragment": "{}"}, {"operation": null, "name": "start", "fragment": "{}"}]}"#,
        "2 edits are invalid",
        &[
            ("edits[0].operation", "missing field"),
            ("edits[1].operation", "expected a string"),
        ],
    ),
    (
        r#"{"edits": [{"operation": "rename", "name": "start", "fragment": "{}"}]}"#,
        "1 edit is invalid",
        &[(
            "edits[0].operation",
            "unknown variant `rename`, expected `replace_body` or `insert_after`",
        )],
    ),
    (
        r#"{"edits": [{"operation": "replace_body", "name": 1, "file": true, "fragment": ["{}"]}, {"operation": "replace_body", "at": {}, "from": 2}]}"#,
        "2 edits are invalid",
        &[
            ("edits[0].name", "expected a string"),
            ("edits[0].file", "expected a string"),
            ("edits[0].fragment", "expected a string"),
            ("edits[1].at", "expected a string"),
            ("edits[1].from", "expected a string"),
        ],
    ),
    // Exactly one of name and at, and of fragment and from; null is absent.
    (
        r#"{"edits": [{"operation": "replace_body", "name": null, "at": null, "fragment": "{}"}, {"operation": "replace_body", "name": "start", "fragment": "{}", "from": "body.ts"}, {"operation": "replace_body", "name": "start", "from": null}]}"#,
        "3 edits are invalid",
        &[
            ("edits[0]", "give exactly one of name and at"),
            ("edits[1]", "give exactly one of fragment and from"),
            ("edits[2]", "give exactly one of fragment and from"),
        ],
    ),
    // An at, with and without file.
    (
        r#"{"edits": [{"operation": "replace_body", "at": "1:17", "fragment": "{}"}, {"operation": "replace_body", "at": "host.ts:1:17", "file": "host.ts", "fragment": "{}"}, {"operation": "replace_body", "at": ":1:17", "fragment": "{}"}, {"operation": "replace_body", "at": "host.ts:0:1", "fragment": "{}"}]}"#,
        "4 edits are invalid",
        &[
            (
                "edits[0].at",
                "LINE:COLUMN needs file, or give FILE:LINE:COLUMN",
            ),
            (
                "edits[1].file",
                "at FILE:LINE:COLUMN names the file; omit file",
            ),
            (
                "edits[2].at",
                "expects LINE:COLUMN or FILE:LINE:COLUMN, got :1:17",
            ),
            (
                "edits[3].at",
                "expects LINE:COLUMN or FILE:LINE:COLUMN, got host.ts:0:1",
            ),
        ],
    ),
    // Every issue of one edit, and of the document beside it.
    (
        r#"{"edits": [{"operation": 7, "name": "start", "at": "x", "file": 1, "fragment": "{}", "from": "body.ts", "extra": null}], "version": 1}"#,
        "not an edit batch",
        &[
            ("edits[0].operation", "expected a string"),
            ("edits[0].file", "expected a string"),
            ("edits[0].extra", "unknown field"),
            ("edits[0]", "give exactly one of name and at"),
            (
                "edits[0].at",
                "expects LINE:COLUMN or FILE:LINE:COLUMN, got x",
            ),
            ("edits[0]", "give exactly one of fragment and from"),
            ("version", "unknown field"),
        ],
    ),
];

#[test]
fn every_issue_is_named_by_its_path() {
    let fixture = fixture();
    for (batch, summary, issues) in ISSUES {
        std::fs::write(fixture.path("edits.json"), batch).unwrap();
        let output = tsparse(&fixture, &["--edit-batch", "edits.json"]);
        assert_eq!(output.status.code(), Some(2), "{batch}: {output:?}");
        assert!(output.stdout.is_empty(), "{batch}: {output:?}");
        let mut expected = format!("input.batch: edits.json: {summary}\n");
        for (path, message) in *issues {
            expected.push_str(&format!("  {path}: {message}\n"));
        }
        expected.push_str("  next: tsparse --edit-batch EDITS --help\n");
        assert_eq!(text(&output.stderr), expected, "{batch}");
    }
}

#[test]
fn the_named_help_describes_the_batch_format() {
    let fixture = fixture();
    let output = tsparse(&fixture, &HELP[1..]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let help = text(&output.stdout);
    assert!(
        help.contains(r#"EDITS is {"edits": [{"operation": "replace_body" or "insert_after", "name" or "at", "file", "fragment" or "from"}]}"#),
        "{help}"
    );
}

/// A valid batch prepares as it did before, in every form its decoding
/// took: a member that is `null` is absent, members may come in any order,
/// the document and an edit may be written as an array of their fields in
/// order (`[edits]`, `[operation, name, at, file, fragment, from]`), and an
/// operation as an object naming it, `{"replace_body": null}`. Each form's
/// preview, through every entry, is its plain form's, and it applies.
#[test]
fn a_valid_batch_prepares_as_before() {
    let service = Service::of(fixture());
    let forms: [(&str, &[&str]); 2] = [
        (
            r#"{"edits": [{"operation": "replace_body", "at": "host.ts:1:17", "from": "body.ts"}]}"#,
            &[
                r#"{"edits": [{"from": "body.ts", "fragment": null, "file": null, "at": "host.ts:1:17", "name": null, "operation": "replace_body"}]}"#,
                r#"[[{"operation": "replace_body", "at": "host.ts:1:17", "from": "body.ts"}]]"#,
                r#"{"edits": [["replace_body", null, "host.ts:1:17", null, null, "body.ts"]]}"#,
                r#"{"edits": [{"operation": {"replace_body": null}, "at": "host.ts:1:17", "from": "body.ts"}]}"#,
                r#"[[[{"replace_body": null}, null, "1:17", "host.ts", null, "body.ts"]]]"#,
            ],
        ),
        (
            r#"{"edits": [{"operation": "replace_body", "name": "start", "file": "host.ts", "fragment": "{ return 2; }"}]}"#,
            &[r#"{"edits": [["replace_body", "start", null, "host.ts", "{ return 2; }", null]]}"#],
        ),
    ];
    for (plain, written) in forms {
        std::fs::write(service.fixture.path("plain.json"), plain).unwrap();
        for entry in ENTRIES {
            let preview = |batch: &str| {
                let output = service
                    .command()
                    .args(["--edit-batch", batch, "--json"])
                    .args(entry)
                    .output()
                    .unwrap();
                assert_eq!(
                    output.status.code(),
                    Some(0),
                    "{batch} {entry:?}: {output:?}"
                );
                assert!(output.stderr.is_empty(), "{batch} {entry:?}: {output:?}");
                let document: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(document["outcome"], "preview", "{document}");
                document
            };
            let expected = preview("plain.json");
            for form in written {
                std::fs::write(service.fixture.path("form.json"), form).unwrap();
                assert_eq!(preview("form.json"), expected, "{form} {entry:?}");
            }
        }
    }
    let run = |args: &[&str]| service.command().args(args).output().unwrap();
    std::fs::write(
        service.fixture.path("form.json"),
        r#"[[[{"replace_body": null}, null, "1:17", "host.ts", null, "body.ts"]]]"#,
    )
    .unwrap();
    let output = run(&["--edit-batch", "form.json", "--plan", "form.plan.json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let output = run(&["--apply", "form.plan.json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        std::fs::read_to_string(service.fixture.path("host.ts")).unwrap(),
        "export function start() { return 2; }\n"
    );
}
