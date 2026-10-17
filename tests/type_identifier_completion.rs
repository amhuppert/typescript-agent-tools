//! Native identifier value recovery must not hide later operands or defaults.
//! Correlated-flow results retain the documented final admission distinction.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-identifier-completion/cases.ts");
const VALUES: &str = include_str!("../fixtures/types/native-identifier-completion/values.ts");

fn unique(source: &str, text: &str) -> usize {
    let mut matches = source.match_indices(text);
    let first = matches.next().unwrap_or_else(|| panic!("missing {text:?}"));
    assert!(matches.next().is_none(), "ambiguous {text:?}");
    first.0
}

#[test]
fn identifier_recovery_keeps_later_reads_and_native_default_gates() {
    check(&[
        "localThenLater",
        "importedThenLater",
        "assignmentThenLater",
        "defaultThenLater",
    ]);
}

#[test]
fn unchanged_correlated_reads_and_alias_suggestions_remain_healthy() {
    check(&[
        "aliasSuggestion",
        "defaultClean",
        "defaultUndefined",
        "defaultOmitted",
        "bindingPlain",
        "mutableUncorrelated",
        "parameterPlain",
        "contextualPlain",
        "defaultUncorrelated",
        "healthyAfter",
    ]);
}

#[test]
fn actual_correlated_changes_require_flow_admission() {
    check(&["bindingNarrowed", "parameterNarrowed", "contextualNarrowed"]);
}

fn check(names: &[&str]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        ("values.ts", VALUES),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-identifier-completion/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-identifier-completion/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-identifier-completion/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-identifier-completion/exact.json"),
        ),
    ]);
    for (profile, baseline) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-identifier-completion/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-identifier-completion/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-identifier-completion/expected-exact.json"),
        ),
    ] {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), 17);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut names = names.to_vec();
            if reverse {
                names.reverse();
            }
            for name in names {
                let row = native
                    .iter()
                    .find(|row| row["name"] == name)
                    .expect("maintained native row");
                let marker = format!("/*#{name}*/");
                let selected = unique(SOURCE, &marker) + marker.len();
                let missing = |text| {
                    (
                        "cases.ts",
                        SemanticFailureReason::UnresolvedSymbol,
                        unique(SOURCE, text),
                    )
                };
                let failures = match name {
                    "localThenLater" => vec![
                        missing("MissingIdentifierLocal"),
                        missing("MissingIdentifierLater"),
                    ],
                    "importedThenLater" => vec![
                        (
                            "values.ts",
                            SemanticFailureReason::UnresolvedSymbol,
                            unique(VALUES, "MissingIdentifierImported"),
                        ),
                        missing("MissingIdentifierLater"),
                    ],
                    "assignmentThenLater" => vec![
                        missing("MissingImmutable"),
                        (
                            "cases.ts",
                            SemanticFailureReason::ErrorType,
                            unique(SOURCE, "(immutable = 2)") + 1,
                        ),
                        missing("MissingIdentifierLater"),
                    ],
                    "defaultThenLater" if profile == "loose" => {
                        vec![missing("MissingIdentifierLater")]
                    }
                    "defaultThenLater" => vec![
                        missing("MissingParameterDefault"),
                        missing("MissingIdentifierLater"),
                    ],
                    _ => Vec::new(),
                };
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, selected),
                        },
                        expand,
                        depth: 2,
                    });
                    if name.ends_with("Narrowed") {
                        assert_eq!(row["errorType"], false);
                        assert!(row["codes"].as_array().unwrap().is_empty());
                        assert!(
                            matches!(&answer, Err(AnalysisError::Semantic(error))
                            if error.kind == SemanticErrorKind::UnsupportedSemantic
                                && error.reason == SemanticFailureReason::FlowRequired.into()),
                            "{profile}/{name}/{expand}: {answer:?}"
                        );
                        continue;
                    }
                    if failures.is_empty() {
                        assert_eq!(row["errorType"], false);
                        let Ok(TypeAnswer::Selected(result)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}")
                        };
                        assert_eq!(result.outcome, TypeOutcome::Found);
                        assert_eq!(
                            result.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{profile}/{name}/{expand}"
                        );
                        assert!(result.issues.is_empty());
                        continue;
                    }
                    assert_eq!(row["errorType"], true);
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}")
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    let (file, reason, offset) = failures[0];
                    let path = fixture.path(file).canonicalize().unwrap();
                    assert_eq!(
                        error.path.as_deref(),
                        Some(path.as_path()),
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(error.reason, reason.into(), "{profile}/{name}: {error:?}");
                    assert_eq!(
                        error.position,
                        Some(position(
                            if file == "values.ts" { VALUES } else { SOURCE },
                            offset
                        )),
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(
                        error.causes.len(),
                        failures.len() - 1,
                        "{profile}/{name}: {error:?}"
                    );
                    for (cause, &(file, reason, offset)) in error.causes.iter().zip(&failures[1..])
                    {
                        assert_eq!(cause.reason, reason.into(), "{profile}/{name}: {error:?}");
                        let location = cause
                            .location
                            .as_ref()
                            .expect("actual native dependency location");
                        assert_eq!(location.file, file);
                        assert_eq!(
                            location.start,
                            position(if file == "values.ts" { VALUES } else { SOURCE }, offset),
                            "{profile}/{name}: {error:?}"
                        );
                    }
                    assert!(error.omitted_causes.is_none());
                }
            }
        }
    }
}
