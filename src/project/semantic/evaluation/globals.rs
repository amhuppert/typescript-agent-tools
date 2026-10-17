//! Native global factories consume canonical binding facts. Recovery retains
//! its precise cause; a fallback object is never an ordinary semantic answer.

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, TypeId, TypeView,
};
use super::{Evaluator, ReferenceSymbol};
use crate::analysis::{AnalysisError, SemanticCause, SemanticFailureReason};
use crate::project::binding::NativeLookupMeaning;
use crate::references::Location;

#[derive(Default)]
pub(super) struct GlobalsState {
    value_symbols:
        std::collections::HashMap<String, NativeCompletion<Option<super::ReferenceSymbol>>>,
    type_symbols: std::collections::HashMap<String, NativeCompletion<Option<ReferenceSymbol>>>,
    types: std::collections::HashMap<
        (String, usize, GlobalTypeGetter),
        NativeCompletion<GlobalTypeLookup>,
    >,
    aliases:
        std::collections::HashMap<(String, usize, bool), NativeCompletion<Option<ReferenceSymbol>>>,
    iteration_builtins: std::collections::HashMap<bool, NativeCompletion<Vec<TypeId>>>,
    fixed: std::collections::HashMap<&'static str, NativeCompletion<GlobalTypeLookup>>,
    readonly_array_name: Option<&'static str>,
    any_array: Option<TypeId>,
    any_readonly_array: Option<TypeId>,
}

/// `getGlobalPromiseType` and `getGlobalPromiseTypeChecked` are distinct
/// memoized closures. Their shared worker can observe different declared slots.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum GlobalTypeGetter {
    Ordinary,
    CheckedPromise,
    CheckedIteration,
    CheckedDisposable,
}

/// `initializeChecker`'s actual fixed field names. Lazy getGlobalType resolver
/// closures use separate per-name caches and never populate these fields.
fn fixed_global_arity(name: &str) -> Option<usize> {
    match name {
        "Array" | "ReadonlyArray" | "ThisType" => Some(1),
        "Object" | "Function" | "CallableFunction" | "NewableFunction" | "String" | "Number"
        | "Boolean" | "RegExp" => Some(0),
        _ => None,
    }
}

/// One actual `getGlobalType` result, distinct from consuming its recovery.
#[derive(Clone)]
pub(super) enum GlobalTypeLookup {
    Target { symbol: ReferenceSymbol, ty: TypeId },
    Recovery(GlobalTypeRecovery),
}

#[derive(Clone)]
pub(super) enum GlobalTypeRecovery {
    Missing,
    WrongKind { declaration: Option<Location> },
    WrongArity { declaration: Option<Location> },
}

/// Fixed checker-initialization slots, unlike the lazy Promise/Iterable
/// resolver closures. Identity comparisons observe the installed slot.
#[derive(Clone, Copy)]
pub(super) enum FixedGlobalType {
    Object,
    Function,
    Array,
    ReadonlyArray,
    ThisType,
}

/// The node-builder reference shape; native readonly identity is a separate
/// predicate because the missing-ReadonlyArray fallback can alias Array.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::project::semantic) enum ReferenceDisplay {
    Ordinary,
    Tuple,
    Array { readonly: bool },
}

impl Evaluator<'_, '_> {
    /// `initializeChecker` establishes these targets before semantic
    /// queries. Each actual phase retains the complete getter result, including
    /// recovery. Identity observation does not consume the getter again; later
    /// factories consume the stored completion at their actual source location.
    pub(super) fn fixed_global_type_identity(
        &mut self,
        global: FixedGlobalType,
        _consumer: &Location,
    ) -> Result<Option<(super::ReferenceSymbol, TypeId)>, DemandError> {
        Ok(self.fixed_global_type_identity_value(global))
    }

    /// `initializeChecker` publishes the native field identity. A read sees
    /// actual nil before its phase and never acquires declarations or a source
    /// association solely for comparing an installed pointer.
    fn fixed_global_type_identity_value(
        &self,
        global: FixedGlobalType,
    ) -> Option<(ReferenceSymbol, TypeId)> {
        let name = match global {
            FixedGlobalType::Object => "Object",
            FixedGlobalType::Function => "Function",
            FixedGlobalType::Array => "Array",
            FixedGlobalType::ReadonlyArray => "ReadonlyArray",
            FixedGlobalType::ThisType => "ThisType",
        };
        self.globals
            .fixed
            .get(name)
            .and_then(|completed| match completed.value() {
                GlobalTypeLookup::Target { symbol, ty } => Some((*symbol, *ty)),
                GlobalTypeLookup::Recovery(_) => None,
            })
    }
    /// `getGlobalValueSymbolResolver` memoizes the raw value symbol, including
    /// real nil, independently of the global type/constructor type questions.
    pub(super) fn global_value_symbol(
        &mut self,
        name: &str,
        consumer: &Location,
    ) -> Result<Option<super::ReferenceSymbol>, DemandError> {
        self.global_value_symbol_completion(name, consumer)?.admit()
    }

    pub(super) fn global_value_symbol_completion(
        &mut self,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<Option<super::ReferenceSymbol>> {
        if let Some(result) = self.globals.value_symbols.get(name) {
            return Ok(result.clone());
        }
        let completed = self.global_symbol_completion(
            name,
            crate::project::binding::NativeLookupMeaning::VALUE,
            consumer,
        )?;
        self.globals
            .value_symbols
            .insert(name.to_owned(), completed.clone());
        Ok(completed)
    }

    /// `getGlobalSymbol` performs the current global-table lookup on every
    /// call. Lazy global resolver closures add their own cache above it.
    pub(super) fn global_symbol_completion(
        &mut self,
        name: &str,
        meaning: NativeLookupMeaning,
        consumer: &Location,
    ) -> NativeResult<Option<super::ReferenceSymbol>> {
        self.reference_global_name_completion(name, meaning, Some(consumer))
    }

    /// `getGlobalTypeSymbolResolver` memoizes the raw global Type-meaning
    /// symbol without declared-type acquisition or an arity question.
    pub(super) fn global_type_symbol_completion(
        &mut self,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        if let Some(result) = self.globals.type_symbols.get(name) {
            return Ok(result.clone());
        }
        let result = self.global_symbol_completion(name, NativeLookupMeaning::TYPE, consumer)?;
        self.globals
            .type_symbols
            .insert(name.to_owned(), result.clone());
        Ok(result)
    }
    /// `isArrayType`: inspect the reference bit before the two fixed global
    /// identities, without resolving any reference arguments or members.
    pub(super) fn is_array_type(&mut self, ty: TypeId) -> Result<bool, DemandError> {
        self.is_array_type_completion(ty)?.admit()
    }

    pub(super) fn is_array_type_completion(&mut self, ty: TypeId) -> NativeResult<bool> {
        let TypeView::Reference(reference) = self.model.get(ty)? else {
            return Ok(NativeCompletion::clean(false));
        };
        if !reference.is_reference {
            return Ok(NativeCompletion::clean(false));
        }
        Ok(NativeCompletion::clean(
            self.matches_array_target(reference.target, false)
                || self.matches_array_target(reference.target, true),
        ))
    }

    /// `isReadonlyArrayType` tests the readonly identity itself. A missing
    /// ReadonlyArray aliases Array, so this can be true for a mutable display.
    pub(super) fn is_readonly_array_type(&mut self, ty: TypeId) -> Result<bool, DemandError> {
        self.is_readonly_array_type_completion(ty)?.admit()
    }

    pub(super) fn is_readonly_array_type_completion(&mut self, ty: TypeId) -> NativeResult<bool> {
        let TypeView::Reference(reference) = self.model.get(ty)? else {
            return Ok(NativeCompletion::clean(false));
        };
        if !reference.is_reference {
            return Ok(NativeCompletion::clean(false));
        }
        Ok(NativeCompletion::clean(
            self.matches_array_target(reference.target, true),
        ))
    }

    fn matches_array_target(&self, target: TypeId, readonly: bool) -> bool {
        let global = if readonly {
            FixedGlobalType::ReadonlyArray
        } else {
            FixedGlobalType::Array
        };
        self.fixed_global_type_identity_value(global)
            .is_some_and(|(_, ty)| ty == target)
    }

    /// `typeReferenceToTypeNode` chooses mutable Array before ReadonlyArray,
    /// then tuple syntax. This display question does not classify a target
    /// when it is constructed, and never asks its deferred arguments.
    pub(in crate::project::semantic) fn reference_display(
        &mut self,
        ty: TypeId,
    ) -> Result<ReferenceDisplay, DemandError> {
        let TypeView::Reference(reference) = self.model.get(ty)? else {
            return Err(
                super::super::internal("reference display lacks a reference header").into(),
            );
        };
        if self.matches_array_target(reference.target, false) {
            Ok(ReferenceDisplay::Array { readonly: false })
        } else if self.matches_array_target(reference.target, true) {
            Ok(ReferenceDisplay::Array { readonly: true })
        } else if self.model.is_tuple_type(ty) {
            Ok(ReferenceDisplay::Tuple)
        } else {
            Ok(ReferenceDisplay::Ordinary)
        }
    }

    /// `getGlobalTypeAliasSymbol`: resolve the actual declaration before
    /// checking arity. A missing/wrong-kind/wrong-arity alias is absent; an
    /// acquired error type remains a consumed semantic failure.
    pub(super) fn global_type_alias(
        &mut self,
        name: &str,
        arity: usize,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        self.global_type_alias_completion(name, arity, consumer)?
            .admit()
    }

    pub(super) fn global_type_alias_completion(
        &mut self,
        name: &str,
        arity: usize,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        self.global_type_alias_resolver_completion(name, arity, false, consumer)
    }

    /// Checked and quiet `getGlobalTypeAliasResolver` closures have distinct
    /// memo cells. A checked missing alias emits a diagnostic but still returns
    /// actual nil; only a factory that consumes recovery can attach a marker.
    pub(super) fn global_type_alias_checked_completion(
        &mut self,
        name: &str,
        arity: usize,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        self.global_type_alias_resolver_completion(name, arity, true, consumer)
    }

    fn global_type_alias_resolver_completion(
        &mut self,
        name: &str,
        arity: usize,
        checked: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let key = (name.to_owned(), arity, checked);
        if let Some(result) = self.globals.aliases.get(&key) {
            return Ok(result.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let candidate = sequence.read(self.global_symbol_completion(
                name,
                NativeLookupMeaning::TYPE_ALIAS,
                consumer,
            ))?;
            let result = match candidate {
                None => None,
                Some(symbol) => {
                    sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
                    // This reads the selected raw symbol's installed typeAliasLinks;
                    // accepted import aliases have no such parameters even when
                    // declared-type resolution followed a generic target alias.
                    if self.installed_alias_parameter_count(symbol, consumer)? == arity {
                        Some(symbol)
                    } else {
                        None
                    }
                }
            };
            Ok(result)
        })();
        let result = sequence.finish(result);
        // The memoized resolver installs actual nil as well as a symbol.
        // An interrupted computation has produced neither native value.
        if let Ok(completed) = &result {
            self.globals.aliases.insert(key, completed.clone());
        }
        result
    }

    /// `getGlobalNonNullableTypeInstantiation`: the declared global alias is
    /// an actual dependency. Only its absence selects the native intersection.
    pub(super) fn global_non_nullable_instantiation_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.global_type_alias_completion("NonNullable", 1, consumer)?
            .and_then(|symbol| {
                if let Some(symbol) = symbol {
                    self.declared_reference_type_completion(symbol, consumer)?
                        .and_then(|declared| {
                            self.type_alias_instantiation_completion(
                                symbol,
                                declared,
                                vec![ty],
                                None,
                                consumer,
                            )
                        })
                } else {
                    let empty = self.model.empty_object_type();
                    self.intersection_type_completion(vec![ty, empty], consumer)
                }
            })
    }

    /// `getGlobalType` for a non-generic object consumes the actual declared
    /// target. Its empty-object recovery is not an established global inventory.
    pub(super) fn global_object_type(
        &mut self,
        name: &str,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.global_object_type_completion(name, consumer)?.admit()
    }

    pub(super) fn global_object_type_completion(
        &mut self,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.global_type_resolver_completion(name, 0, GlobalTypeGetter::Ordinary, consumer)?
            .and_then(|lookup| {
                self.global_type_lookup_installed_type_completion(name, 0, lookup, Some(consumer))
            })
    }

    /// Native `globalObjectType` identity, including its shared empty-object
    /// recovery sentinel. Merely comparing that identity does not consume the
    /// recovered object's inventory as `getGlobalType` factory callers do.
    pub(super) fn global_object_identity(
        &mut self,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.global_object_identity_named("Object", consumer)
    }

    pub(super) fn global_object_identity_named(
        &mut self,
        name: &str,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.global_object_identity_named_completion(name, consumer)?
            .admit()
    }

    pub(super) fn global_object_identity_named_completion(
        &mut self,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if fixed_global_arity(name).is_some() {
            let lookup = self.globals.fixed.get(name).ok_or_else(|| {
                super::super::internal(
                    "fixed global object identity requires an installed native field",
                )
            })?;
            let ty = match lookup.value() {
                GlobalTypeLookup::Target { ty, .. } => *ty,
                GlobalTypeLookup::Recovery(_) => self.model.empty_object_type(),
            };
            return Ok(NativeCompletion::clean(ty));
        }
        self.global_type_lookup_completion(name, 0, consumer)?
            .and_then(|lookup| match lookup {
                GlobalTypeLookup::Target { ty, .. } => Ok(NativeCompletion::clean(ty)),
                GlobalTypeLookup::Recovery(_) => {
                    Ok(NativeCompletion::clean(self.model.empty_object_type()))
                }
            })
    }

    /// `getGlobalDisposableType` / `getGlobalAsyncDisposableType` have
    /// checked resolver cells. A declaration only compares emptyObjectType
    /// here; failed global lookup does not itself replace its initializer.
    pub(super) fn global_disposable_identity_completion(
        &mut self,
        asynchronous: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.global_type_resolver_completion(
            if asynchronous {
                "AsyncDisposable"
            } else {
                "Disposable"
            },
            0,
            GlobalTypeGetter::CheckedDisposable,
            consumer,
        )?
        .and_then(|lookup| match lookup {
            GlobalTypeLookup::Target { ty, .. } => Ok(NativeCompletion::clean(ty)),
            GlobalTypeLookup::Recovery(_) => {
                Ok(NativeCompletion::clean(self.model.empty_object_type()))
            }
        })
    }

    /// `getGlobalStrictFunctionType`: strictBindCallApply selects the actual
    /// CallableFunction/NewableFunction global, otherwise both use Function.
    /// A missing strict global does not fall back to Function.
    pub(super) fn global_strict_function_type_completion(
        &mut self,
        construct: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let name = if self.inputs.semantic_options().strict_bind_call_apply {
            if construct {
                "NewableFunction"
            } else {
                "CallableFunction"
            }
        } else {
            "Function"
        };
        self.global_object_type_completion(name, consumer)
    }

    /// `getGlobalType`: global symbol flags precede declared-type acquisition;
    /// complete target arity is inspected only afterward. The native global
    /// getter owns memoization, and consumed declaration failures stay failures
    /// even if the target would subsequently fail its arity check.
    pub(super) fn global_type_lookup(
        &mut self,
        name: &str,
        arity: usize,
        consumer: &Location,
    ) -> Result<GlobalTypeLookup, DemandError> {
        self.global_type_lookup_completion(name, arity, consumer)?
            .admit()
    }

    pub(super) fn global_type_lookup_completion(
        &mut self,
        name: &str,
        arity: usize,
        consumer: &Location,
    ) -> NativeResult<GlobalTypeLookup> {
        self.global_type_resolver_completion(name, arity, GlobalTypeGetter::Ordinary, consumer)
    }

    fn global_type_resolver_completion(
        &mut self,
        name: &str,
        arity: usize,
        getter: GlobalTypeGetter,
        consumer: &Location,
    ) -> NativeResult<GlobalTypeLookup> {
        if getter == GlobalTypeGetter::Ordinary && fixed_global_arity(name).is_some() {
            if fixed_global_arity(name) != Some(arity) {
                return Err(
                    super::super::internal("fixed global getter used a different arity").into(),
                );
            }
            return self.globals.fixed.get(name).cloned().ok_or_else(|| {
                super::super::internal("fixed global factory ran before native slot installation")
                    .into()
            });
        }
        let key = (name.to_owned(), arity, getter);
        if let Some(result) = self.globals.types.get(&key) {
            return Ok(result.clone());
        }
        let result = self.global_type_worker_completion(name, arity, Some(consumer));
        if let Ok(completed) = &result {
            self.globals.types.insert(key, completed.clone());
        }
        result
    }

    /// `getGlobalType` performs an uncached current-symbol query. Selection and
    /// raw kind checks precede declared acquisition and exact target arity.
    pub(super) fn global_type_worker_completion(
        &mut self,
        name: &str,
        arity: usize,
        consumer: Option<&Location>,
    ) -> NativeResult<GlobalTypeLookup> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(symbol) = sequence.read(self.reference_global_name_completion(
                name,
                NativeLookupMeaning::TYPE,
                consumer,
            ))?
            else {
                return Ok(GlobalTypeLookup::Recovery(GlobalTypeRecovery::Missing));
            };
            let flags = self
                .reference_symbol_native_flags_at(symbol, consumer)?
                .type_flags();
            if !flags.class && !flags.interface {
                return Ok(GlobalTypeLookup::Recovery(GlobalTypeRecovery::WrongKind {
                    declaration: self.reference_global_type_declaration(symbol)?,
                }));
            }
            let ty = sequence
                .read(self.declared_reference_type_at_global_completion(symbol, consumer))?;
            let target = self.model.declared_target(ty)?;
            if target.outer_parameters.len() + target.local_parameters.len() != arity {
                return Ok(GlobalTypeLookup::Recovery(GlobalTypeRecovery::WrongArity {
                    declaration: self.reference_global_type_declaration(symbol)?,
                }));
            }
            Ok(GlobalTypeLookup::Target { symbol, ty })
        })();
        sequence.finish(result)
    }

    /// `getGlobalTypesResolver` owns one memoized complete list. Its members
    /// call getGlobalType directly; ordinary per-name resolver cells are distinct.
    pub(super) fn global_builtin_iteration_types_completion(
        &mut self,
        asynchronous: bool,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        if let Some(completed) = self.globals.iteration_builtins.get(&asynchronous) {
            return Ok(completed.clone());
        }
        let names: &[&str] = if asynchronous {
            &["ReadableStreamAsyncIterator"]
        } else {
            &[
                "ArrayIterator",
                "MapIterator",
                "SetIterator",
                "StringIterator",
            ]
        };
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut types = Vec::new();
            for name in names {
                match sequence.read(self.global_type_worker_completion(name, 1, Some(consumer)))? {
                    GlobalTypeLookup::Target { ty, .. } => types.push(ty),
                    GlobalTypeLookup::Recovery(_) => types.push(self.model.empty_generic_type()),
                }
            }
            Ok(types)
        })();
        let result = sequence.finish(result);
        if let Ok(completed) = &result {
            self.globals
                .iteration_builtins
                .insert(asynchronous, completed.clone());
        }
        result
    }

    /// A native factory consumes the global recovery it branches on, even
    /// when its final value is Unknown or emptyObjectType. Quiet global target
    /// startup identity probes use fixed_global_type_identity instead.
    fn global_type_for_factory_completion(
        &mut self,
        name: &str,
        arity: usize,
        getter: GlobalTypeGetter,
        consumer: &Location,
    ) -> NativeResult<GlobalTypeLookup> {
        self.global_type_resolver_completion(name, arity, getter, consumer)?
            .and_then(|lookup| {
                if let GlobalTypeLookup::Recovery(recovery) = &lookup {
                    let error = self.global_type_recovery_error(
                        name,
                        arity,
                        recovery.clone(),
                        Some(consumer),
                    )?;
                    NativeCompletion::recovered(lookup, AnalysisError::from(error).into())
                } else {
                    Ok(NativeCompletion::clean(lookup))
                }
            })
    }

    /// `getGlobalPromiseTypeChecked` has its own memoized resolver. Factories
    /// consuming its emptyGenericType result retain the global recovery even
    /// when createPromiseType returns native Unknown afterward.
    pub(super) fn global_promise_type_checked_completion(
        &mut self,
        consumer: &Location,
    ) -> NativeResult<GlobalTypeLookup> {
        self.global_type_for_factory_completion(
            "Promise",
            1,
            GlobalTypeGetter::CheckedPromise,
            consumer,
        )
    }

    /// `createPromiseLikeType` consumes the checked global resolver's recovery
    /// sentinel when constructing the generic reference.
    pub(super) fn global_promise_like_type_completion(
        &mut self,
        consumer: &Location,
    ) -> NativeResult<GlobalTypeLookup> {
        self.global_type_for_factory_completion(
            "PromiseLike",
            1,
            GlobalTypeGetter::Ordinary,
            consumer,
        )
    }

    /// `getGlobalIterableTypeChecked` and its async counterpart each have
    /// their own native resolver closure, distinct from quiet identity reads.
    pub(super) fn global_iterable_type_checked_completion(
        &mut self,
        asynchronous: bool,
        consumer: &Location,
    ) -> NativeResult<GlobalTypeLookup> {
        self.global_type_for_factory_completion(
            if asynchronous {
                "AsyncIterable"
            } else {
                "Iterable"
            },
            3,
            GlobalTypeGetter::CheckedIteration,
            consumer,
        )
    }

    /// The consumed-global diagnostic retains its original optional declaration.
    /// Building it can itself fail when recording resource-bounded evidence.
    fn global_type_recovery_error(
        &self,
        name: &str,
        arity: usize,
        recovery: GlobalTypeRecovery,
        consumer: Option<&Location>,
    ) -> Result<crate::analysis::SemanticError, DemandError> {
        let (reason, declaration, message) = match recovery {
            GlobalTypeRecovery::Missing => (
                SemanticFailureReason::UnresolvedSymbol,
                None,
                format!("Cannot find global type '{name}'."),
            ),
            GlobalTypeRecovery::WrongKind { declaration } => (
                SemanticFailureReason::ErrorType,
                declaration,
                format!("Global type '{name}' must be a class or interface type."),
            ),
            GlobalTypeRecovery::WrongArity { declaration } => (
                SemanticFailureReason::ErrorType,
                declaration,
                format!("Global type '{name}' must have {arity} type parameter(s)."),
            ),
        };
        let failure: DemandError = match consumer {
            Some(consumer) => super::super::invalid_at_location(
                self.inputs,
                consumer,
                SemanticFailureReason::ErrorType,
                format!("global type '{name}' dependency consumed compiler recovery"),
            )
            .into(),
            None => super::super::unbound_recovery(
                crate::analysis::UnboundSemanticRecovery::InstalledGlobalType,
            ),
        };
        let AnalysisError::Semantic(mut error) = AnalysisError::from(failure) else {
            return Err(super::super::internal("global recovery marker is not semantic").into());
        };
        error.append_cause(SemanticCause {
            reason: reason.into(),
            location: declaration,
            message,
            kind: Some(crate::analysis::SemanticErrorKind::InvalidSemantic),
            unbound_recovery: None,
        })?;
        Ok(*error)
    }

    /// Native initialization of `globalReadonlyArrayType`: recovery aliases
    /// the actual global Array question. Identity inspection and construction
    /// share this fallback decision; only a factory consumes its final recovery.
    pub(super) fn global_readonly_array_lookup_completion(
        &mut self,
        _consumer: &Location,
    ) -> NativeResult<(&'static str, GlobalTypeLookup)> {
        let name = self.globals.readonly_array_name.ok_or_else(|| {
            super::super::internal("ReadonlyArray factory ran before native slot installation")
        })?;
        let lookup = self
            .globals
            .fixed
            .get("ReadonlyArray")
            .cloned()
            .ok_or_else(|| super::super::internal("installed ReadonlyArray slot is absent"))?;
        Ok(lookup.map(|lookup| (name, lookup)))
    }

    /// `getGlobalType`'s installed value conversion keeps its empty-object and
    /// empty-generic sentinels distinct. Only consuming a recovery attaches its
    /// exact cause; startup publication and identity observations use value().
    pub(super) fn global_type_lookup_installed_type_completion(
        &mut self,
        name: &str,
        arity: usize,
        lookup: GlobalTypeLookup,
        consumer: Option<&Location>,
    ) -> NativeResult<TypeId> {
        match lookup {
            GlobalTypeLookup::Target { ty, .. } => Ok(NativeCompletion::clean(ty)),
            GlobalTypeLookup::Recovery(recovery) => {
                let ty = if arity == 0 {
                    self.model.empty_object_type()
                } else {
                    self.model.empty_generic_type()
                };
                let error = self.global_type_recovery_error(name, arity, recovery, consumer)?;
                NativeCompletion::recovered(ty, AnalysisError::from(error).into())
            }
        }
    }

    /// `initializeChecker` publishes each field before the next getter can
    /// observe it, retaining the complete producer without admitting recovery.
    fn install_fixed_global_completion(
        &mut self,
        name: &'static str,
        arity: usize,
    ) -> NativeResult<()> {
        let lookup = self.global_type_worker_completion(name, arity, None)?;
        self.globals.fixed.insert(name, lookup.clone());
        Ok(lookup.map(|_| ()))
    }

    /// `initializeChecker`'s fixed phase is ordered Array, Object, Function,
    /// strict function globals, primitive objects, startup arrays, ReadonlyArray,
    /// readonly startup array, then ThisType. Lazy closures are separate owners.
    pub(super) fn initialize_fixed_globals_completion(&mut self) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for (name, arity) in [("Array", 1), ("Object", 0), ("Function", 0)] {
                sequence.read(self.install_fixed_global_completion(name, arity))?;
            }
            if self.inputs.semantic_options().strict_bind_call_apply {
                sequence.read(self.install_fixed_global_completion("CallableFunction", 0))?;
                sequence.read(self.install_fixed_global_completion("NewableFunction", 0))?;
            } else {
                let function = self.globals.fixed.get("Function").cloned().ok_or_else(|| {
                    super::super::internal("Function slot was not published by its phase")
                })?;
                self.globals
                    .fixed
                    .insert("CallableFunction", function.clone());
                self.globals.fixed.insert("NewableFunction", function);
            }
            for name in ["String", "Number", "Boolean", "RegExp"] {
                sequence.read(self.install_fixed_global_completion(name, 0))?;
            }
            let array = self.fixed_global_target_value("Array", 1)?;
            let any = self.model.special(super::super::model::SpecialName::Any);
            let any_array = self.type_from_generic_global(array, vec![any])?;
            self.globals.any_array = Some(any_array);
            let automatic = self.model.automatic_variable();
            let mut automatic_array = self.type_from_generic_global(array, vec![automatic])?;
            if automatic_array == self.model.empty_object_type() {
                use super::super::model::{MembersView, Metadata, ObjectKind, TypeSymbol};
                automatic_array = self.model.structured(
                    TypeSymbol::Absent,
                    MembersView {
                        properties: std::sync::Arc::from([]),
                        call_signatures: std::sync::Arc::from([]),
                        construct_signatures: std::sync::Arc::from([]),
                        index_signatures: std::sync::Arc::from([]),
                    },
                    Metadata::default(),
                    ObjectKind::Anonymous,
                );
            }
            self.model.set_automatic_array(automatic_array);
            let readonly = self.global_type_worker_completion("ReadonlyArray", 1, None)?;
            sequence.read(Ok(readonly.clone()))?;
            let selected = match readonly.value() {
                GlobalTypeLookup::Target { .. } => {
                    self.globals.readonly_array_name = Some("ReadonlyArray");
                    readonly
                }
                GlobalTypeLookup::Recovery(_) => {
                    self.globals.readonly_array_name = Some("Array");
                    let array = self.globals.fixed.get("Array").cloned().ok_or_else(|| {
                        super::super::internal("Array fallback slot was not published")
                    })?;
                    readonly.and_then(|_| Ok(array))?
                }
            };
            self.globals.fixed.insert("ReadonlyArray", selected);
            let readonly = self.fixed_global_target_value("ReadonlyArray", 1)?;
            self.globals.any_readonly_array =
                Some(self.type_from_generic_global(readonly, vec![any])?);
            sequence.read(self.install_fixed_global_completion("ThisType", 1))?;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `initializeChecker` startup factories observe the actual installed
    /// pointer; they do not consume the recovery attached to a later query.
    fn fixed_global_target_value(
        &mut self,
        name: &str,
        arity: usize,
    ) -> Result<TypeId, DemandError> {
        let lookup = self.globals.fixed.get(name).ok_or_else(|| {
            super::super::internal("startup factory requires its installed global field")
        })?;
        Ok(match lookup.value() {
            GlobalTypeLookup::Target { ty, .. } => *ty,
            GlobalTypeLookup::Recovery(_) if arity == 0 => self.model.empty_object_type(),
            GlobalTypeLookup::Recovery(_) => self.model.empty_generic_type(),
        })
    }

    /// `convertAutoToAny` reads initializeChecker's cached anyArrayType field.
    /// This is an identity read, so it neither repeats createArrayType nor
    /// consumes an unrelated fixed Array recovery on every automatic variable.
    pub(super) fn installed_any_array_type(&self) -> Result<TypeId, DemandError> {
        self.globals.any_array.ok_or_else(|| {
            super::super::internal("anyArrayType identity was observed before initialization")
                .into()
        })
    }

    /// `isArrayLikeType` and `checkParameter` read initializeChecker's cached
    /// anyReadonlyArrayType. This field observation does not repeat a factory.
    pub(super) fn installed_any_readonly_array_type(&self) -> Result<TypeId, DemandError> {
        self.globals.any_readonly_array.ok_or_else(|| {
            super::super::internal(
                "anyReadonlyArrayType identity was observed before initialization",
            )
            .into()
        })
    }

    /// `createArrayTypeEx` consumes the canonical one-parameter global Array
    /// target after its element is established. Target/reference identity and
    /// deferred native member recipes share the ordinary generic factories.
    pub(super) fn array_type(
        &mut self,
        element: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.array_type_completion(element, consumer)?.admit()
    }

    pub(super) fn array_type_completion(
        &mut self,
        element: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.array_target_completion(false, consumer)?
            .and_then_value(|target| self.type_from_generic_global(target, vec![element]))
    }

    /// `createArrayTypeEx(..., readonly=true)` uses the same real native
    /// target fallback as isReadonlyArrayType. No names or display flags
    /// substitute for its canonical binding identity.
    pub(super) fn readonly_array_type(
        &mut self,
        element: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.readonly_array_type_completion(element, consumer)?
            .admit()
    }

    pub(super) fn readonly_array_type_completion(
        &mut self,
        element: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.array_target_completion(true, consumer)?
            .and_then_value(|target| self.type_from_generic_global(target, vec![element]))
    }

    /// `createTypeFromGenericGlobalType`: the missing generic-global sentinel
    /// produces emptyObjectType. Ordinary type-reference factories ask a
    /// different question and must not substitute this fallback themselves.
    pub(super) fn type_from_generic_global(
        &mut self,
        target: TypeId,
        arguments: Vec<TypeId>,
    ) -> Result<TypeId, DemandError> {
        if target == self.model.empty_generic_type() {
            Ok(self.model.empty_object_type())
        } else {
            self.model.reference(target, arguments)
        }
    }

    /// Actual installed Array/ReadonlyArray target selection shared by syntax,
    /// tuple rest collapse and createArrayTypeEx. Element checking belongs to
    /// each caller's native schedule; this operation consumes no element.
    pub(super) fn array_target(
        &mut self,
        readonly: bool,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.array_target_completion(readonly, consumer)?.admit()
    }

    pub(super) fn array_target_completion(
        &mut self,
        readonly: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let completed = if readonly {
            self.global_readonly_array_lookup_completion(consumer)?
        } else {
            self.global_type_lookup_completion("Array", 1, consumer)?
                .map(|lookup| ("Array", lookup))
        };
        completed.and_then(|(name, lookup)| {
            self.global_type_lookup_installed_type_completion(name, 1, lookup, Some(consumer))
        })
    }
}
