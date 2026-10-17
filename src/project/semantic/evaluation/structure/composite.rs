//! Complete composite inventories. Properties retain their separate synthetic
//! lookup owner; this module installs the native nil member table.

use super::super::SymbolParent;
use super::super::globals::FixedGlobalType;
use super::*;
use crate::project::semantic::model::{TypeSymbol, UnionReduction};

impl Evaluator<'_, '_> {
    /// `resolveUnionTypeMembers` / `resolveIntersectionTypeMembers` cache the
    /// installed inventory on the actual composite, independently of properties.
    pub(in crate::project::semantic::evaluation) fn composite_members_completion(
        &mut self,
        ty: TypeId,
        types: &[TypeId],
        is_union: bool,
        consumer: &Location,
    ) -> NativeResult<MembersView> {
        self.resolved_members_completion(ty)?.and_then(|installed| {
            if let Some(members) = installed {
                return Ok(NativeCompletion::clean(members));
            }
            let members = if is_union {
                self.union_members_completion(ty, types, consumer)
            } else {
                self.intersection_members_completion(types, consumer)
            }?;
            self.set_structured_type_members_completion(ty, None, members, consumer)
        })
    }

    fn union_members_completion(
        &mut self,
        ty: TypeId,
        types: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<MembersView> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<MembersView, DemandError> {
            let mut call_lists = Vec::with_capacity(types.len());
            for &child in types {
                let global_function = sequence
                    .read(self.global_object_identity_named_completion("Function", consumer))?;
                call_lists.push(if child == global_function {
                    Arc::from(vec![self.unknown_signature()?])
                } else {
                    sequence.read(self.signatures_of_type_completion(child, false, consumer))?
                });
            }
            let mut calls =
                sequence.read(self.union_signatures_completion(&call_lists, consumer))?;
            if calls.is_empty() {
                calls = sequence
                    .read(self.array_member_call_signatures_completion(ty, types, consumer))?;
            }
            let mut construct_lists = Vec::with_capacity(types.len());
            for &child in types {
                construct_lists.push(
                    sequence.read(self.signatures_of_type_completion(child, true, consumer))?,
                );
            }
            let constructs =
                sequence.read(self.union_signatures_completion(&construct_lists, consumer))?;
            let indexes = sequence.read(self.union_index_infos_completion(types, consumer))?;
            Ok(MembersView {
                properties: Vec::new().into(),
                call_signatures: calls.into(),
                construct_signatures: constructs.into(),
                index_signatures: indexes.into(),
            })
        })();
        sequence.finish(result)
    }

    /// `getUnionIndexInfos`: all exact-key presence checks precede ordered
    /// value reads, which precede the readonly short-circuit pass.
    pub(in crate::project::semantic::evaluation) fn union_index_infos_completion(
        &mut self,
        types: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<Vec<Index>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Index>, DemandError> {
            let Some(&first) = types.first() else {
                return Err(super::super::super::internal(
                    "union index inventory has no constituents",
                )
                .into());
            };
            let source = sequence.read(self.index_infos_of_type_completion(first, consumer))?;
            let mut result = Vec::new();
            for index in source {
                let key = sequence.read(self.demand_completion(index.key_type))?;
                let mut present = true;
                for &ty in types {
                    if sequence
                        .read(self.index_info_of_type_completion(ty, key, consumer))?
                        .is_none()
                    {
                        present = false;
                        break;
                    }
                }
                if !present {
                    continue;
                }
                let mut values = Vec::with_capacity(types.len());
                for &ty in types {
                    values.push(
                        sequence
                            .read(self.index_type_of_type_completion(ty, key, consumer))?
                            .ok_or_else(|| {
                                super::super::super::internal("established union index disappeared")
                            })?,
                    );
                }
                let value = sequence.read(self.union_type_reduced_completion(
                    values,
                    UnionReduction::Literal,
                    None,
                    None,
                    consumer,
                ))?;
                let mut readonly = false;
                for &ty in types {
                    if sequence
                        .read(self.index_info_of_type_completion(ty, key, consumer))?
                        .ok_or_else(|| {
                            super::super::super::internal(
                                "established readonly union index disappeared",
                            )
                        })?
                        .readonly
                    {
                        readonly = true;
                        break;
                    }
                }
                result.push(self.model.index_info(key, value, readonly, None, None));
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `appendIndexInfo` combines only exact key identities, allocating a
    /// fresh synthetic record. Union mappings retain either readonly flag;
    /// intersections require both. Unmatched infos keep their identity.
    pub(in crate::project::semantic::evaluation) fn append_index_info_completion(
        &mut self,
        result: &mut Vec<Index>,
        new: Index,
        union: bool,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let key = sequence.read(self.demand_completion(new.key_type))?;
            for index in result.iter_mut() {
                if sequence.read(self.demand_completion(index.key_type))? == key {
                    let left = sequence.read(self.demand_completion(index.value_type))?;
                    let right = sequence.read(self.demand_completion(new.value_type))?;
                    let (value, readonly) = if union {
                        (
                            sequence.read(self.union_type_reduced_completion(
                                vec![left, right],
                                UnionReduction::Literal,
                                None,
                                None,
                                consumer,
                            ))?,
                            index.readonly || new.readonly,
                        )
                    } else {
                        (
                            sequence.read(
                                self.intersection_type_completion(vec![left, right], consumer),
                            )?,
                            index.readonly && new.readonly,
                        )
                    };
                    *index = self.model.index_info(key, value, readonly, None, None);
                    return Ok(());
                }
            }
            result.push(new);
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `isMixinConstructorType` asks the single raw parameter's call type;
    /// a known Any is sufficient before the separate mutable-array question.
    pub(in crate::project::semantic::evaluation) fn is_mixin_constructor_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let signatures = sequence.read(self.construct_signatures_completion(ty, consumer))?;
            if signatures.len() != 1 {
                return Ok(false);
            }
            let signature = &signatures[0];
            if !signature.type_parameter_types().is_empty()
                || signature.parameters.len() != 1
                || !signature.flags.has_rest()
            {
                return Ok(false);
            }
            let parameter =
                sequence.read(self.demand_completion(signature.parameters[0].call_type))?;
            Ok(self.model.has_type_flags(parameter, TypeFlags::ANY)
                || sequence.read(self.array_element_type_completion(parameter))?
                    == Some(self.model.special(SpecialName::Any)))
        })();
        sequence.finish(result)
    }

    /// `findMixins` performs the full mixin classification pass before counting
    /// constructors. When all constructors are mixins, the first is retained.
    pub(in crate::project::semantic::evaluation) fn find_mixins(
        &mut self,
        types: &[TypeId],
        consumer: &Location,
    ) -> Result<(Vec<bool>, usize), DemandError> {
        self.find_mixins_completion(types, consumer)?.admit()
    }

    pub(in crate::project::semantic::evaluation) fn find_mixins_completion(
        &mut self,
        types: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<(Vec<bool>, usize)> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(Vec<bool>, usize), DemandError> {
            let mut flags = Vec::with_capacity(types.len());
            for &ty in types {
                flags.push(sequence.read(self.is_mixin_constructor_type_completion(ty, consumer))?);
            }
            let mut constructors = 0;
            let mut mixins = 0;
            let mut first = None;
            for (index, &ty) in types.iter().enumerate() {
                if !sequence
                    .read(self.construct_signatures_completion(ty, consumer))?
                    .is_empty()
                {
                    constructors += 1;
                }
                if flags[index] {
                    first.get_or_insert(index);
                    mixins += 1;
                }
            }
            if constructors > 0 && constructors == mixins {
                flags[first.ok_or_else(|| {
                    super::super::super::internal("mixin count lacks first constructor")
                })?] = false;
                mixins -= 1;
            }
            Ok((flags, mixins))
        })();
        sequence.finish(result)
    }

    fn intersection_members_completion(
        &mut self,
        types: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<MembersView> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<MembersView, DemandError> {
            let (mixins, mixin_count) =
                sequence.read(self.find_mixins_completion(types, consumer))?;
            let mut calls = Vec::new();
            let mut constructs = Vec::new();
            let mut indexes = Vec::new();
            for (index, &ty) in types.iter().enumerate() {
                if !mixins[index] {
                    let mut signatures =
                        sequence.read(self.construct_signatures_completion(ty, consumer))?;
                    if !signatures.is_empty() && mixin_count > 0 {
                        for signature in &mut signatures {
                            let mut clone = self.clone_signature(signature, consumer)?;
                            let return_type = sequence
                                .read(self.raw_return_type_of_signature_completion(signature))?;
                            let mut mixed = Vec::new();
                            for (other_index, &other) in types.iter().enumerate() {
                                if other_index == index {
                                    mixed.push(return_type);
                                } else if mixins[other_index] {
                                    let signatures = sequence.read(
                                        self.construct_signatures_completion(other, consumer),
                                    )?;
                                    mixed.push(sequence.read(
                                        self.raw_return_type_of_signature_completion(
                                            &signatures[0],
                                        ),
                                    )?);
                                }
                            }
                            let return_type = sequence
                                .read(self.intersection_type_completion(mixed, consumer))?;
                            self.install_signature_return(&mut clone, return_type, consumer)?;
                            *signature = clone;
                        }
                    }
                    sequence.read(self.append_signatures_completion(
                        &mut constructs,
                        &signatures,
                        consumer,
                    ))?;
                }
                let signatures = sequence.read(self.call_signatures_completion(ty, consumer))?;
                sequence.read(self.append_signatures_completion(
                    &mut calls,
                    &signatures,
                    consumer,
                ))?;
                for info in sequence.read(self.index_infos_of_type_completion(ty, consumer))? {
                    sequence.read(self.append_index_info_completion(
                        &mut indexes,
                        info,
                        false,
                        consumer,
                    ))?;
                }
            }
            Ok(MembersView {
                properties: Vec::new().into(),
                call_signatures: calls.into(),
                construct_signatures: constructs.into(),
                index_signatures: indexes.into(),
            })
        })();
        sequence.finish(result)
    }

    /// Actual Type.symbol.Parent, including checker-created methods. The
    /// nil parent gate is cheaper than acquiring either global array identity.
    fn array_member_parent(&self, ty: TypeId) -> Result<Option<SymbolParent>, DemandError> {
        match self.model.type_symbol(ty) {
            TypeSymbol::Bound { symbol, .. } => Ok(self
                .source
                .bindings()
                .property_parent(symbol)
                .map(SymbolParent::Bound)),
            TypeSymbol::Property { symbol, .. } => self.property_parent(symbol),
            TypeSymbol::Absent | TypeSymbol::Synthetic { .. } => Ok(None),
        }
    }

    /// `isArrayOrTupleSymbol` / `isReadonlyArraySymbol` compare the actual
    /// merged member parent with canonical globals. Admitted parent containers
    /// cannot be non-local aliases, so resolveSymbol is an identity operation.
    fn array_member_parent_kind_completion(
        &mut self,
        parent: SymbolParent,
        consumer: &Location,
    ) -> NativeResult<Option<(bool, TypeId)>> {
        let array = self.fixed_global_type_identity(FixedGlobalType::Array, consumer)?;
        let readonly = self.fixed_global_type_identity(FixedGlobalType::ReadonlyArray, consumer)?;
        let (Some((array, array_type)), Some((readonly, readonly_type))) = (array, readonly) else {
            return Ok(NativeCompletion::clean(None));
        };
        let parent = self.reference_module_parent_symbol(parent);
        let parent = self.reference_symbol_identity(parent, consumer)?;
        let array_identity = self.reference_symbol_identity(array, consumer)?;
        let readonly_identity = self.reference_symbol_identity(readonly, consumer)?;
        if parent != array_identity && parent != readonly_identity {
            return Ok(NativeCompletion::clean(None));
        }
        let is_readonly = parent == readonly_identity;
        Ok(NativeCompletion::clean(Some((
            is_readonly,
            if is_readonly {
                readonly_type
            } else {
                array_type
            },
        ))))
    }

    /// `getArrayMemberCallSignatures` validates the complete constituent list
    /// before mapping element arguments through the actual anonymous mappers.
    fn array_member_call_signatures_completion(
        &mut self,
        union: TypeId,
        types: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<Vec<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Signature>, DemandError> {
            let mut member_name: Option<crate::project::binding::NativePropertyKey> = None;
            for &ty in types {
                if !self
                    .model
                    .object_flags(ty)?
                    .intersects(ObjectFlags::INSTANTIATED)
                {
                    return Ok(Vec::new());
                }
                let Some(parent) = self.array_member_parent(ty)? else {
                    return Ok(Vec::new());
                };
                if sequence
                    .read(self.array_member_parent_kind_completion(parent, consumer))?
                    .is_none()
                {
                    return Ok(Vec::new());
                }
                let symbol = self.model.type_symbol(ty);
                let name = symbol
                    .native()?
                    .ok_or_else(|| {
                        super::super::super::internal("array member parent exists without symbol")
                    })?
                    .key()
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                if let Some(previous) = &member_name {
                    if previous != &name {
                        return Ok(Vec::new());
                    }
                } else {
                    member_name = Some(name);
                }
            }
            let element =
                sequence.read(self.map_type_completion(union, consumer, |this, ty| {
                    let parent = this.array_member_parent(ty)?.ok_or_else(|| {
                        super::super::super::internal("established array member parent disappeared")
                    })?;
                    this.array_member_parent_kind_completion(parent, consumer)?
                        .and_then(|kind| {
                            let (_, target) = kind.ok_or_else(|| {
                                super::super::super::internal(
                                    "established array parent identity disappeared",
                                )
                            })?;
                            let target = this.model.declared_target(target)?;
                            let parameter =
                                target.local_parameters.first().copied().ok_or_else(|| {
                                    super::super::super::internal(
                                        "global array lacks its established parameter",
                                    )
                                })?;
                            let mapper = this.anonymous_type_mapper(ty)?;
                            this.map_parameter_completion(parameter, mapper, consumer)
                        })
                }))?;
            let mut readonly = false;
            for &ty in types {
                let parent = self.array_member_parent(ty)?.ok_or_else(|| {
                    super::super::super::internal("established array member parent disappeared")
                })?;
                if sequence
                    .read(self.array_member_parent_kind_completion(parent, consumer))?
                    .ok_or_else(|| {
                        super::super::super::internal("established array identity disappeared")
                    })?
                    .0
                {
                    readonly = true;
                    break;
                }
            }
            let array = if readonly {
                sequence.read(self.readonly_array_type_completion(element, consumer))?
            } else {
                sequence.read(self.array_type_completion(element, consumer))?
            };
            let name = member_name.ok_or_else(|| {
                super::super::super::internal("array member union has no constituents")
            })?;
            match sequence
                .read(self.type_of_property_of_type_by_key_completion(array, &name, consumer))?
            {
                Some(member) => sequence.read(self.call_signatures_completion(member, consumer)),
                None => Ok(Vec::new()),
            }
        })();
        sequence.finish(result)
    }
}
