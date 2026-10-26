//! Project configuration, file selection, and module resolution.
//!
//! Loading reads the selected configuration and its `extends` chain, selects
//! root files and standard libraries, then captures each program input while
//! following its module requests and reference directives. An unchanged
//! input can reuse its capture; a changed module status recollects its facts.
//! Search files are the roots and every input reachable from them, or from a root
//! library, without crossing an external-library edge, matching the
//! compiler's `isSourceFileFromExternalLibrary`, less the standard libraries
//! (`IsSourceFileDefaultLibrary`). Program order is `Program.GetSourceFiles`':
//! libraries first, then each file after the files it reaches
//! (`collectFiles`).

mod capture;
mod config;
mod diagnostics;
mod directives;
pub(super) mod disk;
mod files;
mod libraries;
mod manifest;
mod module_semantics;
pub(in crate::project) mod paths;
mod resolve;
pub(in crate::project) mod specifier_environment;
pub(in crate::project) mod specifiers;
#[cfg(test)]
mod tests;
mod version;

use super::binding::{self, FileFacts, FileOptions, ModuleRequest, ModuleResolution, RequestKind};
use super::navigation::Captured;
use super::sources::{FileId, SourceFile, Sources};
use crate::ScriptKind;
use crate::analysis::Parser;
use crate::analysis::{InputError, InputErrorKind};
use crate::input::Case;
use crate::source::SourcePosition;
use config::{Config, Detection};
use diagnostics::{Checking, FileOrigin, Unchecked};
use directives::Directive;
use libraries::Library;
use resolve::{FileFormat, NotAdmitted, Resolved, Resolver};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

pub(super) use config::Resolution as ModuleResolutionKind;
pub(super) use config::SemanticOptions;
pub(super) use diagnostics::Diagnostics;
pub(super) use module_semantics::{FileModuleSemantics, ModuleKind};
pub(super) use paths::FileLookup;
pub(super) use paths::{from_path as native_file_name, remove_file_extension};
pub(super) use resolve::is_relative_name;

/// Reusable file products and resolution decisions of the last successfully
/// published generation.
/// A refresh still assigns new FileIds and rebuilds all project relationships.
#[derive(Default)]
pub(in crate::project) struct Reuse {
    captures: capture::Cache,
    resolutions: resolve::Reuse,
}

#[cfg(test)]
impl Reuse {
    pub(in crate::project) fn capture_identity(&self, path: &Path) -> (usize, Option<usize>) {
        let path = std::fs::canonicalize(path).unwrap();
        let capture = &self.captures[path.to_str().unwrap()];
        match capture.parsed.contents.as_ref().unwrap() {
            capture::Contents::Json(facts) => (facts.collection_identity(), None),
            capture::Contents::TypeScript {
                navigation, facts, ..
            } => (
                facts.as_ref().unwrap().collection_identity(),
                navigation.as_ref().and_then(|index| match index {
                    Captured::Index(index) => Some(std::sync::Arc::as_ptr(index) as usize),
                    Captured::Json => None,
                }),
            ),
        }
    }
}

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
    pub(super) semantic: SemanticOptions,
    pub(super) specifier_environment:
        Result<specifier_environment::SpecifierEnvironment, InputError>,
}

impl Program {
    /// Freeze all captured spellings and their query membership together.
    pub(super) fn file_lookup(&self) -> FileLookup {
        let mut lookup = FileLookup::new(self.case);
        for id in self.sources.ids() {
            let file = &self.files[id.index()];
            let identity = self.sources.get(id).path.to_str();
            lookup.insert(
                identity.expect("loading names its inputs with text"),
                id,
                file.search,
            );
            for path in &file.paths {
                lookup.insert(path, id, file.search);
            }
        }
        lookup
    }
}

#[derive(Debug)]
pub(super) struct ProgramFile {
    pub(super) module_semantics: FileModuleSemantics,
    /// Whether occurrences in this file belong in results.
    pub(super) search: bool,
    /// Normalized absolute paths through which the file was reached or
    /// may be named under the configuration directory as the caller spelled
    /// it; the identity path is in `Sources`.
    pub(super) paths: Vec<String>,
    pub(super) facts: FileFacts,
    /// Parallel to `facts.requests`.
    pub(super) modules: Vec<ModuleResolution>,
    pub(super) diagnostics: Diagnostics,
    /// What navigation captured, for a searched file only.
    pub(super) navigation: Option<Captured>,
}

/// Loads the program a configuration selects. Errors are `Io` for an
/// unreadable configuration or input (including a missing `files` entry),
/// `Config` for invalid configuration content or inheritance, `Syntax` for an
/// input that does not parse, `Unsupported` outside the supported profile,
/// and `UnresolvedImport` for a missing type library or reference directive
/// target that TypeScript would report. A module request that finds no file
/// is `ModuleResolution::Missing` for the binder to settle against ambient
/// module declarations. Every input parses with `parser`.
pub(super) fn load(
    config: &Path,
    disk: &disk::Disk,
    parser: &Parser,
) -> Result<Program, InputError> {
    load_with_libraries(config, disk, parser, &libraries::directory())
}

pub(super) fn load_reusing(
    config: &Path,
    disk: &disk::Disk,
    parser: &Parser,
    previous: &Reuse,
) -> Result<(Program, Reuse), InputError> {
    let mut next = Reuse::default();
    let program = load_program(
        config,
        disk,
        parser,
        &libraries::directory(),
        Some((previous, &mut next)),
    )?;
    Ok((program, next))
}

/// `load` with the standard library declarations in `library_dir`, the
/// vendored directory except where a test removes data from a copy.
fn load_with_libraries(
    config: &Path,
    disk: &disk::Disk,
    parser: &Parser,
    library_dir: &str,
) -> Result<Program, InputError> {
    load_program(config, disk, parser, library_dir, None)
}

fn load_program(
    config: &Path,
    disk: &disk::Disk,
    parser: &Parser,
    library_dir: &str,
    reuse: Option<(&Reuse, &mut Reuse)>,
) -> Result<Program, InputError> {
    // `Program.GetCurrentDirectory` belongs to the module-spelling host. An
    // absolute configuration and lexical query do not ask that question.
    // Retain a failed observation for generation instead of making unrelated
    // loading fail or substituting another directory.
    let cwd = std::env::current_dir()
        .map_err(|error| io_error(Path::new("."), &error))
        .and_then(|cwd| paths::from_path(&cwd).map(|cwd| paths::normalize(&cwd)));
    let path = std::path::absolute(config).map_err(|error| io_error(config, &error))?;
    let config = config::load(&path, disk, parser)?;
    let case = Case::detect();
    let roots = files::roots(&config, case, disk)?;
    // Inputs are captured ahead of the loader while it reserves them.
    let empty = Reuse::default();
    let (previous, next) = match reuse {
        Some((previous, next)) => (previous, Some(next)),
        None => (&empty, None),
    };
    capture::Captures::with_reuse(&previous.captures, |captures| {
        let (next_captures, next_resolutions) = match next {
            Some(next) => (Some(&mut next.captures), Some(&mut next.resolutions)),
            None => (None, None),
        };
        let resolver = Resolver::new(&config, case, disk);
        let resolver = if next_resolutions.is_some() {
            resolver.reusing(&previous.resolutions)
        } else {
            resolver
        };
        let mut loader = Loader {
            config: &config,
            resolver,
            disk,
            parser,
            case,
            library_dir,
            ids: HashMap::new(),
            queue: VecDeque::new(),
            sources: Sources::default(),
            loaded: Vec::new(),
            edges: Vec::new(),
            search_roots: Vec::new(),
            root_tasks: Vec::new(),
            reached: Vec::new(),
            libraries: Vec::new(),
            navigated: Vec::new(),
            library_ids: HashMap::new(),
            program_only: Vec::new(),
            captures,
            next_captures,
        };
        let loaded = loader.load_all(&roots);
        // An unreadable manifest or type root may have caused another failure.
        loader.resolver.check_reads()?;
        loaded?;
        loader.settle_program_only()?;
        // Provenance is known only now that the dependency graph is complete.
        let search = loader.search();
        let order = loader.program_order();
        let renumbered = loader.sources.reorder(&order);
        let spelled_dir = path.parent().unwrap_or(Path::new("/"));
        let mut loaded: Vec<Option<Loaded>> = loader.loaded.into_iter().map(Some).collect();
        let mut files = Vec::with_capacity(order.len());
        for (id, old) in loader.sources.ids().zip(order) {
            let loaded = loaded[old.index()].take().expect("order is a permutation");
            let search = search[old.index()];
            let origin = if loader.libraries[old.index()].is_some() {
                FileOrigin::DefaultLibrary
            } else if search {
                FileOrigin::Project
            } else {
                FileOrigin::Dependency
            };
            let diagnostics = Diagnostics::new(origin, loaded.checking);
            if let Some(error) = loaded.unsupported {
                if !diagnostics.tolerates_unsupported_input() {
                    return Err(error);
                }
            }
            // The first reported missing reference directive target, in source
            // order.
            let text = &loader.sources.get(id).text;
            if let Some((_, error)) = loaded
                .missing
                .into_iter()
                .find(|&(offset, _)| diagnostics.reports(text, offset))
            {
                return Err(error);
            }
            let mut paths = std::mem::take(&mut loader.reached[old.index()]);
            if let Some(spelled) = spelled_under(spelled_dir, &loader.sources.get(id).display) {
                paths.push(spelled);
            }
            let modules = loaded
                .modules
                .into_iter()
                .map(|module| match module {
                    ModuleResolution::File(target) => {
                        ModuleResolution::File(renumbered[target.index()])
                    }
                    other => other,
                })
                .collect();
            // A searched input keeps its catalog, built now where the guess
            // at its reservation left it out (`capture::Job::navigation`).
            let navigation = match loaded.navigation {
                Some(navigation) if search => Some(navigation),
                None if search => {
                    let source = &loader.sources.get(id);
                    let path = source
                        .path
                        .to_str()
                        .expect("loading names its inputs with text");
                    let ast = loader
                        .parser
                        .parse(source.text.as_str(), path)
                        .map_err(|error| {
                            InputError::unparsable(error, &source.text, &source.path)
                        })?;
                    let navigation = Captured::typescript(&ast, &source.text);
                    // The next generation reuses the catalog with the rest of
                    // the input's capture.
                    if let Some(cache) = loader.next_captures.as_deref_mut() {
                        capture::keep_navigation(cache, path, &navigation);
                    }
                    Some(navigation)
                }
                _ => None,
            };
            files.push(ProgramFile {
                module_semantics: loaded.module_semantics,
                search,
                paths,
                facts: loaded.facts,
                modules,
                diagnostics,
                navigation,
            });
        }
        let specifier_environment = cwd.map(|cwd| {
            specifier_environment::SpecifierEnvironment::capture(
                &config,
                &cwd,
                case,
                disk,
                &mut loader.resolver,
                &loader.sources,
                &files,
            )
        });
        if let Some(next) = next_resolutions {
            *next = loader.resolver.take_reuse();
        }
        Ok(Program {
            semantic: config.options.semantic.clone(),
            preserve_const_enums: config.options.preserve_const_enums,
            config: PathBuf::from(&config.file),
            case,
            sources: loader.sources,
            files,
            specifier_environment,
        })
    })
}

/// A file under the configuration directory, named through that directory
/// as the caller spelled it, such as a symlink or `/tmp` for `/private/tmp`.
/// A file outside it has no such spelling: `..` from a link does not lead
/// where it leads from the real directory that `display` is relative to.
/// Nor does a spelling that is not Unicode: a query that names a file must
/// be exact text (`FileLookup::find`), so no query could use it.
fn spelled_under(spelled_dir: &Path, display: &str) -> Option<String> {
    // `display` is relative to the configuration directory unless it lies
    // on another root, where it is absolute by the platform's rules.
    if !Path::new(display).is_relative() || display.starts_with("../") {
        return None;
    }
    let spelled = paths::from_path(&spelled_dir.join(display)).ok()?;
    Some(paths::normalize(&spelled))
}

/// Where a configuration value or directive was written, for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Origin {
    path: PathBuf,
    position: Option<SourcePosition>,
}

impl Origin {
    fn error(&self, kind: InputErrorKind, message: impl Into<String>) -> InputError {
        InputError {
            kind,
            message: message.into(),
            path: Some(self.path.clone()),
            position: self.position,
        }
    }
}

fn io_error(path: &Path, error: &std::io::Error) -> InputError {
    InputError {
        kind: InputErrorKind::Io,
        message: error.to_string(),
        path: Some(path.to_path_buf()),
        position: None,
    }
}

struct Loader<'a> {
    next_captures: Option<&'a mut capture::Cache>,
    disk: &'a disk::Disk,
    parser: &'a Parser,
    config: &'a Config,
    resolver: Resolver<'a>,
    case: Case,
    /// The vendored standard library declarations.
    library_dir: &'a str,
    /// File identity keys to the ids `Sources` reserved for them.
    ids: HashMap<String, FileId>,
    /// Identity paths awaiting capture, in reservation order: breadth first,
    /// each file's targets in TypeScript's subtask order, as TypeScript's
    /// loader starts the tasks that decide a file's provenance.
    queue: VecDeque<(String, FileId)>,
    sources: Sources,
    /// Per captured file, by id.
    loaded: Vec<Loaded>,
    /// Per captured file: its dependencies, TypeScript's subtasks in its
    /// order, and whether each edge is external.
    edges: Vec<Vec<(FileId, bool)>>,
    /// Roots, local type packages, and root libraries: where TypeScript's
    /// files at depth zero are reached from.
    search_roots: Vec<FileId>,
    /// Roots, root libraries, then type packages: TypeScript's root tasks.
    root_tasks: Vec<FileId>,
    /// Paths through which each id was reached, by id.
    reached: Vec<Vec<String>>,
    /// By id: the standard library a library route first reached it as.
    libraries: Vec<Option<Library>>,
    /// By id: whether its reservation guessed it searched and so captured
    /// its catalog (`capture::Job::navigation`).
    navigated: Vec<bool>,
    /// Each library file's id once captured: `pathForLibFile`'s cache.
    library_ids: HashMap<&'static str, FileId>,
    /// Requests, by importing file and request index, that resolved to a
    /// file the loader does not add for them, with that file's path: a
    /// `.tsx` file without `jsx` (`NotAdmitted::Jsx`), or a module
    /// augmentation's target (`shouldAddFile`). Settled once every input is
    /// known (`settle_program_only`).
    program_only: Vec<(FileId, usize, String)>,
    /// Inputs being captured ahead of `load_file`, as `enqueue_as` reserves
    /// them.
    captures: &'a capture::Captures<'a>,
}

/// One captured input whose provenance is not yet known.
struct Loaded {
    module_semantics: FileModuleSemantics,
    facts: FileFacts,
    modules: Vec<ModuleResolution>,
    checking: Checking,
    /// The reference directives whose targets are missing, by offset in
    /// source order: each an error where it is reported
    /// (`Diagnostics::reports`), which provenance decides.
    missing: Vec<(usize, InputError)>,
    /// The first unsupported reference path target, an error unless the file
    /// turns out to be a dependency.
    unsupported: Option<InputError>,
    /// What navigation captured: nothing for a library, which is never
    /// searched, or for an input its reservation guessed unsearched
    /// (`capture::Job::navigation`), whose catalog the final pass builds if
    /// it is searched after all.
    navigation: Option<Captured>,
}

/// What following one reference directive produced.
enum Followed {
    /// An edge to an input, and whether it crosses into an external library.
    Edge(FileId, bool),
    /// Nothing to follow, such as a library reference under `noLib`.
    Nothing,
    /// No input exists; an error where the directive is reported.
    Missing(InputError),
    /// An input outside the supported profile; an error unless the file turns
    /// out to be a dependency.
    Unsupported(InputError),
}

impl Loader<'_> {
    /// Capture the roots, the standard libraries and type packages they
    /// select, and everything they reach, as `processAllProgramFiles` adds
    /// root tasks.
    fn load_all(&mut self, roots: &[files::Root]) -> Result<(), InputError> {
        for root in roots {
            let id = self.enqueue_as(&root.path, None, true)?;
            self.search_roots.push(id);
            self.root_tasks.push(id);
        }
        for file in libraries::roots(&self.config.options, !roots.is_empty()) {
            let id = self.library(file)?;
            self.search_roots.push(id);
            self.root_tasks.push(id);
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
    fn enqueue(&mut self, path: &str) -> Result<FileId, InputError> {
        self.enqueue_as(path, None, false)
    }

    /// The id of a standard library file, as `pathForLibFile` locates it:
    /// in the vendored directory unless `libReplacement` finds its
    /// `@typescript/lib-*` package, resolved like a `require` from the
    /// configuration's directory (`resolveLibrary`). Replacement files are
    /// still libraries: never searched and never external.
    fn library(&mut self, file: &'static str) -> Result<FileId, InputError> {
        if let Some(&id) = self.library_ids.get(file) {
            return Ok(id);
        }
        let replacement = if libraries::replaceable(&self.config.options, file) {
            self.replacement(file)?
        } else {
            None
        };
        let replaced = replacement.is_some();
        let path = replacement.unwrap_or_else(|| paths::combine(self.library_dir, file));
        let id = self.enqueue_as(&path, Some(Library { file, replaced }), false)?;
        self.library_ids.insert(file, id);
        Ok(id)
    }

    /// The installed `@typescript/lib-*` package replacing `file`, if any.
    fn replacement(&mut self, file: &'static str) -> Result<Option<String>, InputError> {
        let package = libraries::replacement_package(file);
        let from = libraries::replacement_lookup(&self.config.dir, file);
        let resolved =
            self.resolver
                .module(&package, &from, Some(binding::ResolutionMode::Require));
        let reason = match resolved {
            Resolved::File { path, .. } if crate::script_kind(&path) == ScriptKind::Json => {
                format!(
                    "resolves to the JSON file '{}', which is not a library",
                    self.display(&path)
                )
            }
            Resolved::File {
                path,
                original_path,
                ..
            } => return Ok(Some(original_path.unwrap_or(path))),
            Resolved::Missing => return Ok(None),
            Resolved::JavaScript(target) => format!(
                "resolves to the JavaScript file '{}', which has no TypeScript declarations",
                self.display(&target.path)
            ),
            Resolved::TooDeep => format!(
                "follows more than {} package imports redirects",
                resolve::MAX_IMPORTS_CHAIN
            ),
        };
        Err(InputError {
            kind: InputErrorKind::Unsupported,
            message: format!("The standard library replacement '{package}' {reason}"),
            path: Some(PathBuf::from(&self.config.file)),
            position: None,
        })
    }

    /// An input's identity path: its real path unless `preserveSymlinks` is
    /// set.
    fn identity(&self, path: &str) -> Result<String, InputError> {
        if self.config.options.preserve_symlinks {
            return Ok(paths::normalize(path));
        }
        let real = self
            .disk
            .canonicalize(path)
            .map_err(|error| io_error(Path::new(path), &error))?;
        paths::from_path(&real)
    }

    /// `enqueue`, recording whether a library route reached the input
    /// first; the first route decides, as in TypeScript's `collectFiles`.
    /// A `searched_route` is one searching follows from a searched input: a
    /// configuration root, or a reference path of an input guessed searched.
    fn enqueue_as(
        &mut self,
        path: &str,
        library: Option<Library>,
        searched_route: bool,
    ) -> Result<FileId, InputError> {
        let identity = self.identity(path)?;
        let key = self.case.key(&identity);
        let lexical = paths::normalize(path);
        let id = match self.ids.get(&key) {
            Some(&id) => id,
            None => {
                let id = self.sources.reserve();
                self.ids.insert(key, id);
                // A guess, corrected once the program is known: a searched
                // route reaches a searched input, and other routes into a
                // package cross an external-library edge, which searching
                // does not cross, unless some later route reaches it.
                let navigation =
                    library.is_none() && (searched_route || !paths::has_node_modules(&identity));
                self.captures.reserve(capture::Job {
                    id,
                    path: identity.clone(),
                    library: library.is_some(),
                    navigation,
                    // What the name means follows TypeScript, which
                    // classifies the exact spelling, or its script kind
                    // without case; the case policy decides only identity and
                    // support.
                    json: crate::script_kind(&identity) == ScriptKind::Json,
                    options: self.file_options(
                        &identity,
                        paths::is_declaration(&identity),
                        FileFormat::undeclared(),
                    ),
                });
                self.queue.push_back((identity, id));
                self.reached.push(Vec::new());
                self.libraries.push(library);
                self.navigated.push(navigation);
                id
            }
        };
        if !self.reached[id.index()].contains(&lexical) {
            self.reached[id.index()].push(lexical);
        }
        Ok(id)
    }

    /// The `types` entries, expanding `*` to every package in the type roots.
    fn type_packages(&mut self) -> Result<(), InputError> {
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
                    InputErrorKind::UnresolvedImport,
                    format!("Cannot find type definition file for '{name}'"),
                ));
            };
            let id = self.enqueue(found.reached_path())?;
            self.root_tasks.push(id);
            if !found.external {
                self.search_roots.push(id);
            }
        }
        Ok(())
    }

    fn display(&self, path: &str) -> String {
        paths::relative(&self.config.dir, path)
    }

    /// Integrates one freshly read input and its capture products, reused or
    /// computed ahead of this or now. Settles its collection's module key
    /// before resolving generation-local edges.
    fn load_file(&mut self, path: String, id: FileId) -> Result<(), InputError> {
        let file_path = PathBuf::from(&path);
        let capture = self.captures.take(id, self.parser);
        let text = self
            .disk
            .receive(capture.read)
            .map_err(|error| io_error(&file_path, &error))?;
        let parsed = capture.parsed.expect("a successful read is parsed");
        let source = parsed.source;
        let library = self.libraries[id.index()].is_some();
        let json = capture.json;
        let format = if library {
            FileFormat::library()
        } else if json {
            FileFormat::json()
        } else {
            self.resolver.file_format(&path)
        };
        let declaration = paths::is_declaration(&path);
        let module_semantics = self.resolver.module_semantics(format, json);
        let jsx = crate::ParseOptions::for_file(path.as_str()).jsx;
        let (navigation, mut facts, collected_module) = match parsed.contents? {
            capture::Contents::Json(facts) => (Some(Captured::Json), facts, None),
            capture::Contents::TypeScript {
                navigation,
                indicators,
                module: captured,
                facts,
            } => {
                let module = indicators.module(self.file_options(&path, declaration, format));
                let facts = if module == captured {
                    facts?
                } else {
                    // The format forces module semantics the capture did not
                    // assume: collect the same text again, with them, in
                    // place of the facts collected without them.
                    drop(facts);
                    let ast = self
                        .parser
                        .parse(source.as_str(), &path)
                        .map_err(|error| InputError::unparsable(error, &source, &file_path))?;
                    let indicators = binding::ModuleIndicators::of(&ast);
                    capture::collect(&ast, &source, indicators, module, &file_path)?
                };
                (navigation, facts, Some((indicators, module)))
            }
        };
        if let Some(cache) = &mut self.next_captures {
            let contents = match collected_module {
                Some((indicators, module)) => capture::Contents::TypeScript {
                    navigation: navigation.clone(),
                    indicators,
                    module,
                    facts: Ok(facts.clone()),
                },
                None => capture::Contents::Json(facts.clone()),
            };
            cache.insert(
                path.clone(),
                capture::Cached {
                    text,
                    library,
                    json,
                    parsed: capture::Parsed {
                        source: source.clone(),
                        contents: Ok(contents),
                    },
                },
            );
        }
        // `resolveImportsAndModuleAugmentations` resolves synthetic helpers
        // before JSX and written requests. Helpers are loaded even when no
        // emitted construct needs one; missing helpers are demanded only by
        // checking, never diagnosed just for this synthetic program edge.
        if !declaration
            && self.config.options.semantic.import_helpers
            && (self.config.options.semantic.isolated_modules || !json && facts.module)
        {
            facts.requests.push(ModuleRequest {
                specifier: "tslib".into(),
                kind: RequestKind::Import { side_effect: true },
                mode: None,
                span: crate::Span { start: 0, end: 0 },
                implicit: Some(binding::ImplicitImport::Helpers),
                emit_mode: None,
                resolution_mode: None,
                resolved_path: None,
            });
        }
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
                implicit: Some(binding::ImplicitImport::JsxRuntime),
                emit_mode: None,
                resolution_mode: None,
                resolved_path: None,
            });
        }
        let options = &self.config.options;
        for request in &mut facts.requests {
            request.emit_mode =
                module_semantics.emit_syntax(options.semantic.module_kind, request.kind);
            request.resolution_mode = module_semantics.resolution_mode(
                options.semantic.module_kind,
                request.kind,
                request.mode,
            );
        }
        let unchecked = Unchecked {
            no_check: options.no_check,
            declaration,
            skip_lib_check: options.skip_lib_check,
            library,
            skip_default_lib_check: options.skip_default_lib_check,
            json,
            nocheck: !json && crate::pragmas::checks(source.as_str()) == Some(false),
        };
        let checking = Checking::new(unchecked, &source, &facts);
        let runtime_request = runtime.map(|_| facts.requests.len() - 1);
        // TypeScript's subtask order: `/// <reference path>`s, then `types`
        // and `lib` references, then module names: helpers, JSX, then each
        // request by its `subtask_rank` and, within a rank, by its position,
        // since binding collects requests in no particular order. Following
        // them in this order keeps the queue's reservation order
        // TypeScript's (see `queue`).
        let mut unsupported: Option<(usize, InputError)> = None;
        let mut missing: Vec<(usize, InputError)> = Vec::new();
        let mut targets = Vec::new();
        let mut references = directives::scan(source.as_str());
        references.sort_by_key(|reference| match reference.directive {
            Directive::Path(_) => 0,
            Directive::Types(..) => 1,
            Directive::Lib(_) => 2,
        });
        // Failures are reported for the first directive in source order.
        let first = |kept: &mut Option<(usize, InputError)>, offset, error| {
            if kept.as_ref().is_none_or(|(at, _)| offset < *at) {
                *kept = Some((offset, error));
            }
        };
        for reference in references {
            let origin = Origin {
                path: file_path.clone(),
                position: Some(source.position(reference.offset)),
            };
            match self.directive(id, &path, format, reference.directive, &origin)? {
                Followed::Edge(target, external) => targets.push((target, external)),
                Followed::Nothing => {}
                // Settled with provenance (`Diagnostics::reports`).
                Followed::Missing(error) => missing.push((reference.offset, error)),
                // Settled with provenance, as an unsupported module target is.
                Followed::Unsupported(error) => first(&mut unsupported, reference.offset, error),
            }
        }
        missing.sort_by_key(|&(offset, _)| offset);
        let unsupported = unsupported.map(|(_, error)| error);
        let mut requests: Vec<usize> = (0..facts.requests.len()).collect();
        requests.sort_by_key(|&index| match facts.requests[index].implicit {
            Some(binding::ImplicitImport::Helpers) => (0, 0),
            Some(binding::ImplicitImport::JsxRuntime) => (1, 0),
            None => (
                2 + facts.requests[index].kind.subtask_rank(),
                facts.requests[index].span.start,
            ),
        });
        let mut modules = vec![ModuleResolution::Missing; facts.requests.len()];
        for index in requests {
            let request = &facts.requests[index];
            let mode = request.resolution_mode;
            // A request that reaches no program input: an ambient module may
            // still satisfy it, and otherwise `not_admitted` says why
            // resolution's file does not count.
            let unresolved = |not_admitted: Option<String>| {
                if request.implicit == Some(binding::ImplicitImport::Helpers) {
                    return ModuleResolution::Unchecked;
                }
                if runtime_request == Some(index) {
                    // TypeScript requires the runtime only where JSX is used.
                    if facts.jsx.is_none() {
                        return ModuleResolution::Unchecked;
                    }
                } else if request.kind == (RequestKind::Import { side_effect: true })
                    && !self.config.options.check_side_effect_imports
                {
                    return ModuleResolution::Unchecked;
                }
                not_admitted.map_or(ModuleResolution::Missing, ModuleResolution::NotAdmitted)
            };
            // TypeScript's loader resolves no empty module name.
            let resolved = if request.specifier.is_empty() {
                Resolved::Missing
            } else {
                self.resolver.module(&request.specifier, &path, mode)
            };
            let resolved_path = match &resolved {
                Resolved::File { path, .. } => Some(path.clone()),
                Resolved::JavaScript(resolved) => Some(resolved.path.clone()),
                Resolved::Missing | Resolved::TooDeep => None,
            };
            modules[index] = match resolved {
                Resolved::File {
                    path: target,
                    original_path,
                    external,
                    extension,
                } => match resolve::admission(&self.config.options, extension, declaration) {
                    // TypeScript's loader adds no file for a module
                    // augmentation's name (`shouldAddFile`): its target is
                    // a program input only by another route.
                    None if matches!(request.kind, RequestKind::Augmentation { .. }) => {
                        self.program_only.push((id, index, target));
                        ModuleResolution::Missing
                    }
                    None => {
                        let target = self.enqueue(original_path.as_deref().unwrap_or(&target))?;
                        targets.push((target, external));
                        ModuleResolution::File(target)
                    }
                    Some(reason) => {
                        let message = reason.message(&request.specifier, &self.display(&target));
                        let resolution = unresolved(Some(message));
                        if reason == NotAdmitted::Jsx
                            && matches!(resolution, ModuleResolution::NotAdmitted(_))
                        {
                            self.program_only.push((id, index, target));
                        }
                        resolution
                    }
                },
                Resolved::Missing => unresolved(None),
                Resolved::JavaScript(target) => ModuleResolution::Unsupported(format!(
                    "'{}' resolves to the JavaScript file '{}', which has no TypeScript declarations",
                    request.specifier,
                    self.display(&target.path)
                )),
                // A limit of this tool, not an unsupported input: no file's
                // diagnostic policy may tolerate it.
                Resolved::TooDeep => {
                    return Err(InputError {
                        kind: InputErrorKind::Unsupported,
                        message: format!(
                            "'{}' follows more than {} package imports redirects",
                            request.specifier,
                            resolve::MAX_IMPORTS_CHAIN
                        ),
                        path: Some(file_path.clone()),
                        position: Some(source.position(request.span.start)),
                    });
                }
            };
            facts.requests[index].resolved_path = resolved_path;
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
            module_semantics,
            facts,
            modules,
            checking,
            missing,
            unsupported,
            navigation,
        });
        self.edges.push(targets);
        Ok(())
    }

    /// TypeScript's checker resolves a request to a file the loader did not
    /// add for it when the program has that file by another route, such as a
    /// root (`resolveExternalModule`): an import whose `.tsx` file lacks
    /// `jsx` (TS6142), and a module augmentation's target. Otherwise the
    /// request stays unresolved, which for an augmentation is TS2664 where
    /// reported.
    fn settle_program_only(&mut self) -> Result<(), InputError> {
        for (file, index, target) in std::mem::take(&mut self.program_only) {
            let key = self.case.key(&self.identity(&target)?);
            if let Some(&id) = self.ids.get(&key) {
                self.loaded[file.index()].modules[index] = ModuleResolution::File(id);
            }
        }
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
        importer: FileId,
        path: &str,
        format: FileFormat,
        directive: Directive,
        origin: &Origin,
    ) -> Result<Followed, InputError> {
        let config = self.config;
        let options = &config.options;
        match directive {
            Directive::Lib(name) => match libraries::reference(options, &name) {
                libraries::Reference::Ignored => Ok(Followed::Nothing),
                libraries::Reference::File(file) => Ok(Followed::Edge(self.library(file)?, false)),
                libraries::Reference::Unknown(lower) => Ok(Followed::Missing(origin.error(
                    InputErrorKind::UnresolvedImport,
                    format!("Cannot find lib definition for '{lower}'"),
                ))),
            },
            Directive::Types(name, mode) => {
                let mode = mode.or_else(|| self.resolver.type_directive_mode(format));
                let Some(found) = self
                    .resolver
                    .type_reference(&name, paths::parent(path), mode)
                else {
                    return Ok(Followed::Missing(origin.error(
                        InputErrorKind::UnresolvedImport,
                        format!("Cannot find type definition file for '{name}'"),
                    )));
                };
                let id = self.enqueue(found.reached_path())?;
                Ok(Followed::Edge(id, found.external))
            }
            Directive::Path(reference) => {
                let target = paths::resolve(paths::parent(path), &reference);
                let unsupported = |reason: String| {
                    Ok(Followed::Unsupported(origin.error(
                        InputErrorKind::Unsupported,
                        format!("Referenced file '{reference}' {reason}"),
                    )))
                };
                match files::literal_input(
                    &target,
                    options.resolve_json_module,
                    self.case,
                    self.disk,
                )? {
                    files::Literal::Unsupported(kind) => unsupported(format!(
                        "is a {} input, which is not supported",
                        kind.label()
                    )),
                    files::Literal::Found(found) => {
                        // A reference path is never an external edge.
                        let navigated = self.navigated[importer.index()];
                        Ok(Followed::Edge(
                            self.enqueue_as(&found, None, navigated)?,
                            false,
                        ))
                    }
                    files::Literal::Missing => Ok(Followed::Missing(origin.error(
                        InputErrorKind::UnresolvedImport,
                        format!("File '{reference}' not found"),
                    ))),
                    files::Literal::UnsupportedExtension => {
                        unsupported("has an unsupported extension".into())
                    }
                }
            }
        }
    }

    /// `GetSourceFiles`: TypeScript's `collectFiles` walks the root tasks
    /// depth first and lists each file after the files its subtasks reach;
    /// the libraries then come first, ordered by `getDefaultLibFilePriority`
    /// (a stable sort, where TypeScript's is not; see docs/references.md).
    fn program_order(&self) -> Vec<FileId> {
        let mut walked = vec![false; self.loaded.len()];
        let mut order = Vec::with_capacity(self.loaded.len());
        let mut walk = |start: FileId, order: &mut Vec<FileId>| {
            if std::mem::replace(&mut walked[start.index()], true) {
                return;
            }
            let mut stack = vec![(start, 0)];
            while let Some((id, next)) = stack.last_mut() {
                let id = *id;
                match self.edges[id.index()].get(*next) {
                    Some(&(target, _)) => {
                        *next += 1;
                        if !std::mem::replace(&mut walked[target.index()], true) {
                            stack.push((target, 0));
                        }
                    }
                    None => {
                        stack.pop();
                        order.push(id);
                    }
                }
            }
        };
        for &root in &self.root_tasks {
            walk(root, &mut order);
        }
        debug_assert_eq!(order.len(), self.loaded.len(), "every input is walked");
        let (mut libraries, files): (Vec<FileId>, Vec<FileId>) = order
            .into_iter()
            .partition(|id| self.libraries[id.index()].is_some());
        libraries.sort_by_key(|id| {
            self.libraries[id.index()]
                .expect("partitioned as a library")
                .priority()
        });
        libraries.extend(files);
        libraries
    }

    /// Inputs TypeScript finds at depth zero, which are reachable from the
    /// roots, local type packages, and root libraries without crossing an
    /// external-library edge, less the standard libraries themselves.
    fn search(&self) -> Vec<bool> {
        let mut reached = vec![false; self.loaded.len()];
        let mut stack = self.search_roots.clone();
        while let Some(id) = stack.pop() {
            if std::mem::replace(&mut reached[id.index()], true) {
                continue;
            }
            for &(target, external) in &self.edges[id.index()] {
                if !external && !reached[target.index()] {
                    stack.push(target);
                }
            }
        }
        reached
            .into_iter()
            .zip(&self.libraries)
            .map(|(reached, library)| reached && library.is_none())
            .collect()
    }
}
