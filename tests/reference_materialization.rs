//! Resource regressions for the public, owned reference results. Count only
//! allocations on the querying thread, after project initialization.
mod support;

#[path = "support/allocations.rs"]
mod allocations;

use allocations::counted;
use support::{Expect, Fixture, project};
use typescript_parser::references::{
    NamedLocation, Occurrence, ReferenceGroup, ReferenceQuery, ReferenceResult, Role,
    SourcePosition,
};

#[test]
fn large_results_allocate_for_owned_output_without_duplicate_location_keys() {
    let uses = 3_000;
    let source = format!("export const x = 1;\n{}", "x;\n".repeat(uses));
    let fixture = project(&[("app.ts", &source)]);
    let project = fixture.load();
    let query = ReferenceQuery::Name {
        name: "x".into(),
        file: None,
    };
    let (result, allocations) = counted(|| project.find_references(query).unwrap());
    assert_eq!(result.groups.len(), 1);
    let group = &result.groups[0];
    assert_eq!(group.declarations.len(), 1);
    assert_eq!(group.occurrences.len(), uses + 1);
    assert_eq!(
        group.occurrences[0].roles,
        [Role::Declaration, Role::Export]
    );
    for (index, occurrence) in group.occurrences.iter().enumerate().skip(1) {
        assert_eq!(occurrence.name, "x");
        assert_eq!(occurrence.location.file, "app.ts");
        assert_eq!(
            occurrence.location.start,
            SourcePosition {
                line: index + 1,
                column: 1
            }
        );
        assert_eq!(
            occurrence.location.end,
            SourcePosition {
                line: index + 1,
                column: 2
            }
        );
        assert_eq!(occurrence.roles, [Role::Use]);
    }
    // Each owned occurrence needs a filename, name, and roles allocation.
    // The allowance covers result vectors, the declaration, and selection.
    assert!(
        allocations <= 3 * group.occurrences.len() + 64,
        "{allocations} allocations for {} occurrences",
        group.occurrences.len()
    );
}

#[test]
fn many_groups_do_not_allocate_while_comparing_their_order() {
    let groups = 128;
    let source: String = (0..groups)
        .map(|i| format!("function f{i}() {{ const x = 1; x; }}\n"))
        .collect();
    let fixture = project(&[("app.ts", &source)]);
    let project = fixture.load();
    let query = ReferenceQuery::Name {
        name: "x".into(),
        file: None,
    };
    let (result, allocations) = counted(|| project.find_references(query).unwrap());
    assert_eq!(result.groups.len(), groups);
    for (index, group) in result.groups.iter().enumerate() {
        assert_eq!(group.declarations.len(), 1);
        assert_eq!(group.declarations[0].location.start.line, index + 1);
        assert_eq!(group.occurrences.len(), 2);
        assert_eq!(group.occurrences[0].roles, [Role::Declaration]);
        assert_eq!(group.occurrences[1].roles, [Role::Use]);
    }
    // Two vectors and two declaration strings per group, plus three owned
    // fields per occurrence. Selection and the outer vector get an allowance.
    assert!(
        allocations <= groups * (2 + 2 + 3 * 2) + 64,
        "{allocations} allocations for {groups} groups"
    );
}

#[test]
fn frozen_results_keep_utf16_locations_aliases_roles_and_file_order() {
    for newline in ["\r", "\n", "\r\n", "\u{2028}", "\u{2029}"] {
        let source = format!(
            "\u{feff}{newline}export const /*@x:de*/𝒳 = 1;{newline}\t/*@x:u*/𝒳;{newline}export {{ /*@x:e*/𝒳 as /*@x:e*/alias }};"
        );
        // Reservation order follows files, while public result order follows
        // filenames. The alias and astral identifier denote the same symbol.
        let fixture = Fixture::new(&[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "noLib": true }, "files": ["z.ts", "a.ts"] }"#,
            ),
            ("z.ts", &source),
            (
                "a.ts",
                "import { /*@x:i*/alias } from './z'; /*@x:u*/alias;",
            ),
        ]);
        let project = fixture.load();
        fixture.check_markers(&project);
        fixture.check_name(&project, "alias", None, &["x"]);
        let result = project
            .find_references(ReferenceQuery::Name {
                name: "𝒳".into(),
                file: None,
            })
            .unwrap();
        assert_eq!(result.groups[0].occurrences[0].location.file, "a.ts");
        let snapshot = result.clone();
        std::fs::write(fixture.path("z.ts"), "invalid later contents").unwrap();
        assert_eq!(
            project
                .find_references(ReferenceQuery::Name {
                    name: "𝒳".into(),
                    file: None
                })
                .unwrap(),
            snapshot
        );
    }
}

#[test]
fn shared_type_and_value_targets_return_exact_ordered_groups_from_both_entry_points() {
    // The pinned TypeScript oracle verifies this fixture's group membership,
    // locations, decoded names, and roles. Construct the complete public
    // vectors from those expectations, without normalizing away duplicates
    // or ordering in the tool's actual result.
    let fixture = Fixture::checked_in("type-value-meanings");
    let project = fixture.load();
    let markers = fixture.markers();
    let mut groups = Vec::new();
    for label in ["typeBox", "valueBox"] {
        let mut declarations = Vec::new();
        let mut occurrences = Vec::new();
        for marker in &markers {
            let Expect::Groups { labels, roles } = &marker.expect else {
                continue;
            };
            if !labels.iter().any(|found| found == label) {
                continue;
            }
            let roles: Vec<_> = roles.as_ref().unwrap().iter().copied().collect();
            if roles.contains(&Role::Declaration) {
                declarations.push(NamedLocation {
                    name: marker.name.clone(),
                    location: marker.location.clone(),
                });
            }
            occurrences.push(Occurrence {
                name: marker.name.clone(),
                location: marker.location.clone(),
                roles,
            });
        }
        groups.push(ReferenceGroup {
            declarations,
            occurrences,
        });
    }
    let expected = ReferenceResult {
        project: fixture.path("tsconfig.json"),
        groups,
    };
    let shared = markers
        .iter()
        .find(|marker| {
            matches!(&marker.expect, Expect::Groups { labels, .. }
            if labels.iter().any(|label| label == "typeBox")
                && labels.iter().any(|label| label == "valueBox"))
        })
        .unwrap();
    for query in [
        ReferenceQuery::Name {
            name: "Box".into(),
            file: None,
        },
        ReferenceQuery::At {
            file: fixture.path(&shared.location.file),
            position: shared.location.start,
        },
    ] {
        assert_eq!(project.find_references(query).unwrap(), expected);
    }
}
