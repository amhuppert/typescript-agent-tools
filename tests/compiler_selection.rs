//! Which compiler's rules read a project: the nearest
//! `node_modules/typescript/package.json` above the configuration selects
//! the TypeScript 5.6 dialect for any 5.x release and TypeScript 7 for
//! anything else or nothing (docs/references.md, "Compiler selection").
//! Each case is a scratch project whose own `node_modules` declares the
//! version, observed where it decides an outcome: `Project::load`, a
//! session's refreshed answer, or the doctor.

mod support;

use support::{BASE_CONFIG, Fixture, MANIFEST, input_error, manifest};
use typescript_parser::analysis::{InputError, InputErrorKind};
use typescript_parser::project::Project;

/// A manifest that cannot name a version fails loading, naming it, rather
/// than selecting a dialect it does not declare: unreadable text is `Io`,
/// and text that is not a JSON object with a version string is
/// `Unsupported`, even with a usable manifest further up.
#[test]
fn an_unusable_compiler_manifest_fails_loading_and_names_it() {
    let cases: [(&[u8], InputErrorKind, &str); 10] = [
        (b"{", InputErrorKind::Unsupported, "is not valid JSON"),
        (
            b"{\"version\":\"5.6.3\",}",
            InputErrorKind::Unsupported,
            "is not valid JSON",
        ),
        (b"[]", InputErrorKind::Unsupported, "is not a JSON object"),
        (
            br#"{"name":"typescript"}"#,
            InputErrorKind::Unsupported,
            "has no 'version'",
        ),
        (
            br#"{"version":5}"#,
            InputErrorKind::Unsupported,
            "'version' that is not a string",
        ),
        (
            br#"{"version":null}"#,
            InputErrorKind::Unsupported,
            "'version' that is not a string",
        ),
        (
            br#"{"version":"latest"}"#,
            InputErrorKind::Unsupported,
            "the 'version' \"latest\", which is not a version",
        ),
        (
            br#"{"version":"5.6"}"#,
            InputErrorKind::Unsupported,
            "the 'version' \"5.6\", which is not a version",
        ),
        (
            br#"{"version":""}"#,
            InputErrorKind::Unsupported,
            "which is not a version",
        ),
        (b"\xff\xfe{}", InputErrorKind::Io, "valid UTF-8"),
    ];
    for (text, kind, message) in cases {
        for nested in [false, true] {
            let fixture = Fixture::new(&[
                ("app/tsconfig.json", &BASE_CONFIG),
                ("app/a.ts", "export const a = 1;\n"),
                (MANIFEST, &manifest("5.6.3")),
            ]);
            let broken = if nested {
                fixture.path(&format!("app/{MANIFEST}"))
            } else {
                fixture.path(MANIFEST)
            };
            std::fs::create_dir_all(broken.parent().unwrap()).unwrap();
            std::fs::write(&broken, text).unwrap();
            let error = fixture
                .try_load_config("app/tsconfig.json")
                .err()
                .unwrap_or_else(|| panic!("{text:?} loaded"));
            let context = format!(
                "{:?} nested={nested}: {error}",
                String::from_utf8_lossy(text)
            );
            assert_eq!(error.kind, kind, "{context}");
            assert!(error.message.contains(message), "{context}");
            assert_eq!(error.path.as_deref(), Some(broken.as_path()), "{context}");
        }
    }
}

/// A manifest the process may not read is `Io`, never a missing package.
#[cfg(unix)]
#[test]
fn an_unreadable_compiler_manifest_is_an_io_error() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new(&[
        ("tsconfig.json", &BASE_CONFIG),
        ("a.ts", "export const a = 1;\n"),
        (MANIFEST, &manifest("5.6.3")),
    ]);
    let path = fixture.path(MANIFEST);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let result = fixture.try_load();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let error = result.expect_err("an unreadable manifest loads");
    assert_eq!(error.kind, InputErrorKind::Io, "{error}");
    assert_eq!(error.path.as_deref(), Some(path.as_path()));
}

/// A file TypeScript 7 rejects and TypeScript 5.6 accepts: `module N {}`
/// is TS1540, an error in 7.0.2 and a suggestion in 5.6.3.
const MODULE_KEYWORD: &str = "module N { export const x = 1; }\nexport const y = N.x;\n";

/// Whether loading read the project with the TypeScript 5.6 dialect, as the
/// TS1540 decision shows; any other failure is the test's.
fn read_as_5_6(fixture: &Fixture, config: &str) -> bool {
    match fixture.try_load_config(config) {
        Ok(_) => true,
        Err(error) if error.kind == InputErrorKind::Syntax => {
            assert!(
                error.message.contains("'module' keyword"),
                "not TS1540: {error}"
            );
            false
        }
        Err(error) => panic!("{config} failed: {error}"),
    }
}

/// Selection by the nearest manifest's major version: 5 selects the
/// TypeScript 5.6 dialect whichever 5.x it is; no package and any other
/// major version select TypeScript 7.
#[test]
fn the_nearest_manifests_major_version_selects_the_dialect() {
    let none = Fixture::new(&[("tsconfig.json", &BASE_CONFIG), ("a.ts", MODULE_KEYWORD)]);
    assert!(!read_as_5_6(&none, "tsconfig.json"), "no package");
    for (version, expected) in [
        ("7.0.2", false),
        ("6.0.3", false),
        ("4.9.5", false),
        ("8.0.0", false),
        ("5.6.3", true),
        ("5.4.5", true),
        ("5.0.0", true),
        ("5.7.0-beta", true),
        ("5.6.3+local", true),
    ] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", &BASE_CONFIG),
            ("a.ts", MODULE_KEYWORD),
            (MANIFEST, &manifest(version)),
        ]);
        assert_eq!(
            read_as_5_6(&fixture, "tsconfig.json"),
            expected,
            "{version}"
        );
    }
}

/// The nearest manifest wins: a package nearer the configuration shadows
/// an ancestor's in either direction, and a candidate without a manifest,
/// a manifest that is a directory, or a dangling link is passed over. No
/// `node_modules/node_modules` is a candidate.
#[test]
fn the_nearest_manifest_shadows_its_ancestors() {
    for (ancestor, nearer, expected) in [("5.6.3", "7.0.2", false), ("7.0.2", "5.6.3", true)] {
        let fixture = Fixture::new(&[
            ("app/tsconfig.json", &BASE_CONFIG),
            ("app/a.ts", MODULE_KEYWORD),
            (MANIFEST, &manifest(ancestor)),
            (&format!("app/{MANIFEST}"), &manifest(nearer)),
        ]);
        assert_eq!(
            read_as_5_6(&fixture, "app/tsconfig.json"),
            expected,
            "{nearer} under {ancestor}"
        );
    }
    let fixture = Fixture::new(&[
        ("app/tsconfig.json", &BASE_CONFIG),
        ("app/a.ts", MODULE_KEYWORD),
        ("app/node_modules/typescript/lib/typescript.js", ""),
        ("deep/tsconfig.json", &BASE_CONFIG),
        ("deep/a.ts", MODULE_KEYWORD),
        ("deep/node_modules/typescript/package.json/index.json", "{}"),
        (
            "node_modules/node_modules/typescript/package.json",
            &manifest("7.0.2"),
        ),
        ("node_modules/inner/tsconfig.json", &BASE_CONFIG),
        ("node_modules/inner/a.ts", MODULE_KEYWORD),
        (MANIFEST, &manifest("5.6.3")),
    ]);
    assert!(read_as_5_6(&fixture, "app/tsconfig.json"), "no manifest");
    assert!(read_as_5_6(&fixture, "deep/tsconfig.json"), "a directory");
    assert!(
        read_as_5_6(&fixture, "node_modules/inner/tsconfig.json"),
        "node_modules/node_modules"
    );
}

/// A package reached through symbolic links is read through them, the
/// link to `node_modules` or to the package directory alike; a dangling
/// link is no package.
#[cfg(unix)]
#[test]
fn a_linked_compiler_package_is_selected_through_its_links() {
    let fixture = Fixture::new(&[
        ("store/typescript-5.6.3/package.json", &manifest("5.6.3")),
        ("store/modules/typescript/package.json", &manifest("5.4.5")),
        ("package/tsconfig.json", &BASE_CONFIG),
        ("package/a.ts", MODULE_KEYWORD),
        ("modules/tsconfig.json", &BASE_CONFIG),
        ("modules/a.ts", MODULE_KEYWORD),
        ("dangling/tsconfig.json", &BASE_CONFIG),
        ("dangling/a.ts", MODULE_KEYWORD),
    ]);
    let link = |link: &str, target: &str| {
        let link = fixture.path(link);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, link).unwrap();
    };
    link(
        "package/node_modules/typescript",
        "../../store/typescript-5.6.3",
    );
    link("modules/node_modules", "../store/modules");
    link("dangling/node_modules/typescript", "../../store/missing");
    assert!(read_as_5_6(&fixture, "package/tsconfig.json"));
    assert!(read_as_5_6(&fixture, "modules/tsconfig.json"));
    assert!(!read_as_5_6(&fixture, "dangling/tsconfig.json"));
}

/// A session's answer after each change that selects another compiler
/// equals a fresh direct load's, failures included: the change is observed
/// although no source changed.
#[test]
fn a_session_refreshes_when_the_selected_compiler_changes() {
    use typescript_parser::project::{ProjectRequest, ProjectSession};
    use typescript_parser::references::ReferenceQuery;
    let fixture = Fixture::new(&[
        ("app/tsconfig.json", &BASE_CONFIG),
        ("app/a.ts", MODULE_KEYWORD),
        (MANIFEST, &manifest("5.6.3")),
    ]);
    let config = fixture.path("app/tsconfig.json");
    let request = ProjectRequest::References(ReferenceQuery::Name {
        name: "y".into(),
        file: None,
    });
    let mut session = ProjectSession::new(&config).unwrap();
    // Asks the session, then a fresh load, which must agree; returns
    // whether the 5.6 dialect read the project.
    let mut answer = |step: &str| {
        let refreshed = session.execute(request.clone());
        let fresh = Project::load(&config).and_then(|project| project.execute(request.clone()));
        match (&refreshed, &fresh) {
            (Ok(refreshed), Ok(fresh)) => assert_eq!(refreshed, fresh, "{step}"),
            (Err(refreshed), Err(fresh)) => {
                assert_eq!(refreshed.to_string(), fresh.to_string(), "{step}")
            }
            _ => panic!("{step}: the session answered {refreshed:?}, a fresh load {fresh:?}"),
        }
        refreshed.map_err(input_error)
    };
    let is_5_6 = |result: Result<_, InputError>, step: &str| match result {
        Ok(_) => true,
        Err(error) if error.kind == InputErrorKind::Syntax => false,
        Err(error) => panic!("{step}: {error}"),
    };
    assert!(is_5_6(answer("5.6.3 above"), "5.6.3 above"));
    // The manifest's version is edited in place.
    fixture.claim_compiler("7.0.2");
    assert!(!is_5_6(answer("edited to 7.0.2"), "edited to 7.0.2"));
    // A nearer package appears, shadowing the ancestor's.
    let nearer = fixture.path(&format!("app/{MANIFEST}"));
    std::fs::create_dir_all(nearer.parent().unwrap()).unwrap();
    std::fs::write(&nearer, manifest("5.4.5")).unwrap();
    assert!(is_5_6(answer("nearer 5.4.5"), "nearer 5.4.5"));
    // The nearer package is removed: the ancestor's is selected again.
    std::fs::remove_dir_all(fixture.path("app/node_modules")).unwrap();
    assert!(!is_5_6(answer("nearer removed"), "nearer removed"));
    // A malformed manifest fails; repairing it answers again.
    std::fs::write(fixture.path(MANIFEST), "{\"version\":").unwrap();
    let error = answer("malformed").expect_err("a malformed manifest loads");
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
    fixture.claim_compiler("5.6.3");
    assert!(is_5_6(answer("repaired"), "repaired"));
    // The manifest becomes unreadable: `Io`, never the next candidate.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = fixture.path(MANIFEST);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let error = answer("unreadable");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let error = error.expect_err("an unreadable manifest loads");
        assert_eq!(error.kind, InputErrorKind::Io, "{error}");
        assert!(is_5_6(answer("readable again"), "readable again"));
    }
}

/// Retargeting a link on the way to the manifest selects the compiler its
/// new target declares, whether the link is the package directory or
/// `node_modules` itself, and a dangling link's target appearing selects it.
#[cfg(unix)]
#[test]
fn a_session_refreshes_when_a_link_to_the_compiler_is_retargeted() {
    use typescript_parser::project::{ProjectRequest, ProjectSession};
    use typescript_parser::references::ReferenceQuery;
    for linked in ["node_modules/typescript", "node_modules"] {
        let target = |name: &str| {
            if linked == "node_modules" {
                format!("../store/{name}")
            } else {
                format!("../../store/{name}/typescript")
            }
        };
        let fixture = Fixture::new(&[
            ("app/tsconfig.json", &BASE_CONFIG),
            ("app/a.ts", MODULE_KEYWORD),
            ("store/five/typescript/package.json", &manifest("5.6.3")),
            ("store/seven/typescript/package.json", &manifest("7.0.2")),
        ]);
        let config = fixture.path("app/tsconfig.json");
        let link = fixture.path(&format!("app/{linked}"));
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target("five"), &link).unwrap();
        let request = ProjectRequest::References(ReferenceQuery::Name {
            name: "y".into(),
            file: None,
        });
        let mut session = ProjectSession::new(&config).unwrap();
        let mut agree = |step: &str| {
            let refreshed = session.execute(request.clone()).map_err(|e| e.to_string());
            let fresh = Project::load(&config)
                .and_then(|project| project.execute(request.clone()))
                .map_err(|e| e.to_string());
            assert_eq!(refreshed, fresh, "{linked}: {step}");
            refreshed.is_ok()
        };
        assert!(agree("linked to 5.6.3"));
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(target("seven"), &link).unwrap();
        assert!(!agree("retargeted to 7.0.2"));
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(target("later"), &link).unwrap();
        assert!(!agree("dangling"));
        let later = fixture.path("store/later/typescript");
        std::fs::create_dir_all(&later).unwrap();
        std::fs::write(later.join("package.json"), manifest("5.4.5")).unwrap();
        assert!(agree("the dangling link's target installed"));
    }
}

/// A setting the TypeScript 7 profile removed is reported with the profile
/// and, when another compiler is installed, that compiler: the profile's
/// rule is not the installed compiler's.
#[test]
fn removed_settings_name_the_installed_compiler_beside_the_profile() {
    for (version, installed) in [
        (None, None),
        (Some("7.0.2"), None),
        (Some("7.1.0-dev"), None),
        (Some("6.0.3"), Some("installed compiler: TypeScript 6.0.3")),
        (Some("4.9.5"), Some("installed compiler: TypeScript 4.9.5")),
    ] {
        for (option, message, kind) in [
            (
                r#""baseUrl":".""#,
                "Option 'baseUrl' was removed in TypeScript 7{}; use 'paths' relative to the configuration",
                InputErrorKind::Unsupported,
            ),
            (
                r#""moduleResolution":"node10""#,
                "Option 'moduleResolution=node10' was removed in TypeScript 7{}",
                InputErrorKind::Unsupported,
            ),
            (
                r#""esModuleInterop":false"#,
                "Option 'esModuleInterop=false' was removed in TypeScript 7{}",
                InputErrorKind::Config,
            ),
        ] {
            let config =
                format!(r#"{{"compilerOptions":{{"noLib":true,{option}}},"files":["a.ts"]}}"#);
            let mut files = vec![("tsconfig.json", config), ("a.ts", "export {};\n".into())];
            if let Some(version) = version {
                files.push((MANIFEST, manifest(version)));
            }
            let files: Vec<(&str, &str)> = files.iter().map(|(a, b)| (*a, b.as_str())).collect();
            let fixture = Fixture::new(&files);
            let error = fixture.try_load().expect_err("loaded");
            let note = installed.map_or(String::new(), |note| {
                format!(", whose rules read this project ({note})")
            });
            assert_eq!(error.kind, kind, "{version:?} {option}");
            assert_eq!(
                error.message,
                message.replace("{}", &note),
                "{version:?} {option}"
            );
        }
    }
}
