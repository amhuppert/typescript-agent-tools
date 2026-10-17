//! A nil getTypeReferenceName skips expression evaluation, while written type
//! arguments are still checked before the reference's errorType acquisition.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-class-reference-nil/cases.ts");

#[test]
fn nonentity_reference_names_preserve_type_argument_recovery_order() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-class-reference-nil"),
    );
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-class-reference-nil/expected-strict.json"),
        ),
        (
            "nocheck",
            include_str!("../fixtures/types/native-class-reference-nil/expected-nocheck.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 5);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let (reference, summary) = match name {
                    "plain" => ("makeBase()", "280"),
                    "argument" => ("makeBase()<number>", "281"),
                    "missing" => ("makeBase()<MissingNilArgument>", "282"),
                    "element" => ("MissingUntouchedReceiver[\"Base\"]", "283"),
                    "parenthesized" => ("(MissingUntouchedParenthesized)", "284"),
                    _ => panic!("unclassified native reference variant {name}"),
                };
                assert_eq!(row["summary"], summary);
                assert_eq!(row["errorType"], false);
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                let full_start =
                    SOURCE.find(&format!("implements {reference}")).unwrap() + "implements ".len();
                let mut expected = Vec::new();
                if name == "missing" {
                    expected.push((
                        SemanticFailureReason::UnresolvedSymbol,
                        SOURCE.find("MissingNilArgument").unwrap(),
                        "MissingNilArgument".len(),
                    ));
                }
                expected.extend([
                    (
                        SemanticFailureReason::ErrorType,
                        full_start,
                        reference.len(),
                    ),
                    (
                        SemanticFailureReason::ErrorType,
                        full_start,
                        reference.len(),
                    ),
                ]);
                // Native Pos includes trivia. Normalize only leading source
                // trivia here; the public exact location starts at the token.
                let native: Vec<_> = row["acquisitions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|event| event["event"] == "exit" && event["errorType"] == true)
                    .map(|event| {
                        assert_eq!(event["operation"], "getTypeFromTypeNode");
                        let start = event["pos"].as_u64().unwrap() as usize;
                        let end = event["end"].as_u64().unwrap() as usize;
                        let trimmed = SOURCE[start..end].trim_start();
                        (end - trimmed.len(), trimmed.len())
                    })
                    .collect();
                assert_eq!(
                    native,
                    expected
                        .iter()
                        .map(|(_, start, len)| (*start, *len))
                        .collect::<Vec<_>>()
                );
                let marker = format!("/*#{name}*/");
                let offset = SOURCE.find(&marker).unwrap() + marker.len();
                for expand in [false, true] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 1,
                    });
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{label}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{label}: {error:?}"
                    );
                    assert_eq!(error.reason, expected[0].0.into(), "{label}: {error:?}");
                    assert_eq!(
                        error.path.as_deref(),
                        Some(fixture.path("cases.ts").as_path())
                    );
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, expected[0].1)),
                        "{label}: {error:?}"
                    );
                    assert_eq!(error.causes.len(), expected.len() - 1, "{label}: {error:?}");
                    assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                    for (cause, (reason, start, len)) in error.causes.iter().zip(&expected[1..]) {
                        assert_eq!(cause.reason, (*reason).into(), "{label}: {error:?}");
                        let location = cause.location.as_ref().unwrap();
                        assert_eq!(location.file, "cases.ts");
                        assert_eq!(
                            location.start,
                            position(SOURCE, *start),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            location.end,
                            position(SOURCE, start + len),
                            "{label}: {error:?}"
                        );
                    }
                }
            }
        }
    }
}
