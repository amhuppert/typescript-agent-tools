//! Object literal syntax retains declaration identity and actual child roles.

use super::super::model::DemandError;
use super::{DeclarationIdentity, ExpressionRoot, Source};
use crate::project::binding::flow::Site;
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) struct ObjectLiteralRecipe {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) destructuring: bool,
    pub(in crate::project::semantic) members: Vec<ObjectMember>,
}

pub(in crate::project::semantic) struct ObjectMember {
    pub(in crate::project::semantic) root: ExpressionRoot,
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) kind: K,
    pub(in crate::project::semantic) name: Option<ExpressionRoot>,
    pub(in crate::project::semantic) computed: Option<ExpressionRoot>,
    pub(in crate::project::semantic) initializer: Option<ExpressionRoot>,
    pub(in crate::project::semantic) annotation: Option<ExpressionRoot>,
    pub(in crate::project::semantic) default_value: bool,
    pub(in crate::project::semantic) stops_grammar: bool,
}

impl Source<'_, '_> {
    /// `checkObjectLiteral`, `checkPropertyAssignment`, and
    /// `checkShorthandPropertyAssignment` use the actual declaration and
    /// distinguish shorthand defaults from ordinary initializers.
    pub(in crate::project::semantic) fn object_literal_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ObjectLiteralRecipe, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let destructuring =
            syntax::assignment_target(ast, |node| captured.parents[node.index()], id).is_some();
        let mut members = Vec::new();
        for &id in syntax::ids(ast.node(id), "properties") {
            members.push(object_member(root.file, ast, id, destructuring));
        }
        Ok(ObjectLiteralRecipe {
            declaration: DeclarationIdentity::new(root.file, id),
            destructuring,
            members,
        })
    }

    /// The declaration branch of `getTypeOfVariableOrParameterOrPropertyWorker`
    /// asks the same `checkPropertyAssignment` / shorthand syntax question as
    /// `checkObjectLiteral`, without acquiring unrelated sibling properties.
    pub(in crate::project::semantic) fn object_member_value_recipe(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<ObjectMember, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        if !matches!(
            ast.node(declaration.node).syntax_kind,
            K::PropertyAssignment | K::ShorthandPropertyAssignment | K::MethodDeclaration
        ) {
            return Err(super::super::internal(
                "object member value recipe has another declaration kind",
            )
            .into());
        }
        Ok(object_member(
            declaration.file,
            ast,
            declaration.node,
            false,
        ))
    }

    /// `isPossiblyDiscriminantValue` never follows an element-access or an
    /// arbitrary unary expression. This limits contextual recursion exactly
    /// where the native discriminator asks the question.
    pub(in crate::project::semantic) fn possibly_discriminant_value(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let mut node = captured.node_at(root.site)?;
        loop {
            let data = ast.node(node);
            match data.syntax_kind {
                K::StringLiteral
                | K::NumericLiteral
                | K::BigIntLiteral
                | K::NoSubstitutionTemplateLiteral
                | K::TemplateExpression
                | K::TrueKeyword
                | K::FalseKeyword
                | K::NullKeyword
                | K::Identifier
                | K::UndefinedKeyword => return Ok(true),
                K::PropertyAccessExpression | K::ParenthesizedExpression | K::JsxExpression => {
                    match syntax::id(data, "expression") {
                        Some(inner) => node = inner,
                        None => return Ok(data.syntax_kind == K::JsxExpression),
                    }
                }
                _ => return Ok(false),
            }
        }
    }
}

fn object_member(
    file: crate::project::sources::FileId,
    ast: &crate::Ast<'_>,
    id: crate::NodeId,
    destructuring: bool,
) -> ObjectMember {
    let at = |node| ExpressionRoot {
        file,
        site: Site::new(ast, node),
    };
    let node = ast.node(id);
    let name = syntax::id(node, "name");
    let computed = name
        .filter(|&name| ast.node(name).syntax_kind == K::ComputedPropertyName)
        .and_then(|name| syntax::id(ast.node(name), "expression"));
    let initializer = match node.syntax_kind {
        K::SpreadAssignment => syntax::id(node, "expression"),
        K::ShorthandPropertyAssignment => syntax::id(node, "objectAssignmentInitializer"),
        _ => syntax::id(node, "initializer"),
    };
    let stops_grammar = destructuring
        && node.syntax_kind == K::SpreadAssignment
        && initializer.is_some_and(|mut expression| {
            while ast.node(expression).syntax_kind == K::ParenthesizedExpression {
                let Some(inner) = syntax::id(ast.node(expression), "expression") else {
                    break;
                };
                expression = inner;
            }
            matches!(
                ast.node(expression).syntax_kind,
                K::ObjectLiteralExpression | K::ArrayLiteralExpression
            )
        });
    ObjectMember {
        root: at(id),
        declaration: DeclarationIdentity::new(file, id),
        kind: node.syntax_kind,
        name: name.map(at),
        computed: computed.map(at),
        initializer: initializer.map(at),
        annotation: syntax::id(node, "type").map(at),
        default_value: syntax::has_default_value(ast, id),
        stops_grammar,
    }
}
