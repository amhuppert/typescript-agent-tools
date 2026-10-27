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

/// `--check` reports the error TypeScript 7's checker reports at the name of
/// a namespace declared with the `module` keyword (TS1540), the first in
/// source order, and fails as a syntax error does. Without `--check` the
/// AST is still written: TypeScript 7 parses the declaration. Forms whose
/// keyword is not `module`, whose name is a string, whose `module` is not a
/// declaration, or that TypeScript's checker reports out of place instead
/// (TS1235, which `--check` leaves to a checker as it does for `namespace`)
/// pass. Positions are the native 7.0.2 compiler's.
#[test]
fn check_reports_namespaces_declared_with_module() {
    const TS1540: &str = "A 'namespace' declaration should not be declared using the 'module' \
                          keyword. Please use the 'namespace' keyword instead.";
    let fixture = Fixture::new(&[
        ("plain.ts", "module X {}"),
        (
            "declared.ts",
            "declare module X { export const a: number; }",
        ),
        ("types.d.ts", "declare module X { export const a: number; }"),
        (
            "dotted.ts",
            "namespace A.B {}\nmodule C.D { export const a = 1; }",
        ),
        ("exported.ts", "export module X { export const a = 1; }"),
        (
            "nested.ts",
            "namespace N {\n  module X { export const a = 1; }\n}",
        ),
        ("crlf.ts", "namespace N {\r\n  module X {}\r\n}"),
        ("wide.ts", "const s = \"\u{1F600}\"; module X {}"),
        (
            "quoted.d.ts",
            "declare module \"x\" { export const a: number; }",
        ),
        ("global.ts", "export {};\ndeclare global { interface G {} }"),
        ("namespace.ts", "namespace X { export const a = 1; }"),
        ("lines.ts", "module\nX\n{}"),
        ("function.ts", "function f() { module X {} }"),
    ]);
    let rejected = |file: &str, line: usize, column: usize| {
        (
            Some(1),
            String::new(),
            format!("{file}:{line}:{column}: {TS1540}\n"),
        )
    };
    for (args, stdin, expected) in [
        (vec!["--check", "plain.ts"], "", rejected("plain.ts", 1, 8)),
        (vec!["--check"], "module X {}", rejected("-", 1, 8)),
        (
            vec!["--check", "declared.ts"],
            "",
            rejected("declared.ts", 1, 16),
        ),
        (
            vec!["--check", "types.d.ts"],
            "",
            rejected("types.d.ts", 1, 16),
        ),
        (
            vec!["--check", "dotted.ts"],
            "",
            rejected("dotted.ts", 2, 8),
        ),
        (
            vec!["--check", "exported.ts"],
            "",
            rejected("exported.ts", 1, 15),
        ),
        (
            vec!["--check", "nested.ts"],
            "",
            rejected("nested.ts", 2, 10),
        ),
        (vec!["--check", "crlf.ts"], "", rejected("crlf.ts", 2, 10)),
        (vec!["--check", "wide.ts"], "", rejected("wide.ts", 1, 24)),
    ] {
        assert_eq!(
            outcome(&tsparse(&fixture, &args, stdin)),
            expected,
            "{args:?}"
        );
    }
    for file in [
        "quoted.d.ts",
        "global.ts",
        "namespace.ts",
        "lines.ts",
        "function.ts",
    ] {
        assert_eq!(
            outcome(&tsparse(&fixture, &["--check", file], "")),
            (
                Some(0),
                format!("ok: {file} parses as TypeScript 7 (jsx: off)\n"),
                String::new()
            ),
            "{file}"
        );
    }
    let ast = tsparse(&fixture, &["plain.ts"], "");
    assert_eq!(ast.status.code(), Some(0), "{ast:?}");
    let ast: serde_json::Value = serde_json::from_slice(&ast.stdout).unwrap();
    assert_eq!(ast["statements"][0]["kind"], 268);
    assert_eq!(ast["statements"][0]["flags"], 0);
    assert_eq!(ast["statements"][0]["name"]["escapedText"], "X");
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
