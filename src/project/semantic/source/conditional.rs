//! Syntax facts for conditional roots and their actual enclosing type context.
use super::{
    AliasRecipe, DeclarationIdentity, ExpressionRoot, Source, TypeExpression, type_expression,
};
use crate::project::binding::flow::Site;
use crate::project::semantic::model::DemandError;
use crate::{Ast, NodeId, syntax, syntax_kind::SyntaxKind as K};

#[derive(Clone)]
pub(in crate::project::semantic) struct ConditionalRecipe {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) ordering: crate::project::binding::NativeDeclaration,
    pub(in crate::project::semantic) parent: Option<DeclarationIdentity>,
    pub(in crate::project::semantic) check: TypeExpression,
    pub(in crate::project::semantic) extends: TypeExpression,
    pub(in crate::project::semantic) true_type: TypeExpression,
    pub(in crate::project::semantic) false_type: TypeExpression,
    pub(in crate::project::semantic) alias: Option<AliasRecipe>,
    pub(in crate::project::semantic) check_tuples: bool,
}

pub(in crate::project::semantic) enum ConditionalFlowStep {
    Constraint {
        covariant: bool,
        check: Box<TypeExpression>,
        extends: Box<TypeExpression>,
    },
    Mapped {
        declaration: ExpressionRoot,
    },
}

pub(in crate::project::semantic) enum InferredConstraint {
    Reference {
        node: Box<TypeExpression>,
        index: usize,
    },
    Rest,
    String,
    Keys,
    Mapped {
        template: Box<TypeExpression>,
        parameter: DeclarationIdentity,
        constraint: Option<Box<TypeExpression>>,
    },
}

impl Source<'_, '_> {
    /// Actual captured node identity; this is the inverse projection of
    /// declaration_expression_root, not a declaration-role lookup.
    pub(in crate::project::semantic) fn declaration_from_expression_root(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<DeclarationIdentity, DemandError> {
        let captured = self.syntax(root.file)?;
        Ok(DeclarationIdentity::new(
            root.file,
            captured.node_at(root.site)?,
        ))
    }

    pub(in crate::project::semantic) fn declaration_expression_root(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<ExpressionRoot, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(ExpressionRoot {
            file: declaration.file,
            site: Site::new(&captured.ast, declaration.node),
        })
    }

    pub(in crate::project::semantic) fn parameter_declaration_children(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<(String, Option<TypeExpression>, Option<TypeExpression>), DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        let name = syntax::id(node, "name")
            .ok_or_else(|| super::super::internal("type parameter lacks name"))?;
        let name = captured
            .ast
            .identifier_name(name)
            .ok_or_else(|| {
                super::super::unsupported_demand_at_location(
                    inputs,
                    &super::super::location(
                        inputs,
                        declaration.file,
                        captured.ast.token_span(name),
                    ),
                    crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
                    "native type-parameter name is not scalar text",
                )
            })?
            .to_owned();
        let expression =
            |id| type_expression(inputs, declaration.file, captured, &captured.parents, id);
        Ok((
            name,
            syntax::id(node, "constraint").map(expression),
            syntax::id(node, "default").map(expression),
        ))
    }
    pub(in crate::project::semantic) fn type_node_declaration(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<DeclarationIdentity, DemandError> {
        let captured = self.syntax(root.file)?;
        Ok(DeclarationIdentity::new(
            root.file,
            captured.node_at(root.site)?,
        ))
    }
    /// `getInferredTypeParameterConstraint` classifies one actual declaration
    /// at a time. Semantic children remain recipes until their native branch.
    pub(in crate::project::semantic) fn inferred_constraint(
        &mut self,
        declaration: DeclarationIdentity,
        omit_type_references: bool,
    ) -> Result<Option<InferredConstraint>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let Some(infer) = parents[declaration.node.index()] else {
            return Ok(None);
        };
        if ast.node(infer).syntax_kind != K::InferType {
            return Ok(None);
        }
        let mut child = infer;
        let mut parent = parents[child.index()];
        while let Some(id) = parent
            && ast.node(id).syntax_kind == K::ParenthesizedType
        {
            child = id;
            parent = parents[id.index()];
        }
        let Some(parent) = parent else {
            return Ok(None);
        };
        let node = ast.node(parent);
        let expression = |id| type_expression(inputs, declaration.file, captured, parents, id);
        if node.syntax_kind == K::TypeReference && !omit_type_references {
            return Ok(syntax::ids(node, "typeArguments")
                .iter()
                .position(|&id| id == child)
                .map(|index| InferredConstraint::Reference {
                    node: Box::new(expression(parent)),
                    index,
                }));
        }
        if node.syntax_kind == K::RestType
            || matches!(node.syntax_kind, K::Parameter | K::NamedTupleMember)
                && syntax::id(node, "dotDotDotToken").is_some()
        {
            return Ok(Some(InferredConstraint::Rest));
        }
        if node.syntax_kind == K::TemplateLiteralTypeSpan {
            return Ok(Some(InferredConstraint::String));
        }
        if node.syntax_kind == K::TypeParameter
            && parents[parent.index()].is_some_and(|id| ast.node(id).syntax_kind == K::MappedType)
        {
            return Ok(Some(InferredConstraint::Keys));
        }
        if node.syntax_kind == K::MappedType
            && syntax::id(node, "type")
                .is_some_and(|id| skip_expression_parentheses(ast, id) == infer)
            && let Some(conditional) = parents[parent.index()]
            && ast.node(conditional).syntax_kind == K::ConditionalType
            && syntax::id(ast.node(conditional), "extendsType") == Some(parent)
        {
            let check = syntax::id(ast.node(conditional), "checkType")
                .ok_or_else(|| super::super::internal("conditional lacks check"))?;
            let check = ast.node(check);
            if check.syntax_kind == K::MappedType
                && let Some(template) = syntax::id(check, "type")
            {
                let parameter = syntax::id(check, "typeParameter")
                    .ok_or_else(|| super::super::internal("mapped check lacks parameter"))?;
                return Ok(Some(InferredConstraint::Mapped {
                    template: Box::new(expression(template)),
                    parameter: DeclarationIdentity::new(declaration.file, parameter),
                    constraint: syntax::id(ast.node(parameter), "constraint")
                        .map(|constraint| Box::new(expression(constraint))),
                }));
            }
        }
        Ok(None)
    }
    /// `getTypeFromConditionalTypeNode` and `isSimpleTupleType` retain only
    /// syntax here; callers acquire each semantic child at its native position.
    pub(in crate::project::semantic) fn conditional_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ConditionalRecipe, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let node = captured.ast.node(id);
        let child = |field| {
            syntax::id(node, field).ok_or_else(|| {
                super::super::internal("conditional node is missing its native child")
            })
        };
        let check = child("checkType")?;
        let extends = child("extendsType")?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        Ok(ConditionalRecipe {
            declaration: DeclarationIdentity::new(root.file, id),
            ordering: crate::project::binding::NativeDeclaration {
                file: root.file,
                span: node.span,
            },
            parent: parents[id.index()].map(|id| DeclarationIdentity::new(root.file, id)),
            check: type_expression(inputs, root.file, captured, parents, check),
            extends: type_expression(inputs, root.file, captured, parents, extends),
            true_type: type_expression(inputs, root.file, captured, parents, child("trueType")?),
            false_type: type_expression(inputs, root.file, captured, parents, child("falseType")?),
            alias: super::alias_for_type_node(root.file, captured, parents, id),
            check_tuples: simple_tuple_arity(ast, check)
                .is_some_and(|arity| Some(arity) == simple_tuple_arity(ast, extends)),
        })
    }

    pub(in crate::project::semantic) fn infer_parameter_declaration(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<DeclarationIdentity, DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let parameter = syntax::id(captured.ast.node(id), "typeParameter")
            .ok_or_else(|| super::super::internal("infer node has no parameter"))?;
        Ok(DeclarationIdentity::new(root.file, parameter))
    }

    /// `getConditionalFlowTypeOfType` walks real parent positions, including
    /// contravariance flips. A type cache never stores these contextual wrappers.
    pub(in crate::project::semantic) fn conditional_flow_steps(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Vec<ConditionalFlowStep>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let mut node = captured.node_at(root.site)?;
        let mut covariant = true;
        let mut steps = Vec::new();
        while !syntax::is_statement(ast, parents, node) && ast.node(node).syntax_kind != K::JSDoc {
            let Some(parent) = parents[node.index()] else {
                break;
            };
            let p = ast.node(parent);
            if p.syntax_kind == K::Parameter {
                covariant = !covariant;
            }
            if p.syntax_kind == K::ConditionalType && syntax::id(p, "trueType") == Some(node) {
                let mut check = syntax::id(p, "checkType")
                    .ok_or_else(|| super::super::internal("conditional has no check type"))?;
                let mut extends = syntax::id(p, "extendsType")
                    .ok_or_else(|| super::super::internal("conditional has no extends type"))?;
                while ast.node(check).syntax_kind == K::TupleType
                    && ast.node(extends).syntax_kind == K::TupleType
                    && syntax::ids(ast.node(check), "elements").len() == 1
                    && syntax::ids(ast.node(extends), "elements").len() == 1
                {
                    check = syntax::ids(ast.node(check), "elements")[0];
                    extends = syntax::ids(ast.node(extends), "elements")[0];
                }
                steps.push(ConditionalFlowStep::Constraint {
                    covariant,
                    check: Box::new(type_expression(inputs, root.file, captured, parents, check)),
                    extends: Box::new(type_expression(
                        inputs, root.file, captured, parents, extends,
                    )),
                });
            } else if p.syntax_kind == K::MappedType
                && syntax::id(p, "nameType").is_none()
                && syntax::id(p, "type") == Some(node)
            {
                steps.push(ConditionalFlowStep::Mapped {
                    declaration: ExpressionRoot {
                        file: root.file,
                        site: Site::new(ast, parent),
                    },
                });
            }
            node = parent;
        }
        Ok(steps)
    }
}

/// `isSimpleTupleType` skips type parentheses before checking every element.
fn simple_tuple_arity(ast: &Ast<'_>, mut id: NodeId) -> Option<usize> {
    while ast.node(id).syntax_kind == K::ParenthesizedType {
        id = syntax::id(ast.node(id), "type")?;
    }
    if ast.node(id).syntax_kind != K::TupleType {
        return None;
    }
    let elements = syntax::ids(ast.node(id), "elements");
    if elements.is_empty()
        || elements.iter().any(|&id| {
            let element = ast.node(id);
            matches!(element.syntax_kind, K::OptionalType | K::RestType)
                || element.syntax_kind == K::NamedTupleMember
                    && (syntax::id(element, "questionToken").is_some()
                        || syntax::id(element, "dotDotDotToken").is_some())
        })
    {
        None
    } else {
        Some(elements.len())
    }
}

// Native ast.SkipParentheses deliberately skips expression parentheses here;
// it does not unwrap ParenthesizedType in a mapped template annotation.
fn skip_expression_parentheses(ast: &Ast<'_>, mut node: NodeId) -> NodeId {
    while ast.node(node).syntax_kind == K::ParenthesizedExpression {
        let Some(inner) = syntax::id(ast.node(node), "expression") else {
            break;
        };
        node = inner;
    }
    node
}
