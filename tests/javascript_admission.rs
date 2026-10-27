//! JavaScript program inputs, at the final outcome: whether a JavaScript
//! file is an input, which the type of a value it exports shows, or whether
//! an import of it stays an untyped module, as TypeScript decides with
//! `allowJs`, `GetResolutionDiagnostic`, `shouldAddFile`, and
//! `maxNodeModuleJsDepth`. Expected answers are native 7.0.2's and 5.6.3's
//! (`tsc --listFiles --incremental false`, `tools/type-oracle.mjs`, and
//! `tools/ts56-oracle.mjs`) on scratch copies of these fixtures.

mod support;

use std::path::PathBuf;
use support::DIALECTS;
use support::Dialect::{self, TypeScript7, TypeScript56};
use support::Fixture;
use typescript_parser::analysis::{AnalysisError, InputError, InputErrorKind};
use typescript_parser::project::Project;
use typescript_parser::source::SourcePosition;

/// An untyped module's value: TypeScript's error type, which this tool
/// reports as a semantic failure rather than spell `any`.
fn assert_untyped(result: Result<String, AnalysisError>, context: &str) {
    match result {
        Err(AnalysisError::Semantic(_)) => {}
        other => panic!("{context}: expected the untyped module's error type, got {other:?}"),
    }
}

fn position(error: &InputError) -> (PathBuf, usize, usize) {
    let position = error.position.expect("a located failure");
    (
        error.path.clone().expect("a failure in a file"),
        position.line,
        position.column,
    )
}

/// Two ES modules of constants, as a JavaScript program commonly keeps
/// design tokens: numeric literals, and an object of nested objects with
/// numeric and string keys, reads of module-local objects, an array, and a
/// template string.
const LAYERS: &str = "// Stacking order for overlays.
export const zBase = 10;
export const zPopover = 24000;
export const zDialogBackdrop = 1000;
";

const PALETTE: &str = r##"/**
 * Shared colour tokens.
 */
const shades = {
  slate100: "#f1f5f9",
  slate300: "#cbd5e1",
};

const accentShade = "#ff8800";

export const PALETTE = {
  white: "#ffffff",
  black: "#000000",
  slate: {
    100: shades.slate100,
    300: shades.slate300,
    "400": "#94a3b8",
    500: "#64748b",
  },
  brand: {
    main: accentShade,
    light: "#ffd1a3",
  },
  chart: {
    legend: {
      swatches: ["#111111", "#222222", "#333333"],
      label: "legend",
    },
    marker: { color: "#123456", leftOpacity: 0.4, rightOpacity: 0.7 },
  },
};

export const GRADIENT_CSS = `linear-gradient(90deg, ${PALETTE.brand.main} 0%, ${PALETTE.brand.light} 100%)`;
"##;

const APP: &str = r#"import { zPopover } from "@/constants/layers";
import { PALETTE, GRADIENT_CSS } from "./constants/palette";
import { PALETTE as THEME } from "@/constants/palette";

export const layer = zPopover;
export const slate300 = PALETTE.slate[300];
export const slate400 = PALETTE.slate["400"];
export const brandMain = PALETTE.brand.main;
export const firstSwatches = PALETTE.chart.legend.swatches.slice(0, 2);
export const gradient = GRADIENT_CSS;
export const themed = THEME.brand.light;
export const opacity = PALETTE.chart.marker.leftOpacity;
"#;

const OTHER: &str = r#"import { PALETTE } from "./constants/palette";
import { zPopover } from "./constants/layers";

export function paint(): string {
  return PALETTE.brand.main + PALETTE.slate[300] + zPopover;
}
"#;

/// The full shape of `PALETTE`, as both compilers print it.
const PALETTE_TYPE: &str = r#"{ white: string; black: string; slate: { 100: string; 300: string; "400": string; 500: string; }; brand: { main: string; light: string; }; chart: { legend: { swatches: string[]; label: string; }; marker: { color: string; leftOpacity: number; rightOpacity: number; }; }; }"#;

fn constants_config(lib: &str) -> String {
    format!(
        r#"{{ "compilerOptions": {{ "allowJs": true, "strict": true, "target": "es2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true, "noEmit": true, "lib": [{lib}], "types": [], "paths": {{ "@/*": ["./src/*"] }} }}, "include": ["src"] }}"#
    )
}

fn constants_files(config: &str) -> Vec<(&str, &str)> {
    vec![
        ("tsconfig.json", config),
        ("src/constants/layers.js", LAYERS),
        ("src/constants/palette.js", PALETTE),
        ("src/app.ts", APP),
        ("src/other.ts", OTHER),
    ]
}

/// What both compilers answer for the constants project.
fn check_constants(project: &Project, fixture: &Fixture) {
    for (line, expected) in [
        (5, "24000"),
        (6, "string"),
        (7, "string"),
        (8, "string"),
        (9, "string[]"),
        (10, "string"),
        (11, "string"),
        (12, "number"),
    ] {
        assert_eq!(
            fixture.summary_at(project, "src/app.ts", line, 14),
            expected,
            "src/app.ts:{line}"
        );
    }
    assert_eq!(
        fixture.summary_at(project, "src/app.ts", 3, 21),
        PALETTE_TYPE
    );
    assert_eq!(
        fixture.summary_at(project, "src/constants/layers.js", 3, 14),
        "24000"
    );
    assert_eq!(
        fixture.summary_at(project, "src/constants/palette.js", 11, 14),
        PALETTE_TYPE
    );
    assert_eq!(
        fixture.summary_at(project, "src/constants/palette.js", 33, 14),
        "string"
    );
}

#[test]
fn constants_modules_are_inputs_whose_types_match_the_compilers() {
    let config = constants_config(r#""es2017""#);
    let fixture = Fixture::new(&constants_files(&config));
    let project = fixture.load();
    check_constants(&project, &fixture);
}

#[test]
fn constants_modules_match_typescript_5_6_with_its_own_libraries() {
    let Some(package) =
        support::typescript_5_6("constants_modules_match_typescript_5_6_with_its_own_libraries")
    else {
        return;
    };
    let config = constants_config(r#""es2017""#);
    let fixture = Fixture::new(&constants_files(&config));
    fixture.install_compiler(&package);
    let project = fixture.load();
    check_constants(&project, &fixture);
}

/// A package of JavaScript importing another, a typed package re-exporting
/// a third JavaScript package, and a chain of local files that reaches the
/// third directly, later than the typed package does.
fn depth_files(depth: &str, no_implicit_any: bool) -> Vec<(String, String)> {
    let config = format!(
        r#"{{ "compilerOptions": {{ "allowJs": true, "strict": true, "noImplicitAny": {no_implicit_any}, {depth} "target": "es2017", "esModuleInterop": true, "module": "esnext", "moduleResolution": "bundler", "noLib": true, "types": [], "noEmit": true }}, "include": ["src"] }}"#
    );
    [
        ("tsconfig.json", config.as_str()),
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
    ]
    .into_iter()
    .map(|(path, text)| (path.to_owned(), text.to_owned()))
    .collect()
}

fn depth_fixture(dialect: Dialect, depth: &str, no_implicit_any: bool) -> Fixture {
    let files = depth_files(depth, no_implicit_any);
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    Fixture::with_dialect(dialect, &files)
}

#[test]
fn node_modules_javascript_is_an_input_only_within_max_node_module_js_depth() {
    for dialect in DIALECTS {
        // Unset and 0 elide every JavaScript file a package lookup finds:
        // under noImplicitAny the import is TS7016 in the first importer.
        for depth in ["", r#""maxNodeModuleJsDepth": 0,"#] {
            let fixture = depth_fixture(dialect, depth, true);
            let error = fixture.try_load().unwrap_err();
            assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
            assert_eq!(
                error.message,
                "Could not find a declaration file for module 'jspkg'. 'node_modules/jspkg/index.js' implicitly has an 'any' type."
            );
            assert_eq!(position(&error), (fixture.path("src/app.ts"), 1, 28));
            // Without it the module is untyped: its values have the error
            // type, and the typed package's re-export is untyped too.
            let fixture = depth_fixture(dialect, depth, false);
            let project = fixture.load();
            assert_untyped(
                fixture.type_at(&project, "src/app.ts", 2, 14),
                "x at depth 0",
            );
            assert_untyped(
                fixture.type_at(&project, "src/deep.ts", 3, 14),
                "z at depth 0",
            );
        }
        // At 1, a package's own JavaScript is an input, but what it imports
        // from another package is not; `jsdeep`, first reached through the
        // typed package at depth 2, is an input once the local chain reaches
        // it at depth 1, and the re-export then names it.
        let fixture = depth_fixture(dialect, r#""maxNodeModuleJsDepth": 1,"#, true);
        let project = fixture.load();
        assert_eq!(fixture.summary_at(&project, "src/app.ts", 2, 14), "1");
        assert_untyped(
            fixture.type_at(&project, "src/app.ts", 3, 14),
            "y at depth 1",
        );
        assert_eq!(fixture.summary_at(&project, "src/deep.ts", 3, 14), "42");
        assert_eq!(fixture.summary_at(&project, "src/local2.ts", 2, 14), "42");
    }
}

/// A package two package lookups from the root, `pkgb`, reached first
/// through `pkga` at depth 2, where its JavaScript `jsp` is beyond
/// `maxNodeModuleJsDepth: 2`, and then through a chain of local files at
/// depth 1. With `chained`, `pkgb` reaches `jsp` through a third typed
/// package, `pkgc`, and the limit is 3.
fn revisit_fixture(dialect: Dialect, chained: bool) -> Fixture {
    let depth = if chained { "3" } else { "2" };
    let config = format!(
        r#"{{ "compilerOptions": {{ "allowJs": true, "maxNodeModuleJsDepth": {depth}, "strict": true, "target": "es2017", "esModuleInterop": true, "module": "esnext", "moduleResolution": "bundler", "noLib": true, "types": [], "noEmit": true }}, "files": ["src/app.ts"] }}"#
    );
    let mut files = vec![
        ("tsconfig.json", config.as_str()),
        (
            "src/app.ts",
            "import { b } from \"pkga\";\nimport { b as c } from \"./a\";\nexport const x = b;\nexport const y = c;\n",
        ),
        ("src/a.ts", "export { b } from \"./b2\";\n"),
        ("src/b2.ts", "export { b } from \"./c3\";\n"),
        ("src/c3.ts", "export { b } from \"./local\";\n"),
        ("src/local.ts", "export { b } from \"pkgb\";\n"),
        (
            "node_modules/pkga/package.json",
            r#"{ "name": "pkga", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/pkga/index.d.ts",
            "export { b } from \"pkgb\";\n",
        ),
        (
            "node_modules/pkgb/package.json",
            r#"{ "name": "pkgb", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/jsp/package.json",
            r#"{ "name": "jsp", "main": "index.js" }"#,
        ),
        ("node_modules/jsp/index.js", "export const j = 7;\n"),
    ];
    if chained {
        files.extend([
            (
                "node_modules/pkgb/index.d.ts",
                "export { b } from \"pkgc\";\n",
            ),
            (
                "node_modules/pkgc/package.json",
                r#"{ "name": "pkgc", "types": "index.d.ts" }"#,
            ),
            (
                "node_modules/pkgc/index.d.ts",
                "export { j as b } from \"jsp\";\n",
            ),
        ]);
    } else {
        files.push((
            "node_modules/pkgb/index.d.ts",
            "export { j as b } from \"jsp\";\n",
        ));
    }
    Fixture::with_dialect(dialect, &files)
}

/// A loaded file that a later route reaches at a lower depth processes its
/// subtasks again from there, so JavaScript it elided earlier is an input
/// once within the limit: `pkgb`'s `jsp` is, and `x` and `y` are both `7`
/// under 5.6.3 (`tsc --listFilesOnly` lists `jsp/index.js`, and its oracle
/// answers `7`), whose `findSourceFile` reprocesses a file with elided
/// imports (`modulesWithElidedImports`) reached below the limit, and under
/// TypeScript 7's `filesParser.start`, which reprocesses the subtasks of a
/// file seen at a lower depth than before (native 7.0.2 races here: its
/// listing includes `jsp` in some runs). Through `pkgc`, a file with no
/// elided import of its own, the dialects part: TypeScript 7 lowers
/// `pkgc`'s depth too and loads `jsp`, while 5.6.3 reprocesses no file
/// without elided imports, never lists `jsp`, and answers `any`.
#[test]
fn a_loaded_file_reached_at_a_lower_depth_reprocesses_its_elided_javascript() {
    for dialect in DIALECTS {
        let fixture = revisit_fixture(dialect, false);
        let project = fixture.load();
        for line in [3, 4] {
            assert_eq!(
                fixture.summary_at(&project, "src/app.ts", line, 14),
                "7",
                "{dialect:?} line {line}"
            );
        }
        let fixture = revisit_fixture(dialect, true);
        let project = fixture.load();
        for line in [3, 4] {
            let answer = fixture.type_at(&project, "src/app.ts", line, 14);
            match dialect {
                TypeScript7 => assert_eq!(answer.unwrap(), "7", "chained line {line}"),
                TypeScript56 => assert_untyped(answer, "chained under TypeScript 5.6"),
            }
        }
    }
}

#[test]
fn javascript_roots_and_reference_paths_are_inputs_under_allow_js() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "allowJs": true, "noLib": true, "noEmit": true, "jsx": "preserve" } }"#,
        ),
        (
            "files.json",
            r#"{ "compilerOptions": { "allowJs": true, "noLib": true, "noEmit": true }, "files": ["a.ts"] }"#,
        ),
        (
            "listed.json",
            r#"{ "compilerOptions": { "allowJs": true, "noLib": true, "noEmit": true }, "files": ["m.mjs", "uses.ts"] }"#,
        ),
        (
            "a.ts",
            "/// <reference path=\"./x\" />\n/// <reference path=\"./y.js\" />\nexport const a = xv + yv;\n",
        ),
        ("x.js", "var xv = 1;\n"),
        ("y.js", "var yv = 2;\n"),
        ("m.mjs", "export const m = 1;\n"),
        ("c.cjs", "export const c = \"c\";\n"),
        ("j.jsx", "export const j = 3;\n"),
        (
            "uses.ts",
            "import { m } from \"./m.mjs\";\nexport const fromM = m;\n",
        ),
    ]);
    // Every JavaScript extension is a root under allowJs, and an
    // extensionless reference path finds `.js`.
    let project = fixture.load();
    assert_eq!(fixture.summary_at(&project, "a.ts", 3, 14), "number");
    assert_eq!(fixture.summary_at(&project, "m.mjs", 1, 14), "1");
    assert_eq!(fixture.summary_at(&project, "c.cjs", 1, 14), "\"c\"");
    assert_eq!(fixture.summary_at(&project, "j.jsx", 1, 14), "3");
    let project = fixture.load_config("files.json");
    assert_eq!(fixture.summary_at(&project, "a.ts", 3, 14), "number");
    let project = fixture.load_config("listed.json");
    assert_eq!(fixture.summary_at(&project, "uses.ts", 2, 14), "1");
}

#[test]
fn without_allow_js_an_imported_javascript_file_is_an_untyped_module() {
    let files = |strict: bool| {
        Fixture::new(&[
            (
                "tsconfig.json",
                &*format!(r#"{{ "compilerOptions": {{ "noLib": true, "strict": {strict} }} }}"#),
            ),
            ("lib.js", "export const v = 1;\n"),
            (
                "app.ts",
                "import { v } from \"./lib.js\";\nexport const x = v;\n",
            ),
        ])
    };
    // GetResolutionDiagnostic: TS7016 under noImplicitAny.
    let fixture = files(true);
    let error = fixture.try_load().unwrap_err();
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
    assert_eq!(
        error.message,
        "Could not find a declaration file for module './lib.js'. 'lib.js' implicitly has an 'any' type."
    );
    assert_eq!(position(&error), (fixture.path("app.ts"), 1, 19));
    // Otherwise the file is still not an input: the module is untyped.
    let fixture = files(false);
    let project = fixture.load();
    assert_untyped(fixture.type_at(&project, "app.ts", 2, 14), "x");
}

/// Every occurrence of `ReferenceQuery::At` `file:line:column`, as
/// `file:line:column` of each occurrence's start, sorted, with the result's
/// outcome.
fn occurrences(
    project: &Project,
    fixture: &Fixture,
    file: &str,
    line: usize,
    column: usize,
) -> (String, Vec<String>) {
    use typescript_parser::references::{
        ReferenceAnswer, ReferenceFilters, ReferenceQuery, ReferenceRequest,
    };
    let answer = project
        .references(ReferenceRequest {
            query: ReferenceQuery::At {
                file: fixture.path(file),
                position: SourcePosition { line, column },
            },
            filters: ReferenceFilters::default(),
        })
        .unwrap_or_else(|error| panic!("{file}:{line}:{column}: {error}"));
    let ReferenceAnswer::Selected(result) = answer else {
        panic!("{file}:{line}:{column}: {answer:?}");
    };
    let mut found: Vec<String> = result
        .groups
        .iter()
        .flat_map(|group| &group.occurrences)
        .map(|occurrence| {
            let start = occurrence.location.start;
            format!(
                "{}:{}:{}",
                occurrence.location.file, start.line, start.column
            )
        })
        .collect();
    found.sort();
    (format!("{:?}", result.outcome), found)
}

/// `--references` for a constant a JavaScript module declares and for a
/// member of one of its object literals reaches every TypeScript import
/// site and use, and the module's own uses: native 7.0.2's semantic and
/// member oracles and 5.6.3's `findReferences` list exactly these.
#[test]
fn references_reach_every_import_of_javascript_constants_and_members() {
    let config = constants_config(r#""es2017""#);
    let fixture = Fixture::new(&constants_files(&config));
    let project = fixture.load();
    let expected = |sites: &[&str]| -> Vec<String> {
        let mut sites: Vec<String> = sites.iter().map(|site| (*site).to_owned()).collect();
        sites.sort();
        sites
    };
    let constant = [
        "src/constants/layers.js:3:14",
        "src/app.ts:1:10",
        "src/app.ts:5:22",
        "src/other.ts:2:10",
        "src/other.ts:5:52",
    ];
    for (file, line, column) in [
        ("src/constants/layers.js", 3, 14),
        ("src/app.ts", 5, 22),
        ("src/other.ts", 5, 52),
    ] {
        let (outcome, found) = occurrences(&project, &fixture, file, line, column);
        assert_eq!(outcome, "Complete", "{file}:{line}:{column}");
        assert_eq!(found, expected(&constant), "{file}:{line}:{column}");
    }
    let member = [
        "src/constants/palette.js:21:5",
        "src/constants/palette.js:33:69",
        "src/app.ts:8:40",
        "src/other.ts:5:24",
    ];
    for (file, line, column) in [
        ("src/constants/palette.js", 21, 5),
        ("src/app.ts", 8, 40),
        ("src/other.ts", 5, 24),
    ] {
        let (outcome, found) = occurrences(&project, &fixture, file, line, column);
        assert_eq!(outcome, "Complete", "{file}:{line}:{column}");
        assert_eq!(found, expected(&member), "{file}:{line}:{column}");
    }
}
