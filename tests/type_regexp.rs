//! checkRegularExpressionLiteral consumes the actual initialized global.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-regexp/cases.ts");
#[test]
fn regexp_literals_and_dependent_reads_use_the_native_global() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/native-regexp"),
    );
    for profile in ["valid", "missing", "arity", "kind"] {
        let project = fixture.load_config(&format!("{profile}.json"));
        for marker in ["literal", "escaped", "read"] {
            for expand in [false, true] {
                let tag = format!("/*#{marker}*/");
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                if profile == "valid" {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{profile}/{marker}/{expand}: {answer:?}")
                    };
                    assert_eq!(result.outcome, TypeOutcome::Found);
                    assert_eq!(
                        result.description.summary,
                        if marker == "read" { "string" } else { "RegExp" }
                    );
                    assert!(result.issues.is_empty());
                } else {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{marker}/{expand}: {answer:?}")
                    };
                    assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic, "{error:?}");
                    assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
                    assert!(error.message.contains("RegExp"));
                }
            }
        }
    }
}
