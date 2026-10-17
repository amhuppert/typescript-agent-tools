//! Effective configuration is validated through the final project load.
//! Expectations follow TypeScript 7.0.2's option catalog and
//! `Program.verifyCompilerOptions`; option-dependent type results are tested
//! with the checker, rather than exposing configuration as a public API.

mod support;

use support::Fixture;
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::Project;

fn configuration(options: &str) -> String {
    format!(r#"{{"compilerOptions":{{"noLib":true,"types":[],{options}}},"files":["a.ts"]}}"#)
}

fn rejects(options: &str, fragment: &str) {
    let config = configuration(options);
    let fixture = Fixture::new(&[("tsconfig.json", &config), ("a.ts", "export {};\n")]);
    let error = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Config, "{options}: {error}");
    assert!(error.message.contains(fragment), "{options}: {error}");
    assert_eq!(error.path, Some(fixture.path("tsconfig.json")));
    assert!(error.position.is_some(), "{options}");
}

#[test]
fn every_semantic_boolean_is_validated_even_when_overridden() {
    for name in [
        "strict",
        "noImplicitAny",
        "noImplicitThis",
        "strictNullChecks",
        "strictFunctionTypes",
        "strictBindCallApply",
        "strictPropertyInitialization",
        "strictBuiltinIteratorReturn",
        "useUnknownInCatchVariables",
        "exactOptionalPropertyTypes",
        "noUncheckedIndexedAccess",
        "noPropertyAccessFromIndexSignature",
        "noErrorTruncation",
        "stableTypeOrdering",
    ] {
        rejects(&format!(r#""{name}":"false","{name}":false"#), name);
        let base = format!(r#"{{"compilerOptions":{{"{name}":"false"}}}}"#);
        let config = format!(
            r#"{{"extends":"./base.json","compilerOptions":{{"noLib":true,"{name}":false}},"files":["a.ts"]}}"#
        );
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            ("base.json", &base),
            ("a.ts", "export {};\n"),
        ]);
        let error = Project::load(&fixture.path("tsconfig.json"))
            .map_err(support::input_error)
            .unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Config, "{name}: {error}");
        assert_eq!(error.path, Some(fixture.path("base.json")));
    }
}

#[test]
fn removed_modes_are_errors_but_supported_modes_and_nulls_load() {
    for name in [
        "alwaysStrict",
        "esModuleInterop",
        "allowSyntheticDefaultImports",
    ] {
        rejects(&format!(r#""{name}":false"#), "removed");
        for value in ["true", "null"] {
            let config = configuration(&format!(r#""{name}":{value}"#));
            let fixture = Fixture::new(&[("tsconfig.json", &config), ("a.ts", "export {};\n")]);
            fixture.load();
        }
    }
    for value in ["true", "false"] {
        rejects(&format!(r#""downlevelIteration":{value}"#), "removed");
        rejects(
            &format!(r#""keyofStringsOnly":{value}"#),
            "Unknown compiler option",
        );
    }
    for value in [r#""x.js""#, r#""""#] {
        rejects(&format!(r#""outFile":{value}"#), "removed");
    }
    let config =
        configuration(r#""downlevelIteration":null,"outFile":null,"keyofStringsOnly":null"#);
    let fixture = Fixture::new(&[("tsconfig.json", &config), ("a.ts", "export {};\n")]);
    fixture.load();
}

#[test]
fn inherited_removed_modes_follow_effective_overrides() {
    for name in [
        "alwaysStrict",
        "esModuleInterop",
        "allowSyntheticDefaultImports",
    ] {
        let base = format!(r#"{{"compilerOptions":{{"{name}":false}}}}"#);
        for value in [None, Some("true"), Some("null")] {
            let setting = value.map_or(String::new(), |value| format!(r#", "{name}":{value}"#));
            let config = format!(
                r#"{{"extends":"./base.json","compilerOptions":{{"noLib":true{setting}}},"files":["a.ts"]}}"#
            );
            let fixture = Fixture::new(&[
                ("tsconfig.json", &config),
                ("base.json", &base),
                ("a.ts", "export {};\n"),
            ]);
            match value {
                None => {
                    let error = Project::load(&fixture.path("tsconfig.json"))
                        .map_err(support::input_error)
                        .unwrap_err();
                    assert_eq!(error.kind, InputErrorKind::Config, "{name}: {error}");
                    assert!(error.message.contains("removed"));
                    assert_eq!(error.path, Some(fixture.path("base.json")));
                }
                Some(_) => {
                    fixture.load();
                }
            }
        }
    }
}

#[test]
fn catalog_validates_values_and_exact_keys_without_rejecting_null_unknowns() {
    for (options, fragment) in [
        (r#""noUnusedLocals":"true""#, "boolean"),
        (r#""sourceRoot":false"#, "string"),
        (r#""checkers":"2""#, "number"),
        (r#""plugins":["plugin"]"#, "object"),
        (r#""plugins":{}"#, "list"),
        (r#""customConditions":[false]"#, "strings"),
        (r#""newLine":"unknown""#, "crlf"),
        (r#""unknownOption":true"#, "Unknown compiler option"),
        (r#""Strict":true"#, "Unknown compiler option"),
        (r#""watch":null"#, "command line"),
    ] {
        rejects(options, fragment);
    }
    let config = configuration(
        r#""unknownOption":null,"Strict":null,"":true,"plugins":[{},null],"checkers":1.5,"newLine":"Lf""#,
    );
    let fixture = Fixture::new(&[("tsconfig.json", &config), ("a.ts", "export {};\n")]);
    fixture.load();
}

#[test]
fn explicit_null_sensitive_option_requires_strict_null_checks() {
    for name in ["strictPropertyInitialization", "exactOptionalPropertyTypes"] {
        rejects(
            &format!(r#""strictNullChecks":false,"{name}":true"#),
            "strictNullChecks",
        );
    }
    // `verifyCompilerOptions` checks the explicit setting, not the strict
    // default of property initialization: strictNullChecks alone may be off.
    let config = configuration(r#""strictNullChecks":false"#);
    let fixture = Fixture::new(&[("tsconfig.json", &config), ("a.ts", "export {};\n")]);
    fixture.load();
}

#[test]
fn check_js_enables_js_inputs_unless_allow_js_is_explicitly_false() {
    for check_js in ["true", "false", "null"] {
        for allow_js in [None, Some("true"), Some("false"), Some("null")] {
            let setting = allow_js.map_or(String::new(), |value| format!(r#", "allowJs":{value}"#));
            let config =
                format!(r#"{{"compilerOptions":{{"noLib":true,"checkJs":{check_js}{setting}}}}}"#);
            let fixture = Fixture::new(&[
                ("tsconfig.json", &config),
                ("a.ts", "export {};\n"),
                ("b.js", "export {};\n"),
            ]);
            let enabled = match allow_js {
                Some("true") => true,
                Some("false") => false,
                _ => check_js == "true",
            };
            let result =
                Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error);
            if enabled {
                let error = result.unwrap_err();
                assert_eq!(error.kind, InputErrorKind::Unsupported, "{config}: {error}");
                assert!(error.message.contains("JavaScript"), "{config}: {error}");
            } else {
                result.unwrap();
            }
        }
    }
    for (base, own, enabled) in [
        (r#""checkJs":true"#, "", true),
        (r#""checkJs":true"#, r#""allowJs":false"#, false),
        (
            r#""checkJs":true,"allowJs":false"#,
            r#""allowJs":null"#,
            true,
        ),
        (r#""checkJs":true"#, r#""checkJs":null"#, false),
        (
            r#""allowJs":true"#,
            r#""allowJs":null,"checkJs":false"#,
            false,
        ),
    ] {
        let base = format!(r#"{{"compilerOptions":{{{base}}}}}"#);
        let settings = if own.is_empty() {
            String::new()
        } else {
            format!(",{own}")
        };
        let leaf =
            format!(r#"{{"extends":"./base.json","compilerOptions":{{"noLib":true{settings}}}}}"#);
        let fixture = Fixture::new(&[
            ("tsconfig.json", &leaf),
            ("base.json", &base),
            ("a.ts", "export {};\n"),
            ("b.js", "export {};\n"),
        ]);
        let result = Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error);
        if enabled {
            let error = result.unwrap_err();
            assert_eq!(error.kind, InputErrorKind::Unsupported, "{leaf}: {error}");
            assert!(error.message.contains("JavaScript"));
        } else {
            result.unwrap();
        }
    }
}
