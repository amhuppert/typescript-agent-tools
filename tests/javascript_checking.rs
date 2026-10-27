//! Which binding and checker diagnostics of a JavaScript file fail loading,
//! by `checkJs` (unset, false, true) and the file's `// @ts-check` or
//! `// @ts-nocheck` pragma: TypeScript's `canIncludeBindAndCheckDiagnostics`,
//! `IsCheckJSEnabledForFile`, and the plain JavaScript filter by diagnostic
//! code (`plainJSErrors`), which comment directives do not reach, and
//! `reportMergeSymbolError`, which reports nothing at a plain JavaScript
//! file's declarations. The expected outcome of each combination is whether
//! native 7.0.2 and 5.6.3 report any error (`tsc --pretty false
//! --incremental false`; both agree on every one).

mod support;

use support::Dialect::{TypeScript7, TypeScript56};
use support::{DIALECTS, Fixture};
use typescript_parser::analysis::InputErrorKind;

/// One construct, its JavaScript files, and the kind of error it is.
struct Case {
    name: &'static str,
    files: &'static [(&'static str, &'static str)],
    kind: InputErrorKind,
}

const CASES: &[Case] = &[
    // TS2451, which plain JavaScript keeps.
    Case {
        name: "letlet",
        files: &[("src/a.js", "let x = 1;\nlet x = 2;\nexport {};\n")],
        kind: InputErrorKind::Binding,
    },
    // TS2300, which it drops.
    Case {
        name: "varlet",
        files: &[("src/a.js", "var y = 1;\nlet y = 2;\nexport {};\n")],
        kind: InputErrorKind::Binding,
    },
    // TS2307, which it drops.
    Case {
        name: "missing",
        files: &[(
            "src/a.js",
            "import { m } from \"./nowhere.js\";\nexport const n = m;\n",
        )],
        kind: InputErrorKind::UnresolvedImport,
    },
    // TS2451 under `@ts-ignore`, which only a checked file honors.
    Case {
        name: "covered",
        files: &[(
            "src/a.js",
            "// @ts-ignore\nlet r = 1;\n// @ts-ignore\nlet r = 2;\nexport {};\n",
        )],
        kind: InputErrorKind::Binding,
    },
    // TS2451 between two scripts' globals, a merge plain JavaScript skips.
    Case {
        name: "globals",
        files: &[("src/a.js", "let g = 1;\n"), ("src/b.js", "let g = 2;\n")],
        kind: InputErrorKind::Binding,
    },
    // TS2528, which plain JavaScript keeps.
    Case {
        name: "defaults",
        files: &[("src/a.js", "export default 1;\nexport default 2;\n")],
        kind: InputErrorKind::Binding,
    },
];

/// Whether native 7.0.2 and 5.6.3 report an error for `case` under
/// `check_js` and `pragma`.
fn expected(case: &str, check_js: Option<bool>, pragma: Option<bool>) -> bool {
    if pragma == Some(false) {
        return false;
    }
    let checked = pragma.unwrap_or(check_js == Some(true));
    let plain = pragma.is_none() && check_js.is_none();
    if plain {
        return matches!(case, "letlet" | "covered" | "defaults");
    }
    checked && case != "covered"
}

fn config(check_js: Option<bool>) -> String {
    let setting = check_js.map_or(String::new(), |value| format!(r#""checkJs": {value},"#));
    format!(
        r#"{{ "compilerOptions": {{ "allowJs": true, {setting} "strict": true, "noEmit": true, "noLib": true, "types": [], "target": "es2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true }}, "include": ["src"] }}"#
    )
}

#[test]
fn check_js_and_its_pragmas_decide_which_javascript_diagnostics_fail_loading() {
    for dialect in DIALECTS {
        for case in CASES {
            for check_js in [None, Some(false), Some(true)] {
                for pragma in [None, Some(true), Some(false)] {
                    let prefix = match pragma {
                        None => "",
                        Some(true) => "// @ts-check\n",
                        Some(false) => "// @ts-nocheck\n",
                    };
                    let config = config(check_js);
                    let texts: Vec<(String, String)> = case
                        .files
                        .iter()
                        .map(|(path, text)| ((*path).to_owned(), format!("{prefix}{text}")))
                        .collect();
                    let mut files: Vec<(&str, &str)> = texts
                        .iter()
                        .map(|(path, text)| (path.as_str(), text.as_str()))
                        .collect();
                    files.push(("tsconfig.json", &config));
                    let fixture = Fixture::with_dialect(dialect, &files);
                    let context = format!(
                        "{dialect:?} {} checkJs {check_js:?} pragma {pragma:?}",
                        case.name
                    );
                    let result = fixture.try_load();
                    if expected(case.name, check_js, pragma) {
                        let error = result.err().unwrap_or_else(|| panic!("{context}: loaded"));
                        assert_eq!(error.kind, case.kind, "{context}: {error}");
                        let path = error.path.clone().unwrap();
                        assert!(
                            case.files
                                .iter()
                                .any(|(file, _)| path == fixture.path(file)),
                            "{context}: {error}"
                        );
                    } else if let Err(error) = result {
                        panic!("{context}: {error}");
                    }
                }
            }
        }
    }
}

/// A TypeScript file's side of a merge with a plain JavaScript file's
/// declarations is reported, and the JavaScript side is not
/// (`reportMergeSymbolError`): both compilers report TS2451 at `a.ts` only,
/// and at both once `checkJs` is true.
#[test]
fn a_merge_conflict_with_plain_javascript_is_reported_on_the_typescript_side() {
    for check_js in [None, Some(true)] {
        let config = config(check_js);
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            ("src/a.ts", "let g = 1;\n"),
            ("src/s2.js", "let g = 2;\n"),
        ]);
        let error = fixture.try_load().unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Binding, "{error}");
        let path = error.path.unwrap();
        if check_js.is_none() {
            assert_eq!(path, fixture.path("src/a.ts"));
        } else {
            assert!(path == fixture.path("src/a.ts") || path == fixture.path("src/s2.js"));
        }
    }
}

/// A parameter decorator without `experimentalDecorators` is TS1206 in a
/// JavaScript file whether or not its checker checks it (both compilers),
/// and nothing with the option.
#[test]
fn a_javascript_parameter_decorator_needs_experimental_decorators() {
    let text = "function dec() {}\nexport class A {\n  m(@dec x) { return x; }\n}\n";
    for (check_js, experimental, fails) in [
        (None, false, true),
        (Some(true), false, true),
        (Some(false), false, true),
        (None, true, false),
    ] {
        let mut config = config(check_js);
        if experimental {
            config = config.replace(r#""noEmit""#, r#""experimentalDecorators": true, "noEmit""#);
        }
        let fixture = Fixture::new(&[("tsconfig.json", &config), ("src/a.js", text)]);
        let result = fixture.try_load();
        if !fails {
            result.unwrap_or_else(|error| panic!("{config}: {error}"));
            continue;
        }
        let error = result.unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Syntax, "{config}: {error}");
        assert_eq!(error.message, "Decorators are not valid here.");
        let position = error.position.unwrap();
        assert_eq!((position.line, position.column), (3, 5), "{config}");
    }
}

/// Every parameter's first decorator is a candidate for TS1206. Under
/// TypeScript 7, a file the checker checks reports the checker's TS1206 at
/// the first candidate's token that no `@ts-ignore` covers, and an
/// unchecked file the first candidate syntactically, from the decorator's
/// full start (`getAdditionalJSSyntacticDiagnostics`, `decorator.Loc`);
/// 5.6.3 reports the first candidate syntactically at its token, however the
/// file is checked (`tsc --pretty false --incremental false` of each).
#[test]
fn the_first_reported_parameter_decorator_fails_loading_by_the_dialects_rule() {
    let suppressed_first = "function dec() {}\nexport class A {\n  // @ts-ignore\n  m(@dec x) { return x; }\n  n(@dec y) { return y; }\n}\n";
    let after_trivia = "function dec() {}\nexport class A {\n  m(\n    // @ts-ignore\n    @dec\n    @dec x) { return x; }\n}\n";
    // Each case: text, `checkJs`, `// @ts-check`, and where TypeScript 7
    // and 5.6 fail, if they do.
    type At = Option<(usize, usize)>;
    let cases: [(&str, Option<bool>, bool, At, At); 9] = [
        (
            suppressed_first,
            Some(true),
            false,
            Some((5, 5)),
            Some((4, 5)),
        ),
        (suppressed_first, None, true, Some((6, 5)), Some((5, 5))),
        (
            suppressed_first,
            Some(false),
            true,
            Some((6, 5)),
            Some((5, 5)),
        ),
        (
            suppressed_first,
            Some(false),
            false,
            Some((4, 5)),
            Some((4, 5)),
        ),
        (suppressed_first, None, false, Some((4, 5)), Some((4, 5))),
        (after_trivia, Some(true), false, None, Some((5, 5))),
        (after_trivia, None, true, None, Some((6, 5))),
        (after_trivia, Some(false), false, Some((3, 5)), Some((5, 5))),
        (after_trivia, None, false, Some((3, 5)), Some((5, 5))),
    ];
    for (text, check_js, pragma, seven, five) in cases {
        for (dialect, expected) in [(TypeScript7, seven), (TypeScript56, five)] {
            let text = if pragma {
                format!("// @ts-check\n{text}")
            } else {
                text.to_owned()
            };
            let config = config(check_js);
            let fixture =
                Fixture::with_dialect(dialect, &[("tsconfig.json", &config), ("src/a.js", &text)]);
            let context = format!("{dialect:?} checkJs {check_js:?} pragma {pragma} {text:?}");
            let result = fixture.try_load();
            match expected {
                None => {
                    result.unwrap_or_else(|error| panic!("{context}: {error}"));
                }
                Some(at) => {
                    let error = result.err().unwrap_or_else(|| panic!("{context}: loaded"));
                    assert_eq!(error.message, "Decorators are not valid here.", "{context}");
                    let position = error.position.unwrap();
                    assert_eq!((position.line, position.column), at, "{context}");
                }
            }
        }
    }
}

/// A syntactic TS1206 sorts among the file's other syntactic errors: before
/// a later type annotation's, as both compilers report it first.
#[test]
fn a_syntactic_decorator_error_before_typescript_only_syntax_is_reported_first() {
    let text = "function dec() {}\nexport class A {\n  m(@dec x) { return x; }\n}\nexport let y: number = 1;\n";
    for dialect in DIALECTS {
        let config = config(None);
        let fixture =
            Fixture::with_dialect(dialect, &[("tsconfig.json", &config), ("src/a.js", text)]);
        let error = fixture.try_load().unwrap_err();
        assert_eq!(
            error.message, "Decorators are not valid here.",
            "{dialect:?}"
        );
        let position = error.position.unwrap();
        assert_eq!((position.line, position.column), (3, 5), "{dialect:?}");
    }
}
