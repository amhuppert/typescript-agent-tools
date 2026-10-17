//! A candidate gap and the native failure it consumed have independent sites.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::references::{
    Location, ReferenceAnswer, ReferenceFilters, ReferenceOutcome, ReferenceQuery, ReferenceRequest,
};
use typescript_parser::types::{TypeQuery, TypeRequest};

fn at(file: &str, text: &str, needle: &str) -> Location {
    let start = text.find(needle).unwrap();
    Location {
        file: file.to_owned(),
        start: position(text, start),
        end: position(text, start + needle.len()),
    }
}

fn query(
    fixture: &Fixture,
    source: &str,
    config: &str,
) -> typescript_parser::references::ReferenceResult {
    let start = source.find("probe: string").unwrap();
    let project = fixture.load_config(config);
    let result = project
        .references(ReferenceRequest {
            query: ReferenceQuery::At {
                file: fixture.path("cases.ts"),
                position: position(source, start),
            },
            filters: ReferenceFilters::default(),
        })
        .unwrap();
    let ReferenceAnswer::Selected(result) = result else {
        panic!("member selection failed: {result:?}");
    };
    assert_eq!(result.outcome, ReferenceOutcome::Partial, "{result:?}");
    assert_eq!(result.coverage.gaps.len(), 1, "{result:?}");
    assert_eq!(result.groups.len(), 1, "{result:?}");
    assert_eq!(result.groups[0].occurrences.len(), 2, "{result:?}");
    result
}

#[test]
fn dependency_location_survives_source_error_conversions_and_stays_out_of_q4_leaves() {
    let source = "export {}; interface Healthy { probe: string } declare const healthy: Healthy; healthy.probe; declare const broken: MissingReceiver; broken.probe;";
    let fixture = Fixture::new(&[
        ("cases.ts", source),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true},"files":["cases.ts"]}"#,
        ),
        (
            "loose.json",
            r#"{"extends":"./tsconfig.json","compilerOptions":{"strict":false}}"#,
        ),
        (
            "exact.json",
            r#"{"extends":"./tsconfig.json","compilerOptions":{"exactOptionalPropertyTypes":true}}"#,
        ),
    ]);
    let candidate_start = source.find("broken.probe").unwrap() + "broken.".len();
    let candidate = Location {
        file: "cases.ts".to_owned(),
        start: position(source, candidate_start),
        end: position(source, candidate_start + 5),
    };
    let dependency = at("cases.ts", source, "MissingReceiver");
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let result = query(&fixture, source, config);
        let gap = &result.coverage.gaps[0];
        assert_eq!(gap.location, candidate, "{config}");
        assert_eq!(
            gap.reason,
            SemanticFailureReason::UnresolvedSymbol,
            "{config}"
        );
        // getSymbolOfNameOrPropertyAccessExpression acquires the receiver
        // during checking and again for the applicable-index fallback. The
        // completion contract preserves both acquisitions, including a cache hit.
        assert_eq!(gap.causes.len(), 2, "{config}: {gap:?}");
        assert_eq!(gap.causes[0], gap.causes[1]);
        assert_eq!(
            gap.causes[0].reason,
            SemanticFailureReason::UnresolvedSymbol
        );
        assert_eq!(gap.causes[0].location, Some(dependency.clone()));
        assert_eq!(gap.causes[0].message, gap.message);
        assert_eq!(gap.omitted_causes, None);
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let mut request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: dependency.start,
            });
            request.expand = expand;
            let Err(AnalysisError::Semantic(error)) = project.type_of(request) else {
                panic!("invalid dependency became a Q4 result");
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
            let wire = serde_json::to_value(error).unwrap();
            assert!(wire.get("exactLocation").is_none());
            assert!(wire.get("exact_location").is_none());
            assert_eq!(
                wire["position"],
                serde_json::to_value(dependency.start).unwrap()
            );
        }
    }
}

#[test]
fn missing_global_cause_keeps_nil_location_after_the_located_primary_dependency() {
    let source = "export {}; interface Healthy { probe: string } declare const healthy: Healthy; healthy.probe; declare const broken: number[]; broken.probe;";
    let fixture = Fixture::new(&[
        ("cases.ts", source),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true},"files":["cases.ts"]}"#,
        ),
        (
            "loose.json",
            r#"{"extends":"./tsconfig.json","compilerOptions":{"strict":false}}"#,
        ),
        (
            "exact.json",
            r#"{"extends":"./tsconfig.json","compilerOptions":{"exactOptionalPropertyTypes":true}}"#,
        ),
    ]);
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let result = query(&fixture, source, config);
        let gap = &result.coverage.gaps[0];
        assert_eq!(
            gap.reason,
            SemanticFailureReason::ErrorType,
            "{config}: {gap:?}"
        );
        // Native checking continues through Object fallback and the absent
        // property before replaying the receiver for applicable-index lookup.
        // Repeated acquired completions are ordered evidence, not a set.
        let receiver = at("cases.ts", source, "number[]");
        let candidate_start = source.find("broken.probe").unwrap() + "broken.".len();
        let candidate = Location {
            file: "cases.ts".into(),
            start: position(source, candidate_start),
            end: position(source, candidate_start + 5),
        };
        let expected = [
            (
                SemanticFailureReason::ErrorType,
                Some(receiver.clone()),
                "global type 'Array' dependency consumed compiler recovery",
            ),
            (
                SemanticFailureReason::UnresolvedSymbol,
                None,
                "Cannot find global type 'Array'.",
            ),
            (
                SemanticFailureReason::ErrorType,
                Some(candidate.clone()),
                "global type 'Object' dependency consumed compiler recovery",
            ),
            (
                SemanticFailureReason::UnresolvedSymbol,
                None,
                "Cannot find global type 'Object'.",
            ),
            (
                SemanticFailureReason::ErrorType,
                Some(candidate.clone()),
                "global type 'Object' dependency consumed compiler recovery",
            ),
            (
                SemanticFailureReason::UnresolvedSymbol,
                None,
                "Cannot find global type 'Object'.",
            ),
            (
                SemanticFailureReason::ErrorType,
                Some(candidate),
                "property and applicable index signature are absent",
            ),
            (
                SemanticFailureReason::ErrorType,
                Some(receiver),
                "global type 'Array' dependency consumed compiler recovery",
            ),
            (
                SemanticFailureReason::UnresolvedSymbol,
                None,
                "Cannot find global type 'Array'.",
            ),
        ];
        assert_eq!(gap.causes.len(), expected.len(), "{config}: {gap:?}");
        for (cause, (reason, location, message)) in gap.causes.iter().zip(expected) {
            assert_eq!(
                (cause.reason, cause.location.clone(), cause.message.as_str()),
                (reason, location, message),
                "{config}"
            );
        }
        assert_eq!(gap.omitted_causes, None);
        let wire = serde_json::to_value(&result).unwrap();
        assert!(
            wire["coverage"]["gaps"][0]["causes"][1]
                .get("location")
                .is_none()
        );
    }
}

#[test]
fn dependency_causes_keep_the_actual_unsearched_file_range() {
    let source = "import { broken } from 'dep'; interface Healthy { probe: string } declare const healthy: Healthy; healthy.probe; broken.probe;";
    let dependency = "export declare const broken: MissingReceiver;";
    let fixture = Fixture::new(&[
        ("cases.ts", source),
        (
            "node_modules/dep/package.json",
            r#"{"name":"dep","types":"index.d.ts"}"#,
        ),
        ("node_modules/dep/index.d.ts", dependency),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true},"files":["cases.ts"]}"#,
        ),
    ]);
    let result = query(&fixture, source, "tsconfig.json");
    let gap = &result.coverage.gaps[0];
    let candidate_start = source.find("broken.probe").unwrap() + "broken.".len();
    assert_eq!(
        gap.location,
        Location {
            file: "cases.ts".to_owned(),
            start: position(source, candidate_start),
            end: position(source, candidate_start + 5),
        }
    );
    assert_eq!(gap.reason, SemanticFailureReason::UnresolvedSymbol);
    assert_eq!(gap.causes.len(), 2, "{gap:?}");
    assert_eq!(gap.causes[0], gap.causes[1]);
    assert_eq!(
        gap.causes[0].reason,
        SemanticFailureReason::UnresolvedSymbol
    );
    assert_eq!(
        gap.causes[0].location,
        Some(at(
            "node_modules/dep/index.d.ts",
            dependency,
            "MissingReceiver"
        ))
    );
    assert_eq!(gap.omitted_causes, None);
}

#[test]
fn missing_module_dependency_keeps_its_original_module_literal_range() {
    let source = "import { broken } from 'dep'; interface Healthy { probe: string } declare const healthy: Healthy; healthy.probe; broken.probe;";
    let dependency = "import { Missing } from 'lost'; export declare const broken: Missing;";
    let fixture = Fixture::new(&[
        ("cases.ts", source),
        (
            "node_modules/dep/package.json",
            r#"{"name":"dep","types":"index.d.ts"}"#,
        ),
        ("node_modules/dep/index.d.ts", dependency),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true,"skipLibCheck":true},"files":["cases.ts"]}"#,
        ),
    ]);
    let result = query(&fixture, source, "tsconfig.json");
    let gap = &result.coverage.gaps[0];
    assert_eq!(gap.reason, SemanticFailureReason::UnresolvedSymbol);
    assert_eq!(gap.causes.len(), 1, "{gap:?}");
    assert_eq!(
        gap.causes[0].reason,
        SemanticFailureReason::UnresolvedSymbol
    );
    assert_eq!(
        gap.causes[0].location,
        Some(at("node_modules/dep/index.d.ts", dependency, "'lost'"))
    );
    assert_eq!(gap.omitted_causes, None);
}
