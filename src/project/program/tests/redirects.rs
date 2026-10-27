//! Package-id redirects in the loaded program. The inputs are TypeScript
//! 7.0.2's `tsc --listFilesOnly --incremental false` listing, which lists
//! each file once; 5.6.3 also lists a redirect file, after the files it
//! precedes depth first, and `tools/program-inputs.mjs --oracle=5.6` folds
//! it into its target, which gives the same list. Module names are 5.6.3's
//! `moduleSpecifiers.getModuleSpecifiers` (`tools/ts56-oracle.mjs`) and both
//! compilers' declaration emit (`import("dup").Box`), or 7.0.2's TS2883
//! without deduplication.

use super::super::test_compiler::Dialect::{self, TypeScript7, TypeScript56};
use super::super::test_compiler::{BASE_OPTIONS, DIALECTS};
use super::*;

fn configuration(extra: &str, files: &[&str]) -> String {
    let files: Vec<String> = files.iter().map(|file| format!("{file:?}")).collect();
    format!(
        r#"{{"compilerOptions":{{{BASE_OPTIONS}{extra}}},"files":[{}]}}"#,
        files.join(",")
    )
}

fn manifest(name: &str, version: &str) -> String {
    format!(r#"{{"name":"{name}","version":"{version}","types":"index.d.ts"}}"#)
}

/// Experiment C's layout: `dup@1.0.0` at the root and under `a`, the nested
/// copy importing `./extra`, which only it reaches, and `shared`, which the
/// project imports too.
fn layout(dialect: Dialect, extra: &str, main: &str) -> Project {
    let config = configuration(extra, &["src/main.ts"]);
    let compiler = dialect.manifest();
    let dup = manifest("dup", "1.0.0");
    Project::new(&[
        (MANIFEST, &compiler),
        ("tsconfig.json", &config),
        ("node_modules/dup/package.json", &dup),
        (
            "node_modules/dup/index.d.ts",
            "export declare class Box { protected secret: number; }\n",
        ),
        ("node_modules/a/package.json", &manifest("a", "1.0.0")),
        (
            "node_modules/a/index.d.ts",
            "import { Box } from \"dup\";\nexport declare function make(): Box;\n",
        ),
        ("node_modules/a/node_modules/dup/package.json", &dup),
        (
            "node_modules/a/node_modules/dup/index.d.ts",
            "import \"./extra\";\nimport \"shared\";\nexport declare class Box { protected secret: number; }\n",
        ),
        (
            "node_modules/a/node_modules/dup/extra.d.ts",
            "export declare const extra: number;\n",
        ),
        (
            "node_modules/shared/package.json",
            &manifest("shared", "1.0.0"),
        ),
        (
            "node_modules/shared/index.d.ts",
            "export declare const shared: number;\n",
        ),
        ("src/main.ts", main),
    ])
}

/// The redirects as display paths: each file, the input it redirects to,
/// and their package id.
fn redirects(program: &Program) -> Vec<(String, String, String)> {
    let config = program.config.parent().unwrap();
    program
        .redirects
        .iter()
        .map(|redirect| {
            (
                redirect
                    .path
                    .strip_prefix(config)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                program.sources.get(redirect.target).display.clone(),
                redirect.package.to_string(),
            )
        })
        .collect()
}

#[test]
fn a_duplicate_package_file_is_a_redirect_and_what_only_it_reaches_is_no_input() {
    let main = "import { Box } from \"dup\";\nimport { make } from \"a\";\nimport \"shared\";\nexport const b: Box = make();\n";
    for dialect in DIALECTS {
        let project = layout(dialect, "", main);
        let program = project.load("tsconfig.json");
        assert_eq!(
            inputs(&program),
            [
                "node_modules/dup/index.d.ts",
                "node_modules/a/index.d.ts",
                "node_modules/shared/index.d.ts",
                "src/main.ts",
            ],
            "{dialect:?}"
        );
        assert_eq!(
            redirects(&program),
            [(
                "node_modules/a/node_modules/dup/index.d.ts".to_owned(),
                "node_modules/dup/index.d.ts".to_owned(),
                "dup/index.d.ts@1.0.0".to_owned(),
            )],
            "{dialect:?}"
        );
        // The redirect's import names the input it redirects to.
        let a = program
            .sources
            .ids()
            .find(|&id| program.sources.get(id).display == "node_modules/a/index.d.ts")
            .unwrap();
        let ModuleResolution::File(target) = program.files[a.index()].modules[0] else {
            panic!("{dialect:?}: {:?}", program.files[a.index()].modules);
        };
        assert_eq!(
            program.sources.get(target).display,
            "node_modules/dup/index.d.ts",
            "{dialect:?}"
        );
        // The raw resolution still names the duplicate, as
        // `GetResolvedModuleFromModuleSpecifier` observes it.
        assert!(
            program.files[a.index()].facts.requests[0]
                .resolved_path
                .as_deref()
                .is_some_and(|path| path.ends_with("a/node_modules/dup/index.d.ts")),
            "{dialect:?}"
        );
    }
}

/// Experiment F's swapped order: the nested copy, met first depth first, is
/// the input, though a breadth-first load reserves the root copy first.
#[test]
fn the_program_keeps_the_copy_collection_meets_first() {
    let main = "import { make } from \"a\";\nimport { Box } from \"dup\";\nimport \"shared\";\nexport const b: Box = make();\n";
    for dialect in DIALECTS {
        let program = layout(dialect, "", main).load("tsconfig.json");
        assert_eq!(
            inputs(&program),
            [
                "node_modules/a/node_modules/dup/extra.d.ts",
                "node_modules/shared/index.d.ts",
                "node_modules/a/node_modules/dup/index.d.ts",
                "node_modules/a/index.d.ts",
                "src/main.ts",
            ],
            "{dialect:?}"
        );
        assert_eq!(
            redirects(&program),
            [(
                "node_modules/dup/index.d.ts".to_owned(),
                "node_modules/a/node_modules/dup/index.d.ts".to_owned(),
                "dup/index.d.ts@1.0.0".to_owned(),
            )],
            "{dialect:?}"
        );
    }
}

/// 7.0.2 lists both copies under `deduplicatePackages: false`.
#[test]
fn without_deduplication_both_copies_are_inputs() {
    let main = "import { Box } from \"dup\";\nimport { make } from \"a\";\nexport const b: Box = make();\n";
    let program =
        layout(TypeScript7, r#","deduplicatePackages":false"#, main).load("tsconfig.json");
    assert_eq!(
        inputs(&program),
        [
            "node_modules/dup/index.d.ts",
            "node_modules/a/node_modules/dup/extra.d.ts",
            "node_modules/shared/index.d.ts",
            "node_modules/a/node_modules/dup/index.d.ts",
            "node_modules/a/index.d.ts",
            "src/main.ts",
        ]
    );
    assert!(program.redirects.is_empty());
}

/// A redirect's spellings name no input: `FileLookup` refuses them, naming
/// the input in its place.
#[test]
fn a_redirect_spelling_is_refused_by_file_lookup() {
    let main = "import { Box } from \"dup\";\nimport { make } from \"a\";\nexport const b: Box = make();\n";
    for dialect in DIALECTS {
        let project = layout(dialect, "", main);
        let program = project.load("tsconfig.json");
        let lookup = program.file_lookup();
        let duplicate = project
            .root
            .join("node_modules/a/node_modules/dup/index.d.ts");
        let error = lookup.find(&duplicate).unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Unsupported, "{dialect:?}");
        assert!(
            error.message.contains("'node_modules/dup/index.d.ts'"),
            "{dialect:?}: {error}"
        );
        // The input itself is a dependency, not searched.
        assert_eq!(
            lookup
                .find(&project.root.join("node_modules/dup/index.d.ts"))
                .unwrap(),
            None,
            "{dialect:?}"
        );
    }
}

/// Experiment M: `src/main.ts` reaches only the nested copy, through `a`,
/// and `src/other.ts` the root copy, which redirects to it. The nested copy
/// is named `dup` through the file that redirects to it, as both compilers'
/// declaration emit names it (`import("dup").Box`) and 5.6.3's
/// `getModuleSpecifiers` lists it; without deduplication 7.0.2 has only the
/// nested path, `a/node_modules/dup`, which its TS2883 reports.
#[test]
fn a_module_is_named_through_the_files_that_redirect_to_it() {
    use super::super::specifiers::{ImportingFile, module_specifiers};
    for (dialect, extra, expected) in [
        (TypeScript7, "", &["dup", "a/node_modules/dup"][..]),
        (TypeScript56, "", &["dup", "a/node_modules/dup"][..]),
        (
            TypeScript7,
            r#","deduplicatePackages":false"#,
            &["a/node_modules/dup"][..],
        ),
    ] {
        let config = configuration(extra, &["src/main.ts", "src/other.ts"]);
        let compiler = dialect.manifest();
        let dup = manifest("dup", "1.0.0");
        let text = "export declare class Box { protected secret: number; }\n";
        let project = Project::new(&[
            (MANIFEST, &compiler),
            ("tsconfig.json", &config),
            ("node_modules/dup/package.json", &dup),
            ("node_modules/dup/index.d.ts", text),
            ("node_modules/a/package.json", &manifest("a", "1.0.0")),
            (
                "node_modules/a/index.d.ts",
                "import { Box } from \"dup\";\nexport declare function make(): Box;\n",
            ),
            ("node_modules/a/node_modules/dup/package.json", &dup),
            ("node_modules/a/node_modules/dup/index.d.ts", text),
            (
                "src/main.ts",
                "import { make } from \"a\";\nexport const made = make();\n",
            ),
            (
                "src/other.ts",
                "import { Box } from \"dup\";\nexport declare const box: Box;\n",
            ),
        ]);
        let program = project.load("tsconfig.json");
        let root = std::fs::canonicalize(&project.root).unwrap();
        let importer = root.join("src/main.ts");
        let target = root.join("node_modules/a/node_modules/dup/index.d.ts");
        let id = program
            .sources
            .ids()
            .find(|&id| program.sources.get(id).path == importer)
            .unwrap();
        let specifiers = module_specifiers(
            &program.specifier_environment,
            ImportingFile {
                file_name: importer.to_str().unwrap(),
                default_mode: program.files[id.index()]
                    .module_semantics
                    .default_resolution_mode,
                imports: &[],
            },
            target.to_str().unwrap(),
            None,
            None,
        )
        .unwrap_or_else(|error| panic!("{dialect:?}{extra}: {error:?}"));
        assert_eq!(specifiers, expected, "{dialect:?}{extra}");
    }
}

/// The root copy imports `x`, whose own copy of `dup` redirects to the root
/// copy while collection is still inside the root copy's imports: 7.0.2
/// lists `x` before the root copy, which 5.6.3's listing does too once the
/// redirect, listed first, is dropped.
#[test]
fn a_redirect_met_inside_its_targets_imports_lists_the_target_in_its_place() {
    for dialect in DIALECTS {
        let config = configuration("", &["src/main.ts"]);
        let compiler = dialect.manifest();
        let dup = manifest("dup", "1.0.0");
        let program = Project::new(&[
            (MANIFEST, &compiler),
            ("tsconfig.json", &config),
            ("node_modules/dup/package.json", &dup),
            (
                "node_modules/dup/index.d.ts",
                "import \"x\";\nexport declare const which: \"root\";\n",
            ),
            ("node_modules/x/package.json", &manifest("x", "1.0.0")),
            ("node_modules/x/index.d.ts", "import \"dup\";\nexport {};\n"),
            ("node_modules/x/node_modules/dup/package.json", &dup),
            (
                "node_modules/x/node_modules/dup/index.d.ts",
                "import \"x\";\nexport declare const which: \"nested\";\n",
            ),
            (
                "src/main.ts",
                "import { which } from \"dup\";\nexport const v = which;\n",
            ),
        ])
        .load("tsconfig.json");
        assert_eq!(
            inputs(&program),
            [
                "node_modules/x/index.d.ts",
                "node_modules/dup/index.d.ts",
                "src/main.ts",
            ],
            "{dialect:?}"
        );
        assert_eq!(
            redirects(&program),
            [(
                "node_modules/x/node_modules/dup/index.d.ts".to_owned(),
                "node_modules/dup/index.d.ts".to_owned(),
                "dup/index.d.ts@1.0.0".to_owned(),
            )],
            "{dialect:?}"
        );
    }
}

/// A `types` entry's package is a root task that collection meets after the
/// roots: under 5.6.3 its file has the directory's package id, so the copy
/// in `@types` redirects to the nested copy the roots reached first; 7.0.2
/// gives the type package no id and lists both copies.
#[test]
fn a_type_package_redirects_only_under_5_6() {
    for (dialect, expected, redirected) in [
        (
            TypeScript7,
            &[
                "node_modules/x/node_modules/@types/foo/index.d.ts",
                "node_modules/x/index.d.ts",
                "src/main.ts",
                "node_modules/@types/foo/index.d.ts",
            ][..],
            false,
        ),
        (
            TypeScript56,
            &[
                "node_modules/x/node_modules/@types/foo/index.d.ts",
                "node_modules/x/index.d.ts",
                "src/main.ts",
            ][..],
            true,
        ),
    ] {
        let config =
            configuration("", &["src/main.ts"]).replace(r#""types":[]"#, r#""types":["foo"]"#);
        let compiler = dialect.manifest();
        let foo = r#"{"name":"@types/foo","version":"1.0.0"}"#;
        let program = Project::new(&[
            (MANIFEST, &compiler),
            ("tsconfig.json", &config),
            ("node_modules/@types/foo/package.json", foo),
            (
                "node_modules/@types/foo/index.d.ts",
                "export declare const which: \"root\";\n",
            ),
            ("node_modules/x/package.json", &manifest("x", "1.0.0")),
            (
                "node_modules/x/index.d.ts",
                "import { which } from \"foo\";\nexport declare const viaX: typeof which;\n",
            ),
            ("node_modules/x/node_modules/@types/foo/package.json", foo),
            (
                "node_modules/x/node_modules/@types/foo/index.d.ts",
                "export declare const which: \"nested\";\n",
            ),
            (
                "src/main.ts",
                "import { viaX } from \"x\";\nexport const v = viaX;\n",
            ),
        ])
        .load("tsconfig.json");
        assert_eq!(inputs(&program), expected, "{dialect:?}");
        let expected_redirects: Vec<(String, String, String)> = if redirected {
            vec![(
                "node_modules/@types/foo/index.d.ts".to_owned(),
                "node_modules/x/node_modules/@types/foo/index.d.ts".to_owned(),
                "@types/foo/index.d.ts@1.0.0".to_owned(),
            )]
        } else {
            Vec::new()
        };
        assert_eq!(redirects(&program), expected_redirects, "{dialect:?}");
    }
}

/// The provenance experiments' layout: `src/main.ts` imports the root
/// `dup`, then `a`, whose nested copy of `dup` redirects to the root copy,
/// then `shared`; a later root, `src/refs.ts`, names a file by a reference
/// path. `files` gives each copy's text and any other file, and may replace
/// the roots'.
fn provenance_layout(dialect: Dialect, files: &[(&str, &str)]) -> Project {
    let config = configuration("", &["src/main.ts", "src/refs.ts"]);
    let compiler = dialect.manifest();
    let dup = manifest("dup", "1.0.0");
    let project = Project::new(&[
        (MANIFEST, &compiler),
        ("tsconfig.json", &config),
        ("node_modules/dup/package.json", &dup),
        ("node_modules/a/package.json", &manifest("a", "1.0.0")),
        ("node_modules/a/index.d.ts", "import \"dup\";\nexport {};\n"),
        ("node_modules/a/node_modules/dup/package.json", &dup),
        (
            "node_modules/shared/package.json",
            &manifest("shared", "1.0.0"),
        ),
        (
            "node_modules/shared/index.d.ts",
            "export declare const shared: number;\n",
        ),
        (
            "src/main.ts",
            "import \"dup\";\nimport \"a\";\nimport \"shared\";\nexport {};\n",
        ),
        (
            "src/refs.ts",
            "/// <reference path=\"../node_modules/a/node_modules/dup/index.d.ts\" />\nexport {};\n",
        ),
    ]);
    project.write(files);
    project
}

/// A file and its text, or an input and its provenance.
type Pair<'a> = (&'a str, &'a str);

/// Each input in program order with its provenance.
fn origins(program: &Program) -> Vec<(&str, &str)> {
    program
        .sources
        .ids()
        .map(|id| {
            let origin = match program.files[id.index()].diagnostics.origin() {
                FileOrigin::Project => "project",
                FileOrigin::Dependency => "dependency",
                FileOrigin::DefaultLibrary => "library",
            };
            (program.sources.get(id).display.as_str(), origin)
        })
        .collect()
}

/// What decides whether an input is found in a package (`FileOrigin`) is
/// the routes each compiler counts. 7.0.2 gives each loaded file the lowest
/// depth of any route to it before collection deduplicates (`lowestDepth`),
/// so the redirected copy's own reference to `shared`, which `src/refs.ts`
/// reaches without a package lookup, makes `shared` a project file. 5.6.3
/// counts only the routes `findSourceFileWorker` processes, never a
/// redirect's own subtasks, so `shared` stays a dependency, whether a root
/// reaches the redirect directly or through a dependency file it reaches
/// again (`node_modules/a/hop.d.ts`), and whether the redirect was met
/// through a package lookup or, under 5.6.3, through a relative import of a
/// directory with a manifest (`vendor/dup`), which 7.0.2 gives no package
/// id. Inputs and provenance are both compilers' (`tools/program-inputs.mjs`
/// with and without `--oracle=5.6`).
#[test]
fn provenance_counts_the_routes_each_dialects_collection_processes() {
    let nested = "/// <reference path=\"../../../shared/index.d.ts\" />\nexport declare const which: \"nested\";\n";
    let root = "export declare const which: \"root\";\n";
    let direct: &[(&str, &str)] = &[
        ("node_modules/dup/index.d.ts", root),
        ("node_modules/a/node_modules/dup/index.d.ts", nested),
    ];
    let hop: &[(&str, &str)] = &[
        ("node_modules/dup/index.d.ts", root),
        ("node_modules/a/node_modules/dup/index.d.ts", nested),
        ("node_modules/b/package.json", &manifest("b", "1.0.0")),
        (
            "node_modules/b/index.d.ts",
            "/// <reference path=\"../a/hop.d.ts\" />\nexport {};\n",
        ),
        (
            "node_modules/a/hop.d.ts",
            "/// <reference path=\"./node_modules/dup/index.d.ts\" />\ndeclare const hop: 1;\n",
        ),
        (
            "src/main.ts",
            "import \"dup\";\nimport \"a\";\nimport \"b\";\nimport \"shared\";\nexport {};\n",
        ),
        (
            "src/refs.ts",
            "/// <reference path=\"../node_modules/a/hop.d.ts\" />\nexport {};\n",
        ),
    ];
    let vendor_manifest = manifest("dup", "1.0.0");
    let vendor: &[(&str, &str)] = &[
        (
            "node_modules/dup/index.d.ts",
            "/// <reference path=\"./note.d.ts\" />\nexport declare const which: \"root\";\n",
        ),
        ("node_modules/dup/note.d.ts", "declare const rootNote: 1;\n"),
        ("vendor/dup/package.json", &vendor_manifest),
        (
            "vendor/dup/index.d.ts",
            "/// <reference path=\"../../node_modules/shared/index.d.ts\" />\nexport declare const which: \"vendor\";\n",
        ),
        ("vendor/dup/note.d.ts", "declare const vendorNote: 1;\n"),
        (
            "src/main.ts",
            "import \"dup\";\nimport \"../vendor/dup\";\nimport \"shared\";\nexport {};\n",
        ),
        (
            "src/refs.ts",
            "/// <reference path=\"../vendor/dup/index.d.ts\" />\nexport {};\n",
        ),
    ];
    let nested_redirect = || {
        vec![(
            "node_modules/a/node_modules/dup/index.d.ts".to_owned(),
            "node_modules/dup/index.d.ts".to_owned(),
            "dup/index.d.ts@1.0.0".to_owned(),
        )]
    };
    let cases = [
        (
            TypeScript7,
            "direct",
            direct,
            vec![
                ("node_modules/dup/index.d.ts", "dependency"),
                ("node_modules/a/index.d.ts", "dependency"),
                ("node_modules/shared/index.d.ts", "project"),
                ("src/main.ts", "project"),
                ("src/refs.ts", "project"),
            ],
            nested_redirect(),
        ),
        (
            TypeScript56,
            "direct",
            direct,
            vec![
                ("node_modules/dup/index.d.ts", "dependency"),
                ("node_modules/a/index.d.ts", "dependency"),
                ("node_modules/shared/index.d.ts", "dependency"),
                ("src/main.ts", "project"),
                ("src/refs.ts", "project"),
            ],
            nested_redirect(),
        ),
        (
            TypeScript7,
            "through a dependency file",
            hop,
            vec![
                ("node_modules/dup/index.d.ts", "dependency"),
                ("node_modules/a/index.d.ts", "dependency"),
                ("node_modules/a/hop.d.ts", "project"),
                ("node_modules/b/index.d.ts", "dependency"),
                ("node_modules/shared/index.d.ts", "project"),
                ("src/main.ts", "project"),
                ("src/refs.ts", "project"),
            ],
            nested_redirect(),
        ),
        (
            TypeScript56,
            "through a dependency file",
            hop,
            vec![
                ("node_modules/dup/index.d.ts", "dependency"),
                ("node_modules/a/index.d.ts", "dependency"),
                ("node_modules/a/hop.d.ts", "project"),
                ("node_modules/b/index.d.ts", "dependency"),
                ("node_modules/shared/index.d.ts", "dependency"),
                ("src/main.ts", "project"),
                ("src/refs.ts", "project"),
            ],
            nested_redirect(),
        ),
        (
            TypeScript7,
            "a relative directory import",
            vendor,
            vec![
                ("node_modules/dup/note.d.ts", "dependency"),
                ("node_modules/dup/index.d.ts", "dependency"),
                ("node_modules/shared/index.d.ts", "project"),
                ("vendor/dup/index.d.ts", "project"),
                ("src/main.ts", "project"),
                ("src/refs.ts", "project"),
            ],
            Vec::new(),
        ),
        // The copy in `vendor` redirects where a route without a package
        // lookup meets it, so a later such route does not process it again.
        (
            TypeScript56,
            "a relative directory import",
            vendor,
            vec![
                ("node_modules/dup/note.d.ts", "dependency"),
                ("node_modules/dup/index.d.ts", "dependency"),
                ("node_modules/shared/index.d.ts", "dependency"),
                ("src/main.ts", "project"),
                ("src/refs.ts", "project"),
            ],
            vec![(
                "vendor/dup/index.d.ts".to_owned(),
                "node_modules/dup/index.d.ts".to_owned(),
                "dup/index.d.ts@1.0.0".to_owned(),
            )],
        ),
    ];
    for (dialect, route, files, expected, expected_redirects) in cases {
        let program = provenance_layout(dialect, files).load("tsconfig.json");
        assert_eq!(origins(&program), expected, "{dialect:?}, {route}");
        assert_eq!(
            redirects(&program),
            expected_redirects,
            "{dialect:?}, {route}"
        );
    }
}

/// When a route without a package lookup reaches a redirect that collection
/// met through one, 5.6.3's `findSourceFileWorker` processes it again
/// (`sourceFilesFoundSearchingNodeModules`): it follows its target's
/// reference paths, type references, and imports, resolved from the
/// redirect's own location, where that route meets it. The root copy's
/// reference to `./note.d.ts` then makes the nested `note.d.ts`, which
/// neither copy names from where it is, a project input; its import of
/// `./note` makes the nested `note.d.ts` another redirect, to the root
/// copy's; its type reference finds the `@types/extra` beside the nested
/// copy. That holds whether a root references the redirect, a root reaches
/// it through a dependency file it reaches again (`node_modules/a/hop.d.ts`),
/// or the redirect is a root itself, listed after `src/main.ts`. 7.0.2
/// never processes a redirect's subtasks. Inputs, provenance, and redirects
/// are both compilers' (`tools/program-inputs.mjs` with and without
/// `--oracle=5.6`).
#[test]
fn a_redirect_typescript_5_6_processes_again_follows_its_targets_subtasks_from_its_location() {
    let notes = [
        (
            "node_modules/dup/note.d.ts",
            "export declare const rootNote: 1;\n",
        ),
        (
            "node_modules/a/node_modules/dup/note.d.ts",
            "export declare const nestedNote: 1;\n",
        ),
    ];
    let extra_manifest = r#"{"name":"@types/extra","version":"1.0.0"}"#;
    let extra = [
        (
            "node_modules/a/node_modules/@types/extra/package.json",
            extra_manifest,
        ),
        (
            "node_modules/a/node_modules/@types/extra/index.d.ts",
            "declare const extra: 1;\n",
        ),
    ];
    /// One kind of the root copy's subtasks.
    struct Subtask<'a> {
        name: &'a str,
        /// The root copy's text.
        root_copy: &'a str,
        /// The files it may reach.
        reached: &'a [Pair<'a>],
        /// What 5.6.3 adds after `src/main.ts`.
        added: Option<Pair<'a>>,
        /// The further redirect 5.6.3 makes, to its target.
        redirected: Option<Pair<'a>>,
    }
    let subtasks = [
        Subtask {
            name: "a reference path",
            root_copy: "/// <reference path=\"./note.d.ts\" />\nexport declare const which: \"root\";\n",
            reached: &notes,
            added: Some(("node_modules/a/node_modules/dup/note.d.ts", "project")),
            redirected: None,
        },
        Subtask {
            name: "an import",
            root_copy: "import \"./note\";\nexport declare const which: \"root\";\n",
            reached: &notes,
            added: None,
            redirected: Some((
                "node_modules/a/node_modules/dup/note.d.ts",
                "node_modules/dup/note.d.ts",
            )),
        },
        Subtask {
            name: "a type reference",
            root_copy: "/// <reference types=\"extra\" />\nexport declare const which: \"root\";\n",
            reached: &extra,
            added: Some((
                "node_modules/a/node_modules/@types/extra/index.d.ts",
                "dependency",
            )),
            redirected: None,
        },
    ];
    let b = manifest("b", "1.0.0");
    let hop = [
        ("node_modules/b/package.json", b.as_str()),
        (
            "node_modules/b/index.d.ts",
            "/// <reference path=\"../a/hop.d.ts\" />\nexport {};\n",
        ),
        (
            "node_modules/a/hop.d.ts",
            "/// <reference path=\"./node_modules/dup/index.d.ts\" />\ndeclare const hop: 1;\n",
        ),
        (
            "src/main.ts",
            "import \"dup\";\nimport \"a\";\nimport \"b\";\nimport \"shared\";\nexport {};\n",
        ),
        (
            "src/refs.ts",
            "/// <reference path=\"../node_modules/a/hop.d.ts\" />\nexport {};\n",
        ),
    ];
    let as_root = configuration(
        "",
        &["src/main.ts", "node_modules/a/node_modules/dup/index.d.ts"],
    );
    let root = [("tsconfig.json", as_root.as_str())];
    for Subtask {
        name: subtask,
        root_copy,
        reached,
        added,
        redirected,
    } in subtasks
    {
        for (route, routes) in [
            ("a root's reference", &[][..]),
            ("a dependency file a root reaches again", &hop[..]),
            ("a root", &root[..]),
        ] {
            let mut files = vec![
                ("node_modules/dup/index.d.ts", root_copy),
                (
                    "node_modules/a/node_modules/dup/index.d.ts",
                    "export declare const which: \"nested\";\n",
                ),
            ];
            files.extend_from_slice(reached);
            files.extend_from_slice(routes);
            // What 7.0.2 lists.
            let mut expected: Vec<Pair> = Vec::new();
            if reached == &notes[..] {
                expected.push(("node_modules/dup/note.d.ts", "dependency"));
            }
            expected.extend([
                ("node_modules/dup/index.d.ts", "dependency"),
                ("node_modules/a/index.d.ts", "dependency"),
            ]);
            if routes == &hop[..] {
                expected.extend([
                    ("node_modules/a/hop.d.ts", "project"),
                    ("node_modules/b/index.d.ts", "dependency"),
                ]);
            }
            expected.extend([
                ("node_modules/shared/index.d.ts", "dependency"),
                ("src/main.ts", "project"),
            ]);
            if routes != &root[..] {
                expected.push(("src/refs.ts", "project"));
            }
            let mut expected_redirects = vec![(
                "node_modules/a/node_modules/dup/index.d.ts".to_owned(),
                "node_modules/dup/index.d.ts".to_owned(),
                "dup/index.d.ts@1.0.0".to_owned(),
            )];
            let context = format!("{subtask}, {route}");
            let program = provenance_layout(TypeScript7, &files).load("tsconfig.json");
            assert_eq!(origins(&program), expected, "7.0.2, {context}");
            assert_eq!(redirects(&program), expected_redirects, "7.0.2, {context}");
            // 5.6.3 lists what the redirect's processing adds where the
            // route meets it, during `src/refs.ts` or after `src/main.ts`.
            if let Some(added) = added {
                let at = expected
                    .iter()
                    .position(|&(file, _)| file == "src/main.ts")
                    .unwrap();
                expected.insert(at + 1, added);
            }
            if let Some((file, target)) = redirected {
                expected_redirects.push((
                    file.to_owned(),
                    target.to_owned(),
                    "dup/note.d.ts@1.0.0".to_owned(),
                ));
            }
            let program = provenance_layout(TypeScript56, &files).load("tsconfig.json");
            assert_eq!(origins(&program), expected, "5.6.3, {context}");
            assert_eq!(redirects(&program), expected_redirects, "5.6.3, {context}");
        }
    }
}

/// A library reference names the same file from any location, so 5.6.3
/// processing the redirect again adds nothing: the program is the one
/// `tools/program-inputs.mjs --oracle=5.6` lists, with `shared` a
/// dependency.
#[test]
fn a_redirect_typescript_5_6_processes_again_loads_where_only_a_library_reference_follows() {
    let Some(package) = typescript_5_6(
        "a_redirect_typescript_5_6_processes_again_loads_where_only_a_library_reference_follows",
    ) else {
        return;
    };
    let config = configuration("", &["src/main.ts", "src/refs.ts"])
        .replace(r#""noLib":true"#, r#""lib":["es5"]"#);
    let project = provenance_layout(
        TypeScript56,
        &[
            ("tsconfig.json", &config),
            (
                "node_modules/dup/index.d.ts",
                "/// <reference lib=\"es2015.promise\" />\nexport declare const which: \"root\";\n",
            ),
            (
                "node_modules/a/node_modules/dup/index.d.ts",
                "/// <reference path=\"../../../shared/index.d.ts\" />\nexport declare const which: \"nested\";\n",
            ),
        ],
    );
    let lib = project.install_compiler(&package);
    let program = project.load("tsconfig.json");
    assert_eq!(
        listed_in(&program, &lib),
        [
            "lib.es5.d.ts",
            "lib.es2015.promise.d.ts",
            "lib.decorators.d.ts",
            "lib.decorators.legacy.d.ts",
            "node_modules/dup/index.d.ts",
            "node_modules/a/index.d.ts",
            "node_modules/shared/index.d.ts",
            "src/main.ts",
            "src/refs.ts",
        ]
    );
    assert_eq!(searched(&program), ["src/main.ts", "src/refs.ts"]);
    assert_eq!(
        redirects(&program),
        [(
            "node_modules/a/node_modules/dup/index.d.ts".to_owned(),
            "node_modules/dup/index.d.ts".to_owned(),
            "dup/index.d.ts@1.0.0".to_owned(),
        )]
    );
}
