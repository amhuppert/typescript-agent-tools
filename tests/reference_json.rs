//! JSON program inputs and TypeScript JSON text, end to end: what loading a
//! project with JSON modules or JSON configuration produces, and the outcome
//! of queries at JSON import sites. Expectations come from TypeScript 7.0.2
//! through its API, recorded in `fixtures/json-probes` (cited by case name):
//! a JSON module's only export is `export=`, its value, so default,
//! namespace, `import =`, and `{ default as x }` imports name that value and
//! a named import names a property of the value's type, a key of an object
//! or, with libraries, a member of a string's, number's, boolean's, or
//! array's library interface (`fixtures/references/json-library-members`).
//! The compiler's oracle classifies both as members, which are not reference
//! targets here (`UnsupportedTarget`), and a name the type lacks is
//! unresolved.

mod support;

use support::{Fixture, check, position, project};
use typescript_parser::analysis::{InputError, InputErrorKind};
use typescript_parser::project::Project;
use typescript_parser::references::ReferenceQuery;

const DATA: &str =
    r#"{ "greeting": "hi", "value": 1, "a-b": 2, "1": 3, "default": 4, "nested": { "x": 1 } }"#;

fn load(fixture: &Fixture) -> Result<Project, InputError> {
    Project::load(&fixture.path("tsconfig.json"))
}

#[test]
fn json_import_forms_resolve_like_the_compiler_beside_ordinary_groups() {
    check(&[
        ("data.json", DATA),
        (
            "main.ts",
            r#"import /*@!*/data from "./data.json";
import { /*@!*/greeting, /*@?*/missing } from "./data.json";
import * as /*@!*/ns from "./data.json";
import /*@!*/req = require("./data.json");
import { default as /*@!*/dflt, "default" as /*@!*/dk } from "./data.json";
import { "a-b" as /*@!*/ab, "1" as /*@!*/one } from "./data.json";
export { /*@!*/greeting as /*@!*/g2, default as /*@!*/d2, /*@?*/missing as /*@?*/m2 } from "./data.json";
export * as /*@!*/whole from "./data.json";
import { /*@?*/greeting as /*@?*/starred } from "./star";
type /*@t:d*/T = typeof import("./data.json");
type /*@u:d*/U = typeof import("./data.json")./*@!*/greeting;
type /*@w:d*/W = typeof /*@!*/data./*@!*/nested;
const /*@dyn:d*/dyn = import("./data.json");
export const /*@l:de*/local = 1;
/*@l:u*/local; /*@!*/data; /*@!*/greeting; /*@!*/ns; /*@!*/req; /*@!*/dflt; /*@!*/dk;
/*@!*/ns./*@!*/greeting; /*@!*/data./*@!*/value; /*@!*/ab; /*@!*/one;
/*@!*/data./*@!*/nested./*@!*/x; /*@dyn:u*/dyn; /*@?*/missing; /*@?*/starred;
"#,
        ),
        ("star.ts", "export * from \"./data.json\";\n"),
        (
            "other.ts",
            "import { /*@!*/g2, /*@!*/d2, /*@!*/whole, /*@l:i*/local } from \"./main\";\n/*@!*/g2; /*@!*/d2; /*@!*/whole; /*@l:u*/local;\n",
        ),
    ]);
}

#[test]
fn every_root_value_is_a_module_value_and_names_select_its_properties() {
    // Cases `array-root`, `string-root`, `number-root`, `empty-file`, and
    // `dup-keys` in `inputs.json`: the default import is the value whatever
    // the root; without libraries, as here, a named import is a property only
    // of an object root, where repeated keys are one property.
    check(&[
        ("array.json", "[1, 2]"),
        ("string.json", "\"text\""),
        ("number.json", "-1"),
        ("empty.json", "// nothing\n"),
        ("dup.json", r#"{ "a": 1, "a": "two" }"#),
        (
            "keys.json",
            "{ \"\\ud800\": 1, \"\\u{1F600}\": 2, \"plain\": 3 }",
        ),
        (
            "main.ts",
            r#"import /*@!*/arr, { /*@?*/length } from "./array.json";
import /*@!*/str, { /*@?*/length as /*@?*/size } from "./string.json";
import /*@!*/num, { /*@?*/x } from "./number.json";
import /*@!*/empty, { /*@?*/y } from "./empty.json";
import { /*@!*/a } from "./dup.json";
import { /*@!*/plain, "😀" as /*@!*/face, /*@?*/other } from "./keys.json";
"#,
        ),
    ]);
}

#[test]
fn json_imports_load_under_every_module_setting() {
    // Cases `nodenext-noattr`, `nodenext-attr`, `nodenext-cjs`,
    // `node18-explicit`, `node20-default`: TS1543 and TS1544 are checker
    // diagnostics that do not change what an import names.
    let marked = "import /*@!*/data from \"./data.json\";\nimport { /*@!*/greeting } from \"./data.json\";\nimport /*@!*/attributed from \"./data.json\" with { type: \"json\" };\n/*@!*/data; /*@!*/greeting; /*@!*/attributed;\nexport const /*@l:de*/local = 1;\n";
    for (options, esm) in [
        (r#""module": "nodenext""#, true),
        (r#""module": "nodenext""#, false),
        (r#""module": "node18", "resolveJsonModule": true"#, true),
        (r#""module": "node20""#, true),
        (r#""module": "commonjs""#, false),
        (r#""module": "preserve""#, false),
    ] {
        let config = format!(r#"{{ "compilerOptions": {{ "noLib": true, {options} }} }}"#);
        let mut files = vec![
            ("tsconfig.json", config.as_str()),
            ("data.json", DATA),
            ("main.ts", marked),
        ];
        if esm {
            files.push(("package.json", r#"{ "type": "module" }"#));
        }
        let fixture = Fixture::new(&files);
        fixture.check_markers(&fixture.load());
    }
}

#[test]
fn without_resolve_json_module_a_json_specifier_is_missing() {
    // Cases `node16-default` and `bundler-off`: TS2732 in a project file.
    for options in [r#""module": "node16""#, r#""resolveJsonModule": false"#] {
        let config = format!(r#"{{ "compilerOptions": {{ "noLib": true, {options} }} }}"#);
        let text = "import data from \"./data.json\";\ndata;\n";
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            ("data.json", DATA),
            ("main.ts", text),
        ]);
        let error = load(&fixture).unwrap_err();
        assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
        assert_eq!(error.position, Some(position(text, 17)), "{error}");
    }
}

#[test]
fn ambient_modules_keep_their_precedence_over_json_files() {
    // Cases `ambient-and-file`, `ambient-off`, `exact-ambient`: a resolved
    // JSON file wins over a pattern, which applies only to a missing module,
    // and an exact ambient name wins over a package file.
    let declarations = "declare module \"*.json\" { export const /*@p:de*/v: number; }\ndeclare module \"pkg/data.json\" { export const /*@e:de*/w: number; }\n";
    let package = [
        ("node_modules/pkg/package.json", r#"{ "name": "pkg" }"#),
        ("node_modules/pkg/data.json", r#"{ "w": 1 }"#),
    ];
    let on = project(&[
        ("decl.d.ts", declarations),
        ("data.json", r#"{ "v": 1 }"#),
        (
            "main.ts",
            "import { /*@!*/v } from \"./data.json\";\nimport { /*@e:i*/w } from \"pkg/data.json\";\n/*@!*/v; /*@e:u*/w;\n",
        ),
        package[0],
        package[1],
    ]);
    on.check_markers(&on.load());
    let off = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "resolveJsonModule": false } }"#,
        ),
        ("decl.d.ts", declarations),
        ("data.json", r#"{ "v": 1 }"#),
        (
            "main.ts",
            "import { /*@p:i*/v } from \"./data.json\";\nimport { /*@e:i*/w } from \"pkg/data.json\";\n/*@p:u*/v; /*@e:u*/w;\n",
        ),
        package[0],
        package[1],
    ]);
    off.check_markers(&off.load());
}

#[test]
fn a_json_dependency_never_prevents_loading() {
    // Case `dep-json`: a package's JSON is an external library input.
    let fixture = project(&[
        (
            "node_modules/pkg/package.json",
            r#"{ "name": "pkg", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/pkg/index.d.ts",
            "import data from \"./data.json\";\nexport declare const x: typeof data;\nexport * from \"./data.json\";\n",
        ),
        ("node_modules/pkg/data.json", DATA),
        (
            "main.ts",
            "import { /*@x:i*/x, /*@?*/greeting } from \"pkg\";\n/*@x:u*/x; /*@?*/greeting;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

/// Load a project whose `main.ts` imports `data.json` holding `text`.
fn load_data(text: &str) -> Result<Project, InputError> {
    let fixture = project(&[
        ("main.ts", "import data from \"./data.json\";\ndata;\n"),
        ("data.json", text),
    ]);
    load(&fixture)
}

/// Each case's verdict is TypeScript 7.0.2's syntactic diagnostics for the
/// JSON module (`fixtures/json-probes/parsing.json`).
#[test]
fn json_modules_follow_the_compilers_grammar() {
    for text in [
        "// c\n{ /* x */ \"a\": [1, 2,], }",
        "\u{feff}{\"a\":1}",
        "#!x\n{\"a\":1}",
        "\u{feff}#!x\n{\"a\":1}",
        "",
        "  \n",
        "// c",
        "{\"a\": 0x10, \"b\": 0b1, \"c\": 0o7, \"d\": 1_000, \"e\": .5, \"f\": 5., \"g\": -0x1, \"h\": 1e3}",
        "{\"a\": \"\\0\\u{1F600}\\x41\\v\\'\\q\"}",
        "{\"\\ud800\": 1, \"\\udc00\": 2}",
        "{\"a\": \"x\\\ny\", \"b\": \"x\u{2028}y\"}",
        "[1, 2]",
        "\"x\"",
        "1",
        "-1",
        "true",
        "null",
        "{\"a\"?: 1, \"b\"!: 2, public \"c\": 3}",
        "{\u{a0}\"a\":\u{feff}1}",
        "{\"a\": 1, \"a\": \"two\"}",
    ] {
        load_data(text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
    }
    for line_break in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let text = format!("{{ // c{line_break}\"a\": 1 }}");
        load_data(&text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
    }
    // The first diagnostic and where TypeScript reports it.
    for (text, at) in [
        ("{\"a\": 'x'}", 6),
        ("{'a': 1}", 1),
        ("{a: 1}", 1),
        ("{a}", 1),
        ("{\"a\"() {}}", 1),
        ("{...x}", 1),
        ("{\"a\": +1}", 6),
        ("{\"a\": NaN}", 6),
        ("{\"a\": undefined}", 6),
        ("{\"a\": 1n}", 6),
        ("{\"a\": 01}", 6),
        ("{\"a\": 08}", 6),
        ("{\"a\": \"\\01\"}", 6),
        ("{\"a\": \"\\8\"}", 6),
        ("{\"a\": \"x\ny\"}", 6),
        ("{\"a\": 1", 7),
        ("{} {}", 3),
        ("'x'", 0),
        ("\"a\": 1", 0),
        ("[1,,2]", 3),
        ("{@x \"a\": 1}", 1),
        ("{\"a\": 1} /* x", 9),
    ] {
        let error = load_data(text).map(|_| ()).unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Syntax, "{text:?}: {error}");
        assert!(
            error.path.as_ref().unwrap().ends_with("data.json"),
            "{error}"
        );
        assert_eq!(
            error.position,
            Some(position(text, at)),
            "{text:?}: {error}"
        );
    }
}

/// TypeScript 7.0.2 reads a JSON member's `?` or, failing that, `!`
/// (`parseObjectLiteralElement`), so a second marker is TS1005 where `:` was
/// expected: `{"a"?!:1}` at the `!` (`fixtures/json-probes/parsing.json`,
/// `postfix-*`). A configuration reads the same grammar; its own conversion
/// then rejects a lone `?` (TS8009) where a JSON module accepts it.
#[test]
fn a_json_member_takes_one_postfix_marker() {
    let config = |text: &str| {
        let fixture = Fixture::new(&[("tsconfig.json", text), ("a.ts", "export {};\n")]);
        load(&fixture).map(|_| ())
    };
    // Positions count from after a leading byte order mark, as TypeScript's
    // file reading drops it.
    let last = |text: &str, marker: char| {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        Some(position(text, text.rfind(marker).unwrap()))
    };
    for prefix in [
        "",
        "\u{feff}",
        "\n",
        "\r",
        "\r\n",
        "\u{2028}",
        "\u{2029}",
        "// c\r",
        "// c\u{2028}",
    ] {
        for member in ["\"a\": 1", "\"a\"?: 1", "\"a\"!: 1"] {
            let text = format!("{prefix}{{{prefix}{member}}}");
            load_data(&text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
        }
        for (member, marker) in [("\"a\"?!: 1", '!'), ("\"a\"!?: 1", '?')] {
            let text = format!("{prefix}{{{prefix}{member}}}");
            let error = load_data(&text).map(|_| ()).unwrap_err();
            assert_eq!(
                (error.kind, error.position),
                (InputErrorKind::Syntax, last(&text, marker)),
                "{text:?}: {error}"
            );
            let text = format!(
                "{prefix}{{\"compilerOptions\":{{{prefix}\"noLib\"{}: true}}}}",
                &member[3..member.len() - 3]
            );
            let error = config(&text).unwrap_err();
            assert_eq!(
                (error.kind, error.position),
                (InputErrorKind::Config, last(&text, marker)),
                "{text:?}: {error}"
            );
        }
        let text = format!("{prefix}{{\"compilerOptions\":{{{prefix}\"noLib\"?: true}}}}");
        let error = config(&text).unwrap_err();
        assert_eq!(
            (error.kind, error.position, error.message.as_str()),
            (
                InputErrorKind::Config,
                last(&text, '?'),
                "The '?' modifier can only be used in TypeScript files"
            ),
            "{text:?}"
        );
        let text = format!("{prefix}{{\"compilerOptions\":{{{prefix}\"noLib\"!: true}}}}");
        config(&text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
    }
}

#[test]
fn json_nesting_past_the_limit_is_unsupported_and_flat_values_load() {
    let nested = |depth: usize| format!("{}{}", "[".repeat(depth), "]".repeat(depth));
    for depth in [127, 128] {
        load_data(&nested(depth)).unwrap_or_else(|error| panic!("{depth}: {error}"));
    }
    for depth in [129, 10_000] {
        let error = load_data(&nested(depth)).map(|_| ()).unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Unsupported, "{depth}: {error}");
        assert!(error.path.as_ref().unwrap().ends_with("data.json"));
    }
    let wide = format!("[{}]", vec!["{\"a\": [1, \"x\"]}"; 100_000].join(","));
    load_data(&wide).unwrap();
}

#[test]
fn a_comment_directive_in_a_json_file_is_still_text() {
    // Strings hide comment starts, as in TypeScript's scanner, so the `//`
    // inside a value does not begin a comment: the file still parses.
    load_data("{ \"url\": \"https://example.com/*\", \"b\": 1 /* c */ }").unwrap();
}

#[cfg(unix)]
#[test]
fn an_unreadable_json_module_is_an_io_failure() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = project(&[
        ("main.ts", "import data from \"./data.json\";\ndata;\n"),
        ("data.json", DATA),
    ]);
    let path = fixture.path("data.json");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&path).is_ok() {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        eprintln!("SKIP an_unreadable_json_module_is_an_io_failure: permissions are not enforced");
        return;
    }
    let error = load(&fixture).map(|_| ()).unwrap_err();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(error.kind, InputErrorKind::Io, "{error}");
    assert!(
        error.path.as_ref().unwrap().ends_with("data.json"),
        "{error}"
    );
}

#[test]
fn queries_inside_a_json_file_find_no_occurrences() {
    // A JSON file is searched, but its keys are string literals, not
    // identifiers, so a position there selects nothing, as at a string
    // literal in a TypeScript file, and no name occurs in it.
    let fixture = project(&[
        ("main.ts", "import data from \"./data.json\";\ndata;\n"),
        ("data.json", DATA),
    ]);
    let project = fixture.load();
    let at = project
        .find_references(ReferenceQuery::At {
            file: fixture.path("data.json"),
            position: position(DATA, 3),
        })
        .unwrap();
    assert!(at.groups.is_empty());
    let named = project
        .find_references(ReferenceQuery::Name {
            name: "greeting".into(),
            file: Some(fixture.path("data.json")),
        })
        .unwrap();
    assert!(named.groups.is_empty());
}

/// Configuration files go through the same JSON reader; their conversion
/// differs only after parsing. Verdicts from TypeScript 7.0.2's
/// configuration diagnostics (`fixtures/json-probes/configuration.json`).
#[test]
fn configuration_text_follows_the_compilers_grammar() {
    let load_config = |config: &str| {
        let fixture = Fixture::new(&[("tsconfig.json", config), ("a.ts", "export const a = 1;\n")]);
        load(&fixture).map(|_| ())
    };
    for config in [
        "#!x\n{\"compilerOptions\":{\"noLib\":true}}",
        "\u{feff}#!x\n{\"compilerOptions\":{\"noLib\":true}}",
        "{\"compilerOptions\":{\"noLib\":true,\"maxNodeModuleJsDepth\":0x1}}",
        "{\"compilerOptions\":{\"noLib\"!:true}}",
        "{\"compilerOptions\":{public \"noLib\":true}}",
        "[{\"compilerOptions\":{\"noLib\":true}}]",
        "{\"compilerOptions\":{\"noLib\":true,\"\\ud800\":1}}",
    ] {
        load_config(config).unwrap_or_else(|error| panic!("{config:?}: {error}"));
    }
    // Each error is reported where TypeScript reports its first diagnostic.
    for (config, at) in [
        (
            "{\"compilerOptions\":{\"noLib\":true,\"maxNodeModuleJsDepth\":01}}",
            "01",
        ),
        (
            "{\"compilerOptions\":{\"noLib\":true,\"jsxImportSource\":\"\\8\"}}",
            "\"\\8",
        ),
        // TS8009: the parser accepts `?`, configuration conversion does not.
        ("{\"compilerOptions\":{\"noLib\"?:true}}", "?"),
        // TS5092, unless an array holds an object.
        ("\"x\"", "\"x"),
        ("[1]", "[1"),
    ] {
        let error = load_config(config).unwrap_err();
        let at = config.find(at).unwrap();
        assert_eq!(
            (error.kind, error.position),
            (InputErrorKind::Config, Some(position(config, at))),
            "{config:?}: {error}"
        );
    }
}

#[test]
fn empty_and_trivia_only_configurations_are_empty() {
    // Case `cfg-comment-only`: no diagnostics, the default `**/*` include.
    for config in ["", "// nothing", "/* nothing */\n"] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", config),
            ("a.ts", "export const /*@a:de*/a = 1;\n"),
            ("b.ts", "import { /*@a:i*/a } from \"./a\";\n/*@a:u*/a;\n"),
        ]);
        fixture.check_markers(&fixture.load());
    }
}

#[test]
fn augmenting_a_json_module_augments_a_non_module_entity() {
    // Case `augment-json`: TypeScript reports TS2671, as for an `export =`
    // of a value, and the augmentation adds no export.
    let text = "import data from \"./data.json\";\ndeclare module \"./data.json\" { export const x: number; }\ndata;\n";
    let error = support::load_error(&[("main.ts", text), ("data.json", r#"{ "a": 1 }"#)]);
    assert_eq!(error.kind, InputErrorKind::Binding, "{error}");
    assert_eq!(
        error.position,
        Some(position(text, text.find("\"./data.json\" {").unwrap()))
    );
    // In a dependency, the failed augmentation merges nothing.
    check(&[
        (
            "node_modules/pkg/package.json",
            r#"{ "name": "pkg", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/pkg/index.d.ts",
            "export {};\ndeclare module \"../../data.json\" { export const x: number; }\n",
        ),
        ("data.json", r#"{ "a": 1 }"#),
        (
            "main.ts",
            "import \"pkg\";\nimport { /*@?*/x, /*@!*/a } from \"./data.json\";\n/*@?*/x; /*@!*/a;\n",
        ),
    ]);
}
