//! Project configuration, file selection, and module resolution.
//!
//! Loading reads the selected configuration and its `extends` chain, selects
//! root files, then parses each program input exactly once while following
//! its module requests and reference directives. Search files are the roots
//! and every input reachable from them without crossing an external-library
//! edge, matching the compiler's `isSourceFileFromExternalLibrary`.

mod config;
mod directives;
mod files;
mod jsonc;
mod paths;
mod resolve;
#[cfg(test)]
mod tests;
mod version;

use super::binding::{
    self, FileFacts, FileOptions, ModuleRequest, ModuleResolution, RequestKind, ResolutionMode,
    Suppressions,
};
use super::source::{FileId, SourceFile, SourceText, Sources};
use super::{ProjectError, ProjectErrorKind, SourcePosition};
use crate::{ParseOptions, parse_with_options};
use config::{Config, Detection, Module, Resolution};
use directives::Directive;
use resolve::{FileFormat, Format, Resolved, Resolver};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

/// The parse budget for program inputs. Generated declaration files, such as
/// nested schema types, need several hundred grammar entries; the default
/// budget suits a parser on an ordinary thread stack.
pub(super) const PARSE_DEPTH: usize = 1024;

/// The stack of the thread that loads a project, sized for `PARSE_DEPTH` in
/// unoptimized builds. It is reserved address space, committed only as used.
pub(super) const LOAD_STACK_BYTES: usize = 256 << 20;

/// Captured program inputs with their facts and resolved module edges.
#[derive(Debug)]
pub(super) struct Program {
    /// Absolute path of the selected configuration file.
    pub(super) config: PathBuf,
    /// Whether the file system distinguishes paths that differ only in case.
    pub(super) case_sensitive: bool,
    pub(super) sources: Sources,
    /// Indexed by `FileId`, parallel to `sources`.
    pub(super) files: Vec<ProgramFile>,
}

#[derive(Debug)]
pub(super) struct ProgramFile {
    /// Whether occurrences in this file belong in results.
    pub(super) search: bool,
    /// Normalized absolute paths through which the file was reached, which
    /// queries may name; the identity path is in `Sources`.
    pub(super) paths: Vec<PathBuf>,
    pub(super) facts: FileFacts,
    /// Parallel to `facts.requests`.
    pub(super) modules: Vec<ModuleResolution>,
    /// A declaration file under `skipLibCheck`, whose errors TypeScript does
    /// not report.
    pub(super) unchecked: bool,
    pub(super) suppressions: Suppressions,
}

/// Loads the program a configuration selects. Errors are `Io` for an
/// unreadable configuration or input (including a missing `files` entry),
/// `Config` for invalid configuration content or inheritance, `Syntax` for an
/// input that does not parse, `Unsupported` outside the supported profile,
/// and `UnresolvedImport` for a missing type library or reference directive
/// target that TypeScript would report. A module request that finds no file
/// is `ModuleResolution::Missing` for the binder to settle against ambient
/// module declarations.
pub(super) fn load(config: &Path) -> Result<Program, ProjectError> {
    let path = std::path::absolute(config).map_err(|error| io_error(config, &error))?;
    let config = config::load(&path)?;
    let case_sensitive = case_sensitive_file_system();
    let roots = files::roots(&config, case_sensitive)?;
    if roots.is_empty() && config.report_no_inputs {
        let specs = |specs: &Option<config::Specs>| {
            let entries: Vec<String> = specs
                .iter()
                .flat_map(|specs| &specs.entries)
                .map(|spec| format!("{:?}", spec.text))
                .collect();
            format!("[{}]", entries.join(","))
        };
        let include = if config.include.is_none() && config.files.is_none() {
            "[\"**/*\"]".to_owned()
        } else {
            specs(&config.include)
        };
        return Err(ProjectError {
            kind: ProjectErrorKind::Config,
            message: format!(
                "No inputs were found in config file '{}'. Specified 'include' paths were '{include}' and 'exclude' paths were '{}'",
                path.display(),
                specs(&config.exclude)
            ),
            path: Some(path),
            position: None,
        });
    }
    for root in &roots {
        let kind = if paths::is_javascript(&root.path) {
            "JavaScript"
        } else if root.path.ends_with(".json") {
            "JSON"
        } else {
            continue;
        };
        let message = format!("{kind} inputs are not supported");
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
    let mut loader = Loader {
        config: &config,
        resolver: Resolver::new(&config.options),
        case_sensitive,
        ids: HashMap::new(),
        queue: VecDeque::new(),
        sources: Sources::default(),
        files: Vec::new(),
        edges: Vec::new(),
        search_roots: Vec::new(),
        reached: Vec::new(),
    };
    for root in &roots {
        let id = loader.enqueue(&root.path)?;
        loader.search_roots.push(id);
    }
    if !roots.is_empty() {
        loader.type_packages()?;
    }
    while let Some(path) = loader.queue.pop_front() {
        loader.load_file(path)?;
    }
    let search = loader.search();
    let mut files = loader.files;
    for ((file, search), paths) in files.iter_mut().zip(search).zip(loader.reached) {
        file.search = search;
        file.paths = paths;
    }
    Ok(Program {
        config: path,
        case_sensitive,
        sources: loader.sources,
        files,
    })
}

/// Where a configuration value or directive was written, for diagnostics.
#[derive(Debug, Clone)]
struct Origin {
    path: PathBuf,
    position: Option<SourcePosition>,
}

impl Origin {
    fn error(&self, kind: ProjectErrorKind, message: impl Into<String>) -> ProjectError {
        ProjectError {
            kind,
            message: message.into(),
            path: Some(self.path.clone()),
            position: self.position,
        }
    }
}

fn io_error(path: &Path, error: &std::io::Error) -> ProjectError {
    ProjectError {
        kind: ProjectErrorKind::Io,
        message: error.to_string(),
        path: Some(path.to_path_buf()),
        position: None,
    }
}

/// TypeScript's test: the file system is case-insensitive when the running
/// executable is also reachable with its letters' case swapped.
fn case_sensitive_file_system() -> bool {
    if cfg!(windows) {
        return false;
    }
    let Ok(executable) = std::env::current_exe() else {
        return true;
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
    swapped == text || !Path::new(&swapped).exists()
}

struct Loader<'a> {
    config: &'a Config,
    resolver: Resolver<'a>,
    case_sensitive: bool,
    /// File identity keys to their ids, assigned in queue order.
    ids: HashMap<String, FileId>,
    queue: VecDeque<String>,
    sources: Sources,
    files: Vec<ProgramFile>,
    /// Per loaded file: its dependencies and whether each edge is external.
    edges: Vec<Vec<(FileId, bool)>>,
    search_roots: Vec<FileId>,
    /// Paths through which each id was reached, by id.
    reached: Vec<Vec<PathBuf>>,
}

impl Loader<'_> {
    /// The id of an input, queueing it on first sight. Identity is the real
    /// path unless `preserveSymlinks` is set.
    fn enqueue(&mut self, path: &str) -> Result<FileId, ProjectError> {
        let identity = if self.config.options.preserve_symlinks {
            paths::normalize(path)
        } else {
            let real =
                std::fs::canonicalize(path).map_err(|error| io_error(Path::new(path), &error))?;
            paths::from_path(&real)
        };
        let key = if self.case_sensitive {
            identity.clone()
        } else {
            identity.to_lowercase()
        };
        let lexical = PathBuf::from(paths::normalize(path));
        let id = match self.ids.get(&key) {
            Some(&id) => id,
            None => {
                let id = FileId(u32::try_from(self.ids.len()).expect("file count fits in u32"));
                self.ids.insert(key, id);
                self.queue.push_back(identity);
                self.reached.push(Vec::new());
                id
            }
        };
        if !self.reached[id.index()].contains(&lexical) {
            self.reached[id.index()].push(lexical);
        }
        Ok(id)
    }

    /// The `types` entries, expanding `*` to every package in the type roots.
    fn type_packages(&mut self) -> Result<(), ProjectError> {
        let config = self.config;
        let mut names: Vec<(String, &Origin)> = Vec::new();
        for (name, origin) in &config.options.types {
            if name == "*" {
                for automatic in self.resolver.automatic_types() {
                    names.push((automatic, origin));
                }
            } else {
                names.push((name.clone(), origin));
            }
        }
        for (name, origin) in names {
            let Some(found) = self.resolver.type_reference(&name, &config.dir, None) else {
                return Err(origin.error(
                    ProjectErrorKind::UnresolvedImport,
                    format!("Cannot find type definition file for '{name}'"),
                ));
            };
            let id = self.enqueue(&found)?;
            if !paths::has_node_modules(&found) {
                self.search_roots.push(id);
            }
        }
        Ok(())
    }

    fn display(&self, path: &str) -> String {
        paths::relative(&self.config.dir, path)
    }

    /// Parses one input once, captures its facts, and resolves its edges.
    fn load_file(&mut self, path: String) -> Result<(), ProjectError> {
        let id = FileId(u32::try_from(self.sources.len()).expect("file count fits in u32"));
        let file_path = PathBuf::from(&path);
        let text =
            std::fs::read_to_string(&file_path).map_err(|error| io_error(&file_path, &error))?;
        let source = SourceText::new(text);
        let format = self.resolver.file_format(&path);
        // Extensions follow the file system's case policy.
        let spelled = if self.case_sensitive {
            path.clone()
        } else {
            path.to_lowercase()
        };
        let declaration = paths::is_declaration(&spelled);
        let facts = {
            let options = ParseOptions {
                jsx: spelled.ends_with(".tsx"),
                file_name: spelled.clone(),
                max_depth: PARSE_DEPTH,
            };
            let ast = parse_with_options(source.as_str(), options).map_err(|error| {
                let (line, column) = error.line_column(source.as_str());
                ProjectError {
                    kind: ProjectErrorKind::Syntax,
                    message: error.message,
                    path: Some(file_path.clone()),
                    position: Some(SourcePosition { line, column }),
                }
            })?;
            binding::collect(id, &ast, self.file_options(&path, declaration, format)).map_err(
                |mut error| {
                    error.path.get_or_insert_with(|| file_path.clone());
                    error
                },
            )?
        };
        let mut facts = facts;
        // TypeScript imports an automatic JSX runtime into every `.tsx` file
        // and reports a missing one at the file's first JSX tag.
        let runtime = self.jsx_runtime(&spelled, declaration, source.as_str());
        if let Some(specifier) = &runtime {
            let at = facts.jsx.unwrap_or(0);
            facts.requests.push(ModuleRequest {
                specifier: specifier.clone(),
                kind: RequestKind::Import { side_effect: true },
                mode: None,
                span: crate::Span { start: at, end: at },
            });
        }
        let unchecked = declaration && self.config.options.skip_lib_check;
        let suppressions = Suppressions::new(&source, &facts);
        let runtime_request = runtime.map(|_| facts.requests.len() - 1);
        let mut edges = Vec::new();
        let mut modules = Vec::with_capacity(facts.requests.len());
        for (index, request) in facts.requests.iter().enumerate() {
            let mode = self.request_mode(request, format);
            modules.push(match self.resolver.module(&request.specifier, &path, mode) {
                Resolved::File { path, external } => {
                    let target = self.enqueue(&path)?;
                    edges.push((target, external));
                    ModuleResolution::File(target)
                }
                Resolved::Missing if runtime_request == Some(index) => match facts.jsx {
                    // TypeScript requires the runtime only where JSX is used.
                    Some(_) => ModuleResolution::Missing,
                    None => ModuleResolution::Unchecked,
                },
                Resolved::Missing
                    if request.kind == (RequestKind::Import { side_effect: true })
                        && !self.config.options.check_side_effect_imports =>
                {
                    ModuleResolution::Unchecked
                }
                Resolved::Missing => ModuleResolution::Missing,
                Resolved::JavaScript(target) => ModuleResolution::Unsupported(format!(
                    "'{}' resolves to the JavaScript file '{}', which has no TypeScript declarations",
                    request.specifier,
                    self.display(&target)
                )),
                Resolved::Json(target) => ModuleResolution::Unsupported(format!(
                    "'{}' resolves to the JSON module '{}'; JSON modules are not supported",
                    request.specifier,
                    self.display(&target)
                )),
                Resolved::TooDeep => ModuleResolution::Unsupported(format!(
                    "'{}' follows more than {} package imports redirects",
                    request.specifier,
                    resolve::MAX_IMPORTS_CHAIN
                )),
            });
        }
        for reference in directives::scan(source.as_str()) {
            let origin = Origin {
                path: file_path.clone(),
                position: Some(source.position(reference.offset)),
            };
            match self.directive(&path, format, reference.directive, &origin) {
                Err(error)
                    if error.kind == ProjectErrorKind::UnresolvedImport
                        && (unchecked || suppressions.covers(&source, reference.offset)) => {}
                result => edges.extend(result?),
            }
        }
        self.sources.add(SourceFile {
            path: file_path,
            display: self.display(&path),
            text: source,
        });
        self.files.push(ProgramFile {
            search: false,
            paths: Vec::new(),
            facts,
            modules,
            unchecked,
            suppressions,
        });
        self.edges.push(edges);
        Ok(())
    }

    /// TypeScript's resolution mode for one request. In Node.js modes static
    /// imports follow the importing file's format; in bundler mode they
    /// follow its emitted syntax, which `module` decides unless the file's
    /// extension (or, in `node_modules`, its package) fixes the format.
    /// An explicit `resolution-mode` is honored wherever the binder records
    /// one; the compiler honors it only on type-only imports and exports and
    /// on import types, and rejects it elsewhere.
    fn request_mode(&self, request: &ModuleRequest, format: FileFormat) -> ResolutionMode {
        let options = &self.config.options;
        let mode = |format: Format| match format {
            Format::Esm => ResolutionMode::Import,
            Format::CommonJs => ResolutionMode::Require,
        };
        let syntax = match options.resolution {
            Resolution::Node16 | Resolution::NodeNext => format.implied,
            Resolution::Bundler => {
                format
                    .declared
                    .unwrap_or(if options.module == Module::CommonJs {
                        Format::CommonJs
                    } else {
                        Format::Esm
                    })
            }
        };
        match request.kind {
            RequestKind::Require => ResolutionMode::Require,
            RequestKind::Dynamic => match options.resolution {
                Resolution::Bundler if options.module != Module::Preserve => mode(syntax),
                _ => ResolutionMode::Import,
            },
            RequestKind::Import { .. }
            | RequestKind::ReExport
            | RequestKind::ImportType
            | RequestKind::Augmentation => request.mode.unwrap_or(mode(syntax)),
        }
    }

    fn file_options(&self, path: &str, declaration: bool, format: FileFormat) -> FileOptions {
        let options = &self.config.options;
        let force_module = !declaration
            && match options.detection {
                Detection::Force => true,
                Detection::Legacy => false,
                Detection::Auto => {
                    paths::has_extension(path, &[".mts", ".cts", ".mjs", ".cjs"])
                        || match options.resolution {
                            Resolution::Node16 | Resolution::NodeNext => {
                                format.implied == Format::Esm
                            }
                            Resolution::Bundler => format.declared == Some(Format::Esm),
                        }
                }
            };
        FileOptions {
            force_module,
            jsx_module: options.detection == Detection::Auto && options.jsx_automatic,
        }
    }

    /// The automatic JSX runtime module a `.tsx` file implicitly imports,
    /// following `jsx`, `jsxImportSource`, and the file's JSX pragmas.
    fn jsx_runtime(&self, path: &str, declaration: bool, text: &str) -> Option<String> {
        if declaration || !path.ends_with(".tsx") {
            return None;
        }
        let options = &self.config.options;
        let pragmas = directives::jsx_pragmas(text);
        if pragmas.runtime.as_deref() == Some("classic") {
            return None;
        }
        let automatic = options.jsx_automatic
            || options.jsx_import_source.is_some()
            || pragmas.import_source.is_some()
            || pragmas.runtime.as_deref() == Some("automatic");
        if !automatic {
            return None;
        }
        let source = pragmas
            .import_source
            .or_else(|| options.jsx_import_source.clone())
            .unwrap_or_else(|| "react".into());
        let module = if options.jsx_dev {
            "jsx-dev-runtime"
        } else {
            "jsx-runtime"
        };
        Some(format!("{source}/{module}"))
    }

    /// Follows one reference directive, returning the edge it adds.
    fn directive(
        &mut self,
        path: &str,
        format: FileFormat,
        directive: Directive,
        origin: &Origin,
    ) -> Result<Option<(FileId, bool)>, ProjectError> {
        let config = self.config;
        let options = &config.options;
        match directive {
            Directive::Lib(name) => {
                if options.no_lib {
                    return Ok(None);
                }
                Err(origin.error(
                    ProjectErrorKind::Unsupported,
                    format!("Standard library '{name}' is not provided; set 'noLib' to ignore library references"),
                ))
            }
            Directive::Types(name, mode) => {
                // Without an attribute, the containing file's format decides.
                let mode = mode.or(match options.resolution {
                    Resolution::Bundler => format.declared,
                    _ => Some(format.implied),
                }
                .map(|format| match format {
                    Format::Esm => ResolutionMode::Import,
                    Format::CommonJs => ResolutionMode::Require,
                }));
                let Some(found) = self
                    .resolver
                    .type_reference(&name, paths::parent(path), mode)
                else {
                    return Err(origin.error(
                        ProjectErrorKind::UnresolvedImport,
                        format!("Cannot find type definition file for '{name}'"),
                    ));
                };
                let id = self.enqueue(&found)?;
                Ok(Some((id, paths::has_node_modules(&found))))
            }
            Directive::Path(reference) => {
                let target = paths::resolve(paths::parent(path), &reference.replace('\\', "/"));
                let found = if !paths::basename(&target).contains('.') {
                    [".ts", ".tsx", ".d.ts"]
                        .iter()
                        .map(|extension| format!("{target}{extension}"))
                        .find(|candidate| Path::new(candidate).is_file())
                } else if paths::is_typescript(&target) {
                    Path::new(&target).is_file().then_some(target)
                } else {
                    let kind = if paths::is_javascript(&target) {
                        "JavaScript"
                    } else if target.ends_with(".json") {
                        "JSON"
                    } else {
                        "unsupported"
                    };
                    return Err(origin.error(
                        ProjectErrorKind::Unsupported,
                        format!("Referenced file '{reference}' is a {kind} input, which is not supported"),
                    ));
                };
                let Some(found) = found else {
                    return Err(origin.error(
                        ProjectErrorKind::UnresolvedImport,
                        format!("File '{reference}' not found"),
                    ));
                };
                Ok(Some((self.enqueue(&found)?, false)))
            }
        }
    }

    /// Inputs reachable from the roots and local type packages without
    /// crossing an external-library edge.
    fn search(&self) -> Vec<bool> {
        let mut search = vec![false; self.files.len()];
        let mut stack = self.search_roots.clone();
        while let Some(id) = stack.pop() {
            if std::mem::replace(&mut search[id.index()], true) {
                continue;
            }
            for &(target, external) in &self.edges[id.index()] {
                if !external && !search[target.index()] {
                    stack.push(target);
                }
            }
        }
        search
    }
}
