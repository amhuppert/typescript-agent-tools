//! Syntax decisions used by relation diagnostics, over actual native nodes.

use super::{DeclarationIdentity, ExpressionNode, ExpressionRoot, Source};
use crate::project::binding::flow::Site;
use crate::project::semantic::model::DemandError;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;

pub(in crate::project::semantic) struct ExcessPropertyDiagnostic {
    pub(in crate::project::semantic) jsx: bool,
    pub(in crate::project::semantic) node: ExpressionNode,
    pub(in crate::project::semantic) suggestion_name: Option<String>,
}

impl Source<'_, '_> {
    /// `shouldCheckAsExcessProperty` compares the actual ValueDeclaration
    /// parent with the source symbol's actual ValueDeclaration.
    pub(in crate::project::semantic) fn should_check_excess_property(
        &mut self,
        property: DeclarationIdentity,
        container: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        if property.file != container.file {
            return Ok(false);
        }
        Ok(self.syntax(property.file)?.parents[property.node.index()] == Some(container.node))
    }

    /// The JSX diagnostic branch of `hasExcessProperties` checks both the
    /// actual error node and its immediate parent.
    pub(in crate::project::semantic) fn is_jsx_relation_error(
        &mut self,
        error: ExpressionNode,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(error.file())?;
        let opening = |kind| {
            matches!(
                kind,
                SyntaxKind::JsxOpeningElement | SyntaxKind::JsxSelfClosingElement
            )
        };
        match error {
            ExpressionNode::Captured(error) => {
                let error_id = captured.node_at(error.site)?;
                Ok(
                    captured.ast.node(error_id).syntax_kind == SyntaxKind::JsxAttributes
                        || opening(captured.ast.node(error_id).syntax_kind)
                        || captured.parents[error_id.index()]
                            .is_some_and(|parent| opening(captured.ast.node(parent).syntax_kind)),
                )
            }
            ExpressionNode::Synthetic { parent, .. } => {
                // SyntheticExpression itself matches no JSX kind. Native
                // checks its immediate Parent, not that parent's parent.
                Ok(opening(
                    captured
                        .ast
                        .node(captured.node_at(parent.site)?)
                        .syntax_kind,
                ))
            }
        }
    }

    /// `hasExcessProperties` selects its diagnostic node by syntax ancestry.
    /// JSX checks its actual attribute name; ordinary literals can suggest an
    /// identifier only after the declaration is proven inside the literal.
    pub(in crate::project::semantic) fn excess_property_diagnostic(
        &mut self,
        error: ExpressionNode,
        property: Option<DeclarationIdentity>,
        object: Option<DeclarationIdentity>,
    ) -> Result<ExcessPropertyDiagnostic, DemandError> {
        let jsx = self.is_jsx_relation_error(error)?;
        let mut result = ExcessPropertyDiagnostic {
            jsx,
            node: error,
            suggestion_name: None,
        };
        let Some(property) = property else {
            return Ok(result);
        };
        if jsx {
            let captured = self.syntax(property.file)?;
            if captured.ast.node(property.node).syntax_kind == SyntaxKind::JsxAttribute
                && property.file == error.file()
            {
                let name = syntax::id(captured.ast.node(property.node), "name")
                    .ok_or_else(|| super::super::internal("JSX attribute has no native name"))?;
                result.node = ExpressionRoot {
                    file: property.file,
                    site: Site::new(&captured.ast, name),
                }
                .into();
            }
            return Ok(result);
        }
        let Some(object) = object else {
            return Ok(result);
        };
        if property.file != object.file {
            return Ok(result);
        }
        let captured = self.syntax(property.file)?;
        if !matches!(
            captured.ast.node(property.node).syntax_kind,
            SyntaxKind::PropertyAssignment
                | SyntaxKind::ShorthandPropertyAssignment
                | SyntaxKind::SpreadAssignment
                | SyntaxKind::MethodDeclaration
                | SyntaxKind::GetAccessor
                | SyntaxKind::SetAccessor
        ) {
            return Ok(result);
        }
        let mut parent = Some(property.node);
        while let Some(node) = parent {
            if node == object.node {
                if object.file == error.file() {
                    let name =
                        syntax::id(captured.ast.node(property.node), "name").ok_or_else(|| {
                            super::super::internal("object literal element has no native name")
                        })?;
                    result.node = ExpressionRoot {
                        file: property.file,
                        site: Site::new(&captured.ast, name),
                    }
                    .into();
                    if captured.ast.node(name).syntax_kind == SyntaxKind::Identifier {
                        result.suggestion_name = Some(
                            captured
                                .ast
                                .identifier_name(name)
                                .ok_or_else(|| {
                                    super::super::internal("identifier lacks exact spelling")
                                })?
                                .to_owned(),
                        );
                    }
                }
                break;
            }
            parent = captured.parents[node.index()];
        }
        Ok(result)
    }
}
