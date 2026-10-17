//! Template inference ranks actual constraint constituents in native order.
use super::super::super::model::{
    DemandError, LiteralValue, NativeResult, NativeSequence, SpecialName, TemplateLiteralView,
    TypeFlags, TypeId, TypeView,
};
use super::{Evaluator, InferencePass, RelationKind};
use crate::references::Location;
use crate::types::PrimitiveName;

impl Evaluator<'_, '_> {
    /// `inferToTemplateLiteralType`: failed character extraction infers Never
    /// only when every delimiter is empty. Matching literals can prefer a
    /// constraint-compatible primitive before ordinary candidate collection.
    pub(super) fn infer_to_template_completion(
        &mut self,
        source: TypeId,
        target: &TemplateLiteralView,
        pass: &mut InferencePass,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let matches =
                sequence.read(self.infer_template_matches_completion(source, target, consumer))?;
            let matches = matches.filter(|matches| !matches.is_empty());
            if matches.is_none() && !target.texts.iter().all(|text| text.is_empty()) {
                return Ok(());
            }
            for (index, &target) in target.types.iter().enumerate() {
                let source = match &matches {
                    Some(matches) => matches[index],
                    None => self.model.special(SpecialName::Never),
                };
                if let TypeView::Literal {
                    base: PrimitiveName::String,
                    value,
                } = self.model.get(source)?
                    && self.model.has_type_flags(target, TypeFlags::TYPE_VARIABLE)
                {
                    let parameter = self.inference.contexts[pass.context.index()]
                        .infos
                        .iter()
                        .find(|info| info.parameter == target)
                        .map(|info| info.parameter);
                    if let Some(parameter) = parameter
                        && let Some(constraint) =
                            sequence.read(self.base_constraint_completion(parameter, consumer))?
                        && !self.model.has_type_flags(constraint, TypeFlags::ANY)
                    {
                        let constraints = self
                            .model
                            .union_constituents(constraint)
                            .map(|types| types.to_vec())
                            .unwrap_or_else(|| vec![constraint]);
                        let mut flags = constraints.iter().fold(TypeFlags::NONE, |flags, &ty| {
                            flags | self.model.type_flags(ty)
                        });
                        if flags & TypeFlags::STRING == TypeFlags::NONE {
                            if flags & TypeFlags::NUMBER_LIKE != TypeFlags::NONE
                                && !self.literal_is_valid_number_string(&value, true, consumer)?
                            {
                                flags = flags & !TypeFlags::NUMBER_LIKE;
                            }
                            if flags & TypeFlags::BIGINT_LIKE != TypeFlags::NONE
                                && !self.literal_is_valid_bigint_string(&value, true, consumer)?
                            {
                                flags = flags & !TypeFlags::BIGINT_LIKE;
                            }
                            let mut matching = self.model.special(SpecialName::Never);
                            for right in constraints {
                                matching =
                                    sequence.read(self.choose_template_constraint_completion(
                                        matching, right, source, &value, flags, consumer,
                                    ))?;
                            }
                            if !self.model.has_type_flags(matching, TypeFlags::NEVER) {
                                sequence.read(self.infer_from_types_completion(
                                    matching, target, pass, consumer,
                                ))?;
                                continue;
                            }
                        }
                    }
                }
                sequence.read(self.infer_from_types_completion(source, target, pass, consumer))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// The `inferToTemplateLiteralType` choose closure gives broad string and
    /// pattern matches precedence over literal and numeric alternatives. This
    /// is a left fold, not an unordered search for a compatible constituent.
    fn choose_template_constraint_completion(
        &mut self,
        left: TypeId,
        right: TypeId,
        source: TypeId,
        text: &LiteralValue,
        flags: TypeFlags,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if !self.model.has_type_flags(right, flags)
                || self.model.has_type_flags(left, TypeFlags::STRING)
            {
                return Ok(left);
            }
            if self.model.has_type_flags(right, TypeFlags::STRING) {
                return Ok(source);
            }
            if self.model.has_type_flags(left, TypeFlags::TEMPLATE_LITERAL) {
                return Ok(left);
            }
            if let TypeView::TemplateLiteral(template) = self.model.get(right)?
                && sequence.read(self.template_type_matches_completion(
                    source,
                    &template,
                    consumer,
                    &mut |checker, source, target| {
                        checker.type_related_completion(
                            source,
                            target,
                            RelationKind::Assignable,
                            consumer,
                        )
                    },
                ))?
            {
                return Ok(source);
            }
            if self.model.has_type_flags(left, TypeFlags::STRING_MAPPING) {
                return Ok(left);
            }
            if let TypeView::StringMapping(mapping) = self.model.get(right)? {
                if mapping
                    .kind
                    .apply(text, self.control.as_ref())
                    .semantic_equal(text, self.control.as_ref())
                    .map_err(|error| self.literal_value_failure(error, consumer))?
                {
                    return Ok(source);
                }
            }
            if self.model.has_type_flags(left, TypeFlags::STRING_LITERAL) {
                return Ok(left);
            }
            if let TypeView::Literal {
                base: PrimitiveName::String,
                value,
            } = self.model.get(right)?
            {
                if self.template_constraint_literal_equal(&value, text, consumer)? {
                    return Ok(right);
                }
            }
            if self.model.has_type_flags(left, TypeFlags::NUMBER) {
                return Ok(left);
            }
            if self.model.has_type_flags(right, TypeFlags::NUMBER) {
                return Ok(self.model.regular_literal_type(
                    PrimitiveName::Number,
                    crate::numbers::number_to_string(crate::numbers::number_from_string(
                        self.literal_scalar_text(text, consumer)?,
                    )),
                ));
            }
            if self.model.has_type_flags(left, TypeFlags::ENUM) {
                return Ok(left);
            }
            if self.model.has_type_flags(right, TypeFlags::ENUM) {
                return Ok(self.model.regular_literal_type(
                    PrimitiveName::Number,
                    crate::numbers::number_to_string(crate::numbers::number_from_string(
                        self.literal_scalar_text(text, consumer)?,
                    )),
                ));
            }
            if self.model.has_type_flags(left, TypeFlags::NUMBER_LITERAL) {
                return Ok(left);
            }
            if let TypeView::Literal {
                base: PrimitiveName::Number,
                value,
            } = self.model.get(right)?
                && crate::numbers::number_from_string(self.literal_scalar_text(&value, consumer)?)
                    == crate::numbers::number_from_string(self.literal_scalar_text(text, consumer)?)
            {
                return Ok(right);
            }
            if self.model.has_type_flags(left, TypeFlags::BIGINT) {
                return Ok(left);
            }
            if self.model.has_type_flags(right, TypeFlags::BIGINT) {
                let value = crate::numbers::bigint_from_string(
                    self.literal_scalar_text(text, consumer)?,
                    true,
                )
                .ok_or_else(|| {
                    super::super::super::internal("validated template bigint lost its parse")
                })?;
                return Ok(self
                    .model
                    .regular_literal_type(PrimitiveName::Bigint, value));
            }
            if self.model.has_type_flags(left, TypeFlags::BIGINT_LITERAL) {
                return Ok(left);
            }
            if let TypeView::Literal {
                base: PrimitiveName::Bigint,
                value,
            } = self.model.get(right)?
            {
                if self.template_constraint_literal_equal(&value, text, consumer)? {
                    return Ok(right);
                }
            }
            if self.model.has_type_flags(left, TypeFlags::BOOLEAN) {
                return Ok(left);
            }
            if self.model.has_type_flags(right, TypeFlags::BOOLEAN) {
                return Ok(if text.equals_text("true") {
                    self.model
                        .regular_literal_type(PrimitiveName::Boolean, "true")
                } else if text.equals_text("false") {
                    self.model
                        .regular_literal_type(PrimitiveName::Boolean, "false")
                } else {
                    self.model.primitive(PrimitiveName::Boolean)
                });
            }
            if self.model.has_type_flags(left, TypeFlags::BOOLEAN_LITERAL) {
                return Ok(left);
            }
            if let TypeView::Literal {
                base: PrimitiveName::Boolean,
                value,
            } = self.model.get(right)?
                && (value.equals_text("true") && text.equals_text("true")
                    || value.equals_text("false") && text.equals_text("false"))
            {
                return Ok(right);
            }
            if self.model.has_type_flags(left, TypeFlags::UNDEFINED) {
                return Ok(left);
            }
            if self.model.has_type_flags(right, TypeFlags::UNDEFINED)
                && text.equals_text("undefined")
            {
                return Ok(right);
            }
            if self.model.has_type_flags(left, TypeFlags::NULL) {
                return Ok(left);
            }
            if self.model.has_type_flags(right, TypeFlags::NULL) && text.equals_text("null") {
                return Ok(right);
            }
            Ok(left)
        })();
        sequence.finish(result)
    }

    /// `inferToTemplateLiteralType` compares the captured string with string
    /// literal values and decimal BigInt spellings through the same reached
    /// native string comparison used by literal relations.
    fn template_constraint_literal_equal(
        &self,
        left: &LiteralValue,
        right: &LiteralValue,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        left.semantic_equal(right, self.control.as_ref())
            .map_err(|error| self.literal_value_failure(error, consumer))
    }
}
