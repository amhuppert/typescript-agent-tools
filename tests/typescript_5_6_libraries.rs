// validation: run alone: it starts a project service, which must become ready within ten seconds.
//! Standard libraries under the TypeScript 5.6 dialect at their final
//! outcomes: a project's binding, `--type` answers, a session's refreshed
//! answers, and the CLI from another directory. Each project installs a copy
//! of the pinned TypeScript 5.6.3 package (`support::typescript_5_6`) as its
//! compiler, whose own declarations the dialect reads. Expected answers are
//! TypeScript 5.6.3's (`tools/ts56-oracle.mjs`, `tsc --incremental false`)
//! and, for the same project under TypeScript 7, 7.0.2's native compiler.
#![cfg(unix)]
mod support;

use std::path::Path;
use support::service::Service;
use support::{Fixture, manifest};
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::{Project, ProjectRequest, ProjectResponse, ProjectSession};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeQuery, TypeRequest};

/// Options both dialects read alike, beside the `lib` a test sets.
const OPTIONS: &str = r#""types": [], "strict": true, "target": "ES2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true"#;

fn config(options: &str) -> String {
    format!(r#"{{ "compilerOptions": {{ {OPTIONS}{options} }}, "files": ["a.ts"] }}"#)
}

fn type_request(file: &Path, line: usize, column: usize) -> ProjectRequest {
    ProjectRequest::Type(TypeRequest::new(TypeQuery::At {
        file: file.to_path_buf(),
        position: SourcePosition { line, column },
    }))
}

/// The summary of a type answer, or the failure's message.
fn summary(response: Result<ProjectResponse, impl std::fmt::Display>) -> Result<String, String> {
    match response {
        Ok(ProjectResponse::Type(answer)) => match *answer {
            TypeAnswer::Selected(result) => Ok(result.description.summary),
            other => panic!("a type request selected nothing: {other:?}"),
        },
        Ok(other) => panic!("a type request answered {other:?}"),
        Err(error) => Err(error.to_string()),
    }
}

/// TypeScript 5.6.3's `lib: ["esnext"]` has no `esnext.date`, so Artemis's
/// `Date.toTemporalInstant` property augmentation is the only declaration
/// and the project binds, with 5.6.3's type for its use (quick info
/// `const instant: { epochMilliseconds: number; }`). Under TypeScript 7 the
/// vendored `lib.esnext.date.d.ts` declares the method too, and the
/// duplicate fails where the project writes it, as 7.0.2 reports TS2300 at
/// `temporal.ts(5,5)`.
#[test]
fn a_temporal_augmentation_binds_under_typescript_5_6_and_duplicates_under_7() {
    let Some(package) = support::typescript_5_6(
        "a_temporal_augmentation_binds_under_typescript_5_6_and_duplicates_under_7",
    ) else {
        return;
    };
    let fixture = Fixture::new(&[
        ("tsconfig.json", &config(r#", "lib": ["esnext"]"#)),
        (
            "a.ts",
            "declare function toTemporalInstant(this: Date): { epochMilliseconds: number };\n\ndeclare global {\n  interface Date {\n    toTemporalInstant: typeof toTemporalInstant;\n  }\n}\n\nexport const instant = new Date().toTemporalInstant();\n",
        ),
    ]);
    fixture.install_compiler(&package);
    let project = Project::load(&fixture.path("tsconfig.json")).unwrap();
    assert_eq!(
        summary(project.execute(type_request(&fixture.path("a.ts"), 9, 14))),
        Ok("{ epochMilliseconds: number; }".into())
    );
    fixture.claim_compiler("7.0.2");
    let error = fixture.try_load().unwrap_err();
    assert_eq!(
        (
            error.kind,
            error.path.as_deref(),
            error.position,
            error.message.as_str()
        ),
        (
            InputErrorKind::Binding,
            Some(fixture.path("a.ts").as_path()),
            Some(SourcePosition { line: 5, column: 5 }),
            "duplicate declaration of member"
        )
    );
}

/// `--type` answers that depend on the library: 5.6.3's `TextEncoder`
/// returns the non-generic `Uint8Array` where 7.0.2's returns
/// `Uint8Array<ArrayBuffer>`, and DOM and `Promise` values have the types
/// both compilers give them, the same through the service as loaded
/// directly.
#[test]
fn library_dependent_types_are_typescript_5_6s_through_the_service_and_directly() {
    let Some(package) = support::typescript_5_6(
        "library_dependent_types_are_typescript_5_6s_through_the_service_and_directly",
    ) else {
        return;
    };
    let service = Service::of(Fixture::new(&[
        (
            "tsconfig.json",
            &config(r#", "lib": ["dom", "dom.iterable", "esnext"]"#),
        ),
        (
            "a.ts",
            "export const encoded = new TextEncoder().encode(\"x\");\nexport const anchor = document.querySelector(\"a\");\nexport const settled = Promise.allSettled([1]);\n",
        ),
    ]));
    service.fixture.install_compiler(&package);
    let answer = |at: &str| {
        let output = service.both(&["--type", "--at", at, "--json"]);
        assert!(output.status.success(), "{at}: {output:?}");
        support::service::json(output)["type"]["summary"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    for (at, typescript_5_6, typescript_7) in [
        ("a.ts:1:14", "Uint8Array", "Uint8Array<ArrayBuffer>"),
        (
            "a.ts:2:14",
            "HTMLAnchorElement | null",
            "HTMLAnchorElement | null",
        ),
        (
            "a.ts:3:14",
            "Promise<[PromiseSettledResult<number>]>",
            "Promise<[PromiseSettledResult<number>]>",
        ),
    ] {
        service.fixture.claim_compiler("5.6.3");
        assert_eq!(answer(at), typescript_5_6, "5.6.3 {at}");
        service.fixture.claim_compiler("7.0.2");
        assert_eq!(answer(at), typescript_7, "7.0.2 {at}");
    }
}

/// A session's answer equals a fresh direct load's after an installed
/// library file is edited and after a `@typescript/lib-*` package appears
/// beside the configuration, which the TypeScript 5.6 dialect then reads
/// in place of the installed file.
#[test]
fn a_session_refreshes_when_an_installed_library_or_a_replacement_changes() {
    let Some(package) = support::typescript_5_6(
        "a_session_refreshes_when_an_installed_library_or_a_replacement_changes",
    ) else {
        return;
    };
    let fixture = Fixture::new(&[
        ("tsconfig.json", &config(r#", "lib": ["es5", "dom"]"#)),
        ("a.ts", "export const value = libraryValue;\n"),
    ]);
    fixture.install_compiler(&package);
    let config = fixture.path("tsconfig.json");
    let request = || type_request(&fixture.path("a.ts"), 1, 14);
    let mut session = ProjectSession::new(&config).unwrap();
    let mut ask = |step: &str| {
        let refreshed = summary(session.execute(request()));
        let fresh = summary(Project::load(&config).and_then(|project| project.execute(request())));
        assert_eq!(refreshed, fresh, "{step}");
        refreshed
    };
    assert!(ask("undeclared").is_err());
    let dom = fixture.path("node_modules/typescript/lib/lib.dom.d.ts");
    let mut text = std::fs::read_to_string(&dom).unwrap();
    text.push_str("declare var libraryValue: number;\n");
    std::fs::write(&dom, text).unwrap();
    assert_eq!(ask("installed library edited"), Ok("number".into()));
    let replacement = fixture.path("node_modules/@typescript/lib-dom");
    std::fs::create_dir_all(&replacement).unwrap();
    std::fs::write(
        replacement.join("package.json"),
        r#"{ "name": "@typescript/lib-dom", "types": "index.d.ts" }"#,
    )
    .unwrap();
    std::fs::write(
        replacement.join("index.d.ts"),
        "declare var libraryValue: string;\n",
    )
    .unwrap();
    assert_eq!(ask("replacement installed"), Ok("string".into()));
}

/// The installed library directory is the `lib` of the package's real
/// directory, wherever its `package.json` links: 5.6.3 reads the libraries
/// beside the `tsc.js` it runs (`getDefaultLibLocation`), and a copy whose
/// `node_modules/typescript/package.json` links to `metadata/package.json`
/// reads `node_modules/typescript/lib` (`tsc --listFilesOnly`) and gives the
/// edited `lib.dom.d.ts`'s `libraryValue` its type. A session notices the
/// manifest relinked to a TypeScript 7 manifest, which selects that
/// dialect, the package directory relinked to another copy, whose own
/// `lib` declares `libraryValue` otherwise, and that package's `lib`, made a
/// link, retargeted to a third copy's while its manifest and directory stay
/// as they are; each answer equals a fresh direct load's.
#[test]
fn the_library_directory_is_the_real_package_directorys() {
    let Some(package) =
        support::typescript_5_6("the_library_directory_is_the_real_package_directorys")
    else {
        return;
    };
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new(&[
        ("tsconfig.json", &config(r#", "lib": ["es5", "dom"]"#)),
        ("a.ts", "export const value = libraryValue;\n"),
        ("metadata/five.json", &manifest("5.6.3")),
        ("metadata/seven.json", &manifest("7.0.2")),
    ]);
    for (copy, declared) in [
        ("first", "number"),
        ("second", "string"),
        ("third", "boolean"),
    ] {
        std::fs::create_dir_all(fixture.path(copy)).unwrap();
        let copy = Fixture::existing(&fixture.path(copy));
        copy.install_compiler(&package);
        let installed = copy.path("node_modules/typescript");
        let dom = installed.join("lib/lib.dom.d.ts");
        let mut text = std::fs::read_to_string(&dom).unwrap();
        text.push_str(&format!("declare var libraryValue: {declared};\n"));
        std::fs::write(&dom, text).unwrap();
        std::fs::remove_file(installed.join("package.json")).unwrap();
        symlink(
            fixture.path("metadata/five.json"),
            installed.join("package.json"),
        )
        .unwrap();
    }
    let link = fixture.path("node_modules/typescript");
    std::fs::create_dir_all(fixture.path("node_modules")).unwrap();
    symlink("../first/node_modules/typescript", &link).unwrap();
    let config = fixture.path("tsconfig.json");
    let request = || type_request(&fixture.path("a.ts"), 1, 14);
    let mut session = ProjectSession::new(&config).unwrap();
    let mut ask = |step: &str| {
        let refreshed = summary(session.execute(request()));
        let fresh = summary(Project::load(&config).and_then(|project| project.execute(request())));
        assert_eq!(refreshed, fresh, "{step}");
        refreshed
    };
    assert_eq!(ask("linked manifest"), Ok("number".into()));
    let manifest = fixture.path("first/node_modules/typescript/package.json");
    std::fs::remove_file(&manifest).unwrap();
    symlink(fixture.path("metadata/seven.json"), &manifest).unwrap();
    // The vendored TypeScript 7.0.2 `lib.dom.d.ts` declares no
    // `libraryValue`.
    assert!(ask("manifest relinked to TypeScript 7").is_err());
    std::fs::remove_file(&manifest).unwrap();
    symlink(fixture.path("metadata/five.json"), &manifest).unwrap();
    assert_eq!(ask("manifest relinked back"), Ok("number".into()));
    std::fs::remove_file(&link).unwrap();
    symlink("../second/node_modules/typescript", &link).unwrap();
    assert_eq!(ask("package directory relinked"), Ok("string".into()));
    let lib = fixture.path("second/node_modules/typescript/lib");
    let moved = fixture.path("second-lib");
    std::fs::rename(&lib, &moved).unwrap();
    symlink(&moved, &lib).unwrap();
    assert_eq!(ask("lib linked"), Ok("string".into()));
    std::fs::remove_file(&lib).unwrap();
    symlink(fixture.path("third/node_modules/typescript/lib"), &lib).unwrap();
    assert_eq!(ask("lib relinked"), Ok("boolean".into()));
}

/// The replacement lookup evaluates a `@typescript/lib-*` package's
/// `typesVersions` with the dialect's compiler version: with `<6` mapped to
/// a declaration of `replaced: number` and `>=6` to `replaced: string`,
/// 5.6.3's quick info is `const value: number` (`tsc --listFilesOnly` lists
/// `v5/index.d.ts`), and native 7.0.2 under `libReplacement` lists
/// `v6/index.d.ts`, whose `string` the TypeScript 7 dialect answers.
#[test]
fn a_replacements_types_versions_are_read_with_the_dialects_version() {
    let Some(package) =
        support::typescript_5_6("a_replacements_types_versions_are_read_with_the_dialects_version")
    else {
        return;
    };
    let fixture = Fixture::new(&[
        ("tsconfig.json", &config(r#", "lib": ["es2017", "dom"]"#)),
        (
            "replacing.json",
            &config(r#", "lib": ["es2017", "dom"], "libReplacement": true"#),
        ),
        ("a.ts", "export const value = replaced;\n"),
        (
            "node_modules/@typescript/lib-dom/package.json",
            r#"{ "name": "@typescript/lib-dom", "version": "1.0.0", "typesVersions": { "<6": { "*": ["v5/*"] }, ">=6": { "*": ["v6/*"] } } }"#,
        ),
        (
            "node_modules/@typescript/lib-dom/index.d.ts",
            "declare const replaced: boolean;\n",
        ),
        (
            "node_modules/@typescript/lib-dom/v5/index.d.ts",
            "declare const replaced: number;\n",
        ),
        (
            "node_modules/@typescript/lib-dom/v6/index.d.ts",
            "declare const replaced: string;\n",
        ),
    ]);
    fixture.install_compiler(&package);
    let answer = |config: &str| {
        let project = Project::load(&fixture.path(config)).unwrap();
        summary(project.execute(type_request(&fixture.path("a.ts"), 1, 14)))
    };
    assert_eq!(answer("tsconfig.json"), Ok("number".into()));
    fixture.claim_compiler("7.0.2");
    assert_eq!(answer("replacing.json"), Ok("string".into()));
}

/// Overload resolution tries the declarations of a later file first
/// (`reorderCandidates`), so program order decides which global `pick`
/// answers `pick("x")`. 5.6.3's `createProgram` processes the type packages
/// before the libraries, so what a replacement library reaches by an import
/// or a `/// <reference types>` follows the type package and answers
/// `"lib"`, where native 7.0.2, which adds the library tasks first, answers
/// `"types"`. A file a library reaches by `/// <reference path>` is ordered
/// with the libraries under 5.6.3 (`processReferencedFiles(file,
/// isDefaultLib)`), before every type package, so both answer `"types"`, as
/// for a declaration in the replacement itself. Between two replacements,
/// one a type package references comes first under 5.6.3, so `lib-dom`'s
/// later `pick` answers, where 7.0.2 answers `lib-scripthost`'s. Expected
/// answers are 5.6.3's quick info and the type native 7.0.2 reports for
/// `picked` under `libReplacement`.
#[test]
fn overloads_follow_each_compilers_program_order() {
    let Some(package) = support::typescript_5_6("overloads_follow_each_compilers_program_order")
    else {
        return;
    };
    let config = |types: &str, lib: &str, replacing: bool| {
        let replacing = if replacing {
            r#", "libReplacement": true"#
        } else {
            ""
        };
        format!(
            r#"{{ "compilerOptions": {{ "types": ["{types}"], "strict": true, "target": "ES2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true, "lib": [{lib}]{replacing} }}, "files": ["a.ts"] }}"#
        )
    };
    let dom = r#""es5", "dom""#;
    let both = r#""es5", "dom", "scripthost""#;
    let fixture = Fixture::new(&[
        ("pick.json", &config("picker", dom, false)),
        ("pick-replacing.json", &config("picker", dom, true)),
        ("tie.json", &config("tie", both, false)),
        ("tie-replacing.json", &config("tie", both, true)),
        ("a.ts", "export const picked = pick(\"x\");\n"),
        (
            "node_modules/picker/package.json",
            r#"{ "name": "picker", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/picker/index.d.ts",
            "declare function pick(value: string): \"types\";\n",
        ),
        (
            "node_modules/pickref/package.json",
            r#"{ "name": "pickref", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/pickref/index.d.ts",
            "declare function pick(value: string): \"lib\";\n",
        ),
        (
            "node_modules/tie/package.json",
            r#"{ "name": "tie", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/tie/index.d.ts",
            "/// <reference lib=\"scripthost\" />\n",
        ),
        (
            "node_modules/@typescript/lib-dom/package.json",
            r#"{ "name": "@typescript/lib-dom", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/@typescript/lib-dom/pick.d.ts",
            "declare function pick(value: string): \"lib\";\n",
        ),
        (
            "node_modules/@typescript/lib-scripthost/package.json",
            r#"{ "name": "@typescript/lib-scripthost", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/@typescript/lib-scripthost/index.d.ts",
            "declare function pick(value: string): \"scripthost\";\n",
        ),
    ]);
    fixture.install_compiler(&package);
    let replacement = fixture.path("node_modules/@typescript/lib-dom/index.d.ts");
    for (variant, config, text, typescript_5_6, typescript_7) in [
        (
            "import",
            "pick",
            "import \"./pick\";\n",
            "\"lib\"",
            "\"types\"",
        ),
        (
            "reference types",
            "pick",
            "/// <reference types=\"pickref\" />\n",
            "\"lib\"",
            "\"types\"",
        ),
        (
            "reference path",
            "pick",
            "/// <reference path=\"./pick.d.ts\" />\n",
            "\"types\"",
            "\"types\"",
        ),
        (
            "declared",
            "pick",
            "declare function pick(value: string): \"lib\";\n",
            "\"types\"",
            "\"types\"",
        ),
        (
            "two replacements",
            "tie",
            "declare function pick(value: string): \"dom\";\n",
            "\"dom\"",
            "\"scripthost\"",
        ),
    ] {
        std::fs::write(&replacement, text).unwrap();
        let answer = |config: &str| {
            let project = Project::load(&fixture.path(config)).unwrap();
            summary(project.execute(type_request(&fixture.path("a.ts"), 1, 14)))
        };
        fixture.claim_compiler("5.6.3");
        assert_eq!(
            answer(&format!("{config}.json")),
            Ok(typescript_5_6.into()),
            "5.6.3 {variant}"
        );
        fixture.claim_compiler("7.0.2");
        assert_eq!(
            answer(&format!("{config}-replacing.json")),
            Ok(typescript_7.into()),
            "7.0.2 {variant}"
        );
    }
}

/// One program-order scenario of
/// `overlapping_routes_and_library_locations_order_as_each_compiler_does`.
struct Scenario {
    name: &'static str,
    /// The `types` and `lib` options.
    types: &'static str,
    lib: &'static str,
    files: &'static [(&'static str, &'static str)],
    /// Text appended to files of the installed compiler's `lib`.
    appended: &'static [(&'static str, &'static str)],
    /// 5.6.3's quick info for `picked`, and the type native 7.0.2 reports
    /// for it under `libReplacement`.
    typescript_5_6: &'static str,
    typescript_7: &'static str,
}

const PICK_TYPES: &str = "declare function pick(value: string): \"types\";\n";
const PICK_LIB: &str = "declare function pick(value: string): \"lib\";\n";
const PICK_SHARED: &str = "declare function pick(value: string): \"shared\";\n";
const PICKER: &str = r#"{ "name": "picker", "types": "index.d.ts" }"#;
const LIB_DOM: &str = r#"{ "name": "@typescript/lib-dom", "types": "index.d.ts" }"#;
const LIB_SCRIPTHOST: &str = r#"{ "name": "@typescript/lib-scripthost", "types": "index.d.ts" }"#;

/// Where two routes reach one file, 5.6.3 orders it by the route that
/// processes it first, depth first with the type packages before the
/// libraries: a replacement a type package imports or names by `/// <reference
/// path>` before its library task runs is an ordinary file, after the type
/// package's other files, so its `pick` answers (`"lib"`), where 7.0.2, which
/// takes the library task it loads first, orders it with the libraries
/// (`"types"`). A file a library reaches first by `/// <reference path>` stays
/// with the libraries though another library imports it (`"types"`), and one
/// a library imports first stays ordinary though another references its path
/// (`"shared"`). The libraries are then ordered by each file's own path
/// (`getDefaultLibFilePriority`): anywhere beneath the compiler's `lib`
/// directory, by its file name, so a replacement that resolves to the
/// installed `lib.dom.d.ts` precedes `lib.scripthost.d.ts` (`"scripthost"`)
/// and a file a replacement references in `lib/nested` is ordered as
/// `lib.es2015.core.d.ts`, before the `lib-dom` replacement (`"dom"`); under
/// 7.0.2 neither lies beneath its own directory. A reference path is
/// normalized before its extension is found (`resolveTripleslashReference`,
/// then `getSourceFileFromReferenceWorker`), so `es5/child/..`, `es5/.`,
/// `es5` and `es5.d.ts` all name `lib/es5.d.ts`, which `getDefaultLibFilePriority`
/// orders as `es5`, before the `lib-dom` replacement (`"dom"`). Expected answers are 5.6.3's
/// quick info and native 7.0.2's reported type for `picked`; the location
/// scenarios need the installed copy's libraries (its `tsc.js`, or
/// `tools/ts56-oracle.mjs` with `TYPE_SCRIPT_56_API` naming the fixture's
/// `node_modules/typescript`), not the pinned package's own.
#[test]
fn overlapping_routes_and_library_locations_order_as_each_compiler_does() {
    let Some(package) = support::typescript_5_6(
        "overlapping_routes_and_library_locations_order_as_each_compiler_does",
    ) else {
        return;
    };
    let scenarios = [
        Scenario {
            name: "a type package imports the replacement",
            types: "picker",
            lib: r#""es5", "dom""#,
            files: &[
                ("node_modules/picker/package.json", PICKER),
                (
                    "node_modules/picker/index.d.ts",
                    "import \"./other\";\nimport \"@typescript/lib-dom\";\n",
                ),
                ("node_modules/picker/other.d.ts", PICK_TYPES),
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                ("node_modules/@typescript/lib-dom/index.d.ts", PICK_LIB),
            ],
            appended: &[],
            typescript_5_6: "\"lib\"",
            typescript_7: "\"types\"",
        },
        Scenario {
            name: "a type package references the replacement's path",
            types: "picker",
            lib: r#""es5", "dom""#,
            files: &[
                ("node_modules/picker/package.json", PICKER),
                (
                    "node_modules/picker/index.d.ts",
                    "/// <reference path=\"./other.d.ts\" />\n/// <reference path=\"../@typescript/lib-dom/index.d.ts\" />\n",
                ),
                ("node_modules/picker/other.d.ts", PICK_TYPES),
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                ("node_modules/@typescript/lib-dom/index.d.ts", PICK_LIB),
            ],
            appended: &[],
            typescript_5_6: "\"lib\"",
            typescript_7: "\"types\"",
        },
        Scenario {
            name: "a library references a path another library imports",
            types: "picker",
            lib: r#""es5", "dom", "scripthost""#,
            files: &[
                ("node_modules/picker/package.json", PICKER),
                ("node_modules/picker/index.d.ts", PICK_TYPES),
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                (
                    "node_modules/@typescript/lib-dom/index.d.ts",
                    "/// <reference path=\"./shared.d.ts\" />\n",
                ),
                ("node_modules/@typescript/lib-dom/shared.d.ts", PICK_SHARED),
                (
                    "node_modules/@typescript/lib-scripthost/package.json",
                    LIB_SCRIPTHOST,
                ),
                (
                    "node_modules/@typescript/lib-scripthost/index.d.ts",
                    "import \"../lib-dom/shared\";\n",
                ),
            ],
            appended: &[],
            typescript_5_6: "\"types\"",
            typescript_7: "\"types\"",
        },
        Scenario {
            name: "a library imports a file another library references by path",
            types: "picker",
            lib: r#""es5", "dom", "scripthost""#,
            files: &[
                ("node_modules/picker/package.json", PICKER),
                ("node_modules/picker/index.d.ts", PICK_TYPES),
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                (
                    "node_modules/@typescript/lib-dom/index.d.ts",
                    "import \"./shared\";\n",
                ),
                ("node_modules/@typescript/lib-dom/shared.d.ts", PICK_SHARED),
                (
                    "node_modules/@typescript/lib-scripthost/package.json",
                    LIB_SCRIPTHOST,
                ),
                (
                    "node_modules/@typescript/lib-scripthost/index.d.ts",
                    "/// <reference path=\"../lib-dom/shared.d.ts\" />\n",
                ),
            ],
            appended: &[],
            typescript_5_6: "\"shared\"",
            typescript_7: "\"types\"",
        },
        Scenario {
            name: "a replacement resolves to the installed library",
            types: "",
            lib: r#""es5", "scripthost", "dom""#,
            files: &[(
                "node_modules/@typescript/lib-dom/package.json",
                r#"{ "name": "@typescript/lib-dom", "types": "../../typescript/lib/lib.dom.d.ts" }"#,
            )],
            appended: &[
                (
                    "lib.dom.d.ts",
                    "declare function pick(value: string): \"dom\";\n",
                ),
                (
                    "lib.scripthost.d.ts",
                    "declare function pick(value: string): \"scripthost\";\n",
                ),
            ],
            typescript_5_6: "\"scripthost\"",
            typescript_7: "\"dom\"",
        },
        Scenario {
            name: "a replacement references a path beneath the installed lib",
            types: "",
            lib: r#""es5", "dom", "scripthost""#,
            files: &[
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                (
                    "node_modules/@typescript/lib-dom/index.d.ts",
                    "declare function pick(value: string): \"dom\";\n",
                ),
                (
                    "node_modules/@typescript/lib-scripthost/package.json",
                    LIB_SCRIPTHOST,
                ),
                (
                    "node_modules/@typescript/lib-scripthost/index.d.ts",
                    "/// <reference path=\"../../typescript/lib/nested/lib.es2015.core.d.ts\" />\n",
                ),
                (
                    "node_modules/typescript/lib/nested/lib.es2015.core.d.ts",
                    "declare function pick(value: string): \"core\";\n",
                ),
            ],
            appended: &[],
            typescript_5_6: "\"dom\"",
            typescript_7: "\"core\"",
        },
        Scenario {
            name: "a reference path climbs back to a file in the installed lib",
            types: "",
            lib: r#""es5", "dom", "scripthost""#,
            files: &[
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                (
                    "node_modules/@typescript/lib-dom/index.d.ts",
                    "declare function pick(value: string): \"dom\";\n",
                ),
                (
                    "node_modules/@typescript/lib-scripthost/package.json",
                    LIB_SCRIPTHOST,
                ),
                (
                    "node_modules/@typescript/lib-scripthost/index.d.ts",
                    "/// <reference path=\"../../typescript/lib/es5/child/..\" />\n",
                ),
                (
                    "node_modules/typescript/lib/es5.d.ts",
                    "declare function pick(value: string): \"core\";\n",
                ),
            ],
            appended: &[],
            typescript_5_6: "\"dom\"",
            typescript_7: "\"core\"",
        },
        Scenario {
            name: "a reference path ends in the current directory",
            types: "",
            lib: r#""es5", "dom", "scripthost""#,
            files: &[
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                (
                    "node_modules/@typescript/lib-dom/index.d.ts",
                    "declare function pick(value: string): \"dom\";\n",
                ),
                (
                    "node_modules/@typescript/lib-scripthost/package.json",
                    LIB_SCRIPTHOST,
                ),
                (
                    "node_modules/@typescript/lib-scripthost/index.d.ts",
                    "/// <reference path=\"../../typescript/lib/es5/.\" />\n",
                ),
                (
                    "node_modules/typescript/lib/es5.d.ts",
                    "declare function pick(value: string): \"core\";\n",
                ),
            ],
            appended: &[],
            typescript_5_6: "\"dom\"",
            typescript_7: "\"core\"",
        },
        Scenario {
            name: "a reference path omits the extension",
            types: "",
            lib: r#""es5", "dom", "scripthost""#,
            files: &[
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                (
                    "node_modules/@typescript/lib-dom/index.d.ts",
                    "declare function pick(value: string): \"dom\";\n",
                ),
                (
                    "node_modules/@typescript/lib-scripthost/package.json",
                    LIB_SCRIPTHOST,
                ),
                (
                    "node_modules/@typescript/lib-scripthost/index.d.ts",
                    "/// <reference path=\"../../typescript/lib/es5\" />\n",
                ),
                (
                    "node_modules/typescript/lib/es5.d.ts",
                    "declare function pick(value: string): \"core\";\n",
                ),
            ],
            appended: &[],
            typescript_5_6: "\"dom\"",
            typescript_7: "\"core\"",
        },
        Scenario {
            name: "a reference path writes the extension",
            types: "",
            lib: r#""es5", "dom", "scripthost""#,
            files: &[
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                (
                    "node_modules/@typescript/lib-dom/index.d.ts",
                    "declare function pick(value: string): \"dom\";\n",
                ),
                (
                    "node_modules/@typescript/lib-scripthost/package.json",
                    LIB_SCRIPTHOST,
                ),
                (
                    "node_modules/@typescript/lib-scripthost/index.d.ts",
                    "/// <reference path=\"../../typescript/lib/es5.d.ts\" />\n",
                ),
                (
                    "node_modules/typescript/lib/es5.d.ts",
                    "declare function pick(value: string): \"core\";\n",
                ),
            ],
            appended: &[],
            typescript_5_6: "\"dom\"",
            typescript_7: "\"core\"",
        },
    ];
    for scenario in scenarios {
        let name = scenario.name;
        let types = if scenario.types.is_empty() {
            String::new()
        } else {
            format!(r#""{}""#, scenario.types)
        };
        let options = format!(
            r#""types": [{types}], "strict": true, "target": "ES2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true, "lib": [{}]"#,
            scenario.lib
        );
        let five = format!(r#"{{ "compilerOptions": {{ {options} }}, "files": ["a.ts"] }}"#);
        let seven = format!(
            r#"{{ "compilerOptions": {{ {options}, "libReplacement": true }}, "files": ["a.ts"] }}"#
        );
        let mut files = vec![
            ("five.json", five.as_str()),
            ("seven.json", seven.as_str()),
            ("a.ts", "export const picked = pick(\"x\");\n"),
        ];
        files.extend_from_slice(scenario.files);
        let fixture = Fixture::new(&files);
        fixture.install_compiler(&package);
        for (file, text) in scenario.appended {
            let path = fixture.path(&format!("node_modules/typescript/lib/{file}"));
            let mut installed = std::fs::read_to_string(&path).unwrap();
            installed.push_str(text);
            std::fs::write(&path, installed).unwrap();
        }
        let answer = |config: &str| {
            let project = Project::load(&fixture.path(config))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            summary(project.execute(type_request(&fixture.path("a.ts"), 1, 14)))
        };
        assert_eq!(
            answer("five.json"),
            Ok(scenario.typescript_5_6.into()),
            "5.6.3: {name}"
        );
        fixture.claim_compiler("7.0.2");
        assert_eq!(
            answer("seven.json"),
            Ok(scenario.typescript_7.into()),
            "7.0.2: {name}"
        );
    }
}

/// 5.6.3 orders a library by the name its first route gives the file
/// against the real directory of the compiler that runs, the `lib` beside
/// its `tsc.js` through every link (`getDefaultLibLocation`). A reference
/// path keeps the spelling it writes, so with `node_modules/typescript`
/// linked to `store/typescript`, or its `lib` linked to `libstore`, a file
/// a replacement references through `node_modules/typescript/lib/nested`
/// lies outside that directory and is ordered last, after the `lib-dom`
/// replacement (`"core"`). A replacement resolves to a real path, so one
/// whose `types` names `../../typescript/lib/lib.dom.d.ts` lies inside it
/// under either link and precedes `lib.scripthost.d.ts` by name
/// (`"scripthost"`). A reference in a file named through the link resolves
/// from that name, so a second reference path stays outside too
/// (`"core"`). Expected answers are 5.6.3's `tsc` and its quick
/// info, run from the linked installed copy (`tools/ts56-oracle.mjs` with
/// `TYPE_SCRIPT_56_API` naming the project's `node_modules/typescript`).
#[test]
fn a_linked_compiler_orders_libraries_by_the_names_5_6_3_processes() {
    let Some(package) =
        support::typescript_5_6("a_linked_compiler_orders_libraries_by_the_names_5_6_3_processes")
    else {
        return;
    };
    use std::os::unix::fs::symlink;
    const PATH_FILES: &[(&str, &str)] = &[
        ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
        (
            "node_modules/@typescript/lib-dom/index.d.ts",
            "declare function pick(value: string): \"dom\";\n",
        ),
        (
            "node_modules/@typescript/lib-scripthost/package.json",
            LIB_SCRIPTHOST,
        ),
        (
            "node_modules/@typescript/lib-scripthost/index.d.ts",
            "/// <reference path=\"../../typescript/lib/nested/lib.es2015.core.d.ts\" />\n",
        ),
    ];
    const CHAIN_FILES: &[(&str, &str)] = &[
        ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
        (
            "node_modules/@typescript/lib-dom/index.d.ts",
            "declare function pick(value: string): \"dom\";\n",
        ),
        (
            "node_modules/@typescript/lib-scripthost/package.json",
            LIB_SCRIPTHOST,
        ),
        (
            "node_modules/@typescript/lib-scripthost/index.d.ts",
            "/// <reference path=\"../../typescript/lib/x.d.ts\" />\n",
        ),
    ];
    // Files added to the installed `lib`.
    const CORE: &[(&str, &str)] = &[(
        "nested/lib.es2015.core.d.ts",
        "declare function pick(value: string): \"core\";\n",
    )];
    const CHAIN: &[(&str, &str)] = &[
        (
            "x.d.ts",
            "/// <reference path=\"./nested/lib.es2015.core.d.ts\" />\n",
        ),
        (
            "nested/lib.es2015.core.d.ts",
            "declare function pick(value: string): \"core\";\n",
        ),
    ];
    const INTO_FILES: &[(&str, &str)] = &[(
        "node_modules/@typescript/lib-dom/package.json",
        r#"{ "name": "@typescript/lib-dom", "types": "../../typescript/lib/lib.dom.d.ts" }"#,
    )];
    for (name, link, lib, files, added, expected) in [
        (
            "a linked package references a path through its link",
            "package",
            r#""es5", "dom", "scripthost""#,
            PATH_FILES,
            CORE,
            "\"core\"",
        ),
        (
            "a linked package's reference path names one that names another",
            "package",
            r#""es5", "dom", "scripthost""#,
            CHAIN_FILES,
            CHAIN,
            "\"core\"",
        ),
        (
            "a linked package's replacement resolves into its lib",
            "package",
            r#""es5", "scripthost", "dom""#,
            INTO_FILES,
            &[],
            "\"scripthost\"",
        ),
        (
            "a linked lib references a path through its link",
            "lib",
            r#""es5", "dom", "scripthost""#,
            PATH_FILES,
            CORE,
            "\"core\"",
        ),
        (
            "a linked lib's replacement resolves into it",
            "lib",
            r#""es5", "scripthost", "dom""#,
            INTO_FILES,
            &[],
            "\"scripthost\"",
        ),
    ] {
        let config = format!(
            r#"{{ "compilerOptions": {{ "types": [], "strict": true, "target": "ES2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true, "lib": [{lib}] }}, "files": ["a.ts"] }}"#
        );
        let mut all = vec![
            ("tsconfig.json", config.as_str()),
            ("a.ts", "export const picked = pick(\"x\");\n"),
        ];
        all.extend_from_slice(files);
        let fixture = Fixture::new(&all);
        // Install the compiler in `store`, then link the package directory
        // or its `lib` into place.
        let store = fixture.path("store");
        std::fs::create_dir_all(&store).unwrap();
        Fixture::existing(&store).install_compiler(&package);
        let installed = store.join("node_modules/typescript");
        let library = if link == "package" {
            std::fs::create_dir_all(fixture.path("node_modules")).unwrap();
            symlink(&installed, fixture.path("node_modules/typescript")).unwrap();
            installed.join("lib")
        } else {
            let package = fixture.path("node_modules/typescript");
            std::fs::create_dir_all(&package).unwrap();
            std::fs::copy(installed.join("package.json"), package.join("package.json")).unwrap();
            let libstore = fixture.path("libstore");
            std::fs::rename(installed.join("lib"), &libstore).unwrap();
            symlink(&libstore, package.join("lib")).unwrap();
            libstore
        };
        for (file, text) in added {
            let path = library.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        if files == INTO_FILES {
            for (file, returned) in [
                ("lib.dom.d.ts", "dom"),
                ("lib.scripthost.d.ts", "scripthost"),
            ] {
                let path = library.join(file);
                let mut text = std::fs::read_to_string(&path).unwrap();
                text.push_str(&format!(
                    "declare function pick(value: string): \"{returned}\";\n"
                ));
                std::fs::write(&path, text).unwrap();
            }
        }
        let project = Project::load(&fixture.path("tsconfig.json"))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            summary(project.execute(type_request(&fixture.path("a.ts"), 1, 14))),
            Ok(expected.into()),
            "{name}"
        );
    }
}

/// Under TypeScript 7 a replacement is ordered last wherever it resolves,
/// even into this build's vendored declarations, which stand in for
/// 7.0.2's own directory and are not that directory: with `lib-dom`
/// resolving to the vendored `lib.dom.d.ts` and `lib-scripthost` declaring
/// `alert(message?: any): "replacement"`, both are last in `lib` order,
/// and DOM's later `alert` answers, as native 7.0.2 reports `void` for
/// `picked` (`tsc --listFilesOnly` lists the scripthost replacement before
/// the vendored `lib.dom.d.ts`).
#[test]
fn a_typescript_7_replacement_is_ordered_last_wherever_it_resolves() {
    let vendored = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/typescript-7.0.2-lib");
    let manifest = format!(
        r#"{{ "name": "@typescript/lib-dom", "types": {} }}"#,
        serde_json::to_string(&vendored.join("lib.dom.d.ts")).unwrap()
    );
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            &config(r#", "lib": ["es5", "scripthost", "dom"], "libReplacement": true"#),
        ),
        ("a.ts", "export const picked = alert(\"x\");\n"),
        ("node_modules/@typescript/lib-dom/package.json", &manifest),
        (
            "node_modules/@typescript/lib-scripthost/package.json",
            LIB_SCRIPTHOST,
        ),
        (
            "node_modules/@typescript/lib-scripthost/index.d.ts",
            "declare function alert(message?: any): \"replacement\";\n",
        ),
    ]);
    let project = Project::load(&fixture.path("tsconfig.json")).unwrap();
    assert_eq!(
        summary(project.execute(type_request(&fixture.path("a.ts"), 1, 14))),
        Ok("void".into())
    );
}

/// Run from an unrelated directory that has its own `@typescript/lib-dom`,
/// the tool reads the package beside the configuration, as both compilers
/// do from the configuration's directory: always under the TypeScript 5.6
/// dialect, and only with `libReplacement` under TypeScript 7.
#[test]
fn replacements_are_found_beside_the_configuration_from_any_directory() {
    let Some(package) = support::typescript_5_6(
        "replacements_are_found_beside_the_configuration_from_any_directory",
    ) else {
        return;
    };
    let fixture = Fixture::new(&[
        (
            "project/tsconfig.json",
            &config(r#", "lib": ["es5", "dom"]"#),
        ),
        (
            "project/replacing.json",
            &config(r#", "lib": ["es5", "dom"], "libReplacement": true"#),
        ),
        ("project/a.ts", "export const value = replaced;\n"),
        (
            "project/node_modules/@typescript/lib-dom/package.json",
            r#"{ "name": "@typescript/lib-dom", "types": "index.d.ts" }"#,
        ),
        (
            "project/node_modules/@typescript/lib-dom/index.d.ts",
            "declare var replaced: number;\n",
        ),
        (
            "unrelated/node_modules/@typescript/lib-dom/package.json",
            r#"{ "name": "@typescript/lib-dom", "types": "index.d.ts" }"#,
        ),
        (
            "unrelated/node_modules/@typescript/lib-dom/index.d.ts",
            "declare var replaced: string;\n",
        ),
    ]);
    let project = Fixture::existing(&fixture.path("project"));
    project.install_compiler(&package);
    let at = format!("{}:1:14", fixture.path("project/a.ts").display());
    let answer = |config: &str| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .current_dir(fixture.path("unrelated"))
            .args(["--project"])
            .arg(fixture.path(&format!("project/{config}")))
            .args(["--type", "--at", &at, "--direct"])
            .output()
            .unwrap();
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .last()
            .map(str::to_owned)
    };
    assert_eq!(answer("tsconfig.json").as_deref(), Some("number"));
    project.claim_compiler("7.0.2");
    assert_eq!(answer("replacing.json").as_deref(), Some("number"));
    // Without `libReplacement`, the vendored `lib.dom.d.ts` declares no
    // `replaced`.
    assert_eq!(answer("tsconfig.json"), None);
}
