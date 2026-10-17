//! Intermediate assignment types precede completed-reference admission.

use super::super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, SpecialName, TypeFlags, TypeId,
};
use super::super::super::source::{AssignmentProjection, AssignmentTypeRecipe};
use super::super::Evaluator;
use super::super::facts::TypeFacts;
use super::super::relations::RelationKind;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::types::PrimitiveName;

enum AssignmentWork {
    Visit(Site),
    Project {
        projection: AssignmentProjection,
        initializer: Option<Site>,
        location: Location,
    },
    Default {
        initializer: Site,
        location: Location,
    },
}

impl Evaluator<'_, '_> {
    /// `getInitialOrAssignedType` / `getNarrowableTypeForReference` in Normal
    /// mode. Current scalar types have the native unchanged early result;
    /// generic constraint/contextual questions remain consumed dependencies.
    pub(super) fn initial_or_assigned_type_completion(
        &mut self,
        file: FileId,
        target: Site,
        reference: super::ReferenceNode,
        location: &Location,
    ) -> NativeResult<TypeId> {
        self.initial_or_assigned_type_worker_completion(file, target)?
            .and_then(|ty| {
                self.narrowable_type_for_reference_completion(
                    ty,
                    reference,
                    super::super::contextual::CheckMode::NORMAL,
                    location,
                )
            })
    }

    /// `getInitialType` / `getAssignedType`: grammar-parent recursion is an
    /// explicit continuation stack. Only the outer reference consumer asks
    /// getNarrowableTypeForReference; intermediate producers retain raw types.
    fn initial_or_assigned_type_worker_completion(
        &mut self,
        file: FileId,
        target: Site,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let mut work = vec![AssignmentWork::Visit(target)];
            let mut result = None;
            while let Some(step) = work.pop() {
                match step {
                    AssignmentWork::Visit(site) => {
                        match self.source.assignment_type_recipe(file, site)? {
                            AssignmentTypeRecipe::Initializer(site) => {
                                result = Some(
                                    sequence
                                        .read(self.type_of_initializer_completion(file, site))?,
                                );
                            }
                            AssignmentTypeRecipe::Expression(site) => {
                                result = Some(
                                    sequence
                                        .read(self.type_of_expression_completion(file, site))?,
                                );
                            }
                            AssignmentTypeRecipe::String => {
                                result = Some(self.model.primitive(PrimitiveName::String));
                            }
                            AssignmentTypeRecipe::Undefined => {
                                result = Some(self.model.special(SpecialName::Undefined));
                            }
                            AssignmentTypeRecipe::Error(location) => {
                                result = Some(sequence.read(NativeCompletion::recovered(self.model.native_error_type(), self.assignment_error(
                                &location,
                                "native initial/assigned type construction consumed errorType",
                            )))?);
                            }
                            AssignmentTypeRecipe::Projected {
                                source,
                                projection,
                                initializer,
                                location,
                            } => {
                                work.push(AssignmentWork::Project {
                                    projection,
                                    initializer,
                                    location,
                                });
                                work.push(AssignmentWork::Visit(source));
                            }
                            AssignmentTypeRecipe::Default {
                                source,
                                initializer,
                                location,
                            } => {
                                work.push(AssignmentWork::Default {
                                    initializer,
                                    location,
                                });
                                work.push(AssignmentWork::Visit(source));
                            }
                            AssignmentTypeRecipe::ForOf {
                                expression,
                                asynchronous,
                            } => {
                                result =
                                    Some(sequence.read(self.assignment_for_of_type_completion(
                                        file,
                                        expression,
                                        asynchronous,
                                    ))?);
                            }
                        }
                    }
                    AssignmentWork::Project {
                        projection,
                        initializer,
                        location,
                    } => {
                        let source = result.take().ok_or_else(|| {
                            super::super::super::internal(
                                "assignment projection has no parent result",
                            )
                        })?;
                        let ty = match projection {
                            AssignmentProjection::Property(name) => {
                                sequence.read(self.destructured_property_type_completion(
                                    file, source, name, &location,
                                ))?
                            }
                            AssignmentProjection::ArrayElement(index) => {
                                sequence.read(self.destructured_array_element_type_completion(
                                    source, index, &location,
                                ))?
                            }
                            AssignmentProjection::Spread => {
                                let element =
                                    sequence.read(self.checked_iteration_type_completion(
                                        source,
                                        super::super::iteration::IterationUse::Destructuring,
                                        None,
                                        &location,
                                    ))?;
                                sequence.read(self.array_type_completion(element, &location))?
                            }
                        };
                        result = Some(ty);
                        if let Some(initializer) = initializer {
                            work.push(AssignmentWork::Default {
                                initializer,
                                location,
                            });
                        }
                    }
                    AssignmentWork::Default {
                        initializer,
                        location,
                    } => {
                        let source = result.take().ok_or_else(|| {
                            super::super::super::internal("assignment default has no source result")
                        })?;
                        let source = sequence.read(
                            self.non_undefined_assignment_type_completion(source, &location),
                        )?;
                        let fallback =
                            sequence.read(self.type_of_expression_completion(file, initializer))?;
                        result = Some(sequence.read(self.union_type_ex_completion(
                            vec![source, fallback],
                            None,
                            None,
                            &location,
                        ))?);
                    }
                }
            }
            result.ok_or_else(|| {
                super::super::super::internal("assignment construction has no result").into()
            })
        })();
        sequence.finish(result)
    }

    /// `getNonUndefinedType`: generic undefined constraints need their actual
    /// base/map operation before the shared native TypeFacts filter.
    pub(in crate::project::semantic::evaluation) fn non_undefined_assignment_type_completion(
        &mut self,
        ty: TypeId,
        location: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let types = self
                .model
                .union_constituents(ty)
                .unwrap_or_else(|| vec![ty].into());
            let mut generic_undefined = false;
            for &part in types.iter() {
                if self.model.has_type_flags(part, TypeFlags::INSTANTIABLE)
                    && let Some(constraint) =
                        sequence.read(self.base_constraint_completion(part, location))?
                    && self
                        .model
                        .maybe_type_of_kind(constraint, TypeFlags::UNDEFINED)
                {
                    generic_undefined = true;
                    break;
                }
            }
            let ty = if generic_undefined {
                sequence.read(self.map_type_completion(ty, location, |checker, part| {
                    if checker.model.has_type_flags(part, TypeFlags::INSTANTIABLE) {
                        Ok(checker
                            .base_constraint_completion(part, location)?
                            .map(|base| base.unwrap_or(part)))
                    } else {
                        Ok(NativeCompletion::clean(part))
                    }
                }))?
            } else {
                ty
            };
            sequence.read(self.type_with_facts_completion(ty, TypeFacts::NOT_UNDEFINED, location))
        })();
        sequence.finish(result)
    }

    /// `getTypeOfDestructuredProperty`: name acquisition follows the parent
    /// type, then property precedes index. Neither missing lookup is Any.
    fn destructured_property_type_completion(
        &mut self,
        file: FileId,
        ty: TypeId,
        name: Site,
        location: &Location,
    ) -> NativeResult<TypeId> {
        self.literal_type_of_property_name_completion(file, name)?.and_then(|key| {
        let Some(name) = self.property_key_from_type(key)? else {
            return NativeCompletion::recovered(self.model.native_error_type(), self.assignment_error(
                location,
                "native destructured property name consumed errorType",
            ));
        };
        self.type_of_property_of_type_by_key_completion(ty, &name, location)?.and_then(|property| {
            if let Some(ty) = property { return Ok(NativeCompletion::clean(ty)); }
            self.applicable_index_info_for_key_completion(ty, &name, location)?.and_then(|index| {
                if let Some(index) = index {
                    return self.demand_completion(index.value_type)?.and_then(|ty| {
                        self.include_undefined_in_index_signature_completion(ty, location)
                    });
                }
                NativeCompletion::recovered(self.model.native_error_type(), self.assignment_error(
                    location, "native missing destructured property/index consumed errorType"))
            })
        })
        })
    }

    /// `getTypeOfDestructuredArrayElement`: the complete tuple-like question
    /// and projection precede the ordinary iteration result.
    fn destructured_array_element_type_completion(
        &mut self,
        ty: TypeId,
        index: usize,
        location: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let subjects = self.model.union_constituents(ty);
            let mut tuple_like = true;
            for &subject in subjects.as_deref().unwrap_or(&[ty]) {
                if !sequence.read(self.is_tuple_like_type_completion(subject, location))? {
                    tuple_like = false;
                    break;
                }
            }
            if tuple_like {
                if let Some(element) =
                    sequence.read(self.tuple_element_type_completion(ty, index, location))?
                {
                    return Ok(element);
                }
            }
            let element = sequence.read(self.checked_iteration_type_completion(
                ty,
                super::super::iteration::IterationUse::Destructuring,
                None,
                location,
            ))?;
            sequence.read(self.include_undefined_in_index_signature_completion(element, location))
        })();
        sequence.finish(result)
    }

    /// `getAssignmentReducedType`: identity and assigned-Never precede the
    /// actual ordered type-pair cache. Ordinary False is not a failure.
    pub(super) fn assignment_reduced_type_completion(
        &mut self,
        declared: TypeId,
        assigned: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if declared == assigned {
            return Ok(NativeCompletion::clean(declared));
        }
        if self.model.has_type_flags(assigned, TypeFlags::NEVER) {
            return Ok(NativeCompletion::clean(assigned));
        }
        let key = (declared, assigned);
        if let Some(result) = self.reads.assignments.get(&key) {
            return Ok(result.clone());
        }
        // Native publishes only after the complete worker. A recovered value
        // is publishable; an interruption supplies no replacement cache value.
        let result =
            self.assignment_reduced_type_worker_completion(declared, assigned, consumer)?;
        self.reads.assignments.insert(key, result.clone());
        Ok(result)
    }

    /// `getAssignmentReducedTypeWorker`: filter in native constituent order,
    /// preserve fresh Boolean literals, then validate the heuristic result.
    fn assignment_reduced_type_worker_completion(
        &mut self,
        declared: TypeId,
        assigned: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.filter_type_completion(declared, |checker, subject| {
            checker.type_maybe_assignable_to_completion(assigned, subject, consumer)
        })?
        .and_then(|filtered| {
            let reduced = if self
                .model
                .has_type_flags(assigned, TypeFlags::BOOLEAN_LITERAL)
                && self.model.is_fresh_literal(assigned)
            {
                self.map_type_completion(filtered, consumer, |checker, ty| {
                    Ok(NativeCompletion::clean(checker.model.fresh_literal(ty)))
                })?
            } else {
                NativeCompletion::clean(filtered)
            };
            reduced.and_then(|reduced| {
                Ok(self
                    .raw_type_related_completion(
                        assigned,
                        reduced,
                        RelationKind::Assignable,
                        consumer,
                    )?
                    .map(|related| if related { reduced } else { declared }))
            })
        })
    }

    /// `typeMaybeAssignableTo`: source unions ask Some in native order.
    fn type_maybe_assignable_to_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if let Some(types) = self.model.union_constituents(source) {
            let mut sequence = NativeSequence::new();
            let result = (|| -> Result<bool, DemandError> {
                for &source in types.iter() {
                    if sequence.read(self.raw_type_related_completion(
                        source,
                        target,
                        RelationKind::Assignable,
                        consumer,
                    ))? {
                        return Ok(true);
                    }
                }
                Ok(false)
            })();
            sequence.finish(result)
        } else {
            self.raw_type_related_completion(source, target, RelationKind::Assignable, consumer)
        }
    }
}
