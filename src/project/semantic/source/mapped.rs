//! Native mapped declarations retain syntax identities; child types remain lazy.
use super::{
    AliasRecipe, DeclarationIdentity, ExpressionRoot, Source, TypeExpression, type_expression,
};
use crate::project::semantic::model::DemandError;
use crate::{syntax, syntax_kind::SyntaxKind as K};

#[derive(Clone)]
pub(in crate::project::semantic) struct MappedRecipe {
    pub declaration: DeclarationIdentity,
    pub parameter: DeclarationIdentity,
    pub parameter_root: ExpressionRoot,
    pub name: Option<TypeExpression>,
    pub template: Option<TypeExpression>,
    pub keyof_operand: Option<TypeExpression>,
    pub readonly: i8,
    pub optional: i8,
    pub readonly_token: Option<K>,
    pub question_token: Option<K>,
    pub alias: Option<AliasRecipe>,
}

impl Source<'_, '_> {
    /// `getConstraintDeclarationForMappedType` and `getMappedTypeModifiers`
    /// inspect the declaration, not a reduced constraint's semantic shape.
    pub(in crate::project::semantic) fn mapped_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<MappedRecipe, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let ast = &captured.ast;
        let node = ast.node(id);
        let parameter = syntax::id(node, "typeParameter")
            .ok_or_else(|| super::super::internal("mapped declaration has no parameter"))?;
        let expression = |id| type_expression(inputs, root.file, captured, &captured.parents, id);
        let modifier = |field| {
            syntax::id(node, field).map_or(0, |id| {
                if ast.node(id).syntax_kind == K::MinusToken {
                    -1
                } else {
                    1
                }
            })
        };
        let constraint = syntax::id(ast.node(parameter), "constraint");
        let keyof_operand = constraint
            .filter(|&id| {
                ast.node(id).syntax_kind == K::TypeOperator
                    && syntax::number(ast.node(id), "operator") == Some(K::KeyOfKeyword.number())
            })
            .and_then(|id| syntax::id(ast.node(id), "type"))
            .map(expression);
        Ok(MappedRecipe {
            declaration: DeclarationIdentity::new(root.file, id),
            parameter: DeclarationIdentity::new(root.file, parameter),
            parameter_root: ExpressionRoot {
                file: root.file,
                site: crate::project::binding::flow::Site::new(ast, parameter),
            },
            name: syntax::id(node, "nameType").map(expression),
            template: syntax::id(node, "type").map(expression),
            keyof_operand,
            readonly: modifier("readonlyToken"),
            optional: modifier("questionToken"),
            readonly_token: syntax::id(node, "readonlyToken").map(|id| ast.node(id).syntax_kind),
            question_token: syntax::id(node, "questionToken").map(|id| ast.node(id).syntax_kind),
            alias: super::alias_for_type_node(root.file, captured, &captured.parents, id),
        })
    }
}
