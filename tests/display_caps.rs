//! Display caps (design, step 4: "Rules shared by every renderer" and
//! "Outline, candidates, show, previews"). Each bounded list is shown just
//! under, at, and over the default its operation publishes in
//! `--OP --help --json`, in text and JSON, with and without `-o`, and for a
//! project through its service and loaded directly. Every cut's printed
//! command, a `--limit all` re-run of the request as given (the run
//! owner's answer to question a61fe38c), is followed as printed from the
//! same directory: it must recover every withheld item and create or
//! replace no file but a query's own `-o`.
#![cfg(unix)]
mod support;

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use support::service::Service;

fn binary() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_tsparse"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// `args` from the fixture's directory, with its service's runtime
/// directory; file navigation and edits in files never reach it.
fn run(service: &Service, args: &[&str]) -> Output {
    service.command().args(args).output().unwrap()
}

/// `args` with `stdin` on standard input.
fn run_with(service: &Service, args: &[&str], stdin: &str) -> Output {
    use std::io::Write as _;
    let mut child = service
        .command()
        .args(args)
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

/// A result: exit `code`, nothing on stderr, and stdout.
fn stdout(output: &Output, code: i32) -> String {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    text(&output.stdout)
}

/// A diagnostic: exit `code`, nothing on stdout, and stderr.
fn stderr(output: &Output, code: i32) -> String {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    text(&output.stderr)
}

fn document(output: &Output, code: i32) -> Value {
    serde_json::from_str(&stdout(output, code)).unwrap()
}

/// The default bound of `list` that the operation `selector` names
/// publishes in its help document: tests derive under, at, and over from
/// it, never from a constant of their own.
fn default(selector: &[&str], list: &str) -> usize {
    let output = Command::new(binary())
        .args(selector)
        .args(["--help", "--json"])
        .output()
        .unwrap();
    let help: Value = serde_json::from_slice(&output.stdout).unwrap();
    let limits = help["limits"].as_array().unwrap();
    let limit = limits
        .iter()
        .find(|limit| limit["list"] == list)
        .unwrap_or_else(|| panic!("{selector:?} publishes no {list} limit: {limits:?}"));
    limit["default"].as_u64().unwrap() as usize
}

/// Every file under `root` with its bytes, to show that following a
/// printed command writes nothing it should not.
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    files
}

/// The command a cut line prints after `see all with: `, without the
/// spillover alternative that may follow it.
fn printed_command(line: &str) -> &str {
    let (_, command) = line
        .split_once("see all with: ")
        .unwrap_or_else(|| panic!("no command: {line}"));
    command
        .strip_suffix(", or write them to a file with -o FILE")
        .unwrap_or(command)
}

/// Follow `command`, a printed command line, through `sh -c` from the
/// fixture's directory with this build first on `PATH`, giving it `stdin`:
/// its output, after checking that it wrote no file but `output`, the
/// query's own `-o`.
fn follow(service: &Service, command: &str, stdin: &str, output: Option<&str>) -> Output {
    use std::io::Write as _;
    let root = &service.fixture.root;
    let before = files(root);
    let path = format!(
        "{}:{}",
        binary().parent().unwrap().display(),
        std::env::var("PATH").unwrap()
    );
    let mut child = service
        .command()
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn_shell(command);
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let followed = child.wait_with_output().unwrap();
    let mut after = files(root);
    let mut before = before;
    if let Some(output) = output {
        before.remove(&root.join(output));
        after.remove(&root.join(output));
    }
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "{command}"
    );
    assert!(before == after, "{command} replaced a file");
    followed
}

/// Follow a document's `display.next.argv` as given, from the fixture's
/// directory: its program name is the literal `tsparse`, this build.
fn follow_argv(service: &Service, argv: &Value, stdin: &str, output: Option<&str>) -> Output {
    let argv: Vec<String> = argv
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| arg.as_str().unwrap().to_string())
        .collect();
    assert_eq!(argv[0], "tsparse", "{argv:?}");
    let words: Vec<String> = argv
        .iter()
        .map(|arg| format!("'{}'", arg.replace('\'', r"'\''")))
        .collect();
    follow(service, &words.join(" "), stdin, output)
}

trait SpawnShell {
    fn spawn_shell(&mut self, command: &str) -> std::process::Child;
}

impl SpawnShell for Command {
    /// This command's environment and directory, running `command` with
    /// `sh -c` instead of its program.
    fn spawn_shell(&mut self, command: &str) -> std::process::Child {
        let mut shell = Command::new("sh");
        shell.arg("-c").arg(command);
        if let Some(directory) = self.get_current_dir() {
            shell.current_dir(directory);
        }
        for (key, value) in self.get_envs() {
            match value {
                Some(value) => shell.env(key, value),
                None => shell.env_remove(key),
            };
        }
        shell
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }
}

/// The last line of `text`, and the lines before it.
fn last_line(text: &str) -> (&str, &str) {
    let text = text.strip_suffix('\n').unwrap_or(text);
    match text.rsplit_once('\n') {
        Some((before, last)) => (before, last),
        None => ("", text),
    }
}

/// `args` through the project's service and loaded with `--direct`, which
/// must agree byte for byte, the command a cut prints included: the direct
/// output.
fn both(service: &Service, args: &[&str]) -> Output {
    let served = run(service, &[args, &["--project", "tsconfig.json"]].concat());
    let direct = run(
        service,
        &[args, &["--project", "tsconfig.json", "--direct"]].concat(),
    );
    assert_eq!(served.status.code(), direct.status.code(), "{args:?}");
    for (served, direct) in [
        (&served.stdout, &direct.stdout),
        (&served.stderr, &direct.stderr),
    ] {
        assert!(
            served == direct,
            "{args:?}: {} != {}",
            String::from_utf8_lossy(served),
            String::from_utf8_lossy(direct)
        );
    }
    direct
}

/// `count` top-level declarations, one per line; `long` ones have a name
/// and a signature of several hundred characters each.
fn declarations(count: usize, long: bool) -> String {
    let mut source = String::new();
    for index in 0..count {
        if long {
            let name = format!("declaration{index}{}", "N".repeat(300));
            let parameter = format!("parameter: {}", "Long".repeat(100));
            source.push_str(&format!("export function {name}({parameter}): void {{}}\n"));
        } else {
            source.push_str(&format!("export const v{index} = {index};\n"));
        }
    }
    source
}

/// The published defaults: every operation that bounds a list accepts
/// `--limit`, and every other refuses it.
#[test]
fn each_operation_publishes_the_lists_it_bounds_and_their_defaults() {
    let limits = |selector: &[&str]| -> Value {
        let output = Command::new(binary())
            .args(selector)
            .args(["--help", "--json"])
            .output()
            .unwrap();
        let help: Value = serde_json::from_slice(&output.stdout).unwrap();
        help["limits"].clone()
    };
    let list = |name: &str, default: u64| json!({"list": name, "default": default});
    for (selector, expected) in [
        (&["--outline"][..], json!([list("entries", 200)])),
        (&["--test-outline"], json!([list("entries", 200)])),
        (
            &["--show"],
            json!([list("lines", 400), list("candidates", 50)]),
        ),
        (&["--type"], json!([list("candidates", 50)])),
        (&["--references"], json!([list("occurrences", 200)])),
        (
            &["--constructs"],
            json!([
                list("sites", 200),
                list("lacks", 20),
                list("gaps", 50),
                list("candidates", 50)
            ]),
        ),
        (
            &["--replace-body"],
            json!([list("candidates", 50), list("diff_lines", 400)]),
        ),
        (
            &["--insert-after"],
            json!([list("candidates", 50), list("diff_lines", 400)]),
        ),
        (
            &["--edit-batch", "EDITS"],
            json!([list("candidates", 50), list("diff_lines", 400)]),
        ),
        (&["--apply", "PLAN"], json!([])),
        (&["--references-status"], json!([])),
    ] {
        let published = limits(selector);
        let mut summaries = Vec::new();
        for limit in published.as_array().unwrap() {
            summaries.push(limit["summary"].as_str().unwrap().to_string());
        }
        let bare: Vec<Value> = published
            .as_array()
            .unwrap()
            .iter()
            .map(|limit| json!({"list": limit["list"], "default": limit["default"]}))
            .collect();
        assert_eq!(Value::from(bare), expected, "{selector:?}");
        assert!(summaries.iter().all(|summary| !summary.is_empty()));
    }
    let registry = limits(&[]);
    assert_eq!(
        registry,
        Value::Null,
        "the index is no operation's document"
    );
    let output = Command::new(binary())
        .args(["--help", "--json"])
        .output()
        .unwrap();
    let registry: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(registry["operations"][0]["name"], "parse");
    assert_eq!(registry["operations"][0]["limits"], json!([]));
    // An operation without a bounded list refuses the flag, naming those
    // that take it.
    let service = Service::new(&[]);
    let output = run(&service, &["--apply", "plan.json", "--limit", "all"]);
    assert_eq!(
        stderr(&output, 2),
        "usage.flag_scope: --limit applies to: --outline, --test-outline, --show, --type, --references, --constructs, --replace-body, --insert-after, --edit-batch\n"
    );
    for value in ["0", "-1", "two", "", "ALL", "1.5"] {
        let output = run(&service, &["--outline", "app.ts", "--limit", value]);
        assert_eq!(
            stderr(&output, 2),
            format!("usage.invalid_value: --limit expects a positive number or all, got {value}\n"),
            "{value:?}"
        );
    }
}

/// Outline entries, counted after `--depth` chooses the levels, in a wide
/// shallow file and one of long names and signatures.
#[test]
fn an_outline_shows_its_default_number_of_entries_and_names_the_rest() {
    let limit = default(&["--outline"], "entries");
    for long in [false, true] {
        for count in [limit - 1, limit, limit + 1] {
            let source = declarations(count, long);
            let service = Service::new(&[("app.ts", &source)]);
            let root = service.fixture.root.clone();
            let complete = stdout(
                &run(&service, &["--outline", "app.ts", "--limit", "all"]),
                0,
            );
            assert_eq!(complete.lines().count(), count + 1);
            let shown = stdout(&run(&service, &["--outline", "app.ts"]), 0);
            let documented = document(&run(&service, &["--outline", "app.ts", "--json"]), 0);
            let all = document(
                &run(
                    &service,
                    &["--outline", "app.ts", "--json", "--limit", "all"],
                ),
                0,
            );
            assert_eq!(all.get("display"), None);
            assert_eq!(all["entries"].as_array().unwrap().len(), count);
            if count <= limit {
                assert_eq!(shown, complete, "{count}");
                assert_eq!(documented, all, "{count}");
                continue;
            }
            // Over: the default number of entries, then the cut line.
            let (lines, cut) = last_line(&shown);
            assert_eq!(format!("{lines}\n"), {
                let mut kept: Vec<&str> = complete.lines().take(limit + 1).collect();
                kept.push("");
                kept.join("\n")
            });
            assert_eq!(
                cut,
                "… 1 more entry; see all with: tsparse --outline app.ts --limit all, or write them to a file with -o FILE"
            );
            let followed = follow(&service, printed_command(cut), "", None);
            assert_eq!(stdout(&followed, 0), complete);
            // JSON says what it withheld and the command that shows it.
            let entries = documented["entries"].as_array().unwrap();
            assert_eq!(entries.len(), limit);
            assert_eq!(entries[..], all["entries"].as_array().unwrap()[..limit]);
            assert_eq!(
                documented["display"],
                json!({
                    "entries": {"total": count, "returned": limit},
                    "truncated": true,
                    "next": {"argv": ["tsparse", "--outline", "app.ts", "--json", "--limit", "all"]},
                })
            );
            let mut without = documented.clone();
            without.as_object_mut().unwrap().remove("display");
            without["entries"] = all["entries"].clone();
            assert_eq!(without, all);
            let followed = follow_argv(&service, &documented["display"]["next"]["argv"], "", None);
            assert_eq!(document(&followed, 0), all);
            // With -o and no --limit nothing is cut; an explicit --limit
            // cuts there too, and its command rewrites that file whole.
            assert!(
                stdout(&run(&service, &["--outline", "app.ts", "-o", "out.txt"]), 0).is_empty()
            );
            assert_eq!(
                std::fs::read_to_string(root.join("out.txt")).unwrap(),
                complete
            );
            assert!(
                stdout(
                    &run(
                        &service,
                        &["--outline", "app.ts", "--json", "-o", "out.json"]
                    ),
                    0
                )
                .is_empty()
            );
            let spilled: Value =
                serde_json::from_str(&std::fs::read_to_string(root.join("out.json")).unwrap())
                    .unwrap();
            assert_eq!(spilled, all);
            let output = run(
                &service,
                &["--outline", "app.ts", "-o", "out.txt", "--limit", "3"],
            );
            assert!(stdout(&output, 0).is_empty());
            let written = std::fs::read_to_string(root.join("out.txt")).unwrap();
            let (lines, cut) = last_line(&written);
            assert_eq!(lines.lines().count(), 4);
            assert_eq!(
                cut,
                format!(
                    "… {} more entries; see all with: tsparse --outline app.ts -o out.txt --limit all",
                    count - 3
                )
            );
            assert!(
                stdout(
                    &follow(&service, printed_command(cut), "", Some("out.txt")),
                    0
                )
                .is_empty()
            );
            // The caller's text file, now whole, and still text.
            assert_eq!(
                std::fs::read_to_string(root.join("out.txt")).unwrap(),
                complete
            );
            // A project's outline, through the service and directly.
            let output = both(&service, &["--outline", "app.ts"]);
            let project = stdout(&output, 0);
            let (_, cut) = last_line(&project);
            assert_eq!(
                cut,
                "… 1 more entry; see all with: tsparse --outline app.ts --project tsconfig.json --limit all, or write them to a file with -o FILE"
            );
            let followed = stdout(&follow(&service, printed_command(cut), "", None), 0);
            assert_eq!(followed.lines().count(), count + 1);
            assert!(!followed.contains("… "), "{followed}");
        }
    }
}

/// `--limit` counts the entries `--depth` shows, so hidden levels never
/// use the bound, and the count line of a hidden level stays.
#[test]
fn outline_entries_are_counted_after_depth_chooses_the_levels() {
    let limit = default(&["--outline"], "entries");
    // `limit` classes of one member each: `2 × limit` entries at two levels.
    let mut source = String::new();
    for index in 0..limit {
        source.push_str(&format!(
            "export class C{index} {{\n  m{index}() {{}}\n}}\n"
        ));
    }
    let service = Service::new(&[("app.ts", &source)]);
    let one = stdout(&run(&service, &["--outline", "app.ts", "--depth", "1"]), 0);
    assert!(!one.contains("more entr"), "{one}");
    assert_eq!(one.lines().count(), 1 + 2 * limit);
    assert!(one.ends_with("  … 1 more child\n"), "{one}");
    let two = stdout(&run(&service, &["--outline", "app.ts"]), 0);
    let (lines, cut) = last_line(&two);
    assert_eq!(lines.lines().count(), 1 + limit);
    assert_eq!(
        cut,
        format!(
            "… {limit} more entries; see all with: tsparse --outline app.ts --limit all, or write them to a file with -o FILE"
        )
    );
    // A cut inside a class keeps the entries before it at their levels.
    let three = stdout(&run(&service, &["--outline", "app.ts", "--limit", "3"]), 0);
    let lines: Vec<&str> = three.lines().collect();
    assert_eq!(lines.len(), 5, "{three}");
    assert!(lines[1].starts_with("1:14 class "), "{three}");
    assert!(lines[2].starts_with("  2:3 method m0"), "{three}");
    assert!(lines[3].starts_with("4:14 class "), "{three}");
    assert!(
        lines[4].starts_with(&format!("… {} more entries; ", 2 * limit - 3)),
        "{three}"
    );
    // In JSON the kept entries keep their nesting.
    let documented = document(
        &run(&service, &["--outline", "app.ts", "--json", "--limit", "3"]),
        0,
    );
    assert_eq!(
        documented["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["children"].as_array().unwrap().len())
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(
        documented["display"]["entries"],
        json!({"total": 2 * limit, "returned": 3})
    );
}

/// An import declaration's entries take one text line, which lists at most
/// eight names and counts the others. Where the cut falls inside it, the
/// line lists the names before the cut and counts the declaration's others
/// the same way, so it never reads as the whole import. The command shows
/// the outline in the same form: every withheld entry is on the import's
/// line, by name up to the eighth and in its count after that.
#[test]
fn a_cut_inside_an_import_declaration_counts_its_other_names() {
    let limit = default(&["--outline"], "entries");
    let import = |count: usize| {
        let names: Vec<String> = (0..count).map(|index| format!("n{index}")).collect();
        format!("import {{ {} }} from \"./other\";\n", names.join(", "))
    };
    // The names a line lists, and its count of the others.
    let line = |listed: usize, count: usize| {
        let names: Vec<String> = (0..listed).map(|index| format!("n{index}")).collect();
        format!(
            "1:10 import {}, … {} more",
            names.join(", "),
            count - listed
        )
    };
    for (count, args, listed, withheld) in [
        (12, &["--outline", "app.ts", "--limit", "3"][..], 3, 9),
        (limit + 1, &["--outline", "app.ts"], 8, 1),
    ] {
        let service = Service::new(&[("app.ts", &import(count))]);
        let file = service.fixture.path("app.ts").display().to_string();
        let complete = stdout(
            &run(&service, &["--outline", "app.ts", "--limit", "all"]),
            0,
        );
        assert_eq!(complete, format!("{file}\n{}\n", line(8, count)));
        let shown = stdout(&run(&service, args), 0);
        let noun = if withheld == 1 { "entry" } else { "entries" };
        assert_eq!(
            shown,
            format!(
                "{file}\n{}\n… {withheld} more {noun}; see all with: tsparse --outline app.ts --limit all, or write them to a file with -o FILE\n",
                line(listed, count)
            ),
            "{args:?}"
        );
        let (_, cut) = last_line(&shown);
        let followed = stdout(&follow(&service, printed_command(cut), "", None), 0);
        assert_eq!(followed, complete, "{args:?}");
        // The withheld entries: named up to the eighth, and counted after.
        let kept = count - withheld;
        for index in kept..8 {
            assert!(followed.contains(&format!(" n{index},")), "{followed}");
        }
        assert!(followed.contains(&format!(", … {} more\n", count - 8)));
    }
}

/// The first `count` lines of `text`, each ending with a newline.
fn head(text: &str, count: usize) -> String {
    text.lines()
        .take(count)
        .map(|line| format!("{line}\n"))
        .collect()
}

/// A function declaration of exactly `lines` lines.
fn function_of(lines: usize) -> String {
    let mut source = String::from("export function f() {\n");
    for index in 0..lines - 2 {
        source.push_str(&format!("  {index};\n"));
    }
    source.push_str("}\n");
    source
}

/// Show's lines, printed from the declaration's first line; an excerpt
/// that reaches `-o` without `--limit` is complete.
#[test]
fn show_prints_its_default_number_of_lines_from_the_first() {
    let limit = default(&["--show"], "lines");
    let args = ["--show", "--name", "f", "app.ts"];
    for count in [limit - 1, limit, limit + 1] {
        let service = Service::new(&[("app.ts", &function_of(count))]);
        let root = service.fixture.root.clone();
        let complete = stdout(
            &run(&service, &[&args[..], &["--limit", "all"]].concat()),
            0,
        );
        assert_eq!(complete.lines().count(), 1 + count);
        let shown = stdout(&run(&service, &args), 0);
        let all = document(
            &run(
                &service,
                &[&args[..], &["--json", "--limit", "all"]].concat(),
            ),
            0,
        );
        let documented = document(&run(&service, &[&args[..], &["--json"]].concat()), 0);
        if count <= limit {
            assert_eq!(shown, complete, "{count}");
            assert_eq!(documented, all, "{count}");
            continue;
        }
        let (lines, cut) = last_line(&shown);
        assert_eq!(format!("{lines}\n"), head(&complete, 1 + limit));
        assert_eq!(
            cut,
            "… 1 more line; see all with: tsparse --show --name f app.ts --limit all, or write them to a file with -o FILE"
        );
        assert_eq!(
            stdout(&follow(&service, printed_command(cut), "", None), 0),
            complete
        );
        // The document's excerpt is the first lines of the source, and
        // everything else is the declaration's.
        let source = all["source"].as_str().unwrap();
        let first: Vec<&str> = source.split('\n').take(limit).collect();
        assert_eq!(documented["source"], first.join("\n"));
        assert_eq!(documented["declaration"], all["declaration"]);
        assert_eq!(
            documented["display"],
            json!({
                "lines": {"total": count, "returned": limit},
                "truncated": true,
                "next": {"argv": ["tsparse", "--show", "--name", "f", "app.ts", "--json", "--limit", "all"]},
            })
        );
        let followed = follow_argv(&service, &documented["display"]["next"]["argv"], "", None);
        assert_eq!(document(&followed, 0), all);
        // -o is the spillover channel; an explicit --limit cuts there too.
        assert!(stdout(&run(&service, &[&args[..], &["-o", "out.txt"]].concat()), 0).is_empty());
        assert_eq!(
            std::fs::read_to_string(root.join("out.txt")).unwrap(),
            complete
        );
        run(
            &service,
            &[&args[..], &["--json", "-o", "out.json"]].concat(),
        );
        let spilled: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("out.json")).unwrap()).unwrap();
        assert_eq!(spilled, all);
        run(
            &service,
            &[&args[..], &["--json", "-o", "out.json", "--limit", "3"]].concat(),
        );
        let spilled: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("out.json")).unwrap()).unwrap();
        assert_eq!(
            spilled["display"]["lines"],
            json!({"total": count, "returned": 3})
        );
        assert_eq!(
            spilled["display"]["next"]["argv"],
            json!([
                "tsparse", "--show", "--name", "f", "app.ts", "--json", "-o", "out.json",
                "--limit", "all"
            ])
        );
        let followed = follow_argv(
            &service,
            &spilled["display"]["next"]["argv"],
            "",
            Some("out.json"),
        );
        assert!(stdout(&followed, 0).is_empty());
        let spilled: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("out.json")).unwrap()).unwrap();
        assert_eq!(spilled, all);
        // A project's declaration, through the service and directly.
        let project = stdout(&both(&service, &["--show", "--name", "f"]), 0);
        let (lines, cut) = last_line(&project);
        assert_eq!(lines.lines().count(), 1 + limit);
        assert_eq!(
            cut,
            "… 1 more line; see all with: tsparse --show --name f --project tsconfig.json --limit all, or write them to a file with -o FILE"
        );
        let followed = stdout(&follow(&service, printed_command(cut), "", None), 0);
        assert_eq!(followed.lines().count(), 1 + count);
    }
}

/// The excerpt ends at the line break the library splits each kind of
/// line at, so a cut document's `source` is a prefix of the declaration's.
#[test]
fn a_cut_excerpt_ends_before_a_line_break_of_every_kind() {
    for line_break in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let source = ["export function f() {", "  1;", "  2;", "}"].join(line_break) + "\n";
        let service = Service::new(&[("app.ts", &source)]);
        let file = service.fixture.path("app.ts");
        let args = ["--show", "--name", "f", "app.ts", "--limit", "2"];
        let documented = document(&run(&service, &[&args[..], &["--json"]].concat()), 0);
        assert_eq!(
            documented["source"],
            format!("export function f() {{{line_break}  1;"),
            "{line_break:?}"
        );
        assert_eq!(
            documented["display"]["lines"],
            json!({"total": 4, "returned": 2})
        );
        assert_eq!(
            stdout(&run(&service, &args), 0),
            format!(
                "{}:1:17 function f\n1 | export function f() {{\n2 |   1;\n… 2 more lines; see all with: tsparse --show --name f app.ts --limit all, or write them to a file with -o FILE\n",
                file.display()
            ),
            "{line_break:?}"
        );
    }
}

/// `count` declarations named `f`, each in its own namespace, so `--name f`
/// is ambiguous and `--name nN.f` selects one.
fn namesakes(count: usize) -> String {
    (0..count)
        .map(|index| format!("namespace n{index} {{ export function f() {{}} }}\n"))
        .collect()
}

/// An ambiguous selection's candidates, in show, type, and edits, whose
/// text is a diagnostic: it never reaches `-o`, and its cut names how to
/// narrow the selection besides the command that lists every candidate.
#[test]
fn ambiguous_selections_list_their_default_number_of_candidates() {
    let narrow = "narrow with --name a.b or --at FILE:LINE:COLUMN, or see all with: ";
    let show = default(&["--show"], "candidates");
    let types = default(&["--type"], "candidates");
    let edits = default(&["--replace-body"], "candidates");
    assert_eq!(default(&["--insert-after"], "candidates"), edits);
    assert_eq!(default(&["--edit-batch", "EDITS"], "candidates"), edits);
    for (limit, selector) in [
        (show, &["--show", "--name", "f", "app.ts"][..]),
        (
            edits,
            &[
                "--replace-body",
                "--name",
                "f",
                "app.ts",
                "--from",
                "body.ts",
                "--plan",
                "p.json",
            ],
        ),
    ] {
        for count in [limit - 1, limit, limit + 1] {
            let service =
                Service::new(&[("app.ts", &namesakes(count)), ("body.ts", "{ return; }")]);
            let root = service.fixture.root.clone();
            let complete = stderr(&run(&service, &[selector, &["--limit", "all"]].concat()), 1);
            assert_eq!(complete.lines().count(), 1 + count);
            let shown = stderr(&run(&service, selector), 1);
            let all = document(
                &run(
                    &service,
                    &[selector, &["--json", "--limit", "all"]].concat(),
                ),
                1,
            );
            let documented = document(&run(&service, &[selector, &["--json"]].concat()), 1);
            assert!(!root.join("p.json").exists());
            if count <= limit {
                assert_eq!(shown, complete, "{count}");
                assert_eq!(documented, all, "{count}");
                continue;
            }
            // An edit's command leaves out --plan, which a preview saves.
            let command = match selector[0] {
                "--show" => "tsparse --show --name f app.ts --limit all",
                _ => "tsparse --replace-body --name f app.ts --from body.ts --limit all",
            };
            let (lines, cut) = last_line(&shown);
            assert_eq!(format!("{lines}\n"), head(&complete, 1 + limit));
            assert_eq!(cut, format!("… 1 more candidate; {narrow}{command}"));
            assert_eq!(
                stderr(&follow(&service, printed_command(cut), "", None), 1),
                complete
            );
            let candidates = documented["candidates"].as_array().unwrap();
            assert_eq!(
                candidates[..],
                all["candidates"].as_array().unwrap()[..limit]
            );
            // The request as given, an edit's without its --plan, then
            // --json where the request gave it, and --limit all.
            let next: Vec<String> = {
                let mut argv: Vec<String> = selector.iter().map(|arg| arg.to_string()).collect();
                if selector[0] != "--show" {
                    argv.truncate(argv.len() - 2);
                }
                argv.insert(0, "tsparse".into());
                argv.extend(["--json".into(), "--limit".into(), "all".into()]);
                argv
            };
            assert_eq!(
                documented["display"],
                json!({
                    "candidates": {"total": count, "returned": limit},
                    "truncated": true,
                    "next": {"argv": next},
                })
            );
            let mut without = documented.clone();
            without.as_object_mut().unwrap().remove("display");
            without["candidates"] = all["candidates"].clone();
            assert_eq!(without, all);
            let followed = follow_argv(&service, &documented["display"]["next"]["argv"], "", None);
            assert_eq!(document(&followed, 1), all);
            // -o without --limit cuts nothing, and the text, a diagnostic,
            // never creates it; a document reaches it.
            let output = run(&service, &[selector, &["-o", "out.txt"]].concat());
            assert_eq!(stderr(&output, 1), complete);
            assert!(!root.join("out.txt").exists());
            let output = run(
                &service,
                &[selector, &["--json", "-o", "out.json"]].concat(),
            );
            assert!(stdout(&output, 1).is_empty());
            let spilled: Value =
                serde_json::from_str(&std::fs::read_to_string(root.join("out.json")).unwrap())
                    .unwrap();
            assert_eq!(spilled, all);
            std::fs::remove_file(root.join("out.json")).unwrap();
            // With an explicit --limit, a query's command keeps its -o and
            // an edit's leaves it out, as that file now exists.
            let output = run(
                &service,
                &[selector, &["--json", "-o", "out.json", "--limit", "3"]].concat(),
            );
            assert!(stdout(&output, 1).is_empty());
            let spilled: Value =
                serde_json::from_str(&std::fs::read_to_string(root.join("out.json")).unwrap())
                    .unwrap();
            assert_eq!(spilled["candidates"].as_array().unwrap().len(), 3);
            let argv = &spilled["display"]["next"]["argv"];
            let editing = selector[0] != "--show";
            assert_eq!(
                argv.as_array().unwrap().contains(&json!("-o")),
                !editing,
                "{argv}"
            );
            let followed = follow_argv(&service, argv, "", (!editing).then_some("out.json"));
            if editing {
                assert_eq!(document(&followed, 1), all);
            } else {
                assert!(stdout(&followed, 1).is_empty());
                let spilled: Value =
                    serde_json::from_str(&std::fs::read_to_string(root.join("out.json")).unwrap())
                        .unwrap();
                assert_eq!(spilled, all);
            }
            assert!(!root.join("p.json").exists());
        }
    }
    // A project's selections, through the service and directly: show,
    // type, and an edit, under, at, and over the default.
    for (limit, selector, command) in [
        (
            show,
            &["--show", "--name", "f"][..],
            "tsparse --show --name f --project tsconfig.json --limit all",
        ),
        (
            types,
            &["--type", "--name", "f"],
            "tsparse --type --name f --project tsconfig.json --limit all",
        ),
        (
            edits,
            &["--insert-after", "--name", "f", "--from", "body.ts"],
            "tsparse --insert-after --name f --from body.ts --project tsconfig.json --limit all",
        ),
    ] {
        for count in [limit - 1, limit, limit + 1] {
            let service =
                Service::new(&[("app.ts", &namesakes(count)), ("body.ts", "export {};\n")]);
            let complete = stderr(
                &both(&service, &[selector, &["--limit", "all"]].concat()),
                1,
            );
            assert_eq!(complete.lines().count(), 1 + count, "{selector:?}");
            for index in 0..count {
                assert!(complete.contains(&format!("n{index}.f")), "{complete}");
            }
            let shown = stderr(&both(&service, selector), 1);
            let all = document(
                &both(
                    &service,
                    &[selector, &["--json", "--limit", "all"]].concat(),
                ),
                1,
            );
            let documented = document(&both(&service, &[selector, &["--json"]].concat()), 1);
            if count <= limit {
                assert_eq!(shown, complete, "{selector:?} {count}");
                assert_eq!(documented, all, "{selector:?} {count}");
                continue;
            }
            let (lines, cut) = last_line(&shown);
            assert_eq!(format!("{lines}\n"), head(&complete, 1 + limit));
            assert_eq!(cut, format!("… 1 more candidate; {narrow}{command}"));
            let followed = stderr(&follow(&service, printed_command(cut), "", None), 1);
            assert_eq!(followed, complete, "{selector:?}");
            assert_eq!(documented["candidates"].as_array().unwrap().len(), limit);
            assert_eq!(
                documented["display"]["candidates"],
                json!({"total": count, "returned": limit})
            );
            let followed = document(
                &follow_argv(&service, &documented["display"]["next"]["argv"], "", None),
                1,
            );
            assert_eq!(followed, all, "{selector:?}");
        }
    }
}

/// `x` declared once and used so that it has `count` occurrences, one per
/// line.
fn occurrences(count: usize) -> String {
    let mut source = String::from("export const x = 0;\n");
    for _ in 1..count {
        source.push_str("x;\n");
    }
    source
}

/// Reference occurrences, counted across groups in the result's order. The
/// header still counts every occurrence, and the document cut is the
/// result with each group's occurrences cut and `display` after its fields.
#[test]
fn references_show_their_default_number_of_occurrences() {
    let limit = default(&["--references"], "occurrences");
    let args = ["--references", "--name", "x"];
    for count in [limit - 1, limit, limit + 1] {
        let service = Service::new(&[("app.ts", &occurrences(count))]);
        let root = service.fixture.root.clone();
        let complete = stdout(
            &both(&service, &[&args[..], &["--limit", "all"]].concat()),
            0,
        );
        assert!(
            complete.contains(&format!("1 group, {count} occurrences in 1 file")),
            "{complete}"
        );
        assert_eq!(complete.lines().count(), 2 + count);
        let shown = stdout(&both(&service, &args), 0);
        let all = stdout(
            &both(
                &service,
                &[&args[..], &["--json", "--limit", "all"]].concat(),
            ),
            0,
        );
        let documented = stdout(&both(&service, &[&args[..], &["--json"]].concat()), 0);
        if count <= limit {
            assert_eq!(shown, complete, "{count}");
            assert_eq!(documented, all, "{count}");
            continue;
        }
        let (lines, cut) = last_line(&shown);
        assert_eq!(format!("{lines}\n"), head(&complete, 2 + limit));
        assert_eq!(
            cut,
            "… 1 more occurrence; see all with: tsparse --references --name x --project tsconfig.json --limit all, or write them to a file with -o FILE"
        );
        assert_eq!(
            stdout(&follow(&service, printed_command(cut), "", None), 0),
            complete
        );
        // The result's fields in their order, then `display`.
        let all: Value = serde_json::from_str(&all).unwrap();
        assert!(
            documented.find(r#""unsearchedMembers""#) < documented.find(r#""display""#),
            "{documented}"
        );
        let documented: Value = serde_json::from_str(&documented).unwrap();
        let occurrences = documented["groups"][0]["occurrences"].as_array().unwrap();
        assert_eq!(
            occurrences[..],
            all["groups"][0]["occurrences"].as_array().unwrap()[..limit]
        );
        assert_eq!(
            documented["display"],
            json!({
                "occurrences": {"total": count, "returned": limit},
                "truncated": true,
                "next": {"argv": [
                    "tsparse", "--references", "--name", "x", "--json",
                    "--project", "tsconfig.json", "--limit", "all",
                ]},
            })
        );
        let mut without = documented.clone();
        without.as_object_mut().unwrap().remove("display");
        without["groups"] = all["groups"].clone();
        assert_eq!(without, all);
        let followed = follow_argv(&service, &documented["display"]["next"]["argv"], "", None);
        assert_eq!(document(&followed, 0), all);
        // --pretty alone writes the indented document, cut alike.
        let pretty = stdout(&both(&service, &[&args[..], &["--pretty"]].concat()), 0);
        assert!(pretty.lines().count() > 1);
        let pretty: Value = serde_json::from_str(&pretty).unwrap();
        assert_eq!(
            pretty["display"]["occurrences"],
            documented["display"]["occurrences"]
        );
        // -o is the spillover channel; an explicit --limit cuts there too,
        // and its command rewrites the file whole.
        run(
            &service,
            &[&args[..], &["--project", "tsconfig.json", "-o", "out.txt"]].concat(),
        );
        let served = stdout(
            &run(
                &service,
                &[&args[..], &["--project", "tsconfig.json", "--limit", "all"]].concat(),
            ),
            0,
        );
        assert_eq!(
            std::fs::read_to_string(root.join("out.txt")).unwrap(),
            served
        );
        let output = run(
            &service,
            &[
                &args[..],
                &[
                    "--project",
                    "tsconfig.json",
                    "-o",
                    "out.txt",
                    "--limit",
                    "5",
                ],
            ]
            .concat(),
        );
        assert!(stdout(&output, 0).is_empty());
        let written = std::fs::read_to_string(root.join("out.txt")).unwrap();
        let (lines, cut) = last_line(&written);
        assert_eq!(lines.lines().count(), 2 + 5);
        assert_eq!(
            cut,
            format!(
                "… {} more occurrences; see all with: tsparse --references --name x --project tsconfig.json -o out.txt --limit all",
                count - 5
            )
        );
        follow(&service, printed_command(cut), "", Some("out.txt"));
        assert_eq!(
            std::fs::read_to_string(root.join("out.txt"))
                .unwrap()
                .lines()
                .count(),
            2 + count
        );
    }
}

/// Many groups: the cap counts occurrences across them, and every group
/// and its declarations are shown, also those whose occurrences it cuts.
#[test]
fn the_occurrence_cap_counts_across_groups_and_keeps_every_group() {
    let limit = default(&["--references"], "occurrences");
    // `limit` groups of two occurrences each: a parameter and its use.
    let source: String = (0..limit)
        .map(|index| format!("export function g{index}(x: number) {{ return x; }}\n"))
        .collect();
    let service = Service::new(&[("app.ts", &source)]);
    let shown = stdout(&both(&service, &["--references", "--name", "x"]), 0);
    let lines: Vec<&str> = shown.lines().collect();
    assert!(
        lines[0].ends_with(&format!(
            "{limit} groups, {} occurrences in 1 file; coverage lexical, complete",
            2 * limit
        )),
        "{}",
        lines[0]
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("group "))
            .count(),
        limit
    );
    assert_eq!(
        lines.iter().filter(|line| line.starts_with("  ")).count(),
        limit
    );
    let (_, cut) = last_line(&shown);
    assert_eq!(
        cut,
        format!(
            "… {limit} more occurrences; see all with: tsparse --references --name x --project tsconfig.json --limit all, or write them to a file with -o FILE"
        )
    );
    let followed = stdout(&follow(&service, printed_command(cut), "", None), 0);
    assert_eq!(
        followed
            .lines()
            .filter(|line| line.starts_with("  "))
            .count(),
        2 * limit
    );
    let documented = document(
        &both(&service, &["--references", "--name", "x", "--json"]),
        0,
    );
    let groups = documented["groups"].as_array().unwrap();
    assert_eq!(groups.len(), limit);
    let kept: Vec<usize> = groups
        .iter()
        .map(|group| group["occurrences"].as_array().unwrap().len())
        .collect();
    assert_eq!(kept[..limit / 2], vec![2; limit / 2][..]);
    assert_eq!(kept[limit / 2..], vec![0; limit - limit / 2][..]);
    assert!(
        groups
            .iter()
            .all(|group| group["declarations"].as_array().unwrap().len() == 1)
    );
    // Its command, followed as given, shows every group's occurrences.
    let all = document(
        &both(
            &service,
            &["--references", "--name", "x", "--json", "--limit", "all"],
        ),
        0,
    );
    let followed = follow_argv(&service, &documented["display"]["next"]["argv"], "", None);
    assert_eq!(document(&followed, 0), all);
    assert_eq!(
        all["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| group["occurrences"].as_array().unwrap().len())
            .sum::<usize>(),
        2 * limit
    );
}

/// A cut leaves a partial result partial: its outcome, coverage, gaps,
/// and exit code are the complete result's, and the gaps precede the cut.
#[test]
fn a_cut_partial_result_keeps_its_outcome_coverage_gaps_and_exit_code() {
    let service = Service::of(support::Fixture::copy_from_repository(
        "fixtures/types/member-search-core",
    ));
    let r = service.fixture.root.display().to_string();
    let args = ["--references", "--at", "cases.ts:2:36", "--limit", "2"];
    let shown = stdout(&both(&service, &args), 1);
    assert_eq!(
        shown,
        format!(
            "\
{r}/tsconfig.json: at {r}/cases.ts:2:36: 1 group, 3 occurrences in 1 file; coverage member, partial
group 1: value, declared at {r}/cases.ts:2:36
  {r}/cases.ts:2:36 declaration
  {r}/cases.ts:5:17 use
gap {r}/cases.ts:28:32: error_type: native non-null checking consumes unknown-type error recovery
  cause {r}/cases.ts:28:1: error_type: native non-null checking consumes unknown-type error recovery
… 1 more occurrence; see all with: tsparse --references --at cases.ts:2:36 --limit all --project tsconfig.json, or write them to a file with -o FILE
"
        )
    );
    let (_, cut) = last_line(&shown);
    let followed = stdout(&follow(&service, printed_command(cut), "", None), 1);
    assert!(
        followed.contains(&format!("\n  {r}/cases.ts:6:19 use\ngap ")),
        "{followed}"
    );
    let cut = document(&both(&service, &[&args[..], &["--json"]].concat()), 1);
    let all = document(
        &both(
            &service,
            &[
                "--references",
                "--at",
                "cases.ts:2:36",
                "--json",
                "--limit",
                "all",
            ],
        ),
        1,
    );
    assert_eq!(cut["outcome"], "partial");
    for field in [
        "outcome",
        "coverage",
        "selection",
        "filters",
        "unsearchedMembers",
    ] {
        assert_eq!(cut[field], all[field], "{field}");
    }
    assert_eq!(
        cut["display"]["occurrences"],
        json!({"total": 3, "returned": 2})
    );
    // Its command, followed as given, gives the whole partial result.
    let followed = follow_argv(&service, &cut["display"]["next"]["argv"], "", None);
    assert_eq!(document(&followed, 1), all);
}

/// A partial reference result reaches `-o` as it reaches stdout, whole or
/// cut, in text and JSON: exit 1, nothing on stdout or stderr, its partial
/// outcome, coverage, and gap kept, and the same bytes through the service
/// and loaded directly. Without `--limit` the file is whole; with
/// `--limit 2` it is cut, and the command it prints, followed as printed,
/// rewrites it whole (design, "Preserved behavior": partial results reach
/// `-o`; Verification plan, item 4).
#[test]
fn a_partial_reference_result_reaches_the_output_file_whole_or_cut() {
    let service = Service::of(support::Fixture::copy_from_repository(
        "fixtures/types/member-search-core",
    ));
    let at = ["--references", "--at", "cases.ts:2:36"];
    for json in [true, false] {
        let (format, file): (&[&str], &str) = if json {
            (&["--json"], "partial.json")
        } else {
            (&[], "partial.txt")
        };
        let path = service.fixture.path(file);
        let whole = stdout(
            &both(&service, &[&at[..], format, &["--limit", "all"]].concat()),
            1,
        );
        // What `-o` receives through each entry, which must agree.
        let written = |bound: &[&str]| -> String {
            let mut written = Vec::new();
            for entry in PROJECT {
                let _ = std::fs::remove_file(&path);
                let output = run(
                    &service,
                    &[&at[..], format, bound, entry, &["-o", file]].concat(),
                );
                assert_eq!(output.status.code(), Some(1), "{output:?}");
                assert!(
                    output.stdout.is_empty() && output.stderr.is_empty(),
                    "{output:?}"
                );
                written.push(std::fs::read_to_string(&path).unwrap());
            }
            assert_eq!(written[0], written[1], "{format:?} {bound:?}");
            written.pop().unwrap()
        };
        assert_eq!(written(&[]), whole, "{format:?}");
        let cut = written(&["--limit", "2"]);
        let followed = if json {
            let document: Value = serde_json::from_str(&cut).unwrap();
            let all: Value = serde_json::from_str(&whole).unwrap();
            assert_eq!(document["outcome"], "partial");
            assert_eq!(document["coverage"], all["coverage"]);
            assert_eq!(document["coverage"]["status"], "partial");
            assert_eq!(document["coverage"]["gaps"].as_array().unwrap().len(), 1);
            assert_eq!(
                document["display"]["occurrences"],
                json!({"total": 3, "returned": 2})
            );
            follow_argv(
                &service,
                &document["display"]["next"]["argv"],
                "",
                Some(file),
            )
        } else {
            let (lines, cut) = last_line(&cut);
            assert!(lines.contains("; coverage member, partial\n"), "{lines}");
            assert!(lines.contains("\ngap "), "{lines}");
            assert_eq!(
                cut,
                "… 1 more occurrence; see all with: tsparse --references --at cases.ts:2:36 --limit all --project tsconfig.json -o partial.txt"
            );
            follow(&service, printed_command(cut), "", Some(file))
        };
        assert_eq!(followed.status.code(), Some(1), "{followed:?}");
        assert!(
            followed.stdout.is_empty() && followed.stderr.is_empty(),
            "{followed:?}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), whole, "{format:?}");
    }
}

/// A complete body of `statements` distinct statements.
fn body(statements: usize) -> String {
    let mut body = String::from("{\n");
    for index in 0..statements {
        body.push_str(&format!("  {index};\n"));
    }
    body.push('}');
    body
}

const EDITED: &str = "export function f() {\n  return 0;\n}\n";

/// The number of lines in the first file's diff of a preview document.
fn diff_lines(document: &Value, file: usize) -> usize {
    document["files"][file]["diff"]
        .as_str()
        .unwrap()
        .split_inclusive('\n')
        .count()
}

/// A body whose replacement of `f` in `EDITED` gives a diff of exactly
/// `lines` lines: each statement adds one.
fn body_of_diff(lines: usize) -> String {
    let service = Service::new(&[("app.ts", EDITED), ("body.ts", &body(1))]);
    let preview = document(
        &run(
            &service,
            &[
                "--replace-body",
                "--name",
                "f",
                "app.ts",
                "--from",
                "body.ts",
                "--json",
            ],
        ),
        0,
    );
    let base = diff_lines(&preview, 0) - 1;
    body(lines - base)
}

/// Preview diffs, cut per file: every changed file is listed, `effect`
/// stays `none`, and the cut line names the files it cut, the plan that
/// holds the complete proposed text, and a command without `--plan`.
#[test]
fn a_preview_shows_its_default_number_of_diff_lines_per_file() {
    let limit = default(&["--replace-body"], "diff_lines");
    let args = [
        "--replace-body",
        "--name",
        "f",
        "app.ts",
        "--from",
        "body.ts",
    ];
    for count in [limit - 1, limit, limit + 1] {
        let fragment = body_of_diff(count);
        let service = Service::new(&[("app.ts", EDITED), ("body.ts", &fragment)]);
        let root = service.fixture.root.clone();
        let file = service.fixture.path("app.ts").display().to_string();
        let all = document(
            &run(
                &service,
                &[&args[..], &["--json", "--limit", "all"]].concat(),
            ),
            0,
        );
        assert_eq!(diff_lines(&all, 0), count);
        let complete = stdout(
            &run(&service, &[&args[..], &["--limit", "all"]].concat()),
            0,
        );
        let shown = stdout(&run(&service, &args), 0);
        let documented = document(&run(&service, &[&args[..], &["--json"]].concat()), 0);
        if count <= limit {
            assert_eq!(shown, complete, "{count}");
            assert_eq!(documented, all, "{count}");
            continue;
        }
        // The site, the verdict, the cut diff, the plan line, and the cut.
        let complete_lines: Vec<&str> = complete.lines().collect();
        let mut expected: Vec<&str> = complete_lines[..2 + limit].to_vec();
        expected.push(complete_lines[complete_lines.len() - 1]);
        let (lines, cut) = last_line(&shown);
        assert_eq!(lines, expected.join("\n"));
        assert_eq!(
            cut,
            format!(
                "… 1 more diff line in {file}; see all with: tsparse --replace-body --name f app.ts --from body.ts --limit all, or write them to a file with -o FILE"
            )
        );
        assert_eq!(
            stdout(&follow(&service, printed_command(cut), "", None), 0),
            complete
        );
        // Every file is listed with its outcome and effect; only the diff
        // is cut.
        assert_eq!(documented["outcome"], "preview");
        assert_eq!(documented["effect"], "none");
        assert_eq!(diff_lines(&documented, 0), limit);
        assert!(
            all["files"][0]["diff"]
                .as_str()
                .unwrap()
                .starts_with(documented["files"][0]["diff"].as_str().unwrap())
        );
        assert_eq!(
            documented["display"],
            json!({
                "diff_lines": {"total": count, "returned": limit},
                "truncated": true,
                "next": {"argv": [
                    "tsparse", "--replace-body", "--name", "f", "app.ts",
                    "--from", "body.ts", "--json", "--limit", "all",
                ]},
            })
        );
        let mut without = documented.clone();
        without.as_object_mut().unwrap().remove("display");
        without["files"] = all["files"].clone();
        assert_eq!(without, all);
        assert_eq!(
            document(
                &follow_argv(&service, &documented["display"]["next"]["argv"], "", None),
                0
            ),
            all
        );
        // A saved plan holds the complete proposed text; the command shows
        // the diff without saving another.
        let planned = stdout(
            &run(&service, &[&args[..], &["--plan", "p.json"]].concat()),
            0,
        );
        assert!(root.join("p.json").exists());
        let (lines, cut) = last_line(&planned);
        assert!(
            lines.ends_with("\nplan: p.json; apply it with: tsparse --apply p.json"),
            "{lines}"
        );
        assert_eq!(
            cut,
            format!(
                "… 1 more diff line in {file}; the plan p.json holds the complete proposed text; see all with: tsparse --replace-body --name f app.ts --from body.ts --limit all, or write them to a file with -o FILE"
            )
        );
        assert_eq!(
            stdout(&follow(&service, printed_command(cut), "", None), 0),
            complete
        );
        // -o without --limit is complete; with --limit it is cut, and the
        // command, which cannot write that file again, prints the preview.
        assert!(stdout(&run(&service, &[&args[..], &["-o", "out.txt"]].concat()), 0).is_empty());
        assert_eq!(
            std::fs::read_to_string(root.join("out.txt")).unwrap(),
            complete
        );
        let output = run(
            &service,
            &[&args[..], &["-o", "cut.txt", "--limit", "3"]].concat(),
        );
        assert!(stdout(&output, 0).is_empty());
        let written = std::fs::read_to_string(root.join("cut.txt")).unwrap();
        let (_, cut) = last_line(&written);
        assert_eq!(
            cut,
            format!(
                "… {} more diff lines in {file}; see all with: tsparse --replace-body --name f app.ts --from body.ts --limit all",
                count - 3
            )
        );
        assert_eq!(
            stdout(&follow(&service, printed_command(cut), "", None), 0),
            complete
        );
        // A command repeating an edit cannot read its stdin again, so a
        // preview of a fragment read from stdin is never cut, and a --limit
        // that would cut it is refused before anything is read.
        let piped = ["--replace-body", "--name", "f", "app.ts", "--from", "-"];
        assert_eq!(stdout(&run_with(&service, &piped, &fragment), 0), complete);
        assert_eq!(
            stdout(
                &run_with(
                    &service,
                    &[&piped[..], &["--limit", "all"]].concat(),
                    &fragment
                ),
                0
            ),
            complete
        );
        let refused = "--limit 3 needs every fragment in a file: a command repeating this edit to show what the limit withholds cannot read stdin again";
        let output = run_with(
            &service,
            &[&piped[..], &["--limit", "3"]].concat(),
            &fragment,
        );
        assert_eq!(stderr(&output, 2), format!("usage.flag_scope: {refused}\n"));
        let output = run_with(
            &service,
            &[
                &piped[..],
                &["--limit", "3", "--json", "-o", "refused.json"],
            ]
            .concat(),
            &fragment,
        );
        let refusal = document(&output, 2);
        assert_eq!(refusal["error"]["code"], "usage.flag_scope");
        assert_eq!(refusal["error"]["message"], refused);
        assert_eq!(refusal["effect"], "none");
        assert!(!root.join("refused.json").exists());
        // A project's edit, through the service and directly.
        let project = stdout(&both(&service, &args), 0);
        let (_, cut) = last_line(&project);
        assert_eq!(
            cut,
            format!(
                "… 1 more diff line in {file}; see all with: tsparse --replace-body --name f app.ts --from body.ts --project tsconfig.json --limit all, or write them to a file with -o FILE"
            )
        );
        let followed = stdout(&follow(&service, printed_command(cut), "", None), 0);
        assert!(!followed.contains("more diff line"), "{followed}");
    }
}

/// A batch's cap applies to each file's diff: the cut names each file it
/// cut, and every changed file stays listed.
#[test]
fn a_batch_preview_cuts_each_file_and_lists_every_one() {
    let service = Service::new(&[
        ("a.ts", EDITED),
        ("b.ts", EDITED),
        ("short.ts", &body(1)),
        ("long.ts", &body(20)),
        (
            "edits.json",
            r#"{"edits": [
                {"operation": "replace_body", "name": "f", "file": "a.ts", "from": "short.ts"},
                {"operation": "replace_body", "name": "f", "file": "b.ts", "from": "long.ts"}
            ]}"#,
        ),
    ]);
    let a = service.fixture.path("a.ts").display().to_string();
    let b = service.fixture.path("b.ts").display().to_string();
    let all = document(
        &run(
            &service,
            &["--edit-batch", "edits.json", "--json", "--limit", "all"],
        ),
        0,
    );
    let (short, long) = (diff_lines(&all, 0), diff_lines(&all, 1));
    assert!(short < long);
    for (limit, cut_files) in [
        (short, vec![b.as_str()]),
        (short - 1, vec![a.as_str(), b.as_str()]),
    ] {
        let bound = limit.to_string();
        let args = ["--edit-batch", "edits.json", "--limit", &bound];
        let shown = stdout(&run(&service, &args), 0);
        let (_, cut) = last_line(&shown);
        let withheld = short + long - limit.min(short) - limit;
        assert_eq!(
            cut,
            format!(
                "… {withheld} more diff lines in {}; see all with: tsparse --edit-batch edits.json --limit all, or write them to a file with -o FILE",
                cut_files.join(", ")
            )
        );
        let complete = stdout(&follow(&service, printed_command(cut), "", None), 0);
        assert_eq!(
            complete,
            stdout(
                &run(&service, &["--edit-batch", "edits.json", "--limit", "all"]),
                0
            )
        );
        let documented = document(&run(&service, &[&args[..], &["--json"]].concat()), 0);
        let files = documented["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(diff_lines(&documented, 0), limit.min(short));
        assert_eq!(diff_lines(&documented, 1), limit);
        assert_eq!(
            documented["display"]["diff_lines"],
            json!({"total": short + long, "returned": limit.min(short) + limit})
        );
        assert_eq!(documented["effect"], "none");
        // Its command, followed as given, shows every file's whole diff.
        let followed = follow_argv(&service, &documented["display"]["next"]["argv"], "", None);
        assert_eq!(document(&followed, 0), all);
    }
    // A project's batch preview, whole or cut, is the same bytes through its
    // service and loaded directly, and each printed command recovers every
    // diff line.
    let whole = both(&service, &["--edit-batch", "edits.json", "--limit", "all"]);
    let whole_json = both(
        &service,
        &["--edit-batch", "edits.json", "--json", "--limit", "all"],
    );
    for limit in [short, short - 1] {
        let bound = limit.to_string();
        let args = ["--edit-batch", "edits.json", "--limit", &bound];
        let shown = stdout(&both(&service, &args), 0);
        let (_, cut) = last_line(&shown);
        let withheld = short + long - limit.min(short) - limit;
        assert!(
            cut.starts_with(&format!("… {withheld} more diff lines in ")),
            "{cut}"
        );
        let followed = follow(&service, printed_command(cut), "", None);
        assert_eq!(stdout(&followed, 0), stdout(&whole, 0));
        let documented = document(&both(&service, &[&args[..], &["--json"]].concat()), 0);
        assert_eq!(
            documented["display"]["diff_lines"],
            json!({"total": short + long, "returned": limit.min(short) + limit})
        );
        let followed = follow_argv(&service, &documented["display"]["next"]["argv"], "", None);
        assert_eq!(document(&followed, 0), document(&whole_json, 0));
    }
    // A batch edit whose fragment is read from stdin is never cut either,
    // and refuses a --limit that would cut it.
    std::fs::write(
        service.fixture.path("piped.json"),
        r#"{"edits": [
            {"operation": "replace_body", "name": "f", "file": "a.ts", "from": "short.ts"},
            {"operation": "replace_body", "name": "f", "file": "b.ts", "from": "-"}
        ]}"#,
    )
    .unwrap();
    let fragment = body(20);
    let piped = stdout(
        &run_with(&service, &["--edit-batch", "piped.json"], &fragment),
        0,
    );
    assert_eq!(
        piped,
        stdout(
            &run(&service, &["--edit-batch", "edits.json", "--limit", "all"]),
            0
        )
    );
    let output = run_with(
        &service,
        &["--edit-batch", "piped.json", "--limit", "2"],
        &fragment,
    );
    assert_eq!(
        stderr(&output, 2),
        "usage.flag_scope: --limit 2 needs every fragment in a file: a command repeating this edit to show what the limit withholds cannot read stdin again\n"
    );
}

/// A batch names its selections in its file, so a cut ambiguity of one of
/// its edits says how to narrow the edit's `name` or `at`, which the batch
/// takes, rather than flags `--edit-batch` refuses.
#[test]
fn a_batch_edit_s_cut_ambiguity_narrows_the_edit_in_its_file() {
    let limit = default(&["--edit-batch", "EDITS"], "candidates");
    let service = Service::new(&[
        ("app.ts", &namesakes(limit + 1)),
        ("body.ts", "{ return; }"),
        (
            "edits.json",
            r#"{"edits": [{"operation": "replace_body", "name": "f", "file": "app.ts", "from": "body.ts"}]}"#,
        ),
    ]);
    let shown = stderr(&run(&service, &["--edit-batch", "edits.json"]), 1);
    let (lines, cut) = last_line(&shown);
    assert!(lines.starts_with("edit 1: "), "{lines}");
    assert_eq!(lines.lines().count(), 1 + limit);
    assert_eq!(
        cut,
        "… 1 more candidate; narrow the edit's name to a.b or give its at as FILE:LINE:COLUMN, or see all with: tsparse --edit-batch edits.json --limit all"
    );
    let followed = stderr(&follow(&service, printed_command(cut), "", None), 1);
    assert_eq!(followed.lines().count(), 1 + limit + 1);
}

/// The printed command repeats the request's arguments as given, a
/// repeated `--limit` each made `all`, an operand that needs quoting quoted,
/// and the filters kept, but names no transport: the service and a direct
/// load answer alike, so `--direct` is left out, and both transports print
/// the same bytes.
#[test]
fn the_printed_command_repeats_the_request_but_not_its_transport() {
    let service = Service::new(&[("my app.ts", &occurrences(5))]);
    let request = [
        "--name",
        "x",
        "my app.ts",
        "--no-tests",
        "--file",
        "my app.ts",
        "--limit",
        "1",
        "--limit",
        "2",
        "--project",
        "tsconfig.json",
    ];
    let mut printed = Vec::new();
    for transport in [&["--references"][..], &["--references", "--direct"]] {
        let output = run(&service, &[transport, &request[..]].concat());
        printed.push(stdout(&output, 0));
        let output = run(&service, &[transport, &request[..], &["--json"]].concat());
        printed.push(stdout(&output, 0));
    }
    for other in &printed[2..] {
        assert_eq!(&printed[other.starts_with('{') as usize], other);
    }
    let (lines, cut) = last_line(&printed[0]);
    assert_eq!(
        lines.lines().filter(|line| line.starts_with("  ")).count(),
        2
    );
    assert_eq!(
        cut,
        "… 3 more occurrences; see all with: tsparse --references --name x 'my app.ts' --no-tests --file 'my app.ts' --limit all --limit all --project tsconfig.json, or write them to a file with -o FILE"
    );
    let followed = stdout(&follow(&service, printed_command(cut), "", None), 0);
    assert_eq!(
        followed
            .lines()
            .filter(|line| line.starts_with("  "))
            .count(),
        5
    );
    let documented: Value = serde_json::from_str(&printed[1]).unwrap();
    assert_eq!(
        documented["display"]["next"]["argv"],
        json!([
            "tsparse",
            "--references",
            "--name",
            "x",
            "my app.ts",
            "--no-tests",
            "--file",
            "my app.ts",
            "--limit",
            "all",
            "--limit",
            "all",
            "--project",
            "tsconfig.json",
            "--json"
        ])
    );
    let followed = document(
        &follow_argv(&service, &documented["display"]["next"]["argv"], "", None),
        0,
    );
    assert_eq!(
        followed["groups"][0]["occurrences"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
}

/// How a list's text form is delivered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Form {
    /// A result, on stdout or `-o`.
    Result,
    /// A diagnostic, on stderr, which never creates or replaces `-o`.
    Diagnostic,
}

/// A request's entries, each the arguments that select it: a file's,
/// answered in this process, or a project's, through its service and loaded
/// directly.
type Entries = &'static [&'static [&'static str]];
const FILE: Entries = &[&[]];
const PROJECT: Entries = &[
    &["--project", "tsconfig.json"],
    &["--project", "tsconfig.json", "--direct"],
];

/// `args` through every entry of `entries`, which must agree byte for
/// byte, the command a cut prints included: the last entry's output.
fn each(service: &Service, entries: &[&[&str]], args: &[&str]) -> Output {
    let mut outputs: Vec<Output> = entries
        .iter()
        .map(|entry| run(service, &[args, entry].concat()))
        .collect();
    for output in &outputs[1..] {
        assert_eq!(output.status.code(), outputs[0].status.code(), "{args:?}");
        for (one, other) in [
            (&outputs[0].stdout, &output.stdout),
            (&outputs[0].stderr, &output.stderr),
        ] {
            assert!(
                one == other,
                "{args:?}: {} != {}",
                String::from_utf8_lossy(one),
                String::from_utf8_lossy(other)
            );
        }
    }
    outputs.pop().unwrap()
}

/// One bounded list of a request, `args`, holding `count` items against its
/// default `limit`, in every channel and through every entry, which give the
/// same bytes (Verification plan, item 4). Its text and its document
/// without `-o`: whole under and at the default, and over it cut, with a
/// printed command that, followed as printed, shows every item. With `-o`
/// and no `--limit`: whole, whatever the count. With `-o` and an explicit
/// `--limit 3`: cut there too, and its command shows every item, in the
/// query's own `-o`, or for an edit, whose `-o` is created only as a new
/// file and which its command leaves out, on stdout. A diagnostic never
/// creates `-o`, nor replaces a query's.
#[allow(clippy::too_many_arguments)]
fn every_channel(
    service: &Service,
    entries: &[&[&str]],
    args: &[&str],
    list: &str,
    code: i32,
    form: Form,
    editing: bool,
    limit: usize,
    count: usize,
) {
    let root = service.fixture.root.clone();
    let context = format!("{args:?} {entries:?} {count}");
    let text = |output: &Output| match form {
        Form::Result => stdout(output, code),
        Form::Diagnostic => stderr(output, code),
    };
    let with = |extra: &[&'static str]| -> Vec<&str> { [args, extra].concat() };
    let complete = text(&each(service, entries, &with(&["--limit", "all"])));
    let all = stdout(
        &each(service, entries, &with(&["--json", "--limit", "all"])),
        code,
    );
    let whole: Value = serde_json::from_str(&all).unwrap();
    assert_eq!(whole.get("display"), None, "{context}");
    let shown = text(&each(service, entries, args));
    let documented = stdout(&each(service, entries, &with(&["--json"])), code);
    if count <= limit {
        assert_eq!(shown, complete, "{context}");
        assert_eq!(documented, all, "{context}");
    } else {
        let (_, cut) = last_line(&shown);
        assert!(
            cut.starts_with(&format!("… {} more ", count - limit)),
            "{context}: {cut}"
        );
        let followed = follow(service, printed_command(cut), "", None);
        assert_eq!(text(&followed), complete, "{context}");
        let documented: Value = serde_json::from_str(&documented).unwrap();
        assert_eq!(documented["display"]["truncated"], true, "{context}");
        assert_eq!(
            documented["display"][list],
            json!({"total": count, "returned": limit}),
            "{context}"
        );
        let followed = follow_argv(service, &documented["display"]["next"]["argv"], "", None);
        assert_eq!(stdout(&followed, code), all, "{context}");
    }
    for json in [false, true] {
        let (format, file, expected): (&[&str], &str, &str) = if json {
            (&["--json"], "out.json", &all)
        } else {
            (&[], "out.txt", &complete)
        };
        let path = root.join(file);
        // Where a diagnostic is the outcome, `-o` is never written: a
        // query's existing file stays as it was, and an edit, which
        // refuses an existing one, creates none.
        let diagnostic = form == Form::Diagnostic && !json;
        let kept = (diagnostic && !editing).then_some("the caller's file\n");
        for bound in [&[][..], &["--limit", "3"]] {
            let mut written = Vec::new();
            for entry in entries {
                let _ = std::fs::remove_file(&path);
                if let Some(kept) = kept {
                    std::fs::write(&path, kept).unwrap();
                }
                let output = run(
                    service,
                    &[args, entry, format, &["-o", file], bound].concat(),
                );
                if diagnostic {
                    written.push(stderr(&output, code));
                    match kept {
                        Some(kept) => assert_eq!(std::fs::read_to_string(&path).unwrap(), kept),
                        None => assert!(!path.exists(), "{context}"),
                    }
                } else {
                    assert!(stdout(&output, code).is_empty(), "{context}");
                    written.push(std::fs::read_to_string(&path).unwrap());
                }
            }
            for other in &written[1..] {
                assert_eq!(&written[0], other, "{context} {bound:?}");
            }
            let produced = &written[0];
            if bound.is_empty() {
                assert_eq!(produced, expected, "{context} {format:?}");
                continue;
            }
            let command = if json {
                let cut: Value = serde_json::from_str(produced).unwrap();
                assert_eq!(
                    cut["display"][list],
                    json!({"total": count, "returned": 3}),
                    "{context}"
                );
                let argv = &cut["display"]["next"]["argv"];
                assert_eq!(
                    argv.as_array().unwrap().contains(&json!("-o")),
                    !editing,
                    "{argv}"
                );
                argv.as_array()
                    .unwrap()
                    .iter()
                    .map(|arg| format!("'{}'", arg.as_str().unwrap().replace('\'', r"'\''")))
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                let (_, cut) = last_line(produced);
                assert!(
                    cut.starts_with(&format!("… {} more ", count - 3)),
                    "{context}: {cut}"
                );
                printed_command(cut).to_string()
            };
            // A query's command names its -o, which receives every item; an
            // edit's, which cannot write that file again, prints them.
            let followed = follow(service, &command, "", (!editing).then_some(file));
            match (diagnostic, editing) {
                (true, _) => {
                    assert_eq!(stderr(&followed, code), complete, "{context}");
                    match kept {
                        Some(kept) => assert_eq!(std::fs::read_to_string(&path).unwrap(), kept),
                        None => assert!(!path.exists(), "{context}"),
                    }
                }
                (false, true) => assert_eq!(stdout(&followed, code), *expected, "{context}"),
                (false, false) => {
                    assert!(stdout(&followed, code).is_empty(), "{context}");
                    assert_eq!(
                        std::fs::read_to_string(&path).unwrap(),
                        expected,
                        "{context}"
                    );
                }
            }
        }
        let _ = std::fs::remove_file(&path);
    }
}

/// A name of several hundred characters, or `short`.
fn spelled(short: &str, long: bool) -> String {
    if long {
        format!("{short}{}", "Long".repeat(100))
    } else {
        short.to_string()
    }
}

/// `name` declared once and used so that it has `count` occurrences, one
/// per line.
fn occurrences_of(name: &str, count: usize) -> String {
    let mut source = format!("export const {name} = 0;\n");
    for _ in 1..count {
        source.push_str(&format!("{name};\n"));
    }
    source
}

/// A function declaration `f` of exactly `lines` lines, each of several
/// hundred characters when `long`.
fn function_with(lines: usize, long: bool) -> String {
    let mut source = String::from("export function f() {\n");
    for index in 0..lines - 2 {
        source.push_str(&format!("  \"{}\";\n", spelled(&index.to_string(), long)));
    }
    source.push_str("}\n");
    source
}

/// `count` declarations named `f`, each in its own namespace, whose name is
/// long when `long`.
fn namesakes_with(count: usize, long: bool) -> String {
    (0..count)
        .map(|index| {
            format!(
                "namespace {} {{ export function f() {{}} }}\n",
                spelled(&format!("n{index}"), long)
            )
        })
        .collect()
}

/// A complete body of `statements` distinct statements, each of several
/// hundred characters when `long`.
fn body_with(statements: usize, long: bool) -> String {
    let mut body = String::from("{\n");
    for index in 0..statements {
        body.push_str(&format!("  \"{}\";\n", spelled(&index.to_string(), long)));
    }
    body.push('}');
    body
}

/// A body whose replacement of `f` in `EDITED` gives a diff of exactly
/// `lines` lines: each statement adds one.
fn body_with_diff(lines: usize, long: bool) -> String {
    let service = Service::new(&[("app.ts", EDITED), ("body.ts", &body_with(1, long))]);
    let preview = document(
        &run(
            &service,
            &[
                "--replace-body",
                "--name",
                "f",
                "app.ts",
                "--from",
                "body.ts",
                "--json",
            ],
        ),
        0,
    );
    let base = diff_lines(&preview, 0) - 1;
    body_with(lines - base, long)
}

/// Reference occurrences under, at, and over the default, of a short name
/// and a long one, in every channel and through both entries.
#[test]
fn reference_occurrences_in_every_channel_and_entry() {
    let limit = default(&["--references"], "occurrences");
    for long in [false, true] {
        let name = spelled("x", long);
        for count in [limit - 1, limit, limit + 1] {
            let service = Service::new(&[("app.ts", &occurrences_of(&name, count))]);
            let args = ["--references", "--name", &name];
            every_channel(
                &service,
                PROJECT,
                &args,
                "occurrences",
                0,
                Form::Result,
                false,
                limit,
                count,
            );
        }
    }
}

/// Outline entries under, at, and over the default, in a wide shallow file
/// and one of long names and signatures, in every channel, for a file and
/// for a project through both entries.
#[test]
fn outline_entries_in_every_channel_and_entry() {
    let limit = default(&["--outline"], "entries");
    for long in [false, true] {
        for count in [limit - 1, limit, limit + 1] {
            let service = Service::new(&[("app.ts", &declarations(count, long))]);
            for entries in [FILE, PROJECT] {
                every_channel(
                    &service,
                    entries,
                    &["--outline", "app.ts"],
                    "entries",
                    0,
                    Form::Result,
                    false,
                    limit,
                    count,
                );
            }
        }
    }
}

/// Show's lines under, at, and over the default, short and long, in every
/// channel, for a file and for a project through both entries.
#[test]
fn show_lines_in_every_channel_and_entry() {
    let limit = default(&["--show"], "lines");
    for long in [false, true] {
        for count in [limit - 1, limit, limit + 1] {
            let service = Service::new(&[("app.ts", &function_with(count, long))]);
            for entries in [FILE, PROJECT] {
                every_channel(
                    &service,
                    entries,
                    &["--show", "--name", "f", "app.ts"],
                    "lines",
                    0,
                    Form::Result,
                    false,
                    limit,
                    count,
                );
            }
        }
    }
}

/// An ambiguous selection's candidates under, at, and over the default,
/// short and long, in show, type, and both single edits, in every channel:
/// a diagnostic in text, a document with `--json`.
#[test]
fn candidates_in_every_channel_and_entry() {
    let cases: [(&[&str], &str, &[Entries], bool); 4] = [
        (
            &["--show", "--name", "f", "app.ts"],
            "",
            &[FILE, PROJECT],
            false,
        ),
        (&["--type", "--name", "f"], "", &[PROJECT], false),
        (
            &[
                "--replace-body",
                "--name",
                "f",
                "app.ts",
                "--from",
                "body.ts",
            ],
            "{ return; }",
            &[FILE, PROJECT],
            true,
        ),
        (
            &["--insert-after", "--name", "f", "--from", "body.ts"],
            "export {};\n",
            &[PROJECT],
            true,
        ),
    ];
    for (args, body, entries, editing) in cases {
        let limit = default(&args[..1], "candidates");
        for long in [false, true] {
            for count in [limit - 1, limit, limit + 1] {
                let service =
                    Service::new(&[("app.ts", &namesakes_with(count, long)), ("body.ts", body)]);
                for entries in entries {
                    every_channel(
                        &service,
                        entries,
                        args,
                        "candidates",
                        1,
                        Form::Diagnostic,
                        editing,
                        limit,
                        count,
                    );
                }
            }
        }
    }
}

/// A preview's diff lines under, at, and over the default, short and long,
/// in every channel, for a file and for a project through both entries.
#[test]
fn preview_diff_lines_in_every_channel_and_entry() {
    let limit = default(&["--replace-body"], "diff_lines");
    for long in [false, true] {
        for count in [limit - 1, limit, limit + 1] {
            let service = Service::new(&[
                ("app.ts", EDITED),
                ("body.ts", &body_with_diff(count, long)),
            ]);
            for entries in [FILE, PROJECT] {
                every_channel(
                    &service,
                    entries,
                    &[
                        "--replace-body",
                        "--name",
                        "f",
                        "app.ts",
                        "--from",
                        "body.ts",
                    ],
                    "diff_lines",
                    0,
                    Form::Result,
                    true,
                    limit,
                    count,
                );
            }
        }
    }
}
