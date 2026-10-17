//! Final Project controls for the compiler-pinned native return-body owner.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeQuestion, TypeRef,
    TypeRequest, TypeResult,
};

const FUNCTIONS: &str = include_str!("../fixtures/types/native-return-body/functions.ts");
const CONSUMER: &str = include_str!("../fixtures/types/native-return-body/consumer.ts");
fn fixture(strict: bool) -> Fixture {
    let config = if strict {
        include_str!("../fixtures/types/native-return-body/tsconfig.json")
    } else {
        include_str!("../fixtures/types/native-return-body/loose.json")
    };
    Fixture::new(&[
        ("tsconfig.json", config),
        ("functions.ts", FUNCTIONS),
        ("consumer.ts", CONSUMER),
    ])
}
fn answer(fixture: &Fixture, file: &str, name: &str) -> Result<TypeAnswer, AnalysisError> {
    let (source, offset) = if file == "consumer.ts" {
        (CONSUMER, CONSUMER.find(&format!("{name},")).unwrap())
    } else {
        let marker = format!("function {name}(");
        (
            FUNCTIONS,
            FUNCTIONS.find(&marker).unwrap() + "function ".len(),
        )
    };
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path(file),
        position: position(source, offset),
    });
    request.expand = true;
    request.depth = 3;
    fixture.load().type_of(request)
}
fn selected(fixture: &Fixture, name: &str, expected: &str) -> TypeResult {
    let answer = answer(fixture, "consumer.ts", name);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
        "{name} must establish its native body return: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.target.kind, TypeQuestion::Value);
    assert_eq!(result.description.summary, expected, "{name}");
    assert!(result.issues.is_empty(), "{name}");
    result
}
fn returned(result: &TypeResult) -> &TypeNodeData {
    let expansion = result.description.expansion.as_ref().unwrap();
    let TypeNodeData::Function { members } = &expansion.nodes[0].data else {
        panic!("a native function's actual signature must be established")
    };
    let calls = members.call_signatures.as_ref().unwrap();
    assert_eq!(calls.len(), 1);
    let TypeRef::Ref { r#ref } = &calls[0].return_type else {
        panic!("a supported return must have an admitted graph node")
    };
    &expansion
        .nodes
        .iter()
        .find(|node| &node.id == r#ref)
        .unwrap()
        .data
}

#[test]
fn primitive_object_and_nested_body_returns_use_actual_cross_file_signatures() {
    for strict in [true, false] {
        let fixture = fixture(strict);
        for (name, expected, primitive) in [
            ("importedScalar", "() => number", PrimitiveName::Number),
            ("importedString", "() => string", PrimitiveName::String),
            (
                "importedBoolean",
                "(flag: boolean) => boolean",
                PrimitiveName::Boolean,
            ),
            ("importedNested", "() => number", PrimitiveName::Number),
        ] {
            let result = selected(&fixture, name, expected);
            assert!(
                matches!(returned(&result), TypeNodeData::Primitive { name } if *name == primitive)
            );
        }
        let object = selected(
            &fixture,
            "importedObject",
            "() => { text: string; count: number; }",
        );
        assert!(matches!(returned(&object), TypeNodeData::Object { .. }));
        let own = answer(&fixture, "functions.ts", "scalar");
        assert!(
            matches!(&own, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == "() => number"),
            "own declaration and renamed import ask the same native return: {own:?}"
        );
    }
}

#[test]
fn implicit_bare_and_empty_return_states_preserve_option_dependent_results() {
    for strict in [true, false] {
        let fixture = fixture(strict);
        for name in [
            "importedNoReturn",
            "importedBareReturn",
            "importedThrow",
            "importedInfinite",
        ] {
            let result = selected(&fixture, name, "() => void");
            assert!(matches!(returned(&result), TypeNodeData::Void));
        }
        for name in ["importedImplicit", "importedExplicitBare"] {
            selected(
                &fixture,
                name,
                if strict {
                    "(flag: boolean) => 1 | undefined"
                } else {
                    "(flag: boolean) => number"
                },
            );
        }
        selected(
            &fixture,
            "importedUnreachableBare",
            if strict {
                "() => 1 | undefined"
            } else {
                "() => number"
            },
        );
    }
}

#[test]
fn ordered_aggregation_preserves_regular_literal_unions_and_unreachable_contributions() {
    for strict in [true, false] {
        let fixture = fixture(strict);
        for (name, expected) in [
            ("importedAllReturns", "(flag: boolean) => 1 | 2"),
            ("importedMixed", "(flag: boolean) => \"ready\" | 1"),
            ("importedUnreachable", "() => \"unreachable\" | 1"),
        ] {
            let result = selected(&fixture, name, expected);
            assert!(
                matches!(returned(&result), TypeNodeData::Union { alternatives, .. } if alternatives.len() == 2)
            );
        }
    }
}

#[test]
fn never_operands_and_native_bare_recursive_calls_keep_their_actual_dependencies() {
    for strict in [true, false] {
        let fixture = fixture(strict);
        for name in ["importedRecursive", "importedNever"] {
            let result = selected(&fixture, name, "() => never");
            assert!(matches!(returned(&result), TypeNodeData::Never));
        }
        selected(
            &fixture,
            "importedRecursiveScalar",
            "(flag: boolean) => number",
        );
    }
}

#[test]
fn annotations_skip_invalid_bodies_but_demanded_missing_returns_preserve_failure() {
    for strict in [true, false] {
        let fixture = fixture(strict);
        selected(&fixture, "importedAnnotated", "() => number");
        let answer = answer(&fixture, "consumer.ts", "importedInvalid");
        let own_path = fixture.path("functions.ts").canonicalize().unwrap();
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref() == Some(own_path.as_path())),
            "the actual unresolved return operand is a consumed dependency at its own source: {answer:?}"
        );
    }
}
