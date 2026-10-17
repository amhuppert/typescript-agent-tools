//! Native diagnostic traversal retains every acquired property read, including caches.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-recovery-display/cases.ts");

#[test]
fn native_display_continues_across_properties_and_accessor_error_type_gates() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-recovery-display/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-recovery-display/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-recovery-display/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-recovery-display/exact.json"),
        ),
    ]);
    let cases: [(&str, &[&str]); 8] = [
        (
            "manyDisplays",
            &[
                "MissingFirstDisplay",
                "MissingSecondDisplay",
                "MissingLastDisplay",
            ],
        ),
        (
            "accessorDisplays",
            &["MissingRead", "MissingWrite", "MissingTail"],
        ),
        ("healthyAccessor", &[]),
        ("anyAccessor", &[]),
        ("missingRead", &["MissingReadOnly", "MissingReadTail"]),
        ("missingWrite", &["MissingWriteOnly", "MissingWriteTail"]),
        ("ordinary", &["MissingOrdinary", "MissingOrdinaryTail"]),
        ("healthyAfter", &[]),
    ];
    for profile in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut cases = cases.to_vec();
            if reverse {
                cases.reverse();
            }
            for (name, missing) in cases {
                let marker = format!("/*#{name}*/");
                let offset = SOURCE.find(&marker).unwrap() + marker.len();
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, offset),
                    });
                    request.expand = expand;
                    let answer = project.type_of(request);
                    if missing.is_empty() {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}: {answer:?}")
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found);
                        assert_eq!(
                            answer.description.summary,
                            if name == "healthyAfter" {
                                "number"
                            } else {
                                "boolean"
                            }
                        );
                        assert!(answer.issues.is_empty());
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}: {answer:?}")
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, SOURCE.find(missing[0]).unwrap()))
                    );
                    // Ordinary property serialization obtains the symbol type and
                    // then its display type. Accessor-specific paths are below.
                    let acquisitions: Vec<_> = missing
                        .iter()
                        .flat_map(|missing| {
                            let copies = if matches!(
                                *missing,
                                "MissingRead"
                                    | "MissingWrite"
                                    | "MissingReadOnly"
                                    | "MissingWriteOnly"
                            ) {
                                1
                            } else {
                                2
                            };
                            std::iter::repeat_n(*missing, copies)
                        })
                        .collect();
                    let numeric = usize::from(name == "manyDisplays");
                    assert_eq!(
                        error.causes.len(),
                        acquisitions.len() - 1 + numeric,
                        "{profile}/{name}: {error:?}"
                    );
                    for (cause, missing) in error.causes.iter().zip(&acquisitions[1..]) {
                        assert_eq!(cause.reason, SemanticFailureReason::UnresolvedSymbol.into());
                        let location = cause
                            .location
                            .as_ref()
                            .expect("declared missing type location");
                        assert_eq!(location.file, "cases.ts");
                        assert_eq!(
                            location.start,
                            position(SOURCE, SOURCE.find(missing).unwrap())
                        );
                    }
                    if numeric != 0 {
                        let cause = error.causes.last().unwrap();
                        assert_eq!(cause.reason, SemanticFailureReason::ErrorType.into());
                        let initializer = offset + SOURCE[offset..].find(" = ").unwrap() + 3;
                        assert_eq!(
                            cause.location.as_ref().unwrap().start,
                            position(SOURCE, initializer)
                        );
                    }
                    assert!(error.omitted_causes.is_none());
                }
            }
        }
    }
}
