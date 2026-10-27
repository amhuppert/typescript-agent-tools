// validation: run alone: its services must become ready within ten seconds, and it reads a service's idle time in seconds, which other test executables' load can break.
//! `--doctor` reports, one line per check, what project queries depend on:
//! the build, the environment, the runtime directory, and with `--project`
//! the configuration, the compiler it is read for, the project's service,
//! and the limits that service inherited. It loads nothing and starts nothing, exits 1 when a check
//! finds something, and each finding's printed `next` clears it.
//! `--references-status` reports a running service's identity, idle time,
//! and limits in text and JSON; `--version` names the build a service
//! reports; and every transport failure ends by naming the doctor.
#![cfg(unix)]
mod support;

use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use support::service::{IDLE_SECS, Service, json as document};

/// The build stamp `build.rs` compiles into this package's targets.
const BUILD: &str = env!("TSPARSE_SERVICE_BUILD");

/// What the doctor appends to every transport failure of a request for
/// `tsconfig.json`.
const DIAGNOSE: &str = "; run: tsparse --doctor --project tsconfig.json";

fn service() -> Service {
    Service::new(&[("value.ts", "export const value = 1;\n")])
}

/// The longest path a socket address holds: one byte less than
/// `sun_path`, which ends with a NUL, 104 bytes on macOS and 108 on Linux.
fn socket_limit() -> usize {
    if cfg!(target_os = "macos") { 103 } else { 107 }
}

/// What a socket's name adds to its directory: `/`, 16 hexadecimal digits
/// of its key, and `.sock`.
const SOCKET_NAME: usize = 22;

/// `tsparse --doctor` and `args` from the project's directory, with this
/// test's runtime directory and idle time, the limits unset, and `env`.
fn doctor(service: &Service, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = service.command();
    command
        .env_remove("TSPARSE_MAX_MEMORY_MB")
        .env_remove("TSPARSE_QUERY_TIMEOUT_MS")
        .envs(env.iter().copied())
        .arg("--doctor")
        .args(args);
    command.output().unwrap()
}

/// `doctor` with `--project tsconfig.json`.
fn project_doctor(service: &Service, args: &[&str], env: &[(&str, &str)]) -> Output {
    doctor(
        service,
        &[&["--project", "tsconfig.json"], args].concat(),
        env,
    )
}

/// A query through the service, with the limits `env` sets, which starts it.
fn start(service: &Service, env: &[(&str, &str)]) {
    let output = service
        .command()
        .env_remove("TSPARSE_MAX_MEMORY_MB")
        .env_remove("TSPARSE_QUERY_TIMEOUT_MS")
        .envs(env.iter().copied())
        .args([
            "--references",
            "--name",
            "value",
            "--project",
            "tsconfig.json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The report of a doctor that exited with `code`, which wrote nothing to
/// stderr.
fn report(output: &Output, code: i32) -> String {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    text(&output.stdout)
}

fn build_line() -> String {
    format!(
        "build: tsparse {} (build {BUILD}) on {} {}: project service yes, --apply yes, memory monitoring yes",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// The environment line when every variable is valid: the limits as
/// `memory` and `timeout` set them, or their defaults.
fn environment_line(service: &Service, memory: Option<&str>, timeout: Option<&str>) -> String {
    let limit = |value: Option<&str>, default: &str| match value {
        Some(value) => value.to_string(),
        None => format!("{default} (default)"),
    };
    format!(
        "environment: TSPARSE_MAX_MEMORY_MB={}, TSPARSE_QUERY_TIMEOUT_MS={}, TSPARSE_SERVICE_IDLE_SECS={IDLE_SECS}, TSPARSE_SERVICE_DIR={}",
        limit(memory, "6144"),
        limit(timeout, "60000"),
        service.runtime.display()
    )
}

fn budget(directory: &Path) -> String {
    format!(
        "its socket paths take {} of at most {} bytes",
        directory.as_os_str().len() + SOCKET_NAME,
        socket_limit()
    )
}

fn private_line(directory: &Path) -> String {
    format!(
        "runtime directory: {} (TSPARSE_SERVICE_DIR) is a private directory owned by this user with mode 0700; {}",
        directory.display(),
        budget(directory)
    )
}

fn absent_line(directory: &Path) -> String {
    format!(
        "runtime directory: {} (TSPARSE_SERVICE_DIR) does not exist yet; a project query creates it with mode 0700; {}",
        directory.display(),
        budget(directory)
    )
}

fn configuration_line(service: &Service) -> String {
    format!(
        "configuration: tsconfig.json is {}, a readable file; another spelling of the same file would own a separate service",
        service.fixture.path("tsconfig.json").display()
    )
}

/// The compatibility line of a project with no TypeScript package
/// (`tests/doctor_compatibility.rs` covers the others).
const COMPATIBILITY: &str = "compatibility: no TypeScript package is installed in or above the configuration's directory; read with the TypeScript 7 dialect; semantics: TypeScript 7.0.2; standard library: the vendored TypeScript 7.0.2 declarations";

const NO_SERVICE: &str =
    "service: no service is running for tsconfig.json; the next project query starts one";

fn no_service_limits(memory: &str, timeout: &str) -> String {
    format!(
        "service limits: no service is running; one started from this environment takes TSPARSE_MAX_MEMORY_MB {memory} and TSPARSE_QUERY_TIMEOUT_MS {timeout}"
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

/// `line` as a POSIX shell runs it in the project's directory, with the
/// service's runtime directory and this build first on `PATH`.
fn shell(service: &Service, line: &str, env: &[(&str, &str)]) -> Output {
    let mut command = Command::new("sh");
    command
        .args(["-c", line])
        .current_dir(&service.fixture.root)
        .env("TSPARSE_SERVICE_DIR", &service.runtime)
        .env("TSPARSE_SERVICE_IDLE_SECS", IDLE_SECS)
        .env_remove("TSPARSE_MAX_MEMORY_MB")
        .env_remove("TSPARSE_QUERY_TIMEOUT_MS")
        .envs(env.iter().copied())
        .env("PATH", path_with_build())
        .stdin(Stdio::null());
    command.output().unwrap()
}

/// The command a report prints on its one `  next: ` line.
fn printed_next(report: &str) -> &str {
    let next: Vec<&str> = report
        .lines()
        .filter_map(|line| line.strip_prefix("  next: "))
        .collect();
    assert_eq!(next.len(), 1, "{report}");
    next[0]
}

/// Seconds in the form status prints them: `1h`, `59m58s`, `0s`.
fn seconds(text: &str) -> u64 {
    if text == "0s" {
        return 0;
    }
    let mut total = 0;
    let mut number = String::new();
    for c in text.chars() {
        match c {
            '0'..='9' => number.push(c),
            'h' | 'm' | 's' => {
                let unit = match c {
                    'h' => 3600,
                    'm' => 60,
                    _ => 1,
                };
                total += number.parse::<u64>().unwrap() * unit;
                number.clear();
            }
            _ => panic!("{text}"),
        }
    }
    assert!(number.is_empty(), "{text}");
    total
}

/// The running service's status document.
fn status(service: &Service) -> Value {
    service.status()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[test]
fn without_a_project_the_doctor_checks_the_build_environment_and_runtime_directory() {
    let service = service();
    // A directory that does not exist yet passes, and the doctor does not
    // create it.
    let absent = report(&doctor(&service, &[], &[]), 0);
    assert!(!service.runtime.exists());
    assert_eq!(
        absent,
        format!(
            "{}\n{}\n{}\nok: 3 checks pass\n",
            build_line(),
            environment_line(&service, None, None),
            absent_line(&service.runtime)
        )
    );
    std::fs::create_dir(&service.runtime).unwrap();
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let private = report(
        &doctor(
            &service,
            &[],
            &[
                ("TSPARSE_MAX_MEMORY_MB", "8192"),
                ("TSPARSE_QUERY_TIMEOUT_MS", "+120000"),
            ],
        ),
        0,
    );
    assert_eq!(
        private,
        format!(
            "{}\n{}\n{}\nok: 3 checks pass\n",
            build_line(),
            environment_line(&service, Some("8192"), Some("120000")),
            private_line(&service.runtime)
        )
    );
    // One document with an entry per check, which say what the lines say.
    let output = doctor(&service, &["--json"], &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        document(output),
        json!({
            "operation": "doctor",
            "outcome": "pass",
            "checks": [
                {"name": "build", "status": "pass", "detail": build_line().strip_prefix("build: ").unwrap()},
                {"name": "environment", "status": "pass", "detail": environment_line(&service, None, None).strip_prefix("environment: ").unwrap()},
                {"name": "runtime directory", "status": "pass", "detail": private_line(&service.runtime).strip_prefix("runtime directory: ").unwrap()},
            ],
        })
    );
    // `--pretty` indents the same document.
    let compact = text(&doctor(&service, &["--json"], &[]).stdout);
    let pretty = text(&doctor(&service, &["--json", "--pretty"], &[]).stdout);
    assert!(pretty.starts_with("{\n  \""), "{pretty}");
    assert_eq!(
        serde_json::from_str::<Value>(&pretty).unwrap(),
        serde_json::from_str::<Value>(&compact).unwrap()
    );
    // Nothing was created in the directory.
    assert_eq!(std::fs::read_dir(&service.runtime).unwrap().count(), 0);
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

#[test]
fn each_invalid_variable_is_an_environment_finding() {
    let service = service();
    let valid = |skip: &[&str]| {
        let mut valid = Vec::new();
        for (name, value) in [
            ("TSPARSE_MAX_MEMORY_MB", "6144 (default)".to_string()),
            ("TSPARSE_QUERY_TIMEOUT_MS", "60000 (default)".to_string()),
            ("TSPARSE_SERVICE_IDLE_SECS", IDLE_SECS.to_string()),
            ("TSPARSE_SERVICE_DIR", service.runtime.display().to_string()),
        ] {
            if !skip.contains(&name) {
                valid.push(format!("{name}={value}"));
            }
        }
        valid.join(", ")
    };
    for (variable, form) in VARIABLES {
        let output = doctor(&service, &[], &[(variable, "0")]);
        let message = format!("{variable} must be {form}, got 0");
        assert_eq!(
            report(&output, 1),
            format!(
                "{}\nenvironment: {message}; {}\n{}\nfindings: environment\n",
                build_line(),
                valid(&[variable]),
                absent_line(&service.runtime)
            ),
            "{variable}"
        );
        let output = doctor(&service, &["--json"], &[(variable, "0")]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let found: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(found["outcome"], "findings");
        assert_eq!(
            found["checks"][1],
            json!({"name": "environment", "status": "finding", "detail": format!("{message}; {}", valid(&[variable]))})
        );
    }
    // Every invalid variable at once, in the owner's order.
    let all: Vec<(&str, &str)> = VARIABLES.iter().map(|&(name, _)| (name, "x")).collect();
    let messages: Vec<String> = VARIABLES
        .iter()
        .map(|(name, form)| format!("{name} must be {form}, got x"))
        .collect();
    assert_eq!(
        report(&doctor(&service, &[], &all), 1).lines().nth(1),
        Some(
            format!(
                "environment: {}; TSPARSE_SERVICE_DIR={}",
                messages.join("; "),
                service.runtime.display()
            )
            .as_str()
        )
    );
}

#[test]
fn a_runtime_directory_that_is_not_private_is_a_finding_whose_next_fixes_it() {
    let service = service();
    std::fs::create_dir(&service.runtime).unwrap();
    let directory = service.runtime.display().to_string();
    for (mode, why) in [
        (
            0o755,
            "any process that can open the socket can run queries as you",
        ),
        (
            0o500,
            "a service could not create its socket or record why it failed",
        ),
    ] {
        std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(mode)).unwrap();
        let found = report(&doctor(&service, &[], &[]), 1);
        assert_eq!(
            found,
            format!(
                "{}\n{}\nruntime directory: {directory} (TSPARSE_SERVICE_DIR): Project service directory must be a private directory owned by this user with mode 0700: {directory}\n  why: {why}\n  next: chmod 700 {directory}\nfindings: runtime directory\n",
                build_line(),
                environment_line(&service, None, None),
            ),
            "{mode:o}"
        );
        // Following the printed command clears the finding.
        let fixed = shell(&service, printed_next(&found), &[]);
        assert!(fixed.status.success(), "{fixed:?}");
        assert_eq!(
            report(&doctor(&service, &[], &[]), 0).lines().nth(2),
            Some(private_line(&service.runtime).as_str())
        );
    }
    // Another kind of file at its path: another directory is the remedy.
    std::fs::remove_dir(&service.runtime).unwrap();
    std::fs::write(&service.runtime, "").unwrap();
    let found = report(&doctor(&service, &["--json"], &[]), 1);
    std::fs::remove_file(&service.runtime).unwrap();
    let found: Value = serde_json::from_str(&found).unwrap();
    assert_eq!(
        found["checks"][2],
        json!({
            "name": "runtime directory",
            "status": "finding",
            "detail": format!("{directory} (TSPARSE_SERVICE_DIR): Project service directory must be a private directory owned by this user with mode 0700: {directory}"),
            "remedy": "set TSPARSE_SERVICE_DIR to a new path or to a private directory this user owns",
        })
    );
}

#[test]
fn the_socket_path_budget_is_checked_under_at_and_over_the_platform_limit() {
    let service = service();
    let prefix = format!("/tmp/tsparse-doctor-{}-", std::process::id());
    let directory = |socket: usize| {
        let length = socket - SOCKET_NAME;
        PathBuf::from(format!("{prefix}{}", "d".repeat(length - prefix.len())))
    };
    let limit = socket_limit();
    for socket in [limit - 1, limit] {
        let directory = directory(socket);
        let output = doctor(
            &service,
            &[],
            &[("TSPARSE_SERVICE_DIR", directory.to_str().unwrap())],
        );
        assert_eq!(
            report(&output, 0).lines().nth(2),
            Some(absent_line(&directory).as_str()),
            "{socket}"
        );
    }
    let over = directory(limit + 1);
    let output = doctor(
        &service,
        &["--json"],
        &[("TSPARSE_SERVICE_DIR", over.to_str().unwrap())],
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let found: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        found["checks"][2],
        json!({
            "name": "runtime directory",
            "status": "finding",
            "detail": format!("{} (TSPARSE_SERVICE_DIR): socket paths in {} take {} bytes, more than the {limit} a socket address holds", over.display(), over.display(), limit + 1),
            "remedy": "set a shorter TSPARSE_SERVICE_DIR",
        })
    );
    // The platform agrees: a service listens at the limit, and a request
    // over it fails on its socket.
    let at = directory(limit);
    let served = |directory: &Path| {
        service
            .command()
            .env("TSPARSE_SERVICE_DIR", directory)
            .args([
                "--references",
                "--name",
                "value",
                "--project",
                "tsconfig.json",
            ])
            .output()
            .unwrap()
    };
    let answered = served(&at);
    let stopped = service
        .command()
        .env("TSPARSE_SERVICE_DIR", &at)
        .args(["--references-stop", "--project", "tsconfig.json"])
        .output()
        .unwrap();
    std::fs::remove_dir_all(&at).unwrap();
    assert!(answered.status.success(), "{answered:?}");
    assert!(stopped.status.success(), "{stopped:?}");
    let refused = served(&over);
    // The client creates the directory before it finds the socket too long.
    std::fs::remove_dir(&over).unwrap();
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
    assert!(
        text(&refused.stderr).starts_with("transport.socket: "),
        "{refused:?}"
    );
}

#[test]
fn an_unreadable_configuration_is_a_finding() {
    let service = service();
    let config = |name: &str| {
        let path = service.fixture.path(name);
        (
            path.display().to_string(),
            project_doctor_named(&service, name),
        )
    };
    let missing = service.fixture.path("missing.json");
    let cause = std::fs::File::open(&missing).unwrap_err();
    let (absolute, output) = config("missing.json");
    let found = report(&output, 1);
    assert_eq!(
        found.lines().nth(3),
        Some(format!("configuration: missing.json ({absolute}) cannot be read: {cause}").as_str()),
        "{found}"
    );
    assert_eq!(found.lines().last(), Some("findings: configuration"));
    std::fs::create_dir(service.fixture.path("directory.json")).unwrap();
    let (absolute, output) = config("directory.json");
    assert_eq!(
        report(&output, 1).lines().nth(3),
        Some(
            format!(
                "configuration: directory.json ({absolute}) cannot be read: not a regular file"
            )
            .as_str()
        )
    );
    let closed = service.fixture.path("closed.json");
    std::fs::write(&closed, "{}").unwrap();
    std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o000)).unwrap();
    match std::fs::File::open(&closed) {
        Ok(_) => eprintln!(
            "SKIP an_unreadable_configuration_is_a_finding (mode 000): this user reads any file"
        ),
        Err(cause) => {
            let (absolute, output) = config("closed.json");
            assert_eq!(
                report(&output, 1).lines().nth(3),
                Some(
                    format!("configuration: closed.json ({absolute}) cannot be read: {cause}")
                        .as_str()
                )
            );
        }
    }
}

/// `--doctor --project NAME` with the limits unset.
fn project_doctor_named(service: &Service, name: &str) -> Output {
    doctor(service, &["--project", name], &[])
}

#[test]
fn a_service_started_with_other_limits_is_a_finding_until_it_is_stopped() {
    let service = service();
    std::fs::create_dir(&service.runtime).unwrap();
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    // Neither check finds anything before a service runs.
    let idle = report(&project_doctor(&service, &[], &[]), 0);
    assert_eq!(
        idle,
        format!(
            "{}\n{}\n{}\n{}\n{COMPATIBILITY}\n{NO_SERVICE}\n{}\nok: 7 checks pass\n",
            build_line(),
            environment_line(&service, None, None),
            private_line(&service.runtime),
            configuration_line(&service),
            no_service_limits("6144", "60000"),
        )
    );
    let raised = [("TSPARSE_QUERY_TIMEOUT_MS", "120000")];
    assert_eq!(
        report(&project_doctor(&service, &[], &raised), 0)
            .lines()
            .nth(6),
        Some(no_service_limits("6144", "120000").as_str())
    );
    for (inherited, current, limits) in [
        (
            &[("TSPARSE_QUERY_TIMEOUT_MS", "60000")][..],
            &raised[..],
            "TSPARSE_QUERY_TIMEOUT_MS 60000 inherited at start, 120000 in this environment",
        ),
        (
            &[("TSPARSE_MAX_MEMORY_MB", "4096")][..],
            &[("TSPARSE_MAX_MEMORY_MB", "8192")][..],
            "TSPARSE_MAX_MEMORY_MB 4096 inherited at start, 8192 in this environment",
        ),
        (
            &[][..],
            &[
                ("TSPARSE_MAX_MEMORY_MB", "8192"),
                ("TSPARSE_QUERY_TIMEOUT_MS", "120000"),
            ][..],
            "TSPARSE_MAX_MEMORY_MB 6144 inherited at start, 8192 in this environment; TSPARSE_QUERY_TIMEOUT_MS 60000 inherited at start, 120000 in this environment",
        ),
    ] {
        start(&service, inherited);
        let found = report(&project_doctor(&service, &[], current), 1);
        let lines: Vec<&str> = found.lines().collect();
        assert!(lines[5].starts_with("service: running: pid "), "{found}");
        assert_eq!(
            &lines[6..],
            [
                format!("service limits: {limits}").as_str(),
                "  why: a service keeps the environment it started with",
                "  next: tsparse --references-stop --project tsconfig.json",
                "findings: service limits",
            ],
            "{found}"
        );
        // The document says the same, and its `next` is the command.
        let output = project_doctor(&service, &["--json"], current);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let found_json: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            found_json["checks"][6],
            json!({
                "name": "service limits",
                "status": "finding",
                "detail": limits,
                "why": "a service keeps the environment it started with",
                "next": {"argv": ["tsparse", "--references-stop", "--project", "tsconfig.json"]},
            })
        );
        // Following the printed command, then running the doctor again,
        // clears the finding.
        let stopped = shell(&service, printed_next(&found), current);
        assert!(stopped.status.success(), "{stopped:?}");
        let cleared = report(&project_doctor(&service, &[], current), 0);
        assert_eq!(cleared.lines().nth(5), Some(NO_SERVICE), "{cleared}");
        assert_eq!(cleared.lines().last(), Some("ok: 7 checks pass"));
    }
}

#[test]
fn a_service_with_this_environments_limits_passes() {
    let service = service();
    let before = now();
    start(&service, &[]);
    let after = now();
    let output = project_doctor(&service, &[], &[]);
    let status = status(&service);
    let passed = report(&output, 0);
    let lines: Vec<&str> = passed.lines().collect();
    assert_eq!(lines.len(), 8, "{passed}");
    assert_eq!(lines[3], configuration_line(&service));
    assert_eq!(lines[4], COMPATIBILITY);
    let pid = status["pid"].as_u64().unwrap();
    let running = lines[5]
        .strip_prefix(&format!(
            "service: running: pid {pid}, generation 1, build {BUILD}, idle 1h, "
        ))
        .unwrap_or_else(|| panic!("{passed}"));
    let (remaining, started) = running.split_once(" remaining; started ").unwrap();
    let remaining = seconds(remaining);
    assert!(
        remaining <= 3600 && remaining >= status["idle_remaining_secs"].as_u64().unwrap(),
        "{passed}"
    );
    let ago = seconds(started.strip_suffix(" ago").unwrap()) as i64;
    assert!(ago <= now() - before, "{passed}");
    let started_unix = status["started_unix"].as_i64().unwrap();
    assert!(before <= started_unix && started_unix <= after, "{status}");
    assert_eq!(
        lines[6],
        "service limits: TSPARSE_MAX_MEMORY_MB 6144 and TSPARSE_QUERY_TIMEOUT_MS 60000, as in this environment"
    );
    assert_eq!(lines[7], "ok: 7 checks pass");
    // The document's entries say the same.
    let output = project_doctor(&service, &["--json"], &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let passed = document(output);
    assert_eq!(passed["outcome"], "pass");
    assert_eq!(passed["project"], "tsconfig.json");
    assert_eq!(passed["checks"][4]["name"], "compatibility");
    assert_eq!(passed["checks"][5]["name"], "service");
    assert_eq!(passed["checks"][5]["status"], "pass");
    assert!(
        passed["checks"][5]["detail"]
            .as_str()
            .unwrap()
            .starts_with(&format!(
                "running: pid {pid}, generation 1, build {BUILD}, idle 1h, "
            )),
        "{passed}"
    );
    assert_eq!(
        passed["checks"][6],
        json!({
            "name": "service limits",
            "status": "pass",
            "detail": "TSPARSE_MAX_MEMORY_MB 6144 and TSPARSE_QUERY_TIMEOUT_MS 60000, as in this environment",
        })
    );
}

#[test]
fn the_doctor_never_loads_a_project_or_starts_a_service() {
    // A configuration that is not JSON and a source that does not parse:
    // loading either would fail.
    let service = Service::new(&[("value.ts", "export const = ;\n")]);
    std::fs::write(service.fixture.path("tsconfig.json"), "{ not json").unwrap();
    for args in [&[][..], &["--json"]] {
        let output = project_doctor(&service, args, &[]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        // No service started: it would have created the runtime directory.
        assert!(!service.runtime.exists(), "{args:?}");
    }
    std::fs::create_dir(&service.runtime).unwrap();
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = project_doctor(&service, &[], &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    // Nor its lock files.
    assert_eq!(std::fs::read_dir(&service.runtime).unwrap().count(), 0);
}

#[test]
fn status_reports_a_running_service_in_text_and_json() {
    let service = service();
    let none = service.run(&["--references-status"]);
    assert_eq!(report(&none, 0), "no service: tsconfig.json\n");
    let output = service.run(&["--references-status", "--json"]);
    assert_eq!(
        document(output),
        json!({
            "running": false,
            "pid": null,
            "generation": 0,
            "build": null,
            "limits": null,
            "idle_secs": null,
            "idle_remaining_secs": null,
            "started_unix": null,
        })
    );
    let before = now();
    start(&service, &[("TSPARSE_QUERY_TIMEOUT_MS", "90000")]);
    let after = now();
    let first = status(&service);
    let pid = first["pid"].as_u64().unwrap();
    let started = first["started_unix"].as_i64().unwrap();
    assert!(before <= started && started <= after, "{first}");
    let remaining = first["idle_remaining_secs"].as_u64().unwrap();
    assert!(remaining <= 3600, "{first}");
    let mut stable = first.clone();
    stable["idle_remaining_secs"] = Value::Null;
    assert_eq!(
        stable,
        json!({
            "running": true,
            "pid": pid,
            "generation": 1,
            "build": BUILD,
            "limits": {"max_memory_mb": 6144, "query_timeout_ms": 90000},
            "idle_secs": 3600,
            "idle_remaining_secs": null,
            "started_unix": started,
        })
    );
    // Text: the same fields, the way status prints them.
    let printed = report(&service.run(&["--references-status"]), 0);
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(lines.len(), 2, "{printed}");
    let left = lines[0]
        .strip_prefix(&format!(
            "running: pid {pid}, generation 1, build {BUILD}, idle 1h, "
        ))
        .unwrap_or_else(|| panic!("{printed}"));
    assert!(
        seconds(left.strip_suffix(" remaining").unwrap()) <= remaining,
        "{printed}"
    );
    assert_eq!(lines[1], "limits: 6144 MiB, 90000 ms");
    // Status is administrative: it does not extend the service's idle time.
    std::thread::sleep(std::time::Duration::from_millis(2100));
    let later = status(&service);
    assert_eq!(later["pid"], first["pid"]);
    assert!(
        later["idle_remaining_secs"].as_u64().unwrap() < remaining,
        "{first} {later}"
    );
    // `--pretty` alone keeps the indented document.
    let pretty = text(&service.run(&["--references-status", "--pretty"]).stdout);
    assert!(pretty.starts_with("{\n  \"running\": true,"), "{pretty}");
    // `-o` receives the same forms, and stdout nothing.
    for (args, file) in [
        (
            &["--references-status", "-o", "status.txt"][..],
            "status.txt",
        ),
        (
            &["--references-status", "--json", "-o", "status.json"],
            "status.json",
        ),
    ] {
        let output = service.run(args);
        assert_eq!(report(&output, 0), "", "{args:?}");
        let written = std::fs::read_to_string(service.fixture.path(file)).unwrap();
        if file.ends_with(".json") {
            let written: Value = serde_json::from_str(&written).unwrap();
            assert_eq!(written["pid"], first["pid"]);
            assert_eq!(written["build"], BUILD);
        } else {
            assert!(
                written.starts_with(&format!(
                    "running: pid {pid}, generation 1, build {BUILD}, "
                )),
                "{written}"
            );
            assert!(
                written.ends_with("\nlimits: 6144 MiB, 90000 ms\n"),
                "{written}"
            );
        }
    }
}

/// Status is administrative (design, "Preserved behavior"): it reports the
/// generation the service last published and loads nothing, so neither an
/// edit nor a configuration that no longer loads changes its answer until
/// a query refreshes the project.
#[test]
fn status_neither_loads_the_project_nor_checks_its_freshness() {
    let service = service();
    start(&service, &[]);
    let before = service.stable_status();
    assert_eq!(before["generation"], 1, "{before}");
    let printed = report(&service.run(&["--references-status"]), 0);
    std::fs::write(
        service.fixture.path("value.ts"),
        "export const changed = 2;\n",
    )
    .unwrap();
    std::fs::write(service.fixture.path("tsconfig.json"), "{ not json").unwrap();
    assert_eq!(service.stable_status(), before);
    let unchanged = report(&service.run(&["--references-status"]), 0);
    assert_eq!(
        unchanged.lines().nth(1),
        printed.lines().nth(1),
        "{unchanged}"
    );
    assert!(
        unchanged.starts_with(&format!(
            "running: pid {}, generation 1, build {BUILD}, ",
            before["pid"]
        )),
        "{unchanged}"
    );
    // A query does load, and publishes the edit as the next generation.
    std::fs::write(service.fixture.path("tsconfig.json"), support::CONFIG).unwrap();
    assert_eq!(
        service.query("changed")["groups"].as_array().unwrap().len(),
        1
    );
    assert_eq!(service.generation(), 2);
}

#[test]
fn version_names_the_build_a_running_service_reports() {
    let service = service();
    start(&service, &[]);
    for flag in ["--version", "-V"] {
        let output = service.command().arg(flag).output().unwrap();
        assert_eq!(
            report(&output, 0),
            format!("tsparse {} (build {BUILD})\n", env!("CARGO_PKG_VERSION"))
        );
        let printed = text(&output.stdout);
        let stamp = printed
            .trim_end()
            .strip_suffix(')')
            .and_then(|rest| rest.split_once("(build "))
            .map(|(_, stamp)| stamp)
            .unwrap();
        assert_eq!(status(&service)["build"], stamp);
    }
}

#[test]
fn every_transport_failure_names_the_doctor_which_reports_its_cause() {
    let service = service();
    std::fs::create_dir(&service.runtime).unwrap();
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o755)).unwrap();
    let message = format!(
        "Project service directory must be a private directory owned by this user with mode 0700: {}{DIAGNOSE}",
        service.runtime.display()
    );
    for args in [
        &["--references", "--name", "value"][..],
        &["--references-status"],
        &["--references-stop"],
    ] {
        let output = service.run(args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let stderr = text(&output.stderr);
        assert_eq!(
            stderr.lines().next(),
            Some(format!("transport.directory: {message}").as_str()),
            "{args:?}"
        );
    }
    for args in [
        &["--references", "--name", "value", "--json"][..],
        &["--references-status", "--json"],
    ] {
        let output = service.run(args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let failed: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(failed["error"]["code"], "transport.directory", "{args:?}");
        assert_eq!(failed["error"]["message"], message.as_str(), "{args:?}");
    }
    // Following it: the doctor reports the directory, and its own `next`.
    let output = service.run(&["--references", "--name", "value"]);
    let stderr = text(&output.stderr);
    let (_, run) = stderr
        .lines()
        .next()
        .unwrap()
        .split_once("; run: ")
        .unwrap();
    let diagnosed = shell(&service, run, &[]);
    let found = report(&diagnosed, 1);
    assert!(
        found.contains(&format!(
            "\nruntime directory: {0} (TSPARSE_SERVICE_DIR): Project service directory must be a private directory owned by this user with mode 0700: {0}\n",
            service.runtime.display()
        )),
        "{found}"
    );
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
}

/// The key of the project's endpoint, from the lock file an earlier start
/// left behind: lock files are never removed.
fn key(service: &Service) -> String {
    let keys: Vec<String> = std::fs::read_dir(&service.runtime)
        .unwrap()
        .filter_map(|entry| {
            let name = entry.unwrap().file_name().into_string().unwrap();
            let key = name.strip_suffix(".lock")?;
            (key.len() == 16 && key.bytes().all(|b| b.is_ascii_hexdigit())).then(|| key.to_string())
        })
        .collect();
    assert_eq!(keys.len(), 1, "{keys:?}");
    keys[0].clone()
}

/// What an earlier start left, which the next start removes, the doctor
/// reports and leaves in place: the failure a foreground service recorded,
/// and a partial copy of a diagnostic. One it cannot read is a finding.
#[test]
fn the_doctor_reports_what_an_earlier_start_left_and_removes_nothing() {
    let service = service();
    start(&service, &[]);
    assert!(service.run(&["--references-stop"]).status.success());
    let key = key(&service);
    let lock = service.runtime.join(format!("{key}.lock"));
    let recorded = service.runtime.join(format!("{key}.start-error"));
    let staged = service.runtime.join(format!("{key}.start-error.staged"));
    // A foreground service that cannot open its lock records why.
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let cause = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock)
        .unwrap_err();
    let serve = service.run(&["--references-serve"]);
    assert_eq!(serve.status.code(), Some(2), "{serve:?}");
    let failed = format!(
        "{NO_SERVICE}; the last start that failed recorded transport.lock: Project service lock {}: {cause}",
        lock.display()
    );
    let line = |output: &Output| report(output, 0).lines().nth(5).unwrap().to_string();
    assert_eq!(line(&project_doctor(&service, &[], &[])), failed);
    std::fs::write(&staged, "transport.lo").unwrap();
    assert_eq!(
        line(&project_doctor(&service, &[], &[])),
        format!(
            "{failed}; a partial startup diagnostic remains at {}, which the next start removes",
            staged.display()
        )
    );
    assert!(recorded.is_file() && staged.is_file());
    // A diagnostic that is not a regular file cannot be read.
    std::fs::remove_file(&recorded).unwrap();
    std::fs::create_dir(&recorded).unwrap();
    let found = report(&project_doctor(&service, &[], &[]), 1);
    assert_eq!(
        found.lines().nth(5),
        Some(
            format!(
                "service: the startup diagnostic {} cannot be read: not a regular file",
                recorded.display()
            )
            .as_str()
        ),
        "{found}"
    );
    assert_eq!(found.lines().last(), Some("findings: service"));
    std::fs::remove_dir(&recorded).unwrap();
    std::fs::remove_dir(&lock).unwrap();
    // The next start removes the copy.
    start(&service, &[]);
    assert!(!staged.exists());
}

/// The doctor's command names the configuration as the request spelled it,
/// quoted for a shell where it must be, so it runs as printed.
#[test]
fn the_doctor_command_a_transport_failure_prints_runs_as_printed() {
    let service = service();
    std::fs::copy(
        service.fixture.path("tsconfig.json"),
        service.fixture.path("my config.json"),
    )
    .unwrap();
    std::fs::create_dir(&service.runtime).unwrap();
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o750)).unwrap();
    let output = service
        .command()
        .args(["--outline", "value.ts", "--project", "my config.json"])
        .output()
        .unwrap();
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = text(&output.stderr);
    let first = stderr.lines().next().unwrap();
    let (_, run) = first.split_once("; run: ").unwrap();
    assert_eq!(run, "tsparse --doctor --project 'my config.json'");
    let checked = report(&shell(&service, run, &[]), 0);
    assert_eq!(
        checked.lines().nth(3),
        Some(
            format!(
                "configuration: my config.json is {}, a readable file; another spelling of the same file would own a separate service",
                service.fixture.path("my config.json").display()
            )
            .as_str()
        )
    );
}

#[test]
fn the_doctor_takes_no_file() {
    let service = service();
    let output = doctor(&service, &["value.ts"], &[]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert_eq!(
        text(&output.stderr),
        "usage.operands: --doctor takes no FILE; name a project with --project TSCONFIG\n"
    );
}

/// The remedy for a directory `TSPARSE_SERVICE_DIR` names that cannot be
/// created.
const UNCREATABLE: &str = "set TSPARSE_SERVICE_DIR to a path in a directory this user can write, or unset it and set TMPDIR to such a directory";

/// A runtime directory that does not exist passes only where a project
/// query could create it, since a query creates only the directory itself:
/// its parent must be a directory this user can write and search.
/// Otherwise it is the finding a query would report, in text and JSON,
/// with and without a project, and the doctor still creates nothing.
#[test]
fn a_runtime_directory_that_cannot_be_created_is_a_finding() {
    let service = service();
    std::fs::create_dir(&service.runtime).unwrap();
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let file = service.runtime.join("file");
    std::fs::write(&file, "").unwrap();
    let closed = service.runtime.join("closed");
    std::fs::create_dir(&closed).unwrap();
    std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o500)).unwrap();
    let mut cases = vec![
        (
            "a missing parent",
            service.runtime.join("missing").join("runtime"),
        ),
        ("a parent that is a file", file.join("runtime")),
    ];
    let denied = closed.join("runtime");
    match std::fs::create_dir(&denied) {
        Err(_) => cases.push(("a parent without write permission", denied)),
        Ok(()) => {
            std::fs::remove_dir(&denied).unwrap();
            eprintln!(
                "SKIP a_runtime_directory_that_cannot_be_created_is_a_finding (a parent without write permission): permissions are not enforced"
            );
        }
    }
    for (context, directory) in cases {
        let cause = std::fs::create_dir(&directory).unwrap_err();
        let message = format!(
            "Project service directory {} cannot be created: {cause}",
            directory.display()
        );
        let env = [("TSPARSE_SERVICE_DIR", directory.to_str().unwrap())];
        for args in [&[][..], &["--project", "tsconfig.json"]] {
            let found = report(&doctor(&service, args, &env), 1);
            let lines: Vec<&str> = found.lines().collect();
            assert_eq!(
                lines[2..4],
                [
                    format!(
                        "runtime directory: {} (TSPARSE_SERVICE_DIR): {message}",
                        directory.display()
                    ),
                    format!("  next: {UNCREATABLE}")
                ],
                "{context} {args:?}"
            );
            assert!(
                lines[4..lines.len() - 1]
                    .iter()
                    .all(|line| !line.starts_with("  ")),
                "{context} {args:?}: {found}"
            );
            // No service can run there, so the service checks pass: the
            // runtime directory is the one finding.
            assert_eq!(
                lines[lines.len() - 1],
                "findings: runtime directory",
                "{context} {args:?}: {found}"
            );
            if !args.is_empty() {
                assert_eq!(lines[6], NO_SERVICE, "{context}: {found}");
                assert_eq!(
                    lines[7],
                    no_service_limits("6144", "60000"),
                    "{context}: {found}"
                );
            }
            let output = doctor(&service, &[args, &["--json"]].concat(), &env);
            assert_eq!(output.status.code(), Some(1), "{context} {output:?}");
            assert!(output.stderr.is_empty(), "{context} {output:?}");
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["outcome"], "findings", "{context} {args:?}");
            assert_eq!(
                document["checks"][2],
                json!({
                    "name": "runtime directory",
                    "status": "finding",
                    "detail": format!("{} (TSPARSE_SERVICE_DIR): {message}", directory.display()),
                    "remedy": UNCREATABLE,
                }),
                "{context} {args:?}"
            );
        }
        // The doctor created nothing; a query reports the same cause.
        assert!(!directory.exists(), "{context}");
        // Status and stop find no service there.
        let administer = |action: &str| {
            service
                .command()
                .env("TSPARSE_SERVICE_DIR", &directory)
                .args([action, "--project", "tsconfig.json"])
                .output()
                .unwrap()
        };
        assert_eq!(
            report(&administer("--references-status"), 0),
            "no service: tsconfig.json\n",
            "{context}"
        );
        assert_eq!(
            report(&administer("--references-stop"), 0),
            "no service: tsconfig.json\n",
            "{context}"
        );
        assert!(!directory.exists(), "{context}");
        let query = service
            .command()
            .env("TSPARSE_SERVICE_DIR", &directory)
            .args([
                "--references",
                "--name",
                "value",
                "--project",
                "tsconfig.json",
            ])
            .output()
            .unwrap();
        assert_eq!(query.status.code(), Some(2), "{context} {query:?}");
        assert_eq!(
            text(&query.stderr).lines().next(),
            Some(format!("transport.directory: {message}{DIAGNOSE}").as_str()),
            "{context}"
        );
    }
    std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o700)).unwrap();
}
