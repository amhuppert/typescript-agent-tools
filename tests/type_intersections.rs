//! Native intersection normalization is observed through the final Project.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-algebra/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-algebra/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-algebra/loose.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str, expand: bool, depth: usize) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    });
    request.expand = expand;
    request.depth = depth;
    request
}

#[test]
fn primitive_and_unit_intersections_return_the_actual_normalized_type() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, expected) in [
            ("intersectionSame", "string"),
            ("intersectionDisjoint", "never"),
            ("intersectionUnknown", "string"),
            ("intersectionAny", "any"),
            ("intersectionLiteral", "\"ready\""),
            ("intersectionUnits", "never"),
            ("intersectionBoolean", "never"),
            ("intersectionEmpty", "{}"),
        ] {
            for (expand, depth) in [(false, 0), (true, 0), (true, 2)] {
                let answer = project.type_of(request(&fixture, marker, expand, depth));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!(
                        "native scalar intersection must normalize ({config}/{marker}): {answer:?}"
                    );
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(
                    result.description.summary, expected,
                    "{config}/{marker}/{depth}"
                );
                assert!(result.issues.is_empty());
                if let Some(graph) = result.description.expansion {
                    assert!(!matches!(
                        graph.nodes[0].data,
                        TypeNodeData::Intersection { .. }
                    ));
                }
            }
        }
    }
}

#[test]
fn nullable_intersections_and_common_union_nullables_follow_the_option_state() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let strict = config == "tsconfig.json";
        for (marker, expected) in [
            (
                "intersectionNullable",
                if strict { "never" } else { "null" },
            ),
            (
                "intersectionUndefined",
                if strict { "never" } else { "undefined" },
            ),
            (
                "intersectionNullUnions",
                if strict { "null" } else { "never" },
            ),
            (
                "intersectionUndefinedUnions",
                if strict { "undefined" } else { "never" },
            ),
        ] {
            let answer = project.type_of(request(&fixture, marker, true, 2));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!(
                    "native nullable intersection must normalize ({config}/{marker}): {answer:?}"
                );
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, expected);
            assert!(result.issues.is_empty());
        }
    }
}

#[test]
fn object_intersections_preserve_first_order_and_exact_native_deduplication() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for marker in ["intersectionOrder", "intersectionFlatten"] {
            let answer = project.type_of(request(&fixture, marker, true, 2));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!(
                    "native ordered object intersection must succeed ({config}/{marker}): {answer:?}"
                );
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, "B & A");
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::Intersection {
                alternatives,
                omitted_alternatives,
            } = &root.data
            else {
                panic!("two actual object constituents retain Intersection kind");
            };
            assert_eq!(alternatives.len(), 2);
            assert_eq!(*omitted_alternatives, None);
            // Graph labels retain the native spelling and the first actual
            // nominal declaration's qualifier; the root summary stays bare.
            for (edge, expected) in alternatives
                .iter()
                .zip(["B (cases.ts:4:11)", "A (cases.ts:3:11)"])
            {
                let TypeRef::Ref { r#ref } = edge else {
                    panic!("the admitted child is established")
                };
                let child = graph.nodes.iter().find(|node| &node.id == r#ref).unwrap();
                assert_eq!(child.summary, expected);
            }
            assert!(!graph.truncated);
        }
    }
}

#[test]
fn intersection_alias_metadata_belongs_to_the_actual_alias_symbol() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, expected) in [
            ("intersectionAlias", "IntersectionAlias"),
            ("intersectionOtherAlias", "OtherIntersectionAlias"),
        ] {
            let answer = project.type_of(request(&fixture, marker, true, 2));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!(
                    "actual intersection alias must be retained ({config}/{marker}): {answer:?}"
                );
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, expected);
            let graph = result.description.expansion.unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let alias = root.alias.as_ref().unwrap();
            assert_eq!(alias.name, expected);
            assert_eq!(alias.origins.len(), 1);
            let declaration = format!("type {expected}");
            assert_eq!(
                alias.origins[0].start,
                position(SOURCE, SOURCE.find(&declaration).unwrap() + 5)
            );
            assert!(matches!(root.data, TypeNodeData::Intersection { .. }));
        }
    }
}

#[test]
fn constrained_parameter_intersections_and_union_products_use_the_native_operation() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, expected) in [
            ("intersectionConstrainedSame", "T"),
            ("intersectionConstrainedDisjoint", "never"),
            ("intersectionConstrainedNarrow", "U & \"ready\""),
            (
                "intersectionProduct",
                "(A | { gamma: boolean; }) & (B | { delta: bigint; })",
            ),
        ] {
            let answer = project.type_of(request(&fixture, marker, true, 3));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!(
                    "native constraint/distribution operation must succeed ({config}/{marker}): {answer:?}"
                );
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, expected);
            assert!(result.issues.is_empty());
            if marker == "intersectionProduct" {
                let graph = result.description.expansion.unwrap();
                let root = graph
                    .nodes
                    .iter()
                    .find(|node| node.id == graph.root)
                    .unwrap();
                let TypeNodeData::Union {
                    alternatives,
                    omitted_alternatives,
                } = &root.data
                else {
                    panic!(
                        "native cross product is a real Union despite its denormalized printed origin"
                    );
                };
                assert_eq!(alternatives.len(), 4);
                assert_eq!(*omitted_alternatives, None);
            }
        }
    }
}

#[test]
fn an_intersection_consumes_missing_type_recovery_before_any_reduction_can_succeed() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let answer = project.type_of(request(&fixture, "intersectionMissing", false, 0));
        let path = fixture.path("cases.ts");
        let missing = position(SOURCE, SOURCE.find("MissingIntersection").unwrap());
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic
            && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
            && error.path.as_deref() == Some(path.as_path())
            && error.position == Some(missing)),
            "consumed actual recovery stays invalid: {answer:?}"
        );
    }
}

#[test]
fn intersection_normalization_skips_unresolved_inventories_and_uses_the_exact_empty_literal_identity()
 {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, expected, intersection) in [
            ("intersectionAnySkipsIndex", "any", false),
            ("intersectionNeverSkipsIndex", "never", false),
            ("intersectionAnySkipsReturn", "any", false),
            ("intersectionNeverSkipsReturn", "never", false),
            ("intersectionLiteralEmptyString", "{} & string", true),
            ("intersectionLiteralEmptyNumber", "number & {}", true),
            ("intersectionLiteralEmptyBigint", "{} & bigint", true),
            ("intersectionAliasedEmptyString", "string", false),
        ] {
            for (expand, depth) in [(false, 0), (true, 0), (true, 2)] {
                let answer = project.type_of(request(&fixture, marker, expand, depth));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!(
                        "native normalization must not demand skipped index/return children ({config}/{marker}): {answer:?}"
                    );
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, expected);
                assert!(result.issues.is_empty());
                if let Some(graph) = result.description.expansion {
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    assert_eq!(
                        matches!(root.data, TypeNodeData::Intersection { .. }),
                        intersection
                    );
                }
            }
        }
    }
}
