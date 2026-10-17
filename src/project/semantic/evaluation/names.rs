use super::super::model::{DemandError, NativeResult, NativeSequence};
use super::reference_symbols::ReferenceRawSymbolTable;
use super::{Evaluator, ReferenceSymbol};
use crate::project::binding::{self, NativeLookupMeaning, NativePropertyKey, NativeSymbolFlags};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;

use super::declarations::DeclarationAssociation;

struct Names<'a, 'p, 'w> {
    checker: &'a mut Evaluator<'p, 'w>,
    sequence: &'a mut NativeSequence,
    consumer: Option<&'a Location>,
}

impl binding::ScopeSymbols for Names<'_, '_, '_> {
    type Symbol = ReferenceSymbol;
    type Table = ReferenceRawSymbolTable;
    type Declaration = DeclarationAssociation;
    type Error = DemandError;
    fn control(&self) -> Option<&crate::resources::Control> {
        self.checker.control.as_ref()
    }

    fn lookup(
        &mut self,
        table: Self::Table,
        key: &NativePropertyKey,
    ) -> Result<Option<Self::Symbol>, Self::Error> {
        self.checker
            .reference_raw_table_lookup_at(table, key, self.consumer)
    }
    fn flags(&mut self, symbol: Self::Symbol) -> Result<NativeSymbolFlags, Self::Error> {
        self.checker
            .reference_symbol_native_flags_at(symbol, self.consumer)
    }
    fn transient(&mut self, symbol: Self::Symbol) -> Result<bool, Self::Error> {
        self.checker.reference_symbol_is_transient_at(symbol)
    }
    fn declarations(
        &mut self,
        symbol: Self::Symbol,
    ) -> Result<Vec<Self::Declaration>, Self::Error> {
        self.checker
            .reference_header_declarations(symbol, self.consumer)
    }
    fn declaration_kind(
        &mut self,
        declaration: Self::Declaration,
    ) -> Result<SyntaxKind, Self::Error> {
        self.checker.header_declaration_kind(declaration)
    }
    fn declaration_is_default(
        &mut self,
        declaration: Self::Declaration,
    ) -> Result<bool, Self::Error> {
        self.checker.header_declaration_default(declaration)
    }
    fn declaration_local(
        &mut self,
        declaration: Self::Declaration,
    ) -> Result<Option<Self::Symbol>, Self::Error> {
        self.checker
            .header_declaration_local(declaration, self.consumer)
    }
    fn name_matches(&mut self, symbol: Self::Symbol, name: &str) -> Result<bool, Self::Error> {
        Ok(self
            .checker
            .reference_symbol_key_at(symbol, self.consumer)?
            .string_value()
            .equals_text(name))
    }
}

impl binding::NameSymbols for Names<'_, '_, '_> {
    fn bound(&mut self, symbol: binding::BoundSymbol) -> ReferenceSymbol {
        symbol.into()
    }
    fn retained_table(&mut self, table: binding::NativeRawSymbolTable) -> ReferenceRawSymbolTable {
        ReferenceRawSymbolTable::Retained(table)
    }
    fn globals(&mut self) -> Result<ReferenceRawSymbolTable, DemandError> {
        Ok(ReferenceRawSymbolTable::Created(
            self.checker.initialization.globals_table()?,
        ))
    }
    fn merged(&mut self, symbol: ReferenceSymbol) -> Result<ReferenceSymbol, DemandError> {
        self.checker
            .get_merged_reference_symbol_at(symbol, self.consumer)
    }
    fn table(
        &mut self,
        symbol: ReferenceSymbol,
        exports: bool,
    ) -> Result<Option<ReferenceRawSymbolTable>, DemandError> {
        self.checker.reference_raw_symbol_table(symbol, exports)
    }
    fn select(
        &mut self,
        symbol: ReferenceSymbol,
        meaning: NativeLookupMeaning,
        direct: bool,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.sequence
            .read(self.checker.select_reference_symbol_at_completion(
                symbol,
                meaning,
                direct,
                self.consumer,
            ))
    }
    fn arguments(&mut self) -> Result<ReferenceSymbol, DemandError> {
        self.checker.initialization.arguments_symbol()
    }
}

impl Evaluator<'_, '_> {
    /// resolveName's getSymbol calls retain recovery in the calling operation.
    /// This is a representative wrapper; entity-head and captured-node lookup
    /// use the same Names adapter and their existing BindingFiles recipes.
    pub(super) fn reference_global_name_completion(
        &mut self,
        name: &str,
        meaning: NativeLookupMeaning,
        consumer: Option<&Location>,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let inputs = self.source.bindings();
        let mut sequence = NativeSequence::new();
        let result = {
            let mut operations = Names {
                checker: self,
                sequence: &mut sequence,
                consumer,
            };
            inputs.select_global_name(&mut operations, name, meaning)
        }
        .map_err(|failure| match failure {
            binding::NativeSelectionFailure::Binding(failure) => {
                super::super::binding_failure(self.inputs, consumer, failure)
            }
            binding::NativeSelectionFailure::Selection(failure) => failure,
        });
        sequence.finish(result)
    }
}

impl Evaluator<'_, '_> {
    pub(super) fn reference_entity_head_completion(
        &mut self,
        recipe: &binding::EntityNameRecipe,
        meaning: NativeLookupMeaning,
        consumer: &Location,
    ) -> super::super::model::NativeResult<Option<ReferenceSymbol>> {
        let inputs = self.source.bindings();
        let mut sequence = NativeSequence::new();
        let result = inputs
            .select_entity_head(
                &mut Names {
                    checker: self,
                    sequence: &mut sequence,
                    consumer: Some(consumer),
                },
                recipe,
                meaning,
            )
            .map_err(|failure| self.reference_selection_failure(failure, consumer));
        sequence.finish(result)
    }
    pub(super) fn reference_name_at_node_completion(
        &mut self,
        file: crate::project::sources::FileId,
        kind: SyntaxKind,
        span: crate::Span,
        name: &str,
        meaning: NativeLookupMeaning,
        consumer: &Location,
    ) -> super::super::model::NativeResult<Option<ReferenceSymbol>> {
        let inputs = self.source.bindings();
        let mut sequence = NativeSequence::new();
        let result = inputs
            .select_name_at_node(
                &mut Names {
                    checker: self,
                    sequence: &mut sequence,
                    consumer: Some(consumer),
                },
                file,
                kind,
                span,
                name,
                meaning,
            )
            .map_err(|failure| self.reference_selection_failure(failure, consumer));
        sequence.finish(result)
    }
    pub(super) fn reference_name_completion(
        &mut self,
        file: crate::project::sources::FileId,
        span: crate::Span,
        meaning: NativeLookupMeaning,
        consumer: &Location,
    ) -> super::super::model::NativeResult<(Option<ReferenceSymbol>, binding::TypeBindingFailure)>
    {
        let inputs = self.source.bindings();
        let mut sequence = NativeSequence::new();
        let result = inputs
            .select_reference_name(
                &mut Names {
                    checker: self,
                    sequence: &mut sequence,
                    consumer: Some(consumer),
                },
                file,
                span,
                meaning,
            )
            .map_err(|failure| self.reference_selection_failure(failure, consumer));
        sequence.finish(result)
    }
    pub(super) fn reference_symbol_is_transient_at(
        &self,
        symbol: ReferenceSymbol,
    ) -> Result<bool, DemandError> {
        if let Some(origin) = self.reference_raw_origin(symbol)? {
            return Ok(self
                .source
                .bindings()
                .native_symbol_is_transient(origin.symbol()));
        }
        Ok(true)
    }
}

/// The lexical observer borrows this view for one completed phase or final
/// admission. It cannot acquire a type/alias or attach/parse a declaration.
pub(in crate::project) struct CurrentFacts<'a, 'p, 'w> {
    pub(super) checker: &'a Evaluator<'p, 'w>,
}

fn native_failure(failure: DemandError) -> binding::LookupFailure {
    binding::LookupFailure::Native(failure.into())
}

struct HeaderFacts<'a, 'p, 'w> {
    checker: &'a Evaluator<'p, 'w>,
}
impl binding::ScopeSymbols for HeaderFacts<'_, '_, '_> {
    type Symbol = ReferenceSymbol;
    type Table = ReferenceRawSymbolTable;
    type Declaration = DeclarationAssociation;
    type Error = binding::LookupFailure;
    fn control(&self) -> Option<&crate::resources::Control> {
        self.checker.control.as_ref()
    }
    fn lookup(
        &mut self,
        table: Self::Table,
        key: &NativePropertyKey,
    ) -> Result<Option<Self::Symbol>, Self::Error> {
        self.checker.reference_raw_table_lookup_facts(table, key)
    }
    fn flags(&mut self, symbol: Self::Symbol) -> Result<NativeSymbolFlags, Self::Error> {
        self.checker
            .reference_symbol_native_flags_at(symbol, None)
            .map_err(native_failure)
    }
    fn transient(&mut self, symbol: Self::Symbol) -> Result<bool, Self::Error> {
        self.checker
            .reference_symbol_is_transient_at(symbol)
            .map_err(native_failure)
    }
    fn declarations(
        &mut self,
        symbol: Self::Symbol,
    ) -> Result<Vec<Self::Declaration>, Self::Error> {
        self.checker
            .reference_header_declarations(symbol, None)
            .map_err(native_failure)
    }
    fn declaration_kind(
        &mut self,
        declaration: Self::Declaration,
    ) -> Result<SyntaxKind, Self::Error> {
        self.checker
            .header_declaration_kind(declaration)
            .map_err(native_failure)
    }
    fn declaration_is_default(
        &mut self,
        declaration: Self::Declaration,
    ) -> Result<bool, Self::Error> {
        self.checker
            .header_declaration_default(declaration)
            .map_err(native_failure)
    }
    fn declaration_local(
        &mut self,
        declaration: Self::Declaration,
    ) -> Result<Option<Self::Symbol>, Self::Error> {
        self.checker.header_declaration_local_facts(declaration)
    }
    fn name_matches(&mut self, symbol: Self::Symbol, name: &str) -> Result<bool, Self::Error> {
        Ok(self
            .checker
            .reference_symbol_key_facts(symbol)?
            .string_value()
            .equals_text(name))
    }
}
impl binding::NativeFacts for CurrentFacts<'_, '_, '_> {
    fn scope_exports(
        &self,
        symbol: binding::BoundSymbol,
        external: bool,
        name: &str,
        meaning: u32,
    ) -> Result<binding::ScopeExportRead, binding::LookupFailure> {
        let symbol = self
            .checker
            .get_merged_reference_symbol_at(symbol.into(), None)
            .map_err(native_failure)?;
        let table = self
            .checker
            .reference_raw_symbol_table(symbol, true)
            .map_err(native_failure)?;
        let result = binding::scope_export_lookup(
            &mut HeaderFacts {
                checker: self.checker,
            },
            table,
            external,
            name,
            NativeLookupMeaning::from_flags(meaning),
        )?;
        Ok(match result {
            binding::ScopeExportLookup::Skip => binding::ScopeExportRead::Skip,
            binding::ScopeExportLookup::Table(table) => {
                binding::ScopeExportRead::Table(table.is_some())
            }
            binding::ScopeExportLookup::NamedDefault(symbol) => {
                binding::ScopeExportRead::NamedDefault {
                    projection: self
                        .checker
                        .reference_symbol_projection(symbol)
                        .map_err(native_failure)?
                        .ok_or(binding::LookupFailure::Binding(
                            binding::TypeBindingFailure::NotRetained,
                        ))?,
                    flags: self
                        .checker
                        .reference_symbol_native_flags_at(symbol, None)
                        .map_err(native_failure)?,
                }
            }
        })
    }
    fn flags(
        &self,
        symbol: binding::BoundSymbol,
    ) -> Result<NativeSymbolFlags, binding::LookupFailure> {
        let symbol = self
            .checker
            .get_merged_reference_symbol_at(symbol.into(), None)
            .map_err(native_failure)?;
        self.checker
            .reference_symbol_native_flags_at(symbol, None)
            .map_err(native_failure)
    }
}

impl Evaluator<'_, '_> {
    pub(super) fn reference_parameter_type_name_completion(
        &mut self,
        file: crate::project::sources::FileId,
        span: crate::Span,
        consumer: &Location,
    ) -> super::super::model::NativeResult<Option<ReferenceSymbol>> {
        let inputs = self.source.bindings();
        let mut sequence = NativeSequence::new();
        let result = inputs
            .select_parameter_type_name(
                &mut Names {
                    checker: self,
                    sequence: &mut sequence,
                    consumer: Some(consumer),
                },
                file,
                span,
            )
            .map_err(|failure| self.reference_selection_failure(failure, consumer));
        sequence.finish(result)
    }
}
