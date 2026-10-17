//! Per-file helpers retain their own native module and requested-bit identity.
use super::*;

fn identity(mode: &str) {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-emit-helper-cache-resources/identity/expected.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 12);
    let rows: Vec<_> = rows
        .iter()
        .filter(|row| row["variant"]["mode"] == mode)
        .collect();
    assert_eq!(rows.len(), 4);
    for row in rows {
        let variant = &row["variant"];
        let label = variant["config"].as_str().unwrap();
        let file = variant["file"].as_str().unwrap();
        let source = variant["source"].as_str().unwrap();
        let mut fixture = Fixture::new("let chosen: number = 1;");
        for dependency in variant["dependencies"].as_array().unwrap() {
            let path = fixture.directory.join(dependency["file"].as_str().unwrap());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, dependency["source"].as_str().unwrap()).unwrap();
        }
        std::fs::write(fixture.directory.join("globals.d.ts"), include_str!(
            "../../../../fixtures/types/native-emit-helper-cache-resources/identity/globals.d.ts"
        )).unwrap();
        std::fs::write(fixture.directory.join(file), source).unwrap();
        std::fs::write(
            fixture.directory.join("tsconfig.json"),
            variant["configuration"].to_string(),
        )
        .unwrap();
        fixture.project = Project::load(&fixture.directory.join("tsconfig.json")).unwrap();
        let request = TypeRequest::new(TypeQuery::At {
            file: fixture.directory.join(file),
            position: SourcePosition {
                line: row["at"]["line"].as_u64().unwrap() as usize,
                column: row["at"]["column"].as_u64().unwrap() as usize,
            },
        });
        let answer = fixture
            .query_request(&request, Limits::default())
            .map(|value| value.0);
        if mode == "both-good" {
            assert_eq!(row["answer"]["diagnosticCodes"], serde_json::json!([]));
            let Ok(TypeAnswer::Selected(answer)) = answer else {
                panic!("{label}: {answer:?}");
            };
            assert_eq!(answer.outcome, TypeOutcome::Found);
            assert_eq!(
                answer.description.summary,
                row["answer"]["summary"].as_str().unwrap()
            );
            assert!(answer.issues.is_empty());
        } else {
            // The native helper signature emits TS2304 yet keeps number. The
            // existing directed-failure policy retains that consumed recovery.
            assert_eq!(row["answer"]["diagnosticCodes"], serde_json::json!([2304]));
            assert_eq!(row["answer"]["errorType"], false);
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("lost per-file helper recovery {label}: {answer:?}");
            };
            let helper = "bad/node_modules/tslib/index.d.ts";
            let text = include_str!(
                "../../../../fixtures/types/native-emit-helper-cache-resources/identity/bad/node_modules/tslib/index.d.ts"
            );
            let start = text.find("MissingGet").unwrap();
            let text = crate::source::SourceText::new(text.to_owned());
            let expected = crate::references::Location {
                file: helper.into(),
                start: text.position(start),
                end: text.position(start + "MissingGet".len()),
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
                error.path,
                Some(fixture.directory.join(helper).canonicalize().unwrap())
            );
            assert_eq!(error.position, Some(expected.start));
            assert_eq!(error.exact_location.as_ref(), Some(&expected));
            assert!(error.message.contains("MissingGet"));
            assert!(error.omitted_causes.is_none());
            for cause in error.causes {
                assert_eq!(cause.reason, SemanticFailureReason::UnresolvedSymbol.into());
                assert_eq!(cause.location.as_ref(), Some(&expected));
            }
        }
        let text = crate::source::SourceText::new(source.to_owned());
        let successor = TypeRequest::new(TypeQuery::At {
            file: fixture.directory.join(file),
            position: text.position(source.find("healthy").unwrap()),
        });
        healthy(fixture.query_request(&successor, Limits::default()));
    }
}

#[test]
fn request_resource_emit_helper_good_then_bad() {
    identity("good-bad");
}
#[test]
fn request_resource_emit_helper_bad_then_good() {
    identity("bad-good");
}
#[test]
fn request_resource_emit_helper_distinct_good_modules() {
    identity("both-good");
}
