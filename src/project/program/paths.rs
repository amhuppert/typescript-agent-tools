//! Absolute `/`-separated path strings, normalized lexically as TypeScript
//! does. Configuration patterns, module candidates, and extension rewriting
//! are string operations; only file system calls convert to `Path`.

use crate::analysis::{InputError, InputErrorKind};
use crate::input::Case;
use crate::project::sources::FileId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Captured spellings of searched files. Both construction and queries use
/// the loader's normalization and case policy; lookup reads no files.
#[derive(Debug)]
pub(crate) struct FileLookup {
    case: Case,
    files: HashMap<String, FileId>,
}

impl FileLookup {
    pub(super) fn new(case: Case) -> Self {
        Self {
            case,
            files: HashMap::new(),
        }
    }

    /// Record `path`, an absolute spelling of `file`.
    pub(super) fn insert(&mut self, path: &str, file: FileId) {
        self.files.insert(self.case.key(path), file);
    }

    /// The searched file a query path names, matched lexically once made
    /// absolute (`absolute`), or `None` for none, including an empty path.
    /// A query path names a file only by its exact text: one that is not
    /// Unicode, which a lossy conversion could turn into another file's
    /// name, is `Unsupported` (`input::exact_path`), not any file.
    pub(crate) fn find(&self, path: &Path) -> Result<Option<FileId>, InputError> {
        if path.as_os_str().is_empty() {
            return Ok(None);
        }
        let absolute = Self::absolute(path)?;
        let text = from_path(&absolute)?;
        Ok(self.files.get(&self.case.key(&text)).copied())
    }

    /// A query path made absolute against the working directory, without
    /// resolving `..` or links: every operation's rule, which a request
    /// applies before another process answers it. An empty path stays
    /// empty, naming no file; a working directory that cannot be read is an
    /// I/O failure, never another directory's file or none.
    pub(crate) fn absolute(path: &Path) -> Result<PathBuf, InputError> {
        if path.as_os_str().is_empty() {
            return Ok(PathBuf::new());
        }
        std::path::absolute(path).map_err(|error| InputError {
            kind: InputErrorKind::Io,
            message: format!("cannot make the path absolute: {error}"),
            path: Some(path.to_path_buf()),
            position: None,
        })
    }
}

impl Case {
    /// A normalized absolute identity key.
    pub(super) fn key(self, path: &str) -> String {
        self.fold(&normalize(path)).into_owned()
    }
}

/// A directory entry's name as exact text. A name that is not Unicode,
/// possible on Linux, would change under a lossy conversion and then name
/// another entry or none, so `Err` holds the lossy form for messages only.
pub(super) fn exact_name(name: std::ffi::OsString) -> Result<String, String> {
    name.into_string()
        .map_err(|name| name.to_string_lossy().into_owned())
}

/// The error for a name `exact_name` could not represent, where it would be
/// used.
pub(super) fn inexact_name(directory: &str, lossy: &str) -> InputError {
    InputError {
        kind: InputErrorKind::Unsupported,
        message: format!("'{lossy}' is not a Unicode file name, so it cannot be named exactly"),
        path: Some(Path::new(directory).join(lossy)),
        position: None,
    }
}

/// A native path as loader text: its exact text (`input::exact_path`), so a
/// path that is not Unicode names no input, with `\\` written as `/` on
/// Windows.
pub(super) fn from_path(path: &Path) -> Result<String, InputError> {
    crate::input::exact_path(path).map(native_text)
}

/// Native path text with `/` separators: `\\` written as `/` on Windows.
pub(super) fn native_text(text: &str) -> String {
    if cfg!(windows) {
        separators(text)
    } else {
        text.to_owned()
    }
}

/// The length of a root prefix: `/`, or a drive such as `C:/`, with either
/// separator, as TypeScript's `getEncodedRootLength` reads one.
fn root_length(path: &str) -> usize {
    let bytes = path.as_bytes();
    let separator = |byte: u8| byte == b'/' || byte == b'\\';
    if bytes.first().copied().is_some_and(separator) {
        1
    } else if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && separator(bytes[2])
    {
        3
    } else {
        0
    }
}

pub(super) fn is_rooted(path: &str) -> bool {
    root_length(path) > 0
}

/// `./x`, `../x`, `.`, or `..`, with either separator: TypeScript's
/// `pathIsRelative`.
pub(super) fn is_relative(path: &str) -> bool {
    let rest = path
        .strip_prefix("..")
        .or_else(|| path.strip_prefix('.'))
        .unwrap_or(path);
    rest.len() < path.len() && (rest.is_empty() || rest.starts_with(['/', '\\']))
}

/// TypeScript's `normalizeSlashes`: `\\` separators written as `/`. Path
/// operations here (`combine`, `normalize`, `resolve`) apply it themselves,
/// as TypeScript's `combinePaths` and `normalizePath` do.
pub(super) fn separators(text: &str) -> String {
    text.replace('\\', "/")
}

/// A directory path without a trailing separator, except at a root such as
/// `/` or `C:/`, so a directory joins a relative name with one `/`.
pub(super) fn directory(mut path: String) -> String {
    while path.len() > root_length(&path) && path.ends_with('/') {
        path.pop();
    }
    path
}

/// Resolves `.` and `..` segments and repeated separators, keeping a
/// trailing separator.
pub(super) fn normalize(path: &str) -> String {
    let path = &separators(path);
    let root = root_length(path);
    let mut parts: Vec<&str> = Vec::new();
    for part in path[root..].split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            ".." if root > 0 => {}
            _ => parts.push(part),
        }
    }
    let mut result = format!("{}{}", &path[..root], parts.join("/"));
    if path.ends_with('/') && !result.is_empty() && !result.ends_with('/') {
        result.push('/');
    }
    result
}

/// TypeScript's `tspath.CombinePaths` for one trailing path, which an empty
/// one leaves out, so `combine("/a", "")` is `/a`.
pub(super) fn combine(base: &str, relative: &str) -> String {
    let (base, relative) = (&separators(base), &separators(relative));
    if relative.is_empty() {
        base.to_owned()
    } else if is_rooted(relative) || base.is_empty() {
        relative.to_owned()
    } else if base.ends_with('/') {
        format!("{base}{relative}")
    } else {
        format!("{base}/{relative}")
    }
}

/// `combine` followed by `normalize`.
pub(super) fn resolve(base: &str, relative: &str) -> String {
    normalize(&combine(base, relative))
}

pub(super) fn parent(path: &str) -> &str {
    let trimmed = path.strip_suffix('/').unwrap_or(path);
    match trimmed.rfind('/') {
        Some(index) if index < root_length(trimmed) => &trimmed[..index + 1],
        Some(index) => &trimmed[..index],
        None => "",
    }
}

pub(super) fn basename(path: &str) -> &str {
    let trimmed = path.strip_suffix('/').unwrap_or(path);
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

/// `path` and each ancestor directory, nearest first.
pub(super) fn ancestors(path: &str) -> impl Iterator<Item = &str> {
    let mut next = Some(
        path.strip_suffix('/')
            .filter(|p| !p.is_empty())
            .unwrap_or(path),
    );
    std::iter::from_fn(move || {
        let current = next?;
        let up = parent(current);
        next = (!up.is_empty() && up != current).then_some(up);
        Some(current)
    })
}

/// A `/`-separated relative path from directory `from` to `to`, using `../`
/// where needed; `to` itself when the roots differ.
pub(super) fn relative(from: &str, to: &str) -> String {
    let (from_root, to_root) = (root_length(from), root_length(to));
    if from[..from_root] != to[..to_root] {
        return to.to_owned();
    }
    let from: Vec<&str> = from[from_root..]
        .split('/')
        .filter(|p| !p.is_empty())
        .collect();
    let to_parts: Vec<&str> = to[to_root..].split('/').filter(|p| !p.is_empty()).collect();
    let common = from
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend(&to_parts[common..]);
    parts.join("/")
}

pub(super) fn has_node_modules(path: &str) -> bool {
    path.split('/').any(|part| part == "node_modules")
}

/// The TypeScript-recognized extensions, longest declaration forms first, as
/// used to strip or replace an extension.
const KNOWN_EXTENSIONS: [&str; 12] = [
    ".d.ts", ".d.mts", ".d.cts", ".mjs", ".mts", ".cjs", ".cts", ".ts", ".js", ".tsx", ".jsx",
    ".json",
];

pub(super) fn known_extension(path: &str) -> Option<&'static str> {
    KNOWN_EXTENSIONS
        .into_iter()
        .find(|extension| path.ends_with(extension))
}

/// Replaces a known extension; paths without one are unchanged.
pub(super) fn change_extension(path: &str, extension: &str) -> String {
    match known_extension(path) {
        Some(old) => format!("{}{extension}", &path[..path.len() - old.len()]),
        None => path.to_owned(),
    }
}

pub(super) fn has_extension(path: &str, extensions: &[&str]) -> bool {
    extensions.iter().any(|extension| path.ends_with(extension))
}

/// The parser's rule, so loading and parsing agree on declaration files.
pub(super) fn is_declaration(path: &str) -> bool {
    crate::declaration_file_name(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_relates_paths_lexically() {
        assert_eq!(normalize("/a/./b/../c//d/"), "/a/c/d/");
        assert_eq!(normalize("/../a"), "/a");
        assert_eq!(normalize("../a/../../b"), "../../b");
        assert_eq!(resolve("/a/b", "../c.ts"), "/a/c.ts");
        assert_eq!(resolve("/a/b", "/x"), "/x");
        assert_eq!(combine("/a/b", ""), "/a/b");
        assert_eq!(combine("", "c"), "c");
        assert_eq!(parent("/a/b"), "/a");
        assert_eq!(parent("/a"), "/");
        assert_eq!(basename("/a/b.d.ts"), "b.d.ts");
        assert_eq!(ancestors("/a/b").collect::<Vec<_>>(), ["/a/b", "/a", "/"]);
        assert_eq!(relative("/a/b", "/a/c/d.ts"), "../c/d.ts");
        assert_eq!(relative("/a/b", "/a/b/d.ts"), "d.ts");
        assert_eq!(change_extension("/a/x.d.ts", ".ts"), "/a/x.ts");
        assert_eq!(change_extension("/a/x.d.css.ts", ".tsx"), "/a/x.d.css.tsx");
        assert!(is_declaration("/a/x.d.css.ts"));
        assert!(!is_declaration("/a.d.b/x.ts"));
        assert_eq!(directory("/a/b//".into()), "/a/b");
        assert_eq!(directory("/".into()), "/");
        assert_eq!(directory("C:/".into()), "C:/");
        assert_eq!(separators("a\\b/c"), "a/b/c");
    }
}
