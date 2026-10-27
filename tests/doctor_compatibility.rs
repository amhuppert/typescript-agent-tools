// validation: run alone: it starts project services, which must become ready within ten seconds.
//! The doctor's `compatibility` line: the compiler installed for the
//! configuration, the dialect it selects, the semantic authority, and the
//! standard library's source (the vendored TypeScript 7.0.2 declarations,
//! or the installed compiler's own under TypeScript 5.6), from the same
//! selection loading makes
//! (`compatibility::select`), read-only. A selection that fails is a
//! finding with a remedy; the line is the same whether or not the project's
//! service runs, and the service reads the project with the dialect it
//! names.
#![cfg(unix)]
mod support;

use std::process::Output;
use support::service::Service;
use support::{BASE_CONFIG, Fixture, MANIFEST, manifest};

/// TS1540 under TypeScript 7, a suggestion under 5.6.
const MODULE_KEYWORD: &str = "module N { export const x = 1; }\nexport const value = N.x;\n";

const SEMANTICS: &str =
    "semantics: TypeScript 7.0.2; standard library: the vendored TypeScript 7.0.2 declarations";

/// The semantic authority and the TypeScript 5.6 dialect's standard
/// library: the declarations of the compiler installed in `package`, its
/// real directory.
fn semantics_5_6(package: &std::path::Path) -> String {
    format!(
        "semantics: TypeScript 7.0.2; standard library: the installed compiler's declarations in {}",
        package.join("lib").display()
    )
}

fn service(files: &[(&str, &str)]) -> Service {
    let mut all = vec![
        ("tsconfig.json", BASE_CONFIG.as_str()),
        ("value.ts", MODULE_KEYWORD),
    ];
    all.extend_from_slice(files);
    Service::of(Fixture::new(&all))
}

fn doctor(service: &Service, json: bool) -> Output {
    let mut command = service.command();
    command
        .env_remove("TSPARSE_MAX_MEMORY_MB")
        .env_remove("TSPARSE_QUERY_TIMEOUT_MS")
        .args(["--doctor", "--project", "tsconfig.json"]);
    if json {
        command.arg("--json");
    }
    command.output().unwrap()
}

/// The report's `compatibility:` line and what follows it up to the next
/// check.
fn compatibility(output: &Output) -> Vec<String> {
    assert!(output.stderr.is_empty(), "{output:?}");
    let report = String::from_utf8(output.stdout.clone()).unwrap();
    let mut lines = report
        .lines()
        .skip_while(|line| !line.starts_with("compatibility: "));
    let mut found: Vec<String> = lines.next().into_iter().map(str::to_owned).collect();
    found.extend(
        lines
            .take_while(|line| line.starts_with("  "))
            .map(str::to_owned),
    );
    assert!(!found.is_empty(), "no compatibility line: {report}");
    found
}

#[test]
fn the_compatibility_line_names_the_compiler_dialect_and_authority() {
    let none = service(&[]);
    let output = doctor(&none, false);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        compatibility(&output),
        [format!(
            "compatibility: no TypeScript package is installed in or above the configuration's directory; read with the TypeScript 7 dialect; {SEMANTICS}"
        )]
    );
    for (version, dialect, note) in [
        ("5.6.3", "TypeScript 5.6", ""),
        ("5.4.5", "TypeScript 5.6", ", validated for 5.6.3 only"),
        ("5.6.4", "TypeScript 5.6", ", validated for 5.6.3 only"),
        ("5.6.3-rc.1", "TypeScript 5.6", ", validated for 5.6.3 only"),
        // A build of 5.6.3 is not the release that was validated.
        (
            "5.6.3+local",
            "TypeScript 5.6",
            ", validated for 5.6.3 only",
        ),
        (
            "5.6.3+sha.abc",
            "TypeScript 5.6",
            ", validated for 5.6.3 only",
        ),
        ("6.0.3", "TypeScript 7", ""),
        ("7.0.2", "TypeScript 7", ""),
    ] {
        let service = service(&[(MANIFEST, &manifest(version))]);
        let output = doctor(&service, false);
        assert_eq!(output.status.code(), Some(0), "{version}: {output:?}");
        let semantics = if dialect == "TypeScript 5.6" {
            semantics_5_6(&service.fixture.path("node_modules/typescript"))
        } else {
            SEMANTICS.to_owned()
        };
        assert_eq!(
            compatibility(&output),
            [format!(
                "compatibility: TypeScript {version} at {}{note}; read with the {dialect} dialect; {semantics}",
                service.fixture.path(MANIFEST).display()
            )],
            "{version}"
        );
    }
}

/// A package reached through a link is named as found and as its real path,
/// where its standard library is read.
#[test]
fn a_linked_compiler_is_named_through_its_link() {
    let service = service(&[("store/typescript/package.json", &manifest("5.6.3"))]);
    std::fs::create_dir_all(service.fixture.path("node_modules")).unwrap();
    std::os::unix::fs::symlink(
        "../store/typescript",
        service.fixture.path("node_modules/typescript"),
    )
    .unwrap();
    let output = doctor(&service, false);
    assert_eq!(
        compatibility(&output),
        [format!(
            "compatibility: TypeScript 5.6.3 at {} ({}); read with the TypeScript 5.6 dialect; {}",
            service.fixture.path(MANIFEST).display(),
            service
                .fixture
                .path("store/typescript/package.json")
                .display(),
            semantics_5_6(&service.fixture.path("store/typescript"))
        )]
    );
}

/// A manifest linked elsewhere is named as found and as its real path, while
/// the standard library is read beside it, in its package directory's
/// `lib`, where the compiler runs from.
#[test]
fn a_linked_manifest_keeps_the_package_directorys_library() {
    let service = service(&[("metadata/package.json", &manifest("5.6.3"))]);
    std::fs::create_dir_all(service.fixture.path("node_modules/typescript")).unwrap();
    std::os::unix::fs::symlink(
        "../../metadata/package.json",
        service.fixture.path(MANIFEST),
    )
    .unwrap();
    let output = doctor(&service, false);
    assert_eq!(
        compatibility(&output),
        [format!(
            "compatibility: TypeScript 5.6.3 at {} ({}); read with the TypeScript 5.6 dialect; {}",
            service.fixture.path(MANIFEST).display(),
            service.fixture.path("metadata/package.json").display(),
            semantics_5_6(&service.fixture.path("node_modules/typescript"))
        )]
    );
}

/// A manifest that names no version is a finding, with what loading would
/// report and a remedy, in text and JSON.
#[test]
fn an_unusable_manifest_is_a_finding_with_a_remedy() {
    let service = service(&[(MANIFEST, r#"{"version":"latest"}"#)]);
    let output = doctor(&service, false);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let manifest = service.fixture.path(MANIFEST);
    assert_eq!(
        compatibility(&output),
        [
            format!(
                "compatibility: {}:1:12: The TypeScript compiler manifest has the 'version' \"latest\", which is not a version, so no dialect can be selected",
                manifest.display()
            ),
            "  next: repair or reinstall the TypeScript package that manifest belongs to, or remove it".into(),
        ]
    );
    let report = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(report.ends_with("findings: compatibility\n"), "{report}");
    let document: serde_json::Value =
        serde_json::from_slice(&doctor(&service, true).stdout).unwrap();
    let check = document["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "compatibility")
        .unwrap();
    assert_eq!(check["status"], "finding");
    assert_eq!(
        check["remedy"],
        "repair or reinstall the TypeScript package that manifest belongs to, or remove it"
    );
}

/// The line is the same before and while the project's service runs, and
/// the service, like a direct load, reads the project with the dialect the
/// line names: a `module` namespace loads under TypeScript 5.6 and fails
/// under TypeScript 7.
#[test]
fn direct_and_service_reads_agree_with_the_doctor() {
    for (version, loads) in [("5.6.3", true), ("7.0.2", false)] {
        let service = service(&[(MANIFEST, &manifest(version))]);
        let before = compatibility(&doctor(&service, false));
        let answer = service.both(&["--references", "--name", "value"]);
        assert_eq!(answer.status.success(), loads, "{version}: {answer:?}");
        if !loads {
            let message = String::from_utf8(answer.stderr.clone()).unwrap();
            assert!(message.contains("'module' keyword"), "{message}");
        }
        let during = compatibility(&doctor(&service, false));
        assert_eq!(before, during, "{version}");
    }
}
