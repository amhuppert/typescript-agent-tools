use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use typescript_parser::project::Project;
use typescript_parser::references::Location;
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{
    PrimitiveName, SelectionFailure, TypeAnswer, TypeKey, TypeNodeData, TypeOutcome, TypeQuery,
    TypeQuestion, TypeRef, TypeRequest,
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "tsparse-native-vertical-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let fixture = Self(root);
        for file in [
            "tsconfig.json",
            "model.ts",
            "barrel.ts",
            "consumer.ts",
            "node_modules/native-fixture/package.json",
            "node_modules/native-fixture/index.ts",
        ] {
            let original = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/types/native-vertical")
                .join(file);
            fixture.write(file, &std::fs::read_to_string(original).unwrap());
        }
        fixture
    }

    fn write(&self, file: &str, source: &str) {
        let path = self.0.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }

    fn annotations() -> Self {
        let fixture = Self::new();
        for file in ["tsconfig.json", "consumer.ts", "model.ts", "types.ts"] {
            let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/types/native-annotations")
                .join(file);
            fixture.write(file, &std::fs::read_to_string(source).unwrap());
        }
        fixture
    }

    fn type_nodes() -> Self {
        let fixture = Self::new();
        for file in ["tsconfig.json", "consumer.ts", "types.ts"] {
            let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/types/native-type-nodes")
                .join(file);
            fixture.write(file, &std::fs::read_to_string(source).unwrap());
        }
        fixture
    }

    fn project(&self) -> Project {
        Project::load(&self.0.join("tsconfig.json")).unwrap()
    }

    fn query(&self, name: &str) -> TypeQuery {
        self.at("consumer.ts", name, name)
    }

    fn at(&self, file: &str, marker: &str, within: &str) -> TypeQuery {
        let source = std::fs::read_to_string(self.0.join(file)).unwrap();
        let start = source.find(marker).unwrap() + marker.find(within).unwrap();
        let before = &source[..start];
        let line = before.bytes().filter(|&byte| byte == b'\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap().encode_utf16().count() + 1;
        TypeQuery::At {
            file: self.0.join(file),
            position: SourcePosition { line, column },
        }
    }
}

#[test]
fn declaration_annotations_and_type_references_preserve_their_questions() {
    let fixture = Fixture::annotations();
    let project = fixture.project();
    for (file, marker, token, kind, primitive) in [
        (
            "consumer.ts",
            "annotated,",
            "annotated",
            TypeQuestion::Value,
            PrimitiveName::String,
        ),
        (
            "consumer.ts",
            "numberValue,",
            "numberValue",
            TypeQuestion::Value,
            PrimitiveName::Number,
        ),
        (
            "consumer.ts",
            "Renamed }",
            "Renamed",
            TypeQuestion::Declared,
            PrimitiveName::String,
        ),
        (
            "model.ts",
            "annotated: Label",
            "annotated",
            TypeQuestion::Value,
            PrimitiveName::String,
        ),
        (
            "model.ts",
            "annotated: Label",
            "Label",
            TypeQuestion::Type,
            PrimitiveName::String,
        ),
        (
            "model.ts",
            "ownPrimitive: string",
            "string",
            TypeQuestion::Type,
            PrimitiveName::String,
        ),
        (
            "types.ts",
            "Label = string",
            "Label",
            TypeQuestion::Declared,
            PrimitiveName::String,
        ),
        (
            "types.ts",
            "Renamed = Label",
            "Renamed",
            TypeQuestion::Declared,
            PrimitiveName::String,
        ),
    ] {
        let mut request = TypeRequest::new(fixture.at(file, marker, token));
        request.expand = true;
        let answer = project.type_of(request);
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(_))),
            "native declared annotation/type reference must be established for {file}:{token}: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.target.kind, kind);
        assert_eq!(result.outcome, TypeOutcome::Found);
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        assert_eq!(root.data, TypeNodeData::Primitive { name: primitive });
    }
    let TypeAnswer::Selected(named) = project
        .type_of(TypeRequest::new(TypeQuery::Name {
            name: "Label".into(),
            file: Some(fixture.0.join("types.ts")),
        }))
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!(named.target.kind, TypeQuestion::Declared);
    assert_eq!(named.description.summary, "string");
}

#[test]
fn declared_variable_type_demand_does_not_check_an_undemanded_initializer() {
    let fixture = Fixture::annotations();
    let project = fixture.project();
    for alias in ["mismatch", "uninitialized", "missingInitializer"] {
        let answer = project.type_of(TypeRequest::new(fixture.query(alias)));
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(_))),
            "declared variable type must follow the compiler demand without initializer validation: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.description.summary, "string");
        assert_eq!(result.target.kind, TypeQuestion::Value);
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
    }
}

#[test]
fn literal_initializer_token_selects_an_expression_type() {
    let fixture = Fixture::annotations();
    let project = fixture.project();
    for (marker, token, base, value, summary) in [
        (
            "ownPrimitive: string = \"ready\"",
            "\"ready\"",
            PrimitiveName::String,
            "ready",
            "\"ready\"",
        ),
        (
            "numberValue: Count = 1",
            "1",
            PrimitiveName::Number,
            "1",
            "1",
        ),
    ] {
        let mut request = TypeRequest::new(fixture.at("model.ts", marker, token));
        request.expand = true;
        let answer = project.type_of(request);
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(_))),
            "literal token must select its own expression question: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.target.kind, TypeQuestion::Expression);
        assert_eq!(result.description.summary, summary);
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        assert_eq!(
            root.data,
            TypeNodeData::Literal {
                base,
                value: value.into()
            }
        );
    }
}

#[test]
fn literal_type_declarations_and_tokens_preserve_regular_literal_values() {
    let fixture = Fixture::type_nodes();
    let project = fixture.project();
    for (file, marker, token, question, base, value, summary) in [
        (
            "types.ts",
            "type Word =",
            "Word",
            TypeQuestion::Declared,
            PrimitiveName::String,
            "ready",
            "\"ready\"",
        ),
        (
            "types.ts",
            "type Digit =",
            "Digit",
            TypeQuestion::Declared,
            PrimitiveName::Number,
            "7",
            "7",
        ),
        (
            "consumer.ts",
            "ImportedWord",
            "ImportedWord",
            TypeQuestion::Declared,
            PrimitiveName::String,
            "ready",
            "\"ready\"",
        ),
        (
            "consumer.ts",
            "ImportedDigit",
            "ImportedDigit",
            TypeQuestion::Declared,
            PrimitiveName::Number,
            "7",
            "7",
        ),
        (
            "consumer.ts",
            "importedLiteral",
            "importedLiteral",
            TypeQuestion::Value,
            PrimitiveName::String,
            "ready",
            "\"ready\"",
        ),
        (
            "consumer.ts",
            "importedNumber",
            "importedNumber",
            TypeQuestion::Value,
            PrimitiveName::Number,
            "7",
            "7",
        ),
        (
            "types.ts",
            "Word = \"ready\"",
            "\"ready\"",
            TypeQuestion::Type,
            PrimitiveName::String,
            "ready",
            "\"ready\"",
        ),
        (
            "types.ts",
            "Digit = 7",
            "7",
            TypeQuestion::Type,
            PrimitiveName::Number,
            "7",
            "7",
        ),
    ] {
        let mut request = TypeRequest::new(fixture.at(file, marker, token));
        request.expand = true;
        let answer = project.type_of(request);
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(_))),
            "declared literal type must be established for {file}:{token}: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.target.kind, question);
        assert_eq!(result.description.summary, summary);
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        assert_eq!(
            root.data,
            TypeNodeData::Literal {
                base,
                value: value.into()
            }
        );
        assert!(root.alias.is_none());
    }
}

#[test]
fn type_literals_preserve_required_readonly_members_and_alias_provenance() {
    let fixture = Fixture::type_nodes();
    let project = fixture.project();
    for (marker, question, summary, alias, line) in [
        (
            "importedInline",
            TypeQuestion::Value,
            "{ readonly title: string; count: number; }",
            false,
            5,
        ),
        (
            "ImportedShape",
            TypeQuestion::Declared,
            "RecordShape",
            true,
            3,
        ),
        (
            "importedRecord",
            TypeQuestion::Value,
            "RecordShape",
            true,
            3,
        ),
    ] {
        let mut request = TypeRequest::new(fixture.at("consumer.ts", marker, marker));
        request.expand = true;
        let answer = project.type_of(request);
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(_))),
            "plain type literal members must be established: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.target.kind, question);
        assert_eq!(result.description.summary, summary);
        assert_eq!(result.outcome, TypeOutcome::Found);
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        if alias {
            let alias = root
                .alias
                .as_ref()
                .expect("native object alias remains visible");
            assert_eq!(alias.name, "RecordShape");
            assert_eq!(alias.origins, vec![location("types.ts", 3, 13, 24)]);
        } else {
            assert!(root.alias.is_none());
        }
        let TypeNodeData::Object { members } = &root.data else {
            panic!("type literal has object shape")
        };
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(properties.len(), 2);
        for (property, name, primitive, readonly) in [
            (&properties[0], "title", PrimitiveName::String, true),
            (&properties[1], "count", PrimitiveName::Number, false),
        ] {
            assert_eq!(property.key, TypeKey::String { name: name.into() });
            assert_eq!(property.readonly, readonly);
            assert!(!property.optional);
            assert_eq!(property.origins.len(), 1);
            assert_eq!(property.origins[0].file, "types.ts");
            assert_eq!(property.origins[0].start.line, line);
            let TypeRef::Ref { r#ref } = &property.read_type else {
                panic!("required primitive member is displayed")
            };
            assert_eq!(
                graph
                    .nodes
                    .iter()
                    .find(|node| node.id == *r#ref)
                    .unwrap()
                    .data,
                TypeNodeData::Primitive { name: primitive }
            );
        }
        assert_eq!(members.call_signatures, Some(vec![]));
        assert_eq!(members.construct_signatures, Some(vec![]));
        assert_eq!(members.index_signatures, Some(vec![]));
    }
}

#[test]
fn interface_declarations_and_aliases_keep_nominal_display_without_type_alias_metadata() {
    let fixture = Fixture::type_nodes();
    let project = fixture.project();
    for (file, marker, token, question) in [
        ("types.ts", "interface Face", "Face", TypeQuestion::Declared),
        (
            "consumer.ts",
            "ImportedFace",
            "ImportedFace",
            TypeQuestion::Declared,
        ),
        (
            "consumer.ts",
            "importedObject",
            "importedObject",
            TypeQuestion::Value,
        ),
    ] {
        let mut request = TypeRequest::new(fixture.at(file, marker, token));
        request.expand = true;
        let answer = project.type_of(request);
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(_))),
            "plain interface type must be established: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.target.kind, question);
        assert_eq!(result.description.summary, "Face");
        assert_eq!(result.outcome, TypeOutcome::Found);
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        assert!(
            root.alias.is_none(),
            "interface display symbol is not a type alias"
        );
        let TypeNodeData::Object { members } = &root.data else {
            panic!("interface has object shape")
        };
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(properties.len(), 2);
        assert_eq!(
            properties[0].key,
            TypeKey::String {
                name: "title".into()
            }
        );
        assert!(properties[0].readonly);
        assert_eq!(properties[0].origins, vec![location("types.ts", 4, 34, 39)]);
        assert_eq!(
            properties[1].key,
            TypeKey::String {
                name: "count".into()
            }
        );
        assert_eq!(properties[1].origins, vec![location("types.ts", 4, 49, 54)]);
    }
}

#[test]
fn property_names_have_value_questions_and_distinct_canonical_source_candidates() {
    let fixture = Fixture::type_nodes();
    let project = fixture.project();
    for (marker, token, summary) in [
        ("Face { readonly title", "title", "string"),
        ("OtherFace { title", "title", "number"),
        ("RecordShape = { readonly title", "title", "string"),
        ("EscapedFace { \\u0074itle", "\\u0074itle", "string"),
    ] {
        let answer = project.type_of(TypeRequest::new(fixture.at("types.ts", marker, token)));
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(_))),
            "own property name must address its value type: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.target.kind, TypeQuestion::Value);
        assert_eq!(result.description.summary, summary);
        assert_eq!(result.outcome, TypeOutcome::Found);
    }
    for (selector, summary) in [
        ("Face.title", "string"),
        ("OtherFace.title", "number"),
        ("EscapedFace.title", "string"),
    ] {
        let answer = project.type_of(TypeRequest::new(TypeQuery::Name {
            name: selector.into(),
            file: Some(fixture.0.join("types.ts")),
        }));
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(_))),
            "interface property selector must select its own value question: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.target.kind, TypeQuestion::Value);
        assert_eq!(result.description.summary, summary);
    }
    let answer = project.type_of(TypeRequest::new(TypeQuery::Name {
        name: "title".into(),
        file: Some(fixture.0.join("types.ts")),
    }));
    assert!(
        matches!(answer, Ok(TypeAnswer::Unselected(_))),
        "same key in unrelated interfaces must remain ambiguous: {answer:?}"
    );
    let TypeAnswer::Unselected(result) = answer.unwrap() else {
        unreachable!()
    };
    let SelectionFailure::Ambiguous { candidates } = result.outcome else {
        panic!("unrelated member roots must not collapse")
    };
    assert_eq!(candidates.len(), 3);
    for (candidate, selector, origin, summary) in [
        (
            &candidates[0],
            "Face.title",
            location("types.ts", 4, 34, 39),
            "string",
        ),
        (
            &candidates[1],
            "OtherFace.title",
            location("types.ts", 10, 30, 35),
            "number",
        ),
        (
            &candidates[2],
            "EscapedFace.title",
            location("types.ts", 11, 32, 42),
            "string",
        ),
    ] {
        assert_eq!(candidate.selector.as_deref(), Some(selector));
        assert_eq!(candidate.question, TypeQuestion::Value);
        assert_eq!(candidate.declarations.len(), 1);
        assert_eq!(candidate.declarations[0].name, "title");
        assert_eq!(candidate.declarations[0].location, origin);
        assert_eq!(candidate.anchor.file, "types.ts");
        assert_eq!(candidate.anchor.line, origin.start.line);
        assert_eq!(candidate.anchor.column, origin.start.column);
        let TypeAnswer::Selected(replayed) = project
            .type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.0.join(&candidate.anchor.file),
                position: SourcePosition {
                    line: candidate.anchor.line,
                    column: candidate.anchor.column,
                },
            }))
            .unwrap()
        else {
            panic!("candidate anchor must replay its actual property value question")
        };
        assert_eq!(replayed.target.kind, candidate.question);
        assert_eq!(replayed.description.summary, summary);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn location(file: &str, line: usize, column: usize, end: usize) -> Location {
    Location {
        file: file.into(),
        start: SourcePosition { line, column },
        end: SourcePosition { line, column: end },
    }
}

#[test]
fn imported_aliases_infer_object_members_across_renamed_barrels() {
    let fixture = Fixture::new();
    let project = fixture.project();
    for alias in ["direct", "renamed", "typeOnly"] {
        let mut request = TypeRequest::new(fixture.query(alias));
        request.expand = true;
        let answer = project.type_of(request);
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(_))),
            "the imported variable must have an established inferred object type: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.target.kind, TypeQuestion::Value);
        assert_eq!(result.target.location.file, "consumer.ts");
        assert_eq!(
            result.description.summary,
            "{ label: string; count: number; }"
        );
        assert!(!result.description.summary_truncated);
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        assert!(!graph.truncated);
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        let TypeNodeData::Object { members } = &root.data else {
            panic!("an inferred object remains an object")
        };
        assert_eq!(members.call_signatures, Some(vec![]));
        assert_eq!(members.construct_signatures, Some(vec![]));
        assert_eq!(members.index_signatures, Some(vec![]));
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(properties.len(), 2);
        for (property, name, primitive, origin) in [
            (
                &properties[0],
                "label",
                PrimitiveName::String,
                location("model.ts", 1, 26, 31),
            ),
            (
                &properties[1],
                "count",
                PrimitiveName::Number,
                location("model.ts", 1, 42, 47),
            ),
        ] {
            assert_eq!(property.key, TypeKey::String { name: name.into() });
            assert!(!property.optional);
            assert!(!property.readonly);
            assert_eq!(property.origins, vec![origin]);
            let TypeRef::Ref { r#ref } = &property.read_type else {
                panic!("admitted member is displayed")
            };
            assert!(
                matches!(graph.nodes.iter().find(|node| node.id == *r#ref).unwrap().data,
                TypeNodeData::Primitive { name } if name == primitive)
            );
        }
    }
}

#[test]
fn imported_primitive_variables_preserve_const_and_mutable_widening() {
    let fixture = Fixture::new();
    let project = fixture.project();
    for (alias, summary, literal) in [("text", "\"ready\"", true), ("mutable", "string", false)] {
        let mut request = TypeRequest::new(fixture.query(alias));
        request.expand = true;
        let answer = project.type_of(request);
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(_))),
            "native primitive variable type must be established: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.description.summary, summary);
        assert_eq!(result.target.kind, TypeQuestion::Value);
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        if literal {
            assert_eq!(
                root.data,
                TypeNodeData::Literal {
                    base: PrimitiveName::String,
                    value: "ready".into()
                }
            );
        } else {
            assert_eq!(
                root.data,
                TypeNodeData::Primitive {
                    name: PrimitiveName::String
                }
            );
        }
    }
}

#[test]
fn alias_name_selection_and_external_member_origins_use_captured_sources() {
    let fixture = Fixture::new();
    let captured_query = fixture.query("remote");
    let project = fixture.project();
    // The caller changes both the selected input and its external dependency.
    // The immutable generation still answers the original captured program.
    fixture.write("consumer.ts", "export const remote = false;");
    fixture.write(
        "node_modules/native-fixture/index.ts",
        "export const remote = { replaced: false };",
    );
    // Unrenamed imports are deliberately absent from Q2's keyed selector
    // catalog. Their exact token remains a Q4 position target.
    let mut request = TypeRequest::new(captured_query);
    request.expand = true;
    let answer = project.type_of(request);
    assert!(
        matches!(answer, Ok(TypeAnswer::Selected(_))),
        "captured imported object must remain selectable: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(
        result.description.summary,
        "{ label: string; count: number; }"
    );
    assert_eq!(result.target.location.file, "consumer.ts");
    let graph = result.description.expansion.unwrap();
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    let TypeNodeData::Object { members } = &root.data else {
        unreachable!()
    };
    let properties = members.properties.as_ref().unwrap();
    assert_eq!(
        properties[0].origins,
        vec![location("node_modules/native-fixture/index.ts", 1, 25, 30)]
    );
    assert_eq!(
        properties[1].origins,
        vec![location("node_modules/native-fixture/index.ts", 1, 44, 49)]
    );
    assert!(
        root.alias.is_none(),
        "variable/import labels are not semantic type aliases"
    );
    let TypeAnswer::Selected(named) = project
        .type_of(TypeRequest::new(TypeQuery::Name {
            name: "renamed".into(),
            file: None,
        }))
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        named.description.summary,
        "{ label: string; count: number; }"
    );
}
