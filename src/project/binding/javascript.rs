//! The JavaScript constructs this tool refuses (docs/references.md,
//! JavaScript inputs): those to which TypeScript 7.0.2 gives JavaScript-only
//! meaning that this tool does not model, each located where TypeScript
//! decides it.
//!
//! - A declaration the binder creates from an assignment or
//!   `Object.defineProperty` call (`GetAssignmentDeclarationKind`): a
//!   CommonJS export where `setCommonJSModuleIndicator` succeeds
//!   (`bindModuleExportsAssignment`, `bindExportsOrObjectDefineProperty`),
//!   but the one `module.exports = name` this tool binds (`commonjs`); a
//!   member of a function, class, or object where
//!   `bindDeferredExpandoAssignment` declares one, judged by its own
//!   `lookupEntity`, `getInitializerSymbol`, and existing-member rules; and a
//!   class or object literal member `bindThisPropertyAssignment` declares.
//! - A `require` call, which makes a script a CommonJS module
//!   (`bindCallExpression`), names a module the loader follows
//!   (`ForEachDynamicImportOrRequireCall`), and resolves to it in the checker.
//! - A JSDoc tag the parser reparses into syntax: at its host
//!   (`reparseHosted`, for the last comment only), or as a declaration of its
//!   own (`reparseUnhosted`). A variable's `@type`, which this parser
//!   reparses as TypeScript does (`crate::parser::jsdoc`), is refused only
//!   outside a variable statement or where its type has JavaScript-only
//!   meaning: JSDoc type syntax, a name `getIntendedTypeFromJSDocTypeReference`
//!   reads as another type, or syntax that needed recovery.
//! - A JSDoc import type with a literal argument, whose module the loader
//!   follows from a JavaScript file (`ForEachDynamicImportOrRequireCall`),
//!   but in a reparsed `@type`, where the loader follows it as syntax.
//!
//! The TypeScript 5.6 dialect also refuses what 5.6.3 gives such meaning
//! (`typescript_5_6`): its binder's declarations, which include a
//! constructor function's members, namespaces, and a top-level `this`
//! variable, and the JSDoc tags its binder and checker read where they
//! stand. A `require` call and a literal JSDoc import type mean the same to
//! both. Both dialects answer through the same binder and checker, which
//! port 7.0.2 for this slice alone and follow it beyond the slice only in
//! part, as the CommonJS module indicator does (`commonjs`), so what 7.0.2
//! alone gives meaning, such as an assignment of `void 0`, to which 5.6.3's
//! `getAssignmentDeclarationKind` gives none, stays refused under both:
//! declaring nothing under 5.6.3 does not make it answerable. Capture keeps
//! every candidate with the dialects whose compilers give it meaning
//! (`Meaning`), and each generation's dialect selects among them (`Rules`).

use super::collect::{Alias, AliasBind, Decl, DeclKind, Scope};
use super::flow::syntax::Facts as Containers;
use super::member::{MemberKey, declaration_key, literal_key};
use crate::analysis::InputError;
use crate::parser::jsdoc::{Attachment, TagFacts, TagSyntax, Tags};
use crate::syntax::{self, AssignmentDeclarationKind, id, ids};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Span};
use std::collections::HashMap;
use std::sync::Arc;

pub(super) mod commonjs;
mod typescript_5_6;

/// A refused construct, where TypeScript decides its meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::project) struct JsUnsupported {
    pub(in crate::project) span: Span,
    pub(in crate::project) construct: Construct,
    meaning: Meaning,
}

/// Which dialects' compilers give a construct its JavaScript-only meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Meaning {
    typescript_7: bool,
    typescript_5_6: bool,
}

impl Meaning {
    const TYPESCRIPT_7: Self = Self {
        typescript_7: true,
        typescript_5_6: false,
    };
    const TYPESCRIPT_5_6: Self = Self {
        typescript_7: false,
        typescript_5_6: true,
    };
    const BOTH: Self = Self {
        typescript_7: true,
        typescript_5_6: true,
    };
}

/// The refusals of one dialect (`compatibility::Dialect`), which only this
/// module interprets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::project) struct Rules {
    typescript_5_6: bool,
}

impl Rules {
    pub(in crate::project) const TYPESCRIPT_7: Self = Self {
        typescript_5_6: false,
    };
    pub(in crate::project) const TYPESCRIPT_5_6: Self = Self {
        typescript_5_6: true,
    };

    /// Whether this dialect refuses `refusal`: whether 7.0.2 gives it
    /// meaning, or the dialect's own compiler does. The binder and checker
    /// every dialect answers through port 7.0.2 for the supported slice
    /// alone, and answer a file only where they model each meaning 7.0.2
    /// gives it, whatever 5.6.3 reads: for `exports.a = void 0`, to which
    /// 5.6.3 gives none, they would set the CommonJS module indicator by
    /// 7.0.2's classification (`commonjs::has_indicator`) and bind no export,
    /// an answer neither compiler gives.
    pub(in crate::project) fn refuses(self, refusal: &JsUnsupported) -> bool {
        refusal.meaning.typescript_7 || self.typescript_5_6 && refusal.meaning.typescript_5_6
    }
}

/// What gives a refused construct its JavaScript-only meaning: mostly an
/// assignment declaration kind (`GetAssignmentDeclarationKind`), or the
/// binder's `this`, `require`, or JSDoc reparse rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::project) enum Construct {
    ModuleExports,
    ExportsAssignment,
    DefinePropertyExport,
    Expando,
    DefinePropertyExpando,
    ThisProperty,
    /// A `this` assignment in a function (5.6.3).
    ConstructorThis,
    /// A `this` assignment at a file's top level (5.6.3).
    TopLevelThis,
    /// A prototype assignment or `Object.defineProperty` call (5.6.3).
    PrototypeMember,
    /// A top-level property assignment (5.6.3).
    NamespaceMember,
    /// A top-level `Object.defineProperty` call (5.6.3).
    DefinePropertyNamespaceMember,
    Require,
    /// A JSDoc tag, by its name as written.
    Tag(String),
    ImportType,
    /// A reparsed `@type`'s type with JavaScript-only meaning.
    JsdocType,
}

impl JsUnsupported {
    /// What loading reports at the construct.
    pub(in crate::project) fn message(&self) -> String {
        let construct = match &self.construct {
            Construct::ModuleExports => "CommonJS module.exports assignment".to_owned(),
            Construct::ExportsAssignment => "CommonJS exports assignment".to_owned(),
            Construct::DefinePropertyExport => "CommonJS Object.defineProperty export".to_owned(),
            Construct::Expando => {
                "property assignment that declares a member of a function, class, or object"
                    .to_owned()
            }
            Construct::DefinePropertyExpando => "Object.defineProperty call that declares a \
                                                 member of a function, class, or object"
                .to_owned(),
            Construct::ThisProperty => {
                "this assignment that declares a class or object literal member".to_owned()
            }
            Construct::ConstructorThis => {
                "this assignment that TypeScript 5.6 reads as a constructor function member"
                    .to_owned()
            }
            Construct::TopLevelThis => {
                "top-level this assignment that TypeScript 5.6 reads as a variable".to_owned()
            }
            Construct::PrototypeMember => {
                "prototype assignment that TypeScript 5.6 reads as a constructor function member"
                    .to_owned()
            }
            Construct::NamespaceMember => "top-level property assignment that TypeScript 5.6 \
                                           reads as a namespace member"
                .to_owned(),
            Construct::DefinePropertyNamespaceMember => "top-level Object.defineProperty call \
                                                         that TypeScript 5.6 reads as a \
                                                         namespace member"
                .to_owned(),
            Construct::Require => "require call".to_owned(),
            Construct::Tag(name) => format!("JSDoc @{name} tag"),
            Construct::ImportType => "JSDoc import type".to_owned(),
            Construct::JsdocType => "JSDoc type with JavaScript-only meaning".to_owned(),
        };
        format!(
            "unsupported JavaScript: {construct} (tsparse reads JavaScript as plain ES modules \
             or one CommonJS module.exports, and JSDoc only as a variable's @type)"
        )
    }
}

/// The refused constructs of a JavaScript file, in source order. `scopes`,
/// `decls`, and `aliases` are the file's collected binding facts, and
/// `exports` what decides its CommonJS exports.
/// `jsdoc` parses a node's attached comments; its failures, such as an
/// exhausted resource, stay failures.
pub(super) fn refusals(
    ast: &Ast<'_>,
    scopes: &[Scope],
    decls: &[Decl],
    aliases: &[Alias],
    exports: commonjs::Exports,
    mut jsdoc: impl FnMut(SyntaxKind, &Attachment) -> Result<Arc<Tags>, InputError>,
) -> Result<Vec<JsUnsupported>, InputError> {
    let commonjs::Exports {
        module_syntax,
        admitted,
    } = exports;
    let mut classifier = Classifier::new(ast, scopes, decls, aliases);
    let mut expandos = Vec::new();
    let mut tags_by_node = HashMap::new();
    for node in ast.walk() {
        let syntax = ast.node(node);
        let span = ast.token_span_of(syntax);
        match syntax::assignment_declaration_kind(ast, node, true) {
            AssignmentDeclarationKind::ModuleExports
                if !module_syntax && admitted != Some(node) =>
            {
                classifier.refuse(span, Construct::ModuleExports);
            }
            AssignmentDeclarationKind::ExportsProperty if !module_syntax => {
                classifier.refuse(span, Construct::ExportsAssignment);
            }
            AssignmentDeclarationKind::ObjectDefinePropertyExports if !module_syntax => {
                classifier.refuse(span, Construct::DefinePropertyExport);
            }
            AssignmentDeclarationKind::ThisProperty if classifier.this_property(node) => {
                classifier.refuse(span, Construct::ThisProperty);
            }
            AssignmentDeclarationKind::Property
            | AssignmentDeclarationKind::ObjectDefinePropertyValue => expandos.push(node),
            _ => {}
        }
        if syntax::is_require_call(ast, node, false) {
            classifier.refuse_in(span, Construct::Require, Meaning::BOTH);
        }
        if let Some(attachment) = &syntax.native_jsdoc {
            let tags = jsdoc(syntax.syntax_kind, attachment)?;
            classifier.jsdoc(node, &tags);
            tags_by_node.insert(node, tags);
        }
    }
    // `bindDeferredExpandoAssignments`, once the file's own declarations
    // are all bound, in binding order.
    for node in expandos {
        if classifier.declares_expando(node) {
            let construct = if ast.node(node).syntax_kind == SyntaxKind::CallExpression {
                Construct::DefinePropertyExpando
            } else {
                Construct::Expando
            };
            classifier.refuse(ast.token_span(node), construct);
        }
    }
    let typescript_5_6 = typescript_5_6::Model::new(
        ast,
        &classifier.containers,
        scopes,
        decls,
        aliases,
        exports,
        &tags_by_node,
    )
    .refusals();
    let mut refusals = classifier.refusals;
    refusals.extend(
        typescript_5_6
            .into_iter()
            .map(|(span, construct)| JsUnsupported {
                span,
                construct,
                meaning: Meaning::TYPESCRIPT_5_6,
            }),
    );
    // One refusal per place, with every compiler that gives it meaning,
    // named as TypeScript 7 names it.
    refusals.sort_by_key(|refusal| {
        (
            refusal.span.start,
            refusal.span.end,
            !refusal.meaning.typescript_7,
        )
    });
    refusals.dedup_by(|later, kept| {
        if later.span != kept.span {
            return false;
        }
        kept.meaning.typescript_7 |= later.meaning.typescript_7;
        kept.meaning.typescript_5_6 |= later.meaning.typescript_5_6;
        true
    });
    Ok(refusals)
}

/// The binder's `container` and `blockScopeContainer` at `node`.
fn containers_of(ast: &Ast<'_>, containers: &Containers, node: NodeId) -> (NodeId, NodeId) {
    let mut block = None;
    let ancestors = std::iter::successors(containers.parent(node), |&id| containers.parent(id));
    for ancestor in ancestors {
        let flags = containers.container_flags(ast, ancestor);
        if flags.container() {
            return (ancestor, block.unwrap_or(ancestor));
        }
        if flags.block_scoped() && block.is_none() {
            block = Some(ancestor);
        }
    }
    let root = ast.root_id();
    (root, block.unwrap_or(root))
}

/// A native node by its kind and range, as a scope's native owner and a
/// declaration name it.
type Site = (SyntaxKind, Span);

/// A name a container's locals hold (`lookupName`): by its value
/// declaration, or an import, an alias with none.
#[derive(Clone, Copy)]
enum Local {
    Value(Site),
    Alias,
}

/// A member a deferred expando assignment finds on its target's symbol.
enum Export {
    /// Declared by an earlier assignment, its value declaration.
    Assignment(NodeId),
    /// A static member or `prototype`, which no assignment redeclares.
    Static,
}

struct Classifier<'a, 's> {
    ast: &'a Ast<'s>,
    containers: Containers,
    /// The value declaration of each name in each container's locals, by
    /// the container's native node (`lookupName`).
    locals: HashMap<(Site, &'a str), Local>,
    /// The nodes value declarations name.
    declarations: HashMap<(SyntaxKind, Span), NodeId>,
    /// The members earlier expando assignments declared, by the node whose
    /// symbol they extend.
    exports: HashMap<(NodeId, MemberKey), Export>,
    refusals: Vec<JsUnsupported>,
}

impl<'a, 's> Classifier<'a, 's> {
    fn new(ast: &'a Ast<'s>, scopes: &'a [Scope], decls: &'a [Decl], aliases: &'a [Alias]) -> Self {
        let mut locals: HashMap<_, Local> = HashMap::new();
        for decl in decls {
            let value = matches!(
                decl.kind,
                DeclKind::Var
                    | DeclKind::Let
                    | DeclKind::Parameter
                    | DeclKind::Function
                    | DeclKind::Class
                    | DeclKind::Enum { .. }
            );
            let (Some(scope), Some(span), true) = (decl.lexical, decl.native_span, value) else {
                continue;
            };
            let Some(owner) = scopes[scope.index()].native_owner else {
                continue;
            };
            // `SetValueDeclaration`, in binding order.
            locals
                .entry((owner, &*decl.name))
                .and_modify(|current: &mut Local| {
                    if let Local::Value(site) = current
                        && super::should_replace_value_declaration(Some(site.0), decl.native_kind)
                    {
                        *site = (decl.native_kind, span);
                    }
                })
                .or_insert(Local::Value((decl.native_kind, span)));
        }
        for alias in aliases {
            let (AliasBind::Lexical { scope, name } | AliasBind::Both { scope, name, .. }) =
                &alias.bind
            else {
                continue;
            };
            if let Some(owner) = scopes[scope.index()].native_owner {
                locals.entry((owner, name.as_str())).or_insert(Local::Alias);
            }
        }
        let declarations = ast
            .walk()
            .filter(|&id| {
                matches!(
                    ast.node(id).syntax_kind,
                    SyntaxKind::VariableDeclaration
                        | SyntaxKind::FunctionDeclaration
                        | SyntaxKind::ClassDeclaration
                )
            })
            .map(|id| ((ast.node(id).syntax_kind, ast.node(id).span), id))
            .collect();
        Self {
            ast,
            containers: Containers::new(ast),
            locals,
            declarations,
            exports: HashMap::new(),
            refusals: Vec::new(),
        }
    }

    fn refuse(&mut self, span: Span, construct: Construct) {
        self.refuse_in(span, construct, Meaning::TYPESCRIPT_7);
    }

    fn refuse_in(&mut self, span: Span, construct: Construct, meaning: Meaning) {
        self.refusals.push(JsUnsupported {
            span,
            construct,
            meaning,
        });
    }

    fn ancestors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::successors(self.containers.parent(node), |&id| {
            self.containers.parent(id)
        })
    }

    /// `bindThisPropertyAssignment`: whether a `this` assignment other than
    /// to a private name declares a member of the class, or object literal,
    /// whose member or constructor is its `thisContainer`.
    fn this_property(&self, node: NodeId) -> bool {
        let ast = self.ast;
        let Some(left) = id(ast.node(node), "left") else {
            return false;
        };
        let left = ast.node(left);
        if left.syntax_kind == SyntaxKind::PropertyAccessExpression
            && id(left, "name")
                .is_some_and(|name| ast.node(name).syntax_kind == SyntaxKind::PrivateIdentifier)
        {
            return false;
        }
        let container = self.ancestors(node).find(|&ancestor| {
            self.containers
                .container_flags(ast, ancestor)
                .this_container()
        });
        container.is_some_and(|container| {
            matches!(
                ast.node(container).syntax_kind,
                SyntaxKind::Constructor
                    | SyntaxKind::PropertyDeclaration
                    | SyntaxKind::MethodDeclaration
                    | SyntaxKind::GetAccessor
                    | SyntaxKind::SetAccessor
                    | SyntaxKind::ClassStaticBlockDeclaration
            )
        })
    }

    /// `bindDeferredExpandoAssignment`: whether the assignment or
    /// `Object.defineProperty` call declares a member of the symbol its
    /// target names, recording it for later assignments.
    fn declares_expando(&mut self, node: NodeId) -> bool {
        let ast = self.ast;
        let syntax = ast.node(node);
        let (parent, name) = if syntax.syntax_kind == SyntaxKind::CallExpression {
            let arguments = ids(syntax, "arguments");
            (arguments[0], Some(arguments[1]))
        } else {
            let Some(left) = id(syntax, "left") else {
                return false;
            };
            let Some(parent) = id(ast.node(left), "expression") else {
                return false;
            };
            // `getElementOrPropertyAccessArgumentExpressionOrName`: an
            // element access without a literal argument is its own,
            // dynamic, name.
            let name = syntax::element_or_property_access_name(ast, left)
                .or_else(|| (!syntax::is_dynamic_name(ast, left)).then_some(left));
            (parent, name)
        };
        let (container, block) = containers_of(self.ast, &self.containers, node);
        let found = self
            .lookup_entity(parent, block)
            .or_else(|| self.lookup_entity(parent, container));
        let Some(target) = found.and_then(|found| self.initializer_symbol(found)) else {
            return false;
        };
        // A dynamic name binds a late-bound member, always.
        let Some(name) = name else {
            return true;
        };
        let key = literal_key(ast, ast.node(name));
        match self.export(target, &key) {
            Some(Export::Static) => false,
            Some(Export::Assignment(_)) => true,
            None => {
                self.exports.insert((target, key), Export::Assignment(node));
                true
            }
        }
    }

    /// A member of `target`'s symbol: one an earlier assignment declared, or
    /// a class's static member or `prototype`.
    fn export(&self, target: NodeId, key: &MemberKey) -> Option<Export> {
        if let Some(export) = self.exports.get(&(target, key.clone())) {
            return Some(match export {
                Export::Assignment(value) => Export::Assignment(*value),
                Export::Static => Export::Static,
            });
        }
        let class = self.ast.node(target);
        if !matches!(
            class.syntax_kind,
            SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
        ) {
            return None;
        }
        if matches!(key, MemberKey::Text(text) if &**text == "prototype") {
            return Some(Export::Static);
        }
        ids(class, "members")
            .iter()
            .any(|&member| {
                syntax::is_static(self.ast, member)
                    && id(self.ast.node(member), "name").is_some()
                    && declaration_key(self.ast, self.ast.node(member)) == *key
            })
            .then_some(Export::Static)
    }

    /// `lookupEntity`: the symbol an entity name denotes in `container`, by
    /// its value declaration, if any. A `this` receiver denotes none, as the
    /// binder has left every `this` container when it binds expandos.
    fn lookup_entity(&self, node: NodeId, container: NodeId) -> Option<Option<NodeId>> {
        let ast = self.ast;
        let syntax = ast.node(node);
        if syntax.syntax_kind == SyntaxKind::Identifier {
            return Some(match self.lookup_local(node, container)? {
                Local::Value(site) => self.declarations.get(&site).copied(),
                Local::Alias => None,
            });
        }
        let receiver = id(syntax, "expression")?;
        if ast.node(receiver).syntax_kind == SyntaxKind::ThisKeyword {
            return None;
        }
        let target = self.initializer_symbol(self.lookup_entity(receiver, container)?)?;
        let name = syntax::element_or_property_access_name(ast, node)?;
        match self.export(target, &literal_key(ast, ast.node(name)))? {
            Export::Assignment(value) => Some(Some(value)),
            Export::Static => Some(None),
        }
    }

    /// `lookupName` in `container`'s locals, for an identifier.
    fn lookup_local(&self, identifier: NodeId, container: NodeId) -> Option<Local> {
        let ast = self.ast;
        let name = ast.identifier_name(identifier)?;
        let owner = (ast.node(container).syntax_kind, ast.node(container).span);
        self.locals.get(&(owner, name)).copied()
    }

    /// `getInitializerSymbol`: the node whose symbol takes expando members,
    /// for a symbol with `value` as its value declaration: a function or
    /// class declaration, or the expando initializer of a variable or an
    /// earlier expando assignment.
    fn initializer_symbol(&self, value: Option<NodeId>) -> Option<NodeId> {
        let ast = self.ast;
        let declaration = value?;
        let syntax = ast.node(declaration);
        let initializer = match syntax.syntax_kind {
            SyntaxKind::FunctionDeclaration | SyntaxKind::ClassDeclaration => {
                return Some(declaration);
            }
            SyntaxKind::VariableDeclaration => id(syntax, "initializer")?,
            SyntaxKind::BinaryExpression => id(syntax, "right")?,
            _ => return None,
        };
        is_expando_initializer(ast, declaration, initializer).then_some(initializer)
    }

    /// The tags of `node`'s attached comments that the reparser reads:
    /// every comment's own declarations, the last comment's tags at their
    /// host, and their literal import types, but for a variable's reparsed
    /// `@type` (`reparsed_type`).
    fn jsdoc(&mut self, node: NodeId, tags: &Tags) {
        let last = tags.comments.len().saturating_sub(1);
        let mut reparsed = Vec::new();
        for (index, comment) in tags.comments.iter().enumerate() {
            let mut parameters = 0;
            for tag in comment {
                let parameter = parameters;
                if matches!(tag.syntax, TagSyntax::Parameter { .. }) {
                    parameters += 1;
                }
                if index == last
                    && let Some(declaration) = self.reparsed_type(node, tag)
                {
                    self.reparsed_type_meaning(tag, declaration);
                    reparsed.extend(tag.type_span);
                    continue;
                }
                if self.unhosted(node, tag) || index == last && self.hosted(node, tag, parameter) {
                    self.refuse(tag.span, Construct::Tag(tag.name.clone()));
                }
            }
        }
        for &start in &tags.import_types {
            if reparsed
                .iter()
                .any(|span: &Span| span.start <= start && start < span.end)
            {
                continue;
            }
            self.refuse_in(
                Span {
                    start,
                    end: start + "import".len(),
                },
                Construct::ImportType,
                Meaning::BOTH,
            );
        }
    }

    /// The declaration to which the parser gave `tag`, a `@type` of a
    /// variable statement or declaration, as its type (`reparseHosted`).
    fn reparsed_type(&self, host: NodeId, tag: &TagFacts) -> Option<NodeId> {
        if tag.syntax != TagSyntax::Type {
            return None;
        }
        crate::parser::jsdoc::reparsed_type_declaration(self.ast, host, tag.type_span?)
    }

    /// Where a reparsed `@type`, which types `declaration`, has meaning this
    /// tool does not model, under both dialects: a declaration outside a
    /// variable statement (a `for`, `for...in`, `for...of`, or `catch`
    /// variable), whose type TypeScript reads otherwise, and a type with
    /// JavaScript-only meaning (`javascript_only_type`).
    fn reparsed_type_meaning(&mut self, tag: &TagFacts, declaration: NodeId) {
        let ast = self.ast;
        let in_statement = self
            .containers
            .parent(declaration)
            .and_then(|list| self.containers.parent(list))
            .is_some_and(|statement| {
                ast.node(statement).syntax_kind == SyntaxKind::VariableStatement
            });
        if !in_statement {
            self.refuse_in(tag.span, Construct::Tag(tag.name.clone()), Meaning::BOTH);
            return;
        }
        let Some(ty) = id(ast.node(declaration), "type") else {
            return;
        };
        let refused = if tag.type_recovered {
            Some(ty)
        } else {
            javascript_only_type(ast, ty)
        };
        if let Some(node) = refused {
            self.refuse_in(
                ast.first_token_span(node),
                Construct::JsdocType,
                Meaning::BOTH,
            );
        }
    }

    /// Whether the parser was within an object literal's members when it
    /// parsed `node` (`PCObjectLiteralMembers`).
    fn in_object_literal(&self, node: NodeId) -> bool {
        self.ancestors(node).any(|ancestor| {
            self.ast.node(ancestor).syntax_kind == SyntaxKind::ObjectLiteralExpression
        })
    }

    /// `reparseUnhosted`: a typedef with a type, a callback, an import with
    /// a clause, and an overload of a function, method, or constructor
    /// outside an object literal declare syntax of their own.
    fn unhosted(&self, host: NodeId, tag: &TagFacts) -> bool {
        match tag.syntax {
            TagSyntax::Typedef { typed } => typed,
            TagSyntax::Callback => true,
            TagSyntax::Import { clause } => clause,
            TagSyntax::Overload => {
                matches!(
                    self.ast.node(host).syntax_kind,
                    SyntaxKind::FunctionDeclaration
                        | SyntaxKind::MethodDeclaration
                        | SyntaxKind::Constructor
                ) && !self.in_object_literal(host)
            }
            _ => false,
        }
    }

    /// `reparseHosted`: whether the tag changes its host's syntax. A host
    /// gains a type, a cast, a full signature, type parameters, a parameter
    /// type or optionality, a `this` parameter, a return type, a modifier,
    /// or a heritage type. `parameter` is the tag's index among the
    /// comment's parameter tags (`findMatchingParameter`).
    fn hosted(&self, host: NodeId, tag: &TagFacts, parameter: usize) -> bool {
        let ast = self.ast;
        let syntax = ast.node(host);
        let kind = syntax.syntax_kind;
        let untyped = |node: NodeId| id(ast.node(node), "type").is_none();
        let declarations = || {
            id(syntax, "declarationList")
                .map(|list| ids(ast.node(list), "declarations").to_vec())
                .unwrap_or_default()
        };
        let assignment_declaration = || {
            id(syntax, "expression").is_some_and(|expression| {
                ast.node(expression).syntax_kind == SyntaxKind::BinaryExpression
                    && syntax::assignment_declaration_kind(ast, expression, true)
                        != AssignmentDeclarationKind::None
            })
        };
        match &tag.syntax {
            TagSyntax::Type => {
                let typed_here = match kind {
                    SyntaxKind::VariableStatement => declarations().into_iter().any(untyped),
                    SyntaxKind::VariableDeclaration
                    | SyntaxKind::ExportAssignment
                    | SyntaxKind::PropertyDeclaration
                    | SyntaxKind::PropertyAssignment
                    | SyntaxKind::ShorthandPropertyAssignment
                    | SyntaxKind::GetAccessor
                    | SyntaxKind::Parameter => untyped(host),
                    SyntaxKind::ExpressionStatement => assignment_declaration(),
                    SyntaxKind::ReturnStatement | SyntaxKind::ParenthesizedExpression => {
                        id(syntax, "expression").is_some()
                    }
                    _ => false,
                };
                typed_here
                    || self.function_like_host(host).is_some_and(|function| {
                        id(ast.node(function), "typeParameters").is_none()
                            && untyped(function)
                            && ids(ast.node(function), "parameters")
                                .iter()
                                .all(|&parameter| untyped(parameter))
                    })
            }
            TagSyntax::Satisfies => match kind {
                SyntaxKind::VariableStatement => declarations()
                    .into_iter()
                    .any(|declaration| id(ast.node(declaration), "initializer").is_some()),
                SyntaxKind::VariableDeclaration
                | SyntaxKind::PropertyDeclaration
                | SyntaxKind::PropertyAssignment => id(syntax, "initializer").is_some(),
                SyntaxKind::ShorthandPropertyAssignment => {
                    id(syntax, "objectAssignmentInitializer").is_some()
                }
                SyntaxKind::ReturnStatement
                | SyntaxKind::ParenthesizedExpression
                | SyntaxKind::ExportAssignment => id(syntax, "expression").is_some(),
                SyntaxKind::ExpressionStatement => assignment_declaration(),
                _ => false,
            },
            TagSyntax::Template => match self.function_like_host(host) {
                Some(function) => id(ast.node(function), "typeParameters").is_none(),
                None => {
                    matches!(
                        kind,
                        SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                    ) && id(syntax, "typeParameters").is_none()
                }
            },
            TagSyntax::Parameter {
                name,
                bracketed,
                typed,
                optional_type,
            } => self
                .function_like_host(host)
                .and_then(|function| self.matching_parameter(function, name.as_deref(), parameter))
                .is_some_and(|matched| {
                    *typed && untyped(matched)
                        || (*bracketed || *optional_type)
                            && id(ast.node(matched), "questionToken").is_none()
                }),
            TagSyntax::This => self.function_like_host(host).is_some_and(|function| {
                ids(ast.node(function), "parameters")
                    .first()
                    .and_then(|&first| id(ast.node(first), "name"))
                    .is_none_or(|name| {
                        let kind = ast.node(name).syntax_kind;
                        kind != SyntaxKind::ThisKeyword
                            && !(kind == SyntaxKind::Identifier
                                && ast.identifier_name(name) == Some("this"))
                    })
            }),
            TagSyntax::Return { typed } => {
                *typed && self.function_like_host(host).is_some_and(untyped)
            }
            TagSyntax::Modifier => {
                let target = if kind == SyntaxKind::ExpressionStatement {
                    match id(syntax, "expression") {
                        Some(expression) => expression,
                        None => return false,
                    }
                } else {
                    host
                };
                match ast.node(target).syntax_kind {
                    SyntaxKind::MethodDeclaration
                    | SyntaxKind::GetAccessor
                    | SyntaxKind::SetAccessor => !self.in_object_literal(target),
                    SyntaxKind::PropertyDeclaration
                    | SyntaxKind::Constructor
                    | SyntaxKind::BinaryExpression => true,
                    _ => false,
                }
            }
            TagSyntax::Implements => matches!(
                kind,
                SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
            ),
            TagSyntax::Augments {
                class,
                type_arguments,
            } => {
                *type_arguments
                    && matches!(
                        kind,
                        SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                    )
                    && self.augments_base(host, class)
            }
            _ => false,
        }
    }

    /// Whether the class's one `extends` type, without type arguments of
    /// its own, names `class` (`HasSamePropertyAccessName`).
    fn augments_base(&self, host: NodeId, class: &[String]) -> bool {
        let ast = self.ast;
        let Some(clause) = ids(ast.node(host), "heritageClauses")
            .iter()
            .copied()
            .find(|&clause| {
                syntax::number(ast.node(clause), "token")
                    == Some(SyntaxKind::ExtendsKeyword.number())
            })
        else {
            return false;
        };
        let [base] = ids(ast.node(clause), "types") else {
            return false;
        };
        let base = ast.node(*base);
        if id(base, "typeArguments").is_some() {
            return false;
        }
        let mut expression = id(base, "expression");
        for name in class.iter().rev() {
            let Some(node) = expression else {
                return false;
            };
            let node = ast.node(node);
            let identifier = match node.syntax_kind {
                SyntaxKind::Identifier => {
                    let identifier = expression;
                    expression = None;
                    identifier
                }
                SyntaxKind::PropertyAccessExpression => {
                    let identifier = id(node, "name");
                    expression = id(node, "expression");
                    identifier
                }
                _ => None,
            };
            if identifier.and_then(|identifier| ast.identifier_name(identifier))
                != Some(name.as_str())
            {
                return false;
            }
        }
        expression.is_none()
    }

    /// `findMatchingParameter`: the parameter a parameter tag names, by an
    /// identifier's name or, for a nameless tag or a binding pattern, by
    /// the tag's position among the comment's parameter tags.
    fn matching_parameter(
        &self,
        function: NodeId,
        name: Option<&str>,
        index: usize,
    ) -> Option<NodeId> {
        let ast = self.ast;
        ids(ast.node(function), "parameters")
            .iter()
            .enumerate()
            .find(|&(position, &parameter)| {
                let Some(parameter_name) = id(ast.node(parameter), "name") else {
                    return false;
                };
                if ast.node(parameter_name).syntax_kind == SyntaxKind::Identifier {
                    name.is_some_and(|name| {
                        ast.identifier_name(parameter_name) == Some(name)
                            || position == index && name.is_empty()
                    })
                } else {
                    position == index
                }
            })
            .map(|(_, &parameter)| parameter)
    }

    /// `getFunctionLikeHost`: the function a tag on `host` describes.
    fn function_like_host(&self, host: NodeId) -> Option<NodeId> {
        let ast = self.ast;
        let syntax = ast.node(host);
        let mut function = match syntax.syntax_kind {
            SyntaxKind::VariableStatement => {
                let list = id(syntax, "declarationList")?;
                match ids(ast.node(list), "declarations").first() {
                    Some(&first) => id(ast.node(first), "initializer")?,
                    None => host,
                }
            }
            SyntaxKind::PropertyAssignment | SyntaxKind::PropertyDeclaration => {
                id(syntax, "initializer")?
            }
            SyntaxKind::ExportAssignment | SyntaxKind::ReturnStatement => id(syntax, "expression")?,
            SyntaxKind::ExpressionStatement => {
                // `GetRightMostAssignedExpression`.
                let mut expression = id(syntax, "expression")?;
                while syntax::is_assignment_expression(ast, expression, true) {
                    expression = id(ast.node(expression), "right")?;
                }
                expression
            }
            _ => host,
        };
        while ast.node(function).syntax_kind == SyntaxKind::SatisfiesExpression {
            function = id(ast.node(function), "expression")?;
        }
        syntax::is_function_like_kind(ast.node(function).syntax_kind).then_some(function)
    }
}

/// The first node, in pre-order, of the reparsed JSDoc type at `root` to
/// which TypeScript gives JavaScript-only meaning: JSDoc type syntax (`*`,
/// `?`, `!`, `T=`, `...T`), or a type reference whose name
/// `getIntendedTypeFromJSDocTypeReference` reads as another type, such as
/// `String` for `string` and `Object` for `any`.
fn javascript_only_type(ast: &Ast<'_>, root: NodeId) -> Option<NodeId> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let syntax = ast.node(node);
        let javascript_only = match syntax.syntax_kind {
            SyntaxKind::JSDocAllType
            | SyntaxKind::JSDocUnknownType
            | SyntaxKind::JSDocNullableType
            | SyntaxKind::JSDocNonNullableType
            | SyntaxKind::JSDocOptionalType
            | SyntaxKind::JSDocFunctionType
            | SyntaxKind::JSDocVariadicType
            | SyntaxKind::JSDocNamepathType
            | SyntaxKind::JSDocTypeLiteral
            | SyntaxKind::JSDocSignature => true,
            SyntaxKind::TypeReference => id(syntax, "typeName")
                .filter(|&name| ast.node(name).syntax_kind == SyntaxKind::Identifier)
                .and_then(|name| ast.identifier_name(name))
                .is_some_and(|name| {
                    matches!(
                        name,
                        "String"
                            | "Number"
                            | "BigInt"
                            | "Boolean"
                            | "Void"
                            | "Undefined"
                            | "Null"
                            | "Function"
                            | "function"
                            | "array"
                            | "promise"
                            | "Object"
                    )
                }),
            _ => false,
        };
        if javascript_only {
            return Some(node);
        }
        let start = stack.len();
        stack.extend(ast.children(node));
        stack[start..].reverse();
    }
    None
}

/// `IsExpandoInitializer` in a JavaScript file: a function or arrow
/// function, a class expression, or an empty object literal of a
/// declaration without a type.
fn is_expando_initializer(ast: &Ast<'_>, declaration: NodeId, initializer: NodeId) -> bool {
    let syntax = ast.node(initializer);
    match syntax.syntax_kind {
        SyntaxKind::FunctionExpression
        | SyntaxKind::ArrowFunction
        | SyntaxKind::ClassExpression => true,
        SyntaxKind::ObjectLiteralExpression => {
            ids(syntax, "properties").is_empty() && id(ast.node(declaration), "type").is_none()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::Construct::{self, Expando, JsdocType, Tag, ThisProperty};
    use crate::source::SourceText;

    /// Every refusal the TypeScript 7 dialect makes in the JavaScript module
    /// `source`, by construct and line, in source order.
    fn refused(source: &str) -> Vec<(Construct, usize)> {
        crate::analysis::on_parsing_thread("javascript refusals", |parser| {
            let ast = parser.parse(source, "/project/a.js").unwrap();
            let text = SourceText::new(source.into());
            let indicators = super::super::ModuleIndicators::of(&ast);
            let facts =
                super::super::collect(&ast, &text, indicators, true, parser, "/project/a.js")
                    .unwrap();
            facts
                .javascript_refusals
                .iter()
                .filter(|refusal| super::Rules::TYPESCRIPT_7.refuses(refusal))
                .map(|refusal| {
                    let line = text.position(refusal.span.start).line;
                    (refusal.construct.clone(), line)
                })
                .collect()
        })
        .unwrap()
    }

    fn tag(name: &str, line: usize) -> (Construct, usize) {
        (Tag(name.to_owned()), line)
    }

    /// `bindDeferredExpandoAssignment` declares a member through an earlier
    /// expando assignment's own expando initializer (`getInitializerSymbol`
    /// of a binary expression), and again for a name an assignment already
    /// declared; not through a static member, for a name a function's other
    /// container declares, or within an object literal, whose container has
    /// no locals.
    #[test]
    fn expandos_follow_the_binders_lookup_and_initializer_rules() {
        assert_eq!(
            refused("export const o = {};\no.a = {};\no.a.b = 1;\n"),
            [(Expando, 2), (Expando, 3)]
        );
        assert_eq!(
            refused("export function f() {}\nf.x = 1;\nf.x = 2;\n"),
            [(Expando, 2), (Expando, 3)]
        );
        assert_eq!(
            refused(
                "export function outer() {}\n{ const inner = () => 1; inner.y = 2; }\nfunction g() { outer.x = 1; }\n"
            ),
            [(Expando, 2)]
        );
        assert_eq!(
            refused(
                "export class C { static s = {}; }\nC.s.t = 1;\nC.s = 2;\nC.prototype.m = 3;\n"
            ),
            []
        );
        assert_eq!(
            refused("export function f() {}\nexport const o = { a: (f.x = 1) };\n"),
            []
        );
    }

    /// `bindThisPropertyAssignment` declares in a class member, a property
    /// initializer, or an object literal method, an arrow inheriting its
    /// container, but not for a private name, and a `this` receiver names
    /// no expando target once the binder has left its container.
    #[test]
    fn this_assignments_follow_the_binders_this_container() {
        assert_eq!(
            refused(
                "export class K {\n  m() {\n    const run = () => {\n      this.v = 1;\n    };\n  }\n}\n"
            ),
            [(ThisProperty, 4)]
        );
        assert_eq!(
            refused("export class Q {\n  f = (this.w = 1);\n}\n"),
            [(ThisProperty, 2)]
        );
        assert_eq!(
            refused("export class P {\n  #p;\n  constructor() {\n    this.#p = 1;\n  }\n}\n"),
            []
        );
        assert_eq!(
            refused(
                "export class R {\n  constructor() {\n    this.a = {};\n    this.a.b = 1;\n  }\n}\n"
            ),
            [(ThisProperty, 3)]
        );
    }

    /// `reparseHosted` gives a tag meaning only at a host it changes, and
    /// only in the node's last comment, where a variable's `@type` is read
    /// as its type, refused only for a type with JavaScript-only meaning;
    /// `reparseUnhosted` declares only a typedef with a type, an import with
    /// a clause, and an overload outside an object literal.
    #[test]
    fn jsdoc_tags_follow_the_reparser() {
        assert_eq!(
            refused("/** @param {string} other */\nexport function p(s) { return s; }\n"),
            []
        );
        assert_eq!(
            refused("/** @param {Object} opts */\nexport function q({ a }) { return a; }\n"),
            [tag("param", 1)]
        );
        assert_eq!(
            refused("/** @param [s] */\nexport function r(s) { return s; }\n"),
            [tag("param", 1)]
        );
        assert_eq!(
            refused("/** @returns */\nexport function t() { return 1; }\n"),
            []
        );
        assert_eq!(
            refused("/** @type {string} */\n/** plain */\nexport const n = 1;\n"),
            []
        );
        assert_eq!(
            refused("/** plain */\n/** @type {?string} */\nexport const n = 1;\n"),
            [(JsdocType, 2)]
        );
        assert_eq!(
            refused("/** plain */\n/** @type {string} */\nexport const n = 1;\n"),
            []
        );
        assert_eq!(
            refused(
                "/** @typedef {string} A */\n/** @typedef B */\n/** @import \"./x.js\" */\nexport {};\n"
            ),
            [tag("typedef", 1)]
        );
        assert_eq!(
            refused(
                "export const lit = {\n  /**\n   * @overload\n   * @param {number} a\n   */\n  m(a) { return a; },\n  /** @readonly */\n  r() { return 1; },\n};\n"
            ),
            []
        );
        assert_eq!(
            refused("export class F {\n  /** @readonly */\n  x = 1;\n}\n"),
            [tag("readonly", 2)]
        );
        assert_eq!(
            refused(
                "class Base {}\n/** @extends {Other<number>} */\nexport class D extends Base {}\n"
            ),
            []
        );
        assert_eq!(
            refused("/** @template T */\nexport class G {}\n"),
            [tag("template", 1)]
        );
        assert_eq!(
            refused(
                "function g() {}\n/** @satisfies {string} */\ng();\n/** @this {Date} */\nexport const v = 1;\n"
            ),
            []
        );
        assert_eq!(
            refused("export const obj = { a: 1 };\n/** @type {number} */\nobj.x = 1;\n"),
            [tag("type", 2)]
        );
    }
}
