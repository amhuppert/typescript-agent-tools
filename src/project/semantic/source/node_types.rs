//! Ordered syntax dispatch for the checker's GetTypeAtLocation question.
//! It deliberately does not pass through the product's Q4 selector.

use super::super::model::DemandError;
use super::{DeclarationIdentity, Expression, ExpressionRoot, Source, TypeExpression};
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) struct NodeTypeRecipe {
    pub(in crate::project::semantic) class: Option<DeclarationIdentity>,
    pub(in crate::project::semantic) form: NodeTypeForm,
}

pub(in crate::project::semantic) enum NodeTypeForm {
    Error,
    Type(TypeExpression),
    Expression(Expression),
    ClassBase,
    Declared(DeclarationIdentity),
    DeclaredName(DeclarationIdentity),
    VariableLike(DeclarationIdentity),
    Value(DeclarationIdentity),
    ValueName {
        declaration: DeclarationIdentity,
        import_property: bool,
    },
    ImportExportRhs,
    MetaKeyword(ExpressionRoot),
    ImportAttributes,
}

impl Source<'_, '_> {
    /// Syntax branches of `getTypeOfNode`, in compiler order. The enclosing
    /// class is acquired before a type or expression branch is evaluated.
    pub(in crate::project::semantic) fn node_type_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<NodeTypeRecipe, DemandError> {
        let inputs = self.inputs;
        let external = inputs.bindings.file_is_module(root.file);
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let data = ast.node(node);
        if data.syntax_kind == K::SourceFile && !external
            || data.flags & crate::compiler::node::IN_WITH_STATEMENT != 0
        {
            return Ok(NodeTypeRecipe {
                class: None,
                form: NodeTypeForm::Error,
            });
        }
        let class = syntax::class_of_heritage_element(ast, parents, node);
        let declaration = |node| DeclarationIdentity::new(root.file, node);
        let parent = parents[node.index()];
        let form = if syntax::is_part_of_type_node(ast, parents, node) {
            NodeTypeForm::Type(super::type_expression(
                inputs, root.file, captured, parents, node,
            ))
        } else if syntax::is_expression_node(ast, parents, node) {
            NodeTypeForm::Expression(super::expression(inputs, root.file, captured, node))
        } else if class.is_some_and(|(_, implements)| !implements) {
            NodeTypeForm::ClassBase
        } else if syntax::is_type_declaration(ast, parents, node) {
            NodeTypeForm::Declared(declaration(node))
        } else if syntax::is_type_declaration_name(ast, parents, node) {
            NodeTypeForm::DeclaredName(declaration(
                parent.expect("type name has declaration parent"),
            ))
        } else if data.syntax_kind == K::BindingElement {
            NodeTypeForm::VariableLike(declaration(node))
        } else if syntax::is_declaration(ast, |node| parents[node.index()], node) {
            NodeTypeForm::Value(declaration(node))
        } else if syntax::is_declaration_name_or_import_property_name(ast, parents, node) {
            let parent = parent.expect("declaration name has parent");
            NodeTypeForm::ValueName {
                declaration: declaration(parent),
                import_property: matches!(
                    ast.node(parent).syntax_kind,
                    K::ImportSpecifier | K::ExportSpecifier
                ) && syntax::id(ast.node(parent), "propertyName") == Some(node),
            }
        } else if matches!(
            data.syntax_kind,
            K::ObjectBindingPattern | K::ArrayBindingPattern
        ) {
            NodeTypeForm::VariableLike(declaration(
                parent.ok_or_else(|| super::super::internal("binding pattern has no owner"))?,
            ))
        } else if in_import_or_export_rhs(ast, parents, node) {
            NodeTypeForm::ImportExportRhs
        } else if parent.is_some_and(|parent| {
            ast.node(parent).syntax_kind == K::MetaProperty
                && syntax::number(ast.node(parent), "keywordToken")
                    == Some(data.syntax_kind.number())
        }) {
            NodeTypeForm::MetaKeyword(ExpressionRoot {
                file: root.file,
                site: crate::project::binding::flow::Site::new(
                    ast,
                    parent.expect("meta parent checked"),
                ),
            })
        } else if data.syntax_kind == K::ImportAttributes {
            NodeTypeForm::ImportAttributes
        } else {
            NodeTypeForm::Error
        };
        Ok(NodeTypeRecipe {
            class: class.map(|(class, _)| declaration(class)),
            form,
        })
    }
}

/// `isInRightSideOfImportOrExportAssignment` walks qualified names only;
/// expression parentheses and property accesses do not share this rule.
pub(super) fn in_import_or_export_rhs(
    ast: &crate::Ast<'_>,
    parents: &[Option<crate::NodeId>],
    mut node: crate::NodeId,
) -> bool {
    while let Some(parent) = parents[node.index()] {
        if ast.node(parent).syntax_kind != K::QualifiedName {
            break;
        }
        node = parent;
    }
    parents[node.index()].is_some_and(|parent| {
        let parent = ast.node(parent);
        parent.syntax_kind == K::ImportEqualsDeclaration
            && syntax::id(parent, "moduleReference") == Some(node)
            || parent.syntax_kind == K::ExportAssignment
                && syntax::id(parent, "expression") == Some(node)
    })
}
