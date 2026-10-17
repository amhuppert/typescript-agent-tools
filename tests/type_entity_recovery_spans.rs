//! resolveEntityName reports its failed head; resolveQualifiedName reports the
//! failed right identifier, including intermediate and optional access forms.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-entity-recovery-spans/cases.ts");

#[test]
fn unresolved_entity_components_keep_exact_native_recovery_locations() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-entity-recovery-spans"),
    );
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-entity-recovery-spans/expected-strict.json"),
        ),
        (
            "nocheck",
            include_str!("../fixtures/types/native-entity-recovery-spans/expected-nocheck.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 10);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let (token, alias) = match name {
                    "single" => ("MissingSingle", false),
                    "head" => ("MissingHead", false),
                    "middle" => ("MissingMiddle", false),
                    "terminal" => ("MissingTerminal", false),
                    "optionalHead" => ("MissingOptional", false),
                    "optionalMiddle" => ("MissingOptionalMiddle", false),
                    "optionalTerminal" => ("MissingOptionalTerminal", false),
                    "typeHead" => ("MissingTypeHead", true),
                    "typeMiddle" => ("MissingTypeMiddle", true),
                    "typeTerminal" => ("MissingTypeTerminal", true),
                    _ => panic!("unclassified native entity variant {name}"),
                };
                assert_eq!(row["errorType"], alias);
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                let errors: Vec<_> = row["acquisitions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|event| event["event"] == "exit" && event["errorType"] == true)
                    .collect();
                assert_eq!(errors.len(), if alias { 3 } else { 2 });
                assert_eq!(errors[0]["operation"], "getTypeFromTypeNode");
                // Alias annotation reads propagate one producer through the
                // alias and variable. Class checking reads its reference twice,
                // so that path additionally replays the cached recovery once.
                let start = SOURCE.find(token).unwrap();
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
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::UnresolvedSymbol.into(),
                        "{label}: {error:?}"
                    );
                    assert_eq!(
                        error.path.as_deref(),
                        Some(fixture.path("cases.ts").as_path())
                    );
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, start)),
                        "{label}: {error:?}"
                    );
                    assert_eq!(
                        error.causes.len(),
                        usize::from(!alias),
                        "{label}: {error:?}"
                    );
                    assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                    for cause in &error.causes {
                        assert_eq!(
                            cause.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{label}: {error:?}"
                        );
                        let location = cause.location.as_ref().unwrap();
                        assert_eq!(location.file, "cases.ts");
                        assert_eq!(
                            location.start,
                            position(SOURCE, start),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            location.end,
                            position(SOURCE, start + token.len()),
                            "{label}: {error:?}"
                        );
                    }
                }
            }
        }
    }
}
