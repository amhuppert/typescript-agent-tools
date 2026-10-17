//! Declaration-local source checks retain syntax without acquiring child types.
use super::super::{DeclarationIdentity, ExpressionRoot, SignatureRoot, Source, TypeExpression};
use crate::project::binding::flow::Site;
use crate::project::semantic::model::DemandError;
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) struct VariableLikeCheck {
    pub(in crate::project::semantic) name: Option<ExpressionRoot>,
    pub(in crate::project::semantic) decorator: Option<ExpressionRoot>,
    pub(in crate::project::semantic) annotation: Option<ExpressionRoot>,
    pub(in crate::project::semantic) initializer: Option<ExpressionRoot>,
    pub(in crate::project::semantic) computed_name: Option<Site>,
    pub(in crate::project::semantic) parameter_without_body: bool,
    pub(in crate::project::semantic) pattern: Option<VariablePatternCheck>,
    pub(in crate::project::semantic) binding: Option<VariableBindingCheck>,
    pub(in crate::project::semantic) for_in: bool,
}

pub(in crate::project::semantic) struct VariablePatternCheck {
    pub(in crate::project::semantic) elements: Vec<ExpressionRoot>,
    pub(in crate::project::semantic) array: bool,
    pub(in crate::project::semantic) empty: bool,
    pub(in crate::project::semantic) ambient: bool,
}

pub(in crate::project::semantic) struct VariableBindingCheck {
    pub(in crate::project::semantic) parent: DeclarationIdentity,
    pub(in crate::project::semantic) rest: bool,
    pub(in crate::project::semantic) object: bool,
    pub(in crate::project::semantic) renamed_in_signature: bool,
    pub(in crate::project::semantic) computed_property: Option<ExpressionRoot>,
    pub(in crate::project::semantic) key: Option<ExpressionRoot>,
    pub(in crate::project::semantic) super_initializer: bool,
}

pub(in crate::project::semantic) struct MemberCheck {
    pub(in crate::project::semantic) declaration: DeclarationIdentity,
    pub(in crate::project::semantic) computed_name: Option<Site>,
    /// Only grammar's eligible entity-name expression acquires a type. Other
    /// dynamic-name grammar rejection is syntax-only.
    pub(in crate::project::semantic) grammar_name: Option<Site>,
    pub(in crate::project::semantic) signature: Option<SignatureRoot>,
}

impl Source<'_, '_> {
    /// `checkVariableLikeDeclaration` uses actual declaration-local annotation,
    /// initializer and parameter body/this facts, not the canonical value node.
    pub(in crate::project::semantic) fn variable_like_check(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<VariableLikeCheck, DemandError> {
        let legacy_decorators = self.inputs.semantic_options().experimental_decorators;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        let child = |field| {
            syntax::id(node, field).map(|id| ExpressionRoot {
                file: declaration.file,
                site: Site::new(ast, id),
            })
        };
        let computed_name = syntax::id(node, "name")
            .filter(|&id| ast.node(id).syntax_kind == K::ComputedPropertyName)
            .map(|id| Site::new(ast, id));
        let root = |id| ExpressionRoot {
            file: declaration.file,
            site: Site::new(ast, id),
        };
        let parameter_root = syntax::root_declaration(ast, &captured.parents, declaration.node);
        let parameter_function = if ast.node(parameter_root).syntax_kind == K::Parameter {
            Some(captured.parents[parameter_root.index()].ok_or_else(|| {
                super::super::super::internal("parameter source check lacks containing signature")
            })?)
        } else {
            None
        };
        let parameter_without_body = parameter_function.is_some_and(|parent| {
            !syntax::node_is_present(ast, syntax::id(ast.node(parent), "body"))
        });
        let is_pattern = |id| {
            matches!(
                ast.node(id).syntax_kind,
                K::ObjectBindingPattern | K::ArrayBindingPattern
            )
        };
        let pattern = syntax::id(node, "name")
            .filter(|&id| is_pattern(id))
            .map(|name| {
                let elements = syntax::ids(ast.node(name), "elements");
                VariablePatternCheck {
                    elements: elements.iter().copied().map(root).collect(),
                    array: ast.node(name).syntax_kind == K::ArrayBindingPattern,
                    empty: !elements
                        .iter()
                        .any(|&element| syntax::id(ast.node(element), "name").is_some()),
                    ambient: syntax::is_in_ambient_or_type_node(
                        ast,
                        |id| captured.parents[id.index()],
                        declaration.node,
                    ),
                }
            });
        let binding = if node.syntax_kind == K::BindingElement {
            let pattern = captured.parents[declaration.node.index()].ok_or_else(|| {
                super::super::super::internal("binding source check lacks pattern")
            })?;
            let parent = captured.parents[pattern.index()].ok_or_else(|| {
                super::super::super::internal("binding source check lacks parent declaration")
            })?;
            let property = syntax::id(node, "propertyName");
            Some(VariableBindingCheck {
                parent: DeclarationIdentity::new(declaration.file, parent),
                rest: syntax::id(node, "dotDotDotToken").is_some(),
                object: ast.node(pattern).syntax_kind == K::ObjectBindingPattern,
                renamed_in_signature: property.is_some()
                    && syntax::id(node, "name")
                        .is_some_and(|name| ast.node(name).syntax_kind == K::Identifier)
                    && parameter_without_body,
                computed_property: property
                    .filter(|&name| ast.node(name).syntax_kind == K::ComputedPropertyName)
                    .map(root),
                key: property
                    .or_else(|| syntax::id(node, "name"))
                    .filter(|&name| !is_pattern(name))
                    .map(root),
                super_initializer: syntax::id(ast.node(parent), "initializer").is_some_and(
                    |initializer| ast.node(initializer).syntax_kind == K::SuperKeyword,
                ),
            })
        } else {
            None
        };
        let for_in = captured.parents[declaration.node.index()]
            .and_then(|parent| captured.parents[parent.index()])
            .is_some_and(|parent| ast.node(parent).syntax_kind == K::ForInStatement);
        Ok(VariableLikeCheck {
            name: child("name"),
            decorator: syntax::node_can_be_decorated(
                ast,
                &captured.parents,
                declaration.node,
                legacy_decorators,
            )
            .then(|| {
                syntax::ids(node, "modifiers")
                    .iter()
                    .copied()
                    .find(|&id| ast.node(id).syntax_kind == K::Decorator)
                    .map(root)
            })
            .flatten(),
            annotation: if node.syntax_kind == K::BindingElement {
                None
            } else {
                child("type")
            },
            initializer: child("initializer"),
            computed_name,
            parameter_without_body,
            pattern,
            binding,
            for_in,
        })
    }

    /// `checkPropertySignature` / `checkMethodDeclaration` grammar asks the
    /// computed name before annotation only through isLateBindableName. The
    /// type-literal modifier gate is complete: only one readonly modifier on
    /// a property or index signature is allowed.
    pub(in crate::project::semantic) fn type_member_check(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<MemberCheck, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        let node = ast.node(id);
        let declaration = DeclarationIdentity {
            file: root.file,
            node: id,
        };
        let name = syntax::id(node, "name")
            .filter(|&id| ast.node(id).syntax_kind == K::ComputedPropertyName);
        let computed_name = name.map(|id| Site::new(ast, id));
        let signature = matches!(
            node.syntax_kind,
            K::MethodSignature | K::GetAccessor | K::SetAccessor
        )
        .then(|| {
            super::super::signature_root(ast, root.file, id).map_err(super::super::super::internal)
        })
        .transpose()?;
        let modifiers = syntax::ids(node, "modifiers");
        let modifiers_valid = modifiers.is_empty()
            || node.syntax_kind == K::PropertySignature
                && modifiers.len() == 1
                && ast.node(modifiers[0]).syntax_kind == K::ReadonlyKeyword;
        let grammar_name = if modifiers_valid
            && matches!(node.syntax_kind, K::PropertySignature | K::MethodSignature)
            && (node.syntax_kind != K::MethodSignature || signature_grammar_valid(ast, id))
        {
            name.filter(|&name| {
                syntax::id(ast.node(name), "expression")
                    .is_some_and(|expr| syntax::is_entity_name_expression(ast, expr))
            })
            .map(|id| Site::new(ast, id))
        } else {
            None
        };
        Ok(MemberCheck {
            declaration,
            computed_name,
            grammar_name,
            signature,
        })
    }

    /// `checkGrammarIndexSignatureParameters` acquires the key only after its
    /// syntax-only diagnostic gates. A trailing comma reports but does not
    /// short-circuit this operation.
    pub(in crate::project::semantic) fn index_signature_grammar_key(
        &mut self,
        root: SignatureRoot,
    ) -> Result<Option<TypeExpression>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site())?;
        let node = ast.node(id);
        if node.syntax_kind != K::IndexSignature {
            return Ok(None);
        }
        let modifiers = syntax::ids(node, "modifiers");
        if !(modifiers.is_empty()
            || modifiers.len() == 1 && ast.node(modifiers[0]).syntax_kind == K::ReadonlyKeyword)
        {
            return Ok(None);
        }
        let parameters = syntax::ids(node, "parameters");
        if parameters.len() != 1 {
            return Ok(None);
        }
        let parameter = ast.node(parameters[0]);
        if syntax::id(parameter, "dotDotDotToken").is_some()
            || !syntax::ids(parameter, "modifiers").is_empty()
            || syntax::id(parameter, "questionToken").is_some()
            || syntax::id(parameter, "initializer").is_some()
        {
            return Ok(None);
        }
        Ok(syntax::id(parameter, "type").map(|ty| {
            super::super::type_expression(inputs, root.file, captured, &captured.parents, ty)
        }))
    }

    /// The duplicate-index worker intentionally has fewer syntax gates than
    /// index grammar and retains each actual declaration before deduplication.
    pub(in crate::project::semantic) fn duplicate_index_key(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<TypeExpression>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if node.syntax_kind != K::IndexSignature {
            return Ok(None);
        }
        let parameters = syntax::ids(node, "parameters");
        Ok(if parameters.len() == 1 {
            syntax::id(ast.node(parameters[0]), "type").map(|ty| {
                super::super::type_expression(
                    inputs,
                    declaration.file,
                    captured,
                    &captured.parents,
                    ty,
                )
            })
        } else {
            None
        })
    }

    pub(in crate::project::semantic) fn declaration_members(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Vec<ExpressionRoot>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        Ok(syntax::ids(ast.node(declaration.node), "members")
            .iter()
            .map(|&id| ExpressionRoot {
                file: declaration.file,
                site: Site::new(ast, id),
            })
            .collect())
    }

    /// `checkFunctionOrConstructorSymbolWorker` observes the whole canonical
    /// family before deciding whether an implementation compatibility check
    /// exists. A bodyless type-literal overload group has no type demands here.
    pub(in crate::project::semantic) fn type_member_overload_has_implementation(
        &mut self,
        declarations: &[DeclarationIdentity],
    ) -> Result<bool, DemandError> {
        for &declaration in declarations {
            let captured = self.syntax(declaration.file)?;
            let node = captured.ast.node(declaration.node);
            if matches!(
                node.syntax_kind,
                K::FunctionDeclaration | K::MethodDeclaration | K::MethodSignature | K::Constructor
            ) && syntax::id(node, "body").is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// The semantic gate of `checkGrammarFunctionLikeDeclaration` in bodyless
/// type members. Diagnostic-only parameter errors prevent the later dynamic
/// name grammar question; child source checking still occurs independently.
fn signature_grammar_valid(ast: &crate::Ast<'_>, id: crate::NodeId) -> bool {
    let node = ast.node(id);
    if matches!(node.field("typeParameters"), Some(crate::Value::Nodes(nodes)) if nodes.is_empty())
    {
        return false;
    }
    let mut optional_seen = false;
    let parameters = syntax::ids(node, "parameters");
    for (index, &parameter) in parameters.iter().enumerate() {
        let parameter = ast.node(parameter);
        let rest = syntax::id(parameter, "dotDotDotToken").is_some();
        let optional = syntax::id(parameter, "questionToken").is_some();
        let initializer = syntax::id(parameter, "initializer").is_some();
        if rest && (index + 1 != parameters.len() || optional || initializer) {
            return false;
        }
        if optional
            && initializer
            && syntax::id(parameter, "questionToken")
                .is_some_and(|token| ast.node(token).flags & crate::compiler::node::REPARSED == 0)
        {
            return false;
        }
        if optional_seen && !optional && !initializer && !rest {
            return false;
        }
        optional_seen |= optional;
    }
    true
}
