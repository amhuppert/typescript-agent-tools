//! Captured emit-format facts are separate from module-resolution overrides.
//! Checker consumers never reopen package metadata or interpret configuration.
use crate::project::binding::{RequestKind, ResolutionMode};

/// Effective `GetEmitModuleKind`, retaining distinctions the loader's grouped
/// resolution policy does not need (in particular Node18 versus Node20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ModuleKind {
    CommonJs,
    Es2015,
    Es2020,
    Es2022,
    EsNext,
    Node16,
    Node18,
    Node20,
    NodeNext,
    Preserve,
}
impl ModuleKind {
    pub(crate) fn is_node(self) -> bool {
        matches!(
            self,
            Self::Node16 | Self::Node18 | Self::Node20 | Self::NodeNext
        )
    }
    pub(crate) fn is_non_node_esm(self) -> bool {
        matches!(
            self,
            Self::Es2015 | Self::Es2020 | Self::Es2022 | Self::EsNext
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::project) struct FileModuleSemantics {
    pub(in crate::project) implied_node_format: Option<ResolutionMode>,
    pub(in crate::project) emit_module_kind: ModuleKind,
    pub(in crate::project) default_resolution_mode: Option<ResolutionMode>,
    pub(super) import_syntax_affects_resolution: bool,
}
impl FileModuleSemantics {
    /// `getModeForUsageLocation`: an admitted explicit override precedes
    /// importSyntaxAffectsModuleResolution and the emit-syntax operation.
    /// Native mode-less results stay None; GetConditions separately supplies
    /// resolution-kind defaults, while specifier reuse compares None itself.
    pub(in crate::project) fn resolution_mode(
        self,
        configured: ModuleKind,
        kind: RequestKind,
        explicit: Option<ResolutionMode>,
    ) -> Option<ResolutionMode> {
        explicit.or_else(|| {
            self.import_syntax_affects_resolution
                .then(|| self.emit_syntax(configured, kind))
                .flatten()
        })
    }

    /// `getEmitSyntaxForUsageLocationWorker` / `ShouldTransformImportCall`.
    /// An import's resolution-mode attribute deliberately does not participate.
    pub(super) fn emit_syntax(
        self,
        configured: ModuleKind,
        kind: RequestKind,
    ) -> Option<ResolutionMode> {
        if kind == RequestKind::Require {
            return Some(ResolutionMode::Require);
        }
        if kind == RequestKind::Dynamic {
            let transform = !configured.is_node()
                && configured != ModuleKind::Preserve
                && self.emit_module_kind == ModuleKind::CommonJs;
            return Some(if transform {
                ResolutionMode::Require
            } else {
                ResolutionMode::Import
            });
        }
        if self.emit_module_kind == ModuleKind::CommonJs {
            Some(ResolutionMode::Require)
        } else if self.emit_module_kind.is_non_node_esm()
            || self.emit_module_kind == ModuleKind::Preserve
        {
            Some(ResolutionMode::Import)
        } else {
            None
        }
    }
}
