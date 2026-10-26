//! The `--test-outline` contract: which statements are test calls, their
//! containment, order, positions, and names, the text and JSON layouts, the
//! display cap, the accepted flags, and failures
//! (`docs/statement-sites-and-test-view-design.md`, "Test view"). Expected
//! outputs are the design's: the view has no TypeScript counterpart.
mod support;

use serde_json::{Value, json};
use std::process::{Command, Output, Stdio};
use support::{EDGE_CASES, Fixture};
use typescript_parser::navigation;

fn tsparse(fixture: &Fixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
        .current_dir(&fixture.root)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The text a successful request printed: exit 0 and nothing on stderr.
fn printed(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    text(&output.stdout)
}

fn json_output(output: &Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}

fn edge_cases() -> Fixture {
    Fixture::new(&[("edge-cases.test.ts", EDGE_CASES)])
}

/// The Examples' listing, naming `file`, the sample's absolute path.
fn sample_listing(file: &str) -> String {
    format!(
        "{file}\n\
         3:1 describe Widget.name\n\
         \x20 4:3 beforeEach\n\
         \x20 5:3 it.each() \"adds %i\"\n\
         \x20 6:3 describe.each() \"case %s\"\n\
         \x20   7:5 it `handles ${{\"x\"}}`\n\
         \x20 9:3 it.todo \"writes later\"\n\
         \x20 10:3 it.skip \"skipped one\"\n\
         \x20 11:3 test \"with options\"\n\
         \x20 12:3 it.skipIf() \"conditional\"\n\
         \x20 13:3 test.concurrent \"parallel\"\n\
         16:3 test \"pw case\"\n"
    )
}

/// The command a text cut line names after `see all with: `, split into
/// its arguments, which need no quotes here.
fn next_command(listing: &str) -> Vec<String> {
    let line = listing.lines().last().unwrap();
    let command = line
        .split_once("see all with: ")
        .unwrap_or_else(|| panic!("no command in {line}"))
        .1;
    let command = command
        .split_once(", or write them to a file")
        .map_or(command, |(command, _)| command);
    let words: Vec<String> = command.split(' ').map(str::to_owned).collect();
    assert_eq!(words[0], "tsparse", "{line}");
    words[1..].to_vec()
}

#[test]
fn the_sample_prints_the_examples_listing() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let name = file.to_str().unwrap();
    for operand in ["edge-cases.test.ts", name] {
        let output = tsparse(&fixture, &["--test-outline", operand]);
        assert_eq!(printed(&output), sample_listing(name), "{operand}");
    }
}

#[test]
fn the_sample_document_is_the_examples_document() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let name = file.to_str().unwrap();
    let expected = r#"{"entries":[{"anchor":{"column":1,"line":3},"call":"describe","children":[{"anchor":{"column":3,"line":4},"call":"beforeEach","children":[]},{"anchor":{"column":3,"line":5},"call":"it.each()","children":[],"name":"\"adds %i\""},{"anchor":{"column":3,"line":6},"call":"describe.each()","children":[{"anchor":{"column":5,"line":7},"call":"it","children":[],"name":"`handles ${\"x\"}`"}],"name":"\"case %s\""},{"anchor":{"column":3,"line":9},"call":"it.todo","children":[],"name":"\"writes later\""},{"anchor":{"column":3,"line":10},"call":"it.skip","children":[],"name":"\"skipped one\""},{"anchor":{"column":3,"line":11},"call":"test","children":[],"name":"\"with options\""},{"anchor":{"column":3,"line":12},"call":"it.skipIf()","children":[],"name":"\"conditional\""},{"anchor":{"column":3,"line":13},"call":"test.concurrent","children":[],"name":"\"parallel\""}],"name":"Widget.name"},{"anchor":{"column":3,"line":16},"call":"test","children":[],"name":"\"pw case\""}],"file":"/abs/edge-cases.test.ts","operation":"test-outline","outcome":"ok"}"#
        .replace("/abs/edge-cases.test.ts", name);
    let output = tsparse(
        &fixture,
        &["--test-outline", "edge-cases.test.ts", "--json"],
    );
    assert_eq!(printed(&output), format!("{expected}\n"));
    // `--pretty` indents the same document.
    let output = tsparse(
        &fixture,
        &["--test-outline", "--json", "--pretty", "edge-cases.test.ts"],
    );
    let pretty = printed(&output);
    assert!(pretty.lines().count() > 1, "{pretty}");
    assert_eq!(
        serde_json::from_str::<Value>(&pretty).unwrap(),
        serde_json::from_str::<Value>(&expected).unwrap()
    );
}

#[test]
fn the_library_gives_flat_entries_in_source_order_with_depths() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let outline = navigation::test_outline_file(&file).unwrap();
    assert_eq!(outline.file, file.to_str().unwrap());
    let entries: Vec<(usize, usize, &str, Option<&str>, usize)> = outline
        .entries
        .iter()
        .map(|entry| {
            (
                entry.anchor.line,
                entry.anchor.column,
                entry.call.as_str(),
                entry.name.as_deref(),
                entry.depth,
            )
        })
        .collect();
    assert_eq!(
        entries,
        [
            (3, 1, "describe", Some("Widget.name"), 1),
            (4, 3, "beforeEach", None, 2),
            (5, 3, "it.each()", Some("\"adds %i\""), 2),
            (6, 3, "describe.each()", Some("\"case %s\""), 2),
            (7, 5, "it", Some("`handles ${\"x\"}`"), 3),
            (9, 3, "it.todo", Some("\"writes later\""), 2),
            (10, 3, "it.skip", Some("\"skipped one\""), 2),
            (11, 3, "test", Some("\"with options\""), 2),
            (12, 3, "it.skipIf()", Some("\"conditional\""), 2),
            (13, 3, "test.concurrent", Some("\"parallel\""), 2),
            (16, 3, "test", Some("\"pw case\""), 1),
        ]
    );
    // An entry without a name serializes without one.
    assert_eq!(
        serde_json::to_value(&outline.entries[1]).unwrap(),
        json!({ "anchor": { "line": 4, "column": 3 }, "call": "beforeEach", "depth": 2 })
    );
}

#[test]
fn a_limit_keeps_a_prefix_and_names_the_command_that_shows_every_entry() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let name = file.to_str().unwrap();
    let output = tsparse(&fixture, &["--test-outline", name, "--limit", "3"]);
    let listing = printed(&output);
    assert_eq!(
        listing,
        format!(
            "{name}\n\
             3:1 describe Widget.name\n\
             \x20 4:3 beforeEach\n\
             \x20 5:3 it.each() \"adds %i\"\n\
             … 8 more entries; see all with: tsparse --test-outline {name} --limit all, or write them to a file with -o FILE\n"
        )
    );
    let next = next_command(&listing);
    let next: Vec<&str> = next.iter().map(String::as_str).collect();
    assert_eq!(printed(&tsparse(&fixture, &next)), sample_listing(name));
    // The document nests the shown prefix and says what it withheld.
    let document = json_output(
        &tsparse(
            &fixture,
            &["--test-outline", name, "--limit", "3", "--json"],
        ),
        0,
    );
    assert_eq!(
        document,
        json!({
            "display": {
                "entries": { "returned": 3, "total": 11 },
                "next": { "argv": ["tsparse", "--test-outline", name, "--limit", "all", "--json"] },
                "truncated": true,
            },
            "entries": [{
                "anchor": { "column": 1, "line": 3 },
                "call": "describe",
                "children": [
                    { "anchor": { "column": 3, "line": 4 }, "call": "beforeEach", "children": [] },
                    {
                        "anchor": { "column": 3, "line": 5 },
                        "call": "it.each()",
                        "children": [],
                        "name": "\"adds %i\"",
                    },
                ],
                "name": "Widget.name",
            }],
            "file": name,
            "operation": "test-outline",
            "outcome": "ok",
        })
    );
    let all = json_output(&tsparse(&fixture, &next_argv(&document)), 0);
    let expected = json_output(&tsparse(&fixture, &["--test-outline", name, "--json"]), 0);
    assert_eq!(all, expected);
    assert!(all.get("display").is_none());
    // `--limit all` and a limit past the count cut nothing.
    for limit in ["all", "11", "12"] {
        let output = tsparse(&fixture, &["--test-outline", name, "--limit", limit]);
        assert_eq!(printed(&output), sample_listing(name), "{limit}");
    }
}

/// `count` top-level tests, one per line.
fn tests(count: usize) -> String {
    (0..count)
        .map(|index| format!("it(\"t{index}\", () => {{}});\n"))
        .collect()
}

/// The listing of `tests(count)`'s first `shown` entries in `file`.
fn tests_listing(file: &str, shown: usize) -> String {
    let mut listing = file.to_owned();
    for index in 0..shown {
        listing.push_str(&format!("\n{}:1 it \"t{index}\"", index + 1));
    }
    listing
}

/// The document entries of `tests(count)`'s first `shown` entries.
fn tests_entries(shown: usize) -> Value {
    (0..shown)
        .map(|index| {
            json!({
                "anchor": { "column": 1, "line": index + 1 },
                "call": "it",
                "children": [],
                "name": format!("\"t{index}\""),
            })
        })
        .collect()
}

/// What a request that names `out.txt` as its output wrote there: it exits
/// 0 and prints nothing. The file is removed afterwards, so no later
/// request can pass by finding it already written.
fn written(fixture: &Fixture, args: &[&str]) -> String {
    let out = fixture.path("out.txt");
    assert!(!out.exists(), "{args:?}");
    assert!(printed(&tsparse(fixture, args)).is_empty(), "{args:?}");
    let text = std::fs::read_to_string(&out).unwrap();
    std::fs::remove_file(&out).unwrap();
    text
}

/// The arguments of a document's `display.next` command, without the
/// program name.
fn next_argv(document: &Value) -> Vec<&str> {
    let argv: Vec<&str> = document["display"]["next"]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| arg.as_str().unwrap())
        .collect();
    assert_eq!(argv[0], "tsparse", "{document}");
    argv[1..].to_vec()
}

#[test]
fn the_default_cap_shows_200_entries_and_o_removes_it() {
    let fixture = Fixture::new(&[
        ("199.ts", &tests(199)),
        ("200.ts", &tests(200)),
        ("201.ts", &tests(201)),
    ]);
    for (file, count) in [("199.ts", 199), ("200.ts", 200), ("201.ts", 201)] {
        let path = fixture.path(file);
        let name = path.to_str().unwrap();
        let all = format!("{}\n", tests_listing(name, count));
        let complete = json!({
            "entries": tests_entries(count),
            "file": name,
            "operation": "test-outline",
            "outcome": "ok",
        });
        // Without `-o`, the default cap shows the first 200 entries, and a
        // cut names the command that shows every one, which does.
        let listing = printed(&tsparse(&fixture, &["--test-outline", file]));
        let document = json_output(&tsparse(&fixture, &["--test-outline", file, "--json"]), 0);
        if count <= 200 {
            assert_eq!(listing, all, "{file}");
            assert_eq!(document, complete, "{file}");
        } else {
            assert_eq!(
                listing,
                format!(
                    "{}\n… 1 more entry; see all with: tsparse --test-outline {file} --limit all, or write them to a file with -o FILE\n",
                    tests_listing(name, 200)
                )
            );
            let next = next_command(&listing);
            let next: Vec<&str> = next.iter().map(String::as_str).collect();
            assert_eq!(next, ["--test-outline", file, "--limit", "all"]);
            assert_eq!(printed(&tsparse(&fixture, &next)), all, "{next:?}");
            assert_eq!(document["entries"], tests_entries(200));
            assert_eq!(
                document["display"],
                json!({
                    "entries": { "returned": 200, "total": 201 },
                    "next": { "argv": ["tsparse", "--test-outline", file, "--json", "--limit", "all"] },
                    "truncated": true,
                })
            );
            let next = next_argv(&document);
            assert_eq!(
                json_output(&tsparse(&fixture, &next), 0),
                complete,
                "{next:?}"
            );
        }
        // With `-o` or `--output` and no `--limit`, nothing is cut, in text
        // and JSON, and the document has no `display`.
        for output in ["-o", "--output"] {
            let args = ["--test-outline", file, output, "out.txt"];
            assert_eq!(written(&fixture, &args), all, "{args:?}");
            let args = ["--test-outline", file, "--json", output, "out.txt"];
            let document: Value = serde_json::from_str(&written(&fixture, &args)).unwrap();
            assert_eq!(document, complete, "{args:?}");
        }
    }
    // An explicit limit applies in the file too. Its cut names no other
    // file, and the command it names, which keeps `-o`, writes every entry
    // there.
    let path = fixture.path("201.ts");
    let name = path.to_str().unwrap();
    let cut = written(
        &fixture,
        &["--test-outline", "201.ts", "-o", "out.txt", "--limit", "5"],
    );
    assert_eq!(
        cut,
        format!(
            "{}\n… 196 more entries; see all with: tsparse --test-outline 201.ts -o out.txt --limit all\n",
            tests_listing(name, 5)
        )
    );
    let next = next_command(&cut);
    let next: Vec<&str> = next.iter().map(String::as_str).collect();
    assert_eq!(
        written(&fixture, &next),
        format!("{}\n", tests_listing(name, 201)),
        "{next:?}"
    );
    let document: Value = serde_json::from_str(&written(
        &fixture,
        &[
            "--test-outline",
            "201.ts",
            "--json",
            "-o",
            "out.txt",
            "--limit",
            "5",
        ],
    ))
    .unwrap();
    assert_eq!(document["entries"], tests_entries(5));
    assert_eq!(
        document["display"],
        json!({
            "entries": { "returned": 5, "total": 201 },
            "next": {
                "argv": ["tsparse", "--test-outline", "201.ts", "--json", "-o", "out.txt", "--limit", "all"],
            },
            "truncated": true,
        })
    );
    let next = next_argv(&document);
    let document: Value = serde_json::from_str(&written(&fixture, &next)).unwrap();
    assert_eq!(
        document,
        json!({
            "entries": tests_entries(201),
            "file": name,
            "operation": "test-outline",
            "outcome": "ok",
        }),
        "{next:?}"
    );
}

/// Each line of a file, at column 1, with what the view lists for it.
const RECOGNITION: &[(&str, Option<&str>)] = &[
    ("import { describe, it, test } from \"vitest\";", None),
    // The roots.
    (
        "describe(\"describe root\", () => {});",
        Some("describe \"describe root\""),
    ),
    ("it(\"it root\");", Some("it \"it root\"")),
    ("test(\"test root\");", Some("test \"test root\"")),
    // Each plain modifier.
    ("it.only(\"only\");", Some("it.only \"only\"")),
    ("it.skip(\"skip\");", Some("it.skip \"skip\"")),
    ("it.todo(\"todo\");", Some("it.todo \"todo\"")),
    (
        "test.concurrent(\"concurrent\");",
        Some("test.concurrent \"concurrent\""),
    ),
    (
        "test.sequential(\"sequential\");",
        Some("test.sequential \"sequential\""),
    ),
    (
        "describe.shuffle(\"shuffle\", () => {});",
        Some("describe.shuffle \"shuffle\""),
    ),
    ("it.fails(\"fails\");", Some("it.fails \"fails\"")),
    // Each called modifier.
    (
        "it.each([1])(\"each %i\", (n) => {});",
        Some("it.each() \"each %i\""),
    ),
    (
        "test.for([1])(\"for %i\", (n) => {});",
        Some("test.for() \"for %i\""),
    ),
    ("it.skipIf(a)(\"skipIf\");", Some("it.skipIf() \"skipIf\"")),
    ("it.runIf(a)(\"runIf\");", Some("it.runIf() \"runIf\"")),
    // Chains of modifiers, in source order.
    (
        "describe.concurrent.each([1])(\"chained %i\", () => {});",
        Some("describe.concurrent.each() \"chained %i\""),
    ),
    (
        "it.concurrent.skip.only(\"plain chain\");",
        Some("it.concurrent.skip.only \"plain chain\""),
    ),
    (
        "test.skipIf(a).each([1])(\"called chain %i\", () => {});",
        Some("test.skipIf().each() \"called chain %i\""),
    ),
    (
        "test.each([1]).skip(\"called then plain\");",
        Some("test.each().skip \"called then plain\""),
    ),
    // Each hook, which has no name.
    ("beforeAll(() => {});", Some("beforeAll")),
    ("beforeEach(() => {});", Some("beforeEach")),
    ("afterAll(() => {});", Some("afterAll")),
    ("afterEach(() => {});", Some("afterEach")),
    (
        "beforeEach(\"a hook has no name\", () => {});",
        Some("beforeEach"),
    ),
    // A call without arguments has no name.
    ("it();", Some("it")),
    // `?.` and type arguments change nothing; names are decoded.
    (
        "it?.skip(\"optional chain\");",
        Some("it.skip \"optional chain\""),
    ),
    (
        "it.each<number>([1])<string>(\"type arguments\", () => {});",
        Some("it.each() \"type arguments\""),
    ),
    (
        "\\u0069t.s\\u006bip(\"escaped\");",
        Some("it.skip \"escaped\""),
    ),
    // Not test calls: a modifier in the other form.
    ("it.each(\"n\", fn);", None),
    ("it.skip(c)(\"n\", fn);", None),
    ("describe.each.skip([1])(\"plain modifier called\");", None),
    ("it.each(table);", None),
    // A tagged template table is not a call.
    ("it.each`a | b`(\"n\", fn);", None),
    // Anything wrapping the call.
    (
        "async function f() { await it(\"awaited\"); await test.step(\"step\"); }",
        None,
    ),
    ("(it(\"parenthesized call\"));", None),
    ("void it(\"void\");", None),
    ("x = it(\"assigned\");", None),
    ("new it(\"constructed\");", None),
    // Anything else in the callee.
    ("this.it(\"receiver\");", None),
    ("test.runtimeHost.advance(\"fixture\");", None),
    ("test.extend(\"fixture\");", None),
    ("test.describe(\"playwright\");", None),
    ("it[\"skip\"](\"element access\");", None),
    ("it!.skip(\"non-null assertion\");", None),
    ("(it)(\"parenthesized callee\");", None),
    ("it.Skip(\"other modifier\");", None),
    ("It(\"other root\");", None),
    ("beforeEach.skip(() => {});", None),
    ("suite(\"suite\");", None),
    ("expect(\"x\");", None),
];

fn recognition() -> (String, String) {
    let source: String = RECOGNITION
        .iter()
        .map(|(line, _)| format!("{line}\n"))
        .collect();
    let listing: String = RECOGNITION
        .iter()
        .enumerate()
        .filter_map(|(index, (_, listed))| {
            listed.map(|listed| format!("{}:1 {listed}\n", index + 1))
        })
        .collect();
    (source, listing)
}

#[test]
fn recognition_follows_the_fixed_grammar() {
    let (source, listing) = recognition();
    let fixture = Fixture::new(&[("recognition.test.ts", &source)]);
    let file = fixture.path("recognition.test.ts");
    let output = tsparse(&fixture, &["--test-outline", "recognition.test.ts"]);
    assert_eq!(printed(&output), format!("{}\n{listing}", file.display()));
}

const CONTAINMENT: &str = "\
test.describe(\"transparent\", () => {
  it(\"inside a transparent call\", () => {});
});
function helper() {
  it(\"in a top-level helper\", () => {});
}
describe(\"suite\", () => {
  function nested() {
    it(\"in a helper inside a suite\", () => {});
  }
  if (flag) it(\"in an unbraced body\", () => {});
  for (const c of cases) {
    test(\"in a loop\", () => {});
  }
  describe(\"inner\", () => {
    helper(() => { beforeEach(() => {}); });
  });
});
@decorate(() => { it(\"in a decorator\", () => {}); })
class Late {
  method() { it(\"in a method\", () => {}); }
}
";

#[test]
fn containment_is_syntax_tree_ancestry_and_order_is_position() {
    let fixture = Fixture::new(&[("containment.test.ts", CONTAINMENT)]);
    let file = fixture.path("containment.test.ts");
    let output = tsparse(&fixture, &["--test-outline", "containment.test.ts"]);
    // A call that is not a test call is transparent, a helper outside every
    // test call is at the top level, and a decorator's test, which the walk
    // reaches after the class's members, is ordered by its position.
    assert_eq!(
        printed(&output),
        format!(
            "{}\n\
             2:3 it \"inside a transparent call\"\n\
             5:3 it \"in a top-level helper\"\n\
             7:1 describe \"suite\"\n\
             \x20 9:5 it \"in a helper inside a suite\"\n\
             \x20 11:13 it \"in an unbraced body\"\n\
             \x20 13:5 test \"in a loop\"\n\
             \x20 15:3 describe \"inner\"\n\
             \x20   16:20 beforeEach\n\
             19:19 it \"in a decorator\"\n\
             21:14 it \"in a method\"\n",
            file.display()
        )
    );
    let document = json_output(
        &tsparse(
            &fixture,
            &["--test-outline", "--json", "containment.test.ts"],
        ),
        0,
    );
    let suite = &document["entries"][2];
    assert_eq!(suite["name"], "\"suite\"");
    assert_eq!(suite["children"].as_array().unwrap().len(), 4);
    assert_eq!(
        suite["children"][3]["children"],
        json!([{ "anchor": { "column": 20, "line": 16 }, "call": "beforeEach", "children": [] }])
    );
}

/// Names with each line break the lexer distinguishes, written over two
/// lines, and names that are not literals.
const NAMES: &str = concat!(
    "it(`lf\nname`, () => {});\n",
    "it(\"continued\\\nstring\", () => {});\n",
    "it(`crlf\r\nname`, () => {});\n",
    "it(`cr\rname`, () => {});\n",
    "it(\"ls\u{2028}name\", () => {});\n",
    "it(`ps\u{2029}name`, () => {});\n",
    "describe(Widget.\nname, () => {});\n",
    "it(`handles ${\"x\"}`, () => {});\n",
    "const s = \"\u{1F600}\"; it(\"after an astral character\");\n",
);

#[test]
fn names_are_written_without_line_breaks_and_columns_count_utf16() {
    let fixture = Fixture::new(&[("names.test.ts", NAMES)]);
    let file = fixture.path("names.test.ts");
    let output = tsparse(&fixture, &["--test-outline", "names.test.ts"]);
    // Only the line breaks are removed: a line continuation keeps its `\`.
    assert_eq!(
        printed(&output),
        format!(
            "{}\n\
             1:1 it `lfname`\n\
             3:1 it \"continued\\string\"\n\
             5:1 it `crlfname`\n\
             7:1 it `crname`\n\
             9:1 it \"lsname\"\n\
             11:1 it `psname`\n\
             13:1 describe Widget.name\n\
             15:1 it `handles ${{\"x\"}}`\n\
             16:17 it \"after an astral character\"\n",
            file.display()
        )
    );
}

const COMPUTED: &str = "\
describe(\"outer\", () => {
  class C {
    [(() => { describe(\"hidden\", () => { it(\"inside hidden\", () => {}); }); return \"m\"; })()]() {}
  }
  const o = { [(() => { it(\"in an object\", () => {}); return \"k\"; })()]: 1 };
  it(\"visible\", () => {});
});
";

#[test]
fn a_test_call_in_a_computed_property_name_is_not_listed() {
    let fixture = Fixture::new(&[("computed.test.ts", COMPUTED)]);
    let file = fixture.path("computed.test.ts");
    let output = tsparse(&fixture, &["--test-outline", "computed.test.ts"]);
    assert_eq!(
        printed(&output),
        format!(
            "{}\n1:1 describe \"outer\"\n  6:3 it \"visible\"\n",
            file.display()
        )
    );
    // `--at` there selects the member the computed name names.
    let shown = json_output(
        &tsparse(
            &fixture,
            &["--show", "--json", "--at", "3:15", "computed.test.ts"],
        ),
        0,
    );
    assert_eq!(shown["declaration"]["kind"], "method", "{shown}");
}

#[test]
fn every_listed_position_selects_its_test_statement() {
    let (recognition, _) = recognition();
    let fixture = Fixture::new(&[
        ("edge-cases.test.ts", EDGE_CASES),
        ("recognition.test.ts", &recognition),
        ("containment.test.ts", CONTAINMENT),
        ("names.test.ts", NAMES),
        ("computed.test.ts", COMPUTED),
    ]);
    let mut followed = 0;
    for file in [
        "edge-cases.test.ts",
        "recognition.test.ts",
        "containment.test.ts",
        "names.test.ts",
        "computed.test.ts",
    ] {
        let path = fixture.path(file);
        let listing = printed(&tsparse(&fixture, &["--test-outline", file]));
        for line in listing.lines().skip(1) {
            let position = line.trim_start().split(' ').next().unwrap();
            let at = format!("{}:{position}", path.display());
            let shown = json_output(&tsparse(&fixture, &["--show", "--json", "--at", &at]), 0);
            let (line, column) = position.split_once(':').unwrap();
            let anchor = json!({ "line": line.parse::<u64>().unwrap(), "column": column.parse::<u64>().unwrap() });
            let declaration = &shown["declaration"];
            assert_eq!(declaration["kind"], "statement", "{at}: {shown}");
            assert_eq!(declaration["anchor"], anchor, "{at}");
            assert_eq!(declaration["range"]["start"], anchor, "{at}");
            assert!(
                shown["source"].as_str().unwrap().ends_with(';'),
                "{at}: {shown}"
            );
            followed += 1;
        }
    }
    assert_eq!(followed, 11 + 27 + 10 + 9 + 2);
}

#[test]
fn every_listed_position_takes_an_insertion_after_its_test() {
    let fixture = Fixture::new(&[
        ("edge-cases.test.ts", EDGE_CASES),
        ("new-test.ts", "it(\"new test\", () => {});\n"),
    ]);
    let file = fixture.path("edge-cases.test.ts");
    let name = file.to_str().unwrap();
    let listing = printed(&tsparse(
        &fixture,
        &["--test-outline", "edge-cases.test.ts"],
    ));
    let mut followed = 0;
    for line in listing.lines().skip(1) {
        let position = line.trim_start().split(' ').next().unwrap();
        let at = format!("{name}:{position}");
        let preview = printed(&tsparse(
            &fixture,
            &["--insert-after", "--at", &at, "--from", "new-test.ts"],
        ));
        // Every test of the sample is in a statement list, so the preview
        // inserts after the test's statement, which starts at the position.
        let site = preview.lines().next().unwrap();
        assert!(
            site.starts_with(&format!("{at}: insert_after statement ")),
            "{site}"
        );
        assert!(
            site.contains(&format!(": after the statement at {position}-")),
            "{site}"
        );
        assert!(
            preview
                .lines()
                .any(|line| line.starts_with('+') && line.ends_with("it(\"new test\", () => {});")),
            "{preview}"
        );
        followed += 1;
    }
    assert_eq!(followed, 11);
    // A preview writes nothing.
    assert_eq!(std::fs::read_to_string(&file).unwrap(), EDGE_CASES);
}

#[test]
fn deep_suites_nest_to_the_parser_s_budget() {
    let nested = |count: usize| {
        format!(
            "{}{}\n",
            "describe(\"d\", () => {".repeat(count),
            "});".repeat(count)
        )
    };
    // 85 nested suites are the most navigation's grammar budget accepts.
    let fixture = Fixture::new(&[("at.test.ts", &nested(85)), ("over.test.ts", &nested(86))]);
    let file = fixture.path("at.test.ts");
    let outline = navigation::test_outline_file(&file).unwrap();
    assert_eq!(outline.entries.len(), 85);
    for (index, entry) in outline.entries.iter().enumerate() {
        assert_eq!(entry.depth, index + 1);
        assert_eq!(entry.anchor.column, 1 + 21 * index);
    }
    let listing = printed(&tsparse(&fixture, &["--test-outline", "at.test.ts"]));
    let last = listing.lines().last().unwrap();
    assert_eq!(last, format!("{}1:1765 describe \"d\"", "  ".repeat(84)));
    // The document nests every level; it is deeper than a test's JSON parser
    // reads, so its text is checked.
    let document = printed(&tsparse(
        &fixture,
        &["--test-outline", "--json", "at.test.ts"],
    ));
    let entry = |index: usize| {
        format!(
            "{{\"anchor\":{{\"column\":{},\"line\":1}},\"call\":\"describe\",\"children\":[",
            1 + 21 * index
        )
    };
    let opened: String = (0..85).map(entry).collect();
    assert!(
        document.starts_with(&format!("{{\"entries\":[{opened}]")),
        "{document}"
    );
    assert_eq!(document.matches("\"children\"").count(), 85);
    let output = tsparse(&fixture, &["--test-outline", "over.test.ts"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(
        text(&output.stderr).ends_with(": Maximum syntax nesting depth exceeded\n"),
        "{output:?}"
    );
    let document = json_output(
        &tsparse(&fixture, &["--test-outline", "--json", "over.test.ts"]),
        1,
    );
    assert_eq!(document["operation"], "test-outline");
    assert_eq!(document["error"]["kind"], "ResourceLimit");
}

#[test]
fn a_file_without_test_calls_lists_only_its_path() {
    let fixture = Fixture::new(&[
        ("plain.ts", "export const x = 1;\nfunction f() {}\n"),
        ("empty.ts", ""),
    ]);
    for file in ["plain.ts", "empty.ts"] {
        let output = tsparse(&fixture, &["--test-outline", file]);
        assert_eq!(
            printed(&output),
            format!("{}\n", fixture.path(file).display())
        );
        let document = json_output(&tsparse(&fixture, &["--test-outline", "--json", file]), 0);
        assert_eq!(
            document,
            json!({
                "entries": [],
                "file": fixture.path(file).to_str().unwrap(),
                "operation": "test-outline",
                "outcome": "ok",
            })
        );
    }
}

#[test]
fn input_failures_are_the_outline_s_with_their_exit_codes() {
    let fixture = Fixture::new(&[
        ("broken.ts", "const = 1;\n"),
        ("data.json", "{}\n"),
        ("script.js", "it(\"x\");\n"),
        ("notes.txt", "it(\"x\");\n"),
    ]);
    std::fs::create_dir(fixture.path("directory.ts")).unwrap();
    // (file, exit code, error kind)
    for (file, code, kind) in [
        ("missing.ts", 2, "Io"),
        ("directory.ts", 2, "Io"),
        ("broken.ts", 1, "Syntax"),
        ("data.json", 1, "Unsupported"),
        ("script.js", 1, "Unsupported"),
        ("notes.txt", 1, "Unsupported"),
    ] {
        let output = tsparse(&fixture, &["--test-outline", file]);
        assert_eq!(output.status.code(), Some(code), "{file}");
        assert!(output.stdout.is_empty(), "{file}");
        assert!(!output.stderr.is_empty(), "{file}");
        // The outline's failure for the same file, as text.
        let outline = tsparse(&fixture, &["--outline", file]);
        assert_eq!(output.stderr, outline.stderr, "{file}");
        let document = json_output(
            &tsparse(&fixture, &["--test-outline", "--json", file]),
            code,
        );
        assert_eq!(document["operation"], "test-outline", "{file}");
        assert_eq!(document["outcome"], "error", "{file}");
        assert_eq!(
            document["file"],
            fixture.path(file).to_str().unwrap(),
            "{file}"
        );
        assert_eq!(document["error"]["kind"], kind, "{file}");
        let mut outline = json_output(&tsparse(&fixture, &["--outline", "--json", file]), code);
        outline["operation"] = json!("test-outline");
        assert_eq!(document, outline, "{file}");
    }
    let output = tsparse(&fixture, &["--test-outline", "broken.ts"]);
    assert_eq!(
        text(&output.stderr),
        format!(
            "{}:1:7: Expected a binding identifier\n",
            fixture.path("broken.ts").display()
        )
    );
    // A diagnostic never creates the `-o` file; a document goes there.
    let output = tsparse(&fixture, &["--test-outline", "broken.ts", "-o", "out.txt"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!fixture.path("out.txt").exists());
    let output = tsparse(
        &fixture,
        &["--test-outline", "--json", "broken.ts", "-o", "out.txt"],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    let document: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.path("out.txt")).unwrap()).unwrap();
    assert_eq!(
        document["error"]["position"],
        json!({ "line": 1, "column": 7 })
    );
    // A result that cannot be written is a stderr diagnostic, exit 2.
    std::fs::write(fixture.path("app.test.ts"), "it(\"x\");\n").unwrap();
    for format in [&[][..], &["--json"]] {
        let args = [
            &["--test-outline", "app.test.ts", "-o", "missing/out.txt"][..],
            format,
        ]
        .concat();
        let output = tsparse(&fixture, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(
            text(&output.stderr).starts_with("write.output: missing/out.txt: "),
            "{output:?}"
        );
    }
}

#[test]
fn the_operation_accepts_exactly_its_flags() {
    let fixture = edge_cases();
    let file = fixture.path("edge-cases.test.ts");
    let name = file.to_str().unwrap();
    let operand = "edge-cases.test.ts";
    let listing = sample_listing(name);
    let help = printed(&tsparse(&fixture, &["--test-outline", "--help"]));
    assert!(help.starts_with("tsparse --test-outline: "), "{help}");
    let version = printed(&tsparse(&fixture, &["--version"]));
    for (args, expected) in [
        (&["--limit", "all"][..], listing.clone()),
        (&["--limit", "20"], listing.clone()),
        (&["--help"], help.clone()),
        (&["-h"], help.clone()),
        (&["--version"], version.clone()),
        (&["-V"], version.clone()),
    ] {
        let output = tsparse(&fixture, &[&["--test-outline", operand][..], args].concat());
        assert_eq!(printed(&output), expected, "{args:?}");
    }
    let document = printed(&tsparse(&fixture, &["--test-outline", operand, "--json"]));
    let pretty = printed(&tsparse(
        &fixture,
        &["--test-outline", operand, "--json", "--pretty"],
    ));
    assert_eq!(
        serde_json::from_str::<Value>(&pretty).unwrap(),
        serde_json::from_str::<Value>(&document).unwrap()
    );
    for output in ["--output", "-o"] {
        let out = fixture.path("out.txt");
        let result = tsparse(&fixture, &["--test-outline", operand, output, "out.txt"]);
        assert!(printed(&result).is_empty(), "{output}");
        assert_eq!(std::fs::read_to_string(&out).unwrap(), listing, "{output}");
        std::fs::remove_file(&out).unwrap();
    }
    // Every other flag is out of scope, and so is stdin.
    for (args, message) in [
        (
            &["--project", "tsconfig.json", operand][..],
            "usage.flag_scope: --project applies to: ",
        ),
        (
            &["--direct", operand],
            "usage.flag_scope: --direct applies to: ",
        ),
        (
            &["--depth", "2", operand],
            "usage.flag_scope: --depth applies to: ",
        ),
        (
            &["--at", "10:3", operand],
            "usage.flag_scope: --at applies to: ",
        ),
        (
            &["--name", "x", operand],
            "usage.flag_scope: --name applies to: ",
        ),
        (&["--tsx", operand], "usage.flag_scope: --tsx applies to: "),
        (
            &["--check", operand],
            "usage.flag_scope: --check applies to: ",
        ),
        (
            &["--from", "x.ts", operand],
            "usage.flag_scope: --from applies to: ",
        ),
        (
            &["--pretty", operand],
            "usage.flag_scope: --pretty applies to --test-outline only with --json",
        ),
        (
            &["-"],
            "usage.operands: --test-outline reads a named FILE, not stdin",
        ),
        (&[], "usage.operands: --test-outline requires a FILE"),
        (
            &[operand, "other.ts"],
            "usage.operands: Expected one input file",
        ),
        (
            &["--limit", "0", operand],
            "usage.invalid_value: --limit expects a positive number or all, got 0",
        ),
        (
            &["--limit", "1", "--limit", "2", operand],
            "usage.repeated_flag: --test-outline takes --limit at most once",
        ),
        (
            &["--outline", operand],
            "usage.operation: Choose one operation: --test-outline, --outline",
        ),
    ] {
        let output = tsparse(&fixture, &[&["--test-outline"][..], args].concat());
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        let stderr = text(&output.stderr);
        assert!(stderr.starts_with(message), "{args:?}: {stderr}");
        // The operations a refused flag applies to do not include this one.
        if let Some((_, accepting)) = stderr.split_once(" applies to: ") {
            assert!(
                accepting
                    .trim_end()
                    .split(", ")
                    .all(|op| op != "--test-outline"),
                "{stderr}"
            );
        }
    }
    // With `--json`, a usage failure is a document naming the operation.
    let document = json_output(
        &tsparse(
            &fixture,
            &["--test-outline", "--json", "--at", "10:3", operand],
        ),
        2,
    );
    assert_eq!(document["operation"], "test-outline");
    assert_eq!(document["outcome"], "error");
    assert_eq!(document["error"]["code"], "usage.flag_scope");
    let document = json_output(&tsparse(&fixture, &["--test-outline", "--json", "-"]), 2);
    assert_eq!(
        document,
        json!({
            "operation": "test-outline",
            "outcome": "error",
            "error": {
                "category": "usage",
                "code": "usage.operands",
                "message": "--test-outline reads a named FILE, not stdin",
            },
        })
    );
}
