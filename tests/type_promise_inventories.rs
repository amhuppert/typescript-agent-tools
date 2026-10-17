//! Native inventory/fact acquisition continues through recoverable dependencies.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-promise-inventories/cases.ts");
const GLOBAL: &str = include_str!("../fixtures/types/native-promise-inventories/global.ts");
const BAD_ALIAS: &str =
    include_str!("../fixtures/types/native-promise-inventories/awaited-bad.d.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        ("global.ts", GLOBAL),
        ("awaited-bad.d.ts", BAD_ALIAS),
        (
            "awaited-good.d.ts",
            include_str!("../fixtures/types/native-promise-inventories/awaited-good.d.ts"),
        ),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-promise-inventories/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-promise-inventories/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-promise-inventories/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-promise-inventories/exact.json"),
        ),
        (
            "global-absent.json",
            include_str!("../fixtures/types/native-promise-inventories/global-absent.json"),
        ),
        (
            "global-bad.json",
            include_str!("../fixtures/types/native-promise-inventories/global-bad.json"),
        ),
        (
            "global-good.json",
            include_str!("../fixtures/types/native-promise-inventories/global-good.json"),
        ),
    ])
}

#[test]
fn constrained_callbacks_and_member_inventories_keep_later_dependencies() {
    for (profile, captured) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-promise-inventories/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-promise-inventories/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-promise-inventories/expected-exact.json"),
        ),
    ] {
        check(profile, "cases.ts", SOURCE, captured);
    }
}

#[test]
fn global_awaited_alias_recovery_differs_from_absent_alias() {
    for (profile, captured) in [
        (
            "global-absent",
            include_str!(
                "../fixtures/types/native-promise-inventories/expected-global-absent.json"
            ),
        ),
        (
            "global-bad",
            include_str!("../fixtures/types/native-promise-inventories/expected-global-bad.json"),
        ),
        (
            "global-good",
            include_str!("../fixtures/types/native-promise-inventories/expected-global-good.json"),
        ),
    ] {
        check(profile, "global.ts", GLOBAL, captured);
    }
}

fn check(profile: &str, file: &str, source: &str, captured: &str) {
    let fixture = fixture();
    let project = fixture.load_config(&format!("{profile}.json"));
    let mut rows: Vec<serde_json::Value> = serde_json::from_str(captured).unwrap();
    // Every request has a fresh evaluator. Opposite order checks isolation,
    // not a claim about native warm-cache scheduling.
    for _ in 0..2 {
        for row in &rows {
            let name = row["name"].as_str().unwrap();
            let expected = dependencies(name, profile);
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path(file),
                    position: marked(source, '#', name),
                });
                request.expand = expand;
                request.depth = 2;
                let answer = project.type_of(request);
                if expected.is_empty() {
                    assert_eq!(row["errorType"], false, "{profile}/{name}");
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        answer.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}/{expand}: {answer:?}"
                    );
                    assert_eq!(
                        answer.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}/{expand}"
                    );
                    assert!(
                        answer.issues.is_empty(),
                        "{profile}/{name}/{expand}: {answer:?}"
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
                assert_eq!(
                    error.path.as_deref(),
                    Some(
                        fixture
                            .path(expected[0].1)
                            .canonicalize()
                            .unwrap()
                            .as_path()
                    )
                );
                assert_eq!(
                    error.position,
                    Some(expected[0].2),
                    "{profile}/{name}/{expand}: {error:?}"
                );
                let mut acquired = vec![0];
                for cause in &error.causes {
                    let at = cause.location.as_ref().expect("located native dependency");
                    let index = expected
                        .iter()
                        .position(|(reason, file, position)| {
                            cause.reason == (*reason).into()
                                && at.file == *file
                                && at.start == *position
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
        rows.reverse();
    }
}

fn dependencies(
    name: &str,
    profile: &str,
) -> Vec<(SemanticFailureReason, &'static str, SourcePosition)> {
    use SemanticFailureReason::{CircularType, ErrorType, UnresolvedSymbol};
    let missing = |site| (UnresolvedSymbol, "cases.ts", marked(SOURCE, '@', site));
    let await_error = || {
        let offset = marker_offset(SOURCE, '#', name);
        let line = SOURCE[offset..].split_once('\n').unwrap().0;
        (
            ErrorType,
            "cases.ts",
            position(SOURCE, offset + line.find("await ").unwrap()),
        )
    };
    match name {
        "callbackConstraint" => vec![
            missing("constraint"),
            await_error(),
            missing("constraintTail"),
        ],
        "callbackCircular" => vec![
            (CircularType, "cases.ts", marked(SOURCE, '@', "circular")),
            await_error(),
            missing("circularTail"),
        ],
        // resolveDeclaredMembers acquires indexes before the subsequent
        // first-parameter type read. Deferred references acquire arguments first.
        "thenInventory" => vec![
            missing("thenIndex"),
            missing("thenValue"),
            missing("thenTail"),
        ],
        "callbackInventory" => vec![
            missing("callbackIndex"),
            missing("callbackValue"),
            missing("callbackTail"),
        ],
        "deferredInventory" => vec![
            missing("deferredArgument"),
            missing("deferredIndex"),
            missing("deferredTail"),
        ],
        "unionInventory" | "intersectionInventory" => vec![
            missing("leftIndex"),
            missing("rightIndex"),
            missing("compositeTail"),
        ],
        "globalAliasTail" | "globalAliasHealthy" => {
            let mut result = Vec::new();
            if profile == "global-bad" {
                result.push((
                    UnresolvedSymbol,
                    "awaited-bad.d.ts",
                    marked(BAD_ALIAS, '@', "globalAwaited"),
                ));
            }
            if name == "globalAliasTail" {
                result.push((
                    UnresolvedSymbol,
                    "global.ts",
                    marked(GLOBAL, '@', "globalTail"),
                ));
            }
            result
        }
        "callbackHealthy"
        | "thenHealthy"
        | "callbackInventoryHealthy"
        | "deferredHealthy"
        | "compositeHealthy"
        | "falsyTemplateBase"
        | "falsyMappingBase" => vec![],
        _ => panic!("unclassified native inventory root {name}"),
    }
}

fn marked(source: &str, kind: char, name: &str) -> SourcePosition {
    position(source, marker_offset(source, kind, name))
}

fn marker_offset(source: &str, kind: char, name: &str) -> usize {
    let marker = format!("/*{kind}{name}*/");
    let mut matches = source.match_indices(&marker);
    let (offset, _) = matches.next().unwrap_or_else(|| panic!("missing {marker}"));
    assert!(matches.next().is_none(), "ambiguous {marker}");
    offset + marker.len()
}
