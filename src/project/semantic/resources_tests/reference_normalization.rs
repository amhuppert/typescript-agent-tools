//! Combined early/late selection keeps the candidate's native directional identity.
use super::*;
use crate::references::{ReferenceBaseline, ReferenceOutcome, ReferenceQuery};
#[test]
fn request_resource_late_reference_normalization_preserves_native_groups() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-late-merge-resources/expected-references.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 24);
    for row in rows {
        let profile = row["profile"].as_str().unwrap();
        let mut fixture = Fixture::new("let chosen:number=1;");
        for (file, source) in [
            (
                "globals.d.ts",
                include_str!("../../../../fixtures/types/native-late-merge-resources/globals.d.ts"),
            ),
            (
                "16.ts",
                include_str!("../../../../fixtures/types/native-late-merge-resources/16.ts"),
            ),
            (
                "64.ts",
                include_str!("../../../../fixtures/types/native-late-merge-resources/64.ts"),
            ),
            (
                "strict.json",
                include_str!("../../../../fixtures/types/native-late-merge-resources/strict.json"),
            ),
            (
                "nocheck.json",
                include_str!("../../../../fixtures/types/native-late-merge-resources/nocheck.json"),
            ),
            (
                "loose.json",
                include_str!("../../../../fixtures/types/native-late-merge-resources/loose.json"),
            ),
            (
                "loose-nocheck.json",
                include_str!(
                    "../../../../fixtures/types/native-late-merge-resources/loose-nocheck.json"
                ),
            ),
        ] {
            std::fs::write(fixture.directory.join(file), source).unwrap();
        }
        fixture.project =
            Project::load(&fixture.directory.join(format!("{profile}.json"))).unwrap();
        let file = row["at"]["file"].as_str().unwrap();
        let request = ReferenceRequest {
            query: ReferenceQuery::At {
                file: fixture.directory.join(file),
                position: crate::source::SourcePosition {
                    line: row["at"]["line"].as_u64().unwrap() as usize,
                    column: row["at"]["column"].as_u64().unwrap() as usize,
                },
            },
            filters: Default::default(),
        };
        let ReferenceAnswer::Selected(answer) = fixture.project.references(request).unwrap() else {
            panic!("unselected {row}")
        };
        assert_eq!(
            answer.outcome,
            ReferenceOutcome::Complete,
            "{profile}/{}: {answer:?}",
            row["seed"]
        );
        assert_eq!(answer.coverage.baseline, ReferenceBaseline::Member);
        assert!(answer.coverage.gaps.is_empty());
        let actual:Vec<_>=answer.groups.iter().map(|g|serde_json::json!({"origins":g.declarations.iter().map(|d|&d.location).collect::<Vec<_>>(),"occurrences":g.occurrences.iter().map(|o|&o.location).collect::<Vec<_>>() })).collect();
        let expected:Vec<_>=row["normalizedGroups"].as_array().unwrap().iter().map(|g|serde_json::json!({"origins":g["origins"].as_array().unwrap().iter().map(|d|&d["location"]).collect::<Vec<_>>(),"occurrences":g["occurrences"]})).collect();
        assert_eq!(
            actual, expected,
            "{profile}/{}/{}",
            row["width"], row["seed"]
        );
        for group in answer.groups {
            for declaration in group.declarations {
                assert_eq!(declaration.name, "method");
            }
        }
        let source = std::fs::read_to_string(fixture.directory.join(file)).unwrap();
        let text = crate::source::SourceText::new(source.clone());
        let healthy_request = TypeRequest::new(TypeQuery::At {
            file: fixture.directory.join(file),
            position: text.position(source.find("healthy").unwrap()),
        });
        healthy(fixture.query_request(&healthy_request, Limits::default()));
    }
}
