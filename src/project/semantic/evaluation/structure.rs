//! Shared apparent types and complete native structured inventories.
//! Call, property, iteration and relation questions use these same operations.

use super::super::model::{
    DemandError, Index, MembersView, NativeCompletion, NativeResult, NativeSequence, ObjectFlags,
    Property, PropertySymbolId, Signature, SpecialName, TypeFlags, TypeId, TypeView,
};
use super::{Evaluator, facts::TypeFacts};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::NativePropertyKey;
use crate::references::Location;
use std::collections::HashMap;
use std::sync::Arc;

mod composite;

pub(super) enum MembersState {
    Resolving(Option<NativeCompletion<MembersView>>),
    Resolved(NativeCompletion<MembersView>),
    Failed(DemandError),
}

/// The actual native table is independent of its public named property list.
/// Existing object producers retain established descriptors; raw symbol tables
/// defer all projection until native name/value eligibility has been evaluated.
#[derive(Clone)]
pub(super) enum MemberTable {
    Projected(Arc<[Property]>),
    Symbols(Arc<[(NativePropertyKey, PropertySymbolId)]>),
}

#[derive(Clone)]
pub(super) enum MemberEntry {
    Projected(Property),
    Symbol {
        key: NativePropertyKey,
        symbol: PropertySymbolId,
    },
}

impl MemberTable {
    pub(super) fn entries(&self) -> Vec<MemberEntry> {
        match self {
            Self::Projected(properties) => properties
                .iter()
                .cloned()
                .map(MemberEntry::Projected)
                .collect(),
            Self::Symbols(symbols) => symbols
                .iter()
                .map(|(key, symbol)| MemberEntry::Symbol {
                    key: key.clone(),
                    symbol: *symbol,
                })
                .collect(),
        }
    }
}

pub(super) enum RawPropertyLookup {
    Absent,
    Hidden,
    Found(Property),
}

pub(super) struct StructureState {
    pub(super) members: HashMap<TypeId, MembersState>,
    // Absence, installed nil and present-empty are distinct native table states.
    pub(super) member_tables: HashMap<TypeId, Option<MemberTable>>,
    // `getApparentTypeOfIntersectionType` uses the intersection's own slot
    // when original == intersection, otherwise a cache keyed by original.
    // Those key domains are disjoint; one map retains both native identities.
    apparent_intersections: HashMap<TypeId, NativeCompletion<TypeId>>,
    generic_flags: HashMap<TypeId, NativeCompletion<(bool, bool)>>,
    reduced: HashMap<TypeId, NativeCompletion<TypeId>>,
    regular_objects: HashMap<TypeId, Result<TypeId, DemandError>>,
}

impl StructureState {
    pub(super) fn new() -> Self {
        Self {
            members: HashMap::new(),
            member_tables: HashMap::new(),
            apparent_intersections: HashMap::new(),
            generic_flags: HashMap::new(),
            reduced: HashMap::new(),
            regular_objects: HashMap::new(),
        }
    }
}

impl Evaluator<'_, '_> {
    /// `getRegularTypeOfObjectLiteral` transforms every property recursively
    /// before allocating the regular anonymous type. Only completed native
    /// values or their original failed dependency are retained in this cache.
    pub(super) fn regular_object_literal_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        if !self
            .model
            .object_flags(ty)?
            .contains(ObjectFlags::OBJECT_LITERAL | ObjectFlags::FRESH_LITERAL)
        {
            return Ok(ty);
        }
        if let Some(result) = self.structure.regular_objects.get(&ty) {
            return result.clone();
        }
        let result = (|| {
            let resolved = self
                .structured_type_members(ty, consumer)?
                .ok_or_else(|| super::super::internal("object literal lacks resolved members"))?;
            let mut properties = Vec::new();
            for property in self.properties_of_object_type(ty, consumer)? {
                let symbol = self.property_symbol(&property, consumer)?;
                let original = self.property_raw_type(symbol, consumer)?;
                let updated = self.regular_object_literal_type(original, consumer)?;
                properties.push(if original == updated {
                    property
                } else {
                    let symbol =
                        self.create_property_with_type(symbol, Some(updated), false, consumer)?;
                    self.project_property_symbol(symbol, consumer)?
                });
            }
            let properties = self.named_member_properties(
                properties,
                self.model.bound_type_symbol(ty)?,
                consumer,
            )?;
            let symbol = self.model.type_symbol(ty);
            let flags = self
                .model
                .object_flags(ty)?
                .without(ObjectFlags::FRESH_LITERAL);
            Ok(self.model.structured_with_flags(
                symbol,
                MembersView {
                    properties: properties.into(),
                    call_signatures: resolved.call_signatures,
                    construct_signatures: resolved.construct_signatures,
                    index_signatures: resolved.index_signatures,
                },
                super::super::model::Metadata::default(),
                super::super::model::ObjectKind::Anonymous,
                flags,
            ))
        })();
        self.structure.regular_objects.insert(ty, result.clone());
        result
    }

    /// `isEmptyObjectType`: generic mapped types are excluded before resolving
    /// inventory; union uses Some and intersection uses Every.
    pub(super) fn is_empty_object_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_empty_object_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn is_empty_object_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.has_type_flags(ty, TypeFlags::OBJECT) {
                if sequence.read(self.is_generic_mapped_type_completion(ty, consumer))? {
                    return Ok(false);
                }
                let members = sequence
                    .read(self.structured_type_members_completion(ty, consumer))?
                    .ok_or_else(|| super::super::internal("object lacks native inventory"))?;
                return Ok(self.model.is_empty_resolved_type(ty, &members));
            }
            if self.model.has_type_flags(ty, TypeFlags::NON_PRIMITIVE) {
                return Ok(true);
            }
            match self.model.get(ty)? {
                TypeView::Union(types) => {
                    for &child in types.iter() {
                        if sequence.read(self.is_empty_object_type_completion(child, consumer))? {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
                TypeView::Intersection(types) => {
                    for &child in types.iter() {
                        if !sequence.read(self.is_empty_object_type_completion(child, consumer))? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            }
        })();
        sequence.finish(result)
    }

    /// `resolveStructuredTypeMembers` observes installed inventories without
    /// starting resolution. The model owns already-resolved synthetic objects.
    pub(super) fn resolved_members_completion(
        &self,
        ty: TypeId,
    ) -> NativeResult<Option<MembersView>> {
        if let TypeView::Object(members) = self.model.get(ty)? {
            return Ok(NativeCompletion::clean(Some(members)));
        }
        match self.structure.members.get(&ty) {
            Some(MembersState::Resolved(members))
                if self
                    .model
                    .object_flags(ty)?
                    .contains(ObjectFlags::MEMBERS_RESOLVED) =>
            {
                Ok(members.clone().map(Some))
            }
            Some(MembersState::Failed(error)) => Err(error.clone()),
            Some(MembersState::Resolved(_) | MembersState::Resolving(_)) | None => {
                Ok(NativeCompletion::clean(None))
            }
        }
    }

    /// Absence, installed nil, and a present-empty raw table remain distinct.
    pub(super) fn resolved_member_table(&self, ty: TypeId) -> Option<Option<MemberTable>> {
        self.structure.member_tables.get(&ty).cloned()
    }

    /// The members field after resolveStructuredTypeMembers. Eager model
    /// factories already hold their complete table in MembersView; lazy
    /// factories retain the independently installed raw table, including nil.
    pub(super) fn structured_member_table(
        &self,
        ty: TypeId,
        members: &MembersView,
    ) -> Option<MemberTable> {
        match self.resolved_member_table(ty) {
            Some(table) => table,
            None => Some(MemberTable::Projected(members.properties.clone())),
        }
    }

    fn raw_member_entry(
        &self,
        ty: TypeId,
        members: &MembersView,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<Option<MemberEntry>, DemandError> {
        let Some(table) = self.structured_member_table(ty, members) else {
            return Ok(None);
        };
        // A native symbol-table lookup does not copy the complete inventory.
        // Keep projected keys in their existing owner and clone only the hit.
        match &table {
            MemberTable::Symbols(symbols) => {
                for (key, symbol) in symbols.iter() {
                    if key == name {
                        return Ok(Some(MemberEntry::Symbol {
                            key: key.clone(),
                            symbol: *symbol,
                        }));
                    }
                }
            }
            MemberTable::Projected(properties) => {
                for property in properties.iter() {
                    if &self.property_key(property, consumer)? == name {
                        return Ok(Some(MemberEntry::Projected(property.clone())));
                    }
                }
            }
        }
        Ok(None)
    }

    /// `isFunctionObjectType` asks raw table presence, with no value-meaning
    /// filter and without projecting the declaration or reading the property.
    pub(super) fn raw_member_present(
        &self,
        ty: TypeId,
        members: &MembersView,
        name: &str,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        Ok(self
            .raw_member_entry(
                ty,
                members,
                &NativePropertyKey::fixed(name.as_bytes()),
                consumer,
            )?
            .is_some())
    }

    /// Shared own-table branch of getPropertyOfTypeEx/getPropertyOfObjectType.
    /// Only the former suppresses type-only export-star views before symbolIsValue.
    pub(super) fn property_in_raw_members_completion(
        &mut self,
        ty: TypeId,
        members: &MembersView,
        name: &NativePropertyKey,
        check_export_star: bool,
        include_type_only: bool,
        consumer: &Location,
    ) -> NativeResult<RawPropertyLookup> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(entry) = self.raw_member_entry(ty, members, name, consumer)? else {
                return Ok(RawPropertyLookup::Absent);
            };
            if check_export_star
                && self
                    .type_symbol_flags(ty)?
                    .is_some_and(|flags| flags.value_module)
                && let Some(symbol) = self.reference_symbol_of_type(ty, consumer)?
                && self.reference_module_type_only_export_key(symbol, name, consumer)?
            {
                return Ok(RawPropertyLookup::Hidden);
            }
            let symbol = match &entry {
                MemberEntry::Projected(property) => property.symbol,
                MemberEntry::Symbol { symbol, .. } => Some(*symbol),
            };
            if let Some(symbol) = symbol {
                if !sequence.read(self.reference_symbol_is_value_ex_completion(
                    super::ReferenceSymbol::from_property(symbol),
                    include_type_only,
                    consumer,
                ))? {
                    return Ok(RawPropertyLookup::Absent);
                }
            }
            Ok(RawPropertyLookup::Found(match entry {
                MemberEntry::Projected(property) => property,
                MemberEntry::Symbol { symbol, .. } => {
                    self.project_property_symbol(symbol, consumer)?
                }
            }))
        })();
        sequence.finish(result)
    }

    /// The caller transfers only this inventory producer's completed prefix.
    /// Native publishes its table/flag before named-property projection; the
    /// prior signature/index fields survive reentry until their assignments.
    pub(super) fn set_structured_member_table_completion(
        &mut self,
        ty: TypeId,
        table: Option<MemberTable>,
        members: NativeCompletion<MembersView>,
        consumer: &Location,
    ) -> NativeResult<MembersView> {
        self.structure.member_tables.insert(ty, table.clone());
        self.model.mark_members_resolved(ty)?;
        let prior = match self.structure.members.get(&ty) {
            Some(MembersState::Resolved(prior) | MembersState::Resolving(Some(prior))) => {
                prior.value().clone()
            }
            _ => MembersView {
                properties: Arc::from([]),
                call_signatures: Arc::from([]),
                construct_signatures: Arc::from([]),
                index_signatures: Arc::from([]),
            },
        };
        self.structure
            .members
            .insert(ty, MembersState::Resolved(members.clone().map(|_| prior)));
        let result = members.and_then(|mut members| {
            let properties = match table {
                Some(properties) => {
                    let symbol = self.model.bound_type_symbol(ty)?;
                    self.named_member_entries_completion(properties.entries(), symbol, consumer)?
                }
                None => NativeCompletion::clean(Vec::new()),
            };
            Ok(properties.map(|properties| {
                members.properties = properties.into();
                members
            }))
        });
        if let Ok(members) = &result {
            self.structure
                .members
                .insert(ty, MembersState::Resolved(members.clone()));
        }
        result
    }

    /// `setStructuredTypeMembers` installs the raw table and resolved flag
    /// before sorting the public list. A nil table skips getNamedMembers.
    pub(super) fn set_structured_type_members_completion(
        &mut self,
        ty: TypeId,
        table: Option<Arc<[Property]>>,
        members: NativeCompletion<MembersView>,
        consumer: &Location,
    ) -> NativeResult<MembersView> {
        self.set_structured_member_table_completion(
            ty,
            table.map(MemberTable::Projected),
            members,
            consumer,
        )
    }

    /// `getPropertiesOfType`: reduction and apparent conversion precede the
    /// separate composite-property synthesis operation.
    pub(super) fn properties_of_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Vec<Property>, DemandError> {
        self.properties_of_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn properties_of_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<Property>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Property>, DemandError> {
            let ty = sequence.read(self.reduced_apparent_type_completion(ty, consumer))?;
            if self
                .model
                .has_type_flags(ty, TypeFlags::UNION | TypeFlags::INTERSECTION)
            {
                return sequence.read(self.properties_of_composite_type_completion(ty, consumer));
            }
            sequence.read(self.properties_of_object_type_completion(ty, consumer))
        })();
        sequence.finish(result)
    }

    /// `getPropertiesOfObjectType` deliberately does not acquire an apparent
    /// wrapper or inspect a composite's properties.
    pub(super) fn properties_of_object_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Vec<Property>, DemandError> {
        self.properties_of_object_type_completion(ty, consumer)?
            .admit()
    }

    pub(super) fn properties_of_object_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<Property>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Property>, DemandError> {
            if !self.model.has_type_flags(ty, TypeFlags::OBJECT) {
                return Ok(Vec::new());
            }
            let members = sequence
                .read(self.structured_type_members_completion(ty, consumer))?
                .ok_or_else(|| super::super::internal("native object lacks a member inventory"))?;
            Ok(members.properties.to_vec())
        })();
        sequence.finish(result)
    }

    /// `getIndexInfosOfType` asks the reduced apparent type before inventory.
    pub(super) fn index_infos_of_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Vec<Index>, DemandError> {
        self.index_infos_of_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn index_infos_of_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<Index>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Index>, DemandError> {
            let ty = sequence.read(self.reduced_apparent_type_completion(ty, consumer))?;
            sequence.read(self.index_infos_of_structured_type_completion(ty, consumer))
        })();
        sequence.finish(result)
    }

    /// `getIndexInfosOfStructuredType` deliberately skips apparent reduction,
    /// as required by contextual index lookup.
    pub(super) fn index_infos_of_structured_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<Index>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Index>, DemandError> {
            if !self.model.has_type_flags(
                ty,
                TypeFlags::OBJECT | TypeFlags::UNION | TypeFlags::INTERSECTION,
            ) {
                return Ok(Vec::new());
            }
            let members = sequence
                .read(self.structured_type_members_completion(ty, consumer))?
                .ok_or_else(|| {
                    super::super::internal("native structured type lacks an index inventory")
                })?;
            Ok(members.index_signatures.to_vec())
        })();
        sequence.finish(result)
    }

    /// `mapTypeWithAlias` maps actual constituents for a supplied alias; the
    /// ordinary path retains mapTypeEx's origin and unchanged-identity rules.
    pub(super) fn map_type_with_alias_completion(
        &mut self,
        ty: TypeId,
        alias: Option<super::super::model::Alias>,
        consumer: &Location,
        mut mapper: impl FnMut(&mut Self, TypeId) -> NativeResult<TypeId>,
    ) -> NativeResult<TypeId> {
        if let Some(types) = self.model.union_constituents(ty)
            && alias.is_some()
        {
            let _depth = self
                .control
                .as_ref()
                .map(|control| crate::resources::complete_or_abort(control.enter()));
            let mut sequence = NativeSequence::new();
            let result = (|| {
                let mut mapped = Vec::with_capacity(types.len());
                for &child in types.iter() {
                    mapped.push(sequence.read(mapper(self, child))?);
                }
                sequence.read(self.union_type_ex_completion(mapped, alias, None, consumer))
            })();
            sequence.finish(result)
        } else {
            self.map_type_completion(ty, consumer, mapper)
        }
    }

    /// `filterType` invokes its predicate again for non-union origin entries
    /// after normalized filtering changes a union. Those are real semantic
    /// reads, even when a previous constituent had the same identity.
    pub(super) fn filter_type_completion(
        &mut self,
        ty: TypeId,
        mut predicate: impl FnMut(&mut Self, TypeId) -> NativeResult<bool>,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut filter = self.model.begin_type_filter(ty);
            while let Some(part) = filter.next(&self.model) {
                let retain = sequence.read(predicate(self, part))?;
                filter.accept(part, retain);
            }
            Ok(filter.finish(&mut self.model))
        })();
        sequence.finish(result)
    }

    /// `mapTypeEx` for a total mapper. The Never type skips the callback;
    /// origins preserve nested traversal, and unchanged mappings preserve identity.
    pub(super) fn map_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
        mut mapper: impl FnMut(&mut Self, TypeId) -> Result<TypeId, DemandError>,
    ) -> Result<TypeId, DemandError> {
        self.map_type_completion(ty, consumer, |checker, child| {
            mapper(checker, child).map(NativeCompletion::clean)
        })?
        .admit()
    }

    pub(super) fn map_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
        mut mapper: impl FnMut(&mut Self, TypeId) -> NativeResult<TypeId>,
    ) -> NativeResult<TypeId> {
        self.map_type_worker_completion(ty, consumer, false, &mut |checker, child| {
            mapper(checker, child).map(|value| value.map(Some))
        })?
        .and_then_value(|value| {
            value.ok_or_else(|| super::super::internal("total type mapping returned nil").into())
        })
    }

    /// `mapTypeEx(noReductions=true)` retains the native no-reduction union.
    pub(super) fn map_type_no_reduction_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
        mut mapper: impl FnMut(&mut Self, TypeId) -> NativeResult<TypeId>,
    ) -> NativeResult<TypeId> {
        self.map_type_worker_completion(ty, consumer, true, &mut |checker, child| {
            mapper(checker, child).map(|value| value.map(Some))
        })?
        .and_then_value(|value| {
            value.ok_or_else(|| super::super::internal("total type mapping returned nil").into())
        })
    }

    /// `mapTypeEx` permits native nil callback results. An all-nil mapping is
    /// nil; every later constituent still runs after a completed nil/recovery.
    pub(super) fn map_type_optional_completion(
        &mut self,
        ty: TypeId,
        no_reductions: bool,
        consumer: &Location,
        mut mapper: impl FnMut(&mut Self, TypeId) -> NativeResult<Option<TypeId>>,
    ) -> NativeResult<Option<TypeId>> {
        self.map_type_worker_completion(ty, consumer, no_reductions, &mut mapper)
    }

    fn map_type_worker_completion<F>(
        &mut self,
        ty: TypeId,
        consumer: &Location,
        no_reductions: bool,
        mapper: &mut F,
    ) -> NativeResult<Option<TypeId>>
    where
        F: FnMut(&mut Self, TypeId) -> NativeResult<Option<TypeId>>,
    {
        // The invocation includes scalar/never shortcuts, callbacks, origin
        // recursion and reconstruction. Drop never asks for more budget.
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        if self.model.has_type_flags(ty, TypeFlags::NEVER) {
            return Ok(NativeCompletion::clean(Some(ty)));
        }
        let Some(types) = self.model.union_mapping_constituents(ty) else {
            return mapper(self, ty);
        };
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut changed = false;
            let mut result = Vec::with_capacity(types.len());
            for &child in types.iter() {
                let mapped = if self.model.has_type_flags(child, TypeFlags::UNION) {
                    sequence.read(self.map_type_worker_completion(
                        child,
                        consumer,
                        no_reductions,
                        mapper,
                    ))?
                } else {
                    sequence.read(mapper(self, child))?
                };
                changed |= mapped != Some(child);
                if let Some(mapped) = mapped {
                    result.push(mapped);
                }
            }
            if !changed {
                return Ok(Some(ty));
            }
            if result.is_empty() {
                return Ok(None);
            }
            sequence
                .read(self.union_type_reduced_completion(
                    result,
                    if no_reductions {
                        super::super::model::UnionReduction::None
                    } else {
                        super::super::model::UnionReduction::Literal
                    },
                    None,
                    None,
                    consumer,
                ))
                .map(Some)
        })();
        sequence.finish(result)
    }

    /// `checkNonNullTypeWithReporter` first rejects strict Unknown, then asks
    /// the shared full TypeFacts owner. A consumed nullable result requires
    /// native GetNonNullableType; no unchanged type substitutes for that owner.
    pub(super) fn check_non_null_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.check_non_null_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn check_non_null_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self.inputs.semantic_options().strict_null_checks
            && self.model.has_type_flags(ty, TypeFlags::UNKNOWN)
        {
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                super::super::invalid_at_location(
                    self.inputs,
                    consumer,
                    SemanticFailureReason::ErrorType,
                    "native non-null checking consumes unknown-type error recovery",
                )
                .into(),
            );
        }
        self.type_facts_completion(ty, TypeFacts::IS_UNDEFINED_OR_NULL, consumer)?.and_then(|facts| {
            if facts.contains(TypeFacts::IS_UNDEFINED_OR_NULL) {
                self.non_nullable_type_completion(ty, consumer)?.and_then(|non_nullable| {
                    if self.model.has_type_flags(non_nullable, TypeFlags::NULL | TypeFlags::UNDEFINED | TypeFlags::NEVER) {
                        NativeCompletion::recovered(self.model.native_error_type(), super::super::invalid_at_location(
                            self.inputs, consumer, SemanticFailureReason::ErrorType,
                            "native non-null checking consumes nullable-type error recovery",
                        ).into())
                    } else { Ok(NativeCompletion::clean(non_nullable)) }
                })
            } else { Ok(NativeCompletion::clean(ty)) }
        })
    }

    /// `checkNonNullNonVoidType` shares the complete non-null producer. Its
    /// additional Void diagnostic keeps that same type and acquires no types.
    pub(super) fn check_non_null_non_void_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.check_non_null_type_completion(ty, consumer)
    }

    /// `getElementTypeOfArrayType` asks native isArrayType, admitting both
    /// actual Array and ReadonlyArray targets before acquiring arguments.
    pub(super) fn array_element_type(&mut self, ty: TypeId) -> Result<Option<TypeId>, DemandError> {
        self.array_element_type_completion(ty)?.admit()
    }

    pub(super) fn array_element_type_completion(
        &mut self,
        ty: TypeId,
    ) -> NativeResult<Option<TypeId>> {
        self.is_array_type_completion(ty)?.and_then(|is_array| {
            if !is_array {
                return Ok(NativeCompletion::clean(None));
            }
            self.get_type_arguments_completion(ty)?
                .and_then_value(|arguments| {
                    arguments.first().copied().map(Some).ok_or_else(|| {
                        super::super::internal("native Array reference has no element argument")
                            .into()
                    })
                })
        })
    }

    /// `GetNonNullableType` is a type transformation, separate from expression
    /// checking's diagnostics. Loose-null mode preserves the actual identity.
    pub(super) fn non_nullable_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.non_nullable_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn non_nullable_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = if self.inputs.semantic_options().strict_null_checks {
            sequence.read(self.adjusted_type_with_facts_completion(
                ty,
                TypeFacts::NOT_NULLABLE,
                consumer,
            ))
        } else {
            Ok(ty)
        };
        sequence.finish(result)
    }

    /// `getNonNullableTypeIfNeeded` asks IsNullableType's facts before the
    /// adjusted non-null operation; generic constraints are observed there.
    pub(super) fn non_nullable_type_if_needed_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if sequence
                .read(self.type_facts_completion(ty, TypeFacts::IS_UNDEFINED_OR_NULL, consumer))?
                .contains(TypeFacts::IS_UNDEFINED_OR_NULL)
            {
                sequence.read(self.non_nullable_type_completion(ty, consumer))
            } else {
                Ok(ty)
            }
        })();
        sequence.finish(result)
    }

    /// `getSingleSignature`: only actual objects qualify, and the opposite
    /// kind must be absent even when own properties/indexes are permitted.
    pub(super) fn single_signature(
        &mut self,
        ty: TypeId,
        construct: bool,
        allow_members: bool,
        consumer: &Location,
    ) -> Result<Option<Signature>, DemandError> {
        self.single_signature_completion(ty, construct, allow_members, consumer)?
            .admit()
    }

    pub(super) fn single_signature_completion(
        &mut self,
        ty: TypeId,
        construct: bool,
        allow_members: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Signature>> {
        if !self.model.has_type_flags(ty, TypeFlags::OBJECT) {
            return Ok(NativeCompletion::clean(None));
        }
        self.structured_type_members_completion(ty, consumer)?
            .and_then_value(|members| {
                let members = members.ok_or_else(|| {
                    super::super::internal("native object type has no structured inventory")
                })?;
                if !allow_members
                    && (!members.properties.is_empty() || !members.index_signatures.is_empty())
                {
                    return Ok(None);
                }
                let (signatures, opposite) = if construct {
                    (&members.construct_signatures, &members.call_signatures)
                } else {
                    (&members.call_signatures, &members.construct_signatures)
                };
                Ok(if signatures.len() == 1 && opposite.is_empty() {
                    Some(signatures[0].clone())
                } else {
                    None
                })
            })
    }

    /// `isGenericType`, `isGenericObjectType` and `isGenericIndexType` ask
    /// different bits of the same native getGenericObjectFlags operation.
    pub(super) fn is_generic_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_generic_type_completion(ty, consumer)?.admit()
    }
    pub(super) fn is_generic_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        Ok(self
            .generic_flags_completion(ty, consumer)?
            .map(|flags| flags.0 || flags.1))
    }

    pub(super) fn is_generic_object_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_generic_object_type_completion(ty, consumer)?
            .admit()
    }
    pub(super) fn is_generic_object_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        Ok(self
            .generic_flags_completion(ty, consumer)?
            .map(|flags| flags.0))
    }

    pub(super) fn is_generic_index_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        Ok(self
            .generic_flags_completion(ty, consumer)?
            .map(|flags| flags.1))
    }

    /// `isGenericMappedType` checks the constraint before substituting it for
    /// the key parameter in an actual remapping expression.
    pub(in crate::project::semantic) fn is_generic_mapped_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_generic_mapped_type_completion(ty, consumer)?
            .admit()
    }
    pub(in crate::project::semantic) fn is_generic_mapped_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self
                .model
                .object_flags(ty)?
                .intersects(super::super::model::ObjectFlags::MAPPED)
            {
                return Ok(false);
            }
            let constraint = sequence.read(self.mapped_constraint_completion(ty, consumer))?;
            if sequence.read(self.is_generic_index_type_completion(constraint, consumer))? {
                return Ok(true);
            }
            if let Some(name) = sequence.read(self.mapped_name_completion(ty, consumer))? {
                let parameter = sequence.read(self.mapped_parameter_completion(ty, consumer))?;
                let mapper = self.model.mapper(vec![parameter], vec![constraint])?;
                let name =
                    sequence.read(self.instantiate_type_completion(name, mapper, consumer))?;
                return sequence.read(self.is_generic_index_type_completion(name, consumer));
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// Native `isNonGenericObjectType` excludes generic mapped types only.
    pub(super) fn is_non_generic_object_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_non_generic_object_type_completion(ty, consumer)?
            .admit()
    }
    pub(super) fn is_non_generic_object_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if !self.model.has_type_flags(ty, TypeFlags::OBJECT) {
            return Ok(NativeCompletion::clean(false));
        }
        Ok(self
            .is_generic_mapped_type_completion(ty, consumer)?
            .map(|generic| !generic))
    }

    /// `getGenericObjectFlags` caches only the actual union/intersection and
    /// substitution flags. Prefix evidence never enters a child cache entry.
    pub(super) fn generic_flags_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<(bool, bool)> {
        if let Some(flags) = self.structure.generic_flags.get(&ty) {
            return Ok(flags.clone());
        }
        let mut sequence = NativeSequence::new();
        let mut install = false;
        let result = (|| {
            if let Some(types) = self
                .model
                .union_constituents(ty)
                .or_else(|| self.model.intersection_constituents(ty))
            {
                let mut flags = (false, false);
                for &ty in types.iter() {
                    let child = sequence.read(self.generic_flags_completion(ty, consumer))?;
                    flags.0 |= child.0;
                    flags.1 |= child.1;
                }
                install = true;
                return Ok(flags);
            }
            if let TypeView::Substitution(view) = self.model.get(ty)? {
                let base = sequence.read(self.generic_flags_completion(view.base, consumer))?;
                let constraint =
                    sequence.read(self.generic_flags_completion(view.constraint, consumer))?;
                install = true;
                return Ok((base.0 || constraint.0, base.1 || constraint.1));
            }
            let instantiable = self
                .model
                .has_type_flags(ty, TypeFlags::INSTANTIABLE_NON_PRIMITIVE);
            let generic_object = instantiable
                || sequence.read(self.is_generic_mapped_type_completion(ty, consumer))?
                || self.model.is_generic_tuple_type(ty)?;
            let index = self
                .model
                .has_type_flags(ty, TypeFlags::INSTANTIABLE_NON_PRIMITIVE | TypeFlags::INDEX)
                || self.is_generic_string_like_type(ty)?;
            Ok((generic_object, index))
        })();
        let result = sequence.finish(result);
        if install && let Ok(flags) = &result {
            self.structure.generic_flags.insert(ty, flags.clone());
        }
        result
    }

    /// `getReducedType` leaves noncomposites and unions without intersections
    /// unchanged. An intersection consumes its synthesized property inventory.
    pub(super) fn reduced_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.reduced_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn reduced_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(result) = self.structure.reduced.get(&ty) {
            return Ok(result.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            match self.model.get(ty)? {
                TypeView::Union(types)
                    if self
                        .model
                        .object_flags(ty)?
                        .contains(ObjectFlags::CONTAINS_INTERSECTIONS) =>
                {
                    let mut reduced = Vec::with_capacity(types.len());
                    let mut changed = false;
                    for &child in types.iter() {
                        let value = sequence.read(self.reduced_type_completion(child, consumer))?;
                        changed |= value != child;
                        reduced.push(value);
                    }
                    if !changed {
                        return Ok(ty);
                    }
                    let value = sequence
                        .read(self.union_type_ex_completion(reduced, None, None, consumer))?;
                    if self.model.has_type_flags(value, TypeFlags::UNION) {
                        // The resulting union's self-reduction slot contains no
                        // acquisitions from the original union's invocation.
                        self.structure
                            .reduced
                            .insert(value, NativeCompletion::clean(value));
                    }
                    Ok(value)
                }
                TypeView::Intersection(_) => {
                    // Native installs IsNeverIntersectionComputed before the
                    // property walk. A stopped walk cannot erase that marker.
                    self.structure
                        .reduced
                        .insert(ty, NativeCompletion::clean(ty));
                    for property in
                        sequence.read(self.properties_of_composite_type_completion(ty, consumer))?
                    {
                        let symbol = property.symbol.ok_or_else(|| {
                            super::super::internal("intersection property lacks native symbol")
                        })?;
                        if sequence
                            .read(self.is_never_reduced_property_completion(symbol, consumer))?
                        {
                            return Ok(self.model.special(SpecialName::Never));
                        }
                    }
                    Ok(ty)
                }
                _ => Ok(ty),
            }
        })();
        let result = sequence.finish(result);
        if let Ok(value) = &result {
            match self.model.get(ty)? {
                TypeView::Union(_) => {
                    self.structure.reduced.insert(ty, value.clone());
                }
                TypeView::Intersection(_) if *value.value() != ty => {
                    // Only IsNeverIntersection is written after the walk.
                    // A false result leaves the earlier computed marker clean;
                    // its traversal evidence belongs to this invocation only.
                    self.structure.reduced.insert(ty, value.clone());
                }
                _ => {}
            }
        }
        result
    }

    /// Native `ObjectFlagsIsNeverIntersection` is an installed reduction
    /// observation. Reading it must not initiate a new property traversal.
    pub(super) fn is_never_intersection(&self, ty: TypeId) -> Result<bool, DemandError> {
        if !self.model.has_type_flags(ty, TypeFlags::INTERSECTION) {
            return Ok(false);
        }
        match self.structure.reduced.get(&ty) {
            Some(reduced) => Ok(self
                .model
                .has_type_flags(*reduced.value(), TypeFlags::NEVER)),
            None => Ok(false),
        }
    }

    /// `getReducedApparentType` reduces both before and after apparent conversion.
    pub(super) fn reduced_apparent_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.reduced_apparent_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn reduced_apparent_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.reduced_type_completion(ty, consumer)?
            .and_then(|ty| self.apparent_type_completion(ty, consumer))?
            .and_then(|ty| self.reduced_type_completion(ty, consumer))
    }

    /// `getApparentType` preserves constraint/global-wrapper dependencies.
    /// Canonical emptyObjectType is healthy native state, never a substitute
    /// for a missing global wrapper's recovery type.
    pub(super) fn apparent_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.apparent_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn apparent_type_completion(
        &mut self,
        mut ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let original = ty;
            if self.model.has_type_flags(ty, TypeFlags::INSTANTIABLE) {
                ty = match sequence.read(self.base_constraint_completion(ty, consumer))? {
                    Some(base) => base,
                    None => self.model.special(SpecialName::Unknown),
                };
            }
            if self
                .model
                .object_flags(ty)?
                .intersects(super::super::model::ObjectFlags::MAPPED)
            {
                ty = sequence.read(self.apparent_mapped_type_completion(ty, consumer))?;
            }
            if ty != original
                && self
                    .model
                    .object_flags(ty)?
                    .intersects(super::super::model::ObjectFlags::REFERENCE)
            {
                return sequence.read(self.get_type_with_this_argument_completion(
                    ty,
                    Some(original),
                    false,
                    consumer,
                ));
            }
            if self.model.has_type_flags(ty, TypeFlags::INTERSECTION) {
                if let Some(apparent) = self.structure.apparent_intersections.get(&original) {
                    return sequence.read(Ok(apparent.clone()));
                }
                let apparent = self.get_type_with_this_argument_completion(
                    ty,
                    Some(original),
                    true,
                    consumer,
                )?;
                self.structure
                    .apparent_intersections
                    .insert(original, apparent.clone());
                return sequence.read(Ok(apparent));
            }
            // Boolean retains union storage, so its native flags rather than the
            // storage view decide the wrapper. Ordinary unions stay unchanged.
            for (flags, name) in [
                (TypeFlags::STRING_LIKE, "String"),
                (TypeFlags::NUMBER_LIKE, "Number"),
                (TypeFlags::BIGINT_LIKE, "BigInt"),
                (TypeFlags::BOOLEAN_LIKE, "Boolean"),
                (TypeFlags::ES_SYMBOL_LIKE, "Symbol"),
            ] {
                if self.model.has_type_flags(ty, flags) {
                    return sequence.read(self.global_object_type_completion(name, consumer));
                }
            }
            if self.model.has_type_flags(ty, TypeFlags::NON_PRIMITIVE)
                || self.model.has_type_flags(ty, TypeFlags::UNKNOWN)
                    && !self.inputs.semantic_options().strict_null_checks
            {
                return Ok(self.model.empty_object_type());
            }
            if self.model.has_type_flags(ty, TypeFlags::INDEX) {
                return Ok(self.model.string_number_symbol_type());
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    pub(super) fn call_signatures(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Vec<Signature>, DemandError> {
        self.call_signatures_completion(ty, consumer)?.admit()
    }

    pub(super) fn call_signatures_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<Signature>> {
        Ok(self
            .signatures_of_type_completion(ty, false, consumer)?
            .map(|signatures| self.model.copy_signature_handles(&signatures)))
    }

    pub(super) fn construct_signatures(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Vec<Signature>, DemandError> {
        self.construct_signatures_completion(ty, consumer)?.admit()
    }

    pub(super) fn construct_signatures_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<Signature>> {
        Ok(self
            .signatures_of_type_completion(ty, true, consumer)?
            .map(|signatures| self.model.copy_signature_handles(&signatures)))
    }

    /// `getSignaturesOfType` applies reduced apparent conversion before the
    /// narrower structured inventory question.
    pub(super) fn signatures_of_type(
        &mut self,
        ty: TypeId,
        construct: bool,
        consumer: &Location,
    ) -> Result<Arc<[Signature]>, DemandError> {
        self.signatures_of_type_completion(ty, construct, consumer)?
            .admit()
    }

    pub(super) fn signatures_of_type_completion(
        &mut self,
        ty: TypeId,
        construct: bool,
        consumer: &Location,
    ) -> NativeResult<Arc<[Signature]>> {
        self.reduced_apparent_type_completion(ty, consumer)?
            .and_then(|ty| self.signatures_of_structured_type_completion(ty, construct, consumer))
    }

    /// `getSignaturesOfStructuredType` does not acquire apparent types or
    /// constraints; alias module wrapping calls this narrower operation.
    pub(super) fn signatures_of_structured_type_completion(
        &mut self,
        ty: TypeId,
        construct: bool,
        consumer: &Location,
    ) -> NativeResult<Arc<[Signature]>> {
        if !self.model.has_type_flags(
            ty,
            TypeFlags::OBJECT | TypeFlags::UNION | TypeFlags::INTERSECTION,
        ) {
            return Ok(NativeCompletion::clean(Arc::from([])));
        }
        self.structured_type_members_completion(ty, consumer)?
            .and_then_value(|members| {
                let members = members.ok_or_else(|| {
                    super::super::internal("native structured type lacks a signature inventory")
                })?;
                Ok(if construct {
                    members.construct_signatures
                } else {
                    members.call_signatures
                })
            })
    }

    /// `removeMissingOrUndefinedType`: exact optionality removes the private
    /// missing identity; the other mode asks native undefined possibility facts.
    pub(super) fn remove_missing_or_undefined_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = if self.inputs.semantic_options().exact_optional_property_types {
            self.model
                .remove_missing_type(ty, true)
                .map_err(|error| error.at(self.inputs, consumer))
        } else {
            sequence.read(self.type_with_facts_completion(ty, TypeFacts::NOT_UNDEFINED, consumer))
        };
        sequence.finish(result)
    }

    /// `getSignaturesOfStructuredType` / resolveStructuredTypeMembers.
    /// Nonstructured types have known empty inventories. Apparent-type and
    /// constraint acquisition belong to the callers that ask those questions.
    pub(super) fn structured_type_members(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<MembersView>, DemandError> {
        self.structured_type_members_completion(ty, consumer)?
            .admit()
    }

    pub(super) fn structured_type_members_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<MembersView>> {
        match self.model.get(ty)? {
            TypeView::Object(members) => Ok(NativeCompletion::clean(Some(members))),
            TypeView::LazyObject(view) => Ok(self
                .members_completion_at(view.members, consumer)?
                .map(Some)),
            TypeView::Reference(view) => Ok(self
                .members_completion_at(view.members, consumer)?
                .map(Some)),
            TypeView::Union(types) => Ok(self
                .composite_members_completion(ty, &types, true, consumer)?
                .map(Some)),
            TypeView::Intersection(types) => Ok(self
                .composite_members_completion(ty, &types, false, consumer)?
                .map(Some)),
            TypeView::TypeParameter(_)
            | TypeView::IndexedAccess(_)
            | TypeView::Index(_)
            | TypeView::TemplateLiteral(_)
            | TypeView::Conditional(_)
            | TypeView::Substitution(_)
            | TypeView::StringMapping(_)
            | TypeView::Special(_)
            | TypeView::Primitive(_)
            | TypeView::Enum
            | TypeView::UniqueSymbol(_)
            | TypeView::Literal { .. } => Ok(NativeCompletion::clean(None)),
        }
    }
}

impl Evaluator<'_, '_> {
    pub(super) fn remove_missing_or_undefined_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.remove_missing_or_undefined_type_completion(ty, consumer)?
            .admit()
    }
}
