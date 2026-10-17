//! Checker-owned resolved tables, late declaration links and merge redirects.
//! Raw tables and module API exports remain separate native observations.
use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, PropertySymbolId, TypeId,
};
use super::super::source::DeclarationIdentity;
use super::reference_symbols::{ReferenceRawSymbolTable, ReferenceSymbolTable};
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::project::binding::{NativePropertyKey, NativeSymbolIdentity};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;
mod entity;
mod indexes;

pub(super) struct SymbolTables {
    pub(super) raw: Vec<crate::project::binding::NativeSymbolTable<ReferenceSymbol>>,
    pub(super) raw_overlays:
        crate::resources::FixedMap<crate::project::binding::NativeRawSymbolTable, usize>,
    pub(super) merged: crate::resources::FixedMap<NativeSymbolIdentity, ReferenceSymbol>,
    pub(super) projections:
        crate::resources::FixedMap<NativeSymbolIdentity, crate::project::binding::SymbolId>,
    pub(super) headers: crate::resources::FixedMap<NativeSymbolIdentity, PropertySymbolId>,
    resolved: crate::resources::FixedMap<
        (NativeSymbolIdentity, bool),
        NativeCompletion<ReferenceSymbolTable>,
    >,
    declaration_symbols:
        crate::resources::FixedMap<DeclarationIdentity, NativeCompletion<ReferenceSymbol>>,
    /// `lateBindMember` / `getLateBoundSymbol`: raw header identity to its resolved late symbol.
    late_symbols: crate::resources::FixedMap<NativeSymbolIdentity, ReferenceSymbol>,
}

/// `lateBindMember` / `lateBindIndexSignature` build one ordered late table.
/// Key payloads are immutable shared values; only the entry array is copied
/// when publishing the final table. Nil-versus-empty selection stays at the caller.
struct LateSymbolTable {
    entries: Vec<(NativePropertyKey, ReferenceSymbol)>,
    control: Option<crate::resources::Control>,
}

impl LateSymbolTable {
    fn new(control: Option<crate::resources::Control>) -> Self {
        Self {
            entries: Vec::new(),
            control,
        }
    }

    fn lookup(&self, key: &NativePropertyKey) -> Option<ReferenceSymbol> {
        lookup(Some(&self.entries), key, self.control.as_ref())
    }

    fn insert(&mut self, key: NativePropertyKey, symbol: ReferenceSymbol) {
        self.entries.push((key, symbol));
    }

    fn into_shared(self) -> ReferenceSymbolTable {
        std::sync::Arc::from(self.entries)
    }
}

/// Native `SymbolTable[name]` probes of an already-observed table, shared by
/// late binding, index acquisition and entity lookup. Exact name comparison
/// remains with NativeStringValue, including opaque atoms.
fn lookup(
    table: Option<&[(NativePropertyKey, ReferenceSymbol)]>,
    key: &NativePropertyKey,
    _control: Option<&crate::resources::Control>,
) -> Option<ReferenceSymbol> {
    table?.iter().find_map(|(name, symbol)| {
        name.string_value()
            .canonical_equal(key.string_value())
            .then_some(*symbol)
    })
}

impl Evaluator<'_, '_> {
    /// `getDeclaredTypeOfClassOrInterface` syntax provenance for the actual
    /// header's complete declaration list. The retained carrier is never its
    /// native type, table, cache or merge identity.
    pub(super) fn class_or_interface_declaration_recipe(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<crate::project::binding::BoundClassOrInterface, DemandError> {
        use crate::project::binding::{BoundClassKind, BoundClassOrInterface};
        let flags = self.reference_symbol_flags(symbol, consumer)?;
        let kind = if flags.class {
            BoundClassKind::Class
        } else if flags.interface {
            BoundClassKind::Interface
        } else {
            return Err(super::super::internal(
                "class/interface recipe requested for another native family",
            )
            .into());
        };
        let mut declarations = Vec::new();
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            let ReferenceDeclaration::Captured(declaration) = declaration else {
                continue;
            };
            let declaration_kind = self.source.declaration_kind(declaration)?;
            if !matches!(
                declaration_kind,
                K::ClassDeclaration | K::ClassExpression | K::InterfaceDeclaration
            ) {
                continue;
            }
            let raw = self
                .source
                .raw_node_symbol_origin(declaration)?
                .ok_or_else(|| {
                    super::super::internal(
                        "class/interface declaration has no retained syntax association",
                    )
                })?
                .symbol();
            let candidates = self
                .source
                .bindings()
                .class_or_interface(raw)
                .map_err(|error| self.generic_binding_failure(error, consumer))?;
            let mut found = None;
            for candidate in candidates.declarations {
                if self.source.declaration_identity(candidate)? == declaration {
                    found = Some(candidate);
                    break;
                }
            }
            declarations.push(found.ok_or_else(|| {
                super::super::internal(
                    "actual class/interface declaration lacks its exact syntax recipe",
                )
            })?);
        }
        if declarations.is_empty() {
            return Err(super::super::internal("class/interface has no own declaration").into());
        }
        Ok(BoundClassOrInterface {
            kind,
            has_interface: flags.interface,
            declarations,
        })
    }

    /// `GetClassLikeDeclarationOfSymbol` selects the first actual class node;
    /// syntax provenance never replaces the header used for semantic caches.
    pub(super) fn reference_class_recipe(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<super::super::source::ClassRecipe, DemandError> {
        let declarations = self
            .reference_symbol_declarations(symbol, consumer)?
            .into_iter()
            .filter_map(|declaration| match declaration {
                ReferenceDeclaration::Captured(declaration) => Some(
                    super::super::source::DeclarationReference::Captured(declaration),
                ),
                ReferenceDeclaration::Json { .. } => None,
            });
        let declaration = self
            .source
            .first_class_declaration(declarations)?
            .ok_or_else(|| super::super::internal("class symbol has no class declaration"))?;
        let declaration = self.source.resolve_declaration_reference(declaration)?;
        self.source.class_recipe_of_declaration(declaration)
    }

    /// `getResolvedMembersOrExportsOfSymbol` publishes earlySymbols before
    /// checking names. Nil stays uninstalled; a present empty table is visible
    /// to reentry. The final completion retains every consumed recovery.
    pub(super) fn resolved_symbol_table_completion(
        &mut self,
        symbol: ReferenceSymbol,
        exports: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbolTable>> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        if let Some(table) = self.symbol_tables.resolved.get(&(identity, exports)) {
            return Ok(table.clone().map(Some));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let early = if exports && self.reference_symbol_flags(symbol, consumer)?.module {
                sequence
                    .read(self.reference_module_exports_worker_completion(symbol, true, consumer))?
            } else {
                self.reference_raw_symbol_table(symbol, exports)?
                    .map(|raw| self.reference_raw_table_entries(raw, consumer))
                    .transpose()?
            };
            if let Some(table) = &early {
                self.symbol_tables
                    .resolved
                    .insert((identity, exports), NativeCompletion::clean(table.clone()));
            }
            let mut late = None;
            for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                let ReferenceDeclaration::Captured(declaration) = declaration else {
                    continue;
                };
                for member in self.source.members_of_declaration(declaration)? {
                    if exports
                        != self
                            .source
                            .declaration_has_modifier(member, K::StaticKeyword)?
                    {
                        continue;
                    }
                    let name = self.source.pseudo_child(member, "name")?;
                    let named = match name {
                        Some(name) => sequence.read(self.is_late_bindable_name_completion(name))?,
                        None => false,
                    };
                    if named {
                        sequence.read(self.late_bind_member_completion(
                            symbol,
                            early.as_ref(),
                            late.get_or_insert_with(|| LateSymbolTable::new(self.control.clone())),
                            member,
                            consumer,
                        ))?;
                    } else if sequence
                        .read(self.has_late_bindable_index_signature_completion(member, consumer))?
                    {
                        self.late_bind_index_signature(
                            early.as_ref(),
                            late.get_or_insert_with(|| LateSymbolTable::new(self.control.clone())),
                            member,
                            consumer,
                        )?;
                    }
                }
            }
            if exports
                && let Some(raw) = self.reference_raw_symbol_table(symbol, true)?
                && self
                    .reference_raw_table_lookup(
                        raw,
                        &NativePropertyKey::fixed(
                            b"\xfeassignment".as_slice(),
                            self.control.as_ref(),
                        ),
                        consumer,
                    )?
                    .is_some()
            {
                return Err(self.access_pending(
                    consumer,
                    "native expando assignment declarations are outside the admitted type profile",
                ));
            }
            sequence.read(self.combine_symbol_tables_completion(
                early,
                late.map(LateSymbolTable::into_shared),
                consumer,
            ))
        })();
        let completed = sequence.finish(result)?;
        if let Some(table) = completed.value() {
            self.symbol_tables.resolved.insert(
                (identity, exports),
                completed.clone().map(|_| table.clone()),
            );
        } else {
            self.symbol_tables.resolved.remove(&(identity, exports));
        }
        Ok(completed)
    }

    /// `lateBindMember`: install raw node.Symbol for recursive name checks,
    /// then create/merge a real late symbol. Conflicting declarations get their
    /// own fresh symbol while the table retains its first entry.
    fn late_bind_member_completion(
        &mut self,
        parent: ReferenceSymbol,
        early: Option<&ReferenceSymbolTable>,
        late: &mut LateSymbolTable,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<ReferenceSymbol> {
        if let Some(symbol) = self.symbol_tables.declaration_symbols.get(&declaration) {
            return Ok(symbol.clone());
        }
        let raw = self
            .reference_raw_symbol_of_declaration(declaration)?
            .ok_or_else(|| super::super::internal("late member has no raw node.Symbol"))?;
        self.symbol_tables
            .declaration_symbols
            .insert(declaration, NativeCompletion::clean(raw));
        let completed = self.late_declaration_name_type_completion(declaration)?.and_then_value(|ty| {
            let Some(key) = self.property_key_from_type(ty)? else { return Ok(raw); };
            let flags = self.reference_symbol_native_flags(raw, consumer)?;
            let entry = late.lookup(&key);
            let mut symbol = match entry {
                Some(symbol) => self.register_reference_symbol(symbol, consumer)?,
                None => {
                    let symbol = self.new_late_symbol(key.clone());
                    late.insert(key.clone(), ReferenceSymbol::from_property(symbol));
                    symbol
                }
            };
            let existing = self.reference_symbol_native_flags(ReferenceSymbol::from_property(symbol), consumer)?;
            if existing.conflicts_with(flags) {
                // Native reports duplicate identifiers using declaration-name
                // syntax; that diagnostic consumes no symbol value type.
                let _early_symbol = lookup(early.map(|table| &**table), &key, self.control.as_ref());
                self.set_merge_symbol_flags(symbol, existing.repair_conflicting_accessor(flags))?;
                symbol = self.new_late_symbol(key);
            }
            self.set_late_symbol_name_type(symbol, ty)?;
            let raw_id = self.reference_symbol_identity(raw, consumer)?;
            self.symbol_tables.late_symbols.insert(raw_id, ReferenceSymbol::from_property(symbol));
            let current = self.reference_symbol_native_flags(ReferenceSymbol::from_property(symbol), consumer)?;
            // The admitted TS producer has no ReplaceableByMethod declarations;
            // that flag belongs to the separate expando assignment producer.
            if flags.replaceable_by_method || current.replaceable_by_method {
                return Err(self.access_pending(consumer, "native replaceable expando member producer is outside the admitted type profile"));
            }
            self.set_merge_symbol_flags(symbol, current.union(flags))?;
            self.append_merge_declaration(symbol, ReferenceDeclaration::Captured(declaration))?;
            if flags.has_value() { self.set_merge_value_declaration(symbol, ReferenceDeclaration::Captured(declaration), consumer)?; }
            if self.reference_symbol_parent(ReferenceSymbol::from_property(symbol), consumer)?.is_none() {
                self.set_merge_symbol_parent(symbol, parent)?;
            }
            Ok(ReferenceSymbol::from_property(symbol))
        })?;
        self.symbol_tables
            .declaration_symbols
            .insert(declaration, completed.clone());
        Ok(completed)
    }

    /// The shared name acquisition in `lateBindMember` and
    /// `getIndexInfosOfIndexSymbol`, preserving cached-expression versus
    /// computed-property-name checking.
    pub(super) fn late_declaration_name_type_completion(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> NativeResult<TypeId> {
        let name = self
            .source
            .pseudo_child(declaration, "name")?
            .ok_or_else(|| super::super::internal("late declaration has no name"))?;
        let (expression, computed) = self
            .source
            .late_bindable_name_expression(name)?
            .ok_or_else(|| {
                super::super::internal("late declaration lost its eligible expression")
            })?;
        if computed {
            let root = self.source.pseudo_node_expression(name)?;
            self.check_computed_property_name_completion(name.file, root.site)
        } else {
            self.check_expression_cached_completion(expression.file, expression.site)
        }
    }

    /// `lateBindIndexSignature` clones an early index once and appends raw
    /// declarations. It deliberately does not write lateBoundLinks.lateSymbol.
    fn late_bind_index_signature(
        &mut self,
        early: Option<&ReferenceSymbolTable>,
        late: &mut LateSymbolTable,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        let key = NativePropertyKey::fixed(b"\xfeindex".as_slice(), self.control.as_ref());
        let symbol = match late.lookup(&key) {
            Some(symbol) => self.register_reference_symbol(symbol, consumer)?,
            None => {
                let symbol = match lookup(early.map(|table| &**table), &key, self.control.as_ref())
                {
                    Some(early) => self.clone_reference_symbol(early, consumer)?,
                    None => self.new_late_symbol(key.clone()),
                };
                self.mark_symbol_late(symbol)?;
                late.insert(key, ReferenceSymbol::from_property(symbol));
                symbol
            }
        };
        let raw = self
            .reference_raw_symbol_of_declaration(declaration)?
            .ok_or_else(|| super::super::internal("late index declaration has no symbol"))?;
        if self
            .property_reference_declarations(symbol, consumer)?
            .is_empty()
            || !self
                .reference_symbol_native_flags(raw, consumer)?
                .replaceable_by_method
        {
            self.append_merge_declaration(symbol, ReferenceDeclaration::Captured(declaration))?;
        }
        Ok(())
    }

    /// `getLateBoundSymbol`: only a computed ClassMember can force its
    /// parent's table. Early accessor declarations never do that work.
    pub(super) fn reference_late_symbol_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<ReferenceSymbol> {
        let property = self.register_reference_symbol(symbol, consumer)?;
        if !self.property_symbol_flags(property)?.class_member()
            || self.reference_symbol_key(symbol, consumer)?.fixed_bytes()
                != Some(b"\xfecomputed".as_slice())
        {
            return Ok(NativeCompletion::clean(symbol));
        }
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self.symbol_tables.late_symbols.contains_key(&identity) {
                let declarations = self.reference_symbol_declarations(symbol, consumer)?;
                let mut has_name = false;
                for &declaration in &declarations {
                    if let ReferenceDeclaration::Captured(declaration) = declaration
                        && let Some(name) = self.source.pseudo_child(declaration, "name")?
                        && sequence.read(self.is_late_bindable_name_completion(name))?
                    {
                        has_name = true;
                        break;
                    }
                }
                if has_name {
                    let parent = self
                        .reference_raw_parent_symbol(symbol, consumer)?
                        .ok_or_else(|| super::super::internal("computed member has no Parent"))?;
                    let parent = self.get_merged_reference_symbol(parent, consumer)?;
                    let mut exports = false;
                    for declaration in declarations {
                        if let ReferenceDeclaration::Captured(declaration) = declaration
                            && self
                                .source
                                .declaration_has_modifier(declaration, K::StaticKeyword)?
                        {
                            exports = true;
                            break;
                        }
                    }
                    if exports {
                        sequence
                            .read(self.reference_exports_of_symbol_completion(parent, consumer))?;
                    } else {
                        sequence
                            .read(self.reference_members_of_symbol_completion(parent, consumer))?;
                    }
                }
            }
            Ok(match self.symbol_tables.late_symbols.get(&identity) {
                Some(&resolved) => resolved,
                None => {
                    self.symbol_tables.late_symbols.insert(identity, symbol);
                    symbol
                }
            })
        })();
        sequence.finish(result)
    }

    /// `combineSymbolTables` preserves native len-zero selection, including
    /// the distinction between nil and a present empty table.
    fn combine_symbol_tables_completion(
        &mut self,
        first: Option<ReferenceSymbolTable>,
        second: Option<ReferenceSymbolTable>,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbolTable>> {
        if first.as_ref().is_none_or(|table| table.is_empty()) {
            return Ok(NativeCompletion::clean(second));
        }
        if second.as_ref().is_none_or(|table| table.is_empty()) {
            return Ok(NativeCompletion::clean(first));
        }
        let target = self.new_reference_raw_table();
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for table in [first, second].into_iter().flatten() {
                sequence.read(self.merge_reference_symbol_table_completion(
                    target, &table, false, None, consumer,
                ))?;
            }
            Ok(Some(self.reference_raw_table_entries(
                ReferenceRawSymbolTable::Created(target),
                consumer,
            )?))
        })();
        sequence.finish(result)
    }

    /// `mergeSymbolTable` publishes each completed entry in the existing
    /// target slot. Recursive alias lookup can observe that slot immediately.
    pub(super) fn merge_reference_symbol_table_completion(
        &mut self,
        target: usize,
        source: &ReferenceSymbolTable,
        unidirectional: bool,
        parent: Option<ReferenceSymbol>,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for (key, source) in source.iter() {
                let current = self.reference_raw_table_lookup(
                    ReferenceRawSymbolTable::Created(target),
                    key,
                    consumer,
                )?;
                let merged = if let Some(current) = current {
                    let merged = sequence.read(self.merge_reference_symbol_completion(
                        current,
                        *source,
                        unidirectional,
                        consumer,
                    ))?;
                    if let Some(parent) = parent
                        && self.reference_symbol_flags(merged, consumer)?.transient
                    {
                        let mutable = self.mutable_reference_symbol(merged, consumer)?;
                        self.set_merge_symbol_parent(mutable, parent)?;
                    }
                    self.reference_current_header(merged)
                } else {
                    self.get_merged_reference_symbol(*source, consumer)?
                };
                self.symbol_tables.raw[target].insert(Ok(key.clone()), merged);
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `mergeSymbol` uses one-step merge redirects, fresh clone identity for
    /// non-transient targets, and declaration-driven value semantics.
    pub(super) fn merge_reference_symbol_completion(
        &mut self,
        target: ReferenceSymbol,
        source: ReferenceSymbol,
        unidirectional: bool,
        consumer: &Location,
    ) -> NativeResult<ReferenceSymbol> {
        let target_flags = self.reference_symbol_native_flags(target, consumer)?;
        let source_flags = self.reference_symbol_native_flags(source, consumer)?;
        if target_flags.conflicts_with(source_flags)
            && !(target_flags.assignment || source_flags.assignment)
        {
            if target_flags.has_namespace_module() {
                let global =
                    self.reference_symbol_identity_at(self.global_this_reference_symbol(), None)?;
                if self.reference_symbol_identity(target, consumer)? == global {
                    return Ok(NativeCompletion::clean(target));
                }
                return self
                    .diagnostic_symbol_display_completion(target, consumer)
                    .map(|display| display.map(|_| target));
            }
            // reportMergeSymbolError constructs symbolToString(source) eagerly.
            return self
                .diagnostic_symbol_display_completion(source, consumer)
                .map(|display| display.map(|_| target));
        }
        if self.reference_symbol_identity(target, consumer)?
            == self.reference_symbol_identity(source, consumer)?
        {
            return Ok(NativeCompletion::clean(target));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mutable = if self.reference_symbol_flags(target, consumer)?.transient {
                self.mutable_reference_symbol(target, consumer)?
            } else {
                let resolved = sequence
                    .read(self.resolve_reference_symbol_completion(target, consumer))?
                    .ok_or_else(|| {
                        super::super::internal("resolveSymbol lost a nonnil merge target")
                    })?;
                if self.is_unknown_reference_symbol(resolved) {
                    return Ok(source);
                }
                let flags = self.reference_symbol_native_flags(resolved, consumer)?;
                if flags.conflicts_with(source_flags)
                    && !(flags.assignment || source_flags.assignment)
                {
                    sequence.read(self.diagnostic_symbol_display_completion(source, consumer))?;
                    return Ok(source);
                }
                self.clone_reference_symbol(resolved, consumer)?
            };
            let target = ReferenceSymbol::from_property(mutable);
            let flags = self
                .reference_symbol_native_flags(target, consumer)?
                .merged(source_flags);
            self.set_merge_symbol_flags(mutable, flags)?;
            if let Some(value) = self.reference_header_value_declaration(source, Some(consumer))? {
                self.set_merge_value_declaration(mutable, value, consumer)?;
            }
            for declaration in self.reference_header_declarations(source, Some(consumer))? {
                self.append_merge_declaration(mutable, declaration)?;
            }
            for exports in [false, true] {
                if let Some(raw) = self.reference_raw_symbol_table(source, exports)? {
                    let target_raw = self.reference_raw_symbol_table(target, exports)?;
                    let table = match target_raw {
                        Some(raw) => self.mutable_reference_raw_table(raw, consumer)?,
                        None => self.new_reference_raw_table(),
                    };
                    // ast.GetSymbolTable publishes allocation before recursion.
                    self.set_merge_raw_table(
                        mutable,
                        exports,
                        Some(ReferenceRawSymbolTable::Created(table)),
                    )?;
                    let source_table = self.reference_raw_table_entries(raw, consumer)?;
                    sequence.read(self.merge_reference_symbol_table_completion(
                        table,
                        &source_table,
                        unidirectional,
                        exports.then_some(target),
                        consumer,
                    ))?;
                }
            }
            if !unidirectional {
                let identity = self.reference_symbol_identity(source, consumer)?;
                self.symbol_tables.merged.insert(identity, target);
            }
            Ok(target)
        })();
        sequence.finish(result)
    }
}

impl SymbolTables {
    /// Request-local backing for the existing native cache questions. Their
    /// identities, child values and provisional publication stay operation-owned.
    pub(super) fn new() -> Self {
        Self {
            raw: Default::default(),
            raw_overlays: crate::resources::FixedMap::new(),
            merged: crate::resources::FixedMap::new(),
            projections: crate::resources::FixedMap::new(),
            headers: crate::resources::FixedMap::new(),
            resolved: crate::resources::FixedMap::new(),
            declaration_symbols: crate::resources::FixedMap::new(),
            late_symbols: crate::resources::FixedMap::new(),
        }
    }
}
