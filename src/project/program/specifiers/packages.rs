use super::super::manifest::{Kind, Value};
use super::super::specifier_environment::PackageInfo;
use super::super::{ModuleResolutionKind, resolve};
use super::endings::Ending;
use super::*;

impl Generator<'_> {
    /// `GetNearestAncestorDirectoryWithPackageJson`. Malformed-but-present
    /// manifests still stop the walk; only captured absence advances it.
    pub(super) fn nearest_package(
        &self,
        directory: &str,
    ) -> Result<Option<std::borrow::Cow<'_, PackageInfo>>, SpecifierInputError> {
        for directory in paths::ancestors(directory) {
            if let Some(package) = self
                .env
                .package_json(&paths::combine(directory, "package.json"))?
            {
                return Ok(Some(package));
            }
        }
        Ok(None)
    }

    /// `tryGetModuleNameAsNodeModule` for packageNameOnly=false. The admitted
    /// program has no project-reference redirect (`ModulePath.IsRedirect`)
    /// or global typings-cache candidates; a package-id redirect is one more
    /// module path (`module_paths::ordered`).
    pub(super) fn node_module(&self, target: &str) -> Result<Option<String>, SpecifierInputError> {
        let Some(parts) = node_module_parts(target) else {
            return Ok(None);
        };
        let endings = self.endings(None);
        let mut index = parts.package_root.map_or(-1, |index| index as isize);
        let mut first_file = None;
        let module = loop {
            // Pinned native code advances index but deliberately passes the
            // original parts to every package attempt. Preserve that quirk.
            match self.package_directory(target, parts, &endings)? {
                DirectoryResult::Blocked => return Ok(None),
                DirectoryResult::Export(specifier) => return Ok(nonempty(specifier)),
                DirectoryResult::Root(root) => break root,
                DirectoryResult::File(file) => {
                    if first_file.is_none() {
                        first_file = Some(file);
                    }
                }
            }
            let start = (index + 1) as usize;
            let next = target
                .get(start..)
                .and_then(|rest| rest.find('/').map(|at| at + start));
            match next {
                Some(next) => index = next as isize,
                None => {
                    break self.ending(
                        first_file.as_deref().expect("first package attempt"),
                        &endings,
                    )?;
                }
            }
        };
        let prefix = &module[..parts.top_node_modules];
        if !paths::has_prefix(
            paths::parent(self.importer.file_name),
            prefix,
            self.env.case,
        ) {
            return Ok(None);
        }
        Ok(nonempty(types_package_name(
            &module[parts.top_package + 1..],
        )))
    }

    /// `tryDirectoryWithPackageJson`: exports are authoritative when present,
    /// followed by reverse typesVersions and the main/types/typings entry.
    fn package_directory(
        &self,
        target: &str,
        parts: NodeParts,
        endings: &[Ending],
    ) -> Result<DirectoryResult, SpecifierInputError> {
        let root = &target[..parts.package_root.unwrap_or(target.len())];
        let Some(package) = self
            .env
            .package_json(&paths::combine(root, "package.json"))?
        else {
            let file = &target[parts.package_root.map_or(0, |index| index + 1)..];
            return Ok(
                if matches!(file, "index.d.ts" | "index.js" | "index.ts" | "index.tsx") {
                    DirectoryResult::Root(root.to_owned())
                } else {
                    DirectoryResult::File(target.to_owned())
                },
            );
        };
        let contents = package.contents.as_deref();
        if self.env.options.package_exports {
            let mut mode = self.override_mode.or(self.importer.default_mode);
            if paths::has_extension(target, &[".cjs", ".cts", ".d.cts"]) {
                mode = Some(ResolutionMode::Require);
            } else if paths::has_extension(target, &[".mjs", ".mts", ".d.mts"]) {
                mode = Some(ResolutionMode::Import);
            }
            if let Some(exports) = contents.and_then(|value| value.get("exports")) {
                let conditions = self.conditions(mode);
                let name = types_package_name(&root[parts.top_package + 1..]);
                return Ok(
                    match self.exports(target, root, &name, exports, &conditions) {
                        Some(name) => DirectoryResult::Export(name),
                        None => DirectoryResult::Blocked,
                    },
                );
            }
        }
        let mut file = target.to_owned();
        let mut blocked_by_versions = false;
        let version_paths = package.version_paths(&self.env.options.typescript);
        if let Some(patterns) = &version_paths {
            // A package.json *under a single-file module* would cause the
            // native slice to panic. Such an entry cannot physically exist
            // beneath that file, so a captured file target makes it unreachable.
            // An empty name is no name here (`len(fromPaths) == 0`).
            match self.reverse_paths(&target[root.len() + 1..], patterns, endings, root)? {
                Some(name) if !name.is_empty() => file = paths::combine(root, &name),
                _ => blocked_by_versions = true,
            }
        }
        let main = contents
            .and_then(|value| {
                ["typings", "types", "main"]
                    .into_iter()
                    .find_map(|key| value.get(key).and_then(Value::as_str))
            })
            .unwrap_or("index.js");
        let main_blocked = blocked_by_versions
            && version_paths
                .as_ref()
                .is_some_and(|patterns| resolve::best_pattern(patterns, main).is_some());
        if !main.is_empty() && !main_blocked {
            let main = self.env.case.key(&paths::resolve(root, main));
            if paths::compare(
                paths::remove_file_extension(&main),
                paths::remove_file_extension(&file),
                &self.env.cwd,
                self.env.case,
            )
            .is_eq()
            {
                return Ok(DirectoryResult::Root(root.to_owned()));
            }
            // The nil-contents disjunct belongs outside the entire remaining
            // conjunction in native; a malformed present manifest chooses root.
            if contents.is_none()
                || contents
                    .and_then(|value| value.get("type"))
                    .and_then(Value::as_str)
                    != Some("module")
                    && !paths::has_extension(&file, paths::REQUIRED_EXTENSIONS)
                    && paths::has_prefix(&file, &main, self.env.case)
                    && paths::compare(
                        paths::parent(&file),
                        &paths::directory(main.clone()),
                        &self.env.cwd,
                        self.env.case,
                    )
                    .is_eq()
                    && paths::remove_file_extension(paths::basename(&file)) == "index"
            {
                return Ok(DirectoryResult::Root(root.to_owned()));
            }
        }
        Ok(DirectoryResult::File(file))
    }

    fn conditions(&self, mode: Option<ResolutionMode>) -> Vec<String> {
        resolve::resolution_conditions(
            self.env.options.module_resolution,
            &self.env.options.custom_conditions,
            mode,
        )
    }

    /// `tryGetModuleNameFromExports`: subpath entries precede the whole-value
    /// conditional fallback, in manifest order.
    fn exports(
        &self,
        target: &str,
        directory: &str,
        name: &str,
        exports: &Value,
        conditions: &[String],
    ) -> Option<String> {
        if exports.is_exports_subpaths() {
            for (key, value) in exports.entries().expect("subpaths object") {
                let subname = paths::resolve("", &paths::combine(name, key));
                if let Some(result) = self.export_or_import(
                    target,
                    directory,
                    &subname,
                    value,
                    conditions,
                    MatchMode::of(key),
                    false,
                    false,
                ) {
                    return Some(result);
                }
            }
        }
        self.export_or_import(
            target,
            directory,
            name,
            exports,
            conditions,
            MatchMode::Exact,
            false,
            false,
        )
    }

    /// `tryGetModuleNameFromPackageJsonImports` does not reject #/ entries in
    /// NodeNext/Bundler, while the literal keys # and #/ are always invalid.
    pub(super) fn package_imports(
        &self,
        target: &str,
        source_directory: &str,
        prefer_ts: bool,
    ) -> Result<Option<String>, SpecifierInputError> {
        if !self.env.options.package_imports {
            return Ok(None);
        }
        let Some(package) = self.nearest_package(source_directory)? else {
            return Ok(None);
        };
        let Some(entries) = package
            .contents
            .as_deref()
            .and_then(|value| value.get("imports"))
            .and_then(Value::entries)
        else {
            return Ok(None);
        };
        let conditions = self.conditions(self.override_mode.or(self.importer.default_mode));
        for (key, value) in entries {
            if key == "#"
                || key == "#/"
                || !key.starts_with('#')
                || key.starts_with("#/")
                    && self.env.options.module_resolution == ModuleResolutionKind::Node16
            {
                continue;
            }
            if let Some(result) = self.export_or_import(
                target,
                &package.directory,
                key,
                value,
                &conditions,
                MatchMode::of(key),
                true,
                prefer_ts,
            ) {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }

    /// `tryGetModuleNameFromExportsOrImports`. Recursion is bounded by the
    /// manifest parser's captured-input limit; invalid scalar values simply
    /// have no reverse spelling, as in the native JSONValue switch.
    #[expect(
        clippy::too_many_arguments,
        reason = "ports tryGetModuleNameFromExportsOrImports's parameters"
    )]
    fn export_or_import(
        &self,
        target: &str,
        directory: &str,
        name: &str,
        value: &Value,
        conditions: &[String],
        mode: MatchMode,
        imports: bool,
        prefer_ts: bool,
    ) -> Option<String> {
        match &value.kind {
            Kind::String(text) => {
                self.string_mapping(target, directory, name, text, mode, imports, prefer_ts)
            }
            Kind::Array(values) => values.iter().find_map(|value| {
                self.export_or_import(
                    target, directory, name, value, conditions, mode, imports, prefer_ts,
                )
            }),
            Kind::Object(values) => values
                .iter()
                .filter(|(key, _)| {
                    resolve::package_condition_applies(
                        key,
                        conditions,
                        &self.env.options.typescript,
                    )
                })
                .find_map(|(_, value)| {
                    self.export_or_import(
                        target, directory, name, value, conditions, mode, imports, prefer_ts,
                    )
                }),
            Kind::Null | Kind::Bool(_) | Kind::Number => None,
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "shares tryGetModuleNameFromExportsOrImports's string-target parameters"
    )]
    fn string_mapping(
        &self,
        target: &str,
        directory: &str,
        name: &str,
        text: &str,
        mode: MatchMode,
        imports: bool,
        prefer_ts: bool,
    ) -> Option<String> {
        let path = paths::resolve("", &paths::combine(directory, text));
        let swapped = paths::has_ts_extension(target).then(|| {
            format!(
                "{}{}",
                paths::remove_file_extension(target),
                paths::js_extension(target, self.env.options.jsx_preserve).unwrap_or("")
            )
        });
        let (output, declaration) = if imports {
            let (a, b) = self.output_paths(target);
            (Some(a), Some(b))
        } else {
            (None, None)
        };
        let can_ts = prefer_ts && paths::has_ts_extension(target) && !paths::is_declaration(target);
        let equals = |left: &str| paths::compare(left, &path, &self.env.cwd, self.env.case).is_eq();
        match mode {
            MatchMode::Exact => {
                if swapped.as_deref().is_some_and(equals)
                    || equals(target)
                    || output.as_deref().is_some_and(equals)
                    || declaration.as_deref().is_some_and(equals)
                {
                    return nonempty(name.to_owned());
                }
            }
            MatchMode::Directory => {
                let fragment =
                    |target| paths::relative_with_case(&path, target, &self.env.cwd, self.env.case);
                let joined = |target| {
                    nonempty(paths::resolve(
                        "",
                        &paths::combine(&paths::combine(name, text), &fragment(target)),
                    ))
                };
                // Native's first ContainsPath arguments are reversed. Keep
                // this distinct from the three ordinary containment checks.
                if can_ts && paths::contains(target, &path, &self.env.cwd, self.env.case) {
                    return joined(target);
                }
                if let Some(swapped) = swapped.as_deref() {
                    if paths::contains(&path, swapped, &self.env.cwd, self.env.case) {
                        return joined(swapped);
                    }
                }
                if !can_ts && paths::contains(&path, target, &self.env.cwd, self.env.case) {
                    return joined(target);
                }
                if let Some(output) = output.as_deref() {
                    if paths::contains(&path, output, &self.env.cwd, self.env.case) {
                        return nonempty(paths::combine(name, &fragment(output)));
                    }
                }
                if let Some(declaration) = declaration.as_deref() {
                    if paths::contains(&path, declaration, &self.env.cwd, self.env.case) {
                        let js = paths::js_extension(declaration, self.env.options.jsx_preserve)
                            .expect("output declaration extension");
                        return nonempty(paths::combine(
                            name,
                            &paths::change_extension(&fragment(declaration), js),
                        ));
                    }
                }
            }
            MatchMode::Pattern => {
                let (prefix, suffix) = path.split_once('*').unwrap_or((&path, ""));
                let replace = |target: &str| {
                    paths::matches_edges(target, prefix, suffix, self.env.case).then(|| {
                        name.replacen('*', &target[prefix.len()..target.len() - suffix.len()], 1)
                    })
                };
                if can_ts && let Some(result) = replace(target) {
                    return nonempty(result);
                }
                if let Some(result) = swapped.as_deref().and_then(replace) {
                    return nonempty(result);
                }
                if !can_ts && let Some(result) = replace(target) {
                    return nonempty(result);
                }
                if let Some(result) = output.as_deref().and_then(replace) {
                    return nonempty(result);
                }
                if let Some(declaration) = declaration.as_deref() {
                    if let Some(result) = replace(declaration) {
                        if let Some(js) =
                            paths::js_extension(declaration, self.env.options.jsx_preserve)
                        {
                            return nonempty(paths::change_full_extension(&result, js));
                        }
                    }
                }
            }
        }
        None
    }

    /// `GetOutputJSFileNameWorker` / `GetOutputDeclarationFileNameWorker`.
    /// These are filename transforms; noEmit/emitDeclarationOnly do not gate
    /// the worker operations used by imports reverse mapping.
    fn output_paths(&self, target: &str) -> (String, String) {
        let options = &self.env.options;
        let output = |directory: Option<&str>, extension: &str| {
            let path = directory.filter(|dir| !dir.is_empty()).map_or_else(
                || target.to_owned(),
                |dir| {
                    paths::resolve(
                        dir,
                        &paths::relative_with_case(
                            &options.common_source_directory,
                            target,
                            &self.env.cwd,
                            self.env.case,
                        ),
                    )
                },
            );
            paths::change_extension(&path, extension)
        };
        let js = output(
            options.out_dir.as_deref(),
            paths::output_extension(target, options.jsx_preserve),
        );
        let declaration = output(
            options
                .declaration_dir
                .as_deref()
                .filter(|dir| !dir.is_empty())
                .or(options.out_dir.as_deref()),
            &paths::declaration_emit_extension(target),
        );
        (js, declaration)
    }
}

enum DirectoryResult {
    File(String),
    Root(String),
    Export(String),
    Blocked,
}

#[derive(Clone, Copy)]
enum MatchMode {
    Exact,
    Directory,
    Pattern,
}
impl MatchMode {
    fn of(key: &str) -> Self {
        if key.ends_with('/') {
            Self::Directory
        } else if key.contains('*') {
            Self::Pattern
        } else {
            Self::Exact
        }
    }
}
fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

#[derive(Clone, Copy)]
pub(super) struct NodeParts {
    top_node_modules: usize,
    top_package: usize,
    pub(super) package_root: Option<usize>,
}

/// `GetNodeModulePathParts`: the top-level node_modules prefix remains fixed
/// while nested node_modules packages replace the final package-root index.
pub(super) fn node_module_parts(path: &str) -> Option<NodeParts> {
    #[derive(PartialEq, PartialOrd)]
    enum State {
        Before,
        NodeModules,
        Scope,
        Content,
    }
    let mut state = State::Before;
    let mut top_node_modules = 0;
    let mut top_package = 0;
    let mut package_root = Some(0);
    let mut end = Some(0);
    while let Some(start) = end {
        end = path
            .get(start + 1..)
            .and_then(|rest| rest.find('/').map(|at| at + start + 1));
        match state {
            State::Before if path[start..].starts_with("/node_modules/") => {
                top_node_modules = start;
                top_package = end.expect("node_modules has a trailing separator");
                state = State::NodeModules;
            }
            State::Before => {}
            State::NodeModules | State::Scope => {
                if state == State::NodeModules && path.as_bytes().get(start + 1) == Some(&b'@') {
                    state = State::Scope;
                } else {
                    package_root = end;
                    state = State::Content;
                }
            }
            State::Content if path[start..].starts_with("/node_modules/") => {
                state = State::NodeModules
            }
            State::Content => {}
        }
    }
    (state > State::NodeModules).then_some(NodeParts {
        top_node_modules,
        top_package,
        package_root,
    })
}

/// `GetPackageNameFromTypesPackageName` / `UnmangleScopedPackageName`.
fn types_package_name(name: &str) -> String {
    let Some(name) = name.strip_prefix("@types/") else {
        return name.to_owned();
    };
    if name.contains("__") {
        format!("@{}", name.replacen("__", "/", 1))
    } else {
        name.to_owned()
    }
}
