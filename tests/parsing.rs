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
            ast.nodes.iter().any(|node| node.kind == kind),
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
    assert!(ast.nodes.iter().any(|node| node.kind == "JsxText"
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
        ast.nodes.len()
    );
}

#[test]
fn javascript_strings_preserve_surrogate_code_units() {
    let ast = parse(r#"const x = "\uD83D\uDE00", y = "\uD800";"#).unwrap();
    assert!(
        ast.nodes
            .iter()
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
            ast.nodes
                .iter()
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
