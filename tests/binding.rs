//! Lexical binding rules, checked by querying every marked token.
mod support;

use support::Fixture;

const CONFIG: &str = r#"{ "compilerOptions": { "noLib": true } }"#;

fn check(files: &[(&str, &str)]) {
    let mut all = vec![("tsconfig.json", CONFIG)];
    all.extend_from_slice(files);
    let fixture = Fixture::new(&all);
    fixture.check_markers(&fixture.load());
}

#[test]
fn block_and_function_scoped_variables() {
    check(&[(
        "scopes.ts",
        r#"export {};
var /*@v:d*/v = 1;
let /*@l:d*/l = 2;
{
  let /*@inner:d*/l = 3;
  var /*@v:d*/v = 4;
  /*@inner:u*/l; /*@v:u*/v;
}
/*@l:u*/l;
for (let /*@i:d*/i = 0; /*@i:u*/i < 1; /*@i:u*/i++) { /*@i:u*/i; }
for (const /*@k:d*/k of [/*@l:u*/l]) { /*@k:u*/k; }
switch (/*@v:u*/v) {
  case 1: let /*@s:d*/s = 1; /*@s:u*/s; break;
  default: /*@s:u*/s;
}
try {} catch (/*@e:d*/e) { /*@e:u*/e; }
class /*@C:d*/C { static { var /*@sv:d*/sv = 1; /*@sv:u*/sv; } }
function /*@f:d*/f() { using /*@r:d*/r = null; /*@r:u*/r; { var /*@hoisted:d*/hoisted = 1; } /*@hoisted:u*/hoisted; }
"#,
    )]);
}

#[test]
fn parameters_defaults_and_function_names() {
    check(&[(
        "functions.ts",
        r#"export {};
const /*@x:d*/x = 1;
function /*@f:d*/f(/*@a:d*/a = /*@x:u*/x, /*@b:d*/b = /*@a:u*/a) {
  const /*@x2:d*/x = 2;
  return /*@a:u*/a + /*@b:u*/b + /*@x2:u*/x;
}
const /*@g:d*/g = function /*@inner:d*/inner(/*@n:d*/n: number): number { return /*@inner:u*/inner(/*@n:u*/n); };
/*@g:u*/g(1); /*@f:u*/f();
const /*@arrow:d*/arrow = (/*@p:d*/p: number) => /*@p:u*/p * 2;
/*@arrow:u*/arrow(2);
function /*@h:d*/h(/*@q:d*/q: number) { var /*@q:d*/q; return /*@q:u*/q; }
/*@h:u*/h(3);
"#,
    )]);
}

#[test]
fn destructuring_shorthand_and_assignment_patterns() {
    check(&[(
        "patterns.ts",
        r#"export {};
const /*@obj:d*/obj = { /*@!*/a: 1, /*@!*/b: { /*@!*/c: 2 } };
const { /*@a:d*/a, /*@!*/b: { /*@!*/c: /*@renamed:d*/renamed }, .../*@rest:d*/rest } = /*@obj:u*/obj;
const [/*@first:d*/first, , /*@third:d*/third = /*@first:u*/first] = [1, 2, 3];
function /*@take:d*/take({ /*@pa:d*/pa, /*@!*/pb: /*@pc:d*/pc = /*@pa:u*/pa }: { /*@!*/pa: number; /*@!*/pb?: number }) { return /*@pc:u*/pc; }
let /*@m:d*/m = 0, /*@n:d*/n = 0;
({ /*@m:u*/m, /*@!*/k: /*@n:u*/n } = { /*@!*/m: 1, /*@!*/k: 2 });
[/*@m:u*/m, /*@n:u*/n] = [/*@n:u*/n, /*@m:u*/m];
const /*@short:d*/short = { /*@a:u*/a, /*@third:u*/third, /*@renamed:u*/renamed, /*@rest:u*/rest };
/*@take:u*/take(/*@short:u*/short as never);
"#,
    )]);
}

#[test]
fn type_and_value_meanings_stay_separate() {
    check(&[(
        "meanings.ts",
        r#"export {};
type /*@T:d*/Thing = { /*@!*/x: number };
const /*@V:d*/Thing = 1;
let /*@a:d*/a: /*@T:u*/Thing = { /*@!*/x: /*@V:u*/Thing };
interface /*@I:d*/Box {}
declare var /*@BV:d*/Box: { new (): /*@I:u*/Box };
const /*@b:d*/b: /*@I:u*/Box = new /*@BV:u*/Box();
let /*@q:d*/q: typeof /*@V:u*/Thing;
/*@a:u*/a; /*@b:u*/b; /*@q:u*/q;
"#,
    )]);
}

#[test]
fn type_parameters_mapped_types_and_infer_scopes() {
    check(&[(
        "generics.ts",
        r#"export {};
type /*@Id:d*/Id</*@T:d*/T> = /*@T:u*/T;
function /*@g:d*/g</*@U:d*/U>(/*@u:d*/u: /*@U:u*/U): /*@U:u*/U { let /*@x:d*/x: /*@U:u*/U = /*@u:u*/u; return /*@x:u*/x; }
type /*@M:d*/Mapped</*@O:d*/O> = { [/*@K:d*/K in keyof /*@O:u*/O]: /*@O:u*/O[/*@K:u*/K] };
type /*@E:d*/Elem</*@A:d*/A> = /*@A:u*/A extends (infer /*@R:d*/R)[] ? /*@R:u*/R : never;
type /*@R2:d*/R = string;
type /*@Ex:d*/Ex</*@B:d*/B> = /*@B:u*/B extends infer /*@R3:d*/R ? /*@R3:u*/R : /*@R2:u*/R;
let /*@use:d*/use: [/*@Id:u*/Id<1>, /*@M:u*/Mapped<{}>, /*@E:u*/Elem<[]>, /*@Ex:u*/Ex<1>, typeof /*@g:u*/g];
/*@use:u*/use;
"#,
    )]);
}

#[test]
fn classes_interfaces_and_members() {
    check(&[(
        "classes.ts",
        r#"export {};
const /*@helper:d*/helper = "key";
class /*@C:d*/C</*@T:d*/T> implements /*@I:u*/Shape {
  /*@!*/value: /*@T:u*/T | undefined;
  constructor(/*@p:d*/p: /*@T:u*/T) { this./*@!*/value = /*@p:u*/p; }
  /*@!*/area(/*@arg:d*/arg: number) { return /*@arg:u*/arg; }
  /*@!*/name = /*@helper:u*/helper;
  static /*@!*/create() { return new /*@C:u*/C<number>(1); }
  get /*@!*/size() { return 1; }
  [/*@helper:u*/helper]() {}
  /*@!*/#secret = 1;
}
interface /*@I:d*/Shape { /*@!*/area(/*@sa:d*/arg: number): number; }
interface /*@I:d*/Shape { /*@!*/name: string; }
const /*@c:d*/c = /*@C:u*/C./*@!*/create();
/*@c:u*/c./*@!*/area(1);
"#,
    )]);
}

#[test]
fn decorators_resolve_outside_the_decorated_member() {
    check(&[(
        "decorators.ts",
        r#"export {};
function /*@dec:d*/dec(.../*@args:d*/args: unknown[]) { /*@args:u*/args; }
@/*@dec:u*/dec class /*@K:d*/K { @/*@dec:u*/dec /*@!*/m(@/*@dec:u*/dec /*@x:d*/x: number) { /*@x:u*/x; } }
/*@K:u*/K;
"#,
    )]);
}

#[test]
fn enum_members_are_not_lexical_targets() {
    check(&[(
        "enums.ts",
        r#"export {};
const /*@Red:d*/Red = 0;
enum /*@E:d*/Color { /*@!*/Red, /*@!*/Green = /*@!*/Red + 1 }
const /*@c:d*/c = /*@E:u*/Color./*@!*/Red + /*@Red:u*/Red;
const enum /*@CE:d*/Flags { /*@!*/A = 1 }
/*@c:u*/c; /*@CE:u*/Flags./*@!*/A;
"#,
    )]);
}

#[test]
fn namespace_blocks_share_exports_but_not_locals() {
    check(&[(
        "namespaces.ts",
        r#"export {};
namespace /*@N:d*/N { export const /*@x:de*/x = 1; const /*@hidden:d*/hidden = 2; /*@hidden:u*/hidden; }
namespace /*@N:d*/N { /*@x:u*/x; /*@?*/hidden; export namespace /*@Inner:de*/Inner { export type /*@T:de*/T = number; } }
const /*@y:d*/y = /*@N:u*/N./*@x:u*/x;
let /*@t:d*/t: /*@N:u*/N./*@Inner:u*/Inner./*@T:u*/T;
namespace /*@A:d*/A./*@B:de*/B { export const /*@ab:de*/ab = 1; }
/*@A:u*/A./*@B:u*/B./*@ab:u*/ab;
/*@y:u*/y; /*@t:u*/t;
"#,
    )]);
}

#[test]
fn labels_and_other_names_are_not_references() {
    check(&[(
        "labels.ts",
        r#"export {};
const /*@target:d*/target = 1;
/*@!*/target: for (;;) { break /*@!*/target; }
/*@target:u*/target;
function /*@f:d*/f() { return new./*@!*/target ?? import./*@!*/meta; }
/*@f:u*/f;
"#,
    )]);
}

#[test]
fn jsx_components_are_references_but_intrinsics_and_attributes_are_not() {
    check(&[(
        "view.tsx",
        r#"export {};
function /*@Comp:d*/Comp(/*@props:d*/props: { /*@!*/title: string }) {
  return </*@!*/div /*@!*/className="x">{/*@props:u*/props./*@!*/title}</ /*@!*/div>;
}
const /*@el:d*/el = </*@Comp:u*/Comp /*@!*/title="t" />;
const /*@el2:d*/el2 = </*@Comp:u*/Comp /*@!*/title="t"></ /*@Comp:u*/Comp>;
/*@el:u*/el; /*@el2:u*/el2;
"#,
    )]);
}

#[test]
fn global_scripts_share_declarations_across_files() {
    check(&[
        (
            "a.ts",
            "var /*@g:d*/shared = 1;\nfunction /*@fn:d*/globalFn() { return /*@g:u*/shared; }\ninterface /*@W:d*/Win { /*@!*/a: number }\n",
        ),
        (
            "b.ts",
            "/*@fn:u*/globalFn(); /*@g:u*/shared;\ninterface /*@W:d*/Win { /*@!*/b: number }\nlet /*@w:d*/w: /*@W:u*/Win;\n",
        ),
        (
            "module.ts",
            "export const /*@local:de*/shared = 2;\n/*@local:u*/shared; /*@fn:u*/globalFn();\n",
        ),
    ]);
}

#[test]
fn names_decode_escapes_and_count_utf16_columns() {
    check(&[(
        "unicode.ts",
        "export {};\nconst /*@u:d*/\\u0061bc = 1;\n/*@u:u*/abc;\nconst /*@astral:d*/𝒳 = 2; /*@astral:u*/𝒳;\r\nconst /*@under:d*/__proto = 3; /*@under:u*/__proto;\u{2028}const /*@esc:d*/\\u{1D4B4}y = 4; /*@esc:u*/𝒴y;\u{2029}/*@u:u*/a\\u{62}c;\n",
    )]);
}

#[test]
fn shared_attribute_children_and_reparsed_nodes_are_visited_once() {
    check(&[
        ("lib.ts", "export const /*@value:de*/value = 1;\n"),
        (
            "app.ts",
            r#"import { /*@value:i*/value } from "./lib" with { /*@!*/kind: "x" };
const /*@result:d*/result = await /*@value:u*/value;
export const /*@later:de*/later = /*@result:u*/result + await /*@value:u*/value;
"#,
        ),
    ]);
}

#[test]
fn global_this_qualifies_global_declarations() {
    check(&[
        (
            "globals.ts",
            "var /*@g:d*/legacy = 1;\nlet /*@b:d*/blockScoped = 2;\ninterface /*@W:d*/Widget {}\n",
        ),
        (
            "app.ts",
            r#"export {};
/*@!*/globalThis./*@g:u*/legacy;
/*@!*/globalThis./*@?*/blockScoped;
let /*@w:d*/w: /*@!*/globalThis./*@W:u*/Widget;
/*@w:u*/w; /*@!*/undefined;
"#,
        ),
    ]);
}

#[test]
fn decorators_on_unusual_declarations_and_orphan_infer_are_classified() {
    check(&[(
        "unusual.ts",
        r#"declare function /*@dec:d*/dec<T>(target: T): T;
namespace /*@M:d*/M { export var /*@X:de*/X: number; }
namespace /*@N:d*/N { @/*@dec:u*/dec import /*@X:i*/Y = /*@M:u*/M./*@X:u*/X; }
class /*@C:d*/C { @/*@dec:u*/dec static {} }
type /*@Orphan:d*/Orphan = (infer /*@U:d*/U)[];
"#,
    )]);
}

#[test]
fn const_assertions_are_not_type_references() {
    check(&[(
        "assertions.ts",
        "export const /*@a:de*/a = [1] as /*@!*/const;\nexport const /*@b:de*/b = </*@!*/const>[/*@a:u*/a];\n",
    )]);
}

#[test]
fn infer_constraints_see_their_own_declaration() {
    check(&[(
        "infer.ts",
        "export {};\ntype /*@U:d*/U = string;\ntype /*@T:d*/T</*@A:d*/A> = /*@A:u*/A extends infer /*@inner:d*/U extends { /*@!*/self: /*@inner:u*/U } ? /*@inner:u*/U : /*@U:u*/U;\n",
    )]);
}

#[test]
fn qualified_receivers_keep_their_lexical_meaning() {
    check(&[
        (
            "lib.ts",
            "const /*@obj:d*/obj = { /*@!*/x: 1 };\nexport default /*@obj:u*/obj./*@!*/x;\nnamespace /*@N:d*/N { export const /*@nx:de*/x = 1; }\nexport const /*@q:de*/q = /*@N:u*/N./*@nx:u*/x;\n",
        ),
        (
            "app.ts",
            "import /*@!*/d from \"./lib\";\n/*@!*/d;\n/*@?*/missing./*@!*/property;\n",
        ),
    ]);
}

#[test]
fn internal_import_aliases_require_namespaces() {
    check(&[(
        "aliases.ts",
        r#"export {};
const /*@x:d*/x = 1;
import /*@?*/y = /*@?*/x;
/*@?*/y;
namespace /*@N:d*/N { export function /*@f:de*/f() {} }
import /*@f:i*/g = /*@N:u*/N./*@f:u*/f;
/*@f:u*/g();
import /*@N:i*/M = /*@N:u*/N;
/*@N:u*/M./*@f:u*/f();
"#,
    )]);
}

#[test]
fn implicit_arguments_cover_parameter_initializers() {
    check(&[(
        "arguments.ts",
        "export {};\nfunction /*@f:d*/f(/*@x:d*/x = /*@!*/arguments) { return [/*@x:u*/x, /*@!*/arguments, () => /*@!*/arguments]; }\n/*@f:u*/f;\n",
    )]);
}

#[test]
fn valid_merges_survive_stricter_duplicate_checks() {
    check(&[(
        "merges.ts",
        r#"export {};
function /*@f:d*/f(): void;
function /*@f:d*/f(/*@x:d*/x?: number) { /*@x:u*/x; }
declare class /*@C:d*/C {}
declare function /*@C:d*/C(): void;
interface /*@I:d*/Box</*@T:d*/T> { /*@!*/a: /*@T:u*/T }
interface /*@I:d*/Box</*@T:d*/T> { /*@!*/b: /*@T:u*/T }
type /*@P:d*/Pair</*@A:d*/A> = /*@A:u*/A extends [infer /*@B:d*/B, infer /*@B:d*/B] ? /*@B:u*/B : never;
catch_scope: try {} catch (/*@e:d*/e) { var /*@v:d*/v = /*@e:u*/e; }
/*@f:u*/f(); /*@C:u*/C; let /*@b:d*/b: /*@I:u*/Box<1> | /*@P:u*/Pair<[1, 1]>; /*@b:u*/b; /*@v:u*/v;
"#,
    )]);
}

#[test]
fn invalid_declaration_arrangements_are_binding_errors() {
    for source in [
        "export {};\nfunction f() {}\nfunction f() {}\n",
        "export {};\nclass C {}\nfunction C() {}\n",
        "export {};\nfunction C() {}\nclass C {}\n",
        "export {};\ntype F<T, T> = T;\n",
        "export {};\nfunction f() { { let x = 1; { var x = 2; } } }\n",
        "export {};\nnamespace N { export interface X {} interface X {} }\n",
        "export interface X {}\ninterface X {}\n",
        "export {};\nnamespace f { export const x = 1; }\nfunction f() {}\n",
        "export {};\ninterface A<T, U> {}\ninterface A<U, T> {}\n",
        "export {};\ninterface B<T> {}\ninterface B {}\n",
        "export {};\ninterface E<T, U> {}\ninterface E<T> {}\n",
        "export {};\ninterface G<T> {}\ninterface G<T, U> {}\n",
        "export {};\ninterface H<T, U = 1> {}\ninterface H<T, V = 1> {}\n",
    ] {
        let fixture = Fixture::new(&[("tsconfig.json", CONFIG), ("app.ts", source)]);
        let error =
            typescript_parser::references::ReferenceProject::load(&fixture.path("tsconfig.json"))
                .expect_err(source);
        assert_eq!(
            error.kind,
            typescript_parser::references::ProjectErrorKind::Binding,
            "{source}: {error}"
        );
    }
}

#[test]
fn typeof_members_of_unknown_values_are_properties() {
    check(&[(
        "typeof.ts",
        "export {};\nlet /*@a:d*/a: typeof /*@?*/Missing./*@!*/member;\nlet /*@b:d*/b: /*@?*/Missing./*@?*/Type;\n/*@a:u*/a; /*@b:u*/b;\n",
    )]);
}

#[test]
fn infer_constraints_see_their_own_name_but_not_siblings() {
    check(&[(
        "infer-siblings.ts",
        "export {};\ntype /*@V:d*/V = string;\ntype /*@T:d*/T</*@A:d*/A> = /*@A:u*/A extends [infer /*@U:d*/U extends { /*@!*/u: /*@U:u*/U; /*@!*/v: /*@V:u*/V }, infer /*@iV:d*/V] ? [/*@U:u*/U, /*@iV:u*/V] : /*@V:u*/V;\n",
    )]);
}

#[test]
fn merged_declarations_may_add_defaulted_type_parameters() {
    check(&[(
        "defaulted-merges.ts",
        "export {};\ninterface /*@A:d*/A</*@T:d*/T> {}\ninterface /*@A:d*/A</*@T:d*/T, /*@U:d*/U = number> {}\nclass /*@D:d*/D</*@DT:d*/T> {}\ninterface /*@D:d*/D</*@DT:d*/T, /*@DU:d*/U = string> {}\ninterface /*@I:d*/I</*@IT:d*/T = 1> {}\ninterface /*@I:d*/I {}\nlet /*@a:d*/a: /*@A:u*/A<1>; let /*@d:d*/d: /*@D:u*/D<1>; let /*@i:d*/i: /*@I:u*/I;\n/*@a:u*/a; /*@d:u*/d; /*@i:u*/i;\n",
    )]);
}

#[test]
fn namespaces_after_functions_and_matching_type_parameters_merge() {
    check(&[(
        "valid-merges.ts",
        "export {};\nfunction /*@f:d*/f() {}\nnamespace /*@f:d*/f { export const /*@x:de*/x = 1; }\ndeclare namespace /*@g:d*/g { const /*@y:d*/y: number; }\ndeclare function /*@g:d*/g(): void;\ninterface /*@A:d*/A</*@T:d*/T> {}\ninterface /*@A:d*/A</*@T:d*/T> {}\n/*@f:u*/f./*@x:u*/x; /*@g:u*/g./*@y:u*/y; let /*@a:d*/a: /*@A:u*/A<1>; /*@a:u*/a;\n",
    )]);
}
