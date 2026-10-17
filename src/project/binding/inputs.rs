//! Captured binding inputs shared by loading and request resolution.
//!
//! This view borrows the existing owners. Loading reads collection's names;
//! retained inputs read the completed occurrence inventory. Neither path copies
//! names, invents reference attachments, resolves symbols, or owns query caches.

use super::JsonFacts;
use super::collect::{Alias, Decl, Host, RefBase, Scope};
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
#[allow(dead_code)] // Shared source facts precede remaining native-reader migration.
pub(super) struct File<'a> {
    pub(super) module: bool,
    pub(super) declaration: bool,
    pub(super) module_semantics: crate::project::program::FileModuleSemantics,
    pub(super) diagnostics: &'a crate::project::program::Diagnostics,
    pub(super) flow: Option<&'a super::flow::Graph>,
    pub(super) json: Option<&'a JsonFacts>,
    pub(super) conditional_scopes: &'a HashMap<crate::Span, ScopeId>,
    pub(super) lookup_scopes: &'a super::site_map::SiteMap<ScopeId>,
    pub(super) module_declarations: &'a [super::collect::ModuleDeclaration],
    pub(super) modules: &'a [super::ModuleResolution],
    pub(super) reference_count: usize,
    pub(super) scopes: &'a [Scope],
    pub(super) decls: &'a [Decl],
    pub(super) aliases: &'a [Alias],
    pub(super) stars: &'a [super::collect::Star],
    pub(super) requests: &'a [super::ModuleRequest],
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
        (0..self.file(file).reference_count).map(move |index| {
            self.reference(RefRef {
                file,
                id: RefId::new(index),
            })
        })
    }

    pub(super) fn file(&self, id: FileId) -> File<'a> {
        match *self {
            BindingFiles::Loading(program) => {
                let program_file = &program.files[id.index()];
                let file = &program_file.facts;
                File {
                    module: file.module,
                    declaration: file.declaration,
                    module_semantics: program_file.module_semantics,
                    diagnostics: &program_file.diagnostics,
                    flow: file.facts.flow.as_ref(),
                    json: file.json.as_deref(),
                    conditional_scopes: &file.facts.conditional_scopes,
                    lookup_scopes: &file.facts.lookup_scopes,
                    module_declarations: &file.facts.module_declarations,
                    modules: &program_file.modules,
                    reference_count: file.facts.refs.len(),
                    scopes: &file.facts.scopes,
                    decls: &file.facts.decls,
                    aliases: &file.facts.aliases,
                    stars: &file.facts.stars,
                    requests: &file.requests,
                }
            }
            BindingFiles::Retained(bindings) => {
                let file = &bindings.files[id.index()];
                File {
                    module: file.module,
                    declaration: file.declaration,
                    module_semantics: file.module_semantics,
                    diagnostics: &file.diagnostics,
                    flow: file.flow.as_ref(),
                    json: file.json.as_deref(),
                    conditional_scopes: &file.conditional_scopes,
                    lookup_scopes: &file.lookup_scopes,
                    module_declarations: &file.module_declarations,
                    modules: &file.modules,
                    reference_count: file.refs.len(),
                    scopes: &file.scopes,
                    decls: &file.decls,
                    aliases: &file.aliases,
                    stars: &file.stars,
                    requests: &file.requests,
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
