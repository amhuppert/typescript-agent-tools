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
//!   `resolvePackageJsonExports`, `resolvePackageJsonImports`, `noLib`,
//!   `outDir`, `declarationDir`, `rootDir`, `composite`, `skipLibCheck`, and
//!   `noUncheckedSideEffectImports`.
//! - Rejected as unsupported: `baseUrl`, `moduleResolution` `node`/`node10`/
//!   `classic`, `module` `amd`/`umd`/`system`, a non-empty `lib`,
//!   `noResolve`, `noDtsResolution`, non-empty `references`, JavaScript or
//!   JSON inputs, and `/// <reference lib>` without `noLib`.
//! - Every other option is ignored: none affects the program graph, binding,
//!   or which failures loading tolerates. A new option is read in
//!   `config::convert`, listed here, and documented in docs/references.md.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

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

    fn try_load(&self, config: &str) -> Result<Program, ProjectError> {
        load(&self.root.join(config), &super::disk::Disk::default())
    }

    fn load(&self, config: &str) -> Program {
        self.try_load(config)
            .unwrap_or_else(|error| panic!("{config} failed to load: {error}"))
    }

    fn error(&self, config: &str) -> ProjectError {
        match self.try_load(config) {
            Ok(_) => panic!("{config} loaded"),
            Err(error) => error,
        }
    }

    fn display(&self, path: &Path) -> String {
        let root = std::fs::canonicalize(&self.root).unwrap();
        path.strip_prefix(&root)
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

/// Input display paths in `FileId` order.
fn inputs(program: &Program) -> Vec<&str> {
    program
        .sources
        .ids()
        .map(|id| program.sources.get(id).display.as_str())
        .collect()
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
    assert_eq!(inputs(&program), ["../src/a.ts", "../lib/l.ts"]);
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
    assert_eq!(inputs(&program), ["main.ts", "two.ts"]);
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
    assert_eq!(cycle.kind, ProjectErrorKind::Config);
    assert!(cycle.message.contains("Circularity"), "{}", cycle.message);
    assert_eq!(project.display(cycle.path.as_ref().unwrap()), "b.json");
    assert_eq!(cycle.position, position(2, 14));

    let missing = project.error("missing.json");
    assert_eq!(
        (missing.kind, missing.position),
        (ProjectErrorKind::Config, position(1, 14))
    );
    // A read failure is located at the `extends` value that named the file.
    let unreadable = project.error("unreadable.json");
    assert_eq!(
        (unreadable.kind, unreadable.position),
        (ProjectErrorKind::Io, position(1, 14))
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
    assert_eq!(project.error("package.json").kind, ProjectErrorKind::Config);
    let invalid = project.error("invalid.json");
    assert_eq!(
        (invalid.kind, invalid.position),
        (ProjectErrorKind::Config, position(3, 3))
    );
    assert_eq!(project.error("array.json").kind, ProjectErrorKind::Config);
    assert_eq!(project.error("absent.json").kind, ProjectErrorKind::Io);
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
    assert_eq!(inputs(&program), ["src/main.ts", "p.ts"]);
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
        (ProjectErrorKind::Unsupported, position(1, 68))
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
            ProjectErrorKind::Unsupported,
        ),
        (
            r#""moduleResolution": "node""#,
            ProjectErrorKind::Unsupported,
        ),
        (
            r#""moduleResolution": "Classic""#,
            ProjectErrorKind::Unsupported,
        ),
        (r#""baseUrl": ".""#, ProjectErrorKind::Unsupported),
        (r#""module": "amd""#, ProjectErrorKind::Unsupported),
        (
            r#""noLib": false, "lib": ["es2020"]"#,
            ProjectErrorKind::Unsupported,
        ),
        (r#""noResolve": true"#, ProjectErrorKind::Unsupported),
        (r#""moduleResolution": "bogus""#, ProjectErrorKind::Config),
        (r#""module": "none""#, ProjectErrorKind::Config),
        (r#""moduleResolution": "node16""#, ProjectErrorKind::Config),
        (
            r#""module": "nodenext", "moduleResolution": "bundler""#,
            ProjectErrorKind::Config,
        ),
        (r#""lib": []"#, ProjectErrorKind::Config),
        (r#""paths": { "a": ["src/a"] }"#, ProjectErrorKind::Config),
        (r#""paths": { "a/*/*": ["./a"] }"#, ProjectErrorKind::Config),
        (r#""types": "node""#, ProjectErrorKind::Config),
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
               "noLib": false, "strict": true, "target": "es5", "outFile": "x.js", "sourceMap": true, "declaration": true,
               "plugins": [{ "name": "x" }], "unknownOption": 1, "allowImportingTsExtensions": true, "esModuleInterop": false },
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
    assert_eq!(
        project.error("trailing.json").kind,
        ProjectErrorKind::Config
    );
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
        ("b.ts", ""),
        ("noext.ts", ""),
        ("sub/z.ts", ""),
        ("node_modules/nm/x.ts", ""),
        ("h.js", ""),
        ("notes.txt", ""),
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
        (ProjectErrorKind::Io, position(2, 21))
    );
    assert_eq!(project.error("js.json").kind, ProjectErrorKind::Unsupported);
    assert_eq!(project.error("text.json").kind, ProjectErrorKind::Config);
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
    assert_eq!(none.kind, ProjectErrorKind::Config);
    assert!(
        none.message.starts_with("No inputs were found"),
        "{}",
        none.message
    );
    assert_eq!(
        project.error("emptyfiles.json").kind,
        ProjectErrorKind::Config
    );
    assert_eq!(
        project.error("emptyrefs.json").kind,
        ProjectErrorKind::Config
    );
    assert!(inputs(&project.load("extended.json")).is_empty());
}

#[test]
fn javascript_and_json_roots_are_unsupported_but_shadowed_javascript_is_not_a_root() {
    let project = Project::new(&[
        (
            "allowjs.json",
            r#"{ "compilerOptions": { "noLib": true, "allowJs": true }, "include": ["src"] }"#,
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
        ("src/a.js", ""),
        ("lib/b.ts", ""),
        ("lib/b.js", ""),
        ("data/d.json", "{}"),
    ]);
    let error = project.error("allowjs.json");
    assert_eq!(error.kind, ProjectErrorKind::Unsupported);
    assert_eq!(project.display(error.path.as_ref().unwrap()), "src/a.js");
    assert_eq!(inputs(&project.load("shadowed.json")), ["lib/b.ts"]);
    assert_eq!(
        project.error("json.json").kind,
        ProjectErrorKind::Unsupported
    );
    assert_eq!(inputs(&project.load("plain.json")), ["lib/b.ts"]);
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

#[test]
fn relative_specifiers_follow_extension_and_directory_rules() {
    let project = Project::new(&[
        ("package.json", r#"{ "type": "module" }"#),
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext" }, "files": ["src/a.ts", "src/c.cts"] }"#,
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
            ("./onlyjs.js", "unsupported"),
            ("./data.json", "unsupported"),
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
        ("jsonly", "unsupported"),
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
        (ProjectErrorKind::UnresolvedImport, position(1, 86))
    );
    assert_eq!(
        project.error("proj/missing.json").kind,
        ProjectErrorKind::UnresolvedImport
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
            r#"{ "compilerOptions": { }, "files": ["lib.ts"] }"#,
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
        ("lib.ts", "/// <reference lib=\"es2015\" />\n"),
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
    let lib = project.error("libs.json");
    assert_eq!(
        (lib.kind, lib.position),
        (ProjectErrorKind::Unsupported, position(1, 21))
    );
    assert_eq!(inputs(&project.load("nolib.json")), ["lib.ts"]);
    let missing = project.error("missing.json");
    assert_eq!(
        (missing.kind, missing.position),
        (ProjectErrorKind::UnresolvedImport, position(2, 22))
    );
    assert_eq!(
        project.display(missing.path.as_ref().unwrap()),
        "missing.ts"
    );
    assert_eq!(
        project.error("missingtype.json").kind,
        ProjectErrorKind::UnresolvedImport
    );
    assert_eq!(project.error("js.json").kind, ProjectErrorKind::Unsupported);
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
        (ProjectErrorKind::Syntax, position(2, 7))
    );
    assert_eq!(project.display(error.path.as_ref().unwrap()), "proj/bad.ts");
}

#[test]
fn skipped_package_reference_directives_may_name_absent_inputs() {
    let project = Project::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true }, "files": ["app.ts"] }"#,
        ),
        (
            "checked.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["app.ts"] }"#,
        ),
        ("app.ts", "import { tool } from \"tool\";\n"),
        (
            "node_modules/tool/package.json",
            r#"{ "name": "tool", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/tool/index.d.ts",
            "/// <reference types=\"absent\" />\n/// <reference path=\"./absent.d.ts\" />\nexport declare const tool: number;\n",
        ),
    ]);
    let program = project.load("tsconfig.json");
    assert_eq!(inputs(&program), ["app.ts", "node_modules/tool/index.d.ts"]);
    let error = project.error("checked.json");
    assert_eq!(error.kind, ProjectErrorKind::UnresolvedImport);
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
    // Loading runs on a stack of this size.
    let error = std::thread::Builder::new()
        .stack_size(LOAD_STACK_BYTES)
        .spawn(move || load(&config, &super::disk::Disk::default()).unwrap_err())
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(error.kind, ProjectErrorKind::Unsupported, "{error}");
    assert_eq!(error.position, Some(SourcePosition { line: 1, column: 8 }));
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
    let config = project.root.join("tsconfig.json");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let program = load(&config, &super::disk::Disk::default()).unwrap();
        let inputs: Vec<String> = inputs(&program).into_iter().map(str::to_owned).collect();
        send.send(inputs).unwrap();
    });
    let inputs = receive
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("loading should finish");
    assert_eq!(inputs, ["app.ts"]);
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
        let program = load(&config, &super::disk::Disk::default()).unwrap();
        let results = ["#x", "#p", "#d0", "#ok"].map(|name| resolved(&program, "app.ts", name));
        send.send(results).unwrap();
    });
    let results = receive
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("resolution should finish");
    assert_eq!(results, ["missing", "missing", "missing", "lib.ts"]);
}
