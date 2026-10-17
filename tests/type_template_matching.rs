//! Final consumers distinguish matching, constrained inference, and literal spelling.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-template-matching/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-template-matching/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-template-matching/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-template-matching/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-template-matching/exact.json"),
        ),
    ])
}
#[test]
fn template_matching_and_constraint_inference_keep_native_final_answers() {
    let fixture = fixture();
    let expected = [
        ("numberRound", "12"),
        ("numberLeading", "number"),
        ("numberRadix", "number"),
        ("numberSpace", "number"),
        ("numberEmpty", "\"invalid\""),
        ("numberInfinity", "\"invalid\""),
        ("numberZero", "number"),
        ("numberExponent", "number"),
        ("numberBom", "number"),
        ("numberNel", "\"invalid\""),
        ("numberZwsp", "\"invalid\""),
        ("bigRound", "12n"),
        ("bigRadix", "bigint"),
        ("bigSeparator", "\"invalid\""),
        ("bigSpace", "\"invalid\""),
        ("bigZero", "bigint"),
        ("bigLeading", "\"invalid\""),
        ("parts", "[\"left\", \"right\"]"),
        ("firstDelimiter", "[\"a\", \"b-c\"]"),
        ("unicode", "[\"👋\", \"x\"]"),
        ("single", "[\"x\", \"\"]"),
        ("genericParts", "[string, `${number}`]"),
        ("crossParts", "[`<${number}>`, string]"),
        ("numericKey", "\"numeric\""),
        ("badKey", "\"other\""),
        ("upperGood", "\"upper\""),
        ("upperBad", "\"other\""),
        ("nestedGood", "\"nested\""),
        ("nestedBad", "\"other\""),
        ("signedRadix", "\"invalid\""),
        ("numberPlus", "number"),
        ("numberBinary", "number"),
        ("numberOctal", "number"),
        ("numberHugeRadix", "number"),
        ("numberOverflowRadix", "\"invalid\""),
        ("numberOverflow", "\"invalid\""),
        ("numberUnderflow", "number"),
        ("numberDecimal", "-1.5"),
        ("numberSeparator", "\"invalid\""),
        ("numberLeadingEight", "number"),
        ("numberNan", "\"invalid\""),
        ("numberNegativeInfinity", "\"invalid\""),
        ("numberBadExponent", "\"invalid\""),
        ("numberArabic", "\"invalid\""),
        ("bigPlus", "\"invalid\""),
        ("bigNegativeRadix", "bigint"),
        ("bigBinary", "bigint"),
        ("bigOctal", "bigint"),
        ("bigTrailing", "\"invalid\""),
        ("bigMinusGap", "\"invalid\""),
        ("bigDoubleZero", "\"invalid\""),
        ("bigEight", "\"invalid\""),
        ("bigFraction", "\"invalid\""),
        ("bigExponent", "\"invalid\""),
        ("bigEmpty", "\"invalid\""),
        ("bigNegative", "-12n"),
        ("bigHuge", "9007199254740993123456789n"),
        ("bigNegativeLeading", "\"invalid\""),
        ("numberWhitespaceTab", "number"),
        ("numberWhitespaceVertical", "number"),
        ("numberWhitespaceForm", "number"),
        ("numberWhitespaceLf", "number"),
        ("numberWhitespaceCr", "number"),
        ("numberWhitespaceNbsp", "number"),
        ("numberWhitespaceOgham", "number"),
        ("numberWhitespaceEnquad", "number"),
        ("numberWhitespaceHair", "number"),
        ("numberWhitespaceLine", "number"),
        ("numberWhitespaceParagraph", "number"),
        ("numberWhitespaceNarrow", "number"),
        ("numberWhitespaceMath", "number"),
        ("numberWhitespaceIdeographic", "number"),
    ];
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (name, summary) in expected {
            let tag = format!("/*#{name}*/");
            for expand in [false, true] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
                    },
                    expand,
                    depth: 0,
                });
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{name}/{expand}: {:?}",
                    result.issues
                );
                assert_eq!(
                    result.description.summary, summary,
                    "{config}/{name}/{expand}"
                );
                assert!(result.issues.is_empty());
            }
        }
    }
}
