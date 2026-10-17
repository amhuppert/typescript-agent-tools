//! Syntax consumed by native diagnostic elaboration; semantic questions stay
//! with the relater and retain the actual expression and declaration identities.

use super::super::model::DemandError;
use super::{DeclarationIdentity, ExpressionRoot, Source};
use crate::project::binding::flow::Site;
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) enum Elaboration {
    None,
    Inner(ExpressionRoot),
    Object(Vec<ElaborationProperty>),
    Array(Vec<Option<ExpressionRoot>>),
    Arrow {
        body: Option<ExpressionRoot>,
        asynchronous: bool,
    },
}

pub(in crate::project::semantic) struct ElaborationProperty {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) name: ExpressionRoot,
    pub(in crate::project::semantic) initializer: Option<ExpressionRoot>,
    pub(in crate::project::semantic) computed_non_literal: bool,
}

impl Source<'_, '_> {
    /// `elaborateError`, `elaborateObjectLiteral`, `elaborateArrayLiteral`, and
    /// `elaborateArrowFunction` inspect only syntax at this boundary. Even
    /// omitted array elements retain their index in the native enumeration.
    pub(in crate::project::semantic) fn elaboration_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Elaboration, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let node = ast.node(id);
        let at = |id| ExpressionRoot {
            file: root.file,
            site: Site::new(ast, id),
        };
        let inner = |field| {
            syntax::id(node, field).map(at).ok_or_else(|| {
                super::super::internal("elaboration expression lacks a required child")
            })
        };
        Ok(match node.syntax_kind {
            K::ParenthesizedExpression | K::JsxExpression => {
                Elaboration::Inner(inner("expression")?)
            }
            K::AsExpression if syntax::is_const_assertion(ast, id) => {
                Elaboration::Inner(inner("expression")?)
            }
            K::BinaryExpression
                if syntax::id(node, "operatorToken").is_some_and(|token| {
                    matches!(ast.node(token).syntax_kind, K::EqualsToken | K::CommaToken)
                }) =>
            {
                Elaboration::Inner(inner("right")?)
            }
            K::ObjectLiteralExpression => {
                let mut properties = Vec::new();
                for &property in syntax::ids(node, "properties") {
                    let prop = ast.node(property);
                    if prop.syntax_kind == K::SpreadAssignment {
                        continue;
                    }
                    let name = syntax::id(prop, "name")
                        .ok_or_else(|| super::super::internal("object property has no name"))?;
                    let initializer = if prop.syntax_kind == K::PropertyAssignment {
                        Some(syntax::id(prop, "initializer").ok_or_else(|| {
                            super::super::internal("property assignment has no initializer")
                        })?)
                    } else {
                        None
                    };
                    // `IsComputedNonLiteralName` uses the direct expression;
                    // parentheses and unary minus deliberately remain nonliteral.
                    let computed_non_literal = ast.node(name).syntax_kind
                        == K::ComputedPropertyName
                        && syntax::id(ast.node(name), "expression").is_some_and(|expression| {
                            !syntax::is_string_or_numeric_literal_like(ast, expression)
                        });
                    properties.push(ElaborationProperty {
                        declaration: DeclarationIdentity::new(root.file, property),
                        name: at(name),
                        initializer: initializer.map(at),
                        computed_non_literal,
                    });
                }
                Elaboration::Object(properties)
            }
            K::ArrayLiteralExpression => Elaboration::Array(
                syntax::ids(node, "elements")
                    .iter()
                    .map(|&id| (ast.node(id).syntax_kind != K::OmittedExpression).then(|| at(id)))
                    .collect(),
            ),
            K::ArrowFunction => {
                let body = syntax::id(node, "body");
                if body.is_some_and(|body| ast.node(body).syntax_kind == K::Block)
                    || syntax::ids(node, "parameters")
                        .iter()
                        .any(|&parameter| syntax::id(ast.node(parameter), "type").is_some())
                {
                    Elaboration::None
                } else {
                    Elaboration::Arrow {
                        body: body.map(at),
                        asynchronous: syntax::function_flags(ast, id).is_async(),
                    }
                }
            }
            _ => Elaboration::None,
        })
    }

    /// `getEffectiveCheckNode` for TypeScript strips parentheses and satisfies
    /// expressions only. Assertions belong to a different native question.
    pub(in crate::project::semantic) fn effective_check_node(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ExpressionRoot, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let mut id = captured.node_at(root.site)?;
        while matches!(
            ast.node(id).syntax_kind,
            K::ParenthesizedExpression | K::SatisfiesExpression
        ) {
            id = syntax::id(ast.node(id), "expression")
                .ok_or_else(|| super::super::internal("outer expression lacks operand"))?;
        }
        Ok(ExpressionRoot {
            file: root.file,
            site: Site::new(ast, id),
        })
    }
}
