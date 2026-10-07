#[path = "support/allocations.rs"]
mod allocations;

use allocations::counted;
use typescript_parser::{Value, parse};

#[test]
fn ordinary_numeric_literals_allocate_only_their_ast_fields() {
    let literals = 2_048;
    let source = format!("const values = [{}];", vec!["1"; literals].join(","));
    let (ast, allocations) = counted(|| parse(&source).unwrap());
    let mut found = 0;
    for node in ast.walk().map(|id| ast.node(id)) {
        if node.kind == "NumericLiteral" {
            found += 1;
            assert!(matches!(node.field("text"), Some(Value::Text(text)) if text == "1"));
            assert!(matches!(
                node.field("numericLiteralFlags"),
                Some(Value::Number(0))
            ));
        }
    }
    assert_eq!(found, literals);
    // Numeric nodes spill their two AST fields into one allocation each.
    // Arena/list growth and enclosing syntax get a fixed allowance.
    assert!(
        allocations <= literals + 64,
        "{allocations} allocations for {literals} numeric literals"
    );
}
