//! One native indexed-access operation serves syntax, substitution and tuple
//! questions. Actual access-node context controls deferral and reference flow.

use super::super::model::{
    AccessFlags, Alias, DemandError, IntersectionFlags, NativeCompletion, NativeResult,
    NativeSequence, ObjectFlags, PropertySymbolId, SpecialName, TupleElementFlags, TypeFlags,
    TypeId, TypeView,
};
use super::super::source::{AccessRecipe, AliasRecipe, TypeExpression};
use super::{Evaluator, reads::AssignmentKind};
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::types::PrimitiveName;
use std::collections::HashMap;

pub(super) struct IndexedAccessState {
    nodes: HashMap<(FileId, Site), NativeResult<TypeId>>,
    unique_filled: HashMap<TypeId, NativeResult<TypeId>>,
    simplified: HashMap<(TypeId, bool), NativeResult<TypeId>>,
}

/// Native nil, IndexedAccessType and ElementAccessExpression contexts. A nil
/// query never inherits the source location used only for failure reporting.
#[derive(Clone, Copy)]
enum AccessNode<'a> {
    None,
    Type,
    Other,
    Element(&'a AccessRecipe, AssignmentKind),
}

#[derive(Clone, Copy)]
pub(super) struct IndexedValue {
    pub(super) ty: TypeId,
    pub(super) symbol: Option<PropertySymbolId>,
}
impl IndexedValue {
    fn plain(ty: TypeId) -> Self {
        Self { ty, symbol: None }
    }
}

impl Evaluator<'_, '_> {
    /// `getIndexedAccessTypeEx` for an actual non-Element access node, such as
    /// a binding name or destructuring synthetic expression. These nodes
    /// participate in deferral/error policy but never trigger Element flow.
    pub(super) fn indexed_access_at_node(
        &mut self,
        object: TypeId,
        index: TypeId,
        flags: AccessFlags,
        node: super::contextual::ContextNode,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.indexed_access_at_node_completion(object, index, flags, node, consumer)?
            .admit()
    }

    pub(super) fn indexed_access_at_node_completion(
        &mut self,
        object: TypeId,
        index: TypeId,
        flags: AccessFlags,
        _node: super::contextual::ContextNode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.required_indexed_access_completion(
            object,
            index,
            flags,
            AccessNode::Other,
            None,
            consumer,
        )
        .map(|value| value.map(|value| value.ty))
    }
    /// `getTypeFromIndexedAccessTypeNode` acquires object, index, then alias.
    pub(super) fn indexed_access_type_node_completion(
        &mut self,
        expression: &TypeExpression,
        site: Site,
        object: &TypeExpression,
        index: &TypeExpression,
        alias: Option<&AliasRecipe>,
    ) -> NativeResult<TypeId> {
        let key = (expression.file, site);
        if let Some(result) = self.indexed_access.nodes.get(&key) {
            return result.clone();
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let object = sequence.read(self.check_type_completion(object))?;
            let index = sequence.read(self.check_type_completion(index))?;
            let alias = alias
                .map(|recipe| sequence.read(self.establish_alias_completion(recipe)))
                .transpose()?;
            sequence
                .read(self.required_indexed_access_completion(
                    object,
                    index,
                    AccessFlags::NONE,
                    AccessNode::Type,
                    alias,
                    &expression.location,
                ))
                .map(|value| value.ty)
        })();
        let result = sequence.finish(result);
        self.indexed_access.nodes.insert(key, result.clone());
        result
    }

    /// `getIndexedAccessType`: a compiler-internal query has no access node.
    pub(super) fn indexed_access_type(
        &mut self,
        object: TypeId,
        index: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.indexed_access_type_ex(object, index, AccessFlags::NONE, None, consumer)
    }

    pub(super) fn indexed_access_type_ex(
        &mut self,
        object: TypeId,
        index: TypeId,
        flags: AccessFlags,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.indexed_access_type_ex_completion(object, index, flags, alias, consumer)?
            .admit()
    }

    pub(super) fn indexed_access_type_completion(
        &mut self,
        object: TypeId,
        index: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.indexed_access_type_ex_completion(object, index, AccessFlags::NONE, None, consumer)
    }

    pub(super) fn indexed_access_type_ex_completion(
        &mut self,
        object: TypeId,
        index: TypeId,
        flags: AccessFlags,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.required_indexed_access_completion(
            object,
            index,
            flags,
            AccessNode::None,
            alias,
            consumer,
        )
        .map(|value| value.map(|value| value.ty))
    }

    /// `checkElementAccessExpression` chooses write/context flags before
    /// calling the same resolver as type nodes. Outer reference flow remains
    /// at the caller; the inner property flow runs only for an element node.
    pub(super) fn indexed_element_value_completion(
        &mut self,
        recipe: &AccessRecipe,
        object: TypeId,
        index: TypeId,
        assignment: AssignmentKind,
    ) -> NativeResult<IndexedValue> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let flags = if assignment == AssignmentKind::None {
                AccessFlags::EXPRESSION_POSITION
            } else {
                let mut flags = AccessFlags::WRITING;
                if assignment != AssignmentKind::Definite {
                    flags |= AccessFlags::EXPRESSION_POSITION;
                }
                if sequence
                    .read(self.is_generic_object_type_completion(object, &recipe.location))?
                    && !matches!(self.model.get(object)?,TypeView::TypeParameter(parameter) if parameter.is_this)
                {
                    flags |= AccessFlags::NO_INDEX_SIGNATURES;
                }
                flags
            };
            sequence.read(self.required_indexed_access_completion(
                object,
                index,
                flags,
                AccessNode::Element(recipe, assignment),
                None,
                &recipe.location,
            ))
        })();
        sequence.finish(result)
    }

    /// `getIndexedAccessTypeEx`: only native absence becomes the native
    /// node-dependent recovery. Consumed failures never pass through Option.
    fn required_indexed_access_completion(
        &mut self,
        object: TypeId,
        index: TypeId,
        flags: AccessFlags,
        node: AccessNode<'_>,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<IndexedValue> {
        self.indexed_access_optional_completion(object, index, flags, node, alias, consumer)?
            .and_then(|value| match value {
                Some(value) => Ok(NativeCompletion::clean(value)),
                None if matches!(node, AccessNode::None) => Ok(NativeCompletion::clean(
                    IndexedValue::plain(self.model.special(SpecialName::Unknown)),
                )),
                None => NativeCompletion::recovered(
                    IndexedValue::plain(self.model.native_error_type()),
                    self.access_error(
                        consumer,
                        "indexed access consumed an absent property or invalid index",
                    ),
                ),
            })
    }

    /// `getIndexedAccessTypeOrUndefined`: flags in the identity key retain
    /// their incoming values; only persistent bits survive on the operator.
    fn indexed_access_optional_completion(
        &mut self,
        object: TypeId,
        mut index: TypeId,
        mut flags: AccessFlags,
        node: AccessNode<'_>,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<Option<IndexedValue>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let object = sequence.read(self.reduced_type_completion(object, consumer))?;
            if sequence.read(self.string_index_signature_only_completion(object, consumer))?
                && !self.model.has_type_flags(index, TypeFlags::NULLABLE)
                && sequence.read(self.type_assignable_to_kind_completion(
                    index,
                    TypeFlags::STRING | TypeFlags::NUMBER,
                    false,
                    consumer,
                ))?
            {
                index = self.model.primitive(PrimitiveName::String);
            }
            if self.inputs.semantic_options().no_unchecked_indexed_access
                && flags.contains(AccessFlags::EXPRESSION_POSITION)
            {
                flags |= AccessFlags::INCLUDE_UNDEFINED;
            }
            if sequence
                .read(self.should_defer_indexed_access_completion(object, index, node, consumer))?
            {
                let ty = if self
                    .model
                    .has_type_flags(object, TypeFlags::ANY | TypeFlags::UNKNOWN)
                {
                    object
                } else {
                    self.model.indexed_access_type(
                        object,
                        index,
                        flags,
                        alias.map(std::sync::Arc::new),
                    )
                };
                return Ok(Some(IndexedValue::plain(ty)));
            }
            let apparent =
                sequence.read(self.reduced_apparent_type_completion(object, consumer))?;
            if let Some(keys) = self
                .model
                .union_constituents(index)
                .filter(|_| !self.model.has_type_flags(index, TypeFlags::BOOLEAN))
            {
                let mut types = Vec::new();
                let mut missing = false;
                for &key in keys.iter() {
                    let flags = flags
                        | if missing {
                            AccessFlags::SUPPRESS_NO_IMPLICIT_ANY
                        } else {
                            AccessFlags::NONE
                        };
                    match sequence.read(
                        self.property_type_for_index_completion(
                            apparent, key, flags, node, consumer,
                        ),
                    )? {
                        Some(value) => types.push(value.ty),
                        None if matches!(node, AccessNode::None) => return Ok(None),
                        None => missing = true,
                    }
                }
                if missing {
                    return Ok(None);
                }
                let ty = if flags.contains(AccessFlags::WRITING) {
                    sequence.read(self.intersection_type_ex_completion(
                        types,
                        IntersectionFlags::NONE,
                        alias,
                        consumer,
                    ))?
                } else {
                    sequence.read(self.union_type_ex_completion(types, alias, None, consumer))?
                };
                return Ok(Some(IndexedValue::plain(ty)));
            }
            sequence.read(self.property_type_for_index_completion(
                apparent,
                index,
                flags | AccessFlags::CACHE_SYMBOL | AccessFlags::REPORT_DEPRECATED,
                node,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `shouldDeferIndexedAccessType` deliberately distinguishes nil/type
    /// nodes from expressions, independently of the access flag bits.
    fn should_defer_indexed_access_completion(
        &mut self,
        object: TypeId,
        index: TypeId,
        node: AccessNode<'_>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if sequence.read(self.is_generic_index_type_completion(index, consumer))? {
                return Ok(true);
            }
            if matches!(node, AccessNode::Element(..) | AccessNode::Other) {
                return Ok(self.model.is_generic_tuple_type(object)?
                    && !self.index_type_within_tuple(object, index)?);
            }
            Ok(
                (sequence.read(self.is_generic_object_type_completion(object, consumer))?
                    && !(self.model.is_tuple_type(object)
                        && self.index_type_within_tuple(object, index)?))
                    || sequence
                        .read(self.is_generic_reducible_type_completion(object, consumer))?,
            )
        })();
        sequence.finish(result)
    }

    /// `indexTypeLessThan` uses every union constituent and the native exact
    /// numeric-name round trip; fractional and signed values keep their value.
    fn index_type_within_tuple(&self, object: TypeId, index: TypeId) -> Result<bool, DemandError> {
        let target = self
            .model
            .tuple_target(object)?
            .ok_or_else(|| super::super::internal("tuple access lacks target"))?;
        let limit = target.total_fixed_element_count() as f64;
        let types = self
            .model
            .union_constituents(index)
            .unwrap_or_else(|| vec![index].into());
        for &ty in types.iter() {
            if !self
                .model
                .has_type_flags(ty, TypeFlags::STRING_LITERAL | TypeFlags::NUMBER_LITERAL)
            {
                return Ok(false);
            }
            let Some(name) = self.property_name_from_type(ty)? else {
                return Ok(false);
            };
            if !crate::numbers::numeric_literal_name_value(&name)
                .is_some_and(|value| value >= 0.0 && value < limit)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `isGenericReducibleType` / `isReducibleIntersection`: the unique Never
    /// mapper preserves a symbolic union exactly when reduction could change
    /// after instantiation. Its filled instantiation is cached independently.
    pub(super) fn is_generic_reducible_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.has_type_flags(ty, TypeFlags::UNION)
                && self
                    .model
                    .object_flags(ty)?
                    .contains(ObjectFlags::CONTAINS_INTERSECTIONS)
            {
                for &ty in self
                    .model
                    .union_constituents(ty)
                    .expect("Union flag has constituents")
                    .iter()
                {
                    if sequence.read(self.is_generic_reducible_type_completion(ty, consumer))? {
                        return Ok(true);
                    }
                }
            }
            if self.model.has_type_flags(ty, TypeFlags::INTERSECTION) {
                let filled = match self.indexed_access.unique_filled.get(&ty) {
                    Some(result) => sequence.read(result.clone())?,
                    None => {
                        let result = self.instantiate_type_completion(
                            ty,
                            self.model.unique_literal_mapper(),
                            consumer,
                        );
                        self.indexed_access.unique_filled.insert(ty, result.clone());
                        sequence.read(result)?
                    }
                };
                return Ok(
                    sequence.read(self.reduced_type_completion(filled, consumer))? != filled,
                );
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isStringIndexSignatureOnlyType` asks the complete structured inventory.
    fn string_index_signature_only_completion(
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
                if !sequence
                    .read(self.properties_of_type_completion(ty, consumer))?
                    .is_empty()
                    || sequence
                        .read(self.index_infos_of_type_completion(ty, consumer))?
                        .len()
                        != 1
                {
                    return Ok(false);
                }
                let string = self.model.primitive(PrimitiveName::String);
                return Ok(sequence
                    .read(self.index_info_of_type_completion(ty, string, consumer))?
                    .is_some());
            }
            if let Some(types) = self
                .model
                .union_constituents(ty)
                .or_else(|| self.model.intersection_constituents(ty))
            {
                for &ty in types.iter() {
                    if !sequence.read(self.string_index_signature_only_completion(ty, consumer))? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `getPropertyTypeForIndexType` owns actual property/write acquisition,
    /// tuple misses and index-signature recovery. Diagnostic-only branches
    /// preserve the compiler's returned type.
    fn property_type_for_index_completion(
        &mut self,
        object: TypeId,
        index: TypeId,
        flags: AccessFlags,
        node: AccessNode<'_>,
        consumer: &Location,
    ) -> NativeResult<Option<IndexedValue>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(name) = self.property_key_from_type(index)? {
                if flags.contains(AccessFlags::CONTEXTUAL) {
                    let ty = sequence.read(self.contextual_property_type_by_key_completion(
                        object, &name, None, consumer,
                    ))?;
                    return Ok(Some(IndexedValue::plain(
                        ty.unwrap_or_else(|| self.model.special(SpecialName::Any)),
                    )));
                }
                if let Some(property) = sequence
                    .read(self.property_of_type_by_key_completion(object, &name, consumer))?
                {
                    let symbol = property.symbol.ok_or_else(|| {
                        super::super::internal("indexed property has no actual symbol")
                    })?;
                    let mut cached = None;
                    if let AccessNode::Element(recipe, assignment) = node {
                        if sequence.read(self.assignment_to_readonly_property_completion(
                            recipe, symbol, assignment,
                        ))? {
                            return Ok(None);
                        }
                        if flags.contains(AccessFlags::CACHE_SYMBOL) {
                            cached = Some(symbol);
                        }
                        if self.property_access_auto_type(symbol, recipe.root, consumer)? {
                            return Ok(Some(IndexedValue {
                                ty: self.model.automatic_variable(),
                                symbol: cached,
                            }));
                        }
                    }
                    let mut ty = if flags.contains(AccessFlags::WRITING) {
                        sequence.read(self.property_write_type_completion(symbol, consumer))?
                    } else {
                        sequence.read(self.property_raw_type_completion(symbol, consumer))?
                    };
                    match node {
                        AccessNode::Element(recipe, assignment)
                            if assignment != AssignmentKind::Definite =>
                        {
                            ty = sequence.read(self.access_reference_flow_completion(
                                recipe.root,
                                ty,
                                ty,
                                consumer,
                            ))?;
                        }
                        AccessNode::Type if self.model.contains_missing_type(ty) => {
                            let undefined = self.model.special(SpecialName::Undefined);
                            ty = sequence.read(self.union_type_ex_completion(
                                vec![ty, undefined],
                                None,
                                None,
                                consumer,
                            ))?;
                        }
                        _ => {}
                    }
                    return Ok(Some(IndexedValue { ty, symbol: cached }));
                }
                let types = self
                    .model
                    .union_constituents(object)
                    .unwrap_or_else(|| vec![object].into());
                if types.iter().all(|&ty| self.model.is_tuple_type(ty)) {
                    if let Some(index) = self
                        .literal_numeric_text(
                            &super::super::model::LiteralValue::from_property_key(&name),
                            consumer,
                        )?
                        .and_then(crate::numbers::numeric_literal_name_value)
                    {
                        let mut all_fixed = true;
                        for &ty in types.iter() {
                            let target = self
                                .model
                                .tuple_target(ty)?
                                .ok_or_else(|| super::super::internal("tuple has no target"))?;
                            all_fixed &= !target
                                .combined_flags
                                .intersects(TupleElementFlags::VARIABLE);
                        }
                        if !matches!(node, AccessNode::None)
                            && all_fixed
                            && !flags.contains(AccessFlags::ALLOW_MISSING)
                            && self.model.is_tuple_type(object)
                            && index < 0.0
                        {
                            return Ok(Some(IndexedValue::plain(
                                self.model.special(SpecialName::Undefined),
                            )));
                        }
                        if index >= 0.0 {
                            // Native asks for the numeric index before its readonly
                            // diagnostic even for a compiler-internal nil node.
                            let number = self.model.primitive(PrimitiveName::Number);
                            sequence.read(
                                self.index_info_of_type_completion(object, number, consumer),
                            )?;
                            let missing = flags
                                .contains(AccessFlags::INCLUDE_UNDEFINED)
                                .then(|| self.model.missing_type());
                            return sequence
                                .read(self.tuple_element_type_out_of_start_count_completion(
                                    object, index, missing, consumer,
                                ))
                                .map(|ty| Some(IndexedValue::plain(ty)));
                        }
                    }
                }
            }
            if !self.model.has_type_flags(index, TypeFlags::NULLABLE)
                && sequence.read(self.type_assignable_to_kind_completion(
                    index,
                    TypeFlags::STRING_LIKE | TypeFlags::NUMBER_LIKE | TypeFlags::ES_SYMBOL_LIKE,
                    false,
                    consumer,
                ))?
            {
                if self
                    .model
                    .has_type_flags(object, TypeFlags::ANY | TypeFlags::NEVER)
                {
                    return Ok(Some(IndexedValue::plain(object)));
                }
                let info = match sequence
                    .read(self.applicable_index_info_completion(object, index, consumer))?
                {
                    Some(info) => Some(info),
                    None => {
                        let string = self.model.primitive(PrimitiveName::String);
                        sequence
                            .read(self.index_info_of_type_completion(object, string, consumer))?
                    }
                };
                if let Some(info) = info {
                    let key = sequence.read(self.demand_completion(info.key_type))?;
                    if flags.contains(AccessFlags::NO_INDEX_SIGNATURES)
                        && key != self.model.primitive(PrimitiveName::Number)
                    {
                        return Ok(None);
                    }
                    if !matches!(node, AccessNode::None)
                        && key == self.model.primitive(PrimitiveName::String)
                        && !sequence.read(self.type_assignable_to_kind_completion(
                            index,
                            TypeFlags::STRING | TypeFlags::NUMBER,
                            false,
                            consumer,
                        ))?
                    {
                        let value = sequence.read(self.demand_completion(info.value_type))?;
                        let ty = if flags.contains(AccessFlags::INCLUDE_UNDEFINED) {
                            let missing = self.model.missing_type();
                            sequence.read(self.union_type_ex_completion(
                                vec![value, missing],
                                None,
                                None,
                                consumer,
                            ))?
                        } else {
                            value
                        };
                        return Ok(Some(IndexedValue::plain(ty)));
                    }
                    let mut ty = sequence.read(self.demand_completion(info.value_type))?;
                    if flags.contains(AccessFlags::INCLUDE_UNDEFINED) {
                        // `getPropertyTypeForIndexType`: E[E.A] retains its
                        // reverse numeric index result without missingness.
                        let own_enum = if let Some(object_symbol) =
                            self.reference_symbol_of_type(object, consumer)?
                            && self.reference_symbol_flags(object_symbol, consumer)?.enum_
                            && let Some(index_symbol) =
                                self.reference_symbol_of_type(index, consumer)?
                            && self.model.has_type_flags(index, TypeFlags::ENUM_LITERAL)
                            && let Some(parent) = sequence.read(
                                self.reference_parent_of_symbol_completion(index_symbol, consumer),
                            )? {
                            self.reference_symbol_identity(parent, consumer)?
                                == self.reference_symbol_identity(object_symbol, consumer)?
                        } else {
                            false
                        };
                        if !own_enum {
                            let missing = self.model.missing_type();
                            ty = sequence.read(self.union_type_ex_completion(
                                vec![ty, missing],
                                None,
                                None,
                                consumer,
                            ))?;
                        }
                    }
                    return Ok(Some(IndexedValue::plain(ty)));
                }
                if self.model.has_type_flags(index, TypeFlags::NEVER) {
                    return Ok(Some(IndexedValue::plain(
                        self.model.special(SpecialName::Never),
                    )));
                }
                if matches!(node, AccessNode::Element(..))
                    && self
                        .model
                        .object_flags(object)?
                        .intersects(ObjectFlags::OBJECT_LITERAL)
                {
                    if self.inputs.semantic_options().no_implicit_any
                        && self.model.has_type_flags(
                            index,
                            TypeFlags::STRING_LITERAL | TypeFlags::NUMBER_LITERAL,
                        )
                    {
                        return Ok(Some(IndexedValue::plain(
                            self.model.special(SpecialName::Undefined),
                        )));
                    }
                    if self
                        .model
                        .has_type_flags(index, TypeFlags::NUMBER | TypeFlags::STRING)
                    {
                        let mut types = Vec::new();
                        for property in
                            sequence.read(self.properties_of_type_completion(object, consumer))?
                        {
                            let symbol = property.symbol.ok_or_else(|| {
                                super::super::internal("object literal property has no symbol")
                            })?;
                            types.push(
                                sequence
                                    .read(self.property_raw_type_completion(symbol, consumer))?,
                            );
                        }
                        types.push(self.model.special(SpecialName::Undefined));
                        return Ok(Some(IndexedValue::plain(sequence.read(
                            self.union_type_ex_completion(types, None, None, consumer),
                        )?)));
                    }
                    return Ok(None);
                }
            }
            if flags.contains(AccessFlags::ALLOW_MISSING)
                && self
                    .model
                    .object_flags(object)?
                    .intersects(ObjectFlags::OBJECT_LITERAL)
            {
                return Ok(Some(IndexedValue::plain(
                    self.model.special(SpecialName::Undefined),
                )));
            }
            if self.model.has_type_flags(index, TypeFlags::ANY) {
                return Ok(Some(IndexedValue::plain(index)));
            }
            Ok(None)
        })();
        sequence.finish(result)
    }
}

impl Evaluator<'_, '_> {
    /// `getSimplifiedType` has one dispatch for all constraint and relation
    /// consumers. The cache stores an original-type reentry marker, then the
    /// actual result or failure; failed work cannot leave a healthy marker.
    pub(super) fn simplified_type_completion(
        &mut self,
        ty: TypeId,
        writing: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let TypeView::IndexedAccess(access) = self.model.get(ty)? {
            if let Some(result) = self.indexed_access.simplified.get(&(ty, writing)) {
                return result.clone();
            }
            self.indexed_access
                .simplified
                .insert((ty, writing), Ok(NativeCompletion::clean(ty)));
            let result = self
                .simplified_indexed_access_completion(ty, access, writing, consumer)
                .map(|result| {
                    result.map(|result| {
                        if result == ty {
                            ty
                        } else {
                            self.model.remove_type(result, ty)
                        }
                    })
                });
            self.indexed_access
                .simplified
                .insert((ty, writing), result.clone());
            return result;
        }
        if self.model.has_type_flags(ty, TypeFlags::CONDITIONAL) {
            return self.simplified_conditional_type_completion(ty, writing, consumer);
        }
        Ok(NativeCompletion::clean(ty))
    }

    /// `distributeIndexOverObjectType` is shared by indexed simplification
    /// and inference, whose caller deliberately permits instantiable indexes.
    pub(super) fn distribute_index_over_object_type_completion(
        &mut self,
        object: TypeId,
        index: TypeId,
        writing: bool,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let intersection = self.model.has_type_flags(object, TypeFlags::INTERSECTION);
            if self.model.has_type_flags(object, TypeFlags::UNION)
                || intersection
                    && !sequence.read(self.should_defer_index_type_completion(
                        object,
                        super::super::model::IndexFlags::NONE,
                        consumer,
                    ))?
            {
                let operands = self
                    .model
                    .union_constituents(object)
                    .or_else(|| self.model.intersection_constituents(object))
                    .ok_or_else(|| {
                        super::super::internal("distributed object has no constituents")
                    })?;
                let mut types = Vec::with_capacity(operands.len());
                for &object in operands.iter() {
                    let ty = sequence
                        .read(self.indexed_access_type_completion(object, index, consumer))?;
                    types.push(
                        sequence.read(self.simplified_type_completion(ty, writing, consumer))?,
                    );
                }
                return if writing || intersection {
                    sequence
                        .read(self.intersection_type_completion(types, consumer))
                        .map(Some)
                } else {
                    sequence
                        .read(self.union_type_ex_completion(types, None, None, consumer))
                        .map(Some)
                };
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `getSimplifiedIndexedAccessTypeWorker`, including distribution order.
    /// Child operators use the native no-node/no-flags/no-alias factory.
    fn simplified_indexed_access_completion(
        &mut self,
        original: TypeId,
        access: super::super::model::IndexedAccessView,
        writing: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let object = sequence.read(self.simplified_type_completion(
                access.object_type,
                writing,
                consumer,
            ))?;
            let index = sequence.read(self.simplified_type_completion(
                access.index_type,
                writing,
                consumer,
            ))?;
            if let Some(keys) = self.model.union_constituents(index) {
                let mut types = Vec::with_capacity(keys.len());
                for &key in keys.iter() {
                    let ty = sequence
                        .read(self.indexed_access_type_completion(object, key, consumer))?;
                    types.push(
                        sequence.read(self.simplified_type_completion(ty, writing, consumer))?,
                    );
                }
                return if writing {
                    sequence.read(self.intersection_type_completion(types, consumer))
                } else {
                    sequence.read(self.union_type_ex_completion(types, None, None, consumer))
                };
            }
            if !self.model.has_type_flags(index, TypeFlags::INSTANTIABLE) {
                if let Some(distributed) =
                    sequence.read(self.distribute_index_over_object_type_completion(
                        object, index, writing, consumer,
                    ))?
                {
                    return Ok(distributed);
                }
            }
            if self.model.is_generic_tuple_type(object)?
                && self.model.has_type_flags(index, TypeFlags::NUMBER_LIKE)
            {
                let start = if self.model.has_type_flags(index, TypeFlags::NUMBER) {
                    0
                } else {
                    self.model
                        .tuple_target(object)?
                        .ok_or_else(|| super::super::internal("generic tuple has no target"))?
                        .fixed_length
                };
                if let Some(ty) =
                    sequence.read(self.get_element_type_of_slice_of_tuple_type_completion(
                        object, start, 0, writing, false, consumer,
                    ))?
                {
                    return Ok(ty);
                }
            }
            if sequence.read(self.is_generic_mapped_type_completion(object, consumer))?
                && sequence.read(self.mapped_name_kind_completion(object, consumer))?
                    != super::mapped::NameKind::Remapping
            {
                let value = sequence.read(self.substitute_indexed_mapped_type_completion(
                    object,
                    access.index_type,
                    consumer,
                ))?;
                return sequence.read(self.map_type_completion(value, consumer, |checker, ty| {
                    checker.simplified_type_completion(ty, writing, consumer)
                }));
            }
            Ok(original)
        })();
        sequence.finish(result)
    }

    /// `getSimplifiedTypeOrConstraint` preserves the distinction between raw
    /// guarded parameter constraints and their recursively flattened bases.
    pub(super) fn simplified_type_or_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let simplified = sequence.read(self.simplified_type_completion(ty, false, consumer))?;
            if simplified != ty {
                return Ok(Some(simplified));
            }
            sequence.read(self.constraint_of_type_completion(ty, consumer))
        })();
        sequence.finish(result)
    }

    /// `getConstraintFromIndexedAccess` first substitutes a changed index,
    /// then a changed object. Absence is distinct from consumed failure.
    pub(super) fn constraint_from_indexed_access_completion(
        &mut self,
        access: super::super::model::IndexedAccessView,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if sequence.read(self.is_mapped_generic_indexed_access_completion(
                access.object_type,
                access.index_type,
                consumer,
            ))? {
                return sequence
                    .read(self.substitute_indexed_mapped_type_completion(
                        access.object_type,
                        access.index_type,
                        consumer,
                    ))
                    .map(Some);
            }
            if let Some(index) = sequence
                .read(self.simplified_type_or_constraint_completion(access.index_type, consumer))?
            {
                if index != access.index_type {
                    if let Some(ty) =
                        sequence.read(self.indexed_access_without_node_optional_completion(
                            access.object_type,
                            index,
                            access.access_flags,
                            consumer,
                        ))?
                    {
                        return Ok(Some(ty));
                    }
                }
            }
            if let Some(object) = sequence
                .read(self.simplified_type_or_constraint_completion(access.object_type, consumer))?
            {
                if object != access.object_type {
                    return sequence.read(self.indexed_access_without_node_optional_completion(
                        object,
                        access.index_type,
                        access.access_flags,
                        consumer,
                    ));
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    pub(super) fn indexed_access_without_node_optional(
        &mut self,
        object: TypeId,
        index: TypeId,
        flags: AccessFlags,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.indexed_access_without_node_optional_completion(object, index, flags, consumer)?
            .admit()
    }

    pub(super) fn indexed_access_without_node_optional_completion(
        &mut self,
        object: TypeId,
        index: TypeId,
        flags: AccessFlags,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.indexed_access_optional_completion(
            object,
            index,
            flags,
            AccessNode::None,
            None,
            consumer,
        )
        .map(|result| result.map(|value| value.map(|value| value.ty)))
    }
}

impl Evaluator<'_, '_> {
    /// `checkIndexedAccessIndexType`: validate every key against actual keyof,
    /// retaining the numeric index exception before diagnosing invalid access.
    pub(super) fn check_indexed_access_index_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let TypeView::IndexedAccess(access) = self.model.get(ty)? else {
                return Ok(ty);
            };
            let keys = if sequence
                .read(self.is_generic_mapped_type_completion(access.object_type, consumer))?
                && sequence.read(self.mapped_name_kind_completion(access.object_type, consumer))?
                    == super::mapped::NameKind::Remapping
            {
                sequence.read(self.mapped_index_type_completion(
                    access.object_type,
                    super::super::model::IndexFlags::NONE,
                    consumer,
                ))?
            } else {
                sequence.read(self.index_type_ex_completion(
                    access.object_type,
                    super::super::model::IndexFlags::NONE,
                    consumer,
                ))?
            };
            let number = self.model.primitive(PrimitiveName::Number);
            let number_index = sequence
                .read(self.index_info_of_type_completion(access.object_type, number, consumer))?
                .is_some();
            let indexes = self
                .model
                .union_constituents(access.index_type)
                .unwrap_or_else(|| vec![access.index_type].into());
            let mut valid = true;
            for &index in indexes.iter() {
                if !(sequence.read(self.type_related_completion(
                    index,
                    keys,
                    super::relations::RelationKind::Assignable,
                    consumer,
                ))? || number_index
                    && sequence
                        .read(self.is_applicable_index_type_completion(index, number, consumer))?)
                {
                    valid = false;
                    break;
                }
            }
            if valid {
                return Ok(ty);
            }
            if sequence
                .read(self.is_generic_object_type_completion(access.object_type, consumer))?
            {
                if let Some(name) = self.property_key_from_type(access.index_type)? {
                    let apparent = sequence
                        .read(self.apparent_type_completion(access.object_type, consumer))?;
                    let objects = self
                        .model
                        .union_constituents(apparent)
                        .unwrap_or_else(|| vec![apparent].into());
                    for &object in objects.iter() {
                        if let Some(property) = sequence.read(
                            self.property_of_type_by_key_completion(object, &name, consumer),
                        )? {
                            let symbol = property.symbol.ok_or_else(|| {
                                super::super::internal("constituent property lacks symbol")
                            })?;
                            if self.property_declaration_modifiers(symbol, false, consumer)?
                                & (crate::syntax::modifiers::PRIVATE
                                    | crate::syntax::modifiers::PROTECTED)
                                != 0
                            {
                                return sequence.read(NativeCompletion::recovered(self.model.native_error_type(),
                                self.access_error(consumer, "private or protected property cannot be indexed on a type parameter")));
                            }
                            break;
                        }
                    }
                }
            }
            sequence.read(self.diagnostic_type_display_completion(
                access.index_type,
                None,
                super::native_display::DiagnosticTypeFormat::DEFAULT,
                consumer,
            ))?;
            sequence.read(self.diagnostic_type_display_completion(
                access.object_type,
                None,
                super::native_display::DiagnosticTypeFormat::DEFAULT,
                consumer,
            ))?;
            sequence.read(NativeCompletion::recovered(
                self.model.native_error_type(),
                self.access_error(
                    consumer,
                    "index type cannot be used to index the object type",
                ),
            ))
        })();
        sequence.finish(result)
    }
}

impl IndexedAccessState {
    /// Request-local backing for the existing native cache questions. Their
    /// identities, child values and provisional publication stay operation-owned.
    pub(super) fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            unique_filled: HashMap::new(),
            simplified: HashMap::new(),
        }
    }
}
