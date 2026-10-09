//! The effective configuration of one `tsconfig.json`: ordered `extends`
//! inheritance, option validation, and path-valued settings resolved against
//! the configuration file that declared them.
//!
//! Files are read with the shared TypeScript JSON reader
//! (`crate::parser::json`); this module owns TypeScript's conversion of that
//! text to configuration, in three steps that ask different questions:
//!
//! - Raw conversion (`convertConfigFileToObject`, `convertToJson`, and
//!   `convertObjectLiteralExpressionToJson`): the root must be an object, a
//!   property with an empty name is left out, and a repeated key keeps its
//!   first position and its last value, `null` included (`entries`, `get`).
//! - A file's own option settings (`parseOwnConfigOfJsonSourceFile`'s
//!   `onPropertySet`): every occurrence of a recognized option, across every
//!   `compilerOptions` object, is validated, and the last one whose converted
//!   value is not nil sets the option. A `null` or an empty enumeration
//!   string converts to nil, so it leaves an earlier setting; a list's `null`
//!   converts to a nil list, which clears it (`Kind::sets`, `own_setting`).
//! - Inheritance (`parseConfig`'s `applyExtendedConfig` and
//!   `mergeCompilerOptions`): `files`, `include`, and `exclude` come from a
//!   file's raw object when it has the key at all, `null` included, and
//!   otherwise from the last base whose value is a list
//!   (`Chain::root_list`). A file with `extends` merges each base's merged
//!   options in order, then its own: a set value that is not the zero value
//!   replaces the one before (`Kind::zero`), and the merged file's explicit
//!   `null`, the last value in its last `compilerOptions` object, unsets the
//!   option (`Chain::option`).

use super::libraries::{self, Target};
use super::paths;
use super::resolve;
use super::{Origin, disk, io_error};
use crate::analysis::Parser;
use crate::analysis::{InputError, InputErrorKind};
use crate::parser::json::{JsonErrorKind, JsonKind, JsonProperty, JsonString, JsonValue};
use crate::source::SourceText;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug)]
pub(super) struct Config {
    /// Absolute path of the selected configuration, not canonicalized.
    pub(super) path: PathBuf,
    /// Canonical directory of the selected configuration.
    pub(super) dir: String,
    /// The selected configuration in its canonical directory.
    pub(super) file: String,
    pub(super) files: Option<Specs>,
    pub(super) include: Option<Specs>,
    pub(super) exclude: Option<Specs>,
    /// An empty selection is TypeScript's "No inputs were found" error only
    /// when neither `files` nor `references` is specified.
    pub(super) report_no_inputs: bool,
    pub(super) options: Options,
}

/// The entries of `files`, `include`, or `exclude`, made absolute.
#[derive(Debug)]
pub(super) struct Specs {
    pub(super) entries: Vec<Spec>,
}

#[derive(Debug)]
pub(super) struct Spec {
    /// As written, for messages.
    pub(super) text: String,
    /// Absolute and normalized.
    pub(super) path: String,
    pub(super) origin: Origin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Module {
    CommonJs,
    /// `es2015` through `esnext`.
    Es,
    /// `node16`, `node18`, `node20`, or `nodenext`.
    Node,
    Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Resolution {
    Node16,
    NodeNext,
    Bundler,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Detection {
    Auto,
    Legacy,
    Force,
}

/// The settings that affect program inputs, module edges, or file binding.
/// Directory-valued settings (`rootDirs`, `typeRoots`, `outDir`,
/// `declarationDir`, `rootDir`) are absolute and normalized, without a
/// trailing separator except at a root.
#[derive(Debug)]
pub(super) struct Options {
    pub(super) module: Module,
    pub(super) resolution: Resolution,
    pub(super) detection: Detection,
    /// `jsx` is set, to any mode: a `.tsx` file may then be a program input
    /// that a module request reaches (`GetResolutionDiagnostic`).
    pub(super) jsx: bool,
    /// `jsx: react-jsx` or `react-jsxdev`.
    pub(super) jsx_automatic: bool,
    /// `jsx: react-jsxdev`, whose runtime module is `jsx-dev-runtime`.
    pub(super) jsx_dev: bool,
    /// `jsxImportSource`, the package providing an automatic JSX runtime.
    pub(super) jsx_import_source: Option<String>,
    pub(super) paths: Option<Paths>,
    pub(super) root_dirs: Vec<String>,
    /// Explicit `typeRoots`; `None` means every ancestor `node_modules/@types`.
    pub(super) type_roots: Option<Vec<String>>,
    pub(super) types: Vec<(String, Origin)>,
    pub(super) custom_conditions: Vec<String>,
    pub(super) module_suffixes: Vec<String>,
    pub(super) preserve_symlinks: bool,
    /// `noUncheckedSideEffectImports`: an unresolved side-effect import is an
    /// error unless this is false.
    pub(super) check_side_effect_imports: bool,
    /// `skipLibCheck`: TypeScript reports no errors in declaration files.
    pub(super) skip_lib_check: bool,
    /// `noCheck`: TypeScript reports no binding or checker errors at all.
    pub(super) no_check: bool,
    pub(super) allow_js: bool,
    /// `allowArbitraryExtensions`: a module request from a file that is not a
    /// declaration file may reach a declaration file for an arbitrary
    /// extension, such as `styles.d.css.ts` (`GetResolutionDiagnostic`).
    pub(super) allow_arbitrary_extensions: bool,
    pub(super) resolve_json_module: bool,
    pub(super) package_exports: bool,
    pub(super) package_imports: bool,
    /// `target`, which selects the default standard library.
    pub(super) target: Target,
    /// The files `lib` names, deduplicated by file and in written order;
    /// `None` when unset, so the target's default applies, and empty for an
    /// empty list, which selects no library.
    pub(super) lib: Option<Vec<&'static str>>,
    /// `noLib`: no standard library, and every library reference ignored.
    pub(super) no_lib: bool,
    /// `libReplacement`: `@typescript/lib-*` packages may replace libraries.
    pub(super) lib_replacement: bool,
    /// `skipDefaultLibCheck`: TypeScript reports no errors in libraries.
    pub(super) skip_default_lib_check: bool,
    pub(super) out_dir: Option<String>,
    pub(super) declaration_dir: Option<String>,
    pub(super) root_dir: Option<String>,
    pub(super) composite: bool,
    /// TypeScript's `shouldPreserveConstEnums`: `preserveConstEnums`,
    /// `isolatedModules`, or `verbatimModuleSyntax`. A namespace of only
    /// const enums then counts as instantiated in merge-order checks.
    pub(super) preserve_const_enums: bool,
}

/// `paths` patterns and their substitutions, relative to `base`: the
/// directory of the configuration that declared them.
#[derive(Debug)]
pub(super) struct Paths {
    pub(super) base: String,
    pub(super) patterns: Vec<(String, Vec<String>)>,
}

impl Options {
    /// The settings TypeScript uses to resolve package-name `extends`, not
    /// the compiler defaults, which `convert` applies.
    pub(super) fn for_extends_lookup() -> Self {
        Self {
            module: Module::Node,
            resolution: Resolution::NodeNext,
            detection: Detection::Force,
            jsx: false,
            jsx_automatic: false,
            jsx_dev: false,
            jsx_import_source: None,
            paths: None,
            root_dirs: Vec::new(),
            type_roots: None,
            types: Vec::new(),
            custom_conditions: Vec::new(),
            module_suffixes: Vec::new(),
            preserve_symlinks: false,
            check_side_effect_imports: true,
            skip_lib_check: false,
            no_check: false,
            allow_js: false,
            allow_arbitrary_extensions: false,
            resolve_json_module: true,
            package_exports: true,
            package_imports: true,
            target: Target::LATEST_STANDARD,
            lib: None,
            no_lib: false,
            lib_replacement: false,
            skip_default_lib_check: false,
            out_dir: None,
            declaration_dir: None,
            root_dir: None,
            composite: false,
            preserve_const_enums: false,
        }
    }
}

/// One parsed configuration file.
#[derive(Debug)]
struct File {
    /// Absolute, as reached; its directory is canonical.
    path: String,
    dir: String,
    /// Fully canonical, for inheritance-cycle detection.
    identity: PathBuf,
    text: SourceText,
    /// The configuration object, after `config_root`.
    root: JsonValue,
}

impl File {
    fn read(path: &Path, disk: &disk::Disk, parser: &Parser) -> Result<Rc<File>, InputError> {
        let text = disk
            .read_to_string(path)
            .map_err(|error| io_error(path, &error))?;
        let identity = disk
            .canonicalize(path)
            .map_err(|error| io_error(path, &error))?;
        let parent = path.parent().unwrap_or(Path::new("/"));
        let dir = disk
            .canonicalize(parent)
            .map_err(|error| io_error(parent, &error))?;
        let dir = paths::from_path(&dir)?;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        let text = SourceText::shared(text);
        let parsed = parser
            .parse_json(text.as_str())
            .map_err(|error| InputError {
                kind: match error.kind {
                    JsonErrorKind::Syntax => InputErrorKind::Config,
                    JsonErrorKind::TooDeep => InputErrorKind::Unsupported,
                },
                message: error.message,
                path: Some(path.to_path_buf()),
                position: Some(text.position(error.offset)),
            })?;
        let mut file = File {
            path: paths::combine(&dir, &name),
            dir,
            identity,
            text,
            root: JsonValue {
                span: crate::Span { start: 0, end: 0 },
                kind: JsonKind::Object(Vec::new()),
            },
        };
        file.root = file.config_root(parsed.root)?;
        file.check_root()?;
        Ok(Rc::new(file))
    }

    /// TypeScript's `convertConfigFileToObject`: empty or trivia-only text is
    /// an empty configuration, and a root that is not an object is TS5092,
    /// unless it is an array holding an object, which the compiler reads in
    /// its place without a diagnostic.
    fn config_root(&self, root: Option<JsonValue>) -> Result<JsonValue, InputError> {
        let Some(root) = root else {
            return Ok(JsonValue {
                span: crate::Span { start: 0, end: 0 },
                kind: JsonKind::Object(Vec::new()),
            });
        };
        let error = self.error(
            &root,
            "The root value of a configuration file must be an object",
        );
        match root.kind {
            JsonKind::Object(_) => Ok(root),
            JsonKind::Array(items) => items
                .into_iter()
                .find(|item| matches!(item.kind, JsonKind::Object(_)))
                .ok_or(error),
            _ => Err(error),
        }
    }

    /// The checks `convertToJson` and `onPropertySet` make as they convert
    /// each property, whichever occurrence of a key applies: no `?` after a
    /// key (TS8009), and the type of every `files`, `include`, `exclude`, and
    /// `references` occurrence. `extends` and `compilerOptions` occurrences
    /// are checked where they are read.
    fn check_root(&self) -> Result<(), InputError> {
        let mut pending = vec![&self.root];
        while let Some(value) = pending.pop() {
            match &value.kind {
                JsonKind::Array(items) => pending.extend(items.iter().rev()),
                JsonKind::Object(properties) => {
                    if let Some(question) = properties.iter().find_map(|p| p.question) {
                        return Err(self.origin_at(question).error(
                            InputErrorKind::Config,
                            "The '?' modifier can only be used in TypeScript files",
                        ));
                    }
                    pending.extend(properties.iter().rev().map(|p| &p.value));
                }
                _ => {}
            }
        }
        for name in ["files", "include", "exclude"] {
            for value in occurrences(&self.root, name).filter(|value| !value.is_null()) {
                string_list(self, value, name, false)?;
            }
        }
        for value in occurrences(&self.root, "references").filter(|value| !value.is_null()) {
            if value.as_array().is_none() {
                return Err(self.error(value, "'references' must be an array"));
            }
        }
        Ok(())
    }

    fn origin(&self, value: &JsonValue) -> Origin {
        self.origin_at(value.span.start)
    }

    fn origin_at(&self, offset: usize) -> Origin {
        Origin {
            path: PathBuf::from(&self.path),
            position: Some(self.text.position(offset)),
        }
    }

    fn error(&self, value: &JsonValue, message: impl Into<String>) -> InputError {
        self.origin(value).error(InputErrorKind::Config, message)
    }

    fn unsupported(&self, value: &JsonValue, message: impl Into<String>) -> InputError {
        self.origin(value)
            .error(InputErrorKind::Unsupported, message)
    }

    /// `message` for a value that is not the string expected, unless it is a
    /// string this tool cannot represent exactly (`JsonString::Utf16`).
    fn not_string(&self, value: &JsonValue, message: impl Into<String>) -> InputError {
        match value.kind {
            JsonKind::String(JsonString::Utf16(_)) => self.unsupported(
                value,
                "This string has an unpaired UTF-16 surrogate, which cannot name a file, module, or option value",
            ),
            _ => self.error(value, message),
        }
    }
}

/// Every value of `key` in a configuration object, in source order.
fn occurrences<'a>(object: &'a JsonValue, key: &'a str) -> impl Iterator<Item = &'a JsonValue> {
    object
        .properties()
        .unwrap_or_default()
        .iter()
        .filter(move |property| property.key.as_str() == Some(key))
        .map(|property| &property.value)
}

/// The value of `key` that applies: TypeScript's `OrderedMap` keeps the last.
fn get<'a>(object: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    object
        .properties()?
        .iter()
        .rev()
        .find(|property| property.key.as_str() == Some(key))
        .map(|property| &property.value)
}

/// A property that applies and is not `null`.
fn field<'a>(object: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    get(object, key).filter(|value| !value.is_null())
}

/// An object as TypeScript's `convertObjectLiteralExpressionToJson` builds
/// its `OrderedMap`: each key once, at its first position (the first property
/// with it), with its last value, and no property whose name is empty, which
/// it neither inserts nor converts as an option. The shared JSON reader, JSON
/// modules, and package manifests keep such properties.
fn entries(object: &JsonValue) -> Option<Vec<(&JsonProperty, &JsonValue)>> {
    let mut entries: Vec<(&JsonProperty, &JsonValue)> = Vec::new();
    let mut positions: HashMap<&JsonString, usize> = HashMap::new();
    for property in object.properties()? {
        if property.key.as_str() == Some("") {
            continue;
        }
        match positions.get(&property.key) {
            Some(&index) => entries[index].1 = &property.value,
            None => {
                positions.insert(&property.key, entries.len());
                entries.push((property, &property.value));
            }
        }
    }
    Some(entries)
}

/// The strings of a `files`, `include`, or `exclude` list. A `null` entry
/// converts to nothing (`convertJsonOptionOfListType`), so it is left out.
/// An entry this tool cannot represent exactly is a string all the same, so
/// it is `Unsupported` only where the list `applies` and its entries name
/// files.
fn string_list<'a>(
    file: &File,
    value: &'a JsonValue,
    name: &str,
    applies: bool,
) -> Result<Vec<(&'a str, &'a JsonValue)>, InputError> {
    let Some(items) = value.as_array() else {
        return Err(file.error(value, format!("'{name}' must be an array")));
    };
    let mut strings = Vec::new();
    for item in items {
        match &item.kind {
            JsonKind::String(JsonString::Text(text)) => strings.push((text.as_str(), item)),
            JsonKind::String(JsonString::Utf16(_)) if !applies => {}
            JsonKind::Null => {}
            _ => return Err(file.not_string(item, format!("'{name}' entries must be strings"))),
        }
    }
    Ok(strings)
}

/// An option value with the file that declared it.
#[derive(Clone, Copy)]
struct Declared<'a> {
    value: &'a JsonValue,
    file: &'a File,
}

pub(super) fn load(path: &Path, disk: &disk::Disk, parser: &Parser) -> Result<Config, InputError> {
    let leaf = File::read(path, disk, parser)?;
    let chain = inheritance(&leaf, disk, parser)?;
    // Every occurrence of each compiler option in every file, which
    // TypeScript validates as it reads each file: every `compilerOptions`
    // object of a file and each repeated key.
    let mut options: HashMap<&str, Vec<Declared>> = HashMap::new();
    for file in &chain.order {
        for compiler_options in occurrences(&file.root, "compilerOptions") {
            if compiler_options.is_null() {
                continue;
            }
            let Some(properties) = compiler_options.properties() else {
                return Err(file.error(compiler_options, "'compilerOptions' must be an object"));
            };
            for property in properties {
                // A key that cannot be written exactly names no option, so it
                // is ignored like any unknown option.
                if let Some(name) = property.key.as_str() {
                    options.entry(name).or_default().push(Declared {
                        value: &property.value,
                        file,
                    });
                }
            }
        }
    }
    let leaf_dir = leaf.dir.clone();
    let [files, include, exclude] = ["files", "include", "exclude"].map(|key| chain.root_list(key));
    // A `null` keeps its key in the raw configuration, which blocks
    // inheritance, but then names no list (`getPropFromRaw`).
    fn list(declared: Option<Declared<'_>>) -> Option<Declared<'_>> {
        declared.filter(|declared| !declared.value.is_null())
    }
    let files_specs = list(files)
        .map(|d| specs_of(d, "files", &leaf_dir, false))
        .transpose()?;
    if let Some(references) = field(&leaf.root, "references") {
        let items = references.as_array().expect("checked when read");
        if !items.is_empty() {
            return Err(leaf.unsupported(
                references,
                "Project references are not supported; load each referenced project separately",
            ));
        }
    }
    // TypeScript's TS18002 takes the list after inheritance, entries that are
    // not strings included, and needs a `null` or absent `extends`.
    if let Some(declared) = list(files) {
        let references = field(&leaf.root, "references").and_then(JsonValue::as_array);
        if declared
            .value
            .as_array()
            .is_some_and(<[JsonValue]>::is_empty)
            && references.is_none_or(<[JsonValue]>::is_empty)
            && field(&leaf.root, "extends").is_none()
        {
            return Err(declared.file.error(
                declared.value,
                format!("The 'files' list in config file '{}' is empty", leaf.path),
            ));
        }
    }
    let options = convert(&options, &chain, &leaf)?;
    Ok(Config {
        path: path.to_path_buf(),
        dir: leaf_dir.clone(),
        file: leaf.path.clone(),
        // `canJsonReportNoInputFiles`: the raw configuration, after
        // inheritance, has neither key, though either may be `null`.
        report_no_inputs: files.is_none() && get(&leaf.root, "references").is_none(),
        files: files_specs,
        include: list(include)
            .map(|d| specs_of(d, "include", &leaf_dir, true))
            .transpose()?,
        exclude: list(exclude)
            .map(|d| specs_of(d, "exclude", &leaf_dir, false))
            .transpose()?,
        options,
    })
}

/// A configuration's `extends` graph.
struct Chain {
    /// Every file once, each `extends` target, in array order, before the
    /// file that names it, and a target shared by several files at its last
    /// application. Occurrences are validated in this order.
    order: Vec<Rc<File>>,
    /// Every file once, each after every file it names, which is the order
    /// in which TypeScript's `parseConfig` completes them.
    bases_first: Vec<Rc<File>>,
    /// By identity: the files a file's applying `extends` names, in order.
    bases: HashMap<PathBuf, Vec<Rc<File>>>,
}

impl Chain {
    /// TypeScript merges a file's own options over its bases'
    /// (`mergeCompilerOptions`) whenever it has an `extends` property, even
    /// an empty list, and only then do its explicit `null`s unset inherited
    /// values.
    fn extends(file: &File) -> bool {
        get(&file.root, "extends").is_some()
    }

    /// A fold over the graph, bases first: `step` computes a file's value
    /// from the values of its bases, in `extends` order, or of none when the
    /// file has no `extends`. Returns the selected configuration's value.
    fn fold<'a, T: Copy>(&'a self, mut step: impl FnMut(&'a File, Option<Vec<T>>) -> T) -> T {
        let mut values: HashMap<&Path, T> = HashMap::new();
        let mut last = None;
        for file in &self.bases_first {
            let bases = Self::extends(file).then(|| {
                self.bases[&file.identity]
                    .iter()
                    .map(|base| values[base.identity.as_path()])
                    .collect()
            });
            let value = step(file, bases);
            values.insert(&file.identity, value);
            last = Some(value);
        }
        last.expect("the selected configuration")
    }

    /// `files`, `include`, or `exclude` as TypeScript's raw configuration
    /// holds it after inheritance (`applyExtendedConfig`'s
    /// `setPropertyValue`): a file's own value when its raw object has the
    /// key at all, `null` included, which blocks inheritance; otherwise, the
    /// value of the last base whose own value, after its own inheritance, is
    /// a list. Paths in it stay relative to the file that declared them.
    fn root_list(&self, key: &str) -> Option<Declared<'_>> {
        self.fold(|file, bases: Option<Vec<Option<Declared>>>| {
            if let Some(value) = get(&file.root, key) {
                return Some(Declared { value, file });
            }
            bases?
                .into_iter()
                .flatten()
                .rfind(|declared| declared.value.as_array().is_some())
        })
    }

    /// Compiler option `name` after inheritance (`parseConfig`): a file
    /// without `extends` has its own setting; otherwise each base's merged
    /// options, in order, then the file's own setting, are merged as
    /// `mergeCompilerOptions` merges them: a set, nonzero value replaces the
    /// one before, and the merged file's explicit `null` unsets it.
    fn option<'a>(&'a self, name: &str, kind: Kind) -> Option<Declared<'a>> {
        let merge = |into: Option<Declared<'a>>, file: &File, value: Option<Declared<'a>>| {
            if explicit_null(file, name) {
                None
            } else {
                value.or(into)
            }
        };
        self.fold(|file, bases| {
            let own = own_setting(file, name, kind);
            let Some(bases) = bases else {
                return own;
            };
            let bases = self.bases[&file.identity].iter().zip(bases);
            let inherited = bases.fold(None, |into, (base, value)| merge(into, base, value));
            merge(inherited, file, own)
        })
    }
}

/// How TypeScript converts an option's value (`convertJsonOption`), which
/// decides whether an occurrence sets the option and whether the setting is
/// the zero value that inheritance passes over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bool,
    /// An enumeration, whose empty string converts to nothing.
    Enum,
    String,
    /// A string naming a file or directory, which an empty string names too:
    /// the declaring configuration's directory.
    Path,
    List,
    /// An object, such as `paths`.
    Object,
}

impl Kind {
    /// Whether an occurrence sets the option: `onPropertySet` sets one only
    /// from a non-nil converted value. A `null` converts to nil, except for a
    /// list, whose conversion is a nil list that still sets it, and so clears
    /// an earlier setting; an empty enumeration string converts to nil.
    fn sets(self, value: &JsonValue) -> bool {
        match self {
            Kind::List => true,
            Kind::Enum => !value.is_null() && value.as_str() != Some(""),
            Kind::Bool | Kind::String | Kind::Path | Kind::Object => !value.is_null(),
        }
    }

    /// Whether the setting an occurrence makes is the option's zero value, a
    /// nil list or an empty string, which leaves the option unset and which
    /// `mergeCompilerOptions` does not copy over an inherited value.
    fn zero(self, value: &JsonValue) -> bool {
        match self {
            Kind::List => value.is_null(),
            Kind::String => value.as_str() == Some(""),
            Kind::Bool | Kind::Enum | Kind::Path | Kind::Object => false,
        }
    }
}

/// A file's own setting of option `name`, before inheritance
/// (`parseOwnConfigOfJsonSourceFile`): the last occurrence, across all its
/// `compilerOptions` objects, that sets it, or none when that setting is the
/// zero value.
fn own_setting<'a>(file: &'a File, name: &str, kind: Kind) -> Option<Declared<'a>> {
    let mut setting = None;
    for options in occurrences(&file.root, "compilerOptions") {
        for property in options.properties().unwrap_or_default() {
            if property.key.as_str() == Some(name) && kind.sets(&property.value) {
                setting = Some(&property.value);
            }
        }
    }
    setting
        .filter(|value| !kind.zero(value))
        .map(|value| Declared { value, file })
}

/// Whether `file`'s raw `compilerOptions`, its last such object, holds an
/// explicit `null` for `name` as its last value, which `mergeCompilerOptions`
/// applies as an unset when it merges the file's options.
fn explicit_null(file: &File, name: &str) -> bool {
    get(&file.root, "compilerOptions")
        .and_then(|options| get(options, name))
        .is_some_and(JsonValue::is_null)
}

/// The configuration's `extends` graph (`Chain`). A first walk reads each
/// file once in `extends` order, reports cycles as it meets them, and
/// completes each file after the files it names (`bases_first`). `order` is
/// then the reverse of a preorder walk that visits targets from last to first
/// and skips files already visited, which keeps each file's last position in
/// linear time.
fn inheritance(leaf: &Rc<File>, disk: &disk::Disk, parser: &Parser) -> Result<Chain, InputError> {
    let mut files: HashMap<String, Rc<File>> = HashMap::new();
    let mut targets: HashMap<PathBuf, Vec<Rc<File>>> = HashMap::new();
    let mut bases_first = Vec::new();
    let mut active = HashSet::from([leaf.identity.clone()]);
    // Each frame is a file, its resolved `extends` targets, and the next one.
    let mut stack = vec![(leaf.clone(), extends_of(leaf, disk)?, 0)];
    while let Some((file, pending, next)) = stack.last_mut() {
        let Some((target, value)) = pending.get(*next).cloned() else {
            let parents = pending
                .iter()
                .map(|(target, _)| files[target].clone())
                .collect();
            active.remove(&file.identity);
            targets.insert(file.identity.clone(), parents);
            bases_first.push(file.clone());
            stack.pop();
            continue;
        };
        *next += 1;
        let file = file.clone();
        let parent = match files.get(&target) {
            Some(parent) => parent.clone(),
            None => {
                // Report an unreadable parent where the chain names it.
                let parent = File::read(Path::new(&target), disk, parser).map_err(|error| {
                    match error.kind {
                        InputErrorKind::Io => file.origin(&value).error(
                            InputErrorKind::Io,
                            format!("Cannot read file '{target}': {}", error.message),
                        ),
                        _ => error,
                    }
                })?;
                files.insert(target, parent.clone());
                parent
            }
        };
        if active.contains(&parent.identity) {
            let mut cycle: Vec<&str> = stack
                .iter()
                .map(|(active, ..)| active.path.as_str())
                .collect();
            cycle.push(&parent.path);
            return Err(file.error(
                &value,
                format!(
                    "Circularity detected while resolving configuration: {}",
                    cycle.join(" -> ")
                ),
            ));
        }
        if targets.contains_key(&parent.identity) {
            continue;
        }
        active.insert(parent.identity.clone());
        let pending = extends_of(&parent, disk)?;
        stack.push((parent, pending, 0));
    }
    let mut visited = HashSet::new();
    let mut order = Vec::new();
    let mut walk = vec![leaf.clone()];
    while let Some(file) = walk.pop() {
        if visited.insert(file.identity.clone()) {
            walk.extend(targets[&file.identity].iter().cloned());
            order.push(file);
        }
    }
    order.reverse();
    Ok(Chain {
        order,
        bases_first,
        bases: targets,
    })
}

/// The paths named by a file's `extends`, with the values that named them.
/// As in `getExtendsConfigPathOrArray`, every occurrence of `extends` is
/// checked and its paths looked up, and the last one applies. Unlike any
/// compiler option, `extends` may not be `null` (TS5024).
fn extends_of(file: &File, disk: &disk::Disk) -> Result<Vec<(String, JsonValue)>, InputError> {
    let mut applied = Vec::new();
    for value in occurrences(&file.root, "extends") {
        applied = Vec::new();
        let names: Vec<&JsonValue> = match &value.kind {
            JsonKind::String(_) => vec![value],
            JsonKind::Array(items) => items.iter().collect(),
            _ => {
                return Err(
                    file.not_string(value, "'extends' must be a string or an array of strings")
                );
            }
        };
        for name in names {
            let Some(text) = name.as_str() else {
                return Err(file.not_string(name, "'extends' entries must be strings"));
            };
            applied.push((extends_path(file, name, text, disk)?, name.clone()));
        }
    }
    Ok(applied)
}

/// TypeScript's `extends` lookup: a relative or rooted path, adding `.json`
/// when the file does not exist, or else a package resolved like a NodeNext
/// `require` of a JSON configuration.
fn extends_path(
    file: &File,
    value: &JsonValue,
    name: &str,
    disk: &disk::Disk,
) -> Result<String, InputError> {
    let name = paths::separators(name);
    if name.is_empty() {
        return Err(file.error(value, "'extends' cannot be an empty string"));
    }
    if paths::is_rooted(&name) || name.starts_with("./") || name.starts_with("../") {
        let path = paths::resolve(&file.dir, &name);
        let is_file = |path: &str| {
            disk.is_file(path)
                .map_err(|error| io_error(Path::new(path), &error))
        };
        if path.ends_with(".json") || is_file(&path)? {
            return Ok(path);
        }
        let with_json = format!("{path}.json");
        if is_file(&with_json)? {
            return Ok(with_json);
        }
    } else if let Some(path) = resolve::config_package(&name, &file.dir, disk)? {
        return Ok(path);
    }
    Err(file.error(value, format!("File '{name}' not found")))
}

/// A path written in a configuration: a leading `${configDir}` names the
/// selected configuration's directory; otherwise it is relative to the
/// declaring file.
fn config_path(file: &File, text: &str, leaf_dir: &str) -> String {
    match config_dir_tail(text) {
        Some(tail) => paths::resolve(leaf_dir, &format!(".{tail}")),
        None => paths::resolve(&file.dir, text),
    }
}

/// The rest of `text` after a leading `${configDir}`, matched without case.
fn config_dir_tail(text: &str) -> Option<&str> {
    const TEMPLATE: &str = "${configDir}";
    text.get(..TEMPLATE.len())
        .filter(|prefix| prefix.eq_ignore_ascii_case(TEMPLATE))
        .map(|_| &text[TEMPLATE.len()..])
}

fn specs_of(
    declared: Declared,
    name: &str,
    leaf_dir: &str,
    include: bool,
) -> Result<Specs, InputError> {
    let file = declared.file;
    let mut entries = Vec::new();
    for (text, item) in string_list(file, declared.value, name, true)? {
        if name != "files" {
            let normalized = paths::separators(text);
            let trailing_recursion =
                normalized == "**" || normalized.ends_with("/**") || normalized.ends_with("/**/");
            if include && trailing_recursion {
                return Err(file.error(
                    item,
                    format!("File specification cannot end in a recursive directory wildcard ('**'): '{text}'"),
                ));
            }
            if let Some(index) = normalized.find("**") {
                let after = &normalized[index..];
                if after.split('/').any(|part| part == "..") {
                    return Err(file.error(
                        item,
                        format!("File specification cannot contain a parent directory ('..') that appears after a recursive directory wildcard ('**'): '{text}'"),
                    ));
                }
            }
        }
        entries.push(Spec {
            text: text.to_owned(),
            path: config_path(file, text, leaf_dir),
            origin: file.origin(item),
        });
    }
    Ok(Specs { entries })
}

/// A string-array option's entries, each with its value for diagnostics.
type StringList<'a> = (Vec<(&'a str, &'a JsonValue)>, Declared<'a>);

/// Typed access to the compiler options. Each accessor validates every
/// occurrence of its option in every file, as `convertJsonOption` does for
/// each one TypeScript reads, and returns the occurrence that applies after
/// each file's own settings and inheritance (`Chain::option`). Only a value
/// that applies names anything, so only its strings this tool cannot
/// represent exactly are `Unsupported`.
struct Reader<'a, 'm> {
    occurrences: &'m HashMap<&'a str, Vec<Declared<'a>>>,
    chain: &'a Chain,
    leaf_dir: &'m str,
}

impl<'a> Reader<'a, '_> {
    /// The occurrence that applies, if the option is set.
    fn get(&self, name: &str, kind: Kind) -> Option<Declared<'a>> {
        self.chain.option(name, kind)
    }

    /// The occurrence that applies, after `check` accepts every non-null
    /// occurrence in order; `check` learns whether the one it sees applies.
    fn checked(
        &self,
        name: &str,
        kind: Kind,
        mut check: impl FnMut(Declared<'a>, bool) -> Result<(), InputError>,
    ) -> Result<Option<Declared<'a>>, InputError> {
        let applies = self.get(name, kind);
        let occurrences = self.occurrences.get(name).map_or(&[][..], Vec::as_slice);
        for &declared in occurrences {
            if !declared.value.is_null() {
                let applying = applies.is_some_and(|a| std::ptr::eq(a.value, declared.value));
                check(declared, applying)?;
            }
        }
        Ok(applies)
    }

    fn string(
        &self,
        name: &str,
        kind: Kind,
    ) -> Result<Option<(&'a str, Declared<'a>)>, InputError> {
        let declared =
            self.checked(name, kind, |declared, applies| match &declared.value.kind {
                JsonKind::String(JsonString::Text(_)) => Ok(()),
                JsonKind::String(JsonString::Utf16(_)) if !applies => Ok(()),
                _ => Err(type_error(declared, name, "string")),
            })?;
        Ok(declared.map(|declared| (declared.value.as_str().expect("checked"), declared)))
    }

    fn bool(&self, name: &str) -> Result<Option<bool>, InputError> {
        let declared = self.checked(name, Kind::Bool, |declared, _| {
            match declared.value.as_bool() {
                Some(_) => Ok(()),
                None => Err(type_error(declared, name, "boolean")),
            }
        })?;
        Ok(declared.and_then(|declared| declared.value.as_bool()))
    }

    /// A list of strings; `check_entry` validates each string entry of every
    /// occurrence, written exactly or not. A `null` entry converts to nothing
    /// and is left out (`convertJsonOptionOfListType`).
    fn strings_with(
        &self,
        name: &str,
        mut check_entry: impl FnMut(Declared<'a>, Option<&'a str>) -> Result<(), InputError>,
    ) -> Result<Option<StringList<'a>>, InputError> {
        let declared = self.checked(name, Kind::List, |declared, applies| {
            let Some(items) = declared.value.as_array() else {
                return Err(type_error(declared, name, "list"));
            };
            for item in items {
                let entry = Declared {
                    value: item,
                    file: declared.file,
                };
                match &item.kind {
                    JsonKind::String(JsonString::Text(text)) => check_entry(entry, Some(text))?,
                    JsonKind::String(JsonString::Utf16(_)) if !applies => {
                        check_entry(entry, None)?;
                    }
                    JsonKind::Null => {}
                    _ => {
                        return Err(declared
                            .file
                            .not_string(item, format!("'{name}' entries must be strings")));
                    }
                }
            }
            Ok(())
        })?;
        Ok(declared.map(|declared| {
            let strings = declared
                .value
                .as_array()
                .expect("checked")
                .iter()
                .filter_map(|item| Some((item.as_str()?, item)))
                .collect();
            (strings, declared)
        }))
    }

    /// A list of names, without its empty strings, which
    /// `convertJsonOptionOfListType` drops as falsy values.
    fn strings(&self, name: &str) -> Result<Option<StringList<'a>>, InputError> {
        Ok(self
            .strings_with(name, |_, _| Ok(()))?
            .map(|(items, declared)| {
                let items = items
                    .into_iter()
                    .filter(|(text, _)| !text.is_empty())
                    .collect();
                (items, declared)
            }))
    }

    /// A directory-valued option, in the form `Options` documents.
    fn directory(&self, name: &str) -> Result<Option<String>, InputError> {
        Ok(self.string(name, Kind::Path)?.map(|(text, declared)| {
            paths::directory(config_path(declared.file, text, self.leaf_dir))
        }))
    }

    /// A list of directories; an empty entry names the declaring
    /// configuration's directory, as any empty path does.
    fn directory_list(&self, name: &str) -> Result<Option<Vec<String>>, InputError> {
        Ok(self
            .strings_with(name, |_, _| Ok(()))?
            .map(|(items, declared)| {
                items
                    .into_iter()
                    .map(|(text, _)| {
                        paths::directory(config_path(declared.file, text, self.leaf_dir))
                    })
                    .collect()
            }))
    }

    /// A case-insensitive enumeration value (`enum_key`), which must be one
    /// of `keys`, the option's whole map: removed values are keys too, and
    /// only the value that applies reports them. `allowed` lists the values
    /// TypeScript's TS6046 message names. An empty string sets nothing.
    fn choice(
        &self,
        name: &str,
        keys: &[&str],
        allowed: &str,
    ) -> Result<Option<(String, Declared<'a>)>, InputError> {
        let declared = self.checked(name, Kind::Enum, |declared, applies| {
            match &declared.value.kind {
                JsonKind::String(JsonString::Text(text)) => match enum_key(text) {
                    Some(key) if !keys.contains(&key.as_str()) => {
                        Err(invalid_value(declared, name, allowed))
                    }
                    _ => Ok(()),
                },
                // No key is an unpaired surrogate.
                JsonKind::String(JsonString::Utf16(_)) if !applies => {
                    Err(invalid_value(declared, name, allowed))
                }
                _ => Err(type_error(declared, name, "string")),
            }
        })?;
        Ok(declared.map(|declared| {
            let key = enum_key(declared.value.as_str().expect("checked"));
            (key.expect("an empty string sets nothing"), declared)
        }))
    }
}

/// The key TypeScript's `convertJsonOptionOfEnumType` looks up for an
/// enumeration value or list entry: none for an empty string, which leaves
/// it unset without a diagnostic; otherwise Go's `strings.ToLower`, one
/// character for one, so `K` (KELVIN SIGN) is `k` and U+0130 is `i`.
fn enum_key(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.chars().map(crate::input::simple_lowercase).collect())
}

/// The values an enumeration's TS6046 message lists.
fn quoted(names: &[&str]) -> String {
    let quoted: Vec<String> = names.iter().map(|name| format!("'{name}'")).collect();
    quoted.join(", ")
}

fn type_error(declared: Declared, name: &str, kind: &str) -> InputError {
    declared.file.not_string(
        declared.value,
        format!("Compiler option '{name}' requires a value of type {kind}"),
    )
}

fn invalid_value(declared: Declared, name: &str, allowed: &str) -> InputError {
    declared.file.error(
        declared.value,
        format!("Argument for '{name}' option must be: {allowed}"),
    )
}

fn convert<'a>(
    options: &HashMap<&'a str, Vec<Declared<'a>>>,
    chain: &'a Chain,
    leaf: &File,
) -> Result<Options, InputError> {
    let read = Reader {
        occurrences: options,
        chain,
        leaf_dir: &leaf.dir,
    };
    if let Some(declared) = read.get("baseUrl", Kind::Path) {
        return Err(declared.file.unsupported(
            declared.value,
            "Option 'baseUrl' was removed in TypeScript 7; use 'paths' relative to the configuration",
        ));
    }
    for name in ["noResolve", "noDtsResolution"] {
        if read.bool(name)? == Some(true) {
            let declared = read.get(name, Kind::Bool).expect("present");
            return Err(declared.file.unsupported(
                declared.value,
                format!("Option '{name}' changes module resolution in a way that is not supported"),
            ));
        }
    }

    // `module` also decides `resolveJsonModule`'s default: TypeScript's
    // `GetResolveJsonModule` turns it on for `node20` and `nodenext`, off for
    // `node16` and `node18`, and otherwise only with bundler resolution.
    let (module, node_next_module, node_json_modules) = match read.choice(
        "module",
        &[
            "commonjs", "amd", "system", "umd", "es6", "es2015", "es2020", "es2022", "esnext",
            "node16", "node18", "node20", "nodenext", "preserve",
        ],
        "'commonjs', 'es6', 'es2015', 'es2020', 'es2022', 'esnext', 'node16', 'node18', 'node20', 'nodenext', 'preserve'",
    )? {
        None => (Module::Es, false, false),
        Some((value, declared)) => match value.as_str() {
            "commonjs" => (Module::CommonJs, false, false),
            "es6" | "es2015" | "es2020" | "es2022" | "esnext" => (Module::Es, false, false),
            "node16" | "node18" => (Module::Node, false, false),
            "node20" => (Module::Node, false, true),
            "nodenext" => (Module::Node, true, true),
            "preserve" => (Module::Preserve, false, false),
            "amd" | "umd" | "system" => {
                return Err(declared.file.unsupported(
                    declared.value,
                    format!("Option 'module={value}' was removed in TypeScript 7"),
                ));
            }
            _ => unreachable!("`choice` accepts only the option's keys"),
        },
    };
    let resolution = match read.choice(
        "moduleResolution",
        &["node16", "nodenext", "bundler", "classic", "node", "node10"],
        "'node16', 'nodenext', 'bundler'",
    )? {
        None => match module {
            Module::Node if node_next_module => Resolution::NodeNext,
            Module::Node => Resolution::Node16,
            _ => Resolution::Bundler,
        },
        Some((value, declared)) => {
            let resolution = match value.as_str() {
                "node16" => Resolution::Node16,
                "nodenext" => Resolution::NodeNext,
                "bundler" => Resolution::Bundler,
                "node" | "node10" | "classic" => {
                    return Err(declared.file.unsupported(
                        declared.value,
                        format!("Option 'moduleResolution={value}' was removed in TypeScript 7"),
                    ));
                }
                _ => unreachable!("`choice` accepts only the option's keys"),
            };
            let node_resolution = resolution != Resolution::Bundler;
            if node_resolution != (module == Module::Node) {
                return Err(declared.file.error(
                    declared.value,
                    if node_resolution {
                        format!("Option 'module' must be set to a Node.js module kind when option 'moduleResolution' is set to '{value}'")
                    } else {
                        "Option 'moduleResolution' must be 'node16' or 'nodenext' (or left unspecified) when option 'module' is a Node.js module kind".into()
                    },
                ));
            }
            resolution
        }
    };
    const DETECTIONS: [&str; 3] = ["auto", "legacy", "force"];
    let detection = match read.choice("moduleDetection", &DETECTIONS, &quoted(&DETECTIONS))? {
        None if module == Module::Node => Detection::Force,
        None => Detection::Auto,
        Some((value, _)) => match value.as_str() {
            "auto" => Detection::Auto,
            "legacy" => Detection::Legacy,
            "force" => Detection::Force,
            _ => unreachable!("`choice` accepts only the option's keys"),
        },
    };
    const JSX: [&str; 5] = [
        "preserve",
        "react-native",
        "react-jsx",
        "react-jsxdev",
        "react",
    ];
    let jsx = read.choice("jsx", &JSX, &quoted(&JSX))?;
    let jsx_set = jsx.is_some();
    let jsx_dev = jsx
        .as_ref()
        .is_some_and(|(value, _)| value == "react-jsxdev");
    let jsx_automatic = jsx
        .as_ref()
        .is_some_and(|(value, _)| matches!(value.as_str(), "react-jsx" | "react-jsxdev"));

    let target = match read.choice("target", &Target::keys(), &Target::option_names())? {
        None => Target::LATEST_STANDARD,
        Some((value, declared)) => match Target::from_key(&value) {
            Some(Target::Es5) => {
                return Err(declared.file.unsupported(
                    declared.value,
                    "Option 'target=ES5' was removed in TypeScript 7",
                ));
            }
            Some(target) => target,
            None => unreachable!("`choice` accepts only the option's keys"),
        },
    };
    let no_lib = read.bool("noLib")?.unwrap_or(false);
    // Each entry of every `lib` occurrence must name a library (TS6046).
    let lib_entry = |entry: Declared, text: Option<&str>| match text.map(enum_key) {
        Some(None) => Ok(()),
        Some(Some(key)) if libraries::option_file(&key).is_some() => Ok(()),
        _ => Err(invalid_value(entry, "lib", &libraries::option_names())),
    };
    let lib = match read.strings_with("lib", lib_entry)? {
        None => None,
        Some((_, declared)) if no_lib => {
            return Err(declared.file.error(
                declared.value,
                "Option 'lib' cannot be specified with option 'noLib'",
            ));
        }
        Some((entries, _)) => {
            let mut files = Vec::new();
            for key in entries.into_iter().filter_map(|(text, _)| enum_key(text)) {
                let file = libraries::option_file(&key).expect("checked");
                if !files.contains(&file) {
                    files.push(file);
                }
            }
            Some(files)
        }
    };

    let types = match read.strings("types")? {
        None => Vec::new(),
        Some((items, declared)) => items
            .into_iter()
            .map(|(name, item)| (name.to_owned(), declared.file.origin(item)))
            .collect(),
    };
    let strings = |name: &str| -> Result<Vec<String>, InputError> {
        Ok(read
            .strings(name)?
            .map(|(items, _)| items.into_iter().map(|(text, _)| text.to_owned()).collect())
            .unwrap_or_default())
    };

    Ok(Options {
        module,
        resolution,
        detection,
        jsx_automatic,
        jsx_dev,
        jsx: jsx_set,
        jsx_import_source: read
            .string("jsxImportSource", Kind::String)?
            .map(|(text, _)| text.to_owned()),
        paths: read
            .checked("paths", Kind::Object, |declared, _| {
                match declared.value.properties() {
                    Some(_) => Ok(()),
                    None => Err(type_error(declared, "paths", "object")),
                }
            })?
            .map(|d| paths_option(d, &leaf.dir))
            .transpose()?,
        root_dirs: read.directory_list("rootDirs")?.unwrap_or_default(),
        type_roots: read.directory_list("typeRoots")?,
        types,
        custom_conditions: strings("customConditions")?,
        // The one list that keeps empty strings (`listPreserveFalsyValues`):
        // an empty suffix tries the name as written.
        module_suffixes: read
            .strings_with("moduleSuffixes", |_, _| Ok(()))?
            .map(|(items, _)| items.into_iter().map(|(text, _)| text.to_owned()).collect())
            .unwrap_or_default(),
        preserve_symlinks: read.bool("preserveSymlinks")?.unwrap_or(false),
        check_side_effect_imports: read.bool("noUncheckedSideEffectImports")?.unwrap_or(true),
        skip_lib_check: read.bool("skipLibCheck")?.unwrap_or(false),
        no_check: read.bool("noCheck")?.unwrap_or(false),
        allow_js: read.bool("allowJs")?.unwrap_or(false),
        allow_arbitrary_extensions: read.bool("allowArbitraryExtensions")?.unwrap_or(false),
        resolve_json_module: read.bool("resolveJsonModule")?.unwrap_or(match module {
            Module::Node => node_json_modules,
            _ => resolution == Resolution::Bundler,
        }),
        package_exports: read.bool("resolvePackageJsonExports")?.unwrap_or(true),
        package_imports: read.bool("resolvePackageJsonImports")?.unwrap_or(true),
        target,
        lib,
        no_lib,
        lib_replacement: read.bool("libReplacement")?.unwrap_or(false),
        skip_default_lib_check: read.bool("skipDefaultLibCheck")?.unwrap_or(false),
        out_dir: read.directory("outDir")?,
        declaration_dir: read.directory("declarationDir")?,
        root_dir: read.directory("rootDir")?,
        composite: read.bool("composite")?.unwrap_or(false),
        preserve_const_enums: read.bool("preserveConstEnums")?.unwrap_or(false)
            || read.bool("isolatedModules")?.unwrap_or(false)
            || read.bool("verbatimModuleSyntax")?.unwrap_or(false),
    })
}

fn paths_option(declared: Declared, leaf_dir: &str) -> Result<Paths, InputError> {
    let file = declared.file;
    let mut patterns = Vec::new();
    for (property, value) in entries(declared.value).expect("checked") {
        // A pattern matches specifiers by their UTF-16 code units, so one
        // with an unpaired surrogate cannot be matched exactly here.
        let Some(pattern) = property.key.as_str() else {
            return Err(file.origin_at(property.key_span.start).error(
                InputErrorKind::Unsupported,
                "A 'paths' pattern has an unpaired UTF-16 surrogate, which cannot match a module name exactly",
            ));
        };
        if pattern.matches('*').count() > 1 {
            return Err(file.error(
                value,
                format!("Pattern '{pattern}' can have at most one '*' character"),
            ));
        }
        let Some(items) = value.as_array() else {
            return Err(file.error(
                value,
                format!("Substitutions for pattern '{pattern}' should be an array"),
            ));
        };
        if items.is_empty() {
            return Err(file.error(
                value,
                format!("Substitutions for pattern '{pattern}' shouldn't be an empty array"),
            ));
        }
        let mut substitutions = Vec::new();
        for item in items {
            let Some(text) = item.as_str() else {
                return Err(file.not_string(
                    item,
                    format!("Substitution for pattern '{pattern}' must be a string"),
                ));
            };
            if text.matches('*').count() > 1 {
                return Err(file.error(
                    item,
                    format!("Substitution '{text}' in pattern '{pattern}' can have at most one '*' character"),
                ));
            }
            let text = text.to_owned();
            let template = config_dir_tail(&text).is_some();
            if !template && !paths::is_relative(&text) && !paths::is_rooted(&text) {
                return Err(file.error(
                    item,
                    "Non-relative paths are not allowed. Did you forget a leading './'?",
                ));
            }
            substitutions.push(if template {
                // Keep a `*` in the template's tail for later substitution.
                config_path(file, &text, leaf_dir)
            } else {
                text
            });
        }
        patterns.push((pattern.to_owned(), substitutions));
    }
    Ok(Paths {
        base: file.dir.clone(),
        patterns,
    })
}
