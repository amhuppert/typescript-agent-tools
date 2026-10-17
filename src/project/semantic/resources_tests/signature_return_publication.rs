//! Publishing a recovered signature return preserves its evidence.
use super::*;

fn cases() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-located-return-copy-resources/expected.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 32);
    for row in rows {
        let variant = &row["variant"];
        let label = variant["config"].as_str().unwrap();
        let file = variant["file"].as_str().unwrap();
        let source = row["source"].as_str().unwrap();
        let count = variant["width"].as_u64().unwrap() as usize;
        let mut fixture = Fixture::new("let chosen: number = 1;");
        std::fs::write(
            fixture.directory.join("globals.d.ts"),
            include_str!(
                "../../../../fixtures/types/native-located-return-copy-resources/globals.d.ts"
            ),
        )
        .unwrap();
        std::fs::write(fixture.directory.join(file), source).unwrap();
        std::fs::write(
            fixture.directory.join("tsconfig.json"),
            variant["configuration"].to_string(),
        )
        .unwrap();
        fixture.project = Project::load(&fixture.directory.join("tsconfig.json")).unwrap();
        let text = crate::source::SourceText::new(source.to_owned());
        let native_location = |index: usize| {
            let position = |end: &str| SourcePosition {
                line: row["failureLocations"][index][end]["line"]
                    .as_u64()
                    .unwrap() as usize,
                column: row["failureLocations"][index][end]["column"]
                    .as_u64()
                    .unwrap() as usize,
            };
            Location {
                file: file.into(),
                start: position("start"),
                end: position("end"),
            }
        };
        assert_eq!(source.matches("(null! as F)()").count(), count);
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
                let answer = type_of_here(&fixture.project, &request, parser, control.clone());
                Ok((answer, control.usage()))
            })
        };
        let verify = |answer: Result<TypeAnswer, AnalysisError>| {
            assert_eq!(row["answer"]["summary"], "17");
            assert_eq!(row["answer"]["errorType"], false);
            assert_eq!(
                row["answer"]["diagnosticCodes"],
                serde_json::json!([2304, 2304])
            );
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("located recovery lost {label}: {answer:?}");
            };
            assert_eq!(
                error.kind,
                SemanticErrorKind::InvalidSemantic,
                "{label}: {error:?}"
            );
            assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
            assert!(
                error
                    .message
                    .contains(variant["missingNames"][0].as_str().unwrap())
            );
            assert_eq!(error.path.as_ref(), Some(&path));
            assert_eq!(error.position, Some(native_location(0).start));
            assert_eq!(error.exact_location, Some(native_location(0)));
            assert!(error.unbound_recovery.is_none());
            let causes = 2 * count - 1;
            assert_eq!(error.causes.len(), causes.min(16), "{label}: {error:?}");
            assert_eq!(
                error.omitted_causes,
                (causes > 16).then(|| causes - 16),
                "{label}: {error:?}"
            );
            for (index, cause) in error.causes.iter().enumerate() {
                let declaration = (index + 1) % 2;
                assert_eq!(cause.reason, SemanticFailureReason::UnresolvedSymbol.into());
                assert_eq!(cause.kind, Some(SemanticErrorKind::InvalidSemantic));
                assert!(
                    cause
                        .message
                        .contains(variant["missingNames"][declaration].as_str().unwrap()),
                    "{label}: {cause:?}"
                );
                assert_eq!(cause.location, Some(native_location(declaration)));
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
fn request_resource_signature_return_publication_native_outcomes() {
    cases();
}
