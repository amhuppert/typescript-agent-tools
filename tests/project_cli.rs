//! The project query commands (`--outline`, `--show`, and `--references`
//! with `--project`): request validity, and the delivery of every outcome
//! to stdout, stderr, `-o`, and exit codes, identically in direct and
//! service entry. Which declarations exist is covered by
//! `project_navigation.rs`; freshness by `project_service.rs`.
mod support;

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Output;
use support::Fixture;
use support::service::Service;

/// Containment and anchors as in `navigation_cli.rs`, whose file-mode
/// expectations come from TypeScript 7.0.2's document symbols.
const HOST: &str = "\
export function createRunHost(deps: Deps) {
  function startEligible() {
    return true;
  }
  return { startEligible, run() {} };
}
function overload(a: string): void;
function overload(a: number): void;
function overload(a: unknown) {}
type Deps = {};
";

const OTHER: &str =
    "export function run() {}\nexport namespace Tools {\n  export function run() {}\n}\n";

const CONFIG: &str = r#"{ "compilerOptions": { "noLib": true, "resolveJsonModule": true, "module": "esnext", "moduleResolution": "bundler" }, "include": ["src"] }"#;

fn fixture() -> Fixture {
    Fixture::new(&[
        ("tsconfig.json", CONFIG),
        ("src/host.ts", HOST),
        ("src/other.ts", OTHER),
        (
            "src/app.ts",
            "import data from \"./data.json\";\nexport const size = data.size;\n",
        ),
        ("src/data.json", "{ \"size\": 1 }\n"),
        ("outside/x.ts", "export const x = 1;\n"),
    ])
}

/// How a project query is answered: in the CLI process (`--direct`), or by
/// the project's service. Every outcome must be delivered identically.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Entry {
    Direct,
    Service,
}

/// The service is Unix-only; other platforms always answer directly.
const ENTRIES: &[Entry] = if cfg!(unix) {
    &[Entry::Direct, Entry::Service]
} else {
    &[Entry::Direct]
};

/// A project whose commands run in `entry` from its directory. Direct
/// commands share the service's private runtime directory, so a direct
/// command that started a service would be seen.
struct Project {
    service: Service,
    entry: Entry,
}

impl Project {
    fn new(entry: Entry) -> Self {
        Self::of(fixture(), entry)
    }

    fn of(fixture: Fixture, entry: Entry) -> Self {
        Self {
            service: Service::of(fixture),
            entry,
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.service.fixture.path(relative)
    }

    fn root(&self) -> &Path {
        &self.service.fixture.root
    }

    /// `args` for the project's `tsconfig.json`, from `directory`.
    fn run_in(&self, directory: &Path, args: &[&str]) -> Output {
        let mut command = self.service.command();
        command.current_dir(directory).args(args);
        if self.entry == Entry::Direct {
            command.arg("--direct");
        }
        command.output().unwrap()
    }

    /// `args` with `--project tsconfig.json`, from the project directory.
    fn run(&self, args: &[&str]) -> Output {
        self.run_in(
            self.root(),
            &[args, &["--project", "tsconfig.json"]].concat(),
        )
    }

    fn config(&self) -> String {
        self.path("tsconfig.json").to_str().unwrap().to_owned()
    }

    /// No direct command may start a service.
    fn check_no_service(&self) {
        if self.entry == Entry::Direct {
            assert!(
                !self.service.runtime.exists(),
                "a direct command used the service"
            );
        }
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// A JSON outcome: its document on stdout, nothing on stderr.
fn document(output: &Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}

/// A transport failure with `code`, exit 2, as its message and the lines
/// text gives after it: with `--json` one document, here on stdout, naming
/// `operation`, whose `why`, `next`, and `remedy` are those lines; in text
/// `code: message` and those lines on stderr.
fn transport(output: &Output, json: Option<&str>, code: &str) -> (String, Vec<String>) {
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    match json {
        Some(operation) => {
            let document = document(output, 2);
            assert_eq!(document["operation"], operation, "{document}");
            assert_eq!(document["outcome"], "error", "{document}");
            assert_eq!(document["error"]["category"], "transport", "{document}");
            assert_eq!(document["error"]["code"], code, "{document}");
            let error = &document["error"];
            // Nothing else: the document's lines are all text gives.
            assert!(
                document.as_object().unwrap().len() == 3
                    && error.as_object().unwrap().keys().all(|key| {
                        ["category", "code", "message", "why", "next", "remedy"]
                            .contains(&key.as_str())
                    }),
                "{document}"
            );
            let mut lines = Vec::new();
            if let Some(why) = error["why"].as_str() {
                lines.push(format!("  why: {why}"));
            }
            if let Some(argv) = error["next"]["argv"].as_array() {
                let argv: Vec<&str> = argv.iter().map(|a| a.as_str().unwrap()).collect();
                lines.push(format!("  next: {}", argv.join(" ")));
            }
            if let Some(remedy) = error["remedy"].as_str() {
                lines.push(format!("  next: {remedy}"));
            }
            (error["message"].as_str().unwrap().to_string(), lines)
        }
        None => {
            assert!(output.stdout.is_empty(), "{output:?}");
            let stderr = text(&output.stderr);
            let mut lines = stderr.lines();
            let message = lines
                .next()
                .and_then(|first| first.strip_prefix(&format!("{code}: ")))
                .unwrap_or_else(|| panic!("{stderr}"))
                .to_string();
            (message, lines.map(String::from).collect())
        }
    }
}

/// A text outcome: the result on stdout for 0, or a diagnostic on stderr.
fn text_outcome(output: &Output, code: i32) -> String {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    if code == 0 {
        assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
        text(&output.stdout)
    } else {
        assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
        text(&output.stderr)
    }
}

#[test]
fn project_outline_matches_file_outline_with_the_project_as_path_base() {
    for &entry in ENTRIES {
        let project = Project::new(entry);
        // File navigation of the same file, which `--direct` does not apply to.
        let file_mode = |args: &[&str]| project.service.command().args(args).output().unwrap();
        let file_mode = file_mode(&["--outline", "src/host.ts"]);
        let expected = text_outcome(&file_mode, 0);
        assert!(expected.starts_with(&format!("{}\n", project.path("src/host.ts").display())));
        // Text names files absolutely in both modes.
        let output = project.run(&["--outline", "src/host.ts"]);
        assert_eq!(text_outcome(&output, 0), expected, "{entry:?}");
        let output = project.run(&["--outline", "--depth", "1", "src/host.ts"]);
        assert!(
            text_outcome(&output, 0).contains("(deps: Deps)\n  … 2 more children\n"),
            "{entry:?}"
        );

        let file_json = document(
            &project
                .service
                .command()
                .args(["--outline", "--json", "--depth", "all", "src/host.ts"])
                .output()
                .unwrap(),
            0,
        );
        let json = document(
            &project.run(&["--outline", "--json", "--depth", "all", "src/host.ts"]),
            0,
        );
        assert_eq!(json["operation"], "outline");
        assert_eq!(json["outcome"], "ok");
        assert_eq!(json["project"], project.config());
        assert_eq!(json["file"], "src/host.ts");
        assert_eq!(json["entries"], file_json["entries"], "{entry:?}");
        assert_eq!(file_json.get("project"), None);
        let pretty = project.run(&["--outline", "--json", "--pretty", "src/host.ts"]);
        assert!(text(&pretty.stdout).starts_with("{\n  \""), "{entry:?}");
        project.check_no_service();
    }
}

#[test]
fn project_show_selects_by_name_across_files_and_by_position_in_one() {
    for &entry in ENTRIES {
        let project = Project::new(entry);
        let output = project.run(&["--show", "--name", "Tools.run"]);
        assert_eq!(
            text_outcome(&output, 0),
            format!(
                "{}:3:19 function run\n3 | export function run() {{}}\n",
                project.path("src/other.ts").display()
            ),
            "{entry:?}"
        );
        let json = document(
            &project.run(&["--show", "--json", "--name", "Tools.run"]),
            0,
        );
        assert_eq!(
            json,
            json!({
                "operation": "show",
                "outcome": "found",
                "project": project.config(),
                "file": "src/other.ts",
                "declaration": {
                    "label": "run", "key": "run", "selector": "Tools.run", "kind": "function",
                    "anchor": { "line": 3, "column": 19 },
                    "range": { "start": { "line": 3, "column": 3 }, "end": { "line": 3, "column": 27 } },
                    "signature": "export function run()",
                },
                "source": "export function run() {}",
            }),
            "{entry:?}"
        );
        // A file narrows a name; a position needs one.
        let json = document(
            &project.run(&["--show", "--json", "--name", "run", "src/host.ts"]),
            0,
        );
        assert_eq!(json["file"], "src/host.ts");
        assert_eq!(json["source"], "run() {}");
        assert_eq!(json["declaration"]["selector"], "createRunHost.run");
        let json = document(
            &project.run(&["--show", "--json", "--at", "8:10", "src/host.ts"]),
            0,
        );
        assert_eq!(json["source"], "function overload(a: number): void;");
        project.check_no_service();
    }
}

/// TypeScript 7.0.2's workspace symbols (`ProvideWorkspaceSymbols` over
/// `computeDeclarationMap`), as `tsc --lsp` answers `workspace/symbol`,
/// name an import or export specifier only when it renames, and never a
/// default import, so a name selects the declaration it was imported from
/// rather than its imports; renamed imports, namespace imports, and
/// renamed exports keep their names, and every import remains selectable
/// by position.
#[test]
fn imports_and_exports_have_a_name_only_where_they_rename() {
    let fixture = || {
        Fixture::new(&[
            ("tsconfig.json", CONFIG),
            (
                "src/thing.ts",
                "export function createThing() { return 1; }\nexport { createThing as aliasThing };\n",
            ),
            (
                "src/b.ts",
                "import { createThing } from \"./thing\";\nexport { createThing };\ncreateThing();\n",
            ),
            (
                "src/c.ts",
                "import { createThing as made } from \"./thing\";\nimport def, * as ns from \"./d\";\nmade(def, ns);\n",
            ),
            ("src/d.ts", "export default function make() {}\n"),
            ("src/e.ts", "export { createThing } from \"./thing\";\n"),
        ])
    };
    for &entry in ENTRIES {
        let project = Project::of(fixture(), entry);
        let found = |name: &str, file: &str, anchor: [u64; 2], kind: &str| {
            let json = document(&project.run(&["--show", "--json", "--name", name]), 0);
            assert_eq!(json["file"], file, "{entry:?} {name}");
            assert_eq!(
                json["declaration"]["anchor"],
                json!({ "line": anchor[0], "column": anchor[1] }),
                "{entry:?} {name}"
            );
            assert_eq!(json["declaration"]["kind"], kind, "{entry:?} {name}");
            assert_eq!(json["declaration"]["selector"], name, "{entry:?} {name}");
        };
        // Three files import or re-export it without renaming.
        found("createThing", "src/thing.ts", [1, 17], "function");
        found("made", "src/c.ts", [1, 25], "import");
        found("ns", "src/c.ts", [2, 18], "import");
        found("aliasThing", "src/thing.ts", [2, 25], "export");
        let json = document(&project.run(&["--show", "--json", "--name", "def"]), 1);
        assert_eq!(json["outcome"], "not_found", "{entry:?}");

        // Narrowed to a file that only imports it, the name selects nothing;
        // the import's name is still its anchor.
        let json = document(
            &project.run(&["--show", "--json", "--name", "createThing", "src/b.ts"]),
            1,
        );
        assert_eq!(json["outcome"], "not_found", "{entry:?}");
        for (file, at, source) in [
            (
                "src/b.ts",
                "1:10",
                "import { createThing } from \"./thing\";",
            ),
            ("src/b.ts", "2:10", "export { createThing };"),
            ("src/c.ts", "2:8", "import def, * as ns from \"./d\";"),
        ] {
            let json = document(&project.run(&["--show", "--json", "--at", at, file]), 0);
            assert_eq!(json["source"], source, "{entry:?} {file} {at}");
            assert!(json["declaration"].get("key").is_none(), "{json}");
            assert!(json["declaration"].get("selector").is_none(), "{json}");
        }
        let outline = document(&project.run(&["--outline", "--json", "src/c.ts"]), 0);
        let names: Vec<(&str, Option<&str>)> = outline["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| (entry["label"].as_str().unwrap(), entry["key"].as_str()))
            .collect();
        assert_eq!(
            names,
            [("made", Some("made")), ("def", None), ("ns", Some("ns"))],
            "{entry:?}"
        );
        project.check_no_service();
    }
}

#[test]
fn ambiguity_lists_candidates_whose_project_relative_anchors_round_trip_from_any_directory() {
    for &entry in ENTRIES {
        let project = Project::new(entry);
        let host = project.path("src/host.ts");
        let other = project.path("src/other.ts");
        let output = project.run(&["--show", "--name", "run"]);
        assert_eq!(
            text_outcome(&output, 1),
            format!(
                "{}: `run` matches 3 declarations; select one with --at FILE:LINE:COLUMN\n\
                 {}:5:27 method run (createRunHost.run)\n\
                 {2}:1:17 function run (run)\n\
                 {2}:3:19 function run (Tools.run)\n",
                project.config(),
                host.display(),
                other.display()
            ),
            "{entry:?}"
        );

        let json = document(&project.run(&["--show", "--json", "--name", "run"]), 1);
        assert_eq!(
            json,
            json!({
                "operation": "show",
                "outcome": "ambiguous",
                "project": project.config(),
                "query": { "name": "run" },
                "candidates": [
                    { "file": "src/host.ts", "label": "run", "kind": "method", "key": "run",
                      "selector": "createRunHost.run", "anchor": { "line": 5, "column": 27 } },
                    { "file": "src/other.ts", "label": "run", "kind": "function", "key": "run",
                      "selector": "run", "anchor": { "line": 1, "column": 17 } },
                    { "file": "src/other.ts", "label": "run", "kind": "function", "key": "run",
                      "selector": "Tools.run", "anchor": { "line": 3, "column": 19 } },
                ],
            }),
            "{entry:?}"
        );
        // Each candidate's file, joined to the project's directory, and its
        // anchor select it, whatever the working directory.
        let base = Path::new(json["project"].as_str().unwrap())
            .parent()
            .unwrap();
        let elsewhere = project.path("outside");
        for candidate in json["candidates"].as_array().unwrap() {
            let file = base.join(candidate["file"].as_str().unwrap());
            let at = format!(
                "{}:{}",
                candidate["anchor"]["line"], candidate["anchor"]["column"]
            );
            let found = document(
                &project.run_in(
                    &elsewhere,
                    &[
                        "--show",
                        "--json",
                        "--project",
                        json["project"].as_str().unwrap(),
                        "--at",
                        &at,
                        file.to_str().unwrap(),
                    ],
                ),
                0,
            );
            assert_eq!(found["outcome"], "found");
            assert_eq!(found["file"], candidate["file"]);
            assert_eq!(found["declaration"]["anchor"], candidate["anchor"]);
            assert_eq!(found["declaration"]["selector"], candidate["selector"]);
        }
        // The text candidates are absolute `FILE:LINE:COLUMN` operands of
        // show and references, and name their selectors.
        for line in text(&output.stderr).lines().skip(1) {
            let place = line.split(' ').next().unwrap();
            let shown = project.run_in(
                &elsewhere,
                &["--show", "--project", &project.config(), "--at", place],
            );
            assert!(
                text_outcome(&shown, 0).starts_with(&format!("{place} ")),
                "{entry:?} {place}"
            );
            let selector = line.rsplit_once(" (").unwrap().1.strip_suffix(')').unwrap();
            let named = project.run_in(
                &elsewhere,
                &["--show", "--project", &project.config(), "--name", selector],
            );
            if selector != "run" {
                assert_eq!(named.stdout, shown.stdout, "{entry:?} {selector}");
            }
            // A valid reference request: one group, or the member that
            // reference queries do not target.
            let references = project.run_in(
                &elsewhere,
                &[
                    "--references",
                    "--project",
                    &project.config(),
                    "--at",
                    place,
                ],
            );
            if references.status.code() == Some(0) {
                assert!(references.stderr.is_empty(), "{entry:?} {place}");
                let answer = text(&references.stdout);
                assert!(
                    answer.lines().next().unwrap().contains(": 1 group, "),
                    "{entry:?} {place} {answer}"
                );
            } else {
                let diagnostic = text_outcome(&references, 1);
                assert!(diagnostic.contains("(member)"), "{entry:?} {diagnostic}");
            }
        }
        project.check_no_service();
    }
}

#[test]
fn missing_selections_exit_1_naming_what_was_searched() {
    for &entry in ENTRIES {
        let project = Project::new(entry);
        let output = project.run(&["--show", "--name", "absent"]);
        assert_eq!(
            text_outcome(&output, 1),
            format!("{}: no declaration matches `absent`\n", project.config())
        );
        assert_eq!(
            document(&project.run(&["--show", "--json", "--name", "absent"]), 1),
            json!({
                "operation": "show",
                "outcome": "not_found",
                "project": project.config(),
                "query": { "name": "absent" },
            })
        );
        let output = project.run(&["--show", "--at", "3:5", "src/host.ts"]);
        assert_eq!(
            text_outcome(&output, 1),
            format!(
                "{}:3:5: no declaration is named or anchored at this position\n",
                project.path("src/host.ts").display()
            )
        );
        assert_eq!(
            document(
                &project.run(&["--show", "--json", "--at", "3:5", "src/host.ts"]),
                1
            ),
            json!({
                "operation": "show",
                "outcome": "not_found",
                "project": project.config(),
                "file": "src/host.ts",
                "query": { "at": { "line": 3, "column": 5 } },
            })
        );
        project.check_no_service();
    }
}

#[test]
fn analysis_failures_are_typed_outcomes_with_their_exit_codes() {
    for &entry in ENTRIES {
        let project = Project::new(entry);
        let data = project.path("src/data.json");
        // (arguments, exit code, kind, text diagnostic, error path)
        let cases: [(&[&str], i32, &str, String, Value); 5] = [
            (
                &["--outline", "outside/x.ts"],
                2,
                "OutsideProject",
                "outside/x.ts: not one of the project's searched files; the files, include, and exclude of tsconfig.json, or of a configuration it extends, decide which files are searched\n".into(),
                json!("outside/x.ts"),
            ),
            (
                &["--show", "--name", "run", "outside/x.ts"],
                2,
                "OutsideProject",
                "outside/x.ts: not one of the project's searched files; the files, include, and exclude of tsconfig.json, or of a configuration it extends, decide which files are searched\n".into(),
                json!("outside/x.ts"),
            ),
            (
                &["--show", "--at", "99:1", "src/host.ts"],
                2,
                "InvalidPosition",
                "src/host.ts:99:1: line is outside the file\n".into(),
                json!("src/host.ts"),
            ),
            (
                &["--outline", "src/data.json"],
                1,
                "Unsupported",
                format!("{}: JSON files are not navigation input\n", data.display()),
                json!(data),
            ),
            (
                &["--show", "--at", "1:3", "src/data.json"],
                1,
                "Unsupported",
                format!("{}: JSON files are not navigation input\n", data.display()),
                json!(data),
            ),
        ];
        for (args, code, kind, diagnostic, path) in cases {
            assert_eq!(
                text_outcome(&project.run(args), code),
                diagnostic,
                "{entry:?} {args:?}"
            );
            let mut json_args = args.to_vec();
            json_args.insert(1, "--json");
            let json = document(&project.run(&json_args), code);
            assert_eq!(
                json,
                json!({
                    "operation": args[0].trim_start_matches('-'),
                    "outcome": "error",
                    "project": "tsconfig.json",
                    "error": {
                        "kind": kind,
                        "message": diagnostic.split_once(": ").unwrap().1.trim_end(),
                        "path": path,
                        "position": if kind == "InvalidPosition" {
                            json!({ "line": 99, "column": 1 })
                        } else {
                            Value::Null
                        },
                    },
                }),
                "{entry:?} {args:?}"
            );
        }
        project.check_no_service();
    }
}

#[test]
fn project_failures_answer_no_operation_and_never_fall_back_to_the_file() {
    for &entry in ENTRIES {
        let project = Project::new(entry);
        let broken = project.path("src/broken.ts");
        std::fs::write(&broken, "const = 1;\n").unwrap();
        let diagnostic = format!("{}:1:7: Expected a binding identifier\n", broken.display());
        for args in [
            &["--outline", "src/host.ts"][..],
            &["--show", "--name", "run"],
            &["--show", "--at", "1:17", "src/other.ts"],
            &["--references", "--name", "run"],
        ] {
            assert_eq!(
                text_outcome(&project.run(args), 1),
                diagnostic,
                "{entry:?} {args:?}"
            );
            if args[0] != "--references" {
                let mut json_args = args.to_vec();
                json_args.insert(1, "--json");
                let json = document(&project.run(&json_args), 1);
                assert_eq!(json["outcome"], "error", "{entry:?} {args:?}");
                assert_eq!(json["project"], "tsconfig.json");
                assert_eq!(
                    json["error"],
                    json!({
                        "kind": "Syntax",
                        "message": "Expected a binding identifier",
                        "path": broken,
                        "position": { "line": 1, "column": 7 },
                    })
                );
            }
        }
        // An unreadable configuration is an I/O failure.
        let output = project.run_in(
            project.root(),
            &[
                "--outline",
                "--json",
                "--project",
                "missing.json",
                "src/host.ts",
            ],
        );
        let json = document(&output, 2);
        assert_eq!(json["project"], "missing.json");
        assert_eq!(json["error"]["kind"], "Io");
        project.check_no_service();
        if entry == Entry::Service {
            let mut stop = project.service.command();
            stop.args(["--references-stop", "--project", "missing.json"]);
            assert!(stop.output().unwrap().status.success());
        }
    }
}

#[test]
fn output_files_receive_results_and_json_outcomes_but_never_text_failures() {
    for &entry in ENTRIES {
        let project = Project::new(entry);
        let out = project.path("out.txt");
        let output = project.run(&["--outline", "src/host.ts", "-o", "out.txt"]);
        assert_eq!(output.status.code(), Some(0), "{entry:?} {output:?}");
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
        assert!(
            std::fs::read_to_string(&out)
                .unwrap()
                .contains("createRunHost")
        );

        // Text failures leave the previous file as it was.
        for args in [
            &["--show", "--name", "run", "-o", "out.txt"][..],
            &["--show", "--name", "absent", "-o", "out.txt"],
            &["--outline", "outside/x.ts", "-o", "out.txt"],
            &["--outline", "src/data.json", "-o", "out.txt"],
            &[
                "--references",
                "--at",
                "99:1",
                "src/host.ts",
                "-o",
                "out.txt",
            ],
            &[
                "--references",
                "--at",
                "1:17",
                "src/data.json",
                "-o",
                "out.txt",
            ],
        ] {
            std::fs::write(&out, "previous result\n").unwrap();
            let output = project.run(args);
            assert_ne!(output.status.code(), Some(0), "{entry:?} {args:?}");
            assert!(output.stdout.is_empty(), "{entry:?} {args:?}");
            assert!(!output.stderr.is_empty(), "{entry:?} {args:?}");
            assert_eq!(
                std::fs::read_to_string(&out).unwrap(),
                "previous result\n",
                "{entry:?} {args:?}"
            );
        }
        std::fs::remove_file(&out).unwrap();
        for args in [
            &["--show", "--name", "run", "-o", "out.txt"][..],
            &["--outline", "outside/x.ts", "-o", "out.txt"],
        ] {
            let output = project.run(args);
            assert_ne!(output.status.code(), Some(0), "{entry:?} {args:?}");
            assert!(!out.exists(), "{entry:?} {args:?} created the output file");
        }

        // JSON outcomes go to the file, with nothing on stdout or stderr.
        for (args, code, outcome) in [
            (
                &["--show", "--json", "--name", "run", "-o", "out.txt"][..],
                1,
                "ambiguous",
            ),
            (
                &["--show", "--json", "--name", "absent", "-o", "out.txt"],
                1,
                "not_found",
            ),
            (
                &["--outline", "--json", "outside/x.ts", "-o", "out.txt"],
                2,
                "error",
            ),
            (
                &["--show", "--json", "--name", "Tools.run", "-o", "out.txt"],
                0,
                "found",
            ),
        ] {
            let output = project.run(args);
            assert_eq!(output.status.code(), Some(code), "{entry:?} {args:?}");
            assert!(output.stdout.is_empty() && output.stderr.is_empty());
            let json: Value =
                serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
            assert_eq!(json["outcome"], outcome, "{entry:?} {args:?}");
            std::fs::remove_file(&out).unwrap();
        }

        // A result that cannot be written is one stderr diagnostic.
        for args in [
            &["--outline", "--json", "src/host.ts"][..],
            &["--show", "--json", "--name", "run"],
            &["--outline", "--json", "outside/x.ts"],
            &["--outline", "src/host.ts"],
            &["--references", "--name", "run"],
        ] {
            let output = project.run(&[args, &["-o", "missing-dir/out.json"]].concat());
            assert_eq!(output.status.code(), Some(2), "{entry:?} {args:?}");
            assert!(output.stdout.is_empty(), "{entry:?} {args:?}");
            let message = text(&output.stderr);
            assert!(
                message.starts_with("write.output: missing-dir/out.json: "),
                "{entry:?} {args:?}: {message}"
            );
            assert_eq!(message.lines().count(), 1, "no second document: {message}");
        }
        project.check_no_service();
    }
}

#[test]
fn unmatched_reference_names_remain_successful_empty_results() {
    for &entry in ENTRIES {
        let project = Project::new(entry);
        let output = project.run(&["--references", "--name", "absent", "--json"]);
        assert_eq!(output.status.code(), Some(0), "{entry:?} {output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(
            document(&output, 0),
            json!({
                "operation": "references", "outcome": "complete",
                "project": project.config(),
                "selection": { "name": { "name": "absent", "file": null } },
                "groups": [],
                "filters": { "calls": false, "excludeTests": false },
                "coverage": { "baseline": "lexical", "status": "complete", "gaps": [] },
                "unsearchedMembers": { "count": 0, "selectors": [] },
            })
        );
        project.check_no_service();
    }
}

#[cfg(unix)]
#[test]
fn transport_failures_are_typed_and_delivered_as_the_request_asks() {
    use std::os::unix::fs::PermissionsExt;
    let project = Project::new(Entry::Service);
    std::fs::create_dir(&project.service.runtime).unwrap();
    std::fs::set_permissions(
        &project.service.runtime,
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let runtime = project.service.runtime.to_str().unwrap().to_owned();
    // Every transport failure ends by naming the doctor for its project.
    let not_private = format!(
        "Project service directory must be a private directory owned by this user with mode 0700: {runtime}; run: tsparse --doctor --project tsconfig.json"
    );
    let remedy = vec![
        "  why: any process that can open the socket can run queries as you".to_string(),
        format!("  next: chmod 700 {runtime}"),
    ];
    // Text on stderr, which never creates -o; with --json, a document.
    for (args, json) in [
        (&["--outline", "src/host.ts"][..], None),
        (&["--outline", "--json", "src/host.ts"], Some("outline")),
        (&["--show", "--json", "--name", "run"], Some("show")),
        (&["--type", "--json", "--name", "run"], Some("type")),
        (
            &["--show", "--at", "1:17", "src/other.ts", "-o", "out.txt"],
            None,
        ),
        (&["--references", "--name", "run"], None),
        (
            &["--references", "--json", "--name", "run"],
            Some("references"),
        ),
    ] {
        let output = project.run(args);
        assert_eq!(
            transport(&output, json, "transport.directory"),
            (not_private.clone(), remedy.clone()),
            "{args:?}"
        );
        assert!(!project.path("out.txt").exists(), "{args:?}");
    }
    // A valid request's failure document goes where its result would.
    let output = project.run(&["--show", "--json", "--name", "run", "-o", "out.txt"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        output.stdout.is_empty() && output.stderr.is_empty(),
        "{output:?}"
    );
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(project.path("out.txt")).unwrap()).unwrap();
    assert_eq!(
        written,
        json!({
            "operation": "show",
            "outcome": "error",
            "error": {
                "category": "transport",
                "code": "transport.directory",
                "message": not_private,
                "why": "any process that can open the socket can run queries as you",
                "next": {"argv": ["chmod", "700", runtime]},
            },
        })
    );
    // Direct execution does not use the service.
    let output = project
        .service
        .command()
        .args([
            "--outline",
            "src/host.ts",
            "--project",
            "tsconfig.json",
            "--direct",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

/// A runtime directory whose socket path is longer than a Unix socket
/// address holds fails every service request before a service is started,
/// and the diagnostic names the socket and the remedy.
#[cfg(unix)]
#[test]
fn an_overlong_service_socket_path_names_the_socket_and_the_remedy() {
    let project = Project::new(Entry::Service);
    let runtime = project.root().join("r".repeat(120));
    // The platform's refusal of a socket path this long.
    let cause =
        std::os::unix::net::UnixStream::connect(runtime.join("0123456789abcdef.sock")).unwrap_err();
    for (args, json) in [
        (&["--outline", "src/host.ts"][..], None),
        (&["--show", "--json", "--name", "run"], Some("show")),
        (&["--references", "--name", "run"], None),
        (&["--references-status"], None),
    ] {
        let output = project
            .service
            .command()
            .env("TSPARSE_SERVICE_DIR", &runtime)
            .args(args)
            .args(["--project", "tsconfig.json"])
            .output()
            .unwrap();
        let (message, lines) = transport(&output, json, "transport.socket");
        // The socket is named by the endpoint's 16-digit key.
        let key = message
            .strip_prefix(&format!("Project service socket {}/", runtime.display()))
            .map(|rest| rest.split_at(16));
        assert!(
            key.is_some_and(|(key, rest)| key.bytes().all(|b| b.is_ascii_hexdigit())
                && rest
                    == format!(".sock: {cause}; run: tsparse --doctor --project tsconfig.json")),
            "{args:?}: {message}"
        );
        assert_eq!(
            lines,
            ["  next: set a shorter TSPARSE_SERVICE_DIR"],
            "{args:?}"
        );
    }
    // No service was started: it would have left its lock file.
    assert_eq!(std::fs::read_dir(&runtime).unwrap().count(), 0);
    // Following the remedy: a shorter directory serves the same request.
    let output = project.run(&["--outline", "src/host.ts"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[test]
fn dotted_reference_names_select_catalog_declarations() {
    let project = Project::new(Entry::Direct);
    for args in [
        &["--references", "--name", "createRunHost.run"][..],
        &["--references", "--name", "Tools.run", "src/other.ts"],
    ] {
        let output = project.run(&[args, &["--json"]].concat());
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{args:?}");
        let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(answer["operation"], "references");
        assert_eq!(answer["outcome"], "complete", "{answer:#}");
        assert_eq!(
            answer["coverage"]["baseline"],
            if args[2] == "Tools.run" {
                "lexical"
            } else {
                "member"
            }
        );
        assert_eq!(answer["groups"].as_array().unwrap().len(), 1);
    }
    project.check_no_service();
}

#[test]
fn valid_project_requests_and_usage_errors() {
    let project = Project::new(Entry::Direct);
    // Each is a valid request: whatever its outcome, it is not a usage
    // error, so it never exits 2 with an empty stdout and the usage text.
    for args in [
        &["--outline", "--project", "tsconfig.json", "src/host.ts"][..],
        &[
            "--outline",
            "--direct",
            "--project",
            "tsconfig.json",
            "--depth",
            "all",
            "--json",
            "--pretty",
            "src/host.ts",
        ],
        &[
            "--show",
            "--project",
            "tsconfig.json",
            "--name",
            "Tools.run",
        ],
        &[
            "--show",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "src/host.ts",
        ],
        &[
            "--show",
            "--project",
            "tsconfig.json",
            "--at",
            "8:10",
            "src/host.ts",
            "--json",
        ],
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "--direct",
        ],
    ] {
        let mut command = project.service.command();
        command.args(args);
        if !args.contains(&"--direct") {
            command.arg("--direct");
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
    }

    let usage = [
        // Outline names exactly one file and takes no selector.
        &["--outline", "--project", "tsconfig.json"][..],
        &["--outline", "--project", "tsconfig.json", "-"],
        &[
            "--outline",
            "--project",
            "tsconfig.json",
            "src/host.ts",
            "src/other.ts",
        ],
        &[
            "--outline",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "src/host.ts",
        ],
        &[
            "--outline",
            "--project",
            "tsconfig.json",
            "--at",
            "1:1",
            "src/host.ts",
        ],
        &[
            "--outline",
            "--project",
            "tsconfig.json",
            "--depth",
            "0",
            "src/host.ts",
        ],
        &["--outline", "--project"],
        // Show takes exactly one selector; a position needs its file.
        &["--show", "--project", "tsconfig.json", "src/host.ts"],
        &["--show", "--project", "tsconfig.json", "--at", "1:1"],
        &[
            "--show",
            "--project",
            "tsconfig.json",
            "--at",
            "1:1",
            "--name",
            "run",
            "src/host.ts",
        ],
        &["--show", "--project", "tsconfig.json", "--name", "run", "-"],
        &[
            "--show",
            "--project",
            "tsconfig.json",
            "--depth",
            "2",
            "--name",
            "run",
        ],
        // A value option given twice conflicts with itself.
        &[
            "--show",
            "--project",
            "tsconfig.json",
            "--project",
            "other/tsconfig.json",
            "--name",
            "run",
        ],
        &[
            "--show",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "--name",
            "Tools.run",
        ],
        &[
            "--outline",
            "--project",
            "tsconfig.json",
            "src/host.ts",
            "-o",
            "other.txt",
        ],
        // Options outside their rows.
        &[
            "--show",
            "--project",
            "tsconfig.json",
            "--pretty",
            "--name",
            "run",
        ],
        &[
            "--show",
            "--project",
            "tsconfig.json",
            "--check",
            "--name",
            "run",
        ],
        &[
            "--outline",
            "--project",
            "tsconfig.json",
            "--tsx",
            "src/host.ts",
        ],
        &["--show", "--direct", "--name", "run", "src/host.ts"],
        &["--outline", "--direct", "src/host.ts"],
        &["--direct", "src/host.ts"],
        &["--project", "tsconfig.json", "src/host.ts"],
        &[
            "--references-status",
            "--project",
            "tsconfig.json",
            "--direct",
        ],
        // Conflicting operations.
        &[
            "--show",
            "--references-direct",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
        ],
        &[
            "--outline",
            "--show",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
            "src/host.ts",
        ],
        &[
            "--outline",
            "--references-status",
            "--project",
            "tsconfig.json",
        ],
    ];
    for args in usage {
        let output = project
            .service
            .command()
            .args(["-o", "out.txt"])
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert_eq!(text(&output.stderr).lines().count(), 1, "{args:?}");
        assert!(!project.path("out.txt").exists(), "{args:?}");
        // A usage error starts nothing.
        assert!(!project.service.runtime.exists(), "{args:?}");
    }
}

#[test]
fn files_outside_the_configuration_directory_are_named_through_it() {
    // A searched file outside the configuration's directory is `../`
    // relative to it in JSON, and absolute without `..` in text; both
    // select it again.
    let fixture = Fixture::new(&[
        (
            "app/tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["src", "../shared"] }"#,
        ),
        ("app/src/main.ts", "export const main = 1;\n"),
        ("shared/util.ts", "export function util() {}\n"),
    ]);
    for &entry in ENTRIES {
        let project = Project::of(Fixture::existing(&fixture.root), entry);
        let config = project.path("app/tsconfig.json");
        let config = config.to_str().unwrap();
        let util = project.path("shared/util.ts");
        let output = project.run_in(
            project.root(),
            &["--outline", "--project", config, "shared/util.ts"],
        );
        assert_eq!(
            text_outcome(&output, 0),
            format!("{}\n1:17 function export function util()\n", util.display()),
            "{entry:?}"
        );
        let json = document(
            &project.run_in(
                project.root(),
                &["--show", "--json", "--project", config, "--name", "util"],
            ),
            0,
        );
        assert_eq!(json["project"], config);
        assert_eq!(json["file"], "../shared/util.ts");
        let joined = project.path("app").join(json["file"].as_str().unwrap());
        for file in [joined.to_str().unwrap(), util.to_str().unwrap()] {
            let output = project.run_in(
                project.path("app/src").as_path(),
                &["--show", "--project", config, "--at", "1:17", file],
            );
            assert_eq!(
                text_outcome(&output, 0),
                format!(
                    "{}:1:17 function util\n1 | export function util() {{}}\n",
                    util.display()
                ),
                "{entry:?} {file}"
            );
        }
        project.check_no_service();
        // The service this spelling started is not the one the fixture
        // stops, which `tsconfig.json` names.
        if entry == Entry::Service {
            let stopped =
                project.run_in(project.root(), &["--references-stop", "--project", config]);
            assert!(stopped.status.success(), "{stopped:?}");
        }
    }
}

/// A project is the configuration `--project` names, relative to the
/// working directory: no ancestor directory is searched, even one holding a
/// `tsconfig.json`, through the service or loaded directly, and without
/// `--project` nothing is searched for (design, "Preserved behavior":
/// explicit project identity).
#[test]
fn no_ancestor_directory_is_searched_for_a_configuration() {
    let service = Service::new(&[("child/app.ts", "export const value = 1;\n")]);
    let child = service.fixture.path("child");
    let named = format!(
        "{}: No such file or directory (os error 2)\n",
        child.join("tsconfig.json").display()
    );
    for args in [
        &["--references", "--name", "value"][..],
        &["--references", "--name", "value", "--direct"],
        &["--type", "--name", "value"],
        &["--outline", "app.ts"],
        &["--show", "--name", "value", "app.ts", "--direct"],
    ] {
        let output = service
            .command()
            .current_dir(&child)
            .args(args)
            .args(["--project", "tsconfig.json"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stderr), named, "{args:?}");
    }
    let output = service
        .command()
        .current_dir(&child)
        .args(["--references", "--name", "value"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "usage.missing_flag: --references requires --project TSCONFIG\n"
    );
    // From the directory that holds it, the same configuration is found.
    let found = service.run(&["--references", "--name", "value", "--json"]);
    assert_eq!(found.status.code(), Some(0), "{found:?}");
}

/// A service is keyed by the configuration as spelled: two spellings of one
/// file, here a symbolic link beside it, each own a service, which only
/// that spelling's queries, status, and stop reach. A spelling through a
/// link can be retargeted, so this is deliberate (design, "Preserved
/// behavior": explicit project identity).
#[cfg(unix)]
#[test]
fn each_spelling_of_a_configuration_owns_its_own_service() {
    let service = Service::new(&[("app.ts", "export const value = 1;\n")]);
    std::os::unix::fs::symlink("tsconfig.json", service.fixture.path("link.json")).unwrap();
    let request = |args: &[&str], config: &str| {
        service
            .command()
            .args(args)
            .args(["--project", config])
            .output()
            .unwrap()
    };
    let status =
        |config: &str| support::service::json(request(&["--references-status", "--json"], config));
    let query = |config: &str| {
        support::service::json(request(
            &["--references", "--name", "value", "--json"],
            config,
        ))
    };
    let answer = query("tsconfig.json");
    assert_eq!(status("link.json")["running"], false);
    assert_eq!(query("link.json")["groups"], answer["groups"]);
    let (spelled, linked) = (status("tsconfig.json"), status("link.json"));
    assert_eq!(
        (&spelled["running"], &linked["running"]),
        (&json!(true), &json!(true))
    );
    assert_ne!(spelled["pid"], linked["pid"]);
    let stopped = request(&["--references-stop"], "link.json");
    assert_eq!(
        String::from_utf8_lossy(&stopped.stdout),
        format!("stopped: pid {}\n", linked["pid"])
    );
    assert_eq!(status("link.json")["running"], false);
    assert_eq!(status("tsconfig.json")["pid"], spelled["pid"]);
}
