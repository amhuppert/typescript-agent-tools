//! Native read syntax setup and exact reference candidates. Neither operation
//! interprets flow or reconstructs bindings from names.

use super::{CapturedSyntax, Source};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;

pub(in crate::project::semantic) struct VariableReadFacts {
    pub(in crate::project::semantic) declaration_kind: SyntaxKind,
    pub(in crate::project::semantic) root_kind: SyntaxKind,
    pub(in crate::project::semantic) parent_kind: SyntaxKind,
    pub(in crate::project::semantic) declaration_flags: u32,
    pub(in crate::project::semantic) combined_flags: u32,
    pub(in crate::project::semantic) parent_flags: u32,
    pub(in crate::project::semantic) combined_modifiers: u32,
    pub(in crate::project::semantic) at_global_source: bool,
    pub(in crate::project::semantic) definite: bool,
    pub(in crate::project::semantic) initialized: bool,
    pub(in crate::project::semantic) for_in_or_of: bool,
}

pub(in crate::project::semantic) struct FlowSyntax<N = Site> {
    pub(in crate::project::semantic) kind: SyntaxKind,
    pub(in crate::project::semantic) expression: Option<N>,
    pub(in crate::project::semantic) argument: Option<N>,
    pub(in crate::project::semantic) operand: Option<N>,
    pub(in crate::project::semantic) left: Option<N>,
    pub(in crate::project::semantic) right: Option<N>,
    pub(in crate::project::semantic) name: Option<N>,
    pub(in crate::project::semantic) reference_name: Option<String>,
    pub(in crate::project::semantic) keyword_token: Option<u32>,
    pub(in crate::project::semantic) operator: Option<SyntaxKind>,
    pub(in crate::project::semantic) unary_operator: Option<u32>,
    pub(in crate::project::semantic) parent: Option<N>,
    pub(in crate::project::semantic) assignment_target: Option<N>,
    pub(in crate::project::semantic) simple_assignment: bool,
    pub(in crate::project::semantic) assignment_expression: bool,
    pub(in crate::project::semantic) in_type_query: bool,
    pub(in crate::project::semantic) this_identifier: bool,
    pub(in crate::project::semantic) optional_chain: bool,
    pub(in crate::project::semantic) optionality_condition: bool,
}

impl Source<'_, '_> {
    /// The same root-declaration facts for `isParameterOrMutableLocalVariable`
    /// and identifier read setup. A pattern name need not have an Identifier
    /// anchor, so native declaration identity is the internal entry point.
    pub(in crate::project::semantic) fn variable_declaration_read_facts(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<VariableReadFacts, AnalysisError> {
        let is_module = self.inputs.bindings.file_is_module(declaration.file);
        let captured = self.syntax(declaration.file)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let id = declaration.node;
        let node = ast.node(id);
        let root = syntax::root_declaration(ast, parents, id);
        let root_kind = ast.node(root).syntax_kind;
        if root_kind == SyntaxKind::Parameter {
            let parent = parents[root.index()]
                .ok_or_else(|| super::super::internal("parameter has no native parent"))?;
            return Ok(VariableReadFacts {
                declaration_kind: node.syntax_kind,
                root_kind,
                parent_kind: ast.node(parent).syntax_kind,
                declaration_flags: node.flags,
                combined_flags: syntax::combined_node_flags(ast, parents, id),
                parent_flags: ast.node(parent).flags,
                combined_modifiers: syntax::combined_modifier_flags(ast, parents, id),
                at_global_source: false,
                definite: false,
                initialized: syntax::id(node, "initializer").is_some(),
                for_in_or_of: false,
            });
        }
        if root_kind != SyntaxKind::VariableDeclaration {
            return Err(super::super::internal(
                "variable read has no native variable/parameter root",
            ));
        }
        let list = parents[root.index()]
            .ok_or_else(|| super::super::internal("ordinary variable has no declaration list"))?;
        let statement = parents[list.index()];
        let at_global_source = !is_module
            && statement.is_some_and(|statement| {
                ast.node(statement).syntax_kind == SyntaxKind::VariableStatement
                    && parents[statement.index()] == Some(ast.root_id())
            });
        let modifiers = syntax::combined_modifier_flags(ast, parents, id);
        Ok(VariableReadFacts {
            declaration_kind: node.syntax_kind,
            root_kind,
            parent_kind: ast.node(list).syntax_kind,
            declaration_flags: node.flags,
            combined_flags: syntax::combined_node_flags(ast, parents, id),
            parent_flags: ast.node(list).flags,
            combined_modifiers: modifiers,
            at_global_source,
            definite: node.syntax_kind == SyntaxKind::VariableDeclaration
                && syntax::id(node, "exclamationToken").is_some(),
            initialized: syntax::id(node, "initializer").is_some(),
            for_in_or_of: statement.is_some_and(|statement| {
                matches!(
                    ast.node(statement).syntax_kind,
                    SyntaxKind::ForInStatement | SyntaxKind::ForOfStatement
                )
            }),
        })
    }

    /// `isConstantReference`'s binding-pattern branch asks GetRootDeclaration
    /// of the actual parent; it does not use the selected binding element.
    pub(in crate::project::semantic) fn binding_pattern_root_declaration(
        &mut self,
        pattern: super::ExpressionRoot,
    ) -> Result<super::DeclarationIdentity, AnalysisError> {
        let captured = self.syntax(pattern.file)?;
        let id = captured.node_at(pattern.site)?;
        let parent = captured.parents[id.index()]
            .ok_or_else(|| super::super::internal("binding pattern has no declaration parent"))?;
        Ok(super::DeclarationIdentity::new(
            pattern.file,
            syntax::root_declaration(&captured.ast, &captured.parents, parent),
        ))
    }

    /// Reproject a retained native flow site through its exact final-tree
    /// role. Source returns syntax context only; the evaluator owns matching,
    /// narrowing admission and checking order.
    pub(in crate::project::semantic) fn flow_syntax(
        &mut self,
        file: FileId,
        site: Site,
    ) -> Result<FlowSyntax, AnalysisError> {
        let captured = self.syntax(file)?;
        let id = captured.node_at(site)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let node = ast.node(id);
        let child = |field| syntax::id(node, field).map(|id| Site::new(ast, id));
        Ok(FlowSyntax {
            kind: node.syntax_kind,
            expression: child("expression"),
            argument: child("argumentExpression"),
            operand: child("operand"),
            left: child("left"),
            right: child("right"),
            name: child("name"),
            reference_name: match node.syntax_kind {
                SyntaxKind::QualifiedName | SyntaxKind::MetaProperty => {
                    let name = syntax::id(
                        node,
                        if node.syntax_kind == SyntaxKind::QualifiedName {
                            "right"
                        } else {
                            "name"
                        },
                    )
                    .ok_or_else(|| super::super::internal("named reference has no name"))?;
                    Some(
                        ast.identifier_name(name)
                            .ok_or_else(|| {
                                super::super::internal(
                                    "named reference has no exact identifier text",
                                )
                            })?
                            .to_owned(),
                    )
                }
                _ => None,
            },
            keyword_token: syntax::number(node, "keywordToken"),
            operator: syntax::id(node, "operatorToken").map(|id| ast.node(id).syntax_kind),
            unary_operator: syntax::number(node, "operator"),
            parent: parents[id.index()].map(|id| Site::new(ast, id)),
            assignment_target: syntax::assignment_target(ast, |id| parents[id.index()], id)
                .map(|id| Site::new(ast, id)),
            simple_assignment: syntax::is_assignment_expression(ast, id, true),
            assignment_expression: syntax::is_assignment_expression(ast, id, false),
            in_type_query: syntax::is_in_type_query(ast, |id| parents[id.index()], id),
            this_identifier: syntax::is_this_identifier(ast, id),
            optional_chain: syntax::is_optional_chain(ast, id),
            optionality_condition: syntax::is_expression_of_optional_chain_root(
                ast,
                id,
                parents[id.index()],
            ) || parents[id.index()].is_some_and(|parent| {
                let parent = ast.node(parent);
                parent.syntax_kind == SyntaxKind::BinaryExpression
                    && syntax::id(parent, "left") == Some(id)
                    && syntax::id(parent, "operatorToken").is_some_and(|operator| {
                        matches!(
                            ast.node(operator).syntax_kind,
                            SyntaxKind::QuestionQuestionToken
                                | SyntaxKind::QuestionQuestionEqualsToken
                        )
                    })
            }),
        })
    }
}
