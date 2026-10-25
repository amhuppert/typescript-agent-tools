//! Exact context/position recipes for the native reference narrowing question.

use super::{CapturedSyntax, Source, TypeExpression, type_expression};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;

/// Immutable syntax position for native references. Created references retain
/// an actual parent node without pretending to occupy a captured source span.
#[derive(Clone, Copy)]
pub(in crate::project::semantic) enum ReferencePosition {
    Captured(Site),
    Created { kind: SyntaxKind, parent: Site },
}
impl From<Site> for ReferencePosition {
    fn from(site: Site) -> Self {
        Self::Captured(site)
    }
}
impl ReferencePosition {
    fn resolve(
        self,
        captured: &CapturedSyntax<'_>,
    ) -> Result<
        (
            Option<crate::NodeId>,
            Option<crate::NodeId>,
            SyntaxKind,
            u32,
        ),
        AnalysisError,
    > {
        Ok(match self {
            Self::Captured(site) => {
                let id = captured.node_at(site)?;
                (
                    Some(id),
                    captured.parents[id.index()],
                    captured.ast.node(id).syntax_kind,
                    captured.ast.node(id).flags,
                )
            }
            Self::Created { kind, parent } => (None, Some(captured.node_at(parent)?), kind, 0),
        })
    }
}

pub(in crate::project::semantic) enum ConstraintPosition {
    No,
    Yes,
    ElementIndex(Site),
}

pub(in crate::project::semantic) struct ReferenceContext {
    pub(in crate::project::semantic) in_with: bool,
    pub(in crate::project::semantic) recipe: ContextRecipe,
}

pub(in crate::project::semantic) enum ContextRecipe {
    Absent,
    VariableInitializer(super::DeclarationIdentity),
    Forward(Site),
    /// `case SpreadAssignment`: the containing object literal's context, as
    /// `Forward`; kept distinct because Q5 asks whether a literal's context
    /// came from an enclosing literal's spread operand.
    Spread(Site),
    AwaitOperand(Site),
    Annotation(Box<TypeExpression>),
    Return(Option<super::SignatureRoot>),
    ObjectElement(Site),
    BinaryOperand {
        parent: Site,
        operator: SyntaxKind,
        left: super::ExpressionRoot,
        is_right: bool,
    },
    CallArgument {
        parent: super::CallRoot,
        argument: Option<Site>,
    },
    ArrayElement {
        parent: Site,
        index: Option<usize>,
        length: usize,
    },
    Pending {
        location: Location,
        message: &'static str,
    },
}

impl Source<'_, '_> {
    pub(in crate::project::semantic) fn expression_is_object_literal_method(
        &mut self,
        root: super::ExpressionRoot,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        Ok(syntax::is_object_literal_method(
            &captured.ast,
            |id| captured.parents[id.index()],
            id,
        ))
    }

    pub(in crate::project::semantic) fn leftmost_expression(
        &mut self,
        root: super::ExpressionRoot,
        stop_at_calls: bool,
    ) -> Result<super::ExpressionRoot, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site)?;
        let node = syntax::leftmost_expression(&captured.ast, node, stop_at_calls)
            .ok_or_else(|| super::super::internal("expression lacks its precedence child"))?;
        Ok(super::ExpressionRoot {
            file: root.file,
            site: Site::new(&captured.ast, node),
        })
    }

    /// `isNodeDescendantOf` compares actual captured nodes, not source spans.
    pub(in crate::project::semantic) fn expression_is_descendant(
        &mut self,
        node: super::ExpressionRoot,
        ancestor: super::ExpressionRoot,
    ) -> Result<bool, AnalysisError> {
        if node.file != ancestor.file {
            return Ok(false);
        }
        let captured = self.syntax(node.file)?;
        let ancestor = captured.node_at(ancestor.site)?;
        Ok(captured.node_is_descendant(captured.node_at(node.site)?, ancestor))
    }

    /// `isConstraintPosition`: project the exact immediate parent and child
    /// role. The generic receiver/index exception belongs to evaluation.
    pub(in crate::project::semantic) fn constraint_position(
        &mut self,
        file: FileId,
        site: impl Into<ReferencePosition>,
    ) -> Result<ConstraintPosition, AnalysisError> {
        let captured = self.syntax(file)?;
        let (id, parent, _, _) = site.into().resolve(captured)?;
        let CapturedSyntax { ast, .. } = captured;
        let Some(parent) = parent else {
            return Ok(ConstraintPosition::No);
        };
        let node = ast.node(parent);
        Ok(match node.syntax_kind {
            SyntaxKind::PropertyAccessExpression | SyntaxKind::QualifiedName => {
                ConstraintPosition::Yes
            }
            SyntaxKind::CallExpression | SyntaxKind::NewExpression
                if id.is_some_and(|id| syntax::id(node, "expression") == Some(id)) =>
            {
                ConstraintPosition::Yes
            }
            SyntaxKind::ElementAccessExpression
                if id.is_some_and(|id| syntax::id(node, "expression") == Some(id)) =>
            {
                let argument = syntax::id(node, "argumentExpression").ok_or_else(|| {
                    super::super::internal("element access has no index expression")
                })?;
                ConstraintPosition::ElementIndex(Site::new(ast, argument))
            }
            _ => ConstraintPosition::No,
        })
    }

    /// `hasContextualTypeWithNoGenericTypes`: JSX tag names deliberately skip
    /// contextual acquisition to avoid resolving their own element recursively.
    pub(in crate::project::semantic) fn reference_has_context_role(
        &mut self,
        file: FileId,
        site: impl Into<ReferencePosition>,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(file)?;
        let (id, parent, kind, _) = site.into().resolve(captured)?;
        let CapturedSyntax { ast, .. } = captured;
        Ok(matches!(
            kind,
            SyntaxKind::Identifier
                | SyntaxKind::PropertyAccessExpression
                | SyntaxKind::ElementAccessExpression
        ) && !parent.is_some_and(|parent| {
            let parent = ast.node(parent);
            matches!(
                parent.syntax_kind,
                SyntaxKind::JsxOpeningElement | SyntaxKind::JsxSelfClosingElement
            ) && id.is_some_and(|id| syntax::id(parent, "tagName") == Some(id))
        }))
    }

    /// `getContextualType` / `getContextualTypeForInitializerExpression` /
    /// `getContextualTypeForVariableLikeDeclaration`: absent context is proved
    /// by the native parent decision; unimplemented semantic dependencies are
    /// retained as located recipes, never silently treated as absent.
    pub(in crate::project::semantic) fn reference_context(
        &mut self,
        file: FileId,
        site: impl Into<ReferencePosition>,
    ) -> Result<ReferenceContext, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(file)?;
        let (id, parent, _, node_flags) = site.into().resolve(captured)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let in_with = node_flags & crate::compiler::node::IN_WITH_STATEMENT != 0;
        let Some(parent_id) = parent else {
            return Ok(ReferenceContext {
                in_with,
                recipe: ContextRecipe::Absent,
            });
        };
        let parent = ast.node(parent_id);
        let pending = |message| ContextRecipe::Pending {
            location: super::super::location(inputs, file, ast.token_span(parent_id)),
            message,
        };
        let annotation = |annotation| {
            ContextRecipe::Annotation(Box::new(type_expression(
                inputs, file, captured, parents, annotation,
            )))
        };
        let recipe = match parent.syntax_kind {
            SyntaxKind::VariableDeclaration
            | SyntaxKind::Parameter
            | SyntaxKind::PropertyDeclaration
            | SyntaxKind::PropertySignature
            | SyntaxKind::BindingElement => {
                if !id.is_some_and(|id| syntax::id(parent, "initializer") == Some(id)) {
                    ContextRecipe::Absent
                } else {
                    ContextRecipe::VariableInitializer(super::DeclarationIdentity::new(
                        file, parent_id,
                    ))
                }
            }
            SyntaxKind::ParenthesizedExpression | SyntaxKind::NonNullExpression => {
                ContextRecipe::Forward(Site::new(ast, parent_id))
            }
            SyntaxKind::TypeAssertionExpression | SyntaxKind::AsExpression => {
                let ty = syntax::id(parent, "type")
                    .ok_or_else(|| super::super::internal("type assertion has no type node"))?;
                if syntax::is_const_assertion(ast, parent_id) {
                    ContextRecipe::Forward(Site::new(ast, parent_id))
                } else {
                    annotation(ty)
                }
            }
            SyntaxKind::SatisfiesExpression => {
                let ty = syntax::id(parent, "type").ok_or_else(|| {
                    super::super::internal("satisfies expression has no type node")
                })?;
                annotation(ty)
            }
            SyntaxKind::ExportAssignment => match syntax::id(parent, "type") {
                Some(ty) => annotation(ty),
                None => ContextRecipe::Absent,
            },
            SyntaxKind::ArrowFunction | SyntaxKind::ReturnStatement => {
                let mut current = Some(parent_id);
                let mut function = None;
                while let Some(parent) = current {
                    if syntax::is_function_like_kind(ast.node(parent).syntax_kind) {
                        function = Some(
                            super::signature_root(ast, file, parent)
                                .map_err(super::super::internal)?,
                        );
                        break;
                    }
                    current = parents[parent.index()];
                }
                ContextRecipe::Return(function)
            }
            SyntaxKind::AwaitExpression => ContextRecipe::AwaitOperand(Site::new(ast, parent_id)),
            SyntaxKind::YieldExpression => {
                pending("native contextual yield operand acquisition is pending")
            }
            SyntaxKind::CallExpression | SyntaxKind::NewExpression => ContextRecipe::CallArgument {
                parent: super::CallRoot::new(file, ast.token_span(parent_id), parent.syntax_kind),
                argument: id.map(|id| Site::new(ast, id)),
            },
            SyntaxKind::Decorator => pending("native contextual decorator acquisition is pending"),
            SyntaxKind::BinaryExpression => {
                // `getContextualTypeForBinaryOperand` observes the binary's
                // annotation before deciding which operand/operator has context.
                if let Some(ty) = syntax::id(parent, "type") {
                    annotation(ty)
                } else {
                    let token = syntax::id(parent, "operatorToken").ok_or_else(|| {
                        super::super::internal("binary context lacks operator token")
                    })?;
                    let left = syntax::id(parent, "left").ok_or_else(|| {
                        super::super::internal("binary context lacks left operand")
                    })?;
                    ContextRecipe::BinaryOperand {
                        parent: Site::new(ast, parent_id),
                        operator: ast.node(token).syntax_kind,
                        left: super::ExpressionRoot {
                            file,
                            site: Site::new(ast, left),
                        },
                        is_right: id.is_some_and(|id| syntax::id(parent, "right") == Some(id)),
                    }
                }
            }
            SyntaxKind::PropertyAssignment | SyntaxKind::ShorthandPropertyAssignment => {
                ContextRecipe::ObjectElement(Site::new(ast, parent_id))
            }
            SyntaxKind::SpreadAssignment => {
                let object = parents[parent_id.index()].ok_or_else(|| {
                    super::super::internal("spread assignment has no object parent")
                })?;
                ContextRecipe::Spread(Site::new(ast, object))
            }
            SyntaxKind::ArrayLiteralExpression => {
                let elements = syntax::ids(parent, "elements");
                let index = id.and_then(|id| elements.iter().position(|&element| element == id));
                ContextRecipe::ArrayElement {
                    parent: Site::new(ast, parent_id),
                    index,
                    length: elements.len(),
                }
            }
            SyntaxKind::ConditionalExpression => {
                // `getContextualTypeForConditionalOperand` forwards the
                // parent's context to both arms, never to the condition.
                if id.is_some_and(|id| {
                    syntax::id(parent, "whenTrue") == Some(id)
                        || syntax::id(parent, "whenFalse") == Some(id)
                }) {
                    ContextRecipe::Forward(Site::new(ast, parent_id))
                } else {
                    ContextRecipe::Absent
                }
            }
            SyntaxKind::TemplateSpan => {
                // getContextualTypeForSubstitutionExpression asks an argument
                // context only for an actual tagged template. An ordinary
                // interpolation has no contextual type for its substitution.
                let template = parents[parent_id.index()]
                    .ok_or_else(|| super::super::internal("template span lacks template parent"))?;
                if parents[template.index()].is_some_and(|parent| {
                    ast.node(parent).syntax_kind == SyntaxKind::TaggedTemplateExpression
                }) {
                    let tagged = parents[template.index()].expect("checked tagged-template parent");
                    ContextRecipe::CallArgument {
                        parent: super::CallRoot::new(
                            file,
                            ast.token_span(tagged),
                            ast.node(tagged).syntax_kind,
                        ),
                        argument: id.map(|id| Site::new(ast, id)),
                    }
                } else {
                    ContextRecipe::Absent
                }
            }
            SyntaxKind::JsxExpression
            | SyntaxKind::JsxAttribute
            | SyntaxKind::JsxSpreadAttribute
            | SyntaxKind::JsxOpeningElement
            | SyntaxKind::JsxSelfClosingElement => {
                pending("native contextual JSX acquisition requires excluded JSX typing")
            }
            SyntaxKind::ImportAttribute => {
                pending("native contextual import-attribute acquisition is pending")
            }
            _ => ContextRecipe::Absent,
        };
        Ok(ReferenceContext { in_with, recipe })
    }
}
