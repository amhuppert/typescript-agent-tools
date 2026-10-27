//! Syntax for the shared TypeQuery, ImportType and runtime
//! instantiation-expression owner.

use super::{
    CallTypeArgument, CapturedSyntax, DeclarationIdentity, Expression, ExpressionRoot, Source,
};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::{NodeId, Value, syntax, syntax_kind::SyntaxKind};

pub(in crate::project::semantic) struct InstantiationExpressionRecipe {
    pub(in crate::project::semantic) operand: Expression,
    pub(in crate::project::semantic) site: InstantiationSite,
}

/// `getInstantiationExpressionType`'s node: its identity and written type
/// arguments. A `typeof import(...)` node instantiates the same way.
pub(in crate::project::semantic) struct InstantiationSite {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
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
        Ok(InstantiationExpressionRecipe {
            operand,
            site: instantiation_site(inputs, root.file, captured, id)?,
        })
    }
}

/// The actual node and its written `typeArguments`, without checking them.
pub(super) fn instantiation_site(
    inputs: super::super::inputs::CheckerInputs<'_>,
    file: FileId,
    captured: &CapturedSyntax<'_>,
    id: NodeId,
) -> Result<InstantiationSite, AnalysisError> {
    let ast = &captured.ast;
    let arguments = match ast.node(id).field("typeArguments") {
        Some(Value::Nodes(arguments)) => Some(
            arguments
                .iter()
                .map(|&argument| CallTypeArgument {
                    root: ExpressionRoot {
                        file,
                        site: Site::new(ast, argument),
                    },
                    expression: super::type_expression(
                        inputs,
                        file,
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
    Ok(InstantiationSite {
        declaration: DeclarationIdentity::new(file, id),
        arguments,
        location: super::super::location(inputs, file, ast.token_span(id)),
    })
}
