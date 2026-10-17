//! Union construction owns semantic reductions and their demand order. The
//! model resumes its pure factory only with established dependency answers.

use super::super::model::{
    Alias, AliasKey, DemandError, NativeCompletion, NativeResult, NativeSequence, ObjectFlags,
    SpecialName, TypeFlags, TypeId, TypeView, UnionFailure, UnionReduction,
};
use super::super::model::{KeyBuilder, TypeListCache, same_alias};
use super::Evaluator;
use super::relations::RelationKind;
use crate::analysis::SemanticFailureReason;
use crate::references::Location;
use crate::resources::{CheckedKey, CheckedMap, Control};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct UnionsState {
    pairs: CheckedMap<UnionPair, NativeResult<TypeId>>,
    subtypes: TypeListCache<NativeResult<Option<Arc<[TypeId]>>>>,
    literal_bases: HashMap<TypeId, NativeCompletion<TypeId>>,
}

/// `UnionOfUnionKey`: canonical input identities, reduction mode and actual alias.
struct UnionPair {
    left: TypeId,
    right: TypeId,
    reduction: UnionReduction,
    alias: Option<AliasKey>,
    fingerprint: u64,
}

impl UnionsState {
    pub(super) fn new(control: Option<Control>) -> Self {
        Self {
            pairs: CheckedMap::new(),
            subtypes: TypeListCache::new(control),
            ..Self::default()
        }
    }
}
impl UnionPair {
    fn new(left: TypeId, right: TypeId, reduction: UnionReduction, alias: Option<&Alias>) -> Self {
        let (left, right) = (left.min(right), left.max(right));
        let alias = AliasKey::of(alias);

        let mut hash = KeyBuilder::new();
        hash.fixed(&left);
        hash.fixed(&right);
        hash.fixed(&reduction);
        hash.alias(alias.as_ref());
        Self {
            left,
            right,
            reduction,
            alias,
            fingerprint: hash.finish(),
        }
    }
}
impl CheckedKey for UnionPair {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        self.left == other.left
            && self.right == other.right
            && self.reduction == other.reduction
            && same_alias(self.alias.as_ref(), other.alias.as_ref())
    }
}

impl Evaluator<'_, '_> {
    pub(super) fn union_type_ex(
        &mut self,
        types: Vec<TypeId>,
        alias: Option<Alias>,
        origin: Option<TypeId>,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.union_type_ex_completion(types, alias, origin, consumer)?
            .admit()
    }

    pub(super) fn union_type_ex_completion(
        &mut self,
        types: Vec<TypeId>,
        alias: Option<Alias>,
        origin: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = sequence.read(self.union_type_reduced_completion(
            types,
            UnionReduction::Literal,
            alias,
            origin,
            consumer,
        ));
        sequence.finish(result)
    }

    /// `getUnionTypeEx` / `getUnionTypeWorker` acquire constrained-variable
    /// bases and then subtype reduction. Factory resumption repeats only pure
    /// established metadata operations; consumed semantic work is never retried.
    pub(super) fn union_type_reduced(
        &mut self,
        types: Vec<TypeId>,
        reduction: UnionReduction,
        alias: Option<Alias>,
        origin: Option<TypeId>,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.union_type_reduced_completion(types, reduction, alias, origin, consumer)?
            .admit()
    }

    pub(super) fn union_type_reduced_completion(
        &mut self,
        types: Vec<TypeId>,
        reduction: UnionReduction,
        alias: Option<Alias>,
        origin: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        if types.is_empty() {
            return Ok(NativeCompletion::clean(
                self.model.special(SpecialName::Never),
            ));
        }
        if types.len() == 1 {
            return Ok(NativeCompletion::clean(types[0]));
        }
        let key = (types.len() == 2
            && origin.is_none()
            && (self.model.has_type_flags(types[0], TypeFlags::UNION)
                || self.model.has_type_flags(types[1], TypeFlags::UNION)))
        .then(|| UnionPair::new(types[0], types[1], reduction, alias.as_ref()));
        if let Some(result) = key.as_ref().and_then(|key| self.unions.pairs.get(key)) {
            return result.clone();
        }
        let result = self.union_type_worker_completion(types, reduction, alias, origin, consumer);
        if let Some(key) = key {
            self.unions.pairs.insert(key, result.clone());
        }
        result
    }

    fn union_type_worker_completion(
        &mut self,
        types: Vec<TypeId>,
        reduction: UnionReduction,
        alias: Option<Alias>,
        origin: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let mut constraints = HashMap::new();
            let mut subtypes = TypeListCache::new(self.control.clone());
            let mut strings = TypeListCache::new(self.control.clone());
            loop {
                match self.model.union_with_reductions(
                    &types,
                    reduction,
                    alias.clone(),
                    origin,
                    &constraints,
                    &subtypes,
                    &strings,
                ) {
                    Ok(ty) => return Ok(ty),
                    Err(UnionFailure::StringLiteralReduction(types)) => {
                        let reduced = sequence.read(
                            self.remove_string_literals_matched_by_templates_completion(
                                self.model.copy_type_list(&types),
                                consumer,
                            ),
                        )?;
                        strings.insert(&types, reduced);
                    }
                    Err(UnionFailure::Constraint(variable)) => {
                        let constraint = sequence.read(self.base_constraint_completion(variable, consumer))?.ok_or_else(|| super::super::internal("native constrained intersection lost its established base constraint"))?;
                        constraints.reserve(1);
                        constraints.insert(variable, constraint);
                    }
                    Err(UnionFailure::SubtypeReduction {
                        types,
                        has_object_types,
                    }) => {
                        let reduced = sequence.read(self.remove_subtypes_completion(
                            &types,
                            has_object_types,
                            consumer,
                        ))?;
                        let Some(reduced) = reduced else {
                            return Ok(self.model.native_error_type());
                        };
                        subtypes.insert(&types, reduced);
                    }
                    Err(failure) => return Err(failure.at(self.inputs, consumer)),
                }
            }
        })();
        sequence.finish(result)
    }

    /// `removeStringLiteralsMatchedByTemplateLiterals` captures pattern types
    /// before scanning literals backward. Each literal short-circuits against
    /// patterns in native canonical order, before constrained-type reductions.
    fn remove_string_literals_matched_by_templates_completion(
        &mut self,
        mut types: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<TypeId>, DemandError> {
            let mut patterns = Vec::new();
            for &ty in &types {
                if self.is_pattern_literal_type(ty)? {
                    patterns.push(ty);
                }
            }
            if patterns.is_empty() {
                return Ok(types);
            }
            for index in (0..types.len()).rev() {
                let ty = types[index];
                if !self.model.has_type_flags(ty, TypeFlags::STRING_LITERAL) {
                    continue;
                }
                for &pattern in &patterns {
                    let matches = match self.model.get(pattern)? {
                        TypeView::TemplateLiteral(template) => {
                            sequence.read(self.template_type_matches_completion(
                                ty,
                                &template,
                                consumer,
                                &mut |checker, source, target| {
                                    checker.raw_type_related_completion(
                                        source,
                                        target,
                                        RelationKind::Assignable,
                                        consumer,
                                    )
                                },
                            ))?
                        }
                        TypeView::StringMapping(_) => sequence.read(
                            self.member_of_string_mapping_completion(ty, pattern, consumer),
                        )?,
                        _ => {
                            return Err(super::super::internal(
                                "pattern factory lacks literal view",
                            )
                            .into());
                        }
                    };
                    if matches {
                        types.remove(index);
                        break;
                    }
                }
            }
            Ok(types)
        })();
        sequence.finish(result)
    }

    /// `removeSubtypes`: the cache key is the original ordered constituent
    /// list, independent from the later removals or the calling expression.
    fn remove_subtypes_completion(
        &mut self,
        types: &[TypeId],
        has_object_types: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Arc<[TypeId]>>> {
        if types.len() < 2 {
            return Ok(NativeCompletion::clean(Some(std::sync::Arc::from(
                self.model.copy_type_list(types),
            ))));
        }
        if let Some(result) = self.unions.subtypes.get(types) {
            return result.clone();
        }
        let input = self.model.copy_type_list(types);
        let result = self
            .remove_subtypes_worker_completion(input, has_object_types, consumer)
            .map(|value| value.map(|types| types.map(std::sync::Arc::from)));
        // Native nil after complexity recovery is not installed in subtypeReductionCache.
        if !result.as_ref().is_ok_and(|value| value.value().is_none()) {
            self.unions.subtypes.insert(types, result.clone());
        }
        result
    }

    fn remove_subtypes_worker_completion(
        &mut self,
        mut types: Vec<TypeId>,
        has_object_types: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Vec<TypeId>>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Vec<TypeId>>, DemandError> {
            let mut has_empty_object = false;
            if has_object_types {
                for &ty in &types {
                    if self.model.has_type_flags(ty, TypeFlags::OBJECT)
                        && !self.is_generic_mapped_type(ty, consumer)?
                    {
                        let members =
                            self.structured_type_members(ty, consumer)?.ok_or_else(|| {
                                super::super::internal(
                                    "object subtype reduction has no structured inventory",
                                )
                            })?;
                        if self.model.is_empty_resolved_type(ty, &members) {
                            has_empty_object = true;
                            break;
                        }
                    }
                }
            }
            let length = types.len();
            let mut index = length;
            let mut count = 0usize;
            while index != 0 {
                index -= 1;
                let source = types[index];
                if !has_empty_object
                    && !self
                        .model
                        .has_type_flags(source, TypeFlags::STRUCTURED_OR_INSTANTIABLE)
                {
                    continue;
                }
                if self.model.has_type_flags(source, TypeFlags::TYPE_PARAMETER) {
                    let constraint = sequence
                        .read(self.base_constraint_completion(source, consumer))?
                        .unwrap_or(source);
                    if self.model.has_type_flags(constraint, TypeFlags::UNION) {
                        let never = self.model.special(SpecialName::Never);
                        let others = types
                            .iter()
                            .map(|&ty| if ty == source { never } else { ty })
                            .collect();
                        let target = sequence
                            .read(self.union_type_ex_completion(others, None, None, consumer))?;
                        if sequence.read(self.raw_type_related_completion(
                            source,
                            target,
                            RelationKind::StrictSubtype,
                            consumer,
                        ))? {
                            types.remove(index);
                        }
                        continue;
                    }
                }
                let mut key = None;
                let structured_key = TypeFlags::OBJECT
                    | TypeFlags::INTERSECTION
                    | TypeFlags::INSTANTIABLE_NON_PRIMITIVE;
                if self.model.has_type_flags(source, structured_key) {
                    for property in self.properties_of_type(source, consumer)? {
                        let symbol = self.property_symbol(&property, consumer)?;
                        let ty =
                            sequence.read(self.property_raw_type_completion(symbol, consumer))?;
                        if self.model.has_type_flags(ty, TypeFlags::UNIT) {
                            // Native asks the same symbol again after Find returns;
                            // the value owner supplies its established cache entry.
                            let ty = sequence
                                .read(self.property_raw_type_completion(symbol, consumer))?;
                            let name = self.property_key(&property, consumer)?;
                            key = Some((name, self.model.regular_literal(ty)));
                            break;
                        }
                    }
                }
                for &target in &types {
                    if source == target {
                        continue;
                    }
                    if count == 100_000 {
                        let estimated = (count / (length - index)).saturating_mul(length);
                        if estimated > 1_000_000 {
                            return sequence.read(NativeCompletion::recovered(None, super::super::invalid_at_location(
                            self.inputs,
                            consumer,
                            SemanticFailureReason::SubtypeComplexityLimit,
                            "expression produces a union type that is too complex to represent",
                        )
                        .into()));
                        }
                    }
                    count += 1;
                    if let Some((name, key_type)) = &key {
                        if self.model.has_type_flags(target, structured_key) {
                            if let Some(ty) =
                                sequence.read(self.type_of_property_of_type_by_key_completion(
                                    target, name, consumer,
                                ))?
                            {
                                if self.model.has_type_flags(ty, TypeFlags::UNIT)
                                    && self.model.regular_literal(ty) != *key_type
                                {
                                    continue;
                                }
                            }
                        }
                    }
                    if (source == self.model.empty_object_type()
                        || source == self.model.unknown_empty_object_type())
                        && self.model.has_type_symbol(target)?
                        && self.is_empty_anonymous_object_type(target, consumer)?
                    {
                        continue;
                    }
                    if sequence.read(self.raw_type_related_completion(
                        source,
                        target,
                        RelationKind::StrictSubtype,
                        consumer,
                    ))? {
                        let source_target = self.model.reference_target(source).unwrap_or(source);
                        let target_target = self.model.reference_target(target).unwrap_or(target);
                        if !self
                            .model
                            .object_flags(source_target)?
                            .intersects(ObjectFlags::CLASS)
                            || !self
                                .model
                                .object_flags(target_target)?
                                .intersects(ObjectFlags::CLASS)
                            || sequence.read(
                                self.is_type_derived_from_completion(source, target, consumer),
                            )?
                        {
                            types.remove(index);
                            break;
                        }
                    }
                }
            }
            Ok(Some(types))
        })();
        sequence.finish(result)
    }

    /// `isTypeDerivedFrom` compares the actual inheritance hierarchy and its
    /// native global/empty-object exceptions. Structural subtyping alone does
    /// not remove two unrelated class identities from a union.
    pub(super) fn is_type_derived_from_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if let Some(types) = self.model.union_constituents(source) {
                for &ty in types.iter() {
                    if !sequence.read(self.is_type_derived_from_completion(ty, target, consumer))? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            if let Some(types) = self.model.union_constituents(target) {
                for &ty in types.iter() {
                    if sequence.read(self.is_type_derived_from_completion(source, ty, consumer))? {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            if let Some(types) = self.model.intersection_constituents(source) {
                for &ty in types.iter() {
                    if sequence.read(self.is_type_derived_from_completion(ty, target, consumer))? {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            if self
                .model
                .has_type_flags(source, TypeFlags::INSTANTIABLE_NON_PRIMITIVE)
            {
                let base = sequence
                    .read(self.base_constraint_completion(source, consumer))?
                    .unwrap_or_else(|| self.model.special(SpecialName::Unknown));
                return sequence.read(self.is_type_derived_from_completion(base, target, consumer));
            }
            let object_like = self
                .model
                .has_type_flags(source, TypeFlags::OBJECT | TypeFlags::NON_PRIMITIVE);
            if self.is_empty_anonymous_object_type(target, consumer)? {
                return Ok(object_like);
            }
            if target == self.global_object_identity(consumer)? {
                return Ok(object_like && !self.is_empty_anonymous_object_type(source, consumer)?);
            }
            if target == self.global_object_identity_named("Function", consumer)? {
                return Ok(self.model.has_type_flags(source, TypeFlags::OBJECT)
                    && self.is_function_object_type(source, consumer)?);
            }
            let target_target = self.model.reference_target(target).unwrap_or(target);
            if self.has_base_type(source, Some(target_target), consumer)? {
                return Ok(true);
            }
            if self.is_array_type(target)? && !self.is_readonly_array_type(target)? {
                let readonly = self.array_target(true, consumer)?;
                return sequence
                    .read(self.is_type_derived_from_completion(source, readonly, consumer));
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `getBaseTypeOfLiteralType` / `getBaseTypeOfLiteralTypeUnion` widen both
    /// fresh and regular literals. Changed unions use this complete union
    /// owner; unchanged unions retain their native identity.
    pub(super) fn base_type_of_literal(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.base_type_of_literal_completion(ty, consumer)?.admit()
    }

    pub(super) fn base_type_of_literal_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self.model.has_type_flags(ty, TypeFlags::ENUM_LIKE) {
            return self.base_type_of_enum_like_completion(ty, consumer);
        }
        match self.model.get(ty)? {
            TypeView::Literal { base, .. } => {
                return Ok(NativeCompletion::clean(self.model.primitive(base)));
            }
            TypeView::TemplateLiteral(_) | TypeView::StringMapping(_) => {
                return Ok(NativeCompletion::clean(
                    self.model.primitive(crate::types::PrimitiveName::String),
                ));
            }
            TypeView::Union(_) => {}
            _ => return Ok(NativeCompletion::clean(ty)),
        }
        // Only the native union operation owns a cachedTypes entry. Scalar
        // conversion cannot install evidence from a different operation.
        if let Some(result) = self.unions.literal_bases.get(&ty) {
            return Ok(result.clone());
        }
        let result = self.map_type_completion(ty, consumer, |checker, child| {
            checker.base_type_of_literal_completion(child, consumer)
        });
        if let Ok(completed) = &result {
            self.unions.literal_bases.insert(ty, completed.clone());
        }
        result
    }

    /// `getWidenedLiteralType`: fresh enum members widen through their actual
    /// parent declared type; regular members retain identity. Union traversal
    /// uses the shared map owner and consumes each actual parent acquisition.
    pub(super) fn widened_literal_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self.model.has_type_flags(ty, TypeFlags::ENUM_LIKE) && self.model.is_fresh_literal(ty) {
            self.base_type_of_enum_like_completion(ty, consumer)
        } else if self.model.has_type_flags(ty, TypeFlags::UNION) {
            self.map_type_completion(ty, consumer, |checker, child| {
                checker.widened_literal_type_completion(child, consumer)
            })
        } else {
            Ok(NativeCompletion::clean(self.model.widened_literal(ty)))
        }
    }

    /// `getCrossProductUnionSize` / `checkCrossProductUnion`: Never empties
    /// the product only when reached before signed machine-word overflow.
    /// Native returns MaxInt immediately on overflow, so a later Never must
    /// not suppress the complexity failure.
    pub(super) fn check_cross_product_union_completion(
        &self,
        types: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<Option<usize>> {
        let mut count = 1usize;
        for &ty in types {
            if let Some(types) = self.model.union_constituents(ty) {
                let maximum = isize::MAX as usize;
                if !types.is_empty() && count > maximum / types.len() {
                    count = maximum;
                    break;
                }
                count *= types.len();
            } else if self.model.has_type_flags(ty, TypeFlags::NEVER) {
                return Ok(NativeCompletion::clean(Some(0)));
            }
        }
        if count >= 100_000 {
            return NativeCompletion::recovered(
                None,
                super::super::invalid_at_location(
                    self.inputs,
                    consumer,
                    SemanticFailureReason::UnionComplexityLimit,
                    "expression produces a union type that is too complex to represent",
                )
                .into(),
            );
        }
        Ok(NativeCompletion::clean(Some(count)))
    }
}
