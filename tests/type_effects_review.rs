//! Adversarial final-boundary controls for native call-effect acquisition.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

#[test]
fn a_json_property_without_an_explicit_annotation_has_no_dotted_call_effect_type() {
    // Fresh pinned API and same-checker hook both return Number/errorfalse,
    // with no call diagnostic or consumed wrapper recovery. resolveAlias must
    // acquire the JSON module, but getExplicitTypeOfSymbol then returns nil
    // for this own property without an annotation.
    let source = "import { count } from \"./data.json\";\nlet healthy = 1;\ncount();\n/*selected*/ healthy;\n";
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            "{\"compilerOptions\":{\"strict\":true,\"noLib\":true,\"resolveJsonModule\":true,\"types\":[]},\"files\":[\"read.ts\"]}",
        ),
        ("data.json", "{\"count\":1}"),
        ("read.ts", source),
    ]);
    let offset = source.find("/*selected*/").unwrap() + "/*selected*/ ".len();
    let answer = fixture.load().type_of(TypeRequest::new(TypeQuery::At {
        file: fixture.path("read.ts"),
        position: position(source, offset),
    }));
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.target.kind == TypeQuestion::Expression && result.description.summary == "number"),
        "a JSON property's inferred value cannot substitute for its absent explicit annotation: {answer:?}"
    );
}

#[test]
fn dotted_call_effect_lookup_recurses_parentheses_and_skips_other_expression_types() {
    let source = include_str!("../fixtures/types/native-effects-review/effects.ts");
    for config in ["tsconfig.json", "loose.json"] {
        let configuration = if config == "tsconfig.json" {
            include_str!("../fixtures/types/native-effects-review/tsconfig.json")
        } else {
            include_str!("../fixtures/types/native-effects-review/loose.json")
        };
        let fixture = Fixture::new(&[
            ("tsconfig.json", configuration),
            ("effects.ts", source),
            (
                "globals.d.ts",
                include_str!("../fixtures/types/native-effects-review/globals.d.ts"),
            ),
        ]);
        let project = fixture.load();
        // The conditional callee also requires its native binder flow graph;
        // its RED belongs to that owner rather than dotted type acquisition.
        for marker in [
            "parenthesized",
            "comma",
            "nonnull",
            "element",
            "nested-call",
            "arrow",
            "conditional",
            "number",
            "string",
            "object",
            "pure-matching",
            "pure-unknown",
            "pure-nested",
            "assigned-branches",
            "dotted-method",
            "dotted-property",
            "dotted-missing",
            "dotted-unannotated",
            "dotted-parentheses",
            "dotted-this",
            "dotted-arrow-this",
            "dotted-class-this",
            "dotted-static-this",
        ] {
            let comment = format!("/*{marker}*/ ");
            let offset = source.find(&comment).unwrap() + comment.len();
            let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("effects.ts"),
                position: position(source, offset),
            }));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.target.kind == TypeQuestion::Expression && result.description.summary == "number"),
                "{config}: {marker} must use the dotted syntax question before any expression-type demand: {answer:?}"
            );
        }
    }
}

#[test]
fn dotted_missing_property_consumes_missing_global_augmentation() {
    use typescript_parser::analysis::{AnalysisError, SemanticErrorKind};
    let source = include_str!("../fixtures/types/native-effects-review/effects.ts");
    let fixture = Fixture::new(&[
        ("effects.ts", source),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-effects-review/missing.json"),
        ),
    ]);
    let project = fixture.load();
    for (marker, recovery) in [("dotted-method", false), ("dotted-missing", true)] {
        let tag = format!("/*{marker}*/ ");
        let answer = project.type_of(TypeRequest::new(TypeQuery::At {
            file: fixture.path("effects.ts"),
            position: position(source, source.find(&tag).unwrap() + tag.len()),
        }));
        if recovery {
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{answer:?}");
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert!(error.message.contains("global type 'Object'"), "{error:?}");
        } else {
            assert!(
                matches!(answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == "number")
            );
        }
    }
}
