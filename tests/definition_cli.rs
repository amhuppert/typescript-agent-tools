//! The `--definition` command contract, on the Examples project of the
//! definitions design (`docs/definition-and-import-hints-design.md`,
//! "Examples"), whose outputs that design states with `/work` for the
//! project's directory. Every answer is compared between the project
//! service and `--direct`, byte for byte, and every printed command is
//! followed as printed.
mod support;

use serde_json::{Value, json};
use std::process::Output;
use support::Fixture;
use support::service::Service;

const CONFIG: &str = r#"{
  "compilerOptions": {
    "strict": true,
    "target": "es2022",
    "module": "esnext",
    "moduleResolution": "bundler"
  },
  "include": ["src"]
}
"#;

const LIB: &str =
    "export function formatTitle(title: string): string {\n  return title.trim();\n}\n";
const INDEX: &str = "export { formatTitle } from \"./lib\";\n";
const APP: &str = "import { formatTitle } from \"./index\";\n\nconst page = { title: formatTitle(\" Docs \") };\nexport const heading = page.title;\n";

/// The Examples project and its service, with `extra` files.
fn examples(extra: &[(&str, &str)]) -> Service {
    let mut files = vec![
        ("tsconfig.json", CONFIG),
        ("src/lib.ts", LIB),
        ("src/index.ts", INDEX),
        ("src/app.ts", APP),
    ];
    files.extend_from_slice(extra);
    Service::of(Fixture::new(&files))
}

/// `/work` in the design's outputs: the project's canonical directory.
fn work(service: &Service, text: &str) -> String {
    text.replace("/work", service.fixture.root.to_str().unwrap())
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// `args` with `--definition` and `--project tsconfig.json`, from the
/// service and directly, which must agree byte for byte.
fn definition(service: &Service, args: &[&str]) -> Output {
    service.both(&[&["--definition"], args].concat())
}

/// A result: exit 0, `expected` on stdout, nothing on stderr.
fn result(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(stderr(output), "", "{output:?}");
    assert_eq!(stdout(output), expected);
}

/// A diagnostic: exit `code`, `expected` on stderr, nothing on stdout.
fn diagnostic(output: &Output, code: i32, expected: &str) {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert_eq!(stdout(output), "", "{output:?}");
    assert_eq!(stderr(output), expected);
}

/// A document on stdout, exiting `code`, with nothing on stderr.
fn document(output: &Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert_eq!(stderr(output), "", "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Run `line`, a command tsparse printed, through a shell in the project's
/// directory with the service's environment.
fn follow(service: &Service, line: &str) -> Output {
    let line = line
        .strip_prefix("tsparse ")
        .unwrap_or_else(|| panic!("{line}"));
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("\"$TSPARSE\" {line}"))
        .current_dir(&service.fixture.root)
        .env("TSPARSE", env!("CARGO_BIN_EXE_tsparse"))
        .env("TSPARSE_SERVICE_DIR", &service.runtime)
        .env("TSPARSE_SERVICE_IDLE_SECS", support::service::IDLE_SECS)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap()
}

/// Stops the service of another spelling of the configuration on drop.
struct Spelled<'a>(&'a Service, &'a str);

impl Drop for Spelled<'_> {
    fn drop(&mut self) {
        let _ = self
            .0
            .command()
            .args(["--references-stop", "--project", self.1])
            .output();
    }
}

const FORMAT_TITLE: &str = "/work/tsconfig.json: at /work/src/app.ts:1:10: 1 group, 1 declaration
group 1: formatTitle
  /work/src/lib.ts:1:17
";

/// The design's examples: the import specifier, a use, and the re-export
/// each print the original declaration.
#[test]
fn positions_on_imports_uses_and_reexports_print_the_original_declaration() {
    let service = examples(&[]);
    for (at, header) in [
        ("src/app.ts:1:10", "/work/src/app.ts:1:10"),
        ("src/app.ts:3:23", "/work/src/app.ts:3:23"),
        ("src/index.ts:1:10", "/work/src/index.ts:1:10"),
    ] {
        let output = definition(&service, &["--at", at]);
        result(
            &output,
            &work(
                &service,
                &FORMAT_TITLE.replace("/work/src/app.ts:1:10", header),
            ),
        );
    }
    // The LINE:COLUMN FILE form selects the same.
    let output = definition(&service, &["--at", "1:10", "src/app.ts"]);
    result(&output, &work(&service, FORMAT_TITLE));
    let output = definition(&service, &["--at", "src/app.ts:1:10", "--json"]);
    assert_eq!(
        document(&output, 0),
        json!({
            "operation": "definition",
            "outcome": "complete",
            "project": service.fixture.path("tsconfig.json"),
            "selection": {"at": {"file": "src/app.ts", "position": {"line": 1, "column": 10}}},
            "groups": [{"declarations": [{
                "name": "formatTitle",
                "location": {
                    "file": "src/lib.ts",
                    "start": {"line": 1, "column": 17},
                    "end": {"line": 1, "column": 28},
                },
            }]}],
        })
    );
    // Fields in the design's order, and --pretty indents the same document.
    assert_eq!(
        stdout(&output),
        work(
            &service,
            "{\"operation\":\"definition\",\"outcome\":\"complete\",\"project\":\"/work/tsconfig.json\",\"selection\":{\"at\":{\"file\":\"src/app.ts\",\"position\":{\"line\":1,\"column\":10}}},\"groups\":[{\"declarations\":[{\"name\":\"formatTitle\",\"location\":{\"file\":\"src/lib.ts\",\"start\":{\"line\":1,\"column\":17},\"end\":{\"line\":1,\"column\":28}}}]}]}\n"
        )
    );
    let pretty = definition(&service, &["--at", "src/app.ts:1:10", "--json", "--pretty"]);
    assert!(stdout(&pretty).lines().count() > 1, "{pretty:?}");
    assert_eq!(document(&pretty, 0), document(&output, 0));
}

/// `--name NAME FILE`, where FILE only imports NAME: the reported question.
#[test]
fn a_name_a_file_imports_prints_its_original_declaration() {
    let service = examples(&[]);
    let output = definition(&service, &["--name", "formatTitle", "src/app.ts"]);
    result(
        &output,
        &work(
            &service,
            "/work/tsconfig.json: `formatTitle` 1 group, 1 declaration; candidates narrowed to /work/src/app.ts
group 1: formatTitle
  /work/src/lib.ts:1:17
",
        ),
    );
    let output = definition(&service, &["--name", "formatTitle", "src/app.ts", "--json"]);
    let answer = document(&output, 0);
    assert_eq!(
        answer["selection"],
        json!({"name": {"name": "formatTitle", "file": "src/app.ts"}})
    );
    assert_eq!(
        answer["groups"][0]["declarations"][0]["name"],
        "formatTitle"
    );
    // Without FILE, every candidate; aliases of one symbol are one group.
    let output = definition(&service, &["--name", "formatTitle"]);
    result(
        &output,
        &work(
            &service,
            "/work/tsconfig.json: `formatTitle` 1 group, 1 declaration
group 1: formatTitle
  /work/src/lib.ts:1:17
",
        ),
    );
    let output = definition(&service, &["--name", "formatTitle", "--json"]);
    assert_eq!(
        document(&output, 0)["selection"],
        json!({"name": {"name": "formatTitle", "file": null}})
    );
}

/// Counts, renamed declarations, and several groups: each group is named
/// by its first declaration, a declaration spelled otherwise shows its
/// name, and the header counts groups and declarations.
#[test]
fn groups_count_their_declarations_and_show_other_names() {
    let service = examples(&[
        (
            "src/over.ts",
            "export function over(a: string): string;\nexport function over(a: number): number;\nexport function over(a: any): any {\n  return a;\n}\n",
        ),
        ("src/def.ts", "export default 42;\n"),
        (
            "src/use.ts",
            "import over from \"./over-default\";\nimport anon from \"./def\";\nexport const n = anon;\nfunction f(x: number) {}\nfunction g(x: string) {}\nexport { f, g, over };\n",
        ),
        (
            "src/over-default.ts",
            "import { over } from \"./over\";\nexport default over;\n",
        ),
        (
            "src/augment.ts",
            "import * as lib from \"./lib\";\ndeclare module \"./lib\" {\n  interface Extra {}\n}\nexport const f = lib.formatTitle;\n",
        ),
    ]);
    // A module and its augmentation are one symbol, whose declarations are
    // spelled differently: the augmentation by its module name, and the
    // file by its path (K2), shown in parentheses after the group's name,
    // its first declaration's.
    let output = definition(&service, &["--at", "src/augment.ts:1:13"]);
    result(
        &output,
        &work(
            &service,
            "/work/tsconfig.json: at /work/src/augment.ts:1:13: 1 group, 2 declarations
group 1: ./lib
  /work/src/augment.ts:2:16
  /work/src/lib.ts:1:1 (src/lib.ts)
",
        ),
    );
    let output = definition(&service, &["--at", "src/use.ts:1:8"]);
    result(
        &output,
        &work(
            &service,
            "/work/tsconfig.json: at /work/src/use.ts:1:8: 1 group, 3 declarations
group 1: over
  /work/src/over.ts:1:17
  /work/src/over.ts:2:17
  /work/src/over.ts:3:17
",
        ),
    );
    // A synthetic anchor (K2) is named by its own spelling.
    let output = definition(&service, &["--at", "src/use.ts:3:18"]);
    result(
        &output,
        &work(
            &service,
            "/work/tsconfig.json: at /work/src/use.ts:3:18: 1 group, 1 declaration
group 1: default
  /work/src/def.ts:1:8
",
        ),
    );
    // Two shadowed parameters matched by one name are two groups, ordered
    // by their first declarations.
    let output = definition(&service, &["--name", "x", "src/use.ts"]);
    result(
        &output,
        &work(
            &service,
            "/work/tsconfig.json: `x` 2 groups, 2 declarations; candidates narrowed to /work/src/use.ts
group 1: x
  /work/src/use.ts:4:12
group 2: x
  /work/src/use.ts:5:12
",
        ),
    );
}

/// A member position answers `member` (exit 1), naming the `--references`
/// search that finds its declarations, which selects the same member when
/// followed as printed.
#[test]
fn a_member_position_names_the_search_that_finds_its_declarations() {
    let service = examples(&[]);
    let output = definition(&service, &["--at", "src/app.ts:4:29"]);
    let expected = work(
        &service,
        "/work/src/app.ts:4:29: `title` is a member, which --definition does not follow; find its declarations with: tsparse --references --project tsconfig.json --at /work/src/app.ts:4:29\n",
    );
    diagnostic(&output, 1, &expected);
    let line = expected.trim_end().split_once("with: ").unwrap().1;
    let followed = follow(&service, line);
    assert_eq!(followed.status.code(), Some(0), "{followed:?}");
    assert_eq!(stderr(&followed), "");
    assert_eq!(
        stdout(&followed),
        work(
            &service,
            "/work/tsconfig.json: at /work/src/app.ts:4:29: 1 group, 2 occurrences in 1 file; coverage member, complete
group 1: title, declared at /work/src/app.ts:3:16
  /work/src/app.ts:3:16 declaration
  /work/src/app.ts:4:29 use
"
        )
    );
    let output = definition(&service, &["--at", "src/app.ts:4:29", "--json"]);
    assert_eq!(
        stdout(&output),
        work(
            &service,
            "{\"operation\":\"definition\",\"project\":\"/work/tsconfig.json\",\"selection\":{\"at\":{\"file\":\"src/app.ts\",\"position\":{\"line\":4,\"column\":29}}},\"outcome\":\"member\",\"name\":\"title\",\"next\":{\"argv\":[\"tsparse\",\"--references\",\"--project\",\"tsconfig.json\",\"--at\",\"/work/src/app.ts:4:29\"]}}\n"
        )
    );
    assert_eq!(document(&output, 1)["outcome"], "member");
}

/// The member's command keeps the request's own `--project` spelling,
/// quoted where a shell needs it, and names no transport.
#[test]
fn the_member_command_keeps_the_project_spelling() {
    let service = examples(&[(
        "my dir/tsconfig.json",
        "{ \"extends\": \"../tsconfig.json\", \"include\": [\"../src\"] }\n",
    )]);
    let _stop = Spelled(&service, "./my dir/tsconfig.json");
    let output = service
        .command()
        .args([
            "--definition",
            "--project",
            "./my dir/tsconfig.json",
            "--at",
            "src/app.ts:4:29",
            "--direct",
        ])
        .output()
        .unwrap();
    let expected = work(
        &service,
        "/work/src/app.ts:4:29: `title` is a member, which --definition does not follow; find its declarations with: tsparse --references --project './my dir/tsconfig.json' --at /work/src/app.ts:4:29\n",
    );
    diagnostic(&output, 1, &expected);
    let followed = follow(
        &service,
        expected.trim_end().split_once("with: ").unwrap().1,
    );
    assert_eq!(followed.status.code(), Some(0), "{followed:?}");
    assert!(
        stdout(&followed).contains("1 group, 2 occurrences in 1 file; coverage member, complete\ngroup 1: title, declared at "),
        "{followed:?}"
    );
}

/// A position in no identifier and a name with no candidate are missing
/// selections: `not_found`, exit 1, never an empty success.
#[test]
fn a_missing_selection_is_not_found() {
    let service = examples(&[]);
    for (at, file) in [
        // The `import` keyword, an empty line, and a module specifier.
        ("src/app.ts:1:1", "/work/src/app.ts:1:1"),
        ("src/app.ts:2:1", "/work/src/app.ts:2:1"),
        ("src/app.ts:1:31", "/work/src/app.ts:1:31"),
    ] {
        let output = definition(&service, &["--at", at]);
        diagnostic(
            &output,
            1,
            &work(
                &service,
                &format!(
                    "{file}: no identifier is at this position; --definition follows identifiers, not keywords or string literals\n"
                ),
            ),
        );
        let output = definition(&service, &["--at", at, "--json"]);
        let (line, column) = at[11..].split_once(':').unwrap();
        assert_eq!(
            stdout(&output),
            work(
                &service,
                &format!(
                    "{{\"operation\":\"definition\",\"project\":\"/work/tsconfig.json\",\"selection\":{{\"at\":{{\"file\":\"src/app.ts\",\"position\":{{\"line\":{line},\"column\":{column}}}}}}},\"outcome\":\"not_found\"}}\n"
                )
            )
        );
        assert_eq!(document(&output, 1)["outcome"], "not_found");
    }
    let output = definition(&service, &["--name", "nothing"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/tsconfig.json: no declaration, import, or export matches `nothing`\n",
        ),
    );
    // A name the file only uses is no candidate there.
    let output = definition(&service, &["--name", "page", "src/index.ts"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/tsconfig.json: no declaration, import, or export matches `page`; candidates narrowed to /work/src/index.ts\n",
        ),
    );
    let output = definition(&service, &["--name", "page", "src/index.ts", "--json"]);
    assert_eq!(
        document(&output, 1),
        json!({
            "operation": "definition",
            "project": service.fixture.path("tsconfig.json"),
            "selection": {"name": {"name": "page", "file": "src/index.ts"}},
            "outcome": "not_found",
        })
    );
    // -o receives the document, and nothing else is written.
    let output = definition(&service, &["--name", "nothing", "--json", "-o", "out.json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(service.fixture.path("out.json")).unwrap())
            .unwrap();
    assert_eq!(written["outcome"], "not_found");
}

/// Each row of the design's usage-error table, exit 2, before anything
/// loads; and the dotted name's command, followed as printed.
#[test]
fn usage_errors_are_refused_before_anything_loads() {
    let service = examples(&[("src/host.ts", "export class RunHost {\n  run() {}\n}\n")]);
    let refused = |args: &[&str], expected: &str| {
        let output = service.command().args(args).output().unwrap();
        diagnostic(&output, 2, &format!("{expected}\n"));
    };
    let project = ["--definition", "--project", "tsconfig.json"];
    let with = |args: &[&'static str]| [&project[..], args].concat();
    refused(
        &["--definition", "--name", "run"],
        "usage.missing_flag: --definition requires --project TSCONFIG",
    );
    for args in [
        with(&[]),
        with(&["--at", "src/app.ts:1:10", "--name", "run"]),
    ] {
        refused(
            &args,
            "usage.selection: --definition requires exactly one of --at or --name",
        );
    }
    refused(
        &with(&["--at", "1:10"]),
        "usage.operands: --definition --at needs a FILE operand or FILE:LINE:COLUMN",
    );
    refused(
        &with(&["--at", "src/app.ts:1:10", "src/app.ts"]),
        "usage.operands: --at FILE:LINE:COLUMN names the file; omit the FILE operand",
    );
    refused(
        &with(&["--at", "1:x", "src/app.ts"]),
        "usage.invalid_value: --at expects LINE:COLUMN or FILE:LINE:COLUMN, got 1:x",
    );
    // FILE `-` is stdin wherever the request names it: as the operand, or
    // as the file of `--at FILE:LINE:COLUMN`.
    for args in [
        with(&["--name", "run", "-"]),
        with(&["--at", "1:1", "-"]),
        with(&["--at", "-:1:1"]),
    ] {
        refused(
            &args,
            "usage.operands: --definition reads files from the project, not stdin",
        );
        // A usage failure's document goes to stdout, never `-o`.
        let mut json = args.clone();
        json.extend(["--json", "-o", "stdin.json"]);
        let output = service.command().args(&json).output().unwrap();
        assert_eq!(
            document(&output, 2),
            json!({
                "operation": "definition",
                "outcome": "error",
                "error": {
                    "category": "usage",
                    "code": "usage.operands",
                    "message": "--definition reads files from the project, not stdin",
                },
            }),
            "{json:?}"
        );
        assert!(!service.fixture.path("stdin.json").exists(), "{json:?}");
    }
    refused(
        &with(&["--name", "run", "a.ts", "b.ts"]),
        "usage.operands: Expected one input file",
    );
    refused(
        &with(&["--name", "run", "--pretty"]),
        "usage.flag_scope: --pretty applies to --definition only with --json",
    );
    refused(
        &with(&["--at", "src/app.ts:1:10", "--at", "src/app.ts:1:11"]),
        "usage.repeated_flag: --definition takes --at at most once",
    );
    refused(
        &with(&["--name", "run", "--name", "page"]),
        "usage.repeated_flag: --definition takes --name at most once",
    );
    for (flag, value) in [
        ("--limit", Some("all")),
        ("--calls", None),
        ("--file", Some("src/app.ts")),
        ("--depth", Some("2")),
        ("--expand", None),
        ("--tsx", None),
    ] {
        let mut args = with(&["--name", "run", flag]);
        args.extend(value);
        let output = service.command().args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(
            stderr(&output).starts_with(&format!("usage.flag_scope: {flag} applies to: ")),
            "{output:?}"
        );
        assert!(!stderr(&output).contains("--definition"), "{output:?}");
    }
    refused(
        &with(&["--name", "run", "--references"]),
        "usage.operation: Choose one operation: --definition, --references",
    );
    // A dotted name, refused with the --show command that selects it, which
    // keeps the request's --project spelling and drops FILE and --direct.
    let output = service
        .command()
        .args([
            "--definition",
            "--project",
            "./tsconfig.json",
            "--name",
            "RunHost.run",
            "src/app.ts",
            "--direct",
        ])
        .output()
        .unwrap();
    diagnostic(
        &output,
        2,
        "usage.invalid_value: --definition --name takes one identifier, not RunHost.run: a dotted name selects a member by containment, which --definition does not follow\n  next: tsparse --show --project ./tsconfig.json --name RunHost.run\n",
    );
    let next = stderr(&output);
    // With --json, the usage failure is a document on stdout, never -o.
    let output = service
        .command()
        .args(with(&[
            "--name",
            "RunHost.run",
            "--json",
            "-o",
            "usage.json",
        ]))
        .output()
        .unwrap();
    assert_eq!(
        document(&output, 2),
        json!({
            "operation": "definition",
            "outcome": "error",
            "error": {
                "category": "usage",
                "code": "usage.invalid_value",
                "message": "--definition --name takes one identifier, not RunHost.run: a dotted name selects a member by containment, which --definition does not follow",
                "next": {"argv": ["tsparse", "--show", "--project", "tsconfig.json", "--name", "RunHost.run"]},
            },
        })
    );
    assert!(!service.fixture.path("usage.json").exists());
    let output = service.command().args(with(&["--json"])).output().unwrap();
    assert_eq!(document(&output, 2)["error"]["code"], "usage.selection");
    // No usage error started a service.
    assert!(!service.runtime.exists());
    let _stop = Spelled(&service, "./tsconfig.json");
    let next = next
        .lines()
        .find_map(|line| line.strip_prefix("  next: "))
        .unwrap();
    let followed = follow(&service, next);
    assert_eq!(followed.status.code(), Some(0), "{followed:?}");
    assert!(stdout(&followed).contains("run() {}"), "{followed:?}");
}

/// Each failure keeps its kind, channel, and exit code.
#[test]
fn failures_keep_their_kind_channel_and_exit_code() {
    let service = examples(&[
        (
            "src/kw.ts",
            "export function loop() {\n  outer: for (;;) {\n    break outer;\n  }\n}\n",
        ),
        (
            "src/nocheck.ts",
            "// @ts-nocheck\nimport { gone } from \"./gone-module\";\nexport const g2 = gone;\n",
        ),
        ("other/outside.ts", "export const outside = 1;\n"),
    ]);
    // UnsupportedTarget and UnresolvedTarget: located diagnostics, exit 1.
    let output = definition(&service, &["--at", "src/kw.ts:3:11"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/src/kw.ts:3:11: `outer` is not a supported reference target (label)\n",
        ),
    );
    let output = definition(&service, &["--at", "src/kw.ts:3:11", "--json"]);
    let failure = document(&output, 1);
    assert_eq!(failure["outcome"], "error");
    assert_eq!(failure["operation"], "definition");
    assert_eq!(failure["error"]["kind"], "UnsupportedTarget");
    let output = definition(&service, &["--at", "src/nocheck.ts:3:19"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/src/nocheck.ts:3:19: `gone`: module is not resolved\n",
        ),
    );
    let output = definition(&service, &["--name", "gone", "--json"]);
    assert_eq!(document(&output, 1)["error"]["kind"], "UnresolvedTarget");
    // OutsideProject and InvalidPosition name the operand as spelled, exit 2.
    let output = definition(&service, &["--at", "other/outside.ts:1:14"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        stderr(&output).starts_with("other/outside.ts")
            && stderr(&output).ends_with(": not one of the project's searched files; the files, include, and exclude of tsconfig.json, or of a configuration it extends, decide which files are searched\n"),
        "{output:?}"
    );
    let output = definition(
        &service,
        &["--name", "outside", "other/outside.ts", "--json"],
    );
    assert_eq!(document(&output, 2)["error"]["kind"], "OutsideProject");
    let output = definition(&service, &["--at", "src/app.ts:99:1"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        stderr(&output).starts_with("src/app.ts:99:1: "),
        "{output:?}"
    );
    let output = definition(&service, &["--at", "src/app.ts:99:1", "--json"]);
    assert_eq!(document(&output, 2)["error"]["kind"], "InvalidPosition");
    // A result that cannot be written is one diagnostic, never a document.
    std::fs::create_dir(service.fixture.path("out")).unwrap();
    let output = definition(
        &service,
        &["--at", "src/app.ts:1:10", "--json", "-o", "out"],
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        stderr(&output).starts_with("write.output: out: "),
        "{output:?}"
    );
}

/// The load fails before anything is selected (K3): an unresolved module
/// in a file TypeScript checks.
#[test]
fn an_unresolved_import_fails_the_load() {
    let service = examples(&[(
        "src/missing.ts",
        "import { missing } from \"./missing-module\";\nexport const m = missing;\n",
    )]);
    let output = definition(&service, &["--at", "src/app.ts:1:10"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/src/missing.ts:1:25: cannot resolve module \"./missing-module\"\n",
        ),
    );
    let output = definition(&service, &["--at", "src/app.ts:1:10", "--json"]);
    assert_eq!(document(&output, 1)["error"]["kind"], "UnresolvedImport");
}

/// A member whose selection fails answers that failure, naming no search.
#[test]
fn a_member_whose_selection_fails_names_no_search() {
    let service = examples(&[(
        "src/any.ts",
        "declare const x: any;\nexport const y = x.foo;\n",
    )]);
    let output = definition(&service, &["--at", "src/any.ts:2:20"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/src/any.ts:2:20: `foo` has no reference symbol at this site\n",
        ),
    );
    let output = definition(&service, &["--at", "src/any.ts:2:20", "--json"]);
    let failure = document(&output, 1);
    assert_eq!(failure["error"]["kind"], "UnresolvedTarget");
    assert!(failure["error"].get("next").is_none(), "{failure}");
}

/// The Examples' configuration with JSX, for the probe's `jsx.tsx`.
const JSX_CONFIG: &str = r#"{
  "compilerOptions": {
    "strict": true,
    "target": "es2022",
    "module": "esnext",
    "moduleResolution": "bundler",
    "jsx": "preserve"
  },
  "include": ["src"]
}
"#;

/// `args` with `--references` instead of `--definition`, from the service
/// and directly.
fn references(service: &Service, args: &[&str]) -> Output {
    service.both(&[&["--references"], args].concat())
}

/// A failure while selecting a member is the answer, delivered as every
/// analysis failure is, from the service and directly alike: its kind,
/// reason, and causes in the document, its location and message on stderr,
/// and its exit code. It is the failure `--references --at` reports at the
/// same position, which selects the member the same way.
#[test]
fn semantic_failures_while_selecting_keep_their_kind_reason_and_channel() {
    let service = Service::of(Fixture::new(&[
        ("tsconfig.json", JSX_CONFIG),
        (
            "src/jsx.tsx",
            "function Item(props: { label: string }) {\n  return null as any;\n}\nexport const item = <Item label=\"x\" />;\nexport const link = <svg xlink:href=\"#x\" />;\n",
        ),
        (
            "src/broken.ts",
            "export {};\ndeclare const broken: MissingReceiver;\nexport const a = broken.probe;\n",
        ),
    ]));
    let same_as_references = |at: &str, code: i32| {
        let failure = document(&definition(&service, &["--at", at, "--json"]), code);
        assert_eq!(failure["operation"], "definition", "{failure}");
        assert_eq!(failure["outcome"], "error", "{failure}");
        assert_eq!(failure["project"], "tsconfig.json", "{failure}");
        let searched = document(&references(&service, &["--at", at, "--json"]), code);
        assert_eq!(failure["error"], searched["error"], "{at}");
        failure
    };
    // A JSX attribute: UnsupportedSemantic, reason jsx_typing_required.
    let output = definition(&service, &["--at", "src/jsx.tsx:4:27"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/src/jsx.tsx:4:27: a JSX attribute relates to its props member only through deferred JSX attribute typing\n",
        ),
    );
    let failure = same_as_references("src/jsx.tsx:4:27", 1);
    assert_eq!(failure["error"]["kind"], "UnsupportedSemantic");
    assert_eq!(failure["error"]["reason"], "jsx_typing_required");
    assert_eq!(
        failure["error"]["path"],
        work(&service, "/work/src/jsx.tsx")
    );
    assert_eq!(
        failure["error"]["position"],
        json!({"line": 4, "column": 27})
    );
    // A member of a receiver whose type does not resolve: InvalidSemantic,
    // located at the dependency, with the causes it consumed.
    let failure = same_as_references("src/broken.ts:3:25", 1);
    assert_eq!(failure["error"]["kind"], "InvalidSemantic");
    assert_eq!(failure["error"]["reason"], "unresolved_symbol");
    assert_eq!(
        failure["error"]["position"],
        json!({"line": 2, "column": 23})
    );
    let causes = failure["error"]["causes"].as_array().unwrap();
    assert!(!causes.is_empty(), "{failure}");
    assert_eq!(
        causes[0]["location"],
        json!({
            "file": "src/broken.ts",
            "start": {"line": 2, "column": 23},
            "end": {"line": 2, "column": 38},
        })
    );
    let output = definition(&service, &["--at", "src/broken.ts:3:25"]);
    diagnostic(
        &output,
        1,
        &format!(
            "{}: {}\n",
            work(&service, "/work/src/broken.ts:2:23"),
            failure["error"]["message"].as_str().unwrap()
        ),
    );
    // Every character of a JSX namespaced attribute, the colon included:
    // Internal today, exit 2, with no location.
    for at in ["src/jsx.tsx:5:26", "src/jsx.tsx:5:31", "src/jsx.tsx:5:32"] {
        let output = definition(&service, &["--at", at]);
        diagnostic(
            &output,
            2,
            "canonical semantic declaration association is not retained\n",
        );
        let failure = same_as_references(at, 2);
        assert_eq!(
            failure["error"],
            json!({
                "kind": "Internal",
                "reason": "internal_failure",
                "message": "canonical semantic declaration association is not retained",
                "path": null,
                "position": null,
            }),
            "{at}"
        );
    }
}

/// `args` for `--definition` with a one-millisecond `TSPARSE_QUERY_TIMEOUT_MS`
/// and no memory limit, from the service unless `direct`.
fn timed_out(service: &Service, direct: bool, args: &[&str]) -> Output {
    let mut command = service.command();
    command
        .args(["--definition", "--project", "tsconfig.json"])
        .args(args)
        .env_remove("TSPARSE_MAX_MEMORY_MB")
        .env("TSPARSE_QUERY_TIMEOUT_MS", "1");
    if direct {
        command.arg("--direct");
    }
    command.output().unwrap()
}

/// What a limit's failure says it cost in each context.
const DIRECT: &str = "; this query process exited";
const SERVICE: &str = " in the project service; that service and its cache exited, and the next request starts a replacement";

/// A time limit stops a definition as it stops every query: the message
/// names the variable and what the limit cost there, and the remedy says
/// what to change; text on stderr, the document on stdout or `-o`, exit 1.
/// A service the limit stops exits, and the next request starts a
/// replacement. `tests/process_limits.rs` pins the limits' mechanics, the
/// memory limit included, for every query; this pins their delivery as a
/// definition's.
#[test]
fn a_time_limit_names_its_variable_and_remedy_from_the_service_and_directly() {
    // Loading a thousand declarations outlasts a one-millisecond deadline.
    let mut padding = String::new();
    for i in 0..1000 {
        padding.push_str(&format!("const padding{i} = {i};\n"));
    }
    let service = examples(&[("src/padding.ts", &padding)]);
    let at = ["--at", "src/app.ts:1:10"];
    let next = "  next: set a higher TSPARSE_QUERY_TIMEOUT_MS before retrying";
    for (direct, cost) in [(true, DIRECT), (false, SERVICE)] {
        let message = format!("query exceeded TSPARSE_QUERY_TIMEOUT_MS (1 ms){cost}");
        let output = timed_out(&service, direct, &at);
        diagnostic(&output, 1, &format!("{message}\n{next}\n"));
        let json = [&at[..], &["--json"]].concat();
        let failure = document(&timed_out(&service, direct, &json), 1);
        assert_eq!(failure["operation"], "definition", "{failure}");
        assert_eq!(failure["outcome"], "error", "{failure}");
        assert_eq!(failure["error"]["kind"], "SemanticDeadline", "{failure}");
        assert_eq!(failure["error"]["reason"], "semantic_deadline", "{failure}");
        assert_eq!(failure["error"]["message"], message.as_str());
        assert_eq!(
            failure["error"]["remedy"],
            "set a higher TSPARSE_QUERY_TIMEOUT_MS before retrying"
        );
        assert!(failure["error"].get("next").is_none(), "{failure}");
        if !direct {
            assert_eq!(service.status()["running"], false);
        }
        // The document goes to -o, and nothing to stdout or stderr.
        let written = [&json[..], &["-o", "limited.json"]].concat();
        let output = timed_out(&service, direct, &written);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(
            output.stdout.is_empty() && output.stderr.is_empty(),
            "{output:?}"
        );
        let document: Value = serde_json::from_str(
            &std::fs::read_to_string(service.fixture.path("limited.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(document, failure);
        std::fs::remove_file(service.fixture.path("limited.json")).unwrap();
    }
    // Without the limit, the service starts again and answers.
    result(&definition(&service, &at), &work(&service, FORMAT_TITLE));
    assert_eq!(service.status()["running"], true);
}
