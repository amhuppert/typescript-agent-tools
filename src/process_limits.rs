//! Process protection for read-only queries. The watchdog owns elapsed time
//! and physical memory; the evaluator owns native cycles and recursion depth.
//! Exceeding a threshold discards the process, including its warm service cache.
//! The limits, and the service's settings, come from `Environment`.
use crate::cli;
use crate::cli::failure::Code;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fmt;
use std::io;
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};
use typescript_parser::analysis::{
    AnalysisError, InputError, InputErrorKind, SemanticError, SemanticErrorKind,
    SemanticResourceReason,
};

const POLL: Duration = Duration::from_millis(25);
const MIB: u64 = 1 << 20;

/// The default of each variable `Environment` reads, as the one literal
/// both this module and help (`cli::registry`), whose text `concat!`
/// builds, take it from.
macro_rules! default_memory_mb {
    () => {
        6144
    };
}
macro_rules! default_timeout_ms {
    () => {
        60000
    };
}
macro_rules! default_idle_secs {
    () => {
        600
    };
}
/// The default runtime directory, before this user's id.
macro_rules! default_service_dir_prefix {
    () => {
        "/tmp/tsparse-"
    };
}
pub(crate) use {
    default_idle_secs, default_memory_mb, default_service_dir_prefix, default_timeout_ms,
};

/// The one reader of the `TSPARSE_*` variables that configure this program's
/// processes: the query limits `TSPARSE_MAX_MEMORY_MB` and
/// `TSPARSE_QUERY_TIMEOUT_MS`, and the service's `TSPARSE_SERVICE_IDLE_SECS`
/// and `TSPARSE_SERVICE_DIR`. A client and the service it starts read the
/// same environment, so a client that reads it before starting a service
/// reports an invalid value as the service would have, and starts nothing.
pub(crate) struct Environment {
    limits: Limits,
    #[cfg(unix)]
    idle: Duration,
}

impl Environment {
    /// Every numeric variable, each a positive integer that takes its
    /// default when it is not set, or the usage failure (`usage.environment`)
    /// of the first one that is invalid.
    pub(crate) fn read() -> Result<Self, cli::Failure> {
        let read = || {
            Ok(Self {
                limits: Limits {
                    max_memory_mb: MEMORY_MB.read()?,
                    query_timeout_ms: TIMEOUT_MS.read()?,
                },
                #[cfg(unix)]
                idle: Duration::from_secs(IDLE_SECS.read()?),
            })
        };
        read().map_err(|invalid: InvalidVariable| {
            cli::Failure::new(Code::Environment, invalid.to_string())
        })
    }

    /// The limits a query runs under.
    pub(crate) fn limits(&self) -> Limits {
        self.limits
    }

    /// How long a service waits for a request before it exits.
    #[cfg(unix)]
    pub(crate) fn idle(&self) -> Duration {
        self.idle
    }

    /// The service's runtime directory: `TSPARSE_SERVICE_DIR`, any path, or
    /// `/tmp/tsparse-UID` when it is not set.
    #[cfg(unix)]
    pub(crate) fn service_dir() -> PathBuf {
        Self::directory().0
    }

    /// The runtime directory, and whether it is the default.
    #[cfg(unix)]
    fn directory() -> (PathBuf, bool) {
        match std::env::var_os(SERVICE_DIR) {
            Some(directory) => (PathBuf::from(directory), false),
            None => (
                PathBuf::from(format!(
                    concat!(default_service_dir_prefix!(), "{}"),
                    crate::service::user_id()
                )),
                true,
            ),
        }
    }

    /// What the environment sets each variable to, in this order: the
    /// limits, the idle time, and the runtime directory, each as `read`
    /// and `service_dir` take it, so the doctor reports every invalid
    /// value, not only the first.
    pub(crate) fn settings() -> Vec<Setting> {
        let mut variables = vec![&MEMORY_MB, &TIMEOUT_MS];
        #[cfg(unix)]
        variables.push(&IDLE_SECS);
        let mut settings: Vec<Setting> = variables
            .into_iter()
            .map(|variable| match variable.given() {
                Ok(Some(number)) => Setting::Valid {
                    name: variable.name,
                    value: number.to_string(),
                    default: false,
                },
                Ok(None) => Setting::Valid {
                    name: variable.name,
                    value: variable.default.to_string(),
                    default: true,
                },
                Err(invalid) => Setting::Invalid(invalid.to_string()),
            })
            .collect();
        #[cfg(unix)]
        {
            let (directory, default) = Self::directory();
            settings.push(Setting::Valid {
                name: SERVICE_DIR,
                value: directory.display().to_string(),
                default,
            });
        }
        settings
    }
}

/// What the environment sets one variable to.
pub(crate) enum Setting {
    /// Its value as given, or its default when it is not set.
    Valid {
        name: &'static str,
        value: String,
        default: bool,
    },
    /// A value it does not accept, as the usage failure names it.
    Invalid(String),
}

/// The variable that names the service's runtime directory, which a client
/// also sets for the service it starts, so that both use one directory.
#[cfg(unix)]
pub(crate) const SERVICE_DIR: &str = "TSPARSE_SERVICE_DIR";

/// A positive integer variable: a value that is not set takes `default`;
/// any other must be an integer from 1 through `maximum`.
#[derive(Debug)]
struct Variable {
    name: &'static str,
    unit: &'static str,
    default: u64,
    maximum: u64,
}

static MEMORY_MB: Variable = Variable {
    name: "TSPARSE_MAX_MEMORY_MB",
    unit: "MiB",
    default: default_memory_mb!(),
    maximum: u64::MAX / MIB,
};
static TIMEOUT_MS: Variable = Variable {
    name: "TSPARSE_QUERY_TIMEOUT_MS",
    unit: "milliseconds",
    default: default_timeout_ms!(),
    maximum: u64::MAX,
};
#[cfg(unix)]
static IDLE_SECS: Variable = Variable {
    name: "TSPARSE_SERVICE_IDLE_SECS",
    unit: "seconds",
    default: default_idle_secs!(),
    maximum: u64::MAX,
};

impl Variable {
    fn read(&'static self) -> Result<u64, InvalidVariable> {
        Ok(match self.given()? {
            Some(number) => number,
            None => self.default,
        })
    }

    /// The value given, or `None` when the variable is not set.
    fn given(&'static self) -> Result<Option<u64>, InvalidVariable> {
        let Some(value) = std::env::var_os(self.name) else {
            return Ok(None);
        };
        match value.to_str().map(str::parse::<u64>) {
            Some(Ok(number)) if (1..=self.maximum).contains(&number) => Ok(Some(number)),
            None | Some(Ok(_)) | Some(Err(_)) => Err(InvalidVariable {
                variable: self,
                value,
            }),
        }
    }
}

/// A variable whose value `Environment` does not accept: its message names
/// the variable, the accepted form, the default, and the value as given.
#[derive(Debug)]
struct InvalidVariable {
    variable: &'static Variable,
    value: OsString,
}

impl fmt::Display for InvalidVariable {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let Variable {
            name,
            unit,
            default,
            maximum,
        } = self.variable;
        write!(
            formatter,
            "{name} must be a positive integer number of {unit}"
        )?;
        if *maximum < u64::MAX {
            write!(formatter, " up to {maximum}")?;
        }
        write!(formatter, " (default {default}), got ")?;
        // A value that is not Unicode is shown lossily. An empty one, or one
        // with a space, quote, or control character, is quoted, so the message
        // stays one line and shows where the value ends.
        let value = self.value.to_string_lossy();
        if value.is_empty()
            || value
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || c == '"')
        {
            write!(formatter, "{value:?}")
        } else {
            formatter.write_str(&value)
        }
    }
}

/// The limits a query runs under, as their variables give them, which a
/// service also reports (`--references-status`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Limits {
    /// `TSPARSE_MAX_MEMORY_MB`; its maximum keeps the byte count in range.
    pub(crate) max_memory_mb: u64,
    /// `TSPARSE_QUERY_TIMEOUT_MS`.
    pub(crate) query_timeout_ms: u64,
}

impl Limits {
    fn memory(&self) -> u64 {
        self.max_memory_mb * MIB
    }

    fn duration(&self) -> Duration {
        Duration::from_millis(self.query_timeout_ms)
    }

    /// Each limit with its variable's name, in `settings` order, for a
    /// report that sets two sets of limits side by side.
    pub(crate) fn named(&self) -> [(&'static str, u64); 2] {
        [
            (MEMORY_MB.name, self.max_memory_mb),
            (TIMEOUT_MS.name, self.query_timeout_ms),
        ]
    }

    fn failure(&self, started: Instant) -> Option<Failure> {
        if started.elapsed() >= self.duration() {
            return Some(Failure::Deadline);
        }
        match memory_bytes() {
            Ok(bytes) if bytes > self.memory() => Some(Failure::Memory { bytes }),
            Ok(_) => None,
            Err(error) => Some(Failure::Monitor(error)),
        }
    }
}

/// Where a query that a limit stopped ran, which decides what stopping it
/// cost.
#[derive(Clone, Copy)]
pub(crate) enum Context {
    /// A process that answers one request and exits.
    Direct,
    /// The project's service, whose cache the next request rebuilds.
    Service,
}

pub(crate) enum Failure {
    Deadline,
    Memory { bytes: u64 },
    Monitor(io::Error),
}
impl Failure {
    /// The analysis failure a limit is reported as: which variable it
    /// exceeded and its configured value (and for memory the footprint
    /// observed), and what exceeding it cost in `context`. It never says to
    /// restart a service, which has already exited when its query hit a
    /// limit (design, step 5); `remedy` says what to change.
    pub(crate) fn into_error(self, limits: &Limits, context: Context) -> AnalysisError {
        let (place, cost) = match context {
            Context::Direct => ("", "this query process exited"),
            Context::Service => (
                " in the project service",
                "that service and its cache exited, and the next request starts a replacement",
            ),
        };
        match self {
            Self::Deadline => SemanticError::resource_limit(
                SemanticResourceReason::SemanticDeadline,
                format!(
                    "query exceeded {} ({} ms){place}; {cost}",
                    TIMEOUT_MS.name, limits.query_timeout_ms
                ),
            )
            .into(),
            Self::Memory { bytes } => SemanticError::resource_limit(
                SemanticResourceReason::SemanticMemoryLimit,
                format!(
                    "query memory footprint {bytes} bytes exceeded {} ({} MiB, {} bytes){place}; {cost}",
                    MEMORY_MB.name,
                    limits.max_memory_mb,
                    limits.memory()
                ),
            )
            .into(),
            Self::Monitor(error) => InputError {
                kind: InputErrorKind::Io,
                message: format!("cannot monitor query process memory: {error}"),
                path: None,
                position: None,
            }
            .into(),
        }
    }
}

/// The remedy of a failure a limit reported, a sentence a document carries
/// as `error.remedy` and text prints as its `next` line (decisions.md,
/// step 5). A query's deadline and memory threshold are this module's
/// alone (`Failure::into_error`), so these kinds name them.
pub(crate) fn remedy(error: &AnalysisError) -> Option<String> {
    let AnalysisError::Semantic(error) = error else {
        return None;
    };
    let variable = match error.kind {
        SemanticErrorKind::SemanticDeadline => TIMEOUT_MS.name,
        SemanticErrorKind::SemanticMemoryLimit => MEMORY_MB.name,
        SemanticErrorKind::UnsupportedSemantic
        | SemanticErrorKind::InvalidSemantic
        | SemanticErrorKind::SemanticDepthLimit
        | SemanticErrorKind::OutputLimit
        | SemanticErrorKind::Internal => return None,
    };
    Some(format!("set a higher {variable} before retrying"))
}

/// Whether this process can sample its own memory, which every query's
/// watchdog does: the footprint it samples now.
pub(crate) fn memory_footprint() -> io::Result<u64> {
    memory_bytes()
}

/// Execute one read-only query under `environment`'s limits. Normal
/// completion cancels and joins the watchdog. A denied query reports through
/// its caller's existing delivery boundary and exits the process; it never
/// resumes partially completed work.
/// The monitor samples memory, so this is a threshold with possible overshoot,
/// not an exact allocator cap. File application must not run inside this scope.
pub(crate) fn run<T>(
    environment: &Environment,
    abort: impl Fn(Failure) -> i32 + Sync,
    work: impl FnOnce() -> T,
) -> io::Result<T> {
    let limits = &environment.limits;
    let started = Instant::now();
    if let Some(failure) = limits.failure(started) {
        std::process::exit(abort(failure));
    }
    let state = (Mutex::new(false), Condvar::new());
    struct Finish<'a>(&'a (Mutex<bool>, Condvar));
    impl Drop for Finish<'_> {
        fn drop(&mut self) {
            *self.0.0.lock().unwrap_or_else(|p| p.into_inner()) = true;
            self.0.1.notify_one();
        }
    }
    std::thread::scope(|scope| {
        let watchdog = std::thread::Builder::new()
            .name("tsparse-watchdog".into())
            .spawn_scoped(scope, || {
                let mut finished = state.0.lock().unwrap_or_else(|p| p.into_inner());
                loop {
                    if *finished {
                        return;
                    }
                    if let Some(failure) = limits.failure(started) {
                        std::process::exit(abort(failure));
                    }
                    let wait = POLL.min(limits.duration().saturating_sub(started.elapsed()));
                    finished = state
                        .1
                        .wait_timeout(finished, wait)
                        .unwrap_or_else(|p| p.into_inner())
                        .0;
                }
            })?;
        let finish = Finish(&state);
        let value = work();
        drop(finish);
        watchdog
            .join()
            .map_err(|_| io::Error::other("query process watchdog panicked"))?;
        Ok(value)
    })
}

#[cfg(target_os = "macos")]
fn memory_bytes() -> io::Result<u64> {
    // libproc's RUSAGE_INFO_V0 is a UUID followed by ten uint64_t fields.
    // phys_footprint includes compressed memory, unlike resident_size alone.
    #[repr(C)]
    #[derive(Default)]
    struct Usage {
        uuid: [u8; 16],
        counters: [u64; 10],
    }
    unsafe extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut std::ffi::c_void) -> i32;
    }
    let mut usage = Usage::default();
    // SAFETY: a live, aligned RUSAGE_INFO_V0 buffer of the documented size;
    // libproc writes only that version's fields for the current process.
    let status = unsafe {
        proc_pid_rusage(
            std::process::id() as i32,
            0,
            (&mut usage as *mut Usage).cast(),
        )
    };
    if status == 0 {
        Ok(usage.counters[7])
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
fn memory_bytes() -> io::Result<u64> {
    // VmRSS + VmSwap counts resident and swapped anonymous memory. Reading the
    // process's own procfs entry needs no privilege and no sampling subprocess.
    let status = std::fs::read_to_string("/proc/self/status")?;
    let read = |key: &str| -> io::Result<u64> {
        status
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|text| text.split_whitespace().next())
            .and_then(|text| text.parse::<u64>().ok())
            .and_then(|kb| kb.checked_mul(1024))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("missing or invalid {key} in /proc/self/status"),
                )
            })
    };
    read("VmRSS:")?
        .checked_add(read("VmSwap:")?)
        .ok_or_else(|| io::Error::other("process memory size overflow"))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn memory_bytes() -> io::Result<u64> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "query memory monitoring supports macOS and Linux",
    ))
}
