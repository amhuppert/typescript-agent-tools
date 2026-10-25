//! Native union key maps and best-match discrimination. Flow and relation
//! consumers share the same installed per-union decision and dependency order.

use super::super::model::{
    DemandError, IndexFlags, NativeCompletion, NativeResult, NativeSequence, ObjectFlags, Property,
    TypeFlags, TypeId, TypeView, UnionReduction,
};
use super::{Evaluator, ReferenceSymbol};
use crate::project::binding::NativePropertyKey;
use crate::references::Location;
use std::collections::HashMap;
use std::sync::Arc;

struct KeyProperty {
    name: NativePropertyKey,
    // None is the native unknownType entry for an ambiguous key, distinct
    // from a key absent from the map.
    constituents: HashMap<TypeId, Option<TypeId>>,
}

#[derive(Default)]
pub(super) struct DiscriminantsState {
    // None is the installed InternalSymbolNameMissing sentinel. Only a
    // completed computation installs the native pair; interruption leaves it
    // absent, without inventing a resolving or failure slot.
    keys: HashMap<TypeId, NativeCompletion<Option<Arc<KeyProperty>>>>,
}

impl Evaluator<'_, '_> {
    /// `getKeyPropertyName`: installation includes native negative answers.
    pub(super) fn union_key_property_name(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<NativePropertyKey>, DemandError> {
        self.union_key_property_name_completion(ty, consumer)?
            .admit()
    }

    pub(super) fn union_key_property_name_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<NativePropertyKey>> {
        if let Some(installed) = self.discriminants.keys.get(&ty) {
            return Ok(installed.clone().map(|key| key.map(|key| key.name.clone())));
        }
        let result = self.compute_key_property_completion(ty, consumer)?;
        // Native assigns both fields after the entire computation, including
        // reentrant work; an earlier nested publication does not win this write.
        self.discriminants.keys.insert(ty, result.clone());
        Ok(result.map(|key| key.map(|key| key.name.clone())))
    }

    /// `computeKeyPropertyNameAndMap`, `getKeyPropertyCandidateName` and
    /// `mapTypesByKeyProperty` own both feasibility gates and ordered reads.
    fn compute_key_property_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<Arc<KeyProperty>>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let types = self
                .model
                .union_constituents(ty)
                .ok_or_else(|| super::super::internal("key property question requires a union"))?;
            let candidates = TypeFlags::OBJECT | TypeFlags::INSTANTIABLE_NON_PRIMITIVE;
            if types.len() < 10
                || self
                    .model
                    .object_flags(ty)?
                    .intersects(ObjectFlags::PRIMITIVE_UNION)
                || types
                    .iter()
                    .filter(|&&ty| self.model.has_type_flags(ty, candidates))
                    .count()
                    < 10
            {
                return Ok(None);
            }
            let mut name = None;
            'candidate: for &ty in types.iter() {
                if self.model.has_type_flags(ty, candidates) {
                    for property in
                        sequence.read(self.properties_of_type_completion(ty, consumer))?
                    {
                        let value = sequence.read(self.demand_completion(property.read_type))?;
                        if self.model.has_type_flags(value, TypeFlags::UNIT) {
                            let symbol = self.property_symbol(&property, consumer)?;
                            name = Some(self.reference_symbol_key(
                                ReferenceSymbol::from_property(symbol),
                                consumer,
                            )?);
                            break 'candidate;
                        }
                    }
                }
            }
            let Some(name) = name.filter(|name| name.fixed_bytes() != Some(b"")) else {
                return Ok(None);
            };
            let mut constituents = HashMap::new();
            let mut count = 0;
            for &ty in types.iter() {
                if self
                    .model
                    .has_type_flags(ty, candidates | TypeFlags::INTERSECTION)
                {
                    let Some(discriminant) = sequence.read(
                        self.type_of_property_of_type_by_key_completion(ty, &name, consumer),
                    )?
                    else {
                        return Ok(None);
                    };
                    if !self.model.is_literal_type(discriminant) {
                        return Ok(None);
                    }
                    let distributed = self.model.union_constituents(discriminant);
                    let mut duplicate = false;
                    for &literal in distributed
                        .as_deref()
                        .unwrap_or(std::slice::from_ref(&discriminant))
                    {
                        let key = self.model.regular_literal(literal);
                        match constituents.entry(key) {
                            std::collections::hash_map::Entry::Vacant(entry) => {
                                entry.insert(Some(ty));
                            }
                            std::collections::hash_map::Entry::Occupied(mut entry) => {
                                if entry.get().is_some() {
                                    entry.insert(None);
                                    duplicate = true;
                                }
                            }
                        }
                    }
                    if !duplicate {
                        count += 1;
                    }
                }
            }
            Ok((count >= 10 && count * 2 >= types.len())
                .then(|| Arc::new(KeyProperty { name, constituents })))
        })();
        sequence.finish(result)
    }

    /// `getConstituentTypeForKeyType` requires an already installed key map;
    /// it does not recompute keys or demand a property.
    pub(super) fn union_constituent_for_key(
        &mut self,
        ty: TypeId,
        key: TypeId,
    ) -> Result<Option<TypeId>, DemandError> {
        let key = self.model.regular_literal(key);
        let installed = self.discriminants.keys.get(&ty).ok_or_else(|| {
            super::super::internal("constituent key lookup precedes key-property acquisition")
        })?;
        // This native operation reads the installed map, not its acquisition.
        Ok(installed
            .value()
            .as_ref()
            .and_then(|map| map.constituents.get(&key).copied().flatten()))
    }

    /// `getMatchingUnionConstituentForType`: key acquisition precedes the
    /// source's property read even when an ordinary candidate would match.
    pub(super) fn matching_union_constituent(
        &mut self,
        target: TypeId,
        source: TypeId,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        let Some(name) = self.union_key_property_name(target, consumer)? else {
            return Ok(None);
        };
        match self.type_of_property_of_type_by_key(source, &name, consumer)? {
            Some(key) => self.union_constituent_for_key(target, key),
            None => Ok(None),
        }
    }

    /// `findMatchingDiscriminantType` and its TypeDiscriminator preserve raw
    /// property reads inside each candidate comparison, without eager values.
    pub(super) fn matching_discriminant_type(
        &mut self,
        source: TypeId,
        target: TypeId,
        mut related: impl FnMut(&mut Self, TypeId, TypeId) -> Result<bool, DemandError>,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        if !self.model.has_type_flags(target, TypeFlags::UNION)
            || !self
                .model
                .has_type_flags(source, TypeFlags::INTERSECTION | TypeFlags::OBJECT)
        {
            return Ok(None);
        }
        if let Some(matched) = self.matching_union_constituent(target, source, consumer)? {
            return Ok(Some(matched));
        }
        let source_properties = self.properties_of_type(source, consumer)?;
        let properties = self.find_discriminant_properties(source_properties, target, consumer)?;
        if properties.is_empty() {
            return Ok(None);
        }
        let discriminated =
            self.discriminate_by_properties(target, &properties, &mut related, consumer)?;
        Ok((discriminated != target).then_some(discriminated))
    }

    /// `findDiscriminantProperties` selects the full source inventory through
    /// the target's shared lazy discriminant-property question.
    pub(super) fn find_discriminant_properties(
        &mut self,
        source: Vec<Property>,
        target: TypeId,
        consumer: &Location,
    ) -> Result<Vec<Property>, DemandError> {
        let mut properties = Vec::new();
        for property in source {
            let symbol = self.property_symbol(&property, consumer)?;
            let name =
                self.reference_symbol_key(ReferenceSymbol::from_property(symbol), consumer)?;
            if self.is_discriminant_property_by_key(target, &name, consumer)? {
                properties.push(property);
            }
        }
        Ok(properties)
    }

    /// TypeDiscriminator supplies native names and raw source types to the
    /// shared `discriminateTypeByDiscriminableItems` operation.
    fn discriminate_by_properties(
        &mut self,
        target: TypeId,
        properties: &[Property],
        related: &mut impl FnMut(&mut Self, TypeId, TypeId) -> Result<bool, DemandError>,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        // This caller still has an admitting relation boundary. Keep that
        // boundary explicit while sharing the native lazy comparison loop.
        self.discriminate_by_items_completion(
            target,
            properties.len(),
            |checker, index| {
                let symbol = checker.property_symbol(&properties[index], consumer)?;
                checker.reference_symbol_key(ReferenceSymbol::from_property(symbol), consumer)
            },
            |checker, index| {
                checker
                    .demand(properties[index].read_type)
                    .map(NativeCompletion::clean)
            },
            &mut |checker, source, target| {
                related(checker, source, target).map(NativeCompletion::clean)
            },
            consumer,
        )?
        .admit()
    }

    /// `discriminateTypeByDiscriminableItems`: names and source values are
    /// acquired inside each surviving target comparison, never precollected.
    /// An erroneous discriminator cannot eliminate every remaining type.
    pub(super) fn discriminate_by_items_completion(
        &mut self,
        target: TypeId,
        count: usize,
        mut name: impl FnMut(&mut Self, usize) -> Result<NativePropertyKey, DemandError>,
        mut source: impl FnMut(&mut Self, usize) -> NativeResult<TypeId>,
        related: &mut impl FnMut(&mut Self, TypeId, TypeId) -> NativeResult<bool>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let types = self
                .model
                .union_constituents(target)
                .ok_or_else(|| super::super::internal("discrimination requires a union"))?;
            let mut include = Vec::with_capacity(types.len());
            for &ty in types.iter() {
                let keep = if self.model.has_type_flags(ty, TypeFlags::PRIMITIVE) {
                    false
                } else {
                    let reduced = sequence.read(self.reduced_type_completion(ty, consumer))?;
                    !self.model.has_type_flags(reduced, TypeFlags::NEVER)
                };
                include.push(keep);
            }
            for item in 0..count {
                let mut matched = false;
                let mut rejected = Vec::new();
                for (index, &ty) in types.iter().enumerate() {
                    if include[index] {
                        let name = name(self, item)?;
                        if let Some(target_type) = sequence.read(
                            self.type_of_property_or_index_signature_by_key_completion(
                                ty, &name, consumer,
                            ),
                        )? {
                            let source_type = sequence.read(source(self, item))?;
                            let distributed = self.model.union_constituents(source_type);
                            let mut matches = false;
                            for &source_type in distributed
                                .as_deref()
                                .unwrap_or(std::slice::from_ref(&source_type))
                            {
                                if sequence.read(related(self, source_type, target_type))? {
                                    matches = true;
                                    break;
                                }
                            }
                            if matches {
                                matched = true;
                            } else {
                                rejected.push(index);
                            }
                        }
                    }
                }
                if matched {
                    for index in rejected {
                        include[index] = false;
                    }
                }
            }
            if include.contains(&false) {
                let retained = types
                    .iter()
                    .zip(include)
                    .filter_map(|(&ty, include)| include.then_some(ty))
                    .collect();
                let filtered = sequence.read(self.union_type_reduced_completion(
                    retained,
                    UnionReduction::None,
                    None,
                    None,
                    consumer,
                ))?;
                if !self.model.has_type_flags(filtered, TypeFlags::NEVER) {
                    return Ok(filtered);
                }
            }
            Ok(target)
        })();
        sequence.finish(result)
    }

    /// `findMatchingTypeReferenceOrTypeAliasReference`'s reference question:
    /// both are references to one generic target.
    pub(super) fn same_reference_target(
        &self,
        source: TypeId,
        target: TypeId,
    ) -> Result<bool, DemandError> {
        Ok(match (self.model.get(source)?, self.model.get(target)?) {
            (TypeView::Reference(source), TypeView::Reference(target)) => {
                source.target == target.target
            }
            _ => false,
        })
    }

    /// `findMatchingTypeReferenceOrTypeAliasReference`'s alias question: both
    /// carry an alias, of one alias symbol.
    pub(super) fn same_alias_symbol(
        &self,
        source: TypeId,
        target: TypeId,
    ) -> Result<bool, DemandError> {
        Ok(
            match (
                self.model.metadata(source)?.alias,
                self.model.metadata(target)?.alias,
            ) {
                (Some(source), Some(target)) => source.symbol == target.symbol,
                _ => false,
            },
        )
    }

    /// `getBestMatchingType` selects one reporting elaboration target in its
    /// native priority order. The callback uses the caller's live silent
    /// comparison, retaining its recursion assumptions and acquired caches.
    pub(super) fn best_matching_type(
        &mut self,
        source: TypeId,
        target: TypeId,
        related: impl FnMut(&mut Self, TypeId, TypeId) -> Result<bool, DemandError>,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        if let Some(matched) = self.matching_discriminant_type(source, target, related, consumer)? {
            return Ok(Some(matched));
        }
        let types = self
            .model
            .union_constituents(target)
            .ok_or_else(|| super::super::internal("best matching type requires a union"))?;
        let source_flags = self.model.object_flags(source)?;
        if source_flags.intersects(ObjectFlags::REFERENCE | ObjectFlags::ANONYMOUS) {
            for &target in types.iter() {
                if self.model.has_type_flags(target, TypeFlags::OBJECT) {
                    let overlap = source_flags & self.model.object_flags(target)?;
                    if overlap.intersects(ObjectFlags::REFERENCE)
                        && self.same_reference_target(source, target)?
                    {
                        return Ok(Some(target));
                    }
                    if overlap.intersects(ObjectFlags::ANONYMOUS)
                        && self.same_alias_symbol(source, target)?
                    {
                        return Ok(Some(target));
                    }
                }
            }
        }
        if source_flags.intersects(ObjectFlags::OBJECT_LITERAL) {
            let mut some_array_like = false;
            for &ty in types.iter() {
                if self.is_array_like_type(ty, consumer)? {
                    some_array_like = true;
                    break;
                }
            }
            if some_array_like {
                for &ty in types.iter() {
                    if !self.is_array_like_type(ty, consumer)? {
                        return Ok(Some(ty));
                    }
                }
            }
        }
        for construct in [false, true] {
            let signatures = if construct {
                self.construct_signatures(source, consumer)?
            } else {
                self.call_signatures(source, consumer)?
            };
            if !signatures.is_empty() {
                for &ty in types.iter() {
                    let signatures = if construct {
                        self.construct_signatures(ty, consumer)?
                    } else {
                        self.call_signatures(ty, consumer)?
                    };
                    if !signatures.is_empty() {
                        return Ok(Some(ty));
                    }
                }
            }
        }
        let mut best = None;
        let primitive = TypeFlags::PRIMITIVE | TypeFlags::INSTANTIABLE_PRIMITIVE;
        if !self.model.has_type_flags(source, primitive) {
            let mut matching_count = 0;
            for &target in types.iter() {
                if !self.model.has_type_flags(target, primitive) {
                    let source_keys = self.index_type_ex(source, IndexFlags::NONE, consumer)?;
                    let target_keys = self.index_type_ex(target, IndexFlags::NONE, consumer)?;
                    let overlap =
                        self.intersection_type(vec![source_keys, target_keys], consumer)?;
                    if self.model.has_type_flags(overlap, TypeFlags::INDEX) {
                        return Ok(Some(target));
                    }
                    if self
                        .model
                        .has_type_flags(overlap, TypeFlags::UNIT | TypeFlags::UNION)
                    {
                        let count = self.model.union_constituents(overlap).map_or(1, |types| {
                            types
                                .iter()
                                .filter(|&&ty| self.model.has_type_flags(ty, TypeFlags::UNIT))
                                .count()
                        });
                        if count >= matching_count {
                            best = Some(target);
                            matching_count = count;
                        }
                    }
                }
            }
        }
        Ok(best)
    }
}
