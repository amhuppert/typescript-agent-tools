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
mod compatibility;
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
/// The compiler a test project installs, shared with the integration tests'
/// `support`.
#[cfg(test)]
#[path = "../../../tests/support/compiler.rs"]
mod test_compiler;
#[cfg(test)]
mod tests;
mod version;

use super::binding::{self, FileFacts, FileOptions, ModuleRequest, ModuleResolution, RequestKind};
use super::navigation::Captured;
use super::sources::{FileId, SourceFile, Sources};
use crate::ScriptKind;
use crate::analysis::Parser;
use crate::analysis::{InputError, InputErrorKind};
use crate::checking::Code;
use crate::input::Case;
use crate::source::SourcePosition;
use config::{Config, Detection};
use diagnostics::{Checking, FileOrigin, Unchecked};
use directives::Directive;
use libraries::Library;
use resolve::{
    Admission, FileFormat, NotAdmitted, PackageId, Resolved, Resolver, TypeReferenceOrigin,
};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

pub(super) use config::Resolution as ModuleResolutionKind;
pub(super) use config::SemanticOptions;
pub(super) use diagnostics::Diagnostics;
pub(super) use module_semantics::{FileModuleSemantics, ModuleKind};
pub(super) use paths::FileLookup;
pub(super) use paths::{from_path as native_file_name, remove_file_extension};
pub(super) use resolve::is_relative_name;
pub(super) use resolve::{untyped_augmentation_message, untyped_message};

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
    /// The files collection met as duplicates of a package another input
    /// already is, in the order it met them (`Redirect`).
    pub(super) redirects: Vec<Redirect>,
    /// The effective options loading read, which tests inspect beside the
    /// program they select.
    #[cfg(test)]
    options: config::Options,
}

/// A file that is a copy of a package another program input already is:
/// both have one package id, and TypeScript's `collectFiles` keeps the copy
/// it meets first, depth first, as the program's input (the target) and
/// redirects the other to it (`redirectTargetsMap`, `redirectFilesByPath`).
/// The redirected file is no input of its own: a request resolving to it
/// reaches the target, and what only it reaches leaves the program. Its text
/// was still read and parsed, so that a session sees it change.
#[derive(Debug)]
pub(super) struct Redirect {
    /// The file's identity path, as an input of its own would have it.
    path: PathBuf,
    /// Normalized absolute paths through which it was reached, its identity
    /// included, each of which names the target instead.
    spellings: Vec<String>,
    /// The input the file redirects to.
    target: FileId,
    /// The package id the two files share.
    package: PackageId,
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
        for redirect in &self.redirects {
            let reason = format!(
                "a duplicate copy of package '{}', which the program reads from '{}' instead (TypeScript's package deduplication); name that file",
                redirect.package,
                self.sources.get(redirect.target).display
            );
            for spelling in &redirect.spellings {
                lookup.insert_duplicate(spelling, reason.clone());
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
    load_program(config, disk, parser, None, None)
}

pub(super) fn load_reusing(
    config: &Path,
    disk: &disk::Disk,
    parser: &Parser,
    previous: &Reuse,
) -> Result<(Program, Reuse), InputError> {
    let mut next = Reuse::default();
    let program = load_program(config, disk, parser, None, Some((previous, &mut next)))?;
    Ok((program, next))
}

/// What the doctor reports of the compiler a configuration's project is
/// read for (`compatibility::CompilerSelection::describe`): the selection
/// loading makes, through `disk`, from the configuration's canonical
/// directory, which reads neither the configuration's contents nor any
/// input.
pub(super) fn compatibility(config: &Path, disk: &disk::Disk) -> Result<String, InputError> {
    let path = std::path::absolute(config).map_err(|error| io_error(config, &error))?;
    let selection = compatibility::select(&config::directory(&path, disk)?, disk)?;
    selection.describe(version::REFERENCE, disk)
}

/// `load` with the standard library declarations in `library_dir` in place
/// of the dialect's directory, for a test that removes data from a copy of
/// the vendored declarations.
#[cfg(test)]
fn load_with_libraries(
    config: &Path,
    disk: &disk::Disk,
    parser: &Parser,
    library_dir: &str,
) -> Result<Program, InputError> {
    load_program(config, disk, parser, Some(library_dir), None)
}

fn load_program(
    config: &Path,
    disk: &disk::Disk,
    parser: &Parser,
    library_dir: Option<&str>,
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
    // Where the dialect's compiler reads its standard libraries.
    let compiler = &config.options.compiler;
    let library_dir = match library_dir {
        Some(directory) => directory.to_owned(),
        None => compiler
            .dialect()
            .libraries()
            .directory(compiler.package_directory(), disk)?,
    };
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
            library_dir: &library_dir,
            ids: HashMap::new(),
            queue: VecDeque::new(),
            sources: Sources::default(),
            loaded: Vec::new(),
            edges: Vec::new(),
            root_tasks: Vec::new(),
            reached: Vec::new(),
            libraries: Vec::new(),
            navigated: Vec::new(),
            depths: Vec::new(),
            library_ids: HashMap::new(),
            program_only: Vec::new(),
            captures,
            next_captures,
            packages: Vec::new(),
            program_only_fallbacks: HashMap::new(),
            borrowed: HashMap::new(),
            borrowed_failures: Vec::new(),
        };
        // A lookup failure is the failure of the root tasks or of the file
        // that met it (`Loader::load_all`).
        loader.load_all(&roots)?;
        loader.settle_program_only();
        // Each redirect collection processes again restarts the walk once
        // its borrowed subtasks are loaded (`Loader::borrow`): a walk per such
        // redirect, which projects rarely have.
        let collected = loop {
            match loader.program_order() {
                Collection::Collected(collected) => break collected,
                Collection::Borrows { redirect, target } => loader.borrow(redirect, target)?,
            }
        };
        loader.finish_program_only();
        // The first input in load order whose capture failed and that stays
        // in the program fails the load (`Loader::failed`).
        let mut kept = vec![false; loader.loaded.len()];
        for old in &collected.order {
            kept[old.index()] = true;
        }
        if let Some(error) = loader
            .loaded
            .iter_mut()
            .zip(&kept)
            .find_map(|(loaded, kept)| loaded.failure.take().filter(|_| *kept))
        {
            return Err(error);
        }
        loader.borrowed_failure()?;
        // Provenance is known only now that the program is collected.
        let search = loader.search(&collected);
        // A dependency's JavaScript with a construct this tool refuses is no
        // input, so that none of its declarations is bound; a request that
        // names it is unsupported, which only a project file's request
        // reports (docs/references.md, JavaScript inputs).
        let javascript = config.options.compiler.dialect().javascript();
        let refused: Vec<Option<String>> = loader
            .sources
            .ids()
            .zip(&loader.loaded)
            .map(|(old, loaded)| {
                let refusal = loaded.facts.javascript_refusal(javascript)?;
                let source = loader.sources.get(old);
                let position = source.text.position(refusal.span.start);
                (!search[old.index()] && loader.libraries[old.index()].is_none()).then(|| {
                    format!(
                        "{}:{}:{}: {}",
                        source.display,
                        position.line,
                        position.column,
                        refusal.message()
                    )
                })
            })
            .collect();
        let spelled_dir = path.parent().unwrap_or(Path::new("/"));
        let Collected {
            mut order,
            redirects,
            ..
        } = collected;
        // What names a redirected file names the input it redirects to
        // (`filesByPath`), and the redirect's own spellings name it.
        let mut redirected: Vec<Option<FileId>> = vec![None; loader.loaded.len()];
        let mut redirect_records = Vec::with_capacity(redirects.len());
        for Redirected {
            file: redirect,
            target,
            package,
            ..
        } in redirects
        {
            redirected[redirect.index()] = Some(target);
            let source = loader.sources.get(redirect);
            let mut spellings = std::mem::take(&mut loader.reached[redirect.index()]);
            let identity = paths::from_path(&source.path)?;
            if !spellings.contains(&identity) {
                spellings.insert(0, identity);
            }
            if let Some(spelled) = spelled_under(spelled_dir, &source.display)
                && !spellings.contains(&spelled)
            {
                spellings.push(spelled);
            }
            redirect_records.push((
                Redirect {
                    path: source.path.clone(),
                    spellings,
                    target,
                    package,
                },
                target,
            ));
        }
        order.retain(|old| refused[old.index()].is_none());
        let renumbered = loader.sources.reorder(&order);
        let redirects: Vec<Redirect> = redirect_records
            .into_iter()
            .filter_map(|(redirect, target)| {
                Some(Redirect {
                    target: renumbered[target.index()]?,
                    ..redirect
                })
            })
            .collect();
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
            let source = loader.sources.get(id);
            let text = &source.text;
            // TypeScript reports a project file's syntactic diagnostics
            // before any other, the earliest first: the dialect's first error
            // of TypeScript-only syntax, or a parameter decorator's TS1206
            // where that is syntactic, as the generation's dialect decides
            // afresh for captured candidates.
            let policy = config.options.compiler.dialect().source_diagnostics();
            let syntax = policy
                .javascript_syntax(loaded.facts.javascript_syntax())
                .filter(|_| diagnostics.reports_syntax())
                .map(|error| (error.span.start, error.message.clone()));
            let decorator = (!config.options.semantic.experimental_decorators)
                .then(|| {
                    diagnostics.reported_parameter_decorator(
                        text,
                        loaded.facts.parameter_decorators(),
                        policy,
                    )
                })
                .flatten();
            let decorator_message = || crate::javascript_syntax::DECORATORS_NOT_VALID.to_owned();
            let first = match (syntax, decorator) {
                (Some(syntax), Some(decorator))
                    if decorator.syntactic && decorator.offset < syntax.0 =>
                {
                    Some((decorator.offset, decorator_message()))
                }
                (Some(syntax), _) => Some(syntax),
                (None, Some(decorator)) => Some((decorator.offset, decorator_message())),
                (None, None) => None,
            };
            if let Some((offset, message)) = first {
                return Err(InputError {
                    kind: InputErrorKind::Syntax,
                    message,
                    path: Some(source.path.clone()),
                    position: Some(text.position(offset)),
                });
            }
            // The first reported error of the file's syntax, in source order,
            // fails as a syntax error. Which candidates are errors is the
            // generation's dialect's, decided afresh for captured facts.
            if let Some(error) = loaded
                .facts
                .source_candidates()
                .iter()
                .filter(|candidate| policy.is_error(candidate))
                .map(|candidate| &candidate.diagnostic)
                .find(|error| diagnostics.reports(text, error.span.start, Code::MODULE_KEYWORD))
            {
                return Err(InputError {
                    kind: InputErrorKind::Syntax,
                    message: error.message.clone(),
                    path: Some(source.path.clone()),
                    position: Some(text.position(error.span.start)),
                });
            }
            // A project file's JavaScript this tool refuses fails at the
            // construct, whatever checks the file: a capability limit, not a
            // diagnostic.
            if let Some(refusal) = loaded.facts.javascript_refusal(javascript)
                && !diagnostics.tolerates_unsupported_input()
            {
                return Err(InputError {
                    kind: InputErrorKind::Unsupported,
                    message: refusal.message(),
                    path: Some(source.path.clone()),
                    position: Some(text.position(refusal.span.start)),
                });
            }
            if let Some(error) = loaded.unsupported {
                if !diagnostics.tolerates_unsupported_input() {
                    return Err(error);
                }
            }
            // The first reported missing reference directive target, in source
            // order.
            if let Some((_, error)) = loaded
                .missing
                .into_iter()
                .find(|&(offset, _)| diagnostics.reports_inclusion(text, offset))
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
                .enumerate()
                .map(|(request, module)| match module {
                    ModuleResolution::File(target) => {
                        let target = redirected[target.index()].unwrap_or(target);
                        match (renumbered[target.index()], &refused[target.index()]) {
                            (Some(target), _) => ModuleResolution::File(target),
                            (None, Some(refusal)) => ModuleResolution::Unsupported(refusal.clone()),
                            // A file only a redirected file reached, which a
                            // request names by a route that adds no file for
                            // it, is no input after all.
                            (None, None) => loader
                                .program_only_fallbacks
                                .remove(&(old, request))
                                .expect("an input left out is refused, or redirected away"),
                        }
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
                specifier_environment::Inputs {
                    sources: &loader.sources,
                    files: &files,
                    redirects: &redirects,
                },
            )
        });
        if let Some(next) = next_resolutions {
            *next = loader.resolver.take_reuse();
        }
        Ok(Program {
            #[cfg(test)]
            options: config.options.clone(),
            semantic: config.options.semantic.clone(),
            preserve_const_enums: config.options.preserve_const_enums,
            config: PathBuf::from(&config.file),
            case,
            sources: loader.sources,
            files,
            specifier_environment,
            redirects,
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
    /// The standard library declarations of the dialect's compiler
    /// (`libraries::Catalog::directory`).
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
    /// order.
    edges: Vec<Vec<Edge>>,
    /// Roots, then root libraries and type packages in the order the
    /// catalog adds them (`libraries::Catalog::tasks`): TypeScript's root
    /// tasks. Those at depth zero are where its files at depth zero are
    /// reached from.
    root_tasks: Vec<RootTask>,
    /// Paths through which each id was reached, by id.
    reached: Vec<Vec<String>>,
    /// By id: the standard library a library route first reached it as.
    libraries: Vec<Option<Library>>,
    /// By id: whether its reservation guessed it searched and so captured
    /// its catalog (`capture::Job::navigation`).
    navigated: Vec<bool>,
    /// Each library file's id once captured, and how `pathForLibFile`
    /// located it: its cache.
    library_ids: HashMap<&'static str, (FileId, Library, Option<InputError>)>,
    /// By id: the fewest package lookups any route so far took to reach
    /// it, TypeScript's `lowestDepth`; its own subtasks are one deeper per
    /// lookup when it is loaded (`filesParser.start`).
    depths: Vec<usize>,
    /// Requests that resolved to a file the loader does not add for them,
    /// in the order they were met; settled once every input is known
    /// (`settle_program_only`).
    program_only: Vec<ProgramOnly>,
    /// Inputs being captured ahead of `load_file`, as `enqueue_as` reserves
    /// them.
    captures: &'a capture::Captures<'a>,
    /// By id: the first package id a route to it gave (`record_package`).
    packages: Vec<Option<PackageId>>,
    /// What a request `settle_program_only` resolved to a file another
    /// route makes an input means if that file leaves the program after
    /// all, by importer and request.
    program_only_fallbacks: HashMap<(FileId, usize), ModuleResolution>,
    /// By redirect that collection processes again: its target's
    /// subtasks, followed from the redirect's location (`borrow`).
    borrowed: HashMap<FileId, Vec<Edge>>,
    /// What following those subtasks met that a project file reports, by
    /// redirect, with its target (`borrow`).
    borrowed_failures: Vec<BorrowedFailures>,
}

/// The missing and unsupported reference targets that following a
/// target's subtasks from its redirect's location met (`Loader::borrow`).
struct BorrowedFailures {
    target: FileId,
    missing: Vec<(usize, InputError)>,
    unsupported: Option<InputError>,
}

/// A request that resolved to a file the loader does not add for it: a
/// `.tsx` or `.jsx` file without `jsx` (`NotAdmitted::Jsx`), a module
/// augmentation's target (`shouldAddFile`), or JavaScript that is not an
/// input on this route. TypeScript's checker names the file where another
/// route makes it an input (`resolveExternalModule`).
struct ProgramOnly {
    importer: FileId,
    request: usize,
    /// The resolved file's path, or for an elided subtask the path its
    /// route would load it by.
    target: String,
    /// The request's resolution where no route makes `target` an input.
    otherwise: ModuleResolution,
    /// For JavaScript elided at its depth on this route, the importer's
    /// subtask, by its position among the importer's edges: TypeScript
    /// keeps that subtask, which a shallower route may load later, and
    /// `collectFiles` then lists the file where this route reached it.
    subtask: Option<usize>,
    /// The package id the request's resolution gave the file.
    package: Option<PackageId>,
}

/// One of TypeScript's root tasks (`processAllProgramFiles`): a root, a
/// root library, or a type package.
struct RootTask {
    file: FileId,
    route: libraries::Route,
    /// The package id the task gives its file.
    package: Option<PackageId>,
    /// The package lookups the task took (`currentNodeModulesDepth`): one
    /// for a type package found in `node_modules`, none otherwise.
    depth: usize,
}

/// One captured input whose provenance is not yet known.
struct Loaded {
    /// The depth its subtasks first started from.
    depth: usize,
    /// Its module format (`impliedNodeFormat`).
    format: FileFormat,
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
    /// What capturing the input met instead, when it failed: an input that
    /// cannot be read, does not parse, or names a target loading cannot
    /// follow keeps no facts or edges, and fails the load only if it stays
    /// in the program (`Loader::failed`).
    failure: Option<InputError>,
}

/// One dependency of a captured file: a TypeScript subtask.
#[derive(Debug, Clone)]
struct Edge {
    target: FileId,
    /// Whether it crosses into an external library.
    external: bool,
    /// What kind of reference or import it is.
    route: libraries::Route,
    /// The package id the route's resolution gave the target.
    package: Option<PackageId>,
}

/// What walking the program gives (`Loader::program_order`).
enum Collection {
    Collected(Collected),
    /// The walk processes `redirect` again, whose target's subtasks from
    /// its location are not loaded yet (`Loader::borrow`).
    Borrows {
        redirect: FileId,
        target: FileId,
    },
}

/// What `collectFiles` makes of the walked program (`Loader::program_order`).
struct Collected {
    /// The program's inputs in program order.
    order: Vec<FileId>,
    /// Each file met as a duplicate of a package, in the order collection
    /// met them.
    redirects: Vec<Redirected>,
    /// By file: whether collection met it, last, without a package lookup,
    /// where its processing clears 5.6.3's
    /// `sourceFilesFoundSearchingNodeModules`.
    at_depth_zero: Vec<bool>,
}

/// A file collection met as a duplicate of a package.
struct Redirected {
    file: FileId,
    /// The input collection met first with the same package id.
    target: FileId,
    package: PackageId,
}

/// Collection's state while it walks the program (`Loader::program_order`).
struct Walk {
    catalog: libraries::Catalog,
    /// `resolve::Rules::first_route_package_id`.
    first_route: bool,
    /// `resolve::Rules::collected_provenance`: whether a route at depth
    /// zero processes again a file a package lookup met.
    reprocesses: bool,
    deduplicate: bool,
    /// By file: whether the walk has met it.
    walked: Vec<bool>,
    /// By file: whether a package lookup met it and no route at depth zero
    /// has since (`sourceFilesFoundSearchingNodeModules`).
    in_packages: Vec<bool>,
    placements: Vec<Option<libraries::Placement>>,
    order: Vec<FileId>,
    primaries: HashMap<PackageId, FileId>,
    redirects: Vec<Redirected>,
    /// Each redirect's target.
    redirected: HashMap<FileId, FileId>,
    /// The files whose subtasks the walk is following, innermost last.
    frames: Vec<Frame>,
}

/// A file whose subtasks the walk is following.
struct Frame {
    file: FileId,
    /// Whether they are a redirect's borrowed subtasks (`Loader::borrow`)
    /// rather than its own.
    borrowed: bool,
    /// The next subtask to follow.
    next: usize,
    /// The package lookups on the route that met the file.
    depth: usize,
    /// Whether the file is listed once its subtasks are followed: not when
    /// it is processed again.
    listed: bool,
}

/// What following one file's references and imports gives
/// (`Loader::subtasks`).
struct Subtasks {
    /// The inputs they add, in TypeScript's subtask order.
    edges: Vec<Edge>,
    /// What each module request names.
    modules: Vec<ModuleResolution>,
    /// The reference directives whose targets are missing, in source order.
    missing: Vec<(usize, InputError)>,
    /// The first unsupported reference path target.
    unsupported: Option<InputError>,
    /// Requests whose resolved file this route does not add.
    program_only: Vec<ProgramOnly>,
}

/// What following one reference directive produced.
enum Followed {
    /// An edge to an input, whether it crosses into an external library, its
    /// route, and the package id the route gives the input.
    Edge(FileId, bool, libraries::Route, Option<PackageId>),
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
        // The root tasks are the program's whatever collection keeps: a
        // failure their lookups meet fails the load, the first met first.
        let tasks = self.root_tasks(roots);
        self.resolver.check_reads()?;
        tasks?;
        self.load_queued();
        Ok(())
    }

    /// Capture each queued input and what it reaches; an input that fails
    /// is kept as such (`failed`).
    fn load_queued(&mut self) {
        while let Some((path, id)) = self.queue.pop_front() {
            if let Err(error) = self.load_file(path.clone(), id) {
                // A lookup failure the file met before this one is its first.
                let error = match self.resolver.check_reads() {
                    Err(earlier) => earlier,
                    Ok(()) => error,
                };
                self.failed(id, path, error);
            }
        }
    }

    /// Reserve the roots, the root libraries, and the type packages, in the
    /// order `processAllProgramFiles` adds root tasks.
    fn root_tasks(&mut self, roots: &[files::Root]) -> Result<(), InputError> {
        for root in roots {
            let id = self.enqueue_as(&root.path, None, true, 0)?;
            self.root_tasks.push(RootTask {
                file: id,
                route: libraries::Route::Other,
                package: None,
                depth: 0,
            });
        }
        let catalog = self.config.options.compiler.dialect().libraries();
        for task in catalog.tasks(&self.config.options, !roots.is_empty()) {
            match task {
                libraries::Task::Library(request) => {
                    let (id, library) = self.library(request)?;
                    self.root_tasks.push(RootTask {
                        file: id,
                        route: libraries::Route::Library(library),
                        package: None,
                        depth: 0,
                    });
                }
                libraries::Task::TypePackages => self.type_packages()?,
            }
        }
        Ok(())
    }

    /// The id of an input, queueing it on first sight, reached `depth`
    /// package lookups deep. Identity is the real path unless
    /// `preserveSymlinks` is set.
    fn enqueue(&mut self, path: &str, depth: usize) -> Result<FileId, InputError> {
        self.enqueue_as(path, None, false, depth)
    }

    /// The id of a standard library file, as `pathForLibFile` locates it:
    /// in the catalog's directory unless a `@typescript/lib-*` package
    /// replaces it (`libraries::Catalog::replacement`). Replacement files
    /// are still libraries: never searched and never external.
    fn library(&mut self, request: libraries::Request) -> Result<(FileId, Library), InputError> {
        let file = request.file;
        if let Some((id, library, failure)) = self.library_ids.get(file).cloned() {
            // The replacement lookup's failure belongs to every request it
            // answers (`Resolver::isolated`).
            self.resolver.relatch(failure.as_ref());
            return Ok((id, library));
        }
        let outer = self.resolver.begin_isolation();
        let replacement = self.replacement(request);
        let failure = self.resolver.end_isolation(outer);
        let replacement = replacement?;
        let replaced = replacement.is_some();
        // The program names a replacement by its resolved file name, as
        // `pathForLibFile` does; a link it was reached through is another
        // spelling of it.
        let (path, spelled) =
            replacement.unwrap_or_else(|| (paths::combine(self.library_dir, file), None));
        let library = Library { file, replaced };
        // A library route never takes a package lookup.
        let id = self.enqueue_as(&path, Some(library), false, 0)?;
        if let Some(spelled) = spelled {
            self.spell(id, &spelled);
        }
        self.library_ids.insert(file, (id, library, failure));
        Ok((id, library))
    }

    /// The installed `@typescript/lib-*` package replacing `request`'s
    /// file, if any: its resolved file name and the spelling it was reached
    /// through, when that differs.
    fn replacement(
        &mut self,
        request: libraries::Request,
    ) -> Result<Option<(String, Option<String>)>, InputError> {
        let catalog = self.config.options.compiler.dialect().libraries();
        let Some((package, resolved)) = catalog.replacement(
            request,
            &self.config.options,
            &self.config.dir,
            &mut self.resolver,
        ) else {
            return Ok(None);
        };
        let reason = match resolved {
            Resolved::File { path, .. } if crate::script_kind(&path) == ScriptKind::Json => {
                format!(
                    "resolves to the JSON file '{}', which is not a library",
                    self.display(&path)
                )
            }
            // TypeScript reports TS6504 without `allowJs`, and would read a
            // JavaScript library with it: outside the supported inputs.
            Resolved::File {
                path, extension, ..
            } if extension.is_javascript() => format!(
                "resolves to the JavaScript file '{}', which has no TypeScript declarations",
                self.display(&path)
            ),
            Resolved::File {
                path,
                original_path,
                ..
            } => return Ok(Some((path, original_path))),
            Resolved::Missing => return Ok(None),
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
    /// `depth` is the route's, which lowers the input's own (`revisit`).
    fn enqueue_as(
        &mut self,
        path: &str,
        library: Option<Library>,
        searched_route: bool,
        depth: usize,
    ) -> Result<FileId, InputError> {
        let (id, known) = self.reserve(path, library, searched_route, depth)?;
        if known {
            self.revisit(id, depth)?;
        }
        Ok(id)
    }

    /// The id of an input and whether it was already known, reserving and
    /// queueing it, at `depth`, on first sight (`enqueue_as`).
    fn reserve(
        &mut self,
        path: &str,
        library: Option<Library>,
        searched_route: bool,
        depth: usize,
    ) -> Result<(FileId, bool), InputError> {
        let identity = self.identity(path)?;
        let key = self.case.key(&identity);
        let (id, known) = match self.ids.get(&key) {
            Some(&id) => (id, true),
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
                self.depths.push(depth);
                self.packages.push(None);
                (id, false)
            }
        };
        self.spell(id, path);
        Ok((id, known))
    }

    /// A route `depth` package lookups deep reaches a known input `id`. A
    /// depth lower than any route's before is its own from now on, which a
    /// queued input's subtasks start from when it loads. A loaded input
    /// whose dialect reprocesses it (`resolve::Rules::reprocesses`) follows
    /// its subtasks again from there, to a fixed point: each reaches its
    /// target lower too, and a JavaScript subtask elided on a deeper route
    /// is an input once within `maxNodeModuleJsDepth`, where that route met
    /// it (`collectFiles`).
    fn revisit(&mut self, id: FileId, depth: usize) -> Result<(), InputError> {
        let rules = self.config.options.compiler.dialect().resolution();
        let mut work = vec![(id, depth)];
        while let Some((id, depth)) = work.pop() {
            if depth >= self.depths[id.index()] {
                continue;
            }
            self.depths[id.index()] = depth;
            let Some(loaded) = self.loaded.get(id.index()) else {
                continue;
            };
            let elided: Vec<usize> = (0..self.program_only.len())
                .filter(|&at| {
                    self.program_only[at].importer == id && self.program_only[at].subtask.is_some()
                })
                .collect();
            let revisit = resolve::Revisit {
                depth,
                max_depth: self.config.options.max_node_module_js_depth,
                elided: !elided.is_empty(),
                found_in_packages: loaded.depth > 0,
            };
            if !rules.reprocesses(revisit) {
                continue;
            }
            for edge in &self.edges[id.index()] {
                work.push((edge.target, depth + usize::from(edge.external)));
            }
            // Elided JavaScript is always found through a package lookup.
            let route = depth + 1;
            if self.beyond_js_depth(route) {
                continue;
            }
            for at in elided.into_iter().rev() {
                let pending = self.program_only.remove(at);
                let position = pending.subtask.expect("an elided subtask");
                let (target, known) = self.reserve(&pending.target, None, false, route)?;
                if known {
                    work.push((target, route));
                }
                self.record_package(target, pending.package.as_ref());
                self.loaded[id.index()].modules[pending.request] = ModuleResolution::File(target);
                self.edges[id.index()].insert(
                    position,
                    Edge {
                        target,
                        external: true,
                        route: libraries::Route::Other,
                        package: pending.package,
                    },
                );
                // Later elided subtasks of this file follow the new edge.
                for later in &mut self.program_only[at..] {
                    if later.importer == id
                        && let Some(position) = &mut later.subtask
                    {
                        *position += 1;
                    }
                }
            }
        }
        Ok(())
    }

    /// Record `path` as a spelling through which `id` was reached.
    fn spell(&mut self, id: FileId, path: &str) {
        let lexical = paths::normalize(path);
        if !self.reached[id.index()].contains(&lexical) {
            self.reached[id.index()].push(lexical);
        }
    }

    /// Record the package id a route gives `id`: TypeScript 7's task data
    /// keeps the first one any route brings, even a route that elides the
    /// file (`filesParser.start`), in the order the routes start, which
    /// this loader's reservation order stands for.
    fn record_package(&mut self, id: FileId, package: Option<&PackageId>) {
        let recorded = &mut self.packages[id.index()];
        if recorded.is_none() {
            recorded.clone_from(&package.cloned());
        }
    }

    /// The type packages the configuration selects (`config::Types`):
    /// every package in the type roots, or the listed ones, a wildcard
    /// standing for every package in its place.
    fn type_packages(&mut self) -> Result<(), InputError> {
        let config = self.config;
        // An automatic package that cannot be resolved is the
        // configuration's error, as TypeScript reports it without a place.
        let automatic = Origin {
            path: config.path.clone(),
            position: None,
        };
        let mut names: Vec<(String, &Origin)> = Vec::new();
        match &config.options.types {
            config::Types::Automatic => {
                for name in self.resolver.automatic_types() {
                    names.push((name, &automatic));
                }
            }
            config::Types::Listed(entries) => {
                for (name, origin) in entries {
                    match name {
                        config::TypeName::Wildcard => {
                            for name in self.resolver.automatic_types() {
                                names.push((name, origin));
                            }
                        }
                        config::TypeName::Package(name) => names.push((name.clone(), origin)),
                    }
                }
            }
        }
        for (name, origin) in names {
            let Some(found) = self.resolver.type_reference(
                &name,
                &config.dir,
                None,
                TypeReferenceOrigin::Inferred,
            ) else {
                return Err(origin.error(
                    InputErrorKind::UnresolvedImport,
                    format!("Cannot find type definition file for '{name}'"),
                ));
            };
            // An automatic type directive's subtask, one lookup deep when
            // external.
            let depth = usize::from(found.external);
            let id = self.enqueue(found.reached_path(), depth)?;
            self.record_package(id, found.package.as_ref());
            self.root_tasks.push(RootTask {
                file: id,
                route: libraries::Route::Other,
                package: found.package.clone(),
                depth,
            });
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
        let script = crate::script_kind(&path);
        let javascript = matches!(script, ScriptKind::Js | ScriptKind::Jsx);
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
                    capture::collect(&ast, &source, indicators, module, self.parser, &file_path)?
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
        if self.config.options.semantic.import_helpers
            && (javascript
                || !declaration
                    && (self.config.options.semantic.isolated_modules || !json && facts.module))
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
        // `jsx_runtime` owns the implicit import rule; a missing runtime is
        // reported at the file's first JSX tag.
        let runtime = self.jsx_runtime(
            matches!(script, ScriptKind::Tsx | ScriptKind::Jsx),
            declaration,
            source.as_str(),
        );
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
            file: crate::checking::FileChecking::new(
                javascript,
                source.as_str(),
                options.semantic.check_js,
            ),
        };
        let checking = Checking::new(unchecked, &source, &facts);
        // The depth this file's subtasks start from, its lowest so far.
        let depth = self.depths[id.index()];
        let navigated = self.navigated[id.index()];
        let Subtasks {
            edges,
            modules,
            missing,
            unsupported,
            program_only,
        } = self.subtasks(id, &path, format, &source, &mut facts, depth, navigated)?;
        self.program_only.extend(program_only);
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
            depth,
            format,
            module_semantics,
            facts,
            modules,
            checking,
            missing,
            unsupported,
            navigation,
            // What the file's lookups could not read is its failure, as a
            // failure of its own text is (`failed`): the load's only if the
            // file stays in the program.
            failure: self.resolver.check_reads().err(),
        });
        self.edges.push(edges);
        Ok(())
    }

    /// Follow the reference directives and module requests of `source`,
    /// read as the file at `path` whose `facts` they are, `depth` package
    /// lookups deep, as `importer`'s subtasks (`processReferencedFiles`,
    /// `processTypeReferenceDirectives`, `processLibReferenceDirectives`,
    /// `processImportedModules`). `navigated` guesses whether a reference
    /// path's target is searched (`enqueue_as`).
    #[allow(clippy::too_many_arguments)]
    fn subtasks(
        &mut self,
        importer: FileId,
        path: &str,
        format: FileFormat,
        source: &crate::source::SourceText,
        facts: &mut FileFacts,
        depth: usize,
        navigated: bool,
    ) -> Result<Subtasks, InputError> {
        let file_path = PathBuf::from(path);
        let declaration = paths::is_declaration(path);
        // TypeScript's subtask order: `/// <reference path>`s, then `types`
        // and `lib` references, then module names: helpers, JSX, then each
        // request by its `subtask_rank` and, within a rank, by its position,
        // since binding collects requests in no particular order. Following
        // them in this order keeps the queue's reservation order
        // TypeScript's (see `queue`).
        let mut unsupported: Option<(usize, InputError)> = None;
        let mut missing: Vec<(usize, InputError)> = Vec::new();
        let mut targets = Vec::new();
        let mut program_only = Vec::new();
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
            match self.directive(depth, navigated, path, format, reference.directive, &origin)? {
                Followed::Edge(target, external, route, package) => targets.push(Edge {
                    target,
                    external,
                    route,
                    package,
                }),
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
            // still satisfy it, and otherwise `otherwise` says what
            // resolution's file means.
            let unresolved = |otherwise: ModuleResolution| {
                if request.implicit == Some(binding::ImplicitImport::Helpers) {
                    return ModuleResolution::Unchecked;
                }
                if request.implicit == Some(binding::ImplicitImport::JsxRuntime) {
                    // TypeScript requires the runtime only where JSX is used.
                    if facts.jsx.is_none() {
                        return ModuleResolution::Unchecked;
                    }
                } else if request.kind == (RequestKind::Import { side_effect: true })
                    && !self.config.options.check_side_effect_imports
                {
                    return ModuleResolution::Unchecked;
                }
                otherwise
            };
            // TypeScript's loader resolves no empty module name.
            let resolved = if request.specifier.is_empty() {
                Resolved::Missing
            } else {
                self.resolver.module(&request.specifier, path, mode)
            };
            let resolved_path = match &resolved {
                Resolved::File { path, .. } => Some(path.clone()),
                Resolved::Missing | Resolved::TooDeep => None,
            };
            modules[index] = match resolved {
                Resolved::File {
                    path: target,
                    original_path,
                    external,
                    extension,
                    package,
                } => {
                    // What the module means where no route makes it an
                    // input: JavaScript is an untyped module.
                    let untyped = || ModuleResolution::Untyped(self.display(&target));
                    let admission = resolve::admission(
                        &self.config.options,
                        (&target, extension, external),
                        declaration,
                    );
                    match admission {
                        // TypeScript's loader adds no file for a module
                        // augmentation's name (`shouldAddFile`): its target
                        // is a program input only by another route.
                        Admission::Input { .. }
                            if matches!(request.kind, RequestKind::Augmentation { .. }) =>
                        {
                            let otherwise = if extension.is_javascript() {
                                untyped()
                            } else {
                                ModuleResolution::Missing
                            };
                            program_only.push(ProgramOnly {
                                importer,
                                request: index,
                                target,
                                otherwise,
                                subtask: None,
                                package: None,
                            });
                            ModuleResolution::Missing
                        }
                        Admission::Input { elide_on_depth } => {
                            let route = depth + usize::from(external);
                            if elide_on_depth && self.beyond_js_depth(route) {
                                // Settled with the program: a shallower
                                // route may still load the file, or reach
                                // this file lower (`revisit`).
                                let otherwise = unresolved(untyped());
                                program_only.push(ProgramOnly {
                                    importer,
                                    request: index,
                                    target: original_path.unwrap_or(target),
                                    otherwise,
                                    subtask: Some(targets.len()),
                                    package,
                                });
                                ModuleResolution::Missing
                            } else {
                                let target = self
                                    .enqueue(original_path.as_deref().unwrap_or(&target), route)?;
                                self.record_package(target, package.as_ref());
                                targets.push(Edge {
                                    target,
                                    external,
                                    route: libraries::Route::Other,
                                    package,
                                });
                                ModuleResolution::File(target)
                            }
                        }
                        // JavaScript without `allowJs`, which no route makes
                        // an input.
                        Admission::NotAdded(None | Some(NotAdmitted::AllowJs)) => {
                            unresolved(untyped())
                        }
                        Admission::NotAdded(Some(reason)) => {
                            let message =
                                reason.message(&request.specifier, &self.display(&target));
                            let resolution =
                                unresolved(ModuleResolution::NotAdmitted(reason.code(), message));
                            if reason == NotAdmitted::Jsx
                                && matches!(resolution, ModuleResolution::NotAdmitted(..))
                            {
                                program_only.push(ProgramOnly {
                                    importer,
                                    request: index,
                                    target,
                                    otherwise: resolution.clone(),
                                    subtask: None,
                                    package: None,
                                });
                            }
                            resolution
                        }
                    }
                }
                Resolved::Missing => unresolved(ModuleResolution::Missing),
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
        Ok(Subtasks {
            edges: targets,
            modules,
            missing,
            unsupported,
            program_only,
        })
    }

    /// An input whose capture failed, kept without facts or edges: its
    /// failure is the load's only if the input stays in the program, which
    /// a package redirect can prevent, since TypeScript reports nothing of a
    /// file `collectFiles` leaves out. Loading goes on, so that the program
    /// is known; the failure of the first such input in load order that
    /// stays in it is the load's (`load_program`).
    fn failed(&mut self, id: FileId, path: String, error: InputError) {
        debug_assert_eq!(
            self.loaded.len(),
            id.index(),
            "a failed input loads nothing"
        );
        self.program_only.retain(|pending| pending.importer != id);
        let display = self.display(&path);
        self.sources.capture(
            id,
            SourceFile {
                path: PathBuf::from(path),
                display,
                text: std::sync::Arc::new(crate::source::SourceText::new(String::new())),
            },
        );
        let unchecked = Unchecked {
            no_check: true,
            declaration: false,
            skip_lib_check: false,
            library: false,
            skip_default_lib_check: false,
            json: false,
            file: crate::checking::FileChecking::new(false, "", None),
        };
        let facts = FileFacts::default();
        self.loaded.push(Loaded {
            depth: self.depths[id.index()],
            format: FileFormat::undeclared(),
            module_semantics: self
                .resolver
                .module_semantics(FileFormat::undeclared(), false),
            checking: Checking::new(unchecked, &self.sources.get(id).text, &facts),
            facts,
            modules: Vec::new(),
            missing: Vec::new(),
            unsupported: None,
            navigation: None,
            failure: Some(error),
        });
        self.edges.push(Vec::new());
    }

    /// TypeScript's checker resolves a request to a file the loader did not
    /// add for it when the program has that file by another route, such as a
    /// root (`resolveExternalModule`): an import whose `.tsx` or `.jsx` file
    /// lacks `jsx` (TS6142), a module augmentation's target, and JavaScript
    /// elided on the request's route. An elided subtask whose file a later
    /// route loaded counts as one of its importer's edges again, where
    /// TypeScript's `collectFiles` meets it. A request whose file no route
    /// has loaded yet stays pending: collection can still add it, processing
    /// a redirect again (`borrow`), until `finish_program_only`.
    fn settle_program_only(&mut self) {
        let mut pending_still = Vec::new();
        // Latest first, so that each elided subtask's position counts only
        // the edges its importer had when it was met.
        for pending in std::mem::take(&mut self.program_only).into_iter().rev() {
            let identity = match self.identity(&pending.target) {
                Ok(identity) => identity,
                // The importer's failure, the load's only if the importer
                // stays in the program (`failed`).
                Err(error) => {
                    self.loaded[pending.importer.index()]
                        .failure
                        .get_or_insert(error);
                    continue;
                }
            };
            let Some(&id) = self.ids.get(&self.case.key(&identity)) else {
                pending_still.push(pending);
                continue;
            };
            self.loaded[pending.importer.index()].modules[pending.request] =
                ModuleResolution::File(id);
            self.program_only_fallbacks
                .insert((pending.importer, pending.request), pending.otherwise);
            if let Some(position) = pending.subtask {
                self.record_package(id, pending.package.as_ref());
                self.edges[pending.importer.index()].insert(
                    position,
                    Edge {
                        target: id,
                        external: true,
                        route: libraries::Route::Other,
                        package: pending.package,
                    },
                );
                // The importer's elided subtasks met after this one, still
                // pending, follow the new edge.
                for later in &mut pending_still {
                    if later.importer == pending.importer
                        && let Some(position) = &mut later.subtask
                    {
                        *position += 1;
                    }
                }
            }
        }
        pending_still.reverse();
        self.program_only = pending_still;
    }

    /// Once collection adds no more files, a request still pending
    /// (`settle_program_only`) keeps what it means without its file: an
    /// unresolved name, which for an augmentation is TS2664 where reported,
    /// or an untyped module.
    fn finish_program_only(&mut self) {
        for pending in std::mem::take(&mut self.program_only) {
            self.loaded[pending.importer.index()].modules[pending.request] = pending.otherwise;
        }
    }

    /// Whether a JavaScript file a package lookup found in `node_modules` is
    /// no input on a route `depth` lookups deep: TypeScript's
    /// `filesParser.start` elides it beyond `maxNodeModuleJsDepth`.
    fn beyond_js_depth(&self, depth: usize) -> bool {
        i64::try_from(depth).map_or(true, |depth| {
            depth > self.config.options.max_node_module_js_depth
        })
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

    /// TypeScript 7.0.2's `resolveImportsAndModuleAugmentations` collects
    /// automatic runtime imports only for JSX/TSX, using
    /// `GetJSXImplicitImportBase` and `GetJSXRuntimeImport` for `jsx`,
    /// `jsxImportSource`, and file pragmas. The 5.6 dialect deliberately
    /// retains this narrower collection rule: 5.6.3's
    /// `collectExternalModuleReferences` also considers JavaScript and
    /// eligible ordinary non-declaration files. See docs/references.md's
    /// compatibility limits for the resulting input-order difference.
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

    /// Follows one reference directive of the file at `path`, whose
    /// subtasks start `depth` package lookups deep. An error is fatal
    /// wherever the directive appears; other failures are outcomes the
    /// caller settles.
    fn directive(
        &mut self,
        depth: usize,
        navigated: bool,
        path: &str,
        format: FileFormat,
        directive: Directive,
        origin: &Origin,
    ) -> Result<Followed, InputError> {
        let config = self.config;
        let options = &config.options;
        match directive {
            Directive::Lib(name) => match options
                .compiler
                .dialect()
                .libraries()
                .reference(options, &name)
            {
                libraries::Reference::Ignored => Ok(Followed::Nothing),
                libraries::Reference::File(request) => {
                    let (id, library) = self.library(request)?;
                    Ok(Followed::Edge(
                        id,
                        false,
                        libraries::Route::Library(library),
                        None,
                    ))
                }
                libraries::Reference::Unknown(message) => Ok(Followed::Missing(
                    origin.error(InputErrorKind::UnresolvedImport, message),
                )),
            },
            Directive::Types(name, mode) => {
                let mode = mode.or_else(|| self.resolver.type_directive_mode(format));
                let Some(found) = self.resolver.type_reference(
                    &name,
                    paths::parent(path),
                    mode,
                    TypeReferenceOrigin::Source,
                ) else {
                    return Ok(Followed::Missing(origin.error(
                        InputErrorKind::UnresolvedImport,
                        format!("Cannot find type definition file for '{name}'"),
                    )));
                };
                let id = self.enqueue(found.reached_path(), depth + usize::from(found.external))?;
                self.record_package(id, found.package.as_ref());
                Ok(Followed::Edge(
                    id,
                    found.external,
                    libraries::Route::Other,
                    found.package,
                ))
            }
            Directive::Path(reference) => {
                let target = paths::resolve(paths::parent(path), &reference);
                let unsupported = |reason: String| {
                    Ok(Followed::Unsupported(origin.error(
                        InputErrorKind::Unsupported,
                        format!("Referenced file '{reference}' {reason}"),
                    )))
                };
                match files::literal_input(&target, options, self.case, self.disk)? {
                    files::Literal::JavaScript => Ok(Followed::Unsupported(origin.error(
                        InputErrorKind::Unsupported,
                        files::needs_allow_js(&reference),
                    ))),
                    files::Literal::Found(found) => {
                        // A reference path is never an external edge. It
                        // keeps what it writes and the extension it was
                        // found with, for its target's program name.
                        let extension = found
                            .strip_prefix(target.as_str())
                            .expect("a literal input is found at its path or with an extension")
                            .to_owned();
                        Ok(Followed::Edge(
                            self.enqueue_as(&found, None, navigated, depth)?,
                            false,
                            libraries::Route::ReferencePath {
                                written: reference,
                                extension,
                            },
                            None,
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
    /// the files ordered with the libraries then come first, ordered by
    /// `getDefaultLibFilePriority` (a stable sort, as 5.6.3's is and 7.0.2's
    /// is not; see docs/references.md). Which files those are the catalog
    /// decides where the walk first meets each one
    /// (`libraries::Catalog::ordering`), as 5.6.3 decides where
    /// it first processes one.
    ///
    /// The walk deduplicates packages too, unless `deduplicatePackages` is
    /// false (`packageIdToSourceFile`): a file it meets with the package id
    /// of an input it met before is that input's redirect, whose subtasks it
    /// does not follow, so that what only the redirect reaches leaves the
    /// program. Which route's id a file has is the dialect's
    /// (`resolve::Rules::first_route_package_id`); in both compilers the
    /// first file met depth first is the program's, whichever file a
    /// breadth-first load reserved first.
    ///
    /// Where the dialect's collection processes a file again
    /// (`resolve::Rules::collected_provenance`), the walk does too: a route
    /// at depth zero that meets a file first met through a package lookup
    /// follows its subtasks again, and a redirect's are its target's,
    /// followed from the redirect's location (`borrow`), which the walk
    /// asks for before it can go on (`Collection::Borrows`).
    fn program_order(&self) -> Collection {
        let rules = self.config.options.compiler.dialect().resolution();
        let mut walk = Walk {
            catalog: self.config.options.compiler.dialect().libraries(),
            first_route: rules.first_route_package_id(),
            reprocesses: rules.collected_provenance(),
            deduplicate: self.config.options.semantic.deduplicate_packages,
            walked: vec![false; self.loaded.len()],
            in_packages: vec![false; self.loaded.len()],
            placements: vec![None; self.loaded.len()],
            order: Vec::with_capacity(self.loaded.len()),
            primaries: HashMap::new(),
            redirects: Vec::new(),
            redirected: HashMap::new(),
            frames: Vec::new(),
        };
        for task in &self.root_tasks {
            let met = self.meet(
                &mut walk,
                task.file,
                &task.route,
                task.package.as_ref(),
                task.depth,
                None,
            );
            if let Some(borrows) = met {
                return borrows;
            }
            while let Some(frame) = walk.frames.last_mut() {
                let edges = if frame.borrowed {
                    &self.borrowed[&frame.file]
                } else {
                    &self.edges[frame.file.index()]
                };
                let Some(edge) = edges.get(frame.next) else {
                    let frame = walk.frames.pop().expect("a frame is on the walk");
                    if frame.listed {
                        walk.order.push(frame.file);
                    }
                    continue;
                };
                frame.next += 1;
                let (from, depth) = (frame.file, frame.depth + usize::from(edge.external));
                let met = self.meet(
                    &mut walk,
                    edge.target,
                    &edge.route,
                    edge.package.as_ref(),
                    depth,
                    Some(from),
                );
                if let Some(borrows) = met {
                    return borrows;
                }
            }
        }
        debug_assert!(
            !walk.redirects.is_empty()
                || walk.order.len() == self.loaded.len()
                || self.loaded.iter().any(|loaded| loaded.failure.is_some()),
            "every input is walked unless a redirect hides it or an input failed"
        );
        let Walk {
            catalog,
            walked,
            in_packages,
            placements,
            order,
            redirects,
            ..
        } = walk;
        let (mut libraries, files): (Vec<FileId>, Vec<FileId>) = order
            .into_iter()
            .partition(|id| placements[id.index()].is_some());
        libraries.sort_by_cached_key(|id| {
            let placement = placements[id.index()].as_ref();
            catalog.priority(
                placement.expect("partitioned with the libraries"),
                self.library_dir,
            )
        });
        libraries.extend(files);
        Collection::Collected(Collected {
            order: libraries,
            redirects,
            at_depth_zero: walked
                .into_iter()
                .zip(in_packages)
                .map(|(walked, in_packages)| walked && !in_packages)
                .collect(),
        })
    }

    /// Collection meets `id` through `route`, `depth` package lookups deep,
    /// which gives it `package`, from the file `from` (none for a root
    /// task): 5.6.3's `findSourceFileWorker`, 7.0.2's `collectFiles`. A file
    /// met before is processed again only where the dialect does so; the
    /// walk then asks for a redirect's borrowed subtasks unless they are
    /// loaded.
    fn meet(
        &self,
        walk: &mut Walk,
        id: FileId,
        route: &libraries::Route,
        package: Option<&PackageId>,
        depth: usize,
        from: Option<FileId>,
    ) -> Option<Collection> {
        if std::mem::replace(&mut walk.walked[id.index()], true) {
            // `sourceFilesFoundSearchingNodeModules`: a route at depth zero
            // processes again a file a package lookup met, once.
            if !walk.reprocesses || depth > 0 || !walk.in_packages[id.index()] {
                return None;
            }
            let borrowed = match walk.redirected.get(&id) {
                None => false,
                Some(_) if self.borrowed.contains_key(&id) => true,
                Some(&target) => {
                    return Some(Collection::Borrows {
                        redirect: id,
                        target,
                    });
                }
            };
            walk.in_packages[id.index()] = false;
            walk.frames.push(Frame {
                file: id,
                borrowed,
                next: 0,
                depth,
                listed: false,
            });
            return None;
        }
        walk.in_packages[id.index()] = depth > 0;
        let package = if walk.first_route {
            package
        } else {
            self.packages[id.index()].as_ref()
        };
        if let Some(package) = package.filter(|_| walk.deduplicate) {
            match walk.primaries.get(package) {
                Some(&primary) => {
                    walk.redirects.push(Redirected {
                        file: id,
                        target: primary,
                        package: package.clone(),
                    });
                    walk.redirected.insert(id, primary);
                    return None;
                }
                None => {
                    walk.primaries.insert(package.clone(), id);
                }
            }
        }
        let identity = |id: FileId| {
            self.sources
                .get(id)
                .path
                .to_str()
                .expect("an input's identity is text")
        };
        let from = from.and_then(|from| {
            walk.placements[from.index()].as_ref().map(|placement| {
                placement
                    .name(self.library_dir, identity(from))
                    .into_owned()
            })
        });
        walk.placements[id.index()] = walk.catalog.ordering(
            self.libraries[id.index()],
            identity(id),
            route,
            from.as_deref(),
        );
        walk.frames.push(Frame {
            file: id,
            borrowed: false,
            next: 0,
            depth,
            listed: true,
        });
        None
    }

    /// 5.6.3's `findSourceFileWorker` processes `redirect`, which a package
    /// lookup met first, again where a route at depth zero meets it. The
    /// redirect's parsed file is its target's (`createRedirectedSourceFile`),
    /// so it follows the target's reference directives and module requests,
    /// in the target's module format, resolved from the redirect's own
    /// location: a reference to `./note.d.ts` reaches the redirect's
    /// sibling, which neither copy names from where it is. Follow them as
    /// such a route does, at depth zero, and load what they reach; the walk
    /// follows them where it met the redirect (`program_order`). What only
    /// the request's own resolution decides, which no checker reads from a
    /// redirect, is not kept, nor is JavaScript elided on these routes.
    fn borrow(&mut self, redirect: FileId, target: FileId) -> Result<(), InputError> {
        let path = self
            .sources
            .get(redirect)
            .path
            .to_str()
            .expect("an input's identity is text")
            .to_owned();
        let source = std::sync::Arc::clone(&self.sources.get(target).text);
        let format = self.loaded[target.index()].format;
        let mut facts = self.loaded[target.index()].facts.clone();
        let Subtasks {
            edges,
            missing,
            unsupported,
            ..
        } = self.subtasks(redirect, &path, format, &source, &mut facts, 0, true)?;
        // Its lookups are those of a file processed at depth zero, which
        // stays in the program: a failure fails the load at once, ahead of
        // the failures of the files in load order (`failed`).
        self.resolver.check_reads()?;
        self.borrowed.insert(redirect, edges);
        if !missing.is_empty() || unsupported.is_some() {
            self.borrowed_failures.push(BorrowedFailures {
                target,
                missing,
                unsupported,
            });
        }
        self.load_queued();
        self.settle_program_only();
        Ok(())
    }

    /// What a redirect processed again met (`borrow`), reported as for a
    /// project file, which a file processed at depth zero is: 5.6.3 reports
    /// a missing reference there at the redirect, whose text and so whose
    /// `SkipTypeChecking` and comment directives are its target's.
    fn borrowed_failure(&self) -> Result<(), InputError> {
        for failures in &self.borrowed_failures {
            let diagnostics = Diagnostics::new(
                FileOrigin::Project,
                self.loaded[failures.target.index()].checking.clone(),
            );
            if let Some(error) = &failures.unsupported
                && !diagnostics.tolerates_unsupported_input()
            {
                return Err(error.clone());
            }
            let text = &self.sources.get(failures.target).text;
            if let Some((_, error)) = failures
                .missing
                .iter()
                .find(|&&(offset, _)| diagnostics.reports_inclusion(text, offset))
            {
                return Err(error.clone());
            }
        }
        Ok(())
    }

    /// Inputs TypeScript finds at depth zero, less the standard libraries
    /// themselves. Which routes count is the dialect's
    /// (`resolve::Rules::collected_provenance`): under 5.6.3 those
    /// collection processes (`program_order`), which never follows a
    /// redirect's own subtasks; under TypeScript 7 every loaded file's
    /// (`lowestDepth`), a redirect's and those of files only it reaches
    /// included, so that an input is searched where the root tasks at depth
    /// zero reach it without crossing an external-library edge.
    fn search(&self, collected: &Collected) -> Vec<bool> {
        let rules = self.config.options.compiler.dialect().resolution();
        let reached = if rules.collected_provenance() {
            collected.at_depth_zero.clone()
        } else {
            let mut reached = vec![false; self.loaded.len()];
            let mut stack: Vec<FileId> = self
                .root_tasks
                .iter()
                .filter(|task| task.depth == 0)
                .map(|task| task.file)
                .collect();
            while let Some(id) = stack.pop() {
                if std::mem::replace(&mut reached[id.index()], true) {
                    continue;
                }
                for edge in &self.edges[id.index()] {
                    if !edge.external && !reached[edge.target.index()] {
                        stack.push(edge.target);
                    }
                }
            }
            reached
        };
        reached
            .into_iter()
            .zip(&self.libraries)
            .map(|(reached, library)| reached && library.is_none())
            .collect()
    }
}
