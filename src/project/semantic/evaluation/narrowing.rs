//! Native baseline substitution is distinct from excluded use-site narrowing.

use super::super::model::{NativeResult, NativeSequence, TypeFlags, TypeId, TypeView};
use super::super::source::ConstraintPosition;
use super::Evaluator;
use super::contextual::{CheckMode, ContextFlags};
use super::reads::ReferenceNode;
use crate::references::Location;

impl Evaluator<'_, '_> {
    /// `getNarrowableTypeForReference`: NoInfer unwrapping precedes mode;
    /// Absence of Inferential mode consumes the union-constraint predicate, then exact
    /// position/context questions. An unchanged result preserves identity.
    pub(super) fn narrowable_type_for_reference_completion(
        &mut self,
        mut ty: TypeId,
        reference: impl Into<ReferenceNode>,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let reference = reference.into();
        let file = reference.file;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.is_no_infer_type(ty) {
                let TypeView::Substitution(substitution) = self.model.get(ty)? else {
                    unreachable!()
                };
                ty = substitution.base;
            }
            if mode.contains(CheckMode::INFERENTIAL)
                || !sequence.read(self.some_generic_union_constraint_completion(ty, consumer))?
            {
                return Ok(ty);
            }
            let syntax_position = self.reference_position(reference)?;
            let position = match self.source.constraint_position(file, syntax_position)? {
                ConstraintPosition::No => false,
                ConstraintPosition::Yes => true,
                ConstraintPosition::ElementIndex(index) => {
                    if sequence.read(self.some_generic_without_nullable_completion(ty, consumer))? {
                        let index =
                            sequence.read(self.type_of_expression_completion(file, index))?;
                        !sequence
                            .read(self.generic_flags_completion(index, consumer))?
                            .1
                    } else {
                        true
                    }
                }
            };
            let substitute = if position {
                true
            } else if !self
                .source
                .reference_has_context_role(file, syntax_position)?
            {
                false
            } else if let Some(context) = sequence.read(self.contextual_type_completion(
                reference,
                if mode.contains(CheckMode::REST_BINDING_ELEMENT) {
                    ContextFlags::SKIP_BINDING_PATTERNS
                } else {
                    ContextFlags::NONE
                },
            ))? {
                let (object, index) =
                    sequence.read(self.generic_flags_completion(context, consumer))?;
                !object && !index
            } else {
                false
            };
            if substitute {
                sequence.read(self.map_base_constraints_completion(ty, consumer))
            } else {
                Ok(ty)
            }
        })();
        sequence.finish(result)
    }

    /// `someType`: ordered union search for a generic union constraint
    /// stops on its first true predicate; false never erases consumed errors.
    fn some_generic_union_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let types = self.model.union_constituents(ty);
            for &ty in types.as_deref().unwrap_or(&[ty]) {
                if sequence.read(self.generic_union_constraint_completion(ty, consumer))? {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isGenericTypeWithUnionConstraint` searches intersection constituents
    /// before asking the native instantiable leaf's base constraint.
    fn generic_union_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(types) = self.model.intersection_constituents(ty) {
                for &child in types.iter() {
                    if sequence.read(self.generic_union_constraint_completion(child, consumer))? {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            if !self.model.has_type_flags(ty, TypeFlags::INSTANTIABLE) {
                return Ok(false);
            }
            let constraint = sequence
                .read(self.base_constraint_completion(ty, consumer))?
                .unwrap_or(ty);
            Ok(self
                .model
                .has_type_flags(constraint, TypeFlags::NULLABLE | TypeFlags::UNION))
        })();
        sequence.finish(result)
    }

    /// `someType` for the index exception: the caller asks the index argument
    /// only after this ordered search succeeds.
    fn some_generic_without_nullable_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let types = self.model.union_constituents(ty);
            for &ty in types.as_deref().unwrap_or(&[ty]) {
                if sequence.read(self.generic_without_nullable_completion(ty, consumer))? {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isGenericTypeWithoutNullableConstraint` recursively searches only
    /// intersections; the surrounding `someType` owns union traversal.
    fn generic_without_nullable_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(types) = self.model.intersection_constituents(ty) {
                for &child in types.iter() {
                    if sequence.read(self.generic_without_nullable_completion(child, consumer))? {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            if !self.model.has_type_flags(ty, TypeFlags::INSTANTIABLE) {
                return Ok(false);
            }
            let constraint = sequence
                .read(self.base_constraint_completion(ty, consumer))?
                .unwrap_or(ty);
            Ok(!self
                .model
                .maybe_type_of_kind(constraint, TypeFlags::NULLABLE))
        })();
        sequence.finish(result)
    }

    /// `mapType` with `getBaseConstraintOrType`: Boolean uses its semantic
    /// constituents, and unchanged unions retain their original identity.
    pub(super) fn map_base_constraints_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> super::super::model::NativeResult<TypeId> {
        self.map_type_completion(ty, consumer, |checker, child| {
            Ok(checker
                .base_constraint_completion(child, consumer)?
                .map(|base| base.unwrap_or(child)))
        })
    }
}
