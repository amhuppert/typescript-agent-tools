//! Await operand contextual typing keeps native absent and nil distinct.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn await_operand_contexts_preserve_actual_property_results() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-await-context"),
    );
    let source = std::fs::read_to_string(fixture.path("cases.ts")).unwrap();
    for config in ["tsconfig.json", "loose.json", "missing.json"] {
        let project = fixture.load_config(config);
        for marker in ["absent", "annotation", "return", "nil"] {
            let tag = format!("/*#{marker}*/");
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(&source, source.find(&tag).unwrap() + tag.len()),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                if config == "missing.json" && matches!(marker, "annotation" | "return") {
                    // Native createPromiseLikeType reads emptyGenericType and
                    // returns unknown after its checked getter fails. Product
                    // admission preserves that consumed recovery, even though
                    // the final native property type remains number.
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{config}/{marker}/{expand}: {answer:?}");
                    };
                    assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
                    assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
                    assert!(
                        error.causes.iter().any(|cause| {
                            cause.reason == SemanticFailureReason::UnresolvedSymbol.into()
                                && cause.message == "Cannot find global type 'PromiseLike'."
                        }),
                        "{error:?}"
                    );
                    // The async return context acquires its own PromiseLike
                    // constituent before the await operand does. Preserve
                    // that first consumer, rather than rebasing its recovery.
                    let start = if marker == "return" {
                        source.find("async function returning()").unwrap()
                    } else {
                        source[..source.find(&tag).unwrap()]
                            .rfind("await ")
                            .unwrap()
                    };
                    assert_eq!(error.position, Some(position(&source, start)));
                    continue;
                }
                let Ok(TypeAnswer::Selected(answer)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(answer.outcome, TypeOutcome::Found);
                assert_eq!(answer.description.summary, "number", "{config}/{marker}");
                assert!(answer.issues.is_empty(), "{config}/{marker}: {answer:?}");
            }
        }
    }
}
