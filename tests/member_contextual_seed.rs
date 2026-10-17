//! Contextual origin grouping retains seed metadata only for filtered results.
mod support;
use support::{Fixture, position};
use typescript_parser::references::{
    ReferenceAnswer, ReferenceFilters, ReferenceOutcome, ReferenceQuery, ReferenceRequest,
};
const SOURCE: &str = include_str!("../fixtures/types/native-contextual-seed/cases.ts");
#[test]
fn unfiltered_contextual_seed_has_only_established_native_groups() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-contextual-seed"),
    );
    let project = fixture.load();
    let offset = |marker| {
        let tag = format!("/*#{marker}*/");
        SOURCE.find(&tag).unwrap() + tag.len()
    };
    let result = project
        .references(ReferenceRequest {
            query: ReferenceQuery::At {
                file: fixture.path("cases.ts"),
                position: position(SOURCE, offset("seed")),
            },
            filters: ReferenceFilters::default(),
        })
        .unwrap();
    let ReferenceAnswer::Selected(result) = result else {
        panic!("{result:?}")
    };
    assert_eq!(result.outcome, ReferenceOutcome::Complete);
    assert_eq!(result.groups.len(), 1, "{result:?}");
    assert_eq!(result.groups[0].declarations.len(), 1);
    assert_eq!(
        result.groups[0].declarations[0].location.start,
        position(SOURCE, offset("origin"))
    );
    assert_eq!(
        result.groups[0]
            .occurrences
            .iter()
            .map(|o| o.location.start)
            .collect::<Vec<_>>(),
        ["origin", "seed", "call"].map(|m| position(SOURCE, offset(m)))
    );
}

#[test]
fn filtered_contextual_seed_retains_its_declaration_metadata() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-contextual-seed"),
    );
    let project = fixture.load();
    let offset = |marker| {
        let tag = format!("/*#{marker}*/");
        SOURCE.find(&tag).unwrap() + tag.len()
    };
    for exclude_tests in [false, true] {
        let result = project
            .references(ReferenceRequest {
                query: ReferenceQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, offset("seed")),
                },
                filters: ReferenceFilters {
                    calls: true,
                    exclude_tests,
                },
            })
            .unwrap();
        let ReferenceAnswer::Selected(result) = result else {
            panic!("{result:?}")
        };
        assert_eq!(result.outcome, ReferenceOutcome::Complete);
        assert_eq!(result.groups.len(), 2, "{result:?}");
        assert_eq!(result.groups[0].occurrences.len(), 1);
        assert_eq!(
            result.groups[0].occurrences[0].location.start,
            position(SOURCE, offset("call"))
        );
        assert!(result.groups[1].occurrences.is_empty());
        assert_eq!(
            result.groups[1].declarations[0].location.start,
            position(SOURCE, offset("seed"))
        );
    }
}
