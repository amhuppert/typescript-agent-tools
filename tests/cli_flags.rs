//! The CLI vocabulary as the real binary applies it: a flag outside its
//! operation's scope names exactly the operations that accept it, an
//! unknown option points back into help, `--output` is `-o`, and each
//! operation keeps its repeated-option policy. What each operation accepts
//! is read from `tsparse --help --json`, so these tests hold the parser to
//! the flags help lists.
mod support;

use serde_json::Value;
use std::process::{Command, Output, Stdio};
use support::Fixture;

/// An invocation each operation accepts, by name, in registry order.
const INVOCATIONS: &[(&str, &[&str])] = &[
    ("parse", &[]),
    ("outline", &["--outline", "app.ts"]),
    ("show", &["--show", "--name", "run", "app.ts"]),
    (
        "type",
        &["--type", "--project", "tsconfig.json", "--name", "run"],
    ),
    (
        "references",
        &[
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "run",
        ],
    ),
    (
        "replace-body",
        &[
            "--replace-body",
            "--name",
            "run",
            "--from",
            "body.ts",
            "app.ts",
        ],
    ),
    (
        "insert-after",
        &[
            "--insert-after",
            "--name",
            "run",
            "--from",
            "body.ts",
            "app.ts",
        ],
    ),
    ("edit-batch", &["--edit-batch", "edits.json"]),
    ("apply", &["--apply", "plan.json"]),
    (
        "references-status",
        &["--references-status", "--project", "tsconfig.json"],
    ),
    (
        "references-stop",
        &["--references-stop", "--project", "tsconfig.json"],
    ),
    (
        "references-serve",
        &["--references-serve", "--project", "tsconfig.json"],
    ),
    ("doctor", &["--doctor"]),
];

/// An operation as help publishes it.
struct Operation {
    /// How scope errors name it: by its flag, and parse mode by none.
    label: String,
    /// An invocation it accepts.
    base: &'static [&'static str],
    /// Every flag it accepts besides its own and the global ones.
    flags: Vec<String>,
    /// Whether `--pretty` stands alone.
    pretty_alone: bool,
}

/// A flag that selects no operation, as written, with its registered name
/// and whether it takes a value.
struct Spelled {
    spelling: String,
    name: String,
    takes_value: bool,
}

/// The complete registry document.
fn registry() -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(["--help", "--json"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap().to_string())
        .collect()
}

/// Each operation in registry order.
fn operations(registry: &Value) -> Vec<Operation> {
    let global = strings(&registry["global_flags"]);
    let operations = registry["operations"].as_array().unwrap();
    assert_eq!(
        operations
            .iter()
            .map(|operation| operation["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        INVOCATIONS
            .iter()
            .map(|&(name, _)| name)
            .collect::<Vec<_>>()
    );
    operations
        .iter()
        .zip(INVOCATIONS)
        .map(|(operation, &(_, base))| {
            let own = operation["flag"].as_str();
            Operation {
                label: own.unwrap_or("parse mode (no operation flag)").into(),
                base,
                flags: operation["flags"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|flag| flag["name"].as_str().unwrap().to_string())
                    .filter(|name| Some(name.as_str()) != own && !global.contains(name))
                    .collect(),
                pretty_alone: operation["pretty"] == "alone",
            }
        })
        .collect()
}

/// Every spelling of every flag that selects no operation and is not
/// global.
fn options(registry: &Value) -> Vec<Spelled> {
    let global = strings(&registry["global_flags"]);
    let mut options = Vec::new();
    for flag in registry["flags"].as_array().unwrap() {
        let name = flag["name"].as_str().unwrap().to_string();
        if !flag["selects"].is_null() || global.contains(&name) {
            continue;
        }
        let mut spellings = vec![name.clone()];
        spellings.extend(strings(&flag["aliases"]));
        for spelling in spellings {
            options.push(Spelled {
                spelling,
                name: name.clone(),
                takes_value: !flag["value"].is_null(),
            });
        }
    }
    options
}

const APPLY_REFUSAL: &str = "--apply takes only the plan: it applies the recorded files and edits without selecting them again";
const BATCH_REFUSAL: &str = "--edit-batch names every edit's target and fragment in the batch file";

/// Run the binary in an empty directory where nothing it might reach
/// exists, under an idle time every query and service start refuses, so an
/// accepted request ends before it loads a project or starts a service.
fn run(fixture: &Fixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
        .current_dir(&fixture.root)
        .env("TSPARSE_SERVICE_DIR", fixture.path("services"))
        .env("TSPARSE_SERVICE_IDLE_SECS", "0")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// Run the binary in `fixture` in the caller's environment.
fn tsparse(fixture: &Fixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
        .current_dir(&fixture.root)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// A usage failure in text: exit 2, nothing on stdout, and `code: message`
/// on stderr.
fn usage(output: &Output, code: &str, message: &str, args: &[&str]) {
    assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
    assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
    assert_eq!(
        text(&output.stderr),
        format!("{code}: {message}\n"),
        "{args:?}"
    );
}

const IDLE: &str =
    "TSPARSE_SERVICE_IDLE_SECS must be a positive integer number of seconds (default 600), got 0";

#[test]
fn a_flag_out_of_scope_names_exactly_the_operations_that_accept_it() {
    let fixture = Fixture::new(&[]);
    let registry = registry();
    let operations = operations(&registry);
    let options = options(&registry);
    // Every alias is checked too.
    assert!(options.iter().any(|option| option.spelling == "-o"));
    for option in &options {
        let name = option.name.as_str();
        let accepting: Vec<&str> = operations
            .iter()
            .filter(|operation| operation.flags.iter().any(|flag| flag == name))
            .map(|operation| operation.label.as_str())
            .collect();
        for operation in &operations {
            let label = operation.label.as_str();
            let mut args = operation.base.to_vec();
            args.push(&option.spelling);
            if option.takes_value {
                args.push("x");
            }
            let output = run(&fixture, &args);
            let stderr = text(&output.stderr);
            if !operation.flags.iter().any(|flag| flag == name) {
                let (code, message) = match (label, name) {
                    ("--apply", "--project" | "--at" | "--name") => {
                        ("usage.selection", APPLY_REFUSAL.into())
                    }
                    ("--edit-batch", "--at" | "--name" | "--from") => {
                        ("usage.selection", BATCH_REFUSAL.into())
                    }
                    _ => (
                        "usage.flag_scope",
                        format!("{name} applies to: {}", accepting.join(", ")),
                    ),
                };
                usage(&output, code, &message, &args);
            } else if name == "--pretty" && !operation.pretty_alone {
                usage(
                    &output,
                    "usage.flag_scope",
                    &format!("--pretty applies to {label} only with --json"),
                    &args,
                );
            } else {
                assert!(
                    !stderr.contains(" applies to: ")
                        && !stderr.starts_with("usage.unknown_option"),
                    "{args:?}: {stderr}"
                );
            }
        }
    }
    // A flag no operation accepts out of every scope is never listed.
    let output = run(&fixture, &["--json"]);
    usage(
        &output,
        "usage.flag_scope",
        "--json applies to: --outline, --show, --type, --references, --replace-body, --insert-after, --edit-batch, --apply, --references-status, --doctor",
        &["--json"],
    );
}

#[test]
fn operations_are_chosen_once_and_references_direct_is_references_with_direct() {
    let fixture = Fixture::new(&[]);
    for (args, message) in [
        (
            &[
                "--type",
                "--references",
                "--project",
                "c.json",
                "--name",
                "x",
            ][..],
            "Choose one operation: --type, --references",
        ),
        (
            &[
                "--replace-body",
                "--insert-after",
                "--name",
                "x",
                "--from",
                "body.ts",
                "app.ts",
            ],
            "Choose one operation: --replace-body, --insert-after",
        ),
        (
            &["--outline", "--show", "--name", "run", "app.ts"],
            "Choose one operation: --outline, --show",
        ),
        (
            &["--edit-batch", "edits.json", "--apply", "plan.json"],
            "Choose one operation: --edit-batch, --apply",
        ),
        (
            &[
                "--references-status",
                "--references-stop",
                "--project",
                "c.json",
            ],
            "Choose one operation: --references-status, --references-stop",
        ),
        (
            &[
                "--references-direct",
                "--references-serve",
                "--project",
                "c.json",
            ],
            "Choose one operation: --references-direct, --references-serve",
        ),
        (
            &[
                "--type",
                "--references-direct",
                "--project",
                "c.json",
                "--name",
                "x",
            ],
            "Choose one operation: --type, --references-direct",
        ),
    ] {
        usage(&run(&fixture, args), "usage.operation", message, args);
        // As with every other request a flag cannot form, help still answers.
        let help = run(&fixture, &[args, &["--help"]].concat());
        assert_eq!(help.status.code(), Some(0), "{args:?}: {help:?}");
        assert!(
            help.stderr.is_empty() && !help.stdout.is_empty(),
            "{help:?}"
        );
    }
    // Each spelling of the one references operation, together or alone,
    // reaches the query, which this environment's idle time then refuses.
    for args in [
        &["--references-direct", "--project", "c.json", "--name", "x"][..],
        &[
            "--references",
            "--references-direct",
            "--project",
            "c.json",
            "--name",
            "x",
        ],
        &[
            "--references-direct",
            "--references-direct",
            "--project",
            "c.json",
            "--name",
            "x",
        ],
    ] {
        usage(&run(&fixture, args), "usage.environment", IDLE, args);
    }
}

/// A service action implies `--references`, so the parser has always taken
/// `--references` beside one: before or after the action, it changes nothing.
#[test]
fn references_beside_a_service_action_changes_nothing() {
    let fixture = Fixture::new(&[]);
    let idle = format!("usage.environment: {IDLE}\n");
    for (action, code, stdout, stderr) in [
        ("--references-status", 0, "no service: missing.json\n", ""),
        ("--references-stop", 0, "no service: missing.json\n", ""),
        // Serve reaches its environment, which this one refuses.
        ("--references-serve", 2, "", idle.as_str()),
    ] {
        for args in [
            &[action, "--project", "missing.json"][..],
            &["--references", action, "--project", "missing.json"],
            &[action, "--project", "missing.json", "--references"],
        ] {
            let output = run(&fixture, args);
            assert_eq!(output.status.code(), Some(code), "{args:?}: {output:?}");
            assert_eq!(text(&output.stdout), stdout, "{args:?}");
            assert_eq!(text(&output.stderr), stderr, "{args:?}");
        }
    }
    // `--references-direct` is a query's flag, which no service action takes.
    let args = [
        "--references-status",
        "--references-direct",
        "--project",
        "c.json",
    ];
    usage(
        &run(&fixture, &args),
        "usage.operation",
        "Choose one operation: --references-status, --references-direct",
        &args,
    );
}

#[test]
fn an_unknown_option_names_the_nearest_flag_only_when_one_is_close() {
    let fixture = Fixture::new(&[]);
    for (arg, nearest) in [
        ("--ouput", "--output"),
        ("-output", "--output"),
        ("--Output", "--output"),
        ("--outputs", "--output"),
        ("--Apply", "--apply"),
        ("--refrences", "--references"),
        ("--no-test", "--no-tests"),
        ("--projct", "--project"),
        ("-help", "--help"),
    ] {
        usage(
            &run(&fixture, &[arg, "out.json"]),
            "usage.unknown_option",
            &format!("Unknown option: {arg}; did you mean {nearest}? See tsparse --help"),
            &[arg],
        );
        // The suggestion, followed as printed, is an option the parser takes.
        let followed = run(&fixture, &[nearest, "out.json"]);
        assert!(
            !text(&followed.stderr).starts_with("usage.unknown_option"),
            "{nearest}: {followed:?}"
        );
    }
    for arg in ["--frobnicate", "-x", "-O", "-5", "--", "--apply=plan.json"] {
        usage(
            &run(&fixture, &[arg]),
            "usage.unknown_option",
            &format!("Unknown option: {arg}. See tsparse --help"),
            &[arg],
        );
    }
    let help = run(&fixture, &["--help"]);
    assert_eq!(help.status.code(), Some(0), "{help:?}");
    assert!(
        help.stderr.is_empty() && !help.stdout.is_empty(),
        "{help:?}"
    );
}

#[test]
fn output_is_the_registered_name_of_o() {
    let fixture = Fixture::new(&[("app.ts", "export function run() {}\n")]);
    for args in [&[][..], &["--outline", "app.ts"]] {
        let short = tsparse(&fixture, &[args, &["-o", "short.out"]].concat());
        let long = tsparse(&fixture, &[args, &["--output", "long.out"]].concat());
        assert_eq!(short.status.code(), Some(0), "{short:?}");
        assert_eq!(long.status.code(), Some(0), "{long:?}");
        assert!(short.stdout.is_empty() && long.stdout.is_empty());
        let written = std::fs::read(fixture.path("long.out")).unwrap();
        assert!(!written.is_empty());
        assert_eq!(std::fs::read(fixture.path("short.out")).unwrap(), written);
        std::fs::remove_file(fixture.path("short.out")).unwrap();
        std::fs::remove_file(fixture.path("long.out")).unwrap();
    }
    let missing = run(&fixture, &["--outline", "app.ts", "-o"]);
    usage(
        &missing,
        "usage.missing_value",
        "Missing value for -o",
        &["-o"],
    );
    let missing = run(&fixture, &["--outline", "app.ts", "--output"]);
    usage(
        &missing,
        "usage.missing_value",
        "Missing value for --output",
        &["--output"],
    );
}

#[test]
fn parse_and_references_take_the_last_value_and_the_others_reject_a_repeat() {
    let fixture = support::project(&[("app.ts", "export function run() {}\nrun();\n")]);
    // Parse mode: the last output wins and the first is never created.
    let output = run(&fixture, &["-o", "first.json", "--output", "last.json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(!fixture.path("first.json").exists());
    let ast: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.path("last.json")).unwrap()).unwrap();
    assert_eq!(ast["kind"], 308);
    // References: the last name selects.
    let query = ["--references", "--project", "tsconfig.json", "--direct"];
    let once = tsparse(&fixture, &[&query[..], &["--name", "run"]].concat());
    let twice = tsparse(
        &fixture,
        &[&query[..], &["--name", "absent", "--name", "run"]].concat(),
    );
    assert_eq!(once.status.code(), Some(0), "{once:?}");
    assert_eq!(twice.status.code(), Some(0), "{twice:?}");
    assert_eq!(twice.stdout, once.stdout);
    // Navigation, type, editing, and the service actions reject a repeat.
    for (args, message) in [
        (
            &["--show", "--name", "x", "--name", "run", "app.ts"][..],
            "--show takes --name at most once",
        ),
        (
            &["--outline", "-o", "a.txt", "--output", "b.txt", "app.ts"],
            "--outline takes --output at most once",
        ),
        (
            &[
                "--type",
                "--project",
                "a.json",
                "--project",
                "tsconfig.json",
                "--name",
                "run",
            ],
            "--type takes --project at most once",
        ),
        (
            &["--apply", "a.json", "--apply", "b.json"],
            "--apply may be given at most once",
        ),
        (
            &[
                "--references-status",
                "--references-status",
                "--project",
                "tsconfig.json",
            ],
            "--references-status may be given at most once",
        ),
        (
            &[
                "--references-serve",
                "--project",
                "tsconfig.json",
                "--references-serve",
            ],
            "--references-serve may be given at most once",
        ),
    ] {
        usage(&run(&fixture, args), "usage.repeated_flag", message, args);
    }
    // A repeated switch changes nothing.
    let output = tsparse(&fixture, &["--outline", "--json", "--json", "app.ts"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}
