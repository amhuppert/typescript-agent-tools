//! Compiler-derived semantic regressions and explicit recursion boundaries.
use super::*;
use crate::resources::{Limits, Usage};
use crate::source::SourcePosition;
use crate::types::TypeQuery;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture {
    directory: PathBuf,
    project: Project,
    request: TypeRequest,
}
impl Fixture {
    fn new(source: &str) -> Self {
        Self::with_query(source, "chosen", false)
    }
    fn with_query(source: &str, marker: &str, no_check: bool) -> Self {
        Self::with_null_checks(source, marker, no_check, true)
    }
    fn with_null_checks(
        source: &str,
        marker: &str,
        no_check: bool,
        strict_null_checks: bool,
    ) -> Self {
        Self::with_target(source, marker, no_check, strict_null_checks, None)
    }
    fn with_target(
        source: &str,
        marker: &str,
        no_check: bool,
        strict_null_checks: bool,
        target: Option<&str>,
    ) -> Self {
        let mut options = serde_json::json!({
            "strict": true, "strictNullChecks": strict_null_checks,
            "noLib": true, "noCheck": no_check,
        });
        if let Some(target) = target {
            options["target"] = target.into();
        }
        Self::with_options(source, marker, options)
    }
    fn with_options(source: &str, marker: &str, options: serde_json::Value) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "tsparse-resource-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("tsconfig.json"),
            serde_json::json!({"compilerOptions": options, "files": ["cases.ts"]}).to_string(),
        )
        .unwrap();
        let path = directory.join("cases.ts");
        std::fs::write(&path, source).unwrap();
        let project = Project::load(&directory.join("tsconfig.json")).unwrap();
        let start = source.rfind(marker).unwrap();
        let mut line = 1;
        let mut column = 1;
        for c in source[..start].chars() {
            if c == '\n' {
                line += 1;
                column = 1;
            } else {
                column += c.len_utf16();
            }
        }
        let request = TypeRequest::new(TypeQuery::At {
            file: path,
            position: SourcePosition { line, column },
        });
        Self {
            directory,
            project,
            request,
        }
    }
    fn query(&self, limits: Limits) -> Result<(TypeAnswer, Usage), AnalysisError> {
        self.query_request(&self.request, limits)
    }
    fn query_request(
        &self,
        request: &TypeRequest,
        limits: Limits,
    ) -> Result<(TypeAnswer, Usage), AnalysisError> {
        worker_with_limits(&self.project, limits, |parser, control| {
            let answer = type_of_here(&self.project, request, parser, control.clone())?;
            Ok((answer, control.usage()))
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}
fn healthy(result: Result<(TypeAnswer, Usage), AnalysisError>) -> Usage {
    let (answer, usage) = result.unwrap();
    let TypeAnswer::Selected(answer) = answer else {
        panic!("selection failed: {answer:?}")
    };
    assert_eq!(answer.outcome, TypeOutcome::Found);
    assert_eq!(answer.description.summary, "number");
    assert!(answer.issues.is_empty());
    assert_eq!(usage.depth, 0);
    usage
}
fn interrupted(
    result: Result<(TypeAnswer, Usage), AnalysisError>,
    kind: SemanticErrorKind,
    reason: SemanticResourceReason,
) {
    assert_interrupted(result, kind, reason);
}

fn assert_interrupted<T>(
    result: Result<T, AnalysisError>,
    kind: SemanticErrorKind,
    reason: SemanticResourceReason,
) {
    let error = match result {
        Err(AnalysisError::Semantic(error)) => error,
        _ => panic!("interruption became a type result or another error family"),
    };
    assert_eq!(error.kind, kind);
    assert_eq!(error.reason, reason.into());
    assert!(error.path.is_none());
    assert!(error.position.is_none());
    assert!(error.causes.is_empty());
}

mod accessor_declarations;
mod completed_return;
mod completed_return_evidence;
mod completed_return_located;
mod demand_cache;
mod emit_helpers;
mod enum_cache;
mod error_source_lookup;
mod late_symbol_fallback;
mod mapping_depth;
mod named_declarations;
mod recovery_child;
mod recovery_observation;
mod reference_gap_evidence;
mod reference_gaps;
mod reference_normalization;
mod signature_return_publication;
mod source_symbol_declarations;
mod value_cache;

mod emit_helper_identity;

mod late_symbol_identity;

mod reference_import_branches;

mod reference_implicit_imports;

mod containment;

mod promise_stack;

#[test]
fn request_resource_source_flat_speculation_does_not_spend_nested_depth() {
    let source = |count| {
        format!(
            "const ignored = ({}) => 1; declare const chosen: number; chosen;",
            (0..count)
                .map(|n| format!("a{n}"))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let short = Fixture::new(&source(8));
    let long = Fixture::new(&source(800));
    let short_usage = healthy(short.query(Limits::default()));
    let long_usage = healthy(long.query(Limits {
        depth: Some(short_usage.peak_depth),
    }));
    assert_eq!(short_usage.peak_depth, long_usage.peak_depth);
}

fn healthy_successor(fixture: &Fixture, source: &str) {
    let prefix = &source[..source.rfind("healthy").unwrap()];
    let successor = TypeRequest::new(TypeQuery::At {
        file: fixture.directory.join("cases.ts"),
        position: SourcePosition {
            line: prefix.bytes().filter(|&c| c == b'\n').count() + 1,
            column: prefix.rsplit('\n').next().unwrap().encode_utf16().count() + 1,
        },
    });
    healthy(fixture.query_request(&successor, Limits::default()));
}
