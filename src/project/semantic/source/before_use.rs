//! The complete syntax half of isBlockScopedNameDeclaredBeforeUse. Ordered
//! static initialization questions are executed by the shared flow owner.
use super::{DeclarationIdentity, DeclarationReference, ExpressionRoot, Source};
use crate::project::binding::flow::Site;
use crate::project::semantic::model::DemandError;
use crate::syntax_kind::SyntaxKind as K;
use crate::{Ast, NodeId, syntax};

pub(in crate::project::semantic) struct StaticInitialization {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) name: ExpressionRoot,
    pub(in crate::project::semantic) blocks: Vec<ExpressionRoot>,
}
pub(in crate::project::semantic) struct BeforeUsePlan {
    pub(in crate::project::semantic) questions: Vec<StaticInitialization>,
    pub(in crate::project::semantic) if_initialized: bool,
    pub(in crate::project::semantic) otherwise: bool,
}
impl BeforeUsePlan {
    fn constant(value: bool) -> Self {
        Self {
            questions: Vec::new(),
            if_initialized: value,
            otherwise: value,
        }
    }
    fn map(mut self, f: impl Fn(bool) -> bool) -> Self {
        self.if_initialized = f(self.if_initialized);
        self.otherwise = f(self.otherwise);
        self
    }
}
pub(in crate::project::semantic) struct PropertyBeforeUse {
    pub(in crate::project::semantic) property: Option<ExpressionRoot>,
    pub(in crate::project::semantic) class: Option<ExpressionRoot>,
    pub(in crate::project::semantic) static_method: bool,
}
impl Source<'_, '_> {
    pub(in crate::project::semantic) fn before_use_plan(
        &mut self,
        declaration: DeclarationReference,
        usage: ExpressionRoot,
    ) -> Result<BeforeUsePlan, DemandError> {
        let declaration = self.resolve_declaration_reference(declaration)?;
        if declaration.file != usage.file {
            return Ok(BeforeUsePlan::constant(true));
        }
        let options = self.inputs.semantic_options();
        let legacy = options.experimental_decorators;
        let standard = options.emit_standard_class_fields;
        let captured = self.syntax(usage.file)?;
        let usage = captured.node_at(usage.site)?;
        let walker = BeforeUse {
            ast: &captured.ast,
            parents: &captured.parents,
            file: declaration.file,
            legacy,
            standard,
        };
        Ok(walker.before(declaration.node, usage))
    }
    /// `checkPropertyNotUsedBeforeDeclaration` syntax gates, before any native
    /// before-use, modifier or ancestor-base semantic question.
    pub(in crate::project::semantic) fn property_before_use(
        &mut self,
        declaration: DeclarationIdentity,
        access: ExpressionRoot,
    ) -> Result<PropertyBeforeUse, DemandError> {
        let (optional, static_method, class, ambient) = {
            let captured = self.syntax(declaration.file)?;
            let ast = &captured.ast;
            let node = ast.node(declaration.node);
            (
                node.syntax_kind == K::PropertyDeclaration
                    && !syntax::has_modifier(ast, node, K::AccessorKeyword)
                    && syntax::id(node, "questionToken").is_some(),
                node.syntax_kind == K::MethodDeclaration
                    && syntax::combined_modifier_flags(ast, &captured.parents, declaration.node)
                        & syntax::modifiers::STATIC
                        != 0,
                node.syntax_kind == K::ClassDeclaration,
                node.flags & crate::compiler::node::AMBIENT != 0,
            )
        };
        let captured = self.syntax(access.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let id = captured.node_at(access.site)?;
        let node = ast.node(id);
        if syntax::flag(ast.root(), "isDeclarationFile") {
            return Ok(PropertyBeforeUse {
                property: None,
                class: None,
                static_method,
            });
        }
        let right = syntax::id(
            node,
            if node.syntax_kind == K::QualifiedName {
                "right"
            } else {
                "name"
            },
        )
        .or_else(|| syntax::id(node, "argumentExpression"))
        .ok_or_else(|| super::super::internal("property before-use access lacks right node"))?;
        let usage = ExpressionRoot {
            file: access.file,
            site: Site::new(ast, right),
        };
        let nested = matches!(
            node.syntax_kind,
            K::PropertyAccessExpression | K::ElementAccessExpression
        ) && syntax::id(node, "expression").is_some_and(|receiver| {
            matches!(
                ast.node(receiver).syntax_kind,
                K::PropertyAccessExpression | K::ElementAccessExpression
            )
        });
        Ok(PropertyBeforeUse {
            property: (syntax::in_property_initializer_or_static_block(ast, parents, id, false)
                && !optional
                && !nested)
                .then_some(usage),
            class: (class
                && !ambient
                && !parents[id.index()]
                    .is_some_and(|parent| ast.node(parent).syntax_kind == K::TypeReference))
            .then_some(usage),
            static_method,
        })
    }
}
struct BeforeUse<'a, 'src> {
    ast: &'a Ast<'src>,
    parents: &'a [Option<NodeId>],
    file: crate::project::sources::FileId,
    legacy: bool,
    standard: bool,
}
impl BeforeUse<'_, '_> {
    fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.parents[id.index()]
    }
    fn kind(&self, id: NodeId) -> K {
        self.ast.node(id).syntax_kind
    }
    fn ancestor(
        &self,
        start: Option<NodeId>,
        predicate: impl Fn(NodeId) -> bool,
    ) -> Option<NodeId> {
        let mut node = start;
        while let Some(id) = node {
            if predicate(id) {
                return Some(id);
            }
            node = self.parent(id);
        }
        None
    }
    /// TypeScript `GetContainingClass` starts with the parent.
    fn class(&self, node: NodeId) -> Option<NodeId> {
        self.ancestor(self.parent(node), |id| {
            matches!(self.kind(id), K::ClassDeclaration | K::ClassExpression)
        })
    }
    fn parameter_property(&self, id: NodeId) -> bool {
        self.parent(id).is_some_and(|parent| {
            syntax::is_parameter_property_declaration(
                self.ast,
                self.ast.node(id),
                self.ast.node(parent),
            )
        })
    }
    /// TypeScript `GetEnclosingBlockScopeContainer`.
    fn block_container(&self, id: NodeId) -> Option<NodeId> {
        self.ancestor(self.parent(id), |id| {
            syntax::is_block_scope(self.ast, id, self.parent(id))
        })
    }
    fn before(&self, declaration: NodeId, usage: NodeId) -> BeforeUsePlan {
        let ast = self.ast;
        let d = ast.node(declaration);
        let u = ast.node(usage);
        let container = self.block_container(declaration);
        if u.flags & crate::compiler::node::JSDOC != 0
            || syntax::is_in_type_query(ast, |id| self.parent(id), usage)
            || syntax::is_in_ambient_or_type_node(ast, |id| self.parent(id), usage)
        {
            return BeforeUsePlan::constant(true);
        }
        let special = d.syntax_kind == K::PropertyDeclaration
            && self
                .parent(usage)
                .is_some_and(|parent| syntax::is_this_property(ast, parent))
            && syntax::id(d, "initializer").is_none()
            && syntax::id(d, "exclamationToken").is_none();
        if d.span.start <= u.span.start && !special {
            let value = match d.syntax_kind {
                K::BindingElement => {
                    if let Some(error) =
                        self.ancestor(Some(usage), |id| self.kind(id) == K::BindingElement)
                    {
                        self.ancestor(Some(error), |id| self.kind(id) == K::BindingElement)
                            != self.ancestor(Some(declaration), |id| {
                                self.kind(id) == K::BindingElement
                            })
                            || d.span.start < ast.node(error).span.start
                    } else {
                        return self.before(
                            self.ancestor(Some(declaration), |id| {
                                self.kind(id) == K::VariableDeclaration
                            })
                            .expect("binding element has variable declaration"),
                            usage,
                        );
                    }
                }
                K::VariableDeclaration => !self.immediate_variable(declaration, usage, container),
                K::ClassDeclaration | K::ClassExpression => {
                    let mut node = Some(usage);
                    while let Some(id) = node {
                        if id == declaration {
                            break;
                        }
                        let parent = self.parent(id);
                        let own_computed = self.kind(id) == K::ComputedPropertyName
                            && parent.and_then(|id| self.parent(id)) == Some(declaration);
                        let decorated = !self.legacy
                            && self.kind(id) == K::Decorator
                            && parent.is_some_and(|parent| {
                                parent == declaration
                                    || matches!(
                                        self.kind(parent),
                                        K::MethodDeclaration
                                            | K::GetAccessor
                                            | K::SetAccessor
                                            | K::PropertyDeclaration
                                    ) && self.parent(parent) == Some(declaration)
                                    || self.kind(parent) == K::Parameter
                                        && self.parent(parent).and_then(|id| self.parent(id))
                                            == Some(declaration)
                            });
                        if own_computed || decorated {
                            break;
                        }
                        node = parent;
                    }
                    match node {
                        None => true,
                        Some(id) if id == declaration => true,
                        Some(id) if !self.legacy && self.kind(id) == K::Decorator => {
                            let mut n = Some(usage);
                            while let Some(current) = n {
                                if current == id
                                    || syntax::is_function_like_kind(self.kind(current))
                                        && syntax::immediately_invoked_function(
                                            ast,
                                            self.parents,
                                            current,
                                        )
                                        .is_none()
                                {
                                    break;
                                }
                                n = self.parent(current);
                            }
                            n.is_some_and(|n| n != id)
                        }
                        _ => false,
                    }
                }
                K::PropertyDeclaration => !self.immediate_property(declaration, usage, false),
                _ if self.parameter_property(declaration) => {
                    if self.standard && self.class(declaration) == self.class(usage) {
                        return self.used(usage, declaration, container).map(|value| !value);
                    }
                    true
                }
                _ => true,
            };
            return BeforeUsePlan::constant(value);
        }
        if self.parent(usage).is_some_and(|parent| {
            self.kind(parent) == K::ExportSpecifier
                || self.kind(parent) == K::ExportAssignment
                    && syntax::flag(ast.node(parent), "isExportEquals")
        }) || self.kind(usage) == K::ExportAssignment && syntax::flag(u, "isExportEquals")
        {
            return BeforeUsePlan::constant(true);
        }
        let admitted = !(self.standard
            && self.class(declaration).is_some()
            && (d.syntax_kind == K::PropertyDeclaration || self.parameter_property(declaration)))
            || !self.immediate_property(declaration, usage, true);
        self.used(usage, declaration, container)
            .map(|value| value && admitted)
    }
    fn used(&self, usage: NodeId, declaration: NodeId, container: Option<NodeId>) -> BeforeUsePlan {
        let ast = self.ast;
        let mut questions = Vec::new();
        let mut node = Some(usage);
        let mut result = false;
        while let Some(current) = node {
            if Some(current) == container {
                break;
            }
            if syntax::is_function_like_kind(self.kind(current))
                && syntax::immediately_invoked_function(ast, self.parents, current).is_none()
            {
                result = true;
                break;
            }
            if self.kind(current) == K::ClassStaticBlockDeclaration
                && ast.node(declaration).span.start < ast.node(usage).span.start
            {
                result = true;
                break;
            }
            if let Some(parent) = self.parent(current)
                && self.kind(parent) == K::PropertyDeclaration
                && syntax::id(ast.node(parent), "initializer") == Some(current)
            {
                if syntax::is_static(ast, parent) {
                    if self.kind(declaration) == K::MethodDeclaration {
                        result = true;
                        break;
                    }
                    if self.kind(declaration) == K::PropertyDeclaration
                        && self.class(usage) == self.class(declaration)
                        && let Some(name) = syntax::id(ast.node(declaration), "name")
                        && matches!(self.kind(name), K::Identifier | K::PrivateIdentifier)
                    {
                        let class = self.parent(declaration).expect("property parent");
                        let start = ast.node(class).span.start;
                        let end = ast.node(current).span.start;
                        let blocks = syntax::ids(ast.node(class), "members")
                            .iter()
                            .copied()
                            .filter(|&id| {
                                self.kind(id) == K::ClassStaticBlockDeclaration
                                    && ast.node(id).span.start >= start
                                    && ast.node(id).span.start <= end
                            })
                            .map(|id| ExpressionRoot {
                                file: self.file,
                                site: Site::new(ast, id),
                            })
                            .collect();
                        questions.push(StaticInitialization {
                            declaration: DeclarationIdentity::new(self.file, declaration),
                            name: ExpressionRoot {
                                file: self.file,
                                site: Site::new(ast, name),
                            },
                            blocks,
                        });
                    }
                } else if self.kind(declaration) != K::PropertyDeclaration
                    || syntax::is_static(ast, declaration)
                    || self.class(usage) != self.class(declaration)
                {
                    result = true;
                    break;
                }
            }
            if let Some(parent) = self.parent(current)
                && self.kind(parent) == K::Decorator
                && syntax::id(ast.node(parent), "expression") == Some(current)
                && let Some(decorated) = self.parent(parent)
            {
                let root = match self.kind(decorated) {
                    K::Parameter => self.parent(decorated).and_then(|id| self.parent(id)),
                    K::MethodDeclaration => self.parent(decorated),
                    _ => None,
                };
                if let Some(root) = root {
                    let other = self.used(root, declaration, container);
                    questions.extend(other.questions);
                    result = other.otherwise;
                    break;
                }
            }
            node = self.parent(current);
        }
        BeforeUsePlan {
            questions,
            if_initialized: true,
            otherwise: result,
        }
    }
    /// TypeScript `isSameScopeDescendentOf`.
    fn same_scope_descendant(
        &self,
        initial: NodeId,
        parent: Option<NodeId>,
        stop: Option<NodeId>,
    ) -> bool {
        let Some(parent) = parent else { return false };
        let mut node = Some(initial);
        while let Some(id) = node {
            if id == parent {
                return true;
            }
            let flags = syntax::function_flags(self.ast, id);
            if Some(id) == stop
                || syntax::is_function_like_kind(self.kind(id))
                    && (syntax::immediately_invoked_function(self.ast, self.parents, id).is_none()
                        || flags.is_async()
                        || flags.is_generator())
            {
                return false;
            }
            node = self.parent(id);
        }
        false
    }
    /// TypeScript `isImmediatelyUsedInInitializerOfBlockScopedVariable`.
    fn immediate_variable(
        &self,
        declaration: NodeId,
        usage: NodeId,
        container: Option<NodeId>,
    ) -> bool {
        let Some(grandparent) = self.parent(declaration).and_then(|id| self.parent(id)) else {
            return false;
        };
        if matches!(
            self.kind(grandparent),
            K::VariableStatement | K::ForStatement | K::ForOfStatement
        ) && self.same_scope_descendant(usage, Some(declaration), container)
        {
            return true;
        }
        matches!(
            self.kind(grandparent),
            K::ForInStatement | K::ForOfStatement
        ) && self.same_scope_descendant(
            usage,
            syntax::id(self.ast.node(grandparent), "expression"),
            container,
        )
    }
    /// TypeScript `isPropertyImmediatelyReferencedWithinDeclaration`.
    fn immediate_property(&self, declaration: NodeId, usage: NodeId, stop_any: bool) -> bool {
        if self.ast.node(usage).span.end > self.ast.node(declaration).span.end {
            return false;
        }
        let mut node = Some(usage);
        while let Some(id) = node {
            if id == declaration {
                break;
            }
            match self.kind(id) {
                K::ArrowFunction => return false,
                K::PropertyDeclaration => {
                    return stop_any
                        && (self.kind(declaration) == K::PropertyDeclaration
                            && self.parent(id) == self.parent(declaration)
                            || self.parameter_property(declaration)
                                && self.parent(id)
                                    == self.parent(declaration).and_then(|id| self.parent(id)));
                }
                K::Block
                    if self.parent(id).is_some_and(|parent| {
                        matches!(
                            self.kind(parent),
                            K::MethodDeclaration | K::GetAccessor | K::SetAccessor
                        )
                    }) =>
                {
                    return false;
                }
                _ => {}
            }
            node = self.parent(id);
        }
        true
    }
}
