//! Replaying an installed native return copies its retained completion evidence.
use super::*;

fn cases() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-recovery-observation-resources/direct-callee/expected.json"
    )).unwrap();
    assert_eq!(rows.len(), 32);
    for row in rows {
        let variant = &row["variant"];
        let label = variant["config"].as_str().unwrap();
        let file = variant["file"].as_str().unwrap();
        let source = row["source"].as_str().unwrap();
        let count = variant["width"].as_u64().unwrap() as usize;
        let mut fixture = Fixture::new("let chosen: number = 1;");
        std::fs::write(fixture.directory.join("globals.d.ts"), include_str!(
            "../../../../fixtures/types/native-recovery-observation-resources/direct-callee/globals.d.ts"
        )).unwrap();
        std::fs::write(fixture.directory.join(file), source).unwrap();
        std::fs::write(
            fixture.directory.join("tsconfig.json"),
            variant["configuration"].to_string(),
        )
        .unwrap();
        fixture.project = Project::load(&fixture.directory.join("tsconfig.json")).unwrap();
        let text = crate::source::SourceText::new(source.to_owned());
        let call_offsets: Vec<_> = source
            .match_indices("(null! as Mixed)()")
            .map(|(offset, _)| offset)
            .collect();
        assert_eq!(call_offsets.len(), count);
        let native_position = |index: usize, endpoint: &str| SourcePosition {
            line: row["callLocations"][index][endpoint]["line"]
                .as_u64()
                .unwrap() as usize,
            column: row["callLocations"][index][endpoint]["column"]
                .as_u64()
                .unwrap() as usize,
        };
        let path = std::fs::canonicalize(fixture.directory.join(file)).unwrap();
        let request = TypeRequest::new(TypeQuery::At {
            file: path.clone(),
            position: SourcePosition {
                line: row["at"]["line"].as_u64().unwrap() as usize,
                column: row["at"]["column"].as_u64().unwrap() as usize,
            },
        });
        let query = |limits| {
            worker_with_limits(&fixture.project, limits, |parser, control| {
                // Keep the public query's failure as the value so the test can inspect
                // successful charges before its return. Private resource aborts still
                // terminate the worker and become the outer error.
                let answer = type_of_here(&fixture.project, &request, parser, control.clone());
                Ok((answer, control.usage()))
            })
        };
        let verify = |answer: Result<TypeAnswer, AnalysisError>| {
            assert_eq!(row["answer"]["summary"], "17");
            assert_eq!(row["answer"]["errorType"], false);
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("recovery lost {label}: {answer:?}");
            };
            assert_eq!(
                error.kind,
                SemanticErrorKind::InvalidSemantic,
                "{label}: {error:?}"
            );
            assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
            assert_eq!(
                error.message,
                "native unknown signature return consumes errorType recovery"
            );
            assert_eq!(error.path.as_ref(), Some(&path));
            assert_eq!(error.position, Some(native_position(0, "start")));
            assert_eq!(
                error.exact_location,
                Some(Location {
                    file: file.into(),
                    start: native_position(0, "start"),
                    end: native_position(0, "end")
                })
            );
            assert!(error.unbound_recovery.is_none());
            assert_eq!(
                error.causes.len(),
                (count - 1).min(16),
                "{label}: {error:?}"
            );
            assert_eq!(error.omitted_causes, (count > 17).then(|| count - 17));
            for (index, cause) in error.causes.iter().enumerate() {
                assert_eq!(cause.reason, SemanticFailureReason::ErrorType.into());
                assert_eq!(cause.kind, Some(SemanticErrorKind::InvalidSemantic));
                assert_eq!(cause.message, error.message);
                assert_eq!(
                    cause.location,
                    Some(Location {
                        file: file.into(),
                        start: native_position(index + 1, "start"),
                        end: native_position(index + 1, "end")
                    })
                );
                assert!(cause.unbound_recovery.is_none());
            }
        };
        let (answer, usage) = query(Limits::default()).unwrap();
        verify(answer);
        assert_eq!(usage.depth, 0);

        let successor = TypeRequest::new(TypeQuery::At {
            file: path,
            position: text.position(source.find("healthy").unwrap()),
        });
        healthy(fixture.query_request(&successor, Limits::default()));
    }
}
#[test]
fn request_resource_completed_return_native_outcomes() {
    cases();
}
