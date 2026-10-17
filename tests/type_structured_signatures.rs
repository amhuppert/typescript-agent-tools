//! Final call consumers preserve native structured-signature decisions.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

const SITES: &str = include_str!("../fixtures/types/native-structured-signatures/sites.ts");
const DEFINITIONS: &str =
    include_str!("../fixtures/types/native-structured-signatures/definitions.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-structured-signatures/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-structured-signatures/loose.json"),
        ),
        (
            "bivariant.json",
            include_str!("../fixtures/types/native-structured-signatures/bivariant.json"),
        ),
        ("definitions.ts", DEFINITIONS),
        ("sites.ts", SITES),
    ])
}

fn answer(project: &Project, fixture: &Fixture, name: &str) -> Result<TypeAnswer, AnalysisError> {
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("sites.ts"),
        position: position(SITES, SITES.find(&format!("{name}Result =")).unwrap()),
    });
    request.expand = true;
    request.depth = 2;
    project.type_of(request)
}

fn found(project: &Project, fixture: &Fixture, name: &str, value: &str) {
    let answer = answer(project, fixture, name);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
        "{name} must complete its actual signature comparison: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.description.summary, format!("\"{value}\""), "{name}");
    assert!(result.issues.is_empty(), "{name}");
    let graph = result.description.expansion.unwrap();
    let root = graph
        .nodes
        .into_iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    assert_eq!(
        root.data,
        TypeNodeData::Literal {
            base: PrimitiveName::String,
            value: value.into(),
        },
        "{name}"
    );
}

#[test]
fn void_any_and_identity_skip_source_signature_returns() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        for name in [
            "returnVoid",
            "returnAny",
            "identity",
            "constructVoid",
            "constructAny",
        ] {
            found(&project, &fixture, name, "chosen");
        }
    }
}

#[test]
fn visited_signature_dependencies_preserve_their_exact_declaration_failures() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        for (name, token) in [
            ("returnUnknown", "MissingReturn;"),
            ("returnNumber", "MissingReturn;"),
            ("targetReturn", "MissingTargetReturn;"),
            ("arityTail", "MissingTail)"),
            ("extraTarget", "MissingExtra)"),
            ("thisEarly", "MissingThis,"),
            ("indexInventory", "MissingInventory;"),
            ("constructUnknown", "MissingNewReturn;"),
            ("predicateMissing", "MissingPredicate;"),
        ] {
            let answer = answer(&project, &fixture, name);
            let path = fixture.path("definitions.ts").canonicalize().unwrap();
            let expected = position(DEFINITIONS, DEFINITIONS.find(token).unwrap());
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                    && error.path.as_deref() == Some(path.as_path())
                    && error.position == Some(expected)),
                "{profile}:{name} must preserve its native visited child: {answer:?}"
            );
        }
    }
}

#[test]
fn arity_and_earlier_collection_mismatches_skip_unvisited_annotations() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        for name in [
            "arityEarly",
            "propertyFirst",
            "callFirst",
            "constructFirst",
            "requiredUndefined",
        ] {
            found(&project, &fixture, name, "same");
        }
        for name in ["requiredVoid", "optional"] {
            found(&project, &fixture, name, "chosen");
        }
    }
}

#[test]
fn strict_function_variance_uses_target_declaration_role_and_callback_rules() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json", "bivariant.json"] {
        let project = fixture.load_config(profile);
        found(
            &project,
            &fixture,
            "functionVariance",
            if profile == "bivariant.json" {
                "chosen"
            } else {
                "same"
            },
        );
        found(&project, &fixture, "methodVariance", "chosen");
        found(&project, &fixture, "callbackVariance", "same");
    }
}

#[test]
fn overload_matrix_and_predicate_kind_and_parameter_index_select_exact_results() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        for name in ["overloadSecond", "overloadAll", "predicateSame"] {
            found(&project, &fixture, name, "chosen");
        }
        for name in ["overloadMismatch", "predicateIndex", "predicateBoolean"] {
            found(&project, &fixture, name, "same");
        }
    }
}

#[test]
fn generic_signature_context_and_erasure_use_native_instantiation() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        for name in ["genericContext", "genericErase"] {
            found(&project, &fixture, name, "chosen");
        }
    }
}

#[test]
fn abstract_and_concrete_constructor_signatures_preserve_direction() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        found(&project, &fixture, "abstractConcrete", "same");
        found(&project, &fixture, "concreteAbstract", "chosen");
    }
}
