//! Compiler-backed import-type, ambient and global branch outcomes.
use super::*;
use crate::references::{ReferenceOutcome, ReferenceQuery};
const FILES: &[(&str, &str)] = &[
    (
        "globals.d.ts",
        include_str!("../../../../fixtures/types/native-import-branch-resources/globals.d.ts"),
    ),
    (
        "factory.ts",
        include_str!("../../../../fixtures/types/native-import-branch-resources/factory.ts"),
    ),
    (
        "default.ts",
        include_str!("../../../../fixtures/types/native-import-branch-resources/default.ts"),
    ),
    (
        "types.ts",
        include_str!("../../../../fixtures/types/native-import-branch-resources/types.ts"),
    ),
    (
        "ambient.d.ts",
        include_str!("../../../../fixtures/types/native-import-branch-resources/ambient.d.ts"),
    ),
    (
        "ambient-use.ts",
        include_str!("../../../../fixtures/types/native-import-branch-resources/ambient-use.ts"),
    ),
    (
        "global-module.d.ts",
        include_str!(
            "../../../../fixtures/types/native-import-branch-resources/global-module.d.ts"
        ),
    ),
    (
        "global-use.ts",
        include_str!("../../../../fixtures/types/native-import-branch-resources/global-use.ts"),
    ),
];
fn configuration(profile: &str) -> &'static str {
    match profile {
        "strict" => {
            include_str!("../../../../fixtures/types/native-import-branch-resources/strict.json")
        }
        "nocheck" => {
            include_str!("../../../../fixtures/types/native-import-branch-resources/nocheck.json")
        }
        "loose" => {
            include_str!("../../../../fixtures/types/native-import-branch-resources/loose.json")
        }
        "loose-nocheck" => include_str!(
            "../../../../fixtures/types/native-import-branch-resources/loose-nocheck.json"
        ),
        _ => unreachable!(),
    }
}
#[test]
fn request_resource_import_native_branch_outcomes() {
    check_matrix(Matrix {
        files: FILES,
        expected: include_str!(
            "../../../../fixtures/types/native-import-branch-resources/expected.json"
        ),
        configuration,
    });
}
pub(super) struct Matrix {
    pub(super) files: &'static [(&'static str, &'static str)],
    pub(super) expected: &'static str,
    pub(super) configuration: fn(&str) -> &'static str,
}
pub(super) fn check_matrix(matrix: Matrix) {
    let rows: Vec<serde_json::Value> = serde_json::from_str(matrix.expected).unwrap();
    for row in rows {
        let profile = row["config"].as_str().unwrap();
        let mut fixture = Fixture::new("let chosen: number = 1;");
        for &(file, text) in matrix.files {
            std::fs::write(fixture.directory.join(file), text).unwrap();
        }
        std::fs::write(
            fixture.directory.join("tsconfig.json"),
            (matrix.configuration)(profile),
        )
        .unwrap();
        fixture.project = Project::load(&fixture.directory.join("tsconfig.json")).unwrap();
        let file = row["file"].as_str().unwrap();
        let source = matrix
            .files
            .iter()
            .find(|(name, _)| *name == file)
            .unwrap()
            .1;
        let tag = format!("/*#{}*/", row["marker"].as_str().unwrap());
        let request = ReferenceRequest {
            query: ReferenceQuery::At {
                file: fixture.directory.join(file),
                position: crate::source::SourceText::new(source.to_owned())
                    .position(source.find(&tag).unwrap() + tag.len()),
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
        let ReferenceAnswer::Selected(answer) = answer else {
            panic!("unselected {row}")
        };
        assert_eq!(
            answer.outcome,
            ReferenceOutcome::Complete,
            "{profile}/{file}: {answer:?}"
        );
        assert!(answer.coverage.gaps.is_empty());
        let actual: Vec<_> = answer.groups.iter().map(|group| serde_json::json!({
            "origins": group.declarations.iter().map(|d| &d.location).collect::<Vec<_>>(),
            "occurrences": group.occurrences.iter().map(|o| &o.location).collect::<Vec<_>>(),
        })).collect();
        let expected: Vec<_> = row["groups"].as_array().unwrap().iter().map(|group| serde_json::json!({
            "origins": group["origins"].as_array().unwrap().iter().map(|d| &d["location"]).collect::<Vec<_>>(),
            "occurrences": group["occurrences"],
        })).collect();
        assert_eq!(actual, expected, "{profile}/{file}");
    }
}
