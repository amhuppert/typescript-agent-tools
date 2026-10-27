//! Module and type-reference resolution for TypeScript 7's `node16`,
//! `nodenext`, and `bundler` modes, in the compiler's lookup order:
//! `paths`, then `baseUrl` (TypeScript 5.6 only) or `rootDirs`, then
//! package `imports`, a self-name reference, ancestor `node_modules`
//! (TypeScript and declarations first, including `@types`, then
//! JavaScript), and finally explicit `typeRoots`. Where the two dialects'
//! compilers answer differently, `Rules` decides. TypeScript 5.6.3's
//! standard library replacement lookup (`Resolver::library`) shares the
//! `node_modules` part alone.
//!
//! Lookups retain native ResolvedFileName and OriginalPath separately from
//! the loader's input identity. Results are cached per directory, specifier,
//! and resolution mode for one load.

use super::compatibility::CompilerSelection;
use super::config::{Config, Options, Resolution};
use super::manifest::{Kind, PackageInfo, Value};
use super::paths;
use super::version::{self, Version};
use super::{disk, io_error};
use crate::analysis::InputError;
use crate::input::{self, Case};
use crate::project::binding::ResolutionMode;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

mod package_id;
mod reuse;
pub(super) use package_id::PackageId;
pub(super) use reuse::Reuse;
use reuse::{Access, Memo};

#[cfg(test)]
mod reuse_tests;

/// Extension families a lookup may produce.
const TS: u8 = 1;
const DTS: u8 = 2;
const JS: u8 = 4;
const JSON: u8 = 8;

/// Native `GetConditions` for the admitted resolution modes. NoDtsResolution
/// is not an admitted compiler option, so the native types condition is set.
pub(super) fn resolution_conditions(
    resolution: Resolution,
    custom_conditions: &[String],
    mode: Option<ResolutionMode>,
) -> Vec<String> {
    let import =
        mode == Some(ResolutionMode::Import) || mode.is_none() && resolution == Resolution::Bundler;
    let mut conditions = vec![
        if import { "import" } else { "require" }.to_owned(),
        "types".to_owned(),
    ];
    if resolution != Resolution::Bundler {
        conditions.push("node".to_owned());
    }
    conditions.extend_from_slice(custom_conditions);
    conditions
}

/// The resolution rules of one dialect (`compatibility::Dialect`), which
/// only this module interprets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Rules(Profile);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Profile {
    TypeScript7,
    TypeScript56,
}

impl Rules {
    pub(super) const TYPESCRIPT_7: Self = Self(Profile::TypeScript7);
    pub(super) const TYPESCRIPT_5_6: Self = Self(Profile::TypeScript56);

    /// Whether a matched `paths` or `typesVersions` pattern whose
    /// substitutions are all missing answers its lookup
    /// (`tryLoadModuleUsingPaths`). TypeScript 5.6.3 returns `{ value:
    /// undefined }` for a matched pattern, which ends the optional settings,
    /// a directory's manifest entry, or a package in one `node_modules`;
    /// TypeScript 7.0.2 continues searching as if no pattern matched.
    fn matched_pattern_answers(self) -> bool {
        self.0 == Profile::TypeScript56
    }

    /// The path `tryLoadModuleUsingPaths` tries for one substitution of the
    /// matched pattern, given the text its `*` matched (`None` for an exact
    /// key). TypeScript 7.0.2 replaces the substitution's first `*` with
    /// every capture; 5.6.3 only with a non-empty one (`matchedStar ?
    /// replaceFirstStar(subst, matchedStar) : subst`), so an empty capture
    /// leaves the substitution as written, `*` included. 5.6.3's
    /// `replaceFirstStar` is JavaScript's `String.prototype.replace`, which
    /// would also expand `$$`, `$&`, `` $` `` and `$'` in a capture; this
    /// tool inserts a capture as written in both dialects.
    fn substitution(self, substitution: &str, star: Option<&str>) -> String {
        match (self.0, star) {
            (_, None) | (Profile::TypeScript56, Some("")) => substitution.to_owned(),
            (_, Some(star)) => substitution.replacen('*', star, 1),
        }
    }

    /// Whether a loaded file that a route reaches at a lower depth than any
    /// route before has its subtasks followed again from that depth, so that
    /// JavaScript elided on a deeper route becomes an input within
    /// `maxNodeModuleJsDepth`. TypeScript 7's `filesParser.start`
    /// reprocesses the subtasks of every file seen at a lower depth (7.0.2's
    /// native loader starts them only once, so its listing races where a
    /// deeper route loads the file first). 5.6.3's `findSourceFile`
    /// reprocesses only a file with elided imports (`modulesWithElidedImports`)
    /// reached below the limit, and a file first found searching
    /// `node_modules` once a route reaches it at depth 0
    /// (`sourceFilesFoundSearchingNodeModules`).
    pub(super) fn reprocesses(self, revisit: Revisit) -> bool {
        match self.0 {
            Profile::TypeScript7 => true,
            Profile::TypeScript56 => {
                revisit.elided
                    && i64::try_from(revisit.depth).is_ok_and(|depth| depth < revisit.max_depth)
                    || revisit.found_in_packages && revisit.depth == 0
            }
        }
    }

    /// Whether a file found through a directory's manifest entry or index
    /// takes the directory's package id: 5.6.3's
    /// `loadNodeModuleFromDirectory` gives it one (`withPackageId`), 7.0.2's
    /// none, so a relative or mapped name, or a type root, that reaches a
    /// package directory gives no id under TypeScript 7.
    fn directory_package_id(self) -> bool {
        self.0 == Profile::TypeScript56
    }

    /// Whether the package id that collection deduplicates a file by is the
    /// one the route that first processes it gives: 5.6.3's
    /// `findSourceFileWorker` decides with the package id of the route that
    /// finds the file first, depth first, and with none for a route without
    /// one; 7.0.2's `filesParser.start` keeps the first id any route brings
    /// (`parseTaskData.packageId`).
    pub(super) fn first_route_package_id(self) -> bool {
        self.0 == Profile::TypeScript56
    }

    /// Whether only the routes collection processes decide that a file is
    /// found at depth 0, and so is no external library's. 7.0.2's
    /// `filesParser` gives every loaded task the lowest depth of any route
    /// to it (`lowestDepth`) before `collectFiles` deduplicates, so a
    /// redirect's own subtasks count. 5.6.3's `findSourceFileWorker` marks a
    /// file where it first meets it and clears the mark where a route at
    /// depth 0 meets it again (`sourceFilesFoundSearchingNodeModules`), and
    /// never processes a redirect's own subtasks: one marked where it was met
    /// (`createRedirectedSourceFile`) is processed again with its target's
    /// subtasks, resolved from the redirect's location.
    pub(super) fn collected_provenance(self) -> bool {
        self.0 == Profile::TypeScript56
    }
}

/// A loaded file that a route reaches at a lower depth than any route
/// before (`Rules::reprocesses`).
#[derive(Debug, Clone, Copy)]
pub(super) struct Revisit {
    /// The route's depth, in package lookups.
    pub(super) depth: usize,
    /// `maxNodeModuleJsDepth`.
    pub(super) max_depth: i64,
    /// Whether the file has JavaScript subtasks elided on deeper routes.
    pub(super) elided: bool,
    /// Whether the file was loaded from a route that had searched
    /// `node_modules`.
    pub(super) found_in_packages: bool,
}

/// Native condition selection and `IsApplicableVersionedTypesKey`, shared
/// wherever resolution or reverse module naming asks the same question. A
/// `types@` range is evaluated against the dialect's compiler version.
pub(super) fn package_condition_applies(
    condition: &str,
    conditions: &[String],
    typescript: &Version,
) -> bool {
    condition == "default"
        || conditions.iter().any(|value| value == condition)
        || condition.strip_prefix("types@").is_some_and(|range| {
            conditions.iter().any(|value| value == "types")
                && version::matches_typescript(range, typescript)
        })
}

/// How many `imports` targets may name further lookups in one chain. Real
/// chains are a few lookups; a longer one is reported as unsupported rather
/// than missing. Each lookup recurses, and a full chain needs under 4 MiB of
/// stack in an unoptimized build, a small part of the loading thread's
/// `crate::DEEP_PARSE_STACK_BYTES` (`crate::analysis::on_parsing_thread`).
/// Cycles end earlier, when a lookup repeats.
pub(super) const MAX_IMPORTS_CHAIN: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Resolved {
    /// A TypeScript, declaration, JSON, or JavaScript file, which the loader
    /// parses by its script kind (`crate::script_kind`). `external` marks
    /// the compiler's external-library imports: package lookups and paths
    /// in `node_modules`. Whether the program admits the file for the
    /// importer is a separate question (`admission`).
    File {
        path: String,
        original_path: Option<String>,
        external: bool,
        extension: Extension,
        /// The file's package id where the lookup that found it gives one
        /// (`ResolvedModule.PackageId`).
        package: Option<PackageId>,
    },
    Missing,
    /// A package `imports` chain longer than `MAX_IMPORTS_CHAIN`.
    TooDeep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedPath {
    pub(super) path: String,
    pub(super) original_path: Option<String>,
    pub(super) external: bool,
    /// `ResolvedTypeReferenceDirective.PackageId`.
    pub(super) package: Option<PackageId>,
}

impl ResolvedPath {
    /// Preserve the spelling through which this input was reached. The
    /// loader's canonical identity is independent of native resolution text.
    pub(super) fn reached_path(&self) -> &str {
        self.original_path.as_deref().unwrap_or(&self.path)
    }
}

/// The extension TypeScript records for a resolved module
/// (`ResolvedModule.Extension`), as far as program admission distinguishes
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Extension {
    /// `.ts`, `.mts`, `.cts`, or a declaration form of one.
    TypeScript,
    Tsx,
    Json,
    /// A declaration file for an arbitrary extension, such as
    /// `styles.d.css.ts` for `./styles.css` or `data.d.json.ts` for
    /// `./data.json`: the extension of a file found by adding `.d<ext>.ts` to
    /// a specifier's own extension (`tryAddingExtensions`). The same file
    /// named another way, such as `./styles.d.css`, has the `.ts` extension.
    Arbitrary,
    /// `.js`, `.mjs`, or `.cjs`.
    JavaScript,
    Jsx,
}

impl Extension {
    /// Whether a module found with this extension is JavaScript, which
    /// TypeScript tests as `!resolutionExtensionIsTSOrJson`.
    pub(super) fn is_javascript(self) -> bool {
        matches!(self, Self::JavaScript | Self::Jsx)
    }
}

/// Why the program does not admit a resolved module for an importer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NotAdmitted {
    /// TS6142: a `.tsx` file without `jsx`. The import still denotes the file
    /// when another route makes it a program input.
    Jsx,
    /// TS7042: a JSON file without `resolveJsonModule`.
    ResolveJsonModule,
    /// TS6263: an arbitrary-extension declaration file, imported by a file
    /// that is not a declaration file, without `allowArbitraryExtensions`.
    AllowArbitraryExtensions,
    /// TS7016: JavaScript without `allowJs`, under `noImplicitAny`. The
    /// import names an untyped module (`ModuleResolution::Untyped`).
    AllowJs,
}

impl NotAdmitted {
    /// The code of the compiler's diagnostic.
    pub(super) fn code(self) -> crate::checking::Code {
        use crate::checking::Code;
        match self {
            NotAdmitted::Jsx => Code::JSX_NOT_SET,
            NotAdmitted::ResolveJsonModule => Code::RESOLVE_JSON_MODULE,
            NotAdmitted::AllowArbitraryExtensions => Code::ARBITRARY_EXTENSIONS,
            NotAdmitted::AllowJs => Code::UNTYPED_MODULE,
        }
    }

    /// The compiler's message for `specifier` resolved to `target`.
    pub(super) fn message(self, specifier: &str, target: &str) -> String {
        let option = match self {
            NotAdmitted::Jsx => "'--jsx' is not set",
            NotAdmitted::ResolveJsonModule => "'--resolveJsonModule' is not used",
            NotAdmitted::AllowArbitraryExtensions => "'--allowArbitraryExtensions' is not set",
            NotAdmitted::AllowJs => return untyped_message(specifier, target),
        };
        format!("Module '{specifier}' was resolved to '{target}', but {option}")
    }
}

/// TS7016, which TypeScript's checker reports for an import of an untyped
/// module, a JavaScript file that is no program input
/// (`errorOnImplicitAnyModule`), where `noImplicitAny` makes it an error.
pub(in crate::project) fn untyped_message(specifier: &str, target: &str) -> String {
    format!(
        "Could not find a declaration file for module '{specifier}'. '{target}' implicitly has an 'any' type."
    )
}

/// TS2665, for a module augmentation whose name resolves to an untyped
/// module (`resolveExternalModule` with `isForAugmentation`).
pub(in crate::project) fn untyped_augmentation_message(specifier: &str, target: &str) -> String {
    format!(
        "Invalid module name in augmentation. Module '{specifier}' resolves to an untyped module at '{target}', which cannot be augmented."
    )
}

/// TypeScript's `GetResolutionDiagnostic`: why the program does not admit a
/// module that resolution found with `extension`, for an importer that is
/// a declaration file or not, if it does not. JavaScript needs `allowJs`
/// only under `noImplicitAny` (`needAllowJs`), and a `.jsx` file `jsx`
/// first. It depends on the importer, so it is decided where a resolution
/// is used, never cached with it.
fn resolution_diagnostic(
    options: &Options,
    extension: Extension,
    importer_is_declaration: bool,
) -> Option<NotAdmitted> {
    let need_allow_js =
        || (!options.allow_js && options.semantic.no_implicit_any).then_some(NotAdmitted::AllowJs);
    let need_jsx = || (!options.jsx).then_some(NotAdmitted::Jsx);
    match extension {
        Extension::TypeScript => None,
        Extension::Tsx => need_jsx(),
        Extension::Json => (!options.resolve_json_module).then_some(NotAdmitted::ResolveJsonModule),
        Extension::Arbitrary => (!importer_is_declaration && !options.allow_arbitrary_extensions)
            .then_some(NotAdmitted::AllowArbitraryExtensions),
        Extension::JavaScript => need_allow_js(),
        Extension::Jsx => need_jsx().or_else(need_allow_js),
    }
}

/// What TypeScript's loader makes of a module that resolution found for one
/// importer (`resolveImportsAndModuleAugmentations`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Admission {
    /// `shouldAddFile`: the file is a subtask of the importer, one package
    /// lookup deeper when `external` (`increaseDepth`). `elide_on_depth`
    /// marks JavaScript that a package lookup found in `node_modules`,
    /// which is no input on this route when that depth exceeds
    /// `maxNodeModuleJsDepth` (`elideOnDepth`, `filesParser.start`).
    Input { elide_on_depth: bool },
    /// Not added for this importer: for `GetResolutionDiagnostic`'s reason,
    /// or with none for JavaScript without `allowJs`. TypeScript's checker
    /// still names the file where another route makes it an input and the
    /// reason, if any, is `Jsx` or `AllowJs` (`resolveExternalModule`).
    NotAdded(Option<NotAdmitted>),
}

/// The one decision whether a resolved module is a program input for an
/// importer, a declaration file or not: TypeScript's `shouldAddFile` with
/// `GetResolutionDiagnostic`, less the depth the loader alone tracks. A
/// JavaScript file is an input only under `allowJs`.
pub(super) fn admission(
    options: &Options,
    resolved: (&str, Extension, bool),
    importer_is_declaration: bool,
) -> Admission {
    let (path, extension, external) = resolved;
    if let Some(reason) = resolution_diagnostic(options, extension, importer_is_declaration) {
        return Admission::NotAdded(Some(reason));
    }
    if extension.is_javascript() && !options.allow_js {
        return Admission::NotAdded(None);
    }
    Admission::Input {
        elide_on_depth: external && extension.is_javascript() && path.contains("/node_modules/"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    CommonJs,
    Esm,
}

impl Format {
    fn mode(self) -> ResolutionMode {
        match self {
            Self::Esm => ResolutionMode::Import,
            Self::CommonJs => ResolutionMode::Require,
        }
    }
}

/// A file's module format, which only the resolver interprets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FileFormat {
    /// The Node.js format TypeScript implies for the file.
    implied: Format,
    /// The format fixed by a `.mts`/`.cts` extension or a consulted
    /// `package.json` `type`, which overrides `module` in bundler mode.
    declared: Option<Format>,
}

impl FileFormat {
    /// A standard library's format, which `parseTask.load` decides without a
    /// `package.json` lookup: every library is a CommonJS script.
    pub(super) fn library() -> Self {
        Self {
            implied: Format::CommonJs,
            declared: None,
        }
    }

    /// The format of a file whose extension and package scope declare none,
    /// as `file_format` decides it without a `type` field.
    pub(super) fn undeclared() -> Self {
        Self {
            implied: Format::CommonJs,
            declared: None,
        }
    }

    /// A JSON file's format: `GetImpliedNodeFormatForFile` gives `.json` none
    /// and consults no `package.json`. A reference directive in it resolves
    /// as TypeScript resolves one without a mode: `require` conditions under
    /// Node.js resolution, and bundler's default.
    pub(super) fn json() -> Self {
        Self {
            implied: Format::CommonJs,
            declared: None,
        }
    }
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
    info: Arc<PackageInfo>,
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
    fn from_info(info: Arc<PackageInfo>) -> Self {
        let Some(value) = info.contents.as_ref() else {
            return Self {
                dir: info.directory.clone(),
                info,
                ..Self::default()
            };
        };
        let string = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_owned);
        let exports = value.field("exports");
        let subpaths = exports
            .filter(|value| value.is_exports_subpaths())
            .and_then(Value::entries)
            .map(Table::new);
        Self {
            dir: info.directory.clone(),
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
            info,
        }
    }

    fn version_paths(&self, typescript: &Version) -> Option<Vec<(String, Vec<String>)>> {
        self.info.version_paths(typescript)
    }
}

/// Cached file system queries.
struct Host<'a> {
    disk: &'a disk::Disk,
    case: Case,
    files: HashMap<String, Memo<bool>>,
    directories: HashMap<String, Memo<bool>>,
    packages: HashMap<String, Memo<Option<Arc<PackageJson>>>>,
    /// Session-only receipts are retained even outside an active resolution:
    /// a later lookup may use a package first read for a source's format.
    recording: bool,
    trace: Option<reuse::Trace>,
    /// The first input that lookups could not use: an existing file or
    /// directory that could not be read, or a manifest beyond the JSON
    /// reader's limit. A lookup continues as if it were absent, but loading
    /// then fails rather than return a resolution that silently skipped it.
    failure: Option<InputError>,
}

impl Host<'_> {
    fn unreadable(&mut self, path: &str, error: std::io::Error) {
        self.fail(io_error(Path::new(path), &error));
    }

    fn fail(&mut self, error: InputError) {
        self.failure.get_or_insert(error);
    }

    fn file_exists(&mut self, path: &str) -> bool {
        let cached = self.files.entry(path.to_owned()).or_insert_with(|| {
            Memo::read(path, self.recording, self.disk, || {
                self.disk
                    .file_exists(path)
                    .map_err(|error| io_error(Path::new(path), &error))
            })
        });
        if let Some(trace) = &mut self.trace {
            trace.observe(cached, Access::File);
        }
        let result = cached.answer();
        self.admit(result).unwrap_or(false)
    }

    fn directory_exists(&mut self, path: &str) -> bool {
        let cached = self.directories.entry(path.to_owned()).or_insert_with(|| {
            Memo::read(path, self.recording, self.disk, || {
                self.disk
                    .is_dir(path)
                    .map_err(|error| io_error(Path::new(path), &error))
            })
        });
        if let Some(trace) = &mut self.trace {
            trace.observe(cached, Access::Directory);
        }
        let result = cached.answer();
        self.admit(result).unwrap_or(false)
    }

    /// Resolution continues through absent candidates, then check_reads reports
    /// the first failed observation. Snapshot capture reads the same cached
    /// Result directly so anticipatory failures stay deferred until demanded.
    fn admit<T>(&mut self, result: Result<T, InputError>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.fail(error);
                None
            }
        }
    }

    fn package_observation(&mut self, dir: &str) -> Result<Option<Arc<PackageJson>>, InputError> {
        let dir = paths::directory(dir.to_owned());
        // `InfoCache.Get/Set` uses the canonical absolute path, while
        // getPackageJsonInfo projects PackageDirectory back to this caller.
        let cached = self
            .packages
            .entry(self.case.key(&dir))
            .or_insert_with_key(|key| {
                Memo::read(key, self.recording, self.disk, || {
                    PackageInfo::read(&dir, self.disk)
                        .map(|info| info.map(|info| Arc::new(PackageJson::from_info(info))))
                })
            });
        if let Some(trace) = &mut self.trace {
            trace.observe(cached, Access::Package);
        }
        let package = cached.answer()?;
        Ok(
            package.map(|package| match package.info.with_directory(&dir) {
                std::borrow::Cow::Borrowed(_) => package.clone(),
                std::borrow::Cow::Owned(info) => Arc::new(PackageJson::from_info(Arc::new(info))),
            }),
        )
    }

    fn package(&mut self, dir: &str) -> Option<Arc<PackageJson>> {
        let result = self.package_observation(dir);
        self.admit(result).flatten()
    }

    /// The nearest `package.json` at or above `dir`.
    fn scope(&mut self, dir: &str) -> Option<Arc<PackageJson>> {
        let result = self.scope_observation(dir);
        self.admit(result).flatten()
    }

    fn scope_observation(&mut self, dir: &str) -> Result<Option<Arc<PackageJson>>, InputError> {
        for ancestor in paths::ancestors(dir) {
            if let Some(package) = self.package_observation(ancestor)? {
                return Ok(Some(package));
            }
        }
        Ok(None)
    }
}

/// Per-lookup settings: TypeScript's resolution features and conditions,
/// and the compiler version package ranges are evaluated against.
#[derive(Clone)]
struct State<'v> {
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
    package_directory_only: bool,
    typescript: &'v Version,
    /// Whether the configuration's `moduleSuffixes` apply.
    module_suffixes: bool,
}

impl State<'_> {
    fn with_extensions(&self, extensions: u8) -> Self {
        Self {
            extensions,
            ..self.clone()
        }
    }

    fn applies(&self, condition: &str) -> bool {
        package_condition_applies(condition, &self.conditions, self.typescript)
    }
}

/// The continuation applied to a candidate path by `paths` and
/// `typesVersions` lookups.
#[derive(Clone)]
enum Continuation {
    /// A relative module name, which may name a directory with a manifest.
    Relative,
    /// A file named by a manifest field of this package directory.
    PackageField(Option<Arc<PackageJson>>),
    /// A path inside a package in `node_modules`.
    Package {
        package: Option<Arc<PackageJson>>,
        subpath: bool,
    },
}

/// Where a type reference's name comes from, which decides whether
/// explicit `typeRoots` end its lookup (`fromInferredTypesContainingFile`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum TypeReferenceOrigin {
    /// A `types` entry or an automatic name, which TypeScript resolves from
    /// the configuration's directory as if `__inferred type names__.ts`
    /// there named it.
    Inferred,
    /// A `/// <reference types>` directive in a source file.
    Source,
}

/// What `tryLoadModuleUsingPaths` answers: no pattern matched, so other
/// lookups follow, or the matched pattern's file, or the matched pattern's
/// definitive miss (`Rules::matched_pattern_answers`).
enum Mapped {
    Unmatched,
    Found(Found),
    Missing,
}

/// A file a lookup found, and the manifest of the package whose id the
/// lookup gives it, where it gives one: each site of TypeScript's
/// `withPackageId` / `getPackageId` names the manifest it reads, which
/// `Resolver::package_id` makes the id of once the resolution ends.
#[derive(Debug, Clone)]
struct Found {
    path: String,
    package: Option<Arc<PackageJson>>,
}

impl Found {
    /// A file without a package id (`noPackageId`).
    fn file(path: String) -> Self {
        Self {
            path,
            package: None,
        }
    }

    fn in_package(path: String, package: Option<Arc<PackageJson>>) -> Self {
        Self { path, package }
    }
}

/// A module resolution's question: the importing directory, the specifier,
/// and the resolution mode.
type ModuleKey = (String, String, Option<ResolutionMode>);

/// A lookup named by an `imports` target: the package directory, the name,
/// and the extension families and configuration-lookup flag of its state.
type Hop = (String, String, u8, bool);

pub(super) struct Resolver<'a> {
    options: &'a Options,
    /// The selected configuration's canonical directory and file.
    config_dir: &'a str,
    config_file: &'a str,
    case: Case,
    host: Host<'a>,
    /// Each module resolution of this load, with the failure it met, which
    /// every later request for it meets again (`isolated`).
    modules: HashMap<ModuleKey, (Resolved, Option<InputError>)>,
    /// Results of `imports` hops during one module resolution, so branches
    /// that share a lookup evaluate it once.
    hops: HashMap<Hop, Option<Found>>,
    /// The hops in progress; repeating one ends that route as a cycle.
    active: HashSet<Hop>,
    /// Files one module resolution found by adding an arbitrary extension's
    /// declaration form (`Extension::Arbitrary`).
    arbitrary: HashSet<String>,
    too_deep: bool,
    symlink_resolutions: Vec<(String, String)>,
    reuse: Option<reuse::State<'a>>,
}

impl<'a> Resolver<'a> {
    pub(super) fn new(config: &'a Config, case: Case, disk: &'a disk::Disk) -> Self {
        Self::with(&config.options, &config.dir, &config.file, case, disk)
    }

    fn with(
        options: &'a Options,
        config_dir: &'a str,
        config_file: &'a str,
        case: Case,
        disk: &'a disk::Disk,
    ) -> Self {
        Self {
            options,
            config_dir,
            config_file,
            case,
            host: Host {
                disk,
                case,
                files: HashMap::new(),
                directories: HashMap::new(),
                packages: HashMap::new(),
                recording: false,
                trace: None,
                failure: None,
            },
            modules: HashMap::new(),
            hops: HashMap::new(),
            active: HashSet::new(),
            arbitrary: HashSet::new(),
            too_deep: false,
            symlink_resolutions: Vec::new(),
            reuse: None,
        }
    }

    /// Retain exact resolution inputs for a session refresh. Configuration,
    /// file identity/admission, automatic type discovery, and all binding
    /// remain current-generation work; only module/type lookup decisions are
    /// reused. Call immediately after construction, before any host access.
    pub(super) fn reusing(mut self, previous: &'a Reuse) -> Self {
        self.reuse = Some(reuse::State::new(
            previous,
            self.options,
            self.config_dir,
            self.config_file,
            self.case,
        ));
        self.host.recording = true;
        self
    }

    /// Finish the current generation after all resolution and specifier
    /// capture. The session publishes this only if binding and final exact
    /// input validation also succeed.
    pub(super) fn take_reuse(&mut self) -> Reuse {
        self.reuse
            .take()
            .map(|state| state.next)
            .unwrap_or_default()
    }

    fn begin_trace(&mut self) -> usize {
        if self.reuse.is_some() {
            assert!(self.host.trace.is_none(), "resolution traces do not nest");
            self.host.trace = Some(reuse::Trace::default());
        }
        self.symlink_resolutions.len()
    }

    fn finish_trace<T>(&mut self, value: T, symlinks: usize) -> Option<Arc<reuse::Saved<T>>> {
        let trace = self.host.trace.take()?;
        if self.host.failure.is_some() {
            return None;
        }
        reuse::Saved::new(value, trace, &self.symlink_resolutions[symlinks..])
    }

    /// `GetConditions` preserves native mode-less resolution: Bundler supplies
    /// import conditions, while Node modes supply require conditions. The
    /// separate ES-module feature gate still requires an explicit import mode.
    fn state(&self, mode: Option<ResolutionMode>, extensions: u8) -> State<'a> {
        let options: &'a Options = self.options;
        let node = options.resolution != Resolution::Bundler;
        let conditions =
            resolution_conditions(options.resolution, &options.custom_conditions, mode);
        State {
            extensions,
            esm: node && mode == Some(ResolutionMode::Import),
            exports: options.package_exports,
            imports: options.package_imports,
            imports_root: options.resolution != Resolution::Node16,
            conditions: conditions.into(),
            config_lookup: false,
            package_directory_only: false,
            typescript: options.compiler.dialect().typescript(),
            module_suffixes: true,
        }
    }

    /// Resolves a module specifier written in `file`.
    pub(super) fn module(
        &mut self,
        specifier: &str,
        file: &str,
        mode: Option<ResolutionMode>,
    ) -> Resolved {
        let directory = paths::parent(file).to_owned();
        let key = (directory.clone(), specifier.to_owned(), mode);
        if let Some((resolved, failure)) = self.modules.get(&key) {
            let (resolved, failure) = (resolved.clone(), failure.clone());
            self.relatch(failure.as_ref());
            return resolved;
        }
        if let Some(saved) = self.reuse.as_ref().and_then(|state| state.module(&key))
            && saved.adopt(&mut self.host)
        {
            self.symlink_resolutions.extend_from_slice(&saved.symlinks);
            let result = saved.value.clone();
            self.modules.insert(key.clone(), (result.clone(), None));
            self.reuse
                .as_mut()
                .expect("reuse is enabled")
                .save_module(key, saved);
            #[cfg(test)]
            reuse_tests::finished(self.config_file, specifier);
            return result;
        }
        let (resolved, failure) =
            self.isolated(|resolver| resolver.fresh_module(specifier, &directory, mode, &key));
        self.modules.insert(key, (resolved.clone(), failure));
        #[cfg(test)]
        reuse_tests::finished(self.config_file, specifier);
        resolved
    }

    /// `module` without a reusable answer: the lookup itself, saved for
    /// reuse where it met no failure.
    fn fresh_module(
        &mut self,
        specifier: &str,
        directory: &str,
        mode: Option<ResolutionMode>,
        key: &ModuleKey,
    ) -> Resolved {
        let symlinks = self.begin_trace();
        #[cfg(test)]
        reuse_tests::worked(self.config_file, false);
        let mut extensions = TS | DTS | JS;
        if self.options.resolve_json_module {
            extensions |= JSON;
        }
        let state = self.state(mode, extensions);
        self.hops.clear();
        self.arbitrary.clear();
        self.too_deep = false;
        let found = self.node_like(&state, specifier, directory);
        let resolved = match found {
            _ if self.too_deep => Resolved::TooDeep,
            None => Resolved::Missing,
            Some((found, _)) => {
                let external = paths::has_node_modules(&found.path);
                let resolve_symlink = external && !is_relative_name(specifier);
                let package = self.package_id(&found);
                self.classify(found.path, package, |resolver, path| {
                    resolver.original_and_resolved(path, resolve_symlink, external)
                })
            }
        };
        if self.reuse.is_some()
            && let Some(saved) = self.finish_trace(resolved.clone(), symlinks)
        {
            self.reuse
                .as_mut()
                .expect("reuse is enabled")
                .save_module(key.clone(), saved);
        }
        resolved
    }

    /// Run `lookup` with a failure slot of its own and return what it found
    /// with the failure it alone met, which also stays latched for the
    /// caller (`check_reads`). A memo that answers later requests from one
    /// lookup keeps that failure for them, so a failure belongs to every
    /// request that depends on it, whichever asked first.
    fn isolated<T>(&mut self, lookup: impl FnOnce(&mut Self) -> T) -> (T, Option<InputError>) {
        let outer = self.begin_isolation();
        let value = lookup(self);
        (value, self.end_isolation(outer))
    }

    /// Set aside the failure latched so far, so that what one lookup meets
    /// can be told apart (`isolated`, `end_isolation`).
    pub(super) fn begin_isolation(&mut self) -> Option<InputError> {
        self.host.failure.take()
    }

    /// The failure met since `begin_isolation`, which stays latched for the
    /// caller behind any earlier one.
    pub(super) fn end_isolation(&mut self, outer: Option<InputError>) -> Option<InputError> {
        let failure = self.host.failure.clone();
        if outer.is_some() {
            self.host.failure = outer;
        }
        failure
    }

    /// Latch a failure a memoized lookup met (`isolated`) for the request
    /// it now answers.
    pub(super) fn relatch(&mut self, failure: Option<&InputError>) {
        if let Some(failure) = failure {
            self.host.fail(failure.clone());
        }
    }

    /// TypeScript 5.6.3's `resolveLibrary`, which resolves a standard
    /// library's replacement package, written in `from`, with the options of
    /// `getOptionsForLibraryResolution` alone, `{ moduleResolution: Node10 }`:
    /// ancestor `node_modules` and their `@types`, TypeScript and
    /// declarations before JavaScript, without package `exports`, `imports`,
    /// or self-names (Node10 resolves with no features), without JSON, and
    /// without the configuration's `paths`, `baseUrl`, `rootDirs`,
    /// `typeRoots`, `moduleSuffixes`, conditions, or `preserveSymlinks`, so
    /// a symbolic link always resolves to its target.
    pub(super) fn library(&mut self, package: &str, from: &str) -> Resolved {
        let state = State {
            extensions: TS | DTS | JS,
            esm: false,
            exports: false,
            imports: false,
            imports_root: false,
            conditions: Vec::new().into(),
            config_lookup: false,
            package_directory_only: false,
            module_suffixes: false,
            typescript: self.options.compiler.dialect().typescript(),
        };
        self.hops.clear();
        self.arbitrary.clear();
        self.too_deep = false;
        match self.node_modules(&state, package, paths::parent(from)) {
            // Unreachable while `imports` is off, but a limit stays a failure.
            _ if self.too_deep => Resolved::TooDeep,
            None => Resolved::Missing,
            // `pathForLibFile` keeps only the file: a standard library is
            // never deduplicated as a package.
            Some(found) => {
                let external = paths::has_node_modules(&found.path);
                self.classify(found.path, None, |resolver, found| {
                    resolver.real_path(found, external)
                })
            }
        }
    }

    /// A module lookup's result for the file it `found`, at the path `reach`
    /// resolves it to, with the extension TypeScript records for it
    /// (`Extension`), which the found spelling decides.
    fn classify(
        &mut self,
        found: String,
        package: Option<PackageId>,
        reach: impl FnOnce(&mut Self, String) -> ResolvedPath,
    ) -> Resolved {
        let extension = if found.ends_with(".jsx") {
            Extension::Jsx
        } else if input::is_javascript(&found) {
            Extension::JavaScript
        } else if self.arbitrary.contains(&found) {
            Extension::Arbitrary
        } else if found.ends_with(".tsx") {
            Extension::Tsx
        } else if found.ends_with(".json") {
            Extension::Json
        } else {
            Extension::TypeScript
        };
        let resolved = reach(self, found);
        Resolved::File {
            path: resolved.path,
            original_path: resolved.original_path,
            external: resolved.external,
            extension,
            package,
        }
    }

    /// Resolves a type library named by `types` or `/// <reference types>`
    /// (`resolveTypeReferenceDirective`): type roots first, then
    /// `node_modules` above `directory`, unless `typeRoots` is explicit and
    /// the configuration inferred the name. The origin is part of the
    /// lookup's identity, as in TypeScript 7's cache.
    pub(super) fn type_reference(
        &mut self,
        name: &str,
        directory: &str,
        mode: Option<ResolutionMode>,
        origin: TypeReferenceOrigin,
    ) -> Option<ResolvedPath> {
        if self.reuse.is_none() {
            return self.type_reference_worker(name, directory, mode, origin);
        }
        let key = (directory.to_owned(), name.to_owned(), mode, origin);
        if let Some(saved) = self.reuse.as_ref().and_then(|state| state.types(&key))
            && saved.adopt(&mut self.host)
        {
            // Unlike module(), type_reference() has no per-load result memo:
            // every invocation contributes its original symlink effect.
            self.symlink_resolutions.extend_from_slice(&saved.symlinks);
            let result = saved.value.clone();
            self.reuse
                .as_mut()
                .expect("reuse is enabled")
                .save_types(key, saved);
            return result;
        }
        let symlinks = self.begin_trace();
        let resolved = self.type_reference_worker(name, directory, mode, origin);
        if let Some(saved) = self.finish_trace(resolved.clone(), symlinks) {
            self.reuse
                .as_mut()
                .expect("reuse is enabled")
                .save_types(key, saved);
        }
        resolved
    }

    fn type_reference_worker(
        &mut self,
        name: &str,
        directory: &str,
        mode: Option<ResolutionMode>,
        origin: TypeReferenceOrigin,
    ) -> Option<ResolvedPath> {
        #[cfg(test)]
        reuse_tests::worked(self.config_file, true);
        let state = self.state(mode, DTS);
        // Explicit roots resolve a name as a file or a directory, just as
        // modules resolve; the default `node_modules/@types` roots hold
        // packages only.
        let explicit = self.options.type_roots.is_some();
        for root in self.type_roots() {
            if !self.host.directory_exists(&root) {
                continue;
            }
            let candidate = type_root_candidate(&root, name);
            let found = if explicit {
                self.load_from_file(&state, &candidate)
                    .map(|path| self.in_containing_package(path))
            } else {
                None
            };
            let found = found.or_else(|| self.load_from_directory(&state, &candidate, true));
            if let Some(found) = found.filter(|found| input::is_typescript(&found.path)) {
                return Some(self.type_reference_file(found));
            }
        }
        if explicit && origin == TypeReferenceOrigin::Inferred {
            return None;
        }
        let found = if is_relative_name(name) {
            self.load_relative(&state, &relative_candidate(directory, name), true)
        } else {
            self.node_modules(&state, name, directory)
        };
        found
            .filter(|found| input::is_typescript(&found.path))
            .map(|found| self.type_reference_file(found))
    }

    /// `createResolvedTypeReferenceDirective`: the file with its package id
    /// and, unless `preserveSymlinks` is set, through its symbolic links.
    fn type_reference_file(&mut self, found: Found) -> ResolvedPath {
        let package = self.package_id(&found);
        let external = paths::has_node_modules(&found.path);
        ResolvedPath {
            package,
            ..self.original_and_resolved(found.path, true, external)
        }
    }

    /// A file a lookup found by name, which takes the id of the package
    /// directory its path is in, if any: `withPackageId` of the manifest
    /// `parseNodeModuleFromPath` names.
    fn in_containing_package(&mut self, path: String) -> Found {
        let package =
            package_id::package_directory(&path).and_then(|directory| self.host.package(directory));
        Found::in_package(path, package)
    }

    /// `withPackageId` / `getPackageId`: the id of a file a lookup found in
    /// a package, from the manifest's `name` and `version` as the dialect
    /// reads them (`package_id::header`), the file's path in the package
    /// (`package_id::sub_module`), and the
    /// package's installed peers (`peers`); none for a file found without a
    /// package (`Found`).
    fn package_id(&mut self, found: &Found) -> Option<PackageId> {
        let package = found.package.as_ref()?;
        let rules = self.rules();
        let (name, version) = package_id::header(rules, &package.info)?;
        let sub_module = package_id::sub_module(rules, &found.path, &package.dir);
        let sub_module = self.host.admit(sub_module)?;
        let peers = self.peers(package);
        Some(PackageId::new(rules, name, sub_module, version, peers))
    }

    /// `readPackageJsonPeerDependencies`: `+name@version` for each of the
    /// package's peers (`package_id::peer_names`) whose manifest is in the
    /// `node_modules` beside the package's real directory
    /// (`package_id::peer_directory`), whatever `preserveSymlinks` says. The
    /// real path and each peer's manifest, present or absent, are inputs of
    /// the resolution.
    fn peers(&mut self, package: &PackageJson) -> String {
        let rules = self.rules();
        let Some(names) = package_id::peer_names(rules, &package.info) else {
            return String::new();
        };
        let real = self.canonical(&package.dir);
        let Some(real) = self.host.admit(real) else {
            return String::new();
        };
        let directory = package_id::peer_directory(rules, &real);
        let Some(Some(directory)) = self.host.admit(directory) else {
            return String::new();
        };
        let mut suffix = String::new();
        for name in names {
            let Some(peer) = self.host.package(&format!("{directory}{name}")) else {
                continue;
            };
            let version = package_id::peer_version(rules, &peer.info);
            if let Some(version) = self.host.admit(version) {
                suffix.push_str(&format!("+{name}@{version}"));
            }
        }
        suffix
    }

    /// `getOriginalAndResolvedFileName`, called at the native module/type
    /// reference gates. A case-only realpath difference keeps the original
    /// spelling; other failures remain active resolution input failures.
    fn original_and_resolved(
        &mut self,
        path: String,
        resolve_symlink: bool,
        external: bool,
    ) -> ResolvedPath {
        if !resolve_symlink || self.options.preserve_symlinks {
            return ResolvedPath {
                path,
                original_path: None,
                external,
                package: None,
            };
        }
        self.real_path(path, external)
    }

    /// `path` through its symbolic links, the original spelling kept beside
    /// it when they lead elsewhere, as `getOriginalAndResolvedFileName`
    /// resolves a file wherever a lookup resolves symbolic links.
    fn real_path(&mut self, path: String, external: bool) -> ResolvedPath {
        let real = match self.canonical(&path) {
            Ok(real) => real,
            Err(error) => {
                self.host.fail(error);
                return ResolvedPath {
                    path,
                    original_path: None,
                    external,
                    package: None,
                };
            }
        };
        if paths::compare(&path, &real, self.config_dir, self.case).is_eq() {
            ResolvedPath {
                path,
                original_path: None,
                external,
                package: None,
            }
        } else {
            self.symlink_resolutions.push((path.clone(), real.clone()));
            ResolvedPath {
                path: real,
                original_path: Some(path),
                external,
                package: None,
            }
        }
    }

    /// `realpath` of `path` through `Disk`, an input of the resolution in
    /// progress.
    fn canonical(&mut self, path: &str) -> Result<String, InputError> {
        let real = if let Some(trace) = &mut self.host.trace {
            let (result, receipt) = self.host.disk.capture(|| self.host.disk.canonicalize(path));
            trace.canonical(receipt);
            result
        } else {
            self.host.disk.canonicalize(path)
        };
        real.map_err(|error| io_error(Path::new(path), &error))
            .and_then(|path| paths::from_path(&path))
    }

    pub(super) fn symlink_resolutions(&self) -> &[(String, String)] {
        &self.symlink_resolutions
    }

    /// Typed snapshot acquisition shares the resolver's parsed package owner
    /// without latching an anticipatory failure as an active load failure.
    pub(super) fn package_observation(
        &mut self,
        directory: &str,
    ) -> Result<Option<Arc<PackageInfo>>, InputError> {
        self.host
            .package_observation(directory)
            .map(|package| package.map(|package| package.info.clone()))
    }

    /// `loadSourceFileMetaData` asks for package scope independently of the
    /// extension and of the narrower package-type-to-module-format gate.
    pub(super) fn source_package_scope(
        &mut self,
        file: &str,
    ) -> Result<Option<Arc<PackageInfo>>, InputError> {
        self.host
            .scope_observation(paths::parent(file))
            .map(|package| package.map(|package| package.info.clone()))
    }

    /// `ResolvePackageDirectory` shares nearest-node_modules and @types
    /// traversal with resolution, but stops at an existing package directory.
    pub(super) fn package_directory(
        &mut self,
        name: &str,
        from: &str,
    ) -> Result<Option<ResolvedPath>, InputError> {
        let mut state = self.state(Some(ResolutionMode::Require), TS | DTS | JS);
        state.package_directory_only = true;
        let result = self
            .node_modules(&state, name, paths::parent(from))
            .map(|found| {
                let external = paths::has_node_modules(&found.path);
                self.original_and_resolved(
                    found.path,
                    external && !is_relative_name(name),
                    external,
                )
            });
        self.check_reads()?;
        Ok(result)
    }

    /// The package names `"types": ["*"]` selects: each type root's
    /// subdirectories, except dot-names and `"typings": null` stubs.
    pub(super) fn automatic_types(&mut self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for root in self.type_roots() {
            let entries = match self.host.disk.read_dir(&root) {
                Ok(entries) => entries,
                Err(error) if disk::is_absence(&error) => continue,
                Err(error) => {
                    self.host.unreadable(&root, error);
                    continue;
                }
            };
            let mut directories: Vec<String> = Vec::new();
            for entry in entries {
                match self.host.disk.entry(&entry.path()) {
                    Ok(Some(disk::Entry::Directory)) => {
                        match paths::exact_name(entry.file_name()) {
                            Ok(name) => directories.push(name),
                            Err(lossy) => self.host.fail(paths::inexact_name(&root, &lossy)),
                        }
                    }
                    Ok(_) => {}
                    Err(error) => self.host.fail(io_error(&entry.path(), &error)),
                }
            }
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
            None => paths::ancestors(self.config_dir)
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
            _ => FileFormat::undeclared(),
        }
    }

    /// `GetImpliedNodeFormatForEmitWorker` / `GetEmitModuleFormatOfFileWorker`
    /// use the same captured extension/package facts as module resolution.
    /// JSON has no implied node format; resolution's CommonJS default does not
    /// turn that absence into an emit-format fact.
    pub(super) fn module_semantics(
        &self,
        format: FileFormat,
        json: bool,
    ) -> super::FileModuleSemantics {
        use super::ModuleKind;
        let configured = self.options.semantic.module_kind;
        let implied = if json {
            None
        } else if configured.is_node() {
            Some(format.implied)
        } else {
            format.declared
        };
        // `importSyntaxAffectsModuleResolution` and
        // `getDefaultResolutionModeForFile` share the captured format facts.
        let import_syntax_affects_resolution = matches!(
            self.options.resolution,
            Resolution::Node16 | Resolution::NodeNext
        ) || self.options.package_exports
            || self.options.package_imports;
        super::FileModuleSemantics {
            implied_node_format: implied.map(Format::mode),
            default_resolution_mode: import_syntax_affects_resolution
                .then_some(implied)
                .flatten()
                .map(Format::mode),
            import_syntax_affects_resolution,
            emit_module_kind: match implied {
                Some(Format::CommonJs) => ModuleKind::CommonJs,
                Some(Format::Esm) => ModuleKind::EsNext,
                None => configured,
            },
        }
    }

    /// Fail if a lookup met a file or directory it could not read.
    pub(super) fn check_reads(&mut self) -> Result<(), InputError> {
        match self.host.failure.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// The mode of a `/// <reference types>` directive without a
    /// `resolution-mode` attribute: the containing file's format decides.
    pub(super) fn type_directive_mode(&self, format: FileFormat) -> Option<ResolutionMode> {
        match self.options.resolution {
            Resolution::Bundler => format.declared,
            Resolution::Node16 | Resolution::NodeNext => Some(format.implied),
        }
        .map(Format::mode)
    }

    /// TypeScript's `isFileForcedToBeModuleByFormat`, which decides module
    /// semantics under `moduleDetection: auto`.
    pub(super) fn format_forces_module(&self, path: &str, format: FileFormat) -> bool {
        paths::has_extension(path, &[".mts", ".cts", ".mjs", ".cjs"])
            || match self.options.resolution {
                Resolution::Node16 | Resolution::NodeNext => format.implied == Format::Esm,
                Resolution::Bundler => format.declared == Some(Format::Esm),
            }
    }

    /// The located file and whether it came from a `node_modules` lookup.
    fn node_like(&mut self, state: &State, name: &str, dir: &str) -> Option<(Found, bool)> {
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

    /// `tryLoadModuleUsingOptionalResolutionSettings`: `paths` for a name
    /// that does not start with `./` or `../`; unless its matched pattern
    /// answered, `baseUrl` for a non-relative name
    /// (`tryLoadModuleUsingBaseUrl`) or `rootDirs` for a relative or rooted
    /// one. The candidate under `baseUrl` loads as a relative name does, a
    /// directory's manifest included.
    fn optional_settings(&mut self, state: &State, name: &str, dir: &str) -> Option<Found> {
        let options = self.options;
        if let Some(mapping) = &options.paths
            && !paths::is_relative(name)
        {
            match self.paths_lookup(
                state,
                name,
                &mapping.base,
                &mapping.patterns,
                &Continuation::Relative,
            ) {
                Mapped::Found(found) => return Some(found),
                Mapped::Missing => return None,
                Mapped::Unmatched => {}
            }
        }
        if is_relative_name(name) {
            return self.root_dirs(state, name, dir);
        }
        let base_url = options.base_url.as_deref()?;
        self.load_relative(state, &paths::resolve(base_url, name), true)
    }

    /// `tryLoadModuleUsingPaths`: an exact pattern, else the matching `*`
    /// pattern with the longest prefix; each substitution is tried in order.
    fn paths_lookup(
        &mut self,
        state: &State,
        name: &str,
        base: &str,
        patterns: &[(String, Vec<String>)],
        then: &Continuation,
    ) -> Mapped {
        let Some((index, star)) = best_pattern(patterns, name) else {
            return Mapped::Unmatched;
        };
        for substitution in &patterns[index].1 {
            let path = self.rules().substitution(substitution, star.as_deref());
            let candidate = paths::resolve(base, &path);
            // A file a substitution names with its extension has no package
            // id in either compiler.
            if paths::known_extension(substitution).is_some() {
                if let Some(found) = self.try_file(state, &candidate) {
                    return Mapped::Found(Found::file(found));
                }
            }
            if let Some(found) = self.load(then, state, &candidate) {
                return Mapped::Found(found);
            }
        }
        if self.rules().matched_pattern_answers() {
            Mapped::Missing
        } else {
            Mapped::Unmatched
        }
    }

    fn rules(&self) -> Rules {
        self.options.compiler.dialect().resolution()
    }

    /// A relative name under the longest matching root directory, tried in
    /// place and then under each other root directory.
    fn root_dirs(&mut self, state: &State, name: &str, dir: &str) -> Option<Found> {
        let options = self.options;
        if options.root_dirs.is_empty() {
            return None;
        }
        let candidate = paths::resolve(dir, name);
        let mut matched: Option<(&String, String)> = None;
        for root in &options.root_dirs {
            // The separator keeps `/src` from matching `/src-other`.
            let mut prefix = root.clone();
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
            let other = paths::combine(root, suffix);
            if let Some(found) = self.load_relative(state, &other, true) {
                return Some(found);
            }
        }
        None
    }

    fn load(&mut self, then: &Continuation, state: &State, candidate: &str) -> Option<Found> {
        match then {
            Continuation::Relative => self.load_relative(state, candidate, true),
            // A manifest field's file has no package id of its own: the
            // lookup that read the manifest decides.
            Continuation::PackageField(package) => {
                if let Some(found) = self.package_field(state, candidate) {
                    return Some(Found::file(found));
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
            // The package's id, whichever way its file is found.
            Continuation::Package { package, subpath } => {
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
                    .map(|path| Found::in_package(path, package.clone()))
            }
        }
    }

    /// `nodeLoadModuleByRelativeName`. Reading `manifest`s, a file found in
    /// a package takes the package's id, and one found through a directory
    /// the directory's where the dialect gives it one
    /// (`load_from_directory`).
    fn load_relative(&mut self, state: &State, candidate: &str, manifest: bool) -> Option<Found> {
        if !candidate.ends_with('/') {
            if let Some(found) = self.load_from_file(state, candidate) {
                return Some(if manifest {
                    self.in_containing_package(found)
                } else {
                    Found::file(found)
                });
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
        // The declaration form of the written extension, `.d.json.ts` or
        // `.d<ext>.ts` for an extension TypeScript does not know.
        let mut arbitrary = None;
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
            // An arbitrary-extension declaration first, as for `.css`.
            ".json" => {
                if has(DTS) {
                    arbitrary = Some(".d.json.ts".to_owned());
                }
                push(has(DTS), &[".d.json.ts"]);
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
                if has(DTS) && !declaration {
                    arbitrary = Some(format!(".d{other}.ts"));
                }
                push(has(DTS) && !declaration, &[&format!(".d{other}.ts")]);
            }
        }
        order.into_iter().find_map(|extension| {
            let found = self.try_file(state, &format!("{stem}{extension}"))?;
            if arbitrary.as_ref() == Some(&extension) {
                self.arbitrary.insert(found.clone());
            }
            Some(found)
        })
    }

    /// An existing file, honoring `moduleSuffixes` before the extension
    /// where they apply.
    fn try_file(&mut self, state: &State, path: &str) -> Option<String> {
        let options = self.options;
        if !state.module_suffixes || options.module_suffixes.is_empty() {
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

    /// `loadNodeModuleFromDirectory`: a directory's manifest entry or index,
    /// with the directory's package id where `Rules::directory_package_id`
    /// gives one.
    fn load_from_directory(
        &mut self,
        state: &State,
        candidate: &str,
        manifest: bool,
    ) -> Option<Found> {
        let package = if manifest {
            self.host.package(candidate)
        } else {
            None
        };
        let found = self.directory(state, candidate, package.clone())?;
        let package = package.filter(|_| self.rules().directory_package_id());
        Some(Found::in_package(found, package))
    }

    /// A directory's manifest entry (`typesVersions`, `typings`/`types`,
    /// `main`, or `tsconfig` for configurations), then its index file.
    fn directory(
        &mut self,
        state: &State,
        candidate: &str,
        package: Option<Arc<PackageJson>>,
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
        let then = Continuation::PackageField(package.clone());
        let version_paths = package
            .as_ref()
            .and_then(|p| p.version_paths(state.typescript));
        if let Some(version_paths) = version_paths {
            // TypeScript's `containsPath` here compares case-sensitively.
            if package_file
                .as_ref()
                .is_none_or(|file| Case::Sensitive.contains(candidate, file))
            {
                let target = package_file.as_deref().unwrap_or(&index);
                let name = paths::relative(candidate, target);
                match self.paths_lookup(state, &name, candidate, &version_paths, &then) {
                    Mapped::Found(found) => return Some(found.path),
                    Mapped::Missing => return None,
                    Mapped::Unmatched => {}
                }
            }
        }
        if let Some(file) = &package_file {
            if let Some(found) = self.load(&then, state, file) {
                return Some(found.path);
            }
        }
        self.load_from_file(state, &index)
    }

    /// A path named by a manifest or an export target: exact for TypeScript
    /// extensions, otherwise with its extension replaced.
    fn package_field(&mut self, state: &State, candidate: &str) -> Option<String> {
        if state.extensions & TS != 0 && input::is_typescript(candidate)
            || state.extensions & DTS != 0
                && paths::has_extension(candidate, &[".d.ts", ".d.mts", ".d.cts"])
            || state.config_lookup && state.extensions == JSON && candidate.ends_with(".json")
        {
            return self.try_file(state, candidate);
        }
        self.no_implicit_extensions(state, candidate)
    }

    /// Searches each ancestor's `node_modules` (and `@types`) for TypeScript
    /// and declarations, then again for other extensions.
    fn node_modules(&mut self, state: &State, name: &str, dir: &str) -> Option<Found> {
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
    fn package_lookup(&mut self, state: &State, name: &str, modules: &str) -> Option<Found> {
        let candidate = paths::resolve(modules, name);
        let (package_name, rest) = split_package_name(name);
        let package_dir = paths::combine(modules, package_name);
        if state.package_directory_only {
            return self
                .host
                .directory_exists(&package_dir)
                .then(|| Found::file(package_dir));
        }
        let nested = self.host.package(&candidate);
        let mut root = None;
        if !rest.is_empty() && nested.is_some() {
            let root_has_exports = state.exports && {
                let package = self.host.package(&package_dir);
                let has = package.as_ref().is_some_and(|p| p.has_exports);
                root = Some(package);
                has
            };
            // A nested manifest's file has no package id; its directory's
            // entry has the nested manifest's.
            if !root_has_exports {
                if let Some(found) = self.load_from_file(state, &candidate) {
                    return Some(Found::file(found));
                }
                return self
                    .directory(state, &candidate, nested.clone())
                    .map(|found| Found::in_package(found, nested));
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
        let then = Continuation::Package {
            package: package.clone(),
            subpath: !rest.is_empty(),
        };
        if !rest.is_empty() {
            let version_paths = package
                .as_ref()
                .and_then(|p| p.version_paths(state.typescript));
            if let Some(version_paths) = version_paths {
                match self.paths_lookup(state, rest, &package_dir, &version_paths, &then) {
                    Mapped::Found(found) => return Some(found),
                    Mapped::Missing => return None,
                    Mapped::Unmatched => {}
                }
            }
        }
        self.load(&then, state, &candidate)
    }

    /// `Some(None)` is a definitive failure; `None` means no export entry
    /// applied, so other lookups may continue.
    fn exports(
        &mut self,
        state: &State,
        scope: &Arc<PackageJson>,
        subpath: &str,
    ) -> Option<Option<Found>> {
        let Some(exports) = scope.exports.as_ref() else {
            return Some(None);
        };
        if subpath == "." {
            let main = match &exports.kind {
                Kind::String(_) | Kind::Array(_) => Some(exports),
                Kind::Object(_) if exports.is_exports_conditions() => Some(exports),
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
        scope: &Arc<PackageJson>,
        name: &str,
        table: &Table,
        imports: bool,
    ) -> Option<Option<Found>> {
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
        scope: &Arc<PackageJson>,
        target: &Value,
        subpath: &str,
        pattern: bool,
        imports: bool,
    ) -> Option<Option<Found>> {
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

    /// A string target's file, which takes the id of the package whose
    /// `exports` or `imports` named it, or the id its own lookup gives a
    /// file another package's `imports` target names.
    fn string_target(
        &mut self,
        state: &State,
        scope: &Arc<PackageJson>,
        target: &str,
        subpath: &str,
        pattern: bool,
        imports: bool,
    ) -> Option<Found> {
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
            return found.map(|found| Found::in_package(found, Some(scope.clone())));
        }
        self.package_field(state, &final_path)
            .map(|found| Found::in_package(found, Some(scope.clone())))
    }

    /// Looks up a name an `imports` target gives, at most once per module
    /// resolution. A lookup already in progress is a cycle and fails that
    /// route; the compiler itself overflows its stack on one.
    fn hop(&mut self, state: &State, dir: &str, name: String) -> Option<Found> {
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
        // As in TypeScript's `tryLoadInputFileForPath`, containment follows
        // the file system's case policy.
        if state.config_lookup
            || path.contains("/node_modules/")
            || !self.case.contains(&scope.dir, self.config_file)
        {
            return None;
        }
        let common = match (&options.root_dir, options.composite) {
            (Some(root), _) => root.clone(),
            (None, true) => self.config_dir.to_owned(),
            (None, false) => return None,
        };
        let mut outputs: Vec<&String> = options.declaration_dir.iter().collect();
        if let Some(out_dir) = &options.out_dir {
            if options.declaration_dir.as_ref() != Some(out_dir) {
                outputs.push(out_dir);
            }
        }
        for output in outputs {
            if !self.case.contains(output, path) || output.len() >= path.len() {
                continue;
            }
            let rest = &path[output.len()..];
            let base = paths::combine(&common, rest.strip_prefix('/').unwrap_or(rest));
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
    fn imports_lookup(&mut self, state: &State, name: &str, dir: &str) -> Option<Found> {
        if name == "#" || name.starts_with("#/") && !state.imports_root {
            return None;
        }
        let scope = self.host.scope(dir)?;
        let table = scope.imports.as_ref()?;
        self.lookup_table(state, &scope, name, table, true)
            .flatten()
    }

    /// A package importing itself by name through its own `exports`.
    fn self_name_lookup(&mut self, state: &State, name: &str, dir: &str) -> Option<Option<Found>> {
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
    fn type_root_lookup(&mut self, state: &State, name: &str) -> Option<Found> {
        let options = self.options;
        let roots = options.type_roots.as_ref()?;
        let state = state.with_extensions(DTS);
        for root in roots {
            let candidate = type_root_candidate(root, name);
            let found = self
                .load_from_file(&state, &candidate)
                .map(|found| self.in_containing_package(found))
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
/// package's `tsconfig.json`, by the selected compiler's resolution rules
/// and version, which a package's `typesVersions` may consult.
pub(super) fn config_package(
    name: &str,
    directory: &str,
    disk: &disk::Disk,
    compiler: &CompilerSelection,
) -> Result<Option<String>, InputError> {
    let options = Options::for_extends_lookup(compiler.clone());
    // A configuration lookup consults neither the configuration's location
    // nor output directories.
    let mut resolver = Resolver::with(&options, "", "", Case::Sensitive, disk);
    let mut state = resolver.state(Some(ResolutionMode::Require), JSON);
    state.config_lookup = true;
    let found = resolver
        .node_like(&state, name, directory)
        .map(|(found, _)| found.path);
    resolver.check_reads()?;
    Ok(found)
}

/// `./x`, `../x`, `.`, `..`, or a rooted path: TypeScript's
/// `isExternalModuleNameRelative`. Other names may match ambient modules.
pub(in crate::project) fn is_relative_name(name: &str) -> bool {
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

/// Native `GetTypesPackageName`, preserving the resolver's scoped-package
/// mangling owner for symlink-cache runtime dependency probes too.
pub(super) fn types_package_name(name: &str) -> String {
    format!("@types/{}", mangle_scoped(name))
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

/// TypeScript's `matchPatternOrExact`: an exact key, else the best pattern
/// (`pattern::best_match`), with the text matched by `*`.
pub(super) fn best_pattern(
    patterns: &[(String, Vec<String>)],
    name: &str,
) -> Option<(usize, Option<String>)> {
    if let Some(index) = patterns
        .iter()
        .position(|(key, _)| !key.contains('*') && key == name)
    {
        return Some((index, None));
    }
    let keys = patterns
        .iter()
        .enumerate()
        .map(|(index, (key, _))| (key.as_str(), index));
    let (index, star) = super::super::pattern::best_match(keys, name)?;
    Some((index, Some(star.to_owned())))
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
