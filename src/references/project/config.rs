//! The effective configuration of one `tsconfig.json`: ordered `extends`
//! inheritance, option validation, and path-valued settings resolved against
//! the configuration file that declared them.

use super::jsonc::{self, Kind, Value};
use super::paths;
use super::resolve;
use super::{Origin, io_error};
use crate::references::source::SourceText;
use crate::references::{ProjectError, ProjectErrorKind};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug)]
pub(super) struct Config {
    /// Absolute path of the selected configuration, not canonicalized.
    pub(super) path: PathBuf,
    /// Canonical directory of the selected configuration.
    pub(super) dir: String,
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
#[derive(Debug)]
pub(super) struct Options {
    pub(super) module: Module,
    pub(super) resolution: Resolution,
    pub(super) detection: Detection,
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
    pub(super) allow_js: bool,
    pub(super) resolve_json_module: bool,
    pub(super) package_exports: bool,
    pub(super) package_imports: bool,
    pub(super) no_lib: bool,
    pub(super) out_dir: Option<String>,
    pub(super) declaration_dir: Option<String>,
    pub(super) root_dir: Option<String>,
    pub(super) composite: bool,
    /// Canonical directory and path of the selected configuration.
    pub(super) config_dir: String,
    pub(super) config_file: String,
}

/// `paths` patterns and their substitutions, relative to `base`: the
/// directory of the configuration that declared them.
#[derive(Debug)]
pub(super) struct Paths {
    pub(super) base: String,
    pub(super) patterns: Vec<(String, Vec<String>)>,
}

impl Default for Options {
    /// The settings TypeScript uses to resolve package-name `extends`.
    fn default() -> Self {
        Self {
            module: Module::Node,
            resolution: Resolution::NodeNext,
            detection: Detection::Force,
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
            allow_js: false,
            resolve_json_module: true,
            package_exports: true,
            package_imports: true,
            no_lib: false,
            out_dir: None,
            declaration_dir: None,
            root_dir: None,
            composite: false,
            config_dir: String::new(),
            config_file: String::new(),
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
    root: Value,
}

impl File {
    fn read(path: &Path) -> Result<Rc<File>, ProjectError> {
        let text = std::fs::read_to_string(path).map_err(|error| io_error(path, &error))?;
        let identity = std::fs::canonicalize(path).map_err(|error| io_error(path, &error))?;
        let parent = path.parent().unwrap_or(Path::new("/"));
        let dir = std::fs::canonicalize(parent).map_err(|error| io_error(parent, &error))?;
        let dir = paths::from_path(&dir);
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        let text = SourceText::new(text);
        let root = jsonc::parse(text.as_str()).map_err(|error| ProjectError {
            kind: ProjectErrorKind::Config,
            message: error.message,
            path: Some(path.to_path_buf()),
            position: Some(text.position(error.offset)),
        })?;
        let file = File {
            path: paths::combine(&dir, &name),
            dir,
            identity,
            text,
            root,
        };
        if file.root.entries().is_none() {
            return Err(file.error(
                &file.root,
                "The root value of a configuration file must be an object",
            ));
        }
        Ok(Rc::new(file))
    }

    fn origin(&self, value: &Value) -> Origin {
        Origin {
            path: PathBuf::from(&self.path),
            position: Some(self.text.position(value.offset)),
        }
    }

    fn error(&self, value: &Value, message: impl Into<String>) -> ProjectError {
        self.origin(value).error(ProjectErrorKind::Config, message)
    }

    fn unsupported(&self, value: &Value, message: impl Into<String>) -> ProjectError {
        self.origin(value)
            .error(ProjectErrorKind::Unsupported, message)
    }
}

/// An option value with the file that declared it.
#[derive(Clone, Copy)]
struct Declared<'a> {
    value: &'a Value,
    file: &'a File,
}

pub(super) fn load(path: &Path) -> Result<Config, ProjectError> {
    let leaf = File::read(path)?;
    let chain = inheritance(&leaf)?;
    let mut options: HashMap<&str, Declared> = HashMap::new();
    let mut specs: [Option<Declared>; 3] = [None, None, None];
    for file in &chain {
        if let Some(compiler_options) = file.root.field("compilerOptions") {
            let Some(entries) = compiler_options.entries() else {
                return Err(file.error(compiler_options, "'compilerOptions' must be an object"));
            };
            for (name, value) in entries {
                options.insert(name, Declared { value, file });
            }
        }
        for (slot, name) in specs.iter_mut().zip(["files", "include", "exclude"]) {
            if let Some(value) = file.root.field(name) {
                *slot = Some(Declared { value, file });
            }
        }
    }
    let leaf_dir = leaf.dir.clone();
    let [files, include, exclude] = specs;
    let files_specs = files
        .map(|d| specs_of(d, "files", &leaf_dir, false))
        .transpose()?;
    if let Some(references) = leaf.root.field("references") {
        let Some(items) = references.as_array() else {
            return Err(leaf.error(references, "'references' must be an array"));
        };
        if !items.is_empty() {
            return Err(leaf.unsupported(
                references,
                "Project references are not supported; load each referenced project separately",
            ));
        }
    }
    if let (Some(specs), Some(declared)) = (&files_specs, files) {
        let references = leaf.root.field("references").and_then(Value::as_array);
        if specs.entries.is_empty()
            && references.is_none_or(<[Value]>::is_empty)
            && leaf.root.get("extends").is_none()
        {
            return Err(declared.file.error(
                declared.value,
                format!("The 'files' list in config file '{}' is empty", leaf.path),
            ));
        }
    }
    let options = convert(&options, &leaf)?;
    Ok(Config {
        path: path.to_path_buf(),
        dir: leaf_dir.clone(),
        report_no_inputs: files.is_none()
            && leaf.root.get("files").is_none()
            && leaf.root.get("references").is_none(),
        files: files_specs,
        include: include
            .map(|d| specs_of(d, "include", &leaf_dir, true))
            .transpose()?,
        exclude: exclude
            .map(|d| specs_of(d, "exclude", &leaf_dir, false))
            .transpose()?,
        options,
    })
}

/// The configuration files in application order: each `extends` target, in
/// array order, before the file that names it. Settings override by "last
/// setter wins", so only the last application of a shared base matters. A
/// first walk reads each file once in `extends` order and reports cycles as
/// it meets them; the order is then the reverse of a preorder walk that
/// visits targets from last to first and skips files already visited, which
/// keeps each file's last position in linear time.
fn inheritance(leaf: &Rc<File>) -> Result<Vec<Rc<File>>, ProjectError> {
    let mut files: HashMap<String, Rc<File>> = HashMap::new();
    let mut targets: HashMap<PathBuf, Vec<Rc<File>>> = HashMap::new();
    let mut active = HashSet::from([leaf.identity.clone()]);
    // Each frame is a file, its resolved `extends` targets, and the next one.
    let mut stack = vec![(leaf.clone(), extends_of(leaf)?, 0)];
    while let Some((file, pending, next)) = stack.last_mut() {
        let Some((target, value)) = pending.get(*next).cloned() else {
            let parents = pending
                .iter()
                .map(|(target, _)| files[target].clone())
                .collect();
            active.remove(&file.identity);
            targets.insert(file.identity.clone(), parents);
            stack.pop();
            continue;
        };
        *next += 1;
        let file = file.clone();
        let parent = match files.get(&target) {
            Some(parent) => parent.clone(),
            None => {
                let parent = File::read(Path::new(&target))?;
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
        let pending = extends_of(&parent)?;
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
    Ok(order)
}

/// The paths named by a file's `extends`, with the values that named them.
fn extends_of(file: &File) -> Result<Vec<(String, Value)>, ProjectError> {
    let Some(value) = file.root.field("extends") else {
        return Ok(Vec::new());
    };
    let names: Vec<&Value> = match &value.kind {
        Kind::String(_) => vec![value],
        Kind::Array(items) => items.iter().collect(),
        _ => return Err(file.error(value, "'extends' must be a string or an array of strings")),
    };
    names
        .into_iter()
        .map(|name| {
            let Some(text) = name.as_str() else {
                return Err(file.error(name, "'extends' entries must be strings"));
            };
            Ok((extends_path(file, name, text)?, name.clone()))
        })
        .collect()
}

/// TypeScript's `extends` lookup: a relative or rooted path, adding `.json`
/// when the file does not exist, or else a package resolved like a NodeNext
/// `require` of a JSON configuration.
fn extends_path(file: &File, value: &Value, name: &str) -> Result<String, ProjectError> {
    let name = name.replace('\\', "/");
    if name.is_empty() {
        return Err(file.error(value, "'extends' cannot be an empty string"));
    }
    if paths::is_rooted(&name) || name.starts_with("./") || name.starts_with("../") {
        let path = paths::resolve(&file.dir, &name);
        if Path::new(&path).is_file() || path.ends_with(".json") {
            return Ok(path);
        }
        let with_json = format!("{path}.json");
        if Path::new(&with_json).is_file() {
            return Ok(with_json);
        }
    } else if let Some(path) = resolve::config_package(&name, &file.dir) {
        return Ok(path);
    }
    Err(file.error(value, format!("File '{name}' not found")))
}

/// Replaces a leading `${configDir}` with the selected configuration's
/// directory, or resolves relative to the declaring file.
fn config_path(file: &File, text: &str, leaf_dir: &str) -> String {
    const TEMPLATE: &str = "${configDir}";
    match text.get(..TEMPLATE.len()) {
        Some(prefix) if prefix.eq_ignore_ascii_case(TEMPLATE) => {
            paths::resolve(leaf_dir, &format!(".{}", &text[TEMPLATE.len()..]))
        }
        _ => paths::resolve(&file.dir, text),
    }
}

fn specs_of(
    declared: Declared,
    name: &str,
    leaf_dir: &str,
    include: bool,
) -> Result<Specs, ProjectError> {
    let file = declared.file;
    let Some(items) = declared.value.as_array() else {
        return Err(file.error(declared.value, format!("'{name}' must be an array")));
    };
    let mut entries = Vec::new();
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(file.error(item, format!("'{name}' entries must be strings")));
        };
        if name != "files" {
            let normalized = text.replace('\\', "/");
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
            path: config_path(file, &text.replace('\\', "/"), leaf_dir),
            origin: file.origin(item),
        });
    }
    Ok(Specs { entries })
}

/// A string-array option's entries, each with its value for diagnostics.
type StringList<'a> = (Vec<(&'a str, &'a Value)>, Declared<'a>);

/// Typed access to the merged `compilerOptions`.
struct Reader<'a, 'm> {
    options: &'m HashMap<&'a str, Declared<'a>>,
    leaf_dir: &'m str,
}

impl<'a> Reader<'a, '_> {
    /// A present, non-null option.
    fn get(&self, name: &str) -> Option<Declared<'a>> {
        self.options
            .get(name)
            .copied()
            .filter(|declared| declared.value.kind != Kind::Null)
    }

    fn string(&self, name: &str) -> Result<Option<(&'a str, Declared<'a>)>, ProjectError> {
        let Some(declared) = self.get(name) else {
            return Ok(None);
        };
        match declared.value.as_str() {
            Some(text) => Ok(Some((text, declared))),
            None => Err(type_error(declared, name, "string")),
        }
    }

    fn bool(&self, name: &str) -> Result<Option<bool>, ProjectError> {
        let Some(declared) = self.get(name) else {
            return Ok(None);
        };
        declared
            .value
            .as_bool()
            .map(Some)
            .ok_or_else(|| type_error(declared, name, "boolean"))
    }

    fn strings(&self, name: &str) -> Result<Option<StringList<'a>>, ProjectError> {
        let Some(declared) = self.get(name) else {
            return Ok(None);
        };
        let items = declared
            .value
            .as_array()
            .ok_or_else(|| type_error(declared, name, "list"))?;
        let strings = items
            .iter()
            .map(|item| {
                item.as_str().map(|text| (text, item)).ok_or_else(|| {
                    declared
                        .file
                        .error(item, format!("'{name}' entries must be strings"))
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Some((strings, declared)))
    }

    fn path(&self, name: &str) -> Result<Option<String>, ProjectError> {
        Ok(self.string(name)?.map(|(text, declared)| {
            config_path(declared.file, &text.replace('\\', "/"), self.leaf_dir)
        }))
    }

    fn path_list(&self, name: &str) -> Result<Option<Vec<String>>, ProjectError> {
        Ok(self.strings(name)?.map(|(items, declared)| {
            items
                .into_iter()
                .map(|(text, _)| {
                    config_path(declared.file, &text.replace('\\', "/"), self.leaf_dir)
                })
                .collect()
        }))
    }

    /// A case-insensitive enumeration value.
    fn choice(&self, name: &str) -> Result<Option<(String, Declared<'a>)>, ProjectError> {
        Ok(self
            .string(name)?
            .map(|(text, declared)| (text.to_ascii_lowercase(), declared)))
    }
}

fn type_error(declared: Declared, name: &str, kind: &str) -> ProjectError {
    declared.file.error(
        declared.value,
        format!("Compiler option '{name}' requires a value of type {kind}"),
    )
}

fn invalid_value(declared: Declared, name: &str, allowed: &str) -> ProjectError {
    declared.file.error(
        declared.value,
        format!("Argument for '{name}' option must be: {allowed}"),
    )
}

fn convert(options: &HashMap<&str, Declared>, leaf: &File) -> Result<Options, ProjectError> {
    let read = Reader {
        options,
        leaf_dir: &leaf.dir,
    };
    if let Some(declared) = read.get("baseUrl") {
        return Err(declared.file.unsupported(
            declared.value,
            "Option 'baseUrl' was removed in TypeScript 7; use 'paths' relative to the configuration",
        ));
    }
    for name in ["noResolve", "noDtsResolution"] {
        if read.bool(name)? == Some(true) {
            let declared = read.get(name).expect("present");
            return Err(declared.file.unsupported(
                declared.value,
                format!("Option '{name}' changes module resolution in a way that is not supported"),
            ));
        }
    }

    let module = match read.choice("module")? {
        None => Module::Es,
        Some((value, declared)) => match value.as_str() {
            "commonjs" => Module::CommonJs,
            "es6" | "es2015" | "es2020" | "es2022" | "esnext" => Module::Es,
            "node16" | "node18" | "node20" | "nodenext" => Module::Node,
            "preserve" => Module::Preserve,
            "amd" | "umd" | "system" => {
                return Err(declared.file.unsupported(
                    declared.value,
                    format!("Option 'module={value}' was removed in TypeScript 7"),
                ));
            }
            _ => {
                return Err(invalid_value(
                    declared,
                    "module",
                    "'commonjs', 'es6', 'es2015', 'es2020', 'es2022', 'esnext', 'node16', 'node18', 'node20', 'nodenext', 'preserve'",
                ));
            }
        },
    };
    let node_next_module = read
        .choice("module")?
        .is_some_and(|(value, _)| value == "nodenext");
    let resolution = match read.choice("moduleResolution")? {
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
                _ => {
                    return Err(invalid_value(
                        declared,
                        "moduleResolution",
                        "'node16', 'nodenext', 'bundler'",
                    ));
                }
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
    let detection = match read.choice("moduleDetection")? {
        None if module == Module::Node => Detection::Force,
        None => Detection::Auto,
        Some((value, declared)) => match value.as_str() {
            "auto" => Detection::Auto,
            "legacy" => Detection::Legacy,
            "force" => Detection::Force,
            _ => {
                return Err(invalid_value(
                    declared,
                    "moduleDetection",
                    "'auto', 'legacy', 'force'",
                ));
            }
        },
    };
    let jsx = read.choice("jsx")?;
    let jsx_dev = jsx
        .as_ref()
        .is_some_and(|(value, _)| value == "react-jsxdev");
    let jsx_automatic = match jsx {
        None => false,
        Some((value, declared)) => match value.as_str() {
            "react-jsx" | "react-jsxdev" => true,
            "preserve" | "react" | "react-native" => false,
            _ => {
                return Err(invalid_value(
                    declared,
                    "jsx",
                    "'preserve', 'react-native', 'react-jsx', 'react-jsxdev', 'react'",
                ));
            }
        },
    };

    let no_lib = read.bool("noLib")?.unwrap_or(false);
    if let Some((libraries, declared)) = read.strings("lib")? {
        if no_lib {
            return Err(declared.file.error(
                declared.value,
                "Option 'lib' cannot be specified with option 'noLib'",
            ));
        }
        if !libraries.is_empty() {
            return Err(declared.file.unsupported(
                declared.value,
                "Standard library declarations are not provided; remove 'lib' or set 'noLib'",
            ));
        }
    }

    let types = match read.strings("types")? {
        None => Vec::new(),
        Some((items, declared)) => items
            .into_iter()
            .map(|(name, item)| (name.to_owned(), declared.file.origin(item)))
            .collect(),
    };
    let strings = |name: &str| -> Result<Vec<String>, ProjectError> {
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
        jsx_import_source: read
            .string("jsxImportSource")?
            .map(|(text, _)| text.to_owned()),
        paths: read
            .get("paths")
            .map(|d| paths_option(d, &leaf.dir))
            .transpose()?,
        root_dirs: read.path_list("rootDirs")?.unwrap_or_default(),
        type_roots: read.path_list("typeRoots")?,
        types,
        custom_conditions: strings("customConditions")?,
        module_suffixes: strings("moduleSuffixes")?,
        preserve_symlinks: read.bool("preserveSymlinks")?.unwrap_or(false),
        check_side_effect_imports: read.bool("noUncheckedSideEffectImports")?.unwrap_or(true),
        skip_lib_check: read.bool("skipLibCheck")?.unwrap_or(false),
        allow_js: read.bool("allowJs")?.unwrap_or(false),
        resolve_json_module: read.bool("resolveJsonModule")?.unwrap_or(true),
        package_exports: read.bool("resolvePackageJsonExports")?.unwrap_or(true),
        package_imports: read.bool("resolvePackageJsonImports")?.unwrap_or(true),
        no_lib,
        out_dir: read.path("outDir")?,
        declaration_dir: read.path("declarationDir")?,
        root_dir: read.path("rootDir")?,
        composite: read.bool("composite")?.unwrap_or(false),
        config_dir: leaf.dir.clone(),
        config_file: leaf.path.clone(),
    })
}

fn paths_option(declared: Declared, leaf_dir: &str) -> Result<Paths, ProjectError> {
    let file = declared.file;
    let Some(entries) = declared.value.entries() else {
        return Err(type_error(declared, "paths", "object"));
    };
    let mut patterns = Vec::new();
    for (pattern, value) in entries {
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
                return Err(file.error(
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
            let text = text.replace('\\', "/");
            let template = text
                .get(.."${configDir}".len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("${configDir}"));
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
        patterns.push((pattern.clone(), substitutions));
    }
    Ok(Paths {
        base: file.dir.clone(),
        patterns,
    })
}
