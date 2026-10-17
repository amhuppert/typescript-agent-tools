//! Native declaration/nameType facts govern display; graph keys stay exact.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn native_property_names_preserve_numeric_quote_computed_and_method_rules() {
    let source = include_str!("../fixtures/types/native-property-name-display/cases.ts");
    let fixture = Fixture::new(&[
        (
            "native-variable-patterns/globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        (
            "native-property-name-display/tsconfig.json",
            include_str!("../fixtures/types/native-property-name-display/tsconfig.json"),
        ),
        ("native-property-name-display/cases.ts", source),
    ]);
    let project = fixture.load_config("native-property-name-display/tsconfig.json");
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-property-name-display/expected.json"
    ))
    .unwrap();
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let marker = format!("/*#{name}*/");
        for expand in [false, true] {
            let answer = project.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("native-property-name-display/cases.ts"),
                    position: position(source, source.find(&marker).unwrap() + marker.len()),
                },
                expand,
                depth: 1,
            });
            let Ok(TypeAnswer::Selected(result)) = &answer else {
                panic!("{name}/{expand}: {answer:?}")
            };
            assert_eq!(
                result.outcome,
                TypeOutcome::Found,
                "{name}/{expand}: {answer:?}"
            );
            assert_eq!(
                result.description.summary,
                row["summary"].as_str().unwrap(),
                "{name}/{expand}"
            );
            assert!(
                result.issues.is_empty(),
                "{name}/{expand}: {:?}",
                result.issues
            );
        }
    }
}

#[test]
fn computed_string_key_lower_bound_elides_only_unadmitted_value_reads() {
    use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
    // Q4's bounded-summary policy can elide a whole impossible row before
    // native nonMissing/name/mode acquisition. The short twin consumes Missing.
    for length in [1, 600] {
        let key = "x".repeat(length);
        let source = format!(
            "declare const key: \"{key}\";\ndeclare const current: {{ [key]: Missing }};\ncurrent;\n"
        );
        let fixture = Fixture::new(&[
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"strict":true,"noLib":true,"types":[]},"files":["cases.ts"]}"#,
            ),
            ("cases.ts", &source),
        ]);
        let project = fixture.load_config("tsconfig.json");
        let answer = project.type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(&source, source.rfind("current;").unwrap()),
            },
            expand: false,
            depth: 0,
        });
        if length == 1 {
            assert!(
                matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==SemanticFailureReason::UnresolvedSymbol.into() && error.position==Some(position(&source,source.find("Missing").unwrap()))),
                "{answer:?}"
            );
        } else {
            let Ok(TypeAnswer::Selected(result)) = &answer else {
                panic!("{answer:?}")
            };
            assert_eq!(result.outcome, TypeOutcome::Found, "{answer:?}");
            assert_eq!(result.description.summary, "{ … 1 more … }");
            assert!(result.description.summary_truncated);
            assert!(result.issues.is_empty(), "{:?}", result.issues);
        }
    }
}
