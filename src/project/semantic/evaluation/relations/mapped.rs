//! Mapped relation alternatives remain in the active native Relater session.
use super::*;

impl Evaluator<'_, '_> {
    /// `mappedTypeRelatedTo`: constraints are contravariant, templates
    /// covariant, and remapped names compare actual instantiated identities.
    pub(super) fn mapped_type_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let kind = self.relations.session(session).kind;
            let related = if kind == RelationKind::Comparable {
                true
            } else if kind == RelationKind::Identity {
                let source = self.mapped_recipe(source)?;
                let target = self.mapped_recipe(target)?;
                source.readonly == target.readonly && source.optional == target.optional
            } else {
                self.combined_mapped_optionality(source, consumer)?
                    <= self.combined_mapped_optionality(target, consumer)?
            };
            if !related {
                return Ok(Ternary::False);
            }
            let target_constraint = self.mapped_constraint(target, consumer)?;
            let source_constraint = self.mapped_constraint(source, consumer)?;
            let unmeasurable = self.combined_mapped_optionality(source, consumer)? < 0;
            let source_constraint = self.instantiate_type(
                source_constraint,
                self.model.report_reliability_mapper(unmeasurable),
                consumer,
            )?;
            let result = sequence.read(self.related_in_session_completion(
                target_constraint,
                source_constraint,
                RecursionFlags::BOTH,
                IntersectionState::NONE,
                session,
                consumer,
            ))?;
            if result == Ternary::False {
                return Ok(result);
            }
            let source_parameter = self.mapped_parameter(source, consumer)?;
            let target_parameter = self.mapped_parameter(target, consumer)?;
            let mapper = self
                .model
                .mapper(vec![source_parameter], vec![target_parameter])?;
            let source_name = self
                .mapped_name(source, consumer)?
                .map(|ty| self.instantiate_type(ty, mapper, consumer))
                .transpose()?;
            let target_name = self
                .mapped_name(target, consumer)?
                .map(|ty| self.instantiate_type(ty, mapper, consumer))
                .transpose()?;
            if source_name != target_name {
                return Ok(Ternary::False);
            }
            let source_template = self.mapped_template(source, consumer)?;
            let source_template = self.instantiate_type(source_template, mapper, consumer)?;
            let target_template = self.mapped_template(target, consumer)?;
            Ok(result.and(sequence.read(self.related_in_session_completion(
                source_template,
                target_template,
                RecursionFlags::BOTH,
                IntersectionState::NONE,
                session,
                consumer,
            ))?))
        })();
        sequence.finish(result)
    }

    /// `structuredTypeRelatedToWorker`'s generic mapped target alternative.
    /// Failed optional/property alternatives preserve their error chain before
    /// the later generic-to-generic comparison is attempted.
    pub(super) fn generic_mapped_target_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        state: &mut StructuredComparison,
        consumer: &Location,
    ) -> NativeResult<Option<Ternary>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Ternary>, DemandError> {
            if !self.is_generic_mapped_type(target, consumer)? {
                return Ok(None);
            }
            let session = state.comparer.session;
            let recipe = self.mapped_recipe(target)?;
            let remapped = recipe.name.is_some();
            let template = self.mapped_template(target, consumer)?;
            if recipe.optional < 0 {
                return Ok(None);
            }
            if !remapped
                && let TypeView::IndexedAccess(access) = self.model.get(template)?
                && access.object_type == source
                && access.index_type == self.mapped_parameter(target, consumer)?
            {
                return Ok(Some(Ternary::True));
            }
            if self.is_generic_mapped_type(source, consumer)? {
                return Ok(None);
            }
            let target_keys = if remapped {
                self.mapped_name(target, consumer)?
                    .ok_or_else(|| super::super::super::internal("mapped name syntax lacks type"))?
            } else {
                self.mapped_constraint(target, consumer)?
            };
            let source_keys =
                self.index_type_ex(source, IndexFlags::NO_INDEX_SIGNATURES, consumer)?;
            let filtered = if recipe.optional > 0 {
                Some(self.intersection_type(vec![target_keys, source_keys], consumer)?)
            } else {
                None
            };
            let applicable = if let Some(filtered) = filtered {
                !self.model.has_type_flags(filtered, TypeFlags::NEVER)
            } else {
                sequence.read(self.related_in_session_completion(
                    target_keys,
                    source_keys,
                    RecursionFlags::BOTH,
                    IntersectionState::NONE,
                    session.reporting(false),
                    consumer,
                ))? != Ternary::False
            };
            if applicable {
                let template = self.mapped_template(target, consumer)?;
                let parameter = self.mapped_parameter(target, consumer)?;
                let non_null = self
                    .model
                    .extract_types_of_kind(template, !TypeFlags::NULLABLE);
                let result = if !remapped
                    && let TypeView::IndexedAccess(access) = self.model.get(non_null)?
                    && access.index_type == parameter
                {
                    sequence.read(self.related_in_session_completion(
                        source,
                        access.object_type,
                        RecursionFlags::TARGET,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?
                } else {
                    let indexing = if remapped {
                        filtered.unwrap_or(target_keys)
                    } else if let Some(filtered) = filtered {
                        self.intersection_type(vec![filtered, parameter], consumer)?
                    } else {
                        parameter
                    };
                    let indexed = self.indexed_access_type(source, indexing, consumer)?;
                    sequence.read(self.related_in_session_completion(
                        indexed,
                        template,
                        RecursionFlags::BOTH,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?
                };
                if result != Ternary::False {
                    return Ok(Some(result));
                }
            }
            state.original_chain = self.relations.session(session).error.chain.clone();
            self.relations.session_mut(session).error = state.saved_error.clone();
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `structuredTypeRelatedToWorker`'s mapped-source/type-parameter target
    /// proof is attempted before Comparable's type-parameter constraint case.
    pub(super) fn mapped_source_parameter_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Option<Ternary>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Ternary>, DemandError> {
            if !self
                .model
                .object_flags(source)?
                .intersects(ObjectFlags::MAPPED)
                || self.mapped_recipe(source)?.name.is_some()
            {
                return Ok(None);
            }
            let index = self.index_type_ex(target, IndexFlags::NONE, consumer)?;
            let constraint = self.mapped_constraint(source, consumer)?;
            if sequence.read(self.related_in_session_completion(
                index,
                constraint,
                RecursionFlags::BOTH,
                IntersectionState::NONE,
                session.reporting(false),
                consumer,
            ))? != Ternary::False
                && self.mapped_recipe(source)?.optional <= 0
            {
                let template = self.mapped_template(source, consumer)?;
                let parameter = self.mapped_parameter(source, consumer)?;
                let indexed = self.indexed_access_type(target, parameter, consumer)?;
                let result = sequence.read(self.related_in_session_completion(
                    template,
                    indexed,
                    RecursionFlags::BOTH,
                    IntersectionState::NONE,
                    session,
                    consumer,
                ))?;
                if result != Ternary::False {
                    return Ok(Some(result));
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }
}
