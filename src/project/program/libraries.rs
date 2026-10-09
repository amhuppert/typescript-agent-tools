//! Standard library declarations as TypeScript 7.0.2 selects them. The
//! catalog ports `tsoptions.LibMap`, `tsoptions.GetLibFileName`, and
//! `tsoptions.GetDefaultLibFileName`; selection ports the library block of
//! `compiler.processAllProgramFiles` and the `/// <reference lib>` branch of
//! `parseTask.load`; replacement lookup ports `fileLoader.pathForLibFile`;
//! program order ports `fileLoader.getDefaultLibFilePriority`.
//!
//! Configuration decodes the options once (`config::convert`); the loader
//! captures the files this module names through the ordinary input path.
//! The declarations are the pinned compiler's own, vendored in this checkout
//! with their license and provenance. The catalog's unit tests check it
//! against that provenance, which records `LibMap` from the compiler source
//! at the pinned tag.

use super::config::Options;
use super::paths;

/// The vendored declarations, as the build's checkout spelled them: the
/// compiler's `defaultLibraryPath`. Nothing searches for another copy or
/// probes this one; a missing or unreadable file is an I/O failure where
/// loading reads it, and moving the checkout requires rebuilding.
pub(super) fn directory() -> String {
    paths::native_text(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/vendor/typescript-7.0.2-lib"
    ))
}

/// `tsoptions.LibMap`: each library name and the file it selects, in
/// declaration order, which is also the order of `tsoptions.Libs`.
const LIB_MAP: [(&str, &str); 107] = [
    ("es5", "lib.es5.d.ts"),
    ("es6", "lib.es2015.d.ts"),
    ("es2015", "lib.es2015.d.ts"),
    ("es7", "lib.es2016.d.ts"),
    ("es2016", "lib.es2016.d.ts"),
    ("es2017", "lib.es2017.d.ts"),
    ("es2018", "lib.es2018.d.ts"),
    ("es2019", "lib.es2019.d.ts"),
    ("es2020", "lib.es2020.d.ts"),
    ("es2021", "lib.es2021.d.ts"),
    ("es2022", "lib.es2022.d.ts"),
    ("es2023", "lib.es2023.d.ts"),
    ("es2024", "lib.es2024.d.ts"),
    ("es2025", "lib.es2025.d.ts"),
    ("esnext", "lib.esnext.d.ts"),
    ("dom", "lib.dom.d.ts"),
    ("dom.iterable", "lib.dom.iterable.d.ts"),
    ("dom.asynciterable", "lib.dom.asynciterable.d.ts"),
    ("webworker", "lib.webworker.d.ts"),
    (
        "webworker.importscripts",
        "lib.webworker.importscripts.d.ts",
    ),
    ("webworker.iterable", "lib.webworker.iterable.d.ts"),
    (
        "webworker.asynciterable",
        "lib.webworker.asynciterable.d.ts",
    ),
    ("scripthost", "lib.scripthost.d.ts"),
    ("es2015.core", "lib.es2015.core.d.ts"),
    ("es2015.collection", "lib.es2015.collection.d.ts"),
    ("es2015.generator", "lib.es2015.generator.d.ts"),
    ("es2015.iterable", "lib.es2015.iterable.d.ts"),
    ("es2015.promise", "lib.es2015.promise.d.ts"),
    ("es2015.proxy", "lib.es2015.proxy.d.ts"),
    ("es2015.reflect", "lib.es2015.reflect.d.ts"),
    ("es2015.symbol", "lib.es2015.symbol.d.ts"),
    (
        "es2015.symbol.wellknown",
        "lib.es2015.symbol.wellknown.d.ts",
    ),
    ("es2016.array.include", "lib.es2016.array.include.d.ts"),
    ("es2016.intl", "lib.es2016.intl.d.ts"),
    ("es2017.arraybuffer", "lib.es2017.arraybuffer.d.ts"),
    ("es2017.date", "lib.es2017.date.d.ts"),
    ("es2017.object", "lib.es2017.object.d.ts"),
    ("es2017.sharedmemory", "lib.es2017.sharedmemory.d.ts"),
    ("es2017.string", "lib.es2017.string.d.ts"),
    ("es2017.intl", "lib.es2017.intl.d.ts"),
    ("es2017.typedarrays", "lib.es2017.typedarrays.d.ts"),
    ("es2018.asyncgenerator", "lib.es2018.asyncgenerator.d.ts"),
    ("es2018.asynciterable", "lib.es2018.asynciterable.d.ts"),
    ("es2018.intl", "lib.es2018.intl.d.ts"),
    ("es2018.promise", "lib.es2018.promise.d.ts"),
    ("es2018.regexp", "lib.es2018.regexp.d.ts"),
    ("es2019.array", "lib.es2019.array.d.ts"),
    ("es2019.object", "lib.es2019.object.d.ts"),
    ("es2019.string", "lib.es2019.string.d.ts"),
    ("es2019.symbol", "lib.es2019.symbol.d.ts"),
    ("es2019.intl", "lib.es2019.intl.d.ts"),
    ("es2020.bigint", "lib.es2020.bigint.d.ts"),
    ("es2020.date", "lib.es2020.date.d.ts"),
    ("es2020.promise", "lib.es2020.promise.d.ts"),
    ("es2020.sharedmemory", "lib.es2020.sharedmemory.d.ts"),
    ("es2020.string", "lib.es2020.string.d.ts"),
    (
        "es2020.symbol.wellknown",
        "lib.es2020.symbol.wellknown.d.ts",
    ),
    ("es2020.intl", "lib.es2020.intl.d.ts"),
    ("es2020.number", "lib.es2020.number.d.ts"),
    ("es2021.promise", "lib.es2021.promise.d.ts"),
    ("es2021.string", "lib.es2021.string.d.ts"),
    ("es2021.weakref", "lib.es2021.weakref.d.ts"),
    ("es2021.intl", "lib.es2021.intl.d.ts"),
    ("es2022.array", "lib.es2022.array.d.ts"),
    ("es2022.error", "lib.es2022.error.d.ts"),
    ("es2022.intl", "lib.es2022.intl.d.ts"),
    ("es2022.object", "lib.es2022.object.d.ts"),
    ("es2022.string", "lib.es2022.string.d.ts"),
    ("es2022.regexp", "lib.es2022.regexp.d.ts"),
    ("es2023.array", "lib.es2023.array.d.ts"),
    ("es2023.collection", "lib.es2023.collection.d.ts"),
    ("es2023.intl", "lib.es2023.intl.d.ts"),
    ("es2024.arraybuffer", "lib.es2024.arraybuffer.d.ts"),
    ("es2024.collection", "lib.es2024.collection.d.ts"),
    ("es2024.object", "lib.es2024.object.d.ts"),
    ("es2024.promise", "lib.es2024.promise.d.ts"),
    ("es2024.regexp", "lib.es2024.regexp.d.ts"),
    ("es2024.sharedmemory", "lib.es2024.sharedmemory.d.ts"),
    ("es2024.string", "lib.es2024.string.d.ts"),
    ("es2025.collection", "lib.es2025.collection.d.ts"),
    ("es2025.float16", "lib.es2025.float16.d.ts"),
    ("es2025.intl", "lib.es2025.intl.d.ts"),
    ("es2025.iterator", "lib.es2025.iterator.d.ts"),
    ("es2025.promise", "lib.es2025.promise.d.ts"),
    ("es2025.regexp", "lib.es2025.regexp.d.ts"),
    ("esnext.asynciterable", "lib.es2018.asynciterable.d.ts"),
    ("esnext.symbol", "lib.es2019.symbol.d.ts"),
    ("esnext.bigint", "lib.es2020.bigint.d.ts"),
    ("esnext.weakref", "lib.es2021.weakref.d.ts"),
    ("esnext.object", "lib.es2024.object.d.ts"),
    ("esnext.regexp", "lib.es2024.regexp.d.ts"),
    ("esnext.string", "lib.es2024.string.d.ts"),
    ("esnext.float16", "lib.es2025.float16.d.ts"),
    ("esnext.iterator", "lib.es2025.iterator.d.ts"),
    ("esnext.promise", "lib.es2025.promise.d.ts"),
    ("esnext.array", "lib.esnext.array.d.ts"),
    ("esnext.collection", "lib.esnext.collection.d.ts"),
    ("esnext.date", "lib.esnext.date.d.ts"),
    ("esnext.decorators", "lib.esnext.decorators.d.ts"),
    ("esnext.disposable", "lib.esnext.disposable.d.ts"),
    ("esnext.error", "lib.esnext.error.d.ts"),
    ("esnext.intl", "lib.esnext.intl.d.ts"),
    ("esnext.sharedmemory", "lib.esnext.sharedmemory.d.ts"),
    ("esnext.temporal", "lib.esnext.temporal.d.ts"),
    ("esnext.typedarrays", "lib.esnext.typedarrays.d.ts"),
    ("decorators", "lib.decorators.d.ts"),
    ("decorators.legacy", "lib.decorators.legacy.d.ts"),
];

/// The file a `lib` option entry names, given the key the option's
/// enumeration lowered (`config`): `LibMap` keys only, never file names.
pub(super) fn option_file(key: &str) -> Option<&'static str> {
    LIB_MAP
        .iter()
        .find(|(name, _)| *name == key)
        .map(|&(_, file)| file)
}

/// The values the `lib` option accepts, for its TS6046 message.
pub(super) fn option_names() -> String {
    quoted(LIB_MAP.iter().map(|(name, _)| *name))
}

/// `tsoptions.GetLibFileName`: a library name, or one of the files library
/// names select, after `tspath.ToFileNameLowerCase`.
pub(super) fn reference_file(name: &str) -> Option<&'static str> {
    let lower = crate::input::to_file_name_lower_case(name);
    LIB_MAP
        .iter()
        .find(|(_, file)| *file == lower)
        .or_else(|| LIB_MAP.iter().find(|(key, _)| *key == lower))
        .map(|&(_, file)| file)
}

fn quoted<'a>(names: impl Iterator<Item = &'a str>) -> String {
    names
        .map(|name| format!("'{name}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// TypeScript's `ScriptTarget` values that `target` selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Es5,
    Es2015,
    Es2016,
    Es2017,
    Es2018,
    Es2019,
    Es2020,
    Es2021,
    Es2022,
    Es2023,
    Es2024,
    Es2025,
    EsNext,
}

/// `tsoptions.targetOptionMap`, in order.
const TARGETS: [(&str, Target); 14] = [
    ("es5", Target::Es5),
    ("es6", Target::Es2015),
    ("es2015", Target::Es2015),
    ("es2016", Target::Es2016),
    ("es2017", Target::Es2017),
    ("es2018", Target::Es2018),
    ("es2019", Target::Es2019),
    ("es2020", Target::Es2020),
    ("es2021", Target::Es2021),
    ("es2022", Target::Es2022),
    ("es2023", Target::Es2023),
    ("es2024", Target::Es2024),
    ("es2025", Target::Es2025),
    ("esnext", Target::EsNext),
];

impl Target {
    /// `core.ScriptTargetLatestStandard`, which `GetEmitScriptTarget` uses
    /// when `target` is unset.
    pub(super) const LATEST_STANDARD: Self = Self::Es2025;

    /// The target a lowered `target` value names.
    pub(super) fn from_key(key: &str) -> Option<Self> {
        TARGETS
            .iter()
            .find(|(name, _)| *name == key)
            .map(|&(_, target)| target)
    }

    /// Every key of TypeScript's `targetOptionMap`, `es5` included.
    pub(super) fn keys() -> Vec<&'static str> {
        TARGETS.iter().map(|&(name, _)| name).collect()
    }

    /// The values `target` accepts, for its TS6046 message, which omits the
    /// deprecated `es5`.
    pub(super) fn option_names() -> String {
        quoted(
            TARGETS
                .iter()
                .map(|(name, _)| *name)
                .filter(|name| *name != "es5"),
        )
    }

    /// `tsoptions.GetDefaultLibFileName` (`targetToLibMap`). ES2015 selects
    /// `lib.es6.d.ts`, which the compiler keeps for compatibility, and ES5
    /// selects `lib.d.ts`.
    pub(super) fn default_library(self) -> &'static str {
        match self {
            Self::Es5 => "lib.d.ts",
            Self::Es2015 => "lib.es6.d.ts",
            Self::Es2016 => "lib.es2016.full.d.ts",
            Self::Es2017 => "lib.es2017.full.d.ts",
            Self::Es2018 => "lib.es2018.full.d.ts",
            Self::Es2019 => "lib.es2019.full.d.ts",
            Self::Es2020 => "lib.es2020.full.d.ts",
            Self::Es2021 => "lib.es2021.full.d.ts",
            Self::Es2022 => "lib.es2022.full.d.ts",
            Self::Es2023 => "lib.es2023.full.d.ts",
            Self::Es2024 => "lib.es2024.full.d.ts",
            Self::Es2025 => "lib.es2025.full.d.ts",
            Self::EsNext => "lib.esnext.full.d.ts",
        }
    }
}

/// The library files a program's options select before following any
/// reference, ported from `processAllProgramFiles`: none without root files
/// or under `noLib`; the target's default without `lib`; otherwise each
/// `lib` entry, so an empty list selects none.
pub(super) fn roots(options: &Options, has_root_files: bool) -> Vec<&'static str> {
    if !has_root_files || options.no_lib {
        return Vec::new();
    }
    match &options.lib {
        None => vec![options.target.default_library()],
        Some(files) => files.clone(),
    }
}

/// What one `/// <reference lib="..." />` directive selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Reference {
    /// `noLib` ignores every library reference, without diagnostics.
    Ignored,
    File(&'static str),
    /// TypeScript's TS2726, which names the lowered spelling.
    Unknown(String),
}

/// A library reference in any file, including a library file, as the
/// `/// <reference lib>` branch of `parseTask.load` reads it. The
/// `no-default-lib` attribute plays no part here: it only makes the parser
/// ignore its whole directive (`crate::pragmas`).
pub(super) fn reference(options: &Options, name: &str) -> Reference {
    if options.no_lib {
        return Reference::Ignored;
    }
    match reference_file(name) {
        Some(file) => Reference::File(file),
        None => Reference::Unknown(crate::input::to_file_name_lower_case(name).into_owned()),
    }
}

/// One library file a program captures: TypeScript's `LibFile`, whose path
/// is the vendored file unless a replacement package supplies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Library {
    pub(super) file: &'static str,
    pub(super) replaced: bool,
}

impl Library {
    /// `fileLoader.getDefaultLibFilePriority`, which orders library files
    /// before every other input: `lib.d.ts` and `lib.es6.d.ts` first, then
    /// the vendored files in `Libs` order, then everything else, such as a
    /// `.full` default or a replacement file outside the vendored directory.
    pub(super) fn priority(self) -> usize {
        let last = LIB_MAP.len() + 2;
        if self.replaced {
            return last;
        }
        if matches!(self.file, "lib.d.ts" | "lib.es6.d.ts") {
            return 0;
        }
        let name = self
            .file
            .strip_prefix("lib.")
            .and_then(|name| name.strip_suffix(".d.ts"))
            .unwrap_or(self.file);
        LIB_MAP
            .iter()
            .position(|(key, _)| *key == name)
            .map_or(last, |index| index + 1)
    }
}

/// `getLibraryNameFromLibFileName`: the package that may replace a library
/// file under `libReplacement`. `lib.dom.iterable.d.ts` is
/// `@typescript/lib-dom/iterable`; further components join with `-`.
pub(super) fn replacement_package(file: &str) -> String {
    let components: Vec<&str> = file.split('.').collect();
    let mut package = String::from("@typescript/lib-");
    if let Some(first) = components.get(1) {
        package.push_str(first);
    }
    for (index, component) in components
        .iter()
        .enumerate()
        .skip(2)
        .take_while(|(_, component)| !component.is_empty() && **component != "d")
    {
        package.push(if index == 2 { '/' } else { '-' });
        package.push_str(component);
    }
    package
}

/// `getInferredLibraryNameResolveFrom`: the file a replacement package is
/// resolved from, as a `require` in the configuration's directory.
pub(super) fn replacement_lookup(config_dir: &str, file: &str) -> String {
    paths::combine(
        config_dir,
        &format!("__lib_node_modules_lookup_{file}__.ts"),
    )
}

/// Whether a replacement may stand in for `file`: never for `lib.d.ts`.
pub(super) fn replaceable(options: &Options, file: &str) -> bool {
    options.lib_replacement && file != "lib.d.ts"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn provenance() -> serde_json::Value {
        let path = Path::new(&directory()).join("provenance.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        serde_json::from_str(&text).unwrap()
    }

    /// The catalog equals `LibMap` and `targetToLibMap` as the vendoring
    /// tool recorded them from the compiler source at the pinned tag.
    #[test]
    fn the_catalog_is_the_pinned_compilers() {
        let provenance = provenance();
        assert_eq!(provenance["version"], "7.0.2");
        assert_eq!(provenance["upstream"]["tag"], "typescript/v7.0.2");
        let recorded: Vec<(String, String)> =
            serde_json::from_value(provenance["libMap"].clone()).unwrap();
        let catalog: Vec<(String, String)> = LIB_MAP
            .iter()
            .map(|&(name, file)| (name.into(), file.into()))
            .collect();
        assert_eq!(catalog, recorded);
        let targets = provenance["targetLibraries"].as_object().unwrap();
        for &(name, target) in &TARGETS {
            let expected = match name {
                "es5" => provenance["otherTargetLibrary"].as_str().unwrap(),
                "es6" => targets["es2015"].as_str().unwrap(),
                _ => targets[name].as_str().unwrap(),
            };
            assert_eq!(target.default_library(), expected, "{name}");
        }
        assert_eq!(targets.len(), TARGETS.len() - 2);
    }

    /// Every file the catalog names is vendored, every vendored library is
    /// one the catalog or a target selects, and every reference inside one
    /// names a cataloged library.
    #[test]
    fn the_vendored_files_are_exactly_the_cataloged_ones() {
        let directory = directory();
        let mut vendored = BTreeSet::new();
        for entry in std::fs::read_dir(&directory).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            if name.starts_with("lib.") {
                vendored.insert(name);
            }
        }
        let mut selected: BTreeSet<String> =
            LIB_MAP.iter().map(|&(_, file)| file.to_owned()).collect();
        selected.extend(TARGETS.iter().map(|(_, t)| t.default_library().to_owned()));
        assert_eq!(vendored, selected);
        let recorded: BTreeSet<String> = provenance()["files"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|name| name.starts_with("lib."))
            .cloned()
            .collect();
        assert_eq!(vendored, recorded);
        for name in &vendored {
            let text = std::fs::read_to_string(Path::new(&directory).join(name)).unwrap();
            for directive in crate::pragmas::references(&text) {
                let crate::pragmas::Directive::Lib(lib, _) = directive.directive() else {
                    panic!("{name} has a directive other than a library reference");
                };
                assert!(reference_file(lib).is_some(), "{name} references {lib}");
            }
        }
    }

    #[test]
    fn names_select_files_as_the_compiler_lowers_them() {
        assert_eq!(
            reference_file("ES2015.Promise"),
            Some("lib.es2015.promise.d.ts")
        );
        assert_eq!(
            reference_file("lib.es2016.array.include.d.ts"),
            Some("lib.es2016.array.include.d.ts")
        );
        assert_eq!(reference_file("es6"), Some("lib.es2015.d.ts"));
        assert_eq!(reference_file("lib.es2025.full.d.ts"), None);
        assert_eq!(reference_file("lib.d.ts"), None);
        assert_eq!(reference_file(""), None);
        assert_eq!(option_file("lib.es2015.d.ts"), None);
        assert_eq!(
            option_file("esnext.promise"),
            Some("lib.es2025.promise.d.ts")
        );
    }

    #[test]
    fn library_files_are_ordered_by_catalog_position() {
        let vendored = |file| Library {
            file,
            replaced: false,
        };
        assert_eq!(vendored("lib.d.ts").priority(), 0);
        assert_eq!(vendored("lib.es6.d.ts").priority(), 0);
        assert_eq!(vendored("lib.es5.d.ts").priority(), 1);
        // `es2015` is the third key; `es6` names the same file.
        assert_eq!(vendored("lib.es2015.d.ts").priority(), 3);
        assert_eq!(vendored("lib.es2025.full.d.ts").priority(), 109);
        let replaced = Library {
            file: "lib.es5.d.ts",
            replaced: true,
        };
        assert_eq!(replaced.priority(), 109);
    }

    #[test]
    fn only_lib_d_ts_is_never_replaced() {
        // `lib.d.ts` is ES5's default, and this tool rejects `target: es5`
        // as removed, so the rule is unreachable through a configuration.
        let mut options = Options::for_extends_lookup();
        options.lib_replacement = true;
        assert!(!replaceable(&options, "lib.d.ts"));
        assert!(replaceable(&options, "lib.es6.d.ts"));
        assert!(replaceable(&options, "lib.es2025.full.d.ts"));
        options.lib_replacement = false;
        assert!(!replaceable(&options, "lib.dom.d.ts"));
    }

    #[test]
    fn replacement_packages_follow_the_file_name() {
        assert_eq!(replacement_package("lib.dom.d.ts"), "@typescript/lib-dom");
        assert_eq!(
            replacement_package("lib.dom.iterable.d.ts"),
            "@typescript/lib-dom/iterable"
        );
        assert_eq!(
            replacement_package("lib.es2015.symbol.wellknown.d.ts"),
            "@typescript/lib-es2015/symbol-wellknown"
        );
        assert_eq!(
            replacement_lookup("/p", "lib.dom.d.ts"),
            "/p/__lib_node_modules_lookup_lib.dom.d.ts__.ts"
        );
    }
}
