//! Absolute `/`-separated path strings, normalized lexically as TypeScript
//! does. Configuration patterns, module candidates, and extension rewriting
//! are string operations; only file system calls convert to `Path`.

use crate::analysis::{InputError, InputErrorKind};
use crate::input::Case;
use crate::project::sources::FileId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Captured input spellings and query membership. Construction and queries use
/// the loader's normalization and case policy; lookup reads no files.
#[derive(Debug)]
pub(crate) struct FileLookup {
    case: Case,
    files: HashMap<String, (FileId, bool)>,
}

impl FileLookup {
    pub(super) fn new(case: Case) -> Self {
        Self {
            case,
            files: HashMap::new(),
        }
    }

    /// Record `path`, an absolute spelling of `file`.
    pub(super) fn insert(&mut self, path: &str, file: FileId, searched: bool) {
        self.files.insert(self.case.key(path), (file, searched));
    }

    /// The searched file a query path names, matched lexically once made
    /// absolute (`absolute`), or `None` for none, including an empty path.
    /// A query path names a file only by its exact text: one that is not
    /// Unicode, which a lossy conversion could turn into another file's
    /// name, is `Unsupported` (`input::exact_path`), not any file.
    pub(crate) fn find(&self, path: &Path) -> Result<Option<FileId>, InputError> {
        Ok(self
            .entry(path)?
            .and_then(|(file, searched)| searched.then_some(file)))
    }

    fn entry(&self, path: &Path) -> Result<Option<(FileId, bool)>, InputError> {
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
        let path = normalize(path);
        match self.fold(&path) {
            std::borrow::Cow::Borrowed(_) => path,
            std::borrow::Cow::Owned(folded) => folded,
        }
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
pub(in crate::project) fn from_path(path: &Path) -> Result<String, InputError> {
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

/// `GetEncodedRootLength`: disk and URL roots retain separate identities.
/// A negative result encodes a URL root by bitwise complement, as in native.
fn encoded_root_length(path: &str) -> isize {
    let bytes = path.as_bytes();
    let Some(&first) = bytes.first() else {
        return 0;
    };
    if matches!(first, b'/' | b'\\') {
        if bytes.get(1) != Some(&first) {
            return 1;
        }
        return bytes[2..]
            .iter()
            .position(|&byte| byte == first)
            .map_or(bytes.len(), |at| at + 3) as isize;
    }
    if first.is_ascii_alphabetic() && bytes.get(1) == Some(&b':') {
        if bytes.len() == 2 {
            return 2;
        }
        if matches!(bytes.get(2), Some(b'/' | b'\\')) {
            return 3;
        }
    }
    if path.starts_with("^/") {
        return 2;
    }
    if let Some(scheme_end) = path.find("://") {
        let authority_start = scheme_end + 3;
        if let Some(length) = path[authority_start..].find('/') {
            let authority_end = authority_start + length;
            let authority = &path[authority_start..authority_end];
            if &path[..scheme_end] == "file"
                && matches!(authority, "" | "localhost")
                && bytes
                    .get(authority_end + 1)
                    .is_some_and(u8::is_ascii_alphabetic)
            {
                let separator = authority_end + 2;
                let end = if bytes.get(separator) == Some(&b':') {
                    Some(separator + 1)
                } else if path
                    .get(separator..separator + 3)
                    .is_some_and(|text| text == "%3a" || text == "%3A")
                {
                    Some(separator + 3)
                } else {
                    None
                };
                if let Some(end) = end {
                    if end == bytes.len() {
                        return !(end as isize);
                    }
                    if bytes.get(end) == Some(&b'/') {
                        return !((end + 1) as isize);
                    }
                }
            }
            return !((authority_end + 1) as isize);
        }
        return !(bytes.len() as isize);
    }
    0
}

fn root_length(path: &str) -> usize {
    let length = encoded_root_length(path);
    if length < 0 {
        (!length) as usize
    } else {
        length as usize
    }
}

/// Native `PathIsAbsolute`, including URL roots.
pub(super) fn is_rooted(path: &str) -> bool {
    encoded_root_length(path) != 0
}

/// Native `IsRootedDiskPath`, excluding URL roots.
pub(super) fn is_rooted_disk(path: &str) -> bool {
    encoded_root_length(path) > 0
}

/// `./x`, `../x`, `.`, or `..`, with either separator: TypeScript's
/// `pathIsRelative`.
pub(in crate::project) fn is_relative(path: &str) -> bool {
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

/// TypeScript's `NormalizePath` / `simpleNormalizePath`: resolves `.` and
/// `..` segments and repeated separators, keeping a trailing separator.
pub(super) fn normalize(path: &str) -> String {
    let path = separators(path);
    if !has_relative_path_segment(&path) {
        return path;
    }
    let root = root_length(&path);
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
    if root_length(path) == path.len() {
        return path;
    }
    let trimmed = path.strip_suffix('/').unwrap_or(path);
    match trimmed.rfind('/') {
        Some(index) if index < root_length(trimmed) => &trimmed[..index + 1],
        Some(index) => &trimmed[..index],
        None => "",
    }
}

pub(super) fn basename(path: &str) -> &str {
    if root_length(path) == path.len() {
        return "";
    }
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
    relative_with_case(from, to, "", Case::Sensitive)
}

/// `GetPathComponentsRelativeTo` / `GetRelativePathFromDirectory`. Roots
/// always use EqualFold; non-root components use the host's equality policy.
/// The operation also serves GetRelativePathToDirectoryOrUrl with a cwd.
pub(super) fn relative_with_case(from: &str, to: &str, cwd: &str, case: Case) -> String {
    let (from, to) = (resolve(cwd, from), resolve(cwd, to));
    let (from_root, to_root) = (root_length(&from), root_length(&to));
    if !text_equal(&from[..from_root], &to[..to_root], Case::Insensitive) {
        return if to_root == to.len() {
            if to.ends_with('/') {
                to
            } else {
                format!("{to}/")
            }
        } else {
            to.trim_end_matches('/').to_owned()
        };
    }
    let from: Vec<_> = from[from_root..]
        .split('/')
        .filter(|p| !p.is_empty())
        .collect();
    let target: Vec<_> = to[to_root..].split('/').filter(|p| !p.is_empty()).collect();
    let common = from
        .iter()
        .zip(&target)
        .take_while(|(a, b)| text_equal(a, b, case))
        .count();
    let mut components = vec![".."; from.len() - common];
    components.extend(&target[common..]);
    components.join("/")
}

/// Native `ContainsPath`, distinct from canonical Path.ContainsPath and
/// StartsWithDirectory. Components are reduced; roots always ignore case.
pub(super) fn contains(parent: &str, child: &str, cwd: &str, case: Case) -> bool {
    let (parent, child) = (combine(cwd, parent), combine(cwd, child));
    if parent.is_empty() || child.is_empty() {
        return false;
    }
    if parent == child {
        return true;
    }
    let (parent, child) = (normalize(&parent), normalize(&child));
    let (a, b) = (root_length(&parent), root_length(&child));
    if !text_equal(&parent[..a], &child[..b], Case::Insensitive) {
        return false;
    }
    let mut children = child[b..].split('/').filter(|part| !part.is_empty());
    parent[a..]
        .split('/')
        .filter(|part| !part.is_empty())
        .all(|part| {
            children
                .next()
                .is_some_and(|child| text_equal(part, child, case))
        })
}

/// Native `StartsWithDirectory`: strict directory ancestry on canonical
/// filename text, without lexical reduction or EqualFold comparison.
pub(super) fn starts_with_directory(file: &str, directory: &str, case: Case) -> bool {
    if directory.is_empty() {
        return false;
    }
    let file = case.fold(file);
    let directory = case.fold(directory);
    let directory = directory.strip_suffix('/').unwrap_or(&directory);
    let directory = directory.strip_suffix('\\').unwrap_or(directory);
    file.starts_with(&format!("{directory}/")) || file.starts_with(&format!("{directory}\\"))
}

/// `ComparePaths` uses a simple-lowercase ordering, which is deliberately
/// different from EqualFold and from canonical filename equality.
pub(super) fn compare(a: &str, b: &str, cwd: &str, case: Case) -> std::cmp::Ordering {
    let (a, b) = (combine(cwd, a), combine(cwd, b));
    if a == b {
        return std::cmp::Ordering::Equal;
    }
    if a.is_empty() || b.is_empty() {
        return a.cmp(&b);
    }
    let (ar, br) = (root_length(&a), root_length(&b));
    let root = text_compare(&a[..ar], &b[..br], Case::Insensitive);
    if !root.is_eq() {
        return root;
    }
    let (arest, brest) = (&a[ar..], &b[br..]);
    if !has_relative_path_segment(arest) && !has_relative_path_segment(brest) {
        return text_compare(arest, brest, case);
    }
    let (a, b) = (normalize(&a), normalize(&b));
    let mut aa = a[root_length(&a)..]
        .split('/')
        .filter(|part| !part.is_empty());
    let mut bb = b[root_length(&b)..]
        .split('/')
        .filter(|part| !part.is_empty());
    loop {
        match (aa.next(), bb.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(a), Some(b)) => {
                let order = text_compare(a, b, case);
                if !order.is_eq() {
                    return order;
                }
            }
        }
    }
}

/// Native `hasRelativePathSegment`. A single trailing separator does not
/// enter normalization or ComparePaths' reduction branch; a duplicate does.
fn has_relative_path_segment(path: &str) -> bool {
    matches!(path, "." | "..")
        || path.starts_with("./")
        || path.starts_with("../")
        || path.ends_with("/.")
        || path.ends_with("/..")
        || path.contains("//")
        || path.contains("/./")
        || path.contains("/../")
}

fn text_compare(a: &str, b: &str, case: Case) -> std::cmp::Ordering {
    match case {
        Case::Sensitive => a.cmp(b),
        Case::Insensitive => a
            .chars()
            .map(crate::input::simple_lowercase)
            .cmp(b.chars().map(crate::input::simple_lowercase)),
    }
}

/// `stringutil.HasPrefix` / `HasSuffix` compare exact byte windows with
/// EqualFold. A window splitting a UTF-8 scalar cannot match valid text.
pub(super) fn has_prefix(text: &str, prefix: &str, case: Case) -> bool {
    text.get(..prefix.len())
        .is_some_and(|part| text_equal(part, prefix, case))
}
pub(super) fn has_suffix(text: &str, suffix: &str, case: Case) -> bool {
    text.len()
        .checked_sub(suffix.len())
        .and_then(|at| text.get(at..))
        .is_some_and(|part| text_equal(part, suffix, case))
}
pub(super) fn matches_edges(text: &str, prefix: &str, suffix: &str, case: Case) -> bool {
    prefix.len() + suffix.len() <= text.len()
        && has_prefix(text, prefix, case)
        && has_suffix(text, suffix, case)
}

/// `stringutil.EquateStringCaseInsensitive` delegates to Go EqualFold.
fn text_equal(a: &str, b: &str, case: Case) -> bool {
    a == b
        || case == Case::Insensitive
            && a.chars()
                .map(crate::input::simple_case_fold)
                .eq(b.chars().map(crate::input::simple_case_fold))
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

/// `tspath.RemoveFileExtension` removes the first recognized exact suffix,
/// including the complete declaration extension, and preserves unknown ones.
pub(in crate::project) fn remove_file_extension(path: &str) -> &str {
    known_extension(path).map_or(path, |extension| &path[..path.len() - extension.len()])
}

/// Replaces a known extension; paths without one are unchanged.
pub(super) fn change_extension(path: &str, extension: &str) -> String {
    let trimmed = path.strip_suffix(['/', '\\']).unwrap_or(path);
    match known_extension(trimmed) {
        Some(old) => {
            let prefix = &path[..path.len() - old.len()];
            if extension.is_empty() || extension.starts_with('.') {
                format!("{prefix}{extension}")
            } else {
                format!("{prefix}.{extension}")
            }
        }
        None => path.to_owned(),
    }
}

pub(super) fn has_extension(path: &str, extensions: &[&str]) -> bool {
    extensions
        .iter()
        .any(|extension| path.len() > extension.len() && path.ends_with(extension))
}

/// `TryGetExtensionFromPath` applies FileExtensionIs's nonempty stem gate;
/// RemoveFileExtension intentionally uses the broader suffix-only operation.
pub(super) fn try_extension(path: &str) -> Option<&'static str> {
    KNOWN_EXTENSIONS
        .into_iter()
        .find(|ext| has_extension(path, &[*ext]))
}

pub(super) const REQUIRED_EXTENSIONS: &[&str] =
    &[".mts", ".d.mts", ".mjs", ".cts", ".d.cts", ".cjs"];

/// Native `HasTSFileExtension` / `HasJSFileExtension` (not input kind,
/// whose case policy and extension question belong to input::InputKind).
pub(super) fn has_ts_extension(path: &str) -> bool {
    has_extension(
        path,
        &[".ts", ".tsx", ".d.ts", ".cts", ".d.cts", ".mts", ".d.mts"],
    )
}
pub(super) fn has_js_extension(path: &str) -> bool {
    has_extension(path, &[".js", ".jsx", ".mjs", ".cjs"])
}

/// `module.TryGetJSExtensionForFile`: JSX source output depends on exactly
/// JsxEmitPreserve; a .jsx input retains its extension in every JSX mode.
pub(super) fn js_extension(path: &str, jsx_preserve: bool) -> Option<&'static str> {
    match try_extension(path)? {
        ".ts" | ".d.ts" => Some(".js"),
        ".tsx" => Some(if jsx_preserve { ".jsx" } else { ".js" }),
        ext @ (".js" | ".jsx" | ".json") => Some(ext),
        ".mts" | ".d.mts" | ".mjs" => Some(".mjs"),
        ".cts" | ".d.cts" | ".cjs" => Some(".cjs"),
        _ => None,
    }
}

/// `outputpaths.GetOutputExtension` is an emit-path question, separate from
/// module resolution's TryGetJSExtensionForFile (.jsx differs).
pub(super) fn output_extension(path: &str, jsx_preserve: bool) -> &'static str {
    if has_extension(path, &[".json"]) {
        ".json"
    } else if jsx_preserve && has_extension(path, &[".jsx", ".tsx"]) {
        ".jsx"
    } else if has_extension(path, &[".mts", ".mjs"]) {
        ".mjs"
    } else if has_extension(path, &[".cts", ".cjs"]) {
        ".cjs"
    } else {
        ".js"
    }
}

/// `GetDeclarationEmitExtensionForPath`, including arbitrary-extension
/// declaration forms. Unknown extensionless files use .d.ts.
pub(super) fn declaration_emit_extension(path: &str) -> String {
    if has_extension(path, &[".mjs", ".mts"]) {
        return ".d.mts".into();
    }
    if has_extension(path, &[".cjs", ".cts"]) {
        return ".d.cts".into();
    }
    if has_extension(path, &[".ts", ".tsx", ".js", ".jsx"]) {
        return ".d.ts".into();
    }
    let base = basename(path);
    match base.rfind('.') {
        Some(dot) => format!(".d{}.ts", &base[dot..]),
        None => ".d.ts".into(),
    }
}

/// `ChangeFullExtension` recognizes the declaration suffix from `.d`.
pub(super) fn change_full_extension(path: &str, extension: &str) -> String {
    match crate::declaration_file_extension(path) {
        Some(old) => format!(
            "{}{}{extension}",
            &path[..path.len() - old.len()],
            if extension.starts_with('.') { "" } else { "." }
        ),
        None => change_extension(path, extension),
    }
}

/// The parser's rule, so loading and parsing agree on declaration files.
pub(super) fn is_declaration(path: &str) -> bool {
    crate::declaration_file_name(path)
}

/// Native `modulespecifiers.CountPathComponents`: only forward slashes count;
/// the initial `./` is excluded without otherwise normalizing the specifier.
pub(in crate::project) fn count_path_components(path: &str) -> usize {
    path.strip_prefix("./")
        .unwrap_or(path)
        .bytes()
        .filter(|&byte| byte == b'/')
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_fast_paths_keep_native_root_separator_and_case_results() {
        // Pinned tspath/path_test.go ResolvePath rows and NormalizePath's
        // simpleNormalizePath shortcut. UNC/URL spellings keep the reducer.
        for (input, expected) in [
            ("", ""),
            (".", ""),
            ("./", ""),
            ("..", ".."),
            ("../", "../"),
            (".//a", "a"),
            ("/", "/"),
            ("/a/", "/a/"),
            (".a", ".a"),
            ("..a", "..a"),
            ("a.", "a."),
            ("a..", "a.."),
            ("C:", "C:"),
            (r"C:\", "C:/"),
            ("C:child", "C:child"),
            ("^/", "^/"),
            (r"a\b", "a/b"),
            (r"a\\b", "a/b"),
            ("a//b", "a/b"),
            ("/a//", "/a/"),
            ("/a/./b/", "/a/b/"),
            ("/a/../b/", "/b/"),
            ("//server/share/", "//server/share/"),
            ("http://server/path/", "http://server/path/"),
            ("file:///c%3A/path/", "file:///c%3A/path/"),
        ] {
            assert_eq!(normalize(input), expected, "{input:?}");
        }
        for (input, sensitive, insensitive) in [
            ("", "", ""),
            ("/a/b.ts", "/a/b.ts", "/a/b.ts"),
            (r"C:\A\B.ts", "C:/A/B.ts", "c:/a/b.ts"),
            ("/A/../B/", "/B/", "/b/"),
            ("/İ/K/Σ.ts", "/İ/K/Σ.ts", "/İ/k/σ.ts"),
        ] {
            assert_eq!(Case::Sensitive.key(input), sensitive);
            assert_eq!(Case::Insensitive.key(input), insensitive);
        }
    }

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
