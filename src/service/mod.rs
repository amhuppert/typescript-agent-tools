//! Local process reuse for CLI reference queries. The wire format, rendezvous,
//! startup races, and shutdown belong here; callers supply complete operations.

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
use typescript_parser::references::{
    QueryErrorKind, ReferenceError, ReferenceQuery, ReferenceResult, ReferenceSession,
};

const STARTUP_WAIT: Duration = Duration::from_secs(10);
const CLIENT_WAIT: Duration = Duration::from_secs(300);
const SOCKET_WAIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);
const REQUEST_LIMIT: u64 = 1 << 20;

#[derive(Debug)]
pub enum Error {
    Reference(ReferenceError),
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
    Query(ReferenceQuery),
    Status,
    Stop,
}

#[derive(Serialize, Deserialize)]
enum Response {
    Query(Result<ReferenceResult, ReferenceError>),
    Status(Status),
    Stopped,
    Failure(String),
}

/// Administrative observation only: generation counts published indexes, and
/// status itself neither loads a project nor validates its freshness.
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
            Err(error) => Err(error),
        }
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

pub fn query(config: &Path, mut query: ReferenceQuery) -> Result<ReferenceResult, Error> {
    let original_file = match &mut query {
        ReferenceQuery::At { file, .. }
        | ReferenceQuery::Name {
            file: Some(file), ..
        } => {
            let original = file.clone();
            // Empty operands remain empty: the query owner reports OutsideProject.
            if !file.as_os_str().is_empty() {
                *file = std::path::absolute(&*file)?;
            }
            Some(original)
        }
        ReferenceQuery::Name { file: None, .. } => None,
    };
    let endpoint = Endpoint::new(config)?;
    match endpoint.exchange(endpoint.start()?, Action::Query(query))? {
        Response::Query(result) => result.map_err(|mut error| {
            if let ReferenceError::Query(query_error) = &mut error {
                // These diagnostics name the query operand. Semantic target
                // errors name captured source files and keep their canonical path.
                if matches!(
                    query_error.kind,
                    QueryErrorKind::OutsideProject | QueryErrorKind::InvalidPosition
                ) {
                    query_error.path = original_file;
                }
            }
            Error::Reference(error)
        }),
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
    let endpoint = Endpoint::new(config)?;
    endpoint.prepare()?;
    let Some(_lease) = lease::Lease::acquire(&endpoint.lock)? else {
        return Ok(());
    };
    remove_socket(&endpoint.socket)?;
    let listener = UnixListener::bind(&endpoint.socket)?;
    listener.set_nonblocking(true)?;
    let mut session = ReferenceSession::new(config).map_err(io::Error::other)?;
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
    let mut last_request = Instant::now();
    let result = loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // A malformed, stalled, or disconnected client must not kill the
                // project's service. The client receives the transport failure.
                match handle(stream, &endpoint.identity, &mut session) {
                    Ok(true) => break Ok(()),
                    Ok(false) | Err(_) => last_request = Instant::now(),
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if last_request.elapsed() >= idle {
                    break Ok(());
                }
                std::thread::sleep(POLL);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => break Err(error),
        }
    };
    let cleanup = remove_socket(&endpoint.socket);
    result.and(cleanup)
}

fn handle(
    mut stream: UnixStream,
    identity: &Identity,
    session: &mut ReferenceSession,
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
            Action::Query(query) => (Response::Query(session.find_references(query)), false),
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
