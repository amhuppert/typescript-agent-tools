//! Syntax questions for native this-expression checking. Signature, class and
//! flow acquisition stay with the evaluator; explicit-this/type-node questions
//! retain their separate operations.

use super::super::model::DemandError;
use super::{DeclarationIdentity, ExpressionRoot, Source};
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) struct ThisExpressionContext {
    pub(in crate::project::semantic) container: DeclarationIdentity,
    pub(in crate::project::semantic) before_super_class: Option<DeclarationIdentity>,
}

pub(in crate::project::semantic) struct ThisTypeContext {
    pub(in crate::project::semantic) container: DeclarationIdentity,
    pub(in crate::project::semantic) function: bool,
    pub(in crate::project::semantic) class: Option<(DeclarationIdentity, bool)>,
    pub(in crate::project::semantic) source_file: bool,
    pub(in crate::project::semantic) external_module: bool,
}

impl Source<'_, '_> {
    /// Parent step of `getThisTypeOfObjectLiteralFromContextualType`:
    /// only an immediate PropertyAssignment permits an enclosing literal.
    pub(in crate::project::semantic) fn outer_this_object_literal(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        let Some(parent) = captured.parents[node.index()]
            .filter(|&parent| captured.ast.node(parent).syntax_kind == K::PropertyAssignment)
        else {
            return Ok(None);
        };
        let literal = captured.parents[parent.index()].ok_or_else(|| {
            super::super::internal("property assignment lacks containing literal")
        })?;
        Ok(Some(ExpressionRoot {
            file: root.file,
            site: crate::project::binding::flow::Site::new(&captured.ast, literal),
        }))
    }

    /// `checkThisExpression`'s constructor gate and arrow/computed-name loop.
    /// The resulting flags only affect diagnostic wording; the final container
    /// and initial derived-constructor read affect semantic acquisitions.
    pub(in crate::project::semantic) fn this_expression_context(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ThisExpressionContext, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = captured.node_at(root.site)?;
        let container_of = |id, arrows, computed| {
            syntax::this_container(ast, &captured.parents, id, arrows, computed).ok_or_else(|| {
                super::super::internal("this expression lacks its containing syntax")
            })
        };
        let mut container = container_of(node, true, true)?;
        let before_super_class = if ast.node(container).syntax_kind == K::Constructor {
            let class = captured.parents[container.index()]
                .ok_or_else(|| super::super::internal("constructor lacks parent class"))?;
            (!syntax::heritage_elements(ast, class, K::ExtendsKeyword).is_empty())
                .then_some(DeclarationIdentity::new(root.file, class))
        } else {
            None
        };
        let mut captured_by_arrow = false;
        let mut computed_name = false;
        loop {
            if ast.node(container).syntax_kind == K::ArrowFunction {
                container = container_of(container, false, !computed_name)?;
                captured_by_arrow = true;
            }
            if ast.node(container).syntax_kind == K::ComputedPropertyName {
                container = container_of(container, !captured_by_arrow, false)?;
                computed_name = true;
                continue;
            }
            break;
        }
        Ok(ThisExpressionContext {
            container: DeclarationIdentity::new(root.file, container),
            before_super_class,
        })
    }

    /// `tryGetThisTypeAtEx` and `isInParameterInitializerBeforeContainingFunction`.
    /// A parameter initializer uses class-this unless its containing function
    /// has an explicit this parameter. Nested binding initializers count too.
    pub(in crate::project::semantic) fn this_type_context(
        &mut self,
        root: ExpressionRoot,
        container: Option<DeclarationIdentity>,
    ) -> Result<ThisTypeContext, DemandError> {
        let external_module = self.inputs.bindings.reference_file_is_external(root.file);
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = captured.node_at(root.site)?;
        let container = match container {
            Some(container) if container.file == root.file => container.node,
            Some(_) => {
                return Err(super::super::internal(
                    "this container belongs to a different source file",
                )
                .into());
            }
            None => syntax::this_container(ast, &captured.parents, node, false, false)
                .ok_or_else(|| super::super::internal("this lookup lacks its containing syntax"))?,
        };
        let mut parameter_initializer = false;
        let mut binding_initializer = false;
        let mut current = node;
        while let Some(parent) = captured.parents[current.index()] {
            let parent_node = ast.node(parent);
            if syntax::is_function_like_kind(parent_node.syntax_kind) {
                break;
            }
            if parent_node.syntax_kind == K::Parameter
                && (binding_initializer || syntax::id(parent_node, "initializer") == Some(current))
            {
                parameter_initializer = true;
                break;
            }
            if parent_node.syntax_kind == K::BindingElement
                && syntax::id(parent_node, "initializer") == Some(current)
            {
                binding_initializer = true;
            }
            current = parent;
        }
        let container_node = ast.node(container);
        let has_this = syntax::ids(container_node, "parameters")
            .first()
            .and_then(|&parameter| syntax::id(ast.node(parameter), "name"))
            .is_some_and(|name| syntax::is_this_identifier(ast, name));
        let class = captured.parents[container.index()]
            .filter(|&parent| {
                matches!(
                    ast.node(parent).syntax_kind,
                    K::ClassDeclaration | K::ClassExpression
                )
            })
            .map(|class| {
                (
                    DeclarationIdentity::new(root.file, class),
                    syntax::is_static(ast, container),
                )
            });
        Ok(ThisTypeContext {
            container: DeclarationIdentity::new(root.file, container),
            function: syntax::is_function_like_kind(container_node.syntax_kind)
                && (!parameter_initializer || has_this),
            class,
            source_file: container_node.syntax_kind == K::SourceFile,
            external_module,
        })
    }
}
