//! One typed failure boundary, as the real binary delivers it: a failure
//! that leaves no outcome prints `CODE: message` on stderr, then its issue,
//! `why`, and `next` lines; with `--json` it is one document, never beside
//! stderr; a usage failure's document goes to stdout, never `-o`; and a
//! result that cannot be written is one stderr diagnostic, never followed by
//! a second document. Every code is one `tsparse --help --json` lists.
mod support;

use serde_json::{Value, json};
use std::process::{Command, Output, Stdio};
use support::Fixture;

const HOST: &str = "export function start() {\n  return 1;\n}\n";

/// Why an editing command refuses an output that names a file it reads or
/// edits.
const NEW_FILES: &str = "no outcome may replace a file the command read or edits";

/// A project with one editable file and the body that replaces it.
fn fixture() -> Fixture {
    support::project(&[("host.ts", HOST), ("body.ts", "{ return 2; }")])
}

/// Run the binary in `fixture`, with a runtime directory inside it that
/// nothing creates unless a service is started.
fn tsparse(fixture: &Fixture, args: &[&str]) -> Output {
    tsparse_with(fixture, args, &[])
}

fn tsparse_with(fixture: &Fixture, args: &[&str], variables: &[(&str, &str)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
        .current_dir(&fixture.root)
        .env("TSPARSE_SERVICE_DIR", fixture.path("services"))
        .env_remove("TSPARSE_SERVICE_IDLE_SECS")
        .envs(variables.iter().copied())
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The failure codes help publishes, with their categories.
fn codes() -> Vec<(String, String)> {
    let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(["--help", "--json"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let registry: Value = serde_json::from_slice(&output.stdout).unwrap();
    registry["failure_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["code"].as_str().unwrap().to_string(),
                entry["category"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// A failure in text: exit 2, nothing on stdout, and exactly `stderr`,
/// whose first line begins with a registered code.
fn failed(output: &Output, stderr: &str, args: &[&str]) {
    assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
    assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
    let printed = text(&output.stderr);
    assert_eq!(printed, stderr, "{args:?}");
    let code = printed.split(':').next().unwrap();
    assert!(
        codes().iter().any(|(registered, _)| registered == code),
        "{args:?}: {code}"
    );
}

/// A failure as a document on stdout: exit 2, nothing on stderr, and
/// exactly `document`, whose code is registered in its category.
fn document(output: &Output, expected: Value, args: &[&str]) {
    assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
    assert!(output.stderr.is_empty(), "{args:?}: {output:?}");
    let stdout = text(&output.stdout);
    assert!(
        stdout.ends_with('\n') && stdout.lines().count() == 1,
        "{stdout}"
    );
    let actual: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(actual, expected, "{args:?}");
    registered(&actual);
}

fn registered(document: &Value) {
    let error = &document["error"];
    let code = (
        error["code"].as_str().unwrap().to_string(),
        error["category"].as_str().unwrap().to_string(),
    );
    assert!(codes().contains(&code), "{document}");
}

/// A usage failure's document names the one operation its flags name.
fn usage(operation: &str, code: &str, message: &str) -> Value {
    json!({
        "operation": operation,
        "outcome": "error",
        "error": {"category": "usage", "code": code, "message": message},
    })
}

#[test]
fn usage_failures_print_their_code_in_text_and_are_documents_with_json() {
    let fixture = fixture();
    let unknown = "Unknown option: --ouput; did you mean --output? See tsparse --help";
    let scope = "--calls applies to: --references";
    let at = "--at expects LINE:COLUMN or FILE:LINE:COLUMN, got 3";
    let missing = "Missing value for --depth";
    for (operation, args, code, message) in [
        (
            "outline",
            &["--outline", "--ouput", "host.ts"][..],
            "usage.unknown_option",
            unknown,
        ),
        (
            "outline",
            &["--outline", "--calls", "host.ts"],
            "usage.flag_scope",
            scope,
        ),
        (
            "show",
            &["--show", "--at", "3", "host.ts"],
            "usage.invalid_value",
            at,
        ),
        (
            "outline",
            &["--outline", "host.ts", "--depth"],
            "usage.missing_value",
            missing,
        ),
    ] {
        failed(
            &tsparse(&fixture, args),
            &format!("{code}: {message}\n"),
            args,
        );
        // With --json anywhere a flag stands, and -o, which the invalid
        // request named, never created.
        for json in [
            [&["--json", "-o", "out.json"][..], args].concat(),
            [args, &["--json"]].concat(),
        ] {
            if json.last() == Some(&"--json") && args.last() == Some(&"--depth") {
                // `--json` would be the value of `--depth`, not a flag.
                continue;
            }
            let output = tsparse(&fixture, &json);
            document(&output, usage(operation, code, message), &json);
            assert!(!fixture.path("out.json").exists(), "{json:?}");
        }
    }
    // Editing documents carry their effect, even for a usage failure.
    let args = [
        "--replace-body",
        "--json",
        "--name",
        "start",
        "--at",
        "1:1",
        "--from",
        "body.ts",
        "host.ts",
    ];
    document(
        &tsparse(&fixture, &args),
        json!({
            "operation": "edit",
            "outcome": "error",
            "effect": "none",
            "error": {
                "category": "usage",
                "code": "usage.selection",
                "message": "--replace-body requires exactly one of --at or --name",
            },
        }),
        &args,
    );
    // Flags naming several operations name none.
    let args = [
        "--outline",
        "--show",
        "--json",
        "--name",
        "start",
        "host.ts",
    ];
    document(
        &tsparse(&fixture, &args),
        json!({
            "outcome": "error",
            "error": {
                "category": "usage",
                "code": "usage.operation",
                "message": "Choose one operation: --outline, --show",
            },
        }),
        &args,
    );
    // Applying reports its effect before writing anything.
    let args = ["--apply", "plan.json", "--json", "--name", "start"];
    document(
        &tsparse(&fixture, &args),
        json!({
            "operation": "apply",
            "outcome": "error",
            "effect": "unchanged",
            "error": {
                "category": "usage",
                "code": "usage.selection",
                "message": "--apply takes only the plan: it applies the recorded files and edits without selecting them again",
            },
        }),
        &args,
    );
    // The failure, not the operation, decides: an error that is not about
    // --json is a document even where no operation, or one without a
    // document, is named.
    for (args, operation, code, message) in [
        (
            &["--json", "--ouput"][..],
            None,
            "usage.unknown_option",
            unknown,
        ),
        (
            &["--check", "--calls", "--json", "host.ts"],
            None,
            "usage.flag_scope",
            scope,
        ),
        (
            &[
                "--references-status",
                "--calls",
                "--json",
                "--project",
                "tsconfig.json",
            ],
            Some("references-status"),
            "usage.flag_scope",
            scope,
        ),
    ] {
        let mut expected = json!({
            "outcome": "error",
            "error": {"category": "usage", "code": code, "message": message},
        });
        if let Some(operation) = operation {
            expected["operation"] = json!(operation);
        }
        document(&tsparse(&fixture, args), expected, args);
    }
}

#[test]
fn a_usage_failure_about_json_or_an_argument_that_is_not_unicode_stays_text() {
    let fixture = fixture();
    let json_scope = "usage.flag_scope: --json applies to: --outline, --show, --type, --references, --replace-body, --insert-after, --edit-batch, --apply, --references-status, --doctor\n";
    for args in [
        &["--json", "host.ts"][..],
        &["--references-stop", "--json", "--project", "tsconfig.json"],
        &["--references-serve", "--project", "tsconfig.json", "--json"],
        // The first flag out of scope is --json, so the error is about it.
        &["--check", "--json", "--calls", "host.ts"],
    ] {
        failed(&tsparse(&fixture, args), json_scope, args);
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args(["--outline", "--json"])
            .arg(std::ffi::OsStr::from_bytes(b"host-\xff.ts"))
            .current_dir(&fixture.root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        failed(
            &output,
            "usage.not_unicode: Arguments must be Unicode: host-\u{FFFD}.ts\n",
            &["--outline", "--json", "host-\\xff.ts"],
        );
    }
}

#[test]
fn an_environment_the_service_would_refuse_is_a_usage_failure() {
    let fixture = fixture();
    let idle = [("TSPARSE_SERVICE_IDLE_SECS", "0")];
    let message = "TSPARSE_SERVICE_IDLE_SECS must be a positive integer number of seconds (default 600), got 0";
    for args in [
        &["--outline", "host.ts"][..],
        &["--outline", "host.ts", "--project", "tsconfig.json"],
        &["--references-serve", "--project", "tsconfig.json"],
    ] {
        failed(
            &tsparse_with(&fixture, args, &idle),
            &format!("usage.environment: {message}\n"),
            args,
        );
    }
    // With --json, a document on stdout, never in -o.
    for (operation, args) in [
        (
            "outline",
            &["--outline", "--json", "host.ts", "-o", "out.json"][..],
        ),
        (
            "references",
            &[
                "--references",
                "--json",
                "--name",
                "start",
                "--project",
                "tsconfig.json",
            ],
        ),
    ] {
        document(
            &tsparse_with(&fixture, args, &idle),
            usage(operation, "usage.environment", message),
            args,
        );
        assert!(!fixture.path("out.json").exists());
    }
    let args = [
        "--replace-body",
        "--json",
        "--name",
        "start",
        "--from",
        "body.ts",
        "--project",
        "tsconfig.json",
    ];
    document(
        &tsparse_with(&fixture, &args, &idle),
        json!({
            "operation": "edit",
            "outcome": "error",
            "effect": "none",
            "error": {"category": "usage", "code": "usage.environment", "message": message},
        }),
        &args,
    );
    assert!(!fixture.path("services").exists());
}

#[test]
fn an_input_that_cannot_be_read_is_an_input_failure_delivered_with_the_request() {
    let fixture = fixture();
    let missing = |name: &str| format!("{name}: No such file or directory (os error 2)");
    // Parse mode has no document.
    failed(
        &tsparse(&fixture, &["missing.ts"]),
        &format!("input.read: {}\n", missing("missing.ts")),
        &["missing.ts"],
    );
    // With --project, an input is read before the request reaches the
    // service or the direct entry, so both deliver the file mode's failure
    // and no service starts.
    for entry in [
        &[][..],
        &["--project", "tsconfig.json"],
        &["--project", "tsconfig.json", "--direct"],
    ] {
        let edit = [
            &[
                "--replace-body",
                "--name",
                "start",
                "--from",
                "missing.ts",
                "host.ts",
            ][..],
            entry,
        ]
        .concat();
        failed(
            &tsparse(&fixture, &edit),
            &format!("input.read: {}\n", missing("missing.ts")),
            &edit,
        );
        let expected = json!({
            "operation": "edit",
            "outcome": "error",
            "effect": "none",
            "error": {"category": "input", "code": "input.read", "message": missing("missing.ts")},
        });
        let json = [&edit[..], &["--json"]].concat();
        document(&tsparse(&fixture, &json), expected.clone(), &json);
        // A valid request's failure follows its delivery: -o, as a new file.
        let to_file = [&json[..], &["-o", "out.json"]].concat();
        let output = tsparse(&fixture, &to_file);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(
            output.stdout.is_empty() && output.stderr.is_empty(),
            "{output:?}"
        );
        let written: Value =
            serde_json::from_slice(&std::fs::read(fixture.path("out.json")).unwrap()).unwrap();
        assert_eq!(written, expected);
        std::fs::remove_file(fixture.path("out.json")).unwrap();
        // A batch file that cannot be read: a document without -o; with -o,
        // whose aliases its unread file names could hide, one diagnostic.
        let batch = [&["--edit-batch", "missing.json"][..], entry].concat();
        failed(
            &tsparse(&fixture, &batch),
            &format!("input.read: {}\n", missing("missing.json")),
            &batch,
        );
        let json = [&batch[..], &["--json"]].concat();
        document(
            &tsparse(&fixture, &json),
            json!({
                "operation": "edit",
                "outcome": "error",
                "effect": "none",
                "error": {"category": "input", "code": "input.read", "message": missing("missing.json")},
            }),
            &json,
        );
        for extra in [&["-o", "out.json"][..], &["--json", "-o", "out.json"]] {
            let args = [&batch[..], extra].concat();
            failed(
                &tsparse(&fixture, &args),
                &format!(
                    "input.read: {}; the result is not written to out.json\n  why: the batch's file names cannot be established, so the result's path could be one of them\n",
                    missing("missing.json")
                ),
                &args,
            );
            assert!(!fixture.path("out.json").exists());
        }
    }
    assert!(!fixture.path("services").exists());
}

#[test]
fn a_result_that_cannot_be_written_is_one_diagnostic() {
    let fixture = fixture();
    let unwritable = "missing/out.json: No such file or directory (os error 2)";
    for args in [
        &["--outline", "host.ts", "-o", "missing/out.json"][..],
        &["--outline", "--json", "host.ts", "-o", "missing/out.json"],
        &["host.ts", "-o", "missing/out.json"],
        // The document of a failure that cannot be written is not
        // followed by a second one.
        &[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "missing.ts",
            "host.ts",
            "--json",
            "-o",
            "missing/out.json",
        ],
    ] {
        failed(
            &tsparse(&fixture, args),
            &format!("write.output: {unwritable}\n"),
            args,
        );
    }
}

#[test]
fn a_plan_that_cannot_be_saved_is_delivered_with_the_request() {
    let fixture = fixture();
    let preview = [
        "--replace-body",
        "--name",
        "start",
        "--from",
        "body.ts",
        "host.ts",
        "--plan",
        "missing/plan.json",
    ];
    let message = "missing/plan.json: cannot save the plan: No such file or directory (os error 2)";
    failed(
        &tsparse(&fixture, &preview),
        &format!("write.plan: {message}\n"),
        &preview,
    );
    let expected = json!({
        "operation": "edit",
        "outcome": "error",
        "effect": "none",
        "error": {"category": "write", "code": "write.plan", "message": message},
    });
    let json = [&preview[..], &["--json"]].concat();
    document(&tsparse(&fixture, &json), expected.clone(), &json);
    let to_file = [&json[..], &["-o", "out.json"]].concat();
    let output = tsparse(&fixture, &to_file);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        output.stdout.is_empty() && output.stderr.is_empty(),
        "{output:?}"
    );
    let written: Value =
        serde_json::from_slice(&std::fs::read(fixture.path("out.json")).unwrap()).unwrap();
    assert_eq!(written, expected);
    assert!(!fixture.path("missing").exists());
}

/// No failure creates a fragment the command reads: an output naming one
/// is refused, whether the edit is one on the command line, in a project
/// answered directly or by its service, or in a batch, and through another
/// name of its directory.
#[test]
fn an_output_never_creates_a_fragment_the_command_reads() {
    let fixture = fixture();
    std::fs::write(
        fixture.path("edits.json"),
        r#"{"edits": [{"operation": "replace_body", "name": "start", "file": "host.ts", "from": "absent.ts"}]}"#,
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&fixture.root, fixture.path("here")).unwrap();
    let file = [
        "--replace-body",
        "--name",
        "start",
        "host.ts",
        "--from",
        "absent.ts",
    ];
    let project = [
        "--replace-body",
        "--name",
        "start",
        "--from",
        "absent.ts",
        "--project",
        "tsconfig.json",
    ];
    let mut entries = vec![
        file.to_vec(),
        [&project[..], &["--direct"]].concat(),
        project.to_vec(),
        vec!["--edit-batch", "edits.json"],
    ];
    let mut outputs = vec!["absent.ts"];
    if cfg!(unix) {
        outputs.push("here/absent.ts");
    }
    for entry in entries.drain(..) {
        for output in &outputs {
            let message = format!(
                "--output {output} names a file the command reads or edits, absent.ts; choose another path"
            );
            let text = [&entry[..], &["-o", output]].concat();
            failed(
                &tsparse(&fixture, &text),
                &format!("usage.output: {message}\n  why: {NEW_FILES}\n"),
                &text,
            );
            let json = [&entry[..], &["--json", "-o", output]].concat();
            document(
                &tsparse(&fixture, &json),
                json!({
                    "operation": "edit",
                    "outcome": "error",
                    "effect": "none",
                    "error": {
                        "category": "usage",
                        "code": "usage.output",
                        "message": message,
                        "why": NEW_FILES,
                    },
                }),
                &json,
            );
            assert!(!fixture.path("absent.ts").exists(), "{json:?}");
        }
    }
    let plan = [&file[..], &["--plan", "absent.ts"]].concat();
    failed(
        &tsparse(&fixture, &plan),
        &format!(
            "usage.output: --plan absent.ts names a file the command reads or edits, absent.ts; choose another path\n  why: {NEW_FILES}\n"
        ),
        &plan,
    );
    assert!(!fixture.path("absent.ts").exists());
    // The service was never contacted.
    assert!(!fixture.path("services").exists());
}
