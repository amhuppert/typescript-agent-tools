//! Exact call syntax for native effect acquisition. Semantic eligibility and
//! signature choice belong to the evaluator, never this source projection.

use super::{
    CapturedSyntax, DeclarationIdentity, Expression, ExpressionRoot, SignatureRoot, Source,
    expression,
};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;

pub(in crate::project::semantic) struct CallSyntax {
    pub(in crate::project::semantic) binary: bool,
    pub(in crate::project::semantic) callee: Expression,
    pub(in crate::project::semantic) expression_statement: bool,
    pub(in crate::project::semantic) optional: bool,
    pub(in crate::project::semantic) callee_optional_root: bool,
    pub(in crate::project::semantic) callee_optional_chain: bool,
    pub(in crate::project::semantic) super_call: bool,
    pub(in crate::project::semantic) location: Location,
}

pub(in crate::project::semantic) enum ExplicitDeclarationType {
    Annotated,
    ForOf {
        expression: Expression,
        await_: bool,
    },
    Absent,
}

pub(in crate::project::semantic) enum DottedName {
    Identifier {
        file: FileId,
        anchor: crate::Span,
    },
    Inner(Expression),
    Access(ExpressionRoot),
    This {
        signature: Option<SignatureRoot>,
        class: Option<(DeclarationIdentity, bool)>,
    },
    Super(ExpressionRoot),
    Absent,
}

impl Source<'_, '_> {
    /// Syntax dispatch for `getTypeOfDottedName` and `getExplicitThisType`.
    /// Default nil is an established answer, including element accesses,
    /// non-null assertions and calls. Only parentheses recurse.
    pub(in crate::project::semantic) fn dotted_name(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<DottedName, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        let ast = &captured.ast;
        let syntax_node = ast.node(node);
        if syntax_node.flags & crate::compiler::node::IN_WITH_STATEMENT != 0 {
            return Ok(DottedName::Absent);
        }
        Ok(match syntax_node.syntax_kind {
            SyntaxKind::Identifier => DottedName::Identifier {
                file: root.file,
                anchor: ast.token_span(node),
            },
            SyntaxKind::PropertyAccessExpression => DottedName::Access(root),
            SyntaxKind::ParenthesizedExpression => DottedName::Inner(expression(
                inputs,
                root.file,
                captured,
                syntax::id(syntax_node, "expression")
                    .ok_or_else(|| super::super::internal("parentheses have no expression"))?,
            )),
            SyntaxKind::SuperKeyword => DottedName::Super(root),
            SyntaxKind::ThisKeyword => {
                let container = syntax::this_container(ast, &captured.parents, node, false, false)
                    .ok_or_else(|| {
                        super::super::internal("explicit this has no containing syntax")
                    })?;
                let signature = if syntax::is_function_like_kind(ast.node(container).syntax_kind) {
                    Some(
                        super::signature_root(ast, root.file, container)
                            .map_err(super::super::internal)?,
                    )
                } else {
                    None
                };
                let class = captured.parents[container.index()]
                    .filter(|&parent| {
                        matches!(
                            ast.node(parent).syntax_kind,
                            SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                        )
                    })
                    .map(|class| {
                        (
                            DeclarationIdentity::new(root.file, class),
                            syntax::is_static(ast, container),
                        )
                    });
                DottedName::This { signature, class }
            }
            _ => DottedName::Absent,
        })
    }
    /// `getExplicitTypeOfSymbol` / `isDeclarationWithExplicitTypeAnnotation`
    /// inspect the actual ValueDeclaration, without constructing its value
    /// recipe or visiting an unneeded initializer. For-of has a separate
    /// expression dependency after the annotation question.
    pub(in crate::project::semantic) fn explicit_declaration_type(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<ExplicitDeclarationType, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let id = declaration.node;
        let ast = &captured.ast;
        let node = ast.node(id);
        let kind = node.syntax_kind;
        let annotated = matches!(
            kind,
            SyntaxKind::VariableDeclaration
                | SyntaxKind::PropertyDeclaration
                | SyntaxKind::PropertySignature
                | SyntaxKind::Parameter
        ) && syntax::id(node, "type").is_some()
            || kind == SyntaxKind::BinaryExpression
                && syntax::id(node, "right").is_some_and(|right| {
                    syntax::is_function_like_kind(ast.node(right).syntax_kind)
                        && syntax::id(ast.node(right), "type").is_some()
                });
        if annotated {
            return Ok(ExplicitDeclarationType::Annotated);
        }
        if kind == SyntaxKind::VariableDeclaration
            && let Some(statement) =
                captured.parents[id.index()].and_then(|list| captured.parents[list.index()])
            && ast.node(statement).syntax_kind == SyntaxKind::ForOfStatement
        {
            let value = syntax::id(ast.node(statement), "expression")
                .ok_or_else(|| super::super::internal("for-of statement has no expression"))?;
            return Ok(ExplicitDeclarationType::ForOf {
                expression: expression(inputs, declaration.file, captured, value),
                await_: syntax::id(ast.node(statement), "awaitModifier").is_some(),
            });
        }
        Ok(ExplicitDeclarationType::Absent)
    }

    /// Actual argument sites for checker `hasMatchingArgument`. Its flow
    /// callers supply calls, while `isUncalledFunctionReference` also supplies
    /// new expressions. Projection never consumes types or chooses a predicate.
    pub(in crate::project::semantic) fn call_arguments(
        &mut self,
        file: FileId,
        site: Site,
    ) -> Result<Vec<Site>, AnalysisError> {
        let captured = self.syntax(file)?;
        let id = captured.node_at(site)?;
        let ast = &captured.ast;
        if !matches!(
            ast.node(id).syntax_kind,
            SyntaxKind::CallExpression | SyntaxKind::NewExpression
        ) {
            return Err(super::super::internal(
                "matching arguments have a different call syntax role",
            ));
        }
        Ok(syntax::ids(ast.node(id), "arguments")
            .iter()
            .map(|&id| Site::new(ast, id))
            .collect())
    }

    /// Exact actual argument for `getTypePredicateArgument` and assertion
    /// reachability. A missing argument is native absence; no expression type
    /// or printed origin is substituted for this syntax dependency.
    pub(in crate::project::semantic) fn call_argument(
        &mut self,
        file: FileId,
        site: Site,
        index: usize,
    ) -> Result<Option<Site>, AnalysisError> {
        let captured = self.syntax(file)?;
        let id = captured.node_at(site)?;
        let ast = &captured.ast;
        if ast.node(id).syntax_kind != SyntaxKind::CallExpression {
            return Err(super::super::internal(
                "predicate argument has a different call syntax role",
            ));
        }
        Ok(syntax::ids(ast.node(id), "arguments")
            .get(index)
            .map(|&id| Site::new(ast, id)))
    }

    /// Reuse the whole checker `isFalseExpression` syntax owner. This query
    /// deliberately does not check the argument's value or diagnostics.
    pub(in crate::project::semantic) fn false_expression(
        &mut self,
        file: FileId,
        site: Site,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(file)?;
        let id = captured.node_at(site)?;
        Ok(syntax::is_false_expression(&captured.ast, id))
    }

    /// Source inputs for `getEffectsSignature` and `getTypeOfDottedName`.
    /// Reuse the final role attachment and ordinary expression projection.
    pub(in crate::project::semantic) fn call_syntax(
        &mut self,
        file: FileId,
        site: Site,
    ) -> Result<CallSyntax, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(file)?;
        let id = captured.node_at(site)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        if !matches!(
            ast.node(id).syntax_kind,
            SyntaxKind::CallExpression | SyntaxKind::BinaryExpression
        ) {
            return Err(super::super::internal(
                "effect call has a different native syntax role",
            ));
        }
        let node = ast.node(id);
        let binary = node.syntax_kind == SyntaxKind::BinaryExpression;
        let callee = syntax::id(node, if binary { "right" } else { "expression" })
            .ok_or_else(|| super::super::internal("native effect call has no target expression"))?;
        Ok(CallSyntax {
            binary,
            expression_statement: parents[id.index()].is_some_and(|parent| {
                ast.node(parent).syntax_kind == SyntaxKind::ExpressionStatement
            }),
            optional: syntax::is_optional_chain(ast, id),
            callee_optional_root: syntax::is_expression_of_optional_chain_root(
                ast,
                callee,
                Some(id),
            ),
            callee_optional_chain: syntax::is_optional_chain(ast, callee),
            super_call: ast.node(callee).syntax_kind == SyntaxKind::SuperKeyword,
            location: super::super::location(inputs, file, ast.token_span(id)),
            callee: expression(inputs, file, captured, callee),
        })
    }
}
