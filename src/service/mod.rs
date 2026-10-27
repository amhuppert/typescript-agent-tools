//! Local process reuse for CLI project queries: references, outline, and
//! show. One service per configuration keeps a `ProjectSession`, whose one
//! request boundary refreshes the generation before every operation. The
//! wire format, rendezvous, startup races, and shutdown belong here; callers
//! supply complete operations.
//!
//! Each transport failure is a `Failure` with its own `transport.*` code,
//! made where its cause is known and kept until `cli::delivery` delivers
//! it: the runtime directory, its socket, its lock files, a service's
//! startup, a wait that ran out, an exchange the other side cannot use, and
//! a service of another identity. Each leaves this module's entry points
//! naming the doctor (`diagnosed`), which observes the same owners without
//! that suffix: the status (`observe`), the runtime directory
//! (`inspect_directory`), and the configuration's identity (`absolute`).

mod lease;
pub(crate) mod runtime;

pub(crate) use lease::user_id;

use crate::cli::Failure;
use crate::cli::command_line;
use crate::cli::delivery;
use crate::cli::failure::{Category, Code};
use crate::cli::registry::FlagId;
use crate::process_limits::{Context, Environment, Limits};
use runtime::{Rule, RuntimeDirectory};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};
use typescript_parser::analysis::AnalysisError;
use typescript_parser::project::{ProjectRequest, ProjectResponse, ProjectSession};

const STARTUP_WAIT: Duration = Duration::from_secs(10);
const CLIENT_WAIT: Duration = Duration::from_secs(300);
const SOCKET_WAIT: Duration = Duration::from_secs(5);
/// How long a client about to start a service waits for the writer of a
/// staged startup diagnostic (`Endpoint::discard_start_errors`).
const STAGED_WRITER_WAIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);
const REQUEST_LIMIT: u64 = 1 << 20;
/// The permissions of the runtime directory: everything for its owner, and
/// nothing for anyone else.
const PRIVATE_DIRECTORY: u32 = 0o700;
/// The hexadecimal digits of an endpoint's key, which names its files.
const KEY_DIGITS: usize = 16;

#[derive(Debug)]
pub(crate) enum Error {
    Analysis(AnalysisError),
    /// The failure that left no answer: a variable the service would refuse,
    /// found before starting one, or a transport failure.
    Unanswered(Failure),
}

impl From<Failure> for Error {
    fn from(failure: Failure) -> Self {
        Self::Unanswered(failure)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct Identity {
    config: PathBuf,
    build: String,
}

#[derive(Serialize, Deserialize)]
struct Request {
    identity: Identity,
    action: Action,
}

#[derive(Serialize, Deserialize)]
enum Action {
    Query(ProjectRequest),
    Status,
    Stop,
}

#[derive(Serialize, Deserialize)]
enum Response {
    Query(Result<ProjectResponse, AnalysisError>),
    Status(Running),
    /// The service exits; its process id.
    Stopped(u32),
    /// The service answered no request, and why.
    Refused(Refusal),
}

/// Why a service answered no request. A client and the service it reaches
/// are one build, so each reason keeps its own code across the socket.
#[derive(Serialize, Deserialize)]
enum Refusal {
    /// The request was incomplete, too large, or not a request.
    Unreadable(String),
    /// The request names another configuration or build.
    Identity,
    /// The query's time and memory watchdog could not run.
    Watchdog(String),
}

impl Refusal {
    fn failure(self) -> Failure {
        match self {
            Refusal::Unreadable(message) => Failure::new(Code::Protocol, message),
            Refusal::Identity => Failure::new(Code::Identity, "Project service identity mismatch"),
            // As a project loaded in this process reports it.
            Refusal::Watchdog(message) => Failure::new(Code::Watchdog, message),
        }
    }
}

/// What `--references-stop` did, which it prints as its verdict (design,
/// step 5, "Clean verdicts").
pub(crate) enum Stopped {
    /// No service was running for the configuration.
    Absent,
    /// The service with this process id stopped.
    Service(u32),
}

impl Stopped {
    /// The verdict: `stopped: pid N`, or `no service: CONFIG`.
    pub(crate) fn text(&self, config: &str) -> String {
        match self {
            Stopped::Absent => no_service(config),
            Stopped::Service(pid) => format!("stopped: pid {pid}"),
        }
    }
}

/// How status and stop say that no service runs for `config`, naming it as
/// the request spelled it.
fn no_service(config: &str) -> String {
    format!("no service: {config}")
}

/// What `--references-status` reports: administrative observation only.
/// Status neither loads a project, validates its freshness, nor extends
/// the service's idle time.
pub(crate) enum Status {
    /// No service is running for the configuration.
    Absent,
    Running(Running),
}

/// A running service as it reports itself.
#[derive(Serialize, Deserialize)]
pub(crate) struct Running {
    pub(crate) pid: u32,
    /// The number of snapshots it has published.
    pub(crate) generation: u64,
    /// Its build stamp, which is its client's: a service refuses a request
    /// of another build.
    pub(crate) build: String,
    /// The limits it read when it started, which its queries run under.
    pub(crate) limits: Limits,
    pub(crate) idle_secs: u64,
    /// Seconds until it exits unless a query arrives first.
    pub(crate) idle_remaining_secs: u64,
    /// When it started listening, in seconds since the Unix epoch.
    pub(crate) started_unix: i64,
}

/// The status document: `running`, `pid`, and `generation`, then what a
/// running service reports of itself, each `null` without one.
#[derive(Serialize)]
struct Document<'a> {
    running: bool,
    pid: Option<u32>,
    generation: u64,
    build: Option<&'a str>,
    limits: Option<Limits>,
    idle_secs: Option<u64>,
    idle_remaining_secs: Option<u64>,
    started_unix: Option<i64>,
}

impl Status {
    /// The status document.
    pub(crate) fn document(&self) -> impl Serialize + '_ {
        match self {
            Status::Absent => Document {
                running: false,
                pid: None,
                generation: 0,
                build: None,
                limits: None,
                idle_secs: None,
                idle_remaining_secs: None,
                started_unix: None,
            },
            Status::Running(running) => Document {
                running: true,
                pid: Some(running.pid),
                generation: running.generation,
                build: Some(&running.build),
                limits: Some(running.limits),
                idle_secs: Some(running.idle_secs),
                idle_remaining_secs: Some(running.idle_remaining_secs),
                started_unix: Some(running.started_unix),
            },
        }
    }

    /// Status as text: `no service: CONFIG`, or a running service's line
    /// and its limits, `limits: MEMORY MiB, TIMEOUT ms`.
    pub(crate) fn text(&self, config: &str) -> String {
        match self {
            Status::Absent => no_service(config),
            Status::Running(running) => format!(
                "running: {}\nlimits: {} MiB, {} ms",
                running.summary(),
                running.limits.max_memory_mb,
                running.limits.query_timeout_ms
            ),
        }
    }
}

impl Running {
    /// The service in one line: its pid, generation, build, idle timeout,
    /// and the idle time it has left, as status prints them.
    pub(crate) fn summary(&self) -> String {
        format!(
            "pid {}, generation {}, build {}, idle {}, {} remaining",
            self.pid,
            self.generation,
            self.build,
            seconds(self.idle_secs),
            seconds(self.idle_remaining_secs)
        )
    }
}

/// A number of seconds as hours, minutes, and seconds, each only when it is
/// not zero: `1h`, `7m12s`, and `0s` for none.
pub(crate) fn seconds(total: u64) -> String {
    if total == 0 {
        return "0s".into();
    }
    let mut text = String::new();
    for (unit, size) in [("h", 3600), ("m", 60), ("s", 1)] {
        let count = total / size % if size == 3600 { u64::MAX } else { 60 };
        if count > 0 {
            text.push_str(&format!("{count}{unit}"));
        }
    }
    text
}

/// `time` in seconds since the Unix epoch, negative before it.
pub(crate) fn unix_seconds(time: SystemTime) -> i64 {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(after) => after.as_secs() as i64,
        Err(before) => -(before.duration().as_secs() as i64),
    }
}

struct Endpoint {
    directory: PathBuf,
    socket: PathBuf,
    lock: PathBuf,
    /// Held by the one client that is starting a service for this endpoint,
    /// until that service listens or exits, so the startup diagnostic it reads
    /// is its own service's. Like `lock`, it is never removed.
    start_lock: PathBuf,
    /// The startup diagnostic: the code and message of a failure before a
    /// service bound its socket, published once by that service, since a
    /// client starts it with null standard streams, and read and removed by
    /// the client that started it. A client removes an earlier one before
    /// starting another.
    start_error: PathBuf,
    /// The one copy of a startup diagnostic being written, renamed to
    /// `start_error` once complete. Its writer creates it new and holds its
    /// lock until then, so a copy whose lock no process holds was abandoned
    /// by a writer that died, and is reclaimed.
    staged_start_error: PathBuf,
    identity: Identity,
    /// The rule that chose `directory`, whose remedy directory and lock
    /// failures name.
    rule: Rule,
}

impl Identity {
    /// The identity of the service for `config`: its absolute spelling,
    /// never resolved through links, and this build.
    fn of(config: &Path) -> Result<Self, Failure> {
        Ok(Self {
            config: std::path::absolute(config).map_err(|error| {
                Failure::new(
                    Code::Identity,
                    format!(
                        "Project service for {} cannot be named: its absolute path cannot be established: {error}",
                        config.display()
                    ),
                )
            })?,
            build: env!("TSPARSE_SERVICE_BUILD").into(),
        })
    }
}

/// The socket of the endpoint whose key is `key`, in `directory`.
fn socket_path(directory: &Path, key: &str) -> PathBuf {
    directory.join(format!("{key}.sock"))
}

/// The socket paths in a directory: their length, which every endpoint's
/// there shares, since every key has `KEY_DIGITS` digits, and the longest
/// path a socket address holds on this platform (`socket_path_limit`).
#[derive(Clone, Copy)]
struct SocketBudget {
    length: usize,
    limit: usize,
}

impl SocketBudget {
    fn of(directory: &Path) -> Self {
        Self {
            length: socket_path(directory, &"0".repeat(KEY_DIGITS))
                .as_os_str()
                .len(),
            limit: socket_path_limit(),
        }
    }

    /// Whether a socket address holds them. `socket_path_limit` is the
    /// longest path `SocketAddr::from_pathname`, which binding and
    /// connecting use, accepts; the only other path it refuses holds a NUL,
    /// which no runtime directory does, as each is an environment
    /// variable's value or a literal.
    fn fits(&self) -> bool {
        self.length <= self.limit
    }
}

impl Endpoint {
    fn new(config: &Path) -> Result<Self, Failure> {
        let identity = Identity::of(config)?;
        Ok(Self::named(identity, runtime::chosen()?))
    }

    #[cfg(test)]
    fn in_directory(config: &Path, directory: PathBuf) -> Result<Self, Failure> {
        Ok(Self::named(
            Identity::of(config)?,
            RuntimeDirectory {
                path: directory,
                rule: Rule::Variable,
            },
        ))
    }

    /// The endpoint of the service `identity` names, in `directory`: its
    /// files are named by a key hashed from the identity.
    fn named(identity: Identity, directory: RuntimeDirectory) -> Self {
        let RuntimeDirectory {
            path: directory,
            rule,
        } = directory;
        let mut hash = DefaultHasher::new();
        identity.build.hash(&mut hash);
        identity.config.hash(&mut hash);
        let key = format!("{:0KEY_DIGITS$x}", hash.finish());
        Self {
            socket: socket_path(&directory, &key),
            lock: directory.join(format!("{key}.lock")),
            start_lock: directory.join(format!("{key}.start.lock")),
            start_error: directory.join(format!("{key}.start-error")),
            staged_start_error: directory.join(format!("{key}.start-error.staged")),
            directory,
            identity,
            rule,
        }
    }

    fn prepare(&self) -> Result<(), Failure> {
        match std::fs::DirBuilder::new()
            .mode(PRIVATE_DIRECTORY)
            .create(&self.directory)
        {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(uncreatable(&self.directory, &self.rule, error)),
        }
        let metadata = std::fs::symlink_metadata(&self.directory)
            .map_err(|error| uninspected(&self.directory, &self.rule, error))?;
        require_private(&self.directory, &metadata)
    }

    /// A connection to a running service, or `None` when none listens. A
    /// runtime directory that is `absent` holds no service.
    fn connect(&self) -> Result<Option<UnixStream>, Failure> {
        let metadata = match std::fs::symlink_metadata(&self.directory) {
            Ok(metadata) => metadata,
            Err(error) if absent(&error) => return Ok(None),
            Err(error) => return Err(uninspected(&self.directory, &self.rule, error)),
        };
        require_private(&self.directory, &metadata)?;
        match UnixStream::connect(&self.socket) {
            Ok(stream) => Ok(Some(stream)),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(self.socket_error(error)),
        }
    }

    /// A failure to bind or connect the socket, naming it. A socket address
    /// holds a path of about a hundred bytes, so a long runtime directory
    /// makes every request fail with `InvalidInput`, whose remedy is a
    /// shorter one.
    fn socket_error(&self, error: io::Error) -> Failure {
        let failure = Failure::new(
            Code::Socket,
            format!("Project service socket {}: {error}", self.socket.display()),
        );
        if error.kind() == io::ErrorKind::InvalidInput {
            failure.with_remedy(SHORTER_DIRECTORY)
        } else {
            failure
        }
    }

    /// A failure of an exchange over a connected socket: a wait that ran
    /// out, `waited`, or a connection that failed.
    fn connection_error(&self, error: io::Error, waited: &str) -> Failure {
        match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
                Failure::new(Code::Timeout, format!("Project service {waited}"))
            }
            _ => self.connection_failed(error),
        }
    }

    fn connection_failed(&self, error: io::Error) -> Failure {
        Failure::new(
            Code::Socket,
            format!(
                "Project service connection {}: {error}",
                self.socket.display()
            ),
        )
    }

    /// Why no answer could be read: the wait for it ran out, the connection
    /// failed or closed first, or what arrived is not an answer.
    fn unanswered(&self, error: serde_json::Error) -> Failure {
        let waited = format!("did not answer within {} seconds", CLIENT_WAIT.as_secs());
        match error.classify() {
            serde_json::error::Category::Io => self.connection_error(error.into(), &waited),
            serde_json::error::Category::Eof => Failure::new(
                Code::Socket,
                format!(
                    "Project service connection {} closed before an answer",
                    self.socket.display()
                ),
            ),
            serde_json::error::Category::Syntax | serde_json::error::Category::Data => {
                Failure::new(
                    Code::Protocol,
                    format!("Project service answer cannot be decoded: {error}"),
                )
            }
        }
    }

    /// Remove the staged diagnostic if its writer abandoned it, and say
    /// whether the staged path is free; a copy whose writer holds its lock is
    /// left to that writer.
    fn reclaim_staged_start_error(&self) -> io::Result<bool> {
        let Some(copy) = open_regular(&self.staged_start_error)? else {
            return Ok(true);
        };
        // Only the holder of a copy's lock removes or renames it, so a path
        // that still names the copy whose lock this process took keeps
        // naming it. Another name there is a later writer's copy.
        if !lease::try_lock(&copy)? || !names(&self.staged_start_error, &copy)? {
            return Ok(false);
        }
        std::fs::remove_file(&self.staged_start_error)?;
        Ok(true)
    }

    /// Remove the diagnostics an earlier start left, before this client
    /// starts a service whose diagnostic it will read: the published one and
    /// an abandoned partial copy. A staged copy's writer holds its lock while
    /// it writes, and also while it dies mid-write: a process killed by a
    /// signal that dumps core keeps its locks until the dump completes, about
    /// a second where a helper such as apport receives it. So a held copy is
    /// waited on, up to `STAGED_WRITER_WAIT`, until its writer publishes it,
    /// which is then removed too, removes it, or dies, and it is reclaimed; a
    /// copy still held then is left to its writer.
    fn discard_start_errors(&self) -> Result<(), Failure> {
        let cannot_remove = |path: &Path, error: io::Error| {
            Failure::new(
                Code::Startup,
                format!(
                    "Project service startup diagnostic {} cannot be removed: {error}",
                    path.display()
                ),
            )
        };
        let waiting = Instant::now();
        loop {
            // The staged copy first: once it is gone, a writer that had
            // created it has already published, and that is removed next.
            let free = self
                .reclaim_staged_start_error()
                .map_err(|error| cannot_remove(&self.staged_start_error, error))?;
            match std::fs::remove_file(&self.start_error) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(cannot_remove(&self.start_error, error)),
            }
            if free || waiting.elapsed() >= STAGED_WRITER_WAIT {
                return Ok(());
            }
            std::thread::sleep(POLL);
        }
    }

    /// A connection to the service, started when none is running. Every
    /// cause the client can see is reported before a service is started: an
    /// environment the service would refuse, a runtime directory it could not
    /// use, and a socket path the platform cannot hold.
    fn start(&self) -> Result<UnixStream, Failure> {
        Environment::read()?;
        self.prepare()?;
        // Clients that find no service start one at a time; the others wait
        // for the start lock or for the service its holder started.
        let waiting = Instant::now();
        let _starting = loop {
            if let Some(stream) = self.connect()? {
                return Ok(stream);
            }
            if let Some(starting) = lease::Lease::acquire(&self.start_lock)
                .map_err(|error| self.lock_error(&self.start_lock, error))?
            {
                break starting;
            }
            if waiting.elapsed() >= STARTUP_WAIT {
                return Err(Failure::new(
                    Code::Timeout,
                    "Project service did not become ready within 10 seconds while another client was starting it",
                ));
            }
            std::thread::sleep(POLL);
        };
        // The lock's last holder may have started a service since.
        if let Some(stream) = self.connect()? {
            return Ok(stream);
        }
        // An earlier start's diagnostic must not be taken for this one's.
        self.discard_start_errors()?;
        let unstarted = |error: io::Error| {
            Failure::new(
                Code::Startup,
                format!("Project service cannot be started: {error}"),
            )
        };
        let mut child = Command::new(std::env::current_exe().map_err(unstarted)?)
            .arg(FlagId::ReferencesServe.flag().name)
            .arg(FlagId::Project.flag().name)
            .arg(&self.identity.config)
            .env(crate::process_limits::SERVICE_DIR, &self.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(unstarted)?;
        let started = Instant::now();
        loop {
            if let Some(stream) = self.connect()? {
                return Ok(stream);
            }
            let exited = child.try_wait().map_err(|error| {
                Failure::new(
                    Code::Startup,
                    format!("Project service process cannot be observed: {error}"),
                )
            })?;
            if let Some(status) = exited {
                if !status.success() {
                    return Err(self.start_failure());
                }
                // A foreground service may hold the lease before bind.
            }
            if started.elapsed() >= STARTUP_WAIT {
                return Err(Failure::new(
                    Code::Timeout,
                    "Project service did not become ready within 10 seconds",
                ));
            }
            std::thread::sleep(POLL);
        }
    }

    /// Why the service this client started exited unsuccessfully: the
    /// failure it published, with its own code, which is removed, or, when
    /// it published none, the instruction to run it in the foreground, where
    /// its standard error shows the cause. A diagnostic that exists but
    /// cannot be read or removed is reported as such, and so is a partial
    /// copy the service abandoned, for example when it was killed while
    /// writing it, that cannot be removed.
    fn start_failure(&self) -> Failure {
        let failure = self.published_failure();
        match self.reclaim_staged_start_error() {
            Ok(_) => failure,
            Err(error) => failure.reworded(|message| {
                format!(
                    "{message}; the startup diagnostic {} cannot be removed: {error}",
                    self.staged_start_error.display()
                )
            }),
        }
    }

    fn published_failure(&self) -> Failure {
        let read = open_regular(&self.start_error)
            .and_then(|published| published.map(io::read_to_string).transpose());
        let recorded = match read {
            Ok(Some(recorded)) => recorded,
            Ok(None) => return self.unrecorded(),
            Err(error) => return self.unreadable_start_error(error),
        };
        let failure = match recorded_failure(&recorded) {
            Ok(failure) if failure.code() == Code::Lock => failure.with_remedy(self.rule.remedy()),
            Ok(failure) => failure,
            Err(error) => self.unreadable_start_error(error),
        };
        match std::fs::remove_file(&self.start_error) {
            Ok(()) => failure,
            Err(error) => failure.reworded(|message| {
                format!(
                    "{message}; the startup diagnostic {} cannot be removed: {error}",
                    self.start_error.display()
                )
            }),
        }
    }

    /// A start whose service exited without recording why: the command
    /// that runs that service in the foreground, as this client started
    /// it, for the configuration's absolute spelling, where its standard
    /// error shows the cause. A path that is not Unicode is never rendered
    /// into a command, so the remedy is said instead.
    fn unrecorded(&self) -> Failure {
        let failure = Failure::new(
            Code::Startup,
            "Project service startup failed without recording why; in the foreground it prints the cause",
        );
        match self.identity.config.to_str() {
            Some(config) => failure.with_next(vec![
                "tsparse".into(),
                FlagId::ReferencesServe.to_string(),
                FlagId::Project.to_string(),
                config.into(),
            ]),
            None => failure.with_remedy(format!(
                "run {} with {} naming this configuration",
                FlagId::ReferencesServe,
                FlagId::Project
            )),
        }
    }

    fn unreadable_start_error(&self, error: io::Error) -> Failure {
        Failure::new(
            Code::Startup,
            format!(
                "Project service startup failed, and its diagnostic {} cannot be read: {error}",
                self.start_error.display()
            ),
        )
    }

    /// Record `failure`, a failure before this service bound its socket, for
    /// the client that started it, and return it. A diagnostic that cannot be
    /// written is added to the failure, which the foreground still prints;
    /// the client then reports the instruction to run in the foreground.
    fn record_start_error(&self, failure: Failure) -> Failure {
        match self.publish_start_error(&failure) {
            Ok(()) => failure,
            Err(error) => failure.reworded(|message| {
                format!(
                    "{message}; the startup diagnostic cannot be written to {}: {error}",
                    self.start_error.display()
                )
            }),
        }
    }

    /// Write `failure`'s code, a line break, and its message to a new
    /// owner-only staged copy, locked while it is written, and rename it into
    /// place, so a client reads a complete diagnostic or none. A copy that
    /// cannot be completed is removed; while another writer holds the staged
    /// copy, this one fails rather than wait. No failure before binding has a
    /// remedy the client could not give itself: the client finds a socket
    /// path that is too long before it starts a service, and restores a
    /// lock failure's runtime remedy from its own selection rule.
    fn publish_start_error(&self, failure: &Failure) -> io::Result<()> {
        let mut copy = self.stage_start_error()?;
        let text = format!("{}\n{}", failure.code().entry().name, failure.message());
        let published = copy
            .write_all(text.as_bytes())
            .and_then(|()| std::fs::rename(&self.staged_start_error, &self.start_error));
        let Err(error) = published else {
            return Ok(());
        };
        // This process holds the copy's lock, so the path still names it.
        match std::fs::remove_file(&self.staged_start_error) {
            Ok(()) => Err(error),
            Err(removal) => Err(io::Error::new(
                error.kind(),
                format!(
                    "{error}; its partial copy {} cannot be removed: {removal}",
                    self.staged_start_error.display()
                ),
            )),
        }
    }

    /// A new staged copy of the diagnostic, locked by this process, once an
    /// abandoned one is reclaimed. A reclaimer can take a new copy's lock in
    /// the moment before its creator does, and remove it; creating it once
    /// more is enough unless another process keeps doing so.
    fn stage_start_error(&self) -> io::Result<std::fs::File> {
        let staged = self.staged_start_error.display();
        for _ in 0..2 {
            let reclaimed = self
                .reclaim_staged_start_error()
                .map_err(|error| io::Error::new(error.kind(), format!("{staged}: {error}")))?;
            if !reclaimed {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!("another service is writing {staged}"),
                ));
            }
            let copy = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&self.staged_start_error)?;
            if lease::try_lock(&copy)? && names(&self.staged_start_error, &copy)? {
                return Ok(copy);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!("{staged} was taken by another process each time it was created"),
        ))
    }

    /// Take the lease, then create the session and bind the socket, or
    /// `None` when another process holds the lease.
    fn listen(
        &self,
        config: &Path,
    ) -> Result<Option<(lease::Lease, ProjectSession, UnixListener)>, Failure> {
        let Some(lease) =
            lease::Lease::acquire(&self.lock).map_err(|e| self.lock_error(&self.lock, e))?
        else {
            return Ok(None);
        };
        let session = ProjectSession::new(config)
            .map_err(|error| Failure::new(Code::Startup, error.to_string()))?;
        remove_socket(&self.socket)?;
        let listener =
            UnixListener::bind(&self.socket).map_err(|error| self.socket_error(error))?;
        Ok(Some((lease, session, listener)))
    }

    fn exchange(&self, mut stream: UnixStream, action: Action) -> Result<Response, Failure> {
        let unusable = |error| self.connection_failed(error);
        stream
            .set_write_timeout(Some(SOCKET_WAIT))
            .map_err(unusable)?;
        stream
            .set_read_timeout(Some(CLIENT_WAIT))
            .map_err(unusable)?;
        let request = Request {
            identity: Identity {
                config: self.identity.config.clone(),
                build: self.identity.build.clone(),
            },
            action,
        };
        let bytes = serde_json::to_vec(&request).map_err(|error| {
            Failure::new(
                Code::Protocol,
                format!("Project service request cannot be encoded: {error}"),
            )
        })?;
        if bytes.len() as u64 + 1 > REQUEST_LIMIT {
            return Err(Failure::new(
                Code::Protocol,
                "Project service request exceeds 1 MiB",
            ));
        }
        stream
            .write_all(&bytes)
            .and_then(|()| stream.write_all(b"\n"))
            .map_err(|error| {
                self.connection_error(
                    error,
                    &format!(
                        "did not take the request within {} seconds",
                        SOCKET_WAIT.as_secs()
                    ),
                )
            })?;
        serde_json::from_reader(BufReader::new(stream)).map_err(|error| self.unanswered(error))
    }

    fn wait_stopped(&self) -> Result<(), Failure> {
        let started = Instant::now();
        loop {
            if lease::Lease::acquire(&self.lock)
                .map_err(|error| self.lock_error(&self.lock, error))?
                .is_some()
            {
                return Ok(());
            }
            if started.elapsed() >= SOCKET_WAIT {
                return Err(Failure::new(
                    Code::Timeout,
                    "Project service did not release process ownership",
                ));
            }
            std::thread::sleep(POLL);
        }
    }

    /// A lock failure keeps its path and I/O cause, with the selected
    /// runtime directory's remedy. Directory metadata alone cannot prove
    /// that sandbox policy permits creating or opening its locks.
    fn lock_error(&self, path: &Path, error: io::Error) -> Failure {
        Failure::new(
            Code::Lock,
            format!("Project service lock {}: {error}", path.display()),
        )
        .with_remedy(self.rule.remedy())
    }
}

/// A runtime `directory` that cannot be created, with the cause, and
/// another directory, by the variables that choose one, as the remedy.
fn uncreatable(directory: &Path, rule: &Rule, error: io::Error) -> Failure {
    Failure::new(
        Code::Directory,
        format!(
            "Project service directory {} cannot be created: {error}",
            directory.display()
        ),
    )
    .with_remedy(rule.remedy())
}

/// Whether `Endpoint::prepare` could create `directory`, asked without
/// creating it: it creates only the directory itself, so its parent must be
/// a directory this user can write and search. The system answers with
/// `access(2)` on the parent's `.`, which fails as creating would: on a
/// missing parent, a parent that is not a directory, one this user cannot
/// write or search, or a read-only file system.
fn creatable(directory: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    /// `W_OK | X_OK`, the same on Linux, macOS, and the BSDs.
    const WRITE_AND_SEARCH: std::ffi::c_int = 2 | 1;
    unsafe extern "C" {
        fn access(path: *const std::ffi::c_char, mode: std::ffi::c_int) -> std::ffi::c_int;
    }
    let Some(parent) = directory.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "it has no parent directory",
        ));
    };
    let path = std::ffi::CString::new(parent.join(".").as_os_str().as_bytes())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    // SAFETY: a NUL-terminated path that lives across the call.
    if unsafe { access(path.as_ptr(), WRITE_AND_SEARCH) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// A runtime `directory` that cannot be inspected, with the cause, and
/// the remedy of one that cannot be created.
fn uninspected(directory: &Path, rule: &Rule, error: io::Error) -> Failure {
    Failure::new(
        Code::Directory,
        format!(
            "Project service directory {} cannot be inspected: {error}",
            directory.display()
        ),
    )
    .with_remedy(rule.remedy())
}

/// The runtime `directory` must be a directory owned by this user that
/// only its owner can use, and the owner must be able to list it, create
/// files in it, and open them: a service could not record why it failed
/// in a directory it cannot write, so the client must find that first.
/// The failure gives the remedy: `chmod 700` for this user's directory,
/// and for anything else another directory.
fn require_private(directory: &Path, metadata: &std::fs::Metadata) -> Result<(), Failure> {
    let mode = metadata.mode() & 0o777;
    let owned = metadata.is_dir() && metadata.uid() == lease::user_id();
    if owned && mode == PRIVATE_DIRECTORY {
        return Ok(());
    }
    let failure = Failure::new(
        Code::Directory,
        format!(
            "Project service directory must be a private directory owned by this user with mode 0700: {}",
            directory.display()
        ),
    );
    if !owned {
        return Err(failure.with_remedy(
            "set TSPARSE_SERVICE_DIR to a new path or to a private directory this user owns",
        ));
    }
    let failure = failure.with_why(if mode & 0o077 != 0 {
        "any process that can open the socket can run queries as you"
    } else {
        "a service could not create its socket or record why it failed"
    });
    // A path that is not Unicode is never rendered into a command, so
    // the remedy is said instead.
    Err(match directory.to_str() {
        Some(directory) => failure.with_next(vec![
            "chmod".into(),
            format!("{PRIVATE_DIRECTORY:o}"),
            directory.into(),
        ]),
        None => failure.with_remedy("give the runtime directory mode 0700"),
    })
}

/// Whether `error`, from inspecting or opening a path, says nothing is
/// there: the path is missing, or runs through a file. Only these are
/// absence; every other error is the failure it reports (AGENTS.md,
/// "Failures stay failures").
fn absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

/// `path` opened for reading when it names a regular file, or `None` when
/// nothing is there (`absent`). Another kind of file, such as a pipe an open
/// would wait on, is refused before it is opened (ENVELOPE.md, "Accepted
/// inputs"). A path replaced by a pipe between the check and the open can
/// block the open; ENVELOPE.md accepts this, since a hostile filesystem
/// layout is outside the envelope.
fn open_regular(path: &Path) -> io::Result<Option<std::fs::File>> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        // Refused whatever it names, so a link to a regular file says why.
        Ok(metadata) if metadata.is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a symbolic link, not a regular file",
            ));
        }
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        Err(error) if absent(&error) => return Ok(None),
        Err(error) => return Err(error),
    }
    match std::fs::File::open(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if absent(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

/// The failure a complete startup diagnostic records: its code, a line
/// break, and its message (`Endpoint::publish_start_error`).
fn recorded_failure(recorded: &str) -> io::Result<Failure> {
    recorded
        .split_once('\n')
        .and_then(|(name, message)| Some(Failure::new(Code::named(name)?, message)))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "it does not begin with a failure code",
            )
        })
}

/// Whether `path` names the file `copy` is open on.
fn names(path: &Path, copy: &std::fs::File) -> io::Result<bool> {
    let open = copy.metadata()?;
    match std::fs::symlink_metadata(path) {
        Ok(named) => Ok(named.dev() == open.dev() && named.ino() == open.ino()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// The failure an answer of another kind than the request expects
/// reports: the service's refusal, or a protocol failure.
fn unexpected(response: Response) -> Failure {
    match response {
        Response::Refused(refusal) => refusal.failure(),
        Response::Query(_) | Response::Status(_) | Response::Stopped(_) => {
            Failure::new(Code::Protocol, "Unexpected project service response")
        }
    }
}

/// `failure` as it leaves this module for a request about `config`: a
/// transport failure ends with the command that diagnoses it,
/// `run: tsparse --doctor --project CONFIG` (design, step 3). It is added
/// only here, where a failure leaves for its caller, so a startup
/// diagnostic a service records never carries it, and the doctor, which
/// reports these failures, observes them without it.
fn diagnosed(config: &str, failure: Failure) -> Failure {
    let doctor = [
        "tsparse",
        FlagId::Doctor.flag().name,
        FlagId::Project.flag().name,
        config,
    ];
    match failure.category() {
        Category::Transport => {
            failure.reworded(|message| format!("{message}; run: {}", command_line::render(&doctor)))
        }
        Category::Usage | Category::Input | Category::Write | Category::Internal => failure,
    }
}

/// Answer `request` from the service of the project `config` selects,
/// starting it when none is running. The service's working directory is
/// not the caller's, so a request's file operand must already be absolute
/// (`ProjectRequest::make_absolute`).
pub(crate) fn execute(config: &str, request: ProjectRequest) -> Result<ProjectResponse, Error> {
    let answer = || {
        let endpoint = Endpoint::new(Path::new(config))?;
        match endpoint.exchange(endpoint.start()?, Action::Query(request))? {
            Response::Query(Ok(response)) => Ok(response),
            Response::Query(Err(error)) => Err(Error::Analysis(error)),
            other => Err(unexpected(other).into()),
        }
    };
    answer().map_err(|error| match error {
        Error::Analysis(error) => Error::Analysis(error),
        Error::Unanswered(failure) => Error::Unanswered(diagnosed(config, failure)),
    })
}

/// The status of the project's service, which starts nothing.
pub(crate) fn status(config: &str) -> Result<Status, Failure> {
    observe(config).map_err(|failure| diagnosed(config, failure))
}

/// `status` as the doctor observes it, whose transport failures it reports
/// itself.
pub(crate) fn observe(config: &str) -> Result<Status, Failure> {
    let endpoint = Endpoint::new(Path::new(config))?;
    let Some(stream) = endpoint.connect()? else {
        return Ok(Status::Absent);
    };
    match endpoint.exchange(stream, Action::Status)? {
        Response::Status(running) => Ok(Status::Running(running)),
        other => Err(unexpected(other)),
    }
}

/// Stop the project's service, once it has released process ownership,
/// and say which one stopped, or that none was running.
pub(crate) fn stop(config: &str) -> Result<Stopped, Failure> {
    let stop = || {
        let endpoint = Endpoint::new(Path::new(config))?;
        let Some(stream) = endpoint.connect()? else {
            return Ok(Stopped::Absent);
        };
        match endpoint.exchange(stream, Action::Stop)? {
            Response::Stopped(pid) => endpoint.wait_stopped().map(|()| Stopped::Service(pid)),
            other => Err(unexpected(other)),
        }
    };
    stop().map_err(|failure| diagnosed(config, failure))
}

/// The configuration's absolute spelling, by which its service is keyed:
/// another spelling of the same file names another service.
pub(crate) fn absolute(config: &str) -> Result<PathBuf, Failure> {
    Ok(Identity::of(Path::new(config))?.config)
}

/// The runtime directory a project query would use, for the doctor: the
/// directory and the rule that chose it, and what observing it without
/// creating it found.
pub(crate) struct DirectoryInspection {
    /// The directory and the rule that chose it (`runtime::choose`), as
    /// `PATH (RULE)`.
    pub(crate) chosen: String,
    /// What `examine` found, or the failure a query would report there.
    pub(crate) found: Result<String, Failure>,
}

/// The runtime directory a project query would use, observed without
/// creating it, for the doctor (`DirectoryInspection`). A configuration
/// whose service cannot be named, or a directory that cannot be made
/// absolute, is the failure, as no directory is chosen.
pub(crate) fn inspect_directory(config: Option<&str>) -> Result<DirectoryInspection, Failure> {
    let (directory, rule) = match config {
        Some(config) => {
            let endpoint = Endpoint::new(Path::new(config))?;
            (endpoint.directory, endpoint.rule)
        }
        None => {
            let RuntimeDirectory { path, rule } = runtime::chosen()?;
            (path, rule)
        }
    };
    Ok(DirectoryInspection {
        chosen: format!("{} ({rule})", directory.display()),
        found: examine(&directory, &rule),
    })
}

/// Whether `directory`, which `rule` chose, exists as a private directory of
/// this user's (`require_private`) or could be created (`creatable`), and the
/// length of its socket paths beside the longest a socket address holds
/// (`SocketBudget`). A directory that does not exist passes when the first
/// query could create it; one that it could not, one that fails the check,
/// and socket paths that are too long are the failures a query would
/// report, each with its remedy.
fn examine(directory: &Path, rule: &Rule) -> Result<String, Failure> {
    let state = match std::fs::symlink_metadata(directory) {
        Ok(metadata) => {
            require_private(directory, &metadata)?;
            "is a private directory owned by this user with mode 0700"
        }
        Err(error) if absent(&error) => {
            creatable(directory).map_err(|error| uncreatable(directory, rule, error))?;
            "does not exist yet; a project query creates it with mode 0700"
        }
        Err(error) => return Err(uninspected(directory, rule, error)),
    };
    let budget = SocketBudget::of(directory);
    let SocketBudget { length, limit } = budget;
    if !budget.fits() {
        return Err(Failure::new(
            Code::Socket,
            format!(
                "socket paths in {} take {length} bytes, more than the {limit} a socket address holds",
                directory.display()
            ),
        )
        .with_remedy(SHORTER_DIRECTORY));
    }
    Ok(format!(
        "{state}; its socket paths take {length} of at most {limit} bytes"
    ))
}

/// What an earlier start left in the runtime directory for `config`'s
/// service, which the next start removes, for the doctor: the failure a
/// service recorded that no client read, such as a foreground service's,
/// and a partial copy of a diagnostic, whose writer may still hold it.
/// Neither is removed, and the copy is not read; a diagnostic that cannot
/// be read is the failure.
pub(crate) fn leftovers(config: &str) -> Result<Vec<String>, Failure> {
    let endpoint = Endpoint::new(Path::new(config))?;
    let unreadable = |path: &Path, error: io::Error| {
        Failure::new(
            Code::Startup,
            format!(
                "the startup diagnostic {} cannot be read: {error}",
                path.display()
            ),
        )
    };
    let mut left = Vec::new();
    let recorded = open_regular(&endpoint.start_error)
        .and_then(|published| published.map(io::read_to_string).transpose())
        .map_err(|error| unreadable(&endpoint.start_error, error))?;
    if let Some(recorded) = recorded {
        let failure = recorded_failure(&recorded)
            .map_err(|error| unreadable(&endpoint.start_error, error))?;
        left.push(format!(
            "the last start that failed recorded {}",
            failure.text()
        ));
    }
    match std::fs::symlink_metadata(&endpoint.staged_start_error) {
        Ok(_) => left.push(format!(
            "a partial startup diagnostic remains at {}, which the next start removes",
            endpoint.staged_start_error.display()
        )),
        Err(error) if absent(&error) => {}
        Err(error) => return Err(unreadable(&endpoint.staged_start_error, error)),
    }
    Ok(left)
}

/// The remedy for a runtime directory whose socket paths are too long.
const SHORTER_DIRECTORY: &str = "set a shorter TSPARSE_SERVICE_DIR";

/// The longest path a socket address holds on this platform, as
/// `SocketAddr::from_pathname`, which binding and connecting use, decides
/// it: one byte less than the address's path field, which ends with a NUL.
fn socket_path_limit() -> usize {
    let fits =
        |length: usize| SocketAddr::from_pathname(format!("/{}", "x".repeat(length - 1))).is_ok();
    // `/` fits every address; no platform's address holds a page.
    let (mut fitting, mut over) = (1, 4096);
    while over - fitting > 1 {
        let middle = (fitting + over) / 2;
        if fits(middle) {
            fitting = middle;
        } else {
            over = middle;
        }
    }
    fitting
}

fn remove_socket(path: &Path) -> Result<(), Failure> {
    let failure = |reason: String| {
        Failure::new(
            Code::Socket,
            format!("Project service socket {}{reason}", path.display()),
        )
    };
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => std::fs::remove_file(path)
            .map_err(|error| failure(format!(" cannot be removed: {error}"))),
        Ok(_) => Err(Failure::new(
            Code::Socket,
            format!(
                "Service socket path names another kind of file: {}",
                path.display()
            ),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failure(format!(" cannot be inspected: {error}"))),
    }
}

/// Serve the project `config` selects until it is idle for `environment`'s
/// idle time, answering queries under its limits. A failure before the
/// socket is bound is also recorded in the runtime directory, for a client
/// that started this service with null standard streams.
pub(crate) fn serve(config: &str, environment: &Environment) -> Result<(), Failure> {
    serve_until_idle(Path::new(config), environment).map_err(|failure| diagnosed(config, failure))
}

fn serve_until_idle(config: &Path, environment: &Environment) -> Result<(), Failure> {
    close_inherited_descriptors().map_err(|error| {
        Failure::new(
            Code::Startup,
            format!("Project service cannot close the descriptors it inherited: {error}"),
        )
    })?;
    let endpoint = Endpoint::new(config)?;
    // A directory that fails the check is not trusted with a diagnostic.
    endpoint.prepare()?;
    let (_lease, mut session, listener) = match endpoint.listen(config) {
        Ok(Some(listening)) => listening,
        Ok(None) => return Ok(()),
        Err(failure) => return Err(endpoint.record_start_error(failure)),
    };
    let idle = environment.idle();
    // A connection is handled as soon as it arrives, and the idle limit
    // passes without polling: one thread blocks in `accept` and hands each
    // connection over a rendezvous channel. While a request is handled it
    // holds at most one more accepted connection; other clients wait in the
    // listen backlog. It stays blocked until the process, which `serve`
    // ends, exits.
    let (connections, accepted) = std::sync::mpsc::sync_channel(0);
    std::thread::Builder::new()
        .name("tsparse-accept".into())
        .spawn(move || {
            for connection in listener.incoming() {
                if connections.send(connection).is_err() {
                    return;
                }
            }
        })
        .map_err(|error| {
            Failure::new(
                Code::Startup,
                format!("Project service cannot start accepting connections: {error}"),
            )
        })?;
    let started = unix_seconds(SystemTime::now());
    let mut last_request = Instant::now();
    let result = loop {
        match accepted.recv_timeout(idle.saturating_sub(last_request.elapsed())) {
            Ok(Ok(stream)) => {
                let service = Serving {
                    identity: &endpoint.identity,
                    environment,
                    started,
                    last_request,
                };
                // A malformed, stalled, or disconnected client must not kill the
                // project's service. The client receives the transport failure.
                // Status is administrative, so it leaves the idle time running.
                match handle(stream, &service, &mut session) {
                    Ok(Handled::Stop) => break Ok(()),
                    Ok(Handled::Observed) => {}
                    Ok(Handled::Served) | Err(_) => last_request = Instant::now(),
                }
            }
            Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => {}
            Ok(Err(error)) => {
                break Err(Failure::new(
                    Code::Socket,
                    format!(
                        "Project service socket {} cannot accept connections: {error}",
                        endpoint.socket.display()
                    ),
                ));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if last_request.elapsed() >= idle {
                    break Ok(());
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                unreachable!("the accept thread only ends when the service stops receiving")
            }
        }
    };
    let cleanup = remove_socket(&endpoint.socket);
    result.and(cleanup)
}

/// Close every descriptor this process inherited beyond the standard
/// streams, before the service opens its own. A service outlives the client
/// that started it, and a descriptor that client inherited by accident, such
/// as a pipe its parent was still marking close-on-exec when another thread
/// spawned it (std on macOS creates a pipe and marks it in two steps), would
/// keep that pipe, and whoever waits for its end of file, open until the
/// service exits.
fn close_inherited_descriptors() -> io::Result<()> {
    /// `EBADF`, the same on Linux, macOS, and the BSDs.
    const NOT_OPEN: i32 = 9;
    unsafe extern "C" {
        fn close(descriptor: std::ffi::c_int) -> std::ffi::c_int;
    }
    let mut inherited = Vec::new();
    for entry in std::fs::read_dir("/dev/fd")? {
        let name = entry?.file_name();
        if let Some(descriptor) = name.to_str().and_then(|name| name.parse::<i32>().ok()) {
            if descriptor > 2 {
                inherited.push(descriptor);
            }
        }
    }
    // The listing's own descriptor is among them, closed with the listing.
    for descriptor in inherited {
        // SAFETY: nothing in this process owns a descriptor above the
        // standard streams yet; `serve` runs before the service opens any.
        if unsafe { close(descriptor) } != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(NOT_OPEN) {
                return Err(error);
            }
        }
    }
    Ok(())
}

/// What a service answers a connection with besides its session.
struct Serving<'a> {
    identity: &'a Identity,
    environment: &'a Environment,
    /// When it started listening, in seconds since the Unix epoch.
    started: i64,
    /// When it last answered a request other than status.
    last_request: Instant,
}

/// What a service does after answering a connection.
enum Handled {
    /// It answered a request, which restarts its idle time.
    Served,
    /// It reported its status, which leaves its idle time running.
    Observed,
    /// It exits.
    Stop,
}

fn handle(
    mut stream: UnixStream,
    service: &Serving,
    session: &mut ProjectSession,
) -> io::Result<Handled> {
    let Serving {
        identity,
        environment,
        ..
    } = *service;
    // On BSD/macOS accepted sockets can inherit the listener's O_NONBLOCK.
    // A client may connect before writing; that gap is not a broken request.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(SOCKET_WAIT))?;
    stream.set_write_timeout(Some(SOCKET_WAIT))?;
    let mut line = Vec::new();
    BufReader::new((&mut stream).take(REQUEST_LIMIT)).read_until(b'\n', &mut line)?;
    let request = if line.last() != Some(&b'\n') {
        Err("Incomplete or oversized project service request".to_owned())
    } else {
        serde_json::from_slice::<Request>(&line).map_err(|e| e.to_string())
    };
    let (response, handled) = match request {
        Err(message) => (
            Response::Refused(Refusal::Unreadable(message)),
            Handled::Served,
        ),
        Ok(request) if request.identity != *identity => {
            (Response::Refused(Refusal::Identity), Handled::Served)
        }
        Ok(Request { action, .. }) => match action {
            Action::Query(request) => {
                let result = crate::process_limits::run(
                    environment,
                    |failure| {
                        // The query thread has not begun response delivery. A
                        // limit response is the process's final socket write.
                        let mut writer = BufWriter::new(&stream);
                        let error = failure.into_error(&environment.limits(), Context::Service);
                        // The process exits as the query it answers does.
                        let code = delivery::failure_code(&error);
                        let response = Response::Query(Err(error));
                        if serde_json::to_writer(&mut writer, &response).is_ok() {
                            let _ = writer.flush();
                        }
                        code.into()
                    },
                    || session.execute(request),
                );
                match result {
                    Ok(result) => (Response::Query(result), Handled::Served),
                    // Preserve a watchdog that could not run instead of
                    // closing the socket with no response. Such a process is
                    // disposable.
                    Err(error) => (
                        Response::Refused(Refusal::Watchdog(error.to_string())),
                        Handled::Stop,
                    ),
                }
            }
            Action::Status => {
                let idle = environment.idle();
                (
                    Response::Status(Running {
                        pid: std::process::id(),
                        generation: session.generation(),
                        build: identity.build.clone(),
                        limits: environment.limits(),
                        idle_secs: idle.as_secs(),
                        idle_remaining_secs: idle
                            .saturating_sub(service.last_request.elapsed())
                            .as_secs(),
                        started_unix: service.started,
                    }),
                    Handled::Observed,
                )
            }
            Action::Stop => (Response::Stopped(std::process::id()), Handled::Stop),
        },
    };
    let mut writer = BufWriter::new(stream);
    serde_json::to_writer(&mut writer, &response)?;
    writer.flush()?;
    Ok(handled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// An endpoint in a new private directory named for `test`, and the
    /// directory, for the diagnostic files' states the real binary reaches
    /// only by chance; tests/service_startup.rs follows the rest through it.
    fn endpoint(test: &str) -> (Endpoint, PathBuf) {
        let directory =
            std::env::temp_dir().join(format!("tsparse-service-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir(&directory).unwrap();
        let endpoint =
            Endpoint::in_directory(Path::new("/project/tsconfig.json"), directory.clone()).unwrap();
        (endpoint, directory)
    }

    /// A start that recorded nothing, for the endpoint `endpoint` names.
    const UNRECORDED: &str = "transport.startup: Project service startup failed without recording why; in the foreground it prints the cause\n  next: tsparse --references-serve --project /project/tsconfig.json";

    fn entries(directory: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect()
    }

    /// An endpoint's files are keyed by its whole identity: the same
    /// configuration spelling and build name the same socket, and another
    /// spelling of the same file, or another build, names another.
    #[test]
    fn each_spelling_and_build_names_its_own_endpoint() {
        let socket = |config: &str, build: &str| {
            Endpoint::named(
                Identity {
                    config: PathBuf::from(config),
                    build: build.into(),
                },
                RuntimeDirectory {
                    path: PathBuf::from("/runtime"),
                    rule: Rule::Variable,
                },
            )
            .socket
        };
        let this = socket("/project/tsconfig.json", "one");
        assert_eq!(this, socket("/project/tsconfig.json", "one"));
        assert_ne!(this, socket("/project/link.json", "one"));
        assert_ne!(this, socket("/project/./sub/../tsconfig.json", "one"));
        assert_ne!(this, socket("/project/tsconfig.json", "two"));
    }

    /// Its next command runs the service the client started, as it
    /// started it: for the configuration's absolute spelling.
    #[test]
    fn a_start_that_recorded_nothing_names_the_foreground_service() {
        let (endpoint, directory) = endpoint("unrecorded");
        let failure = endpoint.start_failure();
        std::fs::remove_dir_all(&directory).unwrap();
        assert_eq!(failure.text(), UNRECORDED);
        assert_eq!(
            failure.error()["next"],
            serde_json::json!({"argv": [
                "tsparse", "--references-serve", "--project", "/project/tsconfig.json"
            ]})
        );
    }

    /// A configuration whose path is not Unicode is never rendered into a
    /// command, so the remedy is said instead.
    #[test]
    fn a_start_that_recorded_nothing_for_a_path_that_is_not_unicode_says_the_remedy() {
        use std::os::unix::ffi::OsStrExt;
        let directory = std::env::temp_dir().join(format!(
            "tsparse-service-{}-unrecorded-bytes",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir(&directory).unwrap();
        let config = std::ffi::OsStr::from_bytes(b"/project/\xff.json");
        let endpoint = Endpoint::in_directory(Path::new(config), directory.clone()).unwrap();
        let failure = endpoint.start_failure();
        std::fs::remove_dir_all(&directory).unwrap();
        assert_eq!(failure.next(), None);
        assert_eq!(
            failure.text(),
            "transport.startup: Project service startup failed without recording why; in the foreground it prints the cause\n  next: run --references-serve with --project naming this configuration"
        );
    }

    #[test]
    fn a_recorded_diagnostic_is_the_failure_with_its_code_and_is_removed() {
        let (endpoint, directory) = endpoint("recorded");
        let recorded = endpoint.record_start_error(Failure::new(Code::Lock, "the first cause"));
        assert_eq!(recorded.text(), "transport.lock: the first cause");
        assert_eq!(
            std::fs::read_to_string(&endpoint.start_error).unwrap(),
            "transport.lock\nthe first cause"
        );
        // A later one replaces it with a new owner-only file.
        std::fs::set_permissions(
            &endpoint.start_error,
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        endpoint.record_start_error(Failure::new(Code::Socket, "a cause\nover two lines"));
        let mode = std::fs::metadata(&endpoint.start_error)
            .unwrap()
            .permissions()
            .mode();
        let failure = endpoint.start_failure();
        let left = entries(&directory);
        std::fs::remove_dir_all(&directory).unwrap();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(failure.text(), "transport.socket: a cause\nover two lines");
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn a_diagnostic_that_names_no_code_is_reported_as_unreadable_and_removed() {
        let (endpoint, directory) = endpoint("uncoded");
        let mut failures = Vec::new();
        for recorded in ["a cause", "transport.nothing\na cause", ""] {
            std::fs::write(&endpoint.start_error, recorded).unwrap();
            failures.push(endpoint.start_failure());
        }
        let left = entries(&directory);
        std::fs::remove_dir_all(&directory).unwrap();
        for failure in failures {
            assert_eq!(
                failure.text(),
                format!(
                    "transport.startup: Project service startup failed, and its diagnostic {} cannot be read: it does not begin with a failure code",
                    endpoint.start_error.display()
                )
            );
        }
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn an_abandoned_partial_copy_is_reclaimed_and_never_reported() {
        let (endpoint, directory) = endpoint("abandoned");
        // As a service killed while writing its diagnostic leaves it.
        std::fs::write(&endpoint.staged_start_error, "transport.lo").unwrap();
        let failure = endpoint.start_failure();
        let left = entries(&directory);
        // A writer reclaims one too.
        std::fs::write(&endpoint.staged_start_error, "transport.lo").unwrap();
        let recorded = endpoint.record_start_error(Failure::new(Code::Lock, "cause"));
        let published = std::fs::read_to_string(&endpoint.start_error).unwrap();
        let staged = endpoint.staged_start_error.exists();
        std::fs::remove_dir_all(&directory).unwrap();
        assert_eq!(failure.text(), UNRECORDED);
        assert!(left.is_empty(), "{left:?}");
        assert_eq!(
            (recorded.text(), published),
            (
                "transport.lock: cause".into(),
                "transport.lock\ncause".into()
            )
        );
        assert!(!staged);
    }

    #[test]
    fn a_partial_copy_its_writer_holds_is_left_to_it() {
        let (endpoint, directory) = endpoint("held");
        let writer = std::fs::File::create(&endpoint.staged_start_error).unwrap();
        assert!(lease::try_lock(&writer).unwrap());
        let failure = endpoint.start_failure();
        let recorded = endpoint.record_start_error(Failure::new(Code::Lock, "the cause"));
        let held = (
            endpoint.staged_start_error.exists(),
            endpoint.start_error.exists(),
        );
        drop(writer);
        // A process another test spawns at this moment can hold a copy of the
        // writer's descriptor, and so its lock, until it executes.
        let waiting = std::time::Instant::now();
        let reclaimed = loop {
            let reclaimed = endpoint.start_failure();
            if !endpoint.staged_start_error.exists()
                || waiting.elapsed() > std::time::Duration::from_secs(10)
            {
                break reclaimed;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        let left = entries(&directory);
        std::fs::remove_dir_all(&directory).unwrap();
        assert_eq!(failure.text(), UNRECORDED);
        assert_eq!(
            recorded.text(),
            format!(
                "transport.lock: the cause; the startup diagnostic cannot be written to {}: another service is writing {}",
                endpoint.start_error.display(),
                endpoint.staged_start_error.display()
            )
        );
        assert_eq!(held, (true, false));
        assert_eq!(reclaimed.text(), UNRECORDED);
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn a_diagnostic_that_is_not_a_regular_file_is_reported_without_waiting() {
        let (endpoint, directory) = endpoint("unreadable");
        let expected = format!(
            "transport.startup: Project service startup failed, and its diagnostic {} cannot be read: not a regular file",
            endpoint.start_error.display()
        );
        std::fs::create_dir(&endpoint.start_error).unwrap();
        let directory_failure = endpoint.start_failure();
        std::fs::remove_dir(&endpoint.start_error).unwrap();
        // A pipe with no writer, which a blocking open would wait on.
        let made = std::process::Command::new("mkfifo")
            .arg(&endpoint.start_error)
            .status()
            .unwrap();
        let pipe_failure = endpoint.start_failure();
        let kept = std::fs::symlink_metadata(&endpoint.start_error).map(|m| m.file_type());
        std::fs::remove_dir_all(&directory).unwrap();
        assert!(made.success());
        assert_eq!(directory_failure.text(), expected);
        assert_eq!(pipe_failure.text(), expected);
        assert!(kept.unwrap().is_fifo());
    }

    #[test]
    fn a_diagnostic_that_cannot_be_published_is_added_to_the_failure_and_removed() {
        let (endpoint, directory) = endpoint("unpublished");
        std::fs::create_dir(&endpoint.start_error).unwrap();
        let file = directory.join("file");
        std::fs::write(&file, "").unwrap();
        let cause = std::fs::rename(&file, &endpoint.start_error).unwrap_err();
        std::fs::remove_file(&file).unwrap();
        let failure = endpoint.record_start_error(Failure::new(Code::Lock, "the cause"));
        let staged = endpoint.staged_start_error.exists();
        std::fs::remove_dir_all(&directory).unwrap();
        assert_eq!(
            failure.text(),
            format!(
                "transport.lock: the cause; the startup diagnostic cannot be written to {}: {cause}",
                endpoint.start_error.display()
            )
        );
        assert!(!staged);
    }

    /// What a service answers `request`, the whole of what a connection
    /// sends it.
    fn answered(endpoint: &Endpoint, request: &[u8]) -> Failure {
        let (client, server) = UnixStream::pair().unwrap();
        (&client).write_all(request).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut session = ProjectSession::new(&endpoint.identity.config).unwrap();
        let environment = Environment::read().unwrap();
        let service = Serving {
            identity: &endpoint.identity,
            environment: &environment,
            started: 0,
            last_request: Instant::now(),
        };
        let handled = handle(server, &service, &mut session).unwrap();
        assert!(matches!(handled, Handled::Served));
        let response: Response = serde_json::from_reader(&client).unwrap();
        unexpected(response)
    }

    #[test]
    fn a_service_refuses_a_request_of_another_identity_or_none_with_its_own_code() {
        let (endpoint, directory) = endpoint("refused");
        std::fs::remove_dir_all(&directory).unwrap();
        // A client of another configuration whose endpoint key collides.
        let mut request = serde_json::to_vec(&Request {
            identity: Identity {
                config: "/another/tsconfig.json".into(),
                build: endpoint.identity.build.clone(),
            },
            action: Action::Status,
        })
        .unwrap();
        request.push(b'\n');
        assert_eq!(
            answered(&endpoint, &request).text(),
            "transport.identity: Project service identity mismatch"
        );
        assert_eq!(
            answered(&endpoint, b"{\"identity\"").text(),
            "transport.protocol: Incomplete or oversized project service request"
        );
        let Err(unread) = serde_json::from_slice::<Request>(b"7\n") else {
            panic!("a number is not a request");
        };
        assert_eq!(
            answered(&endpoint, b"7\n").text(),
            format!("transport.protocol: {unread}")
        );
    }

    /// The client sends a request whose line, with its line break, is at
    /// most the protocol's limit, the size a service reads, and refuses a
    /// larger one as `transport.protocol` before sending anything.
    #[test]
    fn a_request_is_sent_up_to_the_protocol_limit_and_refused_over_it() {
        let (endpoint, directory) = endpoint("limit");
        std::fs::remove_dir_all(&directory).unwrap();
        let outline = |length: usize| {
            Action::Query(ProjectRequest::Outline {
                file: PathBuf::from("x".repeat(length)),
            })
        };
        let line = |length| {
            let request = Request {
                identity: Identity {
                    config: endpoint.identity.config.clone(),
                    build: endpoint.identity.build.clone(),
                },
                action: outline(length),
            };
            serde_json::to_vec(&request).unwrap().len() as u64 + 1
        };
        // The file name's length whose request line is exactly the limit.
        let at = (REQUEST_LIMIT - line(0)) as usize;
        assert_eq!(line(at), REQUEST_LIMIT);
        for length in [at - 1, at, at + 1] {
            let (client, server) = UnixStream::pair().unwrap();
            // The service's side reads one line, as `handle` does, answers
            // one it received, and closes.
            let service = std::thread::spawn(move || {
                let mut received = Vec::new();
                BufReader::new((&server).take(REQUEST_LIMIT))
                    .read_until(b'\n', &mut received)
                    .unwrap();
                if !received.is_empty() {
                    serde_json::to_writer(&server, &Response::Stopped(1)).unwrap();
                }
                received.len() as u64
            });
            let answer = endpoint.exchange(client, outline(length));
            let received = service.join().unwrap();
            match answer {
                Ok(Response::Stopped(_)) if length <= at => {
                    assert_eq!(received, line(length), "{length}");
                }
                Err(failure) if length > at => {
                    assert_eq!(
                        failure.text(),
                        "transport.protocol: Project service request exceeds 1 MiB"
                    );
                    assert_eq!(received, 0, "{length}");
                }
                Ok(_) => panic!("{length}: an answer"),
                Err(failure) => panic!("{length}: {}", failure.text()),
            }
        }
    }

    /// A reader whose wait for an answer runs out.
    struct Stalled;

    impl Read for Stalled {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::WouldBlock.into())
        }
    }

    #[test]
    fn an_answer_that_never_arrives_or_cannot_be_decoded_keeps_its_cause() {
        let (endpoint, directory) = endpoint("unanswered");
        std::fs::remove_dir_all(&directory).unwrap();
        let socket = endpoint.socket.display();
        let failure = |error| endpoint.unanswered(error).text();
        assert_eq!(
            failure(serde_json::from_reader::<_, serde_json::Value>(Stalled).unwrap_err()),
            "transport.timeout: Project service did not answer within 300 seconds"
        );
        assert_eq!(
            failure(serde_json::from_slice::<serde_json::Value>(b"").unwrap_err()),
            format!(
                "transport.socket: Project service connection {socket} closed before an answer"
            )
        );
        assert_eq!(
            failure(serde_json::from_slice::<serde_json::Value>(b"x").unwrap_err()),
            "transport.protocol: Project service answer cannot be decoded: expected value at line 1 column 1"
        );
        let reset = io::Error::from(io::ErrorKind::ConnectionReset);
        assert_eq!(
            endpoint
                .connection_error(reset, "did not take the request within 5 seconds")
                .text(),
            format!(
                "transport.socket: Project service connection {socket}: {}",
                io::Error::from(io::ErrorKind::ConnectionReset)
            )
        );
    }

    #[test]
    fn seconds_name_each_unit_that_is_not_zero() {
        for (total, text) in [
            (0, "0s"),
            (1, "1s"),
            (59, "59s"),
            (60, "1m"),
            (432, "7m12s"),
            (600, "10m"),
            (3600, "1h"),
            (3601, "1h1s"),
            (3661, "1h1m1s"),
            (90_000, "25h"),
        ] {
            assert_eq!(seconds(total), text, "{total}");
        }
        assert_eq!(unix_seconds(SystemTime::UNIX_EPOCH), 0);
        assert_eq!(
            unix_seconds(SystemTime::UNIX_EPOCH - Duration::from_secs(5)),
            -5
        );
    }

    /// A runtime directory that is not Unicode cannot be named in a command:
    /// its failure says the remedy instead of giving it as `next`.
    #[test]
    fn a_directory_that_is_not_unicode_is_never_rendered_into_a_command() {
        use std::os::unix::ffi::OsStrExt;
        let (endpoint, directory) = endpoint("open");
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        let metadata = std::fs::symlink_metadata(&directory).unwrap();
        std::fs::remove_dir_all(&directory).unwrap();
        let unnamed = Endpoint::in_directory(
            &endpoint.identity.config,
            PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/tsparse-\xff")),
        )
        .unwrap();
        let failure = require_private(&unnamed.directory, &metadata).unwrap_err();
        assert_eq!(
            failure.text(),
            "transport.directory: Project service directory must be a private directory owned by this user with mode 0700: /tmp/tsparse-\u{FFFD}\n  why: any process that can open the socket can run queries as you\n  next: give the runtime directory mode 0700"
        );
        assert_eq!(
            require_private(&endpoint.directory, &metadata)
                .unwrap_err()
                .error()["next"],
            serde_json::json!({"argv": ["chmod", "700", endpoint.directory.to_str().unwrap()]})
        );
    }
}
