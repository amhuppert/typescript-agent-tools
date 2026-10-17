//! Native unary/binary checking continues after consumed recovery.
//! Assertions keep constraints lazy until the operator reads them; the comma
//! roots require the first recovered unary value to permit the next operation.
//! Equality initializers are Normal checks, not a TypeOnly suppression witness.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

fn required(name: &str) -> &'static [&'static str] {
    match name {
        "unaryPlus" => &["MissingPlusFirst", "MissingPlusSecond"],
        "unaryMinus" => &["MissingMinusFirst", "MissingMinusSecond"],
        "unaryTilde" => &["MissingTildeFirst", "MissingTildeSecond"],
        "unaryNot" => &["MissingNotFirst", "MissingNotSecond"],
        "prefixUpdate" => &["MissingPrefixFirst", "MissingPrefixSecond"],
        "postfixUpdate" => &["MissingPostfixFirst", "MissingPostfixSecond"],
        "arithmetic" => &["MissingArithmeticFirst", "MissingArithmeticSecond"],
        "addition" => &["MissingAdditionFirst", "MissingAdditionSecond"],
        "relational" => &["MissingRelationalFirst", "MissingRelationalSecond"],
        "equality" => &["MissingEqualityFirst", "MissingEqualitySecond"],
        "equalityInitializer" => &["MissingInitializerFirst", "MissingInitializerSecond"],
        "compound" => &["MissingCompoundFirst", "MissingCompoundLater"],
        "numberLiteral" | "bigintLiteral" | "booleanBitwise" | "symbolShortCircuit" => &[],
        _ => panic!("unclassified native row {name}"),
    }
}

#[test]
fn operators_preserve_later_operand_and_assignment_acquisitions() {
    check(false);
}

#[test]
fn operators_keep_literal_and_symbol_short_circuit_controls() {
    check(true);
}

fn check(controls: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-operator-completion");
    let source = std::fs::read_to_string(root.join("cases.ts")).unwrap();
    let strict = std::fs::read_to_string(root.join("strict.json")).unwrap();
    let loose = std::fs::read_to_string(root.join("loose.json")).unwrap();
    let nocheck = std::fs::read_to_string(root.join("nocheck.json")).unwrap();
    let fixture = Fixture::new(&[
        (
            "native-operators/globals.d.ts",
            include_str!("../fixtures/types/native-operators/globals.d.ts"),
        ),
        ("native-operator-completion/cases.ts", &source),
        ("native-operator-completion/strict.json", &strict),
        ("native-operator-completion/loose.json", &loose),
        ("native-operator-completion/nocheck.json", &nocheck),
    ]);
    let file = fixture.path("native-operator-completion/cases.ts");
    for profile in ["strict", "loose", "nocheck"] {
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        let queries = expected["queries"].as_array().unwrap();
        assert_eq!(queries.len(), 16);
        for reverse in [false, true] {
            let project =
                fixture.load_config(&format!("native-operator-completion/{profile}.json"));
            let mut rows: Vec<_> = queries
                .iter()
                .filter(|row| required(row["name"].as_str().unwrap()).is_empty() == controls)
                .collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let needed = required(name);
                let marker = format!("/*#{name}*/");
                let at = position(&source, source.find(&marker).unwrap() + marker.len());
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert_eq!(
                    row["demandFailures"],
                    serde_json::json!([]),
                    "{profile}/{name}"
                );
                let native_errors = row["errors"].as_array().unwrap();
                assert_eq!(native_errors.len(), needed.len(), "{profile}/{name}");
                let mut locations = Vec::new();
                for (event, token) in native_errors.iter().zip(needed) {
                    let offset = source.find(token).unwrap();
                    assert_eq!(
                        event["operation"], "getTypeFromTypeNode",
                        "{profile}/{name}"
                    );
                    assert!(event["pos"].as_u64().unwrap() as usize <= offset);
                    assert_eq!(
                        event["end"].as_u64().unwrap() as usize,
                        offset + token.len()
                    );
                    locations.push((
                        position(&source, offset),
                        position(&source, offset + token.len()),
                    ));
                }
                for expand in [false, true, false] {
                    let label = format!("{profile}/{name}/{reverse}/{expand}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: file.clone(),
                            position: at,
                        },
                        expand,
                        depth: 1,
                    });
                    let Some((primary, later)) = locations.split_first() else {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{label}"
                        );
                        assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                        assert_eq!(answer.description.expansion.is_some(), expand, "{label}");
                        if let Some(graph) = &answer.description.expansion {
                            assert!(
                                graph.nodes.iter().all(|node| !matches!(
                                    node.data,
                                    TypeNodeData::Unsupported { .. }
                                )),
                                "{label}: {graph:?}"
                            );
                        }
                        continue;
                    };
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
                        SemanticFailureReason::UnresolvedSymbol.into(),
                        "{label}: {error:?}"
                    );
                    assert_eq!(error.path.as_ref(), Some(&file), "{label}: {error:?}");
                    assert_eq!(error.position, Some(primary.0), "{label}: {error:?}");
                    let mut remaining = error.causes.as_slice();
                    for &(start, end) in later {
                        let found = remaining.iter().position(|cause| {
                            cause.reason == SemanticFailureReason::UnresolvedSymbol.into()
                                && cause.location.as_ref().is_some_and(|location| {
                                    location.file == "cases.ts"
                                        && location.start == start
                                        && location.end == end
                                })
                        });
                        let Some(found) = found else {
                            panic!("{label}: missing later acquisition at {start:?}: {error:?}");
                        };
                        remaining = &remaining[found + 1..];
                    }
                }
            }
        }
    }
}
