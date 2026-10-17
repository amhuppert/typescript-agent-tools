//! Symbol-written display names consume canonical own associations, separately
//! from the filtered declarations that supply a type or constraint recipe.

use super::{DeclarationIdentity, Source};
use crate::analysis::AnalysisError;
use crate::project::binding::{BoundDeclaration, BoundSymbol};
use crate::project::semantic::model::DemandError;
use crate::{syntax, syntax_kind::SyntaxKind};

impl Source<'_, '_> {
    /// The declaration-parent branches of `shouldWriteTypeOfFunctionSymbol`.
    /// A top-level expression/arrow also names the variable that replaces its
    /// anonymous function symbol during native serialization.
    pub(in crate::project::semantic) fn function_display_location(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<(bool, Option<DeclarationIdentity>), AnalysisError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let Some(parent) = captured.parents[declaration.node.index()] else {
            return Ok((false, None));
        };
        let parent_kind = ast.node(parent).syntax_kind;
        if matches!(
            parent_kind,
            SyntaxKind::SourceFile | SyntaxKind::ModuleBlock
        ) {
            return Ok((true, None));
        }
        if !matches!(
            ast.node(declaration.node).syntax_kind,
            SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction
        ) || parent_kind != SyntaxKind::VariableDeclaration
        {
            return Ok((false, None));
        }
        let Some(list) = captured.parents[parent.index()] else {
            return Ok((false, None));
        };
        let Some(statement) = captured.parents[list.index()] else {
            return Ok((false, None));
        };
        let Some(container) = captured.parents[statement.index()] else {
            return Ok((false, None));
        };
        let nonlocal = ast.node(list).syntax_kind == SyntaxKind::VariableDeclarationList
            && ast.node(statement).syntax_kind == SyntaxKind::VariableStatement
            && matches!(
                ast.node(container).syntax_kind,
                SyntaxKind::SourceFile | SyntaxKind::ModuleBlock
            );
        Ok((
            nonlocal,
            nonlocal.then_some(DeclarationIdentity::new(declaration.file, parent)),
        ))
    }

    pub(in crate::project::semantic) fn declaration_parent(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<DeclarationIdentity>, AnalysisError> {
        let captured = self.syntax(declaration.file)?;
        Ok(captured.parents[declaration.node.index()]
            .map(|node| DeclarationIdentity::new(declaration.file, node)))
    }
    pub(in crate::project::semantic) fn raw_node_symbol_origin(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<crate::project::binding::ReadOrigin>, DemandError> {
        self.node_symbol_origin_worker(declaration, true)
    }

    pub(in crate::project::semantic) fn declaration_local_symbol(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<crate::project::binding::ReadOrigin>, DemandError> {
        let bindings = self.bindings();
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        if syntax::id(node, "name").is_none()
            && syntax::has_modifier(&captured.ast, node, SyntaxKind::DefaultKeyword)
        {
            return Ok(None);
        }
        bindings
            .declaration_local_symbol(declaration.file, node.syntax_kind, node.span)
            .map_err(|error| {
                super::super::binding_failure_at(
                    inputs,
                    &super::super::location(
                        inputs,
                        declaration.file,
                        captured.ast.token_span(declaration.node),
                    ),
                    error,
                )
            })
    }

    // Both native reads share symbol attachment eligibility; only the raw
    // node.Symbol read omits the checker's getMergedSymbol operation.
    fn node_symbol_origin_worker(
        &mut self,
        declaration: DeclarationIdentity,
        raw: bool,
    ) -> Result<Option<crate::project::binding::ReadOrigin>, DemandError> {
        let bindings = self.bindings();
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if !syntax::can_have_symbol(node.syntax_kind) {
            return Ok(None);
        }
        if node.syntax_kind == SyntaxKind::SourceFile {
            if raw {
                return bindings
                    .raw_file_module_read_origin(declaration.file)
                    .map_err(|error| {
                        super::super::binding_failure_at(
                            inputs,
                            &super::super::location(
                                inputs,
                                declaration.file,
                                captured.ast.token_span(declaration.node),
                            ),
                            error,
                        )
                    });
            }
            return Ok(bindings
                .raw_file_module(declaration.file)
                .map(crate::project::binding::ReadOrigin::Direct));
        }
        if matches!(
            node.syntax_kind,
            SyntaxKind::VariableDeclaration | SyntaxKind::BindingElement
        ) && syntax::id(node, "name").is_some_and(|name| {
            matches!(
                ast.node(name).syntax_kind,
                SyntaxKind::ObjectBindingPattern | SyntaxKind::ArrayBindingPattern
            )
        }) {
            return Ok(None);
        }
        // These DeclarationData-bearing nodes have no bind switch arm. Their
        // children bind independently; neither a spread nor tuple label gets
        // the child's symbol. A class static block is only a flow container.
        if matches!(
            node.syntax_kind,
            SyntaxKind::NamedTupleMember
                | SyntaxKind::SpreadAssignment
                | SyntaxKind::ClassStaticBlockDeclaration
                | SyntaxKind::JsxSpreadAttribute
        ) {
            return Ok(None);
        }
        if raw {
            bindings
                .raw_declaration_read_origin(declaration.file, node.syntax_kind, node.span)
                .map(Some)
                .map_err(|error| {
                    super::super::binding_failure_at(
                        inputs,
                        &super::super::location(
                            inputs,
                            declaration.file,
                            captured.ast.token_span(declaration.node),
                        ),
                        error,
                    )
                })
        } else {
            self.reference_declaration_origin(declaration).map(Some)
        }
    }
    /// Raw `node.Symbol()` by the actual declaration kind and native node span;
    /// name-token anchors can belong to several distinct declarations. No late
    /// binding or checker merge: `getSymbolOfDeclaration` is the evaluator's
    /// `reference_symbol_of_declaration`.
    pub(in crate::project::semantic) fn raw_symbol_of_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<BoundSymbol, DemandError> {
        let bindings = self.bindings();
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let anchor = captured.declaration_anchor(declaration.node);
        let node = captured.ast.node(declaration.node);
        let location = super::super::location(inputs, declaration.file, anchor);
        bindings
            .native_declaration_symbol(declaration.file, node.syntax_kind, node.span)
            .map_err(|error| super::super::binding_failure_at(inputs, &location, error))
    }

    /// `isNonGenericTopLevelType`: an alias without arguments is nongeneric
    /// only when its actual parent chain reaches SourceFile through ModuleDeclarations.
    pub(in crate::project::semantic) fn alias_declaration_is_top_level(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let mut parent = captured.parents[declaration.node.index()];
        while let Some(id) = parent {
            match captured.ast.node(id).syntax_kind {
                SyntaxKind::SourceFile => return Ok(true),
                SyntaxKind::ModuleDeclaration => parent = captured.parents[id.index()],
                _ => return Ok(false),
            }
        }
        Ok(false)
    }

    /// Graph-label eligibility for the declared function family. The shared
    /// function-display source operation owns the exact parent classification;
    /// native typeof emission remains a contextual evaluator operation.
    pub(in crate::project::semantic) fn function_symbol_is_nonlocal(
        &mut self,
        declarations: &[BoundDeclaration],
    ) -> Result<bool, AnalysisError> {
        let mut nonlocal = false;
        for declaration in declarations {
            let kind = self.inputs.bindings.declaration_syntax_kind(*declaration);
            let captured = self.syntax(declaration.file)?;
            let id = captured
                .native_declaration_at(kind, declaration.anchor)?
                .ok_or_else(|| {
                    super::super::internal(
                        "canonical function has no actual declaration attachment",
                    )
                })?;
            if self
                .function_display_location(DeclarationIdentity::new(declaration.file, id))?
                .0
            {
                nonlocal = true;
                break;
            }
        }
        Ok(nonlocal)
    }
}
