//! Each file's own option values, inheritance, and root lists under both
//! dialects, at the loaded program: its effective options (the test-only
//! copy `Program::options`), its searched files, or its failure. Each case
//! names the characterization experiment it reproduces; expected outcomes
//! are TypeScript 5.6.3's (`parseJsonSourceFileConfigFileContent` and
//! `tsc --incremental false` on scratch copies) and native TypeScript
//! 7.0.2's (`tsc --showConfig` and `--listFilesOnly`).

use super::super::test_compiler::Dialect::{self, TypeScript7, TypeScript56};
use super::super::test_compiler::{DIALECTS, MANIFEST};
use super::super::{Program, disk, load as load_program};
use crate::analysis::{InputError, InputErrorKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Options both dialects accept and read alike, with no library or type
/// package: what TypeScript 5.6's refusals need set. `module` is left to
/// the cases: from an ESNext target, 5.6.3 computes ES2015 and 7.0.2
/// ESNext.
const BASE: &str = r#""noLib":true,"types":[],"target":"esnext","moduleResolution":"bundler","esModuleInterop":true,"alwaysStrict":true"#;

/// A scratch project whose installed compiler is `dialect`'s, removed when
/// dropped.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(dialect: Dialect, files: &[(&str, &str)]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "tsparse-conversion-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        let manifest = dialect.manifest();
        for (path, text) in files.iter().copied().chain([(MANIFEST, manifest.as_str())]) {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        Self { root }
    }

    fn load(&self, config: &str) -> Result<Program, InputError> {
        let config = self.root.join(config);
        crate::analysis::on_parsing_thread("tsparse-test-load", |parser| {
            load_program(&config, &disk::Disk::default(), parser)
        })
        .unwrap()
    }

    /// `relative` under the canonical root, as loading names paths.
    fn path(&self, relative: &str) -> String {
        let root = std::fs::canonicalize(&self.root).unwrap();
        let root = root.to_str().unwrap();
        match relative {
            "" => root.to_owned(),
            _ => format!("{root}/{relative}"),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Load `files` under `dialect`'s compiler from `leaf/tsconfig.json`.
fn load(dialect: Dialect, files: &[(&str, &str)]) -> (Scratch, Result<Program, InputError>) {
    let scratch = Scratch::new(dialect, files);
    let loaded = scratch.load("leaf/tsconfig.json");
    (scratch, loaded)
}

fn loaded(dialect: Dialect, files: &[(&str, &str)]) -> (Scratch, Program) {
    let (scratch, loaded) = load(dialect, files);
    let program = loaded.unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
    (scratch, program)
}

/// A leaf configuration with `BASE`, `options`, and `rest` (other root
/// properties, each preceded by a comma), selecting `leaf/a.ts`.
fn leaf(options: &str, rest: &str) -> String {
    format!(r#"{{"compilerOptions":{{{BASE}{options}}},"files":["a.ts"]{rest}}}"#)
}

const SOURCE: (&str, &str) = ("leaf/a.ts", "export const a = 1;\n");

/// The text from where `error`, in a one-line `config`, is located to the
/// end of the line.
fn located<'c>(config: &'c str, error: &InputError) -> &'c str {
    let position = error.position.expect("located");
    assert_eq!(position.line, 1, "{error}");
    &config[position.column - 1..]
}

/// The strict-family flags of the snapshot: all on or all off here.
fn strict(program: &Program) -> bool {
    let semantic = &program.options.semantic;
    let flags = [
        semantic.no_implicit_any,
        semantic.strict_null_checks,
        semantic.strict_function_types,
        semantic.use_unknown_in_catch_variables,
    ];
    assert!(flags.iter().all(|&flag| flag == flags[0]), "{flags:?}");
    flags[0]
}

/// TypeScript 5.6.3's `onPropertySet` assigns every occurrence's converted
/// value, `undefined` for a `null`, so a later `null` in the same object,
/// in a later `compilerOptions` object, or over an inherited value leaves
/// `strict` unset, which is false under 5.6. TypeScript 7.0.2 skips a nil
/// value within a file, and only an explicit `null` over an inherited value
/// unsets it, which is true under 7. Experiments duplicate-strict-null,
/// duplicate-objects-strict, inherit-strict-null, each also with `false`.
#[test]
fn a_null_after_a_value_unsets_it_under_5_6() {
    for value in [true, false] {
        let duplicate = leaf(&format!(r#","strict":{value},"strict":null"#), "");
        let objects = format!(
            r#"{{"compilerOptions":{{{BASE},"strict":{value}}},"compilerOptions":{{"strict":null}},"files":["a.ts"]}}"#
        );
        let inherited = leaf(r#","strict":null"#, r#","extends":"../base.json""#);
        let base = format!(r#"{{"compilerOptions":{{"strict":{value}}}}}"#);
        for (name, config, seven) in [
            ("duplicate-strict-null", duplicate.as_str(), value),
            ("duplicate-objects-strict", objects.as_str(), value),
            ("inherit-strict-null", inherited.as_str(), true),
        ] {
            for dialect in DIALECTS {
                let (_scratch, program) = loaded(
                    dialect,
                    &[("leaf/tsconfig.json", config), ("base.json", &base), SOURCE],
                );
                let expected = match dialect {
                    TypeScript7 => seven,
                    TypeScript56 => false,
                };
                assert_eq!(strict(&program), expected, "{name} {value} {dialect:?}");
            }
        }
    }
}

/// One option's own and inherited variants under both dialects: a later
/// `null` in the same object (`duplicate-*-null`), a `null` in a later
/// `compilerOptions` object (`duplicate-objects-*`), and an own `null` over
/// an inherited value (`inherit-*-null`), with `first` the value before
/// it. `effective` reads the option from the loaded program.
fn null_variants<T: PartialEq + std::fmt::Debug>(
    option: &str,
    first: &str,
    effective: impl Fn(&Scratch, &Program) -> T,
    expected: impl Fn(Dialect, &str, &Scratch) -> T,
) {
    let duplicate = leaf(&format!(r#","{option}":{first},"{option}":null"#), "");
    let objects = format!(
        r#"{{"compilerOptions":{{{BASE},"{option}":{first}}},"compilerOptions":{{"{option}":null}},"files":["a.ts"]}}"#
    );
    let inherited = leaf(
        &format!(r#","{option}":null"#),
        r#","extends":"../base.json""#,
    );
    let base = format!(r#"{{"compilerOptions":{{"{option}":{first}}}}}"#);
    for (name, config) in [
        ("duplicate", duplicate.as_str()),
        ("duplicate-objects", objects.as_str()),
        ("inherit", inherited.as_str()),
    ] {
        for dialect in DIALECTS {
            let (scratch, program) = loaded(
                dialect,
                &[("leaf/tsconfig.json", config), ("base.json", &base), SOURCE],
            );
            assert_eq!(
                effective(&scratch, &program),
                expected(dialect, name, &scratch),
                "{name}-{option}-null {dialect:?}"
            );
        }
    }
}

/// TypeScript 7.0.2 keeps the earlier value of a file's own duplicate and
/// unsets only an inherited one; 5.6.3 unsets all three. Experiments
/// duplicate-jsxFactory-null, duplicate-objects-jsxFactory,
/// inherit-jsxFactory-null and their `rootDir`, `module`, and `paths`
/// counterparts.
#[test]
fn a_null_unsets_strings_paths_enumerations_and_objects_under_5_6() {
    let kept = |dialect: Dialect, name: &str| dialect == TypeScript7 && name != "inherit";
    null_variants(
        "jsxFactory",
        r#""baseFactory""#,
        |_, program| program.options.semantic.jsx_factory.clone(),
        |dialect, name, _| kept(dialect, name).then(|| "baseFactory".to_owned()),
    );
    null_variants(
        "rootDir",
        r#""baseRoot""#,
        |_, program| program.options.root_dir.clone(),
        |dialect, name, scratch| {
            // A file's own `rootDir` is relative to it; an inherited one,
            // to the base.
            kept(dialect, name).then(|| scratch.path("leaf/baseRoot"))
        },
    );
    // From an ESNext target, 5.6.3's unset `module` is ES2015, 7.0.2's
    // ESNext.
    null_variants(
        "module",
        r#""es2020""#,
        |_, program| program.options.semantic.module_kind,
        |dialect, name, _| match dialect {
            TypeScript56 => super::super::ModuleKind::Es2015,
            TypeScript7 if kept(dialect, name) => super::super::ModuleKind::Es2020,
            TypeScript7 => super::super::ModuleKind::EsNext,
        },
    );
    null_variants(
        "paths",
        r#"{"a":["./a"]}"#,
        |_, program| {
            program
                .options
                .paths
                .as_ref()
                .map(|paths| paths.patterns.clone())
        },
        |dialect, name, _| {
            kept(dialect, name).then(|| vec![("a".to_owned(), vec!["./a".to_owned()])])
        },
    );
}

/// An empty string replaces an inherited string under 5.6.3, where an
/// empty JSX factory or import source then selects the default, as every
/// consumer of those options tests them for truthiness; 7.0.2 keeps the
/// inherited value, but takes a file's own empty string over its earlier
/// value. An empty path names the declaring configuration's directory in
/// both. Experiments empty-own-jsxFactory, empty-inherited-jsxFactory,
/// empty-own-rootDir, empty-inherited-rootDir.
#[test]
fn an_empty_string_replaces_an_inherited_string_under_5_6() {
    for option in ["jsxFactory", "jsxImportSource", "rootDir"] {
        let own = leaf(&format!(r#","{option}":"base","{option}":"""#), "");
        let inherited = leaf(
            &format!(r#","{option}":"""#),
            r#","extends":"../base.json""#,
        );
        let base = format!(r#"{{"compilerOptions":{{"{option}":"base"}}}}"#);
        for (name, config) in [("own", own.as_str()), ("inherited", inherited.as_str())] {
            for dialect in DIALECTS {
                let (scratch, program) = loaded(
                    dialect,
                    &[("leaf/tsconfig.json", config), ("base.json", &base), SOURCE],
                );
                let options = &program.options;
                let (actual, expected) = match option {
                    "jsxFactory" => (
                        options.semantic.jsx_factory.clone(),
                        (dialect == TypeScript7 && name == "inherited").then(|| "base".to_owned()),
                    ),
                    "jsxImportSource" => (
                        options.jsx_import_source.clone(),
                        (dialect == TypeScript7 && name == "inherited").then(|| "base".to_owned()),
                    ),
                    _ => (options.root_dir.clone(), Some(scratch.path("leaf"))),
                };
                assert_eq!(actual, expected, "empty-{name}-{option} {dialect:?}");
            }
        }
    }
}

/// An empty enumeration value is TS6046 under 5.6.3, own or over an
/// inherited value; 7.0.2 converts it to nothing, keeping the value before
/// it. Experiments empty-own-module, empty-inherited-module.
#[test]
fn an_empty_enumeration_value_is_an_error_under_5_6() {
    let own = leaf(r#","module":"es2020","module":"""#, "");
    let inherited = leaf(r#","module":"""#, r#","extends":"../base.json""#);
    let base = r#"{"compilerOptions":{"module":"es2020"}}"#;
    for config in [own.as_str(), inherited.as_str()] {
        let files = [("leaf/tsconfig.json", config), ("base.json", base), SOURCE];
        let (_scratch, program) = loaded(TypeScript7, &files);
        assert_eq!(
            program.options.semantic.module_kind,
            super::super::ModuleKind::Es2020
        );
        let (_scratch, loaded) = load(TypeScript56, &files);
        let error = loaded.expect_err("an empty `module` loads");
        assert_eq!(error.kind, InputErrorKind::Config);
        assert_eq!(
            error.message,
            "Argument for '--module' option must be: 'none', 'commonjs', 'amd', 'system', 'umd', 'es6', 'es2015', 'es2020', 'es2022', 'esnext', 'node16', 'nodenext', 'preserve'"
        );
        assert!(located(config, &error).starts_with(r#"""}"#), "{error}");
    }
}

/// The `paths` patterns the program reads.
fn patterns(program: &Program) -> Option<Vec<(String, Vec<String>)>> {
    let paths = program.options.paths.as_ref()?;
    Some(paths.patterns.clone())
}

/// An own `paths: {}` replaces inherited patterns, and an own `{}` after a
/// mapping replaces it, in both dialects; a repeated pattern keeps its last
/// substitutions, and patterns keep their written order. Experiments
/// empty-own-paths, empty-inherited-paths, paths-duplicate-key, and the
/// supplemental integer-path-order.
#[test]
fn an_own_empty_paths_object_replaces_every_pattern() {
    let own = leaf(r#","paths":{"a":["./a"]},"paths":{}"#, "");
    let inherited = leaf(r#","paths":{}"#, r#","extends":"../base.json""#);
    let duplicate = leaf(r#","paths":{"a":["./one"],"a":["./two"]}"#, "");
    let integers = leaf(r#","paths":{"1":["./one"],"2":["./two"],"a":["./a"]}"#, "");
    let base = r#"{"compilerOptions":{"paths":{"a":["./a"]}}}"#;
    for dialect in DIALECTS {
        for (name, config, expected) in [
            ("empty-own-paths", own.as_str(), vec![]),
            ("empty-inherited-paths", inherited.as_str(), vec![]),
            (
                "paths-duplicate-key",
                duplicate.as_str(),
                vec![("a".to_owned(), vec!["./two".to_owned()])],
            ),
            (
                "integer-path-order",
                integers.as_str(),
                ["1", "2", "a"]
                    .map(|key| {
                        let target = match key {
                            "1" => "./one",
                            "2" => "./two",
                            _ => "./a",
                        };
                        (key.to_owned(), vec![target.to_owned()])
                    })
                    .to_vec(),
            ),
        ] {
            let (_scratch, program) = loaded(
                dialect,
                &[("leaf/tsconfig.json", config), ("base.json", base), SOURCE],
            );
            assert_eq!(patterns(&program), Some(expected), "{name} {dialect:?}");
        }
    }
}

/// TypeScript 5.6.3's `convertObjectLiteralExpressionToJson` keeps a
/// property with an empty name, so `""` is a `paths` pattern; 7.0.2 leaves
/// it out. Experiment empty-path-key.
#[test]
fn an_empty_paths_pattern_is_kept_under_5_6() {
    let config = leaf(r#","paths":{"":["./a.ts"],"b":["./b"]}"#, "");
    for dialect in DIALECTS {
        let (_scratch, program) = loaded(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
        let mut expected = vec![("b".to_owned(), vec!["./b".to_owned()])];
        if dialect == TypeScript56 {
            expected.insert(0, (String::new(), vec!["./a.ts".to_owned()]));
        }
        assert_eq!(patterns(&program), Some(expected), "{dialect:?}");
    }
}

/// JavaScript artifacts 5.6.3 would reproduce and this tool does not, as
/// they would change names: a `__proto__` pattern, whose assignment sets
/// the mapping's prototype rather than a pattern (7.0.2 keeps the
/// pattern), and a `null` `moduleSuffixes` entry, which 5.6.3 keeps as
/// `undefined` and spells `undefined` in file names (7.0.2 drops it). Each
/// is refused only where it applies. Experiments proto-path-key,
/// modulesuffix-null.
#[test]
fn javascript_artifacts_are_refused_under_5_6() {
    let proto = leaf(r#","paths":{"__proto__":["./a.ts"]}"#, "");
    let suffixes = leaf(r#","moduleSuffixes":[null,""]"#, "");
    let (_scratch, program) = loaded(TypeScript7, &[("leaf/tsconfig.json", &proto), SOURCE]);
    assert_eq!(
        patterns(&program),
        Some(vec![("__proto__".to_owned(), vec!["./a.ts".to_owned()])])
    );
    let (_scratch, program) = loaded(TypeScript7, &[("leaf/tsconfig.json", &suffixes), SOURCE]);
    assert_eq!(program.options.module_suffixes, [""]);
    for (config, at, message) in [
        (
            &proto,
            r#""__proto__""#,
            "TypeScript 5 would set the prototype of 'paths' with the pattern '__proto__' rather than map it, which is not supported",
        ),
        (
            &suffixes,
            "null",
            "TypeScript 5 keeps a null 'moduleSuffixes' entry and spells it 'undefined' in file names, which is not supported",
        ),
    ] {
        let (_scratch, loaded) = load(TypeScript56, &[("leaf/tsconfig.json", config), SOURCE]);
        let error = loaded.expect_err("loads");
        assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
        assert_eq!(error.message, message);
        assert!(located(config, &error).starts_with(at), "{error}");
    }
    // Overridden, neither applies.
    let overridden = leaf(
        r#","paths":{"__proto__":["./a.ts"]},"paths":{},"moduleSuffixes":[null],"moduleSuffixes":[".ios",""]"#,
        "",
    );
    let (_scratch, program) = loaded(TypeScript56, &[("leaf/tsconfig.json", &overridden), SOURCE]);
    assert_eq!(patterns(&program), Some(vec![]));
    assert_eq!(program.options.module_suffixes, [".ios", ""]);
}

/// List entries that convert to nothing are left out, except that an empty
/// path names the declaring configuration's directory, and `plugins`
/// keeps its objects; in both dialects. Experiments
/// empty-own-rootDirs-null-elements, empty-inherited-rootDirs-null-elements,
/// plugins-null.
#[test]
fn list_entries_that_convert_to_nothing_are_left_out() {
    let own = leaf(r#","rootDirs":["x"],"rootDirs":[null,""]"#, "");
    let inherited = leaf(r#","rootDirs":[null,""]"#, r#","extends":"../base.json""#);
    let plugins = leaf(r#","plugins":[null,{}]"#, "");
    let base = r#"{"compilerOptions":{"rootDirs":["x"]}}"#;
    for dialect in DIALECTS {
        for config in [&own, &inherited] {
            let (scratch, program) = loaded(
                dialect,
                &[("leaf/tsconfig.json", config), ("base.json", base), SOURCE],
            );
            assert_eq!(
                program.options.root_dirs,
                [scratch.path("leaf")],
                "{dialect:?}"
            );
        }
        loaded(dialect, &[("leaf/tsconfig.json", &plugins), SOURCE]);
    }
}

/// Each `compilerOptions` object's properties are set as they are read, so
/// a later `compilerOptions: null` or `{}` erases nothing; an own `null`
/// under `extends: []` unsets the option in both dialects. A value that a
/// later one overrides is still validated. Experiments compilerobjects-null,
/// compilerobjects-empty, own-null-with-extends-empty, bad-earlier-value.
#[test]
fn compiler_options_objects_accumulate_and_every_value_is_validated() {
    for (name, later) in [
        ("compilerobjects-null", "null"),
        ("compilerobjects-empty", "{}"),
    ] {
        let config = format!(
            r#"{{"compilerOptions":{{{BASE},"strict":true}},"compilerOptions":{later},"files":["a.ts"]}}"#
        );
        for dialect in DIALECTS {
            let (_scratch, program) = loaded(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
            assert!(strict(&program), "{name} {dialect:?}");
        }
    }
    // `strict: false` first, which 7.0.2's default would hide.
    let config = leaf(r#","strict":false,"strict":null"#, r#","extends":[]"#);
    for dialect in DIALECTS {
        let (_scratch, program) = loaded(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
        // Unset: 7.0.2's default is on, 5.6.3's off.
        assert_eq!(strict(&program), dialect == TypeScript7, "{dialect:?}");
    }
    let config = leaf(r#","strict":"bad","strict":true"#, "");
    for dialect in DIALECTS {
        let (_scratch, loaded) = load(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
        let error = loaded.expect_err("loads");
        assert_eq!(error.kind, InputErrorKind::Config, "{dialect:?}");
        assert_eq!(
            error.message,
            "Compiler option 'strict' requires a value of type boolean"
        );
        assert!(located(&config, &error).starts_with(r#""bad""#));
    }
}

/// A value of the wrong type is TS5024 in both dialects, under 5.6 naming a
/// list entry by its element declaration and an enumeration as a string.
/// TypeScript 7 cases keep this tool's wording. Experiments
/// wrong-array-element, wrong-enum-type.
#[test]
fn values_of_the_wrong_type_are_errors() {
    for (options, at, five) in [
        (
            r#","types":[123]"#,
            "123",
            "Compiler option 'types' requires a value of type string",
        ),
        (
            r#","module":123"#,
            "123",
            "Compiler option 'module' requires a value of type string",
        ),
    ] {
        let config = leaf(options, "").replace(r#""types":[],"#, "");
        for dialect in DIALECTS {
            let (_scratch, loaded) = load(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
            let error = loaded.expect_err("loads");
            assert_eq!(error.kind, InputErrorKind::Config, "{dialect:?} {error}");
            assert!(located(&config, &error).starts_with(at), "{error}");
            if dialect == TypeScript56 {
                assert_eq!(error.message, five);
            }
        }
    }
}

/// Enumeration values match their keys in any case and are not trimmed, in
/// both dialects. Experiments enum-case, enum-space.
#[test]
fn enumeration_values_ignore_case_but_not_spaces() {
    let config = leaf(r#","module":"EsNeXt""#, "");
    for dialect in DIALECTS {
        let (_scratch, program) = loaded(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
        assert_eq!(
            program.options.semantic.module_kind,
            super::super::ModuleKind::EsNext
        );
    }
    let config = leaf(r#","module":" esnext ""#, "");
    for (dialect, name) in [(TypeScript7, "module"), (TypeScript56, "--module")] {
        let (_scratch, loaded) = load(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
        let error = loaded.expect_err("loads");
        assert_eq!(error.kind, InputErrorKind::Config, "{dialect:?}");
        assert!(
            error
                .message
                .starts_with(&format!("Argument for '{name}' option must be: ")),
            "{error}"
        );
    }
}

/// The last `extends` applies; bases apply in order, a later base's `null`
/// unsetting and its empty string replacing under 5.6.3, while 7.0.2
/// unsets an option a base's last raw value is `null` for and keeps a
/// value over an empty string; a base's `include: null` passes inheritance
/// to the one before it in both. Experiments duplicate-extends,
/// bases-null-last.
#[test]
fn bases_apply_in_order_by_each_dialects_rule() {
    let config = format!(
        r#"{{"compilerOptions":{{{BASE}}},"files":["a.ts"],"extends":"../one.json","extends":"../two.json"}}"#
    );
    let one = r#"{"compilerOptions":{"strict":false,"jsxFactory":"one"},"include":["base.ts"]}"#;
    let two = r#"{"compilerOptions":{"jsxFactory":"two"}}"#;
    for dialect in DIALECTS {
        let files = [
            ("leaf/tsconfig.json", config.as_str()),
            ("one.json", one),
            ("two.json", two),
            SOURCE,
        ];
        let (_scratch, program) = loaded(dialect, &files);
        let semantic = &program.options.semantic;
        assert_eq!(semantic.jsx_factory.as_deref(), Some("two"), "{dialect:?}");
    }
    let config =
        format!(r#"{{"compilerOptions":{{{BASE}}},"extends":["../one.json","../two.json"]}}"#);
    let one = r#"{"compilerOptions":{"strict":true,"jsxFactory":"one"},"include":["base.ts"]}"#;
    let two = r#"{"compilerOptions":{"strict":null,"jsxFactory":""},"include":null}"#;
    for dialect in DIALECTS {
        let files = [
            ("leaf/tsconfig.json", config.as_str()),
            ("one.json", one),
            ("two.json", two),
            ("base.ts", "export {};\n"),
            SOURCE,
        ];
        let (_scratch, program) = loaded(dialect, &files);
        assert_eq!(searched(&program), ["../base.ts"], "{dialect:?}");
        let semantic = &program.options.semantic;
        // `strict` is unset in both: 7.0.2's default is on, 5.6.3's off.
        assert_eq!(strict(&program), dialect == TypeScript7, "{dialect:?}");
        let factory = (dialect == TypeScript7).then_some("one");
        assert_eq!(semantic.jsx_factory.as_deref(), factory, "{dialect:?}");
    }
}

/// The searched files' display paths, relative to the configuration, in
/// order.
fn searched(program: &Program) -> Vec<&str> {
    let mut searched: Vec<&str> = program
        .sources
        .ids()
        .filter(|id| program.files[id.index()].search)
        .map(|id| program.sources.get(id).display.as_str())
        .collect();
    searched.sort_unstable();
    searched
}

/// A nested `ts-node.compilerOptions` object is not compiler options in
/// either dialect: only the root's `compilerOptions` are read.
#[test]
fn ts_node_compiler_options_are_inert() {
    let config = leaf(
        r#","module":"esnext""#,
        r#","ts-node":{"compilerOptions":{"module":"CommonJS","moduleResolution":"node","strict":false}}"#,
    );
    for dialect in DIALECTS {
        let (_scratch, program) = loaded(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
        let semantic = &program.options.semantic;
        assert_eq!(semantic.module_kind, super::super::ModuleKind::EsNext);
        assert_eq!(semantic.module_resolution, super::Resolution::Bundler);
    }
}

/// What a configuration selects: its searched files, or its failure's kind
/// and message.
#[derive(Debug, PartialEq, Eq)]
enum Selected {
    Files(Vec<String>),
    Fails(InputErrorKind, String),
}

fn files(names: &[&str]) -> Selected {
    Selected::Files(names.iter().map(|name| (*name).to_owned()).collect())
}

/// TS18003 for `leaf/tsconfig.json` in `scratch`, as this tool words it.
fn no_inputs(scratch: &Scratch, include: &str, exclude: &str) -> Selected {
    Selected::Fails(
        InputErrorKind::Config,
        format!(
            "No inputs were found in config file '{}'. Specified 'include' paths were '{include}' and 'exclude' paths were '{exclude}'",
            scratch.root.join("leaf/tsconfig.json").display()
        ),
    )
}

/// A case's name, the leaf's own lists, whether it has a base, and what
/// it selects under each dialect.
type OwnCase = (
    &'static str,
    &'static str,
    &'static str,
    fn(&Scratch, Dialect) -> Selected,
);

/// A case's name, its base, and what it selects under each dialect.
type BaseCase = (&'static str, String, fn(&Scratch, Dialect) -> Selected);

fn select(dialect: Dialect, files: &[(&str, &str)]) -> (Scratch, Selected) {
    let (scratch, loaded) = load(dialect, files);
    let selected = match loaded {
        Ok(program) => Selected::Files(searched(&program).into_iter().map(str::to_owned).collect()),
        Err(error) => Selected::Fails(error.kind, error.message),
    };
    (scratch, selected)
}

/// The leaf's own `files`, `include`, or `exclude` value over a base's in
/// the parent directory that sets only that list, with `leaf/main.ts`,
/// `leaf/other.ts`, and `top.ts` beside the base. 5.6.3 inherits over an own `null`; 7.0.2's
/// own key blocks inheritance even as `null`. An own `[]` is an explicit
/// empty list in both. An own `[null]` lists nothing under 5.6.3, so
/// `include` finds no inputs; 7.0.2's raw conversion makes it no list at
/// all, so the default `include` applies. An own `""` names the
/// directory under 7.0.2, while 5.6.3's `include` and `exclude` match
/// nothing with it. Experiments files-null, files-[], files-[null],
/// files-[""], include-null, include-[], include-[null], include-[""],
/// exclude-null, exclude-[], exclude-[null], exclude-[""], and the scratch
/// runs own-files-null-only, own-include-null-only,
/// leaf-include-empty-and-main, own-exclude-empty.
#[test]
fn own_root_lists_follow_each_dialects_rule() {
    let cases: [OwnCase; 16] = [
        (
            "files-null",
            r#""files":null"#,
            "",
            |_, dialect| match dialect {
                TypeScript56 => files(&["../top.ts"]),
                TypeScript7 => files(&["main.ts", "other.ts"]),
            },
        ),
        ("files-[]", r#""files":[]"#, "", |_, _| files(&[])),
        (
            "files-[null]",
            r#""files":[null]"#,
            "",
            |_, dialect| match dialect {
                TypeScript56 => files(&[]),
                TypeScript7 => files(&["main.ts", "other.ts"]),
            },
        ),
        ("files-[\"\"]", r#""files":[""]"#, "", |_, _| {
            Selected::Fails(InputErrorKind::Io, "File '' not found".into())
        }),
        (
            "include-null",
            r#""include":null"#,
            "",
            |_, dialect| match dialect {
                TypeScript56 => files(&["../top.ts"]),
                TypeScript7 => files(&["main.ts", "other.ts"]),
            },
        ),
        ("include-[]", r#""include":[]"#, "", |scratch, _| {
            no_inputs(scratch, "[]", "[]")
        }),
        (
            "include-[null]",
            r#""include":[null]"#,
            "",
            |scratch, dialect| match dialect {
                TypeScript56 => no_inputs(scratch, "[null]", "[]"),
                TypeScript7 => files(&["main.ts", "other.ts"]),
            },
        ),
        (
            "include-[\"\"]",
            r#""include":[""]"#,
            "",
            |scratch, dialect| match dialect {
                TypeScript56 => no_inputs(scratch, r#"[""]"#, "[]"),
                TypeScript7 => files(&["main.ts", "other.ts"]),
            },
        ),
        (
            "include-[\"\", \"main.ts\"]",
            r#""include":["","main.ts"]"#,
            "",
            |_, dialect| match dialect {
                TypeScript56 => files(&["main.ts"]),
                TypeScript7 => files(&["main.ts", "other.ts"]),
            },
        ),
        (
            "exclude-null",
            r#""include":["*.ts"],"exclude":null"#,
            "",
            |_, dialect| match dialect {
                TypeScript56 => files(&["main.ts"]),
                TypeScript7 => files(&["main.ts", "other.ts"]),
            },
        ),
        (
            "exclude-[]",
            r#""include":["*.ts"],"exclude":[]"#,
            "",
            |_, _| files(&["main.ts", "other.ts"]),
        ),
        (
            "exclude-[null]",
            r#""include":["*.ts"],"exclude":[null]"#,
            "",
            |_, _| files(&["main.ts", "other.ts"]),
        ),
        (
            "exclude-[\"\"]",
            r#""include":["*.ts"],"exclude":[""]"#,
            "",
            |scratch, dialect| match dialect {
                TypeScript56 => files(&["main.ts", "other.ts"]),
                TypeScript7 => no_inputs(scratch, r#"["*.ts"]"#, r#"[""]"#),
            },
        ),
        // No base: an own `null` then inherits nothing.
        ("own-files-null-only", r#""files":null"#, "none", |_, _| {
            files(&["main.ts", "other.ts"])
        }),
        (
            "own-include-null-only",
            r#""include":[null]"#,
            "none",
            |scratch, dialect| match dialect {
                TypeScript56 => no_inputs(scratch, "[null]", "[]"),
                TypeScript7 => files(&["main.ts", "other.ts"]),
            },
        ),
        (
            "files-[null, \"main.ts\"]",
            r#""files":[null,"main.ts"]"#,
            "",
            |_, _| files(&["main.ts"]),
        ),
    ];
    for (name, own, bases, expected) in cases {
        let extends = match bases {
            "none" => "",
            _ => r#""extends":"../base.json","#,
        };
        // A base with only the list the case is about.
        let base = if own.contains("exclude") {
            r#"{"exclude":["leaf/other.ts"]}"#
        } else if own.contains("include") {
            r#"{"include":["top.ts"]}"#
        } else {
            r#"{"files":["top.ts"]}"#
        };
        let config = format!(r#"{{{extends}"compilerOptions":{{{BASE}}},{own}}}"#);
        for dialect in DIALECTS {
            let (scratch, selected) = select(
                dialect,
                &[
                    ("leaf/tsconfig.json", &config),
                    ("base.json", base),
                    ("leaf/main.ts", "export {};\n"),
                    ("leaf/other.ts", "export {};\n"),
                    ("top.ts", "export {};\n"),
                ],
            );
            assert_eq!(selected, expected(&scratch, dialect), "{name} {dialect:?}");
        }
    }
}

/// A base's `files`, `include`, or `exclude` value under a leaf that has
/// none, the base in the parent directory unless noted. 5.6.3 rebases
/// each inherited entry before filtering it, so a `null` or `""` names the
/// base's directory, or, from a base beside the leaf, is an empty spec
/// that matches nothing; 7.0.2's raw conversion makes `[null]` no list,
/// which is not inherited. Experiments files-base-[null], files-base-[""],
/// files-base-[], include-base-[null], include-base-[""], include-base-[],
/// exclude-base-[null], exclude-base-[""], exclude-base-[], and the
/// scratch runs same-dir-base-include-empty, same-dir-base-include-null,
/// parent-base-include-null, roundtrip-base-include-null,
/// two-bases-second-null-list.
#[test]
fn inherited_root_lists_are_rebased_before_filtering_under_5_6() {
    let parent = |list: &str| format!(r#"{{{list}}}"#);
    let cases: [BaseCase; 9] = [
        (
            "files-base-[null]",
            parent(r#""files":[null]"#),
            |_, dialect| match dialect {
                // The base's directory, which is no file.
                TypeScript56 => Selected::Fails(InputErrorKind::Io, "File '' not found".into()),
                TypeScript7 => files(&["main.ts"]),
            },
        ),
        ("files-base-[\"\"]", parent(r#""files":[""]"#), |_, _| {
            Selected::Fails(InputErrorKind::Io, "File '' not found".into())
        }),
        ("files-base-[]", parent(r#""files":[]"#), |_, _| files(&[])),
        (
            "include-base-[null]",
            parent(r#""include":[null]"#),
            |_, dialect| match dialect {
                TypeScript56 => files(&["../top.ts", "main.ts"]),
                TypeScript7 => files(&["main.ts"]),
            },
        ),
        (
            "include-base-[\"\"]",
            parent(r#""include":[""]"#),
            |_, _| files(&["../top.ts", "main.ts"]),
        ),
        (
            "include-base-[]",
            parent(r#""include":[]"#),
            |scratch, _| no_inputs(scratch, "[]", "[]"),
        ),
        (
            "exclude-base-[null]",
            parent(r#""exclude":[null]"#),
            |scratch, dialect| match dialect {
                TypeScript56 => no_inputs(scratch, r#"["**/*"]"#, "[null]"),
                TypeScript7 => files(&["main.ts"]),
            },
        ),
        (
            "exclude-base-[\"\"]",
            parent(r#""exclude":[""]"#),
            |scratch, _| no_inputs(scratch, r#"["**/*"]"#, r#"[""]"#),
        ),
        ("exclude-base-[]", parent(r#""exclude":[]"#), |_, _| {
            files(&["main.ts"])
        }),
    ];
    let config = format!(r#"{{"extends":"../base.json","compilerOptions":{{{BASE}}}}}"#);
    for (name, base, expected) in cases {
        for dialect in DIALECTS {
            let (scratch, selected) = select(
                dialect,
                &[
                    ("leaf/tsconfig.json", &config),
                    ("base.json", &base),
                    ("leaf/main.ts", "export {};\n"),
                    ("top.ts", "export {};\n"),
                ],
            );
            assert_eq!(selected, expected(&scratch, dialect), "{name} {dialect:?}");
        }
    }

    // A base beside the leaf: its `null` or `""`, rebased, is an empty spec
    // under 5.6.3, so `include` finds nothing.
    let config = format!(r#"{{"extends":"./base.json","compilerOptions":{{{BASE}}}}}"#);
    for entry in ["null", r#""""#] {
        let base = format!(r#"{{"include":[{entry}]}}"#);
        for dialect in DIALECTS {
            let (scratch, selected) = select(
                dialect,
                &[
                    ("leaf/tsconfig.json", &config),
                    ("leaf/base.json", &base),
                    ("leaf/main.ts", "export {};\n"),
                ],
            );
            let expected = match dialect {
                TypeScript56 => no_inputs(&scratch, &format!("[{entry}]"), "[]"),
                TypeScript7 => files(&["main.ts"]),
            };
            assert_eq!(
                selected, expected,
                "same-dir-base-include {entry} {dialect:?}"
            );
        }
    }

    // A base beside the leaf reached through another directory: rebased
    // twice, the `null` names the base's directory, the leaf's.
    let config = format!(r#"{{"extends":"../mid/mid.json","compilerOptions":{{{BASE}}}}}"#);
    for dialect in DIALECTS {
        let (_scratch, selected) = select(
            dialect,
            &[
                ("leaf/tsconfig.json", &config),
                ("mid/mid.json", r#"{"extends":"../leaf/base.json"}"#),
                ("leaf/base.json", r#"{"include":[null]}"#),
                ("leaf/main.ts", "export {};\n"),
                ("mid/m.ts", "export {};\n"),
            ],
        );
        assert_eq!(selected, files(&["main.ts"]), "roundtrip {dialect:?}");
    }

    // Two bases: the later one's `[null]` is a list under 5.6.3, naming its
    // directory, and no list under 7.0.2, so the earlier one's applies.
    let config =
        format!(r#"{{"extends":["../one.json","../two.json"],"compilerOptions":{{{BASE}}}}}"#);
    for dialect in DIALECTS {
        let (_scratch, selected) = select(
            dialect,
            &[
                ("leaf/tsconfig.json", &config),
                ("one.json", r#"{"include":["leaf/a.ts"]}"#),
                ("two.json", r#"{"include":[null]}"#),
                ("leaf/main.ts", "export {};\n"),
                ("leaf/a.ts", "export {};\n"),
            ],
        );
        let expected = match dialect {
            TypeScript56 => files(&["a.ts", "main.ts"]),
            TypeScript7 => files(&["a.ts"]),
        };
        assert_eq!(selected, expected, "two-bases-second-null-list {dialect:?}");
    }
}

/// `extends` entries: an empty string and a `null` are errors in both
/// dialects; a `null` entry of a list is TS5024 under 5.6.3, which also
/// applies the bases beside it, and nothing under 7.0.2, whose raw
/// conversion drops it. Experiments extends-null, extends-empty,
/// extends-array-null, and the scratch runs extends-null-and-base,
/// extends-number.
#[test]
fn extends_entries_follow_each_dialects_rule() {
    let base = r#"{"compilerOptions":{"strict":false}}"#;
    for (name, extends, seven, five) in [
        (
            "extends-null",
            "null",
            Some("'extends' must be a string or an array of strings"),
            Some("Compiler option 'extends' requires a value of type string or Array"),
        ),
        (
            "extends-empty",
            r#""""#,
            Some("'extends' cannot be an empty string"),
            Some("Compiler option 'extends' cannot be given an empty string"),
        ),
        (
            "extends-array-null",
            "[null]",
            None,
            Some("Compiler option 'extends' requires a value of type string"),
        ),
        (
            "extends-null-and-base",
            r#"[null,"../base.json"]"#,
            None,
            Some("Compiler option 'extends' requires a value of type string"),
        ),
        (
            "extends-number",
            "[1]",
            Some("'extends' entries must be strings"),
            Some("Compiler option 'extends' requires a value of type string"),
        ),
    ] {
        let config = leaf("", &format!(r#","extends":{extends}"#));
        for (dialect, message) in [(TypeScript7, seven), (TypeScript56, five)] {
            let (_scratch, loaded) = load(
                dialect,
                &[("leaf/tsconfig.json", &config), ("base.json", base), SOURCE],
            );
            match (loaded, message) {
                (Ok(program), None) => {
                    // The base applies beside a dropped `null`.
                    let expected = name == "extends-array-null";
                    assert_eq!(strict(&program), expected, "{name} {dialect:?}");
                }
                (Err(error), Some(message)) => {
                    assert_eq!(error.kind, InputErrorKind::Config, "{name} {dialect:?}");
                    assert_eq!(error.message, message, "{name} {dialect:?}");
                    assert!(error.position.is_some(), "{name} {dialect:?}");
                }
                (loaded, _) => panic!("{name} {dialect:?}: {:?}", loaded.err()),
            }
        }
    }
}

/// Loads a leaf under `dialect` that extends `base`, in the parent
/// directory, with the given sources.
fn extending(dialect: Dialect, base: &str, sources: &[(&str, &str)]) -> (Scratch, Program) {
    let config = format!(r#"{{"extends":"../base.json","compilerOptions":{{{BASE}}}}}"#);
    let mut files = vec![("leaf/tsconfig.json", config.as_str()), ("base.json", base)];
    files.extend_from_slice(sources);
    loaded(dialect, &files)
}

/// `${configDir}` at the start of a template-capable value, in any case,
/// names a path under the selected configuration once the value's first
/// exact `${configDir}` becomes `./` (`getSubstitutedPathWithConfigDirTemplate`),
/// so `${CONFIGDIR}` is detected but not replaced: `${CONFIGDIR}/src` is
/// `leaf/${CONFIGDIR}/src`, and a later exact template is the one replaced.
/// Experiment template-origins and the scratch runs tmpl-upper-rootdir,
/// tmpl-upper-files, tmpl-second-occurrence, tmpl-jsx-import-source;
/// native 7.0.2 and 5.6.3 agree on each.
#[test]
fn config_dir_templates_are_detected_in_any_case_and_replaced_exactly() {
    let base = r#"{"compilerOptions":{"rootDir":"${CONFIGDIR}/src","declarationDir":"${CONFIGDIR}","rootDirs":["${CONFIGDIR}/src","${CONFIGDIR}/a/${configDir}/b"],"paths":{"a":["${CONFIGDIR}/x"]}},"files":["${CONFIGDIR}/main.ts"]}"#;
    for dialect in DIALECTS {
        let (scratch, program) = extending(
            dialect,
            base,
            &[
                ("leaf/main.ts", "export {};\n"),
                ("leaf/${CONFIGDIR}/main.ts", "export {};\n"),
            ],
        );
        let options = &program.options;
        let leaf = |path: &str| scratch.path(&format!("leaf/{path}"));
        assert_eq!(
            options.root_dir,
            Some(leaf("${CONFIGDIR}/src")),
            "{dialect:?}"
        );
        assert_eq!(
            options.declaration_dir,
            Some(leaf("${CONFIGDIR}")),
            "{dialect:?}"
        );
        assert_eq!(
            options.root_dirs,
            [leaf("${CONFIGDIR}/src"), leaf("${CONFIGDIR}/a/b")],
            "{dialect:?}"
        );
        assert_eq!(
            patterns(&program),
            Some(vec![("a".into(), vec![leaf("${CONFIGDIR}/x")])]),
            "{dialect:?}"
        );
        assert_eq!(searched(&program), ["${CONFIGDIR}/main.ts"], "{dialect:?}");
    }
}

/// The template's replacement is `./`, so `${configDir}suffix` names
/// `leaf/suffix`, not `leaf/.suffix`; a backslash after it is a separator,
/// the template alone names the directory, and a value with the template
/// later is relative to the file that declared it. Experiments
/// template-origins, template-without-slash, template-backslash,
/// template-middle, and the scratch run tmpl-suffix-include; native 7.0.2
/// and 5.6.3 agree on each.
#[test]
fn config_dir_templates_become_the_configuration_directory_and_a_slash() {
    let base = r#"{"compilerOptions":{"rootDir":"${configDir}/src","outDir":"${configDir}out","rootDirs":["${configDir}suffix","${configDir}\\src","x/${configDir}/src"],"typeRoots":["${configDir}types"],"paths":{"b":["${configDir}y"],"c":["${configDir}"]}},"files":["${configDir}/main.ts"],"include":["${configDir}src"]}"#;
    for dialect in DIALECTS {
        let (scratch, program) = extending(
            dialect,
            base,
            &[
                ("leaf/main.ts", "export {};\n"),
                ("leaf/src/a.ts", "export {};\n"),
                ("leaf/.src/b.ts", "export {};\n"),
            ],
        );
        let options = &program.options;
        let leaf = |path: &str| scratch.path(&format!("leaf/{path}"));
        assert_eq!(options.root_dir, Some(leaf("src")), "{dialect:?}");
        assert_eq!(options.out_dir, Some(leaf("out")), "{dialect:?}");
        assert_eq!(
            options.root_dirs,
            [
                leaf("suffix"),
                leaf("src"),
                scratch.path("x/${configDir}/src")
            ],
            "{dialect:?}"
        );
        assert_eq!(options.type_roots, Some(vec![leaf("types")]), "{dialect:?}");
        assert_eq!(
            patterns(&program),
            Some(vec![
                ("b".into(), vec![leaf("y")]),
                ("c".into(), vec![scratch.path("leaf")]),
            ]),
            "{dialect:?}"
        );
        assert_eq!(searched(&program), ["main.ts", "src/a.ts"], "{dialect:?}");
    }
}

/// Each compiler folds the template's case its own way:
/// 5.6.3's `startsWith(value, "${configDir}", true)` uppercases the first
/// 12 UTF-16 code units with JavaScript's `toUpperCase`, so a dotless `ı`
/// (U+0131) matches `I` and the ligature `ﬁ` (U+FB01) matches `FI` in a
/// value no longer than the template; 7.0.2's lowers the value with Go's
/// `strings.ToLower`, so a dotted `İ` (U+0130) matches `i`. A value that
/// matches is replaced exactly, so not at all here, and named under the
/// selected configuration; one that does not stays relative to the
/// declaring file. Scratch runs tmpl-dotless-i and tmpl-dotted-I.
#[test]
fn config_dir_templates_fold_case_as_each_compiler_does() {
    let config =
        format!(r#"{{"extends":"../base.json","compilerOptions":{{{BASE}}},"files":["a.ts"]}}"#);
    for (value, five, seven) in [
        ("${conf\u{131}gDir}/src", true, false),
        ("${conf\u{130}gDir}/src", false, true),
        ("${con\u{fb01}gDir}", true, false),
        ("${con\u{fb01}gDir}/src", false, false),
        ("${configDir}", true, true),
    ] {
        let base = format!(r#"{{"compilerOptions":{{"rootDir":"{value}"}}}}"#);
        for (dialect, detected) in [(TypeScript56, five), (TypeScript7, seven)] {
            let (scratch, program) = loaded(
                dialect,
                &[
                    ("leaf/tsconfig.json", &config),
                    ("base.json", &base),
                    SOURCE,
                ],
            );
            let expected = match detected {
                true if value == "${configDir}" => scratch.path("leaf"),
                true => scratch.path(&format!("leaf/{value}")),
                false => scratch.path(value),
            };
            assert_eq!(
                program.options.root_dir,
                Some(expected),
                "{value} {dialect:?}"
            );
        }
    }
}

/// `baseUrl` under 5.6.3 is a path option: relative to the configuration
/// that declared it, `${configDir}`-capable, inherited, and unset by an
/// own `null`. The `paths` base is `baseUrl` when it is set
/// (`getPathsBasePath`), so a relative substitution, the leaf's own
/// included, is relative to it, and a non-relative one is allowed; without
/// it, the base is the declaring file's directory and a non-relative
/// substitution is TS5090. TypeScript 7 still refuses `baseUrl`. Experiment
/// baseUrl-paths and the scratch runs baseurl-inherited,
/// baseurl-own-paths-relative, paths-nonrelative-no-baseurl,
/// baseurl-null-own, baseurl-template.
#[test]
fn base_url_sets_the_paths_base_under_5_6() {
    let sources = [
        ("leaf/main.ts", "import { x } from '@/x';\nexport { x };\n"),
        ("src/x.ts", "export const x = 1;\n"),
        ("leaf/src/x.ts", "export const x = 2;\n"),
    ];
    let config = |own: &str| {
        format!(
            r#"{{"extends":"../base.json","compilerOptions":{{{BASE}{own}}},"files":["main.ts"]}}"#
        )
    };
    for (name, base, own) in [
        (
            "baseurl-inherited",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
            "",
        ),
        (
            "baseurl-own-paths-relative",
            r#"{"compilerOptions":{"baseUrl":"."}}"#,
            r#","paths":{"@/*":["./src/*"]}"#,
        ),
    ] {
        let config = config(own);
        let mut files = vec![("leaf/tsconfig.json", config.as_str()), ("base.json", base)];
        files.extend(sources);
        let (scratch, program) = loaded(TypeScript56, &files);
        assert_eq!(program.options.base_url, Some(scratch.path("")), "{name}");
        assert_eq!(
            program
                .options
                .paths
                .as_ref()
                .map(|paths| paths.base.clone()),
            Some(scratch.path("")),
            "{name}"
        );
        assert_eq!(searched(&program), ["../src/x.ts", "main.ts"], "{name}");
        let (_scratch, loaded) = load(TypeScript7, &files);
        let error = loaded.expect_err("TypeScript 7 loads baseUrl");
        assert_eq!(error.kind, InputErrorKind::Unsupported, "{name}");
        assert!(
            error.message.starts_with("Option 'baseUrl' was removed"),
            "{error}"
        );
    }

    // Relative to its declaring file, empty naming that file's directory,
    // or under the selected configuration through `${configDir}`.
    for (value, expected) in [("./lib", "lib"), ("", ""), ("${configDir}/lib", "leaf/lib")] {
        let base = format!(r#"{{"compilerOptions":{{"baseUrl":"{value}"}}}}"#);
        let config = config("");
        let (scratch, program) = loaded(
            TypeScript56,
            &[
                ("leaf/tsconfig.json", &config),
                ("base.json", &base),
                sources[0],
                sources[1],
            ],
        );
        let options = &program.options;
        assert_eq!(options.base_url, Some(scratch.path(expected)), "{value}");
        assert!(options.paths.is_none());
    }

    // Without `baseUrl`, or with an own `null` over an inherited one, a
    // non-relative substitution is TS5090; the `paths` base is the
    // declaring file's directory.
    for (name, base) in [
        ("paths-nonrelative-no-baseurl", "{}"),
        ("baseurl-null-own", r#"{"compilerOptions":{"baseUrl":"."}}"#),
    ] {
        let own = match name {
            "baseurl-null-own" => r#","baseUrl":null,"paths":{"@/*":["src/*"]}"#,
            _ => r#","paths":{"@/*":["src/*"]}"#,
        };
        let config = config(own);
        let (_scratch, loaded) = load(
            TypeScript56,
            &[
                ("leaf/tsconfig.json", &config),
                ("base.json", base),
                sources[0],
                sources[1],
            ],
        );
        let error = loaded.expect_err("loads");
        assert_eq!(error.kind, InputErrorKind::Config, "{name}");
        assert_eq!(
            error.message,
            "Non-relative paths are not allowed when 'baseUrl' is not set. Did you forget a leading './'?",
            "{name}"
        );
        assert!(located(&config, &error).starts_with(r#""src/*""#), "{name}");
    }
    let config = config(r#","paths":{"@/*":["./src/*"]}"#);
    let (scratch, program) = loaded(
        TypeScript56,
        &[
            ("leaf/tsconfig.json", &config),
            ("base.json", "{}"),
            sources[0],
            sources[2],
        ],
    );
    assert_eq!(program.options.base_url, None);
    assert_eq!(
        program
            .options
            .paths
            .as_ref()
            .map(|paths| paths.base.clone()),
        Some(scratch.path("leaf"))
    );
    assert_eq!(searched(&program), ["main.ts", "src/x.ts"]);
}

/// A non-relative `paths` substitution without `baseUrl` is TS5090 at the
/// substitution, which each compiler words its own way. Scratch run
/// ts5090-wording: native 7.0.2 and 5.6.3's `tsc` on the same leaf.
#[test]
fn a_non_relative_substitution_is_ts5090_in_each_compilers_words() {
    let config = leaf(r#","paths":{"@/*":["src/*"]}"#, "");
    for (dialect, message) in [
        (
            TypeScript7,
            "Non-relative paths are not allowed. Did you forget a leading './'?",
        ),
        (
            TypeScript56,
            "Non-relative paths are not allowed when 'baseUrl' is not set. Did you forget a leading './'?",
        ),
    ] {
        let (_scratch, loaded) = load(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
        let error = loaded.expect_err("loads");
        assert_eq!(error.kind, InputErrorKind::Config, "{dialect:?}");
        assert_eq!(error.message, message, "{dialect:?}");
        assert!(
            located(&config, &error).starts_with(r#""src/*""#),
            "{dialect:?}"
        );
    }
}

/// 5.6.3's `verifyCompilerOptions` checks `paths` after the options 5.5
/// removed and before `lib` with `noLib` (TS5053) and TS5091, so of
/// several errors this tool, which reports one, reports the earliest
/// check's.
#[test]
fn paths_are_verified_in_typescript_5_6s_order() {
    let paths = r#","paths":{"@/*":["src/*"]}"#;
    for (options, message) in [
        (
            r#","charset":"utf8""#,
            "Option 'charset' has been removed. Please remove it from your configuration",
        ),
        (
            r#","isolatedModules":true,"preserveConstEnums":false"#,
            "Non-relative paths are not allowed when 'baseUrl' is not set. Did you forget a leading './'?",
        ),
        (
            r#","lib":["es2020"]"#,
            "Non-relative paths are not allowed when 'baseUrl' is not set. Did you forget a leading './'?",
        ),
    ] {
        let config = leaf(&format!("{paths}{options}"), "");
        let (_scratch, loaded) = load(TypeScript56, &[("leaf/tsconfig.json", &config), SOURCE]);
        let error = loaded.expect_err("loads");
        assert_eq!(error.message, message, "{options}");
    }
}

/// Program inputs' display paths, relative to the configuration, sorted.
fn inputs(program: &Program) -> Vec<&str> {
    let mut inputs: Vec<&str> = program
        .sources
        .ids()
        .map(|id| program.sources.get(id).display.as_str())
        .collect();
    inputs.sort_unstable();
    inputs
}

/// Type packages in two type roots, as on real Artemis: `alpha` and `beta`
/// beside the configuration, `alpha`, `beta`, and `gamma` in the
/// ancestor's.
fn type_roots() -> Vec<(String, String)> {
    let mut files = Vec::new();
    for (root, names) in [
        ("leaf/node_modules/@types", &["alpha", "beta"][..]),
        ("node_modules/@types", &["alpha", "beta", "gamma"][..]),
    ] {
        for name in names {
            files.push((
                format!("{root}/{name}/index.d.ts"),
                format!("declare const {name}: number;\n"),
            ));
            files.push((
                format!("{root}/{name}/package.json"),
                format!(r#"{{"name":"@types/{name}","version":"1.0.0"}}"#),
            ));
        }
    }
    files
}

const NEARER: [&str; 3] = [
    "../node_modules/@types/gamma/index.d.ts",
    "node_modules/@types/alpha/index.d.ts",
    "node_modules/@types/beta/index.d.ts",
];

/// Load a leaf whose options are `BASE` without `types`, then `options`,
/// beside `type_roots`.
fn with_type_roots(
    dialect: Dialect,
    options: &str,
    extra: &[(&str, &str)],
) -> (Scratch, Result<Program, InputError>) {
    let config = leaf(options, "").replace(r#""types":[],"#, "");
    let roots = type_roots();
    let mut files: Vec<(&str, &str)> = roots
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    files.push(("leaf/tsconfig.json", &config));
    files.push(SOURCE);
    files.extend_from_slice(extra);
    load(dialect, &files)
}

/// Which type packages a program includes. Under 5.6.3 an omitted `types`
/// includes every package in the type roots, nearest root first, a name in
/// two roots resolving to the nearer (`getAutomaticTypeDirectiveNames`,
/// `resolveTypeReferenceDirective`); `[]` includes none; and `*` is a
/// package's name, TS2688 when no such package exists. Under 7.0.2 an
/// omitted `types` includes none and `*` stands for every package in the
/// type roots. A `null` over a list leaves `types` unset, so 5.6.3 then
/// includes every package. Scratch runs types-omitted-two-roots,
/// types-star, types-empty, types-star-pkg; experiments duplicate-types-null,
/// inherit-types-null, duplicate-objects-types, empty-own-types,
/// empty-inherited-types, empty-own-types-null-elements,
/// empty-inherited-types-null-elements.
#[test]
fn types_select_packages_by_each_dialects_rule() {
    let all: Vec<&str> = NEARER.iter().copied().chain(["a.ts"]).collect();
    let mut all = all;
    all.sort_unstable();
    for (options, five, seven) in [
        ("", all.clone(), vec!["a.ts"]),
        (r#","types":[]"#, vec!["a.ts"], vec!["a.ts"]),
        (
            r#","types":["alpha"],"types":[null,""]"#,
            vec!["a.ts"],
            vec!["a.ts"],
        ),
        (
            r#","types":["alpha"],"types":null"#,
            all.clone(),
            vec!["a.ts"],
        ),
        (
            r#","types":["alpha"],"types":[]"#,
            vec!["a.ts"],
            vec!["a.ts"],
        ),
        (
            r#","types":["alpha"]"#,
            vec!["a.ts", NEARER[1]],
            vec!["a.ts", NEARER[1]],
        ),
        (r#","types":["*"]"#, vec![], all.clone()),
    ] {
        for (dialect, expected) in [(TypeScript56, &five), (TypeScript7, &seven)] {
            let (_scratch, loaded) = with_type_roots(dialect, options, &[]);
            match loaded {
                Ok(program) => assert_eq!(&inputs(&program), expected, "{options} {dialect:?}"),
                Err(error) => {
                    assert!(expected.is_empty(), "{options} {dialect:?}: {error}");
                    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
                    assert_eq!(error.message, "Cannot find type definition file for '*'");
                }
            }
        }
    }
    // A package named `*`: 5.6.3 includes it alone; 7.0.2's `*` includes
    // it among every other.
    let star = [
        (
            "leaf/node_modules/@types/*/index.d.ts",
            "declare const star: 1;\n",
        ),
        (
            "leaf/node_modules/@types/*/package.json",
            r#"{"name":"@types/*","version":"1.0.0"}"#,
        ),
    ];
    for (dialect, expected) in [
        (
            TypeScript56,
            vec!["a.ts", "node_modules/@types/*/index.d.ts"],
        ),
        (
            TypeScript7,
            vec![
                NEARER[0],
                "a.ts",
                "node_modules/@types/*/index.d.ts",
                NEARER[1],
                NEARER[2],
            ],
        ),
    ] {
        let (_scratch, loaded) = with_type_roots(dialect, r#","types":["*"]"#, &star);
        assert_eq!(inputs(&loaded.unwrap()), expected, "{dialect:?}");
    }
    // Inherited lists, an own `null` over one, and a `null` in a later
    // `compilerOptions` object.
    let base = r#"{"compilerOptions":{"types":["alpha"]}}"#;
    let objects = format!(
        r#"{{"compilerOptions":{{{BASE},"types":["alpha"]}},"compilerOptions":{{"types":null}},"files":["a.ts"]}}"#
    )
    .replace(r#""types":[],"#, "");
    for (own, five, seven) in [
        (r#","types":null"#, all.clone(), vec!["a.ts"]),
        (r#","types":[]"#, vec!["a.ts"], vec!["a.ts"]),
        (r#","types":[null,""]"#, vec!["a.ts"], vec!["a.ts"]),
        ("", vec!["a.ts", NEARER[1]], vec!["a.ts", NEARER[1]]),
        ("objects", all.clone(), vec!["a.ts"]),
    ] {
        let config = match own {
            "objects" => objects.clone(),
            _ => leaf(own, r#","extends":"../base.json""#).replace(r#""types":[],"#, ""),
        };
        for (dialect, expected) in [(TypeScript56, &five), (TypeScript7, &seven)] {
            let roots = type_roots();
            let mut files: Vec<(&str, &str)> = roots
                .iter()
                .map(|(a, b)| (a.as_str(), b.as_str()))
                .collect();
            files.extend([
                ("leaf/tsconfig.json", config.as_str()),
                ("base.json", base),
                SOURCE,
            ]);
            let (_scratch, loaded) = load(dialect, &files);
            assert_eq!(
                &inputs(&loaded.unwrap()),
                expected,
                "inherited {own} {dialect:?}"
            );
        }
    }
}

/// A copy of the checked-in fixture `directory` under a scratch root, as
/// loading it in place would read the repository's ancestors.
fn copy_fixture(directory: &str) -> Scratch {
    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "tsparse-fixture-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    copy(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(directory),
        &root,
    );
    Scratch { root }
}

/// A synthetic configuration with the option and pattern shapes of
/// Artemis's (a `baseUrl` of `.`, non-relative `paths` of each form, a
/// nested `ts-node` section), beside a TypeScript 5.6.3 manifest and stubs
/// for its literal `files` entry and one `include` match, loads under the
/// 5.6 dialect with the effective options TypeScript 5.6.3 gives it
/// (`tsc --showConfig --incremental false` and
/// `getParsedCommandLineOfConfigFile` on a scratch copy, which report no
/// error): `baseUrl` kept, its non-relative `paths` substitutions accepted
/// relative to it (`getPathsBasePath` is `baseUrl`), `types` unset and so
/// every type-root package, `ts-node.compilerOptions` inert, and the
/// strict family, interop, and `preserveConstEnums` as computed.
#[test]
fn a_base_url_project_reads_as_typescript_5_6_3_reads_it() {
    use super::super::ModuleKind;
    use super::{Detection, Resolution, Target, Types};
    let scratch = copy_fixture("fixtures/configuration/base-url-project");
    let program = scratch
        .load("tsconfig.json")
        .unwrap_or_else(|error| panic!("{error}"));
    let options = &program.options;
    assert_eq!(
        options.compiler.dialect().config(),
        super::Rules::TYPESCRIPT_5_6
    );
    let dir = scratch.path("");
    assert_eq!(options.base_url.as_deref(), Some(dir.as_str()));
    let paths = options.paths.as_ref().expect("paths");
    assert_eq!(paths.base, dir);
    let pattern =
        |pattern: &str, substitution: &str| (pattern.to_owned(), vec![substitution.to_owned()]);
    assert_eq!(
        paths.patterns,
        [
            pattern("@/*", "src/*"),
            pattern("@checks-client/*", "checks/client/*"),
            pattern("@checks-host/*", "checks/host/*"),
            pattern("@checks-common/*", "checks/common/*"),
            pattern("third*", "third/*"),
            pattern("map-kit", "./node_modules/map-kit/dist/map-kit.d.ts"),
        ]
    );
    assert_eq!(options.types, Types::Automatic);
    assert_eq!(options.type_roots, None);
    assert!(options.root_dirs.is_empty());
    // `target: es2017`, `module: esnext`, `moduleResolution: bundler`.
    assert_eq!(options.target, Target::Es2017);
    assert_eq!(options.resolution, Resolution::Bundler);
    assert_eq!(options.detection, Detection::Auto);
    // `jsx: react-jsx`, no `jsxImportSource`.
    assert!(options.jsx && options.jsx_automatic && !options.jsx_dev && !options.jsx_preserve);
    assert_eq!(options.jsx_import_source, None);
    assert_eq!(
        options.lib.as_deref(),
        Some(&["lib.dom.d.ts", "lib.dom.iterable.d.ts", "lib.esnext.d.ts"][..])
    );
    assert!(!options.no_lib && !options.lib_replacement);
    // `allowJs`, `resolveJsonModule`, `skipLibCheck`, and the computed
    // `resolvePackageJsonExports`/`Imports`, `preserveConstEnums`.
    assert!(options.allow_js && options.resolve_json_module && options.skip_lib_check);
    assert!(options.package_exports && options.package_imports);
    assert!(options.preserve_const_enums);
    assert!(!options.check_side_effect_imports && !options.preserve_symlinks);
    assert!(options.custom_conditions.is_empty() && options.module_suffixes.is_empty());
    assert_eq!(
        (
            &options.out_dir,
            &options.root_dir,
            &options.declaration_dir
        ),
        (&None, &None, &None)
    );
    let semantic = &options.semantic;
    assert_eq!(semantic.module_kind, ModuleKind::EsNext);
    assert_eq!(semantic.module_resolution, Resolution::Bundler);
    // `strict` and the strict family it computes, `noImplicitAny` set.
    assert!(strict(&program));
    assert!(semantic.no_implicit_this && semantic.strict_bind_call_apply);
    assert!(semantic.strict_property_initialization && semantic.strict_builtin_iterator_return);
    assert!(semantic.allow_js && semantic.check_js.is_none());
    assert!(semantic.isolated_modules && semantic.preserve_const_enums);
    assert!(semantic.import_helpers && !semantic.verbatim_module_syntax);
    assert!(!semantic.no_error_truncation && !semantic.no_implicit_override);
    // `useDefineForClassFields` is computed from the target, below ES2022.
    assert!(!semantic.use_define_for_class_fields);
    assert!(semantic.explicit_dom_library());
    assert_eq!(searched(&program), ["src/index.ts", "types/globals.d.ts"]);
}

/// Path settings an inherited configuration declares are relative to it,
/// an empty `rootDirs` entry naming its directory, while `jsxImportSource`
/// stays text, and its `include` names files relative to it too; the
/// inherited `outDir` is a default `exclude`. Both dialects. Experiment
/// paths-origins.
#[test]
fn inherited_paths_stay_relative_to_their_configuration() {
    let base = r#"{"compilerOptions":{"rootDir":"src","outDir":"dist","rootDirs":["src",""],"typeRoots":["types"],"paths":{"alias":["./src"]},"jsxImportSource":"./jsx"},"include":["base.ts","dist/*.ts"]}"#;
    let config = format!(r#"{{"extends":"../base.json","compilerOptions":{{{BASE}}}}}"#);
    for dialect in DIALECTS {
        let (scratch, program) = loaded(
            dialect,
            &[
                ("leaf/tsconfig.json", &config),
                ("base.json", base),
                ("base.ts", "export {};\n"),
                ("dist/out.ts", "export {};\n"),
            ],
        );
        let options = &program.options;
        assert_eq!(options.root_dir, Some(scratch.path("src")), "{dialect:?}");
        assert_eq!(options.out_dir, Some(scratch.path("dist")), "{dialect:?}");
        assert_eq!(options.root_dirs, [scratch.path("src"), scratch.path("")]);
        assert_eq!(options.type_roots, Some(vec![scratch.path("types")]));
        let paths = options.paths.as_ref().unwrap();
        assert_eq!(paths.base, scratch.path(""));
        assert_eq!(
            paths.patterns,
            [("alias".to_owned(), vec!["./src".to_owned()])]
        );
        assert_eq!(options.jsx_import_source.as_deref(), Some("./jsx"));
        assert_eq!(searched(&program), ["../base.ts"], "{dialect:?}");
    }
}

/// An `include` spec that ends in `**` (TS5010) or has `..` after `**`
/// (TS5065) is an error in both dialects. Experiments
/// root-recursive-include, root-parent-wildcard.
#[test]
fn wildcard_specs_are_validated() {
    for (spec, message) in [
        (
            "a/**",
            "File specification cannot end in a recursive directory wildcard ('**'): 'a/**'",
        ),
        (
            "a/**/../b",
            "File specification cannot contain a parent directory ('..') that appears after a recursive directory wildcard ('**'): 'a/**/../b'",
        ),
    ] {
        let config = format!(r#"{{"compilerOptions":{{{BASE}}},"include":["{spec}"]}}"#);
        for dialect in DIALECTS {
            let (_scratch, loaded) = load(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
            let error = loaded.expect_err("loads");
            assert_eq!(error.kind, InputErrorKind::Config, "{dialect:?}");
            assert_eq!(error.message, message, "{dialect:?}");
            assert!(located(&config, &error).starts_with(&format!("{spec:?}")));
        }
    }
}

/// The entries of `include` and `exclude` are converted and checked one by
/// one, in written order, so the first that fails decides: a wildcard
/// spec's TS5010 or TS5065 before an entry with an unpaired UTF-16
/// surrogate, which this tool refuses where it reaches it, and that refusal
/// before a later wildcard spec. Scratch run spec-order: 5.6.3's `tsc` and
/// native 7.0.2, which read the surrogate as text, report the wildcard
/// spec in either order.
#[test]
fn root_list_entries_are_checked_in_written_order() {
    const SURROGATE: &str = r#""\ud800""#;
    for (key, spec, message) in [
        (
            "include",
            "a/**",
            "File specification cannot end in a recursive directory wildcard ('**'): 'a/**'",
        ),
        (
            "include",
            "a/**/../b",
            "File specification cannot contain a parent directory ('..') that appears after a recursive directory wildcard ('**'): 'a/**/../b'",
        ),
        (
            "exclude",
            "a/**/../b",
            "File specification cannot contain a parent directory ('..') that appears after a recursive directory wildcard ('**'): 'a/**/../b'",
        ),
    ] {
        let spec = format!("{spec:?}");
        for (entries, kind, message, at) in [
            (
                format!("{spec},{SURROGATE}"),
                InputErrorKind::Config,
                message,
                spec.as_str(),
            ),
            (
                format!("{SURROGATE},{spec}"),
                InputErrorKind::Unsupported,
                "This string has an unpaired UTF-16 surrogate, which cannot name a file, module, or option value",
                SURROGATE,
            ),
        ] {
            let config =
                leaf("", &format!(r#","{key}":[{entries}]"#)).replace(r#""files":["a.ts"],"#, "");
            let config = match key {
                "exclude" => config.replace(r#","exclude""#, r#","include":["a.ts"],"exclude""#),
                _ => config,
            };
            for dialect in DIALECTS {
                let (_scratch, loaded) = load(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
                let error = loaded.expect_err("loads");
                assert_eq!(error.kind, kind, "{config} {dialect:?}");
                assert_eq!(error.message, message, "{config} {dialect:?}");
                assert!(
                    located(&config, &error).starts_with(at),
                    "{config} {dialect:?}: {error}"
                );
            }
        }
    }
}

/// Enumeration keys and option names fold case as each compiler does:
/// 5.6.3's JavaScript lowercasing turns `İ` into two characters, so
/// `react-natİve` is TS6046, which 7.0.2's one-for-one lowering accepts;
/// `Strict` is unknown in both, 5.6.3 suggesting `strict` (7.0.2's TS5025
/// keeps this tool's TS5023 wording, docs/references.md). Experiments
/// unicode-enum-lowercase, unknown-case-key.
#[test]
fn names_fold_case_as_each_compiler_does() {
    let config = leaf(r#","jsx":"react-nat\u0130ve""#, "");
    let (_scratch, program) = loaded(TypeScript7, &[("leaf/tsconfig.json", &config), SOURCE]);
    assert!(program.options.jsx && !program.options.jsx_automatic);
    let (_scratch, loaded) = load(TypeScript56, &[("leaf/tsconfig.json", &config), SOURCE]);
    assert_eq!(
        loaded.err().map(|error| error.message),
        Some("Argument for '--jsx' option must be: 'preserve', 'react-native', 'react', 'react-jsx', 'react-jsxdev'".into())
    );
    let config = leaf(r#","Strict":true"#, "");
    for (dialect, message) in [
        (TypeScript7, "Unknown compiler option 'Strict'"),
        (
            TypeScript56,
            "Unknown compiler option 'Strict'. Did you mean 'strict'?",
        ),
    ] {
        let (_scratch, loaded) = load(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
        assert_eq!(
            loaded.err().map(|error| error.message).as_deref(),
            Some(message)
        );
    }
}

/// Each `defaults-*` experiment's options, with `noLib` and `types: []`
/// and, where the case says, the settings the TypeScript 5.6 profile's
/// refusals need: 5.6.3's computed defaults (`computedOptions`) or the
/// profile's refusal of them, and 7.0.2's. TypeScript 7's TS5091 and
/// TS5052 are among the checks this tool does not port in that profile
/// (docs/references.md), so defaults-8 and defaults-12 load there.
/// Experiments defaults-0 to defaults-12.
#[test]
fn computed_defaults_follow_each_compiler() {
    use super::super::ModuleKind;
    use super::{Detection, Resolution, Target};
    const STRICT: &str = r#","strict":true"#;
    const MODERN: &str = r#","strict":true,"target":"es2017","module":"esnext","moduleResolution":"bundler","esModuleInterop":true"#;
    let config = |options: &str| {
        format!(r#"{{"compilerOptions":{{"noLib":true,"types":[]{options}}},"files":["a.ts"]}}"#)
    };
    let load_with = |dialect: Dialect, options: &str| {
        let config = config(options);
        load(dialect, &[("leaf/tsconfig.json", &config), SOURCE]).1
    };
    let refused = |options: &str, kind: InputErrorKind, start: &str| {
        let error = load_with(TypeScript56, options).expect_err("loads");
        assert_eq!(error.kind, kind, "{options}: {error}");
        assert!(error.message.starts_with(start), "{options}: {error}");
    };
    let five = |options: &str| load_with(TypeScript56, options).unwrap_or_else(|e| panic!("{e}"));
    let seven = |options: &str| load_with(TypeScript7, options).unwrap_or_else(|e| panic!("{e}"));

    // defaults-0: an unset target is ES5 under 5.6.3, which the profile
    // refuses.
    refused(
        "",
        InputErrorKind::Unsupported,
        "Option 'target' is 'es5' by TypeScript 5's default",
    );
    assert!(strict(&seven("")));
    // defaults-1 and defaults-2: `node16` targets ES2022, `nodenext` ESNext,
    // each forcing module detection with define semantics; JSON modules
    // follow 5.6.3's bundler-only default and 7.0.2's `nodenext` one.
    for (module, target, resolution, json) in [
        ("node16", Target::Es2022, Resolution::Node16, false),
        ("nodenext", Target::EsNext, Resolution::NodeNext, true),
    ] {
        let options = format!(r#","module":"{module}"{STRICT}"#);
        for (dialect, program) in [
            (TypeScript56, five(&options)),
            (TypeScript7, seven(&options)),
        ] {
            let options = &program.options;
            assert_eq!(options.resolution, resolution, "{module} {dialect:?}");
            assert_eq!(options.detection, Detection::Force, "{module} {dialect:?}");
            if dialect == TypeScript56 {
                assert_eq!(options.target, target, "{module}");
                assert!(options.semantic.use_define_for_class_fields, "{module}");
                assert!(!options.resolve_json_module, "{module}");
            } else {
                assert_eq!(options.resolve_json_module, json, "{module}");
            }
        }
    }
    // defaults-3: `preserve` resolves as `bundler`, with JSON modules under
    // 5.6.3; its unset target is ES5 there.
    refused(
        r#","module":"preserve""#,
        InputErrorKind::Unsupported,
        "Option 'target' is 'es5'",
    );
    let preserve = r#","module":"preserve","target":"es2017","strict":true"#;
    for (dialect, program) in [
        (TypeScript56, five(preserve)),
        (TypeScript7, seven(preserve)),
    ] {
        assert_eq!(
            program.options.resolution,
            Resolution::Bundler,
            "{dialect:?}"
        );
        assert_eq!(program.options.semantic.module_kind, ModuleKind::Preserve);
        assert!(program.options.resolve_json_module, "{dialect:?}");
    }
    // defaults-4: from `target` ES2017 alone, 5.6.3's module is ES2015 and
    // its resolution `classic`, which the profile refuses.
    refused(
        r#","target":"es2017","strict":true"#,
        InputErrorKind::Unsupported,
        "Option 'moduleResolution' is 'classic' by TypeScript 5's default",
    );
    assert_eq!(
        seven(r#","target":"es2017""#).options.target,
        Target::Es2017
    );
    // defaults-5: `bundler` with CommonJS is TS5095 under 5.6.3, accepted
    // by 7.0.2.
    let commonjs = r#","module":"commonjs","moduleResolution":"bundler""#;
    refused(
        commonjs,
        InputErrorKind::Config,
        "Option 'bundler' can only be used when 'module' is set to 'preserve' or to 'es2015' or later",
    );
    let program = seven(commonjs);
    assert_eq!(program.options.semantic.module_kind, ModuleKind::CommonJs);
    assert_eq!(program.options.resolution, Resolution::Bundler);
    // defaults-6: `system` is refused by the 5.6 profile and removed in 7.
    refused(
        r#","module":"system","target":"es2017","strict":true"#,
        InputErrorKind::Unsupported,
        "Option 'module=system'",
    );
    let error = load_with(TypeScript7, r#","module":"system""#).expect_err("loads");
    assert_eq!(
        error.message,
        "Option 'module=system' was removed in TypeScript 7"
    );
    // defaults-7: `strict` turns on the whole family in both.
    assert!(strict(&five(MODERN)) && strict(&seven(STRICT)));
    // defaults-8: TS5091 under 5.6.3; this tool does not port it for 7.
    let verbatim =
        r#","verbatimModuleSyntax":true,"isolatedModules":false,"preserveConstEnums":false"#;
    refused(
        verbatim,
        InputErrorKind::Config,
        "Option 'preserveConstEnums' cannot be disabled when 'verbatimModuleSyntax' is enabled",
    );
    let program = seven(verbatim);
    assert!(program.options.semantic.isolated_modules && program.options.preserve_const_enums);
    // defaults-9 and defaults-10: without `module`, `bundler` is TS5095
    // under 5.6.3; resolution reads the raw exports and imports flags in
    // both, not 5.6.3's computed imports getter, which reads exports.
    for (flags, exports, imports) in [
        (
            r#","resolvePackageJsonExports":false,"resolvePackageJsonImports":true"#,
            false,
            true,
        ),
        (r#","resolvePackageJsonImports":false"#, true, false),
    ] {
        refused(
            &format!(r#","moduleResolution":"bundler"{flags}"#),
            InputErrorKind::Config,
            "Option 'bundler' can only be used when 'module' is set to 'preserve' or to 'es2015' or later",
        );
        let options = format!("{MODERN}{flags}");
        for (dialect, program) in [
            (TypeScript56, five(&options)),
            (TypeScript7, seven(&options)),
        ] {
            let options = &program.options;
            assert_eq!(
                (options.package_exports, options.package_imports),
                (exports, imports),
                "{flags} {dialect:?}"
            );
        }
    }
    // defaults-11: `checkJs` implies `allowJs` in both.
    let check = format!(r#"{MODERN},"checkJs":true"#);
    for (dialect, program) in [(TypeScript56, five(&check)), (TypeScript7, seven(&check))] {
        assert!(program.options.allow_js, "{dialect:?}");
        assert_eq!(program.options.semantic.check_js, Some(true));
    }
    // defaults-12: `checkJs` without `allowJs` is TS5052 in both compilers,
    // a check this tool does not port in either dialect.
    let check = format!(r#"{MODERN},"checkJs":true,"allowJs":false"#);
    for (dialect, program) in [(TypeScript56, five(&check)), (TypeScript7, seven(&check))] {
        assert!(!program.options.allow_js, "{dialect:?}");
    }
}

/// Only template-capable values take `${configDir}`: a `types` entry, a
/// module suffix, a condition, a `paths` pattern, and `jsxImportSource`
/// keep it as text in both dialects, so the `types` entry names a package
/// that does not exist (TS2688). Scratch run tmpl-not-capable, where
/// 5.6.3's parsed options and 7.0.2's `--showConfig` keep each as written.
#[test]
fn config_dir_templates_apply_only_to_template_capable_options() {
    let options = r#","moduleSuffixes":["${configDir}",""],"customConditions":["${configDir}"],"paths":{"${configDir}/*":["./src/*"]},"jsxImportSource":"${configDir}/jsx""#;
    let config = leaf(options, "");
    let missing = leaf(options, "").replace(r#""types":[]"#, r#""types":["${configDir}/t"]"#);
    for dialect in DIALECTS {
        let (_scratch, program) = loaded(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
        let options = &program.options;
        assert_eq!(options.module_suffixes, ["${configDir}", ""], "{dialect:?}");
        assert_eq!(options.custom_conditions, ["${configDir}"], "{dialect:?}");
        assert_eq!(
            options.jsx_import_source.as_deref(),
            Some("${configDir}/jsx")
        );
        assert_eq!(
            patterns(&program),
            Some(vec![("${configDir}/*".into(), vec!["./src/*".into()])])
        );
        let (_scratch, loaded) = load(dialect, &[("leaf/tsconfig.json", &missing), SOURCE]);
        let error = loaded.expect_err("loads");
        assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{dialect:?}");
        assert_eq!(
            error.message,
            "Cannot find type definition file for '${configDir}/t'"
        );
    }
}

/// Under the TypeScript 5.6 dialect each option's values convert as its
/// declaration in 5.6.3's catalog says (`normalizeNonListOptionValue`,
/// `convertJsonOptionOfListType`, `configDirTemplateSubstitutionOptions`),
/// here from a base in the parent directory: a path (`isFilePath`:
/// `outDir`, `rootDir`, `declarationDir`, `baseUrl`) is relative to the
/// configuration that declares it, an empty one names that directory, and
/// one that starts with `${configDir}` names a path under the selected
/// configuration; text (`jsxImportSource`, `jsxFactory`,
/// `jsxFragmentFactory`, `reactNamespace`) stays as written, the template
/// included; a list of paths (`rootDirs`, `typeRoots`) converts each entry
/// as a path and leaves out a `null`; a list of names (`customConditions`,
/// `types`) keeps its entries as written, without `null` and empty ones;
/// the one list that keeps falsy entries (`listPreserveFalsyValues`,
/// `moduleSuffixes`) keeps an empty one, and the `null` 5.6.3 keeps there is
/// refused; and `paths` substitutions take the template, its patterns not.
/// Scratch run conv56-classes, where 5.6.3's
/// `parseJsonSourceFileConfigFileContent` gives each value and its `tsc`
/// reports no configuration error.
#[test]
fn options_convert_as_their_typescript_5_6_declarations_say() {
    let leaf = format!(
        r#"{{"extends":"../base.json","compilerOptions":{{{}}},"files":["a.ts"]}}"#,
        BASE.replace(r#""types":[],"#, "")
    );
    let files = |base| {
        [
            ("leaf/tsconfig.json", leaf.as_str()),
            ("base.json", base),
            SOURCE,
        ]
    };
    let base = r#"{"compilerOptions":{"declaration":true,"outDir":"out","rootDir":"","declarationDir":"${configDir}/decl","baseUrl":"${configDir}","jsxImportSource":"${configDir}/jsx","jsxFactory":"h","jsxFragmentFactory":"Fragment","rootDirs":["x","",null,"${configDir}/r"],"typeRoots":["t","",null,"${configDir}/t"],"customConditions":["c","",null,"${configDir}"],"types":["",null],"moduleSuffixes":[".ios","","${configDir}"],"paths":{"${configDir}/*":["./src/*","${configDir}/src/*","src/*"]}}}"#;
    let (scratch, program) = loaded(TypeScript56, &files(base));
    let options = &program.options;
    let path = |relative: &str| scratch.path(relative);
    assert_eq!(options.out_dir, Some(path("out")));
    assert_eq!(options.root_dir, Some(path("")));
    assert_eq!(options.declaration_dir, Some(path("leaf/decl")));
    assert_eq!(options.base_url, Some(path("leaf")));
    assert_eq!(
        options.jsx_import_source.as_deref(),
        Some("${configDir}/jsx")
    );
    assert_eq!(options.semantic.jsx_factory.as_deref(), Some("h"));
    assert_eq!(
        options.semantic.jsx_fragment_factory.as_deref(),
        Some("Fragment")
    );
    assert_eq!(options.root_dirs, [path("x"), path(""), path("leaf/r")]);
    assert_eq!(
        options.type_roots,
        Some(vec![path("t"), path(""), path("leaf/t")])
    );
    assert_eq!(options.custom_conditions, ["c", "${configDir}"]);
    assert_eq!(options.types, super::Types::Listed(Vec::new()));
    assert_eq!(options.module_suffixes, [".ios", "", "${configDir}"]);
    assert_eq!(
        patterns(&program),
        Some(vec![(
            "${configDir}/*".into(),
            vec!["./src/*".into(), path("leaf/src/*"), "src/*".into()]
        )])
    );

    // 5.6.3 rejects `reactNamespace` beside `jsxFactory`, and a template
    // as its identifier (TS5053, TS5059).
    let (_scratch, program) = loaded(
        TypeScript56,
        &files(r#"{"compilerOptions":{"reactNamespace":"x"}}"#),
    );
    assert_eq!(
        program.options.semantic.react_namespace.as_deref(),
        Some("x")
    );

    let base = r#"{"compilerOptions":{"moduleSuffixes":[".ios",null,""]}}"#;
    let (_scratch, loaded) = load(TypeScript56, &files(base));
    let error = loaded.expect_err("loads");
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
    assert_eq!(
        error.message,
        "TypeScript 5 keeps a null 'moduleSuffixes' entry and spells it 'undefined' in file names, which is not supported"
    );
    assert!(located(base, &error).starts_with("null"), "{error}");
}

/// A `paths` substitution that starts with `${configDir}` is checked as
/// both compilers check it, after substitution and normalization
/// (`handleOptionConfigDirTemplateSubstitution`, `getNormalizedAbsolutePath`,
/// then `verifyCompilerOptions`): `${configDir}/discard*/../src/*` is
/// `leaf/src/*`, with one `*`, and resolves; `${configDir}/*` under a
/// directory named `a*b` has two, TS5062 naming the substituted path.
/// A TypeScript 7 fidelity fix as much as a 5.6 rule. Scratch runs
/// paths-tmpl-collapse, paths-tmpl-collapse-resolve, paths-tmpl-star-dir.
#[test]
fn paths_substitutions_are_checked_after_config_dir_substitution() {
    let config = leaf(r#","paths":{"@/*":["${configDir}/discard*/../src/*"]}"#, "")
        .replace(r#""files":["a.ts"]"#, r#""files":["main.ts"]"#);
    for dialect in DIALECTS {
        let (scratch, program) = loaded(
            dialect,
            &[
                ("leaf/tsconfig.json", &config),
                ("leaf/main.ts", "import { x } from '@/x';\nexport { x };\n"),
                ("leaf/src/x.ts", "export const x = 1;\n"),
            ],
        );
        assert_eq!(
            patterns(&program),
            Some(vec![("@/*".into(), vec![scratch.path("leaf/src/*")])]),
            "{dialect:?}"
        );
        assert_eq!(searched(&program), ["main.ts", "src/x.ts"], "{dialect:?}");
    }
    let config = leaf(r#","paths":{"a*":["${configDir}/*"]}"#, "");
    for dialect in DIALECTS {
        let scratch = Scratch::new(
            dialect,
            &[
                ("a*b/leaf/tsconfig.json", &config),
                ("a*b/leaf/a.ts", "export {};\n"),
            ],
        );
        let error = scratch
            .load("a*b/leaf/tsconfig.json")
            .expect_err("two stars load");
        assert_eq!(error.kind, InputErrorKind::Config, "{dialect:?}");
        assert_eq!(
            error.message,
            format!(
                "Substitution '{}' in pattern 'a*' can have at most one '*' character",
                scratch.path("a*b/leaf/*")
            ),
            "{dialect:?}"
        );
        assert!(located(&config, &error).starts_with(r#""${configDir}/*""#));
    }
}

/// `compilerOptions` given as an array: 5.6.3's `convertJsonOption` accepts
/// it, as an object is any `typeof` object, and its array conversion
/// notifies no option, so it sets nothing, converts none of its entries
/// (`"strict": "bad"` inside is no error), and earlier and inherited
/// options stand. 7.0.2 reports TS5024, as this tool does in its words. A
/// value of another type is TS5024 in both. Scratch runs copts-array-empty,
/// copts-array-objects, copts-array-dup.
#[test]
fn an_array_of_compiler_options_sets_nothing_under_5_6() {
    let base = format!(r#"{{"compilerOptions":{{{BASE},"strict":true}}}}"#);
    let empty = r#"{"extends":"../base.json","compilerOptions":[],"files":["a.ts"]}"#;
    let objects = r#"{"extends":"../base.json","compilerOptions":[{"strict":"bad","noLib":false}],"files":["a.ts"]}"#;
    let duplicate = format!(
        r#"{{"compilerOptions":{{{BASE},"strict":true}},"compilerOptions":[],"files":["a.ts"]}}"#
    );
    for config in [empty, objects, &duplicate] {
        let files = [("leaf/tsconfig.json", config), ("base.json", &base), SOURCE];
        let (_scratch, program) = loaded(TypeScript56, &files);
        assert!(strict(&program), "{config}");
        assert!(program.options.no_lib, "{config}");
        let (_scratch, loaded) = load(TypeScript7, &files);
        let error = loaded.expect_err("an array loads");
        assert_eq!(error.kind, InputErrorKind::Config);
        assert_eq!(error.message, "'compilerOptions' must be an object");
        assert!(located(config, &error).starts_with('['), "{config}");
    }
    let config = r#"{"compilerOptions":true,"files":["a.ts"]}"#;
    for (dialect, message) in [
        (TypeScript7, "'compilerOptions' must be an object"),
        (
            TypeScript56,
            "Compiler option 'compilerOptions' requires a value of type object",
        ),
    ] {
        let (_scratch, loaded) = load(dialect, &[("leaf/tsconfig.json", config), SOURCE]);
        let error = loaded.expect_err("a boolean loads");
        assert_eq!(error.message, message, "{dialect:?}");
        assert!(located(config, &error).starts_with("true"));
    }
}

/// A root-level `__proto__` whose value is an object sets the raw
/// configuration's prototype in 5.6.3's `convertObjectLiteralExpressionToJson`,
/// which `applyExtendedConfig`'s truthiness tests then see, in the leaf
/// (blocking a base's `include`, `files`, or `exclude`) and in a base
/// (supplying one), while `getPropFromRaw` does not: a JavaScript artifact
/// that changes the selected files, refused under 5.6 wherever it is.
/// TypeScript 7 reads an ordinary key. A `null`, array, or other value sets
/// no prototype that holds such a list, so it changes nothing. Scratch runs
/// root-proto-include, root-proto-in-base, root-proto-exclude,
/// root-proto-primitive, root-proto-null, root-proto-array.
#[test]
fn a_root_prototype_object_is_refused_under_5_6() {
    let leaf_with = |proto: &str| {
        format!(r#"{{"extends":"../base.json","compilerOptions":{{{BASE}}},"__proto__":{proto}}}"#)
    };
    let sources = [
        ("base.ts", "export {};\n"),
        ("leaf/main.ts", "export {};\n"),
    ];
    let include = r#"{"include":["base.ts"]}"#;
    let message = "TypeScript 5 would set the configuration's prototype with '__proto__' rather than read the key, which can change the files it selects and is not supported";
    for (name, config, base, at, seven) in [
        (
            "root-proto-include",
            leaf_with(r#"{"include":["zzz"]}"#),
            include,
            "leaf/tsconfig.json",
            vec!["../base.ts"],
        ),
        (
            "root-proto-in-base",
            format!(r#"{{"extends":"../base.json","compilerOptions":{{{BASE}}}}}"#),
            r#"{"__proto__":{"include":["base.ts"]}}"#,
            "base.json",
            vec!["main.ts"],
        ),
    ] {
        let mut files = vec![("leaf/tsconfig.json", config.as_str()), ("base.json", base)];
        files.extend(sources);
        let (_scratch, program) = loaded(TypeScript7, &files);
        assert_eq!(searched(&program), seven, "{name}");
        let (scratch, loaded) = load(TypeScript56, &files);
        let error = loaded.expect_err("a prototype loads");
        assert_eq!(error.kind, InputErrorKind::Unsupported, "{name}");
        assert_eq!(error.message, message, "{name}");
        assert_eq!(
            error.path,
            Some(scratch.root.join(at).canonicalize().unwrap()),
            "{name}"
        );
    }
    for proto in [r#""x""#, "null", r#"["x"]"#] {
        let config = leaf_with(proto);
        let mut files = vec![
            ("leaf/tsconfig.json", config.as_str()),
            ("base.json", include),
        ];
        files.extend(sources);
        for dialect in DIALECTS {
            let (_scratch, program) = loaded(dialect, &files);
            assert_eq!(searched(&program), ["../base.ts"], "{proto} {dialect:?}");
        }
    }
}

/// Number options are validated and not read: no effective input this
/// tool keeps depends on one, so a later `0` or `null` after a value loads
/// in both dialects, where 5.6.3 keeps `0` and unsets on `null` and 7.0.2
/// keeps the earlier value over `null`. Experiments number-zero,
/// number-null.
#[test]
fn number_options_are_validated_but_not_read() {
    for later in ["0", "null"] {
        let config = leaf(
            &format!(r#","maxNodeModuleJsDepth":1,"maxNodeModuleJsDepth":{later}"#),
            "",
        );
        for dialect in DIALECTS {
            loaded(dialect, &[("leaf/tsconfig.json", &config), SOURCE]);
        }
    }
}

/// A configuration package's `typesVersions` is read with the selected
/// compiler's version, as each compiler resolves `extends` with its own:
/// 5.6.3 and native 7.0.2 (`tsc --showConfig`) select different bases from
/// `{"<6": ..., "*": ...}`.
#[test]
fn extends_packages_read_types_versions_with_the_compiler_version() {
    use super::Target;
    let files = [
        (
            "leaf/tsconfig.json",
            r#"{"extends":"cfg/base","compilerOptions":{"noLib":true,"types":[],"moduleResolution":"bundler","esModuleInterop":true,"alwaysStrict":true},"files":["a.ts"]}"#,
        ),
        SOURCE,
        (
            "node_modules/cfg/package.json",
            r#"{"name":"cfg","version":"1.0.0","typesVersions":{"<6":{"*":["v5/*"]},"*":{"*":["v7/*"]}}}"#,
        ),
        (
            "node_modules/cfg/v5/base.json",
            r#"{"compilerOptions":{"target":"es2017"}}"#,
        ),
        (
            "node_modules/cfg/v7/base.json",
            r#"{"compilerOptions":{"target":"es2022"}}"#,
        ),
        (
            "node_modules/cfg/base.json",
            r#"{"compilerOptions":{"target":"es2020"}}"#,
        ),
    ];
    for (dialect, target) in [
        (TypeScript56, Target::Es2017),
        (TypeScript7, Target::Es2022),
    ] {
        let (_scratch, program) = loaded(dialect, &files);
        assert_eq!(program.options.target, target, "{dialect:?}");
    }
}
