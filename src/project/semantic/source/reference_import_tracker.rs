//! Native ImportTracker syntax projections. Import identities come from the
//! loader's captured request inventory; this module never scans for imports.

use super::super::model::DemandError;
use super::{DeclarationIdentity, ExpressionRoot, Source};
use crate::project::binding::flow::Site;
use crate::project::binding::{ReferenceModuleSite, RequestKind};
use crate::project::sources::FileId;
use crate::syntax::{self, id, ids};
use crate::syntax_kind::SyntaxKind as K;
use crate::{Ast, NodeId};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) enum ImportNode {
    Captured(ExpressionRoot),
    /// The program owns this synthesized, clause-less ImportDeclaration.
    /// Its request identity must not be confused with an actual source span.
    Implicit(ReferenceModuleSite, FileId),
}

// File/node/request indices plus one fixed variant tag; no names or syntax traversal.

impl ImportNode {
    pub(in crate::project::semantic) fn file(self) -> FileId {
        match self {
            Self::Captured(root) => root.file,
            Self::Implicit(_, file) => file,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) enum SourceFileLike {
    File(FileId),
    Ambient(DeclarationIdentity),
}

// Exact file or ambient-declaration identity, independent of display names.

impl SourceFileLike {
    pub(in crate::project::semantic) fn file(self) -> FileId {
        match self {
            Self::File(file) => file,
            Self::Ambient(declaration) => declaration.file,
        }
    }
}

pub(in crate::project::semantic) struct ImportSpecifier {
    pub(in crate::project::semantic) name: ExpressionRoot,
    pub(in crate::project::semantic) property: Option<ExpressionRoot>,
}

pub(in crate::project::semantic) enum ImportForm {
    Call {
        dynamic: bool,
        variable: Option<ExpressionRoot>,
    },
    Identifier(ExpressionRoot),
    Equals {
        name: ExpressionRoot,
        exported: bool,
        external: bool,
    },
    Import {
        valid_module: bool,
        namespace: Option<ExpressionRoot>,
        named: Vec<ImportSpecifier>,
        default: Option<ExpressionRoot>,
    },
    Export {
        valid_module: bool,
        clause: ExportClause,
    },
    Type {
        typeof_: bool,
        qualifier: Option<ExpressionRoot>,
        argument: ExpressionRoot,
    },
}

pub(in crate::project::semantic) enum ExportClause {
    Star,
    Namespace,
    Named(Vec<ImportSpecifier>),
}

impl Source<'_, '_> {
    /// `forEachImport` over the retained requests. `collectModuleReferences`
    /// publishes a narrower static list than the binder's request inventory;
    /// when that list and implicit imports are empty, the native ambient
    /// fallback admits all top-level/ambient import/export statements instead.
    pub(in crate::project::semantic) fn reference_imports(
        &mut self,
        file: FileId,
    ) -> Result<Vec<(ReferenceModuleSite, ImportNode)>, DemandError> {
        self.reference_import_inventory(file, true)
    }

    /// Actual `SourceFile.Imports`, without synthetic helpers/JSX imports or
    /// forEachImport's ambient fallback.
    pub(in crate::project::semantic) fn reference_source_imports(
        &mut self,
        file: FileId,
    ) -> Result<Vec<(ReferenceModuleSite, ImportNode)>, DemandError> {
        self.reference_import_inventory(file, false)
    }

    fn reference_import_inventory(
        &mut self,
        file: FileId,
        for_each_import: bool,
    ) -> Result<Vec<(ReferenceModuleSite, ImportNode)>, DemandError> {
        let bindings = self.inputs.bindings;

        let sites = bindings.reference_module_sites(file);
        let external = bindings.reference_file_is_external_or_commonjs(file);
        let all_implicit = sites
            .iter()
            .all(|&site| bindings.reference_module_site(site).implicit);
        if all_implicit {
            let mut result = Vec::new();
            if for_each_import {
                for site in sites {
                    (result).push((site, ImportNode::Implicit(site, file)));
                }
            }
            return Ok(result);
        }
        let captured = self.syntax(file)?;
        let ast = &captured.ast;
        let mut native = Vec::new();
        let mut fallback = Vec::new();
        let mut implicit = Vec::new();
        for site in sites {
            let facts = bindings.reference_module_site(site);
            if facts.implicit {
                (implicit).push((site, ImportNode::Implicit(site, file)));
                continue;
            }
            let literal = captured_module_literal(captured, facts.span)?;
            let owner = import_from_module_specifier(ast, &captured.parents, literal)?;
            let node = ImportNode::Captured(root(ast, file, owner));
            if matches!(facts.kind, RequestKind::Dynamic | RequestKind::ImportType) {
                (native).push((site, node));
                continue;
            }
            let parent = captured.parents[owner.index()];
            let mut ancestors = parent;
            let mut ambient_only = true;
            while let Some(current) = ancestors {
                match ast.node(current).syntax_kind {
                    K::SourceFile => break,
                    K::ModuleBlock => {
                        let Some(module) = captured.parents[current.index()] else {
                            ambient_only = false;
                            break;
                        };
                        if !is_ambient_module_declaration(ast, module) {
                            ambient_only = false;
                            break;
                        }
                        ancestors = captured.parents[module.index()];
                    }
                    _ => {
                        ambient_only = false;
                        break;
                    }
                }
            }
            if ambient_only {
                (fallback).push((site, node));
            }
            let name = bindings.reference_module_specifier(site);
            if name.is_empty() {
                continue;
            }
            let top_level = parent.is_some_and(|p| ast.node(p).syntax_kind == K::SourceFile);
            let top_ambient = parent
                .filter(|&p| ast.node(p).syntax_kind == K::ModuleBlock)
                .and_then(|p| captured.parents[p.index()])
                .filter(|&p| syntax::is_ambient_module(ast, p))
                .filter(|&p| {
                    captured.parents[p.index()]
                        .is_some_and(|q| ast.node(q).syntax_kind == K::SourceFile)
                })
                .is_some_and(|p| {
                    !external
                        && (syntax::has_modifier(ast, ast.node(p), K::DeclareKeyword)
                            || syntax::flag(ast.root(), "isDeclarationFile"))
                });
            if top_level || top_ambient && !crate::project::is_relative_name(name) {
                (native).push((site, node));
            }
        }
        if !for_each_import || external || !native.is_empty() || !implicit.is_empty() {
            if for_each_import {
                for value in implicit {
                    (native).push(value);
                }
            }
            Ok(native)
        } else {
            Ok(fallback)
        }
    }

    /// `getImportersForExport` / `getSearchesFromDirectImports` inspect these
    /// actual syntax roles at their native branch; name text remains lazy.
    pub(in crate::project::semantic) fn reference_import_form(
        &mut self,
        import: ImportNode,
    ) -> Result<ImportForm, DemandError> {
        let ImportNode::Captured(root_) = import else {
            return Ok(ImportForm::Import {
                valid_module: true,
                namespace: None,
                named: Vec::new(),
                default: None,
            });
        };
        let captured = self.syntax(root_.file)?;
        let ast = &captured.ast;
        let node = captured.node_at(root_.site)?;
        let n = ast.node(node);
        let make = |id| root(ast, root_.file, id);
        let required = |field| {
            id(n, field)
                .ok_or_else(|| super::super::internal("import node lacks its required child"))
        };
        let specifiers = |clause| {
            (ids(ast.node(clause), "elements").iter().map(|&element| {
                let element = ast.node(element);
                Ok::<ImportSpecifier, DemandError>(ImportSpecifier {
                    name: make(
                        id(element, "name")
                            .ok_or_else(|| super::super::internal("import specifier lacks name"))?,
                    ),
                    property: id(element, "propertyName").map(make),
                })
            }))
            .collect::<Result<Vec<_>, _>>()
        };
        Ok(match n.syntax_kind {
            K::CallExpression => ImportForm::Call {
                dynamic: id(n, "expression")
                    .is_some_and(|n| ast.node(n).syntax_kind == K::ImportKeyword),
                variable: captured.parents[node.index()]
                    .filter(|&p| ast.node(p).syntax_kind == K::VariableDeclaration)
                    .and_then(|p| id(ast.node(p), "name"))
                    .filter(|&n| ast.node(n).syntax_kind == K::Identifier)
                    .map(make),
            },
            K::Identifier => ImportForm::Identifier(root_),
            K::ImportEqualsDeclaration => ImportForm::Equals {
                name: make(required("name")?),
                exported: syntax::has_modifier(ast, n, K::ExportKeyword),
                external: syntax::is_external_module_import_equals_declaration(ast, node),
            },
            K::ImportDeclaration => {
                let clause = id(n, "importClause");
                let bindings = clause.and_then(|c| id(ast.node(c), "namedBindings"));
                ImportForm::Import {
                    valid_module: id(n, "moduleSpecifier")
                        .is_some_and(|n| ast.node(n).syntax_kind == K::StringLiteral),
                    namespace: bindings
                        .filter(|&b| ast.node(b).syntax_kind == K::NamespaceImport)
                        .and_then(|b| id(ast.node(b), "name"))
                        .map(make),
                    named: bindings
                        .filter(|&b| ast.node(b).syntax_kind == K::NamedImports)
                        .map(specifiers)
                        .transpose()?
                        .unwrap_or_default(),
                    default: clause.and_then(|c| id(ast.node(c), "name")).map(make),
                }
            }
            K::ExportDeclaration => ImportForm::Export {
                valid_module: id(n, "moduleSpecifier")
                    .is_some_and(|n| ast.node(n).syntax_kind == K::StringLiteral),
                clause: match id(n, "exportClause") {
                    None => ExportClause::Star,
                    Some(clause) if ast.node(clause).syntax_kind == K::NamespaceExport => {
                        ExportClause::Namespace
                    }
                    Some(clause) => ExportClause::Named(specifiers(clause)?),
                },
            },
            K::ImportType => {
                let argument = required("argument")?;
                let literal = id(ast.node(argument), "literal").ok_or_else(|| {
                    super::super::internal("import type argument is not a literal type")
                })?;
                ImportForm::Type {
                    typeof_: syntax::flag(n, "isTypeOf"),
                    qualifier: id(n, "qualifier").map(|mut q| {
                        while ast.node(q).syntax_kind == K::QualifiedName {
                            q = id(ast.node(q), "left").expect("qualified name left");
                        }
                        make(q)
                    }),
                    argument: make(literal),
                }
            }
            _ => {
                return Err(super::super::internal(
                    "retained module request has no native import statement",
                )
                .into());
            }
        })
    }

    /// `getSourceFileLikeForImportDeclaration`; dynamic-import callers ask
    /// the distinct nearest ambient ancestor operation below.
    pub(in crate::project::semantic) fn reference_import_container(
        &mut self,
        import: ImportNode,
    ) -> Result<SourceFileLike, DemandError> {
        let ImportNode::Captured(root_) = import else {
            return Ok(SourceFileLike::File(import.file()));
        };
        let captured = self.syntax(root_.file)?;
        let node = captured.node_at(root_.site)?;
        if captured.ast.node(node).syntax_kind == K::CallExpression {
            return Ok(SourceFileLike::File(root_.file));
        }
        let parent = captured.parents[node.index()]
            .ok_or_else(|| super::super::internal("import has no parent"))?;
        if captured.ast.node(parent).syntax_kind == K::SourceFile {
            return Ok(SourceFileLike::File(root_.file));
        }
        if captured.ast.node(parent).syntax_kind == K::ModuleBlock
            && let Some(module) = captured.parents[parent.index()]
            && is_ambient_module_declaration(&captured.ast, module)
        {
            return Ok(SourceFileLike::Ambient(DeclarationIdentity::new(
                root_.file, module,
            )));
        }
        Err(super::super::internal(
            "import declaration is outside native source-file-like container",
        )
        .into())
    }

    pub(in crate::project::semantic) fn reference_import_call_container(
        &mut self,
        import: ImportNode,
    ) -> Result<SourceFileLike, DemandError> {
        let ImportNode::Captured(root_) = import else {
            return Ok(SourceFileLike::File(import.file()));
        };
        let captured = self.syntax(root_.file)?;
        let mut current = Some(captured.node_at(root_.site)?);
        while let Some(node) = current {
            if is_ambient_module_declaration(&captured.ast, node) {
                return Ok(SourceFileLike::Ambient(DeclarationIdentity::new(
                    root_.file, node,
                )));
            }
            current = captured.parents[node.index()];
        }
        Ok(SourceFileLike::File(root_.file))
    }

    /// Native `isExported` stops before an ambient module when requested.
    pub(in crate::project::semantic) fn reference_import_is_exported(
        &mut self,
        import: ImportNode,
        stop_ambient: bool,
    ) -> Result<bool, DemandError> {
        let ImportNode::Captured(root_) = import else {
            return Ok(false);
        };
        let captured = self.syntax(root_.file)?;
        let mut current = Some(captured.node_at(root_.site)?);
        while let Some(node) = current {
            if stop_ambient && is_ambient_module_declaration(&captured.ast, node) {
                break;
            }
            if syntax::has_modifier(&captured.ast, captured.ast.node(node), K::ExportKeyword) {
                return Ok(true);
            }
            current = captured.parents[node.index()];
        }
        Ok(false)
    }

    /// `forEachPossibleImportOrExportStatement` for findNamespaceReExports.
    /// This walks statement containers, not an alternate import inventory.
    pub(in crate::project::semantic) fn reference_namespace_reexports(
        &mut self,
        container: SourceFileLike,
    ) -> Result<Vec<DeclarationIdentity>, DemandError> {
        let captured = self.syntax(container.file())?;
        let ast = &captured.ast;
        let node = match container {
            SourceFileLike::File(_) => ast.root_id(),
            SourceFileLike::Ambient(d) => d.node,
        };
        let statements = |node| {
            if ast.node(node).syntax_kind == K::SourceFile {
                ids(ast.node(node), "statements")
            } else {
                id(ast.node(node), "body")
                    .map(|body| ids(ast.node(body), "statements"))
                    .unwrap_or_default()
            }
        };
        let mut pending = Vec::new();
        for node in statements(node).iter().rev().copied() {
            (pending).push(node);
        }
        let mut result = Vec::new();
        loop {
            let node = { pending.pop() };
            let Some(node) = node else {
                break;
            };
            let n = ast.node(node);
            if n.syntax_kind == K::ExportDeclaration
                && id(n, "moduleSpecifier").is_none()
                && let Some(clause) =
                    id(n, "exportClause").filter(|&c| ast.node(c).syntax_kind == K::NamedExports)
            {
                for &element in ids(ast.node(clause), "elements") {
                    (result).push(DeclarationIdentity::new(container.file(), element));
                }
            }
            if is_ambient_module_declaration(ast, node) {
                for node in statements(node).iter().rev().copied() {
                    (pending).push(node);
                }
            }
        }
        Ok(result)
    }

    /// `IsExternalModuleAugmentation`, including global scope augmentations.
    pub(in crate::project::semantic) fn reference_is_external_augmentation(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        let external = self
            .inputs
            .bindings
            .reference_file_is_external_or_commonjs(declaration.file);
        let captured = self.syntax(declaration.file)?;
        let ast = &captured.ast;
        if !syntax::is_ambient_module(ast, declaration.node) {
            return Ok(false);
        }
        Ok(match captured.parents[declaration.node.index()] {
            Some(parent) if ast.node(parent).syntax_kind == K::SourceFile => external,
            Some(parent) if ast.node(parent).syntax_kind == K::ModuleBlock => captured.parents
                [parent.index()]
            .filter(|&module| syntax::is_ambient_module(ast, module))
            .is_some_and(|module| {
                captured.parents[module.index()]
                    .is_some_and(|p| ast.node(p).syntax_kind == K::SourceFile)
                    && !external
            }),
            _ => false,
        })
    }

    pub(in crate::project::semantic) fn reference_import_name(
        &mut self,
        name: ExpressionRoot,
    ) -> Result<Vec<u8>, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(name.file)?;
        let node = captured.node_at(name.site)?;
        let text = super::reference_imports::exact_text(inputs, name.file, &captured.ast, node)?;

        Ok((text.as_bytes()).to_vec())
    }
}

fn root(ast: &Ast<'_>, file: FileId, node: NodeId) -> ExpressionRoot {
    ExpressionRoot {
        file,
        site: Site::new(ast, node),
    }
}
// ImportTracker's `isAmbientModuleDeclaration` deliberately excludes global
// augmentations, unlike the shared AST `IsAmbientModule` owner above.
fn is_ambient_module_declaration(ast: &Ast<'_>, node: NodeId) -> bool {
    syntax::is_module_with_string_literal_name(ast, node)
}
fn import_from_module_specifier(
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    literal: NodeId,
) -> Result<NodeId, DemandError> {
    let parent = parents[literal.index()]
        .ok_or_else(|| super::super::internal("module specifier has no parent"))?;
    match ast.node(parent).syntax_kind {
        K::ImportDeclaration | K::ExportDeclaration | K::CallExpression => Ok(parent),
        K::ExternalModuleReference | K::LiteralType => parents[parent.index()]
            .ok_or_else(|| super::super::internal("module specifier has no import owner").into()),
        _ => Err(super::super::internal("module specifier has no native import owner").into()),
    }
}

/// The explicit module request binding collected for this actual specifier
/// literal, if any. Its resolution, including a `resolution-mode` attribute,
/// is the loader's; callers never resolve a second spelling of the module.
pub(super) fn retained_module_site(
    bindings: crate::project::binding::inputs::BindingFiles<'_>,
    captured: &super::CapturedSyntax<'_>,
    file: FileId,
    literal: NodeId,
) -> Result<Option<ReferenceModuleSite>, DemandError> {
    for site in bindings.reference_module_sites(file) {
        let facts = bindings.reference_module_site(site);
        if !facts.implicit && captured_module_literal(captured, facts.span)? == literal {
            return Ok(Some(site));
        }
    }
    Ok(None)
}

/// One captured attachment question for a retained module request. The caller
/// decides whether native asks for the literal, its direct parent or import.
pub(super) fn captured_module_literal(
    captured: &super::CapturedSyntax<'_>,
    span: crate::Span,
) -> Result<NodeId, DemandError> {
    captured
        .attachment(super::AttachmentRole::Node, K::StringLiteral, span)?
        .or(captured.attachment(
            super::AttachmentRole::Node,
            K::NoSubstitutionTemplateLiteral,
            span,
        )?)
        .ok_or_else(|| {
            super::super::internal("retained module request lacks its literal attachment").into()
        })
}
