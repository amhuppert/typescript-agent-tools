//! Actual syntax facts requested by native diagnostic node serialization.
use super::{DeclarationIdentity, Source};
use crate::project::binding::BoundSymbol;
use crate::project::semantic::model::DemandError;
use crate::references::Location;
use crate::syntax;
use crate::syntax_kind::SyntaxKind as K;

impl Source<'_, '_> {
    /// `VisitEachChild` order for cloneBindingName, including children of
    /// initializers which the clone removes only after visiting them.
    pub(in crate::project::semantic) fn diagnostic_syntax_children(
        &mut self,
        node: DeclarationIdentity,
    ) -> Result<Vec<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(node.file)?;
        Ok(captured
            .ast
            .children(node.node)
            .map(|child| DeclarationIdentity::new(node.file, child))
            .collect())
    }

    /// The retained binding list's native HasTrailingComma fact. The parser
    /// does not publish NodeList metadata, so its final element boundary and
    /// the shared scanner recover only this punctuation observation.
    pub(in crate::project::semantic) fn diagnostic_binding_trailing_comma(
        &mut self,
        node: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let control = self.control.clone();
        let captured = self.syntax(node.file)?;
        let Some(&last) = syntax::ids(captured.ast.node(node.node), "elements").last() else {
            return Ok(false);
        };
        let mut scanner =
            crate::lexer::Lexer::new(captured.ast.source(), captured.ast.node(last).span.end);
        let token = scanner
            .next_controlled(control.as_ref())
            .map_err(super::super::resource_failure)?;
        Ok(token.raw == ",")
    }

    /// `GetFirstIdentifier` consumes the actual entity expression.
    pub(in crate::project::semantic) fn diagnostic_first_identifier(
        &mut self,
        node: DeclarationIdentity,
    ) -> Result<DeclarationIdentity, DemandError> {
        let captured = self.syntax(node.file)?;
        syntax::first_identifier(&captured.ast, node.node)
            .map(|node_id| DeclarationIdentity::new(node.file, node_id))
            .ok_or_else(|| {
                super::super::internal("computed-name entity lacks first identifier").into()
            })
    }

    pub(in crate::project::semantic) fn diagnostic_lookup_node(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<(K, crate::Span), DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok((node.syntax_kind, node.span))
    }

    /// `resolveEntityName`'s leftmost Identifier/This text is source identity,
    /// never a printed entity name or a reconstructed qualified expression.
    pub(in crate::project::semantic) fn diagnostic_identifier_text(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<String, DemandError> {
        let captured = self.syntax(declaration.file)?;
        if captured.ast.node(declaration.node).syntax_kind == K::ThisKeyword {
            return Ok("this".into());
        }
        captured
            .ast
            .identifier_name(declaration.node)
            .map(str::to_owned)
            .ok_or_else(|| {
                super::super::internal("diagnostic name lookup requires Identifier or This").into()
            })
    }

    /// `enterSignatureScope` retains the actual signature declaration, not
    /// merely whether one exists. Fake scopes never become source identities.
    pub(in crate::project::semantic) fn diagnostic_signature_declaration(
        &mut self,
        root: super::SignatureRoot,
    ) -> Result<DeclarationIdentity, DemandError> {
        let captured = self.syntax(root.file)?;
        let node = captured.node_at(root.site())?;
        Ok(DeclarationIdentity::new(root.file, node))
    }

    /// `enterNewScope`'s `bindPattern` intentionally returns after its first
    /// element in pinned 7.0.2. Some(None) is a matched pattern whose first
    /// path is empty/omitted; None means this is not a pattern parameter.
    pub(in crate::project::semantic) fn diagnostic_parameter_binding(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<Option<DeclarationIdentity>>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        if ast.node(declaration.node).syntax_kind != K::Parameter {
            return Ok(None);
        }
        let Some(mut name) = syntax::id(ast.node(declaration.node), "name") else {
            return Ok(None);
        };
        if !matches!(
            ast.node(name).syntax_kind,
            K::ObjectBindingPattern | K::ArrayBindingPattern
        ) {
            return Ok(None);
        }
        loop {
            let Some(&element) = syntax::ids(ast.node(name), "elements").first() else {
                return Ok(Some(None));
            };
            match ast.node(element).syntax_kind {
                K::OmittedExpression => return Ok(Some(None)),
                K::BindingElement => {
                    if let Some(inner) = syntax::id(ast.node(element), "name")
                        && matches!(
                            ast.node(inner).syntax_kind,
                            K::ObjectBindingPattern | K::ArrayBindingPattern
                        )
                    {
                        name = inner;
                    } else {
                        return Ok(Some(Some(DeclarationIdentity::new(
                            declaration.file,
                            element,
                        ))));
                    }
                }
                _ => {
                    return Err(super::super::internal(
                        "native fake-scope pattern has an invalid element",
                    )
                    .into());
                }
            }
        }
    }

    /// `isDefaultBindingContext` used by getNameOfSymbolAsWritten is ambient
    /// modules or SourceFiles, not every lexical namespace/module container.
    pub(in crate::project::semantic) fn diagnostic_default_binding_context(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let mut next = Some(declaration.node);
        while let Some(node) = next {
            if captured.ast.node(node).syntax_kind == K::SourceFile
                || syntax::is_ambient_module(&captured.ast, node)
            {
                return Ok(Some(DeclarationIdentity::new(declaration.file, node)));
            }
            next = captured.parents[node.index()];
        }
        Ok(None)
    }

    /// Native `isOptionalDeclaration` is exactly HasQuestionToken. This is an
    /// actual declaration-token observation, not signature optional arity.
    pub(in crate::project::semantic) fn diagnostic_optional_declaration(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(syntax::id(captured.ast.node(declaration.node), "questionToken").is_some())
    }
    /// `getExternalModuleContainer` / `FindAncestor(hasExternalModuleSymbol)`
    /// observes ambient-module and actual external/CommonJS-file ancestors.
    pub(in crate::project::semantic) fn diagnostic_external_module_container(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<Option<super::DeclarationIdentity>, DemandError> {
        let external = self
            .inputs
            .bindings
            .reference_file_is_external_or_commonjs(declaration.file);
        let captured = self.syntax(declaration.file)?;
        let mut node = Some(declaration.node);
        while let Some(id) = node {
            if syntax::is_ambient_module(&captured.ast, id)
                || captured.ast.node(id).syntax_kind == K::SourceFile && external
            {
                return Ok(Some(super::DeclarationIdentity::new(declaration.file, id)));
            }
            node = captured.parents[id.index()];
        }
        Ok(None)
    }
    /// `getNameOfSymbolAsWritten`'s nameless-declaration fallback. Its parent
    /// variable name precedes the anonymous class/function spelling.
    pub(in crate::project::semantic) fn diagnostic_nameless_symbol_name(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<Option<(String, bool)>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        if let Some(parent) = captured.parents[declaration.node.index()]
            && ast.node(parent).syntax_kind == K::VariableDeclaration
        {
            return Ok(Some((
                syntax::declaration_name_to_string(
                    ast,
                    syntax::id(ast.node(parent), "name"),
                    captured.control.as_ref(),
                ),
                false,
            )));
        }
        Ok(match ast.node(declaration.node).syntax_kind {
            K::ClassExpression => Some(("(Anonymous class)".into(), true)),
            K::FunctionExpression | K::ArrowFunction => Some(("(Anonymous function)".into(), true)),
            _ => None,
        })
    }

    /// `hasNonGlobalAugmentationExternalModuleSymbol` uses the actual module
    /// name or source-file module attachment; it does not resolve any imports.
    pub(in crate::project::semantic) fn diagnostic_external_module_declaration(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let external = self
            .inputs
            .bindings
            .reference_file_is_external_or_commonjs(declaration.file);
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok(
            syntax::is_module_with_string_literal_name(&captured.ast, declaration.node)
                || node.syntax_kind == K::SourceFile && external,
        )
    }

    /// `GetNonAugmentationDeclaration` excludes both independently named
    /// AST predicates. A global augmentation need not be external-module one.
    pub(in crate::project::semantic) fn diagnostic_non_augmentation_declaration(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let global = captured.ast.node(declaration.node).flags
            & crate::compiler::node::GLOBAL_AUGMENTATION
            != 0;
        Ok(!self.reference_is_external_augmentation(declaration)? && !global)
    }
    /// `FirstNonNil(GetNameOfDeclaration)` followed by
    /// `DeclarationNameToString`, used after the symbol/nameType decisions.
    pub(in crate::project::semantic) fn diagnostic_written_name(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<Option<String>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let name = syntax::id(captured.ast.node(declaration.node), "name");
        Ok(name.map(|name| {
            syntax::declaration_name_to_string(&captured.ast, Some(name), captured.control.as_ref())
        }))
    }

    /// `createAccessFromSymbolChain` copies a computed EntityName directly.
    /// Native IsEntityName is Identifier/QualifiedName, deliberately narrower
    /// than isEntityNameExpression (which also permits property access).
    pub(in crate::project::semantic) fn diagnostic_computed_entity_name(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<String>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let Some(name) = syntax::id(captured.ast.node(declaration.node), "name") else {
            return Ok(None);
        };
        if captured.ast.node(name).syntax_kind != K::ComputedPropertyName {
            return Ok(None);
        }
        let Some(expression) = syntax::id(captured.ast.node(name), "expression") else {
            return Ok(None);
        };
        Ok(matches!(
            captured.ast.node(expression).syntax_kind,
            K::Identifier | K::QualifiedName
        )
        .then(|| {
            syntax::declaration_name_to_string(
                &captured.ast,
                Some(expression),
                captured.control.as_ref(),
            )
        }))
    }
    /// `isSingleQuotedStringNamed` observes the actual string token. Its
    /// scanner quote is retained in the token text independently of its value.
    pub(in crate::project::semantic) fn diagnostic_single_quoted_name(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let Some((file, site)) = self.declaration_name_site(declaration)? else {
            return Ok(false);
        };
        if site.kind != K::StringLiteral {
            return Ok(false);
        }
        let captured = self.syntax(file)?;
        let id = captured.node_at(site)?;
        Ok(captured.ast.source().as_bytes()[captured.ast.token_span(id).start] == b'\'')
    }
    /// `typeReferenceToTypeNode` observes written type arguments only on its
    /// actual deferred TypeReference node. Other node kinds and absent lists
    /// both permit native trailing-default elision.
    pub(in crate::project::semantic) fn diagnostic_reference_argument_count(
        &mut self,
        declaration: super::DeclarationIdentity,
    ) -> Result<Option<usize>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok(if node.syntax_kind == K::TypeReference {
            match node.field("typeArguments") {
                Some(crate::Value::Nodes(arguments)) => Some(arguments.len()),
                _ => None,
            }
        } else {
            None
        })
    }

    /// `getTypeAliasForTypeLiteral`: only a TypeLiteral's first declaration,
    /// after walking up parenthesized types, can supply this circular fallback.
    /// The evaluator acquires that alias declaration's symbol.
    pub(in crate::project::semantic) fn diagnostic_type_literal_alias(
        &mut self,
        symbol: BoundSymbol,
        consumer: &Location,
    ) -> Result<Option<super::DeclarationIdentity>, DemandError> {
        let bindings = self.bindings();
        let inputs = self.inputs;
        if !bindings
            .type_symbol_flags(symbol)
            .map_err(|error| super::super::binding_failure_at(inputs, consumer, error))?
            .type_literal
        {
            return Ok(None);
        }
        let declarations = bindings
            .native_declarations(symbol)
            .map_err(|error| super::super::binding_failure_at(inputs, consumer, error))?;
        let Some(declaration) = declarations.first().copied() else {
            return Ok(None);
        };
        let identity = self.declaration_identity(declaration)?;
        let captured = self.syntax(identity.file)?;
        let mut parent = captured.parents[identity.node.index()];
        while let Some(node) = parent {
            if captured.ast.node(node).syntax_kind != K::ParenthesizedType {
                break;
            }
            parent = captured.parents[node.index()];
        }
        let Some(parent) = parent
            .filter(|&parent| captured.ast.node(parent).syntax_kind == K::TypeAliasDeclaration)
        else {
            return Ok(None);
        };
        Ok(Some(super::DeclarationIdentity {
            file: identity.file,
            node: parent,
        }))
    }
}
