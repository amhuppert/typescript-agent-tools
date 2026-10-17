//! Pinned groups with documented candidate failures preserve every blocked site.
use super::*;
use crate::references::{ReferenceBaseline, ReferenceOutcome, ReferenceQuery};
#[test]
fn request_resource_reference_gap_native_outcomes() {
    check();
}

fn check() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-reference-gap-resources/expected.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 32);
    for row in rows {
        let variant = &row["variant"];
        let source = row["source"].as_str().unwrap();
        let file = variant["file"].as_str().unwrap();
        let count = variant["width"].as_u64().unwrap() as usize;
        let mut fixture = Fixture::new("let chosen: number = 1;");
        std::fs::write(fixture.directory.join(file), source).unwrap();
        std::fs::write(
            fixture.directory.join("globals.d.ts"),
            include_str!("../../../../fixtures/types/native-reference-gap-resources/globals.d.ts"),
        )
        .unwrap();
        std::fs::write(
            fixture.directory.join("tsconfig.json"),
            variant["configuration"].to_string(),
        )
        .unwrap();
        fixture.project = Project::load(&fixture.directory.join("tsconfig.json")).unwrap();
        let text = crate::source::SourceText::new(source.to_owned());
        let marker = "/*#declaration*/";
        let request = ReferenceRequest {
            query: ReferenceQuery::At {
                file: fixture.directory.join(file),
                position: text.position(source.find(marker).unwrap() + marker.len()),
            },
            filters: crate::references::ReferenceFilters::default(),
        };
        let query = |limits| {
            worker_with_limits(&fixture.project, limits, |parser, control| {
                let mut evaluator = evaluation::Evaluator::new(
                    CheckerInputs::from_project(&fixture.project, Some(&control)),
                    parser,
                );
                evaluator.ensure_initialized()?;
                let answer = reference_search::search(&fixture.project, &request, &mut evaluator)?;
                Ok((answer, control.usage()))
            })
        };
        let (answer, usage) = query(Limits::default()).unwrap();
        assert_eq!(usage.depth, 0);
        let verify = |answer: ReferenceAnswer| {
            let ReferenceAnswer::Selected(answer) = answer else {
                panic!("unselected {variant}")
            };
            assert_eq!(answer.coverage.baseline, ReferenceBaseline::Member);
            assert_eq!(
                answer.outcome,
                ReferenceOutcome::Partial,
                "{variant}: {answer:?}"
            );
            let actual:Vec<_>=answer.groups.iter().map(|g|serde_json::json!({"origins":g.declarations.iter().map(|d|&d.location).collect::<Vec<_>>(),"occurrences":g.occurrences.iter().map(|o|&o.location).collect::<Vec<_>>() })).collect();
            let expected:Vec<_>=row["answer"]["nativeRangeGroups"].as_array().unwrap().iter().map(|g|serde_json::json!({"origins":g["origins"].as_array().unwrap().iter().map(|d|&d["location"]).collect::<Vec<_>>(),"occurrences":g["occurrences"]})).collect();
            assert_eq!(actual, expected, "{variant}");
            assert_eq!(
                answer.coverage.gaps.len(),
                count,
                "{variant}: {:?}",
                answer.coverage.gaps
            );
            for (index, gap) in answer.coverage.gaps.iter().enumerate() {
                assert_eq!(
                    gap.reason,
                    SemanticFailureReason::UnresolvedSymbol,
                    "{variant}: {gap:?}"
                );
                let name = variant["name"].as_str().unwrap();
                let receiver = source.match_indices(name).nth(index).unwrap().0;
                assert_eq!(
                    gap.location,
                    Location {
                        file: file.into(),
                        start: text.position(receiver + name.len() + 1),
                        end: text.position(receiver + name.len() + 1 + "chosen".len())
                    }
                );
                assert!(
                    gap.message.contains(name)
                        || gap.causes.iter().any(|c| c.message.contains(name)),
                    "lost exact missing name: {gap:?}"
                );
                assert!(!gap.causes.is_empty(), "missing cause metadata: {gap:?}");
                for cause in &gap.causes {
                    assert_eq!(cause.reason, SemanticFailureReason::UnresolvedSymbol);
                    assert_eq!(
                        cause.location,
                        Some(Location {
                            file: file.into(),
                            start: text.position(receiver),
                            end: text.position(receiver + name.len())
                        })
                    );
                    assert!(
                        cause.message.contains(name),
                        "lost cause identity: {cause:?}"
                    );
                }
                assert_eq!(gap.omitted_causes, None);
            }
        };
        verify(answer);

        let successor = TypeRequest::new(TypeQuery::At {
            file: fixture.directory.join(file),
            position: text.position(source.find("healthy").unwrap()),
        });
        healthy(fixture.query_request(&successor, Limits::default()));
    }
}
