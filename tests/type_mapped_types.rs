//! Native mapped declarations, operators, modifier preservation and reverse
//! inference share the public Project query path in each semantic profile.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    TypeAnswer, TypeKey, TypeNodeData, TypeOperator, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const CORE: &str = include_str!("../fixtures/types/native-mapped-types/core.ts");
const HOMOMORPHIC: &str = include_str!("../fixtures/types/native-mapped-types/homomorphic.ts");
const GENERIC: &str = include_str!("../fixtures/types/native-mapped-types/generic.ts");
const INFERENCE: &str = include_str!("../fixtures/types/native-mapped-types/inference.ts");
const LAZY: &str = include_str!("../fixtures/types/native-mapped-types/lazy.ts");
const CHECKING: &str = include_str!("../fixtures/types/native-mapped-types/checking.ts");
const RELATIONS: &str = include_str!("../fixtures/types/native-mapped-types/relations.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "native-variable-patterns/globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        (
            "native-mapped-types/tsconfig.json",
            include_str!("../fixtures/types/native-mapped-types/tsconfig.json"),
        ),
        (
            "native-mapped-types/loose.json",
            include_str!("../fixtures/types/native-mapped-types/loose.json"),
        ),
        (
            "native-mapped-types/exact.json",
            include_str!("../fixtures/types/native-mapped-types/exact.json"),
        ),
        (
            "native-mapped-types/library.d.ts",
            include_str!("../fixtures/types/native-mapped-types/library.d.ts"),
        ),
        ("native-mapped-types/core.ts", CORE),
        ("native-mapped-types/homomorphic.ts", HOMOMORPHIC),
        ("native-mapped-types/generic.ts", GENERIC),
        ("native-mapped-types/inference.ts", INFERENCE),
        ("native-mapped-types/lazy.ts", LAZY),
        ("native-mapped-types/checking.ts", CHECKING),
        ("native-mapped-types/relations.ts", RELATIONS),
    ])
}
fn request(
    fixture: &Fixture,
    file: &str,
    source: &str,
    marker: &str,
    expand: bool,
    depth: usize,
) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path(&format!("native-mapped-types/{file}.ts")),
            position: position(source, source.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth,
    }
}
fn check_group(file: &str, source: &str, expected: &str) {
    let fixture = fixture();
    let rows: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-mapped-types/{config}.json"));
        for row in rows.iter().filter(|row| row["config"] == config) {
            let name = row["name"].as_str().unwrap();
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, file, source, name, expand, 1));
                if file == "inference" && name == "limited" {
                    // Native2353 rejects the sole generic candidate. Its
                    // non-error returned type is resolveCall's fallback, which
                    // the product contract classifies as consumed recovery.
                    let Err(AnalysisError::Semantic(error)) = &answer else {
                        panic!("{config}/{file}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic, "{error:?}");
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::OverloadRecovery.into(),
                        "{error:?}"
                    );
                    let call = source.find("= limited(").unwrap() + 2;
                    let path = fixture
                        .path("native-mapped-types/inference.ts")
                        .canonicalize()
                        .unwrap();
                    assert_eq!(error.path.as_deref(), Some(path.as_path()), "{error:?}");
                    assert_eq!(error.position, Some(position(source, call)), "{error:?}");
                    // A required display operation is still unfinished if its
                    // Unsupported cause survives; do not make that gap pass.
                    assert!(
                        error
                            .causes
                            .iter()
                            .all(|cause| cause.reason
                                == SemanticFailureReason::OverloadRecovery.into()),
                        "{error:?}"
                    );
                    continue;
                }
                let Ok(TypeAnswer::Selected(result)) = &answer else {
                    panic!("{config}/{file}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{file}/{name}/{expand}: {answer:?}"
                );
                assert_eq!(
                    result.description.summary,
                    row["summary"].as_str().unwrap(),
                    "{config}/{file}/{name}/{expand}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{file}/{name}/{expand}: {:?}",
                    result.issues
                );
                if expand && file == "core" {
                    let graph = result.description.expansion.as_ref().unwrap();
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    let TypeNodeData::Object { members } = &root.data else {
                        panic!("{config}/{name}: {root:?}")
                    };
                    let properties = members.properties.as_deref().unwrap_or(&[]);
                    let expected = row["properties"].as_array().unwrap();
                    assert_eq!(
                        properties.len(),
                        expected.len(),
                        "{config}/{name}: {properties:?}"
                    );
                    for (index, (property, native)) in properties.iter().zip(expected).enumerate() {
                        match &property.key {
                            TypeKey::String { name: key } => assert_eq!(
                                key,
                                native["name"].as_str().unwrap(),
                                "{config}/{name}/{index}"
                            ),
                            TypeKey::Symbol { .. } if name == "uniqueKeys" => {
                                assert_eq!(native["name"], format!("unique:{index}"))
                            }
                            other => panic!("{config}/{name}: unexpected key {other:?}"),
                        }
                        let TypeRef::Ref { r#ref } = &property.read_type else {
                            panic!("{config}/{name}: {:?}", property.read_type)
                        };
                        let child = graph.nodes.iter().find(|node| &node.id == r#ref).unwrap();
                        assert_eq!(
                            child.summary,
                            native["summary"].as_str().unwrap(),
                            "{config}/{name}/{index}"
                        );
                        let origins: Vec<_> = native["origins"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|origin| origin["location"].clone())
                            .collect();
                        assert_eq!(
                            serde_json::to_value(&property.origins).unwrap(),
                            serde_json::json!(origins),
                            "{config}/{name}/{index}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn standard_pick_omit_and_renamed_aliases_preserve_keys_and_modifiers() {
    check_group(
        "core",
        CORE,
        include_str!("../fixtures/types/native-mapped-types/expected-core.json"),
    );
    let fixture = fixture();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-mapped-types/{config}.json"));
        for (marker, readonly, optional) in [
            ("keep", true, true),
            ("addReadonly", true, true),
            ("removeReadonly", false, true),
            ("addOptional", true, true),
            ("bothAdd", true, true),
            ("mutableOptional", false, true),
            ("removeOptional", true, false),
            ("readonlyRequired", true, false),
            ("bothRemove", false, false),
        ] {
            let answer = project
                .type_of(request(&fixture, "core", CORE, marker, true, 1))
                .unwrap();
            let TypeAnswer::Selected(result) = answer else {
                panic!("{marker}: {answer:?}")
            };
            let graph = result.description.expansion.unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::Object { members } = &root.data else {
                panic!("{config}/{marker}: {root:?}")
            };
            let keep = members
                .properties
                .as_ref()
                .unwrap()
                .iter()
                .find(|property| matches!(&property.key,TypeKey::String {name} if name=="keep"))
                .unwrap();
            assert_eq!(
                (keep.readonly, keep.optional),
                (readonly, optional),
                "{config}/{marker}"
            );
        }
    }
}
#[test]
fn homomorphic_mapping_and_syntactic_remapping_follow_native_tuple_rules() {
    check_group(
        "homomorphic",
        HOMOMORPHIC,
        include_str!("../fixtures/types/native-mapped-types/expected-homomorphic.json"),
    );
}

#[test]
fn limited_key_inference_without_an_excess_property_is_healthy() {
    // The maintained two-case native matrix distinguishes successful inference
    // from the superficially identical type returned by overload recovery.
    const SOURCE: &str = include_str!("../fixtures/types/native-mapped-limited/cases.ts");
    let fixture = Fixture::new(&[
        (
            "native-variable-patterns/globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        ("native-mapped-limited/cases.ts", SOURCE),
        (
            "native-mapped-limited/tsconfig.json",
            include_str!("../fixtures/types/native-mapped-limited/tsconfig.json"),
        ),
        (
            "native-mapped-limited/loose.json",
            include_str!("../fixtures/types/native-mapped-limited/loose.json"),
        ),
        (
            "native-mapped-limited/exact.json",
            include_str!("../fixtures/types/native-mapped-limited/exact.json"),
        ),
    ]);
    let tag = "/*#healthy*/";
    for profile in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-mapped-limited/{profile}.json"));
        for expand in [false, true] {
            let answer = project.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("native-mapped-limited/cases.ts"),
                    position: position(SOURCE, SOURCE.find(tag).unwrap() + tag.len()),
                },
                expand,
                depth: 1,
            });
            let Ok(TypeAnswer::Selected(answer)) = answer else {
                panic!("{profile}/{expand}: {answer:?}");
            };
            assert_eq!(answer.outcome, TypeOutcome::Found);
            assert_eq!(answer.description.summary, "{ a: string; b: number; }");
            assert!(
                answer.issues.is_empty(),
                "{profile}/{expand}: {:?}",
                answer.issues
            );
            if !expand {
                continue;
            }
            let graph = answer.description.expansion.as_ref().unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::Object { members } = &root.data else {
                panic!("{profile}: {root:?}");
            };
            let properties = members.properties.as_ref().unwrap();
            assert_eq!(properties.len(), 2, "{profile}: {properties:?}");
            for (property, (name, summary)) in
                properties.iter().zip([("a", "string"), ("b", "number")])
            {
                assert!(matches!(&property.key, TypeKey::String { name: key } if key == name));
                assert!(!property.optional);
                let TypeRef::Ref { r#ref } = &property.read_type else {
                    panic!("{profile}: {:?}", property.read_type);
                };
                let child = graph.nodes.iter().find(|node| &node.id == r#ref).unwrap();
                assert_eq!(child.summary, summary, "{profile}/{name}");
            }
        }
    }
}

#[test]
fn generic_maps_keep_symbolic_operators_and_original_modifier_tokens() {
    check_group(
        "generic",
        GENERIC,
        include_str!("../fixtures/types/native-mapped-types/expected-generic.json"),
    );
    let fixture = fixture();
    let project = fixture.load_config("native-mapped-types/tsconfig.json");
    for marker in [
        "genericPick",
        "genericOmit",
        "genericRenamed",
        "plusModifiers",
        "minusModifiers",
        "genericName",
    ] {
        let answer = project
            .type_of(request(&fixture, "generic", GENERIC, marker, true, 0))
            .unwrap();
        let TypeAnswer::Selected(result) = answer else {
            panic!("{answer:?}")
        };
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        assert!(
            matches!(
                root.data,
                TypeNodeData::Operator {
                    operator: TypeOperator::Mapped,
                    ..
                }
            ),
            "{marker}: {root:?}"
        );
    }
}
#[test]
fn alias_display_and_graph_admission_preserve_lazy_mapped_failures() {
    let fixture = fixture();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-mapped-types/{config}.json"));
        for marker in ["excluded", "renamedExcluded", "empty", "healthy"] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, "lazy", LAZY, marker, expand, 1));
                assert!(
                    matches!(&answer,Ok(TypeAnswer::Selected(result)) if result.outcome==TypeOutcome::Found && result.issues.is_empty()),
                    "{config}/{marker}: {answer:?}"
                );
            }
        }
        for (marker, missing) in [
            ("retained", "Missing_retained"),
            ("hidden", "Missing_template"),
            ("genericHidden", "Missing_generic_template"),
        ] {
            let compact = project.type_of(request(&fixture, "lazy", LAZY, marker, false, 1));
            assert!(
                matches!(&compact,Ok(TypeAnswer::Selected(result)) if result.outcome==TypeOutcome::Found),
                "{config}/{marker}: {compact:?}"
            );
            let shallow = project.type_of(request(&fixture, "lazy", LAZY, marker, true, 0));
            if marker == "hidden" {
                // Concrete maps need their inventory to establish object kind;
                // that native factory loads its template before properties.
                assert_unresolved(&fixture, "lazy", LAZY, missing, &shallow);
            } else {
                assert!(
                    matches!(&shallow,Ok(TypeAnswer::Selected(result)) if result.outcome==TypeOutcome::Found),
                    "{config}/{marker}: {shallow:?}"
                );
            }
            for _ in 0..2 {
                let answer = project.type_of(request(&fixture, "lazy", LAZY, marker, true, 1));
                assert_unresolved(&fixture, "lazy", LAZY, missing, &answer);
            }
        }
        let compact = project.type_of(request(&fixture, "lazy", LAZY, "invalidHeader", false, 0));
        assert!(
            matches!(&compact,Ok(TypeAnswer::Selected(result)) if result.outcome==TypeOutcome::Found),
            "{compact:?}"
        );
        let graph = project.type_of(request(&fixture, "lazy", LAZY, "invalidHeader", true, 0));
        assert_unresolved(&fixture, "lazy", LAZY, "Missing_header", &graph);
    }
}
#[test]
fn mapped_source_checks_consume_constraint_name_and_template_errors() {
    let fixture = fixture();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-mapped-types/{config}.json"));
        for marker in ["checkedGood", "checkedInvalidKey", "checkedInvalidName"] {
            let answer = project.type_of(request(&fixture, "checking", CHECKING, marker, false, 0));
            assert!(
                matches!(&answer,Ok(TypeAnswer::Selected(result)) if result.outcome==TypeOutcome::Found && result.description.summary=="unknown"),
                "{config}/{marker}: {answer:?}"
            );
        }
        for (marker, missing) in [
            ("checkedName", "Missing_name"),
            ("checkedTemplate", "Missing_checked_template"),
            ("checkedConstraint", "Missing_constraint"),
            ("quickUnchecked", "Missing_unchecked"),
        ] {
            let answer = project.type_of(request(&fixture, "checking", CHECKING, marker, false, 0));
            assert_unresolved(&fixture, "checking", CHECKING, missing, &answer);
        }
    }
}
#[test]
fn reverse_inference_preserves_properties_arrays_tuples_and_actual_reads() {
    check_group(
        "inference",
        INFERENCE,
        include_str!("../fixtures/types/native-mapped-types/expected-inference.json"),
    );
}
#[test]
fn generic_mapped_relations_preserve_native_diagnostic_only_results() {
    check_group(
        "relations",
        RELATIONS,
        include_str!("../fixtures/types/native-mapped-types/expected-relations.json"),
    );
}
fn assert_unresolved(
    fixture: &Fixture,
    file: &str,
    source: &str,
    missing: &str,
    answer: &Result<TypeAnswer, AnalysisError>,
) {
    let path = fixture
        .path(&format!("native-mapped-types/{file}.ts"))
        .canonicalize()
        .unwrap();
    assert!(
        matches!(answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref()==Some(path.as_path()) && error.position==Some(position(source,source.find(missing).unwrap()))),
        "{missing}: {answer:?}"
    );
}
