//! Pinned native keyof outcomes, including actual expanded key alternatives.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};
const SOURCE: &str = include_str!("../fixtures/types/native-keyof/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-keyof/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-keyof/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-keyof/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-keyof/exact.json"),
        ),
    ])
}
fn request(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{name}*/");
    let offset = match SOURCE.find(&tag) {
        Some(start) => start + tag.len(),
        None => SOURCE.find(&format!("const {name}:")).unwrap() + 6,
    };
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, offset),
    });
    request.expand = expand;
    request
}
#[test]
fn keyof_has_native_concrete_and_deferred_results() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (name, expected) in [
            ("shape", "keyof Shape"),
            ("alias", "keyof Alias"),
            ("literal", "\"a\" | \"z\" | 4"),
            ("classKeys", "keyof Example"),
            ("stringIndex", "string | number"),
            ("numberIndex", "number | \"named\""),
            ("union", "\"common\""),
            ("intersection", "\"left\" | \"right\""),
            ("anyKeys", "string | number | symbol"),
            ("neverKeys", "string | number | symbol"),
            ("unknownKeys", "never"),
            ("objectKeys", "never"),
            ("emptyKeys", "never"),
            ("stringKeys", "\"charAt\" | \"length\""),
            ("numberKeys", "\"toFixed\""),
            ("booleanKeys", "\"valueOf\""),
            ("nullKeys", "never"),
            ("undefinedKeys", "never"),
            (
                "tupleKeys",
                "keyof [first: number, second?: string | undefined]",
            ),
            ("instantiated", "\"a\" | \"b\""),
            ("generic", "keyof T"),
            ("genericIntersection", "keyof (T & {})"),
            ("genericUnion", "never"),
        ] {
            let expected = if name == "tupleKeys" && config != "tsconfig.json" {
                "keyof [first: number, second?: string]"
            } else {
                expected
            };
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, name, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, expected, "{config}/{name}");
                assert!(
                    result.issues.is_empty(),
                    "{config}/{name}: {:?}",
                    result.issues
                );
            }
        }
    }
}
#[test]
fn expanded_keyof_preserves_public_keys_and_native_order() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (name, expected) in [
            ("shape", vec!["\"a\"", "\"broken\"", "\"z\"", "2"]),
            ("alias", vec!["\"first\"", "\"later\""]),
            ("classKeys", vec!["\"method\"", "\"visible\"", "3"]),
        ] {
            let answer = project.type_of(request(&fixture, name, true));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{config}/{name}: {answer:?}");
            };
            let graph = result.description.expansion.as_ref().unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::Union { alternatives, .. } = &root.data else {
                panic!("{config}/{name}: {:?}", root.data);
            };
            let actual: Vec<_> = alternatives
                .iter()
                .map(|reference| match reference {
                    TypeRef::Ref { r#ref } => graph
                        .nodes
                        .iter()
                        .find(|node| &node.id == r#ref)
                        .unwrap()
                        .summary
                        .as_str(),
                    TypeRef::Omitted { summary, .. } => summary.as_str(),
                })
                .collect();
            assert_eq!(actual, expected, "{config}/{name}");
        }
    }
}
#[test]
fn keyof_retains_consumed_operand_and_index_value_failures() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for name in ["invalid", "brokenIndex"] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, name, expand));
                assert!(
                    matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==SemanticFailureReason::UnresolvedSymbol.into()),
                    "{config}/{name}: {answer:?}"
                );
            }
        }
    }
}
