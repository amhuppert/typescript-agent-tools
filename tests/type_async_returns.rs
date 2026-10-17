//! Native async return adoption, contextual inference and whole-body widening.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-async-returns/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-async-returns/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-async-returns/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-async-returns/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-async-returns/exact.json"),
        ),
    ])
}

#[test]
fn async_factories_and_contextual_returns_match_the_pinned_compiler() {
    check(false);
}

#[test]
fn async_recovery_and_widening_keep_ordered_later_dependencies() {
    check(true);
}

fn check(recovery: bool) {
    for (profile, captured) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-async-returns/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-async-returns/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-async-returns/expected-exact.json"),
        ),
    ] {
        let fixture = fixture();
        let project = fixture.load_config(&format!("{profile}.json"));
        let mut rows: Vec<serde_json::Value> = serde_json::from_str(captured).unwrap();
        // A Project request owns a fresh evaluator. Reversed requests prove
        // isolation, not a native within-request cache-hit schedule.
        for _ in 0..2 {
            for row in &rows {
                let name = row["name"].as_str().unwrap();
                let expected = dependencies(name);
                if recovery == expected.is_empty() {
                    continue;
                }
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: marked('#', name),
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
                    // The native outer function/Promise is non-error. Its
                    // inferred return nevertheless consumed real recovery.
                    assert_eq!(row["errorType"], false, "{profile}/{name}");
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
                        Some(fixture.path("cases.ts").canonicalize().unwrap().as_path())
                    );
                    assert_eq!(
                        error.position,
                        Some(expected[0].1),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    let mut acquired = vec![0];
                    for cause in &error.causes {
                        let at = cause
                            .location
                            .as_ref()
                            .expect("located async return dependency");
                        let index = expected
                            .iter()
                            .position(|(reason, start)| {
                                cause.reason == (*reason).into()
                                    && at.file == "cases.ts"
                                    && at.start == *start
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
}

fn dependencies(name: &str) -> Vec<(SemanticFailureReason, SourcePosition)> {
    use SemanticFailureReason::{ErrorType, UnresolvedSymbol};
    let missing = |name| (UnresolvedSymbol, marked('@', name));
    let invalid = || {
        let selected = marker_offset('#', name);
        let start = if name == "invalidArrow" {
            selected + SOURCE[selected..].find("async ").unwrap()
        } else {
            SOURCE[..selected].rfind("async function ").unwrap()
        };
        (ErrorType, position(SOURCE, start))
    };
    match name {
        "invalid" | "invalidArrow" => vec![invalid()],
        "missing" => vec![missing("missingValue")],
        "later" => vec![missing("missingValue"), missing("secondValue")],
        "invalidLater" => vec![invalid(), missing("missingValue")],
        "wideningLazy" => vec![missing("lazyTail")],
        "scalar" | "direct" | "nested" | "structural" | "any" | "empty" | "bare" | "self"
        | "awaitSelf" | "arrow" | "literal" | "annotated" | "contextual" | "contextualEmpty"
        | "nullWidening" | "siblingWidening" => vec![],
        _ => panic!("unclassified native async root {name}"),
    }
}

fn marked(kind: char, name: &str) -> SourcePosition {
    position(SOURCE, marker_offset(kind, name))
}

fn marker_offset(kind: char, name: &str) -> usize {
    let marker = format!("/*{kind}{name}*/");
    let mut matches = SOURCE.match_indices(&marker);
    let (offset, _) = matches.next().unwrap_or_else(|| panic!("missing {marker}"));
    assert!(matches.next().is_none(), "ambiguous {marker}");
    offset + marker.len()
}
