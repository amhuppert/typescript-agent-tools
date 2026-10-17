//! Function expression checking owns ContextChecked and contextual assignment;
//! actual symbol, signature, inference and return caches remain shared owners.

use super::super::model::{
    DemandError, InferenceContextId, MembersView, Metadata, NativeCompletion, NativeResult,
    NativeSequence, ObjectFlags, ObjectKind, ParameterValue, Signature, SignatureData,
    SignatureFlags, TypeDemand, TypeFlags, TypeId,
};
use super::super::source::{ExpressionRoot, FunctionExpressionFacts, SignatureParameterRecipe};
use super::contextual::CheckMode;
use super::relations::RelationKind;
use super::{Evaluator, TypeState};
use crate::analysis::SemanticFailureReason;
use crate::references::Location;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct FunctionsState {
    checked: HashSet<ExpressionRoot>,
    failures: HashMap<ExpressionRoot, DemandError>,
    context_free: HashMap<ExpressionRoot, NativeCompletion<TypeId>>,
}

impl Evaluator<'_, '_> {
    /// `checkFunctionExpressionOrObjectLiteralMethod`: body checking is
    /// deferred; contextual re-entry can install ContextChecked while the
    /// outer invocation is obtaining its contextual signature.
    pub(super) fn check_function_expression_completion(
        &mut self,
        root: ExpressionRoot,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(error) = self.functions.failures.get(&root) {
            return Err(error.clone());
        }
        let result = self.check_function_expression_worker_completion(root, mode, consumer);
        if let Err(error) = &result {
            self.functions.failures.insert(root, error.clone());
        }
        result
    }

    fn check_function_expression_worker_completion(
        &mut self,
        root: ExpressionRoot,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let facts = self.source.function_expression_facts(root)?;
            if mode.contains(CheckMode::SKIP_CONTEXT_SENSITIVE)
                && self.source.expression_is_context_sensitive(root)?
            {
                if !facts.annotation && !self.source.has_context_sensitive_parameters(root)? {
                    if let Some(context) = self.contextual_signature(root, consumer)? {
                        let returned = sequence.read(
                            self.get_return_type_of_signature_completion(&context, consumer),
                        )?;
                        if sequence.read(
                            self.could_contain_type_variables_completion(returned, consumer),
                        )? {
                            if let Some(ty) = self.functions.context_free.get(&root) {
                                return sequence.read(Ok(ty.clone()));
                            }
                            let body = self.source.return_body(facts.signature)?;
                            let ty = self
                                .return_body_type_ex_completion(&body, mode)?
                                .and_then_value(|returned| {
                                    let signature = self.model.signature(SignatureData {
                                        type_parameter_types: Vec::new().into(),
                                        type_parameters: Vec::new(),
                                        this_parameter: None,
                                        parameters: Vec::new().into(),
                                        return_type: TypeDemand::Known(returned),
                                        predicate: None,
                                        origins: Vec::new().into(),
                                        minimum_arguments: 0,
                                        flags: SignatureFlags::non_inferrable(),
                                    })?;
                                    self.attach_signature_declaration(signature.id, None)?;
                                    self.attach_signature_return(&signature, consumer.clone())?;
                                    let symbol = match self
                                        .reference_raw_symbol_of_declaration(facts.declaration)?
                                    {
                                        Some(symbol) => {
                                            self.type_symbol_of_reference(symbol, consumer)?
                                        }
                                        None => super::super::model::TypeSymbol::Absent,
                                    };
                                    let ty = self.model.structured_with_flags(
                                        symbol,
                                        MembersView {
                                            properties: Vec::new().into(),
                                            call_signatures: vec![signature].into(),
                                            construct_signatures: Vec::new().into(),
                                            index_signatures: Vec::new().into(),
                                        },
                                        Metadata::default(),
                                        ObjectKind::Anonymous,
                                        ObjectFlags::NON_INFERRABLE_TYPE,
                                    );
                                    Ok(ty)
                                })?;
                            self.functions.context_free.insert(root, ty.clone());
                            return sequence.read(Ok(ty));
                        }
                    }
                }
                return Ok(self.model.any_function_type());
            }
            if !self.functions.checked.contains(&root) {
                let context = self.contextual_signature(root, consumer)?;
                if !self.functions.checked.contains(&root) {
                    self.functions.checked.insert(root);
                    let ty = sequence
                        .read(self.function_expression_symbol_type_completion(&facts, consumer))?;
                    if let Some(signature) = sequence
                        .read(self.call_signatures_completion(ty, consumer))?
                        .first()
                        .cloned()
                    {
                        if self.source.expression_is_context_sensitive(root)? {
                            if let Some(context) = &context {
                                let inference = self.inference_context_for(root)?;
                                let mut instantiated = None;
                                if mode.contains(CheckMode::INFERENTIAL) {
                                    let inference = inference.ok_or_else(|| {
                                        super::super::internal(
                                            "inferential function has no inference context",
                                        )
                                    })?;
                                    sequence.read(self.infer_annotated_function_completion(
                                        &signature, context, inference, consumer,
                                    ))?;
                                    if let Some(rest) = sequence.read(
                                        self.signature_effective_rest_completion(context, consumer),
                                    )? && self
                                        .model
                                        .has_type_flags(rest, TypeFlags::TYPE_PARAMETER)
                                    {
                                        let mapper =
                                            self.inference_context_mapper(inference, true)?;
                                        instantiated = Some(sequence.read(
                                            self.instantiate_signature_ex_completion(
                                                context, mapper, false, consumer,
                                            ),
                                        )?);
                                    }
                                }
                                let context = match instantiated {
                                    Some(context) => context,
                                    None => {
                                        if let Some(inference) = inference {
                                            let mapper =
                                                self.inference_context_mapper(inference, false)?;
                                            sequence.read(
                                                self.instantiate_signature_ex_completion(
                                                    context, mapper, false, consumer,
                                                ),
                                            )?
                                        } else {
                                            context.clone()
                                        }
                                    }
                                };
                                sequence.read(self.assign_contextual_parameters_completion(
                                    &signature, &context, consumer,
                                ))?;
                            } else {
                                sequence.read(self.assign_noncontextual_parameters_completion(
                                    &signature, consumer,
                                ))?;
                            }
                        } else if let Some(context) = &context
                            && !facts.type_parameters
                            && context.parameters.len() > facts.parameter_count
                        {
                            let inference = self.inference_context_for(root)?;
                            if mode.contains(CheckMode::INFERENTIAL) {
                                sequence.read(self.infer_annotated_function_completion(
                                    &signature,
                                    context,
                                    inference.ok_or_else(|| {
                                        super::super::internal(
                                            "inferential function has no inference context",
                                        )
                                    })?,
                                    consumer,
                                ))?;
                            }
                        }
                        if context.is_some()
                            && sequence
                                .read(self.signature_annotated_return_completion(&signature))?
                                .is_none()
                            && !self
                                .signatures
                                .has_return_property(signature.id, &self.demands)
                        {
                            let body = self.source.return_body(facts.signature)?;
                            let returned = self.return_body_type_ex_completion(&body, mode)?;
                            self.assign_contextual_signature_return(&signature, returned.clone())?;
                            sequence.read(Ok(returned))?;
                        }
                        sequence
                            .read(self.check_signature_declaration_completion(facts.signature))?;
                    }
                }
            }
            sequence.read(self.function_expression_symbol_type_completion(&facts, consumer))
        })();
        sequence.finish(result)
    }

    fn function_expression_symbol_type_completion(
        &mut self,
        facts: &FunctionExpressionFacts,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.reference_symbol_of_declaration_completion(facts.declaration)?
            .and_then(|symbol| self.value_reference_type_completion(symbol, consumer))
    }

    /// `inferFromAnnotatedParametersAndReturn` excludes the rest parameter
    /// and uses raw annotations, independent of contextual symbol caches.
    fn infer_annotated_function_completion(
        &mut self,
        signature: &Signature,
        context: &Signature,
        inference: InferenceContextId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let length = signature.parameters.len() - usize::from(signature.flags.has_rest());
            for (index, parameter) in signature.parameters[..length].iter().enumerate() {
                let declaration = parameter.declaration.ok_or_else(|| {
                    super::super::internal("declared function parameter lacks declaration")
                })?;
                let recipe = self.source.parameter_recipe(declaration)?;
                if let Some(annotation) = recipe.annotation {
                    let source = sequence.read(self.check_type_completion(&annotation))?;
                    let source = sequence.read(self.add_optionality_completion(
                        source,
                        false,
                        recipe.question_mark,
                        &recipe.location,
                    ))?;
                    let target = sequence.read(
                        self.signature_type_at_position_or_any_completion(context, index, consumer),
                    )?;
                    sequence.read(self.infer_contextual_annotation_completion(
                        inference, source, target, consumer,
                    ))?;
                }
            }
            if let Some(source) =
                sequence.read(self.signature_annotated_return_completion(signature))?
            {
                let target = sequence
                    .read(self.get_return_type_of_signature_completion(context, consumer))?;
                sequence.read(
                    self.infer_contextual_annotation_completion(
                        inference, source, target, consumer,
                    ),
                )?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `assignContextualParameterTypes`: contextual generics and this are
    /// mutable fields on the actual signature, while parameter values are
    /// installed on their own canonical symbol questions.
    fn assign_contextual_parameters_completion(
        &mut self,
        signature: &Signature,
        context: &Signature,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !context.type_parameter_types().is_empty() {
                if !signature.type_parameter_types().is_empty() {
                    return Ok(());
                }
                signature.install_contextual_type_parameters(
                    context.type_parameter_types().clone(),
                    context.type_parameters().clone(),
                )?;
            }
            if let Some(context_this) = context.this_parameter() {
                let assign = match signature.this_parameter() {
                    None => true,
                    Some(parameter) => match parameter.declaration {
                        Some(declaration) => self
                            .source
                            .parameter_recipe(declaration)?
                            .annotation
                            .is_none(),
                        None => false,
                    },
                };
                if assign {
                    if signature.this_parameter().is_none() {
                        let parameter = self.unresolved_parameter_clone(context_this, consumer)?;
                        signature.install_contextual_this(parameter)?;
                    }
                    let ty = sequence.read(self.demand_completion(context_this.value_type))?;
                    let parameter = signature.this_parameter().as_ref().ok_or_else(|| {
                        super::super::internal("contextual this installation was lost")
                    })?;
                    sequence.read(self.assign_parameter_type_completion(
                        parameter,
                        Some(ty),
                        consumer,
                    ))?;
                }
            }
            let length = signature.parameters.len() - usize::from(signature.flags.has_rest());
            for (index, parameter) in signature.parameters[..length].iter().enumerate() {
                let declaration = parameter.declaration.ok_or_else(|| {
                    super::super::internal("declared function parameter lacks declaration")
                })?;
                let recipe = self.source.parameter_recipe(declaration)?;
                if recipe.annotation.is_none() {
                    let mut ty = sequence.read(
                        self.signature_type_at_position_completion(context, index, consumer),
                    )?;
                    if let Some(contextual) = ty
                        && let Some(initializer) = &recipe.initializer
                    {
                        let initial = sequence
                            .read(self.check_declaration_initializer_completion(initializer))?;
                        if !sequence.read(self.type_related_completion(
                            initial,
                            contextual,
                            RelationKind::Assignable,
                            consumer,
                        ))? {
                            let initial = sequence
                                .read(self.widened_literal_type_completion(initial, consumer))?;
                            if sequence.read(self.type_related_completion(
                                contextual,
                                initial,
                                RelationKind::Assignable,
                                consumer,
                            ))? {
                                ty = Some(initial);
                            }
                        }
                    }
                    sequence
                        .read(self.assign_parameter_type_completion(parameter, ty, consumer))?;
                }
            }
            if signature.flags.has_rest() {
                let parameter = &signature.parameters[length];
                let declaration = parameter.declaration.ok_or_else(|| {
                    super::super::unsupported_demand_at_location(
                        self.inputs,
                        consumer,
                        SemanticFailureReason::UnsupportedSemanticForm,
                        "native arguments-generated parameter assignment is pending",
                    )
                })?;
                if self
                    .source
                    .parameter_recipe(declaration)?
                    .annotation
                    .is_none()
                {
                    let ty = sequence.read(self.signature_rest_type_at_position_completion(
                        context, length, false, consumer,
                    ))?;
                    sequence.read(self.assign_parameter_type_completion(
                        parameter,
                        Some(ty),
                        consumer,
                    ))?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    fn assign_noncontextual_parameters_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(parameter) = signature.this_parameter() {
                sequence.read(self.assign_parameter_type_completion(parameter, None, consumer))?;
            }
            for parameter in signature.parameters.iter() {
                sequence.read(self.assign_parameter_type_completion(parameter, None, consumer))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `assignParameterType` observes the actual installed raw cache before
    /// fallback inference. Optionality is declaration-owned, not call arity.
    fn assign_parameter_type_completion(
        &mut self,
        parameter: &ParameterValue,
        context: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<()> {
        match self.property_value_state(parameter.symbol, false)? {
            Some(TypeState::Installed(_)) => return Ok(NativeCompletion::clean(())),
            Some(TypeState::Failed(error)) => return Err(error),
            _ => {}
        }
        let facts = parameter
            .declaration
            .map(|declaration| self.source.variable_facts(declaration))
            .transpose()?;
        let value = match context {
            Some(ty) => NativeCompletion::clean(ty),
            None => {
                if let Some(declaration) = parameter.declaration {
                    self.widened_variable_like_type_completion(declaration)?
                } else {
                    self.property_raw_type_completion(parameter.symbol, consumer)?
                }
            }
        };
        value.and_then(|ty| {
            let optional = facts
                .as_ref()
                .is_some_and(|facts| facts.initializer.is_none() && facts.optional);
            self.add_optionality_completion(ty, false, optional, consumer)?
                .and_then(|ty| {
                    // Direct assignment copies the actual native value; acquisition
                    // evidence remains on this invocation, not the assigned symbol.
                    self.set_property_value_state(
                        parameter.symbol,
                        false,
                        TypeState::Installed(NativeCompletion::clean(ty)),
                    )?;
                    let Some(pattern) = facts.as_ref().and_then(|facts| facts.pattern) else {
                        return Ok(NativeCompletion::clean(()));
                    };
                    let value = if ty
                        == self
                            .model
                            .special(super::super::model::SpecialName::Unknown)
                    {
                        let value =
                            self.type_from_binding_pattern_completion(pattern, false, false)?;
                        self.set_property_value_state(
                            parameter.symbol,
                            false,
                            TypeState::Installed(NativeCompletion::clean(*value.value())),
                        )?;
                        value
                    } else {
                        NativeCompletion::clean(ty)
                    };
                    value.and_then(|ty| self.assign_binding_element_types_completion(pattern, ty))
                })
        })
    }

    /// `getContextuallyTypedParameterType` uses the actual IIFE call cache or
    /// actual contextual signature. Neither path infers from printed names.
    pub(super) fn contextually_typed_parameter_completion(
        &mut self,
        parameter: &SignatureParameterRecipe,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let context = self.source.parameter_context(parameter.declaration)?;
            if context.this_parameter {
                return sequence.read(self.contextual_this_parameter_type_completion(
                    context.function,
                    &parameter.location,
                ));
            }
            if !matches!(
                parameter.function_kind,
                crate::syntax_kind::SyntaxKind::FunctionExpression
                    | crate::syntax_kind::SyntaxKind::ArrowFunction
            ) && !parameter.object_literal_method
            {
                return Ok(None);
            }
            if !self
                .source
                .expression_is_context_sensitive(context.function)?
            {
                return Ok(None);
            }
            if let Some(iife) = context.iife {
                return sequence.read(self.iife_parameter_type_completion(
                    iife,
                    context.index,
                    parameter.rest,
                    parameter.initializer.is_some(),
                ));
            }
            if let Some(signature) = sequence
                .read(self.contextual_signature_completion(context.function, &parameter.location))?
            {
                let index = context
                    .index
                    .checked_sub(usize::from(context.has_this))
                    .ok_or_else(|| {
                        super::super::internal("this parameter used ordinary contextual position")
                    })?;
                if parameter.rest && context.last {
                    return sequence
                        .read(self.signature_rest_type_at_position_completion(
                            &signature,
                            index,
                            false,
                            &parameter.location,
                        ))
                        .map(Some);
                }
                return sequence.read(self.signature_type_at_position_completion(
                    &signature,
                    index,
                    &parameter.location,
                ));
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `getContextualThisParameterType`: arrows never acquire a contextual
    /// this; explicit signature this precedes literal and assignment context.
    pub(super) fn contextual_this_parameter_type_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if root.site.kind == crate::syntax_kind::SyntaxKind::ArrowFunction {
                return Ok(None);
            }
            let facts = self.source.function_expression_facts(root)?;
            let (contextual, _, _) = self.source.contextual_function_facts(facts.signature)?;
            if contextual
                && self.source.expression_is_context_sensitive(root)?
                && let Some(signature) =
                    sequence.read(self.contextual_signature_completion(root, consumer))?
                && let Some(parameter) = signature.this_parameter()
            {
                return sequence
                    .read(self.demand_completion(parameter.value_type))
                    .map(Some);
            }
            if self.inputs.semantic_options().no_implicit_this {
                match self.source.contextual_this_source(root)? {
                    super::super::source::ContextualThisSource::None => {}
                    super::super::source::ContextualThisSource::Assignment(receiver) => {
                        let ty = sequence.read(
                            self.check_expression_cached_completion(receiver.file, receiver.site),
                        )?;
                        return sequence
                            .read(self.widened_type_completion(ty, consumer))
                            .map(Some);
                    }
                    super::super::source::ContextualThisSource::Object(literal) => {
                        let context = sequence.read(self.apparent_contextual_type_completion(
                            literal,
                            super::contextual::ContextFlags::NONE,
                            consumer,
                        ))?;
                        if let Some(ty) = sequence.read(
                            self.contextual_object_this_completion(literal, context, consumer),
                        )? {
                            let ty = if let Some(inference) = self.inference_context_for(literal)? {
                                let mapper = self.inference_context_mapper(inference, false)?;
                                sequence
                                    .read(self.instantiate_type_completion(ty, mapper, consumer))?
                            } else {
                                ty
                            };
                            return Ok(Some(ty));
                        }
                        let ty = match context {
                            Some(context) => sequence
                                .read(self.non_nullable_type_completion(context, consumer))?,
                            None => sequence.read(
                                self.check_expression_cached_completion(literal.file, literal.site),
                            )?,
                        };
                        return sequence
                            .read(self.widened_type_completion(ty, consumer))
                            .map(Some);
                    }
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }
}
