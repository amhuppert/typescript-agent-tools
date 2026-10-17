//! Raw Alias lookup, lazy dependency resolution and navigation remain separate.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::references::ReferenceQuery;
use typescript_parser::types::{
    TypeAnswer, TypeKey, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};
const SOURCE: &str = include_str!("../fixtures/types/native-qualified-alias-boundaries/cases.ts");
const HIDDEN: &str = include_str!(
    "../fixtures/types/native-qualified-alias-boundaries/node_modules/hidden/index.d.ts"
);
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-qualified-alias-boundaries/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-qualified-alias-boundaries/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-qualified-alias-boundaries/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-qualified-alias-boundaries/exact.json"),
        ),
        (
            "target.ts",
            include_str!("../fixtures/types/native-qualified-alias-boundaries/target.ts"),
        ),
        (
            "named.ts",
            include_str!("../fixtures/types/native-qualified-alias-boundaries/named.ts"),
        ),
        (
            "star.ts",
            include_str!("../fixtures/types/native-qualified-alias-boundaries/star.ts"),
        ),
        (
            "value.json",
            include_str!("../fixtures/types/native-qualified-alias-boundaries/value.json"),
        ),
        (
            "node_modules/hidden/package.json",
            include_str!(
                "../fixtures/types/native-qualified-alias-boundaries/node_modules/hidden/package.json"
            ),
        ),
        ("node_modules/hidden/index.d.ts", HIDDEN),
        (
            "node_modules/hidden/target.d.ts",
            include_str!(
                "../fixtures/types/native-qualified-alias-boundaries/node_modules/hidden/target.d.ts"
            ),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool, depth: usize) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth,
    }
}
fn healthy(
    project: &typescript_parser::project::Project,
    fixture: &Fixture,
    marker: &str,
    expand: bool,
    depth: usize,
    summary: &str,
) {
    let answer = project.type_of(request(fixture, marker, expand, depth));
    let Ok(TypeAnswer::Selected(result)) = answer else {
        panic!("{marker}/{expand}/{depth}: {answer:?}");
    };
    assert_eq!(
        result.outcome,
        TypeOutcome::Found,
        "{marker}/{expand}/{depth}: {result:?}"
    );
    assert_eq!(
        result.description.summary, summary,
        "{marker}/{expand}/{depth}"
    );
    assert!(
        result.issues.is_empty(),
        "{marker}/{expand}/{depth}: {:?}",
        result.issues
    );
}
#[test]
fn raw_export_aliases_keep_the_supplied_argument_list_before_defaults_are_filled() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("directDefault", "Defaulted<string>"),
            ("namedDefault", "Defaulted"),
            ("starDefault", "Defaulted"),
            ("directPartial", "Pair<boolean, number>"),
            ("namedPartial", "Pair<boolean>"),
        ] {
            for expand in [false, true] {
                healthy(&project, &fixture, marker, expand, 1, summary);
            }
        }
    }
}
#[test]
fn a_selected_json_alias_target_is_a_native_non_type_alias_negative() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            healthy(
                &project,
                &fixture,
                "json",
                expand,
                1,
                "JsonShadow<\"json\">",
            );
        }
    }
}
#[test]
fn an_unsearched_member_resolves_its_actual_type_and_outer_alias_without_changing_navigation() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        let reference = ReferenceQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find("Carrier<").unwrap()),
        };
        let before = project.find_references(reference.clone()).unwrap();
        assert_eq!(before.groups.len(), 1);
        assert!(
            before.groups[0]
                .declarations
                .iter()
                .all(|declaration| declaration.location.file == "node_modules/hidden/index.d.ts")
        );
        assert!(
            before.groups[0]
                .occurrences
                .iter()
                .all(|occurrence| occurrence.location.file == "cases.ts")
        );
        for expand in [false, true] {
            healthy(
                &project,
                &fixture,
                "hidden",
                expand,
                0,
                "Carrier<\"hidden\">",
            );
        }
        let answer = project.type_of(request(&fixture, "hidden", true, 1));
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{config}: {answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found, "{config}: {result:?}");
        assert!(result.issues.is_empty(), "{config}: {:?}", result.issues);
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        let TypeNodeData::Object { members, .. } = &root.data else {
            panic!("{config}: {root:?}");
        };
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(properties.len(), 1);
        assert_eq!(
            properties[0].key,
            TypeKey::String {
                name: "nested".into()
            }
        );
        let summary = match &properties[0].read_type {
            TypeRef::Ref { r#ref } => graph
                .nodes
                .iter()
                .find(|node| node.id == *r#ref)
                .unwrap()
                .summary
                .as_str(),
            TypeRef::Omitted { summary, .. } => summary.as_str(),
        };
        assert_eq!(
            summary,
            "External<\"hidden\"> (node_modules/hidden/target.d.ts:1:13)"
        );
        assert_eq!(project.find_references(reference).unwrap(), before);
    }
}
#[test]
fn unsearched_lookup_errors_remain_unconsumed_until_the_member_type_is_requested() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            healthy(
                &project,
                &fixture,
                "broken",
                expand,
                0,
                "Broken<\"broken\">",
            );
        }
        let answer = project.type_of(request(&fixture, "broken", true, 1));
        let Err(AnalysisError::Semantic(error)) = answer else {
            panic!("{config}: {answer:?}");
        };
        assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
        assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
        assert_eq!(
            error.position,
            Some(position(HIDDEN, HIDDEN.find("MissingHidden").unwrap()))
        );
        healthy(&project, &fixture, "broken", false, 0, "Broken<\"broken\">");
    }
}
