//! Assignment syntax questions shared by binder flow and checker setup.

use super::{id, number};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId};

/// TypeScript `IsAssignmentExpression`: the assignment operator and native
/// left-hand-side grammar are one question, with an explicit simple-only mode.
pub(crate) fn is_assignment_expression(
    ast: &Ast<'_>,
    node: NodeId,
    exclude_compound: bool,
) -> bool {
    let syntax = ast.node(node);
    syntax.syntax_kind == SyntaxKind::BinaryExpression
        && id(syntax, "operatorToken").is_some_and(|operator| {
            let operator = ast.node(operator).syntax_kind;
            if exclude_compound {
                operator == SyntaxKind::EqualsToken
            } else {
                is_assignment_operator(operator)
            }
        })
        && id(syntax, "left").is_some_and(|left| super::is_left_hand_side_expression(ast, left))
}

/// TypeScript `IsDestructuringAssignment`: only a simple assignment whose
/// immediate left operand is an array or object literal has pattern order.
pub(crate) fn is_destructuring_assignment(ast: &Ast<'_>, node: NodeId) -> bool {
    is_assignment_expression(ast, node, true)
        && id(ast.node(node), "left").is_some_and(|left| {
            matches!(
                ast.node(left).syntax_kind,
                SyntaxKind::ObjectLiteralExpression | SyntaxKind::ArrayLiteralExpression
            )
        })
}

/// TypeScript `IsArrayLiteralOrObjectLiteralDestructuringPattern`. This
/// narrower pattern question excludes for-in, compound assignments and
/// parenthesized wrappers admitted by GetAssignmentTarget.
pub(crate) fn is_literal_destructuring_pattern(
    ast: &Ast<'_>,
    parent_of: impl Fn(NodeId) -> Option<NodeId>,
    mut node: NodeId,
) -> bool {
    loop {
        if !matches!(
            ast.node(node).syntax_kind,
            SyntaxKind::ArrayLiteralExpression | SyntaxKind::ObjectLiteralExpression
        ) {
            return false;
        }
        let Some(parent) = parent_of(node) else {
            return false;
        };
        let syntax = ast.node(parent);
        if syntax.syntax_kind == SyntaxKind::BinaryExpression
            && id(syntax, "left") == Some(node)
            && id(syntax, "operatorToken")
                .is_some_and(|operator| ast.node(operator).syntax_kind == SyntaxKind::EqualsToken)
            || syntax.syntax_kind == SyntaxKind::ForOfStatement
                && id(syntax, "initializer") == Some(node)
        {
            return true;
        }
        node = if syntax.syntax_kind == SyntaxKind::PropertyAssignment {
            let Some(container) = parent_of(parent) else {
                return false;
            };
            container
        } else {
            parent
        };
    }
}

/// TypeScript `IsAssignmentOperator`: the complete pinned operator set.
pub(crate) fn is_assignment_operator(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::EqualsToken
            | SyntaxKind::PlusEqualsToken
            | SyntaxKind::MinusEqualsToken
            | SyntaxKind::AsteriskEqualsToken
            | SyntaxKind::AsteriskAsteriskEqualsToken
            | SyntaxKind::SlashEqualsToken
            | SyntaxKind::PercentEqualsToken
            | SyntaxKind::AmpersandEqualsToken
            | SyntaxKind::BarEqualsToken
            | SyntaxKind::CaretEqualsToken
            | SyntaxKind::LessThanLessThanEqualsToken
            | SyntaxKind::GreaterThanGreaterThanEqualsToken
            | SyntaxKind::GreaterThanGreaterThanGreaterThanEqualsToken
            | SyntaxKind::BarBarEqualsToken
            | SyntaxKind::AmpersandAmpersandEqualsToken
            | SyntaxKind::QuestionQuestionEqualsToken
    )
}

/// TypeScript `GetAssignmentTarget` / `IsAssignmentTarget`: climb only the
/// native target-preserving wrappers, including destructuring initializers.
pub(crate) fn assignment_target(
    ast: &Ast<'_>,
    parent_of: impl Fn(NodeId) -> Option<NodeId>,
    mut node: NodeId,
) -> Option<NodeId> {
    loop {
        let parent = parent_of(node)?;
        let syntax = ast.node(parent);
        match syntax.syntax_kind {
            SyntaxKind::BinaryExpression => return (id(syntax, "left") == Some(node) && id(syntax, "operatorToken").is_some_and(|id| is_assignment_operator(ast.node(id).syntax_kind))).then_some(parent),
            SyntaxKind::PrefixUnaryExpression | SyntaxKind::PostfixUnaryExpression => return matches!(number(syntax, "operator"), Some(value) if value == SyntaxKind::PlusPlusToken.number() || value == SyntaxKind::MinusMinusToken.number()).then_some(parent),
            SyntaxKind::ForInStatement | SyntaxKind::ForOfStatement => return (id(syntax, "initializer") == Some(node)).then_some(parent),
            SyntaxKind::ParenthesizedExpression | SyntaxKind::ArrayLiteralExpression | SyntaxKind::SpreadElement | SyntaxKind::NonNullExpression => node = parent,
            SyntaxKind::SpreadAssignment => node = parent_of(parent)?,
            SyntaxKind::PropertyAssignment => {
                if id(syntax, "name") == Some(node) { return None; }
                node = parent_of(parent)?;
            }
            SyntaxKind::ShorthandPropertyAssignment => {
                if id(syntax, "name") != Some(node) { return None; }
                node = parent_of(parent)?;
            }
            _ => return None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccessKind {
    Read,
    Write,
    ReadWrite,
}

/// TypeScript `accessKind` / `IsWriteOnlyAccess`: this parent-role question
/// differs from GetAssignmentTarget (notably logical assignment and names).
pub(crate) fn access_kind(
    ast: &Ast<'_>,
    parent_of: impl Fn(NodeId) -> Option<NodeId>,
    mut node: NodeId,
) -> AccessKind {
    let mut reverse = false;
    let result = loop {
        let Some(parent) = parent_of(node) else {
            break AccessKind::Read;
        };
        let syntax = ast.node(parent);
        match syntax.syntax_kind {
            SyntaxKind::ParenthesizedExpression | SyntaxKind::ArrayLiteralExpression => {
                node = parent
            }
            SyntaxKind::PrefixUnaryExpression | SyntaxKind::PostfixUnaryExpression => {
                break if matches!(number(syntax, "operator"), Some(value) if value == SyntaxKind::PlusPlusToken.number() || value == SyntaxKind::MinusMinusToken.number())
                {
                    AccessKind::ReadWrite
                } else {
                    AccessKind::Read
                };
            }
            SyntaxKind::BinaryExpression => {
                if id(syntax, "left") == Some(node)
                    && let Some(operator) = id(syntax, "operatorToken")
                {
                    let operator = ast.node(operator).syntax_kind;
                    if is_assignment_operator(operator) {
                        break if operator == SyntaxKind::EqualsToken {
                            AccessKind::Write
                        } else {
                            AccessKind::ReadWrite
                        };
                    }
                }
                break AccessKind::Read;
            }
            SyntaxKind::PropertyAccessExpression => {
                if id(syntax, "name") != Some(node) {
                    break AccessKind::Read;
                }
                node = parent;
            }
            SyntaxKind::PropertyAssignment => {
                reverse ^= id(syntax, "name") == Some(node);
                let Some(outer) = parent_of(parent) else {
                    break AccessKind::Read;
                };
                node = outer;
            }
            SyntaxKind::ShorthandPropertyAssignment => {
                if id(syntax, "objectAssignmentInitializer") == Some(node) {
                    break AccessKind::Read;
                }
                let Some(outer) = parent_of(parent) else {
                    break AccessKind::Read;
                };
                node = outer;
            }
            SyntaxKind::ForInStatement | SyntaxKind::ForOfStatement => {
                break if id(syntax, "initializer") == Some(node) {
                    AccessKind::Write
                } else {
                    AccessKind::Read
                };
            }
            _ => break AccessKind::Read,
        }
    };
    if reverse {
        match result {
            AccessKind::Read => AccessKind::Write,
            AccessKind::Write => AccessKind::Read,
            AccessKind::ReadWrite => result,
        }
    } else {
        result
    }
}

/// TypeScript's `JSDeclarationKind`: the declaration an assignment or
/// `Object.defineProperty` call may make (`GetAssignmentDeclarationKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssignmentDeclarationKind {
    None,
    /// `module.exports = value`.
    ModuleExports,
    /// `exports.name = value` or `module.exports.name = value`.
    ExportsProperty,
    /// `this.name = value`.
    ThisProperty,
    /// `entity.name = value` or `entity["name"] = value`.
    Property,
    /// `Object.defineProperty(entity, "name", descriptor)`.
    ObjectDefinePropertyValue,
    /// `Object.defineProperty(exports, "name", descriptor)`.
    ObjectDefinePropertyExports,
}

/// TypeScript `GetAssignmentDeclarationKind`, a syntax question: whether
/// the binder declares anything for it is the binder's. `javascript` is
/// `IsInJSFile`; outside JavaScript only a property assignment counts.
pub(crate) fn assignment_declaration_kind(
    ast: &Ast<'_>,
    node: NodeId,
    javascript: bool,
) -> AssignmentDeclarationKind {
    use AssignmentDeclarationKind as Kind;
    let syntax = ast.node(node);
    match syntax.syntax_kind {
        SyntaxKind::BinaryExpression => {
            let (Some(operator), Some(left), Some(right)) = (
                id(syntax, "operatorToken"),
                id(syntax, "left"),
                id(syntax, "right"),
            ) else {
                return Kind::None;
            };
            let access = ast.node(left).syntax_kind;
            if ast.node(operator).syntax_kind != SyntaxKind::EqualsToken
                || !matches!(
                    access,
                    SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
                )
            {
                return Kind::None;
            }
            let Some(receiver) = id(ast.node(left), "expression") else {
                return Kind::None;
            };
            let exports = |id: NodeId| {
                ast.node(id).syntax_kind == SyntaxKind::Identifier
                    && ast.identifier_name(id) == Some("exports")
            };
            if javascript {
                if super::is_module_exports_access_expression(ast, left) && !exports(right) {
                    return Kind::ModuleExports;
                }
                if (super::is_module_exports_access_expression(ast, receiver) || exports(receiver))
                    && super::element_or_property_access_name(ast, left).is_some()
                {
                    return Kind::ExportsProperty;
                }
                if ast.node(receiver).syntax_kind == SyntaxKind::ThisKeyword {
                    return Kind::ThisProperty;
                }
            }
            let entity = super::is_entity_name_expression_ex(ast, receiver, javascript);
            let identifier_name = id(ast.node(left), "name")
                .is_some_and(|name| ast.node(name).syntax_kind == SyntaxKind::Identifier);
            if entity
                && (access == SyntaxKind::ElementAccessExpression
                    || access == SyntaxKind::PropertyAccessExpression && identifier_name)
            {
                Kind::Property
            } else {
                Kind::None
            }
        }
        SyntaxKind::CallExpression
            if javascript && super::is_bindable_object_define_property_call(ast, node) =>
        {
            let target = super::ids(syntax, "arguments")[0];
            let exports = ast.node(target).syntax_kind == SyntaxKind::Identifier
                && ast.identifier_name(target) == Some("exports");
            if exports || super::is_module_exports_access_expression(ast, target) {
                Kind::ObjectDefinePropertyExports
            } else {
                Kind::ObjectDefinePropertyValue
            }
        }
        _ => Kind::None,
    }
}
