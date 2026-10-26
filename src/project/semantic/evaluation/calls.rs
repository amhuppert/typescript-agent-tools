//! Native quick expression types and checked calls ask different questions.
//! Both consume the request's canonical type/signature/source operations.

use super::super::model::{
    AccessFlags, CallChain, InferenceContextId, MapperId, TupleElementFlags, TupleElementInfo,
    TupleElementLabel,
};
use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, Signature, SignatureData,
    SignatureFlags, SpecialName, TypeFlags, TypeId,
};
use super::super::source::{
    CallRecipe, CallRoot, CallTypeArgument, Expression, ExpressionForm, SignatureParent,
};
use super::Evaluator;
use super::contextual::{CheckMode, ContextNode};
use super::iteration::IterationUse;
use super::relations::{RelationKind, RelationReporting};
use crate::analysis::SemanticFailureReason;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use std::collections::HashMap;

mod construct;
mod instanceof;
mod invocation;
mod recovery;
mod tagged;

#[derive(Clone)]
enum CallState {
    Resolving,
    Resolved(NativeCompletion<Signature>),
    Failed(DemandError),
}

#[derive(Clone)]
pub(super) enum ResolvedSignature {
    Signature(Signature),
    Deferred,
}

/// Callee preparation transfers all its acquired evidence into resolveCall.
/// That owner can publish a signature before reporting finishes, so its
/// installation must already contain this prefix.
struct PreparedCall {
    signatures: Vec<Signature>,
    chain: Option<CallChain>,
    mode: CheckMode,
    construct_via_call: bool,
}

enum SignaturePreparation {
    Resolved(ResolvedSignature),
    Call(PreparedCall),
}

#[derive(Default)]
struct OverloadErrors {
    arguments: Vec<Signature>,
    argument_arity: Option<Signature>,
    type_arguments: Option<Signature>,
}

struct OverloadSelection {
    candidates: Vec<Signature>,
    argument_mode: CheckMode,
    single_nongeneric: bool,
    errors: OverloadErrors,
}

/// One native effective argument inventory is shared by overload selection,
/// inference, and applicability. Synthetic tuple elements retain a fresh
/// contextual node, their spread bit, and their actual tuple-label node.
pub(super) struct CallInput<'a> {
    pub(super) recipe: &'a CallRecipe,
    pub(super) arguments: Vec<CallArgument>,
}

pub(super) enum CallArgument {
    Captured(Expression),
    Synthetic {
        ty: TypeId,
        node: ContextNode,
        location: Location,
        spread: bool,
        label: Option<TupleElementLabel>,
    },
}

impl CallArgument {
    pub(super) fn is_spread(&self) -> bool {
        match self {
            Self::Captured(expression) => expression
                .root
                .is_some_and(|root| root.site.kind == SyntaxKind::SpreadElement),
            Self::Synthetic { spread, .. } => *spread,
        }
    }

    pub(super) fn is_omitted(&self) -> bool {
        matches!(self, Self::Captured(expression) if expression.root.is_some_and(|root| root.site.kind == SyntaxKind::OmittedExpression))
    }

    fn location(&self) -> &Location {
        match self {
            Self::Captured(expression) => &expression.location,
            Self::Synthetic { location, .. } => location,
        }
    }
}

/// `getResolvedSignature` owns the actual call-like node's installed property.
/// Candidate preparation and contextual save/restore keep this same table.
pub(super) struct CallsState {
    resolved: HashMap<CallRoot, CallState>,
    any_signature: Option<Signature>,
    silent_never_signature: Option<Signature>,
}

impl CallsState {
    pub(super) fn new() -> Self {
        Self {
            resolved: HashMap::new(),
            any_signature: None,
            silent_never_signature: None,
        }
    }
}

impl Evaluator<'_, '_> {
    /// `checkSpreadExpression` checks the operand with the incoming mode,
    /// then asks the shared checked Spread iteration operation.
    pub(super) fn check_spread_expression_completion(
        &mut self,
        root: super::super::source::ExpressionRoot,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        let operand = self.source.spread_operand(root)?;
        self.check_expression_completion(&operand, mode)?
            .and_then(|ty| {
                let node = operand
                    .root
                    .ok_or_else(|| super::super::internal("spread operand has no actual node"))?;
                self.checked_iteration_type_completion(
                    ty,
                    IterationUse::Spread,
                    Some(node.into()),
                    &operand.location,
                )
            })
    }

    /// `getContextualTypeForArgument` acquires the effective list even for a
    /// callee. Tuple-spread synthetic nodes are fresh and therefore cannot
    /// equal a captured argument. Their dynamic context is owned by the frame.
    pub(super) fn contextual_argument_type_completion(
        &mut self,
        root: CallRoot,
        argument: Option<crate::project::binding::flow::Site>,
    ) -> NativeResult<Option<TypeId>> {
        let recipe = self.source.call_recipe(root)?;
        self.effective_call_arguments_completion(&recipe)?
            .and_then(|input| {
                let Some(index) = input.arguments.iter().position(|item| {
                    matches!(item,
                CallArgument::Captured(expression) if argument.is_some_and(|argument|
                    expression.root.is_some_and(|root| root.site == argument)))
                }) else {
                    return Ok(NativeCompletion::clean(None));
                };
                Ok(self
                    .contextual_argument_type_at_index_completion(&recipe, index)?
                    .map(Some))
            })
    }

    /// `getContextualTypeForArgumentAtIndex`: resolvingSignature is observed
    /// without recursive resolution. Its empty parameter list makes the
    /// native getTypeAtPosition fallback Any; no synthetic signature is needed.
    fn contextual_argument_type_at_index_completion(
        &mut self,
        recipe: &CallRecipe,
        index: usize,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if recipe.import_call {
                return match index {
                    0 => Ok(self.model.primitive(crate::types::PrimitiveName::String)),
                    1 => self.global_object_type("ImportCallOptions", &recipe.location),
                    _ => Ok(self.model.special(SpecialName::Any)),
                };
            }
            if matches!(
                self.calls.resolved.get(&recipe.root),
                Some(CallState::Resolving)
            ) {
                return Ok(self.model.special(SpecialName::Any));
            }
            let signature = match sequence
                .read(self.resolved_signature_completion(recipe, CheckMode::NORMAL))?
            {
                ResolvedSignature::Signature(signature) => signature,
                ResolvedSignature::Deferred => return Ok(self.model.special(SpecialName::Any)),
            };
            if signature.flags.has_rest()
                && let Some(rest_index) = signature.parameters.len().checked_sub(1)
                && index >= rest_index
            {
                let rest = sequence
                    .read(self.demand_completion(signature.parameters[rest_index].value_type))?;
                let key = self.model.regular_literal_type(
                    crate::types::PrimitiveName::Number,
                    (index - rest_index).to_string(),
                );
                return self.indexed_access_type_ex(
                    rest,
                    key,
                    AccessFlags::CONTEXTUAL,
                    None,
                    &recipe.location,
                );
            }
            sequence.read(self.signature_type_at_position_or_any_completion(
                &signature,
                index,
                &recipe.location,
            ))
        })();
        sequence.finish(result)
    }

    /// The fixed fresh-request prelude executes `getResolvedSignature` for
    /// actual containing call/new/tag nodes, outermost first. This operation
    /// establishes signature context without adding checkCallExpression's
    /// chosen-return demand. A blocked ancestor stops before the site's root.
    pub(super) fn check_site_prelude(&mut self, calls: &[CallRoot]) -> Result<(), DemandError> {
        for &root in calls {
            let recipe = self.source.call_recipe(root)?;
            self.resolved_signature(&recipe, CheckMode::NORMAL)?;
        }
        Ok(())
    }

    /// `getQuickTypeOfExpression`: assertions read their annotation without
    /// checking the operand/source element; sole nongeneric calls read their
    /// return without checking arguments or installing a resolved-call cache.
    /// Await/assertion/call tests use the parenthesis-stripped expression,
    /// while the literal shortcut deliberately tests the original node.
    pub(super) fn quick_type_of_expression_completion(
        &mut self,
        expression: &Expression,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let root = match &expression.form {
                ExpressionForm::Await(operand) => {
                    return self
                        .quick_type_of_expression_completion(operand)?
                        .and_then(|quick| match quick {
                            Some(ty) => self.awaited_type_completion(ty, &expression.location),
                            None => Ok(NativeCompletion::clean(None)),
                        });
                }
                ExpressionForm::Assertion(root) => {
                    let recipe = self.source.assertion_recipe(*root)?;
                    return match recipe.annotation {
                        Some(annotation) => Ok(self.check_type_completion(&annotation)?.map(Some)),
                        None => Ok(NativeCompletion::clean(None)),
                    };
                }
                _ if expression.root.is_some_and(|root| {
                    matches!(
                        root.site.kind,
                        SyntaxKind::StringLiteral
                            | SyntaxKind::NumericLiteral
                            | SyntaxKind::BigIntLiteral
                            | SyntaxKind::RegularExpressionLiteral
                            | SyntaxKind::NoSubstitutionTemplateLiteral
                            | SyntaxKind::TrueKeyword
                            | SyntaxKind::FalseKeyword
                    )
                }) =>
                {
                    return Ok(self
                        .check_expression_completion(expression, CheckMode::NORMAL)?
                        .map(Some));
                }
                ExpressionForm::Call(root) => *root,
                _ => return Ok(NativeCompletion::clean(None)),
            };
            let recipe = self.source.call_recipe(root)?;
            match recipe.kind {
                SyntaxKind::CallExpression => {
                    if recipe.super_call
                        || recipe.require_call
                        || sequence.read(self.is_symbol_or_symbol_for_call_completion(&recipe))?
                        || recipe.import_call
                    {
                        return Ok(NativeCompletion::clean(None));
                    }
                    if recipe.optional {
                        return self
                            .check_expression_completion(&recipe.callee, CheckMode::NORMAL)?
                            .and_then(|ty| {
                                let non_optional = self.optional_expression_type(
                                    ty,
                                    recipe.callee_optional_root,
                                    recipe.callee_optional_chain,
                                    &recipe.callee.location,
                                )?;
                                // Native deliberately asks the original funcType here,
                                // unlike checked overload resolution's nonOptionalType.
                                self.single_nongeneric_return_completion(
                                    ty,
                                    false,
                                    &recipe.location,
                                )?
                                .and_then_value(|returned| match returned {
                                    Some(returned) => self
                                        .propagate_optional_marker(
                                            returned,
                                            non_optional != ty,
                                            recipe.outermost_chain,
                                            &recipe.location,
                                        )
                                        .map(Some),
                                    None => Ok(None),
                                })
                            });
                    }
                    self.check_expression_completion(&recipe.callee, CheckMode::NORMAL)?
                        .and_then(|ty| {
                            let ty = self.check_non_null_type(ty, &recipe.callee.location)?;
                            self.single_nongeneric_return_completion(ty, false, &recipe.location)
                        })
                }
                SyntaxKind::NewExpression => self
                    .check_expression_completion(&recipe.callee, CheckMode::NORMAL)?
                    .and_then(|ty| {
                        let ty = self.check_non_null_type(ty, &recipe.callee.location)?;
                        self.single_nongeneric_return_completion(ty, true, &recipe.location)
                    }),
                _ => Ok(NativeCompletion::clean(None)),
            }
        })();
        let value = result.and_then(|value| sequence.read(Ok(value)));
        sequence.finish(value)
    }

    /// `getSingleSignature(type,kind,true)` does not acquire apparent primitive
    /// wrappers. Only actual object types can have this shortcut; opposite
    /// signature kinds must be empty even when own members are allowed.
    fn single_nongeneric_return_completion(
        &mut self,
        ty: TypeId,
        construct: bool,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let Some(signature) = self.single_signature(ty, construct, true, consumer)? else {
            return Ok(NativeCompletion::clean(None));
        };
        if signature.type_parameter_types().is_empty() {
            Ok(self
                .get_return_type_of_signature_completion(&signature, consumer)?
                .map(Some))
        } else {
            Ok(NativeCompletion::clean(None))
        }
    }

    /// `checkCallExpression` demands the chosen signature's return after
    /// getResolvedSignature. Super returns Void after deprecation checking,
    /// without reading the selected signature's return. The assertion tail
    /// retains diagnostic acquisitions while returning the established
    /// signature type.
    pub(super) fn check_call_expression_completion(
        &mut self,
        root: CallRoot,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        let recipe = self.source.call_recipe(root)?;
        if recipe.kind == SyntaxKind::TaggedTemplateExpression {
            return self.check_tagged_template_expression_completion(&recipe);
        }
        if recipe.import_call {
            return Err(self.pending_call(
                &recipe.location,
                "native checkImportCallExpression module and promise return acquisition is pending",
            ));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let signature =
                match sequence.read(self.resolved_signature_completion(&recipe, mode))? {
                    ResolvedSignature::Signature(signature) => signature,
                    ResolvedSignature::Deferred => return Ok(self.model.silent_never()),
                };
            sequence
                .read(self.check_deprecated_signature_completion(&signature, &recipe.location))?;
            if recipe.super_call {
                return Ok(self.model.special(SpecialName::Void));
            }
            if recipe.kind == SyntaxKind::NewExpression
                && let Some(declaration) = self.signature_declaration_root(signature.id)?
                && !matches!(
                    declaration.site().kind,
                    SyntaxKind::Constructor
                        | SyntaxKind::ConstructSignature
                        | SyntaxKind::ConstructorType
                )
            {
                // A call-only signature used with `new` returns Any without
                // acquiring that signature's return, including noImplicitAny.
                return Ok(self.model.special(SpecialName::Any));
            }
            let returned = sequence
                .read(self.get_return_type_of_signature_completion(&signature, &recipe.location))?;
            if self
                .model
                .has_type_flags(returned, TypeFlags::SYMBOL | TypeFlags::UNIQUE_SYMBOL)
                && sequence.read(self.is_symbol_or_symbol_for_call_completion(&recipe))?
            {
                let root = super::super::source::ExpressionRoot {
                    file: recipe.root.file,
                    site: recipe.root.site(),
                };
                return match self.source.es_symbol_parent(root, false)? {
                    Some(parent) => self.es_symbol_like_type_for_node(parent, &recipe.location),
                    None => Ok(self.model.primitive(crate::types::PrimitiveName::Symbol)),
                };
            }
            if recipe.assertion_statement
                && self.model.has_type_flags(returned, TypeFlags::VOID)
                && sequence
                    .read(
                        self.type_predicate_of_signature_completion(&signature, &recipe.location),
                    )?
                    .is_some()
                && recipe.dotted_callee
                && sequence
                    .read(self.effects_signature_completion(root.file, root.site()))?
                    .is_none()
            {
                // A nondotted target emits only TS2776. Missing explicit
                // annotations additionally acquire symbol displays (TS2775).
                sequence.read(self.explicit_dotted_type_completion(&recipe.callee, true))?;
            }
            Ok(returned)
        })();
        sequence.finish(result)
    }

    /// `getResolvedSignature` retains one actual call-site cache and the
    /// shared type-resolution window. A temporary active-loop result is
    /// returned without replacing the site's prior native cache property.
    pub(super) fn resolved_signature(
        &mut self,
        recipe: &CallRecipe,
        mode: CheckMode,
    ) -> Result<ResolvedSignature, DemandError> {
        self.resolved_signature_completion(recipe, mode)?.admit()
    }

    pub(super) fn resolved_signature_completion(
        &mut self,
        recipe: &CallRecipe,
        mode: CheckMode,
    ) -> NativeResult<ResolvedSignature> {
        let cached = self.calls.resolved.get(&recipe.root).cloned();
        match &cached {
            Some(CallState::Resolved(signature)) => {
                return Ok(signature.clone().map(ResolvedSignature::Signature));
            }
            Some(CallState::Failed(error)) => return Err(error.clone()),
            Some(CallState::Resolving) | None => {}
        }
        let previous = self.resolution.begin_call(cached.is_none());
        self.calls
            .resolved
            .insert(recipe.root, CallState::Resolving);
        let result = self.resolve_signature_completion(recipe, mode);
        self.resolution.end_call(previous);
        if matches!(&result, Ok(completed) if matches!(completed.value(), ResolvedSignature::Deferred))
        {
            return result;
        }
        // The native final nil check selects an installed signature's value.
        // It neither re-acquires that slot's evidence nor erases this
        // invocation's acquisitions. An interrupted outer invocation leaves a
        // signature installed by nested contextual checking intact.
        let installed = match self.calls.resolved.get(&recipe.root) {
            Some(CallState::Resolved(signature)) => Some(signature.value().clone()),
            _ => None,
        };
        let result = result.map(|completed| match installed.as_ref() {
            Some(signature) => completed.map(|_| ResolvedSignature::Signature(signature.clone())),
            None => completed,
        });
        if self.reads.loops.active.is_empty() {
            if installed.is_none() {
                let state = match &result {
                    Ok(completed) => {
                        CallState::Resolved(completed.clone().map(|resolved| match resolved {
                            ResolvedSignature::Signature(signature) => signature,
                            ResolvedSignature::Deferred => {
                                unreachable!("deferred resolution returned above")
                            }
                        }))
                    }
                    Err(error) => CallState::Failed(error.clone()),
                };
                self.calls.resolved.insert(recipe.root, state);
            }
        } else if let Some(cached) = cached {
            self.calls.resolved.insert(recipe.root, cached);
        } else {
            self.calls.resolved.remove(&recipe.root);
        }
        result
    }

    /// `resolveSignature` preserves the native dispatch; new/tagged/decorator,
    /// JSX and instanceof are different argument/acquisition operations.
    fn resolve_signature_completion(
        &mut self,
        recipe: &CallRecipe,
        mode: CheckMode,
    ) -> NativeResult<ResolvedSignature> {
        match recipe.kind {
            SyntaxKind::CallExpression => self.resolve_call_expression_completion(recipe, mode),
            SyntaxKind::NewExpression => self.resolve_new_expression_completion(recipe, mode),
            SyntaxKind::TaggedTemplateExpression => {
                self.resolve_tagged_template_expression_completion(recipe, mode)
            }
            SyntaxKind::BinaryExpression => {
                self.resolve_instanceof_expression_completion(recipe, mode)
            }
            _ => Err(
                super::super::internal("call recipe has an unhandled native signature role").into(),
            ),
        }
    }

    /// `resolveCallExpression`: callee checking precedes non-null/apparent
    /// type and both signature inventories. Genuine any has an untyped path;
    /// a missing/error target remains its original consumed failure.
    fn resolve_call_expression_completion(
        &mut self,
        recipe: &CallRecipe,
        mode: CheckMode,
    ) -> NativeResult<ResolvedSignature> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if recipe.super_call {
                let root = recipe
                    .callee
                    .root
                    .ok_or_else(|| super::super::internal("super callee has no actual node"))?;
                let super_type = sequence
                    .read(self.check_super_expression_completion(root, &recipe.callee.location))?;
                // Native's Any test includes errorType. It visits arguments
                // directly here, without resolveUntypedCall's type arguments.
                if self.model.has_type_flags(super_type, TypeFlags::ANY) {
                    for argument in &recipe.arguments {
                        sequence
                            .read(self.check_expression_completion(argument, CheckMode::NORMAL))?;
                    }
                    return Ok(SignaturePreparation::Resolved(
                        ResolvedSignature::Signature(self.any_signature()?),
                    ));
                }
                if !self.model.is_error_type(super_type) {
                    let node = self.source.declaration_from_expression_root(
                        super::super::source::ExpressionRoot {
                            file: recipe.root.file,
                            site: recipe.root.site(),
                        },
                    )?;
                    if let Some(class) = self.source.containing_class(node)?
                        && let Some(base) = self.source.class_extends_recipe(class)?
                    {
                        let constructors = sequence.read(
                            self.instantiated_constructors_for_type_arguments_completion(
                                super_type,
                                &base.type_arguments,
                                &base.reference.location,
                            ),
                        )?;
                        return Ok(SignaturePreparation::Call(PreparedCall {
                            signatures: constructors,
                            chain: None,
                            mode,
                            construct_via_call: false,
                        }));
                    }
                }
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(
                        sequence.read(self.resolve_untyped_call_completion(recipe))?,
                    ),
                ));
            }
            if recipe.import_call {
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(
                        sequence.read(self.resolve_untyped_call_completion(recipe))?,
                    ),
                ));
            }
            let original = sequence
                .read(self.check_expression_completion(&recipe.callee, CheckMode::NORMAL))?;
            let ty = if recipe.optional {
                self.optional_expression_type(
                    original,
                    recipe.callee_optional_root,
                    recipe.callee_optional_chain,
                    &recipe.callee.location,
                )?
            } else {
                original
            };
            let chain = (ty != original).then_some(if recipe.outermost_chain {
                CallChain::Outer
            } else {
                CallChain::Inner
            });
            let ty = self.check_non_null_type(ty, &recipe.callee.location)?;
            if self.model.is_silent_never(ty) {
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(self.silent_never_signature()?),
                ));
            }
            let apparent =
                sequence.read(self.apparent_type_completion(ty, &recipe.callee.location))?;
            if self.model.is_error_type(apparent) {
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(
                        sequence.read(self.resolve_error_call_completion(recipe))?,
                    ),
                ));
            }
            let calls =
                sequence.read(self.call_signatures_completion(apparent, &recipe.location))?;
            let constructs =
                sequence.read(self.construct_signatures_completion(apparent, &recipe.location))?;
            if sequence.read(self.is_untyped_function_call_completion(
                ty,
                apparent,
                calls.len(),
                constructs.len(),
                &recipe.location,
            ))? {
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(
                        sequence.read(self.resolve_untyped_call_completion(recipe))?,
                    ),
                ));
            }
            if calls.is_empty() {
                if !constructs.is_empty() {
                    sequence.read(self.diagnostic_type_display_completion(
                        ty,
                        None,
                        super::native_display::DiagnosticTypeFormat::DEFAULT,
                        &recipe.location,
                    ))?;
                } else {
                    sequence.read(self.invocation_error_dependencies_completion(
                        apparent,
                        false,
                        &recipe.callee.location,
                    ))?;
                }
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(
                        sequence.read(self.resolve_error_call_completion(recipe))?,
                    ),
                ));
            }
            if mode.contains(CheckMode::SKIP_GENERIC_FUNCTIONS) && recipe.type_arguments.is_empty()
            {
                for signature in calls.iter() {
                    if !signature.type_parameter_types().is_empty() {
                        let returns =
                            sequence.read(self.get_return_type_of_signature_completion(
                                signature,
                                &recipe.location,
                            ))?;
                        if self.model.has_type_flags(returns, TypeFlags::OBJECT)
                            && !sequence
                                .read(self.call_signatures_completion(returns, &recipe.location))?
                                .is_empty()
                        {
                            self.record_skipped_generic_function(
                                super::super::source::ExpressionRoot {
                                    file: recipe.root.file,
                                    site: recipe.root.site(),
                                },
                                mode,
                            )?;
                            return Ok(SignaturePreparation::Resolved(ResolvedSignature::Deferred));
                        }
                    }
                }
            }
            Ok(SignaturePreparation::Call(PreparedCall {
                signatures: calls,
                chain,
                mode,
                construct_via_call: false,
            }))
        })();
        let prepared = sequence.finish(result)?;
        self.resolve_prepared_signature_completion(recipe, prepared)
    }

    /// Complete native resolveCallExpression/resolveNewExpression/tagged
    /// preparation without replaying its prefix in a parent accumulator.
    fn resolve_prepared_signature_completion(
        &mut self,
        recipe: &CallRecipe,
        prepared: NativeCompletion<SignaturePreparation>,
    ) -> NativeResult<ResolvedSignature> {
        let construct_via_call = match prepared.value() {
            SignaturePreparation::Resolved(_) => {
                return Ok(prepared.map(|prepared| match prepared {
                    SignaturePreparation::Resolved(signature) => signature,
                    SignaturePreparation::Call(_) => {
                        unreachable!("preparation discriminant just inspected")
                    }
                }));
            }
            SignaturePreparation::Call(call) => call.construct_via_call,
        };
        let call = prepared.map(|prepared| match prepared {
            SignaturePreparation::Call(call) => call,
            SignaturePreparation::Resolved(_) => {
                unreachable!("preparation discriminant just inspected")
            }
        });
        let signature = self.resolve_call_completion(recipe, call)?;
        let signature = if construct_via_call {
            signature.and_then(|signature| {
                self.finish_new_call_signature_completion(recipe, signature)
            })?
        } else {
            signature
        };
        Ok(signature.map(ResolvedSignature::Signature))
    }

    /// `resolveCall`: source arguments, selection and the actual recovery
    /// factory precede its early cache publication. Diagnostic continuation
    /// extends the returned copy, never the installed signature's evidence.
    fn resolve_call_completion(
        &mut self,
        recipe: &CallRecipe,
        prepared: NativeCompletion<PreparedCall>,
    ) -> NativeResult<Signature> {
        let mut sequence = NativeSequence::new();
        let mut recovery = None;
        let result = (|| {
            let prepared = sequence.read(Ok(prepared))?;
            for argument in &recipe.type_arguments {
                sequence.read(self.check_type_source_element_completion(argument.root))?;
            }
            let candidates = sequence.read(self.reorder_candidates_completion(
                &prepared.signatures,
                prepared.chain,
                &recipe.location,
            ))?;
            if candidates.is_empty() {
                return self.unknown_signature();
            }
            let input = sequence.read(self.effective_call_arguments_completion(recipe))?;
            let single_nongeneric =
                candidates.len() == 1 && candidates[0].type_parameter_types().is_empty();
            let mut argument_mode = CheckMode::NORMAL;
            if !single_nongeneric {
                for argument in &input.arguments {
                    if let CallArgument::Captured(expression) = argument
                        && let Some(root) = expression.root
                        && self.source.expression_is_context_sensitive(root)?
                    {
                        argument_mode = CheckMode::SKIP_CONTEXT_SENSITIVE;
                        break;
                    }
                }
            }
            let mut selection = OverloadSelection {
                candidates,
                argument_mode,
                single_nongeneric,
                errors: OverloadErrors::default(),
            };
            if selection.candidates.len() > 1
                && let Some(signature) = sequence.read(self.choose_overload_completion(
                    &input,
                    &mut selection,
                    RelationKind::Subtype,
                ))?
            {
                return Ok(signature);
            }
            if let Some(signature) = sequence.read(self.choose_overload_completion(
                &input,
                &mut selection,
                RelationKind::Assignable,
            ))? {
                return Ok(signature);
            }
            let signature = sequence.read(self.candidate_for_overload_failure_completion(
                &input,
                &mut selection.candidates,
                prepared.mode,
            ))?;
            let signature = sequence.read(NativeCompletion::recovered(
                signature,
                super::super::invalid_at_location(
                    self.inputs,
                    &recipe.location,
                    SemanticFailureReason::OverloadRecovery,
                    "native call has no applicable candidate and consumes overload recovery",
                )
                .into(),
            ))?;
            recovery = Some((input, selection, prepared.signatures));
            Ok(signature)
        })();
        let completed = sequence.finish(result)?;
        let Some((input, selection, originals)) = recovery else {
            return Ok(completed);
        };
        self.calls
            .resolved
            .insert(recipe.root, CallState::Resolved(completed.clone()));
        completed.and_then(|signature| {
            self.report_call_resolution_errors_completion(&input, &selection, &originals)
                .map(|reported| reported.map(|()| signature))
        })
    }

    /// `reorderCandidates`: `getSymbolOfDeclaration` groups and actual Parent
    /// nodes, plus the native specialized bit, own ordering. Overloads of one
    /// global function declared in several files share their merged symbol.
    fn reorder_candidates_completion(
        &mut self,
        candidates: &[Signature],
        chain: Option<CallChain>,
        consumer: &Location,
    ) -> NativeResult<Vec<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut result = Vec::with_capacity(candidates.len());
            let mut last_parent: Option<SignatureParent> = None;
            let mut last_symbol = None;
            let mut index = 0;
            let mut cutoff = 0;
            let mut specialized_count = 0;
            for signature in candidates {
                let (symbol, parent) = if let Some(root) =
                    self.signature_declaration_root(signature.id)?
                {
                    let facts = self.source.signature_declaration_facts(root)?;
                    let symbol = sequence
                        .read(self.reference_node_declaration_symbol_completion(facts.declaration))?
                        .map(|symbol| self.reference_symbol_identity(symbol, consumer))
                        .transpose()?;
                    (symbol, Some(facts.parent))
                } else {
                    (None, None)
                };
                if last_symbol.is_none() || symbol == last_symbol {
                    if last_parent.is_some() && parent == last_parent {
                        index += 1;
                    } else {
                        last_parent = parent;
                        index = cutoff;
                    }
                } else {
                    index = result.len();
                    cutoff = result.len();
                    last_parent = parent;
                }
                last_symbol = symbol;
                let splice = if signature.flags.has_literal_types() {
                    let splice = specialized_count;
                    specialized_count += 1;
                    cutoff += 1;
                    splice
                } else {
                    index
                };
                if splice > result.len() {
                    return Err(super::super::internal(
                        "native overload reorder splice exceeds candidate inventory",
                    )
                    .into());
                }
                let signature = match chain {
                    Some(chain) => self.optional_call_signature(signature, chain, consumer)?,
                    None => signature.clone(),
                };
                result.insert(splice, signature);
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `chooseOverload`: arity precedes applicability, ordinary False proceeds
    /// to the next actual candidate, and consumed failures stop selection.
    fn choose_overload_completion(
        &mut self,
        input: &CallInput<'_>,
        selection: &mut OverloadSelection,
        relation: RelationKind,
    ) -> NativeResult<Option<Signature>> {
        selection.errors = OverloadErrors::default();
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = input.recipe;
            if selection.single_nongeneric {
                let candidate = &selection.candidates[0];
                if !recipe.type_arguments.is_empty()
                    || !sequence.read(self.correct_arity_completion(input, candidate))?
                {
                    return Ok(None);
                }
                return if sequence.read(self.signature_applicable_completion(
                    input,
                    candidate,
                    RelationKind::Assignable,
                    CheckMode::NORMAL,
                ))? {
                    Ok(Some(candidate.clone()))
                } else {
                    selection.errors.arguments.push(candidate.clone());
                    Ok(None)
                };
            }
            for index in 0..selection.candidates.len() {
                let candidate = selection.candidates[index].clone();
                if !self.correct_type_argument_arity(&candidate, recipe.type_arguments.len())?
                    || !sequence.read(self.correct_arity_completion(input, &candidate))?
                {
                    continue;
                }
                let mut context = None;
                let mut checked = if candidate.type_parameter_types().is_empty() {
                    candidate.clone()
                } else {
                    let arguments = if recipe.type_arguments.is_empty() {
                        let inference = self.new_call_inference_context(&candidate);
                        context = Some(inference);
                        let arguments =
                            sequence.read(self.infer_call_type_arguments_completion(
                                input,
                                &candidate,
                                inference,
                                selection.argument_mode | CheckMode::SKIP_GENERIC_FUNCTIONS,
                            ))?;
                        if self.inference_skipped_generic_function(inference) {
                            selection.argument_mode =
                                selection.argument_mode | CheckMode::SKIP_GENERIC_FUNCTIONS;
                        }
                        arguments
                    } else {
                        let Some(arguments) =
                            sequence.read(self.check_type_arguments_completion(
                                &candidate,
                                &recipe.type_arguments,
                                RelationReporting::Silent,
                                &recipe.location,
                            ))?
                        else {
                            selection.errors.type_arguments = Some(candidate.clone());
                            continue;
                        };
                        arguments
                    };
                    let checked = sequence.read(self.get_signature_instantiation_completion(
                        &candidate,
                        arguments,
                        &recipe.location,
                    ))?;
                    if sequence
                        .read(
                            self.signature_non_array_rest_completion(&candidate, &recipe.location),
                        )?
                        .is_some()
                        && !sequence.read(self.correct_arity_completion(input, &checked))?
                    {
                        selection.errors.argument_arity = Some(checked);
                        continue;
                    }
                    checked
                };
                if !sequence.read(self.signature_applicable_completion(
                    input,
                    &checked,
                    relation,
                    selection.argument_mode,
                ))? {
                    selection.errors.arguments.push(checked);
                    continue;
                }
                if selection.argument_mode != CheckMode::NORMAL {
                    selection.argument_mode = CheckMode::NORMAL;
                    if let Some(context) = context {
                        let arguments =
                            sequence.read(self.infer_call_type_arguments_completion(
                                input,
                                &candidate,
                                context,
                                CheckMode::NORMAL,
                            ))?;
                        checked = sequence.read(self.get_signature_instantiation_completion(
                            &candidate,
                            arguments,
                            &recipe.location,
                        ))?;
                        if sequence
                            .read(
                                self.signature_non_array_rest_completion(
                                    &candidate,
                                    &recipe.location,
                                ),
                            )?
                            .is_some()
                            && !sequence.read(self.correct_arity_completion(input, &checked))?
                        {
                            selection.errors.argument_arity = Some(checked);
                            continue;
                        }
                    }
                    if !sequence.read(self.signature_applicable_completion(
                        input,
                        &checked,
                        relation,
                        selection.argument_mode,
                    ))? {
                        selection.errors.arguments.push(checked);
                        continue;
                    }
                }
                selection.candidates[index] = checked.clone();
                return Ok(Some(checked));
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `checkTypeArguments` fills every omitted slot before checking the
    /// constraints of actual source type-argument nodes. A rejected constraint
    /// is ordinary candidate-selection data; a consumed failure stays located.
    /// `hasCorrectTypeArgumentArity`: even an omitted list first observes the
    /// signature's minimum count; overload and instantiation filtering share it.
    pub(super) fn correct_type_argument_arity(
        &mut self,
        signature: &Signature,
        count: usize,
    ) -> Result<bool, DemandError> {
        let parameters = signature.type_parameter_types();
        let minimum = self.minimum_type_argument_count(parameters)?;
        Ok(count == 0 || count >= minimum && count <= parameters.len())
    }

    /// `checkTypeArguments` is shared by calls and instantiation expressions.
    /// Ordinary constraint rejection returns nil; consumed failures stay errors.
    pub(super) fn check_type_arguments_completion(
        &mut self,
        signature: &Signature,
        nodes: &[CallTypeArgument],
        reporting: RelationReporting,
        consumer: &Location,
    ) -> NativeResult<Option<Vec<TypeId>>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut supplied = Vec::with_capacity(nodes.len());
            for argument in nodes {
                supplied.push(sequence.read(self.check_type_completion(&argument.expression))?);
            }
            let arguments = sequence.read(self.fill_missing_type_arguments_completion(
                supplied,
                signature.type_parameter_types(),
                consumer,
            ))?;
            let mut mapper: Option<MapperId> = None;
            for (index, node) in nodes.iter().enumerate() {
                let error_context = reporting.constraint_at(node.root);
                let node = &node.expression;
                let parameter = signature.type_parameter_types()[index];
                if let Some(constraint) = sequence
                    .read(self.parameter_constraint_completion(parameter, &node.location))?
                {
                    let mapper = match mapper {
                        Some(mapper) => mapper,
                        None => {
                            let created = self.model.mapper(
                                signature.type_parameter_types().to_vec(),
                                arguments.clone(),
                            )?;
                            mapper = Some(created);
                            created
                        }
                    };
                    let constraint = self.instantiate_type(constraint, mapper, &node.location)?;
                    let constraint = self.get_type_with_this_argument(
                        constraint,
                        Some(arguments[index]),
                        false,
                        &node.location,
                    )?;
                    if !sequence.read(self.check_type_related_to_completion(
                        arguments[index],
                        constraint,
                        RelationKind::Assignable,
                        error_context,
                        &node.location,
                    ))? {
                        return Ok(None);
                    }
                }
            }
            Ok(Some(arguments))
        })();
        sequence.finish(result)
    }

    /// `hasCorrectArity`: getParameterCount precedes getMinArgumentCount,
    /// then spread early-return, too-many arguments and omitted Void
    /// acceptance. Source guarantees complete ordinary argument lists.
    fn correct_arity_completion(
        &mut self,
        input: &CallInput<'_>,
        signature: &Signature,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let consumer = &input.recipe.location;
            let parameter_count =
                sequence.read(self.signature_parameter_count_completion(signature, consumer))?;
            let minimum =
                sequence.read(self.minimum_argument_count_completion(signature, consumer))?;
            if input.recipe.kind == SyntaxKind::NewExpression && !input.recipe.has_argument_list {
                return Ok(sequence
                    .read(self.minimum_argument_count_completion(signature, consumer))?
                    == 0);
            }
            if let Some(spread) = input.arguments.iter().position(CallArgument::is_spread) {
                return Ok(spread
                    >= sequence
                        .read(self.minimum_argument_count_completion(signature, consumer))?
                    && (sequence.read(
                        self.signature_has_effective_rest_completion(signature, consumer),
                    )? || spread
                        < sequence.read(
                            self.signature_parameter_count_completion(signature, consumer),
                        )?));
            }
            if !sequence.read(self.signature_has_effective_rest_completion(signature, consumer))?
                && input.arguments.len() > parameter_count
            {
                return Ok(false);
            }
            if input.arguments.len() >= minimum {
                return Ok(true);
            }
            for index in input.arguments.len()..minimum {
                let ty = sequence.read(
                    self.signature_type_at_position_or_any_completion(signature, index, consumer),
                )?;
                if !self.contains_void(ty) {
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        sequence.finish(result)
    }

    fn signature_applicable_completion(
        &mut self,
        input: &CallInput<'_>,
        signature: &Signature,
        relation: RelationKind,
        mode: CheckMode,
    ) -> NativeResult<bool> {
        Ok(self
            .signature_applicability_completion(
                input,
                signature,
                relation,
                mode,
                RelationReporting::Silent,
            )?
            .map(|check| check.related))
    }

    /// `isSignatureApplicable` consumes this, positional arguments, then the
    /// single rest tuple. Reporting uses actual diagnostic output rather than
    /// treating every failed comparison as an emitted error.
    fn signature_applicability_completion(
        &mut self,
        input: &CallInput<'_>,
        signature: &Signature,
        relation: RelationKind,
        mode: CheckMode,
        reporting: RelationReporting,
    ) -> NativeResult<super::relations::RelationCheck> {
        use super::relations::RelationCheck;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = input.recipe;
            let call = super::super::source::ExpressionRoot {
                file: recipe.root.file,
                site: recipe.root.site(),
            };
            let mut diagnostics = 0;
            if let Some(ty) = sequence.read(self.signature_this_type_completion(signature))?
                && ty != self.model.special(SpecialName::Void)
                && recipe.kind != SyntaxKind::NewExpression
            {
                let argument = sequence.read(self.call_this_argument_type_completion(recipe))?;
                let node = match self.source.call_this_argument(recipe.root)? {
                    Some(argument) => argument.expression.root.ok_or_else(|| {
                        super::super::internal("captured this argument has no expression root")
                    })?,
                    None => call,
                };
                let checked = sequence.read(self.check_type_related_to_outcome_completion(
                    argument,
                    ty,
                    relation,
                    reporting.this_context_at(node),
                    &recipe.location,
                ))?;
                diagnostics += checked.diagnostics;
                if !checked.related {
                    return Ok(RelationCheck {
                        related: false,
                        diagnostics,
                    });
                }
            }
            let rest = sequence
                .read(self.signature_non_array_rest_completion(signature, &recipe.location))?;
            let count = if rest.is_some() {
                input.arguments.len().min(
                    sequence.read(
                        self.signature_parameter_count_completion(signature, &recipe.location),
                    )? - 1,
                )
            } else {
                input.arguments.len()
            };
            for (index, argument) in input.arguments.iter().take(count).enumerate() {
                if argument.is_omitted() {
                    continue;
                }
                let contextual =
                    sequence.read(self.signature_type_at_position_or_any_completion(
                        signature,
                        index,
                        &recipe.location,
                    ))?;
                let ty = sequence
                    .read(self.check_call_argument_completion(argument, contextual, None, mode))?;
                let ty = if mode.contains(CheckMode::SKIP_CONTEXT_SENSITIVE) {
                    self.regular_object_literal_type(ty, argument.location())?
                } else {
                    ty
                };
                let node = self.call_argument_check_node(argument)?;
                let checked = sequence.read(
                    self.check_type_related_to_and_optionally_elaborate_outcome_completion(
                        ty,
                        contextual,
                        relation,
                        reporting.argument_at(node),
                        Some(node),
                        argument.location(),
                    ),
                )?;
                diagnostics += checked.diagnostics;
                if !checked.related {
                    sequence.read(self.missing_await_info_completion(
                        ty,
                        contextual,
                        relation,
                        reporting,
                        diagnostics,
                        true,
                        argument.location(),
                    ))?;
                    return Ok(RelationCheck {
                        related: false,
                        diagnostics,
                    });
                }
            }
            if let Some(rest) = rest {
                let spread = sequence.read(self.spread_argument_type_completion(
                    input,
                    count,
                    input.arguments.len(),
                    rest,
                    None,
                    mode,
                ))?;
                let error_location = if count == input.arguments.len() {
                    recipe.location.clone()
                } else {
                    Location {
                        file: input.arguments[count].location().file.clone(),
                        start: input.arguments[count].location().start,
                        end: input
                            .arguments
                            .last()
                            .expect("a rest argument exists")
                            .location()
                            .end,
                    }
                };
                let node = if matches!(reporting, RelationReporting::Report) {
                    Some(match input.arguments.len() - count {
                        0 => call.into(),
                        1 => self.call_argument_check_node(&input.arguments[count])?,
                        _ => self.new_synthetic_context_node(call),
                    })
                } else {
                    None
                };
                let checked = sequence.read(self.check_type_related_to_outcome_completion(
                    spread,
                    rest,
                    relation,
                    node.and_then(|node| reporting.argument_at(node)),
                    &error_location,
                ))?;
                diagnostics += checked.diagnostics;
                if !checked.related {
                    sequence.read(self.missing_await_info_completion(
                        spread,
                        rest,
                        relation,
                        reporting,
                        diagnostics,
                        node.is_some(),
                        &error_location,
                    ))?;
                    return Ok(RelationCheck {
                        related: false,
                        diagnostics,
                    });
                }
            }
            Ok(RelationCheck {
                related: true,
                diagnostics,
            })
        })();
        sequence.finish(result)
    }

    /// `getEffectiveCheckNode` preserves each synthetic node's own identity;
    /// only captured parentheses/satisfies syntax has an inner check node.
    fn call_argument_check_node(
        &mut self,
        argument: &CallArgument,
    ) -> Result<ContextNode, DemandError> {
        match argument {
            CallArgument::Captured(expression) => {
                let root = expression.root.ok_or_else(|| {
                    super::super::internal("captured call argument has no actual expression root")
                })?;
                Ok(self.source.effective_check_node(root)?.into())
            }
            CallArgument::Synthetic { node, .. } => Ok(*node),
        }
    }

    /// `maybeAddMissingAwaitInfo` probes target before source, only when the
    /// actual diagnostic output is nonempty and an error node exists.
    #[expect(
        clippy::too_many_arguments,
        reason = "ports maybeAddMissingAwaitInfo's inputs plus the request consumer"
    )]
    fn missing_await_info_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        reporting: RelationReporting,
        diagnostics: usize,
        has_node: bool,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if has_node && matches!(reporting, RelationReporting::Report) && diagnostics != 0 {
                if sequence
                    .read(self.awaited_type_of_promise_completion(target, consumer))?
                    .is_some()
                {
                    return Ok(());
                }
                if let Some(awaited) =
                    sequence.read(self.awaited_type_of_promise_completion(source, consumer))?
                {
                    sequence
                        .read(self.type_related_completion(awaited, target, relation, consumer))?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// The `getEffectiveCallArguments` count used by `isOptionalParameter`
    /// includes checked tuple spreads; it is not the written argument count.
    pub(super) fn effective_call_argument_count_completion(
        &mut self,
        root: CallRoot,
    ) -> NativeResult<usize> {
        let recipe = self.source.call_recipe(root)?;
        self.effective_call_arguments_completion(&recipe)
            .map(|input| input.map(|input| input.arguments.len()))
    }

    /// `getEffectiveCallArguments`: tuple spreads are checked and expanded
    /// once before candidate selection. Native flow loops deliberately bypass
    /// the ordinary expression cache for these spread operands.
    fn effective_call_arguments_completion<'a>(
        &mut self,
        recipe: &'a CallRecipe,
    ) -> NativeResult<CallInput<'a>> {
        if let Some(template) = &recipe.template {
            return self.effective_tagged_template_arguments_completion(recipe, template);
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut arguments = Vec::with_capacity(recipe.arguments.len());
            for expression in &recipe.arguments {
                if let Some(root) = expression
                    .root
                    .filter(|root| root.site.kind == SyntaxKind::SpreadElement)
                {
                    let operand = self.source.spread_operand(root)?;
                    let ty = if self.reads.loops.active.is_empty() {
                        let operand_root = operand.root.ok_or_else(|| {
                            super::super::internal("native spread operand has no captured root")
                        })?;
                        sequence.read(self.check_expression_cached_completion(
                            operand_root.file,
                            operand_root.site,
                        ))?
                    } else {
                        sequence
                            .read(self.check_expression_completion(&operand, CheckMode::NORMAL))?
                    };
                    if let Some(tuple) = self.model.tuple_target(ty)? {
                        let elements = self.get_element_types(ty)?;
                        for (&element, info) in elements.iter().zip(tuple.elements.iter()) {
                            let ty = if info.flags.intersects(TupleElementFlags::REST) {
                                self.array_type(element, &expression.location)?
                            } else {
                                element
                            };
                            arguments.push(CallArgument::Synthetic {
                                ty,
                                node: self.new_synthetic_context_node(root),
                                location: expression.location.clone(),
                                spread: info.flags.intersects(TupleElementFlags::VARIABLE),
                                label: info.label.clone(),
                            });
                        }
                        continue;
                    }
                }
                arguments.push(CallArgument::Captured(expression.clone()));
            }
            Ok(CallInput { recipe, arguments })
        })();
        sequence.finish(result)
    }

    /// `checkExpressionWithContextualType` applies equally to captured and
    /// synthetic expressions, including literal regularization and inference
    /// frame cleanup. The synthetic worker alone returns its retained type.
    pub(super) fn check_call_argument_completion(
        &mut self,
        argument: &CallArgument,
        contextual: TypeId,
        inference: Option<InferenceContextId>,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        match argument {
            CallArgument::Captured(expression) => self
                .check_expression_with_contextual_type_completion(
                    expression, contextual, inference, mode,
                ),
            CallArgument::Synthetic {
                ty,
                node,
                spread,
                location,
                ..
            } => self.check_synthetic_expression_with_contextual_type_completion(
                *node, *ty, *spread, contextual, inference, mode, location,
            ),
        }
    }

    /// `getSpreadArgumentType` constructs the one rest argument type used by
    /// applicability and inference. Single spreads retain native array/tuple
    /// identities; mixed arguments retain tuple labels and variable flags.
    pub(super) fn spread_argument_type_completion(
        &mut self,
        input: &CallInput<'_>,
        index: usize,
        count: usize,
        rest: TypeId,
        inference: Option<InferenceContextId>,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let consumer = &input.recipe.location;
            let in_const = self.is_const_type_variable(rest, 0, consumer)?;
            if count > 0 && index >= count - 1 {
                let argument = &input.arguments[count - 1];
                if argument.is_spread() {
                    // Keep the captured recipe alive while borrowing its range.
                    let operand;
                    let (ty, location, error) = match argument {
                        CallArgument::Synthetic {
                            ty, location, node, ..
                        } => (*ty, location, *node),
                        CallArgument::Captured(expression) => {
                            let root = expression.root.ok_or_else(|| {
                                super::super::internal(
                                    "native spread argument has no captured root",
                                )
                            })?;
                            operand = self.source.spread_operand(root)?;
                            let ty = sequence.read(
                                self.check_expression_with_contextual_type_completion(
                                    &operand, rest, inference, mode,
                                ),
                            )?;
                            (
                                ty,
                                &operand.location,
                                operand
                                    .root
                                    .ok_or_else(|| {
                                        super::super::internal("spread operand has no actual node")
                                    })?
                                    .into(),
                            )
                        }
                    };
                    if sequence.read(self.is_array_like_type_completion(ty, consumer))? {
                        return self.mutable_array_or_tuple_type(ty, consumer);
                    }
                    let element = sequence.read(self.checked_iteration_type_completion(
                        ty,
                        IterationUse::Spread,
                        Some(error),
                        location,
                    ))?;
                    return if in_const {
                        self.readonly_array_type(element, consumer)
                    } else {
                        self.array_type(element, consumer)
                    };
                }
            }
            let mut types = Vec::new();
            let mut infos = Vec::new();
            for (position, argument) in input.arguments.iter().enumerate().take(count).skip(index) {
                let (ty, flags) = if argument.is_spread() {
                    // Keep the captured recipe alive while borrowing its range.
                    let operand;
                    let (ty, location, error) = match argument {
                        CallArgument::Synthetic {
                            ty, location, node, ..
                        } => (*ty, location, *node),
                        CallArgument::Captured(expression) => {
                            let root = expression.root.ok_or_else(|| {
                                super::super::internal(
                                    "native spread argument has no captured root",
                                )
                            })?;
                            operand = self.source.spread_operand(root)?;
                            (
                                sequence.read(
                                    self.check_expression_completion(&operand, CheckMode::NORMAL),
                                )?,
                                &operand.location,
                                operand
                                    .root
                                    .ok_or_else(|| {
                                        super::super::internal("spread operand has no actual node")
                                    })?
                                    .into(),
                            )
                        }
                    };
                    if sequence.read(self.is_array_like_type_completion(ty, consumer))? {
                        (ty, TupleElementFlags::VARIADIC)
                    } else {
                        (
                            sequence.read(self.checked_iteration_type_completion(
                                ty,
                                IterationUse::Spread,
                                Some(error),
                                location,
                            ))?,
                            TupleElementFlags::REST,
                        )
                    }
                } else {
                    let contextual = if self.model.is_tuple_type(rest) {
                        sequence
                            .read(self.contextual_element_type_completion(
                                Some(rest),
                                position - index,
                                Some(count - index),
                                None,
                                None,
                                consumer,
                            ))?
                            .unwrap_or_else(|| self.model.special(SpecialName::Unknown))
                    } else {
                        let key = self.model.regular_literal_type(
                            crate::types::PrimitiveName::Number,
                            (position - index).to_string(),
                        );
                        sequence.read(self.indexed_access_type_ex_completion(
                            rest,
                            key,
                            AccessFlags::CONTEXTUAL,
                            None,
                            consumer,
                        ))?
                    };
                    let ty = sequence.read(
                        self.check_call_argument_completion(argument, contextual, inference, mode),
                    )?;
                    let primitive = in_const
                        || self.model.maybe_type_of_kind(
                            contextual,
                            TypeFlags::PRIMITIVE
                                | TypeFlags::INDEX
                                | TypeFlags::TEMPLATE_LITERAL
                                | TypeFlags::STRING_MAPPING,
                        );
                    (
                        if primitive {
                            self.model.regular_literal(ty)
                        } else {
                            sequence.read(self.widened_literal_type_completion(ty, consumer))?
                        },
                        TupleElementFlags::REQUIRED,
                    )
                };
                let label = match argument {
                    CallArgument::Synthetic { label, .. } => label.clone(),
                    CallArgument::Captured(_) => None,
                };
                types.push(ty);
                infos.push(TupleElementInfo { flags, label });
            }
            let mut readonly = in_const;
            if readonly {
                let alternatives = self
                    .model
                    .union_constituents(rest)
                    .map(|types| types.to_vec())
                    .unwrap_or_else(|| vec![rest]);
                for alternative in alternatives {
                    if sequence
                        .read(self.is_mutable_array_like_type_completion(alternative, consumer))?
                    {
                        readonly = false;
                        break;
                    }
                }
            }
            self.create_tuple_type_ex(types, infos, readonly, consumer)
        })();
        sequence.finish(result)
    }

    /// `getThisArgumentType` checks the actual receiver in Normal mode, then
    /// removes its nullable or optional marker according to the parent access.
    pub(super) fn call_this_argument_type_completion(
        &mut self,
        recipe: &CallRecipe,
    ) -> NativeResult<TypeId> {
        let Some(argument) = self.source.call_this_argument(recipe.root)? else {
            return Ok(NativeCompletion::clean(
                self.model.special(SpecialName::Void),
            ));
        };
        self.check_expression_completion(&argument.expression, CheckMode::NORMAL)?
            .and_then_value(|ty| {
                self.optional_expression_type(
                    ty,
                    argument.optional_root,
                    argument.optional_chain,
                    &argument.expression.location,
                )
            })
    }

    /// `resolveUntypedCall` checks actual type/argument expressions even
    /// though no contextual signature exists. Its healthy anySignature has
    /// one native identity, distinct from error/resolving signature sentinels.
    fn resolve_untyped_call_completion(&mut self, recipe: &CallRecipe) -> NativeResult<Signature> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for argument in &recipe.type_arguments {
                sequence.read(self.check_type_source_element_completion(argument.root))?;
            }
            if let Some(template) = &recipe.template {
                sequence.read(self.check_expression_completion(template, CheckMode::NORMAL))?;
            } else {
                for argument in &recipe.arguments {
                    sequence.read(self.check_expression_completion(argument, CheckMode::NORMAL))?;
                }
            }
            self.any_signature()
        })();
        sequence.finish(result)
    }

    /// The native anySignature sentinel has one actual identity. Callers that
    /// temporarily install it do not run resolveUntypedCall's child checks.
    fn any_signature(&mut self) -> Result<Signature, DemandError> {
        if let Some(signature) = &self.calls.any_signature {
            return Ok(signature.clone());
        }
        let ty = self.model.special(SpecialName::Any);
        let signature = self.model.signature(SignatureData {
            type_parameter_types: Vec::new().into(),
            type_parameters: Vec::new(),
            this_parameter: None,
            parameters: Vec::new().into(),
            return_type: super::super::model::TypeDemand::Known(ty),
            predicate: None,
            origins: Vec::new().into(),
            minimum_arguments: 0,
            flags: SignatureFlags::declared(false, false, false, false),
        })?;
        self.attach_signature_declaration(signature.id, None)?;
        self.calls.any_signature = Some(signature.clone());
        Ok(signature)
    }

    /// IIFE branch of `getContextuallyTypedParameterType`: effective arguments
    /// precede the rest branch. Ordinary arguments are checked with anySignature
    /// temporarily installed, and the exact previous call state is restored on
    /// both success and failure before exposing the widened result.
    pub(super) fn iife_parameter_type_completion(
        &mut self,
        root: CallRoot,
        index: usize,
        rest: bool,
        has_initializer: bool,
    ) -> NativeResult<Option<TypeId>> {
        let recipe = self.source.call_recipe(root)?;
        self.effective_call_arguments_completion(&recipe)?
            .and_then(|input| {
                if rest {
                    let any = self.model.special(SpecialName::Any);
                    return Ok(self
                        .spread_argument_type_completion(
                            &input,
                            index,
                            input.arguments.len(),
                            any,
                            None,
                            CheckMode::NORMAL,
                        )?
                        .map(Some));
                }
                let any = self.any_signature()?;
                let saved = self
                    .calls
                    .resolved
                    .insert(root, CallState::Resolved(NativeCompletion::clean(any)));
                let result = (|| {
                    if let Some(argument) = input.arguments.get(index) {
                        let ty = match argument {
                            CallArgument::Captured(expression) => {
                                self.check_expression_completion(expression, CheckMode::NORMAL)?
                            }
                            CallArgument::Synthetic {
                                ty,
                                node,
                                spread,
                                location,
                                ..
                            } => self.check_synthetic_expression_completion(
                                *node,
                                *ty,
                                *spread,
                                CheckMode::NORMAL,
                                location,
                            )?,
                        };
                        Ok(ty
                            .and_then(|ty| {
                                self.widened_literal_type_completion(ty, &recipe.location)
                            })?
                            .map(Some))
                    } else if has_initializer {
                        Ok(NativeCompletion::clean(None))
                    } else {
                        Ok(NativeCompletion::clean(Some(
                            self.model.undefined_initializer(),
                        )))
                    }
                })();
                match saved {
                    Some(saved) => {
                        self.calls.resolved.insert(root, saved);
                    }
                    None => {
                        self.calls.resolved.remove(&root);
                    }
                }
                result
            })
    }

    fn pending_call(&self, consumer: &Location, message: &'static str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }
}
