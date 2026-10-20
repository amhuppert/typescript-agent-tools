// validation: run alone: its queries race a three-second deadline, which other test executables' load can break.
//! A failed read-only query may discard its process and warm project cache.
#![cfg(unix)]
mod support;

use serde_json::Value;
use std::process::Output;
use support::service::Service;

fn fixture() -> Service {
    // Ordinary loading work outlasts the one-millisecond deadline tests.
    fixture_with_padding(1000)
}

/// A project whose loading keeps the process above a one-megabyte memory
/// threshold for several of the watchdog's 25-millisecond samples, even in
/// an optimized build, where the ordinary fixture loads within one.
fn memory_fixture() -> Service {
    fixture_with_padding(100_000)
}

fn fixture_with_padding(count: usize) -> Service {
    let mut source = String::from("export const value = 1;\n");
    for i in 0..count {
        source.push_str(&format!("const padding{i} = {i};\n"));
    }
    Service::new(&[("value.ts", &source)])
}

fn query(service: &Service, direct: bool, limit: Option<(&str, &str)>) -> Output {
    query_as(service, direct, limit, true)
}

/// `query`, in text unless `json`.
fn query_as(service: &Service, direct: bool, limit: Option<(&str, &str)>, json: bool) -> Output {
    let mut command = service.command();
    command.args(["--type", "--project", "tsconfig.json", "--name", "value"]);
    if json {
        command.arg("--json");
    }
    command
        .env_remove("TSPARSE_MAX_MEMORY_MB")
        .env_remove("TSPARSE_QUERY_TIMEOUT_MS");
    if direct {
        command.arg("--direct");
    }
    if let Some((name, value)) = limit {
        command.env(name, value);
    }
    command.output().unwrap()
}

/// A variable that is not valid, as the `--json` query reports it: one
/// usage document on stdout naming `option`, nothing on stderr, exit 2.
fn invalid(output: &Output, option: &str) {
    assert_eq!(output.status.code(), Some(2), "{option}: {output:?}");
    assert!(output.stderr.is_empty(), "{option}: {output:?}");
    let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(answer["operation"], "type", "{answer}");
    assert_eq!(answer["outcome"], "error", "{answer}");
    assert_eq!(answer["error"]["category"], "usage", "{answer}");
    assert_eq!(answer["error"]["code"], "usage.environment", "{answer}");
    assert!(
        answer["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{option} must be a positive integer")),
        "{answer}"
    );
}

/// A query a limit stopped, as its document reports it: exit 1, the
/// limit's reason, and its message, which the caller checks, and its
/// remedy, which says what to change and never to stop or restart a
/// service. The message is returned.
fn limited(output: Output, reason: &str) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let answer: Value = serde_json::from_slice(&output.stdout).expect("structured query failure");
    assert_eq!(answer["outcome"], "error");
    assert_eq!(answer["error"]["reason"], reason, "{answer}");
    assert!(answer.get("type").is_none(), "{answer}");
    let variable = match reason {
        "semantic_deadline" => "TSPARSE_QUERY_TIMEOUT_MS",
        _ => "TSPARSE_MAX_MEMORY_MB",
    };
    assert_eq!(
        answer["error"]["remedy"],
        format!("set a higher {variable} before retrying"),
        "{answer}"
    );
    assert!(answer["error"].get("next").is_none(), "{answer}");
    let message = answer["error"]["message"].as_str().unwrap().to_string();
    for restart in ["stop", "restart", "--references"] {
        assert!(!message.contains(restart), "{message}");
    }
    message
}

/// What a limit's failure says it cost in each context.
const DIRECT: &str = "this query process exited";
const SERVICE: &str = " in the project service; that service and its cache exited, and the next request starts a replacement";

/// The memory message: the footprint observed, above the configured
/// threshold of one MiB, then `context`'s cost.
fn memory_message(message: &str, cost: &str) {
    let observed = message
        .strip_prefix("query memory footprint ")
        .and_then(|rest| {
            rest.split_once(" bytes exceeded TSPARSE_MAX_MEMORY_MB (1 MiB, 1048576 bytes)")
        })
        .unwrap_or_else(|| panic!("{message}"));
    assert!(observed.0.parse::<u64>().unwrap() > 1 << 20, "{message}");
    assert_eq!(observed.1, cost, "{message}");
}

/// The text form of a limit's failure: its message, then its remedy on a
/// `next` line, on stderr, exit 1.
fn limited_text(output: Output, message: &str, variable: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!("{message}\n  next: set a higher {variable} before retrying\n")
    );
}

fn healthy(output: Output) -> Value {
    assert!(output.status.success(), "{output:?}");
    let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(answer["outcome"], "found");
    assert_eq!(answer["type"]["summary"], "1");
    answer
}

#[test]
fn direct_memory_threshold_fails_the_query_and_a_new_process_succeeds() {
    let service = memory_fixture();
    let limit = Some(("TSPARSE_MAX_MEMORY_MB", "1"));
    let message = limited(query(&service, true, limit), "semantic_memory_limit");
    memory_message(&message, &format!("; {DIRECT}"));
    // Text: the message and its remedy, on stderr.
    let output = query_as(&service, true, limit, false);
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    let (message, _) = stderr.split_once('\n').unwrap();
    memory_message(message, &format!("; {DIRECT}"));
    limited_text(output, message, "TSPARSE_MAX_MEMORY_MB");
    healthy(query(&service, true, None));
}

#[test]
fn direct_timeout_fails_the_query_and_a_new_process_succeeds() {
    let service = fixture();
    let limit = Some(("TSPARSE_QUERY_TIMEOUT_MS", "1"));
    let message = format!("query exceeded TSPARSE_QUERY_TIMEOUT_MS (1 ms); {DIRECT}");
    assert_eq!(
        limited(query(&service, true, limit), "semantic_deadline"),
        message
    );
    limited_text(
        query_as(&service, true, limit, false),
        &message,
        "TSPARSE_QUERY_TIMEOUT_MS",
    );
    // With `-o`, the document goes there and nothing to stdout or stderr;
    // the text diagnostic never creates the file.
    let written = service.fixture.path("limited.json");
    let mut command = service.command();
    command
        .args([
            "--type",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
            "--direct",
            "--json",
            "-o",
            "limited.json",
        ])
        .env_remove("TSPARSE_MAX_MEMORY_MB")
        .env("TSPARSE_QUERY_TIMEOUT_MS", "1");
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        output.stdout.is_empty() && output.stderr.is_empty(),
        "{output:?}"
    );
    let document: Value =
        serde_json::from_str(&std::fs::read_to_string(&written).unwrap()).unwrap();
    assert_eq!(document["error"]["message"], message.as_str());
    assert_eq!(
        document["error"]["remedy"],
        "set a higher TSPARSE_QUERY_TIMEOUT_MS before retrying"
    );
    let mut command = service.command();
    command
        .args([
            "--type",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
            "--direct",
            "-o",
            "limited.txt",
        ])
        .env_remove("TSPARSE_MAX_MEMORY_MB")
        .env("TSPARSE_QUERY_TIMEOUT_MS", "1");
    limited_text(
        command.output().unwrap(),
        &message,
        "TSPARSE_QUERY_TIMEOUT_MS",
    );
    assert!(!service.fixture.path("limited.txt").exists());
    healthy(query(&service, true, None));
}

/// File navigation runs in this process under the same limits, so its
/// limit failures read as a direct query's, with the same remedy, in text
/// and as a document.
#[test]
fn file_navigation_reports_its_limits_as_this_process() {
    let service = memory_fixture();
    let outline = |limit: (&str, &str), json: bool| {
        let mut command = service.command();
        command
            .args(["--outline", "value.ts"])
            .env_remove("TSPARSE_MAX_MEMORY_MB")
            .env_remove("TSPARSE_QUERY_TIMEOUT_MS")
            .env(limit.0, limit.1);
        if json {
            command.arg("--json");
        }
        command.output().unwrap()
    };
    let memory = ("TSPARSE_MAX_MEMORY_MB", "1");
    let message = limited(outline(memory, true), "semantic_memory_limit");
    memory_message(&message, &format!("; {DIRECT}"));
    let output = outline(memory, false);
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    let (message, _) = stderr.split_once('\n').unwrap();
    memory_message(message, &format!("; {DIRECT}"));
    limited_text(output, message, "TSPARSE_MAX_MEMORY_MB");
    let timeout = ("TSPARSE_QUERY_TIMEOUT_MS", "1");
    let message = format!("query exceeded TSPARSE_QUERY_TIMEOUT_MS (1 ms); {DIRECT}");
    assert_eq!(
        limited(outline(timeout, true), "semantic_deadline"),
        message
    );
    limited_text(
        outline(timeout, false),
        &message,
        "TSPARSE_QUERY_TIMEOUT_MS",
    );
    // No service was involved.
    assert!(!service.runtime.exists());
}

#[test]
fn service_memory_failure_discards_cache_and_next_request_restarts() {
    let service = memory_fixture();
    let limit = Some(("TSPARSE_MAX_MEMORY_MB", "1"));
    let message = limited(query(&service, false, limit), "semantic_memory_limit");
    memory_message(&message, SERVICE);
    assert_eq!(service.status()["running"], false);
    let served = healthy(query(&service, false, None));
    assert_eq!(service.status()["running"], true);
    assert_eq!(served, healthy(query(&service, true, None)));
    assert!(service.run(&["--references-stop"]).status.success());
    let output = query_as(&service, false, limit, false);
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    let (message, _) = stderr.split_once('\n').unwrap();
    memory_message(message, SERVICE);
    limited_text(output, message, "TSPARSE_MAX_MEMORY_MB");
}

#[test]
fn service_timeout_discards_cache_and_next_request_restarts() {
    let service = fixture();
    let limit = Some(("TSPARSE_QUERY_TIMEOUT_MS", "1"));
    let message = format!("query exceeded TSPARSE_QUERY_TIMEOUT_MS (1 ms){SERVICE}");
    assert_eq!(
        limited(query(&service, false, limit), "semantic_deadline"),
        message
    );
    assert_eq!(service.status()["running"], false);
    healthy(query(&service, false, None));
    assert_eq!(service.status()["running"], true);
    assert!(service.run(&["--references-stop"]).status.success());
    limited_text(
        query_as(&service, false, limit, false),
        &message,
        "TSPARSE_QUERY_TIMEOUT_MS",
    );
    // A replacement starts with the environment of the request that starts it.
    healthy(query(&service, false, None));
}

/// A service run in the foreground for a supervisor, which a query's limit
/// stops, exits with the code of that query's failure, and its help's exit
/// table says what that code means.
#[test]
fn a_foreground_service_a_limit_stops_exits_as_its_help_says() {
    let service = fixture();
    let serving = service
        .command()
        .args(["--references-serve", "--project", "tsconfig.json"])
        .env("TSPARSE_QUERY_TIMEOUT_MS", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let started = std::time::Instant::now();
    while service.status()["running"] != true {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "the foreground service did not start"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let message = format!("query exceeded TSPARSE_QUERY_TIMEOUT_MS (1 ms){SERVICE}");
    assert_eq!(
        limited(query(&service, false, None), "semantic_deadline"),
        message
    );
    let exited = serving.wait_with_output().unwrap();
    assert_eq!(exited.status.code(), Some(1), "{exited:?}");
    assert!(exited.stderr.is_empty(), "{exited:?}");
    let help = service
        .command()
        .args(["--references-serve", "--help", "--json"])
        .output()
        .unwrap();
    assert!(help.status.success(), "{help:?}");
    let help: Value = serde_json::from_slice(&help.stdout).unwrap();
    let exits = help["exits"].as_array().unwrap();
    let meaning = exits
        .iter()
        .find(|exit| exit["code"] == 1)
        .unwrap_or_else(|| panic!("no exit 1 in {exits:?}"))["meaning"]
        .as_str()
        .unwrap();
    assert!(meaning.contains("TSPARSE_QUERY_TIMEOUT_MS"), "{meaning}");
    assert!(meaning.contains("TSPARSE_MAX_MEMORY_MB"), "{meaning}");
}

#[test]
fn invalid_process_limits_are_reported_before_query_execution() {
    let service = fixture();
    for option in ["TSPARSE_MAX_MEMORY_MB", "TSPARSE_QUERY_TIMEOUT_MS"] {
        for value in ["0", "-1", "invalid", "18446744073709551616"] {
            invalid(&query(&service, true, Some((option, value))), option);
        }
    }
}

#[test]
fn invalid_service_limits_preserve_the_configuration_error() {
    for option in ["TSPARSE_MAX_MEMORY_MB", "TSPARSE_QUERY_TIMEOUT_MS"] {
        let service = fixture();
        invalid(&query(&service, false, Some((option, "0"))), option);
    }
}

#[test]
fn a_completed_request_cancels_its_watchdog_while_the_service_is_idle() {
    let service = fixture();
    let first = healthy(query(
        &service,
        false,
        Some(("TSPARSE_QUERY_TIMEOUT_MS", "3000")),
    ));
    let before = service.status();
    std::thread::sleep(std::time::Duration::from_millis(3100));
    let after = service.status();
    assert_eq!(after["running"], true);
    assert_eq!(after["pid"], before["pid"]);
    assert_eq!(after["generation"], before["generation"]);
    assert_eq!(healthy(query(&service, false, None)), first);
}

#[test]
fn memory_threshold_stops_growth_after_a_small_query_succeeds_under_the_same_limit() {
    let small = fixture_with_padding(10);
    healthy(query(&small, true, Some(("TSPARSE_MAX_MEMORY_MB", "32"))));
    let large = fixture_with_padding(50_000);
    let message = limited(
        query(&large, true, Some(("TSPARSE_MAX_MEMORY_MB", "32"))),
        "semantic_memory_limit",
    );
    assert!(
        message.contains(" bytes exceeded TSPARSE_MAX_MEMORY_MB (32 MiB, 33554432 bytes); "),
        "{message}"
    );
    // The large, flat, valid project succeeds under the production defaults.
    healthy(query(&large, true, None));
}
