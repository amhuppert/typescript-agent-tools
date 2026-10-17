//! Exact own function-body recipes. Source captures inputs; native aggregation,
//! contextual typing, return resolution and reachability belong to Evaluator.

use super::{CapturedSyntax, Expression, SignatureRoot, Source, expression};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::references::Location;
use crate::syntax::{self, FunctionFlags};
use crate::syntax_kind::SyntaxKind;

pub(in crate::project::semantic) struct ReturnBodyRecipe {
    pub(in crate::project::semantic) root: SignatureRoot,
    pub(in crate::project::semantic) flags: FunctionFlags,
    pub(in crate::project::semantic) may_return_never: bool,
    pub(in crate::project::semantic) body: ReturnBodyKind,
    pub(in crate::project::semantic) location: Location,
}

pub(in crate::project::semantic) enum ReturnBodyKind {
    Expression(Expression),
    Block(Vec<ReturnExpressionRecipe>),
}

pub(in crate::project::semantic) struct ReturnExpressionRecipe {
    pub(in crate::project::semantic) expression: Option<Expression>,
    pub(in crate::project::semantic) bare_call_callee: Option<Expression>,
}

impl Source<'_, '_> {
    /// `getReturnTypeFromAnnotation` must not acquire parameter descriptors
    /// merely to ask the containing function's written return question.
    pub(in crate::project::semantic) fn declaration_return_recipe(
        &mut self,
        root: SignatureRoot,
    ) -> Result<super::SignatureReturnRecipe, super::super::model::DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        super::signature_return_recipe(
            inputs,
            root.file,
            captured,
            &captured.parents,
            id,
            root.role,
        )
        .map_err(|message| {
            super::super::unsupported_demand_at_location(
                inputs,
                &super::super::location(inputs, root.file, root.span()),
                crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
                message,
            )
        })
    }

    pub(in crate::project::semantic) fn contextual_function_facts(
        &mut self,
        root: SignatureRoot,
    ) -> Result<(bool, FunctionFlags, Option<super::ExpressionRoot>), AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        let ast = &captured.ast;
        let contextual = syntax::is_function_expression_or_arrow_function(ast.node(id).syntax_kind)
            || syntax::is_object_literal_method(ast, |id| captured.parents[id.index()], id);
        let iife = syntax::immediately_invoked_function(ast, &captured.parents, id).map(|id| {
            super::ExpressionRoot {
                file: root.file,
                site: Site::new(ast, id),
            }
        });
        Ok((contextual, syntax::function_flags(ast, id), iife))
    }

    /// `isAritySmaller` counts only the required leading syntax parameters,
    /// then removes an actual leading this parameter independently.
    pub(in crate::project::semantic) fn contextual_function_parameter_count(
        &mut self,
        root: super::ExpressionRoot,
    ) -> Result<isize, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let ast = &captured.ast;
        let parameters = syntax::ids(ast.node(id), "parameters");
        let count = parameters
            .iter()
            .take_while(|&&parameter| {
                let parameter = ast.node(parameter);
                syntax::id(parameter, "initializer").is_none()
                    && syntax::id(parameter, "questionToken").is_none()
                    && syntax::id(parameter, "dotDotDotToken").is_none()
            })
            .count() as isize;
        Ok(count
            - if parameters
                .first()
                .is_some_and(|&parameter| syntax::is_this_parameter(ast, parameter))
            {
                1
            } else {
                0
            })
    }
    /// `getEffectiveReturnTypeNode`: admitted TypeScript declarations use
    /// their actual return node. JSDoc return acquisition is a separate input.
    pub(in crate::project::semantic) fn signature_return_has_annotation(
        &mut self,
        root: SignatureRoot,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        Ok(syntax::id(captured.ast.node(id), "type").is_some())
    }

    /// `constructorVisibilitiesAreCompatible` reads ModifierFlags directly
    /// from the actual signature declaration, without symbol-level selection.
    pub(in crate::project::semantic) fn signature_declaration_modifiers(
        &mut self,
        root: SignatureRoot,
    ) -> Result<u32, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        Ok(syntax::modifier_flags(&captured.ast, captured.ast.node(id)))
    }

    /// Inputs for `getReturnTypeFromBody` / `checkAndAggregateReturnExpressionTypes`.
    /// Reuse the final signature attachment and native return traversal; never
    /// inspect another function's returns or infer fallthrough from syntax.
    pub(in crate::project::semantic) fn return_body(
        &mut self,
        root: SignatureRoot,
    ) -> Result<ReturnBodyRecipe, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let node = ast.node(id);
        let flags = syntax::function_flags(ast, id);
        let body = syntax::id(node, "body").ok_or_else(|| {
            super::super::internal("inferred return signature has no actual body")
        })?;
        let may_return_never = match node.syntax_kind {
            kind if syntax::is_function_expression_or_arrow_function(kind) => true,
            SyntaxKind::MethodDeclaration => parents[id.index()].is_some_and(|parent| {
                ast.node(parent).syntax_kind == SyntaxKind::ObjectLiteralExpression
            }),
            _ => false,
        };
        let body = if ast.node(body).syntax_kind == SyntaxKind::Block {
            let returns = syntax::return_statements(ast, body)
                .into_iter()
                .map(|statement| {
                    let expr = syntax::id(ast.node(statement), "expression").map(|expr| {
                        let mut expr = syntax::skip_parentheses(ast, expr);
                        if flags.is_async()
                            && ast.node(expr).syntax_kind == SyntaxKind::AwaitExpression
                        {
                            if let Some(operand) = syntax::id(ast.node(expr), "expression") {
                                expr = syntax::skip_parentheses(ast, operand);
                            }
                        }
                        expr
                    });
                    let bare_call_callee = expr
                        .filter(|&expr| ast.node(expr).syntax_kind == SyntaxKind::CallExpression)
                        .and_then(|expr| syntax::id(ast.node(expr), "expression"))
                        .filter(|&callee| ast.node(callee).syntax_kind == SyntaxKind::Identifier)
                        .map(|callee| expression(inputs, root.file, captured, callee));
                    ReturnExpressionRecipe {
                        expression: expr.map(|expr| expression(inputs, root.file, captured, expr)),
                        bare_call_callee,
                    }
                })
                .collect();
            ReturnBodyKind::Block(returns)
        } else {
            ReturnBodyKind::Expression(expression(inputs, root.file, captured, body))
        };
        Ok(ReturnBodyRecipe {
            root,
            flags,
            may_return_never,
            body,
            location: super::super::location(inputs, root.file, Site::new(ast, id).span),
        })
    }
}
