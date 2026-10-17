//! Native conditional tail recursion has its own recovered error boundary.
//! The limit counts aliased tail transitions, not source or instantiation depth.
//! Pinned string-decrement controls exercise 999/1000/1001 transitions without
//! constructing growing tuple types unrelated to the boundary under test.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const GLOBALS: &str = "interface Object {}\ninterface Function {}\ninterface Array<T> { length: number; [n: number]: T }\ninterface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T }\ninterface String { readonly length: number }\ninterface Number {}\ninterface Boolean {}\ninterface RegExp {}\ninterface IArguments {}\ninterface CallableFunction extends Function {}\ninterface NewableFunction extends Function {}\n";
const BUILD: &str = "type Next<S extends string> = S extends `x${infer R}` ? R : never;\ntype Build<S extends string> = S extends \"\" ? { length: 999 } : Build<Next<S>>;\n";

fn run(count: usize, later: bool) {
    // Indexed access forces the tuple's written operands. A plain tuple alias
    // would leave them deferred and would not prove native continuation.
    let reference = format!("Build<\"{}\">", "x".repeat(count));
    let body = if later {
        format!("[{reference}, MissingAfterTail][number]")
    } else {
        reference.clone()
    };
    let source = format!(
        "{GLOBALS}{BUILD}const /*#query*/query = (null as {body}).length;\nconst /*#healthy*/healthy = 45 as const;\n"
    );
    let fixture = Fixture::new(&[
        ("cases.ts", &source),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true},"files":["cases.ts"]}"#,
        ),
        (
            "nocheck.json",
            r#"{"extends":"./tsconfig.json","compilerOptions":{"noCheck":true}}"#,
        ),
    ]);
    for config in ["tsconfig.json", "nocheck.json"] {
        for reverse in [false, true] {
            let project = fixture.load_config(config);
            let names = if reverse {
                ["healthy", "query"]
            } else {
                ["query", "healthy"]
            };
            for name in names {
                let marker = format!("/*#{name}*/");
                for expand in [false, true] {
                    let label = format!("{config}/{count}/{later}/{reverse}/{name}/{expand}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(
                                &source,
                                source.find(&marker).unwrap() + marker.len(),
                            ),
                        },
                        expand,
                        depth: 0,
                    });
                    if name == "healthy" || count == 999 {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}");
                        assert_eq!(
                            answer.description.summary,
                            if name == "healthy" { "45" } else { "999" },
                            "{label}"
                        );
                        assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                        assert_eq!(
                            answer.description.expansion.is_some(),
                            expand,
                            "{label}: expansion presence"
                        );
                        if let Some(graph) = answer.description.expansion {
                            assert!(
                                graph.nodes.iter().all(|node| !matches!(
                                    node.data,
                                    TypeNodeData::Unsupported { .. }
                                )),
                                "{label}: {graph:?}"
                            );
                        }
                    } else {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::ConditionalTailLimit.into(),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.path,
                            Some(fixture.path("cases.ts")),
                            "{label}: {error:?}"
                        );
                        // Pinned conditionalTailLocation observes c.currentNode
                        // at c.error: the reached Build type-reference token.
                        // Direct type-alias queries have no currentNode; this
                        // expression deliberately reaches source checking.
                        assert_eq!(
                            error.position,
                            Some(position(&source, source.rfind(&reference).unwrap())),
                            "{label}: {error:?}"
                        );
                        if later {
                            let offset = source.find("MissingAfterTail").unwrap();
                            assert!(
                                error.causes.iter().any(|cause| cause.reason
                                    == SemanticFailureReason::UnresolvedSymbol.into()
                                    && cause.location.as_ref().is_some_and(|location| location
                                        .file
                                        == "cases.ts"
                                        && location.start == position(&source, offset)
                                        && location.end
                                            == position(
                                                &source,
                                                offset + "MissingAfterTail".len()
                                            ))),
                                "{label}: native continued to the later tuple operand: {error:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn conditional_tail_under_at_and_over_native_limit() {
    for count in [999, 1000, 1001] {
        run(count, false);
    }
}

#[test]
fn conditional_tail_error_value_preserves_later_operand_acquisition() {
    run(1000, true);
}
