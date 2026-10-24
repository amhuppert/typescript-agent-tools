//! Native intersection normalization. Syntax, normalization and factory cache
//! are distinct questions: only the type-node cache skips operand acquisition.

use super::super::model::{
    Alias, DemandError, IntersectionFlags, NativeResult, NativeSequence, ObjectFlags, SpecialName,
    TypeFlags, TypeId, UnionReduction,
};
use super::super::source::{AliasRecipe, TypeExpression};
use super::Evaluator;
use super::relations::RelationKind;
use crate::analysis::SemanticFailureReason;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::resources::complete_or_abort;
use crate::types::PrimitiveName;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct IntersectionsState {
    nodes: HashMap<(FileId, Site), NativeResult<TypeId>>,
}

impl Evaluator<'_, '_> {
    /// `getOptionalType`: Missing is the exact-optional property marker;
    /// the native first-constituent shortcut precedes the full union owner.
    pub(super) fn optional_type(
        &mut self,
        ty: TypeId,
        is_property: bool,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.optional_type_completion(ty, is_property, consumer)?
            .admit()
    }

    pub(super) fn optional_type_completion(
        &mut self,
        ty: TypeId,
        is_property: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if !self.inputs.semantic_options().strict_null_checks {
                return Err(super::super::unsupported_demand_at_location(
                    self.inputs,
                    consumer,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "native getOptionalType requires strictNullChecks; this loose-mode request reaches its compiler invariant",
                ));
            }
            let undefined =
                if is_property && self.inputs.semantic_options().exact_optional_property_types {
                    self.model.missing_type()
                } else {
                    self.model.special(SpecialName::Undefined)
                };
            if ty == undefined
                || self
                    .model
                    .union_constituents(ty)
                    .is_some_and(|types| types.first() == Some(&undefined))
            {
                return Ok(ty);
            }
            sequence.read(self.union_type_ex_completion(
                self.model.copy_type_list(&[ty, undefined]),
                None,
                None,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `addOptionalityEx` retains the caller's actual property/optional facts
    /// and skips the optional-type question entirely in loose-null mode.
    pub(super) fn add_optionality(
        &mut self,
        ty: TypeId,
        is_property: bool,
        optional: bool,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.add_optionality_completion(ty, is_property, optional, consumer)?
            .admit()
    }

    pub(super) fn add_optionality_completion(
        &mut self,
        ty: TypeId,
        is_property: bool,
        optional: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = if self.inputs.semantic_options().strict_null_checks && optional {
            sequence.read(self.optional_type_completion(ty, is_property, consumer))
        } else {
            Ok(ty)
        };
        sequence.finish(result)
    }

    /// `getTypeFromIntersectionTypeNode`: alias, ordered operands, the exact
    /// emptyTypeLiteralType flag, then getIntersectionTypeEx. The captured
    /// full Site key distinguishes nested nodes sharing their first token.
    pub(super) fn intersection_type_node_completion(
        &mut self,
        expression: &TypeExpression,
        site: Site,
        operands: &[TypeExpression],
        alias: Option<&AliasRecipe>,
    ) -> NativeResult<TypeId> {
        let key = (expression.file, site);
        if let Some(result) = self.intersections.nodes.get(&key) {
            return result.clone();
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let alias = alias
                .map(|recipe| sequence.read(self.establish_alias_completion(recipe)))
                .transpose()?;
            let types = sequence.read(self.type_arguments_from_nodes_completion(operands))?;
            let no_supertype_reduction = types.len() == 2
                && types
                    .iter()
                    .position(|&ty| self.model.is_empty_type_literal(ty))
                    .is_some_and(|index| {
                        self.model.has_type_flags(
                            types[1 - index],
                            TypeFlags::STRING | TypeFlags::NUMBER | TypeFlags::BIGINT,
                        )
                    });
            let flags = if no_supertype_reduction {
                IntersectionFlags::NO_SUPERTYPE_REDUCTION
            } else {
                IntersectionFlags::NONE
            };
            sequence.read(self.intersection_type_ex_completion(
                types,
                flags,
                alias,
                &expression.location,
            ))
        })();
        let result = sequence.finish(result);
        self.intersections.nodes.insert(key, result.clone());
        result
    }

    /// `getTypeFromUnionTypeNode` has the same native TypeNodeLinks question,
    /// including alias acquisition before children and exact failure caching.
    pub(super) fn union_type_node_completion(
        &mut self,
        expression: &TypeExpression,
        site: Site,
        operands: &[TypeExpression],
        alias: Option<&AliasRecipe>,
    ) -> NativeResult<TypeId> {
        let key = (expression.file, site);
        if let Some(result) = self.intersections.nodes.get(&key) {
            return result.clone();
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let alias = alias
                .map(|recipe| sequence.read(self.establish_alias_completion(recipe)))
                .transpose()?;
            let types = sequence.read(self.type_arguments_from_nodes_completion(operands))?;
            sequence.read(self.union_type_ex_completion(types, alias, None, &expression.location))
        })();
        let result = sequence.finish(result);
        self.intersections.nodes.insert(key, result.clone());
        result
    }

    pub(super) fn intersection_type(
        &mut self,
        types: Vec<TypeId>,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.intersection_type_completion(types, consumer)?.admit()
    }

    pub(super) fn intersection_type_completion(
        &mut self,
        types: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = sequence.read(self.intersection_type_ex_completion(
            types,
            IntersectionFlags::NONE,
            None,
            consumer,
        ));
        sequence.finish(result)
    }

    /// `extractRedundantTemplateLiterals` visits symbolic strings backward,
    /// retaining the original literal order. A successful subtype question
    /// removes only that symbolic constituent; a rejected fixed pattern makes
    /// the whole intersection empty before Any or loose-null reduction.
    fn extract_redundant_template_literals_completion(
        &mut self,
        types: &mut Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let mut literals = Vec::new();
            for &ty in types.iter() {
                if self.model.has_type_flags(ty, TypeFlags::STRING_LITERAL) {
                    self.model.push_type(&mut literals, ty);
                }
            }
            for index in (0..types.len()).rev() {
                let ty = types[index];
                if !self
                    .model
                    .has_type_flags(ty, TypeFlags::TEMPLATE_LITERAL | TypeFlags::STRING_MAPPING)
                {
                    continue;
                }
                for &literal in &literals {
                    if sequence.read(self.raw_type_related_completion(
                        literal,
                        ty,
                        RelationKind::Subtype,
                        consumer,
                    ))? {
                        types.remove(index);
                        break;
                    }
                    if self.is_pattern_literal_type(ty)? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `getIntersectionTypeEx` owns normalization and all ordered semantic
    /// dependencies. Original operands survive flattening for propagation and
    /// divide-and-conquer; two-member constraint questions precede cache hits.
    pub(super) fn intersection_type_ex(
        &mut self,
        types: Vec<TypeId>,
        flags: IntersectionFlags,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.intersection_type_ex_completion(types, flags, alias, consumer)?
            .admit()
    }

    pub(super) fn intersection_type_ex_completion(
        &mut self,
        types: Vec<TypeId>,
        flags: IntersectionFlags,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let mut includes = TypeFlags::NONE;
            let mut empty = false;
            let mut missing = false;
            let mut wildcard = false;
            let mut native_error = false;
            let mut set = Vec::new();
            let mut seen = HashSet::new();
            let mut pending = Vec::new();
            for &ty in types.iter().rev() {
                pending.push(ty);
            }
            // `addTypesToIntersection` removes freshness, flattens in insertion
            // order, and tracks empty-object/unit identities before reduction.
            while let Some(ty) = pending.pop() {
                let mut ty = self.model.regular_literal(ty);
                if let Some(children) = self.model.intersection_constituents(ty) {
                    for &child in children.iter().rev() {
                        pending.push(child);
                    }
                    continue;
                }
                let type_flags = self.model.type_flags(ty);
                wildcard |= self.model.is_wildcard_type(ty);
                native_error |= self.model.is_error_type(ty);
                if self.is_empty_anonymous_object_type(ty, consumer)? {
                    if !empty {
                        empty = true;
                        seen.insert(ty);
                        self.model.push_type(&mut set, ty);
                    }
                    continue;
                }
                if !type_flags.intersects(TypeFlags::ANY | TypeFlags::UNKNOWN)
                    && (self.inputs.semantic_options().strict_null_checks
                        || !type_flags.intersects(TypeFlags::NULLABLE))
                {
                    if self.model.is_missing_type(ty) {
                        missing = true;
                        ty = self.model.special(SpecialName::Undefined);
                    }
                    if seen.insert(ty) {
                        if type_flags.intersects(TypeFlags::UNIT)
                            && includes.intersects(TypeFlags::UNIT)
                        {
                            includes = includes | TypeFlags::NON_PRIMITIVE;
                        }
                        self.model.push_type(&mut set, ty);
                    }
                }
                includes = includes | (type_flags & TypeFlags::INCLUDES_MASK);
            }
            if includes.intersects(TypeFlags::NEVER) {
                if set.iter().any(|&ty| self.model.is_silent_never(ty)) {
                    return Ok(self.model.silent_never());
                }
                return Ok(self.model.special(SpecialName::Never));
            }
            let disjoint = self.inputs.semantic_options().strict_null_checks
                && includes.intersects(TypeFlags::NULLABLE)
                && (includes.intersects(TypeFlags::OBJECT | TypeFlags::NON_PRIMITIVE) || empty)
                || [
                    TypeFlags::NON_PRIMITIVE,
                    TypeFlags::STRING_LIKE,
                    TypeFlags::NUMBER_LIKE,
                    TypeFlags::BIGINT_LIKE,
                    TypeFlags::ES_SYMBOL_LIKE,
                    TypeFlags::VOID_LIKE,
                ]
                .iter()
                .any(|&domain| {
                    includes.intersects(domain)
                        && includes.intersects(TypeFlags::DISJOINT_DOMAINS & !domain)
                });
            if disjoint {
                return Ok(self.model.special(SpecialName::Never));
            }
            if includes.intersects(TypeFlags::TEMPLATE_LITERAL | TypeFlags::STRING_MAPPING)
                && includes.intersects(TypeFlags::STRING_LITERAL)
                && sequence
                    .read(self.extract_redundant_template_literals_completion(&mut set, consumer))?
            {
                return Ok(self.model.special(SpecialName::Never));
            }
            if includes.intersects(TypeFlags::ANY) {
                return Ok(if wildcard {
                    self.model.wildcard_type()
                } else if native_error {
                    self.model.native_error_type()
                } else {
                    self.model.special(SpecialName::Any)
                });
            }
            if !self.inputs.semantic_options().strict_null_checks
                && includes.intersects(TypeFlags::NULLABLE)
            {
                return Ok(self.model.special(if empty {
                    SpecialName::Never
                } else if includes.intersects(TypeFlags::UNDEFINED) {
                    SpecialName::Undefined
                } else {
                    SpecialName::Null
                }));
            }
            let redundant = includes.intersects(TypeFlags::STRING)
                && includes.intersects(
                    TypeFlags::STRING_LITERAL
                        | TypeFlags::TEMPLATE_LITERAL
                        | TypeFlags::STRING_MAPPING,
                )
                || includes.intersects(TypeFlags::NUMBER)
                    && includes.intersects(TypeFlags::NUMBER_LITERAL)
                || includes.intersects(TypeFlags::BIGINT)
                    && includes.intersects(TypeFlags::BIGINT_LITERAL)
                || includes.intersects(TypeFlags::SYMBOL)
                    && includes.intersects(TypeFlags::UNIQUE_SYMBOL)
                || includes.intersects(TypeFlags::VOID)
                    && includes.intersects(TypeFlags::UNDEFINED)
                || empty && includes.intersects(TypeFlags::DEFINITELY_NON_NULLABLE);
            if redundant && !flags.contains(IntersectionFlags::NO_SUPERTYPE_REDUCTION) {
                // `removeRedundantSupertypes` walks backwards after the native
                // combined-flags guard, retaining the order of surviving inputs.
                for index in (0..set.len()).rev() {
                    let ty = set[index];
                    let f = self.model.type_flags(ty);
                    let remove = f.intersects(TypeFlags::STRING)
                        && includes.intersects(
                            TypeFlags::STRING_LITERAL
                                | TypeFlags::TEMPLATE_LITERAL
                                | TypeFlags::STRING_MAPPING,
                        )
                        || f.intersects(TypeFlags::NUMBER)
                            && includes.intersects(TypeFlags::NUMBER_LITERAL)
                        || f.intersects(TypeFlags::BIGINT)
                            && includes.intersects(TypeFlags::BIGINT_LITERAL)
                        || f.intersects(TypeFlags::SYMBOL)
                            && includes.intersects(TypeFlags::UNIQUE_SYMBOL)
                        || f.intersects(TypeFlags::VOID)
                            && includes.intersects(TypeFlags::UNDEFINED)
                        || self.is_empty_anonymous_object_type(ty, consumer)?
                            && includes.intersects(TypeFlags::DEFINITELY_NON_NULLABLE);
                    if remove {
                        set.remove(index);
                    }
                }
            }
            if missing {
                let undefined = self.model.special(SpecialName::Undefined);
                let index = set.iter().position(|&ty| ty == undefined).ok_or_else(|| {
                    super::super::internal("intersection missing flag lost undefined identity")
                })?;
                set[index] = self.model.missing_type();
            }
            if set.is_empty() {
                return Ok(self.model.special(SpecialName::Unknown));
            }
            if set.len() == 1 {
                return Ok(set[0]);
            }
            let mut object_flags = ObjectFlags::NONE;
            if set.len() == 2 && !flags.contains(IntersectionFlags::NO_CONSTRAINT_REDUCTION) {
                let variable_index =
                    usize::from(!self.model.has_type_flags(set[0], TypeFlags::TYPE_VARIABLE));
                let variable = set[variable_index];
                let primitive = set[1 - variable_index];
                if self
                    .model
                    .has_type_flags(variable, TypeFlags::TYPE_VARIABLE)
                    && (self
                        .model
                        .has_type_flags(primitive, TypeFlags::PRIMITIVE | TypeFlags::NON_PRIMITIVE)
                        && !self.is_generic_string_like_type(primitive)?
                        || empty)
                {
                    if let Some(constraint) =
                        sequence.read(self.base_constraint_completion(variable, consumer))?
                    {
                        let constituents = self
                            .model
                            .union_constituents(constraint)
                            .map(|types| self.model.copy_type_list(&types))
                            .unwrap_or_else(|| self.model.copy_type_list(&[constraint]));
                        let mut admitted = true;
                        for &ty in &constituents {
                            if !self
                                .model
                                .has_type_flags(ty, TypeFlags::PRIMITIVE | TypeFlags::NON_PRIMITIVE)
                                && !self.is_empty_anonymous_object_type(ty, consumer)?
                            {
                                admitted = false;
                                break;
                            }
                        }
                        if admitted {
                            if sequence.read(self.raw_type_related_completion(
                                constraint,
                                primitive,
                                RelationKind::StrictSubtype,
                                consumer,
                            ))? {
                                return Ok(variable);
                            }
                            let mut any_subtype = false;
                            if self.model.has_type_flags(constraint, TypeFlags::UNION) {
                                for &ty in &constituents {
                                    if sequence.read(self.raw_type_related_completion(
                                        ty,
                                        primitive,
                                        RelationKind::StrictSubtype,
                                        consumer,
                                    ))? {
                                        any_subtype = true;
                                        break;
                                    }
                                }
                            }
                            if !any_subtype
                                && !sequence.read(self.raw_type_related_completion(
                                    primitive,
                                    constraint,
                                    RelationKind::StrictSubtype,
                                    consumer,
                                ))?
                            {
                                return Ok(self.model.special(SpecialName::Never));
                            }
                            object_flags = ObjectFlags::IS_CONSTRAINED_TYPE_VARIABLE;
                        }
                    }
                }
            }
            let key = self.model.intersection_key(&set, flags, alias.as_ref());
            if let Some(completed) = self.model.cached_intersection(&key) {
                return sequence.read(Ok(completed));
            }
            // Only work after the native cache lookup belongs to its installed
            // completion. Template/constraint prefixes above execute on every call.
            let completed = {
                let mut sequence = NativeSequence::new();
                let result = (|| -> Result<(TypeId, bool), DemandError> {
                    let result = if includes.intersects(TypeFlags::UNION) {
                        if let Some(reduced) = sequence
                            .read(self.intersect_primitive_unions_completion(&set, consumer))?
                        {
                            sequence.read(
                                self.intersection_type_ex_completion(
                                    reduced, flags, alias, consumer,
                                ),
                            )?
                        } else if self.all_unions_contain(&set, TypeFlags::UNDEFINED) {
                            let contains_missing = set.iter().any(|&ty| {
                                self.model
                                    .union_constituents(ty)
                                    .expect("union checked")
                                    .iter()
                                    .any(|&ty| self.model.is_missing_type(ty))
                            });
                            let filtered =
                                self.filter_intersection_unions(&set, TypeFlags::UNDEFINED);
                            let intersection =
                                sequence.read(self.intersection_type_ex_completion(
                                    filtered, flags, None, consumer,
                                ))?;
                            let undefined = if contains_missing {
                                self.model.missing_type()
                            } else {
                                self.model.special(SpecialName::Undefined)
                            };
                            sequence.read(self.union_type_ex_completion(
                                self.model.copy_type_list(&[intersection, undefined]),
                                alias,
                                None,
                                consumer,
                            ))?
                        } else if self.all_unions_contain(&set, TypeFlags::NULL) {
                            let filtered = self.filter_intersection_unions(&set, TypeFlags::NULL);
                            let intersection =
                                sequence.read(self.intersection_type_ex_completion(
                                    filtered, flags, None, consumer,
                                ))?;
                            let null = self.model.special(SpecialName::Null);
                            sequence.read(self.union_type_ex_completion(
                                self.model.copy_type_list(&[intersection, null]),
                                alias,
                                None,
                                consumer,
                            ))?
                        } else if set.len() >= 3 && types.len() > 2 {
                            let middle = set.len() / 2;
                            let left = sequence.read(self.intersection_type_ex_completion(
                                self.model.copy_type_list(&set[..middle]),
                                flags,
                                None,
                                consumer,
                            ))?;
                            let right = sequence.read(self.intersection_type_ex_completion(
                                self.model.copy_type_list(&set[middle..]),
                                flags,
                                None,
                                consumer,
                            ))?;
                            sequence.read(self.intersection_type_ex_completion(
                                self.model.copy_type_list(&[left, right]),
                                flags,
                                alias,
                                consumer,
                            ))?
                        } else {
                            let Some(count) = sequence
                                .read(self.check_cross_product_union_completion(&set, consumer))?
                            else {
                                // Native returns errorType before intersectionTypes installation.
                                return Ok((self.model.native_error_type(), false));
                            };
                            let mut constituents = Vec::with_capacity(count);
                            for index in 0..count {
                                let mut operands = self.model.copy_type_list(&set);
                                let mut n = index;
                                for j in (0..set.len()).rev() {
                                    if let Some(types) = self.model.union_constituents(set[j]) {
                                        operands[j] = types[n % types.len()];
                                        n /= types.len();
                                    }
                                }
                                let ty = sequence.read(self.intersection_type_ex_completion(
                                    operands, flags, None, consumer,
                                ))?;
                                if !self.model.has_type_flags(ty, TypeFlags::NEVER) {
                                    self.model.push_type(&mut constituents, ty);
                                }
                            }
                            let origin = if constituents
                                .iter()
                                .any(|&ty| self.model.has_type_flags(ty, TypeFlags::INTERSECTION))
                                && self.constituent_count(&constituents)?
                                    > self.constituent_count(&set)?
                            {
                                Some(self.model.new_intersection(set, ObjectFlags::NONE, None))
                            } else {
                                None
                            };
                            sequence.read(self.union_type_ex_completion(
                                constituents,
                                alias,
                                origin,
                                consumer,
                            ))?
                        }
                    } else {
                        let propagated = self.model.propagating_flags(&types, TypeFlags::NULLABLE);
                        self.model
                            .new_intersection(set, object_flags | propagated, alias)
                    };
                    Ok((result, true))
                })();
                sequence.finish(result)
            }?;
            let install = completed.value().1;
            let completed = completed.map(|(ty, _)| ty);
            if install {
                self.model.cache_intersection(key, completed.clone());
            }
            sequence.read(Ok(completed))
        })();
        sequence.finish(result)
    }

    /// `intersectUnionsOfPrimitiveTypes` / `eachUnionContains`: literal bases
    /// and the Missing/Undefined pair are the only nonidentity matches.
    fn intersect_primitive_unions_completion(
        &mut self,
        types: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<Option<Vec<TypeId>>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Vec<TypeId>>, DemandError> {
            let mut unions = Vec::new();
            for &ty in types {
                if self
                    .model
                    .object_flags(ty)?
                    .intersects(ObjectFlags::PRIMITIVE_UNION)
                {
                    self.model.push_type(&mut unions, ty);
                }
            }
            if unions.len() < 2 {
                return Ok(None);
            }
            let mut checked = HashSet::new();
            let mut result = Vec::new();
            for &union in &unions {
                for &ty in self
                    .model
                    .union_constituents(union)
                    .ok_or_else(|| {
                        super::super::internal("PrimitiveUnion has no union constituents")
                    })?
                    .iter()
                {
                    if !checked.insert(ty) {
                        continue;
                    }
                    let primitive = match self.model.get(ty)? {
                        super::super::model::TypeView::Literal { base, .. }
                            if base != PrimitiveName::Boolean =>
                        {
                            Some(self.model.primitive(base))
                        }
                        _ => None,
                    };
                    let mut matched = true;
                    for &other in &unions {
                        let constituents = self
                            .model
                            .union_constituents(other)
                            .expect("primitive union");
                        if self.model.contains_type(&constituents, ty) {
                            continue;
                        }
                        if self.model.is_missing_type(ty) {
                            let undefined = self.model.special(SpecialName::Undefined);
                            matched = self.model.contains_type(&constituents, undefined);
                            break;
                        }
                        if ty == self.model.special(SpecialName::Undefined) {
                            matched = constituents
                                .iter()
                                .any(|&ty| self.model.is_missing_type(ty));
                            break;
                        }
                        if !primitive
                            .is_some_and(|base| self.model.contains_type(&constituents, base))
                        {
                            matched = false;
                            break;
                        }
                    }
                    if matched {
                        if self.model.has_type_flags(ty, TypeFlags::UNDEFINED) {
                            if let Some(index) = result.iter().position(|&other| {
                                self.model.has_type_flags(other, TypeFlags::UNDEFINED)
                            }) {
                                if self.model.is_missing_type(ty) {
                                    result[index] = ty;
                                }
                                continue;
                            }
                        }
                        self.model.push_type(&mut result, ty);
                    }
                }
            }
            // The primitive intersection is already reduced by identity matching;
            // using None preserves Missing rather than applying union Literal rules.
            let result = sequence.read(self.union_type_reduced_completion(
                result,
                UnionReduction::None,
                None,
                None,
                consumer,
            ))?;
            let mut reduced = Vec::with_capacity(types.len());
            let mut inserted = false;
            for &ty in types {
                if self.model.contains_type(&unions, ty) {
                    if !inserted {
                        inserted = true;
                        self.model.push_type(&mut reduced, result);
                    }
                } else {
                    self.model.push_type(&mut reduced, ty);
                }
            }
            Ok(Some(reduced))
        })();
        sequence.finish(result)
    }

    fn all_unions_contain(&self, types: &[TypeId], flags: TypeFlags) -> bool {
        types.iter().all(|&ty| {
            self.model.union_constituents(ty).is_some_and(|children| {
                children
                    .iter()
                    .take(if flags == TypeFlags::UNDEFINED { 1 } else { 2 })
                    .any(|&child| self.model.has_type_flags(child, flags))
            })
        })
    }

    fn filter_intersection_unions(&mut self, types: &[TypeId], flags: TypeFlags) -> Vec<TypeId> {
        let mut filtered = Vec::new();
        for &ty in types {
            let ty = self
                .model
                .filter_type(ty, |model, child| !model.has_type_flags(child, flags));
            self.model.push_type(&mut filtered, ty);
        }
        filtered
    }

    /// `getConstituentCountOfTypes` follows union origins and counts named
    /// composites as one; the count controls origin allocation, not semantics.
    fn constituent_count(&self, types: &[TypeId]) -> Result<usize, DemandError> {
        let mut count = 0;
        let mut pending = Vec::new();
        for &ty in types {
            pending.push(ty);
        }
        while let Some(ty) = pending.pop() {
            let metadata = self.model.metadata(ty)?;
            if metadata.alias.is_some() {
                count += 1;
            } else if let Some(origin) = metadata.summary_origin {
                pending.push(origin);
            } else if let Some(children) = self
                .model
                .union_constituents(ty)
                .or_else(|| self.model.intersection_constituents(ty))
            {
                for &child in children.iter() {
                    pending.push(child);
                }
            } else {
                count += 1;
            }
        }
        Ok(count)
    }
}
