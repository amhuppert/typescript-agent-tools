//! Exact access syntax; native property identity and flow remain evaluator decisions.

mod missing;

use super::{Expression, ExpressionRoot, Source, expression};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use crate::{Value, syntax};

pub(in crate::project::semantic) enum AccessKey {
    Name {
        name: String,
        private: bool,
        location: Location,
        declaration: super::DeclarationIdentity,
    },
    Element(Expression),
}

pub(in crate::project::semantic) struct AccessRecipe {
    pub(in crate::project::semantic) root: ExpressionRoot,
    pub(in crate::project::semantic) location: Location,
    pub(in crate::project::semantic) receiver: Expression,
    pub(in crate::project::semantic) key: AccessKey,
    pub(in crate::project::semantic) optional_chain: bool,
    pub(in crate::project::semantic) receiver_optional_root: bool,
    pub(in crate::project::semantic) receiver_optional_chain: bool,
    pub(in crate::project::semantic) outermost_chain: bool,
    pub(in crate::project::semantic) method_call: bool,
    pub(in crate::project::semantic) access_kind: syntax::AccessKind,
    pub(in crate::project::semantic) assignment_target: Option<Site>,
}

impl Source<'_, '_> {
    /// Syntax gates from `checkPropertyAccessibilityAtLocation`: this-property
    /// and this-initialized binding are distinct native predicates.
    pub(in crate::project::semantic) fn accessibility_this_initializer(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let node = ast.node(id);
        let parent = captured.parents[id.index()];
        let grandparent = parent.and_then(|parent| captured.parents[parent.index()]);
        let is_this = |id| ast.node(id).syntax_kind == SyntaxKind::ThisKeyword;
        Ok(matches!(
            node.syntax_kind,
            SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
        ) && syntax::id(node, "expression").is_some_and(is_this)
            || matches!(
                node.syntax_kind,
                SyntaxKind::PropertyAssignment | SyntaxKind::ShorthandPropertyAssignment
            ) && grandparent.is_some_and(|id| {
                let node = ast.node(id);
                node.syntax_kind == SyntaxKind::BinaryExpression
                    && syntax::id(node, "operatorToken")
                        .is_some_and(|id| ast.node(id).syntax_kind == SyntaxKind::EqualsToken)
                    && syntax::id(node, "right").is_some_and(is_this)
            })
            || parent.is_some_and(|parent| {
                ast.node(parent).syntax_kind == SyntaxKind::ObjectBindingPattern
            }) && grandparent.is_some_and(|id| {
                ast.node(id).syntax_kind == SyntaxKind::VariableDeclaration
                    && syntax::id(ast.node(id), "initializer").is_some_and(is_this)
            }))
    }

    /// `isNodeUsedDuringClassInitialization` stops at the first class or
    /// function boundary, including an arrow, after the constructor/property gate.
    pub(in crate::project::semantic) fn node_used_during_class_initialization(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let mut next = Some(captured.node_at(root.site)?);
        while let Some(id) = next {
            let node = ast.node(id);
            if node.syntax_kind == SyntaxKind::PropertyDeclaration
                || node.syntax_kind == SyntaxKind::Constructor
                    && syntax::node_is_present(ast, syntax::id(node, "body"))
            {
                return Ok(true);
            }
            if matches!(
                node.syntax_kind,
                SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
            ) || syntax::is_function_like_declaration_kind(node.syntax_kind)
            {
                return Ok(false);
            }
            next = captured.parents[id.index()];
        }
        Ok(false)
    }

    /// Ordinary TS branch of `isClassInstanceProperty`. Assignment-created
    /// JS properties are excluded by the retained source profile.
    pub(in crate::project::semantic) fn class_instance_property(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        Ok(node.syntax_kind == SyntaxKind::PropertyDeclaration
            && !syntax::has_modifier(ast, node, SyntaxKind::AccessorKeyword)
            && captured.parents[declaration.node.index()].is_some_and(|id| {
                matches!(
                    ast.node(id).syntax_kind,
                    SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                )
            }))
    }
    /// `checkPropertyAccessExpression` / `checkQualifiedName` /
    /// `checkElementAccessExpression` use
    /// the actual receiver and key nodes, including exact optional-chain roles.
    pub(in crate::project::semantic) fn access_recipe(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<AccessRecipe, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let node = ast.node(id);
        let qualified = node.syntax_kind == SyntaxKind::QualifiedName;
        let receiver = syntax::id(node, if qualified { "left" } else { "expression" })
            .ok_or_else(|| super::super::internal("access has no receiver"))?;
        let key = match node.syntax_kind {
            SyntaxKind::PropertyAccessExpression | SyntaxKind::QualifiedName => {
                let name = syntax::id(node, if qualified { "right" } else { "name" })
                    .ok_or_else(|| super::super::internal("access has no name"))?;
                AccessKey::Name {
                    name: access_name_text(inputs, root.file, ast, name)?,
                    private: ast.node(name).syntax_kind == SyntaxKind::PrivateIdentifier,
                    location: super::super::location(inputs, root.file, ast.token_span(name)),
                    declaration: super::DeclarationIdentity::new(root.file, name),
                }
            }
            SyntaxKind::ElementAccessExpression => {
                let key = syntax::id(node, "argumentExpression")
                    .ok_or_else(|| super::super::internal("element access has no key"))?;
                AccessKey::Element(expression(inputs, root.file, captured, key))
            }
            _ => {
                return Err(super::super::internal(
                    "access recipe has another syntax family",
                ));
            }
        };
        // `isMethodAccessForCall` removes parentheses only; the reference
        // call-role owner additionally admits tags and other outer expressions.
        let mut method = id;
        while let Some(parent) = parents[method.index()]
            && ast.node(parent).syntax_kind == SyntaxKind::ParenthesizedExpression
        {
            method = parent;
        }
        let method_call = parents[method.index()].is_some_and(|parent| {
            matches!(
                ast.node(parent).syntax_kind,
                SyntaxKind::CallExpression | SyntaxKind::NewExpression
            ) && syntax::id(ast.node(parent), "expression") == Some(method)
        });
        Ok(AccessRecipe {
            root,
            location: super::super::location(inputs, root.file, ast.token_span(id)),
            receiver: if qualified {
                super::type_query_operand(inputs, root.file, captured, parents, receiver)
            } else {
                expression(inputs, root.file, captured, receiver)
            },
            key,
            optional_chain: syntax::is_optional_chain(ast, id),
            receiver_optional_root: syntax::is_expression_of_optional_chain_root(
                ast,
                receiver,
                Some(id),
            ),
            receiver_optional_chain: syntax::is_optional_chain(ast, receiver),
            outermost_chain: syntax::is_outermost_optional_chain(ast, id, parents[id.index()]),
            method_call,
            access_kind: syntax::access_kind(ast, |id| parents[id.index()], id),
            assignment_target: syntax::assignment_target(ast, |id| parents[id.index()], id)
                .map(|id| Site::new(ast, id)),
        })
    }
}

pub(in crate::project::semantic) struct ConstantKeyRecipe {
    pub(in crate::project::semantic) initializer: Option<Expression>,
    pub(in crate::project::semantic) enum_name: Option<ExpressionRoot>,
}

pub(in crate::project::semantic) enum ElementAccessName {
    Text(String),
    Entity,
    None,
}

impl Source<'_, '_> {
    /// `tryGetNameFromEntityNameExpression`'s `hasOnlyExpressionInitializer`
    /// branch excludes binding elements. Annotation acquisition precedes this
    /// operation; before-use and initializer types remain evaluator questions.
    pub(in crate::project::semantic) fn constant_key_recipe(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<Option<ConstantKeyRecipe>, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if !matches!(
            node.syntax_kind,
            SyntaxKind::VariableDeclaration
                | SyntaxKind::Parameter
                | SyntaxKind::PropertyDeclaration
                | SyntaxKind::PropertyAssignment
                | SyntaxKind::EnumMember
        ) {
            return Ok(None);
        }
        Ok(Some(ConstantKeyRecipe {
            initializer: syntax::id(node, "initializer")
                .map(|value| expression(inputs, declaration.file, captured, value)),
            enum_name: if node.syntax_kind == SyntaxKind::EnumMember {
                syntax::id(node, "name").map(|name| ExpressionRoot {
                    file: declaration.file,
                    site: Site::new(ast, name),
                })
            } else {
                None
            },
        }))
    }

    /// Syntax half of `tryGetElementAccessExpressionName`: the established
    /// literal family precedes the established entity-expression predicate.
    pub(in crate::project::semantic) fn element_access_name_syntax(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ElementAccessName, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = captured.node_at(root.site)?;
        if syntax::is_string_or_numeric_literal_like(ast, node) {
            return self
                .try_property_name_text(root)?
                .map(ElementAccessName::Text)
                .ok_or_else(|| super::super::internal("literal index has no native text"));
        }
        Ok(if syntax::is_entity_name_expression(ast, node) {
            ElementAccessName::Entity
        } else {
            ElementAccessName::None
        })
    }
}

/// Native property-access `Name().Text()` has one exact-text owner, shared by
/// captured access checking and checker-created reference matching.
fn access_name_text(
    inputs: crate::project::semantic::inputs::CheckerInputs<'_>,
    file: crate::project::sources::FileId,
    ast: &crate::Ast<'_>,
    name: crate::NodeId,
) -> Result<String, AnalysisError> {
    let Some(Value::Text(text)) = ast.node(name).field("escapedText") else {
        return Err(super::super::unsupported_demand_at_location(
            inputs,
            &super::super::location(inputs, file, ast.token_span(name)),
            crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
            "access name has no exact native text",
        )
        .into());
    };
    Ok(text.to_string())
}
impl Source<'_, '_> {
    pub(in crate::project::semantic) fn access_name_text(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<String, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let name = captured.node_at(root.site)?;
        access_name_text(inputs, root.file, &captured.ast, name)
    }
}

impl Source<'_, '_> {
    /// `ast.GetContainingClass` walks actual parents, excluding the node itself.
    pub(in crate::project::semantic) fn containing_class(
        &mut self,
        node: super::DeclarationIdentity,
    ) -> Result<Option<super::DeclarationIdentity>, AnalysisError> {
        let captured = self.syntax(node.file)?;
        let mut next = captured.parents[node.node.index()];
        while let Some(id) = next {
            if matches!(
                captured.ast.node(id).syntax_kind,
                SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
            ) {
                return Ok(Some(super::DeclarationIdentity::new(node.file, id)));
            }
            next = captured.parents[id.index()];
        }
        Ok(None)
    }

    /// `getContainingClassExcludingClassDecorators` skips a decorated class
    /// only for a decorator on that class. Member decorators retain its scope.
    pub(in crate::project::semantic) fn private_identifier_containing_class(
        &mut self,
        node: super::DeclarationIdentity,
    ) -> Result<Option<super::DeclarationIdentity>, AnalysisError> {
        let captured = self.syntax(node.file)?;
        let mut next = captured.parents[node.node.index()];
        let mut start = node;
        while let Some(id) = next {
            let kind = captured.ast.node(id).syntax_kind;
            if matches!(
                kind,
                SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
            ) {
                break;
            }
            if kind == SyntaxKind::Decorator {
                let parent = captured.parents[id.index()];
                let skip = parent.filter(|&parent| {
                    matches!(
                        captured.ast.node(parent).syntax_kind,
                        SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                    )
                });
                start = super::DeclarationIdentity::new(node.file, skip.unwrap_or(id));
                break;
            }
            next = captured.parents[id.index()];
        }
        self.containing_class(start)
    }

    /// `ast.FindAncestor`/`IsNodeDescendantOf` compare actual retained nodes,
    /// including the starting node for the private-shadowing class test.
    pub(in crate::project::semantic) fn declaration_is_descendant(
        &mut self,
        node: super::DeclarationIdentity,
        ancestor: super::DeclarationIdentity,
    ) -> Result<bool, AnalysisError> {
        if node.file != ancestor.file {
            return Ok(false);
        }
        let captured = self.syntax(node.file)?;
        Ok(captured.node_is_descendant(node.node, ancestor.node))
    }
}

pub(in crate::project::semantic) struct PropertyAliasReference {
    pub(in crate::project::semantic) left: ExpressionRoot,
    pub(in crate::project::semantic) enum_member_parent: bool,
}

impl Source<'_, '_> {
    /// `markLinkedReferences(Property)` and `markPropertyAliasReferenced`
    /// syntax gates. The caller has already supplied the actual parent type.
    pub(in crate::project::semantic) fn property_alias_reference(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<PropertyAliasReference>, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let node = ast.node(id);
        if node.flags & crate::compiler::node::AMBIENT != 0 {
            return Ok(None);
        }
        let mut current = id;
        while let Some(parent) = captured.parents[current.index()] {
            if ast.node(parent).syntax_kind == SyntaxKind::ImportEqualsDeclaration {
                if syntax::id(ast.node(parent), "moduleReference") == Some(current) {
                    return Ok(None);
                }
                break;
            }
            current = parent;
        }
        let left = syntax::id(
            node,
            if node.syntax_kind == SyntaxKind::QualifiedName {
                "left"
            } else {
                "expression"
            },
        )
        .ok_or_else(|| super::super::internal("property alias reference lacks left expression"))?;
        if ast.node(left).syntax_kind != SyntaxKind::Identifier
            || syntax::is_this_identifier(ast, left)
        {
            return Ok(None);
        }
        Ok(Some(PropertyAliasReference {
            left: ExpressionRoot {
                file: root.file,
                site: Site::new(ast, left),
            },
            enum_member_parent: captured.parents[id.index()]
                .is_some_and(|parent| ast.node(parent).syntax_kind == SyntaxKind::EnumMember),
        }))
    }

    /// `isExportOrExportExpression` follows ancestors and only admits the
    /// entity expression of export assignment, or the two export-specifier names.
    pub(in crate::project::semantic) fn is_export_or_export_expression(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let mut current = Some(captured.node_at(root.site)?);
        while let Some(id) = current {
            let parent = captured.parents[id.index()];
            if let Some(parent) = parent {
                let node = ast.node(parent);
                if node.syntax_kind == SyntaxKind::ExportAssignment {
                    if syntax::id(node, "expression") == Some(id)
                        && syntax::is_entity_name_expression(ast, id)
                    {
                        return Ok(true);
                    }
                } else if node.syntax_kind == SyntaxKind::ExportSpecifier
                    && (syntax::id(node, "name") == Some(id)
                        || syntax::id(node, "propertyName") == Some(id))
                {
                    return Ok(true);
                }
            }
            current = parent;
        }
        Ok(false)
    }

    pub(in crate::project::semantic) fn reference_is_in_type_query(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(root.file)?;
        Ok(syntax::is_in_type_query(
            &captured.ast,
            |id| captured.parents[id.index()],
            captured.node_at(root.site)?,
        ))
    }

    /// `isSelfTypeAccess` only resolves the first identifier after the native
    /// IsEntityNameExpression gate; parentheses are deliberately not stripped.
    pub(in crate::project::semantic) fn self_type_access_identifier(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<ExpressionRoot>, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        if !syntax::is_entity_name_expression(ast, id) {
            return Ok(None);
        }
        let first = syntax::first_identifier(ast, id)
            .ok_or_else(|| super::super::internal("entity expression lacks first identifier"))?;
        Ok(Some(ExpressionRoot {
            file: root.file,
            site: Site::new(ast, first),
        }))
    }

    /// `markAliasSymbolAsReferenced` only follows internal import-equals
    /// module references, and uses their actual first identifier.
    pub(in crate::project::semantic) fn internal_alias_reference_identifier(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<Option<ExpressionRoot>, AnalysisError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if node.syntax_kind != SyntaxKind::ImportEqualsDeclaration
            || syntax::is_external_module_import_equals_declaration(ast, declaration.node)
        {
            return Ok(None);
        }
        let module = syntax::id(node, "moduleReference")
            .ok_or_else(|| super::super::internal("internal alias lacks module reference"))?;
        let first = syntax::first_identifier(ast, module)
            .ok_or_else(|| super::super::internal("internal alias lacks first identifier"))?;
        Ok(Some(ExpressionRoot {
            file: declaration.file,
            site: Site::new(ast, first),
        }))
    }

    /// `markPropertyAsReferenced` uses the first FunctionLike ancestor and its
    /// raw Symbol, not the enclosing flow container or a merged symbol.
    pub(in crate::project::semantic) fn property_reference_containing_function(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<super::DeclarationIdentity>, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let mut current = Some(captured.node_at(root.site)?);
        while let Some(id) = current {
            if syntax::is_function_like_declaration_kind(captured.ast.node(id).syntax_kind) {
                return Ok(Some(super::DeclarationIdentity::new(root.file, id)));
            }
            current = captured.parents[id.index()];
        }
        Ok(None)
    }
}

impl Source<'_, '_> {
    /// `ast.IsOptionalChain` observes the actual access node flag.
    pub(in crate::project::semantic) fn node_is_optional_chain(
        &mut self,
        node: super::DeclarationIdentity,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(node.file)?;
        Ok(syntax::is_optional_chain(&captured.ast, node.node))
    }
}

impl Source<'_, '_> {
    /// `isDeleteTarget` checks the direct parent after parentheses.
    pub(in crate::project::semantic) fn access_is_delete_target(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let mut node = captured.node_at(root.site)?;
        while let Some(parent) = captured.parents[node.index()] {
            if captured.ast.node(parent).syntax_kind != SyntaxKind::ParenthesizedExpression {
                return Ok(captured.ast.node(parent).syntax_kind == SyntaxKind::DeleteExpression);
            }
            node = parent;
        }
        Ok(false)
    }
}

impl Source<'_, '_> {
    /// `getThisParameterFromNodeContext` uses GetThisContainer with both
    /// inclusion flags false, then the actual first parameter of a function.
    pub(in crate::project::semantic) fn this_parameter_function_context(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<(ExpressionRoot, Option<super::DeclarationIdentity>)>, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = captured.node_at(root.site)?;
        let Some(function) = syntax::this_container(ast, &captured.parents, node, false, false)
            .filter(|&id| syntax::is_function_like_kind(ast.node(id).syntax_kind))
        else {
            return Ok(None);
        };
        let parameter = syntax::ids(ast.node(function), "parameters")
            .first()
            .copied()
            .filter(|&id| syntax::is_this_parameter(ast, id))
            .map(|id| super::DeclarationIdentity::new(root.file, id));
        Ok(Some((
            ExpressionRoot {
                file: root.file,
                site: Site::new(ast, function),
            },
            parameter,
        )))
    }
}

impl Source<'_, '_> {
    /// `isAssignmentToReadonlyEntity` strips parentheses from an access
    /// receiver only for its two raw identifier-symbol observations.
    pub(in crate::project::semantic) fn readonly_access_receiver_identifier(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<ExpressionRoot>, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = ast.node(captured.node_at(root.site)?);
        if !matches!(
            node.syntax_kind,
            SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
        ) {
            return Ok(None);
        }
        let receiver = syntax::id(node, "expression")
            .ok_or_else(|| super::super::internal("access lacks receiver"))?;
        let receiver = syntax::skip_parentheses(ast, receiver);
        Ok(
            (ast.node(receiver).syntax_kind == SyntaxKind::Identifier).then(|| ExpressionRoot {
                file: root.file,
                site: Site::new(ast, receiver),
            }),
        )
    }
}
