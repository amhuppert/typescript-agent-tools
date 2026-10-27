// validation: run alone: its services must become ready within ten seconds, which other test executables' load can break.
//! Which runtime directory a project's service uses, and why: the
//! directory `TSPARSE_SERVICE_DIR` names, whatever `TMPDIR` is; otherwise
//! `TMPDIR/tsparse-UID` when `TMPDIR` is a non-empty absolute path whose
//! socket paths a socket address holds; otherwise `/tmp/tsparse-UID`. The
//! choice reads only the environment, so a trailing slash in `TMPDIR` names
//! the same directory, and a service a client starts uses the directory the
//! client chose. The doctor names the rule that chose the directory and why
//! `TMPDIR` was not used; a directory that cannot be created names `TMPDIR`
//! and `TSPARSE_SERVICE_DIR` in its remedy, and one that is not private is
//! refused as before. Under a macOS sandbox that denies writes to `/tmp`, a
//! writable `TMPDIR` serves queries.
//!
//! Every test sets or removes both variables. The sandbox tests also
//! inspect the default directory. The denied-lock test creates it if
//! absent, removing only the empty directory it created, and fails naming
//! an existing one that is not private, since no query reaches a lock
//! through it. Directory-denial assertions allow either creation or
//! inspection to fail on each request, independent of changes to that
//! directory between requests. No test starts a service in
//! `/tmp/tsparse-UID`.
#![cfg(unix)]
mod support;

use serde_json::{Value, json};
use std::ffi::OsStr;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use support::service::Service;

/// The longest path a socket address holds: one byte less than
/// `sun_path`, which ends with a NUL, 104 bytes on macOS and 108 on Linux.
fn socket_limit() -> usize {
    if cfg!(target_os = "macos") { 103 } else { 107 }
}

/// What a socket's name adds to its directory: `/`, 16 hexadecimal digits
/// of its key, and `.sock`.
const SOCKET_NAME: usize = 22;

/// What every transport failure of a request for `tsconfig.json` ends its
/// message with.
const DIAGNOSE: &str = "; run: tsparse --doctor --project tsconfig.json";

/// The remedy for a directory of this user's that is not private, or a
/// path that is not this user's directory, as it was before `TMPDIR`
/// could choose the directory.
const ANOTHER_DIRECTORY: &str =
    "set TSPARSE_SERVICE_DIR to a new path or to a private directory this user owns";

/// A private directory under `/tmp`, short enough for socket paths, as
/// `Service`'s runtime directories are, to name as `TMPDIR` or as the
/// working directory; drop removes it and everything in it.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        Self::create(Self::prefix().trim_end_matches('-').into())
    }

    /// One whose path is `length` bytes long.
    fn of_length(length: usize) -> Self {
        let prefix = Self::prefix();
        Self::create(format!("{prefix}{}", "d".repeat(length - prefix.len())))
    }

    /// A path no other scratch directory has.
    fn prefix() -> String {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        format!(
            "/tmp/tsrd-{}-{}-",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    }

    fn create(path: String) -> Self {
        let path = PathBuf::from(path);
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn os(&self) -> &OsStr {
        self.0.as_os_str()
    }

    /// The path, then `suffix`, such as a trailing slash.
    fn with(&self, suffix: &str) -> String {
        format!("{}{suffix}", self.0.display())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// This user's id: the owner of a file this process created.
fn user_id(service: &Service) -> u32 {
    std::fs::metadata(&service.fixture.root).unwrap().uid()
}

/// The directory a variable-free environment gets: `/tmp/tsparse-UID`.
fn default_directory(service: &Service) -> PathBuf {
    PathBuf::from(format!("/tmp/tsparse-{}", user_id(service)))
}

/// `TMPDIR/tsparse-UID`.
fn in_tmpdir(service: &Service, tmpdir: &Path) -> PathBuf {
    tmpdir.join(format!("tsparse-{}", user_id(service)))
}

/// A project whose service, once started from `tmpdir`, drop stops: its
/// runtime directory is the one `tmpdir` chooses.
fn project_in(tmpdir: &Path) -> Service {
    let mut service = Service::new(&[("value.ts", "export const value = 1;\n")]);
    service.runtime = in_tmpdir(&service, tmpdir);
    service
}

/// `tsparse` in the project's directory without `TSPARSE_SERVICE_DIR`,
/// with `TMPDIR` set to `tmpdir`, or unset when it is `None`.
fn choosing(service: &Service, tmpdir: Option<&OsStr>) -> Command {
    let mut command = service.command();
    command.env_remove("TSPARSE_SERVICE_DIR");
    match tmpdir {
        Some(tmpdir) => command.env("TMPDIR", tmpdir),
        None => command.env_remove("TMPDIR"),
    };
    command
}

fn run(mut command: Command, args: &[&str]) -> Output {
    command.args(args).output().unwrap()
}

/// `args` for the project's `tsconfig.json`.
fn project(command: Command, args: &[&str]) -> Output {
    run(command, &[args, &["--project", "tsconfig.json"]].concat())
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The doctor's report as text, which wrote nothing to stderr.
fn doctor(command: Command) -> String {
    let output = run(command, &["--doctor"]);
    assert!(output.stderr.is_empty(), "{output:?}");
    text(&output.stdout)
}

/// The doctor's runtime directory line.
fn runtime_line(command: Command) -> String {
    let report = doctor(command);
    report
        .lines()
        .find(|line| line.starts_with("runtime directory: "))
        .unwrap_or_else(|| panic!("{report}"))
        .to_string()
}

/// The doctor's runtime directory check, as its document has it.
fn runtime_check(command: Command) -> Value {
    let output = run(command, &["--doctor", "--json"]);
    assert!(output.stderr.is_empty(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    report["checks"][2].clone()
}

/// The doctor's line for `directory`, which `rule` chose and which does
/// not exist yet.
fn absent_line(directory: &Path, rule: &str) -> String {
    format!(
        "runtime directory: {} ({rule}) does not exist yet; a project query creates it with mode 0700; its socket paths take {} of at most {} bytes",
        directory.display(),
        directory.as_os_str().len() + SOCKET_NAME,
        socket_limit()
    )
}

/// The doctor's finding for `directory`, which `rule` chose and where a
/// query fails with `message`: the directory and the rule, then the failure.
fn finding(directory: &Path, rule: &str, message: &str) -> String {
    format!("{} ({rule}): {message}", directory.display())
}

/// Why a directory of this user's with group or other permissions is
/// refused.
const OPEN: &str = "any process that can open the socket can run queries as you";

const VARIABLE: &str = "TSPARSE_SERVICE_DIR";
const TMPDIR: &str = "TMPDIR/tsparse-UID, as TSPARSE_SERVICE_DIR is not set";

/// The rule that chose `/tmp/tsparse-UID`, with why `TMPDIR` was not used.
fn default_rule(why: &str) -> String {
    format!("/tmp/tsparse-UID, as TSPARSE_SERVICE_DIR is not set and TMPDIR {why}")
}

/// Why a `TMPDIR` whose socket paths take `length` bytes was not used.
fn too_long(length: usize) -> String {
    format!(
        "makes socket paths of {length} bytes, more than the {} a socket address holds",
        socket_limit()
    )
}

/// The `TMPDIR` whose socket paths take `socket` bytes.
fn tmpdir_for_socket(service: &Service, socket: usize) -> Scratch {
    let name = format!("/tsparse-{}", user_id(service)).len();
    Scratch::of_length(socket - SOCKET_NAME - name)
}

#[test]
fn tsparse_service_dir_names_the_directory_whatever_tmpdir_is() {
    let scratch = Scratch::new();
    let tmpdir = Scratch::new();
    let explicit = scratch.path().join("services");
    let working = std::fs::canonicalize(scratch.path()).unwrap();
    let mut service = project_in(tmpdir.path());
    // Drop stops the service a query below starts there.
    service.runtime = explicit.clone();
    let long = tmpdir_for_socket(&service, socket_limit() + 1);
    let tmpdirs: [Option<&OsStr>; 5] = [
        None,
        Some(OsStr::new("")),
        Some(OsStr::new("relative")),
        Some(tmpdir.os()),
        Some(long.os()),
    ];
    for tmpdir in tmpdirs {
        // An absolute value, and a relative one, which names a directory
        // in the working directory as the system spells it.
        for (value, directory) in [
            (explicit.as_os_str(), explicit.clone()),
            (OsStr::new("services"), working.join("services")),
        ] {
            let mut command = choosing(&service, tmpdir);
            command
                .env("TSPARSE_SERVICE_DIR", value)
                .current_dir(scratch.path());
            assert_eq!(
                runtime_line(command),
                absent_line(&directory, VARIABLE),
                "{tmpdir:?} {value:?}"
            );
        }
    }
    // A query from the relative value starts its service there, though
    // TMPDIR names another directory.
    let mut command = choosing(&service, Some(tmpdir.os()));
    command
        .env("TSPARSE_SERVICE_DIR", "services")
        .current_dir(scratch.path());
    let config = service.fixture.path("tsconfig.json");
    let config = config.to_str().unwrap();
    let answered = run(
        command,
        &["--references", "--name", "value", "--project", config],
    );
    assert!(answered.status.success(), "{answered:?}");
    let status = project(service.command(), &["--references-status", "--json"]);
    let status: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["running"], true, "{status}");
    assert!(!in_tmpdir(&service, tmpdir.path()).exists());
}

#[test]
fn an_absolute_tmpdir_chooses_tmpdir_tsparse_uid_with_or_without_a_trailing_slash() {
    let scratch = Scratch::new();
    let service = project_in(scratch.path());
    let directory = in_tmpdir(&service, scratch.path());
    let spellings = [scratch.with(""), scratch.with("/"), scratch.with("//")];
    for spelling in &spellings {
        let report = doctor(choosing(&service, Some(OsStr::new(spelling))));
        let lines: Vec<&str> = report.lines().collect();
        assert!(
            lines[1].ends_with(&format!(
                ", TSPARSE_SERVICE_DIR={} (default)",
                directory.display()
            )),
            "{spelling}: {report}"
        );
        assert_eq!(lines[2], absent_line(&directory, TMPDIR), "{spelling}");
    }
    assert_eq!(
        runtime_check(choosing(&service, Some(scratch.os()))),
        json!({
            "name": "runtime directory",
            "status": "pass",
            "detail": absent_line(&directory, TMPDIR).strip_prefix("runtime directory: ").unwrap(),
        })
    );
    // A query starts the service there, and every spelling reaches it.
    let answered = project(
        choosing(&service, Some(OsStr::new(&spellings[1]))),
        &["--references", "--name", "value", "--json"],
    );
    assert!(answered.status.success(), "{answered:?}");
    let metadata = std::fs::symlink_metadata(&directory).unwrap();
    assert!(metadata.is_dir());
    assert_eq!(metadata.mode() & 0o777, 0o700);
    let status = |command: Command| {
        let output = project(command, &["--references-status", "--json"]);
        assert!(output.status.success(), "{output:?}");
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let running = status(choosing(&service, Some(OsStr::new(&spellings[0]))));
    assert_eq!(running["running"], true, "{running}");
    assert_eq!(
        status(choosing(&service, Some(OsStr::new(&spellings[2]))))["pid"],
        running["pid"]
    );
    // The service it started uses the directory the client chose, which
    // the variable also names.
    assert_eq!(status(service.command())["pid"], running["pid"]);
    let stopped = project(
        choosing(&service, Some(scratch.os())),
        &["--references-stop"],
    );
    assert_eq!(
        text(&stopped.stdout),
        format!("stopped: pid {}\n", running["pid"])
    );
    assert_eq!(
        status(choosing(&service, Some(scratch.os())))["running"],
        false
    );
}

#[test]
fn an_unusable_tmpdir_is_ignored_and_the_doctor_says_why() {
    let scratch = Scratch::new();
    let service = project_in(scratch.path());
    let default = default_directory(&service);
    let over = tmpdir_for_socket(&service, socket_limit() + 1);
    // Selection reads only the string: this directory need not exist.
    let far_over = format!("/tmp/{}/{}", "d".repeat(200), "d".repeat(94));
    let cases: [(Option<&OsStr>, String); 7] = [
        (None, "is not set".into()),
        (Some(OsStr::new("")), "is empty".into()),
        (
            Some(OsStr::new("relative")),
            "is not an absolute path".into(),
        ),
        (Some(OsStr::new("./tmp/")), "is not an absolute path".into()),
        (Some(OsStr::new("~/tmp")), "is not an absolute path".into()),
        (Some(over.os()), too_long(socket_limit() + 1)),
        (
            Some(OsStr::new(&far_over)),
            too_long(300 + format!("/tsparse-{}", user_id(&service)).len() + SOCKET_NAME),
        ),
    ];
    for (tmpdir, why) in cases {
        // Whatever this machine's default directory holds, the line names
        // it and the rule that chose it.
        let expected = format!(
            "runtime directory: {} ({}) ",
            default.display(),
            default_rule(&why)
        );
        let line = runtime_line(choosing(&service, tmpdir));
        assert!(line.starts_with(&expected), "{tmpdir:?}: {line}");
        let check = runtime_check(choosing(&service, tmpdir));
        assert!(
            check["detail"]
                .as_str()
                .unwrap()
                .starts_with(expected.strip_prefix("runtime directory: ").unwrap()),
            "{tmpdir:?}: {check}"
        );
    }
}

#[test]
fn tmpdir_socket_paths_one_byte_under_at_and_over_the_platform_limit() {
    let scratch = Scratch::new();
    let probe = project_in(scratch.path());
    let limit = socket_limit();
    let under = tmpdir_for_socket(&probe, limit - 1);
    let at = tmpdir_for_socket(&probe, limit);
    let over = tmpdir_for_socket(&probe, limit + 1);
    for tmpdir in [&under, &at] {
        let directory = in_tmpdir(&probe, tmpdir.path());
        // A trailing slash adds nothing to the socket paths.
        for spelling in [tmpdir.with(""), tmpdir.with("/")] {
            assert_eq!(
                runtime_line(choosing(&probe, Some(OsStr::new(&spelling)))),
                absent_line(&directory, TMPDIR),
                "{spelling}"
            );
        }
    }
    let line = runtime_line(choosing(&probe, Some(over.os())));
    let expected = format!(
        "runtime directory: {} ({}) ",
        default_directory(&probe).display(),
        default_rule(&too_long(limit + 1))
    );
    assert!(line.starts_with(&expected), "{line}");
    // The platform agrees: a service started from TMPDIR listens at the
    // limit.
    let service = project_in(at.path());
    let answered = project(
        choosing(&service, Some(OsStr::new(&at.with("/")))),
        &["--references", "--name", "value"],
    );
    assert!(answered.status.success(), "{answered:?}");
    let stopped = project(choosing(&service, Some(at.os())), &["--references-stop"]);
    assert!(
        text(&stopped.stdout).starts_with("stopped: pid "),
        "{stopped:?}"
    );
}

#[test]
fn a_tmpdir_directory_that_is_not_private_is_refused_as_before() {
    let scratch = Scratch::new();
    let service = project_in(scratch.path());
    let directory = service.runtime.clone();
    let shown = directory.to_str().unwrap();
    let message = format!(
        "Project service directory must be a private directory owned by this user with mode 0700: {shown}"
    );
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    // The doctor finds it first, so nothing below starts a service
    // elsewhere; it names the rule that chose the directory, and the
    // failure keeps its remedy.
    let report = doctor(choosing(&service, Some(scratch.os())));
    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(
        lines[2..5],
        [
            format!(
                "runtime directory: {}",
                finding(&directory, TMPDIR, &message)
            ),
            format!("  why: {OPEN}"),
            format!("  next: chmod 700 {shown}"),
        ],
        "{report}"
    );
    assert_eq!(
        runtime_check(choosing(&service, Some(scratch.os()))),
        json!({
            "name": "runtime directory",
            "status": "finding",
            "detail": finding(&directory, TMPDIR, &message),
            "why": OPEN,
            "next": {"argv": ["chmod", "700", shown]},
        })
    );
    for args in [
        &["--references", "--name", "value"][..],
        &["--references-status"],
    ] {
        let output = project(choosing(&service, Some(scratch.os())), args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert_eq!(
            text(&output.stderr),
            format!(
                "transport.directory: {message}{DIAGNOSE}\n  why: {OPEN}\n  next: chmod 700 {shown}\n"
            ),
            "{args:?}"
        );
    }
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::remove_dir(&directory).unwrap();
    // A link to a directory of another user's, and a file: another
    // directory is the remedy.
    let shared = std::fs::canonicalize("/tmp").unwrap();
    std::os::unix::fs::symlink(&shared, &directory).unwrap();
    let linked = project(
        choosing(&service, Some(scratch.os())),
        &["--references", "--json", "--name", "value"],
    );
    let linked_check = runtime_check(choosing(&service, Some(scratch.os())));
    std::fs::remove_file(&directory).unwrap();
    std::fs::write(&directory, "").unwrap();
    let file = project(
        choosing(&service, Some(scratch.os())),
        &["--references", "--name", "value"],
    );
    let file_report = doctor(choosing(&service, Some(scratch.os())));
    std::fs::remove_file(&directory).unwrap();
    assert_eq!(
        linked_check,
        json!({
            "name": "runtime directory",
            "status": "finding",
            "detail": finding(&directory, TMPDIR, &message),
            "remedy": ANOTHER_DIRECTORY,
        })
    );
    let lines: Vec<&str> = file_report.lines().collect();
    assert_eq!(
        lines[2..4],
        [
            format!(
                "runtime directory: {}",
                finding(&directory, TMPDIR, &message)
            ),
            format!("  next: {ANOTHER_DIRECTORY}"),
        ],
        "{file_report}"
    );
    assert_eq!(linked.status.code(), Some(2), "{linked:?}");
    let document: Value = serde_json::from_slice(&linked.stdout).unwrap();
    assert_eq!(
        document["error"],
        json!({
            "category": "transport",
            "code": "transport.directory",
            "message": format!("{message}{DIAGNOSE}"),
            "remedy": ANOTHER_DIRECTORY,
        })
    );
    assert_eq!(file.status.code(), Some(2), "{file:?}");
    assert_eq!(
        text(&file.stderr),
        format!("transport.directory: {message}{DIAGNOSE}\n  next: {ANOTHER_DIRECTORY}\n")
    );
}

#[test]
fn a_directory_that_cannot_be_created_names_tmpdir_and_tsparse_service_dir() {
    let scratch = Scratch::new();
    let missing = scratch.path().join("missing");
    let service = project_in(&missing);
    let explicit = missing.join("runtime");
    let by_variable = || {
        let mut command = choosing(&service, Some(scratch.os()));
        command.env("TSPARSE_SERVICE_DIR", &explicit);
        command
    };
    let by_tmpdir = || choosing(&service, Some(missing.as_os_str()));
    // A TMPDIR that is not a directory.
    let by_file = || choosing(&service, Some(OsStr::new("/dev/null")));
    let variable_remedy = "set TSPARSE_SERVICE_DIR to a path in a directory this user can write, or unset it and set TMPDIR to such a directory";
    let tmpdir_remedy =
        "set TMPDIR to a directory this user can write, or TSPARSE_SERVICE_DIR to a path in one";
    let cases: [(&dyn Fn() -> Command, PathBuf, &str, &str); 3] = [
        (&by_variable, explicit.clone(), VARIABLE, variable_remedy),
        (
            &by_tmpdir,
            in_tmpdir(&service, &missing),
            TMPDIR,
            tmpdir_remedy,
        ),
        (
            &by_file,
            in_tmpdir(&service, Path::new("/dev/null")),
            TMPDIR,
            tmpdir_remedy,
        ),
    ];
    for (command, directory, rule, remedy) in cases {
        let cause = std::fs::create_dir(&directory).unwrap_err();
        let message = format!(
            "Project service directory {} cannot be created: {cause}",
            directory.display()
        );
        // The doctor finds it first, so nothing below starts a service
        // elsewhere.
        let report = doctor(command());
        let lines: Vec<&str> = report.lines().collect();
        assert_eq!(
            lines[2..4],
            [
                format!("runtime directory: {}", finding(&directory, rule, &message)),
                format!("  next: {remedy}")
            ],
            "{report}"
        );
        assert_eq!(
            runtime_check(command()),
            json!({
                "name": "runtime directory",
                "status": "finding",
                "detail": finding(&directory, rule, &message),
                "remedy": remedy,
            })
        );
        let output = project(command(), &["--references", "--name", "value"]);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert_eq!(
            text(&output.stderr),
            format!("transport.directory: {message}{DIAGNOSE}\n  next: {remedy}\n")
        );
        let output = project(command(), &["--show", "--json", "--name", "value"]);
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            document["error"],
            json!({
                "category": "transport",
                "code": "transport.directory",
                "message": format!("{message}{DIAGNOSE}"),
                "remedy": remedy,
            })
        );
    }
    assert!(!missing.exists());
}

/// Whether `sandbox-exec` applies a profile here; a process already in a
/// sandbox may not apply another.
fn sandbox_available() -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("sandbox-exec is macOS's".into());
    }
    match Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
        .output()
    {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(format!(
            "sandbox-exec cannot apply a profile here: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(error) => Err(format!("sandbox-exec cannot run: {error}")),
    }
}

/// `tsparse` with `args` in the project's directory under `profile`, with
/// `TSPARSE_SERVICE_DIR` unset and `TMPDIR` as `tmpdir` sets it.
fn sandboxed(service: &Service, profile: &str, tmpdir: Option<&OsStr>, args: &[&str]) -> Output {
    sandboxed_command(choosing(service, tmpdir), profile, args)
}

fn sandboxed_command(program: Command, profile: &str, args: &[&str]) -> Output {
    let mut command = Command::new("/usr/bin/sandbox-exec");
    command
        .args(["-p", profile])
        .arg(program.get_program())
        .args(args)
        .current_dir(program.get_current_dir().unwrap());
    for (name, value) in program.get_envs() {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
    command.output().unwrap()
}

/// A child receives an explicit runtime directory even when its client
/// selected TMPDIR. A failure only the child sees must reach the client
/// with the client's remedy, not the child's different selection rule.
#[test]
fn a_child_lock_failure_keeps_the_clients_tmpdir_remedy() {
    let scratch = Scratch::new();
    let service = project_in(scratch.path());
    let expected = service.query("value");
    assert!(service.run(&["--references-stop"]).status.success());
    let locks: Vec<_> = std::fs::read_dir(&service.runtime)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_str().unwrap();
            name.ends_with(".lock") && !name.ends_with(".start.lock")
        })
        .collect();
    assert_eq!(locks.len(), 1, "{locks:?}");
    let lock = &locks[0];
    let recorded = lock.with_extension("start-error");
    // The client can acquire its start lock; only the child opens this one.
    std::fs::remove_file(lock).unwrap();
    std::fs::create_dir(lock).unwrap();
    let cause = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock)
        .unwrap_err();
    let message = format!("Project service lock {}: {cause}{DIAGNOSE}", lock.display());
    let remedy =
        "set TMPDIR to a directory this user can write, or TSPARSE_SERVICE_DIR to a path in one";
    for json in [false, true] {
        let mut args = vec!["--references", "--name", "value"];
        if json {
            args.push("--json");
        }
        let output = project(choosing(&service, Some(scratch.os())), &args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        if json {
            assert!(output.stderr.is_empty(), "{output:?}");
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["error"]["code"], "transport.lock", "{document}");
            assert_eq!(document["error"]["message"], message, "{document}");
            assert_eq!(document["error"]["remedy"], remedy, "{document}");
        } else {
            assert!(output.stdout.is_empty(), "{output:?}");
            assert_eq!(
                text(&output.stderr),
                format!("transport.lock: {message}\n  next: {remedy}\n")
            );
        }
        assert!(
            !recorded.exists(),
            "the client removes the child diagnostic"
        );
        assert_eq!(service.status()["running"], false);
    }
    std::fs::remove_dir(lock).unwrap();
    let output = project(
        choosing(&service, Some(scratch.os())),
        &[
            "--references",
            "--name",
            "value",
            "--json",
            "--limit",
            "all",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        expected
    );
}

/// Directory metadata can be readable and private while sandbox policy
/// denies lock creation. The CLI must retain that failure and explain the
/// actual runtime selection in both output modes.
#[test]
fn a_denied_lock_names_the_runtime_remedy_and_why_tmpdir_was_skipped() {
    if let Err(reason) = sandbox_available() {
        eprintln!(
            "SKIP a_denied_lock_names_the_runtime_remedy_and_why_tmpdir_was_skipped: {reason}"
        );
        return;
    }
    let scratch = Scratch::new();
    let service = project_in(scratch.path());
    let default = default_directory(&service);
    // Do not alter an existing default directory or any of its files.
    let _created = match std::fs::create_dir(&default) {
        Ok(()) => {
            std::fs::set_permissions(&default, std::fs::Permissions::from_mode(0o700)).unwrap();
            Some(EmptyDirectory(default.clone()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
        Err(error) => panic!("{}: {error}", default.display()),
    };
    // A query reaches the lock only through a private directory of this
    // user's; any other existing default directory is refused before it.
    let metadata = std::fs::symlink_metadata(&default).unwrap();
    assert!(
        metadata.is_dir()
            && metadata.uid() == user_id(&service)
            && metadata.mode() & 0o777 == 0o700,
        "{} must be a private directory owned by this user with mode 0700, or absent, for a query to reach its lock",
        default.display()
    );
    let long = tmpdir_for_socket(&service, socket_limit() + 1);
    let profile = format!(
        r#"(version 1)(allow default)(deny file-write* (subpath "{}"))"#,
        std::fs::canonicalize(&default).unwrap().display()
    );
    for (tmpdir, why) in [
        (None, "is not set".to_string()),
        (Some(OsStr::new("")), "is empty".to_string()),
        (
            Some(OsStr::new("relative")),
            "is not an absolute path".to_string(),
        ),
        (Some(long.os()), too_long(socket_limit() + 1)),
    ] {
        let remedy = format!(
            "set TMPDIR to an absolute directory this user can write, or TSPARSE_SERVICE_DIR to a path in one; TMPDIR {why}"
        );
        for json in [false, true] {
            let mut args = vec![
                "--references",
                "--name",
                "value",
                "--project",
                "tsconfig.json",
            ];
            if json {
                args.push("--json");
            }
            let output = sandboxed(&service, &profile, tmpdir, &args);
            assert_eq!(output.status.code(), Some(2), "{why}: {output:?}");
            if json {
                assert!(output.stderr.is_empty(), "{output:?}");
                let document: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(document["error"]["code"], "transport.lock", "{document}");
                assert!(
                    document["error"]["message"]
                        .as_str()
                        .unwrap()
                        .contains("Operation not permitted (os error 1)"),
                    "{document}"
                );
                assert_eq!(document["error"]["remedy"], remedy, "{document}");
            } else {
                assert!(output.stdout.is_empty(), "{output:?}");
                let stderr = text(&output.stderr);
                assert!(
                    stderr.starts_with("transport.lock: Project service lock "),
                    "{stderr}"
                );
                assert!(
                    stderr.contains("Operation not permitted (os error 1)"),
                    "{stderr}"
                );
                assert!(
                    stderr.ends_with(&format!("{DIAGNOSE}\n  next: {remedy}\n")),
                    "{stderr}"
                );
            }
        }
    }
    // The two successful environment selections need different remedies;
    // deny writes while preserving mode 0700 and readable metadata.
    std::fs::create_dir(&service.runtime).unwrap();
    std::fs::set_permissions(&service.runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let profile = format!(
        r#"(version 1)(allow default)(deny file-write* (subpath "{}"))"#,
        std::fs::canonicalize(&service.runtime).unwrap().display()
    );
    for (command, remedy) in [
        (
            choosing(&service, Some(scratch.os())),
            "set TMPDIR to a directory this user can write, or TSPARSE_SERVICE_DIR to a path in one",
        ),
        (
            service.command(),
            "set TSPARSE_SERVICE_DIR to a path in a directory this user can write, or unset it and set TMPDIR to such a directory",
        ),
    ] {
        let output = sandboxed_command(
            command,
            &profile,
            &[
                "--references",
                "--name",
                "value",
                "--project",
                "tsconfig.json",
                "--json",
            ],
        );
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["error"]["code"], "transport.lock", "{document}");
        assert_eq!(document["error"]["remedy"], remedy, "{document}");
    }
}

/// Remove only a directory this test created, and only if it is still
/// empty; never remove another process's runtime files, including on panic.
struct EmptyDirectory(PathBuf);

impl Drop for EmptyDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.0);
    }
}

#[test]
fn a_sandbox_that_denies_writes_to_tmp_serves_queries_from_a_writable_tmpdir() {
    if let Err(reason) = sandbox_available() {
        eprintln!(
            "SKIP a_sandbox_that_denies_writes_to_tmp_serves_queries_from_a_writable_tmpdir: {reason}"
        );
        return;
    }
    let scratch = Scratch::new();
    let service = project_in(scratch.path());
    let default = default_directory(&service);
    let private = |path: &Path| {
        std::fs::canonicalize(path)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    };
    // Writes under /tmp are denied except in the scratch directory. A
    // default directory an unsandboxed run left is hidden too, so a query
    // fails on it as on one it cannot create, not later on its lock file.
    let profile = format!(
        r#"(version 1)(allow default)(deny file-write* (subpath "{}"))(deny file-read* file-write* (subpath "{}/{}"))(allow file-write* (subpath "{}"))"#,
        private(Path::new("/tmp")),
        private(Path::new("/tmp")),
        default.file_name().unwrap().to_str().unwrap(),
        private(scratch.path()),
    );
    let config = ["--project", "tsconfig.json"];
    let query = [&["--references", "--name", "value"][..], &config].concat();
    // Another test or process may create or remove the default directory
    // between requests. Either denied operation is correct; do not predict
    // all later responses from one observation of its existence.
    let messages = ["inspected", "created"].map(|operation| {
        format!(
            "Project service directory {} cannot be {operation}: Operation not permitted (os error 1)",
            default.display()
        )
    });
    let remedy = |why: &str| {
        format!(
            "set TMPDIR to an absolute directory this user can write, or TSPARSE_SERVICE_DIR to a path in one; TMPDIR {why}"
        )
    };
    let refused = sandboxed(&service, &profile, None, &query);
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
    assert!(refused.stdout.is_empty(), "{refused:?}");
    let expected = messages.each_ref().map(|message| {
        format!(
            "transport.directory: {message}{DIAGNOSE}\n  next: {}\n",
            remedy("is not set")
        )
    });
    assert!(
        expected.contains(&text(&refused.stderr)),
        "{refused:?}; expected one of {expected:?}"
    );
    // The doctor's finding names the rule that chose the default directory
    // and why TMPDIR was not used.
    for (tmpdir, why) in [
        (None, "is not set"),
        (Some(OsStr::new("")), "is empty"),
        (Some(OsStr::new("relative")), "is not an absolute path"),
    ] {
        let details = messages
            .each_ref()
            .map(|message| finding(&default, &default_rule(why), message));
        let report = sandboxed(&service, &profile, tmpdir, &["--doctor"]);
        assert_eq!(report.status.code(), Some(1), "{report:?}");
        assert!(report.stderr.is_empty(), "{report:?}");
        let report = text(&report.stdout);
        let lines: Vec<&str> = report.lines().collect();
        assert!(
            details.iter().any(|detail| lines[2..4]
                == [
                    format!("runtime directory: {detail}"),
                    format!("  next: {}", remedy(why)),
                ]),
            "{report}"
        );
        let report = sandboxed(&service, &profile, tmpdir, &["--doctor", "--json"]);
        assert_eq!(report.status.code(), Some(1), "{report:?}");
        assert!(report.stderr.is_empty(), "{report:?}");
        let report: Value = serde_json::from_slice(&report.stdout).unwrap();
        assert!(
            details.iter().any(|detail| report["checks"][2]
                == json!({
                    "name": "runtime directory",
                    "status": "finding",
                    "detail": detail,
                    "remedy": remedy(why),
                })),
            "{why}: {report}"
        );
    }
    // With TMPDIR naming the scratch directory, the service runs there:
    // queries are answered, and status and stop see it.
    let tmpdir = Some(scratch.os());
    let answered = sandboxed(
        &service,
        &profile,
        tmpdir,
        &[&query[..], &["--json"]].concat(),
    );
    assert!(answered.status.success(), "{answered:?}");
    let answer: Value = serde_json::from_slice(&answered.stdout).unwrap();
    assert_eq!(answer["outcome"], "complete", "{answer}");
    assert!(service.runtime.is_dir());
    let doctor = sandboxed(
        &service,
        &profile,
        tmpdir,
        &[&["--doctor"][..], &config].concat(),
    );
    assert_eq!(
        text(&doctor.stdout).lines().nth(2),
        Some(
            format!(
                "runtime directory: {} ({TMPDIR}) is a private directory owned by this user with mode 0700; its socket paths take {} of at most {} bytes",
                service.runtime.display(),
                service.runtime.as_os_str().len() + SOCKET_NAME,
                socket_limit()
            )
            .as_str()
        ),
        "{doctor:?}"
    );
    let status = sandboxed(
        &service,
        &profile,
        tmpdir,
        &[&["--references-status", "--json"][..], &config].concat(),
    );
    assert!(status.status.success(), "{status:?}");
    let status: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["running"], true, "{status}");
    let stopped = sandboxed(
        &service,
        &profile,
        tmpdir,
        &[&["--references-stop"][..], &config].concat(),
    );
    assert_eq!(
        text(&stopped.stdout),
        format!("stopped: pid {}\n", status["pid"]),
        "{stopped:?}"
    );
    let after = sandboxed(
        &service,
        &profile,
        tmpdir,
        &[&["--references-status"][..], &config].concat(),
    );
    assert_eq!(
        text(&after.stdout),
        "no service: tsconfig.json\n",
        "{after:?}"
    );
}
