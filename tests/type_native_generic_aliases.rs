//! Native alias construction, instantiation and recovery are final queries.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-generic-aliases/cases.ts");
const IMPORTED: &str = include_str!("../fixtures/types/native-generic-aliases/imported.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        ("imported.ts", IMPORTED),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-generic-aliases/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-generic-aliases/loose.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool, depth: usize) -> TypeRequest {
    let (file, source) = if matches!(marker, "imported" | "nestedImported") {
        ("imported.ts", IMPORTED)
    } else {
        ("cases.ts", SOURCE)
    };
    let tag = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path(file),
        position: position(source, source.find(&tag).unwrap() + tag.len()),
    });
    request.expand = expand;
    request.depth = depth;
    request
}

#[test]
fn alias_arguments_defaults_forwarding_and_imported_identity_match_native() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("scalar", "\"value\""),
            ("choice", "Choice<\"a\"> | Choice<\"z\">"),
            ("pair", "Pair<\"left\", 42>"),
            ("defaulted", "Defaulted<string, string>"),
            ("dependent", "Defaulted<\"value\", \"value\">"),
            ("ignoredSupplied", "number"),
            ("constraint", "42"),
            ("forward", "Forward<\"value\">"),
            ("recursive", "Recursive<string>"),
            ("imported", "Exported<\"value\">"),
            ("nestedImported", "Exported<Exported<Exported<string>>>"),
        ] {
            for expand in [false, true, false, true] {
                let answer = project.type_of(request(&fixture, marker, expand, 1));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}: {answer:?}")
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, summary, "{config}/{marker}");
                assert!(result.issues.is_empty(), "{marker}: {:?}", result.issues);
            }
        }
    }
}

#[test]
fn alias_property_values_use_the_same_canonical_mapper_after_lazy_inventory() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, expected) in [
            ("pair", vec!["\"left\"", "42"]),
            ("defaulted", vec!["string", "string"]),
            ("dependent", vec!["\"value\"", "\"value\""]),
            ("imported", vec!["\"value\""]),
            (
                "recursive",
                vec![
                    "string",
                    if config == "tsconfig.json" {
                        "Recursive<string> | undefined"
                    } else {
                        // Named Q4 graph labels qualify the actual alias;
                        // the top-level compiler spelling stays unchanged.
                        "Recursive<string> (cases.ts:8:6)"
                    },
                ],
            ),
        ] {
            let answer = project.type_of(request(&fixture, marker, true, 1));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{config}/{marker}: {answer:?}")
            };
            let graph = result.description.expansion.unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::Object { members } = &root.data else {
                panic!("{marker}: {root:?}")
            };
            let actual: Vec<_> = members
                .properties
                .as_ref()
                .expect("admitted object properties")
                .iter()
                .map(|property| match &property.read_type {
                    TypeRef::Ref { r#ref } => graph
                        .nodes
                        .iter()
                        .find(|node| node.id == *r#ref)
                        .unwrap()
                        .summary
                        .as_str(),
                    TypeRef::Omitted { summary, .. } => summary.as_str(),
                })
                .collect();
            assert_eq!(actual, expected, "{config}/{marker}");
        }
    }
}

#[test]
fn alias_count_and_circular_recovery_remain_invalid_and_do_not_poison_other_queries() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for marker in ["tooFew", "tooMany", "circular", "ignoredDefault"] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand, 1));
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic),
                    "{config}/{marker}: {answer:?}"
                );
                let valid = project.type_of(request(&fixture, "scalar", expand, 1));
                assert!(
                    matches!(valid, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == "\"value\"")
                );
            }
        }
    }
}

#[test]
fn alias_root_skips_invalid_property_until_expansion_consumes_its_value() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, "lazyMissing", expand, 0));
            assert!(
                matches!(answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == "MissingProperty<number>")
            );
        }
        let answer = project.type_of(request(&fixture, "lazyMissing", true, 1));
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic),
            "{config}: {answer:?}"
        );
    }
}
