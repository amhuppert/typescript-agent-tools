//! Native template matching keeps literal splitting independent of the active
//! relation. Placeholder validation invokes the caller's actual comparer and
//! retains its completion before deciding whether to inspect the next hole.
use super::super::super::model::{
    LiteralError, LiteralPosition, LiteralValue, NativeCompletion, NativeResult, NativeSequence,
    SpecialName, TemplateLiteralView, TypeFlags, TypeId, TypeView,
};
use super::super::{Evaluator, relations::RelationKind};
use crate::references::Location;
use crate::types::PrimitiveName;

/// `templateLiteralTypesDefinitelyUnrelated` compares overlapping prefix and
/// suffix bytes. These are byte comparisons, not potentially invalid str slices.
pub(in crate::project::semantic::evaluation) fn definitely_unrelated(
    source: &TemplateLiteralView,
    target: &TemplateLiteralView,
    control: Option<&crate::resources::Control>,
) -> Result<bool, LiteralError> {
    Ok(
        !source.texts[0].compatible_prefix(&target.texts[0], control)?
            || !source
                .texts
                .last()
                .expect("template has a tail")
                .compatible_suffix(target.texts.last().expect("template has a tail"), control)?,
    )
}

impl Evaluator<'_, '_> {
    /// `inferTypesFromTemplateLiteralType`: identical span lists ask ordinary
    /// Assignable constraints, independently of a surrounding relation session.
    /// This complete prepass precedes validation, including a first false match.
    pub(in crate::project::semantic::evaluation) fn infer_template_matches_completion(
        &mut self,
        source: TypeId,
        target: &TemplateLiteralView,
        consumer: &Location,
    ) -> NativeResult<Option<Vec<TypeId>>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            match self.model.get(source)? {
                TypeView::Literal {
                    base: PrimitiveName::String,
                    value,
                } => sequence.read(self.infer_literal_parts_completion(
                    &[value],
                    &[],
                    target,
                    consumer,
                )),
                TypeView::TemplateLiteral(source) => {
                    // Native slices.Equal is a semantic string observation. An
                    // unequal structural cache key cannot reject opaque spellings.
                    let equal_texts = source
                        .semantic_texts_equal(target, self.control.as_ref())
                        .map_err(|error| self.literal_value_failure(error, consumer))?;
                    if !equal_texts {
                        return sequence.read(self.infer_literal_parts_completion(
                            &source.texts,
                            &source.types,
                            target,
                            consumer,
                        ));
                    }
                    let mut result = Vec::with_capacity(source.types.len());
                    for (&source, &target) in source.types.iter().zip(target.types.iter()) {
                        let source_constraint = sequence
                            .read(self.base_constraint_completion(source, consumer))?
                            .unwrap_or(source);
                        let target_constraint = sequence
                            .read(self.base_constraint_completion(target, consumer))?
                            .unwrap_or(target);
                        result.push(
                            if sequence.read(self.type_related_completion(
                                source_constraint,
                                target_constraint,
                                RelationKind::Assignable,
                                consumer,
                            ))? {
                                source
                            } else {
                                sequence.read(self.string_like_type_completion(source, consumer))?
                            },
                        );
                    }
                    Ok(Some(result))
                }
                _ => Ok(None),
            }
        })();
        sequence.finish(result)
    }

    /// `getStringLikeTypeForType` preserves Any/StringLike identities and
    /// routes every other type through the canonical template factory.
    fn string_like_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self
            .model
            .has_type_flags(ty, TypeFlags::ANY | TypeFlags::STRING_LIKE)
        {
            return Ok(NativeCompletion::clean(ty));
        }
        self.template_literal_type_completion(
            vec![
                LiteralValue::from_text("", self.control.as_ref()),
                LiteralValue::from_text("", self.control.as_ref()),
            ]
            .into(),
            vec![ty],
            consumer,
        )
    }

    /// `inferFromLiteralPartsToTemplateLiteral`: delimiters match only within
    /// text spans. Empty delimiters consume one native Unicode code point or
    /// cross the next placeholder; failure remains distinct from an empty list.
    fn infer_literal_parts_completion(
        &mut self,
        texts: &[LiteralValue],
        types: &[TypeId],
        target: &TemplateLiteralView,
        consumer: &Location,
    ) -> NativeResult<Option<Vec<TypeId>>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let last = texts.len() - 1;
            let target_last = target.texts.len() - 1;
            let Some(mut position) = texts[0]
                .match_prefix_at(texts[0].start(), &target.texts[0], self.control.as_ref())
                .map_err(|error| self.literal_value_failure(error, consumer))?
            else {
                return Ok(None);
            };
            let Some(end) = texts[last]
                .match_suffix(&target.texts[target_last], self.control.as_ref())
                .map_err(|error| self.literal_value_failure(error, consumer))?
            else {
                return Ok(None);
            };
            // The two established boundaries replace the native byte-length
            // guard. An opaque atom's structural width is never its digit count.
            if last == 0 && position > end {
                return Ok(None);
            }
            let mut texts = texts.to_vec();
            texts[last] = texts[last]
                .slice(texts[last].start(), end, self.control.as_ref())
                .map_err(|error| self.literal_value_failure(error, consumer))?;
            let mut segment = 0;
            let mut result = Vec::with_capacity(target.types.len());
            for delimiter in &target.texts[1..target_last] {
                let (next_segment, next_position, after_delimiter) = if !delimiter.is_empty() {
                    let mut next = segment;
                    let mut start = position;
                    loop {
                        if let Some((at, after)) = texts[next]
                            .index_from(start, delimiter, self.control.as_ref())
                            .map_err(|error| self.literal_value_failure(error, consumer))?
                        {
                            break (next, at, after);
                        }
                        next += 1;
                        if next == texts.len() {
                            return Ok(None);
                        }
                        start = texts[next].start();
                    }
                } else if let Some(next) = texts[segment]
                    .next_rune(position, self.control.as_ref())
                    .map_err(|error| self.literal_value_failure(error, consumer))?
                {
                    (segment, next, next)
                } else if segment < last {
                    let next = texts[segment + 1].start();
                    (segment + 1, next, next)
                } else {
                    return Ok(None);
                };
                result.push(sequence.read(self.template_segment_match_completion(
                    &texts,
                    types,
                    (segment, position),
                    (next_segment, next_position),
                    consumer,
                ))?);
                segment = next_segment;
                position = after_delimiter;
            }
            result.push(sequence.read(self.template_segment_match_completion(
                &texts,
                types,
                (segment, position),
                (last, texts[last].end(self.control.as_ref())),
                consumer,
            ))?);
            Ok(Some(result))
        })();
        sequence.finish(result)
    }

    /// The native `addMatch` closure shares one factory for literal and
    /// placeholder-crossing slices, retaining all interior source placeholders.
    fn template_segment_match_completion(
        &mut self,
        texts: &[LiteralValue],
        types: &[TypeId],
        (segment, position): (usize, LiteralPosition),
        (next_segment, next_position): (usize, LiteralPosition),
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let next_text = &texts[next_segment];
        if segment == next_segment {
            let value = next_text
                .slice(position, next_position, self.control.as_ref())
                .map_err(|error| self.literal_value_failure(error, consumer))?;
            return Ok(NativeCompletion::clean(
                self.model.regular_string_literal_value(
                    value.combine_surrogate_pairs(self.control.as_ref()),
                ),
            ));
        }
        let mut result = Vec::with_capacity(next_segment - segment + 1);
        result.push(
            texts[segment]
                .slice(
                    position,
                    texts[segment].end(self.control.as_ref()),
                    self.control.as_ref(),
                )
                .map_err(|error| self.literal_value_failure(error, consumer))?,
        );
        result.extend(texts[segment + 1..next_segment].iter().cloned());
        result.push(
            next_text
                .slice(next_text.start(), next_position, self.control.as_ref())
                .map_err(|error| self.literal_value_failure(error, consumer))?,
        );
        self.template_literal_type_completion(
            result.into(),
            types[segment..next_segment].to_vec(),
            consumer,
        )
    }

    /// `isTypeMatchedByTemplateLiteralType` is shared by relation callbacks,
    /// inference selection, and union literal absorption. The caller supplies
    /// the comparer so an active native relation is never silently restarted.
    /// Its completion belongs to this matcher, not an enclosing captured sequence.
    pub(in crate::project::semantic::evaluation) fn template_type_matches_completion<F>(
        &mut self,
        source: TypeId,
        target: &TemplateLiteralView,
        consumer: &Location,
        compare: &mut F,
    ) -> NativeResult<bool>
    where
        F: FnMut(&mut Self, TypeId, TypeId) -> NativeResult<bool>,
    {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(matches) =
                sequence.read(self.infer_template_matches_completion(source, target, consumer))?
            else {
                return Ok(false);
            };
            for (&source, &target) in matches.iter().zip(target.types.iter()) {
                if !sequence.read(
                    self.valid_template_placeholder_completion(source, target, consumer, compare),
                )? {
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        sequence.finish(result)
    }

    /// `isValidTypeForTemplateLiteralPlaceholder` preserves its intersection
    /// exemption and comparison-before-literal-validation demand schedule.
    fn valid_template_placeholder_completion<F>(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
        compare: &mut F,
    ) -> NativeResult<bool>
    where
        F: FnMut(&mut Self, TypeId, TypeId) -> NativeResult<bool>,
    {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(types) = self.model.intersection_constituents(target) {
                let empty = self.model.empty_type_literal_type();
                for &target in types.iter() {
                    if target != empty
                        && !sequence.read(self.valid_template_placeholder_completion(
                            source, target, consumer, compare,
                        ))?
                    {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            if self.model.has_type_flags(target, TypeFlags::STRING)
                || sequence.read(compare(self, source, target))?
            {
                return Ok(true);
            }
            match self.model.get(source)? {
                TypeView::Literal {
                    base: PrimitiveName::String,
                    value,
                } => {
                    if self.model.has_type_flags(target, TypeFlags::NUMBER)
                        && self.literal_is_valid_number_string(&value, false, consumer)?
                        || self.model.has_type_flags(target, TypeFlags::BIGINT)
                            && self.literal_is_valid_bigint_string(&value, false, consumer)?
                    {
                        return Ok(true);
                    }
                    match self.model.get(target)? {
                        TypeView::Literal {
                            base: PrimitiveName::Boolean,
                            value: target,
                        } => Ok(value == target),
                        TypeView::Special(SpecialName::Null) => Ok(value.equals_text("null")),
                        TypeView::Special(SpecialName::Undefined) => {
                            Ok(value.equals_text("undefined"))
                        }
                        TypeView::StringMapping(_) => sequence.read(
                            self.member_of_string_mapping_completion(source, target, consumer),
                        ),
                        TypeView::TemplateLiteral(target) => {
                            sequence.read(self.template_type_matches_completion(
                                source, &target, consumer, compare,
                            ))
                        }
                        _ => Ok(false),
                    }
                }
                TypeView::TemplateLiteral(source) => {
                    if source.texts.len() == 2 && source.texts.iter().all(|text| text.is_empty()) {
                        sequence.read(compare(self, source.types[0], target))
                    } else {
                        Ok(false)
                    }
                }
                _ => Ok(false),
            }
        })();
        sequence.finish(result)
    }

    /// `isMemberOfStringMapping` compares actual factory identities after
    /// applying every target mapping, then checks the unmapped inner domain.
    pub(in crate::project::semantic::evaluation) fn member_of_string_mapping_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.has_type_flags(target, TypeFlags::ANY) {
                return Ok(true);
            }
            if self
                .model
                .has_type_flags(target, TypeFlags::STRING | TypeFlags::TEMPLATE_LITERAL)
            {
                return sequence.read(self.type_related_completion(
                    source,
                    target,
                    RelationKind::Assignable,
                    consumer,
                ));
            }
            if self.model.has_type_flags(target, TypeFlags::STRING_MAPPING) {
                let (mapped, inner) = sequence
                    .read(self.apply_target_string_mapping_completion(source, target, consumer))?;
                return Ok(mapped == source
                    && sequence.read(
                        self.member_of_string_mapping_completion(source, inner, consumer),
                    )?);
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `applyTargetStringMappingToSource` applies inner mappings before outer
    /// mappings and returns the actual innermost nonmapping target separately.
    fn apply_target_string_mapping_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<(TypeId, TypeId)> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let TypeView::StringMapping(mapping) = self.model.get(target)? else {
                return Err(super::super::super::internal(
                    "string mapping target lost its factory data",
                )
                .into());
            };
            let (source, inner) = if self
                .model
                .has_type_flags(mapping.target, TypeFlags::STRING_MAPPING)
            {
                sequence.read(self.apply_target_string_mapping_completion(
                    source,
                    mapping.target,
                    consumer,
                ))?
            } else {
                (source, mapping.target)
            };
            Ok((
                sequence.read(self.string_mapping_type_completion(
                    mapping.symbol,
                    mapping.kind,
                    source,
                    consumer,
                ))?,
                inner,
            ))
        })();
        sequence.finish(result)
    }
}
