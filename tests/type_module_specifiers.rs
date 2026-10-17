//! Module display's resolution metadata must not demand unrelated import types.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeQuery, TypeRequest};

fn assert_missing_alias(fresh: bool) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-module-specifiers"),
    );
    for profile in ["bundler", "no-package-modes", "node"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        for row in rows {
            let file = row["file"].as_str().unwrap();
            let mode = row["defaultMode"].as_u64().unwrap();
            let first = row["imports"]
                .as_array()
                .unwrap()
                .iter()
                .find(|import| import["resolvedPath"] == row["resolvedPath"])
                .unwrap();
            let first_mode = first["mode"].as_u64().unwrap();
            // The native matrix proves the first matching written import owns
            // mode compatibility, including the native None mode (zero).
            let needs_generation = mode != 0 && first_mode != 0 && mode != first_mode;
            if fresh != needs_generation {
                continue;
            }
            let source = std::fs::read_to_string(fixture.path(file)).unwrap();
            let marker = "/*#missing*/";
            let at = position(&source, source.find(marker).unwrap() + marker.len());
            let project = fixture.load_config(&format!("{profile}-{file}.json"));
            for expand in [false, true, false] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path(file),
                        position: at,
                    },
                    expand,
                    depth: 0,
                });
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{profile}/{file}: {answer:?}");
                };
                assert_eq!(
                    error.kind,
                    SemanticErrorKind::InvalidSemantic,
                    "{profile}/{file}: {error:?}"
                );
                assert_eq!(
                    error.reason,
                    SemanticFailureReason::ErrorType.into(),
                    "{profile}/{file}: {error:?}"
                );
                assert_eq!(error.position, Some(at), "{profile}/{file}: {error:?}");
                assert_eq!(
                    error.path,
                    Some(fixture.path(file)),
                    "{profile}/{file}: {error:?}"
                );
                assert!(error.causes.is_empty(), "{profile}/{file}: {error:?}");
            }
        }
    }
}

#[test]
fn reused_imports_observe_raw_resolution_and_exclude_synthetic_requests() {
    assert_missing_alias(false);
}

#[test]
fn incompatible_first_import_requires_fresh_module_specifier_generation() {
    assert_missing_alias(true);
}
