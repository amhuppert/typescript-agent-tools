//! Work avoidance is checked through a published session response, together
//! with equality to a fresh project. Counters are isolated by fixture path.
use super::super::test_compiler::{BASE_OPTIONS, DIALECTS, Dialect, MANIFEST, manifest};
use crate::project::{Project, ProjectRequest, ProjectResponse, ProjectSession};
use crate::references::ReferenceQuery;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct Work {
    counts: [usize; 2],
    after_module: HashMap<String, Change>,
}

#[derive(Clone)]
enum Change {
    Write(PathBuf, String),
    #[cfg(unix)]
    Link(PathBuf, PathBuf),
}

fn work() -> &'static Mutex<HashMap<String, Work>> {
    static WORK: OnceLock<Mutex<HashMap<String, Work>>> = OnceLock::new();
    WORK.get_or_init(Mutex::default)
}

pub(super) fn worked(config: &str, types: bool) {
    if let Some(work) = work().lock().unwrap().get_mut(config) {
        work.counts[usize::from(types)] += 1;
    }
}

/// Deterministic external changes between demanded lookups. Both fresh and
/// reused loading see the same change schedule; no timing assumption or
/// sleep makes first-answer memo behavior pass accidentally.
pub(super) fn finished(config: &str, specifier: &str) {
    let change = work()
        .lock()
        .unwrap()
        .get(config)
        .and_then(|work| work.after_module.get(specifier))
        .cloned();
    match change {
        Some(Change::Write(path, text)) => std::fs::write(path, text).unwrap(),
        #[cfg(unix)]
        Some(Change::Link(link, target)) => {
            std::fs::remove_file(&link).unwrap();
            std::os::unix::fs::symlink(target, link).unwrap();
        }
        None => {}
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tsparse-resolution-reuse-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        let fixture = Self(std::fs::canonicalize(path).unwrap());
        fixture.write(
            "tsconfig.json",
            r#"{"compilerOptions":{"noLib":true,"types":[],"typeRoots":["./types"]},"include":["*.ts"]}"#,
        );
        fixture
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// Make the installed compiler claim `version`, as the integration
    /// tests' `support::Fixture::claim_compiler` does.
    fn claim_compiler(&self, version: &str) {
        self.write(MANIFEST, &manifest(version));
    }

    fn config(&self) -> PathBuf {
        self.0.join("tsconfig.json")
    }

    fn key(&self) -> String {
        super::super::paths::normalize(&super::super::paths::from_path(&self.config()).unwrap())
    }

    fn reset_work(&self) {
        work().lock().unwrap().entry(self.key()).or_default().counts = [0, 0];
    }

    fn work(&self) -> [usize; 2] {
        work().lock().unwrap()[&self.key()].counts
    }

    fn after_module(&self, specifier: &str, change: Change) {
        work()
            .lock()
            .unwrap()
            .entry(self.key())
            .or_default()
            .after_module
            .insert(specifier.to_owned(), change);
    }

    fn request(&self, name: &str) -> ProjectRequest {
        ProjectRequest::References(ReferenceQuery::Name {
            name: name.into(),
            file: None,
        })
    }

    fn assert_fresh(&self, actual: &ProjectResponse, name: &str) {
        assert_eq!(
            *actual,
            Project::load(&self.config())
                .unwrap()
                .execute(self.request(name))
                .unwrap()
        );
    }
}

fn declaration(answer: &ProjectResponse) -> &str {
    let ProjectResponse::References(answer) = answer else {
        panic!("references request returns references");
    };
    assert_eq!(answer.groups.len(), 1);
    assert_eq!(answer.groups[0].declarations.len(), 1);
    &answer.groups[0].declarations[0].location.file
}

impl Drop for Fixture {
    fn drop(&mut self) {
        work().lock().unwrap().remove(&self.key());
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn body_edit_reuses_module_and_type_resolution() {
    let fixture = Fixture::new();
    fixture.write("b.ts", "export const value = 1;\n");
    fixture.write(
        "types/fixture/index.d.ts",
        "declare const globalValue: number;\n",
    );
    let text = "/// <reference types=\"fixture\" />\nimport { value } from './b';\nvalue; globalValue;\nconst marker = 1;\n";
    fixture.write("a.ts", text);
    fixture.reset_work();
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    let before = session.execute(fixture.request("value")).unwrap();
    assert!(fixture.work().into_iter().all(|count| count > 0));
    fixture.write("a.ts", &text.replace("marker = 1", "marker = 2"));
    fixture.reset_work();
    let after = session.execute(fixture.request("value")).unwrap();
    assert_eq!(after, before);
    assert_eq!(session.generation(), 2);
    let counts = fixture.work();
    fixture.assert_fresh(&after, "value");
    assert_eq!(counts, [0, 0], "unchanged lookups must skip their workers");
}

fn assert_new_higher_priority_candidate_is_found(types: bool) {
    let fixture = Fixture::new();
    fixture.write(
        "tsconfig.json",
        r#"{"compilerOptions":{"noLib":true,"types":[],"typeRoots":["./first","./second"]},"files":["a.ts"]}"#,
    );
    let (old, new) = if types {
        fixture.write("a.ts", "/// <reference types=\"fixture\" />\nvalue;\n");
        fixture.write(
            "second/fixture/index.d.ts",
            "declare const value: number;\n",
        );
        ("second/fixture/index.d.ts", "first/fixture/index.d.ts")
    } else {
        fixture.write("a.ts", "import { value } from './value'; value;\n");
        fixture.write("value/index.ts", "export const value = 1;\n");
        ("value/index.ts", "value.ts")
    };
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    assert_eq!(
        declaration(&session.execute(fixture.request("value")).unwrap()),
        old
    );
    // The next change must invalidate evidence replayed by a cache hit,
    // not just the original generation's physical reads. In particular,
    // the absent first type root is observed only by type resolution.
    let source = std::fs::read_to_string(fixture.0.join("a.ts")).unwrap();
    fixture.write("a.ts", &format!("{source}// unrelated body edit\n"));
    fixture.reset_work();
    let answer = session.execute(fixture.request("value")).unwrap();
    assert_eq!(fixture.work(), [0, 0]);
    assert_eq!(declaration(&answer), old);
    assert_eq!(session.generation(), 2);
    fixture.write(
        new,
        if types {
            "declare const value: number;\n"
        } else {
            "export const value = 2;\n"
        },
    );
    fixture.reset_work();
    let answer = session.execute(fixture.request("value")).unwrap();
    assert_eq!(declaration(&answer), new);
    assert!(fixture.work()[usize::from(types)] > 0);
    assert_eq!(session.generation(), 3);
    fixture.assert_fresh(&answer, "value");
    std::fs::remove_file(fixture.0.join(new)).unwrap();
    fixture.reset_work();
    let answer = session.execute(fixture.request("value")).unwrap();
    assert!(fixture.work()[usize::from(types)] > 0);
    assert_eq!(declaration(&answer), old);
    assert_eq!(session.generation(), 4);
    fixture.assert_fresh(&answer, "value");
}

#[test]
fn new_higher_priority_module_candidates_are_found_after_a_reused_generation() {
    assert_new_higher_priority_candidate_is_found(false);
}

#[test]
fn new_higher_priority_type_candidates_are_found_after_a_reused_generation() {
    assert_new_higher_priority_candidate_is_found(true);
}

#[test]
fn changed_manifest_and_options_replace_resolutions() {
    let fixture = Fixture::new();
    let config = |condition: &str| {
        format!(
            "{{\"compilerOptions\":{{\"noLib\":true,\"types\":[],\"moduleResolution\":\"bundler\",\"module\":\"esnext\",\"customConditions\":[\"{condition}\"]}},\"files\":[\"a.ts\"]}}"
        )
    };
    fixture.write("tsconfig.json", &config("first"));
    fixture.write("a.ts", "import { value } from 'pkg'; value;\n");
    fixture.write(
        "node_modules/pkg/one.d.ts",
        "export declare const value: 1;\n",
    );
    fixture.write(
        "node_modules/pkg/two.d.ts",
        "export declare const value: 2;\n",
    );
    fixture.write(
        "node_modules/pkg/package.json",
        r#"{"exports":{"first":"./one.d.ts","second":"./two.d.ts"}}"#,
    );
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    assert_eq!(
        declaration(&session.execute(fixture.request("value")).unwrap()),
        "node_modules/pkg/one.d.ts"
    );
    fixture.write("tsconfig.json", &config("second"));
    let answer = session.execute(fixture.request("value")).unwrap();
    assert_eq!(declaration(&answer), "node_modules/pkg/two.d.ts");
    fixture.assert_fresh(&answer, "value");
    let manifest = fixture.0.join("node_modules/pkg/package.json");
    let modified = std::fs::metadata(&manifest).unwrap().modified().unwrap();
    fixture.write(
        "node_modules/pkg/package.json",
        r#"{"exports":{"first":"./two.d.ts","second":"./one.d.ts"}}"#,
    );
    // Same-sized contents with the old mtime still invalidate exact evidence.
    std::fs::File::options()
        .write(true)
        .open(manifest)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    let answer = session.execute(fixture.request("value")).unwrap();
    assert_eq!(declaration(&answer), "node_modules/pkg/one.d.ts");
    assert_eq!(session.generation(), 3);
    fixture.assert_fresh(&answer, "value");
}

#[test]
fn a_reused_lookup_seeds_the_first_package_answer_for_a_new_lookup() {
    let fixture = Fixture::new();
    fixture.write("tsconfig.json", r#"{"compilerOptions":{"noLib":true,"types":[],"moduleResolution":"bundler","module":"esnext"},"files":["a.ts"]}"#);
    let old = r#"{"exports":{"./a":"./a.d.ts","./b":"./old.d.ts"}}"#;
    let changed = r#"{"exports":{"./a":"./a.d.ts","./b":"./new.d.ts"}}"#;
    let manifest = fixture.0.join("node_modules/pkg/package.json");
    fixture.write("node_modules/pkg/package.json", old);
    fixture.write(
        "node_modules/pkg/a.d.ts",
        "export declare const first: number;\n",
    );
    fixture.write(
        "node_modules/pkg/old.d.ts",
        "export declare const second: number;\n",
    );
    fixture.write(
        "node_modules/pkg/new.d.ts",
        "export declare const second: number;\n",
    );
    fixture.write("a.ts", "import { first } from 'pkg/a'; first;\n");
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    session.execute(fixture.request("first")).unwrap();
    fixture.write(
        "a.ts",
        "import { first } from 'pkg/a'; import { second } from 'pkg/b'; first; second;\n",
    );
    fixture.after_module("pkg/a", Change::Write(manifest.clone(), changed.into()));
    fixture.after_module("pkg/b", Change::Write(manifest, old.into()));
    fixture.reset_work();
    let answer = session.execute(fixture.request("second")).unwrap();
    assert_eq!(fixture.work(), [1, 0], "only the newly added lookup runs");
    assert_eq!(declaration(&answer), "node_modules/pkg/old.d.ts");
    fixture.assert_fresh(&answer, "second");
}

#[test]
fn the_current_package_answer_wins_over_a_saved_lookup_and_restored_disk() {
    let fixture = Fixture::new();
    fixture.write("tsconfig.json", r#"{"compilerOptions":{"noLib":true,"types":[],"moduleResolution":"bundler","module":"esnext"},"files":["a.ts"]}"#);
    let old = r#"{"exports":{"./a":"./old.d.ts","./c":"./common.d.ts"}}"#;
    let changed = r#"{"exports":{"./a":"./new.d.ts","./c":"./common.d.ts"}}"#;
    let manifest = fixture.0.join("node_modules/pkg/package.json");
    fixture.write("node_modules/pkg/package.json", old);
    for name in ["old", "new"] {
        fixture.write(
            &format!("node_modules/pkg/{name}.d.ts"),
            "export declare const value: number;\n",
        );
    }
    fixture.write(
        "node_modules/pkg/common.d.ts",
        "export declare const first: number;\n",
    );
    fixture.write("a.ts", "import { value } from 'pkg/a'; value;\n");
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    assert_eq!(
        declaration(&session.execute(fixture.request("value")).unwrap()),
        "node_modules/pkg/old.d.ts"
    );
    fixture.write("node_modules/pkg/package.json", changed);
    fixture.write(
        "a.ts",
        "import { first } from 'pkg/c'; import { value } from 'pkg/a'; first; value;\n",
    );
    // The fresh first lookup owns M2. Restoring M1 on disk cannot admit the
    // saved lookup against that different current first answer. Restore M2
    // after the second lookup so exact final validation can publish normally.
    fixture.after_module("pkg/c", Change::Write(manifest.clone(), old.into()));
    fixture.after_module("pkg/a", Change::Write(manifest, changed.into()));
    fixture.reset_work();
    let answer = session.execute(fixture.request("value")).unwrap();
    assert_eq!(declaration(&answer), "node_modules/pkg/new.d.ts");
    assert_eq!(fixture.work(), [2, 0]);
    assert_eq!(session.generation(), 2);
    fixture.assert_fresh(&answer, "value");
}

#[cfg(unix)]
#[test]
fn a_reused_lookup_seeds_the_first_real_path_for_a_new_lookup() {
    let fixture = Fixture::new();
    fixture.write("tsconfig.json", r#"{"compilerOptions":{"noLib":true,"types":[],"moduleResolution":"bundler","module":"esnext"},"files":["a.ts"]}"#);
    for directory in ["old", "new"] {
        fixture.write(
            &format!("packages/{directory}/package.json"),
            r#"{"exports":{"./a":"./index.d.ts","./b":"./index.d.ts"}}"#,
        );
        fixture.write(
            &format!("packages/{directory}/index.d.ts"),
            "export declare const first: number; export declare const second: number;\n",
        );
    }
    std::fs::create_dir_all(fixture.0.join("node_modules")).unwrap();
    let link = fixture.0.join("node_modules/pkg");
    std::os::unix::fs::symlink("../packages/old", &link).unwrap();
    fixture.write("a.ts", "import { first } from 'pkg/a'; first;\n");
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    session.execute(fixture.request("first")).unwrap();
    fixture.write(
        "a.ts",
        "import { first } from 'pkg/a'; import { second } from 'pkg/b'; first; second;\n",
    );
    fixture.after_module(
        "pkg/a",
        Change::Link(link.clone(), "../packages/new".into()),
    );
    fixture.after_module("pkg/b", Change::Link(link, "../packages/old".into()));
    fixture.reset_work();
    let answer = session.execute(fixture.request("second")).unwrap();
    assert_eq!(fixture.work(), [1, 0]);
    assert_eq!(declaration(&answer), "packages/old/index.d.ts");
    fixture.assert_fresh(&answer, "second");
}

#[cfg(unix)]
#[test]
fn the_current_real_path_wins_over_a_saved_lookup_and_restored_link() {
    let fixture = Fixture::new();
    fixture.write("tsconfig.json", r#"{"compilerOptions":{"noLib":true,"types":[],"moduleResolution":"bundler","module":"esnext"},"files":["a.ts"]}"#);
    for directory in ["old", "new"] {
        fixture.write(
            &format!("packages/{directory}/package.json"),
            r#"{"exports":{"./a":"./index.d.ts","./c":"./index.d.ts"}}"#,
        );
        fixture.write(
            &format!("packages/{directory}/index.d.ts"),
            "export declare const first: number; export declare const value: number;\n",
        );
    }
    std::fs::create_dir_all(fixture.0.join("node_modules")).unwrap();
    let link = fixture.0.join("node_modules/pkg");
    std::os::unix::fs::symlink("../packages/old", &link).unwrap();
    fixture.write("a.ts", "import { value } from 'pkg/a'; value;\n");
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    assert_eq!(
        declaration(&session.execute(fixture.request("value")).unwrap()),
        "packages/old/index.d.ts"
    );
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink("../packages/new", &link).unwrap();
    fixture.write(
        "a.ts",
        "import { first } from 'pkg/c'; import { value } from 'pkg/a'; first; value;\n",
    );
    fixture.after_module(
        "pkg/c",
        Change::Link(link.clone(), "../packages/old".into()),
    );
    fixture.after_module("pkg/a", Change::Link(link, "../packages/new".into()));
    fixture.reset_work();
    let answer = session.execute(fixture.request("value")).unwrap();
    assert_eq!(declaration(&answer), "packages/new/index.d.ts");
    assert_eq!(fixture.work(), [2, 0]);
    assert_eq!(session.generation(), 2);
    fixture.assert_fresh(&answer, "value");
}

#[cfg(unix)]
fn assert_symlink_paths_replayed_for_type_display(types: bool) {
    use crate::analysis::{AnalysisError, InputErrorKind};
    use crate::types::{TypeQuery, TypeRequest};

    let fixture = Fixture::new();
    fixture.write("tsconfig.json", r#"{"compilerOptions":{"strict":true,"noLib":true,"types":[],"typeRoots":["./node_modules/@types"],"module":"NodeNext","moduleResolution":"NodeNext"},"files":["globals.d.ts","seed.cts","fresh.cts"]}"#);
    // No runtime dependencies: package inference must not independently
    // reconstruct the symlink relation whose replay this test exercises.
    fixture.write("package.json", r#"{"type":"commonjs"}"#);
    fixture.write(
        "globals.d.ts",
        include_str!("../../../../fixtures/types/native-module-specifier-snapshot/globals.d.ts"),
    );
    fixture.write(
        "physical/pkg/package.json",
        r#"{"name":"pkg","types":"./sub/index.d.ts"}"#,
    );
    fixture.write(
        "physical/pkg/sub/index.d.ts",
        include_str!("../../../../fixtures/types/native-module-specifier-snapshot/target.d.ts"),
    );
    fixture.write(
        "physical/pkg/other/index.d.ts",
        include_str!("../../../../fixtures/types/native-module-specifier-snapshot/target.d.ts"),
    );
    let (alias, target, seed) = if types {
        (
            "node_modules/@types/pkg",
            "../../physical/pkg",
            "/// <reference types=\"pkg\" />\n",
        )
    } else {
        (
            "node_modules/pkg",
            "../physical/pkg",
            "import type { present } from 'pkg/sub/index.js';\n",
        )
    };
    std::fs::create_dir_all(fixture.0.join(alias).parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(target, fixture.0.join(alias)).unwrap();
    fixture.write("seed.cts", seed);
    // The pinned native snapshot fixture establishes that the first
    // incompatible written import forces fresh contextual spelling. The
    // first relative import chooses minimal endings, so native 7.0.2 spells
    // this target as "pkg/other" or "pkg/other/index" depending on collision.
    // These relative imports supply no node_modules symlink themselves.
    fixture.write("home.ts", "export const home = 1;\n");
    let source =
        include_str!("../../../../fixtures/types/native-module-specifier-snapshot/fresh.cts")
            .replace("pkg/sub/index.js", "./physical/pkg/other/index.js");
    let source = format!("import {{ home }} from './home';\n{source}");
    fixture.write("fresh.cts", &source);
    let probe = fixture.0.join(format!("{alias}/other.ts"));
    std::os::unix::fs::symlink("other.ts", &probe).unwrap();
    let request = |name: &str| {
        ProjectRequest::Type(TypeRequest::new(TypeQuery::Name {
            name: name.into(),
            file: Some(fixture.0.join("fresh.cts")),
        }))
    };
    fixture.reset_work();
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    session.execute(request("healthy")).unwrap();
    assert!(fixture.work()[usize::from(types)] > 0);
    let before = session.execute(request("Missing")).unwrap_err();
    let AnalysisError::Input(input) = &before else {
        panic!("contextual module spelling must demand the poisoned alias: {before:?}");
    };
    assert_eq!(input.kind, InputErrorKind::Io);
    assert_eq!(input.path.as_ref(), Some(&probe));
    fixture.write("fresh.cts", &format!("{source}// unrelated body edit\n"));
    fixture.reset_work();
    session.execute(request("healthy")).unwrap();
    assert_eq!(fixture.work(), [0, 0]);
    assert_eq!(session.generation(), 2);
    let after = session.execute(request("Missing")).unwrap_err();
    assert_eq!(after, before, "types={types}");
    assert_eq!(
        after,
        Project::load(&fixture.config())
            .unwrap()
            .execute(request("Missing"))
            .unwrap_err()
    );
}

#[cfg(unix)]
#[test]
fn reused_module_lookups_replay_symlink_paths_for_type_display() {
    assert_symlink_paths_replayed_for_type_display(false);
}

#[cfg(unix)]
#[test]
fn reused_type_lookups_replay_symlink_paths_for_type_display() {
    assert_symlink_paths_replayed_for_type_display(true);
}

#[cfg(unix)]
#[test]
fn changed_symlinks_and_demanded_failures_refresh_without_old_dependencies() {
    let fixture = Fixture::new();
    fixture.write(
        "tsconfig.json",
        r#"{"compilerOptions":{"noLib":true,"types":[]},"files":["a.ts"]}"#,
    );
    fixture.write("a.ts", "import { value } from 'pkg'; value;\n");
    for directory in ["old", "new"] {
        fixture.write(
            &format!("packages/{directory}/package.json"),
            r#"{"types":"index.d.ts"}"#,
        );
        fixture.write(
            &format!("packages/{directory}/index.d.ts"),
            "export declare const value: number;\n",
        );
    }
    std::fs::create_dir_all(fixture.0.join("node_modules")).unwrap();
    let link = fixture.0.join("node_modules/pkg");
    std::os::unix::fs::symlink("../packages/old", &link).unwrap();
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    assert_eq!(
        declaration(&session.execute(fixture.request("value")).unwrap()),
        "packages/old/index.d.ts"
    );
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink("../packages/new", &link).unwrap();
    let answer = session.execute(fixture.request("value")).unwrap();
    assert_eq!(declaration(&answer), "packages/new/index.d.ts");
    fixture.assert_fresh(&answer, "value");
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink("pkg", &link).unwrap();
    let error = session.execute(fixture.request("value")).unwrap_err();
    let fresh = match Project::load(&fixture.config()) {
        Ok(_) => panic!("a demanded symlink loop must remain a failure"),
        Err(error) => error,
    };
    assert_eq!(error, fresh);
    assert_eq!(session.generation(), 2);
    fixture.write("a.ts", "export const value = 2; value;\n");
    let answer = session.execute(fixture.request("value")).unwrap();
    assert_eq!(declaration(&answer), "a.ts");
    assert_eq!(session.generation(), 3);
    fixture.assert_fresh(&answer, "value");
}

/// A type name the configuration infers and one a directive writes are
/// separate lookups (`TypeReferenceOrigin`): under explicit roots only the
/// written one falls back to `node_modules`. When an automatic name appears
/// that a directive already resolved, from the same directory and in the
/// same mode, the refreshed answer is a fresh load's TS2688, not the
/// directive's saved package. Automatic names are 5.6's omitted `types` and
/// TypeScript 7's `"*"`.
#[test]
fn inferred_and_written_type_names_are_separate_reusable_lookups() {
    use crate::source::SourcePosition;
    for dialect in DIALECTS {
        let fixture = Fixture::new();
        fixture.claim_compiler(dialect.version());
        let options = match dialect {
            Dialect::TypeScript56 => {
                r#""strict":true,"target":"es2022","module":"esnext","moduleResolution":"bundler","esModuleInterop":true"#
            }
            Dialect::TypeScript7 => r#""types":["*"]"#,
        };
        fixture.write(
            "tsconfig.json",
            &format!(
                r#"{{"compilerOptions":{{"noLib":true,"typeRoots":["./types"],{options}}},"files":["a.ts"]}}"#
            ),
        );
        fixture.write("a.ts", "/// <reference types=\"x\" />\nvalue;\n");
        fixture.write(
            "node_modules/@types/x/index.d.ts",
            "declare const value: number;\n",
        );
        fixture.write("types/other/index.d.ts", "declare const other: number;\n");
        let request = ProjectRequest::References(ReferenceQuery::At {
            file: fixture.0.join("a.ts"),
            position: SourcePosition { line: 2, column: 1 },
        });
        let mut session = ProjectSession::new(&fixture.config()).unwrap();
        assert_eq!(
            declaration(&session.execute(request.clone()).unwrap()),
            "node_modules/@types/x/index.d.ts",
            "{dialect:?}"
        );
        // `x` becomes an automatic name: a type root directory without a
        // package, which reads no file the directive's lookup observed.
        fixture.write("types/x/README.md", "not a package\n");
        let refreshed = session.execute(request.clone()).map_err(|e| e.to_string());
        let fresh = Project::load(&fixture.config())
            .and_then(|project| project.execute(request.clone()))
            .map_err(|e| e.to_string());
        assert_eq!(refreshed, fresh, "{dialect:?}");
        let error = fresh.expect_err("an inferred name fell back to node_modules");
        assert!(
            error.contains("Cannot find type definition file for 'x'"),
            "{error}"
        );
    }
}

/// The declaration a refreshed session finds for the use at `line:1` of
/// `a.ts`, which must equal a fresh load's answer; either may fail alike.
fn refreshed_declaration(
    fixture: &Fixture,
    session: &mut ProjectSession,
    line: usize,
) -> Result<String, String> {
    use crate::source::SourcePosition;
    let request = ProjectRequest::References(ReferenceQuery::At {
        file: fixture.0.join("a.ts"),
        position: SourcePosition { line, column: 1 },
    });
    let refreshed = session.execute(request.clone()).map_err(|e| e.to_string());
    let fresh = Project::load(&fixture.config())
        .and_then(|project| project.execute(request))
        .map_err(|e| e.to_string());
    assert_eq!(refreshed, fresh);
    refreshed.map(|answer| declaration(&answer).to_owned())
}

/// A file appearing under `baseUrl` takes a bare name from the package, and
/// the package has it again once the file is removed.
#[test]
fn a_base_url_target_appearing_and_disappearing_refreshes_resolution() {
    let fixture = Fixture::new();
    fixture.claim_compiler("5.6.3");
    fixture.write(
        "tsconfig.json",
        &format!(r#"{{"compilerOptions":{{{BASE_OPTIONS},"baseUrl":"."}},"files":["a.ts"]}}"#),
    );
    fixture.write("a.ts", "import { value } from 'lib/thing';\nvalue;\n");
    fixture.write(
        "node_modules/lib/thing.d.ts",
        "export declare const value: number;\n",
    );
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    let mut answer = |step: &str| {
        let answer = refreshed_declaration(&fixture, &mut session, 2);
        answer.unwrap_or_else(|error| panic!("{step}: {error}"))
    };
    assert_eq!(answer("package"), "node_modules/lib/thing.d.ts");
    fixture.write("lib/thing.ts", "export const value = 1;\n");
    assert_eq!(answer("under baseUrl"), "lib/thing.ts");
    std::fs::remove_file(fixture.0.join("lib/thing.ts")).unwrap();
    assert_eq!(answer("removed"), "node_modules/lib/thing.d.ts");
}

/// Under explicit roots a written reference falls back to `node_modules`
/// until a root has the name: first its package directory, then its
/// declaration file, which wins; an automatic name appears with a root's
/// new directory. 5.6 reads automatic names from an omitted `types`.
#[test]
fn type_root_entries_appearing_refresh_type_references() {
    let fixture = Fixture::new();
    fixture.claim_compiler("5.6.3");
    fixture.write(
        "tsconfig.json",
        &format!(
            r#"{{"compilerOptions":{{{}}},"files":["a.ts"]}}"#,
            BASE_OPTIONS.replace(
                r#""types":[]"#,
                r#""typeRoots":["./types","./node_modules/@types"]"#
            )
        ),
    );
    fixture.write("a.ts", "/// <reference types=\"x\" />\nvalue;\nlater;\n");
    // Neither root has `x`: the reference falls back to `node_modules`
    // above `a.ts`, which has no `@types/x` yet.
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    let error = refreshed_declaration(&fixture, &mut session, 2).unwrap_err();
    assert!(
        error.contains("Cannot find type definition file for 'x'"),
        "{error}"
    );
    fixture.write(
        "node_modules/@types/x/index.d.ts",
        "declare const value: number;\n",
    );
    assert_eq!(
        refreshed_declaration(&fixture, &mut session, 2).unwrap(),
        "node_modules/@types/x/index.d.ts"
    );
    fixture.write("types/x/index.d.ts", "declare const value: number;\n");
    assert_eq!(
        refreshed_declaration(&fixture, &mut session, 2).unwrap(),
        "types/x/index.d.ts"
    );
    fixture.write("types/x.d.ts", "declare const value: number;\n");
    assert_eq!(
        refreshed_declaration(&fixture, &mut session, 2).unwrap(),
        "types/x.d.ts"
    );
    // A new `@types` directory is an automatic name, resolved in its root.
    fixture.write(
        "node_modules/@types/later/index.d.ts",
        "declare const later: number;\n",
    );
    assert_eq!(
        refreshed_declaration(&fixture, &mut session, 3).unwrap(),
        "node_modules/@types/later/index.d.ts"
    );
}

/// An installed compiler version change selects another `typesVersions`
/// entry and `types@` condition, with source and packages unchanged: `tv`
/// and `cond` map only for `>=5.6.3 <5.7`, `tv7` only for `>=7`, which
/// TypeScript 7's own version admits whatever other release is installed.
#[test]
fn an_installed_version_change_refreshes_package_selection() {
    let fixture = Fixture::new();
    fixture.claim_compiler("5.6.3");
    fixture.write(
        "tsconfig.json",
        &format!(r#"{{"compilerOptions":{{{BASE_OPTIONS}}},"files":["a.ts"]}}"#),
    );
    fixture.write(
        "a.ts",
        "import { tv } from 'tv/sub';\nimport { cond } from 'cond';\nimport { tv7 } from 'tv7/sub';\ntv;\ncond;\ntv7;\n",
    );
    fixture.write(
        "node_modules/tv/package.json",
        r#"{"name":"tv","version":"1.0.0","typesVersions":{">=5.6.3 <5.7":{"*":["ts56/*"]}}}"#,
    );
    fixture.write("node_modules/tv/sub.d.ts", "export declare const tv: 1;\n");
    fixture.write(
        "node_modules/tv/ts56/sub.d.ts",
        "export declare const tv: 2;\n",
    );
    fixture.write(
        "node_modules/cond/package.json",
        r#"{"name":"cond","version":"1.0.0","exports":{".":{"types@>=5.6.3 <5.7":"./ts56.d.ts","types":"./index.d.ts"}}}"#,
    );
    fixture.write(
        "node_modules/cond/index.d.ts",
        "export declare const cond: 1;\n",
    );
    fixture.write(
        "node_modules/cond/ts56.d.ts",
        "export declare const cond: 2;\n",
    );
    fixture.write(
        "node_modules/tv7/package.json",
        r#"{"name":"tv7","version":"1.0.0","typesVersions":{">=7":{"*":["ts7/*"]}}}"#,
    );
    fixture.write(
        "node_modules/tv7/sub.d.ts",
        "export declare const tv7: 1;\n",
    );
    fixture.write(
        "node_modules/tv7/ts7/sub.d.ts",
        "export declare const tv7: 2;\n",
    );
    let mut session = ProjectSession::new(&fixture.config()).unwrap();
    let mut answers = |step: &str| {
        [4, 5, 6].map(|line| {
            refreshed_declaration(&fixture, &mut session, line)
                .unwrap_or_else(|error| panic!("{step}: {error}"))
        })
    };
    let at_5_6 = [
        "node_modules/tv/ts56/sub.d.ts",
        "node_modules/cond/ts56.d.ts",
        "node_modules/tv7/sub.d.ts",
    ];
    let outside = [
        "node_modules/tv/sub.d.ts",
        "node_modules/cond/index.d.ts",
        "node_modules/tv7/sub.d.ts",
    ];
    let at_7 = [
        "node_modules/tv/sub.d.ts",
        "node_modules/cond/index.d.ts",
        "node_modules/tv7/ts7/sub.d.ts",
    ];
    assert_eq!(answers("5.6.3"), at_5_6);
    fixture.claim_compiler("5.7.0");
    assert_eq!(answers("5.7.0"), outside);
    fixture.claim_compiler("5.6.3");
    assert_eq!(answers("5.6.3 again"), at_5_6);
    fixture.claim_compiler("7.0.2");
    assert_eq!(answers("7.0.2"), at_7);
    fixture.claim_compiler("6.0.3");
    assert_eq!(answers("6.0.3"), at_7);
    fixture.claim_compiler("5.6.3");
    assert_eq!(answers("5.6.3 last"), at_5_6);
}
