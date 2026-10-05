//! Root file selection: literal `files`, then `include` wildcard matches not
//! removed by `exclude`, following TypeScript's matching rules.
//!
//! `*` and `?` match within one path component, and `**` matches any number
//! of directories. In `include`, wildcards never match names beginning with
//! `.`, and wildcard components never match `node_modules`,
//! `bower_components`, or `jspm_packages`; literal components may name them.
//! A final component without `.`, `*`, or `?` names a directory and gains
//! `/**/*`. An `exclude` match also excludes everything beneath it.

use super::config::{Config, Spec};
use super::{Origin, io_error, paths};
use crate::references::{ProjectError, ProjectErrorKind};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// One root input: an absolute path and, for `files` entries, where it was
/// listed.
#[derive(Debug)]
pub(super) struct Root {
    pub(super) path: String,
    pub(super) origin: Option<Origin>,
}

const PACKAGE_FOLDERS: [&str; 3] = ["node_modules", "bower_components", "jspm_packages"];

pub(super) fn roots(config: &Config, case_sensitive: bool) -> Result<Vec<Root>, ProjectError> {
    let options = &config.options;
    // Extension groups in priority order; within a group, earlier wins.
    let mut groups: Vec<Vec<&str>> = if options.allow_js {
        vec![
            vec![".ts", ".tsx", ".d.ts", ".js", ".jsx"],
            vec![".cts", ".d.cts", ".cjs"],
            vec![".mts", ".d.mts", ".mjs"],
        ]
    } else {
        vec![
            vec![".ts", ".tsx", ".d.ts"],
            vec![".cts", ".d.cts"],
            vec![".mts", ".d.mts"],
        ]
    };
    let mut roots = Vec::new();
    let key = |path: &str| {
        if case_sensitive {
            path.to_owned()
        } else {
            path.to_lowercase()
        }
    };
    let mut literal: HashSet<String> = HashSet::new();
    for spec in config.files.iter().flat_map(|specs| &specs.entries) {
        let path = literal_file(spec, case_sensitive)?;
        if literal.insert(key(&path)) {
            roots.push(Root {
                path,
                origin: Some(spec.origin.clone()),
            });
        }
    }

    let default_include;
    let include: Vec<&Spec> = match (&config.include, &config.files) {
        (Some(specs), _) => specs.entries.iter().collect(),
        (None, Some(_)) => Vec::new(),
        (None, None) => {
            default_include = Spec {
                text: "**/*".into(),
                path: paths::combine(&config.dir, "**/*"),
                origin: Origin {
                    path: config.path.clone(),
                    position: None,
                },
            };
            vec![&default_include]
        }
    };
    if include.is_empty() {
        return Ok(roots);
    }
    let exclude: Vec<String> = match &config.exclude {
        Some(specs) => specs.entries.iter().map(|spec| spec.path.clone()).collect(),
        None => [&options.out_dir, &options.declaration_dir]
            .into_iter()
            .flatten()
            .cloned()
            .collect(),
    };
    if options.resolve_json_module {
        groups.push(vec![".json"]);
    }
    let extensions: Vec<&str> = groups.iter().flatten().copied().collect();
    let matcher = Matcher {
        include: include.iter().map(|spec| pattern(&spec.path)).collect(),
        exclude: exclude.iter().map(|path| pattern(path)).collect(),
        case_sensitive,
    };
    let mut wildcard: Vec<String> = Vec::new();
    let mut wildcard_keys: HashMap<String, usize> = HashMap::new();
    let mut json = Vec::new();
    let json_include: Vec<Vec<Segment>> = include
        .iter()
        .filter(|spec| spec.path.ends_with(".json"))
        .map(|spec| pattern(&spec.path))
        .collect();
    for file in matcher.walk(&config.dir, &include, &extensions)? {
        if file.ends_with(".json") {
            let components: Vec<&str> = file.split('/').collect();
            if json_include
                .iter()
                .any(|pattern| matcher.full_match(pattern, &components))
                && !literal.contains(&key(&file))
            {
                json.push(file);
            }
            continue;
        }
        let Some(group) = groups
            .iter()
            .find(|group| paths::has_extension(&file, group))
        else {
            continue;
        };
        if has_higher_priority(&file, group, &|path| {
            literal.contains(&key(path)) || wildcard_keys.contains_key(&key(path))
        }) {
            continue;
        }
        // Drop lower-priority siblings already matched by an earlier entry.
        for extension in group.iter().rev() {
            if file.ends_with(extension) {
                break;
            }
            if let Some(index) =
                wildcard_keys.remove(&key(&paths::change_extension(&file, extension)))
            {
                wildcard[index].clear();
            }
        }
        let file_key = key(&file);
        if !literal.contains(&file_key) && !wildcard_keys.contains_key(&file_key) {
            wildcard_keys.insert(file_key, wildcard.len());
            wildcard.push(file);
        }
    }
    roots.extend(
        wildcard
            .into_iter()
            .chain(json)
            .filter(|path| !path.is_empty())
            .map(|path| Root { path, origin: None }),
    );
    Ok(roots)
}

/// A listed file must exist; a name without an extension may omit `.ts`,
/// `.tsx`, or `.d.ts`.
fn literal_file(spec: &Spec, case_sensitive: bool) -> Result<String, ProjectError> {
    let path = &spec.path;
    // Extensions follow the file system's case policy, as file names do.
    let spelled = if case_sensitive {
        path.clone()
    } else {
        path.to_lowercase()
    };
    if !paths::basename(path).contains('.') {
        if let Some(found) = [".ts", ".tsx", ".d.ts"]
            .iter()
            .map(|extension| format!("{path}{extension}"))
            .find(|candidate| Path::new(candidate).is_file())
        {
            return Ok(found);
        }
    } else if paths::is_typescript(&spelled)
        || paths::is_javascript(&spelled)
        || spelled.ends_with(".json")
    {
        if Path::new(path).is_file() {
            return Ok(path.clone());
        }
    } else {
        return Err(spec.origin.error(
            ProjectErrorKind::Config,
            format!("File '{}' has an unsupported extension", spec.text),
        ));
    }
    Err(spec.origin.error(
        ProjectErrorKind::Io,
        format!("File '{}' not found", spec.text),
    ))
}

/// Whether a file with the same name and a higher-priority extension in its
/// group is already selected. A `.d.ts` file does not displace `.js`.
fn has_higher_priority(file: &str, group: &[&str], selected: &dyn Fn(&str) -> bool) -> bool {
    for extension in group {
        if file.ends_with(extension) && !(*extension == ".ts" && file.ends_with(".d.ts")) {
            return false;
        }
        if selected(&paths::change_extension(file, extension)) {
            if *extension == ".d.ts" && paths::has_extension(file, &[".js", ".jsx"]) {
                continue;
            }
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    /// `**`.
    Recursive,
    Literal(String),
    Wildcard(Vec<Token>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Star,
    Question,
    Char(char),
}

/// Path components of an absolute pattern, with the implicit `**/*` for a
/// final directory name.
fn pattern(path: &str) -> Vec<Segment> {
    let components: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let mut segments = vec![Segment::Literal(String::new())];
    if !path.starts_with('/') {
        segments.clear();
    }
    for component in &components {
        segments.push(if *component == "**" {
            Segment::Recursive
        } else if component.contains(['*', '?']) {
            Segment::Wildcard(
                component
                    .chars()
                    .map(|c| match c {
                        '*' => Token::Star,
                        '?' => Token::Question,
                        c => Token::Char(c),
                    })
                    .collect(),
            )
        } else {
            Segment::Literal((*component).to_owned())
        });
    }
    if components
        .last()
        .is_some_and(|last| !last.contains(['.', '*', '?']))
    {
        segments.push(Segment::Recursive);
        segments.push(Segment::Wildcard(vec![Token::Star]));
    }
    segments
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Usage {
    Files,
    Directories,
    Exclude,
}

struct Matcher {
    include: Vec<Vec<Segment>>,
    exclude: Vec<Vec<Segment>>,
    case_sensitive: bool,
}

impl Matcher {
    /// Files under the include base directories, in TypeScript's order:
    /// grouped by the first include entry that matches, and within a group
    /// by a depth-first walk visiting a directory's sorted files before its
    /// sorted subdirectories. Each real directory is visited once.
    fn walk(
        &self,
        config_dir: &str,
        include: &[&Spec],
        extensions: &[&str],
    ) -> Result<Vec<String>, ProjectError> {
        let mut results: Vec<Vec<String>> = vec![Vec::new(); self.include.len()];
        let mut visited: HashSet<std::path::PathBuf> = HashSet::new();
        for base in self.base_paths(config_dir, include) {
            let mut stack = vec![base];
            while let Some(directory) = stack.pop() {
                let Ok(real) = std::fs::canonicalize(&directory) else {
                    continue;
                };
                if !visited.insert(real) {
                    continue;
                }
                let (files, directories) = entries(&directory)?;
                for name in files {
                    let path = paths::combine(&directory, &name);
                    if !paths::has_extension(&path, extensions) {
                        continue;
                    }
                    let components: Vec<&str> = path.split('/').collect();
                    if self.excluded(&components) {
                        continue;
                    }
                    if let Some(index) = self
                        .include
                        .iter()
                        .position(|pattern| self.full_match(pattern, &components))
                    {
                        results[index].push(path);
                    }
                }
                for name in directories.into_iter().rev() {
                    let path = paths::combine(&directory, &name);
                    let components: Vec<&str> = path.split('/').collect();
                    if self
                        .include
                        .iter()
                        .any(|pattern| self.prefix_match(pattern, &components))
                        && !self.excluded(&components)
                    {
                        stack.push(path);
                    }
                }
            }
        }
        Ok(results.into_iter().flatten().collect())
    }

    /// The configuration directory, then each include's literal prefix that
    /// is not already beneath an earlier base.
    fn base_paths(&self, config_dir: &str, include: &[&Spec]) -> Vec<String> {
        let mut bases: Vec<String> = include
            .iter()
            .map(|spec| {
                let path = &spec.path;
                match path.find(['*', '?']) {
                    Some(wildcard) => match path[..wildcard].rfind('/') {
                        Some(0) | None => "/".to_owned(),
                        Some(slash) => path[..slash].to_owned(),
                    },
                    None if paths::basename(path).contains('.') => paths::parent(path).to_owned(),
                    None => path.clone(),
                }
            })
            .collect();
        if self.case_sensitive {
            bases.sort();
        } else {
            bases.sort_by_key(|base| base.to_lowercase());
        }
        let mut result = vec![config_dir.to_owned()];
        for base in bases {
            if !result
                .iter()
                .any(|existing| paths::contains(existing, &base, self.case_sensitive))
            {
                result.push(base);
            }
        }
        result
    }

    fn full_match(&self, pattern: &[Segment], components: &[&str]) -> bool {
        let states = self.run(pattern, components, Usage::Files, None);
        states.contains(&pattern.len())
    }

    /// Whether some path beneath `components` could match.
    fn prefix_match(&self, pattern: &[Segment], components: &[&str]) -> bool {
        !self
            .run(pattern, components, Usage::Directories, None)
            .is_empty()
    }

    /// Whether the path or one of its ancestors matches an exclude entry.
    fn excluded(&self, components: &[&str]) -> bool {
        self.exclude.iter().any(|pattern| {
            let mut found = false;
            self.run(pattern, components, Usage::Exclude, Some(&mut found));
            found
        })
    }

    /// Simulates the pattern over the components and returns the reachable
    /// pattern positions. With `complete`, records whether any prefix of the
    /// components fully matched.
    fn run(
        &self,
        pattern: &[Segment],
        components: &[&str],
        usage: Usage,
        mut complete: Option<&mut bool>,
    ) -> Vec<usize> {
        let closure = |states: &mut Vec<usize>| {
            let mut index = 0;
            while index < states.len() {
                let state = states[index];
                if pattern.get(state) == Some(&Segment::Recursive) && !states.contains(&(state + 1))
                {
                    states.push(state + 1);
                }
                index += 1;
            }
        };
        let mut states = vec![0];
        closure(&mut states);
        for (position, name) in components.iter().enumerate() {
            let last = position + 1 == components.len();
            let mut next = Vec::new();
            for &state in &states {
                let Some(segment) = pattern.get(state) else {
                    continue;
                };
                let target = match segment {
                    Segment::Recursive => {
                        let allowed = usage == Usage::Exclude
                            || !name.starts_with('.') && !self.package_folder(name);
                        allowed.then_some(state)
                    }
                    Segment::Literal(literal) => self.same(literal, name).then_some(state + 1),
                    Segment::Wildcard(tokens) => self
                        .wildcard(tokens, name, usage, last)
                        .then_some(state + 1),
                };
                if let Some(target) = target {
                    if !next.contains(&target) {
                        next.push(target);
                    }
                }
            }
            closure(&mut next);
            states = next;
            if let Some(found) = complete.as_deref_mut() {
                *found |= states.contains(&pattern.len());
            }
            if states.is_empty() {
                break;
            }
        }
        states
    }

    fn same(&self, a: &str, b: &str) -> bool {
        if self.case_sensitive {
            a == b
        } else {
            a.to_lowercase() == b.to_lowercase()
        }
    }

    fn package_folder(&self, name: &str) -> bool {
        PACKAGE_FOLDERS.iter().any(|folder| self.same(folder, name))
    }

    fn wildcard(&self, tokens: &[Token], name: &str, usage: Usage, last: bool) -> bool {
        let include = usage != Usage::Exclude;
        if include && self.package_folder(name) {
            return false;
        }
        let chars: Vec<char> = name.chars().collect();
        let mut tokens = tokens;
        if include && chars.first() == Some(&'.') {
            match tokens.first() {
                // A leading `*` may only match the empty string here.
                Some(Token::Star) => tokens = &tokens[1..],
                Some(Token::Question) => return false,
                _ => {}
            }
        }
        let lower = |c: char| {
            if self.case_sensitive {
                c
            } else {
                c.to_lowercase().next().unwrap_or(c)
            }
        };
        // In file names, `*` does not consume the `.` of a final `.min.js`.
        let min_js = usage == Usage::Files && last;
        let consumable = |index: usize| {
            !(min_js
                && chars[index] == '.'
                && chars[index + 1..]
                    .iter()
                    .map(|c| c.to_ascii_lowercase())
                    .eq("min.js".chars()))
        };
        // matches[t][c]: tokens[t..] match chars[c..].
        let mut matches = vec![vec![false; chars.len() + 1]; tokens.len() + 1];
        matches[tokens.len()][chars.len()] = true;
        for t in (0..tokens.len()).rev() {
            for c in (0..=chars.len()).rev() {
                matches[t][c] = match tokens[t] {
                    Token::Star => {
                        matches[t + 1][c] || c < chars.len() && consumable(c) && matches[t][c + 1]
                    }
                    Token::Question => c < chars.len() && matches[t + 1][c + 1],
                    Token::Char(expected) => {
                        c < chars.len()
                            && lower(chars[c]) == lower(expected)
                            && matches[t + 1][c + 1]
                    }
                };
            }
        }
        matches[0][0]
    }
}

/// Sorted file and directory names, following symbolic links and skipping
/// broken ones. A missing directory has no entries.
fn entries(directory: &str) -> Result<(Vec<String>, Vec<String>), ProjectError> {
    let mut files = Vec::new();
    let mut directories = Vec::new();
    let reader = match std::fs::read_dir(directory) {
        Ok(reader) => reader,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((files, directories));
        }
        Err(error) => return Err(io_error(Path::new(directory), &error)),
    };
    for entry in reader {
        let entry = entry.map_err(|error| io_error(Path::new(directory), &error))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(metadata) = std::fs::metadata(entry.path()) else {
            continue;
        };
        if metadata.is_dir() {
            directories.push(name);
        } else if metadata.is_file() {
            files.push(name);
        }
    }
    files.sort();
    directories.sort();
    Ok((files, directories))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(include: &[&str], exclude: &[&str], case_sensitive: bool) -> Matcher {
        Matcher {
            include: include.iter().map(|path| pattern(path)).collect(),
            exclude: exclude.iter().map(|path| pattern(path)).collect(),
            case_sensitive,
        }
    }

    fn split(path: &str) -> Vec<&str> {
        path.split('/').collect()
    }

    #[test]
    fn include_wildcards_skip_dot_names_and_package_folders() {
        let m = matcher(&["/p/**/*"], &[], true);
        let all = &m.include[0];
        assert!(m.full_match(all, &split("/p/a.ts")));
        assert!(m.full_match(all, &split("/p/x/y/a.ts")));
        assert!(!m.full_match(all, &split("/p/.dot.ts")));
        assert!(!m.full_match(all, &split("/p/.hidden/a.ts")));
        assert!(!m.full_match(all, &split("/p/node_modules/a.ts")));
        assert!(!m.full_match(all, &split("/p/x/bower_components/a.ts")));
        let explicit = matcher(&["/p/node_modules/**/*", "/p/.hidden/*"], &[], true);
        assert!(explicit.full_match(&explicit.include[0], &split("/p/node_modules/x/a.ts")));
        assert!(explicit.full_match(&explicit.include[1], &split("/p/.hidden/a.ts")));
        let star = matcher(&["/p/*.ts", "/p/?.ts", "/p/a*.min.js"], &[], true);
        assert!(star.full_match(&star.include[0], &split("/p/a.d.ts")));
        assert!(star.full_match(&star.include[0], &split("/p/.ts")));
        assert!(!star.full_match(&star.include[0], &split("/p/x/a.ts")));
        assert!(star.full_match(&star.include[1], &split("/p/a.ts")));
        assert!(!star.full_match(&star.include[1], &split("/p/ab.ts")));
        assert!(star.full_match(&star.include[2], &split("/p/ab.min.js")));
        let js = matcher(&["/p/*.js"], &[], true);
        assert!(!js.full_match(&js.include[0], &split("/p/a.min.js")));
    }

    #[test]
    fn directory_names_imply_recursive_globs_and_excludes_cover_descendants() {
        let m = matcher(&["/p/src"], &["/p/src/gen", "/p/**/*.d.ts"], true);
        assert!(m.full_match(&m.include[0], &split("/p/src/a/b.ts")));
        assert!(m.prefix_match(&m.include[0], &split("/p/src/a")));
        assert!(!m.prefix_match(&m.include[0], &split("/p/lib")));
        assert!(m.excluded(&split("/p/src/gen/x.ts")));
        assert!(m.excluded(&split("/p/.x/y.d.ts")));
        assert!(!m.excluded(&split("/p/src/a.ts")));
        let insensitive = matcher(&["/p/A*.ts"], &[], false);
        assert!(insensitive.full_match(&insensitive.include[0], &split("/p/a2.ts")));
    }
}
