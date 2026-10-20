//! The CLI registry as the binary publishes it in `tsparse --help --json`:
//! the generated reference `docs/cli-reference.md` and its drift check, the
//! registry's contract, and help resolution. The parser-side assertions
//! (every registered spelling is accepted and only those are, every
//! dispatched operation has an entry, and every message names registered
//! flags) are unit tests in `src/cli/request.rs` and `src/cli/registry.rs`.
//!
//! Regenerate the reference with
//! `cargo test --test cli_registry -- --ignored write_reference`.
mod support;

use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use support::Fixture;
use support::service::Service;

const REFERENCE: &str = "docs/cli-reference.md";

/// Run the binary in `fixture` with a runtime directory inside it that
/// nothing creates unless a service is started or contacted.
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

/// The help `output` printed: exit 0 and nothing on stderr.
fn printed(output: &Output, args: &[&str]) -> String {
    assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
    assert!(output.stderr.is_empty(), "{args:?}: {output:?}");
    text(&output.stdout)
}

fn help(fixture: &Fixture, args: &[&str]) -> String {
    printed(&tsparse(fixture, args), args)
}

/// The complete registry document.
fn registry() -> Value {
    let fixture = Fixture::new(&[]);
    serde_json::from_str(&help(&fixture, &["--help", "--json"])).unwrap()
}

fn operations(registry: &Value) -> &Vec<Value> {
    registry["operations"].as_array().unwrap()
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap())
        .collect()
}

/// The arguments that ask for `operation`'s help, as its pointer prints
/// them: the flag, its placeholder when it takes a value, and `--help`.
fn help_args(operation: &Value) -> Vec<&str> {
    let mut args: Vec<&str> = Vec::new();
    if let Some(flag) = operation["flag"].as_str() {
        args.push(flag);
        let entry = flag_entry(operation, flag);
        if let Some(value) = entry["value"].as_str() {
            args.push(value);
        }
    }
    args.push("--help");
    args
}

/// The entry of `name` among `operation`'s flags.
fn flag_entry<'a>(operation: &'a Value, name: &str) -> &'a Value {
    operation["flags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|flag| flag["name"] == name)
        .unwrap_or_else(|| panic!("{} lists no {name}", operation["name"]))
}

/// A flag as help lists it: its aliases and name, then its value.
fn label(flag: &Value) -> String {
    let mut spellings = strings(&flag["aliases"]);
    spellings.push(flag["name"].as_str().unwrap());
    let mut label = spellings.join(", ");
    if let Some(value) = flag["value"].as_str() {
        label.push(' ');
        label.push_str(value);
    }
    label
}

/// `text` as Markdown prose: `<` escaped, so `/// <reference>` is not
/// read as a tag.
fn prose(text: &str) -> String {
    text.replace('<', "&lt;")
}

/// `text` as a Markdown table cell: prose with `|` escaped.
fn cell(text: &str) -> String {
    prose(text).replace('|', "\\|")
}

/// `path`, relative to the repository, as a link from `docs/`.
fn link(path: &str) -> String {
    match path.strip_prefix("docs/") {
        Some(inside) => inside.to_string(),
        None => format!("../{path}"),
    }
}

/// `docs/cli-reference.md`, rendered from the registry document.
fn reference(registry: &Value) -> String {
    let mut out = String::new();
    let operations = operations(registry);
    out.push_str(
        "# tsparse command-line reference\n\n\
         Generated from `tsparse --help --json` by\n\
         `cargo test --test cli_registry -- --ignored write_reference`; change the\n\
         registry in `src/cli/registry.rs`, not this file. `tsparse --help` prints\n\
         the index of these operations and `tsparse --OP --help` one operation's\n\
         section.\n\n## Exit codes\n\n| Code | Meaning |\n| --- | --- |\n",
    );
    for exit in registry["exit_codes"].as_array().unwrap() {
        writeln!(
            out,
            "| {} | {} |",
            exit["code"],
            cell(exit["meaning"].as_str().unwrap())
        )
        .unwrap();
    }
    out.push_str(
        "\nEach operation's section says what its codes report.\n\n## Failure codes\n\n\
         A failure that leaves no result begins its stderr diagnostic with its code,\n\
         `CODE: message`, followed by `  path: message` for each issue, `  why: …`,\n\
         and `  next: …`, a command to run or a remedy to apply. With `--json` it is\n\
         one document,\n\
         `{\"outcome\": \"error\", \"error\": {\"category\", \"code\", \"message\", …}}`, whose\n\
         `error` has the same parts: `issues`, each `{\"path\", \"message\"}` with a JSON\n\
         path whose indexes are zero-based (`edits[1].extra`), `why`, and either\n\
         `next`, a command as `{\"argv\": […]}`, or `remedy`, a sentence. It has\n\
         `operation` when the request names one and `effect` for editing: a usage\n\
         failure's on stdout, never `-o`, and any other's where the result would go.\n\
         Three are always text: a result that cannot be written (`write.output`),\n\
         which is one diagnostic and never a second document; a usage failure about\n\
         `--json` itself, given to an operation that does not take it; and an\n\
         argument that is not Unicode. Every failure exits 2. Analysis\n\
         failures, ambiguous selections, and rejected edits are results with their\n\
         own documents, described in each operation's document.\n\n\
         | Code | Category | Meaning |\n| --- | --- | --- |\n",
    );
    for code in registry["failure_codes"].as_array().unwrap() {
        writeln!(
            out,
            "| `{}` | {} | {} |",
            code["code"].as_str().unwrap(),
            code["category"].as_str().unwrap(),
            cell(code["meaning"].as_str().unwrap())
        )
        .unwrap();
    }
    out.push_str("\n## Operations\n\n");
    out.push_str("| Operation | Flag | Summary |\n| --- | --- | --- |\n");
    for operation in operations {
        let name = operation["name"].as_str().unwrap();
        let flag = match operation["flag"].as_str() {
            Some(flag) => format!("`{flag}`"),
            None => "none".into(),
        };
        writeln!(
            out,
            "| [{name}](#{name}) | {flag} | {} |",
            cell(operation["summary"].as_str().unwrap())
        )
        .unwrap();
    }
    let output = registry["output_flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| format!("`{}`", name.as_str().unwrap()))
        .collect::<Vec<_>>()
        .join(", ");
    let global = strings(&registry["global_flags"])
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(" and ");
    writeln!(
        out,
        "\nThe output flags {output} apply where an operation lists them; {global} apply to every operation."
    )
    .unwrap();
    for operation in operations {
        let name = operation["name"].as_str().unwrap();
        writeln!(
            out,
            "\n## {name}\n\n{}\n",
            prose(operation["summary"].as_str().unwrap())
        )
        .unwrap();
        out.push_str("```text\n");
        for usage in strings(&operation["usage"]) {
            writeln!(out, "{usage}").unwrap();
        }
        out.push_str("```\n\n");
        writeln!(out, "{}\n", prose(operation["details"].as_str().unwrap())).unwrap();
        out.push_str("| Flag | Summary |\n| --- | --- |\n");
        for flag in operation["flags"].as_array().unwrap() {
            writeln!(
                out,
                "| `{}` | {} |",
                label(flag),
                cell(flag["summary"].as_str().unwrap())
            )
            .unwrap();
        }
        writeln!(
            out,
            "\n{}\n\nExamples:\n",
            prose(operation["rules"].as_str().unwrap())
        )
        .unwrap();
        for example in operation["examples"].as_array().unwrap() {
            writeln!(
                out,
                "- `{}`: {}",
                strings(&example["argv"]).join(" "),
                prose(example["summary"].as_str().unwrap())
            )
            .unwrap();
        }
        out.push_str("\n| Exit | Reports |\n| --- | --- |\n");
        for exit in operation["exits"].as_array().unwrap() {
            writeln!(
                out,
                "| {} | {} |",
                exit["code"],
                cell(exit["meaning"].as_str().unwrap())
            )
            .unwrap();
        }
        let related = strings(&operation["related"])
            .iter()
            .map(|related| format!("[{related}](#{related})"))
            .collect::<Vec<_>>()
            .join(", ");
        let doc = operation["doc"].as_str().unwrap();
        writeln!(
            out,
            "\nRelated: {related}. Document: [{doc}]({}).",
            link(doc)
        )
        .unwrap();
    }
    out.push_str("\n## Flags\n\n| Flag | Repeated | Summary |\n| --- | --- | --- |\n");
    for flag in registry["flags"].as_array().unwrap() {
        writeln!(
            out,
            "| `{}` | {} | {} |",
            label(flag),
            flag["repeat_rule"].as_str().unwrap(),
            cell(flag["summary"].as_str().unwrap())
        )
        .unwrap();
    }
    out
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
#[ignore = "writes docs/cli-reference.md"]
fn write_reference() {
    std::fs::write(root().join(REFERENCE), reference(&registry())).unwrap();
}

#[test]
fn the_reference_is_generated_from_the_registry() {
    let checked_in = match std::fs::read_to_string(root().join(REFERENCE)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => panic!("{REFERENCE}: {error}"),
    };
    assert!(
        checked_in == reference(&registry()),
        "{REFERENCE} differs from the registry; regenerate it with \
         `cargo test --test cli_registry -- --ignored write_reference`"
    );
}

/// Assertions 3, 4, and 7: every operation has a summary, usage, an
/// example, and a document that exists; every related edge names an
/// operation; and every entry of one flag agrees with the flag's own, so
/// no flag is a switch in one entry and takes a value in another.
#[test]
fn every_entry_is_complete_and_names_registered_operations_and_flags() {
    let registry = registry();
    let operations = operations(&registry);
    let names: Vec<&str> = operations
        .iter()
        .map(|operation| operation["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names.iter().collect::<BTreeSet<_>>().len(),
        names.len(),
        "{names:?}"
    );
    let flags = registry["flags"].as_array().unwrap();
    let flag = |name: &str| {
        flags
            .iter()
            .find(|flag| flag["name"] == name)
            .unwrap_or_else(|| panic!("no flag {name}"))
    };
    let global = strings(&registry["global_flags"]);
    for name in global.iter().chain(&strings(&registry["output_flags"])) {
        flag(name);
    }
    for operation in operations {
        let name = operation["name"].as_str().unwrap();
        for field in ["summary", "details", "rules", "doc"] {
            assert!(
                !operation[field].as_str().unwrap().is_empty(),
                "{name}: {field}"
            );
        }
        assert!(!strings(&operation["usage"]).is_empty(), "{name}");
        let examples = operation["examples"].as_array().unwrap();
        assert!(!examples.is_empty(), "{name}");
        for example in examples {
            assert_eq!(strings(&example["argv"])[0], "tsparse", "{name}");
            assert!(!example["summary"].as_str().unwrap().is_empty(), "{name}");
        }
        let doc = operation["doc"].as_str().unwrap();
        assert!(root().join(doc).is_file(), "{name}: {doc}");
        for related in strings(&operation["related"]) {
            assert!(
                names.contains(&related) && related != name,
                "{name}: {related}"
            );
        }
        let codes: Vec<u64> = operation["exits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|exit| exit["code"].as_u64().unwrap())
            .collect();
        assert!(
            codes.contains(&0) && codes.contains(&2),
            "{name}: {codes:?}"
        );
        for listed in operation["flags"].as_array().unwrap() {
            assert_eq!(listed, flag(listed["name"].as_str().unwrap()), "{name}");
        }
        for global in &global {
            flag_entry(operation, global);
        }
        if let Some(own) = operation["flag"].as_str() {
            assert_eq!(flag_entry(operation, own)["selects"], name);
        }
    }
    for listed in flags {
        if let Some(selects) = listed["selects"].as_str() {
            assert!(names.contains(&selects), "{listed}");
        }
    }
    let codes: Vec<u64> = registry["exit_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|exit| exit["code"].as_u64().unwrap())
        .collect();
    assert_eq!(codes, [0, 1, 2]);
    // Every failure code is `category.snake_case`, named once, in a
    // category delivery knows.
    let mut failure_codes = BTreeSet::new();
    for entry in registry["failure_codes"].as_array().unwrap() {
        let code = entry["code"].as_str().unwrap();
        let category = entry["category"].as_str().unwrap();
        assert!(
            ["usage", "input", "transport", "write", "internal"].contains(&category),
            "{entry}"
        );
        assert_eq!(
            code.split_once('.').map(|(prefix, _)| prefix),
            Some(category)
        );
        assert!(!entry["meaning"].as_str().unwrap().is_empty(), "{entry}");
        assert!(failure_codes.insert(code), "{code}");
    }
}

/// Assertion 6: the index lists every operation, and each operation's help
/// is its registry entry, listing exactly the flags the entry does.
#[test]
fn the_index_lists_every_operation_and_each_leaf_exactly_its_flags() {
    let fixture = Fixture::new(&[]);
    let registry = registry();
    let index = help(&fixture, &["--help"]);
    for operation in operations(&registry) {
        let summary = operation["summary"].as_str().unwrap();
        let lines: Vec<&str> = index
            .lines()
            .filter(|line| line.ends_with(summary))
            .collect();
        assert_eq!(lines.len(), 1, "{summary}:\n{index}");
        if let Some(flag) = operation["flag"].as_str() {
            assert!(
                lines[0].trim_start().starts_with(&format!("{flag} ")),
                "{index}"
            );
        }
    }
    for exit in registry["exit_codes"].as_array().unwrap() {
        let line = format!("  {}  {}", exit["code"], exit["meaning"].as_str().unwrap());
        assert!(index.lines().any(|l| l == line), "{line}:\n{index}");
    }
    assert!(index.contains("tsparse --OP --help"), "{index}");
    for operation in operations(&registry) {
        let Some(flag) = operation["flag"].as_str() else {
            continue;
        };
        let args = help_args(operation);
        let leaf = help(&fixture, &args);
        assert!(
            leaf.starts_with(&format!(
                "tsparse {flag}: {}\n",
                operation["summary"].as_str().unwrap()
            )),
            "{leaf}"
        );
        for usage in strings(&operation["usage"]) {
            assert!(
                leaf.lines().any(|line| line == format!("  {usage}")),
                "{leaf}"
            );
        }
        let listed: Vec<(String, String)> = leaf
            .lines()
            .skip_while(|line| *line != "Flags:")
            .skip(1)
            .take_while(|line| line.starts_with("  "))
            .map(|line| {
                let (label, summary) = line.trim().split_once("  ").unwrap();
                (label.to_string(), summary.trim().to_string())
            })
            .collect();
        let expected: Vec<(String, String)> = operation["flags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|flag| (label(flag), flag["summary"].as_str().unwrap().to_string()))
            .collect();
        assert_eq!(listed, expected, "{flag}");
        let rules = operation["rules"].as_str().unwrap();
        assert!(leaf.lines().any(|line| line == rules), "{rules}:\n{leaf}");
        for example in operation["examples"].as_array().unwrap() {
            let line = format!("  {}", strings(&example["argv"]).join(" "));
            assert!(leaf.lines().any(|l| l == line), "{line}:\n{leaf}");
        }
        for exit in operation["exits"].as_array().unwrap() {
            let line = format!("  {}  {}", exit["code"], exit["meaning"].as_str().unwrap());
            assert!(leaf.lines().any(|l| l == line), "{line}:\n{leaf}");
        }
        assert!(
            leaf.ends_with(&format!(
                "Document: {}\n",
                operation["doc"].as_str().unwrap()
            )),
            "{leaf}"
        );
        // Each related operation's pointer, followed as printed, prints
        // that operation's help.
        let related = leaf
            .lines()
            .find_map(|line| line.strip_prefix("Related: "))
            .unwrap_or_else(|| panic!("{leaf}"));
        let pointers: Vec<&str> = related.split(", ").collect();
        let names = strings(&operation["related"]);
        assert_eq!(pointers.len(), names.len(), "{related}");
        for (pointer, name) in pointers.iter().zip(names) {
            let target = operations(&registry)
                .iter()
                .find(|operation| operation["name"] == name)
                .unwrap();
            let words: Vec<&str> = pointer.split(' ').collect();
            assert_eq!(words[0], "tsparse", "{pointer}");
            assert_eq!(words[1..], help_args(target)[..], "{pointer}");
            let followed = help(&fixture, &words[1..]);
            assert_eq!(followed, help(&fixture, &help_args(target)), "{pointer}");
        }
        // The structured form is the registry's entry.
        let document: Value =
            serde_json::from_str(&help(&fixture, &[&args[..], &["--json"]].concat())).unwrap();
        assert_eq!(&document, operation, "{flag}");
    }
}

/// Help is the same text however it is asked for and whatever else the
/// request names: it reads no file, no environment variable, and no
/// service, and writes nothing but stdout.
#[cfg(unix)]
#[test]
fn help_reads_nothing_and_starts_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new(&[]);
    let registry = registry();
    let service = Service::new(&[("app.ts", "export function run() {}\nrun();\n")]);
    support::service::json(service.run(&["--references", "--name", "run", "--json"]));
    let generation = service.generation();
    assert!(generation > 0);
    // A project with no service, whose private runtime directory, short
    // enough for a socket, stays empty.
    let idle = Service::new(&[("app.ts", "export function run() {}\n")]);
    std::fs::create_dir(&idle.runtime).unwrap();
    std::fs::set_permissions(&idle.runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let empty = || std::fs::read_dir(&idle.runtime).unwrap().count() == 0;
    let mut requests: Vec<Vec<&str>> = vec![vec!["--help"], vec!["--help", "--json"]];
    for operation in operations(&registry) {
        if operation["flag"].is_string() {
            requests.push(help_args(operation));
            requests.push([&help_args(operation)[..], &["--json"]].concat());
        }
    }
    for request in &requests {
        let expected = tsparse(&fixture, request);
        let expected = printed(&expected, request);
        let short: Vec<&str> = request
            .iter()
            .map(|&arg| if arg == "--help" { "-h" } else { arg })
            .collect();
        assert_eq!(help(&fixture, &short), expected, "{short:?}");
        // Inputs that do not exist, the project with no service, and an
        // output path.
        for named in [
            &["--project", "missing.json", "missing.ts", "-o", "out.txt"][..],
            &["--project", "tsconfig.json", "app.ts", "-o", "out.txt"],
        ] {
            let args = [&request[..], named].concat();
            let output = idle.command().args(&args).output().unwrap();
            assert_eq!(printed(&output, &args), expected, "{args:?}");
            assert!(!idle.fixture.path("out.txt").exists(), "{args:?}");
            assert!(empty(), "{args:?}");
            // An environment every query refuses.
            let output = idle
                .command()
                .args(&args)
                .env("TSPARSE_SERVICE_IDLE_SECS", "0")
                .env("TSPARSE_QUERY_TIMEOUT_MS", "x")
                .output()
                .unwrap();
            assert_eq!(printed(&output, &args), expected, "{args:?}");
        }
        // A running service for the project the request names.
        let output = service
            .command()
            .args(request)
            .args(["--project", "tsconfig.json"])
            .output()
            .unwrap();
        assert_eq!(printed(&output, request), expected, "{request:?}");
    }
    assert_eq!(service.generation(), generation);
    // The runtime directory shows a query that does start a service.
    support::service::json(idle.run(&["--references", "--name", "run", "--json"]));
    assert!(!empty());
}

/// `--help` in any flag position wins over every usage error, and the one
/// operation the flags choose selects its help; a value is no flag position.
#[test]
fn help_wins_over_every_usage_error() {
    let fixture = Fixture::new(&[]);
    let index = help(&fixture, &["--help"]);
    let leaf = |args: &[&str]| help(&fixture, args);
    let show = leaf(&["--show", "--help"]);
    let outline = leaf(&["--outline", "--help"]);
    let apply = leaf(&["--apply", "PLAN", "--help"]);
    let batch = leaf(&["--edit-batch", "EDITS", "--help"]);
    let references = leaf(&["--references", "--help"]);
    for (args, expected) in [
        // Unknown options, before and after.
        (&["--ouput", "--help"][..], &index),
        (&["--help", "--frobnicate"], &index),
        (&["--frobnicate", "--show", "-h", "--ouput"], &show),
        // Missing values: the flag is last, and its operation is chosen.
        (&["--help", "--outline", "--depth"], &outline),
        (&["--help", "--apply"], &apply),
        (&["-h", "--edit-batch"], &batch),
        // Several operations; and flags that name one, however spelled
        // or repeated.
        (&["--type", "--references", "--help"], &index),
        (&["--outline", "--show", "--help", "app.ts"], &index),
        (&["--references-direct", "--help"], &references),
        (&["--references", "--references-direct", "-h"], &references),
        (
            &["--apply", "a.json", "--apply", "b.json", "--help"],
            &apply,
        ),
        // Combination, scope, and repeat errors, and two input files.
        (
            &["--apply", "plan.json", "--project", "x", "--help"],
            &apply,
        ),
        (&["--show", "--depth", "2", "--help"], &show),
        (&["--show", "--name", "a", "--name", "b", "--help"], &show),
        (&["--outline", "a.ts", "b.ts", "--help"], &outline),
        (&["--pretty", "--outline", "--help"], &outline),
        // The version.
        (&["--version", "--help"], &index),
        (&["-h", "-V"], &index),
    ] {
        assert_eq!(&leaf(args), expected, "{args:?}");
    }
    assert!(index != show && show != outline && apply != batch);
    // `--json` and `--pretty` select the document and its indentation.
    let document = leaf(&["--help", "--json"]);
    let pretty = leaf(&["--pretty", "--json", "--frobnicate", "-h"]);
    assert!(!document.contains('\n') || document.trim_end().lines().count() == 1);
    assert!(pretty.lines().count() > 1, "{pretty}");
    assert_eq!(
        serde_json::from_str::<Value>(&pretty).unwrap(),
        serde_json::from_str::<Value>(&document).unwrap()
    );
    assert_eq!(
        leaf(&["--json", "--type", "--references", "--help"]),
        document
    );
    assert_eq!(leaf(&["--help", "--pretty"]), index);
    // Help asks which operations the flags name, not which one a request
    // would run: a service action takes `--references` beside it, but the
    // two flags name two operations, so help is the index.
    for action in [
        "--references-status",
        "--references-stop",
        "--references-serve",
    ] {
        for pair in [["--references", action], [action, "--references"]] {
            assert_eq!(leaf(&[&pair[..], &["--help"]].concat()), index, "{pair:?}");
            assert_eq!(
                leaf(&[&pair[..], &["--help", "--json"]].concat()),
                document,
                "{pair:?}"
            );
        }
    }
    // An argument consumed as a value is not a flag position.
    let output = tsparse(&fixture, &["--outline", "app.ts", "--depth", "--help"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty());
    assert_eq!(
        text(&output.stderr),
        "usage.invalid_value: --depth expects a positive number of levels or all, got --help\n"
    );
    let output = tsparse(&fixture, &["--outline", "app.ts", "-o", "--json", "-h"]);
    assert_eq!(text(&output.stdout), outline);
    assert!(!fixture.path("--json").exists());
}

/// An argument that is not Unicode is a usage error help alone wins over:
/// it refuses every other request, `--version` and an unknown option
/// included, wherever it stands.
#[cfg(unix)]
#[test]
fn help_alone_wins_over_an_argument_that_is_not_unicode() {
    use std::os::unix::ffi::OsStrExt;
    let fixture = Fixture::new(&[]);
    let show = help(&fixture, &["--show", "--help"]);
    let invalid = std::ffi::OsStr::from_bytes(b"\xff.ts");
    let run = |before: &[&str], after: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args(before)
            .arg(invalid)
            .args(after)
            .current_dir(&fixture.root)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    for (before, after) in [
        (&["--show", "--help"][..], &[][..]),
        (&["--show"], &["--help"]),
        (&["--show", "--name"], &["-h"]),
    ] {
        assert_eq!(
            printed(&run(before, after), before),
            show,
            "{before:?} {after:?}"
        );
    }
    let mut refused: Vec<(Vec<&str>, Vec<&str>)> = vec![
        (vec!["--show", "--name", "run"], vec![]),
        (vec!["--ouput"], vec![]),
        (vec![], vec!["--frobnicate"]),
    ];
    for version in ["--version", "-V"] {
        // An operand, then a value, before and after the version.
        refused.extend([
            (vec![version], vec![]),
            (vec![], vec![version]),
            (vec![version, "--show", "--name"], vec![]),
            (vec!["--show", "--name"], vec![version]),
        ]);
    }
    for (before, after) in refused {
        let output = run(&before, &after);
        assert_eq!(output.status.code(), Some(2), "{before:?} {after:?}");
        assert!(output.stdout.is_empty(), "{before:?} {after:?}");
        assert_eq!(
            text(&output.stderr),
            "usage.not_unicode: Arguments must be Unicode: \u{FFFD}.ts\n",
            "{before:?} {after:?}"
        );
    }
}
