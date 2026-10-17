//! `getTypeFromThisTypeNode` consumes the canonical target's real synthetic
//! parameter; binder ContainsThis is captured before source projection.
mod support;

use support::{position, project};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest,
};

#[test]
fn ordinary_interface_this_types_use_the_actual_synthetic_parameter() {
    for (name, body) in [
        ("Direct", "value: /*#query*/this;"),
        ("Method", "method(): /*#query*/this;"),
        ("Call", "(): /*#query*/this;"),
        ("Construct", "new(): /*#query*/this;"),
        ("FunctionProperty", "method: () => /*#query*/this;"),
        ("ConstructorProperty", "method: new() => /*#query*/this;"),
        ("Index", "[index: number]: /*#query*/this;"),
    ] {
        for separator in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
            let source = format!("\u{feff}/*😀*/interface {name} {{{separator}{body}{separator}}}");
            let fixture = project(&[("globals.ts", &source)]);
            let loaded = fixture.load();
            let offset = source.find("/*#query*/").unwrap() + "/*#query*/".len();
            for expand in [false, true] {
                let answer = loaded.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("globals.ts"),
                        position: position(&source, offset),
                    },
                    expand,
                    depth: 0,
                });
                assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
                    "{name}: {answer:?}"
                );
                let TypeAnswer::Selected(result) = answer.unwrap() else {
                    unreachable!()
                };
                assert_eq!(result.target.kind, TypeQuestion::Type);
                assert_eq!(result.description.summary, "this");
                assert!(result.issues.is_empty());
                if let Some(graph) = result.description.expansion {
                    assert_eq!(graph.nodes.len(), 1);
                    let root = &graph.nodes[0];
                    assert!(
                        matches!(&root.data, TypeNodeData::TypeParameter { name, constraint: None, default: None } if name == "this")
                    );
                    assert!(
                        graph.truncated,
                        "actual constraint was omitted before demand"
                    );
                }
            }
        }
    }
}

#[test]
fn invalid_this_type_contexts_preserve_native_error_type_recovery() {
    for source in [
        "type Invalid = /*#query*/this;",
        "interface Nested { value: { nested: /*#query*/this }; }",
        "interface Generic<T extends /*#query*/this> { value: T; }",
        "class Static { static value: /*#query*/this; }",
        "class Constructor { constructor(input: /*#query*/this) {} }",
        "function ordinary(): /*#query*/this { throw 0; }",
        "class StaticArrow { static value = (input: /*#query*/this) => input; }",
        "class NestedFunction { method() { function nested(): /*#query*/this { throw 0; } } }",
        "class ComputedName { [(input: /*#query*/this) => input]: number; }",
    ] {
        let fixture = project(&[("globals.ts", source)]);
        let loaded = fixture.load();
        let offset = source.find("/*#query*/").unwrap() + "/*#query*/".len();
        let answer = loaded.type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("globals.ts"),
                position: position(source, offset),
            },
            expand: false,
            depth: 0,
        });
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::ErrorType.into()),
            "{source}: {answer:?}"
        );
        let Err(AnalysisError::Semantic(error)) = answer else {
            unreachable!()
        };
        assert_eq!(error.path, Some(fixture.path("globals.ts")));
        assert_eq!(error.position, Some(position(source, offset)));
    }
}

#[test]
fn class_body_arrow_and_merged_interface_sites_use_the_established_this_parameter() {
    for source in [
        "class ClassMember { value: /*#query*/this; }",
        "class ConstructorBody { constructor() { let input: /*#query*/this; } }",
        "class ArrowMember { value = (input: /*#query*/this) => input; }",
        "interface Merged { value: number; } interface Merged { value2: /*#query*/this; }",
        "interface Merged { value: /*#query*/this; } interface Merged { value2: number; }",
    ] {
        let fixture = project(&[("globals.ts", source)]);
        let loaded = fixture.load();
        let offset = source.find("/*#query*/").unwrap() + "/*#query*/".len();
        let answer = loaded.type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("globals.ts"),
                position: position(source, offset),
            },
            expand: false,
            depth: 0,
        });
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found
            && result.target.kind == TypeQuestion::Type && result.description.summary == "this" && result.issues.is_empty()),
            "{source}: {answer:?}"
        );
    }
}
