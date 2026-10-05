//! One file's semantic facts, captured from its final syntax tree.
//!
//! Every reachable identifier token becomes a `Name` with a pending target: a
//! declaration, a reference to resolve later, an alias, or a non-reference.
//! Scope discovery happens here; lookup waits until every file is collected.

use super::syntax::{child, children, flag, id as field, ids, text};
use super::{ModuleRequest, RequestKind, ResolutionMode, has_modifier, meaning};
use crate::compiler::node as node_flags;
use crate::references::source::{decoded_name, token_span};
use crate::references::{ProjectError, ProjectErrorKind, SourcePosition};
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
    Decl(u32),
    Ref(u32),
    Alias(u32),
    NotReference(&'static str),
}

#[derive(Debug)]
pub(super) struct Scope {
    pub(super) kind: ScopeKind,
    pub(super) parent: Option<u32>,
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
        decl: u32,
    },
    TypeParameters,
    /// `infer` declarations, visible in a conditional type's true branch.
    Infer,
    /// An `infer` declaration's constraint, which sees that declaration's
    /// own name but not its siblings.
    InferConstraint {
        decl: u32,
    },
    Namespace {
        decl: u32,
    },
    AmbientModule {
        module: u32,
    },
    Augmentation {
        request: u32,
    },
    /// `declare global` declares into the global table.
    Global,
    Enum {
        decl: u32,
    },
}

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
    Namespace {
        instantiated: bool,
    },
    TypeParameter,
    /// An anonymous default or `export =` expression.
    Expression,
}

impl DeclKind {
    pub(super) fn meanings(self) -> u8 {
        match self {
            Self::Var | Self::Parameter | Self::Let | Self::Function | Self::Expression => {
                meaning::VALUE
            }
            Self::Class => meaning::VALUE | meaning::TYPE,
            Self::Interface | Self::TypeAlias | Self::TypeParameter => meaning::TYPE,
            Self::Enum { .. } => meaning::ALL,
            Self::Namespace { instantiated } => {
                meaning::NAMESPACE | if instantiated { meaning::VALUE } else { 0 }
            }
        }
    }
}

/// Where an exported declaration or alias becomes visible to importers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Host {
    File,
    Namespace(u32),
    AmbientModule(u32),
    Augmentation(u32),
}

#[derive(Debug)]
pub(super) struct Decl {
    pub(super) name: String,
    /// The declaring name token, or the anchor of a synthetic declaration.
    pub(super) anchor: Span,
    pub(super) kind: DeclKind,
    /// The scope in which the name is lexically bound, if any.
    pub(super) lexical: Option<u32>,
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
    pub(super) list: u32,
    /// Blocks a `var` is hoisted out of, whose block-scoped bindings it may
    /// not share a name with.
    pub(super) hoisted: Vec<u32>,
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
            list: u32::MAX,
            hoisted: Vec::new(),
            type_parameters: None,
        }
    }
}

#[derive(Debug)]
pub(super) struct Ref {
    pub(super) token: u32,
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
    Lexical(u32),
    /// A name qualified by the namespace-like target of another reference.
    Member(u32),
    /// An export of a module request, as in `import("m").Name`.
    Module(u32),
}

#[derive(Debug)]
pub(super) struct Alias {
    pub(super) kind: AliasKind,
    pub(super) bind: AliasBind,
    pub(super) type_only: bool,
    pub(super) deferred: bool,
    /// The alias's own name token, for diagnostics.
    pub(super) span: Span,
    /// An export specifier, `export * as`, or export assignment, which is not
    /// a lexical name even inside the namespace whose exports hold it.
    pub(super) export_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AliasKind {
    /// A named or default export of a requested module.
    Import { request: u32, name: String },
    /// The module object, or its `export =` target.
    Namespace { request: u32 },
    /// This file as a module, for `export as namespace`.
    SelfModule,
    /// Every meaning of a lexically visible name.
    Lexical { scope: u32, name: String },
    /// The target of an entity-name reference such as `A.B.C`. With
    /// `expand`, a name found as a namespace aliases all of its meanings.
    Entity { reference: u32, expand: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AliasBind {
    Lexical {
        scope: u32,
        name: String,
    },
    Export {
        host: Host,
        name: String,
    },
    Both {
        scope: u32,
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
    pub(super) request: u32,
    pub(super) type_only: bool,
}

#[derive(Debug)]
pub(super) struct AmbientModule {
    pub(super) name: String,
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
    /// End offsets of `@ts-ignore` and `@ts-expect-error` comments.
    pub(super) directives: Vec<u32>,
}

/// The meanings an entity name such as `A.B.C` asks for: a lone name, the
/// head of a qualified name, its middle parts, and its last part.
#[derive(Debug, Clone, Copy)]
struct Path {
    single: u8,
    head: u8,
    middle: u8,
    last: u8,
}

impl Path {
    const TYPE: Self = Self {
        single: meaning::TYPE,
        head: meaning::NAMESPACE,
        middle: meaning::NAMESPACE,
        last: meaning::TYPE,
    };
    const VALUE: Self = Self {
        single: meaning::VALUE,
        head: meaning::VALUE,
        middle: meaning::VALUE,
        last: meaning::VALUE,
    };
    /// `import x = A.B`: namespaces, then any meaning of the last part.
    const IMPORT: Self = Self {
        single: meaning::NAMESPACE,
        head: meaning::NAMESPACE,
        middle: meaning::NAMESPACE,
        last: meaning::ALL,
    };
    /// `export default a.b`: a value or namespace receiver, whose member is an
    /// alias only when the receiver proves to be a namespace.
    const EXPORT: Self = Self {
        single: meaning::ALL,
        head: meaning::VALUE | meaning::NAMESPACE,
        middle: meaning::VALUE | meaning::NAMESPACE,
        last: meaning::ALL,
    };
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
    scope: u32,
    var_scope: u32,
    container: Option<Container>,
    infer: Option<u32>,
    type_context: bool,
    deferred: bool,
}

impl Cx {
    fn nested(self, scope: u32) -> Self {
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
    text: &'s str,
    module: bool,
    pub(super) facts: Facts,
    pub(super) requests: Vec<ModuleRequest>,
    work: Vec<(NodeId, Cx)>,
    named: HashSet<NodeId>,
    /// Type parameter lists seen so far.
    lists: u32,
    /// Exported and local meanings declared per container scope and name.
    container_names: HashMap<(u32, String), (u8, u8)>,
    /// Starts of leading trivia already searched for comment directives.
    trivia: HashSet<usize>,
}

type Result<T> = std::result::Result<T, ProjectError>;

impl<'a, 's> Collector<'a, 's> {
    pub(super) fn new(ast: &'a Ast<'s>, module: bool) -> Self {
        Self {
            ast,
            text: ast.source(),
            module,
            facts: Facts::default(),
            requests: Vec::new(),
            work: Vec::new(),
            named: HashSet::new(),
            lists: 0,
            container_names: HashMap::new(),
            trivia: HashSet::new(),
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
        self.push_all(statements, cx);
        while let Some((id, cx)) = self.work.pop() {
            self.node(id, cx)?;
        }
        self.check_coverage()?;
        Ok((self.facts, self.requests))
    }

    /// Every reachable identifier must have been classified by a syntax rule.
    fn check_coverage(&self) -> Result<()> {
        let mut stack = vec![self.ast.root_id()];
        while let Some(id) = stack.pop() {
            let node = self.ast.node(id);
            if matches!(node.kind, "Identifier" | "PrivateIdentifier") && !self.named.contains(&id)
            {
                return Err(self.unsupported(node, "identifier in unsupported syntax"));
            }
            stack.extend(children(node));
        }
        Ok(())
    }

    fn push(&mut self, id: Option<NodeId>, cx: Cx) {
        if let Some(id) = id {
            self.work.push((id, cx));
        }
    }
    fn push_all(&mut self, ids: &[NodeId], cx: Cx) {
        for &id in ids.iter().rev() {
            self.work.push((id, cx));
        }
    }
    fn push_field(&mut self, node: &Node<'s>, key: &str, cx: Cx) {
        self.push(field(node, key), cx);
    }
    fn push_list(&mut self, node: &Node<'s>, key: &str, cx: Cx) {
        let list = ids(node, key).to_vec();
        self.push_all(&list, cx);
    }

    fn scope(&mut self, kind: ScopeKind, parent: Option<u32>) -> u32 {
        self.facts.scopes.push(Scope { kind, parent });
        (self.facts.scopes.len() - 1) as u32
    }

    /// An invalid arrangement of declarations; the project supplies the path.
    fn binding_error(&self, node: &Node<'s>, message: &str) -> ProjectError {
        ProjectError {
            kind: ProjectErrorKind::Binding,
            message: format!("{message}: `{}`", decoded_name(&text(node))),
            ..self.unsupported(node, "")
        }
    }

    /// The containing project supplies the file path.
    fn unsupported(&self, node: &Node<'s>, what: &str) -> ProjectError {
        let span = token_span(self.text, node.span);
        let (line, column) = crate::Diagnostic {
            message: String::new(),
            span,
        }
        .line_column(self.text);
        ProjectError {
            kind: ProjectErrorKind::Unsupported,
            message: format!("unsupported {what} ({})", node.kind),
            path: None,
            position: Some(SourcePosition { line, column }),
        }
    }

    /// Record an identifier token.
    fn name(&mut self, id: NodeId, target: NameTarget, roles: u8) -> u32 {
        let node = self.ast.node(id);
        let first = self.named.insert(id);
        debug_assert!(first, "identifier {id:?} recorded twice");
        self.facts.names.push(Name {
            span: token_span(self.text, node.span),
            text: decoded_name(&text(node)).to_string(),
            roles,
            candidate: false,
            target,
        });
        (self.facts.names.len() - 1) as u32
    }

    fn not_reference(&mut self, id: Option<NodeId>, why: &'static str) {
        if let Some(id) = id {
            if matches!(self.ast.node(id).kind, "Identifier" | "PrivateIdentifier") {
                self.name(id, NameTarget::NotReference(why), 0);
            }
        }
    }

    fn reference(&mut self, id: NodeId, base: RefBase, meaning: u8, cx: Cx, roles: u8) -> u32 {
        let reference = self.facts.refs.len() as u32;
        let token = self.name(id, NameTarget::Ref(reference), roles);
        self.facts.refs.push(Ref {
            token,
            base,
            meaning,
            property: false,
            type_context: cx.type_context,
        });
        reference
    }

    fn request(&mut self, specifier: &Node<'s>, kind: RequestKind, owner: &Node<'s>) -> u32 {
        // TypeScript honors `resolution-mode` only on import types and on
        // type-only imports and exports.
        let type_only = match owner.kind {
            "ImportType" => true,
            "ImportDeclaration" => child(self.ast, owner, "importClause")
                .is_some_and(|clause| flag(clause, "isTypeOnly")),
            "ExportDeclaration" => flag(owner, "isTypeOnly"),
            _ => false,
        };
        self.requests.push(ModuleRequest {
            specifier: text(specifier),
            kind,
            mode: type_only
                .then(|| resolution_mode(self.ast, owner))
                .flatten(),
            span: specifier.span,
        });
        (self.requests.len() - 1) as u32
    }

    fn declare(&mut self, decl: Decl) -> u32 {
        self.facts.decls.push(decl);
        (self.facts.decls.len() - 1) as u32
    }

    /// Declare a named statement-level declaration according to its container.
    fn declare_statement(
        &mut self,
        node: &Node<'s>,
        name: Option<NodeId>,
        kind: DeclKind,
        cx: Cx,
        lexical: u32,
    ) -> Result<u32> {
        // The inner names of `namespace A.B` are exported from their parents.
        let exported = has_modifier(self.ast, node, "ExportKeyword")
            || node.kind == "ModuleDeclaration" && node.flags & node_flags::NESTED_NAMESPACE != 0;
        let default = has_modifier(self.ast, node, "DefaultKeyword");
        let (export, lexical) = match cx.container {
            Some(container) => match container.host {
                Some(host) if exported || container.export_context => {
                    let export_name = match (default, name) {
                        (true, _) => "default".to_string(),
                        (false, Some(name)) => self.decoded(name),
                        (false, None) => "default".to_string(),
                    };
                    let lexical = (container.lexical_exports && name.is_some()).then_some(lexical);
                    (Some((host, export_name)), lexical)
                }
                _ => (None, Some(lexical)),
            },
            None => (None, Some(lexical)),
        };
        if let (Some(container), Some(name)) = (cx.container, name) {
            // TypeScript requires merged declarations to be all exported or
            // all local within one container.
            if !container.export_context {
                let key = (cx.scope, self.decoded(name));
                let entry = self.container_names.entry(key).or_default();
                let (mine, theirs) = if export.is_some() {
                    (&mut entry.0, entry.1)
                } else {
                    (&mut entry.1, entry.0)
                };
                *mine |= kind.meanings();
                if theirs & kind.meanings() != 0 {
                    return Err(self.binding_error(
                        self.ast.node(name),
                        "declarations of one name must be all exported or all local",
                    ));
                }
            }
        }
        let decl = self.facts.decls.len() as u32;
        let token = name.map(|name| {
            let roles = DECLARATION | if exported { EXPORT } else { 0 };
            let token = self.name(name, NameTarget::Decl(decl), roles);
            self.facts.names[token as usize].candidate = true;
            token
        });
        let anchor = match token {
            Some(token) => self.facts.names[token as usize].span,
            None => default_keyword(self.ast, self.text, node),
        };
        Ok(self.declare(Decl {
            name: name.map_or_else(|| "default".to_string(), |name| self.decoded(name)),
            anchor,
            kind,
            lexical,
            export,
            deferred: cx.deferred,
            implementation: node.kind == "FunctionDeclaration" && field(node, "body").is_some(),
            ambient: node.flags & node_flags::AMBIENT != 0
                || has_modifier(self.ast, node, "DeclareKeyword"),
            ..Decl::default()
        }))
    }

    fn decoded(&self, id: NodeId) -> String {
        decoded_name(&text(self.ast.node(id))).to_string()
    }

    /// Declare the identifiers of a binding name or pattern, visiting default
    /// values and computed keys in `cx`.
    fn binding(
        &mut self,
        statement: &Node<'s>,
        name: Option<NodeId>,
        kind: DeclKind,
        cx: Cx,
        lexical: u32,
    ) -> Result<()> {
        let mut stack: Vec<NodeId> = name.into_iter().collect();
        while let Some(id) = stack.pop() {
            let node = self.ast.node(id);
            match node.kind {
                "Identifier" => {
                    self.declare_statement(statement, Some(id), kind, cx, lexical)?;
                }
                "ObjectBindingPattern" | "ArrayBindingPattern" => {
                    stack.extend(ids(node, "elements").iter().rev());
                }
                "BindingElement" => {
                    match child(self.ast, node, "propertyName").map(|p| p.kind) {
                        Some("ComputedPropertyName") => {
                            self.push_field(node, "propertyName", cx.nested(cx.scope))
                        }
                        Some(_) => self.not_reference(field(node, "propertyName"), "property"),
                        None => {}
                    }
                    self.push_field(node, "initializer", cx.nested(cx.scope));
                    if let Some(name) = field(node, "name") {
                        stack.push(name);
                    }
                }
                "OmittedExpression" => {}
                _ => return Err(self.unsupported(node, "binding name")),
            }
        }
        Ok(())
    }

    fn node(&mut self, id: NodeId, cx: Cx) -> Result<()> {
        let ast = self.ast;
        let node = ast.node(id);
        self.directives(node.span.start);
        match node.kind {
            "Identifier" => {
                self.reference(id, RefBase::Lexical(cx.scope), meaning::VALUE, cx, USE);
            }
            "PrivateIdentifier" => self.not_reference(Some(id), "private member"),

            // Statements
            "Block" => {
                let scope = self.scope(ScopeKind::Block, Some(cx.scope));
                self.push_list(node, "statements", cx.nested(scope));
            }
            "VariableStatement" => {
                self.modifiers(node, cx.scope);
                let list = child(ast, node, "declarationList")
                    .ok_or_else(|| self.unsupported(node, "variable statement"))?;
                self.variable_list(node, list, cx)?;
            }
            "VariableDeclarationList" => self.variable_list(node, node, cx)?,
            "FunctionDeclaration" => {
                self.modifiers(node, cx.scope);
                if field(node, "name").is_some() || has_modifier(ast, node, "DefaultKeyword") {
                    self.declare_statement(
                        node,
                        field(node, "name"),
                        DeclKind::Function,
                        cx,
                        cx.scope,
                    )?;
                }
                self.function(node, cx, cx.scope, true)?;
            }
            "ClassDeclaration" | "ClassExpression" => self.class(node, cx)?,
            "InterfaceDeclaration" => {
                self.modifiers(node, cx.scope);
                let decl = self.declare_statement(
                    node,
                    field(node, "name"),
                    DeclKind::Interface,
                    cx,
                    cx.scope,
                )?;
                self.facts.decls[decl as usize].type_parameters =
                    Some(self.type_parameter_names(node));
                let scope = self.scope(ScopeKind::Members { decl }, Some(cx.scope));
                let inner = cx.nested(scope).types();
                self.type_parameters(node, inner);
                self.heritage(node, inner, false)?;
                self.push_list(node, "members", inner);
            }
            "TypeAliasDeclaration" => {
                self.modifiers(node, cx.scope);
                self.declare_statement(
                    node,
                    field(node, "name"),
                    DeclKind::TypeAlias,
                    cx,
                    cx.scope,
                )?;
                let scope = self.scope(ScopeKind::TypeParameters, Some(cx.scope));
                let inner = cx.nested(scope).types();
                self.type_parameters(node, inner);
                self.push_field(node, "type", inner);
            }
            "EnumDeclaration" => {
                self.modifiers(node, cx.scope);
                let constant = has_modifier(ast, node, "ConstKeyword");
                let decl = self.declare_statement(
                    node,
                    field(node, "name"),
                    DeclKind::Enum { constant },
                    cx,
                    cx.scope,
                )?;
                let scope = self.scope(ScopeKind::Enum { decl }, Some(cx.scope));
                for &member in ids(node, "members") {
                    let member = ast.node(member);
                    match child(ast, member, "name").map(|name| name.kind) {
                        Some("Identifier") => {
                            let name = field(member, "name").unwrap();
                            let member = self.decoded(name);
                            self.facts.decls[decl as usize].members.push(member);
                            self.not_reference(Some(name), "enum member");
                        }
                        Some("ComputedPropertyName") => {
                            self.push_field(member, "name", cx.nested(scope))
                        }
                        Some("StringLiteral") => {
                            let name = child(ast, member, "name").unwrap();
                            self.facts.decls[decl as usize].members.push(text(name));
                        }
                        Some("PrivateIdentifier") => {
                            self.not_reference(field(member, "name"), "enum member")
                        }
                        _ => {}
                    }
                    self.push_field(member, "initializer", cx.nested(scope));
                }
            }
            "ModuleDeclaration" => self.module_declaration(node, cx)?,
            "ImportDeclaration" => self.import_declaration(node, cx)?,
            "ImportEqualsDeclaration" => self.import_equals(node, cx)?,
            "ExportDeclaration" => self.export_declaration(node, cx)?,
            "ExportAssignment" => self.export_assignment(node, cx)?,
            "NamespaceExportDeclaration" => {
                let alias = self.alias(
                    AliasKind::SelfModule,
                    AliasBind::Global {
                        name: self.decoded(field(node, "name").unwrap()),
                    },
                    false,
                    cx,
                );
                let token = self.name(
                    field(node, "name").unwrap(),
                    NameTarget::Alias(alias),
                    EXPORT,
                );
                self.facts.names[token as usize].candidate = true;
            }
            "ExpressionStatement" | "ReturnStatement" | "ThrowStatement" => {
                self.push_field(node, "expression", cx.nested(cx.scope))
            }
            "IfStatement" => {
                let inner = cx.nested(cx.scope);
                self.push_field(node, "expression", inner);
                self.push_field(node, "thenStatement", inner);
                self.push_field(node, "elseStatement", inner);
            }
            "DoStatement" | "WhileStatement" => {
                let inner = cx.nested(cx.scope);
                self.push_field(node, "expression", inner);
                self.push_field(node, "statement", inner);
            }
            "ForStatement" | "ForInStatement" | "ForOfStatement" => {
                let scope = self.scope(ScopeKind::Block, Some(cx.scope));
                let inner = cx.nested(scope);
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
            "SwitchStatement" => {
                self.push_field(node, "expression", cx.nested(cx.scope));
                let scope = self.scope(ScopeKind::Block, Some(cx.scope));
                if let Some(block) = child(ast, node, "caseBlock") {
                    for &clause in ids(block, "clauses") {
                        let clause = ast.node(clause);
                        self.push_field(clause, "expression", cx.nested(scope));
                        self.push_list(clause, "statements", cx.nested(scope));
                    }
                }
            }
            "LabeledStatement" => {
                self.not_reference(field(node, "label"), "label");
                self.push_field(node, "statement", cx);
            }
            "BreakStatement" | "ContinueStatement" => {
                self.not_reference(field(node, "label"), "label")
            }
            "TryStatement" => {
                let inner = cx.nested(cx.scope);
                self.push_field(node, "tryBlock", inner);
                if let Some(clause) = child(ast, node, "catchClause") {
                    let scope = self.scope(ScopeKind::Catch, Some(cx.scope));
                    let catch = cx.nested(scope);
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
                self.push_field(node, "finallyBlock", inner);
            }
            "EmptyStatement" | "DebuggerStatement" => {}
            "WithStatement" => return Err(self.unsupported(node, "with statement")),

            // Expressions
            "PropertyAccessExpression" => self.property_access(id, cx, USE),
            "ElementAccessExpression" => {
                self.push_field(node, "expression", cx);
                self.push_field(node, "argumentExpression", cx);
            }
            "CallExpression" | "NewExpression" => {
                if child(ast, node, "expression")
                    .is_some_and(|callee| callee.kind == "ImportKeyword")
                {
                    if let Some(&argument) = ids(node, "arguments").first() {
                        let argument = ast.node(argument);
                        if matches!(
                            argument.kind,
                            "StringLiteral" | "NoSubstitutionTemplateLiteral"
                        ) {
                            self.request(argument, RequestKind::Dynamic, node);
                        }
                    }
                } else {
                    self.push_field(node, "expression", cx);
                }
                self.push_list(node, "typeArguments", cx.types());
                self.push_list(node, "arguments", cx);
            }
            "TaggedTemplateExpression" => {
                self.push_field(node, "tag", cx);
                self.push_list(node, "typeArguments", cx.types());
                self.push_field(node, "template", cx);
            }
            "TemplateExpression" | "TemplateLiteralType" => {
                let inner = if node.kind == "TemplateLiteralType" {
                    cx.types()
                } else {
                    cx
                };
                for &span in ids(node, "templateSpans") {
                    self.push(field(ast.node(span), "expression"), inner);
                    self.push(field(ast.node(span), "type"), inner);
                }
            }
            "ParenthesizedExpression"
            | "AwaitExpression"
            | "YieldExpression"
            | "TypeOfExpression"
            | "VoidExpression"
            | "DeleteExpression"
            | "SpreadElement"
            | "SpreadAssignment"
            | "NonNullExpression"
            | "JsxSpreadAttribute"
            | "JsxExpression"
            | "ComputedPropertyName"
            | "Decorator"
            | "TemplateSpan" => self.push_field(node, "expression", cx),
            "PrefixUnaryExpression" | "PostfixUnaryExpression" => {
                self.push_field(node, "operand", cx)
            }
            "BinaryExpression" => {
                self.push_field(node, "left", cx);
                self.push_field(node, "right", cx);
            }
            "ConditionalExpression" => {
                for field in ["condition", "whenTrue", "whenFalse"] {
                    self.push_field(node, field, cx);
                }
            }
            "AsExpression" | "SatisfiesExpression" | "TypeAssertionExpression" => {
                self.push_field(node, "expression", cx);
                // `as const` spells the keyword as a type reference.
                let constant = child(ast, node, "type")
                    .filter(|ty| node.kind != "SatisfiesExpression" && ty.kind == "TypeReference")
                    .filter(|ty| ids(ty, "typeArguments").is_empty())
                    .and_then(|ty| field(ty, "typeName"))
                    .filter(|&name| {
                        let name = ast.node(name);
                        name.kind == "Identifier" && text(name) == "const"
                    });
                match constant {
                    Some(name) => self.not_reference(Some(name), "const assertion"),
                    None => self.push_field(node, "type", cx.types()),
                }
            }
            "ExpressionWithTypeArguments" => {
                self.push_field(node, "expression", cx);
                self.push_list(node, "typeArguments", cx.types());
            }
            "ArrayLiteralExpression" => self.push_list(node, "elements", cx),
            "ObjectLiteralExpression" => self.push_list(node, "properties", cx),
            "PropertyAssignment" => {
                self.member_name(node, cx);
                self.push_field(node, "initializer", cx);
            }
            "ShorthandPropertyAssignment" => {
                let name = field(node, "name").unwrap();
                self.reference(name, RefBase::Lexical(cx.scope), meaning::VALUE, cx, USE);
                self.push_field(node, "objectAssignmentInitializer", cx);
            }
            "FunctionExpression" | "ArrowFunction" => {
                let mut scope = cx.scope;
                if let Some(name) = field(node, "name") {
                    scope = self.scope(ScopeKind::ExpressionName, Some(cx.scope));
                    let decl = self.facts.decls.len() as u32;
                    let token = self.name(name, NameTarget::Decl(decl), DECLARATION);
                    self.facts.names[token as usize].candidate = true;
                    let span = self.facts.names[token as usize].span;
                    self.declare(Decl {
                        name: self.decoded(name),
                        anchor: span,
                        kind: DeclKind::Function,
                        lexical: Some(scope),
                        export: None,
                        deferred: cx.deferred,
                        members: Vec::new(),
                        ..Decl::default()
                    });
                }
                self.modifiers(node, cx.scope);
                self.function(node, cx, scope, node.kind == "FunctionExpression")?;
            }
            "MetaProperty" => self.not_reference(field(node, "name"), "meta property"),
            "OmittedExpression"
            | "ThisKeyword"
            | "SuperKeyword"
            | "NullKeyword"
            | "TrueKeyword"
            | "FalseKeyword"
            | "ImportKeyword"
            | "NumericLiteral"
            | "BigIntLiteral"
            | "StringLiteral"
            | "RegularExpressionLiteral"
            | "NoSubstitutionTemplateLiteral"
            | "TemplateHead"
            | "TemplateMiddle"
            | "TemplateTail"
            | "JsxText"
            | "JsxOpeningFragment"
            | "JsxClosingFragment" => {}

            // JSX
            "JsxElement" => {
                self.push_field(node, "openingElement", cx);
                self.push_list(node, "children", cx);
                self.push_field(node, "closingElement", cx);
            }
            "JsxFragment" => self.push_list(node, "children", cx),
            "JsxSelfClosingElement" | "JsxOpeningElement" | "JsxClosingElement" => {
                self.jsx_tag(node, cx);
                self.push_list(node, "typeArguments", cx.types());
                self.push_field(node, "attributes", cx);
            }
            "JsxAttributes" => self.push_list(node, "properties", cx),
            "JsxAttribute" => {
                self.jsx_name(field(node, "name"));
                self.push_field(node, "initializer", cx);
            }

            // Class and object members
            "PropertyDeclaration" | "PropertySignature" => {
                self.modifiers(node, cx.scope);
                self.member_name(node, cx);
                self.push_field(node, "type", cx.types());
                self.push_field(node, "initializer", cx);
            }
            "MethodDeclaration" | "GetAccessor" | "SetAccessor" | "Constructor"
            | "MethodSignature" => {
                self.modifiers(node, cx.scope);
                self.member_name(node, cx);
                self.function(node, cx, cx.scope, node.kind != "MethodSignature")?;
            }
            "CallSignature" | "ConstructSignature" | "IndexSignature" | "FunctionType"
            | "ConstructorType" => {
                self.modifiers(node, cx.scope);
                self.function(node, cx.types(), cx.scope, false)?;
            }
            "ClassStaticBlockDeclaration" => {
                self.modifiers(node, cx.scope);
                let scope = self.scope(ScopeKind::Body, Some(cx.scope));
                let inner = Cx {
                    var_scope: scope,
                    ..cx.nested(scope)
                };
                if let Some(body) = child(ast, node, "body") {
                    self.push_list(body, "statements", inner);
                }
            }
            "SemicolonClassElement" => {}

            // Types
            "TypeReference" => {
                self.entity(field(node, "typeName"), Path::TYPE, cx.types(), USE)?;
                self.push_list(node, "typeArguments", cx.types());
            }
            "TypeQuery" => {
                self.entity(field(node, "exprName"), Path::VALUE, cx.types(), USE)?;
                self.push_list(node, "typeArguments", cx.types());
            }
            "ImportType" => self.import_type(node, cx.types())?,
            "TypePredicate" => {
                if let Some(name) =
                    field(node, "parameterName").filter(|&name| ast.node(name).kind == "Identifier")
                {
                    self.reference(name, RefBase::Lexical(cx.scope), meaning::VALUE, cx, USE);
                }
                self.push_field(node, "type", cx.types());
            }
            "TypeLiteral" => self.push_list(node, "members", cx.types()),
            "NamedTupleMember" => {
                self.not_reference(field(node, "name"), "tuple member label");
                self.push_field(node, "type", cx.types());
            }
            "MappedType" => {
                let scope = self.scope(ScopeKind::TypeParameters, Some(cx.scope));
                let inner = cx.nested(scope).types();
                if let Some(parameter) = field(node, "typeParameter") {
                    self.lists += 1;
                    self.type_parameter(parameter, inner, scope);
                }
                self.push_field(node, "nameType", inner);
                self.push_field(node, "type", inner);
                self.push_list(node, "members", inner);
            }
            "ConditionalType" => {
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
            "InferType" => {
                // TypeScript binds a misplaced `infer` as an anonymous type
                // parameter that no lookup can see.
                let scope = match cx.infer {
                    Some(scope) => scope,
                    None => self.scope(ScopeKind::TypeParameters, Some(cx.scope)),
                };
                if let Some(parameter) = field(node, "typeParameter") {
                    // Each `infer` may repeat a name of its conditional type.
                    self.lists += 1;
                    let decl = self.facts.decls.len() as u32;
                    let constraint =
                        self.scope(ScopeKind::InferConstraint { decl }, Some(cx.scope));
                    self.type_parameter_in(parameter, cx.types(), scope, constraint);
                }
            }
            "ArrayType"
            | "OptionalType"
            | "RestType"
            | "ParenthesizedType"
            | "TypeOperator"
            | "JSDocNullableType"
            | "JSDocNonNullableType" => {
                self.push_field(node, "elementType", cx.types());
                self.push_field(node, "type", cx.types());
            }
            "TupleType" => self.push_list(node, "elements", cx.types()),
            "UnionType" | "IntersectionType" => self.push_list(node, "types", cx.types()),
            "IndexedAccessType" => {
                self.push_field(node, "objectType", cx.types());
                self.push_field(node, "indexType", cx.types());
            }
            "LiteralType" => self.push_field(node, "literal", cx),
            "ThisType" | "JSDocAllType" => {}
            kind if kind.ends_with("Keyword") || kind.ends_with("Token") => {}
            _ => return Err(self.unsupported(node, "syntax")),
        }
        Ok(())
    }

    /// Record `@ts-ignore` and `@ts-expect-error` comments in the leading
    /// trivia at `start`, which holds only whitespace and real comments. As
    /// in TypeScript, a block comment's directive is on its last line.
    fn directives(&mut self, start: usize) {
        if !self.trivia.insert(start) {
            return;
        }
        let end = token_span(
            self.text,
            Span {
                start,
                end: self.text.len(),
            },
        )
        .start;
        let mut rest = &self.text[start..end];
        let mut offset = start;
        loop {
            let trimmed = rest.trim_start();
            offset += rest.len() - trimmed.len();
            rest = trimmed;
            let (comment, body) = if let Some(body) = rest.strip_prefix("//") {
                let length = rest
                    .find(['\n', '\r', '\u{2028}', '\u{2029}'])
                    .unwrap_or(rest.len());
                (length, body.trim_start_matches('/'))
            } else if let Some(body) = rest.strip_prefix("/*") {
                let Some(close) = body.find("*/") else { break };
                let last_line = match rest[..close + 2].rfind(['\n', '\r', '\u{2028}', '\u{2029}'])
                {
                    Some(at) => &rest[at..close + 4],
                    None => &rest[..close + 4],
                };
                let body = last_line.trim_start_matches(|c: char| c.is_whitespace());
                (close + 4, body.trim_start_matches(['/', '*']))
            } else {
                break;
            };
            let body = body.trim_start();
            if body.starts_with("@ts-ignore") || body.starts_with("@ts-expect-error") {
                self.facts.directives.push((offset + comment) as u32);
            }
            offset += comment;
            rest = &rest[comment..];
        }
    }

    /// Visit decorators in `scope`: where a class is declared, or the class
    /// body for members and parameters.
    fn modifiers(&mut self, node: &Node<'s>, scope: u32) {
        let decorators: Vec<NodeId> = ids(node, "modifiers")
            .iter()
            .copied()
            .filter(|&id| self.ast.node(id).kind == "Decorator")
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
            // A `var` in a nested block is not a statement of the container.
            let binding_cx = if lexical == cx.scope {
                cx
            } else {
                cx.nested(cx.scope)
            };
            let first = self.facts.decls.len();
            self.binding(
                statement,
                field(declaration, "name"),
                kind,
                binding_cx,
                lexical,
            )?;
            if lexical != cx.scope {
                let mut hoisted = Vec::new();
                let mut scope = Some(cx.scope);
                while let Some(current) = scope.filter(|&current| current != lexical) {
                    hoisted.push(current);
                    scope = self.facts.scopes[current as usize].parent;
                }
                for decl in &mut self.facts.decls[first..] {
                    decl.hoisted.clone_from(&hoisted);
                }
            }
            self.push_field(declaration, "type", cx.nested(cx.scope).types());
            self.push_field(declaration, "initializer", cx.nested(cx.scope));
        }
        Ok(())
    }

    /// Parameters, type parameters, return type, and body of a function-like
    /// node. `outer` is the scope enclosing its signature.
    fn function(&mut self, node: &Node<'s>, cx: Cx, outer: u32, arguments: bool) -> Result<()> {
        let signature = self.scope(
            ScopeKind::Signature {
                arguments: arguments && node.kind != "ArrowFunction",
            },
            Some(outer),
        );
        let inner = cx.nested(signature);
        self.type_parameters(node, inner.types());
        for &parameter in ids(node, "parameters") {
            let parameter = self.ast.node(parameter);
            self.modifiers(parameter, outer);
            let name = field(parameter, "name");
            let this = name.is_some_and(|name| {
                let name = self.ast.node(name);
                name.kind == "Identifier" && text(name) == "this"
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
                ..cx.nested(scope)
            };
            if body.kind == "Block" {
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
    fn type_parameter(&mut self, id: NodeId, cx: Cx, scope: u32) {
        self.type_parameter_in(id, cx, scope, scope);
    }

    /// Declare a type parameter in `scope`, visiting its constraint and
    /// default in `constraints`.
    fn type_parameter_in(&mut self, id: NodeId, cx: Cx, scope: u32, constraints: u32) {
        let node = self.ast.node(id);
        if let Some(name) = field(node, "name") {
            let decl = self.facts.decls.len() as u32;
            let token = self.name(name, NameTarget::Decl(decl), DECLARATION);
            self.facts.names[token as usize].candidate = true;
            let span = self.facts.names[token as usize].span;
            self.declare(Decl {
                name: self.decoded(name),
                anchor: span,
                kind: DeclKind::TypeParameter,
                lexical: Some(scope),
                export: None,
                deferred: cx.deferred,
                list: self.lists,
                ..Decl::default()
            });
        }
        // Constraints see the parameters of their own list.
        let inner = cx.nested(constraints).types();
        self.push_field(node, "constraint", inner);
        self.push_field(node, "default", inner);
    }

    fn class(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        self.modifiers(node, cx.scope);
        let name = field(node, "name");
        let decl = if node.kind == "ClassDeclaration" {
            if name.is_some() || has_modifier(self.ast, node, "DefaultKeyword") {
                let decl = self.declare_statement(node, name, DeclKind::Class, cx, cx.scope)?;
                self.facts.decls[decl as usize].type_parameters =
                    Some(self.type_parameter_names(node));
                Some(decl)
            } else {
                None
            }
        } else {
            None
        };
        let mut outer = cx.scope;
        let decl = match (decl, name) {
            (Some(decl), _) => Some(decl),
            (None, Some(name)) => {
                outer = self.scope(ScopeKind::ExpressionName, Some(cx.scope));
                let decl = self.facts.decls.len() as u32;
                let token = self.name(name, NameTarget::Decl(decl), DECLARATION);
                self.facts.names[token as usize].candidate = true;
                let span = self.facts.names[token as usize].span;
                Some(self.declare(Decl {
                    name: self.decoded(name),
                    anchor: span,
                    kind: DeclKind::Class,
                    lexical: Some(outer),
                    export: None,
                    deferred: cx.deferred,
                    members: Vec::new(),
                    ..Decl::default()
                }))
            }
            (None, None) => None,
        };
        let kind = match decl {
            Some(decl) => ScopeKind::Members { decl },
            None => ScopeKind::TypeParameters,
        };
        let scope = self.scope(kind, Some(outer));
        let inner = Cx {
            type_context: false,
            ..cx.nested(scope)
        };
        self.type_parameters(node, inner.types());
        self.heritage(node, inner, true)?;
        self.push_list(node, "members", inner);
        Ok(())
    }

    /// `extends` of a class is an expression; `implements` and interface
    /// `extends` name types.
    fn heritage(&mut self, node: &Node<'s>, cx: Cx, class: bool) -> Result<()> {
        const EXTENDS_KEYWORD: u32 = 96;
        for &clause in ids(node, "heritageClauses") {
            let clause = self.ast.node(clause);
            let value = class && super::syntax::number(clause, "token") == Some(EXTENDS_KEYWORD);
            for &ty in ids(clause, "types") {
                let ty = self.ast.node(ty);
                if value {
                    self.push_field(ty, "expression", cx);
                } else {
                    self.entity(field(ty, "expression"), Path::TYPE, cx.types(), USE)?;
                }
                self.push_list(ty, "typeArguments", cx.types());
            }
        }
        Ok(())
    }

    /// A member name is not a lexical reference; a computed key is evaluated.
    fn member_name(&mut self, node: &Node<'s>, cx: Cx) {
        match child(self.ast, node, "name").map(|name| name.kind) {
            Some("ComputedPropertyName") => self.push_field(node, "name", cx),
            Some("Identifier") | Some("PrivateIdentifier") => {
                self.not_reference(field(node, "name"), "member")
            }
            _ => {}
        }
    }

    /// An identifier, qualified name, or property-access chain, with the
    /// meanings `path` asks of each part. Returns the last part's reference.
    fn entity(&mut self, id: Option<NodeId>, path: Path, cx: Cx, roles: u8) -> Result<Option<u32>> {
        let Some(id) = id else { return Ok(None) };
        let mut parts = Vec::new();
        let mut current = id;
        loop {
            let node = self.ast.node(current);
            match node.kind {
                "Identifier" => break,
                "QualifiedName" => {
                    parts.push((field(node, "right").unwrap(), false));
                    current = field(node, "left").unwrap();
                }
                "PropertyAccessExpression" => {
                    parts.push((field(node, "name").unwrap(), true));
                    current = field(node, "expression").unwrap();
                }
                _ => {
                    // `this.x`, calls, and other non-entity heads.
                    self.push(Some(current), cx);
                    for (part, _) in parts {
                        self.not_reference(Some(part), "property");
                    }
                    return Ok(None);
                }
            }
        }
        let head = if parts.is_empty() {
            path.single
        } else {
            path.head
        };
        let mut reference = self.reference(current, RefBase::Lexical(cx.scope), head, cx, roles);
        for (index, &(part, property)) in parts.iter().rev().enumerate() {
            if self.ast.node(part).kind == "PrivateIdentifier" {
                for &(rest, _) in parts.iter().rev().skip(index) {
                    self.not_reference(Some(rest), "private member");
                }
                return Ok(None);
            }
            let meaning = if index + 1 == parts.len() {
                path.last
            } else {
                path.middle
            };
            reference = self.reference(part, RefBase::Member(reference), meaning, cx, roles);
            // Value paths, including `typeof a.b`, access properties.
            self.facts.refs[reference as usize].property =
                property || path.middle == meaning::VALUE;
        }
        Ok(Some(reference))
    }

    /// `a.b.c` resolves `b` and `c` through namespaces when `a` is one.
    fn property_access(&mut self, id: NodeId, cx: Cx, roles: u8) {
        let _ = self.entity(Some(id), Path::VALUE, cx, roles);
    }

    fn jsx_tag(&mut self, node: &Node<'s>, cx: Cx) {
        let Some(tag) = field(node, "tagName") else {
            return;
        };
        let tag_node = self.ast.node(tag);
        match tag_node.kind {
            "Identifier" => {
                let name = text(tag_node);
                let intrinsic =
                    name.starts_with(|c: char| c.is_ascii_lowercase()) || name.contains('-');
                if intrinsic {
                    self.not_reference(Some(tag), "intrinsic element");
                } else {
                    self.reference(tag, RefBase::Lexical(cx.scope), meaning::VALUE, cx, USE);
                }
            }
            "JsxNamespacedName" => self.jsx_name(Some(tag)),
            _ => self.push(Some(tag), cx),
        }
    }

    fn jsx_name(&mut self, id: Option<NodeId>) {
        let Some(id) = id else { return };
        let node = self.ast.node(id);
        if node.kind == "JsxNamespacedName" {
            self.not_reference(field(node, "namespace"), "JSX namespace");
            self.not_reference(field(node, "name"), "JSX attribute");
        } else {
            self.not_reference(Some(id), "JSX attribute");
        }
    }

    fn import_type(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        let literal = child(self.ast, node, "argument")
            .and_then(|argument| child(self.ast, argument, "literal"))
            .filter(|literal| literal.kind == "StringLiteral");
        let request = literal.map(|literal| self.request(literal, RequestKind::ImportType, node));
        if literal.is_none() {
            self.push_field(node, "argument", cx);
        }
        self.import_attributes(node, cx);
        let last = if flag(node, "isTypeOf") {
            meaning::VALUE
        } else {
            meaning::TYPE
        };
        let mut parts = Vec::new();
        let mut current = field(node, "qualifier");
        while let Some(part) = current {
            let part_node = self.ast.node(part);
            if part_node.kind == "QualifiedName" {
                parts.push(field(part_node, "right").unwrap());
                current = field(part_node, "left");
            } else {
                parts.push(part);
                current = None;
            }
        }
        let mut reference = None;
        for (index, part) in parts.iter().rev().enumerate() {
            let meaning = if index + 1 == parts.len() {
                last
            } else if last == meaning::VALUE {
                meaning::VALUE
            } else {
                meaning::NAMESPACE
            };
            let base = match (reference, request) {
                (Some(reference), _) => RefBase::Member(reference),
                (None, Some(request)) => RefBase::Module(request),
                (None, None) => {
                    self.not_reference(Some(*part), "property");
                    continue;
                }
            };
            reference = Some(self.reference(*part, base, meaning, cx, USE));
        }
        self.push_list(node, "typeArguments", cx);
        Ok(())
    }

    fn import_attributes(&mut self, node: &Node<'s>, cx: Cx) {
        if let Some(attributes) = child(self.ast, node, "attributes") {
            for &attribute in ids(attributes, "elements") {
                let attribute = self.ast.node(attribute);
                self.not_reference(field(attribute, "name"), "import attribute");
                self.push(field(attribute, "value"), cx.nested(cx.scope));
            }
        }
    }

    fn alias(&mut self, kind: AliasKind, bind: AliasBind, type_only: bool, cx: Cx) -> u32 {
        self.facts.aliases.push(Alias {
            kind,
            bind,
            type_only,
            deferred: cx.deferred,
            span: Span { start: 0, end: 0 },
            export_only: false,
        });
        (self.facts.aliases.len() - 1) as u32
    }

    /// Create an export specifier, `export * as`, or export assignment alias.
    fn export_alias(&mut self, kind: AliasKind, bind: AliasBind, type_only: bool, cx: Cx) -> u32 {
        let alias = self.alias(kind, bind, type_only, cx);
        self.facts.aliases[alias as usize].export_only = true;
        alias
    }

    fn alias_name(&mut self, id: Option<NodeId>, alias: u32, roles: u8, candidate: bool) {
        let Some(id) = id else { return };
        if self.ast.node(id).kind != "Identifier" {
            return;
        }
        let token = self.name(id, NameTarget::Alias(alias), roles);
        let name = &mut self.facts.names[token as usize];
        name.candidate = candidate;
        self.facts.aliases[alias as usize].span = name.span;
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
        );
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
                cx,
            );
            self.alias_name(Some(name), alias, IMPORT, true);
        }
        let Some(bindings) = child(self.ast, clause, "namedBindings") else {
            return Ok(());
        };
        if bindings.kind == "NamespaceImport" {
            let name = field(bindings, "name").unwrap();
            let alias = self.alias(
                AliasKind::Namespace { request },
                AliasBind::Lexical {
                    scope: cx.scope,
                    name: self.decoded(name),
                },
                type_only,
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
                    name: self.export_name(imported),
                },
                AliasBind::Lexical {
                    scope: cx.scope,
                    name: self.decoded(name),
                },
                type_only || flag(element, "isTypeOnly"),
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
            .filter(|specifier| specifier.kind == "StringLiteral")
            .ok_or_else(|| self.unsupported(node, "non-string module specifier"))
    }

    /// The decoded text of an identifier or string-literal module export name.
    fn export_name(&self, id: NodeId) -> String {
        let node = self.ast.node(id);
        if node.kind == "Identifier" {
            self.decoded(id)
        } else {
            text(node)
        }
    }

    fn import_equals(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        self.modifiers(node, cx.scope);
        let name = field(node, "name").unwrap();
        let type_only = flag(node, "isTypeOnly");
        let reference = child(self.ast, node, "moduleReference")
            .ok_or_else(|| self.unsupported(node, "import equals declaration"))?;
        let kind = if reference.kind == "ExternalModuleReference" {
            let literal = child(self.ast, reference, "expression")
                .filter(|literal| literal.kind == "StringLiteral")
                .ok_or_else(|| self.unsupported(node, "require target"))?;
            AliasKind::Namespace {
                request: self.request(literal, RequestKind::Require, node),
            }
        } else {
            let target = field(node, "moduleReference");
            let reference = self
                .entity(target, Path::IMPORT, cx, USE)?
                .ok_or_else(|| self.unsupported(node, "import equals target"))?;
            let expand = target.is_some_and(|target| self.ast.node(target).kind == "Identifier");
            AliasKind::Entity { reference, expand }
        };
        let exported = has_modifier(self.ast, node, "ExportKeyword");
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
        let alias = self.alias(kind, bind, type_only, cx);
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
            Some(self.request(specifier, RequestKind::ReExport, node))
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
        if clause.kind == "NamespaceExport" {
            let request =
                request.ok_or_else(|| self.unsupported(node, "namespace export without module"))?;
            let name = field(clause, "name").unwrap();
            let alias = self.export_alias(
                AliasKind::Namespace { request },
                AliasBind::Export {
                    host,
                    name: self.export_name(name),
                },
                type_only,
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
                    name: self.export_name(local),
                },
                None => AliasKind::Lexical {
                    scope: cx.scope,
                    name: self.export_name(local),
                },
            };
            let alias = self.export_alias(
                kind,
                AliasBind::Export {
                    host,
                    name: self.export_name(name),
                },
                type_only || flag(element, "isTypeOnly"),
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
        let entity = expression.is_some_and(|expression| is_entity(self.ast, expression));
        if entity {
            let expression = expression.unwrap();
            let identifier = self.ast.node(expression).kind == "Identifier";
            let kind = if identifier {
                AliasKind::Lexical {
                    scope: cx.scope,
                    name: self.decoded(expression),
                }
            } else {
                let reference = self
                    .entity(Some(expression), Path::EXPORT, cx, USE)?
                    .expect("entity names have a reference");
                let token = self.facts.refs[reference as usize].token;
                self.facts.names[token as usize].roles |= EXPORT;
                AliasKind::Entity {
                    reference,
                    expand: false,
                }
            };
            let alias = self.export_alias(
                kind,
                AliasBind::Export {
                    host,
                    name: name.into(),
                },
                false,
                cx,
            );
            if identifier {
                self.alias_name(Some(expression), alias, EXPORT, false);
            }
        } else {
            let anchor = default_keyword(self.ast, self.text, node);
            self.declare(Decl {
                name: name.into(),
                anchor,
                kind: DeclKind::Expression,
                lexical: None,
                export: Some((host, name.into())),
                deferred: cx.deferred,
                members: Vec::new(),
                ..Decl::default()
            });
            self.push(expression, cx.nested(cx.scope));
        }
        Ok(())
    }

    fn module_declaration(&mut self, node: &Node<'s>, cx: Cx) -> Result<()> {
        self.modifiers(node, cx.scope);
        let name = child(self.ast, node, "name").unwrap();
        let ambient = node.flags & node_flags::AMBIENT != 0;
        let body = child(self.ast, node, "body");
        let statements = body
            .filter(|body| body.kind == "ModuleBlock")
            .map_or(&[][..], |body| ids(body, "statements"));
        let export_context = ambient && !has_export_declarations(self.ast, statements);
        if name.kind == "StringLiteral" {
            let (scope_kind, host) = if self.module {
                let request = self.request(name, RequestKind::Augmentation, node);
                (
                    ScopeKind::Augmentation { request },
                    Host::Augmentation(request),
                )
            } else {
                self.facts.ambient_modules.push(AmbientModule {
                    name: text(name),
                    span: name.span,
                    shorthand: body.is_none(),
                });
                let module = (self.facts.ambient_modules.len() - 1) as u32;
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
            self.push_all(statements, inner);
            return Ok(());
        }
        if node.flags & node_flags::GLOBAL_AUGMENTATION != 0 {
            self.not_reference(field(node, "name"), "global augmentation");
            let scope = self.scope(ScopeKind::Global, Some(cx.scope));
            let inner = Cx {
                scope,
                var_scope: scope,
                container: None,
                ..cx
            };
            self.push_all(statements, inner);
            return Ok(());
        }
        let instantiated = body.is_none_or(|body| instantiated(self.ast, body));
        let decl = self.declare_statement(
            node,
            field(node, "name"),
            DeclKind::Namespace { instantiated },
            cx,
            cx.scope,
        )?;
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
        match body {
            Some(body) if body.kind == "ModuleDeclaration" => {
                self.module_declaration(body, inner)?
            }
            _ => self.push_all(statements, inner),
        }
        Ok(())
    }
}

/// TypeScript's module instance state: whether a namespace body creates a
/// runtime value.
fn instantiated<'s>(ast: &Ast<'s>, body: &Node<'s>) -> bool {
    let mut stack = vec![body];
    let mut values: HashSet<String> = HashSet::new();
    while let Some(node) = stack.pop() {
        match node.kind {
            "ModuleBlock" => {
                let statements = ids(node, "statements");
                values.extend(
                    statements
                        .iter()
                        .flat_map(|&id| value_names(ast, ast.node(id))),
                );
                stack.extend(statements.iter().map(|&id| ast.node(id)));
            }
            "ModuleDeclaration" => match child(ast, node, "body") {
                Some(body) => stack.push(body),
                None => return true,
            },
            "InterfaceDeclaration" | "TypeAliasDeclaration" => {}
            "ImportDeclaration" | "ImportEqualsDeclaration"
                if !has_modifier(ast, node, "ExportKeyword") => {}
            // Exporting only names that are not values of the block, such as
            // types, does not instantiate it.
            "ExportDeclaration"
                if field(node, "moduleSpecifier").is_none()
                    && child(ast, node, "exportClause").is_some_and(|clause| {
                        clause.kind == "NamedExports"
                            && ids(clause, "elements").iter().all(|&element| {
                                let element = ast.node(element);
                                let local =
                                    field(element, "propertyName").or(field(element, "name"));
                                !local.is_some_and(|local| values.contains(&text(ast.node(local))))
                            })
                    }) => {}
            _ => return true,
        }
    }
    false
}

/// Names a statement declares as runtime values.
fn value_names<'s>(ast: &Ast<'s>, node: &Node<'s>) -> Vec<String> {
    match node.kind {
        "VariableStatement" => child(ast, node, "declarationList")
            .map(|list| {
                ids(list, "declarations")
                    .iter()
                    .filter_map(|&declaration| child(ast, ast.node(declaration), "name"))
                    .filter(|name| name.kind == "Identifier")
                    .map(text)
                    .collect()
            })
            .unwrap_or_default(),
        "FunctionDeclaration"
        | "ClassDeclaration"
        | "EnumDeclaration"
        | "ModuleDeclaration"
        | "ImportEqualsDeclaration" => child(ast, node, "name").map(text).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn has_export_declarations<'s>(ast: &Ast<'s>, statements: &[NodeId]) -> bool {
    statements
        .iter()
        .any(|&id| matches!(ast.node(id).kind, "ExportDeclaration" | "ExportAssignment"))
}

/// An identifier or a property-access chain of identifiers.
fn is_entity<'s>(ast: &Ast<'s>, mut id: NodeId) -> bool {
    loop {
        let node = ast.node(id);
        match node.kind {
            "Identifier" => return true,
            "PropertyAccessExpression"
                if child(ast, node, "name").is_some_and(|name| name.kind == "Identifier") =>
            {
                id = field(node, "expression").unwrap();
            }
            _ => return false,
        }
    }
}

/// The anchor of an anonymous export: the `default` keyword, or the `=` of
/// `export =`.
fn default_keyword<'s>(ast: &Ast<'s>, text: &str, node: &Node<'s>) -> Span {
    if let Some(&modifier) = ids(node, "modifiers")
        .iter()
        .find(|&&id| ast.node(id).kind == "DefaultKeyword")
    {
        return token_span(text, ast.node(modifier).span);
    }
    let mut lexer = crate::lexer::Lexer {
        src: text,
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
        if text(name) != "resolution-mode" {
            return None;
        }
        match text(child(ast, attribute, "value")?).as_str() {
            "import" => Some(ResolutionMode::Import),
            "require" => Some(ResolutionMode::Require),
            _ => None,
        }
    })
}
