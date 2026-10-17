//! Alias-only lookup keeps raw selection separate from the type/navigation target.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{
    TypeAnswer, TypeKey, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};
const SOURCE: &str = include_str!("../fixtures/types/native-qualified-aliases/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-qualified-aliases/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-qualified-aliases/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-qualified-aliases/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-qualified-aliases/exact.json"),
        ),
        (
            "target.ts",
            include_str!("../fixtures/types/native-qualified-aliases/target.ts"),
        ),
        (
            "named.ts",
            include_str!("../fixtures/types/native-qualified-aliases/named.ts"),
        ),
        (
            "alias-route.ts",
            include_str!("../fixtures/types/native-qualified-aliases/alias-route.ts"),
        ),
        (
            "direct-route.ts",
            include_str!("../fixtures/types/native-qualified-aliases/direct-route.ts"),
        ),
        (
            "alias-first.ts",
            include_str!("../fixtures/types/native-qualified-aliases/alias-first.ts"),
        ),
        (
            "direct-first.ts",
            include_str!("../fixtures/types/native-qualified-aliases/direct-first.ts"),
        ),
        (
            "cycle-a.ts",
            include_str!("../fixtures/types/native-qualified-aliases/cycle-a.ts"),
        ),
        (
            "cycle-b.ts",
            include_str!("../fixtures/types/native-qualified-aliases/cycle-b.ts"),
        ),
        (
            "export-equals.ts",
            include_str!("../fixtures/types/native-qualified-aliases/export-equals.ts"),
        ),
        (
            "node_modules/dependency/package.json",
            include_str!(
                "../fixtures/types/native-qualified-aliases/node_modules/dependency/package.json"
            ),
        ),
        (
            "node_modules/dependency/index.d.ts",
            include_str!(
                "../fixtures/types/native-qualified-aliases/node_modules/dependency/index.d.ts"
            ),
        ),
        (
            "node_modules/dependency/target.d.ts",
            include_str!(
                "../fixtures/types/native-qualified-aliases/node_modules/dependency/target.d.ts"
            ),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 1,
    }
}
#[test]
fn qualified_exports_and_shadowed_aliases_preserve_native_summary_and_actual_properties() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary, property, value) in [
            ("direct", "Original<\"direct\">", "value", "\"direct\""),
            ("named", "Original<\"named\">", "value", "\"named\""),
            (
                "typeOnly",
                "Original<\"typeOnly\">",
                "value",
                "\"typeOnly\"",
            ),
            ("nested", "Original<\"nested\">", "value", "\"nested\""),
            ("aliasFirst", "Original<\"first\">", "value", "\"first\""),
            ("directFirst", "Original<\"first\">", "value", "\"first\""),
            ("cycle", "Original<\"cycle\">", "value", "\"cycle\""),
            ("equals", "Original<\"equals\">", "value", "\"equals\""),
            ("own", "Own<\"own\">", "own", "\"own\""),
            ("localAlias", "Original<\"alias\">", "value", "\"alias\""),
            // The TYPE question still selects the inner alias's object, while
            // the independent raw Alias question supplies its serialized name.
            ("shadow", "Original<\"shadow\">", "inner", "\"shadow\""),
            (
                "dependency",
                "External<\"dependency\">",
                "inner",
                "\"dependency\"",
            ),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}/{expand}: {result:?}"
                );
                assert_eq!(
                    result.description.summary, summary,
                    "{config}/{marker}/{expand}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{marker}/{expand}: {:?}",
                    result.issues
                );
                if !expand {
                    continue;
                }
                let graph = result.description.expansion.unwrap();
                let root = graph
                    .nodes
                    .iter()
                    .find(|node| node.id == graph.root)
                    .unwrap();
                let TypeNodeData::Object { members } = &root.data else {
                    panic!("{config}/{marker}: {root:?}");
                };
                let properties = members.properties.as_ref().unwrap();
                assert_eq!(properties.len(), 1, "{config}/{marker}");
                assert_eq!(
                    properties[0].key,
                    TypeKey::String {
                        name: property.into()
                    },
                    "{config}/{marker}"
                );
                let actual = match &properties[0].read_type {
                    TypeRef::Ref { r#ref } => graph
                        .nodes
                        .iter()
                        .find(|node| node.id == *r#ref)
                        .unwrap()
                        .summary
                        .as_str(),
                    TypeRef::Omitted { summary, .. } => summary.as_str(),
                };
                assert_eq!(actual, value, "{config}/{marker}");
            }
        }
    }
}
#[test]
fn unresolved_outer_alias_does_not_invalidate_the_native_local_type() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, "missing", expand));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{config}/{expand}: {answer:?}");
            };
            // Alias-only display lookup is quiet; the local type remains valid.
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, "MissingShadow<\"missing\">");
            assert!(result.issues.is_empty());
            let healthy = project.type_of(request(&fixture, "shadow", expand));
            assert!(
                matches!(&healthy, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == "Original<\"shadow\">"),
                "{config}/{expand}: {healthy:?}"
            );
        }
    }
}
