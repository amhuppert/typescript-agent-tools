//! Frozen inputs to native module-specifier generation. Loading owns physical
//! observations; generation owns candidate transforms and their demand order.
//! An uncaptured observation is an invariant failure, never a negative probe.

use super::ModuleResolutionKind;
use super::config::Config;
pub(super) use super::manifest::PackageInfo;
use super::paths;
use super::resolve::Resolver;
use super::version::Version;
use super::{ProgramFile, Redirect, disk, specifiers};
use crate::analysis::InputError;
use crate::input::Case;
use crate::project::sources::Sources;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) struct SpecifierOptions {
    pub(super) module_resolution: ModuleResolutionKind,
    pub(super) paths: Option<SpecifierPaths>,
    /// `baseUrl` (TypeScript 5.6 dialect), independently of `paths`.
    pub(super) base_url: Option<String>,
    pub(super) root_dirs: Vec<String>,
    pub(super) custom_conditions: Vec<String>,
    pub(super) package_exports: bool,
    pub(super) package_imports: bool,
    pub(super) jsx_preserve: bool,
    pub(super) allow_importing_ts_extensions: bool,
    pub(super) rewrite_relative_import_extensions: bool,
    pub(super) out_dir: Option<String>,
    pub(super) declaration_dir: Option<String>,
    pub(super) common_source_directory: String,
    /// The dialect's compiler version, which package ranges are evaluated
    /// against here as in resolution.
    pub(super) typescript: Version,
}

#[derive(Debug, Clone)]
pub(super) struct SpecifierPaths {
    pub(super) base: String,
    pub(super) patterns: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone)]
pub(in crate::project) enum SpecifierInputError {
    Input(InputError),
    NotCaptured {
        operation: &'static str,
        path: String,
    },
}

/// Native KnownSymlinks' directory relationships, built from actual module and
/// type-reference resolutions and emitted-source runtime dependencies. Raw
/// aliases remain distinct even when they name the same captured SourceFile.
#[derive(Debug, Clone, Default)]
pub(super) struct KnownSymlinks {
    directories: HashMap<String, String>,
    directories_by_realpath: HashMap<String, Vec<String>>,
}

impl KnownSymlinks {
    pub(super) fn directories_by_realpath(&self, realpath: &str, case: Case) -> Option<&[String]> {
        // Most programs reach no symlinked directory: no key to make.
        if self.directories_by_realpath.is_empty() {
            return None;
        }
        self.directories_by_realpath
            .get(&case.key(realpath))
            .map(Vec::as_slice)
    }

    /// `KnownSymlinks.HasDirectory` observes the inferred directory, not the
    /// existence of a physical link or a file-level alias.
    fn has_directory(&self, directory: &str, case: Case) -> bool {
        self.directories
            .contains_key(&case.key(&trailing_separator(directory)))
    }

    /// `KnownSymlinks.ProcessResolution` / `guessDirectorySymlink`. Only an
    /// native path pair establishes a relationship. Equal tail
    /// components are removed until a package boundary, retaining the raw
    /// alias spelling alongside the case-sensitive or folded identity key.
    fn process_resolution(&mut self, original: &str, resolved: &str, cwd: &str, case: Case) {
        if original.is_empty() || resolved.is_empty() {
            return;
        }
        let resolved = paths::resolve(cwd, resolved);
        let original = paths::resolve(cwd, original);
        let (mut real, mut alias) = (resolved.as_str(), original.as_str());
        let mut directory = false;
        loop {
            let (real_parent, alias_parent) = (paths::parent(real), paths::parent(alias));
            if real_parent.is_empty()
                || alias_parent.is_empty()
                || real_parent == real
                || alias_parent == alias
                || package_boundary(paths::basename(real_parent), case)
                || package_boundary(paths::basename(alias_parent), case)
                || case.fold(paths::basename(real)) != case.fold(paths::basename(alias))
            {
                break;
            }
            real = real_parent;
            alias = alias_parent;
            directory = true;
        }
        if !directory {
            return;
        }
        let alias_key = case.key(alias);
        if ignored_path(&alias_key) {
            return;
        }
        let alias_key = trailing_separator(&alias_key);
        let real = trailing_separator(real);
        if !self.directories.contains_key(&alias_key) {
            let aliases = self
                .directories_by_realpath
                .entry(case.key(&real))
                .or_default();
            if !aliases.iter().any(|existing| existing == alias) {
                aliases.push(alias.to_owned());
            }
        }
        self.directories.insert(alias_key, real);
    }
}

fn trailing_separator(path: &str) -> String {
    if path.ends_with('/') {
        path.to_owned()
    } else {
        format!("{path}/")
    }
}

fn package_boundary(component: &str, case: Case) -> bool {
    case.fold(component) == "node_modules" || component.starts_with('@')
}

/// `tspath.ContainsIgnoredPath`, shared by native symlink acquisition and
/// module-path enumeration; it does not interpret filesystem ignore files.
pub(super) fn ignored_path(path: &str) -> bool {
    path.contains("/node_modules/.") || path.contains("/.git") || path.contains(".#")
}

/// The program whose module names generation spells: its inputs, their
/// captured facts, and the files that redirect to them.
pub(super) struct Inputs<'p> {
    pub(super) sources: &'p Sources,
    pub(super) files: &'p [ProgramFile],
    pub(super) redirects: &'p [Redirect],
}

#[derive(Debug)]
pub(in crate::project) struct SpecifierEnvironment {
    pub(super) options: SpecifierOptions,
    pub(super) case: Case,
    pub(super) cwd: String,
    pub(super) config_file: String,
    pub(super) symlinks: Result<KnownSymlinks, SpecifierInputError>,
    planning_failure: Option<SpecifierInputError>,
    files: HashMap<String, Result<bool, SpecifierInputError>>,
    packages: HashMap<String, Result<Option<Arc<PackageInfo>>, SpecifierInputError>>,
    /// `Program.GetRedirectTargets`: by an input's identity key, the files
    /// that redirect to it, in the order collection met them.
    redirects: HashMap<String, Vec<String>>,
}

impl SpecifierEnvironment {
    /// Capture the native generation host's finite physical inputs at load.
    /// Candidate transforms are shared with generation; this stores probe
    /// answers once rather than storing every importer/target spelling.
    pub(super) fn capture(
        config: &Config,
        cwd: &str,
        case: Case,
        disk: &disk::Disk,
        resolver: &mut Resolver<'_>,
        inputs: Inputs<'_>,
    ) -> Self {
        let Inputs {
            sources,
            files,
            redirects,
        } = inputs;
        let options = &config.options;
        let mut env = Self {
            options: SpecifierOptions {
                module_resolution: options.resolution,
                paths: options.paths.as_ref().map(|paths| SpecifierPaths {
                    base: paths.base.clone(),
                    patterns: paths.patterns.clone(),
                }),
                base_url: options.base_url.clone(),
                root_dirs: options.root_dirs.clone(),
                custom_conditions: options.custom_conditions.clone(),
                package_exports: options.package_exports,
                package_imports: options.package_imports,
                jsx_preserve: options.jsx_preserve,
                allow_importing_ts_extensions: options.allow_importing_ts_extensions,
                rewrite_relative_import_extensions: options.rewrite_relative_import_extensions,
                out_dir: options.out_dir.clone(),
                declaration_dir: options.declaration_dir.clone(),
                // `GetCommonSourceDirectory`: every admitted program has a
                // configuration; project references are outside the profile.
                common_source_directory: trailing_separator(
                    options.root_dir.as_deref().unwrap_or(&config.dir),
                ),
                typescript: options.compiler.dialect().typescript().clone(),
            },
            case,
            cwd: cwd.to_owned(),
            config_file: config.file.clone(),
            symlinks: Ok(KnownSymlinks::default()),
            planning_failure: None,
            files: HashMap::new(),
            packages: HashMap::new(),
            redirects: HashMap::new(),
        };
        for redirect in redirects {
            let target = sources.get(redirect.target).path.to_str();
            let key = case.key(target.expect("captured paths are exact text"));
            let path = redirect
                .path
                .to_str()
                .expect("captured paths are exact text");
            env.redirects.entry(key).or_default().push(path.to_owned());
        }
        let importers: Vec<String> = sources
            .ids()
            .map(|id| {
                sources
                    .get(id)
                    .path
                    .to_str()
                    .expect("captured paths are exact text")
                    .to_owned()
            })
            .collect();
        let mut targets: BTreeSet<String> = importers.iter().cloned().collect();
        for file in files {
            targets.extend(
                file.facts
                    .requests
                    .iter()
                    .filter_map(|request| request.resolved_path.clone()),
            );
        }
        let targets: Vec<_> = targets.into_iter().collect();
        // Source metadata always has a package-scope question, including mts,
        // cts and Bundler files. It is independent from file_format's gate.
        let scopes: Vec<_> = importers
            .iter()
            .map(|file| resolver.source_package_scope(file))
            .collect();
        env.symlinks = env.capture_symlinks(resolver, &importers, files, scopes);
        let mut walked = HashSet::new();
        for path in importers.iter().chain(&targets) {
            env.capture_packages(resolver, &mut walked, path);
        }
        match specifiers::module_path_candidates(&env, &importers, &targets) {
            Ok(paths) => {
                for path in paths {
                    env.capture_packages(resolver, &mut walked, &path);
                }
            }
            Err(error) => env.planning_failure = Some(error),
        }
        if env.planning_failure.is_none() {
            match specifiers::index_collision_paths(&env, &importers, &targets) {
                Ok(paths) => {
                    for path in paths {
                        let key = env.observation_key(&path);
                        env.files.entry(key).or_insert_with(|| {
                            disk.file_exists(&path).map_err(|error| {
                                SpecifierInputError::Input(super::io_error(
                                    Path::new(&path),
                                    &error,
                                ))
                            })
                        });
                    }
                }
                Err(error) => env.planning_failure = Some(error),
            }
        }
        env
    }

    /// The names of `target`'s module (`GetEachFileNameOfModule`): its own
    /// path, then each file that redirects to it, absolute against the
    /// working directory.
    pub(super) fn file_names_of(&self, target: &str) -> Vec<String> {
        let target = paths::resolve(&self.cwd, target);
        let redirects = self
            .redirects
            .get(&self.case.key(&target))
            .map_or(&[][..], Vec::as_slice);
        let mut names = Vec::with_capacity(1 + redirects.len());
        names.push(target);
        names.extend(
            redirects
                .iter()
                .map(|redirect| paths::resolve(&self.cwd, redirect)),
        );
        names
    }

    /// Include the complete filename itself: native single-file packages may
    /// ask for targetFile/package.json before stepping to its parent directory.
    ///
    /// `walked` holds every path whose ancestors were all walked. Ancestors
    /// depend only on a path's spelling, so the walk stops at the first one
    /// walked before, whose own and later probes are captured already: in
    /// order, each package observation is still made where it first was.
    fn capture_packages(
        &mut self,
        resolver: &mut Resolver<'_>,
        walked: &mut HashSet<String>,
        path: &str,
    ) {
        for directory in paths::ancestors(path) {
            if !walked.insert(directory.to_owned()) {
                break;
            }
            let file = paths::combine(directory, "package.json");
            let key = self.observation_key(&file);
            self.packages.entry(key).or_insert_with(|| {
                resolver
                    .package_observation(directory)
                    .map_err(SpecifierInputError::Input)
            });
        }
    }

    /// `Program.GetSymlinkCache`: actual resolution pairs precede additional
    /// runtime dependency directories of source packages that may emit.
    fn capture_symlinks(
        &self,
        resolver: &mut Resolver<'_>,
        paths: &[String],
        files: &[ProgramFile],
        scopes: Vec<Result<Option<Arc<PackageInfo>>, InputError>>,
    ) -> Result<KnownSymlinks, SpecifierInputError> {
        let mut symlinks = KnownSymlinks::default();
        for (original, resolved) in resolver.symlink_resolutions() {
            symlinks.process_resolution(original, resolved, &self.cwd, self.case);
        }
        let mut seen = BTreeSet::new();
        for ((file, metadata), scope) in paths.iter().zip(files).zip(scopes) {
            let Some(scope) = scope.map_err(SpecifierInputError::Input)? else {
                continue;
            };
            if !self.source_file_may_be_emitted(file, metadata)? {
                continue;
            }
            if !seen.insert(self.case.key(&scope.directory)) {
                continue;
            }
            let package_file = paths::combine(&scope.directory, "package.json");
            for dependency in scope.runtime_dependency_names() {
                let modules = paths::combine(&scope.directory, "node_modules");
                if symlinks.has_directory(&paths::combine(&modules, &dependency), self.case)
                    || !dependency.starts_with("@types")
                        && symlinks.has_directory(
                            &paths::combine(
                                &modules,
                                &super::resolve::types_package_name(&dependency),
                            ),
                            self.case,
                        )
                {
                    continue;
                }
                if let Some(resolved) = resolver
                    .package_directory(&dependency, &package_file)
                    .map_err(SpecifierInputError::Input)?
                {
                    // GetSymlinkCache combines before ProcessResolution's
                    // empty-path guard. An empty OriginalPath therefore means
                    // cwd/package.json, even for a physical package directory.
                    symlinks.process_resolution(
                        &paths::combine(
                            resolved.original_path.as_deref().unwrap_or(""),
                            "package.json",
                        ),
                        &paths::combine(&resolved.path, "package.json"),
                        &self.cwd,
                        self.case,
                    );
                }
            }
        }
        Ok(symlinks)
    }

    /// `SourceFileMayBeEmitted(false)`: noEmit is deliberately not a gate.
    /// JS source files and project references are outside the admitted input
    /// profile. JSON needs outDir and an output path different from its input.
    fn source_file_may_be_emitted(
        &self,
        file: &str,
        metadata: &ProgramFile,
    ) -> Result<bool, SpecifierInputError> {
        if paths::is_declaration(file) || !metadata.search {
            return Ok(false);
        }
        if crate::script_kind(file) != crate::ScriptKind::Json {
            return Ok(true);
        }
        let Some(out_dir) = &self.options.out_dir else {
            return Ok(false);
        };
        let common = &self.options.common_source_directory;
        let relative = if self
            .case
            .fold(file)
            .starts_with(self.case.fold(common).as_ref())
        {
            file.get(common.len()..).ok_or_else(|| {
                SpecifierInputError::Input(InputError {
                    kind: crate::analysis::InputErrorKind::Unsupported,
                    message: "Native JSON output path splits a non-Unicode filename boundary"
                        .to_owned(),
                    path: Some(file.into()),
                    position: None,
                })
            })?
        } else {
            file
        };
        let output = paths::combine(out_dir, relative);
        Ok(!paths::compare(file, &output, &self.cwd, self.case).is_eq())
    }

    /// `ModuleSpecifierGenerationHost.FileExists`: consumes the captured
    /// answer only when the spelling algorithm reaches this exact raw path.
    pub(super) fn file_exists(&self, path: &str) -> Result<bool, SpecifierInputError> {
        if let Some(error) = &self.planning_failure {
            return Err(error.clone());
        }
        self.files
            .get(&self.observation_key(path))
            .cloned()
            .unwrap_or_else(|| {
                Err(SpecifierInputError::NotCaptured {
                    operation: "fileExists",
                    path: path.to_owned(),
                })
            })
    }

    /// `GetPackageJsonInfo`: `path` is the complete normalized absolute
    /// package.json filename. Missing, malformed, and unreadable stay distinct.
    pub(super) fn package_json(
        &self,
        path: &str,
    ) -> Result<Option<Cow<'_, PackageInfo>>, SpecifierInputError> {
        match self.packages.get(&self.observation_key(path)) {
            Some(Ok(package)) => Ok(package
                .as_deref()
                .map(|package| package.with_directory(paths::parent(path)))),
            Some(Err(error)) => Err(error.clone()),
            None => Err(SpecifierInputError::NotCaptured {
                operation: "packageJson",
                path: path.to_owned(),
            }),
        }
    }

    /// Native `ToPath` keys the captured observation, including absence and
    /// failures. Payloads retain their original path; sensitive hosts keep
    /// differently cased names distinct. This never probes an uncaptured path.
    fn observation_key(&self, path: &str) -> String {
        self.case.key(&paths::resolve(&self.cwd, path))
    }
}
