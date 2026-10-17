//! Native key collection and deferred Index types share one operation. The
//! property cache captures member-resolution state before asking inventories.

use super::super::model::{
    DemandError, IndexFlags, NativeResult, NativeSequence, ObjectFlags, SpecialName, TypeFlags,
    TypeId, TypeView,
};
use super::super::source::TypeExpression;
use super::Evaluator;
use crate::analysis::SemanticFailureReason;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::types::PrimitiveName;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PropertiesTypesKey {
    ty: TypeId,
    include: TypeFlags,
    include_origin: bool,
    unresolved_members: bool,
}
// One type index, fixed-width flags and two tags, never the property's inventory.

pub(super) struct KeyofState {
    nodes: crate::resources::FixedMap<(FileId, Site), NativeResult<TypeId>>,
    properties: crate::resources::FixedMap<PropertiesTypesKey, NativeResult<TypeId>>,
}

impl Evaluator<'_, '_> {
    /// `getKnownKeysOfTupleType` collects fixed numeric property names before
    /// asking the actual Array/ReadonlyArray target's shared index operation.
    pub(super) fn known_keys_of_tuple_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let tuple = self.model.tuple_target(ty)?.ok_or_else(|| {
                super::super::internal("known tuple keys require an actual tuple target")
            })?;
            let mut keys = Vec::with_capacity(tuple.fixed_length + 1);
            for index in 0..tuple.fixed_length {
                keys.push(self.model.literal(PrimitiveName::String, index.to_string()));
            }
            let target = sequence.read(self.array_target_completion(tuple.readonly, consumer))?;
            keys.push(sequence.read(self.index_type_ex_completion(
                target,
                IndexFlags::NONE,
                consumer,
            ))?);
            sequence.read(self.union_type_ex_completion(keys, None, None, consumer))
        })();
        sequence.finish(result)
    }

    /// `getTypeFromTypeOperatorNode` stores the result of the complete keyof
    /// question; repeated node reads do not reacquire its operand.
    pub(super) fn keyof_type_node_completion(
        &mut self,
        expression: &TypeExpression,
        site: Site,
        value: &TypeExpression,
    ) -> NativeResult<TypeId> {
        let key = (expression.file, site);
        if let Some(result) = self.keyof.nodes.get(&key) {
            return result.clone();
        }
        let result = self.check_type_completion(value).and_then(|value| {
            value.and_then(|ty| {
                self.index_type_ex_completion(ty, IndexFlags::NONE, &expression.location)
            })
        });
        self.keyof.nodes.insert(key, result.clone());
        result
    }

    /// `getIndexTypeEx` retains native branch order after reduction. Mapped
    /// and substitution types require their own producer's semantic payload.
    pub(super) fn index_type_ex(
        &mut self,
        ty: TypeId,
        flags: IndexFlags,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.index_type_ex_completion(ty, flags, consumer)?.admit()
    }

    pub(super) fn index_type_ex_completion(
        &mut self,
        ty: TypeId,
        flags: IndexFlags,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let ty = sequence.read(self.reduced_type_completion(ty, consumer))?;
            if self.model.is_no_infer_type(ty) {
                let TypeView::Substitution(view) = self.model.get(ty)? else {
                    return Err(super::super::internal(
                        "NoInfer type lacks its substitution header",
                    )
                    .into());
                };
                let keys =
                    sequence.read(self.index_type_ex_completion(view.base, flags, consumer))?;
                return sequence.read(self.no_infer_type_completion(keys, consumer));
            }
            if sequence.read(self.should_defer_index_type_completion(ty, flags, consumer))? {
                return Ok(self.model.index_type_for_generic(ty, flags));
            }
            if let Some(types) = self.model.union_constituents(ty) {
                let mut keys = Vec::with_capacity(types.len());
                for &ty in types.iter() {
                    keys.push(sequence.read(self.index_type_ex_completion(ty, flags, consumer))?);
                }
                return sequence.read(self.intersection_type_completion(keys, consumer));
            }
            if let Some(types) = self.model.intersection_constituents(ty) {
                let mut keys = Vec::with_capacity(types.len());
                for &ty in types.iter() {
                    keys.push(sequence.read(self.index_type_ex_completion(ty, flags, consumer))?);
                }
                return sequence.read(self.union_type_ex_completion(keys, None, None, consumer));
            }
            if self.model.object_flags(ty)?.intersects(ObjectFlags::MAPPED) {
                return sequence.read(self.mapped_index_type_completion(ty, flags, consumer));
            }
            if self.model.is_wildcard_type(ty) {
                return Ok(self.model.wildcard_type());
            }
            if self.model.has_type_flags(ty, TypeFlags::UNKNOWN) {
                return Ok(self.model.special(SpecialName::Never));
            }
            if self
                .model
                .has_type_flags(ty, TypeFlags::ANY | TypeFlags::NEVER)
            {
                return Ok(self.model.string_number_symbol_type());
            }
            let include = if flags.contains(IndexFlags::NO_INDEX_SIGNATURES) {
                TypeFlags::STRING_LITERAL
            } else {
                TypeFlags::STRING_LIKE
            } | if flags.contains(IndexFlags::STRINGS_ONLY) {
                TypeFlags::NONE
            } else {
                TypeFlags::NUMBER_LIKE | TypeFlags::ES_SYMBOL_LIKE
            };
            sequence.read(self.literal_type_from_properties_completion(
                ty,
                include,
                flags == IndexFlags::NONE,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `shouldDeferIndexType` is shared by keyof and indexed-access
    /// simplification. NoReducibleCheck suppresses only the union question.
    pub(super) fn should_defer_index_type_completion(
        &mut self,
        ty: TypeId,
        flags: IndexFlags,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self
                .model
                .has_type_flags(ty, TypeFlags::INSTANTIABLE_NON_PRIMITIVE)
                || self.model.is_generic_tuple_type(ty)?
            {
                return Ok(true);
            }
            if sequence.read(self.is_generic_mapped_type_completion(ty, consumer))?
                && sequence
                    .read(self.mapped_name_completion(ty, consumer))?
                    .is_some()
            {
                return Ok(true);
            }
            if self.model.has_type_flags(ty, TypeFlags::UNION)
                && !flags.contains(IndexFlags::NO_REDUCIBLE_CHECK)
                && sequence.read(self.is_generic_reducible_type_completion(ty, consumer))?
            {
                return Ok(true);
            }
            if self.model.has_type_flags(ty, TypeFlags::INTERSECTION)
                && self.model.maybe_type_of_kind(ty, TypeFlags::INSTANTIABLE)
            {
                for &ty in self
                    .model
                    .intersection_constituents(ty)
                    .expect("Intersection flag has constituents")
                    .iter()
                {
                    if sequence
                        .read(self.is_empty_anonymous_object_type_completion(ty, consumer))?
                    {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `getLiteralTypeFromProperties` allocates an origin before inventories,
    /// then collects property keys before index keys. The alias condition is
    /// deliberately independent of includeOrigin, matching native precedence.
    pub(super) fn literal_type_from_properties_completion(
        &mut self,
        ty: TypeId,
        include: TypeFlags,
        include_origin: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let object_flags = self.model.object_flags(ty)?;
        let key = PropertiesTypesKey {
            ty,
            include,
            include_origin,
            unresolved_members: object_flags.intersects(ObjectFlags::UNRESOLVED_MEMBERS),
        };
        if let Some(result) = self.keyof.properties.get(&key) {
            return result.clone();
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let origin = if include_origin
                && object_flags.intersects(
                    ObjectFlags::CLASS | ObjectFlags::INTERFACE | ObjectFlags::REFERENCE,
                )
                || self.model.metadata(ty)?.alias.is_some()
            {
                Some(self.model.new_index_type(ty, IndexFlags::NONE))
            } else {
                None
            };
            let properties = sequence.read(self.properties_of_type_completion(ty, consumer))?;
            let indices = sequence.read(self.index_infos_of_type_completion(ty, consumer))?;
            let mut types = Vec::with_capacity(properties.len() + indices.len());
            for property in properties {
                let symbol = property.symbol.ok_or_else(|| {
                    self.keyof_dependency(
                        consumer,
                        "native property key has no actual symbol association",
                    )
                })?;
                types.push(sequence.read(
                    self.literal_type_from_property_completion(symbol, include, false, consumer),
                )?);
            }
            for index in indices {
                if index.same_identity(&self.model.enum_number_index()) {
                    continue;
                }
                let key_type = sequence.read(self.demand_completion(index.key_type))?;
                if self.is_key_type_included(key_type, include) {
                    if key_type == self.model.primitive(PrimitiveName::String)
                        && include.intersects(TypeFlags::NUMBER)
                    {
                        types.push(self.model.string_or_number_type());
                    } else {
                        types.push(key_type);
                    }
                }
            }
            sequence.read(self.union_type_ex_completion(types, None, origin, consumer))
        })();
        let result = sequence.finish(result);
        self.keyof.properties.insert(key, result.clone());
        result
    }

    /// `isKeyTypeIncluded` recursively observes intersection components only.
    fn is_key_type_included(&self, ty: TypeId, include: TypeFlags) -> bool {
        self.model.has_type_flags(ty, include)
            || self
                .model
                .intersection_constituents(ty)
                .is_some_and(|types| {
                    types
                        .iter()
                        .any(|&ty| self.is_key_type_included(ty, include))
                })
    }

    fn keyof_dependency(&self, consumer: &Location, message: &str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }
}

impl KeyofState {
    /// Request-local backing for the existing native cache questions. Their
    /// identities, child values and provisional publication stay operation-owned.
    pub(super) fn new() -> Self {
        Self {
            nodes: crate::resources::FixedMap::new(),
            properties: crate::resources::FixedMap::new(),
        }
    }
}
