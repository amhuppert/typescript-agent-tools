//! Q5 construction sites against the pinned TypeScript 7.0.2 checker: every
//! object literal of the fixture, per target, option profile and literal
//! universe, as `tools/type-constructs-oracle.test.mjs` froze it.
mod support;
use serde_json::{Value, json};
use support::Fixture;
use typescript_parser::analysis::{AnalysisError, QueryErrorKind, SemanticFailureReason};
use typescript_parser::constructs::{
    ConstructsAnswer, ConstructsOutcome, ConstructsRequest, ConstructsResult, Presence,
};
use typescript_parser::references::Location;
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeKey, TypeQuery};

fn fixture() -> Fixture {
    fixture_at("constructs")
}

fn fixture_at(directory: &str) -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types")
            .join(directory),
    )
}

fn span(location: &Location) -> String {
    format!(
        "{}:{}:{}-{}:{}",
        location.file,
        location.start.line,
        location.start.column,
        location.end.line,
        location.end.column
    )
}

fn at(fixture: &Fixture, row: &Value) -> TypeQuery {
    let target = &row["target_at"];
    TypeQuery::At {
        file: fixture.path(target["file"].as_str().unwrap()),
        position: SourcePosition {
            line: target["line"].as_u64().unwrap() as usize,
            column: target["column"].as_u64().unwrap() as usize,
        },
    }
}

/// The oracle's rows for each target, joined with the target anchors.
fn expected(fixture: &Fixture, profile: &str) -> Vec<Value> {
    let targets: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(fixture.path("targets.json")).unwrap())
            .unwrap();
    let rows: Vec<Value> = serde_json::from_str(
        &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(rows.len(), targets.len());
    rows.into_iter()
        .zip(targets)
        .map(|(mut row, target)| {
            assert_eq!(row["name"], target["name"]);
            row["target_at"] = target["at"].clone();
            row
        })
        .collect()
}

/// The product's sites in the oracle's row vocabulary.
fn product_sites(result: &ConstructsResult) -> Vec<Value> {
    result
        .sites
        .iter()
        .map(|site| {
            json!({
                "at": span(&site.location),
                "context": site.contextual_type.summary,
                "constructed": site.constructs.iter().map(|construction| json!({
                    "alternative": construction.alternative.trim_start_matches('a').parse::<u64>().unwrap(),
                    "summary": construction.summary.summary,
                    "lacks": construction.lacks.iter().map(|lack| {
                        let mut row = json!({
                            "optional": lack.optional,
                            "presence": match lack.presence {
                                Presence::Absent => "absent",
                                Presence::Unknown => "unknown",
                            },
                        });
                        // The oracle names a string key bare and others with
                        // their kind.
                        let (key, kind) = match &lack.key {
                            TypeKey::String { name } => (name, None),
                            TypeKey::Private { name, .. } => (name, Some("private")),
                            TypeKey::Symbol { name, .. } => (name, Some("symbol")),
                        };
                        row["key"] = json!(key);
                        if let Some(kind) = kind {
                            row["kind"] = json!(kind);
                        }
                        row
                    }).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

/// The oracle's rows of the literals a universe examines: every one, or
/// those the compiler-side rule marks for the default universe.
fn examined(row: &Value, all_literals: bool) -> Vec<&Value> {
    row["literals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|literal| all_literals || literal["examinedByDefault"] == true)
        .collect()
}

/// The compiler's sites among the examined literals: those constructing a
/// target alternative, whose checked type is not a recovery type (those are
/// gaps instead).
fn oracle_sites(row: &Value, all_literals: bool) -> (Vec<Value>, Vec<String>) {
    let mut sites = Vec::new();
    let mut gaps = Vec::new();
    for literal in examined(row, all_literals) {
        let Some(constructed) = literal["constructed"].as_array() else {
            continue;
        };
        if constructed.is_empty() {
            continue;
        }
        if literal["literalError"] == true || literal["contextError"] == true {
            gaps.push(literal["at"].as_str().unwrap().to_owned());
            continue;
        }
        sites.push(json!({
            "at": literal["at"],
            "context": literal["context"],
            "constructed": constructed,
        }));
    }
    (sites, gaps)
}

#[test]
fn construction_sites_match_the_pinned_compiler_for_every_literal() {
    // Every family in a fixture is compared, not merely an empty set: the
    // sites and gaps each universe finds, where the default universe omits
    // the eight sites whose literals' named members name only keys their
    // target does not declare.
    for (directory, profile, all_literals, totals) in [
        ("constructs", "tsconfig", false, (158, 1)),
        ("constructs", "tsconfig", true, (166, 1)),
        ("constructs", "loose", false, (158, 1)),
        ("constructs", "loose", true, (166, 1)),
        ("constructs-object", "tsconfig", false, (7, 0)),
        ("constructs-object", "tsconfig", true, (7, 0)),
        ("constructs-object-index", "tsconfig", false, (6, 0)),
        ("constructs-object-index", "tsconfig", true, (6, 0)),
    ] {
        let fixture = fixture_at(directory);
        let project = fixture.load_config(&format!("{profile}.json"));
        let (mut all_sites, mut all_gaps) = (0, 0);
        for row in expected(&fixture, profile) {
            let name = row["name"].as_str().unwrap();
            let name = &format!("{directory} {name} (all literals: {all_literals})");
            let mut request = ConstructsRequest::new(at(&fixture, &row));
            request.filters.all_literals = all_literals;
            let answer = project.constructs(request);
            if row.get("unsupported").is_some() {
                match answer {
                    Err(AnalysisError::Query(error)) => {
                        assert_eq!(
                            error.kind,
                            QueryErrorKind::UnsupportedTarget,
                            "{profile} {name}"
                        )
                    }
                    other => panic!("{profile} {name}: expected UnsupportedTarget, got {other:?}"),
                }
                continue;
            }
            let ConstructsAnswer::Selected(result) =
                answer.unwrap_or_else(|error| panic!("{profile} {name}: {error}"))
            else {
                panic!("{profile} {name}: target not selected");
            };
            assert_eq!(
                result.target.summary.summary, row["target"],
                "{profile} {name}"
            );
            let alternatives: Vec<&str> = result
                .target
                .alternatives
                .iter()
                .map(|alternative| alternative.summary.summary.as_str())
                .collect();
            assert_eq!(json!(alternatives), row["alternatives"], "{profile} {name}");
            let (sites, gaps) = oracle_sites(&row, all_literals);
            assert_eq!(product_sites(&result), sites, "{profile} {name}");
            let product_gaps: Vec<String> = result
                .coverage
                .gaps
                .iter()
                .map(|gap| {
                    assert_eq!(
                        gap.reason,
                        SemanticFailureReason::ErrorType,
                        "{profile} {name}"
                    );
                    span(&gap.location)
                })
                .collect();
            assert_eq!(product_gaps, gaps, "{profile} {name}");
            assert_eq!(
                result.outcome,
                if gaps.is_empty() {
                    ConstructsOutcome::Complete
                } else {
                    ConstructsOutcome::Partial
                },
                "{profile} {name}"
            );
            assert_eq!(
                result.coverage.examined,
                examined(&row, all_literals).len(),
                "{profile} {name}"
            );
            assert_eq!(
                result.filters.all_literals, all_literals,
                "{profile} {name}"
            );
            assert_eq!(
                result.coverage.literals,
                row["literals"].as_array().unwrap().len(),
                "{profile} {name}"
            );
            assert_eq!(
                json!(result.coverage.universe),
                if all_literals {
                    json!("all")
                } else {
                    row["universe"].clone()
                },
                "{profile} {name}"
            );
            assert_eq!(
                result.coverage.sites,
                result.sites.len(),
                "{profile} {name}"
            );
            all_sites += result.sites.len();
            all_gaps += product_gaps.len();
        }
        assert_eq!(
            (all_sites, all_gaps),
            totals,
            "{directory} {profile} all literals: {all_literals}"
        );
    }
}

/// Literals whose context or presence needs a deferred operation are located
/// gaps; the compiler decides each of them, and every other site still
/// matches it. A narrowed read in a literal without spreads is not a gap:
/// its presence needs no initializer type.
#[test]
fn deferred_operations_are_located_gaps_and_other_sites_still_match() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/constructs-gaps"),
    );
    let project = fixture.load();
    let row = expected(&fixture, "tsconfig").remove(0);
    let ConstructsAnswer::Selected(result) = project
        .constructs(ConstructsRequest::new(at(&fixture, &row)))
        .unwrap()
    else {
        panic!("View not selected");
    };
    let gaps: Vec<(String, SemanticFailureReason)> = result
        .coverage
        .gaps
        .iter()
        .map(|gap| (span(&gap.location), gap.reason))
        .collect();
    assert_eq!(
        gaps,
        [
            (
                "flow.ts:6:26-6:53".into(),
                SemanticFailureReason::FlowRequired
            ),
            (
                "flow.ts:14:10-14:32".into(),
                SemanticFailureReason::FlowRequired
            ),
            (
                "flow.ts:18:25-18:54".into(),
                SemanticFailureReason::UnsupportedSemanticForm
            ),
            (
                "flow.ts:26:8-26:40".into(),
                SemanticFailureReason::OverloadRecovery
            ),
            (
                "view.tsx:5:33-5:60".into(),
                SemanticFailureReason::UnsupportedSemanticForm
            ),
        ]
    );
    let (sites, _) = oracle_sites(&row, false);
    let decided: Vec<Value> = sites
        .into_iter()
        .filter(|site| !gaps.iter().any(|(at, _)| site["at"] == at.as_str()))
        .collect();
    assert_eq!(product_sites(&result), decided);
    // `flow.ts:22:22` takes its context from `satisfies`: the call around it
    // needs a narrowed read to resolve, which its contextual type never asks.
    assert_eq!(decided.len(), 2);
    assert_eq!(result.outcome, ConstructsOutcome::Partial);
    assert_eq!(result.coverage.examined, 9);
}

fn selected(answer: Result<ConstructsAnswer, AnalysisError>) -> ConstructsResult {
    match answer {
        Ok(ConstructsAnswer::Selected(result)) => *result,
        other => panic!("expected a selected result, got {other:?}"),
    }
}

/// `--missing`, `--no-tests` and `--file` keep the sites the compiler's rows
/// say they should; selection failures and targets without construction
/// sites are answered as such, not as empty results.
#[test]
fn filters_select_compiler_sites_and_refusals_name_their_cause() {
    use typescript_parser::constructs::ConstructsFilters;
    let fixture = fixture();
    let project = fixture.load();
    let row = expected(&fixture, "tsconfig").remove(0);
    assert_eq!(row["name"], "View");
    let (sites, _) = oracle_sites(&row, false);
    let request = |filters: ConstructsFilters| ConstructsRequest {
        query: at(&fixture, &row),
        filters,
    };

    let lacking_lane: Vec<Value> = sites
        .iter()
        .filter(|site| {
            site["constructed"]
                .as_array()
                .unwrap()
                .iter()
                .any(|construction| {
                    construction["lacks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|lack| lack["key"] == "lane")
                })
        })
        .cloned()
        .collect();
    let missing = selected(project.constructs(request(ConstructsFilters {
        missing: Some("lane".into()),
        ..Default::default()
    })));
    assert_eq!(product_sites(&missing), lacking_lane);
    assert!(lacking_lane.len() < sites.len() && !lacking_lane.is_empty());
    assert_eq!(missing.filters.missing.as_deref(), Some("lane"));

    match project.constructs(request(ConstructsFilters {
        missing: Some("lanes".into()),
        ..Default::default()
    })) {
        Err(AnalysisError::Query(error)) => {
            assert_eq!(error.kind, QueryErrorKind::UnknownProperty);
            assert!(
                error.message.contains("did you mean `lane`?"),
                "{}",
                error.message
            );
        }
        other => panic!("expected UnknownProperty, got {other:?}"),
    }
    // The empty string is a key like any other: View does not declare it.
    match project.constructs(request(ConstructsFilters {
        missing: Some(String::new()),
        ..Default::default()
    })) {
        Err(AnalysisError::Query(error)) => assert_eq!(error.kind, QueryErrorKind::UnknownProperty),
        other => panic!("expected UnknownProperty, got {other:?}"),
    }

    // The literals of the default universe, in all files and in some.
    let literals_in = |prefix: &str| {
        examined(&row, false)
            .into_iter()
            .filter(|literal| literal["at"].as_str().unwrap().starts_with(prefix))
            .count()
    };
    let total = examined(&row, false).len();
    let no_tests = selected(project.constructs(request(ConstructsFilters {
        exclude_tests: true,
        ..Default::default()
    })));
    let outside_tests: Vec<Value> = sites
        .iter()
        .filter(|site| !site["at"].as_str().unwrap().starts_with("tests/"))
        .cloned()
        .collect();
    assert_eq!(product_sites(&no_tests), outside_tests);
    assert!(outside_tests.len() < sites.len());
    assert_eq!(no_tests.coverage.examined, total - literals_in("tests/"));

    let in_unions = selected(project.constructs(request(ConstructsFilters {
        file: Some(fixture.path("unions.ts")),
        ..Default::default()
    })));
    let union_sites: Vec<Value> = sites
        .iter()
        .filter(|site| site["at"].as_str().unwrap().starts_with("unions.ts:"))
        .cloned()
        .collect();
    assert_eq!(product_sites(&in_unions), union_sites);
    assert_eq!(in_unions.coverage.examined, literals_in("unions.ts:"));
    assert_eq!(
        in_unions.filters.file.as_deref(),
        Some(std::path::Path::new("unions.ts"))
    );

    let by_name = selected(project.constructs(ConstructsRequest::new(TypeQuery::Name {
        name: "View".into(),
        file: None,
    })));
    assert_eq!(product_sites(&by_name), sites);
    match project
        .constructs(ConstructsRequest::new(TypeQuery::Name {
            name: "Nowhere".into(),
            file: None,
        }))
        .unwrap()
    {
        ConstructsAnswer::Unselected(selection) => assert!(matches!(
            selection.outcome,
            typescript_parser::types::SelectionFailure::NotFound { .. }
        )),
        other => panic!("expected not_found, got {other:?}"),
    }

    let unsupported = |query: TypeQuery| match project.constructs(ConstructsRequest::new(query)) {
        Err(AnalysisError::Query(error)) => {
            assert_eq!(error.kind, QueryErrorKind::UnsupportedTarget)
        }
        other => panic!("expected UnsupportedTarget, got {other:?}"),
    };
    // A function's value question, and a type reference in an annotation.
    unsupported(TypeQuery::Name {
        name: "returns".into(),
        file: None,
    });
    unsupported(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: SourcePosition {
            line: 14,
            column: 26,
        },
    });
}
