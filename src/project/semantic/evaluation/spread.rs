//! Shared object spread construction used by object and module factories.
use super::super::model::{
    DemandError, MembersView, NativeResult, NativeSequence, ObjectFlags, SpecialName, TypeFlags,
    TypeId, TypeSymbol,
};
use super::Evaluator;
use super::structure::MemberTable;
use crate::references::Location;
use crate::syntax::modifiers;
use std::collections::HashSet;
use std::sync::Arc;

impl Evaluator<'_, '_> {
    /// `getSpreadType` preserves native left-to-right folding, union expansion,
    /// private suppression and optional collision value acquisition.
    pub(super) fn spread_type_completion(
        &mut self,
        mut left: TypeId,
        mut right: TypeId,
        symbol: TypeSymbol,
        flags: ObjectFlags,
        readonly: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.has_type_flags(left, TypeFlags::ANY)
                || self.model.has_type_flags(right, TypeFlags::ANY)
            {
                return Ok(self.model.special(SpecialName::Any));
            }
            if self.model.has_type_flags(left, TypeFlags::UNKNOWN)
                || self.model.has_type_flags(right, TypeFlags::UNKNOWN)
            {
                return Ok(self.model.special(SpecialName::Unknown));
            }
            if self.model.has_type_flags(left, TypeFlags::NEVER) {
                return Ok(right);
            }
            if self.model.has_type_flags(right, TypeFlags::NEVER) {
                return Ok(left);
            }
            left = sequence
                .read(self.merge_object_and_empty_union_completion(left, readonly, consumer))?;
            if self.model.has_type_flags(left, TypeFlags::UNION) {
                sequence
                    .read(self.check_cross_product_union_completion(&[left, right], consumer))?;
                return sequence.read(self.map_type_completion(
                    left,
                    consumer,
                    |checker, child| {
                        checker.spread_type_completion(
                            child,
                            right,
                            symbol.clone(),
                            flags,
                            readonly,
                            consumer,
                        )
                    },
                ));
            }
            right = sequence
                .read(self.merge_object_and_empty_union_completion(right, readonly, consumer))?;
            if self.model.has_type_flags(right, TypeFlags::UNION) {
                sequence
                    .read(self.check_cross_product_union_completion(&[left, right], consumer))?;
                return sequence.read(self.map_type_completion(
                    right,
                    consumer,
                    |checker, child| {
                        checker.spread_type_completion(
                            left,
                            child,
                            symbol.clone(),
                            flags,
                            readonly,
                            consumer,
                        )
                    },
                ));
            }
            if self.model.has_type_flags(
                right,
                TypeFlags::BOOLEAN_LIKE
                    | TypeFlags::NUMBER_LIKE
                    | TypeFlags::BIGINT_LIKE
                    | TypeFlags::STRING_LIKE
                    | TypeFlags::ENUM_LIKE
                    | TypeFlags::NON_PRIMITIVE
                    | TypeFlags::INDEX,
            ) {
                return Ok(left);
            }
            if sequence.read(self.is_generic_object_type_completion(left, consumer))?
                || sequence.read(self.is_generic_object_type_completion(right, consumer))?
            {
                if sequence.read(self.is_empty_object_type_completion(left, consumer))? {
                    return Ok(right);
                }
                if let Some(types) = self.model.intersection_constituents(left) {
                    let mut types = types.to_vec();
                    let last = *types
                        .last()
                        .ok_or_else(|| super::super::internal("intersection lacks constituents"))?;
                    if sequence.read(self.is_non_generic_object_type_completion(last, consumer))?
                        && sequence
                            .read(self.is_non_generic_object_type_completion(right, consumer))?
                    {
                        *types.last_mut().expect("established nonempty intersection") = sequence
                            .read(self.spread_type_completion(
                                last, right, symbol, flags, readonly, consumer,
                            ))?;
                        return sequence.read(self.intersection_type_completion(types, consumer));
                    }
                }
                return sequence
                    .read(self.intersection_type_completion(vec![left, right], consumer));
            }
            let mut members = Vec::new();
            let mut skipped = HashSet::new();
            let indexes = if left == self.model.empty_object_type() {
                sequence.read(self.index_infos_of_type_completion(right, consumer))?
            } else {
                sequence.read(self.union_index_infos_completion(&[left, right], consumer))?
            };
            for property in sequence.read(self.properties_of_type_completion(right, consumer))? {
                let property = self.property_symbol(&property, consumer)?;
                if self.property_declaration_modifiers(property, false, consumer)?
                    & (modifiers::PRIVATE | modifiers::PROTECTED)
                    != 0
                {
                    let name = self.reference_symbol_key(
                        super::ReferenceSymbol::from_property(property),
                        consumer,
                    )?;
                    skipped.insert(name);
                } else if self.is_spreadable_property(property, consumer)? {
                    let name = self.reference_symbol_key(
                        super::ReferenceSymbol::from_property(property),
                        consumer,
                    )?;
                    let spread = sequence
                        .read(self.create_spread_symbol_completion(property, readonly, consumer))?;
                    members.push((name, spread));
                }
            }
            for property in sequence.read(self.properties_of_type_completion(left, consumer))? {
                let property = self.property_symbol(&property, consumer)?;
                let name = self.reference_symbol_key(
                    super::ReferenceSymbol::from_property(property),
                    consumer,
                )?;
                if skipped.contains(&name) || !self.is_spreadable_property(property, consumer)? {
                    continue;
                }
                if let Some(position) = members.iter().position(|(key, _)| key == &name) {
                    let right_property = members[position].1;
                    let right_type = sequence
                        .read(self.property_raw_type_completion(right_property, consumer))?;
                    if self.property_symbol_flags(right_property)?.optional {
                        members[position].1 =
                            sequence.read(self.create_overlapping_spread_symbol_completion(
                                property,
                                right_property,
                                right_type,
                                consumer,
                            ))?;
                    }
                } else {
                    members.push((
                        name,
                        sequence.read(
                            self.create_spread_symbol_completion(property, readonly, consumer),
                        )?,
                    ));
                }
            }
            let indexes = indexes
                .iter()
                .map(|info| self.model.index_info_with_readonly(info, readonly))
                .collect::<Vec<_>>();
            sequence.read(self.new_resolved_anonymous_type_completion(
                symbol,
                Some(MemberTable::Symbols(members.into())),
                MembersView {
                    properties: Arc::from([]),
                    call_signatures: Arc::from([]),
                    construct_signatures: Arc::from([]),
                    index_signatures: indexes.into(),
                },
                ObjectFlags::OBJECT_LITERAL
                    | ObjectFlags::CONTAINS_OBJECT_OR_ARRAY_LITERAL
                    | ObjectFlags::CONTAINS_SPREAD
                    | flags,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `tryMergeUnionOfObjectTypeAndEmptyObject` intentionally repeats the
    /// native Every/Find questions, whose inventory demands can be observable.
    pub(super) fn merge_object_and_empty_union_completion(
        &mut self,
        ty: TypeId,
        readonly: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(types) = self.model.union_constituents(ty) else {
                return Ok(ty);
            };
            let types = types.to_vec();
            let mut every_empty = true;
            for &child in &types {
                if !sequence.read(self.empty_or_spreads_into_empty_completion(child, consumer))? {
                    every_empty = false;
                    break;
                }
            }
            if every_empty {
                for &child in &types {
                    if sequence.read(self.is_empty_object_type_completion(child, consumer))? {
                        return Ok(child);
                    }
                }
                return Ok(self.model.empty_object_type());
            }
            let mut first = None;
            for &child in &types {
                if !sequence.read(self.empty_or_spreads_into_empty_completion(child, consumer))? {
                    first = Some(child);
                    break;
                }
            }
            let Some(first) = first else {
                return Ok(ty);
            };
            for &child in &types {
                if child != first
                    && !sequence
                        .read(self.empty_or_spreads_into_empty_completion(child, consumer))?
                {
                    return Ok(ty);
                }
            }
            let mut members = Vec::new();
            for property in sequence.read(self.properties_of_type_completion(first, consumer))? {
                let property = self.property_symbol(&property, consumer)?;
                if self.property_declaration_modifiers(property, false, consumer)?
                    & (modifiers::PRIVATE | modifiers::PROTECTED)
                    != 0
                {
                    continue;
                }
                if self.is_spreadable_property(property, consumer)? {
                    let name = self.reference_symbol_key(
                        super::ReferenceSymbol::from_property(property),
                        consumer,
                    )?;
                    members.push((
                        name,
                        sequence.read(self.create_optional_spread_symbol_completion(
                            property, readonly, consumer,
                        ))?,
                    ));
                }
            }
            let symbol = self.model.type_symbol(first);
            let indexes = sequence.read(self.index_infos_of_type_completion(first, consumer))?;
            sequence.read(self.new_resolved_anonymous_type_completion(
                symbol,
                Some(MemberTable::Symbols(members.into())),
                MembersView {
                    properties: Arc::from([]),
                    call_signatures: Arc::from([]),
                    construct_signatures: Arc::from([]),
                    index_signatures: indexes.into(),
                },
                ObjectFlags::OBJECT_LITERAL | ObjectFlags::CONTAINS_OBJECT_OR_ARRAY_LITERAL,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `isEmptyObjectTypeOrSpreadsIntoEmptyObject` asks the empty-object worker
    /// before examining primitive flags, preserving its member dependencies.
    fn empty_or_spreads_into_empty_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            Ok(
                sequence.read(self.is_empty_object_type_completion(ty, consumer))?
                    || self.model.has_type_flags(
                        ty,
                        TypeFlags::NULLABLE
                            | TypeFlags::BOOLEAN_LIKE
                            | TypeFlags::NUMBER_LIKE
                            | TypeFlags::BIGINT_LIKE
                            | TypeFlags::STRING_LIKE
                            | TypeFlags::ENUM_LIKE
                            | TypeFlags::NON_PRIMITIVE
                            | TypeFlags::INDEX,
                    ),
            )
        })();
        sequence.finish(result)
    }
}

impl Evaluator<'_, '_> {
    pub(super) fn spread_type(
        &mut self,
        left: TypeId,
        right: TypeId,
        symbol: TypeSymbol,
        flags: ObjectFlags,
        readonly: bool,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.spread_type_completion(left, right, symbol, flags, readonly, consumer)?
            .admit()
    }
    pub(super) fn merge_object_and_empty_union(
        &mut self,
        ty: TypeId,
        readonly: bool,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.merge_object_and_empty_union_completion(ty, readonly, consumer)?
            .admit()
    }
}
