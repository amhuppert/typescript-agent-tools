//! Private access is a lexical-symbol operation, with native helper and
//! diagnostic reads retained before its final type or recovery is consumed.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const CASES: &str = include_str!("../fixtures/types/native-private-access/cases.ts");
const DIAGNOSTICS: &str = include_str!("../fixtures/types/native-private-access/diagnostics.ts");
const HELPERS: &str = include_str!("../fixtures/types/native-private-access/helpers.ts");
const RECOVER: &str = include_str!("../fixtures/types/native-private-access/helpers-recover.d.ts");

type Event<'a> = (&'a str, &'a str, usize, SemanticFailureReason);

#[test]
fn private_access_preserves_lexical_class_identity_alias_policy_and_native_values() {
    for (profile, baseline) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-private-access/expected-tsconfig.json"),
        ),
        (
            "verbatim",
            include_str!("../fixtures/types/native-private-access/expected-verbatim.json"),
        ),
    ] {
        check(profile, "cases.ts", CASES, baseline, 26);
    }
}

#[test]
fn private_access_retains_error_receiver_and_ordered_diagnostic_dependencies() {
    check(
        "diagnostics",
        "diagnostics.ts",
        DIAGNOSTICS,
        include_str!("../fixtures/types/native-private-access/expected-diagnostics.json"),
        14,
    );
}

#[test]
fn private_helpers_observe_get_set_order_and_actual_declaration_signatures() {
    for (profile, baseline) in [
        (
            "helpers-valid",
            include_str!("../fixtures/types/native-private-access/expected-helpers-valid.json"),
        ),
        (
            "helpers-short",
            include_str!("../fixtures/types/native-private-access/expected-helpers-short.json"),
        ),
        (
            "helpers-recover",
            include_str!("../fixtures/types/native-private-access/expected-helpers-recover.json"),
        ),
        (
            "helpers-alias",
            include_str!("../fixtures/types/native-private-access/expected-helpers-alias.json"),
        ),
        (
            "helpers-variables",
            include_str!("../fixtures/types/native-private-access/expected-helpers-variables.json"),
        ),
        (
            "helpers-empty",
            include_str!("../fixtures/types/native-private-access/expected-helpers-empty.json"),
        ),
        (
            "helpers-missing",
            include_str!("../fixtures/types/native-private-access/expected-helpers-missing.json"),
        ),
        (
            "helpers-disabled",
            include_str!("../fixtures/types/native-private-access/expected-helpers-disabled.json"),
        ),
        (
            "helpers-native",
            include_str!("../fixtures/types/native-private-access/expected-helpers-native.json"),
        ),
        (
            "helpers-assign",
            include_str!("../fixtures/types/native-private-access/expected-helpers-assign.json"),
        ),
    ] {
        check(profile, "helpers.ts", HELPERS, baseline, 5);
    }
}

fn check(profile: &str, file: &str, source: &str, baseline: &str, count: usize) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-private-access"),
    );
    let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
    assert_eq!(rows.len(), count);
    let project = fixture.load_config(&format!("{profile}.json"));
    // A query creates its own evaluator. Reverse order proves isolation;
    // repeatedGet/writeThenRead exercise actual reuse within one query.
    for reverse in [false, true] {
        let mut rows = rows.clone();
        if reverse {
            rows.reverse();
        }
        for row in rows {
            let name = row["name"].as_str().unwrap();
            let marker = format!("/*#{name}*/");
            let selected = source.find(&marker).unwrap() + marker.len();
            let expected = expected_events(profile, file, source, name, selected);
            // The compiler returns ordinary number after helper diagnostics;
            // the product must still reject its consumed signature recovery.
            if profile.starts_with("helpers-") {
                assert_eq!(row["errorType"], false);
            }
            if profile == "diagnostics" {
                assert_eq!(row["errorType"], name != "outsideAny");
            }
            for expand in [false, true] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path(file),
                        position: position(source, selected),
                    },
                    expand,
                    depth: 3,
                });
                if expected.is_empty() {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}: {result:?}"
                    );
                    assert_eq!(
                        result.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}/{expand}"
                    );
                    assert!(result.issues.is_empty(), "{profile}/{name}: {result:?}");
                    if expand {
                        let graph = result.description.expansion.as_ref().unwrap();
                        assert!(
                            graph
                                .nodes
                                .iter()
                                .all(|node| !matches!(node.data, TypeNodeData::Unsupported { .. })),
                            "{profile}/{name}: {graph:?}"
                        );
                    }
                    continue;
                }
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                };
                assert_eq!(
                    error.kind,
                    SemanticErrorKind::InvalidSemantic,
                    "{profile}/{name}: {error:?}"
                );
                let first = &expected[0];
                assert_eq!(error.reason, first.3.into(), "{profile}/{name}: {error:?}");
                assert_eq!(
                    error.path.as_deref(),
                    Some(fixture.path(first.0).canonicalize().unwrap().as_path())
                );
                assert_eq!(
                    error.position,
                    Some(position(first.1, first.2)),
                    "{profile}/{name}: {error:?}"
                );
                let mut observed = vec![0];
                for cause in &error.causes {
                    let location = cause
                        .location
                        .as_ref()
                        .unwrap_or_else(|| panic!("unbound cause: {profile}/{name}: {error:?}"));
                    let index = expected
                        .iter()
                        .position(|event| {
                            location.file == event.0
                                && location.start == position(event.1, event.2)
                                && cause.reason == event.3.into()
                        })
                        .unwrap_or_else(|| {
                            panic!("unexpected acquisition: {profile}/{name}: {error:?}")
                        });
                    // Preserve the first acquisition order while allowing the
                    // same cached failure to be consumed by a later native read.
                    if !observed.contains(&index) {
                        observed.push(index);
                    }
                }
                assert_eq!(
                    observed,
                    (0..expected.len()).collect::<Vec<_>>(),
                    "{profile}/{name}: {error:?}"
                );
                assert!(
                    error.omitted_causes.is_none(),
                    "{profile}/{name}: {error:?}"
                );
                if expected.len() == 1 && first.3 == SemanticFailureReason::ErrorType {
                    assert!(error.causes.is_empty(), "{profile}/{name}: {error:?}");
                }
            }
        }
    }
}

/// These orders come from checker11213–11498 and28333–28380, not the compiler's
/// source-sorted diagnostic list. Set is requested before Get for a compound
/// target; a receiver failure returns its own actual error type; display
/// failures precede the access owner's newly produced error type.
fn expected_events<'a>(
    profile: &str,
    file: &'a str,
    source: &'a str,
    name: &str,
    selected: usize,
) -> Vec<Event<'a>> {
    use SemanticFailureReason::{ErrorType, UnresolvedSymbol};
    if matches!(profile, "helpers-recover" | "helpers-assign") {
        let get = (
            "helpers-recover.d.ts",
            RECOVER,
            RECOVER.find("MissingGet").unwrap(),
            UnresolvedSymbol,
        );
        let set = (
            "helpers-recover.d.ts",
            RECOVER,
            RECOVER.find("MissingSet").unwrap(),
            UnresolvedSymbol,
        );
        return match name {
            "get" | "repeatedGet" => vec![get],
            "set" => vec![set],
            "setThenGet" | "writeThenRead" => vec![set, get],
            _ => unreachable!(),
        };
    }
    if profile != "diagnostics" || name == "outsideAny" {
        return Vec::new();
    }
    let own = (file, source, selected, ErrorType);
    match name {
        "errorReceiver" => vec![(
            file,
            source,
            source.find("MissingReceiver").unwrap(),
            UnresolvedSymbol,
        )],
        "shadowedDisplay" => vec![
            (
                file,
                source,
                source.find("MissingShadowDisplay").unwrap(),
                UnresolvedSymbol,
            ),
            own,
        ],
        "nonexistentDisplay" => vec![
            // typeHasStaticProperty first asks getTypeOfSymbol of this
            // TypeLiteral symbol. Its non-value flags select errorType before
            // reportNonexistentProperty reaches annotation/display reads.
            (
                file,
                source,
                source.find("other./*#nonexistentDisplay*/").unwrap(),
                ErrorType,
            ),
            (
                file,
                source,
                source.find("MissingNonexistentDisplay").unwrap(),
                UnresolvedSymbol,
            ),
            own,
        ],
        _ => vec![own],
    }
}
