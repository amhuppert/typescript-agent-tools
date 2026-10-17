//! Native predicate-body syntax eligibility, independent of return inference.

use super::{DeclarationIdentity, SignatureRoot, Source};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;

pub(in crate::project::semantic) struct PredicateBody {
    pub(in crate::project::semantic) expression: Site,
    pub(in crate::project::semantic) parent_return: Option<Site>,
    pub(in crate::project::semantic) block: bool,
    pub(in crate::project::semantic) parameters: Vec<DeclarationIdentity>,
}

impl Source<'_, '_> {
    /// Syntax portion of `getTypePredicateFromBody`: reject non-normal
    /// functions and non-single returns before checking any expression.
    /// Block fallthrough remains the evaluator's native reachability question.
    pub(in crate::project::semantic) fn predicate_body(
        &mut self,
        root: SignatureRoot,
    ) -> Result<Option<PredicateBody>, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site())?;
        let ast = &captured.ast;
        let node = ast.node(id);
        if matches!(
            node.syntax_kind,
            SyntaxKind::Constructor | SyntaxKind::GetAccessor | SyntaxKind::SetAccessor
        ) {
            return Ok(None);
        }
        let flags = syntax::function_flags(ast, id);
        if flags.is_invalid() || flags.is_async() || flags.is_generator() {
            return Ok(None);
        }
        let body = syntax::id(node, "body")
            .ok_or_else(|| super::super::internal("normal predicate function lacks a body"))?;
        let block = ast.node(body).syntax_kind == SyntaxKind::Block;
        let expression = if block {
            let mut single = None;
            for statement in syntax::return_statements(ast, body) {
                if single.is_some() {
                    return Ok(None);
                }
                let Some(expression) = syntax::id(ast.node(statement), "expression") else {
                    return Ok(None);
                };
                single = Some(expression);
            }
            let Some(expression) = single else {
                return Ok(None);
            };
            expression
        } else {
            body
        };
        let expression = syntax::skip_parentheses(ast, expression);
        let parent_return = captured.parents[expression.index()]
            .filter(|&parent| ast.node(parent).syntax_kind == SyntaxKind::ReturnStatement)
            .map(|parent| Site::new(ast, parent));
        Ok(Some(PredicateBody {
            expression: Site::new(ast, expression),
            parent_return,
            block,
            parameters: syntax::ids(node, "parameters")
                .iter()
                .map(|&parameter| DeclarationIdentity::new(root.file, parameter))
                .collect(),
        }))
    }

    /// `checkIfExpressionRefinesAnyParameter` asks identifier eligibility
    /// after the raw parameter type and before assignment/rest checks.
    pub(in crate::project::semantic) fn predicate_parameter_name(
        &mut self,
        parameter: DeclarationIdentity,
    ) -> Result<Option<Site>, AnalysisError> {
        let captured = self.syntax(parameter.file)?;
        let parameter = parameter.node;
        let name = syntax::id(captured.ast.node(parameter), "name")
            .ok_or_else(|| super::super::internal("predicate parameter lacks an actual name"))?;
        Ok(
            (captured.ast.node(name).syntax_kind == SyntaxKind::Identifier)
                .then(|| Site::new(&captured.ast, name)),
        )
    }

    /// Native `isRestParameter` for actual TypeScript parameter declarations.
    pub(in crate::project::semantic) fn predicate_parameter_is_rest(
        &mut self,
        parameter: DeclarationIdentity,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(parameter.file)?;
        let parameter = parameter.node;
        Ok(syntax::id(captured.ast.node(parameter), "dotDotDotToken").is_some())
    }
}
