//! Native unresolved aliases keep actual argument values and every consumed failure.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-unresolved-type-aliases/cases.ts");

#[test]
fn unresolved_alias_names_and_shared_types_preserve_each_actual_failure_site() {
    check(&[
        "plain",
        "escaped",
        "qualified",
        "deepQualified",
        "existingNamespace",
        "sameArguments",
        "sameName",
        "differentParents",
    ]);
}

#[test]
fn recovered_type_arguments_and_defaults_continue_in_native_order() {
    check(&[
        "arguments",
        "nestedArguments",
        "knownArguments",
        "discardedArguments",
        "defaults",
        "discardedDefaults",
        "partialDefaults",
        "union",
        "intersection",
    ]);
}

#[test]
fn arity_recovery_skips_supplied_children_and_healthy_arguments_skip_defaults() {
    check(&[
        "wrongNonGeneric",
        "wrongFew",
        "wrongMany",
        "suppliedDefaults",
        "suppliedDiscardedDefaults",
        "genuineAny",
        "healthyPair",
        "healthyIgnoredAny",
        "healthyNamespace",
        "healthyAfter",
    ]);
}

fn check(names: &[&str]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-unresolved-type-aliases/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-unresolved-type-aliases/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-unresolved-type-aliases/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-unresolved-type-aliases/exact.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-unresolved-type-aliases/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-unresolved-type-aliases/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-unresolved-type-aliases/expected-exact.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(rows.len(), 27);
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
                let selected = marked('#', name);
                let failures = failures(name);
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, selected),
                    });
                    request.expand = expand;
                    request.depth = 2;
                    let answer = project.type_of(request);
                    if failures.is_empty() {
                        assert_eq!(row["errorType"], false);
                        assert_eq!(row["codes"], serde_json::json!([]));
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
                        failures[0].0.into(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, failure_offset(failures[0].1))),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(
                        error.causes.len(),
                        failures.len() - 1,
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    for (cause, (reason, marker)) in error.causes.iter().zip(&failures[1..]) {
                        assert_eq!(
                            cause.reason,
                            (*reason).into(),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                        let location = cause
                            .location
                            .as_ref()
                            .expect("type dependency has its actual source");
                        assert_eq!(location.file, "cases.ts");
                        assert_eq!(
                            location.start,
                            position(SOURCE, failure_offset(marker)),
                            "{profile}/{name}/{expand}: {error:?}"
                        );
                    }
                    assert!(error.omitted_causes.is_none());
                }
            }
        }
    }
}

fn failures(name: &str) -> Vec<(SemanticFailureReason, &'static str)> {
    let missing: &[&str] = match name {
        "plain" => &["plain"],
        "escaped" => &["escaped"],
        "qualified" => &["qualified"],
        "deepQualified" => &["deepQualified"],
        "existingNamespace" => &["existingNamespace"],
        "arguments" => &["arguments", "argumentLeft", "argumentRight"],
        "nestedArguments" => &["outer", "inner", "leaf", "sibling"],
        "sameArguments" => &[
            "sameFirst",
            "sameFirstArgument",
            "sameSecond",
            "sameSecondArgument",
        ],
        "sameName" => &["sameNameFirst", "sameNameSecond"],
        "differentParents" => &["parentFirst", "parentSecond"],
        "knownArguments" => &["knownLeft", "knownRight"],
        "discardedArguments" => &["discardedLeft", "discardedRight"],
        "defaults" => &["defaultLeft", "defaultRight"],
        "discardedDefaults" => &["ignoredDefaultLeft", "ignoredDefaultRight"],
        "partialDefaults" => &["defaultRight"],
        "union" => &["unionFirst", "unionLast"],
        "intersection" => &["intersectionFirst", "intersectionLast"],
        "wrongNonGeneric" => return vec![(SemanticFailureReason::ErrorType, "wrongNonGeneric")],
        "wrongFew" => return vec![(SemanticFailureReason::ErrorType, "wrongFew")],
        "wrongMany" => return vec![(SemanticFailureReason::ErrorType, "wrongMany")],
        "suppliedDefaults"
        | "suppliedDiscardedDefaults"
        | "genuineAny"
        | "healthyPair"
        | "healthyIgnoredAny"
        | "healthyNamespace"
        | "healthyAfter" => &[],
        _ => panic!("unclassified native case {name}"),
    };
    missing
        .iter()
        .map(|&marker| (SemanticFailureReason::UnresolvedSymbol, marker))
        .collect()
}

// resolveEntityName reports an absent export on the right identifier. A
// missing root namespace is instead reported on the leftmost identifier.
fn failure_offset(marker: &str) -> usize {
    marked('@', marker)
        + if marker == "existingNamespace" {
            "Available.".len()
        } else {
            0
        }
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
