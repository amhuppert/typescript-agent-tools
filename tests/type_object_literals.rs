//! Cold native object-literal member, contextual, spread and grammar questions
//! reach the same final public query in each semantic profile.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::Project;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-object-literals/cases.ts");
fn check(markers: &[&str]) {
    let fixture = Fixture::new(&[
        (
            "native-variable-patterns/globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        ("native-object-literals/cases.ts", SOURCE),
        (
            "native-object-literals/tsconfig.json",
            include_str!("../fixtures/types/native-object-literals/tsconfig.json"),
        ),
        (
            "native-object-literals/loose.json",
            include_str!("../fixtures/types/native-object-literals/loose.json"),
        ),
        (
            "native-object-literals/exact.json",
            include_str!("../fixtures/types/native-object-literals/exact.json"),
        ),
    ]);
    for (config, expected) in [
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-object-literals/expected-tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-object-literals/expected-loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-object-literals/expected-exact.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("native-object-literals/{config}"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        for row in rows
            .iter()
            .filter(|row| markers.contains(&row["name"].as_str().unwrap()))
        {
            let name = row["name"].as_str().unwrap();
            assert_eq!(row["errorType"], false, "native {config}/{name}");
            let tag = format!("/*#{name}*/");
            for expand in [false, true] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("native-object-literals/cases.ts"),
                        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
                    },
                    expand,
                    depth: 1,
                });
                let Ok(TypeAnswer::Selected(result)) = &answer else {
                    panic!("{config}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{name}/{expand}: {answer:?}"
                );
                assert_eq!(
                    result.description.summary,
                    row["summary"].as_str().unwrap(),
                    "{config}/{name}/{expand}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{name}/{expand}: {:?}",
                    result.issues
                );
            }
        }
    }
}

#[test]
fn ordinary_shorthand_methods_and_accessors_keep_native_symbol_roles() {
    check(&[
        "ordinary",
        "shorthand",
        "constShorthand",
        "plainMethod",
        "getter",
        "setterOnly",
        "accessor",
        "duplicateProperty",
    ]);
}

#[test]
fn computed_names_and_index_signatures_share_actual_name_types() {
    check(&[
        "computedLiteral",
        "computedString",
        "computedNumber",
        "computedSymbol",
    ]);
}

#[test]
fn spreads_preserve_optional_readonly_setter_and_computed_index_rules() {
    check(&[
        "spreadSetter",
        "spreadOverwrite",
        "spreadOptional",
        "spreadNullable",
        "spreadConst",
        "spreadStringIndex",
        "spreadSymbolIndex",
    ]);
}

#[test]
fn contextual_members_discriminators_and_reverse_mapped_inference_share_one_object_owner() {
    check(&[
        "contextualArrow",
        "contextualMethod",
        "discriminated",
        "shorthandDiscriminated",
        "absentDiscriminant",
        "reverseMapped",
    ]);
}

#[test]
fn duplicate_accessors_and_mixed_member_grammar_keep_native_load_boundary() {
    let expected: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-object-literals/expected-grammar.json"
    ))
    .unwrap();
    for (name, source) in [
        (
            "duplicateGetter",
            include_str!("../fixtures/types/native-object-literals/duplicateGetter.ts"),
        ),
        (
            "duplicateSetter",
            include_str!("../fixtures/types/native-object-literals/duplicateSetter.ts"),
        ),
        (
            "propertyAccessor",
            include_str!("../fixtures/types/native-object-literals/propertyAccessor.ts"),
        ),
        (
            "accessorProperty",
            include_str!("../fixtures/types/native-object-literals/accessorProperty.ts"),
        ),
        (
            "methodProperty",
            include_str!("../fixtures/types/native-object-literals/methodProperty.ts"),
        ),
    ] {
        let fixture = Fixture::new(&[
            (
                "globals.d.ts",
                include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
            ),
            ("cases.ts", source),
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"strict":true,"noLib":true,"types":[],"target":"es2022"},"files":["globals.d.ts","cases.ts"]}"#,
            ),
        ]);
        let row = expected.iter().find(|row| row["name"] == name).unwrap();
        let loaded = Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error);
        if row.get("rejected").is_some() {
            assert!(
                matches!(loaded, Err(error) if error.kind == InputErrorKind::Binding),
                "{name}"
            );
            continue;
        }
        let project = loaded.unwrap_or_else(|error| panic!("{name}: {error}"));
        let tag = format!("/*#{name}*/");
        let answer = project.type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(source, source.find(&tag).unwrap() + tag.len()),
            },
            expand: false,
            depth: 0,
        });
        let Ok(TypeAnswer::Selected(result)) = &answer else {
            panic!("{name}: {answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found, "{name}: {answer:?}");
        assert_eq!(
            result.description.summary,
            row["summary"].as_str().unwrap(),
            "{name}"
        );
        assert!(result.issues.is_empty(), "{name}: {:?}", result.issues);
    }
}
