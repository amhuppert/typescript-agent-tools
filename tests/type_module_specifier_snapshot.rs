//! Physical spelling inputs belong to the loaded generation. Anticipatory
//! failures stay inert until the native spelling operation reaches their path.
#![cfg(unix)]
mod support;
use support::service::Service;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, InputErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true,"types":[],"module":"NodeNext","moduleResolution":"NodeNext"},"files":["globals.d.ts","fresh.cts","reuse.cts"]}"#,
        ),
        ("package.json", r#"{"type":"commonjs"}"#),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-module-specifier-snapshot/globals.d.ts"),
        ),
        (
            "fresh.cts",
            include_str!("../fixtures/types/native-module-specifier-snapshot/fresh.cts"),
        ),
        (
            "reuse.cts",
            include_str!("../fixtures/types/native-module-specifier-snapshot/reuse.cts"),
        ),
        (
            "node_modules/pkg/package.json",
            r#"{"name":"pkg","types":"./entry.d.ts"}"#,
        ),
        (
            "node_modules/pkg/sub/index.d.ts",
            include_str!("../fixtures/types/native-module-specifier-snapshot/target.d.ts"),
        ),
    ])
}

fn missing(project: &Project, fixture: &Fixture, file: &str) -> AnalysisError {
    let source = std::fs::read_to_string(fixture.path(file)).unwrap();
    let marker = "/*#missing*/";
    let at = position(&source, source.find(marker).unwrap() + marker.len());
    project
        .type_of(TypeRequest::new(TypeQuery::At {
            file: fixture.path(file),
            position: at,
        }))
        .unwrap_err()
}

fn assert_unknown(error: AnalysisError) {
    let AnalysisError::Semantic(error) = error else {
        panic!("{error:?}");
    };
    assert_eq!(
        error.reason,
        SemanticFailureReason::ErrorType.into(),
        "{error:?}"
    );
}

fn assert_healthy(project: &Project, fixture: &Fixture) {
    let answer = project
        .type_of(TypeRequest::new(TypeQuery::Name {
            name: "healthy".to_owned(),
            file: Some(fixture.path("fresh.cts")),
        }))
        .unwrap();
    let TypeAnswer::Selected(answer) = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
    assert_eq!(answer.description.summary, "\"healthy\"");
    assert!(answer.issues.is_empty());
}

#[test]
fn speculative_spelling_failure_is_demanded_and_frozen() {
    let fixture = fixture();
    let probe = fixture.path("node_modules/pkg/sub.ts");
    std::os::unix::fs::symlink("sub.ts", &probe).unwrap();
    let project = fixture.load();
    assert_healthy(&project, &fixture);
    assert_unknown(missing(&project, &fixture, "reuse.cts"));
    let error = missing(&project, &fixture, "fresh.cts");
    let AnalysisError::Input(input) = &error else {
        panic!("{error:?}");
    };
    assert_eq!(input.kind, InputErrorKind::Io);
    assert_eq!(input.path.as_ref(), Some(&probe));
    std::fs::remove_file(&probe).unwrap();
    assert_eq!(missing(&project, &fixture, "fresh.cts"), error);
    assert_healthy(&project, &fixture);
    assert_unknown(missing(&fixture.load(), &fixture, "fresh.cts"));
}

#[test]
fn unavailable_cwd_is_demanded_only_by_spelling_and_frozen_by_the_service() {
    for expand in [false, true] {
        let service = Service::of(fixture());
        let config = service.fixture.path("tsconfig.json");
        let gone = service.fixture.path("gone");
        let run_in_removed = |args: &[&str]| {
            std::fs::create_dir(&gone).unwrap();
            std::process::Command::new("/bin/sh")
                .args([
                    "-c",
                    "cd \"$1\" && rmdir \"$1\" && shift && exec \"$@\"",
                    "sh",
                ])
                .arg(&gone)
                .arg(env!("CARGO_BIN_EXE_tsparse"))
                .args(args)
                .arg("--project")
                .arg(&config)
                .env("TSPARSE_SERVICE_DIR", &service.runtime)
                .env("TSPARSE_SERVICE_IDLE_SECS", "60")
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap()
        };
        let selected = service.fixture.path("fresh.cts");
        let healthy = [
            "--type",
            "--name",
            "healthy",
            "--json",
            selected.to_str().unwrap(),
        ];
        for direct in [true, false] {
            let mut args = healthy.to_vec();
            if direct {
                args.push("--direct");
            }
            if expand {
                args.push("--expand");
            }
            let output = run_in_removed(&args);
            assert!(output.status.success(), "{direct}/{expand}: {output:?}");
            let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["outcome"], "found", "{document}");
        }
        assert_eq!(service.generation(), 1);
        for file in ["fresh.cts", "reuse.cts"] {
            let file = service.fixture.path(file);
            let mut args = vec![
                "--type",
                "--name",
                "Missing",
                "--json",
                file.to_str().unwrap(),
            ];
            if expand {
                args.push("--expand");
            }
            // The service still owns the failed cwd captured by its healthy
            // first request, even when this caller has a valid cwd again.
            let served = service
                .command()
                .args(&args)
                .arg("--project")
                .arg(&config)
                .output()
                .unwrap();
            args.push("--direct");
            let direct = run_in_removed(&args);
            for output in [&direct, &served] {
                assert_eq!(output.status.code(), Some(2), "{expand}: {output:?}");
                assert!(output.stderr.is_empty(), "{output:?}");
                let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(document["error"]["kind"], "Io", "{document}");
                assert_eq!(document["error"]["path"], ".", "{document}");
            }
            assert_eq!(direct.stdout, served.stdout);
            assert_eq!(service.generation(), 1);
        }
    }
}

#[test]
fn frozen_absence_ignores_new_failure_but_service_refreshes_probe_changes() {
    let fixture = fixture();
    let probe = fixture.path("node_modules/pkg/sub.ts");
    let project = fixture.load();
    assert_unknown(missing(&project, &fixture, "fresh.cts"));
    let service = Service::of(fixture);
    let healthy = ["--type", "--name", "healthy", "fresh.cts", "--json"];
    assert_eq!(
        support::service::json(service.run(&healthy))["outcome"],
        "found"
    );
    assert_eq!(service.generation(), 1);
    std::os::unix::fs::symlink("sub.ts", &probe).unwrap();
    assert_unknown(missing(&project, &service.fixture, "fresh.cts"));
    assert_eq!(
        support::service::json(service.run(&healthy))["outcome"],
        "found"
    );
    assert_eq!(service.generation(), 2);
    let args = ["--type", "--name", "Missing", "fresh.cts", "--json"];
    let failed = service.run(&args);
    assert_eq!(failed.status.code(), Some(2), "{failed:?}");
    assert!(failed.stderr.is_empty(), "{failed:?}");
    let document: serde_json::Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(document["error"]["kind"], "Io", "{document}");
    assert_eq!(
        document["error"]["path"],
        probe.to_str().unwrap(),
        "{document}"
    );
    assert_eq!(service.generation(), 2);
    std::fs::remove_file(&probe).unwrap();
    assert_eq!(
        support::service::json(service.run(&healthy))["outcome"],
        "found"
    );
    assert_eq!(service.generation(), 3);
    let recovered = service.run(&args);
    assert_eq!(recovered.status.code(), Some(1), "{recovered:?}");
    assert!(recovered.stderr.is_empty(), "{recovered:?}");
    let document: serde_json::Value = serde_json::from_slice(&recovered.stdout).unwrap();
    assert_eq!(document["error"]["reason"], "error_type", "{document}");
    assert_eq!(service.generation(), 3);
}

#[test]
fn runtime_dependency_inference_reuses_the_cwd_alias_before_fresh_probes() {
    // Native runtime.cts controls distinguish the raw cwd alias from the
    // physical package path. A poisoned fresh-only probe makes that decision
    // observable through the public query's final failure category.
    for runtime in [true, false] {
        let fixture = fixture();
        std::fs::write(
            fixture.path("runtime.cts"),
            include_str!("../fixtures/types/native-module-specifier-snapshot/runtime.cts"),
        )
        .unwrap();
        std::fs::create_dir(fixture.path("sub")).unwrap();
        std::fs::write(
            fixture.path("sub/index.d.ts"),
            include_str!("../fixtures/types/native-module-specifier-snapshot/target.d.ts"),
        )
        .unwrap();
        std::fs::write(fixture.path("tsconfig.json"), r#"{"compilerOptions":{"strict":true,"noLib":true,"types":[],"module":"NodeNext","moduleResolution":"NodeNext"},"files":["globals.d.ts","runtime.cts"]}"#).unwrap();
        std::fs::write(
            fixture.path("package.json"),
            if runtime {
                r#"{"type":"commonjs","dependencies":{"pkg":"*"}}"#
            } else {
                r#"{"type":"commonjs","devDependencies":{"pkg":"*"}}"#
            },
        )
        .unwrap();
        std::os::unix::fs::symlink("sub.ts", fixture.path("node_modules/pkg/sub.ts")).unwrap();
        let service = Service::of(fixture);
        // Child cwd matches the native host; changing the test process cwd
        // would interfere with concurrent project tests.
        let output = service.run(&[
            "--type",
            "--name",
            "Missing",
            "runtime.cts",
            "--json",
            "--direct",
        ]);
        assert!(output.stderr.is_empty(), "{runtime}: {output:?}");
        let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        if runtime {
            assert_eq!(output.status.code(), Some(1), "{document}");
            assert_eq!(document["error"]["reason"], "error_type", "{document}");
        } else {
            assert_eq!(output.status.code(), Some(2), "{document}");
            assert_eq!(document["error"]["kind"], "Io", "{document}");
            assert_eq!(
                document["error"]["path"],
                service
                    .fixture
                    .path("node_modules/pkg/sub.ts")
                    .to_str()
                    .unwrap(),
                "{document}"
            );
        }
    }
}
