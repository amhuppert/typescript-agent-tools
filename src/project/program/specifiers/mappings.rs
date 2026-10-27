use super::endings::Ending;
use super::*;
use crate::input::Case;

impl Generator<'_> {
    /// `getLocalModuleSpecifier`, for RelativePreferenceExternalNonRelative.
    /// A package spelling can request pathsOnly; the relative candidate still
    /// runs first whenever the native pathsOnly/paths gate admits the worker.
    /// `baseUrl` follows 5.6.3's `getLocalModuleSpecifier`: it admits the
    /// worker, is the base, and its path is the non-relative name when no
    /// package import or `paths` name applies.
    pub(super) fn local(
        &self,
        target: &str,
        paths_only: bool,
    ) -> Result<Option<String>, SpecifierInputError> {
        let options = &self.env.options;
        if paths_only && options.paths.is_none() {
            return Ok(None);
        }
        let directory = paths::parent(self.importer.file_name);
        let endings = self.endings(self.override_mode.or(self.importer.default_mode));
        let unprocessed = root_dirs_relative(target, directory, self.env).unwrap_or_else(|| {
            non_module_name(paths::relative_with_case(
                directory,
                target,
                &self.env.cwd,
                self.env.case,
            ))
        });
        let relative = self.ending(&unprocessed, &endings)?;
        if options.paths.is_none() && options.base_url.is_none() && !options.package_imports {
            return Ok((!paths_only).then_some(relative));
        }
        let base = local_base(options, &self.env.cwd);
        let relative_to_base = same_volume_relative(target, &base, self.env.case);
        if relative_to_base.is_empty() {
            return Ok((!paths_only).then_some(relative));
        }
        let imports = if paths_only {
            None
        } else {
            self.package_imports(target, directory, endings::prefers_ts(&endings))?
        };
        let from_paths = if paths_only || imports.is_none() {
            match &options.paths {
                Some(paths) => {
                    self.reverse_paths(&relative_to_base, &paths.patterns, &endings, &base)?
                }
                None => None,
            }
        } else {
            None
        };
        if paths_only {
            return Ok(from_paths.filter(|name| !name.is_empty()));
        }
        // `maybeNonRelative`. Only an unmatched `paths` lookup gives way to
        // the path from `baseUrl`; an empty `paths` name leaves no
        // non-relative name. 5.6.3 processes the path from `baseUrl` without
        // a host: an `index` file loses `/index` even beside a file that
        // would collide, and the name asks nothing of the file system.
        let non_relative = match (imports, from_paths) {
            (Some(name), _) | (None, Some(name)) => name,
            (None, None) if options.base_url.is_some() => {
                let result: Result<_, std::convert::Infallible> =
                    endings::process(&relative_to_base, &endings, options, |_| Ok(false));
                result.expect("no collision question")
            }
            (None, None) => String::new(),
        };
        if non_relative.is_empty() {
            return Ok(Some(relative));
        }
        if !paths::is_relative(&non_relative) {
            let project = if self.env.config_file.is_empty() {
                self.env.cwd.as_str()
            } else {
                paths::parent(&self.env.config_file)
            };
            let project = self.env.case.key(&paths::resolve(&self.env.cwd, project));
            let source = self.env.case.key(&paths::resolve(&self.env.cwd, directory));
            let module = self.env.case.key(&paths::resolve(&project, target));
            if self.env.case.contains(&project, &source)
                != self.env.case.contains(&project, &module)
            {
                return Ok(Some(non_relative));
            }
            let target_package = self.nearest_package(paths::parent(&module))?;
            let source_package = self.nearest_package(directory)?;
            let equal = match (target_package, source_package) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    paths::compare(&a.directory, &b.directory, &self.env.cwd, self.env.case).is_eq()
                }
                _ => false,
            };
            return Ok(Some(if equal { relative } else { non_relative }));
        }
        Ok(Some(
            if non_relative.starts_with("..")
                || paths::count_path_components(&relative)
                    < paths::count_path_components(&non_relative)
            {
                relative
            } else {
                non_relative
            },
        ))
    }

    pub(super) fn reverse_paths(
        &self,
        target: &str,
        patterns: &[(String, Vec<String>)],
        endings: &[Ending],
        base: &str,
    ) -> Result<Option<String>, SpecifierInputError> {
        reverse_paths(
            target,
            patterns,
            endings,
            base,
            self.env.case,
            |file, endings| self.ending(file, endings),
        )
    }
}

/// `getLocalModuleSpecifier`'s base directory, which loading's capture
/// shares: `baseUrl` when set, else the `paths` base when there are
/// patterns (`getPathsBasePath`), else the working directory.
pub(super) fn local_base(options: &SpecifierOptions, cwd: &str) -> String {
    let base = options.base_url.as_deref().or_else(|| {
        options
            .paths
            .as_ref()
            .filter(|paths| !paths.patterns.is_empty())
            .map(|paths| paths.base.as_str())
    });
    paths::resolve(cwd, base.unwrap_or(cwd))
}

/// `getRelativePathIfInSameVolume`: both paths are interpreted relative to
/// the directory argument. Rooted results mean different volumes.
pub(super) fn same_volume_relative(path: &str, directory: &str, case: Case) -> String {
    let relative = paths::relative_with_case(directory, path, directory, case);
    if paths::is_rooted_disk(&relative) {
        String::new()
    } else {
        relative
    }
}

/// `tryGetModuleNameFromRootDirs` before processEnding. The first candidate
/// with the fewest literal '/' separators wins; rootDirs order breaks ties.
pub(super) fn root_dirs_relative(
    target: &str,
    directory: &str,
    env: &SpecifierEnvironment,
) -> Option<String> {
    let relative_roots = |path: &str| {
        env.options
            .root_dirs
            .iter()
            .map(|root| same_volume_relative(path, root, env.case))
            .filter(|path| !path.starts_with(".."))
            .collect::<Vec<_>>()
    };
    let targets = relative_roots(target);
    let sources = relative_roots(directory);
    let mut shortest: Option<(String, usize)> = None;
    for source in sources {
        for target in &targets {
            let candidate = non_module_name(paths::relative_with_case(
                &source, target, &env.cwd, env.case,
            ));
            let count = candidate.bytes().filter(|byte| *byte == b'/').count();
            if shortest.as_ref().is_none_or(|(_, best)| count < *best) {
                shortest = Some((candidate, count));
            }
        }
    }
    shortest.map(|(path, _)| path)
}

/// `tryGetModuleNameFromPaths` / `validateEnding`. Literal (non-star)
/// patterns compare exactly even on insensitive hosts; star boundaries use
/// native stringutil's case-aware prefix/suffix operation. The first match
/// ends the search with its name, which is empty for an empty key or for a
/// `*` key whose star matched nothing; `None` means no pattern matched.
pub(super) fn reverse_paths<E>(
    target: &str,
    patterns: &[(String, Vec<String>)],
    endings: &[Ending],
    base: &str,
    case: Case,
    mut process: impl FnMut(&str, &[Ending]) -> Result<String, E>,
) -> Result<Option<String>, E> {
    for (key, values) in patterns {
        for pattern in values {
            let normalized = paths::normalize(pattern);
            let relative = same_volume_relative(&normalized, base, case);
            let pattern = if relative.is_empty() {
                normalized.as_str()
            } else {
                relative.as_str()
            };
            let mut candidates = Vec::new();
            for ending in endings {
                candidates.push((*ending, process(target, &[*ending])?));
            }
            if paths::try_extension(pattern).is_some() {
                candidates.push((Ending::Js, target.to_owned()));
            }
            if let Some((prefix, suffix)) = pattern.split_once('*') {
                for (ending, value) in candidates {
                    if paths::matches_edges(&value, prefix, suffix, case)
                        && (ending != Ending::Minimal
                            || value == process(target, &[Ending::Minimal])?)
                    {
                        let star = &value[prefix.len()..value.len() - suffix.len()];
                        if !paths::is_relative(star) {
                            return Ok(Some(key.replacen('*', star, 1)));
                        }
                    }
                }
            } else if candidates
                .iter()
                .any(|(ending, value)| *ending != Ending::Minimal && value == pattern)
            {
                return Ok(Some(key.clone()));
            } else {
                for (ending, value) in candidates {
                    if ending == Ending::Minimal
                        && value == pattern
                        && value == process(target, &[Ending::Minimal])?
                    {
                        return Ok(Some(key.clone()));
                    }
                }
            }
        }
    }
    Ok(None)
}
