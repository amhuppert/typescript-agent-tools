//! Native property recovery precedes a source-free relation-depth child.
use super::*;
fn cases() {
    for (name, depth, source) in [
        (
            "before99",
            99,
            include_str!("../../../../fixtures/types/native-relation-recovery/before99.ts"),
        ),
        (
            "before100",
            100,
            include_str!("../../../../fixtures/types/native-relation-recovery/before100.ts"),
        ),
        (
            "before101",
            101,
            include_str!("../../../../fixtures/types/native-relation-recovery/before101.ts"),
        ),
    ] {
        for profile in ["strict", "loose", "exact"] {
            let mut fixture = Fixture::new("let chosen: number = 1;");
            let file = format!("{name}.ts");
            std::fs::write(fixture.directory.join(&file), source).unwrap();
            let configuration = serde_json::json!({"compilerOptions":{"strict":profile!="loose","strictNullChecks":profile!="loose","exactOptionalPropertyTypes":profile=="exact","noLib":true},"files":[file]});
            std::fs::write(
                fixture.directory.join("tsconfig.json"),
                configuration.to_string(),
            )
            .unwrap();
            fixture.project = Project::load(&fixture.directory.join("tsconfig.json")).unwrap();
            let text = crate::source::SourceText::new(source.into());
            let start = source.find("MissingBefore").unwrap();
            let primary = Location {
                file: file.clone(),
                start: text.position(start),
                end: text.position(start + "MissingBefore".len()),
            };
            let start = source.rfind("source)").unwrap();
            let child = Location {
                file: file.clone(),
                start: text.position(start),
                end: text.position(start + "source".len()),
            };
            let path = std::fs::canonicalize(fixture.directory.join(&file)).unwrap();
            let request = TypeRequest::new(TypeQuery::At {
                file: path.clone(),
                position: text.position(source.find("selected =").unwrap()),
            });
            let query = |limits| {
                worker_with_limits(&fixture.project, limits, |parser, control| {
                    let answer = type_of_here(&fixture.project, &request, parser, control.clone());
                    Ok((answer, control.usage()))
                })
            };
            let verify = |answer: Result<TypeAnswer, AnalysisError>| {
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("lost native recovery: {answer:?}");
                };
                assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
                assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
                assert!(error.message.contains("MissingBefore"));
                assert_eq!(error.path.as_ref(), Some(&path));
                assert_eq!(error.position, Some(primary.start));
                assert_eq!(error.exact_location, Some(primary.clone()));
                assert!(error.unbound_recovery.is_none());
                assert_eq!(error.causes.len(), usize::from(depth >= 100));
                assert!(error.omitted_causes.is_none());
                if depth >= 100 {
                    let cause = &error.causes[0];
                    assert_eq!(cause.location, Some(child.clone()));
                    assert_eq!(
                        cause.reason,
                        SemanticFailureReason::RelationDepthLimit.into()
                    );
                    assert_eq!(
                        cause.message,
                        "native relation stack depth limit was consumed"
                    );
                    assert_eq!(cause.kind, Some(SemanticErrorKind::InvalidSemantic));
                    assert!(cause.unbound_recovery.is_none());
                }
            };
            let (answer, usage) = query(Limits::default()).unwrap();
            verify(answer);
            assert_eq!(usage.depth, 0);

            let successor = TypeRequest::new(TypeQuery::At {
                file: path,
                position: text.position(source.find("independent:").unwrap()),
            });
            let (TypeAnswer::Selected(answer), _) = fixture
                .query_request(&successor, Limits::default())
                .unwrap()
            else {
                panic!("healthy successor unselected");
            };
            assert_eq!(answer.outcome, TypeOutcome::Found);
            assert_eq!(answer.description.summary, "string");
            assert!(answer.issues.is_empty());
        }
    }
}
#[test]
fn request_resource_recovery_child_native_outcomes() {
    cases();
}
