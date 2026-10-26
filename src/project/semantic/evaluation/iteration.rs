//! The native iterable protocol owns its cache and all three iteration types.
//! Binding, spread and for-of provide their actual error-node policy; none can
//! replace this operation with array syntax or a numeric-index approximation.

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, SpecialName, TypeFlags, TypeId,
    UnionReduction,
};
use super::super::source::{ExpressionNode, ExpressionRoot};
use super::native_display::DiagnosticTypeFormat;
use super::relations::{RelationKind, RelationReporting};
use super::{Evaluator, facts::TypeFacts, globals::GlobalTypeLookup};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::{NativePropertyKey, flow::Site};
use crate::project::sources::FileId;
use crate::references::Location;
use crate::types::PrimitiveName;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) enum IterationUse {
    Spread,
    Element,
    Destructuring,
    Binding,
    BindingOutOfBounds,
    ForOf,
    ForAwaitOf,
    ExplicitForOf,
    ExplicitForAwaitOf,
}
impl IterationUse {
    fn allows_strings(self) -> bool {
        self.for_of()
    }
    fn allows_async(self) -> bool {
        matches!(self, Self::ForAwaitOf | Self::ExplicitForAwaitOf)
    }
    fn for_of(self) -> bool {
        matches!(
            self,
            Self::ForOf | Self::ForAwaitOf | Self::ExplicitForOf | Self::ExplicitForAwaitOf
        )
    }
    fn out_of_bounds(self) -> bool {
        matches!(self, Self::BindingOutOfBounds)
    }
    fn checks_next(self) -> bool {
        !matches!(self, Self::Element)
    }
    fn cache_flags(self) -> u8 {
        1 | if self.allows_async() { 2 } else { 0 } | if self.for_of() { 8 } else { 0 }
    }
}

/// `IterationTypes` retains nil independently for yield, return and next.
#[derive(Clone, Copy, Default)]
struct IterationTypes {
    yield_type: Option<TypeId>,
    return_type: Option<TypeId>,
    next_type: Option<TypeId>,
}
impl IterationTypes {
    fn any(ty: TypeId) -> Self {
        Self {
            yield_type: Some(ty),
            return_type: Some(ty),
            next_type: Some(ty),
        }
    }
    fn has_types(self) -> bool {
        self.yield_type.is_some() || self.return_type.is_some() || self.next_type.is_some()
    }
}

#[derive(Default)]
pub(super) struct IterationState {
    cache: HashMap<(TypeId, u8), NativeCompletion<IterationTypes>>,
}

impl Evaluator<'_, '_> {
    pub(super) fn binding_iteration_type_completion(
        &mut self,
        ty: TypeId,
        out_of_bounds: bool,
        node: ExpressionNode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.checked_iteration_type_completion(
            ty,
            if out_of_bounds {
                IterationUse::BindingOutOfBounds
            } else {
                IterationUse::Binding
            },
            Some(node),
            consumer,
        )
    }

    /// `isArrayLikeType`: Array identity precedes nullable exclusion and the
    /// full relation to the real ReadonlyArray<any> target.
    pub(super) fn is_array_like_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_array_like_type_completion(ty, consumer)?.admit()
    }
    pub(super) fn is_array_like_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.is_array_type_completion(ty)?.and_then(|array| {
            if array {
                return Ok(NativeCompletion::clean(true));
            }
            if self.model.has_type_flags(ty, TypeFlags::NULLABLE) {
                return Ok(NativeCompletion::clean(false));
            }
            let target = self.installed_any_readonly_array_type()?;
            self.type_related_completion(ty, target, RelationKind::Assignable, consumer)
        })
    }

    /// `checkIteratedTypeOrElementType`: genuine Any bypasses the protocol.
    /// A completed nil result supplies native Any and retains that recovery;
    /// an interrupted protocol never supplies a value.
    pub(super) fn checked_iteration_type_completion(
        &mut self,
        ty: TypeId,
        use_: IterationUse,
        error: Option<ExpressionNode>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self.model.has_type_flags(ty, TypeFlags::ANY) {
            return Ok(NativeCompletion::clean(ty));
        }
        self.iterated_type_or_element_type_completion(ty, use_, error, true, consumer)?
            .and_then(|value| match value {
                Some(value) => Ok(NativeCompletion::clean(value)),
                None => NativeCompletion::recovered(
                    self.model.special(SpecialName::Any),
                    self.iteration_recovery(
                        consumer,
                        "native iteration checking consumed anyType after no iterable element type",
                    ),
                ),
            })
    }

    /// `getIteratedTypeOrElementType` owns the protocol/legacy-array choice,
    /// next-input check, string constituents and out-of-bounds inclusion.
    pub(super) fn iterated_type_or_element_type_completion(
        &mut self,
        input: TypeId,
        use_: IterationUse,
        error: Option<ExpressionNode>,
        check_assignability: bool,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if input == self.model.special(SpecialName::Never) {
                if let Some(error) = error {
                    sequence.read(self.report_type_not_iterable_completion(
                        input,
                        use_.allows_async(),
                        error,
                        consumer,
                    ))?;
                }
                return Ok(None);
            }
            let exists = matches!(
                sequence.read(self.global_type_lookup_completion("Iterable", 3, consumer))?,
                GlobalTypeLookup::Target { .. }
            );
            let out =
                use_.out_of_bounds() && self.inputs.semantic_options().no_unchecked_indexed_access;
            if exists || use_.allows_async() {
                let types = sequence.read(self.iteration_types_of_iterable_completion(
                    input,
                    use_,
                    if exists { error } else { None },
                    consumer,
                ))?;
                if check_assignability
                    && use_.checks_next()
                    && let Some(next) = types.next_type
                {
                    let sent = self.model.special(SpecialName::Undefined);
                    let context =
                        error.and_then(|node| RelationReporting::Report.iteration_at(node, use_));
                    sequence.read(self.check_type_related_to_completion(
                        sent,
                        next,
                        RelationKind::Assignable,
                        context,
                        consumer,
                    ))?;
                }
                if types.yield_type.is_some() || exists {
                    return match types.yield_type {
                        Some(ty) if out => sequence
                            .read(
                                self.include_undefined_in_index_signature_completion(ty, consumer),
                            )
                            .map(Some),
                        value => Ok(value),
                    };
                }
            }
            let mut array = input;
            if use_.allows_strings() {
                if let Some(parts) = self.model.union_constituents(input) {
                    let retained: Vec<_> = parts
                        .iter()
                        .copied()
                        .filter(|&ty| !self.model.has_type_flags(ty, TypeFlags::STRING_LIKE))
                        .collect();
                    if retained.len() != parts.len() {
                        array = sequence.read(self.union_type_reduced_completion(
                            retained,
                            UnionReduction::Subtype,
                            None,
                            None,
                            consumer,
                        ))?;
                    }
                } else if self.model.has_type_flags(input, TypeFlags::STRING_LIKE) {
                    array = self.model.special(SpecialName::Never);
                }
            }
            let has_string = array != input;
            let string = self.model.primitive(PrimitiveName::String);
            if has_string && self.model.has_type_flags(array, TypeFlags::NEVER) {
                return if out {
                    sequence
                        .read(
                            self.include_undefined_in_index_signature_completion(string, consumer),
                        )
                        .map(Some)
                } else {
                    Ok(Some(string))
                };
            }
            if !sequence.read(self.is_array_like_type_completion(array, consumer))? {
                if error.is_some() {
                    // `getIterationDiagnosticDetails` first asks the full
                    // protocol quietly; names are examined only after nil.
                    let types = sequence.read(
                        self.iteration_types_of_iterable_completion(input, use_, None, consumer),
                    )?;
                    let maybe_await = if types.yield_type.is_some() {
                        false
                    } else {
                        if let Some(symbol) = self.reference_symbol_of_type(input, consumer)? {
                            let _ = self.reference_symbol_key(symbol, consumer)?;
                        }
                        true
                    };
                    if maybe_await {
                        sequence.read(self.awaited_type_of_promise_completion(array, consumer))?;
                    }
                    sequence.read(self.diagnostic_type_display_completion(
                        array,
                        None,
                        DiagnosticTypeFormat::DEFAULT,
                        consumer,
                    ))?;
                }
                return if has_string {
                    if out {
                        sequence
                            .read(
                                self.include_undefined_in_index_signature_completion(
                                    string, consumer,
                                ),
                            )
                            .map(Some)
                    } else {
                        Ok(Some(string))
                    }
                } else {
                    Ok(None)
                };
            }
            let number = self.model.primitive(PrimitiveName::Number);
            let element =
                sequence.read(self.index_type_of_type_completion(array, number, consumer))?;
            if has_string && let Some(element) = element {
                if self.model.has_type_flags(element, TypeFlags::STRING_LIKE)
                    && !self.inputs.semantic_options().no_unchecked_indexed_access
                {
                    return Ok(Some(string));
                }
                let mut parts = vec![element, string];
                if out {
                    parts.push(self.model.special(SpecialName::Undefined));
                }
                return sequence
                    .read(self.union_type_reduced_completion(
                        parts,
                        UnionReduction::Subtype,
                        None,
                        None,
                        consumer,
                    ))
                    .map(Some);
            }
            if use_.out_of_bounds()
                && let Some(element) = element
            {
                return sequence
                    .read(self.include_undefined_in_index_signature_completion(element, consumer))
                    .map(Some);
            }
            Ok(element)
        })();
        sequence.finish(result)
    }

    /// `getIterationTypesOfIterable`: reduction is invocation prefix, outside
    /// the cached worker. A reporting retry of cached nil cannot replace it.
    fn iteration_types_of_iterable_completion(
        &mut self,
        ty: TypeId,
        use_: IterationUse,
        error: Option<ExpressionNode>,
        consumer: &Location,
    ) -> NativeResult<IterationTypes> {
        self.reduced_type_completion(ty, consumer)?.and_then(|ty| {
            if self.model.has_type_flags(ty, TypeFlags::ANY) {
                return Ok(NativeCompletion::clean(IterationTypes::any(
                    self.model.special(SpecialName::Any),
                )));
            }
            let key = (ty, use_.cache_flags());
            let no_cache = if let Some(cached) = self.iteration.cache.get(&key) {
                if error.is_none() || cached.value().has_types() {
                    return Ok(cached.clone());
                }
                true
            } else {
                false
            };
            let result =
                self.iteration_types_of_iterable_worker_completion(ty, use_, error, consumer)?;
            if !no_cache {
                self.iteration.cache.insert(key, result.clone());
            }
            Ok(result)
        })
    }

    /// `getIterationTypesOfIterableWorker`: constituent checks use no error
    /// node. Missing-protocol diagnostics are native deferred callbacks, so
    /// this query does not acquire their later TypeToString dependencies.
    fn iteration_types_of_iterable_worker_completion(
        &mut self,
        ty: TypeId,
        use_: IterationUse,
        error: Option<ExpressionNode>,
        consumer: &Location,
    ) -> NativeResult<IterationTypes> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(parts) = self.model.union_constituents(ty) {
                let mut all = Vec::new();
                for &part in parts.iter() {
                    let types =
                        sequence.read(self.iteration_types_of_iterable_worker_completion(
                            part, use_, None, consumer,
                        ))?;
                    if !types.has_types() {
                        return Ok(IterationTypes::default());
                    }
                    all.push(types);
                }
                return sequence.read(self.combine_iteration_types_completion(all, consumer));
            }
            if use_.allows_async() {
                let fast = sequence
                    .read(self.iteration_types_fast_completion(ty, true, true, consumer))?;
                if fast.has_types() {
                    return if use_.for_of() {
                        sequence
                            .read(self.async_from_sync_iteration_completion(fast, error, consumer))
                    } else {
                        Ok(fast)
                    };
                }
                let slow = sequence.read(
                    self.iteration_types_of_iterable_slow_completion(ty, true, error, consumer),
                )?;
                if slow.has_types() {
                    return Ok(slow);
                }
            }
            let fast =
                sequence.read(self.iteration_types_fast_completion(ty, false, true, consumer))?;
            if fast.has_types() {
                return if use_.allows_async() {
                    sequence.read(self.async_from_sync_iteration_completion(fast, error, consumer))
                } else {
                    Ok(fast)
                };
            }
            let slow = sequence.read(
                self.iteration_types_of_iterable_slow_completion(ty, false, error, consumer),
            )?;
            if slow.has_types() {
                return if use_.allows_async() {
                    sequence.read(self.async_from_sync_iteration_completion(slow, error, consumer))
                } else {
                    Ok(slow)
                };
            }
            Ok(IterationTypes::default())
        })();
        sequence.finish(result)
    }

    /// Both native fast workers ask their four globals in short-circuit order;
    /// the builtin list is wholly resolved before its membership comparison.
    fn iteration_types_fast_completion(
        &mut self,
        ty: TypeId,
        asynchronous: bool,
        iterable: bool,
        consumer: &Location,
    ) -> NativeResult<IterationTypes> {
        let names = match (asynchronous, iterable) {
            (false, true) => [
                "Iterable",
                "IteratorObject",
                "IterableIterator",
                "Generator",
            ],
            (false, false) => [
                "Iterator",
                "IteratorObject",
                "IterableIterator",
                "Generator",
            ],
            (true, true) => [
                "AsyncIterable",
                "AsyncIteratorObject",
                "AsyncIterableIterator",
                "AsyncGenerator",
            ],
            (true, false) => [
                "AsyncIterator",
                "AsyncIteratorObject",
                "AsyncIterableIterator",
                "AsyncGenerator",
            ],
        };
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for name in names {
                let global =
                    sequence.read(self.global_type_lookup_completion(name, 3, consumer))?;
                if let GlobalTypeLookup::Target { ty: target, .. } = global {
                    if self.model.reference_target(ty) == Some(target) {
                        let args = sequence.read(self.get_type_arguments_completion(ty))?;
                        if args.len() < 3 {
                            return Err(super::super::internal(
                                "iteration reference lacks its three native arguments",
                            )
                            .into());
                        }
                        return sequence.read(self.resolved_iteration_types_completion(
                            args[0],
                            args[1],
                            args[2],
                            asynchronous,
                            consumer,
                        ));
                    }
                }
            }
            let targets = sequence
                .read(self.global_builtin_iteration_types_completion(asynchronous, consumer))?;
            if self
                .model
                .reference_target(ty)
                .is_some_and(|target| targets.contains(&target))
            {
                let args = sequence.read(self.get_type_arguments_completion(ty))?;
                let element = *args.first().ok_or_else(|| {
                    super::super::internal("builtin iterator lacks its native argument")
                })?;
                let returned = self.model.special(
                    if self
                        .inputs
                        .semantic_options()
                        .strict_builtin_iterator_return
                    {
                        SpecialName::Undefined
                    } else {
                        SpecialName::Any
                    },
                );
                let next = self.model.special(SpecialName::Unknown);
                return sequence.read(self.resolved_iteration_types_completion(
                    element,
                    returned,
                    next,
                    asynchronous,
                    consumer,
                ));
            }
            Ok(IterationTypes::default())
        })();
        sequence.finish(result)
    }

    fn resolved_iteration_types_completion(
        &mut self,
        yield_type: TypeId,
        return_type: TypeId,
        next_type: TypeId,
        asynchronous: bool,
        consumer: &Location,
    ) -> NativeResult<IterationTypes> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let yield_type = if asynchronous {
                sequence
                    .read(self.awaited_type_for_iteration_completion(yield_type, false, consumer))?
                    .unwrap_or(yield_type)
            } else {
                yield_type
            };
            let return_type = if asynchronous {
                sequence
                    .read(self.awaited_type_for_iteration_completion(return_type, false, consumer))?
                    .unwrap_or(return_type)
            } else {
                return_type
            };
            Ok(IterationTypes {
                yield_type: Some(yield_type),
                return_type: Some(return_type),
                next_type: Some(next_type),
            })
        })();
        sequence.finish(result)
    }

    fn combine_iteration_types_completion(
        &mut self,
        all: Vec<IterationTypes>,
        consumer: &Location,
    ) -> NativeResult<IterationTypes> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut combined = [None; 3];
            for (kind, slot) in combined.iter_mut().enumerate() {
                let types: Vec<_> = all
                    .iter()
                    .filter_map(|types| match kind {
                        0 => types.yield_type,
                        1 => types.return_type,
                        _ => types.next_type,
                    })
                    .collect();
                if !types.is_empty() {
                    *slot = Some(
                        sequence
                            .read(self.union_type_ex_completion(types, None, None, consumer))?,
                    );
                }
            }
            Ok(IterationTypes {
                yield_type: combined[0],
                return_type: combined[1],
                next_type: combined[2],
            })
        })();
        sequence.finish(result)
    }

    /// Slow iterator adoption consumes the native `awaited ?? anyType`
    /// fallback. Fast generic projection instead retains its original type.
    fn iteration_await_or_any_completion(
        &mut self,
        ty: TypeId,
        report_errors: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let awaited = self.awaited_type_for_iteration_completion(ty, report_errors, consumer)?;
        self.iteration_awaited_or_any_completion(awaited, consumer)
    }

    fn iteration_awaited_or_any_completion(
        &mut self,
        awaited: NativeCompletion<Option<TypeId>>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        awaited.and_then(|awaited| match awaited {
            Some(ty) => Ok(NativeCompletion::clean(ty)),
            None => NativeCompletion::recovered(
                self.model.special(SpecialName::Any),
                self.iteration_recovery(
                    consumer,
                    "native async iteration consumed anyType after rejected awaited type",
                ),
            ),
        })
    }

    fn async_from_sync_iteration_completion(
        &mut self,
        types: IterationTypes,
        error: Option<ExpressionNode>,
        consumer: &Location,
    ) -> NativeResult<IterationTypes> {
        let any = self.model.special(SpecialName::Any);
        if !types.has_types()
            || (types.yield_type == Some(any)
                && types.return_type == Some(any)
                && types.next_type == Some(any))
        {
            return Ok(NativeCompletion::clean(types));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if error.is_some() {
                sequence.read(self.global_type_alias_checked_completion("Awaited", 1, consumer))?;
            }
            // Pinned getAwaitedTypeEx dereferences a nil input. This partial
            // triple is a native compiler invariant failure, not an Any value.
            let missing = || {
                super::super::unsupported_demand_at_location(
                    self.inputs,
                    consumer,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "pinned native async iteration cannot await an absent yield or return type",
                )
            };
            let yield_type = types.yield_type.ok_or_else(missing)?;
            let awaited = self.awaited_type_from_sync_iteration_completion(
                yield_type,
                error.is_some(),
                consumer,
            )?;
            let yield_type =
                sequence.read(self.iteration_awaited_or_any_completion(awaited, consumer))?;
            let return_type = types.return_type.ok_or_else(|| {
                super::super::unsupported_demand_at_location(
                    self.inputs,
                    consumer,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "pinned native async iteration cannot await an absent yield or return type",
                )
            })?;
            let awaited = self.awaited_type_from_sync_iteration_completion(
                return_type,
                error.is_some(),
                consumer,
            )?;
            let return_type =
                sequence.read(self.iteration_awaited_or_any_completion(awaited, consumer))?;
            Ok(IterationTypes {
                yield_type: Some(yield_type),
                return_type: Some(return_type),
                next_type: types.next_type,
            })
        })();
        sequence.finish(result)
    }

    fn iteration_types_of_iterable_slow_completion(
        &mut self,
        ty: TypeId,
        asynchronous: bool,
        error: Option<ExpressionNode>,
        consumer: &Location,
    ) -> NativeResult<IterationTypes> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let name = sequence.read(self.property_key_for_known_symbol_completion(
                if asynchronous {
                    "asyncIterator"
                } else {
                    "iterator"
                },
                consumer,
            ))?;
            if let Some(method) =
                sequence.read(self.property_of_type_by_key_completion(ty, &name, consumer))?
            {
                let symbol = self.property_symbol(&method, consumer)?;
                if !self.property_symbol_flags(symbol)?.optional {
                    let method_type =
                        sequence.read(self.property_raw_type_completion(symbol, consumer))?;
                    if self.model.has_type_flags(method_type, TypeFlags::ANY) {
                        return Ok(IterationTypes::any(self.model.special(SpecialName::Any)));
                    }
                    let all = sequence.read(self.signatures_of_type_completion(
                        method_type,
                        false,
                        consumer,
                    ))?;
                    let mut valid = Vec::new();
                    for signature in all.iter() {
                        if sequence
                            .read(self.minimum_argument_count_completion(signature, consumer))?
                            == 0
                        {
                            valid.push(signature.clone());
                        }
                    }
                    if !valid.is_empty() {
                        let mut returns = Vec::new();
                        for signature in &valid {
                            returns.push(sequence.read(
                                self.get_return_type_of_signature_completion(signature, consumer),
                            )?);
                        }
                        let iterator =
                            sequence.read(self.intersection_type_completion(returns, consumer))?;
                        return sequence.read(self.iteration_types_of_iterator_completion(
                            iterator,
                            asynchronous,
                            error,
                            consumer,
                        ));
                    }
                    if let Some(error) = error
                        && !all.is_empty()
                    {
                        let global = sequence.read(
                            self.global_iterable_type_checked_completion(asynchronous, consumer),
                        )?;
                        let target = match global {
                            GlobalTypeLookup::Target { ty, .. } => ty,
                            GlobalTypeLookup::Recovery(_) => self.model.empty_generic_type(),
                        };
                        sequence.read(self.check_type_related_to_completion(
                            ty,
                            target,
                            RelationKind::Assignable,
                            RelationReporting::Report.assignment_at(error),
                            consumer,
                        ))?;
                    }
                }
            }
            Ok(IterationTypes::default())
        })();
        sequence.finish(result)
    }

    fn iteration_types_of_iterator_completion(
        &mut self,
        ty: TypeId,
        asynchronous: bool,
        error: Option<ExpressionNode>,
        consumer: &Location,
    ) -> NativeResult<IterationTypes> {
        if self.model.has_type_flags(ty, TypeFlags::ANY) {
            return Ok(NativeCompletion::clean(IterationTypes::any(
                self.model.special(SpecialName::Any),
            )));
        }
        self.iteration_types_fast_completion(ty, asynchronous, false, consumer)?
            .and_then(|fast| {
                if fast.has_types() {
                    return Ok(NativeCompletion::clean(fast));
                }
                let mut sequence = NativeSequence::new();
                let result = (|| {
                    let mut types = Vec::new();
                    for name in ["next", "return", "throw"] {
                        types.push(sequence.read(self.iteration_types_of_method_completion(
                            ty,
                            asynchronous,
                            name,
                            error,
                            consumer,
                        ))?);
                    }
                    sequence.read(self.combine_iteration_types_completion(types, consumer))
                })();
                sequence.finish(result)
            })
    }

    fn iteration_types_of_method_completion(
        &mut self,
        ty: TypeId,
        asynchronous: bool,
        name: &str,
        error: Option<ExpressionNode>,
        consumer: &Location,
    ) -> NativeResult<IterationTypes> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let method = sequence.read(self.property_of_type_completion(ty, name, consumer))?;
            if method.is_none() && name != "next" {
                return Ok(IterationTypes::default());
            }
            let mut method_type = None;
            if let Some(method) = method {
                let symbol = self.property_symbol(&method, consumer)?;
                if name != "next" || !self.property_symbol_flags(symbol)?.optional {
                    let raw = sequence.read(self.property_raw_type_completion(symbol, consumer))?;
                    method_type = Some(if name == "next" {
                        raw
                    } else {
                        sequence.read(self.type_with_facts_completion(
                            raw,
                            TypeFacts::NOT_NULLABLE,
                            consumer,
                        ))?
                    });
                }
            }
            if method_type.is_some_and(|ty| self.model.has_type_flags(ty, TypeFlags::ANY)) {
                return Ok(IterationTypes::any(self.model.special(SpecialName::Any)));
            }
            let signatures = match method_type {
                Some(ty) => {
                    sequence.read(self.signatures_of_type_completion(ty, false, consumer))?
                }
                None => std::sync::Arc::from([]),
            };
            // Missing-method diagnostics have only the literal method name;
            // recording them adds no semantic acquisition.
            if signatures.is_empty() {
                return Ok(IterationTypes::default());
            }
            let method_type = method_type.expect("nonnull signatures imply method type");
            if signatures.len() == 1
                && let Some(symbol) = self.reference_symbol_of_type(method_type, consumer)?
            {
                let generator = sequence.read(self.global_type_lookup_completion(
                    if asynchronous {
                        "AsyncGenerator"
                    } else {
                        "Generator"
                    },
                    3,
                    consumer,
                ))?;
                let iterator = sequence.read(self.global_type_lookup_completion(
                    if asynchronous {
                        "AsyncIterator"
                    } else {
                        "Iterator"
                    },
                    3,
                    consumer,
                ))?;
                let mut selected = None;
                for global in [generator, iterator] {
                    if let GlobalTypeLookup::Target { ty, .. } = global {
                        if let Some(owner) = self.reference_symbol_of_type(ty, consumer)? {
                            if let Some(raw) = self.reference_raw_symbol_table(owner, false)? {
                                let members = self.reference_raw_table_entries(raw, consumer)?;
                                if let Some((_, candidate)) = members.iter().find(|(key, _)| {
                                    key == &NativePropertyKey::fixed(name.as_bytes())
                                }) {
                                    if self.reference_symbol_identity(*candidate, consumer)?
                                        == self.reference_symbol_identity(symbol, consumer)?
                                    {
                                        selected = Some(ty);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(target) = selected {
                    let parameters = self.model.declared_target(target)?.all_parameters;
                    if parameters.len() < 3 {
                        return Err(super::super::internal(
                            "iterator method target lacks parameters",
                        )
                        .into());
                    }
                    let mapper = self.model.object_mapper(method_type).ok_or_else(|| {
                        super::super::internal("instantiated iterator method lacks mapper")
                    })?;
                    let next = if name == "next" {
                        Some(sequence.read(self.map_parameter_completion(
                            parameters[2],
                            mapper,
                            consumer,
                        ))?)
                    } else {
                        None
                    };
                    let yield_type = sequence.read(self.map_parameter_completion(
                        parameters[0],
                        mapper,
                        consumer,
                    ))?;
                    let return_type = sequence.read(self.map_parameter_completion(
                        parameters[1],
                        mapper,
                        consumer,
                    ))?;
                    return Ok(IterationTypes {
                        yield_type: Some(yield_type),
                        return_type: Some(return_type),
                        next_type: next,
                    });
                }
            }
            let mut parameters = Vec::new();
            let mut returns = Vec::new();
            for signature in signatures.iter() {
                if name != "throw" && !signature.parameters.is_empty() {
                    parameters.push(sequence.read(
                        self.signature_type_at_position_or_any_completion(signature, 0, consumer),
                    )?);
                }
                returns.push(
                    sequence
                        .read(self.get_return_type_of_signature_completion(signature, consumer))?,
                );
            }
            let mut returned = Vec::new();
            let mut next = None;
            if name != "throw" {
                let parameter = if parameters.is_empty() {
                    self.model.special(SpecialName::Unknown)
                } else {
                    sequence
                        .read(self.union_type_ex_completion(parameters, None, None, consumer))?
                };
                if name == "next" {
                    next = Some(parameter);
                } else {
                    let resolved = if asynchronous {
                        sequence.read(self.iteration_await_or_any_completion(
                            parameter,
                            error.is_some(),
                            consumer,
                        ))?
                    } else {
                        parameter
                    };
                    returned.push(Some(resolved));
                }
            }
            let returned_type = if returns.is_empty() {
                self.model.special(SpecialName::Never)
            } else {
                sequence.read(self.intersection_type_completion(returns, consumer))?
            };
            let returned_type = if asynchronous {
                sequence.read(self.iteration_await_or_any_completion(
                    returned_type,
                    error.is_some(),
                    consumer,
                ))?
            } else {
                returned_type
            };
            let iteration = sequence
                .read(self.iteration_types_of_result_completion(returned_type, consumer))?;
            let yield_type;
            if !iteration.has_types() {
                let any = self.model.special(SpecialName::Any);
                sequence.read(NativeCompletion::recovered((), self.iteration_recovery(consumer, "native iterator method result consumed anyType after missing value property")))?;
                yield_type = Some(any);
                returned.push(Some(any));
            } else {
                yield_type = iteration.yield_type;
                returned.push(iteration.return_type);
            }
            // getUnionTypeEx returns its sole input before inspecting flags,
            // including the real nil from IteratorYieldResult's fast path.
            let return_type = match returned.as_slice() {
                [] => Some(self.model.special(SpecialName::Never)),
                [only] => *only,
                _ => {
                    let returned = returned
                        .into_iter()
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| {
                            super::super::internal(
                                "native iterator multi-type union contains a nil operand",
                            )
                        })?;
                    Some(
                        sequence
                            .read(self.union_type_ex_completion(returned, None, None, consumer))?,
                    )
                }
            };
            Ok(IterationTypes {
                yield_type,
                return_type,
                next_type: next,
            })
        })();
        sequence.finish(result)
    }

    /// `getIterationTypesOfIteratorResult`: each done-filter is independent;
    /// the second traversal follows the first value read, including recovery.
    fn iteration_types_of_result_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<IterationTypes> {
        if self.model.has_type_flags(ty, TypeFlags::ANY) {
            return Ok(NativeCompletion::clean(IterationTypes::any(
                self.model.special(SpecialName::Any),
            )));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for (name, yielded) in [
                ("IteratorYieldResult", true),
                ("IteratorReturnResult", false),
            ] {
                if let GlobalTypeLookup::Target { ty: target, .. } =
                    sequence.read(self.global_type_lookup_completion(name, 1, consumer))?
                {
                    if self.model.reference_target(ty) == Some(target) {
                        let args = sequence.read(self.get_type_arguments_completion(ty))?;
                        let value = *args.first().ok_or_else(|| {
                            super::super::internal("iterator result lacks argument")
                        })?;
                        return Ok(IterationTypes {
                            yield_type: yielded.then_some(value),
                            return_type: (!yielded).then_some(value),
                            next_type: None,
                        });
                    }
                }
            }
            let mut values = [None; 2];
            for (index, truth) in [false, true].into_iter().enumerate() {
                let filtered =
                    sequence.read(self.filter_type_completion(ty, |checker, part| {
                        checker
                            .type_of_property_of_type_completion(part, "done", consumer)?
                            .and_then(|done| {
                                let done = done.unwrap_or_else(|| {
                                    checker
                                        .model
                                        .regular_literal_type(PrimitiveName::Boolean, "false")
                                });
                                let desired = checker.model.regular_literal_type(
                                    PrimitiveName::Boolean,
                                    if truth { "true" } else { "false" },
                                );
                                checker.type_related_completion(
                                    desired,
                                    done,
                                    RelationKind::Assignable,
                                    consumer,
                                )
                            })
                    }))?;
                if filtered != self.model.special(SpecialName::Never) {
                    values[index] = sequence.read(
                        self.type_of_property_of_type_completion(filtered, "value", consumer),
                    )?;
                }
            }
            if values[0].is_none() && values[1].is_none() {
                return Ok(IterationTypes::default());
            }
            Ok(IterationTypes {
                yield_type: values[0],
                return_type: Some(values[1].unwrap_or(self.model.special(SpecialName::Void))),
                next_type: None,
            })
        })();
        sequence.finish(result)
    }

    fn report_type_not_iterable_completion(
        &mut self,
        ty: TypeId,
        asynchronous: bool,
        error: ExpressionNode,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let awaited = sequence.read(self.awaited_type_of_promise_completion(ty, consumer))?;
            if awaited.is_none()
                && !asynchronous
                && self.source.iteration_error_is_for_of_expression(error)?
            {
                if let GlobalTypeLookup::Target { ty: target, .. } = sequence
                    .read(self.global_type_lookup_completion("AsyncIterable", 3, consumer))?
                {
                    let any = self.model.special(SpecialName::Any);
                    let reference = self.model.reference(target, vec![any, any, any])?;
                    sequence.read(self.type_related_completion(
                        ty,
                        reference,
                        RelationKind::Assignable,
                        consumer,
                    ))?;
                }
            }
            sequence.read(self.diagnostic_type_display_completion(
                ty,
                None,
                DiagnosticTypeFormat::DEFAULT,
                consumer,
            ))?;
            Ok(())
        })();
        sequence.finish(result)
    }

    fn iteration_recovery(&self, consumer: &Location, message: &str) -> DemandError {
        super::super::invalid_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::ErrorType,
            message,
        )
        .into()
    }

    /// `checkRightHandSideOfForOf` checks and non-nullifies the actual RHS
    /// before applying the synchronous or asynchronous protocol.
    pub(super) fn assignment_for_of_type(
        &mut self,
        file: FileId,
        expression: Site,
        asynchronous: bool,
    ) -> Result<TypeId, DemandError> {
        self.assignment_for_of_type_completion(file, expression, asynchronous)?
            .admit()
    }

    pub(super) fn assignment_for_of_type_completion(
        &mut self,
        file: FileId,
        expression: Site,
        asynchronous: bool,
    ) -> NativeResult<TypeId> {
        let source = self.source.expression_at(file, expression)?;
        let consumer = &source.location;
        self.check_expression_completion(&source, super::contextual::CheckMode::NORMAL)?
            .and_then(|ty| {
                self.check_non_null_type_completion(ty, consumer)?
                    .and_then(|ty| {
                        self.checked_iteration_type_completion(
                            ty,
                            if asynchronous {
                                IterationUse::ForAwaitOf
                            } else {
                                IterationUse::ForOf
                            },
                            Some(
                                ExpressionRoot {
                                    file,
                                    site: expression,
                                }
                                .into(),
                            ),
                            consumer,
                        )
                    })
            })
    }
}
