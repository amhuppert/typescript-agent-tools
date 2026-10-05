//! Checked-in marker fixtures under `fixtures/references`, whose markers the
//! semantic oracle validates against TypeScript.
mod support;

use std::path::Path;
use support::Fixture;

#[test]
fn every_marked_fixture_project_matches_its_markers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/references");
    let mut directories: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("tsconfig.json").is_file())
        .collect();
    directories.sort();
    assert!(
        !directories.is_empty(),
        "no fixture projects in {}",
        root.display()
    );
    let mut failures = Vec::new();
    for directory in &directories {
        let fixture = Fixture::existing(directory);
        let outcome = std::panic::catch_unwind(|| fixture.check_markers(&fixture.load()));
        if let Err(panic) = outcome {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            failures.push(format!("{}: {message}", directory.display()));
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
