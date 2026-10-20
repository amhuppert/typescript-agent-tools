// validation: run alone: its service starts must become ready within ten seconds, which other test executables' load can break.
//! A command that would start or reach a project's service reports why it
//! cannot: an environment the service would refuse and a runtime directory
//! it could not use are found by the client before it starts anything, with
//! the remedy; a failure only the starting service can see reaches the
//! client through its startup diagnostic file; and each transport failure
//! keeps its own code, in text and, with `--json`, as a document. Nothing is
//! left running.
#![cfg(unix)]
mod support;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use support::service::{Service, json};

fn service() -> Service {
    Service::new(&[
        ("value.ts", "export function value() { return 1; }\n"),
        ("body.txt", "{ return 2; }"),
        (
            "edits.json",
            r#"{"edits": [{"operation": "replace_body", "name": "value", "file": "value.ts", "fragment": "{ return 3; }"}]}"#,
        ),
    ])
}

/// Every positive integer variable, with the accepted form and the default
/// its message names.
const VARIABLES: [(&str, &str); 3] = [
    (
        "TSPARSE_MAX_MEMORY_MB",
        "a positive integer number of MiB up to 17592186044415 (default 6144)",
    ),
    (
        "TSPARSE_QUERY_TIMEOUT_MS",
        "a positive integer number of milliseconds (default 60000)",
    ),
    (
        "TSPARSE_SERVICE_IDLE_SECS",
        "a positive integer number of seconds (default 600)",
    ),
];

/// Values no variable accepts, each with the form its message shows. An
/// empty value, or one with a space or a line break, is quoted, so the
/// message stays one line and shows where the value ends; a value that is
/// not Unicode is shown lossily.
const INVALID: [(&[u8], &str); 9] = [
    (b"0", "0"),
    (b"-1", "-1"),
    (b"ten", "ten"),
    (b"1.5", "1.5"),
    (b"", "\"\""),
    (b" 1", "\" 1\""),
    (b"1\n", "\"1\\n\""),
    (b"\xff", "\u{FFFD}"),
    (b"18446744073709551616", "18446744073709551616"),
];

/// The smallest number of MiB whose bytes overflow, which only memory refuses.
const MEMORY_OVERFLOW: &str = "17592186044416";

/// The files the editing previews below would write.
const EDIT_OUTPUTS: [&str; 2] = ["preview.txt", "plan.json"];

/// Commands that start the project's service when none is running: queries
/// in text and JSON, and editing previews that would save a result and a plan.
const SERVICE_BACKED: [&[&str]; 5] = [
    &["--references", "--name", "value"],
    &["--outline", "value.ts"],
    &["--show", "--json", "--name", "value"],
    &[
        "--replace-body",
        "--name",
        "value",
        "--from",
        "body.txt",
        "-o",
        "preview.txt",
        "--plan",
        "plan.json",
    ],
    &[
        "--edit-batch",
        "edits.json",
        "--json",
        "-o",
        "preview.txt",
        "--plan",
        "plan.json",
    ],
];

/// The same commands answered by a project loaded in this process.
const IN_PROCESS: [&[&str]; 5] = [
    &["--references", "--direct", "--name", "value"],
    &["--type", "--direct", "--name", "value", "--json"],
    &["--outline", "--direct", "value.ts"],
    &[
        "--replace-body",
        "--direct",
        "--name",
        "value",
        "--from",
        "body.txt",
        "-o",
        "preview.txt",
        "--plan",
        "plan.json",
    ],
    &[
        "--edit-batch",
        "edits.json",
        "--direct",
        "--json",
        "-o",
        "preview.txt",
        "--plan",
        "plan.json",
    ],
];

/// `args` for the project, from its directory, with only `variable` of the
/// limits set.
fn with(service: &Service, variable: &str, value: &OsStr, args: &[&str]) -> Output {
    let mut command = service.command();
    command
        .env_remove("TSPARSE_MAX_MEMORY_MB")
        .env_remove("TSPARSE_QUERY_TIMEOUT_MS")
        .env(variable, value)
        .args(args)
        .args(["--project", "tsconfig.json"]);
    finished(command)
}

/// The output of `command`, which must exit within a minute: a foreground
/// service that started by mistake would otherwise never return.
fn finished(command: Command) -> Output {
    finished_within(command, Duration::from_secs(60))
}

/// The output of `command`, which must exit within `limit`.
fn finished_within(mut command: Command, limit: Duration) -> Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > limit {
            child.kill().unwrap();
            panic!("{command:?} did not exit: {:?}", child.wait_with_output());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

/// What every transport failure of a request for `tsconfig.json` ends its
/// message with: the command that diagnoses it.
const DIAGNOSE: &str = "; run: tsparse --doctor --project tsconfig.json";

/// `message`, which ends with a newline, as a failure with `code` ends it:
/// a transport failure's with `DIAGNOSE`, before any `why` or `next` line.
fn diagnosed(code: &str, message: &str) -> String {
    if !code.starts_with("transport.") {
        return message.to_string();
    }
    let end = ["\n  why: ", "\n  next: "]
        .iter()
        .filter_map(|line| message.find(line))
        .min()
        .unwrap_or(message.len() - 1);
    format!("{}{DIAGNOSE}{}", &message[..end], &message[end..])
}

/// A failure with `code` and `message`, which ends with a newline: exit 2,
/// and with `json` one document on stdout and nothing on stderr, or else
/// nothing on stdout and exactly `code: message` on stderr. A transport
/// failure's message ends by naming the doctor (`diagnosed`).
fn refused(output: &Output, code: &str, message: &str, json: bool, context: &str) {
    let message = &diagnosed(code, message);
    assert_eq!(output.status.code(), Some(2), "{context}: {output:?}");
    if json {
        assert!(output.stderr.is_empty(), "{context}: {output:?}");
        let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["outcome"], "error", "{context}: {document}");
        assert_eq!(document["error"]["code"], code, "{context}: {document}");
        assert_eq!(
            document["error"]["message"],
            message.strip_suffix('\n').unwrap(),
            "{context}"
        );
    } else {
        assert!(output.stdout.is_empty(), "{context}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!("{code}: {message}"),
            "{context}"
        );
    }
}

/// The files in the runtime directory, by name.
fn runtime_files(service: &Service) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(&service.runtime)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// The key of the project's endpoint, from the lock file an earlier start
/// left behind: lock files are never removed.
fn key(service: &Service) -> String {
    let names = runtime_files(service);
    let keys: Vec<&str> = names
        .iter()
        .filter_map(|name| name.strip_suffix(".lock"))
        .filter(|key| key.len() == 16 && key.bytes().all(|b| b.is_ascii_hexdigit()))
        .collect();
    assert_eq!(keys.len(), 1, "{names:?}");
    keys[0].to_owned()
}

#[test]
fn an_invalid_variable_is_reported_by_every_entry_and_starts_nothing() {
    let service = service();
    for (variable, form) in VARIABLES {
        let mut invalid: Vec<(&[u8], &str)> = INVALID.to_vec();
        if variable == "TSPARSE_MAX_MEMORY_MB" {
            invalid.push((MEMORY_OVERFLOW.as_bytes(), MEMORY_OVERFLOW));
        }
        for (value, shown) in invalid {
            let value = OsStr::from_bytes(value);
            let message = format!("{variable} must be {form}, got {shown}\n");
            for args in SERVICE_BACKED.iter().chain(&IN_PROCESS) {
                let context = format!("{variable}={value:?} {args:?}");
                refused(
                    &with(&service, variable, value, args),
                    "usage.environment",
                    &message,
                    args.contains(&"--json"),
                    &context,
                );
                // No service was spawned: it would have created the runtime
                // directory, its lock, and its socket. Nothing was written.
                assert!(!service.runtime.exists(), "{context}");
                for output in EDIT_OUTPUTS {
                    assert!(
                        !service.fixture.path(output).exists(),
                        "{context}: {output}"
                    );
                }
            }
            let mut file = service.command();
            file.env_remove("TSPARSE_MAX_MEMORY_MB")
                .env_remove("TSPARSE_QUERY_TIMEOUT_MS")
                .env(variable, value)
                .args(["--outline", "value.ts"]);
            refused(
                &finished(file),
                "usage.environment",
                &message,
                false,
                &format!("{variable}={value:?} file outline"),
            );
            // The foreground service prints the message the client did,
            // before it creates anything.
            let serve = with(&service, variable, value, &["--references-serve"]);
            refused(
                &serve,
                "usage.environment",
                &message,
                false,
                &format!("{variable}={value:?} --references-serve"),
            );
            assert!(!service.runtime.exists(), "{variable}={value:?}");
        }
    }
    assert_eq!(service.status()["running"], false);
}

#[test]
fn every_variable_accepts_its_largest_value_and_a_signed_integer() {
    let service = service();
    let largest = [
        ("TSPARSE_MAX_MEMORY_MB", "17592186044415"),
        ("TSPARSE_QUERY_TIMEOUT_MS", "18446744073709551615"),
        ("TSPARSE_SERVICE_IDLE_SECS", "18446744073709551615"),
    ];
    let mut expected = None;
    for values in [
        &largest[..],
        &[
            ("TSPARSE_MAX_MEMORY_MB", "+6144"),
            ("TSPARSE_QUERY_TIMEOUT_MS", "+60000"),
            ("TSPARSE_SERVICE_IDLE_SECS", "+60"),
        ],
    ] {
        for direct in [true, false] {
            let mut command = service.command();
            command.envs(values.iter().copied()).args([
                "--references",
                "--name",
                "value",
                "--project",
                "tsconfig.json",
                "--json",
            ]);
            if direct {
                command.arg("--direct");
            }
            let answer = json(finished(command));
            assert_eq!(answer["outcome"], "complete", "{values:?} {answer}");
            assert_eq!(expected.get_or_insert_with(|| answer.clone()), &answer);
        }
        assert!(service.run(&["--references-stop"]).status.success());
    }
    // The smallest values are pinned where they take effect: 1 for each
    // limit in tests/process_limits.rs, and 1 idle second in
    // tests/reference_service.rs.
}

/// The message for a runtime directory the service could not safely use.
fn not_private(directory: &Path) -> String {
    format!(
        "Project service directory must be a private directory owned by this user with mode 0700: {}",
        directory.display()
    )
}

/// Why a runtime directory with group or other permissions is refused.
const OPEN: &str = "any process that can open the socket can run queries as you";

/// Why a runtime directory its owner cannot list, write, or search is.
const CLOSED: &str = "a service could not create its socket or record why it failed";

/// The remedy for a runtime directory that is not this user's directory.
const ANOTHER_DIRECTORY: &str =
    "set TSPARSE_SERVICE_DIR to a new path or to a private directory this user owns";

/// The commands a transport failure reaches, as text and as documents:
/// project queries of every operation, project edits, and the service
/// actions, which have no document.
const REACHING: [&[&str]; 11] = [
    &["--references", "--name", "value"],
    &["--references", "--json", "--name", "value"],
    &["--outline", "value.ts"],
    &["--outline", "--json", "value.ts"],
    &["--show", "--json", "--name", "value"],
    &["--type", "--json", "--name", "value"],
    &[
        "--replace-body",
        "--name",
        "value",
        "--from",
        "body.txt",
        "--json",
    ],
    &["--edit-batch", "edits.json", "--json"],
    &["--references-status"],
    &["--references-status", "--json"],
    &["--references-stop"],
];

/// A transport failure, exactly as `args` delivers `error`, exit 2: with
/// `--json` one document naming the operation, and an edit's effect, on
/// stdout and nothing on stderr; in text its `CODE: message`, `why`, and
/// `next` lines on stderr and nothing on stdout. Its message ends by naming
/// the doctor.
fn transport(output: &Output, args: &[&str], error: &serde_json::Value) {
    let mut error = error.clone();
    error["message"] = format!("{}{DIAGNOSE}", error["message"].as_str().unwrap()).into();
    let error = &error;
    assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
    if args.contains(&"--json") {
        assert!(output.stderr.is_empty(), "{args:?}: {output:?}");
        let stdout = String::from_utf8(output.stdout.clone()).unwrap();
        assert_eq!(stdout.lines().count(), 1, "{args:?}: {stdout}");
        let document: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        let mut expected = serde_json::json!({"outcome": "error", "error": error});
        for (flag, operation) in [
            ("--references", "references"),
            ("--references-status", "references-status"),
            ("--outline", "outline"),
            ("--show", "show"),
            ("--type", "type"),
            ("--replace-body", "edit"),
            ("--edit-batch", "edit"),
        ] {
            if args.contains(&flag) {
                expected["operation"] = operation.into();
                if operation == "edit" {
                    expected["effect"] = "none".into();
                }
            }
        }
        assert_eq!(document, expected, "{args:?}");
    } else {
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        let field = |name: &str| error.get(name).and_then(|value| value.as_str());
        let mut text = format!(
            "{}: {}\n",
            field("code").unwrap(),
            field("message").unwrap()
        );
        if let Some(why) = field("why") {
            text.push_str(&format!("  why: {why}\n"));
        }
        if let Some(argv) = error.get("next") {
            let argv: Vec<&str> = argv["argv"]
                .as_array()
                .unwrap()
                .iter()
                .map(|argument| argument.as_str().unwrap())
                .collect();
            // Each is a plain word, which a shell line names unquoted.
            assert!(
                argv.iter().all(|argument| argument
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))),
                "{argv:?}"
            );
            text.push_str(&format!("  next: {}\n", argv.join(" ")));
        }
        if let Some(remedy) = field("remedy") {
            text.push_str(&format!("  next: {remedy}\n"));
        }
        assert_eq!(String::from_utf8_lossy(&output.stderr), text, "{args:?}");
    }
}

/// The command a failure's text prints on its `next` line.
fn printed_next(output: &Output) -> String {
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    let next: Vec<&str> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("  next: "))
        .collect();
    assert_eq!(next.len(), 1, "{stderr}");
    next[0].to_owned()
}

#[test]
fn a_runtime_directory_with_other_permissions_names_chmod_and_why() {
    let service = service();
    let set = |mode| {
        std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(mode)).unwrap()
    };
    std::fs::create_dir(&service.runtime).unwrap();
    let directory = service.runtime.to_str().unwrap();
    // Any group or other permission, and an owner without read, write, or
    // search, without which a service could neither create its files nor
    // record why, so only the client can report it.
    let mut outputs = Vec::new();
    for (mode, why) in [
        (0o740, OPEN),
        (0o720, OPEN),
        (0o710, OPEN),
        (0o704, OPEN),
        (0o702, OPEN),
        (0o701, OPEN),
        (0o755, OPEN),
        (0o777, OPEN),
        (0o570, OPEN),
        (0o300, CLOSED),
        (0o500, CLOSED),
        (0o600, CLOSED),
    ] {
        set(mode);
        let every = [0o755, 0o500].contains(&mode);
        for args in REACHING {
            if every || args == REACHING[0] || args == ["--references-status"] {
                outputs.push((mode, why, args, service.run(args)));
            }
        }
    }
    // Restored before asserting, so the directory can always be removed.
    set(0o700);
    for (mode, why, args, output) in &outputs {
        transport(
            output,
            args,
            &serde_json::json!({
                "category": "transport",
                "code": "transport.directory",
                "message": not_private(&service.runtime),
                "why": why,
                "next": {"argv": ["chmod", "700", directory]},
            }),
        );
        if !args.contains(&"--json") {
            assert_eq!(
                printed_next(output),
                format!("chmod 700 {directory}"),
                "{mode:o}"
            );
        }
    }
    // No service started: it would have left its lock file.
    assert_eq!(runtime_files(&service), Vec::<String>::new());
    // Following the printed command makes the next request succeed.
    for mode in [0o755, 0o500] {
        set(mode);
        let output = service.run(REACHING[0]);
        let chmod = Command::new("sh")
            .args(["-c", &printed_next(&output)])
            .status()
            .unwrap();
        assert!(chmod.success(), "{mode:o}");
        assert_eq!(service.query("value")["outcome"], "complete", "{mode:o}");
        assert!(service.run(&["--references-stop"]).status.success());
    }
}

#[test]
fn a_runtime_directory_that_is_not_this_users_names_another_directory() {
    let service = service();
    let refused = |directory: &Path, args: &[&str], output: &Output| {
        transport(
            output,
            args,
            &serde_json::json!({
                "category": "transport",
                "code": "transport.directory",
                "message": not_private(directory),
                "remedy": ANOTHER_DIRECTORY,
            }),
        );
    };
    let with_directory = |directory: &Path, args: &[&str]| {
        let mut command = service.command();
        command
            .env("TSPARSE_SERVICE_DIR", directory)
            .args(args)
            .args(["--project", "tsconfig.json"]);
        finished(command)
    };
    // Another user's directory, where the platform has one: the system's
    // temporary directory belongs to the superuser.
    let shared = std::fs::canonicalize("/tmp").unwrap();
    let owner = std::fs::metadata(&shared).unwrap();
    let user = std::fs::metadata(&service.fixture.root).unwrap();
    {
        use std::os::unix::fs::MetadataExt;
        if owner.uid() == user.uid() {
            eprintln!(
                "SKIP a_runtime_directory_that_is_not_this_users_names_another_directory: this user owns {}, so no directory of another user's is known",
                shared.display()
            );
        } else {
            for args in REACHING {
                refused(&shared, args, &with_directory(&shared, args));
            }
        }
    }
    // A path naming another kind of file, or a link to a private directory.
    let private = PathBuf::from(format!("{}-target", service.runtime.display()));
    std::fs::create_dir(&private).unwrap();
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::os::unix::fs::symlink(&private, &service.runtime).unwrap();
    let linked: Vec<_> = REACHING
        .iter()
        .map(|args| (*args, service.run(args)))
        .collect();
    std::fs::remove_file(&service.runtime).unwrap();
    std::fs::write(&service.runtime, "").unwrap();
    let file: Vec<_> = REACHING
        .iter()
        .map(|args| (*args, service.run(args)))
        .collect();
    std::fs::remove_file(&service.runtime).unwrap();
    let left: Vec<_> = std::fs::read_dir(&private).unwrap().collect();
    std::fs::remove_dir(&private).unwrap();
    for (args, output) in linked.iter().chain(&file) {
        refused(&service.runtime, args, output);
    }
    assert!(left.is_empty(), "{left:?}");
    // Following the remedy: a new path serves the request.
    assert_eq!(service.query("value")["outcome"], "complete");
}

#[test]
fn a_start_that_waits_too_long_for_another_is_a_timeout() {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn flock(descriptor: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
    }
    let service = service();
    service.query("value");
    assert!(service.run(&["--references-stop"]).status.success());
    // Another client holds the start lock, and no service answers.
    let lock = std::fs::File::open(
        service
            .runtime
            .join(format!("{}.start.lock", key(&service))),
    )
    .unwrap();
    // LOCK_EX | LOCK_NB. SAFETY: the file owns a live descriptor.
    assert_eq!(unsafe { flock(lock.as_raw_fd(), 2 | 4) }, 0);
    let waiting: Vec<_> = [REACHING[0], REACHING[4]]
        .into_iter()
        .map(|args| {
            let mut command = service.command();
            command.args(args).args(["--project", "tsconfig.json"]);
            (args, std::thread::spawn(move || finished(command)))
        })
        .collect();
    for (args, output) in waiting {
        transport(
            &output.join().unwrap(),
            args,
            &serde_json::json!({
                "category": "transport",
                "code": "transport.timeout",
                "message": "Project service did not become ready within 10 seconds while another client was starting it",
            }),
        );
    }
    drop(lock);
    assert_eq!(service.query("value")["outcome"], "complete");
}

#[test]
fn a_request_larger_than_the_service_reads_is_a_protocol_failure() {
    let service = service();
    // A fragment the project's service would receive whole.
    let body = format!("{{ /* {} */ return 2; }}", "x".repeat(1 << 20));
    std::fs::write(service.fixture.path("large.txt"), body).unwrap();
    let args = ["--replace-body", "--name", "value", "--from", "large.txt"];
    for format in [&[][..], &["--json"]] {
        let args = [&args[..], format].concat();
        transport(
            &service.run(&args),
            &args,
            &serde_json::json!({
                "category": "transport",
                "code": "transport.protocol",
                "message": "Project service request exceeds 1 MiB",
            }),
        );
    }
    // A project loaded here prepares it.
    let output = service.run(&[&args[..], &["--direct"]].concat());
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[test]
fn a_runtime_directory_that_cannot_be_created_is_named_with_its_cause() {
    let service = service();
    let check = |directory: &Path, cause: std::io::Error, context: &str| {
        for args in [
            &["--references", "--name", "value"][..],
            &["--show", "--json", "--name", "value"],
        ] {
            let mut command = service.command();
            command
                .env("TSPARSE_SERVICE_DIR", directory)
                .args(args)
                .args(["--project", "tsconfig.json"]);
            refused(
                &finished(command),
                "transport.directory",
                &format!(
                    "Project service directory {} cannot be created: {cause}\n",
                    directory.display()
                ),
                args.contains(&"--json"),
                &format!("{context} {args:?}"),
            );
        }
    };
    // Its parent does not exist.
    let orphan = service.runtime.join("missing").join("runtime");
    let cause = std::fs::create_dir(&orphan).unwrap_err();
    check(&orphan, cause, "a missing parent");
    // Its parent does not accept new entries.
    std::fs::create_dir(&service.runtime).unwrap();
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o500)).unwrap();
    let denied = service.runtime.join("runtime");
    match std::fs::create_dir(&denied) {
        Err(cause) => check(&denied, cause, "a parent without write permission"),
        Ok(()) => {
            std::fs::remove_dir(&denied).unwrap();
            eprintln!(
                "SKIP a_runtime_directory_that_cannot_be_created_is_named_with_its_cause: permissions are not enforced"
            );
        }
    }
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!denied.exists());
}

#[test]
fn a_failure_only_the_starting_service_sees_reaches_the_client_and_its_file_is_removed() {
    let service = service();
    let expected = service.query("value");
    assert!(service.run(&["--references-stop"]).status.success());
    let key = key(&service);
    let lock = service.runtime.join(format!("{key}.lock"));
    let recorded = service.runtime.join(format!("{key}.start-error"));

    // The lease cannot open a lock that is a directory. Only the service
    // opens it, so only the service can see why it cannot start.
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let cause = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock)
        .unwrap_err();
    let message = format!("Project service lock {}: {cause}\n", lock.display());
    for args in [
        &["--references", "--name", "value"][..],
        &["--outline", "--json", "value.ts"],
    ] {
        refused(
            &service.run(args),
            "transport.lock",
            &message,
            args.contains(&"--json"),
            &format!("{args:?}"),
        );
        assert!(!recorded.exists(), "{args:?}: the client removes the file");
        assert_eq!(service.status()["running"], false);
    }
    // The foreground service prints the same cause and leaves it recorded,
    // readable only by its owner, until the next start removes it.
    let serve = finished({
        let mut command = service.command();
        command.args(["--references-serve", "--project", "tsconfig.json"]);
        command
    });
    refused(
        &serve,
        "transport.lock",
        &message,
        false,
        "--references-serve",
    );
    assert_eq!(
        std::fs::read_to_string(&recorded).unwrap(),
        format!("transport.lock\n{}", message.trim_end())
    );
    assert_eq!(
        std::fs::metadata(&recorded).unwrap().permissions().mode() & 0o777,
        0o600
    );
    // A later failure replaces the diagnostic with a new owner-only file.
    std::fs::set_permissions(&recorded, std::fs::Permissions::from_mode(0o644)).unwrap();
    let serve = finished({
        let mut command = service.command();
        command.args(["--references-serve", "--project", "tsconfig.json"]);
        command
    });
    refused(
        &serve,
        "transport.lock",
        &message,
        false,
        "--references-serve again",
    );
    assert_eq!(
        std::fs::metadata(&recorded).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::remove_dir(&lock).unwrap();
    assert_eq!(service.query("value"), expected);
    assert!(!recorded.exists(), "a start removes an earlier diagnostic");
    assert!(service.run(&["--references-stop"]).status.success());

    // A socket path naming a regular file. Where connecting to it reports
    // that no service listens, only the service, which refuses to remove
    // it, sees the cause; elsewhere the client's connection reports it.
    let socket = service.runtime.join(format!("{key}.sock"));
    std::fs::write(&socket, "").unwrap();
    let message = match std::os::unix::net::UnixStream::connect(&socket) {
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => format!(
            "Service socket path names another kind of file: {}\n",
            socket.display()
        ),
        Err(error) => format!("Project service socket {}: {error}\n", socket.display()),
        Ok(_) => unreachable!("a regular file accepts no connection"),
    };
    refused(
        &service.run(&["--references", "--name", "value"]),
        "transport.socket",
        &message,
        false,
        "a regular file at the socket path",
    );
    assert!(!recorded.exists());
    std::fs::remove_file(&socket).unwrap();
    assert_eq!(service.status()["running"], false);
    assert_eq!(service.query("value"), expected);
}

#[test]
fn a_startup_diagnostic_that_cannot_be_removed_is_reported_and_starts_nothing() {
    let service = service();
    service.query("value");
    assert!(service.run(&["--references-stop"]).status.success());
    let recorded = service
        .runtime
        .join(format!("{}.start-error", key(&service)));
    std::fs::create_dir(&recorded).unwrap();
    let cause = std::fs::remove_file(&recorded).unwrap_err();
    // Every command that would start the service, in text and as documents.
    for args in &REACHING[..8] {
        transport(
            &service.run(args),
            args,
            &serde_json::json!({
                "category": "transport",
                "code": "transport.startup",
                "message": format!(
                    "Project service startup diagnostic {} cannot be removed: {cause}",
                    recorded.display()
                ),
            }),
        );
        assert_eq!(service.status()["running"], false, "{args:?}");
    }
    std::fs::remove_dir(&recorded).unwrap();
}

/// The runtime directory's startup diagnostics, complete or partial.
fn diagnostics(service: &Service) -> Vec<String> {
    runtime_files(service)
        .into_iter()
        .filter(|name| name.contains("start-error"))
        .collect()
}

/// A project whose service cannot open its lock, which only the service
/// sees, and that lock and the message naming it.
fn unlockable() -> (Service, PathBuf, String) {
    let service = service();
    service.query("value");
    assert!(service.run(&["--references-stop"]).status.success());
    let lock = service.runtime.join(format!("{}.lock", key(&service)));
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let cause = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock)
        .unwrap_err();
    let message = format!("Project service lock {}: {cause}\n", lock.display());
    (service, lock, message)
}

#[test]
fn simultaneous_starts_each_report_the_cause_their_service_recorded() {
    let (service, _lock, message) = unlockable();
    for round in 0..3 {
        let clients: Vec<_> = (0..12)
            .map(|_| {
                let mut command = service.command();
                command.args([
                    "--references",
                    "--name",
                    "value",
                    "--project",
                    "tsconfig.json",
                ]);
                std::thread::spawn(move || finished(command))
            })
            .collect();
        for (client, output) in clients.into_iter().enumerate() {
            let output = output.join().unwrap();
            refused(
                &output,
                "transport.lock",
                &message,
                false,
                &format!("round {round}, client {client}"),
            );
        }
        assert_eq!(diagnostics(&service), Vec::<String>::new(), "round {round}");
    }
    assert_eq!(service.status()["running"], false);
}

/// What a write beyond the file size limit does.
#[derive(Clone, Copy)]
enum Excess {
    /// It fails with `EFBIG`: `SIGXFSZ` is ignored.
    Fails,
    /// `SIGXFSZ` ends the process mid-write, as an interruption would.
    Kills,
}

/// `command` with a file size limit of `bytes`, inherited by any service it
/// starts.
fn with_file_size_limit(command: &mut Command, bytes: u64, excess: Excess) {
    use std::os::unix::process::CommandExt;
    #[repr(C)]
    struct Limit {
        current: u64,
        maximum: u64,
    }
    unsafe extern "C" {
        fn setrlimit(resource: std::ffi::c_int, limit: *const Limit) -> std::ffi::c_int;
        fn signal(signal: std::ffi::c_int, handler: usize) -> usize;
    }
    /// `RLIMIT_FSIZE`, `SIGXFSZ`, `SIG_DFL`, `SIG_IGN`, and `SIG_ERR`, the
    /// same on macOS and Linux.
    const FILE_SIZE: std::ffi::c_int = 1;
    const EXCEEDED: std::ffi::c_int = 25;
    const DEFAULT: usize = 0;
    const IGNORE: usize = 1;
    const ERROR: usize = usize::MAX;
    let handler = match excess {
        Excess::Fails => IGNORE,
        Excess::Kills => DEFAULT,
    };
    let limit = Limit {
        current: bytes,
        maximum: bytes,
    };
    // SAFETY: between fork and exec the closure calls only signal and
    // setrlimit, which are async-signal-safe, with a live limit.
    unsafe {
        command.pre_exec(move || {
            if signal(EXCEEDED, handler) == ERROR || setrlimit(FILE_SIZE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[test]
fn a_diagnostic_that_cannot_be_written_completely_is_never_reported_as_the_cause() {
    let (service, lock, cause) = unlockable();
    let recorded = service
        .runtime
        .join(format!("{}.start-error", key(&service)));
    let too_large = std::io::Error::from_raw_os_error(27);
    let config = service.fixture.path("tsconfig.json");
    for bytes in [0, 8] {
        // The client reports no partial cause: it names the command that
        // runs the service it started in the foreground.
        let mut command = service.command();
        command.args([
            "--references",
            "--name",
            "value",
            "--project",
            "tsconfig.json",
        ]);
        with_file_size_limit(&mut command, bytes, Excess::Fails);
        let output = finished(command);
        refused(
            &output,
            "transport.startup",
            &unrecorded(&service),
            false,
            &format!("a client limited to {bytes} bytes"),
        );
        assert_eq!(diagnostics(&service), Vec::<String>::new(), "{bytes}");
        // Followed as printed, in the client's environment, it prints both
        // the cause and why it was not recorded.
        let stderr = String::from_utf8(output.stderr).unwrap();
        let next = stderr
            .lines()
            .find_map(|line| line.strip_prefix("  next: "))
            .unwrap_or_else(|| panic!("no next line: {stderr}"));
        let mut serve = Command::new("sh");
        serve
            .args(["-c", next])
            .current_dir(&service.fixture.root)
            .env("TSPARSE_SERVICE_DIR", &service.runtime)
            .env("TSPARSE_SERVICE_IDLE_SECS", support::service::IDLE_SECS)
            .env("PATH", path_with_build())
            .stdin(Stdio::null());
        with_file_size_limit(&mut serve, bytes, Excess::Fails);
        let served = finished(serve);
        assert_eq!(served.status.code(), Some(2), "{next}: {served:?}");
        assert!(served.stdout.is_empty(), "{next}: {served:?}");
        assert_eq!(
            String::from_utf8_lossy(&served.stderr),
            format!(
                "transport.lock: {}; the startup diagnostic cannot be written to {}: {too_large}; run: tsparse --doctor --project {}\n",
                cause.trim_end(),
                recorded.display(),
                config.display()
            ),
            "a foreground service limited to {bytes} bytes"
        );
        assert_eq!(diagnostics(&service), Vec::<String>::new(), "{bytes}");
    }
    std::fs::remove_dir(&lock).unwrap();
}

/// The diagnostic of a start that recorded no cause: the command that runs
/// the service it started, for the configuration's absolute spelling, in
/// the foreground, where its standard error shows the cause.
fn unrecorded(service: &Service) -> String {
    format!(
        "Project service startup failed without recording why; in the foreground it prints the cause\n  next: tsparse --references-serve --project {}\n",
        service.fixture.path("tsconfig.json").display()
    )
}

/// `PATH` with this build's `tsparse` first, so a printed command runs it.
fn path_with_build() -> std::ffi::OsString {
    let build = Path::new(env!("CARGO_BIN_EXE_tsparse"))
        .parent()
        .unwrap()
        .to_path_buf();
    let path = std::env::var_os("PATH").unwrap();
    std::env::join_paths(std::iter::once(build).chain(std::env::split_paths(&path))).unwrap()
}

/// A query command that would start the project's service.
fn query_command(service: &Service) -> Command {
    let mut command = service.command();
    command.args([
        "--references",
        "--name",
        "value",
        "--project",
        "tsconfig.json",
    ]);
    command
}

#[test]
fn interrupted_foreground_starts_leave_one_partial_diagnostic_that_a_start_reclaims() {
    use std::os::unix::process::ExitStatusExt;
    let (service, lock, _) = unlockable();
    for attempt in 0..3 {
        let mut serve = service.command();
        serve.args(["--references-serve", "--project", "tsconfig.json"]);
        with_file_size_limit(&mut serve, 0, Excess::Kills);
        let output = finished(serve);
        assert_eq!(output.status.signal(), Some(25), "{attempt}: {output:?}");
        // Each attempt reclaims the copy the one before abandoned.
        assert_eq!(
            diagnostics(&service).len(),
            1,
            "{attempt}: {:?}",
            diagnostics(&service)
        );
    }
    std::fs::remove_dir(&lock).unwrap();
    assert_eq!(service.query("value")["outcome"], "complete");
    assert_eq!(diagnostics(&service), Vec::<String>::new());
}

#[test]
fn an_interrupted_automatic_start_reports_no_cause_and_leaves_nothing() {
    let (service, lock, _) = unlockable();
    let mut command = query_command(&service);
    with_file_size_limit(&mut command, 0, Excess::Kills);
    refused(
        &finished(command),
        "transport.startup",
        &unrecorded(&service),
        false,
        "a service killed while recording",
    );
    assert_eq!(diagnostics(&service), Vec::<String>::new());

    // A client cancelled once its service began recording leaves at most
    // the partial copy, which the next start reclaims.
    let mut command = query_command(&service);
    with_file_size_limit(&mut command, 0, Excess::Kills);
    let mut client = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while diagnostics(&service).is_empty() && client.try_wait().unwrap().is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "the client never started a service"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    client.kill().unwrap();
    client.wait().unwrap();
    assert!(
        diagnostics(&service).len() <= 1,
        "{:?}",
        diagnostics(&service)
    );
    std::fs::remove_dir(&lock).unwrap();
    assert_eq!(service.query("value")["outcome"], "complete");
    assert_eq!(diagnostics(&service), Vec::<String>::new());
}

#[test]
fn a_partial_diagnostic_being_written_is_never_reclaimed() {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn flock(descriptor: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
    }
    let service = service();
    service.query("value");
    assert!(service.run(&["--references-stop"]).status.success());
    // A copy whose writer holds its lock, as a service recording a
    // diagnostic does.
    let staged = service
        .runtime
        .join(format!("{}.start-error.staged", key(&service)));
    let writer = std::fs::File::create(&staged).unwrap();
    // LOCK_EX | LOCK_NB. SAFETY: the file owns a live descriptor.
    assert_eq!(unsafe { flock(writer.as_raw_fd(), 2 | 4) }, 0);
    assert_eq!(service.query("value")["outcome"], "complete");
    assert!(staged.exists(), "a start removed a copy being written");
    assert!(service.run(&["--references-stop"]).status.success());
    // Once its writer is gone, the next start reclaims it. A process another
    // test spawns at this moment can hold a copy of the writer's descriptor,
    // and so its lock, until it executes; a start then still leaves the copy.
    drop(writer);
    let waiting = Instant::now();
    loop {
        assert_eq!(service.query("value")["outcome"], "complete");
        if diagnostics(&service).is_empty() {
            break;
        }
        assert!(
            waiting.elapsed() < Duration::from_secs(10),
            "a released copy was never reclaimed"
        );
        assert!(service.run(&["--references-stop"]).status.success());
    }
}

#[test]
fn a_staged_diagnostic_of_another_kind_is_reported_promptly_and_left_in_place() {
    use std::os::unix::fs::FileTypeExt;
    let service = service();
    service.query("value");
    assert!(service.run(&["--references-stop"]).status.success());
    let staged = service
        .runtime
        .join(format!("{}.start-error.staged", key(&service)));
    let message = |reason: &str| {
        format!(
            "Project service startup diagnostic {} cannot be removed: {reason}\n",
            staged.display()
        )
    };
    let target = service.runtime.join("target");
    std::fs::write(&target, "").unwrap();
    // A pipe with no writer, which a blocking open would wait on; a
    // directory; and a link to a regular file, which says it is a link.
    let kinds: [(&str, &dyn Fn(), &str); 3] = [
        (
            "a pipe",
            &|| {
                let made = Command::new("mkfifo").arg(&staged).status().unwrap();
                assert!(made.success());
            },
            "not a regular file",
        ),
        (
            "a directory",
            &|| std::fs::create_dir(&staged).unwrap(),
            "not a regular file",
        ),
        (
            "a link",
            &|| std::os::unix::fs::symlink(&target, &staged).unwrap(),
            "a symbolic link, not a regular file",
        ),
    ];
    for (kind, make, reason) in kinds {
        make();
        let kept = std::fs::symlink_metadata(&staged).unwrap().file_type();
        let output = finished_within(query_command(&service), Duration::from_secs(5));
        refused(&output, "transport.startup", &message(reason), false, kind);
        assert_eq!(service.status()["running"], false, "{kind}");
        let left = std::fs::symlink_metadata(&staged).unwrap().file_type();
        assert_eq!(
            (left.is_fifo(), left.is_dir(), left.is_symlink()),
            (kept.is_fifo(), kept.is_dir(), kept.is_symlink()),
            "{kind}: refused, not removed"
        );
        if left.is_dir() {
            std::fs::remove_dir(&staged).unwrap();
        } else {
            std::fs::remove_file(&staged).unwrap();
        }
    }
    assert_eq!(service.query("value")["outcome"], "complete");
    assert!(target.exists());
}
