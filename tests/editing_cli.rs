//! The editing command contract: `--replace-body`, `--insert-after`,
//! `--edit-batch`, and `--apply`; request validity; text and JSON forms;
//! plans; output channels and exit codes; and file, direct, and service
//! entry agreeing. Which edits are valid is covered by `editing.rs`.
mod support;

use serde_json::{Value, json};
use std::io::Write as _;
use std::process::{Command, Output, Stdio};
use support::service::Service;
use support::{EDGE_CASES, Fixture};

const HOST: &str = "\
export function start() {
  return 1;
}
export function start2() {}
const shared = 1;
";

fn fixture() -> Fixture {
    Fixture::new(&[
        ("host.ts", HOST),
        ("body.ts", "{\n  return 2;\n}\n"),
        ("bad.ts", "{\n  return (2;\n}\n"),
        ("statement.ts", "export const added = 3;\n"),
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

fn json(output: &Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The message of a refused output path (`usage.output`), exit 2: with
/// `--json` one document on stdout and nothing on stderr, and in text
/// `usage.output: message` on stderr and nothing on stdout.
fn refusal(output: &Output, json: bool) -> String {
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    if json {
        let document = self::json(output, 2);
        assert_eq!(document["outcome"], "error", "{document}");
        assert_eq!(document["error"]["code"], "usage.output", "{document}");
        document["error"]["message"].as_str().unwrap().to_string()
    } else {
        assert!(output.stdout.is_empty(), "{output:?}");
        let stderr = text(&output.stderr);
        stderr
            .strip_prefix("usage.output: ")
            .and_then(|message| message.strip_suffix('\n'))
            .unwrap_or_else(|| panic!("{stderr}"))
            .to_string()
    }
}

fn host(fixture: &Fixture) -> String {
    std::fs::read_to_string(fixture.path("host.ts")).unwrap()
}

const REPLACED: &str = "\
export function start() {
  return 2;
}
export function start2() {}
const shared = 1;
";

#[test]
fn a_preview_shows_the_site_the_change_and_its_diff_and_writes_nothing() {
    let fixture = fixture();
    let output = tsparse(
        &fixture,
        &[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "body.ts",
            "host.ts",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty());
    let file = fixture.path("host.ts");
    let file = file.to_str().unwrap();
    assert_eq!(
        text(&output.stdout),
        format!(
            "{file}:1:17: replace_body function start (start): body 1:25-3:2\n\
             validated: syntax and edit boundaries; not type-checked\n\
             --- {file}\n\
             +++ {file}\n\
             @@ -1,5 +1,5 @@\n \
             export function start() {{\n\
             -  return 1;\n\
             +  return 2;\n \
             }}\n \
             export function start2() {{}}\n \
             const shared = 1;\n\
             preview only: save a plan with --plan PLAN to apply it\n"
        )
    );
    assert_eq!(host(&fixture), HOST);
}

#[test]
fn a_saved_plan_applies_exactly_the_preview() {
    let fixture = fixture();
    let output = tsparse(
        &fixture,
        &[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "body.ts",
            "host.ts",
            "--plan",
            "change.json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        text(&output.stdout)
            .ends_with("plan: change.json; apply it with: tsparse --apply change.json\n")
    );
    assert_eq!(host(&fixture), HOST);
    // The fragment may change after preview; the plan embeds it.
    std::fs::write(fixture.path("body.ts"), "{ return 99; }").unwrap();
    let output = tsparse(&fixture, &["--apply", "change.json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!("applied: {} changed\n", fixture.path("host.ts").display())
    );
    assert_eq!(host(&fixture), REPLACED);
    // Applying again finds the file changed.
    let output = tsparse(&fixture, &["--apply", "change.json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("changed since the batch was prepared"),
        "{stderr}"
    );
    assert!(stderr.ends_with("no file was changed\n"), "{stderr}");
}

#[test]
fn json_documents_name_the_outcome_and_the_effect() {
    let fixture = fixture();
    let document = json(
        &tsparse(
            &fixture,
            &[
                "--insert-after",
                "--name",
                "shared",
                "--from",
                "statement.ts",
                "host.ts",
                "--json",
                "--plan",
                "plan.json",
            ],
        ),
        0,
    );
    assert_eq!(document["operation"], "edit");
    assert_eq!(document["outcome"], "preview");
    assert_eq!(document["effect"], "none");
    assert_eq!(document["validation"], "syntax_and_edit_boundaries");
    assert_eq!(document["plan"], "plan.json");
    let file = &document["files"][0];
    assert_eq!(file["changed"], true);
    assert_eq!(file["edits"][0]["operation"], "insert_after");
    assert_eq!(file["edits"][0]["site"]["label"], "shared");
    assert_eq!(
        file["edits"][0]["changes"][0]["text"],
        "\nexport const added = 3;"
    );
    assert!(
        file["diff"]
            .as_str()
            .unwrap()
            .contains("+export const added = 3;\n")
    );
    let document = json(&tsparse(&fixture, &["--apply", "plan.json", "--json"]), 0);
    assert_eq!(document["operation"], "apply");
    assert_eq!(document["outcome"], "applied");
    assert_eq!(document["effect"], "applied");
    assert_eq!(document["files"][0]["state"], "replaced");
    assert_eq!(host(&fixture), format!("{HOST}export const added = 3;\n"));
}

#[test]
fn missing_and_ambiguous_selections_and_rejections_exit_1_and_change_nothing() {
    let fixture = fixture();
    let output = tsparse(
        &fixture,
        &[
            "--replace-body",
            "--name",
            "missing",
            "--from",
            "body.ts",
            "host.ts",
        ],
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(text(&output.stderr).starts_with("edit 1: "), "{output:?}");
    assert!(text(&output.stderr).contains("no declaration matches `missing`"));
    let document = json(
        &tsparse(
            &fixture,
            &[
                "--replace-body",
                "--name",
                "missing",
                "--from",
                "body.ts",
                "host.ts",
                "--json",
            ],
        ),
        1,
    );
    assert_eq!(
        (
            &document["operation"],
            &document["outcome"],
            &document["effect"],
            &document["edit"]
        ),
        (
            &"edit".into(),
            &"not_found".into(),
            &"none".into(),
            &0.into()
        )
    );
    let output = tsparse(
        &fixture,
        &[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "bad.ts",
            "host.ts",
        ],
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("edit 1: the proposed file does not parse"),
        "{stderr}"
    );
    assert!(stderr.contains("proposed line 2, column"), "{stderr}");
    let document = json(
        &tsparse(
            &fixture,
            &[
                "--replace-body",
                "--name",
                "shared",
                "--from",
                "body.ts",
                "host.ts",
                "--json",
            ],
        ),
        1,
    );
    assert_eq!(document["outcome"], "rejected");
    assert_eq!(document["error"]["kind"], "UnsupportedSite");
    assert_eq!(host(&fixture), HOST);
}

#[test]
fn invalid_requests_are_usage_errors() {
    let fixture = fixture();
    for (args, message) in [
        (
            &["--replace-body", "--name", "start", "host.ts"][..],
            "requires --from",
        ),
        (
            &["--replace-body", "--from", "body.ts", "host.ts"],
            "exactly one of --at or --name",
        ),
        (
            &["--replace-body", "--name", "start", "--from", "body.ts"],
            "requires a FILE",
        ),
        (
            &[
                "--replace-body",
                "--insert-after",
                "--name",
                "x",
                "--from",
                "body.ts",
                "host.ts",
            ],
            "Choose one operation: --replace-body, --insert-after",
        ),
        (
            &["--apply", "p.json", "--project", "tsconfig.json"],
            "takes only the plan",
        ),
        (
            &["--show", "--name", "x", "host.ts", "--plan", "p.json"],
            "--plan applies to: --replace-body, --insert-after, --edit-batch",
        ),
        (
            &[
                "--replace-body",
                "--name",
                "start",
                "--from",
                "body.ts",
                "host.ts",
                "--direct",
            ],
            "--direct applies",
        ),
        (
            &["--edit-batch", "b.json", "--name", "x"],
            "names every edit",
        ),
        (
            &[
                "--replace-body",
                "--name",
                "s",
                "--from",
                "a",
                "--from",
                "b",
                "host.ts",
            ],
            "at most once",
        ),
        (&["--apply", "p.json", "--pretty"], "only with --json"),
    ] {
        let output = tsparse(&fixture, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(
            text(&output.stderr).contains(message),
            "{args:?}: {}",
            text(&output.stderr)
        );
    }
}

#[test]
fn a_result_that_cannot_be_written_after_applying_still_reports_the_effect() {
    let fixture = fixture();
    let output = tsparse(
        &fixture,
        &[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "body.ts",
            "host.ts",
            "--plan",
            "p.json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    // A result in a directory that does not exist cannot be written.
    let output = tsparse(&fixture, &["--apply", "p.json", "-o", "out/result.json"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    // The write failure, then the effect, on stderr.
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with("write.output: the result could not be written: out/result.json: "),
        "{stderr}"
    );
    assert!(
        stderr.ends_with(&format!(
            "\napplied: {} changed\n",
            fixture.path("host.ts").display()
        )),
        "{stderr}"
    );
    assert_eq!(host(&fixture), REPLACED);
}

#[test]
fn plans_that_cannot_be_used_name_why() {
    let fixture = fixture();
    std::fs::write(fixture.path("bad.json"), "{ nope").unwrap();
    let output = tsparse(&fixture, &["--apply", "bad.json"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(text(&output.stderr).contains("not an edit plan"));
    let output = tsparse(
        &fixture,
        &[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "body.ts",
            "host.ts",
            "--plan",
            "p.json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let plan = std::fs::read_to_string(fixture.path("p.json")).unwrap();
    let mut plan: Value = serde_json::from_str(&plan).unwrap();
    plan["build"] = "another".into();
    std::fs::write(fixture.path("p.json"), plan.to_string()).unwrap();
    let document = json(&tsparse(&fixture, &["--apply", "p.json", "--json"]), 1);
    assert_eq!(document["error"]["kind"], "Incompatible");
    assert_eq!(document["effect"], "unchanged");
    assert_eq!(host(&fixture), HOST);
}

/// A plan that does not decode as a plan names the help that says what a
/// plan is, in text and as a document, and with `-o`; a plan that cannot
/// be read, or that another build saved, names no help, which could not
/// fix it.
#[test]
fn a_plan_that_is_not_a_plan_names_the_help_that_says_what_one_is() {
    let fixture = fixture();
    std::fs::write(fixture.path("edits.json"), r#"{"edits": []}"#).unwrap();
    // The cause the library reports for a document that is not a plan.
    let output = tsparse(&fixture, &["--apply", "edits.json", "--json"]);
    let document = json(&output, 2);
    let message = document["error"]["message"].as_str().unwrap().to_string();
    assert!(message.starts_with("not an edit plan: "), "{message}");
    assert_eq!(
        document,
        json!({
            "operation": "apply",
            "outcome": "error",
            "effect": "unchanged",
            "error": {
                "kind": "Malformed",
                "message": message,
                "path": "edits.json",
                "next": {"argv": ["tsparse", "--apply", "PLAN", "--help"]},
            },
        })
    );
    let output = tsparse(&fixture, &["--apply", "edits.json"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert_eq!(
        text(&output.stderr),
        format!("edits.json: {message}\n  next: tsparse --apply PLAN --help\n")
    );
    for format in [&[][..], &["--json"]] {
        let args = [&["--apply", "edits.json", "-o", "out.json"][..], format].concat();
        let output = tsparse(&fixture, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        assert_eq!(
            text(&output.stderr),
            format!(
                "edits.json: {message}\nthe result is not written to out.json: the plan's file names cannot be read, so it could be one of them\n  next: tsparse --apply PLAN --help\n"
            ),
            "{args:?}"
        );
        assert!(!fixture.path("out.json").exists(), "{args:?}");
    }
    // Following it: the help says a plan is what --plan saved.
    let output = tsparse(&fixture, &["--apply", "PLAN", "--help"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        text(&output.stdout).contains("Writes exactly the bytes --plan saved"),
        "{output:?}"
    );
    // Help could not fix a plan that cannot be read.
    let output = tsparse(&fixture, &["--apply", "missing.json"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(
        text(&output.stderr),
        format!("missing.json: {}\n", std::io::Error::from_raw_os_error(2))
    );
}

#[test]
fn a_batch_file_prepares_every_edit_against_the_originals() {
    let fixture = fixture();
    std::fs::write(
        fixture.path("edits.json"),
        r#"{ "edits": [
            { "operation": "replace_body", "name": "start", "file": "host.ts", "from": "body.ts" },
            { "operation": "insert_after", "at": "host.ts:5:7", "fragment": "const more = 2;" },
            { "operation": "replace_body", "at": "4:17", "file": "host.ts", "fragment": "{ go(); }" }
        ] }"#,
    )
    .unwrap();
    let output = tsparse(
        &fixture,
        &["--edit-batch", "edits.json", "--plan", "p.json"],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let output = tsparse(&fixture, &["--apply", "p.json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        host(&fixture),
        "export function start() {\n  return 2;\n}\nexport function start2() { go(); }\nconst shared = 1;\nconst more = 2;\n"
    );
    std::fs::write(
        fixture.path("bad-batch.json"),
        r#"{ "edits": [ { "operation": "replace_body", "name": "x", "at": "1:1", "fragment": "{}" } ] }"#,
    )
    .unwrap();
    let output = tsparse(&fixture, &["--edit-batch", "bad-batch.json"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(
        text(&output.stderr),
        "input.batch: bad-batch.json: 1 edit is invalid\n  edits[0]: give exactly one of name and at\n  edits[0].at: LINE:COLUMN needs file, or give FILE:LINE:COLUMN\n  next: tsparse --edit-batch EDITS --help\n"
    );
}

#[test]
fn a_fragment_can_come_from_stdin() {
    let fixture = fixture();
    let mut child = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args([
            "--replace-body",
            "--name",
            "start",
            "--from",
            "-",
            "host.ts",
            "--plan",
            "p.json",
        ])
        .current_dir(&fixture.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\n  return 2;\n}\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        tsparse(&fixture, &["--apply", "p.json"]).status.code(),
        Some(0)
    );
    assert_eq!(host(&fixture), REPLACED);
}

#[cfg(unix)]
#[test]
fn project_edits_agree_through_the_service_and_directly() {
    let service = Service::new(&[("src/host.ts", HOST), ("body.ts", "{\n  return 2;\n}\n")]);
    let run = |extra: &[&str]| {
        let mut args = vec![
            "--replace-body",
            "--name",
            "start",
            "--from",
            "body.ts",
            "--json",
        ];
        args.extend_from_slice(extra);
        service.run(&args)
    };
    let direct = json(&run(&["--direct"]), 0);
    let through_service = json(&run(&[]), 0);
    assert_eq!(direct, through_service);
    assert_eq!(
        direct["project"]
            .as_str()
            .map(|p| p.ends_with("tsconfig.json")),
        Some(true)
    );
    assert_eq!(direct["files"][0]["file"], "src/host.ts");
    // Both name the saved plan and the command that applies it alike.
    let direct = json(&run(&["--direct", "--plan", "p.json"]), 0);
    std::fs::remove_file(service.fixture.path("p.json")).unwrap();
    let through_service = json(&run(&["--plan", "p.json"]), 0);
    assert_eq!(direct, through_service);
    assert_eq!(
        through_service["next"],
        json!({"argv": ["tsparse", "--apply", "p.json"]})
    );
    // Through the service, a plan applies locally.
    let output = service
        .command()
        .args(["--apply", "p.json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        std::fs::read_to_string(service.fixture.path("src/host.ts")).unwrap(),
        REPLACED
    );
}

#[cfg(unix)]
#[test]
fn an_analysis_failure_document_names_its_effect() {
    let fixture = Fixture::new(&[("broken.ts", "const = 1;\n"), ("body.ts", "{}")]);
    let document = json(
        &tsparse(
            &fixture,
            &[
                "--replace-body",
                "--name",
                "f",
                "--from",
                "body.ts",
                "broken.ts",
                "--json",
            ],
        ),
        1,
    );
    assert_eq!(document["outcome"], "error");
    assert_eq!(document["effect"], "none");
    assert_eq!(document["error"]["kind"], "Syntax");
}

/// A target that is not a regular file, is not UTF-8, has another hard
/// link, or has macOS flags is refused with its own message and exit code.
#[cfg(unix)]
#[test]
fn a_target_that_cannot_be_replaced_is_refused() {
    let fixture = fixture();
    let status = Command::new("mkfifo")
        .arg(fixture.path("pipe.ts"))
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::write(fixture.path("latin.ts"), b"function start() {}\n\xff\n").unwrap();
    std::fs::write(fixture.path("linked.ts"), "function start() {}\n").unwrap();
    std::fs::hard_link(fixture.path("linked.ts"), fixture.path("other.ts")).unwrap();
    let refused = vec![
        ("pipe.ts", 1, "Unsupported", "not a regular file"),
        ("latin.ts", 2, "Io", "stream did not contain valid UTF-8"),
        (
            "linked.ts",
            1,
            "Unsupported",
            "the file has 2 hard links; replacing it would separate them",
        ),
    ];
    #[cfg(target_os = "macos")]
    let refused = {
        let mut refused = refused;
        std::fs::write(fixture.path("flagged.ts"), "function start() {}\n").unwrap();
        let status = Command::new("chflags")
            .arg("hidden")
            .arg(fixture.path("flagged.ts"))
            .status()
            .unwrap();
        assert!(status.success());
        refused.push((
            "flagged.ts",
            1,
            "Unsupported",
            "the file has flags (0x8000, such as hidden or uchg) that replacing it would drop",
        ));
        refused
    };
    for (file, code, kind, message) in refused {
        let output = tsparse(
            &fixture,
            &[
                "--replace-body",
                "--name",
                "start",
                "--from",
                "body.ts",
                file,
                "--json",
            ],
        );
        let document = json(&output, code);
        assert_eq!(document["outcome"], "error", "{file}: {document}");
        assert_eq!(document["error"]["kind"], kind, "{file}: {document}");
        assert_eq!(document["error"]["message"], message, "{file}: {document}");
    }
}

/// A target that gains another hard link, other permissions, or macOS flags
/// after planning is stale when applying, exit 1, with its reason, and is
/// left unchanged.
#[cfg(unix)]
#[test]
fn a_target_changed_after_planning_is_refused_with_its_reason() {
    use std::os::unix::fs::PermissionsExt;
    let changed = "it was replaced, or its permissions or flags changed";
    type Change = fn(&Fixture);
    let changes: Vec<(&str, Change, &str)> = vec![
        (
            "hard link",
            |f| std::fs::hard_link(f.path("host.ts"), f.path("other.ts")).unwrap(),
            "it now has 2 hard links",
        ),
        (
            "permissions",
            |f| {
                std::fs::set_permissions(f.path("host.ts"), std::fs::Permissions::from_mode(0o600))
                    .unwrap()
            },
            changed,
        ),
    ];
    #[cfg(target_os = "macos")]
    let changes = {
        let mut changes = changes;
        changes.push((
            "flags",
            |f| {
                let status = Command::new("chflags")
                    .arg("hidden")
                    .arg(f.path("host.ts"))
                    .status()
                    .unwrap();
                assert!(status.success());
            },
            changed,
        ));
        changes
    };
    for (name, change, reason) in changes {
        let fixture = fixture();
        save_plan(&fixture, "host.ts", "p.json");
        change(&fixture);
        let document = json(&tsparse(&fixture, &["--apply", "p.json", "--json"]), 1);
        assert_eq!(document["effect"], "unchanged", "{name}: {document}");
        assert_eq!(document["error"]["kind"], "Stale", "{name}: {document}");
        assert_eq!(
            document["error"]["message"],
            format!("the file changed since the batch was prepared: {reason}; prepare it again"),
            "{name}: {document}"
        );
        assert_eq!(host(&fixture), HOST, "{name}");
    }
}

#[test]
fn results_and_plans_are_new_files_so_no_outcome_replaces_a_file() {
    let fixture = fixture();
    std::fs::write(fixture.path("previous.json"), "{}").unwrap();
    std::fs::write(fixture.path("bad-plan.json"), "not a plan").unwrap();
    std::fs::hard_link(fixture.path("body.ts"), fixture.path("linked.json")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("p.json", fixture.path("to-plan.json")).unwrap();
        std::os::unix::fs::symlink("new.ts", fixture.path("to-new.json")).unwrap();
    }
    let preview = |extra: &[&str]| {
        let mut args = vec![
            "--replace-body",
            "--name",
            "start",
            "--from",
            "body.ts",
            "host.ts",
            "--json",
        ];
        args.extend_from_slice(extra);
        tsparse(&fixture, &args)
    };
    let mut outputs = vec![
        // A target, a fragment, another name of the fragment, an earlier
        // result, and a plan that does not load.
        preview(&["-o", "host.ts"]),
        preview(&["-o", "body.ts"]),
        preview(&["-o", "linked.json"]),
        preview(&["-o", "previous.json"]),
        preview(&["--plan", "./host.ts"]),
        preview(&["--plan", "previous.json"]),
        tsparse(
            &fixture,
            &[
                "--replace-body",
                "--name",
                "missing",
                "--from",
                "body.ts",
                "host.ts",
                "--json",
                "-o",
                "host.ts",
            ],
        ),
        tsparse(
            &fixture,
            &["--apply", "bad-plan.json", "--json", "-o", "bad-plan.json"],
        ),
        tsparse(
            &fixture,
            &["--apply", "bad-plan.json", "--json", "-o", "host.ts"],
        ),
        // One new file named twice.
        preview(&["--plan", "x.json", "-o", "x.json"]),
    ];
    if fixture.root.join("HOST.TS").exists() {
        outputs.push(preview(&["--plan", "y.json", "-o", "Y.JSON"]));
    }
    #[cfg(unix)]
    {
        // Links whose files do not exist yet.
        outputs.push(preview(&["--plan", "p.json", "-o", "to-plan.json"]));
        outputs.push(preview(&["-o", "to-new.json"]));
    }
    for output in outputs {
        let message = refusal(&output, true);
        assert!(
            message.contains("already exists") || message.contains("name one file"),
            "{message}"
        );
    }
    assert_eq!(host(&fixture), HOST);
    assert_eq!(
        std::fs::read_to_string(fixture.path("body.ts")).unwrap(),
        "{\n  return 2;\n}\n"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.path("previous.json")).unwrap(),
        "{}"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.path("bad-plan.json")).unwrap(),
        "not a plan"
    );
    for absent in ["x.json", "y.json", "p.json", "new.ts"] {
        assert!(!fixture.path(absent).exists(), "{absent}");
    }
}

#[cfg(unix)]
#[test]
fn a_project_edit_never_writes_over_a_project_file() {
    let service = Service::new(&[("src/host.ts", HOST), ("bad.body", "{\n  return (2;\n}\n")]);
    for direct in [true, false] {
        for output_path in ["src/host.ts", "tsconfig.json"] {
            let mut args = vec![
                "--replace-body",
                "--name",
                "start",
                "--from",
                "bad.body",
                "--json",
                "-o",
                output_path,
            ];
            if direct {
                args.push("--direct");
            }
            let output = service.run(&args);
            assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        }
    }
    assert_eq!(
        std::fs::read_to_string(service.fixture.path("src/host.ts")).unwrap(),
        HOST
    );
}

/// Arguments as owned strings, so a table of commands can hold them.
#[cfg(unix)]
fn args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

#[cfg(unix)]
fn run(fixture: &Fixture, args: &[String]) -> Output {
    tsparse(
        fixture,
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

/// Whether the file system takes `a` and `b` for one name in the fixture.
#[cfg(unix)]
fn one_name(fixture: &Fixture, a: &str, b: &str) -> bool {
    std::fs::write(fixture.path(a), "").unwrap();
    let same = fixture.path(b).exists();
    std::fs::remove_file(fixture.path(a)).unwrap();
    same
}

#[cfg(unix)]
#[test]
fn an_output_never_creates_a_file_the_request_or_plan_names() {
    let fixture = fixture();
    std::fs::write(fixture.path("b.ts"), "{\n  return 2;\n}\n").unwrap();
    let plan = |file: &str, plan: &str| {
        let output = run(
            &fixture,
            &args(&[
                "--replace-body",
                "--name",
                "start",
                "--from",
                "b.ts",
                file,
                "--plan",
                plan,
            ]),
        );
        assert_eq!(output.status.code(), Some(0), "{output:?}");
    };
    plan("host.ts", "change.json");
    std::fs::write(fixture.path("same.ts"), "{\n  return 1;\n}").unwrap();
    let output = run(
        &fixture,
        &args(&[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "same.ts",
            "host.ts",
            "--plan",
            "same.json",
        ]),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    // Plans that decode but cannot be loaded still name their files: one
    // another build saved, and one whose recorded text replay does not give.
    // Plans that do not decode name files no one can know: a field of the
    // wrong type or shape, a later unreadable record, a field given twice,
    // and text cut short.
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.path("change.json")).unwrap())
            .unwrap();
    let variant = |name: &str, change: &dyn Fn(&mut Value)| {
        let mut plan = saved.clone();
        change(&mut plan);
        std::fs::write(fixture.path(name), plan.to_string()).unwrap();
    };
    variant("other-build.json", &|plan| plan["build"] = "another".into());
    variant("altered.json", &|plan| {
        plan["files"][0]["proposed"] = "altered".into()
    });
    variant("null-aliases.json", &|plan| {
        plan["files"][0]["aliases"] = Value::Null
    });
    variant("bad-record.json", &|plan| {
        plan["files"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({ "path": 3 }))
    });
    variant("files-object.json", &|plan| {
        plan["files"] = plan["files"][0].clone();
    });
    let text_of = std::fs::read_to_string(fixture.path("change.json")).unwrap();
    let body = text_of.trim_end().strip_suffix('}').unwrap();
    std::fs::write(
        fixture.path("files-twice.json"),
        format!("{body}, \"files\": [] }}"),
    )
    .unwrap();
    std::fs::write(
        fixture.path("truncated.json"),
        &text_of[..text_of.len() / 2],
    )
    .unwrap();
    // A file named by a first name, a second name, and the file they link
    // to, each of which the plan records.
    std::fs::write(fixture.path("real.ts"), HOST).unwrap();
    std::os::unix::fs::symlink("real.ts", fixture.path("first.ts")).unwrap();
    std::os::unix::fs::symlink("real.ts", fixture.path("second.ts")).unwrap();
    std::fs::write(
        fixture.path("names.json"),
        r#"{ "edits": [
            { "operation": "replace_body", "name": "start", "file": "first.ts", "fragment": "{}" },
            { "operation": "replace_body", "name": "start2", "file": "second.ts", "fragment": "{ go(); }" }
        ] }"#,
    )
    .unwrap();
    let output = run(
        &fixture,
        &args(&["--edit-batch", "names.json", "--plan", "names-plan.json"]),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    // A project's plan records its configuration.
    std::fs::create_dir(fixture.path("proj")).unwrap();
    std::fs::write(fixture.path("proj/p.ts"), HOST).unwrap();
    std::fs::write(
        fixture.path("proj/tsconfig.json"),
        r#"{ "files": ["p.ts"] }"#,
    )
    .unwrap();
    let output = run(
        &fixture,
        &args(&[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "b.ts",
            "--project",
            "proj/tsconfig.json",
            "--direct",
            "--plan",
            "proj-plan.json",
        ]),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    // Names the file system may take for one: another case, the other
    // Unicode normalization, and a full case folding.
    let spellings = [
        ("host.ts", "HOST.TS", "change.json"),
        ("caf\u{e9}.ts", "cafe\u{301}.ts", "cafe.json"),
        ("stra\u{df}e.ts", "strasse.ts", "strasse.json"),
    ];
    for (recorded, _, saved) in &spellings[1..] {
        std::fs::write(fixture.path(recorded), HOST).unwrap();
        plan(recorded, saved);
    }
    std::fs::write(
        fixture.path("gone-batch.json"),
        r#"{ "edits": [{ "operation": "replace_body", "name": "start", "file": "gone.ts", "fragment": "{}" }] }"#,
    )
    .unwrap();
    for removed in [
        "host.ts",
        "real.ts",
        "first.ts",
        "second.ts",
        "proj/tsconfig.json",
        "caf\u{e9}.ts",
        "stra\u{df}e.ts",
    ] {
        std::fs::remove_file(fixture.path(removed)).unwrap();
    }
    std::os::unix::fs::symlink("host.ts", fixture.path("link.ts")).unwrap();
    std::os::unix::fs::symlink(&fixture.root, fixture.path("here")).unwrap();
    let absolute = fixture.path("host.ts");
    let preview = |file: &str, output: &str| {
        args(&[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "b.ts",
            file,
            "--json",
            "-o",
            output,
        ])
    };
    let apply = |plan: &str, output: &str| args(&["--apply", plan, "--json", "-o", output]);
    let refused = vec![
        // A plan's deleted target under each spelling, whether the plan
        // changes it or not.
        (apply("change.json", "host.ts"), "host.ts"),
        (apply("change.json", "./host.ts"), "host.ts"),
        (apply("change.json", absolute.to_str().unwrap()), "host.ts"),
        (apply("change.json", "here/host.ts"), "host.ts"),
        (apply("same.json", "host.ts"), "host.ts"),
        (args(&["--apply", "same.json", "-o", "host.ts"]), "host.ts"),
        // Plans that cannot be loaded.
        (apply("other-build.json", "host.ts"), "host.ts"),
        (apply("altered.json", "host.ts"), "host.ts"),
        // Each name a plan records, and its project's configuration.
        (apply("names-plan.json", "first.ts"), "first.ts"),
        (apply("names-plan.json", "second.ts"), "second.ts"),
        (apply("names-plan.json", "real.ts"), "real.ts"),
        (
            apply("proj-plan.json", "proj/tsconfig.json"),
            "proj/tsconfig.json",
        ),
        // The plan itself, which does not exist.
        (apply("missing.json", "missing.json"), "missing.json"),
        // A deleted file a preview names: as FILE, through a link, in a
        // batch, and as the plan it would save; and a project's
        // configuration.
        (preview("host.ts", "host.ts"), "host.ts"),
        (preview("link.ts", "host.ts"), "host.ts"),
        (
            args(&["--edit-batch", "gone-batch.json", "--json", "-o", "gone.ts"]),
            "gone.ts",
        ),
        (
            args(&[
                "--replace-body",
                "--name",
                "start",
                "--from",
                "b.ts",
                "host.ts",
                "--plan",
                "host.ts",
            ]),
            "host.ts",
        ),
        (
            args(&[
                "--replace-body",
                "--name",
                "start",
                "--from",
                "b.ts",
                "--project",
                "absent.json",
                "--direct",
                "--json",
                "-o",
                "absent.json",
            ]),
            "absent.json",
        ),
    ];
    for (args, absent) in refused {
        let output = run(&fixture, &args);
        let message = refusal(&output, args.iter().any(|arg| arg == "--json"));
        assert!(
            message.contains("names a file the command reads or edits"),
            "{args:?}: {message}"
        );
        assert!(!fixture.path(absent).exists(), "{args:?}");
    }
    // Another spelling is refused exactly when the file system takes it for
    // the recorded name; otherwise it is a new result like any other.
    for (recorded, spelled, saved) in spellings {
        let same = one_name(&fixture, spelled, recorded);
        let output = run(&fixture, &apply(saved, spelled));
        assert!(
            !fixture.path(recorded).exists(),
            "{recorded} {spelled} {output:?}"
        );
        if same {
            assert_eq!(output.status.code(), Some(2), "{spelled}: {output:?}");
            assert!(!fixture.path(spelled).exists());
        } else {
            assert_eq!(output.status.code(), Some(1), "{spelled}: {output:?}");
            assert!(fixture.path(spelled).exists());
        }
    }
    // A plan that does not decode writes its failure to no output.
    for unknown in [
        "null-aliases.json",
        "bad-record.json",
        "files-object.json",
        "files-twice.json",
        "truncated.json",
    ] {
        let output = run(&fixture, &apply(unknown, "host.ts"));
        assert_eq!(output.status.code(), Some(2), "{unknown}: {output:?}");
        let stderr = text(&output.stderr);
        assert!(
            stderr.contains(&format!("{unknown}: not an edit plan")),
            "{stderr}"
        );
        assert!(
            stderr.contains("the plan's file names cannot be read"),
            "{stderr}"
        );
        assert!(!absolute.exists(), "{unknown}");
    }
    // A new name is still a result.
    let output = run(&fixture, &apply("change.json", "result.json"));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(fixture.path("result.json").exists());
    assert!(!absolute.exists());
}

#[test]
fn a_plan_that_cannot_be_loaded_says_why_when_its_result_cannot_be_written() {
    let fixture = fixture();
    let output = tsparse(
        &fixture,
        &[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "body.ts",
            "host.ts",
            "--plan",
            "p.json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let mut plan: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.path("p.json")).unwrap()).unwrap();
    plan["build"] = "another".into();
    std::fs::write(fixture.path("p.json"), plan.to_string()).unwrap();
    let output = tsparse(
        &fixture,
        &["--apply", "p.json", "--json", "-o", "missing/out.json"],
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = text(&output.stderr);
    assert!(stderr.contains("missing/out.json: "), "{stderr}");
    assert!(
        stderr.contains("p.json: another build of tsparse saved this plan"),
        "{stderr}"
    );
    assert_eq!(host(&fixture), HOST);
}

#[test]
fn a_preview_whose_result_cannot_be_written_says_its_plan_was_saved() {
    let fixture = fixture();
    let output = tsparse(
        &fixture,
        &[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "body.ts",
            "host.ts",
            "--plan",
            "p.json",
            "--json",
            "-o",
            "missing/out.json",
        ],
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(fixture.path("p.json").exists());
    let stderr = text(&output.stderr);
    assert_eq!(
        stderr,
        "write.output: missing/out.json: No such file or directory (os error 2); the plan was saved to p.json\n  next: tsparse --apply p.json\n"
    );
}

/// Plan names a shell reads specially, each with the word that names it
/// exactly: a space, a quote, an expansion, a command separator, a
/// character outside ASCII, and a leading `-`, which reads as an option.
const SHELL_NAMES: [(&str, &str); 6] = [
    ("my plan.json", "'my plan.json'"),
    ("it's.json", r"'it'\''s.json'"),
    ("$plan.json", "'$plan.json'"),
    ("a;b.json", "'a;b.json'"),
    ("é.json", "'é.json'"),
    ("-plan.json", "'-plan.json'"),
];

/// `PATH` with this build's `tsparse` first, so a printed command runs it.
fn path_with_build() -> std::ffi::OsString {
    let build = std::path::Path::new(env!("CARGO_BIN_EXE_tsparse"))
        .parent()
        .unwrap()
        .to_path_buf();
    let path = std::env::var_os("PATH").unwrap();
    std::env::join_paths(std::iter::once(build).chain(std::env::split_paths(&path))).unwrap()
}

/// `command` as a POSIX shell runs it in the fixture.
fn shell(fixture: &Fixture, command: &str) -> Output {
    Command::new("sh")
        .args(["-c", command])
        .current_dir(&fixture.root)
        .env("PATH", path_with_build())
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// Assert that `output` applied the plan replacing `start`'s body.
fn applied(fixture: &Fixture, output: &Output) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!("applied: {} changed\n", fixture.path("host.ts").display())
    );
    assert_eq!(host(fixture), REPLACED);
}

/// The command on the last line of `text`, which must be `line`: after
/// `apply it with: ` in a preview, or `  next: ` in a failure.
fn printed_command<'a>(text: &'a str, line: &str) -> &'a str {
    assert!(text.ends_with('\n'), "{text}");
    let last = text.lines().last();
    assert_eq!(last, Some(line), "{text}");
    let last = last.unwrap();
    match last.split_once("apply it with: ") {
        Some((_, command)) => command,
        None => last.strip_prefix("  next: ").unwrap(),
    }
}

/// Run a document's `next.argv` exactly, with this build first on `PATH`.
fn follow_next(fixture: &Fixture, document: &Value) -> Output {
    let argv: Vec<&str> = document["next"]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| arg.as_str().unwrap())
        .collect();
    Command::new(argv[0])
        .args(&argv[1..])
        .current_dir(&fixture.root)
        .env("PATH", path_with_build())
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[cfg(unix)]
#[test]
fn every_printed_apply_command_applies_its_plan_as_printed() {
    for (plan, word) in SHELL_NAMES {
        let command = format!("tsparse --apply {word}");
        let prepare = |fixture: &Fixture, extra: &[&str]| {
            let mut args = vec![
                "--replace-body",
                "--name",
                "start",
                "--from",
                "body.ts",
                "host.ts",
                "--plan",
                plan,
            ];
            args.extend_from_slice(extra);
            tsparse(fixture, &args)
        };

        // The text preview's last line, on stdout and in `-o`, run by a
        // shell.
        for extra in [&[][..], &["-o", "preview.txt"]] {
            let preview = fixture();
            let output = prepare(&preview, extra);
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
            let result = match extra {
                [] => text(&output.stdout),
                _ => {
                    assert!(output.stdout.is_empty(), "{output:?}");
                    std::fs::read_to_string(preview.path("preview.txt")).unwrap()
                }
            };
            let printed =
                printed_command(&result, &format!("plan: {plan}; apply it with: {command}"));
            applied(&preview, &shell(&preview, printed));
        }

        // The JSON preview's `next`, on stdout and in `-o`, run as its
        // argument list.
        for extra in [&["--json"][..], &["--json", "-o", "preview.json"]] {
            let document = fixture();
            let output = prepare(&document, extra);
            let value: Value = match extra {
                [_] => json(&output, 0),
                _ => {
                    assert_eq!(output.status.code(), Some(0), "{output:?}");
                    assert!(output.stdout.is_empty() && output.stderr.is_empty());
                    let result = std::fs::read(document.path("preview.json")).unwrap();
                    serde_json::from_slice(&result).unwrap()
                }
            };
            assert_eq!(value["plan"], plan);
            assert_eq!(
                value["next"],
                json!({"argv": ["tsparse", "--apply", plan]}),
                "{value}"
            );
            applied(&document, &follow_next(&document, &value));
        }

        // A preview that cannot be written, as text or JSON, names the
        // saved plan's command.
        for extra in [
            &["-o", "missing/out.txt"][..],
            &["--json", "-o", "missing/out.json"],
        ] {
            let unwritten = fixture();
            let output = prepare(&unwritten, extra);
            assert_eq!(output.status.code(), Some(2), "{output:?}");
            assert!(output.stdout.is_empty());
            let stderr = text(&output.stderr);
            assert!(
                stderr.starts_with("write.output: missing/out.")
                    && stderr.contains(&format!("; the plan was saved to {plan}\n")),
                "{stderr}"
            );
            let printed = printed_command(&stderr, &format!("  next: {command}"));
            applied(&unwritten, &shell(&unwritten, printed));
        }
    }

    // A preview without a plan names no command.
    let fixture = fixture();
    let value = json(
        &tsparse(
            &fixture,
            &[
                "--replace-body",
                "--name",
                "start",
                "--from",
                "body.ts",
                "host.ts",
                "--json",
            ],
        ),
        0,
    );
    assert_eq!(value["plan"], Value::Null);
    assert_eq!(value.get("next"), None, "{value}");
}

/// A plan path that is not Unicode cannot be named exactly, so it is
/// refused before anything is prepared or saved, and never printed in a
/// command; the message alone shows its lossy form.
#[cfg(unix)]
#[test]
fn a_plan_path_that_is_not_unicode_is_a_usage_error_and_saves_nothing() {
    use std::os::unix::ffi::OsStrExt;
    let fixture = fixture();
    for format in [&[][..], &["--json"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args([
                "--replace-body",
                "--name",
                "start",
                "--from",
                "body.ts",
                "host.ts",
                "--plan",
            ])
            .arg(std::ffi::OsStr::from_bytes(b"\xff.json"))
            .args(format)
            .current_dir(&fixture.root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        assert_eq!(
            text(&output.stderr),
            "usage.not_unicode: Arguments must be Unicode: \u{FFFD}.json\n"
        );
    }
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 4);
    assert_eq!(host(&fixture), HOST);
}

/// In a directory whose new files cannot be removed, checking an output
/// leaves nothing behind when no check needs one: an output whose names all
/// exist is not created to check it, and a plan that does not decode
/// creates none.
#[cfg(target_os = "macos")]
#[test]
fn outputs_are_checked_in_a_directory_whose_new_files_cannot_be_removed() {
    let fixture = fixture();
    std::fs::write(fixture.path("broken.json"), "{broken").unwrap();
    std::fs::copy(fixture.path("host.ts"), fixture.path("other.ts")).unwrap();
    std::fs::remove_file(fixture.path("host.ts")).unwrap();
    let flags = |flag: &str| {
        assert!(
            Command::new("chflags")
                .arg(flag)
                .arg(&fixture.root)
                .status()
                .unwrap()
                .success()
        )
    };
    flags("uappnd");
    let broken = tsparse(
        &fixture,
        &["--apply", "broken.json", "--json", "-o", "host.ts"],
    );
    let preview = tsparse(
        &fixture,
        &[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "body.ts",
            "other.ts",
            "--json",
            "-o",
            "preview.json",
        ],
    );
    flags("nouappnd");
    assert_eq!(broken.status.code(), Some(2), "{broken:?}");
    assert!(text(&broken.stderr).contains("the plan's file names cannot be read"));
    assert!(!fixture.path("host.ts").exists());
    assert_eq!(preview.status.code(), Some(0), "{preview:?}");
    let preview: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.path("preview.json")).unwrap())
            .unwrap();
    assert_eq!(preview["outcome"], "preview");
}

/// Save a plan replacing `start`'s body in `file`.
fn save_plan(fixture: &Fixture, file: &str, plan: &str) {
    let output = tsparse(
        fixture,
        &[
            "--replace-body",
            "--name",
            "start",
            "--from",
            "body.ts",
            file,
            "--plan",
            plan,
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

/// In an append-only directory the output created to check it cannot be
/// removed, so it is reported as left there, with the name it matched.
#[cfg(target_os = "macos")]
#[test]
fn an_output_that_cannot_be_removed_after_checking_is_reported_as_left() {
    let fixture = fixture();
    save_plan(&fixture, "host.ts", "change.json");
    std::fs::remove_file(fixture.path("host.ts")).unwrap();
    let flags = |flag: &str| {
        assert!(
            Command::new("chflags")
                .arg(flag)
                .arg(&fixture.root)
                .status()
                .unwrap()
                .success()
        )
    };
    flags("uappnd");
    let distinct = tsparse(
        &fixture,
        &["--apply", "change.json", "--json", "-o", "out.json"],
    );
    let reserved = tsparse(
        &fixture,
        &["--apply", "change.json", "--json", "-o", "host.ts"],
    );
    flags("nouappnd");
    let message = refusal(&reserved, true);
    assert!(
        message.contains("is left there, empty")
            && message.contains(&format!("(it names {})", fixture.path("host.ts").display())),
        "{message}"
    );
    assert_eq!(std::fs::read(fixture.path("host.ts")).unwrap(), b"");
    let message = refusal(&distinct, true);
    assert!(
        message.contains("is left there, empty") && !message.contains("it names"),
        "{message}"
    );
    assert_eq!(std::fs::read(fixture.path("out.json")).unwrap(), b"");
}

/// A reserved name that cannot be looked up is a failure that names it, not
/// a name that does not exist.
#[cfg(unix)]
#[test]
fn a_reserved_name_that_cannot_be_looked_up_is_named() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = fixture();
    std::fs::create_dir(fixture.path("locked")).unwrap();
    std::fs::write(fixture.path("locked/a.ts"), HOST).unwrap();
    save_plan(&fixture, "locked/a.ts", "locked.json");
    let mode = |mode| {
        std::fs::set_permissions(
            fixture.path("locked"),
            std::fs::Permissions::from_mode(mode),
        )
        .unwrap()
    };
    mode(0o000);
    if std::fs::metadata(fixture.path("locked/a.ts")).is_ok() {
        mode(0o755);
        eprintln!(
            "SKIP a_reserved_name_that_cannot_be_looked_up_is_named: this user reads any directory"
        );
        return;
    }
    let locked = tsparse(
        &fixture,
        &["--apply", "locked.json", "--json", "-o", "result.json"],
    );
    mode(0o755);
    let message = refusal(&locked, true);
    assert!(
        message.contains(&format!(
            "cannot look up {}",
            fixture.path("locked/a.ts").display()
        )),
        "{message}"
    );
    assert!(!fixture.path("result.json").exists());
}

/// The statement-site design's sample, with its two fragments.
fn edge_cases() -> Fixture {
    Fixture::new(&[
        ("edge-cases.test.ts", EDGE_CASES),
        ("new-test.ts", "it(\"new test\", () => {});\n"),
        ("new-hook.ts", "afterEach(() => {});\n"),
        ("body.ts", "{\n  return 2;\n}\n"),
    ])
}

fn edge_cases_text(fixture: &Fixture) -> String {
    std::fs::read_to_string(fixture.path("edge-cases.test.ts")).unwrap()
}

/// `EDGE_CASES` with `line` inserted after its line `after`.
fn edge_cases_with(after: usize, line: &str) -> String {
    let mut lines: Vec<&str> = EDGE_CASES.lines().collect();
    lines.insert(after, line);
    lines.join("\n") + "\n"
}

#[test]
fn insertion_at_a_statement_site_goes_after_the_whole_statement() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let file = file.to_str().unwrap();
    let preview = |at: &str, from: &str| {
        tsparse(
            &fixture,
            &[
                "--insert-after",
                "--at",
                at,
                "edge-cases.test.ts",
                "--from",
                from,
            ],
        )
    };
    let output = preview("10:3", "new-test.ts");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!(
            "{file}:10:3: insert_after statement it.skip(\"skipped one\", () => {{ … }}): after the statement at 10:3-10:36\n\
             validated: syntax and edit boundaries; not type-checked\n\
             --- {file}\n\
             +++ {file}\n\
             @@ -8,6 +8,7 @@\n   \
             }});\n   \
             it.todo(\"writes later\");\n   \
             it.skip(\"skipped one\", () => {{}});\n\
             +  it(\"new test\", () => {{}});\n   \
             test(\"with options\", {{ timeout: 1000 }}, async () => {{}});\n   \
             it.skipIf(process.env.CI)(\"conditional\", () => {{}});\n   \
             test.concurrent(\"parallel\", async () => {{}});\n\
             preview only: save a plan with --plan PLAN to apply it\n"
        )
    );
    // The hook.
    let output = preview("4:3", "new-hook.ts");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!(
            "{file}:4:3: insert_after statement beforeEach(() => {{ … }}): after the statement at 4:3-4:24\n\
             validated: syntax and edit boundaries; not type-checked\n\
             --- {file}\n\
             +++ {file}\n\
             @@ -2,6 +2,7 @@\n \
             class Widget {{}}\n \
             describe(Widget.name, () => {{\n   \
             beforeEach(() => {{}});\n\
             +  afterEach(() => {{}});\n   \
             it.each([1, 2])(\"adds %i\", (n) => {{ expect(n).toBeTruthy(); }});\n   \
             describe.each([[\"a\"], [\"b\"]])(\"case %s\", (x) => {{\n     \
             it(`handles ${{\"x\"}}`, () => {{}});\n\
             preview only: save a plan with --plan PLAN to apply it\n"
        )
    );
    // The statement holding the `it.each` callback, whose anchor `5:30`
    // still selects the callback itself, which insertion refuses.
    let output = preview("5:3", "new-test.ts");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        text(&output.stdout).starts_with(&format!(
            "{file}:5:3: insert_after statement it.each([1, 2])(\"adds %i\", (n) => {{ … }}): after the statement at 5:3-5:66\n"
        )),
        "{output:?}"
    );
    let output = preview("5:30", "new-test.ts");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        text(&output.stderr).starts_with(&format!(
            "{file}:5:30: edit 1: <function> cannot have statements inserted after it: this function is not a statement in a statement list"
        )),
        "{output:?}"
    );
    // The edit document's site is the whole statement site, with its depth:
    // the `describe` callback at depth 1 contains the statement.
    let document = json(
        &tsparse(
            &fixture,
            &[
                "--insert-after",
                "--at",
                "10:3",
                "edge-cases.test.ts",
                "--from",
                "new-test.ts",
                "--json",
            ],
        ),
        0,
    );
    assert_eq!(
        document["files"][0]["edits"][0]["site"],
        json!({
            "label": "it.skip(\"skipped one\", () => { … })",
            "kind": "statement",
            "anchor": { "line": 10, "column": 3 },
            "range": {
                "start": { "line": 10, "column": 3 },
                "end": { "line": 10, "column": 36 },
            },
            "depth": 2,
        })
    );
    assert_eq!(edge_cases_text(&fixture), EDGE_CASES);
}

#[test]
fn a_saved_plan_of_a_statement_site_insertion_applies_inside_the_describe_callback() {
    let fixture = edge_cases();
    let output = tsparse(
        &fixture,
        &[
            "--insert-after",
            "--at",
            "edge-cases.test.ts:10:3",
            "--from",
            "new-test.ts",
            "--plan",
            "plan.json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(edge_cases_text(&fixture), EDGE_CASES);
    let output = tsparse(&fixture, &["--apply", "plan.json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        edge_cases_text(&fixture),
        edge_cases_with(10, "  it(\"new test\", () => {});")
    );
}

#[test]
fn a_batch_inserts_after_the_statement_sites_its_positions_select() {
    let fixture = edge_cases();
    std::fs::write(
        fixture.path("edits.json"),
        r#"{ "edits": [
            { "operation": "insert_after", "at": "4:3", "file": "edge-cases.test.ts", "from": "new-hook.ts" },
            { "operation": "insert_after", "at": "edge-cases.test.ts:10:4", "from": "new-test.ts" }
        ] }"#,
    )
    .unwrap();
    let document = json(
        &tsparse(
            &fixture,
            &["--edit-batch", "edits.json", "--plan", "p.json", "--json"],
        ),
        0,
    );
    let edits = &document["files"][0]["edits"];
    assert_eq!(edits[0]["site"]["kind"], "statement");
    assert_eq!(
        edits[1]["site"]["anchor"],
        json!({ "line": 10, "column": 3 })
    );
    let output = tsparse(&fixture, &["--apply", "p.json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let mut expected: Vec<&str> = EDGE_CASES.lines().collect();
    expected.insert(10, "  it(\"new test\", () => {});");
    expected.insert(4, "  afterEach(() => {});");
    assert_eq!(edge_cases_text(&fixture), expected.join("\n") + "\n");

    // Two insertions after one statement conflict.
    let fixture = edge_cases();
    std::fs::write(
        fixture.path("edits.json"),
        r#"{ "edits": [
            { "operation": "insert_after", "at": "10:3", "file": "edge-cases.test.ts", "from": "new-hook.ts" },
            { "operation": "insert_after", "at": "10:4", "file": "edge-cases.test.ts", "from": "new-test.ts" }
        ] }"#,
    )
    .unwrap();
    let document = json(
        &tsparse(&fixture, &["--edit-batch", "edits.json", "--json"]),
        1,
    );
    assert_eq!(document["outcome"], "rejected");
    assert_eq!(document["error"]["kind"], "Conflict");
    assert!(
        document["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("edits 1 and 2 conflict: "),
        "{document}"
    );
    assert_eq!(edge_cases_text(&fixture), EDGE_CASES);
}

const UNBRACED: &str = "\
declare const ready: boolean;
declare function run(): void;
if (ready) run();
else run();
while (ready) run();
label: run();
class K {
  static {
    run();
  }
}
do run(); while (ready);
for (;;) run();
for (const key in {}) run();
for (const item of []) run();
declare const scope: object;
with (scope) run();
switch (ready) { case true: run(); }
namespace N { run(); }
";

#[test]
fn a_statement_outside_a_statement_list_is_shown_but_refuses_insertion() {
    let fixture = Fixture::new(&[("unbraced.ts", UNBRACED), ("statement.ts", "run();\n")]);
    let file = fixture.path("unbraced.ts");
    let file = file.to_str().unwrap();
    let output = tsparse(&fixture, &["--show", "--at", "3:12", "unbraced.ts"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!("{file}:3:12 statement run()\n3 | run();\n")
    );
    let insert = |at: &str, json: bool| {
        let mut args = vec![
            "--insert-after",
            "--at",
            at,
            "unbraced.ts",
            "--from",
            "statement.ts",
        ];
        if json {
            args.push("--json");
        }
        tsparse(&fixture, &args)
    };
    const REASON: &str = "cannot have statements inserted after it: this statement is not a statement in a statement list (statements can be inserted only after a statement of a file, block, module block, or case clause)";
    // The bodies of `if`, `else`, each loop, `with`, and a label, and a
    // class static block's body.
    for (at, label) in [
        ("3:12", "run()"),
        ("4:6", "run()"),
        ("5:15", "run()"),
        ("6:8", "run()"),
        ("8:10", "{ run(); }"),
        ("12:4", "run()"),
        ("13:10", "run()"),
        ("14:23", "run()"),
        ("15:24", "run()"),
        ("17:14", "run()"),
    ] {
        let output = insert(at, false);
        assert_eq!(output.status.code(), Some(1), "{at}: {output:?}");
        assert!(output.stdout.is_empty(), "{at}");
        assert_eq!(
            text(&output.stderr),
            format!("{file}:{at}: edit 1: {label} {REASON}\n"),
            "{at}"
        );
        let document = json(&insert(at, true), 1);
        assert_eq!(document["outcome"], "rejected", "{at}");
        assert_eq!(document["error"]["kind"], "UnsupportedSite", "{at}");
        assert_eq!(
            document["error"]["message"],
            format!("{label} {REASON}"),
            "{at}"
        );
    }
    // The `if` itself, and statements of the static block's body, a case
    // clause, and a module block, are in statement lists.
    for at in ["3:1", "9:5", "18:29", "19:15"] {
        let output = insert(at, false);
        assert_eq!(output.status.code(), Some(0), "{at}: {output:?}");
    }
    assert_eq!(std::fs::read_to_string(file).unwrap(), UNBRACED);
}

#[test]
fn body_replacement_at_a_statement_site_is_refused() {
    let fixture = edge_cases();
    std::fs::write(
        fixture.path("functions.ts"),
        "export function start() {\n  return 1;\n}\nfunction plain() {}\n",
    )
    .unwrap();
    const REASON: &str = "cannot have its body replaced: a statement site has no body to replace; select the function, method, or arrow function by its name or anchor";
    let replace = |at: &str, file: &str| {
        tsparse(
            &fixture,
            &["--replace-body", "--at", at, file, "--from", "body.ts"],
        )
    };
    for (at, file, label) in [
        (
            "10:3",
            "edge-cases.test.ts",
            "it.skip(\"skipped one\", () => { … })",
        ),
        // A function declaration selected by its `export` or `function`
        // keyword is a statement site, not the function.
        ("1:1", "functions.ts", "export function start() { … }"),
        ("4:1", "functions.ts", "function plain() { … }"),
    ] {
        let output = replace(at, file);
        assert_eq!(output.status.code(), Some(1), "{at}: {output:?}");
        assert!(output.stdout.is_empty(), "{at}");
        assert_eq!(
            text(&output.stderr),
            format!(
                "{}:{at}: edit 1: {label} {REASON}\n",
                fixture.path(file).display()
            ),
            "{at}"
        );
    }
    // The function's name selects the function, whose body is replaced.
    let output = replace("4:10", "functions.ts");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    // In a batch, the refusal names its edit.
    std::fs::write(
        fixture.path("edits.json"),
        r#"{ "edits": [
            { "operation": "insert_after", "at": "4:3", "file": "edge-cases.test.ts", "from": "new-hook.ts" },
            { "operation": "replace_body", "at": "functions.ts:1:1", "from": "body.ts" }
        ] }"#,
    )
    .unwrap();
    let output = tsparse(&fixture, &["--edit-batch", "edits.json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        text(&output.stderr),
        format!(
            "{}:1:1: edit 2: export function start() {{ … }} {REASON}\n",
            fixture.path("functions.ts").display()
        )
    );
    let document = json(
        &tsparse(&fixture, &["--edit-batch", "edits.json", "--json"]),
        1,
    );
    assert_eq!(document["error"]["kind"], "UnsupportedSite");
    assert_eq!(document["error"]["edit"], 1);
    assert_eq!(
        document["error"]["message"],
        format!("export function start() {{ … }} {REASON}")
    );
    assert_eq!(edge_cases_text(&fixture), EDGE_CASES);
}

/// Why an insertion is refused at a site of `kind` outside a statement list.
fn outside_a_statement_list(kind: &str) -> String {
    format!(
        "this {kind} is not a statement in a statement list (statements can be inserted only after a statement of a file, block, module block, or case clause)"
    )
}

/// The remedy a callback's insertion refusal names, for the enclosing
/// statement whose first token starts at `at`.
fn remedy(at: &str) -> String {
    format!("; to insert after the enclosing statement, select it with --at {at}")
}

#[test]
fn a_refused_callback_names_the_position_that_selects_its_enclosing_statement() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let file = file.to_str().unwrap();
    let insert = |at: &str, json: bool| {
        let mut args = vec![
            "--insert-after",
            "--at",
            at,
            "edge-cases.test.ts",
            "--from",
            "new-test.ts",
        ];
        if json {
            args.push("--json");
        }
        tsparse(&fixture, &args)
    };
    let message = format!(
        "<function> cannot have statements inserted after it: {}{}",
        outside_a_statement_list("function"),
        remedy("5:3")
    );
    let output = insert("5:30", false);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert_eq!(
        text(&output.stderr),
        format!("{file}:5:30: edit 1: {message}\n")
    );
    assert_eq!(
        json(&insert("5:30", true), 1),
        json!({
            "effect": "none",
            "error": {
                "edit": 0,
                "file": file,
                "kind": "UnsupportedSite",
                "message": message,
                "position": { "column": 30, "line": 5 },
                "proposed": null,
            },
            "operation": "edit",
            "outcome": "rejected",
        })
    );
    // The printed position inserts after the enclosing statement.
    let stderr = text(&output.stderr);
    let printed = stderr.trim_end().rsplit_once(" --at ").unwrap().1;
    let output = insert(printed, false);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        text(&output.stdout),
        format!(
            "{file}:5:3: insert_after statement it.each([1, 2])(\"adds %i\", (n) => {{ … }}): after the statement at 5:3-5:66\n\
             validated: syntax and edit boundaries; not type-checked\n\
             --- {file}\n\
             +++ {file}\n\
             @@ -3,6 +3,7 @@\n \
             describe(Widget.name, () => {{\n   \
             beforeEach(() => {{}});\n   \
             it.each([1, 2])(\"adds %i\", (n) => {{ expect(n).toBeTruthy(); }});\n\
             +  it(\"new test\", () => {{}});\n   \
             describe.each([[\"a\"], [\"b\"]])(\"case %s\", (x) => {{\n     \
             it(`handles ${{\"x\"}}`, () => {{}});\n   \
             }});\n\
             preview only: save a plan with --plan PLAN to apply it\n"
        )
    );
    let output = tsparse(
        &fixture,
        &[
            "--insert-after",
            "--at",
            &format!("edge-cases.test.ts:{printed}"),
            "--from",
            "new-test.ts",
            "--plan",
            "plan.json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let output = tsparse(&fixture, &["--apply", "plan.json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        edge_cases_text(&fixture),
        edge_cases_with(5, "  it(\"new test\", () => {});")
    );
    // Each other callback of the sample names its own innermost statement:
    // one of the file, of the `describe` callback, of the `describe.each`
    // callback, and one that begins with `await`.
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let file = file.to_str().unwrap();
    for (at, label, statement) in [
        ("3:23", "describe() callback", "3:1"),
        ("4:14", "beforeEach() callback", "4:3"),
        ("7:26", "it(`handles ${\"x\"}`) callback", "7:5"),
        ("17:35", "test.step(\"first step\") callback", "17:5"),
    ] {
        let output = tsparse(
            &fixture,
            &[
                "--insert-after",
                "--at",
                at,
                "edge-cases.test.ts",
                "--from",
                "new-test.ts",
            ],
        );
        assert_eq!(output.status.code(), Some(1), "{at}: {output:?}");
        assert_eq!(
            text(&output.stderr),
            format!(
                "{file}:{at}: edit 1: {label} cannot have statements inserted after it: {}{}\n",
                outside_a_statement_list("function"),
                remedy(statement)
            ),
            "{at}"
        );
        let output = tsparse(
            &fixture,
            &[
                "--insert-after",
                "--at",
                statement,
                "edge-cases.test.ts",
                "--from",
                "new-test.ts",
            ],
        );
        assert_eq!(output.status.code(), Some(0), "{statement}: {output:?}");
        assert!(
            text(&output.stdout)
                .starts_with(&format!("{file}:{statement}: insert_after statement ")),
            "{statement}: {output:?}"
        );
    }
    assert_eq!(edge_cases_text(&fixture), EDGE_CASES);
}

#[test]
fn a_batch_refusal_at_a_callback_names_the_same_remedy() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let file = file.to_str().unwrap();
    std::fs::write(
        fixture.path("edits.json"),
        r#"{ "edits": [
            { "operation": "insert_after", "at": "4:3", "file": "edge-cases.test.ts", "from": "new-hook.ts" },
            { "operation": "insert_after", "at": "edge-cases.test.ts:5:30", "from": "new-test.ts" }
        ] }"#,
    )
    .unwrap();
    let message = format!(
        "<function> cannot have statements inserted after it: {}{}",
        outside_a_statement_list("function"),
        remedy("5:3")
    );
    let output = tsparse(&fixture, &["--edit-batch", "edits.json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert_eq!(
        text(&output.stderr),
        format!("{file}:5:30: edit 2: {message}\n")
    );
    let document = json(
        &tsparse(&fixture, &["--edit-batch", "edits.json", "--json"]),
        1,
    );
    assert_eq!(document["outcome"], "rejected");
    assert_eq!(document["error"]["kind"], "UnsupportedSite");
    assert_eq!(document["error"]["edit"], 1);
    assert_eq!(document["error"]["message"], message);
    assert_eq!(edge_cases_text(&fixture), EDGE_CASES);
}

/// Functions and arrows whose innermost enclosing statement is an item of a
/// statement list that its first token selects: a file's, a block's of
/// `try`, `catch`, and `finally`, a function body's, and a `case` clause's.
const REMEDIED: &str = "\
declare function f(callback: unknown): unknown;
const bound = f(function named() {});
try { f(() => {}); } catch { f(() => {}); } finally { f(() => {}); }
function outer() { return () => {}; }
/*\u{1F600}*/ f(async () => {});
switch (bound) { case 1: f(() => {}); }
export default () => {};
";

#[test]
fn the_remedy_names_any_enclosing_statement_its_first_token_selects() {
    let fixture = Fixture::new(&[
        ("remedied.ts", REMEDIED),
        (
            "assigned.ts",
            "declare function f(callback: unknown): unknown;\nexport = f(() => {});\n",
        ),
        ("statement.ts", "f(0);\n"),
    ]);
    let insert = |at: &str, file: &str, json: bool| {
        let mut args = vec!["--insert-after", "--at", at, file, "--from", "statement.ts"];
        if json {
            args.push("--json");
        }
        tsparse(&fixture, &args)
    };
    // Each refusal, the position it names in UTF-16 columns, and the range of
    // the statement that position selects. The `export =` statement is
    // selected by its `export=` entry, which designates it; every other one
    // is a statement site.
    for (file, at, label, statement, end) in [
        ("remedied.ts", "2:26", "named", "2:1", "2:38"),
        ("remedied.ts", "3:9", "f() callback", "3:7", "3:19"),
        ("remedied.ts", "3:32", "f() callback", "3:30", "3:42"),
        ("remedied.ts", "3:57", "f() callback", "3:55", "3:67"),
        ("remedied.ts", "4:27", "<function>", "4:20", "4:36"),
        ("remedied.ts", "5:10", "f() callback", "5:8", "5:26"),
        ("remedied.ts", "6:28", "f() callback", "6:26", "6:38"),
        ("remedied.ts", "7:16", "default", "7:1", "7:25"),
        ("assigned.ts", "2:12", "f() callback", "2:1", "2:22"),
    ] {
        let path = fixture.path(file);
        let output = insert(at, file, false);
        assert_eq!(output.status.code(), Some(1), "{at}: {output:?}");
        assert_eq!(
            text(&output.stderr),
            format!(
                "{}:{at}: edit 1: {label} cannot have statements inserted after it: {}{}\n",
                path.display(),
                outside_a_statement_list("function"),
                remedy(statement)
            ),
            "{at}"
        );
        let document = json(&insert(statement, file, true), 0);
        let site = &document["files"][0]["edits"][0]["site"];
        let position = |text: &str| {
            let (line, column) = text.split_once(':').unwrap();
            json!({ "line": line.parse::<u64>().unwrap(), "column": column.parse::<u64>().unwrap() })
        };
        assert_eq!(site["anchor"], position(statement), "{at}");
        assert_eq!(
            site["range"],
            json!({ "start": position(statement), "end": position(end) }),
            "{at}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(fixture.path("remedied.ts")).unwrap(),
        REMEDIED
    );
}

/// Refusals that name no remedy: callbacks whose innermost statement is an
/// unbraced `if` or label body, an arrow statement whose first token selects
/// the arrow again, and sites that are not functions: a parameter property,
/// a method, a class expression, loop and `catch` bindings, and a method
/// whose computed name holds an arrow and a statement, either of whose
/// positions selects the method.
const UNREMEDIED: &str = "\
declare const ready: boolean;
declare function f(callback: unknown): unknown;
if (ready) f(() => {});
x => {};
class Members {
  constructor(private readonly kept: number) {}
  method() {}
}
const Expression = f(class {});
for (const item of [1]) f(item);
try {} catch (error) {}
label: f(() => {});
class Computed { [(() => { f(0); })()]() {} }
";

#[test]
fn other_insertion_refusals_name_no_remedy() {
    let fixture = Fixture::new(&[("refused.ts", UNREMEDIED), ("statement.ts", "f(0);\n")]);
    let file = fixture.path("refused.ts");
    let file = file.to_str().unwrap();
    let reason = |kind: &str| outside_a_statement_list(kind);
    // Each position, the selected site's anchor the refusal names, its
    // label, and why it is refused.
    for (at, anchor, label, reason) in [
        ("3:14", (3, 14), "f() callback", reason("function")),
        ("4:1", (4, 1), "<function>", reason("function")),
        ("6:32", (6, 32), "kept", reason("property")),
        ("7:3", (7, 3), "method", reason("method")),
        ("9:22", (9, 22), "<class>", reason("class")),
        (
            "10:12",
            (10, 12),
            "item",
            "this binding is declared in a loop header, which has no statement list to insert into"
                .into(),
        ),
        (
            "11:15",
            (11, 15),
            "error",
            "this binding is a catch clause variable, which has no statement list to insert into"
                .into(),
        ),
        ("12:10", (12, 10), "f() callback", reason("function")),
        // The arrow, and the `f` of the statement in its body.
        ("13:20", (13, 18), "[(() => { f(0); })()]", reason("method")),
        ("13:28", (13, 18), "[(() => { f(0); })()]", reason("method")),
    ] {
        let message = format!("{label} cannot have statements inserted after it: {reason}");
        let insert = |json: bool| {
            let mut args = vec![
                "--insert-after",
                "--at",
                at,
                "refused.ts",
                "--from",
                "statement.ts",
            ];
            if json {
                args.push("--json");
            }
            tsparse(&fixture, &args)
        };
        let output = insert(false);
        assert_eq!(output.status.code(), Some(1), "{at}: {output:?}");
        let (line, column) = anchor;
        assert_eq!(
            text(&output.stderr),
            format!("{file}:{line}:{column}: edit 1: {message}\n"),
            "{at}"
        );
        let document = json(&insert(true), 1);
        assert_eq!(document["error"]["kind"], "UnsupportedSite", "{at}");
        assert_eq!(document["error"]["message"], message, "{at}");
        assert_eq!(
            document["error"]["position"],
            json!({ "line": line, "column": column }),
            "{at}"
        );
    }
    assert_eq!(std::fs::read_to_string(file).unwrap(), UNREMEDIED);
}
