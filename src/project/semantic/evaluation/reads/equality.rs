//! Native equality conditions share the same reference interpreter as source
//! reads and inferred-predicate questions. Only the completed read admits F.

use super::super::super::model::{
    DemandError, NativeResult, NativeSequence, ObjectFlags, SpecialName, TypeFlags, TypeId,
};
use super::super::super::source::ExpressionRoot;
use super::super::Evaluator;
use super::super::facts::TypeFacts;
use super::super::globals::FixedGlobalType;
use super::super::relations::RelationKind;
use super::{ReadState, required, switches::type_name};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use crate::types::PrimitiveName;

impl Evaluator<'_, '_> {
    /// `narrowTypeByBinaryExpression`'s complete equality dispatch. Each native
    /// gate acquires only its own dependencies before moving to the next gate.
    pub(super) fn narrow_equality_condition_completion(
        &mut self,
        state: &ReadState<'_>,
        mut ty: TypeId,
        left: ExpressionRoot,
        right: ExpressionRoot,
        operator: SyntaxKind,
        truthy: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let left = self.reference_candidate(left)?;
            let right = self.reference_candidate(right)?;
            for (first, second) in [(left, right), (right, left)] {
                if first.site.kind == SyntaxKind::TypeOfExpression
                    && matches!(
                        second.site.kind,
                        SyntaxKind::StringLiteral | SyntaxKind::NoSubstitutionTemplateLiteral
                    )
                {
                    return sequence.read(self.narrow_typeof_condition_completion(
                        state, ty, first, second, operator, truthy,
                    ));
                }
            }
            if sequence.read(self.matches_reference_completion(state, left))? {
                return sequence.read(
                    self.narrow_equality_value_completion(state, ty, operator, right, truthy),
                );
            }
            if sequence.read(self.matches_reference_completion(state, right))? {
                return sequence.read(
                    self.narrow_equality_value_completion(state, ty, operator, left, truthy),
                );
            }
            if self.inputs.semantic_options().strict_null_checks {
                if sequence.read(self.optional_chain_contains_completion(state, left))? {
                    ty =
                        sequence.read(self.narrow_optional_equality_completion(
                            state, ty, operator, right, truthy,
                        ))?;
                } else if sequence.read(self.optional_chain_contains_completion(state, right))? {
                    ty = sequence.read(
                        self.narrow_optional_equality_completion(state, ty, operator, left, truthy),
                    )?;
                }
            }
            if let Some(access) =
                sequence.read(self.discriminant_property_access_completion(state, ty, left))?
            {
                return sequence.read(self.narrow_discriminant_equality_completion(
                    state, ty, access, operator, right, truthy,
                ));
            }
            if let Some(access) =
                sequence.read(self.discriminant_property_access_completion(state, ty, right))?
            {
                return sequence.read(self.narrow_discriminant_equality_completion(
                    state, ty, access, operator, left, truthy,
                ));
            }
            if sequence.read(self.matching_constructor_reference_completion(state, left))? {
                return sequence.read(
                    self.narrow_constructor_equality_completion(state, ty, operator, right, truthy),
                );
            }
            if sequence.read(self.matching_constructor_reference_completion(state, right))? {
                return sequence.read(
                    self.narrow_constructor_equality_completion(state, ty, operator, left, truthy),
                );
            }
            // `narrowTypeByBooleanComparison` recursively asks the ordinary whole
            // condition owner; the literal itself causes no expression demand.
            for (expression, boolean) in [(left, right), (right, left)] {
                if matches!(
                    boolean.site.kind,
                    SyntaxKind::TrueKeyword | SyntaxKind::FalseKeyword
                ) && !matches!(
                    expression.site.kind,
                    SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
                ) {
                    let truthy = (truthy != (boolean.site.kind == SyntaxKind::TrueKeyword))
                        == inequality(operator);
                    return sequence
                        .read(self.narrow_condition_completion(state, ty, expression, truthy));
                }
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    /// `narrowTypeByEquality`: Any skips the value demand; nullable facts,
    /// strict empty/unknown replacement, comparison and literal substitution
    /// retain their native order and actual intermediate type identities.
    fn narrow_equality_value_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        operator: SyntaxKind,
        value: ExpressionRoot,
        truthy: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if self.model.has_type_flags(ty, TypeFlags::ANY) {
                return Ok(ty);
            }
            let truthy = truthy != inequality(operator);
            let value =
                sequence.read(self.type_of_expression_completion(value.file, value.site))?;
            let loose = matches!(
                operator,
                SyntaxKind::EqualsEqualsToken | SyntaxKind::ExclamationEqualsToken
            );
            if self.model.has_type_flags(value, TypeFlags::NULLABLE) {
                if !self.inputs.semantic_options().strict_null_checks {
                    return Ok(ty);
                }
                let facts = if loose {
                    if truthy {
                        TypeFacts::NULLABLE
                    } else {
                        TypeFacts::NOT_NULLABLE
                    }
                } else if self.model.has_type_flags(value, TypeFlags::NULL) {
                    if truthy {
                        TypeFacts::EQUAL_NULL
                    } else {
                        TypeFacts::NOT_NULL
                    }
                } else if truthy {
                    TypeFacts::EQUAL_UNDEFINED
                } else {
                    TypeFacts::NOT_UNDEFINED
                };
                return sequence.read(self.adjusted_type_with_facts_completion(
                    ty,
                    facts,
                    state.location,
                ));
            }
            let subjects = self.model.union_constituents(ty);
            let subjects = subjects.as_deref().unwrap_or(std::slice::from_ref(&ty));
            if truthy {
                if !loose {
                    let mut empty_or_unknown = self.model.has_type_flags(ty, TypeFlags::UNKNOWN);
                    if !empty_or_unknown {
                        for &subject in subjects {
                            if self.is_empty_anonymous_object_type(subject, state.location)? {
                                empty_or_unknown = true;
                                break;
                            }
                        }
                    }
                    if empty_or_unknown {
                        if self
                            .model
                            .has_type_flags(value, TypeFlags::PRIMITIVE | TypeFlags::NON_PRIMITIVE)
                            || self.is_empty_anonymous_object_type(value, state.location)?
                        {
                            return Ok(value);
                        }
                        if self.model.has_type_flags(value, TypeFlags::OBJECT) {
                            return Ok(self.model.primitive(PrimitiveName::Object));
                        }
                    }
                }
                let filtered =
                    sequence.read(self.filter_type_completion(ty, |checker, subject| {
                        let mut sequence = NativeSequence::new();
                        let result = (|| {
                            Ok(sequence.read(checker.are_types_comparable_completion(
                                subject,
                                value,
                                state.location,
                            ))? || loose
                                && checker.coercible_under_double_equals(subject, value))
                        })();
                        sequence.finish(result)
                    }))?;
                return sequence.read(self.replace_primitives_with_literals_completion(
                    filtered,
                    value,
                    state.location,
                ));
            }
            if self.model.has_type_flags(value, TypeFlags::UNIT) {
                return sequence.read(self.filter_type_completion(ty, |checker, subject| {
                    let mut sequence = NativeSequence::new();
                    let result = (|| {
                        Ok(!(sequence
                            .read(checker.is_unit_like_type_completion(subject, state.location))?
                            && sequence.read(checker.are_types_comparable_completion(
                                subject,
                                value,
                                state.location,
                            ))?))
                    })();
                    sequence.finish(result)
                }));
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    /// `isCoercibleUnderDoubleEquals` uses flags on the actual pair; unions
    /// and literal targets deliberately do not broaden these masks.
    fn coercible_under_double_equals(&self, source: TypeId, target: TypeId) -> bool {
        self.model.has_type_flags(
            source,
            TypeFlags::NUMBER | TypeFlags::STRING | TypeFlags::BOOLEAN_LITERAL,
        ) && self.model.has_type_flags(
            target,
            TypeFlags::NUMBER | TypeFlags::STRING | TypeFlags::BOOLEAN,
        )
    }

    /// `isUnitLikeType`: acquire the base constraint before scanning an
    /// intersection for Unit. Tagged units and intersections reduced to Never
    /// therefore retain their distinct native answers.
    pub(super) fn is_unit_like_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let ty = match sequence.read(self.base_constraint_completion(ty, consumer))? {
                Some(base) => base,
                None => ty,
            };
            if let Some(types) = self.model.intersection_constituents(ty) {
                return Ok(types
                    .iter()
                    .any(|&ty| self.model.has_type_flags(ty, TypeFlags::UNIT)));
            }
            Ok(self.model.has_type_flags(ty, TypeFlags::UNIT))
        })();
        sequence.finish(result)
    }

    /// `extractUnitType` inspects the original intersection, independently
    /// of the base-constraint acquisition in `isUnitLikeType`.
    pub(super) fn extract_unit_type(&self, ty: TypeId) -> TypeId {
        if let Some(types) = self.model.intersection_constituents(ty) {
            if let Some(&unit) = types
                .iter()
                .find(|&&ty| self.model.has_type_flags(ty, TypeFlags::UNIT))
            {
                return unit;
            }
        }
        ty
    }

    /// `narrowTypeByOptionalChainContainment`: the opposite value is acquired
    /// once, then Every uses actual union constituents and native nullable masks.
    fn narrow_optional_equality_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        operator: SyntaxKind,
        value: ExpressionRoot,
        truthy: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let equals = !inequality(operator);
            let nullable = if matches!(
                operator,
                SyntaxKind::EqualsEqualsToken | SyntaxKind::ExclamationEqualsToken
            ) {
                TypeFlags::NULLABLE
            } else {
                TypeFlags::UNDEFINED
            };
            let value =
                sequence.read(self.type_of_expression_completion(value.file, value.site))?;
            let values = self.model.union_constituents(value);
            let values = values.as_deref().unwrap_or(std::slice::from_ref(&value));
            let remove = if equals != truthy {
                values
                    .iter()
                    .all(|&ty| self.model.has_type_flags(ty, nullable))
            } else {
                values.iter().all(|&ty| {
                    !self
                        .model
                        .has_type_flags(ty, TypeFlags::ANY | TypeFlags::UNKNOWN | nullable)
                })
            };
            if remove {
                sequence.read(self.adjusted_type_with_facts_completion(
                    ty,
                    TypeFacts::NOT_NULLABLE,
                    state.location,
                ))
            } else {
                Ok(ty)
            }
        })();
        sequence.finish(result)
    }

    /// `narrowTypeByTypeof`: matching, optional containment, and discriminant
    /// questions precede the shared literal/type-name reduction.
    fn narrow_typeof_condition_completion(
        &mut self,
        state: &ReadState<'_>,
        mut ty: TypeId,
        expression: ExpressionRoot,
        literal: ExpressionRoot,
        operator: SyntaxKind,
        truthy: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let truthy = truthy != inequality(operator);
            let syntax = self.source.flow_syntax(expression.file, expression.site)?;
            let target = self.reference_candidate(ExpressionRoot {
                file: expression.file,
                site: required(syntax.expression, "typeof has no operand")?,
            })?;
            if !sequence.read(self.matches_reference_completion(state, target))? {
                if self.inputs.semantic_options().strict_null_checks
                    && sequence.read(self.optional_chain_contains_completion(state, target))?
                {
                    let witness = self
                        .source
                        .string_literal_value(literal.file, literal.site)?;
                    if truthy == (type_name(&witness) != "undefined") {
                        ty = sequence.read(self.adjusted_type_with_facts_completion(
                            ty,
                            TypeFacts::NOT_NULLABLE,
                            state.location,
                        ))?;
                    }
                }
                if let Some(access) = sequence
                    .read(self.discriminant_property_access_completion(state, ty, target))?
                {
                    return sequence.read(self.narrow_discriminant_completion(
                        state,
                        ty,
                        access,
                        |checker, property| {
                            checker.narrow_literal_condition_completion(
                                state, property, literal, truthy,
                            )
                        },
                    ));
                }
                return Ok(ty);
            }
            sequence.read(self.narrow_literal_condition_completion(state, ty, literal, truthy))
        })();
        sequence.finish(result)
    }

    /// `narrowTypeByLiteralExpression` shares the exact typeof witness decoder
    /// and native type-name/fact owners with switch conditions.
    fn narrow_literal_condition_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        literal: ExpressionRoot,
        truthy: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let witness = self
                .source
                .string_literal_value(literal.file, literal.site)?;
            let name = type_name(&witness);
            if truthy {
                sequence.read(self.narrow_type_name_completion(ty, name, state.location))
            } else {
                sequence.read(self.adjusted_type_with_facts_completion(
                    ty,
                    TypeFacts::typeof_test(name, false),
                    state.location,
                ))
            }
        })();
        sequence.finish(result)
    }

    /// `narrowTypeByDiscriminantProperty`: strict comparisons first consult
    /// the union key optimization; ordinary unions take the shared callback.
    fn narrow_discriminant_equality_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        access: ExpressionRoot,
        operator: SyntaxKind,
        value: ExpressionRoot,
        truthy: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if matches!(
                operator,
                SyntaxKind::EqualsEqualsEqualsToken | SyntaxKind::ExclamationEqualsEqualsToken
            ) && self.model.has_type_flags(ty, TypeFlags::UNION)
                && let Some(name) =
                    sequence.read(self.union_key_property_name_completion(ty, state.location))?
                && sequence
                    .read(self.accessed_property_name_completion(
                        access.file,
                        access.site,
                        state.location,
                    ))?
                    .as_ref()
                    == Some(&name)
            {
                let key =
                    sequence.read(self.type_of_expression_completion(value.file, value.site))?;
                if let Some(candidate) = self.union_constituent_for_key(ty, key)? {
                    if truthy != inequality(operator) {
                        return Ok(candidate);
                    }
                    if let Some(value) =
                        sequence.read(self.type_of_property_of_type_by_key_completion(
                            candidate,
                            &name,
                            state.location,
                        ))?
                        && self.model.has_type_flags(value, TypeFlags::UNIT)
                    {
                        return Ok(self.model.remove_type(ty, candidate));
                    }
                    return Ok(ty);
                }
            }
            sequence.read(self.narrow_discriminant_completion(
                state,
                ty,
                access,
                |checker, property| {
                    checker
                        .narrow_equality_value_completion(state, property, operator, value, truthy)
                },
            ))
        })();
        sequence.finish(result)
    }

    /// `narrowTypeByDiscriminant`: one owner for optional access adjustment,
    /// property callback and constituent comparability, for every narrow rule.
    pub(super) fn narrow_discriminant_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        access: ExpressionRoot,
        narrow: impl FnOnce(&mut Self, TypeId) -> NativeResult<TypeId>,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let Some(name) = sequence.read(self.accessed_property_name_completion(
                access.file,
                access.site,
                state.location,
            ))?
            else {
                return Ok(ty);
            };
            let syntax = self.source.flow_syntax(access.file, access.site)?;
            let nonnull_access = syntax
                .expression
                .is_some_and(|expression| expression.kind == SyntaxKind::NonNullExpression);
            let remove_nullable = self.inputs.semantic_options().strict_null_checks
                && (syntax.optional_chain || nonnull_access)
                && self.model.maybe_type_of_kind(ty, TypeFlags::NULLABLE);
            let nonnull = if remove_nullable {
                sequence.read(self.type_with_facts_completion(
                    ty,
                    TypeFacts::NOT_NULLABLE,
                    state.location,
                ))?
            } else {
                ty
            };
            let Some(mut property) = sequence.read(
                self.type_of_property_of_type_by_key_completion(nonnull, &name, state.location),
            )?
            else {
                return Ok(ty);
            };
            if remove_nullable && syntax.optional_chain {
                property = sequence.read(self.optional_type_completion(
                    property,
                    false,
                    state.location,
                ))?;
            }
            let narrowed = sequence.read(narrow(self, property))?;
            sequence.read(self.filter_type_completion(ty, |checker, subject| {
                let mut sequence = NativeSequence::new();
                let result = (|| {
                    let discriminant = match sequence.read(
                        checker.type_of_property_or_index_signature_by_key_completion(
                            subject,
                            &name,
                            state.location,
                        ),
                    )? {
                        Some(value) => value,
                        None => checker.model.special(SpecialName::Unknown),
                    };
                    Ok(
                        !checker.model.has_type_flags(discriminant, TypeFlags::NEVER)
                            && !checker.model.has_type_flags(narrowed, TypeFlags::NEVER)
                            && sequence.read(checker.are_types_comparable_completion(
                                narrowed,
                                discriminant,
                                state.location,
                            ))?,
                    )
                })();
                sequence.finish(result)
            }))
        })();
        sequence.finish(result)
    }

    /// `isMatchingConstructorReference`: the actual accessed name precedes
    /// receiver matching; no expression-type or constructor inventory is read.
    fn matching_constructor_reference_completion(
        &mut self,
        state: &ReadState<'_>,
        site: ExpressionRoot,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if !matches!(
                site.site.kind,
                SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
            ) {
                return Ok(false);
            }
            if sequence
                .read(self.accessed_property_name_completion(site.file, site.site, state.location))?
                .as_ref()
                .and_then(|key| key.fixed_bytes())
                != Some(b"constructor".as_slice())
            {
                return Ok(false);
            }
            let syntax = self.source.flow_syntax(site.file, site.site)?;
            sequence.read(self.matches_reference_completion(
                state,
                ExpressionRoot {
                    file: site.file,
                    site: required(syntax.expression, "constructor access has no receiver")?,
                },
            ))
        })();
        sequence.finish(result)
    }

    /// `narrowTypeByConstructor` / `isConstructedBy`: inequality returns before
    /// acquisition; class identity precedes the ordinary subtype relation.
    fn narrow_constructor_equality_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        operator: SyntaxKind,
        value: ExpressionRoot,
        truthy: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if truthy == inequality(operator) {
                return Ok(ty);
            }
            let value =
                sequence.read(self.type_of_expression_completion(value.file, value.site))?;
            let function = self.model.has_type_flags(value, TypeFlags::OBJECT)
                && !sequence
                    .read(self.call_signatures_completion(value, state.location))?
                    .is_empty();
            if !function
                && !sequence.read(self.is_constructor_type_completion(value, state.location))?
            {
                return Ok(ty);
            }
            let Some(property) = sequence.read(self.property_of_type_completion(
                value,
                "prototype",
                state.location,
            ))?
            else {
                return Ok(ty);
            };
            let symbol = self.property_symbol(&property, state.location)?;
            let candidate =
                sequence.read(self.property_raw_type_completion(symbol, state.location))?;
            if self.model.has_type_flags(candidate, TypeFlags::ANY)
                || candidate
                    == self
                        .fixed_global_type_identity(FixedGlobalType::Object, state.location)?
                        .map_or(self.model.empty_object_type(), |(_, ty)| ty)
                || candidate
                    == self
                        .fixed_global_type_identity(FixedGlobalType::Function, state.location)?
                        .map_or(self.model.empty_object_type(), |(_, ty)| ty)
            {
                return Ok(ty);
            }
            if self.model.has_type_flags(ty, TypeFlags::ANY) {
                return Ok(candidate);
            }
            sequence.read(self.filter_type_completion(ty, |checker, subject| {
                let mut sequence = NativeSequence::new();
                let result = (|| {
                    let class = checker.model.has_type_flags(subject, TypeFlags::OBJECT)
                        && checker
                            .model
                            .object_flags(subject)?
                            .intersects(ObjectFlags::CLASS)
                        || checker.model.has_type_flags(candidate, TypeFlags::OBJECT)
                            && checker
                                .model
                                .object_flags(candidate)?
                                .intersects(ObjectFlags::CLASS);
                    if class {
                        Ok(checker.type_symbol_identity(subject)?
                            == checker.type_symbol_identity(candidate)?)
                    } else {
                        sequence.read(checker.type_related_completion(
                            subject,
                            candidate,
                            RelationKind::Subtype,
                            state.location,
                        ))
                    }
                })();
                sequence.finish(result)
            }))
        })();
        sequence.finish(result)
    }
}

fn inequality(operator: SyntaxKind) -> bool {
    matches!(
        operator,
        SyntaxKind::ExclamationEqualsToken | SyntaxKind::ExclamationEqualsEqualsToken
    )
}
