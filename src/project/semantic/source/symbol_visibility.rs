//! Native declaration visibility recipes. Only the evaluator owns the
//! declaration-link cache and ordered recursive visibility decisions.
use super::{DeclarationIdentity, Source};
use crate::project::semantic::model::DemandError;
use crate::{Ast, NodeId, syntax, syntax_kind::SyntaxKind as K};

pub(in crate::project::semantic) enum VisibilityStep {
    Known(bool),
    Parent(Option<DeclarationIdentity>),
}

pub(in crate::project::semantic) enum VisibilityFallback {
    Parent(DeclarationIdentity),
    Visible,
}

impl Source<'_, '_> {
    /// The complete admitted TypeScript grammar of
    /// EmitResolver.determineIfDeclarationIsVisible. Recursive visibility and
    /// its declaration slot are owned by the evaluator, so a stopped Source
    /// operation cannot be cached as an invisible declaration.
    pub(in crate::project::semantic) fn declaration_visibility_step(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<VisibilityStep, DemandError> {
        let kind = self.declaration_kind(declaration)?;
        if kind == K::VariableDeclaration {
            let captured = self.syntax(declaration.file)?;
            if syntax::id(captured.ast.node(declaration.node), "name").is_some_and(|name| {
                matches!(
                    captured.ast.node(name).syntax_kind,
                    K::ArrayBindingPattern | K::ObjectBindingPattern
                ) && syntax::ids(captured.ast.node(name), "elements").is_empty()
            }) {
                return Ok(VisibilityStep::Known(false));
            }
        }
        if matches!(
            kind,
            K::VariableDeclaration
                | K::ModuleDeclaration
                | K::ClassDeclaration
                | K::InterfaceDeclaration
                | K::TypeAliasDeclaration
                | K::FunctionDeclaration
                | K::EnumDeclaration
                | K::ImportEqualsDeclaration
        ) && self.reference_is_external_augmentation(declaration)?
        {
            return Ok(VisibilityStep::Known(true));
        }
        let external = self
            .inputs
            .bindings
            .reference_file_is_external_or_commonjs(declaration.file);
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let node = declaration.node;
        let parent = parents[node.index()];
        let result = match kind {
            K::JSDocCallbackTag | K::JSDocTypedefTag => VisibilityStep::Known(
                parent
                    .and_then(|id| parents[id.index()])
                    .and_then(|id| parents[id.index()])
                    .is_some_and(|id| ast.node(id).syntax_kind == K::SourceFile),
            ),
            K::BindingElement => VisibilityStep::Parent(
                parent
                    .and_then(|id| parents[id.index()])
                    .map(|id| DeclarationIdentity::new(declaration.file, id)),
            ),
            K::VariableDeclaration
            | K::ModuleDeclaration
            | K::ClassDeclaration
            | K::InterfaceDeclaration
            | K::TypeAliasDeclaration
            | K::FunctionDeclaration
            | K::EnumDeclaration
            | K::ImportEqualsDeclaration => {
                let Some(container) = declaration_container(ast, parents, node) else {
                    return Err(super::super::internal(
                        "visible declaration lacks its native container",
                    )
                    .into());
                };
                if syntax::combined_modifier_flags(ast, parents, node) & syntax::modifiers::EXPORT
                    == 0
                    && !(kind != K::ImportEqualsDeclaration
                        && ast.node(container).syntax_kind != K::SourceFile
                        && ast.node(container).flags & crate::compiler::node::AMBIENT != 0)
                {
                    VisibilityStep::Known(
                        ast.node(container).syntax_kind == K::SourceFile && !external,
                    )
                } else {
                    VisibilityStep::Parent(Some(DeclarationIdentity::new(
                        declaration.file,
                        container,
                    )))
                }
            }
            K::PropertyDeclaration
            | K::PropertySignature
            | K::GetAccessor
            | K::SetAccessor
            | K::MethodDeclaration
            | K::MethodSignature => {
                // getEffectiveDeclarationFlags only adds Ambient/Export to the
                // combined flags. Neither changes this exact Private|Protected
                // question; JS/JSDoc declaration binding is outside admission.
                if syntax::combined_modifier_flags(ast, parents, node)
                    & (syntax::modifiers::PRIVATE | syntax::modifiers::PROTECTED)
                    != 0
                {
                    VisibilityStep::Known(false)
                } else {
                    VisibilityStep::Parent(
                        parent.map(|id| DeclarationIdentity::new(declaration.file, id)),
                    )
                }
            }
            K::Constructor
            | K::ConstructSignature
            | K::CallSignature
            | K::IndexSignature
            | K::Parameter
            | K::ModuleBlock
            | K::FunctionType
            | K::ConstructorType
            | K::TypeLiteral
            | K::TypeReference
            | K::ArrayType
            | K::TupleType
            | K::UnionType
            | K::IntersectionType
            | K::ParenthesizedType
            | K::NamedTupleMember => VisibilityStep::Parent(
                parent.map(|id| DeclarationIdentity::new(declaration.file, id)),
            ),
            K::TypeParameter | K::SourceFile | K::NamespaceExportDeclaration => {
                VisibilityStep::Known(true)
            }
            K::ExportSpecifier => match parent.and_then(|id| parents[id.index()]) {
                Some(export)
                    if ast.node(export).syntax_kind == K::ExportDeclaration
                        && syntax::id(ast.node(export), "moduleSpecifier").is_none() =>
                {
                    VisibilityStep::Parent(
                        parents[export.index()]
                            .map(|id| DeclarationIdentity::new(declaration.file, id)),
                    )
                }
                _ => VisibilityStep::Known(false),
            },
            _ => VisibilityStep::Known(false),
        };
        Ok(result)
    }

    /// `hasVisibleDeclarations`'s unexported-alias alternatives. Parent
    /// visibility remains lazy and ordered in the evaluator; constructing this
    /// syntax recipe cannot mark aliases visible. The invoking question has
    /// shouldComputeAliasToMakeVisible=false.
    pub(in crate::project::semantic) fn declaration_visibility_fallbacks(
        &mut self,
        declaration: DeclarationIdentity,
        block_scoped_variable: bool,
    ) -> Result<Vec<VisibilityFallback>, DemandError> {
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        let parents = &captured.parents;
        let node = declaration.node;
        let kind = ast.node(node).syntax_kind;
        let mut candidates = Vec::new();
        let exported =
            |id| syntax::modifier_flags(ast, ast.node(id)) & syntax::modifiers::EXPORT != 0;
        let parent_of = |statement: NodeId| {
            parents[statement.index()].map(|parent| {
                VisibilityFallback::Parent(DeclarationIdentity::new(declaration.file, parent))
            })
        };
        if let Some(import) = any_import_syntax(ast, parents, node)
            && !exported(import)
        {
            candidates.extend(parent_of(import));
        }
        if kind == K::VariableDeclaration
            && let Some(statement) = parents[node.index()].and_then(|id| parents[id.index()])
            && ast.node(statement).syntax_kind == K::VariableStatement
            && !exported(statement)
        {
            candidates.extend(parent_of(statement));
        }
        if late_visibility_painted_statement(kind) && !exported(node) {
            candidates.extend(parent_of(node));
        }
        // Native's Alias + IsInJSFile BindingElement alternative is excluded
        // by the existing TypeScript/TSX input envelope, not by symbol spelling.
        if kind == K::BindingElement
            && block_scoped_variable
            && let Some(root) = syntax::walk_up_binding_elements_and_patterns(ast, parents, node)
            && ast.node(root).syntax_kind != K::Parameter
            && let Some(statement) = parents[root.index()].and_then(|id| parents[id.index()])
            && ast.node(statement).syntax_kind == K::VariableStatement
        {
            if exported(statement) {
                candidates.push(VisibilityFallback::Visible);
            } else {
                candidates.extend(parent_of(statement));
            }
        }
        Ok(candidates)
    }
}

/// `GetDeclarationContainer` starts at the complete binding root and skips
/// variable/import wrappers before asking the containing node's parent.
fn declaration_container(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> Option<NodeId> {
    let mut current = syntax::root_declaration(ast, parents, node);
    while matches!(
        ast.node(current).syntax_kind,
        K::VariableDeclaration
            | K::VariableDeclarationList
            | K::ImportSpecifier
            | K::NamedImports
            | K::NamespaceImport
            | K::ImportClause
    ) {
        current = parents[current.index()]?;
    }
    parents[current.index()]
}

/// Native `getAnyImportSyntax`, distinct from obtaining a module specifier.
fn any_import_syntax(ast: &Ast<'_>, parents: &[Option<NodeId>], node: NodeId) -> Option<NodeId> {
    let steps = match ast.node(node).syntax_kind {
        K::ImportEqualsDeclaration => 0,
        K::ImportClause => 1,
        K::NamespaceImport => 2,
        K::ImportSpecifier => 3,
        _ => return None,
    };
    let mut current = node;
    for _ in 0..steps {
        current = parents[current.index()]?;
    }
    Some(current)
}

/// `IsLateVisibilityPaintedStatement` for the admitted TS grammar.
fn late_visibility_painted_statement(kind: K) -> bool {
    matches!(
        kind,
        K::ImportDeclaration
            | K::ImportEqualsDeclaration
            | K::VariableStatement
            | K::ClassDeclaration
            | K::FunctionDeclaration
            | K::ModuleDeclaration
            | K::TypeAliasDeclaration
            | K::InterfaceDeclaration
            | K::EnumDeclaration
    )
}
