//! Process protection for read-only queries. The watchdog owns elapsed time
//! and physical memory; the evaluator owns native cycles and recursion depth.
//! Exceeding a threshold discards the process, including its warm service cache.
use std::io;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};
use typescript_parser::analysis::{
    AnalysisError, InputError, InputErrorKind, SemanticError, SemanticResourceReason,
};

const POLL: Duration = Duration::from_millis(25);

struct Limits {
    memory: u64,
    duration: Duration,
}
impl Limits {
    fn from_environment() -> io::Result<Self> {
        fn positive(name: &str, default: u64) -> io::Result<u64> {
            match std::env::var(name) {
                Err(std::env::VarError::NotPresent) => Ok(default),
                value => value
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .filter(|n| *n > 0)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("{name} must be a positive integer"),
                        )
                    }),
            }
        }
        let memory = positive("TSPARSE_MAX_MEMORY_MB", 6144)?
            .checked_mul(1024 * 1024)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "TSPARSE_MAX_MEMORY_MB is too large",
                )
            })?;
        Ok(Self {
            memory,
            duration: Duration::from_millis(positive("TSPARSE_QUERY_TIMEOUT_MS", 60_000)?),
        })
    }
    fn failure(&self, started: Instant) -> Option<Failure> {
        if started.elapsed() >= self.duration {
            return Some(Failure::Deadline);
        }
        match memory_bytes() {
            Ok(bytes) if bytes > self.memory => Some(Failure::Memory {
                bytes,
                limit: self.memory,
            }),
            Ok(_) => None,
            Err(error) => Some(Failure::Monitor(error)),
        }
    }
}

pub(crate) enum Failure {
    Deadline,
    Memory { bytes: u64, limit: u64 },
    Monitor(io::Error),
}
impl Failure {
    pub(crate) fn into_error(self) -> AnalysisError {
        match self {
            Self::Deadline => SemanticError::resource_limit(SemanticResourceReason::SemanticDeadline,
                "query process exceeded its elapsed-time limit").into(),
            Self::Memory { bytes, limit } => SemanticError::resource_limit(SemanticResourceReason::SemanticMemoryLimit,
                format!("query process memory footprint {bytes} bytes exceeds its {limit}-byte threshold")).into(),
            Self::Monitor(error) => InputError { kind: InputErrorKind::Io,
                message: format!("cannot monitor query process memory: {error}"), path: None, position: None }.into(),
        }
    }
}

/// Execute one read-only query. Normal completion cancels and joins the
/// watchdog. A denied query reports through its caller's existing delivery
/// boundary and exits the process; it never resumes partially completed work.
/// The monitor samples memory, so this is a threshold with possible overshoot,
/// not an exact allocator cap. File application must not run inside this scope.
pub(crate) fn run<T>(
    abort: impl Fn(Failure) -> i32 + Sync,
    work: impl FnOnce() -> T,
) -> io::Result<T> {
    let limits = Limits::from_environment()?;
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
                    let wait = POLL.min(limits.duration.saturating_sub(started.elapsed()));
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
