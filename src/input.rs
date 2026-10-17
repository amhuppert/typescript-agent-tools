//! Input classification, shared by project loading, project queries, file
//! navigation, and editing: the exact text that names a file, which kind of
//! program input a file name denotes, the file system's case policy that
//! TypeScript applies to file names, and which failures mean a file is
//! absent.

use crate::analysis::{InputError, InputErrorKind};
use std::borrow::Cow;
use std::path::Path;

/// A path's exact text, the only spelling by which it names a file. A path
/// that is not Unicode, possible on Linux, would change under a lossy
/// conversion and could then name another file, such as one whose name
/// has U+FFFD where the path has an invalid byte, so it names no input: an
/// `Unsupported` failure for `path`, whose message alone shows it lossily.
pub(crate) fn exact_path(path: &Path) -> Result<&str, InputError> {
    path.to_str().ok_or_else(|| InputError {
        kind: InputErrorKind::Unsupported,
        message: "the file name is not Unicode, so it cannot be named exactly".into(),
        path: Some(path.to_path_buf()),
        position: None,
    })
}

/// Whether a file system error means the path names nothing: only
/// `NotFound` and `NotADirectory`. Any other failure, such as a permission
/// error, means a file may exist but cannot be inspected, which project
/// loading and editing both report as a failure rather than as absence.
pub(crate) fn is_absence(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
    )
}

/// Whether the file system distinguishes names that differ only in case.
/// On an insensitive one, TypeScript compares file names, and decides
/// whether an input has a supported extension (`InputKind`), after folding
/// case. Other name rules keep the exact spelling: declaration files and
/// module formats (`ParseOptions::for_file`, the loader's file formats).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Case {
    Sensitive,
    Insensitive,
}

impl Case {
    /// TypeScript's test: the file system is case-insensitive when the
    /// running executable is also reachable with its letters' case swapped.
    pub(crate) fn detect() -> Self {
        if cfg!(windows) {
            return Self::Insensitive;
        }
        let Ok(executable) = std::env::current_exe() else {
            return Self::Sensitive;
        };
        let text = executable.to_string_lossy();
        let swapped: String = text
            .chars()
            .map(|c| {
                if c.is_uppercase() {
                    c.to_lowercase().next().unwrap_or(c)
                } else {
                    c.to_uppercase().next().unwrap_or(c)
                }
            })
            .collect();
        if swapped == text || !Path::new(&swapped).exists() {
            Self::Sensitive
        } else {
            Self::Insensitive
        }
    }

    /// The spelling TypeScript compares and classifies: its
    /// `GetCanonicalFileName`, which folds with `to_file_name_lower_case`.
    pub(crate) fn fold(self, text: &str) -> Cow<'_, str> {
        match self {
            Self::Sensitive => Cow::Borrowed(text),
            Self::Insensitive => to_file_name_lower_case(text),
        }
    }

    /// `fold` for one character of a pattern comparison.
    pub(crate) fn fold_char(self, c: char) -> char {
        match self {
            Self::Sensitive => c,
            Self::Insensitive => file_name_lower_char(c),
        }
    }

    pub(crate) fn same(self, a: &str, b: &str) -> bool {
        self.fold(a) == self.fold(b)
    }

    /// Whether `child` is `parent` or lies beneath it.
    pub(crate) fn contains(self, parent: &str, child: &str) -> bool {
        let parent = parent.strip_suffix('/').unwrap_or(parent);
        if child.len() < parent.len() || !child.is_char_boundary(parent.len()) {
            return false;
        }
        let (head, tail) = child.split_at(parent.len());
        self.same(head, parent)
            && (tail.is_empty() || tail.starts_with('/') || parent.ends_with('/'))
    }
}

/// The kind of program input a file name denotes, by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputKind {
    TypeScript,
    JavaScript,
    Json,
    Other,
}

impl InputKind {
    /// Classify `path` as TypeScript does: by its file name under the case
    /// policy, so `A.JS` is JavaScript on a case-insensitive file system.
    pub(crate) fn of(path: &str, case: Case) -> Self {
        let path = case.fold(path);
        if is_typescript(&path) {
            Self::TypeScript
        } else if is_javascript(&path) {
            Self::JavaScript
        } else if path.ends_with(".json") {
            Self::Json
        } else {
            Self::Other
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::TypeScript => "TypeScript",
            Self::JavaScript => "JavaScript",
            Self::Json => "JSON",
            Self::Other => "unsupported",
        }
    }
}

/// TypeScript's supported TypeScript extensions, compared exactly.
pub(crate) fn is_typescript(path: &str) -> bool {
    [".ts", ".tsx", ".mts", ".cts"]
        .iter()
        .any(|extension| path.ends_with(extension))
}

/// TypeScript's JavaScript extensions, compared exactly.
pub(crate) fn is_javascript(path: &str) -> bool {
    [".js", ".jsx", ".mjs", ".cjs"]
        .iter()
        .any(|extension| path.ends_with(extension))
}

/// TypeScript's `tspath.ToFileNameLowerCase`, which folds file names on a
/// case-insensitive file system and names standard libraries
/// (`tsoptions.GetLibFileName`): each character's simple lowercase form, as
/// Go's `unicode.ToLower` gives it, except U+0130 (capital I with dot
/// above), which stays because its lowercase form has its own uppercase.
pub(crate) fn to_file_name_lower_case(text: &str) -> Cow<'_, str> {
    if text.is_ascii() && !text.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.chars().map(file_name_lower_char).collect())
}

fn file_name_lower_char(c: char) -> char {
    if c == '\u{130}' {
        return c;
    }
    simple_lowercase(c)
}

/// Go's `unicode.ToLower`: one character for one. Rust's full mapping
/// differs only where it yields several characters, which is U+0130 alone,
/// whose simple lowercase form is `i`.
pub(crate) fn simple_lowercase(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(single), None) => single,
        _ if c == '\u{130}' => 'i',
        _ => c,
    }
}

/// Canonical representative of Go `unicode.SimpleFold` / `strings.EqualFold`.
/// Unlike filename lowercase, this joins final sigma and the nontrivial simple
/// fold orbits, and keeps dotted/dotless I distinct. The exception orbits are
/// from Go Unicode 17.0; ordinary one/two-element orbits share simple lowercase.
pub(crate) fn simple_case_fold(c: char) -> char {
    match c {
        '\u{4b}' | '\u{6b}' | '\u{212a}' => '\u{4b}',
        '\u{53}' | '\u{73}' | '\u{17f}' => '\u{53}',
        '\u{b5}' | '\u{39c}' | '\u{3bc}' => '\u{b5}',
        '\u{c5}' | '\u{e5}' | '\u{212b}' => '\u{c5}',
        '\u{df}' | '\u{1e9e}' => '\u{df}',
        '\u{130}' => '\u{130}',
        '\u{131}' => '\u{131}',
        '\u{1c4}' | '\u{1c5}' | '\u{1c6}' => '\u{1c4}',
        '\u{1c7}' | '\u{1c8}' | '\u{1c9}' => '\u{1c7}',
        '\u{1ca}' | '\u{1cb}' | '\u{1cc}' => '\u{1ca}',
        '\u{1f1}' | '\u{1f2}' | '\u{1f3}' => '\u{1f1}',
        '\u{345}' | '\u{399}' | '\u{3b9}' | '\u{1fbe}' => '\u{345}',
        '\u{390}' | '\u{1fd3}' => '\u{390}',
        '\u{392}' | '\u{3b2}' | '\u{3d0}' => '\u{392}',
        '\u{395}' | '\u{3b5}' | '\u{3f5}' => '\u{395}',
        '\u{398}' | '\u{3b8}' | '\u{3d1}' | '\u{3f4}' => '\u{398}',
        '\u{39a}' | '\u{3ba}' | '\u{3f0}' => '\u{39a}',
        '\u{3a0}' | '\u{3c0}' | '\u{3d6}' => '\u{3a0}',
        '\u{3a1}' | '\u{3c1}' | '\u{3f1}' => '\u{3a1}',
        '\u{3a3}' | '\u{3c2}' | '\u{3c3}' => '\u{3a3}',
        '\u{3a6}' | '\u{3c6}' | '\u{3d5}' => '\u{3a6}',
        '\u{3a9}' | '\u{3c9}' | '\u{2126}' => '\u{3a9}',
        '\u{3b0}' | '\u{1fe3}' => '\u{3b0}',
        '\u{412}' | '\u{432}' | '\u{1c80}' => '\u{412}',
        '\u{414}' | '\u{434}' | '\u{1c81}' => '\u{414}',
        '\u{41e}' | '\u{43e}' | '\u{1c82}' => '\u{41e}',
        '\u{421}' | '\u{441}' | '\u{1c83}' => '\u{421}',
        '\u{422}' | '\u{442}' | '\u{1c84}' | '\u{1c85}' => '\u{422}',
        '\u{42a}' | '\u{44a}' | '\u{1c86}' => '\u{42a}',
        '\u{462}' | '\u{463}' | '\u{1c87}' => '\u{462}',
        '\u{1c88}' | '\u{a64a}' | '\u{a64b}' => '\u{1c88}',
        '\u{1e60}' | '\u{1e61}' | '\u{1e9b}' => '\u{1e60}',
        '\u{fb05}' | '\u{fb06}' => '\u{fb05}',
        _ => simple_lowercase(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_policies_relate_paths() {
        assert!(Case::Sensitive.contains("/a/b", "/a/b/c"));
        assert!(Case::Sensitive.contains("/a/b", "/a/b"));
        assert!(!Case::Sensitive.contains("/a/b", "/a/bc"));
        assert!(Case::Insensitive.contains("/A/b", "/a/B/c"));
        assert!(Case::Sensitive.contains("/", "/a"));
    }

    #[test]
    fn file_names_lower_one_character_for_one_as_typescript_does() {
        assert_eq!(to_file_name_lower_case("Lib.ES5.d.ts"), "lib.es5.d.ts");
        assert!(matches!(to_file_name_lower_case("a.ts"), Cow::Borrowed(_)));
        // KELVIN SIGN lowers to `k`; U+0130 stays; a final sigma is not
        // contextual, unlike Rust's `str::to_lowercase`.
        assert_eq!(to_file_name_lower_case("WEBWOR\u{212A}ER"), "webworker");
        assert_eq!(to_file_name_lower_case("\u{130}NTL"), "\u{130}ntl");
        assert_eq!(Case::Insensitive.fold("AΣ"), "aσ");
        assert_eq!(Case::Insensitive.fold_char('\u{130}'), '\u{130}');
        assert_eq!(simple_lowercase('\u{130}'), 'i');
        assert_eq!(Case::Sensitive.fold("AΣ"), "AΣ");
    }
}
