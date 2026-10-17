//! Native ImportTracker over one request's captured program. It owns the lazy
//! direct-module map and ordered transitive reachability; the reference search
//! owns filtering/output and the three result continuations.

use super::evaluation::SymbolParent;
use super::evaluation::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use super::model::DemandError;
use super::source::reference_import_tracker::{
    ExportClause, ImportForm, ImportNode, ImportSpecifier, SourceFileLike,
};
use super::source::{ExpressionRoot, ReferenceExportKind};
use crate::project::Project;
use crate::project::binding::{NativeSymbolIdentity, ReadOrigin};
use crate::project::sources::FileId;
use crate::references::Location;
use crate::resources::FixedMap;

type DirectImports = FixedMap<NativeSymbolIdentity, Vec<ImportNode>>;

pub(super) struct ImportSearch {
    pub(super) location: ExpressionRoot,
    pub(super) symbol: ReferenceSymbol,
}

pub(super) struct ImportsResult {
    pub(super) single_references: Vec<ExpressionRoot>,
    pub(super) import_searches: Vec<ImportSearch>,
    pub(super) indirect_users: Vec<FileId>,
}

/// `createImportTracker`: construction is demand-free. The first export
/// question acquires the complete direct map in native program/import order.
/// A failed acquisition stays failed for this request. Per-export traversal
/// has fresh node-seen sets, exactly as the native closure does.
///
/// This is find-references, not rename: renamed imports retain both the source
/// name occurrence and the local-symbol search. No caller must reconstruct
/// namespace/default/export= handling from syntax or binding representation.
pub(super) struct ImportTracker<'p> {
    project: &'p Project,
    direct: Option<Result<DirectImports, DemandError>>,
}

impl<'p> ImportTracker<'p> {
    pub(super) fn new(project: &'p Project) -> Self {
        Self {
            project,
            direct: None,
        }
    }

    pub(super) fn searches(
        &mut self,
        evaluator: &mut Evaluator<'_, '_>,
        export_symbol: ReferenceSymbol,
        module: SymbolParent,
        kind: ReferenceExportKind,
        consumer: &Location,
    ) -> Result<ImportsResult, DemandError> {
        if self.direct.is_none() {
            self.direct = Some(self.direct_imports(evaluator, consumer));
        }
        let direct = self
            .direct
            .as_ref()
            .expect("direct imports initialized")
            .as_ref()
            .map_err(Clone::clone)?;
        let global = evaluator.reference_parent_has_global_exports(module, consumer)?;
        let module = evaluator.reference_module_parent_symbol(module);
        let mut query = Importers {
            inventory: &self.project.bindings,
            direct,
            global,
            kind,
            direct_seen: FixedMap::new(),
            indirect_seen: FixedMap::new(),
            direct_imports: Vec::new(),
            indirect: Vec::new(),
        };
        query.visit_direct(evaluator, module, consumer)?;
        let indirect_users = if global {
            let mut files = Vec::new();
            for file in self.project.sources.ids() {
                if self.project.bindings.reference_file_is_searched(file) {
                    (files).push(file);
                }
            }
            files
        } else {
            for declaration in evaluator.reference_symbol_declarations(module, consumer)? {
                if let ReferenceDeclaration::Captured(declaration) = declaration
                    && evaluator
                        .source()
                        .reference_is_external_augmentation(declaration)?
                    && self
                        .project
                        .bindings
                        .reference_file_is_searched(declaration.file)
                {
                    // The sourceFiles set is the searched program subset;
                    // imported declaration scans elsewhere do not widen it.
                    query.add_indirect(
                        evaluator,
                        SourceFileLike::Ambient(declaration),
                        false,
                        consumer,
                    )?;
                }
            }
            let mut files = Vec::new();
            for container in &query.indirect {
                (files).push(container.file());
            }
            files
        };
        let (import_searches, single_references) = searches_from_direct(
            self.project,
            evaluator,
            query.direct_imports,
            export_symbol,
            kind,
            consumer,
        )?;
        Ok(ImportsResult {
            single_references,
            import_searches,
            indirect_users,
        })
    }

    /// `getDirectImportsMap`. A semantic module dependency failure cannot
    /// become native nil or a partial healthy map. Cancellation/resource
    /// failures likewise retain the outer request's whole-failure contract.
    fn direct_imports(
        &self,
        evaluator: &mut Evaluator<'_, '_>,
        consumer: &Location,
    ) -> Result<DirectImports, DemandError> {
        let mut result: DirectImports = FixedMap::new();
        for file in self.project.sources.ids() {
            if !self.project.bindings.reference_file_is_searched(file) {
                continue;
            }
            for (site, import) in evaluator.source().reference_imports(file)? {
                let symbol = evaluator.reference_module_symbol(site, consumer)?;
                let identity = evaluator.reference_symbol_identity(symbol, consumer)?;
                result.entry(identity).or_default();
                let imports = result
                    .get_mut(&identity)
                    .expect("native direct-module list established");
                (imports).push(import);
            }
        }
        Ok(result)
    }
}

struct Importers<'a> {
    inventory: &'a crate::project::binding::Bindings,
    direct: &'a DirectImports,
    global: bool,
    kind: ReferenceExportKind,
    direct_seen: FixedMap<ImportNode, ()>,
    indirect_seen: FixedMap<SourceFileLike, ()>,
    direct_imports: Vec<ImportNode>,
    indirect: Vec<SourceFileLike>,
}

impl Importers<'_> {
    /// `getImportersForExport.handleDirectImports`. A work stack preserves
    /// native depth-first star-reexport order without recursive Rust frames.
    fn visit_direct(
        &mut self,
        evaluator: &mut Evaluator<'_, '_>,
        module: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        let identity = evaluator.reference_symbol_identity(module, consumer)?;
        let mut work = Vec::new();
        for import in self
            .direct
            .get(&identity)
            .into_iter()
            .flatten()
            .rev()
            .copied()
        {
            (work).push(import);
        }
        loop {
            let import = { work.pop() };
            let Some(import) = import else {
                break;
            };
            if self.direct_seen.insert(import, ()).is_some() {
                continue;
            }
            match evaluator.source().reference_import_form(import)? {
                ImportForm::Call { dynamic: true, .. } => {
                    let container = evaluator.source().reference_import_call_container(import)?;
                    let exported = evaluator
                        .source()
                        .reference_import_is_exported(import, true)?;
                    self.add_indirect(evaluator, container, exported, consumer)?;
                }
                ImportForm::Call { variable, .. } => {
                    if !self.global
                        && self.kind == ReferenceExportKind::Equals
                        && let Some(name) = variable
                    {
                        (self.direct_imports).push(ImportNode::Captured(name));
                    }
                }
                ImportForm::Identifier(_) => {}
                ImportForm::Equals { name, exported, .. } => {
                    self.namespace_import(evaluator, import, name, exported, false, consumer)?;
                }
                ImportForm::Import {
                    namespace, default, ..
                } => {
                    (self.direct_imports).push(import);
                    if let Some(name) = namespace {
                        self.namespace_import(evaluator, import, name, false, true, consumer)?;
                    } else if !self.global && default.is_some() {
                        let container = evaluator.source().reference_import_container(import)?;
                        self.add_indirect(evaluator, container, false, consumer)?;
                    }
                }
                ImportForm::Export {
                    clause: ExportClause::Star,
                    ..
                } => {
                    let container = evaluator.source().reference_import_container(import)?;
                    if let Some(symbol) = container_symbol(evaluator, container)? {
                        let identity = evaluator.reference_symbol_identity(symbol, consumer)?;
                        for import in self
                            .direct
                            .get(&identity)
                            .into_iter()
                            .flatten()
                            .rev()
                            .copied()
                        {
                            (work).push(import);
                        }
                    }
                }
                ImportForm::Export {
                    clause: ExportClause::Namespace,
                    ..
                } => {
                    let container = evaluator.source().reference_import_container(import)?;
                    self.add_indirect(evaluator, container, true, consumer)?;
                }
                ImportForm::Export {
                    clause: ExportClause::Named(_),
                    ..
                } => (self.direct_imports).push(import),
                ImportForm::Type {
                    typeof_, qualifier, ..
                } => {
                    if !self.global
                        && typeof_
                        && qualifier.is_none()
                        && evaluator
                            .source()
                            .reference_import_is_exported(import, false)?
                    {
                        self.add_indirect(
                            evaluator,
                            SourceFileLike::File(import.file()),
                            true,
                            consumer,
                        )?;
                    }
                    (self.direct_imports).push(import);
                }
            }
        }
        Ok(())
    }

    fn namespace_import(
        &mut self,
        evaluator: &mut Evaluator<'_, '_>,
        import: ImportNode,
        name: ExpressionRoot,
        reexport: bool,
        already_direct: bool,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        if self.kind == ReferenceExportKind::Equals {
            if !already_direct {
                (self.direct_imports).push(import);
            }
        } else if !self.global {
            let container = evaluator.source().reference_import_container(import)?;
            let transitive = reexport
                || namespace_reexport(self.inventory, evaluator, container, name, consumer)?;
            self.add_indirect(evaluator, container, transitive, consumer)?;
        }
        Ok(())
    }

    /// Native addIndirectUser marks a container once, even when its first
    /// visit was non-transitive. SourceFileLike identity is not FileId: several
    /// ambient modules in one file produce repeated final file entries.
    fn add_indirect(
        &mut self,
        evaluator: &mut Evaluator<'_, '_>,
        container: SourceFileLike,
        transitive: bool,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        if self.global {
            return Ok(());
        }
        enum Work {
            Container(SourceFileLike, bool),
            Import(ImportNode),
        }
        let mut work = Vec::new();
        (work).push(Work::Container(container, transitive));
        loop {
            let next = { work.pop() };
            let Some(next) = next else {
                break;
            };
            let (container, transitive) = match next {
                Work::Container(container, transitive) => (container, transitive),
                Work::Import(import) => {
                    if matches!(
                        evaluator.source().reference_import_form(import)?,
                        ImportForm::Type { .. }
                    ) {
                        continue;
                    }
                    (evaluator.source().reference_import_container(import)?, true)
                }
            };
            if self.indirect_seen.insert(container, ()).is_some() {
                continue;
            }
            (self.indirect).push(container);
            if !transitive {
                continue;
            }
            if let Some(symbol) = container_symbol(evaluator, container)? {
                let identity = evaluator.reference_symbol_identity(symbol, consumer)?;
                for import in self
                    .direct
                    .get(&identity)
                    .into_iter()
                    .flatten()
                    .rev()
                    .copied()
                {
                    (work).push(Work::Import(import));
                }
            }
        }
        Ok(())
    }
}

fn container_symbol(
    evaluator: &mut Evaluator<'_, '_>,
    container: SourceFileLike,
) -> Result<Option<ReferenceSymbol>, DemandError> {
    match container {
        SourceFileLike::File(file) => Ok(evaluator.reference_source_module_symbol(file)),
        SourceFileLike::Ambient(declaration) => {
            // The native question is getMergedSymbol(node.Symbol), so the
            // shared canonical declaration symbol is the correct observation.
            let symbol = evaluator.source().raw_symbol_of_declaration(declaration)?;
            Ok(Some(ReferenceSymbol::from_origin(ReadOrigin::Direct(
                symbol,
            ))))
        }
    }
}

/// `findNamespaceReExports`: local export targets compare actual symbols,
/// not module names. The namespace symbol is queried before scanning exports;
/// target acquisition stops on the first matching declaration.
fn namespace_reexport(
    inventory: &crate::project::binding::Bindings,
    evaluator: &mut Evaluator<'_, '_>,
    container: SourceFileLike,
    name: ExpressionRoot,
    consumer: &Location,
) -> Result<bool, DemandError> {
    let namespace = super::reference_imports::symbol_at_root(inventory, evaluator, name)?;
    let namespace = namespace
        .map(|symbol| evaluator.reference_symbol_identity(symbol, consumer))
        .transpose()?;
    for declaration in evaluator
        .source()
        .reference_namespace_reexports(container)?
    {
        let target = evaluator.reference_export_specifier_local_target(declaration, consumer)?;
        let target = target
            .map(|symbol| evaluator.reference_symbol_identity(symbol, consumer))
            .transpose()?;
        if target == namespace {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `getSearchesFromDirectImports` with isForRename=false. Syntax/name queries
/// stay at their native branch; an irrelevant malformed name is not decoded.
fn searches_from_direct(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    imports: Vec<ImportNode>,
    export: ReferenceSymbol,
    kind: ReferenceExportKind,
    consumer: &Location,
) -> Result<(Vec<ImportSearch>, Vec<ExpressionRoot>), DemandError> {
    let mut searches = Vec::new();
    let mut singles = Vec::new();
    for import in imports {
        match evaluator.source().reference_import_form(import)? {
            ImportForm::Equals { name, external, .. } => {
                if external && kind == ReferenceExportKind::Equals {
                    add_search(project, evaluator, name, &mut searches)?;
                }
            }
            ImportForm::Identifier(name) => {
                if kind == ReferenceExportKind::Equals {
                    add_search(project, evaluator, name, &mut searches)?;
                }
            }
            ImportForm::Type {
                qualifier,
                argument,
                ..
            } => {
                if let Some(first) = qualifier {
                    let name = evaluator.source().reference_import_name(first)?;
                    let displayed =
                        evaluator.reference_symbol_display_name_bytes(export, consumer)?;
                    if name == displayed {
                        (singles).push(first);
                    }
                } else if kind == ReferenceExportKind::Equals {
                    (singles).push(argument);
                }
            }
            ImportForm::Export {
                valid_module: true,
                clause: ExportClause::Named(named),
            } => {
                named_imports(
                    project,
                    evaluator,
                    named,
                    export,
                    kind,
                    consumer,
                    &mut searches,
                    &mut singles,
                )?;
            }
            ImportForm::Import {
                valid_module: true,
                namespace,
                named,
                default,
            } => {
                if let Some(name) = namespace {
                    if kind == ReferenceExportKind::Equals {
                        add_search(project, evaluator, name, &mut searches)?;
                    }
                } else if matches!(
                    kind,
                    ReferenceExportKind::Named | ReferenceExportKind::Default
                ) {
                    named_imports(
                        project,
                        evaluator,
                        named,
                        export,
                        kind,
                        consumer,
                        &mut searches,
                        &mut singles,
                    )?;
                }
                if let Some(name) = default
                    && matches!(
                        kind,
                        ReferenceExportKind::Default | ReferenceExportKind::Equals
                    )
                {
                    add_search(project, evaluator, name, &mut searches)?;
                }
            }
            ImportForm::Export { .. } | ImportForm::Import { .. } => {}
            ImportForm::Call { .. } => {
                return Err(super::internal(
                    "native direct import search retained an unclassified call",
                )
                .into());
            }
        }
    }
    Ok((searches, singles))
}

#[expect(
    clippy::too_many_arguments,
    reason = "FindAllReferences importTracker's search state is explicit here"
)]
fn named_imports(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    named: Vec<ImportSpecifier>,
    export: ReferenceSymbol,
    kind: ReferenceExportKind,
    consumer: &Location,
    searches: &mut Vec<ImportSearch>,
    singles: &mut Vec<ExpressionRoot>,
) -> Result<(), DemandError> {
    for specifier in named {
        let name = evaluator
            .source()
            .reference_import_name(specifier.property.unwrap_or(specifier.name))?;
        let exported_name = evaluator.reference_symbol_name(export, consumer)?;
        if !((name == exported_name) || kind != ReferenceExportKind::Named && (&name == b"default"))
        {
            continue;
        }
        if let Some(property) = specifier.property {
            (singles).push(property);
        }
        add_search(project, evaluator, specifier.name, searches)?;
    }
    Ok(())
}

fn add_search(
    project: &Project,
    evaluator: &mut Evaluator<'_, '_>,
    name: ExpressionRoot,
    searches: &mut Vec<ImportSearch>,
) -> Result<(), DemandError> {
    let symbol = super::reference_imports::symbol_at_root(&project.bindings, evaluator, name)?
        .ok_or_else(|| {
            super::invalid_at_location(
                evaluator.checker_inputs(),
                &super::location(evaluator.checker_inputs(), name.file, name.site.span),
                crate::analysis::SemanticFailureReason::ErrorType,
                "native import search has no local symbol",
            )
        })?;
    {
        searches.push(ImportSearch {
            location: name,
            symbol,
        });
    }
    Ok(())
}
