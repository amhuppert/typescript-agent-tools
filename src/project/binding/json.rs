//! Owned JSON source facts, captured once from the existing lossless reader.
//!
//! Load's narrow object/apparent-interface lookup and future imported widened
//! types share these facts. Type evaluation, origins grouped across symbols,
//! and source-position selection remain distinct operations with other owners.

use crate::Span;
use crate::parser::json::{JsonKind, JsonString, JsonValue};
use std::collections::{HashMap, VecDeque};

/// One value in one JSON file. This private handle never crosses projects or
/// appears in output; its arena uses native indices without a narrowing cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct JsonValueId(usize);

/// An actual JSON AST declaration in the retained lossless reader. These
/// handles preserve source identity independently of key spelling or spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) enum JsonDeclaration {
    SourceFile,
    Value(JsonValueId),
    Property(JsonPropertyId),
}

/// Identity of one actual key occurrence in the existing JSON inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct JsonPropertyId {
    object: JsonValueId,
    occurrence: usize,
}

pub(in crate::project) struct JsonReferenceKey<'a> {
    pub(in crate::project) id: JsonPropertyId,
    pub(in crate::project) object: JsonValueId,
    pub(in crate::project) key: &'a JsonString,
    pub(in crate::project) span: Span,
}

/// Every JSON value, recursive array element and key occurrence from one file.
/// TypeScript gives the module one `export=` property whose declaration is the
/// file (`bindSourceFileIfExternalModule`). Its imported type widens the root
/// (`getTypeOfVariableOrParameterOrPropertyWorker`); these facts retain scalar
/// kinds, not their unnecessary literal payloads. They do not evaluate types.
#[derive(Debug)]
pub(in crate::project) struct JsonFacts {
    values: Vec<JsonValueFacts>,
}

#[derive(Debug)]
struct JsonValueFacts {
    native_span: Option<Span>,
    span: Option<Span>,
    /// Attachment to the existing object's occurrence, not a second name or
    /// declaration inventory. Array elements and the root have no own key.
    property: Option<JsonPropertyParent>,
    shape: JsonShape,
}

#[derive(Debug, Clone, Copy)]
struct JsonPropertyParent {
    object: JsonValueId,
    occurrence: usize,
}

/// A complete borrowed description of one captured value. Scalar spelling
/// stays absent because the imported-module question uses widened types.
pub(in crate::project) struct JsonValueView<'a> {
    pub(in crate::project) span: Option<Span>,
    pub(in crate::project) shape: &'a JsonShape,
}

/// One source initializer, including an overwritten duplicate. Checking an
/// object must demand these in source order before selecting final properties.
pub(in crate::project) struct JsonInitializerView<'a> {
    pub(in crate::project) id: JsonPropertyId,
    pub(in crate::project) key: &'a JsonString,
    pub(in crate::project) span: Span,
    pub(in crate::project) optional: bool,
    pub(in crate::project) value: JsonValueId,
}

/// Kinds and recursive structure; scalar spelling remains in captured source.
#[derive(Debug)]
pub(in crate::project) enum JsonShape {
    /// Empty or trivia-only JSON text, with no root value or root span.
    Empty,
    Null,
    String,
    Number,
    Boolean,
    Array(Vec<JsonValueId>),
    Object(JsonObjectFacts),
}

#[derive(Debug)]
pub(in crate::project) struct JsonObjectFacts {
    /// Every exact key occurrence in source order, including duplicates.
    keys: Vec<JsonPropertyFacts>,
    /// Derived first-occurrence indices, preserving the existing narrow lookup
    /// bound of O(distinct keys), even for files with many duplicate keys. No
    /// second names or semantic decisions are retained in this accelerator.
    unique: Vec<usize>,
}

#[derive(Debug)]
struct JsonPropertyFacts {
    native_span: Span,
    key: JsonString,
    span: Span,
    optional: bool,
    value: JsonValueId,
    /// The final occurrence's value for this exact cooked key, shared by all
    /// duplicate occurrences. No caller has to reconstruct duplicate semantics.
    last_value: JsonValueId,
}

/// Global apparent interfaces whose captured declarations linking consults.
pub(super) const APPARENT_INTERFACES: [&str; 4] = ["String", "Number", "Boolean", "Array"];

impl JsonShape {
    /// The global interface of a primitive or array (`getApparentType`), without
    /// Object/Function augmentation (`skipObjectFunctionPropertyAugment`). This
    /// is the existing load lookup, not contextual or receiver-type inference.
    pub(super) fn interface(&self) -> Option<&'static str> {
        match self {
            Self::String => Some("String"),
            Self::Number => Some("Number"),
            Self::Boolean => Some("Boolean"),
            Self::Array(_) => Some("Array"),
            Self::Empty | Self::Null | Self::Object(_) => None,
        }
    }
}

impl JsonFacts {
    pub(super) fn object_values(&self) -> impl Iterator<Item = JsonValueId> + '_ {
        self.values.iter().enumerate().filter_map(|(index, value)| {
            matches!(value.shape, JsonShape::Object(_)).then_some(JsonValueId(index))
        })
    }

    pub(in crate::project) fn declaration_facts(
        &self,
        declaration: JsonDeclaration,
        source_span: Span,
    ) -> (crate::syntax_kind::SyntaxKind, Span) {
        use crate::syntax_kind::SyntaxKind as K;
        match declaration {
            JsonDeclaration::SourceFile => (K::SourceFile, source_span),
            JsonDeclaration::Value(value) => (
                K::ObjectLiteralExpression,
                self.values[value.0]
                    .native_span
                    .expect("bound JSON object has an actual AST node"),
            ),
            JsonDeclaration::Property(property) => {
                let JsonShape::Object(object) = &self.values[property.object.0].shape else {
                    unreachable!("JSON property has an object parent")
                };
                (
                    K::PropertyAssignment,
                    object.keys[property.occurrence].native_span,
                )
            }
        }
    }

    pub(in crate::project) fn property_initializer(&self, property: JsonPropertyId) -> JsonValueId {
        let JsonShape::Object(object) = &self.values[property.object.0].shape else {
            unreachable!("JSON property has an object parent")
        };
        object.keys[property.occurrence].value
    }

    /// `shouldCheckAsExcessProperty` compares an actual PropertyAssignment's
    /// Parent pointer with the source object's ValueDeclaration. A duplicate
    /// keeps its original attachment; final-value links do not participate.
    pub(in crate::project) fn declaration_has_parent(
        &self,
        declaration: JsonDeclaration,
        parent: JsonDeclaration,
    ) -> bool {
        match (declaration, parent) {
            (JsonDeclaration::Property(property), JsonDeclaration::Value(value)) => {
                property.object == value
            }
            // JSON object nodes may be initializers of original properties.
            // Root and array-element objects have an ExpressionStatement or
            // ArrayLiteral parent, neither of which is a symbol declaration.
            (JsonDeclaration::Value(value), JsonDeclaration::Property(property)) => {
                self.values[value.0].property.is_some_and(|attachment| {
                    attachment.object == property.object
                        && attachment.occurrence == property.occurrence
                })
            }
            _ => false,
        }
    }

    pub(in crate::project) fn reference_keys(&self) -> impl Iterator<Item = JsonReferenceKey<'_>> {
        self.values.iter().enumerate().flat_map(|(index, value)| {
            let keys = match &value.shape {
                JsonShape::Object(object) => object.keys.as_slice(),
                _ => &[],
            };
            keys.iter()
                .enumerate()
                .map(move |(occurrence, property)| JsonReferenceKey {
                    id: JsonPropertyId {
                        object: JsonValueId(index),
                        occurrence,
                    },
                    object: JsonValueId(index),
                    key: &property.key,
                    span: property.span,
                })
        })
    }

    pub(in crate::project) fn reference_key(&self, id: JsonPropertyId) -> JsonReferenceKey<'_> {
        let JsonShape::Object(object) = &self.values[id.object.0].shape else {
            unreachable!("JSON key belongs to an object")
        };
        let property = &object.keys[id.occurrence];
        JsonReferenceKey {
            id,
            object: id.object,
            key: &property.key,
            span: property.span,
        }
    }

    pub(in crate::project) fn reference_key_at(&self, offset: usize) -> Option<JsonPropertyId> {
        self.reference_keys()
            .find(|key| key.span.start <= offset && offset < key.span.end)
            .map(|key| key.id)
    }

    /// The module's root value, its `export=`. Trivia-only text has Empty here.
    pub(in crate::project) const ROOT: JsonValueId = JsonValueId(0);

    pub(super) fn new(root: Option<&JsonValue>) -> Self {
        let mut facts = Self { values: Vec::new() };
        // Breadth-first capture avoids another recursive traversal. IDs are
        // the queue's final positions, including every discarded duplicate's
        // own child and every element of each array.
        let mut pending = VecDeque::from([(root, None)]);
        while let Some((value, property)) = pending.pop_front() {
            let object = JsonValueId(facts.values.len());
            let next = facts.values.len() + 1 + pending.len();
            let shape = match value.map(|value| &value.kind) {
                None => JsonShape::Empty,
                Some(JsonKind::Null) => JsonShape::Null,
                Some(JsonKind::Bool(_)) => JsonShape::Boolean,
                Some(JsonKind::Number) => JsonShape::Number,
                Some(JsonKind::String(_)) => JsonShape::String,
                Some(JsonKind::Array(elements)) => {
                    pending.extend(elements.iter().map(|value| (Some(value), None)));
                    JsonShape::Array(
                        (0..elements.len())
                            .map(|index| JsonValueId(next + index))
                            .collect(),
                    )
                }
                Some(JsonKind::Object(properties)) => {
                    // Object checking overwrites each exact key's value in
                    // source order (`checkObjectLiteral`); first-occurrence
                    // order still controls the object's property ordering.
                    let mut last: HashMap<&JsonString, usize> = HashMap::new();
                    let mut unique = Vec::new();
                    for (index, property) in properties.iter().enumerate() {
                        if last.insert(&property.key, index).is_none() {
                            unique.push(index);
                        }
                    }
                    let keys = properties
                        .iter()
                        .enumerate()
                        .map(|(index, property)| JsonPropertyFacts {
                            native_span: property.native_span,
                            key: property.key.clone(),
                            span: property.key_span,
                            optional: property.question.is_some(),
                            value: JsonValueId(next + index),
                            last_value: JsonValueId(next + last[&property.key]),
                        })
                        .collect();
                    pending.extend(properties.iter().enumerate().map(|(occurrence, property)| {
                        (
                            Some(&property.value),
                            Some(JsonPropertyParent { object, occurrence }),
                        )
                    }));
                    JsonShape::Object(JsonObjectFacts { keys, unique })
                }
            };
            facts.values.push(JsonValueFacts {
                native_span: value.and_then(|value| value.native_span),
                span: value.map(|value| value.span),
                property,
                shape,
            });
        }
        facts
    }

    pub(super) fn shape(&self, value: JsonValueId) -> &JsonShape {
        &self.values[value.0].shape
    }

    pub(in crate::project) fn describe(&self, value: JsonValueId) -> JsonValueView<'_> {
        let value = &self.values[value.0];
        JsonValueView {
            span: value.span,
            shape: &value.shape,
        }
    }

    /// Own object property selected by exact name (`getPropertyOfType`); its
    /// imported value follows the last exact-key occurrence. No absent,
    /// non-object or unrepresentable UTF-16 key becomes another property.
    pub(super) fn property(&self, value: JsonValueId, name: &str) -> Option<JsonValueId> {
        match self.shape(value) {
            JsonShape::Object(object) => object
                .unique
                .iter()
                .map(|&index| &object.keys[index])
                .find(|property| property.key.as_str() == Some(name))
                .map(|property| property.last_value),
            _ => None,
        }
    }
}

impl JsonObjectFacts {
    pub(in crate::project) fn initializers(
        &self,
        object: JsonValueId,
    ) -> impl Iterator<Item = JsonInitializerView<'_>> {
        self.keys
            .iter()
            .enumerate()
            .map(move |(occurrence, property)| JsonInitializerView {
                id: JsonPropertyId { object, occurrence },
                key: &property.key,
                span: property.span,
                optional: property.optional,
                value: property.value,
            })
    }

    /// `checkObjectLiteral`'s final property table retains first-key order
    /// while each entry points to the last checked initializer.
    pub(in crate::project) fn final_values(&self) -> impl Iterator<Item = JsonValueId> + '_ {
        self.unique.iter().map(|&index| self.keys[index].last_value)
    }
}

#[cfg(test)]
mod tests {
    use super::{JsonDeclaration, JsonFacts, JsonPropertyId, JsonShape, JsonValueId};
    use crate::parser::json::JsonString;
    use crate::project::Project;
    use crate::project::binding::{ReadOrigin, ReferenceSymbol};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(source: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "tsparse-json-facts-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).unwrap();
            for (file, source) in [
                (
                    "tsconfig.json",
                    r#"{"compilerOptions":{"noLib":true,"resolveJsonModule":true},"files":["main.ts"]}"#,
                ),
                ("main.ts", "import data from './data.json';\nexport {};"),
                ("data.json", source),
            ] {
                std::fs::write(root.join(file), source).unwrap();
            }
            Self(root)
        }

        fn load(&self) -> Project {
            Project::load(&self.0.join("tsconfig.json")).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn retained(project: &Project) -> (&JsonFacts, &str) {
        let file = project
            .sources
            .ids()
            .find(|&file| project.sources.get(file).display == "data.json")
            .unwrap();
        (
            project.bindings.files[file.index()].json.as_ref().unwrap(),
            project.sources.get(file).text.as_str(),
        )
    }

    /// The JSON declarations of the binder symbol of root key occurrence
    /// `index`, through the same native inputs the checker reads.
    fn root_key_declarations(project: &Project, index: usize) -> Vec<Option<JsonDeclaration>> {
        let file = project
            .sources
            .ids()
            .find(|&file| project.sources.get(file).display == "data.json")
            .unwrap();
        let inputs = project.bindings.native_inputs();
        let symbol = inputs
            .json_node_symbol(file, JsonDeclaration::Property(root_key(index)))
            .unwrap();
        inputs
            .reference_symbol_declarations(ReferenceSymbol::Known(ReadOrigin::Direct(symbol)))
            .unwrap()
            .into_iter()
            .map(|declaration| declaration.json)
            .collect()
    }

    fn root_key(occurrence: usize) -> JsonPropertyId {
        JsonPropertyId {
            object: JsonFacts::ROOT,
            occurrence,
        }
    }

    /// Each occurrence in `group` shares one symbol whose declarations are
    /// exactly the group's occurrences, in source order.
    fn assert_key_group(project: &Project, group: &[usize]) {
        let expected: Vec<_> = group
            .iter()
            .map(|&index| Some(JsonDeclaration::Property(root_key(index))))
            .collect();
        for &index in group {
            assert_eq!(root_key_declarations(project, index), expected, "{group:?}");
        }
    }

    fn object(facts: &JsonFacts, value: JsonValueId) -> &super::JsonObjectFacts {
        match facts.shape(value) {
            JsonShape::Object(object) => object,
            shape => panic!("expected object, got {shape:?}"),
        }
    }

    // Mechanical private-representation verification at the final Project
    // boundary. Public compatibility is pinned separately; no new inspection
    // operation exists merely to make retained facts integration-testable.
    #[test]
    fn final_project_retains_every_json_child_origin_and_last_value() {
        for separator in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
            let source = [
                "\u{feff}{",
                r#""dup": {"first": 1}, "dup": {"last": true},"#,
                r#""rows": [1, "literal", false, null, [], [{"inner": -2}]],"#,
                r#""optional"?: {"leaf": 3}"#,
                "}",
            ]
            .join(separator);
            let fixture = Fixture::new(&source);
            let project = fixture.load();
            let (facts, captured) = retained(&project);
            assert_eq!(captured, source.strip_prefix('\u{feff}').unwrap());
            let root = object(facts, JsonFacts::ROOT);
            assert_eq!(root.keys.len(), 4);
            assert_eq!(root.unique, [0, 2, 3]);
            assert_ne!(root.keys[0].value, root.keys[1].value);
            assert_eq!(root.keys[0].last_value, root.keys[1].value);
            assert_eq!(root.keys[1].last_value, root.keys[1].value);
            assert_eq!(
                facts.property(JsonFacts::ROOT, "dup"),
                Some(root.keys[1].value)
            );
            assert!(facts.property(root.keys[0].value, "first").is_some());
            assert!(facts.property(root.keys[0].value, "last").is_none());
            assert!(facts.property(root.keys[1].value, "first").is_none());
            assert!(facts.property(root.keys[1].value, "last").is_some());
            for key in &root.keys {
                assert_eq!(
                    &captured[key.span.start..key.span.end],
                    format!("\"{}\"", key.key.as_str().unwrap())
                );
            }
            for group in [&[0, 1][..], &[2][..], &[3][..]] {
                assert_key_group(&project, group);
            }
            assert!(!root.keys[0].optional);
            assert!(root.keys[3].optional);
            let rows = facts.property(JsonFacts::ROOT, "rows").unwrap();
            let JsonShape::Array(elements) = facts.shape(rows) else {
                panic!("rows is an array")
            };
            assert_eq!(elements.len(), 6);
            assert!(matches!(facts.shape(elements[0]), JsonShape::Number));
            assert!(matches!(facts.shape(elements[1]), JsonShape::String));
            assert!(matches!(facts.shape(elements[2]), JsonShape::Boolean));
            assert!(matches!(facts.shape(elements[3]), JsonShape::Null));
            assert!(
                matches!(facts.shape(elements[4]), JsonShape::Array(children) if children.is_empty())
            );
            let JsonShape::Array(nested) = facts.shape(elements[5]) else {
                panic!("nested array")
            };
            let inner = facts.property(nested[0], "inner").unwrap();
            let span = facts.values[inner.0].span.unwrap();
            assert_eq!(&captured[span.start..span.end], "-2");
            let string_span = facts.values[elements[1].0].span.unwrap();
            assert_eq!(&captured[string_span.start..string_span.end], "\"literal\"");
            let root_span = facts.values[JsonFacts::ROOT.0].span.unwrap();
            assert_eq!(&captured[root_span.start..root_span.end], captured);
            assert_eq!(facts.values.len(), 16);
        }
    }

    #[test]
    fn final_project_preserves_raw_keys_and_exact_utf16_duplicate_identity() {
        let source = r#"{"a":1,"\u0061":false,"":null,"01":1,"1":2,"😀":true,"\ud800":1,"\ud800":false,"\udfff":3,"\ud83d\ude00":null}"#;
        let fixture = Fixture::new(source);
        let project = fixture.load();
        let (facts, captured) = retained(&project);
        let root = object(facts, JsonFacts::ROOT);
        assert_eq!(root.keys.len(), 10);
        assert_eq!(root.unique, [0, 2, 3, 4, 5, 6, 8]);
        assert_eq!(root.keys[0].key, root.keys[1].key);
        assert_eq!(root.keys[5].key, root.keys[9].key);
        assert_eq!(
            root.keys[6].key,
            JsonString::NonScalar(vec![0xed, 0xa0, 0x80])
        );
        assert_eq!(root.keys[6].key, root.keys[7].key);
        assert_eq!(
            root.keys[8].key,
            JsonString::NonScalar(vec![0xed, 0xbf, 0xbf])
        );
        assert_ne!(root.keys[6].key, root.keys[8].key);
        for (key, spelling) in root.keys.iter().zip([
            "\"a\"",
            r#""\u0061""#,
            "\"\"",
            "\"01\"",
            "\"1\"",
            "\"😀\"",
            r#""\ud800""#,
            r#""\ud800""#,
            r#""\udfff""#,
            r#""\ud83d\ude00""#,
        ]) {
            assert_eq!(&captured[key.span.start..key.span.end], spelling);
        }
        assert_eq!(
            facts.property(JsonFacts::ROOT, "a"),
            Some(root.keys[1].value)
        );
        assert_eq!(
            facts.property(JsonFacts::ROOT, "😀"),
            Some(root.keys[9].value)
        );
        assert_eq!(
            facts.property(JsonFacts::ROOT, ""),
            Some(root.keys[2].value)
        );
        assert_eq!(
            facts.property(JsonFacts::ROOT, "01"),
            Some(root.keys[3].value)
        );
        assert_eq!(
            facts.property(JsonFacts::ROOT, "1"),
            Some(root.keys[4].value)
        );
        assert!(facts.property(JsonFacts::ROOT, "\u{fffd}").is_none());
        assert_eq!(root.keys[6].last_value, root.keys[7].value);
        assert_eq!(root.keys[8].last_value, root.keys[8].value);
        for group in [
            &[0, 1][..],
            &[2][..],
            &[3][..],
            &[4][..],
            &[5, 9][..],
            &[6, 7][..],
            &[8][..],
        ] {
            assert_key_group(&project, group);
        }
        assert_eq!(facts.values.len(), 11);
    }

    #[test]
    fn final_project_retains_root_kinds_and_flat_duplicate_children() {
        for (source, count) in [
            ("", 1),
            ("// trivia", 1),
            ("null", 1),
            ("true", 1),
            ("42", 1),
            ("\"s\"", 1),
            ("[]", 1),
            ("[false,null,[],{}]", 5),
            ("{}", 1),
        ] {
            let fixture = Fixture::new(source);
            let project = fixture.load();
            let (facts, _) = retained(&project);
            assert_eq!(facts.values.len(), count, "{source}");
            assert_eq!(
                facts.values[0].span.is_none(),
                source.is_empty() || source.starts_with("//")
            );
            let root = facts.shape(JsonFacts::ROOT);
            assert!(
                match source {
                    "" | "// trivia" => matches!(root, JsonShape::Empty),
                    "null" => matches!(root, JsonShape::Null),
                    "true" => matches!(root, JsonShape::Boolean),
                    "42" => matches!(root, JsonShape::Number),
                    "\"s\"" => matches!(root, JsonShape::String),
                    "[]" | "[false,null,[],{}]" => matches!(root, JsonShape::Array(_)),
                    "{}" => matches!(root, JsonShape::Object(_)),
                    _ => unreachable!("fixture root is listed"),
                },
                "{source}"
            );
            assert_eq!(
                facts.shape(JsonFacts::ROOT).interface(),
                match source {
                    "true" => Some("Boolean"),
                    "42" => Some("Number"),
                    "\"s\"" => Some("String"),
                    "[]" | "[false,null,[],{}]" => Some("Array"),
                    _ => None,
                }
            );
        }
        let source = format!(
            "{{{}}}",
            std::iter::repeat_n(r#""dup":[1]"#, 2048)
                .collect::<Vec<_>>()
                .join(",")
        );
        let fixture = Fixture::new(&source);
        let project = fixture.load();
        let (facts, _) = retained(&project);
        let root = object(facts, JsonFacts::ROOT);
        assert_eq!(root.keys.len(), 2048);
        assert_eq!(root.unique, [0]);
        assert_eq!(facts.values.len(), 4097);
        assert_eq!(
            facts.property(JsonFacts::ROOT, "dup"),
            Some(root.keys[2047].value)
        );
        for key in &root.keys {
            assert_eq!(key.last_value, root.keys[2047].value);
            assert!(
                matches!(facts.shape(key.value), JsonShape::Array(elements) if elements.len() == 1)
            );
        }
    }
}
