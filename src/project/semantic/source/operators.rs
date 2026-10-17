//! Operator syntax retains actual operand nodes and diagnostic traversal order.

use super::{CapturedSyntax, Expression, ExpressionRoot, Source, expression};
use crate::analysis::AnalysisError;
use crate::syntax_kind::SyntaxKind as K;
use crate::{Ast, NodeId, syntax};

pub(in crate::project::semantic) struct BinaryRecipe {
    pub(in crate::project::semantic) operator: K,
    pub(in crate::project::semantic) left: Expression,
    pub(in crate::project::semantic) right: Expression,
    pub(in crate::project::semantic) test_truthy: bool,
    pub(in crate::project::semantic) truthy_body: Option<ExpressionRoot>,
    pub(in crate::project::semantic) destructuring: Option<ExpressionRoot>,
    pub(in crate::project::semantic) right_is_this: bool,
}

pub(in crate::project::semantic) struct NonNullAssertionRecipe {
    pub(in crate::project::semantic) operand: Expression,
    pub(in crate::project::semantic) chain: bool,
    pub(in crate::project::semantic) operand_optional_root: bool,
    pub(in crate::project::semantic) operand_optional_chain: bool,
    pub(in crate::project::semantic) outermost_chain: bool,
}

pub(in crate::project::semantic) struct TruthyCheck {
    pub(in crate::project::semantic) expression: Expression,
    pub(in crate::project::semantic) recheck: bool,
    pub(in crate::project::semantic) property_cast: bool,
    pub(in crate::project::semantic) condition: ExpressionRoot,
    pub(in crate::project::semantic) receiver: Option<ExpressionRoot>,
    pub(in crate::project::semantic) tested_node: Option<ExpressionRoot>,
}

pub(in crate::project::semantic) enum SymbolUseReceiver {
    Reference,
    Property {
        receiver: ExpressionRoot,
        name: ExpressionRoot,
    },
    Call(ExpressionRoot),
    Other,
}

/// Pausable ForEachChild traversal. Expanding the previous node on the next
/// call preserves native early success without inspecting its descendants.
pub(in crate::project::semantic) struct SymbolUseWalk {
    next_children: Option<ExpressionRoot>,
    pending: Vec<ExpressionRoot>,
}

impl Source<'_, '_> {
    /// Actual operands and shared optional-chain syntax for
    /// `checkNonNullAssertion` / `checkNonNullChain`.
    pub(in crate::project::semantic) fn non_null_assertion_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<NonNullAssertionRecipe, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let ast = &captured.ast;
        let operand = syntax::id(ast.node(id), "expression")
            .ok_or_else(|| super::super::internal("non-null assertion has no actual operand"))?;
        Ok(NonNullAssertionRecipe {
            operand: expression(inputs, root.file, captured, operand),
            chain: syntax::is_optional_chain(ast, id),
            operand_optional_root: syntax::is_expression_of_optional_chain_root(
                ast,
                operand,
                Some(id),
            ),
            operand_optional_chain: syntax::is_optional_chain(ast, operand),
            outermost_chain: syntax::is_outermost_optional_chain(
                ast,
                id,
                captured.parents[id.index()],
            ),
        })
    }

    pub(in crate::project::semantic) fn symbol_use_walk(
        &self,
        root: ExpressionRoot,
    ) -> SymbolUseWalk {
        SymbolUseWalk {
            next_children: Some(root),
            pending: Vec::new(),
        }
    }

    pub(in crate::project::semantic) fn next_symbol_use(
        &mut self,
        walk: &mut SymbolUseWalk,
    ) -> Result<Option<ExpressionRoot>, super::super::model::DemandError> {
        if let Some(root) = walk.next_children.take() {
            for child in self.symbol_use_children(root)?.into_iter().rev() {
                walk.pending.push(child);
            }
        }
        let next = walk.pending.pop();
        walk.next_children = next;
        Ok(next)
    }

    /// Actual parent identity for native symbol-use traversal. Parentheses
    /// are deliberately not skipped by these checker operations.
    pub(in crate::project::semantic) fn symbol_use_parent(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<ExpressionRoot>, super::super::model::DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        Ok(captured.parents[id.index()].map(|parent| ExpressionRoot {
            file: root.file,
            site: crate::project::binding::flow::Site::new(&captured.ast, parent),
        }))
    }

    /// `ForEachChild` order comes from Ast's shared traversal owner. Both
    /// symbol-use operations visit these children, excluding the root itself.
    fn symbol_use_children(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Vec<ExpressionRoot>, super::super::model::DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let mut children = Vec::new();
        for child in captured.ast.children(id) {
            children.push(ExpressionRoot {
                file: root.file,
                site: crate::project::binding::flow::Site::new(&captured.ast, child),
            });
        }
        Ok(children)
    }

    /// `isSymbolUsedInBinaryExpressionChain` advances through immediate &&
    /// parents and searches only each actual right operand's descendants.
    pub(in crate::project::semantic) fn symbol_use_binary_step(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<(ExpressionRoot, Option<ExpressionRoot>)>, super::super::model::DemandError>
    {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        if ast.node(id).syntax_kind != K::BinaryExpression
            || binary_operator(ast, id)? != K::AmpersandAmpersandToken
        {
            return Ok(None);
        }
        let right = syntax::id(ast.node(id), "right")
            .ok_or_else(|| super::super::internal("binary symbol-use node lacks right operand"))?;
        let at = |id| ExpressionRoot {
            file: root.file,
            site: crate::project::binding::flow::Site::new(ast, id),
        };
        Ok(Some((at(right), captured.parents[id.index()].map(at))))
    }

    /// `isSymbolUsedInConditionBody` compares only identifiers/this, dot
    /// accesses and calls. Assertions, parentheses and element access stop
    /// its receiver comparison even when ordinary reference matching differs.
    pub(in crate::project::semantic) fn symbol_use_receiver(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<SymbolUseReceiver, super::super::model::DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = ast.node(captured.node_at(root.site)?);
        let at = |field| {
            syntax::id(node, field)
                .map(|id| ExpressionRoot {
                    file: root.file,
                    site: crate::project::binding::flow::Site::new(ast, id),
                })
                .ok_or_else(|| super::super::internal("symbol-use receiver lacks required child"))
        };
        Ok(match node.syntax_kind {
            K::Identifier | K::ThisKeyword => SymbolUseReceiver::Reference,
            K::PropertyAccessExpression => SymbolUseReceiver::Property {
                receiver: at("expression")?,
                name: at("name")?,
            },
            K::CallExpression => SymbolUseReceiver::Call(at("expression")?),
            _ => SymbolUseReceiver::Other,
        })
    }

    /// `checkConditionalExpression` keeps the condition and both result arms
    /// as actual expression nodes; mode and checking order belong to the checker.
    pub(in crate::project::semantic) fn conditional_expression_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<[Expression; 3], AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = ast.node(captured.node_at(root.site)?);
        let expression_at = |field| -> Result<Expression, AnalysisError> {
            let id = syntax::id(node, field).ok_or_else(|| {
                super::super::internal("conditional expression lacks a required arm")
            })?;
            Ok(expression(inputs, root.file, captured, id))
        };
        Ok([
            expression_at("condition")?,
            expression_at("whenTrue")?,
            expression_at("whenFalse")?,
        ])
    }

    /// `checkDeleteExpression` checks the original operand, then performs
    /// syntax and resolved-link questions on its parenthesis-stripped node.
    pub(in crate::project::semantic) fn delete_operand(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<(Expression, ExpressionRoot), AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = ast.node(captured.node_at(root.site)?);
        let operand = syntax::id(node, "expression")
            .ok_or_else(|| super::super::internal("delete expression lacks operand"))?;
        let actual = syntax::skip_parentheses(ast, operand);
        Ok((
            expression(inputs, root.file, captured, operand),
            ExpressionRoot {
                file: root.file,
                site: crate::project::binding::flow::Site::new(ast, actual),
            },
        ))
    }

    /// `checkTypeOfExpression` asks its actual operand in Normal mode.
    pub(in crate::project::semantic) fn typeof_operand(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Expression, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let operand = syntax::id(captured.ast.node(id), "expression")
            .ok_or_else(|| super::super::internal("typeof expression lacks operand"))?;
        Ok(expression(inputs, root.file, captured, operand))
    }

    /// Syntax half of `checkBinaryLikeExpression`: operand order and the
    /// enclosing-if truthy diagnostic gate remain independent of result type.
    pub(in crate::project::semantic) fn binary_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<BinaryRecipe, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let ast = &captured.ast;
        let node = ast.node(id);
        let operator = binary_operator(ast, id)?;
        let left = syntax::id(node, "left")
            .ok_or_else(|| super::super::internal("binary lacks left operand"))?;
        let right = syntax::id(node, "right")
            .ok_or_else(|| super::super::internal("binary lacks right operand"))?;
        let mut parent = captured.parents[id.index()];
        while let Some(node) = parent {
            if ast.node(node).syntax_kind != K::ParenthesizedExpression && !logical(ast, node) {
                break;
            }
            parent = captured.parents[node.index()];
        }
        let test_truthy = logical_operator(operator)
            && (operator == K::AmpersandAmpersandToken
                || parent.is_some_and(|node| ast.node(node).syntax_kind == K::IfStatement));
        Ok(BinaryRecipe {
            operator,
            left: expression(inputs, root.file, captured, left),
            right: expression(inputs, root.file, captured, right),
            test_truthy,
            truthy_body: parent
                .filter(|&parent| ast.node(parent).syntax_kind == K::IfStatement)
                .and_then(|parent| syntax::id(ast.node(parent), "thenStatement"))
                .map(|id| ExpressionRoot {
                    file: root.file,
                    site: crate::project::binding::flow::Site::new(ast, id),
                }),
            destructuring: syntax::is_destructuring_assignment(ast, id).then_some(ExpressionRoot {
                file: root.file,
                site: crate::project::binding::flow::Site::new(ast, left),
            }),
            right_is_this: ast.node(right).syntax_kind == K::ThisKeyword,
        })
    }

    /// `checkTestingKnownTruthyTypes` / `checkTestingKnownTruthyType` syntax
    /// traversal. Each emitted question retains whether native rechecks its
    /// selected right-hand location rather than reusing the supplied type.
    pub(in crate::project::semantic) fn truthy_checks(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Vec<TruthyCheck>, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let mut checks = Vec::new();
        collect_truthy(inputs, root.file, captured, id, &mut checks)?;
        Ok(checks)
    }

    /// `isGlobalNaN`'s cheap syntax gate precedes global-symbol acquisition.
    pub(in crate::project::semantic) fn global_nan_candidate(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<ExpressionRoot>, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = syntax::skip_parentheses(&captured.ast, captured.node_at(root.site)?);
        Ok((captured.ast.node(id).syntax_kind == K::Identifier
            && captured.ast.identifier_name(id) == Some("NaN"))
        .then(|| ExpressionRoot {
            file: root.file,
            site: crate::project::binding::flow::Site::new(&captured.ast, id),
        }))
    }
}

fn binary_operator(ast: &Ast<'_>, id: NodeId) -> Result<K, AnalysisError> {
    let token = syntax::id(ast.node(id), "operatorToken")
        .ok_or_else(|| super::super::internal("binary lacks operator token"))?;
    Ok(ast.node(token).syntax_kind)
}

fn logical_operator(kind: K) -> bool {
    matches!(
        kind,
        K::AmpersandAmpersandToken | K::BarBarToken | K::QuestionQuestionToken
    )
}

fn logical(ast: &Ast<'_>, id: NodeId) -> bool {
    ast.node(id).syntax_kind == K::BinaryExpression
        && syntax::id(ast.node(id), "operatorToken")
            .is_some_and(|token| logical_operator(ast.node(token).syntax_kind))
}

fn collect_truthy(
    inputs: crate::project::semantic::inputs::CheckerInputs<'_>,
    file: crate::project::sources::FileId,
    captured: &CapturedSyntax<'_>,
    id: NodeId,
    checks: &mut Vec<TruthyCheck>,
) -> Result<(), AnalysisError> {
    let ast = &captured.ast;
    let mut current = syntax::skip_parentheses(ast, id);
    loop {
        let location = if logical(ast, current) {
            syntax::skip_parentheses(
                ast,
                syntax::id(ast.node(current), "right").ok_or_else(|| {
                    super::super::internal("logical expression lacks right operand")
                })?,
            )
        } else {
            current
        };
        // isModuleExportsAccessExpression uses precisely module.exports.
        let property = ast.node(location).syntax_kind == K::PropertyAccessExpression;
        let receiver = syntax::id(ast.node(location), "expression");
        let module_exports = syntax::is_module_exports_access_expression(ast, location);
        if !module_exports {
            if logical(ast, location) {
                collect_truthy(inputs, file, captured, location, checks)?;
            } else {
                checks.push(TruthyCheck {
                    expression: expression(inputs, file, captured, location),
                    recheck: location != current,
                    property_cast: property
                        && receiver.is_some_and(|node| syntax::is_type_assertion(ast, node)),
                    condition: ExpressionRoot {
                        file,
                        site: crate::project::binding::flow::Site::new(ast, current),
                    },
                    receiver: receiver.filter(|_| property).map(|id| ExpressionRoot {
                        file,
                        site: crate::project::binding::flow::Site::new(ast, id),
                    }),
                    tested_node: (if ast.node(location).syntax_kind == K::Identifier {
                        Some(location)
                    } else if property {
                        syntax::id(ast.node(location), "name")
                    } else {
                        None
                    })
                    .map(|id| ExpressionRoot {
                        file,
                        site: crate::project::binding::flow::Site::new(ast, id),
                    }),
                });
            }
        }
        if ast.node(current).syntax_kind != K::BinaryExpression
            || !matches!(
                binary_operator(ast, current)?,
                K::BarBarToken | K::QuestionQuestionToken
            )
        {
            break;
        }
        current = syntax::skip_parentheses(
            ast,
            syntax::id(ast.node(current), "left")
                .ok_or_else(|| super::super::internal("logical expression lacks left operand"))?,
        );
    }
    Ok(())
}
