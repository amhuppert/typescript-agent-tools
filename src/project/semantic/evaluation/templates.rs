//! Template construction, pattern classification and source checks share one
//! native owner; callers supply exact spans or established placeholder types.
pub(super) mod matching;
use super::super::model::{
    AliasSymbol, DemandError, LiteralValue, NativeResult, NativeSequence, SpecialName, TypeFlags,
    TypeId, TypeView,
};
use super::super::source::ExpressionRoot;
use super::Evaluator;
use super::relations::RelationKind;
use crate::references::Location;
use crate::resources::complete_or_abort;
use crate::types::PrimitiveName;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct TemplatesState {
    nodes: HashMap<ExpressionRoot, NativeResult<TypeId>>,
}

impl Evaluator<'_, '_> {
    /// `checkTemplateExpression`: each substitution is checked in Normal mode,
    /// including symbol-constraint and assignment questions, before constant
    /// evaluation or the short-circuit contextual template-type decision.
    pub(super) fn check_template_expression_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let _guard = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.template_expression_recipe(root)?;
            let constraint = self.model.template_constraint_type();
            let mut types = Vec::new();
            for expression in &recipe.expressions {
                let ty = sequence.read(self.check_expression_completion(
                    expression,
                    super::contextual::CheckMode::NORMAL,
                ))?;
                sequence.read(
                    self.maybe_type_of_kind_considering_base_constraint_completion(
                        ty,
                        TypeFlags::ES_SYMBOL_LIKE,
                        &expression.location,
                    ),
                )?;
                types.push(
                    if sequence.read(self.type_related_completion(
                        ty,
                        constraint,
                        RelationKind::Assignable,
                        &expression.location,
                    ))? {
                        ty
                    } else {
                        self.model.primitive(PrimitiveName::String)
                    },
                );
            }
            if !recipe.tagged
                && let Some(value) = sequence
                    .read(self.evaluate_constant_completion(root, Some(root), consumer))?
                    .value
            {
                let super::super::constants::ConstantValue::String(text) = value else {
                    return Err(super::super::internal(
                        "template evaluation produced a non-string",
                    )
                    .into());
                };
                let regular = self.model.regular_string_literal_value(text);
                return Ok(self.model.fresh_literal(regular));
            }
            let literal_context = sequence
                .read(self.is_const_context_completion(root, consumer))?
                || self.source.is_template_literal_context(root)?;
            let literal_context = if literal_context {
                true
            } else if let Some(ty) = sequence.read(
                self.contextual_type_completion(root, super::contextual::ContextFlags::NONE),
            )? {
                sequence.read(self.template_literal_contextual_type_completion(ty, consumer))?
            } else {
                false
            };
            if literal_context {
                sequence.read(self.template_literal_type_completion(recipe.texts, types, consumer))
            } else {
                Ok(self.model.primitive(PrimitiveName::String))
            }
        })();
        sequence.finish(result)
    }

    /// `someType(..., isTemplateLiteralContextualType)` distributes only a
    /// union; an intersection is inspected as one type with its own constraint.
    fn template_literal_contextual_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let _guard = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(types) = self.model.union_constituents(ty) {
                for &ty in types.iter() {
                    if sequence
                        .read(self.template_literal_contextual_type_completion(ty, consumer))?
                    {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            if self
                .model
                .has_type_flags(ty, TypeFlags::STRING_LITERAL | TypeFlags::TEMPLATE_LITERAL)
            {
                return Ok(true);
            }
            if self
                .model
                .has_type_flags(ty, TypeFlags::INSTANTIABLE_NON_PRIMITIVE)
                && let Some(constraint) =
                    sequence.read(self.base_constraint_completion(ty, consumer))?
            {
                return Ok(self
                    .model
                    .maybe_type_of_kind(constraint, TypeFlags::STRING_LIKE));
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `getTypeFromTemplateTypeNode`: acquire every placeholder in source order
    /// before normalization, even when an earlier placeholder reduces to Never.
    pub(super) fn type_from_template_node_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let _guard = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        if let Some(value) = self.templates.nodes.get(&root) {
            return value.clone();
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.template_type_recipe(root)?;
            let mut types = Vec::new();
            for node in &recipe.types {
                types.push(sequence.read(self.check_type_completion(node))?);
            }
            sequence.read(self.template_literal_type_completion(recipe.texts, types, consumer))
        })();
        let result = sequence.finish(result);
        self.templates.nodes.reserve(1);
        self.templates.nodes.insert(root, result.clone());
        result
    }

    /// `getTemplateLiteralType`: distribute the first union, enforce the native
    /// product limit, then flatten literal and nested-template spans. Invalid
    /// placeholder categories widen to String under the native rule; failures
    /// acquiring those categories remain failures.
    /// Applies `CombineSurrogatePairs` after concatenation at each
    /// surviving placeholder boundary. Source fragments can contain a lone
    /// half even when their normalized semantic text is representable.
    pub(super) fn template_literal_type_completion(
        &mut self,
        texts: Arc<[LiteralValue]>,
        types: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let control = self.control.clone();
        let _guard = control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if texts.len() != types.len() + 1 {
                return Err(super::super::internal("template span cardinality differs").into());
            }
            if let Some(index) = types.iter().position(|&ty| {
                self.model
                    .has_type_flags(ty, TypeFlags::NEVER | TypeFlags::UNION)
            }) {
                if sequence
                    .read(self.check_cross_product_union_completion(&types, consumer))?
                    .is_none()
                {
                    return Ok(self.model.native_error_type());
                }
                return sequence.read(self.map_type_completion(
                    types[index],
                    consumer,
                    |checker, child| {
                        let mut replaced = types.to_vec();
                        replaced[index] = child;
                        checker.template_literal_type_completion(texts.clone(), replaced, consumer)
                    },
                ));
            }
            if types.iter().any(|&ty| self.model.is_wildcard_type(ty)) {
                return Ok(self.model.wildcard_type());
            }
            enum Span {
                Text(LiteralValue),
                Type(TypeId),
            }
            let mut work = Vec::new();
            for index in (0..types.len()).rev() {
                work.push(Span::Text(texts[index + 1].clone()));
                work.push(Span::Type(types[index]));
            }
            let mut text = Vec::new();
            text.push(texts[0].clone());
            let mut new_types = Vec::new();
            let mut new_texts = Vec::new();
            while let Some(span) = work.pop() {
                match span {
                    Span::Text(value) => text.push(value),
                    Span::Type(ty) => match self.model.get(ty)? {
                        // `getTemplateStringForType`: literal payloads already
                        // hold their canonical spelling.
                        TypeView::Literal { value, .. } => text.push(value),
                        TypeView::Special(SpecialName::Null) => {
                            text.push(LiteralValue::from_text("null", self.control.as_ref()))
                        }
                        TypeView::Special(SpecialName::Undefined) => {
                            text.push(LiteralValue::from_text("undefined", self.control.as_ref()))
                        }
                        TypeView::TemplateLiteral(template) => {
                            text.push(template.texts[0].clone());
                            for index in (0..template.types.len()).rev() {
                                work.push(Span::Text(template.texts[index + 1].clone()));
                                work.push(Span::Type(template.types[index]));
                            }
                        }
                        _ if sequence
                            .read(self.is_generic_index_type_completion(ty, consumer))?
                            || self.is_pattern_literal_placeholder_type(ty)? =>
                        {
                            new_types.push(ty);
                            new_texts.push(
                                LiteralValue::concat(text.iter(), control.as_ref())
                                    .combine_surrogate_pairs(control.as_ref()),
                            );
                            // A wide run of literal spans has one retained Arc
                            // per span even when its normalized text is empty.
                            while !text.is_empty() {
                                text.pop();
                            }
                        }
                        _ => return Ok(self.model.primitive(PrimitiveName::String)),
                    },
                }
            }
            if new_types.is_empty() {
                return Ok(self.model.regular_string_literal_value(
                    LiteralValue::concat(text.iter(), control.as_ref())
                        .combine_surrogate_pairs(control.as_ref()),
                ));
            }
            new_texts.push(
                LiteralValue::concat(text.iter(), control.as_ref())
                    .combine_surrogate_pairs(control.as_ref()),
            );
            if new_texts.iter().all(|text| text.is_empty()) {
                if new_types
                    .iter()
                    .all(|&ty| self.model.has_type_flags(ty, TypeFlags::STRING))
                {
                    return Ok(self.model.primitive(PrimitiveName::String));
                }
                if new_types.len() == 1 && self.is_pattern_literal_type(new_types[0])? {
                    return Ok(new_types[0]);
                }
            }
            Ok(self.model.template_literal(new_texts, new_types))
        })();
        sequence.finish(result)
    }

    /// `isPatternLiteralPlaceholderType`: an intersection permits object tags
    /// but requires at least one placeholder, literal, or nullable constituent.
    pub(super) fn is_pattern_literal_placeholder_type(
        &self,
        ty: TypeId,
    ) -> Result<bool, DemandError> {
        let _guard = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        if let Some(types) = self.model.intersection_constituents(ty) {
            let mut seen = false;
            for &ty in types.iter() {
                if self
                    .model
                    .has_type_flags(ty, TypeFlags::LITERAL | TypeFlags::NULLABLE)
                    || self.is_pattern_literal_placeholder_type(ty)?
                {
                    seen = true;
                } else if !self.model.has_type_flags(ty, TypeFlags::OBJECT) {
                    return Ok(false);
                }
            }
            return Ok(seen);
        }
        Ok(self.model.has_type_flags(
            ty,
            TypeFlags::ANY | TypeFlags::STRING | TypeFlags::NUMBER | TypeFlags::BIGINT,
        ) || self.is_pattern_literal_type(ty)?)
    }

    /// `isPatternLiteralType` asks every placeholder; free type variables make
    /// a generic string-like type, independently of their apparent constraint.
    pub(super) fn is_pattern_literal_type(&self, ty: TypeId) -> Result<bool, DemandError> {
        let _guard = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        if let TypeView::TemplateLiteral(template) = self.model.get(ty)? {
            for &ty in template.types.iter() {
                if !self.is_pattern_literal_placeholder_type(ty)? {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        if let TypeView::StringMapping(mapping) = self.model.get(ty)? {
            return self.is_pattern_literal_placeholder_type(mapping.target);
        }
        Ok(false)
    }

    /// `isGenericStringLikeType` is a syntax-free question over factory flags.
    pub(super) fn is_generic_string_like_type(&self, ty: TypeId) -> Result<bool, DemandError> {
        Ok(self
            .model
            .has_type_flags(ty, TypeFlags::TEMPLATE_LITERAL | TypeFlags::STRING_MAPPING)
            && !self.is_pattern_literal_type(ty)?)
    }

    /// `checkTemplateLiteralType`: each child source check is immediately
    /// followed by acquisition and its diagnostic relation, then root creation.
    pub(super) fn check_template_literal_type_completion(
        &mut self,
        root: ExpressionRoot,
    ) -> super::super::model::NativeResult<()> {
        let _guard = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let recipe = self.source.template_type_recipe(root)?;
            let consumer = super::super::location(self.inputs, root.file, root.site.span);
            let constraint = self.model.template_constraint_type();
            for (root, node) in recipe.children.into_iter().zip(recipe.types.iter()) {
                sequence.read(self.check_type_source_element_completion(root))?;
                let ty = sequence.read(self.check_type_completion(node))?;
                let error = super::relations::RelationReporting::Report.assignment_at(root);
                sequence.read(self.check_type_related_to_completion(
                    ty,
                    constraint,
                    RelationKind::Assignable,
                    error,
                    &node.location,
                ))?;
            }
            sequence.read(self.type_from_template_node_completion(root, &consumer))?;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `getStringMappingType` distinguishes a marker-backed native alias from
    /// arbitrary same-named user aliases and maps unions through the common owner.
    pub(super) fn string_mapping_type_completion(
        &mut self,
        symbol: impl Into<AliasSymbol>,
        kind: super::super::model::StringMappingKind,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let _guard = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        let symbol = symbol.into();
        let mut sequence = NativeSequence::new();
        let result = (|| {
            use super::super::model::StringMappingKind as K;
            if self
                .model
                .has_type_flags(ty, TypeFlags::UNION | TypeFlags::NEVER)
            {
                return sequence.read(self.map_type_completion(ty, consumer, |checker, ty| {
                    checker.string_mapping_type_completion(symbol, kind, ty, consumer)
                }));
            }
            match self.model.get(ty)? {
                TypeView::Literal {
                    base: PrimitiveName::String,
                    value,
                } => {
                    let mapped = kind.apply(&value, self.control.as_ref());
                    return Ok(self.model.regular_string_literal_value(mapped));
                }
                TypeView::TemplateLiteral(template) => {
                    let mut texts = Vec::new();
                    for text in template.texts.iter() {
                        texts.push(text.clone());
                    }
                    let mut types = template.types.to_vec();
                    match kind {
                        K::Uppercase | K::Lowercase => {
                            for text in &mut texts {
                                *text = kind.apply(text, self.control.as_ref());
                            }
                            for ty in &mut types {
                                *ty =
                                    sequence.read(self.string_mapping_type_completion(
                                        symbol, kind, *ty, consumer,
                                    ))?;
                            }
                        }
                        K::Capitalize | K::Uncapitalize => {
                            if !texts[0].is_empty() {
                                texts[0] = kind.apply(&texts[0], self.control.as_ref());
                            } else {
                                types[0] = sequence.read(self.string_mapping_type_completion(
                                    symbol, kind, types[0], consumer,
                                ))?;
                            }
                        }
                    }
                    return sequence.read(self.template_literal_type_completion(
                        std::sync::Arc::from(texts),
                        types,
                        consumer,
                    ));
                }
                TypeView::StringMapping(mapping) if mapping.symbol == symbol => return Ok(ty),
                _ => {}
            }
            let target = if self.model.has_type_flags(
                ty,
                TypeFlags::ANY | TypeFlags::STRING | TypeFlags::STRING_MAPPING,
            ) || sequence
                .read(self.is_generic_index_type_completion(ty, consumer))?
            {
                ty
            } else if self.is_pattern_literal_placeholder_type(ty)? {
                sequence.read(self.template_literal_type_completion(
                    std::sync::Arc::from(vec![
                        LiteralValue::from_text("", self.control.as_ref()),
                        LiteralValue::from_text("", self.control.as_ref()),
                    ]),
                    vec![ty],
                    consumer,
                ))?
            } else {
                return Ok(ty);
            };
            let native_symbol =
                self.type_symbol_of_reference(self.alias_reference_symbol(symbol), consumer)?;
            Ok(self
                .model
                .string_mapping_type(symbol, native_symbol, kind, target))
        })();
        sequence.finish(result)
    }
}
