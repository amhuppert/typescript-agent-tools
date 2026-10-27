//! Standard library declarations as each dialect's compiler selects them
//! (`Catalog`). TypeScript 7.0.2's catalog ports `tsoptions.LibMap`,
//! `tsoptions.GetLibFileName`, and `tsoptions.GetDefaultLibFileName`;
//! TypeScript 5.6.3's ports `libs`, `libMap`,
//! `getLibFileNameFromLibReference`, and `getDefaultLibFileName`. Selection
//! ports the library block of `compiler.processAllProgramFiles` (5.6.3:
//! `createProgram`) and the `/// <reference lib>` branch of
//! `parseTask.load` (`processLibReferenceDirectives`); replacement lookup
//! ports `fileLoader.pathForLibFile` (`pathForLibFileWorker`); program order
//! ports where those functions add the libraries' tasks, which files
//! `collectFiles` (5.6.3: `processingDefaultLibFiles`) orders with the
//! libraries, and `fileLoader.getDefaultLibFilePriority`
//! (`getDefaultLibFilePriority`).
//!
//! Configuration decodes the options once (`config::convert`); the loader
//! captures the files this module names through the ordinary input path.
//! TypeScript 7's declarations are the pinned compiler's own, vendored in
//! this checkout with their license and provenance, and its catalog's unit
//! tests check it against that provenance, which records `LibMap` from the
//! compiler source at the pinned tag. TypeScript 5.6's declarations are the
//! installed compiler's, in its package's `lib` directory; its catalog is
//! extracted from the pinned 5.6.3 compiler (`catalog_5_6`).

use super::config::Options;
use super::disk::Disk;
use super::resolve::{Resolved, Resolver};
use super::{io_error, paths};
use crate::analysis::InputError;
use crate::input::Case;
use crate::project::binding::ResolutionMode;
use std::borrow::Cow;
use std::path::Path;

mod catalog_5_6;

/// The standard library rules of one dialect (`compatibility::Dialect`):
/// the library names and the files they select, program order, the
/// target's default, which names a `/// <reference lib>` accepts, where the
/// declarations are read from, and when a `@typescript/lib-*` package
/// replaces one. Only this module's operations interpret it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Catalog(Profile);

/// Whose library rules: TypeScript 7.0.2's or TypeScript 5.6.3's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Profile {
    TypeScript7,
    TypeScript56,
}

/// The vendored TypeScript 7.0.2 declarations, as the build's checkout
/// spelled them: that compiler's `defaultLibraryPath`. Nothing searches for
/// another copy or probes this one; a missing or unreadable file is an I/O
/// failure where loading reads it, and moving the checkout requires
/// rebuilding.
pub(super) fn vendored_directory() -> String {
    paths::native_text(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/vendor/typescript-7.0.2-lib"
    ))
}

impl Catalog {
    pub(super) const TYPESCRIPT_7: Self = Self(Profile::TypeScript7);
    pub(super) const TYPESCRIPT_5_6: Self = Self(Profile::TypeScript56);

    /// `Libs` or `libs`: every library name in declaration order, a name
    /// 5.6.3 declares twice at both places.
    fn names(self) -> impl Iterator<Item = &'static str> {
        let (map, list): (&[(&str, &str)], &[&str]) = match self.0 {
            Profile::TypeScript7 => (&LIB_MAP, &[]),
            Profile::TypeScript56 => (&[], catalog_5_6::LIBS),
        };
        map.iter()
            .map(|&(name, _)| name)
            .chain(list.iter().copied())
    }

    /// `LibMap` or `libMap`: each library name, once, and the file it
    /// selects, in the order names are first declared.
    fn map(self) -> &'static [(&'static str, &'static str)] {
        match self.0 {
            Profile::TypeScript7 => &LIB_MAP,
            Profile::TypeScript56 => catalog_5_6::LIB_MAP,
        }
    }

    /// The directory a program reads the declarations from
    /// (`defaultLibraryPath`): TypeScript 7.0.2's vendored copy, or the
    /// `lib` directory of the installed TypeScript 5 package, as 5.6.3's
    /// `getDefaultLibLocation` finds its own, never another compiler's: the
    /// directory of the compiler that runs, through every link, so a linked
    /// `lib` is its target. `package` is the installed compiler's real
    /// package directory, which the TypeScript 5.6 dialect is selected by.
    /// An absent `lib` is named where it would be, so that each library
    /// read there fails as missing, naming itself; any other failure to
    /// find its real path is an I/O failure.
    pub(super) fn directory(
        self,
        package: Option<&str>,
        disk: &Disk,
    ) -> Result<String, InputError> {
        match self.0 {
            Profile::TypeScript7 => Ok(vendored_directory()),
            Profile::TypeScript56 => {
                let lib = paths::combine(
                    package.expect("an installed compiler selects the TypeScript 5.6 dialect"),
                    "lib",
                );
                match disk.canonicalize(&lib) {
                    Ok(real) => paths::from_path(&real),
                    Err(error) if crate::input::is_absence(&error) => Ok(lib),
                    Err(error) => Err(io_error(Path::new(&lib), &error)),
                }
            }
        }
    }

    /// Where the declarations a program reads come from, as the doctor
    /// names it, given the `directory` it reads them from.
    pub(super) fn source(self, directory: &str) -> String {
        match self.0 {
            Profile::TypeScript7 => "the vendored TypeScript 7.0.2 declarations".into(),
            Profile::TypeScript56 => {
                format!("the installed compiler's declarations in {directory}")
            }
        }
    }

    /// The file a `lib` option entry names, given the key the option's
    /// enumeration lowered (`config`): library names only, never file
    /// names. 5.6.3's `libMap` keeps the last file declared for a name.
    pub(super) fn option_file(self, key: &str) -> Option<&'static str> {
        self.map()
            .iter()
            .find(|(name, _)| *name == key)
            .map(|&(_, file)| file)
    }

    /// The values the `lib` option accepts, for its TS6046 message.
    pub(super) fn option_names(self) -> String {
        quoted(self.map().iter().map(|(name, _)| *name))
    }

    /// `GetDefaultLibFileName` or `getDefaultLibFileName`: the library a
    /// target selects without `lib`. ES2015 selects `lib.es6.d.ts`, which
    /// the compilers keep for compatibility, ES5 selects `lib.d.ts`, and
    /// 5.6.3 also selects `lib.d.ts` for the ES2024 and ES2025 targets it
    /// does not know, which its configuration never names.
    fn default_library(self, target: Target) -> &'static str {
        match self.0 {
            Profile::TypeScript7 => target.default_library(),
            Profile::TypeScript56 => catalog_5_6::DEFAULT_LIBRARIES
                .iter()
                .find(|(key, _)| Target::from_key(key) == Some(target))
                .map_or(catalog_5_6::OTHER_DEFAULT_LIBRARY, |&(_, file)| file),
        }
    }

    /// A program's tasks after its root files, in the order
    /// `processAllProgramFiles` adds them: the libraries the options select
    /// (`roots`), then, with root files, the type packages the configuration
    /// selects (`addAutomaticTypeDirectiveTasks`). 5.6.3's `createProgram`
    /// processes the type packages first. Which comes first decides the
    /// order of the files each reaches, and of libraries that tie in
    /// priority (`priority`), so a type package can precede what a
    /// replacement imports, and a replacement it references one `lib` names.
    pub(super) fn tasks(self, options: &Options, has_root_files: bool) -> Vec<Task> {
        let libraries = self
            .roots(options, has_root_files)
            .into_iter()
            .map(Task::Library);
        let type_packages = has_root_files.then_some(Task::TypePackages);
        match self.0 {
            Profile::TypeScript7 => libraries.chain(type_packages).collect(),
            Profile::TypeScript56 => type_packages.into_iter().chain(libraries).collect(),
        }
    }

    /// The library files a program's options select before following any
    /// reference, ported from `processAllProgramFiles` and 5.6.3's
    /// `createProgram`: none without root files or under `noLib`; the
    /// target's default without `lib`; otherwise each `lib` entry, so an
    /// empty list selects none. 5.6.3 reads the target's default from its
    /// directory without `pathForLibFile`, so no package replaces it there.
    fn roots(self, options: &Options, has_root_files: bool) -> Vec<Request> {
        if !has_root_files || options.no_lib {
            return Vec::new();
        }
        match &options.lib {
            None => vec![Request {
                file: self.default_library(options.target),
                located: self.0 == Profile::TypeScript7,
            }],
            Some(files) => files
                .iter()
                .map(|&file| Request {
                    file,
                    located: true,
                })
                .collect(),
        }
    }

    /// A library reference in any file, including a library file, as the
    /// `/// <reference lib>` branch of `parseTask.load` reads it, or 5.6.3's
    /// `processLibReferenceDirectives`. TypeScript 7.0.2 accepts a library
    /// name or one of the files library names select
    /// (`tsoptions.GetLibFileName`); 5.6.3 accepts names only
    /// (`getLibFileNameFromLibReference`) and suggests the nearest name for
    /// an unknown one (TS2727). The `no-default-lib` attribute plays no part
    /// here: it only makes the parser ignore its whole directive
    /// (`crate::pragmas`), as in TypeScript 7.0.2; 5.6.3's other uses of it
    /// are a documented difference (docs/references.md).
    pub(super) fn reference(self, options: &Options, name: &str) -> Reference {
        if options.no_lib {
            return Reference::Ignored;
        }
        let located = |file| {
            Reference::File(Request {
                file,
                located: true,
            })
        };
        match self.0 {
            Profile::TypeScript7 => match reference_file(name) {
                Some(file) => located(file),
                None => Reference::Unknown(format!(
                    "Cannot find lib definition for '{}'",
                    crate::input::to_file_name_lower_case(name)
                )),
            },
            Profile::TypeScript56 => {
                let lower = crate::input::to_file_name_lower_case_5_6(name);
                if let Some(file) = self.option_file(&lower) {
                    return located(file);
                }
                // `filePreprocessingLibreferenceDiagnostic`.
                let unqualified = lower.strip_prefix("lib.").unwrap_or(&lower);
                let unqualified = unqualified.strip_suffix(".d.ts").unwrap_or(unqualified);
                Reference::Unknown(
                    match crate::spelling::typescript_5_6_suggestion(unqualified, self.names()) {
                        Some(suggestion) => format!(
                            "Cannot find lib definition for '{lower}'. Did you mean '{suggestion}'?"
                        ),
                        None => format!("Cannot find lib definition for '{lower}'"),
                    },
                )
            }
        }
    }

    /// Whether `collectFiles` orders a file with the libraries (5.6.3:
    /// whether `findSourceFileWorker` pushes it to
    /// `processingDefaultLibFiles`), and where it lies for
    /// `getDefaultLibFilePriority` (`priority`), given the `library` the
    /// loader reserved the file as, the file's `identity`, the `route` by
    /// which the depth-first walk of the program's tasks first reaches it,
    /// and, when the file that route leaves is ordered with the libraries,
    /// that file's name (`Placement::name`).
    ///
    /// TypeScript 7.0.2 orders a file with them when the task it loads
    /// first for the file is a library's, which this tool's loader decides
    /// as it reserves each input: when the file is a library. A located
    /// library lies in the catalog's directory; a replacement is last
    /// wherever it resolves, even into the vendored copy, which only stands
    /// in for 7.0.2's own directory.
    ///
    /// 5.6.3 decides by the route that first processes a file and names it
    /// as that route does (`SourceFile.fileName`): a library task or a
    /// `/// <reference lib>`, which names a located library in the
    /// catalog's directory and a replacement by the real path it resolved
    /// to, or a `/// <reference path>` from a file ordered with the
    /// libraries (`processReferencedFiles(file, isDefaultLib)`), which
    /// names the file as the reference spells it from the referencing
    /// file's name (`resolveTripleslashReference`), through any link. An
    /// import or a type reference never orders a file with the libraries,
    /// even one that reaches a replacement before the replacement's own
    /// library task, as a type package's can.
    ///
    /// Only the order follows the route. Which files are libraries to the
    /// rest of this tool, never searched, is the loader's reservation, and
    /// keeping every replacement one is this tool's product policy in both
    /// dialects, not a compiler's rule: 5.6.3's provenance
    /// (`isSourceFileDefaultLibrary`) counts a file with `no-default-lib`
    /// and otherwise only the target's default or a `lib` entry's file
    /// (docs/references.md).
    pub(super) fn ordering(
        self,
        library: Option<Library>,
        identity: &str,
        route: &Route,
        from: Option<&str>,
    ) -> Option<Placement> {
        match self.0 {
            Profile::TypeScript7 => library.map(|library| {
                if library.replaced {
                    Placement::Last
                } else {
                    Placement::Located(library.file)
                }
            }),
            Profile::TypeScript56 => match route {
                Route::Library(library) if library.replaced => {
                    Some(Placement::At(identity.to_owned()))
                }
                Route::Library(library) => Some(Placement::Located(library.file)),
                // `resolveTripleslashReference` normalizes the written path
                // from the referencing file's name, and only then does
                // `getSourceFileFromReferenceWorker` add the extension it
                // finds the file with.
                Route::ReferencePath { written, extension } => from.map(|referencing| {
                    let mut name = paths::resolve(paths::parent(referencing), written);
                    name.push_str(extension);
                    Placement::At(name)
                }),
                Route::Other => None,
            },
        }
    }

    /// `fileLoader.getDefaultLibFilePriority` (5.6.3:
    /// `getDefaultLibFilePriority`), which orders the files ordered with the
    /// libraries before every other input, by where each lies
    /// (`ordering`): a file anywhere beneath `directory`, where the
    /// compiler reads its libraries, by its file name, `lib.d.ts` and
    /// `lib.es6.d.ts` first and then by the first position of the name in
    /// `Libs` (`libs.indexOf`); everything else last, such as a `.full`
    /// default, a file whose name no library has, or a replacement outside
    /// the directory. Both compilers ask case-sensitively, 7.0.2 by prefix
    /// and 5.6.3 by `containsPath`.
    pub(super) fn priority(self, placement: &Placement, directory: &str) -> usize {
        let last = self.names().count() + 2;
        let file = match placement {
            Placement::Located(file) => *file,
            Placement::At(path) if Case::Sensitive.contains(directory, path) => {
                paths::basename(path)
            }
            Placement::At(_) | Placement::Last => return last,
        };
        if matches!(file, "lib.d.ts" | "lib.es6.d.ts") {
            return 0;
        }
        let name = file.strip_prefix("lib.").unwrap_or(file);
        let name = name.strip_suffix(".d.ts").unwrap_or(name);
        self.names()
            .position(|key| key == name)
            .map_or(last, |index| index + 1)
    }

    /// The `@typescript/lib-*` package that `pathForLibFile` looks up for
    /// `request` from the configuration's directory
    /// (`getInferredLibraryNameResolveFrom`), and what it resolves to, or
    /// none where the dialect looks up none. TypeScript 7.0.2 looks one up
    /// only under `libReplacement` and never for `lib.d.ts`, with the
    /// configured resolver as a `require` (`fileLoader.resolveLibrary`);
    /// 5.6.3 looks one up for every request `pathForLibFile` locates, with
    /// options of its own (`resolveLibrary`, `Resolver::library`).
    pub(super) fn replacement(
        self,
        request: Request,
        options: &Options,
        config_dir: &str,
        resolver: &mut Resolver<'_>,
    ) -> Option<(String, Resolved)> {
        if !self.looks_up(request, options) {
            return None;
        }
        let package = replacement_package(request.file);
        let from = replacement_lookup(config_dir, request.file);
        let resolved = match self.0 {
            Profile::TypeScript7 => resolver.module(&package, &from, Some(ResolutionMode::Require)),
            Profile::TypeScript56 => resolver.library(&package, &from),
        };
        Some((package, resolved))
    }

    /// Whether `pathForLibFile` looks a replacement up for `request`:
    /// under TypeScript 7.0.2 only with `libReplacement` and never for
    /// `lib.d.ts`; under 5.6.3 always, `lib.d.ts` included, though only
    /// ES5's default, which `pathForLibFile` never locates, is that file.
    fn looks_up(self, request: Request, options: &Options) -> bool {
        request.located
            && match self.0 {
                Profile::TypeScript7 => options.lib_replacement && request.file != "lib.d.ts",
                Profile::TypeScript56 => true,
            }
    }
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

/// `tsoptions.GetLibFileName`: a library name, or one of the files library
/// names select, after `tspath.ToFileNameLowerCase`.
fn reference_file(name: &str) -> Option<&'static str> {
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
    /// Native `languageVersion >= ScriptTargetES2015`, used by iterator and
    /// binding-pattern factories independently of default library selection.
    pub(super) fn supports_iterables(self) -> bool {
        self != Self::Es5
    }
    /// `LanguageFeatureMinimumTarget.ObjectSpreadRest` is ES2018.
    pub(super) fn supports_object_rest(self) -> bool {
        !matches!(self, Self::Es5 | Self::Es2015 | Self::Es2016 | Self::Es2017)
    }
    /// `LanguageFeatureMinimumTarget.ClassAndClassElementDecorators` is ESNext.
    pub(super) fn supports_class_element_decorators(self) -> bool {
        self == Self::EsNext
    }
    /// The `ScriptTarget >= ES2022` question shared by TypeScript's
    /// `GetUseDefineForClassFields` and `GetEmitStandardClassFields`.
    pub(super) fn is_at_least_es2022(self) -> bool {
        matches!(
            self,
            Self::Es2022 | Self::Es2023 | Self::Es2024 | Self::Es2025 | Self::EsNext
        )
    }

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

    /// `tsoptions.GetDefaultLibFileName` (`targetToLibMap`), which
    /// TypeScript 7's catalog selects with (`Catalog::default_library`).
    fn default_library(self) -> &'static str {
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

/// One library file a program asks for (`Catalog::roots`,
/// `Catalog::reference`), and whether `pathForLibFile` locates it, which may
/// find a replacement package for it (`Catalog::replacement`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Request {
    pub(super) file: &'static str,
    located: bool,
}

/// One of a program's tasks after its root files (`Catalog::tasks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Task {
    Library(Request),
    /// The type packages the configuration selects.
    TypePackages,
}

/// How the depth-first walk of a program's tasks reaches a file
/// (`Catalog::ordering`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Route {
    /// A library task or a `/// <reference lib>`, and the library it
    /// locates.
    Library(Library),
    /// A `/// <reference path>`: the path it writes, which names the file
    /// from the referencing file's directory through any link, and the
    /// extension the file was found with when the normalized path has none
    /// (otherwise empty).
    ReferencePath { written: String, extension: String },
    /// A root file, a type package, a `/// <reference types>`, or an import.
    Other,
}

/// Where a file ordered with the libraries lies (`Catalog::ordering`), which
/// orders it (`Catalog::priority`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Placement {
    /// A library in the catalog's directory, by its file name.
    Located(&'static str),
    /// A file by the name the program gives it.
    At(String),
    /// A file ordered last wherever it lies.
    Last,
}

impl Placement {
    /// The name the program gives the file (`SourceFile.fileName`), from
    /// which its reference paths resolve: a located library's in
    /// `directory`, where the catalog reads libraries, or the file's own
    /// name, else its `identity`.
    pub(super) fn name<'a>(&'a self, directory: &str, identity: &'a str) -> Cow<'a, str> {
        match self {
            Self::Located(file) => Cow::Owned(paths::combine(directory, file)),
            Self::At(path) => Cow::Borrowed(path),
            Self::Last => Cow::Borrowed(identity),
        }
    }
}

/// What one `/// <reference lib="..." />` directive selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Reference {
    /// `noLib` ignores every library reference, without diagnostics.
    Ignored,
    File(Request),
    /// TypeScript's TS2726, or 5.6.3's TS2727 with its suggestion: the
    /// message, which names the lowered spelling.
    Unknown(String),
}

/// One library file a program captures: TypeScript's `LibFile`, whose path
/// is in the catalog's directory unless a replacement package supplies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Library {
    pub(super) file: &'static str,
    pub(super) replaced: bool,
}

/// `getLibraryNameFromLibFileName`, the same in both compilers: the package
/// that may replace a library file. `lib.dom.iterable.d.ts` is
/// `@typescript/lib-dom/iterable`; further components join with `-`.
fn replacement_package(file: &str) -> String {
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

/// `getInferredLibraryNameResolveFrom`, the same in both compilers: the file
/// a replacement package is resolved from, in the configuration's directory.
fn replacement_lookup(config_dir: &str, file: &str) -> String {
    paths::combine(
        config_dir,
        &format!("__lib_node_modules_lookup_{file}__.ts"),
    )
}

#[cfg(test)]
mod tests {
    use super::super::compatibility::CompilerSelection;
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn provenance() -> serde_json::Value {
        let path = Path::new(&vendored_directory()).join("provenance.json");
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
        let catalog: Vec<(String, String)> = Catalog::TYPESCRIPT_7
            .map()
            .iter()
            .map(|&(name, file)| (name.into(), file.into()))
            .collect();
        assert_eq!(catalog, recorded);
        assert!(
            Catalog::TYPESCRIPT_7
                .names()
                .eq(recorded.iter().map(|(name, _)| name.as_str()))
        );
        let targets = provenance["targetLibraries"].as_object().unwrap();
        for &(name, target) in &TARGETS {
            let expected = match name {
                "es5" => provenance["otherTargetLibrary"].as_str().unwrap(),
                "es6" => targets["es2015"].as_str().unwrap(),
                _ => targets[name].as_str().unwrap(),
            };
            assert_eq!(
                Catalog::TYPESCRIPT_7.default_library(target),
                expected,
                "{name}"
            );
        }
        assert_eq!(targets.len(), TARGETS.len() - 2);
    }

    /// Every file the catalog names is vendored, every vendored library is
    /// one the catalog or a target selects, and every reference inside one
    /// names a cataloged library.
    #[test]
    fn the_vendored_files_are_exactly_the_cataloged_ones() {
        let directory = vendored_directory();
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
        let seven = Catalog::TYPESCRIPT_7;
        assert_eq!(seven.option_file("lib.es2015.d.ts"), None);
        assert_eq!(
            seven.option_file("esnext.promise"),
            Some("lib.es2025.promise.d.ts")
        );
    }

    /// A located library is ordered by its file name, and a file at a path
    /// by its file name anywhere beneath the directory and last elsewhere;
    /// the directory is compared case-sensitively, by whole components.
    #[test]
    fn library_files_are_ordered_by_catalog_position() {
        let seven = Catalog::TYPESCRIPT_7;
        let priority = |path: &str| seven.priority(&Placement::At(path.into()), "/c/lib");
        assert_eq!(
            seven.priority(&Placement::Located("lib.es5.d.ts"), "/c/lib"),
            1
        );
        assert_eq!(seven.priority(&Placement::Last, "/c/lib"), 109);
        assert_eq!(priority("/c/lib/lib.d.ts"), 0);
        assert_eq!(priority("/c/lib/lib.es6.d.ts"), 0);
        assert_eq!(priority("/c/lib/lib.es5.d.ts"), 1);
        // `es2015` is the third key; `es6` names the same file.
        assert_eq!(priority("/c/lib/lib.es2015.d.ts"), 3);
        assert_eq!(priority("/c/lib/lib.es2025.full.d.ts"), 109);
        assert_eq!(priority("/c/lib/nested/deeper/lib.es5.d.ts"), 1);
        for elsewhere in [
            "/c/replacement/lib.es5.d.ts",
            "/c/library/lib.es5.d.ts",
            "/c/LIB/lib.es5.d.ts",
            "/c/lib.es5.d.ts",
        ] {
            assert_eq!(priority(elsewhere), 109, "{elsewhere}");
        }
        assert_eq!(
            seven.priority(&Placement::At("/c/lib/lib.es5.d.ts".into()), "/c/lib/"),
            1
        );
    }

    /// TypeScript 7 orders the files the loader reserved as libraries, a
    /// replacement last wherever it lies; 5.6.3 orders by the first route,
    /// a replacement by the path it resolved to and a referenced file by
    /// the path its reference spells from the referencing file's name.
    #[test]
    fn each_dialect_places_a_file_by_its_own_rule() {
        let located = Library {
            file: "lib.dom.d.ts",
            replaced: false,
        };
        let replaced = Library {
            file: "lib.dom.d.ts",
            replaced: true,
        };
        let identity = "/c/lib/lib.dom.d.ts";
        let spelled = Route::ReferencePath {
            written: "../../typescript/lib/nested/lib.es5.d.ts".into(),
            extension: String::new(),
        };
        let seven = Catalog::TYPESCRIPT_7;
        assert_eq!(
            seven.ordering(Some(replaced), identity, &Route::Library(replaced), None),
            Some(Placement::Last)
        );
        assert_eq!(
            seven.ordering(Some(located), identity, &Route::Other, None),
            Some(Placement::Located("lib.dom.d.ts"))
        );
        assert_eq!(
            seven.ordering(None, identity, &spelled, Some(identity)),
            None
        );
        let five = Catalog::TYPESCRIPT_5_6;
        assert_eq!(
            five.ordering(Some(located), identity, &Route::Library(replaced), None),
            Some(Placement::At(identity.into()))
        );
        assert_eq!(
            five.ordering(None, identity, &Route::Library(located), None),
            Some(Placement::Located("lib.dom.d.ts"))
        );
        assert_eq!(
            five.ordering(Some(located), identity, &Route::Other, Some(identity)),
            None
        );
        // From the referencing file's name, through the link it spells.
        assert_eq!(
            five.ordering(
                None,
                identity,
                &spelled,
                Some("/c/node_modules/@typescript/lib-scripthost/index.d.ts")
            ),
            Some(Placement::At(
                "/c/node_modules/typescript/lib/nested/lib.es5.d.ts".into()
            ))
        );
        assert_eq!(five.ordering(None, identity, &spelled, None), None);
        // Normalized before the extension it was found with is added.
        let climbing = Route::ReferencePath {
            written: "../../typescript/lib/es5/child/..".into(),
            extension: ".d.ts".into(),
        };
        assert_eq!(
            five.ordering(
                None,
                identity,
                &climbing,
                Some("/c/node_modules/@typescript/lib-scripthost/index.d.ts")
            ),
            Some(Placement::At(
                "/c/node_modules/typescript/lib/es5.d.ts".into()
            ))
        );
    }

    #[test]
    fn only_lib_d_ts_is_never_replaced() {
        // `lib.d.ts` is ES5's default, and this tool rejects `target: es5`
        // as removed, so the rule is unreachable through a configuration.
        let located = |file| Request {
            file,
            located: true,
        };
        let seven = Catalog::TYPESCRIPT_7;
        let mut options = Options::for_extends_lookup(CompilerSelection::typescript_7());
        options.lib_replacement = true;
        assert!(!seven.looks_up(located("lib.d.ts"), &options));
        assert!(seven.looks_up(located("lib.es6.d.ts"), &options));
        assert!(seven.looks_up(located("lib.es2025.full.d.ts"), &options));
        options.lib_replacement = false;
        assert!(!seven.looks_up(located("lib.dom.d.ts"), &options));
        // 5.6.3 has no such gate and ignores `libReplacement`, but it never
        // locates a target's default, `lib.d.ts` among them, through
        // `pathForLibFile`.
        let five = Catalog::TYPESCRIPT_5_6;
        assert!(five.looks_up(located("lib.d.ts"), &options));
        assert!(five.looks_up(located("lib.dom.d.ts"), &options));
        let mut es5 = Options::for_extends_lookup(CompilerSelection::typescript_7());
        es5.target = Target::Es5;
        assert_eq!(
            five.roots(&es5, true),
            [Request {
                file: "lib.d.ts",
                located: false
            }]
        );
    }

    /// The 5.6.3 data is the pinned compiler's, as its provenance records;
    /// `tools/ts56-library-catalog.test.mjs` re-extracts it. A repeated name
    /// keeps its last file and its first place in program order.
    #[test]
    fn the_typescript_5_6_catalog_is_the_pinned_compilers() {
        assert_eq!(catalog_5_6::VERSION, "5.6.3");
        assert_eq!(
            catalog_5_6::SOURCE_SHA256,
            "f316520790d4db220a10d890c5f85310e26a1bd3c104b8d3b5eb62ba0491651b"
        );
        let five = Catalog::TYPESCRIPT_5_6;
        assert_eq!(five.names().count(), 88);
        assert_eq!(five.map().len(), 86);
        let names: Vec<&str> = five.names().collect();
        assert_eq!(
            (names[70], names[82], names[77], names[84]),
            (
                "esnext.array",
                "esnext.array",
                "esnext.string",
                "esnext.string"
            )
        );
        assert_eq!(
            five.option_file("esnext.array"),
            Some("lib.esnext.array.d.ts")
        );
        assert_eq!(
            five.option_file("esnext.string"),
            Some("lib.esnext.string.d.ts")
        );
        assert_eq!(five.option_file("es2024"), None);
        let priority = |path: &str| five.priority(&Placement::At(path.into()), "/c/lib");
        assert_eq!(priority("/c/lib/lib.esnext.array.d.ts"), 71);
        assert_eq!(priority("/c/lib/lib.esnext.string.d.ts"), 78);
        assert_eq!(priority("/c/lib/lib.esnext.collection.d.ts"), 72);
        assert_eq!(priority("/c/lib/lib.es2017.full.d.ts"), 90);
        assert_eq!(
            priority("/c/node_modules/@typescript/lib-dom/index.d.ts"),
            90
        );
        for (target, file) in [
            (Target::Es5, "lib.d.ts"),
            (Target::Es2015, "lib.es6.d.ts"),
            (Target::Es2017, "lib.es2017.full.d.ts"),
            (Target::Es2023, "lib.es2023.full.d.ts"),
            (Target::Es2024, "lib.d.ts"),
            (Target::Es2025, "lib.d.ts"),
            (Target::EsNext, "lib.esnext.full.d.ts"),
        ] {
            assert_eq!(five.default_library(target), file, "{target:?}");
        }
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
