//! Final consumers for native index-name, variance and inference lifetime rules.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeExpansion, TypeKey, TypeNodeData, TypeOutcome, TypeQuery,
    TypeRef, TypeRequest,
};

const DEFINITIONS: &str =
    include_str!("../fixtures/types/native-inference-boundaries/definitions.ts");
const SITES: &str = include_str!("../fixtures/types/native-inference-boundaries/sites.ts");
const PROFILES: [&str; 4] = [
    "tsconfig.json",
    "loose.json",
    "exact.json",
    "bivariant.json",
];

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-inference-boundaries/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-inference-boundaries/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-inference-boundaries/exact.json"),
        ),
        (
            "bivariant.json",
            include_str!("../fixtures/types/native-inference-boundaries/bivariant.json"),
        ),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-inference-boundaries/globals.d.ts"),
        ),
        ("definitions.ts", DEFINITIONS),
        ("sites.ts", SITES),
    ])
}

fn request(fixture: &Fixture, name: &str) -> TypeRequest {
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("sites.ts"),
        position: position(SITES, SITES.find(&format!("{name}Result =")).unwrap()),
    });
    request.expand = true;
    request.depth = 3;
    request
}

fn expected(name: &str, profile: &str) -> &'static str {
    if matches!(name, "indexPublicClass" | "indexPrivateClass")
        || name == "indexOptional" && !matches!(profile, "exact.json" | "loose.json")
        || matches!(name, "varianceDeclaredIn" | "varianceDeclaredInvariant")
        || matches!(name, "varianceContravariant" | "varianceInvariant")
            && profile != "bivariant.json"
    {
        "same"
    } else {
        "chosen"
    }
}

fn selected(name: &str) {
    let fixture = fixture();
    for profile in PROFILES {
        let project = fixture.load_config(profile);
        let answer = project.type_of(request(&fixture, name));
        let Ok(TypeAnswer::Selected(result)) = &answer else {
            panic!("{profile}:{name}: {answer:?}");
        };
        assert_eq!(
            result.outcome,
            TypeOutcome::Found,
            "{profile}:{name}: {result:?}"
        );
        assert!(result.issues.is_empty(), "{profile}:{name}: {result:?}");
        assert_eq!(
            result.description.summary,
            format!("\"{}\"", expected(name, profile)),
            "{profile}:{name}"
        );
        let graph = result.description.expansion.as_ref().unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        assert_eq!(
            root.data,
            TypeNodeData::Literal {
                base: PrimitiveName::String,
                value: expected(name, profile).into()
            },
            "{profile}:{name}"
        );
    }
}

fn missing(name: &str, token: &str) {
    let fixture = fixture();
    let path = fixture.path("definitions.ts").canonicalize().unwrap();
    let expected = position(DEFINITIONS, DEFINITIONS.find(token).unwrap());
    for profile in PROFILES {
        let answer = fixture
            .load_config(profile)
            .type_of(request(&fixture, name));
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic
                && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                && error.path.as_deref() == Some(path.as_path())
                && error.position == Some(expected)),
            "{profile}:{name}: {answer:?}"
        );
    }
}

fn node<'a>(graph: &'a TypeExpansion, reference: &TypeRef) -> &'a TypeNodeData {
    let TypeRef::Ref { r#ref } = reference else {
        panic!("expected expanded dependency: {reference:?}");
    };
    &graph
        .nodes
        .iter()
        .find(|node| node.id == *r#ref)
        .unwrap()
        .data
}

#[test]
fn index_numeric() {
    selected("indexNumeric");
}

#[test]
fn index_numeric_string() {
    selected("indexNumericString");
}

#[test]
fn index_computed() {
    selected("indexComputed");
}

#[test]
fn index_instantiated() {
    selected("indexInstantiated");
}

#[test]
fn index_optional() {
    selected("indexOptional");
}

#[test]
fn index_string() {
    selected("indexString");
}

#[test]
fn index_public_class() {
    selected("indexPublicClass");
}

#[test]
fn index_private_class() {
    selected("indexPrivateClass");
}

#[test]
fn variance_covariant() {
    selected("varianceCovariant");
}

#[test]
fn variance_contravariant() {
    selected("varianceContravariant");
}

#[test]
fn variance_invariant() {
    selected("varianceInvariant");
}

#[test]
fn variance_method() {
    selected("varianceMethod");
}

#[test]
fn variance_independent() {
    selected("varianceIndependent");
}

#[test]
fn variance_recursive() {
    selected("varianceRecursive");
}

#[test]
fn variance_declared_out() {
    selected("varianceDeclaredOut");
}

#[test]
fn variance_declared_in() {
    selected("varianceDeclaredIn");
}

#[test]
fn variance_declared_invariant() {
    selected("varianceDeclaredInvariant");
}

#[test]
fn variance_dependency_keeps_original_failure() {
    missing("varianceFailure", "MissingVarianceDependency");
}

#[test]
fn late_default_keeps_original_failure() {
    missing("lateFailure", "MissingLateDefault");
}

#[test]
fn late_inferred_member_resolves_after_the_call_context_finishes() {
    let fixture = fixture();
    for profile in PROFILES {
        let answer = fixture
            .load_config(profile)
            .type_of(request(&fixture, "late"));
        let Ok(TypeAnswer::Selected(result)) = &answer else {
            panic!("{profile}: {answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found, "{profile}: {result:?}");
        assert!(result.issues.is_empty(), "{profile}: {result:?}");
        let graph = result.description.expansion.as_ref().unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        let TypeNodeData::Object { members } = &root.data else {
            panic!("{profile}: {root:?}");
        };
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(properties.len(), 1, "{profile}: {properties:?}");
        assert_eq!(
            properties[0].key,
            TypeKey::String {
                name: "value".into()
            }
        );
        assert_eq!(
            node(graph, &properties[0].read_type),
            &TypeNodeData::Primitive {
                name: PrimitiveName::Number
            }
        );
    }
}
