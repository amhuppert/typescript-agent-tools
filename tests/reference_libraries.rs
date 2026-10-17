//! Standard libraries in reference results. Expectations were observed with
//! the pinned TypeScript 7.0.2 compiler: declarations through the semantic
//! API (`tools/semantic-oracle.mjs`), errors with `tsc --noEmit`.
mod support;

use std::path::Path;
use support::Fixture;
use typescript_parser::analysis::{InputErrorKind, QueryErrorKind};
use typescript_parser::project::{Project, ProjectSession};
use typescript_parser::references::ReferenceQuery;
use typescript_parser::source::SourcePosition;

fn at(fixture: &Fixture, file: &str, line: usize, column: usize) -> ReferenceQuery {
    ReferenceQuery::At {
        file: fixture.path(file),
        position: SourcePosition { line, column },
    }
}

/// `file:line:column` of each declaration of each group, sorted, where a
/// vendored library is named `lib:` and its file name.
fn declarations(fixture: &Fixture, project: &Project, query: ReferenceQuery) -> Vec<Vec<String>> {
    let vendored = std::fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/typescript-7.0.2-lib"),
    )
    .unwrap();
    let result = project.find_references(query).unwrap();
    result
        .groups
        .iter()
        .map(|group| {
            let mut declarations = group
                .declarations
                .iter()
                .map(|declaration| {
                    let location = &declaration.location;
                    let path = std::fs::canonicalize(fixture.root.join(&location.file)).unwrap();
                    let file = match path.strip_prefix(&vendored) {
                        Ok(name) => format!("lib:{}", name.display()),
                        Err(_) => location.file.clone(),
                    };
                    format!("{file}:{}:{}", location.start.line, location.start.column)
                })
                .collect::<Vec<_>>();
            declarations.sort();
            declarations
        })
        .collect()
}

#[test]
fn library_globals_and_their_augmentations_form_one_group() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", r#"{ "compilerOptions": {} }"#),
        (
            "augment.ts",
            "declare global {\n  interface Array<T> { last(): T | undefined; }\n}\nexport const list: Array<number> = [];\nexport const pending = Promise.resolve(list);\n",
        ),
    ]);
    let project = fixture.load();
    let array = [
        "augment.ts:2:13",
        "lib:lib.es2015.core.d.ts:17:11",
        "lib:lib.es2015.iterable.d.ts:74:11",
        "lib:lib.es2015.symbol.wellknown.d.ts:90:11",
        "lib:lib.es2016.array.include.d.ts:17:11",
        "lib:lib.es2019.array.d.ts:51:11",
        "lib:lib.es2022.array.d.ts:17:11",
        "lib:lib.es2023.array.d.ts:17:11",
        "lib:lib.es5.d.ts:1323:11",
    ];
    // The augmentation, and a type use, select the same group.
    for (line, column) in [(2, 13), (4, 20)] {
        assert_eq!(
            declarations(&fixture, &project, at(&fixture, "augment.ts", line, column)),
            [array]
        );
    }
    // `Promise` the value is only `declare var Promise`.
    assert_eq!(
        declarations(&fixture, &project, at(&fixture, "augment.ts", 5, 24)),
        [["lib:lib.es2015.promise.d.ts:79:13"]]
    );
}

#[test]
fn library_files_are_not_searched() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "lib": ["es5"] } }"#,
        ),
        ("a.ts", "export const values: Array<number> = [];\n"),
    ]);
    let project = fixture.load();
    let result = project
        .find_references(at(&fixture, "a.ts", 1, 22))
        .unwrap();
    // Occurrences come from searched files only; declarations do not.
    assert_eq!(result.groups.len(), 1);
    let files: Vec<&str> = result.groups[0]
        .occurrences
        .iter()
        .map(|occurrence| occurrence.location.file.as_str())
        .collect();
    assert_eq!(files, ["a.ts"]);
    let library =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/typescript-7.0.2-lib/lib.es5.d.ts");
    let error = support::query_error(project.find_references(ReferenceQuery::At {
        file: library,
        position: SourcePosition {
            line: 1323,
            column: 11,
        },
    }));
    assert_eq!(error.kind, QueryErrorKind::OutsideProject);
    // Name queries select declarations in searched files only.
    let named = project
        .find_references(ReferenceQuery::Name {
            name: "Array".into(),
            file: None,
        })
        .unwrap();
    assert!(named.groups.is_empty());
}

#[test]
fn project_declarations_that_conflict_with_library_globals_fail_where_they_are_written() {
    // TypeScript reports TS2451, TS2300, and TS2428 in a.ts, and also in
    // lib.es5.d.ts unless skipLibCheck is set; libraries bind first.
    for (source, options, column) in [
        ("let Array = 1;\n", "", 5),
        ("let Array = 1;\n", r#", "skipLibCheck": true"#, 5),
        ("class Array {}\n", "", 7),
        (
            "interface Array<U> { x: U }\n",
            r#", "skipLibCheck": true"#,
            11,
        ),
    ] {
        let fixture = Fixture::new(&[
            (
                "tsconfig.json",
                &format!(r#"{{ "compilerOptions": {{ "lib": ["es5"]{options} }} }}"#),
            ),
            ("a.ts", source),
        ]);
        let error = Project::load(&fixture.path("tsconfig.json"))
            .map_err(support::input_error)
            .unwrap_err();
        assert_eq!(
            (error.kind, error.position, error.path.as_deref()),
            (
                InputErrorKind::Binding,
                Some(SourcePosition { line: 1, column }),
                Some(fixture.path("a.ts").as_path())
            ),
            "{source}"
        );
    }
    // Valid merges with library declarations load.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "lib": ["es5"] } }"#,
        ),
        (
            "a.ts",
            "var NaN: number;\nnamespace Intl { export const extra = 1; }\nNaN;\nIntl.extra;\n",
        ),
    ]);
    let project = fixture.load();
    assert_eq!(
        declarations(&fixture, &project, at(&fixture, "a.ts", 3, 1)),
        [["a.ts:1:5", "lib:lib.es5.d.ts:24:13"]]
    );
    assert_eq!(
        declarations(&fixture, &project, at(&fixture, "a.ts", 4, 1)),
        [["a.ts:2:11", "lib:lib.es5.d.ts:4416:19"]]
    );
}

#[test]
fn replacement_library_edits_and_installation_refresh_a_session() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "libReplacement": true, "lib": ["es5", "dom"] }, "files": ["a.ts"] }"#,
        ),
        ("a.ts", "replaced;\n"),
    ]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    let query = || at(&fixture, "a.ts", 1, 1);
    let declaration = |session: &mut ProjectSession| {
        support::references(session, query())
            .map(|result| result.groups[0].declarations[0].location.clone())
    };
    // Without the package, the vendored `lib.dom.d.ts` declares no `replaced`.
    assert!(declaration(&mut session).is_err());
    let package = fixture.path("node_modules/@typescript/lib-dom");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("package.json"), r#"{ "types": "index.d.ts" }"#).unwrap();
    std::fs::write(
        package.join("index.d.ts"),
        "declare var replaced: number;\n",
    )
    .unwrap();
    let installed = declaration(&mut session).unwrap();
    assert_eq!(
        installed.file,
        "node_modules/@typescript/lib-dom/index.d.ts"
    );
    assert_eq!(installed.start.line, 1);
    std::fs::write(
        package.join("index.d.ts"),
        "\ndeclare var replaced: number;\n",
    )
    .unwrap();
    assert_eq!(declaration(&mut session).unwrap().start.line, 2);
    // An unchanged project reuses its generation.
    assert_eq!(declaration(&mut session).unwrap().start.line, 2);
    assert_eq!(session.generation(), 3);
}

/// Whether the occurrences of the group at `file:line:column` include one in
/// a vendored library file: whether that library file is searched.
fn library_searched(
    fixture: &Fixture,
    project: &Project,
    file: &str,
    line: usize,
    column: usize,
) -> bool {
    let vendored = std::fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/typescript-7.0.2-lib"),
    )
    .unwrap();
    let result = project
        .find_references(at(fixture, file, line, column))
        .unwrap();
    result.groups.iter().any(|group| {
        group.occurrences.iter().any(|occurrence| {
            std::fs::canonicalize(fixture.root.join(&occurrence.location.file))
                .unwrap()
                .starts_with(&vendored)
        })
    })
}

#[test]
fn a_file_reached_as_a_library_and_by_path_takes_the_first_routes_provenance() {
    // TypeScript's loader classifies a file by the task it starts first:
    // routes nearer the roots start first. Here one root references
    // `es2015.promise` as a library and another reaches the same file
    // through an import and a `/// <reference path>`, or the other way
    // round. The pinned compiler lists the library first (a library) in the
    // first arrangement and among the project files in the second, in each
    // of 20 runs. A library is never searched; a project file is.
    let library = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/typescript-7.0.2-lib/lib.es2015.promise.d.ts");
    let path = format!(
        "/// <reference path=\"{}\" />\nexport {{}};\n",
        library.display()
    );
    let lib = "/// <reference lib=\"es2015.promise\" />\nexport {};\n";
    for (case, deep, shallow, searched) in [
        ("library route first", path.as_str(), lib, false),
        ("path route first", lib, path.as_str(), true),
    ] {
        let fixture = Fixture::new(&[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "lib": [] }, "files": ["x.ts", "a.ts", "b.ts"] }"#,
            ),
            ("x.ts", "export declare const p: PromiseConstructor;\n"),
            ("a.ts", "import \"./c\";\nexport {};\n"),
            ("c.ts", deep),
            ("b.ts", shallow),
        ]);
        let project = fixture.load();
        assert_eq!(
            library_searched(&fixture, &project, "x.ts", 1, 25),
            searched,
            "{case}"
        );
    }
}

#[cfg(unix)]
#[test]
fn replacement_libraries_are_looked_up_from_the_configurations_real_directory() {
    // A documented difference: TypeScript looks for `@typescript/lib-*` from
    // the configuration's directory as its path spells it, here through a
    // symbolic link, and finds the replacement beside the link; this tool
    // looks from the real directory and finds none, so `replacedPromise` is
    // unresolved. `tools/semantic-oracle.test.mjs` checks the compiler side.
    let fixture = Fixture::new(&[
        (
            "real/tsconfig.json",
            r#"{ "compilerOptions": { "lib": ["es5", "es2015.promise"], "libReplacement": true }, "files": ["a.ts"] }"#,
        ),
        ("real/a.ts", "export const n: number = replacedPromise;\n"),
        (
            "spelled/node_modules/@typescript/lib-es2015/package.json",
            r#"{ "name": "@typescript/lib-es2015" }"#,
        ),
        (
            "spelled/node_modules/@typescript/lib-es2015/promise.d.ts",
            "declare var replacedPromise: number;\n",
        ),
    ]);
    std::os::unix::fs::symlink("../real", fixture.path("spelled/link")).unwrap();
    let project = Project::load(&fixture.path("spelled/link/tsconfig.json")).unwrap();
    let error = support::query_error(project.find_references(ReferenceQuery::At {
        file: fixture.path("real/a.ts"),
        position: SourcePosition {
            line: 1,
            column: 26,
        },
    }));
    assert_eq!(error.kind, QueryErrorKind::UnresolvedTarget, "{error}");
}
