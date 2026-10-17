//! Final Q3 controls derived from the pinned shared-checker member-search hook.
mod support;
use serde_json::{Value, json};
use std::process::Command;
use support::{Fixture, position};

const SOURCE: &str = include_str!("../fixtures/types/member-search-core/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/member-search-core/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/member-search-core/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/member-search-core/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/member-search-core/exact.json"),
        ),
    ])
}
fn location(marker: &str, length: usize) -> Value {
    location_from(marker, 0, length)
}
fn location_from(marker: &str, start: usize, length: usize) -> Value {
    let tag = format!("/*#{marker}*/");
    let offset = SOURCE.find(&tag).unwrap() + tag.len() + start;
    json!({"file":"cases.ts","start":position(SOURCE,offset),"end":position(SOURCE,offset+length)})
}
fn query(fixture: &Fixture, config: &str, marker: &str) -> (bool, Value) {
    let tag = format!("/*#{marker}*/");
    let position = position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len());
    let at = format!(
        "{}:{}:{}",
        fixture.path("cases.ts").display(),
        position.line,
        position.column
    );
    let result = Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(["--references", "--direct", "--json", "--project"])
        .arg(fixture.path(config))
        .args(["--at", &at])
        .output()
        .unwrap();
    assert!(
        result.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    (
        result.status.success(),
        serde_json::from_slice(&result.stdout).unwrap(),
    )
}
fn groups(answer: &Value) -> Vec<Value> {
    answer["groups"].as_array().unwrap_or_else(|| panic!("{answer:#}")).iter().map(|group| {
        json!({"origins":group["declarations"].as_array().unwrap().iter().map(|d|d["location"].clone()).collect::<Vec<_>>(),
            "occurrences":group["occurrences"].as_array().unwrap().iter().map(|o|o["location"].clone()).collect::<Vec<_>>()})
    }).collect()
}
fn group(origins: Vec<Value>, occurrences: Vec<Value>) -> Value {
    json!({"origins":origins,"occurrences":occurrences})
}

#[test]
fn generic_identity_excludes_equal_members_and_accounts_for_unknown_candidates() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        for marker in ["declaration", "first", "second", "other", "otherUse"] {
            let (success, answer) = query(&fixture, config, marker);
            assert!(!success);
            assert_eq!(
                answer["outcome"], "partial",
                "{config}/{marker}: {answer:#}"
            );
            assert_eq!(answer["coverage"]["baseline"], "member");
            assert_eq!(answer["coverage"]["status"], "partial");
            let (origin, occurrences) = if marker.starts_with("other") {
                ("other", vec!["other", "otherUse"])
            } else {
                ("declaration", vec!["declaration", "first", "second"])
            };
            assert_eq!(
                groups(&answer),
                vec![group(
                    vec![location(origin, 5)],
                    occurrences.into_iter().map(|m| location(m, 5)).collect()
                )]
            );
            let gaps = answer["coverage"]["gaps"].as_array().unwrap();
            assert_eq!(gaps.len(), 1, "{answer:#}");
            assert_eq!(gaps[0]["location"], location("unknownUse", 5));
            assert_eq!(gaps[0]["reason"], "error_type");
        }
    }
}

#[test]
fn union_search_keeps_selection_direction_and_separate_synthetic_groups() {
    let fixture = fixture();
    let left = group(
        vec![location("inlineLeft", 4)],
        vec![location("inlineLeft", 4)],
    );
    let right = group(
        vec![location("inlineRight", 4)],
        vec![location("inlineRight", 4)],
    );
    let union = group(
        vec![location("inlineLeft", 4), location("inlineRight", 4)],
        vec![location("inlineUse", 4)],
    );
    let interface_left = group(
        vec![location("interfaceLeft", 4)],
        vec![location("interfaceLeft", 4)],
    );
    let interface_right = group(
        vec![location("interfaceRight", 4)],
        vec![location("interfaceRight", 4)],
    );
    let interface_union = group(
        vec![location("interfaceLeft", 4), location("interfaceRight", 4)],
        vec![location("interfaceUse", 4)],
    );
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        for marker in [
            "inlineLeft",
            "inlineRight",
            "inlineUse",
            "interfaceLeft",
            "interfaceRight",
            "interfaceUse",
        ] {
            let (success, answer) = query(&fixture, config, marker);
            assert!(success, "{config}/{marker}: {answer:#}");
            assert_eq!(
                answer["coverage"],
                json!({"baseline":"member","status":"complete","gaps":[]})
            );
            let expected = match marker {
                "interfaceLeft" => vec![interface_left.clone(), interface_union.clone()],
                "interfaceRight" => vec![interface_union.clone(), interface_right.clone()],
                "interfaceUse" => vec![
                    interface_left.clone(),
                    interface_union.clone(),
                    interface_right.clone(),
                ],
                _ => vec![left.clone(), union.clone(), right.clone()],
            };
            assert_eq!(groups(&answer), expected, "{config}/{marker}");
        }
    }
}

#[test]
fn exact_decoded_inventory_includes_escaped_and_alternate_numeric_declarations() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        for marker in [
            "escapedDeclaration",
            "escapedUse",
            "literalUse",
            "numericDeclaration",
            "numericUse",
            "numericString",
        ] {
            let (success, answer) = query(&fixture, config, marker);
            assert!(success, "{config}/{marker}: {answer:#}");
            let escaped = marker.starts_with("escaped") || marker == "literalUse";
            let declaration = if escaped {
                location("escapedDeclaration", 8)
            } else {
                location("numericDeclaration", 4)
            };
            let uses = if escaped {
                vec![
                    declaration.clone(),
                    location("escapedUse", 3),
                    location_from("literalUse", 1, 3),
                ]
            } else {
                vec![
                    declaration.clone(),
                    location("numericUse", 2),
                    location_from("numericString", 1, 2),
                ]
            };
            assert_eq!(groups(&answer), vec![group(vec![declaration], uses)]);
            assert_eq!(
                answer["coverage"],
                json!({"baseline":"member","status":"complete","gaps":[]})
            );
        }
    }
}
