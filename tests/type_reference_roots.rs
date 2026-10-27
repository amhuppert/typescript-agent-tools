//! Type-reference lookup (`resolveTypeReferenceDirective`), at
//! `Project::load`: which type package each name reaches, as the file
//! declaring its global shows, under TypeScript 7 and the TypeScript 5.6
//! dialect alike. Under explicit `typeRoots` a name is first a declaration
//! file in the root, then a package directory; a name the configuration
//! infers (a `types` entry or an automatic name) is looked up nowhere
//! else, while one a `/// <reference types>` directive writes falls back to
//! `node_modules`. Expected files are native 7.0.2's and 5.6.3's
//! (`tsc --listFilesOnly --incremental false` on scratch copies), which
//! agree on every case.

mod support;

use support::Dialect::{self, TypeScript7, TypeScript56};
use support::{DIALECTS, Fixture};
use typescript_parser::analysis::{InputError, InputErrorKind};
use typescript_parser::project::Project;
use typescript_parser::references::ReferenceQuery;
use typescript_parser::source::SourcePosition;

/// The type packages of every case: each declares a global named for it.
const PACKAGES: &[(&str, &str)] = &[
    ("types/direct.d.ts", "declare const direct: 1;\n"),
    ("types/indir/index.d.ts", "declare const indir: 1;\n"),
    ("types/both.d.ts", "declare const bothFile: 1;\n"),
    ("types/both/index.d.ts", "declare const bothDir: 1;\n"),
    ("types/@sc/nm.d.ts", "declare const scoped: 1;\n"),
    (
        "node_modules/@types/only-in-nm/index.d.ts",
        "declare const onlyInNm: 1;\n",
    ),
    (
        "node_modules/@types/sc__mangled/index.d.ts",
        "declare const mangled: 1;\n",
    ),
    (
        "node_modules/@types/nm-file.d.ts",
        "declare const nmFile: 1;\n",
    ),
];

const GLOBALS: &[&str] = &[
    "direct", "indir", "bothFile", "bothDir", "scoped", "onlyInNm", "mangled", "nmFile",
];

/// A loaded scratch project and where `main.ts` uses each of `GLOBALS`.
struct Loaded {
    project: Project,
    main: std::path::PathBuf,
    /// The line of `GLOBALS[0]`'s use; each next global's is the next line.
    first_use: usize,
    _fixture: Fixture,
}

fn load(
    dialect: Dialect,
    options: &str,
    main: &str,
    packages: &[(&str, &str)],
) -> Result<Loaded, InputError> {
    let config = format!(
        r#"{{"compilerOptions":{{"noLib":true,"strict":true,"target":"es2022","module":"esnext","moduleResolution":"bundler","esModuleInterop":true,{options}}},"files":["main.ts"]}}"#
    );
    // A use of each global, whose references name its declaration wherever
    // the program has one; a package in `node_modules` is not searched.
    let first_use = main.lines().count() + 1;
    let main = format!("{main}{};\n", GLOBALS.join(";\n"));
    let mut files = vec![
        ("tsconfig.json", config.as_str()),
        ("main.ts", main.as_str()),
    ];
    files.extend_from_slice(packages);
    let fixture = Fixture::with_dialect(dialect, &files);
    let project = fixture.try_load()?;
    Ok(Loaded {
        project,
        main: fixture.path("main.ts"),
        first_use,
        _fixture: fixture,
    })
}

/// The file declaring each global the program has, in `GLOBALS` order: the
/// declaration its use in `main.ts` refers to.
fn declared(loaded: &Loaded) -> Vec<String> {
    (0..GLOBALS.len())
        .filter_map(|index| {
            let result = loaded
                .project
                .find_references(ReferenceQuery::At {
                    file: loaded.main.clone(),
                    position: SourcePosition {
                        line: loaded.first_use + index,
                        column: 1,
                    },
                })
                .ok()?;
            let group = result.groups.first()?;
            Some(group.declarations.first()?.location.file.clone())
        })
        .collect()
}

fn loads(dialect: Dialect, options: &str, main: &str, expected: &[&str]) {
    let loaded = load(dialect, options, main, PACKAGES)
        .unwrap_or_else(|error| panic!("{dialect:?} {options}: {error}"));
    assert_eq!(declared(&loaded), expected, "{dialect:?} {options}");
}

fn fails(dialect: Dialect, options: &str, main: &str, name: &str) {
    let error = load(dialect, options, main, PACKAGES)
        .err()
        .unwrap_or_else(|| panic!("{dialect:?} {options} loaded"));
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
    assert_eq!(
        error.message,
        format!("Cannot find type definition file for '{name}'"),
        "{dialect:?} {options}"
    );
}

/// Under explicit roots a declaration file in the root comes before a
/// package directory of the same name (`both`), and a scoped name keeps
/// its scope in a root that is not `node_modules/@types`. A written
/// reference to a name only `node_modules` has still finds it.
#[test]
fn explicit_type_roots_try_a_declaration_file_before_a_directory() {
    for dialect in DIALECTS {
        loads(
            dialect,
            r#""typeRoots":["./types"],"types":["direct","indir","both","@sc/nm"]"#,
            "/// <reference types=\"only-in-nm\" />\nexport {};\n",
            &[
                "types/direct.d.ts",
                "types/indir/index.d.ts",
                "types/both.d.ts",
                "types/@sc/nm.d.ts",
                "node_modules/@types/only-in-nm/index.d.ts",
            ],
        );
        // A written reference finds a declaration file in the root too.
        loads(
            dialect,
            r#""typeRoots":["./types"],"types":[]"#,
            "/// <reference types=\"direct\" />\n/// <reference types=\"both\" />\nexport {};\n",
            &["types/direct.d.ts", "types/both.d.ts"],
        );
        // A root that is `node_modules/@types` mangles a scoped name, and
        // holds declaration files too.
        loads(
            dialect,
            r#""typeRoots":["./node_modules/@types"],"types":["@sc/mangled","nm-file"]"#,
            "export {};\n",
            &[
                "node_modules/@types/sc__mangled/index.d.ts",
                "node_modules/@types/nm-file.d.ts",
            ],
        );
    }
}

/// With explicit roots, empty ones included, a name the configuration
/// lists is looked up in them alone (TS2688), while a written reference
/// falls back to `node_modules`. With the default roots, every name falls
/// back, which finds a declaration file directly in `@types`.
#[test]
fn only_inferred_names_skip_node_modules_under_explicit_roots() {
    for dialect in DIALECTS {
        for roots in [r#"["./types"]"#, "[]"] {
            fails(
                dialect,
                &format!(r#""typeRoots":{roots},"types":["only-in-nm"]"#),
                "export {};\n",
                "only-in-nm",
            );
            loads(
                dialect,
                &format!(r#""typeRoots":{roots},"types":[]"#),
                "/// <reference types=\"only-in-nm\" />\nexport {};\n",
                &["node_modules/@types/only-in-nm/index.d.ts"],
            );
        }
        loads(
            dialect,
            r#""types":["@sc/mangled","nm-file","only-in-nm"]"#,
            "export {};\n",
            &[
                "node_modules/@types/only-in-nm/index.d.ts",
                "node_modules/@types/sc__mangled/index.d.ts",
                "node_modules/@types/nm-file.d.ts",
            ],
        );
    }
}

/// The automatic names are each root's directories, and under explicit
/// roots each is first a declaration file: `both` is `both.d.ts`. 5.6
/// selects them when `types` is omitted, TypeScript 7 for `"*"`.
#[test]
fn automatic_names_under_explicit_roots_try_a_declaration_file_first() {
    let packages: Vec<(&str, &str)> = PACKAGES
        .iter()
        .copied()
        .filter(|(path, _)| !path.starts_with("types/@sc") && *path != "types/direct.d.ts")
        .collect();
    for (dialect, types) in [(TypeScript56, ""), (TypeScript7, r#","types":["*"]"#)] {
        let options = format!(r#""typeRoots":["./types"]{types}"#);
        let loaded = load(dialect, &options, "export {};\n", &packages)
            .unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
        assert_eq!(
            declared(&loaded),
            ["types/indir/index.d.ts", "types/both.d.ts"],
            "{dialect:?}"
        );
    }
}

/// An ESM-mode lookup adds no extension and reads no directory index, in
/// a root as anywhere: `direct` is not found, and `both` in CommonJS mode
/// is its declaration file.
#[test]
fn type_roots_follow_the_reference_mode() {
    for dialect in DIALECTS {
        let options = r#""typeRoots":["./types"],"types":[],"module":"nodenext","moduleResolution":"nodenext""#;
        let loaded = load(
            dialect,
            options,
            "/// <reference types=\"both\" resolution-mode=\"require\" />\nexport {};\n",
            PACKAGES,
        )
        .unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
        assert_eq!(declared(&loaded), ["types/both.d.ts"], "{dialect:?}");
        let error = load(
            dialect,
            options,
            "/// <reference types=\"direct\" resolution-mode=\"import\" />\nexport {};\n",
            PACKAGES,
        )
        .err()
        .unwrap_or_else(|| panic!("{dialect:?}: an ESM reference added an extension"));
        assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
    }
}
