//! Startup identity observations and actual global-dependent factories have
//! different native schedules. The public query must preserve that boundary.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-fixed-globals/cases.ts");

#[test]
fn fixed_global_identity_reads_do_not_turn_unrelated_types_into_factory_recovery() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-fixed-globals"),
    );
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-fixed-globals/expected.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 30);
    for reverse in [false, true] {
        for profile in [
            "healthy",
            "absent",
            "arity",
            "recovery",
            "arrayOnly",
            "readonlyRecovery",
        ] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut selected: Vec<_> = rows
                .iter()
                .filter(|row| row["profile"] == profile)
                .collect();
            if reverse {
                selected.reverse();
            }
            for row in selected {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let offset = SOURCE.find(&marker).unwrap() + marker.len();
                assert_eq!(row["errorType"], false);
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, offset),
                    });
                    request.expand = expand;
                    request.depth = 2;
                    let answer = project.type_of(request);
                    if matches!(profile, "absent" | "arity" | "recovery")
                        && matches!(name, "array" | "readonly")
                    {
                        // Native returns its empty-object recovery sentinel.
                        // The product contract preserves the consumed global
                        // failure instead of publishing that sentinel as {}.
                        assert_eq!(row["summary"], "{}");
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                        };
                        assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
                        assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
                        assert_eq!(
                            error.message,
                            "global type 'Array' dependency consumed compiler recovery"
                        );
                        // createArrayTypeEx consumes the ArrayType node itself;
                        // the enclosing readonly operator is not the consumer.
                        let array_node = offset + SOURCE[offset..].find("number[]").unwrap();
                        assert_eq!(error.position, Some(position(SOURCE, array_node)));
                        assert_eq!(
                            error.path.as_deref(),
                            Some(fixture.path("cases.ts").canonicalize().unwrap().as_path())
                        );
                        assert_eq!(error.causes.len(), 1, "{profile}/{name}: {error:?}");
                        let cause = &error.causes[0];
                        if profile == "absent" {
                            assert_eq!(
                                cause.reason,
                                SemanticFailureReason::UnresolvedSymbol.into()
                            );
                            assert_eq!(cause.location, None);
                        } else {
                            assert_eq!(cause.reason, SemanticFailureReason::ErrorType.into());
                            let source =
                                std::fs::read_to_string(fixture.path(&format!("{profile}.d.ts")))
                                    .unwrap();
                            let anchor = source.find("Array").unwrap();
                            let location = cause.location.as_ref().unwrap();
                            assert_eq!(location.file, format!("{profile}.d.ts"));
                            assert_eq!(location.start, position(&source, anchor));
                            assert_eq!(location.end, position(&source, anchor + "Array".len()));
                        }
                        assert_eq!(
                            error.causes[0].message,
                            match profile {
                                "absent" => "Cannot find global type 'Array'.",
                                "arity" => "Global type 'Array' must have 1 type parameter(s).",
                                "recovery" =>
                                    "Global type 'Array' must be a class or interface type.",
                                _ => unreachable!(),
                            }
                        );
                        assert_eq!(error.omitted_causes, None);
                    } else {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                        };
                        assert_eq!(
                            answer.outcome,
                            TypeOutcome::Found,
                            "{profile}/{name}: {answer:?}"
                        );
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{profile}/{name}"
                        );
                        assert!(answer.issues.is_empty(), "{profile}/{name}: {answer:?}");
                    }
                }
            }
        }
    }
}
