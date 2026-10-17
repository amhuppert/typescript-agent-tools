//! Final reference answers pinned by type-member-imports-oracle.test.mjs.
mod support;
use serde_json::{Value, json};
use std::process::Command;
use support::{Fixture, position};

const FILES: &[(&str, &str)] = &[
    (
        "factory.ts",
        include_str!("../fixtures/types/member-imports/factory.ts"),
    ),
    (
        "default.ts",
        include_str!("../fixtures/types/member-imports/default.ts"),
    ),
    (
        "barrel.ts",
        include_str!("../fixtures/types/member-imports/barrel.ts"),
    ),
    (
        "use.ts",
        include_str!("../fixtures/types/member-imports/use.ts"),
    ),
    (
        "consumer.test.ts",
        include_str!("../fixtures/types/member-imports/consumer.test.ts"),
    ),
    (
        "globals.d.ts",
        include_str!("../fixtures/types/member-imports/globals.d.ts"),
    ),
    (
        "tsconfig.json",
        include_str!("../fixtures/types/member-imports/tsconfig.json"),
    ),
];

fn location(file: &str, marker: &str, length: usize) -> Value {
    let source = FILES.iter().find(|(name, _)| *name == file).unwrap().1;
    let tag = format!("/*#{marker}*/");
    let offset = source.find(&tag).unwrap() + tag.len();
    json!({"file":file,"start":position(source,offset),"end":position(source,offset+length)})
}

fn group(origin: Value, occurrences: Vec<Value>) -> Value {
    json!({"origins":[origin],"occurrences":occurrences})
}

fn expected(equals: bool, calls: bool, no_tests: bool) -> Vec<Value> {
    let mut groups = Vec::new();
    if !equals && !calls {
        groups.push(group(
            location("barrel.ts", "reexportName", 5),
            vec![
                location("barrel.ts", "reexportName", 5),
                location("use.ts", "namedSource", 5),
            ],
        ));
    }
    if !no_tests {
        let (alias, call, length) = if equals {
            ("testEqualsAlias", "testEqualsCall", 7)
        } else {
            ("testDefaultAlias", "testDefaultCall", 6)
        };
        let mut occurrences = Vec::new();
        if !calls {
            occurrences.push(location("consumer.test.ts", alias, length));
        }
        occurrences.push(location("consumer.test.ts", call, length));
        groups.push(group(
            location("consumer.test.ts", alias, length),
            occurrences,
        ));
    }
    let (file, member, exported) = if equals {
        ("factory.ts", "equalsMember", "equalsExport")
    } else {
        ("default.ts", "defaultMember", "defaultExport")
    };
    let mut occurrences = Vec::new();
    if !calls {
        if !equals {
            occurrences.push(location("barrel.ts", "reexportSource", 7));
        }
        occurrences.extend([location(file, member, 3), location(file, exported, 3)]);
    }
    // The selected member's group remains even when only imported calls survive.
    groups.push(group(location(file, member, 3), occurrences));
    let (alias, call, length) = if equals {
        ("equalsAlias", "equalsCall", 7)
    } else {
        ("defaultAlias", "defaultCall", 6)
    };
    let mut occurrences = Vec::new();
    if !calls {
        occurrences.push(location("use.ts", alias, length));
    }
    occurrences.push(location("use.ts", call, length));
    if equals && !calls {
        occurrences.push(location("use.ts", "equalsSaved", 7));
    }
    groups.push(group(location("use.ts", alias, length), occurrences));
    if !equals {
        let mut occurrences = Vec::new();
        if !calls {
            occurrences.push(location("use.ts", "namedAlias", 7));
        }
        occurrences.push(location("use.ts", "renamedCall", 7));
        groups.push(group(location("use.ts", "namedAlias", 7), occurrences));
    }
    groups
}

#[test]
fn exported_members_keep_alias_calls_reachable_through_filtered_headers() {
    let fixture = Fixture::new(FILES);
    for equals in [true, false] {
        let (file, markers) = if equals {
            ("factory.ts", ["equalsMember", "equalsExport"])
        } else {
            ("default.ts", ["defaultMember", "defaultExport"])
        };
        for marker in markers {
            let selected = location(file, marker, 3);
            let at = format!(
                "{}:{}:{}",
                fixture.path(file).display(),
                selected["start"]["line"],
                selected["start"]["column"]
            );
            for calls in [false, true] {
                for no_tests in [false, true] {
                    let mut command = Command::new(env!("CARGO_BIN_EXE_tsparse"));
                    command
                        .args(["--references", "--direct", "--json", "--project"])
                        .arg(fixture.path("tsconfig.json"))
                        .args(["--at", &at]);
                    if calls {
                        command.arg("--calls");
                    }
                    if no_tests {
                        command.arg("--no-tests");
                    }
                    let output = command.output().unwrap();
                    let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
                    assert!(
                        output.status.success(),
                        "{marker} calls={calls} no_tests={no_tests}: {answer:#}"
                    );
                    assert_eq!(answer["outcome"], "complete");
                    assert_eq!(answer["coverage"]["baseline"], "member");
                    assert_eq!(answer["coverage"]["gaps"], json!([]));
                    let actual = answer["groups"].as_array().unwrap().iter().map(|group| json!({
                        "origins":group["declarations"].as_array().unwrap().iter().map(|origin| origin["location"].clone()).collect::<Vec<_>>(),
                        "occurrences":group["occurrences"].as_array().unwrap().iter().map(|occurrence| occurrence["location"].clone()).collect::<Vec<_>>(),
                    })).collect::<Vec<_>>();
                    assert_eq!(
                        actual,
                        expected(equals, calls, no_tests),
                        "{marker} calls={calls} no_tests={no_tests}"
                    );
                }
            }
        }
    }
}
