//! The immutable inputs and reference projection retained by one Project.
//!
//! Native queries consume the captured binder headers and file facts. Lexical
//! queries consume the completed Index. The mutable lexical Store and its
//! resolution work belong only to loading: after final checks they leave
//! with the collection recipes as `LoadOnly`, which the loader frees.

use super::collect::{RefBase, SharedFacts};
use super::ids::OccurrenceId;
use super::index::Index;
use super::{JsonFacts, ModuleRequest, ModuleResolution};
use crate::project::program::{Diagnostics, ProgramFile};
use std::sync::Arc;

/// Published only after lexical checks and index construction complete.
/// Raw native identities remain independent of the Index's lexical groups.
#[derive(Debug)]
pub(in crate::project) struct Bindings {
    pub(in crate::project) index: Index,
    pub(super) raw: super::raw::RawBindingSnapshot,
    pub(super) files: Vec<File>,
}

/// The successful link result before Project transfers its owned file facts.
/// Only reference attachments survive projection; no NameId mapping or second
/// searched token array remains in the published generation.
#[derive(Debug)]
pub(in crate::project) struct Linked {
    pub(super) index: Index,
    pub(super) reference_sources: Vec<Vec<OccurrenceId>>,
    pub(super) raw: super::raw::RawBindingSnapshot,
    /// The linker's working state, which publication leaves behind.
    pub(super) link_state: super::link::LinkState,
}

/// What only loading used, left behind when the files are retained: the
/// linker's working state and each file's collection recipes. Freeing it
/// takes a noticeable part of a load, so its owner decides when and where
/// (`Project::from_program`).
pub(in crate::project) struct LoadOnly {
    _link_state: super::link::LinkState,
    _recipes: Vec<Arc<super::collect::Facts>>,
}

impl Bindings {
    #[cfg(test)]
    pub(in crate::project) fn native_inputs(&self) -> super::NativeInputs<'_> {
        super::NativeInputs::new(super::inputs::BindingFiles::Retained(self), &self.raw)
    }

    /// Immutable binder inputs captured before initializeChecker.
    pub(in crate::project) fn raw_binding_snapshot(&self) -> &super::raw::RawBindingSnapshot {
        &self.raw
    }
}

impl Linked {
    pub(in crate::project) fn retain_files(self, files: Vec<ProgramFile>) -> (Bindings, LoadOnly) {
        assert_eq!(files.len(), self.reference_sources.len());
        let mut recipes = Vec::with_capacity(files.len());
        let files = files
            .into_iter()
            .zip(self.reference_sources)
            .map(|(file, sources)| {
                let (file, facts) = File::retain(file, sources);
                recipes.push(facts);
                file
            })
            .collect();
        let bindings = Bindings {
            index: self.index,
            raw: self.raw,
            files,
        };
        let load_only = LoadOnly {
            _link_state: self.link_state,
            _recipes: recipes,
        };
        (bindings, load_only)
    }
}

/// Published file facts never grow. Boxed slices discard capture capacity
/// without changing the file-local indices used by native identities.
#[derive(Debug)]
pub(super) struct File {
    pub(super) module_semantics: crate::project::program::FileModuleSemantics,
    shared: Arc<SharedFacts>,
    pub(super) search: bool,
    pub(super) module: bool,
    pub(super) declaration: bool,
    pub(super) requests: Box<[ModuleRequest]>,
    pub(super) modules: Box<[ModuleResolution]>,
    pub(super) diagnostics: Diagnostics,
    pub(super) refs: Box<[Reference]>,
    pub(super) json: Option<Arc<JsonFacts>>,
}

impl std::ops::Deref for File {
    type Target = SharedFacts;

    fn deref(&self) -> &Self::Target {
        &self.shared
    }
}

#[derive(Debug)]
pub(super) struct Reference {
    pub(super) source: OccurrenceId,
    pub(super) base: RefBase,
    pub(super) meaning: u8,
    pub(super) property: bool,
    pub(super) type_context: bool,
    pub(super) flow: Option<super::flow::Read>,
}

impl File {
    fn retain(file: ProgramFile, sources: Vec<OccurrenceId>) -> (Self, Arc<super::collect::Facts>) {
        let ProgramFile {
            module_semantics,
            search,
            facts,
            modules,
            diagnostics,
            // These spellings are already owned by FileLookup; navigation was
            // transferred to its owner before this final file-facts transfer.
            paths: _,
            navigation,
        } = file;
        assert!(
            navigation.is_none(),
            "navigation transfers before binding facts"
        );
        let super::FileFacts {
            module,
            declaration,
            requests,
            facts,
            json,
            // Loading already consumed the JSX indicator for the runtime import.
            jsx: _,
            // The Diagnostics product already owns directive coverage.
            directives: _,
            // Loading already failed on the reported errors.
            source_candidates: _,
            javascript_syntax: _,
            parameter_decorators: _,
            javascript_refusals: _,
        } = facts;
        assert_eq!(facts.refs.len(), sources.len());
        let refs = facts
            .refs
            .iter()
            .zip(sources)
            .map(|(reference, source)| Reference {
                source,
                base: reference.base,
                meaning: reference.meaning,
                property: reference.property,
                type_context: reference.type_context,
                flow: facts.names[reference.token.index()].flow,
            })
            .collect();
        let file = Self {
            module_semantics,
            search,
            shared: Arc::clone(&facts.shared),
            module,
            declaration,
            requests: requests.into_boxed_slice(),
            modules: modules.into_boxed_slice(),
            diagnostics,
            refs,
            json,
        };
        (file, facts)
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{AnalysisError, InputErrorKind, QueryErrorKind};
    use crate::project::Project;
    use crate::project::binding::index::{OccurrenceRef, OccurrenceTarget};
    use crate::references::ReferenceQuery;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(files: &[(&str, &str)], roots: &[&str]) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "tsparse-retained-bindings-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).unwrap();
            let fixture = Self(root);
            fixture.write(
                "tsconfig.json",
                &serde_json::json!({
                    "compilerOptions": {"noLib": true, "types": []},
                    "files": roots,
                })
                .to_string(),
            );
            for &(file, source) in files {
                fixture.write(file, source);
            }
            fixture
        }

        fn write(&self, file: &str, source: &str) {
            let file = self.0.join(file);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, source).unwrap();
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

    /// Native declaration association and legacy lexical projection are
    /// separate questions, both pinned after final Project load.
    #[test]
    fn native_same_table_associations_preserve_lexical_meaning_groups() {
        use crate::Span;
        let original = concat!(
            "const \\u0041lias = 1;\n",
            "type Alias = { value: string };\n",
            "declare const value: Alias;\n",
        );
        for separator in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
            let source = format!("\u{feff}/*😀*/{}", original.replace('\n', separator));
            let fixture = Fixture::new(&[("names.ts", &source)], &["names.ts"]);
            let project = fixture.load();
            let file = project.sources.ids().next().unwrap();
            let captured = project.sources.get(file).text.as_str();
            assert_eq!(captured, crate::source::decode(source.as_bytes()).unwrap());
            let anchor = |prefix: &str, raw: &str| {
                let start = captured.find(&format!("{prefix}{raw}")).unwrap() + prefix.len();
                Span {
                    start,
                    end: start + raw.len(),
                }
            };
            let value_anchor = anchor("const ", r"\u0041lias");
            let type_anchor = anchor("type ", "Alias");
            let value = project
                .bindings
                .native_inputs()
                .declaration_symbol(file, value_anchor)
                .unwrap();
            let alias = project
                .bindings
                .native_inputs()
                .declaration_symbol(file, type_anchor)
                .unwrap();
            // Native declareSymbolEx reuses the same compatible name-table
            // symbol. BoundSymbol is that identity, not a lexical meaning slot.
            assert_eq!(value, alias);
            let native = project
                .bindings
                .native_inputs()
                .native_declarations(alias)
                .unwrap();
            assert_eq!(project.bindings.native_inputs().declarations(value), native);
            assert_eq!(
                native.iter().map(|own| own.anchor).collect::<Vec<_>>(),
                [value_anchor, type_anchor]
            );
            assert_eq!(
                project
                    .bindings
                    .native_inputs()
                    .native_declarations(value)
                    .unwrap(),
                native
            );
            let groups = project
                .find_references(ReferenceQuery::Name {
                    name: "Alias".into(),
                    file: None,
                })
                .unwrap()
                .groups;
            let type_use = anchor("value: ", "Alias");
            let location = |span: Span| crate::references::Location {
                file: "names.ts".into(),
                start: project.sources.get(file).text.position(span.start),
                end: project.sources.get(file).text.position(span.end),
            };
            assert_eq!(groups.len(), 2);
            assert_eq!(
                groups
                    .iter()
                    .map(|group| group
                        .declarations
                        .iter()
                        .map(|declaration| declaration.location.clone())
                        .collect::<Vec<_>>())
                    .collect::<Vec<_>>(),
                [vec![location(value_anchor)], vec![location(type_anchor)]]
            );
            assert_eq!(
                groups
                    .iter()
                    .map(|group| group
                        .occurrences
                        .iter()
                        .map(|occurrence| occurrence.location.clone())
                        .collect::<Vec<_>>())
                    .collect::<Vec<_>>(),
                [
                    vec![location(value_anchor)],
                    vec![location(type_anchor), location(type_use)]
                ]
            );
        }
    }

    /// Mechanical native binding-fact retention, checked after final load and
    /// lexical projection rather than against the transient capture helper.
    #[test]
    fn retained_type_literal_symbols_own_exact_tables_after_final_project_load() {
        use super::super::semantic::{BoundMemberKind, BoundValueType};
        use crate::Span;
        let original = concat!(
            "export type Empty = {};\n",
            "export type Box = { \"\\u0078\": number; get pair(): string; set pair(value: number); call(): boolean; };\n",
            "export type Other = { \"\\u0078\": string; };\n",
            "export type Invoker = { (): number; };\n",
            "declare let value: Box;\n",
        );
        for separator in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
            let source = format!("\u{feff}/*😀*/{}", original.replace('\n', separator));
            let fixture = Fixture::new(&[("literal.ts", &source)], &["literal.ts"]);
            let project = fixture.load();
            let file = project.sources.ids().next().unwrap();
            let captured = project.sources.get(file).text.as_str();
            let bindings = &project.bindings;
            let bound = |name| {
                let prefix = format!("type {name} = ");
                let start = captured.find(&prefix).unwrap() + prefix.len();
                bindings
                    .native_inputs()
                    .type_literal(
                        file,
                        Span {
                            start,
                            end: start + 1,
                        },
                    )
                    .unwrap()
            };
            let empty = bound("Empty");
            let box_type = bound("Box");
            let other = bound("Other");
            let invoker = bound("Invoker");
            assert_ne!(box_type.symbol, other.symbol);
            assert_ne!(empty.symbol, invoker.symbol);
            assert!(
                bindings
                    .native_inputs()
                    .type_literal_members_empty(empty)
                    .unwrap()
            );
            assert!(
                !bindings
                    .native_inputs()
                    .type_literal_members_empty(invoker)
                    .unwrap(),
                "a reserved call signature is a real member despite no named properties"
            );
            assert!(
                bindings
                    .native_inputs()
                    .named_members(invoker.symbol)
                    .unwrap()
                    .is_empty()
            );
            let members = bindings
                .native_inputs()
                .named_members(box_type.symbol)
                .unwrap();
            // The raw native table has no order. getNamedMembers owns the
            // later semantic list order; retain exact keys and associations here.
            assert_eq!(members.len(), 3);
            let members: BTreeMap<_, _> = members
                .into_iter()
                .map(|member| {
                    (
                        bindings
                            .native_inputs()
                            .native_symbol(member.symbol)
                            .unwrap()
                            .name()
                            .unwrap(),
                        member.symbol,
                    )
                })
                .collect();
            assert_eq!(
                members.keys().cloned().collect::<Vec<_>>(),
                [b"call".to_vec(), b"pair".to_vec(), b"x".to_vec()]
            );
            let property_symbol = members[b"x".as_slice()];
            let accessor_symbol = members[b"pair".as_slice()];
            let other_x = bindings
                .native_inputs()
                .named_members(other.symbol)
                .unwrap()
                .remove(0);
            assert_ne!(property_symbol, other_x.symbol);
            let BoundValueType::Member(property) = bindings
                .native_inputs()
                .value_type(property_symbol)
                .unwrap()
            else {
                panic!("the type literal property has a canonical member association");
            };
            let anchor = property.value_declaration.anchor;
            assert_eq!(&captured[anchor.start..anchor.end], r#""\u0078""#);
            let BoundValueType::Member(pair) = bindings
                .native_inputs()
                .value_type(accessor_symbol)
                .unwrap()
            else {
                panic!("the accessor pair has one actual member symbol");
            };
            assert_eq!(pair.kind, BoundMemberKind::Accessor);
            let roles: Vec<_> = bindings
                .native_inputs()
                .reference_symbol_declarations(crate::project::binding::ReferenceSymbol::Known(
                    crate::project::binding::ReadOrigin::Direct(accessor_symbol),
                ))
                .unwrap()
                .into_iter()
                .map(|declaration| declaration.kind)
                .collect();
            assert_eq!(
                roles,
                [
                    crate::syntax_kind::SyntaxKind::GetAccessor,
                    crate::syntax_kind::SyntaxKind::SetAccessor
                ]
            );
            assert_eq!(
                bindings
                    .native_inputs()
                    .raw_parent_symbol(
                        bindings
                            .native_inputs()
                            .property_parent(accessor_symbol)
                            .unwrap()
                    )
                    .unwrap(),
                box_type.symbol
            );
            let groups = project
                .find_references(ReferenceQuery::Name {
                    name: "Box".into(),
                    file: None,
                })
                .unwrap()
                .groups;
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].occurrences.len(), 2);
        }
    }

    /// Mechanical `bindFunctionOrConstructorType` retention proof at final
    /// Project load: both own associations survive, node.Symbol is the final
    /// TypeLiteral, and reserved signatures stay separate native identities.
    #[test]
    fn retained_function_constructor_type_nodes_keep_both_native_symbol_roles() {
        use super::super::member::MemberKind;
        use crate::{Span, syntax_kind::SyntaxKind};
        let original = concat!(
            "export type Fn = /*#call*/ (x: string) => number;\n",
            "export type Ctor = /*#new*/ new (x: number) => string;\n",
            "export type Generic = /*#generic*/ <T>(x: T) => T;\n",
            "export type Abstract = /*#abstract*/ abstract new (x: string) => number;\n",
            "declare let value: Fn;\n",
        );
        for separator in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
            let source = format!("\u{feff}/*😀*/{}", original.replace('\n', separator));
            let fixture = Fixture::new(&[("anonymous.ts", &source)], &["anonymous.ts"]);
            let project = fixture.load();
            let file = project.sources.ids().next().unwrap();
            let captured = project.sources.get(file).text.as_str();
            assert_eq!(captured, crate::source::decode(source.as_bytes()).unwrap());
            let bindings = &project.bindings;
            for (marker, first, kind, native_kind) in [
                ("call", "(", MemberKind::Call, SyntaxKind::FunctionType),
                (
                    "new",
                    "new",
                    MemberKind::Construct,
                    SyntaxKind::ConstructorType,
                ),
                ("generic", "<", MemberKind::Call, SyntaxKind::FunctionType),
                (
                    "abstract",
                    "abstract",
                    MemberKind::Construct,
                    SyntaxKind::ConstructorType,
                ),
            ] {
                let prefix = format!("/*#{marker}*/ ");
                let start = captured.find(&prefix).unwrap() + prefix.len();
                let anchor = Span {
                    start,
                    end: start + first.len(),
                };
                let bound = bindings.native_inputs().type_literal(file, anchor).unwrap();
                assert_eq!(
                    bindings
                        .native_inputs()
                        .declaration_symbol(file, anchor)
                        .unwrap(),
                    bound.symbol
                );
                assert!(
                    !bindings
                        .native_inputs()
                        .type_literal_members_empty(bound)
                        .unwrap()
                );
                assert!(
                    bindings
                        .native_inputs()
                        .named_members(bound.symbol)
                        .unwrap()
                        .is_empty()
                );
                let signatures = bindings
                    .native_inputs()
                    .signature_declarations(bound.symbol, kind)
                    .unwrap();
                assert_eq!(signatures.len(), 1);
                assert_eq!(signatures[0].native_kind, native_kind);
                assert_eq!(signatures[0].declaration.anchor, anchor);
                assert_ne!(signatures[0].declaration.id, bound.declaration.id);
                let signature = bindings
                    .native_inputs()
                    .member_symbol(file, anchor)
                    .unwrap();
                assert_ne!(signature, bound.symbol);
                assert!(
                    signature.0.index() < bound.symbol.0.index(),
                    "native binder allocates the Signature first"
                );
                assert_eq!(
                    bindings
                        .native_inputs()
                        .native_symbol_flags(bound.symbol)
                        .bits,
                    super::super::link::flags::TYPE_LITERAL
                );
                assert_eq!(
                    bindings.native_inputs().native_symbol_flags(signature).bits,
                    super::super::link::flags::SIGNATURE
                );
                assert_eq!(
                    bindings.native_inputs().declarations(signature),
                    [signatures[0].declaration]
                );
                assert_eq!(
                    bindings.native_inputs().declarations(bound.symbol),
                    [bound.declaration]
                );
                let other = if kind == MemberKind::Call {
                    MemberKind::Construct
                } else {
                    MemberKind::Call
                };
                assert!(
                    bindings
                        .native_inputs()
                        .signature_declarations(bound.symbol, other)
                        .unwrap()
                        .is_empty()
                );
            }
            let groups = project
                .find_references(ReferenceQuery::Name {
                    name: "Fn".into(),
                    file: None,
                })
                .unwrap()
                .groups;
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].occurrences.len(), 2);
        }
    }

    /// Mechanical native binding-fact retention, checked after final load and
    /// lexical projection rather than against the transient capture helper.
    #[test]
    fn retained_interface_contains_this_matches_native_container_lifecycle() {
        let original = include_str!("../../../fixtures/types/native-contains-this/containers.ts");
        let expected = BTreeMap::from([
            ("Plain", false),
            ("NamedThis", false),
            ("Direct", true),
            ("Method", true),
            ("Call", true),
            ("Construct", true),
            ("FunctionProperty", true),
            ("ConstructorProperty", true),
            ("NestedType", true),
            ("Index", true),
            ("Generic", true),
            ("FunctionExpression", false),
            ("ArrowExpression", true),
            ("ObjectMethod", false),
            ("ClassMethod", false),
            ("ClassInitializer", false),
            ("EarlierThis", true),
            ("Inner", true),
            ("AfterFunction", false),
            ("NamespaceInner", true),
            ("AfterNamespace", false),
        ]);
        for separator in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
            let source = format!("\u{feff}/*😀*/{}", original.replace('\n', separator));
            let fixture = Fixture::new(&[("containers.ts", &source)], &["containers.ts"]);
            let project = fixture.load();
            let file = project.sources.ids().next().unwrap();
            let actual: BTreeMap<_, _> = project.bindings.files[file.index()]
                .decls
                .iter()
                .filter(|declaration| {
                    declaration.kind == super::super::collect::DeclKind::Interface
                })
                .map(|declaration| {
                    (
                        &*declaration.name,
                        declaration
                            .contains_this
                            .expect("final interface flag is attached"),
                    )
                })
                .collect();
            assert_eq!(actual, expected);
            // The canonical header reads the exact same own attachment.
            for declaration in &project.bindings.files[file.index()].decls {
                if declaration.kind != super::super::collect::DeclKind::Interface {
                    continue;
                }
                let symbol = project
                    .bindings
                    .native_inputs()
                    .declaration_symbol(file, declaration.anchor)
                    .unwrap();
                let target = project
                    .bindings
                    .native_inputs()
                    .class_or_interface(symbol)
                    .unwrap();
                assert_eq!(target.declarations.len(), 1);
                assert_eq!(
                    target.declarations[0].contains_this,
                    declaration.contains_this
                );
            }
        }
    }

    /// Mechanical ownership verification through the final Project boundary,
    /// with the existing compiler-pinned lexical question as its control. This
    /// introduces no retained-fact query operation or new semantic behavior.
    #[test]
    fn retained_searched_attachments_survive_projection_and_final_tree_reparse() {
        for separator in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
            for spelling in ["a", r"\u0061", r"\u{61}"] {
                let final_use = format!("/*😀*/{spelling};");
                let source = [
                    "\u{feff}export {};",
                    "await 0;",
                    r"const \u0061 = 1;",
                    "function later(input = a) { return input + a; }",
                    "const obj = { a };",
                    "obj.a;",
                    &final_use,
                ]
                .join(separator);
                let fixture = Fixture::new(&[("a.ts", &source)], &["a.ts"]);
                let project = fixture.load();
                let id = project.sources.ids().next().unwrap();
                let captured = project.sources.get(id).text.as_str();
                assert_eq!(captured, source.strip_prefix('\u{feff}').unwrap());
                let bindings = &project.bindings;
                let facts = &bindings.files[id.index()];
                let mut names = BTreeMap::new();
                for reference in &facts.refs {
                    let occurrence = reference.source;
                    let occurrence = bindings.index.occurrence(OccurrenceRef {
                        file: id,
                        id: occurrence,
                    });
                    let name = bindings.index.text(occurrence.name);
                    *names.entry(name).or_insert(0) += 1;
                    let raw = &captured[occurrence.span.start..occurrence.span.end];
                    if raw.contains('\\') {
                        assert_eq!(name, "a");
                        assert_eq!(raw, spelling);
                    } else {
                        assert_eq!(raw, name);
                    }
                    let result = project.find_references(ReferenceQuery::At {
                        file: fixture.0.join("a.ts"),
                        position: project.sources.get(id).text.position(occurrence.span.start),
                    });
                    match occurrence.target {
                        OccurrenceTarget::Symbols(symbols) => {
                            assert_eq!(symbols.len(), 1);
                            let groups = result.unwrap().groups;
                            assert_eq!(groups.len(), 1);
                            assert_eq!(groups[0].declarations.len(), 1);
                            assert_eq!(groups[0].declarations[0].name, name);
                        }
                        OccurrenceTarget::NotReference(reason) => {
                            assert_eq!(reason, "property");
                            assert!(reference.property);
                            let AnalysisError::Query(error) = result.unwrap_err() else {
                                panic!("a property remains an unsupported lexical target");
                            };
                            assert_eq!(error.kind, QueryErrorKind::UnsupportedTarget);
                        }
                        other => panic!("unexpected final attachment: {other:?}"),
                    }
                }
                assert_eq!(names, BTreeMap::from([("a", 5), ("input", 2), ("obj", 1)]));
                let parameter = facts
                    .decls
                    .iter()
                    .find(|declaration| &*declaration.name == "input")
                    .unwrap();
                let name_reference = parameter
                    .name_reference
                    .expect("getResolvedSymbol retains the parameter Name question");
                let declaring_reference = &facts.refs[name_reference.index()];
                let declaring_occurrence = bindings.index.occurrence(OccurrenceRef {
                    file: id,
                    id: declaring_reference.source,
                });
                assert_eq!(declaring_occurrence.span, parameter.anchor);
                let input = project
                    .find_references(ReferenceQuery::Name {
                        name: "input".into(),
                        file: None,
                    })
                    .unwrap();
                assert_eq!(input.groups.len(), 1);
                assert_eq!(input.groups[0].declarations.len(), 1);
                assert_eq!(
                    input.groups[0].occurrences.len(),
                    2,
                    "retaining the declaring Name question does not add a navigation occurrence"
                );
                let groups = project
                    .find_references(ReferenceQuery::Name {
                        name: "a".into(),
                        file: None,
                    })
                    .unwrap()
                    .groups;
                assert_eq!(groups.len(), 1);
                assert_eq!(groups[0].occurrences.len(), 5);
                assert_eq!(groups[0].declarations[0].name, "a");
                let last = groups[0].occurrences.last().unwrap();
                assert_eq!(last.name, "a");
                assert_eq!(
                    last.location.start,
                    crate::source::SourcePosition { line: 7, column: 7 }
                );
                assert_eq!(
                    last.location.end,
                    crate::source::SourcePosition {
                        line: 7,
                        column: 7 + spelling.len()
                    }
                );
            }
        }
    }

    #[test]
    fn retained_dependency_tokens_stay_unsearched_after_semantic_queries() {
        let fixture = Fixture::new(
            &[
                (
                    "a.ts",
                    "import type { Used } from 'dep'; let used: Used; used;",
                ),
                ("node_modules/dep/package.json", r#"{"types":"index.d.ts"}"#),
                (
                    "node_modules/dep/index.d.ts",
                    r"export interface Used { item: Healthy } interface Healthy {} import type { Never } from './unused'; declare const h\u0069dden: M\u0069ssing;",
                ),
                ("node_modules/dep/unused.d.ts", "export interface Never {}"),
            ],
            &["a.ts"],
        );
        let project = fixture.load();
        // A semantic request resolves its own dependencies while leaving the
        // dependency token inventory outside the lexical query projection.
        let selected = project
            .sources
            .ids()
            .find(|&id| project.sources.get(id).display == "a.ts")
            .unwrap();
        let input = project.sources.get(selected);
        let request = crate::types::TypeRequest {
            query: crate::types::TypeQuery::At {
                file: input.path.clone(),
                position: input
                    .text
                    .position(input.text.as_str().find("used:").unwrap()),
            },
            expand: true,
            depth: 1,
        };
        let answer = project.type_of(request).unwrap();
        let crate::types::TypeAnswer::Selected(answer) = answer else {
            panic!("expected selected type");
        };
        assert_eq!(answer.outcome, crate::types::TypeOutcome::Found);
        assert_eq!(answer.description.summary, "Used");
        let id = project
            .sources
            .ids()
            .find(|&id| project.sources.get(id).display.ends_with("dep/index.d.ts"))
            .unwrap();
        let bindings = &project.bindings;
        let facts = &bindings.files[id.index()];
        assert!(!facts.search);
        assert_eq!(facts.refs.len(), 2);
        assert!(facts.refs.iter().all(|reference| matches!(
            bindings.index.occurrence(OccurrenceRef {
                file: id,
                id: reference.source,
            }).target,
            OccurrenceTarget::Unsearched
        )));
        let missing = facts
            .refs
            .iter()
            .find(|reference| {
                bindings.index.text(
                    bindings
                        .index
                        .occurrence(OccurrenceRef {
                            file: id,
                            id: reference.source,
                        })
                        .name,
                ) == "Missing"
            })
            .unwrap();
        let occurrence = bindings.index.occurrence(OccurrenceRef {
            file: id,
            id: missing.source,
        });
        assert!(matches!(occurrence.target, OccurrenceTarget::Unsearched));
        assert_eq!(bindings.index.text(occurrence.name), "Missing");
        let source = project.sources.get(id).text.as_str();
        assert_eq!(
            &source[occurrence.span.start..occurrence.span.end],
            r"M\u0069ssing"
        );
        assert!(
            bindings
                .index
                .occurrence_at(id, occurrence.span.start)
                .is_none()
        );
        assert!(
            bindings
                .reference_token_at(id, occurrence.span.start)
                .is_some()
        );
        assert_eq!(facts.aliases.len(), 1);
        let alias = &facts.aliases[0];
        assert!(alias.type_only);
        let native_alias = bindings
            .native_inputs()
            .native_declaration_symbol(id, alias.native_kind, alias.native_span)
            .unwrap();
        let mut own = bindings
            .native_inputs()
            .raw_symbol_declarations(native_alias);
        assert_eq!(own.len(), 1);
        assert!(matches!(
            own.next().unwrap().0,
            super::super::store::SymbolDeclaration::Alias(_)
        ));
        assert!(bindings.index.candidates("Never").is_empty());
        assert!(bindings.index.candidates("Missing").is_empty());
        let groups = project
            .find_references(ReferenceQuery::Name {
                name: "Used".into(),
                file: None,
            })
            .unwrap()
            .groups;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].occurrences.len(), 2);
        assert!(
            groups[0].declarations[0]
                .location
                .file
                .ends_with("dep/index.d.ts")
        );
    }

    #[test]
    fn retained_meanings_preserve_type_only_routes_and_merged_declarations() {
        let fixture = Fixture::new(
            &[
                (
                    "a.ts",
                    "export class Shape {} export namespace Shape { export const tag = 1; }",
                ),
                (
                    "b.ts",
                    "import type { Shape } from './a'; let typed: Shape; Shape;",
                ),
            ],
            &["a.ts", "b.ts"],
        );
        let project = fixture.load();
        let a = project
            .sources
            .ids()
            .find(|&id| project.sources.get(id).display == "a.ts")
            .unwrap();
        let b = project
            .sources
            .ids()
            .find(|&id| project.sources.get(id).display == "b.ts")
            .unwrap();
        let bindings = &project.bindings;
        assert!(bindings.files[b.index()].aliases[0].type_only);
        let declarations: Vec<_> = bindings.files[a.index()]
            .decls
            .iter()
            .filter(|decl| &*decl.name == "Shape")
            .collect();
        assert_eq!(declarations.len(), 2);
        let native = bindings.native_inputs();
        let symbols: Vec<_> = declarations
            .iter()
            .map(|decl| native.declaration_symbol(a, decl.anchor).unwrap())
            .collect();
        assert_eq!(symbols[0], symbols[1]);
        assert_eq!(native.declarations(symbols[0]).len(), 2);
        let source = &project.sources.get(b).text;
        let type_offset = source.as_str().find("typed: Shape").unwrap() + "typed: ".len();
        let value_offset = source.as_str().rfind("Shape;").unwrap();
        let type_groups = project
            .find_references(ReferenceQuery::At {
                file: fixture.0.join("b.ts"),
                position: source.position(type_offset),
            })
            .unwrap()
            .groups;
        assert_eq!(type_groups.len(), 1);
        assert_eq!(type_groups[0].declarations.len(), 2);
        let AnalysisError::Query(error) = project
            .find_references(ReferenceQuery::At {
                file: fixture.0.join("b.ts"),
                position: source.position(value_offset),
            })
            .unwrap_err()
        else {
            panic!("type-only import cannot select a lexical value target");
        };
        assert_eq!(error.kind, QueryErrorKind::UnresolvedTarget);
        let groups = project
            .find_references(ReferenceQuery::Name {
                name: "Shape".into(),
                file: None,
            })
            .unwrap()
            .groups;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].declarations.len(), 2);
        assert_eq!(groups[0].occurrences.len(), 4);
    }

    #[test]
    fn retained_binding_publishes_only_after_final_alias_conflict_checks() {
        let fixture = Fixture::new(
            &[
                ("a.ts", "import { Clash } from './dep'; interface Clash {}"),
                ("dep.ts", "export interface Clash {}"),
            ],
            &["a.ts"],
        );
        let crate::analysis::AnalysisError::Input(error) =
            Project::load(&fixture.0.join("tsconfig.json")).unwrap_err()
        else {
            panic!("the final alias check must retain its input binding failure");
        };
        assert_eq!(error.kind, InputErrorKind::Binding);
        assert!(error.message.contains("conflicts with a local declaration"));
        fixture.write("a.ts", "import { Clash } from './dep'; let value: Clash;");
        let project = fixture.load();
        assert_eq!(project.bindings.files.len(), 2);
        assert_eq!(
            project
                .find_references(ReferenceQuery::Name {
                    name: "Clash".into(),
                    file: None
                })
                .unwrap()
                .groups
                .len(),
            1
        );
    }

    #[test]
    fn retained_own_origins_stay_distinct_from_directed_alias_projection() {
        // The pinned case in reference_augmentation.rs: one-way pattern merging
        // adds an alias declaration, while its target keeps its own identity.
        let fixture = Fixture::new(
            &[
                (
                    "ambient.d.ts",
                    "declare module \"*.css\" { namespace Other { interface X { a: number } } export import X = Other.X; }\n",
                ),
                (
                    "aug.ts",
                    "import \"x.css\";\ndeclare module \"x.css\" { interface X { b: number } }\n",
                ),
                ("use.ts", "import { X } from \"x.css\";\nexport let x: X;\n"),
            ],
            &["ambient.d.ts", "aug.ts", "use.ts"],
        );
        let project = fixture.load();
        let bindings = &project.bindings;
        let native = bindings.native_inputs();
        let symbol = |file: &str| {
            let id = project
                .sources
                .ids()
                .find(|&id| project.sources.get(id).display == file)
                .unwrap();
            let declaration = bindings.files[id.index()]
                .decls
                .iter()
                .find(|decl| &*decl.name == "X")
                .unwrap();
            (
                id,
                declaration.anchor,
                native.declaration_symbol(id, declaration.anchor).unwrap(),
            )
        };
        let (target_file, target_anchor, target) = symbol("ambient.d.ts");
        let (augmented_file, augmented_anchor, augmented) = symbol("aug.ts");
        assert_ne!(target, augmented);
        for (file, anchor, symbol, projected_files) in [
            (target_file, target_anchor, target, vec!["ambient.d.ts"]),
            (
                augmented_file,
                augmented_anchor,
                augmented,
                vec!["ambient.d.ts", "aug.ts"],
            ),
        ] {
            let own = native.declarations(symbol);
            assert_eq!(own.len(), 1);
            assert_eq!(own[0].file, file);
            assert_eq!(own[0].anchor, anchor);
            let groups = project
                .find_references(ReferenceQuery::At {
                    file: project.sources.get(file).path.clone(),
                    position: project.sources.get(file).text.position(anchor.start),
                })
                .unwrap()
                .groups;
            assert_eq!(groups.len(), 1);
            let projected: Vec<_> = groups[0]
                .declarations
                .iter()
                .map(|site| site.location.file.as_str())
                .collect();
            assert_eq!(projected, projected_files);
        }
        let groups = project
            .find_references(ReferenceQuery::At {
                file: fixture.0.join("aug.ts"),
                position: crate::source::SourcePosition {
                    line: 2,
                    column: 36,
                },
            })
            .unwrap()
            .groups;
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0]
                .declarations
                .iter()
                .map(|site| site.location.file.as_str())
                .collect::<Vec<_>>(),
            ["ambient.d.ts", "aug.ts"]
        );
        assert_eq!(groups[0].occurrences.len(), 3);
    }
}
