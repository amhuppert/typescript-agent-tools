//! Saving prepared batches as plans and loading them again: loading
//! prepares every edit again from its recorded site and original bytes,
//! independently of current files, fragments, and working directory, and
//! accepts the plan only if that gives exactly the recorded result.
mod support;

use std::path::Path;
use support::Fixture;
use typescript_parser::editing::{
    self, Edit, EditRequest, Effect, LoadErrorKind, Operation, Prepared, PreparedBatch,
};
use typescript_parser::navigation::ShowQuery;

const SOURCE: &str = "\u{feff}function a() { return 1; }\nconst b = 2;\n";

fn prepared(fixture: &Fixture) -> PreparedBatch {
    let edits = vec![
        Edit {
            operation: Operation::ReplaceBody,
            target: ShowQuery::Name {
                name: "a".into(),
                file: Some(fixture.path("a.ts")),
            },
            fragment: "{\n  return 10;\n}\n".into(),
        },
        Edit {
            operation: Operation::InsertAfter,
            target: ShowQuery::Name {
                name: "b".into(),
                file: Some(fixture.path("a.ts")),
            },
            fragment: "const c = 3;".into(),
        },
    ];
    match editing::prepare(&EditRequest { edits }).unwrap() {
        Prepared::Ready(batch) => batch,
        other => panic!("expected a batch, got {other:?}"),
    }
}

fn plan(fixture: &Fixture) -> std::path::PathBuf {
    let path = fixture.root.join("plan.json");
    prepared(fixture).save(&path).unwrap();
    path
}

/// Rewrite a saved plan's JSON.
fn edit_plan(path: &Path, change: impl FnOnce(&mut serde_json::Value)) {
    let mut plan: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    change(&mut plan);
    std::fs::write(path, serde_json::to_string_pretty(&plan).unwrap()).unwrap();
}

#[test]
fn a_loaded_plan_applies_exactly_the_previewed_bytes() {
    let fixture = Fixture::new(&[("a.ts", SOURCE)]);
    let batch = prepared(&fixture);
    let path = fixture.root.join("plan.json");
    batch.save(&path).unwrap();
    let loaded = PreparedBatch::load(&path).unwrap();
    assert_eq!(loaded.files().len(), 1);
    assert_eq!(
        loaded.files()[0].proposed_bytes(),
        batch.files()[0].proposed_bytes()
    );
    assert_eq!(loaded.files()[0].edits(), batch.files()[0].edits());
    assert_eq!(loaded.apply().effect, Effect::Applied);
    assert_eq!(
        std::fs::read_to_string(fixture.path("a.ts")).unwrap(),
        "\u{feff}function a() {\n  return 10;\n}\nconst b = 2;\nconst c = 3;\n"
    );
}

#[test]
fn a_plan_is_a_new_file_and_names_files_absolutely() {
    let fixture = Fixture::new(&[("a.ts", SOURCE)]);
    let batch = prepared(&fixture);
    let error = batch.save(&fixture.path("a.ts")).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        std::fs::read_to_string(fixture.path("a.ts")).unwrap(),
        SOURCE
    );
    let path = plan(&fixture);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(fixture.path("a.ts").to_str().unwrap()));
}

/// A plan records a file's identity as its device, inode, permission mode,
/// and link count, and nothing else about it: no owner, group, or flags.
#[cfg(unix)]
#[test]
fn a_plan_records_a_files_device_inode_mode_and_links_only() {
    use std::os::unix::fs::MetadataExt;
    let fixture = Fixture::new(&[("a.ts", SOURCE)]);
    let path = plan(&fixture);
    let plan: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let metadata = std::fs::metadata(fixture.path("a.ts")).unwrap();
    assert_eq!(
        plan["files"][0]["identity"],
        serde_json::json!({
            "device": metadata.dev(),
            "inode": metadata.ino(),
            "mode": metadata.mode(),
            "links": 1,
        })
    );
}

#[test]
fn loading_ignores_current_files_and_fragments_until_apply_checks_them() {
    let fixture = Fixture::new(&[("a.ts", SOURCE)]);
    let path = plan(&fixture);
    // The file changes after the plan is saved: loading still replays the
    // recorded original, and applying finds the file stale.
    std::fs::write(fixture.path("a.ts"), "function a() {}\n").unwrap();
    let loaded = PreparedBatch::load(&path).unwrap();
    let applied = loaded.apply();
    assert_eq!(applied.effect, Effect::Unchanged);
    assert_eq!(
        applied.failure.unwrap().kind,
        editing::ApplyFailureKind::Stale
    );
}

#[test]
fn a_plan_whose_replay_differs_is_rejected() {
    type Change = fn(&mut serde_json::Value);
    let cases: [(&str, Change, LoadErrorKind); 6] = [
        (
            "altered proposed text",
            |plan| plan["files"][0]["proposed"] = "function a() { evil(); }\n".into(),
            LoadErrorKind::Replay,
        ),
        (
            "altered fragment",
            |plan| plan["files"][0]["edits"][0]["fragment"] = "{ evil(); }".into(),
            LoadErrorKind::Replay,
        ),
        (
            "an anchor that selects nothing",
            |plan| plan["files"][0]["edits"][0]["site"]["anchor"]["column"] = 2.into(),
            LoadErrorKind::Replay,
        ),
        (
            "an ineligible recorded site",
            |plan| plan["files"][0]["edits"][1]["operation"] = "replace_body".into(),
            LoadErrorKind::Replay,
        ),
        (
            "another build",
            |plan| plan["build"] = "other".into(),
            LoadErrorKind::Incompatible,
        ),
        (
            "another format",
            |plan| plan["version"] = 99.into(),
            LoadErrorKind::Incompatible,
        ),
    ];
    for (name, change, kind) in cases {
        let fixture = Fixture::new(&[("a.ts", SOURCE)]);
        let path = plan(&fixture);
        edit_plan(&path, change);
        let error = PreparedBatch::load(&path).unwrap_err();
        assert_eq!(error.kind, kind, "{name}: {error:?}");
    }
    let fixture = Fixture::new(&[("a.ts", SOURCE)]);
    let path = fixture.root.join("plan.json");
    std::fs::write(&path, "{ not json").unwrap();
    assert_eq!(
        PreparedBatch::load(&path).unwrap_err().kind,
        LoadErrorKind::Malformed
    );
    assert_eq!(
        PreparedBatch::load(&fixture.root.join("missing.json"))
            .unwrap_err()
            .kind,
        LoadErrorKind::Io
    );
}

#[test]
fn a_plan_edits_only_files_preparation_would() {
    let fixture = Fixture::new(&[("a.ts", SOURCE)]);
    let path = plan(&fixture);
    std::fs::rename(fixture.path("a.ts"), fixture.path("a.js")).unwrap();
    let js = fixture.path("a.js");
    edit_plan(&path, |plan| {
        plan["files"][0]["path"] = js.to_str().unwrap().into();
        plan["files"][0]["referent"] = js.to_str().unwrap().into();
    });
    // A file editing refuses for its kind is unsupported, as preparing it
    // would be, and named.
    let error = PreparedBatch::load(&path).unwrap_err();
    assert_eq!(error.kind, LoadErrorKind::Unsupported, "{error:?}");
    assert_eq!(error.path, js);
    // Every recorded name is admitted, the other names too.
    let fixture = Fixture::new(&[("a.ts", SOURCE)]);
    let path = plan(&fixture);
    let js = fixture.path("a.js");
    edit_plan(&path, |plan| {
        plan["files"][0]["aliases"] = serde_json::json!([js.to_str().unwrap()])
    });
    let error = PreparedBatch::load(&path).unwrap_err();
    assert_eq!(error.kind, LoadErrorKind::Unsupported, "{error:?}");
    assert_eq!(error.path, js);
    // A relative path names nothing exactly, nor does a relative referent.
    for field in ["path", "referent"] {
        let fixture = Fixture::new(&[("a.ts", SOURCE)]);
        let path = plan(&fixture);
        edit_plan(&path, |plan| plan["files"][0][field] = "a.ts".into());
        let error = PreparedBatch::load(&path).unwrap_err();
        assert_eq!(error.kind, LoadErrorKind::Malformed, "{field}: {error:?}");
        assert!(
            error.message.contains("absolute path"),
            "{field}: {error:?}"
        );
    }
}

/// The names a plan records as its loaded batch gives them, and as a plan
/// that cannot be loaded gives them from its text, which must agree.
fn recorded(path: &Path) -> (Vec<std::path::PathBuf>, Vec<std::path::PathBuf>) {
    let loaded = PreparedBatch::load(path).unwrap();
    let names = loaded.names().into_iter().map(Path::to_path_buf).collect();
    edit_plan(path, |plan| plan["build"] = "another".into());
    let error = PreparedBatch::load(path).unwrap_err();
    assert_eq!(error.kind, LoadErrorKind::Incompatible);
    (names, error.names.unwrap())
}

#[cfg(unix)]
#[test]
fn a_plan_names_its_files_alike_whether_or_not_it_loads() {
    let fixture = Fixture::new(&[("a.ts", SOURCE)]);
    std::os::unix::fs::symlink("a.ts", fixture.path("link.ts")).unwrap();
    let mut edits = vec![Edit {
        operation: Operation::ReplaceBody,
        target: ShowQuery::Name {
            name: "a".into(),
            file: Some(fixture.path("link.ts")),
        },
        fragment: "{ return 10; }".into(),
    }];
    edits.push(Edit {
        operation: Operation::InsertAfter,
        target: ShowQuery::Name {
            name: "b".into(),
            file: Some(fixture.path("a.ts")),
        },
        fragment: "const c = 3;".into(),
    });
    let Prepared::Ready(batch) = editing::prepare(&EditRequest { edits }).unwrap() else {
        panic!("expected a batch");
    };
    let path = fixture.root.join("plan.json");
    batch.save(&path).unwrap();
    let (loaded, unloadable) = recorded(&path);
    let canonical = std::fs::canonicalize(fixture.path("a.ts")).unwrap();
    assert_eq!(
        loaded,
        [fixture.path("link.ts"), fixture.path("a.ts"), canonical]
    );
    assert_eq!(unloadable, loaded);
    // A text that does not decode as a plan, though it is JSON holding
    // the same names, names files no one can know.
    edit_plan(&path, |plan| {
        plan["files"][0]["aliases"] = serde_json::Value::Null
    });
    assert_eq!(PreparedBatch::load(&path).unwrap_err().names, None);
    std::fs::write(&path, "{ \"files\": [").unwrap();
    assert_eq!(PreparedBatch::load(&path).unwrap_err().names, None);
    // A plan that does not exist records nothing.
    std::fs::remove_file(&path).unwrap();
    assert_eq!(PreparedBatch::load(&path).unwrap_err().names, Some(vec![]));
}
