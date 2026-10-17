//! Compiler-measured conditional evaluation through complete public queries.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-conditional-types/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-conditional-types/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-conditional-types/tsconfig.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-conditional-types/exact.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-conditional-types/loose.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    })
}
fn check(cases: &[(&str, &str)]) {
    let fixture = fixture();
    for config in ["tsconfig.json", "exact.json", "loose.json"] {
        let project = fixture.load_config(config);
        for &(marker, expected) in cases {
            for expand in [false, true] {
                let mut query = request(&fixture, marker);
                query.expand = expand;
                let answer = project.type_of(query);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}: {:?}",
                    result.issues
                );
                assert_eq!(
                    result.description.summary, expected,
                    "{config}/{marker}/{expand}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{marker}: {:?}",
                    result.issues
                );
            }
        }
    }
}
#[test]
fn definite_branches_distribute_with_any_never_and_unknown_distinctions() {
    check(&[
        ("true", "\"yes\""),
        ("false", "\"no\""),
        ("union", "\"no\" | \"yes\""),
        ("any", "\"no\" | \"yes\""),
        ("never", "never"),
        ("unknown", "\"no\""),
    ]);
}
#[test]
fn tuple_wrapping_controls_distribution_and_generic_element_deferral() {
    check(&[("wrapped", "\"no\""), ("wrappedNever", "\"yes\"")]);
}
#[test]
fn inferred_slots_combine_candidates_and_apply_declared_constraints() {
    check(&[
        ("arrayInfer", "\"a\" | \"b\""),
        ("repeatInfer", "\"a\" | 1"),
        ("returnInfer", "\"return\""),
        ("constrainedInfer", "\"a\""),
    ]);
}
#[test]
fn true_branch_substitution_and_tail_conditional_instantiation_share_native_owners() {
    check(&[
        ("trueSubstitution", "number"),
        ("nested", "\"number\" | \"other\""),
        ("tail", "\"done\""),
        ("cross", "\"a\""),
    ]);
}
#[test]
fn deferred_type_is_lazy_and_nullable_distribution_follows_the_profile() {
    check(&[("deferred", "<T extends { a: unknown; }>() => Deferred<T>")]);
    let fixture = fixture();
    for (config, expected) in [
        ("tsconfig.json", "\"nullish\" | \"value\""),
        ("exact.json", "\"nullish\" | \"value\""),
        ("loose.json", "\"value\""),
    ] {
        let project = fixture.load_config(config);
        let answer = project.type_of(request(&fixture, "nullable")).unwrap();
        let TypeAnswer::Selected(result) = answer else {
            panic!("{answer:?}");
        };
        assert_eq!(
            result.outcome,
            TypeOutcome::Found,
            "{config}: {:?}",
            result.issues
        );
        assert_eq!(result.description.summary, expected, "{config}");
    }
}
#[test]
fn unselected_missing_branch_stays_unconsumed_but_selected_failure_keeps_its_location() {
    check(&[("skippedMissing", "\"safe\"")]);
    let fixture = fixture();
    for config in ["tsconfig.json", "exact.json", "loose.json"] {
        let project = fixture.load_config(config);
        let answer = project.type_of(request(&fixture, "consumedMissing"));
        let Err(AnalysisError::Semantic(error)) = answer else {
            panic!("{config}: {answer:?}");
        };
        assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
        assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
        assert_eq!(
            error.position,
            Some(position(SOURCE, SOURCE.find("MissingConditional").unwrap()))
        );
    }
}

#[test]
fn inline_inference_summaries_preserve_extends_context_and_native_precedence() {
    check(&[
        ("inlineArray", "<T>() => T extends (infer U)[] ? U : never"),
        (
            "inlineConstrained",
            "<T>() => T extends infer U extends string ? U : never",
        ),
        (
            "inlineReturn",
            "<T>() => T extends () => (infer U extends string) ? U : never",
        ),
        (
            "inlineRest",
            "<T>() => T extends [unknown, ...infer R] ? R : never",
        ),
        (
            "inlineTemplate",
            "<T>() => T extends `${infer U}` ? U : never",
        ),
        (
            "inlineNested",
            "<T, V>() => T extends (V extends infer W ? W : never) ? T : never",
        ),
        ("inlineFunctionCheck", "<T>() => T"),
        (
            "inlineTupleConstrained",
            "<T>() => T extends [infer U extends string] ? U : never",
        ),
    ]);
}
