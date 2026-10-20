//! The text form of a reference answer and the selection it echoes (design,
//! step 4: "References in text" and "Selection echo"). Every answer is asked
//! through the project's service and loaded directly, which must print the
//! same bytes. The groups, occurrences, roles, and gaps rendered here are
//! the pinned compiler's: `fixtures/references/aliases-shadowing` is checked
//! against TypeScript 7.0.2 by the semantic validation, the member search is
//! `tests/member_references.rs`'s, and the selections are
//! `tests/reference_selection.rs`'s.
#![cfg(unix)]
mod support;

use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};
use support::Fixture;
use support::service::Service;

/// One answer, the same bytes through the service and loaded directly.
struct Answer {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// `args` for the project's `tsconfig.json`, from its directory, through the
/// service and with `--direct`, which must agree byte for byte.
fn both(service: &Service, args: &[&str]) -> Answer {
    answer(service.both(args))
}

fn answer(output: Output) -> Answer {
    Answer {
        code: output.status.code(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

/// A result: exit `code`, `text` on stdout, nothing on stderr.
fn result(answer: Answer, code: i32, text: &str) {
    assert_eq!(answer.stderr, "");
    assert_eq!(answer.stdout, format!("{text}\n"));
    assert_eq!(answer.code, Some(code));
}

/// A selection that selected nothing: exit 1, `text` on stderr, nothing on
/// stdout.
fn unselected(answer: Answer, text: &str) {
    assert_eq!(answer.stdout, "");
    assert_eq!(answer.stderr, format!("{text}\n"));
    assert_eq!(answer.code, Some(1));
}

/// The design's alias example, compiler-checked: `start` imports `run`
/// through the barrel's `execute`, and a parameter `start` shadows it.
fn aliases() -> Service {
    Service::of(Fixture::copy_of("aliases-shadowing"))
}

/// The member search of `tests/member_references.rs`: `Box.value` has a use
/// on an `unknown` receiver it cannot establish.
fn members() -> Service {
    Service::of(Fixture::copy_from_repository(
        "fixtures/types/member-search-core",
    ))
}

/// The selections of `tests/reference_selection.rs`: `Both.run` names a
/// static and an instance member.
fn selections() -> Service {
    Service::of(Fixture::new(&[
        (
            "cases.ts",
            "\
export class Overloaded {
  run(value: string): void;
  run(value: number): void;
  run(value: string | number): void {}
}
export class Both {
  static run(): void {}
  run(): void {}
}
export class Parameter { constructor(public run: number) {} }
export namespace Outer.Inner { export const run = 1; }
",
        ),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true},"files":["cases.ts"]}"#,
        ),
    ]))
}

fn root(service: &Service) -> String {
    service.fixture.root.display().to_string()
}

/// The `run` group of `aliases()`, which `start` and `execute` alias.
fn run_group(r: &str, number: usize) -> String {
    format!(
        "\
group {number}: run, declared at {r}/api.ts:2:28
  {r}/api.ts:2:28 declaration, export
  {r}/api.ts:3:35 use
  {r}/app.ts:2:20 import (execute)
  {r}/app.ts:2:41 import (start)
  {r}/app.ts:3:11 use (start)
  {r}/barrel.ts:1:20 export
  {r}/barrel.ts:1:37 export (execute)"
    )
}

#[test]
fn a_complete_answer_prints_its_groups_and_each_alias_spelling() {
    let service = aliases();
    let r = &root(&service);
    result(
        both(&service, &["--references", "--at", "app.ts:3:11"]),
        0,
        &format!(
            "{r}/tsconfig.json: at {r}/app.ts:3:11: 1 group, 7 occurrences in 3 files; coverage lexical, complete\n{}",
            run_group(r, 1)
        ),
    );
}

#[test]
fn a_name_prints_every_group_it_selects() {
    let service = aliases();
    let r = &root(&service);
    result(
        both(&service, &["--references", "--name", "start"]),
        0,
        &format!(
            "\
{r}/tsconfig.json: `start` 2 groups, 9 occurrences in 3 files; coverage lexical, complete
{}
group 2: start, declared at {r}/app.ts:4:40
  {r}/app.ts:4:40 declaration
  {r}/app.ts:4:73 use",
            run_group(r, 1)
        ),
    );
}

#[test]
fn a_file_operand_is_stated_as_narrowing_the_candidates() {
    let service = aliases();
    let r = &root(&service);
    result(
        both(&service, &["--references", "--name", "start", "app.ts"]),
        0,
        &format!(
            "\
{r}/tsconfig.json: `start` 2 groups, 9 occurrences in 3 files; candidates narrowed to {r}/app.ts; coverage lexical, complete
{}
group 2: start, declared at {r}/app.ts:4:40
  {r}/app.ts:4:40 declaration
  {r}/app.ts:4:73 use",
            run_group(r, 1)
        ),
    );
    // No group is not "no references": no candidate is declared there.
    result(
        both(&service, &["--references", "--name", "start", "./api.ts"]),
        0,
        &format!(
            "{r}/tsconfig.json: `start` 0 groups, 0 occurrences in 0 files; candidates narrowed to {r}/api.ts; coverage lexical, complete"
        ),
    );
}

/// The text a member suggestion's hint names, after `search … with `.
fn hint(stdout: &str) -> &str {
    let line = stdout.lines().last().unwrap();
    let (_, flag) = line
        .split_once(" with ")
        .unwrap_or_else(|| panic!("{line}"));
    flag
}

/// `tsparse --references --project tsconfig.json --direct` followed by
/// `words`, as a shell reads them, from the project's directory.
fn follow(service: &Service, words: &str) -> Answer {
    let binary = Path::new(env!("CARGO_BIN_EXE_tsparse"));
    let path = format!(
        "{}:{}",
        binary.parent().unwrap().display(),
        std::env::var("PATH").unwrap()
    );
    answer(
        Command::new("sh")
            .arg("-c")
            .arg(format!(
                "tsparse --references --project tsconfig.json --direct {words}"
            ))
            .current_dir(&service.fixture.root)
            .env("PATH", path)
            .env("TSPARSE_SERVICE_DIR", &service.runtime)
            .output()
            .unwrap(),
    )
}

#[test]
fn unsearched_members_name_a_selection_that_searches_one() {
    let service = members();
    let r = &root(&service);
    let suggested = both(&service, &["--references", "--name", "value"]);
    let stdout = suggested.stdout.clone();
    result(
        suggested,
        0,
        &format!(
            "\
{r}/tsconfig.json: `value` 0 groups, 0 occurrences in 0 files; coverage lexical, complete
2 catalog members named value were not searched; search Box.value with --at {r}/cases.ts:2:36"
        ),
    );
    // Followed as printed, it is a member search: a partial one, whose gap
    // and its cause follow the occurrences.
    let partial = |selection: &str| {
        format!(
            "\
{r}/tsconfig.json: {selection} 1 group, 3 occurrences in 1 file; coverage member, partial
group 1: value, declared at {r}/cases.ts:2:36
  {r}/cases.ts:2:36 declaration
  {r}/cases.ts:5:17 use
  {r}/cases.ts:6:19 use
gap {r}/cases.ts:28:32: error_type: native non-null checking consumes unknown-type error recovery
  cause {r}/cases.ts:28:1: error_type: native non-null checking consumes unknown-type error recovery"
        )
    };
    result(
        follow(&service, hint(&stdout)),
        1,
        &partial(&format!("at {r}/cases.ts:2:36:")),
    );
    result(
        both(&service, &["--references", "--name", "Box.value"]),
        1,
        &partial("`Box.value`"),
    );
}

/// The member searches of `fixtures/types/reference-member-hints`, which
/// `tools/reference-member-hints-oracle.test.mjs` pins to TypeScript 7.0.2.
fn member_hints() -> Service {
    Service::of(Fixture::copy_from_repository(
        "fixtures/types/reference-member-hints",
    ))
}

/// Each hint, followed as printed, searches the member it names, at the
/// position the library names for that (`search`): a member whose selector
/// is a bare name or names a static and an instance member alike, and a
/// parameter property, whose name is its lexical parameter, alone, beside a
/// static member of its name, or in an anonymous class.
#[test]
fn each_member_hint_searches_its_member() {
    let service = member_hints();
    let r = &root(&service);
    for (file, hint_line, searched) in [
        (
            "anon.ts",
            format!("search run with --at {r}/anon.ts:1:24"),
            format!(
                "\
{r}/tsconfig.json: at {r}/anon.ts:1:24: 1 group, 2 occurrences in 2 files; coverage member, complete
group 1: run, declared at {r}/anon.ts:1:24
  {r}/anon.ts:1:24 declaration
  {r}/use.ts:4:12 use"
            ),
        ),
        (
            "both.ts",
            format!("search Both.run with --at {r}/both.ts:1:28"),
            format!(
                "\
{r}/tsconfig.json: at {r}/both.ts:1:28: 1 group, 2 occurrences in 2 files; coverage member, complete
group 1: run, declared at {r}/both.ts:1:28
  {r}/both.ts:1:28 declaration
  {r}/use.ts:5:6 use"
            ),
        ),
        (
            "parameter.ts",
            format!("search Parameter.run with --at {r}/parameter.ts:1:38"),
            format!(
                "\
{r}/tsconfig.json: at {r}/parameter.ts:1:38: 1 group, 2 occurrences in 2 files; coverage member, complete
group 1: run, declared at {r}/parameter.ts:1:45
  {r}/parameter.ts:1:45 declaration
  {r}/use.ts:7:18 use"
            ),
        ),
        (
            "shared.ts",
            format!("search Shared.run with --at {r}/shared.ts:1:35"),
            format!(
                "\
{r}/tsconfig.json: at {r}/shared.ts:1:35: 1 group, 2 occurrences in 2 files; coverage member, complete
group 1: run, declared at {r}/shared.ts:1:42
  {r}/shared.ts:1:42 declaration
  {r}/use.ts:10:15 use"
            ),
        ),
        (
            "anonparam.ts",
            format!("search run with --at {r}/anonparam.ts:1:36"),
            format!(
                "\
{r}/tsconfig.json: at {r}/anonparam.ts:1:36: 1 group, 2 occurrences in 2 files; coverage member, complete
group 1: run, declared at {r}/anonparam.ts:1:43
  {r}/anonparam.ts:1:43 declaration
  {r}/use.ts:12:18 use"
            ),
        ),
    ] {
        let suggested = both(&service, &["--references", "--name", "run", file]);
        assert_eq!(suggested.code, Some(0), "{file}: {}", suggested.stderr);
        assert!(
            suggested.stdout.ends_with(&format!(
                "\n1 catalog member named run was not searched; {hint_line}\n"
            )),
            "{file}: {}",
            suggested.stdout
        );
        result(follow(&service, hint(&suggested.stdout)), 0, &searched);
    }
}

/// Each suggestion names the position that searches its member, beside the
/// catalog anchor that `--show --at` selects: for a parameter property, its
/// modifier, since its name is its lexical parameter.
#[test]
fn suggestions_name_the_position_that_searches_each_member() {
    let service = member_hints();
    let suggested = both(
        &service,
        &["--references", "--name", "run", "./parameter.ts", "--json"],
    );
    assert_eq!(suggested.code, Some(0), "{}", suggested.stderr);
    let document: Value = serde_json::from_str(&suggested.stdout).unwrap();
    assert_eq!(
        document["unsearchedMembers"],
        json!({"count": 1, "selectors": [{
            "file": "parameter.ts",
            "selector": "Parameter.run",
            "anchor": {"file": "parameter.ts", "line": 1, "column": 45},
            "search": {"file": "parameter.ts", "line": 1, "column": 38},
        }]})
    );
    // In this order, after the anchor.
    let entry = suggested.stdout.split("\"selectors\":[").nth(1).unwrap();
    assert!(
        entry.starts_with(
            "{\"file\":\"parameter.ts\",\"selector\":\"Parameter.run\",\"anchor\":{\"file\":\"parameter.ts\",\"line\":1,\"column\":45},\"search\":{\"file\":\"parameter.ts\",\"line\":1,\"column\":38}}]"
        ),
        "{entry}"
    );
    let method = both(
        &service,
        &["--references", "--name", "run", "anon.ts", "--json"],
    );
    let document: Value = serde_json::from_str(&method.stdout).unwrap();
    assert_eq!(
        document["unsearchedMembers"]["selectors"][0],
        json!({
            "file": "anon.ts",
            "selector": "run",
            "anchor": {"file": "anon.ts", "line": 1, "column": 24},
            "search": {"file": "anon.ts", "line": 1, "column": 24},
        })
    );
}

/// The hint repeats the request's filters, so following it answers the
/// same question of the member.
#[test]
fn a_member_hint_keeps_the_filters() {
    let service = members();
    let r = &root(&service);
    let suggested = both(
        &service,
        &[
            "--references",
            "--name",
            "value",
            "--no-tests",
            "--file",
            "cases.ts",
        ],
    );
    assert_eq!(suggested.code, Some(0), "{}", suggested.stderr);
    assert!(
        suggested.stdout.ends_with(&format!(
            "\n2 catalog members named value were not searched; search Box.value with --at {r}/cases.ts:2:36 --no-tests --file {r}/cases.ts\n"
        )),
        "{}",
        suggested.stdout
    );
    let followed = follow(&service, hint(&suggested.stdout));
    assert_eq!(followed.code, Some(1), "{}", followed.stderr);
    assert!(
        followed.stdout.starts_with(&format!(
            "{r}/tsconfig.json: at {r}/cases.ts:2:36: 1 group, 3 occurrences in 1 file; filtered by --no-tests, --file {r}/cases.ts; coverage member, partial\n"
        )),
        "{}",
        followed.stdout
    );
}

#[test]
fn a_member_without_a_selector_is_named_by_its_anchor() {
    let service = Service::new(&[("a.ts", "export const nested = { 'a.b': { run() {} } };\n")]);
    let r = &root(&service);
    let suggested = both(&service, &["--references", "--name", "run"]);
    let stdout = suggested.stdout.clone();
    result(
        suggested,
        0,
        &format!(
            "\
{r}/tsconfig.json: `run` 0 groups, 0 occurrences in 0 files; coverage lexical, complete
1 catalog member named run was not searched; search it with --at {r}/a.ts:1:34"
        ),
    );
    result(
        follow(&service, hint(&stdout)),
        0,
        &format!(
            "\
{r}/tsconfig.json: at {r}/a.ts:1:34: 1 group, 1 occurrence in 1 file; coverage member, complete
group 1: run, declared at {r}/a.ts:1:34
  {r}/a.ts:1:34 declaration"
        ),
    );
}

/// Every reporting filter is stated where the counts it bounds are.
#[test]
fn filters_are_stated_with_the_counts() {
    let service = aliases();
    let r = &root(&service);
    result(
        both(
            &service,
            &["--references", "--name", "run", "--calls", "--no-tests"],
        ),
        0,
        &format!(
            "\
{r}/tsconfig.json: `run` 1 group, 1 occurrence in 1 file; filtered by --calls, --no-tests; coverage lexical, complete
group 1: run, declared at {r}/api.ts:2:28
  {r}/app.ts:3:11 use (start)"
        ),
    );
}

#[test]
fn ambiguous_and_missing_selections_name_the_selection() {
    let service = selections();
    let r = &root(&service);
    unselected(
        both(&service, &["--references", "--name", "Both.run"]),
        &format!(
            "\
{r}/tsconfig.json: `Both.run` matches 2 symbols; select one with --at FILE:LINE:COLUMN
  {r}/cases.ts:7:10 Both.run
  {r}/cases.ts:8:3 Both.run"
        ),
    );
    unselected(
        both(&service, &["--references", "--name", "Both.run", "cases.ts"]),
        &format!(
            "\
{r}/tsconfig.json: `Both.run` matches 2 symbols; candidates narrowed to {r}/cases.ts; select one with --at FILE:LINE:COLUMN
  {r}/cases.ts:7:10 Both.run
  {r}/cases.ts:8:3 Both.run"
        ),
    );
    unselected(
        both(&service, &["--references", "--name", "Absent.run"]),
        &format!("{r}/tsconfig.json: no declaration matches `Absent.run`"),
    );
    unselected(
        both(
            &service,
            &["--references", "--name", "Absent.run", "cases.ts"],
        ),
        &format!(
            "{r}/tsconfig.json: no declaration matches `Absent.run`; candidates narrowed to {r}/cases.ts"
        ),
    );
    // A candidate's location selects it.
    let candidate = both(
        &service,
        &["--references", "--at", &format!("{r}/cases.ts:8:3")],
    );
    assert_eq!(candidate.code, Some(0), "{}", candidate.stderr);
    assert!(
        candidate
            .stdout
            .starts_with(&format!("{r}/tsconfig.json: at {r}/cases.ts:8:3: 1 group,")),
        "{}",
        candidate.stdout
    );
}

/// `--pretty` without `--json` keeps the indented document, of a result and
/// of a selection that selected nothing, which `--json` writes compactly.
#[test]
fn pretty_alone_keeps_the_indented_document() {
    let service = selections();
    for (args, code) in [
        (&["--references", "--name", "Overloaded.run"][..], 0),
        (&["--references", "--name", "Both.run"], 1),
        (&["--references", "--name", "Absent.run"], 1),
    ] {
        let pretty = both(&service, &[args, &["--pretty"]].concat());
        let json = both(&service, &[args, &["--json"]].concat());
        assert_eq!(pretty.code, Some(code), "{args:?}");
        assert_eq!(json.code, Some(code), "{args:?}");
        assert_eq!(pretty.stderr, "");
        assert_eq!(json.stderr, "");
        assert_eq!(
            serde_json::from_str::<Value>(&pretty.stdout).unwrap(),
            serde_json::from_str::<Value>(&json.stdout).unwrap(),
            "{args:?}"
        );
        assert_eq!(keys(&pretty.stdout), keys(&json.stdout), "{args:?}");
        assert!(
            pretty
                .stdout
                .starts_with("{\n  \"operation\": \"references\",\n")
        );
        assert_eq!(json.stdout.matches('\n').count(), 1, "{args:?}");
    }
}

/// The keys of a document, in the order it writes them.
fn keys(document: &str) -> Vec<String> {
    struct Keys(Vec<String>);
    impl<'de> serde::Deserialize<'de> for Keys {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Keys;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("an object")
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> Result<Keys, A::Error> {
                    let mut keys = Vec::new();
                    while let Some((key, _)) = map.next_entry::<String, serde::de::IgnoredAny>()? {
                        keys.push(key);
                    }
                    Ok(Keys(keys))
                }
            }
            deserializer.deserialize_map(Visitor)
        }
    }
    serde_json::from_str::<Keys>(document).unwrap().0
}

/// A result echoes its selection after `project`, tagged in snake case,
/// with its file named relative to the configuration's directory as every
/// location is, whatever the working directory.
#[test]
fn results_echo_the_selection_after_the_project() {
    let service = aliases();
    let named = both(
        &service,
        &["--references", "--name", "start", "./app.ts", "--json"],
    );
    assert_eq!(named.code, Some(0));
    assert_eq!(
        keys(&named.stdout),
        [
            "operation",
            "outcome",
            "project",
            "selection",
            "groups",
            "filters",
            "coverage",
            "unsearchedMembers"
        ]
    );
    let document: Value = serde_json::from_str(&named.stdout).unwrap();
    assert_eq!(
        document["selection"],
        json!({"name": {"name": "start", "file": "app.ts"}})
    );
    let bare = both(&service, &["--references", "--name", "start", "--json"]);
    let document: Value = serde_json::from_str(&bare.stdout).unwrap();
    assert_eq!(
        document["selection"],
        json!({"name": {"name": "start", "file": null}})
    );
    let at = both(
        &service,
        &["--references", "--at", "3:11", "app.ts", "--json"],
    );
    let document: Value = serde_json::from_str(&at.stdout).unwrap();
    assert_eq!(
        document["selection"],
        json!({"at": {"file": "app.ts", "position": {"line": 3, "column": 11}}})
    );
    // From another directory, the echo is the same.
    let root = &service.fixture.root;
    let elsewhere = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .current_dir(root.parent().unwrap())
        .args(["--references", "--direct", "--json", "--project"])
        .arg(root.join("tsconfig.json"))
        .args(["--name", "start"])
        .arg(root.join("app.ts"))
        .output()
        .unwrap();
    assert_eq!(elsewhere.status.code(), Some(0));
    assert_eq!(String::from_utf8(elsewhere.stdout).unwrap(), named.stdout);
}

/// A selection that selected nothing echoes it in the same form.
#[test]
fn unselected_documents_echo_the_selection_in_snake_case() {
    let service = selections();
    let missing = both(
        &service,
        &[
            "--references",
            "--name",
            "Absent.run",
            "./cases.ts",
            "--json",
        ],
    );
    assert_eq!(missing.code, Some(1));
    let document: Value = serde_json::from_str(&missing.stdout).unwrap();
    assert_eq!(
        document,
        json!({
            "operation": "references",
            "project": service.fixture.root.join("tsconfig.json"),
            "selection": {"name": {"name": "Absent.run", "file": "cases.ts"}},
            "outcome": "not_found",
            "candidates": [],
        })
    );
    assert_eq!(
        keys(&missing.stdout),
        ["operation", "project", "selection", "outcome", "candidates"]
    );
    let ambiguous = both(&service, &["--references", "--name", "Both.run", "--json"]);
    let document: Value = serde_json::from_str(&ambiguous.stdout).unwrap();
    assert_eq!(
        document["selection"],
        json!({"name": {"name": "Both.run", "file": null}})
    );
    assert_eq!(document["outcome"], "ambiguous");
}
