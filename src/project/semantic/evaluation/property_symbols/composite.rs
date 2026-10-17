//! `createUnionOrIntersectionProperty` and the two native property caches.
//! Created symbols keep actual declarations and deferred value questions;
//! ordinary lookup and discriminant lookup deliberately see different sets.

use super::{Association, CreatedProperty, CreatedPropertyValue, PropertySymbolFlags, checks};
use crate::project::binding::{NativePropertyKey, NativeSymbol};
use crate::project::semantic::evaluation::Evaluator;
use crate::project::semantic::evaluation::ReferenceDeclaration;
use crate::project::semantic::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, ObjectFlags, Property,
    PropertySymbolId, SpecialName, TypeFlags, TypeId, TypeView,
};
use crate::references::Location;
use crate::syntax::modifiers;
use std::collections::HashSet;
use std::sync::Arc;

/// `getUnionOrIntersectionProperty`: the actual containing type, exact native
/// name and augmentation policy identify a cached symbol. Names retain their
/// shared payload; canonical hashing/equality remain NativeStringValue-owned.
pub(super) struct CompositeKey {
    ty: TypeId,
    name: NativePropertyKey,
    skip_augmentation: bool,
    fingerprint: u64,
}
impl CompositeKey {
    fn new(ty: TypeId, name: &NativePropertyKey, skip_augmentation: bool) -> Self {
        let mut hash = crate::project::semantic::model::KeyBuilder::new();
        hash.fixed(&ty);
        hash.fixed(&skip_augmentation);
        {
            hash.native_string(name.string_value());
        }
        Self {
            ty,
            name: name.clone(),
            skip_augmentation,
            fingerprint: hash.finish(),
        }
    }
}
impl crate::resources::CheckedKey for CompositeKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        if self.ty != other.ty || self.skip_augmentation != other.skip_augmentation {
            return false;
        }

        self.name
            .string_value()
            .canonical_equal(other.name.string_value())
    }
}

impl Evaluator<'_, '_> {
    pub(in crate::project::semantic::evaluation) fn property_of_composite_type_by_key(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        skip_augmentation: bool,
        consumer: &Location,
    ) -> Result<Option<Property>, DemandError> {
        self.property_of_composite_type_by_key_completion(ty, name, skip_augmentation, consumer)?
            .admit()
    }

    pub(in crate::project::semantic::evaluation) fn property_of_composite_type_by_key_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        skip_augmentation: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Property>, DemandError> {
            let property = sequence.read(self.union_or_intersection_property_by_key_completion(
                ty,
                name,
                skip_augmentation,
                consumer,
            ))?;
            match property {
                Some(property)
                    if self.property_check_flags(property.symbol.ok_or_else(|| {
                        crate::project::semantic::internal("composite property has no symbol")
                    })?)?
                        & checks::READ_PARTIAL
                        != 0 =>
                {
                    Ok(None)
                }
                property => Ok(property),
            }
        })();
        sequence.finish(result)
    }

    /// `getUnionOrIntersectionProperty`: one cached composite property per
    /// type, key and augmentation policy; only established results install.
    pub(in crate::project::semantic::evaluation) fn union_or_intersection_property_by_key_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        skip_augmentation: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        let key = CompositeKey::new(ty, name, skip_augmentation);
        if let Some(property) = self.native_members.symbols.composite_cache.get(&key) {
            return Ok(property.clone().map(Some));
        }
        let result =
            self.create_composite_property_completion(ty, name, skip_augmentation, consumer)?;
        if let Some(property) = result.value() {
            let installed = result
                .clone()
                .map(|property| property.expect("established composite property"));
            // Native publishes this cache before inspecting propagation flags.
            self.native_members
                .symbols
                .composite_cache
                .insert(key, installed.clone());
            let symbol = property.symbol.ok_or_else(|| {
                crate::project::semantic::internal("created property lacks symbol")
            })?;
            if skip_augmentation && self.property_check_flags(symbol)? & checks::PARTIAL == 0 {
                let key = CompositeKey::new(ty, name, false);
                if !self.native_members.symbols.composite_cache.contains(&key) {
                    self.native_members
                        .symbols
                        .composite_cache
                        .insert(key, installed);
                }
            }
        }
        Ok(result)
    }

    /// `getPropertiesOfUnionOrIntersectionType`: preserve native constituent
    /// order and stop a union after its first constituent without indexes.
    pub(in crate::project::semantic::evaluation) fn properties_of_composite_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<Property>> {
        if let Some(properties) = self.native_members.symbols.composite_inventories.get(&ty) {
            return Ok(properties.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Property>, DemandError> {
            let (types, union) = self.composite_constituents(ty)?;
            let mut checked = HashSet::new();
            let mut properties = Vec::new();
            for current in types.iter().copied() {
                for property in
                    sequence.read(self.properties_of_type_completion(current, consumer))?
                {
                    let name = self.property_key(&property, consumer)?;
                    if checked.insert(name.clone()) {
                        if let Some(property) =
                            sequence.read(self.property_of_composite_type_by_key_completion(
                                ty, &name, !union, consumer,
                            ))?
                        {
                            properties.push(property);
                        }
                    }
                }
                if union
                    && sequence
                        .read(self.index_infos_of_type_completion(current, consumer))?
                        .is_empty()
                {
                    break;
                }
            }
            Ok(properties)
        })();
        let result = sequence.finish(result)?;
        self.native_members
            .symbols
            .composite_inventories
            .insert(ty, result.clone());
        Ok(result)
    }

    pub(super) fn composite_constituents(
        &self,
        ty: TypeId,
    ) -> Result<(Arc<[TypeId]>, bool), DemandError> {
        match self.model.get(ty)? {
            TypeView::Union(types) => Ok((types, true)),
            TypeView::Intersection(types) => Ok((types, false)),
            _ => Err(crate::project::semantic::internal(
                "composite property owner is not a union or intersection",
            )
            .into()),
        }
    }

    /// Native `createUnionOrIntersectionProperty`: all flag and value decisions
    /// belong here, including partial properties hidden from normal lookup.
    fn create_composite_property_completion(
        &mut self,
        containing: TypeId,
        name: &NativePropertyKey,
        skip_augmentation: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Property>, DemandError> {
            let (types, union) = self.composite_constituents(containing)?;
            let mut single: Option<Property> = None;
            let mut set: Vec<PropertySymbolId> = Vec::new();
            let mut index_types = Vec::new();
            let mut flags = PropertySymbolFlags {
                optional: !union,
                ..Default::default()
            };
            let mut check = if union { 0 } else { checks::READONLY };
            let mut synthetic = checks::SYNTHETIC_METHOD;
            let mut merged_instantiations = false;
            for current in types.iter().copied() {
                let current = sequence.read(self.apparent_type_completion(current, consumer))?;
                if self.model.is_error_type(current)
                    || self.model.has_type_flags(current, TypeFlags::NEVER)
                {
                    continue;
                }
                if let Some(property) =
                    sequence.read(self.property_of_type_by_key_ex_completion(
                        current,
                        name,
                        skip_augmentation,
                        consumer,
                    ))?
                {
                    let symbol = property.symbol.ok_or_else(|| {
                        crate::project::semantic::internal("property has no native symbol")
                    })?;
                    let source_flags = self.property_symbol_flags(symbol)?;
                    let modifiers = self.property_declaration_modifiers(symbol, false, consumer)?;
                    if source_flags.class_member() {
                        if union {
                            flags.optional |= source_flags.optional;
                        } else {
                            flags.optional &= source_flags.optional;
                        }
                    }
                    if let Some(first) = &single {
                        let first_symbol = first.symbol.expect("established property symbol");
                        if symbol != first_symbol {
                            let same_target = self.property_target_symbol(symbol)?
                                == self.property_target_symbol(first_symbol)?;
                            if same_target
                                && sequence.read(self.compare_properties_equal_completion(
                                    first_symbol,
                                    symbol,
                                    consumer,
                                ))?
                            {
                                merged_instantiations = sequence.read(
                                    self.property_parent_has_local_parameters_completion(
                                        first_symbol,
                                        consumer,
                                    ),
                                )?;
                            } else {
                                if set.is_empty() {
                                    set.push(first_symbol);
                                }
                                if !set.contains(&symbol) {
                                    set.push(symbol);
                                }
                            }
                            if (flags.get_accessor || flags.set_accessor)
                                && (flags.get_accessor != source_flags.get_accessor
                                    || flags.set_accessor != source_flags.set_accessor)
                            {
                                flags.get_accessor = false;
                                flags.set_accessor = false;
                                flags.property = true;
                            }
                        }
                    } else {
                        flags.get_accessor = source_flags.get_accessor;
                        flags.set_accessor = source_flags.set_accessor;
                        flags.property = !flags.get_accessor && !flags.set_accessor;
                        single = Some(property);
                    }
                    let readonly =
                        sequence.read(self.property_is_readonly_completion(symbol, consumer))?;
                    if union && readonly {
                        check |= checks::READONLY;
                    } else if !union && !readonly {
                        check &= !checks::READONLY;
                    }
                    if modifiers & (modifiers::PRIVATE | modifiers::PROTECTED) == 0 {
                        check |= checks::PUBLIC;
                    }
                    if modifiers & modifiers::PRIVATE != 0 {
                        check |= checks::PRIVATE;
                    }
                    if modifiers & modifiers::PROTECTED != 0 {
                        check |= checks::PROTECTED;
                    }
                    if modifiers & modifiers::STATIC != 0 {
                        check |= checks::STATIC;
                    }
                    if !self.is_prototype_property(symbol)? {
                        synthetic = checks::SYNTHETIC_PROPERTY;
                    }
                } else if union {
                    let index = if name.is_known_symbol() {
                        None
                    } else {
                        sequence.read(
                            self.applicable_index_info_for_key_completion(current, name, consumer),
                        )?
                    };
                    if let Some(index) = index {
                        flags.get_accessor = false;
                        flags.set_accessor = false;
                        flags.property = true;
                        check |= checks::WRITE_PARTIAL;
                        if index.readonly {
                            check |= checks::READONLY;
                        }
                        let value = if self.model.is_tuple_type(current) {
                            sequence
                                .read(
                                    self.get_rest_type_of_tuple_type_completion(current, consumer),
                                )?
                                .unwrap_or_else(|| self.model.special(SpecialName::Undefined))
                        } else {
                            sequence.read(self.demand_completion(index.value_type))?
                        };
                        index_types.push(value);
                    } else if self
                        .model
                        .object_flags(current)?
                        .contains(ObjectFlags::OBJECT_LITERAL)
                        && !self
                            .model
                            .object_flags(current)?
                            .contains(ObjectFlags::CONTAINS_SPREAD)
                    {
                        check |= checks::WRITE_PARTIAL;
                        index_types.push(self.model.special(SpecialName::Undefined));
                    } else {
                        check |= checks::READ_PARTIAL;
                    }
                }
            }
            let Some(first) = single else {
                return Ok(None);
            };
            if union
                && (!set.is_empty() || check & checks::PARTIAL != 0)
                && check & (checks::PRIVATE | checks::PROTECTED) != 0
                && (set.is_empty() || !self.properties_have_common_declaration(&set, consumer)?)
            {
                return Ok(None);
            }
            let first_symbol = first.symbol.expect("established property symbol");
            if set.is_empty() && check & checks::READ_PARTIAL == 0 && index_types.is_empty() {
                if !merged_instantiations {
                    return Ok(Some(first));
                }
                return sequence
                    .read(self.clone_merged_property_completion(first, consumer))
                    .map(Some);
            }
            if set.is_empty() {
                set.push(first_symbol);
            }
            let mut declarations = Vec::new();
            let mut value_declaration = None;
            let mut different_declaration = false;
            let mut read_types = Vec::new();
            let mut write_types: Option<Vec<TypeId>> = None;
            let mut name_type = None;
            for symbol in set {
                let value = self.property_reference_value_declaration(symbol, consumer)?;
                if value_declaration.is_none() {
                    value_declaration = value;
                } else if value.is_some() && value != value_declaration {
                    different_declaration = true;
                }
                declarations.extend(self.property_reference_declarations(symbol, consumer)?);
                let ty = sequence.read(self.property_raw_type_completion(symbol, consumer))?;
                if read_types.is_empty() {
                    name_type = self.property_symbol_name_facts(symbol, consumer)?.name_type;
                }
                let write = sequence.read(self.property_write_type_completion(symbol, consumer))?;
                if write_types.is_some() || write != ty {
                    write_types
                        .get_or_insert_with(|| read_types.clone())
                        .push(write);
                }
                if read_types.first().is_some_and(|first| *first != ty) {
                    check |= checks::NON_UNIFORM;
                }
                if self.model.is_literal_type(ty) || self.is_pattern_literal_type(ty)? {
                    check |= checks::LITERAL;
                }
                if self.model.has_type_flags(ty, TypeFlags::NEVER)
                    && ty != self.model.unique_literal_type()
                {
                    check |= checks::NEVER;
                }
                read_types.push(ty);
            }
            read_types.extend(index_types);
            if different_declaration {
                value_declaration = None;
            }
            let parent = match value_declaration {
                Some(declaration) => {
                    self.reference_declaration_symbol_parent(declaration, consumer)?
                }
                None => None,
            };
            let identity = self.model.new_symbol_identity();
            let declaration = declarations
                .first()
                .copied()
                .map(|d| self.reference_declaration_ordering(d))
                .transpose()?;
            let header = NativeSymbol::created_with_key(identity, name.clone(), declaration);
            check |= synthetic;
            if read_types.len() > 2 {
                check |= checks::DEFERRED;
            }
            flags.partial = check & checks::PARTIAL != 0;
            let property = CreatedProperty {
                native_flags: None,
                flags,
                checks: check,
                declarations: declarations.into_iter().map(Into::into).collect(),
                value_declaration: value_declaration.map(Into::into),
                parent,
                raw_members: None,
                raw_exports: None,
                name_type,
                header,
                mapper: None,
                target: None,
                synthetic_origin: None,
                spread: None,
                export_type: None,
                value: CreatedPropertyValue::Composite {
                    containing,
                    read_types,
                    write_types,
                },
            };
            let id = self.install_created_property(property);
            if check & checks::DEFERRED == 0 {
                sequence.read(self.property_raw_type_completion(id, consumer))?;
                if matches!(
                    &self.created_property(id)?.value,
                    CreatedPropertyValue::Composite {
                        write_types: Some(_),
                        ..
                    }
                ) {
                    sequence.read(self.property_write_type_completion(id, consumer))?;
                }
            }
            self.project_property_symbol(id, consumer).map(Some)
        })();
        sequence.finish(result)
    }

    fn properties_have_common_declaration(
        &mut self,
        symbols: &[PropertySymbolId],
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        let mut common: Option<HashSet<ReferenceDeclaration>> = None;
        for symbol in symbols {
            let declarations = self.property_reference_declarations(*symbol, consumer)?;
            if declarations.is_empty() {
                return Ok(false);
            }
            let declarations: HashSet<_> = declarations.into_iter().collect();
            match &mut common {
                None => common = Some(declarations),
                Some(common) => common.retain(|d| declarations.contains(d)),
            }
            if common.as_ref().is_some_and(HashSet::is_empty) {
                return Ok(false);
            }
        }
        Ok(common.is_some_and(|common| !common.is_empty()))
    }
}

impl Evaluator<'_, '_> {
    /// `createSymbolWithType` in the merged-instantiation identity branch.
    /// This copy does not acquire Synthetic flags or become its source target
    /// for getTargetSymbol: only CheckFlags.Instantiated follows that link.
    fn clone_merged_property_completion(
        &mut self,
        original: Property,
        consumer: &Location,
    ) -> NativeResult<Property> {
        let source = original.symbol.expect("established property symbol");
        let resolved = if matches!(self.property_association(source)?, Association::Own(_)) {
            None
        } else {
            self.resolved_property_type(source, false)?
        };
        let mapper = match self.property_association(source)? {
            Association::Instantiated { mapper, .. } => Some(mapper),
            Association::Created(_) => self.created_property(source)?.mapper,
            Association::Own(_) => None,
        };
        let value_declaration = self.property_reference_value_declaration(source, consumer)?;
        let declarations = self.property_reference_declarations(source, consumer)?;
        let parent = match value_declaration {
            Some(declaration) => self.reference_declaration_symbol_parent(declaration, consumer)?,
            None => self.property_parent(source)?,
        };
        let name_type = self.property_name_type(source)?;
        let header = self
            .property_native_symbol(source, consumer)?
            .with_identity(self.model.new_symbol_identity());
        let mut flags = self.property_symbol_flags(source)?;
        flags.partial = false;
        let check = self.property_check_flags(source)? & checks::READONLY;
        let data = CreatedProperty {
            native_flags: None,
            flags,
            checks: check,
            declarations: declarations.into_iter().map(Into::into).collect(),
            value_declaration: value_declaration.map(Into::into),
            parent,
            raw_members: None,
            raw_exports: None,
            name_type,
            header,
            mapper,
            target: Some(source),
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: CreatedPropertyValue::Declaration,
        };
        let id = self.install_created_property(data);
        self.native_members.symbols.entries[id.index()].read = resolved.map(|ty| {
            super::TypeState::Installed(crate::project::semantic::model::NativeCompletion::clean(
                ty,
            ))
        });
        // Native always resolves the original write when making this clone.
        let write = self.property_write_type_completion(source, consumer)?;
        self.native_members.symbols.entries[id.index()].write =
            Some(super::TypeState::Installed(write.clone()));
        write.and_then_value(|_| self.project_property_symbol(id, consumer))
    }
}

impl Evaluator<'_, '_> {
    pub(in crate::project::semantic::evaluation) fn is_discriminant_property_by_key(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_discriminant_property_by_key_completion(ty, name, consumer)?
            .admit()
    }

    pub(in crate::project::semantic::evaluation) fn is_discriminant_property_by_key_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if !self.model.has_type_flags(ty, TypeFlags::UNION) {
            return Ok(NativeCompletion::clean(false));
        }
        self.union_or_intersection_property_by_key_completion(ty, name, false, consumer)?
            .and_then(|property| {
                let Some(property) = property else {
                    return Ok(NativeCompletion::clean(false));
                };
                let symbol = property.symbol.ok_or_else(|| {
                    crate::project::semantic::internal("discriminant property lacks symbol")
                })?;
                let check = self.property_check_flags(symbol)?;
                if check & checks::SYNTHETIC_PROPERTY == 0 {
                    return Ok(NativeCompletion::clean(false));
                }
                if let Some(result) = self.native_members.symbols.discriminants.get(&symbol) {
                    return Ok(result.clone());
                }
                self.native_members
                    .symbols
                    .discriminants
                    .insert(symbol, NativeCompletion::clean(false));
                if check & (checks::NON_UNIFORM | checks::LITERAL)
                    != (checks::NON_UNIFORM | checks::LITERAL)
                {
                    return Ok(NativeCompletion::clean(false));
                }
                let result = self
                    .property_raw_type_completion(symbol, consumer)?
                    .and_then(|ty| {
                        self.is_generic_type_completion(ty, consumer)
                            .map(|generic| generic.map(|generic| !generic))
                    })?;
                if *result.value() {
                    // Only the native IsDiscriminant OR publishes the completed
                    // suffix. A false computation leaves its provisional bit and
                    // keeps any acquired recovery local to this invocation.
                    self.native_members
                        .symbols
                        .discriminants
                        .insert(symbol, result.clone());
                }
                let installed = *self
                    .native_members
                    .symbols
                    .discriminants
                    .get(&symbol)
                    .expect("established discriminant state")
                    .value();
                Ok(result.map(|_| installed))
            })
    }
}
