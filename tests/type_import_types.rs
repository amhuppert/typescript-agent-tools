//! `import("...")` type nodes replayed from the native 7.0.2 rows that
//! `tools/type-native-import-types-oracle.test.mjs` captures: type and
//! `typeof` meanings, qualifier chains, aliases, type arguments, the
//! `resolution-mode` attribute, located failures, and member references
//! through import-type qualifiers.
mod support;
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, QueryErrorKind, SemanticErrorKind, SemanticFailureReason, SemanticReason,
};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

fn fixture() -> Fixture {
    Fixture::existing(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/native-import-types"),
    )
}

fn expected(fixture: &Fixture) -> Value {
    serde_json::from_str(&std::fs::read_to_string(fixture.path("expected.json")).unwrap()).unwrap()
}

fn request(fixture: &Fixture, file: &str, name: &str) -> TypeRequest {
    let source = std::fs::read_to_string(fixture.path(file)).unwrap();
    let marker = format!("/*#{name}*/");
    TypeRequest::new(TypeQuery::At {
        file: fixture.path(file),
        position: position(&source, source.find(&marker).unwrap() + marker.len()),
    })
}

/// Where a failure is located: at TypeScript's diagnostic, or at the queried
/// import type when TypeScript reports the cause elsewhere. An alias that
/// resolves to nothing reports TS2305 at the alias, and this tool fails the
/// consuming type node, as it does for a plain type reference.
#[derive(Clone, Copy, PartialEq)]
enum At {
    Diagnostic,
    ImportType,
}

struct Failure {
    name: &'static str,
    code: u64,
    file: &'static str,
    line: usize,
    reason: SemanticFailureReason,
    message: &'static str,
    at: At,
}

/// Each failure the native compiler reports as a diagnostic beside its
/// errorType; the diagnostic's code, file and line identify it.
const FAILURES: &[Failure] = &[
    Failure {
        name: "missingMember",
        code: 2694,
        file: "src/errors.ts",
        line: 2,
        reason: SemanticFailureReason::UnresolvedSymbol,
        message: "namespace has no exported member `Missing`",
        at: At::Diagnostic,
    },
    Failure {
        name: "missingNested",
        code: 2694,
        file: "src/errors.ts",
        line: 3,
        reason: SemanticFailureReason::UnresolvedSymbol,
        message: "namespace has no exported member `Missing`",
        at: At::Diagnostic,
    },
    Failure {
        name: "missingTypeofMember",
        code: 2694,
        file: "src/errors.ts",
        line: 4,
        reason: SemanticFailureReason::UnresolvedSymbol,
        message: "namespace has no exported member `absent`",
        at: At::Diagnostic,
    },
    Failure {
        name: "moduleAsType",
        code: 1340,
        file: "src/errors.ts",
        line: 5,
        reason: SemanticFailureReason::ErrorType,
        message: "module `./lib.js` does not refer to a type, but is used as a type here",
        at: At::Diagnostic,
    },
    Failure {
        name: "typeOnlyModuleValue",
        code: 1339,
        file: "src/errors.ts",
        line: 6,
        reason: SemanticFailureReason::ErrorType,
        message: "module `type-equals` does not refer to a value, but is used as a value here",
        at: At::Diagnostic,
    },
    Failure {
        name: "missingModule",
        code: 2307,
        file: "node_modules/broken/index.d.ts",
        line: 1,
        reason: SemanticFailureReason::UnresolvedSymbol,
        message: "module is not resolved",
        at: At::Diagnostic,
    },
    Failure {
        name: "missingTypeofModule",
        code: 2307,
        file: "node_modules/broken/index.d.ts",
        line: 2,
        reason: SemanticFailureReason::UnresolvedSymbol,
        message: "module is not resolved",
        at: At::Diagnostic,
    },
    Failure {
        name: "wrongArity",
        code: 2314,
        file: "src/aliases.ts",
        line: 3,
        reason: SemanticFailureReason::ErrorType,
        message: "generic type-alias argument count consumes native errorType recovery",
        at: At::Diagnostic,
    },
    Failure {
        name: "typeofTypeAlias",
        code: 2694,
        file: "src/aliases.ts",
        line: 4,
        reason: SemanticFailureReason::UnresolvedSymbol,
        message: "namespace has no exported member `Pair`",
        at: At::Diagnostic,
    },
    Failure {
        name: "aliasToNowhere",
        code: 2305,
        file: "src/broken.ts",
        line: 1,
        reason: SemanticFailureReason::ErrorType,
        message: "native type alias target is unknown",
        at: At::ImportType,
    },
    Failure {
        name: "circular",
        code: 2456,
        file: "src/circular.ts",
        line: 1,
        reason: SemanticFailureReason::CircularType,
        message: "type alias circularly references itself",
        at: At::Diagnostic,
    },
    Failure {
        name: "nonLiteral",
        code: 1141,
        file: "src/errors.ts",
        line: 11,
        reason: SemanticFailureReason::ErrorType,
        message: "an import type's argument must be a string literal",
        at: At::Diagnostic,
    },
];

/// Native diagnostics beside a healthy type: a type argument that violates
/// its import type's constraint is reported by checkTypeArgumentConstraints,
/// whose result does not change the type.
const DIAGNOSTIC_ONLY: &[(u64, &str, usize)] = &[(2344, "src/checks.ts", 4)];

#[test]
fn import_types_answer_native_types_and_located_failures() {
    let fixture = fixture();
    let expected = expected(&fixture);
    assert_eq!(expected["compiler"], "7.0.2");
    let rows = expected["types"].as_array().unwrap();
    let diagnostics = expected["diagnostics"].as_array().unwrap();
    for diagnostic in diagnostics {
        let key = (
            diagnostic["code"].as_u64().unwrap(),
            diagnostic["file"].as_str().unwrap(),
            diagnostic["line"].as_u64().unwrap() as usize,
        );
        assert!(
            FAILURES
                .iter()
                .any(|failure| (failure.code, failure.file, failure.line) == key)
                || DIAGNOSTIC_ONLY.contains(&key),
            "unaccounted native diagnostic {diagnostic}"
        );
    }
    let root = fixture.root.to_str().unwrap();
    for reverse in [false, true] {
        let project = fixture.load();
        let mut ordered: Vec<_> = rows.iter().collect();
        if reverse {
            ordered.reverse();
        }
        for row in ordered {
            let name = row["name"].as_str().unwrap();
            let file = row["file"].as_str().unwrap();
            for expand in [false, true] {
                let mut query = request(&fixture, file, name);
                query.expand = expand;
                let answer = project.type_of(query);
                let label = format!("{name}/{expand}/{reverse}");
                if row["unsupported"] == true {
                    let Err(AnalysisError::Query(error)) = answer else {
                        panic!("{label}: {answer:?}");
                    };
                    assert_eq!(error.kind, QueryErrorKind::UnsupportedTarget, "{label}");
                    continue;
                }
                if row["errorType"] == true {
                    let failure = FAILURES
                        .iter()
                        .find(|failure| failure.name == name)
                        .unwrap_or_else(|| panic!("{label}: native errorType without a failure"));
                    let diagnostic = diagnostics
                        .iter()
                        .find(|diagnostic| {
                            diagnostic["code"] == failure.code
                                && diagnostic["file"] == failure.file
                                && diagnostic["line"] == failure.line
                        })
                        .unwrap_or_else(|| panic!("{label}: no native TS{}", failure.code));
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{label}: {answer:?}");
                    };
                    assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic, "{label}");
                    assert_eq!(
                        error.reason,
                        SemanticReason::Failure(failure.reason),
                        "{label}"
                    );
                    assert_eq!(error.message, failure.message, "{label}");
                    let (path, line, column) = if failure.at == At::Diagnostic {
                        (
                            failure.file,
                            diagnostic["line"].as_u64().unwrap() as usize,
                            diagnostic["column"].as_u64().unwrap() as usize,
                        )
                    } else {
                        let source = std::fs::read_to_string(fixture.path(file)).unwrap();
                        let marker = source.find(&format!("/*#{name}*/")).unwrap();
                        let at =
                            position(&source, marker + source[marker..].find("import(").unwrap());
                        (file, at.line, at.column)
                    };
                    assert_eq!(
                        error.path.as_deref(),
                        Some(fixture.path(path).as_path()),
                        "{label}"
                    );
                    let at = error.position.unwrap();
                    assert_eq!((at.line, at.column), (line, column), "{label}");
                    continue;
                }
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{label}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{label}: {:?}",
                    result.issues
                );
                assert!(result.issues.is_empty(), "{label}: {:?}", result.issues);
                let kind = match row["kind"].as_str().unwrap() {
                    "value" => TypeQuestion::Value,
                    "type" => TypeQuestion::Type,
                    other => panic!("{label}: unexpected question {other}"),
                };
                assert_eq!(result.target.kind, kind, "{label}");
                assert_eq!(
                    result.description.summary.replace(root, "<root>"),
                    row["summary"].as_str().unwrap(),
                    "{label}"
                );
            }
        }
    }
}

fn location(fixture: &Fixture, value: &Value) -> String {
    let file = value["file"].as_str().unwrap();
    let start = &value.get("start").unwrap_or(value);
    let file = Path::new(file)
        .strip_prefix(&fixture.root)
        .map(|file| file.to_str().unwrap().to_owned())
        .unwrap_or_else(|_| file.to_owned());
    format!("{file}:{}:{}", start["line"], start["column"])
}

#[test]
fn member_references_reach_import_type_qualifiers_as_typescript_does() {
    let fixture = fixture();
    let expected = expected(&fixture);
    for row in expected["references"].as_array().unwrap() {
        let name = row["name"].as_str().unwrap();
        let file = row["file"].as_str().unwrap();
        let source = std::fs::read_to_string(fixture.path(file)).unwrap();
        let marker = format!("/*#{name}*/");
        let at = position(&source, source.find(&marker).unwrap() + marker.len());
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args(["--references", "--direct", "--json", "--project"])
            .arg(fixture.path("tsconfig.json"))
            .arg("--at")
            .arg(format!(
                "{}:{}:{}",
                fixture.path(file).display(),
                at.line,
                at.column
            ))
            .output()
            .unwrap();
        let answer: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| panic!("{name}: {}", String::from_utf8_lossy(&output.stderr)));
        // A candidate whose import type consumed a failure is a located gap,
        // even where TypeScript resolves it to an unrelated symbol: the
        // `value` in `typeof import("./lib.js").value.absent` names the
        // constant, but its node fails at `absent` (TS2694).
        let gaps: Vec<_> = answer["coverage"]["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|gap| {
                (
                    location(&fixture, &gap["location"]),
                    gap["reason"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        if name == "boxValue" {
            assert!(!output.status.success(), "{name}: {answer:#}");
            assert_eq!(answer["outcome"], "partial", "{name}: {answer:#}");
            assert_eq!(
                gaps,
                [(
                    "src/errors.ts:4:86".to_owned(),
                    "unresolved_symbol".to_owned()
                )],
                "{name}: {answer:#}"
            );
            let native = expected["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .find(|diagnostic| {
                    diagnostic["code"] == 2694
                        && diagnostic["file"] == "src/errors.ts"
                        && diagnostic["line"] == 4
                })
                .unwrap();
            assert_eq!(
                location(
                    &fixture,
                    &answer["coverage"]["gaps"][0]["causes"][0]["location"]
                ),
                location(&fixture, native),
                "{name}: {answer:#}"
            );
        } else {
            assert!(output.status.success(), "{name}: {answer:#}");
            assert_eq!(answer["outcome"], "complete", "{name}: {answer:#}");
            assert!(gaps.is_empty(), "{name}: {answer:#}");
        }
        let groups: Vec<(Vec<String>, Vec<String>)> = answer["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| {
                let mut origins: Vec<_> = group["declarations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|declaration| location(&fixture, &declaration["location"]))
                    .collect();
                let mut occurrences: Vec<_> = group["occurrences"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|occurrence| location(&fixture, &occurrence["location"]))
                    .collect();
                origins.sort();
                occurrences.sort();
                (origins, occurrences)
            })
            .collect();
        let native: Vec<(Vec<String>, Vec<String>)> = row["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| {
                let mut origins: Vec<_> = group["origins"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|origin| location(&fixture, origin))
                    .collect();
                let mut occurrences: Vec<_> = group["occurrences"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|occurrence| location(&fixture, occurrence))
                    .collect();
                origins.sort();
                occurrences.sort();
                (origins, occurrences)
            })
            .collect();
        assert_eq!(groups, native, "{name}: {answer:#}");
    }
}
