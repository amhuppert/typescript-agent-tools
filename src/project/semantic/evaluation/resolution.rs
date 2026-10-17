//! The compiler's type-resolution stack, distinct from member and relation
//! re-entry. A cache's work-in-progress marker is not itself a cycle decision.

use super::super::model::{PropertySymbolId, SignatureId, TypeId};
use super::{Evaluator, TypeState};
use crate::project::binding::BoundSymbol;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Property {
    Type,
    WriteType,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Key {
    AliasTarget(crate::project::binding::NativeSymbolIdentity),
    DeclaredType(crate::project::binding::NativeSymbolIdentity),
    AliasType(crate::project::binding::NativeSymbolIdentity),
    Symbol {
        target: BoundSymbol,
        property: Property,
    },
    PropertySymbol {
        target: PropertySymbolId,
        property: Property,
    },
    SignatureReturn(SignatureId),
    ParameterInitializerContainsUndefined(super::super::source::DeclarationIdentity),
    BaseConstraint(TypeId),
    BaseTypes(TypeId),
    BaseConstructorType(TypeId),
    TypeArguments(TypeId),
}

struct Frame {
    key: Key,
    result: bool,
}

#[derive(Default)]
pub(super) struct TypeResolution {
    frames: Vec<Frame>,
    // `getResolvedSignature` changes this window during its own operation.
    start: usize,
}

impl TypeResolution {
    /// `getResolvedSignature` saves its resolutionStart and resets it only
    /// for an uncached call, admitting contextual type resolution retries.
    pub(super) fn begin_call(&mut self, uncached: bool) -> usize {
        let previous = self.start;
        if uncached {
            self.start = self.frames.len();
        }
        previous
    }

    pub(super) fn end_call(&mut self, previous: usize) {
        self.start = previous;
    }
    /// `pushTypeResolution` / `findResolutionCycleStartIndex`: an established
    /// intervening property ends the search before key matching. A repeated
    /// target/property marks every participant within the current window;
    /// the rejected push adds no frame and therefore requires no pop.
    fn find_cycle_start(&self, key: Key, has_property: impl Fn(Key) -> bool) -> Option<usize> {
        for index in (self.start..self.frames.len()).rev() {
            let frame = &self.frames[index];
            if has_property(frame.key) {
                break;
            }
            if frame.key == key {
                return Some(index);
            }
        }
        None
    }

    fn push(&mut self, key: Key, cycle: Option<usize>) -> bool {
        if let Some(index) = cycle {
            for participant in &mut self.frames[index..] {
                participant.result = false;
            }
            return false;
        }
        self.frames.push(Frame { key, result: true });
        true
    }

    /// `popTypeResolution`: the operation's result says whether its frame
    /// participated in a cycle, independently of a child's recovered value.
    pub(super) fn pop(&mut self) -> Option<bool> {
        self.frames.pop().map(|frame| frame.result)
    }
}

impl Evaluator<'_, '_> {
    /// `typeResolutionHasProperty`: only the actual native cache property
    /// ends a cycle search. A pending wrapper is not an installed property.
    fn resolution_has_property(&self, key: Key) -> bool {
        match key {
            Key::DeclaredType(origin) => self
                .declared_types
                .get(&origin)
                .is_some_and(TypeState::has_type),
            Key::AliasTarget(origin) => self.symbol_aliases.has_target(origin),
            Key::AliasType(origin) => self.node_types.alias_has_value(origin),
            Key::Symbol { target, property } => match property {
                Property::Type => self.value_types.get(&target),
                Property::WriteType => self.native_members.writes.get(&target),
            }
            .is_some_and(TypeState::has_type),
            Key::PropertySymbol { target, property } => match property {
                Property::Type => self.property_has_value(target, false),
                Property::WriteType => self.property_has_value(target, true),
            },
            Key::SignatureReturn(signature) => self
                .signatures
                .has_return_property(signature, &self.demands),
            Key::ParameterInitializerContainsUndefined(declaration) => {
                self.reads.has_initializer_undefined(declaration)
            }
            Key::BaseConstraint(ty) => self.constraints.has_property(ty),
            Key::BaseTypes(ty) => self.generics.has_base_types(ty),
            Key::BaseConstructorType(ty) => self.generics.has_base_constructor(ty),
            Key::TypeArguments(ty) => self.model.has_resolved_type_arguments(ty),
        }
    }

    /// `findResolutionCycleStartIndex` is also a read-only signature-return
    /// observation. It neither marks participants nor pushes a frame.
    pub(super) fn resolution_cycle_start(&self, key: Key) -> Option<usize> {
        self.resolution
            .find_cycle_start(key, |key| self.resolution_has_property(key))
    }

    /// `pushTypeResolution` uses exactly the same window and property barriers
    /// as return observations, then marks the rejected cycle's participants.
    pub(super) fn push_resolution(&mut self, key: Key) -> bool {
        let cycle = self.resolution_cycle_start(key);
        self.resolution.push(key, cycle)
    }
}
