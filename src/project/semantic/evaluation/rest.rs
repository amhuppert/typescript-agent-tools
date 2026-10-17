//! Object rest shares property identity, anonymous instantiation and real Omit
//! alias acquisition. Removing names does not eagerly acquire property values.

use super::super::model::{
    DemandError, MembersView, NativeCompletion, NativeResult, NativeSequence, ObjectFlags,
    TypeFlags, TypeId, TypeSymbol,
};
use super::super::source::ExpressionRoot;
use super::Evaluator;
use super::facts::TypeFacts;
use super::relations::RelationKind;
use crate::analysis::SemanticFailureReason;
use crate::references::Location;
use std::sync::Arc;

impl Evaluator<'_, '_> {
    /// `isValidSpreadType` maps base constraints before removing definitely
    /// falsy constituents. Intersections then recurse without an apparent-type
    /// conversion: primitive wrappers are not valid spread sources here.
    pub(super) fn is_valid_spread_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_valid_spread_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn is_valid_spread_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let ty = sequence.read(self.map_base_constraints_completion(ty, consumer))?;
            let ty =
                sequence.read(self.type_with_facts_completion(ty, TypeFacts::TRUTHY, consumer))?;
            if self.model.has_type_flags(
                ty,
                TypeFlags::ANY
                    | TypeFlags::NON_PRIMITIVE
                    | TypeFlags::OBJECT
                    | TypeFlags::INSTANTIABLE_NON_PRIMITIVE,
            ) {
                return Ok(true);
            }
            let constituents = self
                .model
                .union_constituents(ty)
                .or_else(|| self.model.intersection_constituents(ty))
                .map(|types| types.to_vec());
            if let Some(types) = constituents {
                for child in types {
                    if !sequence.read(self.is_valid_spread_type_completion(child, consumer))? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `getRestType` filters nullability before keys, visits actual properties
    /// in native order, and only then chooses generic Omit versus spread symbols.
    pub(super) fn rest_type(
        &mut self,
        source: TypeId,
        names: &[ExpressionRoot],
        symbol: TypeSymbol,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.rest_type_completion(source, names, symbol, consumer)?
            .admit()
    }

    pub(super) fn rest_type_completion(
        &mut self,
        source: TypeId,
        names: &[ExpressionRoot],
        symbol: TypeSymbol,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let source = self.model.filter_type(source, |model, ty| {
                !model.has_type_flags(ty, TypeFlags::NULLABLE)
            });
            if self.model.has_type_flags(source, TypeFlags::NEVER) {
                return Ok(self.model.empty_object_type());
            }
            if self.model.has_type_flags(source, TypeFlags::UNION) {
                return sequence.read(self.map_type_completion(
                    source,
                    consumer,
                    |checker, child| {
                        checker.rest_type_completion(child, names, symbol.clone(), consumer)
                    },
                ));
            }
            let mut keys = Vec::with_capacity(names.len());
            for name in names {
                keys.push(
                    sequence.read(
                        self.literal_type_of_property_name_completion(name.file, name.site),
                    )?,
                );
            }
            let mut omit =
                sequence.read(self.union_type_ex_completion(keys, None, None, consumer))?;
            let mut spreadable = Vec::new();
            let mut unspreadable = Vec::new();
            for property in sequence.read(self.properties_of_type_completion(source, consumer))? {
                let property = self.property_symbol(&property, consumer)?;
                let key = sequence.read(self.literal_type_from_property_completion(
                    property,
                    TypeFlags::STRING_LITERAL
                        | TypeFlags::NUMBER_LITERAL
                        | TypeFlags::UNIQUE_SYMBOL,
                    false,
                    consumer,
                ))?;
                if !sequence.read(self.type_related_completion(
                    key,
                    omit,
                    RelationKind::Assignable,
                    consumer,
                ))? && self.property_declaration_modifiers(property, false, consumer)?
                    & (crate::syntax::modifiers::PRIVATE | crate::syntax::modifiers::PROTECTED)
                    == 0
                    && self.is_spreadable_property(property, consumer)?
                {
                    spreadable.push(property);
                } else {
                    unspreadable.push(key);
                }
            }
            if sequence
                .read(self.generic_flags_completion(source, consumer))?
                .0
                || sequence
                    .read(self.generic_flags_completion(omit, consumer))?
                    .1
            {
                if !unspreadable.is_empty() {
                    let mut keys = vec![omit];
                    keys.extend(unspreadable);
                    omit =
                        sequence.read(self.union_type_ex_completion(keys, None, None, consumer))?;
                }
                if self.model.has_type_flags(omit, TypeFlags::NEVER) {
                    return Ok(source);
                }
                let Some(alias) =
                    sequence.read(self.global_type_alias_completion("Omit", 2, consumer))?
                else {
                    return sequence.read(NativeCompletion::recovered(self.model.native_error_type(), super::super::invalid_at_location(
                    self.inputs, consumer, SemanticFailureReason::ErrorType,
                    "native object rest consumes errorType because global Omit is unavailable",
                ).into()));
                };
                let declared =
                    sequence.read(self.declared_reference_type_completion(alias, consumer))?;
                return sequence.read(self.type_alias_instantiation_completion(
                    alias,
                    declared,
                    vec![source, omit],
                    None,
                    consumer,
                ));
            }
            let mut properties = Vec::with_capacity(spreadable.len());
            for property in spreadable {
                let spread = sequence
                    .read(self.create_spread_symbol_completion(property, false, consumer))?;
                properties.push(self.project_property_symbol(spread, consumer)?);
            }
            let indexes = sequence.read(self.index_infos_of_type_completion(source, consumer))?;
            let properties: Arc<[_]> = Arc::from(properties);
            self.new_resolved_anonymous_type(
                symbol,
                Some(super::structure::MemberTable::Projected(properties.clone())),
                MembersView {
                    properties,
                    call_signatures: Arc::from([]),
                    construct_signatures: Arc::from([]),
                    index_signatures: indexes.into(),
                },
                ObjectFlags::OBJECT_REST_TYPE,
                consumer,
            )
        })();
        sequence.finish(result)
    }
}
