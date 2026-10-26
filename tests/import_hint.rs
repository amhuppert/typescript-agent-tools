//! The import hint of a `--show --name NAME FILE` miss
//! (`docs/definition-and-import-hints-design.md`, "The import hint"): each
//! import or export specifier of FILE that spells NAME without declaring it
//! is named, in source order, with the `--definition --at` command that
//! finds its declaration: complete in project mode, with the request's
//! `--project` spelling and no `--direct`, and with the `TSCONFIG`
//! placeholder in file mode. Outputs are the design's, with `/work` for the
//! project's directory; project answers are compared between the service
//! and `--direct`, byte for byte, and every printed project command is
//! followed as printed. Followed definitions are TypeScript 7.0.2's answers
//! from the design's Cases (rows 2, 4 and 10).
mod support;

use serde_json::{Value, json};
use std::process::Output;
use support::Fixture;
use support::service::Service;

/// The design's Examples configuration.
const EXAMPLES_CONFIG: &str = r#"{
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
        ("tsconfig.json", EXAMPLES_CONFIG),
        ("src/lib.ts", LIB),
        ("src/index.ts", INDEX),
        ("src/app.ts", APP),
    ];
    files.extend_from_slice(extra);
    Service::of(Fixture::new(&files))
}

/// The probe's configuration, whose CommonJS modules accept `import x =
/// require()` and `export =`.
const FORMS_CONFIG: &str = r#"{
  "compilerOptions": {
    "strict": true,
    "target": "es2022",
    "module": "nodenext",
    "moduleResolution": "nodenext"
  },
  "include": ["src"]
}
"#;

/// A project with one file per specifier form of the design's table.
fn forms() -> Service {
    Service::of(Fixture::new(&[
        ("tsconfig.json", FORMS_CONFIG),
        (
            "src/lib.ts",
            "export function formatTitle(title: string): string {\n  return title.trim();\n}\nexport interface Shape {\n  n: number;\n}\n",
        ),
        ("src/def.ts", "export default function makeThing() {}\n"),
        ("src/cafe.ts", "export const café = 1;\n"),
        (
            "src/strings.ts",
            "const fg = 1;\nexport { fg as \"a-b\" };\n",
        ),
        ("src/eq.ts", "const config = { a: 1 };\nexport = config;\n"),
        // Matching forms.
        (
            "src/default.ts",
            "import makeThing from \"./def\";\nmakeThing();\n",
        ),
        (
            "src/named.ts",
            "import { formatTitle } from \"./lib\";\nexport const t = formatTitle(\" x \");\n",
        ),
        (
            "src/type_only.ts",
            "import type { Shape } from \"./lib\";\nexport const s: Shape = { n: 1 };\n",
        ),
        (
            "src/type_specifier.ts",
            "import { type Shape } from \"./lib\";\nexport const s: Shape = { n: 1 };\n",
        ),
        (
            "src/reexport.ts",
            "export { formatTitle } from \"./lib\";\n",
        ),
        (
            "src/escaped.ts",
            "import { caf\\u00e9 } from \"./cafe\";\nexport const c = café;\n",
        ),
        // A name both imported and re-exported: two specifiers.
        (
            "src/local_export.ts",
            "import makeThing from \"./def\";\nexport { makeThing };\n",
        ),
        // `export default x` and `export = x` have no specifier entry
        // spelling `x`; only the import matches.
        (
            "src/export_default.ts",
            "import { formatTitle } from \"./lib\";\nexport default formatTitle;\n",
        ),
        (
            "src/export_equals.ts",
            "import { formatTitle } from \"./lib\";\nexport = formatTitle;\n",
        ),
        // Non-matching forms.
        (
            "src/renamed_import.ts",
            "import { formatTitle as fmt } from \"./lib\";\nexport const t = fmt(\" x \");\n",
        ),
        (
            "src/renamed_export.ts",
            "export { formatTitle as renamed } from \"./lib\";\n",
        ),
        (
            "src/namespace.ts",
            "import * as lib from \"./lib\";\nexport const t = lib.formatTitle(\" x \");\n",
        ),
        (
            "src/equals.ts",
            "import eq = require(\"./eq\");\nexport const t = eq.a;\n",
        ),
        ("src/star_as.ts", "export * as ns from \"./lib\";\n"),
        ("src/star.ts", "export * from \"./lib\";\n"),
        (
            "src/string_name.ts",
            "export { \"a-b\" } from \"./strings\";\n",
        ),
    ]))
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

/// `--show ARGS` in file mode, from the project's directory.
fn file_mode(service: &Service, args: &[&str]) -> Output {
    service.command().arg("--show").args(args).output().unwrap()
}

/// `--show ARGS --project tsconfig.json`, from the service and directly,
/// which must agree byte for byte.
fn project_mode(service: &Service, args: &[&str]) -> Output {
    service.both(&[&["--show"], args].concat())
}

/// The command a hint line prints.
fn printed(line: &str) -> &str {
    line.split_once("; find its declaration with: ")
        .unwrap_or_else(|| panic!("{line}"))
        .1
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

/// The definition `--definition --at FILE:LINE:COLUMN` prints for a symbol
/// with one declaration.
fn one_declaration(at: &str, name: &str, declaration: &str) -> String {
    format!(
        "/work/tsconfig.json: at /work/src/{at}: 1 group, 1 declaration\ngroup 1: {name}\n  /work/src/{declaration}\n"
    )
}

/// Follow each project-mode hint line of `diagnostic` and expect `answers`,
/// the definitions TypeScript gives at each specifier, in order.
fn follow_each(service: &Service, diagnostic: &str, answers: &[String]) {
    let hints: Vec<&str> = diagnostic.lines().skip(1).collect();
    assert_eq!(hints.len(), answers.len(), "{diagnostic}");
    for (hint, answer) in hints.iter().zip(answers) {
        let command = printed(hint);
        assert!(!command.contains("--direct"), "{command}");
        let followed = follow(service, command);
        assert_eq!(followed.status.code(), Some(0), "{command}: {followed:?}");
        assert_eq!(stderr(&followed), "", "{command}");
        assert_eq!(stdout(&followed), work(service, answer), "{command}");
    }
}

/// The design's Examples, in project mode: the import and the re-export
/// are named with a complete command, which prints the original
/// declaration.
#[test]
fn a_project_miss_names_the_import_and_the_command_that_finds_its_declaration() {
    let service = examples(&[]);
    let output = project_mode(&service, &["--name", "formatTitle", "src/app.ts"]);
    let expected = work(
        &service,
        "/work/src/app.ts: no declaration matches `formatTitle`
`formatTitle` is imported at /work/src/app.ts:1:10; find its declaration with: tsparse --definition --project tsconfig.json --at /work/src/app.ts:1:10
",
    );
    diagnostic(&output, 1, &expected);
    follow_each(
        &service,
        &expected,
        &[one_declaration("app.ts:1:10", "formatTitle", "lib.ts:1:17")],
    );
    let output = project_mode(&service, &["--name", "formatTitle", "src/app.ts", "--json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(stderr(&output), "");
    assert_eq!(
        stdout(&output),
        work(
            &service,
            "{\"file\":\"src/app.ts\",\"operation\":\"show\",\"outcome\":\"not_found\",\"project\":\"/work/tsconfig.json\",\"query\":{\"name\":\"formatTitle\"},\"specifiers\":[{\"anchor\":{\"column\":10,\"line\":1},\"kind\":\"import\",\"next\":{\"argv\":[\"tsparse\",\"--definition\",\"--project\",\"tsconfig.json\",\"--at\",\"/work/src/app.ts:1:10\"]}}]}\n"
        )
    );
    // The document's `next` is the printed command.
    let next: Vec<String> =
        serde_json::from_value(document(&output, 1)["specifiers"][0]["next"]["argv"].clone())
            .unwrap();
    assert_eq!(
        next.join(" "),
        printed(expected.lines().nth(1).unwrap()),
        "{next:?}"
    );

    // On the barrel.
    let output = project_mode(&service, &["--name", "formatTitle", "src/index.ts"]);
    let expected = work(
        &service,
        "/work/src/index.ts: no declaration matches `formatTitle`
`formatTitle` is re-exported at /work/src/index.ts:1:10; find its declaration with: tsparse --definition --project tsconfig.json --at /work/src/index.ts:1:10
",
    );
    diagnostic(&output, 1, &expected);
    follow_each(
        &service,
        &expected,
        &[one_declaration(
            "index.ts:1:10",
            "formatTitle",
            "lib.ts:1:17",
        )],
    );
    let answer = document(
        &project_mode(
            &service,
            &["--name", "formatTitle", "src/index.ts", "--json"],
        ),
        1,
    );
    assert_eq!(answer["specifiers"][0]["kind"], "export");
}

/// In file mode tsparse knows no configuration: the command names the
/// `TSCONFIG` placeholder, and the document has no `next`.
#[test]
fn a_file_miss_names_the_import_with_the_tsconfig_placeholder() {
    let service = examples(&[]);
    let output = file_mode(&service, &["--name", "formatTitle", "src/app.ts"]);
    let expected = work(
        &service,
        "/work/src/app.ts: no declaration matches `formatTitle`
`formatTitle` is imported at /work/src/app.ts:1:10; find its declaration with: tsparse --definition --project TSCONFIG --at /work/src/app.ts:1:10
",
    );
    diagnostic(&output, 1, &expected);
    let output = file_mode(&service, &["--name", "formatTitle", "src/app.ts", "--json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(stderr(&output), "");
    assert_eq!(
        stdout(&output),
        work(
            &service,
            "{\"file\":\"/work/src/app.ts\",\"operation\":\"show\",\"outcome\":\"not_found\",\"query\":{\"name\":\"formatTitle\"},\"specifiers\":[{\"anchor\":{\"column\":10,\"line\":1},\"kind\":\"import\"}]}\n"
        )
    );
    // With a configuration in place of the placeholder, the position is
    // the one a project-mode hint names.
    let completed = printed(expected.lines().nth(1).unwrap()).replace("TSCONFIG", "tsconfig.json");
    let followed = follow(&service, &completed);
    assert_eq!(followed.status.code(), Some(0), "{followed:?}");
    assert_eq!(
        stdout(&followed),
        work(
            &service,
            &one_declaration("app.ts:1:10", "formatTitle", "lib.ts:1:17")
        )
    );
    // The barrel's re-export.
    let output = file_mode(&service, &["--name", "formatTitle", "src/index.ts"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/src/index.ts: no declaration matches `formatTitle`
`formatTitle` is re-exported at /work/src/index.ts:1:10; find its declaration with: tsparse --definition --project TSCONFIG --at /work/src/index.ts:1:10
",
        ),
    );
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

/// The command keeps the request's own `--project` spelling, quoted where
/// a shell needs it, and names no transport.
#[test]
fn the_command_keeps_the_project_spelling() {
    let service = examples(&[(
        "my dir/tsconfig.json",
        "{ \"extends\": \"../tsconfig.json\", \"include\": [\"../src\"] }\n",
    )]);
    let spelling = "./my dir/tsconfig.json";
    let output = service
        .command()
        .args([
            "--show",
            "--project",
            spelling,
            "--name",
            "formatTitle",
            "src/app.ts",
            "--direct",
        ])
        .output()
        .unwrap();
    let expected = work(
        &service,
        "/work/src/app.ts: no declaration matches `formatTitle`
`formatTitle` is imported at /work/src/app.ts:1:10; find its declaration with: tsparse --definition --project './my dir/tsconfig.json' --at /work/src/app.ts:1:10
",
    );
    diagnostic(&output, 1, &expected);
    let _stop = Spelled(&service, spelling);
    let followed = follow(&service, printed(expected.lines().nth(1).unwrap()));
    assert_eq!(followed.status.code(), Some(0), "{followed:?}");
    assert_eq!(
        stdout(&followed),
        work(
            &service,
            "/work/my dir/tsconfig.json: at /work/src/app.ts:1:10: 1 group, 1 declaration\ngroup 1: formatTitle\n  /work/src/lib.ts:1:17\n"
        )
    );
}

/// A matching specifier: the verb its line uses, its `LINE:COLUMN`, and its
/// kind.
type Hint = (&'static str, &'static str, &'static str);

/// A definition with one declaration: its name and `FILE:LINE:COLUMN` in
/// `src`.
type Declared = (&'static str, &'static str);

/// Each matching form of the design's table, in both modes; following each
/// project command prints the declaration TypeScript gives there.
#[test]
fn each_matching_form_is_named_in_both_modes() {
    let service = forms();
    // (FILE, NAME, each specifier as (verb, LINE:COLUMN, kind), and the
    // definition at each.)
    let cases: &[(&str, &str, &[Hint], &[Declared])] = &[
        (
            "default.ts",
            "makeThing",
            &[("imported", "1:8", "import")],
            &[("makeThing", "def.ts:1:25")],
        ),
        (
            "named.ts",
            "formatTitle",
            &[("imported", "1:10", "import")],
            &[("formatTitle", "lib.ts:1:17")],
        ),
        (
            "type_only.ts",
            "Shape",
            &[("imported", "1:15", "import")],
            &[("Shape", "lib.ts:4:18")],
        ),
        (
            "type_specifier.ts",
            "Shape",
            &[("imported", "1:15", "import")],
            &[("Shape", "lib.ts:4:18")],
        ),
        (
            "reexport.ts",
            "formatTitle",
            &[("re-exported", "1:10", "export")],
            &[("formatTitle", "lib.ts:1:17")],
        ),
        (
            "escaped.ts",
            "café",
            &[("imported", "1:10", "import")],
            &[("café", "cafe.ts:1:14")],
        ),
        (
            "local_export.ts",
            "makeThing",
            &[
                ("imported", "1:8", "import"),
                ("re-exported", "2:10", "export"),
            ],
            &[("makeThing", "def.ts:1:25"), ("makeThing", "def.ts:1:25")],
        ),
        (
            "export_default.ts",
            "formatTitle",
            &[("imported", "1:10", "import")],
            &[("formatTitle", "lib.ts:1:17")],
        ),
        (
            "export_equals.ts",
            "formatTitle",
            &[("imported", "1:10", "import")],
            &[("formatTitle", "lib.ts:1:17")],
        ),
    ];
    for &(file, name, specifiers, answers) in cases {
        let operand = format!("src/{file}");
        for (config, mode) in [("tsconfig.json", true), ("TSCONFIG", false)] {
            let mut expected = format!("/work/src/{file}: no declaration matches `{name}`\n");
            for (verb, at, _) in specifiers {
                expected.push_str(&format!(
                    "`{name}` is {verb} at /work/src/{file}:{at}; find its declaration with: tsparse --definition --project {config} --at /work/src/{file}:{at}\n"
                ));
            }
            let expected = work(&service, &expected);
            let run = |args: &[&str]| {
                if mode {
                    project_mode(&service, args)
                } else {
                    file_mode(&service, args)
                }
            };
            diagnostic(&run(&["--name", name, &operand]), 1, &expected);
            let answer = document(&run(&["--name", name, &operand, "--json"]), 1);
            let listed = answer["specifiers"].as_array().unwrap();
            assert_eq!(listed.len(), specifiers.len(), "{file} {answer}");
            for (listed, (_, at, kind)) in listed.iter().zip(specifiers) {
                let (line, column) = at.split_once(':').unwrap();
                let mut specifier = json!({
                    "anchor": {"line": line.parse::<u32>().unwrap(), "column": column.parse::<u32>().unwrap()},
                    "kind": kind,
                });
                if mode {
                    specifier["next"] = json!({"argv": [
                        "tsparse", "--definition", "--project", "tsconfig.json", "--at",
                        work(&service, &format!("/work/src/{file}:{at}")),
                    ]});
                }
                assert_eq!(listed, &specifier, "{file}");
            }
            if mode {
                let answers: Vec<String> = specifiers
                    .iter()
                    .zip(answers)
                    .map(|((_, at, _), (name, declaration))| {
                        one_declaration(&format!("{file}:{at}"), name, declaration)
                    })
                    .collect();
                follow_each(&service, &expected, &answers);
            }
        }
    }
}

/// The not-found answer of a miss that names no specifier: today's, byte
/// for byte, in both modes and both forms.
fn unchanged_miss(service: &Service, file: &str, name: &str) {
    let operand = format!("src/{file}");
    let output = project_mode(service, &["--name", name, &operand]);
    diagnostic(
        &output,
        1,
        &work(
            service,
            &format!("/work/src/{file}: no declaration matches `{name}`\n"),
        ),
    );
    let output = project_mode(service, &["--name", name, &operand, "--json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        stdout(&output),
        work(
            service,
            &format!(
                "{{\"file\":\"src/{file}\",\"operation\":\"show\",\"outcome\":\"not_found\",\"project\":\"/work/tsconfig.json\",\"query\":{{\"name\":{}}}}}\n",
                json!(name)
            )
        )
    );
    let output = file_mode(service, &["--name", name, &operand]);
    diagnostic(
        &output,
        1,
        &work(
            service,
            &format!("/work/src/{file}: no declaration matches `{name}`\n"),
        ),
    );
    let output = file_mode(service, &["--name", name, &operand, "--json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        stdout(&output),
        work(
            service,
            &format!(
                "{{\"file\":\"/work/src/{file}\",\"operation\":\"show\",\"outcome\":\"not_found\",\"query\":{{\"name\":{}}}}}\n",
                json!(name)
            )
        )
    );
}

/// Each non-matching form of the design's table misses as it does today;
/// a renaming specifier, a namespace import, and `import x =` are found by
/// the name they declare.
#[test]
fn each_non_matching_form_misses_unchanged() {
    let service = forms();
    for (file, name) in [
        ("renamed_import.ts", "formatTitle"),
        ("renamed_export.ts", "formatTitle"),
        ("namespace.ts", "formatTitle"),
        ("equals.ts", "config"),
        ("star_as.ts", "ns"),
        ("star.ts", "formatTitle"),
        ("string_name.ts", "a-b"),
        // Exact names: case, and an escape that is not decoded.
        ("named.ts", "FormatTitle"),
        ("escaped.ts", "caf\\u00e9"),
        // A dotted name.
        ("named.ts", "formatTitle.x"),
    ] {
        unchanged_miss(&service, file, name);
    }
    for (file, name, kind) in [
        ("renamed_import.ts", "fmt", "import"),
        ("renamed_export.ts", "renamed", "export"),
        ("namespace.ts", "lib", "import"),
        ("equals.ts", "eq", "import"),
    ] {
        let output = project_mode(&service, &["--name", name, &format!("src/{file}")]);
        assert_eq!(output.status.code(), Some(0), "{file} {output:?}");
        assert!(
            stdout(&output).starts_with(&work(&service, &format!("/work/src/{file}:1:")))
                && stdout(&output)
                    .lines()
                    .next()
                    .unwrap()
                    .contains(&format!(" {kind} ")),
            "{file}: {output:?}"
        );
    }
}

/// A file whose only match is an undeclared `export { a };`: matching is by
/// syntax alone, and the printed command answers with the failure the
/// binding index has for that token, as `--references --at` does.
#[test]
fn an_undeclared_reexport_is_named_and_its_command_reports_the_failure() {
    let service = Service::of(Fixture::new(&[
        ("tsconfig.json", FORMS_CONFIG),
        ("src/lib.ts", LIB),
        ("src/undeclared.ts", "export { a };\n"),
    ]));
    let output = project_mode(&service, &["--name", "a", "src/undeclared.ts"]);
    let expected = work(
        &service,
        "/work/src/undeclared.ts: no declaration matches `a`
`a` is re-exported at /work/src/undeclared.ts:1:10; find its declaration with: tsparse --definition --project tsconfig.json --at /work/src/undeclared.ts:1:10
",
    );
    diagnostic(&output, 1, &expected);
    let followed = follow(&service, printed(expected.lines().nth(1).unwrap()));
    let references = follow(
        &service,
        "tsparse --references --project tsconfig.json --at src/undeclared.ts:1:10",
    );
    assert_eq!(followed.status.code(), Some(1), "{followed:?}");
    assert_eq!(stdout(&followed), "");
    assert!(stderr(&followed).contains("`a`: "), "{followed:?}");
    assert_eq!(references.status.code(), Some(1), "{references:?}");
    assert_eq!(stderr(&followed), stderr(&references));
    let output = file_mode(&service, &["--name", "a", "src/undeclared.ts"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/src/undeclared.ts: no declaration matches `a`
`a` is re-exported at /work/src/undeclared.ts:1:10; find its declaration with: tsparse --definition --project TSCONFIG --at /work/src/undeclared.ts:1:10
",
        ),
    );
}

/// A project-wide `--name` without FILE, a missing `--at`, and found and
/// ambiguous answers are unchanged.
#[test]
fn other_show_answers_are_unchanged() {
    let service = forms();
    // Without FILE the question is no longer which specifier of a file
    // spells the name.
    let output = project_mode(&service, &["--name", "ns"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/tsconfig.json: no declaration matches `ns`\n",
        ),
    );
    let answer = document(&project_mode(&service, &["--name", "ns", "--json"]), 1);
    assert_eq!(
        answer,
        json!({
            "operation": "show",
            "outcome": "not_found",
            "project": service.fixture.path("tsconfig.json"),
            "query": {"name": "ns"},
        })
    );
    // `export { a }`'s brace is no site and no statement's first token.
    let output = project_mode(&service, &["--at", "src/reexport.ts:1:8"]);
    diagnostic(
        &output,
        1,
        &work(
            &service,
            "/work/src/reexport.ts:1:8: no declaration is named or anchored at this position\n",
        ),
    );
    let output = file_mode(&service, &["--at", "1:8", "src/reexport.ts"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        !stderr(&output).contains("find its declaration"),
        "{output:?}"
    );
    // A found and an ambiguous name.
    let output = project_mode(&service, &["--name", "formatTitle", "src/lib.ts"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let output = project_mode(&service, &["--name", "t"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output)
            .contains("`t` matches 4 declarations; select one with --at FILE:LINE:COLUMN"),
        "{output:?}"
    );
    assert!(
        !stderr(&output).contains("find its declaration"),
        "{output:?}"
    );
}
