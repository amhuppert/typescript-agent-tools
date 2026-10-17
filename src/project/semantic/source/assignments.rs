//! Assignment syntax supplies initial/assigned questions, never inferred types.

use super::{CapturedSyntax, Source};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, syntax};

pub(in crate::project::semantic) enum AssignmentTypeRecipe {
    Initializer(Site),
    Expression(Site),
    Projected {
        source: Site,
        projection: AssignmentProjection,
        initializer: Option<Site>,
        location: Location,
    },
    Default {
        source: Site,
        initializer: Site,
        location: Location,
    },
    ForOf {
        expression: Site,
        asynchronous: bool,
    },
    String,
    Undefined,
    Error(Location),
}

pub(in crate::project::semantic) enum AssignmentProjection {
    Property(Site),
    ArrayElement(usize),
    Spread,
}

impl Source<'_, '_> {
    /// `getInitialOrAssignedType`: the retained assignment's actual node role
    /// chooses initial construction or assignment construction. Narrowing is
    /// a separate evaluator question about the selected reference.
    pub(in crate::project::semantic) fn assignment_type_recipe(
        &mut self,
        file: FileId,
        target: Site,
    ) -> Result<AssignmentTypeRecipe, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(file)?;
        let id = captured.node_at(target)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let location = |id| super::super::location(inputs, file, ast.token_span(id));
        let error = |id| AssignmentTypeRecipe::Error(location(id));
        let for_of = |statement| {
            let node = ast.node(statement);
            let expression = syntax::id(node, "expression")
                .ok_or_else(|| super::super::internal("ForOf has no right-hand expression"))?;
            Ok::<_, AnalysisError>(AssignmentTypeRecipe::ForOf {
                expression: Site::new(ast, expression),
                asynchronous: syntax::id(node, "awaitModifier").is_some(),
            })
        };
        // `getInitialType` / `getInitialTypeOfVariableDeclaration`.
        match ast.node(id).syntax_kind {
            SyntaxKind::VariableDeclaration => {
                if let Some(initializer) = syntax::id(ast.node(id), "initializer") {
                    return Ok(AssignmentTypeRecipe::Initializer(Site::new(
                        ast,
                        initializer,
                    )));
                }
                let statement = parents[id.index()].and_then(|parent| parents[parent.index()]);
                return match statement {
                    Some(statement)
                        if ast.node(statement).syntax_kind == SyntaxKind::ForInStatement =>
                    {
                        Ok(AssignmentTypeRecipe::String)
                    }
                    Some(statement)
                        if ast.node(statement).syntax_kind == SyntaxKind::ForOfStatement =>
                    {
                        for_of(statement)
                    }
                    _ => Ok(error(id)),
                };
            }
            SyntaxKind::BindingElement => {
                // `getInitialTypeOfBindingElement`: the containing pattern
                // acquires its declaration's initial type before projection.
                let pattern = parents[id.index()].ok_or_else(|| {
                    super::super::internal("BindingElement has no binding pattern")
                })?;
                let declaration = parents[pattern.index()].ok_or_else(|| {
                    super::super::internal("binding pattern has no initial declaration")
                })?;
                let projection = match ast.node(pattern).syntax_kind {
                    SyntaxKind::ObjectBindingPattern => {
                        // `getBindingElementPropertyName` is PropertyNameOrName.
                        let name = syntax::id(ast.node(id), "propertyName")
                            .or_else(|| syntax::id(ast.node(id), "name"))
                            .ok_or_else(|| super::super::internal("binding element has no name"))?;
                        AssignmentProjection::Property(Site::new(ast, name))
                    }
                    SyntaxKind::ArrayBindingPattern
                        if syntax::id(ast.node(id), "dotDotDotToken").is_some() =>
                    {
                        AssignmentProjection::Spread
                    }
                    SyntaxKind::ArrayBindingPattern => {
                        AssignmentProjection::ArrayElement(element_index(ast, pattern, id)?)
                    }
                    _ => {
                        return Err(super::super::internal(
                            "BindingElement parent is not a pattern",
                        ));
                    }
                };
                return Ok(AssignmentTypeRecipe::Projected {
                    source: Site::new(ast, declaration),
                    projection,
                    initializer: syntax::id(ast.node(id), "initializer")
                        .map(|initializer| Site::new(ast, initializer)),
                    location: location(id),
                });
            }
            _ => {}
        }
        // `getAssignedType`: all grammar families remain distinct. Only a
        // genuine native errorType fallback is represented as Error.
        let parent = parents[id.index()]
            .ok_or_else(|| super::super::internal("assignment target has no native parent"))?;
        let node = ast.node(parent);
        Ok(match node.syntax_kind {
            SyntaxKind::ForInStatement => AssignmentTypeRecipe::String,
            SyntaxKind::ForOfStatement => return for_of(parent),
            SyntaxKind::BinaryExpression => {
                let grandparent = parents[parent.index()];
                // `getAssignedTypeOfBinaryExpression` distinguishes defaults
                // in actual destructuring assignment targets from ordinary RHS.
                let destructuring_default =
                    grandparent.is_some_and(|container| match ast.node(container).syntax_kind {
                        SyntaxKind::ArrayLiteralExpression => {
                            is_destructuring_assignment_target(ast, parents, container)
                        }
                        SyntaxKind::PropertyAssignment => {
                            parents[container.index()].is_some_and(|object| {
                                is_destructuring_assignment_target(ast, parents, object)
                            })
                        }
                        _ => false,
                    });
                if destructuring_default {
                    let initializer = syntax::id(node, "right").ok_or_else(|| {
                        super::super::internal("destructuring default has no right expression")
                    })?;
                    AssignmentTypeRecipe::Default {
                        source: Site::new(ast, parent),
                        initializer: Site::new(ast, initializer),
                        location: location(parent),
                    }
                } else {
                    let right = syntax::id(node, "right").ok_or_else(|| {
                        super::super::internal("binary assignment has no right expression")
                    })?;
                    AssignmentTypeRecipe::Expression(Site::new(ast, right))
                }
            }
            SyntaxKind::DeleteExpression => AssignmentTypeRecipe::Undefined,
            SyntaxKind::ArrayLiteralExpression => AssignmentTypeRecipe::Projected {
                source: Site::new(ast, parent),
                projection: AssignmentProjection::ArrayElement(element_index(ast, parent, id)?),
                initializer: None,
                location: location(parent),
            },
            SyntaxKind::SpreadElement => {
                let array = parents[parent.index()].ok_or_else(|| {
                    super::super::internal("assignment spread has no containing array")
                })?;
                AssignmentTypeRecipe::Projected {
                    source: Site::new(ast, array),
                    projection: AssignmentProjection::Spread,
                    initializer: None,
                    location: location(parent),
                }
            }
            SyntaxKind::PropertyAssignment | SyntaxKind::ShorthandPropertyAssignment => {
                let object = parents[parent.index()].ok_or_else(|| {
                    super::super::internal("assigned property has no containing object")
                })?;
                let name = syntax::id(node, "name")
                    .ok_or_else(|| super::super::internal("assigned property has no name"))?;
                AssignmentTypeRecipe::Projected {
                    source: Site::new(ast, object),
                    projection: AssignmentProjection::Property(Site::new(ast, name)),
                    initializer: if node.syntax_kind == SyntaxKind::ShorthandPropertyAssignment {
                        syntax::id(node, "objectAssignmentInitializer")
                            .map(|initializer| Site::new(ast, initializer))
                    } else {
                        None
                    },
                    location: location(parent),
                }
            }
            _ => error(id),
        })
    }
}

/// `slices.Index(pattern.Elements(), element)`: omitted elements occupy their
/// real slot. Exact child identity cannot be reconstructed from name/span text.
fn element_index(ast: &Ast<'_>, pattern: NodeId, element: NodeId) -> Result<usize, AnalysisError> {
    syntax::ids(ast.node(pattern), "elements")
        .iter()
        .position(|&candidate| candidate == element)
        .ok_or_else(|| super::super::internal("assigned element is absent from its actual parent"))
}

/// `isDestructuringAssignmentTarget`: native parent and child identity, not
/// the operator spelling or resemblance to a destructuring pattern.
pub(super) fn is_destructuring_assignment_target(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> bool {
    let Some(parent) = parents[id.index()] else {
        return false;
    };
    let parent = ast.node(parent);
    parent.syntax_kind == SyntaxKind::BinaryExpression && syntax::id(parent, "left") == Some(id)
        || parent.syntax_kind == SyntaxKind::ForOfStatement
            && syntax::id(parent, "initializer") == Some(id)
}
