//! Existing public JSON binding/navigation behavior around the mechanical
//! retained-facts expansion. Compiler expectations are pinned in
//! fixtures/types/json-types/evidence and fixtures/json-probes. Private final
//! Project tests prove retained arrays/occurrences separately; this adds no
//! public introspection API or JSON type evaluator.
mod support;

use support::{Fixture, check};
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::Project;

#[test]
fn recursive_and_duplicate_json_facts_keep_lexical_member_results() {
    for separator in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let data = [
            "\u{feff}{",
            r#""dup": {"first": 1}, "dup": {"last": true},"#,
            r#""a": 1, "\u0061": {"decoded": false},"#,
            r#""": 1, "01": "zero", "1": false, "😀": true,"#,
            r#""rows": [[1], [{"inside": null}]], "default": "ordinary""#,
            "}",
        ]
        .join(separator);
        check(&[
            ("data.json", &data),
            (
                "main.ts",
                r#"import /*@!*/data, { /*@!*/dup, /*@!*/a, "" as /*@!*/empty, "1" as /*@!*/one, /*@?*/absent } from "./data.json";
/*@!*/data./*@!*/dup./*@!*/last;
/*@!*/data./*@!*/dup./*@!*/first;
/*@!*/data./*@!*/a./*@!*/decoded;
/*@!*/data./*@!*/rows;
/*@!*/data./*@!*/default;
export const /*@ordinary:de*/ordinary = 1;
/*@ordinary:u*/ordinary;
"#,
            ),
        ]);
    }
}

#[test]
fn json_inputs_keep_type_script_navigation_and_json_query_boundary() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"noLib":true,"resolveJsonModule":true},"files":["main.ts","data.json"]}"#,
        ),
        ("data.json", r#"{"items":[{"x":1}],"items":[false]}"#),
        (
            "main.ts",
            "import data from './data.json';\nexport const ordinary = 1;\n",
        ),
    ]);
    let project = fixture.load();
    let outline = project.outline(&fixture.path("main.ts")).unwrap();
    assert_eq!(outline.file, "main.ts");
    assert_eq!(
        outline
            .entries
            .iter()
            .map(|entry| entry.label.as_str())
            .collect::<Vec<_>>(),
        ["data", "ordinary"]
    );
    let error = project.outline(&fixture.path("data.json")).unwrap_err();
    assert!(
        matches!(error, typescript_parser::analysis::AnalysisError::Input(error) if error.kind == InputErrorKind::Unsupported),
        "JSON remains outside file-navigation grammar"
    );
}

#[test]
fn invalid_json_still_fails_project_loading() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"noLib":true,"resolveJsonModule":true},"files":["main.ts"]}"#,
        ),
        ("main.ts", "import data from './data.json';"),
        ("data.json", r#"{"values":[1, function () {}]}"#),
    ]);
    let error = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Syntax);
}
