//! Refusals with their remedies, as the real binary prints them (design,
//! step 5, "Refusal wording"): each in text and JSON, and each remedy
//! followed as printed in a scratch fixture, where it succeeds.
mod support;

use serde_json::{Value, json};
use std::process::{Command, Output, Stdio};
use support::Fixture;

const HOST: &str = "export interface Shape {
  area(): number;
}
export class Box {
  size = 1;
  grow() {
    return 1;
  }
}
export enum Color {
  Red,
}
export type Alias = { a: string };
export namespace Space {
  export const x = 1;
}
export function start() {
  return 1;
}
";

/// The `why` of a refused output path that exists or names a file the
/// command reads or edits.
const NEW_FILES: &str = "no outcome may replace a file the command read or edits";

/// The `why` of one new path given as both outputs.
const ONE_FILE: &str = "one file cannot hold both the result and the plan";

fn fixture() -> Fixture {
    support::project(&[
        ("host.ts", HOST),
        ("body.ts", "{ return 2; }"),
        ("existing.json", "{}"),
    ])
}

/// Run the binary in `fixture`, with a runtime directory inside it that
/// nothing creates unless a service is started.
fn tsparse(fixture: &Fixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
        .current_dir(&fixture.root)
        .env("TSPARSE_SERVICE_DIR", fixture.path("services"))
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

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

/// The one document `output` wrote to stdout beside an empty stderr, after
/// exiting with `code`.
fn document(output: &Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

/// An editing command refuses a result or plan path that exists, says why,
/// and names the remedy; the command repeated with another path succeeds,
/// and the plan it saves applies. The path is named by the flag as the
/// registry spells it, whichever spelling was given.
#[test]
fn an_existing_output_is_refused_with_its_remedy_and_why() {
    let replace = [
        "--replace-body",
        "--name",
        "start",
        "host.ts",
        "--from",
        "body.ts",
    ];
    let apply_plan = |fixture: &Fixture| {
        let saved = tsparse(fixture, &[&replace[..], &["--plan", "saved.json"]].concat());
        assert_eq!(saved.status.code(), Some(0), "{saved:?}");
    };
    for (flag, given, apply) in [
        ("--plan", "--plan", false),
        ("--output", "--output", false),
        ("--output", "-o", false),
        ("--output", "-o", true),
    ] {
        let fixture = fixture();
        let command: Vec<&str> = if apply {
            apply_plan(&fixture);
            vec!["--apply", "saved.json"]
        } else {
            replace.to_vec()
        };
        let message = format!(
            "{flag} existing.json already exists; an edit command writes its result and plan only as new files; choose another path"
        );
        let refused = [&command[..], &[given, "existing.json"]].concat();
        assert_eq!(
            outcome(&tsparse(&fixture, &refused)),
            (
                Some(2),
                String::new(),
                format!("usage.output: {message}\n  why: {NEW_FILES}\n")
            ),
            "{refused:?}"
        );
        let json = [&refused[..], &["--json"]].concat();
        let documents = if apply { "apply" } else { "edit" };
        let effect = if apply { "unchanged" } else { "none" };
        assert_eq!(
            document(&tsparse(&fixture, &json), 2),
            json!({
                "operation": documents,
                "outcome": "error",
                "effect": effect,
                "error": {
                    "category": "usage",
                    "code": "usage.output",
                    "message": message,
                    "why": NEW_FILES,
                },
            }),
            "{json:?}"
        );
        assert_eq!(
            std::fs::read_to_string(fixture.path("existing.json")).unwrap(),
            "{}"
        );
        assert_eq!(
            std::fs::read_to_string(fixture.path("host.ts")).unwrap(),
            HOST
        );
        // The remedy: another path.
        let followed = [&command[..], &[given, "another.json"]].concat();
        let output = tsparse(&fixture, &followed);
        assert_eq!(output.status.code(), Some(0), "{followed:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert!(fixture.path("another.json").is_file(), "{followed:?}");
        if flag == "--plan" {
            let applied = tsparse(&fixture, &["--apply", "another.json"]);
            assert_eq!(applied.status.code(), Some(0), "{applied:?}");
            assert!(
                std::fs::read_to_string(fixture.path("host.ts"))
                    .unwrap()
                    .contains("export function start() { return 2; }"),
            );
        }
    }
    // One new path given as both outputs is named by both flags as
    // registered, with why.
    let fixture = fixture();
    let one = "--output and --plan name one file; choose another path";
    for given in ["-o", "--output"] {
        let both = [&replace[..], &["--plan", "one.json", given, "one.json"]].concat();
        assert_eq!(
            outcome(&tsparse(&fixture, &both)),
            (
                Some(2),
                String::new(),
                format!("usage.output: {one}\n  why: {ONE_FILE}\n")
            ),
            "{both:?}"
        );
        let json = [&both[..], &["--json"]].concat();
        assert_eq!(
            document(&tsparse(&fixture, &json), 2),
            json!({
                "operation": "edit",
                "outcome": "error",
                "effect": "none",
                "error": {
                    "category": "usage",
                    "code": "usage.output",
                    "message": one,
                    "why": ONE_FILE,
                },
            }),
            "{json:?}"
        );
        let followed = [&replace[..], &["--plan", "one.json", given, "two.json"]].concat();
        let output = tsparse(&fixture, &followed);
        assert_eq!(output.status.code(), Some(0), "{followed:?}: {output:?}");
        std::fs::remove_file(fixture.path("one.json")).unwrap();
        std::fs::remove_file(fixture.path("two.json")).unwrap();
    }
    // A new path the command reads, given as -o, is named by its
    // registered flag too, with why. Only a path that does not exist gets
    // this far, so the request, given another path, goes on to report that
    // its fragment cannot be read.
    let reads = [
        "--replace-body",
        "--name",
        "start",
        "host.ts",
        "--from",
        "absent.ts",
        "-o",
        "absent.ts",
    ];
    let named = "--output absent.ts names a file the command reads or edits, absent.ts; choose another path";
    assert_eq!(
        outcome(&tsparse(&fixture, &reads)),
        (
            Some(2),
            String::new(),
            format!("usage.output: {named}\n  why: {NEW_FILES}\n")
        ),
    );
    let json = [&reads[..], &["--json"]].concat();
    assert_eq!(
        document(&tsparse(&fixture, &json), 2),
        json!({
            "operation": "edit",
            "outcome": "error",
            "effect": "none",
            "error": {
                "category": "usage",
                "code": "usage.output",
                "message": named,
                "why": NEW_FILES,
            },
        }),
    );
    let followed = [&reads[..6], &["-o", "fresh.txt"]].concat();
    let output = tsparse(&fixture, &followed);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        text(&output.stderr).starts_with("input.read: absent.ts: "),
        "{output:?}"
    );
    assert!(!fixture.path("absent.ts").exists());
    assert!(!fixture.path("fresh.txt").exists());
}

/// Replacing the body of a declaration whose content is a member list (an
/// interface, a class, or an enum) is refused with the reason it always
/// had and the remedy, a declaration with a body, never `--insert-after`,
/// which inserts after the whole declaration. Other declarations without a
/// body keep their refusal. The remedy, followed, previews.
#[test]
fn a_member_list_body_is_refused_with_the_remedy() {
    let fixture = fixture();
    let host = fixture.path("host.ts");
    let host = host.to_str().unwrap();
    const REMEDY: &str = "; member-list editing is not supported; select a declaration with a body";
    for (name, position, kind, remedy) in [
        ("Shape", "1:18", "interface", REMEDY),
        ("Box", "4:14", "class", REMEDY),
        ("Color", "10:13", "enum", REMEDY),
        ("Alias", "13:13", "type", ""),
        ("Space", "14:18", "namespace", ""),
    ] {
        let message = format!(
            "{name} cannot have its body replaced: this {kind} has no body to replace{remedy}"
        );
        let file = [
            "--replace-body",
            "--name",
            name,
            "host.ts",
            "--from",
            "body.ts",
        ];
        let project = [
            "--replace-body",
            "--name",
            name,
            "--from",
            "body.ts",
            "--project",
            "tsconfig.json",
            "--direct",
        ];
        // File edits name the file absolutely, and project edits relative
        // to the configuration's directory.
        for (args, named) in [(&file[..], host), (&project[..], "host.ts")] {
            let output = tsparse(&fixture, args);
            assert_eq!(
                outcome(&output),
                (
                    Some(1),
                    String::new(),
                    format!("{named}:{position}: edit 1: {message}\n")
                ),
                "{args:?}"
            );
            let json = [args, &["--json"]].concat();
            let rejected = document(&tsparse(&fixture, &json), 1);
            assert_eq!(rejected["outcome"], "rejected", "{json:?}");
            assert_eq!(rejected["error"]["kind"], "UnsupportedSite", "{json:?}");
            assert_eq!(rejected["error"]["message"], message.as_str(), "{json:?}");
            assert!(!message.contains("insert-after"), "{message}");
        }
    }
    // The remedy: a declaration with a body, such as a class's method.
    for name in ["Box.grow", "start"] {
        let args = [
            "--replace-body",
            "--name",
            name,
            "host.ts",
            "--from",
            "body.ts",
        ];
        let output = tsparse(&fixture, &args);
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
        assert!(text(&output.stdout).contains("{ return 2; }"), "{output:?}");
    }
    assert_eq!(
        std::fs::read_to_string(fixture.path("host.ts")).unwrap(),
        HOST
    );
}

/// A file that is not one of the project's searched files names the
/// configuration whose `files`, `include`, and `exclude` decide membership,
/// in text and JSON, through the service and directly alike, for every
/// kind of project request: navigation, a query, a type, and an edit. Each
/// setting, changed as the failure says, makes the file searched.
#[cfg(unix)]
#[test]
fn a_file_outside_the_project_names_the_configuration_that_decides() {
    const CONFIGURATION: &str = "; the files, include, and exclude of tsconfig.json, or of a configuration it extends, decide which files are searched";
    let config =
        |settings: &str| format!(r#"{{ "compilerOptions": {{ "noLib": true }}, {settings} }}"#);
    // (the configuration, the file outside it, and the configuration that
    // makes it searched)
    let cases = [
        (
            config(r#""files": ["src/a.ts"]"#),
            "src/b.ts",
            config(r#""files": ["src/a.ts", "src/b.ts"]"#),
        ),
        (
            config(r#""include": ["src"]"#),
            "lib/c.ts",
            config(r#""include": ["src", "lib"]"#),
        ),
        (
            config(r#""include": ["src"], "exclude": ["src/b.ts"]"#),
            "src/b.ts",
            config(r#""include": ["src"]"#),
        ),
    ];
    for (outside, file, inside) in cases {
        let service = support::service::Service::of(Fixture::new(&[
            ("tsconfig.json", &outside),
            ("src/a.ts", "export const a = 1;\n"),
            ("src/b.ts", "export function run() {}\n"),
            ("lib/c.ts", "export function run() {}\n"),
            ("body.ts", "{ return; }"),
        ]));
        /// A request, its documents' operation, and the fields its
        /// documents add.
        type Request<'a> = (Vec<&'a str>, &'a str, &'a [(&'a str, &'a str)]);
        let requests: [Request; 5] = [
            (vec!["--outline", file], "outline", &[]),
            (vec!["--show", "--name", "run", file], "show", &[]),
            (
                vec!["--references", "--name", "run", file],
                "references",
                &[],
            ),
            (vec!["--type", "--name", "run", file], "type", &[]),
            (
                vec!["--replace-body", "--name", "run", file, "--from", "body.ts"],
                "edit",
                &[("effect", "none")],
            ),
        ];
        for (args, operation, fields) in &requests {
            let message = format!("not one of the project's searched files{CONFIGURATION}");
            assert_eq!(
                outcome(&service.both(args)),
                (Some(2), String::new(), format!("{file}: {message}\n")),
                "{outside} {args:?}"
            );
            let json = [&args[..], &["--json"]].concat();
            let mut expected = json!({
                "operation": operation,
                "outcome": "error",
                "project": "tsconfig.json",
                "error": {
                    "kind": "OutsideProject",
                    "message": message,
                    "path": file,
                    "position": null,
                },
            });
            for (field, value) in *fields {
                expected[*field] = json!(value);
            }
            assert_eq!(
                document(&service.both(&json), 2),
                expected,
                "{outside} {json:?}"
            );
        }
        // The remedy: the setting changed so that the file is searched.
        std::fs::write(service.fixture.path("tsconfig.json"), &inside).unwrap();
        for (args, _, _) in &requests {
            let output = service.both(args);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{inside} {args:?}: {output:?}"
            );
        }
    }
}

/// Help says that the three service flags administer the service that
/// answers every project query, and no help, in text or JSON, calls it a
/// reference service.
#[test]
fn help_names_the_project_service_and_what_administers_it() {
    const ADMINISTERS: &str = "--references-status, --references-stop, and --references-serve administer the project service, which answers every project query: --references, --type, and --outline, --show, and edits with --project.";
    let help = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
        text(&output.stdout)
    };
    let index = help(&["--help"]);
    assert!(index.contains(ADMINISTERS), "{index}");
    for flag in [
        "--references-status",
        "--references-stop",
        "--references-serve",
    ] {
        let leaf = help(&[flag, "--help"]);
        assert!(leaf.contains(ADMINISTERS), "{flag}: {leaf}");
    }
    let registry: Value = serde_json::from_str(&help(&["--help", "--json"])).unwrap();
    let mut every = vec![index, registry.to_string()];
    for operation in registry["operations"].as_array().unwrap() {
        let mut args: Vec<&str> = operation["flag"].as_str().into_iter().collect();
        if let Some(flag) = registry["flags"]
            .as_array()
            .unwrap()
            .iter()
            .find(|flag| flag["name"] == operation["flag"])
            && let Some(value) = flag["value"].as_str()
        {
            args.push(value);
        }
        args.push("--help");
        every.push(help(&args));
        args.push("--json");
        every.push(help(&args));
    }
    for help in every {
        assert!(!help.to_lowercase().contains("reference service"), "{help}");
    }
}

/// No message, help text, or user-facing document calls the service that
/// answers every project query a reference service. Historical design and
/// measurement records keep their wording, and file names stay.
#[test]
fn no_source_or_user_document_says_reference_service() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![
        root.join("README.md"),
        root.join("docs/references.md"),
        root.join("docs/navigation.md"),
        root.join("docs/editing.md"),
        root.join("docs/cli-reference.md"),
        root.join("docs/agent-guide.md"),
    ];
    let mut directories = vec![root.join("src")];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    let mut found = Vec::new();
    for file in &files {
        let content = std::fs::read_to_string(file).unwrap();
        for (number, line) in content.lines().enumerate() {
            if line.to_lowercase().contains("reference service") {
                found.push(format!("{}:{}: {line}", file.display(), number + 1));
            }
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}
