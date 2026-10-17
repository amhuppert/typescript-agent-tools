//! Pinned native indexed-access outcomes through the public Project API.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-indexed-access/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-indexed-access/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-indexed-access/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-indexed-access/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-indexed-access/exact.json"),
        ),
        (
            "unchecked.json",
            include_str!("../fixtures/types/native-indexed-access/unchecked.json"),
        ),
    ])
}
fn request(fixture: &Fixture, name: &str) -> TypeRequest {
    let tag = format!("/*#{name}*/");
    let mut offset = match SOURCE.find(&tag) {
        Some(start) => start + tag.len(),
        None => SOURCE.find(&format!("const {name}:")).unwrap() + 6,
    };
    if ["expression", "genericKey"].contains(&name) {
        offset += SOURCE[offset..].find(']').unwrap();
    }
    TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, offset),
    })
}
fn check(rows: &[(&str, &str)]) {
    let fixture = fixture();
    for config in [
        "tsconfig.json",
        "loose.json",
        "exact.json",
        "unchecked.json",
    ] {
        let project = fixture.load_config(config);
        for &(name, expected) in rows {
            let expected = if name == "optional" && config == "loose.json" {
                "boolean"
            } else {
                expected
            };
            for expand in [false, true] {
                let mut request = request(&fixture, name);
                request.expand = expand;
                if ["genericKey", "unionGeneric", "constrainedIndex"].contains(&name) {
                    // Box has an intentionally invalid child. Operator metadata
                    // is healthy; deeper child demand is tested separately.
                    request.depth = 1;
                }
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}/expand={expand}: {answer:?}");
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
fn symbolic_operands_consume_invalid_properties_only_at_their_admitted_depth() {
    let fixture = fixture();
    for config in [
        "tsconfig.json",
        "loose.json",
        "exact.json",
        "unchecked.json",
    ] {
        let project = fixture.load_config(config);
        for (name, depth) in [
            ("genericKey", 2),
            ("constrainedIndex", 2),
            ("unionGeneric", 3),
        ] {
            let mut query = request(&fixture, name);
            query.expand = true;
            query.depth = depth;
            let answer = project.type_of(query);
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{config}/{name}/depth={depth}: {answer:?}");
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
            assert_eq!(
                error.position,
                Some(position(SOURCE, SOURCE.find("Missing").unwrap()))
            );
        }
    }
}
#[test]
fn named_optional_union_and_unused_properties_have_native_types() {
    check(&[
        ("plain", "number"),
        ("optional", "boolean | undefined"),
        ("union", "string | number"),
        ("unused", "number"),
    ]);
}
#[test]
fn index_signatures_do_not_add_unchecked_undefined_in_type_positions() {
    check(&[
        ("indexed", "number"),
        ("numbered", "string"),
        ("readonlyIndex", "number"),
        ("indexedUnion", "string | number"),
        ("indexedIntersection", "never"),
    ]);
}
#[test]
fn any_and_never_follow_actual_index_operations() {
    check(&[
        ("neverKey", "never"),
        ("anyObject", "any"),
        ("neverObject", "never"),
    ]);
}
#[test]
fn generic_type_and_expression_access_keep_distinct_deferral_rules() {
    check(&[
        ("instantiated", "42"),
        ("generic", "T[\"p\"]"),
        ("expression", "number"),
        ("genericKey", "Box[K]"),
        ("unionGeneric", "T[\"p\" | \"q\"]"),
        ("constrainedIndex", "Box[K]"),
    ]);
}
#[test]
fn consumed_invalid_operands_keep_the_original_error() {
    let fixture = fixture();
    for config in [
        "tsconfig.json",
        "loose.json",
        "exact.json",
        "unchecked.json",
    ] {
        let project = fixture.load_config(config);
        for (name, reason) in [
            ("absent", SemanticFailureReason::ErrorType),
            ("consumed", SemanticFailureReason::UnresolvedSymbol),
            ("unknownObject", SemanticFailureReason::ErrorType),
            ("invalidKey", SemanticFailureReason::ErrorType),
        ] {
            let answer = project.type_of(request(&fixture, name));
            assert!(
                matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==reason.into()),
                "{config}/{name}: {answer:?}"
            );
        }
    }
}

#[test]
fn symbolic_index_operands_are_ordered_and_depth_limited() {
    use typescript_parser::types::{TypeNodeData, TypeOperator, TypeRef};
    let fixture = fixture();
    let project = fixture.load();
    for depth in [0, 1, 2] {
        let mut query = request(&fixture, "generic");
        query.expand = true;
        query.depth = depth;
        let TypeAnswer::Selected(answer) = project.type_of(query).unwrap() else {
            panic!()
        };
        let graph = answer.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        let TypeNodeData::Operator {
            operator: TypeOperator::IndexedAccess,
            operands,
            parameters,
            ..
        } = &root.data
        else {
            panic!("{root:?}")
        };
        assert!(parameters.is_empty());
        if depth == 0 {
            assert!(operands.is_empty());
            assert_eq!(graph.nodes.len(), 1);
            assert!(graph.truncated);
            assert!(
                root.omitted
                    .as_ref()
                    .unwrap()
                    .fields
                    .iter()
                    .any(|field| field == "operands")
            );
        } else {
            assert_eq!(operands.len(), 2);
            let children: Vec<_> = operands
                .iter()
                .map(|edge| {
                    let TypeRef::Ref { r#ref } = edge else {
                        panic!("{edge:?}")
                    };
                    graph.nodes.iter().find(|node| node.id == *r#ref).unwrap()
                })
                .collect();
            assert!(matches!(&children[0].data,TypeNodeData::TypeParameter{name,..} if name=="T"));
            assert!(matches!(&children[1].data,TypeNodeData::Literal{value,..} if value=="p"));
        }
    }
}

#[test]
fn unary_operators_use_indexed_operand_flags_without_flattening_its_constraint() {
    check(&[
        ("negativeMixed", "number | bigint"),
        ("tildeMixed", "number | bigint"),
        ("negativeBig", "bigint"),
        ("tildeBig", "bigint"),
        ("negativeNumber", "number"),
        ("tildeNumber", "number"),
        ("negativeGeneric", "number"),
        ("tildeGeneric", "number"),
    ]);
}
