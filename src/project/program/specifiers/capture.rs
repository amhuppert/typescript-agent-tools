//! Loading's finite closure of possible processEnding filesystem questions.
//! The query retains raw files/packages/symlink relations, not a quadratic
//! matrix of final module strings. Each unique physical probe is stored once.
use super::endings::Ending;
use super::*;
use std::collections::BTreeSet;

/// Enumerate collision paths after the same relative/rootDirs/paths/package
/// transforms used by generation. Both possible collision answers participate
/// when typesVersions changes the filename that a later processEnding sees.
/// Extension appending and cwd resolution stay inside the shared ending owner.
///
/// Each target's module paths are the union over all importers, and only the
/// relative and rootDirs spellings depend on the importer, through its
/// directory alone. Those spellings keep the file's final component, so they
/// ask the collision question exactly when the file itself does. The probes
/// may therefore include ones a particular importer never asks; each is a
/// recorded physical observation. Ordinary relative spellings are constructed
/// once per distinct path shape, rather than once per importer directory.
/// Comparisons still visit directory/target pairs, and rootDirs keeps its
/// per-pair transforms and order-sensitive choice of spelling.
pub(in crate::project::program) fn index_collision_paths(
    env: &SpecifierEnvironment,
    importers: &[String],
    targets: &[String],
) -> Result<BTreeSet<String>, SpecifierInputError> {
    // Spellings are deduplicated before the pure ending transform: many
    // importer directories share one relative spelling of a target.
    let mut spellings = BTreeSet::new();
    if importers.is_empty() {
        return Ok(spellings);
    }
    let directories: BTreeSet<&str> = importers
        .iter()
        .map(|importer| paths::parent(importer))
        .collect();
    let relatives = paths::RelativePaths::new(directories.iter().copied(), &env.cwd, env.case);
    let mut presence = module_paths::ImporterPresence::new(importers, env.case);
    for target in targets {
        let module_paths = match module_paths::for_any_importer(env, &mut presence, target) {
            Ok(paths) => paths,
            Err(SpecifierInputError::Input(_)) => continue,
            Err(error @ SpecifierInputError::NotCaptured { .. }) => return Err(error),
        };
        for path in module_paths {
            let target = &path.file_name;
            if asks_collision(env, target) {
                spellings.extend(relatives.to(target).into_iter().map(non_module_name));
                if !env.options.root_dirs.is_empty() {
                    for &directory in &directories {
                        if let Some(rooted) = mappings::root_dirs_relative(target, directory, env) {
                            spellings.insert(rooted);
                        }
                    }
                }
            }
            if let Some(paths) = &env.options.paths {
                let base = if paths.patterns.is_empty() {
                    &env.cwd
                } else {
                    &paths.base
                };
                spellings.insert(mappings::same_volume_relative(target, base, env.case));
            }
            if !path.in_node_modules {
                continue;
            }
            spellings.insert(target.clone());
            let Some(parts) = packages::node_module_parts(target) else {
                continue;
            };
            let root = &target[..parts.package_root.unwrap_or(target.len())];
            let package = match env.package_json(&paths::combine(root, "package.json")) {
                Ok(Some(package)) => package,
                Ok(None) | Err(SpecifierInputError::Input(_)) => continue,
                Err(error @ SpecifierInputError::NotCaptured { .. }) => return Err(error),
            };
            let Some(patterns) = package.version_paths() else {
                continue;
            };
            let subpath = &target[root.len() + 1..];
            spellings.insert(subpath.to_owned());
            for endings in ENDING_ORDERS {
                for collision in [false, true] {
                    let result: Result<_, std::convert::Infallible> = mappings::reverse_paths(
                        subpath,
                        &patterns,
                        endings,
                        root,
                        env.case,
                        |file, endings| {
                            endings::process(file, endings, &env.options, |_| Ok(collision))
                        },
                    );
                    if let Some(name) = result.expect("pure ending transform") {
                        spellings.insert(paths::combine(root, &name));
                    }
                }
            }
        }
    }
    let mut probes = BTreeSet::new();
    for spelling in &spellings {
        collect(env, spelling, &mut probes);
    }
    Ok(probes)
}

/// Whether `processEnding` asks its collision question for `file` at all.
fn asks_collision(env: &SpecifierEnvironment, file: &str) -> bool {
    let mut asked = false;
    let result: Result<_, std::convert::Infallible> =
        endings::process(file, &[Ending::Minimal], &env.options, |_| {
            asked = true;
            Ok(false)
        });
    result.expect("capture ending transform");
    asked
}

fn collect(env: &SpecifierEnvironment, file: &str, probes: &mut BTreeSet<String>) {
    let result: Result<_, std::convert::Infallible> =
        endings::process(file, &[Ending::Minimal], &env.options, |stem| {
            probes.extend(endings::collision_files(stem, &env.cwd));
            Ok(false)
        });
    result.expect("capture ending transform");
}

// Complete native allowed-ending orders, including Index even though the
// checker's user preference cannot request it. No final spelling is retained.
const ENDING_ORDERS: &[&[Ending]] = &[
    &[Ending::Ts, Ending::Js],
    &[Ending::Js],
    &[Ending::Js, Ending::Ts, Ending::Minimal, Ending::Index],
    &[Ending::Js, Ending::Minimal, Ending::Index],
    &[Ending::Ts, Ending::Minimal, Ending::Js, Ending::Index],
    &[Ending::Index, Ending::Minimal, Ending::Ts, Ending::Js],
    &[Ending::Index, Ending::Minimal, Ending::Js],
    &[Ending::Minimal, Ending::Index, Ending::Ts, Ending::Js],
    &[Ending::Minimal, Ending::Index, Ending::Js],
];
