//! Actual assignment-marking syntax. Canonical identities and native checker
//! caches remain evaluation questions over the existing completed bindings.

use super::{CapturedSyntax, DeclarationIdentity, ExpressionRoot, Source};
use crate::analysis::AnalysisError;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, syntax};

pub(in crate::project::semantic) struct AssignmentScope {
    pub(in crate::project::semantic) root: Site,
    pub(in crate::project::semantic) ancestors: Vec<Site>,
}

pub(in crate::project::semantic) enum AssignmentSite {
    Target(Site),
    Export(ExpressionRoot),
}

impl Source<'_, '_> {
    /// `ensureAssignmentsMarked` / `hasParentWithAssignmentsMarked` acquire
    /// the actual nearest function/source ancestor and its enclosing roots.
    pub(in crate::project::semantic) fn assignment_scope(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<AssignmentScope>, AnalysisError> {
        let captured = self.syntax(declaration.file)?;
        let id = declaration.node;
        let mut roots = Vec::new();
        let mut current = Some(id);
        while let Some(id) = current {
            if is_function_or_source(&captured.ast, id) {
                roots.push(Site::new(&captured.ast, id));
            }
            current = captured.parents[id.index()];
        }
        if roots.is_empty() {
            return Ok(None);
        }
        let root = roots.remove(0);
        Ok(Some(AssignmentScope {
            root,
            ancestors: roots,
        }))
    }

    /// `markNodeAssignmentsWorker`: native source-order syntax exclusions and
    /// assignment/export candidates. These are transient source projections,
    /// not a retained name inventory or an alternate binding pass.
    pub(in crate::project::semantic) fn assignment_sites(
        &mut self,
        file: FileId,
        root: Site,
    ) -> Result<Vec<AssignmentSite>, AnalysisError> {
        let captured = self.syntax(file)?;
        let root = captured.node_at(root)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let mut work = vec![root];
        let mut sites = Vec::new();
        while let Some(id) = work.pop() {
            let node = ast.node(id);
            match node.syntax_kind {
                SyntaxKind::Identifier => {
                    if syntax::assignment_target(ast, |id| parents[id.index()], id).is_some() {
                        sites.push(AssignmentSite::Target(Site::new(ast, id)));
                    }
                    continue;
                }
                SyntaxKind::ExportSpecifier => {
                    let declaration = parents[id.index()]
                        .and_then(|parent| parents[parent.index()])
                        .ok_or_else(|| {
                            super::super::internal("export specifier has no native declaration")
                        })?;
                    let declaration = ast.node(declaration);
                    let local = syntax::id(node, "propertyName")
                        .or_else(|| syntax::id(node, "name"))
                        .ok_or_else(|| super::super::internal("export specifier has no name"))?;
                    if !syntax::flag(node, "isTypeOnly")
                        && !syntax::flag(declaration, "isTypeOnly")
                        && syntax::id(declaration, "moduleSpecifier").is_none()
                        && ast.node(local).syntax_kind != SyntaxKind::StringLiteral
                    {
                        sites.push(AssignmentSite::Export(ExpressionRoot {
                            file,
                            site: Site::new(ast, local),
                        }));
                    }
                    continue;
                }
                SyntaxKind::InterfaceDeclaration
                | SyntaxKind::TypeAliasDeclaration
                | SyntaxKind::EnumDeclaration => continue,
                kind if syntax::is_type_node_kind(kind) => continue,
                _ => {}
            }
            let children: Vec<_> = ast.children(id).collect();
            work.extend(children.into_iter().rev());
        }
        Ok(sites)
    }

    /// `markNodeAssignmentsWorker` compares actual declaring/referencing
    /// functions. `extendAssignmentPosition` uses raw Pos/End, not trimmed
    /// token spans; trivia and enclosing statement endings remain significant.
    pub(in crate::project::semantic) fn assignment_position(
        &mut self,
        file: FileId,
        reference: Site,
        declaration: DeclarationIdentity,
    ) -> Result<Option<usize>, AnalysisError> {
        if file != declaration.file {
            return Ok(None);
        }
        let captured = self.syntax(file)?;
        let reference = captured.node_at(reference)?;
        let declaration = declaration.node;
        let CapturedSyntax { ast, parents, .. } = captured;
        if function_or_source(ast, parents, reference)
            != function_or_source(ast, parents, declaration)
        {
            return Ok(None);
        }
        let mut position = ast.node(reference).span.start;
        let mut current = Some(reference);
        while let Some(id) = current {
            let node = ast.node(id);
            if node.span.start <= ast.node(declaration).span.start {
                break;
            }
            if matches!(
                node.syntax_kind,
                SyntaxKind::VariableStatement
                    | SyntaxKind::ExpressionStatement
                    | SyntaxKind::IfStatement
                    | SyntaxKind::DoStatement
                    | SyntaxKind::WhileStatement
                    | SyntaxKind::ForStatement
                    | SyntaxKind::ForInStatement
                    | SyntaxKind::ForOfStatement
                    | SyntaxKind::WithStatement
                    | SyntaxKind::SwitchStatement
                    | SyntaxKind::TryStatement
                    | SyntaxKind::ClassDeclaration
            ) {
                position = node.span.end;
            }
            current = parents[id.index()];
        }
        Ok(Some(position))
    }

    /// Native `location.Pos()` for past-last comparison is the accepted node's
    /// untrimmed start. External query positions remain their own UTF-16 API.
    pub(in crate::project::semantic) fn flow_reference_position(
        &mut self,
        file: FileId,
        reference: Site,
    ) -> Result<usize, AnalysisError> {
        let captured = self.syntax(file)?;
        let id = captured.node_at(reference)?;
        Ok(captured.ast.node(id).span.start)
    }
}

/// TypeScript `IsFunctionOrSourceFile`: function-like signatures are included,
/// unlike the binder's narrower control-flow container classification.
fn is_function_or_source(ast: &Ast<'_>, id: NodeId) -> bool {
    let kind = ast.node(id).syntax_kind;
    kind == SyntaxKind::SourceFile || syntax::is_function_like_kind(kind)
}

fn function_or_source(ast: &Ast<'_>, parents: &[Option<NodeId>], mut id: NodeId) -> Option<NodeId> {
    loop {
        if is_function_or_source(ast, id) {
            return Some(id);
        }
        id = parents[id.index()]?;
    }
}
