//! Actual syntax for identifier checking and correlated destructuring reads.
use super::{DeclarationIdentity, ExpressionRoot, Source};
use crate::project::binding::flow::Site;
use crate::project::semantic::model::DemandError;
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) enum NarrowedSymbolRecipe {
    Ordinary,
    Binding {
        parent: DeclarationIdentity,
        pattern: ExpressionRoot,
        root: DeclarationIdentity,
    },
    Parameter {
        function: ExpressionRoot,
        parameters: Vec<DeclarationIdentity>,
        index: i64,
    },
}

impl Source<'_, '_> {
    /// `narrowType`'s aliased-condition declaration gate. Unlike
    /// `getCandidateVariableDeclarationInitializer`, it retains the actual
    /// initializer wrapper so the next narrowType optionality test sees it.
    pub(in crate::project::semantic) fn alias_condition_initializer(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if node.syntax_kind != K::VariableDeclaration || syntax::id(node, "type").is_some() {
            return Ok(None);
        }
        Ok(
            syntax::id(node, "initializer").map(|initializer| ExpressionRoot {
                file: declaration.file,
                site: Site::new(ast, initializer),
            }),
        )
    }

    /// Syntax gates of `getNarrowedTypeOfSymbol`. Semantic constness,
    /// contextual signatures and assignment marks remain checker questions.
    pub(in crate::project::semantic) fn narrowed_symbol_recipe(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<NarrowedSymbolRecipe, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        let identity = |node| DeclarationIdentity::new(declaration.file, node);
        if node.syntax_kind == K::BindingElement
            && syntax::id(node, "initializer").is_none()
            && syntax::id(node, "dotDotDotToken").is_none()
        {
            let pattern = captured.parents[declaration.node.index()]
                .ok_or_else(|| super::super::internal("binding element lacks pattern"))?;
            if syntax::ids(ast.node(pattern), "elements").len() >= 2 {
                let parent = captured.parents[pattern.index()]
                    .ok_or_else(|| super::super::internal("binding pattern lacks parent"))?;
                return Ok(NarrowedSymbolRecipe::Binding {
                    parent: identity(parent),
                    pattern: ExpressionRoot {
                        file: declaration.file,
                        site: Site::new(ast, pattern),
                    },
                    root: identity(syntax::root_declaration(ast, &captured.parents, parent)),
                });
            }
        }
        if node.syntax_kind == K::Parameter
            && syntax::id(node, "type").is_none()
            && syntax::id(node, "initializer").is_none()
            && syntax::id(node, "dotDotDotToken").is_none()
        {
            let parent = captured.parents[declaration.node.index()]
                .ok_or_else(|| super::super::internal("parameter lacks function"))?;
            let parameters = syntax::ids(ast.node(parent), "parameters");
            if parameters.len() >= 2
                && (syntax::is_function_expression_or_arrow_function(ast.node(parent).syntax_kind)
                    || syntax::is_object_literal_method(
                        ast,
                        |id| captured.parents[id.index()],
                        parent,
                    ))
            {
                let position = parameters
                    .iter()
                    .position(|&id| id == declaration.node)
                    .ok_or_else(|| {
                        super::super::internal("parameter absent from actual parameter list")
                    })?;
                let this = parameters.first().is_some_and(|&parameter| {
                    syntax::id(ast.node(parameter), "name")
                        .is_some_and(|name| syntax::is_this_identifier(ast, name))
                });
                let index = position as i64 - i64::from(this);
                return Ok(NarrowedSymbolRecipe::Parameter {
                    function: ExpressionRoot {
                        file: declaration.file,
                        site: Site::new(ast, parent),
                    },
                    parameters: parameters.iter().copied().map(identity).collect(),
                    index,
                });
            }
        }
        Ok(NarrowedSymbolRecipe::Ordinary)
    }

    /// `isSomeSymbolAssignedWorker` visits actual Identifier declarations in
    /// binding-name order, excluding omitted elements without a Name node.
    pub(in crate::project::semantic) fn assignment_name_declarations(
        &mut self,
        root: DeclarationIdentity,
    ) -> Result<Vec<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let name = syntax::id(ast.node(root.node), "name")
            .ok_or_else(|| super::super::internal("assignment root has no declaration name"))?;
        let mut work = vec![name];
        let mut result = Vec::new();
        while let Some(id) = work.pop() {
            if ast.node(id).syntax_kind == K::Identifier {
                let parent = captured.parents[id.index()].ok_or_else(|| {
                    super::super::internal("binding identifier has no declaration")
                })?;
                result.push(DeclarationIdentity::new(root.file, parent));
            } else {
                for &element in syntax::ids(ast.node(id), "elements").iter().rev() {
                    if let Some(name) = syntax::id(ast.node(element), "name") {
                        work.push(name);
                    }
                }
            }
        }
        Ok(result)
    }
}

pub(in crate::project::semantic) enum UncalledReference {
    Call(ExpressionRoot),
    OtherCall,
    Declarations,
}

impl Source<'_, '_> {
    /// `getCandidateDiscriminantPropertyAccess` compares the actual parent
    /// node and excludes both defaults and rest declarations.
    pub(in crate::project::semantic) fn correlated_discriminant_declaration(
        &mut self,
        declaration: DeclarationIdentity,
        reference: ExpressionRoot,
    ) -> Result<bool, DemandError> {
        if declaration.file != reference.file {
            return Ok(false);
        }
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok(matches!(node.syntax_kind, K::BindingElement | K::Parameter)
            && captured.parents[declaration.node.index()]
                .is_some_and(|parent| Site::new(&captured.ast, parent) == reference.site)
            && syntax::id(node, "initializer").is_none()
            && syntax::id(node, "dotDotDotToken").is_none())
    }

    /// `getCandidateVariableDeclarationInitializer`: annotation and actual
    /// declaration kind gate the parenthesis-skipped initializer.
    pub(in crate::project::semantic) fn discriminant_initializer(
        &mut self,
        declaration: DeclarationIdentity,
        binding_parent: bool,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let mut id = declaration.node;
        if binding_parent {
            if ast.node(id).syntax_kind != K::BindingElement
                || syntax::id(ast.node(id), "initializer").is_some()
            {
                return Ok(None);
            }
            let pattern = captured.parents[id.index()]
                .ok_or_else(|| super::super::internal("binding has no pattern"))?;
            id = captured.parents[pattern.index()]
                .ok_or_else(|| super::super::internal("binding pattern has no parent"))?;
        }
        if ast.node(id).syntax_kind != K::VariableDeclaration
            || syntax::id(ast.node(id), "type").is_some()
        {
            return Ok(None);
        }
        Ok(syntax::id(ast.node(id), "initializer").map(|initializer| {
            let initializer = syntax::skip_parentheses(ast, initializer);
            ExpressionRoot {
                file: declaration.file,
                site: Site::new(ast, initializer),
            }
        }))
    }

    /// Pseudo-reference families admitted by `getNarrowedTypeOfSymbol`.
    pub(in crate::project::semantic) fn is_correlated_reference(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        Ok(matches!(
            root.site.kind,
            K::ObjectBindingPattern | K::ArrayBindingPattern
        ) || syntax::is_function_expression_or_arrow_function(root.site.kind)
            || syntax::is_object_literal_method(
                &captured.ast,
                |id| captured.parents[id.index()],
                id,
            ))
    }

    /// `getAccessedPropertyName`'s parameter position includes explicit this;
    /// `getNarrowedTypeOfSymbol` deliberately subtracts it at its own site.
    pub(in crate::project::semantic) fn parameter_access_index(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<String, DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let parent = captured.parents[id.index()]
            .ok_or_else(|| super::super::internal("parameter has no parent"))?;
        let index = syntax::ids(captured.ast.node(parent), "parameters")
            .iter()
            .position(|&parameter| parameter == id)
            .ok_or_else(|| super::super::internal("parameter absent from parent list"))?;
        Ok(index.to_string())
    }

    /// Syntax half of `isUncalledFunctionReference`: skip access ancestors,
    /// then distinguish call/new from the other native call-like families.
    pub(in crate::project::semantic) fn uncalled_reference_context(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<UncalledReference, DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let original = captured.parents[id.index()];
        let mut current = original;
        while let Some(parent) = current {
            if !matches!(
                captured.ast.node(parent).syntax_kind,
                K::PropertyAccessExpression | K::ElementAccessExpression
            ) {
                break;
            }
            current = captured.parents[parent.index()];
        }
        let Some(parent) = current.or(original) else {
            return Ok(UncalledReference::Declarations);
        };
        Ok(match captured.ast.node(parent).syntax_kind {
            K::CallExpression | K::NewExpression => UncalledReference::Call(ExpressionRoot {
                file: root.file,
                site: Site::new(&captured.ast, parent),
            }),
            _ if syntax::is_call_like_expression(&captured.ast, parent) => {
                UncalledReference::OtherCall
            }
            _ => UncalledReference::Declarations,
        })
    }

    /// `isSameScopedBindingElement` and `isInAmbientOrTypeNode` at the
    /// identifier initialization gate. Each uses actual node ancestry.
    pub(in crate::project::semantic) fn identifier_initialization_context(
        &mut self,
        root: ExpressionRoot,
        declaration: Option<DeclarationIdentity>,
    ) -> Result<(bool, bool, bool), DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let ambient =
            syntax::is_in_ambient_or_type_node(ast, |id| captured.parents[id.index()], id);
        let spread = captured.parents[id.index()].is_some_and(|parent| {
            ast.node(parent).syntax_kind == K::SpreadAssignment
                && captured.parents[parent.index()].is_some_and(|object| {
                    super::assignments::is_destructuring_assignment_target(
                        ast,
                        &captured.parents,
                        object,
                    )
                })
        });
        let same = if let Some(declaration) =
            declaration.filter(|declaration| declaration.file == root.file)
            && ast.node(declaration.node).syntax_kind == K::BindingElement
        {
            let mut current = Some(id);
            let mut found = None;
            while let Some(node) = current {
                if ast.node(node).syntax_kind == K::BindingElement {
                    found = Some(node);
                    break;
                }
                current = captured.parents[node.index()];
            }
            found.is_some_and(|element| {
                syntax::root_declaration(ast, &captured.parents, element)
                    == syntax::root_declaration(ast, &captured.parents, declaration.node)
            })
        } else {
            false
        };
        Ok((same, ambient, spread))
    }
}
