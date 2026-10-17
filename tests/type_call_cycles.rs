//! Actual call consumers drive native signature-return resolution recovery.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const FUNCTIONS: &str = include_str!("../fixtures/types/native-calls/functions.ts");
const CONSUMER: &str = include_str!("../fixtures/types/native-calls/consumer.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-calls/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-calls/loose.json"),
        ),
        ("functions.ts", FUNCTIONS),
        ("consumer.ts", CONSUMER),
    ])
}

fn request(fixture: &Fixture, name: &str, expand: bool, depth: usize) -> TypeRequest {
    let (file, text, offset) = if matches!(name, "mutualA" | "mutualB") {
        (
            "functions.ts",
            FUNCTIONS,
            FUNCTIONS.find(&format!("function {name}")).unwrap() + "function ".len(),
        )
    } else {
        (
            "consumer.ts",
            CONSUMER,
            CONSUMER.find(&format!("{name},")).unwrap(),
        )
    };
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path(file),
        position: position(text, offset),
    });
    request.expand = expand;
    request.depth = depth;
    request
}

fn circular(project: &Project, fixture: &Fixture, name: &str, expand: bool, depth: usize) {
    let answer = project.type_of(request(fixture, name, expand, depth));
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error))
        if error.kind == SemanticErrorKind::InvalidSemantic
            && error.reason == SemanticFailureReason::CircularType.into()),
        "{name} consumes signature-return recovery independently of diagnostic options: {answer:?}"
    );
    let Err(AnalysisError::Semantic(error)) = answer else {
        unreachable!()
    };
    assert_eq!(
        error.path,
        Some(fixture.path("functions.ts").canonicalize().unwrap())
    );
    let expected = if name == "mutualB" {
        "mutualB"
    } else {
        "mutualA"
    };
    let offset = FUNCTIONS.find(&format!("function {expected}")).unwrap() + "function ".len();
    assert_eq!(error.position, Some(position(FUNCTIONS, offset)));
}

#[test]
fn result_initializers_consume_actual_mutual_signature_recovery_before_graph_expansion() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (expand, depth) in [(false, 0), (true, 0), (true, 3), (true, 32)] {
            circular(&project, &fixture, "importedMutual", expand, depth);
        }
    }
}

#[test]
fn each_mutual_signature_root_retains_its_own_marked_return_recovery_provenance() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for name in ["mutualA", "mutualB"] {
            circular(&project, &fixture, name, false, 0);
            circular(&project, &fixture, name, true, 3);
        }
    }
}

#[test]
fn bare_self_never_and_established_recursive_annotation_are_healthy_controls() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let TypeAnswer::Selected(result) = project
            .type_of(request(&fixture, "importedDirectSelf", true, 3))
            .unwrap()
        else {
            panic!("the self result is selected")
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, "never");
        assert!(result.issues.is_empty());
        assert!(matches!(
            result.description.expansion.as_ref().unwrap().nodes[0].data,
            TypeNodeData::Never
        ));

        let answer = project.type_of(request(&fixture, "importedRecursiveAnnotation", true, 3));
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
            "established function value is the actual recursive annotation dependency: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(
            result.description.summary,
            "() => typeof recursiveAnnotation"
        );
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        let TypeNodeData::Function { members } = &root.data else {
            panic!("recursive annotation returns its established function")
        };
        let signatures = members.call_signatures.as_ref().unwrap();
        assert_eq!(signatures.len(), 1);
        assert_eq!(
            signatures[0].return_type,
            TypeRef::Ref { r#ref: graph.root }
        );
    }
}
