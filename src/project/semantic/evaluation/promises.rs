//! Promise adoption and awaited types own their native caches and recursion.

use super::super::model::{
    NativeCompletion, NativeResult, NativeSequence, Signature, SpecialName, TypeFlags, TypeId,
    TypeView, UnionReduction,
};
use super::globals::GlobalTypeLookup;
use super::native_display::DiagnosticTypeFormat;
use super::{Evaluator, facts::TypeFacts, relations::RelationKind};
use crate::references::Location;
use crate::resources::{FixedMap, complete_or_abort};

pub(super) struct PromisesState {
    // Only nonnil native cachedTypes values become hits. Its explicit nil
    // writes remain misses; neither nil nor interruption installs a value here.
    promised: FixedMap<TypeId, NativeCompletion<TypeId>>,
    awaited: FixedMap<TypeId, NativeCompletion<TypeId>>,
    stack: Vec<TypeId>,
}

impl PromisesState {
    pub(super) fn new() -> Self {
        Self {
            promised: FixedMap::new(),
            awaited: FixedMap::new(),
            stack: Vec::new(),
        }
    }
}

/// `getPromisedTypeOfPromiseEx`'s optional thisTypeForErrorOut is invocation
/// data. It is populated only when every then signature rejects its receiver;
/// cached promised values never replay a rejected candidate's this type.
#[derive(Clone)]
struct PromisedType {
    value: Option<TypeId>,
    rejected_this: Option<TypeId>,
}

impl PromisedType {
    fn value(value: Option<TypeId>) -> Self {
        Self {
            value,
            rejected_this: None,
        }
    }
}

#[derive(Clone, Copy)]
enum AwaitDiagnostics {
    Quiet,
    Checked,
    CheckedWithoutMessage,
}

impl Evaluator<'_, '_> {
    /// `createPromiseType` consumes the checked global before unwrapping its
    /// argument. Only a completed nil awaited value supplies native Unknown.
    pub(super) fn create_promise_type_completion(
        &mut self,
        promised: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.global_promise_type_checked_completion(consumer)?
            .and_then(|target| {
                self.instantiate_promise_factory_completion(target, promised, consumer)
            })
    }

    /// `createPromiseLikeType` consumes its checked global getter's actual
    /// result, then shares the same argument adoption as createPromiseType.
    pub(super) fn create_promise_like_type_completion(
        &mut self,
        promised: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.global_promise_like_type_completion(consumer)?
            .and_then(|target| {
                self.instantiate_promise_factory_completion(target, promised, consumer)
            })
    }

    /// Argument/factory portion of `createPromiseType` / `createPromiseLikeType`.
    /// Global acquisition is outside this shared worker so each entry keeps
    /// its own native getter identity and diagnostic policy.
    fn instantiate_promise_factory_completion(
        &mut self,
        target: GlobalTypeLookup,
        promised: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let GlobalTypeLookup::Target { ty: target, .. } = target else {
                return Ok(self.model.special(SpecialName::Unknown));
            };
            let promised =
                sequence.read(self.unwrap_awaited_type_completion(promised, consumer))?;
            let promised =
                sequence.read(self.awaited_type_no_alias_completion(promised, consumer))?;
            let promised = match promised {
                Some(promised) => promised,
                None => self.model.special(SpecialName::Unknown),
            };
            self.model.reference(target, vec![promised])
        })();
        sequence.finish(result)
    }

    /// `checkAwaitExpression` / `checkAwaitedType(withAlias=true)` checks the
    /// operand in Normal mode and converts only actual nil into errorType.
    /// Grammar and no-effect suggestions acquire no additional types.
    pub(super) fn check_await_expression_completion(
        &mut self,
        operand: &super::super::source::Expression,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.check_expression_completion(operand, super::contextual::CheckMode::NORMAL)?
            .and_then(|ty| self.check_awaited_type_completion(ty, true, consumer))
    }

    /// `checkAwaitedType` consumes actual nil as errorType; withAlias selects
    /// the native alias factory independently of checked diagnostic traversal.
    pub(super) fn check_awaited_type_completion(
        &mut self,
        ty: TypeId,
        with_alias: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let awaited = if with_alias {
            self.awaited_type_ex_completion(ty, AwaitDiagnostics::Checked, consumer)?
        } else {
            self.awaited_type_no_alias_ex_completion(ty, AwaitDiagnostics::Checked, consumer)?
        };
        awaited.and_then(|awaited| match awaited {
                Some(ty) => Ok(NativeCompletion::clean(ty)),
                None => NativeCompletion::recovered(self.model.native_error_type(),
                    super::super::invalid_at_location(self.inputs, consumer,
                        crate::analysis::SemanticFailureReason::ErrorType,
                        "native checked await consumes errorType for a recursive or invalid thenable operand").into()),
            })
    }

    /// The async iteration resolver calls `getAwaitedTypeEx` with an optional
    /// error node. Nil stays nil for its caller's native fallback. Diagnostic
    /// text does not change the rejected-this display acquisitions.
    pub(super) fn awaited_type_for_iteration_completion(
        &mut self,
        ty: TypeId,
        report_errors: bool,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.awaited_type_ex_completion(
            ty,
            if report_errors {
                AwaitDiagnostics::Checked
            } else {
                AwaitDiagnostics::Quiet
            },
            consumer,
        )
    }

    /// `getAsyncFromSyncIterationTypes` supplies an error node but no message,
    /// unlike the async iteration resolver. Most branches need no message;
    /// the rejected-thenable diagnostic dereferences it in the pinned compiler.
    pub(super) fn awaited_type_from_sync_iteration_completion(
        &mut self,
        ty: TypeId,
        report_errors: bool,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.awaited_type_ex_completion(
            ty,
            if report_errors {
                AwaitDiagnostics::CheckedWithoutMessage
            } else {
                AwaitDiagnostics::Quiet
            },
            consumer,
        )
    }

    /// `createPromiseReturnType` adds its own nil-global/errorType branch and
    /// Promise constructor-symbol question after the shared Promise factory.
    pub(super) fn create_promise_return_type_completion(
        &mut self,
        promised: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.create_promise_type_completion(promised, consumer)?.and_then(|ty| {
            if ty == self.model.special(SpecialName::Unknown) {
                return NativeCompletion::recovered(self.model.native_error_type(),
                    super::super::invalid_at_location(self.inputs, consumer,
                        crate::analysis::SemanticFailureReason::ErrorType,
                        "native async return consumes errorType because no Promise type is available").into());
            }
            // Its missing value only emits a diagnostic; the actual Promise
            // type remains unchanged. This lookup performs no value-type read.
            self.global_value_symbol("Promise", consumer)?;
            Ok(NativeCompletion::clean(ty))
        })
    }

    /// `getPromisedTypeOfPromiseEx` with no diagnostic node. Its then-signature
    /// filtering retains the optional rejected-this output for checked await.
    fn promised_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<PromisedType> {
        let _depth = {
            self.control
                .as_ref()
                .map(|control| complete_or_abort(control.enter()))
        };
        if self.model.has_type_flags(ty, TypeFlags::ANY) {
            return Ok(NativeCompletion::clean(PromisedType::value(None)));
        }
        if let Some(cached) = self.promises.promised.get(&ty) {
            return Ok(cached.clone().map(|ty| PromisedType::value(Some(ty))));
        }
        let result = self.promised_type_worker_completion(ty, consumer);
        if let Ok(completed) = &result
            && let Some(value) = completed.value().value
        {
            self.promises
                .promised
                .insert(ty, completed.clone().map(|_| value));
        }
        result
    }

    /// Public `GetPromisedTypeOfPromise` projects the same worker's optional
    /// value; rejected-this metadata is used only by checked await diagnostics.
    pub(super) fn promised_type_of_promise_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        Ok(self
            .promised_type_completion(ty, consumer)?
            .map(|promised| promised.value))
    }

    /// Worker for `getPromisedTypeOfPromiseEx`. Candidate indices retain native
    /// signature order without copying another signature header inventory.
    fn promised_type_worker_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<PromisedType> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let global =
                match sequence.read(self.global_type_lookup_completion("Promise", 1, consumer))? {
                    GlobalTypeLookup::Target { ty, .. } => Some(ty),
                    // No admitted reference has the emptyGenericType recovery target.
                    GlobalTypeLookup::Recovery(_) => None,
                };
            if let TypeView::Reference(reference) = self.model.get(ty)?
                && reference.is_reference
                && Some(reference.target) == global
            {
                let arguments = sequence.read(self.get_type_arguments_completion(ty))?;
                return Ok(PromisedType::value(Some(arguments[0])));
            }
            let base = sequence
                .read(self.base_constraint_completion(ty, consumer))?
                .unwrap_or(ty);
            if sequence.read(self.all_types_assignable_to_kind_completion(
                base,
                TypeFlags::PRIMITIVE | TypeFlags::NEVER,
                false,
                consumer,
            ))? {
                return Ok(PromisedType::value(None));
            }
            let then_type =
                sequence.read(self.type_of_property_of_type_completion(ty, "then", consumer))?;
            if then_type.is_some_and(|ty| self.model.has_type_flags(ty, TypeFlags::ANY)) {
                return Ok(PromisedType::value(None));
            }
            let signatures = match then_type {
                Some(ty) => sequence.read(self.call_signatures_completion(ty, consumer))?,
                None => Vec::new(),
            };
            if signatures.is_empty() {
                return Ok(PromisedType::value(None));
            }
            let mut candidates = Vec::new();
            let mut rejected_this = None;
            for (index, signature) in signatures.iter().enumerate() {
                let this = sequence.read(self.signature_this_type_completion(signature))?;
                if let Some(this) = this
                    && this != self.model.special(SpecialName::Void)
                    && !sequence.read(self.type_related_completion(
                        ty,
                        this,
                        RelationKind::Subtype,
                        consumer,
                    ))?
                {
                    rejected_this = Some(this);
                } else {
                    candidates.push(index);
                }
            }
            if candidates.is_empty() {
                return Ok(PromisedType {
                    value: None,
                    rejected_this,
                });
            }
            let mut parameters = Vec::new();
            for candidate in candidates {
                let parameter =
                    sequence.read(self.first_signature_parameter_type_completion(
                        &signatures[candidate],
                        consumer,
                    ))?;
                {
                    parameters.push(parameter);
                }
            }
            let parameter =
                sequence.read(self.union_type_ex_completion(parameters, None, None, consumer))?;
            let parameter = sequence.read(self.type_with_facts_completion(
                parameter,
                TypeFacts::NOT_NULLABLE,
                consumer,
            ))?;
            if self.model.has_type_flags(parameter, TypeFlags::ANY) {
                return Ok(PromisedType::value(None));
            }
            let callbacks = sequence.read(self.call_signatures_completion(parameter, consumer))?;
            if callbacks.is_empty() {
                return Ok(PromisedType::value(None));
            }
            let mut values = Vec::new();
            for callback in callbacks {
                let value = sequence
                    .read(self.first_signature_parameter_type_completion(&callback, consumer))?;
                {
                    values.push(value);
                }
            }
            let value = sequence.read(self.union_type_reduced_completion(
                values,
                UnionReduction::Subtype,
                None,
                None,
                consumer,
            ))?;
            Ok(PromisedType::value(Some(value)))
        })();
        sequence.finish(result)
    }

    /// `getTypeOfFirstParameterOfSignature`: native absence is Never, unlike
    /// `getTypeAtPosition`'s separate out-of-range fallback to Any.
    fn first_signature_parameter_type_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if signature.parameters.is_empty() {
            Ok(NativeCompletion::clean(
                self.model.special(SpecialName::Never),
            ))
        } else {
            self.signature_type_at_position_or_any_completion(signature, 0, consumer)
        }
    }

    /// `isThenableType`: base-primitive exclusion precedes the actual then
    /// property and its non-nullable call-signature inventory.
    fn is_thenable_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let base = sequence
                .read(self.base_constraint_completion(ty, consumer))?
                .unwrap_or(ty);
            if sequence.read(self.all_types_assignable_to_kind_completion(
                base,
                TypeFlags::PRIMITIVE | TypeFlags::NEVER,
                false,
                consumer,
            ))? {
                return Ok(false);
            }
            let Some(then_type) =
                sequence.read(self.type_of_property_of_type_completion(ty, "then", consumer))?
            else {
                return Ok(false);
            };
            let then_type = sequence.read(self.type_with_facts_completion(
                then_type,
                TypeFacts::NOT_NULLABLE,
                consumer,
            ))?;
            Ok(!sequence
                .read(self.signatures_of_type_completion(then_type, false, consumer))?
                .is_empty())
        })();
        sequence.finish(result)
    }

    /// `getAwaitedTypeOfPromise`: only a nonnil promised value is recursively
    /// awaited; native recovery accompanying that value still continues.
    pub(super) fn awaited_type_of_promise_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.promised_type_completion(ty, consumer)?
            .and_then(|promised| match promised.value {
                Some(promised) => self.awaited_type_completion(promised, consumer),
                None => Ok(NativeCompletion::clean(None)),
            })
    }

    pub(super) fn awaited_type_no_alias_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.awaited_type_no_alias_ex_completion(ty, AwaitDiagnostics::Quiet, consumer)
    }

    /// `getAwaitedTypeNoAliasEx`: the Any/alias prefix is outside cached worker
    /// evidence. Actual nil remains retryable; recursion cleanup precedes Err.
    /// `getAwaitedTypeNoAliasEx`: check the entire invocation, including any,
    /// existing Awaited and cache-hit paths, before recursive native traversal.
    fn awaited_type_no_alias_ex_completion(
        &mut self,
        ty: TypeId,
        diagnostics: AwaitDiagnostics,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let _depth = {
            self.control
                .as_ref()
                .map(|control| complete_or_abort(control.enter()))
        };
        if self.model.has_type_flags(ty, TypeFlags::ANY) {
            return Ok(NativeCompletion::clean(Some(ty)));
        }
        self.is_awaited_type_instantiation_completion(ty, consumer)?
            .and_then(|instantiated| {
                if instantiated {
                    return Ok(NativeCompletion::clean(Some(ty)));
                }
                if let Some(cached) = self.promises.awaited.get(&ty) {
                    return Ok(cached.clone().map(Some));
                }
                let result = self.awaited_type_worker_completion(ty, diagnostics, consumer);
                if let Ok(completed) = &result
                    && let Some(value) = *completed.value()
                {
                    self.promises
                        .awaited
                        .insert(ty, completed.clone().map(|_| value));
                }
                result
            })
    }

    fn awaited_type_worker_completion(
        &mut self,
        ty: TypeId,
        diagnostics: AwaitDiagnostics,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        if self.model.has_type_flags(ty, TypeFlags::UNION) {
            let recursive = { self.promises.stack.contains(&ty) };
            if recursive {
                return Ok(NativeCompletion::clean(None));
            }
            {
                self.promises.stack.push(ty);
            }
            let result =
                self.map_type_optional_completion(ty, false, consumer, |checker, child| {
                    checker.awaited_type_no_alias_ex_completion(child, diagnostics, consumer)
                });
            {
                let _ = self.promises.stack.pop();
            }
            return result;
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if sequence.read(self.is_awaited_type_needed_completion(ty, consumer))? {
                return Ok(Some(ty));
            }
            let promised = sequence.read(self.promised_type_completion(ty, consumer))?;
            if let Some(value) = promised.value {
                let recursive = value == ty || { self.promises.stack.contains(&value) };
                if recursive {
                    return Ok(None);
                }
                {
                    self.promises.stack.push(ty);
                }
                let result = self.awaited_type_no_alias_ex_completion(value, diagnostics, consumer);
                {
                    let _ = self.promises.stack.pop();
                }
                return sequence.read(result);
            }
            if sequence.read(self.is_thenable_type_completion(ty, consumer))? {
                if !matches!(diagnostics, AwaitDiagnostics::Quiet)
                    && let Some(rejected_this) = promised.rejected_this
                {
                    sequence.read(self.diagnostic_type_display_completion(
                        ty,
                        None,
                        DiagnosticTypeFormat::DEFAULT,
                        consumer,
                    ))?;
                    sequence.read(self.diagnostic_type_display_completion(
                        rejected_this,
                        None,
                        DiagnosticTypeFormat::DEFAULT,
                        consumer,
                    ))?;
                }
                if matches!(diagnostics, AwaitDiagnostics::CheckedWithoutMessage) {
                    return Err(super::super::unsupported_demand_at_location(
                        self.inputs,
                        consumer,
                        crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
                        "pinned native async-from-sync iteration cannot report an invalid thenable without a diagnostic message",
                    ));
                }
                return Ok(None);
            }
            Ok(Some(ty))
        })();
        sequence.finish(result)
    }

    /// `isAwaitedTypeInstantiation` recognizes the actual global alias and
    /// its one argument. Matching a printed name is insufficient.
    fn is_awaited_type_instantiation_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if !self.model.has_type_flags(ty, TypeFlags::CONDITIONAL) {
            return Ok(NativeCompletion::clean(false));
        }
        self.global_type_alias_completion("Awaited", 1, consumer)?
            .and_then_value(|symbol| {
                let alias = self.model.metadata(ty)?.alias;
                let symbol = symbol
                    .map(|symbol| self.reference_alias_symbol(symbol, consumer))
                    .transpose()?;
                Ok(symbol.is_some_and(|symbol| {
                    alias.is_some_and(|alias| {
                        alias.symbol == symbol
                            && alias
                                .arguments
                                .as_ref()
                                .is_some_and(|arguments| arguments.len() == 1)
                    })
                }))
            })
    }

    /// `isAwaitedTypeNeeded` preserves the ordered generic/base-constraint and
    /// someType predicates; its nested thenable reads share the adoption owner.
    fn is_awaited_type_needed_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.has_type_flags(ty, TypeFlags::ANY)
                || sequence.read(self.is_awaited_type_instantiation_completion(ty, consumer))?
            {
                return Ok(false);
            }
            if !sequence.read(self.is_generic_object_type_completion(ty, consumer))? {
                return Ok(false);
            }
            let Some(constraint) = sequence.read(self.base_constraint_completion(ty, consumer))?
            else {
                return Ok(self.model.maybe_type_of_kind(ty, TypeFlags::TYPE_VARIABLE));
            };
            if self
                .model
                .has_type_flags(constraint, TypeFlags::ANY | TypeFlags::UNKNOWN)
                || sequence.read(self.is_empty_object_type_completion(constraint, consumer))?
            {
                return Ok(true);
            }
            // someType borrows the installed native union order and stops on
            // its first match. Neither a union nor a scalar needs new backing.
            let shared = self.model.union_constituents(constraint);
            let alternatives = match &shared {
                Some(types) => types.as_ref(),
                None => std::slice::from_ref(&constraint),
            };
            for &alternative in alternatives {
                if sequence.read(self.is_thenable_type_completion(alternative, consumer))? {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    pub(super) fn awaited_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.awaited_type_ex_completion(ty, AwaitDiagnostics::Quiet, consumer)
    }

    /// `getAwaitedTypeEx` / `createAwaitedTypeIfNeeded`: acquire the global
    /// alias, then unwrap its argument before instantiation. An absent global
    /// Awaited leaves the actual type unchanged.
    fn awaited_type_ex_completion(
        &mut self,
        ty: TypeId,
        diagnostics: AwaitDiagnostics,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(awaited) = sequence.read(self.awaited_type_no_alias_ex_completion(
                ty,
                diagnostics,
                consumer,
            ))?
            else {
                return Ok(None);
            };
            if sequence.read(self.is_awaited_type_needed_completion(awaited, consumer))?
                && let Some(symbol) = sequence
                    .read(self.global_type_alias_checked_completion("Awaited", 1, consumer))?
            {
                let unwrapped =
                    sequence.read(self.unwrap_awaited_type_completion(awaited, consumer))?;
                let declared =
                    sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
                return sequence
                    .read(self.type_alias_instantiation_completion(
                        symbol,
                        declared,
                        vec![unwrapped],
                        None,
                        consumer,
                    ))
                    .map(Some);
            }
            Ok(Some(awaited))
        })();
        sequence.finish(result)
    }

    /// `unwrapAwaitedType` uses mapType for unions and unwraps only an actual
    /// Awaited alias instantiation. The shared mapper retains all child reads.
    pub(super) fn unwrap_awaited_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self.model.has_type_flags(ty, TypeFlags::UNION) {
            return self.map_type_completion(ty, consumer, |checker, child| {
                checker.unwrap_awaited_type_completion(child, consumer)
            });
        }
        self.is_awaited_type_instantiation_completion(ty, consumer)?
            .and_then_value(|instantiated| {
                if instantiated {
                    Ok(self
                        .model
                        .metadata(ty)?
                        .alias
                        .expect("established Awaited alias")
                        .arguments
                        .as_ref()
                        .expect("established Awaited arguments")[0])
                } else {
                    Ok(ty)
                }
            })
    }
}
