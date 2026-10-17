//! Syntax gates for checkSuperExpression. Class and base-type acquisition
//! belongs to the evaluator, including repeated native cache observations.

use super::super::model::DemandError;
use super::{DeclarationIdentity, ExpressionRoot, Source};
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) enum SuperExpressionContext {
    Invalid,
    ObjectLiteral,
    Class {
        class: DeclarationIdentity,
        has_extends: bool,
        before_super: bool,
        call: bool,
        static_: bool,
        in_constructor_argument: bool,
    },
}

impl Source<'_, '_> {
    /// `checkSuperExpression`'s complete legal-container predicate and
    /// `isInConstructorArgumentInitializer`. Only property use escapes arrows;
    /// super calls must belong directly to a constructor.
    pub(in crate::project::semantic) fn super_expression_context(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<SuperExpressionContext, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = captured.node_at(root.site)?;
        let call = captured.parents[node.index()].is_some_and(|parent| {
            ast.node(parent).syntax_kind == K::CallExpression
                && syntax::id(ast.node(parent), "expression") == Some(node)
        });
        let immediate = syntax::super_container(ast, &captured.parents, node, true);
        let mut container = immediate;
        if !call {
            while let Some(arrow) =
                container.filter(|&id| ast.node(id).syntax_kind == K::ArrowFunction)
            {
                container = syntax::super_container(ast, &captured.parents, arrow, true);
            }
        }
        let Some(container) = container else {
            return Ok(SuperExpressionContext::Invalid);
        };
        let kind = ast.node(container).syntax_kind;
        let parent = captured.parents[container.index()];
        let static_ = syntax::is_static(ast, container);
        let legal = if call {
            kind == K::Constructor
        } else if parent.is_some_and(|parent| {
            matches!(
                ast.node(parent).syntax_kind,
                K::ClassDeclaration | K::ClassExpression | K::ObjectLiteralExpression
            )
        }) {
            if static_ {
                matches!(
                    kind,
                    K::MethodDeclaration
                        | K::MethodSignature
                        | K::GetAccessor
                        | K::SetAccessor
                        | K::PropertyDeclaration
                        | K::ClassStaticBlockDeclaration
                )
            } else {
                matches!(
                    kind,
                    K::MethodDeclaration
                        | K::MethodSignature
                        | K::GetAccessor
                        | K::SetAccessor
                        | K::PropertyDeclaration
                        | K::PropertySignature
                        | K::Constructor
                )
            }
        } else {
            false
        };
        if !legal {
            return Ok(SuperExpressionContext::Invalid);
        }
        let parent =
            parent.ok_or_else(|| super::super::internal("legal super container lacks parent"))?;
        if ast.node(parent).syntax_kind == K::ObjectLiteralExpression {
            return Ok(SuperExpressionContext::ObjectLiteral);
        }
        let mut in_constructor_argument = false;
        if kind == K::Constructor {
            let mut current = Some(node);
            while let Some(id) = current {
                if syntax::is_function_like_kind(ast.node(id).syntax_kind) {
                    break;
                }
                if ast.node(id).syntax_kind == K::Parameter
                    && captured.parents[id.index()] == Some(container)
                {
                    in_constructor_argument = true;
                    break;
                }
                current = captured.parents[id.index()];
            }
        }
        Ok(SuperExpressionContext::Class {
            class: DeclarationIdentity::new(root.file, parent),
            has_extends: !syntax::heritage_elements(ast, parent, K::ExtendsKeyword).is_empty(),
            before_super: !call
                && immediate.is_some_and(|id| ast.node(id).syntax_kind == K::Constructor),
            call,
            static_,
            in_constructor_argument,
        })
    }
}
