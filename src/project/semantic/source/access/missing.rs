//! Syntax facts for the complete missing-property report operation.
use super::super::{DeclarationIdentity, ExpressionRoot, Source};
use crate::project::semantic::model::DemandError;
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) struct MissingPropertySyntax {
    pub(in crate::project::semantic) name: String,
    pub(in crate::project::semantic) display_name: String,
    pub(in crate::project::semantic) private: bool,
    pub(in crate::project::semantic) access: Option<(ExpressionRoot, bool)>,
}
impl Source<'_, '_> {
    /// `getEntityNameForExtendingInterface` climbs only Identifier and
    /// PropertyAccessExpression parents. The heritage expression must itself
    /// be an EntityNameExpression; parentheses are not transparent here.
    pub(in crate::project::semantic) fn entity_name_for_extending_interface(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let mut node = captured.node_at(root.site)?;
        loop {
            match ast.node(node).syntax_kind {
                K::Identifier | K::PropertyAccessExpression => {
                    let Some(parent) = captured.parents[node.index()] else {
                        return Ok(None);
                    };
                    node = parent;
                }
                K::ExpressionWithTypeArguments => {
                    return Ok(syntax::id(ast.node(node), "expression")
                        .filter(|&expression| syntax::is_entity_name_expression(ast, expression))
                        .map(|expression| ExpressionRoot {
                            file: root.file,
                            site: crate::project::binding::flow::Site::new(ast, expression),
                        }));
                }
                _ => return Ok(None),
            }
        }
    }

    /// `reportNonexistentProperty` reads Text for lookup and the separate
    /// DeclarationNameToString question for feature suggestions. TS inputs do
    /// not admit JSDoc name-reference contexts.
    pub(in crate::project::semantic) fn missing_property_syntax(
        &mut self,
        node: DeclarationIdentity,
    ) -> Result<MissingPropertySyntax, DemandError> {
        let captured = self.syntax(node.file)?;
        let ast = &captured.ast;
        let parent = captured.parents[node.node.index()];
        let access = parent
            .filter(|&id| ast.node(id).syntax_kind == K::PropertyAccessExpression)
            .map(|id| {
                (
                    ExpressionRoot {
                        file: node.file,
                        site: crate::project::binding::flow::Site::new(ast, id),
                    },
                    syntax::id(ast.node(id), "expression")
                        .is_some_and(|left| ast.node(left).syntax_kind == K::SuperKeyword),
                )
            });
        let display_name =
            syntax::declaration_name_to_string(ast, Some(node.node), captured.control.as_ref());
        let private = ast.node(node.node).syntax_kind == K::PrivateIdentifier;
        let root = ExpressionRoot {
            file: node.file,
            site: crate::project::binding::flow::Site::new(ast, node.node),
        };
        let name = self.access_name_text(root)?;
        Ok(MissingPropertySyntax {
            name,
            display_name,
            private,
            access,
        })
    }
}
