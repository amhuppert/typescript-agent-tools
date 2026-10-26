//! Native type identities and construction. Presentation borrows established
//! views without knowing the factory layout or asking for nested child types.

use crate::analysis::AnalysisError;
use crate::project::binding::{
    BoundSymbol, IdentityOrder, NativeDeclaration, NativePropertyKey, NativeSymbol,
    NativeSymbolIdentity, TypeBindingFailure, TypeSymbolFlags,
};
use crate::references::Location;
use crate::types::{
    PrimitiveName, TypeAccessibility, TypeAlias, TypeIssue, TypeKey, TypePredicateKind,
    TypePredicateSubject, TypePropertyKind,
};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

mod completion;
mod conditional;
pub(super) use completion::{NativeCompletion, NativeResult, NativeSequence};
mod indexed_access;
mod keys;
pub(super) use conditional::{ConditionalRootId, ConditionalView, SubstitutionView};
pub(super) use keys::{KeyBuilder, TypeListCache, same_alias, same_types};
mod keyof;
mod literals;
use indexed_access::IndexedAccessKey;
pub(super) use indexed_access::{AccessFlags, IndexedAccessView};
pub(super) use keyof::{IndexFlags, IndexView};
use literals::LiteralKey;
pub(super) use literals::{
    LiteralError, LiteralPart, LiteralPosition, LiteralValue, literal_value_failure,
};
mod references;
use references::ReferenceKey;
mod templates;
pub(super) use templates::{StringMappingKind, TemplateLiteralView};
mod tuples;
pub(super) use tuples::{TupleElementFlags, TupleElementInfo, TupleElementLabel, TupleTarget};

/// One semantic identity in this request. It never becomes a public graph ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct TypeId(usize);

// Hash/equality observes one request-local arena index, never type structure.

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct MapperId(usize);
// Hash/Eq observes one mapper arena index, never its mapped type graph.

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct DeferredMapperId(pub(super) usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct InferenceContextId(usize);

impl InferenceContextId {
    pub(super) fn new(index: usize) -> Self {
        Self(index)
    }
    pub(super) fn index(self) -> usize {
        self.0
    }
}

pub(super) enum Mapper {
    UniqueLiteral,
    Permissive,
    Restrictive,
    Deferred {
        id: DeferredMapperId,
        sources: Vec<TypeId>,
    },
    ReportReliability {
        unmeasurable: bool,
    },
    Inference {
        context: InferenceContextId,
        fixing: bool,
    },
    ArrayToSingle {
        sources: Vec<TypeId>,
        target: TypeId,
    },
    Merged {
        first: MapperId,
        second: MapperId,
    },
    Simple {
        sources: Vec<TypeId>,
        targets: Vec<MapperTarget>,
    },
    Composite {
        first: MapperId,
        second: MapperId,
    },
}

/// `fillMissingTypeArguments` initializes unsupplied targets to errorType.
/// Keeping recovery distinct prevents later substitution/comparison from
/// establishing a healthy Any when it consumes an unsupplied argument.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum MapperTarget {
    Known(TypeId),
    ErrorType,
}

/// The actual native Type.symbol association. A missing producer is retained
/// as a failure and consumed only by the native comparison branch that needs
/// it; it must never become the successful nil-symbol case.
#[derive(Clone)]
pub(super) enum TypeSymbol {
    Absent,
    Bound {
        symbol: BoundSymbol,
        native: Result<Arc<NativeSymbol>, DemandError>,
        flags: Result<TypeSymbolFlags, DemandError>,
    },
    /// A checker-created property symbol remains distinct from every bound
    /// declaration copied into it. Its header carries that actual identity.
    Property {
        symbol: PropertySymbolId,
        native: Arc<NativeSymbol>,
    },
    Synthetic {
        native: Arc<NativeSymbol>,
        flags: TypeSymbolFlags,
    },
}

impl TypeSymbol {
    pub(super) fn native(&self) -> Result<Option<&NativeSymbol>, DemandError> {
        match self {
            Self::Absent => Ok(None),
            Self::Bound { native, .. } => native
                .as_ref()
                .map(|symbol| Some(symbol.as_ref()))
                .map_err(Clone::clone),
            Self::Property { native, .. } => Ok(Some(native.as_ref())),
            Self::Synthetic { native, .. } => Ok(Some(native.as_ref())),
        }
    }
}

/// Native `TypeFlags` for established types. Presentation kind is a separate
/// question: boolean displays as primitive while retaining its union flags.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct TypeFlags(u32);

impl TypeFlags {
    pub(super) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub(super) const NONE: Self = Self(0);
    pub(super) const ANY: Self = Self(1 << 0);
    pub(super) const UNKNOWN: Self = Self(1 << 1);
    pub(super) const UNDEFINED: Self = Self(1 << 2);
    pub(super) const NULL: Self = Self(1 << 3);
    pub(super) const VOID: Self = Self(1 << 4);
    pub(super) const STRING: Self = Self(1 << 5);
    pub(super) const NUMBER: Self = Self(1 << 6);
    pub(super) const BIGINT: Self = Self(1 << 7);
    pub(super) const BOOLEAN: Self = Self(1 << 8);
    pub(super) const SYMBOL: Self = Self(1 << 9);
    pub(super) const UNIQUE_SYMBOL: Self = Self(1 << 14);
    pub(super) const ENUM_LITERAL: Self = Self(1 << 15);
    pub(super) const ENUM: Self = Self(1 << 16);
    pub(super) const STRING_LITERAL: Self = Self(1 << 10);
    pub(super) const NUMBER_LITERAL: Self = Self(1 << 11);
    pub(super) const BIGINT_LITERAL: Self = Self(1 << 12);
    pub(super) const BOOLEAN_LITERAL: Self = Self(1 << 13);
    pub(super) const NON_PRIMITIVE: Self = Self(1 << 17);
    pub(super) const NEVER: Self = Self(1 << 18);
    pub(super) const TYPE_PARAMETER: Self = Self(1 << 19);
    pub(super) const OBJECT: Self = Self(1 << 20);
    pub(super) const INDEX: Self = Self(1 << 21);
    pub(super) const SUBSTITUTION: Self = Self(1 << 24);
    pub(super) const INDEXED_ACCESS: Self = Self(1 << 25);
    pub(super) const CONDITIONAL: Self = Self(1 << 26);
    pub(super) const UNION: Self = Self(1 << 27);
    pub(super) const INTERSECTION: Self = Self(1 << 28);
    pub(super) const TYPE_VARIABLE: Self = Self((1 << 19) | (1 << 25));
    pub(super) const TEMPLATE_LITERAL: Self = Self(1 << 22);
    pub(super) const STRING_MAPPING: Self = Self(1 << 23);
    pub(super) const VOID_LIKE: Self = Self((1 << 4) | (1 << 2));
    pub(super) const SINGLETON: Self = Self(
        Self::ANY.0
            | Self::UNKNOWN.0
            | Self::STRING.0
            | Self::NUMBER.0
            | Self::BOOLEAN.0
            | Self::BIGINT.0
            | Self::SYMBOL.0
            | Self::VOID.0
            | Self::UNDEFINED.0
            | Self::NULL.0
            | Self::NEVER.0
            | Self::NON_PRIMITIVE.0,
    );
    pub(super) const DEFINITELY_NON_NULLABLE: Self = Self(
        Self::STRING_LIKE.0
            | Self::NUMBER_LIKE.0
            | Self::BIGINT_LIKE.0
            | Self::BOOLEAN_LIKE.0
            | Self::ES_SYMBOL_LIKE.0
            | Self::ENUM_LITERAL.0
            | Self::OBJECT.0
            | Self::NON_PRIMITIVE.0,
    );
    pub(super) const DISJOINT_DOMAINS: Self = Self(
        Self::NON_PRIMITIVE.0
            | Self::STRING_LIKE.0
            | Self::NUMBER_LIKE.0
            | Self::BIGINT_LIKE.0
            | Self::BOOLEAN_LIKE.0
            | Self::ES_SYMBOL_LIKE.0
            | Self::VOID_LIKE.0
            | Self::NULL.0,
    );
    pub(super) const INCLUDES_MASK: Self = Self(
        Self::ANY.0
            | Self::UNKNOWN.0
            | Self::PRIMITIVE.0
            | Self::NEVER.0
            | Self::OBJECT.0
            | Self::UNION.0
            | Self::INTERSECTION.0
            | Self::NON_PRIMITIVE.0,
    );
    pub(super) const NULLABLE: Self = Self((1 << 2) | (1 << 3));
    pub(super) const OBJECT_FLAGS_TYPE: Self = Self(
        Self::ANY.0
            | Self::NULLABLE.0
            | Self::NEVER.0
            | Self::OBJECT.0
            | Self::UNION.0
            | Self::INTERSECTION.0,
    );
    /// Native combined flags retain semantic categories independently from
    /// display views (in particular Boolean is also a Union).
    pub(super) const LITERAL: Self = Self((1 << 10) | (1 << 11) | (1 << 12) | (1 << 13));
    pub(super) const FRESHABLE: Self =
        Self((1 << 16) | (1 << 10) | (1 << 11) | (1 << 12) | (1 << 13));
    pub(super) const STRING_LIKE: Self = Self((1 << 5) | (1 << 10) | (1 << 22) | (1 << 23));
    pub(super) const NUMBER_LIKE: Self = Self((1 << 6) | (1 << 11) | (1 << 16));
    pub(super) const ENUM_LIKE: Self = Self((1 << 15) | (1 << 16));
    pub(super) const BIGINT_LIKE: Self = Self((1 << 7) | (1 << 12));
    pub(super) const BOOLEAN_LIKE: Self = Self((1 << 8) | (1 << 13));
    pub(super) const ES_SYMBOL_LIKE: Self = Self((1 << 9) | (1 << 14));
    pub(super) const PRIMITIVE: Self = Self(
        (1 << 5)
            | (1 << 10)
            | (1 << 22)
            | (1 << 23)
            | (1 << 6)
            | (1 << 11)
            | (1 << 16)
            | (1 << 7)
            | (1 << 12)
            | (1 << 8)
            | (1 << 13)
            | (1 << 15)
            | (1 << 9)
            | (1 << 14)
            | (1 << 4)
            | (1 << 2)
            | (1 << 3),
    );
    pub(super) const STRUCTURED_OR_INSTANTIABLE: Self = Self(
        (1 << 20)
            | (1 << 27)
            | (1 << 28)
            | (1 << 19)
            | (1 << 25)
            | (1 << 26)
            | (1 << 24)
            | (1 << 21)
            | (1 << 22)
            | (1 << 23),
    );
    pub(super) const INSTANTIABLE_NON_PRIMITIVE: Self =
        Self((1 << 19) | (1 << 25) | (1 << 26) | (1 << 24));
    pub(super) const INSTANTIABLE_PRIMITIVE: Self = Self((1 << 21) | (1 << 22) | (1 << 23));
    pub(super) const INSTANTIABLE: Self =
        Self(Self::INSTANTIABLE_NON_PRIMITIVE.0 | Self::INSTANTIABLE_PRIMITIVE.0);
    pub(super) const STRING_LITERAL_CONTEXT: Self =
        Self((1 << 10) | (1 << 21) | (1 << 22) | (1 << 23));
    /// Native Unit includes Enum/UniqueESSymbol, each literal class and the
    /// nullable flags. A union of literal types has Union, not Unit flags.
    pub(super) const UNIT: Self = Self(
        (1 << 14) | (1 << 16) | (1 << 10) | (1 << 11) | (1 << 12) | (1 << 13) | (1 << 2) | (1 << 3),
    );
}

impl std::ops::BitOr for TypeFlags {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl std::ops::BitAnd for TypeFlags {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl std::ops::Not for TypeFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

/// Native `ObjectFlags`: the factory records construction facts, while the
/// inventory owner alone marks MembersResolved. Reused bits require the
/// caller's native TypeFlags qualification.
#[derive(Clone, Copy, Default)]
pub(super) struct ObjectFlags(u32);
impl ObjectFlags {
    pub(super) const NONE: Self = Self(0);
    pub(super) const CLASS: Self = Self(1 << 0);
    pub(super) const INTERFACE: Self = Self(1 << 1);
    pub(super) const REFERENCE: Self = Self(1 << 2);
    pub(super) const TUPLE: Self = Self(1 << 3);
    pub(super) const ANONYMOUS: Self = Self(1 << 4);
    pub(super) const MAPPED: Self = Self(1 << 5);
    pub(super) const INSTANTIATED: Self = Self(1 << 6);
    pub(super) const JS_LITERAL: Self = Self(1 << 12);
    pub(super) const OBJECT_LITERAL: Self = Self(1 << 7);
    pub(super) const EVOLVING_ARRAY: Self = Self(1 << 8);
    pub(super) const OBJECT_LITERAL_PATTERN_WITH_COMPUTED_PROPERTIES: Self = Self(1 << 9);
    pub(super) const JSX_ATTRIBUTES: Self = Self(1 << 11);
    pub(super) const INSTANTIATION_EXPRESSION_TYPE: Self = Self(1 << 24);
    pub(super) const SINGLE_SIGNATURE_TYPE: Self = Self(1 << 25);
    pub(super) const FRESH_LITERAL: Self = Self(1 << 13);
    pub(super) const ARRAY_LITERAL: Self = Self(1 << 14);
    pub(super) const PRIMITIVE_UNION: Self = Self(1 << 15);
    pub(super) const CONTAINS_WIDENING_TYPE: Self = Self(1 << 16);
    pub(super) const CONTAINS_OBJECT_OR_ARRAY_LITERAL: Self = Self(1 << 17);
    pub(super) const REVERSE_MAPPED: Self = Self(1 << 10);
    pub(super) const NON_INFERRABLE_TYPE: Self = Self(1 << 18);
    pub(super) const COULD_CONTAIN_TYPE_VARIABLES_COMPUTED: Self = Self(1 << 19);
    pub(super) const COULD_CONTAIN_TYPE_VARIABLES: Self = Self(1 << 20);
    pub(super) const MEMBERS_RESOLVED: Self = Self(1 << 21);
    pub(super) const CONTAINS_SPREAD: Self = Self(1 << 22);
    pub(super) const OBJECT_REST_TYPE: Self = Self(1 << 23);
    pub(super) const CONTAINS_INTERSECTIONS: Self = Self(1 << 25);
    pub(super) const IS_CONSTRAINED_TYPE_VARIABLE: Self = Self(1 << 27);
    pub(super) const IDENTICAL_BASE_TYPE_CALCULATED: Self = Self(1 << 27);
    pub(super) const UNRESOLVED_MEMBERS: Self = Self(1 << 29);
    pub(super) const FROM_TYPE_NODE: Self = Self(1 << 30);
    pub(super) const REQUIRES_WIDENING: Self =
        Self(Self::CONTAINS_WIDENING_TYPE.0 | Self::CONTAINS_OBJECT_OR_ARRAY_LITERAL.0);
    pub(super) const PROPAGATING_FLAGS: Self =
        Self(Self::REQUIRES_WIDENING.0 | Self::NON_INFERRABLE_TYPE.0);
    pub(super) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
    pub(super) fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub(super) fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}
impl std::ops::BitOr for ObjectFlags {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl std::ops::BitAnd for ObjectFlags {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

/// The `getIntersectionKey` marker partitions aliases only when constraint
/// reduction is disabled; NoSupertypeReduction affects normalization instead.
#[derive(Clone, Copy)]
pub(super) struct IntersectionFlags(u8);
impl IntersectionFlags {
    pub(super) const NONE: Self = Self(0);
    pub(super) const NO_SUPERTYPE_REDUCTION: Self = Self(1);
    pub(super) const NO_CONSTRAINT_REDUCTION: Self = Self(2);
    pub(super) fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct AliasKey {
    symbol: AliasSymbol,
    arguments: Arc<[TypeId]>,
}
impl AliasKey {
    pub(super) fn of(alias: Option<&Alias>) -> Option<Self> {
        alias.map(|alias| Self {
            symbol: alias.symbol,
            arguments: alias.arguments.clone().unwrap_or_else(|| Arc::from([])),
        })
    }
}
#[derive(Clone)]
pub(super) struct IntersectionKey {
    types: Arc<[TypeId]>,
    no_constraint_reduction: bool,
    alias: Option<AliasKey>,
    fingerprint: u64,
}
#[derive(Clone)]
enum UnionIdentity {
    Types(Arc<[TypeId]>),
    UnionOrigin(Arc<[TypeId]>),
    IntersectionOrigin(Arc<[TypeId]>),
    IndexOrigin {
        origin: TypeId,
        types: Arc<[TypeId]>,
    },
}
#[derive(Clone)]
struct UnionKey {
    identity: UnionIdentity,
    alias: Option<AliasKey>,
    fingerprint: u64,
}

/// Canonical native property-symbol identity; serialization has a distinct ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct PropertySymbolId(usize);
// One canonical symbol-arena index; Eq/Hash never traverses symbol metadata.

impl PropertySymbolId {
    pub(super) fn new(index: usize) -> Self {
        Self(index)
    }
    pub(super) fn index(self) -> usize {
        self.0
    }
}

/// One exact semantic child question. A member's read and write questions can
/// differ, and not every deferred child is a member operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct DemandId(usize);
// Eq/Hash observes only the fixed request-local arena index.

impl DemandId {
    pub(super) fn new(index: usize) -> Self {
        Self(index)
    }
    pub(super) fn index(self) -> usize {
        self.0
    }
}

/// A child operation is separate from an established semantic type identity.
/// Deferred members acquire a type only when their owning evaluator demands it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum TypeDemand {
    Known(TypeId),
    Deferred(DemandId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SpecialName {
    Any,
    Unknown,
    Never,
    Void,
    Undefined,
    Null,
}

impl SpecialName {
    pub(super) fn text(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Unknown => "unknown",
            Self::Never => "never",
            Self::Void => "void",
            Self::Undefined => "undefined",
            Self::Null => "null",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum UnionReduction {
    None,
    Literal,
    Subtype,
}

pub(super) enum UnionFailure {
    UnsupportedComparison,
    Demand(DemandError),
    Binding(TypeBindingFailure),
    Literal(LiteralError),
    SubtypeReduction {
        types: Vec<TypeId>,
        has_object_types: bool,
    },
    Constraint(TypeId),
    StringLiteralReduction(Vec<TypeId>),
}

impl UnionFailure {
    /// Preserve the semantic cause through every union-producing consumer.
    /// Only native operations still awaiting implementation create a located
    /// unsupported failure; a dependency failure is never reclassified.
    pub(super) fn at(
        self,
        inputs: crate::project::semantic::inputs::CheckerInputs<'_>,
        consumer: &Location,
    ) -> DemandError {
        let message = match self {
            Self::Demand(error) => return error,
            Self::Binding(error) => return super::binding_failure_at(inputs, consumer, error),
            Self::Literal(error) => return literal_value_failure(inputs, consumer, error),
            Self::Constraint(_)
            | Self::SubtypeReduction { .. }
            | Self::StringLiteralReduction(_) => {
                return super::internal("union semantic dependency escaped its evaluator owner")
                    .into();
            }
            Self::UnsupportedComparison => "native canonical union type comparison is pending",
        };
        super::unsupported_demand_at_location(
            inputs,
            consumer,
            crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }
}

impl From<DemandError> for UnionFailure {
    fn from(error: DemandError) -> Self {
        Self::Demand(error)
    }
}

impl From<TypeBindingFailure> for UnionFailure {
    fn from(error: TypeBindingFailure) -> Self {
        Self::Binding(error)
    }
}

impl From<LiteralError> for UnionFailure {
    fn from(error: LiteralError) -> Self {
        Self::Literal(error)
    }
}

#[derive(Clone)]
pub(super) struct Property {
    metadata: Arc<PropertyMetadata>,
    pub(super) symbol: Option<PropertySymbolId>,
    pub(super) read_type: TypeDemand,
    /// Some is an independent writable question whose established identity
    /// may equal the read. Readonly suppresses this operation.
    pub(super) write_type: Option<TypeDemand>,
    pub(super) display: PropertyDisplayDemand,
    pub(super) kind: TypePropertyKind,
    pub(super) optional: bool,
    pub(super) readonly: bool,
    pub(super) accessibility: TypeAccessibility,
}

pub(super) struct PropertyData {
    pub(super) symbol: Option<PropertySymbolId>,
    pub(super) key: TypeKey,
    pub(super) read_type: TypeDemand,
    /// Some is an independent writable question whose established identity
    /// may equal the read. Readonly suppresses this operation.
    pub(super) write_type: Option<TypeDemand>,
    pub(super) display: PropertyDisplayDemand,
    pub(super) kind: TypePropertyKind,
    pub(super) optional: bool,
    pub(super) readonly: bool,
    pub(super) accessibility: TypeAccessibility,
    pub(super) origins: Vec<Location>,
}

/// Immutable public-key/origin metadata, independent of mutable type/flags.
/// No Clone or mutable dereference: ordinary property snapshots share it.
pub(super) struct PropertyMetadata {
    pub(super) key: TypeKey,
    pub(super) origins: Vec<Location>,
}
impl std::ops::Deref for Property {
    type Target = PropertyMetadata;
    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}
impl Property {
    /// Retain the existing `project_property_symbol` inputs without acquiring any
    /// new symbol, declaration, value or identity question.
    pub(super) fn new(data: PropertyData) -> Self {
        let metadata = {
            std::sync::Arc::new(PropertyMetadata {
                key: data.key,
                origins: data.origins,
            })
        };
        Self {
            metadata,
            symbol: data.symbol,
            read_type: data.read_type,
            write_type: data.write_type,
            display: data.display,
            kind: data.kind,
            optional: data.optional,
            readonly: data.readonly,
            accessibility: data.accessibility,
        }
    }
    pub(super) fn key_snapshot(&self) -> TypeKey {
        let name = match &self.key {
            TypeKey::String { name }
            | TypeKey::Private { name, .. }
            | TypeKey::Symbol { name, .. } => name,
        };
        let name = {
            let mut output = String::new();
            output.push_str(name);
            output
        };
        let origins = |values: &[Location]| Model::copy_origins(values);
        match &self.key {
            TypeKey::String { .. } => TypeKey::String { name },
            TypeKey::Private {
                origins: values, ..
            } => TypeKey::Private {
                name,
                origins: origins(values),
            },
            TypeKey::Symbol {
                origins: values, ..
            } => TypeKey::Symbol {
                name,
                origins: origins(values),
            },
        }
    }
    pub(super) fn origins_snapshot(&self) -> Vec<Location> {
        Model::copy_origins(&self.origins)
    }
}

/// One native `addPropertyToElementList` decision. Its owning evaluator retains
/// the canonical symbol/mapper; display spelling and origins are not identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct PropertySerializationId(usize);

impl PropertySerializationId {
    pub(super) fn new(index: usize) -> Self {
        Self(index)
    }
    pub(super) fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy)]
pub(super) enum PropertyDisplayDemand {
    Value(TypeDemand),
    Native(PropertySerializationId),
}

/// Established native serialization clauses. The presenter prints signatures;
/// it never reconstructs accessor/method decisions from graph property kinds.
pub(super) enum PropertyDisplay {
    Plain { value_type: TypeDemand },
    Methods { signatures: Arc<[Signature]> },
    Accessors(Box<AccessorDisplay>),
}

pub(super) struct AccessorDisplay {
    pub(super) getter: Option<Signature>,
    pub(super) setter: Option<Signature>,
}

/// Native `VarianceFlags` keeps measured polarity and reliability separate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct VarianceFlags(u32);

impl VarianceFlags {
    pub(super) const INVARIANT: Self = Self(0);
    pub(super) const COVARIANT: Self = Self(1);
    pub(super) const CONTRAVARIANT: Self = Self(2);
    pub(super) const BIVARIANT: Self = Self(3);
    pub(super) const INDEPENDENT: Self = Self(4);
    pub(super) const VARIANCE_MASK: Self = Self(7);
    pub(super) const UNMEASURABLE: Self = Self(8);
    pub(super) const UNRELIABLE: Self = Self(16);
    pub(super) const ALLOWS_STRUCTURAL_FALLBACK: Self = Self(24);

    pub(super) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for VarianceFlags {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl std::ops::BitAnd for VarianceFlags {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

/// Actual In/Out declaration modifiers, distinct from measured variance.
#[derive(Clone, Copy)]
pub(super) enum DeclaredVariance {
    In,
    Out,
    InOut,
}

impl DeclaredVariance {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
            Self::InOut => "in out",
        }
    }
    fn flags(self) -> VarianceFlags {
        match self {
            Self::In => VarianceFlags::CONTRAVARIANT,
            Self::Out => VarianceFlags::COVARIANT,
            Self::InOut => VarianceFlags::INVARIANT,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct MarkerTypes {
    pub(super) super_type: TypeId,
    pub(super) sub_type: TypeId,
    pub(super) other_type: TypeId,
    pub(super) super_for_check: TypeId,
    pub(super) sub_for_check: TypeId,
}

/// Immutable exact decoded/written names share storage across native parameter
/// snapshots. Constraint/default/factory state stays in the by-value header.
#[derive(Clone)]
pub(super) struct Parameter {
    names: Arc<ParameterNames>,
    pub(super) constraint: Option<TypeDemand>,
    pub(super) default: Option<TypeDemand>,
    pub(super) variance: Option<DeclaredVariance>,
    /// Native `getTypeParameterModifiers` retains Const independently from
    /// declared variance; later inference/serialization asks that exact fact.
    pub(super) constant: bool,
    /// `TypeParameter.isThisType` is a native construction fact, independent
    /// of the exact printed name and canonical declaration association.
    pub(super) is_this: bool,
}

impl std::ops::Deref for Parameter {
    type Target = ParameterNames;
    fn deref(&self) -> &Self::Target {
        &self.names
    }
}

impl Parameter {
    pub(super) fn copy_name(&self) -> String {
        self.name.clone()
    }

    pub(super) fn copy_display_name(&self) -> String {
        self.display_name.clone()
    }
}

pub(super) struct ParameterNames {
    pub(super) name: String,
    /// `getNameOfSymbolAsWritten` keeps the first own declaration spelling.
    /// It remains separate from the decoded exact semantic/graph name.
    pub(super) display_name: String,
}

pub(super) struct ParameterData {
    pub(super) name: String,
    pub(super) display_name: String,
    pub(super) constraint: Option<TypeDemand>,
    pub(super) default: Option<TypeDemand>,
    pub(super) variance: Option<DeclaredVariance>,
    /// Native `getTypeParameterModifiers` retains Const independently from
    /// declared variance; later inference/serialization asks that exact fact.
    pub(super) constant: bool,
    /// `TypeParameter.isThisType` is a native construction fact, independent
    /// of the exact printed name and canonical declaration association.
    pub(super) is_this: bool,
}

pub(super) enum ParameterInput {
    Owned(ParameterData),
    Existing(Parameter),
}
impl From<ParameterData> for ParameterInput {
    fn from(value: ParameterData) -> Self {
        Self::Owned(value)
    }
}
impl From<Parameter> for ParameterInput {
    fn from(value: Parameter) -> Self {
        Self::Existing(value)
    }
}

#[derive(Clone)]
pub(super) struct ParameterValue {
    /// Actual native parameter symbol. Descriptors and demands never supply
    /// its identity; clones and instantiations retain their native factory rules.
    pub(super) symbol: PropertySymbolId,
    pub(super) name: String,
    pub(super) declaration: Option<super::source::DeclarationIdentity>,
    pub(super) value_type: TypeDemand,
    /// `getTypeOfParameter` adds call-site optionality independently from the
    /// symbol read and declaration serialization questions.
    pub(super) call_type: TypeDemand,
    /// `serializeTypeForDeclaration` asks a distinct display question from
    /// `getTypeOfSymbol`; initialized required parameters can print an added
    /// undefined constituent while their semantic read type remains unchanged.
    pub(super) display_type: TypeDemand,
    pub(super) optional: bool,
    pub(super) rest: bool,
    pub(super) transforms: ParameterTypeTransforms,
}

/// Declaration-owned transforms remain distinct from Symbol.Optional and
/// CheckFlags.OptionalParameter. Replacing a symbol's raw type preserves them;
/// a newly synthesized parameter has no declaration and uses neither.
#[derive(Clone, Copy, Default)]
pub(super) struct ParameterTypeTransforms {
    pub(super) call_optional: bool,
    pub(super) display_optional: bool,
}

#[derive(Clone)]
pub(super) struct Predicate {
    pub(super) kind: TypePredicateKind,
    pub(super) subject: TypePredicateSubject,
    /// Actual native parameterName; standalone predicate display cannot
    /// recover this from a signature's potentially renamed parameters.
    pub(super) parameter_name: Option<Arc<str>>,
    pub(super) value_type: Option<TypeDemand>,
}

/// One native `newSignature` identity. Equal descriptors/origins are not the
/// same signature, and instantiated signatures retain their own identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct SignatureId(usize);
// Eq/Hash observes only the fixed request-local arena index.

/// Complete native `SignatureFlags` bits. Fact acquisition stays with the
/// declaring/instantiating operation; this owner preserves the exact mask.
#[derive(Clone, Copy)]
pub(super) struct SignatureFlags(u32);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum CallChain {
    Inner,
    Outer,
}
// Two fixed native call-chain tags; no signature structure participates.

impl SignatureFlags {
    /// Native createUnionOfSignaturesForOverloadFailure does not propagate
    /// construct, abstract, untyped, call-chain, or non-inferrable bits.
    pub(super) fn overload_failure(rest: bool, literal_types: bool) -> Self {
        Self((1 << 8) | u32::from(rest) | (u32::from(literal_types) << 1))
    }
    pub(super) fn is_overload_failure_candidate(self) -> bool {
        self.0 & (1 << 8) != 0
    }
    pub(super) fn call_chain(self) -> Option<CallChain> {
        if self.0 & (1 << 4) != 0 {
            Some(CallChain::Inner)
        } else if self.0 & (1 << 5) != 0 {
            Some(CallChain::Outer)
        } else {
            None
        }
    }
    pub(super) fn with_call_chain(self, chain: CallChain) -> Self {
        Self(
            (self.0 & !((1 << 4) | (1 << 5)))
                | match chain {
                    CallChain::Inner => 1 << 4,
                    CallChain::Outer => 1 << 5,
                },
        )
    }
    pub(super) fn non_inferrable() -> Self {
        Self(1 << 7)
    }
    /// `combineUnionOrIntersectionMemberSignatures` propagates both inputs,
    /// then derives HasRestParameter from the actual combined last parameter.
    pub(super) fn combined(self, other: Self, rest: bool) -> Self {
        let flags = Self(self.0 | other.0).propagating();
        Self((flags.0 & !(1 << 0)) | u32::from(rest))
    }
    pub(super) fn is_non_inferrable(self) -> bool {
        self.0 & (1 << 7) != 0
    }
    pub(super) fn with_abstract(self, value: bool) -> Self {
        Self(if value {
            self.0 | (1 << 3)
        } else {
            self.0 & !(1 << 3)
        })
    }
    pub(super) fn has_rest(self) -> bool {
        self.0 & (1 << 0) != 0
    }
    /// Native `signatureHasLiteralTypes` asks the specialized-signature bit,
    /// independently of parameter display or inferred argument types.
    pub(super) fn has_literal_types(self) -> bool {
        self.0 & (1 << 1) != 0
    }
    /// Native ConstructorType serialization preserves SignatureFlagsAbstract.
    pub(super) fn has_abstract(self) -> bool {
        self.0 & (1 << 3) != 0
    }
    pub(super) fn is_construct(self) -> bool {
        self.0 & (1 << 2) != 0
    }
    /// `getSignatureFromDeclaration`'s admitted TS declaration facts. JS and
    /// optional-chain signature flags have distinct later factory operations.
    pub(super) fn declared(
        rest: bool,
        specialized: bool,
        construct: bool,
        abstract_: bool,
    ) -> Self {
        Self(
            u32::from(rest)
                | u32::from(specialized) << 1
                | u32::from(construct) << 2
                | u32::from(abstract_) << 3,
        )
    }

    /// `SignatureFlagsPropagatingFlags`: rest, specialized, construct,
    /// abstract, untyped-JS and overload-failure candidate bits survive.
    /// Inner/outer call-chain and non-inferrable bits deliberately do not.
    pub(super) fn propagating(self) -> Self {
        Self(self.0 & (1 << 0 | 1 << 1 | 1 << 2 | 1 << 3 | 1 << 6 | 1 << 8))
    }
}

pub(super) struct SignatureData {
    pub(super) type_parameter_types: Arc<[TypeId]>,
    pub(super) type_parameters: Vec<Parameter>,
    pub(super) this_parameter: Option<ParameterValue>,
    pub(super) parameters: SignatureSlice<ParameterValue>,
    pub(super) return_type: TypeDemand,
    pub(super) predicate: Option<Predicate>,
    pub(super) origins: SignatureSlice<Location>,
    pub(super) minimum_arguments: usize,
    pub(super) flags: SignatureFlags,
}

/// Factory input distinguishes an already-shared native field snapshot from
/// owned storage. Conversion to this carrier never allocates; `newSignature`
/// shares owned arrays through `into_shared`. Acquisition owns the original
/// Vec and its children.
pub(super) enum SignatureSlice<T> {
    Owned(Vec<T>),
    Shared(Arc<[T]>),
}
impl<T> From<Vec<T>> for SignatureSlice<T> {
    fn from(values: Vec<T>) -> Self {
        Self::Owned(values)
    }
}
impl<T> From<Arc<[T]>> for SignatureSlice<T> {
    fn from(values: Arc<[T]>) -> Self {
        Self::Shared(values)
    }
}
impl<T> SignatureSlice<T> {
    fn into_shared(self) -> Arc<[T]> {
        match self {
            Self::Owned(values) => std::sync::Arc::from(values),
            Self::Shared(values) => values,
        }
    }
}

#[derive(Clone)]
pub(super) struct Signature {
    pub(super) id: SignatureId,
    contextual: Arc<SignatureContextualFields>,
    pub(super) parameters: Arc<[ParameterValue]>,
    pub(super) return_type: TypeDemand,
    pub(super) predicate: Option<Predicate>,
    pub(super) origins: Arc<[Location]>,
    pub(super) minimum_arguments: usize,
    pub(super) flags: SignatureFlags,
}

struct SignatureTypeParameters {
    types: Arc<[TypeId]>,
    display: Vec<Parameter>,
}

/// `assignContextualParameterTypes` changes these fields once after the
/// declaration's ContextChecked flag is installed. Rust handle clones observe
/// the same slots; native cloneSignature snapshots their current values into
/// a fresh signature. One-time cells keep references stable across reentry.
struct SignatureContextualFields {
    parameters: SignatureTypeParameters,
    contextual_parameters: OnceLock<SignatureTypeParameters>,
    this: Option<ParameterValue>,
    contextual_this: OnceLock<Option<ParameterValue>>,
}

impl Signature {
    pub(super) fn type_parameter_types(&self) -> &Arc<[TypeId]> {
        &self
            .contextual
            .contextual_parameters
            .get()
            .unwrap_or(&self.contextual.parameters)
            .types
    }

    pub(super) fn type_parameters(&self) -> &Vec<Parameter> {
        &self
            .contextual
            .contextual_parameters
            .get()
            .unwrap_or(&self.contextual.parameters)
            .display
    }

    pub(super) fn this_parameter(&self) -> &Option<ParameterValue> {
        self.contextual
            .contextual_this
            .get()
            .unwrap_or(&self.contextual.this)
    }

    pub(super) fn install_contextual_type_parameters(
        &self,
        types: Arc<[TypeId]>,
        display: Vec<Parameter>,
    ) -> Result<(), DemandError> {
        if !self.type_parameter_types().is_empty() || types.len() != display.len() {
            return Err(super::internal(
                "contextual signature parameters violate native installation invariant",
            )
            .into());
        }
        self.contextual
            .contextual_parameters
            .set(SignatureTypeParameters { types, display })
            .map_err(|_| super::internal("contextual signature parameters installed twice").into())
    }

    pub(super) fn install_contextual_this(
        &self,
        parameter: ParameterValue,
    ) -> Result<(), DemandError> {
        if self.this_parameter().is_some() {
            return Err(super::internal("contextual this parameter already exists").into());
        }
        self.contextual
            .contextual_this
            .set(Some(parameter))
            .map_err(|_| super::internal("contextual this parameter installed twice").into())
    }

    /// Private-factory replacement is distinct from contextual installation.
    /// A shared handle proves the factory has already published the signature.
    pub(super) fn replace_factory_type_parameters(
        &mut self,
        types: Arc<[TypeId]>,
        display: Vec<Parameter>,
    ) -> Result<(), DemandError> {
        if types.len() != display.len() {
            return Err(super::internal("factory signature parameter cardinality differs").into());
        }
        let fields = Arc::get_mut(&mut self.contextual)
            .ok_or_else(|| super::internal("factory replaced published signature parameters"))?;
        fields.parameters = SignatureTypeParameters { types, display };
        Ok(())
    }

    pub(super) fn replace_factory_this(
        &mut self,
        parameter: Option<ParameterValue>,
    ) -> Result<(), DemandError> {
        let fields = Arc::get_mut(&mut self.contextual)
            .ok_or_else(|| super::internal("factory replaced published this parameter"))?;
        fields.this = parameter;
        Ok(())
    }
}

/// One native `IndexInfo` allocation. Cloning a member inventory retains its
/// identity; changing its value or modifiers requires a new factory result.
#[derive(Clone)]
pub(super) struct Index(Arc<IndexInfo>);

pub(super) struct IndexInfo {
    /// Established `getNameFromIndexInfo` display spelling. The renderer does
    /// not reconstruct it from the key type or a declaration origin.
    pub(super) parameter_name: String,
    pub(super) key_type: TypeDemand,
    pub(super) value_type: TypeDemand,
    pub(super) readonly: bool,
    pub(super) declaration: Option<super::source::DeclarationIdentity>,
    pub(super) components: Option<Arc<[super::source::DeclarationIdentity]>>,
}

impl std::ops::Deref for Index {
    type Target = IndexInfo;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Index {
    pub(super) fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for Index {
    fn eq(&self, other: &Self) -> bool {
        self.same_identity(other)
    }
}
impl Eq for Index {}
impl std::hash::Hash for Index {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&Arc::as_ptr(&self.0), state);
    }
}

impl Model {
    /// `getIndexInfoWithReadonly` preserves a matching record's identity and
    /// copies already established type fields without acquiring new values.
    pub(super) fn index_info_with_readonly(&self, info: &Index, readonly: bool) -> Index {
        if info.readonly == readonly {
            return info.clone();
        }
        Index(Arc::new(IndexInfo {
            parameter_name: info.parameter_name.clone(),
            key_type: info.key_type,
            value_type: info.value_type,
            readonly,
            declaration: info.declaration,
            components: info.components.clone(),
        }))
    }

    /// `newIndexInfo` allocates a distinct record on every native factory
    /// call. `getNameFromIndexInfo` uses the captured declaration's parameter
    /// spelling, or "x" for a synthetic index with no declaration.
    pub(super) fn index_info(
        &mut self,
        key: TypeId,
        value: TypeId,
        readonly: bool,
        declaration: Option<(super::source::DeclarationIdentity, String)>,
        components: Option<Arc<[super::source::DeclarationIdentity]>>,
    ) -> Index {
        let (declaration, parameter_name) = match declaration {
            Some((declaration, name)) => (Some(declaration), name),
            None => (None, "x".into()),
        };
        Index(Arc::new(IndexInfo {
            parameter_name,
            key_type: TypeDemand::Known(key),
            value_type: TypeDemand::Known(value),
            readonly,
            declaration,
            components,
        }))
    }
}

/// Established enumeration in compiler order. Empty slices mean known empty;
/// future blocked enumeration must be returned as a located demand failure.
#[derive(Clone)]
pub(super) struct MembersView {
    pub(super) properties: Arc<[Property]>,
    pub(super) call_signatures: Arc<[Signature]>,
    pub(super) construct_signatures: Arc<[Signature]>,
    pub(super) index_signatures: Arc<[Index]>,
}

/// `resolveStructuredTypeMembers` is one fixed inventory question for an
/// established type. Its token carries that type identity, not a second ID.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct MembersOfType(TypeId);

impl MembersOfType {
    pub(super) fn type_id(self) -> TypeId {
        self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetKind {
    Class,
    Interface,
    Tuple,
}

/// Native object kind is independent from alias/nominal display metadata.
/// Anonymous functions/type literals and declared interfaces can have the
/// same empty member shape while asking different native type-facts questions.
#[derive(Clone, Copy)]
pub(super) enum ObjectKind {
    Anonymous,
}

pub(super) struct TargetHeader {
    pub(super) symbol: TypeSymbol,
    pub(super) kind: TargetKind,
    pub(super) name: String,
}

#[derive(Clone)]
pub(super) struct TargetView {
    pub(super) target: TypeId,
    pub(super) name: Arc<str>,
    pub(super) outer_parameters: Arc<[TypeId]>,
    pub(super) local_parameters: Arc<[TypeId]>,
    pub(super) all_parameters: Arc<[TypeId]>,
    pub(super) this_parameter: Option<TypeId>,
    pub(super) is_reference: bool,
    pub(super) tuple: Option<Rc<TupleTarget>>,
}

#[derive(Clone)]
pub(super) struct ReferenceView {
    pub(super) ty: TypeId,
    pub(super) target: TypeId,
    pub(super) name: Arc<str>,
    pub(super) tuple: Option<Rc<TupleTarget>>,
    pub(super) is_reference: bool,
    pub(super) members: MembersOfType,
}

/// Display's nonforcing reference-argument header. A missing vector retains
/// the actual collection question; it never denotes a recovered empty list.
pub(super) struct ReferenceArgumentsForGraph {
    pub(super) length: usize,
    pub(super) established: Option<Arc<[TypeId]>>,
}

/// A native lazy object has actual identity and flags before its member table
/// resolves. Anonymous, mapped and reverse-mapped factories share this header;
/// the evaluator retains their target/mapper and family-specific lazy links.
#[derive(Clone)]
pub(super) struct LazyObjectView {
    pub(super) members: MembersOfType,
}

/// Established function-family metadata for graph labels and recursive
/// placeholders. The native display owner decides whether to emit typeof;
/// factories do not cache a symbol's rendered name.
#[derive(Clone)]
pub(super) enum FunctionSymbolSerialization {
    Local,
    NonLocal,
}

#[derive(Clone, Default)]
pub(super) struct MetadataView {
    pub(super) summary_origin: Option<TypeId>,
    pub(super) alias: Option<Arc<Alias>>,
    pub(super) nominal_name: Option<Arc<str>>,
    pub(super) type_arguments: Option<Arc<[TypeDemand]>>,
    pub(super) function_symbol: Option<FunctionSymbolSerialization>,
}

/// Type alias provenance and an interface's own display symbol are different
/// compiler facts (`getAliasForTypeNode` versus `typeToTypeNodeHelper`).
#[derive(Default, Clone)]
pub(super) struct Metadata {
    pub(super) alias: Option<Alias>,
    pub(super) nominal_name: Option<String>,
}

/// Native alias cache identity, public provenance and written printer spelling
/// are separate facts. Generic arguments use established semantic identities.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum AliasSymbol {
    Bound(BoundSymbol),
    Created(PropertySymbolId),
}
impl From<BoundSymbol> for AliasSymbol {
    fn from(symbol: BoundSymbol) -> Self {
        Self::Bound(symbol)
    }
}

#[derive(Clone)]
pub(super) struct Alias {
    pub(super) native_symbol: TypeSymbol,
    pub(super) symbol: AliasSymbol,
    pub(super) provenance: TypeAlias,
    pub(super) display_name: String,
    pub(super) arguments: Option<Arc<[TypeId]>>,
}

/// Identity of the actual property-cache question that acquired a failure.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PropertyValueQuestion {
    Read(PropertySymbolId),
    Write(PropertySymbolId),
}

/// Keep failure-only payload out of every successful native completion/cache
/// slot. The payload and semantic error have separate boxes.
#[derive(Clone)]
pub(super) struct DemandError {
    payload: Box<DemandErrorPayload>,
    /// The native property cache operation that failed. Dependent non-missing
    /// writes and serialization preserve that question instead of turning one
    /// failed read into several issues identified by their presentation edge.
    pub(super) failed_property: Option<PropertyValueQuestion>,
}

#[derive(Clone)]
struct DemandErrorPayload {
    error: AnalysisError,
    /// Explicit eligibility for a Q4 unsupported leaf, separate from the
    /// complete provenance carried by SemanticError. Invalid failures can
    /// have an exact source range and still must remain whole Q4 errors.
    issue: Option<TypeIssue>,
}

impl DemandError {
    pub(super) fn new(error: AnalysisError, issue: Option<TypeIssue>) -> Self {
        Self {
            payload: Box::new(DemandErrorPayload { error, issue }),
            failed_property: None,
        }
    }

    /// Copy the evidence of an installed native completion. Its private
    /// representation excludes whole failures; outer interrupted results do
    /// not use this operation. Retain both public and private provenance.
    pub(super) fn error(&self) -> &AnalysisError {
        &self.payload.error
    }

    pub(super) fn error_mut(&mut self) -> &mut AnalysisError {
        &mut self.payload.error
    }

    pub(super) fn take_issue(&mut self) -> Option<TypeIssue> {
        self.payload.issue.take()
    }

    /// Preserve ordered semantic dependencies after a native recovery value.
    /// Whole-operation failures take precedence and terminate continuation.
    /// This changes provenance only; it never turns a failure into a type.
    pub(super) fn append_failure(mut self, failure: Self) -> Self {
        if self.is_whole_operation_failure() {
            return self;
        }
        if failure.is_whole_operation_failure() {
            return failure;
        }
        let AnalysisError::Semantic(semantic) = failure.error() else {
            unreachable!("acquired semantic cause");
        };
        let AnalysisError::Semantic(error) = self.error_mut() else {
            unreachable!("primary semantic failure");
        };
        if let Err(error) = error.append_dependency(semantic) {
            return error.into();
        }
        self
    }

    /// Semantic failures may collect later, ordered diagnostic dependencies.
    /// Resource, I/O and internal failures terminate the whole operation.
    pub(super) fn is_whole_operation_failure(&self) -> bool {
        !matches!(self.error(), AnalysisError::Semantic(error)
            if matches!(error.kind, crate::analysis::SemanticErrorKind::UnsupportedSemantic | crate::analysis::SemanticErrorKind::InvalidSemantic))
    }

    pub(super) fn at_property(mut self, question: PropertyValueQuestion) -> Self {
        self.failed_property.get_or_insert(question);
        self
    }
}

impl From<DemandError> for AnalysisError {
    fn from(failure: DemandError) -> Self {
        failure.payload.error
    }
}

impl From<AnalysisError> for DemandError {
    fn from(error: AnalysisError) -> Self {
        // Existing exact semantic provenance survives this conversion, but
        // only the unsupported operation's owner can attach a Q4 leaf.
        // Parse/I/O/invalid/internal failures remain whole-operation errors.
        Self::new(error, None)
    }
}

/// Describing a root or collection does not recursively demand its child IDs.
/// Future lazy child acquisition reports unsupported, invalid and resource
/// failures distinctly through the same operation when that child is demanded.
#[derive(Clone)]
pub(super) enum TypeView {
    UniqueSymbol(NativePropertyKey),
    Enum,
    Special(SpecialName),
    Primitive(PrimitiveName),
    Literal {
        base: PrimitiveName,
        value: LiteralValue,
    },
    Object(MembersView),
    LazyObject(LazyObjectView),
    Union(Arc<[TypeId]>),
    Intersection(Arc<[TypeId]>),
    TypeParameter(Parameter),
    Reference(ReferenceView),
    Conditional(ConditionalView),
    Substitution(SubstitutionView),
    IndexedAccess(IndexedAccessView),
    TemplateLiteral(TemplateLiteralView),
    StringMapping(templates::StringMappingView),
    Index(IndexView),
}

enum TargetState {
    Allocating(TargetHeader),
    // Retain the native factory kind without exposing an unused observation.
    Class(TargetView),
    Interface(TargetView),
    Tuple(TargetView),
    Failed(DemandError),
}

/// `createDeferredTypeReference` keeps its actual node and mapper even after
/// getTypeArguments installs a value. CompareTypes observes this header only.
#[derive(Clone, Copy)]
pub(super) struct DeferredReference {
    pub(super) node: NativeDeclaration,
    pub(super) declaration: crate::project::binding::NativeNodeIdentity,
    pub(super) mapper: Option<MapperId>,
}

enum TypeArgumentsState {
    Pending,
    Installed(NativeCompletion<Arc<[TypeId]>>),
    Failed(DemandError),
}

enum ParameterCloneState {
    Allocating,
    Complete(Parameter),
    Failed(DemandError),
}

enum Type {
    UniqueSymbol(NativePropertyKey),
    IntrinsicMarker,
    StringMapping(templates::StringMappingView),
    TemplateLiteral(TemplateLiteralView),
    Special(SpecialName),
    AutomaticAny,
    NonInferrableAny,
    Wildcard,
    Missing,
    OptionalMarker,
    WideningNull,
    WideningUndefined,
    ImplicitNever,
    UnreachableNever,
    SilentNever,
    UniqueLiteral,
    Parameter(Parameter),
    ParameterClone {
        target: TypeId,
        state: Box<ParameterCloneState>,
    },
    Target(Box<TargetState>),
    Reference {
        target: TypeId,
        arguments: TypeArgumentsState,
        deferred: Option<DeferredReference>,
        alias: Option<Arc<Alias>>,
        array_literal: bool,
    },
    LazyObject {
        metadata: MetadataView,
    },
    Conditional {
        data: ConditionalView,
        alias: Option<Arc<Alias>>,
    },
    Substitution(SubstitutionView),
    Index(IndexView),
    IndexedAccess {
        data: IndexedAccessView,
        alias: Option<Arc<Alias>>,
    },
    Primitive(PrimitiveName),
    Enum {
        regular: Option<TypeId>,
    },
    Literal {
        base: PrimitiveName,
        value: LiteralValue,
        enum_: bool,
        regular: Option<TypeId>,
    },
    Object {
        members: MembersView,
        metadata: MetadataView,
        literal: bool,
        kind: ObjectKind,
    },
    Union {
        alternatives: Arc<[TypeId]>,
        boolean: bool,
        enum_: bool,
        alias: Option<Arc<Alias>>,
        origin: Option<TypeId>,
    },
    Intersection {
        constituents: Arc<[TypeId]>,
        alias: Option<Arc<Alias>>,
    },
}

/// The two predicate passes of native `filterType`. Both pure model filters
/// and semantic completion filters use this controller; callers never infer
/// an origin predicate from the normalized identities that survived.
pub(super) struct TypeFilter {
    original: TypeId,
    types: Arc<[TypeId]>,
    origin: Option<Arc<[TypeId]>>,
    cursor: usize,
    retained: Vec<TypeId>,
    origin_filtered: Option<Vec<TypeId>>,

    // The same invocation includes effectful predicates and reconstruction.
    // Flat constituent count changes work/storage, not checked nesting.
    _depth: Option<crate::resources::DepthGuard>,
}

impl TypeFilter {
    pub(super) fn next(&mut self, model: &Model) -> Option<TypeId> {
        if self.origin_filtered.is_none() {
            if let Some(&ty) = self.types.get(self.cursor) {
                self.cursor += 1;
                return Some(ty);
            }
            if self.retained.len() == self.types.len() || self.origin.is_none() {
                return None;
            }
            self.origin_filtered = Some(Vec::new());
            self.cursor = 0;
        }
        while let Some(&ty) = self
            .origin
            .as_ref()
            .expect("origin pass has an origin")
            .get(self.cursor)
        {
            self.cursor += 1;
            if model.has_type_flags(ty, TypeFlags::UNION) {
                // Native origin filtering retains nested unions without a
                // predicate, but still visits and stores each such entry.
                self.retain(ty);
            } else {
                return Some(ty);
            }
        }
        None
    }

    pub(super) fn accept(&mut self, ty: TypeId, retain: bool) {
        if retain {
            self.retain(ty);
        }
    }

    fn retain(&mut self, ty: TypeId) {
        let retained = self.origin_filtered.as_mut().unwrap_or(&mut self.retained);
        retained.push(ty);
    }

    pub(super) fn finish(self, model: &mut Model) -> TypeId {
        model.filtered_type_with_origin(self.original, self.retained, self.origin_filtered)
    }
}

#[derive(Default)]
pub(super) struct Model {
    // Recursive model operations share the request guard. Loading supplies None.
    control: Option<crate::resources::Control>,
    symbol_order: IdentityOrder,
    synthetic_symbols: usize,
    types: Vec<Type>,
    object_flags: Vec<ObjectFlags>,
    symbols: Vec<TypeSymbol>,
    type_mappers: Vec<Option<MapperId>>,
    primitives: HashMap<PrimitiveName, TypeId>,
    specials: HashMap<SpecialName, TypeId>,
    automatic: Option<TypeId>,
    non_inferrable_any: Option<TypeId>,
    wildcard: Option<TypeId>,
    native_error: Option<TypeId>,
    unresolved: Option<TypeId>,
    error_alias_types: crate::resources::CheckedMap<keys::ErrorAliasKey, TypeId>,
    intrinsic_aliases: HashMap<TypeId, Arc<Alias>>,
    automatic_array: Option<TypeId>,
    fresh_literals: HashMap<TypeId, TypeId>,
    enum_literals: crate::resources::CheckedMap<LiteralKey, TypeId>,
    regular_literals: crate::resources::CheckedMap<LiteralKey, TypeId>,
    unions: crate::resources::CheckedMap<UnionKey, TypeId>,
    intersections: crate::resources::CheckedMap<IntersectionKey, NativeCompletion<TypeId>>,
    indexed_accesses: crate::resources::CheckedMap<IndexedAccessKey, TypeId>,
    substitutions: HashMap<(TypeId, TypeId), TypeId>,
    index_types: HashMap<(TypeId, bool), TypeId>,
    template_literals: crate::resources::CheckedMap<templates::TemplateLiteralKey, TypeId>,
    string_mappings: HashMap<(AliasSymbol, TypeId), TypeId>,
    intrinsic_marker: Option<TypeId>,
    strict_null_checks: bool,
    exact_optional_property_types: bool,
    missing: Option<TypeId>,
    optional_marker: Option<TypeId>,
    widening_null: Option<TypeId>,
    widening_undefined: Option<TypeId>,
    implicit_never: Option<TypeId>,
    unreachable_never: Option<TypeId>,
    silent_never: Option<TypeId>,
    unique_literal: Option<TypeId>,
    unique_literal_mapper: Option<MapperId>,
    permissive_mapper: Option<MapperId>,
    restrictive_mapper: Option<MapperId>,
    signatures: usize,
    empty_object: Option<TypeId>,
    empty_generic: Option<TypeId>,
    empty_type_literal: Option<TypeId>,
    unknown_union: Option<TypeId>,
    unknown_empty_object: Option<TypeId>,
    any_function: Option<TypeId>,
    any_base_index: Option<Index>,
    enum_number_index: Option<Index>,
    string_number_symbol: Option<TypeId>,
    string_or_number: Option<TypeId>,
    typeof_type: Option<TypeId>,
    numeric_string: Option<TypeId>,
    template_constraint: Option<TypeId>,
    markers: Option<MarkerTypes>,
    references: crate::resources::CheckedMap<ReferenceKey, TypeId>,
    array_literals: HashMap<TypeId, TypeId>,
    mappers: Vec<Mapper>,
    report_unreliable_mapper: Option<MapperId>,
    report_unmeasurable_mapper: Option<MapperId>,
}

impl Model {
    /// The native wildcard is a distinct Any intrinsic, used only by
    /// permissive instantiation and inference; it is not error recovery.
    pub(super) fn wildcard_type(&self) -> TypeId {
        self.wildcard.expect("startup wildcard intrinsic")
    }

    /// `errorType` is a native recovery identity, never genuine Any. Consumers
    /// that continue with it must retain the separate product failure and
    /// return that failure before admitting a public result.
    pub(super) fn native_error_type(&self) -> TypeId {
        self.native_error.expect("initialized native errorType")
    }

    /// `getUnresolvedSymbolForEntityName` installs this distinct intrinsic in
    /// the synthetic TypeAlias's declaredType; references create aliased errors.
    pub(super) fn unresolved_type(&self) -> TypeId {
        self.unresolved.expect("initialized native unresolvedType")
    }

    /// `getTypeFromTypeAliasReference` interns an intrinsic error Any by the
    /// actual unresolved alias and its complete, ordered type-argument vector.
    pub(super) fn unresolved_alias_type(&mut self, alias: Alias) -> TypeId {
        let key = keys::ErrorAliasKey::new(&alias);
        if let Some(&ty) = self.error_alias_types.get(&key) {
            return ty;
        }
        let ty = self.push(Type::Special(SpecialName::Any));
        self.intrinsic_aliases.insert(ty, Arc::new(alias));
        self.error_alias_types.insert(key, ty);
        ty
    }

    /// `isErrorType` also recognizes Any with an actual alias association.
    pub(super) fn is_error_type(&self, ty: TypeId) -> bool {
        Some(ty) == self.native_error
            || self.has_type_flags(ty, TypeFlags::ANY) && self.alias(ty).is_some()
    }

    pub(super) fn is_wildcard_type(&self, ty: TypeId) -> bool {
        self.wildcard == Some(ty)
    }

    pub(super) fn permissive_mapper(&self) -> MapperId {
        self.permissive_mapper.expect("startup permissive mapper")
    }

    pub(super) fn restrictive_mapper(&self) -> MapperId {
        self.restrictive_mapper.expect("startup restrictive mapper")
    }

    /// Actual transient symbol allocation is separate from the first semantic
    /// observation used by the request's lazy ordering owner.
    pub(super) fn new_symbol_identity(&mut self) -> NativeSymbolIdentity {
        let identity = NativeSymbolIdentity::Transient(self.synthetic_symbols);
        self.synthetic_symbols += 1;
        identity
    }

    /// A semantic key write observes identity without exposing numeric ranks
    /// or sharing mutable order with another Project request.
    pub(super) fn observe_symbol_identity(&self, identity: NativeSymbolIdentity) {
        self.symbol_order.observe(identity);
    }

    pub(super) fn compare_native_symbols(
        &self,
        left: Option<&NativeSymbol>,
        right: Option<&NativeSymbol>,
    ) -> Result<Ordering, TypeBindingFailure> {
        self.symbol_order
            .compare_symbols(left, right, self.control.as_ref())
    }

    /// newTypeMapper allocates a native mapper identity on each call. Equal
    /// mappings are not interned: active instantiation caches and native limits
    /// observe mapper pointer identity. Evaluator owns actual substitution.
    pub(super) fn mapper(
        &mut self,
        sources: Vec<TypeId>,
        targets: Vec<TypeId>,
    ) -> Result<MapperId, DemandError> {
        self.mapper_targets(
            sources,
            targets.into_iter().map(MapperTarget::Known).collect(),
        )
    }

    pub(super) fn mapper_targets(
        &mut self,
        sources: Vec<TypeId>,
        targets: Vec<MapperTarget>,
    ) -> Result<MapperId, DemandError> {
        if sources.len() != targets.len() {
            return Err(super::internal("native type mapper arity mismatch").into());
        }
        Ok(self.push_mapper(Mapper::Simple { sources, targets }))
    }

    /// `combineTypeMappers` retains the two ordered native mapper identities.
    pub(super) fn combine_mappers(&mut self, first: MapperId, second: MapperId) -> MapperId {
        self.push_mapper(Mapper::Composite { first, second })
    }

    /// mergeTypeMappers maps through each operand without instantiating the
    /// first result. Composite is deliberately a different operation.
    pub(super) fn merge_mappers(&mut self, first: MapperId, second: MapperId) -> MapperId {
        self.push_mapper(Mapper::Merged { first, second })
    }

    /// newInferenceContext constructs distinct fixing/non-fixing callbacks;
    /// Model retains their opaque identity and never invokes the callbacks.
    pub(super) fn inference_mapper(
        &mut self,
        context: InferenceContextId,
        fixing: bool,
    ) -> MapperId {
        self.push_mapper(Mapper::Inference { context, fixing })
    }

    /// newArrayToSingleTypeMapper is used by newBackreferenceMapper with the
    /// actual forward parameter list and the initialized unknown intrinsic.
    pub(super) fn array_to_single_mapper(
        &mut self,
        sources: Vec<TypeId>,
        target: TypeId,
    ) -> MapperId {
        self.push_mapper(Mapper::ArrayToSingle { sources, target })
    }

    pub(super) fn mapper_data(&self, id: MapperId) -> &Mapper {
        &self.mappers[id.0]
    }

    /// `newDeferredTypeMapper` allocates fresh callback identity. The source
    /// list supports the native MapsThisOnly question without calling it.
    pub(super) fn deferred_type_mapper(
        &mut self,
        id: DeferredMapperId,
        sources: Vec<TypeId>,
    ) -> MapperId {
        self.push_mapper(Mapper::Deferred { id, sources })
    }

    /// SimpleTypeMapper/ArrayTypeMapper.MapsThisOnly; inference and composition
    /// inherit TypeMapperBase's false result.
    pub(super) fn mapper_maps_this_only(&self, mapper: MapperId) -> Result<bool, DemandError> {
        match self.mapper_data(mapper) {
            Mapper::Simple { sources, .. }
            | Mapper::ArrayToSingle { sources, .. }
            | Mapper::Deferred { sources, .. }
                if sources.len() == 1 =>
            {
                Ok(
                    matches!(self.get(sources[0])?, TypeView::TypeParameter(parameter) if parameter.is_this),
                )
            }
            Mapper::Simple { .. }
            | Mapper::Composite { .. }
            | Mapper::Merged { .. }
            | Mapper::Inference { .. }
            | Mapper::ReportReliability { .. }
            | Mapper::UniqueLiteral
            | Mapper::Permissive
            | Mapper::Restrictive
            | Mapper::Deferred { .. }
            | Mapper::ArrayToSingle { .. } => Ok(false),
        }
    }

    /// Observe the factory's actual source symbol without demanding members.
    pub(super) fn bound_type_symbol(&self, ty: TypeId) -> Result<Option<BoundSymbol>, DemandError> {
        match &self.symbols[ty.0] {
            TypeSymbol::Bound { symbol, .. } => Ok(Some(*symbol)),
            TypeSymbol::Absent | TypeSymbol::Synthetic { .. } | TypeSymbol::Property { .. } => {
                Ok(None)
            }
        }
    }

    pub(super) fn type_symbol(&self, ty: TypeId) -> TypeSymbol {
        self.symbols[ty.0].clone()
    }

    /// Observe native Type.symbol presence, independently from symbol header
    /// questions such as spelling, ordering or declarations.
    pub(super) fn has_type_symbol(&self, ty: TypeId) -> Result<bool, DemandError> {
        match &self.symbols[ty.0] {
            TypeSymbol::Absent => Ok(false),
            TypeSymbol::Bound { .. }
            | TypeSymbol::Property { .. }
            | TypeSymbol::Synthetic { .. } => Ok(true),
        }
    }

    pub(super) fn new(
        strict_null_checks: bool,
        exact_optional_property_types: bool,
        control: Option<crate::resources::Control>,
    ) -> Self {
        let mut model = Self {
            regular_literals: crate::resources::CheckedMap::new(),
            enum_literals: crate::resources::CheckedMap::new(),
            template_literals: crate::resources::CheckedMap::new(),
            references: crate::resources::CheckedMap::new(),
            intersections: crate::resources::CheckedMap::new(),
            unions: crate::resources::CheckedMap::new(),
            indexed_accesses: crate::resources::CheckedMap::new(),
            error_alias_types: crate::resources::CheckedMap::new(),
            control,
            strict_null_checks,
            exact_optional_property_types,
            ..Self::default()
        };
        model.initialize_intrinsics();
        model
    }

    /// initializeTypeCaches allocates established intrinsics in native source
    /// order, before any user type. CompareTypes' final ID fallback observes
    /// this order; lazily creating intrinsics at their first query is wrong.
    fn initialize_intrinsics(&mut self) {
        self.special(SpecialName::Any);
        self.automatic_variable();
        self.wildcard = Some(self.push(Type::Wildcard));
        // Native errorType has Any flags but its own intrinsic identity.
        self.native_error = Some(self.push(Type::Special(SpecialName::Any)));
        self.unresolved = Some(self.push(Type::Special(SpecialName::Any)));
        self.non_inferrable_any = Some(self.push(Type::NonInferrableAny));
        self.intrinsic_marker = Some(self.push(Type::IntrinsicMarker));
        self.special(SpecialName::Unknown);
        self.special(SpecialName::Undefined);
        self.undefined_initializer();
        self.missing_type();
        self.optional_marker_type();
        self.special(SpecialName::Null);
        self.null_initializer();
        let string = self.primitive(PrimitiveName::String);
        let number = self.primitive(PrimitiveName::Number);
        let bigint = self.primitive(PrimitiveName::Bigint);
        self.primitive(PrimitiveName::Boolean);
        let symbol = self.primitive(PrimitiveName::Symbol);
        self.special(SpecialName::Void);
        self.special(SpecialName::Never);
        self.silent_never();
        self.implicit_never = Some(self.push(Type::ImplicitNever));
        self.unreachable_never();
        self.primitive(PrimitiveName::Object);
        self.string_or_number = Some(self.union_sorted(vec![string, number]));
        self.string_number_symbol = Some(self.union_sorted(vec![string, number, symbol]));
        self.union_sorted(vec![number, bigint]);
        // getTemplateLiteralType(["", ""], [number]) is already normalized.
        self.numeric_string = Some(self.template_literal(
            vec![LiteralValue::from_text(""), LiteralValue::from_text("")],
            vec![number],
        ));
        let undefined = self.special(SpecialName::Undefined);
        let null = self.special(SpecialName::Null);
        self.template_constraint = Some(
            match self.union_with_reductions(
                &[
                    string,
                    number,
                    self.primitives[&PrimitiveName::Boolean],
                    bigint,
                    null,
                    undefined,
                ],
                UnionReduction::Literal,
                None,
                None,
                &HashMap::new(),
                &TypeListCache::new(),
                &TypeListCache::new(),
            ) {
                Ok(ty) => ty,
                Err(_) => unreachable!("primitive startup union has no semantic dependencies"),
            },
        );
        self.unique_literal = Some(self.push(Type::UniqueLiteral));
        self.unique_literal_mapper = Some(self.push_mapper(Mapper::UniqueLiteral));
        self.report_unreliable_mapper = Some(self.push_mapper(Mapper::ReportReliability {
            unmeasurable: false,
        }));
        self.report_unmeasurable_mapper =
            Some(self.push_mapper(Mapper::ReportReliability { unmeasurable: true }));
        self.restrictive_mapper = Some(self.push_mapper(Mapper::Restrictive));
        self.permissive_mapper = Some(self.push_mapper(Mapper::Permissive));
        self.empty_object_type();
        self.empty_generic_type();
        self.empty_type_literal_type();
        let empty = self.structured(
            TypeSymbol::Absent,
            own_properties(Vec::new()),
            Metadata::default(),
            ObjectKind::Anonymous,
        );
        self.unknown_empty_object = Some(empty);
        self.unknown_union();
        self.any_function = Some(self.structured_with_flags(
            TypeSymbol::Absent,
            own_properties(Vec::new()),
            Metadata::default(),
            ObjectKind::Anonymous,
            ObjectFlags::NON_INFERRABLE_TYPE,
        ));
        // initializeTypeCaches uses five distinct newTypeParameter(nil)
        // identities before the empty literal intrinsics.
        let super_type = self.marker_parameter(None);
        let sub_type = self.marker_parameter(Some(super_type));
        let other_type = self.marker_parameter(None);
        let super_for_check = self.marker_parameter(None);
        let sub_for_check = self.marker_parameter(Some(super_for_check));
        self.markers = Some(MarkerTypes {
            super_type,
            sub_type,
            other_type,
            super_for_check,
            sub_for_check,
        });
        self.literal(PrimitiveName::String, String::new());
        self.literal(PrimitiveName::Number, "0".into());
        self.literal(PrimitiveName::Bigint, "0".into());
        let names = [
            "bigint",
            "boolean",
            "function",
            "number",
            "object",
            "string",
            "symbol",
            "undefined",
        ];
        let types = names
            .into_iter()
            .map(|name| self.regular_literal_type(PrimitiveName::String, name))
            .collect();
        self.typeof_type = Some(self.union_sorted(types));
        let any = self.special(SpecialName::Any);
        self.any_base_index = Some(self.index_info(string, any, false, None, None));
        let number = self.primitive(PrimitiveName::Number);
        self.enum_number_index = Some(self.index_info(number, string, true, None, None));
    }

    /// `initializeTypeCaches` creates one `anyBaseTypeIndexInfo`, shared by
    /// static and instance inheritance and excluded by getApplicableIndexSymbol.
    pub(super) fn any_base_index(&self) -> Index {
        self.any_base_index
            .as_ref()
            .expect("startup Any base index")
            .clone()
    }

    /// The startup `enumNumberIndexInfo` identity is shared by enum inventories.
    pub(super) fn enum_number_index(&self) -> Index {
        self.enum_number_index
            .as_ref()
            .expect("startup enum number index")
            .clone()
    }

    /// `initializeTypeCaches` allocates typeofType from sorted typeofNEFacts
    /// keys. `checkTypeOfExpression` returns this established union identity.
    pub(super) fn typeof_type(&self) -> TypeId {
        self.typeof_type.expect("native intrinsics initialized")
    }

    /// The initializeTypeCaches stringNumberSymbolType intrinsic is shared by
    /// getApparentType(Index) and checkComputedPropertyName.
    pub(super) fn string_number_symbol_type(&self) -> TypeId {
        self.string_number_symbol
            .expect("native intrinsics initialized")
    }

    /// Native initialized `stringOrNumberType`, shared by string indexes.
    pub(super) fn string_or_number_type(&self) -> TypeId {
        self.string_or_number
            .expect("native stringOrNumberType initialized")
    }

    /// `initializeTypeCaches`' anyFunctionType is a distinct resolved empty
    /// anonymous sentinel for skipped contextual function checking.
    pub(super) fn any_function_type(&self) -> TypeId {
        self.any_function.expect("native intrinsics initialized")
    }

    /// `isEmptyResolvedType`: complete member collections are required, and
    /// the native skipped-function sentinel is never an empty object type.
    pub(super) fn is_empty_resolved_type(&self, ty: TypeId, members: &MembersView) -> bool {
        ty != self.any_function_type()
            && members.properties.is_empty()
            && members.call_signatures.is_empty()
            && members.construct_signatures.is_empty()
            && members.index_signatures.is_empty()
    }

    /// initializeTypeCaches retains these actual FunctionTypeMapper identities;
    /// their callbacks belong to the current relation reliability operation.
    pub(super) fn report_reliability_mapper(&self, unmeasurable: bool) -> MapperId {
        (if unmeasurable {
            self.report_unmeasurable_mapper
        } else {
            self.report_unreliable_mapper
        })
        .expect("native mapper intrinsics initialized")
    }

    pub(super) fn get(&self, id: TypeId) -> Result<TypeView, DemandError> {
        Ok(match &self.types[id.0] {
            Type::UniqueSymbol(key) => TypeView::UniqueSymbol(key.clone()),
            Type::IntrinsicMarker => TypeView::Special(SpecialName::Any),
            Type::StringMapping(data) => TypeView::StringMapping(data.clone()),
            Type::TemplateLiteral(data) => TypeView::TemplateLiteral(data.clone()),
            Type::Special(name) => TypeView::Special(*name),
            Type::AutomaticAny | Type::NonInferrableAny | Type::Wildcard => {
                TypeView::Special(SpecialName::Any)
            }
            Type::Missing | Type::OptionalMarker => TypeView::Special(SpecialName::Undefined),
            Type::WideningNull => TypeView::Special(SpecialName::Null),
            Type::WideningUndefined => TypeView::Special(SpecialName::Undefined),
            Type::ImplicitNever => TypeView::Special(SpecialName::Never),
            Type::UnreachableNever => TypeView::Special(SpecialName::Never),
            Type::SilentNever | Type::UniqueLiteral => TypeView::Special(SpecialName::Never),
            Type::Parameter(parameter) => TypeView::TypeParameter(parameter.clone()),
            Type::ParameterClone { state, .. } => match state.as_ref() {
                ParameterCloneState::Complete(parameter) => {
                    TypeView::TypeParameter(parameter.clone())
                }
                ParameterCloneState::Allocating => {
                    return Err(super::internal(
                        "parameter clone description precedes its mapper completion",
                    )
                    .into());
                }
                ParameterCloneState::Failed(error) => return Err(error.clone()),
            },
            Type::Target(_) => self.reference_view(id, id)?,
            Type::Reference { target, .. } => self.reference_view(id, *target)?,
            Type::Primitive(name) => TypeView::Primitive(*name),
            Type::Index(data) => TypeView::Index(*data),
            Type::IndexedAccess { data, .. } => TypeView::IndexedAccess(*data),
            Type::Conditional { data, .. } => TypeView::Conditional(*data),
            Type::Substitution(data) => TypeView::Substitution(*data),
            Type::Enum { .. } => TypeView::Enum,
            Type::Literal { base, value, .. } => TypeView::Literal {
                base: *base,
                value: value.clone(),
            },
            Type::Object { members, .. } => TypeView::Object(members.clone()),
            Type::LazyObject { .. } => TypeView::LazyObject(LazyObjectView {
                members: MembersOfType(id),
            }),
            Type::Union { boolean: true, .. } => TypeView::Primitive(PrimitiveName::Boolean),
            Type::Union { alternatives, .. } => TypeView::Union(Arc::clone(alternatives)),
            Type::Intersection { constituents, .. } => {
                TypeView::Intersection(Arc::clone(constituents))
            }
        })
    }

    pub(super) fn metadata(&self, id: TypeId) -> Result<MetadataView, DemandError> {
        let mut metadata = match &self.types[id.0] {
            Type::IntrinsicMarker => MetadataView {
                nominal_name: Some("intrinsic".into()),
                ..MetadataView::default()
            },
            Type::ParameterClone { .. } => {
                self.get(id)?;
                MetadataView::default()
            }
            Type::Object { metadata, .. } | Type::LazyObject { metadata, .. } => metadata.clone(),
            Type::Union { alias, origin, .. } => MetadataView {
                alias: alias.clone(),
                summary_origin: *origin,
                ..MetadataView::default()
            },
            Type::Intersection { alias, .. }
            | Type::IndexedAccess { alias, .. }
            | Type::Conditional { alias, .. } => MetadataView {
                alias: alias.clone(),
                ..MetadataView::default()
            },
            Type::Reference { alias, .. } => MetadataView {
                alias: alias.clone(),
                ..MetadataView::default()
            },
            _ => MetadataView::default(),
        };
        if let Some(alias) = self.intrinsic_aliases.get(&id) {
            metadata.alias = Some(alias.clone());
        }
        if let Some(arguments) = metadata
            .alias
            .as_ref()
            .and_then(|alias| alias.arguments.as_ref())
        {
            metadata.type_arguments = (!arguments.is_empty())
                .then(|| arguments.iter().copied().map(TypeDemand::Known).collect());
        }
        Ok(metadata)
    }

    /// Attach the complete native function-symbol serialization fact only
    /// after its actual anonymous function type is established. It is not an
    /// alias and does not replace the ordinary first-visit arrow summary.
    pub(super) fn attach_function_symbol(
        &mut self,
        id: TypeId,
        serialization: FunctionSymbolSerialization,
    ) -> Result<(), DemandError> {
        let metadata = match &mut self.types[id.0] {
            Type::Object {
                metadata,
                kind: ObjectKind::Anonymous,
                ..
            }
            | Type::LazyObject { metadata } => metadata,
            _ => {
                return Err(super::internal(
                    "function symbol requires an established anonymous type",
                )
                .into());
            }
        };
        if metadata.function_symbol.is_some() {
            return Err(
                super::internal("function symbol serialization is already established").into(),
            );
        }
        metadata.function_symbol = Some(serialization);
        Ok(())
    }

    /// `getDeclaredTypeOfClassOrInterface` allocates and installs the actual
    /// target before acquiring parameters. Only its deep evaluator operation
    /// manages begin/complete/fail; ordinary callers never see a fake type.
    pub(super) fn begin_declared_target(&mut self, header: TargetHeader) -> TypeId {
        let symbol = header.symbol.clone();
        let id = self.push(Type::Target(Box::new(TargetState::Allocating(header))));
        self.symbols[id.0] = symbol;
        id
    }

    pub(super) fn complete_declared_target(
        &mut self,
        id: TypeId,
        outer_parameters: Vec<TypeId>,
        local_parameters: Vec<TypeId>,
        needs_this: bool,
    ) -> Result<TargetView, DemandError> {
        let Type::Target(state) = &self.types[id.0] else {
            return Err(
                super::internal("declared target completion has a different type family").into(),
            );
        };
        let TargetState::Allocating(header) = state.as_ref() else {
            return Err(
                super::internal("declared target completion did not follow allocation").into(),
            );
        };
        let kind = header.kind;
        let name: Arc<str> = header.name.clone().into();
        let mut parameters = outer_parameters.clone();
        parameters.extend_from_slice(&local_parameters);
        let actual_arguments = self.share_type_list(&parameters);
        let this_parameter = needs_this.then(|| {
            self.type_parameter(
                self.symbols[id.0].clone(),
                ParameterData {
                    name: self.fixed_parameter_text("this"),
                    display_name: self.fixed_parameter_text("this"),
                    constraint: Some(TypeDemand::Known(id)),
                    default: None,
                    variance: None,
                    constant: false,
                    is_this: true,
                },
            )
        });
        parameters.extend(this_parameter);
        let view = TargetView {
            target: id,
            name,
            outer_parameters: outer_parameters.into(),
            local_parameters: local_parameters.into(),
            all_parameters: parameters.into(),
            this_parameter,
            is_reference: needs_this || !actual_arguments.is_empty(),
            tuple: None,
        };
        let state = match kind {
            TargetKind::Class => TargetState::Class(view.clone()),
            TargetKind::Interface => TargetState::Interface(view.clone()),
            TargetKind::Tuple => TargetState::Tuple(view.clone()),
        };
        self.types[id.0] = Type::Target(Box::new(state));
        if view.is_reference {
            self.object_flags[id.0] = self.object_flags[id.0] | ObjectFlags::REFERENCE;
        }
        // The target is its own reference at its outer/local parameters. The
        // synthetic this argument belongs to mapper padding, not this key.
        self.references
            .insert(ReferenceKey::new(id, actual_arguments), id);
        Ok(view)
    }

    /// A partially published target may already be reachable from another
    /// established type. Poison preserves its original failure at every view
    /// or later reference acquisition instead of exposing guessed empties.
    pub(super) fn fail_declared_target(&mut self, id: TypeId, failure: DemandError) {
        let Type::Target(state) = &mut self.types[id.0] else {
            unreachable!("declared target failure has a different type family")
        };
        **state = TargetState::Failed(failure);
    }

    /// `isThislessInterface` observes the actual target's thisType while
    /// construction can still be in progress. Native nil here is not an
    /// established empty target or an invitation to describe its members.
    pub(super) fn target_this_parameter(&self, id: TypeId) -> Result<Option<TypeId>, DemandError> {
        match &self.types[id.0] {
            Type::Target(state) => match state.as_ref() {
                TargetState::Allocating(_) => Ok(None),
                TargetState::Class(view)
                | TargetState::Interface(view)
                | TargetState::Tuple(view) => Ok(view.this_parameter),
                TargetState::Failed(error) => Err(error.clone()),
            },
            _ => Err(super::internal("thisType observation requires a declared target").into()),
        }
    }

    pub(super) fn declared_target(&self, id: TypeId) -> Result<TargetView, DemandError> {
        match &self.types[id.0] {
            Type::Target(state) => match state.as_ref() {
                TargetState::Class(view)
                | TargetState::Interface(view)
                | TargetState::Tuple(view) => Ok(view.clone()),
                TargetState::Failed(failure) => Err(failure.clone()),
                TargetState::Allocating(_) => Err(super::internal(
                    "declared target was described before construction completed",
                )
                .into()),
            },
            _ => Err(
                super::internal("declared target acquisition has a different type family").into(),
            ),
        }
    }

    /// `createTupleTargetType` publishes the nil-symbol target only after its
    /// element parameters and length property have been created. Its separate
    /// synthetic this occupies the last mapper slot, never the reference key.
    pub(super) fn tuple_target_type(
        &mut self,
        parameters: Vec<TypeId>,
        tuple: TupleTarget,
    ) -> Result<TypeId, DemandError> {
        let id = self.begin_declared_target(TargetHeader {
            symbol: TypeSymbol::Absent,
            kind: TargetKind::Tuple,
            name: String::new(),
        });
        self.complete_declared_target(id, Vec::new(), parameters, true)?;
        let Type::Target(state) = &mut self.types[id.0] else {
            unreachable!()
        };
        let TargetState::Tuple(view) = state.as_mut() else {
            unreachable!()
        };
        view.tuple = Some(Rc::new(tuple));
        Ok(id)
    }

    /// Native `isTupleType` observes Reference on the instance and Tuple on
    /// its target. No argument or member acquisition belongs to this question.
    pub(super) fn is_tuple_type(&self, id: TypeId) -> bool {
        self.object_flags[id.0].contains(ObjectFlags::REFERENCE)
            && self
                .reference_target(id)
                .is_some_and(|target| self.object_flags[target.0].contains(ObjectFlags::TUPLE))
    }

    pub(super) fn is_generic_tuple_type(&self, id: TypeId) -> Result<bool, DemandError> {
        Ok(self
            .tuple_target(id)?
            .is_some_and(|tuple| tuple.combined_flags.intersects(TupleElementFlags::VARIADIC)))
    }

    pub(super) fn unique_literal_type(&self) -> TypeId {
        self.unique_literal
            .expect("native uniqueLiteralType initialized")
    }

    pub(super) fn unique_literal_mapper(&self) -> MapperId {
        self.unique_literal_mapper
            .expect("native uniqueLiteralMapper initialized")
    }

    pub(super) fn tuple_target(&self, id: TypeId) -> Result<Option<Rc<TupleTarget>>, DemandError> {
        if !self.is_tuple_type(id) {
            return Ok(None);
        }
        Ok(self
            .declared_target(self.reference_target(id).expect("tuple reference target"))?
            .tuple)
    }

    /// `newTypeParameter(nil)` is shared by tuple elements and other actual
    /// synthetic parameters; absence of a symbol is a retained factory fact.
    pub(super) fn synthetic_type_parameter(&mut self) -> TypeId {
        self.marker_parameter(None)
    }

    pub(super) fn is_synthetic_parameter(&self, ty: TypeId) -> bool {
        self.flags(ty).intersects(TypeFlags::TYPE_PARAMETER)
            && matches!(self.symbols[ty.0], TypeSymbol::Absent)
    }

    pub(super) fn deferred_reference_type(
        &mut self,
        target: TypeId,
        deferred: DeferredReference,
        alias: Option<Alias>,
    ) -> Result<TypeId, DemandError> {
        self.declared_target(target)?;
        let id = self.push(Type::Reference {
            target,
            arguments: TypeArgumentsState::Pending,
            deferred: Some(deferred),
            alias: alias.map(Arc::new),
            array_literal: false,
        });
        self.type_mappers[id.0] = deferred.mapper;
        Ok(id)
    }

    pub(super) fn deferred_reference(&self, ty: TypeId) -> Option<DeferredReference> {
        match &self.types[ty.0] {
            Type::Reference { deferred, .. } => *deferred,
            _ => None,
        }
    }

    /// Native reference arity is available independently from its deferred
    /// argument values. Manifest deferred recipes have exactly the retained
    /// outer/local parameter count; installed vectors may also carry this.
    /// A cached child failure is consumed only by getTypeArguments, not by
    /// display metadata that can still omit the whole argument collection.
    pub(super) fn reference_arguments_for_graph(
        &self,
        ty: TypeId,
    ) -> Result<Option<ReferenceArgumentsForGraph>, DemandError> {
        let Some(target) = self.reference_target(ty) else {
            return Ok(None);
        };
        if let Type::Reference {
            arguments: TypeArgumentsState::Installed(arguments),
            ..
        } = &self.types[ty.0]
        {
            return Ok(Some(ReferenceArgumentsForGraph {
                length: arguments.value().len(),
                established: arguments.clean_value().cloned(),
            }));
        }
        let target = self.declared_target(target)?;
        let length = target.outer_parameters.len() + target.local_parameters.len();
        let established = matches!(&self.types[ty.0], Type::Target(_)).then(|| {
            target
                .outer_parameters
                .iter()
                .chain(target.local_parameters.iter())
                .copied()
                .collect()
        });
        Ok(Some(ReferenceArgumentsForGraph {
            length,
            established,
        }))
    }

    /// Product admission for legacy argument consumers. Native operations use
    /// the completion getter; observing the slot does not mutate its evidence.
    pub(super) fn resolved_type_arguments(
        &self,
        ty: TypeId,
    ) -> Option<Result<Arc<[TypeId]>, DemandError>> {
        self.resolved_type_arguments_completion(ty)
            .map(|result| result.and_then(NativeCompletion::admit))
    }

    pub(super) fn resolved_type_arguments_completion(
        &self,
        ty: TypeId,
    ) -> Option<NativeResult<Arc<[TypeId]>>> {
        match &self.types[ty.0] {
            Type::Target(_) => Some(self.declared_target(ty).map(|target| {
                NativeCompletion::clean(
                    target
                        .outer_parameters
                        .iter()
                        .chain(target.local_parameters.iter())
                        .copied()
                        .collect(),
                )
            })),
            Type::Reference { arguments, .. } => match arguments {
                TypeArgumentsState::Pending => None,
                TypeArgumentsState::Installed(value) => Some(Ok(value.clone())),
                TypeArgumentsState::Failed(error) => Some(Err(error.clone())),
            },
            _ => Some(Err(super::internal(
                "type arguments require a reference identity",
            )
            .into())),
        }
    }

    pub(super) fn has_resolved_type_arguments(&self, ty: TypeId) -> bool {
        match &self.types[ty.0] {
            Type::Target(_) => true,
            Type::Reference { arguments, .. } => {
                matches!(arguments, TypeArgumentsState::Installed(_))
            }
            _ => false,
        }
    }

    /// `getTypeArguments` final nil check. Only the evaluator's actual native
    /// assignment calls this owner. A failed invocation preserves an earlier
    /// installed array; false pop alone never proves a value was published.
    pub(super) fn complete_type_arguments(
        &mut self,
        ty: TypeId,
        result: NativeResult<Arc<[TypeId]>>,
    ) -> NativeResult<Arc<[TypeId]>> {
        let Type::Reference { arguments, .. } = &mut self.types[ty.0] else {
            return Err(
                super::internal("deferred arguments completion requires a reference").into(),
            );
        };
        match result {
            Ok(value) => {
                if let TypeArgumentsState::Installed(installed) = arguments {
                    Ok(value.map(|_| installed.value().clone()))
                } else {
                    *arguments = TypeArgumentsState::Installed(value.clone());
                    Ok(value)
                }
            }
            Err(error) => {
                if !matches!(arguments, TypeArgumentsState::Installed(_)) {
                    *arguments = TypeArgumentsState::Failed(error.clone());
                }
                Err(error)
            }
        }
    }

    /// `createArrayLiteralType` / `cloneTypeReference` cache a distinct source
    /// literal reference and preserve its actual target/arguments. Widening
    /// later returns the canonical reference, even when arguments stay equal.
    pub(super) fn array_literal_type(&mut self, ty: TypeId) -> Result<TypeId, DemandError> {
        let TypeView::Reference(reference) = self.get(ty)? else {
            // Native non-Reference fallback is unchanged. Recovery admission
            // belongs to the consumed global-type operation before this call.
            return Ok(ty);
        };
        if !reference.is_reference {
            return Ok(ty);
        }
        if let Some(&literal) = self.array_literals.get(&ty) {
            return Ok(literal);
        }
        let id = self.clone_type_reference(ty)?;
        if let Type::Reference { array_literal, .. } = &mut self.types[id.0] {
            *array_literal = true;
        }
        self.object_flags[id.0] =
            ObjectFlags(self.object_flags[ty.0].0 & !ObjectFlags::MEMBERS_RESOLVED.0)
                | ObjectFlags::ARRAY_LITERAL
                | ObjectFlags::CONTAINS_OBJECT_OR_ARRAY_LITERAL;
        self.array_literals.insert(ty, id);
        Ok(id)
    }

    /// `cloneTypeReference`: a fresh identity copies the symbol, target,
    /// resolved arguments and flags except MembersResolved. It deliberately
    /// does not copy a deferred node, mapper or alias metadata.
    pub(super) fn clone_type_reference(&mut self, ty: TypeId) -> Result<TypeId, DemandError> {
        let TypeView::Reference(reference) = self.get(ty)? else {
            return Err(super::internal("cloneTypeReference requires a native reference").into());
        };
        let arguments = match self.resolved_type_arguments_completion(ty) {
            Some(result) => TypeArgumentsState::Installed(result?),
            None => TypeArgumentsState::Pending,
        };
        let id = self.push(Type::Reference {
            target: reference.target,
            arguments,
            deferred: None,
            alias: None,
            array_literal: false,
        });
        self.symbols[id.0] = self.symbols[ty.0].clone();
        self.object_flags[id.0] =
            ObjectFlags(self.object_flags[ty.0].0 & !ObjectFlags::MEMBERS_RESOLVED.0);
        Ok(id)
    }

    /// The native binding-pattern contextual Any is a distinct widening
    /// intrinsic, and inference tests its exact identity before candidates.
    pub(super) fn non_inferrable_any_type(&self) -> TypeId {
        self.non_inferrable_any
            .expect("startup binding-pattern intrinsic")
    }

    /// `getTypeFromArrayBindingPattern` attaches a pattern to a fresh clone,
    /// never to the tuple factory's interned reference.
    pub(super) fn binding_pattern_reference(&mut self, ty: TypeId) -> Result<TypeId, DemandError> {
        let result = self.clone_type_reference(ty)?;
        self.object_flags[result.0] =
            self.object_flags[result.0] | ObjectFlags::CONTAINS_OBJECT_OR_ARRAY_LITERAL;
        Ok(result)
    }

    fn reference_view(&self, id: TypeId, target: TypeId) -> Result<TypeView, DemandError> {
        let view = self.declared_target(target)?;
        Ok(TypeView::Reference(ReferenceView {
            ty: id,
            target,
            name: view.name,
            tuple: view.tuple,
            is_reference: view.is_reference,
            members: MembersOfType(id),
        }))
    }

    /// `getDeclaredTypeOfTypeParameter` establishes a distinct canonical
    /// parameter identity before resolving its constraint or default. The
    /// evaluator owns canonical-symbol caching and supplies separate demands.
    pub(super) fn type_parameter(
        &mut self,
        symbol: TypeSymbol,
        parameter: impl Into<ParameterInput>,
    ) -> TypeId {
        let parameter = match parameter.into() {
            ParameterInput::Existing(parameter) => parameter,
            ParameterInput::Owned(data) => {
                let names = {
                    std::sync::Arc::new(ParameterNames {
                        name: data.name,
                        display_name: data.display_name,
                    })
                };
                Parameter {
                    names,
                    constraint: data.constraint,
                    default: data.default,
                    variance: data.variance,
                    constant: data.constant,
                    is_this: data.is_this,
                }
            }
        };
        let id = self.push(Type::Parameter(parameter));
        self.symbols[id.0] = symbol;
        id
    }

    // Exact fixed spellings for thisType / nil-symbol native factories. The
    // producing operation owns the spelling; this only copies its bytes.
    fn fixed_parameter_text(&self, text: &str) -> String {
        let mut result = String::new();
        result.push_str(text);
        result
    }

    fn marker_parameter(&mut self, constraint: Option<TypeId>) -> TypeId {
        self.type_parameter(
            TypeSymbol::Absent,
            ParameterData {
                // typeToTypeNode's nil-symbol parameter fallback; no spelling
                // participates in symbol identity, recursion or mapper keys.
                name: self.fixed_parameter_text("?"),
                display_name: self.fixed_parameter_text("?"),
                constraint: constraint.map(TypeDemand::Known),
                default: None,
                variance: None,
                constant: false,
                is_this: false,
            },
        )
    }

    pub(super) fn marker_types(&self) -> MarkerTypes {
        self.markers.expect("native marker intrinsics initialized")
    }

    /// `getTypeParameterModifiers` projects the retained declared In/Out
    /// facts. None means neither modifier, distinct from explicit Invariant.
    pub(super) fn get_parameter_variance_modifiers(
        &self,
        ty: TypeId,
    ) -> Result<Option<VarianceFlags>, DemandError> {
        let TypeView::TypeParameter(parameter) = self.get(ty)? else {
            return Err(super::internal("variance modifiers require a type parameter").into());
        };
        Ok(parameter.variance.map(DeclaredVariance::flags))
    }

    /// Type.Target on a native cloned type parameter; no constraint or mapper
    /// is acquired by getCanonicalSignature's identity observation.
    pub(super) fn parameter_target(&self, ty: TypeId) -> Option<TypeId> {
        match self.types[ty.0] {
            Type::ParameterClone { target, .. } => Some(target),
            _ => None,
        }
    }

    /// `cloneTypeParameter` allocates fresh actual identity before all local
    /// clones are composed into their mapper. It retains its native target;
    /// incomplete clones cannot expose guessed absent constraints/defaults.
    pub(super) fn clone_parameter(&mut self, target: TypeId) -> Result<TypeId, DemandError> {
        if !matches!(self.get(target)?, TypeView::TypeParameter(_)) {
            return Err(super::internal("parameter clone has a non-parameter target").into());
        }
        Ok(self.push(Type::ParameterClone {
            target,
            state: Box::new(ParameterCloneState::Allocating),
        }))
    }

    /// `instantiateSignatureEx` stores the composed mapper on every fresh
    /// parameter before any immutable descriptor escapes the full operation.
    pub(super) fn complete_parameter_clone(
        &mut self,
        clone: TypeId,
        constraint: Option<TypeDemand>,
        default: Option<TypeDemand>,
    ) -> Result<(), DemandError> {
        let Type::ParameterClone { target, state } = &self.types[clone.0] else {
            return Err(super::internal("parameter clone completion has another family").into());
        };
        if !matches!(state.as_ref(), ParameterCloneState::Allocating) {
            return Err(
                super::internal("parameter clone completion did not follow allocation").into(),
            );
        }
        let TypeView::TypeParameter(mut parameter) = self.get(*target)? else {
            return Err(super::internal("parameter clone target changed its native family").into());
        };
        parameter.constraint = constraint;
        parameter.default = default;
        // `cloneTypeParameter` calls newTypeParameter(symbol), whose fresh
        // parameter is ordinary even if a broader caller supplied thisType.
        parameter.is_this = false;
        let Type::ParameterClone { state, .. } = &mut self.types[clone.0] else {
            unreachable!()
        };
        **state = ParameterCloneState::Complete(parameter);
        Ok(())
    }

    /// A failed complete factory poisons its actual early identities so any
    /// later dependency preserves the original cause rather than a fake view.
    pub(super) fn fail_parameter_clone(
        &mut self,
        clone: TypeId,
        error: DemandError,
    ) -> Result<(), DemandError> {
        let Type::ParameterClone { state, .. } = &mut self.types[clone.0] else {
            return Err(super::internal("failed clone attachment has another family").into());
        };
        **state = ParameterCloneState::Failed(error);
        Ok(())
    }

    /// `reportErrorResults` installs a known synthetic constraint after its
    /// substitution. The fresh clone is already a complete ordinary parameter
    /// during that substitution, with a genuinely empty constraint slot.
    pub(super) fn install_parameter_clone_constraint(
        &mut self,
        clone: TypeId,
        constraint: TypeId,
    ) -> Result<(), DemandError> {
        let Type::ParameterClone { state, .. } = &mut self.types[clone.0] else {
            return Err(super::internal("synthetic constraint has no clone").into());
        };
        let ParameterCloneState::Complete(parameter) = state.as_mut() else {
            return Err(super::internal("synthetic constraint clone is not complete").into());
        };
        parameter.constraint = Some(TypeDemand::Known(constraint));
        Ok(())
    }

    /// `newSignature` allocates one actual signature identity with fresh
    /// contextual slots. Native clone factories pass current field snapshots;
    /// Rust handle clones share the slots of the same identity.
    pub(super) fn signature(&mut self, data: SignatureData) -> Result<Signature, DemandError> {
        if data.type_parameter_types.len() != data.type_parameters.len() {
            return Err(super::internal(
                "signature canonical/display type-parameter cardinality differs",
            )
            .into());
        }
        let parameters = { data.parameters.into_shared() };
        let origins = { data.origins.into_shared() };

        let id = SignatureId(self.signatures);
        self.signatures = self.signatures.checked_add(1).ok_or_else(|| {
            super::internal("request-local signature identity capacity exhausted")
        })?;
        Ok(Signature {
            id,
            contextual: Arc::new(SignatureContextualFields {
                parameters: SignatureTypeParameters {
                    types: data.type_parameter_types,
                    display: data.type_parameters,
                },
                contextual_parameters: OnceLock::new(),
                this: data.this_parameter,
                contextual_this: OnceLock::new(),
            }),
            parameters,
            return_type: data.return_type,
            predicate: data.predicate,
            origins,
            minimum_arguments: data.minimum_arguments,
            flags: data.flags,
        })
    }

    /// Owned `getSignaturesOfType` projection preserves established IDs and
    /// contextual slots. Ordinary handle Clone is bounded: parameter/origin
    /// arrays and contextual state share immutable storage; predicate names
    /// already share text. This is not native `cloneSignature`'s fresh factory.
    pub(super) fn copy_signature_handles(&self, signatures: &[Signature]) -> Vec<Signature> {
        let mut result = Vec::with_capacity(signatures.len());
        for signature in signatures {
            result.push(signature.clone());
        }
        result
    }

    /// Owned `getExpandedParameters` snapshots copy exact names without
    /// changing the actual parameter symbols or declaration-owned transforms.
    pub(super) fn copy_parameter_values(
        &self,
        parameters: &[ParameterValue],
    ) -> Vec<ParameterValue> {
        let mut result = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            let mut name = String::new();
            name.push_str(&parameter.name);
            result.push(ParameterValue {
                symbol: parameter.symbol,
                name,
                declaration: parameter.declaration,
                value_type: parameter.value_type,
                call_type: parameter.call_type,
                display_type: parameter.display_type,
                optional: parameter.optional,
                rest: parameter.rest,
                transforms: parameter.transforms,
            });
        }
        result
    }

    /// Public metadata owns these exact retained location strings. Copying
    /// performs no declaration/child acquisition or location-policy decision.
    pub(super) fn copy_origins(origins: &[Location]) -> Vec<Location> {
        let mut result = Vec::with_capacity(origins.len());
        for origin in origins {
            result.push(crate::references::metadata::copy_location(origin));
        }
        result
    }

    /// `getRecursionIdentity` accesses an established reference's actual
    /// target without interpreting its printed name or acquiring members.
    pub(super) fn reference_target(&self, ty: TypeId) -> Option<TypeId> {
        match &self.types[ty.0] {
            Type::Reference { target, .. } => Some(*target),
            Type::Target(_) => Some(ty),
            _ => None,
        }
    }

    /// `containsMissingType` deliberately examines only the first ordered
    /// union constituent, not arbitrary nested undefined-like children.
    pub(super) fn contains_missing_type(&self, ty: TypeId) -> bool {
        Some(ty) == self.missing
            || self
                .union_constituents(ty)
                .is_some_and(|types| types.first().copied() == self.missing)
    }

    /// Native `missingType` is a distinct intrinsic with Undefined flags.
    /// Initialize genuine Undefined first, preserving its native ordering when
    /// both identities enter a union before literal reduction removes Missing.
    pub(super) fn missing_type(&mut self) -> TypeId {
        if let Some(id) = self.missing {
            return id;
        }
        self.special(SpecialName::Undefined);
        let id = self.push(Type::Missing);
        self.missing = Some(id);
        id
    }

    /// initializeTypeCaches creates optionalType after missingType. Optional
    /// chaining observes this exact identity, independently from property
    /// missingness and ordinary Undefined despite their shared native flags.
    pub(super) fn optional_marker_type(&mut self) -> TypeId {
        if let Some(id) = self.optional_marker {
            return id;
        }
        let id = self.push(Type::OptionalMarker);
        self.optional_marker = Some(id);
        id
    }

    /// `removeMissingType` / `removeType`: remove the exact private identity
    /// only for the exact-optional question. An equal displayed Undefined type
    /// remains distinct and unchanged.
    pub(super) fn remove_missing_type(
        &mut self,
        ty: TypeId,
        is_optional: bool,
    ) -> Result<TypeId, UnionFailure> {
        if !self.exact_optional_property_types || !is_optional {
            return Ok(ty);
        }
        let Some(missing) = self.missing else {
            return Ok(ty);
        };
        Ok(self.remove_type(ty, missing))
    }

    /// `removeType` delegates exact identity removal to the established
    /// filterType owner, preserving unchanged unions and their origin.
    pub(super) fn remove_type(&mut self, ty: TypeId, target: TypeId) -> TypeId {
        self.filter_type(ty, |_, child| child != target)
    }

    /// `filterType(t, constituent.flags & TypeFlagsUndefined == 0)` in the
    /// native member serializer removes both Undefined and Missing by flags.
    /// Established constituent filtering preserves unchanged canonical identity.
    pub(super) fn filter_undefined(&mut self, ty: TypeId) -> TypeId {
        self.filter_type(ty, |model, child| {
            !model.has_type_flags(child, TypeFlags::UNDEFINED)
        })
    }

    /// `containsUndefinedType` tests the native flag on the first canonical
    /// union constituent, not displayed text or equality with one intrinsic.
    pub(super) fn contains_undefined(&self, ty: TypeId) -> bool {
        let first = match &self.types[ty.0] {
            Type::Union { alternatives, .. } => alternatives[0],
            _ => ty,
        };
        self.has_type_flags(first, TypeFlags::UNDEFINED)
    }

    /// `containsNonMissingUndefinedType` has the same first-union-constituent
    /// gate as containsUndefinedType, excluding only the actual missingType.
    pub(super) fn contains_non_missing_undefined(&self, ty: TypeId) -> bool {
        self.contains_undefined(ty) && !self.contains_missing_type(ty)
    }

    /// `newUniqueESSymbolType` retains the actual symbol and generated key.
    pub(super) fn new_unique_es_symbol_type(
        &mut self,
        symbol: TypeSymbol,
        key: NativePropertyKey,
    ) -> TypeId {
        let ty = self.push(Type::UniqueSymbol(key));
        self.symbols[ty.0] = symbol;
        ty
    }

    pub(super) fn primitive(&mut self, name: PrimitiveName) -> TypeId {
        if let Some(&id) = self.primitives.get(&name) {
            return id;
        }
        let id = if name == PrimitiveName::Boolean {
            let fresh_false = self.literal(name, "false".into());
            let regular_false = self.regular_literal(fresh_false);
            let fresh_true = self.literal(name, "true".into());
            let regular_true = self.regular_literal(fresh_true);
            self.union_sorted(vec![regular_false, regular_true])
        } else {
            self.push(Type::Primitive(name))
        };
        self.primitives.insert(name, id);
        id
    }

    /// `newIntrinsicType` establishes shared genuine intrinsic identities.
    /// Recovery-any and missing slots are distinct compiler types, never
    /// aliases of this successful `any`/`undefined` construction.
    pub(super) fn special(&mut self, name: SpecialName) -> TypeId {
        if let Some(&id) = self.specials.get(&name) {
            return id;
        }
        let id = self.push(Type::Special(name));
        self.specials.insert(name, id);
        id
    }

    /// `initializeTypeCaches` creates autoType with NonInferrableType for
    /// `getTypeForVariableLikeDeclaration`. Its identity remains separate from
    /// genuine any, even when the bounded public description reads `any`.
    pub(super) fn automatic_variable(&mut self) -> TypeId {
        if let Some(id) = self.automatic {
            return id;
        }
        let id = self.push(Type::AutomaticAny);
        self.automatic = Some(id);
        id
    }

    /// `checkExpression` returns `nullWideningType`; `createWideningType`
    /// shares ordinary null under strictNullChecks and keeps a distinct
    /// ContainsWideningType identity otherwise. This is not literal freshness.
    pub(super) fn null_initializer(&mut self) -> TypeId {
        if self.strict_null_checks {
            return self.special(SpecialName::Null);
        }
        if let Some(id) = self.widening_null {
            return id;
        }
        let id = self.push(Type::WideningNull);
        self.widening_null = Some(id);
        id
    }

    /// `createWideningType(undefinedType)` supplies the shared undefined
    /// initializer identity. Strict mode shares genuine undefined; otherwise
    /// nullable widening remains separate from that successful intrinsic.
    pub(super) fn undefined_initializer(&mut self) -> TypeId {
        if self.strict_null_checks {
            return self.special(SpecialName::Undefined);
        }
        if let Some(id) = self.widening_undefined {
            return id;
        }
        let id = self.push(Type::WideningUndefined);
        self.widening_undefined = Some(id);
        id
    }

    /// `checkArrayLiteral`'s empty element question uses implicitNeverType
    /// under strictNullChecks and undefinedWideningType otherwise. Neither
    /// question can be replaced with an unqualified never/any fallback.
    pub(super) fn empty_array_element(&mut self) -> TypeId {
        if !self.strict_null_checks {
            return self.undefined_initializer();
        }
        if let Some(id) = self.implicit_never {
            return id;
        }
        let id = self.push(Type::ImplicitNever);
        self.implicit_never = Some(id);
        id
    }

    /// `getStringLiteralType` / `getNumberLiteralType` acquire only the regular
    /// identity. Merely asking for a literal type does not allocate its fresh
    /// counterpart; that separate native question goes through `literal`.
    pub(super) fn regular_literal_type(
        &mut self,
        base: PrimitiveName,
        value: impl AsRef<str>,
    ) -> TypeId {
        self.intern_regular_literal(base, LiteralValue::from_text(value))
    }

    /// `getStringLiteralType` accepts an exact native value, including generated
    /// property names. This joins source-text literals in the same interner and
    /// leaves fresh identity allocation to `getFreshTypeOfLiteralType`.
    pub(super) fn regular_string_literal_value(&mut self, value: LiteralValue) -> TypeId {
        self.intern_regular_literal(PrimitiveName::String, value)
    }

    pub(super) fn string_literal_for_key(&mut self, key: &NativePropertyKey) -> TypeId {
        self.regular_string_literal_value(LiteralValue::from_property_key(key))
    }

    fn intern_regular_literal(&mut self, base: PrimitiveName, value: LiteralValue) -> TypeId {
        let key = LiteralKey::new(base, None, value.clone());
        if let Some(&id) = self.regular_literals.get(&key) {
            return id;
        }
        let id = self.push(Type::Literal {
            base,
            value,
            enum_: false,
            regular: None,
        });
        self.regular_literals.insert(key, id);
        id
    }

    /// `getFreshTypeOfLiteralType` uses the regular identity, retaining enum
    /// symbol/flags rather than re-interning an equal primitive payload.
    pub(super) fn literal(&mut self, base: PrimitiveName, value: String) -> TypeId {
        let regular = self.regular_literal_type(base, value);
        self.fresh_literal(regular)
    }

    pub(super) fn is_fresh_literal(&self, id: TypeId) -> bool {
        matches!(
            self.types[id.0],
            Type::Literal {
                regular: Some(_),
                ..
            } | Type::Enum { regular: Some(_) }
        )
    }

    pub(super) fn alternate_literal_type(&self, id: TypeId) -> Option<TypeId> {
        match self.types[id.0] {
            Type::Literal {
                regular: Some(regular),
                ..
            }
            | Type::Enum {
                regular: Some(regular),
            } => Some(regular),
            Type::Literal { regular: None, .. } | Type::Enum { regular: None } => {
                self.fresh_literals.get(&id).copied()
            }
            _ => None,
        }
    }

    pub(super) fn fresh_literal(&mut self, id: TypeId) -> TypeId {
        if !self.has_type_flags(id, TypeFlags::FRESHABLE) || self.is_fresh_literal(id) {
            return id;
        }
        if let Some(&fresh) = self.fresh_literals.get(&id) {
            return fresh;
        }
        let value = match &self.types[id.0] {
            Type::Literal {
                base, value, enum_, ..
            } => Type::Literal {
                base: *base,
                value: value.clone(),
                enum_: *enum_,
                regular: Some(id),
            },
            Type::Enum { .. } => Type::Enum { regular: Some(id) },
            _ => unreachable!("Freshable factory must retain its literal pair"),
        };
        let fresh = self.push(value);
        self.symbols[fresh.0] = self.symbols[id.0].clone();
        self.fresh_literals.insert(id, fresh);
        fresh
    }

    /// `getEnumLiteralType`: the cache is keyed by actual enum identity and
    /// canonical value. Equal values in another enum never share a type.
    pub(super) fn enum_literal_type(
        &mut self,
        enumeration: NativeSymbolIdentity,
        symbol: TypeSymbol,
        base: PrimitiveName,
        value: LiteralValue,
    ) -> TypeId {
        let key = LiteralKey::new(base, Some(enumeration), value.clone());
        // Native uses enumNaNLiteralTypes for NaN. The canonical numeric
        // payload key here expresses the same per-enum singleton identity.
        if let Some(&ty) = self.enum_literals.get(&key) {
            return ty;
        }
        let ty = self.push(Type::Literal {
            base,
            value,
            enum_: true,
            regular: None,
        });
        self.symbols[ty.0] = symbol;
        self.enum_literals.insert(key, ty);
        ty
    }

    /// `createComputedEnumType` allocates both native identities eagerly;
    /// computed members have no numeric/string literal payload.
    pub(super) fn computed_enum_type(&mut self, symbol: TypeSymbol) -> TypeId {
        let regular = self.push(Type::Enum { regular: None });
        self.symbols[regular.0] = symbol;
        self.fresh_literal(regular);
        regular
    }

    pub(super) fn attach_enum_union_symbol(&mut self, ty: TypeId, symbol: TypeSymbol) {
        if let Type::Union { enum_, .. } = &mut self.types[ty.0] {
            *enum_ = true;
            self.symbols[ty.0] = symbol;
        }
    }

    /// `getWidenedLiteralType` for established ordinary primitive literals.
    /// The regular form remains literal; only a fresh form widens here.
    pub(super) fn widened_literal(&mut self, id: TypeId) -> TypeId {
        match &self.types[id.0] {
            Type::Literal {
                base,
                enum_: false,
                regular: Some(_),
                ..
            } => self.primitive(*base),
            _ => id,
        }
    }

    /// `getRegularTypeOfLiteralType` returns the cached regular identity of a
    /// fresh literal; established nonliteral identities remain unchanged.
    pub(super) fn regular_literal(&mut self, id: TypeId) -> TypeId {
        match &self.types[id.0] {
            Type::Literal {
                regular: Some(regular),
                ..
            }
            | Type::Enum {
                regular: Some(regular),
            } => *regular,
            Type::Union { alternatives, .. } => {
                let alternatives = alternatives.clone();
                let mapped: Vec<_> = alternatives
                    .iter()
                    .map(|&ty| self.regular_literal(ty))
                    .collect();
                if alternatives.as_ref() == mapped.as_slice() {
                    id
                } else {
                    self.union_sorted(mapped)
                }
            }
            _ => id,
        }
    }

    /// Native LiteralType.value equality, independent of fresh/enum identity.
    /// NaN is unequal to itself; both signed zeros have one canonical payload.
    pub(super) fn literal_value_equal(
        &self,
        left: TypeId,
        right: TypeId,
    ) -> Result<bool, LiteralError> {
        match (&self.types[left.0], &self.types[right.0]) {
            (
                Type::Literal {
                    base: a, value: av, ..
                },
                Type::Literal {
                    base: b, value: bv, ..
                },
            ) => {
                if a != b || *a == PrimitiveName::Number && av.equals_text("NaN") {
                    Ok(false)
                } else {
                    av.semantic_equal(bv)
                }
            }
            _ => Ok(false),
        }
    }

    /// Native UnionType.origin is established factory metadata; observing it
    /// does not acquire an origin's structure or reinterpret its kind.
    pub(super) fn union_origin(&self, ty: TypeId) -> Option<TypeId> {
        match &self.types[ty.0] {
            Type::Union { origin, .. } => *origin,
            _ => None,
        }
    }

    // Recursion exhaustion abandons the entire request; it never becomes a
    // resumable UnionFailure or a semantic comparison result.

    fn resource_depth(&self) -> Option<crate::resources::DepthGuard> {
        self.control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()))
    }

    /// Copies only fixed-size type identities. Separately owned type payloads
    /// remain in their arenas.
    pub(super) fn copy_type_list(&self, types: &[TypeId]) -> Vec<TypeId> {
        let mut copy = Vec::with_capacity(types.len());
        for &ty in types {
            copy.push(ty);
        }
        copy
    }

    pub(super) fn push_type(&self, types: &mut Vec<TypeId>, ty: TypeId) {
        types.push(ty);
    }

    pub(super) fn contains_type(&self, types: &[TypeId], ty: TypeId) -> bool {
        types.contains(&ty)
    }

    fn share_type_list(&self, types: &[TypeId]) -> Arc<[TypeId]> {
        // Actual allocator OOM remains outside unwind recovery.
        Arc::from(types)
    }

    /// `getUnionTypeWorker` owns pure ordered normalization and construction.
    /// Missing constraint/subtype answers suspend at their native operation;
    /// the evaluator supplies established results before the factory resumes.
    #[expect(
        clippy::too_many_arguments,
        reason = "getUnionTypeWorker's suspended native answers are explicit inputs"
    )]
    pub(super) fn union_with_reductions(
        &mut self,
        types: &[TypeId],
        reduction: UnionReduction,
        alias: Option<Alias>,
        mut origin: Option<TypeId>,
        constraints: &HashMap<TypeId, TypeId>,
        subtype_reductions: &TypeListCache<Arc<[TypeId]>>,
        string_reductions: &TypeListCache<Vec<TypeId>>,
    ) -> Result<TypeId, UnionFailure> {
        let _depth = self.resource_depth();
        if types.is_empty() {
            return Ok(self.special(SpecialName::Never));
        }
        if types.len() == 1 {
            return Ok(types[0]);
        }
        let mut flat = Vec::new();
        let mut pending = Vec::new();
        self.push_union_inputs(&mut pending, types);
        let mut included = Vec::new();
        let mut includes_named_union = false;
        // addTypesToUnion / addTypeToUnion visit source order and insert by
        // CompareTypes before reductions. This preserves comparison demand
        // order even when a later Any/Unknown absorbs the entire union.
        while let Some(id) = pending.pop() {
            if let Type::Union {
                alternatives,
                alias,
                origin,
                ..
            } = &self.types[id.0]
            {
                includes_named_union |= alias.is_some() || origin.is_some();
                self.push_union_inputs(&mut pending, alternatives);
            } else if !self.has_type_flags(id, TypeFlags::NEVER) {
                self.push_type(&mut included, id);
                if self.strict_null_checks || !self.has_type_flags(id, TypeFlags::NULLABLE) {
                    self.insert_union_type(&mut flat, id)?;
                }
            }
        }
        let precomputed_flags = self.union_object_flags(&included);
        let has_object_types = included
            .iter()
            .any(|&id| self.has_type_flags(id, TypeFlags::OBJECT));
        let had_null = included
            .iter()
            .any(|&id| self.has_type_flags(id, TypeFlags::NULL));
        let had_undefined = included
            .iter()
            .any(|&id| self.has_type_flags(id, TypeFlags::UNDEFINED));
        let non_widening_nullable = included.iter().any(|&id| {
            self.has_type_flags(id, TypeFlags::NULLABLE)
                && !self.object_flags[id.0].intersects(ObjectFlags::CONTAINS_WIDENING_TYPE)
        });
        if reduction != UnionReduction::None {
            if included
                .iter()
                .any(|&id| self.has_type_flags(id, TypeFlags::ANY))
            {
                if included.iter().any(|&id| self.is_wildcard_type(id)) {
                    return Ok(self.wildcard_type());
                }
                if included.iter().any(|&id| self.is_error_type(id)) {
                    return Ok(self.native_error_type());
                }
                return Ok(self.special(SpecialName::Any));
            }
            if included
                .iter()
                .any(|&id| matches!(self.types[id.0], Type::Special(SpecialName::Unknown)))
            {
                return Ok(self.special(SpecialName::Unknown));
            }
        }
        if reduction != UnionReduction::None {
            let contains_undefined = flat
                .iter()
                .any(|&id| matches!(self.types[id.0], Type::Special(SpecialName::Undefined)));
            if contains_undefined {
                flat.retain(|&id| !matches!(self.types[id.0], Type::Missing));
            }
            let includes = included
                .iter()
                .fold(TypeFlags::NONE, |flags, &ty| flags | self.flags(ty));
            let identities = self.copy_type_list(&flat);
            // `removeRedundantLiteralTypes`: use native categories, including
            // templates/string mappings and unique symbols, not display views.
            flat.retain(|&ty| {

                let flags=self.flags(ty);
                !(flags.intersects(TypeFlags::STRING_LITERAL|TypeFlags::TEMPLATE_LITERAL|TypeFlags::STRING_MAPPING) && includes.intersects(TypeFlags::STRING)
                    || flags.intersects(TypeFlags::NUMBER_LITERAL) && includes.intersects(TypeFlags::NUMBER)
                    || flags.intersects(TypeFlags::BIGINT_LITERAL) && includes.intersects(TypeFlags::BIGINT)
                    || flags.intersects(TypeFlags::UNIQUE_SYMBOL) && includes.intersects(TypeFlags::SYMBOL)
                    || reduction==UnionReduction::Subtype && flags.intersects(TypeFlags::UNDEFINED) && includes.intersects(TypeFlags::VOID)
                    || matches!(&self.types[ty.0],Type::Literal {regular:Some(regular),..} | Type::Enum { regular: Some(regular) } if self.contains_type(&identities, *regular)))
            });
            if includes.intersects(TypeFlags::STRING_LITERAL)
                && includes.intersects(TypeFlags::TEMPLATE_LITERAL | TypeFlags::STRING_MAPPING)
            {
                flat = match string_reductions.get(&flat) {
                    Some(reduced) => self.copy_type_list(reduced),
                    None => return Err(UnionFailure::StringLiteralReduction(flat)),
                };
            }
        }
        if reduction != UnionReduction::None {
            let mut variables = Vec::new();
            for &ty in &flat {
                if self.has_type_flags(ty, TypeFlags::INTERSECTION)
                    && self.object_flags[ty.0].intersects(ObjectFlags::IS_CONSTRAINED_TYPE_VARIABLE)
                {
                    let types = self
                        .intersection_constituents(ty)
                        .expect("intersection checked");
                    let variable = types
                        [usize::from(!self.has_type_flags(types[0], TypeFlags::TYPE_VARIABLE))];
                    if !self.contains_type(&variables, variable) {
                        self.push_type(&mut variables, variable);
                    }
                }
            }
            for variable in variables {
                let mut primitives = Vec::new();
                let mut matching = Vec::new();
                for &ty in &flat {
                    if self.has_type_flags(ty, TypeFlags::INTERSECTION)
                        && self.object_flags[ty.0]
                            .intersects(ObjectFlags::IS_CONSTRAINED_TYPE_VARIABLE)
                    {
                        let types = self
                            .intersection_constituents(ty)
                            .expect("intersection checked");
                        let index =
                            usize::from(!self.has_type_flags(types[0], TypeFlags::TYPE_VARIABLE));
                        if types[index] == variable {
                            self.insert_union_type(&mut primitives, types[1 - index])?;
                            self.push_type(&mut matching, ty);
                        }
                    }
                }
                let Some(&constraint) = constraints.get(&variable) else {
                    return Err(UnionFailure::Constraint(variable));
                };
                let covered = self
                    .union_constituents(constraint)
                    .map(|types| types.iter().all(|&ty| self.contains_type(&primitives, ty)))
                    .unwrap_or_else(|| self.contains_type(&primitives, constraint));
                if covered {
                    flat.retain(|&ty| !self.contains_type(&matching, ty));
                    self.insert_union_type(&mut flat, variable)?;
                }
            }
        }
        if flat.is_empty() {
            if reduction != UnionReduction::None && had_null && !non_widening_nullable {
                return Ok(self.null_initializer());
            }
            if reduction != UnionReduction::None && had_undefined && !non_widening_nullable {
                return Ok(self.undefined_initializer());
            }
            return Ok(
                self.special(if reduction != UnionReduction::None && had_null {
                    SpecialName::Null
                } else if reduction != UnionReduction::None && had_undefined {
                    SpecialName::Undefined
                } else {
                    SpecialName::Never
                }),
            );
        }
        if reduction == UnionReduction::Subtype && flat.len() > 1 {
            match subtype_reductions.get(&flat) {
                Some(reduced) => flat = self.copy_type_list(reduced),
                None => {
                    return Err(UnionFailure::SubtypeReduction {
                        types: flat,
                        has_object_types,
                    });
                }
            }
        }
        // `addNamedUnions` walks original input order and descends only
        // through union origins; an ordinary unnamed union contributes none.
        if origin.is_none() && includes_named_union {
            let mut named = Vec::new();
            let mut pending = Vec::new();
            for &ty in types.iter().rev() {
                self.push_type(&mut pending, ty);
            }
            while let Some(id) = pending.pop() {
                if let Type::Union { alias, origin, .. } = &self.types[id.0] {
                    if alias.is_some()
                        || origin
                            .is_some_and(|origin| !self.has_type_flags(origin, TypeFlags::UNION))
                    {
                        if !self.contains_type(&named, id) {
                            self.push_type(&mut named, id);
                        }
                    } else if let Some(origin) = origin {
                        let Type::Union { alternatives, .. } = &self.types[origin.0] else {
                            unreachable!("nonunion origin handled above")
                        };
                        for &ty in alternatives.iter().rev() {
                            self.push_type(&mut pending, ty);
                        }
                    }
                }
            }
            let mut rest = Vec::new();
            for &ty in &flat {
                if !named.iter().any(|&id| {
                    self.contains_type(&self.union_constituents(id).expect("named union"), ty)
                }) {
                    self.push_type(&mut rest, ty);
                }
            }
            if alias.is_none() && named.len() == 1 && rest.is_empty() {
                return Ok(named[0]);
            }
            let named_count: usize = named
                .iter()
                .map(|id| self.union_constituents(*id).expect("named union").len())
                .sum();
            if named_count + rest.len() == flat.len() {
                for ty in named {
                    self.insert_union_type(&mut rest, ty)?;
                }
                origin = Some(self.new_union_origin(rest));
            }
        }
        Ok(self.union_sorted_ex(flat, alias, origin, precomputed_flags))
    }

    /// `addTypesToUnion` skips adjacent identity duplicates within each
    /// input list before any recursive traversal or comparison question.
    fn push_union_inputs(&self, pending: &mut Vec<TypeId>, types: &[TypeId]) {
        for index in (0..types.len()).rev() {
            if index == 0 || types[index] != types[index - 1] {
                self.push_type(pending, types[index]);
            }
        }
    }

    /// `insertType` / `addTypeToUnion`: binary insertion asks the shared
    /// comparator in native search order; identity is its final tie breaker.
    fn insert_union_type(&self, types: &mut Vec<TypeId>, ty: TypeId) -> Result<(), UnionFailure> {
        let mut low = 0;
        let mut high = types.len();
        while low < high {
            let middle = low + (high - low) / 2;
            if self.compare_types(types[middle], ty)? == Ordering::Less {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        if low == types.len() || self.compare_types(types[low], ty)? != Ordering::Equal {
            // Binary search has already observed every semantic comparator.
            // The remaining move is plain TypeId storage.
            types.insert(low, ty);
        }
        Ok(())
    }

    /// CompareTypes owns canonical order from established native facts;
    /// comparison never asks for members or reconstructs display symbols.
    fn compare_types(&self, left: TypeId, right: TypeId) -> Result<Ordering, UnionFailure> {
        let _depth = self.resource_depth();
        if left == right {
            return Ok(Ordering::Equal);
        }
        let flags = self.flags(left).0.cmp(&self.flags(right).0);
        if flags != Ordering::Equal {
            return Ok(flags);
        }
        let names = self.compare_type_names(left, right)?;
        if names != Ordering::Equal {
            return Ok(names);
        }
        let order = match (&self.types[left.0], &self.types[right.0]) {
            (
                Type::Object { .. }
                | Type::LazyObject { .. }
                | Type::Reference { .. }
                | Type::Target(_),
                _,
            ) => {
                let symbols = self.compare_native_symbols(
                    self.symbols[left.0].native()?,
                    self.symbols[right.0].native()?,
                )?;
                if symbols != Ordering::Equal {
                    return Ok(symbols);
                }
                let left_flags = self.object_flags(left)?;
                let right_flags = self.object_flags(right)?;
                match (
                    left_flags.contains(ObjectFlags::REFERENCE),
                    right_flags.contains(ObjectFlags::REFERENCE),
                ) {
                    (true, true) => {
                        let TypeView::Reference(a) = self.get(left)? else {
                            unreachable!("reference factory flags")
                        };
                        let TypeView::Reference(b) = self.get(right)? else {
                            unreachable!("reference factory flags")
                        };
                        if let (Some(a), Some(b)) = (&a.tuple, &b.tuple) {
                            let order = tuples::compare_tuple_targets(a, b)?;
                            if order != Ordering::Equal {
                                return Ok(order);
                            }
                        }
                        match (
                            self.deferred_reference(left),
                            self.deferred_reference(right),
                        ) {
                            (None, None) => self.compare_type_lists(
                                &self
                                    .resolved_type_arguments(left)
                                    .expect("ordinary reference arguments")?,
                                &self
                                    .resolved_type_arguments(right)
                                    .expect("ordinary reference arguments")?,
                            )?,
                            (a, b) => {
                                let order = NativeDeclaration::compare(
                                    a.as_ref().map(|reference| &reference.node),
                                    b.as_ref().map(|reference| &reference.node),
                                );
                                if order != Ordering::Equal {
                                    return Ok(order);
                                }
                                self.compare_mappers(
                                    a.and_then(|reference| reference.mapper),
                                    b.and_then(|reference| reference.mapper),
                                )?
                            }
                        }
                    }
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    (false, false) => {
                        // Native ObjectTypeKindMask excludes Instantiated/ObjectLiteral.
                        let mask = 63 | (1 << 8) | (1 << 10) | (1 << 24) | (1 << 25);
                        let kind = (left_flags.0 & mask).cmp(&(right_flags.0 & mask));
                        if kind != Ordering::Equal {
                            return Ok(kind);
                        }
                        self.compare_mappers(self.type_mappers[left.0], self.type_mappers[right.0])?
                    }
                }
            }
            (
                Type::Union {
                    alternatives: a,
                    origin: ao,
                    boolean,
                    ..
                },
                Type::Union {
                    alternatives: b,
                    origin: bo,
                    ..
                },
            ) => {
                // Boolean carries native Boolean flags and takes the intrinsic
                // branch before the Union branch in CompareTypes.
                if *boolean {
                    Ordering::Equal
                } else {
                    match (ao, bo) {
                        (None, None) => self.compare_type_lists(a, b)?,
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (Some(a), Some(b)) => self.compare_types(*a, *b)?,
                    }
                }
            }
            (
                Type::Intersection {
                    constituents: a, ..
                },
                Type::Intersection {
                    constituents: b, ..
                },
            ) => self.compare_type_lists(a, b)?,
            (Type::Enum { .. }, _) => self.compare_native_symbols(
                self.symbols[left.0].native()?,
                self.symbols[right.0].native()?,
            )?,
            (Type::Literal { enum_: true, .. }, _) => self.compare_native_symbols(
                self.symbols[left.0].native()?,
                self.symbols[right.0].native()?,
            )?,
            (
                Type::Literal {
                    base: PrimitiveName::String | PrimitiveName::Boolean,
                    value: a,
                    ..
                },
                Type::Literal { value: b, .. },
            ) => a.compare_value(b)?,
            (
                Type::Literal {
                    base: PrimitiveName::Number,
                    value: a,
                    ..
                },
                Type::Literal { value: b, .. },
            ) => {
                let a = a
                    .scalar_text()
                    .expect("number literal factories retain canonical scalar text")
                    .parse::<f64>()
                    .map_err(|_| UnionFailure::UnsupportedComparison)?;
                let b = b
                    .scalar_text()
                    .expect("number literal factories retain canonical scalar text")
                    .parse::<f64>()
                    .map_err(|_| UnionFailure::UnsupportedComparison)?;
                // Go cmp.Compare orders NaN first and considers two NaNs equal.
                a.partial_cmp(&b)
                    .unwrap_or_else(|| a.is_nan().cmp(&b.is_nan()).reverse())
            }
            (Type::StringMapping(a), Type::StringMapping(b)) => {
                self.compare_types(a.target, b.target)?
            }
            (Type::Conditional { data: a, .. }, Type::Conditional { data: b, .. }) => {
                let nodes = crate::project::binding::NativeDeclaration::compare(
                    Some(&a.declaration),
                    Some(&b.declaration),
                );
                if nodes != Ordering::Equal {
                    nodes
                } else {
                    self.compare_mappers(a.mapper, b.mapper)?
                }
            }
            (Type::Substitution(a), Type::Substitution(b)) => {
                let base = self.compare_types(a.base, b.base)?;
                if base != Ordering::Equal {
                    base
                } else {
                    self.compare_types(a.constraint, b.constraint)?
                }
            }
            (Type::TemplateLiteral(a), Type::TemplateLiteral(b)) => {
                let mut texts = Ordering::Equal;
                for (a, b) in a.texts.iter().zip(b.texts.iter()) {
                    texts = a.compare_value(b)?;
                    if texts != Ordering::Equal {
                        break;
                    }
                }
                if texts == Ordering::Equal {
                    texts = a.texts.len().cmp(&b.texts.len());
                }
                if texts != Ordering::Equal {
                    texts
                } else {
                    self.compare_type_lists(&a.types, &b.types)?
                }
            }
            (Type::Index(a), Type::Index(b)) => {
                let target = self.compare_types(a.target, b.target)?;
                if target != Ordering::Equal {
                    target
                } else {
                    a.index_flags.cmp(&b.index_flags)
                }
            }
            (Type::IndexedAccess { data: a, .. }, Type::IndexedAccess { data: b, .. }) => {
                let object = self.compare_types(a.object_type, b.object_type)?;
                if object != Ordering::Equal {
                    object
                } else {
                    self.compare_types(a.index_type, b.index_type)?
                }
            }
            (Type::Parameter(_) | Type::ParameterClone { .. } | Type::UniqueSymbol(_), _) => self
                .compare_native_symbols(
                self.symbols[left.0].native()?,
                self.symbols[right.0].native()?,
            )?,
            // BigInt literals deliberately have no value branch in native
            // CompareTypes. Equal established facts fall through to type ID.
            _ => Ordering::Equal,
        };
        Ok(if order == Ordering::Equal {
            left.cmp(&right)
        } else {
            order
        })
    }

    /// getTypeNameSymbol / compareTypeNames distinguish an alias name from
    /// the actual object's symbol used later by compareSymbols.
    fn comparison_name(&self, ty: TypeId) -> Result<Option<&NativeSymbol>, UnionFailure> {
        if let Some(alias) = self.alias(ty) {
            return Ok(alias.native_symbol.native()?);
        }
        if self.has_type_flags(ty, TypeFlags::TYPE_PARAMETER | TypeFlags::STRING_MAPPING)
            || self.object_flags[ty.0]
                .intersects(ObjectFlags::CLASS | ObjectFlags::INTERFACE | ObjectFlags::REFERENCE)
        {
            return Ok(self.symbols[ty.0].native()?);
        }
        Ok(None)
    }

    fn alias(&self, ty: TypeId) -> Option<&Alias> {
        if let Some(alias) = self.intrinsic_aliases.get(&ty) {
            return Some(alias);
        }
        match &self.types[ty.0] {
            Type::Union { alias, .. }
            | Type::Intersection { alias, .. }
            | Type::IndexedAccess { alias, .. }
            | Type::Reference { alias, .. } => alias.as_deref(),
            Type::Object { metadata, .. } | Type::LazyObject { metadata } => {
                metadata.alias.as_deref()
            }
            _ => None,
        }
    }

    fn compare_type_names(&self, left: TypeId, right: TypeId) -> Result<Ordering, UnionFailure> {
        match (self.comparison_name(left)?, self.comparison_name(right)?) {
            (None, None) => Ok(Ordering::Equal),
            (None, Some(_)) => Ok(Ordering::Greater),
            (Some(_), None) => Ok(Ordering::Less),
            (Some(a), Some(b)) if a.same_symbol(b) => {
                if let Some(alias) = self.alias(left) {
                    let other = self.alias(right).ok_or_else(|| {
                        UnionFailure::Demand(
                            super::internal("equal alias symbol has no corresponding native alias")
                                .into(),
                        )
                    })?;
                    self.compare_type_lists(
                        alias.arguments.as_deref().unwrap_or(&[]),
                        other.arguments.as_deref().unwrap_or(&[]),
                    )
                } else {
                    Ok(Ordering::Equal)
                }
            }
            (Some(a), Some(b)) => {
                Ok(self
                    .symbol_order
                    .compare_names(a, b, self.control.as_ref())?)
            }
        }
    }

    /// compareTypeLists tests length before visiting any element.
    fn compare_type_lists(
        &self,
        left: &[TypeId],
        right: &[TypeId],
    ) -> Result<Ordering, UnionFailure> {
        if left.len() != right.len() {
            return Ok(left.len().cmp(&right.len()));
        }
        for (&a, &b) in left.iter().zip(right) {
            let order = self.compare_types(a, b)?;
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        Ok(Ordering::Equal)
    }

    /// compareTypeMappers observes raw mappings; it never evaluates a
    /// substitution or calls a dynamic mapper. Composite has KindUnknown.
    fn compare_mappers(
        &self,
        left: Option<MapperId>,
        right: Option<MapperId>,
    ) -> Result<Ordering, UnionFailure> {
        let _depth = self.resource_depth();
        if left == right {
            return Ok(Ordering::Equal);
        }
        let (left, right) = match (left, right) {
            (None, Some(_)) => return Ok(Ordering::Greater),
            (Some(_), None) => return Ok(Ordering::Less),
            (Some(a), Some(b)) => (self.mapper_data(a), self.mapper_data(b)),
            (None, None) => unreachable!(),
        };
        let kind = |mapper: &Mapper| match mapper {
            Mapper::Simple { sources, .. } => {
                if sources.len() == 1 {
                    1
                } else {
                    2
                }
            }
            Mapper::Merged { .. } => 3,
            Mapper::Composite { .. }
            | Mapper::Inference { .. }
            | Mapper::ArrayToSingle { .. }
            | Mapper::ReportReliability { .. }
            | Mapper::Permissive
            | Mapper::Restrictive
            | Mapper::Deferred { .. }
            | Mapper::UniqueLiteral => 0,
        };
        let order = kind(left).cmp(&kind(right));
        if order != Ordering::Equal {
            return Ok(order);
        }
        match (left, right) {
            (
                Mapper::Simple {
                    sources: a,
                    targets: at,
                },
                Mapper::Simple {
                    sources: b,
                    targets: bt,
                },
            ) => {
                let sources = self.compare_type_lists(a, b)?;
                if sources != Ordering::Equal {
                    return Ok(sources);
                }
                for (&a, &b) in at.iter().zip(bt) {
                    let order = self.compare_mapper_targets(a, b)?;
                    if order != Ordering::Equal {
                        return Ok(order);
                    }
                }
                Ok(Ordering::Equal)
            }
            (
                Mapper::Merged {
                    first: a,
                    second: aa,
                },
                Mapper::Merged {
                    first: b,
                    second: bb,
                },
            ) => {
                let first = self.compare_mappers(Some(*a), Some(*b))?;
                if first != Ordering::Equal {
                    return Ok(first);
                }
                self.compare_mappers(Some(*aa), Some(*bb))
            }
            _ => Ok(Ordering::Equal),
        }
    }

    fn compare_mapper_targets(
        &self,
        left: MapperTarget,
        right: MapperTarget,
    ) -> Result<Ordering, UnionFailure> {
        match (left, right) {
            (MapperTarget::Known(a), MapperTarget::Known(b)) => self.compare_types(a, b),
            (MapperTarget::ErrorType, MapperTarget::ErrorType) => Ok(Ordering::Equal),
            (MapperTarget::ErrorType, MapperTarget::Known(ty)) => {
                Ok(self.compare_recovery_to_type(ty))
            }
            (MapperTarget::Known(ty), MapperTarget::ErrorType) => {
                Ok(self.compare_recovery_to_type(ty).reverse())
            }
        }
    }

    /// errorType has Any flags and follows anyType/autoType during native
    /// initializeTypeCaches. This identity-only observation does not consume
    /// recovery as a substituted value; map_parameter still preserves failure.
    fn compare_recovery_to_type(&self, ty: TypeId) -> Ordering {
        let flags = TypeFlags::ANY.0.cmp(&self.flags(ty).0);
        if flags != Ordering::Equal {
            return flags;
        }
        match self.types[ty.0] {
            Type::Special(SpecialName::Any) | Type::AutomaticAny | Type::NonInferrableAny => {
                Ordering::Greater
            }
            Type::Wildcard | Type::IntrinsicMarker => Ordering::Less,
            _ => unreachable!("unregistered native Any intrinsic ordering"),
        }
    }

    /// The native flag owner for every admitted established type. `CompareTypes`
    /// uses these flags before literal/name ordering; evaluator read decisions
    /// consume the same flags rather than reconstructing them from display.
    fn flags(&self, id: TypeId) -> TypeFlags {
        match self.types[id.0] {
            Type::UniqueSymbol(_) => TypeFlags::UNIQUE_SYMBOL,
            Type::Special(name) => match name {
                SpecialName::Any => TypeFlags::ANY,
                SpecialName::Unknown => TypeFlags::UNKNOWN,
                SpecialName::Undefined => TypeFlags::UNDEFINED,
                SpecialName::Null => TypeFlags::NULL,
                SpecialName::Void => TypeFlags::VOID,
                SpecialName::Never => TypeFlags::NEVER,
            },
            Type::AutomaticAny | Type::NonInferrableAny | Type::Wildcard => TypeFlags::ANY,
            Type::WideningNull => TypeFlags::NULL,
            Type::WideningUndefined => TypeFlags::UNDEFINED,
            Type::Missing | Type::OptionalMarker => TypeFlags::UNDEFINED,
            Type::ImplicitNever
            | Type::UnreachableNever
            | Type::SilentNever
            | Type::UniqueLiteral => TypeFlags::NEVER,
            Type::Parameter(_) | Type::ParameterClone { .. } => TypeFlags::TYPE_PARAMETER,
            Type::Primitive(name) => match name {
                PrimitiveName::String => TypeFlags::STRING,
                PrimitiveName::Number => TypeFlags::NUMBER,
                PrimitiveName::Bigint => TypeFlags::BIGINT,
                PrimitiveName::Boolean => TypeFlags::BOOLEAN,
                PrimitiveName::Symbol => TypeFlags::SYMBOL,
                PrimitiveName::Object => TypeFlags::NON_PRIMITIVE,
            },
            Type::Enum { .. } => TypeFlags::ENUM,
            Type::Literal { base, enum_, .. } => {
                (match base {
                    PrimitiveName::String => TypeFlags::STRING_LITERAL,
                    PrimitiveName::Number => TypeFlags::NUMBER_LITERAL,
                    PrimitiveName::Bigint => TypeFlags::BIGINT_LITERAL,
                    PrimitiveName::Boolean => TypeFlags::BOOLEAN_LITERAL,
                    PrimitiveName::Symbol | PrimitiveName::Object => {
                        unreachable!("nonliteral base has no literal construction")
                    }
                }) | if enum_ {
                    TypeFlags::ENUM_LITERAL
                } else {
                    TypeFlags(0)
                }
            }
            Type::Object { .. }
            | Type::LazyObject { .. }
            | Type::Target(_)
            | Type::Reference { .. } => TypeFlags::OBJECT,
            Type::Intersection { .. } => TypeFlags::INTERSECTION,
            Type::IndexedAccess { .. } => TypeFlags::INDEXED_ACCESS,
            Type::Conditional { .. } => TypeFlags::CONDITIONAL,
            Type::Substitution(_) => TypeFlags::SUBSTITUTION,
            Type::Index(_) => TypeFlags::INDEX,
            Type::TemplateLiteral(_) => TypeFlags::TEMPLATE_LITERAL,
            Type::StringMapping(_) => TypeFlags::STRING_MAPPING,
            Type::IntrinsicMarker => TypeFlags::ANY,
            Type::Union { boolean, enum_, .. } => {
                (if enum_ {
                    TypeFlags::ENUM_LITERAL
                } else {
                    TypeFlags(0)
                }) | TypeFlags::UNION
                    | if boolean {
                        TypeFlags::BOOLEAN
                    } else {
                        TypeFlags(0)
                    }
            }
        }
    }

    /// Native `someType(t => t.flags & flags)`: visit only union alternatives,
    /// otherwise inspect the scalar itself. Intersections are never expanded.
    pub(super) fn some_type_of_kind(&self, ty: TypeId, flags: TypeFlags) -> bool {
        match self.union_constituents(ty) {
            Some(types) => types.iter().any(|&ty| self.has_type_flags(ty, flags)),
            None => self.has_type_flags(ty, flags),
        }
    }

    /// `isLiteralType`: Boolean and enum-literal unions have native early
    /// returns; other unions require every actual constituent to be Unit.
    pub(super) fn is_literal_type(&self, ty: TypeId) -> bool {
        if self.has_type_flags(ty, TypeFlags::BOOLEAN) {
            return true;
        }
        if self.has_type_flags(ty, TypeFlags::UNION) {
            if self.has_type_flags(ty, TypeFlags::ENUM_LITERAL) {
                return true;
            }
            return self
                .union_constituents(ty)
                .expect("native union has constituents")
                .iter()
                .all(|&child| self.has_type_flags(child, TypeFlags::UNIT));
        }
        self.has_type_flags(ty, TypeFlags::UNIT)
    }

    pub(super) fn is_string_literal_value(&self, ty: TypeId, value: &str) -> bool {
        matches!(&self.types[ty.0], Type::Literal { base: PrimitiveName::String, value: actual, .. } if actual.equals_text(value))
    }

    pub(super) fn has_type_flags(&self, id: TypeId, mask: TypeFlags) -> bool {
        self.flags(id).0 & mask.0 != 0
    }

    /// Native equality with the cached `autoType`; genuine any and widening
    /// nullable identities are distinct even when their display kind agrees.
    pub(super) fn is_automatic(&self, id: TypeId) -> bool {
        self.automatic == Some(id) || self.automatic_array == Some(id)
    }

    pub(super) fn is_automatic_array(&self, id: TypeId) -> bool {
        self.automatic_array == Some(id)
    }
    pub(super) fn automatic_array(&self) -> Option<TypeId> {
        self.automatic_array
    }
    /// `initializeTypeCaches` retains createArrayType(autoType), independently
    /// from the genuine anyArrayType reference and its canonical arguments.
    pub(super) fn set_automatic_array(&mut self, ty: TypeId) {
        self.automatic_array = Some(ty);
    }

    /// The native cached `unreachableNeverType` is distinct from ordinary and
    /// implicit never, even though their flags and display kinds agree.
    pub(super) fn unreachable_never(&mut self) -> TypeId {
        if let Some(id) = self.unreachable_never {
            return id;
        }
        let id = self.push(Type::UnreachableNever);
        self.unreachable_never = Some(id);
        id
    }

    pub(super) fn is_unreachable_never(&self, id: TypeId) -> bool {
        self.unreachable_never == Some(id)
    }

    /// `newFlowType` uses the compiler's cached silentNeverType for an
    /// incomplete Never result. This identity must survive separately from
    /// ordinary, implicit and unreachable Never despite their shared view.
    pub(super) fn silent_never(&mut self) -> TypeId {
        if let Some(id) = self.silent_never {
            return id;
        }
        let id = self.push(Type::SilentNever);
        self.silent_never = Some(id);
        id
    }

    /// `Type.Types` exposes actual semantic constituents, including Boolean's
    /// false/true union whose presentation view is intentionally primitive.
    pub(super) fn union_constituents(&self, id: TypeId) -> Option<Arc<[TypeId]>> {
        match &self.types[id.0] {
            Type::Union { alternatives, .. } => Some(Arc::clone(alternatives)),
            _ => None,
        }
    }

    /// `mapTypeEx` observes a denormalized union origin before its flattened
    /// constituents. This is semantic traversal state, not summary metadata.
    pub(super) fn union_mapping_constituents(&self, id: TypeId) -> Option<Arc<[TypeId]>> {
        match &self.types[id.0] {
            Type::Union {
                alternatives,
                origin,
                ..
            } => Some(
                origin
                    .and_then(|origin| self.union_constituents(origin))
                    .unwrap_or_else(|| alternatives.clone()),
            ),
            _ => None,
        }
    }

    pub(super) fn intersection_constituents(&self, id: TypeId) -> Option<Arc<[TypeId]>> {
        match &self.types[id.0] {
            Type::Intersection { constituents, .. } => Some(constituents.clone()),
            _ => None,
        }
    }

    pub(super) fn type_flags(&self, id: TypeId) -> TypeFlags {
        self.flags(id)
    }

    /// Native `getObjectFlags` observes a factory header without forcing its
    /// member inventory. A poisoned target remains its original failure.
    pub(super) fn object_flags(&self, id: TypeId) -> Result<ObjectFlags, DemandError> {
        match &self.types[id.0] {
            Type::Target(state) => {
                if let TargetState::Failed(error) = state.as_ref() {
                    return Err(error.clone());
                }
            }
            Type::Reference { target, .. } => {
                self.object_flags(*target)?;
            }
            _ => {}
        }
        Ok(self.object_flags[id.0])
    }

    /// Native factories may add propagating/object-kind flags after dependent
    /// fields have been acquired; the actual TypeId remains unchanged.
    pub(super) fn add_object_flags(
        &mut self,
        id: TypeId,
        flags: ObjectFlags,
    ) -> Result<(), DemandError> {
        self.object_flags[id.0] = self.object_flags(id)? | flags;
        Ok(())
    }

    /// `setStructuredTypeMembers` installs this fact after the inventory.
    pub(super) fn mark_members_resolved(&mut self, id: TypeId) -> Result<(), DemandError> {
        let flags = self.object_flags(id)?;
        self.object_flags[id.0] = flags | ObjectFlags::MEMBERS_RESOLVED;
        Ok(())
    }

    /// `getBaseTypes` invalidates any member completion reached while the
    /// target's bases were being resolved; its inventory can be acquired anew.
    pub(super) fn clear_members_resolved(&mut self, id: TypeId) -> Result<(), DemandError> {
        let flags = self.object_flags(id)?;
        self.object_flags[id.0] = ObjectFlags(flags.0 & !ObjectFlags::MEMBERS_RESOLVED.0);
        Ok(())
    }

    pub(super) fn set_unresolved_members(
        &mut self,
        id: TypeId,
        unresolved: bool,
    ) -> Result<(), DemandError> {
        let flags = self.object_flags(id)?;
        self.object_flags[id.0] = if unresolved {
            flags | ObjectFlags::UNRESOLVED_MEMBERS
        } else {
            ObjectFlags(flags.0 & !ObjectFlags::UNRESOLVED_MEMBERS.0)
        };
        Ok(())
    }

    pub(super) fn mark_identical_base_calculated(&mut self, id: TypeId) -> Result<(), DemandError> {
        self.object_flags[id.0] =
            self.object_flags(id)? | ObjectFlags::IDENTICAL_BASE_TYPE_CALCULATED;
        Ok(())
    }

    /// `couldContainTypeVariables` records its native computed/result bits.
    pub(super) fn mark_could_contain_type_variables(
        &mut self,
        id: TypeId,
        contains: bool,
    ) -> Result<(), DemandError> {
        let flags = self.object_flags(id)?;
        self.object_flags[id.0] = flags
            | ObjectFlags::COULD_CONTAIN_TYPE_VARIABLES_COMPUTED
            | if contains {
                ObjectFlags::COULD_CONTAIN_TYPE_VARIABLES
            } else {
                ObjectFlags::NONE
            };
        Ok(())
    }

    /// `getPropagatingFlagsOfTypes` reads only native object flags, omitting
    /// the exact excluded kinds before collecting PropagatingFlags.
    pub(super) fn propagating_flags(&self, types: &[TypeId], exclude: TypeFlags) -> ObjectFlags {
        types
            .iter()
            .filter(|&&ty| !self.has_type_flags(ty, exclude))
            .fold(ObjectFlags::NONE, |flags, ty| {
                flags | (self.object_flags[ty.0] & ObjectFlags::PROPAGATING_FLAGS)
            })
    }

    /// `getIntersectionKey`: ordered type IDs and either actual alias or `*`.
    pub(super) fn intersection_key(
        &self,
        types: &[TypeId],
        flags: IntersectionFlags,
        alias: Option<&Alias>,
    ) -> IntersectionKey {
        IntersectionKey::new(self.share_type_list(types), flags, alias)
    }
    pub(super) fn cached_intersection(
        &self,
        key: &IntersectionKey,
    ) -> Option<NativeCompletion<TypeId>> {
        self.intersections.get(key).cloned()
    }
    pub(super) fn cache_intersection(
        &mut self,
        key: IntersectionKey,
        ty: NativeCompletion<TypeId>,
    ) {
        self.intersections.insert(key, ty);
    }

    /// `newIntersectionType` allocates the native ordered structure. Only the
    /// complete evaluator factory decides normalization, flags and cache use.
    pub(super) fn new_intersection(
        &mut self,
        types: Vec<TypeId>,
        flags: ObjectFlags,
        alias: Option<Alias>,
    ) -> TypeId {
        let constituents = std::sync::Arc::from(types);

        self.push_with_flags(
            Type::Intersection {
                constituents,
                alias: alias.map(Arc::new),
            },
            flags,
        )
    }

    pub(super) fn is_missing_type(&self, ty: TypeId) -> bool {
        self.missing == Some(ty)
    }

    pub(super) fn is_silent_never(&self, ty: TypeId) -> bool {
        self.silent_never == Some(ty)
    }
    pub(super) fn is_empty_type_literal(&self, ty: TypeId) -> bool {
        self.empty_type_literal == Some(ty)
    }

    /// `maybeTypeOfKind`: test the actual flag first, then whether some
    /// union or intersection constituent has it. Type parameters do not imply their constraint
    /// here; native asks that as a separate operation.
    pub(super) fn maybe_type_of_kind(&self, ty: TypeId, flags: TypeFlags) -> bool {
        // The invocation owns its check, including flag shortcuts. Only
        // visited children recurse; flat width does not spend nested depth.
        let _depth = self.resource_depth();
        self.has_type_flags(ty, flags)
            || self
                .union_constituents(ty)
                .or_else(|| self.intersection_constituents(ty))
                .is_some_and(|types| types.iter().any(|&ty| self.maybe_type_of_kind(ty, flags)))
    }

    /// `extractTypesOfKind` filters actual union constituents through the
    /// shared `filterType` factory, preserving its identity and origin policy.
    pub(super) fn extract_types_of_kind(&mut self, ty: TypeId, flags: TypeFlags) -> TypeId {
        self.filter_type(ty, |model, child| model.has_type_flags(child, flags))
    }

    pub(super) fn begin_type_filter(&self, ty: TypeId) -> TypeFilter {
        let depth = self.resource_depth();
        let types = self.union_constituents(ty).unwrap_or_else(|| {
            if self.has_type_flags(ty, TypeFlags::NEVER) {
                Arc::from([])
            } else {
                self.share_type_list(&[ty])
            }
        });
        let origin = self
            .union_origin(ty)
            .and_then(|origin| self.union_constituents(origin));
        TypeFilter {
            original: ty,
            types,
            origin,
            cursor: 0,
            retained: Vec::new(),
            origin_filtered: None,

            _depth: depth,
        }
    }

    /// Pure predicates use the same `filterType` traversal as effectful ones.
    pub(super) fn filter_type(
        &mut self,
        ty: TypeId,
        mut predicate: impl FnMut(&Self, TypeId) -> bool,
    ) -> TypeId {
        let mut filter = self.begin_type_filter(ty);
        while let Some(part) = filter.next(self) {
            let retain = predicate(self, part);
            filter.accept(part, retain);
        }
        filter.finish(self)
    }

    /// `filterType` reconstruction after its actual predicate acquisitions.
    /// The shared traversal supplies its independently filtered origin.
    fn filtered_type_with_origin(
        &mut self,
        id: TypeId,
        retained: Vec<TypeId>,
        origin_filtered: Option<Vec<TypeId>>,
    ) -> TypeId {
        if self.has_type_flags(id, TypeFlags::NEVER) {
            return id;
        }
        if let Type::Union {
            alternatives,
            origin,
            ..
        } = &self.types[id.0]
        {
            // Both controller callers retain only the exact next() identity,
            // in order. Equal lengths therefore prove the same subset without
            // repeating an unbounded element-by-element equality scan.
            if alternatives.len() == retained.len() {
                return id;
            }
            let removed = alternatives.len() - retained.len();
            let origin_count = origin
                .and_then(|origin| self.union_constituents(origin))
                .map(|types| types.len());
            let mut new_origin = None;
            if let (Some(count), Some(filtered)) = (origin_count, origin_filtered) {
                if count - filtered.len() == removed {
                    if filtered.len() == 1 {
                        return filtered[0];
                    }
                    new_origin = Some(self.new_union_origin(filtered));
                }
            }
            let flags = self.object_flags[id.0]
                & (ObjectFlags::PRIMITIVE_UNION | ObjectFlags::CONTAINS_INTERSECTIONS);
            return self.union_sorted_ex(retained, None, new_origin, flags);
        }
        if retained.is_empty() {
            self.special(SpecialName::Never)
        } else {
            id
        }
    }

    /// `newUnionType(ObjectFlagsNone, types)` for a denormalized origin is
    /// a real uninterned native identity without normalized factory flags.
    fn new_union_origin(&mut self, types: Vec<TypeId>) -> TypeId {
        let alternatives = self.share_type_list(&types);
        let id = self.push(Type::Union {
            alternatives,
            boolean: false,
            enum_: false,
            alias: None,
            origin: None,
        });
        self.object_flags[id.0] = ObjectFlags::NONE;
        id
    }

    /// `initializeTypeCaches`' canonical emptyObjectType. This healthy
    /// intrinsic is distinct from a fresh source object and global recovery.
    pub(super) fn empty_object_type(&mut self) -> TypeId {
        if let Some(id) = self.empty_object {
            return id;
        }
        let id = self.structured(
            TypeSymbol::Absent,
            MembersView {
                properties: Arc::from([]),
                call_signatures: Arc::from([]),
                construct_signatures: Arc::from([]),
                index_signatures: Arc::from([]),
            },
            Metadata::default(),
            ObjectKind::Anonymous,
        );
        self.empty_object = Some(id);
        id
    }

    /// `initializeTypeCaches` allocates a distinct emptyGenericType sentinel.
    /// Its empty members are real; consuming its global-recovery provenance
    /// remains the global factory's question, independent of identity tests.
    pub(super) fn empty_generic_type(&mut self) -> TypeId {
        if let Some(id) = self.empty_generic {
            return id;
        }
        let id = self.structured(
            TypeSymbol::Absent,
            own_properties(Vec::new()),
            Metadata::default(),
            ObjectKind::Anonymous,
        );
        self.empty_generic = Some(id);
        id
    }

    /// `initializeTypeCaches` / `getTypeFromTypeLiteralOrFunctionOrConstructorTypeNode`:
    /// the proven empty, unaliased type literal uses its own intrinsic identity,
    /// separate from the apparent-type emptyObjectType.
    pub(super) fn empty_type_literal_type(&mut self) -> TypeId {
        if let Some(id) = self.empty_type_literal {
            return id;
        }
        let id = self.structured(
            TypeSymbol::Synthetic {
                native: NativeSymbol::empty_type_literal(),
                flags: TypeSymbolFlags {
                    type_literal: true,
                    ..TypeSymbolFlags::default()
                },
            },
            MembersView {
                properties: Arc::from([]),
                call_signatures: Arc::from([]),
                construct_signatures: Arc::from([]),
                index_signatures: Arc::from([]),
            },
            Metadata::default(),
            ObjectKind::Anonymous,
        );
        self.empty_type_literal = Some(id);
        id
    }

    pub(super) fn unknown_empty_object_type(&self) -> TypeId {
        self.unknown_empty_object
            .expect("native unknown-empty intrinsic initialized")
    }

    /// createUnknownUnionType uses its distinct unknownEmptyObjectType in
    /// strict mode and the actual unknown intrinsic in loose mode.
    pub(super) fn unknown_union(&mut self) -> TypeId {
        if let Some(id) = self.unknown_union {
            return id;
        }
        if !self.strict_null_checks {
            return self.special(SpecialName::Unknown);
        }
        let empty = self
            .unknown_empty_object
            .expect("native unknown-empty intrinsic initialized");
        let undefined = self.special(SpecialName::Undefined);
        let null = self.special(SpecialName::Null);
        let id = self.union_sorted(vec![undefined, null, empty]);
        self.unknown_union = Some(id);
        id
    }

    /// `recombineUnknownType`: only the exact cached intrinsic union is
    /// normalized back to unknown; an equal printed union is not sufficient.
    pub(super) fn recombine_unknown(&mut self, id: TypeId) -> TypeId {
        if self.unknown_union == Some(id) {
            self.special(SpecialName::Unknown)
        } else {
            id
        }
    }

    /// Native `ObjectFlagsAnonymous` is a factory fact, independent from the
    /// displayed name or whether deferred member resolution has completed.
    pub(super) fn is_anonymous(&self, id: TypeId) -> Result<bool, DemandError> {
        Ok(self.object_flags(id)?.intersects(ObjectFlags::ANONYMOUS))
    }

    /// `getUnionTypeFromSortedList` caches semantic constituent identities.
    /// Mapping fresh to regular literals preserves their established ordering
    /// and removes repeated identities without touching alias provenance.
    fn union_sorted(&mut self, alternatives: Vec<TypeId>) -> TypeId {
        let flags = self.union_object_flags(&alternatives);
        self.union_sorted_ex(alternatives, None, None, flags)
    }

    /// `getUnionKey`: union/intersection origins key their member identities;
    /// an Index origin keys both its own identity and the current alternatives.
    /// A resolving `keyof` can expose more than one intermediate key set.
    fn union_sorted_ex(
        &mut self,
        mut alternatives: Vec<TypeId>,
        alias: Option<Alias>,
        origin: Option<TypeId>,
        precomputed_flags: ObjectFlags,
    ) -> TypeId {
        alternatives.dedup();
        if alternatives.is_empty() {
            return self.special(SpecialName::Never);
        }
        if alternatives.len() == 1 {
            return alternatives[0];
        }
        let identity = match origin.map(|id| &self.types[id.0]) {
            Some(Type::Union { alternatives, .. }) => {
                UnionIdentity::UnionOrigin(alternatives.clone())
            }
            Some(Type::Intersection { constituents, .. }) => {
                UnionIdentity::IntersectionOrigin(constituents.clone())
            }
            Some(Type::Index(_)) => UnionIdentity::IndexOrigin {
                origin: origin.expect("present Index origin"),
                types: self.share_type_list(&alternatives),
            },
            None => UnionIdentity::Types(self.share_type_list(&alternatives)),
            Some(_) => unreachable!("getUnionKey requires a union, intersection or Index origin"),
        };
        let key = UnionKey::new(identity, alias.as_ref());
        if let Some(&id) = self.unions.get(&key) {
            return id;
        }
        let boolean = alternatives.len() == 2
            && alternatives.iter().all(|&id| {
                matches!(
                    self.types[id.0],
                    Type::Literal {
                        base: PrimitiveName::Boolean,
                        ..
                    }
                )
            });
        let alternatives = match &key.identity {
            // Reuse the key payload; the semantic cache and
            // the type share immutable IDs, with no additional native identity.
            UnionIdentity::Types(types) | UnionIdentity::IndexOrigin { types, .. } => types.clone(),
            _ => self.share_type_list(&alternatives),
        };

        let flags = precomputed_flags | self.propagating_flags(&alternatives, TypeFlags::NULLABLE);
        let id = self.push_with_flags(
            Type::Union {
                alternatives,
                boolean,
                enum_: false,
                alias: alias.map(Arc::new),
                origin,
            },
            flags,
        );
        self.unions.insert(key, id);
        id
    }

    /// `checkObjectLiteral` aggregates the already checked property values'
    /// PropagatingFlags before installing the actual literal factory flags.
    pub(super) fn object(
        &mut self,
        symbol: TypeSymbol,
        properties: Vec<Property>,
        checked_values: &[TypeId],
    ) -> Result<TypeId, DemandError> {
        // checkObjectLiteral propagates every checked initializer, including
        // overwritten properties. Its symbol table can hold deferred reads;
        // the model never acquires those semantic questions itself.
        let propagated = self.propagating_flags(checked_values, TypeFlags::NONE);
        let id = self.push_with_flags(
            Type::Object {
                members: own_properties(properties),
                metadata: MetadataView::default(),
                literal: true,
                kind: ObjectKind::Anonymous,
            },
            propagated,
        );
        self.symbols[id.0] = symbol;
        Ok(id)
    }

    /// `getUnionTypeWorker` computes PrimitiveUnion and ContainsIntersections
    /// from original flattened includes before literal/constraint reductions.
    /// Native NonPrimitive is intentionally absent from NotPrimitiveUnion.
    fn union_object_flags(&self, types: &[TypeId]) -> ObjectFlags {
        let not_primitive = TypeFlags::ANY
            | TypeFlags::UNKNOWN
            | TypeFlags::VOID
            | TypeFlags::NEVER
            | TypeFlags::OBJECT
            | TypeFlags::INTERSECTION
            | TypeFlags::INSTANTIABLE;
        let primitive = types
            .iter()
            .all(|&ty| !self.has_type_flags(ty, not_primitive));
        (if primitive {
            ObjectFlags::PRIMITIVE_UNION
        } else {
            ObjectFlags::NONE
        }) | if types
            .iter()
            .any(|&ty| self.has_type_flags(ty, TypeFlags::INTERSECTION))
        {
            ObjectFlags::CONTAINS_INTERSECTIONS
        } else {
            ObjectFlags::NONE
        }
    }

    /// `newAnonymousType` / `setStructuredTypeMembers`: establish complete
    /// compiler-ordered collections. Empty collections are caller-established
    /// facts; callable construction cannot invent missing object members.
    pub(super) fn structured(
        &mut self,
        symbol: TypeSymbol,
        members: MembersView,
        metadata: Metadata,
        kind: ObjectKind,
    ) -> TypeId {
        self.structured_with_flags(symbol, members, metadata, kind, ObjectFlags::NONE)
    }

    /// `newAnonymousType` with the additional flags installed by its actual
    /// factory (such as getOrCreateTypeFromSignature). Complete member facts
    /// and the base object flags remain owned by the structured factory.
    pub(super) fn structured_with_flags(
        &mut self,
        symbol: TypeSymbol,
        members: MembersView,
        metadata: Metadata,
        kind: ObjectKind,
        additional_flags: ObjectFlags,
    ) -> TypeId {
        let id = self.push_with_flags(
            Type::Object {
                members,
                metadata: MetadataView {
                    summary_origin: None,
                    alias: metadata.alias.map(Arc::new),
                    nominal_name: metadata.nominal_name.map(Arc::from),
                    type_arguments: None,
                    function_symbol: None,
                },
                literal: false,
                kind,
            },
            additional_flags,
        );
        self.symbols[id.0] = symbol;
        id
    }

    /// `createObjectType(ObjectFlagsAnonymous)` / `instantiateAnonymousType`
    /// allocate an actual symbol value without resolving overloads. Canonical
    /// value and instantiation caches belong to the owning evaluator operation.
    pub(super) fn anonymous_type(&mut self, symbol: TypeSymbol, metadata: Metadata) -> TypeId {
        self.anonymous_type_ex(symbol, metadata, ObjectFlags::ANONYMOUS, None)
    }

    pub(super) fn anonymous_type_ex(
        &mut self,
        symbol: TypeSymbol,
        metadata: Metadata,
        flags: ObjectFlags,
        mapper: Option<MapperId>,
    ) -> TypeId {
        let id = self.push_with_flags(
            Type::LazyObject {
                metadata: MetadataView {
                    summary_origin: None,
                    alias: metadata.alias.map(Arc::new),
                    nominal_name: metadata.nominal_name.map(Arc::from),
                    type_arguments: None,
                    function_symbol: None,
                },
            },
            flags,
        );
        self.symbols[id.0] = symbol;
        self.type_mappers[id.0] = mapper;
        id
    }

    /// `getTypeFromMappedTypeNode` allocates its native identity before
    /// acquiring an alias, then installs that alias before publishing the node.
    pub(super) fn install_lazy_object_alias(
        &mut self,
        ty: TypeId,
        alias: Option<Alias>,
    ) -> Result<(), DemandError> {
        let Type::LazyObject { metadata } = &mut self.types[ty.0] else {
            return Err(super::internal("lazy object alias installed on another family").into());
        };
        metadata.alias = alias.map(Arc::new);
        Ok(())
    }

    /// Native ObjectType.mapper is also used by mapped locals and isolated
    /// generic return-signature types; object representation does not decide it.
    pub(super) fn install_object_mapper(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
    ) -> Result<(), DemandError> {
        if !self.has_type_flags(ty, TypeFlags::OBJECT) {
            return Err(super::internal("object mapper installed on another family").into());
        }
        self.type_mappers[ty.0] = Some(mapper);
        Ok(())
    }

    /// Native `ObjectType.Mapper()` observes the installed mapper without
    /// resolving members or instantiating a parameter.
    pub(super) fn object_mapper(&self, ty: TypeId) -> Option<MapperId> {
        self.type_mappers[ty.0]
    }

    /// A completed eager construction can read its established child without
    /// acquiring evaluator authority. Deferred recipes are never resolved by
    /// the model; encountering one here is an internal ownership violation.
    pub(super) fn established_type(&self, demand: TypeDemand) -> Result<TypeId, DemandError> {
        match demand {
            TypeDemand::Known(ty) => Ok(ty),
            TypeDemand::Deferred(_) => Err(super::internal(
                "eager model lookup encountered a deferred semantic demand",
            )
            .into()),
        }
    }

    /// `newTypeMapper` and its specialized factories allocate a fresh identity.
    /// All mapper variants share one arena publication owner.
    fn push_mapper(&mut self, mapper: Mapper) -> MapperId {
        let id = MapperId(self.mappers.len());
        self.mappers.push(mapper);
        id
    }

    fn push(&mut self, ty: Type) -> TypeId {
        self.push_with_flags(ty, ObjectFlags::NONE)
    }

    /// `newType` clears inherited member-resolution and type-variable caches
    /// on every fresh identity. Eager objects then install their own completed
    /// inventory, as `newAnonymousType` / `setStructuredTypeMembers` do.
    fn push_with_flags(&mut self, ty: Type, input_flags: ObjectFlags) -> TypeId {
        let id = TypeId(self.types.len());
        let flags = match &ty {
            Type::WideningNull | Type::WideningUndefined | Type::NonInferrableAny => {
                ObjectFlags::CONTAINS_WIDENING_TYPE
            }
            Type::AutomaticAny | Type::SilentNever => ObjectFlags::NON_INFERRABLE_TYPE,
            Type::Object { literal, kind, .. } => {
                let kind = match kind {
                    ObjectKind::Anonymous => ObjectFlags::ANONYMOUS,
                };
                kind | if *literal {
                    ObjectFlags::OBJECT_LITERAL
                        | ObjectFlags::FRESH_LITERAL
                        | ObjectFlags::CONTAINS_OBJECT_OR_ARRAY_LITERAL
                } else {
                    ObjectFlags::NONE
                }
            }
            Type::LazyObject { .. } => ObjectFlags::NONE,
            Type::Target(state) => match state.as_ref() {
                TargetState::Allocating(header) => match header.kind {
                    TargetKind::Class => ObjectFlags::CLASS,
                    TargetKind::Interface => ObjectFlags::INTERFACE,
                    TargetKind::Tuple => ObjectFlags::TUPLE | ObjectFlags::REFERENCE,
                },
                _ => unreachable!("declared target allocation always starts with its header"),
            },
            Type::Reference {
                arguments,
                array_literal,
                ..
            } => {
                ObjectFlags::REFERENCE
                    | match arguments {
                        TypeArgumentsState::Installed(arguments) => {
                            self.propagating_flags(arguments.value(), TypeFlags::NONE)
                        }
                        _ => ObjectFlags::NONE,
                    }
                    | if *array_literal {
                        ObjectFlags::ARRAY_LITERAL | ObjectFlags::CONTAINS_OBJECT_OR_ARRAY_LITERAL
                    } else {
                        ObjectFlags::NONE
                    }
            }
            _ => ObjectFlags::NONE,
        };
        let flags = (flags | input_flags).without(
            ObjectFlags::COULD_CONTAIN_TYPE_VARIABLES_COMPUTED
                | ObjectFlags::COULD_CONTAIN_TYPE_VARIABLES
                | ObjectFlags::MEMBERS_RESOLVED,
        );
        let flags = if matches!(&ty, Type::Object { .. }) {
            flags | ObjectFlags::MEMBERS_RESOLVED
        } else {
            flags
        };
        let symbol = match &ty {
            Type::Reference { target, .. } | Type::ParameterClone { target, .. } => {
                self.symbols[target.0].clone()
            }
            _ => TypeSymbol::Absent,
        };
        self.symbols.push(symbol);
        self.type_mappers.push(None);
        self.types.push(ty);
        self.object_flags.push(flags);
        id
    }
}

/// The ordinary property-only factory establishes the other collections as
/// known empty once. Describing the object clones those established views,
/// including their empty collections, without allocating another shell.
fn own_properties(properties: Vec<Property>) -> MembersView {
    MembersView {
        properties: properties.into(),
        call_signatures: Arc::from([]),
        construct_signatures: Arc::from([]),
        index_signatures: Arc::from([]),
    }
}
