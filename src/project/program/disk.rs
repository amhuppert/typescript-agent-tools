//! File system probes for loading. What counts as absent is
//! `crate::input::is_absence`: any other failure, such as a permission
//! error, means an input may exist but cannot be inspected; loading reports
//! it as `Io` rather than resolving past it.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Filesystem access for one load. Recording is explicit and local to that load;
/// the ordinary immutable-project API does not retain observations.
#[derive(Default)]
pub(in crate::project) struct Disk {
    observations: RefCell<Option<Observations>>,
    /// Reads of one primitive resolver operation, only while a session is
    /// capturing reusable inputs. Ordinary project loads never open a trace.
    receipt: RefCell<Option<Vec<(PathBuf, Operation, Observation)>>>,
    /// The real paths this load found. It asks each path's once, so it sees
    /// one file system however often it asks, as the module resolution of
    /// every importer does.
    real_paths: RefCell<HashMap<PathBuf, PathBuf>>,
    waits: Waits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Operation {
    Text,
    Bytes,
    Entries,
    Canonical,
    Entry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Observation {
    Text(Arc<str>),
    Bytes(Arc<[u8]>),
    Entries(Vec<OsString>),
    Canonical(PathBuf),
    Entry(Option<Entry>),
    Absent,
    /// Anticipatory module-specifier probes can fail without failing load;
    /// query-time demand still depends on that exact captured failure.
    Failed(io::ErrorKind, Option<i32>, String),
}

/// Exact physical evidence for one logical resolver-host answer. Logical
/// memoization belongs to the host: this does not imply that another host
/// operation asking about the same path has already run.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Receipt(Box<[(PathBuf, Operation, Observation)]>);

/// A primitive read's temporary trace is discarded even if its reader panics.
struct ReceiptScope<'a>(&'a Disk);

impl Drop for ReceiptScope<'_> {
    fn drop(&mut self) {
        self.0.receipt.borrow_mut().take();
    }
}

/// The actual answers used during construction, including unsuccessful probes.
/// No timestamps or content hashes stand in for exact captured contents.
#[derive(Debug, Default)]
pub(in crate::project) struct Observations {
    values: BTreeMap<(PathBuf, Operation), Observation>,
    inconsistent: bool,
}

/// Threads that repeat observations. The work is file system calls, which
/// on a laptop scale to about eight threads (5.5 times one thread's rate).
const VALIDATION_THREADS: usize = 8;
/// Observations a validation thread claims at a time.
const VALIDATION_BATCH: usize = 32;

impl Observations {
    /// Whether every observation still holds. The first observation in key
    /// order that differs or cannot be repeated decides, as one pass in key
    /// order would: a difference means the inputs changed, and a failure is
    /// that failure. Several threads repeat observations at once; whatever
    /// they find after the deciding observation has no effect, and a read
    /// that could wait is made only where that pass would make it, before
    /// any later observation is repeated.
    pub(in crate::project) fn unchanged(&self) -> Result<bool, super::InputError> {
        self.unchanged_with(&|| {})
    }

    /// `unchanged`, calling `stale` as soon as some observation differs or
    /// fails: the answer is then not `Ok(true)`, though which observation
    /// decides it may not be known yet.
    pub(in crate::project) fn unchanged_with(
        &self,
        stale: &(dyn Fn() + Sync),
    ) -> Result<bool, super::InputError> {
        if self.inconsistent {
            stale();
            return Ok(false);
        }
        let observations: Vec<_> = self.values.iter().collect();
        let threads = std::thread::available_parallelism()
            .map_or(1, std::num::NonZero::get)
            .min(VALIDATION_THREADS);
        let decisive = first_decisive(
            threads,
            &observations,
            |((path, operation), before), reading| match still(path, *operation, before, reading) {
                None => Outcome::Deferred,
                Some(Ok(true)) => Outcome::Same,
                Some(decided) => {
                    stale();
                    Outcome::Decisive(decided)
                }
            },
        );
        decisive.unwrap_or(Ok(true))
    }
}

/// Whether `path` still answers `operation` with `before`, or nothing when a
/// speculative `reading` declines to read it. Absence is an answer; another
/// failure is one only where loading recorded a failure, and otherwise an
/// error.
fn still(
    path: &Path,
    operation: Operation,
    before: &Observation,
    reading: Reading<'_>,
) -> Option<Result<bool, super::InputError>> {
    let result = match operation {
        Operation::Text => read(path, reading)?
            .and_then(crate::source::decode_text)
            .map(|s| Observation::Text(s.into())),
        Operation::Bytes => read(path, reading)?.map(|b| Observation::Bytes(b.into())),
        Operation::Entries => directory(path).map(|(_, names)| Observation::Entries(names)),
        Operation::Canonical => std::fs::canonicalize(path).map(Observation::Canonical),
        Operation::Entry => entry(path).map(Observation::Entry),
    };
    let after = match result {
        Ok(value) => value,
        Err(error) if is_absence(&error) => Observation::Absent,
        Err(error) if matches!(before, Observation::Failed(..)) => failure(&error),
        Err(error) => return Some(Err(super::io_error(path, &error))),
    };
    Some(Ok(*before == after))
}

/// How a read may wait. A blocking reading reads as an ordinary read does,
/// waiting as long as the file makes it wait. A speculative reading, made
/// before anyone knows its answer is needed, reads only a regular file, and
/// declines a pipe, which waits for its writer: a read in order might never
/// have reached it. A reading in order is speculative where that reads and
/// blocking otherwise, counting in `Waits` each blocking read that may have
/// waited.
#[derive(Debug, Clone, Copy)]
pub(super) enum Reading<'w> {
    Blocking,
    Speculative,
    InOrder(&'w Waits),
}

/// The reads in order of one load that may have waited, such as a pipe's,
/// counted as they end. A speculative read made before such a read ended
/// may differ from reading in order (`capture::Captures::take`).
#[derive(Debug, Default)]
pub(super) struct Waits(AtomicU64);

impl Waits {
    pub(super) fn count(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// The contents of `path`, or nothing when a speculative `reading` declines.
fn read(path: &Path, reading: Reading<'_>) -> Option<io::Result<Vec<u8>>> {
    let regular = match reading {
        Reading::Blocking => return Some(std::fs::read(path)),
        Reading::Speculative | Reading::InOrder(_) => read_regular(path),
    };
    match (regular, reading) {
        (Regular::Read(bytes), _) => Some(Ok(bytes)),
        (Regular::Absent(error), _) => Some(Err(error)),
        (Regular::Declined { .. }, Reading::Speculative) => None,
        (Regular::Declined { may_wait }, Reading::InOrder(waits)) => {
            let result = std::fs::read(path);
            if may_wait {
                waits.0.fetch_add(1, Ordering::SeqCst);
            }
            Some(result)
        }
        (_, Reading::Blocking) => unreachable!("a blocking reading reads"),
    }
}

/// A speculative read's outcome.
enum Regular {
    Read(Vec<u8>),
    /// `stat` and `open` resolve a path alike, so its absence is the same
    /// answer either way.
    Absent(io::Error),
    /// Not a regular file, or a failure without waiting, which need not be
    /// the failure a blocking read reports, such as a lease's
    /// `EWOULDBLOCK`. A blocking read decides; it may wait when the path is
    /// not a regular file or the failure is one of waiting.
    Declined {
        may_wait: bool,
    },
}

fn read_regular(path: &Path) -> Regular {
    let declined = |error: io::Error| Regular::Declined {
        may_wait: error.kind() == io::ErrorKind::WouldBlock,
    };
    // Opening a pipe can itself wait for, or release, its writer: decline
    // what is not a regular file before opening it. This needs the failure
    // itself, which `entry` classifies away.
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Regular::Declined { may_wait: true },
        Err(error) if is_absence(&error) => return Regular::Absent(error),
        Err(error) => return declined(error),
    }
    // Opening without waiting, and checking what was opened, also declines
    // a file replaced by a pipe in between.
    let mut file = match open_without_waiting(path) {
        Ok(file) => file,
        Err(error) => return declined(error),
    };
    match file.metadata() {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Regular::Declined { may_wait: true },
        Err(error) => return declined(error),
    }
    let mut bytes = Vec::new();
    match io::Read::read_to_end(&mut file, &mut bytes) {
        Ok(_) => Regular::Read(bytes),
        Err(error) => declined(error),
    }
}

/// `O_NONBLOCK` where this build knows its value; reading a regular file
/// ignores it.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
pub(super) const NONBLOCK: Option<i32> = Some(0x0004);
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "riscv64"
    )
))]
pub(super) const NONBLOCK: Option<i32> = Some(0o4000);
/// Elsewhere only the check before opening declines a pipe.
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    all(
        any(target_os = "linux", target_os = "android"),
        any(
            target_arch = "x86",
            target_arch = "x86_64",
            target_arch = "arm",
            target_arch = "aarch64",
            target_arch = "riscv64"
        )
    )
)))]
pub(super) const NONBLOCK: Option<i32> = None;

#[cfg(unix)]
fn open_without_waiting(path: &Path) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    if let Some(flag) = NONBLOCK {
        options.custom_flags(flag);
    }
    options.open(path)
}

#[cfg(not(unix))]
fn open_without_waiting(path: &Path) -> io::Result<std::fs::File> {
    std::fs::File::open(path)
}

/// What `first_decisive`'s `decide` made of one item.
enum Outcome<R> {
    Same,
    Decisive(R),
    /// A speculative reading declined to decide; a blocking one must.
    Deferred,
}

/// `decide`'s answer for the first of `items` it answers, as a loop in
/// order would find it, computed on up to `threads` threads with speculative
/// readings. Every item before the answered one is decided; later items may
/// be too, and those answers are discarded. An item a speculative reading
/// defers is decided by a blocking reading, in order, only if no earlier item
/// answered: where the loop would have read it. The items between it and the
/// next deferred item are then decided again, after that read, as the loop
/// decides them: a change made while the read waited is found. Each item is
/// decided again at most once per pass that defers it.
fn first_decisive<T: Sync, R: Send>(
    threads: usize,
    items: &[T],
    decide: impl Fn(&T, Reading<'_>) -> Outcome<R> + Sync,
) -> Option<R> {
    decide_in_order(threads, items, &decide)
}

fn decide_in_order<T: Sync, R: Send>(
    threads: usize,
    items: &[T],
    decide: &(impl Fn(&T, Reading<'_>) -> Outcome<R> + Sync),
) -> Option<R> {
    let (found, deferred) = speculative_pass(threads, items, decide);
    let reached = found.as_ref().map_or(items.len(), |&(at, _)| at);
    let deferred: Vec<usize> = deferred
        .into_iter()
        .filter(|&index| index < reached)
        .collect();
    if deferred.is_empty() {
        return found.map(|(_, answer)| answer);
    }
    for (position, &index) in deferred.iter().enumerate() {
        match decide(&items[index], Reading::Blocking) {
            Outcome::Same => {}
            Outcome::Decisive(answer) => return Some(answer),
            Outcome::Deferred => unreachable!("a blocking reading decides"),
        }
        let next = deferred.get(position + 1).copied().unwrap_or(items.len());
        if let Some(answer) = decide_in_order(threads, &items[index + 1..next], decide) {
            return Some(answer);
        }
    }
    None
}

/// One speculative pass of `first_decisive` over `items`: the first item it
/// answers, by index, and the items it deferred, in order.
fn speculative_pass<T: Sync, R: Send>(
    threads: usize,
    items: &[T],
    decide: &(impl Fn(&T, Reading<'_>) -> Outcome<R> + Sync),
) -> (Option<(usize, R)>, Vec<usize>) {
    let next = AtomicUsize::new(0);
    // Items at or after the earliest answer found so far need no decision.
    let bound = AtomicUsize::new(items.len());
    let found: Mutex<Option<(usize, R)>> = Mutex::new(None);
    let deferred = Mutex::new(Vec::new());
    let work = || {
        loop {
            let start = next.fetch_add(VALIDATION_BATCH, Ordering::Relaxed);
            if start >= bound.load(Ordering::Relaxed) {
                return;
            }
            for (index, item) in items.iter().enumerate().skip(start).take(VALIDATION_BATCH) {
                if index >= bound.load(Ordering::Relaxed) {
                    break;
                }
                match decide(item, Reading::Speculative) {
                    Outcome::Same => {}
                    Outcome::Deferred => deferred
                        .lock()
                        .expect("no thread panics holding the deferred items")
                        .push(index),
                    Outcome::Decisive(answer) => {
                        let mut found = found.lock().expect("no thread panics holding the answer");
                        if found.as_ref().is_none_or(|&(at, _)| index < at) {
                            *found = Some((index, answer));
                            bound.fetch_min(index, Ordering::Relaxed);
                        }
                        break;
                    }
                }
            }
        }
    };
    let threads = threads.min(items.len().div_ceil(VALIDATION_BATCH)).max(1);
    std::thread::scope(|scope| {
        let work = &work;
        for _ in 1..threads {
            // A helper that cannot start leaves its share to the others: the
            // calling thread decides every item no helper claims, so the
            // answer is the same with any number of helpers.
            let _ = std::thread::Builder::new().spawn_scoped(scope, work);
        }
        work();
    });
    let mut deferred = deferred
        .into_inner()
        .expect("no thread panics holding the deferred items");
    deferred.sort_unstable();
    (
        found
            .into_inner()
            .expect("no thread panics holding the answer"),
        deferred,
    )
}

impl Disk {
    pub(in crate::project) fn recording() -> Self {
        Self {
            observations: RefCell::new(Some(Observations::default())),
            receipt: RefCell::default(),
            real_paths: RefCell::default(),
            waits: Waits::default(),
        }
    }

    pub(in crate::project) fn finish(self) -> Observations {
        self.observations
            .into_inner()
            .expect("a recording disk owns observations")
    }

    /// Capture one primitive host operation, including its unsuccessful
    /// probes. Its normal load observations and failures are unchanged.
    pub(super) fn capture<T>(&self, read: impl FnOnce() -> T) -> (T, Receipt) {
        assert!(
            self.receipt.borrow().is_none(),
            "primitive reads do not nest"
        );
        *self.receipt.borrow_mut() = Some(Vec::new());
        let _scope = ReceiptScope(self);
        let value = read();
        let receipt = self.receipt.borrow_mut().take().expect("active receipt");
        (value, Receipt(receipt.into_boxed_slice()))
    }

    /// Whether a missing logical host answer may be adopted. This has no
    /// effects on the current load and never opens a pipe: a changed, failed,
    /// or deferred probe means the resolver must perform its ordinary lookup,
    /// which owns demand and error order. Real paths already memoized by this
    /// load are authoritative, just as they are in `canonicalize`.
    pub(super) fn can_replay(&self, receipt: &Receipt) -> bool {
        for (path, operation, before) in &receipt.0 {
            if let Observation::Canonical(expected) = before
                && let Some(current) = self.real_paths.borrow().get(path)
            {
                if current != expected {
                    return false;
                }
                continue;
            }
            match still(path, *operation, before, Reading::Speculative) {
                Some(Ok(true)) => {}
                Some(Ok(false) | Err(_)) | None => return false,
            }
        }
        true
    }

    /// Commit evidence after every access in the resolution has been checked.
    /// This also restores canonicalization's first-answer memo, a side effect
    /// a cached resolution must preserve for subsequent fresh lookups.
    pub(super) fn replay(&self, receipt: &Receipt) {
        for (path, operation, value) in &receipt.0 {
            if let Observation::Canonical(real) = value {
                let mut paths = self.real_paths.borrow_mut();
                match paths.entry(path.clone()) {
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(real.clone());
                    }
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        assert_eq!(entry.get(), real, "replay was validated before commit");
                    }
                }
            }
            self.record(path, *operation, Ok(value.clone()));
        }
    }

    fn record(&self, path: &Path, operation: Operation, result: Result<Observation, &io::Error>) {
        let mut observations = self.observations.borrow_mut();
        if observations.is_none() && self.receipt.borrow().is_none() {
            return;
        }
        let value = match result {
            Ok(value) => value,
            Err(error) if is_absence(error) => Observation::Absent,
            Err(error) => failure(error),
        };
        if let Some(receipt) = self.receipt.borrow_mut().as_mut() {
            receipt.push((path.to_path_buf(), operation, value.clone()));
        }
        let Some(observations) = observations.as_mut() else {
            return;
        };
        match observations.values.entry((path.to_path_buf(), operation)) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(value);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                if *entry.get() != value {
                    observations.inconsistent = true;
                }
            }
        }
    }

    /// The reads this load made in order that may have waited.
    pub(super) fn waits(&self) -> &Waits {
        &self.waits
    }

    /// A file's text as TypeScript reads it (`source::read_text`), read in
    /// load order (`Reading::InOrder`).
    pub(super) fn read_to_string(&self, path: impl AsRef<Path>) -> io::Result<Arc<str>> {
        self.receive(
            TextRead::with(path.as_ref(), Reading::InOrder(&self.waits))
                .expect("a reading in order reads"),
        )
    }

    /// The text of a read made on another thread, observed now, where the
    /// load uses it, as `read_to_string` observes its own read. A read the
    /// load never receives is no observation.
    pub(super) fn receive(&self, read: TextRead) -> io::Result<Arc<str>> {
        self.record(
            &read.path,
            Operation::Text,
            read.text.as_ref().map(|s| Observation::Text(s.clone())),
        );
        read.text
    }

    /// A file's bytes, read in load order (`Reading::InOrder`).
    pub(super) fn read(&self, path: impl AsRef<Path>) -> io::Result<Arc<[u8]>> {
        let path = path.as_ref();
        let result = read(path, Reading::InOrder(&self.waits))
            .expect("a reading in order reads")
            .map(Arc::<[u8]>::from);
        self.record(
            path,
            Operation::Bytes,
            result.as_ref().map(|b| Observation::Bytes(b.clone())),
        );
        result
    }

    /// A path's real path, the first answer this load found for it. A failure
    /// is asked again: it records nothing a later success would contradict.
    pub(super) fn canonicalize(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
        let path = path.as_ref();
        if let Some(real) = self.real_paths.borrow().get(path) {
            // A newly captured resolver decision may depend on a real path
            // that another operation already asked for in this generation.
            if self.receipt.borrow().is_some() {
                self.record(
                    path,
                    Operation::Canonical,
                    Ok(Observation::Canonical(real.clone())),
                );
            }
            return Ok(real.clone());
        }
        let result = std::fs::canonicalize(path);
        self.record(
            path,
            Operation::Canonical,
            result.as_ref().map(|p| Observation::Canonical(p.clone())),
        );
        if let Ok(real) = &result {
            self.real_paths
                .borrow_mut()
                .insert(path.to_path_buf(), real.clone());
        }
        result
    }

    pub(super) fn read_dir(&self, path: impl AsRef<Path>) -> io::Result<Vec<std::fs::DirEntry>> {
        let path = path.as_ref();
        let result = directory(path);
        self.record(
            path,
            Operation::Entries,
            result
                .as_ref()
                .map(|(_, names)| Observation::Entries(names.clone())),
        );
        result.map(|(entries, _)| entries)
    }

    pub(super) fn entry(&self, path: &Path) -> io::Result<Option<Entry>> {
        let result = entry(path);
        self.record(
            path,
            Operation::Entry,
            result.as_ref().map(|e| Observation::Entry(*e)),
        );
        result
    }

    pub(super) fn is_file(&self, path: &str) -> io::Result<bool> {
        Ok(self.entry(Path::new(path))? == Some(Entry::File))
    }

    /// Native `vfs.Common.FileExists` includes every non-directory entry.
    /// Input admission's regular-file test (`is_file`) is a different question.
    pub(super) fn file_exists(&self, path: &str) -> io::Result<bool> {
        Ok(matches!(
            self.entry(Path::new(path))?,
            Some(Entry::File | Entry::Other)
        ))
    }

    pub(super) fn is_dir(&self, path: &str) -> io::Result<bool> {
        Ok(self.entry(Path::new(path))? == Some(Entry::Directory))
    }
}

/// A file's text as TypeScript reads it, read on any thread and observed
/// only when a load receives it (`Disk::receive`).
pub(super) struct TextRead {
    path: PathBuf,
    text: io::Result<Arc<str>>,
}

impl TextRead {
    /// The read `reading` makes, as `source::read_text` reads, or nothing
    /// when it declines to read.
    pub(super) fn with(path: &Path, reading: Reading<'_>) -> Option<Self> {
        Some(Self {
            path: path.to_path_buf(),
            text: read(path, reading)?
                .and_then(crate::source::decode_text)
                .map(Arc::<str>::from),
        })
    }

    /// The text read, before any load observes it.
    pub(super) fn text(&self) -> Result<&Arc<str>, &io::Error> {
        self.text.as_ref()
    }

    /// Share an earlier allocation only after this read returned the exact
    /// same decoded text. The current read (including any failure or wait)
    /// still happens at its normal place in loading.
    pub(super) fn reuse_text(&mut self, previous: &Arc<str>) -> bool {
        match &mut self.text {
            Ok(text) if text.as_ref() == previous.as_ref() => {
                *text = Arc::clone(previous);
                true
            }
            _ => false,
        }
    }
}

fn failure(error: &io::Error) -> Observation {
    Observation::Failed(error.kind(), error.raw_os_error(), error.to_string())
}

fn directory(path: &Path) -> io::Result<(Vec<std::fs::DirEntry>, Vec<OsString>)> {
    let entries = std::fs::read_dir(path)?.collect::<io::Result<Vec<_>>>()?;
    let mut names: Vec<_> = entries.iter().map(std::fs::DirEntry::file_name).collect();
    names.sort();
    Ok((entries, names))
}

/// The one rule for absence, which loading shares with editing.
pub(super) use crate::input::is_absence;

/// What a path names, following symbolic links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Entry {
    File,
    Directory,
    Other,
}

/// What `path` names, or `None` when it is absent; a broken symbolic link is
/// absent. A path with a NUL character is absent without a lookup: no file
/// system can store such a name, so there is no error to classify. Module
/// names come from source text, where an ambient module may still satisfy one
/// (`declare module "a\0b"`).
fn entry(path: &Path) -> io::Result<Option<Entry>> {
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Ok(None);
    }
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(Some(Entry::File)),
        Ok(metadata) if metadata.is_dir() => Ok(Some(Entry::Directory)),
        Ok(_) => Ok(Some(Entry::Other)),
        Err(error) if is_absence(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{Disk, Outcome, Reading, VALIDATION_BATCH, first_decisive};
    use crate::analysis::InputErrorKind;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    struct Directory(PathBuf);

    impl Directory {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "tsparse-disk-{name}-{}-{:?}",
                std::process::id(),
                std::time::SystemTime::now()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Record a read of each of `count` files, named so that their key order
    /// is their numbering.
    fn recorded(directory: &Directory, count: usize) -> (Vec<PathBuf>, super::Observations) {
        let disk = Disk::recording();
        let paths: Vec<PathBuf> = (0..count)
            .map(|index| directory.0.join(format!("f{index:05}.ts")))
            .collect();
        for path in &paths {
            std::fs::write(path, "export {};").unwrap();
            disk.read_to_string(path).unwrap();
        }
        (paths, disk.finish())
    }

    /// A file that can no longer be read, nor is absent: a directory.
    fn unreadable(path: &PathBuf) {
        std::fs::remove_file(path).unwrap();
        std::fs::create_dir(path).unwrap();
    }

    /// The earliest decisive observation in key order decides, however many
    /// unchanged observations surround it: a change before a failure means
    /// the inputs changed, and a failure before a change is that failure.
    #[test]
    fn the_first_decisive_observation_in_key_order_decides() {
        let count = 3000;
        for (changed, failed) in [(10, 2990), (2990, 10), (1500, 1501), (1501, 1500)] {
            let directory = Directory::new("order");
            let (paths, observations) = recorded(&directory, count);
            assert!(observations.unchanged().unwrap());
            std::fs::write(&paths[changed], "export const x = 1;").unwrap();
            unreadable(&paths[failed]);
            let result = observations.unchanged();
            if changed < failed {
                assert!(
                    matches!(result, Ok(false)),
                    "a change at {changed} precedes a failure at {failed}: {result:?}"
                );
            } else {
                let error = result.expect_err("a failure precedes the change");
                assert_eq!(error.kind, InputErrorKind::Io);
                assert_eq!(error.path.as_ref(), Some(&paths[failed]));
            }
        }
    }

    /// One change anywhere among many unchanged observations is found.
    #[test]
    fn a_single_change_is_found_at_every_position() {
        let count = 2000;
        for changed in [0, 1, 999, 1998, 1999] {
            let directory = Directory::new("single");
            let (paths, observations) = recorded(&directory, count);
            std::fs::write(&paths[changed], "export {}; ").unwrap();
            assert!(!observations.unchanged().unwrap(), "change at {changed}");
            std::fs::remove_file(&paths[changed]).unwrap();
            assert!(!observations.unchanged().unwrap(), "removal at {changed}");
        }
    }

    /// `first_decisive` answers as a loop in order would, wherever the
    /// answer falls among the batches threads claim and with any number of
    /// threads, also when a later item answers too.
    #[test]
    fn first_decisive_answers_like_a_loop_at_every_position() {
        for threads in [1, 2, 3, 8] {
            for count in (0..=130).chain([3 * 8 * VALIDATION_BATCH + 5]) {
                let items: Vec<usize> = (0..count).collect();
                let none = first_decisive(threads, &items, |_, _| Outcome::<usize>::Same);
                assert_eq!(none, None, "{threads} threads, {count} items");
                for answer in 0..count {
                    let found = first_decisive(threads, &items, |&item, _| {
                        if item == answer || item + 1 == count {
                            Outcome::Decisive(item)
                        } else {
                            Outcome::Same
                        }
                    });
                    assert_eq!(found, Some(answer), "{threads} threads, {count} items");
                }
            }
        }
    }

    /// An item `decide` defers on a speculative reading is decided by a
    /// blocking reading, in order, only if no earlier item answered: as a
    /// loop in order would have read it.
    #[test]
    fn deferred_items_are_decided_in_order_only_where_a_loop_reaches_them() {
        for threads in [1, 2, 8] {
            for (deferred, answer, expected) in [(5, 300, 5), (300, 5, 5), (5, 300, 300)] {
                let items: Vec<usize> = (0..400).collect();
                let blocking = Mutex::new(Vec::new());
                // In the third case the deferred item, decided, is the same.
                let deferred_answers = expected == deferred;
                let found = first_decisive(threads, &items, |&item, reading| {
                    if item == deferred {
                        match reading {
                            Reading::Speculative => return Outcome::Deferred,
                            Reading::Blocking | Reading::InOrder(_) => {
                                blocking.lock().unwrap().push(item);
                                if deferred_answers {
                                    return Outcome::Decisive(item);
                                }
                                return Outcome::Same;
                            }
                        }
                    }
                    if item == answer {
                        Outcome::Decisive(item)
                    } else {
                        Outcome::Same
                    }
                });
                assert_eq!(
                    found,
                    Some(expected),
                    "{threads} threads, deferred {deferred}"
                );
                let blocking = blocking.into_inner().unwrap();
                if deferred < answer {
                    assert_eq!(blocking, [deferred]);
                } else {
                    assert!(blocking.is_empty(), "a later deferred item was read");
                }
            }
        }
    }

    #[cfg(unix)]
    fn pipe(path: &Path) {
        std::fs::remove_file(path).unwrap();
        let made = std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap();
        assert!(made.success());
    }

    /// Let a reader waiting on the pipe at `path` see its end; nothing when
    /// no reader waits.
    #[cfg(unix)]
    fn release(path: &Path) {
        use std::os::unix::fs::OpenOptionsExt;
        let _ = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(super::NONBLOCK.expect("a test platform's O_NONBLOCK"))
            .open(path);
    }

    /// Write `contents` to the pipe at `path` once a reader opens it, a
    /// second later.
    #[cfg(unix)]
    fn write_later(path: &Path, contents: &'static [u8]) -> std::thread::JoinHandle<()> {
        let path = path.to_path_buf();
        std::thread::spawn(move || {
            use std::io::Write;
            let mut pipe = std::fs::OpenOptions::new().write(true).open(path).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(1));
            let _ = pipe.write_all(contents);
        })
    }

    /// A later input whose read would wait, a pipe without a writer, cannot
    /// withhold the answer an earlier change decides, as a pass in key order
    /// would never read it. The earlier change itself arrives through a pipe
    /// a second late, while every other observation is long decided.
    #[cfg(unix)]
    #[test]
    fn a_later_read_that_would_wait_does_not_withhold_an_earlier_change() {
        let directory = Directory::new("pipe-later");
        let (paths, observations) = recorded(&directory, 600);
        pipe(&paths[3]);
        pipe(&paths[599]);
        let writer = write_later(&paths[3], b"export const x = 1;");
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(observations.unchanged().map_err(|error| error.kind));
        });
        let result = receiver.recv_timeout(std::time::Duration::from_secs(20));
        release(&paths[599]);
        writer.join().unwrap();
        assert!(matches!(result, Ok(Ok(false))), "{result:?}");
    }

    /// An earlier input whose read waits is still read, and decides before a
    /// later change: here its writer supplies bytes that are not UTF-8.
    #[cfg(unix)]
    #[test]
    fn an_earlier_read_that_waits_is_still_decided_first() {
        let directory = Directory::new("pipe-earlier");
        let (paths, observations) = recorded(&directory, 600);
        std::fs::write(&paths[400], "export const x = 1;").unwrap();
        pipe(&paths[2]);
        let writer = write_later(&paths[2], b"export const broken = \"\xff\";");
        let error = observations
            .unchanged()
            .expect_err("the pipe's contents fail");
        writer.join().unwrap();
        assert_eq!(error.kind, InputErrorKind::Io);
        assert_eq!(error.path.as_ref(), Some(&paths[2]));
    }

    /// One load answers each path's real path once: a later question gets
    /// the first answer, so the load sees one file system, and the check
    /// after loading still finds the path changed.
    #[cfg(unix)]
    #[test]
    fn a_load_answers_each_real_path_once() {
        let directory = Directory::new("canonical");
        let (first, second, link) = (
            directory.0.join("first.ts"),
            directory.0.join("second.ts"),
            directory.0.join("link.ts"),
        );
        std::fs::write(&first, "export {};").unwrap();
        std::fs::write(&second, "export {};").unwrap();
        std::os::unix::fs::symlink(&first, &link).unwrap();
        let disk = Disk::recording();
        let answer = disk.canonicalize(&link).unwrap();
        assert_eq!(answer, std::fs::canonicalize(&first).unwrap());
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&second, &link).unwrap();
        assert_eq!(disk.canonicalize(&link).unwrap(), answer);
        assert!(!disk.finish().unchanged().unwrap());
    }

    /// Validation reports staleness as soon as some observation differs,
    /// before the deciding one is known, and never for unchanged inputs.
    #[test]
    fn staleness_is_reported_once_an_observation_differs() {
        let directory = Directory::new("stale");
        let (paths, observations) = recorded(&directory, 500);
        let reported = std::sync::atomic::AtomicUsize::new(0);
        let report = || {
            reported.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        };
        assert!(observations.unchanged_with(&report).unwrap());
        assert_eq!(reported.load(std::sync::atomic::Ordering::SeqCst), 0);
        std::fs::write(&paths[250], "export const x = 1;").unwrap();
        assert!(!observations.unchanged_with(&report).unwrap());
        assert!(reported.load(std::sync::atomic::Ordering::SeqCst) > 0);
    }

    /// Items a speculative reading defers and a blocking one finds the same,
    /// such as unchanged unreadable inputs, cost a bounded number of
    /// decisions each, not one more pass over every later item.
    #[test]
    fn deferred_items_that_stay_the_same_are_decided_a_bounded_number_of_times() {
        for pattern in [1, 2, 7] {
            let items: Vec<usize> = (0..1000).collect();
            let decisions = std::sync::atomic::AtomicUsize::new(0);
            let found = first_decisive(8, &items, |&item, reading| {
                decisions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                match reading {
                    Reading::Speculative if item % pattern == 0 => Outcome::Deferred,
                    _ => Outcome::<usize>::Same,
                }
            });
            assert_eq!(found, None);
            let decisions = decisions.into_inner();
            assert!(
                decisions <= 3 * items.len(),
                "{decisions} decisions, deferring every {pattern}"
            );
        }
    }

    /// Staleness is reported while an earlier read still waits: here the
    /// writer of a pipe that sorts first answers only once it has been.
    #[cfg(unix)]
    #[test]
    fn staleness_is_reported_while_an_earlier_read_still_waits() {
        let directory = Directory::new("stale-early");
        let (paths, observations) = recorded(&directory, 600);
        pipe(&paths[0]);
        std::fs::write(&paths[300], "export const x = 1;").unwrap();
        let reported = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let writer = {
            let (path, reported) = (paths[0].clone(), reported.clone());
            std::thread::spawn(move || {
                use std::io::Write;
                let mut pipe = std::fs::OpenOptions::new().write(true).open(path).unwrap();
                let started = std::time::Instant::now();
                while !reported.load(std::sync::atomic::Ordering::SeqCst)
                    && started.elapsed() < std::time::Duration::from_secs(20)
                {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                let seen = reported.load(std::sync::atomic::Ordering::SeqCst);
                let _ = pipe.write_all(b"export {};");
                seen
            })
        };
        let result = observations
            .unchanged_with(&|| reported.store(true, std::sync::atomic::Ordering::SeqCst));
        assert!(
            writer.join().unwrap(),
            "staleness waited for the earlier read"
        );
        assert!(!result.unwrap());
    }
}
