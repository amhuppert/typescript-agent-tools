//! Relative path construction shared by individual queries and load-time unions.
use super::{Case, resolve, root_length, text_equal};
use std::collections::BTreeSet;
use std::ops::Range;

struct Components {
    path: String,
    root: usize,
    parts: Vec<Range<usize>>,
}

impl Components {
    fn new(path: &str, cwd: &str) -> Self {
        let path = resolve(cwd, path);
        let root = root_length(&path);
        let mut offset = root;
        let parts = path[root..]
            .split('/')
            .filter_map(|part| {
                let range = offset..offset + part.len();
                offset += part.len() + 1;
                (!part.is_empty()).then_some(range)
            })
            .collect();
        Self { path, root, parts }
    }

    /// `GetPathComponentsRelativeTo`: roots always use EqualFold; non-root
    /// components use the host's equality policy. Only the target's spelling
    /// survives in the result, so this shape identifies duplicate results.
    fn relative_to(&self, target: &Self, case: Case) -> Shape {
        if !text_equal(
            &self.path[..self.root],
            &target.path[..target.root],
            Case::Insensitive,
        ) {
            return Shape::Rooted;
        }
        let common = self
            .parts
            .iter()
            .zip(&target.parts)
            .take_while(|(a, b)| {
                text_equal(&self.path[(*a).clone()], &target.path[(*b).clone()], case)
            })
            .count();
        Shape::Relative {
            parents: self.parts.len() - common,
            common,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Shape {
    Rooted,
    Relative { parents: usize, common: usize },
}

impl Shape {
    /// `GetPathFromPathComponents`: join relative components, or preserve the
    /// target root when the source and target have different roots.
    fn render(self, target: &Components) -> String {
        let (parents, common) = match self {
            Self::Rooted => {
                return if target.root == target.path.len() {
                    if target.path.ends_with('/') {
                        target.path.clone()
                    } else {
                        format!("{}/", target.path)
                    }
                } else {
                    target.path.trim_end_matches('/').to_owned()
                };
            }
            Self::Relative { parents, common } => (parents, common),
        };
        let parts = &target.parts[common..];
        let length = parents * 3 + parts.iter().map(|part| part.len() + 1).sum::<usize>();
        let mut result = String::with_capacity(length.saturating_sub(1));
        for part in std::iter::repeat_n("..", parents)
            .chain(parts.iter().map(|part| &target.path[part.clone()]))
        {
            if !result.is_empty() {
                result.push('/');
            }
            result.push_str(part);
        }
        result
    }
}

/// `GetRelativePathFromDirectory` / `GetRelativePathToDirectoryOrUrl`, with
/// normalization and component equality owned by `GetPathComponentsRelativeTo`.
pub(in crate::project::program) fn relative_with_case(
    from: &str,
    to: &str,
    cwd: &str,
    case: Case,
) -> String {
    let from = Components::new(from, cwd);
    let to = Components::new(to, cwd);
    from.relative_to(&to, case).render(&to)
}

/// The union of `GetRelativePathFromDirectory` results from fixed directories.
/// Normalize directories once and each target once, then construct only the
/// distinct shapes. Many sibling directories have the same common prefix and
/// number of parents, regardless of their names. This is load-local scratch;
/// it retains no query results or filesystem observations. Comparisons still
/// visit every directory for each target; only construction is deduplicated.
pub(in crate::project::program) struct RelativePaths<'a> {
    directories: Vec<Components>,
    cwd: &'a str,
    case: Case,
}

impl<'a> RelativePaths<'a> {
    pub(in crate::project::program) fn new<'d>(
        directories: impl IntoIterator<Item = &'d str>,
        cwd: &'a str,
        case: Case,
    ) -> Self {
        Self {
            directories: directories
                .into_iter()
                .map(|directory| Components::new(directory, cwd))
                .collect(),
            cwd,
            case,
        }
    }

    pub(in crate::project::program) fn to(&self, target: &str) -> Vec<String> {
        let target = Components::new(target, self.cwd);
        // BTreeSet::from_iter first collects and sorts every input. Insert
        // directly so scratch storage contains only distinct shapes too.
        let mut shapes = BTreeSet::new();
        for directory in &self.directories {
            shapes.insert(directory.relative_to(&target, self.case));
        }
        shapes
            .into_iter()
            .map(|shape| shape.render(&target))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CaseRow {
        from_path: String,
        to_path: String,
        cwd: String,
        insensitive: bool,
        expected: String,
    }

    #[test]
    fn individual_paths_and_directory_unions_match_native() {
        // TypeScript 7.0.2, 2bd066d87f5bafd315be9f40889d0a60b9e58e0b:
        // tspath.GetRelativePathToDirectoryOrUrl(from, to, false,
        // ComparePathsOptions{CurrentDirectory: cwd,
        //                     UseCaseSensitiveFileNames: !insensitive}).
        let rows: Vec<CaseRow> =
            serde_json::from_str(include_str!("../../../../fixtures/paths/relative.json")).unwrap();
        for row in &rows {
            let case = if row.insensitive {
                Case::Insensitive
            } else {
                Case::Sensitive
            };
            assert_eq!(
                relative_with_case(&row.from_path, &row.to_path, &row.cwd, case),
                row.expected,
                "{} -> {}, cwd {}, {:?}",
                row.from_path,
                row.to_path,
                row.cwd,
                case
            );
            let matching: Vec<_> = rows
                .iter()
                .filter(|other| {
                    other.to_path == row.to_path
                        && other.cwd == row.cwd
                        && other.insensitive == row.insensitive
                })
                .collect();
            let directories = matching.iter().map(|other| other.from_path.as_str());
            let actual: BTreeSet<_> = RelativePaths::new(directories, &row.cwd, case)
                .to(&row.to_path)
                .into_iter()
                .collect();
            let expected: BTreeSet<_> = matching
                .iter()
                .map(|other| other.expected.clone())
                .collect();
            assert_eq!(actual, expected, "union to {}", row.to_path);
        }
        assert!(
            RelativePaths::new([], "", Case::Sensitive)
                .to("/index.ts")
                .is_empty()
        );
    }
}
