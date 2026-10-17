//! One file's semantic facts, captured from its final syntax tree.
//!
//! Every reachable identifier token becomes a `Name` with a pending target: a
//! declaration, a reference to resolve later, an alias, or a non-reference.
//! Scope discovery happens here; lookup waits until every file is collected.

use super::ids::{
    AliasId, AmbientId, DeclId, ModuleDeclarationId, NameId, RefId, RequestId, ScopeId,
    TypeParameterListId,
};
use super::instances::{Instance, Instances};
use super::member::{MemberBinding, MemberKind, declaration_anchor, declaration_key};
use super::text::{exact_text, is_text};
use super::{ModuleRequest, RequestKind, ResolutionMode, meaning};
use crate::analysis::{InputError, InputErrorKind};
use crate::compiler::node as node_flags;
use crate::project;
use crate::source::SourceText;
use crate::syntax::{child, flag, has_modifier, id as field, ids, is_token, number};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Node, NodeId, Span};
use std::collections::{HashMap, HashSet};

mod facts;
pub(super) use facts::{Facts, SharedFacts};

pub(crate) const DECLARATION: u8 = 1;
pub(crate) const USE: u8 = 2;
pub(crate) const IMPORT: u8 = 4;
pub(crate) const EXPORT: u8 = 8;

/// An identifier token and what it denotes.
#[derive(Debug)]
pub(super) struct Name {
    node: NodeId,
    pub(super) span: Span,
    pub(super) text: String,
    pub(super) roles: u8,
    pub(super) call: bool,
    /// Declaration names, local import names, and exported alias names.
    pub(super) candidate: bool,
    pub(super) target: NameTarget,
    pub(super) flow: Option<super::flow::Read>,
    pub(super) lexical: bool,
    pub(super) reference: Option<super::reference_tokens::CollectedReferenceToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NameTarget {
    Decl(DeclId),
    Ref(RefId),
    Alias(AliasId),
    NotReference(&'static str),
    /// A name that declares nothing, to which TypeScript gives no symbol.
    Unresolved(&'static str),
}

#[derive(Debug)]
pub(super) struct Scope {
    /// Actual native node whose Locals field this logical scope contributes to.
    pub(super) native_owner: Option<(SyntaxKind, Span)>,
    pub(super) kind: ScopeKind,
    pub(super) parent: Option<ScopeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScopeKind {
    /// The file: module-local for modules, the global table for scripts.
    Root,
    Block,
    /// A catch clause, whose binding a `var` in its block may redeclare.
    Catch,
    /// Parameters and signature type parameters. A non-arrow function's
    /// signature also binds the implicit `arguments` object.
    Signature {
        arguments: bool,
    },
    /// A function-like or static block body.
    Body,
    /// The own name of a function or class expression.
    ExpressionName,
    /// Class or interface type parameters, shared by merged declarations.
    Members {
        decl: DeclId,
    },
    TypeParameters,
    /// `infer` declarations, visible in a conditional type's true branch.
    Infer,
    /// An `infer` declaration's constraint, which sees that declaration's
    /// own name but not its siblings.
    InferConstraint {
        decl: DeclId,
    },
    Namespace {
        decl: DeclId,
    },
    AmbientModule {
        module: AmbientId,
    },
    Augmentation {
        request: RequestId,
    },
    /// A `declare global` block that augments the global scope: its
    /// declarations are exports of the file's global augmentation, which
    /// merges into the global table.
    Global,
    /// A `declare global` block at the top level of a script, which
    /// TypeScript rejects (TS2669) and merges nowhere, as `Unmerged`; a
    /// string-named module directly inside it is placed as one directly
    /// inside a script's top-level ambient module.
    ScriptGlobal,
    /// A `declare global` or string-named module block that TypeScript
    /// merges nowhere (`MisplacedModule::Global` and `Nested`, `Quoted` at
    /// the top level of a module, or a relative name directly inside a
    /// script's ambient module or `declare global` block): its declarations
    /// see only one another.
    Unmerged,
    Enum {
        decl: DeclId,
    },
}

/// What a declaration declares. `link` decides its symbol flags and, from
/// them, its meanings (`DeclKind::meanings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeclKind {
    Var,
    Parameter,
    /// `let`, `const`, `using`, catch variables.
    Let,
    Function,
    Class,
    Interface,
    /// `bindAnonymousDeclaration`: a TypeLiteral owns a native member table
    /// but is inserted in neither a lexical nor an export name table.
    TypeLiteral,
    /// `bindAnonymousDeclaration` attaches the literal's actual node.Symbol.
    /// Its direct members bind through the same declaring-container owner.
    ObjectLiteral,
    TypeAlias,
    Enum {
        constant: bool,
    },
    EnumMember,
    /// The instance state decides whether the namespace is a value
    /// (`Instance::NonInstantiated` is not) and, with the configuration,
    /// whether TypeScript's merge-order check applies to it.
    Namespace {
        instance: Instance,
    },
    TypeParameter,
    Member(MemberKind),
    /// An anonymous default or `export =` expression.
    Expression,
}

/// Where a `declare module "name"` or `declare global` block is declared,
/// as TypeScript's `IsModuleAugmentationExternal` classifies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleContext {
    /// The top level of a module, or directly inside an ambient module at the
    /// top level of a script: an augmentation of a module or of the global
    /// scope.
    External,
    /// The top level of a script, where a string-named block declares an
    /// ambient module.
    Script,
    /// Anywhere else, where TypeScript merges neither kind of block.
    Nested,
}

/// A module declaration TypeScript rejects where bindings are checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MisplacedModule {
    /// A string-named module outside an ambient context (TS1035).
    Quoted,
    /// `declare global` outside an external augmentation context (TS2669).
    Global,
    /// A string-named module nested where it is neither an augmentation nor
    /// an ambient module (TS2435).
    Nested,
    /// A script's top-level ambient module with a relative name (TS2436).
    Relative,
    /// `export as namespace` below the top level of a file (TS1316).
    UmdNested,
    /// `export as namespace` in a script (TS1314).
    UmdScript,
    /// `export as namespace` in a module that is not a declaration file
    /// (TS1315).
    UmdImplementation,
}

/// Where an exported declaration or alias becomes visible to importers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Host {
    File,
    Namespace(DeclId),
    AmbientModule(AmbientId),
    Augmentation(RequestId),
    /// A file's `declare global` blocks, whose exports merge into the global
    /// table.
    Global,
}

#[derive(Debug)]
pub(super) struct Decl {
    pub(super) name: String,
    /// The declaring name token, or the anchor of a synthetic declaration.
    pub(super) anchor: Span,
    /// `getResolvedSymbol` can ask a parameter's actual declaring Name.
    /// Its lexical question is separate from getSymbolOfDeclaration and uses
    /// the same token without introducing a navigation occurrence.
    pub(super) name_reference: Option<RefId>,
    pub(super) flow: Option<super::flow::Read>,
    /// Final `bindContainer` interface ContainsThis fact. None is a different
    /// declaration role, not an unproved false flag.
    pub(super) contains_this: Option<bool>,
    pub(super) kind: DeclKind,
    /// Actual native declaration-kind comparison for SetValueDeclaration.
    pub(super) native_kind: SyntaxKind,
    /// Actual declaration Loc, distinct from the declaring name anchor.
    pub(super) native_span: Option<Span>,
    /// Native member-table attachment, absent for ordinary lexical roles.
    pub(super) member: Option<Box<MemberBinding>>,
    /// The scope in which the name is lexically bound, if any.
    pub(super) lexical: Option<ScopeId>,
    /// `declareModuleMember` creates a raw local even where navigation resolves
    /// namespace names through exports instead of a projected lexical binding.
    pub(super) native_export_local: Option<ScopeId>,
    pub(super) export: Option<(Host, String)>,
    /// A function declaration with a body.
    pub(super) implementation: bool,
    /// Declared in an ambient context.
    pub(super) ambient: bool,
    /// The type parameter list of a type parameter; one list cannot repeat
    /// a name, while lists of merged declarations share their names.
    pub(super) list: Option<TypeParameterListId>,
    /// Blocks a `var` is hoisted out of, whose block-scoped bindings it may
    /// not share a name with.
    pub(super) hoisted: Vec<ScopeId>,
    /// Actual class/interface/alias parameter names, default presence and
    /// declaration anchors. Names/defaults serve merge checks; anchors retain
    /// getSymbolOfDeclaration identity for native local parameter acquisition.
    pub(super) type_parameters: Option<Vec<(String, bool, Span)>>,
    /// The members of an interface that may be a JSON value's apparent type
    /// (`super::APPARENT_INTERFACES`); other declarations record none.
    pub(super) interface_members: Option<Box<InterfaceMembers>>,
}

/// The property and method names one interface declaration declares, and
/// whether it extends other types, whose members its type has too.
#[derive(Debug, Clone, Default)]
pub(super) struct InterfaceMembers {
    pub(super) names: Vec<String>,
    pub(super) inherits: bool,
}

impl Default for Decl {
    fn default() -> Self {
        Self {
            name: String::new(),
            anchor: Span { start: 0, end: 0 },
            name_reference: None,
            flow: None,
            contains_this: None,
            kind: DeclKind::Var,
            native_kind: SyntaxKind::VariableDeclaration,
            native_span: None,
            member: None,
            lexical: None,
            native_export_local: None,
            export: None,
            implementation: false,
            ambient: false,
            list: None,
            hoisted: Vec::new(),
            type_parameters: None,
            interface_members: None,
        }
    }
}

#[derive(Debug)]
pub(super) struct Ref {
    pub(super) token: NameId,
    pub(super) base: RefBase,
    pub(super) meaning: u8,
    /// A property-access name, which is a property when its receiver is not
    /// a resolved namespace.
    pub(super) property: bool,
    /// Inside a type, where type-only aliases may supply values to `typeof`.
    pub(super) type_context: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefBase {
    Lexical(ScopeId),
    /// A name qualified by the namespace-like target of another reference.
    Member(RefId),
    /// An export of a module request, as in `import("m").Name`.
    Module(RequestId),
}

#[derive(Debug)]
pub(super) struct Alias {
    pub(super) kind: AliasKind,
    /// The actual native alias declaration, distinct from its diagnostic/name anchor.
    pub(super) native_kind: SyntaxKind,
    pub(super) native_span: Span,
    /// Actual target syntax for getTargetOfAliasDeclaration, separate from
    /// the exported name token used as the public/navigation anchor.
    pub(super) target_name: Option<(SyntaxKind, Span)>,
    pub(super) bind: AliasBind,
    pub(super) type_only: bool,
    /// Where diagnostics point: the alias's own name token, or for the
    /// entity alias of `export =` or `export default`, that keyword.
    pub(super) span: Span,
    /// `checkAliasSymbol` reports on the actual declaration through
    /// `GetErrorRangeForNode`, a distinct question from the alias name anchor.
    pub(super) conflict_span: Span,
    /// An export specifier, `export * as`, or export assignment, which is not
    /// a lexical name even inside the namespace whose exports hold it.
    pub(super) export_only: bool,
    pub(super) flow: Option<super::flow::Read>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AliasKind {
    /// A named or default export of a requested module.
    Import { request: RequestId, name: String },
    /// The module object, or its `export =` target.
    Namespace { request: RequestId },
    /// This file as a module, for `export as namespace`.
    SelfModule,
    /// Every meaning of a lexically visible name.
    Lexical { scope: ScopeId, name: String },
    /// The target of an entity-name reference such as `A.B.C`. With
    /// `expand`, a name found as a namespace aliases all of its meanings.
    Entity { reference: RefId, expand: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AliasBind {
    Lexical {
        scope: ScopeId,
        name: String,
    },
    Export {
        host: Host,
        name: String,
    },
    Both {
        scope: ScopeId,
        host: Host,
        name: String,
    },
    Global {
        name: String,
    },
}

#[derive(Debug)]
pub(super) struct Star {
    pub(super) declaration_span: Span,
    pub(super) host: Host,
    pub(super) request: RequestId,
    pub(super) type_only: bool,
}

#[derive(Debug)]
pub(super) struct AmbientModule {
    pub(super) name: String,
    /// The name's string literal token.
    pub(super) span: Span,
    /// `declare module "x";` without a body.
    pub(super) shorthand: bool,
}

/// Native ambient/module-augmentation declaration attachment. Navigation's
/// module request and ambient-host records do not stand in for node.Symbol or
/// Symbol.Declarations, so the binder consumes this actual event in order.
#[derive(Debug)]
pub(super) struct ModuleDeclaration {
    pub(super) shorthand: bool,
    pub(super) scope: ScopeId,
    pub(super) host: Option<Host>,
    pub(super) span: Span,
    pub(super) name: String,
    pub(super) global: bool,
    pub(super) value: bool,
}

/// Captured facts for one file, with file-local indices.
#[derive(Debug, Default)]
struct CollectedFacts {
    pub(super) module_declarations: Vec<ModuleDeclaration>,
    /// The existing lexical traversal's exact lookup location, for checker
    /// resolveName questions whose text is synthesized instead of in source.
    pub(super) lookup_scopes: super::site_map::SiteMap<ScopeId>,
    /// Native ConditionalType.Locals, retained separately from source names.
    pub(super) conditional_scopes: HashMap<Span, ScopeId>,
    pub(super) flow: Option<super::flow::Graph>,
    pub(super) names: Vec<Name>,
    pub(super) scopes: Vec<Scope>,
    pub(super) decls: Vec<Decl>,
    /// Binder visitation order across declarations and aliases. Native class
    /// prototype installation observes earlier export aliases before children.
    pub(super) binding_order: Vec<BindingEntry>,
    /// Transient `bindFunctionOrConstructorType` role pairs. The linker
    /// allocates the reserved Signature before the final TypeLiteral owner;
    /// completed canonical associations replace this sparse capture map.
    pub(super) anonymous_signatures: HashMap<DeclId, DeclId>,
    pub(super) refs: Vec<Ref>,
    pub(super) aliases: Vec<Alias>,
    pub(super) stars: Vec<Star>,
    pub(super) ambient_modules: Vec<AmbientModule>,
    /// The declaration names TypeScript reports where declarations of one
    /// name in one container are both exported and local (TS2395, TS2383),
    /// in source order (`Collector::mixed_exports`).
    pub(super) mixed_exports: Vec<NameId>,
    /// Module declarations TypeScript rejects, for the binding policy.
    pub(super) misplaced_modules: Vec<(Span, MisplacedModule)>,
    /// Module augmentations whose bodies create values, which TypeScript
    /// marks `ValueModule` (`getModuleInstanceState`).
    pub(super) instantiated_augmentations: Vec<RequestId>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum BindingEntry {
    Declaration(DeclId),
    Alias(AliasId),
    Module(ModuleDeclarationId),
    /// `bindContainer` calls `bindCommonJSTypeExports` after all children;
    /// None identifies this external source file, Some an ambient module node.
    CommonJsTypeExports(Option<ModuleDeclarationId>),
}

enum CollectWork {
    Visit(NodeId, Cx),
    FinishAmbientModule(ModuleDeclarationId),
}

/// The meanings an entity name such as `A.B.C` asks for: a lone name, the
/// head of a qualified name, its middle parts, and its last part.
#[derive(Debug, Clone, Copy)]
struct Path {
    single: u8,
    head: u8,
    middle: u8,
    last: u8,
    /// Qualified-name parts access properties, as in `typeof a.b`, so an
    /// unresolved receiver makes them properties rather than unresolved.
    /// Property-access parts, as in `a.b`, always do.
    properties: bool,
}

impl Path {
    const TYPE: Self = Self {
        single: meaning::TYPE,
        head: meaning::NAMESPACE,
        middle: meaning::NAMESPACE,
        last: meaning::TYPE,
        properties: false,
    };
    const VALUE: Self = Self {
        single: meaning::VALUE,
        head: meaning::VALUE,
        middle: meaning::VALUE,
        last: meaning::VALUE,
        properties: true,
    };
    /// `typeof import("m").a.b`: values. TypeScript reads `b` as a property
    /// of `a`'s type, as in `typeof a.b`, but a part with an unresolved
    /// receiver stays unresolved here, as the semantic oracle expects.
    const IMPORT_TYPEOF: Self = Self {
        properties: false,
        ..Self::VALUE
    };
    /// `import x = A.B`: namespaces, then any meaning of the last part.
    const IMPORT: Self = Self {
        single: meaning::NAMESPACE,
        head: meaning::NAMESPACE,
        middle: meaning::NAMESPACE,
        last: meaning::ALL,
        properties: false,
    };
    /// `export default a.b`: a value or namespace receiver, whose member is an
    /// alias only when the receiver proves to be a namespace.
    const EXPORT: Self = Self {
        single: meaning::ALL,
        head: meaning::VALUE | meaning::NAMESPACE,
        middle: meaning::VALUE | meaning::NAMESPACE,
        last: meaning::ALL,
        properties: false,
    };
}

/// Where the first part of an entity name is looked up.
#[derive(Debug, Clone, Copy)]
enum Head {
    /// The enclosing lexical scope.
    Lexical,
    /// The exports of a module request, as in `import("m").A`.
    Module(RequestId),
}

/// Statements directly inside a declaration container.
#[derive(Debug, Clone, Copy)]
struct Container {
    host: Option<Host>,
    /// Ambient containers without export declarations export every member.
    export_context: bool,
    /// Module-file exports stay visible as local names; namespace exports are
    /// found through the namespace's export table instead.
    lexical_exports: bool,
}

#[derive(Debug, Clone, Copy)]
struct Cx {
    scope: ScopeId,
    var_scope: ScopeId,
    /// The container of a node visited as one of its statements. Only
    /// `push_statements` keeps it; `enter` and every other push clear it.
    container: Option<Container>,
    infer: Option<ScopeId>,
    type_context: bool,
    /// Actual named class/interface member container, independent of lexical
    /// scope. Entering another binder container resets it.
    member_owner: Option<DeclId>,
    /// `GetContainingClass` crosses ordinary function/type containers. A new
    /// class replaces it, independently of lexical and member-table scope.
    private_class: Option<DeclId>,
}

impl Cx {
    /// Bind children in `scope`, where they are not statements of a container.
    fn enter(self, scope: ScopeId) -> Self {
        Self {
            scope,
            container: None,
            member_owner: None,
            ..self
        }
    }
    fn types(self) -> Self {
        Self {
            type_context: true,
            ..self
        }
    }
}

pub(super) struct Collector<'a, 's> {
    ast: &'a Ast<'s>,
    /// The parsed text, for error positions.
    source: &'a SourceText,
    module: bool,
    facts: CollectedFacts,
    // Scope capture mixes overwrite and first-write-only rules. Keep that
    // builder behavior intact, then compact its final associations once.
    scope_sites: HashMap<(SyntaxKind, Span), ScopeId>,
    requests: Vec<ModuleRequest>,
    work: Vec<CollectWork>,
    named: HashSet<NodeId>,
    call_targets: HashSet<NodeId>,
    /// Type parameter lists seen so far.
    lists: usize,
    /// The declarations of each name per container scope, for TS2395 and
    /// TS2383.
    container_names: HashMap<(ScopeId, String), ContainerName>,
    instances: Instances,
}

/// The declarations of one name in one container: the declaration spaces
/// its exported and its local declarations occupy, and each declaration.
#[derive(Debug, Default)]
struct ContainerName {
    exported: u8,
    local: u8,
    names: Vec<Declared>,
}

/// One declaration of a name in a container, for `Collector::mixed_exports`.
#[derive(Debug)]
struct Declared {
    name: NameId,
    spaces: u8,
    exported: bool,
    /// For a function declaration, whether it is an implementation.
    function: Option<bool>,
}

type Result<T> = std::result::Result<T, InputError>;

impl<'a, 's> Collector<'a, 's> {
    pub(super) fn new(ast: &'a Ast<'s>, source: &'a SourceText, module: bool) -> Self {
        Self {
            ast,
            source,
            module,
            facts: CollectedFacts::default(),
            scope_sites: HashMap::new(),
            requests: Vec::new(),
            work: Vec::new(),
            named: HashSet::new(),
            call_targets: ast
                .walk()
                .filter_map(|node| crate::syntax::call_target(ast, node))
                .collect(),
            lists: 0,
            container_names: HashMap::new(),
            instances: Instances::default(),
        }
    }

    pub(super) fn run(mut self) -> Result<(Facts, Vec<ModuleRequest>)> {
        let root = self.ast.root();
        let scope = self.native_scope(ScopeKind::Root, None, root);
        let statements = ids(root, "statements");
        let container = Container {
            host: self.module.then_some(Host::File),
            export_context: self.module
                && root.flags & node_flags::AMBIENT != 0
                && !has_export_declarations(self.ast, statements),
            lexical_exports: true,
        };
        let cx = Cx {
            scope,
            var_scope: scope,
            container: Some(container),
            infer: None,
            type_context: false,
            member_owner: None,
            private_class: None,
        };
        self.push_statements(statements, cx);
        while let Some(work) = self.work.pop() {
            match work {
                CollectWork::Visit(id, cx) => self.node(id, cx)?,
                CollectWork::FinishAmbientModule(module) => self
                    .facts
                    .binding_order
                    .push(BindingEntry::CommonJsTypeExports(Some(module))),
            }
        }
        if self.module {
            self.facts
                .binding_order
                .push(BindingEntry::CommonJsTypeExports(None));
        }
        self.check_coverage()?;
        self.capture_reference_tokens();
        self.facts.mixed_exports = self.mixed_exports();
        let capture = super::flow::Graph::capture(self.ast);
        for name in &mut self.facts.names {
            name.flow = capture.read(name.span);
        }
        for declaration in &mut self.facts.decls {
            declaration.flow = capture.read(declaration.anchor);
            declaration.contains_this = capture.interface_contains_this(declaration.anchor);
        }
        for alias in &mut self.facts.aliases {
            alias.flow = capture.read(alias.span);
        }
        self.facts.flow = Some(capture.graph);
        self.facts.lookup_scopes =
            super::site_map::SiteMap::from_entries(self.scope_sites.into_iter().collect());
        Ok((self.facts.into(), self.requests))
    }

    /// The declarations TypeScript reports for one name declared both
    /// exported and local in one container. `checkExportsOnMergedDeclarations`
    /// (TS2395), which any declaration but a function's runs, reports every
    /// declaration in a declaration space that both exported and local ones
    /// occupy; `checkFlagAgreementBetweenOverloads` (TS2383) reports each
    /// function declaration whose export differs from the canonical one's:
    /// the implementation, or else the first.
    fn mixed_exports(&self) -> Vec<NameId> {
        let mut mixed: Vec<NameId> = Vec::new();
        for declared in self.container_names.values() {
            let common = declared.exported & declared.local;
            if common != 0 && declared.names.iter().any(|d| d.function.is_none()) {
                mixed.extend(
                    declared
                        .names
                        .iter()
                        .filter(|d| d.spaces & common != 0)
                        .map(|d| d.name),
                );
            }
            let mut functions = declared.names.iter().filter(|d| d.function.is_some());
            let canonical = functions
                .clone()
                .find(|d| d.function == Some(true))
                .or_else(|| functions.clone().next());
            if let Some(canonical) = canonical {
                mixed.extend(
                    functions
                        .by_ref()
                        .filter(|d| d.exported != canonical.exported)
                        .map(|d| d.name),
                );
            }
        }
        mixed.sort_by_key(|name| self.facts.names[name.index()].span.start);
        mixed.dedup();
        mixed
    }

    /// Every reachable identifier must have been classified by a syntax rule.
    /// An unclassified one is reported with the syntax that contains it.
    fn check_coverage(&self) -> Result<()> {
        let root = self.ast.root_id();
        let mut stack = vec![(root, root)];
        while let Some((id, parent)) = stack.pop() {
            let node = self.ast.node(id);
            if matches!(
                node.syntax_kind,
                SyntaxKind::Identifier | SyntaxKind::PrivateIdentifier
            ) && !self.named.contains(&id)
            {
                let parent = self.ast.node(parent).kind;
                return Err(self.unsupported(node, &format!("identifier in {parent}")));
            }
            stack.extend(self.ast.children(id).map(|child| (child, id)));
        }
        Ok(())
    }

    /// Visit a node that is not a statement of a container.
    fn push(&mut self, id: Option<NodeId>, cx: Cx) {
        self.push_all(id.as_slice(), cx);
    }
    fn push_all(&mut self, ids: &[NodeId], cx: Cx) {
        let cx = Cx {
            container: None,
            ..cx
        };
        self.work
            .extend(ids.iter().rev().map(|&id| CollectWork::Visit(id, cx)));
    }
    fn push_field(&mut self, node: &Node<'s>, key: &str, cx: Cx) {
        self.push(field(node, key), cx);
    }
    fn push_list(&mut self, node: &Node<'s>, key: &str, cx: Cx) {
        if key == "statements"
            && matches!(
                node.syntax_kind,
                SyntaxKind::SourceFile | SyntaxKind::Block | SyntaxKind::ModuleBlock
            )
        {
            self.push_statements(
                ids(node, key),
                Cx {
                    container: None,
                    ..cx
                },
            );
        } else {
            self.push_all(ids(node, key), cx);
        }
    }
    /// Visit the statements of `cx.container`.
    fn push_statements(&mut self, ids: &[NodeId], cx: Cx) {
        self.work.extend(
            crate::syntax::statement_bind_order(self.ast, ids)
                .rev()
                .map(|id| CollectWork::Visit(id, cx)),
        );
    }

    fn scope(&mut self, kind: ScopeKind, parent: Option<ScopeId>) -> ScopeId {
        self.facts.scopes.push(Scope {
            kind,
            parent,
            native_owner: None,
        });
        ScopeId::new(self.facts.scopes.len() - 1)
    }

    /// `bindContainer`: retain the actual Locals owner independently from
    /// lookup-only scope partitions (notably function signature versus body).
    fn native_scope(
        &mut self,
        kind: ScopeKind,
        parent: Option<ScopeId>,
        node: &Node<'s>,
    ) -> ScopeId {
        let scope = self.scope(kind, parent);
        self.facts.scopes[scope.index()].native_owner = Some((node.syntax_kind, node.span));
        // Conditional locals apply only while entering its true child. The
        // declaration node itself keeps the outer context captured by node().
        if kind != ScopeKind::Infer {
            self.scope_sites
                .insert((node.syntax_kind, node.span), scope);
        }
        scope
    }

    /// The containing project supplies the file path.
    fn unsupported(&self, node: &Node<'s>, what: &str) -> InputError {
        let start = self.ast.token_span_of(node).start;
        InputError {
            kind: InputErrorKind::Unsupported,
            message: format!("unsupported {what} ({})", node.kind),
            path: None,
            position: Some(self.source.position(start)),
        }
    }

    /// The exact text of a string literal that names a module or an export,
    /// which tables key exactly. A name with an unpaired surrogate has no
    /// exact Rust form; replacing the surrogate would merge it with other
    /// names, so it is unsupported instead.
    fn semantic_name(&self, node: &Node<'s>) -> Result<String> {
        exact_text(self.ast, node)
            .map(|text| text.to_owned())
            .map_err(|display| {
                let what = format!("name \"{display}\" with an unpaired UTF-16 surrogate");
                self.unsupported(node, &what)
            })
    }

    /// The exact token of a node.
    fn token(&self, id: NodeId) -> Span {
        self.ast.token_span(id)
    }

    /// Record an identifier token. Candidates are the names a name query
    /// selects: declaration names, local import names, and exported alias
    /// names.
    fn name(&mut self, id: NodeId, target: NameTarget, roles: u8, candidate: bool) -> NameId {
        let first = self.named.insert(id);
        debug_assert!(first, "identifier {id:?} recorded twice");
        self.facts.names.push(Name {
            node: id,
            span: self.token(id),
            text: self.decoded(id),
            roles,
            call: self.call_targets.contains(&id),
            candidate,
            target,
            flow: None,
            lexical: true,
            reference: None,
        });
        NameId::new(self.facts.names.len() - 1)
    }

    /// Complete the same name inventory with literal keys and semantic role
    /// anchors after lexical capture. Existing names keep their lexical target.
    fn capture_reference_tokens(&mut self) {
        let mut parents = vec![None; self.ast.nodes().len()];
        for id in self.ast.walk() {
            for child in self.ast.children(id) {
                parents[child.index()] = Some(id);
            }
        }
        let mut captured: HashMap<_, _> = self
            .facts
            .names
            .iter()
            .enumerate()
            .map(|(index, name)| (name.node, index))
            .collect();
        for id in self.ast.walk() {
            let Some(reference) = super::reference_tokens::capture(self.ast, &parents, id) else {
                continue;
            };
            if let Some(&index) = captured.get(&id) {
                self.facts.names[index].reference = Some(reference);
            } else {
                // Lexical projection never observes this administrative name;
                // an unavailable exact key remains retained in `reference`.
                let text = match &reference.key {
                    Ok(text) => text.clone(),
                    Err(_) => String::new(),
                };
                captured.insert(id, self.facts.names.len());
                self.facts.names.push(Name {
                    node: id,
                    span: self.token(id),
                    text,
                    roles: 0,
                    call: self.call_targets.contains(&id),
                    candidate: false,
                    target: NameTarget::NotReference("semantic reference anchor"),
                    flow: None,
                    lexical: false,
                    reference: Some(reference),
                });
            }
        }
    }

    fn not_reference(&mut self, id: Option<NodeId>, why: &'static str) {
        if let Some(id) = id {
            if matches!(
                self.ast.node(id).syntax_kind,
                SyntaxKind::Identifier | SyntaxKind::PrivateIdentifier
            ) {
                self.name(id, NameTarget::NotReference(why), 0, false);
            }
        }
    }

    fn reference(&mut self, id: NodeId, base: RefBase, meaning: u8, cx: Cx, roles: u8) -> RefId {
        if let RefBase::Lexical(scope) = base {
            let node = self.ast.node(id);
            self.scope_sites
                .insert((node.syntax_kind, node.span), scope);
        }
        let reference = RefId::new(self.facts.refs.len());
        let token = self.name(id, NameTarget::Ref(reference), roles, false);
        self.facts.refs.push(Ref {
            token,
            base,
            meaning,
            property: false,
            type_context: cx.type_context,
        });
        reference
    }

    fn request(
        &mut self,
        specifier: &Node<'s>,
        kind: RequestKind,
        owner: &Node<'s>,
    ) -> Result<RequestId> {
        // TypeScript honors `resolution-mode` only on import types and on
        // type-only imports and exports.
        let type_only = match owner.syntax_kind {
            SyntaxKind::ImportType => true,
            SyntaxKind::ImportDeclaration => child(self.ast, owner, "importClause")
                .is_some_and(|clause| flag(clause, "isTypeOnly")),
            SyntaxKind::ExportDeclaration => flag(owner, "isTypeOnly"),
            _ => false,
        };
        self.requests.push(ModuleRequest {
            specifier: self.semantic_name(specifier)?,
            kind,
            mode: type_only
                .then(|| resolution_mode(self.ast, owner))
                .flatten(),
            span: self.ast.token_span_of(specifier),
            implicit: None,
            emit_mode: None,
            resolution_mode: None,
            resolved_path: None,
        });
        Ok(RequestId::new(self.requests.len() - 1))
    }

    fn declare(&mut self, decl: Decl) -> DeclId {
        self.facts.decls.push(decl);
        let id = DeclId::new(self.facts.decls.len() - 1);
        self.facts.binding_order.push(BindingEntry::Declaration(id));
        id
    }

    /// Declare a named statement-level declaration according to its container.
    /// `scope` is where the name is bound unless it is only exported.
    fn declare_statement(
        &mut self,
        node: &Node<'s>,
        name: Option<NodeId>,
        kind: DeclKind,
        cx: Cx,
        scope: ScopeId,
    ) -> DeclId {
        self.scope_sites
            .entry((node.syntax_kind, node.span))
            .or_insert(scope);
        // The inner names of `namespace A.B` are exported from their parents.
        let exported = has_modifier(self.ast, node, SyntaxKind::ExportKeyword)
            || node.syntax_kind == SyntaxKind::ModuleDeclaration
                && node.flags & node_flags::NESTED_NAMESPACE != 0;
        let default = has_modifier(self.ast, node, SyntaxKind::DefaultKeyword);
        let (export, lexical) = match cx.container {
            Some(container) => match container.host {
                Some(host) if exported || container.export_context => {
                    let export_name = match (default, name) {
                        (true, _) => "default".to_string(),
                        (false, Some(name)) => self.decoded(name),
                        (false, None) => "default".to_string(),
                    };
                    let lexical = (container.lexical_exports && name.is_some()).then_some(scope);
                    (Some((host, export_name)), lexical)
                }
                _ => (None, Some(scope)),
            },
            None => (None, Some(scope)),
        };
        let native_export_local = (export.is_some() && name.is_some()).then_some(scope);
        let decl = DeclId::new(self.facts.decls.len());
        let token = name.map(|name| {
            let roles = DECLARATION | if exported { EXPORT } else { 0 };
            self.name(name, NameTarget::Decl(decl), roles, true)
        });
        // TypeScript requires merged declarations to be all exported or all
        // local within one container (`mixed_exports`).
        if let (Some(container), Some(name), Some(token)) = (cx.container, name, token) {
            if !container.export_context {
                let key = (cx.scope, self.decoded(name));
                let declared = self.container_names.entry(key).or_default();
                let spaces = kind.declaration_spaces();
                if export.is_some() {
                    declared.exported |= spaces;
                } else {
                    declared.local |= spaces;
                }
                declared.names.push(Declared {
                    name: token,
                    spaces,
                    exported: export.is_some(),
                    function: (kind == DeclKind::Function).then(|| field(node, "body").is_some()),
                });
            }
        }
        let anchor = match token {
            Some(token) => self.facts.names[token.index()].span,
            None => default_keyword(self.ast, node),
        };
        let name = name.map_or_else(|| "default".to_string(), |name| self.decoded(name));
        let interface_members = (kind == DeclKind::Interface
            && super::APPARENT_INTERFACES.contains(&name.as_str()))
        .then(|| Box::new(self.interface_members(node)));
        let name_reference = token.filter(|_| kind == DeclKind::Parameter).map(|token| {
            let reference = RefId::new(self.facts.refs.len());
            self.facts.refs.push(Ref {
                token,
                base: RefBase::Lexical(scope),
                meaning: meaning::VALUE,
                property: false,
                type_context: false,
            });
            reference
        });
        self.declare(Decl {
            name,
            anchor,
            name_reference,
            native_span: Some(node.span),
            flow: None,
            contains_this: None,
            kind,
            native_kind: match kind {
                _ if node.syntax_kind == SyntaxKind::BindingElement => SyntaxKind::BindingElement,
                DeclKind::Var | DeclKind::Let => SyntaxKind::VariableDeclaration,
                DeclKind::Parameter => SyntaxKind::Parameter,
                _ => node.syntax_kind,
            },
            member: None,
            lexical,
            native_export_local,
            export,
            implementation: node.syntax_kind == SyntaxKind::FunctionDeclaration
                && field(node, "body").is_some(),
            ambient: node.flags & node_flags::AMBIENT != 0
                || has_modifier(self.ast, node, SyntaxKind::DeclareKeyword),
            list: None,
            hoisted: self.hoisted(cx.scope, scope),
            type_parameters: matches!(
                kind,
                DeclKind::Class | DeclKind::Interface | DeclKind::TypeAlias
            )
            .then(|| self.type_parameter_names(node)),
            interface_members,
        })
    }

    /// The names of an interface's property and method signatures
    /// (`getPropertyNameForPropertyNameNode`): identifiers, literals, and
    /// computed literals. Any other computed name is a symbol's, which no
    /// import names, and a literal with an unpaired surrogate names nothing
    /// an import can name exactly here.
    fn interface_members(&self, node: &Node<'s>) -> InterfaceMembers {
        let names = ids(node, "members")
            .iter()
            .filter_map(|&member| {
                let member = self.ast.node(member);
                if !matches!(
                    member.syntax_kind,
                    SyntaxKind::PropertySignature | SyntaxKind::MethodSignature
                ) {
                    return None;
                }
                let mut name = field(member, "name")?;
                if self.ast.node(name).syntax_kind == SyntaxKind::ComputedPropertyName {
                    name = field(self.ast.node(name), "expression")?;
                }
                match self.ast.node(name).syntax_kind {
                    SyntaxKind::Identifier => Some(self.decoded(name)),
                    SyntaxKind::StringLiteral
                    | SyntaxKind::NoSubstitutionTemplateLiteral
                    | SyntaxKind::NumericLiteral => exact_text(self.ast, self.ast.node(name))
                        .ok()
                        .map(|text| text.to_owned()),
                    _ => None,
                }
            })
            .collect();
        InterfaceMembers {
            names,
            inherits: !ids(node, "heritageClauses").is_empty(),
        }
    }

    /// Declare a name bound only in `scope`: an expression's own name or a
    /// type parameter of `list`.
    fn declare_local(
        &mut self,
        declaration: &Node<'s>,
        name: NodeId,
        kind: DeclKind,
        scope: ScopeId,
        list: Option<TypeParameterListId>,
    ) -> DeclId {
        self.scope_sites
            .insert((declaration.syntax_kind, declaration.span), scope);
        let name_node = self.ast.node(name);
        self.scope_sites
            .insert((name_node.syntax_kind, name_node.span), scope);
        let decl = DeclId::new(self.facts.decls.len());
        let token = self.name(name, NameTarget::Decl(decl), DECLARATION, true);
        self.declare(Decl {
            name: self.decoded(name),
            native_span: Some(declaration.span),
            anchor: self.facts.names[token.index()].span,
            kind,
            native_kind: match kind {
                DeclKind::TypeParameter => SyntaxKind::TypeParameter,
                DeclKind::Class => SyntaxKind::ClassExpression,
                DeclKind::Function => SyntaxKind::FunctionExpression,
                _ => unreachable!("declare_local has a native own-name/type-parameter role"),
            },
            lexical: Some(scope),
            list,
            // `getLocalTypeParametersOfClassOrInterfaceOrTypeAlias` asks the
            // same declaration-owned list for a class expression and a class
            // declaration. An empty list is established data, not absence.
            type_parameters: (kind == DeclKind::Class)
                .then(|| self.type_parameter_names(declaration)),
            ..Decl::default()
        })
    }

    /// The blocks between `from` and the enclosing `to` that a `var` bound in
    /// `to` is hoisted out of.
    fn hoisted(&self, from: ScopeId, to: ScopeId) -> Vec<ScopeId> {
        let mut hoisted = Vec::new();
        let mut scope = Some(from);
        while let Some(current) = scope.filter(|&current| current != to) {
            hoisted.push(current);
            scope = self.facts.scopes[current.index()].parent;
        }
        hoisted
    }

    /// The name of an identifier.
    fn decoded(&self, id: NodeId) -> String {
        self.ast
            .identifier_name(id)
            .expect("an identifier has escapedText")
            .to_string()
    }

    /// Declare the identifiers of a binding name or pattern, visiting default
    /// values and computed keys in `cx`.
    fn binding(
        &mut self,
        statement: &Node<'s>,
        native_declaration: &Node<'s>,
        name: Option<NodeId>,
        kind: DeclKind,
        cx: Cx,
        lexical: ScopeId,
    ) -> Result<()> {
        self.scope_sites.insert(
            (native_declaration.syntax_kind, native_declaration.span),
            cx.scope,
        );
        let mut stack: Vec<_> = name
            .into_iter()
            .map(|id| (id, native_declaration))
            .collect();
        while let Some((id, declaration)) = stack.pop() {
            let node = self.ast.node(id);
            self.scope_sites
                .insert((node.syntax_kind, node.span), cx.scope);
            self.scope_sites
                .insert((declaration.syntax_kind, declaration.span), cx.scope);
            match node.syntax_kind {
                SyntaxKind::Identifier => {
                    let own = self.declare_statement(statement, Some(id), kind, cx, lexical);
                    // Binding retains the statement's export/modifier policy,
                    // but the symbol owns the actual Variable/BindingElement
                    // declaration, not that enclosing statement's Loc.
                    self.facts.decls[own.index()].native_span = Some(declaration.span);
                    self.facts.decls[own.index()].native_kind = declaration.syntax_kind;
                }
                SyntaxKind::ObjectBindingPattern | SyntaxKind::ArrayBindingPattern => {
                    stack.extend(
                        ids(node, "elements")
                            .iter()
                            .rev()
                            .map(|&id| (id, declaration)),
                    );
                }
                SyntaxKind::BindingElement => {
                    match child(self.ast, node, "propertyName").map(|p| p.syntax_kind) {
                        Some(SyntaxKind::ComputedPropertyName) => {
                            self.push_field(node, "propertyName", cx)
                        }
                        Some(_) => self.not_reference(field(node, "propertyName"), "property"),
                        None => {}
                    }
                    self.push_field(node, "initializer", cx);
                    if let Some(name) = field(node, "name") {
                        stack.push((name, node));
                    }
                }
                SyntaxKind::OmittedExpression => {}
                _ => return Err(self.unsupported(node, "binding name")),
            }
        }
        Ok(())
    }

    fn node(&mut self, id: NodeId, mut cx: Cx) -> Result<()> {
        let ast = self.ast;
        let node = ast.node(id);
        self.scope_sites
            .entry((node.syntax_kind, node.span))
            .or_insert(cx.scope);
        // `IsExpression`: expression subtrees use value-reference rules even
        // inside a type (computed names and declaration initializers). Entity
        // names of TypeReference/TypeQuery are captured by those branches,
        // without visiting their identifiers here, so their explicit type
        // context remains intact. Binding names likewise have their own owner.
        if crate::syntax::is_expression(ast, id) {
            cx.type_context = false;
        }
        match node.syntax_kind {
            SyntaxKind::Identifier => {
                self.reference(id, RefBase::Lexical(cx.scope), meaning::VALUE, cx, USE);
            }
            SyntaxKind::PrivateIdentifier => self.not_reference(Some(id), "private member"),

            // Statements
            SyntaxKind::Block => {
                let scope = self.native_scope(ScopeKind::Block, Some(cx.scope), node);
                self.push_list(node, "statements", cx.enter(scope));
            }
            SyntaxKind::VariableStatement => {
                self.modifiers(node, cx);
                let list = child(ast, node, "declarationList")
                    .ok_or_else(|| self.unsupported(node, "variable statement"))?;
                self.variable_list(node, list, cx)?;
            }
            SyntaxKind::VariableDeclarationList => self.variable_list(node, node, cx)?,
            SyntaxKind::FunctionDeclaration => {
                self.modifiers(node, cx);
                if field(node, "name").is_some()
                    || has_modifier(ast, node, SyntaxKind::DefaultKeyword)
                {
                    self.declare_statement(
                        node,
                        field(node, "name"),
                        DeclKind::Function,
                        cx,
                        cx.scope,
                    );
                }
                self.function(node, cx, cx.scope, true)?;
            }
            SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression => self.class(node, cx),
            SyntaxKind::InterfaceDeclaration => {
                self.modifiers(node, cx);
                let decl = self.declare_statement(
                    node,
                    field(node, "name"),
                    DeclKind::Interface,
                    cx,
                    cx.scope,
                );
                let scope = self.scope(ScopeKind::Members { decl }, Some(cx.scope));
                self.scope_sites
                    .insert((node.syntax_kind, node.span), scope);
                let inner = Cx {
                    member_owner: Some(decl),
                    ..cx.enter(scope).types()
                };
                self.type_parameters(node, inner);
                self.heritage(node, inner, false);
                self.push_list(node, "members", inner);
            }
            SyntaxKind::TypeAliasDeclaration => {
                self.modifiers(node, cx);
                self.declare_statement(
                    node,
                    field(node, "name"),
                    DeclKind::TypeAlias,
                    cx,
                    cx.scope,
                );
                let scope = self.native_scope(ScopeKind::TypeParameters, Some(cx.scope), node);
                let inner = cx.enter(scope).types();
                self.type_parameters(node, inner);
                self.push_field(node, "type", inner);
            }
            SyntaxKind::EnumDeclaration => {
                self.modifiers(node, cx);
                let constant = has_modifier(ast, node, SyntaxKind::ConstKeyword);
                let decl = self.declare_statement(
                    node,
                    field(node, "name"),
                    DeclKind::Enum { constant },
                    cx,
                    cx.scope,
                );
                let inner = Cx {
                    member_owner: Some(decl),
                    ..cx.enter(self.scope(ScopeKind::Enum { decl }, Some(cx.scope)))
                };
                for &member in ids(node, "members") {
                    self.capture_member_kind(member, inner, DeclKind::EnumMember);
                    let member = ast.node(member);
                    match child(ast, member, "name").map(|name| name.syntax_kind) {
                        Some(SyntaxKind::Identifier | SyntaxKind::PrivateIdentifier) => {
                            self.not_reference(field(member, "name"), "enum member")
                        }
                        Some(SyntaxKind::ComputedPropertyName) => {
                            self.push_field(member, "name", inner)
                        }
                        _ => {}
                    }
                    self.push_field(member, "initializer", inner);
                }
            }
            SyntaxKind::ModuleDeclaration => self.module_declaration(id, cx)?,
            SyntaxKind::ImportDeclaration => self.import_declaration(node, cx)?,
            SyntaxKind::ImportEqualsDeclaration => self.import_equals(node, cx)?,
            SyntaxKind::ExportDeclaration => self.export_declaration(node, cx)?,
            SyntaxKind::ExportAssignment => self.export_assignment(node, cx)?,
            SyntaxKind::NamespaceExportDeclaration => self.umd_global(id, cx),
            SyntaxKind::ExpressionStatement
            | SyntaxKind::ReturnStatement
            | SyntaxKind::ThrowStatement => self.push_field(node, "expression", cx),
            SyntaxKind::IfStatement => {
                self.push_field(node, "expression", cx);
                self.push_field(node, "thenStatement", cx);
                self.push_field(node, "elseStatement", cx);
            }
            SyntaxKind::DoStatement | SyntaxKind::WhileStatement => {
                self.push_field(node, "expression", cx);
                self.push_field(node, "statement", cx);
            }
            SyntaxKind::ForStatement | SyntaxKind::ForInStatement | SyntaxKind::ForOfStatement => {
                let scope = self.native_scope(ScopeKind::Block, Some(cx.scope), node);
                let inner = cx.enter(scope);
                for field in [
                    "initializer",
                    "condition",
                    "incrementor",
                    "expression",
                    "statement",
                ] {
                    self.push_field(node, field, inner);
                }
            }
            SyntaxKind::SwitchStatement => {
                self.push_field(node, "expression", cx);
                if let Some(block) = child(ast, node, "caseBlock") {
                    let inner =
                        cx.enter(self.native_scope(ScopeKind::Block, Some(cx.scope), block));
                    for &clause in ids(block, "clauses") {
                        let clause = ast.node(clause);
                        self.push_field(clause, "expression", inner);
                        self.push_list(clause, "statements", inner);
                    }
                }
            }
            SyntaxKind::LabeledStatement => {
                self.not_reference(field(node, "label"), "label");
                // A labeled declaration is still a statement of its container.
                let statement = field(node, "statement");
                self.push_statements(statement.as_slice(), cx);
            }
            SyntaxKind::BreakStatement | SyntaxKind::ContinueStatement => {
                self.not_reference(field(node, "label"), "label")
            }
            SyntaxKind::TryStatement => {
                self.push_field(node, "tryBlock", cx);
                if let Some(clause) = child(ast, node, "catchClause") {
                    let scope = self.native_scope(ScopeKind::Catch, Some(cx.scope), clause);
                    let catch = cx.enter(scope);
                    if let Some(declaration) = child(ast, clause, "variableDeclaration") {
                        self.binding(
                            declaration,
                            declaration,
                            field(declaration, "name"),
                            DeclKind::Let,
                            catch,
                            scope,
                        )?;
                        self.push_field(declaration, "type", catch.types());
                    }
                    // The block is a scope of its own, so the catch scope
                    // holds only the binding a `var` may redeclare.
                    self.push_field(clause, "block", catch);
                }
                self.push_field(node, "finallyBlock", cx);
            }
            SyntaxKind::EmptyStatement | SyntaxKind::DebuggerStatement => {}
            SyntaxKind::WithStatement => return Err(self.unsupported(node, "with statement")),

            // Expressions
            // `a.b.c` resolves `b` and `c` through namespaces when `a` is one.
            SyntaxKind::PropertyAccessExpression => {
                self.entity(Some(id), Head::Lexical, Path::VALUE, cx, USE);
            }
            SyntaxKind::ElementAccessExpression => {
                self.push_field(node, "expression", cx);
                self.push_field(node, "argumentExpression", cx);
            }
            SyntaxKind::CallExpression | SyntaxKind::NewExpression => {
                if child(ast, node, "expression")
                    .is_some_and(|callee| callee.syntax_kind == SyntaxKind::ImportKeyword)
                {
                    if let Some(&argument) = ids(node, "arguments").first() {
                        let argument = ast.node(argument);
                        if matches!(
                            argument.syntax_kind,
                            SyntaxKind::StringLiteral | SyntaxKind::NoSubstitutionTemplateLiteral
                        ) {
                            self.request(argument, RequestKind::Dynamic, node)?;
                        }
                    }
                } else {
                    self.push_field(node, "expression", cx);
                }
                self.push_list(node, "typeArguments", cx.types());
                self.push_list(node, "arguments", cx);
            }
            SyntaxKind::TaggedTemplateExpression => {
                self.push_field(node, "tag", cx);
                self.push_list(node, "typeArguments", cx.types());
                self.push_field(node, "template", cx);
            }
            SyntaxKind::TemplateExpression | SyntaxKind::TemplateLiteralType => {
                let inner = if node.syntax_kind == SyntaxKind::TemplateLiteralType {
                    cx.types()
                } else {
                    cx
                };
                for &span in ids(node, "templateSpans") {
                    self.push(field(ast.node(span), "expression"), inner);
                    self.push(field(ast.node(span), "type"), inner);
                }
            }
            SyntaxKind::ParenthesizedExpression
            | SyntaxKind::AwaitExpression
            | SyntaxKind::YieldExpression
            | SyntaxKind::TypeOfExpression
            | SyntaxKind::VoidExpression
            | SyntaxKind::DeleteExpression
            | SyntaxKind::SpreadElement
            | SyntaxKind::SpreadAssignment
            | SyntaxKind::NonNullExpression
            | SyntaxKind::JsxSpreadAttribute
            | SyntaxKind::JsxExpression
            | SyntaxKind::ComputedPropertyName
            | SyntaxKind::Decorator
            | SyntaxKind::TemplateSpan => self.push_field(node, "expression", cx),
            SyntaxKind::PrefixUnaryExpression | SyntaxKind::PostfixUnaryExpression => {
                self.push_field(node, "operand", cx)
            }
            SyntaxKind::BinaryExpression => {
                self.push_field(node, "left", cx);
                self.push_field(node, "right", cx);
            }
            SyntaxKind::ConditionalExpression => {
                for field in ["condition", "whenTrue", "whenFalse"] {
                    self.push_field(node, field, cx);
                }
            }
            SyntaxKind::AsExpression
            | SyntaxKind::SatisfiesExpression
            | SyntaxKind::TypeAssertionExpression => {
                self.push_field(node, "expression", cx);
                // `as const` spells the keyword as a type reference.
                let constant = crate::syntax::id(node, "type")
                    .filter(|_| crate::syntax::is_const_assertion(ast, id))
                    .and_then(|ty| crate::syntax::id(ast.node(ty), "typeName"));
                match constant {
                    Some(name) => self.not_reference(Some(name), "const assertion"),
                    None => self.push_field(node, "type", cx.types()),
                }
            }
            SyntaxKind::ExpressionWithTypeArguments => {
                self.push_field(node, "expression", cx);
                self.push_list(node, "typeArguments", cx.types());
            }
            SyntaxKind::ArrayLiteralExpression => self.push_list(node, "elements", cx),
            SyntaxKind::ObjectLiteralExpression => {
                let declaration = self.declare(Decl {
                    anchor: declaration_anchor(ast, id),
                    native_span: Some(node.span),
                    kind: DeclKind::ObjectLiteral,
                    native_kind: SyntaxKind::ObjectLiteralExpression,
                    ..Decl::default()
                });
                self.push_list(
                    node,
                    "properties",
                    Cx {
                        member_owner: Some(declaration),
                        ..cx
                    },
                );
            }
            SyntaxKind::PropertyAssignment => {
                self.capture_member(id, cx, MemberKind::Property);
                self.member_name(node, cx);
                self.push_field(node, "type", cx.types());
                self.push_field(node, "initializer", cx);
            }
            SyntaxKind::ShorthandPropertyAssignment => {
                self.capture_member(id, cx, MemberKind::Property);
                let name = field(node, "name").unwrap();
                self.reference(name, RefBase::Lexical(cx.scope), meaning::VALUE, cx, USE);
                self.push_field(node, "type", cx.types());
                self.push_field(node, "objectAssignmentInitializer", cx);
            }
            SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction => {
                let mut scope = cx.scope;
                if let Some(name) = field(node, "name") {
                    scope = self.scope(ScopeKind::ExpressionName, Some(cx.scope));
                    self.declare_local(node, name, DeclKind::Function, scope, None);
                } else {
                    // `bindFunctionExpression` binds one anonymous Function
                    // symbol even for an arrow or nameless expression.
                    self.declare(Decl {
                        anchor: declaration_anchor(ast, id),
                        native_span: Some(node.span),
                        kind: DeclKind::Function,
                        native_kind: node.syntax_kind,
                        ..Decl::default()
                    });
                }
                self.modifiers(node, cx);
                self.function(
                    node,
                    cx,
                    scope,
                    node.syntax_kind == SyntaxKind::FunctionExpression,
                )?;
            }
            SyntaxKind::MetaProperty => self.not_reference(field(node, "name"), "meta property"),
            SyntaxKind::OmittedExpression
            | SyntaxKind::ThisKeyword
            | SyntaxKind::SuperKeyword
            | SyntaxKind::NullKeyword
            | SyntaxKind::TrueKeyword
            | SyntaxKind::FalseKeyword
            | SyntaxKind::ImportKeyword
            | SyntaxKind::NumericLiteral
            | SyntaxKind::BigIntLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::RegularExpressionLiteral
            | SyntaxKind::NoSubstitutionTemplateLiteral
            | SyntaxKind::TemplateHead
            | SyntaxKind::TemplateMiddle
            | SyntaxKind::TemplateTail
            | SyntaxKind::JsxText
            | SyntaxKind::JsxOpeningFragment
            | SyntaxKind::JsxClosingFragment => {}

            // JSX
            SyntaxKind::JsxElement => {
                self.push_field(node, "openingElement", cx);
                self.push_list(node, "children", cx);
                self.push_field(node, "closingElement", cx);
            }
            SyntaxKind::JsxFragment => self.push_list(node, "children", cx),
            SyntaxKind::JsxSelfClosingElement
            | SyntaxKind::JsxOpeningElement
            | SyntaxKind::JsxClosingElement => {
                self.jsx_tag(node, cx);
                self.push_list(node, "typeArguments", cx.types());
                self.push_field(node, "attributes", cx);
            }
            SyntaxKind::JsxAttributes => self.push_list(node, "properties", cx),
            SyntaxKind::JsxAttribute => {
                self.jsx_name(field(node, "name"));
                self.push_field(node, "initializer", cx);
            }

            // Class and object members
            SyntaxKind::PropertyDeclaration | SyntaxKind::PropertySignature => {
                self.capture_member(
                    id,
                    cx,
                    if has_modifier(ast, node, SyntaxKind::AccessorKeyword) {
                        MemberKind::AutoAccessor
                    } else {
                        MemberKind::Property
                    },
                );
                self.modifiers(node, cx);
                self.member_name(node, cx);
                self.push_field(node, "type", cx.types());
                self.push_field(node, "initializer", cx);
            }
            SyntaxKind::MethodDeclaration
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
            | SyntaxKind::Constructor
            | SyntaxKind::MethodSignature => {
                self.capture_member(
                    id,
                    cx,
                    match node.syntax_kind {
                        SyntaxKind::GetAccessor => MemberKind::Getter,
                        SyntaxKind::SetAccessor => MemberKind::Setter,
                        SyntaxKind::Constructor => MemberKind::Constructor,
                        _ => MemberKind::Method,
                    },
                );
                self.modifiers(node, cx);
                self.member_name(node, cx);
                self.function(
                    node,
                    cx,
                    cx.scope,
                    node.syntax_kind != SyntaxKind::MethodSignature,
                )?;
            }
            SyntaxKind::CallSignature
            | SyntaxKind::ConstructSignature
            | SyntaxKind::IndexSignature
            | SyntaxKind::FunctionType
            | SyntaxKind::ConstructorType => {
                if matches!(
                    node.syntax_kind,
                    SyntaxKind::FunctionType | SyntaxKind::ConstructorType
                ) {
                    let owner = self.declare(Decl {
                        name: "__type".into(),
                        native_span: Some(node.span),
                        anchor: declaration_anchor(ast, id),
                        kind: DeclKind::TypeLiteral,
                        native_kind: node.syntax_kind,
                        ..Decl::default()
                    });
                    let signature = self.declare(Decl {
                        native_span: Some(node.span),
                        anchor: declaration_anchor(ast, id),
                        kind: DeclKind::Member(if node.syntax_kind == SyntaxKind::FunctionType {
                            MemberKind::Call
                        } else {
                            MemberKind::Construct
                        }),
                        native_kind: node.syntax_kind,
                        member: Some(Box::new(MemberBinding {
                            owner,
                            private_class: None,
                            static_member: false,
                            key: declaration_key(ast, node),
                            optional: false,
                        })),
                        ..Decl::default()
                    });
                    self.facts.anonymous_signatures.insert(owner, signature);
                }
                if let Some(kind) = match node.syntax_kind {
                    SyntaxKind::CallSignature => Some(MemberKind::Call),
                    SyntaxKind::ConstructSignature => Some(MemberKind::Construct),
                    SyntaxKind::IndexSignature => Some(MemberKind::Index),
                    _ => None,
                } {
                    self.capture_member(id, cx, kind);
                }
                self.modifiers(node, cx);
                self.function(node, cx.types(), cx.scope, false)?;
            }
            SyntaxKind::ClassStaticBlockDeclaration => {
                self.modifiers(node, cx);
                let scope = self.native_scope(ScopeKind::Body, Some(cx.scope), node);
                let inner = Cx {
                    var_scope: scope,
                    ..cx.enter(scope)
                };
                if let Some(body) = child(ast, node, "body") {
                    self.push_list(body, "statements", inner);
                }
            }
            SyntaxKind::SemicolonClassElement => {}

            // Types
            SyntaxKind::TypeReference => {
                let name = field(node, "typeName");
                self.entity(name, Head::Lexical, Path::TYPE, cx.types(), USE);
                self.push_list(node, "typeArguments", cx.types());
            }
            SyntaxKind::TypeQuery => {
                let name = field(node, "exprName");
                self.entity(name, Head::Lexical, Path::VALUE, cx.types(), USE);
                self.push_list(node, "typeArguments", cx.types());
            }
            SyntaxKind::ImportType => self.import_type(node, cx.types())?,
            SyntaxKind::TypePredicate => {
                if let Some(name) = field(node, "parameterName")
                    .filter(|&name| ast.node(name).syntax_kind == SyntaxKind::Identifier)
                {
                    self.reference(name, RefBase::Lexical(cx.scope), meaning::VALUE, cx, USE);
                }
                self.push_field(node, "type", cx.types());
            }
            SyntaxKind::TypeLiteral => {
                let declaration = self.declare(Decl {
                    name: "__type".into(),
                    native_span: Some(node.span),
                    anchor: declaration_anchor(ast, id),
                    kind: DeclKind::TypeLiteral,
                    native_kind: SyntaxKind::TypeLiteral,
                    ..Decl::default()
                });
                let scope = self.scope(ScopeKind::Members { decl: declaration }, Some(cx.scope));
                self.push_list(
                    node,
                    "members",
                    Cx {
                        member_owner: Some(declaration),
                        ..cx.enter(scope).types()
                    },
                );
            }
            SyntaxKind::NamedTupleMember => {
                self.not_reference(field(node, "name"), "tuple member label");
                self.push_field(node, "type", cx.types());
            }
            SyntaxKind::MappedType => {
                // bindAnonymousDeclaration(TypeLiteral): the mapped object owns
                // an actual symbol even though its members are checker-created.
                self.declare(Decl {
                    name: "__type".into(),
                    native_span: Some(node.span),
                    anchor: declaration_anchor(ast, id),
                    kind: DeclKind::TypeLiteral,
                    native_kind: SyntaxKind::MappedType,
                    ..Decl::default()
                });
                let scope = self.native_scope(ScopeKind::TypeParameters, Some(cx.scope), node);
                let inner = cx.enter(scope).types();
                if let Some(parameter) = field(node, "typeParameter") {
                    self.lists += 1;
                    self.type_parameter(parameter, inner, scope);
                }
                self.push_field(node, "nameType", inner);
                self.push_field(node, "type", inner);
                self.push_list(node, "members", inner);
            }
            SyntaxKind::ConditionalType => {
                let types = cx.types();
                let infer = self.native_scope(ScopeKind::Infer, Some(cx.scope), node);
                self.facts.conditional_scopes.insert(node.span, infer);
                self.push_field(node, "checkType", types);
                self.push_field(
                    node,
                    "extendsType",
                    Cx {
                        infer: Some(infer),
                        ..types
                    },
                );
                self.push_field(
                    node,
                    "trueType",
                    Cx {
                        scope: infer,
                        ..types
                    },
                );
                self.push_field(node, "falseType", types);
            }
            SyntaxKind::InferType => {
                // TypeScript binds a misplaced `infer` as an anonymous type
                // parameter that no lookup can see.
                let scope = match cx.infer {
                    Some(scope) => scope,
                    None => self.scope(ScopeKind::TypeParameters, Some(cx.scope)),
                };
                if let Some(parameter) = field(node, "typeParameter") {
                    // Each `infer` may repeat a name of its conditional type.
                    self.lists += 1;
                    let decl = DeclId::new(self.facts.decls.len());
                    let constraint =
                        self.scope(ScopeKind::InferConstraint { decl }, Some(cx.scope));
                    self.scope_sites
                        .insert((node.syntax_kind, node.span), constraint);
                    self.type_parameter_in(parameter, cx.types(), scope, constraint);
                }
            }
            SyntaxKind::ArrayType
            | SyntaxKind::OptionalType
            | SyntaxKind::RestType
            | SyntaxKind::ParenthesizedType
            | SyntaxKind::TypeOperator
            | SyntaxKind::JSDocNullableType
            | SyntaxKind::JSDocNonNullableType => {
                self.push_field(node, "elementType", cx.types());
                self.push_field(node, "type", cx.types());
            }
            SyntaxKind::TupleType => self.push_list(node, "elements", cx.types()),
            SyntaxKind::UnionType | SyntaxKind::IntersectionType => {
                self.push_list(node, "types", cx.types())
            }
            SyntaxKind::IndexedAccessType => {
                self.push_field(node, "objectType", cx.types());
                self.push_field(node, "indexType", cx.types());
            }
            SyntaxKind::LiteralType => self.push_field(node, "literal", cx),
            SyntaxKind::ThisType | SyntaxKind::JSDocAllType => {}
            kind if is_token(kind) => {}
            _ => return Err(self.unsupported(node, "syntax")),
        }
        Ok(())
    }

    /// Visit decorators in `scope`: where a class is declared, or the class
    /// body for members and parameters.
    fn modifiers(&mut self, node: &Node<'s>, outer: Cx) {
        let scope = outer.scope;
        let decorators: Vec<NodeId> = ids(node, "modifiers")
            .iter()
            .copied()
            .filter(|&id| self.ast.node(id).syntax_kind == SyntaxKind::Decorator)
            .collect();
        let cx = Cx {
            scope,
            var_scope: scope,
            container: None,
            infer: None,
            type_context: false,
            member_owner: None,
            private_class: outer.private_class,
        };
        self.push_all(&decorators, cx);
    }

    fn variable_list(&mut self, statement: &Node<'s>, list: &Node<'s>, cx: Cx) -> Result<()> {
        let block = list.flags & (node_flags::LET | node_flags::CONST | node_flags::USING) != 0;
        let (kind, lexical) = if block {
            (DeclKind::Let, cx.scope)
        } else {
            (DeclKind::Var, cx.var_scope)
        };
        for &declaration in ids(list, "declarations") {
            let declaration = self.ast.node(declaration);
            let name = field(declaration, "name");
            self.binding(statement, declaration, name, kind, cx, lexical)?;
            self.push_field(declaration, "type", cx.types());
            self.push_field(declaration, "initializer", cx);
        }
        Ok(())
    }

    /// Parameters, type parameters, return type, and body of a function-like
    /// node. `outer` is the scope enclosing its signature.
    fn function(&mut self, node: &Node<'s>, cx: Cx, outer: ScopeId, arguments: bool) -> Result<()> {
        let signature = self.native_scope(
            ScopeKind::Signature {
                arguments: arguments && node.syntax_kind != SyntaxKind::ArrowFunction,
            },
            Some(outer),
            node,
        );
        let inner = cx.enter(signature);
        self.type_parameters(node, inner.types());
        for (index, &parameter_id) in ids(node, "parameters").iter().enumerate() {
            let parameter = self.ast.node(parameter_id);
            self.modifiers(parameter, Cx { scope: outer, ..cx });
            let name = field(parameter, "name");
            let this = name.is_some_and(|name| {
                let name = self.ast.node(name);
                name.syntax_kind == SyntaxKind::Identifier && is_text(self.ast, name, "this")
            });
            if this {
                // `bindParameter` also declares the explicit-this parameter.
                // Its token remains excluded from lexical navigation.
                self.scope_sites
                    .insert((parameter.syntax_kind, parameter.span), signature);
                let name_node = self.ast.node(name.expect("explicit this has a name"));
                self.scope_sites
                    .insert((name_node.syntax_kind, name_node.span), signature);
                self.declare(Decl {
                    name: "this".into(),
                    anchor: self.ast.token_span(name.expect("explicit this has a name")),
                    native_span: Some(parameter.span),
                    kind: DeclKind::Parameter,
                    native_kind: SyntaxKind::Parameter,
                    lexical: Some(signature),
                    ..Decl::default()
                });
                self.not_reference(name, "this parameter");
            } else {
                // `bindParameter` gives a binding-pattern parameter its own
                // anonymous symbol before binding the named elements. It is
                // neither a lexical entry nor an extra name occurrence.
                if name.is_some_and(|name| {
                    matches!(
                        self.ast.node(name).syntax_kind,
                        SyntaxKind::ObjectBindingPattern | SyntaxKind::ArrayBindingPattern
                    )
                }) {
                    self.declare(Decl {
                        name: format!("__{index}"),
                        anchor: declaration_anchor(self.ast, parameter_id),
                        native_span: Some(parameter.span),
                        kind: DeclKind::Parameter,
                        native_kind: SyntaxKind::Parameter,
                        ..Decl::default()
                    });
                }
                self.binding(
                    parameter,
                    parameter,
                    name,
                    DeclKind::Parameter,
                    inner,
                    signature,
                )?;
                if name
                    .is_some_and(|name| self.ast.node(name).syntax_kind == SyntaxKind::Identifier)
                    && crate::syntax::is_parameter_property_declaration(self.ast, parameter, node)
                {
                    self.capture_member(parameter_id, cx, MemberKind::ParameterProperty);
                }
            }
            self.push_field(parameter, "type", inner.types());
            self.push_field(parameter, "initializer", inner);
        }
        self.push_field(node, "type", inner.types());
        if let Some(body) = child(self.ast, node, "body") {
            let scope = self.native_scope(ScopeKind::Body, Some(signature), node);
            let body_cx = Cx {
                var_scope: scope,
                type_context: false,
                ..cx.enter(scope)
            };
            if body.syntax_kind == SyntaxKind::Block {
                self.scope_sites
                    .insert((body.syntax_kind, body.span), scope);
                self.push_list(body, "statements", body_cx);
            } else {
                self.push(field(node, "body"), body_cx);
            }
        }
        Ok(())
    }

    fn type_parameters(&mut self, node: &Node<'s>, cx: Cx) {
        self.lists += 1;
        for &parameter in ids(node, "typeParameters") {
            self.type_parameter(parameter, cx, cx.scope);
        }
    }

    fn type_parameter_names(&self, node: &Node<'s>) -> Vec<(String, bool, Span)> {
        ids(node, "typeParameters")
            .iter()
            .map(|&parameter| self.ast.node(parameter))
            .filter_map(|parameter| {
                let name = field(parameter, "name")?;
                Some((
                    self.decoded(name),
                    field(parameter, "default").is_some(),
                    self.ast.token_span(name),
                ))
            })
            .collect()
    }

    /// Declare a type parameter of the current list (see `Decl::list`).
    fn type_parameter(&mut self, id: NodeId, cx: Cx, scope: ScopeId) {
        self.type_parameter_in(id, cx, scope, scope);
    }

    /// Declare a type parameter in `scope`, visiting its constraint and
    /// default in `constraints`.
    fn type_parameter_in(&mut self, id: NodeId, cx: Cx, scope: ScopeId, constraints: ScopeId) {
        let node = self.ast.node(id);
        if let Some(name) = field(node, "name") {
            let list = Some(TypeParameterListId(self.lists));
            self.declare_local(node, name, DeclKind::TypeParameter, scope, list);
            // `NameResolver.Resolve` reaches an infer's own parameter before
            // rejecting the conditional's locals outside its true branch.
            // Binding ownership remains the shared infer declaration table.
            self.scope_sites
                .insert((node.syntax_kind, node.span), constraints);
            let name_node = self.ast.node(name);
            self.scope_sites
                .insert((name_node.syntax_kind, name_node.span), constraints);
        }
        // Constraints see the parameters of their own list.
        let inner = cx.enter(constraints).types();
        self.push_field(node, "constraint", inner);
        self.push_field(node, "default", inner);
    }

    fn class(&mut self, node: &Node<'s>, cx: Cx) {
        let name = field(node, "name");
        let mut outer = cx.scope;
        let decl = if node.syntax_kind == SyntaxKind::ClassDeclaration
            && (name.is_some() || has_modifier(self.ast, node, SyntaxKind::DefaultKeyword))
        {
            self.declare_statement(node, name, DeclKind::Class, cx, cx.scope)
        } else if let Some(name) = name {
            outer = self.scope(ScopeKind::ExpressionName, Some(cx.scope));
            self.declare_local(node, name, DeclKind::Class, outer, None)
        } else {
            // `bindClassLikeDeclaration` gives an unnamed class expression
            // its own Class header before binding private names and members.
            self.declare(Decl {
                native_span: Some(node.span),
                anchor: node.span,
                kind: DeclKind::Class,
                native_kind: node.syntax_kind,
                type_parameters: Some(self.type_parameter_names(node)),
                ..Decl::default()
            })
        };
        self.modifiers(
            node,
            Cx {
                private_class: Some(decl),
                ..cx
            },
        );
        let scope = self.scope(ScopeKind::Members { decl }, Some(outer));
        self.scope_sites
            .insert((node.syntax_kind, node.span), scope);
        let inner = Cx {
            type_context: false,
            member_owner: Some(decl),
            private_class: Some(decl),
            ..cx.enter(scope)
        };
        self.type_parameters(node, inner.types());
        self.heritage(node, inner, true);
        self.push_list(node, "members", inner);
    }

    /// `extends` of a class is an expression; `implements` and interface
    /// `extends` name types.
    fn heritage(&mut self, node: &Node<'s>, cx: Cx, class: bool) {
        for &clause in ids(node, "heritageClauses") {
            let clause = self.ast.node(clause);
            let value =
                class && number(clause, "token") == Some(SyntaxKind::ExtendsKeyword.number());
            for &ty in ids(clause, "types") {
                let ty = self.ast.node(ty);
                if value {
                    self.push_field(ty, "expression", cx);
                } else {
                    let name = field(ty, "expression");
                    self.entity(name, Head::Lexical, Path::TYPE, cx.types(), USE);
                }
                self.push_list(ty, "typeArguments", cx.types());
            }
        }
    }

    /// A member name is not a lexical reference; a computed key is evaluated.
    /// `bindPropertyOrMethodOrAccessor`: capture this native declaring
    /// container independently of lexical scope. A container without a
    /// retained native symbol never attaches its members to an outer owner.
    fn capture_member(&mut self, id: NodeId, cx: Cx, kind: MemberKind) {
        self.capture_member_kind(id, cx, DeclKind::Member(kind));
    }

    fn capture_member_kind(&mut self, id: NodeId, cx: Cx, kind: DeclKind) {
        let Some(owner) = cx.member_owner else {
            return;
        };
        let node = self.ast.node(id);
        let key = declaration_key(self.ast, node);
        let name = match &key {
            super::member::MemberKey::Text(name) | super::member::MemberKey::Private(name) => {
                name.clone()
            }
            _ => String::new(),
        };
        self.declare(Decl {
            name,
            native_span: Some(node.span),
            anchor: declaration_anchor(self.ast, id),
            kind,
            native_kind: node.syntax_kind,
            member: Some(Box::new(MemberBinding {
                owner,
                private_class: matches!(key, super::member::MemberKey::Private(_))
                    .then_some(cx.private_class)
                    .flatten(),
                static_member: kind == DeclKind::EnumMember
                    || has_modifier(self.ast, node, SyntaxKind::StaticKeyword),
                key,
                optional: field(node, "questionToken").is_some(),
            })),
            ..Decl::default()
        });
    }

    /// A member name is not a lexical reference; a computed key is evaluated.
    fn member_name(&mut self, node: &Node<'s>, cx: Cx) {
        match child(self.ast, node, "name").map(|name| name.syntax_kind) {
            Some(SyntaxKind::ComputedPropertyName) => self.push_field(node, "name", cx),
            Some(SyntaxKind::Identifier) | Some(SyntaxKind::PrivateIdentifier) => {
                self.not_reference(field(node, "name"), "member")
            }
            _ => {}
        }
    }

    /// An identifier, qualified name, or property-access chain, with the
    /// meanings `path` asks of each part and its first part looked up in
    /// `head`. Returns the last part's reference.
    fn entity(
        &mut self,
        id: Option<NodeId>,
        head: Head,
        path: Path,
        cx: Cx,
        roles: u8,
    ) -> Option<RefId> {
        let (first, parts) = entity_parts(self.ast, id?);
        if self.ast.node(first).syntax_kind != SyntaxKind::Identifier {
            // `this.x`, calls, and other non-entity heads.
            self.push(Some(first), cx);
            for &(part, _) in &parts {
                self.not_reference(Some(part), "property");
            }
            return None;
        }
        if matches!(head, Head::Lexical) && self.decoded(first) == "this" {
            // `typeof this.x` names the enclosing class's instance.
            self.not_reference(Some(first), "this");
            for &(part, _) in &parts {
                self.not_reference(Some(part), "property");
            }
            return None;
        }
        let base = match head {
            Head::Lexical => RefBase::Lexical(cx.scope),
            Head::Module(request) => RefBase::Module(request),
        };
        let meaning = if parts.is_empty() {
            path.single
        } else {
            path.head
        };
        let mut reference = self.reference(first, base, meaning, cx, roles);
        for (index, &(part, property)) in parts.iter().enumerate() {
            if self.ast.node(part).syntax_kind == SyntaxKind::PrivateIdentifier {
                for &(rest, _) in &parts[index..] {
                    self.not_reference(Some(rest), "private member");
                }
                return None;
            }
            let meaning = if index + 1 == parts.len() {
                path.last
            } else {
                path.middle
            };
            reference = self.reference(part, RefBase::Member(reference), meaning, cx, roles);
            self.facts.refs[reference.index()].property = property || path.properties;
        }
        Some(reference)
    }

    fn jsx_tag(&mut self, node: &Node<'s>, cx: Cx) {
        let Some(tag) = field(node, "tagName") else {
            return;
        };
        let tag_node = self.ast.node(tag);
        match tag_node.syntax_kind {
            SyntaxKind::Identifier => {
                let name = self.decoded(tag);
                let intrinsic =
                    name.starts_with(|c: char| c.is_ascii_lowercase()) || name.contains('-');
                if intrinsic {
                    self.not_reference(Some(tag), "intrinsic element");
                } else {
                    self.reference(tag, RefBase::Lexical(cx.scope), meaning::VALUE, cx, USE);
                }
            }
            SyntaxKind::JsxNamespacedName => self.jsx_name(Some(tag)),
            _ => self.push(Some(tag), cx),
        }
    }

    fn jsx_name(&mut self, id: Option<NodeId>) {
        let Some(id) = id else { return };
        let node = self.ast.node(id);
        if node.syntax_kind == SyntaxKind::JsxNamespacedName {
            self.not_reference(field(node, "namespace"), "JSX namespace");
            self.not_reference(field(node, "name"), "JSX attribute");
        } else {
            self.not_reference(Some(id), "JSX attribute");
        }
    }

    fn import_type(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        let literal = child(self.ast, node, "argument")
            .and_then(|argument| child(self.ast, argument, "literal"))
            .filter(|literal| literal.syntax_kind == SyntaxKind::StringLiteral);
        let request = literal
            .map(|literal| self.request(literal, RequestKind::ImportType, node))
            .transpose()?;
        if literal.is_none() {
            self.push_field(node, "argument", cx);
        }
        self.import_attributes(node, cx);
        let qualifier = field(node, "qualifier");
        match (request, qualifier) {
            (Some(request), _) => {
                let path = if flag(node, "isTypeOf") {
                    Path::IMPORT_TYPEOF
                } else {
                    Path::TYPE
                };
                self.entity(qualifier, Head::Module(request), path, cx, USE);
            }
            // Without a module, the qualifier names nothing to resolve.
            (None, Some(qualifier)) => {
                let (first, parts) = entity_parts(self.ast, qualifier);
                self.not_reference(Some(first), "property");
                for (part, _) in parts {
                    self.not_reference(Some(part), "property");
                }
            }
            (None, None) => {}
        }
        self.push_list(node, "typeArguments", cx);
        Ok(())
    }

    fn import_attributes(&mut self, node: &Node<'s>, cx: Cx) {
        if let Some(attributes) = child(self.ast, node, "attributes") {
            for &attribute in ids(attributes, "elements") {
                let attribute = self.ast.node(attribute);
                self.not_reference(field(attribute, "name"), "import attribute");
                self.push(field(attribute, "value"), cx);
            }
        }
    }

    /// Record an alias. `anchor` locates it in diagnostics: its own name
    /// token, or the keyword of a nameless export.
    fn alias(
        &mut self,
        kind: AliasKind,
        bind: AliasBind,
        type_only: bool,
        anchor: Span,
        declaration: &Node<'s>,
    ) -> AliasId {
        self.add_alias(kind, bind, type_only, anchor, false, declaration)
    }

    /// Record an export specifier, `export * as`, or export assignment alias,
    /// which is not a lexical name even inside the namespace whose exports
    /// hold it.
    fn export_alias(
        &mut self,
        kind: AliasKind,
        bind: AliasBind,
        type_only: bool,
        anchor: Span,
        declaration: &Node<'s>,
    ) -> AliasId {
        self.add_alias(kind, bind, type_only, anchor, true, declaration)
    }

    fn add_alias(
        &mut self,
        kind: AliasKind,
        bind: AliasBind,
        type_only: bool,
        span: Span,
        export_only: bool,
        declaration: &Node<'s>,
    ) -> AliasId {
        // `GetErrorRangeForNode`'s complete alias-declaration branch: only
        // NamespaceImport selects its Name; all other alias declarations use
        // their own non-trivia range (including ImportEquals' `import`).
        let conflict_node = if declaration.syntax_kind == SyntaxKind::NamespaceImport {
            child(self.ast, declaration, "name").expect("namespace import alias has a name")
        } else {
            declaration
        };
        let conflict_span = self.ast.token_span_of(conflict_node);
        self.facts.aliases.push(Alias {
            kind,
            native_kind: declaration.syntax_kind,
            native_span: declaration.span,
            target_name: match declaration.syntax_kind {
                SyntaxKind::ExportAssignment => child(self.ast, declaration, "expression"),
                SyntaxKind::ImportEqualsDeclaration => {
                    child(self.ast, declaration, "moduleReference")
                }
                _ => child(self.ast, declaration, "propertyName")
                    .or_else(|| child(self.ast, declaration, "name")),
            }
            .map(|name| (name.syntax_kind, self.ast.token_span_of(name))),
            bind,
            type_only,
            span,
            conflict_span,
            export_only,
            flow: None,
        });
        let id = AliasId::new(self.facts.aliases.len() - 1);
        self.facts.binding_order.push(BindingEntry::Alias(id));
        id
    }

    /// Record a name token of an alias; a string-literal name is no token.
    fn alias_name(&mut self, id: Option<NodeId>, alias: AliasId, roles: u8, candidate: bool) {
        let Some(id) = id else { return };
        if self.ast.node(id).syntax_kind == SyntaxKind::Identifier {
            self.name(id, NameTarget::Alias(alias), roles, candidate);
        }
    }

    fn import_declaration(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        self.modifiers(node, cx);
        let specifier = self.module_specifier(node)?;
        let clause = child(self.ast, node, "importClause");
        let request = self.request(
            specifier,
            RequestKind::Import {
                side_effect: clause.is_none(),
            },
            node,
        )?;
        self.import_attributes(node, cx);
        let Some(clause) = clause else { return Ok(()) };
        let type_only = flag(clause, "isTypeOnly");
        if let Some(name) = field(clause, "name") {
            let alias = self.alias(
                AliasKind::Import {
                    request,
                    name: "default".into(),
                },
                AliasBind::Lexical {
                    scope: cx.scope,
                    name: self.decoded(name),
                },
                type_only,
                self.token(name),
                clause,
            );
            self.alias_name(Some(name), alias, IMPORT, true);
        }
        let Some(bindings) = child(self.ast, clause, "namedBindings") else {
            return Ok(());
        };
        if bindings.syntax_kind == SyntaxKind::NamespaceImport {
            let name = field(bindings, "name").unwrap();
            let alias = self.alias(
                AliasKind::Namespace { request },
                AliasBind::Lexical {
                    scope: cx.scope,
                    name: self.decoded(name),
                },
                type_only,
                self.token(name),
                bindings,
            );
            self.alias_name(Some(name), alias, IMPORT, true);
            return Ok(());
        }
        for &element in ids(bindings, "elements") {
            let element = self.ast.node(element);
            let name = field(element, "name").unwrap();
            let property = field(element, "propertyName");
            let imported = property.unwrap_or(name);
            let alias = self.alias(
                AliasKind::Import {
                    request,
                    name: self.export_name(imported)?,
                },
                AliasBind::Lexical {
                    scope: cx.scope,
                    name: self.decoded(name),
                },
                type_only || flag(element, "isTypeOnly"),
                self.token(name),
                element,
            );
            self.alias_name(property, alias, IMPORT, false);
            self.alias_name(Some(name), alias, IMPORT, true);
        }
        Ok(())
    }

    /// A string-literal module specifier; other specifiers are invalid syntax
    /// that TypeScript only recovers from.
    fn module_specifier(&self, node: &Node<'s>) -> Result<&'a Node<'s>> {
        child(self.ast, node, "moduleSpecifier")
            .filter(|specifier| specifier.syntax_kind == SyntaxKind::StringLiteral)
            .ok_or_else(|| self.unsupported(node, "non-string module specifier"))
    }

    /// The decoded text of an identifier or string-literal module export name.
    fn export_name(&self, id: NodeId) -> Result<String> {
        let node = self.ast.node(id);
        if node.syntax_kind == SyntaxKind::Identifier {
            Ok(self.decoded(id))
        } else {
            self.semantic_name(node)
        }
    }

    fn import_equals(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        self.modifiers(node, cx);
        let name = field(node, "name").unwrap();
        let type_only = flag(node, "isTypeOnly");
        let reference = child(self.ast, node, "moduleReference")
            .ok_or_else(|| self.unsupported(node, "import equals declaration"))?;
        let kind = if reference.syntax_kind == SyntaxKind::ExternalModuleReference {
            let literal = child(self.ast, reference, "expression")
                .filter(|literal| literal.syntax_kind == SyntaxKind::StringLiteral)
                .ok_or_else(|| self.unsupported(node, "require target"))?;
            AliasKind::Namespace {
                request: self.request(literal, RequestKind::Require, node)?,
            }
        } else {
            let target = field(node, "moduleReference");
            let reference = self
                .entity(target, Head::Lexical, Path::IMPORT, cx, USE)
                .ok_or_else(|| self.unsupported(node, "import equals target"))?;
            let expand = target
                .is_some_and(|target| self.ast.node(target).syntax_kind == SyntaxKind::Identifier);
            AliasKind::Entity { reference, expand }
        };
        let exported = has_modifier(self.ast, node, SyntaxKind::ExportKeyword);
        let local = self.decoded(name);
        let bind = match (exported, cx.container.and_then(|c| c.host)) {
            (true, Some(Host::File)) => AliasBind::Both {
                scope: cx.scope,
                host: Host::File,
                name: local,
            },
            (true, Some(host)) => AliasBind::Export { host, name: local },
            _ => AliasBind::Lexical {
                scope: cx.scope,
                name: local,
            },
        };
        let alias = self.alias(kind, bind, type_only, self.token(name), node);
        self.alias_name(
            Some(name),
            alias,
            IMPORT | if exported { EXPORT } else { 0 },
            true,
        );
        Ok(())
    }

    fn export_host(&self, node: &Node<'s>, cx: Cx) -> Result<Host> {
        cx.container
            .and_then(|container| container.host)
            .ok_or_else(|| self.unsupported(node, "export outside a module"))
    }

    fn export_declaration(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        self.modifiers(node, cx);
        let host = self.export_host(node, cx)?;
        let type_only = flag(node, "isTypeOnly");
        let request = if field(node, "moduleSpecifier").is_some() {
            let specifier = self.module_specifier(node)?;
            Some(self.request(specifier, RequestKind::ReExport, node)?)
        } else {
            None
        };
        self.import_attributes(node, cx);
        let Some(clause) = child(self.ast, node, "exportClause") else {
            let request =
                request.ok_or_else(|| self.unsupported(node, "export without a clause"))?;
            self.facts.stars.push(Star {
                declaration_span: node.span,
                host,
                request,
                type_only,
            });
            return Ok(());
        };
        if clause.syntax_kind == SyntaxKind::NamespaceExport {
            let request =
                request.ok_or_else(|| self.unsupported(node, "namespace export without module"))?;
            let name = field(clause, "name").unwrap();
            let alias = self.export_alias(
                AliasKind::Namespace { request },
                AliasBind::Export {
                    host,
                    name: self.export_name(name)?,
                },
                type_only,
                self.token(name),
                clause,
            );
            self.alias_name(Some(name), alias, EXPORT, true);
            return Ok(());
        }
        for &element in ids(clause, "elements") {
            let element = self.ast.node(element);
            let name = field(element, "name").unwrap();
            let property = field(element, "propertyName");
            let local = property.unwrap_or(name);
            let kind = match request {
                Some(request) => AliasKind::Import {
                    request,
                    name: self.export_name(local)?,
                },
                None => AliasKind::Lexical {
                    scope: cx.scope,
                    name: self.export_name(local)?,
                },
            };
            let alias = self.export_alias(
                kind,
                AliasBind::Export {
                    host,
                    name: self.export_name(name)?,
                },
                type_only || flag(element, "isTypeOnly"),
                self.token(name),
                element,
            );
            self.alias_name(property, alias, EXPORT, false);
            self.alias_name(Some(name), alias, EXPORT, true);
        }
        Ok(())
    }

    fn export_assignment(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        self.modifiers(node, cx);
        let host = self.export_host(node, cx)?;
        let name = if flag(node, "isExportEquals") {
            "export="
        } else {
            "default"
        };
        let expression = field(node, "expression");
        let bind = AliasBind::Export {
            host,
            name: name.into(),
        };
        match expression.filter(|&expression| is_entity(self.ast, expression)) {
            Some(expression) if self.ast.node(expression).syntax_kind == SyntaxKind::Identifier => {
                let kind = AliasKind::Lexical {
                    scope: cx.scope,
                    name: self.decoded(expression),
                };
                let alias = self.export_alias(kind, bind, false, self.token(expression), node);
                self.alias_name(Some(expression), alias, EXPORT, false);
            }
            Some(expression) => {
                let reference = self
                    .entity(Some(expression), Head::Lexical, Path::EXPORT, cx, USE)
                    .expect("entity names have a reference");
                let token = self.facts.refs[reference.index()].token;
                self.facts.names[token.index()].roles |= EXPORT;
                let kind = AliasKind::Entity {
                    reference,
                    expand: false,
                };
                let anchor = default_keyword(self.ast, node);
                self.export_alias(kind, bind, false, anchor, node);
            }
            None => {
                self.declare(Decl {
                    name: name.into(),
                    native_span: Some(node.span),
                    anchor: default_keyword(self.ast, node),
                    kind: DeclKind::Expression,
                    native_kind: SyntaxKind::ExportAssignment,
                    lexical: None,
                    export: Some((host, name.into())),
                    ..Decl::default()
                });
                self.push(expression, cx);
            }
        }
        Ok(())
    }

    /// A UMD global, `export as namespace`, which TypeScript's binder
    /// (`bindNamespaceExportDeclaration`) declares only at the top level of a
    /// module that is a declaration file; elsewhere it reports TS1316,
    /// TS1314, or TS1315, in that order of precedence, and declares nothing.
    fn umd_global(&mut self, id: NodeId, cx: Cx) {
        let node = self.ast.node(id);
        let name = field(node, "name").unwrap();
        let misplaced = if self.facts.scopes[cx.scope.index()].kind != ScopeKind::Root {
            Some(MisplacedModule::UmdNested)
        } else if !self.module {
            Some(MisplacedModule::UmdScript)
        } else if !flag(self.ast.root(), "isDeclarationFile") {
            Some(MisplacedModule::UmdImplementation)
        } else {
            None
        };
        if let Some(misplaced) = misplaced {
            let span = self.ast.token_span_of(node);
            self.facts.misplaced_modules.push((span, misplaced));
            if self.ast.node(name).syntax_kind == SyntaxKind::Identifier {
                self.name(
                    name,
                    NameTarget::Unresolved("misplaced UMD global declares nothing"),
                    EXPORT,
                    true,
                );
            }
            return;
        }
        let alias = self.alias(
            AliasKind::SelfModule,
            AliasBind::Global {
                name: self.decoded(name),
            },
            false,
            self.token(name),
            node,
        );
        self.alias_name(Some(name), alias, EXPORT, true);
    }

    /// Where a `declare module "name"` or `declare global` statement of
    /// `scope` merges: TypeScript's `IsModuleAugmentationExternal`, the one
    /// rule its binder (`bindModuleDeclaration`) and parser
    /// (`collectModuleReferences`) read for both kinds of block. As the
    /// parser recurses, a script's top-level `declare global` counts as one
    /// of its top-level ambient modules, and `module_declaration` adds the
    /// parser's other condition: a string-named module counts only in an
    /// ambient context.
    fn module_context(&self, scope: ScopeId) -> ModuleContext {
        let scopes = &self.facts.scopes;
        let parent = scopes[scope.index()].parent;
        match scopes[scope.index()].kind {
            ScopeKind::Root if self.module => ModuleContext::External,
            ScopeKind::Root => ModuleContext::Script,
            // Only a script's top-level ambient modules have this scope.
            ScopeKind::AmbientModule { .. }
                if parent.is_some_and(|parent| scopes[parent.index()].kind == ScopeKind::Root) =>
            {
                ModuleContext::External
            }
            // `IsAmbientModule` holds for `declare global` too.
            ScopeKind::ScriptGlobal => ModuleContext::External,
            _ => ModuleContext::Nested,
        }
    }

    fn module_declaration(&mut self, id: NodeId, cx: Cx) -> Result<()> {
        let node = self.ast.node(id);
        self.modifiers(node, cx);
        let name = child(self.ast, node, "name").unwrap();
        let ambient = node.flags & node_flags::AMBIENT != 0;
        let body = child(self.ast, node, "body");
        let statements = body
            .filter(|body| body.syntax_kind == SyntaxKind::ModuleBlock)
            .map_or(&[][..], |body| ids(body, "statements"));
        let export_context = ambient && !has_export_declarations(self.ast, statements);
        let context = self.module_context(cx.scope);
        if name.syntax_kind == SyntaxKind::StringLiteral {
            let span = self.ast.token_span_of(name);
            let specifier = self.semantic_name(name)?;
            let relative = project::is_relative_name(&specifier);
            // TypeScript's parser lists a string-named module only in an
            // ambient context: declared, inside a declared module, or in a
            // declaration file (`collectModuleReferences`). Elsewhere its
            // checker reports TS1035, and the module is no augmentation,
            // though its binder still makes a script's an ambient module.
            if !ambient {
                self.facts
                    .misplaced_modules
                    .push((span, MisplacedModule::Quoted));
            }
            let (scope_kind, host) = match context {
                // TypeScript's parser lists a relative name nested in a
                // script's ambient module as no augmentation, so it merges
                // nowhere, without an error.
                ModuleContext::External if ambient && (self.module || !relative) => {
                    // TypeScript reports a missing augmented module (TS2664)
                    // only where the augmentation's parent is not ambient: a
                    // module that is not a declaration file.
                    let ambient = match self.facts.scopes[cx.scope.index()].kind {
                        ScopeKind::Root => self.ast.root().flags & node_flags::AMBIENT != 0,
                        _ => ambient,
                    };
                    let request =
                        self.request(name, RequestKind::Augmentation { ambient }, node)?;
                    if self.instances.namespace(self.ast, id) != Instance::NonInstantiated {
                        self.facts.instantiated_augmentations.push(request);
                    }
                    (
                        ScopeKind::Augmentation { request },
                        Some(Host::Augmentation(request)),
                    )
                }
                ModuleContext::Script => {
                    if relative {
                        self.facts
                            .misplaced_modules
                            .push((span, MisplacedModule::Relative));
                    }
                    self.facts.ambient_modules.push(AmbientModule {
                        name: specifier,
                        span,
                        shorthand: body.is_none(),
                    });
                    let module = AmbientId::new(self.facts.ambient_modules.len() - 1);
                    (
                        ScopeKind::AmbientModule { module },
                        Some(Host::AmbientModule(module)),
                    )
                }
                ModuleContext::External => (ScopeKind::Unmerged, None),
                ModuleContext::Nested => {
                    self.facts
                        .misplaced_modules
                        .push((span, MisplacedModule::Nested));
                    (ScopeKind::Unmerged, None)
                }
            };
            let native_name = self.semantic_name(name)?;
            let value = context != ModuleContext::External
                || self.instances.namespace(self.ast, id) != Instance::NonInstantiated;
            let module = self.capture_module_declaration(ModuleDeclaration {
                shorthand: body.is_none(),
                scope: cx.scope,
                host,
                span: node.span,
                name: native_name,
                global: false,
                value,
            });
            self.work.push(CollectWork::FinishAmbientModule(module));
            let scope = self.native_scope(scope_kind, Some(cx.scope), node);
            let inner = match host {
                Some(host) => Cx {
                    scope,
                    var_scope: scope,
                    container: Some(Container {
                        host: Some(host),
                        export_context: !has_export_declarations(self.ast, statements),
                        lexical_exports: false,
                    }),
                    ..cx
                },
                None => Cx {
                    var_scope: scope,
                    ..cx.enter(scope)
                },
            };
            self.push_statements(statements, inner);
            return Ok(());
        }
        if node.flags & node_flags::GLOBAL_AUGMENTATION != 0 {
            self.not_reference(field(node, "name"), "global augmentation");
            let value = context != ModuleContext::External
                || self.instances.namespace(self.ast, id) != Instance::NonInstantiated;
            let module = self.capture_module_declaration(ModuleDeclaration {
                shorthand: body.is_none(),
                scope: cx.scope,
                host: (context == ModuleContext::External).then_some(Host::Global),
                span: node.span,
                name: "global".into(),
                global: true,
                value,
            });
            self.work.push(CollectWork::FinishAmbientModule(module));
            if context != ModuleContext::External {
                self.facts
                    .misplaced_modules
                    .push((self.ast.token_span_of(name), MisplacedModule::Global));
                // TypeScript merges such a block nowhere, but its parser lists
                // a script's top-level block's string-named modules as those
                // of a top-level ambient module (`collectModuleReferences`).
                let kind = if context == ModuleContext::Script {
                    ScopeKind::ScriptGlobal
                } else {
                    ScopeKind::Unmerged
                };
                let scope = self.native_scope(kind, Some(cx.scope), node);
                let inner = Cx {
                    var_scope: scope,
                    ..cx.enter(scope)
                };
                self.push_statements(statements, inner);
                return Ok(());
            }
            // The file's global augmentation exports what the block declares,
            // as an ambient module does.
            let scope = self.native_scope(ScopeKind::Global, Some(cx.scope), node);
            let inner = Cx {
                scope,
                var_scope: scope,
                container: Some(Container {
                    host: Some(Host::Global),
                    export_context: !has_export_declarations(self.ast, statements),
                    lexical_exports: false,
                }),
                ..cx
            };
            self.push_statements(statements, inner);
            return Ok(());
        }
        let instance = self.instances.namespace(self.ast, id);
        let decl = self.declare_statement(
            node,
            field(node, "name"),
            DeclKind::Namespace { instance },
            cx,
            cx.scope,
        );
        let scope = self.native_scope(ScopeKind::Namespace { decl }, Some(cx.scope), node);
        let inner = Cx {
            scope,
            var_scope: scope,
            container: Some(Container {
                host: Some(Host::Namespace(decl)),
                export_context,
                lexical_exports: false,
            }),
            ..cx
        };
        // The `B` of `namespace A.B` is a statement of `A`.
        match body {
            Some(body) if body.syntax_kind == SyntaxKind::ModuleDeclaration => {
                let body = field(node, "body");
                self.push_statements(body.as_slice(), inner);
            }
            _ => self.push_statements(statements, inner),
        }
        Ok(())
    }

    fn capture_module_declaration(
        &mut self,
        declaration: ModuleDeclaration,
    ) -> ModuleDeclarationId {
        let id = ModuleDeclarationId::new(self.facts.module_declarations.len());
        self.facts.module_declarations.push(declaration);
        self.facts.binding_order.push(BindingEntry::Module(id));
        id
    }
}

fn has_export_declarations<'s>(ast: &Ast<'s>, statements: &[NodeId]) -> bool {
    statements.iter().any(|&id| {
        matches!(
            ast.node(id).syntax_kind,
            SyntaxKind::ExportDeclaration | SyntaxKind::ExportAssignment
        )
    })
}

/// The first node of a qualified name or property-access chain, which may be
/// any expression, and the names qualifying it in source order. Each name
/// records whether it is accessed as a property, as in `a.b`.
fn entity_parts<'s>(ast: &Ast<'s>, id: NodeId) -> (NodeId, Vec<(NodeId, bool)>) {
    let mut parts = Vec::new();
    let mut current = id;
    loop {
        let node = ast.node(current);
        let (left, right, property) = match node.syntax_kind {
            SyntaxKind::QualifiedName => ("left", "right", false),
            SyntaxKind::PropertyAccessExpression => ("expression", "name", true),
            _ => break,
        };
        parts.push((field(node, right).unwrap(), property));
        current = field(node, left).unwrap();
    }
    parts.reverse();
    (current, parts)
}

/// An identifier or a property-access chain of identifiers.
fn is_entity<'s>(ast: &Ast<'s>, id: NodeId) -> bool {
    let identifier = |id: NodeId| ast.node(id).syntax_kind == SyntaxKind::Identifier;
    let (first, parts) = entity_parts(ast, id);
    identifier(first)
        && parts
            .iter()
            .all(|&(part, property)| property && identifier(part))
}

/// The anchor of an anonymous export: the `default` keyword, or the `=` of
/// `export =`.
fn default_keyword<'s>(ast: &Ast<'s>, node: &Node<'s>) -> Span {
    if let Some(&modifier) = ids(node, "modifiers")
        .iter()
        .find(|&&id| ast.node(id).syntax_kind == SyntaxKind::DefaultKeyword)
    {
        return ast.token_span(modifier);
    }
    let mut lexer = crate::lexer::Lexer::new(ast.source(), node.span.start);
    let mut token = lexer.next();
    while token.raw != "export" && token.end < node.span.end {
        token = lexer.next();
    }
    let token = lexer.next();
    Span {
        start: token.start,
        end: token.end,
    }
}

/// A `resolution-mode` import attribute on an import declaration or type.
fn resolution_mode<'s>(ast: &Ast<'s>, node: &Node<'s>) -> Option<ResolutionMode> {
    let attributes = child(ast, node, "attributes")?;
    ids(attributes, "elements").iter().find_map(|&id| {
        let attribute = ast.node(id);
        let name = child(ast, attribute, "name")?;
        if !is_text(ast, name, "resolution-mode") {
            return None;
        }
        match exact_text(ast, child(ast, attribute, "value")?).as_deref() {
            Ok("import") => Some(ResolutionMode::Import),
            Ok("require") => Some(ResolutionMode::Require),
            _ => None,
        }
    })
}
