//! Staging never removes a name it did not create. Alone in its test
//! binary, so this process's staging names start from its first.
#![cfg(unix)]
mod support;

use support::Fixture;
use typescript_parser::editing::{self, Edit, EditRequest, Effect, Operation, Prepared};
use typescript_parser::navigation::ShowQuery;

#[test]
fn names_other_files_hold_are_left_alone() {
    let fixture = Fixture::new(&[("a.ts", "function a() { return 1; }\n")]);
    // Files at the first names this process would stage and back up with.
    let taken: Vec<_> = (0..4)
        .flat_map(|n| {
            ["new", "orig"]
                .map(|suffix| format!(".a.ts.tsparse-{}-{n}.{suffix}", std::process::id()))
        })
        .collect();
    for name in &taken {
        std::fs::write(fixture.path(name), "someone else's").unwrap();
    }
    let Prepared::Ready(batch) = editing::prepare(&EditRequest {
        edits: vec![Edit {
            operation: Operation::ReplaceBody,
            target: ShowQuery::Name {
                name: "a".into(),
                file: Some(fixture.path("a.ts")),
            },
            fragment: "{ return 2; }".into(),
        }],
    })
    .unwrap() else {
        panic!("expected a batch");
    };
    let applied = batch.apply();
    assert_eq!(applied.effect, Effect::Applied, "{applied:?}");
    assert!(applied.leftovers.is_empty());
    assert_eq!(
        std::fs::read_to_string(fixture.path("a.ts")).unwrap(),
        "function a() { return 2; }\n"
    );
    for name in &taken {
        assert_eq!(
            std::fs::read_to_string(fixture.path(name)).unwrap(),
            "someone else's",
            "{name}"
        );
    }
    let mut names: Vec<String> = std::fs::read_dir(&fixture.root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| !taken.contains(name))
        .collect();
    names.sort();
    assert_eq!(names, ["a.ts"]);
}
