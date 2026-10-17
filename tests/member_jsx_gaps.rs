//! JSX attribute uses of a props member are compiler reference candidates
//! related through the contextual props type. JSX attribute typing is deferred,
//! so each such candidate is a located `jsx_typing_required` gap while the
//! ordinary declaration, destructuring and property-access uses stay grouped.
//! Expected relationships come from the pinned compiler control
//! tools/type-native-jsx-member-gaps-oracle.test.mjs.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::SemanticFailureReason;
use typescript_parser::references::{
    Location, ReferenceAnswer, ReferenceFilters, ReferenceOutcome, ReferenceQuery, ReferenceRequest,
};

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-jsx-member-gaps"),
    )
}

/// The name that follows `/*#marker*/` in `file`.
fn marked(fixture: &Fixture, file: &str, marker: &str) -> Location {
    let text = std::fs::read_to_string(fixture.path(file)).unwrap();
    let tag = format!("/*#{marker}*/");
    let start = text.find(&tag).unwrap() + tag.len();
    let end = start + "maxCollapsedHeight".len();
    assert_eq!(&text[start..end], "maxCollapsedHeight");
    Location {
        file: file.to_owned(),
        start: position(&text, start),
        end: position(&text, end),
    }
}

#[test]
fn jsx_attribute_candidates_are_located_jsx_typing_gaps() {
    let fixture = fixture();
    let declaration = marked(&fixture, "comp.tsx", "prop");
    let project = fixture.load_config("tsconfig.json");
    let answer = project
        .references(ReferenceRequest {
            query: ReferenceQuery::At {
                file: fixture.path("comp.tsx"),
                position: declaration.start,
            },
            filters: ReferenceFilters::default(),
        })
        .unwrap();
    let ReferenceAnswer::Selected(result) = answer else {
        panic!("member selection failed: {answer:?}");
    };
    assert_eq!(result.outcome, ReferenceOutcome::Partial, "{result:?}");
    let gaps: Vec<_> = result
        .coverage
        .gaps
        .iter()
        .map(|gap| (gap.reason, gap.location.clone()))
        .collect();
    assert_eq!(
        gaps,
        [
            (
                SemanticFailureReason::JsxTypingRequired,
                marked(&fixture, "use.tsx", "attrFirst")
            ),
            (
                SemanticFailureReason::JsxTypingRequired,
                marked(&fixture, "use.tsx", "attrSecond")
            ),
        ],
        "{result:?}"
    );
    assert_eq!(result.groups.len(), 1, "{result:?}");
    let occurrences: Vec<_> = result.groups[0]
        .occurrences
        .iter()
        .map(|occurrence| (occurrence.location.file.clone(), occurrence.location.start))
        .collect();
    let destructured = {
        let text = std::fs::read_to_string(fixture.path("comp.tsx")).unwrap();
        position(&text, text.find("{ maxCollapsedHeight = 3").unwrap() + 2)
    };
    assert_eq!(
        occurrences,
        [
            ("comp.tsx".to_owned(), declaration.start),
            ("comp.tsx".to_owned(), destructured),
            (
                "comp.tsx".to_owned(),
                marked(&fixture, "comp.tsx", "access").start
            ),
        ],
        "{result:?}"
    );
}

#[test]
fn jsx_result_types_are_explicit_excluded_queries() {
    use typescript_parser::analysis::{AnalysisError, SemanticErrorKind};
    use typescript_parser::types::{TypeQuery, TypeRequest};
    for jsx in ["<div />", "<div></div>", "<></>"] {
        let source = format!("const selected = {jsx};");
        let fixture = Fixture::new(&[
            ("cases.tsx", &source),
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"strict":true,"jsx":"preserve","types":[]},"files":["cases.tsx"]}"#,
            ),
        ]);
        let project = fixture.load();
        for expand in [false, true] {
            let mut request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.tsx"),
                position: position(&source, source.find("selected").unwrap()),
            });
            request.expand = expand;
            let answer = project.type_of(request);
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{jsx}: {answer:?}")
            };
            assert_eq!(error.kind, SemanticErrorKind::UnsupportedSemantic);
            assert_eq!(
                error.reason,
                SemanticFailureReason::JsxTypingRequired.into(),
                "{jsx}: {error:?}"
            );
            assert_eq!(
                error.position,
                Some(position(&source, source.find('<').unwrap()))
            );
        }
    }
}
