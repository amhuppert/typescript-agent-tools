//! The editing command contract: `--replace-body`, `--insert-after`,
//! `--edit-batch`, and `--apply`; request validity; text and JSON forms;
//! plans; output channels and exit codes; and file, direct, and service
//! entry agreeing. Which edits are valid is covered by `editing.rs`.
mod support;

use serde_json::{Value, json};
use std::io::Write as _;
use std::process::{Command, Output, Stdio};
use support::Fixture;
use support::service::Service;

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
    let mut refused = vec![
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
    {
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
    }
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
    let mut changes: Vec<(&str, Change, &str)> = vec![
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
