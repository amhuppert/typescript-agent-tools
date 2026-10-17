//! Captured enum syntax; evaluation owns value and declared-type caches.
use super::super::model::DemandError;
use super::{DeclarationIdentity, Expression, Source, expression};
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) struct EnumDeclarationRecipe {
    pub(in crate::project::semantic) members: Vec<DeclarationIdentity>,
    pub(in crate::project::semantic) ambient: bool,
    pub(in crate::project::semantic) constant: bool,
}

pub(in crate::project::semantic) struct EnumMemberRecipe {
    pub(in crate::project::semantic) parent: DeclarationIdentity,
    pub(in crate::project::semantic) initializer: Option<Expression>,
}

impl Source<'_, '_> {
    /// `computeEnumMemberValues` / `computeEnumMemberValue`: declaration order,
    /// actual parent flags and optional initializer come from the captured AST.
    pub(in crate::project::semantic) fn enum_declaration_recipe(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<EnumDeclarationRecipe, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if node.syntax_kind != K::EnumDeclaration {
            return Err(
                super::super::internal("enum recipe has a different declaration kind").into(),
            );
        }
        Ok(EnumDeclarationRecipe {
            members: syntax::ids(node, "members")
                .iter()
                .map(|&node| DeclarationIdentity {
                    file: declaration.file,
                    node,
                })
                .collect(),
            ambient: node.flags & crate::compiler::node::AMBIENT != 0,
            constant: syntax::has_modifier(ast, node, K::ConstKeyword),
        })
    }

    pub(in crate::project::semantic) fn enum_member_recipe(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<EnumMemberRecipe, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        if node.syntax_kind != K::EnumMember {
            return Err(super::super::internal(
                "enum member value has a different declaration kind",
            )
            .into());
        }
        let parent = captured.parents[declaration.node.index()]
            .ok_or_else(|| super::super::internal("enum member lacks actual parent"))?;
        Ok(EnumMemberRecipe {
            parent: DeclarationIdentity {
                file: declaration.file,
                node: parent,
            },
            initializer: syntax::id(node, "initializer")
                .map(|id| expression(inputs, declaration.file, captured, id)),
        })
    }
}
