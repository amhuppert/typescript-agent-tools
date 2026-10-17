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
        .args(["--name", "run", "--calls"])
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
        .args(["--name", "run", "--calls", "--no-tests"])
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

#[test]
fn test_directory_matching_preserves_case_on_case_insensitive_filesystems_too() {
    let fixture = project(&[
        ("decl.ts", "export function run() {}"),
        ("Tests/case.ts", "import { run } from '../decl'; run();"),
    ]);
    let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(["--references", "--direct", "--project"])
        .arg(fixture.path("tsconfig.json"))
        .args(["--name", "run", "--calls", "--no-tests"])
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
            .args(["--name", name, "--calls"])
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
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
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
