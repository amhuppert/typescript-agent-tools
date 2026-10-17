//! Refresh reuse must preserve fresh-load answers while retaining unchanged
//! file products. These tests cross the public session request boundary.
use super::*;
use crate::references::ReferenceQuery;
use std::sync::atomic::AtomicUsize;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let fixture = Self(std::env::temp_dir().join(format!(
            "tsparse-refresh-reuse-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir_all(&fixture.0).unwrap();
        fixture.write(
            "tsconfig.json",
            r#"{"compilerOptions":{"noLib":true,"types":[]},"include":["*.ts"]}"#,
        );
        fixture
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn session(&self) -> ProjectSession {
        ProjectSession::new(&self.0.join("tsconfig.json")).unwrap()
    }

    fn references(&self, session: &mut ProjectSession, name: &str) -> ProjectResponse {
        let request = ProjectRequest::References(ReferenceQuery::Name {
            name: name.into(),
            file: None,
        });
        let actual = session.execute(request.clone()).unwrap();
        let fresh = Project::load(&self.0.join("tsconfig.json"))
            .unwrap()
            .execute(request)
            .unwrap();
        assert_eq!(actual, fresh, "refresh and a fresh load must agree");
        actual
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn source(session: &ProjectSession, display: &str) -> Arc<crate::source::SourceText> {
    let project = &session.current.as_ref().unwrap().0;
    project
        .sources
        .ids()
        .map(|id| project.sources.get(id))
        .find(|file| file.display == display)
        .unwrap()
        .text
        .clone()
}

fn identity(session: &ProjectSession, fixture: &Fixture, file: &str) -> (usize, Option<usize>) {
    session
        .reuse
        .as_ref()
        .unwrap()
        .capture_identity(&fixture.0.join(file))
}

#[test]
fn refresh_reuses_unchanged_source_and_publishes_the_edit() {
    let fixture = Fixture::new();
    fixture.write("a.ts", "export const before = 1;\n");
    fixture.write("b.ts", "export const value = 1; value;\n");
    let mut session = fixture.session();
    let before = fixture.references(&mut session, "value");
    let unchanged = source(&session, "b.ts");
    let products = identity(&session, &fixture, "b.ts");
    let changed = source(&session, "a.ts");
    fixture.write("a.ts", "export const after = 2;\n");
    assert_eq!(fixture.references(&mut session, "value"), before);
    let ProjectResponse::References(after) = fixture.references(&mut session, "after") else {
        panic!("references request returns references");
    };
    assert_eq!(after.groups.len(), 1);
    assert_eq!(after.groups[0].declarations[0].location.file, "a.ts");
    assert_eq!(session.generation(), 2);
    assert!(!Arc::ptr_eq(&changed, &source(&session, "a.ts")));
    assert!(
        Arc::ptr_eq(&unchanged, &source(&session, "b.ts")),
        "refresh must reuse the unchanged file's captured source"
    );
    assert_eq!(products, identity(&session, &fixture, "b.ts"));
}

#[test]
fn reused_files_receive_current_ids_after_roots_and_imports_change() {
    let fixture = Fixture::new();
    fixture.write("b.ts", "export const value = 1;\n");
    fixture.write("c.ts", "import { value } from './b'; value;\n");
    let mut session = fixture.session();
    fixture.references(&mut session, "value");
    let b = identity(&session, &fixture, "b.ts");
    let c = identity(&session, &fixture, "c.ts");
    fixture.write("a.ts", "import { value } from './b'; value;\n");
    let ProjectResponse::References(answer) = fixture.references(&mut session, "value") else {
        unreachable!();
    };
    assert_eq!(answer.groups.len(), 1);
    assert_eq!(answer.groups[0].declarations[0].location.file, "b.ts");
    assert_eq!(b, identity(&session, &fixture, "b.ts"));
    assert_eq!(c, identity(&session, &fixture, "c.ts"));
    let request = ProjectRequest::Outline {
        file: fixture.0.join("b.ts"),
    };
    assert_eq!(
        session.execute(request.clone()).unwrap(),
        Project::load(&fixture.0.join("tsconfig.json"))
            .unwrap()
            .execute(request)
            .unwrap()
    );
}

#[test]
fn package_format_recollects_facts_but_reuses_text_and_navigation() {
    let fixture = Fixture::new();
    fixture.write("tsconfig.json", r#"{"compilerOptions":{"noLib":true,"types":[],"module":"nodenext","moduleDetection":"auto"},"files":["a.ts","b.ts"]}"#);
    fixture.write("package.json", r#"{"type":"commonjs"}"#);
    fixture.write("a.ts", "interface Shape { value: number }\n");
    fixture.write("b.ts", "let value: Shape;\n");
    let mut session = fixture.session();
    let before = fixture.references(&mut session, "Shape");
    let text = source(&session, "a.ts");
    let (facts, navigation) = identity(&session, &fixture, "a.ts");
    fixture.write("package.json", r#"{"type":"module"}"#);
    let after = fixture.references(&mut session, "Shape");
    assert_ne!(before, after, "module-local Shape no longer binds b.ts");
    let (new_facts, new_navigation) = identity(&session, &fixture, "a.ts");
    assert_ne!(facts, new_facts);
    assert_eq!(navigation, new_navigation);
    assert!(Arc::ptr_eq(&text, &source(&session, "a.ts")));
    fixture.write("b.ts", "let value: Shape; value;\n");
    fixture.references(&mut session, "Shape");
    assert_eq!(
        (new_facts, new_navigation),
        identity(&session, &fixture, "a.ts"),
        "cache retains final package-format facts"
    );
    fixture.write("package.json", r#"{"type":"commonjs"}"#);
    let restored = fixture.references(&mut session, "Shape");
    let ProjectResponse::References(restored) = restored else {
        unreachable!()
    };
    assert_eq!(restored.groups[0].occurrences.len(), 2);
    assert_ne!(new_facts, identity(&session, &fixture, "a.ts").0);
    assert_eq!(navigation, identity(&session, &fixture, "a.ts").1);
}

#[test]
fn configuration_changes_recollect_only_when_module_status_changes() {
    let fixture = Fixture::new();
    fixture.write("a.ts", "interface Shape { value: number }\n");
    fixture.write("b.ts", "let value: Shape;\n");
    let mut session = fixture.session();
    for (detection, occurrences) in [("legacy", 2), ("force", 1), ("auto", 2), ("legacy", 2)] {
        fixture.write("tsconfig.json", &format!(r#"{{"compilerOptions":{{"noLib":true,"types":[],"module":"esnext","moduleResolution":"bundler","moduleDetection":"{detection}"}},"files":["a.ts","b.ts"]}}"#));
        let ProjectResponse::References(answer) = fixture.references(&mut session, "Shape") else {
            unreachable!()
        };
        assert_eq!(
            answer.groups[0].occurrences.len(),
            occurrences,
            "{detection}"
        );
    }
}

#[test]
fn library_capture_is_reused_and_rebuilt_when_it_becomes_a_search_root() {
    let fixture = Fixture::new();
    let library = "node_modules/@typescript/lib-es5/index.d.ts";
    fixture.write(
        "node_modules/@typescript/lib-es5/package.json",
        r#"{"types":"index.d.ts"}"#,
    );
    fixture.write(library, "declare const libraryValue: number;\n");
    fixture.write("a.ts", "export const value = 1; libraryValue;\n");
    fixture.write(
        "tsconfig.json",
        r#"{"compilerOptions":{"types":[],"lib":["es5"],"libReplacement":true},"files":["a.ts"]}"#,
    );
    let mut session = fixture.session();
    fixture.references(&mut session, "value");
    let products = identity(&session, &fixture, library);
    assert!(products.1.is_none());
    fixture.write("a.ts", "export const value = 2; libraryValue;\n");
    fixture.references(&mut session, "value");
    assert_eq!(products, identity(&session, &fixture, library));
    fixture.write(
        "tsconfig.json",
        &format!(
            r#"{{"compilerOptions":{{"noLib":true,"types":[]}},"files":["a.ts","{library}"]}}"#
        ),
    );
    fixture.references(&mut session, "value");
    let searched = identity(&session, &fixture, library);
    assert_ne!(products.0, searched.0);
    assert!(searched.1.is_some());
    let request = ProjectRequest::Outline {
        file: fixture.0.join(library),
    };
    assert_eq!(
        session.execute(request.clone()).unwrap(),
        Project::load(&fixture.0.join("tsconfig.json"))
            .unwrap()
            .execute(request)
            .unwrap()
    );
}

#[test]
fn decoded_text_reuse_survives_bom_addition_and_removal() {
    let fixture = Fixture::new();
    fixture.write("a.ts", "export const value = 1;\n");
    fixture.write("b.ts", "export const stable = 2;\n");
    let mut session = fixture.session();
    fixture.references(&mut session, "value");
    let before = identity(&session, &fixture, "b.ts");
    for (i, bom) in ["\u{feff}", ""].into_iter().enumerate() {
        fixture.write("a.ts", &format!("export const value = {};\n", i + 2));
        fixture.write("b.ts", &format!("{bom}export const stable = 2;\n"));
        fixture.references(&mut session, "stable");
        assert_eq!(before, identity(&session, &fixture, "b.ts"));
    }
}

#[test]
fn synthetic_helpers_and_jsx_requests_remain_local_to_each_generation() {
    let fixture = Fixture::new();
    fixture.write("tsconfig.json", r#"{"compilerOptions":{"noLib":true,"types":[],"jsx":"react-jsx","module":"esnext","moduleResolution":"bundler","importHelpers":true,"isolatedModules":true},"files":["a.ts","view.tsx"]}"#);
    fixture.write(
        "node_modules/tslib/package.json",
        r#"{"types":"index.d.ts"}"#,
    );
    fixture.write(
        "node_modules/tslib/index.d.ts",
        "export declare const __assign: any;\n",
    );
    fixture.write(
        "node_modules/react/package.json",
        r#"{"exports":{"./jsx-runtime":"./jsx-runtime.d.ts"}}"#,
    );
    fixture.write(
        "node_modules/react/jsx-runtime.d.ts",
        "export namespace JSX { interface IntrinsicElements { div: any } }\n",
    );
    fixture.write("view.tsx", "export const view = <div />;\n");
    let mut session = fixture.session();
    let mut previous = None;
    for generation in 1..=3 {
        fixture.write("a.ts", &format!("export const value = {generation};\n"));
        fixture.references(&mut session, "view");
        let products = identity(&session, &fixture, "view.tsx");
        if let Some(previous) = previous {
            assert_eq!(products, previous);
        }
        previous = Some(products);
        assert_eq!(session.generation(), generation);
    }
}

#[test]
fn json_products_are_shared_and_removed_inputs_are_released() {
    let fixture = Fixture::new();
    fixture.write("tsconfig.json", r#"{"compilerOptions":{"noLib":true,"types":[],"resolveJsonModule":true,"module":"esnext","moduleResolution":"bundler"},"files":["a.ts"]}"#);
    fixture.write(
        "a.ts",
        "import data from './data.json'; export const value = data;\n",
    );
    fixture.write("data.json", r#"{"value":1}"#);
    let mut session = fixture.session();
    fixture.references(&mut session, "value");
    let data = identity(&session, &fixture, "data.json");
    let weak = Arc::downgrade(&source(&session, "data.json"));
    fixture.write(
        "a.ts",
        "import data from './data.json'; export const value = data; value;\n",
    );
    fixture.references(&mut session, "value");
    assert_eq!(data, identity(&session, &fixture, "data.json"));
    fixture.write("a.ts", "export const value = 2;\n");
    fixture.references(&mut session, "value");
    assert!(
        weak.upgrade().is_none(),
        "only reached inputs survive a successful generation"
    );
}

#[test]
fn parse_failure_preserves_reuse_but_never_answers_from_it() {
    let fixture = Fixture::new();
    fixture.write("a.ts", "export const value = 1;\n");
    fixture.write("b.ts", "export const stable = 2;\n");
    let mut session = fixture.session();
    fixture.references(&mut session, "value");
    let stable = identity(&session, &fixture, "b.ts");
    fixture.write("a.ts", "export const value = ;\n");
    let request = ProjectRequest::References(ReferenceQuery::Name {
        name: "value".into(),
        file: None,
    });
    assert!(session.execute(request).is_err());
    assert!(session.current.is_none());
    assert_eq!(session.generation(), 1);
    fixture.write("a.ts", "export const value = 3;\n");
    fixture.references(&mut session, "value");
    assert_eq!(session.generation(), 2);
    assert_eq!(stable, identity(&session, &fixture, "b.ts"));
}
