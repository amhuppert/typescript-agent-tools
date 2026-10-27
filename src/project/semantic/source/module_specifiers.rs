//! Syntax inputs for module spelling; symbol acquisition belongs to evaluation.
use super::super::model::DemandError;
use super::{DeclarationIdentity, Source};
use crate::project::binding::{RequestKind, ResolutionMode};
use crate::project::sources::FileId;
use crate::syntax::{self, id};
use crate::syntax_kind::SyntaxKind as K;

impl Source<'_, '_> {
    /// `canHaveModuleSpecifier` / `TryGetModuleSpecifierFromDeclaration`.
    /// This asks about the actual enclosing node, not an arbitrary ancestor's
    /// import. The final native gate accepts StringLiteral alone. The outer
    /// option distinguishes no original specifier from a specifier whose
    /// GetModeForUsageLocation result is actually ResolutionModeNone.
    pub(in crate::project::semantic) fn diagnostic_module_specifier_mode(
        &mut self,
        context_file: FileId,
        declaration: DeclarationIdentity,
    ) -> Result<Option<Option<ResolutionMode>>, DemandError> {
        let bindings = self.inputs.bindings;

        let semantics = bindings.file_module_semantics(context_file);
        let configured = self.inputs.semantic_options().module_kind;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        let parent = |node: crate::NodeId| captured.parents[node.index()];
        let module_specifier = |node| id(ast.node(node), "moduleSpecifier");
        let literal = match node.syntax_kind {
            K::VariableDeclaration | K::BindingElement => {
                let mut current = id(node, "initializer");
                let mut literal = None;
                while let Some(node) = current {
                    if syntax::is_require_call(ast, node, true) {
                        literal = syntax::ids(ast.node(node), "arguments").first().copied();
                        break;
                    }
                    current = parent(node);
                }
                literal
            }
            K::ImportDeclaration | K::ExportDeclaration => module_specifier(declaration.node),
            K::ImportEqualsDeclaration => id(node, "moduleReference")
                .filter(|&node| ast.node(node).syntax_kind == K::ExternalModuleReference)
                .and_then(|node| id(ast.node(node), "expression")),
            K::ImportClause | K::NamespaceExport => {
                parent(declaration.node).and_then(module_specifier)
            }
            K::NamespaceImport | K::ExportSpecifier => parent(declaration.node)
                .and_then(parent)
                .and_then(module_specifier),
            K::ImportSpecifier => parent(declaration.node)
                .and_then(parent)
                .and_then(parent)
                .and_then(module_specifier),
            K::ImportType => syntax::literal_import_type_specifier(ast, node),
            _ => None,
        };
        let Some(literal) = literal.filter(|&node| ast.node(node).syntax_kind == K::StringLiteral)
        else {
            return Ok(None);
        };
        if let Some(site) = super::reference_import_tracker::retained_module_site(
            bindings,
            captured,
            declaration.file,
            literal,
        )? {
            let facts = bindings.reference_module_site(site);
            return Ok(Some(
                semantics.resolution_mode(configured, facts.kind, facts.mode),
            ));
        }
        // A TS require call is a valid syntax context for module spelling but
        // is not a dependency gathered by collectModuleReferences. Its usage
        // mode still goes through the same file/configuration owner.
        if parent(literal).is_some_and(|node| syntax::is_require_call(ast, node, false)) {
            return Ok(Some(semantics.resolution_mode(
                configured,
                RequestKind::Require,
                None,
            )));
        }
        Err(super::super::internal("module specifier has no retained resolution site").into())
    }

    /// `IsModuleWithStringLiteralName` plus exact native name acquisition.
    pub(in crate::project::semantic) fn diagnostic_ambient_module_name(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<String>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = ast.node(declaration.node);
        if !syntax::is_module_with_string_literal_name(ast, declaration.node) {
            return Ok(None);
        }
        let name = id(node, "name").expect("string-named module has a name");
        Ok(Some(
            super::reference_imports::exact_text(inputs, declaration.file, ast, name)?.to_owned(),
        ))
    }

    /// `tryGetModuleNameFromAmbientModule`'s second-pass container question:
    /// FindAncestor starts at the declaration, and the first string-named
    /// module must itself have a SourceFile parent.
    pub(in crate::project::semantic) fn diagnostic_ambient_module_container(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        if ast.node(declaration.node).syntax_kind != K::ModuleDeclaration {
            return Ok(None);
        }
        let mut current = Some(declaration.node);
        while let Some(node) = current {
            if syntax::is_module_with_string_literal_name(ast, node) {
                return Ok(captured.parents[node.index()]
                    .filter(|&parent| ast.node(parent).syntax_kind == K::SourceFile)
                    .map(|_| DeclarationIdentity::new(declaration.file, node)));
            }
            current = captured.parents[node.index()];
        }
        Ok(None)
    }
}
