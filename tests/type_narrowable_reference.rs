//! Actual generic reference reads use the native constraint/context question.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-narrowable-reference/cases.ts");
const STRICT: &str = include_str!("../fixtures/types/native-narrowable-reference/tsconfig.json");
const LOOSE: &str = include_str!("../fixtures/types/native-narrowable-reference/loose.json");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        ("tsconfig.json", STRICT),
        ("loose.json", LOOSE),
    ])
}

fn request(fixture: &Fixture, marker: &str) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    })
}

#[test]
fn ordinary_generic_references_preserve_the_native_parameter_identity() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for marker in [
            "unconstrained",
            "primitiveConstraint",
            "unionStatement",
            "nullableStatement",
            "typeofOperand",
            "uninitializedContext",
        ] {
            let mut request = request(&fixture, marker);
            request.expand = true;
            request.depth = 0;
            let answer = project.type_of(request);
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "an unchanged native generic reference is a valid type ({config}, {marker}): {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, "T");
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.unwrap();
            assert!(
                matches!(&graph.nodes[0].data, TypeNodeData::TypeParameter { name, .. } if name == "T")
            );
        }
    }
}

#[test]
fn constraint_predicate_skips_unconsumed_invalid_contexts() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, expected) in [
            ("nonGenericMissingContext", "string"),
            ("primitiveMissingContext", "T"),
        ] {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "native false predicate must skip the Missing context ({config}, {marker}): {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, expected);
            assert!(result.issues.is_empty());
        }
    }
}

#[test]
fn initializer_context_substitutes_only_native_union_constraints() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, expected) in [
            ("primitiveContext", "string | number"),
            (
                "nullableContext",
                if config == "tsconfig.json" {
                    "string | undefined"
                } else {
                    "T"
                },
            ),
            ("genericContext", "T"),
            (
                "unionOfParameter",
                if config == "tsconfig.json" {
                    "string | number | undefined"
                } else {
                    "number | T"
                },
            ),
        ] {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "native contextual substitution must establish its exact baseline ({config}, {marker}): {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, expected);
            assert!(result.issues.is_empty());
        }
    }
}

#[test]
fn consumed_constraint_recovery_stays_invalid_even_when_native_root_is_t() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let answer = project.type_of(request(&fixture, "missingConstraint"));
        let missing = position(
            SOURCE,
            SOURCE.find("missingConstraint<T extends Missing>").unwrap()
                + "missingConstraint<T extends ".len(),
        );
        let own_path = fixture.path("cases.ts");
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic
            && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
            && error.path.as_deref() == Some(own_path.as_path())
            && error.position == Some(missing)),
            "the union-constraint predicate consumes Missing recovery before returning T ({config}): {answer:?}"
        );
    }
}
