//! Compiler-derived final function factories and contextual parameter checks.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-function-expressions/cases.ts");
#[test]
fn function_factories_and_contextual_assignment_preserve_final_types() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-function-expressions/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-function-expressions/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-function-expressions/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-function-expressions/exact.json"),
        ),
    ]);
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("ordinary", "(value: number) => number"),
            ("named", "(value: string) => string"),
            ("arrow", "(value: number) => number"),
            ("generic", "<T>(value: T) => T"),
            ("initialized", "(value?: number) => number"),
            ("rest", "(...values: string[]) => string"),
            ("iife", "number"),
            ("context", "number"),
            ("contextParameter", "string"),
            ("contextThis", "number"),
            ("constantPredicate", "(x: number) => true"),
            ("constantComparison", "(x: number) => boolean"),
        ] {
            for expand in [false, true] {
                let label = format!("/*#{marker}*/");
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, SOURCE.find(&label).unwrap() + label.len()),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}")
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}/{expand}: {result:?}"
                );
                assert_eq!(
                    result.description.summary, summary,
                    "{config}/{marker}/{expand}"
                );
            }
        }
    }
}
