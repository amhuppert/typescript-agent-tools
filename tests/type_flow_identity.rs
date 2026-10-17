//! Stable index keys preserve actual symbol identity and the documented
//! distinction between identity components and literal property-name bytes.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-flow-identity/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-flow-identity/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-flow-identity/loose.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    });
    request.expand = true;
    request.depth = 2;
    request
}

#[test]
fn stable_and_assigned_index_symbols_keep_completed_declared_reads() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for marker in [
            "stableParameter",
            "stableConst",
            "stableMutable",
            "assignedMutable",
            "nestedSuffix",
        ] {
            let answer = project.type_of(request(&fixture, marker));
            let Ok(TypeAnswer::Selected(result)) = &answer else {
                panic!("{config}:{marker}: {answer:?}");
            };
            assert_eq!(
                result.outcome,
                TypeOutcome::Found,
                "{config}:{marker}: {result:?}"
            );
            assert!(result.issues.is_empty(), "{config}:{marker}: {result:?}");
            assert_eq!(
                result.description.summary, "string | number",
                "{config}:{marker}"
            );
            let graph = result.description.expansion.as_ref().unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            assert!(
                matches!(root.data, TypeNodeData::Union { .. }),
                "{config}:{marker}: {root:?}"
            );
        }
    }
}

#[test]
fn ordinary_named_dot_concatenation_keeps_its_native_cache_behavior() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let answer = fixture
            .load_config(config)
            .type_of(request(&fixture, "namedDots"));
        let Ok(TypeAnswer::Selected(result)) = &answer else {
            panic!("{config}: {answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found, "{config}: {result:?}");
        assert!(result.issues.is_empty(), "{config}: {result:?}");
        assert_eq!(
            result.description.summary,
            "{ warm: string | number; result: string | number; }"
        );
    }
}

#[test]
fn literal_binary_bytes_cannot_hide_a_dynamic_read_requiring_flow() {
    let fixture = fixture();
    let file = fixture.path("cases.ts").canonicalize().unwrap();
    let tag = "/*#binaryRead*/";
    let expected = position(SOURCE, SOURCE.find(tag).unwrap() + tag.len());
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for _ in 0..2 {
            let answer = project.type_of(request(&fixture, "binaryIdentity"));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::UnsupportedSemantic
                    && error.reason == SemanticFailureReason::FlowRequired.into()
                    && error.path.as_deref() == Some(file.as_path())
                    && error.position == Some(expected)),
                "{config}: {answer:?}"
            );
        }
    }
}
