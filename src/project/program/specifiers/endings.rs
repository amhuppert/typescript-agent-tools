use super::super::ModuleResolutionKind;
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Ending {
    Minimal,
    Index,
    Js,
    Ts,
}

impl Generator<'_> {
    /// `GetAllowedEndingsInPreferredOrder`, with the checker's fixed user
    /// preference (Js or None) and no old import specifier.
    pub(super) fn endings(&self, syntax_mode: Option<ResolutionMode>) -> Vec<Ending> {
        let options = &self.env.options;
        let node = options.module_resolution != ModuleResolutionKind::Bundler;
        let allow =
            options.allow_importing_ts_extensions || options.rewrite_relative_import_extensions;
        let declaration = paths::is_declaration(self.importer.file_name);
        let mode = syntax_mode.or(self.importer.default_mode);
        if syntax_mode == Some(ResolutionMode::Import) && node {
            return if allow || declaration {
                vec![Ending::Ts, Ending::Js]
            } else {
                vec![Ending::Js]
            };
        }
        let preferred = if self.prefer_js || mode == Some(ResolutionMode::Import) && node {
            if !allow || self.infer_ending(mode, node) == Ending::Js {
                Ending::Js
            } else {
                Ending::Ts
            }
        } else if !allow {
            if self.uses_extensions() {
                Ending::Js
            } else {
                Ending::Minimal
            }
        } else {
            self.infer_ending(mode, node)
        };
        match preferred {
            Ending::Js if allow || declaration => {
                vec![Ending::Js, Ending::Ts, Ending::Minimal, Ending::Index]
            }
            Ending::Js => vec![Ending::Js, Ending::Minimal, Ending::Index],
            Ending::Ts => vec![Ending::Ts, Ending::Minimal, Ending::Js, Ending::Index],
            Ending::Minimal if allow || declaration => {
                vec![Ending::Minimal, Ending::Index, Ending::Ts, Ending::Js]
            }
            Ending::Minimal => vec![Ending::Minimal, Ending::Index, Ending::Js],
            Ending::Index => unreachable!("checker preferences never request Index"),
        }
    }

    /// `usesExtensionsOnImports` returns at the first eligible relative
    /// import, even if that import has neither a TS nor a JS extension.
    fn uses_extensions(&self) -> bool {
        self.importer
            .imports
            .iter()
            .find(|import| {
                paths::is_relative(&import.text)
                    && !paths::has_extension(&import.text, paths::REQUIRED_EXTENSIONS)
            })
            .is_some_and(|import| {
                paths::has_ts_extension(&import.text) || paths::has_js_extension(&import.text)
            })
    }

    /// Native `inferPreference` currently skips *every* relative import in
    /// CommonJS Node mode, including its documented unimplemented usage gate.
    fn infer_ending(&self, mode: Option<ResolutionMode>, node: bool) -> Ending {
        let mut js = false;
        for import in self.importer.imports {
            let text = &import.text;
            if !paths::is_relative(text)
                || node && mode == Some(ResolutionMode::Require)
                || paths::has_extension(text, paths::REQUIRED_EXTENSIONS)
            {
                continue;
            }
            if paths::has_ts_extension(text) {
                return Ending::Ts;
            }
            js |= paths::has_js_extension(text);
        }
        if js { Ending::Js } else { Ending::Minimal }
    }

    pub(super) fn ending(
        &self,
        file: &str,
        endings: &[Ending],
    ) -> Result<String, SpecifierInputError> {
        process(file, endings, &self.env.options, |stem| {
            for file in collision_files(stem, &self.env.cwd) {
                if self.env.file_exists(&file)? {
                    return Ok(true);
                }
            }
            Ok(false)
        })
    }
}

/// Native `tryGetAnyFileFromPath`: GetSupportedExtensions(AllowJs=true)
/// filters the extra `node`/`json` entries because their ScriptKinds are not
/// JS-like. The order matters when an earlier positive probe hides an error.
pub(super) const COLLISION_EXTENSIONS: &[&str] = &[
    ".ts", ".tsx", ".d.ts", ".js", ".jsx", ".cts", ".d.cts", ".cjs", ".mts", ".d.mts", ".mjs",
];

/// Append first, then resolve against cwd. Resolving the bare stem first
/// changes native probes such as `..` + `.ts` into a different filename.
pub(super) fn collision_files<'a>(
    stem: &'a str,
    cwd: &'a str,
) -> impl Iterator<Item = String> + 'a {
    COLLISION_EXTENSIONS
        .iter()
        .map(move |extension| paths::resolve(cwd, &format!("{stem}{extension}")))
}

pub(super) fn prefers_ts(endings: &[Ending]) -> bool {
    let ts = priority(endings, Ending::Ts);
    ts >= 0 && ts < priority(endings, Ending::Js)
}
fn priority(endings: &[Ending], ending: Ending) -> isize {
    endings
        .iter()
        .position(|item| *item == ending)
        .map_or(-1, |index| index as isize)
}

/// `processEnding`. Its only physical dependency is the minimal-ending
/// collision question; loading enumerates that same question and query-time
/// generation consumes the frozen answer. The callback keeps their path
/// transforms identical without giving the query an I/O host.
pub(super) fn process<E>(
    file: &str,
    endings: &[Ending],
    options: &SpecifierOptions,
    mut collision: impl FnMut(&str) -> Result<bool, E>,
) -> Result<String, E> {
    if paths::has_extension(file, &[".json", ".mjs", ".cjs"]) {
        return Ok(file.to_owned());
    }
    let bare = paths::remove_file_extension(file);
    if bare == file {
        return Ok(file.to_owned());
    }
    if paths::has_extension(file, &[".mts", ".cts"]) && prefers_ts(endings) {
        return Ok(file.to_owned());
    }
    if paths::has_extension(file, &[".d.mts", ".d.cts", ".mts", ".cts"]) {
        return Ok(format!(
            "{bare}{}",
            paths::js_extension(file, options.jsx_preserve).expect("recognized native extension")
        ));
    }
    if let Some(real) = real_non_js_declaration(file) {
        return Ok(real);
    }
    Ok(match endings[0] {
        Ending::Minimal => {
            let without_index = bare.strip_suffix("/index").unwrap_or(bare);
            if without_index != bare && collision(without_index)? {
                bare.to_owned()
            } else {
                without_index.to_owned()
            }
        }
        Ending::Index => bare.to_owned(),
        Ending::Js => format!(
            "{bare}{}",
            paths::js_extension(file, options.jsx_preserve).expect("recognized native extension")
        ),
        Ending::Ts if paths::is_declaration(file) => {
            let extensionless = endings
                .iter()
                .position(|e| matches!(e, Ending::Minimal | Ending::Index));
            if extensionless.is_some_and(|i| (i as isize) < priority(endings, Ending::Js)) {
                bare.to_owned()
            } else {
                format!(
                    "{bare}{}",
                    paths::js_extension(file, options.jsx_preserve)
                        .expect("recognized native extension")
                )
            }
        }
        Ending::Ts => file.to_owned(),
    })
}

/// `TryGetRealFileNameForNonJSDeclarationFileName`. Native finds the first
/// `.d.` in the *whole path* after checking for one in its basename.
fn real_non_js_declaration(file: &str) -> Option<String> {
    if !file.ends_with(".ts") || file.ends_with(".d.ts") || !paths::basename(file).contains(".d.") {
        return None;
    }
    let bare = &file[..file.len() - 3];
    let extension = &bare[bare.rfind('.')?..];
    Some(format!("{}{extension}", bare.split_once(".d.")?.0))
}
