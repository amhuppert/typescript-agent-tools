//! Native relation decisions are shared by applicability and narrowing.
//! False is established data; an unavailable or invalid dependency is an error.

use super::super::model::{
    AccessFlags, DemandError, Index, IndexFlags, IntersectionFlags, NativeCompletion, NativeResult,
    NativeSequence, ObjectFlags, Property, PropertySymbolId, Signature, SpecialName,
    TupleElementFlags, TypeFlags, TypeId, TypeView, VarianceFlags,
};
use super::property_symbols::RecursionIdentity;
use super::{Evaluator, ReferenceSymbol, facts::TypeFacts};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::{NativePropertyKey, NativeSymbolIdentity};
use crate::references::Location;
use crate::syntax::modifiers;
use crate::syntax_kind::SyntaxKind;
use crate::types::{PrimitiveName, TypePredicateKind};
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, DefaultHasher, Hash, Hasher};
use std::sync::Arc;

mod discriminants;
mod elaboration;
mod enums;
mod excess;
mod mapped;
mod reporting;
use reporting::{Argument, ErrorChain, ErrorContext, ErrorState, Message, RelationInvocation};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum RelationKind {
    Identity,
    Assignable,
    Subtype,
    StrictSubtype,
    Comparable,
}

/// One native checking invocation's relation result and emitted diagnostics.
/// Direct `checkTypeRelatedToEx` emits zero or one; optional elaboration may
/// emit several. This is invocation output, never relation-cache metadata.
#[derive(Clone, Copy)]
pub(in crate::project::semantic::evaluation) struct RelationCheck {
    pub(in crate::project::semantic::evaluation) related: bool,
    pub(in crate::project::semantic::evaluation) diagnostics: usize,
}

/// `checkTypeRelatedToEx`'s error-node presence is distinct from the relation.
#[derive(Clone, Copy)]
pub(super) enum RelationReporting {
    Silent,
    Report,
}

impl RelationReporting {
    /// `checkClassLikeDeclaration` and `issueMemberSpecificError` retain
    /// their native diagnostic heads separately from ordinary assignment.
    pub(super) fn class_heritage_at(
        self,
        node: impl Into<super::super::source::ExpressionNode>,
        implemented: Option<bool>,
        static_: bool,
    ) -> Option<ErrorContext> {
        let head = match implemented {
            Some(true) => Message::ClassImplementsClass,
            Some(false) => Message::ClassImplementsInterface,
            None if static_ => Message::ClassStaticExtends,
            None => Message::ClassExtends,
        };
        match self {
            Self::Silent => None,
            Self::Report => Some(ErrorContext {
                node: node.into(),
                head: Some(head),
            }),
        }
    }
    /// `computeConstantEnumMemberValue` supplies its own diagnostic head.
    pub(super) fn computed_enum_at(
        self,
        node: impl Into<super::super::source::ExpressionNode>,
    ) -> Option<ErrorContext> {
        match self {
            Self::Silent => None,
            Self::Report => Some(ErrorContext {
                node: node.into(),
                head: Some(Message::ComputedEnumMember),
            }),
        }
    }
    /// `checkSatisfiesExpression` supplies a distinct diagnostic head.
    pub(super) fn satisfies_at(
        self,
        node: impl Into<super::super::source::ExpressionNode>,
    ) -> Option<ErrorContext> {
        match self {
            Self::Silent => None,
            Self::Report => Some(ErrorContext {
                node: node.into(),
                head: Some(Message::Satisfies),
            }),
        }
    }

    pub(super) fn exact_optional_assignment_at(
        self,
        node: impl Into<super::super::source::ExpressionNode>,
    ) -> Option<ErrorContext> {
        match self {
            Self::Silent => None,
            Self::Report => Some(ErrorContext {
                node: node.into(),
                head: Some(Message::ExactOptionalAssignable),
            }),
        }
    }
    /// `checkTypeAssignableTo` reports at the actual assignment node without
    /// a caller-supplied diagnostic head.
    pub(super) fn assignment_at(
        self,
        node: impl Into<super::super::source::ExpressionNode>,
    ) -> Option<ErrorContext> {
        match self {
            Self::Silent => None,
            Self::Report => Some(ErrorContext {
                node: node.into(),
                head: None,
            }),
        }
    }

    /// `checkVariableLikeDeclaration` supplies the using-kind-specific head
    /// while the relation retains its ordinary source/target arguments.
    pub(super) fn using_initializer_at(
        self,
        node: impl Into<super::super::source::ExpressionNode>,
        asynchronous: bool,
    ) -> Option<ErrorContext> {
        match self {
            Self::Silent => None,
            Self::Report => Some(ErrorContext {
                node: node.into(),
                head: Some(if asynchronous {
                    Message::AwaitUsingInitializer
                } else {
                    Message::UsingInitializer
                }),
            }),
        }
    }

    /// `getIteratedTypeOrElementType` selects one diagnostic head from its
    /// native iteration use, while the ordinary relation owns all arguments.
    pub(super) fn iteration_at(
        self,
        node: super::super::source::ExpressionNode,
        use_: super::iteration::IterationUse,
    ) -> Option<ErrorContext> {
        let head = match use_ {
            super::iteration::IterationUse::ForOf
            | super::iteration::IterationUse::ForAwaitOf
            | super::iteration::IterationUse::ExplicitForOf
            | super::iteration::IterationUse::ExplicitForAwaitOf => Message::IterationForOf,
            super::iteration::IterationUse::Spread => Message::IterationSpread,
            _ => Message::IterationDestructuring,
        };
        match self {
            Self::Silent => None,
            Self::Report => Some(ErrorContext {
                node,
                head: Some(head),
            }),
        }
    }

    /// `checkTypeArguments` supplies this exact head and the actual written
    /// type-argument node only when reportErrors is enabled.
    pub(super) fn constraint_at(
        self,
        node: impl Into<super::super::source::ExpressionNode>,
    ) -> Option<ErrorContext> {
        match self {
            Self::Silent => None,
            Self::Report => Some(ErrorContext {
                node: node.into(),
                head: Some(Message::Constraint),
            }),
        }
    }

    /// `checkSignatureApplicability` uses distinct argument/this heads. The
    /// latter is not the nested-signature `ThisTypes` compatibility message.
    pub(super) fn argument_at(
        self,
        node: impl Into<super::super::source::ExpressionNode>,
    ) -> Option<ErrorContext> {
        match self {
            Self::Silent => None,
            Self::Report => Some(ErrorContext {
                node: node.into(),
                head: Some(Message::Argument),
            }),
        }
    }

    pub(super) fn this_context_at(
        self,
        node: impl Into<super::super::source::ExpressionNode>,
    ) -> Option<ErrorContext> {
        match self {
            Self::Silent => None,
            Self::Report => Some(ErrorContext {
                node: node.into(),
                head: Some(Message::ThisContext),
            }),
        }
    }
}

/// `Ternary` preserves provisional recursion and variance answers until the
/// native relation session commits them. Only False rejects a comparison.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
enum Ternary {
    False = 0,
    Unknown = 1,
    Maybe = 3,
    True = -1,
}

#[derive(Clone, Copy)]
enum TypeComparer {
    Equal,
    Independent(RelationKind),
    Relation(RelationInvocation),
}

/// The native reference switch can select its variance branch yet require
/// structural fallback. That differs from not selecting the branch at all.
enum ReferenceRelation {
    NotApplicable,
    Structural,
    Related(Ternary),
}

/// One `structuredTypeRelatedToWorker` invocation owns the error snapshot and
/// variance fallback decision. These are local to this comparison, including
/// during inference callback reentry into the same relation session.
struct StructuredComparison {
    comparer: RelationComparer,
    saved_error: ErrorState,
    variance_failed: bool,
    original_chain: Option<Arc<ErrorChain>>,
}

/// `compareSignaturesIdentical` independently controls matching arity and
/// whether this/return questions participate in the comparison.
#[derive(Clone, Copy, Default)]
pub(super) struct SignatureComparison {
    pub(super) partial_match: bool,
    pub(super) ignore_this_types: bool,
    pub(super) ignore_return_types: bool,
}

impl Ternary {
    fn and(self, other: Self) -> Self {
        match (self as i8) & (other as i8) {
            0 => Self::False,
            1 => Self::Unknown,
            3 => Self::Maybe,
            -1 => Self::True,
            _ => unreachable!("native Ternary conjunction is closed"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct IntersectionState(u8);

impl IntersectionState {
    const NONE: Self = Self(0);
    const SOURCE: Self = Self(1);
    const TARGET: Self = Self(2);
}

#[derive(Clone, Copy)]
struct RecursionFlags(u8);

impl RecursionFlags {
    const SOURCE: Self = Self(1);
    const TARGET: Self = Self(2);
    const BOTH: Self = Self(3);
}

/// `SignatureCheckMode` belongs to the comparison invocation, including nested
/// callbacks; it is not a property of either signature or the relation cache.
#[derive(Clone, Copy)]
struct SignatureCheckMode(u8);

impl SignatureCheckMode {
    const NONE: Self = Self(0);
    const BIVARIANT_CALLBACK: u8 = 1;
    const STRICT_CALLBACK: u8 = 2;
    const CALLBACK: u8 = 3;
    const STRICT_TOP: u8 = 4;
    const STRICT_ARITY: u8 = 8;

    fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum KeyPart {
    Type(TypeId),
    Parameter(usize),
    Argument,
    Open,
    Close,
    Pair,
}

/// `keyBuilder.writeGenericTypeReferences` shares first-seen parameter numbering
/// across both references. Own the scan and scratch growth together so repeated
/// parameters pay only for the comparisons actually made by the native builder.
struct GenericKeyWriter {
    ignore_constraints: bool,
    parameters: Vec<TypeId>,
    parts: Vec<KeyPart>,
    constrained: bool,
}

impl GenericKeyWriter {
    fn parameter(&mut self, argument: TypeId) -> usize {
        for (index, &parameter) in self.parameters.iter().enumerate() {
            if parameter == argument {
                return index;
            }
        }
        let index = self.parameters.len();
        self.parameters.push(argument);
        index
    }

    fn push(&mut self, part: KeyPart) {
        self.parts.push(part);
    }
}

#[derive(Clone)]
enum RelationKeyValue {
    Ordinary(TypeId, TypeId, IntersectionState),
    Generic(Arc<[KeyPart]>, IntersectionState),
}

/// `getRelationKey` emits an immutable key. Compute its fingerprint once;
/// retain the exact representation so a hash collision cannot merge answers.
#[derive(Clone)]
struct RelationKey {
    value: RelationKeyValue,
    fingerprint: u64,
}

impl RelationKey {
    fn new(value: RelationKeyValue) -> Self {
        let mut hash = DefaultHasher::new();
        std::mem::discriminant(&value).hash(&mut hash);
        match &value {
            RelationKeyValue::Ordinary(source, target, intersection) => {
                (source, target, intersection).hash(&mut hash);
            }
            RelationKeyValue::Generic(parts, intersection) => {
                parts.len().hash(&mut hash);
                for part in parts.iter() {
                    part.hash(&mut hash);
                }
                intersection.hash(&mut hash);
            }
        }
        Self {
            value,
            fingerprint: hash.finish(),
        }
    }
}

impl crate::resources::CheckedKey for RelationKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn equivalent(&self, other: &Self) -> bool {
        match (&self.value, &other.value) {
            (RelationKeyValue::Ordinary(a, b, c), RelationKeyValue::Ordinary(x, y, z)) => {
                (a, b, c) == (x, y, z)
            }
            (RelationKeyValue::Generic(a, state_a), RelationKeyValue::Generic(b, state_b)) => {
                if state_a != state_b || a.len() != b.len() {
                    return false;
                }
                if Arc::ptr_eq(a, b) {
                    return true;
                }
                for (left, right) in a.iter().zip(b.iter()) {
                    if left != right {
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    }
}

// Reproducible buckets also make capacity changes after deletion reproducible.
// Semantic ordering never depends on iteration of these private tables.
type RelationHasher = BuildHasherDefault<DefaultHasher>;
/// Native Relation storage and the Maybe-key set share checked collision and
/// capacity mechanics; RelationKey retains the compiler's exact key identity.
type RelationTable<V> = crate::resources::CheckedMap<RelationKey, V>;

#[derive(Clone, Copy)]
enum RelationOverflow {
    Depth,
    Complexity,
}

/// Native cache installation and product failure provenance are separate.
/// Inner Failed entries still install during overflow unwinding, and every
/// cache hit preserves an overflow that its completed operation consumed.
#[derive(Clone)]
struct RelationEntry {
    result: NativeCompletion<bool>,
    overflow: Option<RelationOverflow>,
    reliability: VarianceFlags,
}

pub(super) struct RelationsState {
    // `isEnumTypeRelatedTo`: ordered source/target identity, never an unordered pair.
    enum_relations: HashMap<
        (
            crate::project::binding::NativeSymbolIdentity,
            crate::project::binding::NativeSymbolIdentity,
        ),
        NativeCompletion<bool>,
    >,
    caches: HashMap<RelationKind, RelationTable<RelationEntry>>,
    active: HashMap<RelationSessionId, Relater, RelationHasher>,
    next_session: usize,
    variances: HashMap<NativeSymbolIdentity, NativeResult<Vec<VarianceFlags>>>,
    marker_types: HashSet<TypeId>,
    in_variance: bool,
    reliability: VarianceFlags,
}

/// A request-local invocation identity. Inference mappers retain this handle
/// without borrowing or removing the active relater during semantic reentry.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct RelationSessionId(usize);

/// Native TypeComparer captures both the invocation and its intersection
/// state. Inference's independent assignability questions do not use it.
#[derive(Clone, Copy)]
pub(super) struct RelationComparer {
    session: RelationInvocation,
    intersection: IntersectionState,
}

impl RelationsState {
    pub(super) fn new() -> Self {
        Self {
            enum_relations: HashMap::new(),
            caches: HashMap::new(),
            active: HashMap::default(),
            next_session: 0,
            variances: HashMap::new(),
            marker_types: HashSet::new(),
            in_variance: false,
            reliability: VarianceFlags::default(),
        }
    }

    /// `Relation.set`: all native commits share this storage owner, including
    /// failed comparisons, Maybe commits and root overflow entries. Replacement
    /// retains the existing table capacity.
    fn store(&mut self, kind: RelationKind, key: RelationKey, entry: RelationEntry) {
        self.cache_mut(kind).insert(key, entry);
    }

    fn cache_mut(&mut self, kind: RelationKind) -> &mut RelationTable<RelationEntry> {
        self.caches.entry(kind).or_default()
    }

    fn session(&self, invocation: RelationInvocation) -> &Relater {
        self.active
            .get(&invocation.session)
            .expect("relation operation has an active session")
    }

    fn session_mut(&mut self, invocation: RelationInvocation) -> &mut Relater {
        self.active
            .get_mut(&invocation.session)
            .expect("relation operation has an active session")
    }

    fn begin(&mut self, kind: RelationKind) -> RelationInvocation {
        let id = RelationSessionId(self.next_session);
        self.next_session = self
            .next_session
            .checked_add(1)
            .expect("request-local relation identity fits usize");
        let cache_size = self.cache_mut(kind).len();
        self.active.insert(id, Relater::new(kind, cache_size));
        RelationInvocation::silent(id)
    }
}

/// A pending native assumption retains only the work its own comparison
/// acquired after lookup. Ancestor work is never copied into a completed child.
struct MaybeRelation {
    key: RelationKey,
    result: Option<NativeCompletion<Ternary>>,
}

/// One native `Relater` invocation. Nested checker questions can create a
/// separate session while using the same relation-kind cache; comparisons
/// within this invocation retain their Ternary assumptions and stack depths.
struct Relater {
    kind: RelationKind,
    maybe_keys: Vec<MaybeRelation>,
    maybe_set: RelationTable<()>,
    source_stack: Vec<TypeId>,
    target_stack: Vec<TypeId>,
    expanding: u8,
    allowance: i64,
    overflow: Option<RelationOverflow>,
    error: ErrorState,
    error_node: Option<super::super::source::ExpressionNode>,
}

impl Relater {
    fn new(kind: RelationKind, cache_size: usize) -> Self {
        Self {
            kind,
            maybe_keys: Vec::new(),
            maybe_set: RelationTable::new(),
            source_stack: Vec::new(),
            target_stack: Vec::new(),
            expanding: 0,
            allowance: (16_000_000 - cache_size as i64) / 8,
            overflow: None,
            error: ErrorState::default(),
            error_node: None,
        }
    }
}

impl Evaluator<'_, '_> {
    /// `getVariances` uses the native global Array/ReadonlyArray identities;
    /// all other declared references use measured symbol variance.
    pub(super) fn reference_variances_completion(
        &mut self,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<VarianceFlags>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<VarianceFlags>, DemandError> {
            if self.model.is_tuple_type(target) {
                return Ok(vec![VarianceFlags::COVARIANT]);
            }
            let view = self.model.declared_target(target)?;
            if sequence.read(self.is_array_type_completion(target))? {
                return Ok(vec![VarianceFlags::COVARIANT]);
            }
            let symbol = self
                .reference_symbol_of_type(target, consumer)?
                .ok_or_else(|| super::super::internal("variance target has no native symbol"))?;
            let parameters = view
                .outer_parameters
                .iter()
                .chain(view.local_parameters.iter())
                .copied()
                .collect();
            sequence.read(self.variances_worker_completion(symbol, parameters, consumer))
        })();
        sequence.finish(result)
    }

    pub(super) fn alias_variances_completion(
        &mut self,
        symbol: impl Into<super::super::model::AliasSymbol>,
        consumer: &Location,
    ) -> NativeResult<Vec<VarianceFlags>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<VarianceFlags>, DemandError> {
            let symbol = symbol.into();
            let parameters = sequence.read(self.alias_parameters_completion(symbol, consumer))?;
            sequence.read(self.variances_worker_completion(
                self.alias_reference_symbol(symbol),
                parameters,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `getVariancesWorker` publishes the recursive empty slice before marker
    /// comparisons. Product failures poison that provisional cache and always
    /// restore the native resolution window and reliability accumulator.
    fn variances_worker_completion(
        &mut self,
        symbol: ReferenceSymbol,
        parameters: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<Vec<VarianceFlags>> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        if let Some(result) = self.relations.variances.get(&identity) {
            return result.clone();
        }
        let old_variance = self.relations.in_variance;
        let saved_start = self.resolution.begin_call(!old_variance);
        self.relations.in_variance = true;
        self.relations
            .variances
            .insert(identity, Ok(NativeCompletion::clean(Vec::new())));
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut variances = Vec::with_capacity(parameters.len());
            for parameter in parameters {
                if let Some(declared) = self.model.get_parameter_variance_modifiers(parameter)? {
                    variances.push(declared);
                    continue;
                }
                let saved_reliability = self.relations.reliability;
                self.relations.reliability = VarianceFlags::INVARIANT;
                let measured = (|| {
                    let markers = self.model.marker_types();
                    let with_super = sequence.read(self.create_marker_type_completion(
                        symbol,
                        parameter,
                        markers.super_type,
                        consumer,
                    ))?;
                    let with_sub = sequence.read(self.create_marker_type_completion(
                        symbol,
                        parameter,
                        markers.sub_type,
                        consumer,
                    ))?;
                    let mut variance = VarianceFlags::INVARIANT;
                    if sequence.read(self.raw_type_related_completion(
                        with_sub,
                        with_super,
                        RelationKind::Assignable,
                        consumer,
                    ))? {
                        variance = variance | VarianceFlags::COVARIANT;
                    }
                    if sequence.read(self.raw_type_related_completion(
                        with_super,
                        with_sub,
                        RelationKind::Assignable,
                        consumer,
                    ))? {
                        variance = variance | VarianceFlags::CONTRAVARIANT;
                    }
                    if variance == VarianceFlags::BIVARIANT {
                        let with_other = sequence.read(self.create_marker_type_completion(
                            symbol,
                            parameter,
                            markers.other_type,
                            consumer,
                        ))?;
                        if sequence.read(self.raw_type_related_completion(
                            with_other,
                            with_super,
                            RelationKind::Assignable,
                            consumer,
                        ))? {
                            variance = VarianceFlags::INDEPENDENT;
                        }
                    }
                    Ok::<_, DemandError>(variance | self.relations.reliability)
                })();
                self.relations.reliability = saved_reliability;
                variances.push(measured?);
            }
            Ok(variances)
        })();
        self.relations.in_variance = old_variance;
        self.resolution.end_call(saved_start);
        let result = sequence.finish(result);
        self.relations.variances.insert(identity, result.clone());
        result
    }

    /// `createMarkerType` instantiates the declared parameter list through a
    /// fresh simple mapper and marks only the resulting generic instantiation.
    fn create_marker_type_completion(
        &mut self,
        symbol: ReferenceSymbol,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let mapper = self.model.mapper(vec![source], vec![target])?;
            let declared =
                sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
            // Native isErrorType includes alias-bearing Any. Such a declared
            // result skips parameter instantiation and marker registration alike.
            if self.model.is_error_type(declared) {
                return Ok(declared);
            }
            let alias = self
                .reference_symbol_native_flags(symbol, consumer)?
                .type_flags()
                .type_alias;
            let parameters = if alias {
                let alias_symbol = self.reference_alias_symbol(symbol, consumer)?;
                sequence.read(self.alias_parameters_completion(alias_symbol, consumer))?
            } else {
                let target = self.model.declared_target(declared)?;
                target
                    .outer_parameters
                    .iter()
                    .chain(target.local_parameters.iter())
                    .copied()
                    .collect()
            };
            let mut arguments = Vec::with_capacity(parameters.len());
            for parameter in parameters {
                arguments.push(
                    sequence.read(self.instantiate_type_completion(parameter, mapper, consumer))?,
                );
            }
            let result = if alias {
                sequence.read(self.type_alias_instantiation_completion(
                    symbol, declared, arguments, None, consumer,
                ))?
            } else {
                self.model.reference(declared, arguments)?
            };
            self.relations.marker_types.insert(result);
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `reportUnreliableWorker` / `reportUnmeasurableWorker` observe exact
    /// inference marker identities, excluding the diagnostic ForCheck pair.
    pub(super) fn report_variance_reliability(&mut self, ty: TypeId, unmeasurable: bool) {
        let markers = self.model.marker_types();
        if [markers.super_type, markers.sub_type, markers.other_type].contains(&ty) {
            self.relations.reliability = self.relations.reliability
                | if unmeasurable {
                    VarianceFlags::UNMEASURABLE
                } else {
                    VarianceFlags::UNRELIABLE
                };
        }
    }

    /// `relateVariances` permits structural fallback only for recorded
    /// reliability flags or an actual covariant Void target argument.
    fn relate_variances_completion(
        &mut self,
        source: &[TypeId],
        target: &[TypeId],
        variances: &[VarianceFlags],
        state: &mut StructuredComparison,
        consumer: &Location,
    ) -> NativeResult<Option<Ternary>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Ternary>, DemandError> {
            let comparer = state.comparer;
            let result = sequence.read(self.type_arguments_related_completion(
                source, target, variances, comparer, consumer,
            ))?;
            if result != Ternary::False {
                return Ok(Some(result));
            }
            if variances
                .iter()
                .any(|variance| variance.intersects(VarianceFlags::ALLOWS_STRUCTURAL_FALLBACK))
            {
                state.original_chain = None;
                self.relations.session_mut(comparer.session).error = state.saved_error.clone();
                return Ok(None);
            }
            let covariant_void = variances.iter().zip(target).any(|(&variance, &target)| {
                variance & VarianceFlags::VARIANCE_MASK == VarianceFlags::COVARIANT
                    && self.model.has_type_flags(target, TypeFlags::VOID)
            });
            state.variance_failed = !covariant_void;
            if !variances.is_empty() && !covariant_void {
                if state.variance_failed
                    && !(comparer.session.report
                        && variances.iter().any(|&variance| {
                            variance & VarianceFlags::VARIANCE_MASK == VarianceFlags::INVARIANT
                        }))
                {
                    return Ok(Some(Ternary::False));
                }
                state.original_chain = self.relations.session(comparer.session).error.chain.clone();
                self.relations.session_mut(comparer.session).error = state.saved_error.clone();
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `structuredTypeRelatedToWorker`'s alias variance branch is shared by
    /// identity and ordinary relations, including marker bypass and defaults.
    fn relation_alias_variances_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        state: &mut StructuredComparison,
        consumer: &Location,
    ) -> NativeResult<Option<Ternary>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Ternary>, DemandError> {
            if !self
                .model
                .has_type_flags(source, TypeFlags::OBJECT | TypeFlags::CONDITIONAL)
            {
                return Ok(None);
            }
            let source_alias = self.model.metadata(source)?.alias;
            let target_alias = self.model.metadata(target)?.alias;
            if let (Some(source_alias), Some(target_alias)) = (source_alias, target_alias)
                && source_alias
                    .arguments
                    .as_ref()
                    .is_some_and(|arguments| !arguments.is_empty())
                && source_alias.symbol == target_alias.symbol
                && !self.relations.marker_types.contains(&source)
                && !self.relations.marker_types.contains(&target)
            {
                let variances = sequence
                    .read(self.alias_variances_completion(source_alias.symbol, consumer))?;
                if variances.is_empty() {
                    return Ok(Some(Ternary::Unknown));
                }
                let parameters = sequence
                    .read(self.alias_parameters_completion(source_alias.symbol, consumer))?;
                let source_arguments = sequence.read(
                    self.fill_missing_type_arguments_completion(
                        source_alias
                            .arguments
                            .as_ref()
                            .map(|args| args.to_vec())
                            .unwrap_or_default(),
                        &parameters,
                        consumer,
                    ),
                )?;
                let target_arguments = sequence.read(
                    self.fill_missing_type_arguments_completion(
                        target_alias
                            .arguments
                            .as_ref()
                            .map(|args| args.to_vec())
                            .unwrap_or_default(),
                        &parameters,
                        consumer,
                    ),
                )?;
                return sequence.read(self.relate_variances_completion(
                    &source_arguments,
                    &target_arguments,
                    &variances,
                    state,
                    consumer,
                ));
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// Same-target reference variance is bypassed for actual marker
    /// instantiations, which intentionally undergo structural comparison.
    fn relation_reference_variances_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        state: &mut StructuredComparison,
        consumer: &Location,
    ) -> NativeResult<ReferenceRelation> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<ReferenceRelation, DemandError> {
            if let (TypeView::Reference(source_ref), TypeView::Reference(target_ref)) =
                (self.model.get(source)?, self.model.get(target)?)
            {
                if source_ref.is_reference
                    && target_ref.is_reference
                    && source_ref.target == target_ref.target
                    && !self.model.is_tuple_type(source)
                    && !self.relations.marker_types.contains(&source)
                    && !self.relations.marker_types.contains(&target)
                {
                    if self.is_empty_array_literal_type(source)? {
                        return Ok(ReferenceRelation::Related(Ternary::True));
                    }
                    let variances = sequence
                        .read(self.reference_variances_completion(source_ref.target, consumer))?;
                    if variances.is_empty() {
                        return Ok(ReferenceRelation::Related(Ternary::Unknown));
                    }
                    let source_arguments =
                        sequence.read(self.get_type_arguments_completion(source))?;
                    let target_arguments =
                        sequence.read(self.get_type_arguments_completion(target))?;
                    return sequence
                        .read(self.relate_variances_completion(
                            &source_arguments,
                            &target_arguments,
                            &variances,
                            state,
                            consumer,
                        ))
                        .map(|result| match result {
                            Some(result) => ReferenceRelation::Related(result),
                            None => ReferenceRelation::Structural,
                        });
                }
            }
            Ok(ReferenceRelation::NotApplicable)
        })();
        sequence.finish(result)
    }

    /// Array alternative of `structuredTypeRelatedToWorker`'s reference
    /// switch, shared by identity's immediate False and other relations'
    /// numeric-index comparison. EveryType decomposes unions only.
    fn array_relation_candidate_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
    ) -> NativeResult<bool> {
        let result = (|| -> Result<bool, DemandError> {
            if !self.is_array_type(target)? {
                return Ok(false);
            }
            let sources = self
                .model
                .union_constituents(source)
                .map(|types| types.to_vec())
                .unwrap_or_else(|| vec![source]);
            if self.is_readonly_array_type(target)? {
                let mut all = true;
                for &ty in &sources {
                    if !self.is_array_or_tuple_type(ty)? {
                        all = false;
                        break;
                    }
                }
                if all {
                    return Ok(true);
                }
            }
            for &ty in &sources {
                if self
                    .model
                    .tuple_target(ty)?
                    .is_none_or(|tuple| tuple.readonly)
                {
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        result.map(NativeCompletion::clean)
    }

    /// Generic-tuple alternative of `structuredTypeRelatedToWorker`'s
    /// reference switch. `Some` records a selected switch case even when its
    /// constraint is unchanged, so later alternatives cannot acquire members.
    fn tuple_relation_constraint_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            if self.model.is_generic_tuple_type(source)?
                && self.model.is_tuple_type(target)
                && !self.model.is_generic_tuple_type(target)?
            {
                return Ok(Some(
                    sequence
                        .read(self.base_constraint_completion(source, consumer))?
                        .unwrap_or(source),
                ));
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `structuredTypeRelatedToWorker` tests single variadic tuple shortcuts
    /// after alias variance for every relation, including Identity. Mutable
    /// targets acquire the source constraint even when a nested identity fails.
    fn single_generic_tuple_relation_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Option<Ternary>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Ternary>, DemandError> {
            if let Some(tuple) = self.model.tuple_target(source)?
                && tuple.elements.len() == 1
                && tuple
                    .combined_flags
                    .intersects(super::super::model::TupleElementFlags::VARIADIC)
                && !tuple.readonly
            {
                let arguments = sequence.read(self.get_type_arguments_completion(source))?;
                let related = sequence.read(self.related_in_session_completion(
                    arguments[0],
                    target,
                    RecursionFlags::SOURCE,
                    IntersectionState::NONE,
                    session,
                    consumer,
                ))?;
                if related != Ternary::False {
                    return Ok(Some(related));
                }
            }
            if let Some(tuple) = self.model.tuple_target(target)?
                && tuple.elements.len() == 1
                && tuple
                    .combined_flags
                    .intersects(super::super::model::TupleElementFlags::VARIADIC)
            {
                let mutable = if tuple.readonly {
                    true
                } else {
                    let constrained = sequence
                        .read(self.base_constraint_completion(source, consumer))?
                        .unwrap_or(source);
                    self.is_mutable_array_or_tuple(constrained)?
                };
                if mutable {
                    let arguments = sequence.read(self.get_type_arguments_completion(target))?;
                    let related = sequence.read(self.related_in_session_completion(
                        source,
                        arguments[0],
                        RecursionFlags::TARGET,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    if related != Ternary::False {
                        return Ok(Some(related));
                    }
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `typeArgumentsRelatedTo` preserves native direction and Ternary
    /// accumulation. Unknown recursive variance defaults to covariance.
    fn type_arguments_related_completion(
        &mut self,
        source: &[TypeId],
        target: &[TypeId],
        variances: &[VarianceFlags],
        comparer: RelationComparer,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let identity = self.relations.session(comparer.session).kind == RelationKind::Identity;
            if identity && source.len() != target.len() {
                return Ok(Ternary::False);
            }
            let mut result = Ternary::True;
            for (index, (&source, &target)) in source.iter().zip(target).enumerate() {
                let flags = variances
                    .get(index)
                    .copied()
                    .unwrap_or(VarianceFlags::COVARIANT);
                let variance = flags & VarianceFlags::VARIANCE_MASK;
                if variance == VarianceFlags::INDEPENDENT {
                    continue;
                }
                let related = if flags.intersects(VarianceFlags::UNMEASURABLE) {
                    if identity {
                        sequence.read(self.related_in_session_completion(
                            source,
                            target,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            comparer.session,
                            consumer,
                        ))?
                    } else if sequence.read(self.raw_type_related_completion(
                        source,
                        target,
                        RelationKind::Identity,
                        consumer,
                    ))? {
                        Ternary::True
                    } else {
                        Ternary::False
                    }
                } else {
                    if self.relations.in_variance && flags.intersects(VarianceFlags::UNRELIABLE) {
                        let mapper = self.model.report_reliability_mapper(false);
                        self.instantiate_type(source, mapper, consumer)?;
                    }
                    if variance == VarianceFlags::CONTRAVARIANT {
                        sequence.read(self.related_in_session_completion(
                            target,
                            source,
                            RecursionFlags::BOTH,
                            comparer.intersection,
                            comparer.session,
                            consumer,
                        ))?
                    } else if variance == VarianceFlags::BIVARIANT {
                        let related = sequence.read(self.related_in_session_completion(
                            target,
                            source,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            comparer.session.reporting(false),
                            consumer,
                        ))?;
                        if related != Ternary::False {
                            related
                        } else {
                            sequence.read(self.related_in_session_completion(
                                source,
                                target,
                                RecursionFlags::BOTH,
                                comparer.intersection,
                                comparer.session,
                                consumer,
                            ))?
                        }
                    } else {
                        let related = sequence.read(self.related_in_session_completion(
                            source,
                            target,
                            RecursionFlags::BOTH,
                            comparer.intersection,
                            comparer.session,
                            consumer,
                        ))?;
                        if related != Ternary::False && variance != VarianceFlags::COVARIANT {
                            related.and(sequence.read(self.related_in_session_completion(
                                target,
                                source,
                                RecursionFlags::BOTH,
                                comparer.intersection,
                                comparer.session,
                                consumer,
                            ))?)
                        } else {
                            related
                        }
                    }
                };
                if related == Ternary::False {
                    return Ok(related);
                }
                result = result.and(related);
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `InferenceContext.compareTypes`: only False rejects a constraint;
    /// provisional Maybe remains in the original relation invocation.
    pub(super) fn inference_constraint_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        comparer: RelationComparer,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if !self
                .relations
                .active
                .contains_key(&comparer.session.session)
            {
                return Err(super::super::internal(
                    "uncached inference comparison escaped its native relation invocation",
                )
                .into());
            }
            Ok(sequence.read(self.related_in_session_completion(
                source,
                target,
                RecursionFlags::BOTH,
                comparer.intersection,
                comparer.session.reporting(false),
                consumer,
            ))? != Ternary::False)
        })();
        sequence.finish(result)
    }

    /// `isTypeAssignableToKindEx` / `isTypeAssignableToKind`: exact flag
    /// overlap and strict exclusions precede the ordered intrinsic relations.
    /// These are relation dependencies, independently of diagnostic emission.
    pub(super) fn type_assignable_to_kind_completion(
        &mut self,
        source: TypeId,
        kind: TypeFlags,
        strict: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if self.model.has_type_flags(source, kind) {
                return Ok(true);
            }
            if strict
                && self.model.has_type_flags(
                    source,
                    TypeFlags::ANY | TypeFlags::UNKNOWN | TypeFlags::VOID | TypeFlags::NULLABLE,
                )
            {
                return Ok(false);
            }
            for (flags, primitive, special) in [
                (TypeFlags::NUMBER_LIKE, Some(PrimitiveName::Number), None),
                (TypeFlags::BIGINT_LIKE, Some(PrimitiveName::Bigint), None),
                (TypeFlags::STRING_LIKE, Some(PrimitiveName::String), None),
                (TypeFlags::BOOLEAN_LIKE, Some(PrimitiveName::Boolean), None),
                (TypeFlags::VOID, None, Some(SpecialName::Void)),
                (TypeFlags::NEVER, None, Some(SpecialName::Never)),
                (TypeFlags::NULL, None, Some(SpecialName::Null)),
                (TypeFlags::UNDEFINED, None, Some(SpecialName::Undefined)),
                (TypeFlags::SYMBOL, Some(PrimitiveName::Symbol), None),
                (TypeFlags::NON_PRIMITIVE, Some(PrimitiveName::Object), None),
            ] {
                if kind.intersects(flags) {
                    let target = match (primitive, special) {
                        (Some(name), None) => self.model.primitive(name),
                        (None, Some(name)) => self.model.special(name),
                        _ => unreachable!("native kind relation has one intrinsic target"),
                    };
                    if sequence.read(self.raw_type_related_completion(
                        source,
                        target,
                        RelationKind::Assignable,
                        consumer,
                    ))? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `allTypesAssignableToKindEx`: a union asks every constituent in order;
    /// other types use the same flag and relation operation as a scalar call.
    pub(super) fn all_types_assignable_to_kind_completion(
        &mut self,
        source: TypeId,
        kind: TypeFlags,
        strict: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if let Some(types) = self.model.union_constituents(source) {
                for &ty in types.iter() {
                    if !sequence.read(
                        self.all_types_assignable_to_kind_completion(ty, kind, strict, consumer),
                    )? {
                        return Ok(false);
                    }
                }
                Ok(true)
            } else {
                sequence
                    .read(self.type_assignable_to_kind_completion(source, kind, strict, consumer))
            }
        })();
        sequence.finish(result)
    }

    /// `isTypeRelatedTo` keeps literal, identity, simple and installed object
    /// cache answers ahead of a new `checkTypeRelatedToEx` session. Product
    /// overflow provenance survives the native bool/cache representation.
    pub(super) fn type_related(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.type_related_completion(source, target, relation, consumer)?
            .admit()
    }

    pub(super) fn type_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        consumer: &Location,
    ) -> NativeResult<bool> {
        Ok(self
            .raw_type_related_completion(source, target, relation, consumer)
            .map_err(|failure| super::super::observe_recovery(self.inputs, failure, consumer))?
            .observe(|failure| super::super::observe_recovery(self.inputs, failure, consumer)))
    }

    pub(super) fn raw_type_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let source = self.regular_relation_literal(source);
            let target = self.regular_relation_literal(target);
            if source == target {
                return Ok(true);
            }
            if let Some(related) = self.plain_string_literals_related(source, target, consumer)? {
                return Ok(related);
            }
            if relation == RelationKind::Identity {
                let simplifiable = TypeFlags::UNION
                    | TypeFlags::INTERSECTION
                    | TypeFlags::INDEXED_ACCESS
                    | TypeFlags::CONDITIONAL
                    | TypeFlags::SUBSTITUTION;
                if !self.model.has_type_flags(source, simplifiable)
                    && !self.model.has_type_flags(target, simplifiable)
                {
                    if self.model.type_flags(source) != self.model.type_flags(target) {
                        return Ok(false);
                    }
                    if self.model.has_type_flags(source, TypeFlags::SINGLETON) {
                        return Ok(true);
                    }
                }
            } else if relation == RelationKind::Comparable
                && !self.model.has_type_flags(target, TypeFlags::NEVER)
                && sequence.read(
                    self.simple_type_related_completion(target, source, relation, None, consumer),
                )?
                || sequence.read(
                    self.simple_type_related_completion(source, target, relation, None, consumer),
                )?
            {
                return Ok(true);
            }
            if self.model.has_type_flags(source, TypeFlags::OBJECT)
                && self.model.has_type_flags(target, TypeFlags::OBJECT)
            {
                let (key, _) = sequence.read(self.relation_key_completion(
                    source,
                    target,
                    IntersectionState::NONE,
                    relation,
                    false,
                    consumer,
                ))?;
                if let Some(entry) = self
                    .relations
                    .caches
                    .get(&relation)
                    .and_then(|cache| cache.get(&key))
                    .cloned()
                {
                    return sequence.read(Ok(entry.result));
                }
            }
            if !self
                .model
                .has_type_flags(source, TypeFlags::STRUCTURED_OR_INSTANTIABLE)
                && !self
                    .model
                    .has_type_flags(target, TypeFlags::STRUCTURED_OR_INSTANTIABLE)
            {
                return Ok(false);
            }
            let session = self.relations.begin(relation);
            let result = self.check_relation_session_completion(source, target, session, consumer);
            self.relations.active.remove(&session.session);
            sequence.read(result).map(|check| check.related)
        })();
        sequence.finish(result)
    }

    /// `checkTypeRelatedToEx` is the direct entry used by checking operations;
    /// unlike isTypeRelatedTo it does not apply the outer quick/cache wrapper.
    pub(super) fn check_type_related_to_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        error: Option<ErrorContext>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        Ok(self
            .raw_check_type_related_to_completion(source, target, relation, error, consumer)
            .map_err(|failure| super::super::observe_recovery(self.inputs, failure, consumer))?
            .observe(|failure| super::super::observe_recovery(self.inputs, failure, consumer)))
    }

    pub(super) fn raw_check_type_related_to_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        error: Option<ErrorContext>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        Ok(self
            .raw_check_type_related_to_outcome_completion(
                source, target, relation, error, consumer,
            )?
            .map(|check| check.related))
    }

    pub(super) fn check_type_related_to_outcome_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        error: Option<ErrorContext>,
        consumer: &Location,
    ) -> NativeResult<RelationCheck> {
        Ok(self
            .raw_check_type_related_to_outcome_completion(source, target, relation, error, consumer)
            .map_err(|failure| super::super::observe_recovery(self.inputs, failure, consumer))?
            .observe(|failure| super::super::observe_recovery(self.inputs, failure, consumer)))
    }

    pub(super) fn raw_check_type_related_to_outcome_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        error: Option<ErrorContext>,
        consumer: &Location,
    ) -> NativeResult<RelationCheck> {
        let mut sequence = NativeSequence::new();
        let result = {
            let mut session = self.relations.begin(relation);
            if let Some(error) = error {
                self.relations.session_mut(session).error_node = Some(error.node);
                session.report = true;
                session.head = error.head;
            }
            let result = self.check_relation_session_completion(source, target, session, consumer);
            self.relations.active.remove(&session.session);
            sequence.read(result)
        };
        sequence.finish(result)
    }

    /// `checkTypeRelatedToEx` owns root overflow installation. Its caller
    /// retires the session on every exit, including failed dependency reads.
    fn check_relation_session_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<RelationCheck> {
        let relation = self.relations.session(session).kind;
        // An interrupted comparison has no next native operation. Earlier
        // recursive slots stay installed, but the root key/write/display have
        // not yet happened. The actual overflow False producer already carries
        // its failure in acquisition order, including through cache replay.
        let result = self.related_in_session_completion(
            source,
            target,
            RecursionFlags::BOTH,
            IntersectionState::NONE,
            session,
            consumer,
        )?;
        if let Some(overflow) = self.relations.session(session).overflow {
            let installed = result.clone().map(|_| false);
            return result.and_then(|result| {
                self.relation_key_completion(
                    source,
                    target,
                    IntersectionState::NONE,
                    relation,
                    false,
                    consumer,
                )?
                .and_then(|(key, _)| {
                    // Only this native root write adds Overflow flags.
                    // Later diagnostic work remains invocation evidence.
                    self.relations.store(
                        relation,
                        key,
                        RelationEntry {
                            result: installed,
                            overflow: Some(overflow),
                            reliability: VarianceFlags::INVARIANT,
                        },
                    );
                    Ok(self
                        .report_relation_overflow_completion(
                            source, target, overflow, session, consumer,
                        )?
                        .map(|()| RelationCheck {
                            related: result != Ternary::False,
                            diagnostics: 1,
                        }))
                })
            });
        }
        result.and_then(|result| {
            // Helpful-import retry is a fresh silent native relater. A restored
            // diagnostic chain does not remove the work of either invocation.
            let mut sequence = NativeSequence::new();
            let value = (|| {
                let state = self.relations.session(session);
                if state.error.chain.is_some()
                    && session.head.is_some()
                    && state.error_node.is_some()
                    && result == Ternary::False
                    && let Some(links) = self.type_export_links(source)?
                {
                    let declaration = self
                        .source
                        .type_node_declaration(links.originating_import)?;
                    if !self.source.declaration_is_import_call(declaration)? {
                        let source = sequence
                            .read(self.value_reference_type_completion(links.target, consumer))?;
                        sequence.read(self.raw_check_type_related_to_completion(
                            source, target, relation, None, consumer,
                        ))?;
                    }
                }
                Ok(RelationCheck {
                    related: result != Ternary::False,
                    diagnostics: usize::from(
                        self.relations.session(session).error.produces_diagnostic(),
                    ),
                })
            })();
            sequence.finish(value)
        })
    }

    /// `areTypesComparable` is ordered forward-or-reverse Comparable.
    pub(super) fn are_types_comparable_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if sequence.read(self.raw_type_related_completion(
                source,
                target,
                RelationKind::Comparable,
                consumer,
            ))? {
                Ok(true)
            } else {
                sequence.read(self.raw_type_related_completion(
                    target,
                    source,
                    RelationKind::Comparable,
                    consumer,
                ))
            }
        })();
        sequence.finish(result)
    }

    /// `isTypeRelatedTo` / `isRelatedToEx` normally decide plain string-literal
    /// equality through native interning. Symbolic spellings require the same
    /// semantic observation when unequal TypeIds alone cannot decide it. Enum
    /// literals keep their directional symbol rule in isSimpleTypeRelatedTo.
    fn plain_string_literals_related(
        &self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> Result<Option<bool>, DemandError> {
        if self.model.has_type_flags(source, TypeFlags::STRING_LITERAL)
            && self.model.has_type_flags(target, TypeFlags::STRING_LITERAL)
            && !self.model.has_type_flags(source, TypeFlags::ENUM_LITERAL)
            && !self.model.has_type_flags(target, TypeFlags::ENUM_LITERAL)
        {
            return self
                .model
                .literal_value_equal(source, target)
                .map(Some)
                .map_err(|error| self.literal_value_failure(error, consumer));
        }
        Ok(None)
    }

    fn regular_relation_literal(&mut self, ty: TypeId) -> TypeId {
        if self.model.is_fresh_literal(ty) {
            self.model.regular_literal(ty)
        } else {
            ty
        }
    }

    /// `isSimpleTypeRelatedTo`: whole branches for current intrinsic/literal/
    /// ordinary object factories, with enum comparison delegated to its
    /// directional symbol cache and optional native diagnostic reporter.
    fn simple_type_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        relation: RelationKind,
        reporter: Option<RelationInvocation>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if self.model.has_type_flags(target, TypeFlags::ANY)
                || self.model.has_type_flags(source, TypeFlags::NEVER)
            {
                return Ok(true);
            }
            if self.model.has_type_flags(target, TypeFlags::UNKNOWN)
                && !(relation == RelationKind::StrictSubtype
                    && self.model.has_type_flags(source, TypeFlags::ANY))
            {
                return Ok(true);
            }
            if self.model.has_type_flags(target, TypeFlags::NEVER) {
                return Ok(false);
            }
            for (source_flags, target_flags) in [
                (TypeFlags::STRING_LIKE, TypeFlags::STRING),
                (TypeFlags::NUMBER_LIKE, TypeFlags::NUMBER),
                (TypeFlags::BIGINT_LIKE, TypeFlags::BIGINT),
                (TypeFlags::BOOLEAN_LIKE, TypeFlags::BOOLEAN),
                (TypeFlags::ES_SYMBOL_LIKE, TypeFlags::SYMBOL),
            ] {
                if self.model.has_type_flags(source, source_flags)
                    && self.model.has_type_flags(target, target_flags)
                {
                    return Ok(true);
                }
            }
            if self.model.has_type_flags(source, TypeFlags::ENUM_LITERAL)
                && !self.model.has_type_flags(target, TypeFlags::ENUM_LITERAL)
                && self
                    .model
                    .literal_value_equal(source, target)
                    .map_err(|error| self.literal_value_failure(error, consumer))?
            {
                return Ok(true);
            }
            if self.model.has_type_flags(source, TypeFlags::ENUM_LIKE)
                && self.model.has_type_flags(target, TypeFlags::ENUM_LIKE)
            {
                let compared = self.model.has_type_flags(source, TypeFlags::ENUM)
                    && self.model.has_type_flags(target, TypeFlags::ENUM)
                    || self.model.has_type_flags(source, TypeFlags::ENUM_LITERAL)
                        && self.model.has_type_flags(target, TypeFlags::ENUM_LITERAL)
                        && (self.model.has_type_flags(source, TypeFlags::UNION)
                            && self.model.has_type_flags(target, TypeFlags::UNION)
                            || self
                                .model
                                .literal_value_equal(source, target)
                                .map_err(|error| self.literal_value_failure(error, consumer))?);
                if compared {
                    let source_symbol = self
                        .reference_symbol_of_type(source, consumer)?
                        .ok_or_else(|| super::super::internal("enum source type lacks symbol"))?;
                    let target_symbol = self
                        .reference_symbol_of_type(target, consumer)?
                        .ok_or_else(|| super::super::internal("enum target type lacks symbol"))?;
                    if (!self.model.has_type_flags(source, TypeFlags::ENUM)
                        || self.reference_symbol_key(source_symbol, consumer)?
                            == self.reference_symbol_key(target_symbol, consumer)?)
                        && sequence.read(self.enum_type_related_completion(
                            source_symbol,
                            target_symbol,
                            reporter,
                            consumer,
                        ))?
                    {
                        return Ok(true);
                    }
                }
            }
            let strict = self.inputs.semantic_options().strict_null_checks;
            if self.model.has_type_flags(source, TypeFlags::UNDEFINED)
                && (!strict
                    && !self
                        .model
                        .has_type_flags(target, TypeFlags::UNION | TypeFlags::INTERSECTION)
                    || self
                        .model
                        .has_type_flags(target, TypeFlags::UNDEFINED | TypeFlags::VOID))
            {
                return Ok(true);
            }
            if self.model.has_type_flags(source, TypeFlags::NULL)
                && (!strict
                    && !self
                        .model
                        .has_type_flags(target, TypeFlags::UNION | TypeFlags::INTERSECTION)
                    || self.model.has_type_flags(target, TypeFlags::NULL))
            {
                return Ok(true);
            }
            if self.model.has_type_flags(source, TypeFlags::OBJECT)
                && self.model.has_type_flags(target, TypeFlags::NON_PRIMITIVE)
            {
                if relation == RelationKind::StrictSubtype
                    && self.is_empty_anonymous_object_type(source, consumer)?
                    && !self
                        .model
                        .object_flags(source)?
                        .intersects(ObjectFlags::FRESH_LITERAL)
                {
                    return Ok(false);
                }
                return Ok(true);
            }
            if matches!(
                relation,
                RelationKind::Assignable | RelationKind::Comparable
            ) {
                if self.model.has_type_flags(source, TypeFlags::ANY) {
                    return Ok(true);
                }
                let numeric_enum = self.model.has_type_flags(target, TypeFlags::ENUM)
                    || self.model.has_type_flags(target, TypeFlags::NUMBER_LITERAL)
                        && self.model.has_type_flags(target, TypeFlags::ENUM_LITERAL);
                if self.model.has_type_flags(source, TypeFlags::NUMBER) && numeric_enum {
                    return Ok(true);
                }
                if self.model.has_type_flags(source, TypeFlags::NUMBER_LITERAL)
                    && !self.model.has_type_flags(source, TypeFlags::ENUM_LITERAL)
                    && (self.model.has_type_flags(target, TypeFlags::ENUM)
                        || numeric_enum
                            && self
                                .model
                                .literal_value_equal(source, target)
                                .map_err(|error| self.literal_value_failure(error, consumer))?)
                {
                    return Ok(true);
                }
                if sequence.read(self.unknown_like_union_completion(target, consumer))? {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isUnknownLikeUnionType`: exact intrinsic constituent flags and the
    /// existing complete IsEmptyAnonymousObjectType owner, not printed shape.
    fn unknown_like_union_completion(
        &mut self,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let result = (|| -> Result<bool, DemandError> {
            if !self.inputs.semantic_options().strict_null_checks {
                return Ok(false);
            }
            let Some(types) = self.model.union_constituents(target) else {
                return Ok(false);
            };
            if types.len() < 3
                || !self.model.has_type_flags(types[0], TypeFlags::UNDEFINED)
                || !self.model.has_type_flags(types[1], TypeFlags::NULL)
            {
                return Ok(false);
            }
            for &ty in types.iter() {
                if self.is_empty_anonymous_object_type(ty, consumer)? {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        result.map(NativeCompletion::clean)
    }

    /// `unionOrIntersectionRelatedTo` preserves ordered each/some questions
    /// inside the active session. Composite normalization and discriminant
    /// fallback stay distinct operations from these ordinary union branches.
    fn union_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        intersection: IntersectionState,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            if let Some(sources) = self.model.union_constituents(source) {
                let session = session.reporting(
                    session.report && !self.model.has_type_flags(source, TypeFlags::PRIMITIVE),
                );
                if self.model.has_type_flags(target, TypeFlags::UNION) {
                    if let Some(origin) = self.model.union_origin(source)
                        && let Some(types) = self.model.intersection_constituents(origin)
                        && self.model.metadata(target)?.alias.is_some()
                        && types.contains(&target)
                    {
                        return Ok(Ternary::True);
                    }
                    if let Some(origin) = self.model.union_origin(target)
                        && let Some(types) = self.model.union_constituents(origin)
                        && self.model.metadata(source)?.alias.is_some()
                        && types.contains(&source)
                    {
                        return Ok(Ternary::True);
                    }
                }
                if self.relations.session(session).kind == RelationKind::Comparable {
                    if sources.contains(&target) {
                        return Ok(Ternary::True);
                    }
                    for (index, &ty) in sources.iter().enumerate() {
                        let result = sequence.read(self.related_in_session_completion(
                            ty,
                            target,
                            RecursionFlags::SOURCE,
                            intersection,
                            session.reporting(session.report && index + 1 == sources.len()),
                            consumer,
                        ))?;
                        if result != Ternary::False {
                            return Ok(result);
                        }
                    }
                    return Ok(Ternary::False);
                }
                let mut stripped = target;
                if let Some(targets) = self.model.union_constituents(target)
                    && !self.model.has_type_flags(sources[0], TypeFlags::UNDEFINED)
                    && self.model.has_type_flags(targets[0], TypeFlags::UNDEFINED)
                {
                    stripped = self.model.filter_undefined(target);
                }
                let corresponding = self.model.union_constituents(stripped);
                let mut result = Ternary::True;
                for (index, &ty) in sources.iter().enumerate() {
                    if let Some(targets) = &corresponding
                        && sources.len() >= targets.len()
                        && sources.len() % targets.len() == 0
                    {
                        let related = sequence.read(self.related_in_session_completion(
                            ty,
                            targets[index % targets.len()],
                            RecursionFlags::BOTH,
                            intersection,
                            session.reporting(false),
                            consumer,
                        ))?;
                        if related != Ternary::False {
                            result = result.and(related);
                            continue;
                        }
                    }
                    let related = sequence.read(self.related_in_session_completion(
                        ty,
                        target,
                        RecursionFlags::SOURCE,
                        intersection,
                        session,
                        consumer,
                    ))?;
                    if related == Ternary::False {
                        return Ok(related);
                    }
                    result = result.and(related);
                }
                return Ok(result);
            }
            if let Some(targets) = self.model.union_constituents(target) {
                let source = self.regular_object_literal_type(source, consumer)?;
                let session = session.reporting(
                    session.report
                        && !self.model.has_type_flags(source, TypeFlags::PRIMITIVE)
                        && !self.model.has_type_flags(target, TypeFlags::PRIMITIVE),
                );
                if targets.contains(&source) {
                    return Ok(Ternary::True);
                }
                if self.relations.session(session).kind != RelationKind::Comparable
                    && self
                        .model
                        .object_flags(target)?
                        .intersects(ObjectFlags::PRIMITIVE_UNION)
                    && !self.model.has_type_flags(source, TypeFlags::ENUM_LITERAL)
                    && (self.model.has_type_flags(
                        source,
                        TypeFlags::STRING_LITERAL
                            | TypeFlags::BOOLEAN_LITERAL
                            | TypeFlags::BIGINT_LITERAL,
                    ) || matches!(
                        self.relations.session(session).kind,
                        RelationKind::Subtype | RelationKind::StrictSubtype
                    ) && self.model.has_type_flags(source, TypeFlags::NUMBER_LITERAL))
                {
                    let alternate = self.model.alternate_literal_type(source);
                    let primitive = if self.model.has_type_flags(source, TypeFlags::STRING_LITERAL)
                    {
                        Some(PrimitiveName::String)
                    } else if self.model.has_type_flags(source, TypeFlags::NUMBER_LITERAL) {
                        Some(PrimitiveName::Number)
                    } else if self.model.has_type_flags(source, TypeFlags::BIGINT_LITERAL) {
                        Some(PrimitiveName::Bigint)
                    } else {
                        None
                    };
                    let primitive = primitive.map(|name| self.model.primitive(name));
                    return Ok(
                        if primitive.is_some_and(|ty| targets.contains(&ty))
                            || alternate.is_some_and(|ty| targets.contains(&ty))
                        {
                            Ternary::True
                        } else {
                            Ternary::False
                        },
                    );
                }
                if let Some(matched) = self.matching_union_constituent(target, source, consumer)? {
                    let result = sequence.read(self.related_in_session_completion(
                        source,
                        matched,
                        RecursionFlags::TARGET,
                        intersection,
                        session.reporting(false),
                        consumer,
                    ))?;
                    if result != Ternary::False {
                        return Ok(result);
                    }
                }
                for &ty in targets.iter() {
                    let result = sequence.read(self.related_in_session_completion(
                        source,
                        ty,
                        RecursionFlags::TARGET,
                        intersection,
                        session.reporting(false),
                        consumer,
                    ))?;
                    if result != Ternary::False {
                        return Ok(result);
                    }
                }
                if session.report
                    && !self.model.has_type_flags(source, TypeFlags::PRIMITIVE)
                    && !self.model.has_type_flags(target, TypeFlags::PRIMITIVE)
                    && let Some(best) = self.best_matching_type(
                        source,
                        target,
                        |checker, source, target| {
                            Ok(sequence.read(checker.related_in_session_completion(
                                source,
                                target,
                                RecursionFlags::BOTH,
                                IntersectionState::NONE,
                                session.reporting(false),
                                consumer,
                            ))? != Ternary::False)
                        },
                        consumer,
                    )?
                {
                    sequence.read(self.related_in_session_completion(
                        source,
                        best,
                        RecursionFlags::TARGET,
                        intersection,
                        session.without_head(),
                        consumer,
                    ))?;
                }
                return Ok(Ternary::False);
            }
            if let Some(targets) = self.model.intersection_constituents(target) {
                let mut result = Ternary::True;
                for &ty in targets.iter() {
                    let related = sequence.read(self.related_in_session_completion(
                        source,
                        ty,
                        RecursionFlags::TARGET,
                        IntersectionState::TARGET,
                        session,
                        consumer,
                    ))?;
                    if related == Ternary::False {
                        return Ok(related);
                    }
                    result = result.and(related);
                }
                return Ok(result);
            }
            let mut source = source;
            let mut sources = self
                .model
                .intersection_constituents(source)
                .ok_or_else(|| {
                    super::super::internal("composite relation requires a union or intersection")
                })?;
            if self.relations.session(session).kind == RelationKind::Comparable
                && self.model.has_type_flags(target, TypeFlags::PRIMITIVE)
            {
                let mut constraints = Vec::with_capacity(sources.len());
                for &ty in sources.iter() {
                    constraints.push(if self.model.has_type_flags(ty, TypeFlags::INSTANTIABLE) {
                        match sequence.read(self.base_constraint_completion(ty, consumer))? {
                            Some(constraint) => constraint,
                            None => self.model.special(SpecialName::Unknown),
                        }
                    } else {
                        ty
                    });
                }
                if constraints.as_slice() != sources.as_ref() {
                    source = self.intersection_type(constraints, consumer)?;
                    if self.model.has_type_flags(source, TypeFlags::NEVER) {
                        return Ok(Ternary::False);
                    }
                    if let Some(types) = self.model.intersection_constituents(source) {
                        sources = types;
                    } else {
                        let related = sequence.read(self.related_in_session_completion(
                            source,
                            target,
                            RecursionFlags::SOURCE,
                            IntersectionState::NONE,
                            session.reporting(false),
                            consumer,
                        ))?;
                        if related != Ternary::False {
                            return Ok(related);
                        }
                        return sequence.read(self.related_in_session_completion(
                            target,
                            source,
                            RecursionFlags::SOURCE,
                            IntersectionState::NONE,
                            session.reporting(false),
                            consumer,
                        ));
                    }
                }
            }
            for &ty in sources.iter() {
                let related = sequence.read(self.related_in_session_completion(
                    ty,
                    target,
                    RecursionFlags::SOURCE,
                    IntersectionState::SOURCE,
                    session.reporting(false),
                    consumer,
                ))?;
                if related != Ternary::False {
                    return Ok(related);
                }
            }
            Ok(Ternary::False)
        })();
        sequence.finish(result)
    }

    /// `isRelatedToEx` owns normalization, native simple answers, excess/weak
    /// checks and the small-union cache bypass before recursive comparison.
    fn related_in_session_completion(
        &mut self,
        original_source: TypeId,
        original_target: TypeId,
        recursion: RecursionFlags,
        intersection: IntersectionState,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let head_message = session.head;
            let session = session.without_head();
            if original_source == original_target {
                return Ok(Ternary::True);
            }
            if self.relations.session(session).kind != RelationKind::Identity
                && self
                    .model
                    .has_type_flags(original_source, TypeFlags::OBJECT)
                && self
                    .model
                    .has_type_flags(original_target, TypeFlags::PRIMITIVE)
            {
                let related = if self.relations.session(session).kind == RelationKind::Comparable
                    && !self.model.has_type_flags(original_target, TypeFlags::NEVER)
                    && sequence.read(self.simple_type_related_completion(
                        original_target,
                        original_source,
                        self.relations.session(session).kind,
                        None,
                        consumer,
                    ))?
                    || sequence.read(self.simple_type_related_completion(
                        original_source,
                        original_target,
                        self.relations.session(session).kind,
                        session.report.then_some(session),
                        consumer,
                    ))? {
                    Ternary::True
                } else {
                    Ternary::False
                };
                if related == Ternary::False && session.report {
                    sequence.read(self.report_error_results_completion(
                        (original_source, original_target),
                        (original_source, original_target),
                        head_message,
                        session,
                        consumer,
                    ))?;
                }
                return Ok(related);
            }
            let source = sequence.read(self.normalized_relation_type_completion(
                original_source,
                false,
                consumer,
            ))?;
            let mut target = sequence.read(self.normalized_relation_type_completion(
                original_target,
                true,
                consumer,
            ))?;
            if source == target {
                return Ok(Ternary::True);
            }
            if self.relations.session(session).kind == RelationKind::Identity {
                if let Some(related) =
                    self.plain_string_literals_related(source, target, consumer)?
                {
                    return Ok(if related {
                        Ternary::True
                    } else {
                        Ternary::False
                    });
                }
                if self.model.type_flags(source) != self.model.type_flags(target) {
                    return Ok(Ternary::False);
                }
                if self.model.has_type_flags(source, TypeFlags::SINGLETON) {
                    return Ok(Ternary::True);
                }
                return sequence.read(self.recursive_relation_completion(
                    source,
                    target,
                    recursion,
                    IntersectionState::NONE,
                    session.reporting(false),
                    consumer,
                ));
            }
            if self.model.has_type_flags(source, TypeFlags::TYPE_PARAMETER)
                && sequence.read(self.constraint_of_type_completion(source, consumer))?
                    == Some(target)
            {
                return Ok(Ternary::True);
            }
            // Native DefinitelyNonNullable excludes Never, Any, Unknown and
            // type variables; these established categories carry the same mask.
            if self
                .model
                .has_type_flags(source, TypeFlags::DEFINITELY_NON_NULLABLE)
                && let Some(types) = self.model.union_constituents(target)
            {
                let candidate = match types.as_ref() {
                    [nullable, candidate]
                        if self.model.has_type_flags(*nullable, TypeFlags::NULLABLE) =>
                    {
                        Some(*candidate)
                    }
                    [first, second, candidate]
                        if self.model.has_type_flags(*first, TypeFlags::NULLABLE)
                            && self.model.has_type_flags(*second, TypeFlags::NULLABLE) =>
                    {
                        Some(*candidate)
                    }
                    _ => None,
                };
                if let Some(candidate) = candidate
                    && !self.model.has_type_flags(candidate, TypeFlags::NULLABLE)
                {
                    target = sequence.read(
                        self.normalized_relation_type_completion(candidate, true, consumer),
                    )?;
                    if source == target {
                        return Ok(Ternary::True);
                    }
                }
            }
            // Nullable-union normalization above may have exposed a literal target.
            if let Some(related) = self.plain_string_literals_related(source, target, consumer)? {
                if related {
                    return Ok(Ternary::True);
                }
                // Preserve the existing diagnostic schedule for unequal values.
            }
            if self.relations.session(session).kind == RelationKind::Comparable
                && !self.model.has_type_flags(target, TypeFlags::NEVER)
                && sequence.read(self.simple_type_related_completion(
                    target,
                    source,
                    self.relations.session(session).kind,
                    None,
                    consumer,
                ))?
                || sequence.read(self.simple_type_related_completion(
                    source,
                    target,
                    self.relations.session(session).kind,
                    session.report.then_some(session),
                    consumer,
                ))?
            {
                return Ok(Ternary::True);
            }
            if !self
                .model
                .has_type_flags(source, TypeFlags::STRUCTURED_OR_INSTANTIABLE)
                && !self
                    .model
                    .has_type_flags(target, TypeFlags::STRUCTURED_OR_INSTANTIABLE)
            {
                if session.report {
                    sequence.read(self.report_error_results_completion(
                        (original_source, original_target),
                        (source, target),
                        head_message,
                        session,
                        consumer,
                    ))?;
                }
                return Ok(Ternary::False);
            }
            if intersection.0 & IntersectionState::TARGET.0 == 0 {
                if self
                    .model
                    .object_flags(source)?
                    .contains(ObjectFlags::OBJECT_LITERAL | ObjectFlags::FRESH_LITERAL)
                    && sequence.read(
                        self.has_excess_properties_completion(source, target, session, consumer),
                    )?
                {
                    if session.report {
                        let error_target = if self.model.metadata(original_target)?.alias.is_some()
                        {
                            original_target
                        } else {
                            target
                        };
                        sequence.read(self.report_relation_error_completion(
                            head_message,
                            source,
                            error_target,
                            session,
                            consumer,
                        ))?;
                    }
                    return Ok(Ternary::False);
                }
                if (self.relations.session(session).kind != RelationKind::Comparable
                    || self.model.has_type_flags(source, TypeFlags::UNIT))
                    && self.model.has_type_flags(
                        source,
                        TypeFlags::PRIMITIVE | TypeFlags::OBJECT | TypeFlags::INTERSECTION,
                    )
                    && source != self.global_object_identity(consumer)?
                    && self
                        .model
                        .has_type_flags(target, TypeFlags::OBJECT | TypeFlags::INTERSECTION)
                    && sequence.read(self.weak_relation_target_completion(target, consumer))?
                    && (!self.properties_of_type(source, consumer)?.is_empty()
                        || !self.call_signatures(source, consumer)?.is_empty()
                        || !self.construct_signatures(source, consumer)?.is_empty())
                {
                    let jsx = self
                        .model
                        .object_flags(source)?
                        .intersects(ObjectFlags::JSX_ATTRIBUTES);
                    if !sequence.read(
                        self.has_common_properties_completion(source, target, jsx, consumer),
                    )? {
                        if session.report {
                            sequence.read(self.report_no_common_properties_completion(
                                source,
                                target,
                                (original_source, original_target),
                                session,
                                consumer,
                            ))?;
                        }
                        return Ok(Ternary::False);
                    }
                }
            }
            let skip_cache = self
                .model
                .union_constituents(source)
                .is_some_and(|types| types.len() < 4)
                && !self.model.has_type_flags(target, TypeFlags::UNION)
                || self
                    .model
                    .union_constituents(target)
                    .is_some_and(|types| types.len() < 4)
                    && !self
                        .model
                        .has_type_flags(source, TypeFlags::STRUCTURED_OR_INSTANTIABLE);
            let related = if skip_cache {
                sequence.read(self.union_related_completion(
                    source,
                    target,
                    intersection,
                    session,
                    consumer,
                ))
            } else {
                sequence.read(self.recursive_relation_completion(
                    source,
                    target,
                    recursion,
                    intersection,
                    session,
                    consumer,
                ))
            }?;
            if related == Ternary::False && session.report {
                sequence.read(self.report_error_results_completion(
                    (original_source, original_target),
                    (source, target),
                    head_message,
                    session,
                    consumer,
                ))?;
            }
            Ok(related)
        })();
        sequence.finish(result)
    }

    /// `getNormalizedType` repeatedly applies the native family operation until
    /// the actual identity stabilizes. Reading and writing simplification stay
    /// distinct through tuple elements and intersection constituents.
    fn normalized_relation_type_completion(
        &mut self,
        mut ty: TypeId,
        writing: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            loop {
                let normalized = if self.model.is_fresh_literal(ty) {
                    self.regular_relation_literal(ty)
                } else if self.model.is_generic_tuple_type(ty)? {
                    let elements = sequence.read(self.get_element_types_completion(ty))?;
                    // `getNormalizedTupleType`: visit every actual element,
                    // even when simplification preserves all identities.
                    let mut normalized = Vec::with_capacity(elements.len());
                    let mut changed = false;
                    for &element in elements.iter() {
                        let value = if self.model.has_type_flags(
                            element,
                            TypeFlags::INDEXED_ACCESS | TypeFlags::CONDITIONAL,
                        ) {
                            sequence
                                .read(self.simplified_type_completion(element, writing, consumer))?
                        } else {
                            element
                        };
                        changed |= value != element;
                        normalized.push(value);
                    }
                    if !changed {
                        ty
                    } else {
                        let TypeView::Reference(reference) = self.model.get(ty)? else {
                            unreachable!()
                        };
                        sequence.read(self.create_normalized_reference_completion(
                            reference.target,
                            normalized,
                            consumer,
                        ))?
                    }
                } else if let TypeView::Reference(reference) = self.model.get(ty)?
                    && reference.is_reference
                {
                    if self.model.deferred_reference(ty).is_some() {
                        let arguments = sequence.read(self.get_type_arguments_completion(ty))?;
                        let arguments = arguments.to_vec();
                        self.model.reference(reference.target, arguments)?
                    } else {
                        sequence
                            .read(self.non_augmenting_reference_base_completion(ty, consumer))?
                            .unwrap_or(ty)
                    }
                } else if self
                    .model
                    .has_type_flags(ty, TypeFlags::UNION | TypeFlags::INTERSECTION)
                {
                    sequence.read(
                        self.normalized_relation_composite_completion(ty, writing, consumer),
                    )?
                } else if let TypeView::Substitution(view) = self.model.get(ty)? {
                    if writing {
                        view.base
                    } else {
                        sequence.read(self.substitution_intersection_completion(ty, consumer))?
                    }
                } else if self
                    .model
                    .has_type_flags(ty, TypeFlags::INDEXED_ACCESS | TypeFlags::CONDITIONAL)
                {
                    sequence.read(self.simplified_type_completion(ty, writing, consumer))?
                } else {
                    return Ok(ty);
                };
                if normalized == ty {
                    return Ok(ty);
                }
                ty = normalized;
            }
        })();
        sequence.finish(result)
    }

    /// `getNormalizedUnionOrIntersectionType` / `shouldNormalizeIntersection`.
    /// The ordered nullable-or-empty test is short-circuited independently of
    /// the instantiable test, before any child normalization is requested.
    fn normalized_relation_composite_completion(
        &mut self,
        ty: TypeId,
        writing: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let reduced = sequence.read(self.reduced_type_completion(ty, consumer))?;
            if reduced != ty {
                return Ok(reduced);
            }
            let TypeView::Intersection(types) = self.model.get(ty)? else {
                return Ok(ty);
            };
            let mut instantiable = false;
            let mut nullable_or_empty = false;
            for &part in types.iter() {
                instantiable =
                    instantiable || self.model.has_type_flags(part, TypeFlags::INSTANTIABLE);
                nullable_or_empty = nullable_or_empty
                    || self.model.has_type_flags(part, TypeFlags::NULLABLE)
                    || sequence
                        .read(self.is_empty_anonymous_object_type_completion(part, consumer))?;
                if instantiable && nullable_or_empty {
                    let mut normalized = Vec::with_capacity(types.len());
                    let mut changed = false;
                    for &part in types.iter() {
                        let value = sequence.read(
                            self.normalized_relation_type_completion(part, writing, consumer),
                        )?;
                        changed |= value != part;
                        normalized.push(value);
                    }
                    return if !changed {
                        Ok(ty)
                    } else {
                        sequence.read(self.intersection_type_completion(normalized, consumer))
                    };
                }
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    /// `isWeakType` asks the complete inventory even when the first target
    /// property will later reject. In particular, index types are dependencies
    /// of this inventory and cannot be deferred past that property mismatch.
    fn weak_relation_target_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if let TypeView::Substitution(view) = self.model.get(ty)? {
                return sequence.read(self.weak_relation_target_completion(view.base, consumer));
            }
            if self.model.has_type_flags(ty, TypeFlags::INTERSECTION) {
                let TypeView::Intersection(types) = self.model.get(ty)? else {
                    unreachable!()
                };
                for &ty in types.iter() {
                    if !sequence.read(self.weak_relation_target_completion(ty, consumer))? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            if !self.model.has_type_flags(ty, TypeFlags::OBJECT) {
                return Ok(false);
            }
            let Some(members) = self.structured_type_members(ty, consumer)? else {
                return Err(super::super::internal(
                    "weak object target lacks its native inventory",
                )
                .into());
            };
            if !members.call_signatures.is_empty()
                || !members.construct_signatures.is_empty()
                || !members.index_signatures.is_empty()
                || members.properties.is_empty()
            {
                return Ok(false);
            }
            for property in members.properties.iter() {
                let symbol = self.property_symbol(property, consumer)?;
                if !self.property_symbol_flags(symbol)?.optional {
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        sequence.finish(result)
    }

    /// `hasCommonProperties` asks known names in source property order. This
    /// is a symbol-name question and deliberately never reads property types.
    fn has_common_properties_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        jsx: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            for property in self.properties_of_type(source, consumer)? {
                let symbol = self.property_symbol(&property, consumer)?;
                let name = self.property_lookup_key(symbol, consumer)?;
                if sequence.read(self.is_known_property_completion(target, &name, jsx, consumer))? {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isKnownProperty`: ordinary object membership precedes applicable
    /// indexes and JSX names. Composite targets search eligible constituents.
    fn is_known_property_completion(
        &mut self,
        target: TypeId,
        name: &NativePropertyKey,
        jsx: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if self.model.has_type_flags(target, TypeFlags::OBJECT)
                && (self
                .property_of_object_type_by_key(target, name, consumer)?
                .is_some()
                || self
                    .applicable_index_info_for_key(target, name, consumer)?
                    .is_some()
                // isKnownProperty deliberately accepts a symbol-named member
                // through a string index; element access has no such exception.
                || name.is_known_symbol() && {
                    let string = self.model.primitive(PrimitiveName::String);
                    sequence.read(self.index_info_of_type_completion(target, string, consumer))?.is_some()
                }
                || jsx && name.is_hyphenated_jsx_name())
            {
                return Ok(true);
            }
            if let TypeView::Substitution(view) = self.model.get(target)? {
                return sequence
                    .read(self.is_known_property_completion(view.base, name, jsx, consumer));
            }
            if self
                .model
                .has_type_flags(target, TypeFlags::UNION | TypeFlags::INTERSECTION)
                && sequence.read(self.is_excess_property_check_target_completion(target))?
            {
                let types = match self.model.get(target)? {
                    TypeView::Union(types) | TypeView::Intersection(types) => types,
                    _ => unreachable!(),
                };
                for &ty in types.iter() {
                    if sequence.read(self.is_known_property_completion(ty, name, jsx, consumer))? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isExcessPropertyCheckTarget` keeps union Some and intersection Every
    /// distinct, including their native empty-list results.
    fn is_excess_property_check_target_completion(&self, ty: TypeId) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if self.model.has_type_flags(ty, TypeFlags::OBJECT)
                && !self
                    .model
                    .object_flags(ty)?
                    .intersects(ObjectFlags::OBJECT_LITERAL_PATTERN_WITH_COMPUTED_PROPERTIES)
                || self.model.has_type_flags(ty, TypeFlags::NON_PRIMITIVE)
            {
                return Ok(true);
            }
            if let TypeView::Substitution(view) = self.model.get(ty)? {
                return sequence.read(self.is_excess_property_check_target_completion(view.base));
            }
            let (types, every) = match self.model.get(ty)? {
                TypeView::Union(types) => (types, false),
                TypeView::Intersection(types) => (types, true),
                _ => return Ok(false),
            };
            for &ty in types.iter() {
                if sequence.read(self.is_excess_property_check_target_completion(ty))? != every {
                    return Ok(!every);
                }
            }
            Ok(every)
        })();
        sequence.finish(result)
    }

    /// `recursiveTypeRelatedTo` owns cache-before-allowance, Maybe-before-depth,
    /// independent source/target stack depths, expanding recursion, and the
    /// native commit schedule. Ordinary failures restore the session without
    /// installing an unfinished result; resource aborts discard the whole request.
    fn recursive_relation_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        recursion: RecursionFlags,
        intersection: IntersectionState,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let control = self.control.clone();
        let _depth = control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        if self.relations.session(session).overflow.is_some() {
            return Ok(NativeCompletion::clean(Ternary::False));
        }
        // getRelationKey precedes lookup on every invocation. Its work is a
        // prefix, not part of the installed comparison replayed by a cache hit.
        self.relation_key_completion(
            source,
            target,
            intersection,
            self.relations.session(session).kind,
            false,
            consumer,
        )?
        .and_then(|(key, constrained)| {
            if let Some(entry) = self
                .relations
                .caches
                .get(&self.relations.session(session).kind)
                .and_then(|cache| cache.get(&key))
                .cloned()
            {
                // Reporting replays an ordinary Failed comparison from scratch.
                // Only a returned cache hit consumes its retained evidence.
                if *entry.result.value() || !session.report || entry.overflow.is_some() {
                    self.relations.reliability = self.relations.reliability | entry.reliability;
                    let result = entry.result.map(|succeeded| {
                        if succeeded {
                            Ternary::True
                        } else {
                            Ternary::False
                        }
                    });
                    if let Some(overflow) = entry.overflow {
                        if session.report {
                            return result.and_then(|result| {
                                Ok(self
                                    .report_relation_overflow_completion(
                                        source, target, overflow, session, consumer,
                                    )?
                                    .map(|()| result))
                            });
                        }
                    }
                    return Ok(result);
                }
            }
            if self.relations.session(session).allowance <= 0 {
                let active = self.relations.session_mut(session);
                active.overflow = Some(RelationOverflow::Complexity);
                return NativeCompletion::recovered(
                    Ternary::False,
                    self.relation_overflow(RelationOverflow::Complexity),
                );
            }
            if self.relations.session(session).maybe_set.contains(&key) {
                return Ok(NativeCompletion::clean(Ternary::Maybe));
            }
            let mut prefix = NativeSequence::new();
            let result = (|| {
                if constrained {
                    let (broadest, _) = prefix.read(self.relation_key_completion(
                        source,
                        target,
                        intersection,
                        self.relations.session(session).kind,
                        true,
                        consumer,
                    ))?;
                    if self
                        .relations
                        .session(session)
                        .maybe_set
                        .contains(&broadest)
                    {
                        return Ok(Ternary::Maybe);
                    }
                }
                if self.relations.session(session).source_stack.len() == 100
                    || self.relations.session(session).target_stack.len() == 100
                {
                    let active = self.relations.session_mut(session);
                    active.overflow = Some(RelationOverflow::Depth);
                    return prefix.read(NativeCompletion::recovered(
                        Ternary::False,
                        self.relation_overflow(RelationOverflow::Depth),
                    ));
                }
                let active = self.relations.session_mut(session);
                let maybe_start = active.maybe_keys.len();
                active.maybe_keys.push(MaybeRelation {
                    key: key.clone(),
                    result: None,
                });
                active.maybe_set.insert(key.clone(), ());
                let source_depth = active.source_stack.len();
                let target_depth = active.target_stack.len();
                let expanding = active.expanding;
                let mut saved_reliability = None;
                let mut comparison = NativeSequence::new();
                let result = (|| {
                    if recursion.0 & RecursionFlags::SOURCE.0 != 0 {
                        self.relations
                            .session_mut(session)
                            .source_stack
                            .push(source);
                        if self.relations.session(session).expanding & 1 == 0 {
                            let stack = self.relations.session(session).source_stack.to_vec();
                            if comparison.read(self.deeply_nested_relation_type_completion(
                                source, &stack, 3, consumer,
                            ))? {
                                self.relations.session_mut(session).expanding |= 1;
                            }
                        }
                    }
                    if recursion.0 & RecursionFlags::TARGET.0 != 0 {
                        self.relations
                            .session_mut(session)
                            .target_stack
                            .push(target);
                        if self.relations.session(session).expanding & 2 == 0 {
                            let stack = self.relations.session(session).target_stack.to_vec();
                            if comparison.read(self.deeply_nested_relation_type_completion(
                                target, &stack, 3, consumer,
                            ))? {
                                self.relations.session_mut(session).expanding |= 2;
                            }
                        }
                    }
                    saved_reliability = Some(self.relations.reliability);
                    self.relations.reliability = VarianceFlags::INVARIANT;
                    if self.relations.session(session).expanding == 3 {
                        Ok(Ternary::Maybe)
                    } else {
                        comparison.read(self.structured_relation_completion(
                            source,
                            target,
                            intersection,
                            session,
                            consumer,
                        ))
                    }
                })();
                let propagating_reliability = if let Some(saved) = saved_reliability {
                    let propagating = self.relations.reliability;
                    self.relations.reliability = propagating | saved;
                    propagating
                } else {
                    VarianceFlags::INVARIANT
                };
                let active = self.relations.session_mut(session);
                active.source_stack.truncate(source_depth);
                active.target_stack.truncate(target_depth);
                active.expanding = expanding;
                // Record this invocation before resetMaybeStack can commit it.
                // A child that completed before later ancestor work stays local.
                let result = comparison.finish(result);
                match &result {
                    Err(_) => self.reset_relation_maybe(
                        maybe_start,
                        false,
                        VarianceFlags::INVARIANT,
                        session,
                    )?,
                    Ok(completed) => {
                        self.relations.session_mut(session).maybe_keys[maybe_start].result =
                            Some(completed.clone());
                        let value = *completed.value();
                        if value == Ternary::False {
                            let kind = self.relations.session(session).kind;
                            self.relations.store(
                                kind,
                                key.clone(),
                                RelationEntry {
                                    result: completed.clone().map(|_| false),
                                    overflow: None,
                                    reliability: propagating_reliability,
                                },
                            );
                            self.relations.session_mut(session).allowance -= 1;
                            self.reset_relation_maybe(
                                maybe_start,
                                false,
                                VarianceFlags::INVARIANT,
                                session,
                            )?;
                        } else if value == Ternary::True || source_depth == 0 && target_depth == 0 {
                            self.reset_relation_maybe(
                                maybe_start,
                                value != Ternary::Unknown,
                                propagating_reliability,
                                session,
                            )?;
                        }
                    }
                }
                prefix.read(result)
            })();
            prefix.finish(result)
        })
    }

    /// `resetMaybeStack` commits only native True or root Maybe assumptions.
    fn reset_relation_maybe(
        &mut self,
        start: usize,
        succeeded: bool,
        reliability: VarianceFlags,
        session: RelationInvocation,
    ) -> Result<(), DemandError> {
        let active = self.relations.session_mut(session);
        let count = active
            .maybe_keys
            .len()
            .checked_sub(start)
            .expect("native Maybe reset starts within the active stack");
        let mut keys = Vec::with_capacity(count);
        for pending in active.maybe_keys.drain(start..) {
            keys.push(pending);
        }
        let kind = active.kind;
        for pending in keys {
            self.relations
                .session_mut(session)
                .maybe_set
                .remove(&pending.key);
            if succeeded {
                let completed = pending.result.ok_or_else(|| {
                    super::super::internal("native Maybe commit lacks its completed comparison")
                })?;
                self.relations.store(
                    kind,
                    pending.key,
                    RelationEntry {
                        result: completed.map(|_| true),
                        overflow: None,
                        reliability,
                    },
                );
                self.relations.session_mut(session).allowance -= 1;
            }
        }
        Ok(())
    }

    /// `isDeeplyNestedType`: only increasing
    /// native allocation identities count toward the generative recursion
    /// heuristic; an exact active pair is handled earlier by Maybe keys.
    pub(super) fn deeply_nested_relation_type_completion(
        &mut self,
        ty: TypeId,
        stack: &[TypeId],
        maximum: usize,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if stack.len() < maximum {
                return Ok(false);
            }
            let ty = sequence.read(self.mapped_target_with_symbol_completion(ty, consumer))?;
            if let TypeView::Intersection(types) = self.model.get(ty)? {
                for &ty in types.iter() {
                    if sequence.read(
                        self.deeply_nested_relation_type_completion(ty, stack, maximum, consumer),
                    )? {
                        return Ok(true);
                    }
                }
            }
            let identity = self.recursion_identity(ty, consumer)?;
            let mut count = 0;
            let mut last = None;
            for &candidate in stack {
                if sequence.read(
                    self.matching_relation_recursion_completion(candidate, identity, consumer),
                )? {
                    if last.is_none_or(|last| candidate >= last) {
                        count += 1;
                        if count >= maximum {
                            return Ok(true);
                        }
                    }
                    last = Some(candidate);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `hasMatchingRecursionIdentity` tests mapped targets and recursively
    /// examines intersection constituents before comparing their actual identity.
    fn matching_relation_recursion_completion(
        &mut self,
        ty: TypeId,
        identity: RecursionIdentity,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let ty = sequence.read(self.mapped_target_with_symbol_completion(ty, consumer))?;
            if let TypeView::Intersection(types) = self.model.get(ty)? {
                for &ty in types.iter() {
                    if sequence
                        .read(self.matching_relation_recursion_completion(ty, identity, consumer))?
                    {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            Ok(self.recursion_identity(ty, consumer)? == identity)
        })();
        sequence.finish(result)
    }

    /// `getRelationKey` / `writeGenericTypeReferences`: canonical parameter
    /// positions are shared across the ordered pair. The ordinary key asks
    /// actual constraints; broadest-equivalent keys deliberately skip them.
    fn relation_key_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        intersection: IntersectionState,
        relation: RelationKind,
        ignore_constraints: bool,
        consumer: &Location,
    ) -> NativeResult<(RelationKey, bool)> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(RelationKey, bool), DemandError> {
            let (source, target) = if relation == RelationKind::Identity && source > target {
                (target, source)
            } else {
                (source, target)
            };
            let (value, constrained) = if sequence
                .read(self.reference_with_generic_arguments_completion(source))?
                && sequence.read(self.reference_with_generic_arguments_completion(target))?
            {
                let mut writer = GenericKeyWriter {
                    ignore_constraints,
                    parameters: Vec::new(),
                    parts: Vec::new(),
                    constrained: false,
                };
                sequence.read(self.write_generic_relation_reference_completion(
                    source,
                    0,
                    &mut writer,
                    consumer,
                ))?;
                writer.push(KeyPart::Pair);
                sequence.read(self.write_generic_relation_reference_completion(
                    target,
                    0,
                    &mut writer,
                    consumer,
                ))?;
                (
                    RelationKeyValue::Generic(std::sync::Arc::from(writer.parts), intersection),
                    writer.constrained,
                )
            } else {
                (
                    RelationKeyValue::Ordinary(source, target, intersection),
                    false,
                )
            };
            Ok((RelationKey::new(value), constrained))
        })();
        sequence.finish(result)
    }

    /// `isTypeReferenceWithGenericArguments` examines actual nondeferred
    /// reference arguments, independently from alias or printed type names.
    fn reference_with_generic_arguments_completion(&mut self, ty: TypeId) -> NativeResult<bool> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let TypeView::Reference(reference) = self.model.get(ty)? else {
                return Ok(false);
            };
            if !reference.is_reference || self.model.deferred_reference(ty).is_some() {
                return Ok(false);
            }
            let arguments = sequence.read(self.get_type_arguments_completion(ty))?;
            for &argument in arguments.iter() {
                if self
                    .model
                    .has_type_flags(argument, TypeFlags::TYPE_PARAMETER)
                    || sequence.read(self.reference_with_generic_arguments_completion(argument))?
                {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    fn write_generic_relation_reference_completion(
        &mut self,
        ty: TypeId,
        depth: usize,
        writer: &mut GenericKeyWriter,
        consumer: &Location,
    ) -> NativeResult<()> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let TypeView::Reference(reference) = self.model.get(ty)? else {
                return Err(
                    super::super::internal("generic relation key has no actual reference").into(),
                );
            };
            writer.push(KeyPart::Type(reference.target));
            let arguments = self.model.resolved_type_arguments(ty).ok_or_else(|| {
                super::super::internal(
                    "nondeferred generic key reference lacks established arguments",
                )
            })??;
            for &argument in arguments.iter() {
                if self
                    .model
                    .has_type_flags(argument, TypeFlags::TYPE_PARAMETER)
                {
                    if writer.ignore_constraints
                        || sequence
                            .read(self.parameter_constraint_completion(argument, consumer))?
                            .is_none()
                    {
                        let index = writer.parameter(argument);
                        writer.push(KeyPart::Parameter(index));
                        continue;
                    }
                    writer.constrained = true;
                } else if depth < 4
                    && sequence.read(self.reference_with_generic_arguments_completion(argument))?
                {
                    writer.push(KeyPart::Open);
                    sequence.read(self.write_generic_relation_reference_completion(
                        argument,
                        depth + 1,
                        writer,
                        consumer,
                    ))?;
                    writer.push(KeyPart::Close);
                    continue;
                }
                writer.push(KeyPart::Argument);
                writer.push(KeyPart::Type(argument));
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `structuredTypeRelatedTo` supplements decomposition with effective
    /// constraints and the native nested/optional property checks. These use
    /// the same active session and preserve provisional Ternary answers.
    fn structured_relation_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        intersection: IntersectionState,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let error_state = self.relations.session(session).error.clone();
            let mut result = sequence.read(self.structured_relation_worker_completion(
                source,
                target,
                intersection,
                session,
                consumer,
            ))?;
            if self.relations.session(session).kind == RelationKind::Identity {
                return Ok(result);
            }
            if result == Ternary::False
                && (self.model.has_type_flags(source, TypeFlags::INTERSECTION)
                    || self.model.has_type_flags(source, TypeFlags::TYPE_PARAMETER)
                        && self.model.has_type_flags(target, TypeFlags::UNION))
            {
                let sources = self
                    .model
                    .intersection_constituents(source)
                    .map(|types| types.to_vec())
                    .unwrap_or_else(|| vec![source]);
                if let Some(constraint) =
                    sequence.read(self.effective_constraint_of_intersection_completion(
                        &sources,
                        self.model.has_type_flags(target, TypeFlags::UNION),
                        consumer,
                    ))?
                {
                    let distinct = match self.model.union_constituents(constraint) {
                        Some(types) => !types.contains(&source),
                        None => constraint != source,
                    };
                    if distinct {
                        result = sequence.read(self.related_in_session_completion(
                            constraint,
                            target,
                            RecursionFlags::SOURCE,
                            intersection,
                            session.reporting(false),
                            consumer,
                        ))?;
                    }
                }
            }
            if result != Ternary::False
                && intersection.0 & IntersectionState::TARGET.0 == 0
                && self.model.has_type_flags(target, TypeFlags::INTERSECTION)
                && !self.is_generic_object_type(target, consumer)?
                && self
                    .model
                    .has_type_flags(source, TypeFlags::OBJECT | TypeFlags::INTERSECTION)
            {
                result = result.and(sequence.read(self.properties_related_completion(
                    source,
                    target,
                    &HashSet::new(),
                    false,
                    RelationComparer {
                        session,
                        intersection: IntersectionState::NONE,
                    },
                    consumer,
                ))?);
                if result != Ternary::False
                    && self
                        .model
                        .object_flags(source)?
                        .contains(ObjectFlags::OBJECT_LITERAL | ObjectFlags::FRESH_LITERAL)
                {
                    result = result.and(sequence.read(self.relation_indexes_completion(
                        source,
                        target,
                        false,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?);
                }
            } else if result != Ternary::False
                && self.is_non_generic_object_type(target, consumer)?
                && !self.is_array_or_tuple_type(target)?
                && sequence.read(
                    self.source_intersection_needs_extra_check_completion(source, target, consumer),
                )?
            {
                result = result.and(sequence.read(self.properties_related_completion(
                    source,
                    target,
                    &HashSet::new(),
                    true,
                    RelationComparer {
                        session,
                        intersection,
                    },
                    consumer,
                ))?);
            }
            if result != Ternary::False {
                self.relations.session_mut(session).error = error_state;
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `getEffectiveConstraintOfIntersection` follows only constraints whose
    /// native kinds guarantee a finite chain, then normalizes the combined
    /// constraint without restarting constraint reduction in the factory.
    fn effective_constraint_of_intersection_completion(
        &mut self,
        types: &[TypeId],
        target_is_union: bool,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let mut constraints = Vec::new();
            let mut has_disjoint = false;
            for &ty in types {
                if self.model.has_type_flags(ty, TypeFlags::INSTANTIABLE) {
                    let mut constraint =
                        sequence.read(self.constraint_of_type_completion(ty, consumer))?;
                    while let Some(current) = constraint
                        && self.model.has_type_flags(
                            current,
                            TypeFlags::TYPE_PARAMETER | TypeFlags::INDEX | TypeFlags::CONDITIONAL,
                        )
                    {
                        constraint =
                            sequence.read(self.constraint_of_type_completion(current, consumer))?;
                    }
                    if let Some(constraint) = constraint {
                        constraints.push(constraint);
                        if target_is_union {
                            constraints.push(ty);
                        }
                    }
                } else if self.model.has_type_flags(ty, TypeFlags::DISJOINT_DOMAINS)
                    || self.is_empty_anonymous_object_type(ty, consumer)?
                {
                    has_disjoint = true;
                }
            }
            if constraints.is_empty() || !(target_is_union || has_disjoint) {
                return Ok(None);
            }
            if has_disjoint {
                for &ty in types {
                    if self.model.has_type_flags(ty, TypeFlags::DISJOINT_DOMAINS)
                        || self.is_empty_anonymous_object_type(ty, consumer)?
                    {
                        constraints.push(ty);
                    }
                }
            }
            let constraint = self.intersection_type_ex(
                constraints,
                IntersectionFlags::NO_CONSTRAINT_REDUCTION,
                None,
                consumer,
            )?;
            sequence
                .read(self.normalized_relation_type_completion(constraint, false, consumer))
                .map(Some)
        })();
        sequence.finish(result)
    }

    /// `isSourceIntersectionNeedingExtraCheck` excludes a matching constituent
    /// or the exact NonInferrableType factory flag before optional reads.
    fn source_intersection_needs_extra_check_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let result = (|| -> Result<bool, DemandError> {
            let Some(types) = self.model.intersection_constituents(source) else {
                return Ok(false);
            };
            let apparent = self.apparent_type(source, consumer)?;
            if !self.model.has_type_flags(
                apparent,
                TypeFlags::OBJECT | TypeFlags::UNION | TypeFlags::INTERSECTION,
            ) {
                return Ok(false);
            }
            for &ty in types.iter() {
                if ty == target
                    || self
                        .model
                        .object_flags(ty)?
                        .intersects(ObjectFlags::NON_INFERRABLE_TYPE)
                {
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        result.map(NativeCompletion::clean)
    }

    /// `structuredTypeRelatedToWorker`: ordinary structures compare complete
    /// inventories in property/call/construct/index order. A completed False
    /// skips later collections. Its final False also owns object-to-Never.
    fn structured_relation_worker_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        intersection: IntersectionState,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            if self.relations.session(session).kind == RelationKind::Identity {
                return sequence
                    .read(self.structured_identity_completion(source, target, session, consumer));
            }
            let mut state = StructuredComparison {
                comparer: RelationComparer {
                    session,
                    intersection,
                },
                saved_error: self.relations.session(session).error.clone(),
                variance_failed: false,
                original_chain: None,
            };
            if self
                .model
                .has_type_flags(source, TypeFlags::UNION | TypeFlags::INTERSECTION)
                || self
                    .model
                    .has_type_flags(target, TypeFlags::UNION | TypeFlags::INTERSECTION)
            {
                let result = sequence.read(self.union_related_completion(
                    source,
                    target,
                    intersection,
                    session,
                    consumer,
                ))?;
                if result != Ternary::False {
                    return Ok(result);
                }
                if !(self.model.has_type_flags(source, TypeFlags::INSTANTIABLE)
                    || self.model.has_type_flags(source, TypeFlags::OBJECT)
                        && self.model.has_type_flags(target, TypeFlags::UNION)
                    || self.model.has_type_flags(source, TypeFlags::INTERSECTION)
                        && self.model.has_type_flags(
                            target,
                            TypeFlags::OBJECT | TypeFlags::UNION | TypeFlags::INSTANTIABLE,
                        ))
                {
                    return Ok(Ternary::False);
                }
            }
            if let Some(result) = sequence.read(
                self.relation_alias_variances_completion(source, target, &mut state, consumer),
            )? {
                return Ok(result);
            }
            if let Some(result) = sequence.read(
                self.single_generic_tuple_relation_completion(source, target, session, consumer),
            )? {
                return Ok(result);
            }
            if self.model.has_type_flags(target, TypeFlags::TYPE_PARAMETER)
                && let Some(result) =
                    sequence.read(self.mapped_source_parameter_related_completion(
                        source, target, session, consumer,
                    ))?
            {
                return Ok(result);
            }
            if self.model.has_type_flags(target, TypeFlags::TYPE_PARAMETER)
                && self.relations.session(session).kind == RelationKind::Comparable
                && self.model.has_type_flags(source, TypeFlags::TYPE_PARAMETER)
            {
                if let Some(constraint) =
                    sequence.read(self.parameter_constraint_completion(source, consumer))?
                {
                    let types = self
                        .model
                        .union_constituents(constraint)
                        .map(|types| types.to_vec())
                        .unwrap_or_else(|| vec![constraint]);
                    if types
                        .iter()
                        .any(|&ty| self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER))
                    {
                        return sequence.read(self.related_in_session_completion(
                            constraint,
                            target,
                            RecursionFlags::SOURCE,
                            IntersectionState::NONE,
                            session.reporting(false),
                            consumer,
                        ));
                    }
                }
                return Ok(Ternary::False);
            }
            if let Some(related) = sequence.read(
                self.operator_target_related_completion(source, target, &mut state, consumer),
            )? {
                return Ok(related);
            }
            if let Some(result) = sequence.read(
                self.generic_mapped_target_related_completion(source, target, &mut state, consumer),
            )? {
                return Ok(result);
            }
            if self.model.has_type_flags(source, TypeFlags::TYPE_VARIABLE) {
                // Native handled both indexed components in the target branch;
                // this source case then skips constraints and exits with False.
                if self.model.has_type_flags(source, TypeFlags::INDEXED_ACCESS)
                    && self.model.has_type_flags(target, TypeFlags::INDEXED_ACCESS)
                {
                    return Ok(Ternary::False);
                }
                let constraint =
                    match sequence.read(self.constraint_of_type_completion(source, consumer))? {
                        Some(constraint) => constraint,
                        None => self.model.special(SpecialName::Unknown),
                    };
                let related = sequence.read(self.related_in_session_completion(
                    constraint,
                    target,
                    RecursionFlags::SOURCE,
                    intersection,
                    session.reporting(false),
                    consumer,
                ))?;
                if related != Ternary::False {
                    return Ok(related);
                }
                let with_this = sequence.read(self.get_type_with_this_argument_completion(
                    constraint,
                    Some(source),
                    false,
                    consumer,
                ))?;
                let report = session.report
                    && constraint != self.model.special(SpecialName::Unknown)
                    && !(self.model.has_type_flags(source, TypeFlags::TYPE_PARAMETER)
                        && self.model.has_type_flags(target, TypeFlags::TYPE_PARAMETER));
                let related = sequence.read(self.related_in_session_completion(
                    with_this,
                    target,
                    RecursionFlags::SOURCE,
                    intersection,
                    session.reporting(report),
                    consumer,
                ))?;
                if related != Ternary::False {
                    return Ok(related);
                }
                if let TypeView::IndexedAccess(access) = self.model.get(source)?
                    && sequence.read(self.is_mapped_generic_indexed_access_completion(
                        access.object_type,
                        access.index_type,
                        consumer,
                    ))?
                    && let Some(constraint) = sequence
                        .read(self.constraint_of_type_completion(access.index_type, consumer))?
                {
                    let indexed = sequence.read(self.indexed_access_type_completion(
                        access.object_type,
                        constraint,
                        consumer,
                    ))?;
                    let result = sequence.read(self.related_in_session_completion(
                        indexed,
                        target,
                        RecursionFlags::SOURCE,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    if result != Ternary::False {
                        return Ok(result);
                    }
                }
                return Ok(Ternary::False);
            }
            if let TypeView::Index(index) = self.model.get(source)? {
                let deferred_mapped = sequence.read(self.should_defer_index_type_completion(
                    index.target,
                    index.index_flags,
                    consumer,
                ))? && self
                    .model
                    .object_flags(index.target)?
                    .intersects(ObjectFlags::MAPPED);
                let keys = self.model.string_number_symbol_type();
                let related = sequence.read(self.related_in_session_completion(
                    keys,
                    target,
                    RecursionFlags::SOURCE,
                    IntersectionState::NONE,
                    session.reporting(session.report && !deferred_mapped),
                    consumer,
                ))?;
                if related != Ternary::False {
                    return Ok(related);
                }
                if deferred_mapped {
                    let name =
                        sequence.read(self.mapped_name_completion(index.target, consumer))?;
                    let keys = if let Some(name) = name {
                        if self.mapped_recipe(index.target)?.keyof_operand.is_some() {
                            sequence.read(self.apparent_mapped_type_keys_completion(
                                name,
                                index.target,
                                consumer,
                            ))?
                        } else {
                            name
                        }
                    } else {
                        sequence.read(self.mapped_constraint_completion(index.target, consumer))?
                    };
                    let result = sequence.read(self.related_in_session_completion(
                        keys,
                        target,
                        RecursionFlags::SOURCE,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    if result != Ternary::False {
                        return Ok(result);
                    }
                }
                return Ok(Ternary::False);
            }
            if let TypeView::Conditional(source_view) = self.model.get(source)? {
                let stack = self.relations.session(session).source_stack.clone();
                if sequence.read(
                    self.deeply_nested_relation_type_completion(source, &stack, 10, consumer),
                )? {
                    return Ok(Ternary::Maybe);
                }
                if let TypeView::Conditional(target_view) = self.model.get(target)? {
                    let parameters = self.conditional_parameters(source)?;
                    let mut extends = source_view.extends_type;
                    let mapper = if parameters.is_empty() {
                        None
                    } else {
                        let mapper =
                            sequence.read(self.conditional_inference_mapper_completion(
                                parameters,
                                Some(target_view.extends_type),
                                extends,
                                None,
                                Some(RelationComparer {
                                    session,
                                    intersection: IntersectionState::NONE,
                                }),
                                consumer,
                            ))?;
                        extends = sequence
                            .read(self.instantiate_type_completion(extends, mapper, consumer))?;
                        Some(mapper)
                    };
                    if sequence.read(self.raw_type_related_completion(
                        extends,
                        target_view.extends_type,
                        RelationKind::Identity,
                        consumer,
                    ))? && (sequence.read(self.related_in_session_completion(
                        source_view.check_type,
                        target_view.check_type,
                        RecursionFlags::BOTH,
                        IntersectionState::NONE,
                        session.reporting(false),
                        consumer,
                    ))? != Ternary::False
                        || sequence.read(self.related_in_session_completion(
                            target_view.check_type,
                            source_view.check_type,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session.reporting(false),
                            consumer,
                        ))? != Ternary::False)
                    {
                        let source_true = sequence
                            .read(self.conditional_true_type_completion(source, consumer))?;
                        let source_true = match mapper {
                            Some(mapper) => sequence.read(self.instantiate_type_completion(
                                source_true,
                                mapper,
                                consumer,
                            ))?,
                            None => source_true,
                        };
                        let target_true = sequence
                            .read(self.conditional_true_type_completion(target, consumer))?;
                        let mut result = sequence.read(self.related_in_session_completion(
                            source_true,
                            target_true,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))?;
                        if result != Ternary::False {
                            let source_false = sequence
                                .read(self.conditional_false_type_completion(source, consumer))?;
                            let target_false = sequence
                                .read(self.conditional_false_type_completion(target, consumer))?;
                            result =
                                result.and(sequence.read(self.related_in_session_completion(
                                    source_false,
                                    target_false,
                                    RecursionFlags::BOTH,
                                    IntersectionState::NONE,
                                    session,
                                    consumer,
                                ))?);
                            if result != Ternary::False {
                                return Ok(result);
                            }
                        }
                    }
                }
                let constraint = sequence
                    .read(self.default_conditional_constraint_completion(source, consumer))?;
                let result = sequence.read(self.related_in_session_completion(
                    constraint,
                    target,
                    RecursionFlags::SOURCE,
                    IntersectionState::NONE,
                    session,
                    consumer,
                ))?;
                if result != Ternary::False {
                    return Ok(result);
                }
                if !self.model.has_type_flags(target, TypeFlags::CONDITIONAL)
                    && sequence
                        .read(self.has_non_circular_base_constraint_completion(source, consumer))?
                    && let Some(constraint) = sequence.read(
                        self.distributive_conditional_constraint_completion(source, consumer),
                    )?
                {
                    self.relations.session_mut(session).error = state.saved_error.clone();
                    let result = sequence.read(self.related_in_session_completion(
                        constraint,
                        target,
                        RecursionFlags::SOURCE,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    if result != Ternary::False {
                        return Ok(result);
                    }
                }
                return Ok(Ternary::False);
            }
            if self
                .model
                .has_type_flags(source, TypeFlags::TEMPLATE_LITERAL)
                && !self.model.has_type_flags(target, TypeFlags::OBJECT)
            {
                if !self
                    .model
                    .has_type_flags(target, TypeFlags::TEMPLATE_LITERAL)
                    && let Some(constraint) =
                        sequence.read(self.base_constraint_completion(source, consumer))?
                    && constraint != source
                {
                    let result = sequence.read(self.related_in_session_completion(
                        constraint,
                        target,
                        RecursionFlags::SOURCE,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    if result != Ternary::False {
                        return Ok(result);
                    }
                }
            } else if let TypeView::StringMapping(mapping) = self.model.get(source)? {
                if let TypeView::StringMapping(target) = self.model.get(target)? {
                    if mapping.symbol != target.symbol {
                        return Ok(Ternary::False);
                    }
                    let result = sequence.read(self.related_in_session_completion(
                        mapping.target,
                        target.target,
                        RecursionFlags::BOTH,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    if result != Ternary::False {
                        return Ok(result);
                    }
                } else if let Some(constraint) =
                    sequence.read(self.base_constraint_completion(source, consumer))?
                {
                    let result = sequence.read(self.related_in_session_completion(
                        constraint,
                        target,
                        RecursionFlags::SOURCE,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    if result != Ternary::False {
                        return Ok(result);
                    }
                }
            }
            if self.model.has_type_flags(
                source,
                TypeFlags::INSTANTIABLE
                    & !(TypeFlags::TYPE_PARAMETER
                        | TypeFlags::INDEX
                        | TypeFlags::INDEXED_ACCESS
                        | TypeFlags::TEMPLATE_LITERAL
                        | TypeFlags::STRING_MAPPING
                        | TypeFlags::CONDITIONAL
                        | TypeFlags::SUBSTITUTION),
            ) || self.model.has_type_flags(
                target,
                TypeFlags::INSTANTIABLE
                    & !(TypeFlags::TYPE_PARAMETER
                        | TypeFlags::INDEX
                        | TypeFlags::INDEXED_ACCESS
                        | TypeFlags::TEMPLATE_LITERAL
                        | TypeFlags::STRING_MAPPING
                        | TypeFlags::CONDITIONAL
                        | TypeFlags::SUBSTITUTION),
            ) {
                return Err(self.relation_pending(
                    consumer,
                    "native type-variable constraint relation comparison is pending",
                ));
            }
            if !matches!(
                self.relations.session(session).kind,
                RelationKind::Subtype | RelationKind::StrictSubtype
            ) && self
                .model
                .object_flags(target)?
                .intersects(ObjectFlags::MAPPED)
                && self.mapped_recipe(target)?.optional > 0
                && sequence.read(self.is_empty_object_type_completion(source, consumer))?
            {
                return Ok(Ternary::True);
            }
            if sequence.read(self.is_generic_mapped_type_completion(target, consumer))? {
                return if sequence.read(self.is_generic_mapped_type_completion(source, consumer))? {
                    sequence.read(
                        self.mapped_type_related_completion(source, target, session, consumer),
                    )
                } else {
                    Ok(Ternary::False)
                };
            }
            let source_is_primitive = self.model.has_type_flags(source, TypeFlags::PRIMITIVE);
            let source = sequence.read(self.apparent_type_completion(source, consumer))?;
            let mut reference_alternatives = match sequence.read(
                self.relation_reference_variances_completion(source, target, &mut state, consumer),
            )? {
                ReferenceRelation::Related(result) => return Ok(result),
                ReferenceRelation::Structural => false,
                ReferenceRelation::NotApplicable => true,
            };
            if reference_alternatives
                && sequence.read(self.array_relation_candidate_completion(source, target))?
            {
                let number = self.model.primitive(PrimitiveName::Number);
                let source = sequence
                    .read(self.index_type_of_type_completion(source, number, consumer))?
                    .unwrap_or_else(|| self.model.special(SpecialName::Any));
                let target = sequence
                    .read(self.index_type_of_type_completion(target, number, consumer))?
                    .unwrap_or_else(|| self.model.special(SpecialName::Any));
                return sequence.read(self.related_in_session_completion(
                    source,
                    target,
                    RecursionFlags::BOTH,
                    IntersectionState::NONE,
                    session,
                    consumer,
                ));
            }
            if reference_alternatives
                && let Some(constraint) = sequence
                    .read(self.tuple_relation_constraint_completion(source, target, consumer))?
            {
                reference_alternatives = false;
                if constraint != source {
                    return sequence.read(self.related_in_session_completion(
                        constraint,
                        target,
                        RecursionFlags::SOURCE,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ));
                }
            }
            if reference_alternatives
                && matches!(
                    self.relations.session(session).kind,
                    RelationKind::Subtype | RelationKind::StrictSubtype
                )
                && sequence.read(self.is_empty_object_type_completion(target, consumer))?
                && self
                    .model
                    .object_flags(target)?
                    .intersects(ObjectFlags::FRESH_LITERAL)
                && !sequence.read(self.is_empty_object_type_completion(source, consumer))?
            {
                return Ok(Ternary::False);
            }
            if self
                .model
                .has_type_flags(source, TypeFlags::OBJECT | TypeFlags::INTERSECTION)
                && self.model.has_type_flags(target, TypeFlags::OBJECT)
            {
                let structural_session = session.reporting(
                    session.report
                        && self
                            .relations
                            .session(session)
                            .error
                            .same_head(&state.saved_error)
                        && !source_is_primitive,
                );
                let mut result = sequence.read(self.properties_related_completion(
                    source,
                    target,
                    &HashSet::new(),
                    false,
                    RelationComparer {
                        session: structural_session,
                        intersection,
                    },
                    consumer,
                ))?;
                if result == Ternary::False {
                    return Ok(result);
                }
                for construct in [false, true] {
                    result = result.and(sequence.read(self.relation_signatures_completion(
                        source,
                        target,
                        construct,
                        intersection,
                        structural_session,
                        consumer,
                    ))?);
                    if result == Ternary::False {
                        return Ok(result);
                    }
                }
                result = result.and(sequence.read(self.relation_indexes_completion(
                    source,
                    target,
                    source_is_primitive,
                    intersection,
                    structural_session,
                    consumer,
                ))?);
                if result != Ternary::False && state.variance_failed {
                    let error = &mut self.relations.session_mut(session).error;
                    if state.original_chain.is_some() {
                        error.chain = state.original_chain;
                    } else if error.chain.is_none() {
                        error.chain = state.saved_error.chain;
                    }
                    return Ok(Ternary::False);
                }
                return Ok(result);
            }
            if self
                .model
                .has_type_flags(source, TypeFlags::OBJECT | TypeFlags::INTERSECTION)
                && self.model.union_constituents(target).is_some()
            {
                let object_target = self.model.filter_type(target, |model, ty| {
                    model.has_type_flags(
                        ty,
                        TypeFlags::OBJECT | TypeFlags::INTERSECTION | TypeFlags::SUBSTITUTION,
                    )
                });
                if self.model.has_type_flags(object_target, TypeFlags::UNION) {
                    return sequence.read(self.discriminated_type_related_completion(
                        source,
                        object_target,
                        session.reporting(false),
                        consumer,
                    ));
                }
            }
            Ok(Ternary::False)
        })();
        sequence.finish(result)
    }

    /// Indexed-access and keyof targets in `structuredTypeRelatedToWorker`.
    /// Returning None means the native target branch did not establish a
    /// relationship; source constraints and ordinary structure still follow.
    fn operator_target_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        state: &mut StructuredComparison,
        consumer: &Location,
    ) -> NativeResult<Option<Ternary>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Ternary>, DemandError> {
            let RelationComparer {
                session,
                intersection,
            } = state.comparer;
            match self.model.get(target)? {
                TypeView::Conditional(view) => {
                    let stack = self.relations.session(session).target_stack.clone();
                    if sequence.read(
                        self.deeply_nested_relation_type_completion(target, &stack, 10, consumer),
                    )? {
                        return Ok(Some(Ternary::Maybe));
                    }
                    if self.conditional_parameters(target)?.is_empty()
                        && !sequence
                            .read(self.conditional_distribution_dependent_completion(target))?
                        && !matches!(self.model.get(source)?, TypeView::Conditional(source) if source.root == view.root)
                    {
                        let check = sequence.read(
                            self.permissive_instantiation_completion(view.check_type, consumer),
                        )?;
                        let extends = sequence.read(
                            self.permissive_instantiation_completion(view.extends_type, consumer),
                        )?;
                        let skip_true = !sequence.read(self.raw_type_related_completion(
                            check,
                            extends,
                            RelationKind::Assignable,
                            consumer,
                        ))?;
                        let skip_false = if skip_true {
                            false
                        } else {
                            let check =
                                sequence.read(self.restrictive_instantiation_completion(
                                    view.check_type,
                                    consumer,
                                ))?;
                            let extends =
                                sequence.read(self.restrictive_instantiation_completion(
                                    view.extends_type,
                                    consumer,
                                ))?;
                            sequence.read(self.raw_type_related_completion(
                                check,
                                extends,
                                RelationKind::Assignable,
                                consumer,
                            ))?
                        };
                        let mut result = if skip_true {
                            Ternary::True
                        } else {
                            let branch = sequence
                                .read(self.conditional_true_type_completion(target, consumer))?;
                            sequence.read(self.related_in_session_completion(
                                source,
                                branch,
                                RecursionFlags::TARGET,
                                intersection,
                                session.reporting(false),
                                consumer,
                            ))?
                        };
                        if result != Ternary::False {
                            if !skip_false {
                                let branch = sequence.read(
                                    self.conditional_false_type_completion(target, consumer),
                                )?;
                                result = result.and(sequence.read(
                                    self.related_in_session_completion(
                                        source,
                                        branch,
                                        RecursionFlags::TARGET,
                                        intersection,
                                        session.reporting(false),
                                        consumer,
                                    ),
                                )?);
                            }
                            if result != Ternary::False {
                                return Ok(Some(result));
                            }
                        }
                    }
                }
                TypeView::TemplateLiteral(template) => {
                    if let TypeView::TemplateLiteral(source_template) = self.model.get(source)? {
                        if self.relations.session(session).kind == RelationKind::Comparable {
                            return Ok(Some(
                                if super::templates::matching::definitely_unrelated(
                                    &source_template,
                                    &template,
                                )
                                .map_err(|error| self.literal_value_failure(error, consumer))?
                                {
                                    Ternary::False
                                } else {
                                    Ternary::True
                                },
                            ));
                        }
                        let mapper = self.model.report_reliability_mapper(false);
                        sequence
                            .read(self.instantiate_type_completion(source, mapper, consumer))?;
                    }
                    if sequence.read(self.template_type_matches_completion(
                        source,
                        &template,
                        consumer,
                        &mut |checker, source, target| {
                            Ok(checker
                                .related_in_session_completion(
                                    source,
                                    target,
                                    RecursionFlags::BOTH,
                                    IntersectionState::NONE,
                                    session,
                                    consumer,
                                )?
                                .map(|result| result != Ternary::False))
                        },
                    ))? {
                        return Ok(Some(Ternary::True));
                    }
                }
                TypeView::StringMapping(_) => {
                    if !self.model.has_type_flags(source, TypeFlags::STRING_MAPPING)
                        && sequence.read(
                            self.member_of_string_mapping_completion(source, target, consumer),
                        )?
                    {
                        return Ok(Some(Ternary::True));
                    }
                }
                TypeView::IndexedAccess(target) => {
                    if let TypeView::IndexedAccess(source) = self.model.get(source)? {
                        let mut result = sequence.read(self.related_in_session_completion(
                            source.object_type,
                            target.object_type,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))?;
                        if result != Ternary::False {
                            result =
                                result.and(sequence.read(self.related_in_session_completion(
                                    source.index_type,
                                    target.index_type,
                                    RecursionFlags::BOTH,
                                    IntersectionState::NONE,
                                    session,
                                    consumer,
                                ))?);
                        }
                        if result != Ternary::False {
                            return Ok(Some(result));
                        }
                        if session.report {
                            state.original_chain =
                                self.relations.session(session).error.chain.clone();
                        }
                    }
                    if matches!(
                        self.relations.session(session).kind,
                        RelationKind::Assignable | RelationKind::Comparable
                    ) {
                        let object = sequence
                            .read(self.base_constraint_completion(target.object_type, consumer))?
                            .unwrap_or(target.object_type);
                        let index = sequence
                            .read(self.base_constraint_completion(target.index_type, consumer))?
                            .unwrap_or(target.index_type);
                        if !sequence
                            .read(self.is_generic_object_type_completion(object, consumer))?
                            && !sequence
                                .read(self.is_generic_index_type_completion(index, consumer))?
                        {
                            let flags = AccessFlags::WRITING
                                | if object != target.object_type {
                                    AccessFlags::NO_INDEX_SIGNATURES
                                } else {
                                    AccessFlags::NONE
                                };
                            if let Some(constraint) = sequence.read(
                                self.indexed_access_without_node_optional_completion(
                                    object, index, flags, consumer,
                                ),
                            )? {
                                if session.report && state.original_chain.is_some() {
                                    self.relations.session_mut(session).error =
                                        state.saved_error.clone();
                                }
                                let result = sequence.read(self.related_in_session_completion(
                                    source,
                                    constraint,
                                    RecursionFlags::TARGET,
                                    intersection,
                                    session,
                                    consumer,
                                ))?;
                                if result != Ternary::False {
                                    return Ok(Some(result));
                                }
                                if session.report
                                    && let Some(original) = &state.original_chain
                                {
                                    let original_state = ErrorState {
                                        chain: Some(original.clone()),
                                    };
                                    let error = &mut self.relations.session_mut(session).error;
                                    if error.chain.is_some()
                                        && original_state.depth() <= error.depth()
                                    {
                                        error.chain = Some(original.clone());
                                    }
                                }
                            }
                        }
                    }
                    if session.report {
                        state.original_chain = None;
                    }
                }
                TypeView::Index(target) => {
                    if let TypeView::Index(source) = self.model.get(source)? {
                        let result = sequence.read(self.related_in_session_completion(
                            target.target,
                            source.target,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session.reporting(false),
                            consumer,
                        ))?;
                        if result != Ternary::False {
                            return Ok(Some(result));
                        }
                    }
                    if self.model.is_tuple_type(target.target) {
                        let keys = sequence.read(
                            self.known_keys_of_tuple_type_completion(target.target, consumer),
                        )?;
                        let result = sequence.read(self.related_in_session_completion(
                            source,
                            keys,
                            RecursionFlags::TARGET,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))?;
                        if result != Ternary::False {
                            return Ok(Some(result));
                        }
                    } else if let Some(constraint) = sequence.read(
                        self.simplified_type_or_constraint_completion(target.target, consumer),
                    )? {
                        let keys = sequence.read(self.index_type_ex_completion(
                            constraint,
                            target.index_flags | IndexFlags::NO_REDUCIBLE_CHECK,
                            consumer,
                        ))?;
                        if sequence.read(self.related_in_session_completion(
                            source,
                            keys,
                            RecursionFlags::TARGET,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))? == Ternary::True
                        {
                            return Ok(Some(Ternary::True));
                        }
                    } else if sequence
                        .read(self.is_generic_mapped_type_completion(target.target, consumer))?
                    {
                        let name =
                            sequence.read(self.mapped_name_completion(target.target, consumer))?;
                        let constraint = sequence
                            .read(self.mapped_constraint_completion(target.target, consumer))?;
                        let keys = if let Some(name) = name {
                            if self.mapped_recipe(target.target)?.keyof_operand.is_some() {
                                let apparent =
                                    sequence.read(self.apparent_mapped_type_keys_completion(
                                        name,
                                        target.target,
                                        consumer,
                                    ))?;
                                sequence.read(self.union_type_ex_completion(
                                    vec![apparent, name],
                                    None,
                                    None,
                                    consumer,
                                ))?
                            } else {
                                name
                            }
                        } else {
                            constraint
                        };
                        let result = sequence.read(self.related_in_session_completion(
                            source,
                            keys,
                            RecursionFlags::TARGET,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))?;
                        if result == Ternary::True {
                            return Ok(Some(Ternary::True));
                        }
                    }
                }
                _ => {}
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// Identity's `structuredTypeRelatedToWorker` uses the same active
    /// relation session, but exact property/signature/index questions.
    fn structured_identity_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            if let Some(sources) = self
                .model
                .union_constituents(source)
                .or_else(|| self.model.intersection_constituents(source))
            {
                let targets = self
                    .model
                    .union_constituents(target)
                    .or_else(|| self.model.intersection_constituents(target))
                    .ok_or_else(|| {
                        super::super::internal("equal composite flags lack target constituents")
                    })?;
                let mut result = Ternary::True;
                for (sources, targets) in [(&sources, &targets), (&targets, &sources)] {
                    for &source in sources.iter() {
                        let mut related = Ternary::False;
                        for &target in targets.iter() {
                            related = sequence.read(self.related_in_session_completion(
                                source,
                                target,
                                RecursionFlags::BOTH,
                                IntersectionState::NONE,
                                session,
                                consumer,
                            ))?;
                            if related != Ternary::False {
                                break;
                            }
                        }
                        if related == Ternary::False {
                            return Ok(related);
                        }
                        result = result.and(related);
                    }
                }
                return Ok(result);
            }
            match (self.model.get(source)?, self.model.get(target)?) {
                (TypeView::Conditional(source_view), TypeView::Conditional(target_view)) => {
                    if self.conditional_is_distributive(source)?
                        != self.conditional_is_distributive(target)?
                    {
                        return Ok(Ternary::False);
                    }
                    let mut result = sequence.read(self.related_in_session_completion(
                        source_view.check_type,
                        target_view.check_type,
                        RecursionFlags::BOTH,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    if result != Ternary::False {
                        result = result.and(sequence.read(self.related_in_session_completion(
                            source_view.extends_type,
                            target_view.extends_type,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))?);
                    }
                    if result != Ternary::False {
                        let source_true = sequence
                            .read(self.conditional_true_type_completion(source, consumer))?;
                        let target_true = sequence
                            .read(self.conditional_true_type_completion(target, consumer))?;
                        result = result.and(sequence.read(self.related_in_session_completion(
                            source_true,
                            target_true,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))?);
                    }
                    if result != Ternary::False {
                        let source_false = sequence
                            .read(self.conditional_false_type_completion(source, consumer))?;
                        let target_false = sequence
                            .read(self.conditional_false_type_completion(target, consumer))?;
                        result = result.and(sequence.read(self.related_in_session_completion(
                            source_false,
                            target_false,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))?);
                    }
                    return Ok(result);
                }
                (TypeView::Substitution(source), TypeView::Substitution(target)) => {
                    let result = sequence.read(self.related_in_session_completion(
                        source.base,
                        target.base,
                        RecursionFlags::BOTH,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    return if result == Ternary::False {
                        Ok(result)
                    } else {
                        Ok(result.and(sequence.read(self.related_in_session_completion(
                            source.constraint,
                            target.constraint,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))?))
                    };
                }
                (TypeView::TemplateLiteral(source), TypeView::TemplateLiteral(target)) => {
                    if !source
                        .semantic_texts_equal(&target)
                        .map_err(|error| self.literal_value_failure(error, consumer))?
                    {
                        return Ok(Ternary::False);
                    }
                    let mut result = Ternary::True;
                    for (&source, &target) in source.types.iter().zip(target.types.iter()) {
                        result = result.and(sequence.read(self.related_in_session_completion(
                            source,
                            target,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))?);
                        if result == Ternary::False {
                            break;
                        }
                    }
                    return Ok(result);
                }
                (TypeView::StringMapping(source), TypeView::StringMapping(target)) => {
                    return if source.symbol == target.symbol {
                        sequence.read(self.related_in_session_completion(
                            source.target,
                            target.target,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))
                    } else {
                        Ok(Ternary::False)
                    };
                }
                (TypeView::Index(source), TypeView::Index(target)) => {
                    return sequence.read(self.related_in_session_completion(
                        source.target,
                        target.target,
                        RecursionFlags::BOTH,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ));
                }
                (TypeView::IndexedAccess(source), TypeView::IndexedAccess(target)) => {
                    let mut result = sequence.read(self.related_in_session_completion(
                        source.object_type,
                        target.object_type,
                        RecursionFlags::BOTH,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    if result != Ternary::False {
                        result = result.and(sequence.read(self.related_in_session_completion(
                            source.index_type,
                            target.index_type,
                            RecursionFlags::BOTH,
                            IntersectionState::NONE,
                            session,
                            consumer,
                        ))?);
                    }
                    return Ok(result);
                }
                _ => {}
            }
            if !self.model.has_type_flags(source, TypeFlags::OBJECT) {
                return Ok(Ternary::False);
            }
            let mut state = StructuredComparison {
                comparer: RelationComparer {
                    session: session.reporting(false),
                    intersection: IntersectionState::NONE,
                },
                saved_error: self.relations.session(session).error.clone(),
                variance_failed: false,
                original_chain: None,
            };
            if let Some(result) = sequence.read(
                self.relation_alias_variances_completion(source, target, &mut state, consumer),
            )? {
                return Ok(result);
            }
            if let Some(result) = sequence.read(
                self.single_generic_tuple_relation_completion(source, target, session, consumer),
            )? {
                return Ok(result);
            }
            if sequence.read(self.is_generic_mapped_type_completion(target, consumer))? {
                return if sequence.read(self.is_generic_mapped_type_completion(source, consumer))? {
                    sequence.read(
                        self.mapped_type_related_completion(source, target, session, consumer),
                    )
                } else {
                    Ok(Ternary::False)
                };
            }
            if sequence.read(self.is_generic_mapped_type_completion(source, consumer))? {
                return Ok(Ternary::False);
            }
            let reference_alternatives = match sequence.read(
                self.relation_reference_variances_completion(source, target, &mut state, consumer),
            )? {
                ReferenceRelation::Related(result) => return Ok(result),
                ReferenceRelation::Structural => false,
                ReferenceRelation::NotApplicable => true,
            };
            if reference_alternatives
                && sequence.read(self.array_relation_candidate_completion(source, target))?
            {
                return Ok(Ternary::False);
            }
            if reference_alternatives
                && let Some(constraint) = sequence
                    .read(self.tuple_relation_constraint_completion(source, target, consumer))?
                && constraint != source
            {
                return sequence.read(self.related_in_session_completion(
                    constraint,
                    target,
                    RecursionFlags::SOURCE,
                    IntersectionState::NONE,
                    session,
                    consumer,
                ));
            }
            let source_properties =
                sequence.read(self.properties_of_object_type_completion(source, consumer))?;
            let target_properties =
                sequence.read(self.properties_of_object_type_completion(target, consumer))?;
            if source_properties.len() != target_properties.len() {
                return Ok(Ternary::False);
            }
            let mut result = Ternary::True;
            for source_property in source_properties {
                let source_symbol = self.property_symbol(&source_property, consumer)?;
                let name = self.property_lookup_key(source_symbol, consumer)?;
                let Some(target_property) = sequence.read(
                    self.property_of_object_type_by_key_completion(target, &name, consumer),
                )?
                else {
                    return Ok(Ternary::False);
                };
                let target_symbol = self.property_symbol(&target_property, consumer)?;
                let related = sequence.read(self.compare_properties_completion(
                    source_symbol,
                    target_symbol,
                    TypeComparer::Relation(session),
                    consumer,
                ))?;
                if related == Ternary::False {
                    return Ok(related);
                }
                result = result.and(related);
            }
            for construct in [false, true] {
                let sources = if construct {
                    sequence.read(self.construct_signatures_completion(source, consumer))?
                } else {
                    sequence.read(self.call_signatures_completion(source, consumer))?
                };
                let targets = if construct {
                    sequence.read(self.construct_signatures_completion(target, consumer))?
                } else {
                    sequence.read(self.call_signatures_completion(target, consumer))?
                };
                if sources.len() != targets.len() {
                    return Ok(Ternary::False);
                }
                for (source, target) in sources.iter().zip(&targets) {
                    let related = sequence.read(self.compare_signatures_with_completion(
                        source,
                        target,
                        SignatureComparison::default(),
                        TypeComparer::Relation(session),
                        consumer,
                    ))?;
                    if related == Ternary::False {
                        return Ok(related);
                    }
                    result = result.and(related);
                }
            }
            let sources = sequence.read(self.index_infos_of_type_completion(source, consumer))?;
            let targets = sequence.read(self.index_infos_of_type_completion(target, consumer))?;
            if sources.len() != targets.len() {
                return Ok(Ternary::False);
            }
            for target in targets {
                let key = sequence.read(self.demand_completion(target.key_type))?;
                let Some(source) =
                    sequence.read(self.index_info_of_type_completion(source, key, consumer))?
                else {
                    return Ok(Ternary::False);
                };
                let source_type = sequence.read(self.demand_completion(source.value_type))?;
                let target_type = sequence.read(self.demand_completion(target.value_type))?;
                if sequence.read(self.related_in_session_completion(
                    source_type,
                    target_type,
                    RecursionFlags::BOTH,
                    IntersectionState::NONE,
                    session,
                    consumer,
                ))? == Ternary::False
                    || source.readonly != target.readonly
                {
                    return Ok(Ternary::False);
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `compareProperties` with `compareTypesEqual`, used when combining
    /// properties whose non-missing types must have the same native identity.
    pub(super) fn compare_properties_equal_completion(
        &mut self,
        source: PropertySymbolId,
        target: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = sequence
            .read(self.compare_properties_completion(source, target, TypeComparer::Equal, consumer))
            .map(|result| result == Ternary::True);
        sequence.finish(result)
    }

    /// `compareProperties`: nonpublic identity asks original target symbols;
    /// public identity asks optionality, and both then ask readonly and types.
    /// The comparer changes only the final type question, after these gates.
    fn compare_properties_completion(
        &mut self,
        source: PropertySymbolId,
        target: PropertySymbolId,
        comparer: TypeComparer,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            if source == target {
                return Ok(Ternary::True);
            }
            let source_access = self.property_declaration_modifiers(source, false, consumer)?
                & (modifiers::PRIVATE | modifiers::PROTECTED);
            let target_access = self.property_declaration_modifiers(target, false, consumer)?
                & (modifiers::PRIVATE | modifiers::PROTECTED);
            if source_access != target_access {
                return Ok(Ternary::False);
            }
            if source_access != 0 {
                if self.property_target_symbol(source)? != self.property_target_symbol(target)? {
                    return Ok(Ternary::False);
                }
            } else if self.property_symbol_flags(source)?.optional
                != self.property_symbol_flags(target)?.optional
            {
                return Ok(Ternary::False);
            }
            if self.property_is_readonly(source, consumer)?
                != self.property_is_readonly(target, consumer)?
            {
                return Ok(Ternary::False);
            }
            let source =
                sequence.read(self.non_missing_property_type_completion(source, consumer))?;
            let target =
                sequence.read(self.non_missing_property_type_completion(target, consumer))?;
            sequence.read(self.compare_types_with_completion(source, target, comparer, consumer))
        })();
        sequence.finish(result)
    }

    /// Native comparison callbacks have distinct session boundaries. Exact
    /// equality observes IDs; compareTypesIdentical enters isTypeRelatedTo for
    /// each pair; isRelatedToSimple reuses its caller's active Relater.
    fn compare_types_with_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        comparer: TypeComparer,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            match comparer {
                TypeComparer::Equal => Ok(if source == target {
                    Ternary::True
                } else {
                    Ternary::False
                }),
                TypeComparer::Independent(relation) => Ok(
                    if sequence.read(
                        self.raw_type_related_completion(source, target, relation, consumer),
                    )? {
                        Ternary::True
                    } else {
                        Ternary::False
                    },
                ),
                TypeComparer::Relation(session) => {
                    sequence.read(self.related_in_session_completion(
                        source,
                        target,
                        RecursionFlags::BOTH,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))
                }
            }
        })();
        sequence.finish(result)
    }

    /// `findMatchingSignature` selects compareTypesSubtypeOf for partial
    /// matching and compareTypesIdentical otherwise. The first accepted
    /// signature retains its actual factory identity.
    pub(super) fn find_matching_signature_completion(
        &mut self,
        signatures: &[Signature],
        candidate: &Signature,
        options: SignatureComparison,
        consumer: &Location,
    ) -> NativeResult<Option<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Signature>, DemandError> {
            let comparer = TypeComparer::Independent(if options.partial_match {
                RelationKind::Subtype
            } else {
                RelationKind::Identity
            });
            for signature in signatures {
                if sequence.read(self.compare_signatures_with_completion(
                    signature, candidate, options, comparer, consumer,
                ))? != Ternary::False
                {
                    return Ok(Some(signature.clone()));
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `compareSignaturesIdentical` with the checker's independent identity
    /// callback; live-relater callers retain their separate comparer below.
    pub(super) fn compare_signatures_identical_completion(
        &mut self,
        source: &Signature,
        target: &Signature,
        options: SignatureComparison,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = sequence
            .read(self.compare_signatures_with_completion(
                source,
                target,
                options,
                TypeComparer::Independent(RelationKind::Identity),
                consumer,
            ))
            .map(|result| result != Ternary::False);
        sequence.finish(result)
    }

    /// `isMatchingSignature` acquires both counts, minima and effective-rest
    /// facts before deciding exact or partial arity. Partial matching does not
    /// impose the exact parameter-count/rest restrictions.
    fn is_matching_signature_completion(
        &mut self,
        source: &Signature,
        target: &Signature,
        partial_match: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let source_count =
                sequence.read(self.signature_parameter_count_completion(source, consumer))?;
            let target_count =
                sequence.read(self.signature_parameter_count_completion(target, consumer))?;
            let source_minimum =
                sequence.read(self.minimum_argument_count_completion(source, consumer))?;
            let target_minimum =
                sequence.read(self.minimum_argument_count_completion(target, consumer))?;
            let source_rest =
                sequence.read(self.signature_has_effective_rest_completion(source, consumer))?;
            let target_rest =
                sequence.read(self.signature_has_effective_rest_completion(target, consumer))?;
            Ok(source_count == target_count
                && source_minimum == target_minimum
                && source_rest == target_rest
                || partial_match && source_minimum <= target_minimum)
        })();
        sequence.finish(result)
    }

    /// `compareSignaturesIdentical`: constraints and defaults precede source
    /// instantiation; this and returns are skipped only by their independent
    /// options. Every client shares these gates and the ordered parameter walk.
    fn compare_signatures_with_completion(
        &mut self,
        source: &Signature,
        target: &Signature,
        options: SignatureComparison,
        comparer: TypeComparer,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            if source.id == target.id {
                return Ok(Ternary::True);
            }
            if !sequence.read(self.is_matching_signature_completion(
                source,
                target,
                options.partial_match,
                consumer,
            ))? || source.type_parameter_types().len() != target.type_parameter_types().len()
            {
                return Ok(Ternary::False);
            }
            let mut source = source.clone();
            if !target.type_parameter_types().is_empty() {
                let mapper = self.model.mapper(
                    source.type_parameter_types().to_vec(),
                    target.type_parameter_types().to_vec(),
                )?;
                for (&source, &target) in source
                    .type_parameter_types()
                    .iter()
                    .zip(target.type_parameter_types().iter())
                {
                    if source == target {
                        continue;
                    }
                    let constraint =
                        sequence.read(self.parameter_constraint_or_unknown_completion(source))?;
                    let constraint = self.instantiate_type(constraint, mapper, consumer)?;
                    let target_constraint =
                        sequence.read(self.parameter_constraint_or_unknown_completion(target))?;
                    if sequence.read(self.compare_types_with_completion(
                        constraint,
                        target_constraint,
                        comparer,
                        consumer,
                    ))? == Ternary::False
                    {
                        return Ok(Ternary::False);
                    }
                    let source_default =
                        sequence.read(self.parameter_default_or_unknown_completion(source))?;
                    let source_default = self.instantiate_type(source_default, mapper, consumer)?;
                    let target_default =
                        sequence.read(self.parameter_default_or_unknown_completion(target))?;
                    if sequence.read(self.compare_types_with_completion(
                        source_default,
                        target_default,
                        comparer,
                        consumer,
                    ))? == Ternary::False
                    {
                        return Ok(Ternary::False);
                    }
                }
                source = sequence.read(
                    self.instantiate_signature_ex_completion(&source, mapper, true, consumer),
                )?;
            }
            let mut result = Ternary::True;
            if !options.ignore_this_types {
                if let Some(source_this) =
                    sequence.read(self.signature_this_type_completion(&source))?
                {
                    if let Some(target_this) =
                        sequence.read(self.signature_this_type_completion(target))?
                    {
                        let related = sequence.read(self.compare_types_with_completion(
                            source_this,
                            target_this,
                            comparer,
                            consumer,
                        ))?;
                        if related == Ternary::False {
                            return Ok(related);
                        }
                        result = result.and(related);
                    }
                }
            }
            for index in
                0..sequence.read(self.signature_parameter_count_completion(target, consumer))?
            {
                let source_type = sequence.read(
                    self.signature_type_at_position_or_any_completion(&source, index, consumer),
                )?;
                let target_type = sequence.read(
                    self.signature_type_at_position_or_any_completion(target, index, consumer),
                )?;
                let related = sequence.read(self.compare_types_with_completion(
                    target_type,
                    source_type,
                    comparer,
                    consumer,
                ))?;
                if related == Ternary::False {
                    return Ok(related);
                }
                result = result.and(related);
            }
            if !options.ignore_return_types {
                let source_predicate = self.type_predicate_of_signature(&source, consumer)?;
                let target_predicate = self.type_predicate_of_signature(target, consumer)?;
                let related = match (source_predicate, target_predicate) {
                    (None, None) => {
                        let source =
                            sequence.read(self.raw_return_type_of_signature_completion(&source))?;
                        let target =
                            sequence.read(self.raw_return_type_of_signature_completion(target))?;
                        sequence.read(
                            self.compare_types_with_completion(source, target, comparer, consumer),
                        )?
                    }
                    (Some(source), Some(target))
                        if super::signatures::predicate_kinds_match(&source, &target) =>
                    {
                        match (source.value_type, target.value_type) {
                            (None, None) => Ternary::True,
                            (Some(source), Some(target)) => {
                                let source = sequence.read(self.demand_completion(source))?;
                                let target = sequence.read(self.demand_completion(target))?;
                                sequence.read(self.compare_types_with_completion(
                                    source, target, comparer, consumer,
                                ))?
                            }
                            _ => Ternary::False,
                        }
                    }
                    _ => Ternary::False,
                };
                result = result.and(related);
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `compareTypeParametersIdentical` maps target constraints into the
    /// source identity domain. Defaults deliberately do not participate: union
    /// signatures retain the first matching signature's defaults.
    pub(super) fn compare_type_parameters_identical_completion(
        &mut self,
        source: &[TypeId],
        target: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if source.len() != target.len() {
                return Ok(false);
            }
            let mapper = self.model.mapper(target.to_vec(), source.to_vec())?;
            for (&source, &target) in source.iter().zip(target) {
                if source == target {
                    continue;
                }
                let source =
                    sequence.read(self.parameter_constraint_or_unknown_completion(source))?;
                let target =
                    sequence.read(self.parameter_constraint_or_unknown_completion(target))?;
                let target = self.instantiate_type(target, mapper, consumer)?;
                if !sequence.read(self.raw_type_related_completion(
                    source,
                    target,
                    RelationKind::Identity,
                    consumer,
                ))? {
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        sequence.finish(result)
    }

    /// `getConstraintOrUnknownFromTypeParameter` deliberately uses the raw
    /// constraint question, without getConstraintOfTypeParameter's base walk.
    fn parameter_constraint_or_unknown_completion(&mut self, ty: TypeId) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            Ok(sequence
                .read(self.raw_constraint_completion(ty))?
                .unwrap_or_else(|| self.model.special(SpecialName::Unknown)))
        })();
        sequence.finish(result)
    }

    /// `getDefaultOrUnknownFromTypeParameter` observes the existing default
    /// demand; absent defaults supply the canonical Unknown type.
    fn parameter_default_or_unknown_completion(&mut self, ty: TypeId) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            Ok(sequence
                .read(self.default_from_type_parameter_completion(ty))?
                .unwrap_or_else(|| self.model.special(SpecialName::Unknown)))
        })();
        sequence.finish(result)
    }

    /// `propertiesRelatedTo` / `getUnmatchedPropertiesWorker`: the first pass
    /// asks ordinary property lookup, including its global augmentations.
    /// The second pass skips actual symbol identity before any member read.
    fn properties_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        excluded: &HashSet<NativePropertyKey>,
        optionals_only: bool,
        comparer: RelationComparer,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let session = comparer.session;
            if let Some(related) = sequence.read(self.tuple_properties_related_completion(
                source, target, excluded, comparer, consumer,
            ))? {
                return Ok(related);
            }
            let require_optional = matches!(
                self.relations.session(session).kind,
                RelationKind::Subtype | RelationKind::StrictSubtype
            ) && !self
                .model
                .object_flags(source)?
                .intersects(ObjectFlags::OBJECT_LITERAL)
                && !self.is_empty_array_literal_type(source)?
                && !self.model.is_tuple_type(source);
            if let Some(unmatched) = sequence.read(self.unmatched_property_completion(
                source,
                target,
                require_optional,
                false,
                consumer,
            ))? {
                if session.report
                    && sequence.read(
                        self.should_report_unmatched_property_completion(source, target, consumer),
                    )?
                {
                    let symbol = self.property_symbol(&unmatched, consumer)?;
                    sequence.read(self.report_unmatched_property_completion(
                        source,
                        target,
                        symbol,
                        require_optional,
                        session,
                        consumer,
                    ))?;
                }
                return Ok(Ternary::False);
            }
            if self
                .model
                .object_flags(target)?
                .intersects(ObjectFlags::OBJECT_LITERAL)
            {
                for property in self.properties_of_type(source, consumer)? {
                    let symbol = self.property_symbol(&property, consumer)?;
                    let name = self.property_lookup_key(symbol, consumer)?;
                    if excluded.contains(&name) {
                        continue;
                    }
                    if self
                        .property_of_object_type_by_key(target, &name, consumer)?
                        .is_none()
                    {
                        if session.report {
                            sequence.read(self.report_property_relation_completion(
                                Message::PropertyDoesNotExist,
                                symbol,
                                &[target],
                                session,
                                consumer,
                            ))?;
                        }
                        return Ok(Ternary::False);
                    }
                }
            }
            let properties = self.properties_of_type(target, consumer)?;
            let numeric_names_only =
                self.model.is_tuple_type(source) && self.model.is_tuple_type(target);
            let mut result = Ternary::True;
            for target_property in &properties {
                let target_symbol = self.property_symbol(target_property, consumer)?;
                let name = self.property_lookup_key(target_symbol, consumer)?;
                if excluded.contains(&name) {
                    continue;
                }
                let flags = self.property_symbol_flags(target_symbol)?;
                if flags.prototype || optionals_only && !flags.optional {
                    continue;
                }
                if numeric_names_only
                    && name.fixed_bytes() != Some(b"length".as_slice())
                    && !self.literal_is_numeric_name(
                        &super::super::model::LiteralValue::from_property_key(&name),
                        consumer,
                    )?
                {
                    continue;
                }
                if let Some(source_property) =
                    self.property_of_type_by_key(source, &name, consumer)?
                {
                    let source_symbol = self.property_symbol(&source_property, consumer)?;
                    if source_symbol == target_symbol {
                        continue;
                    }
                    let related = sequence.read(self.property_related_completion(
                        (source, source_symbol),
                        (target, target_symbol),
                        None,
                        comparer,
                        self.relations.session(session).kind == RelationKind::Comparable,
                        consumer,
                    ))?;
                    if related == Ternary::False {
                        return Ok(related);
                    }
                    result = result.and(related);
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// Tuple branch of `propertiesRelatedTo`: shape and mutability precede
    /// argument acquisition, then aligned element flags control each demand.
    fn tuple_properties_related_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        excluded: &HashSet<NativePropertyKey>,
        comparer: RelationComparer,
        consumer: &Location,
    ) -> NativeResult<Option<Ternary>> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Ternary>, DemandError> {
            let Some(target_tuple) = self.model.tuple_target(target)? else {
                return Ok(None);
            };
            if !self.is_array_or_tuple_type(source)? {
                return Ok(target_tuple
                    .combined_flags
                    .intersects(TupleElementFlags::VARIABLE)
                    .then_some(Ternary::False));
            }
            let source_tuple = self.model.tuple_target(source)?;
            if !target_tuple.readonly && !self.is_mutable_array_or_tuple(source)? {
                return Ok(Some(Ternary::False));
            }
            let source_arity = self.type_reference_arity(source)?;
            let target_arity = self.type_reference_arity(target)?;
            let source_rest = source_tuple
                .as_ref()
                .is_none_or(|tuple| tuple.combined_flags.intersects(TupleElementFlags::REST));
            let target_rest = target_tuple
                .combined_flags
                .intersects(TupleElementFlags::VARIABLE);
            let source_minimum = source_tuple
                .as_ref()
                .map_or(0, |tuple| tuple.minimum_length);
            let shape_error = if !source_rest && source_arity < target_tuple.minimum_length {
                Some((
                    Message::TupleTooShort,
                    vec![source_arity, target_tuple.minimum_length],
                ))
            } else if !target_rest && target_arity < source_minimum {
                Some((Message::TupleTooLong, vec![source_minimum, target_arity]))
            } else if !target_rest && (source_rest || target_arity < source_arity) {
                Some(if source_minimum < target_tuple.minimum_length {
                    (
                        Message::TupleMayBeShorter,
                        vec![target_tuple.minimum_length],
                    )
                } else {
                    (Message::TupleMayBeLonger, vec![target_arity])
                })
            } else {
                None
            };
            if let Some((message, positions)) = shape_error {
                if comparer.session.report {
                    self.relations.session_mut(comparer.session).error.report(
                        message,
                        positions
                            .into_iter()
                            .map(|position| Argument::Number(position as isize))
                            .collect(),
                    );
                }
                return Ok(Some(Ternary::False));
            }
            let source_arguments = sequence.read(self.get_type_arguments_completion(source))?;
            let target_arguments = sequence.read(self.get_type_arguments_completion(target))?;
            let non_rest = TupleElementFlags::FIXED | TupleElementFlags::VARIADIC;
            let target_start = target_tuple.start_element_count(non_rest);
            let target_end = target_tuple.end_element_count(non_rest);
            let mut result = Ternary::True;
            let mut can_exclude = !excluded.is_empty();
            for source_position in 0..source_arity {
                let source_flags = source_tuple
                    .as_ref()
                    .map_or(TupleElementFlags::REST, |tuple| {
                        tuple.elements[source_position].flags
                    });
                let from_end = source_arity - 1 - source_position;
                let target_position = if target_rest && source_position >= target_start {
                    target_arity.checked_sub(1 + from_end.min(target_end))
                } else {
                    Some(source_position)
                };
                let target_flags = target_position.map_or(TupleElementFlags::NONE, |position| {
                    target_tuple.elements[position].flags
                });
                let flag_error = if target_flags.intersects(TupleElementFlags::VARIADIC)
                    && !source_flags.intersects(TupleElementFlags::VARIADIC)
                {
                    Some((
                        Message::TupleMissingVariadic,
                        vec![target_position.map_or(-1, |position| position as isize)],
                    ))
                } else if source_flags.intersects(TupleElementFlags::VARIADIC)
                    && !target_flags.intersects(TupleElementFlags::VARIABLE)
                {
                    Some((
                        Message::TupleUnexpectedVariadic,
                        vec![
                            source_position as isize,
                            target_position.map_or(-1, |position| position as isize),
                        ],
                    ))
                } else if target_flags.intersects(TupleElementFlags::REQUIRED)
                    && !source_flags.intersects(TupleElementFlags::REQUIRED)
                {
                    Some((
                        Message::TupleMissingRequired,
                        vec![target_position.map_or(-1, |position| position as isize)],
                    ))
                } else {
                    None
                };
                if let Some((message, positions)) = flag_error {
                    if comparer.session.report {
                        self.relations.session_mut(comparer.session).error.report(
                            message,
                            positions.into_iter().map(Argument::Number).collect(),
                        );
                    }
                    return Ok(Some(Ternary::False));
                }
                if can_exclude {
                    if source_flags.intersects(TupleElementFlags::VARIABLE)
                        || target_flags.intersects(TupleElementFlags::VARIABLE)
                    {
                        can_exclude = false;
                    }
                    if can_exclude
                        && excluded.contains(&NativePropertyKey::fixed(
                            source_position.to_string().into_bytes(),
                        ))
                    {
                        continue;
                    }
                }
                let source_type = self
                    .model
                    .remove_missing_type(
                        source_arguments[source_position],
                        source_flags.intersects(TupleElementFlags::OPTIONAL)
                            && target_flags.intersects(TupleElementFlags::OPTIONAL),
                    )
                    .map_err(|failure| failure.at(self.inputs, consumer))?;
                let target_type = target_arguments[target_position.ok_or_else(|| {
                    super::super::internal(
                        "tuple element alignment lacks a target after native flag checks",
                    )
                })?];
                let target_check = if source_flags.intersects(TupleElementFlags::VARIADIC)
                    && target_flags.intersects(TupleElementFlags::REST)
                {
                    self.array_type(target_type, consumer)?
                } else {
                    self.model
                        .remove_missing_type(
                            target_type,
                            target_flags.intersects(TupleElementFlags::OPTIONAL),
                        )
                        .map_err(|failure| failure.at(self.inputs, consumer))?
                };
                let related = sequence.read(self.related_in_session_completion(
                    source_type,
                    target_check,
                    RecursionFlags::BOTH,
                    comparer.intersection,
                    comparer.session,
                    consumer,
                ))?;
                if related == Ternary::False {
                    if comparer.session.report && (target_arity > 1 || source_arity > 1) {
                        let (message, positions) = if target_rest
                            && source_position >= target_start
                            && from_end >= target_end
                            && target_start != source_arity - target_end - 1
                        {
                            (
                                Message::TupleRangeTypes,
                                vec![
                                    target_start,
                                    source_arity - target_end - 1,
                                    target_position.expect("aligned tuple element"),
                                ],
                            )
                        } else {
                            (
                                Message::TupleElementTypes,
                                vec![
                                    source_position,
                                    target_position.expect("aligned tuple element"),
                                ],
                            )
                        };
                        self.relations.session_mut(comparer.session).error.report(
                            message,
                            positions
                                .into_iter()
                                .map(|position| Argument::Number(position as isize))
                                .collect(),
                        );
                    }
                    return Ok(Some(related));
                }
                result = result.and(related);
            }
            Ok(Some(result))
        })();
        sequence.finish(result)
    }

    /// `propertyRelatedTo` keeps modifiers, private declarations, readonly,
    /// target-first non-missing read and optional rejection in native order.
    fn property_related_completion(
        &mut self,
        (source_type, source): (TypeId, PropertySymbolId),
        (target_type, target): (TypeId, PropertySymbolId),
        source_override: Option<TypeId>,
        comparer: RelationComparer,
        skip_optional: bool,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let RelationComparer {
                intersection,
                session,
            } = comparer;
            let source_modifiers = self.property_declaration_modifiers(source, false, consumer)?;
            let target_modifiers = self.property_declaration_modifiers(target, false, consumer)?;
            if (source_modifiers | target_modifiers) & modifiers::PRIVATE != 0 {
                if self.property_value_declaration_identity(source, consumer)?
                    != self.property_value_declaration_identity(target, consumer)?
                {
                    if session.report {
                        if source_modifiers & target_modifiers & modifiers::PRIVATE != 0 {
                            sequence.read(self.report_property_relation_completion(
                                Message::SeparatePrivateDeclarations,
                                target,
                                &[],
                                session,
                                consumer,
                            ))?;
                        } else {
                            let types = if source_modifiers & modifiers::PRIVATE != 0 {
                                [source_type, target_type]
                            } else {
                                [target_type, source_type]
                            };
                            sequence.read(self.report_property_relation_completion(
                                Message::PrivateProperty,
                                target,
                                &types,
                                session,
                                consumer,
                            ))?;
                        }
                    }
                    return Ok(Ternary::False);
                }
            } else if target_modifiers & modifiers::PROTECTED != 0 {
                if !sequence.read(self.is_valid_override_of_completion(source, target, consumer))? {
                    if session.report {
                        let source_class = sequence
                            .read(self.property_declaring_class_completion(source, consumer))?
                            .unwrap_or(source_type);
                        let target_class = sequence
                            .read(self.property_declaring_class_completion(target, consumer))?
                            .unwrap_or(target_type);
                        sequence.read(self.report_property_relation_completion(
                            Message::ProtectedNotDerived,
                            target,
                            &[source_class, target_class],
                            session,
                            consumer,
                        ))?;
                    }
                    return Ok(Ternary::False);
                }
            } else if source_modifiers & modifiers::PROTECTED != 0 {
                if session.report {
                    sequence.read(self.report_property_relation_completion(
                        Message::ProtectedProperty,
                        target,
                        &[source_type, target_type],
                        session,
                        consumer,
                    ))?;
                }
                return Ok(Ternary::False);
            }
            if self.relations.session(session).kind == RelationKind::StrictSubtype
                && self.property_is_readonly(source, consumer)?
                && !self.property_is_readonly(target, consumer)?
            {
                return Ok(Ternary::False);
            }
            let target_flags = self.property_symbol_flags(target)?;
            let effective_target =
                sequence.read(self.non_missing_property_type_completion(target, consumer))?;
            let effective_target = self.add_optionality(
                effective_target,
                false,
                self.inputs.semantic_options().strict_null_checks && target_flags.partial,
                consumer,
            )?;
            let shortcut = if self.relations.session(session).kind == RelationKind::StrictSubtype {
                TypeFlags::ANY
            } else {
                TypeFlags::ANY | TypeFlags::UNKNOWN
            };
            let related = if self.model.has_type_flags(effective_target, shortcut) {
                Ternary::True
            } else {
                let effective_source = match source_override {
                    Some(ty) => ty,
                    None => sequence
                        .read(self.non_missing_property_type_completion(source, consumer))?,
                };
                sequence.read(self.related_in_session_completion(
                    effective_source,
                    effective_target,
                    RecursionFlags::BOTH,
                    intersection,
                    session,
                    consumer,
                ))?
            };
            if related == Ternary::False {
                if session.report {
                    sequence.read(self.report_property_relation_completion(
                        Message::PropertyTypes,
                        target,
                        &[],
                        session,
                        consumer,
                    ))?;
                }
                return Ok(related);
            }
            if !skip_optional
                && self.property_symbol_flags(source)?.optional
                && target_flags.class_member()
                && !target_flags.optional
            {
                if session.report {
                    sequence.read(self.report_property_relation_completion(
                        Message::OptionalProperty,
                        target,
                        &[source_type, target_type],
                        session,
                        consumer,
                    ))?;
                }
                return Ok(Ternary::False);
            }
            Ok(related)
        })();
        sequence.finish(result)
    }

    /// `isValidOverrideOf` checks every protected target declaration through
    /// the property arena's native synthetic-property traversal. One failed
    /// protected origin rejects the override before either value is read.
    fn is_valid_override_of_completion(
        &mut self,
        source: PropertySymbolId,
        target: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        Ok(self
            .for_each_property_completion(target, consumer, &mut |this, target| {
                if this.property_declaration_modifiers(target, false, consumer)?
                    & modifiers::PROTECTED
                    == 0
                {
                    return Ok(NativeCompletion::clean(false));
                }
                this.property_declaring_class_completion(target, consumer)?
                    .and_then(|base| {
                        Ok(this
                            .is_property_in_class_derived_from_completion(source, base, consumer)?
                            .map(|derived| !derived))
                    })
            })?
            .map(|invalid| !invalid))
    }

    /// `isPropertyInClassDerivedFrom` keeps each declaring-class acquisition
    /// before the native base traversal and stops at its first true result.
    pub(super) fn is_property_in_class_derived_from_completion(
        &mut self,
        property: PropertySymbolId,
        base: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.for_each_property_completion(property, consumer, &mut |this, property| {
            this.property_declaring_class_completion(property, consumer)?
                .and_then(|class| match class {
                    Some(class) => this.has_base_type_completion(class, base, consumer),
                    None => Ok(NativeCompletion::clean(false)),
                })
        })
    }

    /// `getUnmatchedProperty` keeps the target-first required/discriminant
    /// demand order shared by structured relations and inference pruning.
    pub(super) fn unmatched_property_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        require_optional: bool,
        discriminants: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        let mut sequence = NativeSequence::new();
        let result = sequence.read(self.unmatched_properties_worker_completion(
            source,
            target,
            require_optional,
            discriminants,
            None,
            consumer,
        ));
        sequence.finish(result)
    }

    /// `getUnmatchedProperties` visits the full native target inventory;
    /// getUnmatchedProperty uses the same worker's first-match short circuit.
    fn unmatched_properties_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        require_optional: bool,
        discriminants: bool,
        consumer: &Location,
    ) -> NativeResult<Vec<Property>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Property>, DemandError> {
            let mut properties = Vec::new();
            sequence.read(self.unmatched_properties_worker_completion(
                source,
                target,
                require_optional,
                discriminants,
                Some(&mut properties),
                consumer,
            ))?;
            Ok(properties)
        })();
        sequence.finish(result)
    }

    /// `getUnmatchedPropertiesWorker` visits each target property, including
    /// private and optional skips; only an actual unmatched result is retained.
    fn unmatched_properties_worker_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        require_optional: bool,
        discriminants: bool,
        mut properties: Option<&mut Vec<Property>>,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Property>, DemandError> {
            for property in sequence.read(self.properties_of_type_completion(target, consumer))? {
                let symbol = self.property_symbol(&property, consumer)?;
                if self.is_static_private_identifier_property(symbol, consumer)? {
                    continue;
                }
                let flags = self.property_symbol_flags(symbol)?;
                if require_optional || !flags.optional && !flags.partial {
                    let name = self.property_lookup_key(symbol, consumer)?;
                    let unmatched = match sequence
                        .read(self.property_of_type_by_key_completion(source, &name, consumer))?
                    {
                        None => true,
                        Some(source_property) if discriminants => {
                            let target_type =
                                sequence.read(self.demand_completion(property.read_type))?;
                            if self.model.has_type_flags(target_type, TypeFlags::UNIT) {
                                let source_type = sequence
                                    .read(self.demand_completion(source_property.read_type))?;
                                !self.model.has_type_flags(source_type, TypeFlags::ANY)
                                    && self.model.regular_literal(source_type)
                                        != self.model.regular_literal(target_type)
                            } else {
                                false
                            }
                        }
                        Some(_) => false,
                    };
                    if unmatched {
                        if let Some(properties) = &mut properties {
                            (*properties).push(property);
                        } else {
                            return Ok(Some(property));
                        }
                    }
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `isObjectTypeWithInferableIndex` uses actual Type.symbol flags before
    /// asking signatures. Interface shape alone never establishes this rule.
    pub(super) fn object_type_with_inferable_index_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if let Some(types) = self.model.intersection_constituents(ty) {
                for &ty in types.iter() {
                    if !sequence
                        .read(self.object_type_with_inferable_index_completion(ty, consumer))?
                    {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            if let Some(flags) = self.type_symbol_flags(ty)? {
                if (flags.object_literal || flags.type_literal || flags.enum_ || flags.value_module)
                    && !flags.class
                    && sequence
                        .read(self.call_signatures_completion(ty, consumer))?
                        .is_empty()
                    && sequence
                        .read(self.construct_signatures_completion(ty, consumer))?
                        .is_empty()
                {
                    return Ok(true);
                }
            }
            let flags = self.model.object_flags(ty)?;
            if flags.intersects(ObjectFlags::JS_LITERAL | ObjectFlags::OBJECT_REST_TYPE) {
                return Ok(true);
            }
            if flags.intersects(ObjectFlags::REVERSE_MAPPED) {
                let source = self.reverse_mapped_source(ty)?;
                return sequence
                    .read(self.object_type_with_inferable_index_completion(source, consumer));
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `signaturesRelatedTo` preserves source/target acquisition, constructor
    /// restrictions, same-symbol pairwise erasure and the target/source matrix.
    fn relation_signatures_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        construct: bool,
        intersection: IntersectionState,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            if source == self.model.any_function_type() {
                return Ok(Ternary::True);
            }
            if target == self.model.any_function_type() {
                return Ok(Ternary::False);
            }
            let source_members = self.structured_type_members(source, consumer)?;
            let target_members = self.structured_type_members(target, consumer)?;
            let source_signatures = source_members.as_ref().map_or(&[][..], |members| {
                if construct {
                    &members.construct_signatures
                } else {
                    &members.call_signatures
                }
            });
            let target_signatures = target_members.as_ref().map_or(&[][..], |members| {
                if construct {
                    &members.construct_signatures
                } else {
                    &members.call_signatures
                }
            });
            if construct && !source_signatures.is_empty() && !target_signatures.is_empty() {
                if source_signatures[0].flags.has_abstract()
                    && !target_signatures[0].flags.has_abstract()
                {
                    if session.report {
                        self.relations
                            .session_mut(session)
                            .error
                            .report(Message::AbstractConstructor, Vec::new());
                    }
                    return Ok(Ternary::False);
                }
                let source_root = self.signature_declaration_root(source_signatures[0].id)?;
                let target_root = self.signature_declaration_root(target_signatures[0].id)?;
                if let (Some(source), Some(target)) = (source_root, target_root) {
                    let source = self.source.signature_declaration_modifiers(source)?
                        & (modifiers::PRIVATE | modifiers::PROTECTED);
                    let target = self.source.signature_declaration_modifiers(target)?
                        & (modifiers::PRIVATE | modifiers::PROTECTED);
                    if !(target == modifiers::PRIVATE
                        || target == modifiers::PROTECTED && source != modifiers::PRIVATE
                        || target != modifiers::PROTECTED && source == 0)
                    {
                        if session.report {
                            let visibility = |flags| match flags {
                                modifiers::PRIVATE => b"private".to_vec(),
                                modifiers::PROTECTED => b"protected".to_vec(),
                                _ => b"public".to_vec(),
                            };
                            self.relations.session_mut(session).error.report(
                                Message::ConstructorVisibility,
                                vec![
                                    Argument::Text(visibility(source)),
                                    Argument::Text(visibility(target)),
                                ],
                            );
                        }
                        return Ok(Ternary::False);
                    }
                }
            }
            let same_instantiation = self
                .model
                .object_flags(source)?
                .intersects(ObjectFlags::INSTANTIATED)
                && self
                    .model
                    .object_flags(target)?
                    .intersects(ObjectFlags::INSTANTIATED)
                && self.type_symbol_identity(source)? == self.type_symbol_identity(target)?;
            let same_reference = match (self.model.get(source)?, self.model.get(target)?) {
                (TypeView::Reference(source), TypeView::Reference(target)) => {
                    source.is_reference && target.is_reference && source.target == target.target
                }
                _ => false,
            };
            let mut result = Ternary::True;
            if same_instantiation || same_reference {
                for (index, target) in target_signatures.iter().enumerate() {
                    let source = source_signatures.get(index).ok_or_else(|| {
                        super::super::internal(
                            "same native signature factory has different signature counts",
                        )
                    })?;
                    let related = sequence.read(self.signature_related_completion(
                        source,
                        target,
                        true,
                        intersection,
                        session,
                        consumer,
                    ))?;
                    if related == Ternary::False {
                        return Ok(related);
                    }
                    result = result.and(related);
                }
            } else if source_signatures.len() == 1 && target_signatures.len() == 1 {
                result = sequence.read(self.signature_related_completion(
                    &source_signatures[0],
                    &target_signatures[0],
                    self.relations.session(session).kind == RelationKind::Comparable,
                    intersection,
                    session,
                    consumer,
                ))?;
            } else {
                for target in target_signatures {
                    let saved_error = self.relations.session(session).error.clone();
                    let mut report = session.report;
                    let mut found = false;
                    for source in source_signatures {
                        let related = sequence.read(self.signature_related_completion(
                            source,
                            target,
                            true,
                            intersection,
                            session.reporting(report),
                            consumer,
                        ))?;
                        if related != Ternary::False {
                            result = result.and(related);
                            self.relations.session_mut(session).error = saved_error.clone();
                            found = true;
                            break;
                        }
                        report = false;
                    }
                    if !found {
                        if report {
                            sequence.read(self.report_no_matching_signature_completion(
                                source, target, session, consumer,
                            ))?;
                        }
                        return Ok(Ternary::False);
                    }
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `signatureRelatedTo` selects native mode and erasure before entering
    /// compareSignaturesRelated. Child type relations retain this session.
    fn signature_related_completion(
        &mut self,
        source: &Signature,
        target: &Signature,
        erase: bool,
        intersection: IntersectionState,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let mode = match self.relations.session(session).kind {
                RelationKind::Identity => {
                    return sequence.read(self.compare_signatures_with_completion(
                        source,
                        target,
                        SignatureComparison::default(),
                        TypeComparer::Relation(session),
                        consumer,
                    ));
                }
                RelationKind::Subtype => SignatureCheckMode(SignatureCheckMode::STRICT_TOP),
                RelationKind::StrictSubtype => SignatureCheckMode(
                    SignatureCheckMode::STRICT_TOP | SignatureCheckMode::STRICT_ARITY,
                ),
                RelationKind::Assignable | RelationKind::Comparable => SignatureCheckMode::NONE,
            };
            let (source, target) = if erase {
                (
                    sequence.read(self.erased_signature_completion(source, consumer))?,
                    sequence.read(self.erased_signature_completion(target, consumer))?,
                )
            } else {
                (source.clone(), target.clone())
            };
            sequence.read(self.compare_signatures_related_completion(
                &source,
                &target,
                mode,
                intersection,
                session,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `compareSignaturesRelated` owns the precise parameter, callback and
    /// target-first return schedule. A False skips later dependencies, while
    /// failures from any visited native question remain failures.
    fn compare_signatures_related_completion(
        &mut self,
        source: &Signature,
        target: &Signature,
        mode: SignatureCheckMode,
        intersection: IntersectionState,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            if source.id == target.id {
                return Ok(Ternary::True);
            }
            if !(mode.has(SignatureCheckMode::STRICT_TOP)
                && sequence.read(self.is_top_signature_completion(source, consumer))?)
                && sequence.read(self.is_top_signature_completion(target, consumer))?
            {
                return Ok(Ternary::True);
            }
            if mode.has(SignatureCheckMode::STRICT_TOP)
                && sequence.read(self.is_top_signature_completion(source, consumer))?
                && !sequence.read(self.is_top_signature_completion(target, consumer))?
            {
                return Ok(Ternary::False);
            }
            let target_count =
                sequence.read(self.signature_parameter_count_completion(target, consumer))?;
            if !sequence.read(self.signature_has_effective_rest_completion(target, consumer))? {
                let more = if mode.has(SignatureCheckMode::STRICT_ARITY) {
                    sequence.read(self.signature_has_effective_rest_completion(source, consumer))?
                        || sequence
                            .read(self.signature_parameter_count_completion(source, consumer))?
                            > target_count
                } else {
                    sequence.read(self.minimum_argument_count_completion(source, consumer))?
                        > target_count
                };
                if more {
                    if session.report && !mode.has(SignatureCheckMode::STRICT_ARITY) {
                        let minimum = sequence
                            .read(self.minimum_argument_count_completion(source, consumer))?;
                        self.relations.session_mut(session).error.report(
                            Message::TooFewArguments,
                            vec![
                                Argument::Number(minimum as isize),
                                Argument::Number(target_count as isize),
                            ],
                        );
                    }
                    return Ok(Ternary::False);
                }
            }
            let instantiated;
            let canonical;
            let (source, target) = if !source.type_parameter_types().is_empty()
                && source.type_parameter_types() != target.type_parameter_types()
            {
                canonical = sequence.read(self.canonical_signature_completion(target, consumer))?;
                instantiated =
                    sequence.read(self.instantiate_signature_in_context_of_completion(
                        source,
                        &canonical,
                        RelationComparer {
                            session,
                            intersection,
                        },
                        consumer,
                    ))?;
                (&instantiated, &canonical)
            } else {
                (source, target)
            };
            let source_count =
                sequence.read(self.signature_parameter_count_completion(source, consumer))?;
            let source_rest =
                sequence.read(self.signature_non_array_rest_completion(source, consumer))?;
            let target_rest =
                sequence.read(self.signature_non_array_rest_completion(target, consumer))?;
            if let Some(rest) = source_rest.or(target_rest) {
                let mapper = self.model.report_reliability_mapper(false);
                self.instantiate_type(rest, mapper, consumer)?;
            }
            let kind = self
                .signature_declaration_root(target.id)?
                .map(|root| root.site().kind);
            let strict_variance = !mode.has(SignatureCheckMode::CALLBACK)
                && self.inputs.semantic_options().strict_function_types
                && !matches!(
                    kind,
                    Some(
                        SyntaxKind::MethodDeclaration
                            | SyntaxKind::MethodSignature
                            | SyntaxKind::Constructor
                    )
                );
            let mut result = Ternary::True;
            if let Some(source_this) = sequence.read(self.signature_this_type_completion(source))? {
                if source_this != self.model.special(SpecialName::Void) {
                    if let Some(target_this) =
                        sequence.read(self.signature_this_type_completion(target))?
                    {
                        let mut related = Ternary::False;
                        if !strict_variance {
                            related = sequence.read(self.related_in_session_completion(
                                source_this,
                                target_this,
                                RecursionFlags::BOTH,
                                intersection,
                                session.reporting(false),
                                consumer,
                            ))?;
                        }
                        if related == Ternary::False {
                            related = sequence.read(self.related_in_session_completion(
                                target_this,
                                source_this,
                                RecursionFlags::BOTH,
                                intersection,
                                session,
                                consumer,
                            ))?;
                        }
                        if related == Ternary::False {
                            if session.report {
                                self.relations
                                    .session_mut(session)
                                    .error
                                    .report(Message::ThisTypes, Vec::new());
                            }
                            return Ok(related);
                        }
                        result = result.and(related);
                    }
                }
            }
            let has_non_array_rest = source_rest.is_some() || target_rest.is_some();
            let count = if has_non_array_rest {
                source_count.min(target_count)
            } else {
                source_count.max(target_count)
            };
            for index in 0..count {
                let rest_position = has_non_array_rest && index + 1 == count;
                let source_type = if rest_position {
                    Some(sequence.read(
                        self.signature_rest_or_any_at_position_completion(source, index, consumer),
                    )?)
                } else {
                    sequence
                        .read(self.signature_type_at_position_completion(source, index, consumer))?
                };
                let target_type = if rest_position {
                    Some(sequence.read(
                        self.signature_rest_or_any_at_position_completion(target, index, consumer),
                    )?)
                } else {
                    sequence
                        .read(self.signature_type_at_position_completion(target, index, consumer))?
                };
                let (Some(source_type), Some(target_type)) = (source_type, target_type) else {
                    continue;
                };
                if source_type == target_type && !mode.has(SignatureCheckMode::STRICT_ARITY) {
                    continue;
                }
                let source_callback = if !mode.has(SignatureCheckMode::CALLBACK)
                    && !sequence.read(
                        self.is_instantiated_generic_parameter_completion(source, index, consumer),
                    )? {
                    let ty = self.non_nullable_type(source_type, consumer)?;
                    self.single_signature(ty, false, false, consumer)?
                } else {
                    None
                };
                let target_callback = if !mode.has(SignatureCheckMode::CALLBACK)
                    && !sequence.read(
                        self.is_instantiated_generic_parameter_completion(target, index, consumer),
                    )? {
                    let ty = self.non_nullable_type(target_type, consumer)?;
                    self.single_signature(ty, false, false, consumer)?
                } else {
                    None
                };
                let callbacks =
                    if let (Some(source), Some(target)) = (&source_callback, &target_callback) {
                        self.type_predicate_of_signature(source, consumer)?
                            .is_none()
                            && self
                                .type_predicate_of_signature(target, consumer)?
                                .is_none()
                            && self.type_facts(
                                source_type,
                                TypeFacts::IS_UNDEFINED_OR_NULL,
                                consumer,
                            )? == self.type_facts(
                                target_type,
                                TypeFacts::IS_UNDEFINED_OR_NULL,
                                consumer,
                            )?
                    } else {
                        false
                    };
                let mut related = if callbacks {
                    let callback_mode = SignatureCheckMode(
                        (mode.0 & SignatureCheckMode::STRICT_ARITY)
                            | if strict_variance {
                                SignatureCheckMode::STRICT_CALLBACK
                            } else {
                                SignatureCheckMode::BIVARIANT_CALLBACK
                            },
                    );
                    sequence.read(
                        self.compare_signatures_related_completion(
                            target_callback
                                .as_ref()
                                .expect("established target callback"),
                            source_callback
                                .as_ref()
                                .expect("established source callback"),
                            callback_mode,
                            intersection,
                            session,
                            consumer,
                        ),
                    )?
                } else {
                    let mut related = Ternary::False;
                    if !mode.has(SignatureCheckMode::CALLBACK) && !strict_variance {
                        related = sequence.read(self.related_in_session_completion(
                            source_type,
                            target_type,
                            RecursionFlags::BOTH,
                            intersection,
                            session.reporting(false),
                            consumer,
                        ))?;
                    }
                    if related == Ternary::False {
                        related = sequence.read(self.related_in_session_completion(
                            target_type,
                            source_type,
                            RecursionFlags::BOTH,
                            intersection,
                            session,
                            consumer,
                        ))?;
                    }
                    related
                };
                if related != Ternary::False
                    && mode.has(SignatureCheckMode::STRICT_ARITY)
                    && index
                        >= sequence
                            .read(self.minimum_argument_count_completion(source, consumer))?
                    && index
                        < sequence.read(self.minimum_argument_count_completion(target, consumer))?
                    && sequence.read(self.related_in_session_completion(
                        source_type,
                        target_type,
                        RecursionFlags::BOTH,
                        intersection,
                        session.reporting(false),
                        consumer,
                    ))? != Ternary::False
                {
                    related = Ternary::False;
                }
                if related == Ternary::False {
                    if session.report {
                        let source_name = sequence.read(
                            self.signature_parameter_name_completion(source, index, consumer),
                        )?;
                        let target_name = sequence.read(
                            self.signature_parameter_name_completion(target, index, consumer),
                        )?;
                        self.relations.session_mut(session).error.report(
                            Message::ParameterTypes,
                            vec![
                                Argument::Text(source_name.into_bytes()),
                                Argument::Text(target_name.into_bytes()),
                            ],
                        );
                    }
                    return Ok(related);
                }
                result = result.and(related);
            }
            let target_return =
                sequence.read(self.non_circular_signature_return_completion(target, consumer))?;
            if target_return == self.model.special(SpecialName::Void)
                || target_return == self.model.special(SpecialName::Any)
            {
                return Ok(result);
            }
            let source_return =
                sequence.read(self.non_circular_signature_return_completion(source, consumer))?;
            if let Some(target_predicate) = self.type_predicate_of_signature(target, consumer)? {
                if let Some(source_predicate) =
                    self.type_predicate_of_signature(source, consumer)?
                {
                    let related = sequence.read(self.predicate_related_completion(
                        &source_predicate,
                        &target_predicate,
                        intersection,
                        session,
                        consumer,
                    ))?;
                    result = result.and(related);
                } else if target_predicate.kind == TypePredicateKind::Is {
                    if session.report {
                        sequence.read(self.report_signature_must_be_predicate_completion(
                            source, session, consumer,
                        ))?;
                    }
                    return Ok(Ternary::False);
                }
            } else {
                let mut related = Ternary::False;
                if mode.has(SignatureCheckMode::BIVARIANT_CALLBACK) {
                    related = sequence.read(self.related_in_session_completion(
                        target_return,
                        source_return,
                        RecursionFlags::BOTH,
                        intersection,
                        session.reporting(false),
                        consumer,
                    ))?;
                }
                if related == Ternary::False {
                    related = sequence.read(self.related_in_session_completion(
                        source_return,
                        target_return,
                        RecursionFlags::BOTH,
                        intersection,
                        session,
                        consumer,
                    ))?;
                }
                result = result.and(related);
                if result == Ternary::False && session.report {
                    sequence.read(self.report_signature_return_types_completion(
                        source,
                        target,
                        (source_return, target_return),
                        session,
                        consumer,
                    ))?;
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `indexSignaturesRelatedTo` has its own collection schedule after the
    /// two signature kinds. Nonempty indexes require native applicability and
    /// inferred-member rules, never an unconditional structural approximation.
    fn relation_indexes_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        source_is_primitive: bool,
        intersection: IntersectionState,
        session: RelationInvocation,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let targets = self.index_infos_of_type(target, consumer)?;
            let string = self.model.primitive(PrimitiveName::String);
            let mut string_index = false;
            for target in &targets {
                if sequence.read(self.demand_completion(target.key_type))? == string {
                    string_index = true;
                    break;
                }
            }
            let mut result = Ternary::True;
            for target in targets {
                let target_value = sequence.read(self.demand_completion(target.value_type))?;
                if self.relations.session(session).kind != RelationKind::StrictSubtype
                    && !source_is_primitive
                    && string_index
                    && self.model.has_type_flags(target_value, TypeFlags::ANY)
                {
                    continue;
                }
                if self.is_generic_mapped_type(source, consumer)? && string_index {
                    let template = self.mapped_template(source, consumer)?;
                    let related = sequence.read(self.related_in_session_completion(
                        template,
                        target_value,
                        RecursionFlags::BOTH,
                        IntersectionState::NONE,
                        session,
                        consumer,
                    ))?;
                    if related == Ternary::False {
                        return Ok(related);
                    }
                    result = result.and(related);
                    continue;
                }
                let target_key = sequence.read(self.demand_completion(target.key_type))?;
                let related = if let Some(source) =
                    self.applicable_index_info(source, target_key, consumer)?
                {
                    sequence.read(self.index_info_related_completion(
                        &source,
                        &target,
                        RelationComparer {
                            session,
                            intersection,
                        },
                        consumer,
                    ))?
                } else if intersection.0 & 1 == 0
                    && (self.relations.session(session).kind != RelationKind::StrictSubtype
                        || self
                            .model
                            .object_flags(source)?
                            .intersects(ObjectFlags::FRESH_LITERAL))
                    && sequence
                        .read(self.object_type_with_inferable_index_completion(source, consumer))?
                {
                    sequence.read(self.members_related_to_index_completion(
                        source,
                        &target,
                        RelationComparer {
                            session,
                            intersection,
                        },
                        consumer,
                    ))?
                } else {
                    if session.report {
                        let key = sequence
                            .read(self.raw_diagnostic_type_display_completion(
                                target_key,
                                None,
                                super::native_display::DiagnosticTypeFormat::DEFAULT,
                                consumer,
                            ))?
                            .into_bytes();
                        let source = sequence
                            .read(self.raw_diagnostic_type_display_completion(
                                source,
                                None,
                                super::native_display::DiagnosticTypeFormat::DEFAULT,
                                consumer,
                            ))?
                            .into_bytes();
                        self.relations.session_mut(session).error.report(
                            Message::MissingIndex,
                            vec![Argument::Text(key), Argument::Text(source)],
                        );
                    }
                    Ternary::False
                };
                if related == Ternary::False {
                    return Ok(related);
                }
                result = result.and(related);
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `indexInfoRelatedTo` keeps value comparison and failed-key elaboration
    /// under the same live native relation session.
    fn index_info_related_completion(
        &mut self,
        source: &Index,
        target: &Index,
        comparer: RelationComparer,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let source_type = sequence.read(self.demand_completion(source.value_type))?;
            let target_type = sequence.read(self.demand_completion(target.value_type))?;
            let related = sequence.read(self.related_in_session_completion(
                source_type,
                target_type,
                RecursionFlags::BOTH,
                comparer.intersection,
                comparer.session,
                consumer,
            ))?;
            if related == Ternary::False && comparer.session.report {
                let source_key = sequence.read(self.demand_completion(source.key_type))?;
                let target_key = sequence.read(self.demand_completion(target.key_type))?;
                sequence.read(self.report_index_relation_completion(
                    source_key,
                    target_key,
                    comparer.session,
                    consumer,
                ))?;
            }
            Ok(related)
        })();
        sequence.finish(result)
    }

    /// `membersRelatedToIndexInfo` checks applicable named properties before
    /// source index infos, with native exact-optional and number-index rules.
    fn members_related_to_index_completion(
        &mut self,
        source: TypeId,
        target: &Index,
        comparer: RelationComparer,
        consumer: &Location,
    ) -> NativeResult<Ternary> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Ternary, DemandError> {
            let key = sequence.read(self.demand_completion(target.key_type))?;
            let properties = if self.model.has_type_flags(source, TypeFlags::INTERSECTION) {
                self.properties_of_type(source, consumer)?
            } else {
                self.properties_of_object_type(source, consumer)?
            };
            let mut result = Ternary::True;
            for property in properties {
                let symbol = self.property_symbol(&property, consumer)?;
                let property_key = self.literal_type_from_property(
                    symbol,
                    TypeFlags::STRING_LITERAL
                        | TypeFlags::NUMBER_LITERAL
                        | TypeFlags::UNIQUE_SYMBOL,
                    false,
                    consumer,
                )?;
                if self.is_applicable_index_type(property_key, key, consumer)? {
                    let mut ty = sequence
                        .read(self.non_missing_property_type_completion(symbol, consumer))?;
                    if !self.inputs.semantic_options().exact_optional_property_types
                        && !self.model.has_type_flags(ty, TypeFlags::UNDEFINED)
                        && key != self.model.primitive(PrimitiveName::Number)
                        && self.property_symbol_flags(symbol)?.optional
                    {
                        ty = self.type_with_facts(ty, TypeFacts::NOT_UNDEFINED, consumer)?;
                    }
                    let target_type = sequence.read(self.demand_completion(target.value_type))?;
                    let related = sequence.read(self.related_in_session_completion(
                        ty,
                        target_type,
                        RecursionFlags::BOTH,
                        comparer.intersection,
                        comparer.session,
                        consumer,
                    ))?;
                    if related == Ternary::False {
                        if comparer.session.report {
                            sequence.read(self.report_property_relation_completion(
                                Message::PropertyIndex,
                                symbol,
                                &[],
                                comparer.session,
                                consumer,
                            ))?;
                        }
                        return Ok(related);
                    }
                    result = result.and(related);
                }
            }
            for source in self.index_infos_of_type(source, consumer)? {
                let source_key = sequence.read(self.demand_completion(source.key_type))?;
                if self.is_applicable_index_type(source_key, key, consumer)? {
                    let related = sequence.read(
                        self.index_info_related_completion(&source, target, comparer, consumer),
                    )?;
                    if related == Ternary::False {
                        return Ok(related);
                    }
                    result = result.and(related);
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    fn relation_pending(&self, consumer: &Location, message: &'static str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }

    /// The recursive native False producer has no source declaration. Its
    /// typed marker is bound only on a returned copy at the outer observation.
    fn relation_overflow(&self, overflow: RelationOverflow) -> DemandError {
        super::super::unbound_recovery(match overflow {
            RelationOverflow::Depth => crate::analysis::UnboundSemanticRecovery::RelationDepth,
            RelationOverflow::Complexity => {
                crate::analysis::UnboundSemanticRecovery::RelationComplexity
            }
        })
    }
}
