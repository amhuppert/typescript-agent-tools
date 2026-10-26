//! Failed assignments have one elaboration owner. Diagnostic text is private,
//! but all native type, symbol, context and display acquisitions remain ordered.

use crate::project::semantic::model::{NativeCompletion, NativeResult, NativeSequence};

use super::{ErrorContext, Evaluator, Message, RelationCheck, RelationKind};
use crate::project::semantic::evaluation::{ReferenceDeclaration, contextual::CheckMode};
use crate::project::semantic::model::{AccessFlags, DemandError, TypeFlags, TypeId, TypeView};
use crate::project::semantic::source::{ExpressionNode, ExpressionRoot, elaboration::Elaboration};
use crate::references::Location;
use crate::types::PrimitiveName;

impl Evaluator<'_, '_> {
    /// `checkTypeRelatedToAndOptionallyElaborate`: a failed silent relation
    /// enters elaboration only with an actual reporting node. The expression
    /// being elaborated can differ from that node (assignment and satisfies).
    pub(in crate::project::semantic::evaluation) fn check_type_related_to_and_optionally_elaborate(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        error: Option<ErrorContext>,
        expression: Option<ExpressionRoot>,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.check_type_related_to_and_optionally_elaborate_completion(
            source, target, relation, error, expression, consumer,
        )?
        .admit()
    }

    pub(in crate::project::semantic::evaluation) fn check_type_related_to_and_optionally_elaborate_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        error: Option<ErrorContext>,
        expression: Option<ExpressionRoot>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        Ok(self
            .check_type_related_to_and_optionally_elaborate_outcome_completion(
                source,
                target,
                relation,
                error,
                expression.map(Into::into),
                consumer,
            )?
            .map(|check| check.related))
    }

    pub(in crate::project::semantic::evaluation) fn check_type_related_to_and_optionally_elaborate_outcome_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        error: Option<ErrorContext>,
        expression: Option<ExpressionNode>,
        consumer: &Location,
    ) -> NativeResult<RelationCheck> {
        Ok(self
            .raw_check_type_related_to_and_optionally_elaborate_outcome_completion(
                source, target, relation, error, expression, consumer,
            )
            .map_err(|failure| {
                crate::project::semantic::observe_recovery(self.inputs, failure, consumer)
            })?
            .observe(|failure| {
                crate::project::semantic::observe_recovery(self.inputs, failure, consumer)
            }))
    }

    pub(in crate::project::semantic::evaluation) fn raw_check_type_related_to_and_optionally_elaborate_outcome_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        error: Option<ErrorContext>,
        expression: Option<ExpressionNode>,
        consumer: &Location,
    ) -> NativeResult<RelationCheck> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<RelationCheck, DemandError> {
            if sequence
                .read(self.raw_type_related_completion(source, target, relation, consumer))?
            {
                return Ok(RelationCheck {
                    related: true,
                    diagnostics: 0,
                });
            }
            if let Some(error) = error {
                let diagnostics = sequence.read(self.elaborate_error_completion(
                    expression, source, target, relation, error.head, consumer,
                ))?;
                if diagnostics != 0 {
                    return Ok(RelationCheck {
                        related: false,
                        diagnostics,
                    });
                }
                return sequence.read(self.raw_check_type_related_to_outcome_completion(
                    source,
                    target,
                    relation,
                    Some(error),
                    consumer,
                ));
            }
            Ok(RelationCheck {
                related: false,
                diagnostics: 0,
            })
        })();
        sequence.finish(result)
    }

    /// `isExactOptionalPropertyMismatch` is shared by assignment heads and
    /// element/reporting elaboration; nil is distinct from an unknown type.
    pub(in crate::project::semantic::evaluation) fn exact_optional_property_mismatch(
        &self,
        source: Option<TypeId>,
        target: Option<TypeId>,
    ) -> bool {
        source.is_some_and(|ty| self.model.maybe_type_of_kind(ty, TypeFlags::UNDEFINED))
            && target.is_some_and(|ty| self.model.contains_missing_type(ty))
    }

    /// `isOrHasGenericConditional` deliberately traverses intersections only.
    fn elaboration_target_has_conditional_completion(&self, ty: TypeId) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if self.model.has_type_flags(ty, TypeFlags::CONDITIONAL) {
                return Ok(true);
            }
            if let TypeView::Intersection(types) = self.model.get(ty)? {
                for &ty in types.iter() {
                    if sequence.read(self.elaboration_target_has_conditional_completion(ty))? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `elaborateError`: suggestions precede syntax dispatch at every recursive
    /// node. A report can roll back text, never a consumed semantic failure.
    fn elaborate_error_completion(
        &mut self,
        node: Option<ExpressionNode>,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        head: Option<Message>,
        consumer: &Location,
    ) -> NativeResult<usize> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<usize, DemandError> {
            let Some(node) = node else {
                return Ok(0);
            };
            if sequence.read(self.elaboration_target_has_conditional_completion(target))?
                || self.inputs.semantic_options().no_check
            {
                return Ok(0);
            }
            for construct in [true, false] {
                let diagnostics = sequence.read(self.elaborate_call_suggestion_completion(
                    node, source, target, relation, construct, head, consumer,
                ))?;
                if diagnostics != 0 {
                    return Ok(diagnostics);
                }
            }
            // SyntheticExpression still participates in the ordered call/construct
            // suggestion prelude. Only native's later syntax switch has no arm.
            let ExpressionNode::Captured(node) = node else {
                return Ok(0);
            };
            match self.source.elaboration_recipe(node)? {
                Elaboration::None => Ok(0),
                Elaboration::Inner(inner) => sequence.read(self.elaborate_error_completion(
                    Some(inner.into()),
                    source,
                    target,
                    relation,
                    head,
                    consumer,
                )),
                Elaboration::Object(properties) => {
                    if self
                        .model
                        .has_type_flags(target, TypeFlags::PRIMITIVE | TypeFlags::NEVER)
                    {
                        return Ok(0);
                    }
                    let mut reported = 0usize;
                    for property in properties {
                        let symbol = self.reference_symbol_of_declaration(property.declaration)?;
                        let symbol = self.register_reference_symbol(symbol, consumer)?;
                        let name = self.literal_type_from_property(
                            symbol,
                            TypeFlags::STRING_LITERAL
                                | TypeFlags::NUMBER_LITERAL
                                | TypeFlags::UNIQUE_SYMBOL,
                            false,
                            consumer,
                        )?;
                        if self.model.has_type_flags(name, TypeFlags::NEVER) {
                            continue;
                        }
                        let head = (property.initializer.is_some()
                            && property.computed_non_literal)
                            .then_some(Message::ComputedProperty);
                        let next = sequence.read(self.elaborate_element_completion(
                            source,
                            target,
                            relation,
                            property.name,
                            property.initializer,
                            name,
                            head,
                            consumer,
                        ))?;
                        reported = reported.checked_add(next).ok_or_else(|| {
                            crate::project::semantic::internal(
                                "elaboration diagnostic count exceeds addressable input",
                            )
                        })?;
                    }
                    Ok(reported)
                }
                Elaboration::Array(elements) => {
                    if self
                        .model
                        .has_type_flags(target, TypeFlags::PRIMITIVE | TypeFlags::NEVER)
                    {
                        return Ok(0);
                    }
                    let mut source = source;
                    if !self.is_tuple_like_type(source, consumer)? {
                        source = self.with_contextual_type_override(node, target, |checker| {
                            checker.check_array_literal(
                                node,
                                CheckMode::CONTEXTUAL | CheckMode::FORCE_TUPLE,
                                consumer,
                            )
                        })?;
                        if !self.is_tuple_like_type(source, consumer)? {
                            return Ok(0);
                        }
                    }
                    let mut reported = 0usize;
                    for (index, element) in elements.into_iter().enumerate() {
                        let Some(element) = element else {
                            continue;
                        };
                        if self.is_tuple_like_type(target, consumer)?
                            && self
                                .property_of_type(target, &index.to_string(), consumer)?
                                .is_none()
                        {
                            continue;
                        }
                        let name = self.model.literal(PrimitiveName::Number, index.to_string());
                        let name = self.model.regular_literal(name);
                        let check = self.source.effective_check_node(element)?;
                        let next = sequence.read(self.elaborate_element_completion(
                            source,
                            target,
                            relation,
                            check,
                            Some(check),
                            name,
                            None,
                            consumer,
                        ))?;
                        reported = reported.checked_add(next).ok_or_else(|| {
                            crate::project::semantic::internal(
                                "elaboration diagnostic count exceeds addressable input",
                            )
                        })?;
                    }
                    Ok(reported)
                }
                Elaboration::Arrow { body, asynchronous } => {
                    sequence.read(self.elaborate_arrow_completion(
                        source,
                        target,
                        relation,
                        body,
                        asynchronous,
                        consumer,
                    ))
                }
            }
        })();
        sequence.finish(result)
    }

    /// `elaborateDidYouMeanToCallOrConstruct`: each signature return is read
    /// only until the first qualifying suggestion, constructs before calls.
    #[expect(
        clippy::too_many_arguments,
        reason = "ports elaborateDidYouMeanToCallOrConstruct's parameters plus the request consumer"
    )]
    fn elaborate_call_suggestion_completion(
        &mut self,
        node: ExpressionNode,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        construct: bool,
        head: Option<Message>,
        consumer: &Location,
    ) -> NativeResult<usize> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<usize, DemandError> {
            for signature in self.signatures_of_type(source, construct, consumer)?.iter() {
                let returned =
                    sequence.read(self.raw_return_type_of_signature_completion(signature))?;
                if !self
                    .model
                    .has_type_flags(returned, TypeFlags::ANY | TypeFlags::NEVER)
                    && sequence.read(self.raw_check_type_related_to_completion(
                        returned, target, relation, None, consumer,
                    ))?
                {
                    let check =
                        sequence.read(self.raw_check_type_related_to_outcome_completion(
                            source,
                            target,
                            relation,
                            Some(ErrorContext { node, head }),
                            consumer,
                        ))?;
                    if check.related {
                        return Ok(0);
                    }
                    if check.diagnostics == 0 {
                        return Err(crate::project::semantic::internal(
                            "native call suggestion requires its first diagnostic",
                        )
                        .into());
                    }
                    return Ok(1);
                }
            }
            Ok(0)
        })();
        sequence.finish(result)
    }

    /// `getBestMatchIndexedAccessTypeOrUndefined` first asks the complete
    /// target. The fallback comparer is independent assignability even when
    /// the outer elaboration is using another relation.
    fn best_match_indexed_type_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        name: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let ty = self.indexed_access_without_node_optional(
                target,
                name,
                AccessFlags::NONE,
                consumer,
            )?;
            if ty.is_some() {
                return Ok(ty);
            }
            if self.model.has_type_flags(target, TypeFlags::UNION)
                && let Some(best) = self.best_matching_type(
                    source,
                    target,
                    |checker, source, target| {
                        checker.type_related(source, target, RelationKind::Assignable, consumer)
                    },
                    consumer,
                )?
            {
                return self.indexed_access_without_node_optional(
                    best,
                    name,
                    AccessFlags::NONE,
                    consumer,
                );
            }
            Ok(None)
        })();
        result.map(NativeCompletion::clean)
    }

    /// `elaborateElement` keeps the rechecked expression type separate from
    /// its property's type; optionality removes Missing only from the latter.
    #[expect(
        clippy::too_many_arguments,
        reason = "ports elaborateElementwise's per-element inputs plus the request consumer"
    )]
    fn elaborate_element_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        prop: ExpressionRoot,
        next: Option<ExpressionRoot>,
        name: TypeId,
        head: Option<Message>,
        consumer: &Location,
    ) -> NativeResult<usize> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<usize, DemandError> {
            let Some(mut target_prop) = sequence
                .read(self.best_match_indexed_type_completion(source, target, name, consumer))?
            else {
                return Ok(0);
            };
            if self
                .model
                .has_type_flags(target_prop, TypeFlags::INDEXED_ACCESS)
            {
                return Ok(0);
            }
            let Some(mut source_prop) = self.indexed_access_without_node_optional(
                source,
                name,
                AccessFlags::NONE,
                consumer,
            )?
            else {
                return Ok(0);
            };
            if sequence.read(self.raw_check_type_related_to_completion(
                source_prop,
                target_prop,
                relation,
                None,
                consumer,
            ))? {
                return Ok(0);
            }
            if next.is_some() {
                let diagnostics = sequence.read(self.elaborate_error_completion(
                    next.map(Into::into),
                    source_prop,
                    target_prop,
                    relation,
                    None,
                    consumer,
                ))?;
                if diagnostics != 0 {
                    return Ok(diagnostics);
                }
            }
            let specific_source = if let Some(next) = next {
                self.with_contextual_type_override(next, source_prop, |checker| {
                    let expression = checker.source.expression_at(next.file, next.site)?;
                    checker
                        .check_expression_for_mutable_location(&expression, CheckMode::CONTEXTUAL)
                })?
            } else {
                source_prop
            };
            let reported = if self.inputs.semantic_options().exact_optional_property_types
                && self.exact_optional_property_mismatch(Some(specific_source), Some(target_prop))
            {
                sequence.read(
                    self.raw_check_diagnostic_type_display_completion(specific_source, consumer),
                )?;
                sequence.read(
                    self.raw_check_diagnostic_type_display_completion(target_prop, consumer),
                )?;
                1
            } else {
                // `getPropertyNameFromIndex(name, nil)` returns this exact native
                // internal name when the type is not usable as a property key.
                let key = self.property_key_from_type(name)?.unwrap_or_else(|| {
                    crate::project::binding::NativePropertyKey::fixed(b"\xfemissing".as_slice())
                });
                let target_optional = sequence
                    .read(self.elaboration_property_optional_completion(target, &key, consumer))?;
                let source_optional = sequence
                    .read(self.elaboration_property_optional_completion(source, &key, consumer))?;
                target_prop = self
                    .model
                    .remove_missing_type(target_prop, target_optional)
                    .map_err(|failure| failure.at(self.inputs, consumer))?;
                source_prop = self
                    .model
                    .remove_missing_type(source_prop, target_optional && source_optional)
                    .map_err(|failure| failure.at(self.inputs, consumer))?;
                let check = sequence.read(self.raw_check_type_related_to_outcome_completion(
                    specific_source,
                    target_prop,
                    relation,
                    Some(ErrorContext {
                        node: prop.into(),
                        head,
                    }),
                    consumer,
                ))?;
                if check.related && specific_source != source_prop {
                    let retry =
                        sequence.read(self.raw_check_type_related_to_outcome_completion(
                            source_prop,
                            target_prop,
                            relation,
                            Some(ErrorContext {
                                node: prop.into(),
                                head,
                            }),
                            consumer,
                        ))?;
                    check
                        .diagnostics
                        .checked_add(retry.diagnostics)
                        .ok_or_else(|| {
                            crate::project::semantic::internal(
                                "element diagnostic count exceeds addressable input",
                            )
                        })?
                } else {
                    check.diagnostics
                }
            };
            if reported == 0 {
                return Ok(0);
            }
            let name_bytes = self.property_key_from_type(name)?;
            let target_property = match &name_bytes {
                Some(name) => self.property_of_type_by_key(target, name, consumer)?,
                None => None,
            };
            let mut issued = false;
            if target_property.is_none()
                && let Some(index) = self.applicable_index_info(target, name, consumer)?
                && let Some(declaration) = index.declaration
            {
                issued = !self
                    .inputs
                    .bindings
                    .is_source_file_default_library(declaration.file);
            }
            if !issued {
                let declaration = if let Some(property) = target_property {
                    let symbol = self.property_symbol(&property, consumer)?;
                    self.property_reference_declarations(symbol, consumer)?
                        .first()
                        .copied()
                } else {
                    None
                };
                let declaration = match declaration {
                    Some(declaration) => Some(declaration),
                    None => match self.reference_symbol_of_type(target, consumer)? {
                        Some(symbol) => self
                            .reference_symbol_declarations(symbol, consumer)?
                            .first()
                            .copied(),
                        None => None,
                    },
                };
                if let Some(declaration) = declaration {
                    if name_bytes
                        .as_ref()
                        .is_none_or(|name| name.fixed_bytes() == Some(b"".as_slice()))
                        || self.model.has_type_flags(name, TypeFlags::UNIQUE_SYMBOL)
                    {
                        sequence.read(
                            self.raw_check_diagnostic_type_display_completion(name, consumer),
                        )?;
                    }
                    let file = match declaration {
                        ReferenceDeclaration::Captured(declaration) => declaration.file,
                        ReferenceDeclaration::Json { file, .. } => file,
                    };
                    if !self.inputs.bindings.is_source_file_default_library(file) {
                        sequence.read(
                            self.raw_check_diagnostic_type_display_completion(target, consumer),
                        )?;
                    }
                }
            }
            // Native forwards only diags[0], not the entire local diagnostic list.
            Ok(1)
        })();
        sequence.finish(result)
    }

    fn elaboration_property_optional_completion(
        &mut self,
        ty: TypeId,
        key: &crate::project::binding::NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let result = (|| -> Result<bool, DemandError> {
            let Some(property) = self.property_of_type_by_key(ty, key, consumer)? else {
                return Ok(false);
            };
            let symbol = self.property_symbol(&property, consumer)?;
            Ok(self.property_symbol_flags(symbol)?.optional)
        })();
        result.map(NativeCompletion::clean)
    }

    /// `elaborateArrowFunction`: only one source call signature is eligible;
    /// target returns are acquired completely before union construction.
    fn elaborate_arrow_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        body: Option<ExpressionRoot>,
        asynchronous: bool,
        consumer: &Location,
    ) -> NativeResult<usize> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<usize, DemandError> {
            let Some(source_signature) = self.single_signature(source, false, false, consumer)?
            else {
                return Ok(0);
            };
            let target_signatures = self.signatures_of_type(target, false, consumer)?;
            if target_signatures.is_empty() {
                return Ok(0);
            }
            let source_return =
                sequence.read(self.raw_return_type_of_signature_completion(&source_signature))?;
            let mut returns = Vec::with_capacity(target_signatures.len());
            for signature in target_signatures.iter() {
                returns
                    .push(sequence.read(self.raw_return_type_of_signature_completion(signature))?);
            }
            let target_return =
                sequence.read(self.union_type_ex_completion(returns, None, None, consumer))?;
            if sequence.read(self.raw_check_type_related_to_completion(
                source_return,
                target_return,
                relation,
                None,
                consumer,
            ))? {
                return Ok(0);
            }
            if body.is_some() {
                let diagnostics = sequence.read(self.elaborate_error_completion(
                    body.map(Into::into),
                    source_return,
                    target_return,
                    relation,
                    None,
                    consumer,
                ))?;
                if diagnostics != 0 {
                    return Ok(diagnostics);
                }
            }
            let error = body.map(|node| ErrorContext {
                node: node.into(),
                head: None,
            });
            let check = sequence.read(self.raw_check_type_related_to_outcome_completion(
                source_return,
                target_return,
                relation,
                error,
                consumer,
            ))?;
            if check.diagnostics == 0 {
                return Ok(0);
            }
            // Related information needs actual declaration metadata, even though
            // the user-facing graph has no diagnostic-text field.
            if let Some(symbol) = self.reference_symbol_of_type(target, consumer)? {
                self.reference_symbol_declarations(symbol, consumer)?;
            }
            if !asynchronous
                && sequence
                    .read(self.type_of_property_of_type_completion(
                        source_return,
                        "then",
                        consumer,
                    ))?
                    .is_none()
            {
                let promise =
                    sequence.read(self.create_promise_type_completion(source_return, consumer))?;
                sequence.read(self.raw_check_type_related_to_completion(
                    promise,
                    target_return,
                    relation,
                    None,
                    consumer,
                ))?;
            }
            Ok(1)
        })();
        sequence.finish(result)
    }
}
