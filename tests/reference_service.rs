//! Independent CLI processes share an index and its lifecycle.
#![cfg(unix)]
mod support;

use serde_json::Value;
use std::process::Command;
use support::service::{Service, json};

#[test]
fn independent_invocations_reuse_the_service_and_stop_releases_it() {
    let service = Service::new(&[("a.ts", "export const value = 1; value;")]);
    assert_eq!(service.status()["running"], false);
    let first = service.query("value");
    assert_eq!(
        first["groups"][0]["occurrences"].as_array().unwrap().len(),
        2
    );
    let warm = service.status();
    assert_eq!(
        warm["running"], true,
        "a CLI query must leave a reusable service"
    );
    assert_eq!(warm["generation"], 1);
    assert_eq!(service.query("value"), first);
    assert_eq!(service.status(), warm);
    assert!(service.run(&["--references-stop"]).status.success());
    assert_eq!(service.status()["running"], false);
    assert_eq!(service.query("value"), first);
    assert_ne!(service.status()["pid"], warm["pid"]);
}

#[test]
fn concurrent_cold_clients_share_one_build() {
    let service = Service::new(&[("a.ts", "export const value = 1; value;")]);
    let clients: Vec<_> = (0..4)
        .map(|_| {
            let mut command = service.command();
            command.args([
                "--references",
                "--project",
                "tsconfig.json",
                "--name",
                "value",
            ]);
            std::thread::spawn(move || json(command.output().unwrap()))
        })
        .collect();
    let results: Vec<_> = clients
        .into_iter()
        .map(|client| client.join().unwrap())
        .collect();
    assert!(results.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(service.status()["generation"], 1);
}

#[test]
fn edits_and_failed_refreshes_cross_the_process_boundary() {
    let service = Service::new(&[("a.ts", "export const before = 1;")]);
    assert_eq!(
        service.query("before")["groups"].as_array().unwrap().len(),
        1
    );
    std::fs::write(service.fixture.path("a.ts"), "export const after = 1;").unwrap();
    assert!(
        service.query("before")["groups"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        service.query("after")["groups"].as_array().unwrap().len(),
        1
    );
    assert_eq!(service.status()["generation"], 2);
    std::fs::write(service.fixture.path("a.ts"), "const = ;").unwrap();
    let failure = service.run(&["--references", "--name", "after"]);
    assert_eq!(failure.status.code(), Some(1));
    assert!(failure.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("a.ts:1:"));
    std::fs::write(service.fixture.path("a.ts"), "export const repaired = 1;").unwrap();
    assert_eq!(
        service.query("repaired")["groups"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(service.status()["generation"], 3);
}

#[test]
fn client_working_directory_and_utf16_queries_match_direct_loading() {
    let service = Service::new(&[("src/a.ts", "// 😀\r\nexport const café = 1;\u{2028}café;\n")]);
    let cold = json(service.run(&["--references-direct", "--at", "3:1", "src/a.ts"]));
    assert_eq!(
        cold["groups"][0]["occurrences"].as_array().unwrap().len(),
        2
    );
    let mut client = service.command();
    client
        .current_dir(service.fixture.path("src"))
        .args(["--references", "--project"])
        .arg(service.fixture.path("tsconfig.json"))
        .args(["--at", "3:1", "a.ts"]);
    assert_eq!(json(client.output().unwrap()), cold);
    assert_eq!(service.query("café"), cold);
    assert_eq!(service.status()["generation"], 1);
}

#[test]
fn unrelated_projects_and_configurations_do_not_share_an_index() {
    let left = Service::new(&[("a.ts", "export const left = 1;")]);
    let mut right = Service::new(&[("a.ts", "export const right = 1;")]);
    right.runtime = left.runtime.clone();
    assert_eq!(left.query("left")["groups"].as_array().unwrap().len(), 1);
    assert!(right.query("left")["groups"].as_array().unwrap().is_empty());
    assert_eq!(right.query("right")["groups"].as_array().unwrap().len(), 1);
    assert_ne!(left.status()["pid"], right.status()["pid"]);
    assert!(left.run(&["--references-stop"]).status.success());
    assert!(right.run(&["--references-stop"]).status.success());
}

#[test]
fn crashed_service_releases_its_lease_and_a_new_client_recovers_the_stale_socket() {
    let service = Service::new(&[("a.ts", "export const value = 1;")]);
    let expected = service.query("value");
    let pid = service.status()["pid"].as_u64().unwrap();
    assert!(
        Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()
            .unwrap()
            .success()
    );
    let start = std::time::Instant::now();
    loop {
        let output = service.run(&["--references-status"]);
        if output.status.success() && json(output)["running"] == false {
            break;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "killed service still owns a live socket"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(service.query("value"), expected);
    assert_ne!(service.status()["pid"], pid);
    assert_eq!(service.status()["generation"], 1);
}

#[test]
fn service_rejects_malformed_and_disconnected_clients_without_losing_the_index() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    let service = Service::new(&[("a.ts", "export const value = 1;")]);
    let expected = service.query("value");
    let status = service.status();
    let socket = std::fs::read_dir(&service.runtime)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "sock"))
        .unwrap();
    let mut stream = UnixStream::connect(&socket).unwrap();
    stream.write_all(b"invalid JSON\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.contains("Failure"));
    let mut delayed = UnixStream::connect(&socket).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let write = delayed.write_all(b"invalid JSON\n");
    assert!(
        write.is_ok(),
        "connecting before writing must not close the request: {write:?}"
    );
    let mut response = String::new();
    delayed.read_to_string(&mut response).unwrap();
    assert!(response.contains("Failure"));
    drop(UnixStream::connect(&socket).unwrap());
    assert_eq!(service.query("value"), expected);
    assert_eq!(service.status(), status);
}

#[test]
fn idle_shutdown_releases_the_service() {
    let service = Service::new(&[("a.ts", "export const value = 1;")]);
    let mut command = service.command();
    command.env("TSPARSE_SERVICE_IDLE_SECS", "1").args([
        "--references",
        "--project",
        "tsconfig.json",
        "--name",
        "value",
    ]);
    let expected = json(command.output().unwrap());
    let first = service.status();
    std::thread::sleep(std::time::Duration::from_millis(1300));
    assert_eq!(service.status()["running"], false);
    assert_eq!(service.query("value"), expected);
    assert_ne!(service.status()["pid"], first["pid"]);
}

#[test]
fn runtime_permission_errors_are_not_silent_direct_loads() {
    use std::os::unix::fs::PermissionsExt;
    let service = Service::new(&[("a.ts", "export const value = 1;")]);
    std::fs::create_dir(&service.runtime).unwrap();
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = service.run(&["--references", "--name", "value"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("private directory"));
    assert!(output.stdout.is_empty());
}

#[test]
fn service_results_and_diagnostics_match_direct_cli_output() {
    let service = Service::new(&[("a.ts", "export const value = 1; value; value.missing;")]);
    for query in [
        &["--name", "value", "--pretty"][..],
        &["--name", "absent"],
        &["--at", "1:37", "a.ts"],
        &["--at", "99:1", "a.ts"],
        &["--name", "value", "missing.ts"],
    ] {
        let mut direct = vec!["--references-direct"];
        direct.extend_from_slice(query);
        let mut cached = vec!["--references"];
        cached.extend_from_slice(query);
        let expected = service.run(&direct);
        let actual = service.run(&cached);
        assert_eq!(actual.status.code(), expected.status.code(), "{query:?}");
        assert_eq!(actual.stdout, expected.stdout, "{query:?}");
        assert_eq!(actual.stderr, expected.stderr, "{query:?}");
    }
    let output = service.run(&["--references", "--name", "value", "-o", "answer.json"]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let saved: Value =
        serde_json::from_slice(&std::fs::read(service.fixture.path("answer.json")).unwrap())
            .unwrap();
    assert_eq!(saved, service.query("value"));
}

#[test]
fn protocol_request_limit_accepts_under_and_at_limit_and_rejects_over_it() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    let service = Service::new(&[("a.ts", "export const value = 1;")]);
    let expected = service.query("value");
    let socket = std::fs::read_dir(&service.runtime)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "sock"))
        .unwrap();
    for size in [(1 << 20) - 1, 1 << 20, (1 << 20) + 1] {
        let mut request = serde_json::to_vec(&serde_json::json!({
            "identity": {"config": service.fixture.path("tsconfig.json"), "build": env!("TSPARSE_SERVICE_BUILD")},
            "action": {"Query": {"References": {"Name": {"name": "value", "file": null}}}}
        })).unwrap();
        request.resize(size - 1, b' ');
        request.push(b'\n');
        let mut stream = UnixStream::connect(&socket).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        let write = stream.write_all(&request);
        let mut response = String::new();
        let read = stream.read_to_string(&mut response);
        if size <= 1 << 20 {
            assert!(write.is_ok(), "size {size}: {write:?}");
            read.unwrap();
            let response: Value = serde_json::from_str(&response).unwrap();
            assert_eq!(
                response["Query"]["Ok"]["References"], expected,
                "size {size}: full response {response}"
            );
        } else {
            // A peer may reset on close with an unread suffix. Either outcome is
            // a failed request, and must not kill or rebuild the project service.
            assert!(
                write.is_err() || read.is_err() || response.contains("oversized"),
                "{response}"
            );
        }
        assert_eq!(service.query("value"), expected);
        assert_eq!(service.status()["generation"], 1);
    }
}

#[test]
fn a_service_never_holds_a_descriptor_its_launcher_inherited() {
    // A launcher can inherit a descriptor its parent never meant to pass:
    // on macOS, std creates a pipe and marks it close-on-exec in two steps,
    // so a concurrent spawn can inherit it. The service that launcher
    // starts outlives it and must not hold that descriptor, or whoever
    // waits for the pipe's end of file, such as `Command::output` waiting
    // for another client, waits until the service exits.
    use std::io::Read;
    use std::os::fd::FromRawFd;
    use std::process::Stdio;
    unsafe extern "C" {
        fn pipe(descriptors: *mut std::ffi::c_int) -> std::ffi::c_int;
    }
    let service = Service::new(&[("a.ts", "export const value = 1;")]);
    let mut descriptors = [0; 2];
    // SAFETY: `pipe` writes two new descriptors, neither close-on-exec.
    assert_eq!(unsafe { pipe(descriptors.as_mut_ptr()) }, 0);
    // SAFETY: each descriptor is new and owned once.
    let (mut reader, writer) = unsafe {
        (
            std::fs::File::from_raw_fd(descriptors[0]),
            std::fs::File::from_raw_fd(descriptors[1]),
        )
    };
    let client = service
        .command()
        .args([
            "--references",
            "--project",
            "tsconfig.json",
            "--name",
            "value",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(writer);
    let output = client.wait_with_output().unwrap();
    assert_eq!(json(output)["groups"].as_array().unwrap().len(), 1);
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = send.send(reader.read_to_end(&mut bytes).map(|_| bytes));
    });
    let read = receive
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("the service still holds the pipe its launcher inherited");
    assert!(read.unwrap().is_empty());
    assert_eq!(service.status()["running"], true);
}
