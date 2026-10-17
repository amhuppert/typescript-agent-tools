//! Final parameter-pattern answers measured with TypeScript 7.0.2.
//! Native assignment traversal supplies dependency order, not sorted diagnostics.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, SemanticErrorKind, SemanticFailureReason, SemanticReason,
};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-contextual-patterns/cases.ts");
const TARGET: &str = include_str!("../fixtures/types/native-contextual-patterns/target.ts");

#[test]
fn contextual_properties_nested_patterns_and_tuple_positions_keep_native_values() {
    check(&[
        "object",
        "renamed",
        "nested",
        "objectRest",
        "tuple",
        "omitted",
        "tupleDefault",
        "tupleRest",
        "annotated",
        "initializedContext",
        "thisPattern",
        "healthyAfter",
    ]);
}

#[test]
fn unknown_context_builds_actual_implied_patterns_and_exact_keys() {
    check(&[
        "soleRest",
        "unknownObject",
        "unknownArray",
        "unknownNested",
        "duplicateKey",
        "computedLiteralKey",
        "numericKey",
        "targetSoleRest",
        "targetFixedAndRest",
    ]);
}

#[test]
fn quick_call_results_do_not_acquire_unused_callback_dependencies() {
    check(&[
        "emptyObject",
        "emptyArray",
        "quickUnusedFields",
        "targetEmptyObject",
        "targetEmptyArray",
    ]);
}

#[test]
fn assigning_patterns_keeps_later_fields_defaults_and_independent_parameters() {
    check(&[
        "unusedFields",
        "nestedDefaults",
        "unknownDefaults",
        "independentParameters",
        "dynamicPattern",
    ]);
}

fn check(names: &[&str]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        ("target.ts", TARGET),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-contextual-patterns/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-contextual-patterns/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-contextual-patterns/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-contextual-patterns/exact.json"),
        ),
        (
            "protocol.d.ts",
            include_str!("../fixtures/types/native-contextual-patterns/protocol.d.ts"),
        ),
        (
            "protocol-tsconfig.json",
            include_str!("../fixtures/types/native-contextual-patterns/protocol-tsconfig.json"),
        ),
        (
            "protocol-loose.json",
            include_str!("../fixtures/types/native-contextual-patterns/protocol-loose.json"),
        ),
        (
            "protocol-exact.json",
            include_str!("../fixtures/types/native-contextual-patterns/protocol-exact.json"),
        ),
    ]);
    let mut covered = Vec::new();
    for (profile, file, source, baseline) in [
        (
            "tsconfig",
            "cases.ts",
            SOURCE,
            include_str!(
                "../fixtures/types/native-contextual-patterns/expected-iteration-tsconfig-cases.json"
            ),
        ),
        (
            "loose",
            "cases.ts",
            SOURCE,
            include_str!(
                "../fixtures/types/native-contextual-patterns/expected-iteration-loose-cases.json"
            ),
        ),
        (
            "exact",
            "cases.ts",
            SOURCE,
            include_str!(
                "../fixtures/types/native-contextual-patterns/expected-iteration-exact-cases.json"
            ),
        ),
        (
            "tsconfig",
            "target.ts",
            TARGET,
            include_str!(
                "../fixtures/types/native-contextual-patterns/expected-iteration-tsconfig-target.json"
            ),
        ),
        (
            "protocol-tsconfig",
            "cases.ts",
            SOURCE,
            include_str!(
                "../fixtures/types/native-contextual-patterns/expected-iteration-protocol-tsconfig-cases.json"
            ),
        ),
        (
            "protocol-loose",
            "cases.ts",
            SOURCE,
            include_str!(
                "../fixtures/types/native-contextual-patterns/expected-iteration-protocol-loose-cases.json"
            ),
        ),
        (
            "protocol-exact",
            "cases.ts",
            SOURCE,
            include_str!(
                "../fixtures/types/native-contextual-patterns/expected-iteration-protocol-exact-cases.json"
            ),
        ),
        (
            "protocol-tsconfig",
            "target.ts",
            TARGET,
            include_str!(
                "../fixtures/types/native-contextual-patterns/expected-iteration-protocol-tsconfig-target.json"
            ),
        ),
    ] {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), if file == "cases.ts" { 27 } else { 4 });
        let project = fixture.load_config(&format!("{profile}.json"));
        let path = fixture.path(file).canonicalize().unwrap();
        // Each request creates its own evaluator; this checks order isolation,
        // not cache warmth within a single compiler query.
        for reverse in [false, true] {
            let mut rows = native.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                if !names.contains(&name) {
                    continue;
                }
                if !covered.contains(&name.to_owned()) {
                    covered.push(name.to_owned());
                }
                let marker = format!("/*#{name}*/");
                let selected = unique(source, &marker) + marker.len();
                let mut expected = dependencies(name)
                    .iter()
                    .map(|text| {
                        (
                            SemanticFailureReason::UnresolvedSymbol,
                            position(SOURCE, unique(SOURCE, text)),
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(row["errorType"], !expected.is_empty(), "{profile}/{name}");
                // checkIteratedTypeOrElementType may return Any after native
                // nil even when a later tuple index produces a healthy summary.
                // Its actual error node is the full ArrayBindingPattern.
                let fallbacks = row["iteration"]
                    .as_object()
                    .unwrap()
                    .values()
                    .filter(|event| event["fallbackAny"] == true)
                    .map(|event| event["pos"].as_u64().unwrap() as usize)
                    .collect::<Vec<_>>();
                if let Some(&at) = fallbacks.first() {
                    assert!(
                        expected.is_empty(),
                        "unclassified mixed factory evidence {profile}/{name}"
                    );
                    assert!(fallbacks.iter().all(|offset| *offset == at));
                    assert_eq!(
                        source.as_bytes()[at],
                        b'[',
                        "native error node is the pattern"
                    );
                    expected.push((SemanticFailureReason::ErrorType, position(source, at)));
                }
                if profile.starts_with("protocol-") {
                    assert!(
                        fallbacks.is_empty(),
                        "the valid iterable twin must not consume fallback"
                    );
                }
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path(file),
                            position: position(source, selected),
                        },
                        expand,
                        depth: 2,
                    });
                    if expected.is_empty() {
                        let Ok(TypeAnswer::Selected(result)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(result.outcome, TypeOutcome::Found);
                        assert_eq!(
                            result.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{profile}/{name}/{expand}"
                        );
                        assert!(
                            result.issues.is_empty(),
                            "{profile}/{name}/{expand}: {result:?}"
                        );
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(
                        error.reason,
                        expected[0].0.into(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(error.path.as_deref(), Some(path.as_path()));
                    assert_eq!(
                        error.position,
                        Some(expected[0].1),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    let mut acquired = vec![0];
                    for cause in &error.causes {
                        let location = cause.location.as_ref().expect("actual consumed dependency");
                        assert_eq!(location.file, file);
                        let index = expected
                            .iter()
                            .position(|(reason, at)| {
                                SemanticReason::from(*reason) == cause.reason
                                    && *at == location.start
                            })
                            .unwrap_or_else(|| {
                                panic!("unexpected {profile}/{name}/{expand}: {error:?}")
                            });
                        if !acquired.contains(&index) {
                            acquired.push(index);
                        }
                    }
                    assert_eq!(
                        acquired,
                        (0..expected.len()).collect::<Vec<_>>(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert!(
                        error.omitted_causes.is_none(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                }
            }
        }
    }
    assert_eq!(
        covered.len(),
        names.len(),
        "all requested native rows must be exercised"
    );
}

fn dependencies(name: &str) -> &'static [&'static str] {
    match name {
        // assignContextualParameterTypes visits parameters in order; each
        // assignBindingElementTypes visit descends before the next sibling.
        "unusedFields" => &["MissingPatternFirst", "MissingPatternSecond"],
        "nestedDefaults" => &["MissingNestedFirst", "MissingNestedSecond"],
        "unknownDefaults" => &["MissingUnknownFirst", "MissingUnknownSecond"],
        "independentParameters" => &["MissingFirstParameter", "MissingSecondParameter"],
        // Unknown first builds an implied object. Its dynamic computed key is
        // skipped before its default, so the ordinary default is acquired
        // first. The subsequent assignment pass acquires the dynamic default.
        "dynamicPattern" => &["MissingOrdinaryDefault", "MissingDynamicDefault"],
        "object" | "renamed" | "nested" | "objectRest" | "tuple" | "omitted" | "tupleDefault"
        | "tupleRest" | "emptyObject" | "emptyArray" | "soleRest" | "unknownObject"
        | "unknownArray" | "unknownNested" | "annotated" | "initializedContext" | "thisPattern"
        | "quickUnusedFields" | "duplicateKey" | "computedLiteralKey" | "numericKey"
        | "healthyAfter" | "targetEmptyObject" | "targetEmptyArray" | "targetSoleRest"
        | "targetFixedAndRest" => &[],
        _ => panic!("unclassified contextual pattern row {name}"),
    }
}

fn unique(source: &str, text: &str) -> usize {
    let mut matches = source.match_indices(text);
    let (offset, _) = matches.next().unwrap_or_else(|| panic!("missing {text}"));
    assert!(matches.next().is_none(), "ambiguous {text}");
    offset
}
