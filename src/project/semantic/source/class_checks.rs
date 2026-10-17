//! Class checks retain syntax order; symbols, types and recovery stay in the evaluator.
use super::super::model::DemandError;
use super::{DeclarationIdentity, ExpressionRoot, Source, TypeExpression, type_expression};
use crate::project::binding::flow::Site;
use crate::syntax;
use crate::syntax_kind::SyntaxKind as K;

pub(in crate::project::semantic) struct ClassCheckRecipe {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) parameters: Vec<ExpressionRoot>,
    pub(in crate::project::semantic) members: Vec<ExpressionRoot>,
    pub(in crate::project::semantic) decorators: Vec<ExpressionRoot>,
    pub(in crate::project::semantic) implements: Vec<TypeExpression>,
    pub(in crate::project::semantic) ambient: bool,
    pub(in crate::project::semantic) constructor: Option<ExpressionRoot>,
}

pub(in crate::project::semantic) struct InitializationProperty {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) name: ExpressionRoot,
    pub(in crate::project::semantic) argument: Option<ExpressionRoot>,
}

impl Source<'_, '_> {
    /// Syntax inputs of `checkClassLikeDeclaration`. Member bodies remain
    /// deferred, independently of the immediate index/initialization checks.
    pub(in crate::project::semantic) fn class_check_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ClassCheckRecipe, DemandError> {
        let inputs = self.inputs;
        let legacy = inputs.semantic_options().experimental_decorators;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let node = ast.node(id);
        let expression = |id| ExpressionRoot {
            file: root.file,
            site: Site::new(ast, id),
        };
        let members = syntax::ids(node, "members");
        Ok(ClassCheckRecipe {
            declaration: DeclarationIdentity::new(root.file, id),
            parameters: syntax::ids(node, "typeParameters")
                .iter()
                .copied()
                .map(expression)
                .collect(),
            members: members.iter().copied().map(expression).collect(),
            decorators: if syntax::node_can_be_decorated(ast, &captured.parents, id, legacy) {
                syntax::ids(node, "modifiers")
                    .iter()
                    .copied()
                    .filter(|&id| ast.node(id).syntax_kind == K::Decorator)
                    .map(expression)
                    .collect()
            } else {
                Vec::new()
            },
            implements: syntax::heritage_elements(ast, id, K::ImplementsKeyword)
                .iter()
                .map(|&id| type_expression(inputs, root.file, captured, &captured.parents, id))
                .collect(),
            ambient: node.flags & crate::compiler::node::AMBIENT != 0,
            constructor: syntax::find_constructor_declaration(ast, id).map(expression),
        })
    }

    /// `checkPropertyInitialization`'s syntax gates precede symbol/type demand.
    pub(in crate::project::semantic) fn initialization_properties(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Vec<InitializationProperty>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let expression = |id| ExpressionRoot {
            file: declaration.file,
            site: Site::new(ast, id),
        };
        let mut properties = Vec::new();
        for &id in syntax::ids(ast.node(declaration.node), "members") {
            let node = ast.node(id);
            if syntax::has_modifier(ast, node, K::DeclareKeyword)
                || syntax::is_static(ast, id)
                || !syntax::is_property_without_initializer(ast, id)
            {
                continue;
            }
            let Some(name) = syntax::id(node, "name") else {
                continue;
            };
            if !matches!(
                ast.node(name).syntax_kind,
                K::Identifier | K::PrivateIdentifier | K::ComputedPropertyName
            ) {
                continue;
            }
            properties.push(InitializationProperty {
                declaration: DeclarationIdentity::new(declaration.file, id),
                name: expression(name),
                argument: if ast.node(name).syntax_kind == K::ComputedPropertyName {
                    Some(expression(
                        syntax::id(ast.node(name), "expression").ok_or_else(|| {
                            super::super::internal("computed property name lacks expression")
                        })?,
                    ))
                } else {
                    None
                },
            });
        }
        Ok(properties)
    }

    /// `IsParameterPropertyDeclaration` inventory. Binding-pattern exclusion
    /// belongs only to the duplicate-member checker, not override checking.
    pub(in crate::project::semantic) fn constructor_parameter_properties(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Vec<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let constructor = ast.node(declaration.node);
        Ok(syntax::ids(constructor, "parameters")
            .iter()
            .copied()
            .filter(|&id| {
                let parameter = ast.node(id);
                syntax::is_parameter_property_declaration(ast, parameter, constructor)
            })
            .map(|node| DeclarationIdentity::new(declaration.file, node))
            .collect())
    }

    pub(in crate::project::semantic) fn declaration_is_ambient(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        Ok(self
            .syntax(declaration.file)?
            .ast
            .node(declaration.node)
            .flags
            & crate::compiler::node::AMBIENT
            != 0)
    }

    /// `checkClassExpressionExternalHelpers` after decorators have been checked.
    /// Decorated classes currently stop in their required decorator checker;
    /// this recipe covers the independent static-element transformation path.
    pub(in crate::project::semantic) fn class_static_helper_site(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<(DeclarationIdentity, bool)>, DemandError> {
        let options = self.inputs.semantic_options();
        if !options.class_static_elements_need_transform() {
            return Ok(None);
        }
        let transform_initializers = !options.emit_standard_class_fields;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if syntax::id(node, "name").is_some() {
            return Ok(None);
        }
        // `walkUpOuterExpressions` returns the first non-outer parent.
        let mut parent = captured.parents[declaration.node.index()];
        while let Some(id) = parent
            && syntax::is_outer_expression(ast, id, true)
        {
            parent = captured.parents[id.index()];
        }
        let Some(parent) = parent.filter(|&id| syntax::is_named_evaluation_source(ast, id)) else {
            return Ok(None);
        };
        for &member in syntax::ids(node, "members") {
            let current = ast.node(member);
            let private = syntax::id(current, "name")
                .is_some_and(|id| ast.node(id).syntax_kind == K::PrivateIdentifier);
            if current.syntax_kind == K::ClassStaticBlockDeclaration
                || syntax::is_static(ast, member)
                    && (private
                        || transform_initializers
                            && current.syntax_kind == K::PropertyDeclaration
                            && syntax::id(current, "initializer").is_some())
            {
                let parent = ast.node(parent);
                let computed = matches!(
                    parent.syntax_kind,
                    K::PropertyAssignment | K::PropertyDeclaration | K::BindingElement
                ) && syntax::id(parent, "name")
                    .is_some_and(|id| ast.node(id).syntax_kind == K::ComputedPropertyName);
                return Ok(Some((
                    DeclarationIdentity::new(declaration.file, member),
                    computed,
                )));
            }
        }
        Ok(None)
    }
}
