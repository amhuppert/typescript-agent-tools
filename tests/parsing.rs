use typescript_parser::{ParseOptions, Value, parse, parse_with_options};

fn json(source: &str) -> serde_json::Value {
    serde_json::to_value(parse(source).unwrap()).unwrap()
}

#[test]
fn function_contexts_follow_compiler_parser_boundaries() {
    let value = json(
        "export function accepts(@decorator value: unknown): value is string { return true; }",
    );
    let function = &value["statements"][0];
    assert_eq!(function["parameters"][0]["flags"], 0);
    assert_eq!(
        function["parameters"][0]["modifiers"][0]["expression"]["flags"],
        98304
    );
    assert_eq!(function["type"]["flags"], 65536);
    assert_eq!(function["type"]["type"]["flags"], 0);
    assert_eq!(function["body"]["flags"], 0);

    let value = json("const fn = async function await() {};");
    assert_eq!(
        value["statements"][0]["declarationList"]["declarations"][0]["initializer"]["name"]["flags"],
        65536
    );
}

#[test]
fn class_member_contexts_are_isolated() {
    let value = json(
        "export class C { value = helper(); static { work(); } constructor(@decorator value: string) {} }",
    );
    let members = &value["statements"][0]["members"];
    assert_eq!(members[0]["initializer"]["flags"], 0);
    assert_eq!(members[1]["body"]["flags"], 65536);
    assert_eq!(
        members[2]["parameters"][0]["modifiers"][0]["expression"]["flags"],
        98304
    );
    assert_eq!(members[2]["body"]["flags"], 0);

    let value = json("class C { declare value: string; other = 1; }");
    assert_eq!(
        value["statements"][0]["members"][0]["name"]["flags"],
        33554432
    );
    assert_eq!(value["statements"][0]["members"][1]["flags"], 0);
}

#[test]
fn nested_grammar_scopes_preserve_the_enclosing_async_context() {
    let value = json(
        r#"export async function outer() {
            type Result<T> = T extends infer U extends string ? U : never;
            const object = {
                async run() { return await task; },
                plain() { return await; },
                value: await task
            };
            const comparison = (left < Type) + (right >> count);
            return await task;
        }"#,
    );
    let statements = &value["statements"][0]["body"]["statements"];
    assert_eq!(statements[0]["type"]["flags"], 0);
    let properties =
        &statements[1]["declarationList"]["declarations"][0]["initializer"]["properties"];
    assert_eq!(properties[0]["body"]["flags"], 65536);
    assert_eq!(properties[1]["body"]["flags"], 0);
    assert_eq!(
        properties[1]["body"]["statements"][0]["expression"]["escapedText"],
        "await"
    );
    assert_eq!(properties[2]["initializer"]["flags"], 65536);
    assert_eq!(
        statements[2]["declarationList"]["declarations"][0]["initializer"]["flags"],
        65536
    );
    assert_eq!(statements[3]["expression"]["kind"], 224);
    assert_eq!(statements[3]["expression"]["flags"], 65536);
}

#[test]
fn for_initializer_restrictions_survive_nested_and_ambiguous_expressions() {
    let value = json(
        r#"for (let value = probe((left < Type), `${left in right}`); value; value--) {}
        after();"#,
    );
    let statement = &value["statements"][0];
    let initializer = &statement["initializer"]["declarations"][0]["initializer"];
    assert_eq!(initializer["flags"], 8192);
    let comparison = &initializer["arguments"][0]["expression"];
    assert_eq!(comparison["operatorToken"]["kind"], 30);
    assert_eq!(comparison["flags"], 0);
    let substitution = &initializer["arguments"][1]["templateSpans"][0]["expression"];
    assert_eq!(substitution["operatorToken"]["kind"], 103);
    assert_eq!(substitution["flags"], 0);
    assert_eq!(statement["condition"]["flags"], 0);
    assert_eq!(value["statements"][1]["expression"]["flags"], 0);
}

#[test]
fn contextual_syntax_errors_are_rejected() {
    for source in [
        "async function f(value = await => await) {}",
        "async function f() { ++await value; }",
        "function* f() { --yield value; }",
        "import { value as 'local' } from 'pkg';",
        "import { type 'exported' } from 'pkg';",
        "type T = C[#private];",
        "class C { public @decorator value: string; }",
        "const value = { get *method() {} };",
    ] {
        assert!(parse(source).is_err(), "incorrectly accepted {source:?}");
    }
}

#[test]
fn compiler_style_json() {
    let value = json("const value: number = 42;");
    assert_eq!(value["kind"], 308);
    let statement = &value["statements"][0];
    assert_eq!(statement["kind"], 244);
    assert_eq!(statement["declarationList"]["flags"], 2);
    let declaration = &statement["declarationList"]["declarations"][0];
    assert_eq!(declaration["name"]["escapedText"], "value");
    assert_eq!(declaration["type"]["kind"], 150);
    assert_eq!(declaration["initializer"]["text"], "42");
}

#[test]
fn operators_have_correct_precedence_and_associativity() {
    let value = json("a = b = 1 + 2 * 3 ** 4 ** 5;");
    let expression = &value["statements"][0]["expression"];
    assert_eq!(expression["operatorToken"]["kind"], 64);
    assert_eq!(expression["right"]["operatorToken"]["kind"], 64);
    let sum = &expression["right"]["right"];
    assert_eq!(sum["operatorToken"]["kind"], 40);
    assert_eq!(sum["right"]["operatorToken"]["kind"], 42);
    assert_eq!(sum["right"]["right"]["right"]["operatorToken"]["kind"], 43);
}

#[test]
fn complete_input_is_required_and_trivia_is_skipped() {
    parse("\u{FEFF}#!/usr/bin/env node\r\n/* comment */ const x = 1 // line\n").unwrap();
    for source in [
        "const x = ;",
        "let a = 1 garbage",
        "/* unterminated",
        "const s = 'unfinished",
        "type X = [number;",
        "if (true) {",
    ] {
        assert!(parse(source).is_err(), "incorrectly accepted {source:?}");
    }
}

#[test]
fn arrow_functions_and_type_arguments_do_not_consume_relational_operators() {
    for source in [
        "const f = <const T extends string>(x: T): T => x;",
        "const f = async (x: number) => await Promise.resolve(x);",
        "const f = (x: unknown): x is number => typeof x === 'number';",
        "a < b && b > c; foo<Map<string, Array<number>>>(); a >>= 2;",
        "const x = fn<string>; const y = new Box<Map<string, number>>(x);",
        "const x = <number> value; const y = x as const;",
    ] {
        parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

#[test]
fn modern_type_syntax_is_structural() {
    let source = r#"
type Getters<T> = { -readonly [K in keyof T as `get${Capitalize<string & K>}`]+?: T[K] };
type Flatten<T> = T extends readonly (infer U)[] ? U : never;
type Tuple = readonly [first: number, second?: string, ...rest: boolean[]];
type Callable = { new <T>(x: T): Box<T>; <T>(x: T): T; readonly [key: string]: unknown };
type Constructor = abstract new <T>(x: T) => Box<T>;
type External = typeof import("pkg", { with: { type: "json" } }).default;
"#;
    let ast = parse(source).unwrap();
    for kind in [
        "MappedType",
        "ConditionalType",
        "InferType",
        "TemplateLiteralType",
        "IndexedAccessType",
        "NamedTupleMember",
        "ConstructorType",
        "ImportType",
    ] {
        assert!(
            ast.walk()
                .map(|id| ast.node(id))
                .any(|node| node.kind == kind),
            "missing {kind}"
        );
    }
}

#[test]
fn declarations_and_control_flow() {
    let source = r#"
import type { Foo as Bar, type Baz } from "pkg";
import data from "data" with { type: "json" };
import legacy = require("legacy");
export type { Bar }; export * as ns from "pkg"; export as namespace Library;
declare module "pkg" { export interface Thing { readonly name: string; get value(): number; } }
declare global { interface Window { custom: boolean } }
namespace A.B.C { export const enum E { A, B = 1 << 2 } }
@sealed export abstract class C<T> extends factory<T>() implements Bar {
  static { this.count = 0; }
  #value!: T; public accessor data?: T;
  constructor(public readonly id: string) {}
  get value(): T { return this.#value; }
  set value(x: T) { this.#value = x; }
  async *items(): AsyncGenerator<T> { yield this.#value; }
}
async function run() {
  await using resource = acquire();
  for await (const x of xs) { if (x) continue; else break; }
  for (let i = 0; i < 4; i++) { do { i--; } while (i > 3); }
  for (const key in object) { switch (key) { case "a": break; default: debugger; } }
  try { throw new Error("test"); } catch ({ message }: any) { console.log(message); } finally { cleanup(); }
}
"#;
    parse(source).unwrap();
}

#[test]
fn regex_templates_and_binding_patterns() {
    let source = r#"
const {a: renamed = 2, nested: { b }, ...other} = value;
const [first, , ...rest] = values;
const x = /[a-z\/]\w+/giu.test("path") ? a / b / c : tag`head ${ {a: `nested ${b}`} } tail`;
const object = { renamed, a: 1, [key]: 2, ...other, async *method<T>(x: T) { yield x; }, get length() { return 0; } };
"#;
    parse(source).unwrap();
}

#[test]
fn jsx_has_a_separate_text_and_attribute_scanner() {
    let source = "const view = <><UI.Button<string> data-id=\"line\nnext\" {...props}>hello // text {value}<x:tag attr='\\' /></UI.Button></>;";
    let ast = parse_with_options(
        source,
        ParseOptions {
            jsx: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(ast.walk().map(|id| ast.node(id)).any(|node| node.kind == "JsxText"
        && matches!(node.field("text"), Some(Value::Text(text)) if text.contains("// text"))));
    assert!(
        parse_with_options(
            "const x = <A></B>;",
            ParseOptions {
                jsx: true,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn byte_spans_serialize_as_utf16_compiler_positions() {
    let source = "const café = '😀'; café;";
    let value = json(source);
    assert_eq!(value["end"], source.encode_utf16().count());
    assert_eq!(value["statements"][1]["expression"]["pos"], 18);
    assert_eq!(value["statements"][1]["expression"]["end"], 23);
}

#[test]
fn nesting_is_bounded_without_panicking() {
    let source = format!("{}1{};", "(".repeat(1_000), ")".repeat(1_000));
    let error = parse_with_options(
        &source,
        ParseOptions {
            max_depth: 32,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(error.message.contains("nesting"));
}

#[test]
fn asi_respects_return_throw_and_postfix_line_breaks() {
    let value = json("function f() { return\n1 } let x = 1\nx\n++x;");
    assert!(
        value["statements"][0]["body"]["statements"][0]
            .get("expression")
            .is_none()
    );
    assert!(parse("throw\nnew Error();").is_err());
}

#[test]
fn iterative_json_matches_serde_and_handles_long_chains() {
    let ast = parse("const x = {a: ['hello', true], b: 1 + 2};").unwrap();
    for pretty in [false, true] {
        let mut output = Vec::new();
        ast.write_json(&mut output, pretty).unwrap();
        let expected = if pretty {
            serde_json::to_vec_pretty(&ast).unwrap()
        } else {
            serde_json::to_vec(&ast).unwrap()
        };
        assert_eq!(output, expected);
    }
    let source = format!("{}1;", "1+".repeat(10_000));
    let ast = parse(&source).unwrap();
    let mut output = Vec::new();
    ast.write_json(&mut output, false).unwrap();
    assert!(output.len() > source.len());
    assert_eq!(
        output.iter().filter(|c| **c == b'{').count(),
        ast.nodes().len()
    );
}

#[test]
fn javascript_strings_preserve_surrogate_code_units() {
    let ast = parse(r#"const x = "\uD83D\uDE00", y = "\uD800";"#).unwrap();
    assert!(
        ast.walk()
            .map(|id| ast.node(id))
            .any(|node| matches!(node.field("text"), Some(Value::Text(text)) if text == "😀"))
    );
    let mut output = Vec::new();
    ast.write_json(&mut output, false).unwrap();
    assert!(String::from_utf8(output).unwrap().contains("\\ud800"));
    for source in [r#"const x = "\xQQ";"#, r#"const x = "\u{110000}";"#] {
        assert!(parse(source).is_err());
    }
}

#[test]
fn contextual_keywords_and_tagged_template_escapes() {
    for source in [
        "async x => x;",
        "export default async () => 1;",
        "const await = f; await(x);",
        "const yield = 1; yield + 1;",
        r"tag`\u{}`;",
        r"tag`\xGG ${value} \u{110000}`;",
    ] {
        parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
    for source in [
        r"`\u{}`;",
        r#"const s = "\123";"#,
        "const x = 0_1;",
        "const x = 012;",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
    assert!(
        parse_with_options(
            "const f = <T>(x: T) => x;",
            ParseOptions {
                jsx: true,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn module_await_reparses_ambiguous_expressions() {
    let value = json("export {}; await(foo); const f = () => await(foo); await /pattern/;");
    assert_eq!(value["statements"][1]["expression"]["kind"], 224);
    assert_eq!(value["statements"][1]["flags"], 65536);
    assert_eq!(
        value["statements"][2]["declarationList"]["declarations"][0]["initializer"]["body"]["kind"],
        214
    );
    assert_eq!(
        value["statements"][3]["expression"]["expression"]["kind"],
        14
    );
    let value = json("const x = obj.await; const y = {await: 1}; export {};");
    assert_eq!(value["statements"][0]["flags"], 0);
    assert_eq!(value["statements"][1]["flags"], 0);
    let value = json("await\nx; export {};");
    assert_eq!(value["statements"].as_array().unwrap().len(), 2);
    assert_eq!(value["statements"][0]["expression"]["kind"], 224);
}

#[test]
fn tuple_elements_and_contextual_names_are_unambiguous() {
    let value = json(
        "type X = [T, U?, name?: V]; const f = (async) => async; import type from from 'pkg'; const call = fn\n<T>(x);",
    );
    assert_eq!(value["statements"][0]["type"]["elements"][1]["kind"], 191);
    assert_eq!(value["statements"][0]["type"]["elements"][2]["kind"], 203);
    assert_eq!(value["statements"][2]["importClause"]["isTypeOnly"], true);
    assert_eq!(
        value["statements"][2]["importClause"]["name"]["escapedText"],
        "from"
    );
    assert_eq!(
        value["statements"][3]["declarationList"]["declarations"][0]["initializer"]["kind"],
        214
    );
    for source in [
        "type X = number | () => void;",
        "x + y = z;",
        "const x = new C<T>.member();",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
}

#[test]
fn declaration_files_and_all_line_breaks_are_recognized() {
    for name in ["source.d.ts", "source.d.mts", "source.d.cts"] {
        let ast = parse_with_options(
            "/** @deprecated */ interface Old {}",
            ParseOptions {
                file_name: name.into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(ast.root().flags, 33554432);
        assert!(
            ast.walk()
                .map(|id| ast.node(id))
                .any(|node| node.kind == "InterfaceDeclaration" && node.flags & 536870912 != 0)
        );
    }
    for separator in ["\r", "\n", "\r\n", "\u{2028}", "\u{2029}"] {
        let source = format!("const x = '😀';{separator}const y = ;");
        let error = parse(&source).unwrap_err();
        assert_eq!(error.line_column(&source), (2, 11));
    }
}

#[test]
fn bindings_reject_reserved_words_but_properties_allow_them() {
    for source in [
        "const for = 1;",
        "function return() {}",
        "type for = T;",
        "const {default} = value;",
        "const {\"x\"} = value;",
        "const x = {default};",
        r"const \u0066or = 1;",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
    for source in [
        "const x = {default: 1};",
        "const {default: x} = value;",
        "const interface = 1;",
        r"const \u0061sync = 1;",
        "#field in object;",
    ] {
        parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

#[test]
fn adjacent_type_argument_and_generic_function_openers_are_rescanned() {
    let value = json("const x = fn<<T>(x: T) => T>(); type X = Wrapper<<T>() => T>;");
    let call = &value["statements"][0]["declarationList"]["declarations"][0]["initializer"];
    assert_eq!(call["kind"], 214);
    assert_eq!(call["typeArguments"][0]["kind"], 185);
    assert_eq!(
        value["statements"][1]["type"]["typeArguments"][0]["kind"],
        185
    );
    parse("x << y; const value = f<T>();").unwrap();
}

#[test]
fn nested_class_heritage_obeys_the_depth_budget() {
    let source = format!(
        "class C extends {}class {{}}{}",
        "class extends ".repeat(31),
        " {}".repeat(32)
    );
    let error = parse_with_options(
        &source,
        ParseOptions {
            max_depth: 4,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.message, "Maximum syntax nesting depth exceeded");
    parse_with_options(
        &source,
        ParseOptions {
            max_depth: 256,
            ..Default::default()
        },
    )
    .unwrap();
    // Integration tests run on Rust's ordinary worker-thread stack, which is
    // smaller than the CLI main stack. The default budget must protect both.
    let source = format!(
        "class C extends {}class {{}}{}",
        "class extends ".repeat(9_999),
        " {}".repeat(10_000)
    );
    let error = parse(&source).unwrap_err();
    assert_eq!(error.message, "Maximum syntax nesting depth exceeded");
}

#[test]
fn nested_assignments_remain_expressions_during_arrow_speculation() {
    // Before failed probes were remembered, each nesting level parsed its
    // initializer twice; even 24 levels could time out in a release build.
    let source = format!("const v = {}x{};", "(x = ".repeat(32), ")".repeat(32));
    let ast = parse(&source).unwrap();
    let child = |id, key| match ast.node(id).field(key).unwrap() {
        Value::Node(id) => *id,
        _ => panic!("Expected node field {key}"),
    };
    let first = |id, key| match ast.node(id).field(key).unwrap() {
        Value::Nodes(ids) => ids[0],
        _ => panic!("Expected node list {key}"),
    };
    let statement = first(ast.root_id(), "statements");
    let list = child(statement, "declarationList");
    let declaration = first(list, "declarations");
    let mut expression = child(declaration, "initializer");
    for _ in 0..32 {
        assert_eq!(ast.node(expression).kind, "ParenthesizedExpression");
        let assignment = child(expression, "expression");
        assert_eq!(ast.node(assignment).kind, "BinaryExpression");
        expression = child(assignment, "right");
    }
    assert!(
        matches!(ast.node(expression).field("escapedText"), Some(Value::Text(text)) if text == "x")
    );
    parse("const f = (x = (y: T) => y) => x; export {}; (x = await(foo));").unwrap();
}

#[test]
fn deep_valid_types_fit_the_default_budget() {
    let source = format!("type T = {}string{};", "(".repeat(40), ")".repeat(40));
    let ast = parse(&source).unwrap();
    assert_eq!(
        ast.walk()
            .map(|id| ast.node(id))
            .filter(|node| node.kind == "ParenthesizedType")
            .count(),
        40
    );

    let arms = (0..100)
        .map(|value| format!("T extends {value} ? '{value}' : "))
        .collect::<String>();
    let source = format!("type Foo<T> = {arms}never;");
    let ast = parse(&source).unwrap();
    assert_eq!(
        ast.walk()
            .map(|id| ast.node(id))
            .filter(|node| node.kind == "ConditionalType")
            .count(),
        100
    );
}

/// Inputs nested 4,096 levels deep along every recursive grammar path, each
/// with whether it needs JSX. Each exceeds both depth budgets.
fn deep_inputs() -> Vec<(&'static str, String, bool)> {
    vec![
        (
            "arrows",
            format!("const f = {}x;", "(x) => ".repeat(1_000)),
            false,
        ),
        (
            "simple arrows",
            format!("const f = {}x;", "x => ".repeat(1_000)),
            false,
        ),
        (
            "arrays",
            format!("{}0{};", "[".repeat(4_096), "]".repeat(4_096)),
            false,
        ),
        ("unary", format!("{}x;", "!".repeat(4_096)), false),
        ("new", format!("{}C;", "new ".repeat(4_096)), false),
        (
            "calls",
            format!("{}x{};", "f(".repeat(4_096), ")".repeat(4_096)),
            false,
        ),
        (
            "types",
            format!(
                "type T = {}T{};",
                "Promise<".repeat(4_096),
                ">".repeat(4_096)
            ),
            false,
        ),
        (
            "parenthesized types",
            format!("type T = {}T{};", "(".repeat(4_096), ")".repeat(4_096)),
            false,
        ),
        (
            "conditional false types",
            format!("type T = {}never;", "T extends T ? T : ".repeat(4_096)),
            false,
        ),
        (
            "conditional true types",
            format!(
                "type T = {}T{};",
                "T extends T ? ".repeat(4_096),
                " : T".repeat(4_096)
            ),
            false,
        ),
        (
            "function types",
            format!("type T = {}T;", "() => ".repeat(4_096)),
            false,
        ),
        (
            "blocks",
            format!("{}{}", "{".repeat(4_096), "}".repeat(4_096)),
            false,
        ),
        (
            "jsx",
            format!("const x = {}{};", "<X>".repeat(4_096), "</X>".repeat(4_096)),
            true,
        ),
        (
            "templates",
            format!("{}x{};", "`a${".repeat(4_096), "}`".repeat(4_096)),
            false,
        ),
        (
            "tagged templates",
            format!("{}x{};", "tag`a${".repeat(4_096), "}`".repeat(4_096)),
            false,
        ),
        (
            "bindings",
            format!("const {}x{} = y;", "[".repeat(4_096), "]".repeat(4_096)),
            false,
        ),
        (
            "namespaces",
            format!("namespace A{} {{}}", ".A".repeat(4_096)),
            false,
        ),
        (
            "mapped types",
            format!(
                "type T = {}T{};",
                "{[K in keyof T]: ".repeat(4_096),
                "}".repeat(4_096)
            ),
            false,
        ),
        (
            "template types",
            format!("type T = {}T{};", "`a${".repeat(4_096), "}`".repeat(4_096)),
            false,
        ),
    ]
}

#[test]
fn default_budget_protects_rust_worker_thread_stacks() {
    for (name, source, jsx) in deep_inputs() {
        let error = parse_with_options(
            &source,
            ParseOptions {
                jsx,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            error.message, "Maximum syntax nesting depth exceeded",
            "{name}"
        );
    }
}

#[test]
fn the_deep_budget_fits_its_documented_stack() {
    // A parse that exhausts the budget has recursed as deeply as it can.
    std::thread::Builder::new()
        .stack_size(typescript_parser::DEEP_PARSE_STACK_BYTES)
        .spawn(|| {
            for (name, source, jsx) in deep_inputs() {
                let error = parse_with_options(
                    &source,
                    ParseOptions {
                        jsx,
                        max_depth: ParseOptions::DEEP_MAX_DEPTH,
                        ..Default::default()
                    },
                )
                .unwrap_err();
                assert_eq!(
                    error.message, "Maximum syntax nesting depth exceeded",
                    "{name}"
                );
            }
            // The deep budget admits what the default rejects.
            let source = format!("{}x{};", "f(".repeat(200), ")".repeat(200));
            assert!(parse(&source).is_err());
            parse_with_options(
                &source,
                ParseOptions {
                    max_depth: ParseOptions::DEEP_MAX_DEPTH,
                    ..Default::default()
                },
            )
            .unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn module_await_in_member_names_and_decorators_uses_the_enclosing_scope() {
    let value = json(
        "class C { [await(foo)]() { await(bar); } @decorator(await(baz)) method() {} value = await(qux); } export {};",
    );
    let class = &value["statements"][0];
    assert_eq!(class["flags"], 65536);
    assert_eq!(class["members"][0]["name"]["expression"]["kind"], 224);
    assert_eq!(
        class["members"][0]["body"]["statements"][0]["expression"]["kind"],
        214
    );
    assert_eq!(
        class["members"][1]["modifiers"][0]["expression"]["arguments"][0]["kind"],
        224
    );
    assert_eq!(class["members"][2]["initializer"]["kind"], 214);
    let value = json("const o = { [await(foo)]() {} }; export {};");
    assert_eq!(value["statements"][0]["flags"], 65536);
    assert_eq!(
        value["statements"][0]["declarationList"]["declarations"][0]["initializer"]["properties"]
            [0]["name"]["expression"]["kind"],
        224
    );
    let value = json("class C { await() {} value = await(foo); } export {};");
    assert_eq!(value["statements"][0]["flags"], 65536);
    assert_eq!(
        value["statements"][0]["members"][1]["initializer"]["kind"],
        214
    );
    let value = json("const o = { await() {}, await: 1 }; export {};");
    assert_eq!(value["statements"][0]["flags"], 0);
    assert!(parse("class C { @await(foo) method() {} } export {};").is_err());
}

#[test]
fn type_argument_recovery_commits_only_a_disambiguated_list() {
    for source in [
        "x << y; const f = (x: T): U => x;",
        "f < T > +x; const f = <T>(x: T) => x;",
        "f<<T>() => T>(); f?.<<T>() => T>();",
        "type T = Box<Box<X>>; f<Box<X>>();",
        "const f = async () => { f < T > x; await(y); };",
    ] {
        parse(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
    for source in [
        "f<<T>() T>();",
        "f<<T>(x T) => T>();",
        "f<<T>(x: ) => T>();",
        "f?.<<T>() T>();",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
    let error = parse("f<<T>() T>();").unwrap_err();
    assert_eq!(error.message, "Expected '=>'");
}

#[test]
fn walking_visits_each_node_of_the_final_tree_once() {
    // Module reparsing replaces the script reading of `await(foo)`, a call,
    // which stays allocated but is not part of the tree.
    let ast = parse("export {}; await(foo);").unwrap();
    assert!(ast.nodes().iter().any(|node| node.kind == "CallExpression"));
    let walked: Vec<_> = ast.walk().collect();
    assert_eq!(walked[0], ast.root_id());
    assert!(
        walked
            .iter()
            .all(|&id| ast.node(id).kind != "CallExpression")
    );
    assert!(
        walked
            .iter()
            .any(|&id| ast.node(id).kind == "AwaitExpression")
    );
    let mut unique = walked.clone();
    unique.sort_by_key(|id| id.index());
    unique.dedup();
    assert_eq!(unique.len(), walked.len());

    // `assertClause` repeats `attributes`; their shared child is one child.
    let ast = parse("import x from \"m\" with { type: \"json\" };").unwrap();
    let attributes: Vec<_> = ast
        .walk()
        .filter(|&id| ast.node(id).kind == "ImportAttributes")
        .collect();
    assert_eq!(attributes.len(), 1);
    let import = ast
        .walk()
        .find(|&id| ast.node(id).kind == "ImportDeclaration")
        .unwrap();
    assert_eq!(
        ast.children(import)
            .filter(|&id| id == attributes[0])
            .count(),
        1
    );
}

#[test]
fn token_spans_and_identifier_names_undo_the_json_projection() {
    let source = "/* c */ let __x = 1; const y = <a>// text</a>;";
    let ast = parse_with_options(
        source,
        ParseOptions {
            jsx: true,
            ..Default::default()
        },
    )
    .unwrap();
    let find = |kind: &str| ast.walk().find(|&id| ast.node(id).kind == kind).unwrap();
    let name = find("Identifier");
    assert_eq!(ast.text(name), " __x");
    let span = ast.token_span(name);
    assert_eq!(&source[span.start..span.end], "__x");
    assert!(
        matches!(ast.node(name).field("escapedText"), Some(Value::Text(text)) if text == "___x")
    );
    assert_eq!(ast.identifier_name(name), Some("__x"));
    let statement = find("VariableStatement");
    let span = ast.token_span(statement);
    assert_eq!(&source[span.start..span.end], "let __x = 1;");
    // JSX text has no trivia, so its `//` is text.
    let text = find("JsxText");
    let span = ast.token_span(text);
    assert_eq!(&source[span.start..span.end], "// text");
    assert_eq!(ast.identifier_name(text), None);
}

#[test]
fn parse_options_for_a_file_follow_typescripts_name_rules() {
    // JSX follows the extension without case, as TypeScript's
    // `getScriptKindFromFileName`; declaration context needs the exact name.
    for (name, jsx, declaration) in [
        ("view.tsx", true, false),
        ("VIEW.TSX", true, false),
        ("view.jsx", true, false),
        ("view.ts", false, false),
        ("dir.tsx/view.ts", false, false),
        ("types.d.ts", false, true),
        ("styles.d.css.ts", false, true),
        ("G.D.TS", false, false),
    ] {
        let options = ParseOptions::for_file(name);
        assert_eq!(options.jsx, jsx, "{name}");
        let source = if jsx {
            "const a = <div />;"
        } else {
            "declare const a: number;"
        };
        let ast =
            parse_with_options(source, options).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            matches!(
                ast.root().field("isDeclarationFile"),
                Some(Value::Bool(true))
            ),
            declaration,
            "{name}"
        );
    }
}
