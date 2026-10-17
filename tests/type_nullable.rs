//! Actual global aliases control nullable adjustment; spelling is not a shortcut.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-nullable/cases.ts");

#[test]
fn optional_receivers_consume_actual_global_non_nullable_aliases() {
    let variants = [
        (
            "missing",
            include_str!("../fixtures/types/native-nullable/missing.d.ts"),
        ),
        (
            "kind",
            include_str!("../fixtures/types/native-nullable/kind.d.ts"),
        ),
        (
            "arity",
            include_str!("../fixtures/types/native-nullable/arity.d.ts"),
        ),
        (
            "zero",
            include_str!("../fixtures/types/native-nullable/zero.d.ts"),
        ),
        (
            "custom",
            include_str!("../fixtures/types/native-nullable/custom.d.ts"),
        ),
        (
            "identity",
            include_str!("../fixtures/types/native-nullable/identity.d.ts"),
        ),
        (
            "intersection",
            include_str!("../fixtures/types/native-nullable/intersection.d.ts"),
        ),
        (
            "broken",
            include_str!("../fixtures/types/native-nullable/broken.d.ts"),
        ),
    ];
    for strict in [true, false] {
        for (variant, declaration) in variants {
            let config = format!(
                r#"{{"compilerOptions":{{"strict":{strict},"noLib":true,"types":[],"target":"esnext"}},"files":["globals.d.ts","alias.d.ts","cases.ts"]}}"#
            );
            let fixture = Fixture::new(&[
                ("cases.ts", SOURCE),
                ("alias.d.ts", declaration),
                (
                    "globals.d.ts",
                    include_str!("../fixtures/types/native-nullable/globals.d.ts"),
                ),
                ("tsconfig.json", &config),
            ]);
            let project = fixture.load_config("tsconfig.json");
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, SOURCE.find("probe").unwrap()),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                if strict && variant == "broken" {
                    assert!(
                        matches!(answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic),
                        "broken global alias must remain invalid"
                    );
                } else {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{strict}/{variant}: {answer:?}");
                    };
                    assert_eq!(result.outcome, TypeOutcome::Found, "{strict}/{variant}");
                    assert_eq!(
                        result.description.summary,
                        if strict && variant == "custom" {
                            "\"custom\" | undefined"
                        } else {
                            "any"
                        },
                        "{strict}/{variant}"
                    );
                }
            }
        }
    }
}
