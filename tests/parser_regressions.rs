use typescript_parser::{ParseOptions, parse, parse_with_options};

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
            .nodes()
            .iter()
            .find(|node| {
                node.kind == "VariableDeclarationList"
                    && node.span.start > source.find('{').unwrap()
            })
            .unwrap();
        // The compiler constructs the list after restoring the inherited
        // DisallowIn flag, while its ordinary declarations permit `in`.
        assert_eq!(declaration_list.flags, 8194, "{source}");
        let initializer = ast
            .nodes()
            .iter()
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
            .nodes()
            .iter()
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
            .nodes()
            .iter()
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
fn uncommitted_type_interpretations_can_exhaust_depth_before_expression_fallback() {
    // Speculating that the true arm has an arrow return type parses the false
    // arm as a function type. That interpretation uses more grammar frames
    // than the actual arrow expression and must remain eligible for rollback.
    let source = "const f = test ? (x) : (y) => ((((z))));";
    parse_with_options(
        source,
        ParseOptions {
            max_depth: 40,
            ..Default::default()
        },
    )
    .unwrap();
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
            .nodes()
            .iter()
            .find(|node| node.kind == "JsxText" && matches!(node.field("text"), Some(typescript_parser::Value::Text(text)) if text == "="));
        assert!(text.is_some(), "{source}: no `=` JSX text");
    }
}
