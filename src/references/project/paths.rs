//! Absolute `/`-separated path strings, normalized lexically as TypeScript
//! does. Configuration patterns, module candidates, and extension rewriting
//! are string operations; only file system calls convert to `Path`.

use std::path::Path;

pub(super) fn from_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    }
}

/// The length of a root prefix: `/`, or a drive such as `C:/`.
fn root_length(path: &str) -> usize {
    let bytes = path.as_bytes();
    if path.starts_with('/') {
        1
    } else if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'/'
    {
        3
    } else {
        0
    }
}

pub(super) fn is_rooted(path: &str) -> bool {
    root_length(path) > 0
}

/// `./x`, `../x`, `.`, or `..`.
pub(super) fn is_relative(path: &str) -> bool {
    path == "." || path == ".." || path.starts_with("./") || path.starts_with("../")
}

/// Resolves `.` and `..` segments and repeated separators, keeping a
/// trailing separator.
pub(super) fn normalize(path: &str) -> String {
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

pub(super) fn combine(base: &str, relative: &str) -> String {
    if is_rooted(relative) || base.is_empty() {
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

/// Whether `child` is `parent` or lies beneath it.
pub(super) fn contains(parent: &str, child: &str, case_sensitive: bool) -> bool {
    let parent = parent.strip_suffix('/').unwrap_or(parent);
    if child.len() < parent.len() || !child.is_char_boundary(parent.len()) {
        return false;
    }
    let (head, tail) = child.split_at(parent.len());
    let same = if case_sensitive {
        head == parent
    } else {
        head.to_lowercase() == parent.to_lowercase()
    };
    same && (tail.is_empty() || tail.starts_with('/') || parent.ends_with('/'))
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

/// `.d.ts`, `.d.mts`, `.d.cts`, or an arbitrary-extension declaration such
/// as `styles.d.css.ts`.
pub(super) fn is_declaration(path: &str) -> bool {
    has_extension(path, &[".d.ts", ".d.mts", ".d.cts"])
        || path.ends_with(".ts") && basename(path).contains(".d.")
}

pub(super) fn is_typescript(path: &str) -> bool {
    has_extension(path, &[".ts", ".tsx", ".mts", ".cts"])
}

pub(super) fn is_javascript(path: &str) -> bool {
    has_extension(path, &[".js", ".jsx", ".mjs", ".cjs"])
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
        assert_eq!(parent("/a/b"), "/a");
        assert_eq!(parent("/a"), "/");
        assert_eq!(basename("/a/b.d.ts"), "b.d.ts");
        assert_eq!(ancestors("/a/b").collect::<Vec<_>>(), ["/a/b", "/a", "/"]);
        assert!(contains("/a/b", "/a/b/c", true));
        assert!(contains("/a/b", "/a/b", true));
        assert!(!contains("/a/b", "/a/bc", true));
        assert!(contains("/A/b", "/a/B/c", false));
        assert!(contains("/", "/a", true));
        assert_eq!(relative("/a/b", "/a/c/d.ts"), "../c/d.ts");
        assert_eq!(relative("/a/b", "/a/b/d.ts"), "d.ts");
        assert_eq!(change_extension("/a/x.d.ts", ".ts"), "/a/x.ts");
        assert_eq!(change_extension("/a/x.d.css.ts", ".tsx"), "/a/x.d.css.tsx");
        assert!(is_declaration("/a/x.d.css.ts"));
        assert!(!is_declaration("/a.d.b/x.ts"));
    }
}
