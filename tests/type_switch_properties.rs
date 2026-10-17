//! Switch properties use the actual clause-group operation and its cache.
//! Native selected identity controls F admission; acquired errors remain ordered.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, SemanticError, SemanticErrorKind, SemanticFailureReason,
};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-switch-properties/cases.ts");
const PROFILES: [(&str, &str); 3] = [
    (
        "tsconfig",
        include_str!("../fixtures/types/native-switch-properties/expected-tsconfig.json"),
    ),
    (
        "loose",
        include_str!("../fixtures/types/native-switch-properties/expected-loose.json"),
    ),
    (
        "exact",
        include_str!("../fixtures/types/native-switch-properties/expected-exact.json"),
    ),
];

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-switch-properties"),
    )
}

fn selected(name: &str) -> SourcePosition {
    let marker = format!("/*#{name}*/");
    position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len())
}

fn request(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: selected(name),
        },
        expand,
        depth: 1,
    }
}

fn dependencies(name: &str) -> &'static [&'static str] {
    match name {
        "caseRecovery" => &["MissingCaseFirst", "MissingCaseSecond"],
        "groupRecovery" => &["MissingGroupFirst", "MissingGroupSecond"],
        "defaultRecovery" => &["MissingDefaultFirst", "MissingDefaultSecond"],
        "smallRecovery" => &["MissingSmallFirst", "MissingSmallSecond"],
        "emptyNameRecovery" => &["MissingEmptyFirst", "MissingEmptySecond"],
        "noiseTen" | "noiseEleven" => &["MissingKeyNoise0"],
        "keyBeforeCases" => &[
            "MissingKeyNoise0",
            "MissingKeyCaseFirst",
            "MissingKeyCaseSecond",
        ],
        _ => &[],
    }
}

fn semantic(answer: Result<TypeAnswer, AnalysisError>, label: &str) -> SemanticError {
    let Err(AnalysisError::Semantic(error)) = answer else {
        panic!("{label}: {answer:?}");
    };
    *error
}

/// Keep every repeated acquisition. The returned indices identify annotation
/// roles, allowing distinct declarations to prove the same cache schedule.
fn recovery_events(
    fixture: &Fixture,
    error: &SemanticError,
    name: &str,
    changed: bool,
    label: &str,
) -> Vec<usize> {
    let expected: Vec<_> = dependencies(name)
        .iter()
        .map(|name| position(SOURCE, SOURCE.find(name).unwrap()))
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(
        error.kind,
        SemanticErrorKind::InvalidSemantic,
        "{label}: {error:?}"
    );
    assert_eq!(
        error.reason,
        SemanticFailureReason::UnresolvedSymbol.into(),
        "{label}: {error:?}"
    );
    assert_eq!(
        error.path.as_deref(),
        Some(fixture.path("cases.ts").as_path())
    );
    assert_eq!(error.position, Some(expected[0]), "{label}: {error:?}");
    assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
    let mut events = vec![0];
    let mut flow = false;
    for cause in &error.causes {
        let at = cause
            .location
            .as_ref()
            .expect("each switch acquisition has an actual source site");
        assert_eq!(at.file, "cases.ts", "{label}: {error:?}");
        if cause.reason == SemanticFailureReason::FlowRequired.into() {
            assert!(changed && !flow, "{label}: {error:?}");
            assert_eq!(at.start, selected(name), "{label}: {error:?}");
            flow = true;
        } else {
            assert!(
                !flow,
                "final F admission must follow all acquisitions: {label}: {error:?}"
            );
            assert_eq!(
                cause.reason,
                SemanticFailureReason::UnresolvedSymbol.into(),
                "{label}: {error:?}"
            );
            events.push(
                expected
                    .iter()
                    .position(|expected| *expected == at.start)
                    .unwrap_or_else(|| panic!("unexpected switch dependency: {label}: {error:?}")),
            );
        }
    }
    assert_eq!(flow, changed, "{label}: {error:?}");
    let mut first_occurrences = Vec::new();
    for &event in &events {
        if !first_occurrences.contains(&event) {
            first_occurrences.push(event);
        }
    }
    assert_eq!(
        first_occurrences,
        (0..expected.len()).collect::<Vec<_>>(),
        "{label}: {error:?}"
    );
    events
}

#[test]
fn switch_name_threshold_and_group_gates_preserve_native_final_identity() {
    let fixture = fixture();
    for (profile, baseline) in PROFILES {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), 40);
        let rows: Vec<_> = native
            .iter()
            .filter(|row| dependencies(row["name"].as_str().unwrap()).is_empty())
            .collect();
        assert_eq!(rows.len(), 32);
        for reverse in [false, true] {
            // Separate requests test isolation; they do not establish warm hits.
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut rows = rows.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["errorType"], false);
                assert!(row["acquired"].as_array().unwrap().is_empty());
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                assert!(
                    row["codes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|code| code == 2318)
                );
                for expand in [false, true] {
                    // Expansion independently acquires these latent properties;
                    // compact queries isolate the native key-map skip gates.
                    if expand && matches!(name, "noiseNine" | "emptyNameSkipsMap") {
                        continue;
                    }
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(request(&fixture, name, expand));
                    if row["sameDeclared"] == false {
                        let error = semantic(answer, &label);
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::UnsupportedSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::FlowRequired.into(),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.path.as_deref(),
                            Some(fixture.path("cases.ts").as_path())
                        );
                        assert_eq!(error.position, Some(selected(name)), "{label}: {error:?}");
                        assert!(error.causes.is_empty(), "{label}: {error:?}");
                        assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                    } else {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{label}"
                        );
                        assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                        if expand {
                            assert!(
                                answer
                                    .description
                                    .expansion
                                    .as_ref()
                                    .unwrap()
                                    .nodes
                                    .iter()
                                    .all(|node| !matches!(
                                        node.data,
                                        TypeNodeData::Unsupported { .. }
                                    )),
                                "{label}: {answer:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn recovered_key_and_case_reads_continue_before_final_flow_admission() {
    let fixture = fixture();
    for (profile, baseline) in PROFILES {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        let rows: Vec<_> = native
            .iter()
            .filter(|row| !dependencies(row["name"].as_str().unwrap()).is_empty())
            .collect();
        assert_eq!(rows.len(), 8);
        let project = fixture.load_config(&format!("{profile}.json"));
        for row in rows {
            let name = row["name"].as_str().unwrap();
            assert_eq!(row["errorType"], false);
            assert!(row["demandFailures"].as_array().unwrap().is_empty());
            assert_eq!(
                row["codes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|code| *code == 2304)
                    .count(),
                dependencies(name).len()
            );
            assert!(
                row["acquired"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|read| read["errorType"] == true)
            );
            for expand in [false, true] {
                let label = format!("{profile}/{name}/{expand}");
                let error = semantic(project.type_of(request(&fixture, name, expand)), &label);
                recovery_events(&fixture, &error, name, row["sameDeclared"] == false, &label);
            }
        }
    }
}

#[test]
fn unknown_key_candidate_replays_the_completed_clause_vector_in_fallback() {
    let fixture = fixture();
    for (profile, _) in PROFILES {
        let project = fixture.load_config(&format!("{profile}.json"));
        for expand in [false, true] {
            let label = format!("{profile}/caseRecovery/{expand}");
            let error = semantic(
                project.type_of(request(&fixture, "caseRecovery", expand)),
                &label,
            );
            let once = recovery_events(&fixture, &error, "caseRecovery", true, &label);
            for name in [
                "groupRecovery",
                "defaultRecovery",
                "smallRecovery",
                "emptyNameRecovery",
            ] {
                let label = format!("{profile}/{name}/{expand}");
                let error = semantic(project.type_of(request(&fixture, name, expand)), &label);
                let events = recovery_events(&fixture, &error, name, false, &label);
                // Native flow1212 reads the complete vector before key lookup.
                // Unknown candidate repeats that getter through flow1078; the
                // small/empty-name paths enter only the fallback. Source proves
                // this schedule; the native observer establishes presence only.
                let expected = if matches!(name, "groupRecovery" | "defaultRecovery") {
                    once.iter().chain(&once).copied().collect::<Vec<_>>()
                } else {
                    once.clone()
                };
                assert_eq!(events, expected, "{label}: {error:?}");
            }
        }
    }
}
