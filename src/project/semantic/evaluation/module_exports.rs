//! Request-owned module links over actual checker symbols. Traversal policy
//! is shared with loading; resolved late-container exports use the same worker
//! without publishing this module cache or its type-only export-star map.
use super::super::model::{DemandError, NativeResult, NativeSequence};
use super::reference_symbols::ReferenceSymbolTable;
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::project::binding::{NativePropertyKey, NativeSymbolIdentity, module_exports as worker};
use crate::references::Location;
use std::collections::HashMap;

#[derive(Clone)]
struct Links {
    exports: ReferenceSymbolTable,
    type_only: HashMap<NativePropertyKey, ReferenceDeclaration>,
}

#[derive(Default)]
pub(super) struct ModuleExports {
    links: HashMap<NativeSymbolIdentity, NativeResult<Links>>,
}

impl Evaluator<'_, '_> {
    /// `getExportsOfModule`: only the completed worker publishes these links.
    /// An allocated empty table is installed too; nil is never its final value.
    pub(super) fn reference_module_exports_worker_completion(
        &mut self,
        symbol: ReferenceSymbol,
        late_worker: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbolTable>> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        if !late_worker {
            if let Some(links) = self.module_exports.links.get(&identity) {
                return links
                    .clone()
                    .map(|links| links.map(|links| Some(links.exports)));
            }
        }
        let mut operations = Symbols {
            checker: self,
            consumer,
            sequence: NativeSequence::new(),
        };
        let result = (|| {
            let original = worker::Symbols::module(&mut operations, symbol)?;
            let exports = worker::exports(&mut operations, original)?;
            Ok(Links {
                exports: exports.symbols.into_iter().collect(),
                type_only: exports.type_only,
            })
        })();
        let result = operations.sequence.finish(result);
        if !late_worker {
            self.module_exports.links.insert(identity, result.clone());
        }
        result.map(|links| links.map(|links| Some(links.exports)))
    }

    /// Reading `typeOnlyExportStarMap` never resolves exports. A clone's own
    /// links remain distinct from its export-type target and its raw table.
    pub(super) fn reference_module_type_only_export_declaration(
        &mut self,
        module: ReferenceSymbol,
        name: &[u8],
        consumer: &Location,
    ) -> Result<Option<ReferenceDeclaration>, DemandError> {
        let identity = self.reference_symbol_identity(module, consumer)?;
        Ok(match self.module_exports.links.get(&identity) {
            Some(Ok(links)) => links
                .value()
                .type_only
                .get(&NativePropertyKey::fixed(name))
                .copied(),
            Some(Err(_)) | None => None,
        })
    }

    /// Membership-only observation for value-module property inventories.
    pub(super) fn reference_module_type_only_export(
        &self,
        module: ReferenceSymbol,
        name: &[u8],
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        let identity = self.reference_symbol_identity(module, consumer)?;
        if let Some(links) = self.module_exports.links.get(&identity) {
            return match links {
                Ok(links) => Ok(links
                    .value()
                    .type_only
                    .contains_key(&NativePropertyKey::fixed(name))),
                Err(_) => Ok(false),
            };
        }
        Ok(false)
    }
}

struct Symbols<'a, 'p, 's> {
    checker: &'a mut Evaluator<'p, 's>,
    consumer: &'a Location,
    sequence: NativeSequence,
}

impl worker::Symbols for Symbols<'_, '_, '_> {
    type Symbol = ReferenceSymbol;
    type Identity = NativeSymbolIdentity;
    type Declaration = ReferenceDeclaration;
    type Error = DemandError;
    fn module(
        &mut self,
        symbol: ReferenceSymbol,
    ) -> Result<worker::Module<ReferenceSymbol, NativeSymbolIdentity>, DemandError> {
        let identity = self
            .checker
            .reference_symbol_identity(symbol, self.consumer)?;
        let exports = self
            .checker
            .reference_raw_symbol_table(symbol, true)?
            .map(|raw| self.checker.reference_raw_table_entries(raw, self.consumer))
            .transpose()?
            .map(|table| table.iter().cloned().collect());
        Ok(worker::Module { identity, exports })
    }
    fn resolve(
        &mut self,
        symbol: ReferenceSymbol,
    ) -> Result<worker::Resolved<ReferenceSymbol>, DemandError> {
        Ok(
            match self.sequence.read(
                self.checker
                    .resolve_reference_symbol_completion(symbol, self.consumer),
            )? {
                Some(symbol) if self.checker.is_unknown_reference_symbol(symbol) => {
                    worker::Resolved::Unknown
                }
                Some(symbol) => worker::Resolved::Symbol(symbol),
                None => worker::Resolved::Absent,
            },
        )
    }
    fn merged(&mut self, symbol: ReferenceSymbol) -> Result<ReferenceSymbol, DemandError> {
        self.checker
            .get_merged_reference_symbol(symbol, self.consumer)
    }
    fn name(&mut self, symbol: ReferenceSymbol) -> Result<NativePropertyKey, DemandError> {
        self.checker.reference_symbol_key(symbol, self.consumer)
    }
    fn stars(
        &mut self,
        symbol: ReferenceSymbol,
    ) -> Result<Vec<worker::Star<ReferenceDeclaration>>, DemandError> {
        let mut result = Vec::new();
        for declaration in self
            .checker
            .reference_symbol_declarations(symbol, self.consumer)?
        {
            let ReferenceDeclaration::Captured(captured) = declaration else {
                return Err(super::super::internal(
                    "export-star symbol contains a non-syntax declaration",
                )
                .into());
            };
            let (_, span) = self.checker.source.diagnostic_lookup_node(captured)?;
            let (_, type_only) = self
                .checker
                .inputs
                .bindings
                .reference_export_star_recipe(captured.file, span)
                .map_err(|error| self.checker.generic_binding_failure(error, self.consumer))?;
            result.push(worker::Star {
                declaration,
                type_only,
            });
        }
        Ok(result)
    }
    fn star_module(
        &mut self,
        declaration: &ReferenceDeclaration,
    ) -> Result<ReferenceSymbol, DemandError> {
        let ReferenceDeclaration::Captured(captured) = *declaration else {
            return Err(super::super::internal("export-star request lacks captured syntax").into());
        };
        let (_, span) = self.checker.source.diagnostic_lookup_node(captured)?;
        let (site, _) = self
            .checker
            .inputs
            .bindings
            .reference_export_star_recipe(captured.file, span)
            .map_err(|error| self.checker.generic_binding_failure(error, self.consumer))?;
        self.checker.reference_module_symbol(site, self.consumer)
    }
    fn supplemental_type(&mut self, symbol: ReferenceSymbol) -> Result<bool, DemandError> {
        let flags = self
            .sequence
            .read(self.checker.reference_symbol_flags_ex_completion(
                symbol,
                false,
                false,
                self.consumer,
            ))?;
        Ok(flags.type_or_namespace && !flags.value)
    }
}
