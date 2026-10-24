//! Clean verdicts (design, step 5): a command that checks or stops
//! something and succeeds says so in one line on stdout, naming what it
//! checked, rather than printing nothing.
mod support;

use std::io::Write as _;
use std::process::{Command, Output, Stdio};
use support::Fixture;

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The exit code, stdout, and stderr of `output`.
fn outcome(output: &Output) -> (Option<i32>, String, String) {
    (
        output.status.code(),
        text(&output.stdout),
        text(&output.stderr),
    )
}

/// Run the binary in `fixture` with `stdin`.
fn tsparse(fixture: &Fixture, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
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
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// `--check` prints `ok: FILE parses as TypeScript 7 (jsx: off)`, naming
/// the file as given, `-` for stdin, and `on` where JSX is enabled: by
/// `--tsx`, or by a `.tsx` or `.jsx` name in any case. It still writes no
/// AST, so `--output` and `--pretty` change nothing, and a syntax error
/// prints its diagnostic and no verdict.
#[test]
fn check_prints_one_verdict_line() {
    let fixture = Fixture::new(&[
        ("a.ts", "type X = {a: string};"),
        ("my file.ts", "let x = 1;"),
        ("view.tsx", "const a = <div />;"),
        ("VIEW.TSX", "const a = <div />;"),
        ("view.jsx", "const a = <div />;"),
        ("bad.ts", "let = ;"),
    ]);
    let verdict =
        |file: &str, jsx: &str| format!("ok: {file} parses as TypeScript 7 (jsx: {jsx})\n");
    let mut cases: Vec<(Vec<&str>, &str, String)> = vec![
        (vec!["--check", "a.ts"], "", verdict("a.ts", "off")),
        (
            vec!["--check", "my file.ts"],
            "",
            verdict("my file.ts", "off"),
        ),
        (vec!["--check", "view.tsx"], "", verdict("view.tsx", "on")),
        (vec!["--check", "view.jsx"], "", verdict("view.jsx", "on")),
        (vec!["--check", "--tsx", "a.ts"], "", verdict("a.ts", "on")),
        (vec!["--check"], "let x = 1;", verdict("-", "off")),
        (vec!["--check", "-"], "let x = 1;", verdict("-", "off")),
        (vec!["--check", "--tsx"], "<div />;", verdict("-", "on")),
        (
            vec!["--tsx", "--check", "-"],
            "<div />;",
            verdict("-", "on"),
        ),
        (
            vec!["--check", "--pretty", "a.ts"],
            "",
            verdict("a.ts", "off"),
        ),
        (
            vec!["--check", "a.ts", "-o", "out.json"],
            "",
            verdict("a.ts", "off"),
        ),
    ];
    // A case-insensitive file system names one file by both spellings.
    if std::fs::read_to_string(fixture.path("VIEW.TSX")).is_ok() {
        cases.push((vec!["--check", "VIEW.TSX"], "", verdict("VIEW.TSX", "on")));
    }
    for (args, stdin, expected) in cases {
        assert_eq!(
            outcome(&tsparse(&fixture, &args, stdin)),
            (Some(0), expected, String::new()),
            "{args:?}"
        );
        assert!(!fixture.path("out.json").exists(), "{args:?}");
    }
    let rejected = tsparse(&fixture, &["--check", "bad.ts"], "");
    assert_eq!(rejected.status.code(), Some(1), "{rejected:?}");
    assert!(rejected.stdout.is_empty(), "{rejected:?}");
    assert!(
        text(&rejected.stderr).starts_with("bad.ts:1:"),
        "{rejected:?}"
    );
}

/// `--references-stop` prints `stopped: pid N`, the pid status reported,
/// when it stopped a service, and `no service: CONFIG`, naming the
/// configuration as given, when none was running.
#[cfg(unix)]
#[test]
fn stop_names_the_service_it_stopped() {
    let service = support::service::Service::new(&[("a.ts", "export const a = 1;\n")]);
    // No runtime directory yet, so no service.
    assert_eq!(
        outcome(&service.run(&["--references-stop"])),
        (Some(0), "no service: tsconfig.json\n".into(), String::new())
    );
    let started = service.run(&["--outline", "a.ts"]);
    assert_eq!(started.status.code(), Some(0), "{started:?}");
    let pid = service.status()["pid"].as_u64().unwrap();
    assert_eq!(
        outcome(&service.run(&["--references-stop"])),
        (Some(0), format!("stopped: pid {pid}\n"), String::new())
    );
    assert_eq!(service.status()["running"], false);
    // The runtime directory remains, without a service.
    assert_eq!(
        outcome(&service.run(&["--references-stop"])),
        (Some(0), "no service: tsconfig.json\n".into(), String::new())
    );
    // The configuration is named as given.
    let output = service
        .command()
        .args(["--references-stop", "--project", "./tsconfig.json"])
        .output()
        .unwrap();
    assert_eq!(
        outcome(&output),
        (
            Some(0),
            "no service: ./tsconfig.json\n".into(),
            String::new()
        )
    );
}
