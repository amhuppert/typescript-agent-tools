//! Local process reuse for CLI project queries: references, outline, and
//! show. One service per configuration keeps a `ProjectSession`, whose one
//! request boundary refreshes the generation before every operation. The
//! wire format, rendezvous, startup races, and shutdown belong here; callers
//! supply complete operations.

mod lease;

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use typescript_parser::analysis::AnalysisError;
use typescript_parser::project::{ProjectRequest, ProjectResponse, ProjectSession};

const STARTUP_WAIT: Duration = Duration::from_secs(10);
const CLIENT_WAIT: Duration = Duration::from_secs(300);
const SOCKET_WAIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);
const REQUEST_LIMIT: u64 = 1 << 20;

#[derive(Debug)]
pub enum Error {
    Analysis(AnalysisError),
    Transport(io::Error),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Transport(error)
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
    Status(Status),
    Stopped,
    Failure(String),
}

/// Administrative observation only: generation counts published snapshots,
/// and status itself neither loads a project nor validates its freshness.
#[derive(Serialize, Deserialize)]
pub struct Status {
    pub running: bool,
    pub pid: Option<u32>,
    pub generation: u64,
}

struct Endpoint {
    directory: PathBuf,
    socket: PathBuf,
    lock: PathBuf,
    identity: Identity,
}

impl Endpoint {
    fn new(config: &Path) -> io::Result<Self> {
        let mut hash = DefaultHasher::new();
        let identity = Identity {
            config: std::path::absolute(config)?,
            build: env!("TSPARSE_SERVICE_BUILD").into(),
        };
        identity.build.hash(&mut hash);
        identity.config.hash(&mut hash);
        let key = format!("{:016x}", hash.finish());
        let directory = std::env::var_os("TSPARSE_SERVICE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("/tmp/tsparse-{}", lease::user_id())));
        let directory = std::path::absolute(directory)?;
        Ok(Self {
            socket: directory.join(format!("{key}.sock")),
            lock: directory.join(format!("{key}.lock")),
            directory,
            identity,
        })
    }

    fn prepare(&self) -> io::Result<()> {
        match std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&self.directory)
        {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        self.check_directory()
    }

    fn check_directory(&self) -> io::Result<()> {
        let metadata = std::fs::symlink_metadata(&self.directory)?;
        if !metadata.is_dir() || metadata.uid() != lease::user_id() || metadata.mode() & 0o077 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "Reference service directory must be a private directory owned by this user: {}",
                    self.directory.display()
                ),
            ));
        }
        Ok(())
    }

    fn connect(&self) -> io::Result<Option<UnixStream>> {
        match self.check_directory() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
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
    /// makes every request fail with `InvalidInput`.
    fn socket_error(&self, error: io::Error) -> io::Error {
        let hint = if error.kind() == io::ErrorKind::InvalidInput {
            "; set a shorter TSPARSE_SERVICE_DIR"
        } else {
            ""
        };
        io::Error::new(
            error.kind(),
            format!(
                "Reference service socket {}: {error}{hint}",
                self.socket.display()
            ),
        )
    }

    fn start(&self) -> io::Result<UnixStream> {
        if let Some(stream) = self.connect()? {
            return Ok(stream);
        }
        self.prepare()?;
        let mut child = Command::new(std::env::current_exe()?)
            .arg("--references-serve")
            .arg("--project")
            .arg(&self.identity.config)
            .env("TSPARSE_SERVICE_DIR", &self.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()?;
        let started = Instant::now();
        loop {
            if let Some(stream) = self.connect()? {
                return Ok(stream);
            }
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    return Err(io::Error::other(
                        "Reference service startup failed; run --references-serve --project CONFIG to see its diagnostic",
                    ));
                }
                // Another simultaneous launcher may hold the lease before bind.
            }
            if started.elapsed() >= STARTUP_WAIT {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Reference service did not become ready within 10 seconds",
                ));
            }
            std::thread::sleep(POLL);
        }
    }

    fn exchange(&self, mut stream: UnixStream, action: Action) -> io::Result<Response> {
        stream.set_write_timeout(Some(SOCKET_WAIT))?;
        stream.set_read_timeout(Some(CLIENT_WAIT))?;
        let request = Request {
            identity: Identity {
                config: self.identity.config.clone(),
                build: self.identity.build.clone(),
            },
            action,
        };
        let bytes = serde_json::to_vec(&request)?;
        if bytes.len() as u64 + 1 > REQUEST_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Reference service request exceeds 1 MiB",
            ));
        }
        stream.write_all(&bytes)?;
        stream.write_all(b"\n")?;
        serde_json::from_reader(BufReader::new(stream)).map_err(io::Error::other)
    }

    fn wait_stopped(&self) -> io::Result<()> {
        let started = Instant::now();
        loop {
            if lease::Lease::acquire(&self.lock)?.is_some() {
                return Ok(());
            }
            if started.elapsed() >= SOCKET_WAIT {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Reference service did not release process ownership",
                ));
            }
            std::thread::sleep(POLL);
        }
    }
}

fn unexpected(response: Response) -> io::Error {
    match response {
        Response::Failure(message) => io::Error::other(message),
        _ => io::Error::new(
            io::ErrorKind::InvalidData,
            "Unexpected reference service response",
        ),
    }
}

/// Answer `request` from the service of the project `config` selects,
/// starting it when none is running. The service's working directory is
/// not the caller's, so a request's file operand must already be absolute
/// (`ProjectRequest::make_absolute`).
pub fn execute(config: &Path, request: ProjectRequest) -> Result<ProjectResponse, Error> {
    let endpoint = Endpoint::new(config)?;
    match endpoint.exchange(endpoint.start()?, Action::Query(request))? {
        Response::Query(Ok(response)) => Ok(response),
        Response::Query(Err(error)) => Err(Error::Analysis(error)),
        other => Err(unexpected(other).into()),
    }
}

pub fn status(config: &Path) -> io::Result<Status> {
    let endpoint = Endpoint::new(config)?;
    let Some(stream) = endpoint.connect()? else {
        return Ok(Status {
            running: false,
            pid: None,
            generation: 0,
        });
    };
    match endpoint.exchange(stream, Action::Status)? {
        Response::Status(status) => Ok(status),
        other => Err(unexpected(other)),
    }
}

pub fn stop(config: &Path) -> io::Result<()> {
    let endpoint = Endpoint::new(config)?;
    let Some(stream) = endpoint.connect()? else {
        return Ok(());
    };
    match endpoint.exchange(stream, Action::Stop)? {
        Response::Stopped => endpoint.wait_stopped(),
        other => Err(unexpected(other)),
    }
}

fn remove_socket(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => std::fs::remove_file(path),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "Service socket path names another kind of file: {}",
                path.display()
            ),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn serve(config: &Path) -> io::Result<()> {
    close_inherited_descriptors()?;
    let endpoint = Endpoint::new(config)?;
    endpoint.prepare()?;
    let Some(_lease) = lease::Lease::acquire(&endpoint.lock)? else {
        return Ok(());
    };
    remove_socket(&endpoint.socket)?;
    let listener =
        UnixListener::bind(&endpoint.socket).map_err(|error| endpoint.socket_error(error))?;
    let mut session = ProjectSession::new(config).map_err(io::Error::other)?;
    let idle = match std::env::var("TSPARSE_SERVICE_IDLE_SECS") {
        Ok(value) => Duration::from_secs(value.parse::<u64>().ok().filter(|&s| s > 0).ok_or_else(
            || {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "TSPARSE_SERVICE_IDLE_SECS must be a positive integer",
                )
            },
        )?),
        Err(std::env::VarError::NotPresent) => Duration::from_secs(600),
        Err(error) => return Err(io::Error::new(io::ErrorKind::InvalidInput, error)),
    };
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
        })?;
    let mut last_request = Instant::now();
    let result = loop {
        match accepted.recv_timeout(idle.saturating_sub(last_request.elapsed())) {
            Ok(Ok(stream)) => {
                // A malformed, stalled, or disconnected client must not kill the
                // project's service. The client receives the transport failure.
                match handle(stream, &endpoint.identity, &mut session) {
                    Ok(true) => break Ok(()),
                    Ok(false) | Err(_) => last_request = Instant::now(),
                }
            }
            Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => {}
            Ok(Err(error)) => break Err(error),
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

fn handle(
    mut stream: UnixStream,
    identity: &Identity,
    session: &mut ProjectSession,
) -> io::Result<bool> {
    // On BSD/macOS accepted sockets can inherit the listener's O_NONBLOCK.
    // A client may connect before writing; that gap is not a broken request.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(SOCKET_WAIT))?;
    stream.set_write_timeout(Some(SOCKET_WAIT))?;
    let mut line = Vec::new();
    BufReader::new((&mut stream).take(REQUEST_LIMIT)).read_until(b'\n', &mut line)?;
    let request = if line.last() != Some(&b'\n') {
        Err("Incomplete or oversized reference service request".to_owned())
    } else {
        serde_json::from_slice::<Request>(&line).map_err(|e| e.to_string())
    };
    let (response, stop) = match request {
        Err(message) => (Response::Failure(message), false),
        Ok(request) if request.identity != *identity => (
            Response::Failure("Reference service identity mismatch".into()),
            false,
        ),
        Ok(Request { action, .. }) => match action {
            Action::Query(request) => {
                let result = crate::process_limits::run(
                    |failure| {
                        // The query thread has not begun response delivery. A
                        // limit response is the process's final socket write.
                        let mut writer = BufWriter::new(&stream);
                        let response = Response::Query(Err(failure.into_error()));
                        if serde_json::to_writer(&mut writer, &response).is_ok() {
                            let _ = writer.flush();
                        }
                        1
                    },
                    || session.execute(request),
                );
                match result {
                    Ok(result) => (Response::Query(result), false),
                    // Preserve setup failures instead of closing the socket
                    // with no response. A misconfigured process is disposable.
                    Err(error) => (Response::Failure(error.to_string()), true),
                }
            }
            Action::Status => (
                Response::Status(Status {
                    running: true,
                    pid: Some(std::process::id()),
                    generation: session.generation(),
                }),
                false,
            ),
            Action::Stop => (Response::Stopped, true),
        },
    };
    let mut writer = BufWriter::new(stream);
    serde_json::to_writer(&mut writer, &response)?;
    writer.flush()?;
    Ok(stop)
}
