//! Immutable inputs to native `initializeChecker`, before checker merging.
//!
//! These are binder identities and ordered initialization inputs. Identities
//! never follow lexical representatives; no completed alias target or checker
//! type cache is retained here.
//! The arena moves here before checker initialization; loading and requests
//! replay the same driver in fresh evaluator-owned semantic state.

use super::JsonDeclaration;
use super::ids::{NativeHeaderId, NativeTableId, RequestId};
use super::store::{AliasRef, native::NativeSymbols};
use crate::project::sources::FileId;
use std::collections::HashMap;

#[derive(Debug)]
#[allow(dead_code)] // Captured before the coordinated semantic reader migration.
pub(in crate::project) struct RawBindingSnapshot {
    pub(super) native: NativeSymbols,
    pub(super) scope_tables: Vec<Vec<Option<NativeTableId>>>,
    pub(super) declaration_locals: Vec<Vec<Option<NativeHeaderId>>>,
    pub(super) file_modules: Vec<Option<NativeHeaderId>>,
    pub(super) ambient_hosts: Vec<Vec<NativeHeaderId>>,
    pub(super) augmentations: Vec<Vec<Option<NativeHeaderId>>>,
    pub(super) global_augmentations: Vec<Option<NativeHeaderId>>,
    pub(super) json: RawJsonSymbols,
    pub(super) requests: Vec<Vec<RawModuleRequest>>,
    pub(super) ambient: HashMap<String, NativeHeaderId>,
    pub(super) patterns: Vec<(String, NativeHeaderId)>,
    pub(super) initialization: InitializationRecipe,
}

/// `resolveExternalModule` starts from this captured request result, then
/// observes this checker's pattern augmentation and one-step merge maps.
/// Missing/unchecked/unsupported distinctions remain in the file's existing
/// ModuleResolution facts; Script and Ignored are not interchangeable.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(super) enum RawModuleRequest {
    Module {
        header: NativeHeaderId,
        pattern: bool,
    },
    Script,
    Ignored,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(super) struct RawJsonSymbols {
    pub(super) nodes: HashMap<(FileId, JsonDeclaration), NativeHeaderId>,
    pub(super) exports: HashMap<FileId, NativeHeaderId>,
}

/// Native `initializeChecker` consumes these groups in separate phases:
/// script globals/UMD, global augmentations, intrinsic/fixed globals, deferred
/// ambient modules, then nonglobal augmentations. No alias is resolved here.
#[derive(Debug)]
#[allow(dead_code)]
pub(super) struct InitializationRecipe {
    pub(super) files: Vec<InitializationFile>,
    pub(super) modules: Vec<ModuleAugmentation>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(super) struct InitializationFile {
    pub(super) file: FileId,
    /// The initializer classifies every raw entry, queuing all ambient module
    /// symbols (including first declarations) for the later mergeGlobalSymbol
    /// pass. Navigation's later-file Fragment pairs are not that native list.
    pub(super) script_locals: Option<NativeTableId>,
    pub(super) global_exports: Vec<(String, AliasRef)>,
    pub(super) global_augmentation: Option<NativeHeaderId>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(super) struct ModuleAugmentation {
    pub(super) file: FileId,
    pub(super) request: RequestId,
    pub(super) source: NativeHeaderId,
}
