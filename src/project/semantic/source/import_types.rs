//! Syntax for `getTypeFromImportTypeNode`. A literal argument keeps the
//! module request binding collected for it, so its resolution (including a
//! `resolution-mode` attribute) is the loader's and is never repeated here.

use super::super::model::DemandError;
use super::instantiation_expressions::{InstantiationSite, instantiation_site};
use super::{AliasRecipe, ExpressionRoot, Source, TypeExpression};
use crate::project::binding::ReferenceModuleSite;
use crate::project::binding::flow::Site;
use crate::references::Location;
use crate::syntax;
use crate::syntax_kind::SyntaxKind as K;

pub(in crate::project::semantic) struct ImportTypeRecipe {
    pub(in crate::project::semantic) is_type_of: bool,
    /// `IsLiteralImportTypeNode`: the request of a string-literal argument,
    /// or the location of any other argument, which names no module.
    pub(in crate::project::semantic) module: Result<ReferenceModuleSite, Location>,
    /// `getIdentifierChain` of the qualifier, empty without one.
    pub(in crate::project::semantic) qualifier: Vec<ImportTypeName>,
    /// The ImportType node itself, which `getInstantiationExpressionType`
    /// and `typeArgumentsFromTypeReferenceNode` both read.
    pub(in crate::project::semantic) site: InstantiationSite,
    pub(in crate::project::semantic) alias: Option<AliasRecipe>,
}

impl ImportTypeRecipe {
    /// `typeArgumentsFromTypeReferenceNode`'s written children.
    pub(in crate::project::semantic) fn type_arguments(&self) -> Vec<TypeExpression> {
        self.site
            .arguments
            .iter()
            .flatten()
            .map(|argument| argument.expression.clone())
            .collect()
    }
}

/// One identifier of the qualifier. The checker associates its resolved
/// symbol with the identifier and with the identifier's parent.
pub(in crate::project::semantic) struct ImportTypeName {
    pub(in crate::project::semantic) name: String,
    pub(in crate::project::semantic) root: ExpressionRoot,
    pub(in crate::project::semantic) parent: ExpressionRoot,
    pub(in crate::project::semantic) location: Location,
}

impl Source<'_, '_> {
    /// Retain the actual ImportType node's syntax without resolving or
    /// checking anything; evaluation owns every acquisition and its order.
    pub(in crate::project::semantic) fn import_type_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ImportTypeRecipe, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let id = captured.node_at(root.site)?;
        let node = ast.node(id);
        if node.syntax_kind != K::ImportType {
            return Err(super::super::internal("import type recipe lacks its ImportType").into());
        }
        let argument = syntax::id(node, "argument")
            .ok_or_else(|| super::super::internal("import type lacks its argument"))?;
        let module = match syntax::literal_import_type_specifier(ast, node) {
            Some(literal) => Ok(super::reference_import_tracker::retained_module_site(
                inputs.bindings,
                captured,
                root.file,
                literal,
            )?
            .ok_or_else(|| {
                super::super::internal("import type literal lacks its retained module request")
            })?),
            None => Err(super::super::location(
                inputs,
                root.file,
                ast.token_span(argument),
            )),
        };
        let mut qualifier = Vec::new();
        if let Some(mut name) = syntax::id(node, "qualifier") {
            // getIdentifierChain: rights from the outermost QualifiedName in,
            // then the leftmost identifier, reversed into source order.
            loop {
                let (identifier, left) = match ast.node(name).syntax_kind {
                    K::Identifier => (name, None),
                    K::QualifiedName => (
                        syntax::id(ast.node(name), "right").ok_or_else(|| {
                            super::super::internal("qualified name lacks its right")
                        })?,
                        syntax::id(ast.node(name), "left"),
                    ),
                    _ => {
                        return Err(super::super::internal(
                            "import type qualifier is not an entity name",
                        )
                        .into());
                    }
                };
                let parent = parents[identifier.index()]
                    .ok_or_else(|| super::super::internal("qualifier name lacks its parent"))?;
                qualifier.push(ImportTypeName {
                    name: super::reference_imports::exact_text(inputs, root.file, ast, identifier)?,
                    root: ExpressionRoot {
                        file: root.file,
                        site: Site::new(ast, identifier),
                    },
                    parent: ExpressionRoot {
                        file: root.file,
                        site: Site::new(ast, parent),
                    },
                    location: super::super::location(inputs, root.file, ast.token_span(identifier)),
                });
                match left {
                    Some(left) => name = left,
                    None => break,
                }
            }
            qualifier.reverse();
        }
        Ok(ImportTypeRecipe {
            is_type_of: syntax::flag(node, "isTypeOf"),
            module,
            qualifier,
            site: instantiation_site(inputs, root.file, captured, id)?,
            alias: super::alias_for_type_node(root.file, captured, parents, id),
        })
    }
}
