//! Syntax inputs to getTargetOfAliasDeclaration. Resolution remains with the
//! existing binding name/module owners and evaluator symbol/type factories.
use super::super::model::DemandError;
use super::{DeclarationIdentity, ExpressionRoot, Source, TypeExpression, type_expression};
use crate::project::binding::flow::Site;
use crate::{syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) enum SymbolAliasTarget {
    Expression {
        root: ExpressionRoot,
        class: bool,
        entity: bool,
    },
    External {
        name: Option<String>,
    },
    InternalImportEquals(ExpressionRoot),
    LocalExport(Option<ExpressionRoot>),
    NamespaceExport(Option<DeclarationIdentity>),
    Bound,
}

impl Source<'_, '_> {
    /// `ast.IsAliasSymbolDeclaration`, over the admitted syntax: of the
    /// JavaScript-only binder products, the one `module.exports = name`
    /// binding admits; require-initialized variables and other assignment
    /// aliases are refused at loading.
    pub(in crate::project::semantic) fn is_alias_symbol_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        Ok(match node.syntax_kind {
            K::ImportEqualsDeclaration
            | K::NamespaceExportDeclaration
            | K::NamespaceImport
            | K::NamespaceExport
            | K::ImportSpecifier
            | K::ExportSpecifier => true,
            K::ImportClause => syntax::id(node, "name").is_some(),
            K::ExportAssignment => syntax::id(node, "expression").is_some_and(|expression| {
                syntax::is_entity_name_expression(ast, expression)
                    || ast.node(expression).syntax_kind == K::ClassExpression
            }),
            K::BinaryExpression => {
                matches!(
                    syntax::assignment_declaration_kind(
                        ast,
                        declaration.node,
                        crate::javascript_syntax::is_javascript(ast)
                    ),
                    syntax::AssignmentDeclarationKind::ModuleExports
                        | syntax::AssignmentDeclarationKind::ExportsProperty
                ) && syntax::id(node, "right").is_some_and(|right| {
                    syntax::is_entity_name_expression(ast, right)
                        || ast.node(right).syntax_kind == K::ClassExpression
                })
            }
            _ => false,
        })
    }

    /// `getExternalModuleMember` supplies its enclosing import/export
    /// declaration to errorNoModuleMemberSymbol, not the selected name token.
    pub(in crate::project::semantic) fn alias_external_container(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<DeclarationIdentity, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let mut node = declaration.node;
        loop {
            if matches!(
                captured.ast.node(node).syntax_kind,
                K::ImportDeclaration | K::ExportDeclaration
            ) {
                return Ok(DeclarationIdentity {
                    file: declaration.file,
                    node,
                });
            }
            node = captured.parents[node.index()].ok_or_else(|| {
                super::super::internal("external alias lacks import/export container")
            })?;
        }
    }

    /// `errorNoModuleMemberSymbol` uses PropertyNameOrName, preserving the
    /// identifier/string-literal distinction that gates spelling suggestions.
    pub(in crate::project::semantic) fn alias_diagnostic_name(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<(String, bool), DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        let name = syntax::id(node, "propertyName")
            .or_else(|| syntax::id(node, "name"))
            .ok_or_else(|| super::super::internal("alias diagnostic has no declaration name"))?;
        Ok((
            super::reference_imports::exact_text(inputs, declaration.file, ast, name)?.to_owned(),
            ast.node(name).syntax_kind == K::Identifier,
        ))
    }

    /// `reportNonDefaultExport` probes actual ExportDeclaration module
    /// requests in order. This only associates syntax; resolution stays lazy.
    pub(in crate::project::semantic) fn export_declaration_module_site(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<crate::project::binding::ReferenceModuleSite>, DemandError> {
        let bindings = self.inputs.bindings;

        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if node.syntax_kind != K::ExportDeclaration {
            return Ok(None);
        }
        let Some(literal) = syntax::id(node, "moduleSpecifier") else {
            return Ok(None);
        };
        super::reference_import_tracker::retained_module_site(
            bindings,
            captured,
            declaration.file,
            literal,
        )?
        .map(Some)
        .ok_or_else(|| {
            super::super::internal("export declaration lacks its retained module request").into()
        })
    }

    /// `resolveESModuleSymbol` retains the actual moduleSpecifier.Parent for
    /// exportTypeLinks, independently from the alias declaration's identity.
    pub(in crate::project::semantic) fn module_specifier_parent(
        &mut self,
        site: crate::project::binding::ReferenceModuleSite,
    ) -> Result<ExpressionRoot, DemandError> {
        let facts = self.inputs.bindings.reference_module_site(site);
        if facts.implicit {
            return Err(
                super::super::internal("implicit module request has no captured parent").into(),
            );
        }
        let captured = self.syntax(site.file())?;
        let literal =
            super::reference_import_tracker::captured_module_literal(captured, facts.span)?;
        let parent = captured.parents[literal.index()]
            .ok_or_else(|| super::super::internal("module specifier lacks actual parent"))?;
        Ok(ExpressionRoot {
            file: site.file(),
            site: Site::new(&captured.ast, parent),
        })
    }

    pub(in crate::project::semantic) fn symbol_alias_target(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<SymbolAliasTarget, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if let Some(expression) = match node.syntax_kind {
            K::ExportAssignment => syntax::id(node, "expression"),
            K::BinaryExpression => syntax::id(node, "right"),
            K::PropertyAssignment => syntax::id(node, "initializer"),
            _ => None,
        } {
            let kind = ast.node(expression).syntax_kind;
            return Ok(SymbolAliasTarget::Expression {
                root: ExpressionRoot {
                    file: declaration.file,
                    site: Site::new(ast, expression),
                },
                class: kind == K::ClassExpression,
                entity: matches!(kind, K::Identifier | K::QualifiedName)
                    || syntax::is_entity_name_expression(ast, expression),
            });
        }
        let external = match node.syntax_kind {
            K::ImportEqualsDeclaration => {
                syntax::is_external_module_import_equals_declaration(ast, declaration.node)
            }
            K::ImportClause | K::NamespaceImport | K::NamespaceExport | K::ImportSpecifier => true,
            K::ExportSpecifier => captured.parents[declaration.node.index()]
                .and_then(|clause| captured.parents[clause.index()])
                .is_some_and(|export| syntax::id(ast.node(export), "moduleSpecifier").is_some()),
            _ => false,
        };
        if external {
            let name = if matches!(node.syntax_kind, K::ImportSpecifier | K::ExportSpecifier) {
                syntax::id(node, "propertyName")
                    .or_else(|| syntax::id(node, "name"))
                    .map(|name| {
                        super::reference_imports::exact_text(inputs, declaration.file, ast, name)
                    })
                    .transpose()?
            } else {
                None
            };
            return Ok(SymbolAliasTarget::External { name });
        }
        if node.syntax_kind == K::ImportEqualsDeclaration {
            let name = syntax::id(node, "moduleReference").ok_or_else(|| {
                super::super::internal("internal import-equals has no module reference")
            })?;
            return Ok(SymbolAliasTarget::InternalImportEquals(ExpressionRoot {
                file: declaration.file,
                site: Site::new(ast, name),
            }));
        }
        if node.syntax_kind == K::ExportSpecifier {
            let name = syntax::id(node, "propertyName")
                .or_else(|| syntax::id(node, "name"))
                .ok_or_else(|| super::super::internal("local export lacks its source name"))?;
            return Ok(SymbolAliasTarget::LocalExport(
                (ast.node(name).syntax_kind != K::StringLiteral).then(|| ExpressionRoot {
                    file: declaration.file,
                    site: Site::new(ast, name),
                }),
            ));
        }
        if node.syntax_kind == K::NamespaceExportDeclaration {
            let parent = captured.parents[declaration.node.index()]
                .filter(|parent| syntax::can_have_symbol(ast.node(*parent).syntax_kind));
            return Ok(SymbolAliasTarget::NamespaceExport(parent.map(|node| {
                DeclarationIdentity {
                    file: declaration.file,
                    node,
                }
            })));
        }
        Ok(SymbolAliasTarget::Bound)
    }

    /// `isReadonlyAssignmentDeclaration` only considers actual calls. Its
    /// binder producer guarantees the descriptor is the third argument.
    pub(in crate::project::semantic) fn readonly_assignment_descriptor(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        if node.syntax_kind != K::CallExpression {
            return Ok(None);
        }
        let arguments = syntax::ids(node, "arguments");
        let argument = arguments.get(2).copied().ok_or_else(|| {
            super::super::internal("readonly assignment symbol lacks property descriptor")
        })?;
        Ok(Some(ExpressionRoot {
            file: declaration.file,
            site: Site::new(&captured.ast, argument),
        }))
    }

    /// The PropertyAssignment-specific initializer question in
    /// isReadonlyAssignmentDeclaration deliberately skips every other kind.
    pub(in crate::project::semantic) fn readonly_writable_initializer(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        if node.syntax_kind != K::PropertyAssignment {
            return Ok(None);
        }
        let initializer = syntax::id(node, "initializer")
            .ok_or_else(|| super::super::internal("property assignment lacks initializer"))?;
        Ok(Some(ExpressionRoot {
            file: declaration.file,
            site: Site::new(&captured.ast, initializer),
        }))
    }

    /// `isSyntacticDefault` is an annotation-independent syntax observation.
    pub(in crate::project::semantic) fn declaration_is_syntactic_default(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok(
            node.syntax_kind == K::ExportAssignment && !syntax::flag(node, "isExportEquals")
                || syntax::has_modifier(&captured.ast, node, K::DefaultKeyword)
                || matches!(node.syntax_kind, K::ExportSpecifier | K::NamespaceExport),
        )
    }

    /// The actual `ValueDeclaration.Type()` question used by
    /// getPropertyOfVariable; unlike getExplicitTypeOfSymbol it has no
    /// function/class/for-of fallback.
    pub(in crate::project::semantic) fn declaration_type_annotation(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<TypeExpression>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        Ok(syntax::id(captured.ast.node(declaration.node), "type")
            .map(|ty| type_expression(inputs, declaration.file, captured, &captured.parents, ty)))
    }

    /// `reportCircularityError` tests Type first, then (only under
    /// noImplicitAny) the parameter/initializer diagnostic gate.
    pub(in crate::project::semantic) fn circularity_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<(bool, bool), DemandError> {
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        Ok((
            syntax::id(node, "type").is_some(),
            node.syntax_kind != K::Parameter || syntax::id(node, "initializer").is_some(),
        ))
    }
}

impl Source<'_, '_> {
    /// The next entity in `checkAndReportErrorForResolvingImportAliasToTypeOnlySymbol`.
    pub(in crate::project::semantic) fn import_alias_left_entity(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<ExpressionRoot>, DemandError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let id = captured.node_at(root.site)?;
        if ast.node(id).syntax_kind == K::Identifier {
            return Ok(None);
        }
        let left = syntax::id(ast.node(id), "left")
            .ok_or_else(|| super::super::internal("import alias entity has no left branch"))?;
        Ok(Some(ExpressionRoot {
            file: root.file,
            site: Site::new(ast, left),
        }))
    }

    /// Actual related-info name observation; export-star uses the native
    /// literal '*', all other type-only declarations ask Name().Text().
    pub(in crate::project::semantic) fn import_alias_type_only_diagnostic_name(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<String, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if node.syntax_kind == K::ExportDeclaration {
            return Ok("*".into());
        }
        let name = syntax::id(node, "name")
            .ok_or_else(|| super::super::internal("type-only alias declaration has no name"))?;
        Ok(super::reference_imports::exact_text(inputs, declaration.file, ast, name)?.to_owned())
    }
}
