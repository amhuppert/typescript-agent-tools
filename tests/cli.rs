use std::{
    io::Write,
    process::{Command, Stdio},
};

fn cli(args: &[&str], source: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(source).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn stdin_emits_compiler_style_json() {
    let output = cli(&["--pretty"], b"const answer = 42;");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["kind"], 308);
    assert_eq!(
        json["statements"][0]["declarationList"]["declarations"][0]["initializer"]["text"],
        "42"
    );
}

#[test]
fn syntax_and_usage_errors_have_distinct_exit_codes() {
    let output = cli(&[], b"const answer = ;");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .starts_with("-:1:16:")
    );
    assert_eq!(cli(&["--unknown"], b"").status.code(), Some(2));
    assert_eq!(cli(&[], &[255]).status.code(), Some(2));
}

/// Help is the registry's, which `tests/cli_registry.rs` checks. The
/// version names the complete build stamp, which `tests/doctor.rs` checks
/// against a running service's.
#[test]
fn version_prints_the_package_version_and_build() {
    for arg in ["-V", "--version"] {
        let output = cli(&[arg], b"");
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!(
                "tsparse {} (build {})\n",
                env!("CARGO_PKG_VERSION"),
                env!("TSPARSE_SERVICE_BUILD")
            )
        );
    }
}

#[cfg(unix)]
#[test]
fn help_and_version_report_closed_stdout_as_io_errors() {
    for arg in ["--help", "--version"] {
        let mut reader = Command::new("sh")
            .args(["-c", "exit 0"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pipe = reader.stdin.take().unwrap();
        // Wait until the pipe's only reader has exited before launching tsparse.
        assert!(reader.wait().unwrap().success());
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .arg(arg)
            .stdin(Stdio::null())
            .stdout(pipe)
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{arg}: {output:?}");
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn jsx_and_check_modes_work() {
    assert!(cli(&["--tsx"], b"const view = <div />;").status.success());
    let output = cli(&["--check"], b"type X = {a: string};");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "ok: - parses as TypeScript 7 (jsx: off)\n"
    );
}

#[test]
fn output_file_contains_the_complete_json_ast() {
    let file = std::env::temp_dir().join(format!("tsparse-cli-test-{}.json", std::process::id()));
    let output = cli(&["-o", file.to_str().unwrap()], b"export const x = 1;");
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(json["text"], "export const x = 1;");
    std::fs::remove_file(file).unwrap();
}

#[test]
fn deeply_nested_class_heritage_returns_a_diagnostic() {
    let source = format!(
        "class C extends {}class {{}}{}",
        "class extends ".repeat(9_999),
        " {}".repeat(10_000)
    );
    let output = cli(&["--check"], source.as_bytes());
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Maximum syntax nesting depth exceeded")
    );
}

#[test]
fn file_extensions_select_jsx_without_case() {
    let dir = std::env::temp_dir().join(format!("tsparse-cli-jsx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["view.tsx", "VIEW.TSX", "view.jsx"] {
        let file = dir.join(name);
        std::fs::write(&file, "const a = <div />;").unwrap();
        let output = cli(&["--check", file.to_str().unwrap()], b"");
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_file(file).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}
