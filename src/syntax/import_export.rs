//! Import/export syntax questions shared by reference capture and traversal.

use super::{id, walk_up_binding_elements_and_patterns};
use crate::{Ast, NodeId, syntax_kind::SyntaxKind as K};

/// `getSearchesFromDirectImports` reports the literal argument of an
/// unqualified import type for export-equals. It is a single reference, not
/// an ordinary `isValidReferencePosition` search candidate.
pub(crate) fn is_reference_import_type_literal(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    if ast.node(node).syntax_kind != K::StringLiteral {
        return false;
    }
    let Some(literal) = parents[node.index()] else {
        return false;
    };
    ast.node(literal).syntax_kind == K::LiteralType
        && id(ast.node(literal), "literal") == Some(node)
        && parents[literal.index()].is_some_and(|import| {
            ast.node(import).syntax_kind == K::ImportType
                && id(ast.node(import), "argument") == Some(literal)
        })
}

/// TypeScript `getExportNode`; a variable use must not inherit the export
/// modifier of the declaration that happens to contain its initializer.
pub(crate) fn reference_export_node(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    parent: NodeId,
    node: NodeId,
) -> Option<NodeId> {
    let declaration = match ast.node(parent).syntax_kind {
        K::VariableDeclaration => Some(parent),
        K::BindingElement => walk_up_binding_elements_and_patterns(ast, parents, parent),
        _ => return Some(parent),
    }?;
    let owner = parents[declaration.index()]?;
    let statement = parents[owner.index()]?;
    (id(ast.node(parent), "name") == Some(node)
        && ast.node(owner).syntax_kind != K::CatchClause
        && ast.node(statement).syntax_kind == K::VariableStatement)
        .then_some(statement)
}

/// TypeScript `isNodeImport` / `isExternalModuleImportEquals`. Renamed named
/// imports deliberately stop traversal toward their imported symbol.
pub(crate) fn is_reference_import(ast: &Ast<'_>, parent: NodeId, node: NodeId) -> bool {
    let p = ast.node(parent);
    match p.syntax_kind {
        K::ImportEqualsDeclaration => {
            id(p, "name") == Some(node)
                && id(p, "moduleReference").is_some_and(|reference| {
                    let reference = ast.node(reference);
                    reference.syntax_kind == K::ExternalModuleReference
                        && id(reference, "expression").is_some_and(|expression| {
                            ast.node(expression).syntax_kind == K::StringLiteral
                        })
                })
        }
        K::ImportSpecifier => id(p, "propertyName").is_none(),
        K::ImportClause | K::NamespaceImport => id(p, "name") == Some(node),
        _ => false,
    }
}

/// Product filter adaptation of `getImportOrExportSymbol` and
/// `getReferencesAtExportSpecifier`. These syntax roles can establish an
/// import/export edge even when their occurrences are filtered. This is only
/// an admission gate: native identity, meaning and direction still determine
/// whether traversal follows an edge. It never admits a general receiver use.
pub(crate) fn is_reference_traversal_header(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    let Some(parent) = parents[node.index()] else {
        return false;
    };
    let p = ast.node(parent);
    if p.syntax_kind == K::ExportAssignment
        || parents[parent.index()]
            .is_some_and(|owner| ast.node(owner).syntax_kind == K::ExportAssignment)
    {
        return true;
    }
    match p.syntax_kind {
        K::ImportSpecifier | K::ExportSpecifier => true,
        K::VariableDeclaration
        | K::BindingElement
        | K::FunctionDeclaration
        | K::ClassDeclaration
        | K::InterfaceDeclaration
        | K::TypeAliasDeclaration
        | K::EnumDeclaration
        | K::ModuleDeclaration
        | K::ImportClause
        | K::NamespaceImport
        | K::NamespaceExport
        | K::NamespaceExportDeclaration => id(p, "name") == Some(node),
        K::ImportEqualsDeclaration => {
            id(p, "name") == Some(node) || id(p, "moduleReference") == Some(node)
        }
        _ => false,
    }
}
