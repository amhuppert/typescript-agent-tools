//! Captured binding inputs shared by loading and request resolution.
//!
//! This view borrows the existing owners. Loading reads collection's names;
//! retained inputs read the completed occurrence inventory. Neither path copies
//! names, invents reference attachments, resolves symbols, or owns query caches.

use super::JsonFacts;
use super::collect::{Alias, Decl, Host, RefBase, Scope, SharedFacts};
use super::ids::{ModuleId, RefId, ScopeId, SymbolId, TableId};
use super::link::{AmbientHost, Fragment, ValueTarget};
use super::resolution::RefRef;
use super::retained::Bindings;
use super::store::Store;
use crate::project::program::Program;
use crate::project::sources::FileId;
use std::collections::HashMap;

/// Facts are borrowed from collection during load and from the frozen binding
/// product during a query. Neither adapter repeats binding or source lookup.
#[derive(Clone, Copy)]
pub(in crate::project) enum BindingFiles<'a> {
    Loading(&'a Program),
    Retained(&'a Bindings),
}

/// Navigation host associations are separate from immutable source facts.
#[derive(Clone, Copy)]
pub(super) enum ProjectedHosts<'a> {
    Loading {
        ambient_hosts: &'a [Vec<AmbientHost>],
        fragments: &'a [Fragment],
    },
}

#[derive(Clone, Copy)]
pub(in crate::project) struct BindingInputs<'a> {
    pub(super) files: BindingFiles<'a>,
    pub(super) hosts: ProjectedHosts<'a>,
    pub(super) store: &'a Store,
    pub(super) scope_tables: &'a [Vec<Option<TableId>>],
    pub(super) decl_symbols: &'a [Vec<Option<SymbolId>>],
    pub(super) file_modules: &'a [Option<ModuleId>],
    pub(super) augmentations: &'a [Vec<Option<SymbolId>>],
    pub(super) global_augmentations: &'a [Option<SymbolId>],
    pub(super) native_inputs: Option<super::NativeInputs<'a>>,
    pub(super) native_facts: Option<&'a dyn super::NativeFacts>,
    pub(super) values: &'a HashMap<SymbolId, ValueTarget>,
}
/// One file's binding facts, borrowed from collection while loading and from
/// the published product afterwards. Lookups ask for it at every scope step,
/// so it is two pointers, and each accessor reads one field where it lives.
#[derive(Clone, Copy)]
pub(super) struct File<'a> {
    shared: &'a SharedFacts,
    owner: Owner<'a>,
}

#[derive(Clone, Copy)]
enum Owner<'a> {
    Loading(&'a crate::project::program::ProgramFile),
    Retained(&'a super::retained::File),
}

#[allow(dead_code)] // Shared source facts precede remaining native-reader migration.
impl<'a> File<'a> {
    pub(super) fn module(self) -> bool {
        match self.owner {
            Owner::Loading(file) => file.facts.module,
            Owner::Retained(file) => file.module,
        }
    }
    pub(super) fn declaration(self) -> bool {
        match self.owner {
            Owner::Loading(file) => file.facts.declaration,
            Owner::Retained(file) => file.declaration,
        }
    }
    pub(super) fn module_semantics(self) -> crate::project::program::FileModuleSemantics {
        match self.owner {
            Owner::Loading(file) => file.module_semantics,
            Owner::Retained(file) => file.module_semantics,
        }
    }
    pub(super) fn diagnostics(self) -> &'a crate::project::program::Diagnostics {
        match self.owner {
            Owner::Loading(file) => &file.diagnostics,
            Owner::Retained(file) => &file.diagnostics,
        }
    }
    pub(super) fn json(self) -> Option<&'a JsonFacts> {
        match self.owner {
            Owner::Loading(file) => file.facts.json.as_deref(),
            Owner::Retained(file) => file.json.as_deref(),
        }
    }
    pub(super) fn modules(self) -> &'a [super::ModuleResolution] {
        match self.owner {
            Owner::Loading(file) => &file.modules,
            Owner::Retained(file) => &file.modules,
        }
    }
    pub(super) fn requests(self) -> &'a [super::ModuleRequest] {
        match self.owner {
            Owner::Loading(file) => &file.facts.requests,
            Owner::Retained(file) => &file.requests,
        }
    }
    pub(super) fn reference_count(self) -> usize {
        match self.owner {
            Owner::Loading(file) => file.facts.facts.refs.len(),
            Owner::Retained(file) => file.refs.len(),
        }
    }
    pub(super) fn flow(self) -> Option<&'a super::flow::Graph> {
        self.shared.flow.as_ref()
    }
    pub(super) fn conditional_scopes(self) -> &'a HashMap<crate::Span, ScopeId> {
        &self.shared.conditional_scopes
    }
    pub(super) fn lookup_scopes(self) -> &'a super::site_map::SiteMap<ScopeId> {
        &self.shared.lookup_scopes
    }
    pub(super) fn module_declarations(self) -> &'a [super::collect::ModuleDeclaration] {
        &self.shared.module_declarations
    }
    pub(super) fn scopes(self) -> &'a [Scope] {
        &self.shared.scopes
    }
    pub(super) fn decls(self) -> &'a [Decl] {
        &self.shared.decls
    }
    pub(super) fn aliases(self) -> &'a [Alias] {
        &self.shared.aliases
    }
    pub(super) fn stars(self) -> &'a [super::collect::Star] {
        &self.shared.stars
    }
}
#[derive(Clone, Copy)]
pub(super) struct Reference<'a> {
    pub(super) name: &'a str,
    pub(super) span: crate::Span,
    pub(super) base: RefBase,
    pub(super) meaning: u8,
    pub(super) property: bool,
    pub(super) type_context: bool,
    pub(super) flow: Option<super::flow::Read>,
}
impl<'a> BindingInputs<'a> {
    pub(super) fn file(&self, id: FileId) -> File<'a> {
        self.files.file(id)
    }
    pub(super) fn reference(&self, id: RefRef) -> Reference<'a> {
        self.files.reference(id)
    }

    pub(super) fn host_symbol(&self, file: FileId, host: Host) -> Option<SymbolId> {
        match host {
            Host::File => {
                self.file_modules[file.index()].map(|module| self.store.module(module).symbol)
            }
            Host::AmbientModule(index) => Some(match self.hosts {
                ProjectedHosts::Loading {
                    ambient_hosts,
                    fragments,
                    ..
                } => match ambient_hosts[file.index()][index.index()] {
                    AmbientHost::Module(module) => self.store.module(module).symbol,
                    AmbientHost::Fragment(index) => fragments[index].symbol,
                },
            }),
            Host::Augmentation(request) => Some(
                self.augmentations[file.index()][request.index()]
                    .expect("an augmentation request has a symbol"),
            ),
            Host::Namespace(decl) => self.decl_symbols[file.index()][decl.index()],
            Host::Global => self.global_augmentations[file.index()],
        }
    }
}

impl<'a> BindingFiles<'a> {
    pub(super) fn references(self, file: FileId) -> impl ExactSizeIterator<Item = Reference<'a>> {
        (0..self.file(file).reference_count()).map(move |index| {
            self.reference(RefRef {
                file,
                id: RefId::new(index),
            })
        })
    }

    pub(super) fn file(&self, id: FileId) -> File<'a> {
        match *self {
            BindingFiles::Loading(program) => {
                let file = &program.files[id.index()];
                File {
                    shared: &file.facts.facts.shared,
                    owner: Owner::Loading(file),
                }
            }
            BindingFiles::Retained(bindings) => {
                let file = &bindings.files[id.index()];
                File {
                    shared: file,
                    owner: Owner::Retained(file),
                }
            }
        }
    }
    pub(super) fn reference(&self, id: RefRef) -> Reference<'a> {
        match *self {
            BindingFiles::Loading(program) => {
                let facts = &program.files[id.file.index()].facts.facts;
                let data = &facts.refs[id.id.index()];
                Reference {
                    name: &facts.names[data.token.index()].text,
                    span: facts.names[data.token.index()].span,
                    base: data.base,
                    meaning: data.meaning,
                    property: data.property,
                    type_context: data.type_context,
                    flow: facts.names[data.token.index()].flow,
                }
            }
            BindingFiles::Retained(bindings) => {
                let data = &bindings.files[id.file.index()].refs[id.id.index()];
                let occurrence = bindings.index.occurrence(super::index::OccurrenceRef {
                    file: id.file,
                    id: data.source,
                });
                Reference {
                    name: bindings.index.text(occurrence.name),
                    span: occurrence.span,
                    base: data.base,
                    meaning: data.meaning,
                    property: data.property,
                    type_context: data.type_context,
                    flow: data.flow,
                }
            }
        }
    }
}
