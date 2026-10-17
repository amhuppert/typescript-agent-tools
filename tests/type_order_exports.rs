//! Native canonical symbol names differ from written declaration names.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCES: &[(&str, &str)] = &[
    (
        "default.ts",
        include_str!("../fixtures/types/native-type-order-exports/default.ts"),
    ),
    (
        "imported.ts",
        include_str!("../fixtures/types/native-type-order-exports/imported.ts"),
    ),
    (
        "normal.ts",
        include_str!("../fixtures/types/native-type-order-exports/normal.ts"),
    ),
    (
        "escaped.ts",
        include_str!("../fixtures/types/native-type-order-exports/escaped.ts"),
    ),
    (
        "tsconfig.json",
        include_str!("../fixtures/types/native-type-order-exports/tsconfig.json"),
    ),
];

#[test]
fn native_declared_symbol_names_order_default_exports_escaped_names_and_parameters() {
    let fixture = Fixture::new(SOURCES);
    let project = fixture.load_config("tsconfig.json");
    for (file, marker, expected) in [
        ("default.ts", "local", "Zebra | Alpha"),
        ("imported.ts", "imported", "Zebra | Alpha"),
        ("normal.ts", "normal", "Alpha | Zebra"),
        ("escaped.ts", "escaped", "Alpha | \\u005aebra"),
        ("escaped.ts", "parameters", "Alpha | Zebra"),
    ] {
        let source = SOURCES.iter().find(|(name, _)| *name == file).unwrap().1;
        let tag = format!("/*#{marker}*/");
        for expand in [false, true, false, true] {
            let mut request = TypeRequest::new(TypeQuery::At {
                file: fixture.path(file),
                position: position(source, source.find(&tag).unwrap() + tag.len()),
            });
            request.expand = expand;
            request.depth = 1;
            let answer = project.type_of(request);
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("native canonical name union must establish ({marker}): {answer:?}");
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, expected, "{marker}/{expand}");
            assert!(result.issues.is_empty(), "{marker}: {:?}", result.issues);
        }
    }
}
