//! Checker symbol questions for reference search. Search policy never reads
//! property-arena representation or substitutes public origins for identity.

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, PropertySymbolId,
};
use super::super::source::{
    DeclarationIdentity, EntitySymbolMissing, ExpressionRoot, ReferenceSite, ReferenceSiteTarget,
    ReferenceSymbolRecipe,
};
use super::{Evaluator, SymbolParent};
use crate::project::binding::{self, NativePropertyKey, NativeSymbolIdentity, ReadOrigin};
use crate::project::sources::FileId;
use crate::references::Location;
use std::collections::HashMap;
use std::sync::Arc;

/// Immutable contents of one observed native symbol table. Accessibility's
/// native table category/cache identity is independent of these contents.
pub(in crate::project::semantic) type ReferenceSymbolTable =
    Arc<[(NativePropertyKey, ReferenceSymbol)]>;

/// Actual binder-retained Symbol.Members/Exports, including the initialized
/// globalThis header whose Exports is the shared global table.
#[derive(Clone, Copy)]
pub(super) enum ReferenceRawSymbolTable {
    Retained(binding::NativeRawSymbolTable),
    Created(usize),
}

#[derive(Clone, Copy)]
pub(in crate::project::semantic) struct ReferenceSymbol(Symbol);

impl From<crate::project::binding::BoundSymbol> for ReferenceSymbol {
    fn from(symbol: crate::project::binding::BoundSymbol) -> Self {
        Self::from_origin(ReadOrigin::Direct(symbol))
    }
}

impl ReferenceSymbol {
    pub(in crate::project::semantic) fn from_origin(origin: ReadOrigin) -> Self {
        Self(Symbol::Bound(origin))
    }
    pub(super) fn from_property(property: PropertySymbolId) -> Self {
        Self(Symbol::Property(property))
    }
    pub(super) fn property(self) -> Option<PropertySymbolId> {
        match self.0 {
            Symbol::Bound(_) => None,
            Symbol::Property(property) => Some(property),
        }
    }
}

#[derive(Clone, Copy)]
enum Symbol {
    Bound(ReadOrigin),
    Property(PropertySymbolId),
}

/// Established Symbol.Name and optional valueSymbolLinks.nameType. Reading
/// these facts never normalizes a raw symbol or allocates value-symbol links.
pub(super) struct ReferenceSymbolNameFacts {
    pub(super) key: NativePropertyKey,
    pub(super) name_type: Option<super::super::model::TypeId>,
    pub(super) late: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) enum ReferenceDeclaration {
    Captured(DeclarationIdentity),
    Json {
        file: FileId,
        declaration: binding::JsonDeclaration,
    },
}

#[derive(Clone, Copy, Default)]
pub(in crate::project::semantic) struct ReferenceSymbolFlags {
    pub(super) native: binding::NativeSymbolFlags,
    pub(in crate::project::semantic) value: bool,
    pub(in crate::project::semantic) type_: bool,
    pub(in crate::project::semantic) namespace: bool,
    pub(in crate::project::semantic) type_or_namespace: bool,
    pub(in crate::project::semantic) function: bool,
    pub(in crate::project::semantic) value_module: bool,
    pub(in crate::project::semantic) module_exports: bool,
    pub(in crate::project::semantic) variable: bool,
    pub(in crate::project::semantic) enum_: bool,
    pub(in crate::project::semantic) regular_enum: bool,
    pub(in crate::project::semantic) const_enum: bool,
    pub(in crate::project::semantic) enum_member: bool,
    pub(in crate::project::semantic) property: bool,
    pub(in crate::project::semantic) accessor: bool,
    pub(in crate::project::semantic) method: bool,
    pub(in crate::project::semantic) type_parameter: bool,
    pub(in crate::project::semantic) type_literal: bool,
    pub(in crate::project::semantic) signature: bool,
    pub(in crate::project::semantic) object_literal: bool,
    pub(in crate::project::semantic) function_scoped_variable: bool,
    pub(in crate::project::semantic) block_scoped_variable: bool,
    pub(in crate::project::semantic) synthetic: bool,
    pub(in crate::project::semantic) transient: bool,
    pub(in crate::project::semantic) alias: bool,
    pub(in crate::project::semantic) assignment: bool,
    pub(in crate::project::semantic) export_value: bool,
    pub(in crate::project::semantic) class: bool,
    pub(in crate::project::semantic) interface: bool,
    pub(in crate::project::semantic) module: bool,
}

impl ReferenceSymbolFlags {
    /// Native SymbolFlagsLateBindingContainer.
    pub(super) fn late_binding_container(self) -> bool {
        self.class || self.interface || self.type_literal || self.object_literal || self.function
    }

    /// `getTypeOfSymbol`'s native dispatch mask, before Alias and the final
    /// errorType return. SymbolFlagsValue is broader (e.g. ObjectLiteral).
    pub(super) fn has_value_type(self) -> bool {
        self.accessor
            || self.variable
            || self.property
            || self.function
            || self.method
            || self.class
            || self.enum_
            || self.value_module
            || self.enum_member
    }

    /// Native SymbolFlagsBlockScoped is a mask, not BlockScopedVariable alone.
    pub(super) fn is_block_scoped(self) -> bool {
        self.block_scoped_variable || self.class || self.enum_
    }

    /// `ast.IsNonLocalAlias(symbol, Value | Type | Namespace)` preserves
    /// mixed local meanings, except native Assignment aliases.
    pub(super) fn is_non_local_alias(self) -> bool {
        self.alias && (self.assignment || !(self.value || self.type_or_namespace))
    }

    fn union(mut self, other: Self) -> Self {
        self.native = self.native.union(other.native);
        macro_rules! include { ($($field:ident),*) => { $(self.$field |= other.$field;)* }; }
        include!(
            value,
            type_,
            namespace,
            type_or_namespace,
            function,
            value_module,
            module_exports,
            variable,
            enum_,
            regular_enum,
            const_enum,
            enum_member,
            property,
            accessor,
            method,
            type_parameter,
            type_literal,
            signature,
            object_literal,
            function_scoped_variable,
            block_scoped_variable,
            synthetic,
            transient,
            alias,
            assignment,
            export_value,
            class,
            interface,
            module
        );
        self
    }

    fn all() -> Self {
        Self {
            native: binding::NativeSymbolFlags::all_meanings(),
            value: true,
            type_: true,
            namespace: true,
            type_or_namespace: true,
            function: true,
            value_module: true,
            module_exports: true,
            variable: true,
            enum_: true,
            regular_enum: true,
            const_enum: true,
            enum_member: true,
            property: true,
            accessor: true,
            method: true,
            type_parameter: true,
            type_literal: true,
            signature: true,
            object_literal: true,
            function_scoped_variable: true,
            block_scoped_variable: true,
            synthetic: true,
            transient: true,
            alias: true,
            assignment: true,
            export_value: true,
            class: true,
            interface: true,
            module: true,
        }
    }
}

#[derive(Default)]
pub(super) struct ReferenceSymbolsState {
    /// Actual symbolNodeLinks.resolvedSymbol on access expressions. This is
    /// populated by the native access owner, before its value-type demand.
    accesses: HashMap<ExpressionRoot, PropertySymbolId>,
    identifiers: HashMap<ExpressionRoot, NativeCompletion<ReferenceSymbol>>,
    immediate_aliases: HashMap<NativeSymbolIdentity, NativeResult<Option<ReferenceSymbol>>>,
}

impl Evaluator<'_, '_> {
    /// A retained transient header can be mutated by mergeSymbol. The arena
    /// overlay preserves its identity while all raw header observations see the
    /// same request-local fields; it is not a getMergedSymbol redirection.
    pub(super) fn reference_current_header(&self, symbol: ReferenceSymbol) -> ReferenceSymbol {
        match symbol.0 {
            Symbol::Bound(origin) => self
                .symbol_tables
                .headers
                .get(&origin.symbol().native_identity())
                .copied()
                .map(ReferenceSymbol::from_property)
                .unwrap_or(symbol),
            Symbol::Property(_) => symbol,
        }
    }
    pub(in crate::project::semantic) fn reference_parent_has_global_exports(
        &self,
        parent: SymbolParent,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        match parent {
            SymbolParent::Bound(parent) => Ok(self
                .source
                .bindings()
                .parent_has_source_file_global_exports(parent)),
            SymbolParent::Created(parent) if !self.property_symbol_flags(parent)?.types.module => {
                Ok(false)
            }
            SymbolParent::Created(_) => {
                Err(self.access_pending(consumer, "created module global-export facts are pending"))
            }
        }
    }
    pub(super) fn alias_reference_symbol(
        &self,
        symbol: super::super::model::AliasSymbol,
    ) -> ReferenceSymbol {
        match symbol {
            super::super::model::AliasSymbol::Bound(symbol) => symbol.into(),
            super::super::model::AliasSymbol::Created(symbol) => {
                ReferenceSymbol::from_property(symbol)
            }
        }
    }
    /// Register a symbol in the shared arena without changing native identity.
    /// This is independent of its raw declaration or value-type operation.
    pub(super) fn register_reference_symbol(
        &mut self,
        symbol: ReferenceSymbol,
        _consumer: &Location,
    ) -> Result<PropertySymbolId, DemandError> {
        let symbol = self.reference_current_header(symbol);
        match symbol.0 {
            Symbol::Bound(origin) => self.register_property_symbol(origin.symbol()),
            Symbol::Property(property) => Ok(property),
        }
    }

    /// Raw binder association of an arena-registered symbol. Registration
    /// preserves Alias identity rather than allocating another alias cache.
    pub(super) fn reference_raw_origin(
        &self,
        symbol: ReferenceSymbol,
    ) -> Result<Option<ReadOrigin>, DemandError> {
        let symbol = self.reference_current_header(symbol);
        match symbol.0 {
            Symbol::Bound(origin) => Ok(Some(origin)),
            Symbol::Property(property) => self.property_raw_origin(property),
        }
    }

    /// Actual raw Symbol.Members/Exports, including combined Alias headers.
    /// Checker-created clones retain their own copied table snapshots.
    pub(super) fn reference_raw_symbol_table(
        &self,
        symbol: ReferenceSymbol,
        exports: bool,
    ) -> Result<Option<ReferenceRawSymbolTable>, DemandError> {
        let symbol = self.reference_current_header(symbol);
        match symbol.0 {
            Symbol::Bound(origin) => Ok(self
                .source
                .bindings()
                .raw_symbol_table(origin.symbol(), exports)
                .map(ReferenceRawSymbolTable::Retained)),
            Symbol::Property(property) => self.property_raw_symbol_table(property, exports),
        }
    }

    fn current_raw_table(&self, raw: ReferenceRawSymbolTable) -> ReferenceRawSymbolTable {
        match raw {
            ReferenceRawSymbolTable::Retained(raw) => self
                .symbol_tables
                .raw_overlays
                .get(&raw)
                .copied()
                .map(ReferenceRawSymbolTable::Created)
                .unwrap_or(ReferenceRawSymbolTable::Retained(raw)),
            ReferenceRawSymbolTable::Created(_) => raw,
        }
    }

    pub(super) fn reference_raw_table_entries(
        &self,
        raw: ReferenceRawSymbolTable,
        consumer: &Location,
    ) -> Result<ReferenceSymbolTable, DemandError> {
        self.reference_raw_table_entries_at(raw, Some(consumer))
    }

    pub(super) fn reference_raw_table_entries_at(
        &self,
        raw: ReferenceRawSymbolTable,
        consumer: Option<&Location>,
    ) -> Result<ReferenceSymbolTable, DemandError> {
        match self.current_raw_table(raw) {
            ReferenceRawSymbolTable::Retained(raw) => self
                .source
                .bindings()
                .reference_raw_table_entries(raw)
                .map(|entries| {
                    entries
                        .into_iter()
                        .map(|(key, origin)| (key, ReferenceSymbol::from_origin(origin)))
                        .collect()
                })
                .map_err(|failure| super::super::binding_failure(self.inputs, consumer, failure)),
            ReferenceRawSymbolTable::Created(index) => {
                let table = self.symbol_tables.raw.get(index).ok_or_else(|| {
                    super::super::internal("created raw table lacks its inventory")
                })?;
                Ok(table
                    .entries()
                    .map_err(|failure| {
                        super::super::binding_failure(self.inputs, consumer, failure)
                    })?
                    .iter()
                    .map(|(name, symbol)| (name.clone(), *symbol))
                    .collect())
            }
        }
    }

    pub(super) fn reference_raw_table_lookup(
        &self,
        raw: ReferenceRawSymbolTable,
        key: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.reference_raw_table_lookup_at(raw, key, Some(consumer))
    }

    pub(super) fn reference_raw_table_lookup_at(
        &self,
        raw: ReferenceRawSymbolTable,
        key: &NativePropertyKey,
        consumer: Option<&Location>,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.reference_raw_table_lookup_facts(raw, key)
            .map_err(|failure| self.current_fact_failure(failure, consumer))
    }
    /// The keyed raw-table observation owns its failure schedule. Known keys
    /// precede unavailable inventory; both semantic and lexical views use it.
    pub(super) fn reference_raw_table_lookup_facts(
        &self,
        raw: ReferenceRawSymbolTable,
        key: &NativePropertyKey,
    ) -> Result<Option<ReferenceSymbol>, binding::LookupFailure> {
        match self.current_raw_table(raw) {
            ReferenceRawSymbolTable::Retained(raw) => self
                .source
                .bindings()
                .reference_raw_table_lookup(raw, key)
                .map(|symbol| symbol.map(ReferenceSymbol::from_origin))
                .map_err(Into::into),
            ReferenceRawSymbolTable::Created(index) => self
                .symbol_tables
                .raw
                .get(index)
                .ok_or_else(|| {
                    binding::LookupFailure::Native(super::super::internal(
                        "created raw table lacks its inventory",
                    ))
                })?
                .lookup(key)
                .map_err(Into::into),
        }
    }
    pub(super) fn current_fact_failure(
        &self,
        failure: binding::LookupFailure,
        consumer: Option<&Location>,
    ) -> DemandError {
        match failure {
            binding::LookupFailure::Binding(failure) => {
                super::super::binding_failure(self.inputs, consumer, failure)
            }
            binding::LookupFailure::Native(failure) => failure.into(),
        }
    }

    /// `maps.Clone` preserves raw-table failures without enumerating keys.
    pub(super) fn clone_reference_raw_table(
        &mut self,
        raw: ReferenceRawSymbolTable,
        consumer: &Location,
    ) -> Result<ReferenceRawSymbolTable, DemandError> {
        let table = match self.current_raw_table(raw) {
            ReferenceRawSymbolTable::Retained(raw) => self
                .source
                .bindings()
                .reference_raw_table_copy(raw, ReferenceSymbol::from_origin),
            ReferenceRawSymbolTable::Created(index) => self
                .symbol_tables
                .raw
                .get(index)
                .ok_or_else(|| super::super::internal("created raw table lacks its inventory"))?
                .checker_copy(|symbol| symbol),
        }
        .map_err(|failure| self.generic_binding_failure(failure, consumer))?;
        let index = self.symbol_tables.raw.len();
        self.symbol_tables.raw.push(table);
        Ok(ReferenceRawSymbolTable::Created(index))
    }

    /// A retained transient symbol mutates its actual table. All retained
    /// handles of that table observe this request-local slot; clones get their
    /// own slot through clone_reference_raw_table instead.
    pub(super) fn mutable_reference_raw_table(
        &mut self,
        raw: ReferenceRawSymbolTable,
        consumer: &Location,
    ) -> Result<usize, DemandError> {
        match self.current_raw_table(raw) {
            ReferenceRawSymbolTable::Created(index) => Ok(index),
            ReferenceRawSymbolTable::Retained(raw) => {
                let ReferenceRawSymbolTable::Created(index) = self
                    .clone_reference_raw_table(ReferenceRawSymbolTable::Retained(raw), consumer)?
                else {
                    unreachable!("a copied table is checker-owned");
                };
                self.symbol_tables.raw_overlays.insert(raw, index);
                Ok(index)
            }
        }
    }

    pub(super) fn new_reference_raw_table(&mut self) -> usize {
        let index = self.symbol_tables.raw.len();
        self.symbol_tables.raw.push(Default::default());
        index
    }

    /// Raw `SymbolFlagsConstEnumOnlyModule`, including copied actual headers.
    pub(super) fn reference_symbol_const_enum_only_module(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        Ok(self
            .reference_symbol_native_flags(symbol, consumer)?
            .const_enum_only_module())
    }

    /// Actual captured Node.Locals, with native nil distinct from empty.
    pub(super) fn reference_raw_locals(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<ReferenceSymbolTable>, DemandError> {
        let native = self.source.declaration_ordering(declaration)?;
        let kind = self.source.declaration_kind(declaration)?;
        let consumer = self.source.declaration_location(declaration)?;
        self.source
            .bindings()
            .reference_raw_locals(native.file, kind, native.span)
            .map(|table| {
                table.map(|entries| {
                    entries
                        .into_iter()
                        .map(|(key, origin)| (key, ReferenceSymbol::from_origin(origin)))
                        .collect()
                })
            })
            .map_err(|error| self.generic_binding_failure(error, &consumer))
    }

    pub(super) fn reference_globals(
        &self,
        consumer: &Location,
    ) -> Result<ReferenceSymbolTable, DemandError> {
        self.reference_raw_table_entries(
            ReferenceRawSymbolTable::Created(self.initialization.globals_table()?),
            consumer,
        )
    }

    /// Equality-only name observation. Generated keys retain owner identity;
    /// no display conversion or lazy identity ordering is performed here.
    pub(in crate::project::semantic) fn reference_symbol_key(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<NativePropertyKey, DemandError> {
        self.reference_symbol_key_at(symbol, Some(consumer))
    }

    pub(super) fn reference_symbol_key_at(
        &self,
        symbol: ReferenceSymbol,
        consumer: Option<&Location>,
    ) -> Result<NativePropertyKey, DemandError> {
        self.reference_symbol_key_facts(symbol)
            .map_err(|failure| self.current_fact_failure(failure, consumer))
    }
    pub(super) fn reference_symbol_key_facts(
        &self,
        symbol: ReferenceSymbol,
    ) -> Result<NativePropertyKey, binding::LookupFailure> {
        let symbol = self.reference_current_header(symbol);
        match symbol.0 {
            Symbol::Bound(origin) => self
                .source
                .bindings()
                .read_origin_key(origin)
                .map_err(Into::into),
            Symbol::Property(property) => self.property_native_key_facts(property),
        }
    }

    /// `getSymbol(getExportsOfModule(module), name, Value)` shares binding's
    /// table/alias eligibility and never consumes the exported value's type.
    pub(super) fn reference_module_value_export_completion(
        &mut self,
        module: ReferenceSymbol,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let table =
                sequence.read(self.reference_exports_of_module_completion(module, consumer))?;
            let symbol = table.and_then(|table| {
                table
                    .iter()
                    .find(|(key, _)| key.fixed_bytes() == Some(name.as_bytes()))
                    .map(|(_, symbol)| *symbol)
            });
            let Some(symbol) = symbol else {
                return Ok(None);
            };
            let symbol = self.get_merged_reference_symbol(symbol, consumer)?;
            if self.reference_symbol_flags(symbol, consumer)?.value
                || sequence
                    .read(
                        self.reference_symbol_flags_ex_completion(symbol, false, false, consumer),
                    )?
                    .value
            {
                Ok(Some(symbol))
            } else {
                Ok(None)
            }
        })();
        sequence.finish(result)
    }

    /// `Symbol.Exports[name]` observes a copied raw table without resolving
    /// stars or following the selected alias.
    pub(super) fn reference_own_module_export(
        &self,
        module: ReferenceSymbol,
        name: &str,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        match self.reference_raw_symbol_table(module, true)? {
            Some(raw) => self.reference_raw_table_lookup(
                raw,
                &NativePropertyKey::fixed(name.as_bytes()),
                consumer,
            ),
            None => Ok(None),
        }
    }

    /// `getDeclarationNodeFlagsFromSymbol` observes only ValueDeclaration;
    /// JSON nodes carry no combined declaration flags.
    pub(super) fn reference_declaration_node_flags(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<u32, DemandError> {
        match self.reference_symbol_value_declaration(symbol, consumer)? {
            Some(ReferenceDeclaration::Captured(declaration)) => {
                self.source.constant_declaration_flags(declaration)
            }
            Some(ReferenceDeclaration::Json { .. }) | None => Ok(0),
        }
    }

    /// `getExportOfModule` selects getExportsOfSymbol's unfiltered entry.
    /// Late containers take priority even when a merged header is a Module.
    pub(super) fn reference_module_export_symbol_completion(
        &mut self,
        module: ReferenceSymbol,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let table =
                sequence.read(self.reference_exports_of_symbol_completion(module, consumer))?;
            Ok(table.and_then(|table| {
                table
                    .iter()
                    .find(|(key, _)| key.fixed_bytes() == Some(name.as_bytes()))
                    .map(|(_, symbol)| *symbol)
            }))
        })();
        sequence.finish(result)
    }

    pub(super) fn reference_module_type_only_export_key(
        &self,
        module: ReferenceSymbol,
        key: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        // Source export names are ordinary text. Generated property identities
        // cannot occur in the native typeOnlyExportStarMap.
        match key.fixed_bytes() {
            Some(name) => self.reference_module_type_only_export(module, name, consumer),
            None => Ok(false),
        }
    }

    /// `ast.GetSourceFileOfModule` asks ValueDeclaration before the first
    /// nonaugmentation declaration. This differs from Find(IsSourceFile).
    pub(super) fn source_file_of_module(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<FileId>, DemandError> {
        if let Some(declaration) = self.reference_symbol_value_declaration(symbol, consumer)? {
            return Ok(Some(match declaration {
                ReferenceDeclaration::Captured(declaration) => declaration.file,
                ReferenceDeclaration::Json { file, .. } => file,
            }));
        }
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            match declaration {
                ReferenceDeclaration::Captured(declaration) => {
                    if self
                        .source
                        .diagnostic_non_augmentation_declaration(declaration)?
                    {
                        return Ok(Some(declaration.file));
                    }
                }
                ReferenceDeclaration::Json { file, .. } => return Ok(Some(file)),
            }
        }
        Ok(None)
    }

    /// `getSymbolFlags` follows aliases even when resolveAlias stops at mixed
    /// local meanings. The cycle set is populated only for alias targets.
    pub(super) fn reference_combined_symbol_flags_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<ReferenceSymbolFlags> {
        let mut sequence = NativeSequence::new();
        let result = sequence
            .read(self.reference_symbol_flags_ex_completion(symbol, false, false, consumer));
        sequence.finish(result)
    }

    /// `getSymbolFlagsEx` owns both native exclusion modes and alias cycles.
    pub(super) fn reference_symbol_flags_ex_completion(
        &mut self,
        mut symbol: ReferenceSymbol,
        exclude_type_only: bool,
        exclude_local: bool,
        consumer: &Location,
    ) -> NativeResult<ReferenceSymbolFlags> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut own = self.reference_symbol_flags(symbol, consumer)?;
            let mut flags = if exclude_local {
                ReferenceSymbolFlags::default()
            } else {
                own
            };
            let mut seen = std::collections::HashSet::new();
            while own.alias {
                if exclude_type_only
                    && sequence
                        .read(self.reference_alias_is_type_only_completion(symbol, consumer))?
                {
                    break;
                }
                let Some(target) =
                    sequence.read(self.resolve_alias_target_completion(symbol, consumer))?
                else {
                    return Err(super::super::internal("resolveAlias returned native nil").into());
                };
                let target = self.reference_export_symbol_of_value_if_exported(target, consumer)?;
                if self.is_unknown_reference_symbol(target) {
                    return Ok(ReferenceSymbolFlags::all());
                }
                let next = self.reference_symbol_flags(target, consumer)?;
                if next.alias {
                    let identity = self.reference_symbol_identity(target, consumer)?;
                    let current = self.reference_symbol_identity(symbol, consumer)?;
                    if identity == current || seen.contains(&identity) {
                        break;
                    }
                    if seen.is_empty() {
                        seen.insert(current);
                    }
                    seen.insert(identity);
                }
                flags = flags.union(next);
                symbol = target;
                own = next;
            }
            Ok(flags)
        })();
        sequence.finish(result)
    }
    /// `isNamedMember` / `symbolIsValueEx` retains the native reserved byte
    /// prefix and excludes type-only alias meanings from the named property list.
    pub(super) fn reference_is_named_member_completion(
        &mut self,
        symbol: ReferenceSymbol,
        key: &NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if key.is_reserved_member_name() {
                return Ok(false);
            }
            sequence.read(self.reference_symbol_is_value_completion(symbol, consumer))
        })();
        sequence.finish(result)
    }

    /// `symbolIsValue` excludes type-only Alias meanings, but has no name gate.
    pub(super) fn reference_symbol_is_value_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result =
            sequence.read(self.reference_symbol_is_value_ex_completion(symbol, false, consumer));
        sequence.finish(result)
    }

    /// `symbolIsValueEx` keeps QualifiedName's includeTypeOnlyMembers policy
    /// separate from ordinary value lookups while sharing alias traversal.
    pub(super) fn reference_symbol_is_value_ex_completion(
        &mut self,
        symbol: ReferenceSymbol,
        include_type_only: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let flags = self.reference_symbol_flags(symbol, consumer)?;
            Ok(flags.value
                || flags.alias
                    && sequence
                        .read(self.reference_symbol_flags_ex_completion(
                            symbol,
                            !include_type_only,
                            false,
                            consumer,
                        ))?
                        .value)
        })();
        sequence.finish(result)
    }

    /// Actual `Type.symbol`, with nil kept separate from unavailable synthetic
    /// metadata. No declaration, type, or alias-target question is performed.
    pub(super) fn reference_symbol_of_type(
        &self,
        ty: super::super::model::TypeId,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        use super::super::model::TypeSymbol;
        match self.model.type_symbol(ty) {
            TypeSymbol::Absent => Ok(None),
            TypeSymbol::Bound { symbol, .. } => Ok(Some(symbol.into())),
            TypeSymbol::Property { symbol, .. } => Ok(Some(ReferenceSymbol::from_property(symbol))),
            TypeSymbol::Synthetic { .. } => Err(self.access_pending(
                consumer,
                "native synthetic symbol metadata requires its shared symbol association",
            )),
        }
    }

    /// Attach an actual symbol to a newly allocated native type. This does not
    /// resolve aliases or substitute a declaration's merged symbol.
    pub(super) fn type_symbol_of_reference(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<super::super::model::TypeSymbol, DemandError> {
        self.type_symbol_of_reference_at(symbol, Some(consumer))
    }
    pub(super) fn type_symbol_of_reference_at(
        &self,
        symbol: ReferenceSymbol,
        consumer: Option<&Location>,
    ) -> Result<super::super::model::TypeSymbol, DemandError> {
        use super::super::model::TypeSymbol;
        let symbol = self.reference_current_header(symbol);
        Ok(match symbol.0 {
            Symbol::Bound(origin) => TypeSymbol::Bound {
                symbol: origin.symbol(),
                flags: self
                    .source
                    .bindings()
                    .type_symbol_flags(origin.symbol())
                    .map_err(|failure| {
                        super::super::binding_failure(self.inputs, consumer, failure)
                    }),
                native: self
                    .source
                    .bindings()
                    .native_symbol(origin.symbol())
                    .map_err(|failure| {
                        super::super::binding_failure(self.inputs, consumer, failure)
                    }),
            },
            Symbol::Property(symbol) => TypeSymbol::Property {
                symbol,
                native: self.property_native_symbol_at(symbol, consumer)?,
            },
        })
    }

    /// `getMergedSymbol` observes one checker map entry for every retained
    /// header. Created/instantiated property headers are already checker symbols.
    pub(super) fn get_merged_reference_symbol(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<ReferenceSymbol, DemandError> {
        self.get_merged_reference_symbol_at(symbol, Some(consumer))
    }
    /// `getMergedSymbol` observes exactly one evaluator-local redirect. Binder
    /// identity is never canonicalized by lexical projection or another checker.
    pub(super) fn get_merged_reference_symbol_at(
        &self,
        symbol: ReferenceSymbol,
        consumer: Option<&Location>,
    ) -> Result<ReferenceSymbol, DemandError> {
        let identity = self.reference_symbol_identity_at(symbol, consumer)?;
        Ok(self
            .symbol_tables
            .merged
            .get(&identity)
            .copied()
            .unwrap_or_else(|| self.reference_current_header(symbol)))
    }
    /// Auxiliary navigation association copied by cloneSymbol. It is not an
    /// input to native identities, type caches, aliases, or header selection.
    pub(super) fn reference_symbol_projection(
        &self,
        symbol: ReferenceSymbol,
    ) -> Result<Option<binding::SymbolId>, DemandError> {
        let identity = self.reference_symbol_identity_at(symbol, None)?;
        if let Some(projection) = self.symbol_tables.projections.get(&identity) {
            return Ok(Some(*projection));
        }
        Ok(binding::BoundSymbol::from_native_identity(identity)
            .and_then(|symbol| self.source.bindings().symbol_projection(symbol)))
    }

    pub(super) fn reference_symbol_name_facts(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<ReferenceSymbolNameFacts, DemandError> {
        let symbol = self.reference_current_header(symbol);
        match symbol.0 {
            Symbol::Bound(origin) => Ok(ReferenceSymbolNameFacts {
                key: self
                    .source
                    .bindings()
                    .read_origin_key(origin)
                    .map_err(|error| self.generic_binding_failure(error, consumer))?,
                name_type: None,
                late: false,
            }),
            Symbol::Property(property) => {
                let facts = self.property_symbol_name_facts(property, consumer)?;
                Ok(ReferenceSymbolNameFacts {
                    key: facts.key,
                    name_type: facts.name_type,
                    late: self.property_check_flags(property)?
                        & super::property_symbols::checks::LATE
                        != 0,
                })
            }
        }
    }
    pub(in crate::project::semantic) fn reference_module_symbol(
        &self,
        site: binding::ReferenceModuleSite,
        consumer: &Location,
    ) -> Result<ReferenceSymbol, DemandError> {
        self.reference_module_symbol_at(site, Some(consumer))
    }
    pub(super) fn reference_module_symbol_at(
        &self,
        site: binding::ReferenceModuleSite,
        consumer: Option<&Location>,
    ) -> Result<ReferenceSymbol, DemandError> {
        let selected = self
            .source
            .bindings()
            .module_selection(site)
            .map_err(|failure| super::super::binding_failure(self.inputs, consumer, failure))?;
        let symbol = selected
            .pattern_name
            .and_then(|name| self.initialization.pattern_augmentations.get(name).copied())
            .unwrap_or_else(|| selected.symbol.into());
        self.get_merged_reference_symbol_at(symbol, consumer)
    }
    pub(super) fn reference_module_symbol_ignore_errors(
        &self,
        site: binding::ReferenceModuleSite,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let selected = self
            .source
            .bindings()
            .module_selection_ignore_errors(site)
            .map_err(|failure| self.generic_binding_failure(failure, consumer))?;
        selected
            .map(|_| self.reference_module_symbol_at(site, Some(consumer)))
            .transpose()
    }

    pub(super) fn global_this_reference_symbol(&self) -> ReferenceSymbol {
        self.initialization.global_this_symbol()
    }

    pub(super) fn reference_raw_parent_symbol(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.reference_symbol_parent(symbol, consumer)?
            .map(|parent| match parent {
                SymbolParent::Bound(parent) => self
                    .source
                    .bindings()
                    .raw_parent_symbol(parent)
                    .map(ReferenceSymbol::from)
                    .map_err(|error| self.generic_binding_failure(error, consumer)),
                SymbolParent::Created(parent) => Ok(ReferenceSymbol::from_property(parent)),
            })
            .transpose()
    }

    /// `getParentOfSymbol`: retain the raw Parent, acquire its late symbol,
    /// then getMergedSymbol. A replacement cannot be normalized before this.
    pub(super) fn reference_parent_of_symbol(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.reference_parent_of_symbol_completion(symbol, consumer)?
            .admit()
    }

    pub(super) fn reference_parent_of_symbol_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let Some(parent) = self.reference_raw_parent_symbol(symbol, consumer)? else {
            return Ok(NativeCompletion::clean(None));
        };
        self.reference_late_symbol_completion(parent, consumer)?
            .and_then_value(|parent| self.get_merged_reference_symbol(parent, consumer).map(Some))
    }

    /// `getExportsOfSymbol` selects late containers before ordinary modules.
    pub(super) fn reference_exports_of_symbol(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbolTable>, DemandError> {
        self.reference_exports_of_symbol_completion(symbol, consumer)?
            .admit()
    }
    pub(super) fn reference_exports_of_symbol_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbolTable>> {
        let flags = self.reference_symbol_flags(symbol, consumer)?;
        if flags.late_binding_container() {
            return self.resolved_symbol_table_completion(symbol, true, consumer);
        }
        if flags.module {
            return self.reference_exports_of_module_completion(symbol, consumer);
        }
        Ok(NativeCompletion::clean(
            self.reference_raw_symbol_table(symbol, true)?
                .map(|raw| self.reference_raw_table_entries(raw, consumer))
                .transpose()?,
        ))
    }
    /// `getMembersOfSymbol` acquires its own complete table automatically.
    pub(super) fn reference_members_of_symbol(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbolTable>, DemandError> {
        self.reference_members_of_symbol_completion(symbol, consumer)?
            .admit()
    }
    pub(super) fn reference_members_of_symbol_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbolTable>> {
        if self
            .reference_symbol_flags(symbol, consumer)?
            .late_binding_container()
        {
            return self.resolved_symbol_table_completion(symbol, false, consumer);
        }
        Ok(NativeCompletion::clean(
            self.reference_raw_symbol_table(symbol, false)?
                .map(|raw| self.reference_raw_table_entries(raw, consumer))
                .transpose()?,
        ))
    }

    /// `getExportsOfModule` owns its cache and type-only star map, independently
    /// of late-container resolvedExports and its discarded worker map.
    pub(super) fn reference_exports_of_module_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbolTable>> {
        self.reference_module_exports_worker_completion(symbol, false, consumer)
    }
    pub(in crate::project::semantic) fn reference_source_module_symbol(
        &self,
        file: FileId,
    ) -> Option<ReferenceSymbol> {
        self.source
            .bindings()
            .raw_file_module(file)
            .map(ReferenceSymbol::from)
    }

    pub(in crate::project::semantic) fn reference_module_parent_symbol(
        &self,
        parent: SymbolParent,
    ) -> ReferenceSymbol {
        match parent {
            SymbolParent::Bound(parent) => ReferenceSymbol::from(
                self.source
                    .bindings()
                    .raw_parent_symbol(parent)
                    .expect("retained raw parent header"),
            ),
            SymbolParent::Created(parent) => ReferenceSymbol::from_property(parent),
        }
    }

    pub(in crate::project::semantic) fn reference_merged_symbol_parent(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<SymbolParent>, DemandError> {
        self.reference_symbol_parent(symbol, consumer)
    }

    pub(in crate::project::semantic) fn reference_parent_is_external_module(
        &self,
        parent: SymbolParent,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        match parent {
            SymbolParent::Bound(parent) => self
                .source
                .bindings()
                .parent_is_external_module(parent)
                .map_err(|error| self.generic_binding_failure(error, consumer)),
            SymbolParent::Created(parent) if !self.property_symbol_flags(parent)?.types.module => {
                Ok(false)
            }
            SymbolParent::Created(_) => {
                Err(self
                    .access_pending(consumer, "created module external-parent facts are pending"))
            }
        }
    }

    /// `getTargetOfAliasDeclaration` remains an uncached native operation;
    /// the direct getTypeOfAlias caller cannot skip it after resolveAlias.
    pub(super) fn reference_alias_declaration_target_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = sequence.read(self.checker_alias_target_completion(symbol, consumer));
        sequence.finish(result)
    }

    /// `getResolvedSymbol` caches the actual raw symbol, including native
    /// unknownSymbol, after resolveName(Value | ExportValue) completes. Alias
    /// eligibility may acquire types, so its recovery belongs to this link.
    pub(super) fn reference_resolved_identifier_symbol_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<ReferenceSymbol> {
        if let Some(symbol) = self.reference_symbols.identifiers.get(&root) {
            return Ok(symbol.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let (symbol, missing) = sequence.read(self.reference_name_completion(
                root.file,
                root.site.span,
                binding::NativeLookupMeaning::VALUE_OR_EXPORT,
                consumer,
            ))?;
            match symbol {
                Some(symbol) => Ok(symbol),
                None => {
                    let symbol = self.unknown_reference_symbol();
                    let failure = match missing {
                        binding::TypeBindingFailure::Unresolved(message)
                            if self.source.flow_syntax(root.file, root.site)?.in_type_query =>
                        {
                            super::super::invalid_at_location(
                                self.inputs,
                                consumer,
                                crate::analysis::SemanticFailureReason::ErrorType,
                                message,
                            )
                            .into()
                        }
                        missing => self.generic_binding_failure(missing, consumer),
                    };
                    sequence.read(NativeCompletion::recovered(symbol, failure))
                }
            }
        })();
        let result = sequence.finish(result);
        if let Ok(symbol) = &result {
            self.reference_symbols
                .identifiers
                .insert(root, symbol.clone());
        }
        result
    }

    /// The quiet `resolveName(..., Value)` question is separate from the
    /// installed getResolvedSymbol link and resolveEntityName's merge step.
    pub(super) fn reference_value_name_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let (symbol, _) = sequence.read(self.reference_name_completion(
                root.file,
                root.site.span,
                binding::NativeLookupMeaning::VALUE,
                consumer,
            ))?;
            Ok(symbol)
        })();
        sequence.finish(result)
    }

    /// `getResolvedSymbolOrNil` reads only an installed identifier/access
    /// link; it never resolves a name or re-enters product selection.
    pub(super) fn reference_resolved_symbol_if_any(
        &self,
        root: ExpressionRoot,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        Ok(self
            .reference_symbols
            .identifiers
            .get(&root)
            .map(|symbol| *symbol.value())
            .or_else(|| {
                self.reference_symbols
                    .accesses
                    .get(&root)
                    .copied()
                    .map(ReferenceSymbol::from_property)
            }))
    }

    /// `getImmediateAliasedSymbol` has its own native immediateTarget cache,
    /// distinct from both resolveAlias and the direct declaration operation.
    pub(in crate::project::semantic) fn reference_immediate_aliased_symbol_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        if !self.reference_symbol_flags(symbol, consumer)?.alias {
            return Ok(NativeCompletion::clean(None));
        }
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        if let Some(result) = self.reference_symbols.immediate_aliases.get(&identity) {
            return result.clone();
        }
        if self
            .reference_alias_declaration(symbol, consumer)?
            .is_none()
        {
            return Err(super::super::internal(
                "immediate alias symbol lacks an admitted alias declaration",
            )
            .into());
        }
        let result = self.reference_alias_declaration_target_completion(symbol, consumer);
        // Native nil immediateTarget is not an installed cache entry. Failed
        // semantic acquisitions retain their original product failure.
        if !matches!(&result, Ok(value) if value.value().is_none()) {
            self.reference_symbols
                .immediate_aliases
                .insert(identity, result.clone());
        }
        result
    }

    pub(super) fn reference_entity_name_symbol_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let anchor = self.source.reference_entity_name_anchor(root)?;
        self.reference_entity_at_completion(
            root.file,
            anchor,
            false,
            binding::NativeLookupMeaning::ALL.with_alias(),
            false,
            consumer,
        )
        .map(|result| result.map(|(symbol, _)| symbol))
    }

    pub(in crate::project::semantic) fn reference_export_specifier_local_target(
        &mut self,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let name = self.source.symbol_alias_target(declaration)?;
        match name {
            super::super::source::symbol_aliases::SymbolAliasTarget::LocalExport(Some(name)) => {
                let anchor = self.source.reference_entity_name_anchor(name)?;
                self.reference_entity_at_completion(
                    name.file,
                    anchor,
                    false,
                    binding::NativeLookupMeaning::ALL.with_alias(),
                    true,
                    consumer,
                )
                .map(|completed| completed.map(|(symbol, _)| symbol))
                .and_then(NativeCompletion::admit)
            }
            super::super::source::symbol_aliases::SymbolAliasTarget::LocalExport(None) => Ok(None),
            _ => {
                let symbol = self
                    .reference_symbol_of_declaration_completion(declaration)?
                    .admit()?;
                self.checker_alias_target_completion(symbol, consumer)?
                    .admit()
            }
        }
    }
    pub(in crate::project::semantic) fn reference_umd_alias(
        &mut self,
        symbol: ReferenceSymbol,
        site: &ReferenceSite,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        if self
            .source
            .reference_namespace_export_declaration(site)?
            .is_none()
        {
            return Ok(None);
        }
        let Some(target) = self
            .resolve_alias_target_completion(symbol, &site.selection)?
            .admit()?
        else {
            return Ok(None);
        };
        if self.is_unknown_reference_symbol(target) {
            return Ok(None);
        }
        let merged = self.get_merged_reference_symbol(target, &site.selection)?;
        Ok((self.reference_symbol_identity(target, &site.selection)?
            != self.reference_symbol_identity(merged, &site.selection)?)
        .then_some(merged))
    }

    /// `SymbolName` preserves a private identifier's declaration spelling;
    /// other symbols use their actual native Name, not a type summary.
    pub(in crate::project::semantic) fn reference_symbol_display_name(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<String, DemandError> {
        String::from_utf8(self.reference_symbol_display_name_bytes(symbol, consumer)?).map_err(
            |_| {
                super::super::unsupported_demand_at_location(
                    self.inputs,
                    consumer,
                    crate::analysis::SemanticFailureReason::UnrepresentableIdentity,
                    "native reference symbol name is not representable as exact text",
                )
            },
        )
    }

    pub(in crate::project::semantic) fn reference_symbol_display_name_bytes(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Vec<u8>, DemandError> {
        if let Some(ReferenceDeclaration::Captured(declaration)) =
            self.reference_symbol_value_declaration(symbol, consumer)?
            && let Some(name) = self
                .source
                .private_identifier_declaration_name(declaration)?
        {
            return Ok(name.into_bytes());
        }
        self.reference_symbol_name(symbol, consumer)
    }

    pub(in crate::project::semantic) fn reference_symbol_of_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<ReferenceSymbol, DemandError> {
        self.reference_symbol_of_declaration_completion(declaration)?
            .admit()
    }
    pub(in crate::project::semantic) fn reference_symbol_of_declaration_completion(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> NativeResult<ReferenceSymbol> {
        self.reference_node_declaration_symbol_completion(declaration)?
            .and_then_value(|symbol| {
                symbol.ok_or_else(|| {
                    super::super::internal("required declaration has no native symbol").into()
                })
            })
    }

    pub(in crate::project::semantic) fn reference_raw_symbol_of_node(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let declaration = self.source.type_node_declaration(root)?;
        self.reference_raw_symbol_of_declaration(declaration)
    }

    pub(in crate::project::semantic) fn reference_raw_symbol_of_declaration(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.source
            .raw_node_symbol_origin(declaration)
            .map(|origin| origin.map(ReferenceSymbol::from_origin))
    }

    pub(in crate::project::semantic) fn reference_declaration_local_symbol(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.source
            .declaration_local_symbol(declaration)
            .map(|origin| origin.map(ReferenceSymbol::from_origin))
    }

    /// `getExportSymbolOfValueSymbolIfExported` substitutes the actual export
    /// link only for ExportValue, then merges even when no substitution occurs.
    pub(super) fn reference_export_symbol_of_value_if_exported(
        &self,
        mut symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<ReferenceSymbol, DemandError> {
        if self.reference_symbol_flags(symbol, consumer)?.export_value
            && let Some(export) = self.reference_export_symbol(symbol, consumer)?
        {
            symbol = export;
        }
        self.get_merged_reference_symbol(symbol, consumer)
    }

    pub(in crate::project::semantic) fn reference_export_symbol(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let Some(origin) = self.reference_raw_origin(symbol)? else {
            return Ok(None);
        };
        self.source
            .bindings()
            .reference_export_symbol(origin)
            .map(|origin| origin.map(ReferenceSymbol::from_origin))
            .map_err(|error| self.generic_binding_failure(error, consumer))
    }

    pub(super) fn reference_node_declaration_symbol(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.reference_node_declaration_symbol_completion(declaration)?
            .admit()
    }
    /// `getSymbolOfDeclaration` / `getSymbolOfNode`: raw node.Symbol, late,
    /// then exactly one merge-map observation. Genuine nil stays nil.
    pub(super) fn reference_node_declaration_symbol_completion(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let Some(symbol) = self.reference_raw_symbol_of_declaration(declaration)? else {
            return Ok(NativeCompletion::clean(None));
        };
        let consumer = self.source.declaration_location(declaration)?;
        self.reference_late_symbol_completion(symbol, &consumer)?
            .and_then_value(|symbol| {
                self.get_merged_reference_symbol(symbol, &consumer)
                    .map(Some)
            })
    }
    pub(in crate::project::semantic) fn reference_declaration_location(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<Location, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.declaration_location(declaration)
            }
            ReferenceDeclaration::Json { file, declaration } => Ok(super::super::location(
                self.inputs,
                file,
                self.inputs.bindings.json_declaration_location_span(
                    file,
                    declaration,
                    self.inputs.sources.get(file).text.as_str().len(),
                ),
            )),
        }
    }

    pub(in crate::project::semantic) fn reference_parent_is_class_or_interface(
        &self,
        parent: SymbolParent,
    ) -> Result<bool, DemandError> {
        match parent {
            SymbolParent::Bound(parent) => {
                Ok(self.source.bindings().parent_is_class_or_interface(parent))
            }
            SymbolParent::Created(parent) => {
                let flags = self.property_symbol_flags(parent)?.types;
                Ok(flags.class || flags.interface)
            }
        }
    }

    pub(in crate::project::semantic) fn reference_parent_limits_scope(
        &self,
        parent: SymbolParent,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        Ok(self.reference_parent_is_external_module(parent, consumer)?
            && !self.reference_parent_has_global_exports(parent, consumer)?)
    }

    pub(in crate::project::semantic) fn reference_file_is_external_or_commonjs(
        &self,
        file: FileId,
    ) -> bool {
        self.inputs
            .bindings
            .reference_file_is_external_or_commonjs(file)
    }

    /// Product member normalization asks the native resolved own member or
    /// export table after raw symbol selection, without inherited members or
    /// value-type acquisition. Inline-union substitution is a later question.
    pub(in crate::project::semantic) fn combined_reference_symbol(
        &mut self,
        symbol: ReferenceSymbol,
        site: &ReferenceSite,
    ) -> Result<ReferenceSymbol, DemandError> {
        let Some(parent) = self.reference_symbol_parent(symbol, &site.selection)? else {
            return Ok(symbol);
        };
        if !self.reference_parent_is_class_or_interface(parent)? {
            return Ok(symbol);
        }
        let mut is_static = false;
        for declaration in self.reference_symbol_declarations(symbol, &site.selection)? {
            if let ReferenceDeclaration::Captured(declaration) = declaration
                && self.source.declaration_has_modifier(
                    declaration,
                    crate::syntax_kind::SyntaxKind::StaticKeyword,
                )?
            {
                is_static = true;
                break;
            }
        }
        let name = self.reference_symbol_key(symbol, &site.selection)?;
        Ok(self
            .resolved_reference_parent_member(parent, is_static, &name, &site.selection)?
            .unwrap_or(symbol))
    }

    /// `getMembersOfSymbol` / `getExportsOfSymbol` own-table lookup, including
    /// the table's late-name prerequisite. This never searches base types.
    pub(super) fn resolved_reference_parent_member(
        &mut self,
        parent: SymbolParent,
        exports: bool,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let parent = match parent {
            SymbolParent::Bound(parent) => self
                .source
                .bindings()
                .raw_parent_symbol(parent)
                .map(ReferenceSymbol::from)
                .map_err(|error| self.generic_binding_failure(error, consumer))?,
            SymbolParent::Created(parent) => ReferenceSymbol::from_property(parent),
        };
        let table = if exports {
            self.reference_exports_of_symbol(parent, consumer)?
        } else {
            self.reference_members_of_symbol(parent, consumer)?
        };
        Ok(table.and_then(|table| {
            table
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, symbol)| *symbol)
        }))
    }

    pub(super) fn record_access_symbol(&mut self, root: ExpressionRoot, symbol: PropertySymbolId) {
        self.reference_symbols.accesses.insert(root, symbol);
    }

    /// `GetSymbolAtLocation` uses the same checking prelude and checker state
    /// as all subsequent candidates. Actual nil is distinct from a failed
    /// prerequisite and from an unavailable native symbol producer.
    pub(in crate::project::semantic) fn reference_symbol_at_site(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let (root, role) = match site.target {
            ReferenceSiteTarget::Captured { root, role } => (root, role),
            ReferenceSiteTarget::Json {
                property, object, ..
            } => {
                let symbol = self
                    .source
                    .bindings()
                    .json_node_symbol(object.file, binding::JsonDeclaration::Property(property))
                    .map_err(|error| self.generic_binding_failure(error, &site.selection))?;
                return Ok(Some(ReferenceSymbol::from(symbol)));
            }
        };
        // Selection setup must run before the raw question's cache lookup:
        // an earlier checker query cannot satisfy or poison this prelude.
        let prelude = self
            .source
            .enclosing_calls(root.file, root.site.span.start)?;
        self.check_site_prelude(&prelude)?;
        if role == binding::ReferenceTokenRole::ParameterPropertyModifier {
            let declaration = self.source.selected_parameter_property_declaration(root)?;
            return self.reference_symbol_of_declaration(declaration).map(Some);
        }
        // A JSX attribute's own symbol belongs to its JsxAttributes object;
        // it relates to any other member only through the contextual props
        // type (`getPropertySymbolsFromContextualType`), which is deferred
        // JSX attribute typing.
        if role == binding::ReferenceTokenRole::JsxAttribute {
            return Err(super::super::unsupported_demand_at_location(
                self.inputs,
                &site.selection,
                crate::analysis::SemanticFailureReason::JsxTypingRequired,
                "a JSX attribute relates to its props member only through deferred JSX attribute typing",
            ));
        }
        self.reference_symbol_at_node(root, &site.selection)
    }

    /// Native `getSymbolAtLocation` has no whole-question cache. Each branch
    /// uses its own declaration/entity/access owner and replays that owner's
    /// completion evidence; a genuine nil is independent of a failed query.
    pub(in crate::project::semantic) fn reference_symbol_at_node_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.reference_symbol_recipe(root)?;
            // getSymbolOfNameOrPropertyAccessExpression first probes all
            // meanings for an export assignment. Only nil/unknown falls
            // through to its ordinary expression interpretation.
            if let ReferenceSymbolRecipe::Identifier {
                root,
                export_assignment: true,
            }
            | ReferenceSymbolRecipe::Access {
                root,
                export_assignment: true,
            } = &recipe
            {
                let anchor = self.source.reference_entity_name_anchor(*root)?;
                let (symbol, _) = sequence.read(self.reference_entity_at_completion(
                    root.file,
                    anchor,
                    false,
                    binding::NativeLookupMeaning::ALL.with_alias(),
                    false,
                    consumer,
                ))?;
                if let Some(symbol) =
                    symbol.filter(|symbol| !self.is_unknown_reference_symbol(*symbol))
                {
                    return Ok(Some(symbol));
                }
            }
            match recipe {
                ReferenceSymbolRecipe::Declaration(declaration) => sequence
                    .read(self.reference_symbol_of_declaration_completion(declaration))
                    .map(Some),
                ReferenceSymbolRecipe::Node(declaration) => {
                    sequence.read(self.reference_node_declaration_symbol_completion(declaration))
                }
                ReferenceSymbolRecipe::RawNode {
                    declaration,
                    required,
                } => {
                    let symbol = self.reference_raw_symbol_of_declaration(declaration)?;
                    if required && symbol.is_none() {
                        return Err(super::super::internal(
                            "required symbol-at-location raw node attachment is absent",
                        )
                        .into());
                    }
                    Ok(symbol)
                }
                ReferenceSymbolRecipe::Entity {
                    root,
                    meaning,
                    missing: disposition,
                } => {
                    let anchor = self.source.reference_entity_name_anchor(root)?;
                    let dont_resolve_alias = !matches!(disposition, EntitySymbolMissing::Quiet);
                    let (symbol, missing) = sequence.read(self.reference_entity_at_completion(
                        root.file,
                        anchor,
                        false,
                        meaning,
                        dont_resolve_alias,
                        consumer,
                    ))?;
                    match disposition {
                        EntitySymbolMissing::Unresolved => match symbol
                            .filter(|symbol| !self.is_unknown_reference_symbol(*symbol))
                        {
                            Some(symbol) => Ok(Some(symbol)),
                            None => self.unresolved_symbol_for_entity_name(root).map(Some),
                        },
                        EntitySymbolMissing::Report if symbol.is_none() => {
                            let missing = missing.ok_or_else(|| {
                                super::super::internal(
                                    "failed import-equals entity has no diagnostic component",
                                )
                            })?;
                            sequence.read(NativeCompletion::recovered(None, missing))
                        }
                        EntitySymbolMissing::Quiet | EntitySymbolMissing::Report => Ok(symbol),
                    }
                }
                ReferenceSymbolRecipe::Private { root, description } => {
                    if let Some(symbol) = self.reference_symbols.identifiers.get(&root) {
                        return sequence.read(Ok(symbol.clone())).map(Some);
                    }
                    let declaration = self.source.declaration_from_expression_root(root)?;
                    let symbol = self.lookup_private_identifier_declaration(
                        &description,
                        declaration,
                        consumer,
                    )?;
                    // getSymbolForPrivateIdentifierExpression installs only
                    // a non-nil raw symbol; native nil is retried next time.
                    if let Some(symbol) = symbol {
                        self.reference_symbols
                            .identifiers
                            .insert(root, NativeCompletion::clean(symbol));
                    }
                    Ok(symbol)
                }
                ReferenceSymbolRecipe::ImmediateAlias(declaration) => {
                    let symbol = sequence
                        .read(self.reference_symbol_of_declaration_completion(declaration))?;
                    sequence
                        .read(self.reference_immediate_aliased_symbol_completion(symbol, consumer))
                }
                ReferenceSymbolRecipe::Identifier { root, .. } => {
                    let anchor = self.source.reference_entity_name_anchor(root)?;
                    let (symbol, _) = sequence.read(self.reference_entity_at_completion(
                        root.file,
                        anchor,
                        false,
                        binding::NativeLookupMeaning::VALUE,
                        true,
                        consumer,
                    ))?;
                    Ok(symbol)
                }
                ReferenceSymbolRecipe::Access { root: access, .. } => {
                    if let Some(&symbol) = self.reference_symbols.accesses.get(&access) {
                        return Ok(Some(ReferenceSymbol(Symbol::Property(symbol))));
                    }
                    sequence.read(self.check_access_expression_completion(
                        access,
                        super::contextual::CheckMode::NORMAL,
                        false,
                    ))?;
                    match self.reference_symbols.accesses.get(&access) {
                        Some(&symbol) => Ok(Some(ReferenceSymbol(Symbol::Property(symbol)))),
                        None => {
                            // Native next asks getApplicableIndexSymbol for a
                            // dot access; the index-symbol producer must supply
                            // its own identity, never a guessed property name.
                            let recipe = self.source.access_recipe(access)?;
                            let super::super::source::AccessKey::Name {
                                private: false,
                                declaration,
                                ..
                            } = recipe.key
                            else {
                                return Ok(None);
                            };
                            let receiver =
                                sequence.read(self.check_expression_cached_ex_completion(
                                    &recipe.receiver,
                                    super::contextual::CheckMode::NORMAL,
                                ))?;
                            let name = self.source.declaration_expression_root(declaration)?;
                            let key = sequence.read(
                                self.literal_type_of_property_name_completion(name.file, name.site),
                            )?;
                            let symbol = sequence.read(
                                self.applicable_index_symbol_completion(receiver, key, consumer),
                            )?;
                            if let Some(symbol) = symbol {
                                self.record_access_symbol(access, symbol);
                            }
                            Ok(symbol.map(ReferenceSymbol::from_property))
                        }
                    }
                }
                ReferenceSymbolRecipe::ExpressionProperty { object, name } => {
                    let ty = sequence.read(self.check_expression_cached_ex_completion(
                        &object,
                        super::contextual::CheckMode::NORMAL,
                    ))?;
                    sequence.read(self.reference_property_symbol_completion(ty, &name, consumer))
                }
                ReferenceSymbolRecipe::BindingProperty { pattern, name } => {
                    let ty = sequence.read(self.type_of_node_completion(pattern))?;
                    sequence.read(self.reference_property_symbol_completion(ty, &name, consumer))
                }
                ReferenceSymbolRecipe::TypeProperty { object, name } => {
                    let ty = sequence.read(self.check_type_completion(&object))?;
                    sequence.read(self.reference_property_symbol_completion(ty, &name, consumer))
                }
                ReferenceSymbolRecipe::Absent => Ok(None),
                ReferenceSymbolRecipe::Pending(message) => {
                    Err(self.access_pending(consumer, message))
                }
            }
        })();
        sequence.finish(result)
    }

    pub(super) fn reference_symbol_at_node(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.reference_symbol_at_node_completion(root, consumer)?
            .admit()
    }

    pub(super) fn reference_property_symbol_completion(
        &mut self,
        ty: super::super::model::TypeId,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        self.reference_property_symbol_by_key_completion(
            ty,
            &NativePropertyKey::fixed(name.as_bytes()),
            consumer,
        )
    }

    pub(super) fn reference_property_symbol_by_key_completion(
        &mut self,
        ty: super::super::model::TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            sequence
                .read(self.property_of_type_by_key_completion(ty, name, consumer))?
                .map(|property| {
                    self.property_symbol(&property, consumer)
                        .map(|symbol| ReferenceSymbol(Symbol::Property(symbol)))
                })
                .transpose()
        })();
        sequence.finish(result)
    }

    pub(in crate::project::semantic) fn reference_symbol_identity(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<NativeSymbolIdentity, DemandError> {
        self.reference_symbol_identity_at(symbol, Some(consumer))
    }

    pub(super) fn reference_symbol_identity_at(
        &self,
        symbol: ReferenceSymbol,
        consumer: Option<&Location>,
    ) -> Result<NativeSymbolIdentity, DemandError> {
        let symbol = self.reference_current_header(symbol);
        match symbol.0 {
            Symbol::Bound(origin) => self
                .source
                .bindings()
                .reference_symbol_identity(origin)
                .map_err(|error| super::super::binding_failure(self.inputs, consumer, error)),
            Symbol::Property(property) => Ok(self
                .property_native_symbol_at(property, consumer)?
                .identity()),
        }
    }

    pub(in crate::project::semantic) fn reference_symbol_name(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Vec<u8>, DemandError> {
        let symbol = self.reference_current_header(symbol);
        match symbol.0 {
            Symbol::Bound(origin) => self
                .source
                .bindings()
                .read_origin_name(origin)
                .map_err(|error| self.generic_binding_failure(error, consumer)),
            Symbol::Property(property) => self
                .property_native_symbol(property, consumer)?
                .name()
                .map_err(|failure| self.generic_binding_failure(failure, consumer)),
        }
    }

    pub(in crate::project::semantic) fn reference_symbol_flags(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<ReferenceSymbolFlags, DemandError> {
        let symbol = self.reference_current_header(symbol);
        Ok(match symbol.0 {
            Symbol::Bound(origin) => {
                let bound = origin.symbol();
                let flags = self
                    .source
                    .bindings()
                    .native_property_flags(bound)
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                let types = self
                    .source
                    .bindings()
                    .type_symbol_flags(bound)
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                ReferenceSymbolFlags {
                    native: self.reference_symbol_native_flags(symbol, consumer)?,
                    property: flags.property,
                    accessor: flags.getter || flags.setter,
                    method: flags.method,
                    value: types.value,
                    type_: types.type_,
                    namespace: types.namespace,
                    type_or_namespace: types.type_or_namespace,
                    function: types.function,
                    value_module: types.value_module,
                    module_exports: types.module_exports,
                    variable: flags.variable,
                    enum_: types.enum_,
                    regular_enum: types.regular_enum,
                    const_enum: types.const_enum,
                    enum_member: types.enum_member,
                    type_parameter: types.type_parameter,
                    type_literal: types.type_literal,
                    signature: types.signature,
                    object_literal: types.object_literal,
                    function_scoped_variable: types.function_scoped_variable,
                    block_scoped_variable: types.block_scoped_variable,
                    synthetic: false,
                    transient: self.source.bindings().native_symbol_is_transient(bound),
                    alias: types.alias,
                    assignment: self
                        .source
                        .bindings()
                        .native_symbol_has_assignment_flag(bound),
                    export_value: types.export_value,
                    class: types.class,
                    interface: types.interface,
                    module: types.module,
                }
            }
            Symbol::Property(property) => {
                let flags = self.property_symbol_flags(property)?;
                ReferenceSymbolFlags {
                    native: self.reference_symbol_native_flags(symbol, consumer)?,
                    property: flags.property,
                    accessor: flags.get_accessor || flags.set_accessor,
                    method: flags.method,
                    value: flags.types.value
                        || flags.property
                        || flags.variable
                        || flags.method
                        || flags.get_accessor
                        || flags.set_accessor,
                    type_: flags.types.type_,
                    namespace: flags.types.namespace,
                    type_or_namespace: flags.types.type_or_namespace,
                    function: flags.types.function,
                    value_module: flags.types.value_module,
                    module_exports: flags.types.module_exports,
                    variable: flags.variable,
                    enum_: flags.types.enum_,
                    regular_enum: flags.types.regular_enum,
                    const_enum: flags.types.const_enum,
                    enum_member: flags.types.enum_member,
                    type_parameter: flags.types.type_parameter,
                    type_literal: flags.types.type_literal,
                    signature: flags.types.signature,
                    object_literal: flags.types.object_literal,
                    function_scoped_variable: flags.types.function_scoped_variable,
                    block_scoped_variable: flags.types.block_scoped_variable,
                    synthetic: self.property_check_flags(property)?
                        & super::property_symbols::checks::SYNTHETIC
                        != 0,
                    transient: self.property_is_transient(property)?,
                    alias: flags.types.alias,
                    assignment: self
                        .reference_symbol_native_flags(symbol, consumer)?
                        .assignment,
                    export_value: flags.types.export_value,
                    class: flags.types.class,
                    interface: flags.types.interface,
                    module: flags.types.module,
                }
            }
        })
    }

    /// `getDeclarationOfAliasSymbol` keeps FindLast alias selection separate
    /// from the complete symbol.Declarations observation used by diagnostics.
    pub(super) fn reference_alias_declaration(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceDeclaration>, DemandError> {
        let declarations = self.reference_symbol_declarations(symbol, consumer)?;
        for declaration in declarations.into_iter().rev() {
            if let ReferenceDeclaration::Captured(captured) = declaration
                && self.source.is_alias_symbol_declaration(captured)?
            {
                return Ok(Some(declaration));
            }
        }
        Ok(None)
    }

    pub(super) fn native_reference_declaration(
        &mut self,
        declaration: binding::NativeSymbolDeclaration,
    ) -> Result<ReferenceDeclaration, DemandError> {
        match declaration.json {
            Some(json) => Ok(ReferenceDeclaration::Json {
                file: declaration.file,
                declaration: json,
            }),
            None => self
                .source
                .reference_declaration_identity(declaration)
                .map(ReferenceDeclaration::Captured),
        }
    }

    /// Native `symbol.Declarations != nil` observes the retained header only;
    /// it does not attach or parse any declaration syntax.
    pub(super) fn reference_symbol_has_declarations(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        match symbol.0 {
            Symbol::Property(property) => self.property_has_declarations(property, consumer),
            Symbol::Bound(origin) => self
                .source
                .bindings()
                .reference_symbol_declarations(binding::ReferenceSymbol::Known(origin))
                .map(|declarations| !declarations.is_empty())
                .map_err(|error| self.generic_binding_failure(error, consumer)),
        }
    }

    pub(in crate::project::semantic) fn reference_symbol_declarations(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Vec<ReferenceDeclaration>, DemandError> {
        let symbol = self.reference_current_header(symbol);
        match symbol.0 {
            Symbol::Property(property) => self.property_reference_declarations(property, consumer),
            Symbol::Bound(origin) => {
                let declarations = self
                    .source
                    .bindings()
                    .reference_symbol_declarations(binding::ReferenceSymbol::Known(origin))
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;

                declarations
                    .into_iter()
                    .map(|declaration| self.native_reference_declaration(declaration))
                    .collect::<Result<Vec<_>, _>>()
            }
        }
    }

    /// `SymbolName` / `Symbol.Declarations` feed the common Q3/Q4 named
    /// projection. Product output orders and deduplicates the established facts;
    /// this read neither resolves aliases nor acquires a value type.
    pub(in crate::project::semantic) fn reference_symbol_named_declarations(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Vec<crate::references::NamedLocation>, DemandError> {
        let name = self.reference_symbol_display_name(symbol, consumer)?;

        let mut declarations = Vec::new();
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            let location = self.reference_declaration_location(declaration)?;
            let name = { crate::references::metadata::copy_text(&name) };
            {
                declarations.push(crate::references::NamedLocation { name, location });
            }
        }
        {
            declarations.sort_by(|a, b| {
                crate::references::metadata::location_order(&a.location, &b.location)
                    .then_with(|| crate::references::metadata::text_order(&a.name, &b.name))
            });
            declarations.dedup_by(|a, b| {
                crate::references::metadata::text_order(&a.name, &b.name).is_eq()
                    && crate::references::metadata::location_order(&a.location, &b.location).is_eq()
            });
        }
        Ok(declarations)
    }

    /// Public metadata for an exact native identity. `Symbol.Declarations`
    /// supplies origins; reading them neither acquires a type nor observes
    /// identity order. Distinct identities may have identical origin lists.
    pub(in crate::project::semantic) fn symbol_identity_origins(
        &mut self,
        identity: NativeSymbolIdentity,
        consumer: &Location,
    ) -> Result<Vec<Location>, DemandError> {
        let symbol = match identity {
            NativeSymbolIdentity::Bound(header) => {
                ReferenceSymbol::from_origin(self.source.bindings().read_origin(header))
            }
            NativeSymbolIdentity::Transient(_) => {
                ReferenceSymbol::from_property(self.property_symbol_for_identity(identity)?)
            }
            NativeSymbolIdentity::EmptyTypeLiteral => return Ok(Vec::new()),
        };
        self.reference_symbol_origins(symbol, consumer)
    }

    /// The presentation projection of `Symbol.Declarations` preserves native
    /// declaration order, removing repeated locations only within this symbol.
    pub(super) fn reference_symbol_origins(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Vec<Location>, DemandError> {
        let mut origins = Vec::new();
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            let location = self.reference_declaration_location(declaration)?;
            if !origins
                .iter()
                .any(|previous| same_origin_location(previous, &location))
            {
                origins.push(location);
            }
        }
        Ok(origins)
    }

    /// `FirstOrNil(symbol.Declarations)` projects only the selected native
    /// declaration. Later merged declarations do not trigger syntax capture.
    pub(super) fn reference_symbol_first_declaration(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceDeclaration>, DemandError> {
        match symbol.0 {
            Symbol::Property(property) => Ok(self
                .property_reference_declarations(property, consumer)?
                .first()
                .copied()),
            Symbol::Bound(origin) => self
                .source
                .bindings()
                .reference_symbol_declarations(binding::ReferenceSymbol::Known(origin))
                .map_err(|error| self.generic_binding_failure(error, consumer))?
                .first()
                .copied()
                .map(|declaration| self.native_reference_declaration(declaration))
                .transpose(),
        }
    }

    pub(in crate::project::semantic) fn reference_symbol_value_declaration(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceDeclaration>, DemandError> {
        let symbol = self.reference_current_header(symbol);
        match symbol.0 {
            Symbol::Bound(origin) => self
                .source
                .bindings()
                .symbol_value_declaration(origin.symbol())
                .map_err(|error| self.generic_binding_failure(error, consumer))?
                .map(|declaration| self.native_reference_declaration(declaration))
                .transpose(),
            Symbol::Property(property) => {
                self.property_reference_value_declaration(property, consumer)
            }
        }
    }

    pub(super) fn reference_declaration_ordering(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<binding::NativeDeclaration, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.declaration_ordering(declaration)
            }
            ReferenceDeclaration::Json { file, declaration } => {
                let facts = self.inputs.bindings.json_facts(binding::BoundJson {
                    file,
                    value: binding::JsonFacts::ROOT,
                });
                let (_, span) = facts.declaration_facts(
                    declaration,
                    crate::Span {
                        start: 0,
                        end: self.inputs.sources.get(file).text.as_str().len(),
                    },
                );
                Ok(binding::NativeDeclaration { file, span })
            }
        }
    }

    /// Actual declaration.Kind, including captured JSON nodes. Syntax-family
    /// predicates do not reconstruct a JSON declaration's kind from spelling.
    pub(super) fn reference_declaration_kind(
        &mut self,
        declaration: ReferenceDeclaration,
    ) -> Result<crate::syntax_kind::SyntaxKind, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => {
                self.source.declaration_kind(declaration)
            }
            ReferenceDeclaration::Json { file, declaration } => {
                let facts = self.inputs.bindings.json_facts(binding::BoundJson {
                    file,
                    value: binding::JsonFacts::ROOT,
                });
                Ok(facts
                    .declaration_facts(
                        declaration,
                        crate::Span {
                            start: 0,
                            end: self.inputs.sources.get(file).text.as_str().len(),
                        },
                    )
                    .0)
            }
        }
    }
    pub(super) fn reference_declaration_symbol_parent(
        &mut self,
        declaration: ReferenceDeclaration,
        consumer: &Location,
    ) -> Result<Option<SymbolParent>, DemandError> {
        match declaration {
            ReferenceDeclaration::Captured(declaration) => self
                .source
                .declaration_symbol_parent(declaration)
                .map(|parent| parent.map(SymbolParent::Bound)),
            ReferenceDeclaration::Json { file, declaration } => {
                let symbol = self
                    .source
                    .bindings()
                    .json_node_symbol(file, declaration)
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                Ok(self
                    .source
                    .bindings()
                    .property_parent(symbol)
                    .map(SymbolParent::Bound))
            }
        }
    }

    pub(in crate::project::semantic) fn reference_symbol_parent(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<SymbolParent>, DemandError> {
        let symbol = self.reference_current_header(symbol);
        match symbol.0 {
            Symbol::Bound(origin) => self
                .source
                .bindings()
                .read_origin_parent(origin)
                .map(|parent| parent.map(SymbolParent::Bound))
                .map_err(|error| self.generic_binding_failure(error, consumer)),
            Symbol::Property(property) => self.property_parent(property),
        }
    }

    /// `GetRootSymbols` recursively flattens the native immediate-root vector.
    /// Empty means this symbol itself; order and duplicates are preserved.
    pub(in crate::project::semantic) fn reference_root_symbols(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Vec<ReferenceSymbol>, DemandError> {
        let mut result = Vec::new();
        let mut pending = vec![symbol];
        while let Some(symbol) = pending.pop() {
            let roots = match symbol.0 {
                Symbol::Bound(_) => Vec::new(),
                Symbol::Property(property) => {
                    self.property_immediate_root_symbols(property, consumer)?
                }
            };
            if roots.is_empty() {
                result.push(symbol);
            } else {
                pending.extend(
                    roots
                        .into_iter()
                        .rev()
                        .map(|property| ReferenceSymbol(Symbol::Property(property))),
                );
            }
        }
        Ok(result)
    }
}

impl Evaluator<'_, '_> {
    pub(super) fn reference_property_symbol(
        &mut self,
        ty: super::super::model::TypeId,
        name: &str,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.reference_property_symbol_completion(ty, name, consumer)?
            .admit()
    }
}

impl Evaluator<'_, '_> {
    pub(in crate::project::semantic) fn reference_immediate_aliased_symbol(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.reference_immediate_aliased_symbol_completion(symbol, consumer)?
            .admit()
    }
}

impl Evaluator<'_, '_> {
    pub(super) fn reference_as_parent(
        &self,
        symbol: ReferenceSymbol,
    ) -> Result<SymbolParent, DemandError> {
        if let Some(origin) = self.reference_raw_origin(symbol)? {
            return Ok(self
                .source
                .bindings()
                .checker_symbol_parent(origin.symbol())
                .into());
        }
        symbol
            .property()
            .map(SymbolParent::Created)
            .ok_or_else(|| super::super::internal("parent has no actual header").into())
    }
    /// `Find(Symbol.Declarations, IsSourceFile)` differs from GetSourceFileOfModule's ValueDeclaration-first question.
    pub(super) fn reference_module_source_file(
        &self,
        symbol: ReferenceSymbol,
    ) -> Result<Option<FileId>, DemandError> {
        for declaration in self.reference_header_declarations(symbol, None)? {
            if self.header_declaration_kind(declaration)?
                == crate::syntax_kind::SyntaxKind::SourceFile
            {
                return Ok(Some(match declaration {
                    super::declarations::DeclarationAssociation::Raw(raw) => {
                        self.source
                            .bindings()
                            .raw_declaration_facts(raw)
                            .map_err(|failure| {
                                super::super::binding_failure(self.inputs, None, failure)
                            })?
                            .file
                    }
                    super::declarations::DeclarationAssociation::Attached(
                        ReferenceDeclaration::Captured(node),
                    ) => node.file,
                    super::declarations::DeclarationAssociation::Attached(
                        ReferenceDeclaration::Json { file, .. },
                    ) => file,
                }));
            }
        }
        Ok(None)
    }
    pub(super) fn reference_module_is_shorthand(
        &self,
        symbol: ReferenceSymbol,
    ) -> Result<bool, DemandError> {
        Ok(
            match self.reference_header_value_declaration(symbol, None)? {
                Some(super::declarations::DeclarationAssociation::Raw(raw)) => {
                    self.source.bindings().raw_declaration_is_shorthand(raw)
                }
                Some(super::declarations::DeclarationAssociation::Attached(
                    ReferenceDeclaration::Captured(node),
                )) => self.source.observed_declaration_is_shorthand(node)?,
                Some(super::declarations::DeclarationAssociation::Attached(
                    ReferenceDeclaration::Json { .. },
                ))
                | None => false,
            },
        )
    }
}

/// Exact presentation-origin identity, preserving the existing Location
/// equality. Distinct file names never merge merely because coordinates match.
/// Range comparison is bounded; variable file bytes use the shared owner.
fn same_origin_location(left: &Location, right: &Location) -> bool {
    if left.start != right.start || left.end != right.end {
        return false;
    }

    left.file.as_bytes() == right.file.as_bytes()
}
