//! Native declaration predicates. DeclarationData, type declarations, and
//! declaration names are separate grammar questions over one final tree.

use super::{flag, id};
use crate::{Ast, NodeId, syntax_kind::SyntaxKind};

/// TypeScript `HasInferredType`: these declaration kinds participate in the
/// syntactic type serializer. This does not ask whether an annotation exists.
pub(crate) fn has_inferred_type(kind: SyntaxKind) -> bool {
    use SyntaxKind::*;
    matches!(
        kind,
        Parameter
            | PropertySignature
            | PropertyDeclaration
            | BindingElement
            | PropertyAccessExpression
            | ElementAccessExpression
            | BinaryExpression
            | CallExpression
            | VariableDeclaration
            | ExportAssignment
            | PropertyAssignment
            | ShorthandPropertyAssignment
            | JSDocParameterTag
            | JSDocPropertyTag
    )
}

/// TypeScript `CanHaveSymbol`: the node data has an actual Symbol slot.
/// This differs from IsDeclaration (notably for access and literal nodes).
pub(crate) fn can_have_symbol(kind: SyntaxKind) -> bool {
    use SyntaxKind::*;
    matches!(
        kind,
        ArrowFunction
            | BinaryExpression
            | BindingElement
            | CallExpression
            | CallSignature
            | ClassDeclaration
            | ClassExpression
            | ClassStaticBlockDeclaration
            | Constructor
            | ConstructorType
            | ConstructSignature
            | ElementAccessExpression
            | EnumDeclaration
            | EnumMember
            | ExportAssignment
            | ExportDeclaration
            | ExportSpecifier
            | FunctionDeclaration
            | FunctionExpression
            | FunctionType
            | GetAccessor
            | ImportClause
            | ImportEqualsDeclaration
            | ImportSpecifier
            | IndexSignature
            | InterfaceDeclaration
            | JsxAttribute
            | JsxAttributes
            | JsxSpreadAttribute
            | MappedType
            | MethodDeclaration
            | MethodSignature
            | ModuleDeclaration
            | NamedTupleMember
            | NamespaceExport
            | NamespaceExportDeclaration
            | NamespaceImport
            | NewExpression
            | NoSubstitutionTemplateLiteral
            | NumericLiteral
            | ObjectLiteralExpression
            | Parameter
            | PropertyAccessExpression
            | PropertyAssignment
            | PropertyDeclaration
            | PropertySignature
            | SetAccessor
            | ShorthandPropertyAssignment
            | SourceFile
            | SpreadAssignment
            | StringLiteral
            | TypeAliasDeclaration
            | TypeLiteral
            | TypeParameter
            | VariableDeclaration
    )
}

/// TypeScript `ModuleExportNameIsDefault`: compare the exact stored name,
/// including a literal's UTF-16 payload, without converting it to identity.
pub(crate) fn module_export_name_is_default(node: &crate::Node<'_>) -> bool {
    match node.field("escapedText").or_else(|| node.field("text")) {
        Some(crate::Value::Text(text)) => *text == "default",
        Some(crate::Value::Utf16(units)) => units.iter().copied().eq("default".encode_utf16()),
        _ => false,
    }
}

/// TypeScript `WalkUpBindingElementsAndPatterns`: starting at an element,
/// return the declaration that owns its outermost binding pattern.
pub(crate) fn walk_up_binding_elements_and_patterns(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    binding: NodeId,
) -> Option<NodeId> {
    let mut pattern = parents[binding.index()]?;
    while let Some(element) = parents[pattern.index()]
        && ast.node(element).syntax_kind == SyntaxKind::BindingElement
    {
        pattern = parents[element.index()]?;
    }
    parents[pattern.index()]
}

/// TypeScript `IsDeclaration` / `IsDeclarationNode`, including anonymous
/// DeclarationData-bearing nodes. Detached TypeParameters are the exception.
pub(crate) fn is_declaration(
    ast: &Ast<'_>,
    parent: impl FnOnce(NodeId) -> Option<NodeId>,
    node: NodeId,
) -> bool {
    use SyntaxKind::*;
    match ast.node(node).syntax_kind {
        TypeParameter => parent(node).is_some(),
        SourceFile
        | VariableDeclaration
        | Parameter
        | BindingElement
        | MissingDeclaration
        | FunctionDeclaration
        | ClassDeclaration
        | ClassExpression
        | InterfaceDeclaration
        | TypeAliasDeclaration
        | EnumMember
        | EnumDeclaration
        | ImportDeclaration
        | NamespaceImport
        | ExportAssignment
        | NamespaceExportDeclaration
        | NamespaceExport
        | ExportSpecifier
        | CallSignature
        | ConstructSignature
        | Constructor
        | GetAccessor
        | SetAccessor
        | IndexSignature
        | MethodSignature
        | MethodDeclaration
        | PropertySignature
        | PropertyDeclaration
        | SemicolonClassElement
        | ClassStaticBlockDeclaration
        | NoSubstitutionTemplateLiteral
        | BinaryExpression
        | ArrowFunction
        | FunctionExpression
        | CallExpression
        | ObjectLiteralExpression
        | SpreadAssignment
        | PropertyAssignment
        | ShorthandPropertyAssignment
        | MappedType
        | TypeLiteral
        | NamedTupleMember
        | FunctionType
        | ConstructorType
        | JsxAttributes
        | JsxAttribute
        | JSDocSignature
        | ModuleDeclaration
        | ImportEqualsDeclaration
        | ExportDeclaration
        | ImportClause
        | ImportSpecifier
        | JSDocTypeLiteral => true,
        _ => false,
    }
}

/// TypeScript `IsDeclarationName`: identity is the actual Name field; catalog
/// visibility and binder declaration spaces ask different questions.
pub(crate) fn is_declaration_name(
    ast: &Ast<'_>,
    mut parent: impl FnMut(NodeId) -> Option<NodeId>,
    node: NodeId,
) -> bool {
    if matches!(
        ast.node(node).syntax_kind,
        SyntaxKind::SourceFile | SyntaxKind::ObjectBindingPattern | SyntaxKind::ArrayBindingPattern
    ) {
        return false;
    }
    let Some(owner) = parent(node) else {
        return false;
    };
    is_declaration(ast, &mut parent, owner) && id(ast.node(owner), "name") == Some(node)
}

/// TypeScript `IsDeclarationNameOrImportPropertyName`. Specifier property
/// names are included even though they are not the declaration's Name field.
pub(crate) fn is_declaration_name_or_import_property_name(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    let Some(parent) = parents[node.index()] else {
        return false;
    };
    if matches!(
        ast.node(parent).syntax_kind,
        SyntaxKind::ImportSpecifier | SyntaxKind::ExportSpecifier
    ) {
        matches!(
            ast.node(node).syntax_kind,
            SyntaxKind::Identifier | SyntaxKind::StringLiteral
        )
    } else {
        is_declaration_name(ast, |node| parents[node.index()], node)
    }
}

/// TypeScript `IsTypeDeclaration`: a specifier inherits the enclosing clause's
/// IsTypeOnly bit. An individual `type` specifier does not change this rule.
pub(crate) fn is_type_declaration(ast: &Ast<'_>, parents: &[Option<NodeId>], node: NodeId) -> bool {
    use SyntaxKind::*;
    match ast.node(node).syntax_kind {
        TypeParameter | ClassDeclaration | InterfaceDeclaration | TypeAliasDeclaration
        | EnumDeclaration => true,
        ImportClause => flag(ast.node(node), "isTypeOnly"),
        ImportSpecifier | ExportSpecifier => parents[node.index()]
            .and_then(|parent| parents[parent.index()])
            .is_some_and(|owner| flag(ast.node(owner), "isTypeOnly")),
        _ => false,
    }
}

/// TypeScript `IsTypeDeclarationName` only accepts identifier names.
pub(crate) fn is_type_declaration_name(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    ast.node(node).syntax_kind == SyntaxKind::Identifier
        && parents[node.index()].is_some_and(|parent| {
            is_type_declaration(ast, parents, parent) && id(ast.node(parent), "name") == Some(node)
        })
}

/// TypeScript `TryGetClassImplementingOrExtendingExpressionWithTypeArguments`.
pub(crate) fn class_of_heritage_element(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> Option<(NodeId, bool)> {
    if ast.node(node).syntax_kind != SyntaxKind::ExpressionWithTypeArguments {
        return None;
    }
    let clause = parents[node.index()]?;
    if ast.node(clause).syntax_kind != SyntaxKind::HeritageClause {
        return None;
    }
    let class = parents[clause.index()]?;
    matches!(
        ast.node(class).syntax_kind,
        SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
    )
    .then(|| {
        (
            class,
            super::number(ast.node(clause), "token")
                == Some(SyntaxKind::ImplementsKeyword.number()),
        )
    })
}
