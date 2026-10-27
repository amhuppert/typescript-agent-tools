mod support;

use serde_json::Value;
use std::process::Command;
use support::project;

#[test]
fn calls_filter_keeps_direct_callees_and_declarations_without_value_propagation() {
    let fixture = project(&[(
        "calls.ts",
        concat!(
            "export function run<T>(x?: T) {}\n",
            "run();\n",
            "(run)();\n",
            "run!();\n",
            "(run as typeof run)();\n",
            "(<typeof run>run)();\n",
            "(run satisfies typeof run)();\n",
            "(run<string>)();\n",
            "run?.();\n",
            "run`tag`;\n",
            "new run;\n",
            "const f = run;\n",
            "f();\n",
            "run.call(null);\n",
            "run.apply(null);\n",
            "run.bind(null);\n",
            "run(run);\n",
            "run instanceof run;\n",
        ),
    )]);
    let result = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(["--references", "--direct", "--project"])
        .arg(fixture.path("tsconfig.json"))
        .args(["--name", "run", "--calls", "--json"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let document: Value = serde_json::from_slice(&result.stdout).unwrap();
    let group = &document["groups"][0];
    assert_eq!(group["declarations"].as_array().unwrap().len(), 1);
    let lines: Vec<_> = group["occurrences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|occurrence| occurrence["location"]["start"]["line"].as_u64().unwrap())
        .collect();
    assert_eq!(lines, [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 17]);
    assert_eq!(
        document["filters"],
        serde_json::json!({"calls": true, "excludeTests": false})
    );
}

#[test]
fn no_tests_is_an_exact_path_filter_and_does_not_remove_declarations() {
    let paths = [
        "tests/a.ts",
        "test/a.ts",
        "__tests__/a.ts",
        "a.test.ts",
        "a.spec.ts",
        "a.test.tsx",
        "a.spec.tsx",
        "a.test.mts",
        "a.spec.mts",
        "a.test.cts",
        "a.spec.cts",
        "fixtures/a.ts",
        "spec/a.ts",
        "case.TEST.ts",
        "contest/a.ts",
        "declaration.test.d.ts",
    ];
    let mut files = vec![(
        "decl.test.ts".to_owned(),
        "export function run() {}".to_owned(),
    )];
    for path in paths {
        let depth = path.matches('/').count();
        let source = format!(
            "import {{ run }} from \"{}decl.test\"; run();",
            "../".repeat(depth).to_owned() + if depth == 0 { "./" } else { "" }
        );
        files.push((path.to_owned(), source));
    }
    let borrowed: Vec<_> = files
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect();
    let fixture = project(&borrowed);
    let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(["--references", "--direct", "--project"])
        .arg(fixture.path("tsconfig.json"))
        .args(["--name", "run", "--calls", "--no-tests", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let group = &document["groups"][0];
    assert_eq!(group["declarations"][0]["location"]["file"], "decl.test.ts");
    let actual: Vec<_> = group["occurrences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|occurrence| occurrence["location"]["file"].as_str().unwrap())
        .collect();
    assert_eq!(
        actual,
        [
            "case.TEST.ts",
            "contest/a.ts",
            "declaration.test.d.ts",
            "fixtures/a.ts",
            "spec/a.ts"
        ]
    );
}

/// JavaScript inputs follow the same test-path policy, by their own
/// extensions.
#[test]
fn no_tests_filters_javascript_test_paths_too() {
    let fixture = support::Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "allowJs": true, "noEmit": true } }"#,
        ),
        ("decl.ts", "export function run() {}"),
        ("a.test.js", "import { run } from \"./decl\"; run();"),
        ("a.spec.jsx", "import { run } from \"./decl\"; run();"),
        ("b.test.mjs", "import { run } from \"./decl\"; run();"),
        ("c.spec.cjs", "import { run } from \"./decl\"; run();"),
        ("kept.js", "import { run } from \"./decl\"; run();"),
    ]);
    let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(["--references", "--direct", "--project"])
        .arg(fixture.path("tsconfig.json"))
        .args(["--name", "run", "--calls", "--no-tests", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let actual: Vec<_> = document["groups"][0]["occurrences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|occurrence| occurrence["location"]["file"].as_str().unwrap())
        .collect();
    assert_eq!(actual, ["kept.js"]);
}

#[test]
fn test_directory_matching_preserves_case_on_case_insensitive_filesystems_too() {
    let fixture = project(&[
        ("decl.ts", "export function run() {}"),
        ("Tests/case.ts", "import { run } from '../decl'; run();"),
    ]);
    let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(["--references", "--direct", "--project"])
        .arg(fixture.path("tsconfig.json"))
        .args(["--name", "run", "--calls", "--no-tests", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        document["groups"][0]["occurrences"][0]["location"]["file"],
        "Tests/case.ts"
    );
}

#[test]
fn namespace_receivers_and_implicit_decorators_are_not_callees() {
    let fixture = project(&[(
        "namespace.ts",
        concat!(
            "namespace ns { export function run() {} }\n",
            "ns.run();\n",
            "@ns.run class A {}\n",
            "@ns.run() class B {}\n",
            "const alias = ns.run;\n",
        ),
    )]);
    let query = |name: &str| {
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args(["--references", "--direct", "--project"])
            .arg(fixture.path("tsconfig.json"))
            .args(["--name", name, "--calls", "--json"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let run = query("run");
    let lines: Vec<_> = run["groups"][0]["occurrences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|occurrence| occurrence["location"]["start"]["line"].as_u64().unwrap())
        .collect();
    assert_eq!(lines, [2, 4]);
    let receiver = query("ns");
    assert_eq!(receiver["groups"][0]["occurrences"], serde_json::json!([]));
    assert_eq!(
        receiver["groups"][0]["declarations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn filters_require_reference_queries_and_are_rejected_on_service_administration() {
    for args in [
        vec!["--calls"],
        vec!["--no-tests", "--outline", "a.ts"],
        vec![
            "--references-status",
            "--project",
            "tsconfig.json",
            "--calls",
        ],
        vec![
            "--references-stop",
            "--project",
            "tsconfig.json",
            "--no-tests",
        ],
        vec!["--file", "a.ts", "--outline", "a.ts"],
        vec![
            "--references-status",
            "--project",
            "tsconfig.json",
            "--file",
            "a.ts",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn explicit_reference_json_delivers_analysis_errors_and_preserves_usage_outputs() {
    let fixture = project(&[("a.ts", "export function run() {}")]);
    let result_file = fixture.path("result.json");
    std::fs::write(&result_file, "sentinel").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(["--references", "--direct", "--project"])
        .arg(fixture.path("tsconfig.json"))
        .args(["--at", "missing.ts:1:1", "--json", "--calls", "-o"])
        .arg(&result_file)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value =
        serde_json::from_str(&std::fs::read_to_string(&result_file).unwrap()).unwrap();
    assert_eq!(document["operation"], "references");
    assert_eq!(document["outcome"], "error");
    assert_eq!(document["error"]["kind"], "OutsideProject");
    assert_eq!(document["error"]["path"], "missing.ts");

    std::fs::write(&result_file, "sentinel").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(["--references", "--json", "--calls", "-o"])
        .arg(&result_file)
        .output()
        .unwrap();
    // A usage failure's document goes to stdout, never to the -o the
    // invalid request named.
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty(), "{output:?}");
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        document,
        serde_json::json!({
            "operation": "references",
            "outcome": "error",
            "error": {
                "category": "usage",
                "code": "usage.missing_flag",
                "message": "--references requires --project TSCONFIG",
            },
        })
    );
    assert_eq!(std::fs::read_to_string(&result_file).unwrap(), "sentinel");
}

#[cfg(unix)]
#[test]
fn filtered_service_answers_refresh_and_never_fall_back_after_invalid_configuration() {
    let service = support::service::Service::new(&[
        ("decl.test.ts", "export function run() {}"),
        ("src/use.ts", "import { run } from '../decl.test'; run();"),
        (
            "tests/mock.ts",
            "import { run } from '../decl.test'; run();",
        ),
    ]);
    let arguments = [
        "--references",
        "--name",
        "run",
        "--calls",
        "--no-tests",
        "--json",
    ];
    let check = |count: usize| {
        let direct = service.run(&[&arguments[..], &["--direct"]].concat());
        let remote = service.run(&arguments);
        assert_eq!(remote.status.code(), direct.status.code());
        assert_eq!(remote.stdout, direct.stdout);
        assert_eq!(remote.stderr, direct.stderr);
        let result = support::service::json(remote);
        let group = &result["groups"][0];
        assert_eq!(group["declarations"][0]["location"]["file"], "decl.test.ts");
        let occurrences = group["occurrences"].as_array().unwrap();
        assert_eq!(occurrences.len(), count);
        assert!(
            occurrences
                .iter()
                .all(|occurrence| occurrence["location"]["file"] == "src/use.ts")
        );
    };
    check(1);
    assert_eq!(service.generation(), 1);
    std::fs::write(
        service.fixture.path("src/use.ts"),
        "import { run } from '../decl.test'; run(); run();",
    )
    .unwrap();
    check(2);
    assert_eq!(service.generation(), 2);

    let configuration = service.fixture.path("tsconfig.json");
    let original = std::fs::read(&configuration).unwrap();
    std::fs::write(
        &configuration,
        r#"{"compilerOptions":{"strict":"invalid"}}"#,
    )
    .unwrap();
    let failed = service.run(&arguments);
    assert_eq!(failed.status.code(), Some(1));
    assert!(failed.stderr.is_empty());
    let document: Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(document["outcome"], "error");
    assert_eq!(document["error"]["kind"], "Config");
    assert!(document.get("groups").is_none());

    std::fs::write(&configuration, original).unwrap();
    check(2);
    assert_eq!(service.generation(), 3);
}

/// Exit code, stdout, and stderr of one answer, the same bytes through the
/// service and loaded directly.
#[cfg(unix)]
fn both(service: &support::service::Service, args: &[&str]) -> (Option<i32>, String, String) {
    let output = service.both(args);
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// `--file FILE` reports only the occurrences in FILE, named as the FILE
/// operand names a file and compared as the project compares file names,
/// and echoes it in `filters` as locations name files. The groups are those
/// of `fixtures/references/aliases-shadowing`, which the semantic validation
/// checks against TypeScript 7.0.2.
#[cfg(unix)]
#[test]
fn file_reports_only_the_occurrences_in_that_file() {
    let service = support::service::Service::of(support::Fixture::copy_of("aliases-shadowing"));
    let r = &service.fixture.root.display().to_string();
    let only_app = format!(
        "\
{r}/tsconfig.json: `run` 1 group, 3 occurrences in 1 file; filtered by --file {r}/app.ts; coverage lexical, complete
group 1: run, declared at {r}/api.ts:2:28
  {r}/app.ts:2:20 import (execute)
  {r}/app.ts:2:41 import (start)
  {r}/app.ts:3:11 use (start)
"
    );
    let text = |file: &str| both(&service, &["--references", "--name", "run", "--file", file]);
    assert_eq!(text("app.ts"), (Some(0), only_app.clone(), String::new()));
    let (code, document, stderr) = both(
        &service,
        &[
            "--references",
            "--name",
            "run",
            "--file",
            "app.ts",
            "--json",
        ],
    );
    assert_eq!((code, stderr.as_str()), (Some(0), ""));
    let document: Value = serde_json::from_str(&document).unwrap();
    assert_eq!(
        document["filters"],
        serde_json::json!({"calls": false, "excludeTests": false, "file": "app.ts"})
    );
    let files: Vec<&str> = document["groups"][0]["occurrences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|occurrence| occurrence["location"]["file"].as_str().unwrap())
        .collect();
    assert_eq!(files, ["app.ts", "app.ts", "app.ts"]);
    // TypeScript writes `\` as `/` in every file name (`normalizeSlashes`).
    assert_eq!(text("./app.ts"), (Some(0), only_app.clone(), String::new()));
    assert_eq!(
        text(".\\app.ts"),
        (Some(0), only_app.clone(), String::new())
    );
    assert_eq!(
        text(&format!("{r}/app.ts")),
        (Some(0), only_app.clone(), String::new())
    );
    // Without the filter, the same group spans three files.
    let (_, unfiltered, _) = both(&service, &["--references", "--name", "run"]);
    assert!(
        unfiltered.contains("7 occurrences in 3 files"),
        "{unfiltered}"
    );
}

/// A different-case spelling names the same file only where the file system
/// ignores case, as the project's file names do (`Case::detect`).
#[cfg(unix)]
#[test]
fn file_compares_case_as_the_project_does() {
    // The base name differs in case: `.ts` is an input under either policy.
    let fixture = support::Fixture::copy_of("aliases-shadowing");
    let insensitive = fixture.path("App.ts").exists();
    if !insensitive {
        // A file the project searches beside `app.ts`.
        std::fs::write(
            fixture.path("App.ts"),
            "import { execute } from \"./barrel\";\nexecute();\n",
        )
        .unwrap();
    }
    let service = support::service::Service::of(fixture);
    let filtered = |file: &str| {
        both(
            &service,
            &["--references", "--name", "run", "--file", file, "--json"],
        )
    };
    let (code, app, stderr) = filtered("app.ts");
    assert_eq!((code, stderr.as_str()), (Some(0), ""), "{app}");
    let (code, other, stderr) = filtered("App.ts");
    assert_eq!((code, stderr.as_str()), (Some(0), ""), "{other}");
    let file = |document: &str| {
        let document: Value = serde_json::from_str(document).unwrap();
        document["filters"]["file"].clone()
    };
    if insensitive {
        // The project's one `app.ts`, echoed as it names it.
        assert_eq!(other, app);
        assert_eq!(file(&other), "app.ts");
        return;
    }
    let occurrences = |document: &str| -> Vec<String> {
        let document: Value = serde_json::from_str(document).unwrap();
        document["groups"][0]["occurrences"]
            .as_array()
            .unwrap()
            .iter()
            .map(|occurrence| {
                let location = &occurrence["location"];
                format!(
                    "{}:{}:{}",
                    location["file"].as_str().unwrap(),
                    location["start"]["line"],
                    location["start"]["column"]
                )
            })
            .collect()
    };
    assert_eq!(
        occurrences(&app),
        ["app.ts:2:20", "app.ts:2:41", "app.ts:3:11"]
    );
    assert_eq!(occurrences(&other), ["App.ts:1:10", "App.ts:2:1"]);
    assert_eq!(file(&app), "app.ts");
    assert_eq!(file(&other), "App.ts");
}

/// The FILE operand narrows the candidate declarations; `--file` then keeps
/// their occurrences in another file, and a group without one keeps its
/// declarations.
#[cfg(unix)]
#[test]
fn file_filters_the_occurrences_of_candidates_a_file_operand_narrows() {
    let service = support::service::Service::of(support::Fixture::copy_of("aliases-shadowing"));
    let r = &service.fixture.root.display().to_string();
    assert_eq!(
        both(
            &service,
            &["--references", "--name", "start", "app.ts", "--file", "barrel.ts"],
        ),
        (
            Some(0),
            format!(
                "\
{r}/tsconfig.json: `start` 2 groups, 2 occurrences in 1 file; candidates narrowed to {r}/app.ts; filtered by --file {r}/barrel.ts; coverage lexical, complete
group 1: run, declared at {r}/api.ts:2:28
  {r}/barrel.ts:1:20 export
  {r}/barrel.ts:1:37 export (execute)
group 2: start, declared at {r}/app.ts:4:40
"
            ),
            String::new()
        )
    );
}

/// `--file` names a searched file as the FILE operand does: another path is
/// a file outside the project, named as given, never an empty answer.
#[cfg(unix)]
#[test]
fn file_outside_the_project_is_a_failure() {
    let service = support::service::Service::of(support::Fixture::copy_of("aliases-shadowing"));
    assert_eq!(
        both(
            &service,
            &["--references", "--name", "run", "--file", "missing.ts"],
        ),
        (
            Some(2),
            String::new(),
            "missing.ts: not one of the project's searched files; the files, include, and exclude of tsconfig.json, or of a configuration it extends, decide which files are searched\n".into()
        )
    );
    let (code, document, stderr) = both(
        &service,
        &[
            "--references",
            "--name",
            "run",
            "--file",
            "missing.ts",
            "--json",
        ],
    );
    assert_eq!((code, stderr.as_str()), (Some(2), ""));
    let document: Value = serde_json::from_str(&document).unwrap();
    assert_eq!(
        document,
        serde_json::json!({
            "operation": "references",
            "outcome": "error",
            "project": "tsconfig.json",
            "error": {
                "kind": "OutsideProject",
                "message": "not one of the project's searched files; the files, include, and exclude of tsconfig.json, or of a configuration it extends, decide which files are searched",
                "path": "missing.ts",
                "position": null,
            },
        })
    );
}

/// A member search applies `--file` as it applies the other filters: it
/// establishes only the candidates in FILE, so a use elsewhere that it could
/// not establish is no gap of the filtered answer while one in FILE still is,
/// and it keeps the selected declaration when no occurrence is in FILE. The
/// declarations, occurrences, and unestablished candidate are those
/// `tools/reference-member-hints-oracle.test.mjs` pins to TypeScript 7.0.2.
#[cfg(unix)]
#[test]
fn file_filters_a_member_search_and_bounds_its_coverage() {
    let service = support::service::Service::of(support::Fixture::copy_from_repository(
        "fixtures/types/reference-file-filter",
    ));
    let r = &service.fixture.root.display().to_string();
    let gap = format!(
        "\
gap {r}/d.ts:3:17: error_type: native non-null checking consumes unknown-type error recovery
  cause {r}/d.ts:3:1: error_type: native non-null checking consumes unknown-type error recovery
"
    );
    let search = |args: &[&str]| {
        both(
            &service,
            &[&["--references", "--name", "Box.value"][..], args].concat(),
        )
    };
    let declared = format!("group 1: value, declared at {r}/lib.ts:1:24\n");
    assert_eq!(
        search(&[]),
        (
            Some(1),
            format!(
                "\
{r}/tsconfig.json: `Box.value` 1 group, 3 occurrences in 3 files; coverage member, partial
{declared}  {r}/a.ts:3:3 use
  {r}/b.ts:3:3 use
  {r}/lib.ts:1:24 declaration
{gap}"
            ),
            String::new()
        )
    );
    for file in ["a.ts", "b.ts"] {
        assert_eq!(
            search(&["--file", file]),
            (
                Some(0),
                format!(
                    "\
{r}/tsconfig.json: `Box.value` 1 group, 1 occurrence in 1 file; filtered by --file {r}/{file}; coverage member, complete
{declared}  {r}/{file}:3:3 use
"
                ),
                String::new()
            )
        );
    }
    assert_eq!(
        search(&["--file", "c.ts"]),
        (
            Some(0),
            format!(
                "\
{r}/tsconfig.json: `Box.value` 1 group, 0 occurrences in 0 files; filtered by --file {r}/c.ts; coverage member, complete
{declared}"
            ),
            String::new()
        )
    );
    assert_eq!(
        search(&["--file", "d.ts"]),
        (
            Some(1),
            format!(
                "\
{r}/tsconfig.json: `Box.value` 1 group, 0 occurrences in 0 files; filtered by --file {r}/d.ts; coverage member, partial
{declared}{gap}"
            ),
            String::new()
        )
    );
}

/// `--file -` names no stdin, as a `-` FILE operand does not.
#[test]
fn file_is_not_stdin() {
    let fixture = project(&[("a.ts", "export function run() {}")]);
    for args in [
        &["--name", "run", "--file", "-"][..],
        &["--name", "run", "-"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .current_dir(&fixture.root)
            .args(["--references", "--direct", "--project", "tsconfig.json"])
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "usage.operands: --references reads files from the project, not stdin\n",
            "{args:?}"
        );
    }
}

/// A file the program reads but does not search, such as an installed
/// package's declarations, or a file it does not read, such as the
/// configuration, is outside the project for `--file` as for a FILE
/// operand, and the FILE operand's failure comes first.
#[cfg(unix)]
#[test]
fn file_that_is_not_searched_is_a_failure() {
    let service = support::service::Service::new(&[
        (
            "node_modules/pkg/package.json",
            r#"{ "name": "pkg", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/pkg/index.d.ts",
            "export declare const value: number;\n",
        ),
        ("use.ts", "import { value } from \"pkg\";\nvalue;\n"),
    ]);
    let r = &service.fixture.root.display().to_string();
    // The package declares the group, so its location is printed.
    assert_eq!(
        both(&service, &["--references", "--name", "value"]),
        (
            Some(0),
            format!(
                "\
{r}/tsconfig.json: `value` 1 group, 2 occurrences in 1 file; coverage lexical, complete
group 1: value, declared at {r}/node_modules/pkg/index.d.ts:1:22
  {r}/use.ts:1:10 import
  {r}/use.ts:2:1 use
"
            ),
            String::new()
        )
    );
    for file in ["node_modules/pkg/index.d.ts", "tsconfig.json"] {
        assert_eq!(
            both(
                &service,
                &["--references", "--name", "value", "--file", file]
            ),
            (
                Some(2),
                String::new(),
                format!(
                    "{file}: not one of the project's searched files; the files, include, and exclude of tsconfig.json, or of a configuration it extends, decide which files are searched\n"
                )
            )
        );
    }
    assert_eq!(
        both(
            &service,
            &[
                "--references",
                "--name",
                "value",
                "missing.ts",
                "--file",
                "tsconfig.json",
            ],
        ),
        (
            Some(2),
            String::new(),
            "missing.ts: not one of the project's searched files; the files, include, and exclude of tsconfig.json, or of a configuration it extends, decide which files are searched\n".into()
        )
    );
}
