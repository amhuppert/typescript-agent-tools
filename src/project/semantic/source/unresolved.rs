//! Actual entity-name syntax for the native unresolved-symbol factory.

use super::super::model::DemandError;
use super::{ExpressionRoot, Source};
use crate::syntax_kind::SyntaxKind as K;
use crate::{Ast, NodeId, Value, syntax};

/// `getTypeReferenceName`: a heritage expression contributes a name only
/// when IsEntityNameExpression accepts it. A call, computed access or outer
/// expression has native nil syntax; it is still a supported type reference.
pub(super) fn type_reference_name_node(ast: &Ast<'_>, id: NodeId) -> Option<NodeId> {
    let node = ast.node(id);
    match node.syntax_kind {
        K::TypeReference => syntax::id(node, "typeName"),
        K::ExpressionWithTypeArguments => syntax::id(node, "expression")
            .filter(|&expression| syntax::is_entity_name_expression(ast, expression)),
        _ => None,
    }
}

impl Source<'_, '_> {
    /// `getTypeReferenceName` retains the actual entity node independently of
    /// the terminal binder attachment used by resolveEntityName.
    pub(in crate::project::semantic) fn type_reference_name(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        if !matches!(
            captured.ast.node(id).syntax_kind,
            K::TypeReference | K::ExpressionWithTypeArguments
        ) {
            return Err(
                super::super::internal("type-reference name requested for another node").into(),
            );
        }
        Ok(
            type_reference_name_node(&captured.ast, id).map(|id| ExpressionRoot {
                file: root.file,
                site: crate::project::binding::flow::Site::new(&captured.ast, id),
            }),
        )
    }

    /// `getUnresolvedSymbolForEntityName` observes terminal text before its
    /// left side. An empty identifier stops that walk and selects unknownSymbol.
    /// Return root-to-leaf components without normalizing their exact spelling.
    pub(in crate::project::semantic) fn unresolved_entity_name(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Vec<std::sync::Arc<str>>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let mut id = captured.node_at(root.site)?;
        let mut names = Vec::new();
        loop {
            let node = ast.node(id);
            let (identifier, left) = match node.syntax_kind {
                K::QualifiedName => (syntax::id(node, "right"), syntax::id(node, "left")),
                K::PropertyAccessExpression => {
                    (syntax::id(node, "name"), syntax::id(node, "expression"))
                }
                K::Identifier => (Some(id), None),
                _ => {
                    return Err(
                        super::super::internal("unresolved entity has non-entity syntax").into(),
                    );
                }
            };
            let identifier = identifier
                .ok_or_else(|| super::super::internal("entity has no terminal identifier"))?;
            let identifier = ast.node(identifier);
            let Some(Value::Text(text)) = identifier
                .field("escapedText")
                .or_else(|| identifier.field("text"))
            else {
                return Err(super::super::unsupported_demand_at_location(
                    inputs,
                    &super::super::location(inputs, root.file, identifier.span),
                    crate::analysis::SemanticFailureReason::UnrepresentableIdentity,
                    "unresolved symbol name has no exact scalar spelling",
                ));
            };
            let name = { captured.shared_text(text) };
            names.push(name);
            if text.is_empty() {
                break;
            }
            match left {
                Some(left) => id = left,
                None => break,
            }
        }
        for index in 0..names.len() / 2 {
            let opposite = names.len() - 1 - index;
            names.swap(index, opposite);
        }
        Ok(names)
    }
}
