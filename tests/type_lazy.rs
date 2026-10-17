use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeKey, TypeNodeData, TypeOutcome, TypeQuery, TypeQuestion,
    TypeRef, TypeRequest,
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "tsparse-native-lazy-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        for file in ["tsconfig.json", "types.ts", "consumer.ts"] {
            let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/types/native-lazy-types")
                .join(file);
            std::fs::copy(source, root.join(file)).unwrap();
        }
        Self(root)
    }
    fn project(&self) -> Project {
        Project::load(&self.0.join("tsconfig.json")).unwrap()
    }
    fn at(&self, file: &str, marker: &str, token: &str) -> TypeQuery {
        let source = std::fs::read_to_string(self.0.join(file)).unwrap();
        let offset = source.find(marker).unwrap() + marker.find(token).unwrap();
        let before = &source[..offset];
        let line = before.bytes().filter(|&byte| byte == b'\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap().encode_utf16().count() + 1;
        TypeQuery::At {
            file: self.0.join(file),
            position: SourcePosition { line, column },
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn named_roots_and_depth_zero_do_not_demand_lazy_member_types() {
    let fixture = Fixture::new();
    let project = fixture.project();
    for (file, marker, token, question, summary) in [
        (
            "types.ts",
            "interface Lazy",
            "Lazy",
            TypeQuestion::Declared,
            "Lazy",
        ),
        (
            "types.ts",
            "interface Broken",
            "Broken",
            TypeQuestion::Declared,
            "Broken",
        ),
        (
            "consumer.ts",
            "ImportedLazy",
            "ImportedLazy",
            TypeQuestion::Declared,
            "Lazy",
        ),
        (
            "consumer.ts",
            "ImportedBroken",
            "ImportedBroken",
            TypeQuestion::Declared,
            "Broken",
        ),
        (
            "consumer.ts",
            "importedValue",
            "importedValue",
            TypeQuestion::Value,
            "Lazy",
        ),
        (
            "consumer.ts",
            "importedBrokenValue",
            "importedBrokenValue",
            TypeQuestion::Value,
            "Broken",
        ),
    ] {
        for expand in [false, true] {
            let mut request = TypeRequest::new(fixture.at(file, marker, token));
            request.expand = expand;
            request.depth = 0;
            let answer = project.type_of(request);
            assert!(
                matches!(answer, Ok(TypeAnswer::Selected(_))),
                "valid named root must not demand an unshown member ({summary}, expand={expand}): {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.target.kind, question);
            assert_eq!(result.description.summary, summary);
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            if expand {
                let graph = result.description.expansion.unwrap();
                assert_eq!(graph.nodes.len(), 1);
                assert!(graph.truncated);
                let root = &graph.nodes[0];
                assert!(
                    matches!(&root.data, TypeNodeData::Object { members } if members.properties.is_none())
                );
                assert!(
                    root.omitted
                        .as_ref()
                        .unwrap()
                        .fields
                        .iter()
                        .any(|field| field == "properties")
                );
            } else {
                assert!(result.description.expansion.is_none());
            }
        }
    }
}

#[test]
fn expansion_evaluates_the_native_conditional_member() {
    let fixture = Fixture::new();
    let project = fixture.project();
    let mut request = TypeRequest::new(fixture.at("consumer.ts", "ImportedLazy", "ImportedLazy"));
    request.expand = true;
    let answer = project.type_of(request);
    assert!(
        matches!(answer, Ok(TypeAnswer::Selected(_))),
        "a valid lazy root must resolve its demanded conditional member: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    // The pinned native oracle resolves string extends number to number.
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert_eq!(result.description.summary, "Lazy");
    assert!(result.issues.is_empty());
    let graph = result.description.expansion.unwrap();
    assert!(
        !graph.truncated,
        "semantic partial details are distinct from display elision"
    );
    let TypeNodeData::Object { members } = &graph.nodes[0].data else {
        panic!("the established root stays an object")
    };
    let properties = members.properties.as_ref().unwrap();
    assert_eq!(
        properties
            .iter()
            .map(|property| &property.key)
            .collect::<Vec<_>>(),
        vec![
            &TypeKey::String {
                name: "known".into()
            },
            &TypeKey::String {
                name: "deferred".into()
            }
        ]
    );
    let node_for = |reference: &TypeRef| {
        let TypeRef::Ref { r#ref } = reference else {
            panic!("the admitted branch is a real graph node")
        };
        graph.nodes.iter().find(|node| node.id == *r#ref).unwrap()
    };
    assert_eq!(
        node_for(&properties[0].read_type).data,
        TypeNodeData::Primitive {
            name: PrimitiveName::String
        }
    );
    assert_eq!(
        node_for(&properties[1].read_type).data,
        TypeNodeData::Primitive {
            name: PrimitiveName::Number
        }
    );
}

#[test]
fn a_consumed_unresolved_lazy_member_fails_the_whole_operation() {
    let fixture = Fixture::new();
    let project = fixture.project();
    for token in ["ImportedBroken", "importedBrokenValue"] {
        let mut request = TypeRequest::new(fixture.at("consumer.ts", token, token));
        request.expand = true;
        let answer = project.type_of(request);
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
            "consumed unresolved member recovery must remain invalid, not a partial leaf or circular cache result: {answer:?}"
        );
        let Err(AnalysisError::Semantic(error)) = answer else {
            unreachable!()
        };
        assert_eq!(
            error.path,
            Some(std::fs::canonicalize(fixture.0.join("types.ts")).unwrap())
        );
        assert_eq!(
            error.position,
            Some(SourcePosition {
                line: 7,
                column: 12
            })
        );
    }
}

#[test]
fn repeated_deferred_alias_demands_share_the_native_primitive_result() {
    let fixture = Fixture::new();
    let project = fixture.project();
    let mut request =
        TypeRequest::new(fixture.at("consumer.ts", "ImportedRepeated", "ImportedRepeated"));
    request.expand = true;
    let answer = project.type_of(request);
    assert!(
        matches!(answer, Ok(TypeAnswer::Selected(_))),
        "a repeated pending declared dependency must not become a circular failure: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert_eq!(result.description.summary, "Repeated");
    assert!(result.issues.is_empty());
    let graph = result.description.expansion.unwrap();
    assert!(!graph.truncated);
    let TypeNodeData::Object { members } = &graph.nodes[0].data else {
        panic!("the established root stays an object")
    };
    let properties = members.properties.as_ref().unwrap();
    assert_eq!(properties.len(), 2);
    assert_eq!(properties[0].read_type, properties[1].read_type);
    for property in properties {
        let TypeRef::Ref { r#ref } = &property.read_type else {
            panic!("the pending child is admitted")
        };
        let child = graph.nodes.iter().find(|node| node.id == *r#ref).unwrap();
        assert_eq!(
            child.data,
            TypeNodeData::Primitive {
                name: PrimitiveName::Number
            }
        );
    }
}
