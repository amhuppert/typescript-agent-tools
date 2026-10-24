//! File system probes for loading. What counts as absent is
//! `crate::input::is_absence`: any other failure, such as a permission
//! error, means an input may exist but cannot be inspected; loading reports
//! it as `Io` rather than resolving past it.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
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
    /// they find after the deciding observation has no effect.
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
        let decisive =
            first_decisive(
                threads,
                &observations,
                |((path, operation), before)| match still(path, *operation, before) {
                    Ok(true) => None,
                    decided => {
                        stale();
                        Some(decided)
                    }
                },
            );
        decisive.unwrap_or(Ok(true))
    }
}

/// Whether `path` still answers `operation` with `before`. Absence is an
/// answer; another failure is one only where loading recorded a failure,
/// and otherwise an error.
fn still(
    path: &Path,
    operation: Operation,
    before: &Observation,
) -> Result<bool, super::InputError> {
    let result = match operation {
        Operation::Text => read(path)
            .and_then(crate::source::decode_text)
            .map(|s| Observation::Text(s.into())),
        Operation::Bytes => read(path).map(|b| Observation::Bytes(b.into())),
        Operation::Entries => directory(path).map(|(_, names)| Observation::Entries(names)),
        Operation::Canonical => std::fs::canonicalize(path).map(Observation::Canonical),
        Operation::Entry => entry(path).map(Observation::Entry),
    };
    let after = match result {
        Ok(value) => value,
        Err(error) if is_absence(&error) => Observation::Absent,
        Err(error) if matches!(before, Observation::Failed(..)) => failure(&error),
        Err(error) => return Err(super::io_error(path, &error)),
    };
    Ok(*before == after)
}

/// The contents of `path`, which must name a regular file: the one owner of
/// that rule for every read loading and validation make. A pipe, a device,
/// a socket, or a directory is refused before it is opened, as
/// `InvalidInput`, "not a regular file" (ENVELOPE.md, "Accepted inputs"),
/// where TypeScript's `ReadFile` would read it, waiting on a pipe for its
/// writer. The check follows symbolic links, as loading does everywhere. A
/// path replaced by a pipe between the check and the open can still block
/// the read; ENVELOPE.md accepts this, since a hostile filesystem layout is
/// outside the envelope.
fn read(path: &Path) -> io::Result<Vec<u8>> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    std::fs::read(path)
}

/// `decide`'s answer for the first of `items` it answers, as a loop in
/// order would find it, computed on up to `threads` threads. Every item
/// before the answered one is decided; later items may be too, and those
/// answers are discarded.
fn first_decisive<T: Sync, R: Send>(
    threads: usize,
    items: &[T],
    decide: impl Fn(&T) -> Option<R> + Sync,
) -> Option<R> {
    let next = AtomicUsize::new(0);
    // Items at or after the earliest answer found so far need no decision.
    let bound = AtomicUsize::new(items.len());
    let found: Mutex<Option<(usize, R)>> = Mutex::new(None);
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
                if let Some(answer) = decide(item) {
                    let mut found = found.lock().expect("no thread panics holding the answer");
                    if found.as_ref().is_none_or(|&(at, _)| index < at) {
                        *found = Some((index, answer));
                        bound.fetch_min(index, Ordering::Relaxed);
                    }
                    break;
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
    found
        .into_inner()
        .expect("no thread panics holding the answer")
        .map(|(_, answer)| answer)
}

impl Disk {
    pub(in crate::project) fn recording() -> Self {
        Self {
            observations: RefCell::new(Some(Observations::default())),
            receipt: RefCell::default(),
            real_paths: RefCell::default(),
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
    /// effects on the current load: a changed or failed probe means the
    /// resolver must perform its ordinary lookup, which owns demand and error
    /// order. Real paths already memoized by this
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
            if !matches!(still(path, *operation, before), Ok(true)) {
                return false;
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

    /// A file's text as TypeScript reads it (`source::read_text`), of a
    /// regular file only (`read`).
    pub(super) fn read_to_string(&self, path: impl AsRef<Path>) -> io::Result<Arc<str>> {
        self.receive(TextRead::new(path.as_ref()))
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

    /// A file's bytes, of a regular file only (`read`).
    pub(super) fn read(&self, path: impl AsRef<Path>) -> io::Result<Arc<[u8]>> {
        let path = path.as_ref();
        let result = read(path).map(Arc::<[u8]>::from);
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

    /// Native `vfs.Common.FileExists`: every entry but a directory. One that
    /// is not a regular file is found, and refused where it is read (`read`).
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
    /// The read of `path`'s text, as `source::read_text` reads it, of a
    /// regular file only (`read`).
    pub(super) fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            text: read(path)
                .and_then(crate::source::decode_text)
                .map(Arc::<str>::from),
        }
    }

    /// The text read, before any load observes it.
    pub(super) fn text(&self) -> Result<&Arc<str>, &io::Error> {
        self.text.as_ref()
    }

    /// Share an earlier allocation only after this read returned the exact
    /// same decoded text. The current read (including any failure) still
    /// happens at its normal place in loading.
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
    use super::{Disk, VALIDATION_BATCH, first_decisive};
    use crate::analysis::InputErrorKind;
    use std::path::PathBuf;

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
                let none = first_decisive(threads, &items, |_| None::<usize>);
                assert_eq!(none, None, "{threads} threads, {count} items");
                for answer in 0..count {
                    let found = first_decisive(threads, &items, |&item| {
                        (item == answer || item + 1 == count).then_some(item)
                    });
                    assert_eq!(found, Some(answer), "{threads} threads, {count} items");
                }
            }
        }
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
}
