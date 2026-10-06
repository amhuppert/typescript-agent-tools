//! Root file selection: literal `files`, then `include` wildcard matches not
//! removed by `exclude`, following TypeScript's matching rules.
//!
//! `*` and `?` match within one path component, and `**` matches any number
//! of directories. In `include`, wildcards never match names beginning with
//! `.`, and wildcard components never match `node_modules`,
//! `bower_components`, or `jspm_packages`; literal components may name them.
//! A final component without `.`, `*`, or `?` names a directory and gains
//! `/**/*`. An `exclude` match also excludes everything beneath it.

use super::config::{Config, Spec, Specs};
use super::paths::{self, Case, InputKind};
use super::{Origin, disk, io_error};
use crate::references::{ProjectError, ProjectErrorKind};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// One root input: an absolute path and, for `files` entries, where it was
/// listed.
#[derive(Debug)]
pub(super) struct Root {
    pub(super) path: String,
    pub(super) origin: Option<Origin>,
}

const PACKAGE_FOLDERS: [&str; 3] = ["node_modules", "bower_components", "jspm_packages"];

/// The include pattern of a configuration without `files` or `include`.
const DEFAULT_INCLUDE: &str = "**/*";

/// The root inputs a configuration selects. JavaScript and JSON files take
/// part in extension priority, then are rejected as unsupported inputs.
pub(super) fn roots(config: &Config, case: Case) -> Result<Vec<Root>, ProjectError> {
    let roots = select(config, case)?;
    if roots.is_empty() && config.report_no_inputs {
        return Err(no_inputs(config));
    }
    for root in &roots {
        let kind = InputKind::of(&root.path, case);
        if !matches!(kind, InputKind::JavaScript | InputKind::Json) {
            continue;
        }
        let message = format!("{} inputs are not supported", kind.label());
        return Err(match &root.origin {
            Some(origin) => origin.error(ProjectErrorKind::Unsupported, message),
            None => ProjectError {
                kind: ProjectErrorKind::Unsupported,
                message,
                path: Some(PathBuf::from(&root.path)),
                position: None,
            },
        });
    }
    Ok(roots)
}

/// TypeScript's TS18003 message.
fn no_inputs(config: &Config) -> ProjectError {
    let specs = |specs: &Option<Specs>| {
        let entries: Vec<String> = specs
            .iter()
            .flat_map(|specs| &specs.entries)
            .map(|spec| format!("{:?}", spec.text))
            .collect();
        format!("[{}]", entries.join(","))
    };
    let include = if config.include.is_none() && config.files.is_none() {
        format!("[{DEFAULT_INCLUDE:?}]")
    } else {
        specs(&config.include)
    };
    ProjectError {
        kind: ProjectErrorKind::Config,
        message: format!(
            "No inputs were found in config file '{}'. Specified 'include' paths were '{include}' and 'exclude' paths were '{}'",
            config.path.display(),
            specs(&config.exclude)
        ),
        path: Some(config.path.clone()),
        position: None,
    }
}

/// Literal `files`, then `include` matches not removed by `exclude`.
fn select(config: &Config, case: Case) -> Result<Vec<Root>, ProjectError> {
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
    let key = |path: &str| case.fold(path).into_owned();
    let mut literal: HashSet<String> = HashSet::new();
    for spec in config.files.iter().flat_map(|specs| &specs.entries) {
        let path = literal_file(spec, case)?;
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
                text: DEFAULT_INCLUDE.into(),
                path: paths::combine(&config.dir, DEFAULT_INCLUDE),
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
        case,
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

/// A listed file must exist. JavaScript and JSON entries, present or not,
/// are selected here, for extension priority, and rejected with the other
/// roots.
fn literal_file(spec: &Spec, case: Case) -> Result<String, ProjectError> {
    match literal_input(&spec.path, case)? {
        Literal::Found(path) => Ok(path),
        Literal::Unsupported(_) => Ok(spec.path.clone()),
        Literal::Missing => Err(spec.origin.error(
            ProjectErrorKind::Io,
            format!("File '{}' not found", spec.text),
        )),
        Literal::UnsupportedExtension => Err(spec.origin.error(
            ProjectErrorKind::Config,
            format!("File '{}' has an unsupported extension", spec.text),
        )),
    }
}

/// What a file name written literally, as a `files` entry or a
/// `/// <reference path>`, selects.
pub(super) enum Literal {
    /// An existing file, which may still be outside the supported profile.
    Found(String),
    /// No such file, under any default extension of a name without one.
    Missing,
    /// An extension that never names a program input, such as `.css`.
    UnsupportedExtension,
    /// A JavaScript or JSON input, which this tool does not read, whether
    /// or not it exists; TypeScript rejects both forms of literal name alike.
    Unsupported(InputKind),
}

/// TypeScript's literal file rule: a name with a program-input extension
/// must exist as written, and a name without an extension may omit `.ts`,
/// `.tsx`, or `.d.ts`. Each caller decides how to report the outcome.
pub(super) fn literal_input(path: &str, case: Case) -> Result<Literal, ProjectError> {
    let kind = InputKind::of(path, case);
    if matches!(kind, InputKind::JavaScript | InputKind::Json) {
        return Ok(Literal::Unsupported(kind));
    }
    let candidates: Vec<String> = if kind != InputKind::Other {
        vec![path.to_owned()]
    } else if paths::basename(path).contains('.') {
        return Ok(Literal::UnsupportedExtension);
    } else {
        [".ts", ".tsx", ".d.ts"]
            .iter()
            .map(|extension| format!("{path}{extension}"))
            .collect()
    };
    for candidate in candidates {
        if disk::is_file(&candidate).map_err(|error| io_error(Path::new(&candidate), &error))? {
            return Ok(Literal::Found(candidate));
        }
    }
    Ok(Literal::Missing)
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
    case: Case,
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
                // An include base may name no directory, as `entries` allows.
                let real = match std::fs::canonicalize(&directory) {
                    Ok(real) => real,
                    Err(error) if disk::is_absence(&error) => continue,
                    Err(error) => return Err(io_error(Path::new(&directory), &error)),
                };
                if !visited.insert(real) {
                    continue;
                }
                let (files, directories) = entries(&directory)?;
                for name in files {
                    let name = match name {
                        Ok(name) => name,
                        Err(lossy) if paths::has_extension(&lossy, extensions) => {
                            return Err(paths::inexact_name(&directory, &lossy));
                        }
                        Err(_) => continue,
                    };
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
                    let (Ok(text) | Err(text)) = &name;
                    let path = paths::combine(&directory, text);
                    let components: Vec<&str> = path.split('/').collect();
                    if self
                        .include
                        .iter()
                        .any(|pattern| self.prefix_match(pattern, &components))
                        && !self.excluded(&components)
                    {
                        // A directory the walk would enter must be named exactly.
                        if name.is_err() {
                            return Err(paths::inexact_name(&directory, text));
                        }
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
        bases.sort_by(|a, b| self.case.fold(a).cmp(&self.case.fold(b)));
        let mut result = vec![config_dir.to_owned()];
        for base in bases {
            if !result
                .iter()
                .any(|existing| self.case.contains(existing, &base))
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
                    Segment::Literal(literal) => self.case.same(literal, name).then_some(state + 1),
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

    fn package_folder(&self, name: &str) -> bool {
        PACKAGE_FOLDERS
            .iter()
            .any(|folder| self.case.same(folder, name))
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
        let lower = |c: char| self.case.fold_char(c);
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

/// A directory entry's exact name, or the lossy form of one that is not
/// Unicode (`paths::exact_name`).
type Name = Result<String, String>;

/// Sorted file and directory names, following symbolic links and skipping
/// broken ones. A missing directory, or a file named as one, has no entries.
fn entries(directory: &str) -> Result<(Vec<Name>, Vec<Name>), ProjectError> {
    let mut files = Vec::new();
    let mut directories = Vec::new();
    let reader = match std::fs::read_dir(directory) {
        Ok(reader) => reader,
        Err(error) if disk::is_absence(&error) => return Ok((files, directories)),
        Err(error) => return Err(io_error(Path::new(directory), &error)),
    };
    for entry in reader {
        let entry = entry.map_err(|error| io_error(Path::new(directory), &error))?;
        let name = paths::exact_name(entry.file_name());
        let path = entry.path();
        match disk::entry(&path).map_err(|error| io_error(&path, &error))? {
            Some(disk::Entry::Directory) => directories.push(name),
            Some(disk::Entry::File) => files.push(name),
            Some(disk::Entry::Other) | None => {}
        }
    }
    let key = |name: &Name| -> String {
        let (Ok(text) | Err(text)) = name;
        text.clone()
    };
    files.sort_by_key(key);
    directories.sort_by_key(key);
    Ok((files, directories))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(include: &[&str], exclude: &[&str], case_sensitive: bool) -> Matcher {
        Matcher {
            include: include.iter().map(|path| pattern(path)).collect(),
            exclude: exclude.iter().map(|path| pattern(path)).collect(),
            case: if case_sensitive {
                Case::Sensitive
            } else {
                Case::Insensitive
            },
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
