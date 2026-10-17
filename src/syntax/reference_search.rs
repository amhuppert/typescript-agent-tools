//! Native syntax questions shared by reference capture and semantic search.
//! Decoded name identity and semantic meaning are separate decisions.

use super::{id, ids};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId};

/// TypeScript `getRangeOfNode`. The scanner/parser owns the node's nontrivia
/// token start; the optional end node keeps its native full span. Definition
/// and selection spans ask different questions and do not use this operation.
pub(crate) fn reference_occurrence_range(
    kind: SyntaxKind,
    token_span: crate::Span,
    end_node: Option<(SyntaxKind, crate::Span)>,
) -> crate::Span {
    let mut start = token_span.start;
    let mut end = end_node.map_or(token_span.end, |(_, span)| span.end);
    if matches!(
        kind,
        SyntaxKind::StringLiteral | SyntaxKind::NoSubstitutionTemplateLiteral
    ) && end - start > 2
    {
        assert!(
            end_node.is_none(),
            "native string reference cannot have an end node"
        );
        start += 1;
        end -= 1;
    }
    if let Some((SyntaxKind::CaseBlock, span)) = end_node {
        end = span.start;
    }
    crate::Span { start, end }
}

/// Product replay anchor for a parameter property's member role. The native
/// `isParameterPropertyDeclaration` owns eligibility; the first contributing
/// modifier selects the member while the Name keeps its lexical precedence.
pub(crate) fn parameter_property_reference_anchor(
    ast: &Ast<'_>,
    mut parent: impl FnMut(NodeId) -> Option<NodeId>,
    declaration: NodeId,
) -> Option<NodeId> {
    let node = ast.node(declaration);
    if node.syntax_kind != SyntaxKind::Parameter
        || !super::is_parameter_property_declaration(ast, node, ast.node(parent(declaration)?))
    {
        return None;
    }
    ids(node, "modifiers").iter().copied().find(|&modifier| {
        matches!(
            ast.node(modifier).syntax_kind,
            SyntaxKind::PublicKeyword
                | SyntaxKind::PrivateKeyword
                | SyntaxKind::ProtectedKeyword
                | SyntaxKind::ReadonlyKeyword
                | SyntaxKind::OverrideKeyword
        )
    })
}

/// TypeScript `isValidReferencePosition`, after the inventory has matched the
/// exact decoded search key. The product's decoded-key index replaces the
/// compiler's raw-text/length prefilter; all native syntax gates remain here.
pub(crate) fn is_valid_reference_position(
    ast: &Ast<'_>,
    mut parent: impl FnMut(NodeId) -> Option<NodeId>,
    node: NodeId,
) -> bool {
    use SyntaxKind::*;
    match ast.node(node).syntax_kind {
        Identifier | PrivateIdentifier | DefaultKeyword => true,
        StringLiteral | NoSubstitutionTemplateLiteral => {
            if is_literal_name_of_property_declaration_or_index_access(ast, &mut parent, node) {
                return true;
            }
            let Some(owner) = parent(node) else {
                return false;
            };
            let owner_node = ast.node(owner);
            match owner_node.syntax_kind {
                ModuleDeclaration => id(owner_node, "name") == Some(node),
                ExternalModuleReference => {
                    id(owner_node, "expression") == Some(node)
                        && parent(owner).is_some_and(|declaration| {
                            let declaration = ast.node(declaration);
                            declaration.syntax_kind == ImportEqualsDeclaration
                                && id(declaration, "moduleReference") == Some(owner)
                        })
                }
                CallExpression => {
                    is_bindable_object_define_property_call(ast, owner)
                        && ids(owner_node, "arguments")[1] == node
                }
                ImportSpecifier | ExportSpecifier => true,
                _ => false,
            }
        }
        NumericLiteral => {
            is_literal_name_of_property_declaration_or_index_access(ast, parent, node)
        }
        _ => false,
    }
}

/// TypeScript `isLiteralNameOfPropertyDeclarationOrIndexAccess`. In particular,
/// either literal type directly under an IndexedAccessType qualifies: the
/// compiler does not restrict this question to its indexType slot.
fn is_literal_name_of_property_declaration_or_index_access(
    ast: &Ast<'_>,
    mut parent: impl FnMut(NodeId) -> Option<NodeId>,
    node: NodeId,
) -> bool {
    use SyntaxKind::*;
    let Some(owner) = parent(node) else {
        return false;
    };
    match ast.node(owner).syntax_kind {
        PropertyDeclaration | PropertySignature | PropertyAssignment | EnumMember
        | MethodDeclaration | MethodSignature | GetAccessor | SetAccessor | ModuleDeclaration => {
            id(ast.node(owner), "name") == Some(node)
        }
        ElementAccessExpression => id(ast.node(owner), "argumentExpression") == Some(node),
        ComputedPropertyName => true,
        LiteralType => parent(owner)
            .is_some_and(|ancestor| ast.node(ancestor).syntax_kind == IndexedAccessType),
        _ => false,
    }
}

/// TypeScript `IsStringOrNumericLiteralLike`, including a template with no
/// substitutions. Signed numbers are expressions, not this literal family.
pub(crate) fn is_string_or_numeric_literal_like(ast: &Ast<'_>, node: NodeId) -> bool {
    matches!(
        ast.node(node).syntax_kind,
        SyntaxKind::StringLiteral
            | SyntaxKind::NoSubstitutionTemplateLiteral
            | SyntaxKind::NumericLiteral
    )
}

/// TypeScript `IsBindableStaticNameExpression` and its static property/element
/// access recursion. The initial access alone may admit a this receiver; all
/// recursive receiver questions exclude it. An explicit walk keeps deep,
/// otherwise valid access chains off the Rust call stack.
pub(crate) fn is_bindable_static_name_expression(
    ast: &Ast<'_>,
    mut node: NodeId,
    mut exclude_this: bool,
) -> bool {
    use SyntaxKind::*;
    loop {
        let current = ast.node(node);
        let receiver = match current.syntax_kind {
            Identifier => return true,
            PropertyAccessExpression => {
                let Some(receiver) = id(current, "expression") else {
                    return false;
                };
                if !exclude_this && ast.node(receiver).syntax_kind == ThisKeyword {
                    return true;
                }
                if !id(current, "name").is_some_and(|name| ast.node(name).syntax_kind == Identifier)
                {
                    return false;
                }
                receiver
            }
            ElementAccessExpression => {
                if !id(current, "argumentExpression")
                    .is_some_and(|argument| is_string_or_numeric_literal_like(ast, argument))
                {
                    return false;
                }
                let Some(receiver) = id(current, "expression") else {
                    return false;
                };
                if !exclude_this && ast.node(receiver).syntax_kind == ThisKeyword {
                    return true;
                }
                receiver
            }
            _ => return false,
        };
        node = receiver;
        exclude_this = true;
    }
}

/// TypeScript `IsBindableObjectDefinePropertyCall`. Spelling alone is not
/// enough: arity, callee shape, key kind and static receiver all participate.
pub(crate) fn is_bindable_object_define_property_call(ast: &Ast<'_>, node: NodeId) -> bool {
    let node = ast.node(node);
    if node.syntax_kind != SyntaxKind::CallExpression {
        return false;
    }
    let arguments = ids(node, "arguments");
    let Some(expression) = id(node, "expression") else {
        return false;
    };
    let expression = ast.node(expression);
    arguments.len() == 3
        && expression.syntax_kind == SyntaxKind::PropertyAccessExpression
        && id(expression, "expression").is_some_and(|receiver| {
            ast.node(receiver).syntax_kind == SyntaxKind::Identifier
                && ast.identifier_name(receiver) == Some("Object")
        })
        && id(expression, "name")
            .is_some_and(|name| ast.identifier_name(name) == Some("defineProperty"))
        && is_string_or_numeric_literal_like(ast, arguments[1])
        && is_bindable_static_name_expression(ast, arguments[0], true)
}
