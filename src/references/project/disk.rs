//! File system probes, and the one rule for what counts as absent. Only
//! `NotFound` and `NotADirectory` mean a path names nothing. Any other
//! failure, such as a permission error, means an input may exist but cannot
//! be inspected; loading reports it as `Io` rather than resolving past it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Filesystem access for one load. Recording is explicit and local to that load;
/// the ordinary immutable-project API does not retain observations.
#[derive(Default)]
pub(in crate::references) struct Disk {
    observations: RefCell<Option<Observations>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Operation {
    Text,
    Bytes,
    Entries,
    Canonical,
    Entry,
}

#[derive(Debug, PartialEq, Eq)]
enum Observation {
    Text(Arc<str>),
    Bytes(Arc<[u8]>),
    Entries(Vec<OsString>),
    Canonical(PathBuf),
    Entry(Option<Entry>),
    Absent,
}

/// The actual answers used during construction, including unsuccessful probes.
/// No timestamps or content hashes stand in for exact captured contents.
#[derive(Debug, Default)]
pub(in crate::references) struct Observations {
    values: BTreeMap<(PathBuf, Operation), Observation>,
    inconsistent: bool,
}

impl Observations {
    pub(in crate::references) fn unchanged(&self) -> Result<bool, super::ProjectError> {
        if self.inconsistent {
            return Ok(false);
        }
        for ((path, operation), before) in &self.values {
            let result = match operation {
                Operation::Text => {
                    std::fs::read_to_string(path).map(|s| Observation::Text(s.into()))
                }
                Operation::Bytes => std::fs::read(path).map(|b| Observation::Bytes(b.into())),
                Operation::Entries => directory(path).map(|(_, names)| Observation::Entries(names)),
                Operation::Canonical => std::fs::canonicalize(path).map(Observation::Canonical),
                Operation::Entry => entry(path).map(Observation::Entry),
            };
            let after = match result {
                Ok(value) => value,
                Err(error) if is_absence(&error) => Observation::Absent,
                Err(error) => return Err(super::io_error(path, &error)),
            };
            if *before != after {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl Disk {
    pub(in crate::references) fn recording() -> Self {
        Self {
            observations: RefCell::new(Some(Observations::default())),
        }
    }

    pub(in crate::references) fn finish(self) -> Observations {
        self.observations
            .into_inner()
            .expect("a recording disk owns observations")
    }

    fn record(&self, path: &Path, operation: Operation, result: Result<Observation, &io::Error>) {
        let mut observations = self.observations.borrow_mut();
        let Some(observations) = observations.as_mut() else {
            return;
        };
        let value = match result {
            Ok(value) => value,
            Err(error) if is_absence(error) => Observation::Absent,
            // The caller owns reporting this failure; failed loads are not cached.
            Err(_) => return,
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

    pub(super) fn read_to_string(&self, path: impl AsRef<Path>) -> io::Result<Arc<str>> {
        let path = path.as_ref();
        let result = std::fs::read_to_string(path).map(Arc::<str>::from);
        self.record(
            path,
            Operation::Text,
            result.as_ref().map(|s| Observation::Text(s.clone())),
        );
        result
    }

    pub(super) fn read(&self, path: impl AsRef<Path>) -> io::Result<Arc<[u8]>> {
        let path = path.as_ref();
        let result = std::fs::read(path).map(Arc::<[u8]>::from);
        self.record(
            path,
            Operation::Bytes,
            result.as_ref().map(|b| Observation::Bytes(b.clone())),
        );
        result
    }

    pub(super) fn canonicalize(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
        let path = path.as_ref();
        let result = std::fs::canonicalize(path);
        self.record(
            path,
            Operation::Canonical,
            result.as_ref().map(|p| Observation::Canonical(p.clone())),
        );
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

    pub(super) fn is_dir(&self, path: &str) -> io::Result<bool> {
        Ok(self.entry(Path::new(path))? == Some(Entry::Directory))
    }
}

fn directory(path: &Path) -> io::Result<(Vec<std::fs::DirEntry>, Vec<OsString>)> {
    let entries = std::fs::read_dir(path)?.collect::<io::Result<Vec<_>>>()?;
    let mut names: Vec<_> = entries.iter().map(std::fs::DirEntry::file_name).collect();
    names.sort();
    Ok((entries, names))
}

/// Whether a file system error means the path names nothing.
pub(super) fn is_absence(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

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
