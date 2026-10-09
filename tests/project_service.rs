//! Outline, show, and references through the project service: the same
//! bytes as direct answers on identical inputs, one generation per change
//! for every operation, no answer from an earlier generation after a failed
//! refresh, and clients in any working directory, one after another or at
//! once.
#![cfg(unix)]
mod support;

use serde_json::Value;
use std::path::Path;
use std::process::Output;
use support::service::{Service, json};

const CONFIG: &str = r#"{ "compilerOptions": { "noLib": true, "moduleResolution": "bundler", "module": "esnext" }, "include": ["src"] }"#;

/// A project with overloads and same-named declarations across files, and
/// an installed dependency.
fn service() -> Service {
    Service::of(support::Fixture::new(&[
        ("tsconfig.json", CONFIG),
        (
            "src/a.ts",
            "// 😀\r\nimport { helper } from \"dep\";\nexport function run(): void;\nexport function run(value?: number) {}\nexport const café = run();\u{2028}helper();\n",
        ),
        (
            "src/b.ts",
            "import { run } from \"./a\";\nexport namespace Tools {\n  export function run() {}\n}\nrun();\n",
        ),
        (
            "node_modules/dep/package.json",
            r#"{ "name": "dep", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/dep/index.d.ts",
            "export declare function helper(): void;\n",
        ),
        ("other/outside.ts", "export const outside = 1;\n"),
    ]))
}

/// Queries of every operation and outcome, as arguments after `--project`.
const QUERIES: &[&[&str]] = &[
    &["--outline", "src/a.ts"],
    &["--outline", "--json", "--depth", "all", "src/b.ts"],
    &["--outline", "--json", "--pretty", "other/outside.ts"],
    &["--outline", "src/missing.ts"],
    &["--show", "--name", "run"],
    &["--show", "--json", "--name", "run"],
    &["--show", "--json", "--name", "Tools.run"],
    &["--show", "--name", "café", "src/a.ts"],
    &["--show", "--at", "5:14", "src/a.ts"],
    &["--show", "--json", "--at", "5:14", "src/a.ts"],
    &["--show", "--json", "--at", "9:1", "src/a.ts"],
    &["--show", "--at", "1:99", "src/b.ts"],
    &["--show", "--json", "--name", "absent", "src/b.ts"],
    &["--references", "--name", "run", "--pretty"],
    &["--references", "--name", "absent"],
    &["--references", "--at", "5:21", "src/a.ts"],
    &["--references", "--at", "99:1", "src/a.ts"],
    &["--references", "--name", "run", "other/outside.ts"],
];

fn run(service: &Service, directory: &Path, project: &str, args: &[&str]) -> Output {
    service
        .command()
        .current_dir(directory)
        .args(args)
        .args(["--project", project])
        .output()
        .unwrap()
}

fn direct(service: &Service, args: &[&str]) -> Output {
    service.run(&[args, &["--direct"]].concat())
}

/// The same exit status and the same bytes on stdout and stderr.
fn same(left: &Output, right: &Output, context: &str) {
    assert_eq!(left.status.code(), right.status.code(), "{context}");
    for (left, right) in [(&left.stdout, &right.stdout), (&left.stderr, &right.stderr)] {
        assert!(
            left == right,
            "{context}:\n{}\n!=\n{}",
            String::from_utf8_lossy(left),
            String::from_utf8_lossy(right)
        );
    }
}

#[test]
fn service_answers_are_the_bytes_of_direct_answers_for_every_operation() {
    let service = service();
    let expected: Vec<Output> = QUERIES
        .iter()
        .map(|query| direct(&service, query))
        .collect();
    assert!(!service.runtime.exists(), "a direct query used the service");
    for (query, expected) in QUERIES.iter().zip(&expected) {
        same(&service.run(query), expected, &format!("{query:?}"));
        // The `--references-direct` spelling is the same request.
        if query[0] == "--references" {
            let mut spelled = query.to_vec();
            spelled[0] = "--references-direct";
            same(&service.run(&spelled), expected, &format!("{query:?}"));
        }
    }
    // Every outcome kind occurs above.
    let codes: Vec<Option<i32>> = expected.iter().map(|output| output.status.code()).collect();
    for code in [Some(0), Some(1), Some(2)] {
        assert!(codes.contains(&code), "{code:?} in {codes:?}");
    }
    assert_eq!(service.generation(), 1);

    // Output files receive the same bytes.
    for query in [
        &["--outline", "--json", "src/a.ts"][..],
        &["--show", "--name", "Tools.run"],
        &["--references", "--name", "run"],
    ] {
        let to_file = |direct: bool, name: &str| {
            let mut args = query.to_vec();
            args.extend(["-o", name]);
            if direct {
                args.push("--direct");
            }
            let output = service.run(&args);
            assert!(output.status.success(), "{query:?} {output:?}");
            assert!(output.stdout.is_empty() && output.stderr.is_empty());
            std::fs::read(service.fixture.path(name)).unwrap()
        };
        assert_eq!(to_file(false, "service.out"), to_file(true, "direct.out"));
    }
}

#[test]
fn clients_in_other_directories_share_the_service_and_its_answers() {
    let service = service();
    let config = service.fixture.path("tsconfig.json");
    let config = config.to_str().unwrap();
    let root = service.fixture.root.clone();
    let source = service.fixture.path("src");
    let elsewhere = service.fixture.path("other");
    let from_root: Vec<Output> = [
        &["--outline", "--json", "src/a.ts"][..],
        &["--show", "--at", "5:14", "src/a.ts"],
        &["--show", "--json", "--name", "run", "src/a.ts"],
        &["--references", "--at", "5:21", "src/a.ts"],
        &["--show", "--at", "1:99", "src/b.ts"],
    ]
    .iter()
    .map(|args| run(&service, &root, config, args))
    .collect();
    for (index, args) in [
        &["--outline", "--json", "a.ts"][..],
        &["--show", "--at", "5:14", "a.ts"],
        &["--show", "--json", "--name", "run", "a.ts"],
        &["--references", "--at", "5:21", "a.ts"],
    ]
    .iter()
    .enumerate()
    {
        same(
            &run(&service, &source, config, args),
            &from_root[index],
            &format!("{args:?} from src"),
        );
        // A direct answer resolves its operand by the same rule.
        same(
            &run(
                &service,
                &source,
                config,
                &[*args, &["--direct"][..]].concat(),
            ),
            &from_root[index],
            &format!("{args:?} directly from src"),
        );
    }
    // Absolute operands from an unrelated directory.
    let absolute = service.fixture.path("src/a.ts");
    let absolute = absolute.to_str().unwrap();
    for (index, args) in [
        &["--outline", "--json", absolute][..],
        &["--show", "--at", "5:14", absolute],
        &["--show", "--json", "--name", "run", absolute],
        &["--references", "--at", "5:21", absolute],
    ]
    .iter()
    .enumerate()
    {
        same(
            &run(&service, &elsewhere, config, args),
            &from_root[index],
            &format!("{args:?} from other"),
        );
    }
    // Diagnostics that name the operand keep the caller's spelling.
    let output = run(
        &service,
        &source,
        config,
        &["--show", "--at", "1:99", "b.ts"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "b.ts:1:99: column is outside the line\n"
    );
    same(
        &run(
            &service,
            &source,
            config,
            &["--show", "--at", "1:99", "b.ts", "--direct"],
        ),
        &output,
        "a direct diagnostic names the operand as spelled",
    );
    assert_eq!(
        String::from_utf8_lossy(&from_root[4].stderr),
        "src/b.ts:1:99: column is outside the line\n"
    );
    assert_eq!(service.generation(), 1);
}

#[test]
fn a_removed_working_directory_fails_every_operand_alike_in_both_entries() {
    // A relative operand needs the working directory, which the request
    // resolves before either entry answers: when it was removed, both fail
    // with the same I/O diagnostic, and neither guesses another directory.
    let service = service();
    let config = service.fixture.path("tsconfig.json");
    let config = config.to_str().unwrap();
    let gone = service.fixture.path("gone");
    // `sh` enters the directory and removes it before running tsparse there.
    let run_in_removed = |args: &[&str]| {
        std::fs::create_dir(&gone).unwrap();
        let mut command = std::process::Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("cd \"$1\" && rmdir \"$1\" && shift && exec \"$@\"")
            .arg("sh")
            .arg(&gone)
            .arg(env!("CARGO_BIN_EXE_tsparse"))
            .args(args)
            .args(["--project", config])
            .env("TSPARSE_SERVICE_DIR", &service.runtime)
            .env("TSPARSE_SERVICE_IDLE_SECS", "60")
            .stdin(std::process::Stdio::null());
        let output = command.output().unwrap();
        assert!(!gone.exists());
        output
    };
    for args in [
        &["--outline", "src/a.ts"][..],
        &["--outline", "--json", "src/a.ts"],
        &["--show", "--at", "5:14", "src/a.ts"],
        &["--show", "--json", "--name", "run", "src/a.ts"],
        &["--references", "--at", "5:21", "src/a.ts"],
        &["--references", "--name", "run", "src/a.ts"],
    ] {
        let direct = run_in_removed(&[args, &["--direct"]].concat());
        let served = run_in_removed(args);
        same(&served, &direct, &format!("{args:?}"));
        assert_eq!(direct.status.code(), Some(2), "{args:?} {direct:?}");
        let diagnostic = if args.contains(&"--json") {
            let document: Value = serde_json::from_slice(&direct.stdout).unwrap();
            assert_eq!(document["outcome"], "error", "{args:?}");
            assert_eq!(document["error"]["kind"], "Io", "{args:?}");
            assert_eq!(document["error"]["path"], "src/a.ts", "{args:?}");
            format!(
                "src/a.ts: {}",
                document["error"]["message"].as_str().unwrap()
            )
        } else {
            String::from_utf8(direct.stderr.clone()).unwrap()
        };
        assert!(
            diagnostic.starts_with("src/a.ts: cannot make the path absolute: "),
            "{args:?} {diagnostic}"
        );
    }
    // No operand needs no working directory.
    let expected = service.run(&["--show", "--name", "Tools.run"]);
    assert!(expected.status.success(), "{expected:?}");
    let direct = run_in_removed(&["--show", "--name", "Tools.run", "--direct"]);
    same(&direct, &expected, "direct, without an operand");
    same(
        &run_in_removed(&["--show", "--name", "Tools.run"]),
        &expected,
        "served, without an operand",
    );
}

#[test]
fn concurrent_cold_clients_of_every_operation_share_one_generation() {
    let service = service();
    let queries: Vec<&[&str]> = vec![
        &["--outline", "--json", "src/a.ts"],
        &["--show", "--json", "--name", "run"],
        &["--references", "--name", "run"],
        &["--outline", "--json", "src/b.ts"],
        &["--show", "--json", "--name", "Tools.run"],
        &["--references", "--at", "5:21", "src/a.ts"],
    ];
    let expected: Vec<Output> = queries
        .iter()
        .map(|query| direct(&service, query))
        .collect();
    let clients: Vec<_> = queries
        .iter()
        .map(|query| {
            let mut command = service.command();
            command.args(*query).args(["--project", "tsconfig.json"]);
            std::thread::spawn(move || command.output().unwrap())
        })
        .collect();
    for ((client, expected), query) in clients.into_iter().zip(&expected).zip(&queries) {
        same(&client.join().unwrap(), expected, &format!("{query:?}"));
    }
    assert_eq!(service.generation(), 1);
    // Sequential clients reuse it.
    for (query, expected) in queries.iter().zip(&expected) {
        same(&service.run(query), expected, &format!("{query:?}"));
    }
    assert_eq!(service.generation(), 1);
}

/// The labels of an outline document's top-level entries.
fn labels(document: &Value) -> Vec<String> {
    document["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["label"].as_str().unwrap().to_owned())
        .collect()
}

fn outline(service: &Service, file: &str) -> Value {
    json(service.run(&["--outline", "--json", file]))
}

/// The source a name selects, or the outcome when it selects none.
fn shown(service: &Service, name: &str) -> String {
    let output = service.run(&["--show", "--json", "--name", name]);
    let document: Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| panic!("{name}: {output:?}"));
    match document["outcome"].as_str().unwrap() {
        "found" => document["source"].as_str().unwrap().to_owned(),
        outcome => outcome.to_owned(),
    }
}

/// The first declaration of each reference group a name selects.
fn declarations(service: &Service, name: &str) -> Vec<Value> {
    service.query(name)["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| group["declarations"][0]["location"].clone())
        .collect()
}

#[test]
fn source_configuration_and_dependency_changes_serve_a_new_generation_to_every_operation() {
    let service = service();
    assert_eq!(labels(&outline(&service, "src/b.ts")), ["run", "Tools"]);
    assert_eq!(shown(&service, "Tools.run"), "export function run() {}");
    assert_eq!(declarations(&service, "helper")[0]["start"]["line"], 1);
    assert_eq!(service.generation(), 1);

    // A source edit.
    std::fs::write(
        service.fixture.path("src/b.ts"),
        "export namespace Tools {\n  export function start() {}\n}\n",
    )
    .unwrap();
    assert_eq!(labels(&outline(&service, "src/b.ts")), ["Tools"]);
    assert_eq!(service.generation(), 2);
    assert_eq!(shown(&service, "Tools.run"), "not_found");
    assert_eq!(shown(&service, "Tools.start"), "export function start() {}");
    assert_eq!(declarations(&service, "start").len(), 1);
    assert_eq!(service.generation(), 2);

    // A configuration edit adds a searched file.
    std::fs::write(
        service.fixture.path("tsconfig.json"),
        CONFIG.replace(r#"["src"]"#, r#"["src", "other"]"#),
    )
    .unwrap();
    assert_eq!(shown(&service, "outside"), "export const outside = 1;");
    assert_eq!(service.generation(), 3);
    assert_eq!(labels(&outline(&service, "other/outside.ts")), ["outside"]);
    assert_eq!(declarations(&service, "outside").len(), 1);

    // A dependency edit: show is the first request after it.
    std::fs::write(
        service.fixture.path("node_modules/dep/index.d.ts"),
        "\nexport declare function helper(): void;\n",
    )
    .unwrap();
    assert_eq!(shown(&service, "Tools.start"), "export function start() {}");
    assert_eq!(service.generation(), 4);
    assert_eq!(declarations(&service, "helper")[0]["start"]["line"], 2);
    // And an unchanged project reuses its generation.
    assert_eq!(labels(&outline(&service, "src/b.ts")), ["Tools"]);
    assert_eq!(service.generation(), 4);
}

#[test]
fn library_replacement_changes_serve_a_new_generation_to_every_operation() {
    let service = Service::of(support::Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "libReplacement": true, "lib": ["es5", "dom"] }, "files": ["a.ts"] }"#,
        ),
        ("a.ts", "function use() { return replaced; }\n"),
    ]));
    let unresolved = service.run(&["--references", "--at", "1:25", "a.ts"]);
    assert_eq!(unresolved.status.code(), Some(1), "{unresolved:?}");
    assert_eq!(labels(&outline(&service, "a.ts")), ["use"]);
    assert_eq!(service.generation(), 1);

    let package = service.fixture.path("node_modules/@typescript/lib-dom");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("package.json"), r#"{ "types": "index.d.ts" }"#).unwrap();
    std::fs::write(
        package.join("index.d.ts"),
        "declare var replaced: number;\n",
    )
    .unwrap();
    // The outline is the first request to see the installed replacement.
    assert_eq!(labels(&outline(&service, "a.ts")), ["use"]);
    assert_eq!(service.generation(), 2);
    let groups = json(service.run(&["--references", "--at", "1:25", "a.ts"]));
    assert_eq!(
        groups["groups"][0]["declarations"][0]["location"]["file"],
        "node_modules/@typescript/lib-dom/index.d.ts"
    );
    std::fs::write(
        package.join("index.d.ts"),
        "\ndeclare var replaced: number;\n",
    )
    .unwrap();
    assert_eq!(
        shown(&service, "use"),
        "function use() { return replaced; }"
    );
    assert_eq!(service.generation(), 3);
    let groups = json(service.run(&["--references", "--at", "1:25", "a.ts"]));
    assert_eq!(
        groups["groups"][0]["declarations"][0]["location"]["start"]["line"],
        2
    );
}

/// Every operation fails while `break_input` is in effect, with the same
/// diagnostic and no answer from the previous generation, and answers
/// again once `repair` restores a valid project.
fn check_failed_refresh(
    service: &Service,
    break_input: impl Fn(),
    diagnostic: &str,
    repair: impl Fn(),
) {
    let queries: [&[&str]; 4] = [
        &["--outline", "src/a.ts"],
        &["--show", "--name", "Tools.run"],
        &["--show", "--json", "--at", "5:14", "src/a.ts"],
        &["--references", "--name", "run"],
    ];
    let before: Vec<Output> = queries.iter().map(|query| service.run(query)).collect();
    assert!(before.iter().all(|output| output.status.success()));
    let generation = service.generation();
    break_input();
    for query in queries {
        // Twice: a failed refresh leaves no generation to answer from.
        for _ in 0..2 {
            let output = service.run(query);
            assert_eq!(output.status.code(), Some(1), "{query:?} {output:?}");
            if query.contains(&"--json") {
                assert!(output.stderr.is_empty());
                let document: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(document["outcome"], "error", "{query:?}");
                let error = &document["error"];
                let located = format!(
                    "{}:{}:{}: {}",
                    error["path"].as_str().unwrap(),
                    error["position"]["line"],
                    error["position"]["column"],
                    error["message"].as_str().unwrap()
                );
                assert!(located.contains(diagnostic), "{query:?}: {document}");
            } else {
                assert!(output.stdout.is_empty(), "{query:?}");
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(stderr.contains(diagnostic), "{query:?}: {stderr}");
            }
        }
    }
    assert_eq!(service.status()["running"], true);
    repair();
    for (query, before) in queries.iter().zip(&before) {
        same(
            &service.run(query),
            before,
            &format!("{query:?} after repair"),
        );
    }
    assert_eq!(service.generation(), generation + 1);
}

#[test]
fn failed_refreshes_answer_no_operation_until_the_input_is_repaired() {
    let service = service();
    let write = |file: &str, text: &str| {
        let (path, text) = (service.fixture.path(file), text.to_owned());
        move || std::fs::write(&path, &text).unwrap()
    };
    let source = std::fs::read_to_string(service.fixture.path("src/b.ts")).unwrap();
    check_failed_refresh(
        &service,
        write("src/b.ts", "export const = 1;\n"),
        "b.ts:1:14: ",
        write("src/b.ts", &source),
    );
    check_failed_refresh(
        &service,
        write("tsconfig.json", "{ \"compilerOptions\": { , } }"),
        "tsconfig.json:1:",
        write("tsconfig.json", CONFIG),
    );
    // A comment directive reaches only across lines that are empty or a
    // `//` comment after spaces and tabs (`isCommentOrBlankLine`): a line
    // holding a no-break space, or an empty line that U+2028 ends, stops it,
    // so the compiler reports the missing module and the conflicting
    // declarations, and the service publishes no answer.
    check_failed_refresh(
        &service,
        write(
            "src/b.ts",
            &format!("// @ts-ignore\n\u{a0}\nimport {{ gone }} from \"./gone\";\n{source}"),
        ),
        "b.ts:3:22: ",
        write("src/b.ts", &source),
    );
    check_failed_refresh(
        &service,
        write(
            "src/b.ts",
            &format!(
                "// @ts-ignore\n\u{2028}let twice = 1;\n// @ts-ignore\n\u{2028}let twice = 2;\n{source}"
            ),
        ),
        "b.ts:6:5: ",
        write("src/b.ts", &source),
    );
    let dependency =
        std::fs::read_to_string(service.fixture.path("node_modules/dep/index.d.ts")).unwrap();
    check_failed_refresh(
        &service,
        write(
            "node_modules/dep/index.d.ts",
            "export declare function (): void;\n",
        ),
        "index.d.ts:1:",
        write("node_modules/dep/index.d.ts", &dependency),
    );
}

#[test]
fn a_failed_library_replacement_refresh_answers_no_operation_until_repaired() {
    let service = service();
    std::fs::write(
        service.fixture.path("tsconfig.json"),
        r#"{ "compilerOptions": { "libReplacement": true, "lib": ["es5"], "moduleResolution": "bundler", "module": "esnext" }, "include": ["src"] }"#,
    )
    .unwrap();
    let package = service.fixture.path("node_modules/@typescript/lib-es5");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("package.json"), r#"{ "types": "index.d.ts" }"#).unwrap();
    let library = package.join("index.d.ts");
    std::fs::write(&library, "interface Array<T> {}\ninterface Boolean {}\ninterface CallableFunction {}\ninterface Function {}\ninterface IArguments {}\ninterface NewableFunction {}\ninterface Number {}\ninterface Object {}\ninterface RegExp {}\ninterface String {}\n").unwrap();
    let valid = std::fs::read_to_string(&library).unwrap();
    check_failed_refresh(
        &service,
        || std::fs::write(&library, "interface Array<T> {\n").unwrap(),
        "index.d.ts:2:1: ",
        || std::fs::write(&library, &valid).unwrap(),
    );
}
