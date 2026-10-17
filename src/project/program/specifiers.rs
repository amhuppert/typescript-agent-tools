//! Native checker module spelling over a frozen program. The checker owns
//! symbol acquisition and its specifier cache; this module owns ordered raw
//! paths, import reuse, package mappings, and ending preferences. Loading uses
//! the same transforms to capture possible index-collision probes.
use super::paths;
use super::specifier_environment::{SpecifierEnvironment, SpecifierInputError, SpecifierOptions};
use crate::project::binding::ResolutionMode;

mod capture;
mod endings;
mod mappings;
mod module_paths;
mod packages;

pub(super) use capture::index_collision_paths;

/// Loading captures package ancestors for this raw-path union before asking
/// for index-collision paths. It uses the query's actual symlink owner.
pub(super) fn module_path_candidates(
    env: &SpecifierEnvironment,
    importers: &[String],
    targets: &[String],
) -> Result<std::collections::BTreeSet<String>, SpecifierInputError> {
    let mut paths = std::collections::BTreeSet::new();
    if importers.is_empty() {
        return Ok(paths);
    }
    let mut presence = module_paths::ImporterPresence::new(importers, env.case);
    for target in targets {
        match module_paths::for_any_importer(env, &mut presence, target) {
            Ok(candidates) => paths.extend(candidates.into_iter().map(|path| path.file_name)),
            Err(SpecifierInputError::Input(_)) => continue,
            Err(error @ SpecifierInputError::NotCaptured { .. }) => return Err(error),
        }
    }
    Ok(paths)
}

pub(in crate::project) struct WrittenImport {
    pub(in crate::project) text: String,
    pub(in crate::project) resolved_file_name: Option<String>,
    pub(in crate::project) mode: Option<ResolutionMode>,
}

pub(in crate::project) struct ImportingFile<'a> {
    pub(in crate::project) file_name: &'a str,
    pub(in crate::project) default_mode: Option<ResolutionMode>,
    pub(in crate::project) imports: &'a [WrittenImport],
}

struct Generator<'a> {
    env: &'a SpecifierEnvironment,
    importer: ImportingFile<'a>,
    /// `getSpecifierForModuleSymbol` chooses the user's Js preference from
    /// its effective cache mode. This is distinct from OverrideImportMode.
    prefer_js: bool,
    override_mode: Option<ResolutionMode>,
}

/// `GetModuleSpecifiersForFileWithInfo` / `computeModuleSpecifiers`, for the
/// checker's ProjectRelative preferences and forAutoImports=false. Project
/// references are rejected by configuration, so redirect candidates are nil.
pub(in crate::project) fn module_specifiers(
    env: &Result<SpecifierEnvironment, crate::analysis::InputError>,
    importer: ImportingFile<'_>,
    target: &str,
    preference_mode: Option<ResolutionMode>,
    override_mode: Option<ResolutionMode>,
) -> Result<Vec<String>, SpecifierInputError> {
    // This is the generation operation that consumes the host's retained
    // `Program.GetCurrentDirectory` observation. Failure stays Input/Io;
    // unrelated lexical operations never consume it.
    let env = env
        .as_ref()
        .map_err(|error| SpecifierInputError::Input(error.clone()))?;
    let generator = Generator {
        env,
        importer,
        prefer_js: preference_mode == Some(ResolutionMode::Import),
        override_mode,
    };
    let module_paths = module_paths::ordered(env, generator.importer.file_name, target)?;
    for path in &module_paths {
        let key = env.case.key(&paths::resolve(&env.cwd, &path.file_name));
        let existing = generator.importer.imports.iter().find(|import| {
            import
                .resolved_file_name
                .as_ref()
                .is_some_and(|resolved| env.case.key(&paths::resolve(&env.cwd, resolved)) == key)
        });
        if let Some(import) = existing {
            let mode = override_mode.or(generator.importer.default_mode);
            // A rejected first import advances to the next raw module path,
            // never to a second written import of the same resolved path.
            if import.mode.is_some() && mode.is_some() && import.mode != mode {
                continue;
            }
            if !import.text.is_empty() {
                return Ok(vec![import.text.clone()]);
            }
            break;
        }
    }
    let in_node_modules = module_paths.iter().any(|path| path.in_node_modules);
    let mut mapped = Vec::new();
    let mut packages = Vec::new();
    let mut relative = Vec::new();
    for path in module_paths {
        let package = if path.in_node_modules {
            generator.node_module(&path.file_name)?
        } else {
            None
        };
        let local = generator.local(&path.file_name, package.is_some())?;
        if let Some(package) = package {
            packages.push(package);
        }
        let Some(local) = local else {
            continue;
        };
        if is_bare(&local) {
            if contains_node_modules(&local) {
                relative.push(local);
            } else {
                mapped.push(local);
            }
        } else if !in_node_modules || path.in_node_modules {
            relative.push(local);
        }
    }
    Ok(if !mapped.is_empty() {
        mapped
    } else if !packages.is_empty() {
        packages
    } else {
        relative
    })
}

/// Native `PathIsBareSpecifier` and `ensurePathIsNonModuleName`.
fn is_bare(path: &str) -> bool {
    !paths::is_rooted(path) && !paths::is_relative(path)
}
fn non_module_name(path: String) -> String {
    if is_bare(&path) {
        format!("./{path}")
    } else {
        path
    }
}

/// Native `ContainsNodeModules` deliberately requires both separators.
fn contains_node_modules(path: &str) -> bool {
    path.contains("/node_modules/")
}
