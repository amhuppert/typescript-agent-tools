//! Declaration projection: one final syntax tree to its declaration sites.
//!
//! This module alone knows syntax: which nodes are declaration sites, their
//! kinds, names, containment, anchors, ranges, excerpts, and signatures.
//! Selection and rendering see only `Record`s.
//!
//! Inclusion, containment, and labels port TypeScript 7.0.2's document
//! symbols (`getDocumentSymbolsForChildren`, `newDocumentSymbol`, and
//! `getUnnamedNodeLabel` in internal/ls/symbols.go). Product rules, marked
//! where they apply, keep every syntax site separately selectable: no merging
//! (`mergeExpandos`), no expando, shorthand, or spread entries (their
//! contents are transparent, so declarations inside them remain sites), and
//! no entries from parameters, types, decorators, or heritage clauses.
//!
//! Every helper is bounded by the input, not by native recursion: the walk
//! follows only the final tree from its root with an explicit work stack,
//! so nesting depth never becomes native recursion; a callback's callee
//! chain, which the parser reads in a loop and so may be as long as the
//! file, is walked in a loop (`names::callback`); and a signature scans its
//! source only until it is long enough (`signature::render`), finding the
//! bodies it elides on the way. Apart from rendering its output, such as a
//! label that repeats a long callee chain, a projection's work is linear in
//! the tree and its text. A selection projection (`Detail::Selection`)
//! renders no signature and no unnamed site's label, which its index
//! discards.

use super::Kind;
use super::names::{self, Name};
use super::signature::{self, Spec};
use crate::lexer::Lexer;
use crate::syntax::{self, has_modifier};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Span};
use smallvec::{SmallVec, smallvec};

/// One declaration site, in byte offsets of the projected source.
#[derive(Debug)]
pub(super) struct Record {
    /// The nearest enclosing site; records precede their descendants.
    pub(super) parent: Option<usize>,
    pub(super) depth: usize,
    pub(super) kind: Kind,
    /// Empty for an unnamed site in a selection projection.
    pub(super) label: String,
    /// The exact lookup key's segments, empty without a key. Only a dotted
    /// namespace (`namespace A.B.C`) has more than one.
    pub(super) key: SmallVec<[String; 1]>,
    pub(super) anchor: usize,
    /// Where `--at` selects this site: its name tokens, or its anchor token.
    pub(super) targets: SmallVec<[Span; 1]>,
    /// The excerpt `show` returns, from the first token through the last:
    /// the token span of `designation`.
    pub(super) range: Span,
    pub(super) signature: Option<String>,
    /// The site's own syntax, such as a variable declaration or method. An
    /// identity in the projected tree only: a record that outlives its tree
    /// holds it unread, and only `catalog::Syntax`, which holds the tree,
    /// lends it.
    pub(super) node: NodeId,
    /// The syntax the site designates as a whole, which its excerpt shows:
    /// a binding's variable statement (or a loop's declaration list, or a
    /// `catch` variable itself), a specifier's import or export declaration,
    /// and otherwise the site's own node. Held as `node` is.
    pub(super) designation: NodeId,
}

/// What a projection renders besides each site's selection facts: its
/// kind, containment, key, anchor, `--at` targets, and range, and the label
/// of a named site, which a selection index keeps when it differs from the
/// key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Detail {
    /// Nothing else: an index keeps only selection facts, so an unnamed
    /// site's label and every signature would be discarded.
    Selection,
    /// Every label and signature, for outlines and shows.
    Complete,
}

/// The declaration sites of `ast` in source order, in `detail`.
pub(super) fn project(ast: &Ast<'_>, detail: Detail) -> Vec<Record> {
    let mut projection = Projection {
        ast,
        detail,
        literals: ast.literal_token_spans(),
        records: Vec::new(),
        work: Vec::new(),
    };
    projection.children(ast.root_id(), None, None);
    while let Some(work) = projection.work.pop() {
        match work {
            Work::Visit(visit) => projection.visit(visit),
            Work::ParameterProperties {
                constructor,
                container,
            } => projection.parameter_properties(constructor, container),
        }
    }
    projection.records
}

struct Projection<'a, 'src> {
    ast: &'a Ast<'src>,
    detail: Detail,
    literals: Vec<Span>,
    records: Vec<Record>,
    work: Vec<Work>,
}

enum Work {
    Visit(Visit),
    /// A constructor's parameter properties, which follow it as siblings.
    ParameterProperties {
        constructor: NodeId,
        container: Option<usize>,
    },
}

#[derive(Clone, Copy)]
struct Visit {
    node: NodeId,
    parent: Option<NodeId>,
    /// The parent after skipping parenthesized expressions, as TypeScript's
    /// `WalkUpParenthesizedExpressions` finds it.
    outer: Option<NodeId>,
    container: Option<usize>,
    statement: Statement,
}

/// The statement a binding or specifier site designates, and so shows as
/// its excerpt.
#[derive(Clone, Copy)]
enum Statement {
    None,
    /// A variable statement, or a loop's declaration list, and where its
    /// declarations begin after the modifiers and keyword.
    Variables {
        node: NodeId,
        declarations: usize,
    },
    /// The whole import or export declaration of a specifier.
    Declaration(NodeId),
}

/// A site's naming: its label, key, anchor, and `--at` targets.
struct Naming {
    label: String,
    key: SmallVec<[String; 1]>,
    anchor: usize,
    targets: SmallVec<[Span; 1]>,
    /// Where abbreviation may begin in its signature.
    name_end: usize,
}

const FUNCTIONS: [SyntaxKind; 7] = [
    SyntaxKind::FunctionDeclaration,
    SyntaxKind::FunctionExpression,
    SyntaxKind::ArrowFunction,
    SyntaxKind::MethodDeclaration,
    SyntaxKind::GetAccessor,
    SyntaxKind::SetAccessor,
    SyntaxKind::Constructor,
];

/// Fields never traversed for entries: product rule. They contribute to
/// signatures instead. TypeScript visits them inside initializers and class
/// headers, which lists, for example, destructured arrow parameters and type
/// literal members; one rule for every site is easier to predict.
const UNTRAVERSED: [&str; 6] = [
    "modifiers",
    "type",
    "typeArguments",
    "typeParameters",
    "parameters",
    "heritageClauses",
];

impl<'a, 'src> Projection<'a, 'src> {
    /// Visit the children of `node` that may hold declarations, in source
    /// order: a function's body only, a class-like's members only, and any
    /// other node's children outside `UNTRAVERSED`. This is the compiler's
    /// `ForEachChild(node)` under that product rule.
    fn children(&mut self, node: NodeId, outer: Option<NodeId>, container: Option<usize>) {
        let kind = self.ast.node(node).syntax_kind;
        let child_outer = if kind == SyntaxKind::ParenthesizedExpression {
            outer
        } else {
            Some(node)
        };
        let only = if FUNCTIONS.contains(&kind) {
            Some("body")
        } else if matches!(
            kind,
            SyntaxKind::ClassDeclaration
                | SyntaxKind::ClassExpression
                | SyntaxKind::InterfaceDeclaration
                | SyntaxKind::EnumDeclaration
        ) {
            Some("members")
        } else {
            None
        };
        let start = self.work.len();
        for (field, value) in self.ast.logical_fields(node) {
            if only.map_or(UNTRAVERSED.contains(&field), |only| only != field) {
                continue;
            }
            let ids = match value {
                crate::Value::Node(id) => std::slice::from_ref(id),
                crate::Value::Nodes(ids) => ids.as_slice(),
                _ => &[],
            };
            for &child in ids {
                self.work.push(Work::Visit(Visit {
                    node: child,
                    parent: Some(node),
                    outer: child_outer,
                    container,
                    statement: Statement::None,
                }));
            }
        }
        self.work[start..].reverse();
    }

    /// Visit one child of `v` that is not a parenthesized expression's.
    fn visit_node(
        &mut self,
        v: &Visit,
        node: NodeId,
        container: Option<usize>,
        statement: Statement,
    ) {
        self.work.push(Work::Visit(Visit {
            node,
            parent: Some(v.node),
            outer: Some(v.node),
            container,
            statement,
        }));
    }

    /// TypeScript's `visit` in `getDocumentSymbolsForChildren`: an entry for
    /// declaration syntax with children from its designated contents, and
    /// transparent traversal of everything else.
    fn visit(&mut self, v: Visit) {
        use SyntaxKind::*;
        let node = self.ast.node(v.node);
        match node.syntax_kind {
            ClassDeclaration | ClassExpression | InterfaceDeclaration | EnumDeclaration => {
                let container = self.site(&v, self.open_brace(v.node));
                self.children(v.node, v.outer, container.or(v.container));
            }
            FunctionDeclaration | FunctionExpression | ArrowFunction | MethodDeclaration
            | GetAccessor | SetAccessor | Constructor => {
                let body = syntax::id(node, "body");
                let header = body.map(|body| self.own_range(body).start);
                let container = self.site(&v, header);
                if node.syntax_kind == Constructor {
                    self.work.push(Work::ParameterProperties {
                        constructor: v.node,
                        container: v.container,
                    });
                }
                if let Some(body) = body {
                    self.children(body, Some(v.node), container.or(v.container));
                }
            }
            MethodSignature
            | PropertySignature
            | CallSignature
            | ConstructSignature
            | IndexSignature
            | EnumMember
            | TypeAliasDeclaration
            | ImportEqualsDeclaration => {
                self.site(&v, None);
            }
            ModuleDeclaration => self.module(v),
            VariableDeclaration | BindingElement | PropertyAssignment | PropertyDeclaration => {
                self.binding(v)
            }
            ExportAssignment if syntax::flag(node, "isExportEquals") => self.export_equals(v),
            ImportClause => self.import_clause(v),
            ExportSpecifier => {
                if let Some(name) = syntax::id(node, "name") {
                    let designation = self.statement_of(&v);
                    let renames = syntax::id(node, "propertyName").is_some();
                    let naming = self.alias_naming(name, renames);
                    self.push(&v, Kind::Export, naming, designation, None);
                }
            }
            // The statements a binding or specifier site shows as its excerpt.
            VariableStatement => {
                if let Some(list) = syntax::id(node, "declarationList") {
                    let statement = Statement::Variables {
                        node: v.node,
                        declarations: self.declarations_start(list),
                    };
                    self.visit_node(&v, list, v.container, statement);
                }
            }
            VariableDeclarationList => {
                let statement = match v.statement {
                    Statement::Variables { .. } => v.statement,
                    // A loop's declarations have no statement of their own.
                    _ => Statement::Variables {
                        node: v.node,
                        declarations: self.declarations_start(v.node),
                    },
                };
                self.visit_list(&v, "declarations", statement);
            }
            ObjectBindingPattern | ArrayBindingPattern | NamedExports => {
                self.visit_list(&v, "elements", v.statement);
            }
            ImportDeclaration | ExportDeclaration => {
                let statement = Statement::Declaration(v.node);
                for field in ["importClause", "exportClause"] {
                    if let Some(clause) = syntax::id(node, field) {
                        self.visit_node(&v, clause, v.container, statement);
                    }
                }
            }
            // Product rule: shorthand properties and spreads, which
            // TypeScript lists as leaves without contents, have no entry and
            // are transparent like any other syntax: a spread's expression
            // and a shorthand's default in a destructuring assignment
            // contribute their declarations to the enclosing entry.
            _ => self.children(v.node, v.outer, v.container),
        }
    }

    /// The kind of a site its syntax alone decides: every site except
    /// modules, import clauses, and exports, whose visitors choose theirs.
    fn kind(&self, node: NodeId) -> Kind {
        use SyntaxKind::*;
        match self.ast.node(node).syntax_kind {
            FunctionDeclaration | FunctionExpression | ArrowFunction => Kind::Function,
            ClassDeclaration | ClassExpression => Kind::Class,
            InterfaceDeclaration => Kind::Interface,
            EnumDeclaration => Kind::Enum,
            EnumMember => Kind::EnumMember,
            TypeAliasDeclaration => Kind::Type,
            MethodDeclaration | MethodSignature => Kind::Method,
            GetAccessor | SetAccessor => Kind::Accessor,
            Constructor => Kind::Constructor,
            CallSignature | ConstructSignature | IndexSignature => Kind::Signature,
            PropertySignature | PropertyDeclaration | PropertyAssignment | Parameter => {
                Kind::Property
            }
            VariableDeclaration | BindingElement => Kind::Variable,
            ImportEqualsDeclaration => Kind::Import,
            other => unreachable!("{other:?} is not a declaration site"),
        }
    }

    /// A site with its own range, named by its `name` or else by
    /// `getUnnamedNodeLabel`, and its record when it has one.
    fn site(&mut self, v: &Visit, header: Option<usize>) -> Option<usize> {
        let naming = self.naming(v)?;
        let signature = self.signature_of(v, &naming, header);
        Some(self.push(v, self.kind(v.node), naming, v.node, signature))
    }

    /// A namespace, ambient module, or global augmentation. A dotted
    /// namespace is one site whose children are the innermost body's.
    fn module(&mut self, v: Visit) {
        let ast = self.ast;
        let mut interior = v.node;
        let mut names: Vec<NodeId> = syntax::id(ast.node(v.node), "name").into_iter().collect();
        while let Some(body) = syntax::id(ast.node(interior), "body")
            .filter(|&body| ast.node(body).syntax_kind == SyntaxKind::ModuleDeclaration)
        {
            names.extend(syntax::id(ast.node(body), "name"));
            interior = body;
        }
        let Some(&first) = names.first() else {
            return;
        };
        let kind = if ast.node(first).syntax_kind == SyntaxKind::StringLiteral
            || ast.node(v.node).flags & crate::compiler::node::GLOBAL_AUGMENTATION != 0
        {
            Kind::Module
        } else {
            Kind::Namespace
        };
        let naming = self.module_naming(&names);
        let header = syntax::id(ast.node(interior), "body").map(|body| self.own_range(body).start);
        let signature = self.signature_of(&v, &naming, header);
        let record = self.push(&v, kind, naming, v.node, signature);
        self.children(interior, Some(v.node), Some(record));
    }

    /// A variable, binding element, or property: one site whose children
    /// are its initializer's contents, or, for a binding pattern, one site
    /// per identifier binding and no initializer (as in TypeScript).
    fn binding(&mut self, v: Visit) {
        let ast = self.ast;
        let node = ast.node(v.node);
        let Some(name) = syntax::id(node, "name") else {
            return;
        };
        let statement = match (node.syntax_kind, v.statement) {
            // A `catch` clause's variable has no list or statement.
            (SyntaxKind::VariableDeclaration, Statement::None) => Statement::Variables {
                node: v.node,
                declarations: self.own_range(v.node).start,
            },
            (_, statement) => statement,
        };
        if matches!(
            ast.node(name).syntax_kind,
            SyntaxKind::ObjectBindingPattern | SyntaxKind::ArrayBindingPattern
        ) {
            return self.visit_node(&v, name, v.container, statement);
        }
        let naming = self.named(name);
        let (designation, signature) = match (self.kind(v.node), statement) {
            (
                Kind::Variable,
                Statement::Variables {
                    node: designation,
                    declarations,
                },
            ) => {
                let range = self.own_range(designation);
                // Product rule: a pattern's binding has no signature of its
                // own; a declaration's shows its statement's keyword.
                let signature = (self.detail == Detail::Complete
                    && node.syntax_kind == SyntaxKind::VariableDeclaration)
                    .then(|| {
                        let mut parts: SmallVec<[Span; 2]> = SmallVec::new();
                        if declarations > range.start {
                            parts.push(Span {
                                start: range.start,
                                end: declarations,
                            });
                        }
                        parts.push(self.own_range(v.node));
                        self.render(
                            v.node,
                            Spec {
                                parts,
                                name_end: naming.name_end,
                            },
                        )
                    });
                (designation, signature)
            }
            _ => (v.node, self.signature_of(&v, &naming, None)),
        };
        let record = self.push(&v, self.kind(v.node), naming, designation, signature);
        if let Some(initializer) = syntax::id(node, "initializer") {
            self.children(initializer, Some(v.node), Some(record));
        }
    }

    /// `export =`: a site labelled `export=` whose children come from its
    /// expression itself, anchored at an identifier expression or else at
    /// the statement (`newDocumentSymbol`).
    fn export_equals(&mut self, v: Visit) {
        let ast = self.ast;
        let expression = syntax::id(ast.node(v.node), "expression");
        let target = match expression {
            Some(expression) if ast.node(expression).syntax_kind == SyntaxKind::Identifier => {
                ast.token_span(expression)
            }
            _ => self.first_token(v.node),
        };
        let naming = Naming {
            label: "export=".into(),
            key: SmallVec::new(),
            anchor: target.start,
            targets: smallvec![target],
            name_end: self.own_range(v.node).start,
        };
        let signature = self.signature_of(&v, &naming, None);
        let record = self.push(&v, Kind::Export, naming, v.node, signature);
        if let Some(expression) = expression {
            self.visit_node(&v, expression, Some(record), Statement::None);
        }
    }

    /// The default import, namespace import, and named imports of a clause,
    /// each a site showing the whole import declaration. Which of them have
    /// keys is `alias_naming`'s rule: the namespace import and renaming
    /// specifiers.
    fn import_clause(&mut self, v: Visit) {
        let ast = self.ast;
        let node = ast.node(v.node);
        let mut names: Vec<(NodeId, bool)> = syntax::id(node, "name")
            .map(|name| (name, false))
            .into_iter()
            .collect();
        if let Some(bindings) = syntax::id(node, "namedBindings").map(|id| ast.node(id)) {
            if bindings.syntax_kind == SyntaxKind::NamespaceImport {
                names.extend(syntax::id(bindings, "name").map(|name| (name, true)));
            } else {
                names.extend(
                    syntax::ids(bindings, "elements")
                        .iter()
                        .filter_map(|&element| {
                            let element = ast.node(element);
                            let renames = syntax::id(element, "propertyName").is_some();
                            syntax::id(element, "name").map(|name| (name, renames))
                        }),
                );
            }
        }
        let designation = self.statement_of(&v);
        for (name, keyed) in names {
            let naming = self.alias_naming(name, keyed);
            self.push(&v, Kind::Import, naming, designation, None);
        }
    }

    /// Visit the nodes of `v`'s list `field` within `v`'s container.
    fn visit_list(&mut self, v: &Visit, field: &str, statement: Statement) {
        for &item in syntax::ids(self.ast.node(v.node), field).iter().rev() {
            self.visit_node(v, item, v.container, statement);
        }
    }

    /// TypeScript's `IsParameterPropertyDeclaration`: a constructor
    /// parameter with an accessibility, `readonly`, or `override` modifier.
    fn parameter_properties(&mut self, constructor: NodeId, container: Option<usize>) {
        let ast = self.ast;
        for &parameter in syntax::ids(ast.node(constructor), "parameters") {
            let node = ast.node(parameter);
            let property = [
                SyntaxKind::PublicKeyword,
                SyntaxKind::PrivateKeyword,
                SyntaxKind::ProtectedKeyword,
                SyntaxKind::ReadonlyKeyword,
                SyntaxKind::OverrideKeyword,
            ]
            .into_iter()
            .any(|modifier| has_modifier(ast, node, modifier));
            let Some(name) = syntax::id(node, "name").filter(|_| property) else {
                continue;
            };
            let v = Visit {
                node: parameter,
                parent: Some(constructor),
                outer: Some(constructor),
                container,
                statement: Statement::None,
            };
            let naming = self.named(name);
            let signature = self.signature_of(&v, &naming, None);
            self.push(&v, self.kind(parameter), naming, parameter, signature);
        }
    }

    /// Record the site of `v`, designating `designation`, whose token span
    /// is the site's excerpt.
    fn push(
        &mut self,
        v: &Visit,
        kind: Kind,
        naming: Naming,
        designation: NodeId,
        signature: Option<String>,
    ) -> usize {
        let depth = v
            .container
            .map_or(1, |parent| self.records[parent].depth + 1);
        self.records.push(Record {
            parent: v.container,
            depth,
            kind,
            label: naming.label,
            key: naming.key,
            anchor: naming.anchor,
            targets: naming.targets,
            range: self.own_range(designation),
            signature,
            node: v.node,
            designation,
        });
        self.records.len() - 1
    }

    /// The naming of a site with an optional `name`: TypeScript's
    /// `GetNameOfDeclaration`, then `getUnnamedNodeLabel`. `None` for a site
    /// TypeScript drops because it has no label.
    fn naming(&self, v: &Visit) -> Option<Naming> {
        use SyntaxKind::*;
        let node = self.ast.node(v.node);
        if let Some(name) = syntax::id(node, "name") {
            return Some(self.named(name));
        }
        let anchor = self.first_token(v.node);
        let label = match node.syntax_kind {
            FunctionExpression | ArrowFunction | ClassExpression => {
                match self.assigned_name(v) {
                    // The name lies outside the site, so like TypeScript's
                    // zero-width selection the anchor stays the site's first
                    // token, and the label is presentation only.
                    Some(name) => Unnamed::Assigned(name),
                    None => self.unnamed_label(v)?,
                }
            }
            _ => self.unnamed_label(v)?,
        };
        let label = match self.detail {
            Detail::Selection => String::new(),
            Detail::Complete => label.render(self.ast),
        };
        let mut targets: SmallVec<[Span; 1]> = smallvec![anchor];
        if node.syntax_kind == Constructor {
            // The parser keeps no name node for the `constructor` keyword
            // (or string), but it names the site as well as its anchor does.
            let after = syntax::ids(node, "modifiers")
                .last()
                .map_or(anchor.start, |&modifier| self.ast.node(modifier).span.end);
            let keyword = Lexer {
                src: self.ast.source(),
                pos: after,
            }
            .next();
            if keyword.start != anchor.start && keyword.end > keyword.start {
                targets.push(Span {
                    start: keyword.start,
                    end: keyword.end,
                });
            }
        }
        Some(Naming {
            label,
            key: SmallVec::new(),
            anchor: anchor.start,
            targets,
            name_end: anchor.start,
        })
    }

    /// A site named by `name`, which is also its anchor.
    fn named(&self, name: NodeId) -> Naming {
        let Name { label, key } = names::declared(self.ast, name);
        let target = self.ast.token_span(name);
        Naming {
            label,
            key: key.into_iter().collect(),
            anchor: target.start,
            targets: smallvec![target],
            name_end: target.end,
        }
    }

    /// An import or export site named by `name`, keyed only when `keyed`.
    /// Port of the specifier rule of TypeScript's `computeDeclarationMap`
    /// (internal/ast/ast.go), the declarations `ProvideWorkspaceSymbols`
    /// searches by name across a project: an import or export specifier is
    /// a declared name only when it renames, with a property name (`{ a as
    /// b }`, which declares `b`), and a default import's name never is,
    /// since `GetDeclarationName` finds no name on the identifier it adds.
    /// A namespace import and `import x =` keep their keys. So a name
    /// selects the declaration an import or a re-export without renaming
    /// refers to, not every file that mentions it; such a site keeps its
    /// label, anchor, and `--at` targets.
    fn alias_naming(&self, name: NodeId, keyed: bool) -> Naming {
        let mut naming = self.named(name);
        if !keyed {
            naming.key.clear();
        }
        naming
    }

    /// `newDocumentSymbol` for a module declaration: a dotted namespace is one
    /// site named by its whole chain (`getModuleName`), keyed by each segment.
    fn module_naming(&self, names: &[NodeId]) -> Naming {
        let mut naming = self.named(names[0]);
        for &name in &names[1..] {
            let segment = self.named(name);
            naming.label.push('.');
            naming.label.push_str(&segment.label);
            naming.key.extend(segment.key);
            naming.targets.extend(segment.targets);
            naming.name_end = segment.name_end;
        }
        if naming.key.len() != names.len() {
            naming.key.clear();
        }
        naming
    }

    /// TypeScript's `GetAssignedName` for a function or class expression on
    /// the right of a binary expression. Its other parents (variables,
    /// properties, binding elements) never reach `visit`: their initializers
    /// are traversed as contents.
    fn assigned_name(&self, v: &Visit) -> Option<NodeId> {
        let ast = self.ast;
        let parent = ast.node(v.parent?);
        if parent.syntax_kind != SyntaxKind::BinaryExpression
            || syntax::id(parent, "right") != Some(v.node)
        {
            return None;
        }
        let left = syntax::id(parent, "left")?;
        let node = ast.node(left);
        match node.syntax_kind {
            SyntaxKind::Identifier => Some(left),
            SyntaxKind::PropertyAccessExpression => syntax::id(node, "name"),
            SyntaxKind::ElementAccessExpression => {
                let argument =
                    syntax::skip_parentheses(ast, syntax::id(node, "argumentExpression")?);
                matches!(
                    ast.node(argument).syntax_kind,
                    SyntaxKind::StringLiteral
                        | SyntaxKind::NumericLiteral
                        | SyntaxKind::NoSubstitutionTemplateLiteral
                )
                .then_some(argument)
            }
            _ => None,
        }
    }

    /// TypeScript's `getUnnamedNodeLabel`, deciding which label an unnamed
    /// site has; `None` for a site it drops.
    fn unnamed_label(&self, v: &Visit) -> Option<Unnamed> {
        use SyntaxKind::*;
        let ast = self.ast;
        if let Some(outer) = v.outer.map(|outer| ast.node(outer))
            && outer.syntax_kind == ExportAssignment
        {
            return Some(Unnamed::Fixed(if syntax::flag(outer, "isExportEquals") {
                "export="
            } else {
                "default"
            }));
        }
        let node = ast.node(v.node);
        let default = has_modifier(ast, node, DefaultKeyword);
        Some(match node.syntax_kind {
            FunctionDeclaration | FunctionExpression | ArrowFunction => {
                if default {
                    Unnamed::Fixed("default")
                } else {
                    match v.parent {
                        Some(call) if ast.node(call).syntax_kind == CallExpression => {
                            Unnamed::Callback(call)
                        }
                        _ => Unnamed::Fixed("<function>"),
                    }
                }
            }
            ClassDeclaration | ClassExpression if default => Unnamed::Fixed("default"),
            ClassDeclaration | ClassExpression => Unnamed::Fixed("<class>"),
            Constructor => Unnamed::Fixed("constructor"),
            CallSignature => Unnamed::Fixed("()"),
            ConstructSignature => Unnamed::Fixed("new()"),
            IndexSignature => Unnamed::Fixed("[]"),
            _ => return None,
        })
    }

    /// The signature of a site whose header is its source up to `header`
    /// (a body or member list), or else its whole source without a final
    /// separator. Product rule: anonymous function and class expressions
    /// have none, since their label identifies them better than `() =>`.
    fn signature_of(&self, v: &Visit, naming: &Naming, header: Option<usize>) -> Option<String> {
        if self.detail == Detail::Selection {
            return None;
        }
        let node = self.ast.node(v.node);
        if matches!(
            node.syntax_kind,
            SyntaxKind::FunctionExpression
                | SyntaxKind::ArrowFunction
                | SyntaxKind::ClassExpression
        ) && syntax::id(node, "name").is_none()
        {
            return None;
        }
        let range = self.own_range(v.node);
        let end = header.unwrap_or_else(|| self.without_separator(range).end);
        let spec = Spec {
            parts: smallvec![Span {
                start: range.start,
                end,
            }],
            name_end: naming.name_end.min(end),
        };
        Some(self.render(v.node, spec))
    }

    fn render(&self, root: NodeId, spec: Spec) -> String {
        let bounds = Span {
            start: spec.parts[0].start,
            end: spec.parts[spec.parts.len() - 1].end,
        };
        let mut elisions = Elisions {
            projection: self,
            bounds,
            stack: vec![Elision::Node(root)],
        };
        signature::render(self.ast.source(), &self.literals, &spec, |at| {
            elisions.starting_at(at)
        })
    }

    /// What a specifier site designates: its import or export declaration.
    fn statement_of(&self, v: &Visit) -> NodeId {
        match v.statement {
            Statement::Declaration(node) => node,
            _ => v.node,
        }
    }

    /// A node from its first token, including decorators and modifiers,
    /// through its last token.
    fn own_range(&self, node: NodeId) -> Span {
        self.ast.token_span(node)
    }

    fn declarations_start(&self, list: NodeId) -> usize {
        syntax::ids(self.ast.node(list), "declarations")
            .first()
            .map_or_else(
                || self.own_range(list).end,
                |&first| self.own_range(first).start,
            )
    }

    /// A member's or statement's range without its final `;` or `,`.
    fn without_separator(&self, mut range: Span) -> Span {
        if self.ast.source()[range.start..range.end].ends_with([';', ',']) {
            range.end -= 1;
        }
        range
    }

    fn first_token(&self, node: NodeId) -> Span {
        let start = self.own_range(node).start;
        let token = Lexer {
            src: self.ast.source(),
            pos: start,
        }
        .next();
        Span {
            start,
            end: token.end.max(start),
        }
    }

    /// Where a class-like's member list opens, after its modifiers, name,
    /// type parameters, and heritage clauses. The tokens between those parts
    /// and `{` are only keywords and a closing `>`.
    fn open_brace(&self, node: NodeId) -> Option<usize> {
        let ast = self.ast;
        let declaration = ast.node(node);
        let mut after = self.own_range(node).start;
        for field in ["modifiers", "name", "typeParameters", "heritageClauses"] {
            let last = match declaration.field(field) {
                Some(crate::Value::Node(id)) => Some(*id),
                Some(crate::Value::Nodes(ids)) => ids.last().copied(),
                _ => None,
            };
            if let Some(last) = last {
                after = after.max(ast.node(last).span.end);
            }
        }
        let mut lexer = Lexer {
            src: ast.source(),
            pos: after,
        };
        (0..8)
            .map(|_| lexer.next())
            .take_while(|token| token.end > token.start)
            .find(|token| token.raw == "{")
            .map(|token| token.start)
            .filter(|&brace| brace < declaration.span.end)
    }
}

/// The label of an unnamed site, rendered only by a complete projection.
enum Unnamed {
    Fixed(&'static str),
    /// The name of a function or class expression's assignment target.
    Assigned(NodeId),
    /// A function passed to, or called by, this call: `names::callback`.
    Callback(NodeId),
}

impl Unnamed {
    fn render(self, ast: &Ast<'_>) -> String {
        match self {
            Self::Fixed(label) => label.into(),
            Self::Assigned(name) => names::declared(ast, name).label,
            Self::Callback(call) => {
                names::callback(ast, call).unwrap_or_else(|| "<function>".into())
            }
        }
    }
}

/// The bodies a signature elides, block bodies of functions and member
/// lists of classes nested within `bounds`, found in source order as the
/// rendering reaches them. Only a node that contains a rendered position is
/// expanded, one child at a time, so a signature that is long enough after
/// a few tokens never visits the rest of its declaration, however large or
/// often nested in other signatures' declarations.
struct Elisions<'p, 'a, 'src> {
    projection: &'p Projection<'a, 'src>,
    bounds: Span,
    /// The work ahead, last first, in source order.
    stack: Vec<Elision>,
}

enum Elision {
    /// A node not yet expanded.
    Node(NodeId),
    /// A node's children from `next` on, except its elided body, `skip`.
    Children {
        node: NodeId,
        next: Child,
        skip: Option<&'static str>,
    },
    /// An elided body, which follows its node's other children.
    Body(Span),
}

/// A position among a node's children in source order: its modifiers,
/// which the parser stores last, then its other logical fields in order.
/// Field 0 is `modifiers`; field `n` is the node's `n - 1`th logical field.
#[derive(Clone, Copy)]
struct Child {
    field: usize,
    item: usize,
}

impl Elisions<'_, '_, '_> {
    /// The elided body that starts at `at`, asked at increasing positions.
    /// Work wholly before `at` is discarded, including bodies that began in
    /// text the rendering skipped, and work after it waits.
    fn starting_at(&mut self, at: usize) -> Option<Span> {
        let ast = self.projection.ast;
        loop {
            match self.stack.last_mut()? {
                Elision::Body(span) => {
                    let span = *span;
                    if span.start > at {
                        return None;
                    }
                    self.stack.pop();
                    if span.start == at {
                        return Some(span);
                    }
                }
                Elision::Children { node, next, skip } => {
                    match next_child(ast, *node, next, *skip) {
                        Some(child) => self.stack.push(Elision::Node(child)),
                        None => {
                            self.stack.pop();
                        }
                    }
                }
                Elision::Node(id) => {
                    let id = *id;
                    let node = ast.node(id);
                    if node.span.start > at {
                        return None;
                    }
                    self.stack.pop();
                    if node.span.end <= at.max(self.bounds.start)
                        || node.span.start >= self.bounds.end
                    {
                        continue;
                    }
                    let body = self.body(id).filter(|(_, span)| {
                        span.start >= self.bounds.start && span.end <= self.bounds.end
                    });
                    if let Some((_, span)) = body {
                        self.stack.push(Elision::Body(span));
                    }
                    self.stack.push(Elision::Children {
                        node: id,
                        next: Child { field: 0, item: 0 },
                        skip: body.map(|(field, _)| field),
                    });
                }
            }
        }
    }

    /// The body field a signature elides in a function or class, and its
    /// span: a function's block body, or a class's member list.
    fn body(&self, id: NodeId) -> Option<(&'static str, Span)> {
        let ast = self.projection.ast;
        let node = ast.node(id);
        if FUNCTIONS.contains(&node.syntax_kind) {
            syntax::id(node, "body")
                .filter(|&body| ast.node(body).syntax_kind == SyntaxKind::Block)
                .map(|body| ("body", ast.token_span(body)))
        } else if matches!(
            node.syntax_kind,
            SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
        ) {
            self.projection.open_brace(id).map(|brace| {
                (
                    "members",
                    Span {
                        start: brace,
                        end: node.span.end,
                    },
                )
            })
        } else {
            None
        }
    }
}

/// The child of `id` at `next`, in source order, advancing `next`: its
/// modifiers, which the parser stores last, then its other logical fields
/// (`Ast::logical_fields`) in order, never the `skip` field.
fn next_child(ast: &Ast<'_>, id: NodeId, next: &mut Child, skip: Option<&str>) -> Option<NodeId> {
    let node = ast.node(id);
    loop {
        let value = match next.field {
            0 => node.field("modifiers"),
            field => {
                let (name, value) = ast.logical_fields(id).nth(field - 1)?;
                (name != "modifiers" && Some(name) != skip).then_some(value)
            }
        };
        let children = match value {
            Some(crate::Value::Node(id)) => std::slice::from_ref(id),
            Some(crate::Value::Nodes(ids)) => ids.as_slice(),
            _ => &[],
        };
        if let Some(&child) = children.get(next.item) {
            next.item += 1;
            return Some(child);
        }
        next.field += 1;
        next.item = 0;
    }
}
