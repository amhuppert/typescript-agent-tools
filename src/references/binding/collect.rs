//! One file's semantic facts, captured from its final syntax tree.
//!
//! Every reachable identifier token becomes a `Name` with a pending target: a
//! declaration, a reference to resolve later, an alias, or a non-reference.
//! Scope discovery happens here; lookup waits until every file is collected.

use super::ids::{
    AliasId, AmbientId, DeclId, NameId, RefId, RequestId, ScopeId, TypeParameterListId,
};
use super::instances::{Instance, Instances};
use super::syntax::{child, exact_text, flag, id as field, ids, is_text, is_token, number};
use super::{ModuleRequest, RequestKind, ResolutionMode, has_modifier, meaning};
use crate::compiler::node as node_flags;
use crate::references::source::SourceText;
use crate::references::{ProjectError, ProjectErrorKind};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Node, NodeId, Span};
use std::collections::{HashMap, HashSet};

pub(crate) const DECLARATION: u8 = 1;
pub(crate) const USE: u8 = 2;
pub(crate) const IMPORT: u8 = 4;
pub(crate) const EXPORT: u8 = 8;

/// An identifier token and what it denotes.
#[derive(Debug)]
pub(super) struct Name {
    pub(super) span: Span,
    pub(super) text: String,
    pub(super) roles: u8,
    /// Declaration names, local import names, and exported alias names.
    pub(super) candidate: bool,
    pub(super) target: NameTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NameTarget {
    Decl(DeclId),
    Ref(RefId),
    Alias(AliasId),
    NotReference(&'static str),
}

#[derive(Debug)]
pub(super) struct Scope {
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
    /// `declare global` declares into the global table.
    Global,
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
    TypeAlias,
    Enum {
        constant: bool,
    },
    /// The instance state decides whether the namespace is a value
    /// (`Instance::NonInstantiated` is not) and, with the configuration,
    /// whether TypeScript's merge-order check applies to it.
    Namespace {
        instance: Instance,
    },
    TypeParameter,
    /// An anonymous default or `export =` expression.
    Expression,
}

/// Where an exported declaration or alias becomes visible to importers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Host {
    File,
    Namespace(DeclId),
    AmbientModule(AmbientId),
    Augmentation(RequestId),
}

#[derive(Debug)]
pub(super) struct Decl {
    pub(super) name: String,
    /// The declaring name token, or the anchor of a synthetic declaration.
    pub(super) anchor: Span,
    pub(super) kind: DeclKind,
    /// The scope in which the name is lexically bound, if any.
    pub(super) lexical: Option<ScopeId>,
    pub(super) export: Option<(Host, String)>,
    /// Declared inside a module augmentation, after ordinary declarations.
    pub(super) deferred: bool,
    /// Enum member names, which enum initializers see as members.
    pub(super) members: Vec<String>,
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
    /// A class or interface's type parameter names and whether each has a
    /// default, which merged declarations must agree on.
    pub(super) type_parameters: Option<Vec<(String, bool)>>,
}

impl Default for Decl {
    fn default() -> Self {
        Self {
            name: String::new(),
            anchor: Span { start: 0, end: 0 },
            kind: DeclKind::Var,
            lexical: None,
            export: None,
            deferred: false,
            members: Vec::new(),
            implementation: false,
            ambient: false,
            list: None,
            hoisted: Vec::new(),
            type_parameters: None,
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
    pub(super) bind: AliasBind,
    pub(super) type_only: bool,
    pub(super) deferred: bool,
    /// Where diagnostics point: the alias's own name token, or for the
    /// entity alias of `export =` or `export default`, that keyword.
    pub(super) span: Span,
    /// An export specifier, `export * as`, or export assignment, which is not
    /// a lexical name even inside the namespace whose exports hold it.
    pub(super) export_only: bool,
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

/// Captured facts for one file, with file-local indices.
#[derive(Debug, Default)]
pub(super) struct Facts {
    pub(super) names: Vec<Name>,
    pub(super) scopes: Vec<Scope>,
    pub(super) decls: Vec<Decl>,
    pub(super) refs: Vec<Ref>,
    pub(super) aliases: Vec<Alias>,
    pub(super) stars: Vec<Star>,
    pub(super) ambient_modules: Vec<AmbientModule>,
    /// Declaration names whose container also declares the name with the
    /// other export status (TypeScript's TS2395), for the binding policy.
    pub(super) mixed_exports: Vec<NameId>,
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
    deferred: bool,
}

impl Cx {
    /// Bind children in `scope`, where they are not statements of a container.
    fn enter(self, scope: ScopeId) -> Self {
        Self {
            scope,
            container: None,
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
    facts: Facts,
    requests: Vec<ModuleRequest>,
    work: Vec<(NodeId, Cx)>,
    named: HashSet<NodeId>,
    /// Type parameter lists seen so far.
    lists: usize,
    /// Exported and local meanings declared per container scope and name.
    container_names: HashMap<(ScopeId, String), (u8, u8)>,
    instances: Instances,
}

type Result<T> = std::result::Result<T, ProjectError>;

impl<'a, 's> Collector<'a, 's> {
    pub(super) fn new(ast: &'a Ast<'s>, source: &'a SourceText, module: bool) -> Self {
        Self {
            ast,
            source,
            module,
            facts: Facts::default(),
            requests: Vec::new(),
            work: Vec::new(),
            named: HashSet::new(),
            lists: 0,
            container_names: HashMap::new(),
            instances: Instances::default(),
        }
    }

    pub(super) fn run(mut self) -> Result<(Facts, Vec<ModuleRequest>)> {
        let root = self.ast.root();
        let scope = self.scope(ScopeKind::Root, None);
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
            deferred: false,
        };
        self.push_statements(statements, cx);
        while let Some((id, cx)) = self.work.pop() {
            self.node(id, cx)?;
        }
        self.check_coverage()?;
        Ok((self.facts, self.requests))
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
        self.work.extend(ids.iter().rev().map(|&id| (id, cx)));
    }
    fn push_field(&mut self, node: &Node<'s>, key: &str, cx: Cx) {
        self.push(field(node, key), cx);
    }
    fn push_list(&mut self, node: &Node<'s>, key: &str, cx: Cx) {
        self.push_all(ids(node, key), cx);
    }
    /// Visit the statements of `cx.container`.
    fn push_statements(&mut self, ids: &[NodeId], cx: Cx) {
        self.work.extend(ids.iter().rev().map(|&id| (id, cx)));
    }

    fn scope(&mut self, kind: ScopeKind, parent: Option<ScopeId>) -> ScopeId {
        self.facts.scopes.push(Scope { kind, parent });
        ScopeId::new(self.facts.scopes.len() - 1)
    }

    /// The containing project supplies the file path.
    fn unsupported(&self, node: &Node<'s>, what: &str) -> ProjectError {
        let start = self.ast.token_span_of(node).start;
        ProjectError {
            kind: ProjectErrorKind::Unsupported,
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
        exact_text(node).map(str::to_string).map_err(|display| {
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
            span: self.token(id),
            text: self.decoded(id),
            roles,
            candidate,
            target,
        });
        NameId::new(self.facts.names.len() - 1)
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
        });
        Ok(RequestId::new(self.requests.len() - 1))
    }

    fn declare(&mut self, decl: Decl) -> DeclId {
        self.facts.decls.push(decl);
        DeclId::new(self.facts.decls.len() - 1)
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
        // TypeScript requires merged declarations to be all exported or all
        // local within one container.
        let mut mixed = false;
        if let (Some(container), Some(name)) = (cx.container, name) {
            if !container.export_context {
                let key = (cx.scope, self.decoded(name));
                let entry = self.container_names.entry(key).or_default();
                let (mine, theirs) = if export.is_some() {
                    (&mut entry.0, entry.1)
                } else {
                    (&mut entry.1, entry.0)
                };
                *mine |= kind.declaration_spaces();
                mixed = theirs & kind.declaration_spaces() != 0;
            }
        }
        let decl = DeclId::new(self.facts.decls.len());
        let token = name.map(|name| {
            let roles = DECLARATION | if exported { EXPORT } else { 0 };
            self.name(name, NameTarget::Decl(decl), roles, true)
        });
        if mixed {
            self.facts.mixed_exports.extend(token);
        }
        let anchor = match token {
            Some(token) => self.facts.names[token.index()].span,
            None => default_keyword(self.ast, node),
        };
        self.declare(Decl {
            name: name.map_or_else(|| "default".to_string(), |name| self.decoded(name)),
            anchor,
            kind,
            lexical,
            export,
            deferred: cx.deferred,
            members: match kind {
                DeclKind::Enum { .. } => self.enum_member_names(node),
                _ => Vec::new(),
            },
            implementation: node.syntax_kind == SyntaxKind::FunctionDeclaration
                && field(node, "body").is_some(),
            ambient: node.flags & node_flags::AMBIENT != 0
                || has_modifier(self.ast, node, SyntaxKind::DeclareKeyword),
            list: None,
            hoisted: self.hoisted(cx.scope, scope),
            type_parameters: matches!(kind, DeclKind::Class | DeclKind::Interface)
                .then(|| self.type_parameter_names(node)),
        })
    }

    /// Declare a name bound only in `scope`: an expression's own name or a
    /// type parameter of `list`.
    fn declare_local(
        &mut self,
        name: NodeId,
        kind: DeclKind,
        scope: ScopeId,
        list: Option<TypeParameterListId>,
        cx: Cx,
    ) -> DeclId {
        let decl = DeclId::new(self.facts.decls.len());
        let token = self.name(name, NameTarget::Decl(decl), DECLARATION, true);
        self.declare(Decl {
            name: self.decoded(name),
            anchor: self.facts.names[token.index()].span,
            kind,
            lexical: Some(scope),
            deferred: cx.deferred,
            list,
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

    /// Member names an enum's initializers see as members. Only identifiers
    /// refer to members, so a string name with an unpaired surrogate, which
    /// no identifier can spell, is left out.
    fn enum_member_names(&self, node: &Node<'s>) -> Vec<String> {
        ids(node, "members")
            .iter()
            .filter_map(|&member| {
                let name = field(self.ast.node(member), "name")?;
                match self.ast.node(name).syntax_kind {
                    SyntaxKind::Identifier => Some(self.decoded(name)),
                    SyntaxKind::StringLiteral => {
                        exact_text(self.ast.node(name)).ok().map(str::to_string)
                    }
                    _ => None,
                }
            })
            .collect()
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
        name: Option<NodeId>,
        kind: DeclKind,
        cx: Cx,
        lexical: ScopeId,
    ) -> Result<()> {
        let mut stack: Vec<NodeId> = name.into_iter().collect();
        while let Some(id) = stack.pop() {
            let node = self.ast.node(id);
            match node.syntax_kind {
                SyntaxKind::Identifier => {
                    self.declare_statement(statement, Some(id), kind, cx, lexical);
                }
                SyntaxKind::ObjectBindingPattern | SyntaxKind::ArrayBindingPattern => {
                    stack.extend(ids(node, "elements").iter().rev());
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
                        stack.push(name);
                    }
                }
                SyntaxKind::OmittedExpression => {}
                _ => return Err(self.unsupported(node, "binding name")),
            }
        }
        Ok(())
    }

    fn node(&mut self, id: NodeId, cx: Cx) -> Result<()> {
        let ast = self.ast;
        let node = ast.node(id);
        match node.syntax_kind {
            SyntaxKind::Identifier => {
                self.reference(id, RefBase::Lexical(cx.scope), meaning::VALUE, cx, USE);
            }
            SyntaxKind::PrivateIdentifier => self.not_reference(Some(id), "private member"),

            // Statements
            SyntaxKind::Block => {
                let scope = self.scope(ScopeKind::Block, Some(cx.scope));
                self.push_list(node, "statements", cx.enter(scope));
            }
            SyntaxKind::VariableStatement => {
                self.modifiers(node, cx.scope);
                let list = child(ast, node, "declarationList")
                    .ok_or_else(|| self.unsupported(node, "variable statement"))?;
                self.variable_list(node, list, cx)?;
            }
            SyntaxKind::VariableDeclarationList => self.variable_list(node, node, cx)?,
            SyntaxKind::FunctionDeclaration => {
                self.modifiers(node, cx.scope);
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
                self.modifiers(node, cx.scope);
                let decl = self.declare_statement(
                    node,
                    field(node, "name"),
                    DeclKind::Interface,
                    cx,
                    cx.scope,
                );
                let scope = self.scope(ScopeKind::Members { decl }, Some(cx.scope));
                let inner = cx.enter(scope).types();
                self.type_parameters(node, inner);
                self.heritage(node, inner, false);
                self.push_list(node, "members", inner);
            }
            SyntaxKind::TypeAliasDeclaration => {
                self.modifiers(node, cx.scope);
                self.declare_statement(
                    node,
                    field(node, "name"),
                    DeclKind::TypeAlias,
                    cx,
                    cx.scope,
                );
                let scope = self.scope(ScopeKind::TypeParameters, Some(cx.scope));
                let inner = cx.enter(scope).types();
                self.type_parameters(node, inner);
                self.push_field(node, "type", inner);
            }
            SyntaxKind::EnumDeclaration => {
                self.modifiers(node, cx.scope);
                let constant = has_modifier(ast, node, SyntaxKind::ConstKeyword);
                let decl = self.declare_statement(
                    node,
                    field(node, "name"),
                    DeclKind::Enum { constant },
                    cx,
                    cx.scope,
                );
                let inner = cx.enter(self.scope(ScopeKind::Enum { decl }, Some(cx.scope)));
                for &member in ids(node, "members") {
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
            SyntaxKind::NamespaceExportDeclaration => {
                let name = field(node, "name").unwrap();
                let alias = self.alias(
                    AliasKind::SelfModule,
                    AliasBind::Global {
                        name: self.decoded(name),
                    },
                    false,
                    self.token(name),
                    cx,
                );
                self.alias_name(Some(name), alias, EXPORT, true);
            }
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
                let scope = self.scope(ScopeKind::Block, Some(cx.scope));
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
                let inner = cx.enter(self.scope(ScopeKind::Block, Some(cx.scope)));
                if let Some(block) = child(ast, node, "caseBlock") {
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
                    let scope = self.scope(ScopeKind::Catch, Some(cx.scope));
                    let catch = cx.enter(scope);
                    if let Some(declaration) = child(ast, clause, "variableDeclaration") {
                        self.binding(
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
                let constant = child(ast, node, "type")
                    .filter(|ty| {
                        node.syntax_kind != SyntaxKind::SatisfiesExpression
                            && ty.syntax_kind == SyntaxKind::TypeReference
                    })
                    .filter(|ty| ids(ty, "typeArguments").is_empty())
                    .and_then(|ty| field(ty, "typeName"))
                    .filter(|&name| {
                        let name = ast.node(name);
                        name.syntax_kind == SyntaxKind::Identifier && is_text(name, "const")
                    });
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
            SyntaxKind::ObjectLiteralExpression => self.push_list(node, "properties", cx),
            SyntaxKind::PropertyAssignment => {
                self.member_name(node, cx);
                self.push_field(node, "initializer", cx);
            }
            SyntaxKind::ShorthandPropertyAssignment => {
                let name = field(node, "name").unwrap();
                self.reference(name, RefBase::Lexical(cx.scope), meaning::VALUE, cx, USE);
                self.push_field(node, "objectAssignmentInitializer", cx);
            }
            SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction => {
                let mut scope = cx.scope;
                if let Some(name) = field(node, "name") {
                    scope = self.scope(ScopeKind::ExpressionName, Some(cx.scope));
                    self.declare_local(name, DeclKind::Function, scope, None, cx);
                }
                self.modifiers(node, cx.scope);
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
                self.modifiers(node, cx.scope);
                self.member_name(node, cx);
                self.push_field(node, "type", cx.types());
                self.push_field(node, "initializer", cx);
            }
            SyntaxKind::MethodDeclaration
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
            | SyntaxKind::Constructor
            | SyntaxKind::MethodSignature => {
                self.modifiers(node, cx.scope);
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
                self.modifiers(node, cx.scope);
                self.function(node, cx.types(), cx.scope, false)?;
            }
            SyntaxKind::ClassStaticBlockDeclaration => {
                self.modifiers(node, cx.scope);
                let scope = self.scope(ScopeKind::Body, Some(cx.scope));
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
            SyntaxKind::TypeLiteral => self.push_list(node, "members", cx.types()),
            SyntaxKind::NamedTupleMember => {
                self.not_reference(field(node, "name"), "tuple member label");
                self.push_field(node, "type", cx.types());
            }
            SyntaxKind::MappedType => {
                let scope = self.scope(ScopeKind::TypeParameters, Some(cx.scope));
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
                let infer = self.scope(ScopeKind::Infer, Some(cx.scope));
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
    fn modifiers(&mut self, node: &Node<'s>, scope: ScopeId) {
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
            deferred: false,
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
            self.binding(statement, name, kind, cx, lexical)?;
            self.push_field(declaration, "type", cx.types());
            self.push_field(declaration, "initializer", cx);
        }
        Ok(())
    }

    /// Parameters, type parameters, return type, and body of a function-like
    /// node. `outer` is the scope enclosing its signature.
    fn function(&mut self, node: &Node<'s>, cx: Cx, outer: ScopeId, arguments: bool) -> Result<()> {
        let signature = self.scope(
            ScopeKind::Signature {
                arguments: arguments && node.syntax_kind != SyntaxKind::ArrowFunction,
            },
            Some(outer),
        );
        let inner = cx.enter(signature);
        self.type_parameters(node, inner.types());
        for &parameter in ids(node, "parameters") {
            let parameter = self.ast.node(parameter);
            self.modifiers(parameter, outer);
            let name = field(parameter, "name");
            let this = name.is_some_and(|name| {
                let name = self.ast.node(name);
                name.syntax_kind == SyntaxKind::Identifier && is_text(name, "this")
            });
            if this {
                self.not_reference(name, "this parameter");
            } else {
                self.binding(parameter, name, DeclKind::Parameter, inner, signature)?;
            }
            self.push_field(parameter, "type", inner.types());
            self.push_field(parameter, "initializer", inner);
        }
        self.push_field(node, "type", inner.types());
        if let Some(body) = child(self.ast, node, "body") {
            let scope = self.scope(ScopeKind::Body, Some(signature));
            let body_cx = Cx {
                var_scope: scope,
                type_context: false,
                ..cx.enter(scope)
            };
            if body.syntax_kind == SyntaxKind::Block {
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

    fn type_parameter_names(&self, node: &Node<'s>) -> Vec<(String, bool)> {
        ids(node, "typeParameters")
            .iter()
            .map(|&parameter| self.ast.node(parameter))
            .filter_map(|parameter| {
                let name = self.decoded(field(parameter, "name")?);
                Some((name, field(parameter, "default").is_some()))
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
            self.declare_local(name, DeclKind::TypeParameter, scope, list, cx);
        }
        // Constraints see the parameters of their own list.
        let inner = cx.enter(constraints).types();
        self.push_field(node, "constraint", inner);
        self.push_field(node, "default", inner);
    }

    fn class(&mut self, node: &Node<'s>, cx: Cx) {
        self.modifiers(node, cx.scope);
        let name = field(node, "name");
        let mut outer = cx.scope;
        let decl = if node.syntax_kind == SyntaxKind::ClassDeclaration
            && (name.is_some() || has_modifier(self.ast, node, SyntaxKind::DefaultKeyword))
        {
            Some(self.declare_statement(node, name, DeclKind::Class, cx, cx.scope))
        } else if let Some(name) = name {
            outer = self.scope(ScopeKind::ExpressionName, Some(cx.scope));
            Some(self.declare_local(name, DeclKind::Class, outer, None, cx))
        } else {
            None
        };
        let kind = match decl {
            Some(decl) => ScopeKind::Members { decl },
            None => ScopeKind::TypeParameters,
        };
        let scope = self.scope(kind, Some(outer));
        let inner = Cx {
            type_context: false,
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
        cx: Cx,
    ) -> AliasId {
        self.add_alias(kind, bind, type_only, anchor, false, cx)
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
        cx: Cx,
    ) -> AliasId {
        self.add_alias(kind, bind, type_only, anchor, true, cx)
    }

    fn add_alias(
        &mut self,
        kind: AliasKind,
        bind: AliasBind,
        type_only: bool,
        span: Span,
        export_only: bool,
        cx: Cx,
    ) -> AliasId {
        self.facts.aliases.push(Alias {
            kind,
            bind,
            type_only,
            deferred: cx.deferred,
            span,
            export_only,
        });
        AliasId::new(self.facts.aliases.len() - 1)
    }

    /// Record a name token of an alias; a string-literal name is no token.
    fn alias_name(&mut self, id: Option<NodeId>, alias: AliasId, roles: u8, candidate: bool) {
        let Some(id) = id else { return };
        if self.ast.node(id).syntax_kind == SyntaxKind::Identifier {
            self.name(id, NameTarget::Alias(alias), roles, candidate);
        }
    }

    fn import_declaration(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        self.modifiers(node, cx.scope);
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
                cx,
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
                cx,
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
                cx,
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
        self.modifiers(node, cx.scope);
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
        let alias = self.alias(kind, bind, type_only, self.token(name), cx);
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
        self.modifiers(node, cx.scope);
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
                cx,
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
                cx,
            );
            self.alias_name(property, alias, EXPORT, false);
            self.alias_name(Some(name), alias, EXPORT, true);
        }
        Ok(())
    }

    fn export_assignment(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        self.modifiers(node, cx.scope);
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
                let alias = self.export_alias(kind, bind, false, self.token(expression), cx);
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
                self.export_alias(kind, bind, false, anchor, cx);
            }
            None => {
                self.declare(Decl {
                    name: name.into(),
                    anchor: default_keyword(self.ast, node),
                    kind: DeclKind::Expression,
                    lexical: None,
                    export: Some((host, name.into())),
                    deferred: cx.deferred,
                    ..Decl::default()
                });
                self.push(expression, cx);
            }
        }
        Ok(())
    }

    fn module_declaration(&mut self, id: NodeId, cx: Cx) -> Result<()> {
        let node = self.ast.node(id);
        self.modifiers(node, cx.scope);
        let name = child(self.ast, node, "name").unwrap();
        let ambient = node.flags & node_flags::AMBIENT != 0;
        let body = child(self.ast, node, "body");
        let statements = body
            .filter(|body| body.syntax_kind == SyntaxKind::ModuleBlock)
            .map_or(&[][..], |body| ids(body, "statements"));
        let export_context = ambient && !has_export_declarations(self.ast, statements);
        if name.syntax_kind == SyntaxKind::StringLiteral {
            let (scope_kind, host) = if self.module {
                let request = self.request(name, RequestKind::Augmentation, node)?;
                (
                    ScopeKind::Augmentation { request },
                    Host::Augmentation(request),
                )
            } else {
                self.facts.ambient_modules.push(AmbientModule {
                    name: self.semantic_name(name)?,
                    span: self.ast.token_span_of(name),
                    shorthand: body.is_none(),
                });
                let module = AmbientId::new(self.facts.ambient_modules.len() - 1);
                (
                    ScopeKind::AmbientModule { module },
                    Host::AmbientModule(module),
                )
            };
            let scope = self.scope(scope_kind, Some(cx.scope));
            let inner = Cx {
                scope,
                var_scope: scope,
                container: Some(Container {
                    host: Some(host),
                    export_context: !has_export_declarations(self.ast, statements),
                    lexical_exports: false,
                }),
                deferred: cx.deferred || matches!(host, Host::Augmentation(_)),
                ..cx
            };
            self.push_statements(statements, inner);
            return Ok(());
        }
        if node.flags & node_flags::GLOBAL_AUGMENTATION != 0 {
            self.not_reference(field(node, "name"), "global augmentation");
            let scope = self.scope(ScopeKind::Global, Some(cx.scope));
            let inner = Cx {
                var_scope: scope,
                ..cx.enter(scope)
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
        let scope = self.scope(ScopeKind::Namespace { decl }, Some(cx.scope));
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
    let mut lexer = crate::lexer::Lexer {
        src: ast.source(),
        pos: node.span.start,
    };
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
        if !is_text(name, "resolution-mode") {
            return None;
        }
        match exact_text(child(ast, attribute, "value")?) {
            Ok("import") => Some(ResolutionMode::Import),
            Ok("require") => Some(ResolutionMode::Require),
            _ => None,
        }
    })
}
