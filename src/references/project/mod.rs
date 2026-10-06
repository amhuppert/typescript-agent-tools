//! Project configuration, file selection, and module resolution.
//!
//! Loading reads the selected configuration and its `extends` chain, selects
//! root files, then parses each program input exactly once while following
//! its module requests and reference directives. Search files are the roots
//! and every input reachable from them without crossing an external-library
//! edge, matching the compiler's `isSourceFileFromExternalLibrary`.

mod config;
mod diagnostics;
mod directives;
mod disk;
mod files;
mod jsonc;
mod paths;
mod resolve;
#[cfg(test)]
mod tests;
mod version;

use super::binding::{self, FileFacts, FileOptions, ModuleRequest, ModuleResolution, RequestKind};
use super::source::{FileId, SourceFile, SourceText, Sources};
use super::{ProjectError, ProjectErrorKind, SourcePosition};
use crate::{ParseOptions, parse_with_options};
use config::{Config, Detection};
use diagnostics::{Diagnostics, FileOrigin, MissingInputs};
use directives::Directive;
use resolve::{FileFormat, Resolved, Resolver};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use paths::Case;
pub(super) use paths::FileLookup;
pub(super) use resolve::is_relative_name;

/// The stack of the thread that loads a project. Program inputs parse with
/// `ParseOptions::DEEP_MAX_DEPTH`, because generated declaration files need
/// several hundred grammar entries. The other recursion during loading, a
/// package `imports` chain of `resolve::MAX_IMPORTS_CHAIN` lookups, needs
/// under 4 MiB and never runs during a parse.
pub(super) const LOAD_STACK_BYTES: usize = crate::DEEP_PARSE_STACK_BYTES;

/// Captured program inputs with their facts and resolved module edges.
#[derive(Debug)]
pub(super) struct Program {
    /// The selected configuration file in its canonical directory, which
    /// result paths are relative to.
    pub(super) config: PathBuf,
    pub(super) case: Case,
    pub(super) sources: Sources,
    /// Indexed by `FileId`, parallel to `sources`.
    pub(super) files: Vec<ProgramFile>,
    /// TypeScript's `shouldPreserveConstEnums`, which binding's merge checks
    /// need (`config::Options::preserve_const_enums`).
    pub(super) preserve_const_enums: bool,
}

impl Program {
    /// Freeze every spelling through which a searched file may be queried.
    pub(super) fn file_lookup(&self) -> FileLookup {
        let mut lookup = FileLookup::new(self.case);
        for id in self.sources.ids() {
            let file = &self.files[id.index()];
            if file.search {
                lookup.insert(&self.sources.get(id).path, id);
                for path in &file.paths {
                    lookup.insert(path, id);
                }
            }
        }
        lookup
    }
}

#[derive(Debug)]
pub(super) struct ProgramFile {
    /// Whether occurrences in this file belong in results.
    pub(super) search: bool,
    /// Normalized absolute paths through which the file was reached or
    /// may be named under the configuration directory as the caller spelled
    /// it; the identity path is in `Sources`.
    pub(super) paths: Vec<PathBuf>,
    pub(super) facts: FileFacts,
    /// Parallel to `facts.requests`.
    pub(super) modules: Vec<ModuleResolution>,
    pub(super) diagnostics: Diagnostics,
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
    let case = Case::detect();
    let roots = files::roots(&config, case)?;
    let mut loader = Loader {
        config: &config,
        resolver: Resolver::new(&config, case),
        case,
        ids: HashMap::new(),
        queue: VecDeque::new(),
        sources: Sources::default(),
        loaded: Vec::new(),
        edges: Vec::new(),
        search_roots: Vec::new(),
        reached: Vec::new(),
    };
    let loaded = loader.load_all(&roots);
    // An unreadable manifest or type root may have caused another failure.
    loader.resolver.check_reads()?;
    loaded?;
    // Provenance is known only now that the dependency graph is complete.
    let search = loader.search();
    let spelled_dir = path.parent().unwrap_or(Path::new("/"));
    let mut files = Vec::with_capacity(loader.loaded.len());
    let inputs = loader.loaded.into_iter().zip(search).zip(loader.reached);
    for (id, ((loaded, search), mut paths)) in loader.sources.ids().zip(inputs) {
        let origin = if search {
            FileOrigin::Project
        } else {
            FileOrigin::Dependency
        };
        let diagnostics = Diagnostics::new(origin, loaded.missing);
        if let Some(error) = loaded.unsupported {
            if !diagnostics.tolerates_unsupported_input() {
                return Err(error);
            }
        }
        if let Some(spelled) = spelled_under(spelled_dir, &loader.sources.get(id).display) {
            paths.push(spelled);
        }
        files.push(ProgramFile {
            search,
            paths,
            facts: loaded.facts,
            modules: loaded.modules,
            diagnostics,
        });
    }
    Ok(Program {
        preserve_const_enums: config.options.preserve_const_enums,
        config: PathBuf::from(&config.file),
        case,
        sources: loader.sources,
        files,
    })
}

/// A file under the configuration directory, named through that directory
/// as the caller spelled it, such as a symlink or `/tmp` for `/private/tmp`.
/// A file outside it has no such spelling: `..` from a link does not lead
/// where it leads from the real directory that `display` is relative to.
fn spelled_under(spelled_dir: &Path, display: &str) -> Option<PathBuf> {
    // `display` is relative to the configuration directory unless it lies
    // on another root, where it is absolute by the platform's rules.
    let under = Path::new(display).is_relative() && !display.starts_with("../");
    under.then(|| {
        PathBuf::from(paths::normalize(&paths::from_path(
            &spelled_dir.join(display),
        )))
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

struct Loader<'a> {
    config: &'a Config,
    resolver: Resolver<'a>,
    case: Case,
    /// File identity keys to the ids `Sources` reserved for them.
    ids: HashMap<String, FileId>,
    /// Identity paths awaiting capture, in reservation order.
    queue: VecDeque<(String, FileId)>,
    sources: Sources,
    /// Per captured file, by id.
    loaded: Vec<Loaded>,
    /// Per captured file: its dependencies and whether each edge is external.
    edges: Vec<Vec<(FileId, bool)>>,
    search_roots: Vec<FileId>,
    /// Paths through which each id was reached, by id.
    reached: Vec<Vec<PathBuf>>,
}

/// One captured input whose provenance is not yet known.
struct Loaded {
    facts: FileFacts,
    modules: Vec<ModuleResolution>,
    missing: MissingInputs,
    /// The first unsupported reference path target, an error unless the file
    /// turns out to be a dependency.
    unsupported: Option<ProjectError>,
}

/// What following one reference directive produced.
enum Followed {
    /// An edge to an input, and whether it crosses into an external library.
    Edge(FileId, bool),
    /// Nothing to follow, such as a library reference under `noLib`.
    Nothing,
    /// No input exists; an error unless a missing input is tolerated where
    /// the directive is written.
    Missing(ProjectError),
    /// An input outside the supported profile; an error unless the file turns
    /// out to be a dependency.
    Unsupported(ProjectError),
}

impl Loader<'_> {
    /// Capture the roots, the configured type packages, and everything they
    /// reach.
    fn load_all(&mut self, roots: &[files::Root]) -> Result<(), ProjectError> {
        for root in roots {
            let id = self.enqueue(&root.path)?;
            self.search_roots.push(id);
        }
        if !roots.is_empty() {
            self.type_packages()?;
        }
        while let Some((path, id)) = self.queue.pop_front() {
            self.load_file(path, id)?;
        }
        Ok(())
    }

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
        let key = self.case.key(&identity);
        let lexical = PathBuf::from(paths::normalize(path));
        let id = match self.ids.get(&key) {
            Some(&id) => id,
            None => {
                let id = self.sources.reserve();
                self.ids.insert(key, id);
                self.queue.push_back((identity, id));
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
    fn load_file(&mut self, path: String, id: FileId) -> Result<(), ProjectError> {
        let file_path = PathBuf::from(&path);
        let text =
            std::fs::read_to_string(&file_path).map_err(|error| io_error(&file_path, &error))?;
        let source = SourceText::new(text);
        let format = self.resolver.file_format(&path);
        // What the name means follows TypeScript, which classifies the exact
        // spelling; the case policy decides only identity and support.
        let declaration = paths::is_declaration(&path);
        let options = ParseOptions {
            max_depth: ParseOptions::DEEP_MAX_DEPTH,
            ..ParseOptions::for_file(path.as_str())
        };
        let jsx = options.jsx;
        let mut facts = {
            let ast =
                parse_with_options(source.as_str(), options).map_err(|error| ProjectError {
                    kind: ProjectErrorKind::Syntax,
                    position: Some(source.position(error.span.start)),
                    message: error.message,
                    path: Some(file_path.clone()),
                })?;
            binding::collect(&ast, &source, self.file_options(&path, declaration, format)).map_err(
                |mut error| {
                    error.path.get_or_insert_with(|| file_path.clone());
                    error
                },
            )?
        };
        // TypeScript imports an automatic JSX runtime into every TSX file and
        // reports a missing one at the file's first JSX tag.
        let runtime = self.jsx_runtime(jsx, declaration, source.as_str());
        if let Some(specifier) = &runtime {
            let at = facts.jsx.unwrap_or(0);
            facts.requests.push(ModuleRequest {
                specifier: specifier.clone(),
                kind: RequestKind::Import { side_effect: true },
                mode: None,
                span: crate::Span { start: at, end: at },
            });
        }
        let missing = MissingInputs::new(
            declaration,
            self.config.options.skip_lib_check,
            &source,
            &facts,
        );
        let runtime_request = runtime.map(|_| facts.requests.len() - 1);
        let mut edges = Vec::new();
        let mut modules = Vec::with_capacity(facts.requests.len());
        for (index, request) in facts.requests.iter().enumerate() {
            let mode = self
                .resolver
                .request_mode(format, request.kind, request.mode);
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
                // A limit of this tool, not an unsupported input: no file's
                // diagnostic policy may tolerate it.
                Resolved::TooDeep => {
                    return Err(ProjectError {
                        kind: ProjectErrorKind::Unsupported,
                        message: format!(
                            "'{}' follows more than {} package imports redirects",
                            request.specifier,
                            resolve::MAX_IMPORTS_CHAIN
                        ),
                        path: Some(file_path.clone()),
                        position: Some(source.position(request.span.start)),
                    });
                }
            });
        }
        let mut unsupported = None;
        for reference in directives::scan(source.as_str()) {
            let origin = Origin {
                path: file_path.clone(),
                position: Some(source.position(reference.offset)),
            };
            match self.directive(&path, format, reference.directive, &origin)? {
                Followed::Edge(target, external) => edges.push((target, external)),
                Followed::Nothing => {}
                Followed::Missing(error) => {
                    if !missing.tolerated(&source, reference.offset) {
                        return Err(error);
                    }
                }
                // Settled with provenance, as an unsupported module target is.
                Followed::Unsupported(error) => {
                    unsupported.get_or_insert(error);
                }
            }
        }
        let display = self.display(&path);
        self.sources.capture(
            id,
            SourceFile {
                path: file_path,
                display,
                text: source,
            },
        );
        self.loaded.push(Loaded {
            facts,
            modules,
            missing,
            unsupported,
        });
        self.edges.push(edges);
        Ok(())
    }

    fn file_options(&self, path: &str, declaration: bool, format: FileFormat) -> FileOptions {
        let options = &self.config.options;
        let force_module = !declaration
            && match options.detection {
                Detection::Force => true,
                Detection::Legacy => false,
                Detection::Auto => self.resolver.format_forces_module(path, format),
            };
        FileOptions {
            force_module,
            jsx_module: options.detection == Detection::Auto && options.jsx_automatic,
        }
    }

    /// The automatic JSX runtime module a TSX file implicitly imports,
    /// following `jsx`, `jsxImportSource`, and the file's JSX pragmas.
    fn jsx_runtime(&self, jsx: bool, declaration: bool, text: &str) -> Option<String> {
        if declaration || !jsx {
            return None;
        }
        let options = &self.config.options;
        let pragmas = crate::pragmas::jsx_pragmas(text);
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

    /// Follows one reference directive. An error is fatal wherever the
    /// directive appears; other failures are outcomes the caller settles.
    fn directive(
        &mut self,
        path: &str,
        format: FileFormat,
        directive: Directive,
        origin: &Origin,
    ) -> Result<Followed, ProjectError> {
        let config = self.config;
        let options = &config.options;
        match directive {
            Directive::Lib(name) => {
                if options.no_lib {
                    return Ok(Followed::Nothing);
                }
                Err(origin.error(
                    ProjectErrorKind::Unsupported,
                    format!("Standard library '{name}' is not provided; set 'noLib' to ignore library references"),
                ))
            }
            Directive::Types(name, mode) => {
                let mode = mode.or_else(|| self.resolver.type_directive_mode(format));
                let Some(found) = self
                    .resolver
                    .type_reference(&name, paths::parent(path), mode)
                else {
                    return Ok(Followed::Missing(origin.error(
                        ProjectErrorKind::UnresolvedImport,
                        format!("Cannot find type definition file for '{name}'"),
                    )));
                };
                let id = self.enqueue(&found)?;
                Ok(Followed::Edge(id, paths::has_node_modules(&found)))
            }
            Directive::Path(reference) => {
                let target = paths::resolve(paths::parent(path), &reference);
                let unsupported = |reason: String| {
                    Ok(Followed::Unsupported(origin.error(
                        ProjectErrorKind::Unsupported,
                        format!("Referenced file '{reference}' {reason}"),
                    )))
                };
                match files::literal_input(&target, self.case)? {
                    files::Literal::Unsupported(kind) => unsupported(format!(
                        "is a {} input, which is not supported",
                        kind.label()
                    )),
                    files::Literal::Found(found) => {
                        Ok(Followed::Edge(self.enqueue(&found)?, false))
                    }
                    files::Literal::Missing => Ok(Followed::Missing(origin.error(
                        ProjectErrorKind::UnresolvedImport,
                        format!("File '{reference}' not found"),
                    ))),
                    files::Literal::UnsupportedExtension => {
                        unsupported("has an unsupported extension".into())
                    }
                }
            }
        }
    }

    /// Inputs reachable from the roots and local type packages without
    /// crossing an external-library edge.
    fn search(&self) -> Vec<bool> {
        let mut search = vec![false; self.loaded.len()];
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
