//! File navigation through the public library operations.
//!
//! Inclusion, containment, labels, and anchors of the checked-in fixtures
//! were observed with TypeScript 7.0.2's `textDocument/documentSymbol`
//! (`fixtures/navigation/*.symbols.json`, pinned by
//! `tools/navigation-oracle.test.mjs`), with kinds mapped to the catalog
//! vocabulary and the deliberate differences of `docs/navigation.md` applied:
//! no shorthand, spread, expando, or merged entries, and nothing from
//! parameters, types, decorators, or heritage clauses.
mod support;

use std::path::{Path, PathBuf};
use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, InputError, InputErrorKind, QueryError, QueryErrorKind,
};
use typescript_parser::navigation::{
    self, Declaration, Kind, Outline, Selector, Show, ShowOutcome, Specifier,
};
use typescript_parser::source::SourcePosition;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/navigation")
        .join(name)
}

fn outline(path: &Path) -> Outline {
    navigation::outline_file(path).unwrap_or_else(|error| panic!("{error}"))
}

/// One line per entry: indentation by depth, kind, label, and anchor.
fn tree(outline: &Outline) -> String {
    outline
        .entries
        .iter()
        .map(|entry| {
            format!(
                "{}{} {} {}:{}\n",
                "  ".repeat(entry.depth - 1),
                entry.kind,
                entry.label,
                entry.anchor.line,
                entry.anchor.column
            )
        })
        .collect()
}

fn at(line: usize, column: usize) -> SourcePosition {
    SourcePosition { line, column }
}

fn show(path: &Path, selector: Selector) -> Show {
    navigation::show_file(path, &selector).unwrap_or_else(|error| panic!("{error}"))
}

fn found(path: &Path, selector: Selector) -> (Declaration, String) {
    match show(path, selector.clone()).outcome {
        ShowOutcome::Found(found) => (found.declaration, found.source),
        other => panic!("{selector:?} selected {other:?}"),
    }
}

/// The input failure a navigation request should report.
fn input_error<T: std::fmt::Debug>(result: Result<T, AnalysisError>) -> InputError {
    match result {
        Err(AnalysisError::Input(error)) => error,
        other => panic!("expected an input failure: {other:?}"),
    }
}

/// The query failure a navigation request should report.
fn query_error<T: std::fmt::Debug>(result: Result<T, AnalysisError>) -> QueryError {
    match result {
        Err(AnalysisError::Query(error)) => error,
        other => panic!("expected a query failure: {other:?}"),
    }
}

fn name(text: &str) -> Selector {
    Selector::Name(text.into())
}

fn candidates(path: &Path, selector: Selector) -> Vec<SourcePosition> {
    match show(path, selector.clone()).outcome {
        ShowOutcome::Ambiguous(candidates) => candidates.iter().map(|c| c.anchor).collect(),
        other => panic!("{selector:?} selected {other:?}"),
    }
}

fn missing(path: &Path, selector: Selector) {
    let outcome = show(path, selector.clone()).outcome;
    assert_eq!(
        outcome,
        ShowOutcome::NotFound {
            specifiers: Vec::new()
        },
        "{selector:?}"
    );
}

#[test]
fn factories_list_nested_functions_returned_members_and_initializer_contents() {
    // The returned `{ startEligible }` shorthand and the `...defaults` spread
    // are compiler entries this catalog omits.
    assert_eq!(
        tree(&outline(&fixture("factory.ts"))),
        "\
interface RunHostDeps 2:18
  method start 2:32
interface RunHost 3:18
  method startEligible 3:28
  method run 3:54
function createRunHost 5:17
  function startEligible 6:12
  variable helper 9:9
    function step 10:14
  method run 13:27
  property stop 13:48
    function halt 13:71
variable settings 16:7
  property retries 17:3
  property \"display-name\" 18:3
  property handlers 19:3
    method onStart 19:15
    property onStop 19:29
  property compute 20:3
variable defaults 23:15
variable wrapped 25:7
  function wrap() callback 25:22
    variable inner 26:9
function wrap 28:18
class Widget 30:14
  property handle 31:3
    function deep 32:14
  property #count 34:10
  property label 35:3
  method method 36:3
  method method 37:3
  method method 38:3
  constructor constructor 39:3
  property id 39:32
  property name 39:51
  accessor size 40:7
  accessor size 43:7
  variable blockLocal 45:11
  signature [] 47:3
interface Shape 50:18
  method area 51:3
  property name 52:3
  signature () 53:3
  signature new() 54:3
  signature [] 55:3
"
    );
}

#[test]
fn expando_assignments_have_no_entries_and_their_functions_stay_where_written() {
    // The compiler lists each `f.x = …` assignment as an entry merged under
    // `f`, containing the assigned function; it also merges a property into a
    // same-named sibling function, class, or variable. The catalog lists
    // neither: an assigned function is an entry where its assignment is,
    // labelled by the assigned name, and a property keeps its own site.
    let path = fixture("expandos.ts");
    assert_eq!(
        tree(&outline(&path)),
        "\
function target 2:10
function callback 4:19
  variable inCallback 5:9
function createHost 8:10
  function startEligible 9:12
  variable run 10:9
  property run 13:5
    function nested 14:16
  method other 16:5
class Counter 20:7
  accessor size 21:7
  property size2 22:3
function size 24:10
variable config 26:7
  property retries 26:18
property config 27:8
property standalone 27:19
function call 28:18
function method 29:17
  function keepMe 30:12
function outer 32:10
  function inner 33:12
  function attached 34:20
    function keepInner 35:14
"
    );
    // Assigned names are labels, not keys.
    missing(&path, name("method"));
    missing(&path, name("attached.keepInner"));
    let (keep, source) = found(&path, name("outer.keepInner"));
    assert_eq!(
        (keep.anchor, source.as_str()),
        (at(35, 14), "function keepInner() {}")
    );
    let (method, source) = found(&path, Selector::At(at(29, 17)));
    assert_eq!(method.label, "method");
    assert_eq!(method.key, None);
    assert_eq!(source, "function () {\n  function keepMe() {}\n}");
}

#[test]
fn jsdoc_type_tags_declare_nothing_in_typescript_files() {
    // The compiler lists `@typedef` and `@callback` tags as entries (and
    // parses `@typedef` mid-sentence as one); in a TypeScript file they
    // declare nothing, so the catalog lists only the declarations.
    let path = fixture("jsdoc.ts");
    assert_eq!(
        tree(&outline(&path)),
        "variable named 3:14\nfunction handle 10:17\n  variable local 12:9\nclass Documented 16:14\n"
    );
    for selector in ["Name", "Handler", "Local", "is"] {
        missing(&path, name(selector));
    }
    missing(&path, Selector::At(at(2, 23)));
    // An excerpt excludes the leading JSDoc comment.
    assert_eq!(
        found(&path, name("handle")).1,
        "export function handle(value: string): void {\n  /** @typedef {{ inner: number }} Local */\n  const local = 1;\n}"
    );
}

#[test]
fn spreads_and_shorthand_properties_have_no_entries_but_their_contents_belong_to_the_container() {
    // The compiler lists each object spread as a leaf labelled by its
    // expression (`{ run() { … } }`) and lists nothing inside it; the
    // catalog lists no spread entry and traverses its expression under the
    // enclosing entry, as it does for array and argument spreads. A
    // shorthand property's default in a destructuring assignment is
    // traversed the same way.
    let path = fixture("spreads.ts");
    assert_eq!(
        tree(&outline(&path)),
        "\
function build 2:18
variable base 3:15
variable spread 5:7
  method run 5:23
    function nested 5:40
variable spreadCall 6:7
  function build() callback 6:31
    function inSpread 6:48
function host 7:17
  property inner 10:10
    method deep 10:19
  property own 11:5
variable elements 14:7
  function inArray 14:32
function <function> 15:11
  function inArguments 15:28
variable target 16:5
function fallback 17:22
"
    );
    for (selector, label, excerpt) in [
        ("spread.run", "run", "run() { function nested() {} }"),
        ("spread.run.nested", "nested", "function nested() {}"),
        ("run.nested", "nested", "function nested() {}"),
        ("spreadCall.inSpread", "inSpread", "function inSpread() {}"),
        ("host.inner", "inner", "inner: { deep() {} }"),
        ("host.inner.deep", "deep", "deep() {}"),
        ("elements.inArray", "inArray", "function inArray() {}"),
        ("inArguments", "inArguments", "function inArguments() {}"),
        ("fallback", "fallback", "function fallback() {}"),
    ] {
        let (declaration, source) = found(&path, name(selector));
        assert_eq!(
            (declaration.label.as_str(), source.as_str()),
            (label, excerpt),
            "{selector}"
        );
        // The returned anchor selects the same site.
        let (again, again_source) = found(&path, Selector::At(declaration.anchor));
        assert_eq!(again, declaration, "{selector}");
        assert_eq!(again_source, source, "{selector}");
    }
    let (callback, source) = found(&path, Selector::At(at(6, 31)));
    assert_eq!(callback.label, "build() callback");
    assert_eq!(source, "() => { function inSpread() {} }");
    // The spread expression is no site, and the shorthand declares nothing.
    missing(&path, Selector::At(at(5, 18)));
    missing(&path, Selector::At(at(9, 8)));
    missing(&path, Selector::At(at(17, 4)));
    assert_eq!(found(&path, name("target")).0.anchor, at(16, 5));
}

#[test]
fn anonymous_functions_and_classes_are_labelled_containers() {
    assert_eq!(
        tree(&outline(&fixture("callbacks.ts"))),
        "\
function describe 2:18
function it 3:18
variable suite 4:15
variable tname 5:15
function describe('runs') callback 7:18
  function it(\"starts\") callback 8:16
    variable started 9:11
  function it(`stops ${tname}`) callback 11:24
  function describe(`plain`) callback 12:21
    function it('multi\\line') callback 14:8
function <function> 18:28
function map() callback 19:12
function then() callback 20:24
function namedExpression 21:14
  variable insideNamed 22:9
function <function> 24:13
function <function> 25:2
  function hidden 26:12
variable parenthesized 28:7
  function <function> 28:24
    function insideParenthesized 29:12
variable conditional 31:7
  function <function> 31:37
  function <function> 31:48
function default 32:17
function foo 34:18
class Factory 35:15
  constructor constructor 35:25
"
    );
    assert_eq!(
        tree(&outline(&fixture("decorators.ts"))),
        "\
function component 2:18
function field 3:18
function argument 4:18
function mixin 5:18
class Decorated 9:14
  property decorated 10:18
  method method 11:3
class Shape 14:23
  method area 15:12
  property name 16:12
class default 19:1
  method anonymousMember 20:3
variable Expression 23:7
  method member 24:3
class <class> 26:10
  method registered 27:3
function register 29:18
"
    );
    // Unnamed sites have labels but no keys; their descendants' qualified
    // selectors skip them.
    let callbacks = outline(&fixture("callbacks.ts"));
    let started = &callbacks.entries[6];
    assert_eq!(started.selector.as_deref(), Some("started"));
    assert!(callbacks.entries[4].key.is_none());
    let (declaration, source) = found(&fixture("callbacks.ts"), Selector::At(at(7, 18)));
    assert_eq!(declaration.label, "describe('runs') callback");
    assert!(source.starts_with("() => {\n  it(\"starts\""));
    assert!(source.ends_with("});\n}"));
}

#[test]
fn callback_labels_name_the_callee_as_typescript_does() {
    // `getCallExpressionName` keeps identifiers (decoded), private names,
    // and property-access names, and drops any other left part with the
    // `.` after it: `this`, `super`, a call, an element access, and
    // parentheses. A callee that is itself a call names nothing.
    let path = fixture("callback-chains.ts");
    assert_eq!(
        tree(&outline(&path)),
        "\
variable chain 5:15
function make 6:18
class Base 8:7
  method run 9:3
class Holder 11:7
  property #queue 12:3
  property tasks 13:3
  method start 14:3
    function #queue.push() callback 15:22
    function tasks.add() callback 16:20
    function run() callback 17:15
function chain.a.b() callback 21:11
function b() callback 22:12
function then() callback 23:13
function b() callback 24:11
function chain.e() callback 25:9
function chain.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.aa.bb.cc.dd.ee.ff.gg.hh.ii.jj.kk.ll.mm.nn() callback 26:101
function <function> 27:24
function chain.each('x', `y ${chain}`) callback 28:34
function chain.each() callback 29:22
"
    );
    let (declaration, source) = found(&path, Selector::At(at(15, 22)));
    assert_eq!(declaration.label, "#queue.push() callback");
    assert_eq!(source, "() => {}");
}

#[test]
fn every_binding_is_a_site_whose_excerpt_is_its_declaration_statement() {
    let path = fixture("bindings.ts");
    assert_eq!(
        tree(&outline(&path)),
        "\
variable source 2:15
variable first 3:5
variable second 3:16
  function inSecond 3:42
variable a 4:9
variable renamed 4:15
variable c 4:34
variable others 4:42
variable head 5:8
variable tail 5:16
variable legacy 6:5
variable item 8:12
variable perItem 9:9
variable index 11:10
variable limit 11:21
variable key 12:12
variable failure 15:10
variable handled 16:9
function locals 19:10
  variable shadowed 21:11
  variable shadowed 23:11
variable message 26:17
variable reason 26:33
variable described 27:9
"
    );
    for (selector, excerpt) in [
        (
            "second",
            "let first = 1, second = () => { function inSecond() {} };",
        ),
        (
            "renamed",
            "const { a, b: renamed, nested: { c }, ...others } = source;",
        ),
        (
            "tail",
            "const [head, , tail = () => {}] = [1, 2, () => {}];",
        ),
        ("item", "const item"),
        ("limit", "let index = 0, limit = 2"),
        ("failure", "failure"),
        ("reason", "{ message, cause: reason }"),
        ("legacy", "var legacy;"),
    ] {
        assert_eq!(found(&path, name(selector)).1, excerpt, "{selector}");
    }
    let (second, _) = found(&path, name("second"));
    assert_eq!(second.anchor, at(3, 16));
    assert_eq!(
        second.signature.as_deref(),
        Some("let second = () => { … }")
    );
}

#[test]
fn declarations_merge_nothing_and_dotted_namespaces_are_one_site() {
    let path = fixture("declarations.ts");
    assert_eq!(
        tree(&outline(&path)),
        "\
type Alias 2:13
type Union 3:6
enum Color 5:19
  enum_member Red 6:3
  enum_member Green 7:3
  enum_member \"Light-Blue\" 8:3
interface Merged 11:11
  property first 11:20
interface Merged 12:11
  property second 12:20
namespace NS 14:11
  variable value 15:16
namespace NS 17:11
  function helper 18:19
namespace A.B.C 21:18
  variable deep 22:16
  namespace Inner 23:13
    type T 24:17
class Both 28:7
namespace Both 29:11
  variable extra 30:16
function fnNs 33:10
namespace fnNs 34:11
  variable attached 35:16
module \"ambient-mod\" 38:16
  function ambient 39:19
module global 42:9
  interface Window 43:13
    property navigation 44:5
"
    );
    // Two blocks of one namespace, merged interfaces, and a class with its
    // namespace stay separate sites.
    assert_eq!(candidates(&path, name("NS")), [at(14, 11), at(17, 11)]);
    assert_eq!(candidates(&path, name("Merged")).len(), 2);
    assert_eq!(candidates(&path, name("Both")), [at(28, 7), at(29, 11)]);
    // Qualified selectors are suffixes of containment; a dotted namespace
    // contributes each of its segments.
    for selector in ["deep", "C.deep", "B.C.deep", "A.B.C.deep"] {
        assert_eq!(
            found(&path, name(selector)).0.anchor,
            at(22, 16),
            "{selector}"
        );
    }
    for selector in ["C", "B.C", "A.B.C"] {
        assert_eq!(found(&path, name(selector)).0.label, "A.B.C", "{selector}");
    }
    assert_eq!(found(&path, name("A.B.C.Inner.T")).0.anchor, at(24, 17));
    assert_eq!(found(&path, name("C.Inner.T")).0.anchor, at(24, 17));
    for selector in [
        "A",
        "A.B",
        "A.deep",
        "B.deep",
        "Inner.deep",
        "a.b.c.deep",
        "NS.extra",
    ] {
        missing(&path, name(selector));
    }
    let (namespace, source) = found(&path, name("A.B.C"));
    assert_eq!(namespace.key.as_deref(), Some("A.B.C"));
    assert_eq!(namespace.selector.as_deref(), Some("A.B.C"));
    assert!(source.starts_with("export namespace A.B.C {"));
    // Every segment of the chain selects it by position.
    for column in [18, 20, 22] {
        assert_eq!(found(&path, Selector::At(at(21, column))).0.label, "A.B.C");
    }
    let (global, _) = found(&path, name("global.Window.navigation"));
    assert_eq!(global.kind, Kind::Property);
    let (ambient, _) = found(&path, Selector::At(at(38, 16)));
    assert_eq!(ambient.key.as_deref(), Some("ambient-mod"));
    assert_eq!(ambient.selector.as_deref(), Some("ambient-mod"));
    assert_eq!(
        found(&path, name("ambient-mod.ambient")).0.anchor,
        at(39, 19)
    );
    assert_eq!(ambient.kind, Kind::Module);
}

#[test]
fn a_selector_spells_every_key_segment_that_is_nonempty_and_has_no_dot() {
    // One rule decides both which selectors match and which entries report
    // a `selector`: each segment is non-empty and contains no `.`.
    let source = "\
const settings = { \"display-name\": { inner: 1 }, \"a.b\": { c: 2 }, \"\": { e: 3 }, \" sp ace\": 4 };
namespace A.B.C { export const z = 1; }
declare module \"@scope/pkg\" { export const X: number; }
";
    let file = Fixture::new(&[("selectors.ts", source)]);
    let path = file.path("selectors.ts");
    let entries = outline(&path).entries;
    let selectors: Vec<(&str, Option<&str>)> = entries
        .iter()
        .map(|entry| (entry.label.as_str(), entry.selector.as_deref()))
        .collect();
    assert_eq!(
        selectors,
        [
            ("settings", Some("settings")),
            ("\"display-name\"", Some("settings.display-name")),
            ("inner", Some("settings.display-name.inner")),
            ("\"a.b\"", None),
            ("c", None),
            ("\"\"", None),
            ("e", None),
            ("\" sp ace\"", Some("settings. sp ace")),
            ("A.B.C", Some("A.B.C")),
            ("z", Some("A.B.C.z")),
            ("\"@scope/pkg\"", Some("@scope/pkg")),
            ("X", Some("@scope/pkg.X")),
        ]
    );
    // Every reported selector selects its entry.
    for entry in &entries {
        if let Some(selector) = &entry.selector {
            assert_eq!(found(&path, name(selector)).0, *entry, "{selector}");
        }
    }
    // A dotted namespace's key is its segments: any suffix of the chain
    // that ends at its last segment selects it, and its descendants extend
    // that suffix; a prefix does not.
    for selector in ["C", "B.C", "A.B.C"] {
        assert_eq!(found(&path, name(selector)).0.label, "A.B.C", "{selector}");
    }
    for selector in ["C.z", "B.C.z", "A.B.C.z"] {
        assert_eq!(found(&path, name(selector)).0.label, "z", "{selector}");
    }
    // Empty segments spell nothing, and a dot never matches inside a key.
    for selector in [
        "",
        ".",
        "settings..e",
        "settings.",
        ".settings",
        "..e",
        "a.b",
        "settings.a.b.c",
        "A",
        "A.B",
        "B.z",
        "A.z",
    ] {
        missing(&path, name(selector));
    }
    // Entries without a selector remain selectable by position.
    assert_eq!(found(&path, Selector::At(at(1, 73))).0.label, "e");
    assert_eq!(found(&path, Selector::At(at(1, 59))).0.label, "c");
}

#[test]
fn imports_and_exports_show_their_whole_declarations() {
    let path = fixture("modules.ts");
    assert_eq!(
        tree(&outline(&path)),
        "\
import def 2:8
import alias 2:24
import Only 2:36
import starNs 3:13
import Req 5:8
import Reexported 6:15
variable value 8:7
export exportedValue 9:19
export \"string name\" 9:43
export value 10:10
export OnlyType 13:23
function default 15:1
  variable insideDefault 16:9
"
    );
    let import = "import def, { named as alias, type Only } from \"./other\";";
    assert_eq!(found(&path, name("alias")).1, import);
    assert_eq!(
        found(&path, name("starNs")).1,
        "import * as starNs from \"./other\";"
    );
    assert_eq!(
        found(&path, name("Req")).1,
        "import Req = require(\"./other\");"
    );
    // A default import and a specifier that does not rename declare no
    // name (`computeDeclarationMap`), so they have no key, and a name that
    // misses lists them; their names still select them by position.
    for (selector, anchor) in [("def", at(2, 8)), ("Only", at(2, 36))] {
        assert_eq!(
            show(&path, name(selector)).outcome,
            ShowOutcome::NotFound {
                specifiers: vec![Specifier {
                    kind: Kind::Import,
                    anchor,
                }],
            },
            "{selector}"
        );
        let (import_site, source) = found(&path, Selector::At(anchor));
        assert_eq!((import_site.key, source.as_str()), (None, import));
    }
    assert_eq!(
        found(&path, name("exportedValue")).1,
        "export { value as exportedValue, value as \"string name\" };"
    );
    assert_eq!(found(&path, name("string name")).0.anchor, at(9, 43));
    assert_eq!(
        found(&path, name("Reexported")).1,
        "export import Reexported = Req.inner;"
    );
    assert_eq!(found(&path, name("OnlyType")).0.anchor, at(13, 23));
    // An export site shows itself, not the declaration it exports, and one
    // that does not rename leaves the name to that declaration.
    assert_eq!(found(&path, name("value")).0.anchor, at(8, 7));
    assert_eq!(
        found(&path, Selector::At(at(10, 10))).1,
        "export { value };"
    );
    let (default, source) = found(&path, Selector::At(at(15, 1)));
    assert_eq!(default.label, "default");
    assert_eq!(default.key, None);
    assert!(source.starts_with("export default function () {"));
    let path = fixture("export-equals.ts");
    let entries = outline(&path).entries;
    assert_eq!(entries[1].label, "handler");
    assert_eq!(
        entries[1].key, None,
        "an assigned name labels but does not key"
    );
    assert_eq!(entries[3].label, "export=");
    assert_eq!(entries[3].kind, Kind::Export);
    assert_eq!(found(&path, name("nested.deep")).1, "deep: 1");
}

#[test]
fn keys_are_exact_names_and_labels_never_stand_in_for_them() {
    let path = fixture("names.ts");
    let entries = outline(&path).entries;
    let pairs: Vec<(&str, Option<&str>)> = entries
        .iter()
        .map(|entry| (entry.label.as_str(), entry.key.as_deref()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("café", Some("café")),
            ("__dunder", Some("__dunder")),
            ("bravo", Some("bravo")),
            ("members", Some("members")),
            ("plain", Some("plain")),
            ("\"single-quoted\"", Some("single-quoted")),
            ("\"with\\\"quote\"", Some("with\"quote")),
            ("\"\\uD800\"", None),
            ("\"\\uDBFF\"", None),
            ("\"😀\"", Some("😀")),
            ("\"\u{FFFD}\"", Some("\u{FFFD}")),
            ("\"\u{FFFD}\"", Some("\u{FFFD}")),
            ("\"tab\\there\"", Some("tab\there")),
            ("16", Some("16")),
            ("1000", Some("1000")),
            ("1000", Some("1000")),
            ("0.5", Some("0.5")),
            ("[Symbol.iterator]", None),
            ("\"computed\"", None),
            ("`template`", None),
            ("42", None),
            ("[-1]", None),
            ("Secrets", Some("Secrets")),
            ("#secret", Some("#secret")),
            ("#shared", Some("#shared")),
            ("#hidden", Some("#hidden")),
            ("\"string method\"", Some("string method")),
            ("42", Some("42")),
            ("[Symbol.toPrimitive]", None),
            ("\"café export\"", Some("café export")),
        ]
    );
    // Escapes decode; a literal U+FFFD and its escape name one property,
    // which neither unpaired surrogate joins.
    assert_eq!(found(&path, name("café")).0.anchor, at(2, 7));
    assert_eq!(found(&path, name("bravo")).0.anchor, at(4, 10));
    assert_eq!(candidates(&path, name("\u{FFFD}")), [at(13, 3), at(14, 3)]);
    assert_eq!(found(&path, name("😀")).0.anchor, at(12, 3));
    assert_eq!(found(&path, name("members.16")).0.anchor, at(16, 3));
    assert_eq!(candidates(&path, name("1000")), [at(17, 3), at(18, 3)]);
    assert_eq!(found(&path, name("Secrets.#secret")).0.anchor, at(28, 3));
    assert_eq!(
        found(&path, name("Secrets.string method"))
            .0
            .selector
            .as_deref(),
        Some("Secrets.string method")
    );
    for selector in [
        "computed",
        "\"computed\"",
        "template",
        "Symbol.iterator",
        "-1",
        "\\uD800",
    ] {
        missing(&path, name(selector));
    }
    // Sites without keys are selected by position, each distinctly.
    let (first, source) = found(&path, Selector::At(at(10, 4)));
    assert_eq!(
        (first.label.as_str(), source.as_str()),
        ("\"\\uD800\"", "\"\\uD800\": 4")
    );
    let (second, _) = found(&path, Selector::At(at(11, 3)));
    assert_eq!(second.label, "\"\\uDBFF\"");
    assert_eq!(
        found(&path, Selector::At(at(20, 10))).0.label,
        "[Symbol.iterator]"
    );
}

#[test]
fn positions_count_utf16_columns_after_every_line_break() {
    // CR, LF, CRLF, U+2028, and U+2029 each end a line; a BOM, tab, NBSP,
    // and astral character precede declarations on their lines. The leading
    // BOM is not text (see `a_leading_byte_order_mark_is_not_text`), so
    // positions count from the character after it.
    let file_source = "\u{FEFF}const a = 1;\rconst b = 2;\nconst c = 3;\r\nconst d = 4;\u{2028}const e = 5;\u{2029}\tconst f = 6;\n\u{A0}/* 😀 */ function g() {}\n";
    let source = file_source.strip_prefix('\u{FEFF}').unwrap();
    let file = Fixture::new(&[("breaks.ts", file_source)]);
    let path = file.path("breaks.ts");
    let entries = outline(&path).entries;
    assert_eq!(entries.len(), 7);
    for (entry, label) in entries.iter().zip(["a", "b", "c", "d", "e", "f", "g"]) {
        assert_eq!(entry.label, label);
        let keyword = if label == "g" { "function " } else { "const " };
        let start = source.find(&format!("{keyword}{label}")).unwrap();
        let name = start + keyword.len();
        assert_eq!(entry.anchor, position(source, name), "{label}");
        assert_eq!(entry.range.start, position(source, start), "{label}");
        // Ranges are exclusive: the end follows the last character.
        let end = source[name..].find([';', '}']).unwrap() + name + 1;
        assert_eq!(entry.range.end, position(source, end), "{label}");
        assert_eq!(found(&path, Selector::At(entry.anchor)).0.label, label);
    }
    assert_eq!(entries[0].anchor, at(1, 7), "tsc reports `a` at 1:7");
    assert_eq!(entries[4].anchor, at(5, 7));
    assert_eq!(entries[5].anchor, at(6, 8), "a tab is one column");
    assert_eq!(
        entries[6].anchor,
        at(7, 20),
        "NBSP and the comment's astral character"
    );
    assert_eq!(entries[6].range.start, at(7, 11));
    let (g, excerpt) = found(&path, name("g"));
    assert_eq!(excerpt, "function g() {}");
    assert_eq!(g.signature.as_deref(), Some("function g()"));
    // The checked-in fixture's document symbols, pinned from the compiler
    // and converted from the protocol's lines (CR, LF, and CRLF only) to
    // ECMAScript's.
    let path = fixture("line-breaks.ts");
    assert_eq!(
        tree(&outline(&path)),
        "\
variable cr 2:7
variable lf 3:7
variable crlf 4:7
variable ls 5:7
variable ps 6:7
function spans 7:10
  variable inner 8:9
class After 11:14
"
    );
    assert_eq!(found(&path, name("spans")).0.range.end, at(10, 2));
}

#[test]
fn a_leading_byte_order_mark_is_not_text() {
    // TypeScript's `decodeBytes` drops a leading UTF-8 byte order mark when
    // it reads a file. For `\u{FEFF}const a: number = "x";` tsc 7.0.2
    // reports TS2322 at (1,7), and for `\u{FEFF}const = 1;` TS1134 at
    // (1,7); with or without the mark, every result is the same.
    for prefix in ["", "\u{FEFF}"] {
        let file = Fixture::new(&[
            (
                "typed.ts",
                &format!("{prefix}const a: number = \"x\"; function f() {{}}\nconst b = 2;\n"),
            ),
            ("broken.ts", &format!("{prefix}const = 1;\n")),
            (
                "shebang.ts",
                &format!("{prefix}#!/usr/bin/env node\nconst c = 3;\n"),
            ),
            ("only.ts", prefix),
        ]);
        let path = file.path("typed.ts");
        assert_eq!(
            tree(&outline(&path)),
            "variable a 1:7\nfunction f 1:33\nvariable b 2:7\n",
            "{prefix:?}"
        );
        let (a, source) = found(&path, Selector::At(at(1, 7)));
        assert_eq!(a.label, "a");
        assert_eq!(
            a.range,
            navigation::SourceRange {
                start: at(1, 1),
                end: at(1, 23)
            }
        );
        assert_eq!(source, "const a: number = \"x\";");
        assert_eq!(a.signature.as_deref(), Some("const a: number = \"x\""));
        let (f, source) = found(&path, name("f"));
        assert_eq!((f.anchor, source.as_str()), (at(1, 33), "function f() {}"));
        // The column after a name is outside it.
        missing(&path, Selector::At(at(1, 8)));
        missing(&path, Selector::At(at(1, 34)));
        // The first line has as many columns as its text.
        let error = query_error(navigation::show_file(&path, &Selector::At(at(1, 40))));
        assert_eq!(error.kind, QueryErrorKind::InvalidPosition, "{prefix:?}");
        missing(&path, Selector::At(at(1, 39)));

        let error = input_error(navigation::outline_file(&file.path("broken.ts")));
        assert_eq!(error.kind, InputErrorKind::Syntax);
        assert_eq!(error.position, Some(at(1, 7)), "{prefix:?}");

        assert_eq!(tree(&outline(&file.path("shebang.ts"))), "variable c 2:7\n");
        assert_eq!(outline(&file.path("only.ts")).entries, []);
    }
    // Only one leading mark is dropped; any other is text (whitespace) that
    // occupies a column: tsc reports TS2322 at (1,8) and (1,31) here.
    let file = Fixture::new(&[(
        "twice.ts",
        "\u{FEFF}\u{FEFF}const a: number = \"x\";\u{FEFF}const b: number = \"y\";\n",
    )]);
    assert_eq!(
        tree(&outline(&file.path("twice.ts"))),
        "variable a 1:8\nvariable b 1:31\n"
    );
    // The checked-in fixture's document symbols, pinned from the compiler.
    let path = fixture("bom.ts");
    assert_eq!(
        tree(&outline(&path)),
        "variable first 1:14\nfunction second 1:41\n  variable inner 1:58\nclass Third 4:14\n"
    );
    assert_eq!(found(&path, name("second.inner")).1, "const inner = 2;");
}

#[test]
fn invalid_positions_are_errors_and_valid_ones_without_a_site_are_not_found() {
    // U+1D465, an identifier start outside the BMP, takes two UTF-16 units.
    let file = Fixture::new(&[("astral.ts", "const \u{1D465}y = 1;\nfunction f() {}\n")]);
    let path = file.path("astral.ts");
    for (position, message) in [
        (at(0, 1), "line is outside the file"),
        (at(4, 1), "line is outside the file"),
        (at(1, 0), "column is outside the line"),
        (at(2, 17), "column is outside the line"),
        (at(1, 8), "column is inside a surrogate pair"),
    ] {
        let error = query_error(navigation::show_file(&path, &Selector::At(position)));
        assert_eq!(error.kind, QueryErrorKind::InvalidPosition, "{position:?}");
        assert_eq!(error.message, message);
        assert_eq!(error.position, Some(position));
        assert_eq!(error.path.as_deref(), Some(path.as_path()));
    }
    // Column 9 begins `y`, after the pair's two units, within the name.
    assert_eq!(found(&path, Selector::At(at(1, 7))).0.label, "\u{1D465}y");
    assert_eq!(found(&path, Selector::At(at(1, 9))).0.label, "\u{1D465}y");
    // The end of a name is outside it; the end of a line is a valid position.
    missing(&path, Selector::At(at(1, 10)));
    missing(&path, Selector::At(at(2, 16)));
    missing(&path, Selector::At(at(3, 1)));
}

#[test]
fn ambiguity_returns_every_candidate_whose_anchor_selects_it() {
    for (file, selector, count) in [
        ("factory.ts", "method", 3),
        ("factory.ts", "Widget.size", 2),
        ("bindings.ts", "locals.shadowed", 2),
        ("globals.d.ts", "pkg.run", 2),
    ] {
        let path = fixture(file);
        let shown = show(&path, name(selector));
        // The result crosses a process boundary as JSON.
        let json = serde_json::to_string(&shown).unwrap();
        let shown: Show = serde_json::from_str(&json).unwrap();
        let ShowOutcome::Ambiguous(candidates) = shown.outcome else {
            panic!("{selector}: {json}");
        };
        assert_eq!(candidates.len(), count, "{selector}");
        let mut sources = Vec::new();
        for candidate in &candidates {
            assert!(candidate.selector.as_deref().unwrap().ends_with(selector));
            let (declaration, source) =
                found(Path::new(&candidate.file), Selector::At(candidate.anchor));
            assert_eq!(declaration.anchor, candidate.anchor);
            assert_eq!(declaration.label, candidate.label);
            sources.push(source);
        }
        sources.dedup();
        assert_eq!(sources.len(), count, "each candidate is a distinct site");
    }
    let path = fixture("factory.ts");
    assert_eq!(
        found(&path, Selector::At(at(37, 3))).1,
        "method(value: number): void;"
    );
}

#[test]
fn selection_by_position_accepts_names_and_anchors_only() {
    let path = fixture("factory.ts");
    // The constructor keyword, a parameter property's name, a callback's
    // anchor, and the last character of a name.
    assert_eq!(
        found(&path, Selector::At(at(39, 3))).0.kind,
        Kind::Constructor
    );
    assert_eq!(
        found(&path, Selector::At(at(39, 13))).0.kind,
        Kind::Constructor
    );
    assert_eq!(found(&path, Selector::At(at(39, 33))).0.label, "id");
    assert_eq!(
        found(&path, Selector::At(at(25, 22))).0.label,
        "wrap() callback"
    );
    assert_eq!(
        found(&path, Selector::At(at(5, 29))).0.label,
        "createRunHost"
    );
    // Inside a declaration but not on its name or a statement's first
    // token: no selection.
    for position in [at(5, 30), at(39, 22), at(26, 17)] {
        missing(&path, Selector::At(position));
    }
    // A statement's first token that no entry claims selects the statement.
    for (position, source) in [
        (at(5, 1), "export function createRunHost("),
        (at(26, 3), "const inner = 1;"),
    ] {
        let (declaration, excerpt) = found(&path, Selector::At(position));
        assert_eq!(declaration.kind.as_str(), "statement", "{position:?}");
        assert!(excerpt.starts_with(source), "{excerpt}");
    }
    missing(&path, name("absent"));
    missing(&path, name("createRunHost.absent"));
    missing(&path, name("Widget.startEligible"));
    missing(&path, name(""));
}

#[test]
fn excerpts_include_decorators_and_modifiers_but_not_leading_comments() {
    let path = fixture("decorators.ts");
    let (class, source) = found(&path, name("Decorated"));
    assert!(
        source.starts_with("@component({ selector: 'app-root',"),
        "{source}"
    );
    assert!(source.ends_with("method(@argument value: string) {}\n}"));
    assert_eq!(class.range.start, at(8, 1));
    assert_eq!(class.anchor, at(9, 14));
    assert_eq!(
        found(&path, name("decorated")).1,
        "@field declare decorated: string;"
    );
    assert_eq!(
        class.signature.as_deref(),
        Some(
            "@component({ selector: 'app-root', setup: () => { … } }) export class Decorated extends mixin(class Base { … })"
        )
    );
}

#[test]
fn signatures_summarize_written_syntax_without_altering_tokens() {
    let long = (0..40)
        .map(|n| format!("item{n}"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "export async function run<T>(/* note */ input: T,\n    flag = \"a  b\"): Promise<T> {{ return input; }}\n\
         const sql = `select\n  *`;\n\
         const list = [{long}];\n\
         class Box<T> extends Base<T> implements Shape {{ value = 1; }}\n\
         const handler = {{ onClick(event: Event) {{ event.stopPropagation(); }} }};\n"
    );
    let file = Fixture::new(&[("signatures.ts", &source)]);
    let path = file.path("signatures.ts");
    let signatures: Vec<String> = outline(&path)
        .entries
        .into_iter()
        .filter(|entry| entry.depth == 1)
        .map(|entry| entry.signature.unwrap())
        .collect();
    assert_eq!(
        signatures[0],
        "export async function run<T>( input: T, flag = \"a  b\"): Promise<T>"
    );
    assert_eq!(signatures[1], "const sql = …");
    assert!(signatures[2].starts_with("const list = [item0, item1,"));
    assert!(signatures[2].ends_with('…'));
    assert!(signatures[2].chars().count() <= 161);
    let tail = signatures[2]
        .trim_end_matches('…')
        .trim_end_matches([',', ' ']);
    assert!(
        source.contains(tail),
        "abbreviation ends at a token boundary: {tail}"
    );
    assert_eq!(
        signatures[3],
        "class Box<T> extends Base<T> implements Shape"
    );
    assert_eq!(
        signatures[4],
        "const handler = { onClick(event: Event) { … } }"
    );
}

#[test]
fn abbreviation_keeps_every_retained_literal_whole_including_trailing_whitespace() {
    // Signatures abbreviate past 160 characters at the last token boundary.
    // A JSX text, string, or template token that the signature retains keeps
    // its trailing space, tab, or NBSP; only whitespace between tokens is
    // generated, and it never precedes the `…`.
    for space in [" ", "\t", "\u{A0}"] {
        // Lengths of the whole signature: under, at, and over the limit,
        // and far enough over that the literal itself crosses it.
        for length in [159, 160, 161, 174] {
            let text = format!("{}{space}{space}", "x".repeat(length - 24 - 2));
            let jsx = format!("const view = <div>{text}</div>");
            assert_eq!(jsx.chars().count(), length);
            let expected = if length <= 160 {
                jsx.clone()
            } else {
                format!("const view = <div>{text}…")
            };
            let mut files = vec![("view.tsx".to_owned(), format!("{jsx};\n"), expected)];
            for quote in ["\"", "'", "`"] {
                let literal = format!(
                    "{quote}{}{space}{space}{quote}",
                    "x".repeat(length - 19 - 4)
                );
                let whole = format!("const s = {literal} as const");
                assert_eq!(whole.chars().count(), length);
                let expected = match length {
                    ..=160 => whole.clone(),
                    161 => format!("const s = {literal} as…"),
                    _ => format!("const s = {literal}…"),
                };
                files.push((
                    format!("literal{}.ts", files.len()),
                    format!("{whole};\n"),
                    expected,
                ));
            }
            let fixture = Fixture::new(
                &files
                    .iter()
                    .map(|(name, source, _)| (name.as_str(), source.as_str()))
                    .collect::<Vec<_>>(),
            );
            for (name, source, expected) in &files {
                let path = fixture.path(name);
                let entries = outline(&path).entries;
                assert_eq!(
                    entries[0].signature.as_deref(),
                    Some(expected.as_str()),
                    "{length} {space:?} {name}"
                );
                // Show keeps the exact source whatever the signature shows.
                let (_, excerpt) = found(&path, Selector::At(entries[0].anchor));
                assert_eq!(format!("{excerpt}\n"), *source, "{length} {space:?} {name}");
            }
        }
    }
}

#[test]
fn outlines_cover_every_kind_across_ts_tsx_and_declaration_files() {
    let mut kinds = Vec::new();
    for name in [
        "factory.ts",
        "declarations.ts",
        "modules.ts",
        "export-equals.ts",
        "component.tsx",
        "globals.d.ts",
    ] {
        kinds.extend(
            outline(&fixture(name))
                .entries
                .iter()
                .map(|entry| entry.kind),
        );
    }
    kinds.sort_by_key(|kind| kind.as_str());
    kinds.dedup();
    let mut all = vec![
        Kind::Function,
        Kind::Variable,
        Kind::Class,
        Kind::Interface,
        Kind::Type,
        Kind::Enum,
        Kind::Namespace,
        Kind::Module,
        Kind::Method,
        Kind::Accessor,
        Kind::Constructor,
        Kind::Signature,
        Kind::Property,
        Kind::EnumMember,
        Kind::Import,
        Kind::Export,
    ];
    all.sort_by_key(|kind| kind.as_str());
    assert_eq!(kinds, all);
    assert_eq!(
        tree(&outline(&fixture("component.tsx"))),
        "\
variable items 2:15
function useState 3:18
function List 5:17
  variable selected 6:10
  variable setSelected 6:20
  variable onSelect 7:9
  function <function> 12:24
  function items.map() callback 13:18
    function <function> 14:35
variable Inline 22:14
"
    );
}

#[test]
fn navigation_parses_with_the_deep_budget_whatever_the_display_depth() {
    // Navigation shares project loading's budget of 1,024 grammar entries:
    // 204 nested functions are the most it accepts, and parse mode's default
    // budget rejects far fewer.
    let nested =
        |count: usize| format!("{}{}\n", "function f() {".repeat(count), "}".repeat(count));
    assert!(typescript_parser::parse(&nested(60)).is_err());
    let file = Fixture::new(&[
        ("under.ts", &nested(203)),
        ("at.ts", &nested(204)),
        ("over.ts", &nested(205)),
    ]);
    for (name, depth) in [("under.ts", 203), ("at.ts", 204)] {
        let entries = outline(&file.path(name)).entries;
        assert_eq!(entries.len(), depth, "{name}");
        assert_eq!(entries.last().unwrap().depth, depth, "{name}");
        assert_eq!(
            entries.last().unwrap().selector.as_deref(),
            Some(vec!["f"; depth].join(".").as_str())
        );
    }
    let error = input_error(navigation::outline_file(&file.path("over.ts")));
    assert_eq!(error.kind, InputErrorKind::ResourceLimit);
    assert_eq!(error.message, "Maximum syntax nesting depth exceeded");
    assert!(error.position.is_some());
    let error = input_error(navigation::show_file(&file.path("over.ts"), &name("f")));
    assert_eq!(error.kind, InputErrorKind::ResourceLimit);
}

#[test]
fn long_flat_inputs_stay_within_the_budget() {
    let functions: String = (0..20_000)
        .map(|n| format!("function f{n}() {{}}\n"))
        .collect();
    let array = format!("const values = [{}];\n", "0, ".repeat(200_000));
    let file = Fixture::new(&[("functions.ts", &functions), ("array.ts", &array)]);
    let entries = outline(&file.path("functions.ts")).entries;
    assert_eq!(entries.len(), 20_000);
    assert_eq!(
        found(&file.path("functions.ts"), name("f19999")).0.anchor,
        at(20_000, 10)
    );
    let entries = outline(&file.path("array.ts")).entries;
    assert_eq!(entries.len(), 1);
    assert!(entries[0].signature.as_ref().unwrap().ends_with('…'));
}

#[test]
fn signatures_render_only_as_far_as_they_reach() {
    // Each of 250 nested properties is a site whose signature ends shortly
    // after its name, so rendering it must not visit the rest of its
    // initializer. When every signature searched its whole declaration for
    // bodies to elide, the 200,000-element array inside them all cost an
    // unoptimized build about 19 seconds; projection is linear in the tree.
    let (depth, items) = (250, 200_000);
    let source = format!(
        "export const a = {}[{}]{};\n",
        "{ a: ".repeat(depth),
        "1, ".repeat(items),
        " }".repeat(depth)
    );
    let file = Fixture::new(&[("nested.ts", &source)]);
    let start = std::time::Instant::now();
    let entries = outline(&file.path("nested.ts")).entries;
    let elapsed = start.elapsed();
    assert_eq!(entries.len(), depth + 1);
    let innermost = entries.last().unwrap();
    assert_eq!(innermost.depth, depth + 1);
    let signature = innermost.signature.as_deref().unwrap();
    assert!(signature.starts_with("a: [1, 1, 1,"), "{signature}");
    assert!(signature.ends_with("1, 1,…"), "{signature}");
    assert!(signature.chars().count() <= 162, "{signature}");
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "outline took {elapsed:?}"
    );
}

#[test]
fn empty_files_have_empty_outlines() {
    let file = Fixture::new(&[
        ("empty.ts", ""),
        ("trivia.ts", "\u{FEFF}// only\n/* comments */\n"),
    ]);
    for name in ["empty.ts", "trivia.ts"] {
        let outline = outline(&file.path(name));
        assert_eq!(outline.entries, []);
        assert_eq!(outline.file, file.path(name));
    }
    missing(&file.path("empty.ts"), name("x"));
    missing(&file.path("empty.ts"), Selector::At(at(1, 1)));
}

#[test]
fn inputs_outside_typescript_and_unreadable_files_are_typed_failures() {
    let file = Fixture::new(&[
        ("data.json", "{ \"a\": 1 }\n"),
        ("script.js", "function f() {}\n"),
        ("notes.txt", "function f() {}\n"),
        ("dir.ts/inner.ts", ""),
    ]);
    for (name, kind) in [
        ("data.json", InputErrorKind::Unsupported),
        ("script.js", InputErrorKind::Unsupported),
        ("notes.txt", InputErrorKind::Unsupported),
        ("missing.ts", InputErrorKind::Io),
        ("dir.ts", InputErrorKind::Io),
    ] {
        let error = input_error(navigation::outline_file(&file.path(name)));
        assert_eq!(error.kind, kind, "{name}: {error}");
        assert_eq!(error.path.as_deref(), Some(file.path(name).as_path()));
    }
    let error = input_error(navigation::outline_file(&file.path("data.json")));
    assert_eq!(error.message, "JSON files are not navigation input");
}

#[test]
fn relative_paths_resolve_against_the_working_directory() {
    let relative = Path::new("fixtures/navigation/bindings.ts");
    let outline = outline(relative);
    assert!(Path::new(&outline.file).is_absolute());
    assert!(Path::new(&outline.file).ends_with(relative));
}

/// Each kind of statement, at its first token, and the tokens of its
/// syntax that are no statement's first token.
const STATEMENTS: &str = r#""use strict";
label: inner: f();
if (a) if (b) g(); else h();
@dec export class C {
  static {
    s();
  }
}
export default x;
export { y };
namespace A.B {
  k();
}
for (const i of [1]) continue;
switch (v) {
  case 1:
    m();
  default:
    n();
}
try {
  p();
} catch {
  q();
} finally {
  r();
}
/a/.test(t);
;
{
  u();
}
function outer() {
  return () => {
    deep();
  };
}
const w = 1;
do ; while (w)
while (w) break;
throw w;
debugger;
interface I {}
type T = 1;
enum E { A }
declare module "m" {}
import z = require("m");
export * from "m";
export const e1 = 1;
export function ef() {}
for (let j = 0; j < 1; j++) fj();
for (const key in o) fk();
with (o) wk();
export as namespace NS;
export import ea = A.B;
import { im } from "m";
/=a/.test(t);
@wrap(() => { dw(); }) class W {
  @mark(() => { dm(); }) method(@param(() => { dp(); }) x = (() => { di(); })()) { db(); }
}
"#;

/// The position of `needle`'s first occurrence in `text`, plus `offset`
/// bytes.
fn at_text(text: &str, needle: &str, offset: usize) -> SourcePosition {
    let start = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} is not in the text"));
    position(text, start + offset)
}

/// The first token of these statements: an identifier or keyword, a
/// string, or one punctuator.
fn first_token(source: &str) -> &str {
    let word = source
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(source.len());
    if word > 0 {
        &source[..word]
    } else if let Some(rest) = source.strip_prefix('"') {
        &source[..rest.find('"').unwrap() + 2]
    } else {
        &source[..1]
    }
}

#[test]
fn a_statement_s_first_token_selects_the_statement_when_no_entry_is_there() {
    let file = Fixture::new(&[("statements.ts", STATEMENTS)]);
    let path = file.path("statements.ts");
    for (needle, offset, label, source, depth) in [
        ("\"use strict\"", 0, "\"use strict\"", "\"use strict\";", 1),
        ("label:", 0, "label: inner: f()", "label: inner: f();", 1),
        ("inner:", 0, "inner: f()", "inner: f();", 1),
        ("f();", 0, "f()", "f();", 1),
        (
            "if (a)",
            0,
            "if (a) if (b) g(); else h()",
            "if (a) if (b) g(); else h();",
            1,
        ),
        (
            "if (b)",
            0,
            "if (b) g(); else h()",
            "if (b) g(); else h();",
            1,
        ),
        ("g();", 0, "g()", "g();", 1),
        ("h();", 0, "h()", "h();", 1),
        (
            "@dec",
            0,
            "@dec export class C { … }",
            "@dec export class C {\n  static {\n    s();\n  }\n}",
            1,
        ),
        // A class static block's body, inside the class entry.
        ("{\n    s();", 0, "{ s(); }", "{\n    s();\n  }", 2),
        ("s();", 0, "s()", "s();", 2),
        (
            "export default x",
            0,
            "export default x",
            "export default x;",
            1,
        ),
        ("export { y }", 0, "export { y }", "export { y };", 1),
        (
            "namespace A.B",
            0,
            "namespace A.B { k(); }",
            "namespace A.B {\n  k();\n}",
            1,
        ),
        ("k();", 0, "k()", "k();", 2),
        (
            "for (",
            0,
            "for (const i of [1]) continue",
            "for (const i of [1]) continue;",
            1,
        ),
        ("continue", 0, "continue", "continue;", 1),
        (
            "switch",
            0,
            "switch (v) { case 1: m(); default: n(); }",
            "switch (v) {\n  case 1:\n    m();\n  default:\n    n();\n}",
            1,
        ),
        ("m();", 0, "m()", "m();", 1),
        ("n();", 0, "n()", "n();", 1),
        (
            "try {",
            0,
            "try { p(); } catch { q(); } finally { r(); }",
            "try {\n  p();\n} catch {\n  q();\n} finally {\n  r();\n}",
            1,
        ),
        ("p();", 0, "p()", "p();", 1),
        ("q();", 0, "q()", "q();", 1),
        ("r();", 0, "r()", "r();", 1),
        // The lexer scans `/` there, without the parser's rescan.
        ("/a/", 0, "/a/.test(t)", "/a/.test(t);", 1),
        // An empty statement's text without its `;` is empty.
        ("\n;\n", 1, "", ";", 1),
        ("{\n  u();", 0, "{ u(); }", "{\n  u();\n}", 1),
        ("u();", 0, "u()", "u();", 1),
        // The function entry has the statement's range, so it adds no level.
        (
            "function outer",
            0,
            "function outer() { … }",
            "function outer() {\n  return () => {\n    deep();\n  };\n}",
            1,
        ),
        (
            "return () =>",
            0,
            "return () => { … }",
            "return () => {\n    deep();\n  };",
            2,
        ),
        ("deep();", 0, "deep()", "deep();", 3),
        ("const w", 0, "const w = 1", "const w = 1;", 1),
        ("do ;", 0, "do ; while (w)", "do ; while (w)", 1),
        ("do ;", 3, "", ";", 1),
        (
            "while (w) break",
            0,
            "while (w) break",
            "while (w) break;",
            1,
        ),
        ("break", 0, "break", "break;", 1),
        ("throw w", 0, "throw w", "throw w;", 1),
        ("debugger", 0, "debugger", "debugger;", 1),
        ("interface I", 0, "interface I {}", "interface I {}", 1),
        ("type T", 0, "type T = 1", "type T = 1;", 1),
        ("enum E", 0, "enum E { A }", "enum E { A }", 1),
        (
            "declare module",
            0,
            "declare module \"m\" {}",
            "declare module \"m\" {}",
            1,
        ),
        (
            "import z",
            0,
            "import z = require(\"m\")",
            "import z = require(\"m\");",
            1,
        ),
        (
            "export * from",
            0,
            "export * from \"m\"",
            "export * from \"m\";",
            1,
        ),
        (
            "export const e1",
            0,
            "export const e1 = 1",
            "export const e1 = 1;",
            1,
        ),
        (
            "export function ef",
            0,
            "export function ef() { … }",
            "export function ef() {}",
            1,
        ),
        (
            "for (let j",
            0,
            "for (let j = 0; j < 1; j++) fj()",
            "for (let j = 0; j < 1; j++) fj();",
            1,
        ),
        ("fj();", 0, "fj()", "fj();", 1),
        (
            "for (const key",
            0,
            "for (const key in o) fk()",
            "for (const key in o) fk();",
            1,
        ),
        ("fk();", 0, "fk()", "fk();", 1),
        ("with (o)", 0, "with (o) wk()", "with (o) wk();", 1),
        ("wk();", 0, "wk()", "wk();", 1),
        (
            "export as namespace",
            0,
            "export as namespace NS",
            "export as namespace NS;",
            1,
        ),
        (
            "export import ea",
            0,
            "export import ea = A.B",
            "export import ea = A.B;",
            1,
        ),
        (
            "import { im }",
            0,
            "import { im } from \"m\"",
            "import { im } from \"m\";",
            1,
        ),
        // The lexer scans `/=` there, without the parser's rescan.
        ("/=a/", 0, "/=a/.test(t)", "/=a/.test(t);", 1),
        (
            "@wrap",
            0,
            "@wrap(() => { … }) class W { … }",
            "@wrap(() => { dw(); }) class W {\n  @mark(() => { dm(); }) method(@param(() => { dp(); }) x = (() => { di(); })()) { db(); }\n}",
            1,
        ),
        // Statements in decorators, a parameter's initializer, and a body,
        // in source order though the parser stores modifiers last.
        ("dw();", 0, "dw()", "dw();", 2),
        ("dm();", 0, "dm()", "dm();", 3),
        ("dp();", 0, "dp()", "dp();", 3),
        ("di();", 0, "di()", "di();", 3),
        ("db();", 0, "db()", "db();", 3),
    ] {
        let start = at_text(STATEMENTS, needle, offset);
        // Every position in the first token selects it: here its first and
        // last characters.
        let last = at_text(STATEMENTS, needle, offset + first_token(source).len() - 1);
        for position in [start, last] {
            let (declaration, excerpt) = found(&path, Selector::At(position));
            assert_eq!(
                declaration.kind.as_str(),
                "statement",
                "{needle:?} at {position:?}"
            );
            assert_eq!(declaration.label, label, "{needle:?}");
            assert_eq!(excerpt, source, "{needle:?}");
            assert_eq!(declaration.anchor, start, "{needle:?}");
            assert_eq!(declaration.range.start, start, "{needle:?}");
            assert_eq!(declaration.depth, depth, "{needle:?}");
            assert_eq!(
                (
                    &declaration.key,
                    &declaration.selector,
                    &declaration.signature
                ),
                (&None, &None, &None),
                "{needle:?}"
            );
        }
    }
    // Tokens that begin no statement: a decorator's name and the modifiers
    // and keywords after `@dec`, `static`, a clause keyword, the inside of a
    // regular expression, a loop header's declarations, and the keyword
    // after `export`.
    for (needle, offset) in [
        ("@dec", 1),
        ("export class C", 0),
        ("class C", 0),
        ("static {", 0),
        ("case 1", 0),
        ("default:", 0),
        ("catch {", 0),
        ("finally {", 0),
        ("/a/", 1),
        ("const i", 0),
        ("export function ef", 7),
        ("for (let j", 4),
        ("let j", 0),
        ("const key", 0),
        ("with (o)", 5),
        ("export as namespace", 7),
        ("export import ea", 7),
        // Past `/=`, the first token, inside the regular expression.
        ("/=a/", 2),
    ] {
        missing(&path, Selector::At(at_text(STATEMENTS, needle, offset)));
    }
    // `/=` is one token: its `=` selects the statement too.
    assert_eq!(
        found(&path, Selector::At(at_text(STATEMENTS, "/=a/", 1)))
            .0
            .label,
        "/=a/.test(t)"
    );
    // Names select their entries, as before.
    for (needle, kind) in [
        ("C {", "class"),
        ("B {", "namespace"),
        ("y }", "export"),
        ("i of", "variable"),
        ("outer", "function"),
        ("() => {\n    deep", "function"),
        ("\"m\" {}", "module"),
        ("z =", "import"),
        ("ef()", "function"),
    ] {
        let (declaration, _) = found(&path, Selector::At(at_text(STATEMENTS, needle, 0)));
        assert_eq!(declaration.kind.as_str(), kind, "{needle:?}");
    }
    // Statement sites are never outline entries.
    assert!(
        outline(&path)
            .entries
            .iter()
            .all(|entry| entry.kind.as_str() != "statement")
    );
}

#[test]
fn statement_sites_begin_after_every_line_break_and_kind_of_trivia() {
    // CR, LF, CRLF, U+2028, and U+2029 each end a line; a leading BOM, a
    // tab, NBSP, a space, a block comment with an astral character, a line
    // comment, and JSDoc precede statements and are never their first
    // tokens. Columns count UTF-16 units from the character after the BOM.
    let file_source = "\u{FEFF}a();\rb();\nc();\r\nd();\u{2028}e();\u{2029}\tf();\n\u{A0}/* 😀 */ g();\n // line\n h();\n/** doc */\ni();\n";
    let shebang = "#!/usr/bin/env node\nrun();\n";
    let file = Fixture::new(&[("breaks.ts", file_source), ("shebang.ts", shebang)]);
    let path = file.path("breaks.ts");
    for (label, line, column) in [
        ("a", 1, 1),
        ("b", 2, 1),
        ("c", 3, 1),
        ("d", 4, 1),
        ("e", 5, 1),
        ("f", 6, 2),
        ("g", 7, 11),
        ("h", 9, 2),
        ("i", 11, 1),
    ] {
        let (declaration, excerpt) = found(&path, Selector::At(at(line, column)));
        assert_eq!(declaration.kind.as_str(), "statement", "{label}");
        assert_eq!(declaration.label, format!("{label}()"));
        assert_eq!(excerpt, format!("{label}();"));
        assert_eq!(declaration.anchor, at(line, column), "{label}");
        assert_eq!(declaration.range.start, at(line, column), "{label}");
        assert_eq!(declaration.range.end, at(line, column + 4), "{label}");
    }
    // The trivia before them: the tab, NBSP, the block comment, the line
    // comment's indentation and text, and JSDoc.
    for (line, column) in [
        (6, 1),
        (7, 1),
        (7, 2),
        (7, 5),
        (8, 1),
        (8, 2),
        (9, 1),
        (10, 1),
    ] {
        missing(&path, Selector::At(at(line, column)));
    }
    // A shebang line is trivia too.
    let path = file.path("shebang.ts");
    assert_eq!(found(&path, Selector::At(at(2, 1))).0.label, "run()");
    missing(&path, Selector::At(at(1, 1)));
}

#[test]
fn a_jsx_statement_s_first_token_is_its_opening_angle_bracket() {
    // The lexer scans `<` there, without the parser's rescan.
    let file = Fixture::new(&[("view.tsx", "const view = 1;\n<div>{view}</div>;\n")]);
    let path = file.path("view.tsx");
    let (declaration, excerpt) = found(&path, Selector::At(at(2, 1)));
    assert_eq!(declaration.kind.as_str(), "statement");
    assert_eq!(declaration.label, "<div>{view}</div>");
    assert_eq!(excerpt, "<div>{view}</div>;");
    missing(&path, Selector::At(at(2, 2)));
}

#[test]
fn an_entry_at_a_statement_s_first_token_keeps_the_position() {
    let file = Fixture::new(&[
        (
            "precedence.ts",
            "x => {};\nasync () => {};\nexport default function () {}\nclass K { [(() => { cf(); })()]() {} }\n",
        ),
        ("equals.ts", "export = f(() => {});\n"),
    ]);
    let path = file.path("precedence.ts");
    // An arrow's entry is anchored at the statement's first token.
    for position in [at(1, 1), at(2, 1)] {
        let (declaration, _) = found(&path, Selector::At(position));
        assert_eq!(
            (declaration.kind, declaration.label.as_str()),
            (Kind::Function, "<function>")
        );
    }
    let (declaration, source) = found(&path, Selector::At(at(3, 1)));
    assert_eq!(
        (declaration.kind, declaration.label.as_str()),
        (Kind::Function, "default")
    );
    assert_eq!(source, "export default function () {}");
    // A statement inside a computed name lies in the member's name token.
    let (declaration, _) = found(&path, Selector::At(at(4, 21)));
    assert_eq!(declaration.kind, Kind::Method);
    let (declaration, source) = found(&file.path("equals.ts"), Selector::At(at(1, 1)));
    assert_eq!(
        (declaration.kind, declaration.label.as_str()),
        (Kind::Export, "export=")
    );
    assert_eq!(source, "export = f(() => {});");
}

#[test]
fn a_statement_label_abbreviates_only_past_the_signature_limit() {
    // `f("…")` around a string of `length - 5` characters is `length`
    // characters long; the string is one literal, with a boundary on each
    // side.
    let call = |length: usize| format!("f(\"{}\")", "a".repeat(length - 5));
    let source = format!(
        "{};\n{};\n{};\na{};\n",
        call(159),
        call(160),
        call(161),
        ".b()".repeat(50),
    );
    let file = Fixture::new(&[("labels.ts", &source)]);
    let path = file.path("labels.ts");
    let label = |line: usize| found(&path, Selector::At(at(line, 1))).0.label;
    assert_eq!(label(1), call(159));
    assert_eq!(label(2), call(160));
    // Over the limit, the text is cut at its last boundary, after the
    // string.
    assert_eq!(label(3), format!("f(\"{}\"…", "a".repeat(156)));
    // No boundary in the first 160 characters, and no name to keep.
    assert_eq!(label(4), "…");
    // The excerpt is never cut.
    assert_eq!(
        found(&path, Selector::At(at(3, 1))).1,
        format!("{};", call(161))
    );
}

#[test]
fn statement_labels_render_only_as_far_as_they_reach() {
    // Each of 250 nested blocks is a statement whose label reaches its cut
    // long before the 200,000-element array inside them all, so building
    // every label stays linear in the file.
    let (depth, items) = (250, 200_000);
    let source = format!(
        "{}f([{}]);{}\n",
        "{ ".repeat(depth),
        "1, ".repeat(items),
        " }".repeat(depth)
    );
    let file = Fixture::new(&[("nested.ts", &source)]);
    let path = file.path("nested.ts");
    let start = std::time::Instant::now();
    let (outer, _) = found(&path, Selector::At(at(1, 1)));
    let elapsed = start.elapsed();
    // The cut falls before the space the renderer would write next.
    assert_eq!(outer.label, format!("{}{{…", "{ ".repeat(79)));
    let (inner, source) = found(&path, Selector::At(at(1, 2 * depth + 1)));
    assert!(inner.label.starts_with("f([1, 1, 1,"), "{}", inner.label);
    assert!(inner.label.ends_with("1, 1,…"), "{}", inner.label);
    assert!(inner.label.chars().count() <= 162, "{}", inner.label);
    assert_eq!(inner.depth, 1);
    assert!(source.starts_with("f([1, 1,") && source.ends_with("]);"));
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "show took {elapsed:?}"
    );
}
