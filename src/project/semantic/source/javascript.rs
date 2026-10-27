//! The syntax the checker's JavaScript-only decisions read
//! (`crate::project::semantic::evaluation::javascript`).

use super::{DeclarationIdentity, ExpressionRoot, Source};
use crate::analysis::AnalysisError;
use crate::project::sources::FileId;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;

impl Source<'_, '_> {
    /// `isUncheckedJSSuggestion`'s `suggestionHasNoExtendsOrDecorators` for
    /// a suggestion's value declaration: none, one that is not class-like,
    /// or a class with an `extends` clause or decorators of its own (with
    /// standard decorators a constructor's parameters are never decorated).
    pub(in crate::project::semantic) fn suggestion_has_no_extends_or_decorators(
        &mut self,
        value_declaration: Option<DeclarationIdentity>,
    ) -> Result<bool, AnalysisError> {
        let Some(declaration) = value_declaration else {
            return Ok(true);
        };
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if !matches!(
            node.syntax_kind,
            SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
        ) {
            return Ok(true);
        }
        let extends = !syntax::heritage_elements(ast, declaration.node, SyntaxKind::ExtendsKeyword)
            .is_empty();
        let decorated = syntax::ids(node, "modifiers")
            .iter()
            .any(|&modifier| ast.node(modifier).syntax_kind == SyntaxKind::Decorator);
        Ok(extends || decorated)
    }

    /// Whether an access is a property access of `this`, `this.name`.
    pub(in crate::project::semantic) fn is_this_property_access(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(root.file)?;
        let ast = &captured.ast;
        let node = ast.node(captured.node_at(root.site)?);
        Ok(node.syntax_kind == SyntaxKind::PropertyAccessExpression
            && syntax::id(node, "expression").is_some_and(|expression| {
                ast.node(expression).syntax_kind == SyntaxKind::ThisKeyword
            }))
    }
}

impl Source<'_, '_> {
    /// Whether a file has ES module syntax, TypeScript's
    /// `isFileProbablyExternalModule` (`crate::ast::external_module`), the
    /// external module indicator that is a statement rather than the file.
    pub(in crate::project::semantic) fn has_module_syntax(
        &mut self,
        file: FileId,
    ) -> Result<bool, AnalysisError> {
        let captured = self.syntax(file)?;
        let ast = &captured.ast;
        let root = ast.root();
        Ok(crate::ast::external_module(
            ast.nodes(),
            syntax::ids(root, "statements"),
            root.flags,
        ))
    }
}
