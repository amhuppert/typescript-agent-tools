//! The one binder-to-checker input capture point. This copies no resolutions
//! and never follows navigation's union-find or a checker merge link.

use super::{AmbientHost, Linker, RequestId};
use crate::project::binding::raw::{
    InitializationFile, InitializationRecipe, ModuleAugmentation, RawBindingSnapshot,
};

impl Linker<'_, '_> {
    /// Capture inputs for native `initializeChecker` after bindExportDeclaration
    /// star publication and raw module selection, before merge_globals creates
    /// globalThis or consumes the ordered ambient/augmentation inputs.
    pub(super) fn capture_raw_bindings(&mut self) -> RawBindingSnapshot {
        let native = self.state.store.native_headers();
        let header = |symbol| self.state.store.header_for_symbol(symbol);
        let module_header = |module| header(self.state.store.module(module).symbol);
        let files = self
            .file_ids()
            .map(|file| InitializationFile {
                file,
                script_locals: if self.file(file).module {
                    None
                } else {
                    self.state.scope_tables[file.index()]
                        .first()
                        .copied()
                        .flatten()
                        .map(|table| self.state.store.native_table(table))
                },
                global_exports: self.state.global_exports[file.index()].clone(),
                global_augmentation: self.state.global_augmentations[file.index()].map(header),
            })
            .collect();
        let requests = self
            .file_ids()
            .map(|file| {
                (0..self.file(file).requests.len())
                    .map(|index| {
                        let request = RequestId::new(index);
                        native
                            .raw_request(file, request)
                            .expect("raw module selection precedes binder capture")
                    })
                    .collect()
            })
            .collect();
        let ambient_hosts = self
            .state
            .ambient_hosts
            .iter()
            .map(|hosts| {
                hosts
                    .iter()
                    .map(|host| match *host {
                        AmbientHost::Module(module) => module_header(module),
                        AmbientHost::Fragment(index) => header(self.state.fragments[index].symbol),
                    })
                    .collect()
            })
            .collect();
        let modules = self
            .state
            .augmentation_order
            .iter()
            .map(|&(file, request, source)| ModuleAugmentation {
                file,
                request,
                source: header(source),
            })
            .collect();
        let scope_tables = self
            .state
            .scope_tables
            .iter()
            .map(|tables| {
                tables
                    .iter()
                    .map(|table| table.map(|table| self.state.store.native_table(table)))
                    .collect()
            })
            .collect();
        let declaration_locals = self
            .state
            .decl_local_symbols
            .iter()
            .map(|symbols| symbols.iter().map(|symbol| symbol.map(header)).collect())
            .collect();
        let file_modules = self
            .state
            .file_modules
            .iter()
            .map(|module| module.map(module_header))
            .collect();
        let augmentations = self
            .state
            .augmentations
            .iter()
            .map(|symbols| symbols.iter().map(|symbol| symbol.map(header)).collect())
            .collect();
        let global_augmentations = self
            .state
            .global_augmentations
            .iter()
            .map(|symbol| symbol.map(header))
            .collect();
        let json = self.state.store.snapshot_json_symbols();
        let ambient = self
            .state
            .ambient
            .iter()
            .map(|(name, &module)| (name.clone(), module_header(module)))
            .collect();
        let patterns = self
            .state
            .patterns
            .iter()
            .map(|(name, module)| (name.clone(), module_header(*module)))
            .collect();
        RawBindingSnapshot {
            native: self.state.store.take_binding_headers(),
            scope_tables,
            declaration_locals,
            file_modules,
            ambient_hosts,
            augmentations,
            global_augmentations,
            json,
            requests,
            ambient,
            patterns,
            initialization: InitializationRecipe { files, modules },
        }
    }
}
