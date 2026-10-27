// validation: run alone: it asserts that parses finish within ten seconds, which other test executables' load can break.
use typescript_parser::{ParseOptions, Value, parse, parse_with_options};

#[test]
fn ordinary_declarations_permit_in_inside_for_initializer_functions() {
    for source in [
        "for (let x = function () { const y = a in b; }; ; ) {}",
        "for (let x = () => { const y = a in b; }; ; ) {}",
        "for (let x = class { m() { const y = a in b; } }; ; ) {}",
        "for (let x = { m() { const y = a in b; } }; ; ) {}",
        "for (let x = function () { const y = a in b; }; x in xs; ) {}",
    ] {
        let ast = parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
        let declaration_list = ast
            .walk()
            .map(|id| ast.node(id))
            .find(|node| {
                node.kind == "VariableDeclarationList"
                    && node.span.start > source.find('{').unwrap()
            })
            .unwrap();
        // The compiler constructs the list after restoring the inherited
        // DisallowIn flag, while its ordinary declarations permit `in`.
        assert_eq!(declaration_list.flags, 8194, "{source}");
        let initializer = ast
            .walk()
            .map(|id| ast.node(id))
            .find(|node| {
                node.kind == "BinaryExpression" && node.span.end <= source.find(';').unwrap()
            })
            .unwrap();
        assert_eq!(initializer.flags, 0, "{source}");
    }

    // The statement override must not change restrictions on either nested
    // loop headers or concise arrow bodies in the original initializer.
    for source in [
        "for (let x = function () { for (const y = a in b; ; ) {} }; ; ) {}",
        "for (let x = () => a in b; ; ) {}",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
}

#[test]
fn nested_loop_headers_restore_each_expression_context() {
    for source in [
        "for (let x = function () { for (const y = a; ; ) { const z = a in b; } }; ; ) {}",
        "for (let x = function () { for (const y of a in b) {} }; ; ) {}",
        "for (let x = function () { for (const y in a in b) {} }; ; ) {}",
        "for (let x = function () { for (let y = a; a in b; y = a in b) {} }; ; ) {}",
    ] {
        let ast = parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
        let declaration = if source.contains("const y") {
            "const y"
        } else {
            "let y"
        };
        let declaration_list = ast
            .walk()
            .map(|id| ast.node(id))
            .find(|node| {
                node.kind == "VariableDeclarationList"
                    && source[node.span.start..node.span.end]
                        .trim_start()
                        .starts_with(declaration)
            })
            .unwrap();
        assert_eq!(
            declaration_list.flags,
            if declaration == "let y" { 8193 } else { 8194 },
            "{source}"
        );
        for expression in ast
            .walk()
            .map(|id| ast.node(id))
            .filter(|node| node.kind == "BinaryExpression")
        {
            assert_eq!(expression.flags, 0, "{source}");
        }
    }
}

#[test]
fn committed_syntax_reports_the_error_in_its_remainder() {
    for (source, error_token) in [
        ("const f = (x) => { const y = ; };", ";"),
        ("const f = x => { const y = ; };", ";"),
        ("type T = (x: number) => ;", ";"),
        ("type T = {[K in keyof T]: };", "}"),
    ] {
        let error = parse(source).unwrap_err();
        assert_eq!(
            error.span.start,
            source.find(error_token).unwrap(),
            "{source}: {error}"
        );
    }
}

#[test]
fn committed_syntax_preserves_depth_diagnostics() {
    for source in [
        format!(
            "const f = (x) => {{{}{}}};",
            "{".repeat(100),
            "}".repeat(100)
        ),
        format!("type T = {}T;", "() => ".repeat(100)),
        format!(
            "type T = {}T{};",
            "{[K in keyof T]: ".repeat(100),
            "}".repeat(100)
        ),
    ] {
        let error = parse_with_options(
            &source,
            ParseOptions {
                max_depth: 32,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            error.message, "Maximum syntax nesting depth exceeded",
            "{source}"
        );
        assert!(error.span.start > source.find("=>").unwrap_or(0));
    }
}

#[test]
fn speculative_depth_exhaustion_is_reported_instead_of_a_fallback() {
    // Past these depths, parsing the type arguments exceeds the default budget.
    // The comparison reading `f < (...) > (x)` still fits within it, but must
    // not replace the generic call.
    let parentheses = |depth| format!("f<{}B{}>(x);", "(".repeat(depth), ")".repeat(depth));
    let generics = |depth| format!("f<{}B{}(x);", "A<".repeat(depth), ">".repeat(depth + 1));
    for source in [parentheses(40), generics(34)] {
        let ast = parse(&source).unwrap_or_else(|error| panic!("{source}: {error}"));
        let Some(Value::Nodes(statements)) = ast.node(ast.root_id()).field("statements") else {
            panic!("{source}: no statements");
        };
        let Some(Value::Node(expression)) = ast.node(statements[0]).field("expression") else {
            panic!("{source}: no expression");
        };
        let expression = ast.node(*expression);
        assert_eq!(expression.kind, "CallExpression", "{source}");
        assert!(expression.field("typeArguments").is_some(), "{source}");
    }
    for source in [
        parentheses(41),
        parentheses(45),
        parentheses(50),
        generics(35),
        generics(40),
    ] {
        let Err(error) = parse(&source) else {
            panic!("{source}: accepted another interpretation");
        };
        assert_eq!(
            error.message, "Maximum syntax nesting depth exceeded",
            "{source}"
        );
    }
}

fn first_expression<'a, 's>(
    ast: &'a typescript_parser::Ast<'s>,
) -> &'a typescript_parser::Node<'s> {
    let Some(Value::Nodes(statements)) = ast.root().field("statements") else {
        panic!("no statements");
    };
    let Some(Value::Node(expression)) = ast.node(statements[0]).field("expression") else {
        panic!("no expression statement");
    };
    ast.node(*expression)
}

#[test]
fn flat_comparison_lists_stay_within_the_depth_budget() {
    // Each `<` could open type arguments, but none of these lists has a `>`
    // that could close one. Reading every later item as a nested type argument
    // once rejected 36-item lists although the final trees are shallow.
    let list = |count: usize, item: &dyn Fn(usize) -> String| {
        (0..count).map(item).collect::<Vec<_>>().join(", ")
    };
    for count in [35, 36, 400] {
        for (source, kind, items) in [
            (
                format!("f({});", list(count, &|i| format!("x < {i}"))),
                "CallExpression",
                "arguments",
            ),
            (
                format!("[{}];", list(count, &|i| format!("a{i} < b{i}"))),
                "ArrayLiteralExpression",
                "elements",
            ),
            (
                format!("f({}, y > 1);", list(count, &|i| format!("x < {i}"))),
                "CallExpression",
                "arguments",
            ),
            (
                format!("f({});", list(count, &|i| format!("x < {i}, y > {i}"))),
                "CallExpression",
                "arguments",
            ),
        ] {
            let ast = parse(&source).unwrap_or_else(|error| panic!("{count}: {error}"));
            let expression = first_expression(&ast);
            assert_eq!(expression.kind, kind, "{count}");
            let Some(Value::Nodes(elements)) = expression.field(items) else {
                panic!("{count}: no {items}");
            };
            assert!(elements.len() >= count, "{count}: {}", elements.len());
            for &element in elements {
                assert_eq!(ast.node(element).kind, "BinaryExpression", "{count}");
            }
        }
        let chain = format!(
            "x = {};",
            (0..count)
                .map(|i| format!("a{i}"))
                .collect::<Vec<_>>()
                .join(" < ")
        );
        parse(&chain).unwrap_or_else(|error| panic!("chain {count}: {error}"));
    }

    // One forward scan resolves every `<` in a list, so work grows linearly.
    let source = format!("f({});", list(40_000, &|i| format!("x < {i}, y > {i}")));
    let start = std::time::Instant::now();
    parse(&source).unwrap();
    assert!(
        start.elapsed() < std::time::Duration::from_secs(10),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn expression_type_arguments_close_at_a_lone_greater_than() {
    // TypeScript rescans the closing `>`: joined to `=` or `>`, it is an
    // operator, and a following `>` of any length cannot follow a list.
    let ast = parse("f(a < b, c >= d);").unwrap();
    let Some(Value::Nodes(arguments)) = first_expression(&ast).field("arguments") else {
        panic!("no arguments");
    };
    assert_eq!(arguments.len(), 2);
    let ast = parse("a < b >= c;").unwrap();
    let Some(Value::Node(left)) = first_expression(&ast).field("left") else {
        panic!("no left operand");
    };
    assert_eq!(ast.text(*left).trim(), "a < b");
    let ast = parse("a < b >> c;").unwrap();
    assert!(
        ast.walk()
            .all(|id| ast.node(id).field("typeArguments").is_none())
    );
    assert!(parse("x = f<T> >= y;").is_err());

    // Lists that do close still parse as type arguments, including ones whose
    // members contain semicolons, template literal types, or joined `>>`.
    for source in [
        "f<{a: string; b: number}>();",
        "f<`a${B}c${D}e`>();",
        "f<A<B<C>>>();",
        "f<<T>(x: T) => T>();",
        "f<(a: Array<string>) => void>();",
        "f<{ [K in keyof T]: T[K] }>();",
        "a < b > (c);",
    ] {
        let ast = parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
        let expression = first_expression(&ast);
        assert_eq!(expression.kind, "CallExpression", "{source}");
        assert!(expression.field("typeArguments").is_some(), "{source}");
    }
}

#[test]
fn conditional_arrows_keep_their_colon_interpretations() {
    for source in [
        "a ? (b) : c => (d) : e => f;",
        "a ? b => (c) : d => e;",
        "const f = test ? (x) : (y) => y;",
        "const f = test ? (x): T => x : other;",
        "const f = test ? (): T => x : other;",
        "const f = test ? (x: T): T => x : other;",
        "const f = test ? <T>(x): T => x : other;",
    ] {
        parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }

    let source = "const f = test ? (x) : (y) => { const v = ; };";
    let error = parse(source).unwrap_err();
    assert_eq!(error.span.start, source.find(';').unwrap());
}

#[test]
fn speculative_interpretations_count_against_the_depth_budget() {
    // Speculating that the true arm has an arrow return type parses the false
    // arm as a function type, which takes more grammar frames than the actual
    // arrow expression. Exhausting the budget there ends the parse rather than
    // falling back; a budget of 30 suffices for the final interpretation alone.
    let source = "const f = test ? (x) : (y) => ((((z))));";
    let parse_within = |max_depth| {
        parse_with_options(
            source,
            ParseOptions {
                max_depth,
                ..Default::default()
            },
        )
    };
    for max_depth in [30, 41] {
        let Err(error) = parse_within(max_depth) else {
            panic!("{max_depth}: accepted within the budget");
        };
        assert_eq!(error.message, "Maximum syntax nesting depth exceeded");
    }
    let ast = parse_within(42).unwrap_or_else(|error| panic!("{source}: {error}"));
    let conditional = ast
        .walk()
        .map(|id| ast.node(id))
        .find(|node| node.kind == "ConditionalExpression")
        .unwrap();
    let branch = |key| match conditional.field(key) {
        Some(Value::Node(id)) => ast.node(*id).kind,
        _ => panic!("Expected node field {key}"),
    };
    assert_eq!(branch("whenTrue"), "ParenthesizedExpression");
    assert_eq!(branch("whenFalse"), "ArrowFunction");
}

#[test]
fn jsx_tags_end_at_a_single_greater_than_before_equals_text() {
    for source in [
        "const a = <p><b>x</b>=<i>y</i></p>;",
        "const b = <p>=</p>;",
        "const c = <p><br/>=</p>;",
        "const d = <>=</>;",
    ] {
        let ast = parse_with_options(
            source,
            ParseOptions {
                jsx: true,
                ..Default::default()
            },
        )
        .unwrap_or_else(|error| panic!("{source}: {error}"));
        let text = ast
            .walk()
            .map(|id| ast.node(id))
            .find(|node| node.kind == "JsxText" && matches!(node.field("text"), Some(typescript_parser::Value::Text(text)) if text == "="));
        assert!(text.is_some(), "{source}: no `=` JSX text");
    }
}

#[test]
fn deprecated_jsdoc_is_collected_where_typescript_collects_comments() {
    const DEPRECATED: u32 = 536_870_912;
    let deprecated = |source: &str| {
        let ast = parse(source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
        let function = ast
            .walk()
            .map(|id| ast.node(id))
            .find(|node| node.kind == "FunctionDeclaration")
            .unwrap();
        function.flags & DEPRECATED != 0
    };
    // At the start of a file, collection begins after a `#!` line, which a
    // byte order mark may precede.
    for prefix in ["", "\u{feff}"] {
        let source =
            format!("{prefix}#!/usr/bin/env node\n/** @deprecated */\nexport function f() {{}}");
        assert!(deprecated(&source), "{prefix:?}");
    }
    // Elsewhere TypeScript's `iterateCommentRanges` starts collecting after a
    // CR or LF only, not after U+2028 or U+2029.
    for (separator, expected) in [
        ("\n", true),
        ("\r", true),
        ("\r\n", true),
        ("\u{2028}", false),
        ("\u{2029}", false),
        (" ", false),
    ] {
        let source = format!("let a = 1;{separator}/** @deprecated */ function f() {{}}");
        assert_eq!(deprecated(&source), expected, "{separator:?}");
    }
}

#[test]
fn only_typescripts_module_indicators_reparse_top_level_await() {
    // In a script, `await (x)` calls a function named `await`; in a module it
    // awaits. `import x = N.y` and `export as namespace` leave a file a
    // script, as in TypeScript's `isAnExternalModuleIndicatorNode`.
    for (header, module) in [
        (
            "namespace N { export const y = 1; }\nimport x = N.y;",
            false,
        ),
        ("export as namespace X;", false),
        ("declare module \"m\" {}", false),
        ("import x = require(\"m\");", true),
        ("export const a = 1;", true),
        ("import.meta;", true),
    ] {
        let source = format!("{header}\nawait (x);");
        let ast = parse(&source).unwrap_or_else(|error| panic!("{header}: {error}"));
        let Some(Value::Nodes(statements)) = ast.root().field("statements") else {
            panic!("{header}: no statements");
        };
        let Some(Value::Node(expression)) =
            ast.node(*statements.last().unwrap()).field("expression")
        else {
            panic!("{header}: no expression");
        };
        let expected = if module {
            "AwaitExpression"
        } else {
            "CallExpression"
        };
        assert_eq!(ast.node(*expression).kind, expected, "{header}");
    }
}

#[test]
fn type_lookaheads_skip_every_modifier_typescript_skips() {
    // TypeScript's `isModifierKind` includes `export` and `default`, so a
    // parenthesized parameter list with one is a function type; its checker,
    // not its parser, rejects the modifier (TS1090).
    for source in [
        "type F = (export x: number) => void;",
        "type F = (public x: number) => void;",
        "type G = { [readonly x: string]: number };",
    ] {
        let ast = parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(
            ast.walk()
                .any(|id| matches!(ast.node(id).kind, "FunctionType" | "IndexSignature")),
            "{source}"
        );
    }
}

/// Native `nextIsStartOfMappedType` decides a braced type by its tokens:
/// `{` then an optional `readonly`, `+readonly`, or `-readonly`, `[`, an
/// identifier (not a reserved word, nor `yield` or `await` where they are
/// keywords), and `in`. Anything else is a type literal, such as an index
/// signature or a computed member.
#[test]
fn a_mapped_type_starts_with_its_key_and_in() {
    for (written, kind) in [
        ("{ [K in \"a\"]: number }", "MappedType"),
        ("{ readonly [K in \"a\"]: number }", "MappedType"),
        ("{ +readonly [K in \"a\"]: number }", "MappedType"),
        ("{ -readonly [K in \"a\"]: number }", "MappedType"),
        ("{ [K in \"a\" as `x${K}`]?: number }", "MappedType"),
        ("{ [k: string]: number }", "TypeLiteral"),
        ("{ readonly [k: string]: number }", "TypeLiteral"),
        ("{ [k]: number }", "TypeLiteral"),
        ("{ readonly x: number }", "TypeLiteral"),
        ("{ k: number }", "TypeLiteral"),
    ] {
        let source = format!("declare const k: unique symbol;\ntype T = {written};\n");
        let ast = parse(&source).unwrap_or_else(|error| panic!("{written}: {error}"));
        let alias = ast
            .walk()
            .map(|id| ast.node(id))
            .find(|node| node.kind == "TypeAliasDeclaration")
            .unwrap();
        let Some(Value::Node(ty)) = alias.field("type") else {
            panic!("{written}: alias has no type");
        };
        assert_eq!(ast.node(*ty).kind, kind, "{written}");
    }
}
