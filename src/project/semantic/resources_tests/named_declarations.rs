//! Pinned same-file and cross-file overload searches with bounded named metadata.
use super::*;
use crate::references::{ReferenceBaseline, ReferenceOutcome, ReferenceQuery};

#[test]
fn request_resource_named_declaration_native_outcomes() {
    check(None, None);
}

fn check(owner: Option<&'static str>, filters: Option<crate::references::ReferenceFilters>) {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-named-declaration-resources/expected.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 16);
    for row in rows {
        let variant = &row["variant"];
        if filters.is_some() && variant["layout"] != "split" {
            continue;
        }
        if owner.is_some() && variant["layout"] == "split" {
            continue;
        }
        let width = variant["width"].as_u64().unwrap() as usize;
        let name = variant["name"].as_str().unwrap();
        let file = variant["file"].as_str().unwrap();
        let source = row["source"].as_str().unwrap();
        let mut fixture = Fixture::new("let chosen: number = 1;");
        for (file, source) in [
            (
                "globals.d.ts",
                include_str!(
                    "../../../../fixtures/types/native-named-declaration-resources/globals.d.ts"
                ),
            ),
            (
                "a16.d.ts",
                include_str!(
                    "../../../../fixtures/types/native-named-declaration-resources/a16.d.ts"
                ),
            ),
            (
                "z16.d.ts",
                include_str!(
                    "../../../../fixtures/types/native-named-declaration-resources/z16.d.ts"
                ),
            ),
            (
                "a64.d.ts",
                include_str!(
                    "../../../../fixtures/types/native-named-declaration-resources/a64.d.ts"
                ),
            ),
            (
                "z64.d.ts",
                include_str!(
                    "../../../../fixtures/types/native-named-declaration-resources/z64.d.ts"
                ),
            ),
            (file, source),
        ] {
            std::fs::write(fixture.directory.join(file), source).unwrap();
        }
        std::fs::write(
            fixture.directory.join("tsconfig.json"),
            variant["configuration"].to_string(),
        )
        .unwrap();
        fixture.project = Project::load(&fixture.directory.join("tsconfig.json")).unwrap();
        let text = crate::source::SourceText::new(source.to_owned());
        let marker = "/*#chosen*/";
        let request = ReferenceRequest {
            query: ReferenceQuery::At {
                file: fixture.directory.join(file),
                position: text.position(source.find(marker).unwrap() + marker.len()),
            },
            filters: filters.clone().unwrap_or_default(),
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
        let expected:Vec<_>=row["answer"]["nativeRangeGroups"].as_array().unwrap().iter().map(|g|serde_json::json!({"origins":g["origins"].as_array().unwrap().iter().map(|d|&d["location"]).collect::<Vec<_>>(),"occurrences":g["occurrences"]})).collect();
        // Product filters retain the selected seed even when the native search
        // established no group. The frozen normalized seed supplies its origins.
        let expected = if filters.is_some() {
            vec![
                serde_json::json!({"origins":row["answer"]["combinedOrigins"].as_array().unwrap().iter().map(|d|&d["location"]).collect::<Vec<_>>(),"occurrences":[]}),
            ]
        } else {
            expected
        };
        let count: usize = expected
            .iter()
            .map(|g| g["origins"].as_array().unwrap().len())
            .sum();
        assert_eq!(
            count,
            if filters.is_some() {
                width / 2
            } else if variant["layout"] == "split" {
                0
            } else {
                width
            }
        );
        let verify = |answer: ReferenceAnswer| {
            let ReferenceAnswer::Selected(answer) = answer else {
                panic!("unselected {variant}")
            };
            assert_eq!(
                answer.outcome,
                ReferenceOutcome::Complete,
                "{variant}: {answer:?}"
            );
            assert_eq!(answer.coverage.baseline, ReferenceBaseline::Member);
            assert!(answer.coverage.gaps.is_empty());
            let actual:Vec<_>=answer.groups.iter().map(|g|serde_json::json!({"origins":g.declarations.iter().map(|d|&d.location).collect::<Vec<_>>(),"occurrences":g.occurrences.iter().map(|o|&o.location).collect::<Vec<_>>() })).collect();
            assert_eq!(actual, expected, "{variant}");
            for group in &answer.groups {
                for declaration in &group.declarations {
                    assert_eq!(declaration.name, name);
                }
            }
        };
        let (answer, usage) = query(Limits::default()).unwrap();
        verify(answer);
        assert_eq!(usage.depth, 0);

        let healthy_request = TypeRequest::new(TypeQuery::At {
            file: fixture.directory.join(file),
            position: text.position(source.find("healthy").unwrap()),
        });
        healthy(fixture.query_request(&healthy_request, Limits::default()));
    }
}
