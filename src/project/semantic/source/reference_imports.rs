//! Syntax questions for native reference import/export traversal. The search
//! owns traversal direction and symbol comparisons; these recipes retain the
//! actual nodes on which checker operations must be performed.

use super::super::model::DemandError;
use super::{DeclarationIdentity, ExpressionRoot, ReferenceSite, ReferenceSiteTarget, Source};
use crate::project::binding::flow::Site;
use crate::syntax::{self, id};
use crate::syntax_kind::SyntaxKind as K;
use crate::{Ast, NodeId};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::project::semantic) enum ReferenceExportKind {
    Named,
    Default,
    Equals,
}

pub(in crate::project::semantic) enum ReferenceExportSyntax {
    None,
    Declaration {
        kind: ReferenceExportKind,
        import_equals_lhs: Option<ExpressionRoot>,
    },
    Assignment {
        declaration: DeclarationIdentity,
        kind: ReferenceExportKind,
    },
}

pub(in crate::project::semantic) struct ReferenceImportExportSyntax {
    pub(in crate::project::semantic) property_access: bool,
    pub(in crate::project::semantic) parent_export_kind: ReferenceExportKind,
    pub(in crate::project::semantic) export: ReferenceExportSyntax,
    pub(in crate::project::semantic) import: bool,
}

pub(in crate::project::semantic) struct ReferenceExportSpecifier {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) name: ExpressionRoot,
    pub(in crate::project::semantic) renamed: bool,
    pub(in crate::project::semantic) at_property: bool,
    pub(in crate::project::semantic) has_module: bool,
    pub(in crate::project::semantic) kind: ReferenceExportKind,
}

impl Source<'_, '_> {
    /// `getExportEqualsLocalSymbol`'s raw node-symbol sites. No symbol or
    /// expression type is acquired by this syntax question.
    pub(in crate::project::semantic) fn reference_export_equals_symbol_node(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        let target = match node.syntax_kind {
            K::ExportAssignment => id(node, "expression"),
            K::BinaryExpression => id(node, "right"),
            K::SourceFile => Some(declaration.node),
            _ => None,
        };
        Ok(target.map(|target| ExpressionRoot {
            file: declaration.file,
            site: Site::new(&captured.ast, target),
        }))
    }

    pub(in crate::project::semantic) fn reference_node_site(
        &mut self,
        inventory: &crate::project::binding::Bindings,
        root: ExpressionRoot,
    ) -> Result<Option<ReferenceSite>, DemandError> {
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        let offset = captured.ast.token_span(node).start;
        let token = inventory.reference_token_at(root.file, offset);
        token
            .map(|token| self.reference_site(inventory.reference_token_facts(token)))
            .transpose()
    }

    /// `getImportOrExportSymbol`'s syntactic export cases, `getExportNode`, and
    /// `isNodeImport`. ExportSymbol is queried first by the caller; merely
    /// collecting these facts never acquires an alias or a value type.
    pub(in crate::project::semantic) fn reference_import_export_syntax(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<ReferenceImportExportSyntax>, DemandError> {
        let ReferenceSiteTarget::Captured { root, .. } = site.target else {
            return Ok(None);
        };
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let node = captured.node_at(root.site)?;
        let Some(parent) = parents[node.index()] else {
            return Ok(None);
        };
        let p = ast.node(parent);
        let grandparent = parents[parent.index()];
        let export_node = syntax::reference_export_node(ast, parents, parent, node);
        let export = if let Some(export) = export_node
            && syntax::has_modifier(ast, ast.node(export), K::ExportKeyword)
        {
            let import_equals_lhs = (ast.node(export).syntax_kind == K::ImportEqualsDeclaration
                && id(ast.node(export), "moduleReference") == Some(node))
            .then(|| id(ast.node(export), "name"))
            .flatten()
            .map(|name| ExpressionRoot {
                file: root.file,
                site: Site::new(ast, name),
            });
            ReferenceExportSyntax::Declaration {
                kind: export_kind(ast, export),
                import_equals_lhs,
            }
        } else if p.syntax_kind == K::NamespaceExport {
            ReferenceExportSyntax::Declaration {
                kind: ReferenceExportKind::Named,
                import_equals_lhs: None,
            }
        } else if let Some(assignment) = Some(parent)
            .filter(|&n| ast.node(n).syntax_kind == K::ExportAssignment)
            .or_else(|| grandparent.filter(|&n| ast.node(n).syntax_kind == K::ExportAssignment))
        {
            ReferenceExportSyntax::Assignment {
                declaration: DeclarationIdentity::new(root.file, assignment),
                kind: if syntax::flag(ast.node(assignment), "isExportEquals") {
                    ReferenceExportKind::Equals
                } else {
                    ReferenceExportKind::Default
                },
            }
        } else {
            ReferenceExportSyntax::None
        };
        Ok(Some(ReferenceImportExportSyntax {
            property_access: p.syntax_kind == K::PropertyAccessExpression,
            parent_export_kind: export_kind(ast, parent),
            export,
            import: syntax::is_reference_import(ast, parent, node),
        }))
    }

    /// `getReferencesAtExportSpecifier` and `getLocalSymbolForExportSpecifier`
    /// use exact property/name roles, independently of decoded name equality.
    pub(in crate::project::semantic) fn reference_export_specifier(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<ReferenceExportSpecifier>, DemandError> {
        let ReferenceSiteTarget::Captured { root, .. } = site.target else {
            return Ok(None);
        };
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = captured.node_at(root.site)?;
        let Some(specifier) = captured.parents[node.index()]
            .filter(|&parent| ast.node(parent).syntax_kind == K::ExportSpecifier)
        else {
            return Ok(None);
        };
        let declaration = captured.parents[specifier.index()]
            .and_then(|clause| captured.parents[clause.index()])
            .ok_or_else(|| super::super::internal("export specifier lacks its declaration"))?;
        let name = id(ast.node(specifier), "name")
            .ok_or_else(|| super::super::internal("export specifier lacks its name"))?;
        let property = id(ast.node(specifier), "propertyName");
        Ok(Some(ReferenceExportSpecifier {
            declaration: DeclarationIdentity::new(root.file, specifier),
            name: ExpressionRoot {
                file: root.file,
                site: Site::new(ast, name),
            },
            renamed: property.is_some(),
            at_property: property == Some(node),
            has_module: id(ast.node(declaration), "moduleSpecifier").is_some(),
            kind: if syntax::module_export_name_is_default(ast.node(node))
                || syntax::module_export_name_is_default(ast.node(name))
            {
                ReferenceExportKind::Default
            } else {
                ReferenceExportKind::Named
            },
        }))
    }

    /// The TypeScript-only branch of `skipExportSpecifierSymbol`.
    pub(in crate::project::semantic) fn reference_is_unrenamed_local_export(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok(node.syntax_kind == K::ExportSpecifier
            && id(node, "propertyName").is_none()
            && captured.parents[declaration.node.index()]
                .and_then(|clause| captured.parents[clause.index()])
                .is_some_and(|export| id(captured.ast.node(export), "moduleSpecifier").is_none()))
    }

    /// `symbolNameNoDefault` inspects Identifier declaration names only. This
    /// is not the broader property-text operation used by contextual objects.
    pub(in crate::project::semantic) fn reference_identifier_declaration_name(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<String>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        id(captured.ast.node(declaration.node), "name")
            .filter(|&name| captured.ast.node(name).syntax_kind == K::Identifier)
            .map(|name| exact_text(inputs, declaration.file, &captured.ast, name))
            .transpose()
    }
}

fn export_kind(ast: &Ast<'_>, node: NodeId) -> ReferenceExportKind {
    if syntax::has_modifier(ast, ast.node(node), K::DefaultKeyword) {
        ReferenceExportKind::Default
    } else {
        ReferenceExportKind::Named
    }
}

/// Native `Node.Text` for identity-bearing identifiers and literal names.
/// Non-scalar payloads are a located unsupported identity, never lossy text.
pub(super) fn exact_text(
    inputs: crate::project::semantic::inputs::CheckerInputs<'_>,
    file: crate::project::sources::FileId,
    ast: &Ast<'_>,
    node: NodeId,
) -> Result<String, DemandError> {
    let span = ast.token_span(node);
    let node = ast.node(node);
    match super::property_names::scalar_name_text(ast, node) {
        Ok(Some(text)) => Ok(text.to_owned()),
        _ => Err(super::super::unsupported_demand_at_location(
            inputs,
            &super::super::location(inputs, file, span),
            crate::analysis::SemanticFailureReason::UnrepresentableIdentity,
            "reference name has no exact scalar identity",
        )),
    }
}
