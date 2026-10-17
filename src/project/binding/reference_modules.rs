//! Source-file import inventories keep the loader's retained module request
//! identity. Reference search never resolves a second spelling of a module.

use super::ids::RequestId;
use super::inputs::BindingFiles;
use super::{RequestKind, TypeBindingFailure};
use crate::{Span, project::sources::FileId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::project) struct ReferenceModuleSite {
    pub(super) file: FileId,
    pub(super) request: RequestId,
}

impl ReferenceModuleSite {
    pub(in crate::project) fn file(self) -> FileId {
        self.file
    }
}

#[derive(Clone, Copy)]
pub(in crate::project) struct ReferenceModuleSiteFacts {
    /// Explicit type-only/import-type override, before any file-mode rule.
    pub(in crate::project) mode: Option<super::ResolutionMode>,
    pub(in crate::project) emit_mode: Option<super::ResolutionMode>,
    pub(in crate::project) resolution_mode: Option<super::ResolutionMode>,
    pub(in crate::project) span: Span,
    pub(in crate::project) kind: RequestKind,
    pub(in crate::project) implicit: bool,
}

impl<'a> BindingFiles<'a> {
    /// `getExportsOfModuleWorker`'s actual export-star declaration request.
    /// The caller selects Symbol.Declarations; this only retains syntax facts.
    pub(in crate::project) fn reference_export_star_recipe(
        &self,
        file: FileId,
        declaration_span: Span,
    ) -> Result<(ReferenceModuleSite, bool), TypeBindingFailure> {
        let mut matches = self
            .file(file)
            .stars
            .iter()
            .filter(|star| star.declaration_span == declaration_span);
        let star = matches.next().ok_or(TypeBindingFailure::NotRetained)?;
        if matches.next().is_some() {
            return Err(TypeBindingFailure::NotRetained);
        }
        Ok((
            ReferenceModuleSite {
                file,
                request: star.request,
            },
            star.type_only,
        ))
    }

    pub(in crate::project) fn file_is_declaration(&self, file: FileId) -> bool {
        self.file(file).declaration
    }

    pub(in crate::project) fn file_is_json(&self, file: FileId) -> bool {
        self.file(file).json.is_some()
    }

    /// Candidate request inventory for `SourceFile.Imports` and the fallback
    /// of `forEachImport`. Source owns their distinct ancestor/name gates;
    /// neither may acquire a module before choosing its actual candidates.
    /// Static and dynamic requests retain their respective source order,
    /// followed by implicit requests. Augmentations are a separate inventory.
    pub(in crate::project) fn reference_module_sites(
        &self,
        file: FileId,
    ) -> Vec<ReferenceModuleSite> {
        let requests = self.file(file).requests;
        let mut sites = Vec::new();
        for (index, request) in requests.iter().enumerate() {
            if !matches!(request.kind, RequestKind::Augmentation { .. }) {
                sites.push(ReferenceModuleSite {
                    file,
                    request: RequestId::new(index),
                });
            }
        }
        // collectModuleReferences' existing ordering is stable. The retained
        // request index is exactly the input order, including filtered gaps;
        // using it only on ties preserves that result without sort scratch.
        let key = |site: &ReferenceModuleSite| {
            let request = &requests[site.request.index()];
            (
                request.implicit,
                request.kind.subtask_rank(),
                request.span.start,
                site.request.index(),
            )
        };
        sites.sort_unstable_by_key(key);
        sites
    }

    pub(in crate::project) fn reference_module_site(
        &self,
        site: ReferenceModuleSite,
    ) -> ReferenceModuleSiteFacts {
        let request = &self.file(site.file).requests[site.request.index()];
        ReferenceModuleSiteFacts {
            mode: request.mode,
            emit_mode: request.emit_mode,
            resolution_mode: request.resolution_mode,
            span: request.span,
            kind: request.kind,
            implicit: request.implicit.is_some(),
        }
    }

    /// Captured `GetImpliedNodeFormatForEmit` / `GetEmitModuleFormatOfFile`.
    pub(in crate::project) fn file_module_semantics(
        &self,
        file: FileId,
    ) -> crate::project::program::FileModuleSemantics {
        self.file(file).module_semantics
    }

    pub(in crate::project) fn reference_module_specifier(
        &self,
        site: ReferenceModuleSite,
    ) -> &'a str {
        &self.file(site.file).requests[site.request.index()].specifier
    }

    /// `GetResolvedModuleFromModuleSpecifier` observes raw resolution, before
    /// program admission, ambient symbol lookup, or export= resolution.
    pub(in crate::project) fn reference_resolved_module_path(
        &self,
        site: ReferenceModuleSite,
    ) -> Option<&'a str> {
        self.file(site.file).requests[site.request.index()]
            .resolved_path
            .as_deref()
    }
}
