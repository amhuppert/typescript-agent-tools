//! Temporary projects whose sources mark expected reference groups.
//!
//! A comment `/*@a*/` immediately before an identifier puts that token in
//! group `a`; `/*@a,b*/` puts it in two groups. Roles may follow a colon:
//! `d` declaration, `u` use, `i` import, and `e` export, as in `/*@a:de*/`.
//! `/*@!*/` expects an unsupported target and `/*@?*/` an unresolved target.
//! Markers are comments, so the same sources remain valid TypeScript.
#![allow(dead_code)]

pub mod service;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use typescript_parser::analysis::{AnalysisError, InputError, QueryError, QueryErrorKind};
use typescript_parser::project::{Project, ProjectRequest, ProjectResponse, ProjectSession};
use typescript_parser::references::{
    Location, ReferenceGroup, ReferenceQuery, ReferenceResult, Role,
};
use typescript_parser::source::SourcePosition;

/// A reference query through a session's one request boundary.
pub fn references(
    session: &mut ProjectSession,
    query: ReferenceQuery,
) -> Result<ReferenceResult, AnalysisError> {
    session
        .execute(ProjectRequest::References(query))
        .map(|response| match response {
            ProjectResponse::References(result) => result,
            other => panic!("a references request answered {other:?}"),
        })
}

/// The query failure of a result that must be one.
pub fn query_error<T: std::fmt::Debug>(result: Result<T, AnalysisError>) -> QueryError {
    match result {
        Err(AnalysisError::Query(error)) => error,
        other => panic!("expected a query failure: {other:?}"),
    }
}

/// `query_error` as a method, for a query's result.
pub trait QueryFailure {
    fn query_error(self) -> QueryError;
}

impl<T: std::fmt::Debug> QueryFailure for Result<T, AnalysisError> {
    fn query_error(self) -> QueryError {
        query_error(self)
    }
}

/// The configuration of most test projects: no standard library.
pub const CONFIG: &str = r#"{ "compilerOptions": { "noLib": true } }"#;

/// A temporary `CONFIG` project with `files`.
pub fn project(files: &[(&str, &str)]) -> Fixture {
    let mut all = vec![("tsconfig.json", CONFIG)];
    all.extend_from_slice(files);
    Fixture::new(&all)
}

/// Query every marked token of a temporary `CONFIG` project with `files`.
pub fn check(files: &[(&str, &str)]) {
    let fixture = project(files);
    fixture.check_markers(&fixture.load());
}

/// Assert the legacy input classification at a load boundary. Semantic and
/// query failures must fail these fixtures, never masquerade as input errors.
pub fn input_error(error: AnalysisError) -> InputError {
    match error {
        AnalysisError::Input(error) => error,
        AnalysisError::Semantic(error) => {
            panic!("expected an input failure, got semantic failure: {error:?}")
        }
        AnalysisError::Query(error) => {
            panic!("expected an input failure, got query failure: {error:?}")
        }
    }
}

/// The error from loading a temporary `CONFIG` project with `files`.
pub fn load_error(files: &[(&str, &str)]) -> InputError {
    let fixture = project(files);
    Project::load(&fixture.path("tsconfig.json"))
        .map_err(input_error)
        .expect_err("the project should fail to load")
}

/// The directory of the checked-in marker fixtures.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/references")
}

pub struct Fixture {
    pub root: PathBuf,
    /// Temporary fixtures are removed on drop; checked-in ones are not.
    temporary: bool,
}

impl Fixture {
    /// Write `(relative path, contents)` pairs into a new temporary directory.
    pub fn new(files: &[(&str, &str)]) -> Self {
        let root = temporary_directory();
        for (path, contents) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        Self::temporary(root)
    }

    /// A checked-in fixture under `fixtures/references`, which is never modified.
    pub fn checked_in(name: &str) -> Self {
        Self::existing(&fixtures_root().join(name))
    }

    /// A checked-in fixture directory, which is never modified.
    pub fn existing(root: &Path) -> Self {
        Self {
            root: std::fs::canonicalize(root).unwrap(),
            temporary: false,
        }
    }

    /// A temporary copy of a checked-in fixture, for tests that change files.
    pub fn copy_of(name: &str) -> Self {
        fn copy(from: &Path, to: &Path) {
            std::fs::create_dir_all(to).unwrap();
            for entry in std::fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    copy(&entry.path(), &target);
                } else {
                    std::fs::copy(entry.path(), target).unwrap();
                }
            }
        }
        let root = temporary_directory();
        copy(&fixtures_root().join(name), &root);
        Self::temporary(root)
    }

    fn temporary(root: PathBuf) -> Self {
        // Results are relative to the canonical configuration directory.
        Self {
            root: std::fs::canonicalize(&root).unwrap(),
            temporary: true,
        }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn load(&self) -> Project {
        self.load_config("tsconfig.json")
    }

    pub fn load_config(&self, config: &str) -> Project {
        Project::load(&self.path(config))
            .unwrap_or_else(|error| panic!("project failed to load: {error}"))
    }

    pub fn markers(&self) -> Vec<Marker> {
        let mut markers = Vec::new();
        collect_markers(&self.root, &self.root, &mut markers);
        markers.sort_by(|a, b| a.location.cmp(&b.location));
        markers
    }

    /// Query every marked token and compare its groups with the marked sets.
    pub fn check_markers(&self, project: &Project) {
        let markers = self.markers();
        self.check_marker_set(project, &markers);
    }

    /// Query markers supplied in analyzed-source coordinates. Encoding-prefix
    /// tests retain that template separately from the bytes written to disk.
    pub fn check_marker_set(&self, project: &Project, markers: &[Marker]) {
        assert!(!markers.is_empty(), "fixture has no markers");
        let groups = expected_groups(markers);
        for marker in markers {
            let query = ReferenceQuery::At {
                file: self.path(&marker.location.file),
                position: marker.location.start,
            };
            let result = project.find_references(query);
            let context = || format!("query at {} `{}`", describe(&marker.location), marker.name);
            match &marker.expect {
                Expect::Unsupported | Expect::Unresolved => {
                    let wanted = if marker.expect == Expect::Unsupported {
                        QueryErrorKind::UnsupportedTarget
                    } else {
                        QueryErrorKind::UnresolvedTarget
                    };
                    match result {
                        Err(AnalysisError::Query(error)) if error.kind == wanted => {}
                        Err(error) => panic!("{}: expected {wanted:?}, got {error}", context()),
                        Ok(result) => panic!(
                            "{}: expected {wanted:?}, got groups\n{}",
                            context(),
                            render(&result.groups)
                        ),
                    }
                }
                Expect::Groups { labels, .. } => {
                    let result = result.unwrap_or_else(|error| panic!("{}: {error}", context()));
                    let wanted: Vec<_> =
                        labels.iter().map(|label| (label, &groups[label])).collect();
                    compare_groups(&context(), &result.groups, &wanted, markers);
                }
            }
        }
    }

    /// Compare an exact-name query with the marked groups `labels`.
    pub fn check_name(&self, project: &Project, name: &str, file: Option<&str>, labels: &[&str]) {
        let markers = self.markers();
        let groups = expected_groups(&markers);
        let query = ReferenceQuery::Name {
            name: name.into(),
            file: file.map(|file| self.path(file)),
        };
        let context = format!("name query `{name}` in {file:?}");
        let result = project
            .find_references(query)
            .unwrap_or_else(|error| panic!("{context}: {error}"));
        let labels: Vec<String> = labels.iter().map(|label| label.to_string()).collect();
        let wanted: Vec<_> = labels
            .iter()
            .map(|label| {
                let set = groups
                    .get(label)
                    .unwrap_or_else(|| panic!("no marker labelled {label}"));
                (label, set)
            })
            .collect();
        compare_groups(&context, &result.groups, &wanted, &markers);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // TSPARSE_KEEP_FIXTURES keeps temporary projects for oracle checks.
        if self.temporary && std::env::var_os("TSPARSE_KEEP_FIXTURES").is_none() {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    Groups {
        labels: Vec<String>,
        roles: Option<BTreeSet<Role>>,
    },
    Unsupported,
    Unresolved,
}

#[derive(Debug, Clone)]
pub struct Marker {
    pub location: Location,
    pub name: String,
    pub expect: Expect,
}

fn expected_groups(markers: &[Marker]) -> BTreeMap<String, BTreeSet<Location>> {
    let mut groups: BTreeMap<String, BTreeSet<Location>> = BTreeMap::new();
    for marker in markers {
        if let Expect::Groups { labels, .. } = &marker.expect {
            for label in labels {
                groups
                    .entry(label.clone())
                    .or_default()
                    .insert(marker.location.clone());
            }
        }
    }
    groups
}

fn compare_groups(
    context: &str,
    actual: &[ReferenceGroup],
    wanted: &[(&String, &BTreeSet<Location>)],
    markers: &[Marker],
) {
    let actual_sets: Vec<BTreeSet<Location>> = actual
        .iter()
        .map(|group| {
            group
                .occurrences
                .iter()
                .map(|occurrence| occurrence.location.clone())
                .collect()
        })
        .collect();
    let mut report = String::new();
    for (label, set) in wanted {
        if !actual_sets.contains(set) {
            let _ = writeln!(report, "missing group `{label}`:");
            for location in *set {
                let _ = writeln!(report, "  {}", describe(location));
            }
        }
    }
    if actual.len() != wanted.len() {
        let _ = writeln!(
            report,
            "expected {} groups, got {}",
            wanted.len(),
            actual.len()
        );
    }
    if !report.is_empty() {
        panic!("{context}:\n{report}actual:\n{}", render(actual));
    }
    for group in actual {
        for occurrence in &group.occurrences {
            let Some(marker) = markers.iter().find(|m| m.location == occurrence.location) else {
                continue;
            };
            assert_eq!(
                occurrence.name,
                marker.name,
                "{context}: name at {}",
                describe(&occurrence.location)
            );
            if let Expect::Groups {
                roles: Some(roles), ..
            } = &marker.expect
            {
                let actual: BTreeSet<Role> = occurrence.roles.iter().copied().collect();
                assert_eq!(
                    &actual,
                    roles,
                    "{context}: roles at {}",
                    describe(&occurrence.location)
                );
            }
        }
        // Declarations on marked tokens must be exactly the declaration-role
        // occurrences; synthetic anchors and package declarations are not marked.
        let marked: BTreeSet<&Location> = markers.iter().map(|m| &m.location).collect();
        let marked_declarations: BTreeSet<Location> = group
            .declarations
            .iter()
            .filter(|declaration| marked.contains(&declaration.location))
            .map(|declaration| declaration.location.clone())
            .collect();
        let declaration_roles: BTreeSet<Location> = group
            .occurrences
            .iter()
            .filter(|occurrence| occurrence.roles.contains(&Role::Declaration))
            .map(|occurrence| occurrence.location.clone())
            .collect();
        assert_eq!(
            marked_declarations,
            declaration_roles,
            "{context}: marked declarations should be the declaration-role occurrences\n{}",
            render(actual)
        );
    }
}

pub fn describe(location: &Location) -> String {
    format!(
        "{}:{}:{}-{}:{}",
        location.file,
        location.start.line,
        location.start.column,
        location.end.line,
        location.end.column
    )
}

pub fn render(groups: &[ReferenceGroup]) -> String {
    let mut out = String::new();
    for (index, group) in groups.iter().enumerate() {
        let _ = writeln!(out, "group {index}:");
        for declaration in &group.declarations {
            let _ = writeln!(
                out,
                "  declaration {} {}",
                declaration.name,
                describe(&declaration.location)
            );
        }
        for occurrence in &group.occurrences {
            let _ = writeln!(
                out,
                "  {} {} {:?}",
                occurrence.name,
                describe(&occurrence.location),
                occurrence.roles
            );
        }
    }
    out
}

fn temporary_directory() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "tsparse-references-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn collect_markers(root: &Path, directory: &Path, markers: &mut Vec<Marker>) {
    let mut entries: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_markers(root, &path, markers);
            continue;
        }
        let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !matches!(extension, "ts" | "tsx" | "mts" | "cts") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let file = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        markers.extend(parse_markers(&text, &file));
    }
}

/// The markers of one source file, in source order. The grammar is the one
/// documented at the top of this module; tools/semantic-conformance.mjs
/// parses it identically. Malformed markers panic.
pub fn parse_markers(text: &str, file: &str) -> Vec<Marker> {
    let mut markers = Vec::new();
    let mut search = 0;
    while let Some(found) = text[search..].find("/*@") {
        let open = search + found;
        let at = || {
            let position = position(text, open);
            format!("{file}:{}:{}", position.line, position.column)
        };
        let close = open
            + text[open..]
                .find("*/")
                .unwrap_or_else(|| panic!("unclosed marker at {}", at()));
        let spec = &text[open + 3..close];
        let start = close + 2;
        let end = start + identifier_length(&text[start..]);
        assert!(end > start, "marker at {} precedes no identifier", at());
        let expect = match spec {
            "!" => Expect::Unsupported,
            "?" => Expect::Unresolved,
            _ => {
                let (labels, roles) = match spec.split_once(':') {
                    Some((labels, roles)) => (labels, Some(parse_roles(roles, &at))),
                    None => (spec, None),
                };
                let labels: Vec<String> = labels.split(',').map(str::to_string).collect();
                assert!(
                    labels.iter().all(|label| !label.is_empty()),
                    "empty marker label at {}",
                    at()
                );
                Expect::Groups { labels, roles }
            }
        };
        markers.push(Marker {
            location: Location {
                file: file.to_string(),
                start: position(text, start),
                end: position(text, end),
            },
            name: decode(&text[start..end]),
            expect,
        });
        search = start;
    }
    markers
}

fn parse_roles(roles: &str, at: &dyn Fn() -> String) -> BTreeSet<Role> {
    roles
        .chars()
        .map(|role| match role {
            'd' => Role::Declaration,
            'u' => Role::Use,
            'i' => Role::Import,
            'e' => Role::Export,
            _ => panic!("unknown role {role} at {}", at()),
        })
        .collect()
}

fn identifier_length(text: &str) -> usize {
    let private = usize::from(text.starts_with('#'));
    let mut length = private;
    let mut rest = &text[private..];
    loop {
        if let Some(after) = rest.strip_prefix("\\u{") {
            let close = after.find('}').unwrap() + 1;
            length += 3 + close;
            rest = &after[close..];
        } else if rest.starts_with("\\u") {
            length += 6;
            rest = &rest[6..];
        } else if let Some(c) = rest
            .chars()
            .next()
            .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '\u{200C}' | '\u{200D}'))
        {
            length += c.len_utf8();
            rest = &rest[c.len_utf8()..];
        } else {
            return length;
        }
    }
}

fn decode(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("\\u{") {
            let close = after.find('}').unwrap();
            let value = u32::from_str_radix(&after[..close], 16).unwrap();
            out.push(char::from_u32(value).unwrap());
            rest = &after[close + 1..];
        } else if let Some(after) = rest.strip_prefix("\\u") {
            let value = u32::from_str_radix(&after[..4], 16).unwrap();
            out.push(char::from_u32(value).unwrap());
            rest = &after[4..];
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// One-based line and UTF-16 column, independent of the library's conversion.
pub fn position(text: &str, byte: usize) -> SourcePosition {
    let mut line = 1;
    let mut column = 1;
    let mut previous_cr = false;
    for c in text[..byte].chars() {
        match c {
            '\n' if previous_cr => {}
            '\n' | '\r' | '\u{2028}' | '\u{2029}' => {
                line += 1;
                column = 1;
            }
            _ => column += c.len_utf16(),
        }
        previous_cr = c == '\r';
    }
    SourcePosition { line, column }
}
