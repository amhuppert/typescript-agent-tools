//! Module and type-reference resolution for TypeScript 7's `node16`,
//! `nodenext`, and `bundler` modes, in the compiler's lookup order:
//! `paths` or `rootDirs`, then package `imports`, a self-name reference,
//! ancestor `node_modules` (TypeScript and declarations first, including
//! `@types`, then JavaScript), and finally explicit `typeRoots`.
//!
//! Lookups return the path as found; callers decide file identity. Results
//! are cached per directory, specifier, and resolution mode for one load.

use super::config::{Options, Resolution};
use super::jsonc::{self, Kind, Value};
use super::{paths, version};
use crate::references::binding::ResolutionMode;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

/// Extension families a lookup may produce.
const TS: u8 = 1;
const DTS: u8 = 2;
const JS: u8 = 4;
const JSON: u8 = 8;

/// How many `imports` targets may name further lookups in one chain. Valid
/// chains are far shorter; a longer one is reported as unsupported rather
/// than missing. Loading runs on a thread whose stack fits this recursion.
/// Cycles end earlier, when a lookup repeats.
pub(super) const MAX_IMPORTS_CHAIN: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Resolved {
    /// A TypeScript or declaration file. `external` marks the compiler's
    /// external-library imports: package lookups and paths in `node_modules`.
    File {
        path: String,
        external: bool,
    },
    /// JavaScript found without TypeScript declarations.
    JavaScript(String),
    Json(String),
    Missing,
    /// A package `imports` chain longer than `MAX_IMPORTS_CHAIN`.
    TooDeep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Format {
    CommonJs,
    Esm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FileFormat {
    /// The Node.js format TypeScript implies for the file.
    pub(super) implied: Format,
    /// The format fixed by a `.mts`/`.cts` extension or a consulted
    /// `package.json` `type`, which overrides `module` in bundler mode.
    pub(super) declared: Option<Format>,
}

#[derive(Debug, Default)]
struct PackageJson {
    dir: String,
    name: Option<String>,
    kind: Option<String>,
    types: Option<String>,
    typings: Option<String>,
    typings_null: bool,
    main: Option<String>,
    tsconfig: Option<String>,
    /// `exports` when present and not `null`.
    exports: Option<Value>,
    /// `exports` when it maps subpaths.
    subpaths: Option<Table>,
    has_exports: bool,
    imports: Option<Table>,
    types_versions: Option<Value>,
}

/// An `exports` subpath or `imports` table, indexed once so a chain of
/// lookups through it stays linear.
#[derive(Debug)]
struct Table {
    entries: Vec<(String, Value)>,
    /// The first entry for each key.
    exact: HashMap<String, usize>,
    /// Pattern and directory keys in TypeScript's pattern-key order.
    expanding: Vec<usize>,
}

impl Table {
    fn new(entries: &[(String, Value)]) -> Self {
        let mut exact = HashMap::new();
        for (index, (key, _)) in entries.iter().enumerate() {
            exact.entry(key.clone()).or_insert(index);
        }
        let mut expanding: Vec<usize> = (0..entries.len())
            .filter(|&index| {
                let key = &entries[index].0;
                key.matches('*').count() == 1 || key.ends_with('/')
            })
            .collect();
        expanding.sort_by(|&a, &b| compare_pattern_keys(&entries[a].0, &entries[b].0));
        Self {
            entries: entries.to_vec(),
            exact,
            expanding,
        }
    }
}

impl PackageJson {
    /// A missing or unparsable manifest still establishes a package scope.
    fn read(dir: &str, path: &str) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let Some(value) = jsonc::parse(&text).ok().filter(|v| v.entries().is_some()) else {
            return Self {
                dir: dir.to_owned(),
                ..Self::default()
            };
        };
        let string = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_owned);
        let exports = value.field("exports");
        let subpaths = exports
            .and_then(Value::entries)
            .filter(|entries| entries.iter().all(|(key, _)| key.starts_with('.')))
            .map(Table::new);
        Self {
            dir: dir.to_owned(),
            name: string("name"),
            kind: string("type"),
            types: string("types"),
            typings: string("typings"),
            typings_null: value.get("typings").is_some_and(|v| v.kind == Kind::Null),
            main: string("main"),
            tsconfig: string("tsconfig"),
            has_exports: value.get("exports").is_some(),
            exports: exports.cloned(),
            subpaths,
            imports: value
                .field("imports")
                .and_then(Value::entries)
                .map(Table::new),
            types_versions: value.field("typesVersions").cloned(),
        }
    }

    /// The `typesVersions` path map of the first range admitting the
    /// reference compiler.
    fn version_paths(&self) -> Option<Vec<(String, Vec<String>)>> {
        let ranges = self.types_versions.as_ref()?.entries()?;
        let (_, paths) = ranges
            .iter()
            .find(|(range, _)| version::matches_typescript(range) == Some(true))?;
        Some(
            paths
                .entries()?
                .iter()
                .map(|(pattern, targets)| {
                    let targets = targets
                        .as_array()
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|target| target.as_str().map(str::to_owned))
                        .collect();
                    (pattern.clone(), targets)
                })
                .collect(),
        )
    }
}

/// Cached file system queries.
#[derive(Default)]
struct Host {
    files: HashMap<String, bool>,
    directories: HashMap<String, bool>,
    packages: HashMap<String, Option<Rc<PackageJson>>>,
}

impl Host {
    fn file_exists(&mut self, path: &str) -> bool {
        *self
            .files
            .entry(path.to_owned())
            .or_insert_with(|| Path::new(path).is_file())
    }

    fn directory_exists(&mut self, path: &str) -> bool {
        *self
            .directories
            .entry(path.to_owned())
            .or_insert_with(|| Path::new(path).is_dir())
    }

    fn package(&mut self, dir: &str) -> Option<Rc<PackageJson>> {
        let dir = if dir.len() > 1 {
            dir.trim_end_matches('/')
        } else {
            dir
        };
        if let Some(package) = self.packages.get(dir) {
            return package.clone();
        }
        let path = paths::combine(dir, "package.json");
        let package = self
            .file_exists(&path)
            .then(|| Rc::new(PackageJson::read(dir, &path)));
        self.packages.insert(dir.to_owned(), package.clone());
        package
    }

    /// The nearest `package.json` at or above `dir`.
    fn scope(&mut self, dir: &str) -> Option<Rc<PackageJson>> {
        paths::ancestors(dir).find_map(|ancestor| self.package(ancestor))
    }
}

/// Per-lookup settings: TypeScript's resolution features and conditions.
#[derive(Clone)]
struct State {
    extensions: u8,
    /// ESM-mode Node.js resolution: no implicit extensions or directory
    /// indexes for relative paths.
    esm: bool,
    exports: bool,
    imports: bool,
    /// Whether `imports` keys may begin with `#/`.
    imports_root: bool,
    conditions: Rc<[String]>,
    config_lookup: bool,
}

impl State {
    fn with_extensions(&self, extensions: u8) -> Self {
        Self {
            extensions,
            ..self.clone()
        }
    }

    fn applies(&self, condition: &str) -> bool {
        condition == "default"
            || self.conditions.iter().any(|c| c == condition)
            || condition.strip_prefix("types@").is_some_and(|range| {
                self.conditions.iter().any(|c| c == "types")
                    && version::matches_typescript(range) == Some(true)
            })
    }
}

/// The continuation applied to a candidate path by `paths` and
/// `typesVersions` lookups.
#[derive(Clone)]
enum Loader {
    /// A relative module name, which may name a directory with a manifest.
    Relative,
    /// A file named by a manifest field of this package directory.
    PackageField(Option<Rc<PackageJson>>),
    /// A path inside a package in `node_modules`.
    Package {
        package: Option<Rc<PackageJson>>,
        subpath: bool,
    },
}

/// A lookup named by an `imports` target: the package directory, the name,
/// and the extension families and configuration-lookup flag of its state.
type Hop = (String, String, u8, bool);

pub(super) struct Resolver<'a> {
    options: &'a Options,
    host: Host,
    modules: HashMap<(String, String, bool), Resolved>,
    /// Results of `imports` hops during one module resolution, so branches
    /// that share a lookup evaluate it once.
    hops: HashMap<Hop, Option<String>>,
    /// The hops in progress; repeating one ends that route as a cycle.
    active: HashSet<Hop>,
    too_deep: bool,
}

impl<'a> Resolver<'a> {
    pub(super) fn new(options: &'a Options) -> Self {
        Self {
            options,
            host: Host::default(),
            modules: HashMap::new(),
            hops: HashMap::new(),
            active: HashSet::new(),
            too_deep: false,
        }
    }

    fn state(&self, mode: Option<ResolutionMode>, extensions: u8) -> State {
        let node = self.options.resolution != Resolution::Bundler;
        let import = match mode {
            Some(ResolutionMode::Import) => true,
            Some(ResolutionMode::Require) => false,
            None => !node,
        };
        let mut conditions = vec![
            if import { "import" } else { "require" }.to_owned(),
            "types".to_owned(),
        ];
        if node {
            conditions.push("node".into());
        }
        conditions.extend(self.options.custom_conditions.iter().cloned());
        State {
            extensions,
            esm: node && mode == Some(ResolutionMode::Import),
            exports: self.options.package_exports,
            imports: self.options.package_imports,
            imports_root: self.options.resolution != Resolution::Node16,
            conditions: conditions.into(),
            config_lookup: false,
        }
    }

    /// Resolves a module specifier written in `file`.
    pub(super) fn module(&mut self, specifier: &str, file: &str, mode: ResolutionMode) -> Resolved {
        let directory = paths::parent(file).to_owned();
        let key = (
            directory.clone(),
            specifier.to_owned(),
            mode == ResolutionMode::Import,
        );
        if let Some(resolved) = self.modules.get(&key) {
            return resolved.clone();
        }
        let mut extensions = TS | DTS | JS;
        if self.options.resolve_json_module {
            extensions |= JSON;
        }
        let state = self.state(Some(mode), extensions);
        self.hops.clear();
        self.too_deep = false;
        let found = self.node_like(&state, specifier, &directory);
        let resolved = match found {
            _ if self.too_deep => Resolved::TooDeep,
            None => Resolved::Missing,
            Some((path, _)) if paths::is_javascript(&path) => Resolved::JavaScript(path),
            Some((path, _)) if path.ends_with(".json") => Resolved::Json(path),
            Some((path, package)) => Resolved::File {
                external: package || paths::has_node_modules(&path),
                path,
            },
        };
        self.modules.insert(key, resolved.clone());
        resolved
    }

    /// Resolves a type library named by `types` or `/// <reference types>`:
    /// type roots first, then, unless `typeRoots` is explicit, `node_modules`
    /// above `directory`.
    pub(super) fn type_reference(
        &mut self,
        name: &str,
        directory: &str,
        mode: Option<ResolutionMode>,
    ) -> Option<String> {
        let state = self.state(mode, DTS);
        for root in self.type_roots() {
            if !self.host.directory_exists(&root) {
                continue;
            }
            let candidate = type_root_candidate(&root, name);
            if let Some(found) = self.load_from_directory(&state, &candidate, true) {
                if paths::is_typescript(&found) {
                    return Some(found);
                }
            }
        }
        if self.options.type_roots.is_some() {
            return None;
        }
        let found = if is_relative_name(name) {
            self.load_relative(&state, &relative_candidate(directory, name), true)
        } else {
            self.node_modules(&state, name, directory)
        };
        found.filter(|found| paths::is_typescript(found))
    }

    /// The package names `"types": ["*"]` selects: each type root's
    /// subdirectories, except dot-names and `"typings": null` stubs.
    pub(super) fn automatic_types(&mut self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for root in self.type_roots() {
            let Ok(entries) = std::fs::read_dir(&root) else {
                continue;
            };
            let mut directories: Vec<String> = entries
                .flatten()
                .filter(|entry| std::fs::metadata(entry.path()).is_ok_and(|m| m.is_dir()))
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            directories.sort();
            for name in directories {
                let stub = self
                    .host
                    .package(&paths::combine(&root, &name))
                    .is_some_and(|package| package.typings_null);
                if !name.starts_with('.') && !stub && !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        names
    }

    fn type_roots(&self) -> Vec<String> {
        match &self.options.type_roots {
            Some(roots) => roots.clone(),
            None => paths::ancestors(&self.options.config_dir)
                .map(|dir| paths::combine(dir, "node_modules/@types"))
                .collect(),
        }
    }

    pub(super) fn file_format(&mut self, path: &str) -> FileFormat {
        let fixed = |format| FileFormat {
            implied: format,
            declared: Some(format),
        };
        if paths::has_extension(path, &[".mts", ".mjs"]) {
            return fixed(Format::Esm);
        }
        if paths::has_extension(path, &[".cts", ".cjs"]) {
            return fixed(Format::CommonJs);
        }
        // Bundler mode consults `type` only inside `node_modules`.
        let consult =
            self.options.resolution != Resolution::Bundler || paths::has_node_modules(path);
        let scope = if consult {
            self.host.scope(paths::parent(path))
        } else {
            None
        };
        match scope.as_ref().and_then(|package| package.kind.as_deref()) {
            Some("module") => fixed(Format::Esm),
            Some("commonjs") => fixed(Format::CommonJs),
            _ => FileFormat {
                implied: Format::CommonJs,
                declared: None,
            },
        }
    }

    /// The located file and whether it came from a `node_modules` lookup.
    fn node_like(&mut self, state: &State, name: &str, dir: &str) -> Option<(String, bool)> {
        if let Some(found) = self.optional_settings(state, name, dir) {
            return Some((found, false));
        }
        if is_relative_name(name) {
            let candidate = relative_candidate(dir, name);
            return self
                .load_relative(state, &candidate, true)
                .map(|found| (found, false));
        }
        if state.imports && name.starts_with('#') {
            return self
                .imports_lookup(state, name, dir)
                .map(|found| (found, false));
        }
        if let Some(result) = self.self_name_lookup(state, name, dir) {
            return result.map(|found| (found, false));
        }
        if name.contains(':') {
            return None;
        }
        if let Some(found) = self.node_modules(state, name, dir) {
            return Some((found, true));
        }
        if state.extensions & DTS != 0 {
            return self
                .type_root_lookup(state, name)
                .map(|found| (found, false));
        }
        None
    }

    /// `paths` for non-relative names, `rootDirs` for relative ones.
    fn optional_settings(&mut self, state: &State, name: &str, dir: &str) -> Option<String> {
        let options = self.options;
        if let Some(mapping) = &options.paths {
            if !paths::is_relative(name) {
                let found = self.paths_lookup(
                    state,
                    name,
                    &mapping.base,
                    &mapping.patterns,
                    &Loader::Relative,
                );
                if found.is_some() {
                    return found;
                }
            }
        }
        if is_relative_name(name) {
            self.root_dirs(state, name, dir)
        } else {
            None
        }
    }

    /// An exact pattern, else the matching `*` pattern with the longest
    /// prefix; each substitution is tried in order.
    fn paths_lookup(
        &mut self,
        state: &State,
        name: &str,
        base: &str,
        patterns: &[(String, Vec<String>)],
        loader: &Loader,
    ) -> Option<String> {
        let (index, star) = best_pattern(patterns, name)?;
        for substitution in &patterns[index].1 {
            let path = match &star {
                Some(text) => substitution.replacen('*', text, 1),
                None => substitution.clone(),
            };
            let candidate = paths::resolve(base, &path);
            if paths::known_extension(substitution).is_some() {
                if let Some(found) = self.try_file(&candidate) {
                    return Some(found);
                }
            }
            if let Some(found) = self.load(loader, state, &candidate) {
                return Some(found);
            }
        }
        None
    }

    /// A relative name under the longest matching root directory, tried in
    /// place and then under each other root directory.
    fn root_dirs(&mut self, state: &State, name: &str, dir: &str) -> Option<String> {
        let options = self.options;
        if options.root_dirs.is_empty() {
            return None;
        }
        let candidate = paths::resolve(dir, name);
        let mut matched: Option<(&String, String)> = None;
        for root in &options.root_dirs {
            let mut prefix = paths::normalize(root);
            if !prefix.ends_with('/') {
                prefix.push('/');
            }
            if candidate.starts_with(&prefix)
                && matched
                    .as_ref()
                    .is_none_or(|(_, longest)| longest.len() < prefix.len())
            {
                matched = Some((root, prefix));
            }
        }
        let (matched_root, prefix) = matched?;
        if let Some(found) = self.load_relative(state, &candidate, true) {
            return Some(found);
        }
        let suffix = &candidate[prefix.len()..];
        for root in &options.root_dirs {
            if root == matched_root {
                continue;
            }
            let other = paths::combine(&paths::normalize(root), suffix);
            if let Some(found) = self.load_relative(state, &other, true) {
                return Some(found);
            }
        }
        None
    }

    fn load(&mut self, loader: &Loader, state: &State, candidate: &str) -> Option<String> {
        match loader {
            Loader::Relative => self.load_relative(state, candidate, true),
            Loader::PackageField(package) => {
                if let Some(found) = self.package_field(state, candidate) {
                    return Some(found);
                }
                // A manifest field may also name an extensionless file or a
                // directory, resolved as CommonJS unless the package is ESM.
                let mut inner = state.clone();
                if inner.extensions == DTS {
                    inner.extensions = TS | DTS;
                }
                if package.as_ref().and_then(|p| p.kind.as_deref()) != Some("module") {
                    inner.esm = false;
                }
                self.load_relative(&inner, candidate, false)
            }
            Loader::Package { package, subpath } => {
                let file = if *subpath || !state.esm {
                    self.load_from_file(state, candidate)
                } else {
                    None
                };
                file.or_else(|| self.directory(state, candidate, package.clone()))
                    .or_else(|| {
                        // ESM still assumes `index.js` for a package without
                        // `main` or `exports`.
                        let no_exports = package.as_ref().is_some_and(|p| p.exports.is_none());
                        if no_exports && state.esm {
                            self.load_from_file(state, &paths::combine(candidate, "index.js"))
                        } else {
                            None
                        }
                    })
            }
        }
    }

    fn load_relative(&mut self, state: &State, candidate: &str, manifest: bool) -> Option<String> {
        if !candidate.ends_with('/') {
            if let Some(found) = self.load_from_file(state, candidate) {
                return Some(found);
            }
        }
        // ESM-mode relative imports never resolve directories.
        if state.esm {
            return None;
        }
        let directory = candidate.strip_suffix('/').unwrap_or(candidate);
        if !self.host.directory_exists(directory) {
            return None;
        }
        self.load_from_directory(state, directory, manifest)
    }

    fn load_from_file(&mut self, state: &State, candidate: &str) -> Option<String> {
        self.no_implicit_extensions(state, candidate).or_else(|| {
            if state.esm {
                None
            } else {
                self.try_adding_extensions(state, candidate, "")
            }
        })
    }

    /// Replaces a written extension: `./a.js` may be `./a.ts`, and
    /// `./a.css` may be `./a.d.css.ts`.
    fn no_implicit_extensions(&mut self, state: &State, candidate: &str) -> Option<String> {
        if !paths::basename(candidate).contains('.') {
            return None;
        }
        let stem = match paths::known_extension(candidate) {
            Some(extension) => &candidate[..candidate.len() - extension.len()],
            None => &candidate[..candidate.rfind('.')?],
        };
        self.try_adding_extensions(state, stem, &candidate[stem.len()..])
    }

    fn try_adding_extensions(
        &mut self,
        state: &State,
        stem: &str,
        original: &str,
    ) -> Option<String> {
        let has = |family: u8| state.extensions & family != 0;
        let mut order: Vec<String> = Vec::new();
        let mut push = |enabled: bool, extensions: &[&str]| {
            if enabled {
                order.extend(extensions.iter().map(|e| (*e).to_owned()));
            }
        };
        match original {
            ".mjs" | ".mts" | ".d.mts" => {
                push(has(TS), &[".mts"]);
                push(has(DTS), &[".d.mts"]);
                push(has(JS), &[".mjs"]);
            }
            ".cjs" | ".cts" | ".d.cts" => {
                push(has(TS), &[".cts"]);
                push(has(DTS), &[".d.cts"]);
                push(has(JS), &[".cjs"]);
            }
            ".json" => {
                push(has(DTS), &[".json.d.ts"]);
                push(has(JSON), &[".json"]);
            }
            ".tsx" | ".jsx" => {
                push(has(TS), &[".tsx", ".ts"]);
                push(has(DTS), &[".d.ts"]);
                push(has(JS), &[".jsx", ".js"]);
            }
            ".ts" | ".d.ts" | ".js" | "" => {
                push(has(TS), &[".ts", ".tsx"]);
                push(has(DTS), &[".d.ts"]);
                push(has(JS), &[".js", ".jsx"]);
                push(state.config_lookup, &[".json"]);
            }
            other => {
                let declaration = paths::is_declaration(&format!("{stem}{other}"));
                push(has(DTS) && !declaration, &[&format!(".d{other}.ts")]);
            }
        }
        order
            .into_iter()
            .find_map(|extension| self.try_file(&format!("{stem}{extension}")))
    }

    /// An existing file, honoring `moduleSuffixes` before the extension.
    fn try_file(&mut self, path: &str) -> Option<String> {
        let options = self.options;
        if options.module_suffixes.is_empty() {
            return self.host.file_exists(path).then(|| path.to_owned());
        }
        let extension = paths::known_extension(path).unwrap_or("");
        let stem = &path[..path.len() - extension.len()];
        options
            .module_suffixes
            .iter()
            .map(|suffix| format!("{stem}{suffix}{extension}"))
            .find(|candidate| self.host.file_exists(candidate))
    }

    fn load_from_directory(
        &mut self,
        state: &State,
        candidate: &str,
        manifest: bool,
    ) -> Option<String> {
        let package = if manifest {
            self.host.package(candidate)
        } else {
            None
        };
        self.directory(state, candidate, package)
    }

    /// A directory's manifest entry (`typesVersions`, `typings`/`types`,
    /// `main`, or `tsconfig` for configurations), then its index file.
    fn directory(
        &mut self,
        state: &State,
        candidate: &str,
        package: Option<Rc<PackageJson>>,
    ) -> Option<String> {
        let field = package.as_ref().and_then(|p| {
            if state.config_lookup {
                return p.tsconfig.clone();
            }
            let types = (state.extensions & DTS != 0)
                .then(|| p.typings.clone().or_else(|| p.types.clone()))
                .flatten();
            types.or_else(|| {
                (state.extensions & (TS | JS | DTS) != 0)
                    .then(|| p.main.clone())
                    .flatten()
            })
        });
        let package_file = field.map(|field| paths::resolve(candidate, &field));
        let index = paths::combine(
            candidate,
            if state.config_lookup {
                "tsconfig"
            } else {
                "index"
            },
        );
        let loader = Loader::PackageField(package.clone());
        if let Some(version_paths) = package.as_ref().and_then(|p| p.version_paths()) {
            if package_file
                .as_ref()
                .is_none_or(|file| paths::contains(candidate, file, true))
            {
                let target = package_file.as_deref().unwrap_or(&index);
                let name = paths::relative(candidate, target);
                if let Some(found) =
                    self.paths_lookup(state, &name, candidate, &version_paths, &loader)
                {
                    return Some(found);
                }
            }
        }
        if let Some(file) = &package_file {
            if let Some(found) = self.load(&loader, state, file) {
                return Some(found);
            }
        }
        self.load_from_file(state, &index)
    }

    /// A path named by a manifest or an export target: exact for TypeScript
    /// extensions, otherwise with its extension replaced.
    fn package_field(&mut self, state: &State, candidate: &str) -> Option<String> {
        if state.extensions & TS != 0 && paths::is_typescript(candidate)
            || state.extensions & DTS != 0
                && paths::has_extension(candidate, &[".d.ts", ".d.mts", ".d.cts"])
            || state.config_lookup && state.extensions == JSON && candidate.ends_with(".json")
        {
            return self.try_file(candidate);
        }
        self.no_implicit_extensions(state, candidate)
    }

    /// Searches each ancestor's `node_modules` (and `@types`) for TypeScript
    /// and declarations, then again for other extensions.
    fn node_modules(&mut self, state: &State, name: &str, dir: &str) -> Option<String> {
        for extensions in [
            state.extensions & (TS | DTS),
            state.extensions & !(TS | DTS),
        ] {
            if extensions == 0 {
                continue;
            }
            let state = state.with_extensions(extensions);
            for ancestor in paths::ancestors(dir) {
                if paths::basename(ancestor) == "node_modules" {
                    continue;
                }
                let modules = paths::combine(ancestor, "node_modules");
                if !self.host.directory_exists(&modules) {
                    continue;
                }
                if let Some(found) = self.package_lookup(&state, name, &modules) {
                    return Some(found);
                }
                if state.extensions & DTS != 0 {
                    let types = paths::combine(&modules, "@types");
                    if self.host.directory_exists(&types) {
                        let found = self.package_lookup(
                            &state.with_extensions(DTS),
                            &mangle_scoped(name),
                            &types,
                        );
                        if found.is_some() {
                            return found;
                        }
                    }
                }
            }
        }
        None
    }

    /// A package name, possibly with a subpath, in one `node_modules`.
    /// `exports` takes precedence and blocks other lookups.
    fn package_lookup(&mut self, state: &State, name: &str, modules: &str) -> Option<String> {
        let candidate = paths::resolve(modules, name);
        let (package_name, rest) = split_package_name(name);
        let package_dir = paths::combine(modules, package_name);
        let nested = self.host.package(&candidate);
        let mut root = None;
        if !rest.is_empty() && nested.is_some() {
            let root_has_exports = state.exports && {
                let package = self.host.package(&package_dir);
                let has = package.as_ref().is_some_and(|p| p.has_exports);
                root = Some(package);
                has
            };
            if !root_has_exports {
                return self
                    .load_from_file(state, &candidate)
                    .or_else(|| self.directory(state, &candidate, nested));
            }
        }
        let package = if rest.is_empty() {
            nested
        } else {
            root.unwrap_or_else(|| self.host.package(&package_dir))
        };
        if state.exports {
            if let Some(package) = package.as_ref().filter(|p| p.exports.is_some()) {
                let subpath = if rest.is_empty() {
                    ".".to_owned()
                } else {
                    format!("./{rest}")
                };
                return self.exports(state, package, &subpath).flatten();
            }
        }
        let loader = Loader::Package {
            package: package.clone(),
            subpath: !rest.is_empty(),
        };
        if !rest.is_empty() {
            if let Some(version_paths) = package.as_ref().and_then(|p| p.version_paths()) {
                if let Some(found) =
                    self.paths_lookup(state, rest, &package_dir, &version_paths, &loader)
                {
                    return Some(found);
                }
            }
        }
        self.load(&loader, state, &candidate)
    }

    /// `Some(None)` is a definitive failure; `None` means no export entry
    /// applied, so other lookups may continue.
    fn exports(
        &mut self,
        state: &State,
        scope: &Rc<PackageJson>,
        subpath: &str,
    ) -> Option<Option<String>> {
        let Some(exports) = scope.exports.as_ref() else {
            return Some(None);
        };
        if subpath == "." {
            let main = match &exports.kind {
                Kind::String(_) | Kind::Array(_) => Some(exports),
                Kind::Object(entries) if entries.iter().all(|(key, _)| !key.starts_with('.')) => {
                    Some(exports)
                }
                Kind::Object(_) => exports.get("."),
                _ => None,
            };
            return match main {
                Some(main) => self.target(state, scope, main, "", false, false),
                None => Some(None),
            };
        }
        match &scope.subpaths {
            Some(table) => Some(
                self.lookup_table(state, scope, subpath, table, false)
                    .flatten(),
            ),
            None => Some(None),
        }
    }

    /// An exact `exports`/`imports` key, else the best pattern or directory
    /// key, ordered by TypeScript's pattern-key comparison.
    fn lookup_table(
        &mut self,
        state: &State,
        scope: &Rc<PackageJson>,
        name: &str,
        table: &Table,
        imports: bool,
    ) -> Option<Option<String>> {
        if !name.ends_with('/') && !name.contains('*') {
            if let Some(&index) = table.exact.get(name) {
                let target = &table.entries[index].1;
                return self.target(state, scope, target, "", false, imports);
            }
        }
        for &index in &table.expanding {
            let (key, target) = &table.entries[index];
            if let Some((prefix, suffix)) = key.split_once('*') {
                if name.starts_with(prefix)
                    && name.ends_with(suffix)
                    && name.len() >= prefix.len() + suffix.len()
                {
                    let middle = &name[prefix.len()..name.len() - suffix.len()];
                    return self.target(state, scope, target, middle, true, imports);
                }
            } else if let Some(rest) = name.strip_prefix(key.as_str()) {
                return self.target(state, scope, target, rest, false, imports);
            }
        }
        None
    }

    /// Evaluates a target: condition objects in key order and arrays in
    /// element order, falling back past candidates whose files are missing;
    /// `null` blocks resolution. Nesting uses an explicit stack.
    fn target(
        &mut self,
        state: &State,
        scope: &Rc<PackageJson>,
        target: &Value,
        subpath: &str,
        pattern: bool,
        imports: bool,
    ) -> Option<Option<String>> {
        let mut stack: Vec<(&Value, usize)> = Vec::new();
        let mut current = Some(target);
        loop {
            if let Some(value) = current.take() {
                match &value.kind {
                    Kind::String(text) => {
                        let found =
                            self.string_target(state, scope, text, subpath, pattern, imports);
                        if found.is_some() {
                            return Some(found);
                        }
                    }
                    Kind::Object(_) => stack.push((value, 0)),
                    Kind::Array(items) if !items.is_empty() => stack.push((value, 0)),
                    Kind::Null => return Some(None),
                    _ => {}
                }
            }
            let Some((container, next)) = stack.last_mut() else {
                return Some(None);
            };
            match &container.kind {
                Kind::Object(entries) => {
                    let found = entries[*next..]
                        .iter()
                        .position(|(condition, _)| state.applies(condition));
                    match found {
                        Some(offset) => {
                            current = Some(&entries[*next + offset].1);
                            *next += offset + 1;
                        }
                        None => {
                            stack.pop();
                        }
                    }
                }
                Kind::Array(items) => {
                    if *next == items.len() {
                        stack.pop();
                    } else {
                        current = Some(&items[*next]);
                        *next += 1;
                    }
                }
                _ => unreachable!("only containers are stacked"),
            }
        }
    }

    fn string_target(
        &mut self,
        state: &State,
        scope: &Rc<PackageJson>,
        target: &str,
        subpath: &str,
        pattern: bool,
        imports: bool,
    ) -> Option<String> {
        if !pattern && !subpath.is_empty() && !target.ends_with('/') {
            return None;
        }
        if !target.starts_with("./") {
            // An `imports` target may name another package.
            if imports
                && !target.starts_with("../")
                && !target.starts_with('/')
                && !paths::is_rooted(target)
            {
                let lookup = if pattern {
                    target.replace('*', subpath)
                } else {
                    format!("{target}{subpath}")
                };
                return self.hop(state, &scope.dir, lookup);
            }
            return None;
        }
        let invalid = |part: &str| matches!(part, ".." | "." | "node_modules");
        if target.split('/').skip(1).any(invalid) || subpath.split('/').any(invalid) {
            return None;
        }
        let resolved = paths::combine(&scope.dir, target);
        let final_path = paths::normalize(&if pattern {
            resolved.replace('*', subpath)
        } else {
            format!("{resolved}{subpath}")
        });
        if let Some(found) = self.input_for_output(state, scope, &final_path) {
            return found;
        }
        self.package_field(state, &final_path)
    }

    /// Looks up a name an `imports` target gives, at most once per module
    /// resolution. A lookup already in progress is a cycle and fails that
    /// route; the compiler itself overflows its stack on one.
    fn hop(&mut self, state: &State, dir: &str, name: String) -> Option<String> {
        let key = (dir.to_owned(), name, state.extensions, state.config_lookup);
        if let Some(found) = self.hops.get(&key) {
            return found.clone();
        }
        if self.active.len() == MAX_IMPORTS_CHAIN {
            self.too_deep = true;
            return None;
        }
        if !self.active.insert(key.clone()) {
            return None;
        }
        let found = self.node_like(state, &key.1, dir).map(|(found, _)| found);
        self.active.remove(&key);
        self.hops.insert(key, found.clone());
        found
    }

    /// Maps an export or import target inside this project's `outDir` or
    /// `declarationDir` back to its source, when `rootDir` (or `composite`)
    /// fixes the source root and the manifest's package contains the
    /// configuration.
    fn input_for_output(
        &mut self,
        state: &State,
        scope: &PackageJson,
        path: &str,
    ) -> Option<Option<String>> {
        let options = self.options;
        if state.config_lookup
            || path.contains("/node_modules/")
            || !paths::contains(&scope.dir, &options.config_file, true)
        {
            return None;
        }
        let common = match (&options.root_dir, options.composite) {
            (Some(root), _) => root.clone(),
            (None, true) => options.config_dir.clone(),
            (None, false) => return None,
        };
        let mut outputs: Vec<&String> = options.declaration_dir.iter().collect();
        if let Some(out_dir) = &options.out_dir {
            if options.declaration_dir.as_ref() != Some(out_dir) {
                outputs.push(out_dir);
            }
        }
        for output in outputs {
            if !paths::contains(output, path, true) || output.len() >= path.len() {
                continue;
            }
            let base = paths::combine(&common, &path[output.len() + 1..]);
            for extension in [".mjs", ".cjs", ".js", ".json", ".d.mts", ".d.cts", ".d.ts"] {
                if !base.ends_with(extension) {
                    continue;
                }
                let inputs: &[&str] = if paths::has_extension(&base, &[".d.mts", ".mjs", ".mts"]) {
                    &[".mts", ".mjs"]
                } else if paths::has_extension(&base, &[".d.cts", ".cjs", ".cts"]) {
                    &[".cts", ".cjs"]
                } else if base.ends_with(".d.json.ts") {
                    &[".json"]
                } else {
                    &[".tsx", ".ts", ".jsx", ".js"]
                };
                for input in inputs {
                    let family = match *input {
                        ".ts" | ".tsx" | ".mts" | ".cts" => TS,
                        ".json" => JSON,
                        _ => JS,
                    };
                    if state.extensions & family == 0 {
                        continue;
                    }
                    let source = format!("{}{input}", &base[..base.len() - extension.len()]);
                    if self.host.file_exists(&source) {
                        return Some(self.package_field(state, &source));
                    }
                }
            }
        }
        None
    }

    /// `#name` through the nearest manifest's `imports`; always definitive.
    fn imports_lookup(&mut self, state: &State, name: &str, dir: &str) -> Option<String> {
        if name == "#" || name.starts_with("#/") && !state.imports_root {
            return None;
        }
        let scope = self.host.scope(dir)?;
        let table = scope.imports.as_ref()?;
        self.lookup_table(state, &scope, name, table, true)
            .flatten()
    }

    /// A package importing itself by name through its own `exports`.
    fn self_name_lookup(&mut self, state: &State, name: &str, dir: &str) -> Option<Option<String>> {
        let scope = self.host.scope(dir)?;
        scope.exports.as_ref()?;
        let package_name = scope.name.as_deref()?;
        let parts: Vec<&str> = name.trim_end_matches('/').split('/').collect();
        let name_parts: Vec<&str> = package_name.trim_end_matches('/').split('/').collect();
        if name_parts.len() > parts.len() || name_parts.iter().zip(&parts).any(|(a, b)| a != b) {
            return None;
        }
        let rest = &parts[name_parts.len()..];
        let subpath = if rest.is_empty() {
            ".".to_owned()
        } else {
            format!("./{}", rest.join("/"))
        };
        let primary = state.with_extensions(state.extensions & (TS | DTS));
        match self.exports(&primary, &scope, &subpath) {
            Some(result) => Some(result),
            None => {
                let secondary = state.with_extensions(state.extensions & !(TS | DTS));
                self.exports(&secondary, &scope, &subpath)
            }
        }
    }

    /// A non-relative name as a declaration package under explicit
    /// `typeRoots`, the last module lookup.
    fn type_root_lookup(&mut self, state: &State, name: &str) -> Option<String> {
        let options = self.options;
        let roots = options.type_roots.as_ref()?;
        let state = state.with_extensions(DTS);
        for root in roots {
            let candidate = type_root_candidate(root, name);
            let found = self
                .load_from_file(&state, &candidate)
                .or_else(|| self.load_from_directory(&state, &candidate, true));
            if found.is_some() {
                return found;
            }
        }
        None
    }
}

/// Resolves a package-name `extends` as TypeScript does: a NodeNext
/// `require` of JSON that honors a manifest's `tsconfig` field and a
/// package's `tsconfig.json`.
pub(super) fn config_package(name: &str, directory: &str) -> Option<String> {
    let options = Options::default();
    let mut resolver = Resolver::new(&options);
    let mut state = resolver.state(Some(ResolutionMode::Require), JSON);
    state.config_lookup = true;
    resolver
        .node_like(&state, name, directory)
        .map(|(found, _)| found)
}

/// `./x`, `../x`, `.`, `..`, or a rooted path.
fn is_relative_name(name: &str) -> bool {
    paths::is_relative(name) || paths::is_rooted(name)
}

/// The candidate path for a relative name; a final `.` or `..` names a
/// directory.
fn relative_candidate(dir: &str, name: &str) -> String {
    let combined = paths::combine(dir, name);
    let last = paths::basename(&combined);
    let mut candidate = paths::normalize(&combined);
    if (last == "." || last == "..") && !candidate.ends_with('/') {
        candidate.push('/');
    }
    candidate
}

fn type_root_candidate(root: &str, name: &str) -> String {
    let root = root.trim_end_matches('/');
    if root.ends_with("/node_modules/@types") {
        paths::resolve(root, &mangle_scoped(name))
    } else {
        paths::resolve(root, name)
    }
}

/// `@scope/name` as published under `@types`: `scope__name`.
fn mangle_scoped(name: &str) -> String {
    match name.strip_prefix('@') {
        Some(rest) if rest.contains('/') => rest.replacen('/', "__", 1),
        _ => name.to_owned(),
    }
}

/// `@scope/name/rest` or `name/rest`, split after the package name.
fn split_package_name(name: &str) -> (&str, &str) {
    let mut slash = name.find('/');
    if name.starts_with('@') {
        slash =
            slash.and_then(|first| name[first + 1..].find('/').map(|second| first + 1 + second));
    }
    match slash {
        Some(index) => (&name[..index], &name[index + 1..]),
        None => (name, ""),
    }
}

/// An exact key, else the single-`*` pattern with the longest prefix
/// (earliest wins ties), with the text matched by `*`.
fn best_pattern(patterns: &[(String, Vec<String>)], name: &str) -> Option<(usize, Option<String>)> {
    if let Some(index) = patterns
        .iter()
        .position(|(key, _)| !key.contains('*') && key == name)
    {
        return Some((index, None));
    }
    let mut best: Option<(usize, usize)> = None;
    for (index, (key, _)) in patterns.iter().enumerate() {
        let Some((prefix, suffix)) = key.split_once('*') else {
            continue;
        };
        if suffix.contains('*') {
            continue;
        }
        if name.len() >= prefix.len() + suffix.len()
            && name.starts_with(prefix)
            && name.ends_with(suffix)
            && best.is_none_or(|(_, length)| prefix.len() > length)
        {
            best = Some((index, prefix.len()));
        }
    }
    let (index, _) = best?;
    let (prefix, suffix) = patterns[index].0.split_once('*')?;
    Some((
        index,
        Some(name[prefix.len()..name.len() - suffix.len()].to_owned()),
    ))
}

/// Longer prefixes before `*` first; then patterns before plain keys; then
/// longer keys.
fn compare_pattern_keys(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (a_star, b_star) = (a.find('*'), b.find('*'));
    let base = |key: &str, star: Option<usize>| star.map_or(key.len(), |index| index + 1);
    match base(b, b_star).cmp(&base(a, a_star)) {
        Ordering::Equal => {}
        other => return other,
    }
    match (a_star, b_star) {
        (None, _) => Ordering::Greater,
        (_, None) => Ordering::Less,
        _ => b.len().cmp(&a.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_mangles_package_names() {
        assert_eq!(split_package_name("pkg"), ("pkg", ""));
        assert_eq!(split_package_name("pkg/a/b"), ("pkg", "a/b"));
        assert_eq!(split_package_name("@s/pkg"), ("@s/pkg", ""));
        assert_eq!(split_package_name("@s/pkg/a"), ("@s/pkg", "a"));
        assert_eq!(mangle_scoped("@s/pkg/a"), "s__pkg/a");
        assert_eq!(mangle_scoped("pkg"), "pkg");
    }

    #[test]
    fn orders_pattern_keys_by_specificity() {
        let mut keys = vec!["./*", "./a/*", "./a/b.js", "./a/*.js", "./a/"];
        keys.sort_by(|a, b| compare_pattern_keys(a, b));
        assert_eq!(keys, ["./a/b.js", "./a/*.js", "./a/*", "./a/", "./*"]);
        let patterns = vec![
            ("al*".to_owned(), vec![]),
            ("alias/*".to_owned(), vec![]),
            ("alias/exact".to_owned(), vec![]),
        ];
        assert_eq!(
            best_pattern(&patterns, "alias/one"),
            Some((1, Some("one".into())))
        );
        assert_eq!(best_pattern(&patterns, "alias/exact"), Some((2, None)));
        assert_eq!(
            best_pattern(&patterns, "alpha"),
            Some((0, Some("pha".into())))
        );
        assert_eq!(best_pattern(&patterns, "beta"), None);
    }
}
