//! Whole native fact questions and their lazy dependencies within one checker.

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, ObjectFlags, SpecialName,
    TypeFlags, TypeId, TypeView,
};
use super::Evaluator;
use crate::references::Location;
use crate::types::PrimitiveName;

/// Native TypeFacts masks are possibilities, not runtime value predicates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct TypeFacts(u32);

impl TypeFacts {
    pub(super) const NONE: Self = Self(0);
    pub(super) const EQUAL_NULL: Self = Self(Self::EQ_NULL);
    pub(super) const EQUAL_UNDEFINED: Self = Self(Self::EQ_UNDEFINED);
    pub(super) const ALL_TYPEOF_NOT_EQUAL: Self = Self((0x7f << 8) | Self::NE_UNDEFINED);
    pub(super) const TRUTHY: Self = Self(1 << 22);
    pub(super) const FALSY: Self = Self(1 << 23);
    const ALL: u32 = (1 << 27) - 1;
    const EQ_UNDEFINED: u32 = 1 << 16;
    const EQ_NULL: u32 = 1 << 17;
    const EQ_NULLABLE: u32 = 1 << 18;
    const NE_UNDEFINED: u32 = 1 << 19;
    const NE_NULL: u32 = 1 << 20;
    const NE_NULLABLE: u32 = 1 << 21;
    const IS_UNDEFINED: u32 = 1 << 24;
    const IS_NULL: u32 = 1 << 25;
    pub(super) const IS_UNDEFINED_OR_NULL: Self = Self(Self::IS_UNDEFINED | Self::IS_NULL);
    pub(super) const ONLY_UNDEFINED: Self = Self(Self::IS_UNDEFINED);

    pub(super) fn includes(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub(super) fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// `typeofEQFacts` / `typeofNEFacts` use exact native result strings.
    pub(super) fn typeof_test(name: &str, equal: bool) -> Self {
        let bit = match name {
            "string" => 0,
            "number" => 1,
            "bigint" => 2,
            "boolean" => 3,
            "symbol" => 4,
            "object" => 5,
            "function" => 6,
            "undefined" => {
                return Self(if equal {
                    Self::EQ_UNDEFINED
                } else {
                    Self::NE_UNDEFINED
                });
            }
            _ => 7,
        };
        Self(1 << (bit + if equal { 0 } else { 8 }))
    }

    pub(super) const NOT_UNDEFINED: Self = Self(Self::NE_UNDEFINED);
    pub(super) const NOT_NULL: Self = Self(Self::NE_NULL);
    pub(super) const NOT_NULLABLE: Self = Self(Self::NE_NULLABLE);
    pub(super) const NULLABLE: Self = Self(Self::EQ_NULLABLE);

    fn masked(self, mask: Self) -> Self {
        Self(self.0 & mask.0)
    }
    fn unknown() -> Self {
        Self(Self::ALL & !(Self::IS_UNDEFINED | Self::IS_NULL))
    }
    fn empty_object(strict: bool) -> Self {
        Self(
            Self::unknown().0
                & if strict {
                    !(Self::EQ_UNDEFINED | Self::EQ_NULL | Self::EQ_NULLABLE)
                } else {
                    u32::MAX
                },
        )
    }
    fn base(eq: u32, strict: bool, host: bool) -> u32 {
        let neq = (0xff ^ eq) & if host { !(1 << 7) } else { 0xff };
        eq | if host { 1 << 7 } else { 0 }
            | (neq << 8)
            | Self::NE_UNDEFINED
            | Self::NE_NULL
            | Self::NE_NULLABLE
            | if strict {
                0
            } else {
                Self::EQ_UNDEFINED | Self::EQ_NULL | Self::EQ_NULLABLE | Self::FALSY.0
            }
    }
    fn primitive(eq: u32, strict: bool, literal_truth: Option<bool>) -> Self {
        Self(
            Self::base(eq, strict, false)
                | match literal_truth {
                    Some(true) => Self::TRUTHY.0,
                    Some(false) => Self::FALSY.0,
                    None => Self::TRUTHY.0 | Self::FALSY.0,
                },
        )
    }
    fn object(strict: bool, function: bool) -> Self {
        Self(Self::base(1 << if function { 6 } else { 5 }, strict, true) | Self::TRUTHY.0)
    }
}

impl std::ops::BitOr for TypeFacts {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl Evaluator<'_, '_> {
    /// `getTypeFacts` / `getTypeFactsWorker`: branch order, mask early-out and
    /// every base/member/subtype dependency belong to this complete question.
    pub(super) fn type_facts(
        &mut self,
        ty: TypeId,
        mask: TypeFacts,
        consumer: &Location,
    ) -> Result<TypeFacts, DemandError> {
        self.type_facts_completion(ty, mask, consumer)?.admit()
    }

    pub(super) fn type_facts_completion(
        &mut self,
        ty: TypeId,
        mask: TypeFacts,
        consumer: &Location,
    ) -> NativeResult<TypeFacts> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let strict = self.inputs.semantic_options().strict_null_checks;
            // `getTypeFactsWorker` acquires the base once, then handles a surviving
            // intersection locally; an unchanged base is never a recursive call.
            let ty = if self
                .model
                .has_type_flags(ty, TypeFlags::INTERSECTION | TypeFlags::INSTANTIABLE)
            {
                match sequence.read(self.base_constraint_completion(ty, consumer))? {
                    Some(base) => base,
                    None => self.model.special(SpecialName::Unknown),
                }
            } else {
                ty
            };
            let facts = match self.model.get(ty)? {
                TypeView::Enum => TypeFacts::primitive(2, strict, None),
                TypeView::UniqueSymbol(_) => TypeFacts::primitive(16, strict, Some(true)),
                TypeView::Primitive(base) => match base {
                    PrimitiveName::String => TypeFacts::primitive(1, strict, None),
                    PrimitiveName::Number => TypeFacts::primitive(2, strict, None),
                    PrimitiveName::Bigint => TypeFacts::primitive(4, strict, None),
                    PrimitiveName::Boolean => TypeFacts::primitive(8, strict, None),
                    PrimitiveName::Symbol => TypeFacts::primitive(16, strict, Some(true)),
                    PrimitiveName::Object => TypeFacts::object(strict, false),
                },
                TypeView::StringMapping(_) => TypeFacts::primitive(1, strict, None),
                TypeView::TemplateLiteral(_) => TypeFacts::primitive(1, strict, Some(true)),
                TypeView::Literal { base, value } => {
                    let (eq, truthy) = match base {
                        PrimitiveName::String => (1, !value.is_empty()),
                        PrimitiveName::Number => (2, !value.equals_text("0")),
                        PrimitiveName::Bigint => (4, !value.equals_text("0")),
                        PrimitiveName::Boolean => (8, value.equals_text("true")),
                        PrimitiveName::Symbol | PrimitiveName::Object => {
                            return Err(super::super::internal(
                                "nonliteral native base reached type facts",
                            )
                            .into());
                        }
                    };
                    TypeFacts::primitive(eq, strict, Some(truthy))
                }
                TypeView::Special(name) => match name {
                    SpecialName::Any | SpecialName::Unknown => TypeFacts::unknown(),
                    SpecialName::Never => TypeFacts::NONE,
                    SpecialName::Void | SpecialName::Undefined => TypeFacts(
                        0xff00
                            | TypeFacts::EQ_UNDEFINED
                            | TypeFacts::EQ_NULLABLE
                            | TypeFacts::NE_NULL
                            | TypeFacts::FALSY.0
                            | if name == SpecialName::Undefined {
                                TypeFacts::IS_UNDEFINED
                            } else {
                                0
                            },
                    ),
                    SpecialName::Null => TypeFacts(
                        32 | ((0xff ^ 32) << 8)
                            | TypeFacts::EQ_NULL
                            | TypeFacts::EQ_NULLABLE
                            | TypeFacts::NE_UNDEFINED
                            | TypeFacts::FALSY.0
                            | TypeFacts::IS_NULL,
                    ),
                },
                TypeView::Union(types) => {
                    let mut facts = TypeFacts::NONE;
                    for &ty in types.iter() {
                        facts = facts
                            | sequence.read(self.type_facts_completion(ty, mask, consumer))?;
                    }
                    facts
                }
                TypeView::TypeParameter(_)
                | TypeView::IndexedAccess(_)
                | TypeView::Index(_)
                | TypeView::Conditional(_)
                | TypeView::Substitution(_) => TypeFacts::unknown(),
                TypeView::Intersection(types) => {
                    let ignore_objects = self.model.maybe_type_of_kind(ty, TypeFlags::PRIMITIVE);
                    let mut ored = 0;
                    let mut anded = TypeFacts::ALL;
                    for &child in types.iter() {
                        if ignore_objects && self.model.has_type_flags(child, TypeFlags::OBJECT) {
                            continue;
                        }
                        let facts =
                            sequence.read(self.type_facts_completion(child, mask, consumer))?;
                        ored |= facts.0;
                        anded &= facts.0;
                    }
                    // TypeofEQFunction and TypeofNEObject are the native OR facts;
                    // every other native possibility is combined by AND.
                    let or_mask = (1 << 6) | (1 << 13);
                    TypeFacts((ored & or_mask) | (anded & (TypeFacts::ALL & !or_mask)))
                }
                TypeView::Object(_) => {
                    if !TypeFacts::empty_object(strict).contains(mask) {
                        return Ok(TypeFacts::NONE);
                    }
                    if sequence.read(self.is_empty_anonymous_completion(ty, consumer))? {
                        TypeFacts::empty_object(strict)
                    } else {
                        TypeFacts::object(
                            strict,
                            sequence.read(self.is_function_object_type_completion(ty, consumer))?,
                        )
                    }
                }
                TypeView::Reference(_) => {
                    if !TypeFacts::empty_object(strict).contains(mask) {
                        return Ok(TypeFacts::NONE);
                    }
                    TypeFacts::object(
                        strict,
                        sequence.read(self.is_function_object_type_completion(ty, consumer))?,
                    )
                }
                TypeView::LazyObject(_) => {
                    if !TypeFacts::empty_object(strict).contains(mask) {
                        TypeFacts::NONE
                    } else if sequence.read(self.is_empty_anonymous_completion(ty, consumer))? {
                        TypeFacts::empty_object(strict)
                    } else {
                        TypeFacts::object(
                            strict,
                            sequence.read(self.is_function_object_type_completion(ty, consumer))?,
                        )
                    }
                }
            };
            Ok(facts.masked(mask))
        })();
        sequence.finish(result)
    }

    /// `getTypeFactsWorker`'s Anonymous guard followed by `isEmptyObjectType`:
    /// facts require the resolved inventory. This is deliberately distinct
    /// from IsEmptyAnonymousObjectType, which can use the symbol member table.
    fn is_empty_anonymous_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if self.model.is_anonymous(ty)? {
            self.is_empty_object_type_completion(ty, consumer)
        } else {
            Ok(NativeCompletion::clean(false))
        }
    }

    /// `isFunctionObjectType`: evolving arrays precede actual member
    /// resolution. A bind member admits the complete global Function subtype
    /// question; its name alone never establishes callable structure.
    pub(super) fn is_function_object_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_function_object_type_completion(ty, consumer)?
            .admit()
    }

    pub(super) fn is_function_object_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::EVOLVING_ARRAY)
            {
                return Ok(false);
            }
            let members = sequence
                .read(self.structured_type_members_completion(ty, consumer))?
                .ok_or_else(|| {
                    super::super::internal("function object question requires structured type")
                })?;
            if !members.call_signatures.is_empty() || !members.construct_signatures.is_empty() {
                return Ok(true);
            }
            if self.raw_member_present(ty, &members, "bind", consumer)? {
                let target =
                    sequence.read(self.global_object_type_completion("Function", consumer))?;
                return sequence.read(self.type_related_completion(
                    ty,
                    target,
                    super::relations::RelationKind::Subtype,
                    consumer,
                ));
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `getTypeWithFacts` keeps the native filterType identity, rather than
    /// widening a result or rebuilding every surviving union.
    pub(super) fn type_with_facts(
        &mut self,
        ty: TypeId,
        include: TypeFacts,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.type_with_facts_completion(ty, include, consumer)?
            .admit()
    }

    pub(super) fn type_with_facts_completion(
        &mut self,
        ty: TypeId,
        include: TypeFacts,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.filter_type_completion(ty, |checker, subject| {
            checker
                .type_facts_completion(subject, include, consumer)
                .map(|facts| facts.map(|facts| facts.contains(include)))
        })
    }

    /// `getAdjustedTypeWithFacts`: unknown initialization/recombination precede
    /// the native nullable adjustment. Missing intersection/global alias owners
    /// remain precise demanded dependencies, never unchanged healthy answers.
    pub(super) fn adjusted_type_with_facts_completion(
        &mut self,
        ty: TypeId,
        facts: TypeFacts,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let strict = self.inputs.semantic_options().strict_null_checks;
            let subject = if strict && self.model.has_type_flags(ty, TypeFlags::UNKNOWN) {
                self.model.unknown_union()
            } else {
                ty
            };
            let filtered =
                sequence.read(self.type_with_facts_completion(subject, facts, consumer))?;
            let reduced = self.model.recombine_unknown(filtered);
            if strict {
                if facts == TypeFacts::NOT_UNDEFINED || facts == TypeFacts::NOT_NULL {
                    let (target, other, includes, other_type) = if facts == TypeFacts::NOT_UNDEFINED
                    {
                        (
                            TypeFacts(TypeFacts::EQ_UNDEFINED),
                            TypeFacts(TypeFacts::EQ_NULL),
                            TypeFacts(TypeFacts::IS_NULL),
                            self.model.special(SpecialName::Null),
                        )
                    } else {
                        (
                            TypeFacts(TypeFacts::EQ_NULL),
                            TypeFacts(TypeFacts::EQ_UNDEFINED),
                            TypeFacts(TypeFacts::IS_UNDEFINED),
                            self.model.special(SpecialName::Undefined),
                        )
                    };
                    return sequence.read(self.remove_nullable_by_intersection_completion(
                        reduced, target, other, includes, other_type, consumer,
                    ));
                }
                if facts == TypeFacts::NOT_NULLABLE || facts == TypeFacts::TRUTHY {
                    let nullable = TypeFacts(TypeFacts::EQ_NULLABLE);
                    return sequence.read(self.map_type_completion(
                        reduced,
                        consumer,
                        |checker, subject| {
                            checker
                                .type_facts_completion(subject, nullable, consumer)?
                                .and_then(|facts| {
                                    if facts.contains(nullable) {
                                        checker.global_non_nullable_instantiation_completion(
                                            subject, consumer,
                                        )
                                    } else {
                                        Ok(NativeCompletion::clean(subject))
                                    }
                                })
                        },
                    ));
                }
            }
            Ok(reduced)
        })();
        sequence.finish(result)
    }

    /// `removeNullableByIntersection`: the aggregate facts decide whether
    /// the opposite nullable is already included; each constituent then asks
    /// its own possibility facts before the ordered intersection factory.
    fn remove_nullable_by_intersection_completion(
        &mut self,
        ty: TypeId,
        target: TypeFacts,
        other: TypeFacts,
        other_includes: TypeFacts,
        other_type: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let facts = sequence.read(self.type_facts_completion(
                ty,
                TypeFacts(
                    TypeFacts::EQ_UNDEFINED
                        | TypeFacts::EQ_NULL
                        | TypeFacts::IS_UNDEFINED
                        | TypeFacts::IS_NULL,
                ),
                consumer,
            ))?;
            if !facts.contains(target) {
                return Ok(ty);
            }
            let empty = self.model.empty_object_type();
            let empty_and_other = sequence.read(self.union_type_ex_completion(
                vec![empty, other_type],
                None,
                None,
                consumer,
            ))?;
            sequence.read(self.map_type_completion(ty, consumer, |checker, subject| {
                checker
                    .type_facts_completion(subject, target, consumer)?
                    .and_then(|subject_facts| {
                        if !subject_facts.contains(target) {
                            return Ok(NativeCompletion::clean(subject));
                        }
                        let other = if facts.contains(other_includes) {
                            NativeCompletion::clean(false)
                        } else {
                            checker
                                .type_facts_completion(subject, other, consumer)?
                                .map(|facts| facts.contains(other))
                        };
                        other.and_then(|other| {
                            let intersect = if other { empty_and_other } else { empty };
                            checker.intersection_type_completion(vec![subject, intersect], consumer)
                        })
                    })
            }))
        })();
        sequence.finish(result)
    }
}
