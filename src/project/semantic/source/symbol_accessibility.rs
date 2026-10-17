//! Actual lexical ancestry for native symbol accessibility. Table contents,
//! alias eligibility and chain caches remain with their semantic owners.

use super::{DeclarationIdentity, Source};
use crate::project::semantic::model::DemandError;
use crate::syntax;
use crate::syntax_kind::SyntaxKind as K;

pub(in crate::project::semantic) struct AccessibilityScope {
    pub(in crate::project::semantic) parent: Option<DeclarationIdentity>,
    pub(in crate::project::semantic) locals: bool,
    pub(in crate::project::semantic) table: AccessibilityTable,
}

pub(in crate::project::semantic) enum AccessibilityTable {
    None,
    Exports,
    Members { named_class_expression: bool },
}

impl Source<'_, '_> {
    pub(in crate::project::semantic) fn accessibility_external_import_equals(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(syntax::is_external_module_import_equals_declaration(
            &captured.ast,
            declaration.node,
        ))
    }
    /// `someSymbolTableInScope` examines one actual node before its callback
    /// can stop traversal. Global SourceFile locals are already in globals;
    /// class/interface type parameters come from raw Members, never late keys.
    pub(in crate::project::semantic) fn accessibility_scope(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<AccessibilityScope, DemandError> {
        let external = self
            .inputs
            .bindings
            .reference_file_is_external_or_commonjs(declaration.file);
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        let table = match node.syntax_kind {
            K::SourceFile if external => AccessibilityTable::Exports,
            K::ModuleDeclaration => AccessibilityTable::Exports,
            K::ClassDeclaration | K::ClassExpression | K::InterfaceDeclaration => {
                AccessibilityTable::Members {
                    named_class_expression: node.syntax_kind == K::ClassExpression
                        && syntax::id(node, "name").is_some(),
                }
            }
            _ => AccessibilityTable::None,
        };
        Ok(AccessibilityScope {
            parent: captured.parents[declaration.node.index()]
                .map(|node| DeclarationIdentity::new(declaration.file, node)),
            locals: syntax::can_have_locals(node.syntax_kind)
                && (node.syntax_kind != K::SourceFile || external),
            table,
        })
    }

    /// `getClassExpressionNameTable` uses the actual name's Text, rather than
    /// the anonymous class symbol's generated Name or a displayed spelling.
    pub(in crate::project::semantic) fn accessibility_class_expression_name(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<String>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let node = captured.ast.node(declaration.node);
        if node.syntax_kind != K::ClassExpression {
            return Ok(None);
        }
        syntax::id(node, "name")
            .map(|name| {
                super::reference_imports::exact_text(inputs, declaration.file, &captured.ast, name)
            })
            .transpose()
    }

    /// Native alias candidates exclude namespace reexports from local-name
    /// lookup, but keep them while constructing an exported dotted chain.
    pub(in crate::project::semantic) fn accessibility_namespace_reexport(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(declaration.file)?;
        Ok(
            captured.ast.node(declaration.node).syntax_kind == K::NamespaceExport
                && captured.parents[declaration.node.index()].is_some_and(|parent| {
                    syntax::id(captured.ast.node(parent), "moduleSpecifier").is_some()
                }),
        )
    }
}

pub(in crate::project::semantic) enum ContainerDeclaration {
    None,
    Parent {
        parent: DeclarationIdentity,
        module_block: Option<DeclarationIdentity>,
    },
    AssignedClass {
        receiver: super::ExpressionRoot,
        module_exports: bool,
    },
}

impl Source<'_, '_> {
    /// `getContainersOfSymbol`'s declaration alternatives retain actual parent
    /// nodes. Alias resolution and the assigned receiver's cached expression
    /// check remain ordered operations in the checker.
    pub(in crate::project::semantic) fn accessibility_container_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<ContainerDeclaration, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let node = declaration.node;
        let Some(parent) = captured.parents[node.index()] else {
            return Ok(ContainerDeclaration::None);
        };
        if ast.node(node).syntax_kind == K::ClassExpression
            && ast.node(parent).syntax_kind == K::BinaryExpression
            && syntax::id(ast.node(parent), "operatorToken")
                .is_some_and(|op| ast.node(op).syntax_kind == K::EqualsToken)
            && let Some(left) = syntax::id(ast.node(parent), "left")
            && matches!(
                ast.node(left).syntax_kind,
                K::PropertyAccessExpression | K::ElementAccessExpression
            )
            && let Some(receiver) = syntax::id(ast.node(left), "expression")
            && syntax::is_entity_name_expression(ast, receiver)
        {
            return Ok(ContainerDeclaration::AssignedClass {
                receiver: super::ExpressionRoot {
                    file: declaration.file,
                    site: crate::project::binding::flow::Site::new(ast, receiver),
                },
                module_exports: syntax::is_module_exports_access_expression(ast, left)
                    || ast.identifier_name(receiver) == Some("exports"),
            });
        }
        if syntax::is_ambient_module(ast, node) {
            return Ok(ContainerDeclaration::None);
        }
        Ok(ContainerDeclaration::Parent {
            parent: DeclarationIdentity::new(declaration.file, parent),
            module_block: (ast.node(parent).syntax_kind == K::ModuleBlock)
                .then(|| captured.parents[parent.index()])
                .flatten()
                .map(|node| DeclarationIdentity::new(declaration.file, node)),
        })
    }

    /// `getVariableDeclarationOfObjectLiteral` matches the declaration's exact
    /// initializer/type node; an ancestor variable is not sufficient.
    pub(in crate::project::semantic) fn accessibility_object_variable(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let Some(parent) = captured.parents[declaration.node.index()] else {
            return Ok(None);
        };
        if ast.node(parent).syntax_kind != K::VariableDeclaration {
            return Ok(None);
        }
        let field = match ast.node(declaration.node).syntax_kind {
            K::ObjectLiteralExpression => "initializer",
            K::TypeLiteral => "type",
            _ => return Ok(None),
        };
        Ok(
            (syntax::id(ast.node(parent), field) == Some(declaration.node))
                .then_some(DeclarationIdentity::new(declaration.file, parent)),
        )
    }
}
