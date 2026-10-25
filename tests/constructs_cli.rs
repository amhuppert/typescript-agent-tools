//! `--constructs` through the command line: text and documents, exit codes,
//! the site and lacking-property cuts with the command that reveals them,
//! and service answers that are the bytes of direct ones.
#![cfg(unix)]
mod support;

use serde_json::{Value, json};
use support::Fixture;
use support::service::Service;

fn service() -> Service {
    Service::of(Fixture::copy_from_repository("fixtures/types/constructs"))
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn text_and_documents_name_each_site_and_what_it_lacks() {
    let service = service();
    let root = service.fixture.root.display().to_string();
    let output = service.both(&["--constructs", "--name", "Pair"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        stdout(&output),
        format!(
            "{root}/tsconfig.json: `Pair` 1 construction site in 1 file; 12 of 133 literals examined; coverage complete\n\
             target Pair, declared at {root}/definitions.ts:3:13\n\
             {root}/cases.ts:44:27 lacks \"right\"?\n\
             121 literals naming only keys the target does not declare were not examined; examine them with --all-literals\n"
        )
    );
    // The flag examines them: a union member chosen by other keys.
    let output = service.both(&["--constructs", "--name", "Pair", "--all-literals"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        format!(
            "{root}/tsconfig.json: `Pair` 2 construction sites in 2 files; 133 literals examined; coverage complete\n\
             target Pair, declared at {root}/definitions.ts:3:13\n\
             {root}/cases.ts:44:27 lacks \"right\"?\n\
             {root}/universe.ts:9:40 in Pair | View lacks \"left\", \"right\"?\n"
        )
    );

    // A union context, a generic instantiation and an unknown presence.
    let output = service.both(&["--constructs", "--name", "Box"]);
    let text = stdout(&output);
    assert!(
        text.contains(&format!(
            "\n{root}/cases.ts:42:33 as Box<string> lacks \"label\"?\n"
        )),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "\n{root}/cases.ts:43:38 as Box<number> lacks nothing\n"
        )),
        "{text}"
    );
    let output = service.both(&["--constructs", "--name", "View"]);
    let text = stdout(&output);
    assert!(
        text.contains(&format!(
            "\n{root}/cases.ts:18:62 in PromiseLike<View> | View lacks \"lane\"?, \"note\"?\n"
        )),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "\n{root}/cases.ts:29:33 lacks \"lane\"? (unknown), \"note\"? (unknown)\n"
        )),
        "{text}"
    );

    let document: Value = serde_json::from_slice(
        &service
            .both(&["--constructs", "--name", "Pair", "--json"])
            .stdout,
    )
    .unwrap();
    assert_eq!(document["operation"], "constructs");
    assert_eq!(document["outcome"], "complete");
    assert_eq!(
        document["selection"],
        json!({"Name": {"name": "Pair", "file": null}})
    );
    assert_eq!(
        document["target"]["alternatives"],
        json!([{
            "id": "a0",
            "summary": "Pair",
            "summaryTruncated": false,
            "origin": {"file": "definitions.ts", "start": {"line": 3, "column": 13}, "end": {"line": 3, "column": 17}},
        }])
    );
    assert_eq!(
        document["sites"],
        json!([{
            "location": {"file": "cases.ts", "start": {"line": 44, "column": 27}, "end": {"line": 44, "column": 38}},
            "contextualType": {"summary": "Pair", "summaryTruncated": false},
            "constructs": [{
                "alternative": "a0",
                "summary": "Pair",
                "summaryTruncated": false,
                "lacks": [{"key": {"kind": "string", "name": "right"}, "optional": true, "presence": "absent"}],
            }],
        }])
    );
    assert_eq!(
        document["coverage"],
        json!({"status": "complete", "universe": "target_keys", "literals": 133, "examined": 12, "sites": 1, "gaps": []})
    );
    assert_eq!(document["filters"]["allLiterals"], false);
    assert!(document.get("display").is_none());
    let document: Value = serde_json::from_slice(
        &service
            .both(&["--constructs", "--name", "Pair", "--all-literals", "--json"])
            .stdout,
    )
    .unwrap();
    assert_eq!(document["filters"]["allLiterals"], true);
    assert_eq!(document["coverage"]["universe"], "all");
    assert_eq!(document["coverage"]["examined"], 133);
}

#[test]
fn text_spells_each_key_exactly_and_missing_takes_any_declared_key() {
    let service = service();
    let root = service.fixture.root.display().to_string();
    // Keys are quoted, so a name's own `?`, parentheses, newline or emptiness
    // is not read as a marker.
    let text = stdout(&service.both(&["--constructs", "--name", "TextKeys"]));
    assert!(
        text.contains(&format!(
            "\n{root}/keys.ts:11:31 lacks \"lane?\", \"status (unknown)\", \"x\\ny\", \"\"\n"
        )),
        "{text}"
    );
    let text = stdout(&service.both(&["--constructs", "--name", "Secret"]));
    assert!(
        text.contains(&format!(
            "\n{root}/keys.ts:8:31 lacks private \"#hidden\"\n"
        )),
        "{text}"
    );
    // The empty string is a key TextKeys declares, echoed quoted.
    let text = stdout(&service.both(&["--constructs", "--name", "TextKeys", "--missing", ""]));
    assert!(
        text.lines()
            .next()
            .unwrap()
            .contains("1 construction site (missing \"\") in 1 file"),
        "{text}"
    );
    let output = service.both(&[
        "--constructs",
        "--name",
        "TextKeys",
        "--missing",
        "",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["filters"]["missing"], "");
    assert_eq!(document["sites"].as_array().unwrap().len(), 1);
    // A private name is not a string-keyed property.
    let output = service.both(&["--constructs", "--name", "Secret", "--missing", "#hidden"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("no alternative of the target declares the string-keyed property `#hidden`"),
        "{output:?}"
    );
}

#[test]
fn cuts_say_what_they_withhold_and_how_to_see_it() {
    let service = service();
    let output = service.both(&["--constructs", "--name", "View", "--limit", "2"]);
    assert_eq!(output.status.code(), Some(0));
    let text = stdout(&output);
    let last = text.lines().last().unwrap();
    assert!(
        last.starts_with(
            "… 39 more sites; see all with: tsparse --constructs --name View --limit all"
        ),
        "{last}"
    );
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[2].ends_with("calls.ts:15:15 lacks \"lane\"?, \"note\"?"),
        "{text}"
    );
    assert!(!text.contains("lacking propert"), "{text}");
    // `--limit 1` also shows one lacking property of each construction.
    let text = stdout(&service.both(&["--constructs", "--name", "View", "--limit", "1"]));
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[2].ends_with("calls.ts:15:15 lacks \"lane\"?, … 1 more"),
        "{text}"
    );
    assert!(
        lines[3].ends_with("were not examined; examine them with --all-literals"),
        "{text}"
    );
    assert!(
        lines[4].starts_with("… 1 more lacking property across the sites shown; see all with: "),
        "{text}"
    );
    assert!(
        lines[5].starts_with("… 40 more sites; see all with: "),
        "{text}"
    );

    let document: Value = serde_json::from_slice(
        &service
            .both(&["--constructs", "--name", "View", "--limit", "1", "--json"])
            .stdout,
    )
    .unwrap();
    assert_eq!(document["sites"].as_array().unwrap().len(), 1);
    let construction = &document["sites"][0]["constructs"][0];
    assert_eq!(construction["lacks"].as_array().unwrap().len(), 1);
    assert_eq!(construction["omittedLacks"], 1);
    assert_eq!(
        document["display"]["sites"],
        json!({"total": 41, "returned": 1})
    );
    assert_eq!(
        document["display"]["lacks"],
        json!({"total": 2, "returned": 1})
    );
    assert_eq!(document["display"]["truncated"], true);
    assert_eq!(document["coverage"]["sites"], 41);
    let argv: Vec<&str> = document["display"]["next"]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert!(
        argv.windows(2).any(|pair| pair == ["--limit", "all"]),
        "{argv:?}"
    );

    // The reveal shows everything.
    let all: Value = serde_json::from_slice(
        &service
            .both(&["--constructs", "--name", "View", "--limit", "all", "--json"])
            .stdout,
    )
    .unwrap();
    assert_eq!(all["sites"].as_array().unwrap().len(), 41);
    assert!(all.get("display").is_none());
}

#[test]
fn partial_unselected_and_refused_answers_exit_one() {
    let service = Service::of(Fixture::copy_from_repository(
        "fixtures/types/constructs-gaps",
    ));
    let output = service.both(&["--constructs", "--name", "View"]);
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    assert!(
        text.lines().next().unwrap().ends_with(
            "coverage partial (5 gaps: 2 flow_required, 2 unsupported_semantic_form, 1 overload_recovery)"
        ),
        "{text}"
    );
    assert_eq!(text.matches("\ngap ").count(), 5, "{text}");
    // Gaps are a bounded list too; the library result keeps every one.
    let output = service.both(&["--constructs", "--name", "View", "--limit", "2"]);
    let text = stdout(&output);
    assert_eq!(text.matches("\ngap ").count(), 2, "{text}");
    assert!(
        text.lines().any(|line| line.starts_with(
            "… 3 more gaps; see all with: tsparse --constructs --name View --limit all"
        )),
        "{text}"
    );
    let document: Value = serde_json::from_slice(
        &service
            .both(&["--constructs", "--name", "View", "--limit", "2", "--json"])
            .stdout,
    )
    .unwrap();
    assert_eq!(document["coverage"]["gaps"].as_array().unwrap().len(), 2);
    assert_eq!(
        document["display"]["gaps"],
        json!({"total": 5, "returned": 2})
    );
    assert_eq!(document["outcome"], "partial");

    let service = self::service();
    let output = service.both(&["--constructs", "--name", "Nowhere"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .ends_with("no class, interface or type alias matches `Nowhere`\n"),
        "{output:?}"
    );

    let output = service.both(&[
        "--constructs",
        "--name",
        "View",
        "--missing",
        "lanes",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["outcome"], "error");
    assert_eq!(document["error"]["kind"], "UnknownProperty");
    assert!(
        document["error"]["message"]
            .as_str()
            .unwrap()
            .ends_with("did you mean `lane`?")
    );

    let output = service.both(&["--constructs", "--name", "returns"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("select a class, interface or type alias"),
        "{output:?}"
    );

    // Usage failures exit 2 before any project work.
    for args in [
        &["--constructs", "--name", "View", "--file", "-"][..],
        &["--constructs"],
    ] {
        let output = service.run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
    }
}
