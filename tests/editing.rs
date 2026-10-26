//! Preparing source-preserving edits of named files: which sites support
//! body replacement (E1) and statement insertion (E2), the exact proposed
//! text, and the typed rejections when an edit cannot keep the syntax and
//! comment relationships it must preserve. Expected language behavior comes
//! from TypeScript 7.0.2; layout and seams are the design's product rules.
mod support;

use std::path::PathBuf;
use support::Fixture;
use typescript_parser::editing::{
    self, Edit, EditRequest, Operation, Prepared, Rejection, RejectionKind,
};
use typescript_parser::navigation::{ShowOutcome, ShowQuery};
use typescript_parser::source::SourcePosition;

fn named(fixture: &Fixture, file: &str, name: &str) -> ShowQuery {
    ShowQuery::Name {
        name: name.into(),
        file: Some(fixture.path(file)),
    }
}

fn at(fixture: &Fixture, file: &str, line: usize, column: usize) -> ShowQuery {
    ShowQuery::At {
        file: fixture.path(file),
        position: SourcePosition { line, column },
    }
}

fn edit(operation: Operation, target: ShowQuery, fragment: &str) -> Edit {
    Edit {
        operation,
        target,
        fragment: fragment.into(),
    }
}

fn prepare(edits: Vec<Edit>) -> Prepared {
    editing::prepare(&EditRequest { edits }).expect("the inputs can be analyzed")
}

/// The proposed text of the one file a successful preparation edits.
fn proposed(edits: Vec<Edit>) -> String {
    match prepare(edits) {
        Prepared::Ready(batch) => {
            assert_eq!(batch.files().len(), 1);
            batch.files()[0].proposed_text().to_owned()
        }
        other => panic!("expected a prepared batch, got {other:?}"),
    }
}

fn rejected(edits: Vec<Edit>) -> Rejection {
    match prepare(edits) {
        Prepared::Rejected(rejection) => rejection,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// Replace the body of `name` in a one-file fixture holding `source`.
fn replace(source: &str, name: &str, fragment: &str) -> String {
    let fixture = Fixture::new(&[("a.ts", source)]);
    proposed(vec![edit(
        Operation::ReplaceBody,
        named(&fixture, "a.ts", name),
        fragment,
    )])
}

fn replace_rejected(source: &str, name: &str, fragment: &str) -> Rejection {
    let fixture = Fixture::new(&[("a.ts", source)]);
    rejected(vec![edit(
        Operation::ReplaceBody,
        named(&fixture, "a.ts", name),
        fragment,
    )])
}

#[test]
fn a_function_body_is_replaced_and_everything_else_is_kept_byte_for_byte() {
    let source = "\
// leading comment
export async function load<T>(id: string /* the key */): Promise<T> {
  // old
  return fetch(id) as T;
} // trailing
const after = 1;
";
    assert_eq!(
        replace(
            source,
            "load",
            "{\n  const value = await fetch(id);\n  return value as T;\n}\n"
        ),
        "\
// leading comment
export async function load<T>(id: string /* the key */): Promise<T> {
  const value = await fetch(id);
  return value as T;
} // trailing
const after = 1;
"
    );
}

#[test]
fn body_lines_take_the_indentation_of_the_declaration_line() {
    let source = "\
class Host {
  start(): boolean {
    return false;
  }
}
";
    let expected = "\
class Host {
  start(): boolean {
    if (ready) {
      return true;
    }
    return false;
  }
}
";
    // Unindented, and already indented for the target, give the same text.
    for fragment in [
        "{\n  if (ready) {\n    return true;\n  }\n  return false;\n}",
        "  {\n      if (ready) {\n        return true;\n      }\n      return false;\n    }\n",
    ] {
        assert_eq!(replace(source, "Host.start", fragment), expected);
    }
}

#[test]
fn every_callable_form_has_its_body_replaced() {
    let source = "\
class C {
  constructor(private x: number) { this.x = 1; }
  get value() { return 1; }
  set value(v: number) {}
  *items() { yield 1; }
  async load() {}
  static #secret() {}
}
const o = {
  m() { return 1; },
  p: function () { return 2; },
  q: (a: number) => a,
};
const f = function named() { return 3; };
const g = ((x: number) => { return x; });
const h = async () => { await 0; };
";
    let fixture = Fixture::new(&[("a.ts", source)]);
    let cases = [
        (
            at(&fixture, "a.ts", 2, 3),
            "{ this.x = 2; }",
            "  constructor(private x: number) { this.x = 2; }\n",
        ),
        (
            at(&fixture, "a.ts", 3, 7),
            "{ return 2; }",
            "  get value() { return 2; }\n",
        ),
        (
            at(&fixture, "a.ts", 4, 7),
            "{ void v; }",
            "  set value(v: number) { void v; }\n",
        ),
        (
            named(&fixture, "a.ts", "C.items"),
            "{ yield 2; }",
            "  *items() { yield 2; }\n",
        ),
        (
            named(&fixture, "a.ts", "C.load"),
            "{ await 1; }",
            "  async load() { await 1; }\n",
        ),
        (
            named(&fixture, "a.ts", "C.#secret"),
            "{ return; }",
            "  static #secret() { return; }\n",
        ),
        (
            named(&fixture, "a.ts", "o.m"),
            "{ return 9; }",
            "  m() { return 9; },\n",
        ),
        (
            named(&fixture, "a.ts", "o.p"),
            "{ return 8; }",
            "  p: function () { return 8; },\n",
        ),
        (
            named(&fixture, "a.ts", "o.q"),
            "a * 2",
            "  q: (a: number) => a * 2,\n",
        ),
        (
            named(&fixture, "a.ts", "f"),
            "{ return 4; }",
            "const f = function named() { return 4; };\n",
        ),
        (
            at(&fixture, "a.ts", 15, 12),
            "x * 3",
            "const g = ((x: number) => x * 3);\n",
        ),
        (
            named(&fixture, "a.ts", "g"),
            "x + 1",
            "const g = ((x: number) => x + 1);\n",
        ),
        (
            named(&fixture, "a.ts", "h"),
            "{ await 1; }",
            "const h = async () => { await 1; };\n",
        ),
    ];
    for (target, fragment, line) in cases {
        let text = proposed(vec![edit(Operation::ReplaceBody, target.clone(), fragment)]);
        assert!(
            text.contains(line),
            "{target:?} with {fragment:?} should give {line:?} in:\n{text}"
        );
        let unchanged: Vec<&str> = source.lines().collect();
        let changed: Vec<&str> = text.lines().collect();
        assert_eq!(unchanged.len(), changed.len(), "{target:?}");
        assert_eq!(
            unchanged
                .iter()
                .zip(&changed)
                .filter(|(a, b)| a != b)
                .count(),
            1,
            "{target:?} changes only its own line:\n{text}"
        );
    }
}

#[test]
fn an_arrow_may_change_between_a_block_and_an_expression_in_a_multi_binding_statement() {
    let source = "const run = () => {\n  return 1;\n}, other = 2;\n";
    assert_eq!(
        replace(source, "run", "1 + 1"),
        "const run = () => 1 + 1, other = 2;\n"
    );
    let source = "const run = () => 1, other = 2;\n";
    assert_eq!(
        replace(source, "run", "{\n  return 1;\n}"),
        "const run = () => {\n  return 1;\n}, other = 2;\n"
    );
}

#[test]
fn sites_without_a_direct_body_are_unsupported_and_change_nothing() {
    let source = "\
function over(a: string): void;
function over(a: unknown) {}
declare function ambient(): void;
class K {}
const wrapped = wrap(() => {});
const asserted = (() => {}) as unknown;
const plain = 1;
interface I { m(): void }
abstract class A { abstract m(): void; }
";
    let fixture = Fixture::new(&[("a.ts", source)]);
    for (target, reason) in [
        (at(&fixture, "a.ts", 1, 10), "no body"),
        (named(&fixture, "a.ts", "ambient"), "no body"),
        (named(&fixture, "a.ts", "K"), "no body"),
        (named(&fixture, "a.ts", "wrapped"), "CallExpression"),
        (named(&fixture, "a.ts", "asserted"), "AsExpression"),
        (named(&fixture, "a.ts", "plain"), "NumericLiteral"),
        (named(&fixture, "a.ts", "I.m"), "no body"),
        (named(&fixture, "a.ts", "A.m"), "no body"),
    ] {
        let rejection = rejected(vec![edit(Operation::ReplaceBody, target.clone(), "{}")]);
        assert_eq!(rejection.kind, RejectionKind::UnsupportedSite, "{target:?}");
        assert!(
            rejection.message.contains(reason),
            "{target:?}: {}",
            rejection.message
        );
        assert_eq!(rejection.edit, Some(0));
    }
    // A callback is selected by its own anchor instead.
    assert!(
        proposed(vec![edit(
            Operation::ReplaceBody,
            at(&fixture, "a.ts", 5, 22),
            "{ return 1; }"
        )])
        .contains("const wrapped = wrap(() => { return 1; });\n")
    );
}

#[test]
fn a_body_that_escapes_its_slot_is_rejected() {
    // Both proposed files parse; each adds syntax outside the body.
    let rejection = replace_rejected(
        "function f() {}\n",
        "f",
        "{} export function unexpected() {}",
    );
    assert_eq!(rejection.kind, RejectionKind::Boundary, "{rejection:?}");
    let rejection = replace_rejected("const run = () => 1, other = 2;\n", "run", "x, extra = 3");
    assert_eq!(rejection.kind, RejectionKind::Boundary, "{rejection:?}");
    let rejection = replace_rejected("call(() => 1, 2);\nconst f = 1;\n", "f", "{}");
    assert_eq!(rejection.kind, RejectionKind::UnsupportedSite);
}

#[test]
fn a_fragment_that_does_not_parse_is_located_in_the_proposed_text() {
    let rejection = replace_rejected(
        "function f() {\n  return 1;\n}\nconst g = 2;\n",
        "f",
        "{\n  return (1;\n}",
    );
    assert_eq!(rejection.kind, RejectionKind::Syntax, "{rejection:?}");
    assert_eq!(rejection.edit, Some(0));
    let (position, line) = rejection.proposed.expect("a proposed location");
    assert_eq!(position.line, 2);
    assert_eq!(line, "  return (1;");
    // A method needs a block body.
    let rejection = replace_rejected("class C { m() {} }\n", "C.m", "1");
    assert_eq!(rejection.kind, RejectionKind::Syntax, "{rejection:?}");
}

#[test]
fn selection_is_shows_selection_and_misses_or_ambiguity_change_nothing() {
    let source = "function f() {}\nnamespace N { function f() {} }\n";
    let fixture = Fixture::new(&[("a.ts", source)]);
    match prepare(vec![edit(
        Operation::ReplaceBody,
        named(&fixture, "a.ts", "f"),
        "{}",
    )]) {
        Prepared::Unselected(unselected) => {
            assert_eq!(unselected.edit, 0);
            let ShowOutcome::Ambiguous(candidates) = unselected.outcome else {
                panic!("expected candidates");
            };
            assert_eq!(candidates.len(), 2);
            assert_eq!(candidates[1].selector.as_deref(), Some("N.f"));
        }
        other => panic!("expected an ambiguous selection, got {other:?}"),
    }
    match prepare(vec![edit(
        Operation::ReplaceBody,
        named(&fixture, "a.ts", "missing"),
        "{}",
    )]) {
        Prepared::Unselected(unselected) => {
            assert_eq!(
                unselected.outcome,
                ShowOutcome::NotFound {
                    specifiers: Vec::new()
                }
            );
        }
        other => panic!("expected no selection, got {other:?}"),
    }
    assert_eq!(
        proposed(vec![edit(
            Operation::ReplaceBody,
            named(&fixture, "a.ts", "N.f"),
            "{ return; }"
        )]),
        "function f() {}\nnamespace N { function f() { return; } }\n"
    );
}

#[test]
fn literal_and_multiline_comment_lines_keep_their_bytes() {
    let source = "class C {\n  m() {\n    return 0;\n  }\n}\n";
    let fragment = "{\n  /* keep\n     this */\n  const s = `a\n  b`;\n  const t = 'x\\\n  y';\n  return s + t;\n}";
    assert_eq!(
        replace(source, "C.m", fragment),
        "class C {\n  m() {\n    /* keep\n     this */\n    const s = `a\n  b`;\n    const t = 'x\\\n  y';\n    return s + t;\n  }\n}\n"
    );
}

#[test]
fn line_endings_follow_the_file_and_a_byte_order_mark_is_kept() {
    let source = "\u{feff}function f() {\r\n  return 0;\r\n}\r\nconst g = 1;\r\n";
    let fixture = Fixture::new(&[("a.ts", source)]);
    let Prepared::Ready(batch) = prepare(vec![edit(
        Operation::ReplaceBody,
        named(&fixture, "a.ts", "f"),
        "{\n  return 1;\n}\n",
    )]) else {
        panic!("expected a batch");
    };
    let file = &batch.files()[0];
    assert_eq!(
        file.proposed_text(),
        "function f() {\r\n  return 1;\r\n}\r\nconst g = 1;\r\n"
    );
    assert_eq!(
        file.proposed_bytes(),
        "\u{feff}function f() {\r\n  return 1;\r\n}\r\nconst g = 1;\r\n".as_bytes()
    );
    assert_eq!(file.original_bytes(), source.as_bytes());
    for line_break in ["\r", "\u{2028}", "\u{2029}"] {
        let source = format!("function f() {{{line_break}  return 0;{line_break}}}{line_break}");
        let expected = format!("function f() {{{line_break}  return 1;{line_break}}}{line_break}");
        assert_eq!(
            replace(&source, "f", "{\n  return 1;\n}"),
            expected,
            "{line_break:?}"
        );
    }
}

#[test]
fn a_new_directive_or_jsdoc_may_govern_only_new_code() {
    let source = "function f() {}\nfoo();\nfunction g() {}\n";
    // A directive ending the body would reach the preserved next line.
    let rejection = replace_rejected(source, "f", "{} // @ts-ignore");
    assert_eq!(rejection.kind, RejectionKind::Attachment, "{rejection:?}");
    assert_eq!(
        rejection.position,
        Some(SourcePosition { line: 2, column: 1 })
    );
    // An ordinary trailing comment and a directive for new code are fine.
    assert_eq!(
        replace(source, "f", "{} // done"),
        "function f() {} // done\nfoo();\nfunction g() {}\n"
    );
    assert_eq!(
        replace(source, "f", "{\n  // @ts-ignore\n  missing();\n}"),
        "function f() {\n  // @ts-ignore\n  missing();\n}\nfoo();\nfunction g() {}\n"
    );
    // A JSDoc comment after the body would document the next declaration.
    let source = "function f() {}\nfunction g() {}\n";
    let rejection = replace_rejected(source, "f", "{}\n/** @deprecated */");
    assert!(
        matches!(
            rejection.kind,
            RejectionKind::Boundary | RejectionKind::Attachment
        ),
        "{rejection:?}"
    );
    let rejection = replace_rejected(source, "f", "{}\n/** Documents g. */");
    assert_eq!(rejection.kind, RejectionKind::Attachment, "{rejection:?}");
}

#[test]
fn a_new_body_may_change_the_files_import_summaries() {
    let source = "function f() {}\nfunction g() {}\n";
    assert_eq!(
        replace(source, "f", "{ return import('./x'); }"),
        "function f() { return import('./x'); }\nfunction g() {}\n"
    );
    assert_eq!(
        replace(source, "f", "{ return import.meta.url; }"),
        "function f() { return import.meta.url; }\nfunction g() {}\n"
    );
}

#[test]
fn files_that_are_not_typescript_or_do_not_parse_are_input_failures() {
    let fixture = Fixture::new(&[("a.js", "function f() {}\n"), ("b.ts", "const = 1;\n")]);
    let error = editing::prepare(&EditRequest {
        edits: vec![edit(
            Operation::ReplaceBody,
            named(&fixture, "a.js", "f"),
            "{}",
        )],
    })
    .unwrap_err();
    assert!(error.to_string().contains("cannot be edited"), "{error}");
    let error = editing::prepare(&EditRequest {
        edits: vec![edit(
            Operation::ReplaceBody,
            named(&fixture, "b.ts", "f"),
            "{}",
        )],
    })
    .unwrap_err();
    assert!(error.to_string().contains("b.ts:1:7"), "{error}");
    let _ = PathBuf::new();
}

/// Insert `fragment` after `name`'s statement in a one-file fixture.
fn insert(source: &str, name: &str, fragment: &str) -> String {
    let fixture = Fixture::new(&[("a.ts", source)]);
    proposed(vec![edit(
        Operation::InsertAfter,
        named(&fixture, "a.ts", name),
        fragment,
    )])
}

fn insert_rejected(source: &str, name: &str, fragment: &str) -> Rejection {
    let fixture = Fixture::new(&[("a.ts", source)]);
    rejected(vec![edit(
        Operation::InsertAfter,
        named(&fixture, "a.ts", name),
        fragment,
    )])
}

#[test]
fn statements_go_after_the_anchor_and_its_trailing_comment_before_the_next_group() {
    let source = "\
import { a } from './a';
function first() {} // first's note

// standalone, with the next declaration
/** Documents second. */
function second() {}
";
    assert_eq!(
        insert(source, "first", "const added = 1;\nfunction helper() {}\n"),
        "\
import { a } from './a';
function first() {} // first's note

const added = 1;
function helper() {}

// standalone, with the next declaration
/** Documents second. */
function second() {}
"
    );
}

#[test]
fn a_binding_or_specifier_inserts_after_its_whole_statement() {
    let source = "const a = 1, b = 2;\nimport { x as y } from './x';\nlet last;\n";
    assert_eq!(
        insert(source, "b", "const c = 3;"),
        "const a = 1, b = 2;\nconst c = 3;\nimport { x as y } from './x';\nlet last;\n"
    );
    assert_eq!(
        insert(source, "y", "const c = 3;"),
        "const a = 1, b = 2;\nimport { x as y } from './x';\nconst c = 3;\nlet last;\n"
    );
}

#[test]
fn every_statement_list_takes_insertions_at_its_indentation() {
    let source = "\
function outer() {
  function inner() {}
  return inner;
}
namespace N {
\texport const v = 1;
}
switch (k) {
  case 1:
    function inCase() {}
    break;
}
class C {
  static {
    const s = 1;
  }
}
";
    let fixture = Fixture::new(&[("a.ts", source)]);
    for (target, fragment, expected) in [
        (
            named(&fixture, "a.ts", "outer.inner"),
            "if (ready) {\n  go();\n}",
            "  function inner() {}\n  if (ready) {\n    go();\n  }\n  return inner;\n",
        ),
        (
            named(&fixture, "a.ts", "N.v"),
            "export const w = 2;",
            "\texport const v = 1;\n\texport const w = 2;\n}",
        ),
        (
            named(&fixture, "a.ts", "inCase"),
            "go();",
            "    function inCase() {}\n    go();\n    break;\n",
        ),
        (
            at(&fixture, "a.ts", 15, 11),
            "const t = 2;",
            "    const s = 1;\n    const t = 2;\n  }\n",
        ),
    ] {
        let text = proposed(vec![edit(Operation::InsertAfter, target.clone(), fragment)]);
        assert!(text.contains(expected), "{target:?}:\n{text}");
    }
}

#[test]
fn sites_outside_a_statement_list_are_unsupported() {
    let source = "\
for (const item of items) {}
try {} catch (error) {}
class C {
  constructor(private p: number) {}
  m() {}
}
const o = { k: 1 };
call(() => {});
if (x) function inIf() {}
label: function labeled() {}
";
    let fixture = Fixture::new(&[("a.ts", source)]);
    for (target, reason) in [
        (named(&fixture, "a.ts", "item"), "loop header"),
        (named(&fixture, "a.ts", "error"), "catch clause"),
        (named(&fixture, "a.ts", "C.p"), "not a statement"),
        (named(&fixture, "a.ts", "C.m"), "not a statement"),
        (named(&fixture, "a.ts", "o.k"), "not a statement"),
        (at(&fixture, "a.ts", 8, 6), "not a statement"),
        (named(&fixture, "a.ts", "inIf"), "not a statement"),
        (named(&fixture, "a.ts", "labeled"), "not a statement"),
    ] {
        let rejection = rejected(vec![edit(Operation::InsertAfter, target.clone(), "go();")]);
        assert_eq!(rejection.kind, RejectionKind::UnsupportedSite, "{target:?}");
        assert!(
            rejection.message.contains(reason),
            "{target:?}: {}",
            rejection.message
        );
    }
}

#[test]
fn terminators_separate_statements_a_line_break_would_join() {
    // Without the `;`, the call would apply to `factory()`'s result.
    assert_eq!(
        insert(
            "const run = factory()\nexport {}\n",
            "run",
            "(function added() {})()"
        ),
        "const run = factory();\n(function added() {})()\nexport {}\n"
    );
    // The terminator goes before the statement's trailing comment.
    assert_eq!(
        insert(
            "const run = factory() // made\nlet z\n",
            "run",
            "[1].forEach(go)"
        ),
        "const run = factory(); // made\n[1].forEach(go)\nlet z\n"
    );
    // A statement that cannot continue needs none: no-semicolon style stays.
    assert_eq!(
        insert("const a = 1\nconst b = 2\n", "a", "go()"),
        "const a = 1\ngo()\nconst b = 2\n"
    );
    // The fragment's end is terminated before a statement it would absorb.
    assert_eq!(
        insert("const a = 1;\n(async () => {})();\n", "a", "const b = a"),
        "const a = 1;\nconst b = a;\n(async () => {})();\n"
    );
    for start in ["(x)", "[x]", "`x`", "+x", "-x", "/x/.test(s)", "<T>x"] {
        let source = format!("const a = 1;\n{start};\n");
        let text = insert(&source, "a", "const b = a");
        assert_eq!(
            text,
            format!("const a = 1;\nconst b = a;\n{start};\n"),
            "{start}"
        );
    }
    // An object literal ending the fragment is terminated too.
    assert_eq!(
        insert("const a = 1;\n(async () => {})();\n", "a", "const b = {}"),
        "const a = 1;\nconst b = {};\n(async () => {})();\n"
    );
    // A class declaration ending in `}` needs nothing.
    assert_eq!(
        insert("const a = 1;\n(async () => {})();\n", "a", "class B {}"),
        "const a = 1;\nclass B {}\n(async () => {})();\n"
    );
}

#[test]
fn code_after_the_anchor_on_its_line_moves_after_the_insertion() {
    assert_eq!(
        insert("const a = 1;   const b = 2;\n", "a", "go();"),
        "const a = 1;\ngo();\nconst b = 2;\n"
    );
    assert_eq!(
        insert("{ const a = 1; }\n", "a", "go();"),
        "{ const a = 1;\ngo();\n}\n"
    );
}

#[test]
fn an_insertion_may_not_retarget_a_directive_or_jsdoc() {
    // A trailing directive on the anchor's line governs the next line.
    let rejection = insert_rejected("const a = 1; // @ts-ignore\nmissing();\n", "a", "go();");
    assert_eq!(rejection.kind, RejectionKind::Attachment, "{rejection:?}");
    assert_eq!(
        rejection.position,
        Some(SourcePosition { line: 2, column: 1 })
    );
    // A directive on its own line stays with the statement it governs.
    assert_eq!(
        insert(
            "const a = 1;\n// @ts-expect-error\nmissing();\n",
            "a",
            "go();"
        ),
        "const a = 1;\ngo();\n// @ts-expect-error\nmissing();\n"
    );
    // A directive or JSDoc ending the fragment would govern what follows.
    for fragment in ["go();\n// @ts-ignore", "go();\n/** Documents b. */"] {
        let rejection = insert_rejected("const a = 1;\nconst b = 2;\n", "a", fragment);
        assert_eq!(rejection.kind, RejectionKind::Attachment, "{fragment:?}");
    }
    // A directive the fragment gives its own code is fine, as is a
    // trailing line comment.
    assert_eq!(
        insert(
            "const a = 1;\nconst b = 2;\n",
            "a",
            "// @ts-ignore\nmissing(); // why"
        ),
        "const a = 1;\n// @ts-ignore\nmissing(); // why\nconst b = 2;\n"
    );
    // A deprecation tag ending the fragment would mark what follows.
    let rejection = insert_rejected(
        "const a = 1;\nfunction b() {}\n",
        "a",
        "go();\n/** @deprecated */",
    );
    assert!(
        matches!(
            rejection.kind,
            RejectionKind::Boundary | RejectionKind::Attachment
        ),
        "{rejection:?}"
    );
}

#[test]
fn a_module_indicator_that_reparses_preserved_code_is_rejected() {
    // `export {}` makes the file a module, where `await(foo)` is an await
    // expression rather than a call (TypeScript 7.0.2 parses it so).
    let rejection = insert_rejected("const a = 1;\nconst b = await(foo);\n", "a", "export {};");
    assert_eq!(rejection.kind, RejectionKind::Boundary, "{rejection:?}");
    // Without such code, adding a module indicator is a syntax-preserving
    // edit, whatever it does to binding.
    assert_eq!(
        insert("const a = 1;\nconst b = 2;\n", "a", "export {};"),
        "const a = 1;\nexport {};\nconst b = 2;\n"
    );
}

#[test]
fn insertions_follow_the_files_layout() {
    // CRLF, tabs, a byte order mark, and the last line without a newline.
    assert_eq!(
        insert(
            "namespace N {\r\n\tconst a = 1;\r\n}",
            "N.a",
            "if (x) {\n  go();\n}\n"
        ),
        "namespace N {\r\n\tconst a = 1;\r\n\tif (x) {\r\n\t  go();\r\n\t}\r\n}"
    );
    assert_eq!(insert("const a = 1;", "a", "go();"), "const a = 1;\ngo();");
    let fixture = Fixture::new(&[("a.ts", "\u{feff}const a = 1;\n")]);
    let Prepared::Ready(batch) = prepare(vec![edit(
        Operation::InsertAfter,
        named(&fixture, "a.ts", "a"),
        "go();",
    )]) else {
        panic!("expected a batch");
    };
    assert_eq!(
        batch.files()[0].proposed_bytes(),
        "\u{feff}const a = 1;\ngo();\n".as_bytes()
    );
    // Blank lines inside the fragment stay blank; literal lines keep bytes.
    assert_eq!(
        insert(
            "function f() {\n    const a = 1;\n}\n",
            "f.a",
            "go();\n\n  const s = `x\n  y`;"
        ),
        "function f() {\n    const a = 1;\n    go();\n\n      const s = `x\n  y`;\n}\n"
    );
}

#[test]
fn a_batch_selects_every_site_in_the_original_sources() {
    let source = "\
function a() { return 1; }
function b() { return 2; }
const c = () => 3;
";
    let fixture = Fixture::new(&[("a.ts", source), ("b.ts", "export const x = 1;\n")]);
    let Prepared::Ready(batch) = prepare(vec![
        edit(
            Operation::ReplaceBody,
            named(&fixture, "a.ts", "b"),
            "{ return 20; }",
        ),
        edit(
            Operation::InsertAfter,
            named(&fixture, "a.ts", "c"),
            "function c2() {}",
        ),
        edit(Operation::ReplaceBody, named(&fixture, "a.ts", "c"), "30"),
        edit(
            Operation::InsertAfter,
            named(&fixture, "b.ts", "x"),
            "export const y = 2;",
        ),
        edit(
            Operation::ReplaceBody,
            named(&fixture, "a.ts", "a"),
            "{ return 10; }",
        ),
    ]) else {
        panic!("expected a batch");
    };
    let files: Vec<(&str, &str)> = batch
        .files()
        .iter()
        .map(|file| (file.display(), file.proposed_text()))
        .collect();
    assert_eq!(
        files,
        [
            (
                fixture.path("a.ts").to_str().unwrap(),
                "function a() { return 10; }\nfunction b() { return 20; }\nconst c = () => 30;\nfunction c2() {}\n"
            ),
            (
                fixture.path("b.ts").to_str().unwrap(),
                "export const x = 1;\nexport const y = 2;\n"
            ),
        ]
    );
    let edits: Vec<usize> = batch.files()[0]
        .edits()
        .iter()
        .map(|edit| edit.index)
        .collect();
    assert_eq!(edits, [0, 1, 2, 4]);
    assert_eq!(batch.files()[0].edits()[1].site.label, "c");
    // Every site is selected in the original, where `c2` does not exist.
    match prepare(vec![
        edit(
            Operation::InsertAfter,
            named(&fixture, "a.ts", "c"),
            "function c2() {}",
        ),
        edit(Operation::ReplaceBody, named(&fixture, "a.ts", "c2"), "{}"),
    ]) {
        Prepared::Unselected(unselected) => {
            assert_eq!(
                (unselected.edit, unselected.outcome),
                (
                    1,
                    ShowOutcome::NotFound {
                        specifiers: Vec::new()
                    }
                )
            );
        }
        other => panic!("expected no selection, got {other:?}"),
    }
}

#[test]
fn edits_that_would_need_an_order_conflict() {
    let source = "\
function outer() {
  function inner() {}
}
function last() {} // note
";
    let fixture = Fixture::new(&[("a.ts", source)]);
    let replace = |name: &str| edit(Operation::ReplaceBody, named(&fixture, "a.ts", name), "{}");
    let after = |name: &str| {
        edit(
            Operation::InsertAfter,
            named(&fixture, "a.ts", name),
            "go();",
        )
    };
    for edits in [
        // Nested and duplicate replacements.
        vec![replace("outer"), replace("outer.inner")],
        vec![replace("last"), replace("last")],
        // Two insertions at one seam.
        vec![after("outer"), after("outer")],
    ] {
        let rejection = rejected(edits.clone());
        assert_eq!(rejection.kind, RejectionKind::Conflict, "{edits:?}");
        assert!(
            rejection.message.contains("edits 1 and 2"),
            "{}",
            rejection.message
        );
    }
    // A trailing comment separates a replacement from the insertion seam.
    assert_eq!(
        proposed(vec![replace("last"), after("last")]),
        "function outer() {\n  function inner() {}\n}\nfunction last() {} // note\ngo();\n"
    );
}

#[cfg(unix)]
#[test]
fn one_file_named_two_ways_is_one_captured_source() {
    let fixture = Fixture::new(&[("a.ts", "function f() {}\nfunction g() {}\n")]);
    std::os::unix::fs::symlink(fixture.path("a.ts"), fixture.path("link.ts")).unwrap();
    let Prepared::Ready(batch) = prepare(vec![
        edit(
            Operation::ReplaceBody,
            named(&fixture, "a.ts", "f"),
            "{ 1; }",
        ),
        edit(
            Operation::ReplaceBody,
            named(&fixture, "link.ts", "g"),
            "{ 2; }",
        ),
    ]) else {
        panic!("expected a batch");
    };
    assert_eq!(batch.files().len(), 1);
    assert_eq!(
        batch.files()[0].proposed_text(),
        "function f() { 1; }\nfunction g() { 2; }\n"
    );
    // A link whose name parses differently is refused rather than guessed.
    std::os::unix::fs::symlink(fixture.path("a.ts"), fixture.path("link.tsx")).unwrap();
    let error = editing::prepare(&EditRequest {
        edits: vec![
            edit(Operation::ReplaceBody, named(&fixture, "a.ts", "f"), "{}"),
            edit(
                Operation::ReplaceBody,
                named(&fixture, "link.tsx", "g"),
                "{}",
            ),
        ],
    })
    .unwrap_err();
    assert!(error.to_string().contains("parse differently"), "{error}");
}

#[cfg(unix)]
#[test]
fn a_hard_linked_file_is_not_edited() {
    let fixture = Fixture::new(&[("a.ts", "function f() {}\n")]);
    std::fs::hard_link(fixture.path("a.ts"), fixture.path("b.ts")).unwrap();
    let error = editing::prepare(&EditRequest {
        edits: vec![edit(
            Operation::ReplaceBody,
            named(&fixture, "a.ts", "f"),
            "{}",
        )],
    })
    .unwrap_err();
    assert!(error.to_string().contains("hard links"), "{error}");
}

#[test]
fn positions_count_utf16_and_every_line_break() {
    // Astral and non-ASCII text before the site, NBSP indentation that is
    // not layout indentation, and U+2028 and U+2029 line breaks.
    let source = "const s = '😀é';\u{2028}function f() {}\u{2029}\u{a0}const t = 1;\n";
    let fixture = Fixture::new(&[("a.ts", source)]);
    assert_eq!(
        proposed(vec![edit(
            Operation::InsertAfter,
            at(&fixture, "a.ts", 2, 10),
            "go();"
        )]),
        "const s = '😀é';\u{2028}function f() {}\u{2029}go();\u{2029}\u{a0}const t = 1;\n"
    );
    assert_eq!(
        proposed(vec![edit(
            Operation::ReplaceBody,
            at(&fixture, "a.ts", 2, 10),
            "{\n  go();\n}"
        )]),
        "const s = '😀é';\u{2028}function f() {\u{2029}  go();\u{2029}}\u{2029}\u{a0}const t = 1;\n"
    );
    let rejection = rejected(vec![edit(
        Operation::InsertAfter,
        named(&fixture, "a.ts", "t"),
        "const = 1;",
    )]);
    assert_eq!(rejection.kind, RejectionKind::Syntax);
    assert_eq!(
        rejection.proposed.unwrap().0,
        SourcePosition { line: 4, column: 7 }
    );
}

#[test]
fn a_preserved_line_keeps_the_directive_that_governed_it() {
    // TypeScript 7.0.2 reports TS2578 (unused @ts-expect-error) at the old
    // directive once a fragment's directive takes over its line.
    for line_break in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        for old in ["@ts-ignore", "@ts-expect-error"] {
            for new in ["@ts-ignore", "@ts-expect-error"] {
                let source =
                    format!("function f() {{}} // {old}{line_break}missing();{line_break}");
                let fixture = Fixture::new(&[("a.ts", &source)]);
                let rejection = rejected(vec![edit(
                    Operation::InsertAfter,
                    named(&fixture, "a.ts", "f"),
                    &format!("const added = 1; // {new}"),
                )]);
                assert_eq!(
                    rejection.kind,
                    RejectionKind::Attachment,
                    "{old} then {new} with {line_break:?}"
                );
            }
        }
    }
}

#[test]
fn jsx_text_starting_with_a_line_break_keeps_its_bytes() {
    let source = "class C {\r\n  m() { return <div/>; }\r\n}\r\nconst t = 1;\r\n";
    let fixture = Fixture::new(&[("a.tsx", source)]);
    let body = "{\n  return <div>\n  hello\nworld</div>;\n}";
    let text = proposed(vec![edit(
        Operation::ReplaceBody,
        named(&fixture, "a.tsx", "C.m"),
        body,
    )]);
    assert!(
        text.contains("return <div>\n  hello\nworld</div>;"),
        "{text:?}"
    );
    let statement = "const u = <p>\n  x\n</p>;";
    let text = proposed(vec![edit(
        Operation::InsertAfter,
        named(&fixture, "a.tsx", "t"),
        statement,
    )]);
    assert!(text.ends_with("const u = <p>\n  x\n</p>;\r\n"), "{text:?}");
}

#[test]
fn a_fragment_ending_in_any_complete_statement_is_separated_from_what_follows() {
    let source = "const a = 1;\n(function(){})();\n";
    for (fragment, inserted) in [
        ("const b = 'https://host'", "const b = 'https://host';"),
        ("const b = {}", "const b = {};"),
        ("const b = 2 // note", "const b = 2; // note"),
        ("const b = 2 /* note */", "const b = 2; /* note */"),
        ("const b = `x\ny`", "const b = `x\ny`;"),
        ("class B {}", "class B {}"),
        ("function g() {}", "function g() {}"),
        ("const b = 2;", "const b = 2;"),
        ("if (x) {\n  go();\n}", "if (x) {\n  go();\n}"),
        ("go()\nmore()", "go()\nmore();"),
    ] {
        assert_eq!(
            insert(source, "a", fragment),
            format!("const a = 1;\n{inserted}\n(function(){{}})();\n"),
            "{fragment:?}"
        );
    }
    // An incomplete statement would take the next one as its body.
    let rejection = insert_rejected(source, "a", "if (x)");
    assert_eq!(rejection.kind, RejectionKind::Boundary, "{rejection:?}");
}

#[test]
fn a_fragment_without_a_statement_is_rejected_and_an_empty_statement_is_not() {
    for source in [
        "const a = 1;\n(function(){})();\n",
        "const a = 1;\nconst c = 3;\n",
    ] {
        for fragment in ["", "  \n", "/* nothing */", "// nothing"] {
            let rejection = insert_rejected(source, "a", fragment);
            assert_eq!(
                rejection.kind,
                RejectionKind::Boundary,
                "{source:?} {fragment:?}"
            );
            assert!(
                rejection.message.contains("no statement"),
                "{}",
                rejection.message
            );
        }
        assert!(insert(source, "a", ";").starts_with("const a = 1;\n;\n"));
    }
}

#[test]
fn an_original_over_the_nesting_budget_is_a_resource_limit() {
    use typescript_parser::analysis::{AnalysisError, InputErrorKind};
    let nested = |depth: usize| {
        format!(
            "function f() {{ return {}0{}; }}\n",
            "(".repeat(depth),
            ")".repeat(depth)
        )
    };
    let fixture = Fixture::new(&[("under.ts", &nested(202)), ("over.ts", &nested(203))]);
    proposed(vec![edit(
        Operation::ReplaceBody,
        named(&fixture, "under.ts", "f"),
        "{}",
    )]);
    let error = editing::prepare(&EditRequest {
        edits: vec![edit(
            Operation::ReplaceBody,
            named(&fixture, "over.ts", "f"),
            "{}",
        )],
    })
    .unwrap_err();
    let AnalysisError::Input(error) = error else {
        panic!("{error:?}");
    };
    assert_eq!(error.kind, InputErrorKind::ResourceLimit);
    assert!(error.position.is_some());
    // The same budget over a proposed file is a rejection of the edit.
    let rejection = rejected(vec![edit(
        Operation::ReplaceBody,
        named(&fixture, "under.ts", "f"),
        &format!("{{ return {}0{}; }}", "(".repeat(203), ")".repeat(203)),
    )]);
    assert_eq!(rejection.kind, RejectionKind::ResourceLimit);
}

#[test]
fn a_new_directive_may_not_take_over_a_line_in_either_operation() {
    // E1: the new body's directive would govern `bad()` after the body,
    // which the preserved `@ts-expect-error` governed.
    let source = "declare function f(cb: () => void): void;\n// @ts-expect-error\nf(function () {}); bad();\n";
    let fixture = Fixture::new(&[("a.ts", source)]);
    let rejection = rejected(vec![edit(
        Operation::ReplaceBody,
        at(&fixture, "a.ts", 3, 3),
        "{\n  // @ts-ignore\n}",
    )]);
    assert_eq!(rejection.kind, RejectionKind::Attachment, "{rejection:?}");
    // A block comment's directive is on its last line.
    let source = "/* first\n @ts-expect-error */\nconst a = 1; bad();\n";
    let rejection = insert_rejected(source, "a", "go();\n/*\n @ts-ignore */");
    assert_eq!(rejection.kind, RejectionKind::Attachment, "{rejection:?}");
    assert_eq!(rejection.edit, Some(0));
}

#[test]
fn jsdoc_is_compared_for_the_same_preserved_declaration() {
    // The terminator is its own change before the trailing comment; the
    // declaration after the seam keeps its JSDoc.
    assert_eq!(
        insert(
            "const run = factory() // made\n/** Doc */\nfunction next() {}\n",
            "run",
            "(function added() {})()"
        ),
        "const run = factory(); // made\n(function added() {})()\n/** Doc */\nfunction next() {}\n"
    );
    assert_eq!(
        insert(
            "const run = factory() // made\nlet z\n",
            "run",
            "/** Doc */\n(function added() {})()"
        ),
        "const run = factory(); // made\n/** Doc */\n(function added() {})()\nlet z\n"
    );
}

#[test]
fn the_last_statement_of_a_clause_takes_insertions() {
    for (source, expected) in [
        (
            "switch (x) {\n  case 1:\n    const a = 1; // note\n  case 2:\n    break;\n}\n",
            "switch (x) {\n  case 1:\n    const a = 1; // note\n    go();\n  case 2:\n    break;\n}\n",
        ),
        (
            "switch (x) {\n  case 1: const a = 1; case 2: break;\n}\n",
            "switch (x) {\n  case 1: const a = 1;\n  go();\n  case 2: break;\n}\n",
        ),
        (
            "switch (x) {\n  default:\n    const a = 1; // note\n}\n",
            "switch (x) {\n  default:\n    const a = 1; // note\n    go();\n}\n",
        ),
    ] {
        assert_eq!(insert(source, "a", "go();"), expected, "{source:?}");
    }
    let source = "switch (x) {\n  case 1:\n    const a = 1;\n}\n";
    for (fragment, inserted) in [
        ("go(); // why", "go(); // why"),
        ("go();\n// done", "go();\n    // done"),
        ("go(); /* done */", "go(); /* done */"),
    ] {
        assert_eq!(
            insert(source, "a", fragment),
            format!("switch (x) {{\n  case 1:\n    const a = 1;\n    {inserted}\n}}\n"),
            "{fragment:?}"
        );
    }
}

#[test]
fn increments_do_not_continue_a_statement() {
    // `++` and `--` after a line break start a new statement (TypeScript's
    // restricted production), so no-semicolon code stays as it is.
    assert_eq!(
        insert("let a = b\nlet c\n", "a", "++i"),
        "let a = b\n++i\nlet c\n"
    );
    assert_eq!(
        insert("let a = b\n++i\n", "a", "go()"),
        "let a = b\ngo()\n++i\n"
    );
}

#[test]
fn a_single_line_fragment_takes_the_seams_indentation() {
    let source = "function f() {\n  const a = 1;\n}\n";
    assert_eq!(
        insert(source, "f.a", "    go();"),
        "function f() {\n  const a = 1;\n  go();\n}\n"
    );
    assert_eq!(
        insert(source, "f.a", "    go();\n    stop();"),
        "function f() {\n  const a = 1;\n  go();\n  stop();\n}\n"
    );
}

#[test]
fn a_deprecation_tag_changing_a_preserved_declaration_is_a_boundary_violation() {
    let rejection = insert_rejected(
        "const a = 1;\nfunction b() {}\n",
        "a",
        "go();\n/** @deprecated */",
    );
    assert_eq!(rejection.kind, RejectionKind::Boundary, "{rejection:?}");
    assert!(
        rejection.message.contains("deprecated"),
        "{}",
        rejection.message
    );
}

#[test]
fn the_nesting_budget_applies_to_every_parse_of_a_proposal() {
    let nested = |depth: usize, lines: bool| {
        let (open, close) = if lines { ("\n  ", "\n") } else { (" ", " ") };
        format!(
            "{{{open}return {}0{};{close}}}",
            "(".repeat(depth),
            ")".repeat(depth)
        )
    };
    // A one-line body is parsed once; a body of several lines in a CRLF
    // file is drafted, settled to CRLF, and parsed again. 202 levels of
    // parentheses fit the budget here and 203 do not.
    for (source, lines) in [
        ("function f() {}\nconst g = 1;\n", false),
        ("function f() {}\r\nconst g = 1;\r\n", true),
    ] {
        for depth in [201, 202] {
            let text = replace(source, "f", &nested(depth, lines));
            assert!(text.contains(&format!("{}0", "(".repeat(depth))), "{depth}");
            assert_eq!(text.contains("{\r\n  return"), lines, "{depth}");
        }
        let rejection = replace_rejected(source, "f", &nested(203, lines));
        assert_eq!(rejection.kind, RejectionKind::ResourceLimit, "{lines}");
    }
    // A long flat list stays well under the budget.
    let flat = format!("{{ return [{}]; }}", vec!["0"; 40_000].join(", "));
    assert!(replace("function f() {}\n", "f", &flat).contains("return [0, 0"));
}

#[test]
fn an_attachment_rejection_names_its_edit() {
    let fixture = Fixture::new(&[("a.ts", "const a = 1;\nconst b = 2;\nconst c = 3;\n")]);
    let rejection = rejected(vec![
        edit(
            Operation::InsertAfter,
            named(&fixture, "a.ts", "a"),
            "go();",
        ),
        edit(
            Operation::InsertAfter,
            named(&fixture, "a.ts", "b"),
            "go();\n// @ts-ignore",
        ),
    ]);
    assert_eq!(rejection.kind, RejectionKind::Attachment, "{rejection:?}");
    assert_eq!(rejection.edit, Some(1));
}

#[test]
fn an_insertion_is_separated_as_the_anchor_is_from_what_follows() {
    // A blank line after the anchor separates the insertion on both sides.
    assert_eq!(
        insert(
            "function a() {}\n\nfunction b() {}\n",
            "a",
            "function c() {}"
        ),
        "function a() {}\n\nfunction c() {}\n\nfunction b() {}\n"
    );
    assert_eq!(
        insert(
            "function a() {} // note\r\n\r\n// b's group\r\nfunction b() {}\r\n",
            "a",
            "function c() {}"
        ),
        "function a() {} // note\r\n\r\nfunction c() {}\r\n\r\n// b's group\r\nfunction b() {}\r\n"
    );
    // Without one, none is added.
    assert_eq!(
        insert("const a = 1;\nconst b = 2;\n", "a", "const c = 3;"),
        "const a = 1;\nconst c = 3;\nconst b = 2;\n"
    );
}

#[test]
fn complete_compound_and_bodiless_statements_are_terminated_after_their_last_token() {
    // Each is complete before a statement it would otherwise run into;
    // TypeScript 6.0.3's syntax API and 7.0.2 (--noCheck) accept each
    // separated form.
    let source = "function f() {\n  const a = 1;\n  (function(){})();\n}\n";
    for (fragment, inserted) in [
        ("do {} while (false)", "do {} while (false);"),
        ("if (true) go()", "if (true) go();"),
        ("while (false) go()", "while (false) go();"),
        ("for (;;) go()", "for (;;) go();"),
        ("label: go()", "label: go();"),
        ("if (a) {} else b()", "if (a) {} else b();"),
        ("if (a) go() // why", "if (a) go(); // why"),
    ] {
        assert_eq!(
            insert(source, "f.a", fragment),
            format!("function f() {{\n  const a = 1;\n  {inserted}\n  (function(){{}})();\n}}\n"),
            "{fragment:?}"
        );
    }
    let source = "const a = 1;\n(async () => {})();\n";
    for (fragment, inserted) in [
        (
            "declare function f(a: number)",
            "declare function f(a: number);",
        ),
        ("declare function h<T>()", "declare function h<T>();"),
    ] {
        assert_eq!(
            insert(source, "a", fragment),
            format!("const a = 1;\n{inserted}\n(async () => {{}})();\n"),
            "{fragment:?}"
        );
    }
}

#[test]
fn a_body_and_an_insertion_after_its_declaration_compose() {
    // The body lies inside the anchor statement, so the insertion follows it.
    let fixture = Fixture::new(&[("a.ts", "function f() {}\nconst f2 = () => {}\nlet z = 1\n")]);
    assert_eq!(
        proposed(vec![
            edit(
                Operation::ReplaceBody,
                named(&fixture, "a.ts", "f"),
                "{ go(); }"
            ),
            edit(
                Operation::InsertAfter,
                named(&fixture, "a.ts", "f"),
                "function g() {}"
            ),
            edit(Operation::ReplaceBody, named(&fixture, "a.ts", "f2"), "1"),
            edit(
                Operation::InsertAfter,
                named(&fixture, "a.ts", "f2"),
                "(go)()"
            ),
        ]),
        "function f() { go(); }\nfunction g() {}\nconst f2 = () => 1;\n(go)()\nlet z = 1\n"
    );
}
