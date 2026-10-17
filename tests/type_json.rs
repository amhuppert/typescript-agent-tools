mod support;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use support::position as support_position;
use typescript_parser::analysis::{
    AnalysisError, SemanticError, SemanticErrorKind, SemanticFailureReason, SemanticReason,
};
use typescript_parser::project::Project;
use typescript_parser::references::{Location, NamedLocation};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{
    PrimitiveName, SelectionFailure, TypeAnswer, TypeKey, TypeNodeData, TypeOutcome, TypeQuery,
    TypeQuestion, TypeRef, TypeRequest, TypeResult,
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        Self::from("json-native-basic")
    }

    fn from(directory: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "tsparse-native-json-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types")
            .join(directory);
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                std::fs::copy(entry.path(), root.join(entry.file_name())).unwrap();
            }
        }
        Self(root)
    }

    fn project(&self, strict: bool, exact: bool, unchecked: bool) -> Project {
        Project::load(&self.0.join(format!(
            "strict-{}-exact-{}-unchecked-{}.json",
            u8::from(strict),
            u8::from(exact),
            u8::from(unchecked)
        )))
        .unwrap()
    }

    fn at(&self, marker: &str) -> TypeQuery {
        let source = std::fs::read_to_string(self.0.join("consumer.ts")).unwrap();
        let annotation = format!("/*#{marker}*/");
        let offset = source.find(&annotation).unwrap() + annotation.len();
        let before = &source[..offset];
        TypeQuery::At {
            file: self.0.join("consumer.ts"),
            position: SourcePosition {
                line: before.bytes().filter(|&byte| byte == b'\n').count() + 1,
                column: before.rsplit('\n').next().unwrap().encode_utf16().count() + 1,
            },
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn selected(project: &Project, query: TypeQuery) -> TypeResult {
    let mut request = TypeRequest::new(query);
    request.expand = true;
    let answer = project.type_of(request);
    assert!(
        matches!(answer, Ok(TypeAnswer::Selected(_))),
        "imported JSON must establish its native widened type: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert_eq!(result.target.kind, TypeQuestion::Value);
    assert!(result.issues.is_empty());
    assert!(!result.description.summary_truncated);
    result
}

#[test]
fn imported_json_scalar_aliases_widen_the_last_exact_value_in_each_admitted_profile() {
    let fixture = Fixture::new();
    // Pinned TS 7.0.2 controls contain all eight flag states. The two states
    // with exact=true/strict=false report native Program TS5052 and are not
    // healthy oracle profiles; global diagnostic admission is a separate gate.
    for strict in [false, true] {
        for exact in [false, true] {
            if !strict && exact {
                continue;
            }
            for unchecked in [false, true] {
                let project = fixture.project(strict, exact, unchecked);
                for (alias, summary, primitive) in [
                    ("stringValue", "string", PrimitiveName::String),
                    ("numberValue", "number", PrimitiveName::Number),
                    ("booleanValue", "boolean", PrimitiveName::Boolean),
                    ("duplicateValue", "number", PrimitiveName::Number),
                    ("decodedValue", "boolean", PrimitiveName::Boolean),
                    ("emptyValue", "number", PrimitiveName::Number),
                    ("canonicalNumber", "boolean", PrimitiveName::Boolean),
                    ("noncanonicalNumber", "string", PrimitiveName::String),
                    ("astralValue", "boolean", PrimitiveName::Boolean),
                    ("typeOnlyValue", "string", PrimitiveName::String),
                ] {
                    let result = selected(&project, fixture.at(alias));
                    assert_eq!(result.description.summary, summary, "{alias}");
                    let graph = result.description.expansion.unwrap();
                    assert!(!graph.truncated);
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    assert_eq!(root.data, TypeNodeData::Primitive { name: primitive });
                    assert!(root.alias.is_none());
                }
            }
        }
    }
}

#[test]
fn imported_json_arrays_use_the_real_generic_target_member_and_index_types() {
    let fixture = Fixture::from("json-array-targets/valid-members");
    // Fresh TS7 controls establish a genuine interface Array<T>, a number
    // element/argument, its own length origin and its substituted number index.
    // noLib does not imply an empty or missing global target.
    for strict in [false, true] {
        for exact in [false, true] {
            if !strict && exact {
                continue;
            }
            for unchecked in [false, true] {
                let project = fixture.project(strict, exact, unchecked);
                let result = selected(&project, fixture.at("numbers"));
                assert_eq!(result.description.summary, "number[]");
                let graph = result.description.expansion.unwrap();
                assert!(!graph.truncated);
                let root = graph
                    .nodes
                    .iter()
                    .find(|node| node.id == graph.root)
                    .unwrap();
                let TypeNodeData::Array {
                    element,
                    readonly,
                    members,
                } = &root.data
                else {
                    panic!("native reference to the actual global Array target: {root:?}")
                };
                assert!(!readonly);
                assert_eq!(root.type_arguments, Some(vec![element.clone()]));
                let TypeRef::Ref { r#ref } = element else {
                    panic!("established element identity")
                };
                assert_eq!(
                    graph
                        .nodes
                        .iter()
                        .find(|node| node.id == *r#ref)
                        .unwrap()
                        .data,
                    TypeNodeData::Primitive {
                        name: PrimitiveName::Number
                    }
                );
                let properties = members.properties.as_ref().unwrap();
                assert_eq!(properties.len(), 1);
                assert_eq!(
                    properties[0].key,
                    TypeKey::String {
                        name: "length".into()
                    }
                );
                assert_eq!(properties[0].read_type, *element);
                assert_eq!(
                    properties[0].origins,
                    vec![Location {
                        file: "globals.ts".into(),
                        start: SourcePosition {
                            line: 1,
                            column: 22
                        },
                        end: SourcePosition {
                            line: 1,
                            column: 28
                        },
                    }]
                );
                assert!(!properties[0].optional);
                assert!(!properties[0].readonly);
                let indexes = members.index_signatures.as_ref().unwrap();
                assert_eq!(indexes.len(), 1);
                assert_eq!(indexes[0].key_type, *element);
                assert_eq!(indexes[0].value_type, *element);
                assert!(!indexes[0].readonly);
                assert!(members.call_signatures.as_ref().unwrap().is_empty());
                assert!(members.construct_signatures.as_ref().unwrap().is_empty());
                assert!(members.blocked_collections.is_none());
                assert!(root.omitted.is_none());
            }
        }
    }
}

fn location(line: usize, start: usize, end: usize) -> Location {
    Location {
        file: "data.json".into(),
        start: SourcePosition {
            line,
            column: start,
        },
        end: SourcePosition { line, column: end },
    }
}

/// Native controls in type-native-json-recovery-replay-oracle.test.mjs pin
/// construction, getWidenedProperty's cache read, and the optional selected
/// property read. Each acquisition retains its complete recovery evidence.
fn assert_array_recovery_replays(
    error: &SemanticError,
    message: &str,
    global: Option<Location>,
    array: Location,
    replays: usize,
) {
    let global = serde_json::json!({
        "reason": if message == "Cannot find global type 'Array'." {
            SemanticFailureReason::UnresolvedSymbol
        } else { SemanticFailureReason::ErrorType },
        "location": global, "message": message,
    });
    let consumed = serde_json::json!({
        "reason": SemanticFailureReason::ErrorType,
        "location": array,
        "message": "global type 'Array' dependency consumed compiler recovery",
    });
    let mut expected = vec![global.clone()];
    for _ in 0..replays {
        expected.push(consumed.clone());
        expected.push(global.clone());
    }
    let actual: Vec<_> = error
        .causes
        .iter()
        .map(|cause| {
            serde_json::json!({
                "reason": cause.reason, "location": cause.location, "message": cause.message,
            })
        })
        .collect();
    assert_eq!(actual, expected, "{error:?}");
    assert!(error.omitted_causes.is_none(), "{error:?}");
}

#[test]
fn imported_json_objects_preserve_final_values_all_origins_and_captured_generation() {
    let fixture = Fixture::new();
    let query = fixture.at("nestedValue");
    let project = fixture.project(true, false, false);
    // Type construction must use retained facts from the successful generation.
    std::fs::write(fixture.0.join("data.json"), r#"{"nested":{"changed":1}}"#).unwrap();
    std::fs::write(
        fixture.0.join("consumer.ts"),
        "export const nestedValue = false;",
    )
    .unwrap();
    for query in [
        query,
        TypeQuery::Name {
            name: "nestedValue".into(),
            file: None,
        },
    ] {
        let result = selected(&project, query);
        assert_eq!(
            result.description.summary,
            "{ kept: boolean; dup: { last: string; }; }"
        );
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        let TypeNodeData::Object { members } = &root.data else {
            panic!("JSON object type")
        };
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(properties.len(), 2);
        assert_eq!(
            properties[0].key,
            TypeKey::String {
                name: "kept".into()
            }
        );
        assert_eq!(properties[0].origins, vec![location(10, 14, 20)]);
        assert_eq!(properties[1].key, TypeKey::String { name: "dup".into() });
        assert_eq!(
            properties[1].origins,
            vec![location(10, 29, 34), location(10, 48, 53)]
        );
        assert!(
            properties
                .iter()
                .all(|property| !property.optional && !property.readonly)
        );
        let TypeRef::Ref { r#ref } = &properties[1].read_type else {
            panic!("JSON child type")
        };
        let nested = graph.nodes.iter().find(|node| node.id == *r#ref).unwrap();
        let TypeNodeData::Object { members } = &nested.data else {
            panic!("last JSON duplicate wins")
        };
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(properties.len(), 1);
        assert_eq!(
            properties[0].key,
            TypeKey::String {
                name: "last".into()
            }
        );
        assert_eq!(properties[0].origins, vec![location(10, 56, 62)]);
        let TypeRef::Ref { r#ref } = &properties[0].read_type else {
            panic!("JSON primitive type")
        };
        assert_eq!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == *r#ref)
                .unwrap()
                .data,
            TypeNodeData::Primitive {
                name: PrimitiveName::String
            }
        );
    }
}

#[test]
fn rich_json_aliases_consume_the_whole_modules_invalid_array_dependency() {
    let fixture = Fixture::new();
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/json-native/data.json"),
        fixture.0.join("data.json"),
    )
    .unwrap();
    // The raw pinned scalar root is non-error, but array siblings consume
    // missing-global recovery with noLib. Its complete module construction
    // consumes the empty-array factory before returning any named export.
    // Evaluating only the selected leaf would drop that invalid dependency.
    for strict in [false, true] {
        let project = fixture.project(strict, false, false);
        let error = project
            .type_of(TypeRequest::new(fixture.at("stringValue")))
            .unwrap_err();
        let AnalysisError::Semantic(error) = error else {
            panic!("consumed native global recovery")
        };
        assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
        assert_eq!(
            error.reason,
            SemanticReason::Failure(SemanticFailureReason::ErrorType)
        );
        assert_eq!(
            error.position,
            Some(SourcePosition {
                line: 6,
                column: 12
            })
        );
        assert!(
            error
                .causes
                .iter()
                .all(|cause| !cause.message.contains("'Object'")),
            "JSON object value declarations must retain native excess-property gating: {error:?}"
        );
        // Native checks seven arrays, then getWidenedProperty reads each
        // array-bearing slot once (the recursive slot retains three arrays).
        // Fourteen consumed recoveries produce 27 cause entries: the initial
        // global cause, followed by a primary/global pair for each acquisition.
        // The public cause limit retains the first sixteen and omits eleven.
        let source = std::fs::read_to_string(fixture.0.join("data.json")).unwrap();
        let arrays: Vec<_> = [
            "[]",
            "[1, 2]",
            "[1, \"s\", null]",
            "[{\"a\": 1}, {\"b\": \"s\"}]",
            "[1]",
            "[\"s\", null]",
            "[[1], [\"s\", null]]",
        ]
        .iter()
        .map(|text| {
            let start = source.find(text).unwrap();
            Location {
                file: "data.json".into(),
                start: support_position(&source, start),
                end: support_position(&source, start + text.len()),
            }
        })
        .collect();
        let global = serde_json::json!({"reason": "unresolved_symbol", "location": null,
            "message": "Cannot find global type 'Array'."});
        let mut expected = vec![global.clone()];
        for array in arrays.iter().skip(1).chain(arrays.iter().take(2)) {
            expected.push(
                serde_json::json!({"reason": "error_type", "location": array,
                "message": "global type 'Array' dependency consumed compiler recovery"}),
            );
            expected.push(global.clone());
        }
        expected.truncate(16);
        let actual: Vec<_> = error
            .causes
            .iter()
            .map(|cause| {
                serde_json::json!({
                    "reason": cause.reason, "location": cause.location, "message": cause.message,
                })
            })
            .collect();
        assert_eq!(actual, expected, "{error:?}");
        assert_eq!(error.omitted_causes, Some(11), "{error:?}");
        assert_eq!(
            error.causes[0].reason,
            SemanticReason::Failure(SemanticFailureReason::UnresolvedSymbol)
        );
        assert_eq!(error.causes[0].message, "Cannot find global type 'Array'.");
        assert_eq!(error.causes[0].location, None);
        assert_eq!(
            error.path,
            Some(std::fs::canonicalize(fixture.0.join("data.json")).unwrap())
        );
    }
}

#[test]
fn ambiguous_json_aliases_keep_own_exact_origins_and_replay_the_value_question() {
    let fixture = Fixture::from("json-native-ambiguity");
    let project = fixture.project(true, false, false);
    let answer = project.type_of(TypeRequest::new(TypeQuery::Name {
        name: "collision".into(),
        file: None,
    }));
    assert!(
        matches!(answer, Ok(TypeAnswer::Unselected(_))),
        "distinct JSON values must return their native candidates: {answer:?}"
    );
    let TypeAnswer::Unselected(selection) = answer.unwrap() else {
        unreachable!()
    };
    let SelectionFailure::Ambiguous { mut candidates } = selection.outcome else {
        panic!("two canonical JSON property targets")
    };
    candidates.sort_by(|left, right| left.file.cmp(&right.file));
    assert_eq!(candidates.len(), 2);
    for (candidate, file, summary, origins) in [
        (
            &candidates[0],
            "first.ts",
            "number",
            vec![location(11, 3, 8), location(12, 3, 8)],
        ),
        (
            &candidates[1],
            "second.ts",
            "boolean",
            vec![Location {
                file: "other.json".into(),
                start: SourcePosition { line: 1, column: 2 },
                end: SourcePosition { line: 1, column: 7 },
            }],
        ),
    ] {
        assert_eq!(candidate.selector.as_deref(), Some("collision"));
        assert_eq!(candidate.file, file);
        assert_eq!(candidate.question, TypeQuestion::Value);
        assert_eq!(
            candidate.declarations,
            origins
                .into_iter()
                .map(|location| NamedLocation {
                    name: "dup".into(),
                    location
                })
                .collect::<Vec<_>>()
        );
        assert_eq!(candidate.anchor.file, file);
        assert_eq!((candidate.anchor.line, candidate.anchor.column), (1, 31));
        let result = selected(
            &project,
            TypeQuery::At {
                file: fixture.0.join(&candidate.anchor.file),
                position: SourcePosition {
                    line: candidate.anchor.line,
                    column: candidate.anchor.column,
                },
            },
        );
        assert_eq!(result.description.summary, summary);
        assert_eq!(result.target.kind, candidate.question);
    }
}

#[test]
fn imported_json_null_uses_shared_widening_and_keeps_genuine_any_distinct() {
    let fixture = Fixture::from("json-native-null");
    for strict in [false, true] {
        for exact in [false, true] {
            if !strict && exact {
                continue;
            }
            for unchecked in [false, true] {
                let project = fixture.project(strict, exact, unchecked);
                let summary = if strict { "null" } else { "any" };
                let expected = if strict {
                    TypeNodeData::Null
                } else {
                    TypeNodeData::Any
                };
                for marker in ["nullValue", "toNull", "typeOnlyNull"] {
                    let result = selected(&project, fixture.at(marker));
                    assert_eq!(result.description.summary, summary, "{marker}");
                    let graph = result.description.expansion.unwrap();
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    assert_eq!(root.data, expected, "{marker}");
                }
                let result = selected(&project, fixture.at("fromNull"));
                assert_eq!(result.description.summary, "number");
                let nested = selected(&project, fixture.at("nestedNull"));
                assert_eq!(nested.description.summary, format!("{{ nil: {summary}; }}"));
                let graph = nested.description.expansion.unwrap();
                let root = graph
                    .nodes
                    .iter()
                    .find(|node| node.id == graph.root)
                    .unwrap();
                let TypeNodeData::Object { members } = &root.data else {
                    panic!("nested JSON object")
                };
                let properties = members.properties.as_ref().unwrap();
                assert_eq!(properties.len(), 1);
                assert_eq!(properties[0].key, TypeKey::String { name: "nil".into() });
                assert_eq!(properties[0].origins, vec![location(3, 14, 19)]);
                let TypeRef::Ref { r#ref } = &properties[0].read_type else {
                    panic!("established nullable child")
                };
                assert_eq!(
                    graph
                        .nodes
                        .iter()
                        .find(|node| node.id == *r#ref)
                        .unwrap()
                        .data,
                    expected
                );
            }
        }
    }
}

#[test]
fn json_array_consumes_exact_global_array_recovery_before_returning_any_module_export() {
    for (case, message, cause_site, missing) in [
        ("missing", "Cannot find global type 'Array'.", None, true),
        ("value-only", "Cannot find global type 'Array'.", None, true),
        (
            "module-local",
            "Cannot find global type 'Array'.",
            None,
            true,
        ),
        (
            "type-alias",
            "Global type 'Array' must be a class or interface type.",
            Some((1, 6)),
            false,
        ),
        (
            "enum",
            "Global type 'Array' must be a class or interface type.",
            Some((1, 6)),
            false,
        ),
        (
            "arity-zero",
            "Global type 'Array' must have 1 type parameter(s).",
            Some((1, 11)),
            false,
        ),
        (
            "arity-two",
            "Global type 'Array' must have 1 type parameter(s).",
            Some((1, 11)),
            false,
        ),
        (
            "script-alias",
            "Global type 'Array' must be a class or interface type.",
            None,
            false,
        ),
        (
            "merged-optional-two",
            "Global type 'Array' must have 1 type parameter(s).",
            Some((1, 11)),
            false,
        ),
    ] {
        let fixture = Fixture::from(&format!("json-array-targets/{case}"));
        for strict in [false, true] {
            for exact in [false, true] {
                if !strict && exact {
                    continue;
                }
                for unchecked in [false, true] {
                    let project = fixture.project(strict, exact, unchecked);
                    for marker in ["numbers", "healthy"] {
                        let answer = project.type_of(TypeRequest::new(fixture.at(marker)));
                        assert!(
                            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic),
                            "{case}/{marker} must preserve consumed global recovery: {answer:?}"
                        );
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            unreachable!()
                        };
                        assert_eq!(
                            error.reason,
                            SemanticReason::Failure(SemanticFailureReason::ErrorType)
                        );
                        assert_eq!(
                            error.path,
                            Some(std::fs::canonicalize(fixture.0.join("data.json")).unwrap())
                        );
                        let json = std::fs::read_to_string(fixture.0.join("data.json")).unwrap();
                        assert_eq!(
                            error.position,
                            Some(SourcePosition {
                                line: 1,
                                column: json[..json.find('[').unwrap()].encode_utf16().count() + 1
                            })
                        );
                        let start = json.find('[').unwrap();
                        assert_array_recovery_replays(
                            &error,
                            message,
                            cause_site.map(|(line, column)| Location {
                                file: "globals.ts".into(),
                                start: SourcePosition { line, column },
                                end: SourcePosition {
                                    line,
                                    column: column + 5,
                                },
                            }),
                            Location {
                                file: "data.json".into(),
                                start: support_position(&json, start),
                                end: support_position(&json, start + "[1,2]".len()),
                            },
                            if marker == "numbers" { 2 } else { 1 },
                        );
                        let cause = &error.causes[0];
                        assert_eq!(cause.message, message, "{case}");
                        assert_eq!(
                            cause.reason,
                            SemanticReason::Failure(if missing {
                                SemanticFailureReason::UnresolvedSymbol
                            } else {
                                SemanticFailureReason::ErrorType
                            })
                        );
                        let expected = cause_site.map(|(line, column)| Location {
                            file: "globals.ts".into(),
                            start: SourcePosition { line, column },
                            end: SourcePosition {
                                line,
                                column: column + 5,
                            },
                        });
                        assert_eq!(
                            cause.location, expected,
                            "absent native global cause sites stay absent: {case}"
                        );
                        assert_eq!(error.omitted_causes, None);
                    }
                }
            }
        }
    }
}

#[test]
fn global_array_causes_preserve_canonical_declaration_order_and_raw_name_spans() {
    for (case, message, cause) in [
        (
            "augmentation-before-script",
            "Global type 'Array' must have 1 type parameter(s).",
            Some(("globals-second.ts", 11, 16)),
        ),
        (
            "scripts-forward",
            "Global type 'Array' must have 1 type parameter(s).",
            Some(("globals-first.ts", 11, 16)),
        ),
        (
            "scripts-reverse",
            "Global type 'Array' must have 1 type parameter(s).",
            Some(("globals-second.ts", 11, 16)),
        ),
        (
            "escaped-type-alias",
            "Global type 'Array' must be a class or interface type.",
            Some(("globals.ts", 6, 16)),
        ),
        ("value-alias", "Cannot find global type 'Array'.", None),
        ("folded-case", "Cannot find global type 'Array'.", None),
    ] {
        let fixture = Fixture::from(&format!("json-array-targets-review/{case}"));
        let project = fixture.project(true, false, false);
        let Err(AnalysisError::Semantic(error)) =
            project.type_of(TypeRequest::new(fixture.at("healthy")))
        else {
            panic!("selected scalar consumes its module's invalid Array dependency")
        };
        assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic, "{case}");
        let json = std::fs::read_to_string(fixture.0.join("data.json")).unwrap();
        let start = json.find('[').unwrap();
        assert_array_recovery_replays(
            &error,
            message,
            cause.map(|(file, start, end)| Location {
                file: file.into(),
                start: SourcePosition {
                    line: 1,
                    column: start,
                },
                end: SourcePosition {
                    line: 1,
                    column: end,
                },
            }),
            Location {
                file: "data.json".into(),
                start: support_position(&json, start),
                end: support_position(&json, start + "[1,2]".len()),
            },
            1,
        );
        assert_eq!(error.causes[0].message, message, "{case}");
        assert_eq!(
            error.causes[0].location,
            cause.map(|(file, start, end)| Location {
                file: file.into(),
                start: SourcePosition {
                    line: 1,
                    column: start
                },
                end: SourcePosition {
                    line: 1,
                    column: end
                },
            }),
            "canonical declaration order and exact raw name span: {case}"
        );
    }
}
