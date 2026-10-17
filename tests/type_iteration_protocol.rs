//! Native iterator acquisitions distinguish real Any from consumed recovery.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, SemanticErrorKind, SemanticFailureReason, SemanticReason,
};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-iteration-protocol/cases.ts");
const LEGACY: &str = include_str!("../fixtures/types/native-iteration-protocol/legacy.ts");

#[test]
fn actual_iterator_protocol_values_and_ordered_recoveries() {
    check(false);
}

#[test]
fn quiet_missing_or_invalid_global_identity_uses_native_legacy_iteration() {
    check(true);
}

fn check(legacy: bool) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        ("legacy.ts", LEGACY),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-iteration-protocol/globals.d.ts"),
        ),
        (
            "protocol.d.ts",
            include_str!("../fixtures/types/native-iteration-protocol/protocol.d.ts"),
        ),
        (
            "wrong-kind.d.ts",
            include_str!("../fixtures/types/native-iteration-protocol/wrong-kind.d.ts"),
        ),
        (
            "wrong-arity.d.ts",
            include_str!("../fixtures/types/native-iteration-protocol/wrong-arity.d.ts"),
        ),
        (
            "strict.json",
            include_str!("../fixtures/types/native-iteration-protocol/strict.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-iteration-protocol/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-iteration-protocol/exact.json"),
        ),
        (
            "absent.json",
            include_str!("../fixtures/types/native-iteration-protocol/absent.json"),
        ),
        (
            "kind.json",
            include_str!("../fixtures/types/native-iteration-protocol/kind.json"),
        ),
        (
            "arity.json",
            include_str!("../fixtures/types/native-iteration-protocol/arity.json"),
        ),
    ]);
    for (profile, baseline, is_legacy) in [
        (
            "strict",
            include_str!("../fixtures/types/native-iteration-protocol/expected-strict.json"),
            false,
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-iteration-protocol/expected-loose.json"),
            false,
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-iteration-protocol/expected-exact.json"),
            false,
        ),
        (
            "absent",
            include_str!("../fixtures/types/native-iteration-protocol/expected-absent.json"),
            true,
        ),
        (
            "kind",
            include_str!("../fixtures/types/native-iteration-protocol/expected-kind.json"),
            true,
        ),
        (
            "arity",
            include_str!("../fixtures/types/native-iteration-protocol/expected-arity.json"),
            true,
        ),
    ] {
        if legacy != is_legacy {
            continue;
        }
        let (file, source) = if legacy {
            ("legacy.ts", LEGACY)
        } else {
            ("cases.ts", SOURCE)
        };
        let project = fixture.load_config(&format!("{profile}.json"));
        let path = fixture.path(file).canonicalize().unwrap();
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), if legacy { 6 } else { 29 });
        // Each request owns an evaluator. Reversal checks isolation, not a
        // warm native cache across independent requests.
        for reverse in [false, true] {
            let mut rows = native.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let selected = unique(source, &marker) + marker.len();
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path(file),
                            position: position(source, selected),
                        },
                        expand,
                        depth: 2,
                    });
                    if matches!(name, "asyncOnlyReturn" | "asyncBadThen") {
                        let (panic, rhs, message) = if name == "asyncOnlyReturn" {
                            (
                                "getAsyncFromSyncIterationTypes awaited nil yield",
                                "of returnsOnly",
                                "pinned native async iteration cannot await an absent yield or return type",
                            )
                        } else {
                            (
                                "getAsyncFromSyncIterationTypes invalid thenable nil diagnostic",
                                "of badAsyncThen",
                                "pinned native async-from-sync iteration cannot report an invalid thenable without a diagnostic message",
                            )
                        };
                        assert_eq!(row["nativePanic"], panic);
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(error.kind, SemanticErrorKind::UnsupportedSemantic);
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnsupportedSemanticForm.into()
                        );
                        assert_eq!(error.path.as_deref(), Some(path.as_path()));
                        assert_eq!(
                            error.position,
                            Some(position(source, unique(source, rhs) + 3))
                        );
                        assert_eq!(error.message, message);
                        assert!(error.causes.is_empty(), "{error:?}");
                        assert!(error.omitted_causes.is_none());
                        continue;
                    }
                    let observations = row["iteration"].as_object().unwrap();
                    let fallback = observations
                        .values()
                        .any(|event| event["fallbackAny"] == true);
                    let mut expected = dependencies(name)
                        .iter()
                        .map(|text| {
                            (
                                SemanticFailureReason::UnresolvedSymbol,
                                position(source, unique(source, text)),
                            )
                        })
                        .collect::<Vec<_>>();
                    if fallback {
                        // Every fixture has one actual error node. Native pos
                        // includes leading whitespace on a for-of RHS; Project
                        // locations use that node's token start.
                        let mut nodes = observations
                            .iter()
                            .filter(|(key, _)| key.starts_with("checkedIteration:"))
                            .map(|(_, event)| event["pos"].as_u64().unwrap() as usize);
                        let mut at = nodes.next().expect("checked protocol consumer");
                        assert!(nodes.all(|other| other == at));
                        while source.as_bytes()[at].is_ascii_whitespace() {
                            at += 1;
                        }
                        expected.push((SemanticFailureReason::ErrorType, position(source, at)));
                    }
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
                    assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic, "{error:?}");
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
                        let location = cause
                            .location
                            .as_ref()
                            .expect("actual iteration dependency");
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
                    assert!(error.omitted_causes.is_none(), "{error:?}");
                }
            }
        }
    }
}

fn dependencies(name: &str) -> &'static [&'static str] {
    match name {
        // getIterationTypesOfIterator visits next, return, then throw; every
        // method acquires parameter types before its return. Iterator-result
        // yield filtering/value precedes its separate return filtering/value.
        "orderedMethods" => &[
            "MissingNextResult",
            "MissingReturnResult",
            "MissingThrowResult",
        ],
        "doneRecovery" => &["MissingDone"],
        "valueRecovery" => &["MissingYieldValue", "MissingReturnValue"],
        "actualAny" | "fastIterable" | "iteratorObject" | "builtin" | "array" | "tuple"
        | "structural" | "doneUnion" | "nextInput" | "overloads" | "requiredArgument"
        | "optionalIterator" | "missingMethod" | "invalidResult" | "optionalNext" | "union"
        | "unionMissing" | "never" | "asyncFast" | "asyncFromSync" | "asyncSlow" | "after"
        | "onlyYield" | "asyncBadThen" | "asyncBadReturn" | "legacyArray" | "legacyTuple"
        | "legacyString" | "legacyUnion" | "legacyMissing" | "legacyAny" => &[],
        _ => panic!("unclassified iteration row {name}"),
    }
}

fn unique(source: &str, text: &str) -> usize {
    let mut matches = source.match_indices(text);
    let (at, _) = matches.next().unwrap_or_else(|| panic!("missing {text}"));
    assert!(matches.next().is_none(), "ambiguous {text}");
    at
}
