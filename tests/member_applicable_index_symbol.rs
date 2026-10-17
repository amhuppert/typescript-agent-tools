//! Applicable index symbols are real native results, independent of a member seed.
mod support;
use support::{Fixture, position};
use typescript_parser::references::{
    ReferenceAnswer, ReferenceFilters, ReferenceOutcome, ReferenceQuery, ReferenceRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-applicable-index-symbol/cases.ts");

#[test]
fn indexed_candidates_are_known_unrelated_members() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-applicable-index-symbol/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-applicable-index-symbol/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-applicable-index-symbol/loose.json"),
        ),
    ]);
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for marker in ["seed", "healthy"] {
            let tag = format!("/*#{marker}*/");
            let answer = project
                .references(ReferenceRequest {
                    query: ReferenceQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
                    },
                    filters: ReferenceFilters::default(),
                })
                .unwrap();
            let ReferenceAnswer::Selected(result) = answer else {
                panic!("{config}/{marker}: {answer:?}");
            };
            assert_eq!(
                result.outcome,
                ReferenceOutcome::Complete,
                "{config}/{marker}: {result:?}"
            );
            assert_eq!(result.coverage.status, ReferenceOutcome::Complete);
            assert!(result.coverage.gaps.is_empty());
            assert_eq!(result.groups.len(), 1);
            assert_eq!(result.groups[0].occurrences.len(), 2);
        }
    }
}
