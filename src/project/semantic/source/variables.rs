//! Actual syntax inputs for variable-like and checked destructuring questions.
//! These recipes contain no inferred type and do not read initializer symbols.

use super::{
    DeclarationIdentity, Expression, ExpressionRoot, SignatureParameterRecipe, Source,
    TypeExpression,
};
use crate::project::semantic::model::DemandError;
use crate::references::Location;
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) enum VariableEarly {
    Ordinary,
    BindingElement,
    ForIn(Expression),
    ForOf {
        expression: ExpressionRoot,
        asynchronous: bool,
    },
}

pub(in crate::project::semantic) enum PropertyInference {
    None,
    AutomaticFlow,
    BaseClass,
}

pub(in crate::project::semantic) struct VariableFacts {
    pub(in crate::project::semantic) location: Location,
    pub(in crate::project::semantic) kind: K,
    pub(in crate::project::semantic) early: VariableEarly,
    pub(in crate::project::semantic) annotation: Option<TypeExpression>,
    pub(in crate::project::semantic) initializer: Option<Expression>,
    pub(in crate::project::semantic) pattern: Option<DeclarationIdentity>,
    pub(in crate::project::semantic) catch: bool,
    pub(in crate::project::semantic) property: bool,
    pub(in crate::project::semantic) optional: bool,
    pub(in crate::project::semantic) constant: bool,
    /// `isDeclarationReadonly`, never true for a parameter property.
    pub(in crate::project::semantic) readonly: bool,
    pub(in crate::project::semantic) automatic_eligible: bool,
    pub(in crate::project::semantic) empty_array: bool,
    pub(in crate::project::semantic) rest: bool,
    pub(in crate::project::semantic) root_parameter: bool,
    pub(in crate::project::semantic) parent: Option<DeclarationIdentity>,
    /// The declaration is in a JavaScript file (`IsInJSFile`).
    pub(in crate::project::semantic) javascript: bool,
}

pub(in crate::project::semantic) enum AssignmentPatternRecipe {
    ForOf {
        expression: ExpressionRoot,
        asynchronous: bool,
    },
    Binary(ExpressionRoot),
    Object {
        pattern: ExpressionRoot,
        index: usize,
    },
    Array {
        pattern: ExpressionRoot,
        index: usize,
    },
}

pub(in crate::project::semantic) struct BindingElementFacts {
    pub(in crate::project::semantic) pattern: DeclarationIdentity,
    pub(in crate::project::semantic) parent: DeclarationIdentity,
    pub(in crate::project::semantic) index: usize,
    pub(in crate::project::semantic) object: bool,
    pub(in crate::project::semantic) name: ExpressionRoot,
    pub(in crate::project::semantic) root_annotated: bool,
    pub(in crate::project::semantic) ambient_parameter: bool,
}

pub(in crate::project::semantic) struct PatternElement {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) kind: K,
    pub(in crate::project::semantic) name: Option<ExpressionRoot>,
    pub(in crate::project::semantic) initializer: Option<ExpressionRoot>,
    pub(in crate::project::semantic) operand: Option<ExpressionRoot>,
    pub(in crate::project::semantic) left: Option<ExpressionRoot>,
    pub(in crate::project::semantic) rest: bool,
    pub(in crate::project::semantic) default_value: bool,
}

pub(in crate::project::semantic) enum DestructuringParent {
    None,
    Expression(ExpressionRoot),
    Nested(DeclarationIdentity),
}
pub(in crate::project::semantic) enum DestructuringKey {
    Name(ExpressionRoot),
    Index(String),
}

impl Source<'_, '_> {
    /// `reportImplicitAny`'s parameter-name diagnostic performs a quiet Type
    /// lookup only for actual parameters of these three signature forms.
    /// Type keywords short-circuit that lookup, including escaped spellings.
    pub(in crate::project::semantic) fn implicit_any_parameter_type_lookup(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<crate::ast::Span>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if node.syntax_kind != K::Parameter {
            return Ok(None);
        }
        let Some(name) = syntax::id(node, "name") else {
            return Ok(None);
        };
        if ast.node(name).syntax_kind != K::Identifier {
            return Ok(None);
        }
        let Some(parent) = captured.parents[declaration.node.index()] else {
            return Err(super::super::internal("implicit-any parameter has no parent").into());
        };
        let parent = ast.node(parent);
        if !matches!(
            parent.syntax_kind,
            K::CallSignature | K::MethodSignature | K::FunctionType
        ) || !syntax::ids(parent, "parameters").contains(&declaration.node)
        {
            return Ok(None);
        }
        let name = ast.identifier_name(name).ok_or_else(|| {
            super::super::internal("implicit-any parameter identifier has no name")
        })?;
        if crate::lexer::identifier_keyword_syntax(name).is_some_and(syntax::is_type_node_kind) {
            return Ok(None);
        }
        Ok(Some(node.span))
    }

    /// `isValidESSymbolDeclaration` is distinct from readonly/const widening.
    pub(in crate::project::semantic) fn valid_es_symbol_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        Ok(match node.syntax_kind {
            K::VariableDeclaration => {
                syntax::combined_node_flags(ast, &captured.parents, declaration.node)
                    & crate::compiler::node::CONST
                    != 0
                    && syntax::id(node, "name")
                        .is_some_and(|name| ast.node(name).syntax_kind == K::Identifier)
                    && captured.parents[declaration.node.index()].is_some_and(|parent| {
                        ast.node(parent).syntax_kind == K::VariableDeclarationList
                            && captured.parents[parent.index()].is_some_and(|parent| {
                                ast.node(parent).syntax_kind == K::VariableStatement
                            })
                    })
            }
            K::PropertyDeclaration => {
                syntax::has_modifier(ast, node, K::ReadonlyKeyword)
                    && syntax::has_modifier(ast, node, K::StaticKeyword)
            }
            K::PropertySignature => syntax::has_modifier(ast, node, K::ReadonlyKeyword),
            _ => false,
        })
    }
    /// `getContextualTypeForBindingElement` excludes pattern names and
    /// computed names whose expression is not string/numeric literal-like.
    pub(in crate::project::semantic) fn contextual_binding_name_eligible(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let node = ast.node(id);
        Ok(!matches!(
            node.syntax_kind,
            K::ObjectBindingPattern | K::ArrayBindingPattern
        ) && (node.syntax_kind != K::ComputedPropertyName
            || syntax::id(node, "expression").is_some_and(|expression| {
                syntax::is_string_or_numeric_literal_like(ast, expression)
            })))
    }

    /// `declarationBelongsToPrivateAmbientMember` uses the root declaration,
    /// then its containing member for parameters; it performs no type query.
    pub(in crate::project::semantic) fn declaration_is_private_ambient(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let mut id = declaration.node;
        while ast.node(id).syntax_kind == K::BindingElement {
            id = captured.parents[id.index()]
                .and_then(|pattern| captured.parents[pattern.index()])
                .ok_or_else(|| super::super::internal("binding element has no root declaration"))?;
        }
        if ast.node(id).syntax_kind == K::Parameter {
            id = captured.parents[id.index()]
                .ok_or_else(|| super::super::internal("parameter has no member"))?;
        }
        self.is_private_within_ambient(DeclarationIdentity {
            file: declaration.file,
            node: id,
        })
    }
    /// `checkReferenceExpression` emits diagnostics only; its Boolean gates
    /// the subsequent reporting relation. Assertions and parentheses strip,
    /// while actual optional-chain flags reject the resulting reference.
    pub(in crate::project::semantic) fn assignment_reference_is_valid(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(root.file)?;
        let id =
            syntax::skip_outer_expressions_ex(&captured.ast, captured.node_at(root.site)?, false);
        let node = captured.ast.node(id);
        Ok(matches!(
            node.syntax_kind,
            K::Identifier | K::PropertyAccessExpression | K::ElementAccessExpression
        ) && node.flags & crate::compiler::node::OPTIONAL_CHAIN == 0)
    }

    pub(in crate::project::semantic) fn initializer_declaration(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<DeclarationIdentity, DemandError> {
        let captured = self.syntax(root.file)?;
        let id = captured.node_at(root.site)?;
        let parent = captured.parents[id.index()]
            .ok_or_else(|| super::super::internal("declaration initializer has no declaration"))?;
        if syntax::id(captured.ast.node(parent), "initializer") != Some(id) {
            return Err(super::super::internal(
                "initializer recipe is not its declaration's actual initializer",
            )
            .into());
        }
        Ok(DeclarationIdentity::new(root.file, parent))
    }

    /// `getTypeForVariableLikeDeclaration` selects actual constructor bodies
    /// or static blocks before the own Ambient modifier's base-class fallback.
    /// A bodyless signature supplies no flow producer. Enclosing ambient context
    /// does not substitute for this property's syntactic ModifierFlags().
    pub(in crate::project::semantic) fn property_inference(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<PropertyInference, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        let flags = syntax::modifier_flags(ast, node);
        let Some(parent) = captured.parents[declaration.node.index()] else {
            return Err(super::super::internal("property inference lacks its class parent").into());
        };
        let automatic = if flags & syntax::modifiers::STATIC != 0 {
            syntax::ids(ast.node(parent), "members")
                .iter()
                .any(|&member| ast.node(member).syntax_kind == K::ClassStaticBlockDeclaration)
        } else {
            syntax::find_constructor_declaration(ast, parent).is_some()
        };
        Ok(if automatic {
            PropertyInference::AutomaticFlow
        } else if flags & syntax::modifiers::AMBIENT != 0 {
            PropertyInference::BaseClass
        } else {
            PropertyInference::None
        })
    }
    pub(in crate::project::semantic) fn destructuring_expression_is_left_hand_side(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(root.file)?;
        Ok(syntax::is_left_hand_side_expression(
            &captured.ast,
            captured.node_at(root.site)?,
        ))
    }
    /// `getParentElementAccess` follows the actual two-level ancestor and
    /// preserves nested synthetic access construction as its own question.
    pub(in crate::project::semantic) fn destructuring_parent(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<DestructuringParent, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let Some(parent) = captured.parents[declaration.node.index()] else {
            return Ok(DestructuringParent::None);
        };
        let Some(ancestor) = captured.parents[parent.index()] else {
            return Ok(DestructuringParent::None);
        };
        let expression = |node| {
            DestructuringParent::Expression(ExpressionRoot {
                file: declaration.file,
                site: crate::project::binding::flow::Site::new(ast, node),
            })
        };
        Ok(match ast.node(ancestor).syntax_kind {
            K::BindingElement | K::PropertyAssignment => {
                DestructuringParent::Nested(DeclarationIdentity::new(declaration.file, ancestor))
            }
            K::ArrayLiteralExpression => {
                DestructuringParent::Nested(DeclarationIdentity::new(declaration.file, parent))
            }
            K::VariableDeclaration => syntax::id(ast.node(ancestor), "initializer")
                .map_or(DestructuringParent::None, expression),
            K::BinaryExpression => syntax::id(ast.node(ancestor), "right")
                .map_or(DestructuringParent::None, expression),
            _ => DestructuringParent::None,
        })
    }

    /// `getDestructuringPropertyName` acquires a literal key only after the
    /// synthetic parent's FlowNode is present. Array positions are exact text.
    pub(in crate::project::semantic) fn destructuring_key(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<DestructuringKey>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        let Some(parent) = captured.parents[declaration.node.index()] else {
            return Ok(None);
        };
        if (node.syntax_kind == K::BindingElement
            && ast.node(parent).syntax_kind == K::ObjectBindingPattern)
            || matches!(
                node.syntax_kind,
                K::PropertyAssignment | K::ShorthandPropertyAssignment
            )
        {
            return Ok(syntax::id(node, "propertyName")
                .or_else(|| syntax::id(node, "name"))
                .map(|name| {
                    DestructuringKey::Name(ExpressionRoot {
                        file: declaration.file,
                        site: crate::project::binding::flow::Site::new(ast, name),
                    })
                }));
        }
        if matches!(
            ast.node(parent).syntax_kind,
            K::ArrayLiteralExpression | K::ArrayBindingPattern
        ) {
            let index = syntax::ids(ast.node(parent), "elements")
                .iter()
                .position(|&element| element == declaration.node)
                .ok_or_else(|| super::super::internal("destructuring child absent from array"))?;
            return Ok(Some(DestructuringKey::Index(index.to_string())));
        }
        Ok(None)
    }
    /// `getBindingElementTypeFromParentType` asks the immediate pattern and
    /// root annotation independently of the child's own annotation/default.
    pub(in crate::project::semantic) fn binding_element_facts(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<BindingElementFacts, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let pattern = parents[declaration.node.index()]
            .ok_or_else(|| super::super::internal("binding element has no pattern"))?;
        let parent = parents[pattern.index()]
            .ok_or_else(|| super::super::internal("binding pattern has no declaration"))?;
        let index = syntax::ids(ast.node(pattern), "elements")
            .iter()
            .position(|&element| element == declaration.node)
            .ok_or_else(|| super::super::internal("binding element absent from pattern"))?;
        let node = ast.node(declaration.node);
        let name = syntax::id(node, "propertyName")
            .or_else(|| syntax::id(node, "name"))
            .ok_or_else(|| super::super::internal("binding element has no key"))?;
        let mut root = parent;
        while ast.node(root).syntax_kind == K::BindingElement {
            root = parents[root.index()]
                .and_then(|pattern| parents[pattern.index()])
                .ok_or_else(|| super::super::internal("nested binding element has no root"))?;
        }
        Ok(BindingElementFacts {
            pattern: DeclarationIdentity::new(declaration.file, pattern),
            parent: DeclarationIdentity::new(declaration.file, parent),
            index,
            object: ast.node(pattern).syntax_kind == K::ObjectBindingPattern,
            name: ExpressionRoot {
                file: declaration.file,
                site: crate::project::binding::flow::Site::new(ast, name),
            },
            root_annotated: syntax::id(ast.node(root), "type").is_some(),
            ambient_parameter: node.flags & crate::compiler::node::AMBIENT != 0
                && ast.node(root).syntax_kind == K::Parameter,
        })
    }

    /// Source list order for object/array binding and assignment patterns.
    pub(in crate::project::semantic) fn pattern_elements(
        &mut self,
        pattern: DeclarationIdentity,
    ) -> Result<Vec<PatternElement>, DemandError> {
        let captured = self.syntax(pattern.file)?;
        let ast = &captured.ast;
        let kind = ast.node(pattern.node).syntax_kind;
        let list = if matches!(kind, K::ObjectLiteralExpression) {
            "properties"
        } else {
            "elements"
        };
        syntax::ids(ast.node(pattern.node), list)
            .iter()
            .map(|&node| pattern_element(ast, pattern.file, node))
            .collect()
    }

    /// `assignBindingElementTypes` walks actual Name fields, independently
    /// of the PropertyNameOrName projection used for property lookup.
    pub(in crate::project::semantic) fn binding_pattern_assignments(
        &mut self,
        pattern: DeclarationIdentity,
    ) -> Result<Vec<(DeclarationIdentity, Option<DeclarationIdentity>)>, DemandError> {
        let captured = self.syntax(pattern.file)?;
        let ast = &captured.ast;
        let mut result = Vec::new();
        for &element in syntax::ids(ast.node(pattern.node), "elements") {
            let Some(name) = syntax::id(ast.node(element), "name") else {
                continue;
            };
            let nested = if ast.node(name).syntax_kind == K::Identifier {
                None
            } else {
                Some(DeclarationIdentity::new(pattern.file, name))
            };
            result.push((DeclarationIdentity::new(pattern.file, element), nested));
        }
        Ok(result)
    }

    pub(in crate::project::semantic) fn destructuring_target(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<PatternElement, DemandError> {
        let captured = self.syntax(root.file)?;
        pattern_element(&captured.ast, root.file, captured.node_at(root.site)?)
    }

    /// Syntax observations for `getTypeForVariableLikeDeclaration`. The early
    /// loop/binding dispatch is retained separately from annotation acquisition.
    pub(in crate::project::semantic) fn variable_facts(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<VariableFacts, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let node = ast.node(declaration.node);
        let parent = parents[declaration.node.index()];
        let root = |node| ExpressionRoot {
            file: declaration.file,
            site: crate::project::binding::flow::Site::new(ast, node),
        };
        let required = |node, field| {
            syntax::id(ast.node(node), field).ok_or_else(|| {
                super::super::internal("variable-like syntax lacks its required child")
            })
        };
        let early = if node.syntax_kind == K::BindingElement {
            VariableEarly::BindingElement
        } else if node.syntax_kind == K::VariableDeclaration {
            match parent.and_then(|parent| parents[parent.index()]) {
                Some(statement) if ast.node(statement).syntax_kind == K::ForInStatement => {
                    VariableEarly::ForIn(super::expression(
                        inputs,
                        declaration.file,
                        captured,
                        required(statement, "expression")?,
                    ))
                }
                Some(statement) if ast.node(statement).syntax_kind == K::ForOfStatement => {
                    VariableEarly::ForOf {
                        expression: root(required(statement, "expression")?),
                        asynchronous: syntax::id(ast.node(statement), "awaitModifier").is_some(),
                    }
                }
                _ => VariableEarly::Ordinary,
            }
        } else {
            VariableEarly::Ordinary
        };
        let name = syntax::id(node, "name");
        let pattern = name.filter(|&name| {
            matches!(
                ast.node(name).syntax_kind,
                K::ObjectBindingPattern | K::ArrayBindingPattern
            )
        });
        let initializer = syntax::id(node, "initializer");
        let mut outer = declaration.node;
        while ast.node(outer).syntax_kind == K::BindingElement {
            let Some(pattern) = parents[outer.index()] else {
                break;
            };
            let Some(declaration) = parents[pattern.index()] else {
                break;
            };
            outer = declaration;
        }
        let flags = syntax::combined_node_flags(ast, parents, declaration.node);
        let modifiers = syntax::combined_modifier_flags(ast, parents, declaration.node);
        let property = node.syntax_kind == K::PropertySignature
            || node.syntax_kind == K::PropertyDeclaration
                && modifiers & syntax::modifiers::ACCESSOR == 0;
        Ok(VariableFacts {
            location: super::super::location(
                inputs,
                declaration.file,
                ast.token_span(declaration.node),
            ),
            kind: node.syntax_kind,
            early,
            annotation: syntax::id(node, "type")
                .map(|ty| super::type_expression(inputs, declaration.file, captured, parents, ty)),
            initializer: initializer.map(|initializer| {
                super::expression(inputs, declaration.file, captured, initializer)
            }),
            pattern: pattern.map(|pattern| DeclarationIdentity::new(declaration.file, pattern)),
            catch: parents[outer.index()]
                .is_some_and(|parent| ast.node(parent).syntax_kind == K::CatchClause),
            property,
            optional: syntax::id(node, "questionToken").is_some(),
            constant: syntax::is_var_const_like_flags(flags),
            readonly: syntax::is_declaration_readonly(ast, parents, declaration.node),
            automatic_eligible: node.syntax_kind == K::VariableDeclaration
                && pattern.is_none()
                && modifiers & syntax::modifiers::EXPORT == 0
                && node.flags & crate::compiler::node::AMBIENT == 0,
            empty_array: initializer.is_some_and(|initializer| {
                ast.node(initializer).syntax_kind == K::ArrayLiteralExpression
                    && syntax::ids(ast.node(initializer), "elements").is_empty()
            }),
            rest: syntax::id(node, "dotDotDotToken").is_some(),
            root_parameter: ast.node(outer).syntax_kind == K::Parameter,
            parent: parent.map(|parent| DeclarationIdentity::new(declaration.file, parent)),
            javascript: inputs.in_javascript_file(declaration.file),
        })
    }

    /// The existing parameter recipe is also used by the raw variable-like
    /// operation. It retains signature display/call facts independently.
    pub(in crate::project::semantic) fn variable_parameter_recipe(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<SignatureParameterRecipe, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        super::signature_parameter_recipe(
            inputs,
            declaration.file,
            captured,
            &captured.parents,
            declaration.node,
        )
        .map_err(|message| {
            super::super::unsupported_demand_at_location(
                inputs,
                &super::super::location(
                    inputs,
                    declaration.file,
                    captured.ast.token_span(declaration.node),
                ),
                crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
                message,
            )
        })
    }

    /// Syntax input to `isNullOrUndefined`; resolving an Identifier belongs
    /// to the evaluator's shared getResolvedSymbol completion.
    pub(in crate::project::semantic) fn nullish_initializer(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<NullishInitializer, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let Some(initializer) = syntax::id(captured.ast.node(declaration.node), "initializer")
        else {
            return Ok(NullishInitializer::AbsentOrNull);
        };
        let initializer = syntax::skip_parentheses(&captured.ast, initializer);
        Ok(match captured.ast.node(initializer).syntax_kind {
            K::NullKeyword => NullishInitializer::AbsentOrNull,
            K::Identifier => NullishInitializer::Identifier(ExpressionRoot {
                file: declaration.file,
                site: crate::project::binding::flow::Site::new(&captured.ast, initializer),
            }),
            _ => NullishInitializer::Other,
        })
    }

    /// `getTypeOfAssignmentPattern` dispatches on the immediate actual parent.
    pub(in crate::project::semantic) fn assignment_pattern_recipe(
        &mut self,
        pattern: ExpressionRoot,
    ) -> Result<AssignmentPatternRecipe, DemandError> {
        let captured = self.syntax(pattern.file)?;
        let ast = &captured.ast;
        let node = captured.node_at(pattern.site)?;
        let parent = captured.parents[node.index()]
            .ok_or_else(|| super::super::internal("assignment pattern has no parent"))?;
        let root = |node| ExpressionRoot {
            file: pattern.file,
            site: crate::project::binding::flow::Site::new(ast, node),
        };
        let child = |node, field| {
            syntax::id(ast.node(node), field)
                .ok_or_else(|| super::super::internal("assignment pattern parent lacks child"))
        };
        Ok(match ast.node(parent).syntax_kind {
            K::ForOfStatement => AssignmentPatternRecipe::ForOf {
                expression: root(child(parent, "expression")?),
                asynchronous: syntax::id(ast.node(parent), "awaitModifier").is_some(),
            },
            K::BinaryExpression => AssignmentPatternRecipe::Binary(root(child(parent, "right")?)),
            K::PropertyAssignment => {
                let object = captured.parents[parent.index()]
                    .ok_or_else(|| super::super::internal("property assignment has no object"))?;
                let index = syntax::ids(ast.node(object), "properties")
                    .iter()
                    .position(|&property| property == parent)
                    .ok_or_else(|| {
                        super::super::internal("assignment property absent from object")
                    })?;
                AssignmentPatternRecipe::Object {
                    pattern: root(object),
                    index,
                }
            }
            K::ArrayLiteralExpression => {
                let index = syntax::ids(ast.node(parent), "elements")
                    .iter()
                    .position(|&element| element == node)
                    .ok_or_else(|| {
                        super::super::internal("assignment element absent from array")
                    })?;
                AssignmentPatternRecipe::Array {
                    pattern: root(parent),
                    index,
                }
            }
            _ => {
                return Err(super::super::internal(
                    "assignment pattern has an invalid native parent",
                )
                .into());
            }
        })
    }
}

/// `hasDefaultValue` and the exact declaration/expression child roles needed
/// by the checked destructuring workers. A rest marker is not an initializer.
fn pattern_element(
    ast: &crate::Ast<'_>,
    file: crate::project::sources::FileId,
    node: crate::NodeId,
) -> Result<PatternElement, DemandError> {
    let data = ast.node(node);
    let root = |node| ExpressionRoot {
        file,
        site: crate::project::binding::flow::Site::new(ast, node),
    };
    let assignment = data.syntax_kind == K::BinaryExpression
        && syntax::id(data, "operatorToken")
            .is_some_and(|token| ast.node(token).syntax_kind == K::EqualsToken);
    let initializer =
        syntax::id(data, "initializer").or_else(|| syntax::id(data, "objectAssignmentInitializer"));
    let default_value = syntax::has_default_value(ast, node);
    Ok(PatternElement {
        declaration: DeclarationIdentity::new(file, node),
        kind: data.syntax_kind,
        name: syntax::id(data, "propertyName")
            .or_else(|| syntax::id(data, "name"))
            .map(root),
        initializer: initializer.map(root),
        operand: syntax::id(data, "expression").map(root),
        left: assignment
            .then(|| syntax::id(data, "left").map(root))
            .flatten(),
        rest: syntax::id(data, "dotDotDotToken").is_some()
            || matches!(data.syntax_kind, K::SpreadElement | K::SpreadAssignment),
        default_value,
    })
}

pub(in crate::project::semantic) enum NullishInitializer {
    AbsentOrNull,
    Identifier(ExpressionRoot),
    Other,
}
