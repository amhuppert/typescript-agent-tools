//! Checker property-name questions retain actual syntax and exact payloads.

use super::{Expression, ExpressionRoot, Source, TypeExpression, expression, type_expression};
use crate::analysis::{AnalysisError, SemanticFailureReason};
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use crate::{Value, syntax};

/// Scalar admission for syntax property-name observations. Native scanner
/// bytes, rather than the public UTF-16 projection, decide representability.
pub(super) fn scalar_name_text<'a>(
    ast: &crate::Ast<'_>,
    node: &'a crate::Node<'_>,
) -> Result<Option<&'a str>, ()> {
    if let Some(bytes) = ast.cooked_literal(node) {
        return std::str::from_utf8(bytes).map(Some).map_err(|_| ());
    }
    match node.field("escapedText").or_else(|| node.field("text")) {
        Some(Value::Text(text)) => Ok(Some(text)),
        Some(Value::Utf16(_)) => Err(()),
        _ => Ok(None),
    }
}

pub(in crate::project::semantic) struct PropertyNameRecipe {
    pub(in crate::project::semantic) location: Location,
    pub(in crate::project::semantic) form: PropertyNameForm,
}

pub(in crate::project::semantic) struct DeclarationNameBinding {
    pub(in crate::project::semantic) name: Option<Site>,
    pub(in crate::project::semantic) dynamic: bool,
    pub(in crate::project::semantic) late_bindable: bool,
}

pub(in crate::project::semantic) enum PropertyNameForm {
    String(String),
    Expression {
        expression: Expression,
        computed: bool,
        forbidden_in: bool,
    },
    Never,
}

pub(in crate::project::semantic) enum EffectivePropertyName {
    Text(String),
    Dynamic(ExpressionRoot),
}

pub(in crate::project::semantic) struct ContextualObjectElement {
    pub(in crate::project::semantic) annotation: Option<TypeExpression>,
    pub(in crate::project::semantic) object: ExpressionRoot,
    pub(in crate::project::semantic) name: Option<Site>,
    pub(in crate::project::semantic) dynamic: bool,
    pub(in crate::project::semantic) computed: Option<Expression>,
    pub(in crate::project::semantic) method_in_with: bool,
}

impl Source<'_, '_> {
    /// `ast.TryGetTextOfPropertyName`: syntax text only, including the narrow
    /// literal-computed branch. Semantic computed names use their own owner.
    pub(in crate::project::semantic) fn try_property_name_text(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<String>, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let mut id = captured.node_at(root.site)?;
        if ast.node(id).syntax_kind == SyntaxKind::ComputedPropertyName {
            let Some(expression) = syntax::id(ast.node(id), "expression") else {
                return Err(super::super::internal(
                    "computed property name has no expression",
                ));
            };
            if !syntax::is_string_or_numeric_literal_like(ast, expression) {
                return Ok(None);
            }
            id = expression;
        }
        let text = |id| {
            let node = ast.node(id);
            match scalar_name_text(ast, node) {
                Ok(Some(text)) => Ok(text.to_owned()),
                Err(()) => Err(super::super::failure_at_location(
                    inputs,
                    &super::super::location(inputs, root.file, ast.token_span(id)),
                    SemanticFailureReason::UnrepresentableIdentity,
                    "property name contains unpaired UTF-16 units",
                )),
                _ => Err(super::super::internal(
                    "property name lacks its native text",
                )),
            }
        };
        match ast.node(id).syntax_kind {
            SyntaxKind::Identifier
            | SyntaxKind::PrivateIdentifier
            | SyntaxKind::StringLiteral
            | SyntaxKind::NumericLiteral
            | SyntaxKind::BigIntLiteral
            | SyntaxKind::NoSubstitutionTemplateLiteral => text(id).map(Some),
            SyntaxKind::JsxNamespacedName => {
                let node = ast.node(id);
                let namespace = syntax::id(node, "namespace")
                    .ok_or_else(|| super::super::internal("JSX name lacks namespace"))?;
                let name = syntax::id(node, "name")
                    .ok_or_else(|| super::super::internal("JSX name lacks local name"))?;
                Ok(Some(format!("{}:{}", text(namespace)?, text(name)?)))
            }
            _ => Ok(None),
        }
    }

    /// `GetPropertyNameForPropertyNameNode` supplies the syntax-only arm of
    /// `getEffectivePropertyNameForPropertyNameNode`. Signed numeric names
    /// retain the prefix and literal text; arbitrary computed names require
    /// the separate flow-only expression question.
    pub(in crate::project::semantic) fn effective_property_name_syntax(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<EffectivePropertyName, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let mut id = captured.node_at(root.site)?;
        let mut minus = false;
        if ast.node(id).syntax_kind == SyntaxKind::ComputedPropertyName {
            let computed = syntax::id(ast.node(id), "expression")
                .ok_or_else(|| super::super::internal("computed name has no expression"))?;
            if syntax::is_dynamic_name(ast, id) {
                return Ok(EffectivePropertyName::Dynamic(ExpressionRoot {
                    file: root.file,
                    site: Site::new(ast, computed),
                }));
            }
            id = computed;
            if ast.node(id).syntax_kind == SyntaxKind::PrefixUnaryExpression {
                minus = syntax::unary_operator(ast.node(id)) == Some(SyntaxKind::MinusToken);
                id = syntax::id(ast.node(id), "operand")
                    .ok_or_else(|| super::super::internal("signed property name has no operand"))?;
            }
        }
        let node = ast.node(id);
        let Ok(Some(text)) = scalar_name_text(ast, node) else {
            return Err(super::super::failure_at_location(
                inputs,
                &super::super::location(inputs, root.file, ast.token_span(id)),
                SemanticFailureReason::UnrepresentableIdentity,
                "effective property name has no exact scalar spelling",
            ));
        };
        Ok(EffectivePropertyName::Text(if minus {
            format!("-{text}")
        } else {
            text.to_string()
        }))
    }

    /// Syntax half of `isNumericName` and `isSymbolWithComputedName`.
    /// Numeric spelling is an ASCII predicate, so a non-scalar UTF-16 literal
    /// is false without converting or changing its identity.
    pub(in crate::project::semantic) fn index_property_name_syntax(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<(Option<ExpressionRoot>, bool), AnalysisError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let node = ast.node(id);
        let computed = if node.syntax_kind == SyntaxKind::ComputedPropertyName {
            syntax::id(node, "expression").map(|node| ExpressionRoot {
                file: root.file,
                site: Site::new(ast, node),
            })
        } else {
            None
        };
        let numeric = if matches!(
            node.syntax_kind,
            SyntaxKind::Identifier | SyntaxKind::NumericLiteral | SyntaxKind::StringLiteral
        ) {
            match scalar_name_text(ast, node) {
                Ok(Some(value)) => crate::numbers::numeric_literal_name_value(value).is_some(),
                Err(()) => false,
                _ => {
                    return Err(super::super::internal(
                        "numeric property name lacks its actual text",
                    ));
                }
            }
        } else {
            false
        };
        Ok((computed, numeric))
    }

    /// `HasDynamicName` / `isLateBindableAST` retain the actual name question;
    /// its eligible expression type remains evaluator-owned.
    pub(in crate::project::semantic) fn declaration_name_binding(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<DeclarationNameBinding, AnalysisError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let name = syntax::id(ast.node(declaration.node), "name");
        Ok(DeclarationNameBinding {
            name: name.map(|name| Site::new(ast, name)),
            dynamic: name.is_some_and(|name| syntax::is_dynamic_name(ast, name)),
            late_bindable: name.is_some_and(|name| syntax::is_late_bindable_ast(ast, name)),
        })
    }

    /// The syntax gate and actual checked child of `isLateBindableName`.
    pub(in crate::project::semantic) fn late_bindable_name_expression(
        &mut self,
        node: super::DeclarationIdentity,
    ) -> Result<Option<(ExpressionRoot, bool)>, AnalysisError> {
        let captured = self.syntax(node.file)?;
        if !syntax::is_late_bindable_ast(&captured.ast, node.node) {
            return Ok(None);
        }
        let computed = captured.ast.node(node.node).syntax_kind == SyntaxKind::ComputedPropertyName;
        let field = if computed {
            "expression"
        } else {
            "argumentExpression"
        };
        let expression = syntax::id(captured.ast.node(node.node), field)
            .ok_or_else(|| super::super::internal("late-bindable syntax lost its expression"))?;
        Ok(Some((
            ExpressionRoot {
                file: node.file,
                site: Site::new(&captured.ast, expression),
            },
            computed,
        )))
    }
    /// Syntax of `getContextualTypeForObjectLiteralElement`, `HasDynamicName`
    /// and `isLateBindableAST`. Checker name types remain lazy dependencies.
    pub(in crate::project::semantic) fn contextual_object_element(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ContextualObjectElement, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let ast = &captured.ast;
        let node = ast.node(id);
        let object = captured.parents[id.index()]
            .ok_or_else(|| super::super::internal("object element lacks parent"))?;
        let method =
            syntax::is_object_literal_method(ast, |node| captured.parents[node.index()], id);
        let annotation = if method {
            None
        } else {
            syntax::id(node, "type")
                .map(|ty| type_expression(inputs, root.file, captured, &captured.parents, ty))
        };
        let name = syntax::id(node, "name");
        let computed = name
            .filter(|&name| ast.node(name).syntax_kind == SyntaxKind::ComputedPropertyName)
            .and_then(|name| syntax::id(ast.node(name), "expression"));
        let dynamic = name.is_some_and(|name| syntax::is_dynamic_name(ast, name));
        Ok(ContextualObjectElement {
            annotation,
            object: ExpressionRoot {
                file: root.file,
                site: Site::new(ast, object),
            },
            name: name.map(|name| Site::new(ast, name)),
            dynamic,
            computed: computed.map(|expr| expression(inputs, root.file, captured, expr)),
            method_in_with: method && node.flags & crate::compiler::node::IN_WITH_STATEMENT != 0,
        })
    }

    /// `getLiteralTypeFromPropertyName` / `GetPropertyNameForPropertyNameNode`:
    /// checker name construction differs from the binder declaration-key
    /// question. Numeric and computed names retain their actual checking sites.
    pub(in crate::project::semantic) fn property_name_recipe(
        &mut self,
        file: FileId,
        site: Site,
    ) -> Result<PropertyNameRecipe, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let id = captured.node_at(site)?;
        let node = ast.node(id);
        let location = super::super::location(inputs, file, ast.token_span(id));
        let form = match node.syntax_kind {
            SyntaxKind::PrivateIdentifier => PropertyNameForm::Never,
            SyntaxKind::NumericLiteral => PropertyNameForm::Expression {
                expression: expression(inputs, file, captured, id),
                computed: false,
                forbidden_in: false,
            },
            SyntaxKind::ComputedPropertyName => {
                let operand = syntax::id(node, "expression").ok_or_else(|| {
                    super::super::internal("computed name has no actual expression")
                })?;
                let member = parents[id.index()];
                let container = member.and_then(|member| parents[member.index()]);
                // `checkComputedPropertyName` has a direct BinaryExpression
                // test here, deliberately without removing parentheses.
                let forbidden_in = container.is_some_and(|container| {
                    matches!(
                        ast.node(container).syntax_kind,
                        SyntaxKind::TypeLiteral
                            | SyntaxKind::InterfaceDeclaration
                            | SyntaxKind::ClassDeclaration
                            | SyntaxKind::ClassExpression
                    )
                }) && member.is_some_and(|member| {
                    !matches!(
                        ast.node(member).syntax_kind,
                        SyntaxKind::GetAccessor | SyntaxKind::SetAccessor
                    )
                }) && ast.node(operand).syntax_kind
                    == SyntaxKind::BinaryExpression
                    && syntax::id(ast.node(operand), "operatorToken")
                        .is_some_and(|token| ast.node(token).syntax_kind == SyntaxKind::InKeyword);
                PropertyNameForm::Expression {
                    expression: expression(inputs, file, captured, operand),
                    computed: true,
                    forbidden_in,
                }
            }
            SyntaxKind::Identifier
            | SyntaxKind::StringLiteral
            | SyntaxKind::NoSubstitutionTemplateLiteral
            | SyntaxKind::BigIntLiteral
            | SyntaxKind::JsxNamespacedName => {
                let value = scalar_name_text(ast, node).map_err(|()| {
                    super::super::failure_at_location(
                        inputs,
                        &location,
                        SemanticFailureReason::UnrepresentableIdentity,
                        "property-name identity has no exact scalar payload",
                    )
                })?;
                let Some(value) = value else {
                    return Err(super::super::unsupported_demand_at_location(
                        inputs,
                        &location,
                        SemanticFailureReason::UnsupportedSemanticForm,
                        "property-name identity has no exact scalar payload",
                    )
                    .into());
                };
                PropertyNameForm::String(value.to_string())
            }
            _ if syntax::is_expression(ast, id) => PropertyNameForm::Expression {
                expression: expression(inputs, file, captured, id),
                computed: false,
                forbidden_in: false,
            },
            _ => PropertyNameForm::Never,
        };
        Ok(PropertyNameRecipe { location, form })
    }
}
