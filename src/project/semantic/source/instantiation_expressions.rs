//! Syntax for the shared TypeQuery and runtime instantiation-expression owner.

use super::{CallTypeArgument, DeclarationIdentity, Expression, ExpressionRoot, Source};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::references::Location;
use crate::{Value, syntax, syntax_kind::SyntaxKind};

pub(in crate::project::semantic) struct InstantiationExpressionRecipe {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) operand: Expression,
    // Native nil TypeArgumentList differs from a written empty list.
    pub(in crate::project::semantic) arguments: Option<Vec<CallTypeArgument>>,
    pub(in crate::project::semantic) location: Location,
}

impl Source<'_, '_> {
    /// `checkExpressionWithTypeArguments`: retain syntax without checking the
    /// operand or any type argument. Evaluation owns their ordered acquisition.
    pub(in crate::project::semantic) fn instantiation_expression_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<InstantiationExpressionRecipe, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let node = ast.node(id);
        let operand = match node.syntax_kind {
            SyntaxKind::ExpressionWithTypeArguments => super::expression(
                inputs,
                root.file,
                captured,
                syntax::id(node, "expression").ok_or_else(|| {
                    super::super::internal("instantiation expression has no operand")
                })?,
            ),
            SyntaxKind::TypeQuery => super::type_query_operand(
                inputs,
                root.file,
                captured,
                &captured.parents,
                syntax::id(node, "exprName")
                    .ok_or_else(|| super::super::internal("type query has no expression name"))?,
            ),
            _ => {
                return Err(super::super::internal(
                    "instantiation recipe requires its actual native node",
                ));
            }
        };
        let arguments = match node.field("typeArguments") {
            Some(Value::Nodes(arguments)) => Some(
                arguments
                    .iter()
                    .map(|&argument| CallTypeArgument {
                        root: ExpressionRoot {
                            file: root.file,
                            site: Site::new(ast, argument),
                        },
                        expression: super::type_expression(
                            inputs,
                            root.file,
                            captured,
                            &captured.parents,
                            argument,
                        ),
                    })
                    .collect(),
            ),
            None => None,
            _ => {
                return Err(super::super::internal(
                    "native type argument list has another captured shape",
                ));
            }
        };
        Ok(InstantiationExpressionRecipe {
            declaration: DeclarationIdentity::new(root.file, id),
            operand,
            arguments,
            location: super::super::location(inputs, root.file, ast.token_span(id)),
        })
    }
}
