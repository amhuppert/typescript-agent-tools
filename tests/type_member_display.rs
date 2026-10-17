//! Native inline serialization is distinct from raw member graph reads.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-member-display/cases.ts");

fn fixture(config: &str) -> Fixture {
    Fixture::new(&[("tsconfig.json", config), ("cases.ts", SOURCE)])
}

fn request(fixture: &Fixture, marker: &str, expand: bool, depth: usize) -> TypeRequest {
    let token = format!("/*#{marker}*/ ");
    let offset = SOURCE.find(&token).unwrap() + token.len();
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, offset),
    });
    request.expand = expand;
    request.depth = depth;
    request
}

fn configs() -> [&'static str; 3] {
    [
        include_str!("../fixtures/types/native-member-display/strict.json"),
        include_str!("../fixtures/types/native-member-display/exact.json"),
        include_str!("../fixtures/types/native-member-display/loose.json"),
    ]
}

fn assert_summary(marker: &str, summaries: [&str; 3]) {
    for (config, expected) in configs().into_iter().zip(summaries) {
        let fixture = fixture(config);
        let loaded = fixture.load();
        for (expand, depth) in [(false, 0), (true, 0), (true, 2)] {
            let answer = loaded.type_of(request(&fixture, marker, expand, depth));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!(
                    "the native inline serialization question must succeed ({marker}/{depth}): {answer:?}"
                );
            };
            assert_eq!(result.outcome, TypeOutcome::Found, "{marker}/{depth}");
            assert_eq!(result.description.summary, expected, "{marker}/{depth}");
            assert!(!result.description.summary_truncated);
            assert!(result.issues.is_empty(), "{result:?}");
        }
    }
}

#[test]
fn optional_property_summary_asks_nonmissing_type_without_changing_raw_read() {
    assert_summary(
        "property",
        [
            "{ value?: string | undefined; }",
            "{ value?: string; }",
            "{ value?: string; }",
        ],
    );
}

#[test]
fn optional_method_summary_asks_actual_signature_syntax() {
    assert_summary("method", ["{ value?(): number; }"; 3]);
}

#[test]
fn distinct_inline_accessor_summary_asks_both_actual_signatures() {
    assert_summary(
        "accessor",
        ["{ get value(): string; set value(next: number); }"; 3],
    );
}

#[test]
fn invalid_setter_consumed_by_inline_summary_stays_invalid_at_every_depth() {
    for config in configs() {
        let fixture = fixture(config);
        let loaded = fixture.load();
        for (expand, depth) in [(false, 0), (true, 0), (true, 2)] {
            let answer = loaded.type_of(request(&fixture, "ignoredBadSetter", expand, depth));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
                "native printer consumes this recovered setter dependency ({depth}): {answer:?}"
            );
            let Err(AnalysisError::Semantic(error)) = answer else {
                unreachable!()
            };
            assert_eq!(error.path, Some(fixture.path("cases.ts")));
            assert_eq!(
                error.position,
                Some(position(SOURCE, SOURCE.find("Missing").unwrap()))
            );
        }
    }
}

#[test]
fn empty_type_literals_share_native_identity_while_an_alias_keeps_its_own_identity() {
    for config in configs() {
        let fixture = fixture(config);
        let loaded = fixture.load();
        let answer = loaded.type_of(request(&fixture, "emptyIdentities", true, 3));
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("empty literals have complete native identities: {answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(
            result.description.summary,
            "(first: {}, second: {}, named: Empty) => Empty"
        );
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.as_ref().unwrap();
        let TypeNodeData::Function { members } = &graph.nodes[0].data else {
            panic!("the selected function has an actual signature");
        };
        let signatures = members.call_signatures.as_ref().unwrap();
        let [signature] = signatures.as_slice() else {
            panic!("one own function signature is retained");
        };
        assert_eq!(
            signature.parameters[0].value_type,
            signature.parameters[1].value_type
        );
        assert_ne!(
            signature.parameters[0].value_type,
            signature.parameters[2].value_type
        );
        assert_eq!(signature.parameters[2].value_type, signature.return_type);
        assert!(!graph.truncated);
    }
}
