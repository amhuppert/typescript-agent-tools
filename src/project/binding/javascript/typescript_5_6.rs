//! TypeScript 5.6.3's own decisions of JavaScript-only meaning, which the
//! TypeScript 5.6 dialect refuses besides 7.0.2's (`super::Rules`): what its
//! binder declares for an assignment, an `Object.defineProperty` call, or a
//! `this` assignment (`bindWorker`, `bindSpecialPropertyAssignment`,
//! `bindPropertyAssignment`, `bindPotentiallyMissingNamespaces`,
//! `bindPotentiallyNewExpandoMemberToNamespace`, `isExpandoSymbol`,
//! `bindThisPropertyAssignment`), and which JSDoc tags its binder and
//! checker read where they stand (`getJSDocCommentsAndTags`,
//! `filterOwnedJSDocTags`, `getNextJSDocCommentLocation`).
//!
//! 5.6.3 binds as it walks the tree: a statement list's function
//! declarations first, then each node before its children. A lookup sees
//! only what it has bound, and a symbol gains members, and the class and
//! namespace flags, as it goes; this model keeps the same order and state.

use super::super::collect::{Alias, AliasBind, Decl, DeclKind, Scope};
use super::super::flow::syntax::Facts as Containers;
use super::super::member::{MemberKey, literal_key};
use super::{Construct, Site, containers_of};
use crate::parser::jsdoc::{TagFacts, TagSyntax, Tags};
use crate::syntax::{self, id, ids};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Span, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

type SymbolId = usize;

/// One symbol as 5.6.3's binder holds it while it binds the file.
#[derive(Default)]
struct Symbol {
    function: bool,
    class: bool,
    namespace: bool,
    alias: bool,
    value: Option<NodeId>,
    exports: HashMap<MemberKey, SymbolId>,
    members: HashMap<MemberKey, SymbolId>,
}

/// One declaration of a name in a container's locals.
struct Local {
    node: NodeId,
    kind: LocalKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LocalKind {
    Function,
    Class,
    Value,
    Alias,
}

/// `getAssignmentDeclarationKind` as 5.6.3 decides it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    None,
    ExportsProperty,
    ModuleExports,
    PrototypeProperty,
    ThisProperty,
    Property,
    Prototype,
    DefinePropertyValue,
    DefinePropertyExports,
    DefinePrototypeProperty,
}

/// 5.6.3's binder (`createBinder`) as far as it decides JavaScript-only
/// meaning: the symbols, locals, and order its walk would leave.
pub(super) struct Model<'a, 's> {
    ast: &'a Ast<'s>,
    containers: &'a Containers,
    parents: Vec<Option<NodeId>>,
    /// Each node's place in 5.6.3's binding order.
    order: Vec<usize>,
    locals: HashMap<(Site, String), Vec<Local>>,
    /// The container and name each function declaration is a local of.
    function_locals: HashMap<NodeId, (Site, String)>,
    symbols: Vec<Symbol>,
    local_symbols: HashMap<(Site, String), SymbolId>,
    /// `jsGlobalAugmentations`.
    globals: HashMap<MemberKey, SymbolId>,
    /// The file's own symbol, which CommonJS exports join.
    file: SymbolId,
    module_syntax: bool,
    /// The `module.exports` assignment binding admits
    /// (`super::commonjs::module_exports`).
    commonjs: Option<NodeId>,
    tags: &'a HashMap<NodeId, Arc<Tags>>,
    /// The assignments and access expressions whose declarations the binder
    /// made, which read their JSDoc as declarations.
    declared_nodes: HashSet<NodeId>,
    refusals: Vec<(Span, Construct)>,
}

impl<'a, 's> Model<'a, 's> {
    /// The binder as `bindSourceFile` starts `ast`: each container's
    /// declarations, which a lookup sees once `bindWorker` has passed them.
    pub(super) fn new(
        ast: &'a Ast<'s>,
        containers: &'a Containers,
        scopes: &[Scope],
        decls: &[Decl],
        aliases: &[Alias],
        exports: super::commonjs::Exports,
        tags: &'a HashMap<NodeId, Arc<Tags>>,
    ) -> Self {
        let count = ast.nodes().len();
        let parents = (0..count)
            .map(|index| containers.parent(NodeId(index)))
            .collect();
        let nodes: HashMap<Site, NodeId> = ast
            .walk()
            .map(|id| ((ast.node(id).syntax_kind, ast.node(id).span), id))
            .collect();
        let mut locals: HashMap<(Site, String), Vec<Local>> = HashMap::new();
        let mut function_locals = HashMap::new();
        for decl in decls {
            let kind = match decl.kind {
                DeclKind::Function if decl.native_kind == SyntaxKind::FunctionDeclaration => {
                    LocalKind::Function
                }
                DeclKind::Class if decl.native_kind == SyntaxKind::ClassDeclaration => {
                    LocalKind::Class
                }
                DeclKind::Var | DeclKind::Let | DeclKind::Parameter | DeclKind::Enum { .. } => {
                    LocalKind::Value
                }
                _ => continue,
            };
            let (Some(scope), Some(span)) = (decl.lexical, decl.native_span) else {
                continue;
            };
            let (Some(owner), Some(&node)) = (
                scopes[scope.index()].native_owner,
                nodes.get(&(decl.native_kind, span)),
            ) else {
                continue;
            };
            if kind == LocalKind::Function {
                function_locals.insert(node, (owner, decl.name.to_string()));
            }
            locals
                .entry((owner, decl.name.to_string()))
                .or_default()
                .push(Local { node, kind });
        }
        for alias in aliases {
            let (AliasBind::Lexical { scope, name } | AliasBind::Both { scope, name, .. }) =
                &alias.bind
            else {
                continue;
            };
            let (Some(owner), Some(&node)) = (
                scopes[scope.index()].native_owner,
                nodes.get(&(alias.native_kind, alias.native_span)),
            ) else {
                continue;
            };
            locals
                .entry((owner, name.clone()))
                .or_default()
                .push(Local {
                    node,
                    kind: LocalKind::Alias,
                });
        }
        Self {
            ast,
            containers,
            parents,
            order: binding_order(ast),
            locals,
            function_locals,
            symbols: vec![Symbol::default()],
            local_symbols: HashMap::new(),
            globals: HashMap::new(),
            file: 0,
            module_syntax: exports.module_syntax,
            commonjs: exports.admitted,
            tags,
            declared_nodes: HashSet::new(),
            refusals: Vec::new(),
        }
    }

    /// Bind the file's assignments and calls in 5.6.3's order, then classify
    /// its JSDoc: every construct 5.6.3 gives JavaScript-only meaning.
    pub(super) fn refusals(mut self) -> Vec<(Span, Construct)> {
        let ast = self.ast;
        let mut nodes: Vec<NodeId> = ast
            .walk()
            .filter(|&id| {
                matches!(
                    ast.node(id).syntax_kind,
                    SyntaxKind::BinaryExpression
                        | SyntaxKind::CallExpression
                        | SyntaxKind::PropertyAccessExpression
                        | SyntaxKind::ElementAccessExpression
                )
            })
            .collect();
        nodes.sort_by_key(|id| self.order[id.index()]);
        for node in nodes {
            let construct = match ast.node(node).syntax_kind {
                SyntaxKind::BinaryExpression | SyntaxKind::CallExpression => self.bind(node),
                _ => self.special_property_declaration(node),
            };
            if let Some(construct) = construct {
                self.declared_nodes.insert(node);
                self.refusals.push((ast.token_span(node), construct));
            }
        }
        self.jsdoc();
        self.refusals
    }

    fn kind(&self, id: NodeId) -> SyntaxKind {
        self.ast.node(id).syntax_kind
    }

    fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.parents[id.index()]
    }

    fn field(&self, node: NodeId, key: &str) -> Option<NodeId> {
        id(self.ast.node(node), key)
    }

    fn is_identifier_named(&self, node: NodeId, name: &str) -> bool {
        self.kind(node) == SyntaxKind::Identifier && self.ast.identifier_name(node) == Some(name)
    }

    /// `getElementOrPropertyAccessName`, as a member key.
    fn access_name(&self, node: NodeId) -> Option<MemberKey> {
        syntax::element_or_property_access_name(self.ast, node)
            .map(|name| literal_key(self.ast, self.ast.node(name)))
    }

    fn named(&self, node: NodeId, name: &str) -> bool {
        matches!(self.access_name(node), Some(MemberKey::Text(text)) if &*text == name)
    }

    fn is_access(&self, node: NodeId) -> bool {
        matches!(
            self.kind(node),
            SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
        )
    }

    /// `isBindableStaticAccessExpression`.
    fn bindable_access(&self, node: NodeId, exclude_this: bool) -> bool {
        self.kind(node) != SyntaxKind::Identifier
            && syntax::is_bindable_static_name_expression(self.ast, node, exclude_this)
    }

    /// `isPrototypeAccess`.
    fn prototype_access(&self, node: NodeId) -> bool {
        self.bindable_access(node, false) && self.named(node, "prototype")
    }

    /// `getRightMostAssignedExpression`.
    fn rightmost(&self, mut node: NodeId) -> NodeId {
        while syntax::is_assignment_expression(self.ast, node, true) {
            match self.field(node, "right") {
                Some(right) => node = right,
                None => break,
            }
        }
        node
    }

    fn operator(&self, binary: NodeId) -> Option<SyntaxKind> {
        self.field(binary, "operatorToken")
            .map(|token| self.kind(token))
    }

    /// `getAssignmentDeclarationKindWorker` in a JavaScript file.
    fn assignment_kind(&self, node: NodeId) -> Kind {
        let ast = self.ast;
        if self.kind(node) == SyntaxKind::CallExpression {
            if !syntax::is_bindable_object_define_property_call(ast, node) {
                return Kind::None;
            }
            let entity = ids(ast.node(node), "arguments")[0];
            if self.is_identifier_named(entity, "exports")
                || syntax::is_module_exports_access_expression(self.ast, entity)
            {
                return Kind::DefinePropertyExports;
            }
            if self.bindable_access(entity, false) && self.named(entity, "prototype") {
                return Kind::DefinePrototypeProperty;
            }
            return Kind::DefinePropertyValue;
        }
        let Some(left) = self.field(node, "left") else {
            return Kind::None;
        };
        if self.operator(node) != Some(SyntaxKind::EqualsToken)
            || !self.is_access(left)
            || is_void_zero(ast, self.rightmost(node))
        {
            return Kind::None;
        }
        let Some(receiver) = self.field(left, "expression") else {
            return Kind::None;
        };
        // `getInitializerOfBinaryExpression`.
        let mut initializer = node;
        while let Some(right) = self.field(initializer, "right")
            && self.kind(right) == SyntaxKind::BinaryExpression
        {
            initializer = right;
        }
        let initializer = self.field(initializer, "right");
        if syntax::is_bindable_static_name_expression(ast, receiver, true)
            && self.named(left, "prototype")
            && initializer
                .is_some_and(|init| self.kind(init) == SyntaxKind::ObjectLiteralExpression)
        {
            return Kind::Prototype;
        }
        self.property_access_kind(left)
    }

    /// `getAssignmentDeclarationPropertyAccessKind`.
    fn property_access_kind(&self, left: NodeId) -> Kind {
        let ast = self.ast;
        let Some(receiver) = self.field(left, "expression") else {
            return Kind::None;
        };
        if self.kind(receiver) == SyntaxKind::ThisKeyword {
            return Kind::ThisProperty;
        }
        if syntax::is_module_exports_access_expression(self.ast, left) {
            return Kind::ModuleExports;
        }
        if !syntax::is_bindable_static_name_expression(ast, receiver, true) {
            return Kind::None;
        }
        if self.prototype_access(receiver) {
            return Kind::PrototypeProperty;
        }
        let mut next_to_last = left;
        while let Some(expression) = self.field(next_to_last, "expression")
            && self.kind(expression) != SyntaxKind::Identifier
        {
            next_to_last = expression;
        }
        let root = self.field(next_to_last, "expression");
        let commonjs = root.is_some_and(|root| {
            self.is_identifier_named(root, "exports")
                || self.is_identifier_named(root, "module") && self.named(next_to_last, "exports")
        });
        if commonjs && self.bindable_access(left, false) {
            return Kind::ExportsProperty;
        }
        if syntax::is_bindable_static_name_expression(ast, left, true)
            || self.kind(left) == SyntaxKind::ElementAccessExpression
                && syntax::is_dynamic_name(ast, left)
        {
            return Kind::Property;
        }
        Kind::None
    }

    /// `bindWorker` for an assignment or call: what it declares.
    fn bind(&mut self, node: NodeId) -> Option<Construct> {
        let left = self.field(node, "left");
        match self.assignment_kind(node) {
            Kind::None => None,
            // `setCommonJsModuleIndicator` succeeds only without module syntax.
            Kind::ExportsProperty => (!self.module_syntax).then_some(Construct::ExportsAssignment),
            // `bindModuleExportsAssignment` binds the admitted assignment as
            // 7.0.2 does, but not where its name is an alias of `exports`.
            Kind::ModuleExports if Some(node) == self.commonjs => {
                let right = self.field(node, "right")?;
                self.exports_or_alias(right, node)
                    .then_some(Construct::ModuleExports)
            }
            Kind::ModuleExports => (!self.module_syntax).then_some(Construct::ModuleExports),
            Kind::DefinePropertyExports => {
                (!self.module_syntax).then_some(Construct::DefinePropertyExport)
            }
            Kind::PrototypeProperty => {
                let left = left?;
                let constructor = self.field(self.field(left, "expression")?, "expression")?;
                self.property_assignment(node, constructor, left, true, true)
            }
            Kind::Prototype => {
                let left = left?;
                let constructor = self.field(left, "expression")?;
                self.property_assignment(node, constructor, left, false, true)
            }
            Kind::ThisProperty => self.this_property(node, left?),
            Kind::Property => {
                let left = left?;
                let receiver = self.field(left, "expression")?;
                if self.kind(receiver) == SyntaxKind::Identifier {
                    let (_, block) = containers_of(self.ast, self.containers, node);
                    let this_initialized = self
                        .lookup_name(block, receiver, node)
                        .and_then(|symbol| self.symbols[symbol].value)
                        .is_some_and(|value| {
                            self.kind(value) == SyntaxKind::VariableDeclaration
                                && self
                                    .field(value, "initializer")
                                    .is_some_and(|init| self.kind(init) == SyntaxKind::ThisKeyword)
                        });
                    if this_initialized {
                        return self.this_property(node, left);
                    }
                }
                self.special_property_assignment(node, left)
            }
            Kind::DefinePropertyValue => self.define_property(node),
            Kind::DefinePrototypeProperty => self.define_prototype_property(node),
        }
    }

    /// `bindSpecialPropertyDeclaration`: an access expression statement that
    /// a `@type` tag makes a declaration (`isSpecialPropertyDeclaration`).
    fn special_property_declaration(&mut self, access: NodeId) -> Option<Construct> {
        let statement = self.parent(access)?;
        if self.kind(statement) != SyntaxKind::ExpressionStatement {
            return None;
        }
        if self.kind(access) == SyntaxKind::ElementAccessExpression
            && syntax::element_or_property_access_name(self.ast, access).is_none()
        {
            return None;
        }
        let typed = self
            .owned_tags(statement)
            .iter()
            .any(|(tag, _)| tag.syntax == TagSyntax::Type);
        if !typed {
            return None;
        }
        let receiver = self.field(access, "expression")?;
        if self.kind(receiver) == SyntaxKind::ThisKeyword {
            return self.this_property(access, access);
        }
        if self.bindable_access(access, false) && self.parent(statement) == Some(self.ast.root_id())
        {
            if self.prototype_access(receiver) {
                let constructor = self.field(receiver, "expression")?;
                return self.property_assignment(access, constructor, access, true, true);
            }
            return self.property_assignment(access, receiver, access, false, false);
        }
        None
    }

    /// `bindThisPropertyAssignment` for `node`, which assigns `left`.
    fn this_property(&mut self, node: NodeId, left: NodeId) -> Option<Construct> {
        let ast = self.ast;
        if self.kind(left) == SyntaxKind::PropertyAccessExpression
            && self
                .field(left, "name")
                .is_some_and(|name| self.kind(name) == SyntaxKind::PrivateIdentifier)
        {
            return None;
        }
        let container = syntax::this_container(ast, &self.parents, node, false, false)?;
        match self.kind(container) {
            SyntaxKind::FunctionDeclaration | SyntaxKind::FunctionExpression => {
                // For `C.prototype.m = function () { this.x = 0; }`, `C`.
                let assigned = self.parent(container).filter(|&parent| {
                    self.kind(parent) == SyntaxKind::BinaryExpression
                        && self.operator(parent) == Some(SyntaxKind::EqualsToken)
                });
                let prototype_member = assigned
                    .and_then(|parent| self.field(parent, "left"))
                    .filter(|&left| {
                        self.bindable_access(left, false)
                            && self
                                .field(left, "expression")
                                .is_some_and(|receiver| self.prototype_access(receiver))
                    });
                let constructor = match prototype_member {
                    Some(left) => {
                        let receiver = self.field(left, "expression")?;
                        let constructor = self.field(receiver, "expression")?;
                        let (outer, _) = containers_of(self.ast, self.containers, container);
                        self.lookup_access(constructor, outer, node)
                    }
                    None => self.function_symbol(container),
                };
                let constructor = constructor?;
                self.symbols[constructor].value?;
                self.symbols[constructor].class = true;
                Some(Construct::ConstructorThis)
            }
            SyntaxKind::Constructor
            | SyntaxKind::PropertyDeclaration
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
            | SyntaxKind::ClassStaticBlockDeclaration => Some(Construct::ThisProperty),
            SyntaxKind::SourceFile if !self.dynamic_name(left) => Some(Construct::TopLevelThis),
            _ => None,
        }
    }

    /// The symbol of a function a `this` assignment is in: a function
    /// declaration's local, or an expression's own, which no name looks up.
    fn function_symbol(&mut self, function: NodeId) -> Option<SymbolId> {
        if let Some((owner, name)) = self.function_locals.get(&function).cloned() {
            return Some(self.local_symbol(owner, name, function, LocalKind::Function));
        }
        let symbol = self.new_symbol(Symbol {
            function: true,
            value: Some(function),
            ..Symbol::default()
        });
        Some(symbol)
    }

    /// `HasDynamicName` for an assignment's target.
    fn dynamic_name(&self, left: NodeId) -> bool {
        self.kind(left) == SyntaxKind::ElementAccessExpression
            && syntax::element_or_property_access_name(self.ast, left).is_none()
            && syntax::is_dynamic_name(self.ast, left)
    }

    /// `bindSpecialPropertyAssignment`.
    fn special_property_assignment(&mut self, node: NodeId, left: NodeId) -> Option<Construct> {
        let root_id = self.ast.root_id();
        let receiver = self.field(left, "expression")?;
        let (container, block) = containers_of(self.ast, self.containers, node);
        let parent_symbol = self
            .lookup_access(receiver, block, node)
            .or_else(|| self.lookup_access(receiver, container, node));
        // `getLeftmostAccessExpression`: an import's member declares nothing.
        let mut leftmost = left;
        while self.is_access(leftmost) {
            match self.field(leftmost, "expression") {
                Some(expression) => leftmost = expression,
                None => break,
            }
        }
        if self.kind(leftmost) == SyntaxKind::Identifier
            && self
                .lookup_name(container, leftmost, node)
                .is_some_and(|symbol| self.symbols[symbol].alias)
        {
            return None;
        }
        if self.kind(receiver) == SyntaxKind::Identifier
            && container == root_id
            && self.exports_or_alias(receiver, node)
        {
            return (!self.module_syntax).then_some(Construct::ExportsAssignment);
        }
        if self.dynamic_name(left) {
            let top = self.top_level_namespace_assignment(left);
            let (symbol, created) =
                self.missing_namespaces(parent_symbol, receiver, top, false, false, node);
            if created {
                return Some(Construct::NamespaceMember);
            }
            return symbol
                .filter(|&symbol| self.is_expando(symbol))
                .map(|_| Construct::Expando);
        }
        self.property_assignment(node, receiver, left, false, false)
    }

    /// `bindPropertyAssignment`: `name` names the symbol `access` extends.
    fn property_assignment(
        &mut self,
        node: NodeId,
        name: NodeId,
        access: NodeId,
        prototype: bool,
        class: bool,
    ) -> Option<Construct> {
        let (container, block) = containers_of(self.ast, self.containers, node);
        let symbol = self
            .lookup_access(name, block, node)
            .or_else(|| self.lookup_access(name, container, node));
        let top = self.top_level_namespace_assignment(access);
        let entity = self.field(access, "expression")?;
        let (symbol, created) =
            self.missing_namespaces(symbol, entity, top, prototype, class, node);
        let member = self.new_member(access, symbol, prototype);
        if prototype || class {
            (created || member).then_some(Construct::PrototypeMember)
        } else if created {
            Some(Construct::NamespaceMember)
        } else {
            member.then_some(Construct::Expando)
        }
    }

    /// `bindObjectDefinePropertyAssignment`.
    fn define_property(&mut self, call: NodeId) -> Option<Construct> {
        let target = *ids(self.ast.node(call), "arguments").first()?;
        let (container, _) = containers_of(self.ast, self.containers, call);
        let symbol = self.lookup_access(target, container, call);
        let top =
            self.parent(call).and_then(|parent| self.parent(parent)) == Some(self.ast.root_id());
        let (symbol, created) = self.missing_namespaces(symbol, target, top, false, false, call);
        let member = self.new_member(call, symbol, false);
        if created {
            Some(Construct::DefinePropertyNamespaceMember)
        } else {
            member.then_some(Construct::DefinePropertyExpando)
        }
    }

    /// `bindObjectDefinePrototypeProperty`.
    fn define_prototype_property(&mut self, call: NodeId) -> Option<Construct> {
        let prototype = *ids(self.ast.node(call), "arguments").first()?;
        let constructor = self.field(prototype, "expression")?;
        let (container, _) = containers_of(self.ast, self.containers, call);
        let symbol = self.lookup_access(constructor, container, call);
        let mut declared = false;
        if let Some(symbol) = symbol
            && self.symbols[symbol].value.is_some()
        {
            self.symbols[symbol].class = true;
            declared = true;
        }
        let member = self.new_member(call, symbol, true);
        (declared || member).then_some(Construct::PrototypeMember)
    }

    /// `isTopLevelNamespaceAssignment`.
    fn top_level_namespace_assignment(&self, access: NodeId) -> bool {
        let root = Some(self.ast.root_id());
        let Some(parent) = self.parent(access) else {
            return false;
        };
        if self.kind(parent) == SyntaxKind::BinaryExpression {
            // `getParentOfBinaryExpression`.
            let mut outermost = parent;
            while let Some(above) = self.parent(outermost)
                && self.kind(above) == SyntaxKind::BinaryExpression
            {
                outermost = above;
            }
            self.parent(outermost).and_then(|above| self.parent(above)) == root
        } else {
            self.parent(parent) == root
        }
    }

    /// `bindPotentiallyMissingNamespaces`: the symbol `entity` names, made a
    /// namespace at the top level, and whether anything was declared.
    fn missing_namespaces(
        &mut self,
        symbol: Option<SymbolId>,
        entity: NodeId,
        top: bool,
        prototype: bool,
        class: bool,
        at: NodeId,
    ) -> (Option<SymbolId>, bool) {
        if symbol.is_some_and(|symbol| self.symbols[symbol].alias) {
            return (symbol, false);
        }
        let mut created = false;
        let mut symbol = symbol;
        if top && !prototype {
            symbol = self.declare_namespaces(entity, symbol, at, &mut created);
        }
        if class
            && let Some(symbol) = symbol
            && self.symbols[symbol].value.is_some()
        {
            self.symbols[symbol].class = true;
            created = true;
        }
        (symbol, created)
    }

    /// `forEachIdentifierInEntityName` with the namespace declaring action.
    fn declare_namespaces(
        &mut self,
        entity: NodeId,
        parent: Option<SymbolId>,
        at: NodeId,
        created: &mut bool,
    ) -> Option<SymbolId> {
        if self.exports_or_alias(entity, at) {
            return Some(self.file);
        }
        if self.kind(entity) == SyntaxKind::Identifier {
            let (container, _) = containers_of(self.ast, self.containers, entity);
            let found = self.lookup_name(container, entity, at);
            let key = literal_key(self.ast, self.ast.node(entity));
            return Some(self.namespace(entity, key, found, parent, created));
        }
        let receiver = self.field(entity, "expression")?;
        let outer = self.declare_namespaces(receiver, parent, at, created);
        let key = self.access_name(entity)?;
        let found = outer.and_then(|outer| self.symbols[outer].exports.get(&key).copied());
        let name = syntax::element_or_property_access_name(self.ast, entity)?;
        Some(self.namespace(name, key, found, outer, created))
    }

    fn namespace(
        &mut self,
        name: NodeId,
        key: MemberKey,
        found: Option<SymbolId>,
        parent: Option<SymbolId>,
        created: &mut bool,
    ) -> SymbolId {
        *created = true;
        if let Some(found) = found {
            self.symbols[found].namespace = true;
            return found;
        }
        let symbol = self.new_symbol(Symbol {
            namespace: true,
            value: Some(name),
            ..Symbol::default()
        });
        match parent {
            Some(parent) => self.symbols[parent].exports.insert(key, symbol),
            None => self.globals.insert(key, symbol),
        };
        symbol
    }

    /// `bindPotentiallyNewExpandoMemberToNamespace`: whether `declaration`
    /// declares a member of `symbol`.
    fn new_member(
        &mut self,
        declaration: NodeId,
        symbol: Option<SymbolId>,
        prototype: bool,
    ) -> bool {
        let Some(symbol) = symbol else {
            return false;
        };
        if !self.is_expando(symbol) {
            return false;
        }
        let key = if self.kind(declaration) == SyntaxKind::CallExpression {
            ids(self.ast.node(declaration), "arguments")
                .get(1)
                .map(|&name| literal_key(self.ast, self.ast.node(name)))
        } else {
            self.access_name(declaration)
        };
        // A dynamic name binds a late-bound member.
        let Some(key) = key else {
            return true;
        };
        let table = if prototype {
            &self.symbols[symbol].members
        } else {
            &self.symbols[symbol].exports
        };
        if !table.contains_key(&key) {
            let member = self.new_symbol(Symbol {
                value: Some(declaration),
                ..Symbol::default()
            });
            let table = if prototype {
                &mut self.symbols[symbol].members
            } else {
                &mut self.symbols[symbol].exports
            };
            table.insert(key, member);
        }
        true
    }

    /// `isExpandoSymbol`.
    fn is_expando(&self, symbol: SymbolId) -> bool {
        let symbol = &self.symbols[symbol];
        if symbol.function || symbol.class || symbol.namespace {
            return true;
        }
        let Some(value) = symbol.value else {
            return false;
        };
        let (initializer, name) = match self.kind(value) {
            SyntaxKind::CallExpression => return self.call_expando_initializer(value),
            SyntaxKind::VariableDeclaration => {
                (self.field(value, "initializer"), self.field(value, "name"))
            }
            SyntaxKind::BinaryExpression => (self.field(value, "right"), self.field(value, "left")),
            SyntaxKind::PropertyAccessExpression => match self.parent(value) {
                Some(parent) if self.kind(parent) == SyntaxKind::BinaryExpression => {
                    (self.field(parent, "right"), Some(value))
                }
                _ => return false,
            },
            _ => return false,
        };
        let Some(initializer) = initializer else {
            return false;
        };
        let initializer = self.rightmost(initializer);
        let prototype = name.is_some_and(|name| self.prototype_access(name));
        let initializer = if self.kind(initializer) == SyntaxKind::BinaryExpression
            && matches!(
                self.operator(initializer),
                Some(SyntaxKind::BarBarToken | SyntaxKind::QuestionQuestionToken)
            ) {
            match self.field(initializer, "right") {
                Some(right) => right,
                None => return false,
            }
        } else {
            initializer
        };
        self.expando_initializer(initializer, prototype)
    }

    /// `getExpandoInitializer`.
    fn expando_initializer(&self, initializer: NodeId, prototype: bool) -> bool {
        match self.kind(initializer) {
            SyntaxKind::CallExpression => {
                self.field(initializer, "expression").is_some_and(|callee| {
                    matches!(
                        self.kind(syntax::skip_parentheses(self.ast, callee)),
                        SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction
                    )
                })
            }
            SyntaxKind::FunctionExpression
            | SyntaxKind::ClassExpression
            | SyntaxKind::ArrowFunction => true,
            SyntaxKind::ObjectLiteralExpression => {
                prototype || ids(self.ast.node(initializer), "properties").is_empty()
            }
            _ => false,
        }
    }

    /// `getAssignedExpandoInitializer` of an `Object.defineProperty` call:
    /// a descriptor whose `value` is an expando initializer.
    fn call_expando_initializer(&self, call: NodeId) -> bool {
        let arguments = ids(self.ast.node(call), "arguments");
        let (Some(&name), Some(&descriptor)) = (arguments.get(1), arguments.get(2)) else {
            return false;
        };
        let prototype = matches!(literal_key(self.ast, self.ast.node(name)), MemberKey::Text(text) if &*text == "prototype");
        self.kind(descriptor) == SyntaxKind::ObjectLiteralExpression
            && ids(self.ast.node(descriptor), "properties")
                .iter()
                .any(|&property| {
                    self.kind(property) == SyntaxKind::PropertyAssignment
                        && self
                            .field(property, "name")
                            .is_some_and(|name| self.is_identifier_named(name, "value"))
                        && self
                            .field(property, "initializer")
                            .is_some_and(|init| self.expando_initializer(init, prototype))
                })
    }

    /// `isExportsOrModuleExportsOrAlias`: `exports`, `module.exports`, or a
    /// variable initialized to one, at the file's top level.
    fn exports_or_alias(&mut self, node: NodeId, at: NodeId) -> bool {
        let root = self.ast.root_id();
        let mut queue = std::collections::VecDeque::from([node]);
        let mut steps = 0;
        while let Some(node) = queue.pop_front() {
            steps += 1;
            if steps > 100 {
                break;
            }
            if self.is_identifier_named(node, "exports")
                || syntax::is_module_exports_access_expression(self.ast, node)
            {
                return true;
            }
            if self.kind(node) == SyntaxKind::Identifier
                && let Some(symbol) = self.lookup_name(root, node, at)
                && let Some(value) = self.symbols[symbol].value
                && self.kind(value) == SyntaxKind::VariableDeclaration
                && let Some(initializer) = self.field(value, "initializer")
            {
                queue.push_back(initializer);
                if syntax::is_assignment_expression(self.ast, initializer, true) {
                    queue.extend(self.field(initializer, "left"));
                    queue.extend(self.field(initializer, "right"));
                }
            }
        }
        false
    }

    /// `lookupSymbolForPropertyAccess`: an identifier in `container`, and a
    /// member of what its receiver names in the binder's current container.
    fn lookup_access(&mut self, node: NodeId, container: NodeId, at: NodeId) -> Option<SymbolId> {
        if self.kind(node) == SyntaxKind::Identifier {
            return self.lookup_name(container, node, at);
        }
        let receiver = self.field(node, "expression")?;
        let (current, _) = containers_of(self.ast, self.containers, node);
        let symbol = self.lookup_access(receiver, current, at)?;
        let key = self.access_name(node)?;
        self.symbols[symbol].exports.get(&key).copied()
    }

    /// `lookupSymbolForName`: a local of `container` bound before `at`, or
    /// at the top level a global a namespace assignment declared.
    fn lookup_name(
        &mut self,
        container: NodeId,
        identifier: NodeId,
        at: NodeId,
    ) -> Option<SymbolId> {
        let name = self.ast.identifier_name(identifier)?.to_owned();
        let owner = (self.kind(container), self.ast.node(container).span);
        let bound_before = self.order[at.index()];
        let visible: Vec<(NodeId, LocalKind)> = self
            .locals
            .get(&(owner, name.clone()))
            .map(|locals| {
                locals
                    .iter()
                    .filter(|local| self.order[local.node.index()] < bound_before)
                    .map(|local| (local.node, local.kind))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(&(node, kind)) = visible.first() {
            let symbol = self.local_symbol(owner, name, node, kind);
            for &(node, kind) in &visible {
                let symbol = &mut self.symbols[symbol];
                match kind {
                    LocalKind::Function => symbol.function = true,
                    LocalKind::Class => symbol.class = true,
                    LocalKind::Alias => symbol.alias = true,
                    LocalKind::Value => {}
                }
                if kind != LocalKind::Alias && symbol.value.is_none() {
                    symbol.value = Some(node);
                }
            }
            return Some(symbol);
        }
        if container == self.ast.root_id() {
            let key = literal_key(self.ast, self.ast.node(identifier));
            return self.globals.get(&key).copied();
        }
        None
    }

    fn local_symbol(
        &mut self,
        owner: Site,
        name: String,
        node: NodeId,
        kind: LocalKind,
    ) -> SymbolId {
        if let Some(&symbol) = self.local_symbols.get(&(owner, name.clone())) {
            return symbol;
        }
        let symbol = self.new_symbol(Symbol {
            function: kind == LocalKind::Function,
            class: kind == LocalKind::Class,
            alias: kind == LocalKind::Alias,
            value: (kind != LocalKind::Alias).then_some(node),
            ..Symbol::default()
        });
        self.local_symbols.insert((owner, name), symbol);
        symbol
    }

    fn new_symbol(&mut self, symbol: Symbol) -> SymbolId {
        self.symbols.push(symbol);
        self.symbols.len() - 1
    }

    /// Every JSDoc tag 5.6.3 gives meaning: a type alias or import its binder
    /// declares from any attached comment, and each tag a node reads where
    /// its comments reach it.
    fn jsdoc(&mut self) {
        let ast = self.ast;
        let mut refused: Vec<(Span, Construct)> = Vec::new();
        for tags in self.tags.values() {
            for tag in tags.comments.iter().flatten() {
                let declares = match &tag.syntax {
                    TagSyntax::Typedef { .. } | TagSyntax::Callback => true,
                    TagSyntax::Import { clause } => *clause,
                    TagSyntax::Other => tag.name == "enum",
                    _ => false,
                };
                if declares {
                    refused.push((tag.span, Construct::Tag(tag.name.clone())));
                }
            }
        }
        for node in ast.walk() {
            if !self.may_read(node) {
                continue;
            }
            let mut first_type = true;
            for (tag, attached) in self.owned_tags(node) {
                // A variable's first `@type` is its declared type
                // (`getJSDocType`), the meaning of the one the parser
                // reparsed onto it (`crate::parser::jsdoc`); a later
                // declaration's reparsed one 5.6.3 does not read.
                if tag.syntax == TagSyntax::Type
                    && std::mem::take(&mut first_type)
                    && self.kind(node) == SyntaxKind::VariableDeclaration
                    && tag.type_span.and_then(|span| {
                        crate::parser::jsdoc::reparsed_type_declaration(ast, attached, span)
                    }) == Some(node)
                {
                    continue;
                }
                if self.reads(node, tag, attached) {
                    refused.push((tag.span, Construct::Tag(tag.name.clone())));
                }
            }
            if self.kind(node) == SyntaxKind::Parameter {
                for tag in self.parameter_tags(node) {
                    if let TagSyntax::Parameter {
                        typed,
                        bracketed,
                        optional_type,
                        ..
                    } = tag.syntax
                        && (typed || bracketed || optional_type)
                    {
                        refused.push((tag.span, Construct::Tag(tag.name.clone())));
                    }
                }
            }
        }
        self.refusals.extend(refused);
    }

    /// `getJSDocCommentsAndTags`: the tags `host` owns, each with the node
    /// its comment is attached to, up to a parameter (whose function's
    /// parameter tags `parameter_tags` gives).
    fn owned_tags(&self, host: NodeId) -> Vec<(&'a TagFacts, NodeId)> {
        let mut owned = Vec::new();
        if is_variable_like(self.kind(host))
            && let Some(initializer) = self.field(host, "initializer")
        {
            self.filter_owned(host, initializer, &mut owned);
        }
        let mut node = host;
        while self.parent(node).is_some() {
            self.filter_owned(host, node, &mut owned);
            if matches!(
                self.kind(node),
                SyntaxKind::Parameter | SyntaxKind::TypeParameter
            ) {
                break;
            }
            match self.next_comment_location(node) {
                Some(next) => node = next,
                None => break,
            }
        }
        owned
    }

    /// `filterOwnedJSDocTags`: of `attached`'s comments, the last one's tags
    /// but a `@type` or `@satisfies` of a parenthesized expression other than
    /// `host`, and only `@overload` tags of the others.
    fn filter_owned(
        &self,
        host: NodeId,
        attached: NodeId,
        owned: &mut Vec<(&'a TagFacts, NodeId)>,
    ) {
        let tags: &'a HashMap<NodeId, Arc<Tags>> = self.tags;
        let Some(tags) = tags.get(&attached) else {
            return;
        };
        let last = tags.comments.len().saturating_sub(1);
        for (index, comment) in tags.comments.iter().enumerate() {
            for tag in comment {
                let keep = if index == last {
                    !matches!(tag.syntax, TagSyntax::Type | TagSyntax::Satisfies)
                        || self.kind(attached) != SyntaxKind::ParenthesizedExpression
                        || attached == host
                } else {
                    tag.syntax == TagSyntax::Overload
                };
                if keep {
                    owned.push((tag, attached));
                }
            }
        }
    }

    /// `getNextJSDocCommentLocation`.
    fn next_comment_location(&self, node: NodeId) -> Option<NodeId> {
        let parent = self.parent(node)?;
        let parent_kind = self.kind(parent);
        if matches!(
            parent_kind,
            SyntaxKind::PropertyAssignment
                | SyntaxKind::ExportAssignment
                | SyntaxKind::PropertyDeclaration
        ) || parent_kind == SyntaxKind::ExpressionStatement
            && self.kind(node) == SyntaxKind::PropertyAccessExpression
            || parent_kind == SyntaxKind::ReturnStatement
            || syntax::is_assignment_expression(self.ast, node, false)
        {
            return Some(parent);
        }
        let grandparent = self.parent(parent)?;
        if self.single_variable(grandparent) == Some(node)
            || syntax::is_assignment_expression(self.ast, parent, false)
        {
            return Some(grandparent);
        }
        let great = self.parent(grandparent)?;
        if self.single_variable(great).is_some()
            || self.single_initializer(great) == Some(node)
            || self.defaulted_assignment_source(great).is_some()
        {
            return Some(great);
        }
        None
    }

    /// `getSingleVariableOfVariableStatement`.
    fn single_variable(&self, node: NodeId) -> Option<NodeId> {
        if self.kind(node) != SyntaxKind::VariableStatement {
            return None;
        }
        let list = self.field(node, "declarationList")?;
        ids(self.ast.node(list), "declarations").first().copied()
    }

    /// `getSingleInitializerOfVariableStatementOrPropertyDeclaration`.
    fn single_initializer(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            SyntaxKind::VariableStatement => self
                .single_variable(node)
                .and_then(|declaration| self.field(declaration, "initializer")),
            SyntaxKind::PropertyDeclaration | SyntaxKind::PropertyAssignment => {
                self.field(node, "initializer")
            }
            _ => None,
        }
    }

    /// `getSourceOfDefaultedAssignment`.
    fn defaulted_assignment_source(&self, node: NodeId) -> Option<NodeId> {
        if self.kind(node) != SyntaxKind::ExpressionStatement {
            return None;
        }
        let expression = self.field(node, "expression")?;
        if self.kind(expression) != SyntaxKind::BinaryExpression
            || self.assignment_kind(expression) == Kind::None
        {
            return None;
        }
        let right = self.field(expression, "right")?;
        (self.kind(right) == SyntaxKind::BinaryExpression
            && matches!(
                self.operator(right),
                Some(SyntaxKind::BarBarToken | SyntaxKind::QuestionQuestionToken)
            ))
        .then(|| self.field(right, "right"))
        .flatten()
    }

    /// `getJSDocParameterTags`: of the function's tags, those naming the
    /// parameter, or for a binding pattern the one at its position.
    fn parameter_tags(&self, parameter: NodeId) -> Vec<&'a TagFacts> {
        let Some(function) = self.parent(parameter) else {
            return Vec::new();
        };
        let tags: Vec<&'a TagFacts> = self
            .owned_tags(function)
            .into_iter()
            .map(|(tag, _)| tag)
            .filter(|tag| matches!(tag.syntax, TagSyntax::Parameter { .. }))
            .collect();
        let Some(name) = self.field(parameter, "name") else {
            return Vec::new();
        };
        if self.kind(name) == SyntaxKind::Identifier {
            let name = self.ast.identifier_name(name);
            return tags
                .into_iter()
                .filter(|tag| {
                    matches!(&tag.syntax, TagSyntax::Parameter { name: Some(tag_name), .. } if Some(tag_name.as_str()) == name)
                })
                .collect();
        }
        let position = ids(self.ast.node(function), "parameters")
            .iter()
            .position(|&each| each == parameter);
        position
            .and_then(|position| tags.get(position).copied())
            .into_iter()
            .collect()
    }

    /// Whether `node` is one of the hosts at which `reads` takes a tag.
    fn may_read(&self, node: NodeId) -> bool {
        self.declared_nodes.contains(&node)
            || matches!(
                self.kind(node),
                SyntaxKind::VariableDeclaration
                    | SyntaxKind::Parameter
                    | SyntaxKind::PropertyDeclaration
                    | SyntaxKind::PropertyAssignment
                    | SyntaxKind::ShorthandPropertyAssignment
                    | SyntaxKind::BindingElement
                    | SyntaxKind::ExportAssignment
                    | SyntaxKind::ExpressionStatement
                    | SyntaxKind::ParenthesizedExpression
                    | SyntaxKind::FunctionDeclaration
                    | SyntaxKind::FunctionExpression
                    | SyntaxKind::ArrowFunction
                    | SyntaxKind::MethodDeclaration
                    | SyntaxKind::Constructor
                    | SyntaxKind::GetAccessor
                    | SyntaxKind::SetAccessor
                    | SyntaxKind::ClassDeclaration
                    | SyntaxKind::ClassExpression
            )
    }

    /// Whether 5.6.3 reads `tag`, attached to `attached`, at `host`.
    fn reads(&self, host: NodeId, tag: &TagFacts, attached: NodeId) -> bool {
        let kind = self.kind(host);
        let function_like = matches!(
            kind,
            SyntaxKind::FunctionDeclaration
                | SyntaxKind::FunctionExpression
                | SyntaxKind::ArrowFunction
                | SyntaxKind::MethodDeclaration
                | SyntaxKind::Constructor
                | SyntaxKind::GetAccessor
                | SyntaxKind::SetAccessor
        );
        let class = matches!(
            kind,
            SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
        );
        // A declaration the binder made of an assignment or an access
        // expression statement.
        let declared = self.declared_nodes.contains(&host)
            || kind == SyntaxKind::ExpressionStatement
                && self
                    .field(host, "expression")
                    .is_some_and(|expression| self.declared_nodes.contains(&expression));
        match &tag.syntax {
            // `getEffectiveTypeAnnotationNode`, `getSignatureOfTypeTag`, and a
            // JSDoc type assertion (`isJSDocTypeAssertion`).
            TagSyntax::Type => {
                function_like
                    || declared
                    || matches!(
                        kind,
                        SyntaxKind::VariableDeclaration
                            | SyntaxKind::Parameter
                            | SyntaxKind::PropertyDeclaration
                            | SyntaxKind::PropertyAssignment
                            | SyntaxKind::ShorthandPropertyAssignment
                            | SyntaxKind::BindingElement
                            | SyntaxKind::ExportAssignment
                    )
                    || kind == SyntaxKind::ParenthesizedExpression && attached == host
            }
            // `isJSDocSatisfiesExpression`, and a declaration's, which types
            // its initializer contextually
            // (`getContextualTypeForVariableLikeDeclaration`, for a property
            // assignment through `getContextualTypeForObjectLiteralElement`)
            // and, but for a property assignment, checks it
            // (`checkDeclarationInitializer`).
            TagSyntax::Satisfies => {
                kind == SyntaxKind::ParenthesizedExpression && attached == host
                    || matches!(
                        kind,
                        SyntaxKind::VariableDeclaration
                            | SyntaxKind::Parameter
                            | SyntaxKind::PropertyDeclaration
                            | SyntaxKind::PropertyAssignment
                            | SyntaxKind::BindingElement
                    ) && self.field(host, "initializer").is_some()
            }
            TagSyntax::Return { typed } => *typed && function_like,
            TagSyntax::This => function_like,
            TagSyntax::Template => function_like || class,
            // `getEffectiveImplementsTypeNodes`, and `getEffectiveBaseTypeNode`
            // for a class with an `extends` clause.
            TagSyntax::Implements => class,
            TagSyntax::Augments { .. } => {
                class
                    && ids(self.ast.node(host), "heritageClauses")
                        .iter()
                        .any(|&clause| {
                            syntax::number(self.ast.node(clause), "token")
                                == Some(SyntaxKind::ExtendsKeyword.number())
                        })
            }
            // `getEffectiveModifierFlags` of a member: a class's takes each
            // modifier, while `getDeclarationModifierFlagsFromSymbol` drops the
            // accessibility of an object literal's, which keeps `@readonly`.
            TagSyntax::Modifier => {
                let in_class = self.parent(host).is_some_and(|parent| {
                    matches!(
                        self.kind(parent),
                        SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                    )
                });
                match kind {
                    SyntaxKind::PropertyDeclaration | SyntaxKind::Constructor => true,
                    SyntaxKind::MethodDeclaration
                    | SyntaxKind::GetAccessor
                    | SyntaxKind::SetAccessor
                    | SyntaxKind::PropertyAssignment
                    | SyntaxKind::ShorthandPropertyAssignment => in_class || tag.name == "readonly",
                    _ => declared,
                }
            }
            // `getJSDocOverloadTags` of a declaration with its own comments.
            TagSyntax::Overload => function_like && self.tags.contains_key(&host),
            // `isJSConstructor`.
            TagSyntax::Other if matches!(tag.name.as_str(), "class" | "constructor") => {
                matches!(
                    kind,
                    SyntaxKind::FunctionDeclaration | SyntaxKind::FunctionExpression
                ) || matches!(
                    kind,
                    SyntaxKind::VariableDeclaration | SyntaxKind::PropertyAssignment
                ) && self.field(host, "initializer").is_some_and(|initializer| {
                    self.kind(initializer) == SyntaxKind::FunctionExpression
                })
            }
            _ => false,
        }
    }
}

/// `isVariableLike`.
fn is_variable_like(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::BindingElement
            | SyntaxKind::EnumMember
            | SyntaxKind::Parameter
            | SyntaxKind::PropertyAssignment
            | SyntaxKind::PropertyDeclaration
            | SyntaxKind::PropertySignature
            | SyntaxKind::ShorthandPropertyAssignment
            | SyntaxKind::VariableDeclaration
    )
}

/// `isVoidZero`.
fn is_void_zero(ast: &Ast<'_>, node: NodeId) -> bool {
    let syntax = ast.node(node);
    syntax.syntax_kind == SyntaxKind::VoidExpression
        && id(syntax, "expression").is_some_and(|operand| {
            let operand = ast.node(operand);
            operand.syntax_kind == SyntaxKind::NumericLiteral
                && matches!(operand.field("text"), Some(Value::Text(text)) if text.as_ref() == "0")
        })
}

/// 5.6.3's binding order: each node before its children, a source file's,
/// block's, or module block's function declarations before its other
/// statements (`bindEachFunctionsFirst`), and the children `bindChildren`
/// takes out of syntax order as it does.
fn binding_order(ast: &Ast<'_>) -> Vec<usize> {
    let mut order = vec![usize::MAX; ast.nodes().len()];
    let mut next = 0;
    let mut stack = vec![ast.root_id()];
    while let Some(node) = stack.pop() {
        order[node.index()] = next;
        next += 1;
        let kind = ast.node(node).syntax_kind;
        let mut children = Vec::new();
        let mut fields: Vec<_> = ast.logical_fields(node).collect();
        // The fields `bindChildren` visits out of syntax order: a loop's
        // iterated expression before its variable (`bindForInOrForOfStatement`),
        // its body before its incrementor (`bindForStatement`), an immediately
        // invoked function's arguments before it (`bindCallExpressionFlow`),
        // and an initializer before the name it binds (`bindParameterFlow`,
        // `bindBindingElementFlow`).
        let iife = kind == SyntaxKind::CallExpression
            && syntax::id(ast.node(node), "expression").is_some_and(|callee| {
                matches!(
                    ast.node(syntax::skip_parentheses(ast, callee)).syntax_kind,
                    SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction
                )
            });
        let (early, late) = match kind {
            SyntaxKind::ForInStatement | SyntaxKind::ForOfStatement => ("expression", ""),
            SyntaxKind::ForStatement => ("", "incrementor"),
            SyntaxKind::CallExpression if iife => ("", "expression"),
            SyntaxKind::Parameter | SyntaxKind::BindingElement => ("", "name"),
            _ => ("", ""),
        };
        fields.sort_by_key(|&(key, _)| match key {
            _ if key == early => 0,
            _ if key == late => 2,
            _ => 1,
        });
        for (key, value) in fields {
            match value {
                Value::Node(child) => children.push(*child),
                Value::Nodes(list)
                    if key == "statements"
                        && matches!(
                            kind,
                            SyntaxKind::SourceFile | SyntaxKind::Block | SyntaxKind::ModuleBlock
                        ) =>
                {
                    let function = |id: &&NodeId| {
                        ast.node(**id).syntax_kind == SyntaxKind::FunctionDeclaration
                    };
                    children.extend(list.iter().filter(function));
                    children.extend(list.iter().filter(|id| !function(id)));
                }
                Value::Nodes(list) => children.extend(list.iter().copied()),
                _ => {}
            }
        }
        children.reverse();
        stack.extend(children);
    }
    order
}
