//! Canonical parameter identities and admitted lazy native child questions.
mod support;

use std::path::Path;
use support::{Fixture, position, project};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeQuestion, TypeRef,
    TypeRequest, TypeResult,
};

fn at(fixture: &Fixture, source: &str) -> TypeQuery {
    let offset = source.find("value: T").unwrap() + "value: ".len();
    TypeQuery::At {
        file: fixture.path("globals.ts"),
        position: position(source, offset),
    }
}

fn selected(project: &Project, query: TypeQuery, expand: bool, depth: usize) -> TypeResult {
    let answer = project.type_of(TypeRequest {
        query,
        expand,
        depth,
    });
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(_))),
        "a canonical parameter is established before its child questions: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert_eq!(result.target.kind, TypeQuestion::Type);
    assert_eq!(result.description.summary, "T");
    assert!(!result.description.summary_truncated);
    assert!(result.issues.is_empty());
    result
}

#[test]
fn canonical_merged_parameters_select_constraint_and_default_independently() {
    for (case, expected_constraint, expected_default) in [
        ("basic", None, None),
        (
            "merged-later-constraint",
            Some(TypeNodeData::Primitive {
                name: PrimitiveName::String,
            }),
            None,
        ),
        (
            "merged-later-default",
            None,
            Some(TypeNodeData::Literal {
                base: PrimitiveName::String,
                value: "ready".into(),
            }),
        ),
    ] {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/json-generic-targets")
            .join(case);
        let fixture = Fixture::existing(&directory);
        let source = std::fs::read_to_string(fixture.path("globals.ts")).unwrap();
        for strict in [false, true] {
            for exact in [false, true] {
                if exact && !strict {
                    continue;
                }
                for unchecked in [false, true] {
                    let loaded = fixture.load_config(&format!(
                        "strict-{}-exact-{}-unchecked-{}.json",
                        u8::from(strict),
                        u8::from(exact),
                        u8::from(unchecked)
                    ));
                    let result = selected(&loaded, at(&fixture, &source), true, 3);
                    let graph = result.description.expansion.unwrap();
                    assert!(!graph.truncated, "{case}");
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    assert_eq!(root.alias, None);
                    assert_eq!(root.type_arguments, None);
                    assert_eq!(root.omitted, None);
                    let TypeNodeData::TypeParameter {
                        name,
                        constraint,
                        default,
                    } = &root.data
                    else {
                        panic!("{case}: {root:?}")
                    };
                    assert_eq!(name, "T");
                    for (reference, expected) in [
                        (constraint, &expected_constraint),
                        (default, &expected_default),
                    ] {
                        let actual = reference.as_ref().map(|reference| {
                            let TypeRef::Ref { r#ref } = reference else {
                                panic!("admitted child must be a real graph identity")
                            };
                            graph
                                .nodes
                                .iter()
                                .find(|node| node.id == *r#ref)
                                .unwrap()
                                .data
                                .clone()
                        });
                        assert_eq!(&actual, expected, "{case}");
                    }
                }
            }
        }
    }
}

#[test]
fn parameter_roots_and_omitted_children_do_not_consume_invalid_annotations() {
    for (case, missing) in [
        ("unused-constraint", "MissingConstraint"),
        ("unused-default", "MissingDefault"),
    ] {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/json-generic-targets")
            .join(case);
        let fixture = Fixture::existing(&directory);
        let source = std::fs::read_to_string(fixture.path("globals.ts")).unwrap();
        let loaded = fixture.load();
        for expand in [false, true] {
            let result = selected(&loaded, at(&fixture, &source), expand, 0);
            if expand {
                let graph = result.description.expansion.unwrap();
                assert_eq!(graph.nodes.len(), 1);
                assert!(graph.truncated);
            } else {
                assert_eq!(result.description.expansion, None);
            }
        }
        let answer = loaded.type_of(TypeRequest {
            query: at(&fixture, &source),
            expand: true,
            depth: 3,
        });
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
            "admitted {case} recovery must fail the final operation: {answer:?}"
        );
        let Err(AnalysisError::Semantic(error)) = answer else {
            unreachable!()
        };
        assert_eq!(error.path, Some(fixture.path("globals.ts")));
        assert_eq!(
            error.position,
            Some(position(&source, source.find(missing).unwrap()))
        );
    }
}

#[test]
fn ordinary_any_constraints_use_native_unknown_normalization() {
    for constraint in ["any", "unknown"] {
        let source = format!("interface Sample<T extends {constraint}> {{ value: T; }}\n");
        let fixture = project(&[("globals.ts", &source)]);
        let result = selected(&fixture.load(), at(&fixture, &source), true, 3);
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        let TypeNodeData::TypeParameter {
            constraint: Some(TypeRef::Ref { r#ref }),
            default: None,
            ..
        } = &root.data
        else {
            panic!("{root:?}")
        };
        assert_eq!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == *r#ref)
                .unwrap()
                .data,
            TypeNodeData::Unknown
        );
    }
}

#[test]
fn generic_reference_arity_recovery_is_invalid_in_the_final_type_question() {
    // Fresh 7.0.2 GetTypeAtLocation(actual TypeReference) controls all return
    // native errorType any (flags1), with zero input diagnostics. A generic
    // declared target is a different question and cannot stand in for it.
    for (declaration, reference) in [
        ("interface Box<T> { value: T; }", "Box"),
        ("interface Box<T, U> { left: T; right: U; }", "Box<string>"),
        ("interface Box<T> { value: T; }", "Box<string, number>"),
        ("interface Box<T = string, U> { left: T; right: U; }", "Box"),
        (
            "interface Box<T> { value: T; }",
            "Box<MissingArgument, string>",
        ),
        (
            "interface Box<T = MissingDefault, U> { left: T; right: U; }",
            "Box",
        ),
        ("interface Box { value: string; }", "Box<number>"),
    ] {
        let source = format!("{declaration}\ndeclare const result: /*#query*/{reference};\n");
        let fixture = project(&[("globals.ts", &source)]);
        let offset = source.find("/*#query*/").unwrap() + "/*#query*/".len();
        let answer = fixture.load().type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("globals.ts"),
                position: position(&source, offset),
            },
            expand: false,
            depth: 0,
        });
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::ErrorType.into()),
            "a {reference} type question must refuse actual arity recovery: {answer:?}",
        );
        let Err(AnalysisError::Semantic(error)) = answer else {
            unreachable!()
        };
        assert_eq!(error.path, Some(fixture.path("globals.ts")));
        assert_eq!(error.position, Some(position(&source, offset)));
    }
}

#[test]
fn reference_defaults_substitute_actual_canonical_parameter_identities() {
    // Fresh actual TypeReference questions establish these arguments without
    // demanding an unused constraint or an explicitly overridden default.
    for (declaration, reference, summary) in [
        (
            "interface Box<T = number> { value: T; }",
            "Box",
            "Box<number>",
        ),
        (
            "interface Box<T> { value: T; }\ninterface Box<T = number> { other: T; }",
            "Box",
            "Box<number>",
        ),
        (
            "interface Box<T = string, U = T> { left: T; right: U; }",
            "Box",
            "Box<string, string>",
        ),
        (
            "interface Box<T, U = T> { left: T; right: U; }",
            "Box<number>",
            "Box<number, number>",
        ),
        (
            "interface Box<T = MissingDefault> { value: T; }",
            "Box<number>",
            "Box<number>",
        ),
        (
            "interface Box<T extends MissingConstraint> { value: T; }",
            "Box<number>",
            "Box<number>",
        ),
    ] {
        let source = format!("{declaration}\ndeclare const result: /*#query*/{reference};\n");
        let fixture = project(&[("globals.ts", &source)]);
        let offset = source.find("/*#query*/").unwrap() + "/*#query*/".len();
        let answer = fixture.load().type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("globals.ts"),
                position: position(&source, offset),
            },
            expand: false,
            depth: 0,
        });
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{reference}: {answer:?}")
        };
        assert_eq!(result.description.summary, summary);
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
    }
}

#[test]
fn consumed_default_recovery_stays_invalid_even_when_reference_root_is_not_error_type() {
    // Native roots are objects; their chosen arguments contain real errorType.
    // Forward/self defaults receive the fillMissingTypeArguments sentinel,
    // rather than becoming a genuine any type or a spelling-based substitute.
    for declaration in [
        "interface Box<T = MissingDefault> { value: T; }",
        "interface Box<T = U, U = string> { left: T; right: U; }",
        "interface Box<T = T> { value: T; }",
        "interface Box<T = U, U = T> { left: T; right: U; }",
    ] {
        let source = format!("{declaration}\ndeclare const result: /*#query*/Box;\n");
        let fixture = project(&[("globals.ts", &source)]);
        let offset = source.find("/*#query*/").unwrap() + "/*#query*/".len();
        let answer = fixture.load().type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("globals.ts"),
                position: position(&source, offset),
            },
            expand: false,
            depth: 0,
        });
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic),
            "{declaration}: {answer:?}"
        );
    }
}

#[test]
fn canonical_parameter_references_reject_type_arguments_without_demanding_them() {
    let source = "interface Box<T> { value: /*#query*/T<MissingArgument>; }\n";
    let fixture = project(&[("globals.ts", source)]);
    let offset = source.find("/*#query*/").unwrap() + "/*#query*/".len();
    let answer = fixture.load().type_of(TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("globals.ts"),
            position: position(source, offset),
        },
        expand: false,
        depth: 0,
    });
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::ErrorType.into()),
        "{answer:?}"
    );
}

#[test]
fn class_references_use_the_canonical_class_or_interface_target_operation() {
    // Fresh 7.0.2 actual TypeReference controls: Object roots, native
    // errorType false, exact chosen arguments, zero input diagnostics. These
    // are instance type questions, separately from constructor value types.
    for (case, summary) in [
        ("generic", "Box<number>"),
        ("nongeneric", "Box"),
        ("merged-default", "Box<string>"),
        ("namespace-first-types", "Box<number>"),
        ("interface-first", "Box<number>"),
        ("escaped-first", "\\u0042ox<number>"),
        ("unused-constraint", "Box<number>"),
        ("overridden-default", "Box<number>"),
    ] {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-class-targets")
            .join(case);
        let fixture = Fixture::existing(&directory);
        let source = std::fs::read_to_string(fixture.path("globals.ts")).unwrap();
        let offset = source.find("/*#query*/").unwrap() + "/*#query*/".len();
        let answer = fixture.load().type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("globals.ts"),
                position: position(&source, offset),
            },
            expand: false,
            depth: 0,
        });
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{case}: {answer:?}")
        };
        assert_eq!(result.target.kind, TypeQuestion::Type, "{case}");
        assert_eq!(result.description.summary, summary, "{case}");
        assert_eq!(result.outcome, TypeOutcome::Found, "{case}");
        assert!(result.issues.is_empty(), "{case}");
    }
}
