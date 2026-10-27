//! Configuration under the TypeScript 5.6 dialect, beside the same
//! configuration under TypeScript 7, at `Project::load`. Expected messages
//! are TypeScript 5.6.3's (`parseJsonSourceFileConfigFileContent` and
//! `tsc --noEmit --incremental false` on scratch copies) without the final
//! period, as this tool words every diagnostic; TypeScript 7 cases keep this
//! tool's existing wording.

mod support;

use support::Dialect::{self, TypeScript7, TypeScript56};
use support::{BASE_OPTIONS, DIALECTS, Fixture};
use typescript_parser::analysis::{InputError, InputErrorKind};
use typescript_parser::project::Project;

/// Load a scratch project whose compiler options are `options`, under
/// `dialect`'s compiler: `node_modules/typescript` 7.0.2 or 5.6.3.
fn load(dialect: Dialect, options: &str) -> Result<Project, InputError> {
    load_with(dialect, options, &[])
}

/// `load`, with `files` beside the configuration, such as stand-ins for the
/// installed compiler's library declarations, which the TypeScript 5.6
/// dialect reads from its `lib` directory.
fn load_with(
    dialect: Dialect,
    options: &str,
    files: &[(&str, &str)],
) -> Result<Project, InputError> {
    let config = format!(r#"{{"compilerOptions":{{{options}}},"files":["a.ts"]}}"#);
    let mut all = vec![
        ("tsconfig.json", config.as_str()),
        ("a.ts", "export const a = 1;\n"),
    ];
    all.extend_from_slice(files);
    Fixture::with_dialect(dialect, &all).try_load()
}

/// The failure loading `options` must give.
fn fails(dialect: Dialect, options: &str, kind: InputErrorKind, message: &str) {
    match load(dialect, options) {
        Ok(_) => panic!("{dialect:?} loaded {options}"),
        Err(error) => {
            assert_eq!(error.kind, kind, "{dialect:?} {options}: {error}");
            assert_eq!(error.message, message, "{dialect:?} {options}");
            assert!(error.position.is_some(), "{dialect:?} {options}: unlocated");
        }
    }
}

fn loads(dialect: Dialect, options: &str) {
    if let Err(error) = load(dialect, options) {
        panic!("{dialect:?} failed {options}: {error}");
    }
}

/// 5.6.3's catalog decides which options and values exist: options 7.0.2
/// added are unknown (TS5023), a near miss gets its suggestion (TS5025), a
/// `--build` option is TS5093, and an unknown option is reported even when
/// `null`, all where TypeScript 7 loads.
#[test]
fn options_typescript_5_6_does_not_declare_are_its_errors() {
    for (options, message) in [
        (
            r#""stableTypeOrdering":true"#,
            "Unknown compiler option 'stableTypeOrdering'",
        ),
        (
            r#""deduplicatePackages":true"#,
            "Unknown compiler option 'deduplicatePackages'",
        ),
        (
            r#""libReplacement":false"#,
            "Unknown compiler option 'libReplacement'",
        ),
        (
            r#""erasableSyntaxOnly":true"#,
            "Unknown compiler option 'erasableSyntaxOnly'",
        ),
        (r#""bogus":null"#, "Unknown compiler option 'bogus'"),
    ] {
        loads(TypeScript7, &format!("{BASE_OPTIONS},{options}"));
        fails(
            TypeScript56,
            &format!("{BASE_OPTIONS},{options}"),
            InputErrorKind::Config,
            message,
        );
    }
    for (options, message) in [
        (
            r#""Strict":true"#,
            "Unknown compiler option 'Strict'. Did you mean 'strict'?",
        ),
        (
            r#""outfile":"x.js""#,
            "Unknown compiler option 'outfile'. Did you mean 'outFile'?",
        ),
        (
            r#""verbose":true"#,
            "Compiler option '--verbose' may only be used with '--build'",
        ),
        (
            r#""Verbose":true"#,
            "Compiler option '--Verbose' may only be used with '--build'",
        ),
    ] {
        fails(
            TypeScript56,
            &format!("{BASE_OPTIONS},{options}"),
            InputErrorKind::Config,
            message,
        );
    }
}

/// 5.6.3's `getSpellingSuggestion` and `levenshteinWithMax` measure names
/// in UTF-16 code units and lower each unit with JavaScript's
/// `toLowerCase`, so an astral character counts twice and U+0130 lowers to
/// two units, matching no ASCII letter. An unknown name that lowers to any
/// `--build` option's name, compiler options included, is TS5093
/// (`compilerOptionsAlternateMode`). Each expectation is 5.6.3's
/// `convertCompilerOptionsFromJson`.
#[test]
fn unknown_option_suggestions_follow_typescript_5_6s_spelling_rule() {
    let unknown = |name: &str| format!("Unknown compiler option '{name}'");
    let suggest = |name: &str, option: &str| {
        format!("Unknown compiler option '{name}'. Did you mean '{option}'?")
    };
    let build = |name: &str| format!("Compiler option '--{name}' may only be used with '--build'");
    for (name, message) in [
        ("stric😀", unknown("stric😀")),
        ("strict😀😀", unknown("strict😀😀")),
        ("li😀", unknown("li😀")),
        ("lİb", unknown("lİb")),
        ("LİB", unknown("LİB")),
        ("ab", unknown("ab")),
        ("AB", unknown("AB")),
        ("lib😀", suggest("lib😀", "lib")),
        ("😀strict", suggest("😀strict", "strict")),
        ("STRICT", suggest("STRICT", "strict")),
        ("allowJS", suggest("allowJS", "allowJs")),
        ("strïct", suggest("strïct", "strict")),
        ("stricΣ", suggest("stricΣ", "strict")),
        ("wätch", suggest("wätch", "watch")),
        ("noImplicitAnÿ", suggest("noImplicitAnÿ", "noImplicitAny")),
        (
            "esModuleInteropp",
            suggest("esModuleInteropp", "esModuleInterop"),
        ),
        (
            "tsBuildInfoFilE",
            suggest("tsBuildInfoFilE", "tsBuildInfoFile"),
        ),
        ("watcH", build("watcH")),
        ("Incremental", build("Incremental")),
        ("Declaration", build("Declaration")),
        ("listfiles", build("listfiles")),
        ("NoCheck", build("NoCheck")),
        ("Help", build("Help")),
    ] {
        let options = format!(
            "{BASE_OPTIONS},{}:true",
            serde_json::to_string(name).unwrap()
        );
        fails(TypeScript56, &options, InputErrorKind::Config, &message);
    }
}

/// A key with an unpaired surrogate names no option, so 5.6.3 reports it as
/// unknown even when `null`; this tool cannot name it exactly, so it is
/// `Unsupported`. TypeScript 7 ignores an unknown `null`.
#[test]
fn an_unrepresentable_key_is_unsupported_even_when_null() {
    let options = format!(r#"{BASE_OPTIONS},"\uD800":null"#);
    loads(TypeScript7, &options);
    fails(
        TypeScript56,
        &options,
        InputErrorKind::Unsupported,
        "A compiler option key has an unpaired UTF-16 surrogate",
    );
}

/// Each occurrence's value is converted by 5.6.3's declaration: its type
/// names (`Array` for a list), its list elements' names (`suffix`,
/// `condition`, `plugin`), and its enumerations, whose TS6046 message names
/// the option as `--name` and lists 5.6.3's keys.
#[test]
fn values_are_converted_by_typescript_5_6s_declarations() {
    for (options, typescript_7, typescript_56) in [
        (
            r#""lib":"es5""#,
            "Compiler option 'lib' requires a value of type list",
            "Compiler option 'lib' requires a value of type Array",
        ),
        (
            r#""moduleSuffixes":[1]"#,
            "'moduleSuffixes' entries must be strings",
            "Compiler option 'suffix' requires a value of type string",
        ),
        (
            r#""customConditions":[{}]"#,
            "'customConditions' entries must be strings",
            "Compiler option 'condition' requires a value of type string",
        ),
        (
            r#""types":[null,2]"#,
            "'types' entries must be strings",
            "Compiler option 'types' requires a value of type string",
        ),
        (
            r#""plugins":[1]"#,
            "Compiler option 'plugin' requires a value of type object",
            "Compiler option 'plugin' requires a value of type object",
        ),
        (
            r#""target":1"#,
            "Compiler option 'target' requires a value of type string",
            "Compiler option 'target' requires a value of type string",
        ),
        (
            r#""strict":"yes""#,
            "Compiler option 'strict' requires a value of type boolean",
            "Compiler option 'strict' requires a value of type boolean",
        ),
    ] {
        let options = format!("{BASE_OPTIONS},{options}");
        fails(TypeScript7, &options, InputErrorKind::Config, typescript_7);
        fails(
            TypeScript56,
            &options,
            InputErrorKind::Config,
            typescript_56,
        );
    }
    for (options, message) in [
        (
            r#""module":"node18","moduleResolution":"nodenext""#,
            "Argument for '--module' option must be: 'none', 'commonjs', 'amd', 'system', 'umd', 'es6', 'es2015', 'es2020', 'es2022', 'esnext', 'node16', 'nodenext', 'preserve'",
        ),
        (
            r#""target":"es2024""#,
            "Argument for '--target' option must be: 'es5', 'es6', 'es2015', 'es2016', 'es2017', 'es2018', 'es2019', 'es2020', 'es2021', 'es2022', 'es2023', 'esnext'",
        ),
        (
            r#""importsNotUsedAsValues":"bogus""#,
            "Argument for '--importsNotUsedAsValues' option must be: 'remove', 'preserve', 'error'",
        ),
        (
            r#""jsx":"react-natİve""#,
            "Argument for '--jsx' option must be: 'preserve', 'react-native', 'react', 'react-jsx', 'react-jsxdev'",
        ),
        (
            r#""jsx":"""#,
            "Argument for '--jsx' option must be: 'preserve', 'react-native', 'react', 'react-jsx', 'react-jsxdev'",
        ),
        (
            r#""moduleDetection":" force ""#,
            "Argument for '--moduleDetection' option must be: 'auto', 'legacy', 'force'",
        ),
    ] {
        fails(
            TypeScript56,
            &format!("{BASE_OPTIONS},{options}"),
            InputErrorKind::Config,
            message,
        );
    }
    // TypeScript 7 lowers one character for one: U+0130 is `i`.
    loads(
        TypeScript7,
        &format!(r#"{BASE_OPTIONS},"jsx":"react-natİve""#),
    );
    // Both lower ASCII case alike.
    for dialect in DIALECTS {
        loads(dialect, &format!(r#"{BASE_OPTIONS},"jsx":"React-JSX""#));
    }
}

/// `lib` entries are 5.6.3's library names: one only 7.0.2 knows is TS6046
/// with 5.6.3's list, where TypeScript 7 loads it, and a name both know
/// selects the library under each.
#[test]
fn library_names_are_typescript_5_6s() {
    let message = lib_message();
    for name in ["es2024", "esnext.date", "esnext.temporal"] {
        let options = format!(r#"{BASE_OPTIONS},"lib":["{name}"]"#).replace(r#""noLib":true,"#, "");
        fails(TypeScript56, &options, InputErrorKind::Config, &message);
        loads(TypeScript7, &options);
    }
    // KELVIN SIGN lowers to `k` in both. TypeScript 5.6 reads the library
    // from the installed compiler, here a stand-in.
    let options =
        format!("{BASE_OPTIONS},\"lib\":[\"webwor\u{212A}er\"]").replace(r#""noLib":true,"#, "");
    loads(TypeScript7, &options);
    let webworker = [(
        "node_modules/typescript/lib/lib.webworker.d.ts",
        "declare var stand_in: number;\n",
    )];
    if let Err(error) = load_with(TypeScript56, &options, &webworker) {
        panic!("TypeScript56 failed {options}: {error}");
    }
}

/// TypeScript 5.6.3's TS6046 text for `lib`.
fn lib_message() -> String {
    "Argument for '--lib' option must be: 'es5', 'es6', 'es2015', 'es7', 'es2016', 'es2017', 'es2018', 'es2019', 'es2020', 'es2021', 'es2022', 'es2023', 'esnext', 'dom', 'dom.iterable', 'dom.asynciterable', 'webworker', 'webworker.importscripts', 'webworker.iterable', 'webworker.asynciterable', 'scripthost', 'es2015.core', 'es2015.collection', 'es2015.generator', 'es2015.iterable', 'es2015.promise', 'es2015.proxy', 'es2015.reflect', 'es2015.symbol', 'es2015.symbol.wellknown', 'es2016.array.include', 'es2016.intl', 'es2017.date', 'es2017.object', 'es2017.sharedmemory', 'es2017.string', 'es2017.intl', 'es2017.typedarrays', 'es2018.asyncgenerator', 'es2018.asynciterable', 'es2018.intl', 'es2018.promise', 'es2018.regexp', 'es2019.array', 'es2019.object', 'es2019.string', 'es2019.symbol', 'es2019.intl', 'es2020.bigint', 'es2020.date', 'es2020.promise', 'es2020.sharedmemory', 'es2020.string', 'es2020.symbol.wellknown', 'es2020.intl', 'es2020.number', 'es2021.promise', 'es2021.string', 'es2021.weakref', 'es2021.intl', 'es2022.array', 'es2022.error', 'es2022.intl', 'es2022.object', 'es2022.sharedmemory', 'es2022.string', 'es2022.regexp', 'es2023.array', 'es2023.collection', 'es2023.intl', 'esnext.array', 'esnext.collection', 'esnext.symbol', 'esnext.asynciterable', 'esnext.intl', 'esnext.disposable', 'esnext.bigint', 'esnext.string', 'esnext.promise', 'esnext.weakref', 'esnext.decorators', 'esnext.object', 'esnext.regexp', 'esnext.iterator', 'decorators', 'decorators.legacy'".into()
}

/// The failure loading `options` must give, at the configuration that
/// leaves an implied value unset rather than at any one value.
fn fails_unlocated(dialect: Dialect, options: &str, kind: InputErrorKind, message: &str) {
    match load(dialect, options) {
        Ok(_) => panic!("{dialect:?} loaded {options}"),
        Err(error) => {
            assert_eq!(error.kind, kind, "{dialect:?} {options}: {error}");
            assert_eq!(error.message, message, "{dialect:?} {options}");
            assert!(
                error
                    .path
                    .as_ref()
                    .is_some_and(|path| path.ends_with("tsconfig.json")),
                "{dialect:?} {options}: {error}"
            );
        }
    }
}

/// What TypeScript 5's defaults imply and this tool refuses is named with
/// the option and whether the default set it (`computedOptions`): with
/// nothing set, `target` is ES5; with a modern target, `module` is ES2015
/// and `moduleResolution` Classic; a non-Node module leaves
/// `esModuleInterop` false; and without `strict`, `alwaysStrict` is false.
/// TypeScript 7 loads each.
#[test]
fn refused_defaults_name_typescript_5s_default() {
    const PROFILE: &str = "is not supported under the TypeScript 5.6 profile";
    let none = r#""noLib":true,"types":[]"#;
    for (options, message) in [
        (
            none.to_owned(),
            format!(
                "Option 'target' is 'es5' by TypeScript 5's default, which {PROFILE}; set 'target' to 'es2015' or later"
            ),
        ),
        (
            format!(r#"{none},"target":"es2022","strict":true"#),
            format!(
                "Option 'moduleResolution' is 'classic' by TypeScript 5's default for this 'module', which {PROFILE}; set 'moduleResolution' to 'node16', 'nodenext', or 'bundler'"
            ),
        ),
        (
            format!(
                r#"{none},"target":"es2022","module":"esnext","moduleResolution":"bundler","strict":true"#
            ),
            format!(
                "Option 'esModuleInterop' is false by TypeScript 5's default for this 'module', which {PROFILE}; set 'esModuleInterop' to true"
            ),
        ),
        (
            format!(r#"{none},"module":"node16""#),
            format!(
                "Option 'alwaysStrict' is false by TypeScript 5's default when 'strict' is unset, which {PROFILE}; set 'strict' or 'alwaysStrict' to true"
            ),
        ),
    ] {
        loads(TypeScript7, &options);
        fails_unlocated(
            TypeScript56,
            &options,
            InputErrorKind::Unsupported,
            &message,
        );
    }
    // A module named only by its default is named at `module`.
    fails(
        TypeScript56,
        &format!(r#"{none},"module":"commonjs","target":"es2022","strict":true"#),
        InputErrorKind::Unsupported,
        &format!(
            "Option 'moduleResolution' is 'node10' by TypeScript 5's default for this 'module', which {PROFILE}; set 'moduleResolution' to 'node16', 'nodenext', or 'bundler'"
        ),
    );
    fails(
        TypeScript56,
        &format!(r#"{none},"module":"node16","strict":false"#),
        InputErrorKind::Unsupported,
        &format!(
            "Option 'alwaysStrict' is false because 'strict' is false, which {PROFILE}; set 'strict' or 'alwaysStrict' to true"
        ),
    );
}

/// Explicit settings this tool refuses under the dialect, each named at
/// the value that sets it.
#[test]
fn refused_settings_are_named_where_they_are_set() {
    const PROFILE: &str = "is not supported under the TypeScript 5.6 profile";
    let modern = r#""noLib":true,"types":[],"strict":true,"esModuleInterop":true"#;
    let bundler =
        format!(r#"{modern},"target":"es2022","module":"esnext","moduleResolution":"bundler""#);
    let mut cases = vec![
        (
            format!(r#"{modern},"target":"es5","module":"nodenext""#),
            format!("Option 'target=ES5' {PROFILE}; set 'target' to 'es2015' or later"),
        ),
        (
            format!(r#"{bundler},"esModuleInterop":false"#),
            format!("Option 'esModuleInterop=false' {PROFILE}; set 'esModuleInterop' to true"),
        ),
        (
            format!(r#"{bundler},"allowSyntheticDefaultImports":false"#),
            format!(
                "Option 'allowSyntheticDefaultImports=false' {PROFILE}; set 'allowSyntheticDefaultImports' to true"
            ),
        ),
        (
            format!(r#"{bundler},"alwaysStrict":false"#),
            format!(
                "Option 'alwaysStrict=false' {PROFILE}; set 'strict' or 'alwaysStrict' to true"
            ),
        ),
        (
            format!(r#"{bundler},"outFile":"out.js""#),
            format!("Option 'outFile' {PROFILE}"),
        ),
        (
            format!(r#"{bundler},"noResolve":true"#),
            "Option 'noResolve' changes module resolution in a way that is not supported".into(),
        ),
    ];
    for module in ["none", "amd", "umd", "system"] {
        cases.push((
            format!(r#"{modern},"target":"es2022","module":"{module}""#),
            format!("Option 'module={module}' {PROFILE}; set 'module' to 'es2015' or later, 'node16', 'nodenext', or 'preserve'"),
        ));
    }
    for resolution in ["classic", "node", "node10"] {
        cases.push((
            format!(r#"{modern},"target":"es2022","module":"esnext","moduleResolution":"{resolution}""#),
            format!("Option 'moduleResolution={resolution}' {PROFILE}; set 'moduleResolution' to 'node16', 'nodenext', or 'bundler'"),
        ));
    }
    for (options, message) in cases {
        fails(
            TypeScript56,
            &options,
            InputErrorKind::Unsupported,
            &message,
        );
    }
    // `baseUrl` and an unset `types` are read as TypeScript 5.6.3 reads
    // them, and `*` names a package called `*` (TS2688 when it is absent).
    loads(TypeScript56, &format!(r#"{bundler},"baseUrl":".""#));
    loads(TypeScript56, &bundler.replace(r#""types":[],"#, ""));
    fails(
        TypeScript56,
        &bundler.replace(r#""types":[]"#, r#""types":["*"]"#),
        InputErrorKind::UnresolvedImport,
        "Cannot find type definition file for '*'",
    );
    // TypeScript 5.6.3 still reads `downlevelIteration`, which only ES5
    // output uses; TypeScript 7 removed it.
    loads(
        TypeScript56,
        &format!(r#"{bundler},"downlevelIteration":true"#),
    );
    fails(
        TypeScript7,
        &format!(r#"{bundler},"downlevelIteration":true"#),
        InputErrorKind::Config,
        "Option 'downlevelIteration' was removed in TypeScript 7",
    );
}

/// What the defaults compute where they load: `module: node16` alone
/// targets ES2022 and `nodenext` ESNext, `preserve` resolves as `bundler`,
/// and `alwaysStrict` without `strict` loads.
#[test]
fn computed_defaults_that_load() {
    for options in [
        r#""noLib":true,"types":[],"module":"node16","strict":true"#,
        r#""noLib":true,"types":[],"module":"nodenext","strict":true"#,
        r#""noLib":true,"types":[],"module":"preserve","target":"es2022","strict":true"#,
        r#""noLib":true,"types":[],"module":"esnext","moduleResolution":"bundler","target":"es2022","alwaysStrict":true,"esModuleInterop":true"#,
    ] {
        loads(TypeScript56, options);
    }
}

/// `resolveJsonModule` is implied for bundler resolution alone in
/// TypeScript 5.6.3, so `module: preserve` (bundler by default) admits a
/// JSON import while `nodenext` does not, where TypeScript 7 admits both.
#[test]
fn json_modules_follow_the_computed_resolution() {
    let load_json = |dialect: Dialect, options: &str| {
        let config = format!(r#"{{"compilerOptions":{{{options}}},"files":["a.ts"]}}"#);
        Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", &config),
                (
                    "a.ts",
                    "import data from \"./data.json\" with { type: \"json\" };\nexport const x = data;\n",
                ),
                ("data.json", "{\"value\": 1}\n"),
            ],
        )
        .try_load()
    };
    let preserve = r#""noLib":true,"types":[],"module":"preserve","target":"es2022","strict":true"#;
    let nodenext = r#""noLib":true,"types":[],"module":"nodenext","strict":true"#;
    for dialect in DIALECTS {
        assert!(load_json(dialect, preserve).is_ok(), "{dialect:?} preserve");
    }
    assert!(load_json(TypeScript7, nodenext).is_ok());
    let error = load_json(TypeScript56, nodenext).expect_err("loaded");
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
}

/// Options removed in TypeScript 5.5 are TS5102, or TS5108 with a value,
/// even with `ignoreDeprecations: "5.0"`, which silences only deprecations
/// still to be removed (`verifyDeprecatedCompilerOptions`); a set value
/// other than `5.0` is TS5103. A setting JavaScript reads as falsy passes.
#[test]
fn options_removed_in_typescript_5_5_are_errors() {
    let base = r#""noLib":true,"types":[],"module":"node16","strict":true"#;
    for (option, message) in [
        (r#""charset":"utf8""#, "Option 'charset' has been removed. Please remove it from your configuration".to_owned()),
        (r#""keyofStringsOnly":true"#, "Option 'keyofStringsOnly' has been removed. Please remove it from your configuration".into()),
        (r#""noImplicitUseStrict":true"#, "Option 'noImplicitUseStrict' has been removed. Please remove it from your configuration".into()),
        (r#""noStrictGenericChecks":true"#, "Option 'noStrictGenericChecks' has been removed. Please remove it from your configuration".into()),
        (r#""suppressExcessPropertyErrors":true"#, "Option 'suppressExcessPropertyErrors' has been removed. Please remove it from your configuration".into()),
        (r#""suppressImplicitAnyIndexErrors":true"#, "Option 'suppressImplicitAnyIndexErrors' has been removed. Please remove it from your configuration".into()),
        (r#""out":"x.js""#, "Option 'out' has been removed. Please remove it from your configuration. Use 'outFile' instead".into()),
        (r#""importsNotUsedAsValues":"error""#, "Option 'importsNotUsedAsValues' has been removed. Please remove it from your configuration. Use 'verbatimModuleSyntax' instead".into()),
        (r#""importsNotUsedAsValues":"preserve""#, "Option 'importsNotUsedAsValues' has been removed. Please remove it from your configuration. Use 'verbatimModuleSyntax' instead".into()),
        (r#""preserveValueImports":true"#, "Option 'preserveValueImports' has been removed. Please remove it from your configuration. Use 'verbatimModuleSyntax' instead".into()),
        (r#""target":"es3""#, "Option 'target=ES3' has been removed. Please remove it from your configuration".into()),
    ] {
        for ignore in ["", r#","ignoreDeprecations":"5.0""#] {
            fails(
                TypeScript56,
                &format!("{base},{option}{ignore}"),
                InputErrorKind::Config,
                &message,
            );
        }
    }
    for option in [
        r#""importsNotUsedAsValues":"remove""#,
        r#""keyofStringsOnly":false"#,
        r#""charset":"""#,
        r#""ignoreDeprecations":"5.0""#,
        r#""ignoreDeprecations":"""#,
    ] {
        loads(TypeScript56, &format!("{base},{option}"));
    }
    for value in ["5.5", "5", "4.0", "6.0"] {
        fails(
            TypeScript56,
            &format!(r#"{base},"ignoreDeprecations":"{value}""#),
            InputErrorKind::Config,
            "Invalid value for '--ignoreDeprecations'",
        );
    }
}

/// `verifyCompilerOptions`' checks under 5.6.3's computed values and in its
/// words: TS5052, TS5053, TS5091, TS5095, TS5109, TS5110, and the `paths`
/// checks TS5064 and TS5090.
#[test]
fn verification_follows_typescript_5_6() {
    let base = r#""noLib":true,"types":[],"strict":true"#;
    for (options, message) in [
        (
            r#""module":"commonjs","moduleResolution":"bundler","target":"es2022""#,
            "Option 'bundler' can only be used when 'module' is set to 'preserve' or to 'es2015' or later",
        ),
        (
            r#""module":"node16","moduleResolution":"bundler""#,
            "Option 'bundler' can only be used when 'module' is set to 'preserve' or to 'es2015' or later",
        ),
        (
            r#""module":"node16","verbatimModuleSyntax":true,"preserveConstEnums":false"#,
            "Option 'preserveConstEnums' cannot be disabled when 'verbatimModuleSyntax' is enabled",
        ),
        (
            r#""module":"node16","isolatedModules":true,"preserveConstEnums":false"#,
            "Option 'preserveConstEnums' cannot be disabled when 'isolatedModules' is enabled",
        ),
        (
            r#""module":"esnext","moduleResolution":"node16""#,
            "Option 'module' must be set to 'Node16' when option 'moduleResolution' is set to 'Node16'",
        ),
        (
            r#""module":"esnext","moduleResolution":"nodenext""#,
            "Option 'module' must be set to 'NodeNext' when option 'moduleResolution' is set to 'NodeNext'",
        ),
        (
            r#""module":"node16","strictNullChecks":false,"strictPropertyInitialization":true"#,
            "Option 'strictPropertyInitialization' cannot be specified without specifying option 'strictNullChecks'",
        ),
        (
            r#""module":"node16","strictNullChecks":false,"exactOptionalPropertyTypes":true"#,
            "Option 'exactOptionalPropertyTypes' cannot be specified without specifying option 'strictNullChecks'",
        ),
        (
            r#""module":"node16","lib":["es5"]"#,
            "Option 'lib' cannot be specified with option 'noLib'",
        ),
        (
            r#""module":"node16","paths":{"a":[1]}"#,
            "Substitution '1' for pattern 'a' has incorrect type, expected 'string', got 'number'",
        ),
        (
            r#""module":"node16","paths":{"a":[1e3]}"#,
            "Substitution '1000' for pattern 'a' has incorrect type, expected 'string', got 'number'",
        ),
        (
            r#""module":"node16","paths":{"a":[[1,null,{}]]}"#,
            "Substitution '1,,[object Object]' for pattern 'a' has incorrect type, expected 'string', got 'object'",
        ),
        (
            r#""module":"node16","paths":{"a":["x"]}"#,
            "Non-relative paths are not allowed when 'baseUrl' is not set. Did you forget a leading './'?",
        ),
    ] {
        fails(
            TypeScript56,
            &format!("{base},{options}"),
            InputErrorKind::Config,
            message,
        );
    }
    // TS5109 at the explicit `moduleResolution` once TS5095 is satisfied.
    fails(
        TypeScript56,
        &format!(r#"{base},"module":"node16","moduleResolution":"classic""#),
        InputErrorKind::Config,
        "Option 'moduleResolution' must be set to 'Node16' (or left unspecified) when option 'module' is set to 'Node16'",
    );
    // Both Node kinds pair with either Node resolution.
    loads(
        TypeScript56,
        &format!(r#"{base},"module":"nodenext","moduleResolution":"node16""#),
    );
    // An empty `paths` array maps nothing, as `{}` does; another is refused.
    loads(
        TypeScript56,
        &format!(r#"{base},"module":"node16","paths":[]"#),
    );
    fails(
        TypeScript56,
        &format!(r#"{base},"module":"node16","paths":[["./a"]]"#),
        InputErrorKind::Unsupported,
        "TypeScript 5 would read the indices of this 'paths' array as its patterns, which is not supported",
    );
}

/// A synthetic configuration with the option and pattern shapes of
/// Artemis's, beside a TypeScript 5.6.3 manifest, stubs of the three
/// library files its `lib` names, and two source stubs, loads end to end
/// under the 5.6 dialect: its `baseUrl`, non-relative `paths`, unset
/// `types`, and `ts-node` section are read as TypeScript 5.6.3 reads them
/// (the effective options are `config::conversion_tests`' to compare). The
/// same configuration under TypeScript 7 still fails at `baseUrl`, as
/// native 7.0.2's TS5102 does.
#[test]
fn a_base_url_project_loads_under_typescript_5_6() {
    use typescript_parser::project::ProjectRequest;
    use typescript_parser::references::ReferenceQuery;
    let fixture = Fixture::copy_from_repository("fixtures/configuration/base-url-project");
    let project = fixture.load();
    let request = ProjectRequest::References(ReferenceQuery::Name {
        name: "fixture".into(),
        file: None,
    });
    project.execute(request).unwrap();
    fixture.claim_compiler(TypeScript7.version());
    let error = fixture.try_load().unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
    assert!(
        error
            .message
            .starts_with("Option 'baseUrl' was removed in TypeScript 7")
    );
}

/// A session's answer equals a fresh direct load's, failures included,
/// after each change: `ask` asks both and returns the session's.
fn refreshes(config: &std::path::Path) -> impl FnMut(&str, &str) -> Result<String, String> + use<> {
    use typescript_parser::project::{ProjectRequest, ProjectSession};
    use typescript_parser::references::ReferenceQuery;
    let config = config.to_path_buf();
    let mut session = ProjectSession::new(&config).unwrap();
    move |step: &str, name: &str| {
        let request = ProjectRequest::References(ReferenceQuery::Name {
            name: name.into(),
            file: None,
        });
        let refreshed = session
            .execute(request.clone())
            .map(|response| format!("{response:?}"))
            .map_err(|error| error.to_string());
        let fresh = Project::load(&config)
            .and_then(|project| project.execute(request))
            .map(|response| format!("{response:?}"))
            .map_err(|error| error.to_string());
        assert_eq!(refreshed, fresh, "{step}");
        refreshed
    }
}

/// Editing an `extends` base changes what a session reads, as a fresh load
/// reads it: under 5.6 the base's `baseUrl` moves the `paths` target, and
/// removing it makes the non-relative substitution TS5090.
#[test]
fn a_session_refreshes_when_an_extends_base_changes() {
    let base = |base_url: &str| {
        format!(r#"{{"compilerOptions":{{{BASE_OPTIONS}{base_url},"paths":{{"@/*":["src/*"]}}}}}}"#)
    };
    let fixture = Fixture::with_dialect(
        TypeScript56,
        &[
            (
                "app/tsconfig.json",
                r#"{"extends":"../base.json","files":["main.ts"]}"#,
            ),
            ("base.json", &base(r#","baseUrl":".""#)),
            (
                "app/main.ts",
                "import { x } from '@/x';\nexport const y = x;\n",
            ),
            ("src/x.ts", "export const x = 1;\n"),
            ("alt/src/x.ts", "export const x = 2;\n"),
        ],
    );
    let mut ask = refreshes(&fixture.path("app/tsconfig.json"));
    let first = ask("baseUrl .", "x").unwrap();
    assert!(
        first.contains("src/x.ts") && !first.contains("alt/"),
        "{first}"
    );
    std::fs::write(fixture.path("base.json"), base(r#","baseUrl":"./alt""#)).unwrap();
    let moved = ask("baseUrl ./alt", "x").unwrap();
    assert!(moved.contains("alt/src/x.ts"), "{moved}");
    std::fs::write(fixture.path("base.json"), base("")).unwrap();
    let error = ask("baseUrl removed", "x").unwrap_err();
    assert!(
        error.contains("Non-relative paths are not allowed when 'baseUrl' is not set"),
        "{error}"
    );
}

/// `${configDir}` names the selected configuration's directory, so moving
/// the configuration, here by retargeting the link to its directory,
/// changes what a session reads, as a fresh load reads it, in both
/// dialects.
#[cfg(unix)]
#[test]
fn a_session_refreshes_when_a_config_dir_leaf_moves() {
    for dialect in DIALECTS {
        let base =
            format!(r#"{{"compilerOptions":{{{BASE_OPTIONS}}},"include":["${{configDir}}/src"]}}"#);
        let fixture = Fixture::with_dialect(
            dialect,
            &[
                ("base.json", &base),
                ("v1/tsconfig.json", r#"{"extends":"../base.json"}"#),
                ("v1/src/a.ts", "export const one = 1;\n"),
                ("v2/tsconfig.json", r#"{"extends":"../base.json"}"#),
                ("v2/src/a.ts", "export const two = 2;\n"),
            ],
        );
        std::os::unix::fs::symlink("v1", fixture.path("current")).unwrap();
        let mut ask = refreshes(&fixture.path("current/tsconfig.json"));
        // A name nothing declares answers with no groups.
        let found = |answer: Result<String, String>| !answer.unwrap().contains("groups: []");
        assert!(found(ask("v1", "one")), "{dialect:?}");
        std::fs::remove_file(fixture.path("current")).unwrap();
        std::os::unix::fs::symlink("v2", fixture.path("current")).unwrap();
        assert!(!found(ask("v2", "one")), "{dialect:?}");
        assert!(found(ask("v2", "two")), "{dialect:?}");
    }
}
