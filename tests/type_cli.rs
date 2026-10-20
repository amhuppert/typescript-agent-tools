//! Type queries use the existing project request and delivery boundaries.
//! These usage checks pin product policy before semantic evaluation begins.
mod support;

use serde_json::{Value, json};
use std::process::{Command, Output, Stdio};
use support::project;
use support::service::Service;

fn run(args: &[&str]) -> Output {
    let fixture = project(&[("value.ts", "export const value = 1;\n")]);
    Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
        .current_dir(&fixture.root)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn usage(args: &[&str], message: &str) {
    let output = run(args);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostic.contains(message), "{args:?}: {diagnostic}");
}

#[test]
fn type_queries_require_an_explicit_project_and_one_selection() {
    usage(
        &["--type", "--name", "value"],
        "--type requires --project TSCONFIG",
    );
    usage(
        &["--type", "--project", "tsconfig.json"],
        "--type requires exactly one of --at or --name",
    );
    usage(
        &[
            "--type",
            "--project",
            "tsconfig.json",
            "--at",
            "value.ts:1:14",
            "--name",
            "value",
        ],
        "--type requires exactly one of --at or --name",
    );
    usage(
        &[
            "--type",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
            "-",
        ],
        "--type reads files from the project, not stdin",
    );
}

#[test]
fn type_display_options_and_operation_flags_are_validated_before_loading() {
    usage(
        &[
            "--type",
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
        ],
        "Choose one operation:",
    );
    usage(
        &[
            "--type",
            "--outline",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
        ],
        "Choose one operation:",
    );
    usage(
        &[
            "--type",
            "--calls",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
        ],
        "--calls applies to: --references",
    );
    usage(
        &[
            "--type",
            "--check",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
        ],
        "--check applies to: parse mode (no operation flag)",
    );
    usage(
        &[
            "--type",
            "--pretty",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
        ],
        "--pretty applies to --type only with --json",
    );
    usage(
        &[
            "--type",
            "--depth",
            "1",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
        ],
        "--depth applies to --type only with --expand",
    );
    for depth in ["-1", "all", "1.5", ""] {
        usage(
            &[
                "--type",
                "--expand",
                "--depth",
                depth,
                "--project",
                "tsconfig.json",
                "--name",
                "value",
            ],
            "--depth expects an integer from 0 through 32",
        );
    }
    usage(
        &[
            "--type",
            "--expand",
            "--depth",
            "33",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
        ],
        "type expansion depth must be between 0 and 32, got 33",
    );
    usage(
        &["--expand", "--outline", "value.ts"],
        "--expand applies to: --type",
    );
    usage(
        &[
            "--type",
            "--project",
            "tsconfig.json",
            "--project",
            "other.json",
            "--name",
            "value",
        ],
        "--type takes --project at most once",
    );
}

#[test]
fn native_alias_type_uses_the_same_document_and_locations_in_direct_and_service_entry() {
    let service = Service::new(&[
        (
            "model.ts",
            "export const current = { label: 'ready', count: 1 };\n",
        ),
        (
            "barrel.ts",
            "export { current as chosen } from './model';\n",
        ),
        (
            "consumer.ts",
            "import { chosen as renamed } from './barrel';\n",
        ),
    ]);
    let arguments = [
        "--type",
        "--name",
        "renamed",
        "consumer.ts",
        "--expand",
        "--json",
    ];
    let direct = service
        .command()
        .args(arguments)
        .args(["--project", "tsconfig.json", "--direct"])
        .output()
        .unwrap();
    assert_eq!(direct.status.code(), Some(0), "{direct:?}");
    assert!(direct.stderr.is_empty(), "{direct:?}");
    let document: Value = serde_json::from_slice(&direct.stdout).unwrap();
    assert_eq!(document["operation"], "type");
    assert_eq!(document["outcome"], "found");
    assert_eq!(document["target"]["kind"], "value");
    assert_eq!(document["target"]["location"]["file"], "consumer.ts");
    assert_eq!(
        document["type"]["summary"],
        "{ label: string; count: number; }"
    );
    let root = &document["type"]["expansion"]["nodes"][0];
    assert_eq!(root["kind"], "object");
    let keys: Vec<_> = root["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["key"].clone())
        .collect();
    assert_eq!(
        keys,
        vec![
            json!({"kind":"string","name":"label"}),
            json!({"kind":"string","name":"count"})
        ]
    );
    assert_eq!(root["properties"][0]["origins"][0]["file"], "model.ts");
    let remote = service.run(&arguments);
    assert_eq!(remote.status.code(), direct.status.code(), "{remote:?}");
    assert_eq!(remote.stdout, direct.stdout);
    assert_eq!(remote.stderr, direct.stderr);
    let pretty = service.run(&[
        "--type",
        "--name",
        "renamed",
        "consumer.ts",
        "--expand",
        "--json",
        "--pretty",
    ]);
    assert_eq!(pretty.status.code(), Some(0), "{pretty:?}");
    assert_eq!(
        serde_json::from_slice::<Value>(&pretty.stdout).unwrap(),
        document
    );
    let text = service.run(&["--type", "--name", "renamed", "consumer.ts", "--expand"]);
    assert_eq!(text.status.code(), Some(0), "{text:?}");
    assert!(text.stderr.is_empty(), "{text:?}");
    let rendered = String::from_utf8(text.stdout).unwrap();
    assert!(rendered.starts_with("TYPE\n"), "{rendered}");
    assert!(rendered.contains("\n  \"label\": string"), "{rendered}");
    assert!(rendered.contains("\n  \"count\": number"), "{rendered}");
}

#[test]
fn completed_conditional_members_are_found_in_direct_and_service_delivery() {
    let service = Service::new(&[
        (
            "model.ts",
            "export interface Model { known: string; deferred: string extends number ? string : number; }\n",
        ),
        ("consumer.ts", "import { Model as View } from './model';\n"),
    ]);
    let arguments = [
        "--type",
        "--name",
        "View",
        "consumer.ts",
        "--expand",
        "--json",
    ];
    let direct = service
        .command()
        .args(arguments)
        .args(["--project", "tsconfig.json", "--direct"])
        .output()
        .unwrap();
    assert_eq!(direct.status.code(), Some(0), "{direct:?}");
    assert!(direct.stderr.is_empty(), "{direct:?}");
    let document: Value = serde_json::from_slice(&direct.stdout).unwrap();
    assert_eq!(document["outcome"], "found");
    assert_eq!(document["issues"], json!([]));
    let nodes = document["type"]["expansion"]["nodes"].as_array().unwrap();
    let properties = nodes[0]["properties"].as_array().unwrap();
    let deferred = properties
        .iter()
        .find(|property| property["key"]["name"] == "deferred")
        .unwrap();
    let result = nodes
        .iter()
        .find(|node| node["id"] == deferred["type"]["ref"])
        .unwrap();
    assert_eq!(
        (result["kind"].as_str(), result["name"].as_str()),
        (Some("primitive"), Some("number"))
    );
    let remote = service.run(&arguments);
    assert_eq!(remote.status.code(), Some(0), "{remote:?}");
    assert_eq!(remote.stdout, direct.stdout);
    assert_eq!(remote.stderr, direct.stderr);
}

/// A project whose `View`, expanded, is a partial type. Native proves
/// healthy Model/Element; demanding JSX expression typing is explicitly
/// deferred by the product profile. Keep partial delivery coverage
/// independent of newly implemented type operators.
fn partial_type_project() -> Service {
    Service::of(support::Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-jsx-partial/tsconfig.json"),
        ),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-jsx-partial/globals.d.ts"),
        ),
        (
            "model.tsx",
            include_str!("../fixtures/types/native-jsx-partial/model.tsx"),
        ),
        (
            "consumer.ts",
            "import type { Model as View } from './model';\n",
        ),
    ]))
}

#[test]
fn partial_type_delivery_keeps_results_on_stdout_and_in_requested_output_files() {
    let service = partial_type_project();
    let arguments = [
        "--type",
        "--name",
        "View",
        "consumer.ts",
        "--expand",
        "--json",
    ];
    let direct = service
        .command()
        .args(arguments)
        .args(["--project", "tsconfig.json", "--direct"])
        .output()
        .unwrap();
    assert_eq!(direct.status.code(), Some(1), "{direct:?}");
    assert!(direct.stderr.is_empty(), "{direct:?}");
    let document: Value = serde_json::from_slice(&direct.stdout).unwrap();
    assert_eq!(document["operation"], "type");
    assert_eq!(document["outcome"], "partial");
    assert_eq!(document["type"]["summary"], "Model");
    assert_eq!(document["type"]["expansion"]["truncated"], false);
    assert_eq!(document["issues"].as_array().unwrap().len(), 1);
    assert_eq!(document["issues"][0]["location"]["file"], "model.tsx");
    assert!(
        document["type"]["expansion"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["kind"] == "unsupported")
    );
    let remote = service.run(&arguments);
    assert_eq!(remote.status.code(), Some(1), "{remote:?}");
    assert_eq!(remote.stdout, direct.stdout);
    assert_eq!(remote.stderr, direct.stderr);
    let text_arguments = ["--type", "--name", "View", "consumer.ts", "--expand"];
    let text = service.run(&text_arguments);
    assert_eq!(text.status.code(), Some(1), "{text:?}");
    assert!(text.stderr.is_empty(), "{text:?}");
    let rendered = String::from_utf8(text.stdout).unwrap();
    assert!(
        rendered.starts_with("PARTIAL TYPE — unsupported details\n"),
        "{rendered}"
    );
    assert!(rendered.contains("\n  \"known\": string"), "{rendered}");
    assert!(
        rendered.contains("\n  \"deferred\": unsupported"),
        "{rendered}"
    );
    assert!(
        rendered.contains("Unsupported at ") && rendered.contains("model.tsx:"),
        "{rendered}"
    );
    let written = service
        .command()
        .args(text_arguments)
        .args(["--project", "tsconfig.json", "--output", "partial.txt"])
        .output()
        .unwrap();
    assert_eq!(written.status.code(), Some(1), "{written:?}");
    assert!(
        written.stdout.is_empty() && written.stderr.is_empty(),
        "{written:?}"
    );
    assert_eq!(
        std::fs::read_to_string(service.fixture.path("partial.txt")).unwrap(),
        rendered
    );
}

/// A partial type document reaches `-o` as it reaches stdout: exit 1,
/// nothing on stdout or stderr, its partial outcome and its issue kept, and
/// the same bytes through the service and loaded directly, plain and
/// indented (design, "Preserved behavior": with `--json`, partial results
/// reach `-o`).
#[test]
fn a_partial_type_document_reaches_the_output_file_through_both_entries() {
    let service = partial_type_project();
    for format in [&["--json"][..], &["--json", "--pretty"]] {
        let request = [
            &["--type", "--name", "View", "consumer.ts", "--expand"][..],
            format,
            &["--project", "tsconfig.json"],
        ]
        .concat();
        let printed = service
            .command()
            .args(&request)
            .arg("--direct")
            .output()
            .unwrap();
        assert_eq!(printed.status.code(), Some(1), "{printed:?}");
        let mut written = Vec::new();
        for transport in [&[][..], &["--direct"]] {
            let path = service.fixture.path("partial.json");
            let _ = std::fs::remove_file(&path);
            let output = service
                .command()
                .args(&request)
                .args(transport)
                .args(["-o", "partial.json"])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1), "{transport:?}: {output:?}");
            assert!(
                output.stdout.is_empty() && output.stderr.is_empty(),
                "{transport:?}: {output:?}"
            );
            written.push(std::fs::read(&path).unwrap());
        }
        assert_eq!(written[0], written[1], "{format:?}");
        assert_eq!(written[0], printed.stdout, "{format:?}");
        let document: Value = serde_json::from_slice(&written[0]).unwrap();
        assert_eq!(document["operation"], "type");
        assert_eq!(document["outcome"], "partial");
        assert_eq!(document["type"]["summary"], "Model");
        assert_eq!(document["issues"].as_array().unwrap().len(), 1);
        assert_eq!(document["issues"][0]["location"]["file"], "model.tsx");
    }
}
