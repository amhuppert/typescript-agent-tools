//! Array and const-context syntax retains actual children and ancestry.

use super::{
    CapturedSyntax, Expression, ExpressionRoot, Source, TypeExpression, expression, type_expression,
};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::syntax_kind::SyntaxKind as K;
use crate::{NodeId, syntax};

pub(in crate::project::semantic) struct ArrayLiteralRecipe {
    pub(in crate::project::semantic) elements: Vec<ArrayElement>,
    pub(in crate::project::semantic) destructuring: bool,
    pub(in crate::project::semantic) spread_into_call: bool,
}
pub(in crate::project::semantic) struct ArrayElement {
    pub(in crate::project::semantic) expression: Expression,
    pub(in crate::project::semantic) spread: bool,
    pub(in crate::project::semantic) omitted: bool,
}

#[derive(Clone, Copy, Default)]
pub(in crate::project::semantic) struct SpreadIndices {
    pub(in crate::project::semantic) first: Option<usize>,
    pub(in crate::project::semantic) last: Option<usize>,
}
pub(in crate::project::semantic) enum ConstArgument {
    Yes,
    No,
    EnumReference(ExpressionRoot),
}
pub(in crate::project::semantic) struct ConstContext {
    pub(in crate::project::semantic) assertion: bool,
    pub(in crate::project::semantic) argument: ConstArgument,
    pub(in crate::project::semantic) inherited: Option<ExpressionRoot>,
}
pub(in crate::project::semantic) struct AssertionRecipe {
    pub(in crate::project::semantic) expression: Expression,
    pub(in crate::project::semantic) annotation: Option<TypeExpression>,
    pub(in crate::project::semantic) annotation_site: Option<Site>,
}

impl Source<'_, '_> {
    /// `getSpreadIndices`: cache the first and last spread once per actual
    /// array literal, including the completed no-spread result. Contextual
    /// checking asks this only after acquiring the parent's contextual type.
    pub(in crate::project::semantic) fn array_spread_indices(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<SpreadIndices, AnalysisError> {
        if let Some(indices) = self.array_spreads.get(&root) {
            return Ok(*indices);
        }
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let mut indices = SpreadIndices::default();
        for (index, &element) in syntax::ids(captured.ast.node(id), "elements")
            .iter()
            .enumerate()
        {
            if captured.ast.node(element).syntax_kind == K::SpreadElement {
                if indices.first.is_none() {
                    indices.first = Some(index);
                }
                indices.last = Some(index);
            }
        }
        self.array_spreads.reserve(1);
        self.array_spreads.insert(root, indices);
        Ok(indices)
    }

    pub(in crate::project::semantic) fn is_type_assertion(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(root.file)?;
        Ok(syntax::is_type_assertion(
            &captured.ast,
            captured.node_at(root.site)?,
        ))
    }
    /// `checkArrayLiteral` / `isSpreadIntoCallOrNew`: direct element roles,
    /// assignment-target ancestry and parentheses above the spread operand.
    pub(in crate::project::semantic) fn array_literal_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ArrayLiteralRecipe, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let destructuring = syntax::assignment_target(ast, |id| parents[id.index()], id).is_some();
        let mut parent = parents[id.index()];
        while let Some(node) =
            parent.filter(|&node| ast.node(node).syntax_kind == K::ParenthesizedExpression)
        {
            parent = parents[node.index()];
        }
        let spread_into_call = parent.is_some_and(|parent| {
            ast.node(parent).syntax_kind == K::SpreadElement
                && parents[parent.index()].is_some_and(|outer| {
                    matches!(
                        ast.node(outer).syntax_kind,
                        K::CallExpression | K::NewExpression
                    )
                })
        });
        let mut elements = Vec::new();
        for &element in syntax::ids(ast.node(id), "elements") {
            let spread = ast.node(element).syntax_kind == K::SpreadElement;
            let operand = if spread {
                syntax::id(ast.node(element), "expression")
                    .ok_or_else(|| super::super::internal("array spread has no expression"))?
            } else {
                element
            };
            elements.push(ArrayElement {
                expression: expression(inputs, root.file, captured, operand),
                spread,
                omitted: ast.node(element).syntax_kind == K::OmittedExpression,
            });
        }
        Ok(ArrayLiteralRecipe {
            elements,
            destructuring,
            spread_into_call,
        })
    }

    /// Syntax half of `isConstContext`. The contextual type and enum identity
    /// questions remain evaluator dependencies, in native short-circuit order.
    pub(in crate::project::semantic) fn const_context(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ConstContext, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let parent = parents[id.index()];
        let assertion = parent.is_some_and(|parent| syntax::is_const_assertion(ast, parent));
        let inherited = parent
            .and_then(|parent| match ast.node(parent).syntax_kind {
                K::ParenthesizedExpression | K::ArrayLiteralExpression | K::SpreadElement => {
                    Some(parent)
                }
                K::PropertyAssignment | K::ShorthandPropertyAssignment | K::TemplateSpan => {
                    parents[parent.index()]
                }
                _ => None,
            })
            .map(|node| ExpressionRoot {
                file: root.file,
                site: Site::new(ast, node),
            });
        let argument = const_argument(ast, root.file, id)?;
        Ok(ConstContext {
            assertion,
            argument,
            inherited,
        })
    }

    /// `checkAssertion` distinguishes the exact const reference before asking
    /// for a type-node recipe. It still checks its expression first.
    pub(in crate::project::semantic) fn assertion_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<AssertionRecipe, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let node = ast.node(id);
        let operand = syntax::id(node, "expression")
            .ok_or_else(|| super::super::internal("assertion has no expression"))?;
        let annotation = if syntax::is_const_assertion(ast, id) {
            None
        } else {
            let annotation = syntax::id(node, "type")
                .ok_or_else(|| super::super::internal("assertion has no type"))?;
            Some(type_expression(
                inputs, root.file, captured, parents, annotation,
            ))
        };
        let annotation_site = annotation.as_ref().map(|_| {
            Site::new(
                ast,
                syntax::id(node, "type").expect("established assertion type"),
            )
        });
        Ok(AssertionRecipe {
            expression: expression(inputs, root.file, captured, operand),
            annotation,
            annotation_site,
        })
    }
}

/// `isValidConstAssertionArgument`: parentheses recurse, direct unary operands
/// have the native numeric/bigint asymmetry, and member receivers ask an enum
/// symbol question rather than inspecting the member's resulting type.
fn const_argument(
    ast: &crate::Ast<'_>,
    file: crate::project::sources::FileId,
    id: NodeId,
) -> Result<ConstArgument, AnalysisError> {
    let id = syntax::skip_parentheses(ast, id);
    let node = ast.node(id);
    Ok(match node.syntax_kind {
        K::StringLiteral
        | K::NoSubstitutionTemplateLiteral
        | K::NumericLiteral
        | K::BigIntLiteral
        | K::TrueKeyword
        | K::FalseKeyword
        | K::ArrayLiteralExpression
        | K::ObjectLiteralExpression
        | K::TemplateExpression => ConstArgument::Yes,
        K::PrefixUnaryExpression => {
            let operand = syntax::id(node, "operand")
                .ok_or_else(|| super::super::internal("unary const argument has no operand"))?;
            let kind = ast.node(operand).syntax_kind;
            if matches!(syntax::number(node,"operator"), Some(op) if op == K::MinusToken.number() && matches!(kind,K::NumericLiteral|K::BigIntLiteral) || op == K::PlusToken.number() && kind == K::NumericLiteral)
            {
                ConstArgument::Yes
            } else {
                ConstArgument::No
            }
        }
        K::PropertyAccessExpression | K::ElementAccessExpression => {
            let operand = syntax::id(node, "expression")
                .ok_or_else(|| super::super::internal("const argument access has no receiver"))?;
            let operand = syntax::skip_parentheses(ast, operand);
            if syntax::is_entity_name_expression(ast, operand) {
                ConstArgument::EnumReference(ExpressionRoot {
                    file,
                    site: Site::new(ast, operand),
                })
            } else {
                ConstArgument::No
            }
        }
        _ => ConstArgument::No,
    })
}
