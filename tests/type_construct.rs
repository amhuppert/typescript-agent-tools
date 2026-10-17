//! Checked constructor acquisition and quick-query return types are separate
//! native operations. Expected types and consumed errors come from the pinned
//! same-checker constructor oracle, including its signature-sentinel evidence.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn constructors_preserve_quick_types_and_acquired_recovery() {
    let source = include_str!("../fixtures/types/native-construct/cases.ts");
    for (name, config) in [
        (
            "strict",
            include_str!("../fixtures/types/native-construct/tsconfig.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-construct/exact.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-construct/loose.json"),
        ),
    ] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", config),
            ("cases.ts", source),
            (
                "globals.d.ts",
                include_str!("../fixtures/types/native-construct/globals.d.ts"),
            ),
        ]);
        let project = fixture.load();
        for (marker, summary) in [
            ("public", Some("Box<number>")),
            ("explicit", Some("Box<string>")),
            ("newable", Some("{ readonly tag: \"newable\"; }")),
            ("any", Some("any")),
            ("anyBad", None),
            ("callOnly", Some("any")),
            (
                "callMissing",
                if name == "loose" { None } else { Some("any") },
            ),
            ("privateInside", Some("Private")),
            ("privateNested", Some("Private")),
            ("privateOutside", Some("Private")),
            ("protectedInside", Some("Protected")),
            ("protectedGrandchild", Some("Protected")),
            ("protectedOutside", Some("Protected")),
            ("inheritedOutside", Some("Derived")),
            ("abstract", Some("Abstract")),
            ("abstractSignature", Some("{ value: number; }")),
            ("voidParen", Some("{ value: \"void\"; }")),
            ("voidBare", Some("{ value: \"void\"; }")),
            ("bare", Some("{ value: \"empty\"; }")),
            ("argumentFailure", None),
            ("abstractArgument", None),
            ("genericAbstract", None),
            ("checkedPrivate", None),
            ("checkedAbstract", None),
            ("notConstructor", None),
            ("receiverCall", Some("any")),
            ("receiverMissing", None),
            ("unionAbstract", None),
        ] {
            let tag = format!("/*#{marker}*/");
            let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(source, source.find(&tag).unwrap() + tag.len()),
            }));
            match summary {
                Some(summary) => assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == summary),
                    "{name} {marker}: {answer:?}"
                ),
                None => {
                    let missing = match marker {
                        "anyBad" => Some("MissingArgument"),
                        "callMissing" => Some("MissingReturn"),
                        "argumentFailure" => Some("MissingConstructorArgument"),
                        "abstractArgument" => Some("MissingAbstractArgument"),
                        "receiverMissing" => Some("MissingThis"),
                        _ => None,
                    };
                    let reason = if missing.is_some() {
                        SemanticFailureReason::UnresolvedSymbol
                    } else {
                        SemanticFailureReason::ErrorType
                    };
                    let start = if let Some(missing) = missing {
                        source.find(missing).unwrap()
                    } else if marker == "checkedPrivate" {
                        source.find("new Private()./*#checkedPrivate*/").unwrap()
                    } else if marker == "checkedAbstract" {
                        source.find("new Abstract()./*#checkedAbstract*/").unwrap()
                    } else {
                        source.find(&tag).unwrap() + tag.len()
                    };
                    assert!(
                        matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == reason.into() && error.path.as_deref() == Some(fixture.path("cases.ts").canonicalize().unwrap().as_path()) && error.position == Some(position(source, start))),
                        "{name} {marker}: {answer:?}"
                    );
                }
            }
        }
    }
}
