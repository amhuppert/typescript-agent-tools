use super::super::specifier_environment::ignored_path;
use super::*;
use crate::input::Case;
use std::collections::{BTreeMap, HashMap, HashSet};

pub(super) struct ModulePath {
    pub(super) file_name: String,
    pub(super) in_node_modules: bool,
}

/// `GetEachFileNameOfModule` / `getAllModulePathsWorker`. Project-reference
/// redirects and the global typings cache are absent in the admitted program.
/// Symlink alternatives retain raw spellings independently of source identity.
pub(super) fn ordered(
    env: &SpecifierEnvironment,
    importer: &str,
    target: &str,
) -> Result<Vec<ModulePath>, SpecifierInputError> {
    let target = paths::resolve(&env.cwd, target);
    let symlinks = env.symlinks.as_ref().map_err(Clone::clone)?;
    let mut all = BTreeMap::new();
    let mut filter_ignored = !ignored_path(&target);
    for real in paths::ancestors(paths::parent(&target)) {
        let key = format!("{}/", real.trim_end_matches('/'));
        let Some(aliases) = symlinks.directories_by_realpath(&key, env.case) else {
            continue;
        };
        // Native deliberately tests this only after finding a symlink set.
        if paths::starts_with_directory(importer, real, env.case) {
            break;
        }
        if !paths::starts_with_directory(&target, real, env.case) {
            continue;
        }
        let relative = paths::relative_with_case(real, &target, &env.cwd, env.case);
        for alias in aliases {
            let name = paths::resolve(alias, &relative);
            all.insert(
                name.clone(),
                ModulePath {
                    in_node_modules: contains_node_modules(&name),
                    file_name: name,
                },
            );
            filter_ignored = true;
        }
    }
    if !(filter_ignored && ignored_path(&target)) {
        all.insert(
            target.clone(),
            ModulePath {
                in_node_modules: contains_node_modules(&target),
                file_name: target,
            },
        );
    }
    let compare = |a: &ModulePath, b: &ModulePath| {
        a.file_name
            .bytes()
            .filter(|c| *c == b'/')
            .count()
            .cmp(&b.file_name.bytes().filter(|c| *c == b'/').count())
            .then_with(|| paths::compare(&a.file_name, &b.file_name, "", env.case))
    };
    let mut result = Vec::new();
    for directory in paths::ancestors(paths::parent(importer)) {
        let prefix = format!("{}/", directory.trim_end_matches('/'));
        // This grouping prefix is case-sensitive in native, even on a
        // case-insensitive host. Sorting applies the host's separate policy.
        let keys: Vec<_> = all
            .keys()
            .filter(|name| name.starts_with(&prefix))
            .cloned()
            .collect();
        let mut group: Vec<_> = keys
            .into_iter()
            .map(|key| all.remove(&key).expect("collected key"))
            .collect();
        group.sort_by(compare);
        result.extend(group);
    }
    let mut rest: Vec<_> = all.into_values().collect();
    rest.sort_by(compare);
    result.extend(rest);
    Ok(result)
}

/// Whether any importer is inside, and whether any is outside, a directory,
/// as `ordered`'s `starts_with_directory(importer, real)` break asks it.
/// That test compares a directory prefix ending in a separator, so importers
/// sharing their text through the last separator always answer alike; one
/// representative of each such prefix decides for all of them.
pub(super) struct ImporterPresence<'a> {
    representatives: Vec<&'a str>,
    case: Case,
    answers: HashMap<String, (bool, bool)>,
}

impl<'a> ImporterPresence<'a> {
    pub(super) fn new(importers: &'a [String], case: Case) -> Self {
        let mut prefixes = HashSet::new();
        let representatives = importers
            .iter()
            .map(String::as_str)
            .filter(|importer| {
                let end = importer.rfind(['/', '\\']).map_or(0, |at| at + 1);
                prefixes.insert(&importer[..end])
            })
            .collect();
        Self {
            representatives,
            case,
            answers: HashMap::new(),
        }
    }

    fn of(&mut self, directory: &str) -> (bool, bool) {
        if let Some(&answer) = self.answers.get(directory) {
            return answer;
        }
        let mut answer = (false, false);
        for importer in &self.representatives {
            if paths::starts_with_directory(importer, directory, self.case) {
                answer.0 = true;
            } else {
                answer.1 = true;
            }
            if answer == (true, true) {
                break;
            }
        }
        self.answers.insert(directory.to_owned(), answer);
        answer
    }
}

/// The union of `ordered`'s paths for `target` over every importer, without
/// visiting each importer. Ancestors of the target are nested, and an
/// importer stops at the first symlinked ancestor containing it, so a level's
/// aliases appear for some importer exactly when some importer is outside
/// that level. An ignored target is dropped only for an importer that added
/// an alias, which every importer does unless one is inside the first
/// symlinked level. Order is not retained; loading only needs the set.
pub(super) fn for_any_importer(
    env: &SpecifierEnvironment,
    importers: &mut ImporterPresence<'_>,
    target: &str,
) -> Result<Vec<ModulePath>, SpecifierInputError> {
    let target = paths::resolve(&env.cwd, target);
    let symlinks = env.symlinks.as_ref().map_err(Clone::clone)?;
    let mut names = BTreeMap::new();
    let mut first_level_inside = None;
    for real in paths::ancestors(paths::parent(&target)) {
        let key = format!("{}/", real.trim_end_matches('/'));
        let Some(aliases) = symlinks.directories_by_realpath(&key, env.case) else {
            continue;
        };
        let (inside, outside) = importers.of(real);
        if !paths::starts_with_directory(&target, real, env.case) {
            // `ordered` breaks here for an inside importer; that importer is
            // inside every later (enclosing) level too.
            continue;
        }
        first_level_inside.get_or_insert(inside);
        if !outside {
            continue;
        }
        let relative = paths::relative_with_case(real, &target, &env.cwd, env.case);
        for alias in aliases {
            let name = paths::resolve(alias, &relative);
            names.insert(name.clone(), contains_node_modules(&name));
        }
    }
    if !ignored_path(&target) || first_level_inside != Some(false) {
        names.insert(target.clone(), contains_node_modules(&target));
    }
    Ok(names
        .into_iter()
        .map(|(file_name, in_node_modules)| ModulePath {
            file_name,
            in_node_modules,
        })
        .collect())
}
