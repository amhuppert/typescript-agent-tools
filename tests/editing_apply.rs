//! Applying prepared batches to files: the bytes and permission mode
//! applying writes, the checks that reject a batch whose files changed, and
//! the filesystem effect of each failure, including rollback, recovery, and
//! cleanup, injected deterministically at the replacement boundary.
#![cfg(unix)]
mod support;

use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use support::Fixture;
use typescript_parser::editing::{
    self, Applied, Edit, EditRequest, Effect, FileState, Operation, Prepared, PreparedBatch, Step,
};
use typescript_parser::navigation::ShowQuery;

const A: &str = "function a() { return 1; }\n";
const B: &str = "function b() { return 2; }\n";
const A2: &str = "function a() { return 10; }\n";
const B2: &str = "function b() { return 20; }\n";

fn fixture() -> Fixture {
    Fixture::new(&[("a.ts", A), ("b.ts", B)])
}

fn replace(fixture: &Fixture, file: &str, name: &str, body: &str) -> Edit {
    Edit {
        operation: Operation::ReplaceBody,
        target: ShowQuery::Name {
            name: name.into(),
            file: Some(fixture.path(file)),
        },
        fragment: body.into(),
    }
}

fn batch(edits: Vec<Edit>) -> PreparedBatch {
    match editing::prepare(&EditRequest { edits }).unwrap() {
        Prepared::Ready(batch) => batch,
        other => panic!("expected a batch, got {other:?}"),
    }
}

/// Both files' bodies replaced.
fn both(fixture: &Fixture) -> PreparedBatch {
    batch(vec![
        replace(fixture, "a.ts", "a", "{ return 10; }"),
        replace(fixture, "b.ts", "b", "{ return 20; }"),
    ])
}

fn read(fixture: &Fixture, file: &str) -> String {
    std::fs::read_to_string(fixture.path(file)).unwrap()
}

/// Every file in the fixture's directory, to show no temporary or backup
/// file remains.
fn names(fixture: &Fixture) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(&fixture.root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

fn state<'a>(applied: &'a Applied, file: &Path) -> &'a FileState {
    &applied
        .files
        .iter()
        .find(|outcome| outcome.path == file)
        .unwrap_or_else(|| panic!("{} has an outcome in {applied:?}", file.display()))
        .state
}

#[test]
fn applying_writes_the_proposed_bytes_and_keeps_permissions() {
    let fixture = fixture();
    std::fs::set_permissions(fixture.path("a.ts"), std::fs::Permissions::from_mode(0o640)).unwrap();
    let batch = both(&fixture);
    let applied = batch.apply();
    assert_eq!(applied.effect, Effect::Applied, "{applied:?}");
    assert!(applied.failure.is_none() && applied.leftovers.is_empty());
    assert_eq!(read(&fixture, "a.ts"), A2);
    assert_eq!(read(&fixture, "b.ts"), B2);
    let mode = std::fs::metadata(fixture.path("a.ts")).unwrap().mode();
    assert_eq!(mode & 0o7777, 0o640);
    assert_eq!(names(&fixture), ["a.ts", "b.ts"]);
    assert_eq!(state(&applied, &fixture.path("a.ts")), &FileState::Replaced);
    // Applying again finds the files changed from the originals.
    let again = batch.apply();
    assert_eq!(again.effect, Effect::Unchanged, "{again:?}");
    assert_eq!(
        again.failure.unwrap().kind,
        editing::ApplyFailureKind::Stale
    );
}

#[test]
fn a_link_keeps_pointing_at_the_edited_file() {
    let fixture = fixture();
    std::os::unix::fs::symlink(fixture.path("a.ts"), fixture.path("link.ts")).unwrap();
    let applied = batch(vec![replace(&fixture, "link.ts", "a", "{ return 10; }")]).apply();
    assert_eq!(applied.effect, Effect::Applied, "{applied:?}");
    assert!(
        std::fs::symlink_metadata(fixture.path("link.ts"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(read(&fixture, "a.ts"), A2);
}

#[test]
fn a_batch_that_changes_nothing_writes_nothing() {
    let fixture = fixture();
    let before = std::fs::metadata(fixture.path("a.ts")).unwrap();
    let batch = batch(vec![replace(&fixture, "a.ts", "a", "{ return 1; }")]);
    assert!(batch.is_empty());
    let applied = batch.apply();
    assert_eq!(applied.effect, Effect::NoChange, "{applied:?}");
    let after = std::fs::metadata(fixture.path("a.ts")).unwrap();
    assert_eq!(
        (before.ino(), before.mtime_nsec()),
        (after.ino(), after.mtime_nsec())
    );
}

#[test]
fn changed_files_reject_the_whole_batch() {
    type Change = fn(&Fixture);
    let changes: [(&str, Change); 6] = [
        ("same size", |f| {
            std::fs::write(f.path("b.ts"), B.replace('2', "3")).unwrap()
        }),
        ("byte order mark", |f| {
            std::fs::write(f.path("b.ts"), format!("\u{feff}{B}")).unwrap()
        }),
        ("removed", |f| std::fs::remove_file(f.path("b.ts")).unwrap()),
        ("replaced with the same bytes", |f| {
            std::fs::write(f.path("b.new"), B).unwrap();
            std::fs::rename(f.path("b.new"), f.path("b.ts")).unwrap();
        }),
        ("permissions", |f| {
            std::fs::set_permissions(f.path("b.ts"), std::fs::Permissions::from_mode(0o600))
                .unwrap()
        }),
        ("hard linked", |f| {
            std::fs::hard_link(f.path("b.ts"), f.path("b.link")).unwrap()
        }),
    ];
    for (name, change) in changes {
        let fixture = fixture();
        let batch = both(&fixture);
        change(&fixture);
        let applied = batch.apply();
        assert_eq!(applied.effect, Effect::Unchanged, "{name}: {applied:?}");
        assert_eq!(
            applied.failure.as_ref().unwrap().kind,
            editing::ApplyFailureKind::Stale,
            "{name}: {applied:?}"
        );
        assert_eq!(read(&fixture, "a.ts"), A, "{name}");
    }
    // A link retargeted to a file with the same bytes is another file.
    let fixture = Fixture::new(&[("a.ts", A), ("copy.ts", A)]);
    std::os::unix::fs::symlink(fixture.path("a.ts"), fixture.path("link.ts")).unwrap();
    let batch = batch(vec![replace(&fixture, "link.ts", "a", "{ return 10; }")]);
    std::fs::remove_file(fixture.path("link.ts")).unwrap();
    std::os::unix::fs::symlink(fixture.path("copy.ts"), fixture.path("link.ts")).unwrap();
    let applied = batch.apply();
    assert_eq!(applied.effect, Effect::Unchanged, "{applied:?}");
    assert_eq!(
        (read(&fixture, "a.ts"), read(&fixture, "copy.ts")),
        (A.into(), A.into())
    );
}

#[test]
fn a_file_that_cannot_be_inspected_is_an_io_failure() {
    // A directory without search permission hides its file without
    // changing it; a changed mode would itself make the file stale.
    let fixture = Fixture::new(&[("a.ts", A), ("dir/b.ts", B)]);
    let batch = batch(vec![
        replace(&fixture, "a.ts", "a", "{ return 10; }"),
        replace(&fixture, "dir/b.ts", "b", "{ return 20; }"),
    ]);
    let directory = fixture.path("dir");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o600)).unwrap();
    let hidden = std::fs::read(fixture.path("dir/b.ts")).is_err();
    let applied = batch.apply();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !hidden {
        eprintln!(
            "SKIP a_file_that_cannot_be_inspected_is_an_io_failure: permissions are not enforced"
        );
        return;
    }
    assert_eq!(applied.effect, Effect::Unchanged, "{applied:?}");
    assert_eq!(applied.failure.unwrap().kind, editing::ApplyFailureKind::Io);
    assert_eq!(read(&fixture, "a.ts"), A);
}

#[test]
fn a_staging_failure_changes_nothing_and_leaves_nothing() {
    let fixture = fixture();
    let applied = both(&fixture).apply_with(&mut |step| match step {
        Step::Stage(1) => Err(io::Error::other("injected staging failure")),
        _ => Ok(()),
    });
    assert_eq!(applied.effect, Effect::Unchanged, "{applied:?}");
    let failure = applied.failure.unwrap();
    assert_eq!(failure.kind, editing::ApplyFailureKind::Io);
    assert!(failure.message.contains("injected staging failure"));
    assert_eq!(
        (read(&fixture, "a.ts"), read(&fixture, "b.ts")),
        (A.into(), B.into())
    );
    assert_eq!(names(&fixture), ["a.ts", "b.ts"]);
}

#[test]
fn an_install_failure_restores_the_installed_files() {
    let fixture = fixture();
    let applied = both(&fixture).apply_with(&mut |step| match step {
        Step::Install(1) => Err(io::Error::other("injected install failure")),
        _ => Ok(()),
    });
    assert_eq!(applied.effect, Effect::RolledBack, "{applied:?}");
    assert!(
        applied
            .failure
            .as_ref()
            .unwrap()
            .message
            .contains("injected install failure")
    );
    assert_eq!(
        (read(&fixture, "a.ts"), read(&fixture, "b.ts")),
        (A.into(), B.into())
    );
    assert_eq!(state(&applied, &fixture.path("a.ts")), &FileState::Restored);
    assert_eq!(
        state(&applied, &fixture.path("b.ts")),
        &FileState::Unchanged
    );
    assert_eq!(names(&fixture), ["a.ts", "b.ts"]);
}

#[test]
fn a_failed_restoration_keeps_the_original_for_recovery() {
    let fixture = fixture();
    let applied = both(&fixture).apply_with(&mut |step| match step {
        Step::Install(1) => Err(io::Error::other("injected install failure")),
        Step::Restore(0) => Err(io::Error::other("injected restore failure")),
        _ => Ok(()),
    });
    assert_eq!(applied.effect, Effect::RecoveryRequired, "{applied:?}");
    let FileState::RecoveryRequired { original, .. } = state(&applied, &fixture.path("a.ts"))
    else {
        panic!("{applied:?}");
    };
    // The installed file stays; the original is kept beside it.
    assert_eq!(read(&fixture, "a.ts"), A2);
    assert_eq!(std::fs::read_to_string(original).unwrap(), A);
    assert_eq!(read(&fixture, "b.ts"), B);
}

#[test]
fn restoration_never_overwrites_another_writers_bytes() {
    let fixture = fixture();
    let path = fixture.path("a.ts");
    let applied = both(&fixture).apply_with(&mut |step| match step {
        Step::Install(1) => {
            std::fs::write(&path, "// another writer\n").unwrap();
            Err(io::Error::other("injected install failure"))
        }
        _ => Ok(()),
    });
    assert_eq!(applied.effect, Effect::RecoveryRequired, "{applied:?}");
    assert_eq!(read(&fixture, "a.ts"), "// another writer\n");
    let FileState::RecoveryRequired { original, observed } = state(&applied, &fixture.path("a.ts"))
    else {
        panic!("{applied:?}");
    };
    assert!(observed.contains("neither"), "{observed}");
    assert_eq!(std::fs::read_to_string(original).unwrap(), A);
}

#[test]
fn a_cleanup_failure_still_reports_the_edits_as_applied() {
    let fixture = fixture();
    let applied = both(&fixture).apply_with(&mut |step| match step {
        Step::Cleanup(0) => Err(io::Error::other("injected cleanup failure")),
        _ => Ok(()),
    });
    assert_eq!(applied.effect, Effect::Applied, "{applied:?}");
    assert_eq!(
        (read(&fixture, "a.ts"), read(&fixture, "b.ts")),
        (A2.into(), B2.into())
    );
    assert_eq!(applied.leftovers.len(), 1, "{applied:?}");
    let leftover = &applied.leftovers[0];
    assert!(leftover.message.contains("injected cleanup failure"));
    assert_eq!(std::fs::read_to_string(&leftover.path).unwrap(), A);
}

#[test]
fn a_directory_that_refuses_new_files_fails_staging() {
    let fixture = Fixture::new(&[("dir/a.ts", A)]);
    let batch = batch(vec![replace(&fixture, "dir/a.ts", "a", "{ return 10; }")]);
    let directory = fixture.path("dir");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555)).unwrap();
    let applied = batch.apply();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    if applied.effect == Effect::Applied {
        eprintln!(
            "SKIP a_directory_that_refuses_new_files_fails_staging: permissions are not enforced"
        );
        return;
    }
    assert_eq!(applied.effect, Effect::Unchanged, "{applied:?}");
    assert_eq!(read(&fixture, "dir/a.ts"), A);
}

#[test]
fn a_failure_at_the_first_replacement_changes_nothing() {
    let fixture = fixture();
    let applied = both(&fixture).apply_with(&mut |step| match step {
        Step::Install(0) => Err(io::Error::other("injected install failure")),
        _ => Ok(()),
    });
    assert_eq!(applied.effect, Effect::Unchanged, "{applied:?}");
    assert!(
        applied
            .files
            .iter()
            .all(|file| file.state == FileState::Unchanged)
    );
    assert_eq!(
        (read(&fixture, "a.ts"), read(&fixture, "b.ts")),
        (A.into(), B.into())
    );
    assert_eq!(names(&fixture), ["a.ts", "b.ts"]);
}

#[test]
fn a_file_changed_while_staging_is_found_by_the_second_check() {
    let fixture = fixture();
    let path = fixture.path("a.ts");
    let applied = both(&fixture).apply_with(&mut |step| {
        if step == Step::Stage(1) {
            // a.ts is staged first; change it in place before b.ts stages.
            std::fs::write(&path, A.replace('1', "7")).unwrap();
        }
        Ok(())
    });
    assert_eq!(applied.effect, Effect::Unchanged, "{applied:?}");
    assert_eq!(
        applied.failure.unwrap().kind,
        editing::ApplyFailureKind::Stale
    );
    assert_eq!(read(&fixture, "b.ts"), B);
    assert_eq!(names(&fixture), ["a.ts", "b.ts"]);
}

#[test]
fn a_failure_after_staging_began_releases_what_it_made() {
    let fixture = fixture();
    let applied = both(&fixture).apply_with(&mut |step| match step {
        Step::Link(1) => Err(io::Error::other("injected link failure")),
        _ => Ok(()),
    });
    assert_eq!(applied.effect, Effect::Unchanged, "{applied:?}");
    assert!(
        applied
            .failure
            .unwrap()
            .message
            .contains("injected link failure")
    );
    assert!(applied.leftovers.is_empty());
    assert_eq!(names(&fixture), ["a.ts", "b.ts"]);
    // A staged file that cannot be removed is reported where it remains.
    let applied = both(&fixture).apply_with(&mut |step| match step {
        Step::Link(1) => Err(io::Error::other("injected link failure")),
        Step::Cleanup(1) => Err(io::Error::other("injected cleanup failure")),
        _ => Ok(()),
    });
    assert_eq!(applied.effect, Effect::Unchanged, "{applied:?}");
    assert_eq!(applied.leftovers.len(), 1, "{applied:?}");
    assert!(applied.leftovers[0].path.exists());
    assert_eq!(
        std::fs::read_to_string(&applied.leftovers[0].path).unwrap(),
        B2
    );
}

#[test]
fn every_name_of_a_file_is_checked_again() {
    let fixture = Fixture::new(&[
        ("a.ts", "function f() {}\nfunction g() {}\n"),
        ("b.ts", "function f() {}\nfunction g() {}\n"),
    ]);
    std::os::unix::fs::symlink(fixture.path("a.ts"), fixture.path("alias.ts")).unwrap();
    let batch = batch(vec![
        replace(&fixture, "a.ts", "f", "{ 1; }"),
        replace(&fixture, "alias.ts", "g", "{ 2; }"),
    ]);
    let plan = fixture.root.join("plan.json");
    batch.save(&plan).unwrap();
    // The second name now names another file with the same bytes.
    std::fs::remove_file(fixture.path("alias.ts")).unwrap();
    std::os::unix::fs::symlink(fixture.path("b.ts"), fixture.path("alias.ts")).unwrap();
    for applied in [batch.apply(), PreparedBatch::load(&plan).unwrap().apply()] {
        assert_eq!(applied.effect, Effect::Unchanged, "{applied:?}");
        assert_eq!(
            applied.failure.unwrap().kind,
            editing::ApplyFailureKind::Stale
        );
    }
    assert_eq!(read(&fixture, "a.ts"), "function f() {}\nfunction g() {}\n");
}

#[cfg(target_os = "macos")]
#[test]
fn a_file_with_flags_is_not_edited() {
    let fixture = fixture();
    let status = std::process::Command::new("chflags")
        .args(["hidden"])
        .arg(fixture.path("a.ts"))
        .status()
        .unwrap();
    assert!(status.success());
    let error = editing::prepare(&EditRequest {
        edits: vec![replace(&fixture, "a.ts", "a", "{ return 10; }")],
    })
    .unwrap_err();
    assert!(error.to_string().contains("flags"), "{error}");
    // Flags set after preparing make the file stale.
    let fixture = self::fixture();
    let batch = both(&fixture);
    let status = std::process::Command::new("chflags")
        .args(["hidden"])
        .arg(fixture.path("b.ts"))
        .status()
        .unwrap();
    assert!(status.success());
    let applied = batch.apply();
    assert_eq!(applied.effect, Effect::Unchanged, "{applied:?}");
    assert_eq!(read(&fixture, "a.ts"), A);
}
