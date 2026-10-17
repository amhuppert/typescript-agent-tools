//! Final optional-chain calls consume native signature clones and return
//! transformations; receiver and call chains share their nullability owner.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn optional_calls_preserve_inner_markers_and_outer_undefined() {
    let source = include_str!("../fixtures/types/native-optional-calls/cases.ts");
    for (name, config) in [
        (
            "strict",
            include_str!("../fixtures/types/native-optional-calls/tsconfig.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-optional-calls/exact.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-optional-calls/loose.json"),
        ),
    ] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", config),
            ("cases.ts", source),
            (
                "globals.d.ts",
                include_str!("../fixtures/types/native-optional-calls/globals.d.ts"),
            ),
        ]);
        let project = fixture.load();
        let nullable = |value: &str| {
            if name == "loose" {
                value.to_owned()
            } else {
                format!("{value} | undefined")
            }
        };
        for (marker, summary) in [
            ("plain", nullable("{ value: \"plain\"; }")),
            ("plainProperty", nullable("\"plain\"")),
            ("inferred", nullable("{ value: number; }")),
            ("explicit", nullable("{ value: string; }")),
            ("inferredProperty", nullable("number")),
            ("overload", nullable("\"number\"")),
            ("receiver", nullable("{ value: number; }")),
            ("receiverProperty", nullable("number")),
            ("both", nullable("{ value: number; }")),
            ("bothProperty", nullable("number")),
            ("required", "1".to_owned()),
            ("any", "any".to_owned()),
            ("nested", nullable("\"nested\"")),
            ("grouped", "\"nested\"".to_owned()),
            ("explicitThis", nullable("\"receiver\"")),
            ("optionalThis", nullable("\"optionalReceiver\"")),
            ("nullableReturn", nullable("string")),
        ] {
            let tag = format!("/*#{marker}*/");
            let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(source, source.find(&tag).unwrap() + tag.len()),
            }));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == summary),
                "{name} {marker}: {answer:?}"
            );
        }
        for (marker, missing) in [
            ("anyBad", "MissingArgument"),
            ("missingReturn", "MissingReturn"),
        ] {
            let tag = format!("/*#{marker}*/");
            let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(source, source.find(&tag).unwrap() + tag.len()),
            }));
            let path = fixture.path("cases.ts").canonicalize().unwrap();
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref() == Some(path.as_path()) && error.position == Some(position(source, source.find(missing).unwrap()))),
                "{name} {marker}: {answer:?}"
            );
        }
    }
}
