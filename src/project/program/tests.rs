//! Loader behavior over temporary project directories. Each expectation was
//! observed with the pinned TypeScript 7.0.2 compiler (`tsc --listFilesOnly`,
//! `--traceResolution`, and the API's `isSourceFileFromExternalLibrary`).
//!
//! Supported configuration surface:
//!
//! - `extends` (string or array; relative, rooted, or package names through
//!   `node_modules`, a manifest `tsconfig` field, `exports`, or a package's
//!   `tsconfig.json`), `${configDir}`, `files`, `include`, `exclude`.
//!   `references` must be absent or empty.
//! - `module` (`commonjs`, `es2015`…`esnext`, `node16`, `node18`, `node20`,
//!   `nodenext`, `preserve`), `moduleResolution` (`node16`, `nodenext`,
//!   `bundler`), `moduleDetection`, `jsx`, `jsxImportSource`, `paths`,
//!   `rootDirs`, `typeRoots`, `types` (including `"*"`), `customConditions`,
//!   `moduleSuffixes`, `preserveSymlinks`, `allowJs`, `resolveJsonModule`,
//!   `resolvePackageJsonExports`, `resolvePackageJsonImports`, `target`
//!   (`es6`/`es2015`…`esnext`), `lib`, `noLib`, `libReplacement`,
//!   `outDir`, `declarationDir`, `rootDir`, `composite`, `skipLibCheck`,
//!   `skipDefaultLibCheck`, and `noUncheckedSideEffectImports`.
//! - Rejected as unsupported: `baseUrl`, `moduleResolution` `node`/`node10`/
//!   `classic`, `module` `amd`/`umd`/`system`, `target` `es5`,
//!   `noResolve`, `noDtsResolution`, non-empty `references`, and JavaScript
//!   inputs. JSON files are inputs under `resolveJsonModule`.
//! - The pinned option catalog validates every option's input type and exact
//!   key; unknown non-null options and removed settings are configuration
//!   errors. Effective strict-family inputs are retained in the snapshot.
//!   `docs/types-options.md` classifies the remaining accepted options.

use super::test_compiler::{self, BASE_OPTIONS, MANIFEST, manifest, typescript_5_6};
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

mod redirects;

struct Project {
    root: PathBuf,
}

impl Project {
    fn new(files: &[(&str, &str)]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "tsparse-project-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let project = Self { root };
        project.write(files);
        project
    }

    fn write(&self, files: &[(&str, &str)]) {
        for (path, contents) in files {
            let path = self.root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
    }

    #[cfg(unix)]
    fn link(&self, link: &str, target: &str) {
        let link = self.root.join(link);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    fn try_load(&self, config: &str) -> Result<Program, InputError> {
        load_on_parsing_thread(&self.root.join(config))
    }

    fn load(&self, config: &str) -> Program {
        self.try_load(config)
            .unwrap_or_else(|error| panic!("{config} failed to load: {error}"))
    }

    fn error(&self, config: &str) -> InputError {
        match self.try_load(config) {
            Ok(_) => panic!("{config} loaded"),
            Err(error) => error,
        }
    }

    fn display(&self, path: &Path) -> String {
        let root = std::fs::canonicalize(&self.root).unwrap();
        path.strip_prefix(&root)
            .or_else(|_| path.strip_prefix(&self.root))
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// `load` with the parser, on the parsing thread that alone provides it.
fn load_on_parsing_thread(config: &Path) -> Result<Program, InputError> {
    crate::analysis::on_parsing_thread("tsparse-test-load", |parser| {
        load(config, &super::disk::Disk::default(), parser)
    })
    .unwrap()
}

/// Input display paths in `FileId` order.
fn inputs(program: &Program) -> Vec<&str> {
    program
        .sources
        .ids()
        .map(|id| program.sources.get(id).display.as_str())
        .collect()
}

/// Inputs in program order as `tsc --listFilesOnly` names them: a standard
/// library from the dialect's directory, the vendored TypeScript 7.0.2 one
/// or the installed compiler's, by its file name, any other input by its
/// display path.
fn listed(program: &Program) -> Vec<String> {
    let compiler = &program.options.compiler;
    let directory = compiler
        .dialect()
        .libraries()
        .directory(compiler.package_directory(), &disk::Disk::default())
        .unwrap();
    let directory = std::fs::canonicalize(directory).unwrap();
    listed_in(program, &paths::from_path(&directory).unwrap())
}

fn sorted(mut paths: Vec<&str>) -> Vec<&str> {
    paths.sort_unstable();
    paths
}

fn searched(program: &Program) -> Vec<&str> {
    sorted(
        program
            .sources
            .ids()
            .filter(|id| program.files[id.index()].search)
            .map(|id| program.sources.get(id).display.as_str())
            .collect(),
    )
}

fn file<'p>(program: &'p Program, display: &str) -> (FileId, &'p ProgramFile) {
    let id = program
        .sources
        .ids()
        .find(|&id| program.sources.get(id).display == display)
        .unwrap_or_else(|| panic!("{display} is not an input of {:?}", inputs(program)));
    (id, &program.files[id.index()])
}

/// How the first request for `specifier` in `from` resolved: a display path,
/// `missing`, or `unsupported`.
fn resolved(program: &Program, from: &str, specifier: &str) -> String {
    let (_, file) = file(program, from);
    let index = file
        .facts
        .requests
        .iter()
        .position(|request| request.specifier == specifier)
        .unwrap_or_else(|| panic!("{from} does not request {specifier}"));
    match &file.modules[index] {
        ModuleResolution::File(id) => program.sources.get(*id).display.clone(),
        ModuleResolution::Missing => "missing".into(),
        ModuleResolution::Unchecked => "unchecked".into(),
        ModuleResolution::Unsupported(_) => "unsupported".into(),
        ModuleResolution::NotAdmitted(..) => "not admitted".into(),
        ModuleResolution::Untyped(_) => "untyped".into(),
    }
}

fn position(line: usize, column: usize) -> Option<SourcePosition> {
    Some(SourcePosition { line, column })
}

const NO_LIB: &str = r#"{ "compilerOptions": { "noLib": true } }"#;

#[test]
fn extends_resolves_paths_against_the_declaring_configuration() {
    let project = Project::new(&[
        (
            "configs/base.json",
            "// comment\n{ \"compilerOptions\": { \"noLib\": true, /* c */ \"paths\": { \"@lib/*\": [\"../lib/*\"] }, }, \"include\": [\"../src/**/*\"], }",
        ),
        ("proj/tsconfig.json", r#"{ "extends": "../configs/base" }"#),
        (
            "src/a.ts",
            "import { l } from \"@lib/l\"; export const a = l;",
        ),
        ("lib/l.ts", "export const l = 1;"),
        ("proj/ignored.ts", "export {}"),
    ]);
    let program = project.load("proj/tsconfig.json");
    // Reported in the canonical directory that result paths are relative to.
    let directory = std::fs::canonicalize(project.root.join("proj")).unwrap();
    assert_eq!(program.config, directory.join("tsconfig.json"));
    assert_eq!(inputs(&program), ["../lib/l.ts", "../src/a.ts"]);
    assert_eq!(resolved(&program, "../src/a.ts", "@lib/l"), "../lib/l.ts");
    assert_eq!(searched(&program), ["../lib/l.ts", "../src/a.ts"]);
}

#[test]
fn extends_arrays_apply_in_order_and_file_lists_replace() {
    let project = Project::new(&[
        (
            "one.json",
            r#"{ "compilerOptions": { "noLib": true, "paths": { "x": ["./one.ts"] } }, "include": ["one.ts"] }"#,
        ),
        (
            "two.json",
            r#"{ "compilerOptions": { "paths": { "x": ["./two.ts"] } }, "include": ["main.ts"] }"#,
        ),
        (
            "tsconfig.json",
            r#"{ "extends": ["./one.json", "./two.json"] }"#,
        ),
        ("main.ts", "import \"x\";"),
        ("one.ts", "export {}"),
        ("two.ts", "export {}"),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(inputs(&program), ["two.ts", "main.ts"]);
    assert_eq!(resolved(&program, "main.ts", "x"), "two.ts");
}

#[test]
fn extends_resolves_package_configurations() {
    let config = |name: &str| {
        format!(
            r#"{{ "compilerOptions": {{ "noLib": true, "paths": {{ "probe": ["${{configDir}}/{name}.ts"] }} }} }}"#
        )
    };
    let leaf = |extends: &str| format!(r#"{{ "extends": "{extends}", "files": ["main.ts"] }}"#);
    let project = Project::new(&[
        (
            "node_modules/@tsconfig/strict/tsconfig.json",
            &config("scoped"),
        ),
        (
            "node_modules/field/package.json",
            r#"{ "tsconfig": "./base.json" }"#,
        ),
        ("node_modules/field/base.json", &config("field")),
        (
            "node_modules/exported/package.json",
            r#"{ "exports": { "./strict": "./configs/strict.json" } }"#,
        ),
        (
            "node_modules/exported/configs/strict.json",
            &config("exported"),
        ),
        ("node_modules/rooted/package.json", "{}"),
        ("node_modules/rooted/tsconfig.json", &config("rooted")),
        ("scoped.json", &leaf("@tsconfig/strict/tsconfig.json")),
        ("field.json", &leaf("field")),
        ("exported.json", &leaf("exported/strict")),
        ("rooted.json", &leaf("rooted")),
        ("main.ts", "import \"probe\";"),
        ("scoped.ts", ""),
        ("field.ts", ""),
        ("exported.ts", ""),
        ("rooted.ts", ""),
    ]);
    for name in ["scoped", "field", "exported", "rooted"] {
        let program = project.load(&format!("{name}.json"));
        assert_eq!(resolved(&program, "main.ts", "probe"), format!("{name}.ts"));
    }
}

#[test]
fn extends_errors_are_located() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "extends": "./b.json", "files": ["main.ts"] }"#,
        ),
        ("b.json", "{\n  \"extends\": \"./tsconfig.json\"\n}"),
        ("missing.json", r#"{ "extends": "./nope" }"#),
        ("unreadable.json", r#"{ "extends": "./nope.json" }"#),
        ("package.json", r#"{ "extends": "no-such-package" }"#),
        (
            "invalid.json",
            "{\n  \"compilerOptions\": { \"noLib\": true, }\n  x\n}",
        ),
        ("array.json", "[]"),
        ("main.ts", "export {}"),
    ]);
    let cycle = project.error("tsconfig.json");
    assert_eq!(cycle.kind, InputErrorKind::Config);
    assert!(cycle.message.contains("Circularity"), "{}", cycle.message);
    assert_eq!(project.display(cycle.path.as_ref().unwrap()), "b.json");
    assert_eq!(cycle.position, position(2, 14));

    let missing = project.error("missing.json");
    assert_eq!(
        (missing.kind, missing.position),
        (InputErrorKind::Config, position(1, 14))
    );
    // A read failure is located at the `extends` value that named the file.
    let unreadable = project.error("unreadable.json");
    assert_eq!(
        (unreadable.kind, unreadable.position),
        (InputErrorKind::Io, position(1, 14))
    );
    assert_eq!(
        project.display(unreadable.path.as_ref().unwrap()),
        "unreadable.json"
    );
    assert!(
        unreadable.message.contains("nope.json"),
        "{}",
        unreadable.message
    );
    assert_eq!(project.error("package.json").kind, InputErrorKind::Config);
    let invalid = project.error("invalid.json");
    assert_eq!(
        (invalid.kind, invalid.position),
        (InputErrorKind::Config, position(3, 3))
    );
    assert_eq!(project.error("array.json").kind, InputErrorKind::Config);
    assert_eq!(project.error("absent.json").kind, InputErrorKind::Io);
}

#[test]
fn config_dir_template_names_the_selected_configuration() {
    let project = Project::new(&[
        (
            "base/base.json",
            r#"{ "compilerOptions": { "noLib": true, "paths": { "p": ["${configDir}/p.ts"] } }, "include": ["${configDir}/src"] }"#,
        ),
        ("tsconfig.json", r#"{ "extends": "./base/base.json" }"#),
        ("src/main.ts", "import \"p\";"),
        ("p.ts", "export {}"),
        ("base/src/other.ts", "export {}"),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(inputs(&program), ["p.ts", "src/main.ts"]);
}

#[test]
fn project_references_are_rejected_and_not_inherited() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": [], "references": [{ "path": "./sub" }] }"#,
        ),
        (
            "empty.json",
            r#"{ "compilerOptions": { "noLib": true }, "references": [] }"#,
        ),
        (
            "base.json",
            r#"{ "compilerOptions": { "noLib": true }, "references": [{ "path": "./sub" }] }"#,
        ),
        ("child.json", r#"{ "extends": "./base.json" }"#),
        ("a.ts", "export {}"),
    ]);
    let error = project.error("tsconfig.json");
    assert_eq!(
        (error.kind, error.position),
        (InputErrorKind::Unsupported, position(1, 68))
    );
    assert_eq!(inputs(&project.load("empty.json")), ["a.ts"]);
    assert_eq!(inputs(&project.load("child.json")), ["a.ts"]);
}

#[test]
fn removed_and_unsupported_options_are_rejected() {
    let options =
        |options: &str| format!(r#"{{ "compilerOptions": {{ "noLib": true, {options} }} }}"#);
    let cases = [
        (
            r#""moduleResolution": "node10""#,
            InputErrorKind::Unsupported,
        ),
        (r#""moduleResolution": "node""#, InputErrorKind::Unsupported),
        (
            r#""moduleResolution": "Classic""#,
            InputErrorKind::Unsupported,
        ),
        (r#""baseUrl": ".""#, InputErrorKind::Unsupported),
        (r#""module": "amd""#, InputErrorKind::Unsupported),
        // TS5108; TypeScript still loads `lib.d.ts`.
        (r#""target": "ES5""#, InputErrorKind::Unsupported),
        (r#""target": "es3""#, InputErrorKind::Config),
        (r#""target": 2022"#, InputErrorKind::Config),
        (r#""noResolve": true"#, InputErrorKind::Unsupported),
        (r#""moduleResolution": "bogus""#, InputErrorKind::Config),
        (r#""module": "none""#, InputErrorKind::Config),
        (r#""moduleResolution": "node16""#, InputErrorKind::Config),
        (
            r#""module": "nodenext", "moduleResolution": "bundler""#,
            InputErrorKind::Config,
        ),
        (r#""lib": []"#, InputErrorKind::Config),
        (r#""paths": { "a": ["src/a"] }"#, InputErrorKind::Config),
        (r#""paths": { "a/*/*": ["./a"] }"#, InputErrorKind::Config),
        (r#""types": "node""#, InputErrorKind::Config),
    ];
    let mut files: Vec<(String, String)> = vec![("a.ts".into(), "export {}".into())];
    for (index, (option, _)) in cases.iter().enumerate() {
        files.push((format!("{index}.json"), options(option)));
    }
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let project = Project::new(&files);
    for (index, (option, kind)) in cases.iter().enumerate() {
        let error = project.error(&format!("{index}.json"));
        assert_eq!(error.kind, *kind, "{option}: {error}");
        assert!(error.position.is_some(), "{option}");
    }
}

#[test]
fn supported_combinations_and_irrelevant_options_load() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "node18", "moduleResolution": "nodenext", "lib": [],
               "noLib": false, "strict": true, "target": "ESNext", "sourceMap": true, "declaration": true,
               "plugins": [{ "name": "x" }], "allowImportingTsExtensions": true, "esModuleInterop": true },
               "watchOptions": {}, "compileOnSave": true }"#,
        ),
        (
            "upper.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "NodeNext", "moduleResolution": "NodeNext" } }"#,
        ),
        (
            "preserve.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "preserve", "moduleResolution": "Bundler" } }"#,
        ),
        ("a.ts", "export {}"),
    ]);
    for config in ["tsconfig.json", "upper.json", "preserve.json"] {
        assert_eq!(inputs(&project.load(config)), ["a.ts"]);
    }
}

#[test]
fn default_include_selects_supported_files_in_compiler_order() {
    let declaration = "export {}";
    let project = Project::new(&[
        ("tsconfig.json", NO_LIB),
        ("b.ts", declaration),
        ("a.ts", declaration),
        ("A2.ts", declaration),
        ("a.d.ts", declaration),
        ("only.d.ts", declaration),
        ("c.d.ts", declaration),
        ("c.tsx", declaration),
        ("d.d.mts", declaration),
        ("d.mts", declaration),
        ("e.cts", declaration),
        ("f.d.cts", declaration),
        ("g.d.css.ts", declaration),
        ("h.js", declaration),
        ("i.json", "{}"),
        ("j.ts.bak", declaration),
        ("sub/z.ts", declaration),
        ("sub/deep/y.ts", declaration),
        ("node_modules/nm/x.ts", declaration),
        ("sub/node_modules/x.ts", declaration),
        ("bower_components/x.ts", declaration),
        ("jspm_packages/x.ts", declaration),
        ("out/o.ts", declaration),
        (".hidden/h.ts", declaration),
        (".dot.ts", declaration),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(
        inputs(&program),
        [
            "A2.ts",
            "a.ts",
            "b.ts",
            "c.tsx",
            "d.mts",
            "e.cts",
            "f.d.cts",
            "g.d.css.ts",
            "only.d.ts",
            "out/o.ts",
            "sub/z.ts",
            "sub/deep/y.ts"
        ]
    );
    assert!(program.files.iter().all(|file| file.search));
}

#[test]
fn include_and_exclude_follow_wildcard_rules() {
    let project = Project::new(&[
        ("a.ts", ""),
        ("a.d.ts", ""),
        ("ab.ts", ""),
        ("x.d.ts", ""),
        ("sub/z.ts", ""),
        ("sub/deep/y.ts", ""),
        ("sub/gen/g.ts", ""),
        ("node_modules/nm/x.ts", ""),
        (".hidden/h.ts", ""),
        (
            "dir.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["sub"], "exclude": ["sub/gen"] }"#,
        ),
        (
            "question.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["?.ts"] }"#,
        ),
        (
            "explicit.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["node_modules/**/*", ".hidden/*"] }"#,
        ),
        (
            "declarations.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["*.d.ts"] }"#,
        ),
        (
            "deep.json",
            r#"{ "compilerOptions": { "noLib": true }, "exclude": ["**/deep", "*.d.*", "a*"] }"#,
        ),
        (
            "trailing.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["sub/**"] }"#,
        ),
    ]);
    assert_eq!(
        inputs(&project.load("dir.json")),
        ["sub/z.ts", "sub/deep/y.ts"]
    );
    assert_eq!(inputs(&project.load("question.json")), ["a.ts"]);
    assert_eq!(
        inputs(&project.load("explicit.json")),
        ["node_modules/nm/x.ts", ".hidden/h.ts"]
    );
    assert_eq!(
        inputs(&project.load("declarations.json")),
        ["a.d.ts", "x.d.ts"]
    );
    assert_eq!(
        inputs(&project.load("deep.json")),
        ["sub/z.ts", "sub/gen/g.ts"]
    );
    assert_eq!(project.error("trailing.json").kind, InputErrorKind::Config);
}

#[test]
fn output_directories_are_default_excludes_even_when_inherited() {
    let project = Project::new(&[
        (
            "base/base.json",
            r#"{ "compilerOptions": { "noLib": true, "outDir": "../out", "declarationDir": "../types" } }"#,
        ),
        ("tsconfig.json", r#"{ "extends": "./base/base.json" }"#),
        (
            "explicit.json",
            r#"{ "extends": "./base/base.json", "exclude": ["sub"] }"#,
        ),
        ("a.ts", ""),
        ("out/o.ts", ""),
        ("types/t.d.ts", ""),
        ("sub/s.ts", ""),
    ]);
    assert_eq!(inputs(&project.load("tsconfig.json")), ["a.ts", "sub/s.ts"]);
    assert_eq!(
        inputs(&project.load("explicit.json")),
        ["a.ts", "out/o.ts", "types/t.d.ts"]
    );
}

#[test]
fn files_entries_are_literal_and_must_exist() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["sub/z.ts", "node_modules/nm/x.ts", "noext"], "include": ["b.ts"], "exclude": ["sub"] }"#,
        ),
        (
            "missing.json",
            "{ \"compilerOptions\": { \"noLib\": true },\n  \"files\": [\"b.ts\", \"missing.ts\"] }",
        ),
        (
            "js.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["h.js"] }"#,
        ),
        (
            "text.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["notes.txt"] }"#,
        ),
        // TypeScript 7.0.2: a JSON entry is an input under
        // `resolveJsonModule` (on by default with bundler resolution) and
        // must then exist (TS6053); without it, its extension is
        // unsupported (TS6054), whether or not it exists.
        (
            "json.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["b.ts", "data.json"] }"#,
        ),
        (
            "jsonmissing.json",
            "{ \"compilerOptions\": { \"noLib\": true },\n  \"files\": [\"b.ts\", \"nope.json\"] }",
        ),
        (
            "jsonoff.json",
            r#"{ "compilerOptions": { "noLib": true, "resolveJsonModule": false }, "files": ["b.ts", "data.json"] }"#,
        ),
        (
            "jsonoffmissing.json",
            r#"{ "compilerOptions": { "noLib": true, "resolveJsonModule": false }, "files": ["b.ts", "nope.json"] }"#,
        ),
        // Support follows the case policy; the script kind ignores case.
        (
            "uppercase.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["b.ts", "Mixed.JSON"] }"#,
        ),
        ("b.ts", ""),
        ("noext.ts", ""),
        ("sub/z.ts", ""),
        ("node_modules/nm/x.ts", ""),
        ("h.js", ""),
        ("notes.txt", ""),
        ("data.json", "{}"),
        ("Mixed.JSON", "{ \"parsed\": [\"as\", \"JSON\"] }"),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(
        inputs(&program),
        ["sub/z.ts", "node_modules/nm/x.ts", "noext.ts", "b.ts"]
    );
    assert_eq!(searched(&program).len(), 4);
    let missing = project.error("missing.json");
    assert_eq!(
        (missing.kind, missing.position),
        (InputErrorKind::Io, position(2, 21))
    );
    assert_eq!(project.error("js.json").kind, InputErrorKind::Unsupported);
    assert_eq!(project.error("text.json").kind, InputErrorKind::Config);
    let json = project.load("json.json");
    assert_eq!(inputs(&json), ["b.ts", "data.json"]);
    assert_eq!(searched(&json), ["b.ts", "data.json"]);
    let missing = project.error("jsonmissing.json");
    assert_eq!(
        (missing.kind, missing.position),
        (InputErrorKind::Io, position(2, 21))
    );
    for config in ["jsonoff.json", "jsonoffmissing.json"] {
        let error = project.error(config);
        assert_eq!(error.kind, InputErrorKind::Config, "{config}: {error}");
        assert!(error.message.contains("unsupported extension"), "{error}");
    }
    // Case `upper-file-files` in `fixtures/json-probes/inputs.json`
    // (case-insensitive): TypeScript includes it.
    match Case::detect() {
        Case::Insensitive => {
            let upper = project.load("uppercase.json");
            assert_eq!(inputs(&upper), ["b.ts", "Mixed.JSON"]);
            let (_, file) = file(&upper, "Mixed.JSON");
            assert!(file.facts.module);
        }
        Case::Sensitive => {
            assert_eq!(project.error("uppercase.json").kind, InputErrorKind::Config);
        }
    }
}

#[test]
fn empty_programs_follow_the_no_inputs_rule() {
    let project = Project::new(&[
        (
            "none.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["nothing/*"] }"#,
        ),
        (
            "emptyfiles.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": [] }"#,
        ),
        (
            "emptyrefs.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": [], "references": [] }"#,
        ),
        ("base.json", NO_LIB),
        (
            "extended.json",
            r#"{ "extends": "./base.json", "files": [] }"#,
        ),
    ]);
    let none = project.error("none.json");
    assert_eq!(none.kind, InputErrorKind::Config);
    assert!(
        none.message.starts_with("No inputs were found"),
        "{}",
        none.message
    );
    assert_eq!(
        project.error("emptyfiles.json").kind,
        InputErrorKind::Config
    );
    assert_eq!(project.error("emptyrefs.json").kind, InputErrorKind::Config);
    assert!(inputs(&project.load("extended.json")).is_empty());
}

#[test]
fn javascript_roots_need_allow_js_json_roots_are_inputs_and_shadowed_javascript_is_not_a_root() {
    let project = Project::new(&[
        (
            "allowjs.json",
            r#"{ "compilerOptions": { "noLib": true, "allowJs": true }, "include": ["src"] }"#,
        ),
        (
            "listed.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["src/a.js"] }"#,
        ),
        (
            "shadowed.json",
            r#"{ "compilerOptions": { "noLib": true, "allowJs": true }, "include": ["lib"] }"#,
        ),
        (
            "json.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["data/*.json", "lib"] }"#,
        ),
        (
            "plain.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["data", "lib"] }"#,
        ),
        (
            "jsonoff.json",
            r#"{ "compilerOptions": { "noLib": true, "resolveJsonModule": false }, "include": ["data/*.json", "lib"] }"#,
        ),
        (
            "all.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["lib/*", "*.json"] }"#,
        ),
        (
            "star.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["**/*"] }"#,
        ),
        ("src/a.js", ""),
        ("lib/b.ts", ""),
        ("lib/b.js", ""),
        ("data/d.json", "{}"),
    ]);
    assert_eq!(inputs(&project.load("allowjs.json")), ["src/a.js"]);
    // TS6504 for a JavaScript file named without allowJs.
    let error = project.error("listed.json");
    assert_eq!(error.kind, InputErrorKind::Unsupported);
    assert_eq!(project.display(error.path.as_ref().unwrap()), "listed.json");
    assert!(error.message.contains("allowJs"), "{error}");
    assert_eq!(inputs(&project.load("shadowed.json")), ["lib/b.ts"]);
    // TypeScript 7.0.2 (`tsc --listFilesOnly`): an include spec ending in
    // `.json` selects JSON files, after the other wildcard matches, only
    // under `resolveJsonModule`; `*.json` also selects the configurations.
    assert_eq!(
        inputs(&project.load("json.json")),
        ["lib/b.ts", "data/d.json"]
    );
    assert_eq!(inputs(&project.load("plain.json")), ["lib/b.ts"]);
    assert_eq!(inputs(&project.load("jsonoff.json")), ["lib/b.ts"]);
    assert_eq!(inputs(&project.load("star.json")), ["lib/b.ts"]);
    assert_eq!(
        inputs(&project.load("all.json")),
        [
            "lib/b.ts",
            "all.json",
            "allowjs.json",
            "json.json",
            "jsonoff.json",
            "listed.json",
            "plain.json",
            "shadowed.json",
            "star.json"
        ]
    );
}

const CONDITIONAL: &str = r#"{ "name": "pkg", "exports": { "./*": { "import": "./esm/*.d.ts", "require": "./cjs/*.d.ts" } } }"#;

/// A package whose `exports` pattern distinguishes conditions per subpath.
fn conditional_package(files: &mut Vec<(String, String)>) {
    files.push(("node_modules/pkg/package.json".into(), CONDITIONAL.into()));
    for name in ["import", "require", "dynamic", "type"] {
        for kind in ["esm", "cjs"] {
            files.push((
                format!("node_modules/pkg/{kind}/{name}.d.ts"),
                "export {}".into(),
            ));
        }
    }
}

const REQUESTS: &str = "import \"pkg/import\";\nimport r = require(\"pkg/require\");\nconst d = import(\"pkg/dynamic\");\ntype T = typeof import(\"pkg/type\");\nexport {}\n";

/// The condition each request kind selected: `esm` or `cjs` for static
/// import, `import =`, dynamic import, and import type, in that order.
fn conditions(program: &Program, from: &str) -> [String; 4] {
    ["import", "require", "dynamic", "type"].map(|name| {
        let target = resolved(program, from, &format!("pkg/{name}"));
        target.split('/').nth(2).unwrap_or(&target).to_owned()
    })
}

fn project_with_requests(configs: &[(&str, &str)], sources: &[&str]) -> Project {
    let mut files: Vec<(String, String)> = Vec::new();
    conditional_package(&mut files);
    for source in sources {
        files.push(((*source).into(), REQUESTS.into()));
    }
    files.push((
        "cjspkg/package.json".into(),
        r#"{ "type": "commonjs" }"#.into(),
    ));
    files.push((
        "esmpkg/package.json".into(),
        r#"{ "type": "module" }"#.into(),
    ));
    for (name, contents) in configs {
        files.push(((*name).into(), (*contents).into()));
    }
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    Project::new(&files)
}

#[test]
fn bundler_conditions_follow_emitted_syntax() {
    let sources = [
        "plain/a.ts",
        "plain/b.cts",
        "plain/c.mts",
        "cjspkg/a.ts",
        "esmpkg/a.ts",
    ];
    let project = project_with_requests(
        &[
            ("default.json", NO_LIB),
            (
                "commonjs.json",
                r#"{ "compilerOptions": { "noLib": true, "module": "commonjs" } }"#,
            ),
            (
                "preserve.json",
                r#"{ "compilerOptions": { "noLib": true, "module": "preserve" } }"#,
            ),
        ],
        &sources,
    );
    let expect = |config: &str, rows: [[&str; 4]; 5]| {
        let program = project.load(config);
        for (source, row) in sources.iter().zip(rows) {
            assert_eq!(conditions(&program, source), row, "{config} {source}");
        }
    };
    let esm = ["esm", "cjs", "esm", "esm"];
    let cjs = ["cjs"; 4];
    expect("default.json", [esm, cjs, esm, esm, esm]);
    expect("commonjs.json", [cjs, cjs, esm, cjs, cjs]);
    expect(
        "preserve.json",
        [esm, ["cjs", "cjs", "esm", "cjs"], esm, esm, esm],
    );
}

#[test]
fn node_modes_follow_the_importing_file_format() {
    let sources = [
        "plain/a.ts",
        "plain/b.cts",
        "plain/c.mts",
        "cjspkg/a.ts",
        "esmpkg/a.ts",
    ];
    let project = project_with_requests(
        &[
            (
                "node16.json",
                r#"{ "compilerOptions": { "noLib": true, "module": "node16" } }"#,
            ),
            (
                "nodenext.json",
                r#"{ "compilerOptions": { "noLib": true, "module": "nodenext" } }"#,
            ),
        ],
        &sources,
    );
    let cjs = ["cjs", "cjs", "esm", "cjs"];
    let esm = ["esm", "cjs", "esm", "esm"];
    for config in ["node16.json", "nodenext.json"] {
        let program = project.load(config);
        for (source, row) in sources.iter().zip([cjs, cjs, esm, cjs, esm]) {
            assert_eq!(conditions(&program, source), row, "{config} {source}");
        }
    }
}

#[test]
fn custom_conditions_and_resolution_mode_attributes_select_exports() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext", "customConditions": ["custom"] } }"#,
        ),
        (
            "a.mts",
            "import type { A } from \"cond\" with { \"resolution-mode\": \"require\" };\ntype T = typeof import(\"cond/sub\", { with: { \"resolution-mode\": \"require\" } });\nimport \"custom\";\n// A value import's resolution-mode is ignored.\nimport { B } from \"cond/value\" with { \"resolution-mode\": \"require\" };\n",
        ),
        (
            "node_modules/cond/package.json",
            r#"{ "exports": { ".": { "import": "./esm.d.ts", "require": "./cjs.d.ts" }, "./value": { "import": "./esm.d.ts", "require": "./cjs.d.ts" }, "./sub": { "types@>=7": { "require": "./new.d.ts" }, "types": "./old.d.ts" } } }"#,
        ),
        ("node_modules/cond/esm.d.ts", ""),
        ("node_modules/cond/cjs.d.ts", ""),
        ("node_modules/cond/new.d.ts", ""),
        ("node_modules/cond/old.d.ts", ""),
        (
            "node_modules/custom/package.json",
            r#"{ "exports": { "custom": "./custom.d.ts", "default": "./default.d.ts" } }"#,
        ),
        ("node_modules/custom/custom.d.ts", ""),
        ("node_modules/custom/default.d.ts", ""),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(
        resolved(&program, "a.mts", "cond"),
        "node_modules/cond/cjs.d.ts"
    );
    assert_eq!(
        resolved(&program, "a.mts", "cond/sub"),
        "node_modules/cond/new.d.ts"
    );
    assert_eq!(
        resolved(&program, "a.mts", "custom"),
        "node_modules/custom/custom.d.ts"
    );
    assert_eq!(
        resolved(&program, "a.mts", "cond/value"),
        "node_modules/cond/esm.d.ts"
    );
    assert_eq!(searched(&program), ["a.mts"]);
}

/// Candidate selection only: `jsx` and `allowArbitraryExtensions` let the
/// program admit every candidate (`resolve::admission`, whose cases are in
/// `fixtures/json-probes/admission.json`).
#[test]
fn relative_specifiers_follow_extension_and_directory_rules() {
    let project = Project::new(&[
        ("package.json", r#"{ "type": "module" }"#),
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext", "jsx": "preserve", "allowArbitraryExtensions": true }, "files": ["src/a.ts", "src/c.cts"] }"#,
        ),
        (
            "bundler.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["src/a.ts"] }"#,
        ),
        (
            "src/a.ts",
            "import \"./noext\";\nimport \"./withjs.js\";\nimport \"./dir\";\nimport \"./dir/index.js\";\nimport \"./explicit.ts\";\nimport \"./decl.js\";\nimport \"./m.mjs\";\nimport \"./c.cjs\";\nimport \"./tsxfile.js\";\nimport \"./jsxfile.jsx\";\nimport \"./both.js\";\nimport \"./onlyjs.js\";\nimport \"./data.json\";\nimport \"./styles.css\";\nimport \"node:fs\";\nimport \"./dirpkg\";\n",
        ),
        (
            "src/c.cts",
            "import \"./noext\";\nimport \"./dir\";\nimport \"./m.mjs\";\n",
        ),
        ("src/noext.ts", ""),
        ("src/withjs.ts", ""),
        ("src/dir/index.ts", ""),
        ("src/explicit.ts", ""),
        ("src/decl.d.ts", ""),
        ("src/m.mts", ""),
        ("src/tsxfile.tsx", ""),
        ("src/jsxfile.tsx", ""),
        ("src/both.ts", ""),
        ("src/both.d.ts", ""),
        ("src/onlyjs.js", ""),
        ("src/data.json", "{}"),
        ("src/styles.d.css.ts", ""),
        ("src/dirpkg/package.json", r#"{ "types": "typed.d.ts" }"#),
        ("src/dirpkg/typed.d.ts", ""),
    ]);
    let program = project.load("tsconfig.json");
    let expect = |program: &Program, from: &str, cases: &[(&str, &str)]| {
        for (specifier, expected) in cases {
            assert_eq!(
                resolved(program, from, specifier),
                *expected,
                "{from} {specifier}"
            );
        }
    };
    expect(
        &program,
        "src/a.ts",
        &[
            ("./noext", "missing"),
            ("./withjs.js", "src/withjs.ts"),
            ("./dir", "missing"),
            ("./dir/index.js", "src/dir/index.ts"),
            ("./explicit.ts", "src/explicit.ts"),
            ("./decl.js", "src/decl.d.ts"),
            ("./m.mjs", "src/m.mts"),
            ("./c.cjs", "src/c.cts"),
            ("./tsxfile.js", "src/tsxfile.tsx"),
            ("./jsxfile.jsx", "src/jsxfile.tsx"),
            ("./both.js", "src/both.ts"),
            // JavaScript without allowJs is an untyped module.
            ("./onlyjs.js", "untyped"),
            // `nodenext` turns `resolveJsonModule` on; TS1543's import
            // attribute rule does not change the resolution.
            ("./data.json", "src/data.json"),
            ("./styles.css", "src/styles.d.css.ts"),
            ("node:fs", "missing"),
            ("./dirpkg", "missing"),
        ],
    );
    expect(
        &program,
        "src/c.cts",
        &[
            ("./noext", "src/noext.ts"),
            ("./dir", "src/dir/index.ts"),
            ("./m.mjs", "src/m.mts"),
        ],
    );
    let bundler = project.load("bundler.json");
    expect(
        &bundler,
        "src/a.ts",
        &[
            ("./noext", "src/noext.ts"),
            ("./dir", "src/dir/index.ts"),
            ("./dirpkg", "src/dirpkg/typed.d.ts"),
        ],
    );
}

#[test]
fn package_imports_and_self_names_resolve_locally() {
    let project = Project::new(&[
        (
            "package.json",
            r##"{ "name": "self", "type": "module", "exports": { ".": "./src/index.ts", "./feature": "./src/feature.ts" }, "imports": { "#int": "./src/internal.ts", "#star/*": "./src/star/*.ts", "#dep": "dep", "#missing": "./src/none.ts" } }"##,
        ),
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext" }, "files": ["src/a.ts"] }"#,
        ),
        (
            "src/a.ts",
            "import \"#int\";\nimport \"#star/s\";\nimport \"self/feature\";\nimport \"self\";\nimport \"#dep\";\nimport \"#missing\";\nimport \"#unknown\";\n",
        ),
        ("src/index.ts", ""),
        ("src/feature.ts", ""),
        ("src/internal.ts", ""),
        ("src/star/s.ts", ""),
        ("node_modules/dep/package.json", r#"{ "name": "dep" }"#),
        ("node_modules/dep/index.d.ts", ""),
        ("node_modules/#unknown/index.d.ts", ""),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(resolved(&program, "src/a.ts", "#int"), "src/internal.ts");
    assert_eq!(resolved(&program, "src/a.ts", "#star/s"), "src/star/s.ts");
    assert_eq!(
        resolved(&program, "src/a.ts", "self/feature"),
        "src/feature.ts"
    );
    assert_eq!(resolved(&program, "src/a.ts", "self"), "src/index.ts");
    assert_eq!(
        resolved(&program, "src/a.ts", "#dep"),
        "node_modules/dep/index.d.ts"
    );
    assert_eq!(resolved(&program, "src/a.ts", "#missing"), "missing");
    assert_eq!(resolved(&program, "src/a.ts", "#unknown"), "missing");
    assert_eq!(
        searched(&program),
        [
            "src/a.ts",
            "src/feature.ts",
            "src/index.ts",
            "src/internal.ts",
            "src/star/s.ts"
        ]
    );
}

#[test]
fn package_metadata_exports_and_type_versions() {
    let source = [
        "types-field",
        "typings-field",
        "main-field",
        "main-dts-sibling",
        "index-only",
        "tv",
        "tv/sub",
        "exp",
        "exp/feature",
        "exp/pattern/x",
        "exp/blocked/x",
        "exp/notexported",
        "expsugar",
        "exparr",
        "@scope/scoped",
        "typesonly",
        "exptv",
        "nested/deep",
        "jsonly",
        "nullblock",
    ]
    .map(|name| format!("import \"{name}\";\n"))
    .concat();
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts"] }"#,
        ),
        ("a.ts", &source),
        (
            "node_modules/types-field/package.json",
            r#"{ "types": "./lib/t.d.ts", "main": "./lib/m.js" }"#,
        ),
        ("node_modules/types-field/lib/t.d.ts", ""),
        (
            "node_modules/typings-field/package.json",
            r#"{ "typings": "lib/t.d.ts", "types": "other.d.ts" }"#,
        ),
        ("node_modules/typings-field/lib/t.d.ts", ""),
        ("node_modules/typings-field/other.d.ts", ""),
        (
            "node_modules/main-field/package.json",
            r#"{ "main": "./lib/main" }"#,
        ),
        ("node_modules/main-field/lib/main.ts", ""),
        (
            "node_modules/main-dts-sibling/package.json",
            r#"{ "main": "./lib/main.js" }"#,
        ),
        ("node_modules/main-dts-sibling/lib/main.d.ts", ""),
        ("node_modules/main-dts-sibling/lib/main.js", ""),
        ("node_modules/index-only/index.d.ts", ""),
        (
            "node_modules/tv/package.json",
            r#"{ "types": "index.d.ts", "typesVersions": { "<4": { "*": ["old/*"] }, ">=4.0": { "*": ["ts4/*"] } } }"#,
        ),
        ("node_modules/tv/index.d.ts", ""),
        ("node_modules/tv/ts4/index.d.ts", ""),
        ("node_modules/tv/ts4/sub.d.ts", ""),
        ("node_modules/tv/sub.d.ts", ""),
        (
            "node_modules/exp/package.json",
            r#"{ "types": "./wrong.d.ts", "exports": { ".": { "types": "./root.d.ts", "default": "./root.js" }, "./feature": { "import": { "types": "./feat-esm.d.ts" }, "require": { "types": "./feat-cjs.d.ts" } }, "./pattern/*": "./pat/*.d.ts", "./blocked/*": null } }"#,
        ),
        ("node_modules/exp/root.d.ts", ""),
        ("node_modules/exp/wrong.d.ts", ""),
        ("node_modules/exp/feat-esm.d.ts", ""),
        ("node_modules/exp/feat-cjs.d.ts", ""),
        ("node_modules/exp/pat/x.d.ts", ""),
        ("node_modules/exp/blocked/x.d.ts", ""),
        ("node_modules/exp/notexported.d.ts", ""),
        (
            "node_modules/expsugar/package.json",
            r#"{ "exports": "./main.js" }"#,
        ),
        ("node_modules/expsugar/main.d.ts", ""),
        (
            "node_modules/exparr/package.json",
            r#"{ "exports": { ".": [{ "worker": "./worker.d.ts" }, "./fallback.d.ts"] } }"#,
        ),
        ("node_modules/exparr/fallback.d.ts", ""),
        (
            "node_modules/@scope/scoped/package.json",
            r#"{ "name": "@scope/scoped" }"#,
        ),
        ("node_modules/@scope/scoped/index.js", ""),
        ("node_modules/@types/scope__scoped/index.d.ts", ""),
        ("node_modules/@types/typesonly/index.d.ts", ""),
        (
            "node_modules/exptv/package.json",
            r#"{ "exports": { ".": "./e.d.ts" }, "typesVersions": { "*": { "*": ["tv/*"] } } }"#,
        ),
        ("node_modules/exptv/e.d.ts", ""),
        ("node_modules/exptv/tv/index.d.ts", ""),
        (
            "node_modules/nested/package.json",
            r#"{ "name": "nested" }"#,
        ),
        (
            "node_modules/nested/deep/package.json",
            r#"{ "types": "d.d.ts" }"#,
        ),
        ("node_modules/nested/deep/d.d.ts", ""),
        (
            "node_modules/jsonly/package.json",
            r#"{ "main": "index.js" }"#,
        ),
        ("node_modules/jsonly/index.js", ""),
        (
            "node_modules/nullblock/package.json",
            r#"{ "exports": { "types": null, "default": "./x.d.ts" } }"#,
        ),
        ("node_modules/nullblock/x.d.ts", ""),
    ]);
    let program = project.load("tsconfig.json");
    for (specifier, expected) in [
        ("types-field", "node_modules/types-field/lib/t.d.ts"),
        ("typings-field", "node_modules/typings-field/lib/t.d.ts"),
        ("main-field", "node_modules/main-field/lib/main.ts"),
        (
            "main-dts-sibling",
            "node_modules/main-dts-sibling/lib/main.d.ts",
        ),
        ("index-only", "node_modules/index-only/index.d.ts"),
        ("tv", "node_modules/tv/ts4/index.d.ts"),
        ("tv/sub", "node_modules/tv/ts4/sub.d.ts"),
        ("exp", "node_modules/exp/root.d.ts"),
        ("exp/feature", "node_modules/exp/feat-esm.d.ts"),
        ("exp/pattern/x", "node_modules/exp/pat/x.d.ts"),
        ("exp/blocked/x", "missing"),
        ("exp/notexported", "missing"),
        ("expsugar", "node_modules/expsugar/main.d.ts"),
        ("exparr", "node_modules/exparr/fallback.d.ts"),
        (
            "@scope/scoped",
            "node_modules/@types/scope__scoped/index.d.ts",
        ),
        ("typesonly", "node_modules/@types/typesonly/index.d.ts"),
        ("exptv", "node_modules/exptv/e.d.ts"),
        ("nested/deep", "node_modules/nested/deep/d.d.ts"),
        ("jsonly", "untyped"),
        ("nullblock", "missing"),
    ] {
        assert_eq!(
            resolved(&program, "a.ts", specifier),
            expected,
            "{specifier}"
        );
    }
    assert_eq!(searched(&program), ["a.ts"]);
}

#[test]
fn paths_root_dirs_suffixes_and_type_roots() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "paths": { "alias/*": ["./aliased/*", "./fallback/*"], "alias/exact": ["./exact.ts"], "al*": ["./short/*"], "fall": ["./nothere"], "js": ["./lib.js"] } }, "files": ["src/a.ts"] }"#,
        ),
        (
            "other.json",
            r#"{ "compilerOptions": { "noLib": true, "moduleSuffixes": [".ios", ""], "rootDirs": ["src", "generated"], "typeRoots": ["./types"] }, "files": ["src/b.ts"] }"#,
        ),
        (
            "src/a.ts",
            "import \"alias/one\";\nimport \"alias/two\";\nimport \"alias/exact\";\nimport \"alpha\";\nimport \"fall\";\nimport \"js\";\n",
        ),
        (
            "src/b.ts",
            "import \"./plat\";\nimport \"./gen\";\nimport \"fromtyperoot\";\n",
        ),
        ("aliased/one.ts", ""),
        ("fallback/two.ts", ""),
        ("exact.ts", ""),
        ("short/pha.ts", ""),
        ("node_modules/fall/index.d.ts", ""),
        ("lib.ts", ""),
        ("src/plat.ios.ts", ""),
        ("src/plat.ts", ""),
        ("generated/gen.ts", ""),
        ("types/fromtyperoot/index.d.ts", ""),
    ]);
    let program = project.load("tsconfig.json");
    for (specifier, expected) in [
        ("alias/one", "aliased/one.ts"),
        ("alias/two", "fallback/two.ts"),
        ("alias/exact", "exact.ts"),
        ("alpha", "short/pha.ts"),
        ("fall", "node_modules/fall/index.d.ts"),
        ("js", "lib.ts"),
    ] {
        assert_eq!(
            resolved(&program, "src/a.ts", specifier),
            expected,
            "{specifier}"
        );
    }
    assert_eq!(
        searched(&program),
        [
            "aliased/one.ts",
            "exact.ts",
            "fallback/two.ts",
            "lib.ts",
            "short/pha.ts",
            "src/a.ts"
        ]
    );
    let other = project.load("other.json");
    assert_eq!(resolved(&other, "src/b.ts", "./plat"), "src/plat.ios.ts");
    assert_eq!(resolved(&other, "src/b.ts", "./gen"), "generated/gen.ts");
    assert_eq!(
        resolved(&other, "src/b.ts", "fromtyperoot"),
        "types/fromtyperoot/index.d.ts"
    );
    assert!(file(&other, "types/fromtyperoot/index.d.ts").1.search);
}

#[test]
fn directory_options_take_one_form_and_match_whole_components() {
    // Trailing separators in the configuration change nothing, and `src`
    // does not contain `src-other`.
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "rootDirs": ["src/", "./gen//"], "typeRoots": ["./types/"] }, "files": ["src/a.ts", "src-other/b.ts"] }"#,
        ),
        ("src/a.ts", "import \"./x\";\nimport \"t\";\n"),
        ("src-other/b.ts", "import \"./y\";\n"),
        ("gen/x.ts", ""),
        ("gen/y.ts", ""),
        ("types/t/index.d.ts", ""),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(resolved(&program, "src/a.ts", "./x"), "gen/x.ts");
    assert_eq!(resolved(&program, "src/a.ts", "t"), "types/t/index.d.ts");
    assert_eq!(resolved(&program, "src-other/b.ts", "./y"), "missing");
}

#[test]
fn output_paths_in_exports_map_to_sources_only_with_a_fixed_root() {
    let project = Project::new(&[
        (
            "package.json",
            r##"{ "name": "selfpkg", "type": "module", "exports": { ".": "./dist/index.js", "./util": { "types": "./dist/util.d.ts", "default": "./dist/util.js" } }, "imports": { "#cfg": "./dist/cfg.js" } }"##,
        ),
        (
            "unrooted.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext", "outDir": "dist" }, "files": ["src/main.ts"] }"#,
        ),
        (
            "rooted.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext", "outDir": "dist", "rootDir": "src" }, "files": ["src/main.ts"] }"#,
        ),
        (
            "src/main.ts",
            "import \"selfpkg\";\nimport \"selfpkg/util\";\nimport \"#cfg\";\n",
        ),
        ("src/index.ts", ""),
        ("src/util.ts", ""),
        ("src/cfg.ts", ""),
    ]);
    let unrooted = project.load("unrooted.json");
    for specifier in ["selfpkg", "selfpkg/util", "#cfg"] {
        assert_eq!(resolved(&unrooted, "src/main.ts", specifier), "missing");
    }
    let rooted = project.load("rooted.json");
    assert_eq!(resolved(&rooted, "src/main.ts", "selfpkg"), "src/index.ts");
    assert_eq!(
        resolved(&rooted, "src/main.ts", "selfpkg/util"),
        "src/util.ts"
    );
    assert_eq!(resolved(&rooted, "src/main.ts", "#cfg"), "src/cfg.ts");
}

#[test]
fn search_files_exclude_external_library_dependencies() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "paths": { "@v/*": ["./vendor/*"], "@nm/*": ["./node_modules/*"] }, "typeRoots": ["./types", "./node_modules/@types"], "types": ["localtype", "nmtype"] }, "include": ["src"] }"#,
        ),
        (
            "src/a.ts",
            "import \"pkg\";\nimport \"../node_modules/explicit/index\";\nimport \"@v/v\";\nimport \"@nm/viapaths/index\";\nimport \"./shared\";\nimport \"../excluded/e\";\n",
        ),
        ("src/shared.ts", ""),
        ("excluded/e.ts", ""),
        (
            "node_modules/pkg/package.json",
            r#"{ "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/pkg/index.d.ts",
            "/// <reference path=\"./refd.d.ts\" />\nimport \"./sib\";\nimport \"../../src/shared\";\nimport \"../../outside/o\";\n",
        ),
        ("node_modules/pkg/sib.d.ts", ""),
        ("node_modules/pkg/refd.d.ts", ""),
        ("outside/o.ts", ""),
        ("node_modules/explicit/index.d.ts", ""),
        ("vendor/v.ts", ""),
        ("node_modules/viapaths/index.d.ts", ""),
        (
            "types/localtype/index.d.ts",
            "import \"./more\";\ndeclare const lt: 1;",
        ),
        ("types/localtype/more.d.ts", ""),
        (
            "node_modules/@types/nmtype/index.d.ts",
            "declare const nt: 1;",
        ),
        (
            "node_modules/@types/unlisted/index.d.ts",
            "declare const un: 1;",
        ),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(
        sorted(inputs(&program)),
        [
            "excluded/e.ts",
            "node_modules/@types/nmtype/index.d.ts",
            "node_modules/explicit/index.d.ts",
            "node_modules/pkg/index.d.ts",
            "node_modules/pkg/refd.d.ts",
            "node_modules/pkg/sib.d.ts",
            "node_modules/viapaths/index.d.ts",
            "outside/o.ts",
            "src/a.ts",
            "src/shared.ts",
            "types/localtype/index.d.ts",
            "types/localtype/more.d.ts",
            "vendor/v.ts",
        ]
    );
    assert_eq!(
        searched(&program),
        [
            "excluded/e.ts",
            "src/a.ts",
            "src/shared.ts",
            "types/localtype/index.d.ts",
            "types/localtype/more.d.ts",
            "vendor/v.ts",
        ]
    );
}

#[test]
fn type_packages_follow_types_and_type_roots() {
    let project = Project::new(&[
        (
            "proj/none.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts"] }"#,
        ),
        (
            "proj/star.json",
            r#"{ "compilerOptions": { "noLib": true, "types": ["*"] }, "files": ["a.ts"] }"#,
        ),
        (
            "proj/listed.json",
            r#"{ "compilerOptions": { "noLib": true, "types": ["alpha", "plainpkg", "exppkg", "@scope/beta"] }, "files": ["a.ts"] }"#,
        ),
        (
            "proj/nodenext.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext", "types": ["exppkg"] }, "files": ["a.ts"] }"#,
        ),
        (
            "proj/roots.json",
            r#"{ "compilerOptions": { "noLib": true, "typeRoots": ["./mytypes"], "types": ["*"] }, "files": ["a.ts"] }"#,
        ),
        (
            "proj/custom.json",
            r#"{ "compilerOptions": { "noLib": true, "typeRoots": ["./mytypes"], "types": ["local", "alpha"] }, "files": ["a.ts"] }"#,
        ),
        (
            "proj/missing.json",
            r#"{ "compilerOptions": { "noLib": true, "types": ["nonexistent"] }, "files": ["a.ts"] }"#,
        ),
        ("proj/a.ts", "export {}"),
        ("proj/node_modules/@types/alpha/index.d.ts", ""),
        ("proj/node_modules/@types/scope__beta/index.d.ts", ""),
        (
            "proj/node_modules/@types/notneeded/package.json",
            r#"{ "typings": null }"#,
        ),
        ("proj/node_modules/@types/notneeded/index.d.ts", ""),
        ("proj/node_modules/@types/.hidden/index.d.ts", ""),
        ("node_modules/@types/ancestor/index.d.ts", ""),
        (
            "proj/node_modules/plainpkg/package.json",
            r#"{ "types": "p.d.ts" }"#,
        ),
        ("proj/node_modules/plainpkg/p.d.ts", ""),
        (
            "proj/node_modules/exppkg/package.json",
            r#"{ "exports": { ".": { "types": { "import": "./i.d.ts", "require": "./r.d.ts" } } } }"#,
        ),
        ("proj/node_modules/exppkg/i.d.ts", ""),
        ("proj/node_modules/exppkg/r.d.ts", ""),
        ("proj/mytypes/local/index.d.ts", ""),
    ]);
    assert_eq!(inputs(&project.load("proj/none.json")), ["a.ts"]);
    assert_eq!(
        sorted(inputs(&project.load("proj/star.json"))),
        [
            "../node_modules/@types/ancestor/index.d.ts",
            "a.ts",
            "node_modules/@types/alpha/index.d.ts",
            "node_modules/@types/scope__beta/index.d.ts",
        ]
    );
    assert_eq!(
        sorted(inputs(&project.load("proj/listed.json"))),
        [
            "a.ts",
            "node_modules/@types/alpha/index.d.ts",
            "node_modules/@types/scope__beta/index.d.ts",
            "node_modules/exppkg/i.d.ts",
            "node_modules/plainpkg/p.d.ts",
        ]
    );
    assert_eq!(
        sorted(inputs(&project.load("proj/nodenext.json"))),
        ["a.ts", "node_modules/exppkg/r.d.ts"]
    );
    let roots = project.load("proj/roots.json");
    assert_eq!(sorted(inputs(&roots)), ["a.ts", "mytypes/local/index.d.ts"]);
    assert_eq!(searched(&roots), ["a.ts", "mytypes/local/index.d.ts"]);
    let custom = project.error("proj/custom.json");
    assert_eq!(
        (custom.kind, custom.position),
        (InputErrorKind::UnresolvedImport, position(1, 86))
    );
    assert_eq!(
        project.error("proj/missing.json").kind,
        InputErrorKind::UnresolvedImport
    );
}

#[test]
fn reference_directives_add_inputs() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts"] }"#,
        ),
        (
            "libs.json",
            r#"{ "compilerOptions": { "lib": [] }, "files": ["lib.ts"] }"#,
        ),
        (
            "nolib.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["lib.ts"] }"#,
        ),
        (
            "missing.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["missing.ts"] }"#,
        ),
        (
            "missingtype.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["missingtype.ts"] }"#,
        ),
        (
            "js.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["js.ts"] }"#,
        ),
        (
            "a.ts",
            "/// <reference path=\"refs/one.ts\" />\n/// <reference path=\"refs/two\" />\n/// <reference types=\"viaref\" />\n/// <reference no-default-lib=\"true\" />\n/// <reference path=\"./node_modules/foo/index.d.ts\" />\nexport {}\n",
        ),
        ("refs/one.ts", ""),
        ("refs/two.d.ts", ""),
        ("node_modules/@types/viaref/index.d.ts", ""),
        ("node_modules/foo/index.d.ts", "import \"./dep\";\n"),
        ("node_modules/foo/dep.d.ts", ""),
        ("lib.ts", "/// <reference lib=\"es2015.promise\" />\n"),
        ("missing.ts", "\n/// <reference path=\"./gone.ts\" />\n"),
        ("missingtype.ts", "/// <reference types=\"gone\" />\n"),
        ("js.ts", "/// <reference path=\"./x.js\" />\n"),
        ("x.js", ""),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(
        sorted(inputs(&program)),
        [
            "a.ts",
            "node_modules/@types/viaref/index.d.ts",
            "node_modules/foo/dep.d.ts",
            "node_modules/foo/index.d.ts",
            "refs/one.ts",
            "refs/two.d.ts"
        ]
    );
    // A referenced file keeps its referrer's status; its own imports inside
    // `node_modules` are external.
    assert_eq!(
        searched(&program),
        [
            "a.ts",
            "node_modules/foo/index.d.ts",
            "refs/one.ts",
            "refs/two.d.ts"
        ]
    );
    // A library reference adds a library, which is not searched.
    let libs = project.load("libs.json");
    assert_eq!(listed(&libs), ["lib.es2015.promise.d.ts", "lib.ts"]);
    assert_eq!(searched(&libs), ["lib.ts"]);
    assert_eq!(inputs(&project.load("nolib.json")), ["lib.ts"]);
    let missing = project.error("missing.json");
    assert_eq!(
        (missing.kind, missing.position),
        (InputErrorKind::UnresolvedImport, position(2, 22))
    );
    assert_eq!(
        project.display(missing.path.as_ref().unwrap()),
        "missing.ts"
    );
    assert_eq!(
        project.error("missingtype.json").kind,
        InputErrorKind::UnresolvedImport
    );
    assert_eq!(project.error("js.json").kind, InputErrorKind::Unsupported);
}

/// Each outcome is TypeScript 7.0.2's (`fixtures/json-probes/inputs.json`,
/// cases `ref-json-on`, `ref-json-off`, `ref-json-missing`,
/// `json-ref-directive`, and `json-ref-missing`): a
/// `/// <reference path>` names a JSON input under `resolveJsonModule`, and
/// a JSON file's own references are followed, but TypeScript checks no JSON
/// file (`canIncludeBindAndCheckDiagnostics`), so a missing target there is
/// never reported.
#[test]
fn reference_paths_reach_json_inputs_and_json_files_reference_others() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts"] }"#,
        ),
        (
            "off.json",
            r#"{ "compilerOptions": { "noLib": true, "resolveJsonModule": false }, "files": ["a.ts"] }"#,
        ),
        (
            "missing.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["missing.ts"] }"#,
        ),
        (
            "inside.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["b.ts"] }"#,
        ),
        (
            "a.ts",
            "/// <reference path=\"./data.json\" />\nexport {}\n",
        ),
        (
            "missing.ts",
            "/// <reference path=\"./nope.json\" />\nexport {}\n",
        ),
        ("b.ts", "import data from \"./refs.json\";\ndata;\n"),
        ("data.json", "{ \"a\": 1 }"),
        (
            "refs.json",
            "/// <reference path=\"./extra.ts\" />\n/// <reference path=\"./gone.ts\" />\n{ \"a\": 1 }",
        ),
        ("extra.ts", "export {}\n"),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(inputs(&program), ["data.json", "a.ts"]);
    assert_eq!(searched(&program), ["a.ts", "data.json"]);
    let off = project.error("off.json");
    assert_eq!(
        (off.kind, off.message.as_str()),
        (
            InputErrorKind::Unsupported,
            "Referenced file './data.json' has an unsupported extension"
        )
    );
    let missing = project.error("missing.json");
    assert_eq!(
        (missing.kind, missing.position),
        (InputErrorKind::UnresolvedImport, position(1, 22))
    );
    let inside = project.load("inside.json");
    assert_eq!(sorted(inputs(&inside)), ["b.ts", "extra.ts", "refs.json"]);
    assert_eq!(searched(&inside), ["b.ts", "extra.ts", "refs.json"]);
}

/// `GetResolveJsonModule` in TypeScript 7.0.2: an explicit value, else on
/// for `module` `node20` and `nodenext` and off for `node16` and `node18`
/// (which imply `node16` resolution), else on exactly with bundler
/// resolution. Recorded in `fixtures/json-probes/inputs.json`, cases
/// `node16-default`, `node18-default`, `node20-default`, `nodenext-default`,
/// `commonjs-default`, `preserve-default`, `bundler-off`,
/// `node16-explicit-true`, `node16-resolution-nodenext`, and
/// `node20-explicit-false`; off, `./data.json` is TS2732.
#[test]
fn resolve_json_module_follows_the_compilers_defaults() {
    let cases = [
        (r#""module": "esnext""#, true),
        (r#""module": "commonjs""#, true),
        (r#""module": "preserve""#, true),
        ("", true),
        (r#""module": "node16""#, false),
        (r#""module": "node18""#, false),
        (r#""module": "node20""#, true),
        (r#""module": "nodenext""#, true),
        (r#""module": "node16", "resolveJsonModule": true"#, true),
        (
            r#""module": "node16", "moduleResolution": "nodenext""#,
            false,
        ),
        (r#""module": "node20", "resolveJsonModule": false"#, false),
        (r#""module": "esnext", "resolveJsonModule": false"#, false),
    ];
    let mut files: Vec<(String, String)> = cases
        .iter()
        .enumerate()
        .map(|(index, (options, _))| {
            let options = if options.is_empty() {
                r#""noLib": true"#.to_owned()
            } else {
                format!(r#""noLib": true, {options}"#)
            };
            (
                format!("{index}.json"),
                format!(r#"{{ "compilerOptions": {{ {options} }}, "files": ["a.ts"] }}"#),
            )
        })
        .collect();
    files.push((
        "a.ts".into(),
        "import data from \"./data.json\";\ndata;\n".into(),
    ));
    files.push(("data.json".into(), "{ \"a\": 1 }".into()));
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let project = Project::new(&files);
    for (index, (options, json)) in cases.iter().enumerate() {
        let program = project.load(&format!("{index}.json"));
        assert_eq!(
            resolved(&program, "a.ts", "./data.json"),
            if *json { "data.json" } else { "missing" },
            "{options}"
        );
    }
}

/// TypeScript 7.0.2's resolver for JSON (`tryAddingExtensions`): a `.json`
/// specifier tries `<stem>.d.json.ts`, then `<stem>.json` under
/// `resolveJsonModule`, then, outside ESM resolution, the name plus
/// `.ts`, `.tsx`, `.d.ts`; an extensionless name never adds `.json`, and the
/// `.json` test is exact. JSON in `node_modules` is an external library.
/// A `<stem>.d.json.ts` found first is an arbitrary-extension declaration,
/// which the program does not admit for a source file without
/// `allowArbitraryExtensions` (TS6263), so the request is unresolved rather
/// than resolved to the JSON file beside it. Recorded in
/// `fixtures/json-probes/inputs.json`, cases `json-dts-sibling`,
/// `dts-json-wins`, `json-ts-sibling`, `extensionless`, `upper-specifier`,
/// `paths-json`, `pkg-exports-json`, `pkg-main-json`, `pkg-json-subpath`,
/// `pkg-exports-json-off`, `dep-json`, and `dep-json-local`.
#[test]
fn json_specifiers_follow_the_compilers_extension_order() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "paths": { "@d": ["./one/data.json"] } }, "files": ["a.ts"] }"#,
        ),
        (
            "off.json",
            r#"{ "compilerOptions": { "noLib": true, "resolveJsonModule": false }, "files": ["a.ts"] }"#,
        ),
        (
            "a.ts",
            "import \"./one/data.json\";\nimport \"./two/data.json\";\nimport \"./three/data.json\";\nimport \"./one/data\";\nimport \"./four/DATA.JSON\";\nimport \"@d\";\nimport \"exp/data\";\nimport \"main\";\nimport \"sub/data.json\";\nimport \"dep\";\nimport \"./local\";\n",
        ),
        ("one/data.json", "{}"),
        ("one/data.json.d.ts", "export {};\n"),
        ("two/data.json", "{}"),
        ("two/data.d.json.ts", "export {};\n"),
        ("three/data.json", "{}"),
        ("three/data.json.ts", "export {};\n"),
        ("four/data.json", "{}"),
        (
            "node_modules/exp/package.json",
            r#"{ "name": "exp", "exports": { "./data": "./data.json" } }"#,
        ),
        ("node_modules/exp/data.json", "{}"),
        (
            "node_modules/main/package.json",
            r#"{ "name": "main", "main": "data.json" }"#,
        ),
        ("node_modules/main/data.json", "{}"),
        ("node_modules/sub/package.json", r#"{ "name": "sub" }"#),
        ("node_modules/sub/data.json", "{}"),
        (
            "node_modules/dep/package.json",
            r#"{ "name": "dep", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/dep/index.d.ts",
            "import data from \"./data.json\";\nexport declare const x: typeof data;\n",
        ),
        ("node_modules/dep/data.json", "{}"),
        (
            "local.ts",
            "import data from \"./local.json\";\nexport const x = data;\n",
        ),
        ("local.json", "{}"),
    ]);
    let program = project.load("tsconfig.json");
    for (specifier, expected) in [
        ("./one/data.json", "one/data.json"),
        ("./two/data.json", "not admitted"),
        ("./three/data.json", "three/data.json"),
        ("./one/data", "missing"),
        ("./four/DATA.JSON", "missing"),
        ("@d", "one/data.json"),
        ("exp/data", "node_modules/exp/data.json"),
        ("main", "node_modules/main/data.json"),
        ("sub/data.json", "node_modules/sub/data.json"),
    ] {
        assert_eq!(
            resolved(&program, "a.ts", specifier),
            expected,
            "{specifier}"
        );
    }
    assert_eq!(
        resolved(&program, "node_modules/dep/index.d.ts", "./data.json"),
        "node_modules/dep/data.json"
    );
    assert_eq!(
        searched(&program),
        [
            "a.ts",
            "local.json",
            "local.ts",
            "one/data.json",
            "three/data.json",
        ]
    );
    let off = project.load("off.json");
    for (specifier, expected) in [
        ("./one/data.json", "one/data.json.d.ts"),
        ("./two/data.json", "not admitted"),
        ("./three/data.json", "three/data.json.ts"),
        ("exp/data", "missing"),
        ("main", "missing"),
        ("sub/data.json", "missing"),
    ] {
        assert_eq!(resolved(&off, "a.ts", specifier), expected, "{specifier}");
    }
}

/// TypeScript 7.0.2 converts every occurrence of a recognized option as it
/// sets it (`onPropertySet`), whichever one applies: through the `extends`
/// chain, every `compilerOptions` object of a file, and each repeated key,
/// of which the last that sets the option applies. `extends` may not be
/// `null` (TS5024). Recorded in `fixtures/json-probes/configuration.json`,
/// cases `cfg-dup-bad-first`, `cfg-dup-bad-type-first`,
/// `cfg-dup-lib-bad-first`, `cfg-dup-removed-first`,
/// `cfg-dup-compiler-options`, `cfg-base-bad-overridden`,
/// `cfg-dup-files-bad-first`, `cfg-root-dup-include-bad-first`,
/// `cfg-references-dup`, `cfg-dup-paths-bad-first`,
/// `cfg-dup-extends-missing-first`, and `cfg-lone-surrogate-shadowed`, and in
/// `fixtures/json-probes/config-merge.json`, case `extends-null-after-path`;
/// the cases there pin the rest of the rule end to end.
#[test]
fn configuration_validates_every_occurrence_and_applies_the_last() {
    let with = |body: &str| format!("{{ {body}, \"files\": [\"a.ts\", \"c.ts\"] }}");
    let configs = [
        ("badfirst.json", with(r#""compilerOptions": { "noLib": true, "module": "bogus", "module": "esnext" }"#)),
        ("badtype.json", with(r#""compilerOptions": { "noLib": true, "allowJs": "yes", "allowJs": false }"#)),
        ("badlib.json", with(r#""compilerOptions": { "lib": ["nope"], "lib": ["es5"] }"#)),
        ("removedfirst.json", with(r#""compilerOptions": { "noLib": true, "module": "amd", "module": "esnext" }"#)),
        ("merged.json", with(r#""compilerOptions": { "noLib": true }, "compilerOptions": { "types": [] }"#)),
        ("lastwins.json", with(r#""compilerOptions": { "noLib": false, "noLib": true }"#)),
        ("overridden.json", with(r#""extends": "./bogusbase.json", "compilerOptions": { "module": "esnext" }"#)),
        ("filesfirst.json", r#"{ "compilerOptions": { "noLib": true }, "files": [1], "files": ["a.ts", "c.ts"] }"#.into()),
        ("includefirst.json", r#"{ "compilerOptions": { "noLib": true }, "include": "x", "include": ["a.ts", "c.ts"] }"#.into()),
        ("referencesfirst.json", with(r#""compilerOptions": { "noLib": true }, "references": 5, "references": []"#)),
        ("pathsfirst.json", with(r#""compilerOptions": { "noLib": true, "paths": 5, "paths": { "@x": ["./a.ts"] } }"#)),
        ("paths.json", with(r#""compilerOptions": { "noLib": true, "paths": { "@x": ["./b.ts"], "@y": ["./b.ts"], "@x": ["./a.ts"] } }"#)),
        ("extendsfirst.json", with(r#""extends": "./nope", "extends": "./nolib.json""#)),
        ("extendsjsonfirst.json", with(r#""extends": "./nope.json", "extends": "./nolib.json""#)),
        ("extendsnull.json", with(r#""extends": "./es5base.json", "extends": null, "compilerOptions": { "noLib": true }"#)),
        ("shadowedsurrogate.json", with(r#""compilerOptions": { "noLib": true, "jsxImportSource": "\ud800", "jsxImportSource": "react" }"#)),
        ("surrogate.json", with(r#""compilerOptions": { "noLib": true, "jsxImportSource": "react", "jsxImportSource": "\ud800" }"#)),
        ("pathsurrogate.json", with(r#""compilerOptions": { "noLib": true, "paths": { "\ud83d*": ["./a.ts"] } }"#)),
    ];
    let mut files: Vec<(&str, &str)> = configs
        .iter()
        .map(|(name, text)| (*name, text.as_str()))
        .collect();
    files.extend([
        ("a.ts", "export {}\n"),
        ("b.ts", "export {}\n"),
        ("c.ts", "import \"@x\";\nexport {}\n"),
        (
            "bogusbase.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "bogus" } }"#,
        ),
        (
            "es5base.json",
            r#"{ "compilerOptions": { "lib": ["es5"] } }"#,
        ),
        ("nolib.json", NO_LIB),
    ]);
    let project = Project::new(&files);
    for (config, at, kind) in [
        ("badfirst.json", "\"bogus", InputErrorKind::Config),
        ("badtype.json", "\"yes", InputErrorKind::Config),
        ("badlib.json", "\"nope", InputErrorKind::Config),
        ("filesfirst.json", "1]", InputErrorKind::Config),
        ("includefirst.json", "\"x", InputErrorKind::Config),
        ("referencesfirst.json", "5", InputErrorKind::Config),
        ("pathsfirst.json", "5", InputErrorKind::Config),
        ("extendsfirst.json", "\"./nope\"", InputErrorKind::Config),
        ("extendsnull.json", "null", InputErrorKind::Config),
        ("surrogate.json", "\"\\ud800", InputErrorKind::Unsupported),
        (
            "pathsurrogate.json",
            "\"\\ud83d",
            InputErrorKind::Unsupported,
        ),
    ] {
        let text = &configs.iter().find(|(name, _)| *name == config).unwrap().1;
        let error = project.error(config);
        let offset = text.find(at).unwrap();
        let line = text[..offset].matches('\n').count() + 1;
        assert_eq!(
            (error.kind, error.position),
            (kind, position(line, offset + 1)),
            "{config}: {error}"
        );
    }
    let overridden = project.error("overridden.json");
    assert_eq!(overridden.kind, InputErrorKind::Config);
    assert_eq!(
        project.display(overridden.path.as_ref().unwrap()),
        "bogusbase.json"
    );
    for config in [
        "removedfirst.json",
        "merged.json",
        "lastwins.json",
        "extendsjsonfirst.json",
        "shadowedsurrogate.json",
    ] {
        let program = project.load(config);
        // `noLib` applies, from whichever occurrence set it last.
        assert_eq!(listed(&program), ["a.ts", "c.ts"], "{config}");
    }
    let paths = project.load("paths.json");
    assert_eq!(resolved(&paths, "c.ts", "@x"), "a.ts");
}

#[test]
fn module_detection_sets_file_options() {
    let project = Project::new(&[
        ("plain/v_ts.ts", "var v_ts: number;"),
        ("plain/v_mts.mts", "var v_mts: number;"),
        ("plain/v_cts.cts", "var v_cts: number;"),
        ("plain/v_dts.d.ts", "declare var v_dts: number;"),
        ("plain/v_dmts.d.mts", "declare var v_dmts: number;"),
        ("plain/v_tsx.tsx", "var v_tsx: number; const el = <div />;"),
        ("plain/v_tsxnojsx.tsx", "var v_tsxnojsx: number;"),
        ("esm/package.json", r#"{ "type": "module" }"#),
        ("esm/v_esm.ts", "var v_esm: number;"),
        ("esm/v_esmd.d.ts", "declare var v_esmd: number;"),
        (
            "bundler.json",
            r#"{ "compilerOptions": { "noLib": true, "jsx": "preserve" } }"#,
        ),
        (
            "force.json",
            r#"{ "compilerOptions": { "noLib": true, "moduleDetection": "force" } }"#,
        ),
        (
            "legacy.json",
            r#"{ "compilerOptions": { "noLib": true, "moduleDetection": "legacy", "jsx": "react-jsx" } }"#,
        ),
        (
            "reactjsx.json",
            r#"{ "compilerOptions": { "noLib": true, "jsx": "react-jsx" } }"#,
        ),
        (
            "nodenext.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext" } }"#,
        ),
        (
            "nodenext_auto.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext", "moduleDetection": "auto" } }"#,
        ),
    ]);
    let modules = |config: &str| -> Vec<String> {
        let program = project.load(config);
        let mut names: Vec<String> = program
            .sources
            .ids()
            .filter(|id| program.files[id.index()].facts.module)
            .map(|id| {
                program
                    .sources
                    .get(id)
                    .display
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        names.sort();
        names
    };
    assert_eq!(modules("bundler.json"), ["v_cts.cts", "v_mts.mts"]);
    assert_eq!(
        modules("force.json"),
        [
            "v_cts.cts",
            "v_esm.ts",
            "v_mts.mts",
            "v_ts.ts",
            "v_tsx.tsx",
            "v_tsxnojsx.tsx"
        ]
    );
    assert!(modules("legacy.json").is_empty());
    assert_eq!(
        modules("reactjsx.json"),
        ["v_cts.cts", "v_mts.mts", "v_tsx.tsx"]
    );
    assert_eq!(
        modules("nodenext.json"),
        [
            "v_cts.cts",
            "v_esm.ts",
            "v_mts.mts",
            "v_ts.ts",
            "v_tsx.tsx",
            "v_tsxnojsx.tsx"
        ]
    );
    assert_eq!(
        modules("nodenext_auto.json"),
        ["v_cts.cts", "v_esm.ts", "v_mts.mts"]
    );
}

#[cfg(unix)]
#[test]
fn symlinks_share_real_identity_unless_preserved() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["src/a.ts"] }"#,
        ),
        (
            "preserve.json",
            r#"{ "compilerOptions": { "noLib": true, "preserveSymlinks": true }, "files": ["src/a.ts"] }"#,
        ),
        (
            "src/a.ts",
            "import \"./link/target\";\nimport \"./real/target\";\nimport \"lib\";\n",
        ),
        ("src/real/target.ts", ""),
        (
            "packages/lib/package.json",
            r#"{ "name": "lib", "types": "index.d.ts" }"#,
        ),
        ("packages/lib/index.d.ts", "import \"dep\";\n"),
        ("packages/lib/node_modules/dep/index.d.ts", ""),
    ]);
    project.link("src/link", "real");
    project.link("node_modules/lib", "../packages/lib");
    let program = project.load("tsconfig.json");
    assert_eq!(
        resolved(&program, "src/a.ts", "./link/target"),
        "src/real/target.ts"
    );
    assert_eq!(
        resolved(&program, "src/a.ts", "lib"),
        "packages/lib/index.d.ts"
    );
    assert_eq!(
        sorted(inputs(&program)),
        [
            "packages/lib/index.d.ts",
            "packages/lib/node_modules/dep/index.d.ts",
            "src/a.ts",
            "src/real/target.ts"
        ]
    );
    assert_eq!(searched(&program), ["src/a.ts", "src/real/target.ts"]);
    let preserved = project.load("preserve.json");
    assert_eq!(
        sorted(inputs(&preserved)),
        [
            "node_modules/lib/index.d.ts",
            "node_modules/lib/node_modules/dep/index.d.ts",
            "src/a.ts",
            "src/link/target.ts",
            "src/real/target.ts",
        ]
    );
}

#[test]
fn syntax_errors_and_display_paths_are_reported_relative_to_the_configuration() {
    let project = Project::new(&[
        (
            "proj/tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["a.ts", "../shared/*.ts"] }"#,
        ),
        ("proj/a.ts", "export {}"),
        ("shared/s.ts", "export {}"),
        (
            "proj/bad.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["bad.ts"] }"#,
        ),
        ("proj/bad.ts", "const ok = 1;\nconst = ;\n"),
    ]);
    let program = project.load("proj/tsconfig.json");
    assert_eq!(inputs(&program), ["a.ts", "../shared/s.ts"]);
    let error = project.error("proj/bad.json");
    assert_eq!(
        (error.kind, error.position),
        (InputErrorKind::Syntax, position(2, 7))
    );
    assert_eq!(project.display(error.path.as_ref().unwrap()), "proj/bad.ts");
}

/// A missing reference directive target is reported only in a project file
/// (`Diagnostics::reports`): a package's directives may name absent inputs
/// with or without skipLibCheck, while the same directive in a root fails.
#[test]
fn package_reference_directives_may_name_absent_inputs() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true }, "files": ["app.ts"] }"#,
        ),
        (
            "checked.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["app.ts"] }"#,
        ),
        (
            "root.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["root.ts"] }"#,
        ),
        ("app.ts", "import { tool } from \"tool\";\n"),
        (
            "root.ts",
            "/// <reference types=\"absent\" />\nexport {};\n",
        ),
        (
            "node_modules/tool/package.json",
            r#"{ "name": "tool", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/tool/index.d.ts",
            "/// <reference types=\"absent\" />\n/// <reference path=\"./absent.d.ts\" />\nexport declare const tool: number;\n",
        ),
    ]);
    for config in ["tsconfig.json", "checked.json"] {
        let program = project.load(config);
        assert_eq!(
            inputs(&program),
            ["node_modules/tool/index.d.ts", "app.ts"],
            "{config}"
        );
    }
    let error = project.error("root.json");
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
    assert_eq!(error.position, position(1, 23));
}

#[test]
fn oversized_version_ranges_do_not_match_or_overflow() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["app.ts"] }"#,
        ),
        (
            "app.ts",
            "import { x } from \"pkg\";\nimport { y } from \"cond\";\n",
        ),
        (
            "node_modules/pkg/package.json",
            r#"{ "types": "index.d.ts", "typesVersions": { "^18446744073709551615": { "*": ["alt/*"] }, "~99999999999999999999999.18446744073709551615.18446744073709551615": { "*": ["alt/*"] } } }"#,
        ),
        (
            "node_modules/pkg/index.d.ts",
            "export declare const x: number;\n",
        ),
        (
            "node_modules/cond/package.json",
            r#"{ "exports": { ".": { "types@^18446744073709551615": "./alt.d.ts", "types": "./index.d.ts" } } }"#,
        ),
        (
            "node_modules/cond/index.d.ts",
            "export declare const y: number;\n",
        ),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(
        resolved(&program, "app.ts", "pkg"),
        "node_modules/pkg/index.d.ts"
    );
    assert_eq!(
        resolved(&program, "app.ts", "cond"),
        "node_modules/cond/index.d.ts"
    );
}

#[test]
fn shared_extends_bases_are_evaluated_once_and_reapplied_in_order() {
    let mut files: Vec<(String, String)> = vec![
        ("app.ts".into(), "export const x = 1;\n".into()),
        ("d.ts".into(), "export const d = 1;\n".into()),
        ("b.ts".into(), "export const b = 1;\n".into()),
        (
            "base0.json".into(),
            r#"{ "compilerOptions": { "noLib": true }, "files": ["app.ts"] }"#.into(),
        ),
    ];
    for level in 1..24 {
        files.push((
            format!("base{level}.json"),
            format!(
                r#"{{ "extends": ["./base{0}.json", "./base{0}.json"] }}"#,
                level - 1
            ),
        ));
    }
    files.push((
        "tsconfig.json".into(),
        r#"{ "extends": "./base23.json" }"#.into(),
    ));
    // `d` is reapplied after `b`, so its `files` win, as in TypeScript.
    files.push((
        "diamond-d.json".into(),
        r#"{ "compilerOptions": { "noLib": true }, "files": ["d.ts"] }"#.into(),
    ));
    files.push((
        "diamond-b.json".into(),
        r#"{ "extends": "./diamond-d.json", "files": ["b.ts"] }"#.into(),
    ));
    files.push((
        "diamond-c.json".into(),
        r#"{ "extends": "./diamond-d.json" }"#.into(),
    ));
    files.push((
        "diamond.json".into(),
        r#"{ "extends": ["./diamond-b.json", "./diamond-c.json"] }"#.into(),
    ));
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let project = Project::new(&borrowed);
    let start = std::time::Instant::now();
    assert_eq!(inputs(&project.load("tsconfig.json")), ["app.ts"]);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
    assert_eq!(inputs(&project.load("diamond.json")), ["d.ts"]);
}

#[test]
fn package_targets_fall_back_past_missing_candidates_but_not_null() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["app.ts"] }"#,
        ),
        (
            "package.json",
            r##"{ "imports": { "#x": ["./no.ts", "./yes.ts"] } }"##,
        ),
        ("yes.ts", "export const x = 1;\n"),
        (
            "app.ts",
            "import \"arrays\";\nimport \"conditions\";\nimport \"nested\";\nimport \"blocked\";\nimport \"#x\";\n",
        ),
        (
            "node_modules/arrays/package.json",
            r#"{ "exports": { ".": ["./missing.d.ts", "./good.d.ts"] } }"#,
        ),
        ("node_modules/arrays/good.d.ts", ""),
        (
            "node_modules/conditions/package.json",
            r#"{ "exports": { ".": { "types": "./missing.d.ts", "default": "./good.d.ts" } } }"#,
        ),
        ("node_modules/conditions/good.d.ts", ""),
        (
            "node_modules/nested/package.json",
            r#"{ "exports": { ".": [{ "types": "./missing.d.ts" }, { "default": ["./missing.d.ts", "./good.d.ts"] }] } }"#,
        ),
        ("node_modules/nested/good.d.ts", ""),
        (
            "node_modules/blocked/package.json",
            r#"{ "exports": { ".": [null, "./good.d.ts"] } }"#,
        ),
        ("node_modules/blocked/good.d.ts", ""),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(
        resolved(&program, "app.ts", "arrays"),
        "node_modules/arrays/good.d.ts"
    );
    assert_eq!(
        resolved(&program, "app.ts", "conditions"),
        "node_modules/conditions/good.d.ts"
    );
    assert_eq!(
        resolved(&program, "app.ts", "nested"),
        "node_modules/nested/good.d.ts"
    );
    assert_eq!(resolved(&program, "app.ts", "blocked"), "missing");
    assert_eq!(resolved(&program, "app.ts", "#x"), "yes.ts");
}

#[test]
fn long_imports_chains_resolve_and_cycles_end() {
    // The longest chain within the limit.
    let length = resolve::MAX_IMPORTS_CHAIN - 1;
    let mut imports: Vec<String> = (0..length)
        .map(|i| format!(r##""#a{i}": "#a{}""##, i + 1))
        .collect();
    imports.push(format!(r##""#a{length}": "./lib.ts""##));
    imports.push(r##""#c0": "#c1", "#c1": "#c0""##.into());
    let manifest = format!("{{ \"imports\": {{ {} }} }}", imports.join(", "));
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext" }, "files": ["app.ts"] }"#,
        ),
        ("package.json", &manifest),
        ("lib.ts", "export const x = 1;\n"),
        ("app.ts", "import \"#a0\";\nimport \"#c0\";\n"),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(resolved(&program, "app.ts", "#a0"), "lib.ts");
    assert_eq!(resolved(&program, "app.ts", "#c0"), "missing");
}

#[test]
fn imports_chains_beyond_the_limit_fail_the_load() {
    let limit = resolve::MAX_IMPORTS_CHAIN;
    let mut imports: Vec<String> = (0..=limit)
        .map(|i| format!(r##""#a{i}": "#a{}""##, i + 1))
        .collect();
    imports.push(format!(r##""#a{}": "./lib.ts""##, limit + 1));
    imports.push(r##""#b": "#a2""##.into());
    let manifest = format!("{{ \"imports\": {{ {} }} }}", imports.join(", "));
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext" }, "files": ["app.ts"] }"#,
        ),
        ("package.json", &manifest),
        ("lib.ts", "export const x = 1;\n"),
        ("app.ts", "import \"#a0\";\nimport \"#b\";\n"),
    ]);
    let config = project.root.join("tsconfig.json");
    let error = load_on_parsing_thread(&config).unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
    assert_eq!(error.position, Some(SourcePosition { line: 1, column: 8 }));
}

#[test]
fn module_specifier_capture_does_not_visit_every_importer_target_pair() {
    // 1,200 importers in 120 directories and 300 `index` modules, all
    // imported by one file. Capturing module-specifier probes deduplicates
    // importer directories and relative path shapes; it must never construct
    // candidates per importer file times every target.
    let mut files: Vec<(String, String)> = vec![(
        "tsconfig.json".into(),
        r#"{ "compilerOptions": { "noLib": true }, "include": ["src"] }"#.into(),
    )];
    let mut imports = String::new();
    for i in 0..300 {
        files.push((
            format!("src/lib{i}/index.ts"),
            format!("export const v{i} = {i};\n"),
        ));
        imports.push_str(&format!("import \"./lib{i}\";\n"));
    }
    files.push(("src/main.ts".into(), imports));
    for d in 0..120 {
        for f in 0..10 {
            files.push((
                format!("src/d{d}/f{f}.ts"),
                format!("export const f{d}_{f} = 1;\n"),
            ));
        }
    }
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let project = Project::new(&files);
    let config = project.root.join("tsconfig.json");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let program = load_on_parsing_thread(&config).unwrap();
        send.send(program.sources.ids().count()).unwrap();
    });
    let count = receive
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("loading should finish");
    assert_eq!(count, 1501);
}

#[test]
fn long_extends_chains_load_in_linear_time() {
    let count = 10_000;
    let mut files: Vec<(String, String)> = vec![
        ("app.ts".into(), "export const y = 1;\n".into()),
        (
            "base0.json".into(),
            r#"{ "compilerOptions": { "noLib": true }, "files": ["app.ts"] }"#.into(),
        ),
        (
            "tsconfig.json".into(),
            format!(r#"{{ "extends": "./base{}.json" }}"#, count - 1),
        ),
    ];
    files.extend((1..count).map(|i| {
        (
            format!("base{i}.json"),
            format!(r#"{{ "extends": "./base{}.json" }}"#, i - 1),
        )
    }));
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let project = Project::new(&files);
    // A host may hold the first read of a newly written file, as a file
    // scanner does, for milliseconds per file: a cost of the host, not of
    // loading, which the time limit bounds. Every file is read once first.
    for (name, _) in &files {
        std::fs::read(project.root.join(name)).unwrap();
    }
    let config = project.root.join("tsconfig.json");
    // Linear time is budgeted in the CPU time of the thread that loads,
    // which resolves the whole `extends` chain: elapsed time also counts
    // every wait for a file to open, which a host that inspects each open,
    // or a loaded machine, can make far longer than the loading itself. CPU
    // time counts system time too, so re-reading or re-hashing the chain for
    // each link still exceeds the budget. Elapsed time only guards against
    // a hang.
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let loaded = crate::analysis::on_parsing_thread("tsparse-test-load", |parser| {
            let started = thread_cpu_time();
            let program = load(&config, &super::disk::Disk::default(), parser).unwrap();
            let spent = thread_cpu_time() - started;
            let inputs: Vec<String> = inputs(&program).into_iter().map(str::to_owned).collect();
            (inputs, spent)
        })
        .unwrap();
        send.send(loaded).unwrap();
    });
    let (inputs, spent) = receive
        .recv_timeout(std::time::Duration::from_secs(600))
        .expect("loading should finish");
    assert_eq!(inputs, ["app.ts"]);
    assert!(
        spent < std::time::Duration::from_secs(20),
        "loading took {spent:?} of CPU time"
    );
}

/// The CPU time, user and system, the calling thread has used, which no
/// wait counts (`clock_gettime` with `CLOCK_THREAD_CPUTIME_ID`).
fn thread_cpu_time() -> std::time::Duration {
    use std::ffi::{c_int, c_long};
    /// `struct timespec`, whose fields are `long` on 64-bit macOS and Linux.
    #[repr(C)]
    struct Timespec {
        seconds: c_long,
        nanoseconds: c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: c_int, time: *mut Timespec) -> c_int;
    }
    const CLOCK_THREAD_CPUTIME_ID: c_int = if cfg!(target_os = "macos") { 16 } else { 3 };
    let mut time = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    // SAFETY: `time` is a live, writable `struct timespec`.
    let status = unsafe { clock_gettime(CLOCK_THREAD_CPUTIME_ID, &mut time) };
    assert_eq!(status, 0, "{}", std::io::Error::last_os_error());
    std::time::Duration::new(
        u64::try_from(time.seconds).unwrap(),
        u32::try_from(time.nanoseconds).unwrap(),
    )
}

#[test]
fn branching_imports_cycles_and_shared_failures_end_quickly() {
    // `#x` and the `#p`/`#q` pair offer cycles twice per step. Each `#dN`
    // offers `#dN+1` twice and `#d40` is absent, so evaluating shared
    // failures again would take 2^40 steps.
    let mut imports = vec![
        r##""#x": ["#x", "#x"]"##.to_owned(),
        r##""#p": ["#q", "#q"], "#q": ["#p", "#p"]"##.to_owned(),
    ];
    imports.extend((0..40).map(|i| format!(r##""#d{i}": ["#d{n}", "#d{n}"]"##, n = i + 1)));
    imports.push(r##""#d40": "./absent.ts""##.into());
    imports.push(r##""#ok": ["#d0", "#x", "./lib.ts"]"##.into());
    let manifest = format!("{{ \"imports\": {{ {} }} }}", imports.join(", "));
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext" }, "files": ["app.ts"] }"#,
        ),
        ("package.json", &manifest),
        ("lib.ts", "export const x = 1;\n"),
        (
            "app.ts",
            "import \"#x\";\nimport \"#p\";\nimport \"#d0\";\nimport \"#ok\";\n",
        ),
    ]);
    let config = project.root.join("tsconfig.json");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let program = load_on_parsing_thread(&config).unwrap();
        let results = ["#x", "#p", "#d0", "#ok"].map(|name| resolved(&program, "app.ts", name));
        send.send(results).unwrap();
    });
    let results = receive
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("resolution should finish");
    assert_eq!(results, ["missing", "missing", "missing", "lib.ts"]);
}

// Standard libraries. Each list is the pinned compiler's
// `tsc --listFilesOnly` output for the same configuration, with vendored
// libraries named by file name.

/// The default for an unset, null, or empty `target`: ES2025.
const ES2025_FULL: &str = "lib.es5.d.ts lib.es2015.d.ts lib.es2016.d.ts lib.es2017.d.ts lib.es2018.d.ts lib.es2019.d.ts lib.es2020.d.ts lib.es2021.d.ts lib.es2022.d.ts lib.es2023.d.ts lib.es2024.d.ts lib.es2025.d.ts lib.dom.d.ts lib.dom.iterable.d.ts lib.dom.asynciterable.d.ts lib.webworker.importscripts.d.ts lib.scripthost.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2016.array.include.d.ts lib.es2016.intl.d.ts lib.es2017.arraybuffer.d.ts lib.es2017.date.d.ts lib.es2017.object.d.ts lib.es2017.sharedmemory.d.ts lib.es2017.string.d.ts lib.es2017.intl.d.ts lib.es2017.typedarrays.d.ts lib.es2018.asyncgenerator.d.ts lib.es2018.asynciterable.d.ts lib.es2018.intl.d.ts lib.es2018.promise.d.ts lib.es2018.regexp.d.ts lib.es2019.array.d.ts lib.es2019.object.d.ts lib.es2019.string.d.ts lib.es2019.symbol.d.ts lib.es2019.intl.d.ts lib.es2020.bigint.d.ts lib.es2020.date.d.ts lib.es2020.promise.d.ts lib.es2020.sharedmemory.d.ts lib.es2020.string.d.ts lib.es2020.symbol.wellknown.d.ts lib.es2020.intl.d.ts lib.es2020.number.d.ts lib.es2021.promise.d.ts lib.es2021.string.d.ts lib.es2021.weakref.d.ts lib.es2021.intl.d.ts lib.es2022.array.d.ts lib.es2022.error.d.ts lib.es2022.intl.d.ts lib.es2022.object.d.ts lib.es2022.string.d.ts lib.es2022.regexp.d.ts lib.es2023.array.d.ts lib.es2023.collection.d.ts lib.es2023.intl.d.ts lib.es2024.arraybuffer.d.ts lib.es2024.collection.d.ts lib.es2024.object.d.ts lib.es2024.promise.d.ts lib.es2024.regexp.d.ts lib.es2024.sharedmemory.d.ts lib.es2024.string.d.ts lib.es2025.collection.d.ts lib.es2025.float16.d.ts lib.es2025.intl.d.ts lib.es2025.iterator.d.ts lib.es2025.promise.d.ts lib.es2025.regexp.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts lib.es2025.full.d.ts";
const ES2016_FULL: &str = "lib.es5.d.ts lib.es2015.d.ts lib.es2016.d.ts lib.dom.d.ts lib.dom.iterable.d.ts lib.webworker.importscripts.d.ts lib.scripthost.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2016.array.include.d.ts lib.es2016.intl.d.ts lib.es2018.asynciterable.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts lib.es2016.full.d.ts";
const ES2022_FULL: &str = "lib.es5.d.ts lib.es2015.d.ts lib.es2016.d.ts lib.es2017.d.ts lib.es2018.d.ts lib.es2019.d.ts lib.es2020.d.ts lib.es2021.d.ts lib.es2022.d.ts lib.dom.d.ts lib.dom.iterable.d.ts lib.dom.asynciterable.d.ts lib.webworker.importscripts.d.ts lib.scripthost.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2016.array.include.d.ts lib.es2016.intl.d.ts lib.es2017.arraybuffer.d.ts lib.es2017.date.d.ts lib.es2017.object.d.ts lib.es2017.sharedmemory.d.ts lib.es2017.string.d.ts lib.es2017.intl.d.ts lib.es2017.typedarrays.d.ts lib.es2018.asyncgenerator.d.ts lib.es2018.asynciterable.d.ts lib.es2018.intl.d.ts lib.es2018.promise.d.ts lib.es2018.regexp.d.ts lib.es2019.array.d.ts lib.es2019.object.d.ts lib.es2019.string.d.ts lib.es2019.symbol.d.ts lib.es2019.intl.d.ts lib.es2020.bigint.d.ts lib.es2020.date.d.ts lib.es2020.promise.d.ts lib.es2020.sharedmemory.d.ts lib.es2020.string.d.ts lib.es2020.symbol.wellknown.d.ts lib.es2020.intl.d.ts lib.es2020.number.d.ts lib.es2021.promise.d.ts lib.es2021.string.d.ts lib.es2021.weakref.d.ts lib.es2021.intl.d.ts lib.es2022.array.d.ts lib.es2022.error.d.ts lib.es2022.intl.d.ts lib.es2022.object.d.ts lib.es2022.string.d.ts lib.es2022.regexp.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts lib.es2022.full.d.ts";
const ESNEXT_FULL: &str = "lib.es5.d.ts lib.es2015.d.ts lib.es2016.d.ts lib.es2017.d.ts lib.es2018.d.ts lib.es2019.d.ts lib.es2020.d.ts lib.es2021.d.ts lib.es2022.d.ts lib.es2023.d.ts lib.es2024.d.ts lib.es2025.d.ts lib.esnext.d.ts lib.dom.d.ts lib.dom.iterable.d.ts lib.dom.asynciterable.d.ts lib.webworker.importscripts.d.ts lib.scripthost.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2016.array.include.d.ts lib.es2016.intl.d.ts lib.es2017.arraybuffer.d.ts lib.es2017.date.d.ts lib.es2017.object.d.ts lib.es2017.sharedmemory.d.ts lib.es2017.string.d.ts lib.es2017.intl.d.ts lib.es2017.typedarrays.d.ts lib.es2018.asyncgenerator.d.ts lib.es2018.asynciterable.d.ts lib.es2018.intl.d.ts lib.es2018.promise.d.ts lib.es2018.regexp.d.ts lib.es2019.array.d.ts lib.es2019.object.d.ts lib.es2019.string.d.ts lib.es2019.symbol.d.ts lib.es2019.intl.d.ts lib.es2020.bigint.d.ts lib.es2020.date.d.ts lib.es2020.promise.d.ts lib.es2020.sharedmemory.d.ts lib.es2020.string.d.ts lib.es2020.symbol.wellknown.d.ts lib.es2020.intl.d.ts lib.es2020.number.d.ts lib.es2021.promise.d.ts lib.es2021.string.d.ts lib.es2021.weakref.d.ts lib.es2021.intl.d.ts lib.es2022.array.d.ts lib.es2022.error.d.ts lib.es2022.intl.d.ts lib.es2022.object.d.ts lib.es2022.string.d.ts lib.es2022.regexp.d.ts lib.es2023.array.d.ts lib.es2023.collection.d.ts lib.es2023.intl.d.ts lib.es2024.arraybuffer.d.ts lib.es2024.collection.d.ts lib.es2024.object.d.ts lib.es2024.promise.d.ts lib.es2024.regexp.d.ts lib.es2024.sharedmemory.d.ts lib.es2024.string.d.ts lib.es2025.collection.d.ts lib.es2025.float16.d.ts lib.es2025.intl.d.ts lib.es2025.iterator.d.ts lib.es2025.promise.d.ts lib.es2025.regexp.d.ts lib.esnext.array.d.ts lib.esnext.collection.d.ts lib.esnext.date.d.ts lib.esnext.decorators.d.ts lib.esnext.disposable.d.ts lib.esnext.error.d.ts lib.esnext.intl.d.ts lib.esnext.sharedmemory.d.ts lib.esnext.temporal.d.ts lib.esnext.typedarrays.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts lib.esnext.full.d.ts";
/// `es6` and `es2015` select `lib.es6.d.ts`, which sorts first.
const ES6: &str = "lib.es6.d.ts lib.es5.d.ts lib.es2015.d.ts lib.dom.d.ts lib.dom.iterable.d.ts lib.webworker.importscripts.d.ts lib.scripthost.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2018.asynciterable.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
/// `lib: ["ES2015.Core", "DOM"]`.
const CORE_AND_DOM: &str = "lib.es5.d.ts lib.es2015.d.ts lib.dom.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2018.asynciterable.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
/// `lib: ["es6", "es2015"]`, one file for both names.
const ES2015: &str = "lib.es5.d.ts lib.es2015.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
/// `lib: ["webworker"]`.
const WEBWORKER: &str = "lib.es5.d.ts lib.es2015.d.ts lib.webworker.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2018.asynciterable.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
/// `lib: ["esnext.\u{130}ntl", "WEBWOR\u{212A}ER"]`: a configuration
/// lowers U+0130 to `i` and KELVIN SIGN to `k`.
const WEBWORKER_AND_INTL: &str = "lib.es5.d.ts lib.es2015.d.ts lib.webworker.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2018.asynciterable.d.ts lib.es2018.intl.d.ts lib.es2020.intl.d.ts lib.es2025.intl.d.ts lib.esnext.intl.d.ts lib.esnext.temporal.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
/// Command Center's `lib: ["dom", "dom.iterable", "esnext"]` with
/// `target: ES2022`, which an explicit list overrides.
const DOM_ESNEXT: &str = "lib.es5.d.ts lib.es2015.d.ts lib.es2016.d.ts lib.es2017.d.ts lib.es2018.d.ts lib.es2019.d.ts lib.es2020.d.ts lib.es2021.d.ts lib.es2022.d.ts lib.es2023.d.ts lib.es2024.d.ts lib.es2025.d.ts lib.esnext.d.ts lib.dom.d.ts lib.dom.iterable.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2016.array.include.d.ts lib.es2016.intl.d.ts lib.es2017.arraybuffer.d.ts lib.es2017.date.d.ts lib.es2017.object.d.ts lib.es2017.sharedmemory.d.ts lib.es2017.string.d.ts lib.es2017.intl.d.ts lib.es2017.typedarrays.d.ts lib.es2018.asyncgenerator.d.ts lib.es2018.asynciterable.d.ts lib.es2018.intl.d.ts lib.es2018.promise.d.ts lib.es2018.regexp.d.ts lib.es2019.array.d.ts lib.es2019.object.d.ts lib.es2019.string.d.ts lib.es2019.symbol.d.ts lib.es2019.intl.d.ts lib.es2020.bigint.d.ts lib.es2020.date.d.ts lib.es2020.promise.d.ts lib.es2020.sharedmemory.d.ts lib.es2020.string.d.ts lib.es2020.symbol.wellknown.d.ts lib.es2020.intl.d.ts lib.es2020.number.d.ts lib.es2021.promise.d.ts lib.es2021.string.d.ts lib.es2021.weakref.d.ts lib.es2021.intl.d.ts lib.es2022.array.d.ts lib.es2022.error.d.ts lib.es2022.intl.d.ts lib.es2022.object.d.ts lib.es2022.string.d.ts lib.es2022.regexp.d.ts lib.es2023.array.d.ts lib.es2023.collection.d.ts lib.es2023.intl.d.ts lib.es2024.arraybuffer.d.ts lib.es2024.collection.d.ts lib.es2024.object.d.ts lib.es2024.promise.d.ts lib.es2024.regexp.d.ts lib.es2024.sharedmemory.d.ts lib.es2024.string.d.ts lib.es2025.collection.d.ts lib.es2025.float16.d.ts lib.es2025.intl.d.ts lib.es2025.iterator.d.ts lib.es2025.promise.d.ts lib.es2025.regexp.d.ts lib.esnext.array.d.ts lib.esnext.collection.d.ts lib.esnext.date.d.ts lib.esnext.decorators.d.ts lib.esnext.disposable.d.ts lib.esnext.error.d.ts lib.esnext.intl.d.ts lib.esnext.sharedmemory.d.ts lib.esnext.temporal.d.ts lib.esnext.typedarrays.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";

/// `libraries` followed by the given non-library inputs.
fn expected(libraries: &str, files: &[&str]) -> Vec<String> {
    libraries
        .split_whitespace()
        .chain(files.iter().copied())
        .map(str::to_owned)
        .collect()
}

/// A project whose `a.ts` root is compiled under each named configuration.
fn configurations(configs: &[(&str, &str)]) -> Project {
    let mut files = vec![("a.ts", "export {}\n")];
    files.extend_from_slice(configs);
    Project::new(&files)
}

fn with_options(options: &str) -> String {
    format!(r#"{{ "compilerOptions": {{ {options} }}, "files": ["a.ts"] }}"#)
}

#[test]
fn the_target_selects_the_default_libraries() {
    let cases = [
        ("", ES2025_FULL),
        (r#""target": null"#, ES2025_FULL),
        (r#""target": """#, ES2025_FULL),
        (r#""target": "ES2016""#, ES2016_FULL),
        (r#""target": "es2022""#, ES2022_FULL),
        (r#""target": "ESNext""#, ESNEXT_FULL),
        (r#""target": "es6""#, ES6),
        (r#""target": "ES2015""#, ES6),
    ];
    let configs: Vec<(String, String)> = cases
        .iter()
        .enumerate()
        .map(|(index, (options, _))| (format!("{index}.json"), with_options(options)))
        .collect();
    let configs: Vec<(&str, &str)> = configs
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let project = configurations(&configs);
    for (index, (options, expected_libraries)) in cases.iter().enumerate() {
        let program = project.load(&format!("{index}.json"));
        assert_eq!(
            listed(&program),
            expected(expected_libraries, &["a.ts"]),
            "{options}"
        );
        // Libraries are never searched, and every one is a library.
        assert_eq!(searched(&program), ["a.ts"], "{options}");
        for id in program.sources.ids() {
            let library = program
                .sources
                .get(id)
                .display
                .contains("typescript-7.0.2-lib/");
            let origin = program.files[id.index()].diagnostics.origin();
            assert_eq!(library, origin == FileOrigin::DefaultLibrary, "{options}");
        }
    }
}

#[test]
fn lib_lists_replace_the_default_libraries() {
    let cases = [
        (r#""lib": []"#, ""),
        // TypeScript drops an empty entry without a diagnostic.
        (r#""lib": [""]"#, ""),
        (r#""lib": ["ES2015.Core", "DOM"]"#, CORE_AND_DOM),
        (r#""lib": ["es6", "es2015"]"#, ES2015),
        (
            r#""lib": ["es2015.promise", "es5"]"#,
            "lib.es5.d.ts lib.es2015.promise.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts",
        ),
        (r#""lib": ["webworker"]"#, WEBWORKER),
        (
            "\"lib\": [\"esnext.\u{130}ntl\", \"WEBWOR\u{212A}ER\"]",
            WEBWORKER_AND_INTL,
        ),
        (
            r#""lib": ["dom", "dom.iterable", "esnext"], "target": "ES2022""#,
            DOM_ESNEXT,
        ),
        (r#""lib": null"#, ES2025_FULL),
    ];
    let mut configs: Vec<(String, String)> = cases
        .iter()
        .enumerate()
        .map(|(index, (options, _))| (format!("{index}.json"), with_options(options)))
        .collect();
    let rejected = [
        (r#""lib": ["bogus"]"#, InputErrorKind::Config, 32),
        // Only library names, not the files they select.
        (r#""lib": ["lib.es2015.d.ts"]"#, InputErrorKind::Config, 32),
        (r#""lib": [1]"#, InputErrorKind::Config, 32),
        (r#""lib": "dom""#, InputErrorKind::Config, 31),
        // TS5053, even for an empty list.
        (
            r#""noLib": true, "lib": ["dom"]"#,
            InputErrorKind::Config,
            46,
        ),
        (r#""noLib": true, "lib": []"#, InputErrorKind::Config, 46),
    ];
    for (index, (options, ..)) in rejected.iter().enumerate() {
        configs.push((format!("rejected{index}.json"), with_options(options)));
    }
    let configs: Vec<(&str, &str)> = configs
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let project = configurations(&configs);
    for (index, (options, expected_libraries)) in cases.iter().enumerate() {
        let program = project.load(&format!("{index}.json"));
        assert_eq!(
            listed(&program),
            expected(expected_libraries, &["a.ts"]),
            "{options}"
        );
    }
    for (index, (options, kind, column)) in rejected.iter().enumerate() {
        let error = project.error(&format!("rejected{index}.json"));
        assert_eq!(
            (error.kind, error.position),
            (*kind, position(1, *column)),
            "{options}: {error}"
        );
    }
    let unknown = project.error("rejected0.json");
    assert!(
        unknown
            .message
            .starts_with("Argument for 'lib' option must be: 'es5', 'es6', 'es2015', 'es7'"),
        "{}",
        unknown.message
    );
}

#[test]
fn library_settings_follow_extends() {
    let project = Project::new(&[
        ("a.ts", "export {}\n"),
        (
            "base.json",
            r#"{ "compilerOptions": { "lib": ["es2015.core", "DOM"] } }"#,
        ),
        ("nolib.json", NO_LIB),
        (
            "inherited.json",
            r#"{ "extends": "./base.json", "files": ["a.ts"] }"#,
        ),
        (
            "reset.json",
            r#"{ "extends": "./base.json", "compilerOptions": { "lib": null }, "files": ["a.ts"] }"#,
        ),
        (
            "empty.json",
            r#"{ "extends": "./base.json", "compilerOptions": { "lib": [] }, "files": ["a.ts"] }"#,
        ),
        (
            "conflict.json",
            r#"{ "extends": "./base.json", "compilerOptions": { "noLib": true }, "files": ["a.ts"] }"#,
        ),
        (
            "restored.json",
            r#"{ "extends": "./nolib.json", "compilerOptions": { "noLib": null }, "files": ["a.ts"] }"#,
        ),
        (
            "nofiles.json",
            r#"{ "extends": "./base.json", "files": [] }"#,
        ),
    ]);
    let listed_by = |config| listed(&project.load(config));
    assert_eq!(
        listed_by("inherited.json"),
        expected(CORE_AND_DOM, &["a.ts"])
    );
    assert_eq!(listed_by("reset.json"), expected(ES2025_FULL, &["a.ts"]));
    assert_eq!(listed_by("empty.json"), ["a.ts"]);
    assert_eq!(listed_by("restored.json"), expected(ES2025_FULL, &["a.ts"]));
    // Without root files, nothing selects a library.
    assert!(listed_by("nofiles.json").is_empty());
    let conflict = project.error("conflict.json");
    assert_eq!(conflict.kind, InputErrorKind::Config);
    assert_eq!(
        project.display(conflict.path.as_ref().unwrap()),
        "base.json"
    );
}

#[test]
fn library_references_follow_the_compilers_rules() {
    let reference = |name: &str| format!("/// <reference lib=\"{name}\" />\nexport {{}}\n");
    let sources = [
        ("forms.ts", "/// <reference lib=\"ES2015.Promise\" />\n/// <reference lib=\"lib.es2016.array.include.d.ts\" />\n/// <reference lib=\"es2015.promise\" />\nexport {}\n".to_owned()),
        ("kelvin.ts", reference("WEBWOR\u{212A}ER")),
        ("dotted.ts", reference("esnext.\u{130}ntl")),
        ("empty.ts", reference("")),
        ("unknown.ts", reference("bogus")),
        ("ignored.ts", format!("// @ts-ignore\n{}", reference("bogus"))),
        ("unchecked.d.ts", reference("bogus")),
        ("both.ts", "/// <reference lib=\"bogus\" />\n/// <reference lib=\"es2015\" />\nexport {}\n".to_owned()),
        ("hidden.ts", "/// <reference no-default-lib=\"true\" lib=\"es2015.promise\" />\nexport {}\n".to_owned()),
        ("nodefault.ts", "/// <reference no-default-lib=\"true\" />\nexport {}\n".to_owned()),
    ];
    let mut files: Vec<(String, String)> = sources
        .iter()
        .map(|(name, text)| (name.to_string(), text.clone()))
        .collect();
    let configs = [
        ("forms.json", r#""lib": []"#, "forms.ts"),
        ("kelvin.json", r#""lib": []"#, "kelvin.ts"),
        ("dotted.json", r#""lib": []"#, "dotted.ts"),
        ("empty.json", r#""lib": []"#, "empty.ts"),
        ("unknown.json", r#""lib": []"#, "unknown.ts"),
        ("ignored.json", r#""lib": []"#, "ignored.ts"),
        (
            "unchecked.json",
            r#""lib": [], "skipLibCheck": true"#,
            "unchecked.d.ts",
        ),
        ("checked.json", r#""lib": []"#, "unchecked.d.ts"),
        // skipDefaultLibCheck covers libraries, not this project file.
        (
            "default.json",
            r#""lib": [], "skipDefaultLibCheck": true"#,
            "unknown.ts",
        ),
        ("nolib.json", r#""noLib": true"#, "both.ts"),
        ("hidden.json", r#""lib": []"#, "hidden.ts"),
        ("nodefault.json", "", "nodefault.ts"),
    ];
    for (name, options, root) in configs {
        files.push((
            name.into(),
            format!(r#"{{ "compilerOptions": {{ {options} }}, "files": ["{root}"] }}"#),
        ));
    }
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let project = Project::new(&files);
    assert_eq!(
        listed(&project.load("forms.json")),
        [
            "lib.es2015.promise.d.ts",
            "lib.es2016.array.include.d.ts",
            "forms.ts"
        ]
    );
    assert_eq!(
        listed(&project.load("kelvin.json")),
        expected(WEBWORKER, &["kelvin.ts"])
    );
    for (config, name) in [
        ("dotted.json", "esnext.\u{130}ntl"),
        ("empty.json", ""),
        ("unknown.json", "bogus"),
        ("checked.json", "bogus"),
        ("default.json", "bogus"),
    ] {
        let error = project.error(config);
        assert_eq!(
            (error.kind, error.position, error.message.as_str()),
            (
                InputErrorKind::UnresolvedImport,
                position(1, 21),
                format!("Cannot find lib definition for '{name}'").as_str()
            ),
            "{config}"
        );
    }
    assert_eq!(listed(&project.load("ignored.json")), ["ignored.ts"]);
    assert_eq!(listed(&project.load("unchecked.json")), ["unchecked.d.ts"]);
    assert_eq!(listed(&project.load("nolib.json")), ["both.ts"]);
    assert_eq!(listed(&project.load("hidden.json")), ["hidden.ts"]);
    assert_eq!(
        listed(&project.load("nodefault.json")),
        expected(ES2025_FULL, &["nodefault.ts"])
    );
}

#[test]
fn libraries_referenced_from_dependencies_are_libraries() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "lib": [] }, "files": ["a.ts"] }"#,
        ),
        ("a.ts", "import \"dep\";\n"),
        (
            "node_modules/dep/index.d.ts",
            "/// <reference lib=\"es2015.promise\" />\nexport {};\n",
        ),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(
        listed(&program),
        [
            "lib.es2015.promise.d.ts",
            "node_modules/dep/index.d.ts",
            "a.ts"
        ]
    );
    assert_eq!(searched(&program), ["a.ts"]);
    assert_eq!(
        program.files[0].diagnostics.origin(),
        FileOrigin::DefaultLibrary
    );
}

#[test]
fn replacement_packages_stand_in_for_libraries_when_enabled() {
    let replaced = |options: &str| with_options(&format!(r#""libReplacement": true, {options}"#));
    let project = Project::new(&[
        ("a.ts", "export {}\n"),
        (
            "node_modules/@typescript/lib-dom/package.json",
            r#"{ "name": "@typescript/lib-dom", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/@typescript/lib-dom/index.d.ts",
            "/// <reference lib=\"es2015.collection\" />\ndeclare var replacedDom: number;\n",
        ),
        (
            "node_modules/@typescript/lib-es2015/package.json",
            r#"{ "name": "@typescript/lib-es2015" }"#,
        ),
        (
            "node_modules/@typescript/lib-es2015/promise.d.ts",
            "declare var replacedPromise: number;\n",
        ),
        ("on.json", &replaced(r#""lib": ["dom", "es2015.promise"]"#)),
        (
            "unset.json",
            &with_options(r#""lib": ["dom", "es2015.promise"]"#),
        ),
        (
            "off.json",
            &with_options(r#""libReplacement": false, "lib": ["dom", "es2015.promise"]"#),
        ),
        // No `@typescript/lib-dom/iterable` subpath exists.
        ("absent.json", &replaced(r#""lib": ["dom.iterable"]"#)),
        ("target.json", &replaced(r#""target": "es2016""#)),
    ]);
    let dom = "node_modules/@typescript/lib-dom/index.d.ts";
    let promise = "node_modules/@typescript/lib-es2015/promise.d.ts";
    let on = project.load("on.json");
    // A replacement's own references replace the vendored closure, and
    // replacements sort after the vendored libraries.
    assert_eq!(
        listed(&on),
        ["lib.es2015.collection.d.ts", dom, promise, "a.ts"]
    );
    assert_eq!(searched(&on), ["a.ts"]);
    for replacement in [dom, promise] {
        let (_, file) = file(&on, replacement);
        assert_eq!(file.diagnostics.origin(), FileOrigin::DefaultLibrary);
    }
    for config in ["unset.json", "off.json"] {
        assert_eq!(
            listed(&project.load(config)),
            expected(CORE_AND_DOM, &["a.ts"])
        );
    }
    assert_eq!(
        listed(&project.load("absent.json")),
        ["lib.dom.iterable.d.ts", "a.ts"]
    );
    // Replacements reached through a `.full` default tie with it in
    // priority. TypeScript lists them as `dom`, `promise`, then the default:
    // Go's unstable sort reorders this tie among 21 libraries, which the
    // tool keeps in the compiler's depth-first order instead.
    let mut target = listed(&project.load("target.json"));
    let ties = target.split_off(18);
    assert_eq!(
        target,
        expected(
            "lib.es5.d.ts lib.es2015.d.ts lib.es2016.d.ts lib.dom.iterable.d.ts lib.webworker.importscripts.d.ts lib.scripthost.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2016.array.include.d.ts lib.es2016.intl.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts",
            &[]
        )
    );
    assert_eq!(ties, [promise, dom, "lib.es2016.full.d.ts", "a.ts"]);
}

#[test]
fn replacement_libraries_are_commonjs_scripts_without_package_scopes() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "libReplacement": true, "lib": ["dom"], "module": "nodenext" }, "files": ["a.ts"] }"#,
        ),
        ("a.ts", "export {}\n"),
        (
            "node_modules/@typescript/lib-dom/package.json",
            r#"{ "name": "@typescript/lib-dom", "type": "module", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/@typescript/lib-dom/index.d.ts",
            "import \"cond\";\ndeclare var replacedDom: number;\n",
        ),
        (
            "node_modules/cond/package.json",
            r#"{ "name": "cond", "exports": { ".": { "import": "./esm.d.ts", "require": "./cjs.d.ts" } } }"#,
        ),
        ("node_modules/cond/esm.d.ts", "export {};\n"),
        ("node_modules/cond/cjs.d.ts", "export {};\n"),
    ]);
    // In a `"type": "module"` package, a library still imports as `require`.
    let program = project.load("tsconfig.json");
    assert_eq!(
        listed(&program),
        [
            "node_modules/@typescript/lib-dom/index.d.ts",
            "a.ts",
            "node_modules/cond/cjs.d.ts"
        ]
    );
    assert_eq!(searched(&program), ["a.ts"]);
    // TypeScript reports TS6504 for a JavaScript replacement.
    let javascript = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "libReplacement": true, "lib": ["dom"] }, "files": ["a.ts"] }"#,
        ),
        ("a.ts", "export {}\n"),
        (
            "node_modules/@typescript/lib-dom/package.json",
            r#"{ "name": "@typescript/lib-dom", "main": "index.js" }"#,
        ),
        ("node_modules/@typescript/lib-dom/index.js", "var x = 1;\n"),
    ]);
    let error = javascript.error("tsconfig.json");
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
}

/// A copy of the vendored declarations, for tests that damage it.
fn library_copy(project: &Project) -> String {
    let copy = project.root.join("vendored");
    std::fs::create_dir_all(&copy).unwrap();
    for entry in std::fs::read_dir(libraries::vendored_directory()).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), copy.join(entry.file_name())).unwrap();
    }
    paths::from_path(&std::fs::canonicalize(&copy).unwrap()).unwrap()
}

#[test]
fn missing_library_data_is_an_io_error() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "target": "es2016" }, "files": ["a.ts"] }"#,
        ),
        ("a.ts", "export {}\n"),
    ]);
    let directory = library_copy(&project);
    let load = || {
        crate::analysis::on_parsing_thread("tsparse-test-load", |parser| {
            load_with_libraries(
                &project.root.join("tsconfig.json"),
                &disk::Disk::default(),
                parser,
                &directory,
            )
        })
        .unwrap()
    };
    let program = load().unwrap();
    assert_eq!(
        listed_in(&program, &directory),
        expected(ES2016_FULL, &["a.ts"])
    );
    // A file the default references, and the default itself.
    for name in ["lib.es2015.core.d.ts", "lib.es2016.full.d.ts"] {
        let path = Path::new(&directory).join(name);
        let saved = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let error = load().unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Io, "{error}");
        assert_eq!(error.path.as_deref(), Some(path.as_path()));
        std::fs::write(&path, saved).unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = Path::new(&directory).join("lib.es5.d.ts");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&path).is_ok() {
            eprintln!(
                "SKIP missing_library_data_is_an_io_error: permissions are not enforced, as for the superuser"
            );
        } else {
            let error = load().unwrap_err();
            assert_eq!(error.kind, InputErrorKind::Io, "{error}");
            assert_eq!(error.path.as_deref(), Some(path.as_path()));
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
}

/// `listed`, for libraries in `directory` rather than the vendored one.
fn listed_in(program: &Program, directory: &str) -> Vec<String> {
    program
        .sources
        .ids()
        .map(|id| {
            let source = program.sources.get(id);
            match source.path.strip_prefix(directory) {
                Ok(name) => name.to_string_lossy().into_owned(),
                Err(_) => source.display.clone(),
            }
        })
        .collect()
}

// Standard libraries under the TypeScript 5.6 dialect: the installed
// compiler's own declarations, selected by 5.6.3's catalog. Each list is
// 5.6.3's `tsc --listFilesOnly --incremental false` for the same
// configuration, with the compiler's libraries named by file name.

/// Artemis's `lib: ["dom", "dom.iterable", "esnext"]` with `target: ES2017`:
/// 74 libraries, `esnext.array` and `esnext.string` ordered by the first of
/// their two places in `libs`.
const ARTEMIS_5_6: &str = "lib.es5.d.ts lib.es2015.d.ts lib.es2016.d.ts lib.es2017.d.ts lib.es2018.d.ts lib.es2019.d.ts lib.es2020.d.ts lib.es2021.d.ts lib.es2022.d.ts lib.es2023.d.ts lib.esnext.d.ts lib.dom.d.ts lib.dom.iterable.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2016.array.include.d.ts lib.es2016.intl.d.ts lib.es2017.date.d.ts lib.es2017.object.d.ts lib.es2017.sharedmemory.d.ts lib.es2017.string.d.ts lib.es2017.intl.d.ts lib.es2017.typedarrays.d.ts lib.es2018.asyncgenerator.d.ts lib.es2018.asynciterable.d.ts lib.es2018.intl.d.ts lib.es2018.promise.d.ts lib.es2018.regexp.d.ts lib.es2019.array.d.ts lib.es2019.object.d.ts lib.es2019.string.d.ts lib.es2019.symbol.d.ts lib.es2019.intl.d.ts lib.es2020.bigint.d.ts lib.es2020.date.d.ts lib.es2020.promise.d.ts lib.es2020.sharedmemory.d.ts lib.es2020.string.d.ts lib.es2020.symbol.wellknown.d.ts lib.es2020.intl.d.ts lib.es2020.number.d.ts lib.es2021.promise.d.ts lib.es2021.string.d.ts lib.es2021.weakref.d.ts lib.es2021.intl.d.ts lib.es2022.array.d.ts lib.es2022.error.d.ts lib.es2022.intl.d.ts lib.es2022.object.d.ts lib.es2022.sharedmemory.d.ts lib.es2022.string.d.ts lib.es2022.regexp.d.ts lib.es2023.array.d.ts lib.es2023.collection.d.ts lib.es2023.intl.d.ts lib.esnext.array.d.ts lib.esnext.collection.d.ts lib.esnext.intl.d.ts lib.esnext.disposable.d.ts lib.esnext.string.d.ts lib.esnext.promise.d.ts lib.esnext.decorators.d.ts lib.esnext.object.d.ts lib.esnext.regexp.d.ts lib.esnext.iterator.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
/// `target: ES2015` without `lib`: `lib.es6.d.ts` first.
const ES2015_DEFAULT_5_6: &str = "lib.es6.d.ts lib.es5.d.ts lib.es2015.d.ts lib.dom.d.ts lib.dom.iterable.d.ts lib.webworker.importscripts.d.ts lib.scripthost.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
/// `target: ES2017` without `lib`: the `.full` default last.
const ES2017_DEFAULT_5_6: &str = "lib.es5.d.ts lib.es2015.d.ts lib.es2016.d.ts lib.es2017.d.ts lib.dom.d.ts lib.dom.iterable.d.ts lib.webworker.importscripts.d.ts lib.scripthost.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2016.array.include.d.ts lib.es2016.intl.d.ts lib.es2017.date.d.ts lib.es2017.object.d.ts lib.es2017.sharedmemory.d.ts lib.es2017.string.d.ts lib.es2017.intl.d.ts lib.es2017.typedarrays.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts lib.es2017.full.d.ts";
/// `target: ESNext` without `lib`.
const ESNEXT_DEFAULT_5_6: &str = "lib.es5.d.ts lib.es2015.d.ts lib.es2016.d.ts lib.es2017.d.ts lib.es2018.d.ts lib.es2019.d.ts lib.es2020.d.ts lib.es2021.d.ts lib.es2022.d.ts lib.es2023.d.ts lib.esnext.d.ts lib.dom.d.ts lib.dom.iterable.d.ts lib.dom.asynciterable.d.ts lib.webworker.importscripts.d.ts lib.scripthost.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2016.array.include.d.ts lib.es2016.intl.d.ts lib.es2017.date.d.ts lib.es2017.object.d.ts lib.es2017.sharedmemory.d.ts lib.es2017.string.d.ts lib.es2017.intl.d.ts lib.es2017.typedarrays.d.ts lib.es2018.asyncgenerator.d.ts lib.es2018.asynciterable.d.ts lib.es2018.intl.d.ts lib.es2018.promise.d.ts lib.es2018.regexp.d.ts lib.es2019.array.d.ts lib.es2019.object.d.ts lib.es2019.string.d.ts lib.es2019.symbol.d.ts lib.es2019.intl.d.ts lib.es2020.bigint.d.ts lib.es2020.date.d.ts lib.es2020.promise.d.ts lib.es2020.sharedmemory.d.ts lib.es2020.string.d.ts lib.es2020.symbol.wellknown.d.ts lib.es2020.intl.d.ts lib.es2020.number.d.ts lib.es2021.promise.d.ts lib.es2021.string.d.ts lib.es2021.weakref.d.ts lib.es2021.intl.d.ts lib.es2022.array.d.ts lib.es2022.error.d.ts lib.es2022.intl.d.ts lib.es2022.object.d.ts lib.es2022.sharedmemory.d.ts lib.es2022.string.d.ts lib.es2022.regexp.d.ts lib.es2023.array.d.ts lib.es2023.collection.d.ts lib.es2023.intl.d.ts lib.esnext.array.d.ts lib.esnext.collection.d.ts lib.esnext.intl.d.ts lib.esnext.disposable.d.ts lib.esnext.string.d.ts lib.esnext.promise.d.ts lib.esnext.decorators.d.ts lib.esnext.object.d.ts lib.esnext.regexp.d.ts lib.esnext.iterator.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts lib.esnext.full.d.ts";

/// Options both dialects read alike, beside the `lib` and `target` a test
/// sets: without `target`, TypeScript 5's default is ES5, which the
/// TypeScript 5.6 dialect refuses.
const MODERN: &str = r#""types": [], "strict": true, "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true"#;

impl Project {
    /// Install a copy of the TypeScript package at `package` as this
    /// project's compiler (`test_compiler::install`). Returns the real path
    /// of the copy's `lib`.
    fn install_compiler(&self, package: &Path) -> String {
        let installed = test_compiler::install(package, &self.root);
        paths::from_path(&std::fs::canonicalize(installed.join("lib")).unwrap()).unwrap()
    }
}

/// The TypeScript 5.6 dialect reads the installed compiler's `lib`
/// directory and selects its files by 5.6.3's catalog: Artemis's library
/// list, and each target's default (ES2015's `lib.es6.d.ts`, ES2016 to
/// ES2023's `.full` files, ESNext's), every one a library, never searched.
/// The same Artemis options under TypeScript 7 read the vendored 7.0.2
/// declarations exactly as before.
#[test]
fn typescript_5_6_reads_the_installed_compilers_libraries() {
    let Some(package) = typescript_5_6("typescript_5_6_reads_the_installed_compilers_libraries")
    else {
        return;
    };
    let config = |options: &str| {
        format!(r#"{{ "compilerOptions": {{ {MODERN}, {options} }}, "files": ["a.ts"] }}"#)
    };
    let project = Project::new(&[
        ("a.ts", "export {}\n"),
        (
            "artemis.json",
            &config(r#""lib": ["dom", "dom.iterable", "esnext"], "target": "ES2017""#),
        ),
        ("es2015.json", &config(r#""target": "ES2015""#)),
        ("es2017.json", &config(r#""target": "es2017""#)),
        ("esnext.json", &config(r#""target": "ESNext""#)),
    ]);
    let lib = project.install_compiler(&package);
    for (name, libraries) in [
        ("artemis.json", ARTEMIS_5_6),
        ("es2015.json", ES2015_DEFAULT_5_6),
        ("es2017.json", ES2017_DEFAULT_5_6),
        ("esnext.json", ESNEXT_DEFAULT_5_6),
    ] {
        let program = project.load(name);
        assert_eq!(
            listed_in(&program, &lib),
            expected(libraries, &["a.ts"]),
            "{name}"
        );
        assert_eq!(searched(&program), ["a.ts"], "{name}");
        for id in program.sources.ids() {
            let library = program.sources.get(id).path.starts_with(&lib);
            let origin = program.files[id.index()].diagnostics.origin();
            assert_eq!(library, origin == FileOrigin::DefaultLibrary, "{name}");
        }
    }
    assert_eq!(ARTEMIS_5_6.split_whitespace().count(), 74);
    project.write(&[(MANIFEST, &manifest("7.0.2"))]);
    assert_eq!(
        listed(&project.load("artemis.json")),
        expected(DOM_ESNEXT, &["a.ts"])
    );
}

/// `lib: ["es5", "dom"]` under TypeScript 7.0.2.
const CORE_AND_DOM_ES5: &str = "lib.es5.d.ts lib.es2015.d.ts lib.dom.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2018.asynciterable.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
/// `lib: ["es5"]` and a reference to `es2024`, under TypeScript 7.0.2.
const ES5_AND_ES2024: &str = "lib.es5.d.ts lib.es2015.d.ts lib.es2016.d.ts lib.es2017.d.ts lib.es2018.d.ts lib.es2019.d.ts lib.es2020.d.ts lib.es2021.d.ts lib.es2022.d.ts lib.es2023.d.ts lib.es2024.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2016.array.include.d.ts lib.es2016.intl.d.ts lib.es2017.arraybuffer.d.ts lib.es2017.date.d.ts lib.es2017.object.d.ts lib.es2017.sharedmemory.d.ts lib.es2017.string.d.ts lib.es2017.intl.d.ts lib.es2017.typedarrays.d.ts lib.es2018.asyncgenerator.d.ts lib.es2018.asynciterable.d.ts lib.es2018.intl.d.ts lib.es2018.promise.d.ts lib.es2018.regexp.d.ts lib.es2019.array.d.ts lib.es2019.object.d.ts lib.es2019.string.d.ts lib.es2019.symbol.d.ts lib.es2019.intl.d.ts lib.es2020.bigint.d.ts lib.es2020.date.d.ts lib.es2020.promise.d.ts lib.es2020.sharedmemory.d.ts lib.es2020.string.d.ts lib.es2020.symbol.wellknown.d.ts lib.es2020.intl.d.ts lib.es2020.number.d.ts lib.es2021.promise.d.ts lib.es2021.string.d.ts lib.es2021.weakref.d.ts lib.es2021.intl.d.ts lib.es2022.array.d.ts lib.es2022.error.d.ts lib.es2022.intl.d.ts lib.es2022.object.d.ts lib.es2022.string.d.ts lib.es2022.regexp.d.ts lib.es2023.array.d.ts lib.es2023.collection.d.ts lib.es2023.intl.d.ts lib.es2024.arraybuffer.d.ts lib.es2024.collection.d.ts lib.es2024.object.d.ts lib.es2024.promise.d.ts lib.es2024.regexp.d.ts lib.es2024.sharedmemory.d.ts lib.es2024.string.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
/// `lib: ["es5"]` and a reference to `webworker`, under TypeScript 7.0.2.
const ES5_AND_WEBWORKER: &str = "lib.es5.d.ts lib.es2015.d.ts lib.webworker.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2018.asynciterable.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";

/// A `/// <reference lib>` under the TypeScript 5.6 dialect names a library
/// by `libMap` key only, lowered as 5.6.3's `toFileNameLowerCase` lowers it,
/// and an unknown one is TS2727 with 5.6.3's suggestion from `libs`, or
/// TS2726 without one. TypeScript 7 also accepts the file a name selects
/// and keeps its own lowering and wording, which has no suggestion.
#[test]
fn typescript_5_6_library_references_name_libraries_only() {
    let Some(package) = typescript_5_6("typescript_5_6_library_references_name_libraries_only")
    else {
        return;
    };
    /// What `lib: ["es5"]` and the reference load: the libraries in
    /// program order, or the reference's message.
    type Outcome = Result<&'static str, &'static str>;
    let only = |file: &'static str| -> &'static str {
        match file {
            "lib.es2015.promise.d.ts" => {
                "lib.es5.d.ts lib.es2015.promise.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts"
            }
            "lib.webworker.d.ts" => {
                "lib.es5.d.ts lib.webworker.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts"
            }
            "lib.esnext.array.d.ts" => {
                "lib.es5.d.ts lib.esnext.array.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts"
            }
            _ => unreachable!(),
        }
    };
    let es5 = "lib.es5.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
    let cases: [(&str, Outcome, Outcome); 11] = [
        (
            "lib.es2015.promise.d.ts",
            Err(
                "Cannot find lib definition for 'lib.es2015.promise.d.ts'. Did you mean 'es2018.promise'?",
            ),
            Ok(only("lib.es2015.promise.d.ts")),
        ),
        (
            "ES2015.Promise",
            Ok(only("lib.es2015.promise.d.ts")),
            Ok(only("lib.es2015.promise.d.ts")),
        ),
        (
            "es2015.promis",
            Err("Cannot find lib definition for 'es2015.promis'. Did you mean 'es2015.promise'?"),
            Err("Cannot find lib definition for 'es2015.promis'"),
        ),
        (
            "es2024",
            Err("Cannot find lib definition for 'es2024'. Did you mean 'es2020'?"),
            Ok(ES5_AND_ES2024),
        ),
        // U+0130 stays, and the rest lowers.
        (
            "esnext.\u{130}ntl",
            Err("Cannot find lib definition for 'esnext.\u{130}ntl'. Did you mean 'esnext.intl'?"),
            Err("Cannot find lib definition for 'esnext.\u{130}ntl'"),
        ),
        // KELVIN SIGN lowers to `k`.
        (
            "WEBWOR\u{212A}ER",
            Ok(only("lib.webworker.d.ts")),
            Ok(ES5_AND_WEBWORKER),
        ),
        // 5.6.3 lowers a capital sigma as final by its run alone.
        (
            "A\u{3a3}.b",
            Err("Cannot find lib definition for 'a\u{3c2}.b'"),
            Err("Cannot find lib definition for 'a\u{3c3}.b'"),
        ),
        (
            "lib.es5.d.ts",
            Err("Cannot find lib definition for 'lib.es5.d.ts'"),
            Ok(es5),
        ),
        // A name declared twice selects its last file.
        (
            "esnext.array",
            Ok(only("lib.esnext.array.d.ts")),
            Ok(only("lib.esnext.array.d.ts")),
        ),
        (
            "",
            Err("Cannot find lib definition for ''"),
            Err("Cannot find lib definition for ''"),
        ),
        (
            "lib.esnext.full.d.ts",
            Err("Cannot find lib definition for 'lib.esnext.full.d.ts'"),
            Err("Cannot find lib definition for 'lib.esnext.full.d.ts'"),
        ),
    ];
    let project = Project::new(&[
        (
            "tsconfig.json",
            &format!(
                r#"{{ "compilerOptions": {{ {MODERN}, "target": "es2017", "lib": ["es5"] }}, "files": ["a.ts"] }}"#
            ),
        ),
        (
            "nolib.json",
            &format!(
                r#"{{ "compilerOptions": {{ {MODERN}, "target": "es2017", "noLib": true }}, "files": ["a.ts"] }}"#
            ),
        ),
    ]);
    let installed = project.install_compiler(&package);
    let vendored =
        paths::from_path(&std::fs::canonicalize(libraries::vendored_directory()).unwrap()).unwrap();
    for (version, directory) in [("5.6.3", &installed), ("7.0.2", &vendored)] {
        project.write(&[(MANIFEST, &manifest(version))]);
        for (name, typescript_5_6, typescript_7) in &cases {
            let reference = format!("/// <reference lib=\"{name}\" />\nexport {{}}\n");
            project.write(&[("a.ts", &reference)]);
            let outcome = if version == "5.6.3" {
                typescript_5_6
            } else {
                typescript_7
            };
            match (project.try_load("tsconfig.json"), outcome) {
                (Ok(program), Ok(libraries)) => assert_eq!(
                    listed_in(&program, directory),
                    expected(libraries, &["a.ts"]),
                    "{version} {name}"
                ),
                (Err(error), Err(message)) => assert_eq!(
                    (error.kind, error.position, error.message.as_str()),
                    (InputErrorKind::UnresolvedImport, position(1, 21), *message),
                    "{version} {name}"
                ),
                (outcome, expected) => panic!(
                    "{version} {name}: {:?}, expected {expected:?}",
                    outcome.map(|program| listed_in(&program, directory))
                ),
            }
        }
        // `noLib` ignores every reference, unknown ones included.
        project.write(&[(
            "a.ts",
            "/// <reference lib=\"lib.es2015.promise.d.ts\" />\nexport {}\n",
        )]);
        assert_eq!(listed(&project.load("nolib.json")), ["a.ts"], "{version}");
    }
}

/// A file the TypeScript 5.6 dialect's catalog selects but the installed
/// compiler lacks is an `Io` failure naming it, whether it is the target's
/// default, a file the default references, or a `lib` entry: the vendored
/// TypeScript 7.0.2 copy never stands in for it. So is one that cannot be
/// read.
#[test]
fn a_library_missing_from_the_installed_compiler_is_an_io_error() {
    let Some(package) =
        typescript_5_6("a_library_missing_from_the_installed_compiler_is_an_io_error")
    else {
        return;
    };
    let config = |options: &str| {
        format!(r#"{{ "compilerOptions": {{ {MODERN}, {options} }}, "files": ["a.ts"] }}"#)
    };
    let project = Project::new(&[
        ("a.ts", "export {}\n"),
        ("es2017.json", &config(r#""target": "es2017""#)),
        (
            "dom.json",
            &config(r#""target": "es2017", "lib": ["es5", "dom"]"#),
        ),
    ]);
    let lib = project.install_compiler(&package);
    for (config, name) in [
        ("es2017.json", "lib.es2017.full.d.ts"),
        ("es2017.json", "lib.es2015.core.d.ts"),
        ("dom.json", "lib.dom.d.ts"),
    ] {
        project.load(config);
        let path = Path::new(&lib).join(name);
        let saved = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let error = project.error(config);
        assert_eq!(error.kind, InputErrorKind::Io, "{config} {name}: {error}");
        assert_eq!(
            error.path.as_deref(),
            Some(path.as_path()),
            "{config} {name}"
        );
        std::fs::write(&path, saved).unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = Path::new(&lib).join("lib.es5.d.ts");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&path).is_ok() {
            eprintln!(
                "SKIP a_library_missing_from_the_installed_compiler_is_an_io_error: permissions are not enforced, as for the superuser"
            );
        } else {
            let error = project.error("dom.json");
            assert_eq!(error.kind, InputErrorKind::Io, "{error}");
            assert_eq!(error.path.as_deref(), Some(path.as_path()));
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
}

/// Under the TypeScript 5.6 dialect a library read from the installed
/// compiler's `node_modules/typescript/lib` is a default library, never
/// searched or external, while `typescript/lib/typescript.d.ts` beside it,
/// which a package imports, is a dependency like any other package file.
#[test]
fn installed_libraries_are_libraries_and_the_compiler_api_is_a_dependency() {
    let Some(package) =
        typescript_5_6("installed_libraries_are_libraries_and_the_compiler_api_is_a_dependency")
    else {
        return;
    };
    let project = Project::new(&[
        (
            "tsconfig.json",
            &format!(
                r#"{{ "compilerOptions": {{ {MODERN}, "target": "es2017", "lib": ["es5"] }}, "files": ["a.ts"] }}"#
            ),
        ),
        (
            "a.ts",
            "import { kind } from \"dep\";\nexport const k = kind;\n",
        ),
        (
            "node_modules/dep/package.json",
            r#"{ "name": "dep", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/dep/index.d.ts",
            "import type * as ts from \"typescript\";\nexport declare const kind: ts.SyntaxKind;\n",
        ),
    ]);
    let lib = project.install_compiler(&package);
    let program = project.load("tsconfig.json");
    assert_eq!(
        listed_in(&program, &lib),
        [
            "lib.es5.d.ts",
            "lib.decorators.d.ts",
            "lib.decorators.legacy.d.ts",
            "typescript.d.ts",
            "node_modules/dep/index.d.ts",
            "a.ts"
        ]
    );
    assert_eq!(searched(&program), ["a.ts"]);
    let origins: Vec<FileOrigin> = program
        .files
        .iter()
        .map(|file| file.diagnostics.origin())
        .collect();
    assert_eq!(
        origins,
        [
            FileOrigin::DefaultLibrary,
            FileOrigin::DefaultLibrary,
            FileOrigin::DefaultLibrary,
            FileOrigin::Dependency,
            FileOrigin::Dependency,
            FileOrigin::Project
        ]
    );
}

/// Under the TypeScript 5.6 dialect a `@typescript/lib-*` package beside
/// the configuration replaces every library `pathForLibFile` locates, by
/// 5.6.3's own lookup (`resolveLibrary`): `{ moduleResolution: Node10 }`
/// without the project's `paths`, `baseUrl`, `moduleSuffixes`, or
/// `preserveSymlinks`, and without package `exports`, so a link resolves to
/// its target. The target's default library, which 5.6.3 reads without
/// `pathForLibFile`, is never replaced, though the libraries it references
/// are. TypeScript 7 looks a package up only under `libReplacement`, with
/// the configured resolver as a `require`, and replaces the default too.
/// Each list is the compiler's, run from another directory with its own
/// competing `@typescript/lib-dom`.
#[test]
fn typescript_5_6_replaces_libraries_by_its_own_lookup() {
    let Some(package) = typescript_5_6("typescript_5_6_replaces_libraries_by_its_own_lookup")
    else {
        return;
    };
    let config = |options: &str| {
        format!(
            r#"{{ "compilerOptions": {{ {MODERN}, "target": "ES2017"{options} }}, "files": ["a.ts"] }}"#
        )
    };
    let dom = r#", "lib": ["es5", "dom"]"#;
    let scripthost = r#", "lib": ["es5", "scripthost"]"#;
    let configs: Vec<(String, String)> = [
        ("five-plain", dom.to_owned()),
        (
            "five-paths",
            format!(r#"{dom}, "paths": {{ "@typescript/lib-dom": ["./mapped/lib-dom.d.ts"] }}"#),
        ),
        ("five-base-url", format!(r#"{dom}, "baseUrl": "./base""#)),
        ("five-suffix", format!(r#"{dom}, "moduleSuffixes": [".ios", ""]"#)),
        ("five-default", String::new()),
        (
            "five-linked-suffix",
            format!(r#"{scripthost}, "moduleSuffixes": [".ios", ""]"#),
        ),
        (
            "five-linked-preserve",
            format!(r#"{scripthost}, "preserveSymlinks": true"#),
        ),
        ("seven-plain", format!(r#"{dom}, "libReplacement": true"#)),
        ("seven-off", dom.to_owned()),
        (
            "seven-paths",
            format!(
                r#"{dom}, "libReplacement": true, "paths": {{ "@typescript/lib-dom": ["./mapped/lib-dom.d.ts"] }}"#
            ),
        ),
        (
            "seven-linked-suffix",
            format!(r#"{scripthost}, "libReplacement": true, "moduleSuffixes": [".ios", ""]"#),
        ),
        (
            "seven-linked-preserve",
            format!(r#"{scripthost}, "libReplacement": true, "preserveSymlinks": true"#),
        ),
        ("seven-default", r#", "libReplacement": true"#.to_owned()),
    ]
    .into_iter()
    .map(|(name, options)| (format!("{name}.json"), config(&options)))
    .collect();
    let mut files: Vec<(&str, &str)> = vec![
        ("a.ts", "export {}\n"),
        (
            "node_modules/@typescript/lib-dom/package.json",
            r#"{ "name": "@typescript/lib-dom", "types": "index.d.ts", "exports": { ".": { "types": "./exported.d.ts" } } }"#,
        ),
        (
            "node_modules/@typescript/lib-dom/index.d.ts",
            "declare var fromIndex: number;\n",
        ),
        (
            "node_modules/@typescript/lib-dom/exported.d.ts",
            "declare var fromExports: number;\n",
        ),
        (
            "node_modules/@typescript/lib-dom/index.ios.d.ts",
            "declare var fromSuffix: number;\n",
        ),
        ("mapped/lib-dom.d.ts", "declare var fromPaths: number;\n"),
        (
            "base/@typescript/lib-dom/index.d.ts",
            "declare var fromBaseUrl: number;\n",
        ),
        (
            "node_modules/@typescript/lib-es2017/package.json",
            r#"{ "name": "@typescript/lib-es2017" }"#,
        ),
        (
            "node_modules/@typescript/lib-es2017/index.d.ts",
            "declare var es2017Index: number;\n",
        ),
        (
            "node_modules/@typescript/lib-es2017/full.d.ts",
            "declare var es2017Full: number;\n",
        ),
        (
            "store/sh/package.json",
            r#"{ "name": "@typescript/lib-scripthost", "types": "index.d.ts" }"#,
        ),
        ("store/sh/index.d.ts", "declare var linked: number;\n"),
        (
            "store/sh/index.ios.d.ts",
            "declare var linkedSuffix: number;\n",
        ),
    ];
    files.extend(
        configs
            .iter()
            .map(|(name, text)| (name.as_str(), text.as_str())),
    );
    let project = Project::new(&files);
    project.link("node_modules/@typescript/lib-scripthost", "../../store/sh");
    let installed = project.install_compiler(&package);
    let vendored =
        paths::from_path(&std::fs::canonicalize(libraries::vendored_directory()).unwrap()).unwrap();
    let es5 = "lib.es5.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
    let replaced = |replacement: &str| format!("{es5} {replacement}");
    let cases = [
        (
            "five-plain",
            replaced("node_modules/@typescript/lib-dom/index.d.ts"),
        ),
        (
            "five-paths",
            replaced("node_modules/@typescript/lib-dom/index.d.ts"),
        ),
        (
            "five-base-url",
            replaced("node_modules/@typescript/lib-dom/index.d.ts"),
        ),
        (
            "five-suffix",
            replaced("node_modules/@typescript/lib-dom/index.d.ts"),
        ),
        (
            "five-default",
            "lib.dom.iterable.d.ts lib.webworker.importscripts.d.ts node_modules/@typescript/lib-es2017/index.d.ts node_modules/@typescript/lib-dom/index.d.ts store/sh/index.d.ts lib.es2017.full.d.ts".into(),
        ),
        ("five-linked-suffix", replaced("store/sh/index.d.ts")),
        ("five-linked-preserve", replaced("store/sh/index.d.ts")),
        (
            "seven-plain",
            replaced("node_modules/@typescript/lib-dom/exported.d.ts"),
        ),
        ("seven-off", CORE_AND_DOM_ES5.into()),
        ("seven-paths", replaced("mapped/lib-dom.d.ts")),
        ("seven-linked-suffix", replaced("store/sh/index.ios.d.ts")),
        (
            "seven-linked-preserve",
            replaced("node_modules/@typescript/lib-scripthost/index.d.ts"),
        ),
        (
            "seven-default",
            "node_modules/@typescript/lib-es2017/full.d.ts".into(),
        ),
    ];
    for (name, libraries) in cases {
        let (version, directory) = if name.starts_with("five") {
            ("5.6.3", &installed)
        } else {
            ("7.0.2", &vendored)
        };
        project.write(&[(MANIFEST, &manifest(version))]);
        let program = project.load(&format!("{name}.json"));
        assert_eq!(
            listed_in(&program, directory),
            expected(&libraries, &["a.ts"]),
            "{name}"
        );
        assert_eq!(searched(&program), ["a.ts"], "{name}");
    }
}

/// Program order around a replacement library, as each compiler lists it:
/// 5.6.3's `createProgram` processes the type packages before the
/// libraries, so a type package precedes the files a replacement reaches by
/// an import or a `/// <reference types>`, and a replacement it references
/// precedes one a `lib` entry names; a file a library reaches by `/// <reference path>` is
/// ordered with the libraries (`processReferencedFiles(file, isDefaultLib)`).
/// TypeScript 7.0.2 adds the library tasks first and orders a referenced
/// file with the other inputs. Each list is the compiler's `tsc
/// --listFilesOnly`, 7.0.2's under `libReplacement`.
#[test]
fn program_order_around_replacements_is_each_compilers() {
    let Some(package) = typescript_5_6("program_order_around_replacements_is_each_compilers")
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
            r#"{{ "compilerOptions": {{ "types": ["{types}"], "strict": true, "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true, "target": "ES2017", "lib": [{lib}]{replacing} }}, "files": ["a.ts"] }}"#
        )
    };
    let dom = r#""es5", "dom""#;
    let both = r#""es5", "dom", "scripthost""#;
    let configs = [
        ("pick.json", config("picker", dom, false)),
        ("pick-replacing.json", config("picker", dom, true)),
        ("tie.json", config("tie", both, false)),
        ("tie-replacing.json", config("tie", both, true)),
    ];
    let mut files: Vec<(&str, &str)> = vec![
        ("a.ts", "export {}\n"),
        (
            "node_modules/picker/package.json",
            r#"{ "name": "picker", "types": "index.d.ts" }"#,
        ),
        ("node_modules/picker/index.d.ts", "export {}\n"),
        (
            "node_modules/pickref/package.json",
            r#"{ "name": "pickref", "types": "index.d.ts" }"#,
        ),
        ("node_modules/pickref/index.d.ts", "export {}\n"),
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
        ("node_modules/@typescript/lib-dom/pick.d.ts", "export {}\n"),
        (
            "node_modules/@typescript/lib-scripthost/package.json",
            r#"{ "name": "@typescript/lib-scripthost", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/@typescript/lib-scripthost/index.d.ts",
            "export {}\n",
        ),
    ];
    files.extend(configs.iter().map(|(name, text)| (*name, text.as_str())));
    let project = Project::new(&files);
    project.install_compiler(&package);
    let libraries = "lib.es5.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
    let dom = "node_modules/@typescript/lib-dom/index.d.ts";
    let pick = "node_modules/@typescript/lib-dom/pick.d.ts";
    let picker = "node_modules/picker/index.d.ts";
    let pickref = "node_modules/pickref/index.d.ts";
    let scripthost = "node_modules/@typescript/lib-scripthost/index.d.ts";
    let tie = "node_modules/tie/index.d.ts";
    let listing = |inputs: &[&str]| expected(libraries, inputs);
    for (variant, text, config, typescript_5_6, typescript_7) in [
        (
            "import",
            "import \"./pick\";\n",
            "pick",
            listing(&[dom, "a.ts", picker, pick]),
            listing(&[dom, "a.ts", pick, picker]),
        ),
        (
            "reference types",
            "/// <reference types=\"pickref\" />\n",
            "pick",
            listing(&[dom, "a.ts", picker, pickref]),
            listing(&[dom, "a.ts", pickref, picker]),
        ),
        (
            "reference path",
            "/// <reference path=\"./pick.d.ts\" />\n",
            "pick",
            listing(&[pick, dom, "a.ts", picker]),
            listing(&[dom, "a.ts", pick, picker]),
        ),
        // A file the reference reaches in the compiler's directory is
        // ordered by its name there under 5.6.3, and is the installed
        // package's file, not a library, under 7.0.2.
        (
            "reference path into the compiler",
            "/// <reference path=\"../../typescript/lib/lib.es2015.core.d.ts\" />\n",
            "pick",
            expected(
                "lib.es5.d.ts lib.es2015.core.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts",
                &[dom, "a.ts", picker],
            ),
            listing(&[
                dom,
                "a.ts",
                "node_modules/typescript/lib/lib.es2015.core.d.ts",
                picker,
            ]),
        ),
        (
            "two replacements",
            "export {}\n",
            "tie",
            listing(&[scripthost, dom, "a.ts", tie]),
            listing(&[dom, scripthost, "a.ts", tie]),
        ),
    ] {
        project.write(&[("node_modules/@typescript/lib-dom/index.d.ts", text)]);
        for (version, config, inputs) in [
            ("5.6.3", format!("{config}.json"), typescript_5_6),
            ("7.0.2", format!("{config}-replacing.json"), typescript_7),
        ] {
            project.write(&[(MANIFEST, &manifest(version))]);
            assert_eq!(
                listed(&project.load(&config)),
                inputs,
                "{version} {variant}"
            );
        }
    }
}

/// Where two routes reach one file, 5.6.3 orders it by the route that
/// processes it first, depth first with the type packages before the
/// libraries, and then orders the libraries by each file's own path,
/// anywhere beneath its `lib` directory, by file name
/// (`getDefaultLibFilePriority`). TypeScript 7.0.2 orders a file by the
/// library task it loads first, and none of these files lies beneath its
/// own directory. Each list is the compiler's `tsc --listFilesOnly`, 7.0.2's
/// under `libReplacement`.
#[test]
fn overlapping_routes_and_library_locations_order_as_each_compiler_lists() {
    let Some(package) =
        typescript_5_6("overlapping_routes_and_library_locations_order_as_each_compiler_lists")
    else {
        return;
    };
    const PICKER: &str = r#"{ "name": "picker", "types": "index.d.ts" }"#;
    const LIB_DOM: &str = r#"{ "name": "@typescript/lib-dom", "types": "index.d.ts" }"#;
    const LIB_SCRIPTHOST: &str =
        r#"{ "name": "@typescript/lib-scripthost", "types": "index.d.ts" }"#;
    let es5 = "lib.es5.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts";
    let dom = "node_modules/@typescript/lib-dom/index.d.ts";
    let shared = "node_modules/@typescript/lib-dom/shared.d.ts";
    let scripthost = "node_modules/@typescript/lib-scripthost/index.d.ts";
    let other = "node_modules/picker/other.d.ts";
    let picker = "node_modules/picker/index.d.ts";
    type Case<'a> = (
        &'a str,
        &'a str,
        &'a str,
        Vec<(&'a str, &'a str)>,
        Vec<String>,
        Vec<String>,
    );
    let cases: Vec<Case> = vec![
        (
            "a type package imports the replacement",
            "picker",
            r#""es5", "dom""#,
            vec![
                ("node_modules/picker/package.json", PICKER),
                (
                    "node_modules/picker/index.d.ts",
                    "import \"./other\";\nimport \"@typescript/lib-dom\";\n",
                ),
                ("node_modules/picker/other.d.ts", "export {}\n"),
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                ("node_modules/@typescript/lib-dom/index.d.ts", "export {}\n"),
            ],
            expected(es5, &["a.ts", other, dom, picker]),
            expected(es5, &[dom, "a.ts", other, picker]),
        ),
        (
            "a type package references the replacement's path",
            "picker",
            r#""es5", "dom""#,
            vec![
                ("node_modules/picker/package.json", PICKER),
                (
                    "node_modules/picker/index.d.ts",
                    "/// <reference path=\"./other.d.ts\" />\n/// <reference path=\"../@typescript/lib-dom/index.d.ts\" />\n",
                ),
                ("node_modules/picker/other.d.ts", "export {}\n"),
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                ("node_modules/@typescript/lib-dom/index.d.ts", "export {}\n"),
            ],
            expected(es5, &["a.ts", other, dom, picker]),
            expected(es5, &[dom, "a.ts", other, picker]),
        ),
        (
            "a library references a path another library imports",
            "picker",
            r#""es5", "dom", "scripthost""#,
            vec![
                ("node_modules/picker/package.json", PICKER),
                ("node_modules/picker/index.d.ts", "export {}\n"),
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                (
                    "node_modules/@typescript/lib-dom/index.d.ts",
                    "/// <reference path=\"./shared.d.ts\" />\n",
                ),
                (
                    "node_modules/@typescript/lib-dom/shared.d.ts",
                    "export {}\n",
                ),
                (
                    "node_modules/@typescript/lib-scripthost/package.json",
                    LIB_SCRIPTHOST,
                ),
                (
                    "node_modules/@typescript/lib-scripthost/index.d.ts",
                    "import \"../lib-dom/shared\";\n",
                ),
            ],
            expected(es5, &[shared, dom, scripthost, "a.ts", picker]),
            expected(es5, &[dom, scripthost, "a.ts", shared, picker]),
        ),
        (
            "a library imports a file another library references by path",
            "picker",
            r#""es5", "dom", "scripthost""#,
            vec![
                ("node_modules/picker/package.json", PICKER),
                ("node_modules/picker/index.d.ts", "export {}\n"),
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                (
                    "node_modules/@typescript/lib-dom/index.d.ts",
                    "import \"./shared\";\n",
                ),
                (
                    "node_modules/@typescript/lib-dom/shared.d.ts",
                    "export {}\n",
                ),
                (
                    "node_modules/@typescript/lib-scripthost/package.json",
                    LIB_SCRIPTHOST,
                ),
                (
                    "node_modules/@typescript/lib-scripthost/index.d.ts",
                    "/// <reference path=\"../lib-dom/shared.d.ts\" />\n",
                ),
            ],
            expected(es5, &[dom, scripthost, "a.ts", picker, shared]),
            expected(es5, &[dom, scripthost, "a.ts", shared, picker]),
        ),
        (
            "a replacement resolves to the installed library",
            "",
            r#""es5", "scripthost", "dom""#,
            vec![(
                "node_modules/@typescript/lib-dom/package.json",
                r#"{ "name": "@typescript/lib-dom", "types": "../../typescript/lib/lib.dom.d.ts" }"#,
            )],
            expected(
                "lib.es5.d.ts lib.dom.d.ts lib.scripthost.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts",
                &["a.ts"],
            ),
            expected(
                "lib.es5.d.ts lib.scripthost.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts",
                &["node_modules/typescript/lib/lib.dom.d.ts", "a.ts"],
            ),
        ),
        (
            "a replacement references a path beneath the installed lib",
            "",
            r#""es5", "dom", "scripthost""#,
            vec![
                ("node_modules/@typescript/lib-dom/package.json", LIB_DOM),
                ("node_modules/@typescript/lib-dom/index.d.ts", "export {}\n"),
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
                    "export {}\n",
                ),
            ],
            expected(
                "lib.es5.d.ts nested/lib.es2015.core.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts",
                &[dom, scripthost, "a.ts"],
            ),
            expected(
                es5,
                &[
                    dom,
                    scripthost,
                    "a.ts",
                    "node_modules/typescript/lib/nested/lib.es2015.core.d.ts",
                ],
            ),
        ),
    ];
    for (name, types, lib, files, typescript_5_6, typescript_7) in cases {
        let types = if types.is_empty() {
            String::new()
        } else {
            format!(r#""{types}""#)
        };
        let options = format!(
            r#""types": [{types}], "strict": true, "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true, "target": "ES2017", "lib": [{lib}]"#
        );
        let five = format!(r#"{{ "compilerOptions": {{ {options} }}, "files": ["a.ts"] }}"#);
        let seven = format!(
            r#"{{ "compilerOptions": {{ {options}, "libReplacement": true }}, "files": ["a.ts"] }}"#
        );
        let mut all = vec![
            ("a.ts", "export {}\n"),
            ("five.json", five.as_str()),
            ("seven.json", seven.as_str()),
        ];
        all.extend(files);
        let project = Project::new(&all);
        project.install_compiler(&package);
        assert_eq!(
            listed(&project.load("five.json")),
            typescript_5_6,
            "5.6.3: {name}"
        );
        project.write(&[(MANIFEST, &manifest("7.0.2"))]);
        assert_eq!(
            listed(&project.load("seven.json")),
            typescript_7,
            "7.0.2: {name}"
        );
    }
}

/// Under TypeScript 7 every replacement is ordered last, even one that
/// resolves into this build's vendored declarations, which only stand in
/// for 7.0.2's own directory: native 7.0.2's `tsc --listFilesOnly` under
/// `libReplacement` lists the scripthost replacement and then the vendored
/// `lib.dom.d.ts` after the located libraries, in `lib` order.
#[test]
fn a_typescript_7_replacement_in_the_vendored_directory_is_ordered_last() {
    let vendored = paths::combine(&libraries::vendored_directory(), "lib.dom.d.ts");
    let manifest = format!(
        r#"{{ "name": "@typescript/lib-dom", "types": {} }}"#,
        serde_json::to_string(&vendored).unwrap()
    );
    let project = Project::new(&[
        ("a.ts", "export {}\n"),
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "types": [], "lib": ["es5", "scripthost", "dom"], "libReplacement": true }, "files": ["a.ts"] }"#,
        ),
        ("node_modules/@typescript/lib-dom/package.json", &manifest),
        (
            "node_modules/@typescript/lib-scripthost/package.json",
            r#"{ "name": "@typescript/lib-scripthost", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/@typescript/lib-scripthost/index.d.ts",
            "export {}\n",
        ),
    ]);
    assert_eq!(
        listed(&project.load("tsconfig.json")),
        expected(
            "lib.es5.d.ts lib.es2015.d.ts lib.es2015.core.d.ts lib.es2015.collection.d.ts lib.es2015.generator.d.ts lib.es2015.iterable.d.ts lib.es2015.promise.d.ts lib.es2015.proxy.d.ts lib.es2015.reflect.d.ts lib.es2015.symbol.d.ts lib.es2015.symbol.wellknown.d.ts lib.es2018.asynciterable.d.ts lib.decorators.d.ts lib.decorators.legacy.d.ts",
            &[
                "node_modules/@typescript/lib-scripthost/index.d.ts",
                "lib.dom.d.ts",
                "a.ts"
            ]
        )
    );
}

/// TypeScript keeps an empty directive value and reports it where it looks
/// for the target: TS6231 for `path`, TS2688 for `types`, TS2726 for `lib`.
#[test]
fn empty_reference_values_name_missing_targets() {
    let project = Project::new(&[
        ("path.ts", "/// <reference path=\"\" />\nexport {}\n"),
        ("types.ts", "\n/// <reference types=\"\" />\nexport {}\n"),
        (
            "suppressed.ts",
            "// @ts-ignore\n/// <reference path=\"\" />\nexport {}\n",
        ),
        (
            "path.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["path.ts"] }"#,
        ),
        (
            "types.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["types.ts"] }"#,
        ),
        (
            "suppressed.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["suppressed.ts"] }"#,
        ),
    ]);
    let path = project.error("path.json");
    assert_eq!(
        (path.kind, path.position),
        (InputErrorKind::UnresolvedImport, position(1, 22))
    );
    let types = project.error("types.json");
    assert_eq!(
        (types.kind, types.position, types.message.as_str()),
        (
            InputErrorKind::UnresolvedImport,
            position(2, 23),
            "Cannot find type definition file for ''"
        )
    );
    assert_eq!(listed(&project.load("suppressed.json")), ["suppressed.ts"]);
}

/// TypeScript's `convertJsonOptionOfEnumType` leaves an option unset for an
/// empty string, without a diagnostic, and lowers others with Go's
/// `strings.ToLower` (observed with `tsc --showConfig`).
#[test]
fn empty_enumeration_values_leave_options_unset() {
    let project = configurations(&[
        (
            "empty.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "", "moduleResolution": "", "jsx": "", "moduleDetection": "", "target": "" }, "files": ["a.ts"] }"#,
        ),
        (
            "upper.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "ESNEXT", "target": "ES2022" }, "files": ["a.ts"] }"#,
        ),
    ]);
    for config in ["empty.json", "upper.json"] {
        assert_eq!(listed(&project.load(config)), ["a.ts"], "{config}");
    }
}

/// A standard library, vendored or replaced, is never a project file, so a
/// missing reference in it is not reported (`Diagnostics::reports`),
/// whether or not TypeScript's `SkipTypeChecking` covers it as well.
#[test]
fn library_files_tolerate_missing_references() {
    let config = |options: &str| {
        format!(
            r#"{{ "compilerOptions": {{ "libReplacement": true, "lib": ["es5", "dom"]{options} }}, "files": ["a.ts"] }}"#
        )
    };
    let project = Project::new(&[
        ("a.ts", "replacedDom;\n"),
        (
            "node_modules/@typescript/lib-dom/package.json",
            r#"{ "name": "@typescript/lib-dom", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/@typescript/lib-dom/index.d.ts",
            "/// <reference lib=\"bogus\" />\ndeclare var replacedDom: number;\n",
        ),
        ("checked.json", &config("")),
        ("default.json", &config(r#", "skipDefaultLibCheck": true"#)),
        ("declarations.json", &config(r#", "skipLibCheck": true"#)),
    ]);
    for config in ["checked.json", "default.json", "declarations.json"] {
        assert_eq!(
            listed(&project.load(config)),
            [
                "lib.es5.d.ts",
                "lib.decorators.d.ts",
                "lib.decorators.legacy.d.ts",
                "node_modules/@typescript/lib-dom/index.d.ts",
                "a.ts"
            ],
            "{config}"
        );
    }
}

/// TypeScript's program order (`collectFiles`, as `tsc --listFilesOnly`
/// prints it): libraries by priority, then every other file after the files
/// its subtasks reach, depth first from the root tasks in order. A file's
/// subtasks are its `/// <reference path>`s, `types` and `lib` references,
/// the automatic JSX runtime, its static imports, then its dynamic imports
/// and import types. Each list below is the pinned compiler's.
#[test]
fn program_order_lists_each_file_after_its_subtasks() {
    /// A case's name, files, and the compiler's program order.
    type Case<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "import chain",
            &[
                (
                    "tsconfig.json",
                    r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts"] }"#,
                ),
                ("a.ts", "import \"./b\";\nexport {};\n"),
                ("b.ts", "import \"./c\";\nexport {};\n"),
                ("c.ts", "export {};\n"),
            ],
            &["c.ts", "b.ts", "a.ts"],
        ),
        (
            "diamond",
            &[
                (
                    "tsconfig.json",
                    r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts", "e.ts"] }"#,
                ),
                ("a.ts", "import \"./b\";\nimport \"./c\";\nexport {};\n"),
                ("b.ts", "import \"./d\";\nexport {};\n"),
                ("c.ts", "import \"./d\";\nexport {};\n"),
                ("d.ts", "export {};\n"),
                ("e.ts", "import \"./c\";\nexport {};\n"),
            ],
            &["d.ts", "b.ts", "c.ts", "a.ts", "e.ts"],
        ),
        (
            "references before imports",
            &[
                (
                    "tsconfig.json",
                    r#"{ "compilerOptions": { "lib": ["es5"] }, "files": ["a.ts"] }"#,
                ),
                (
                    "a.ts",
                    "/// <reference lib=\"es2015.promise\" />\n/// <reference types=\"t\" />\n/// <reference path=\"./p.ts\" />\nimport \"./i\";\nexport {};\n",
                ),
                ("i.ts", "export {};\n"),
                ("p.ts", "declare const p: number;\n"),
                (
                    "node_modules/@types/t/index.d.ts",
                    "declare const t: number;\n",
                ),
            ],
            &[
                "lib.es5.d.ts",
                "lib.es2015.promise.d.ts",
                "lib.decorators.d.ts",
                "lib.decorators.legacy.d.ts",
                "p.ts",
                "node_modules/@types/t/index.d.ts",
                "i.ts",
                "a.ts",
            ],
        ),
        (
            "static imports before dynamic imports and import types",
            &[
                (
                    "tsconfig.json",
                    r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts"] }"#,
                ),
                (
                    "a.ts",
                    "type T = import(\"./t\");\nimport \"./s\";\nexport const d = import(\"./d\");\n",
                ),
                ("t.ts", "export {};\n"),
                ("s.ts", "export {};\n"),
                ("d.ts", "export {};\n"),
            ],
            &["s.ts", "t.ts", "d.ts", "a.ts"],
        ),
        (
            // `ForEachDynamicImportOrRequireCall` finds them by their text:
            // an `else` branch, a call's arguments, a conditional's second
            // branch, an operator's right operand, a variable's initializer
            // after its type, and a return type after a parameter's come
            // later. Command Center's `scripts/probes/checkpoint-handoff.ts`
            // has the `else if` chain.
            "dynamic imports and import types in source order",
            &[
                (
                    "tsconfig.json",
                    r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts"] }"#,
                ),
                (
                    "a.ts",
                    "declare const c: boolean;\nexport async function f() {\n  if (c) await import(\"./i1\");\n  else if (!c) await import(\"./i2\");\n  else await import(\"./i3\");\n  (await import(\"./call\")).f(await import(\"./arg\"));\n  const v: import(\"./vt\") = c ? await import(\"./then\") : await import(\"./else\");\n  return import(\"./left\") || import(\"./right\");\n}\ntype F = (p: import(\"./param\")) => import(\"./ret\");\n",
                ),
                ("i1.ts", "export {};\n"),
                ("i2.ts", "export {};\n"),
                ("i3.ts", "export {};\n"),
                ("call.ts", "export const f = (x?: unknown) => x;\n"),
                ("arg.ts", "export {};\n"),
                ("vt.ts", "export {};\n"),
                ("then.ts", "export {};\n"),
                ("else.ts", "export {};\n"),
                ("left.ts", "export {};\n"),
                ("right.ts", "export {};\n"),
                ("param.ts", "export {};\n"),
                ("ret.ts", "export {};\n"),
            ],
            &[
                "i1.ts", "i2.ts", "i3.ts", "call.ts", "arg.ts", "vt.ts", "then.ts", "else.ts",
                "left.ts", "right.ts", "param.ts", "ret.ts", "a.ts",
            ],
        ),
        (
            "a script's ambient module imports",
            &[
                (
                    "tsconfig.json",
                    r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts"] }"#,
                ),
                (
                    "a.ts",
                    "type Q = import(\"./q\");\ndeclare module \"w\" { import \"y\"; }\n",
                ),
                ("q.ts", "export {};\n"),
                (
                    "node_modules/y/package.json",
                    r#"{ "name": "y", "types": "index.d.ts" }"#,
                ),
                ("node_modules/y/index.d.ts", "export {};\n"),
            ],
            &["node_modules/y/index.d.ts", "q.ts", "a.ts"],
        ),
        (
            "type packages after the roots",
            &[
                (
                    "tsconfig.json",
                    r#"{ "compilerOptions": { "noLib": true, "types": ["t", "u"] }, "files": ["a.ts"] }"#,
                ),
                ("a.ts", "import \"./b\";\nexport {};\n"),
                ("b.ts", "export {};\n"),
                (
                    "node_modules/@types/t/index.d.ts",
                    "/// <reference types=\"u\" />\ndeclare const t: number;\n",
                ),
                (
                    "node_modules/@types/u/index.d.ts",
                    "declare const u: number;\n",
                ),
            ],
            &[
                "b.ts",
                "a.ts",
                "node_modules/@types/u/index.d.ts",
                "node_modules/@types/t/index.d.ts",
            ],
        ),
        (
            "the JSX runtime before imports",
            &[
                (
                    "tsconfig.json",
                    r#"{ "compilerOptions": { "noLib": true, "jsx": "react-jsx" }, "files": ["a.tsx"] }"#,
                ),
                ("a.tsx", "import \"./i\";\nexport const x = <div />;\n"),
                ("i.ts", "export {};\n"),
                (
                    "node_modules/react/package.json",
                    r#"{ "name": "react", "types": "index.d.ts" }"#,
                ),
                ("node_modules/react/index.d.ts", "export {};\n"),
                (
                    "node_modules/react/jsx-runtime.d.ts",
                    "export {};\ndeclare global { namespace JSX { interface IntrinsicElements { div: any } } }\n",
                ),
            ],
            &["node_modules/react/jsx-runtime.d.ts", "i.ts", "a.tsx"],
        ),
        (
            "a library a dependency references",
            &[
                (
                    "tsconfig.json",
                    r#"{ "compilerOptions": { "lib": [] }, "files": ["a.ts"] }"#,
                ),
                ("a.ts", "import \"dep\";\n"),
                (
                    "node_modules/dep/index.d.ts",
                    "/// <reference lib=\"es2015.promise\" />\nexport {};\n",
                ),
            ],
            &[
                "lib.es2015.promise.d.ts",
                "node_modules/dep/index.d.ts",
                "a.ts",
            ],
        ),
    ];
    for (case, files, order) in cases {
        let project = Project::new(files);
        assert_eq!(listed(&project.load("tsconfig.json")), *order, "{case}");
    }
    // TypeScript adds no file for an augmentation's target (`shouldAddFile`;
    // TS2664 if nothing else includes it). Here `b.ts` imports `m.ts`, so
    // the compiler lists `i.ts a.ts m.ts b.ts`, and without `b.ts` only
    // `i.ts a.ts`.
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts", "b.ts"] }"#,
        ),
        (
            "a.ts",
            "declare module \"./m\" { interface X {} }\nimport \"./i\";\nexport {};\n",
        ),
        ("i.ts", "export {};\n"),
        ("m.ts", "export interface X {}\n"),
        ("b.ts", "import \"./m\";\nexport {};\n"),
        (
            "only.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts"] }"#,
        ),
    ]);
    assert_eq!(
        listed(&project.load("tsconfig.json")),
        ["i.ts", "a.ts", "m.ts", "b.ts"]
    );
    assert_eq!(listed(&project.load("only.json")), ["i.ts", "a.ts"]);
}

/// One recorded answer of `tools/json-probes.mjs` for a watched identifier,
/// `<line>:<column> <name> -> <target>`, as this tool's documented model
/// expects it: no symbol is `unresolved`; a JSON module's value or property,
/// or a property or method of an interface, is a member, which is
/// `unsupported`; any other symbol is a group with the recorded declarations.
fn probe_expectation(target: &str) -> (&'static str, Vec<String>) {
    let Some(declarations) = target
        .split_once(" [")
        .and_then(|(_, rest)| rest.strip_suffix(']'))
    else {
        assert_eq!(target, "none", "an unexpected target");
        return ("unresolved", Vec::new());
    };
    let declarations: Vec<&str> = declarations
        .split(", ")
        .filter(|declaration| !declaration.is_empty())
        .collect();
    if declarations.is_empty() {
        return ("unresolved", Vec::new());
    }
    let member = declarations.iter().any(|declaration| {
        let (place, kind) = declaration.rsplit_once(' ').unwrap();
        matches!(
            kind,
            "PropertyAssignment" | "PropertySignature" | "MethodSignature"
        ) || kind == "SourceFile" && place.ends_with(".json")
    });
    if member {
        return ("unsupported", Vec::new());
    }
    let places = declarations
        .iter()
        .map(|declaration| declaration.rsplit_once(' ').unwrap().0.to_owned())
        .collect();
    ("declared", places)
}

/// The program as `tools/json-probes.mjs` records it: `GetSourceFiles` order,
/// a standard library as `lib:<name>`, and an external library's file marked.
fn probe_program(program: &Program) -> Vec<String> {
    let vendored = std::fs::canonicalize(libraries::vendored_directory()).unwrap();
    let vendored = paths::from_path(&vendored).unwrap();
    program
        .sources
        .ids()
        .map(|id| {
            let source = program.sources.get(id);
            match source.path.strip_prefix(&vendored) {
                Ok(name) => format!("lib:{}", name.to_string_lossy()),
                Err(_) if program.files[id.index()].search => source.display.clone(),
                Err(_) => format!("{} (external)", source.display),
            }
        })
        .collect()
}

/// The maintained compiler probes, `fixtures/json-probes/*.json`: small
/// projects whose program, diagnostics, and watched names the pinned
/// TypeScript 7.0.2 compiler reports, recorded under `native` by
/// `tools/json-probes.mjs`, which re-derives them (docs/validation.md). This
/// tool must load the recorded program and answer each watched name as the
/// documented model expects (`probe_expectation`), or, where a case's
/// `tool.error` names an error kind and the compiler diagnostic (or
/// position) it reports, fail there. `tool.names` overrides the expected
/// answer at a position where docs/references.md records a difference, which
/// `tool.note` names.
#[test]
fn compiler_probe_projects_load_and_answer_as_recorded() {
    use crate::analysis::{AnalysisError, QueryErrorKind};
    use crate::references::ReferenceQuery;
    use crate::source::SourcePosition;
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/json-probes");
    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    fixtures.sort();
    assert!(!fixtures.is_empty());
    let mut failures = Vec::new();
    let mut checked = 0;
    for fixture_path in fixtures {
        let fixture: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&fixture_path).unwrap()).unwrap();
        let fixture_name = fixture_path.file_name().unwrap().to_string_lossy();
        for case in fixture["cases"].as_array().unwrap() {
            checked += 1;
            let name = format!("{fixture_name} {}", case["name"].as_str().unwrap());
            let mut files: Vec<(&str, &str)> = case["files"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(path, text)| (path.as_str(), text.as_str().unwrap()))
                .collect();
            if !files.iter().any(|(path, _)| *path == "tsconfig.json") {
                files.push(("tsconfig.json", fixture["config"].as_str().unwrap()));
            }
            let project = Project::new(&files);
            // A case's answers hold on a case-insensitive file system; its
            // `caseSensitive` answers replace them where names keep their case.
            let answers = match (Case::detect(), case.get("caseSensitive")) {
                (Case::Sensitive, Some(sensitive)) => sensitive,
                (Case::Sensitive, None) | (Case::Insensitive, _) => case,
            };
            let native = &answers["native"];
            let tool = &answers["tool"];
            // Loading includes binding, which settles missing modules.
            let loaded = crate::project::Project::load(&project.root.join("tsconfig.json"));
            if let Some(error) = tool["error"].as_array() {
                let kind = error[0].as_str().unwrap();
                let place = error[1].as_str().unwrap();
                let place = match place.strip_prefix("TS") {
                    Some(code) => native["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|diagnostic| diagnostic.as_str())
                        .find_map(|diagnostic| {
                            diagnostic
                                .strip_prefix(&format!("TS{code} "))
                                .map(str::to_owned)
                        })
                        .unwrap_or_else(|| panic!("{name}: no TS{code} is recorded")),
                    None => place.to_owned(),
                };
                match loaded {
                    Ok(_) => failures.push(format!("{name}: loaded; expected {kind} at {place}")),
                    Err(crate::analysis::AnalysisError::Input(error)) => {
                        let actual = format!(
                            "{:?} {}:{}:{}",
                            error.kind,
                            error
                                .path
                                .as_deref()
                                .map(|path| project.display(path))
                                .unwrap_or_default(),
                            error.position.map_or(0, |position| position.line),
                            error.position.map_or(0, |position| position.column),
                        );
                        if actual != format!("{kind} {place}") {
                            failures.push(format!(
                                "{name}: failed as {actual} ({}); expected {kind} {place}",
                                error.message
                            ));
                        }
                    }
                    Err(error) => failures.push(format!(
                        "{name}: non-input failure {error:?}; expected {kind} at {place}"
                    )),
                }
                continue;
            }
            let references = match loaded {
                Ok(references) => references,
                Err(error) => {
                    failures.push(format!("{name}: failed to load: {error}"));
                    continue;
                }
            };
            let program = project.load("tsconfig.json");
            let expected: Vec<&str> = native["program"]
                .as_array()
                .unwrap()
                .iter()
                .map(|file| file.as_str().unwrap())
                .collect();
            let actual = probe_program(&program);
            if actual != expected {
                failures.push(format!("{name}: program {actual:?}; expected {expected:?}"));
            }
            let Some(names) = native["names"].as_array() else {
                continue;
            };
            let watch_file = case["watchFile"].as_str().unwrap_or_else(|| {
                if files.iter().any(|(path, _)| *path == "main.ts") {
                    "main.ts"
                } else {
                    "main.d.ts"
                }
            });
            for recorded in names {
                let recorded = recorded.as_str().unwrap();
                let (at, target) = recorded.split_once(" -> ").unwrap();
                let (place, _) = at.split_once(' ').unwrap();
                let (line, column) = place.split_once(':').unwrap();
                let (mut expected, places) = probe_expectation(target);
                if let Some(answer) = tool["names"][place].as_str() {
                    expected = match answer {
                        "unresolved" => "unresolved",
                        "unsupported" => "unsupported",
                        "declared" => "declared",
                        other => panic!("{name}: unknown answer {other}"),
                    };
                }
                let result = references.find_references(ReferenceQuery::At {
                    file: project.root.join(watch_file),
                    position: SourcePosition {
                        line: line.parse().unwrap(),
                        column: column.parse().unwrap(),
                    },
                });
                let actual = match &result {
                    Err(AnalysisError::Query(error))
                        if error.kind == QueryErrorKind::UnresolvedTarget =>
                    {
                        "unresolved"
                    }
                    Err(AnalysisError::Query(error))
                        if error.kind == QueryErrorKind::UnsupportedTarget =>
                    {
                        "unsupported"
                    }
                    Err(error) => panic!("{name}: {recorded}: {error}"),
                    Ok(_) => "declared",
                };
                // The compiler records a declaration node's start, this tool
                // its name, so they are compared by file and line.
                let declared = |result: &crate::references::ReferenceResult| {
                    places.iter().all(|place| {
                        let (file_line, _) = place.rsplit_once(':').unwrap();
                        result.groups.iter().any(|group| {
                            group.declarations.iter().any(|declaration| {
                                let location = &declaration.location;
                                format!("{}:{}", location.file, location.start.line) == file_line
                            })
                        })
                    })
                };
                let matches = actual == expected
                    && (expected != "declared"
                        || tool["names"][place].is_string()
                        || result.as_ref().is_ok_and(declared));
                if !matches {
                    failures.push(format!(
                        "{name}: {recorded}: {}; expected {expected}",
                        match &result {
                            Ok(result) => format!(
                                "{:?}",
                                result
                                    .groups
                                    .iter()
                                    .map(|group| group
                                        .declarations
                                        .iter()
                                        .map(|d| format!(
                                            "{}:{}:{}",
                                            d.location.file,
                                            d.location.start.line,
                                            d.location.start.column
                                        ))
                                        .collect::<Vec<_>>())
                                    .collect::<Vec<_>>()
                            ),
                            Err(error) => error.to_string(),
                        }
                    ));
                }
            }
        }
    }
    assert!(checked > 0);
    assert!(
        failures.is_empty(),
        "{} of {checked} probe cases differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Evidence for a real project, not a regression: the inputs a
/// configuration selects, in program order, each with its provenance, for
/// comparison with the pinned compiler's `tsc --listFilesOnly` and its
/// API's `getSourceFileMetadata` (`docs/validation.md`, Command Center
/// checkpoint), and each file that redirects to an input as a duplicate of
/// its package, with that input, for comparison with 5.6.3's
/// `redirectInfo`. `TSPARSE_PROGRAM_CONFIG` names the configuration and
/// `TSPARSE_PROGRAM_LISTING` the JSON file to write.
#[test]
#[ignore = "needs TSPARSE_PROGRAM_CONFIG and TSPARSE_PROGRAM_LISTING"]
fn list_configured_program() {
    let config = std::env::var_os("TSPARSE_PROGRAM_CONFIG").expect("TSPARSE_PROGRAM_CONFIG");
    let output = std::env::var_os("TSPARSE_PROGRAM_LISTING").expect("TSPARSE_PROGRAM_LISTING");
    let program = load_on_parsing_thread(Path::new(&config))
        .unwrap_or_else(|error| panic!("{} failed to load: {error}", config.display()));
    // Only a standard library is named by its file name, as the compiler
    // comparison names it: another file in the compiler's `lib` directory,
    // such as `typescript.d.ts`, keeps its path.
    let files: Vec<serde_json::Value> = listed(&program)
        .into_iter()
        .zip(program.sources.ids().zip(&program.files))
        .map(|(file, (id, input))| {
            let (origin, file) = match input.diagnostics.origin() {
                FileOrigin::Project => ("project", program.sources.get(id).display.clone()),
                FileOrigin::Dependency => ("dependency", program.sources.get(id).display.clone()),
                FileOrigin::DefaultLibrary => ("library", file),
            };
            serde_json::json!({ "file": file, "origin": origin })
        })
        .collect();
    let directory = program.config.parent().unwrap().to_str().unwrap();
    let redirects: Vec<serde_json::Value> = program
        .redirects
        .iter()
        .map(|redirect| {
            serde_json::json!({
                "file": paths::relative(directory, redirect.path.to_str().unwrap()),
                "target": program.sources.get(redirect.target).display,
            })
        })
        .collect();
    let listing = serde_json::json!({
        "config": program.config,
        "files": files,
        "redirects": redirects,
    });
    std::fs::write(&output, serde_json::to_vec_pretty(&listing).unwrap()).unwrap();
}

/// Compare exact diagnostic spellings after loading the native fixture. Public
/// Q4 queries assert error-type admission separately in type_module_spelling;
/// diagnostic wording is intentionally absent from that public result.
#[test]
fn fresh_module_spelling_matches_native_loaded_inputs() {
    use super::specifiers::{ImportingFile, WrittenImport, module_specifiers};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/native-module-spelling");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join("cases.json")).unwrap()).unwrap();
    let native: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(root.join("expected-workspace.json")).unwrap(),
    )
    .unwrap();
    let globals = std::fs::read_to_string(root.join("globals.d.ts")).unwrap();
    assert_eq!(cases.len(), native.len());
    for (case, expected) in cases.iter().zip(native) {
        let name = case["name"].as_str().unwrap();
        assert_eq!(expected["name"], name);
        let files = case["files"].as_object().unwrap();
        let mut inputs: Vec<_> = files
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str().unwrap()))
            .collect();
        inputs.push(("globals.d.ts", &globals));
        let fixture = Project::new(&inputs);
        let config = case["config"].as_str().unwrap();
        let program = fixture.load(config);
        let source_path =
            std::fs::canonicalize(fixture.root.join(case["source"].as_str().unwrap())).unwrap();
        let id = program
            .sources
            .ids()
            .find(|&id| program.sources.get(id).path == source_path)
            .unwrap();
        let file = &program.files[id.index()];
        let imports: Vec<_> = file
            .facts
            .requests
            .iter()
            .filter(|request| request.implicit.is_none())
            .map(|request| WrittenImport {
                text: request.specifier.clone(),
                resolved_file_name: request.resolved_path.clone(),
                mode: request.resolution_mode,
            })
            .collect();
        let target = imports
            .first()
            .unwrap()
            .resolved_file_name
            .as_deref()
            .unwrap();
        let mode = file.module_semantics.default_resolution_mode;
        assert_ne!(
            imports.first().unwrap().mode,
            mode,
            "{name}: must generate a fresh spelling"
        );
        let actual = module_specifiers(
            &program.specifier_environment,
            ImportingFile {
                file_name: source_path.to_str().unwrap(),
                default_mode: mode,
                imports: &imports,
            },
            target,
            mode,
            None,
        )
        .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let spelling: String = serde_json::from_str(expected["display"].as_str().unwrap()).unwrap();
        assert_eq!(actual.first(), Some(&spelling), "{name}: {actual:?}");
    }
}

/// Native root/subpath resolution and reverse spelling ask different questions
/// of the same ordered manifest values. Public Q4 tests assert the selected
/// values and error-type boundary; this checks loaded files and exact strings.
#[test]
fn manifest_edges_match_native_loaded_files_and_fresh_spellings() {
    use super::specifiers::{ImportingFile, WrittenImport, module_specifiers};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/native-manifest-edges");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join("cases.json")).unwrap()).unwrap();
    let native: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(root.join("expected-workspace.json")).unwrap(),
    )
    .unwrap();
    let template = std::fs::read_to_string(root.join("main.cts")).unwrap();
    let globals = std::fs::read_to_string(root.join("globals.d.ts")).unwrap();
    assert_eq!(cases.len(), native.len());
    for (case, expected) in cases.iter().zip(native) {
        let name = case["name"].as_str().unwrap();
        assert_eq!(expected["name"], name);
        let source = template.replace(
            "from \"pkg\"",
            &format!(
                "from {}",
                serde_json::to_string(case["specifier"].as_str().unwrap_or("pkg")).unwrap()
            ),
        );
        let fixture = Project::new(&[
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"strict":true,"noLib":true,"types":[],"module":"NodeNext","moduleResolution":"NodeNext"},"files":["globals.d.ts","main.cts"]}"#,
            ),
            ("package.json", r#"{"type":"commonjs"}"#),
            ("globals.d.ts", &globals),
            ("main.cts", &source),
            (
                "node_modules/pkg/package.json",
                case["manifest"].as_str().unwrap(),
            ),
            (
                "node_modules/pkg/index.d.ts",
                "export declare const present: \"base\";\n",
            ),
            (
                "node_modules/pkg/first.d.ts",
                "export declare const present: \"first\";\n",
            ),
            (
                "node_modules/pkg/second.d.ts",
                "export declare const present: \"second\";\n",
            ),
        ]);
        let program = fixture.load("tsconfig.json");
        let mut loaded: Vec<_> = program
            .sources
            .ids()
            .map(|id| fixture.display(&program.sources.get(id).path))
            .filter(|path| path.starts_with("node_modules/pkg/"))
            .collect();
        loaded.sort();
        assert_eq!(
            serde_json::to_value(loaded).unwrap(),
            expected["selected"]["loaded"],
            "{name}: final loaded files"
        );
        let source_path = std::fs::canonicalize(fixture.root.join("main.cts")).unwrap();
        let id = program
            .sources
            .ids()
            .find(|&id| program.sources.get(id).path == source_path)
            .unwrap();
        let file = &program.files[id.index()];
        let imports: Vec<_> = file
            .facts
            .requests
            .iter()
            .filter(|request| request.implicit.is_none())
            .map(|request| WrittenImport {
                text: request.specifier.clone(),
                resolved_file_name: request.resolved_path.clone(),
                mode: request.resolution_mode,
            })
            .collect();
        let target = imports
            .first()
            .unwrap()
            .resolved_file_name
            .as_deref()
            .unwrap();
        let mode = file.module_semantics.default_resolution_mode;
        assert_ne!(
            imports.first().unwrap().mode,
            mode,
            "{name}: must generate a fresh spelling"
        );
        let actual = module_specifiers(
            &program.specifier_environment,
            ImportingFile {
                file_name: source_path.to_str().unwrap(),
                default_mode: mode,
                imports: &imports,
            },
            target,
            mode,
            None,
        )
        .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let spelling: String = serde_json::from_str(expected["display"].as_str().unwrap()).unwrap();
        assert_eq!(actual.first(), Some(&spelling), "{name}: {actual:?}");
    }
}

/// The checker's fresh spelling of `target`'s module from `importer`, both
/// display paths of `program`'s project, with no written import to reuse.
fn spelled(fixture: &Project, program: &Program, importer: &str, target: &str) -> Vec<String> {
    use super::specifiers::{ImportingFile, module_specifiers};
    let path = |display: &str| {
        let (id, _) = file(program, display);
        program.sources.get(id).path.to_str().unwrap().to_owned()
    };
    let (id, _) = file(program, importer);
    let mode = program.files[id.index()]
        .module_semantics
        .default_resolution_mode;
    let importer = path(importer);
    module_specifiers(
        &program.specifier_environment,
        ImportingFile {
            file_name: &importer,
            default_mode: mode,
            imports: &[],
        },
        &path(target),
        mode,
        None,
    )
    .unwrap_or_else(|error| panic!("{}: {error:?}", fixture.root.display()))
}

/// Reverse naming reads `typesVersions` and `types@` conditions with the
/// same compiler version as resolution: the installed one under the 5.6
/// dialect, at, below and above a range, and 7.0.2 under TypeScript 7.
/// 5.6.3's `getModuleSpecifiers` with the checker's preferences
/// (`tools/ts56-oracle.mjs` `specifier`) gives the matching range's
/// spellings and, for a range no version matches, the others.
#[test]
fn module_spelling_reads_package_ranges_with_the_compiler_version() {
    for (version, matches) in [
        ("5.6.3", true),
        ("5.6.2", false),
        ("5.7.0", false),
        ("7.0.2", false),
    ] {
        let manifest = manifest(version);
        let project = Project::new(&[
            (
                "tsconfig.json",
                &format!(
                    r#"{{"compilerOptions":{{{BASE_OPTIONS}}},"files":["main.ts","other.ts"]}}"#
                ),
            ),
            (MANIFEST, &manifest),
            (
                "main.ts",
                "/// <reference path=\"node_modules/tv/ts56/sub.d.ts\" />\n/// <reference path=\"node_modules/cond/ts56.d.ts\" />\nimport \"tv/sub\";\nimport \"cond\";\n",
            ),
            ("other.ts", "export const other = 1;\n"),
            (
                "node_modules/tv/package.json",
                r#"{"name":"tv","version":"1.0.0","typesVersions":{">=5.6.3 <5.7":{"*":["ts56/*"]}}}"#,
            ),
            ("node_modules/tv/sub.d.ts", "export {};\n"),
            ("node_modules/tv/ts56/sub.d.ts", "export {};\n"),
            (
                "node_modules/cond/package.json",
                r#"{"name":"cond","version":"1.0.0","exports":{".":{"types@>=5.6.3 <5.7":"./ts56.d.ts","types":"./index.d.ts"}}}"#,
            ),
            ("node_modules/cond/ts56.d.ts", "export {};\n"),
            ("node_modules/cond/index.d.ts", "export {};\n"),
        ]);
        let program = project.load("tsconfig.json");
        let expected: &[(&str, &str)] = if matches {
            &[
                ("node_modules/tv/ts56/sub.d.ts", "tv/sub"),
                ("node_modules/cond/ts56.d.ts", "cond"),
            ]
        } else {
            &[
                ("node_modules/tv/ts56/sub.d.ts", "tv/ts56/sub"),
                ("node_modules/tv/sub.d.ts", "tv/sub"),
                ("node_modules/cond/ts56.d.ts", "./node_modules/cond/ts56"),
                ("node_modules/cond/index.d.ts", "cond"),
            ]
        };
        for (target, spelling) in expected {
            assert_eq!(
                spelled(&project, &program, "other.ts", target),
                [*spelling],
                "{version}: {target}"
            );
        }
    }
}

/// The same version decides a fresh import-mode spelling, which each
/// compiler shows when a CommonJS file's only imports of a target use the
/// `import` mode: a range only 5.6.3 admits and one only 7.0.2 admits each
/// map one dialect's name. Expected names are the TS2305 messages of
/// 5.6.3's `tsc` and native 7.0.2's for this layout with `"lib":
/// ["es2022"]` in place of `noLib`, which would stop `tsc` at TS2318 before
/// any semantic diagnostic, and an import of a missing member beside each
/// import (`Module '"tv7/ts7/sub.js"'` and `'"tv/sub.js"'` from 5.6.3;
/// `'"tv7/sub.js"'` and `'"tv/ts56/sub.js"'` from 7.0.2).
#[test]
fn import_mode_spelling_maps_the_dialect_versions_range() {
    use super::specifiers::{ImportingFile, module_specifiers};
    use crate::project::binding::ResolutionMode;
    for (version, expected) in [
        (
            "5.6.3",
            [
                ("node_modules/tv7/ts7/sub.d.ts", "tv7/ts7/sub.js"),
                ("node_modules/tv/ts56/sub.d.ts", "tv/sub.js"),
            ],
        ),
        (
            "7.0.2",
            [
                ("node_modules/tv7/ts7/sub.d.ts", "tv7/sub.js"),
                ("node_modules/tv/ts56/sub.d.ts", "tv/ts56/sub.js"),
            ],
        ),
    ] {
        let manifest = manifest(version);
        let project = Project::new(&[
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"noLib":true,"types":[],"strict":true,"target":"es2022","module":"nodenext","moduleResolution":"nodenext","esModuleInterop":true},"files":["use.cts"]}"#,
            ),
            (MANIFEST, &manifest),
            (
                "use.cts",
                "import type { present } from \"./node_modules/tv7/ts7/sub.js\" with { \"resolution-mode\": \"import\" };\nimport type { p2 } from \"./node_modules/tv/ts56/sub.js\" with { \"resolution-mode\": \"import\" };\n",
            ),
            (
                "node_modules/tv7/package.json",
                r#"{"name":"tv7","version":"1.0.0","typesVersions":{">=7":{"*":["ts7/*"]}}}"#,
            ),
            (
                "node_modules/tv7/ts7/sub.d.ts",
                "export declare const present: number;\n",
            ),
            ("node_modules/tv7/sub.d.ts", "export {};\n"),
            (
                "node_modules/tv/package.json",
                r#"{"name":"tv","version":"1.0.0","typesVersions":{">=5.6.3 <5.7":{"*":["ts56/*"]}}}"#,
            ),
            (
                "node_modules/tv/ts56/sub.d.ts",
                "export declare const p2: number;\n",
            ),
            ("node_modules/tv/sub.d.ts", "export {};\n"),
        ]);
        let program = project.load("tsconfig.json");
        let path = |display: &str| {
            let (id, _) = file(&program, display);
            program.sources.get(id).path.to_str().unwrap().to_owned()
        };
        let (id, _) = file(&program, "use.cts");
        let importer = path("use.cts");
        for (target, spelling) in expected {
            let spelled = module_specifiers(
                &program.specifier_environment,
                ImportingFile {
                    file_name: &importer,
                    default_mode: program.files[id.index()]
                        .module_semantics
                        .default_resolution_mode,
                    imports: &[],
                },
                &path(target),
                Some(ResolutionMode::Import),
                Some(ResolutionMode::Import),
            )
            .unwrap_or_else(|error| panic!("{version}: {target}: {error:?}"));
            assert_eq!(spelled, [spelling], "{version}: {target}");
        }
    }
}

/// `getLocalModuleSpecifier` with `baseUrl` (5.6 dialect): the path from
/// `baseUrl` is the non-relative name when no package import or `paths`
/// name applies, processed for its ending as 5.6.3 does, without asking
/// whether an `index` file collides with a sibling; the checker's
/// project-relative preference takes it only for a target across the
/// project's boundary or in another package scope, and a name that climbs
/// out of `baseUrl` stays relative. `baseUrl` may come from an inherited
/// configuration and is the base even when `paths` is empty. Expected
/// spellings are 5.6.3's `getModuleSpecifiers` with the checker's
/// preferences (`tools/ts56-oracle.mjs` `specifier`), from `src/deep/use.ts`
/// under `app/`; TypeScript 7 has no `baseUrl`.
#[test]
fn module_spelling_names_targets_from_base_url_under_5_6() {
    let options = BASE_OPTIONS;
    let include = r#""include":["src","../shared","scoped"]"#;
    for (name, version, config, expected) in [
        (
            "baseUrl above the project",
            "5.6.3",
            format!(r#"{{"compilerOptions":{{{options},"baseUrl":".."}},{include}}}"#),
            &[
                ("../shared/lib.ts", "shared/lib"),
                ("../shared/sub/index.ts", "shared/sub"),
                ("src/x.ts", "../x"),
                ("scoped/thing.ts", "app/scoped/thing"),
            ][..],
        ),
        (
            "a paths name before the baseUrl name",
            "5.6.3",
            format!(
                r#"{{"compilerOptions":{{{options},"baseUrl":"..","paths":{{"lib/*":["shared/*"]}}}},{include}}}"#
            ),
            &[("../shared/lib.ts", "lib/lib"), ("src/x.ts", "../x")][..],
        ),
        // An empty `paths` name, from an empty key or a `*` that matches
        // nothing, is still the `paths` answer: 5.6.3 then has no
        // non-relative name and keeps the relative one, not the path from
        // `baseUrl`.
        (
            "an empty key's empty name",
            "5.6.3",
            format!(
                r#"{{"compilerOptions":{{{options},"baseUrl":"..","paths":{{"":["shared/lib.ts"]}}}},{include}}}"#
            ),
            &[("../shared/lib.ts", "../../../shared/lib")][..],
        ),
        (
            "a star's empty name",
            "5.6.3",
            format!(
                r#"{{"compilerOptions":{{{options},"baseUrl":"..","paths":{{"*":["shared/lib*"]}}}},{include}}}"#
            ),
            &[("../shared/lib.ts", "../../../shared/lib")][..],
        ),
        (
            "empty paths",
            "5.6.3",
            format!(r#"{{"compilerOptions":{{{options},"baseUrl":"..","paths":{{}}}},{include}}}"#),
            &[("../shared/lib.ts", "shared/lib")][..],
        ),
        (
            "a name climbing out of baseUrl",
            "5.6.3",
            format!(r#"{{"compilerOptions":{{{options},"baseUrl":"./src"}},{include}}}"#),
            &[
                ("../shared/lib.ts", "../../../shared/lib"),
                ("scoped/thing.ts", "../../scoped/thing"),
            ][..],
        ),
        (
            "baseUrl inherited",
            "5.6.3",
            format!(
                r#"{{"extends":"./base/tsconfig.base.json","compilerOptions":{{{options}}},{include}}}"#
            ),
            &[("../shared/lib.ts", "shared/lib")][..],
        ),
        (
            "TypeScript 7",
            "7.0.2",
            format!(r#"{{"compilerOptions":{{{options}}},{include}}}"#),
            &[
                ("../shared/lib.ts", "../../../shared/lib"),
                ("scoped/thing.ts", "../../scoped/thing"),
            ][..],
        ),
    ] {
        let manifest = manifest(version);
        let project = Project::new(&[
            ("app/tsconfig.json", &config),
            (MANIFEST, &manifest),
            ("shared/lib.ts", "export const lib = 1;\n"),
            ("shared/sub/index.ts", "export const sub = 1;\n"),
            ("shared/sub.ts", "export const subFile = 1;\n"),
            ("app/src/x.ts", "export const x = 1;\n"),
            ("app/src/deep/use.ts", "export const use = 1;\n"),
            ("app/scoped/package.json", r#"{"name":"scoped"}"#),
            ("app/scoped/thing.ts", "export const thing = 1;\n"),
            (
                "app/base/tsconfig.base.json",
                r#"{"compilerOptions":{"baseUrl":"../.."}}"#,
            ),
        ]);
        let program = project.load("app/tsconfig.json");
        for (target, spelling) in expected {
            assert_eq!(
                spelled(&project, &program, "src/deep/use.ts", target),
                [*spelling],
                "{name}: {target}"
            );
        }
    }
    // The fresh spelling a type query demands in the integration test of
    // the same layout, from a CommonJS importer under `nodenext`.
    let project = Project::new(&[
        (
            "app/tsconfig.json",
            r#"{"compilerOptions":{"noLib":true,"types":[],"strict":true,"target":"es2022","module":"nodenext","moduleResolution":"nodenext","esModuleInterop":true,"baseUrl":".."},"files":["use.cts","../shared/target.d.ts"]}"#,
        ),
        (MANIFEST, &manifest("5.6.3")),
        (
            "app/use.cts",
            "import type { present } from \"shared/target.js\" with { \"resolution-mode\": \"import\" };\n",
        ),
        (
            "shared/target.d.ts",
            "export declare const present: number;\n",
        ),
    ]);
    let program = project.load("app/tsconfig.json");
    assert_eq!(
        spelled(&project, &program, "use.cts", "../shared/target.d.ts"),
        ["shared/target"]
    );
    // In the `import` mode of that file's only import, the name 5.6.3's
    // `tsc` shows for the module (TS2305, `Module '"shared/target.js"' has
    // no exported member`, for import-mode imports of missing members from
    // `shared/target.js` and `../shared/target.js` beside this one, with
    // `"lib": ["es2022"]` in place of `noLib`).
    let importer = program
        .sources
        .get(file(&program, "use.cts").0)
        .path
        .clone();
    let target = program
        .sources
        .get(file(&program, "../shared/target.d.ts").0)
        .path
        .clone();
    let spelled = super::specifiers::module_specifiers(
        &program.specifier_environment,
        super::specifiers::ImportingFile {
            file_name: importer.to_str().unwrap(),
            default_mode: Some(crate::project::binding::ResolutionMode::Require),
            imports: &[],
        },
        target.to_str().unwrap(),
        Some(crate::project::binding::ResolutionMode::Import),
        Some(crate::project::binding::ResolutionMode::Import),
    )
    .unwrap();
    assert_eq!(spelled, ["shared/target.js"]);
}

/// Inputs are captured on worker threads ahead of the loader, yet every
/// number of workers, none included, loads the same program, in the same
/// order and with the same facts, and the project answers the same. The
/// inputs cover JSON, TSX with and without JSX, `.mts`/`.cts`, files whose
/// package scope forces module semantics on them (collected again after
/// capture), scripts, libraries, a package with a reference directive and
/// an augmentation, and many roots, which workers capture ahead of the
/// loader, one of which begins a long import chain.
#[test]
fn every_number_of_capture_workers_loads_the_same_project() {
    let mut files: Vec<(String, String)> = vec![
        (
            "tsconfig.json".into(),
            r#"{ "compilerOptions": { "module": "nodenext", "moduleDetection": "auto", "jsx": "react-jsx", "resolveJsonModule": true, "lib": ["es5"], "types": [] }, "include": ["src", "esm", "scripts"] }"#.into(),
        ),
        (
            "src/index.ts".into(),
            "import { util } from './util.js';\nimport data = require('./data.json');\nimport { fromPkg } from 'pkg';\nimport { chain0 } from './fill/f0.js';\nexport const total = util + data.value + fromPkg + chain0;\n".into(),
        ),
        ("src/util.ts".into(), "export const util = 1;\n".into()),
        ("src/data.json".into(), r#"{ "value": 2 }"#.into()),
        ("src/view.tsx".into(), "const view = <div />;\n".into()),
        ("src/plain.tsx".into(), "var plainTsx = 1;\n".into()),
        ("src/m.mts".into(), "var inMts = 1;\n".into()),
        ("src/c.cts".into(), "var inCts = 1;\n".into()),
        (
            "src/augment.ts".into(),
            "import 'pkg';\ndeclare module 'pkg' { export const augmented: number; }\n".into(),
        ),
        ("esm/package.json".into(), r#"{ "type": "module" }"#.into()),
        ("esm/a.ts".into(), "var forced = 1;\nvar forcedA = forced;\n".into()),
        ("esm/b.ts".into(), "var forced = 2;\nvar forcedB = forced;\n".into()),
        ("esm/c.ts".into(), "export const exported = 1;\n".into()),
        ("scripts/g1.ts".into(), "var globalOne = 1;\n".into()),
        ("scripts/g2.ts".into(), "var useGlobal = globalOne;\n".into()),
        (
            "node_modules/pkg/package.json".into(),
            r#"{ "name": "pkg", "types": "index.d.ts" }"#.into(),
        ),
        (
            "node_modules/pkg/index.d.ts".into(),
            "/// <reference path=\"./extra.d.ts\" />\nexport declare const fromPkg: number;\n".into(),
        ),
        (
            "node_modules/pkg/extra.d.ts".into(),
            "declare const fromExtra: number;\n".into(),
        ),
        (
            "node_modules/react/package.json".into(),
            r#"{ "name": "react", "exports": { ".": { "types": "./index.d.ts" }, "./jsx-runtime": { "types": "./jsx-runtime.d.ts" } } }"#.into(),
        ),
        ("node_modules/react/index.d.ts".into(), "export {};\n".into()),
        (
            "node_modules/react/jsx-runtime.d.ts".into(),
            "export declare function jsx(...args: any[]): any;\ndeclare global { namespace JSX { interface IntrinsicElements { [name: string]: any } } }\n".into(),
        ),
    ];
    for index in 0..80 {
        let next = if index + 1 < 80 {
            format!(
                "import {{ chain{} }} from './f{}.js';\n",
                index + 1,
                index + 1
            )
        } else {
            String::new()
        };
        files.push((
            format!("src/fill/f{index}.ts"),
            format!("{next}export const chain{index} = {index};\nfunction local{index}() {{ if (chain{index}) {{ return 1; }} return 0; }}\nlocal{index}();\n"),
        ));
    }
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let project = Project::new(&borrowed);
    let names = [
        "util",
        "total",
        "fromPkg",
        "fromExtra",
        "augmented",
        "view",
        "plainTsx",
        "inMts",
        "inCts",
        "forced",
        "forcedA",
        "forcedB",
        "exported",
        "globalOne",
        "chain0",
        "chain79",
        "local40",
        "jsx",
    ];
    let summary = |workers: usize| -> String {
        let _workers = capture::tests::use_workers(Some(workers));
        let program = project.load("tsconfig.json");
        let mut summary = String::new();
        for id in program.sources.ids() {
            let source = program.sources.get(id);
            let file = &program.files[id.index()];
            summary.push_str(&format!(
                "{} {:?} search={} paths={:?} semantics={:?} module={} declaration={} jsx={:?} directives={:?} requests={:?} modules={:?} diagnostics={:?} navigation={}\n",
                source.display,
                source.path,
                file.search,
                file.paths,
                file.module_semantics,
                file.facts.module,
                file.facts.declaration,
                file.facts.jsx,
                file.facts.comment_directives(),
                file.facts.requests,
                file.modules,
                file.diagnostics,
                file.navigation.is_some(),
            ));
        }
        let loaded = crate::project::Project::load(&project.root.join("tsconfig.json"))
            .unwrap_or_else(|error| panic!("{workers} workers: {error:?}"));
        for name in names {
            let references = loaded.find_references(crate::references::ReferenceQuery::Name {
                name: name.into(),
                file: None,
            });
            summary.push_str(&format!("{name}: {references:?}\n"));
        }
        for (path, _) in &files {
            if path.starts_with("src/") || path.starts_with("esm/") || path.starts_with("scripts/")
            {
                if path.ends_with(".json") {
                    continue;
                }
                let outline = loaded.outline(&project.root.join(path));
                summary.push_str(&format!("{path}: {outline:?}\n"));
            }
        }
        summary
    };
    let sequential = summary(0);
    // Under `moduleDetection: auto`, the package scope's `type: module`
    // makes each `esm` file a module, which its capture did not assume, so
    // their `forced` variables are separate; the scripts share a global.
    let loaded = crate::project::Project::load(&project.root.join("tsconfig.json")).unwrap();
    let groups = |name: &str| {
        loaded
            .find_references(crate::references::ReferenceQuery::Name {
                name: name.into(),
                file: None,
            })
            .unwrap()
            .groups
    };
    assert_eq!(groups("forced").len(), 2, "{:?}", groups("forced"));
    assert_eq!(groups("globalOne").len(), 1);
    assert_eq!(groups("globalOne")[0].occurrences.len(), 2);
    assert_eq!(groups("chain79").len(), 1);
    for workers in [1, 2, 3, 8] {
        let parallel = summary(workers);
        if parallel != sequential {
            let first = sequential
                .lines()
                .zip(parallel.lines())
                .find(|(left, right)| left != right);
            panic!("{workers} workers load differently: {first:?}");
        }
    }
}

/// Under bundler resolution, a package's `type: module` makes a TypeScript
/// script inside `node_modules` a module, so its top-level variable is not
/// global; a package without `type` leaves its script global. TypeScript
/// 7.0.2 reports TS2304 for `scriptGlobal` and nothing for `cjsGlobal`. The
/// package format decides this after the input was captured, which collects
/// it again.
#[test]
fn a_bundler_package_type_forces_module_semantics_on_its_scripts() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "module": "esnext", "moduleResolution": "bundler", "noLib": true, "types": [] }, "files": ["app.ts"] }"#,
        ),
        (
            "app.ts",
            "import 'pkg/script';\nimport 'cjspkg/script';\nscriptGlobal;\ncjsGlobal;\n",
        ),
        (
            "node_modules/pkg/package.json",
            r#"{ "name": "pkg", "type": "module", "exports": { "./script": "./script.ts" } }"#,
        ),
        ("node_modules/pkg/script.ts", "var scriptGlobal = 1;\n"),
        (
            "node_modules/cjspkg/package.json",
            r#"{ "name": "cjspkg", "exports": { "./script": "./script.ts" } }"#,
        ),
        ("node_modules/cjspkg/script.ts", "var cjsGlobal = 1;\n"),
    ]);
    for workers in [None, Some(0)] {
        let chosen = capture::tests::use_workers(workers);
        let loaded = crate::project::Project::load(&project.root.join("tsconfig.json")).unwrap();
        drop(chosen);
        let at = |line| {
            loaded.find_references(crate::references::ReferenceQuery::At {
                file: project.root.join("app.ts"),
                position: SourcePosition { line, column: 1 },
            })
        };
        assert!(
            matches!(
                at(3),
                Err(crate::analysis::AnalysisError::Query(ref error))
                    if error.kind == crate::analysis::QueryErrorKind::UnresolvedTarget
            ),
            "{:?}",
            at(3)
        );
        let global = at(4).unwrap();
        assert_eq!(global.groups.len(), 1);
        assert_eq!(global.groups[0].occurrences.len(), 1, "{global:?}");
        assert_eq!(
            global.groups[0].declarations[0].location.file,
            "node_modules/cjspkg/script.ts"
        );
    }
}

/// JavaScript that package lookups find, by `maxNodeModuleJsDepth`: the
/// program TypeScript 7.0.2 lists (`tsc --listFiles --incremental false`).
/// `jsdeep` is first met through the typed package two lookups deep, which
/// elides it at depth 1, then loaded through the local chain one lookup
/// deep; 7.0.2's `collectFiles` lists it where the elided route met it,
/// before the typed package. (5.6.3 lists it after the typed package, where
/// it processed the file, a node_modules order difference docs/references.md
/// records; the inputs are the same.)
#[test]
fn node_modules_javascript_follows_max_node_module_js_depth() {
    let config = |depth: &str| {
        format!(
            r#"{{ "compilerOptions": {{ "allowJs": true, {depth} "module": "esnext", "moduleResolution": "bundler", "noLib": true, "types": [], "noEmit": true }}, "include": ["src"] }}"#
        )
    };
    let unset = config("");
    let zero = config(r#""maxNodeModuleJsDepth": 0,"#);
    let one = config(r#""maxNodeModuleJsDepth": 1,"#);
    let fraction = config(r#""maxNodeModuleJsDepth": 1.5,"#);
    let negative = config(r#""maxNodeModuleJsDepth": -1,"#);
    let project = Project::new(&[
        ("unset.json", &unset),
        ("zero.json", &zero),
        ("one.json", &one),
        ("fraction.json", &fraction),
        ("negative.json", &negative),
        (
            "node_modules/jspkg/package.json",
            r#"{ "name": "jspkg", "version": "1.0.0", "main": "index.js" }"#,
        ),
        (
            "node_modules/jspkg/index.js",
            "import { d } from \"jsdep\";\nexport const v = 1;\nexport const fromDep = d;\n",
        ),
        (
            "node_modules/jsdep/package.json",
            r#"{ "name": "jsdep", "version": "1.0.0", "main": "index.js" }"#,
        ),
        ("node_modules/jsdep/index.js", "export const d = \"dep\";\n"),
        (
            "node_modules/typed/package.json",
            r#"{ "name": "typed", "version": "1.0.0", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/typed/index.d.ts",
            "export { w } from \"jsdeep\";\n",
        ),
        (
            "node_modules/jsdeep/package.json",
            r#"{ "name": "jsdeep", "version": "1.0.0", "main": "index.js" }"#,
        ),
        ("node_modules/jsdeep/index.js", "export const w = 42;\n"),
        (
            "src/app.ts",
            "import { v, fromDep } from \"jspkg\";\nexport const x = v;\nexport const y = fromDep;\n",
        ),
        (
            "src/deep.ts",
            "import { w } from \"typed\";\nimport \"./local1\";\nexport const z = w;\n",
        ),
        ("src/local1.ts", "import \"./local2\";\nexport {};\n"),
        (
            "src/local2.ts",
            "import { w } from \"jsdeep\";\nexport const again = w;\n",
        ),
    ]);
    let none = [
        "src/app.ts",
        "node_modules/typed/index.d.ts",
        "src/local2.ts",
        "src/local1.ts",
        "src/deep.ts",
    ];
    for config in ["unset.json", "zero.json", "negative.json"] {
        let program = project.load(config);
        assert_eq!(listed(&program), none, "{config}");
        assert_eq!(resolved(&program, "src/app.ts", "jspkg"), "untyped");
        assert_eq!(
            resolved(&program, "node_modules/typed/index.d.ts", "jsdeep"),
            "untyped"
        );
    }
    for config in ["one.json", "fraction.json"] {
        let program = project.load(config);
        assert_eq!(
            listed(&program),
            [
                "node_modules/jspkg/index.js",
                "src/app.ts",
                "node_modules/jsdeep/index.js",
                "node_modules/typed/index.d.ts",
                "src/local2.ts",
                "src/local1.ts",
                "src/deep.ts",
            ],
            "{config}"
        );
        assert_eq!(
            resolved(&program, "node_modules/jspkg/index.js", "jsdep"),
            "untyped"
        );
        // The elided route names the file a later route loaded.
        assert_eq!(
            resolved(&program, "node_modules/typed/index.d.ts", "jsdeep"),
            "node_modules/jsdeep/index.js"
        );
        assert_eq!(
            searched(&program),
            [
                "src/app.ts",
                "src/deep.ts",
                "src/local1.ts",
                "src/local2.ts"
            ]
        );
    }
}

/// A JavaScript file imports the helpers under `importHelpers`, a script
/// too (`resolveImportsAndModuleAugmentations`' `isJavaScriptFile`), while
/// only a JSX or TSX file, by its script kind, imports the automatic JSX
/// runtime: a `.js` file that parses JSX does not (native 7.0.2 `tsc
/// --listFilesOnly`; 5.6.3 imports the runtime into every JavaScript file
/// and TypeScript module, a difference docs/references.md records).
#[test]
fn javascript_files_import_helpers_and_only_jsx_files_the_jsx_runtime() {
    let config = |include: &str| {
        format!(
            r#"{{ "compilerOptions": {{ "allowJs": true, "jsx": "react-jsx", "importHelpers": true, "noLib": true, "types": [], "module": "esnext", "moduleResolution": "bundler" }}, "files": ["{include}"] }}"#
        )
    };
    let (js, jsx, script) = (config("a.js"), config("b.jsx"), config("c.js"));
    let project = Project::new(&[
        ("js.json", &js),
        ("jsx.json", &jsx),
        ("script.json", &script),
        ("a.js", "export const e = <div/>;\n"),
        ("b.jsx", "export const e = <div/>;\n"),
        ("c.js", "var s = 1;\n"),
        (
            "node_modules/react/package.json",
            r#"{ "name": "react", "version": "18.0.0", "types": "index.d.ts" }"#,
        ),
        ("node_modules/react/index.d.ts", "export {};\n"),
        (
            "node_modules/react/jsx-runtime.d.ts",
            "export declare function jsx(...a: any[]): any;\n",
        ),
        (
            "node_modules/tslib/package.json",
            r#"{ "name": "tslib", "version": "2.0.0", "types": "tslib.d.ts" }"#,
        ),
        (
            "node_modules/tslib/tslib.d.ts",
            "export declare function __assign(...a: any[]): any;\n",
        ),
    ]);
    assert_eq!(
        listed(&project.load("js.json")),
        ["node_modules/tslib/tslib.d.ts", "a.js"]
    );
    assert_eq!(
        listed(&project.load("jsx.json")),
        [
            "node_modules/tslib/tslib.d.ts",
            "node_modules/react/jsx-runtime.d.ts",
            "b.jsx"
        ]
    );
    assert_eq!(
        listed(&project.load("script.json")),
        ["node_modules/tslib/tslib.d.ts", "c.js"]
    );
}
