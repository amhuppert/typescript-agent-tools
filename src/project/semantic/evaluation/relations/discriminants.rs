//! The Relater's bounded discriminant product is distinct from the checker's
//! key-map lookup. It compares every matching target without reporting errors.

use crate::project::semantic::model::{NativeResult, NativeSequence};

use super::{
    Evaluator, HashSet, IntersectionState, RelationComparer, RelationInvocation, RelationKind,
    Ternary,
};
use crate::project::semantic::model::{DemandError, TypeFlags, TypeId};
use crate::references::Location;

impl Evaluator<'_, '_> {
    /// `typeRelatedToDiscriminatedType` first counts the complete product,
    /// then matches every combination and checks the remaining members. Its
    /// native 25-combination bound is ordinary False, not a resource failure.
    pub(super) fn discriminated_type_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let source_properties = self.properties_of_type(source, consumer)?;
            let properties =
                self.find_discriminant_properties(source_properties, target, consumer)?;
            if properties.is_empty() {
                return Ok(Ternary::False);
            }
            let mut combinations = 1usize;
            for property in &properties {
                let symbol = self.property_symbol(property, consumer)?;
                let ty =
                    sequence.read(self.non_missing_property_type_completion(symbol, consumer))?;
                let count = if let Some(types) = self.model.union_constituents(ty) {
                    types.len()
                } else if self.model.has_type_flags(ty, TypeFlags::NEVER) {
                    0
                } else {
                    1
                };
                // The running product is at most 25; dividing before multiplying
                // preserves the native rejection without overflowing host usize.
                if count == 0 || count > 25 / combinations {
                    return Ok(Ternary::False);
                }
                combinations *= count;
            }
            let mut types = Vec::new();
            let mut symbols = Vec::new();
            let mut names = Vec::new();
            let mut excluded = HashSet::new();
            for property in &properties {
                let symbol = self.property_symbol(property, consumer)?;
                let ty =
                    sequence.read(self.non_missing_property_type_completion(symbol, consumer))?;
                types.push(
                    self.model
                        .union_constituents(ty)
                        .map_or_else(|| vec![ty], |types| types.to_vec()),
                );
                let name = self.property_lookup_key(symbol, consumer)?;
                excluded.insert(name.clone());
                names.push(name);
                symbols.push(symbol);
            }
            let targets = self.model.union_constituents(target).ok_or_else(|| {
                super::super::super::internal(
                    "discriminated relation requires an actual target union",
                )
            })?;
            let comparer = RelationComparer {
                session: session.reporting(false),
                intersection: IntersectionState::NONE,
            };
            let skip_optional = self.inputs.semantic_options().strict_null_checks
                || self.relations.session(session).kind == RelationKind::Comparable;
            let mut matching = Vec::new();
            for combination in 0..combinations {
                let mut selected = vec![0; types.len()];
                let mut n = combination;
                for index in (0..types.len()).rev() {
                    selected[index] = n % types[index].len();
                    n /= types[index].len();
                }
                let mut matched = false;
                'target: for &ty in targets.iter() {
                    for (index, &symbol) in symbols.iter().enumerate() {
                        let Some(property) =
                            self.property_of_type_by_key(ty, &names[index], consumer)?
                        else {
                            continue 'target;
                        };
                        let target_symbol = self.property_symbol(&property, consumer)?;
                        if symbol == target_symbol {
                            continue;
                        }
                        if sequence.read(self.property_related_completion(
                            (source, symbol),
                            (target, target_symbol),
                            Some(types[index][selected[index]]),
                            comparer,
                            skip_optional,
                            consumer,
                        ))? == Ternary::False
                        {
                            continue 'target;
                        }
                    }
                    if !matching.contains(&ty) {
                        matching.push(ty);
                    }
                    matched = true;
                }
                if !matched {
                    return Ok(Ternary::False);
                }
            }
            let mut result = Ternary::True;
            for ty in matching {
                result = result.and(sequence.read(self.properties_related_completion(
                    source, ty, &excluded, false, comparer, consumer,
                ))?);
                if result != Ternary::False {
                    result = result.and(sequence.read(self.relation_signatures_completion(
                        source,
                        ty,
                        false,
                        IntersectionState::NONE,
                        comparer.session,
                        consumer,
                    ))?);
                    if result != Ternary::False {
                        result = result.and(sequence.read(self.relation_signatures_completion(
                            source,
                            ty,
                            true,
                            IntersectionState::NONE,
                            comparer.session,
                            consumer,
                        ))?);
                        if result != Ternary::False
                            && !(self.model.is_tuple_type(source) && self.model.is_tuple_type(ty))
                        {
                            result =
                                result.and(sequence.read(self.relation_indexes_completion(
                                    source,
                                    ty,
                                    false,
                                    IntersectionState::NONE,
                                    comparer.session,
                                    consumer,
                                ))?);
                        }
                    }
                }
                if result == Ternary::False {
                    return Ok(result);
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }
}
