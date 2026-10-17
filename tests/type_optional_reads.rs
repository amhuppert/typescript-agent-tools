//! Optional-chain binding preserves native branch lifetime and read identity.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-optional-flow/optional.ts");
const STRICT: &str = include_str!("../fixtures/types/native-optional-flow/tsconfig.json");
const LOOSE: &str = include_str!("../fixtures/types/native-optional-flow/loose.json");

#[test]
fn optional_chain_forms_preserve_unrelated_read_identity() {
    let fixture = Fixture::new(&[
        ("optional.ts", SOURCE),
        ("tsconfig.json", STRICT),
        ("loose.json", LOOSE),
    ]);
    let markers: Vec<_> = SOURCE
        .match_indices("/*")
        .map(|(start, _)| {
            let end = SOURCE[start..].find("*/ ").unwrap() + start;
            (&SOURCE[start + 2..end], end + 3)
        })
        .collect();
    assert_eq!(markers.len(), 13);
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for &(marker, offset) in &markers {
            let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("optional.ts"),
                position: position(SOURCE, offset),
            }));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "native optional identity must succeed ({config}, {marker}): {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.target.kind, TypeQuestion::Expression);
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, "number", "{config}: {marker}");
            assert!(result.issues.is_empty());
        }
    }
}
