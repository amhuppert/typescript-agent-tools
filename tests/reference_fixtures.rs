//! Checked-in marker fixtures under `fixtures/references`, whose markers the
//! semantic oracle validates against TypeScript, and the marker grammar.
mod support;

use std::collections::BTreeSet;
use support::{Expect, Fixture, fixtures_root, parse_markers};
use typescript_parser::references::{Role, SourcePosition};

#[test]
fn every_marked_fixture_project_matches_its_markers() {
    let root = fixtures_root();
    let mut directories: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    directories.sort();
    assert!(
        !directories.is_empty(),
        "no fixture projects in {}",
        root.display()
    );
    let mut failures = Vec::new();
    for directory in &directories {
        if !directory.join("tsconfig.json").is_file() {
            failures.push(format!("{}: no tsconfig.json", directory.display()));
            continue;
        }
        let fixture = Fixture::existing(directory);
        let outcome = std::panic::catch_unwind(|| fixture.check_markers(&fixture.load()));
        if let Err(panic) = outcome {
            failures.push(format!("{}: {}", directory.display(), message(&*panic)));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} fixtures failed:\n{}",
        failures.len(),
        directories.len(),
        failures.join("\n\n")
    );
}

fn message(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default()
}

fn at(line: usize, column: usize) -> SourcePosition {
    SourcePosition { line, column }
}

/// The cases of the matching JavaScript parser test in
/// tools/semantic-oracle.test.mjs, so both harnesses read one grammar.
#[test]
fn markers_follow_the_shared_grammar() {
    let text = "const /*@a:de*/x = 1;\r\n/*@a,b:u*/x; /*@!*/\\u0061b; /*@?*/\\u{1D465}y;\u{2028}/*@c*/\u{1D465}_$ + 1;";
    let markers = parse_markers(text, "f.ts");
    let found: Vec<_> = markers
        .iter()
        .map(|m| (m.name.as_str(), m.location.start, m.location.end))
        .collect();
    assert_eq!(
        found,
        [
            ("x", at(1, 16), at(1, 17)),
            ("x", at(2, 11), at(2, 12)),
            ("ab", at(2, 20), at(2, 27)),
            ("\u{1D465}y", at(2, 35), at(2, 45)),
            ("\u{1D465}_$", at(3, 7), at(3, 11)),
        ]
    );
    let groups = |labels: &[&str], roles: Option<&[Role]>| Expect::Groups {
        labels: labels.iter().map(|label| label.to_string()).collect(),
        roles: roles.map(|roles| roles.iter().copied().collect::<BTreeSet<_>>()),
    };
    let expected = [
        groups(&["a"], Some(&[Role::Declaration, Role::Export])),
        groups(&["a", "b"], Some(&[Role::Use])),
        Expect::Unsupported,
        Expect::Unresolved,
        groups(&["c"], None),
    ];
    assert_eq!(
        markers.iter().map(|m| &m.expect).collect::<Vec<_>>(),
        expected.iter().collect::<Vec<_>>()
    );
    assert!(markers.iter().all(|m| m.location.file == "f.ts"));
    let private = parse_markers("class C { /*@!*/#count = 0 }", "f.ts");
    assert_eq!(
        (
            private[0].name.as_str(),
            private[0].location.start,
            private[0].location.end
        ),
        ("#count", at(1, 17), at(1, 23))
    );
}

#[test]
fn malformed_markers_are_rejected_with_their_location() {
    for (text, expected) in [
        ("/*@a*/ x", "marker at f.ts:1:1 precedes no identifier"),
        ("x;\n/*@a:z*/x", "unknown role z at f.ts:2:1"),
        ("/*@a", "unclosed marker at f.ts:1:1"),
        ("/*@a,*/x", "empty marker label at f.ts:1:1"),
        ("/*@:d*/x", "empty marker label at f.ts:1:1"),
    ] {
        let panic = std::panic::catch_unwind(|| parse_markers(text, "f.ts"))
            .expect_err(&format!("{text:?} should be rejected"));
        assert_eq!(message(&*panic), expected, "{text:?}");
    }
}
