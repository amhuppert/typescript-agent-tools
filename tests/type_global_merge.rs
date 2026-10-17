//! Global interfaces declared in different script files merge into one
//! symbol, and so do their type parameters: `getSymbolOfDeclaration` is
//! `getMergedSymbol(getLateBoundSymbol(node.Symbol))`, so each declaration's
//! `T` names the same parameter and the merged interface keeps one
//! type-parameter list. The same question decides outer type parameters of
//! anonymous member types and `this` in either declaration, overload order
//! for a global function declared in two files, and the setter that supplies
//! a computed getter's contextual return type.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn cross_file_interface_merges_keep_one_type_parameter_list() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-global-merge"),
    );
    let source = std::fs::read_to_string(fixture.path("main.ts")).unwrap();
    // Expected answers come from the pinned compiler control
    // tools/type-native-global-merge-oracle.test.mjs.
    let expected = [
        ("array", "number[]"),
        ("at", "number | undefined"),
        ("merged", "string"),
        ("own", "string"),
        ("firstNested", "string"),
        ("secondNested", "string"),
        ("callback", "(value: string) => void"),
        ("self", "Box<string>"),
        ("element", "number"),
        ("picked", "\"second-number\""),
        ("green", "Color.Green"),
        ("literalGetter", "number"),
        ("symbolGetter", "string"),
        ("plainGetter", "boolean"),
    ];
    let markers = source.matches("/*#").count();
    assert_eq!(markers, expected.len());
    for config in ["tsconfig.json", "nocheck.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in expected {
            let tag = format!("/*#{marker}*/");
            let at = position(&source, source.find(&tag).unwrap() + tag.len());
            let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("main.ts"),
                position: at,
            }));
            let Ok(TypeAnswer::Selected(result)) = &answer else {
                panic!("{config}/{marker}: {answer:?}");
            };
            assert_eq!(
                result.outcome,
                TypeOutcome::Found,
                "{config}/{marker}: {answer:?}"
            );
            assert_eq!(result.description.summary, summary, "{config}/{marker}");
            assert!(result.issues.is_empty(), "{config}/{marker}");
        }
    }
}
