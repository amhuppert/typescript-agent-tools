//! Adversarial final-load regressions for configuration conversion.
//! TypeScript 7.0.2's `convertJsonOption` checks the value kind before
//! interpreting string identity. A string is a wrong boolean, number,
//! list, or object even when it contains unpaired UTF-16 code units.

mod support;

use support::Fixture;
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::Project;

#[test]
fn malformed_primitive_values_are_config_errors_before_string_identity() {
    for (name, expected, valid) in [
        ("strict", "boolean", "false"),
        ("noUnusedLocals", "boolean", "false"),
        ("checkers", "number", "1"),
        ("plugins", "list", "[]"),
        ("paths", "object", "{}"),
    ] {
        for value in [
            r#""wrong""#,
            r#""\ud800""#,
            r#""\udfff""#,
            r#""\ud83d\ude00""#,
        ] {
            for overwrite in [false, true] {
                let replacement = if overwrite {
                    format!(r#", "{name}":{valid}"#)
                } else {
                    String::new()
                };
                let config = format!(
                    r#"{{"compilerOptions":{{"noLib":true,"types":[],"{name}":{value}{replacement}}},"files":["a.ts"]}}"#
                );
                let fixture = Fixture::new(&[("tsconfig.json", &config), ("a.ts", "export {};\n")]);
                let error = Project::load(&fixture.path("tsconfig.json"))
                    .map_err(support::input_error)
                    .unwrap_err();
                assert_eq!(
                    error.kind,
                    InputErrorKind::Config,
                    "{name}:{value}: {error}"
                );
                assert!(error.message.contains(expected), "{name}:{value}: {error}");
                assert_eq!(error.path, Some(fixture.path("tsconfig.json")));
                assert!(error.position.is_some(), "{name}:{value}");
            }
            let base = format!(r#"{{"compilerOptions":{{"{name}":{value}}}}}"#);
            let config = format!(
                r#"{{"extends":"./base.json","compilerOptions":{{"noLib":true,"types":[],"{name}":{valid}}},"files":["a.ts"]}}"#
            );
            let fixture = Fixture::new(&[
                ("tsconfig.json", &config),
                ("base.json", &base),
                ("a.ts", "export {};\n"),
            ]);
            let error = Project::load(&fixture.path("tsconfig.json"))
                .map_err(support::input_error)
                .unwrap_err();
            assert_eq!(
                error.kind,
                InputErrorKind::Config,
                "inherited {name}:{value}: {error}"
            );
            assert!(
                error.message.contains(expected),
                "inherited {name}:{value}: {error}"
            );
            assert_eq!(error.path, Some(fixture.path("base.json")));
            assert!(error.position.is_some(), "inherited {name}:{value}");
        }
    }
}

#[test]
fn malformed_plugin_entries_are_config_errors_before_string_identity() {
    for value in [
        r#""wrong""#,
        r#""\ud800""#,
        r#""\udfff""#,
        r#""\ud83d\ude00""#,
    ] {
        let config = format!(
            r#"{{"compilerOptions":{{"noLib":true,"plugins":[{value}]}},"files":["a.ts"]}}"#
        );
        let fixture = Fixture::new(&[("tsconfig.json", &config), ("a.ts", "export {};\n")]);
        let error = Project::load(&fixture.path("tsconfig.json"))
            .map_err(support::input_error)
            .unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Config, "{value}: {error}");
        assert!(error.message.contains("object"), "{value}: {error}");
    }
}
