//! Literal token questions retain syntax roles and checking dependencies.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, QueryErrorKind, SemanticErrorKind, SemanticFailureReason,
};
use typescript_parser::types::{
    SelectionFailure, TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/constant-positions/positions.ts");
const CONFIG: &str = include_str!("../fixtures/types/constant-positions/tsconfig.json");
const VALUES: &str = include_str!("../fixtures/types/constants/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("constant-positions/tsconfig.json", CONFIG),
        ("constant-positions/positions.ts", SOURCE),
        (
            "constant-positions/globals.d.ts",
            include_str!("../fixtures/types/constant-positions/globals.d.ts"),
        ),
        (
            "constant-positions/missing-globals.json",
            include_str!("../fixtures/types/constant-positions/missing-globals.json"),
        ),
        ("constants/cases.ts", VALUES),
    ])
}

fn offset(marker: &str) -> usize {
    let marker = format!("/*#{marker}*/");
    SOURCE.find(&marker).unwrap() + marker.len()
}

fn query(fixture: &Fixture, offset: usize) -> TypeRequest {
    TypeRequest::new(TypeQuery::At {
        file: fixture.path("constant-positions/positions.ts"),
        position: position(SOURCE, offset),
    })
}

#[test]
fn nested_literal_tokens_and_template_ends_select_their_own_expression_types() {
    let fixture = fixture();
    let project = fixture.load_config("constant-positions/tsconfig.json");
    for (marker, summary) in [
        ("number", "1"),
        ("twice", "1"),
        ("big", "42n"),
        ("boolean", "false"),
        ("propertyInitializer", "1"),
        ("template", "\"A💡\""),
        ("arrayElement", "\"array\""),
        ("conditional", "\"branch\""),
        ("computed", "\"member\""),
        ("returned", "\"result\""),
        ("condition", "true"),
    ] {
        let answer = project.type_of(query(&fixture, offset(marker)));
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "{marker} must select its own native expression type: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.target.kind, TypeQuestion::Expression);
        assert_eq!(result.target.location.file, "positions.ts");
        assert_eq!(
            result.target.location.start,
            position(SOURCE, offset(marker))
        );
        assert_eq!(result.description.summary, summary, "{marker}");
    }
    let start = offset("template");
    let end = start + SOURCE[start + 1..].find('`').unwrap() + 1;
    for offset in [start, start + 1, start + 2, end] {
        let answer = project.type_of(query(&fixture, offset)).unwrap();
        let TypeAnswer::Selected(result) = answer else {
            panic!("whole template token")
        };
        assert_eq!(result.target.location.start, position(SOURCE, start));
        assert_eq!(result.target.location.end, position(SOURCE, end + 1));
        assert_eq!(result.description.summary, "\"A💡\"");
    }
}

#[test]
fn punctuation_and_trivia_do_not_select_a_nearby_expression() {
    let fixture = fixture();
    let project = fixture.load_config("constant-positions/tsconfig.json");
    for offset in [
        offset("operator"),
        SOURCE.find(" /*💡*/").unwrap(),
        SOURCE.find(" /*💡*/").unwrap() + 1,
    ] {
        let answer = project.type_of(query(&fixture, offset)).unwrap();
        assert!(
            matches!(answer, TypeAnswer::Unselected(selection) if matches!(selection.outcome, SelectionFailure::NotFound { .. })),
            "punctuation/trivia must not promote its enclosing expression"
        );
    }
}

#[test]
fn name_bearing_strings_are_not_approximated_as_cooked_literal_types() {
    let fixture = fixture();
    let project = fixture.load_config("constant-positions/tsconfig.json");
    // The pinned compiler asks for the property's value, rather than the
    // string literal type of its written name.
    let TypeAnswer::Selected(property) = project
        .type_of(query(&fixture, offset("propertyKey")))
        .unwrap()
    else {
        panic!("property name must select its own symbol")
    };
    assert_eq!(property.outcome, TypeOutcome::Found);
    assert_eq!(property.target.kind, TypeQuestion::Value);
    assert_eq!(property.target.location.file, "positions.ts");
    assert_eq!(
        property.target.location.start,
        position(SOURCE, offset("propertyKey"))
    );
    assert_eq!(property.description.summary, "number");
    assert!(property.issues.is_empty());
    let module = project.type_of(query(&fixture, offset("moduleKey")));
    assert!(
        matches!(module, Err(AnalysisError::Query(error)) if error.kind == QueryErrorKind::UnsupportedTarget),
        "module specifier has no supported Q4 type anchor"
    );
}

#[test]
fn selected_literal_arguments_run_the_actual_enclosing_signature_operation() {
    let fixture = fixture();
    let project = fixture.load_config("constant-positions/tsconfig.json");
    // The compiler oracle records the required call/new/tagged-call preludes
    // for these exact inputs. Successful preludes preserve the selected token's
    // own question, including value questions inside callbacks and arguments.
    let cases = [
        ("argument", TypeQuestion::Expression, "\"ready\""),
        ("nestedCall", TypeQuestion::Expression, "\"nested\""),
        ("newArg", TypeQuestion::Expression, "\"new\""),
        ("tagToken", TypeQuestion::Expression, "\"tagged\""),
        ("callbackName", TypeQuestion::Value, "1"),
        ("callProperty", TypeQuestion::Value, "number"),
    ];
    for reverse in [false, true] {
        for index in 0..cases.len() {
            let (marker, kind, summary) = cases[if reverse {
                cases.len() - 1 - index
            } else {
                index
            }];
            for expand in [false, true] {
                let mut request = query(&fixture, offset(marker));
                request.expand = expand;
                let answer = project.type_of(request.clone()).unwrap();
                assert_eq!(
                    answer,
                    project.type_of(request).unwrap(),
                    "{marker}: repeated query"
                );
                let TypeAnswer::Selected(result) = answer else {
                    panic!("the native call prelude must retain the selected question: {marker}")
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{marker}");
                assert_eq!(result.target.kind, kind, "{marker}");
                assert_eq!(result.target.location.file, "positions.ts");
                assert_eq!(
                    result.target.location.start,
                    position(SOURCE, offset(marker)),
                    "{marker}"
                );
                assert_eq!(result.description.summary, summary, "{marker}");
                assert!(result.issues.is_empty(), "{marker}");
            }
        }
    }
}

#[test]
fn a_selected_tagged_literal_preserves_global_then_annotation_recovery() {
    let fixture = fixture();
    let project = fixture.load_config("constant-positions/missing-globals.json");
    let annotation = SOURCE.find("TemplateStringsArray").unwrap();
    let start = position(SOURCE, annotation);
    let end = position(SOURCE, annotation + "TemplateStringsArray".len());
    let file = fixture.path("constant-positions/positions.ts");
    for expand in [false, true, false] {
        let mut request = query(&fixture, offset("tagToken"));
        request.expand = expand;
        let answer = project.type_of(request.clone());
        assert_eq!(answer, project.type_of(request), "repeated tagged prelude");
        let Err(AnalysisError::Semantic(error)) = answer else {
            panic!("the native tagged prelude must retain both consumed recoveries: {answer:?}")
        };
        assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
        assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
        assert_eq!(error.path.as_ref(), Some(&file));
        assert_eq!(error.position, Some(position(SOURCE, offset("tagToken"))));
        let global = error
            .causes
            .iter()
            .position(|cause| {
                cause.reason == SemanticFailureReason::UnresolvedSymbol.into()
                    && cause.location.is_none()
                    && cause.message == "Cannot find global type 'TemplateStringsArray'."
            })
            .expect("source-free global fallback must retain its actual cause");
        assert!(
            error.causes[global + 1..].iter().any(|cause| cause.reason
                == SemanticFailureReason::UnresolvedSymbol.into()
                && cause
                    .location
                    .as_ref()
                    .is_some_and(|location| location.file == "positions.ts"
                        && location.start == start
                        && location.end == end)),
            "the recovered synthetic argument must permit later parameter annotation acquisition: {error:?}"
        );
        let TypeAnswer::Selected(healthy) =
            project.type_of(query(&fixture, offset("number"))).unwrap()
        else {
            panic!("unrelated literal must remain selectable")
        };
        assert_eq!(healthy.outcome, TypeOutcome::Found);
        assert_eq!(healthy.description.summary, "1");
        assert!(healthy.issues.is_empty());
    }
}
