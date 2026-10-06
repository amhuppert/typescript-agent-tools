//! File system probes, and the one rule for what counts as absent. Only
//! `NotFound` and `NotADirectory` mean a path names nothing. Any other
//! failure, such as a permission error, means an input may exist but cannot
//! be inspected; loading reports it as `Io` rather than resolving past it.

use std::io;
use std::path::Path;

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
pub(super) fn entry(path: &Path) -> io::Result<Option<Entry>> {
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

pub(super) fn is_file(path: &str) -> io::Result<bool> {
    Ok(entry(Path::new(path))? == Some(Entry::File))
}

pub(super) fn is_dir(path: &str) -> io::Result<bool> {
    Ok(entry(Path::new(path))? == Some(Entry::Directory))
}
