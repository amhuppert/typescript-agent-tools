//! Source checking has a different schedule from type acquisition. These
//! queries check the final primary and ordered distinct dependency sites;
//! native diagnostic arrays are sorted and cannot establish that order.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-source-check-recovery/cases.ts");

#[test]
fn written_children_precede_reference_acquisition_and_arity_recovery() {
    check(&[
        "untypedList",
        "nestedReference",
        "discardedAliasArguments",
        "arityStillChecksChildren",
        "arityAcquisitionSkipsChildren",
    ]);
}

#[test]
fn aggregate_checks_preserve_tuple_prepass_and_grammar_stop_continuations() {
    check(&[
        "unionChildren",
        "intersectionChildren",
        "wrappedArray",
        "tupleChildren",
        "tuplePrepass",
        "tupleGrammarStop",
    ]);
}

#[test]
fn special_type_workers_keep_every_checked_child_and_runtime_argument() {
    check(&[
        "conditionalAllChildren",
        "inferConstraint",
        "signatureChildren",
        "constructorChildren",
        "literalMembers",
        "indexedChildren",
        "templateChildren",
        "mappedChildren",
        "typeQuery",
    ]);
}

#[test]
fn false_constraint_comparison_and_untyped_invocations_keep_later_checks() {
    check(&[
        "rejectedThenConstraint",
        "newTypeArguments",
        "tagTypeArguments",
    ]);
}

#[test]
fn diagnostic_only_source_checks_and_following_requests_remain_healthy() {
    check(&["healthySource", "healthyAfter"]);
}

fn check(names: &[&str]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-source-check-recovery/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-source-check-recovery/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-source-check-recovery/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-source-check-recovery/exact.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, captured) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-source-check-recovery/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-source-check-recovery/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-source-check-recovery/expected-exact.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(captured).unwrap();
        assert_eq!(rows.len(), 25);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut rows = rows.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                if !names.contains(&name) {
                    continue;
                }
                let expected = dependency_sites(name);
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, marked('#', name)),
                    });
                    request.expand = expand;
                    request.depth = 2;
                    let answer = project.type_of(request);
                    if expected.is_empty() {
                        assert_eq!(row["errorType"], false);
                        // 2347 is the diagnostic for written type arguments
                        // on an untyped call. It supplies no error type.
                        assert_eq!(
                            row["codes"],
                            if name == "healthySource" {
                                serde_json::json!([2347])
                            } else {
                                serde_json::json!([])
                            }
                        );
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found);
                        assert_eq!(answer.description.summary, row["summary"].as_str().unwrap());
                        assert!(answer.issues.is_empty());
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
                    assert_eq!(error.path.as_deref(), Some(path.as_path()));
                    assert_eq!(
                        error.reason,
                        expected[0].0.into(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, dependency_offset(expected[0].1))),
                        "{profile}/{name}/{expand}: {error:?}"
                    );

                    // Native postorder getType reads can revisit a child's
                    // established recovery. This matrix proves distinct-site
                    // order and complete later acquisition, not cache-read
                    // multiplicity. Every retained cause must still have an
                    // expected reason and actual declaration location.
                    let mut visited = vec![0];
                    for cause in &error.causes {
                        let location = cause
                            .location
                            .as_ref()
                            .expect("source dependency has its actual location");
                        assert_eq!(location.file, "cases.ts");
                        let slot = expected
                            .iter()
                            .position(|(reason, marker)| {
                                cause.reason == (*reason).into()
                                    && location.start == position(SOURCE, dependency_offset(marker))
                            })
                            .unwrap_or_else(|| {
                                panic!("unexpected dependency {profile}/{name}/{expand}: {error:?}")
                            });
                        if !visited.contains(&slot) {
                            visited.push(slot);
                        }
                    }
                    assert_eq!(
                        visited,
                        (0..expected.len()).collect::<Vec<_>>(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert!(
                        error.omitted_causes.is_none(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    if matches!(
                        name,
                        "untypedList"
                            | "arityAcquisitionSkipsChildren"
                            | "newTypeArguments"
                            | "tagTypeArguments"
                    ) {
                        // These source arguments have no postorder child
                        // acquisition. Each failure is consumed exactly once.
                        assert_eq!(
                            error.causes.len(),
                            expected.len() - 1,
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                    }
                }
            }
        }
    }
}

/// Pinned checker 2970–3023/3219–3395/9878 and signature checks 2591/2711.
/// Entries are first acquired failure sites, not sorted diagnostic positions.
fn dependency_sites(name: &str) -> Vec<(SemanticFailureReason, &'static str)> {
    let missing: &[&str] = match name {
        "untypedList" => &["first", "last", "runtimeTail"],
        "nestedReference" => &["inner", "outer", "nestedLast", "runtimeTail"],
        "discardedAliasArguments" => &["ignoredFirst", "ignoredLast", "runtimeTail"],
        "arityStillChecksChildren" => {
            return vec![
                (SemanticFailureReason::UnresolvedSymbol, "checkedArityChild"),
                (SemanticFailureReason::ErrorType, "checkedArity"),
                (SemanticFailureReason::UnresolvedSymbol, "checkedArityLast"),
                (SemanticFailureReason::UnresolvedSymbol, "runtimeTail"),
            ];
        }
        "arityAcquisitionSkipsChildren" => {
            return vec![(SemanticFailureReason::ErrorType, "uncheckedArity")];
        }
        "unionChildren" => &["unionFirst", "unionLast", "runtimeTail"],
        "intersectionChildren" => &["intersectionFirst", "intersectionLast", "runtimeTail"],
        "wrappedArray" => &["arrayElement", "arrayLast", "runtimeTail"],
        "tupleChildren" => &["tupleFirst", "tupleSecond", "tupleLast", "runtimeTail"],
        // checkTupleType acquires a variadic element during validation before
        // its checkSourceElements pass visits the preceding required element.
        "tuplePrepass" => &["tupleRest", "tupleHead", "tupleTail", "runtimeTail"],
        "tupleGrammarStop" => {
            // Grammar's break still reaches getTypeFromTypeNode(tuple).
            // Native TupleNormalizer selects errorType for ...number after
            // acquiring both children, before the later runtime argument.
            return vec![
                (SemanticFailureReason::UnresolvedSymbol, "invalidTupleHead"),
                (SemanticFailureReason::UnresolvedSymbol, "invalidTupleTail"),
                (SemanticFailureReason::ErrorType, "invalidTupleRoot"),
                (SemanticFailureReason::UnresolvedSymbol, "runtimeTail"),
            ];
        }
        "conditionalAllChildren" => &["conditionalTrue", "conditionalFalse", "runtimeTail"],
        "inferConstraint" => &["inferConstraint", "inferLast", "runtimeTail"],
        "signatureChildren" => &[
            "signatureConstraint",
            "signatureDefault",
            "signatureParameter",
            "signatureReturn",
            "signatureLast",
            "runtimeTail",
        ],
        "constructorChildren" => &["constructorParameter", "constructorReturn", "runtimeTail"],
        "literalMembers" => &[
            "literalFirst",
            "literalSecond",
            "literalLast",
            "runtimeTail",
        ],
        "indexedChildren" => &["indexedValue", "indexedLast", "runtimeTail"],
        "templateChildren" => &["templateFirst", "templateLast", "runtimeTail"],
        "mappedChildren" => &["mappedName", "mappedTemplate", "mappedLast", "runtimeTail"],
        "typeQuery" => &["queryValue", "queryLast", "runtimeTail"],
        // The first diagnostic-only False skips later comparisons, but not
        // acquisition of the next type parameter's constraint.
        "rejectedThenConstraint" => &["lateConstraint", "afterConstraint", "runtimeTail"],
        "newTypeArguments" => &["newFirst", "newLast", "runtimeTail"],
        "tagTypeArguments" => &["tagFirst", "tagLast", "runtimeTail"],
        "healthySource" | "healthyAfter" => &[],
        _ => panic!("unclassified native case {name}"),
    };
    missing
        .iter()
        .map(|&marker| (SemanticFailureReason::UnresolvedSymbol, marker))
        .collect()
}

fn dependency_offset(name: &str) -> usize {
    if name != "invalidTupleRoot" {
        return marked('@', name);
    }
    let declaration = marked('#', "tupleGrammarStop");
    let tail = &SOURCE[declaration..];
    let line = tail.split_once('\n').unwrap().0;
    let mut starts = line.match_indices("anyCall<[");
    let (start, _) = starts
        .next()
        .expect("actual tuple argument in selected declaration");
    assert!(
        starts.next().is_none(),
        "ambiguous tuple argument in selected declaration"
    );
    declaration + start + "anyCall<".len()
}

fn marked(kind: char, name: &str) -> usize {
    let marker = format!("/*{kind}{name}*/");
    let mut matches = SOURCE.match_indices(&marker);
    let (offset, _) = matches
        .next()
        .unwrap_or_else(|| panic!("missing marker {marker}"));
    assert!(matches.next().is_none(), "ambiguous marker {marker}");
    offset + marker.len()
}
