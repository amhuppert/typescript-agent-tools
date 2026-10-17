//! Final calls expose inherited native property-symbol identity and demand order.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

const CASES: &str = include_str!("../fixtures/types/native-heritage/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-heritage/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-heritage/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-heritage/exact.json"),
        ),
        ("cases.ts", CASES),
    ])
}

fn answer(project: &Project, fixture: &Fixture, name: &str) -> Result<TypeAnswer, AnalysisError> {
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(CASES, CASES.find(&format!("{name}Result =")).unwrap()),
    });
    request.expand = true;
    request.depth = 2;
    project.type_of(request)
}

fn found(project: &Project, fixture: &Fixture, name: &str) {
    let answer = answer(project, fixture, name);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
        "{name} must preserve the native unvisited property dependency: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.description.summary, "\"chosen\"", "{name}");
    assert!(result.issues.is_empty(), "{name}");
    let graph = result.description.expansion.unwrap();
    assert!(!graph.truncated, "{name}");
    let root = graph
        .nodes
        .into_iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    assert_eq!(
        root.data,
        TypeNodeData::Literal {
            base: PrimitiveName::String,
            value: "chosen".into(),
        },
        "{name}"
    );
}

#[test]
fn inherited_thisless_symbols_skip_latent_reads_in_structural_calls() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(profile);
        // Each A/B has an own marker, so equivalent-base normalization cannot
        // replace this actual inherited-property comparison with Shared.
        for name in ["shared", "methods", "parameter", "getter", "methodParen"] {
            found(&project, &fixture, name);
        }
    }
}

#[test]
fn distinct_and_non_thisless_symbols_consume_the_target_property_annotation() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(profile);
        for (name, annotation) in [
            ("parenthesized", "MissingParenthesized"),
            ("nested", "MissingNested"),
            ("union", "MissingUnion"),
            ("independent", "MissingIndependentTarget"),
        ] {
            let answer = answer(&project, &fixture, name);
            let path = fixture.path("cases.ts").canonicalize().unwrap();
            let expected = position(CASES, CASES.find(annotation).unwrap());
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                    && error.path.as_deref() == Some(path.as_path())
                    && error.position == Some(expected)),
                "{profile}/{name} must preserve the visited native property dependency: {answer:?}"
            );
        }
    }
}

#[test]
fn parenthesized_types_preserve_selected_roots_and_enclosing_aliases() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(profile);
        for (name, summary) in [
            ("wrappedRoot", "number"),
            ("wrappedAliasRoot", "WrappedAlias"),
        ] {
            let offset = CASES.find(&format!("{name}:")).unwrap();
            let answer = project
                .type_of(TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(CASES, offset),
                }))
                .unwrap();
            let TypeAnswer::Selected(result) = answer else {
                panic!("{profile}/{name}")
            };
            assert_eq!(result.outcome, TypeOutcome::Found, "{profile}/{name}");
            assert_eq!(result.description.summary, summary, "{profile}/{name}");
            assert_eq!(result.target.location.file, "cases.ts");
            assert_eq!(result.target.location.start, position(CASES, offset));
            assert_eq!(
                result.target.location.end,
                position(CASES, offset + name.len())
            );
            assert!(result.issues.is_empty());
        }
    }
}

#[test]
fn empty_extending_interfaces_use_the_native_nonaugmenting_base_shortcut() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(profile);
        found(&project, &fixture, "emptyParents");
    }
}
