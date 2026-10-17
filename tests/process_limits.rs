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

fn fixture_with_padding(count: usize) -> Service {
    let mut source = String::from("export const value = 1;\n");
    for i in 0..count {
        source.push_str(&format!("const padding{i} = {i};\n"));
    }
    Service::new(&[("value.ts", &source)])
}

fn query(service: &Service, direct: bool, limit: Option<(&str, &str)>) -> Output {
    let mut command = service.command();
    command.args([
        "--type",
        "--project",
        "tsconfig.json",
        "--name",
        "value",
        "--json",
    ]);
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

fn limited(output: Output, reason: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let answer: Value = serde_json::from_slice(&output.stdout).expect("structured query failure");
    assert_eq!(answer["outcome"], "error");
    assert_eq!(answer["error"]["reason"], reason, "{answer}");
    assert!(answer.get("type").is_none(), "{answer}");
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
    let service = fixture();
    limited(
        query(&service, true, Some(("TSPARSE_MAX_MEMORY_MB", "1"))),
        "semantic_memory_limit",
    );
    healthy(query(&service, true, None));
}

#[test]
fn direct_timeout_fails_the_query_and_a_new_process_succeeds() {
    let service = fixture();
    limited(
        query(&service, true, Some(("TSPARSE_QUERY_TIMEOUT_MS", "1"))),
        "semantic_deadline",
    );
    healthy(query(&service, true, None));
}

#[test]
fn service_memory_failure_discards_cache_and_next_request_restarts() {
    let service = fixture();
    limited(
        query(&service, false, Some(("TSPARSE_MAX_MEMORY_MB", "1"))),
        "semantic_memory_limit",
    );
    assert_eq!(service.status()["running"], false);
    let served = healthy(query(&service, false, None));
    assert_eq!(service.status()["running"], true);
    assert_eq!(served, healthy(query(&service, true, None)));
}

#[test]
fn service_timeout_discards_cache_and_next_request_restarts() {
    let service = fixture();
    limited(
        query(&service, false, Some(("TSPARSE_QUERY_TIMEOUT_MS", "1"))),
        "semantic_deadline",
    );
    assert_eq!(service.status()["running"], false);
    healthy(query(&service, false, None));
    assert_eq!(service.status()["running"], true);
}

#[test]
fn invalid_process_limits_are_reported_before_query_execution() {
    let service = fixture();
    for option in ["TSPARSE_MAX_MEMORY_MB", "TSPARSE_QUERY_TIMEOUT_MS"] {
        for value in ["0", "-1", "invalid", "18446744073709551616"] {
            let output = query(&service, true, Some((option, value)));
            assert_eq!(
                output.status.code(),
                Some(2),
                "{option}={value}: {output:?}"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(option),
                "{output:?}"
            );
        }
    }
}

#[test]
fn invalid_service_limits_preserve_the_configuration_error() {
    for option in ["TSPARSE_MAX_MEMORY_MB", "TSPARSE_QUERY_TIMEOUT_MS"] {
        let service = fixture();
        let output = query(&service, false, Some((option, "0")));
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(option),
            "{output:?}"
        );
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
    limited(
        query(&large, true, Some(("TSPARSE_MAX_MEMORY_MB", "32"))),
        "semantic_memory_limit",
    );
    // The large, flat, valid project succeeds under the production defaults.
    healthy(query(&large, true, None));
}
