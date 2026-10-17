//! Native switch admission and exhaustive-state ownership are one operation.
//! Read traversal and end reachability keep their separate scheduling policies.

use super::super::super::model::{
    DemandError, LiteralValue, NativeCompletion, NativeResult, NativeSequence, SpecialName,
    TypeFlags, TypeId,
};
use super::super::super::source::ExpressionRoot;
use super::super::Evaluator;
use super::super::facts::TypeFacts;
use super::super::globals::FixedGlobalType;
use super::super::relations::RelationKind;
use super::{ReadState, required};
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use crate::types::PrimitiveName;
use std::collections::HashMap;

type Key = (FileId, SyntaxKind, usize, usize);
type Witnesses = Option<Vec<Option<LiteralValue>>>;

#[derive(Clone)]
enum Exhaustive {
    Computing,
    Ready(NativeCompletion<bool>),
}

#[derive(Default)]
pub(super) struct SwitchCache {
    witnesses: HashMap<Key, Witnesses>,
    types: HashMap<Key, NativeCompletion<Vec<TypeId>>>,
    exhaustive: HashMap<Key, Exhaustive>,
}

fn key(file: FileId, site: Site) -> Key {
    (file, site.kind, site.span.start, site.span.end)
}

/// `narrowTypeByTypeName` asks standard names separately from raw witness
/// identity. Unknown exact native names take the native host-object branch.
pub(super) fn type_name(witness: &LiteralValue) -> &'static str {
    for name in [
        "string",
        "number",
        "bigint",
        "boolean",
        "symbol",
        "object",
        "function",
        "undefined",
    ] {
        if witness.equals_text(name) {
            return name;
        }
    }
    ""
}

impl Evaluator<'_, '_> {
    /// `getTypeAtSwitchClause`: antecedent typing belongs to its caller's
    /// child invocation. Native matching and branch order decide exactly which
    /// syntax/type questions are consumed. Intermediate native types remain
    /// available to joins; the complete reference read owns F admission.
    pub(super) fn read_switch_clause_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        statement: Site,
        start: usize,
        end: usize,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let syntax = self.source.flow_syntax(state.file, statement)?;
            let mut expression = ExpressionRoot {
                file: state.file,
                site: required(syntax.expression, "switch has no expression")?,
            };
            loop {
                let syntax = self.source.flow_syntax(expression.file, expression.site)?;
                if syntax.kind != SyntaxKind::ParenthesizedExpression {
                    break;
                }
                expression.site = required(syntax.expression, "switch wrapper has no expression")?;
            }
            let syntax = self.source.flow_syntax(expression.file, expression.site)?;
            let narrowed = if sequence.read(self.matches_reference_completion(state, expression))? {
                sequence.read(self.switch_discriminant_completion(
                    ty,
                    state.file,
                    statement,
                    start,
                    end,
                    state.location,
                ))?
            } else if syntax.kind == SyntaxKind::TypeOfExpression
                && sequence.read(self.matches_reference_completion(
                    state,
                    ExpressionRoot {
                        file: expression.file,
                        site: required(syntax.expression, "typeof switch has no operand")?,
                    },
                ))?
            {
                sequence.read(self.switch_typeof_completion(
                    ty,
                    state.file,
                    statement,
                    start,
                    end,
                    state.location,
                ))?
            } else if syntax.kind == SyntaxKind::TrueKeyword {
                let syntax = self.source.switch_syntax(state.file, statement)?;
                let mut ty = ty;
                let default = syntax
                    .clauses
                    .iter()
                    .position(|clause| clause.expression.is_none());
                for clause in &syntax.clauses[..start] {
                    if let Some(expression) = clause.expression {
                        ty = sequence.read(self.narrow_condition_completion(
                            state,
                            ty,
                            ExpressionRoot {
                                file: state.file,
                                site: expression,
                            },
                            false,
                        ))?;
                    }
                }
                if start == end || default.is_some_and(|index| (start..end).contains(&index)) {
                    for clause in &syntax.clauses[end..] {
                        if let Some(expression) = clause.expression {
                            ty = sequence.read(self.narrow_condition_completion(
                                state,
                                ty,
                                ExpressionRoot {
                                    file: state.file,
                                    site: expression,
                                },
                                false,
                            ))?;
                        }
                    }
                    ty
                } else {
                    let mut types = Vec::new();
                    for clause in &syntax.clauses[start..end] {
                        if let Some(expression) = clause.expression {
                            types.push(sequence.read(self.narrow_condition_completion(
                                state,
                                ty,
                                ExpressionRoot {
                                    file: state.file,
                                    site: expression,
                                },
                                true,
                            ))?);
                        } else {
                            types.push(self.model.special(SpecialName::Never));
                        }
                    }
                    sequence.read(self.switch_union_completion(types, state.location))?
                }
            } else {
                let mut result = ty;
                if self.inputs.semantic_options().strict_null_checks {
                    let optional = sequence
                        .read(self.optional_chain_contains_completion(state, expression))?;
                    let optional_typeof = !optional
                        && syntax.kind == SyntaxKind::TypeOfExpression
                        && sequence.read(self.optional_chain_contains_completion(
                            state,
                            ExpressionRoot {
                                file: expression.file,
                                site: required(syntax.expression, "typeof switch has no operand")?,
                            },
                        ))?;
                    if optional || optional_typeof {
                        let types = sequence
                            .read(self.switch_clause_types_completion(state.file, statement))?;
                        let checks = start != end
                            && types[start..end].iter().all(|&ty| {
                                !self.model.has_type_flags(ty, TypeFlags::NEVER)
                                    && if optional {
                                        !self.model.has_type_flags(ty, TypeFlags::UNDEFINED)
                                    } else {
                                        !self.model.is_string_literal_value(ty, "undefined")
                                    }
                            });
                        if checks {
                            result = sequence.read(self.type_with_facts_completion(
                                result,
                                TypeFacts::NOT_NULLABLE,
                                state.location,
                            ))?;
                        }
                    }
                }
                if let Some(access) = sequence
                    .read(self.discriminant_property_access_completion(state, result, expression))?
                {
                    result = sequence.read(self.switch_discriminant_property_completion(
                        state, result, access, statement, start, end,
                    ))?;
                }
                result
            };
            Ok(narrowed)
        })();
        sequence.finish(result)
    }

    /// `getSwitchClauseTypeOfWitnesses`: syntax-only, all cases literal-like,
    /// exact decoded duplicate/default entries empty. No expression checking.
    fn switch_witnesses(
        &mut self,
        file: FileId,
        statement: Site,
    ) -> Result<Witnesses, DemandError> {
        let key = key(file, statement);
        if let Some(witnesses) = self.reads.switches.witnesses.get(&key) {
            return Ok(witnesses.clone());
        }
        let syntax = self.source.switch_syntax(file, statement)?;
        let mut witnesses = Vec::new();
        for clause in syntax.clauses {
            if clause.expression.is_some() {
                let Some(witness) = clause.witness else {
                    self.reads.switches.witnesses.insert(key, None);
                    return Ok(None);
                };
                if witness.is_empty()
                    || witnesses
                        .iter()
                        .any(|prior: &Option<LiteralValue>| prior.as_ref() == Some(&witness))
                {
                    witnesses.push(None);
                } else {
                    witnesses.push(Some(witness));
                }
            } else {
                witnesses.push(None);
            }
        }
        self.reads
            .switches
            .witnesses
            .insert(key, Some(witnesses.clone()));
        Ok(Some(witnesses))
    }

    /// `getSwitchClauseTypes` / `getTypeOfSwitchClause`: complete source-order
    /// vector, getTypeOfExpression and regular literals, genuine default Never.
    fn switch_clause_types_completion(
        &mut self,
        file: FileId,
        statement: Site,
    ) -> NativeResult<Vec<TypeId>> {
        let key = key(file, statement);
        if let Some(types) = self.reads.switches.types.get(&key) {
            return Ok(types.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<TypeId>, DemandError> {
            let syntax = self.source.switch_syntax(file, statement)?;
            let mut types = Vec::new();
            for clause in syntax.clauses {
                types.push(if let Some(expression) = clause.expression {
                    let ty = sequence.read(self.type_of_expression_completion(file, expression))?;
                    self.model.regular_literal(ty)
                } else {
                    self.model.special(SpecialName::Never)
                });
            }
            Ok(types)
        })();
        let completed = sequence.finish(result)?;
        self.reads.switches.types.insert(key, completed.clone());
        Ok(completed)
    }

    /// `getNotEqualFactsFromTypeofSwitch`: OR the exact native facts outside
    /// the group; inclusion requires every requested fact, not any one bit.
    fn switch_not_equal(start: usize, end: usize, witnesses: &[Option<LiteralValue>]) -> TypeFacts {
        let mut facts = TypeFacts::NONE;
        for (index, witness) in witnesses.iter().enumerate() {
            if !(start..end).contains(&index)
                && let Some(witness) = witness
            {
                facts = facts | TypeFacts::typeof_test(type_name(witness), false);
            }
        }
        facts
    }

    /// `narrowTypeBySwitchOnTypeOf`: nil witnesses preserve identity without
    /// demanded case types. Default filters conjunctive outside-group facts.
    fn switch_typeof_completion(
        &mut self,
        ty: TypeId,
        file: FileId,
        statement: Site,
        start: usize,
        end: usize,
        location: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let Some(witnesses) = self.switch_witnesses(file, statement)? else {
                return Ok(ty);
            };
            let syntax = self.source.switch_syntax(file, statement)?;
            let default = syntax
                .clauses
                .iter()
                .position(|clause| clause.expression.is_none());
            if start == end || default.is_some_and(|index| (start..end).contains(&index)) {
                let facts = Self::switch_not_equal(start, end, &witnesses);
                sequence.read(self.filter_type_completion(ty, |checker, subject| {
                    let mut sequence = NativeSequence::new();
                    let result = (|| {
                        Ok(sequence
                            .read(checker.type_facts_completion(subject, facts, location))?
                            == facts)
                    })();
                    sequence.finish(result)
                }))
            } else {
                let mut types = Vec::new();
                for witness in &witnesses[start..end] {
                    types.push(if let Some(witness) = witness {
                        sequence.read(self.narrow_type_name_completion(
                            ty,
                            type_name(witness),
                            location,
                        ))?
                    } else {
                        self.model.special(SpecialName::Never)
                    });
                }
                sequence.read(self.switch_union_completion(types, location))
            }
        })();
        sequence.finish(result)
    }

    /// `narrowTypeByTypeName` / `narrowTypeByTypeFacts`: native relation order,
    /// including the actual global Function identity and intersection factory.
    pub(super) fn narrow_type_name_completion(
        &mut self,
        ty: TypeId,
        name: &str,
        location: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if (name == "object" || name == "function")
                && self.model.has_type_flags(ty, TypeFlags::ANY)
            {
                return Ok(ty);
            }
            if name == "object" {
                let object = self.model.primitive(PrimitiveName::Object);
                let object = sequence.read(self.narrow_type_facts_completion(
                    ty,
                    object,
                    TypeFacts::typeof_test(name, true),
                    location,
                ))?;
                let null = self.model.special(SpecialName::Null);
                let null = sequence.read(self.narrow_type_facts_completion(
                    ty,
                    null,
                    TypeFacts::EQUAL_NULL,
                    location,
                ))?;
                return sequence.read(self.switch_union_completion(vec![object, null], location));
            }
            let implied = match name {
                "string" => self.model.primitive(PrimitiveName::String),
                "number" => self.model.primitive(PrimitiveName::Number),
                "bigint" => self.model.primitive(PrimitiveName::Bigint),
                "boolean" => self.model.primitive(PrimitiveName::Boolean),
                "symbol" => self.model.primitive(PrimitiveName::Symbol),
                "undefined" => self.model.special(SpecialName::Undefined),
                "function" => self
                    .fixed_global_type_identity(FixedGlobalType::Function, location)?
                    .map_or(self.model.empty_object_type(), |(_, ty)| ty),
                _ => self.model.primitive(PrimitiveName::Object),
            };
            sequence.read(self.narrow_type_facts_completion(
                ty,
                implied,
                TypeFacts::typeof_test(name, true),
                location,
            ))
        })();
        sequence.finish(result)
    }

    fn narrow_type_facts_completion(
        &mut self,
        ty: TypeId,
        implied: TypeId,
        facts: TypeFacts,
        location: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = sequence.read(self.map_type_completion(ty, location, |checker, subject| {
            let mut sequence = NativeSequence::new();
            let result = (|| {
                if sequence.read(checker.type_related_completion(
                    subject,
                    implied,
                    RelationKind::StrictSubtype,
                    location,
                ))? {
                    if sequence
                        .read(checker.type_facts_completion(subject, facts, location))?
                        .contains(facts)
                    {
                        Ok(subject)
                    } else {
                        Ok(checker.model.special(SpecialName::Never))
                    }
                } else if sequence.read(checker.type_related_completion(
                    implied,
                    subject,
                    RelationKind::Subtype,
                    location,
                ))? {
                    Ok(implied)
                } else if sequence
                    .read(checker.type_facts_completion(subject, facts, location))?
                    .contains(facts)
                {
                    sequence.read(
                        checker.intersection_type_completion(vec![subject, implied], location),
                    )
                } else {
                    Ok(checker.model.special(SpecialName::Never))
                }
            })();
            sequence.finish(result)
        }));
        sequence.finish(result)
    }

    /// `narrowTypeBySwitchOnDiscriminantProperty`: the nonempty accessed name
    /// precedes the union key map and whole clause vector. An Unknown candidate
    /// takes the ordinary property callback, including its second clause-cache
    /// acquisition; only the completed reference read decides F admission.
    fn switch_discriminant_property_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        access: ExpressionRoot,
        statement: Site,
        start: usize,
        end: usize,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if start < end
                && self.model.has_type_flags(ty, TypeFlags::UNION)
                && let Some(name) = sequence.read(self.accessed_property_name_completion(
                    access.file,
                    access.site,
                    state.location,
                ))?
                && name.fixed_bytes() != Some(b"".as_slice())
                && sequence.read(self.union_key_property_name_completion(ty, state.location))?
                    == Some(name)
            {
                let clauses =
                    sequence.read(self.switch_clause_types_completion(state.file, statement))?;
                let unknown = self.model.special(SpecialName::Unknown);
                let mut candidates = Vec::new();
                for &clause in &clauses[start..end] {
                    candidates.push(match self.union_constituent_for_key(ty, clause)? {
                        Some(candidate) => candidate,
                        None => unknown,
                    });
                }
                let candidate =
                    sequence.read(self.switch_union_completion(candidates, state.location))?;
                if candidate != unknown {
                    return Ok(candidate);
                }
            }
            sequence.read(self.narrow_discriminant_completion(
                state,
                ty,
                access,
                |checker, property| {
                    checker.switch_discriminant_completion(
                        property,
                        state.file,
                        statement,
                        start,
                        end,
                        state.location,
                    )
                },
            ))
        })();
        sequence.finish(result)
    }

    /// `narrowTypeBySwitchOnDiscriminant`: full case vector before comparison,
    /// primitive replacement, then native unit/default exclusion.
    fn switch_discriminant_completion(
        &mut self,
        ty: TypeId,
        file: FileId,
        statement: Site,
        start: usize,
        end: usize,
        location: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let types = sequence.read(self.switch_clause_types_completion(file, statement))?;
            if types.is_empty() {
                return Ok(ty);
            }
            let clauses = &types[start..end];
            // Native `slices.Contains(clauseTypes, c.neverType)`.
            let has_default =
                start == end || clauses.contains(&self.model.special(SpecialName::Never));
            if self.model.has_type_flags(ty, TypeFlags::UNKNOWN) && !has_default {
                let mut ground = Vec::new();
                for &clause in clauses {
                    if self
                        .model
                        .has_type_flags(clause, TypeFlags::PRIMITIVE | TypeFlags::NON_PRIMITIVE)
                    {
                        ground.push(clause);
                    } else if self.model.has_type_flags(clause, TypeFlags::OBJECT) {
                        ground.push(self.model.primitive(PrimitiveName::Object));
                    } else {
                        return Ok(ty);
                    }
                }
                return sequence.read(self.switch_union_completion(ground, location));
            }
            let discriminant =
                sequence.read(self.switch_union_completion(clauses.to_vec(), location))?;
            let case = if self.model.has_type_flags(discriminant, TypeFlags::NEVER) {
                self.model.special(SpecialName::Never)
            } else {
                let filtered =
                    sequence.read(self.filter_type_completion(ty, |checker, subject| {
                        checker.are_types_comparable_completion(discriminant, subject, location)
                    }))?;
                sequence.read(self.replace_primitives_with_literals_completion(
                    filtered,
                    discriminant,
                    location,
                ))?
            };
            if !has_default {
                return Ok(case);
            }
            let default = sequence.read(self.filter_type_completion(ty, |checker, subject| {
                let mut sequence = NativeSequence::new();
                let result = (|| {
                    if sequence.read(checker.is_unit_like_type_completion(subject, location))? {
                        let unit = if checker.model.has_type_flags(subject, TypeFlags::UNDEFINED) {
                            checker.model.special(SpecialName::Undefined)
                        } else {
                            let unit = checker.extract_unit_type(subject);
                            checker.model.regular_literal(unit)
                        };
                        for &clause in &types {
                            if checker.model.has_type_flags(clause, TypeFlags::UNIT)
                                && sequence.read(
                                    checker.are_types_comparable_completion(clause, unit, location),
                                )?
                            {
                                return Ok(false);
                            }
                        }
                    }
                    Ok(true)
                })();
                sequence.finish(result)
            }))?;
            if self.model.has_type_flags(case, TypeFlags::NEVER) {
                Ok(default)
            } else {
                sequence.read(self.switch_union_completion(vec![case, default], location))
            }
        })();
        sequence.finish(result)
    }

    /// `replacePrimitivesWithLiterals`: native broad flag gates precede the
    /// per-constituent pattern question. Mapping preserves unchanged identity.
    pub(super) fn replace_primitives_with_literals_completion(
        &mut self,
        ty: TypeId,
        literals: TypeId,
        location: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if !self.model.maybe_type_of_kind(
                ty,
                TypeFlags::STRING
                    | TypeFlags::TEMPLATE_LITERAL
                    | TypeFlags::NUMBER
                    | TypeFlags::BIGINT,
            ) || !self.model.maybe_type_of_kind(
                literals,
                TypeFlags::STRING_LITERAL
                    | TypeFlags::TEMPLATE_LITERAL
                    | TypeFlags::STRING_MAPPING
                    | TypeFlags::NUMBER_LITERAL
                    | TypeFlags::BIGINT_LITERAL,
            ) {
                return Ok(ty);
            }
            sequence.read(self.map_type_completion(ty, location, |checker, subject| {
                let result = (|| -> Result<TypeId, DemandError> {
                    let mask = if checker.model.has_type_flags(subject, TypeFlags::STRING) {
                        Some(
                            TypeFlags::STRING
                                | TypeFlags::STRING_LITERAL
                                | TypeFlags::TEMPLATE_LITERAL
                                | TypeFlags::STRING_MAPPING,
                        )
                    } else if checker.is_pattern_literal_type(subject)?
                        && !checker.model.maybe_type_of_kind(
                            literals,
                            TypeFlags::STRING
                                | TypeFlags::TEMPLATE_LITERAL
                                | TypeFlags::STRING_MAPPING,
                        )
                    {
                        Some(TypeFlags::STRING_LITERAL)
                    } else if checker.model.has_type_flags(subject, TypeFlags::NUMBER) {
                        Some(TypeFlags::NUMBER | TypeFlags::NUMBER_LITERAL)
                    } else if checker.model.has_type_flags(subject, TypeFlags::BIGINT) {
                        Some(TypeFlags::BIGINT | TypeFlags::BIGINT_LITERAL)
                    } else {
                        None
                    };
                    if let Some(mask) = mask {
                        Ok(checker.model.extract_types_of_kind(literals, mask))
                    } else {
                        Ok(subject)
                    }
                })();
                result.map(NativeCompletion::clean)
            }))
        })();
        sequence.finish(result)
    }

    /// `isExhaustiveSwitchStatement`: reentry installs clean native False.
    /// A completed outer invocation returns its own acquired evidence but does
    /// not retroactively attach it to that reentrant installation. An interrupted
    /// suffix likewise preserves a value already installed by reentry.
    pub(super) fn exhaustive_switch_completion(
        &mut self,
        file: FileId,
        statement: Site,
        location: &Location,
    ) -> NativeResult<bool> {
        let key = key(file, statement);
        match self.reads.switches.exhaustive.get(&key).cloned() {
            Some(Exhaustive::Ready(value)) => return Ok(value),
            Some(Exhaustive::Computing) => {
                let completed = NativeCompletion::clean(false);
                self.reads
                    .switches
                    .exhaustive
                    .insert(key, Exhaustive::Ready(completed.clone()));
                return Ok(completed);
            }
            None => {}
        }
        self.reads
            .switches
            .exhaustive
            .insert(key, Exhaustive::Computing);
        match self.compute_exhaustive_switch_completion(file, statement, location) {
            Ok(completed) => {
                if let Some(Exhaustive::Ready(installed)) = self.reads.switches.exhaustive.get(&key)
                {
                    let value = *installed.value();
                    return Ok(completed.map(|_| value));
                }
                self.reads
                    .switches
                    .exhaustive
                    .insert(key, Exhaustive::Ready(completed.clone()));
                Ok(completed)
            }
            // An interruption returns no native boolean and ends this query.
            // Leave Computing or a reentrant False exactly as already written;
            // it neither installs a product-only failure slot nor resets state.
            Err(error) => Err(error),
        }
    }

    /// `computeExhaustiveSwitchStatement`: typeof witnesses precede operand
    /// checking; ordinary literal eligibility precedes all demanded case types.
    fn compute_exhaustive_switch_completion(
        &mut self,
        file: FileId,
        statement: Site,
        location: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let expression = self.source.switch_syntax(file, statement)?.expression;
            let syntax = self.source.flow_syntax(file, expression)?;
            if syntax.kind == SyntaxKind::TypeOfExpression {
                let Some(witnesses) = self.switch_witnesses(file, statement)? else {
                    return Ok(false);
                };
                let operand = sequence.read(self.check_expression_cached_completion(
                    file,
                    required(syntax.expression, "typeof switch has no operand")?,
                ))?;
                let operand =
                    match sequence.read(self.base_constraint_completion(operand, location))? {
                        Some(base) => base,
                        None => operand,
                    };
                let facts = Self::switch_not_equal(0, 0, &witnesses);
                if self
                    .model
                    .has_type_flags(operand, TypeFlags::ANY | TypeFlags::UNKNOWN)
                {
                    return Ok(facts.includes(TypeFacts::ALL_TYPEOF_NOT_EQUAL));
                }
                let subjects = self.model.union_constituents(operand);
                for &subject in subjects
                    .as_deref()
                    .unwrap_or(std::slice::from_ref(&operand))
                {
                    if sequence.read(self.type_facts_completion(subject, facts, location))? == facts
                    {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            let ty = sequence.read(self.check_expression_cached_completion(file, expression))?;
            let ty = match sequence.read(self.base_constraint_completion(ty, location))? {
                Some(base) => base,
                None => ty,
            };
            if !self.model.is_literal_type(ty) {
                return Ok(false);
            }
            let types = sequence.read(self.switch_clause_types_completion(file, statement))?;
            if types.is_empty()
                || types.iter().any(|&ty| {
                    !self
                        .model
                        .has_type_flags(ty, TypeFlags::UNIT | TypeFlags::NEVER)
                })
            {
                return Ok(false);
            }
            let ty = sequence.read(self.map_type_completion(ty, location, |checker, ty| {
                Ok(NativeCompletion::clean(checker.model.regular_literal(ty)))
            }))?;
            let subjects = self.model.union_constituents(ty);
            Ok(subjects
                .as_deref()
                .unwrap_or(std::slice::from_ref(&ty))
                .iter()
                .all(|ty| types.contains(ty)))
        })();
        sequence.finish(result)
    }

    fn switch_union_completion(
        &mut self,
        types: Vec<TypeId>,
        location: &Location,
    ) -> NativeResult<TypeId> {
        self.union_type_ex_completion(types, None, None, location)
    }
}
