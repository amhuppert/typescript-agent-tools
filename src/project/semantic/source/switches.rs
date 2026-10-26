//! Actual switch syntax supplies distinct type and typeof-witness questions.

use super::super::model::LiteralValue;
use super::{Expression, Source, expression};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;

pub(in crate::project::semantic) struct Clause {
    pub(in crate::project::semantic) expression: Option<Site>,
    pub(in crate::project::semantic) witness: Option<LiteralValue>,
}

pub(in crate::project::semantic) struct SwitchSyntax {
    pub(in crate::project::semantic) expression: Site,
    pub(in crate::project::semantic) clauses: Vec<Clause>,
}

impl Source<'_, '_> {
    /// Native string literal `.Text()` for exact property-name comparison.
    /// Its byte spelling comes from the shared scanner decoder, independently
    /// of the witness operation used by typeof switch branches.
    pub(in crate::project::semantic) fn string_literal_key(
        &mut self,
        file: FileId,
        site: Site,
    ) -> Result<crate::project::binding::NativePropertyKey, AnalysisError> {
        let captured = self.syntax(file)?;
        let id = captured.node_at(site)?;
        let bytes = captured
            .ast
            .cooked_literal(captured.ast.node(id))
            .ok_or_else(|| super::super::internal("string literal has no native cooked text"))?;
        Ok(crate::project::binding::NativePropertyKey::fixed(
            captured.literal_bytes(bytes),
        ))
    }

    /// Native literal `.Text()` for typeof witnesses. Preserve cooked bytes;
    /// unknown strings, including lone surrogates, take the host-object branch.
    pub(in crate::project::semantic) fn string_literal_value(
        &mut self,
        file: FileId,
        site: Site,
    ) -> Result<LiteralValue, AnalysisError> {
        let captured = self.syntax(file)?;
        let id = captured.node_at(site)?;
        literal_value(captured, captured.ast.node(id))
    }
    /// Reproject the actual expression role. Normal and type-only checking
    /// caches remain evaluator questions, separate from source attachment.
    pub(in crate::project::semantic) fn expression_at(
        &mut self,
        file: FileId,
        site: Site,
    ) -> Result<Expression, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(file)?;
        let id = captured.node_at(site)?;
        Ok(expression(inputs, file, captured, id))
    }

    /// `getSwitchClauseTypeOfWitnesses` / `getTypeOfSwitchClause` acquire the
    /// same actual ordered clauses but consume different facts. Exact cooked
    /// byte identity survives the syntax-only question without conversion.
    pub(in crate::project::semantic) fn switch_syntax(
        &mut self,
        file: FileId,
        statement: Site,
    ) -> Result<SwitchSyntax, AnalysisError> {
        let captured = self.syntax(file)?;
        let id = captured.node_at(statement)?;
        let ast = &captured.ast;
        let node = ast.node(id);
        if node.syntax_kind != SyntaxKind::SwitchStatement {
            return Err(super::super::internal(
                "switch syntax has a different final role",
            ));
        }
        let expression = syntax::id(node, "expression")
            .ok_or_else(|| super::super::internal("switch has no expression"))?;
        let block = syntax::id(node, "caseBlock")
            .ok_or_else(|| super::super::internal("switch has no case block"))?;
        let mut clauses = Vec::new();
        for &clause in syntax::ids(ast.node(block), "clauses") {
            let clause = ast.node(clause);
            let expression = syntax::id(clause, "expression");
            let witness = if let Some(expression) = expression
                && matches!(
                    ast.node(expression).syntax_kind,
                    SyntaxKind::StringLiteral | SyntaxKind::NoSubstitutionTemplateLiteral
                ) {
                Some(literal_value(captured, ast.node(expression))?)
            } else {
                None
            };
            clauses.push(Clause {
                expression: expression.map(|id| Site::new(ast, id)),
                witness,
            });
        }
        Ok(SwitchSyntax {
            expression: Site::new(ast, expression),
            clauses,
        })
    }
}

fn literal_value(
    captured: &super::CapturedSyntax<'_>,
    node: &crate::Node<'_>,
) -> Result<LiteralValue, AnalysisError> {
    let ast = &captured.ast;
    if !matches!(
        node.syntax_kind,
        SyntaxKind::StringLiteral | SyntaxKind::NoSubstitutionTemplateLiteral
    ) {
        return Err(super::super::internal(
            "string witness has a different final role",
        ));
    }
    ast.cooked_literal(node)
        .map(|bytes| captured.literal_value(bytes))
        .ok_or_else(|| super::super::internal("string witness lacks exact cooked bytes"))
}
