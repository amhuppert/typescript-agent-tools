//! Contextual binary operands follow the exact native operator/side dispatch.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-binary-context/cases.ts");

#[test]
fn binary_context_preserves_operator_and_operand_distinctions() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-binary-context/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-binary-context/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-binary-context/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-binary-context/exact.json"),
        ),
    ]);
    for (profile, captured) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-binary-context/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-binary-context/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-binary-context/expected-exact.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(captured).unwrap();
        assert_eq!(rows.len(), 18);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut rows: Vec<_> = rows.iter().collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let mut matches = SOURCE.match_indices(&marker);
                let offset = matches.next().expect("native selected marker").0 + marker.len();
                assert!(matches.next().is_none());
                assert_eq!(row["errorType"], false);
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, offset),
                    });
                    request.expand = expand;
                    let answer = project.type_of(request);
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}")
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found);
                    assert_eq!(
                        answer.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}/{expand}"
                    );
                    assert!(
                        answer.issues.is_empty(),
                        "{profile}/{name}/{expand}: {:?}",
                        answer.issues
                    );
                }
            }
        }
    }
}
