//! Warm comparison keys must not admit a different overload's argument.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-relation-key-identity/cases.ts");

#[test]
fn canonical_relation_keys_preserve_parameters_targets_and_nested_arguments() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-relation-key-identity"),
    );
    for profile in ["strict", "nocheck"] {
        let project = fixture.load_config(&format!("{profile}.json"));
        for marker in ["parameter", "target", "nested", "constrained"] {
            let offset = SOURCE.find(&format!("{marker}Chosen =")).unwrap();
            for expand in [false, true] {
                let request = TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, offset),
                    },
                    expand,
                    depth: 2,
                };
                let TypeAnswer::Selected(answer) = project.type_of(request).unwrap() else {
                    panic!("expected selected {profile}/{marker}");
                };
                assert_eq!(answer.outcome, TypeOutcome::Found, "{profile}/{marker}");
                assert_eq!(answer.description.summary, "number", "{profile}/{marker}");
                assert!(answer.issues.is_empty(), "{profile}/{marker}");
            }
        }
    }
}
