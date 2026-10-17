//! Actual function and parameter declaration facts for contextual checking.

use super::super::model::DemandError;
use super::{DeclarationIdentity, ExpressionRoot, SignatureParameterRecipe, SignatureRoot, Source};
use crate::project::binding::flow::Site;
use crate::{Span, syntax, syntax_kind::SyntaxKind};

pub(in crate::project::semantic) struct FunctionExpressionFacts {
    pub(in crate::project::semantic) signature: SignatureRoot,
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) annotation: bool,
    pub(in crate::project::semantic) type_parameters: bool,
    pub(in crate::project::semantic) parameter_count: usize,
    pub(in crate::project::semantic) parameters: Vec<DeclarationIdentity>,
    pub(in crate::project::semantic) type_parameter_nodes: Vec<ExpressionRoot>,
    pub(in crate::project::semantic) annotation_node: Option<ExpressionRoot>,
    pub(in crate::project::semantic) flags: syntax::FunctionFlags,
}

pub(in crate::project::semantic) struct ParameterContext {
    pub(in crate::project::semantic) function: ExpressionRoot,
    pub(in crate::project::semantic) index: usize,
    pub(in crate::project::semantic) this_parameter: bool,
    pub(in crate::project::semantic) iife: Option<super::CallRoot>,
    pub(in crate::project::semantic) last: bool,
    pub(in crate::project::semantic) has_this: bool,
}

pub(in crate::project::semantic) enum ContextualThisSource {
    None,
    Object(ExpressionRoot),
    Assignment(ExpressionRoot),
}

impl Source<'_, '_> {
    /// `getContainingObjectLiteral` and the assignment arm of
    /// `getContextualThisParameterType` preserve actual parent roles.
    pub(in crate::project::semantic) fn contextual_this_source(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ContextualThisSource, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let mut parent = captured.parents[id.index()];
        if ast.node(id).syntax_kind == SyntaxKind::FunctionExpression {
            if let Some(property) =
                parent.filter(|&node| ast.node(node).syntax_kind == SyntaxKind::PropertyAssignment)
            {
                parent = captured.parents[property.index()];
            } else {
                parent = None;
            }
        }
        if let Some(literal) =
            parent.filter(|&node| ast.node(node).syntax_kind == SyntaxKind::ObjectLiteralExpression)
        {
            return Ok(ContextualThisSource::Object(ExpressionRoot {
                file: root.file,
                site: Site::new(ast, literal),
            }));
        }
        let mut parent = captured.parents[id.index()];
        while let Some(node) =
            parent.filter(|&node| ast.node(node).syntax_kind == SyntaxKind::ParenthesizedExpression)
        {
            parent = captured.parents[node.index()];
        }
        if let Some(parent) =
            parent.filter(|&node| syntax::is_assignment_expression(ast, node, false))
        {
            let target = syntax::id(ast.node(parent), "left")
                .ok_or_else(|| super::super::internal("assignment has no left operand"))?;
            if matches!(
                ast.node(target).syntax_kind,
                SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
            ) {
                let receiver = syntax::id(ast.node(target), "expression")
                    .ok_or_else(|| super::super::internal("access has no receiver"))?;
                return Ok(ContextualThisSource::Assignment(ExpressionRoot {
                    file: root.file,
                    site: Site::new(ast, receiver),
                }));
            }
        }
        Ok(ContextualThisSource::None)
    }
    pub(in crate::project::semantic) fn signature_from_root(
        &mut self,
        root: SignatureRoot,
    ) -> Result<super::SignatureRecipe, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        super::signature_recipe(inputs, root.file, captured, &captured.parents, id).map_err(
            |message| {
                super::super::unsupported_demand_at_location(
                    inputs,
                    &super::super::location(inputs, root.file, root.span()),
                    crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
                    message,
                )
            },
        )
    }
    /// `checkFunctionExpressionOrObjectLiteralMethod` asks syntax without
    /// consuming parameter/return recipes before its contextual signature.
    pub(in crate::project::semantic) fn function_expression_facts(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<FunctionExpressionFacts, DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let node = captured.ast.node(id);
        Ok(FunctionExpressionFacts {
            signature: super::signature_root(&captured.ast, root.file, id)
                .map_err(super::super::internal)?,
            declaration: DeclarationIdentity::new(root.file, id),
            annotation: syntax::id(node, "type").is_some(),
            type_parameters: node.field("typeParameters").is_some(),
            parameter_count: syntax::ids(node, "parameters").len(),
            parameters: syntax::ids(node, "parameters")
                .iter()
                .map(|&node| DeclarationIdentity::new(root.file, node))
                .collect(),
            type_parameter_nodes: syntax::ids(node, "typeParameters")
                .iter()
                .map(|&node| ExpressionRoot {
                    file: root.file,
                    site: Site::new(&captured.ast, node),
                })
                .collect(),
            annotation_node: syntax::id(node, "type").map(|node| ExpressionRoot {
                file: root.file,
                site: Site::new(&captured.ast, node),
            }),
            flags: syntax::function_flags(&captured.ast, id),
        })
    }

    pub(in crate::project::semantic) fn parameter_recipe(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<SignatureParameterRecipe, DemandError> {
        let inputs = self.inputs;
        let location = self.declaration_location(declaration)?;
        let captured = self.syntax(declaration.file)?;
        super::signature_parameter_recipe(
            inputs,
            declaration.file,
            captured,
            &captured.parents,
            declaration.node,
        )
        .map_err(|message| {
            super::super::unsupported_demand_at_location(
                inputs,
                &location,
                crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
                message,
            )
        })
    }

    /// `getContextuallyTypedParameterType` asks the actual owning function,
    /// positional index and declaration Name role independently of its type.
    pub(in crate::project::semantic) fn parameter_context(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<ParameterContext, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let id = declaration.node;
        let parent = captured.parents[id.index()]
            .ok_or_else(|| super::super::internal("parameter has no actual function"))?;
        let index = syntax::ids(ast.node(parent), "parameters")
            .iter()
            .position(|&parameter| parameter == id)
            .ok_or_else(|| super::super::internal("parameter has no actual signature position"))?;
        Ok(ParameterContext {
            function: ExpressionRoot {
                file: declaration.file,
                site: Site::new(ast, parent),
            },
            index,
            this_parameter: syntax::is_this_parameter(ast, id),
            iife: syntax::immediately_invoked_function(ast, &captured.parents, parent).map(
                |call| {
                    super::CallRoot::new(
                        declaration.file,
                        ast.token_span(call),
                        ast.node(call).syntax_kind,
                    )
                },
            ),
            last: syntax::ids(ast.node(parent), "parameters").last() == Some(&id),
            has_this: syntax::ids(ast.node(parent), "parameters")
                .first()
                .is_some_and(|&parameter| syntax::is_this_parameter(ast, parameter)),
        })
    }

    /// `checkTypeParameter` consumes constraint and default syntax before the
    /// canonical declared-type question. The actual node owns both roots.
    pub(in crate::project::semantic) fn type_parameter_check_children(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<(Option<ExpressionRoot>, Option<ExpressionRoot>, Span), DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let ast = &captured.ast;
        let child = |field| {
            syntax::id(ast.node(id), field).map(|node| ExpressionRoot {
                file: root.file,
                site: Site::new(ast, node),
            })
        };
        let constraint = child("constraint");
        let default = child("default");
        let anchor = captured.declaration_anchor(id);
        Ok((constraint, default, anchor))
    }

    /// `checkTypeParametersNotReferenced` visits every child, including
    /// nested type scopes, and asks getTypeFromTypeReference at each reference.
    pub(in crate::project::semantic) fn type_parameter_default_references(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Vec<super::TypeExpression>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let ast = &captured.ast;
        let mut work = vec![id];
        let mut references = Vec::new();
        while let Some(id) = work.pop() {
            if ast.node(id).syntax_kind == SyntaxKind::TypeReference {
                references.push(super::type_expression(
                    inputs,
                    root.file,
                    captured,
                    &captured.parents,
                    id,
                ));
            }
            let children: Vec<_> = ast.children(id).collect();
            work.extend(children.into_iter().rev());
        }
        Ok(references)
    }

    /// Syntax traversal of `isParameterOfContextSensitiveSignature`: walk
    /// BindingElements to the root parameter before the contextual-function gate.
    pub(in crate::project::semantic) fn parameter_declaration_context_sensitive_root(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let mut id = declaration.node;
        let ast = &captured.ast;
        while ast.node(id).syntax_kind == SyntaxKind::BindingElement {
            let pattern = captured.parents[id.index()]
                .ok_or_else(|| super::super::internal("binding element lacks pattern"))?;
            id = captured.parents[pattern.index()]
                .ok_or_else(|| super::super::internal("binding pattern lacks declaration"))?;
        }
        if ast.node(id).syntax_kind != SyntaxKind::Parameter {
            return Ok(None);
        }
        let parent = captured.parents[id.index()]
            .ok_or_else(|| super::super::internal("parameter lacks function"))?;
        let kind = ast.node(parent).syntax_kind;
        let contextual = syntax::is_function_expression_or_arrow_function(kind)
            || syntax::is_object_literal_method(ast, |node| captured.parents[node.index()], parent);
        Ok(contextual.then(|| ExpressionRoot {
            file: declaration.file,
            site: Site::new(ast, parent),
        }))
    }
}
