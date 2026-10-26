//! Object literals own member checking, contextual lifetimes and fresh types;
//! symbol factories, spread algebra and discriminator filtering remain shared.

use super::super::model::{
    DemandError, MembersView, NativeCompletion, NativeResult, NativeSequence, ObjectFlags,
    PropertySymbolId, SpecialName, TypeFlags, TypeId, TypeSymbol,
};
use super::super::source::{
    ExpressionRoot,
    object_literals::{ObjectLiteralRecipe, ObjectMember},
};
use super::name_map::NameMap;
use super::{
    Evaluator, ReferenceSymbol,
    contextual::{CheckMode, ContextFlags},
    relations::{RelationKind, RelationReporting},
    structure::MemberTable,
};
use crate::project::binding::NativePropertyKey;
use crate::{
    analysis::SemanticFailureReason, references::Location, syntax_kind::SyntaxKind as K,
    types::PrimitiveName,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub(super) struct ObjectLiteralsState {
    discriminated: HashMap<(ExpressionRoot, TypeId), NativeResult<TypeId>>,
}

struct ObjectMembers {
    table: Vec<(NativePropertyKey, PropertySymbolId)>,
    positions: NameMap<usize>,

    array: Vec<PropertySymbolId>,
    computed_string: bool,
    computed_number: bool,
    computed_symbol: bool,
}
impl ObjectMembers {
    fn new() -> Self {
        Self {
            table: Vec::new(),
            positions: NameMap::new(),

            array: Vec::new(),
            computed_string: false,
            computed_number: false,
            computed_symbol: false,
        }
    }
    /// `checkObjectLiteral`'s symbol table replaces values at the first name
    /// position. The index avoids repeatedly scanning prior exact names.
    fn insert(&mut self, name: NativePropertyKey, symbol: PropertySymbolId) {
        if let Some(&index) = self.positions.get(&name) {
            self.table[index].1 = symbol;
        } else {
            self.positions.insert(name.clone(), self.table.len());
            self.table.push((name, symbol));
        }
    }
    fn snapshot(&self) -> Arc<[(NativePropertyKey, PropertySymbolId)]> {
        let mut table = Vec::new();
        for (name, symbol) in &self.table {
            table.push((name.clone(), *symbol));
        }
        std::sync::Arc::from(table)
    }
}

impl Evaluator<'_, '_> {
    /// `checkObjectLiteral`: grammar's effective-name reads precede the
    /// cached contextual frame. All computed names are checked before the
    /// member loop, and the frame is popped before the final spread/type.
    pub(super) fn check_object_literal(
        &mut self,
        root: ExpressionRoot,
        mode: CheckMode,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        let recipe = self.source.object_literal_recipe(root)?;
        let symbol = self.reference_raw_symbol_of_declaration(recipe.declaration)?;
        if recipe.members.is_empty()
            && let Some(symbol) = symbol
            && let Some(exports) = self.reference_raw_symbol_table(symbol, true)?
        {
            let exports = self.reference_raw_table_entries(exports, consumer)?;
            if !exports.is_empty() {
                let mut members = Vec::new();
                for (name, symbol) in exports.iter() {
                    members.push((
                        name.clone(),
                        self.register_reference_symbol(*symbol, consumer)?,
                    ));
                }
                let symbol = self.type_symbol_of_reference(symbol, consumer)?;
                return self.new_resolved_anonymous_type(
                    symbol,
                    Some(MemberTable::Symbols(members.into())),
                    empty_members(),
                    ObjectFlags::NONE,
                    consumer,
                );
            }
        }
        self.check_object_literal_grammar(&recipe)?;
        let symbol = match symbol {
            Some(symbol) => self.type_symbol_of_reference(symbol, consumer)?,
            None => TypeSymbol::Absent,
        };
        let (mut members, spread, flags, constant, mut failure) = self
            .with_cached_contextual_type(root, consumer, |checker| {
                checker.object_literal_members(root, &recipe, symbol.clone(), mode, consumer)
            })?;
        let result = (|| {
            if self.model.is_error_type(spread) {
                return Ok(spread);
            }
            if spread != self.model.empty_object_type() {
                let spread = if !members.array.is_empty() {
                    let current = self.object_literal_type(
                        root,
                        &recipe,
                        &members,
                        symbol.clone(),
                        flags,
                        consumer,
                    )?;
                    let spread = self.spread_type(
                        spread,
                        current,
                        symbol.clone(),
                        flags,
                        constant,
                        consumer,
                    )?;
                    let symbol_index = members.computed_symbol;
                    members = ObjectMembers::new();
                    members.computed_symbol = symbol_index;
                    spread
                } else {
                    spread
                };
                return self.map_type(spread, consumer, |checker, ty| {
                    if ty == checker.model.empty_object_type() {
                        checker.object_literal_type(
                            root,
                            &recipe,
                            &members,
                            symbol.clone(),
                            flags,
                            consumer,
                        )
                    } else {
                        Ok(ty)
                    }
                });
            }
            self.object_literal_type(root, &recipe, &members, symbol, flags, consumer)
        })();
        match (failure.take(), result) {
            (Some(primary), Err(later)) => Err(primary.append_failure(later)),
            (Some(primary), Ok(_)) => Err(primary),
            (None, result) => result,
        }
    }

    /// `checkGrammarObjectLiteralExpression`: the native grammar result is
    /// diagnostic-only, but its early return controls later effective-name
    /// reads before the cached contextual frame is installed.
    fn check_object_literal_grammar(
        &mut self,
        recipe: &ObjectLiteralRecipe,
    ) -> Result<(), DemandError> {
        let mut seen = NameMap::<u8>::new();
        for member in &recipe.members {
            if member.stops_grammar {
                break;
            }
            if member.kind == K::SpreadAssignment || recipe.destructuring {
                continue;
            }
            let name = member
                .name
                .ok_or_else(|| super::super::internal("object grammar member has no name"))?;
            let key = self.effective_property_name_completion(name)?.admit()?;
            let Some(key) = key else {
                continue;
            };
            let current = match member.kind {
                K::PropertyAssignment | K::ShorthandPropertyAssignment => 1,
                K::MethodDeclaration => 2,
                K::GetAccessor => 4,
                K::SetAccessor => 8,
                _ => return Err(super::super::internal("unexpected object grammar member").into()),
            };
            match seen.get(&key).copied() {
                None => {
                    seen.insert(key, current);
                }
                Some(previous)
                    if current == 1 && previous == 1 || current == 2 && previous == 2 => {}
                Some(previous) if current & 12 != 0 && previous & 12 != 0 => {
                    if previous != 12 && current != previous {
                        seen.insert(key, previous | current);
                    } else {
                        break;
                    }
                }
                Some(_) => break,
            }
        }
        Ok(())
    }

    fn object_literal_members(
        &mut self,
        root: ExpressionRoot,
        recipe: &ObjectLiteralRecipe,
        symbol: TypeSymbol,
        mode: CheckMode,
        consumer: &Location,
    ) -> Result<
        (
            ObjectMembers,
            TypeId,
            ObjectFlags,
            bool,
            Option<DemandError>,
        ),
        DemandError,
    > {
        let contextual = self.apparent_contextual_type(root, ContextFlags::NONE, consumer)?;
        let contextual_pattern = match contextual {
            Some(ty)
                if self.object_context_has_pattern(ty)?
                    && !self.model.object_flags(ty)?.intersects(
                        ObjectFlags::OBJECT_LITERAL_PATTERN_WITH_COMPUTED_PROPERTIES,
                    ) =>
            {
                Some(ty)
            }
            _ => None,
        };
        let constant = self.is_const_context(root, consumer)?;
        let mut flags = ObjectFlags::FRESH_LITERAL;
        let mut members = ObjectMembers::new();
        let mut all_properties = self
            .inputs
            .semantic_options()
            .strict_null_checks
            .then(NameMap::new);
        let mut spread = self.model.empty_object_type();
        let mut failure: Option<DemandError> = None;
        for member in &recipe.members {
            if let Some(name) = member.computed {
                self.check_object_computed_name(name)?;
            }
        }
        let result: Result<(), DemandError> = (|| {
            for member in &recipe.members {
                // SpreadAssignment has an actual nil symbol. Ask the same raw
                // optional attachment instead of inventing a property for it.
                let raw = if member.kind == K::SpreadAssignment {
                    self.reference_node_declaration_symbol(member.declaration)?
                } else {
                    Some(self.reference_symbol_of_declaration(member.declaration)?)
                };
                let computed = member
                    .computed
                    .map(|name| self.check_object_computed_name(name))
                    .transpose()?;
                if member.kind == K::SpreadAssignment {
                    if !members.array.is_empty() {
                        let current = self.object_literal_type(
                            root,
                            recipe,
                            &members,
                            symbol.clone(),
                            flags,
                            consumer,
                        )?;
                        spread = self.spread_type(
                            spread,
                            current,
                            symbol.clone(),
                            flags,
                            constant,
                            consumer,
                        )?;
                        members = ObjectMembers::new();
                    }
                    let operand = member
                        .initializer
                        .ok_or_else(|| super::super::internal("object spread has no expression"))?;
                    let expression = self.source.expression_at(operand.file, operand.site)?;
                    let ty = self.check_expression_ex(
                        &expression,
                        if mode.contains(CheckMode::INFERENTIAL) {
                            CheckMode::INFERENTIAL
                        } else {
                            CheckMode::NORMAL
                        },
                    )?;
                    let ty = self.reduced_type(ty, &expression.location)?;
                    if self.is_valid_spread_type(ty, &expression.location)? {
                        let merged =
                            self.merge_object_and_empty_union(ty, constant, &expression.location)?;
                        if let Some(properties) = &all_properties {
                            self.check_spread_property_overrides(merged, properties, consumer)?;
                        }
                        if !self.model.is_error_type(spread) {
                            spread = self.spread_type(
                                spread,
                                merged,
                                symbol.clone(),
                                flags,
                                constant,
                                consumer,
                            )?;
                        }
                    } else {
                        let error = super::super::invalid_at_location(
                            self.inputs,
                            &expression.location,
                            SemanticFailureReason::ErrorType,
                            "spread types may only be created from object types",
                        )
                        .into();
                        failure = Some(match failure.take() {
                            Some(prior) => prior.append_failure(error),
                            None => error,
                        });
                        spread = self.model.native_error_type();
                    }
                    continue;
                }
                let raw = raw
                    .ok_or_else(|| super::super::internal("object member has no native symbol"))?;
                let raw = self.register_reference_symbol(raw, consumer)?;
                let checked = if matches!(
                    member.kind,
                    K::PropertyAssignment | K::ShorthandPropertyAssignment | K::MethodDeclaration
                ) {
                    let ty = self.check_object_member_value(
                        member,
                        recipe.destructuring,
                        mode,
                        consumer,
                    )?;
                    flags = flags | self.model.propagating_flags(&[ty], TypeFlags::NONE);
                    let name_type = computed.filter(|&ty| {
                        self.model.has_type_flags(
                            ty,
                            TypeFlags::STRING_LITERAL
                                | TypeFlags::NUMBER_LITERAL
                                | TypeFlags::UNIQUE_SYMBOL,
                        )
                    });
                    let prop = self.create_object_literal_property(
                        raw,
                        NativeCompletion::clean(ty),
                        super::property_symbols::ObjectLiteralPropertyOptions {
                            readonly: constant,
                            name_type,
                            default_optional: recipe.destructuring && member.default_value,
                            contextual_pattern,
                        },
                        consumer,
                    )?;
                    if let Some(properties) = &mut all_properties {
                        properties.insert(
                            self.reference_symbol_key(
                                ReferenceSymbol::from_property(prop),
                                consumer,
                            )?,
                            prop,
                        );
                    }
                    if contextual.is_some()
                        && mode.contains(CheckMode::INFERENTIAL)
                        && !mode.contains(CheckMode::SKIP_CONTEXT_SENSITIVE)
                        && matches!(member.kind, K::PropertyAssignment | K::MethodDeclaration)
                        && self.source.expression_is_context_sensitive(member.root)?
                    {
                        let context = self.inference_context_for(root)?.ok_or_else(|| {
                            super::super::internal("inferential object has no inference context")
                        })?;
                        let node = if member.kind == K::PropertyAssignment {
                            member.initializer.ok_or_else(|| {
                                super::super::internal("property has no initializer")
                            })?
                        } else {
                            member.root
                        };
                        self.add_intra_expression_inference_site(context, node, ty);
                    }
                    prop
                } else {
                    // checkNodeDeferred only queues accessors for a later whole
                    // source-file diagnostics pass; the expression question does
                    // not check their signatures or bodies here.
                    if !matches!(member.kind, K::GetAccessor | K::SetAccessor) {
                        return Err(
                            super::super::internal("unexpected object literal member").into()
                        );
                    }
                    raw
                };
                if let Some(computed) = computed.filter(|&ty| {
                    !self.model.has_type_flags(
                        ty,
                        TypeFlags::STRING_LITERAL
                            | TypeFlags::NUMBER_LITERAL
                            | TypeFlags::UNIQUE_SYMBOL,
                    )
                }) {
                    let keys = self.model.string_number_symbol_type();
                    if self.type_related(computed, keys, RelationKind::Assignable, consumer)? {
                        let number = self.model.primitive(PrimitiveName::Number);
                        if self.type_related(
                            computed,
                            number,
                            RelationKind::Assignable,
                            consumer,
                        )? {
                            members.computed_number = true;
                        } else {
                            let symbol = self.model.primitive(PrimitiveName::Symbol);
                            if self.type_related(
                                computed,
                                symbol,
                                RelationKind::Assignable,
                                consumer,
                            )? {
                                members.computed_symbol = true;
                            } else {
                                members.computed_string = true;
                            }
                        }
                        if recipe.destructuring {
                            flags = flags
                                | ObjectFlags::OBJECT_LITERAL_PATTERN_WITH_COMPUTED_PROPERTIES;
                        }
                    }
                } else {
                    members.insert(
                        self.reference_symbol_key(
                            ReferenceSymbol::from_property(checked),
                            consumer,
                        )?,
                        checked,
                    );
                }
                members.array.push(checked);
            }
            Ok(())
        })();
        if let Err(error) = result {
            return Err(match failure {
                Some(primary) => primary.append_failure(error),
                None => error,
            });
        }
        Ok((members, spread, flags, constant, failure))
    }

    fn check_object_computed_name(&mut self, root: ExpressionRoot) -> Result<TypeId, DemandError> {
        let expression = self.source.expression_at(root.file, root.site)?;
        self.computed_property_name_type(&expression, false, &expression.location)
    }

    /// `checkPropertyAssignment`, `checkShorthandPropertyAssignment`, and
    /// `checkObjectLiteralMethod` preserve the repeated computed-name check,
    /// actual shorthand expression selection and post-value annotation.
    fn check_object_member_value(
        &mut self,
        member: &ObjectMember,
        destructuring: bool,
        mode: CheckMode,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.check_object_member_value_completion(member, destructuring, mode, consumer)?
            .admit()
    }

    pub(super) fn check_object_member_value_completion(
        &mut self,
        member: &ObjectMember,
        destructuring: bool,
        mode: CheckMode,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if matches!(member.kind, K::PropertyAssignment | K::MethodDeclaration)
                && let Some(name) = member.computed
            {
                let expression = self.source.expression_at(name.file, name.site)?;
                sequence.read(self.computed_property_name_type_completion(
                    &expression,
                    false,
                    &expression.location,
                ))?;
            }
            if member.kind == K::MethodDeclaration {
                let ty = sequence.read(self.check_function_expression_completion(
                    member.root,
                    mode,
                    consumer,
                ))?;
                return self.instantiate_expression_signature(
                    Some(member.root.into()),
                    ty,
                    mode,
                    consumer,
                );
            }
            let expression = if member.kind == K::ShorthandPropertyAssignment {
                if destructuring {
                    member.name
                } else {
                    member.initializer.or(member.name)
                }
            } else {
                member.initializer
            }
            .ok_or_else(|| {
                super::super::internal("object property has no checked value expression")
            })?;
            let value = self
                .source
                .expression_at(expression.file, expression.site)?;
            let ty = sequence
                .read(self.check_expression_for_mutable_location_completion(&value, mode))?;
            if member.annotation.is_some() {
                let node = self
                    .source
                    .declaration_type_annotation(member.declaration)?
                    .ok_or_else(|| {
                        super::super::internal("object property annotation disappeared")
                    })?;
                let target = sequence.read(self.check_type_completion(&node))?;
                sequence.read(
                    self.check_type_related_to_and_optionally_elaborate_completion(
                        ty,
                        target,
                        RelationKind::Assignable,
                        RelationReporting::Report.assignment_at(member.root),
                        Some(expression),
                        consumer,
                    ),
                )?;
                return Ok(target);
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    fn object_literal_type(
        &mut self,
        root: ExpressionRoot,
        recipe: &ObjectLiteralRecipe,
        members: &ObjectMembers,
        symbol: TypeSymbol,
        flags: ObjectFlags,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        let readonly = self.is_const_context(root, consumer)?;
        let mut indexes = Vec::new();
        for (enabled, kind) in [
            (members.computed_string, PrimitiveName::String),
            (members.computed_number, PrimitiveName::Number),
            (members.computed_symbol, PrimitiveName::Symbol),
        ] {
            if enabled {
                let key = self.model.primitive(kind);
                indexes.push(self.object_literal_index_info(
                    readonly,
                    &members.array,
                    key,
                    consumer,
                )?);
            }
        }
        let mut inventory = empty_members();
        inventory.index_signatures = indexes.into();
        let ty = self.new_resolved_anonymous_type(
            symbol,
            Some(MemberTable::Symbols(members.snapshot())),
            inventory,
            flags | ObjectFlags::OBJECT_LITERAL | ObjectFlags::CONTAINS_OBJECT_OR_ARRAY_LITERAL,
            consumer,
        )?;
        if recipe.destructuring {
            self.record_pattern_type(ty, recipe.declaration);
        }
        Ok(ty)
    }

    /// `checkSpreadPropOverrides` consumes the complete right inventory and
    /// native flags; diagnostic formatting here reads only established names.
    fn check_spread_property_overrides(
        &mut self,
        ty: TypeId,
        properties: &NameMap<PropertySymbolId>,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        for property in self.properties_of_type(ty, consumer)? {
            let symbol = self.property_symbol(&property, consumer)?;
            let flags = self.property_symbol_flags(symbol)?;
            if !flags.optional && !flags.partial {
                let name =
                    self.reference_symbol_key(ReferenceSymbol::from_property(symbol), consumer)?;
                let _overridden = properties.get(&name);
            }
        }
        Ok(())
    }

    /// `discriminateContextualTypeByObjectMembers` caches the complete
    /// question by actual node and contextual type. Key-map selection comes
    /// first; fallback discriminants keep expression acquisition lazy inside
    /// each native target comparison.
    pub(super) fn discriminate_contextual_object_completion(
        &mut self,
        root: ExpressionRoot,
        contextual: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let key = (root, contextual);
        if let Some(result) = self.object_literals.discriminated.get(&key) {
            return result.clone();
        }
        let result =
            self.discriminate_contextual_object_worker_completion(root, contextual, consumer);
        self.object_literals
            .discriminated
            .insert(key, result.clone());
        result
    }

    fn discriminate_contextual_object_worker_completion(
        &mut self,
        root: ExpressionRoot,
        contextual: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.object_literal_recipe(root)?;
            if let Some(key) =
                sequence.read(self.union_key_property_name_completion(contextual, consumer))?
            {
                for member in &recipe.members {
                    let Some(symbol) =
                        self.reference_raw_symbol_of_declaration(member.declaration)?
                    else {
                        continue;
                    };
                    if member.kind != K::PropertyAssignment {
                        continue;
                    }
                    let symbol = self.register_reference_symbol(symbol, consumer)?;
                    if self
                        .reference_symbol_key(ReferenceSymbol::from_property(symbol), consumer)?
                        != key
                    {
                        continue;
                    }
                    let initializer = member.initializer.ok_or_else(|| {
                        super::super::internal("discriminant property has no initializer")
                    })?;
                    if !self.source.possibly_discriminant_value(initializer)? {
                        continue;
                    }
                    let ty =
                        sequence.read(self.context_free_expression_type_completion(initializer))?;
                    if let Some(matched) = self.union_constituent_for_key(contextual, ty)? {
                        return Ok(matched);
                    }
                    break;
                }
            }
            let mut items: Vec<(NativePropertyKey, Option<ExpressionRoot>)> = Vec::new();
            for member in &recipe.members {
                let Some(symbol) = self.reference_raw_symbol_of_declaration(member.declaration)?
                else {
                    continue;
                };
                let value = match member.kind {
                    K::PropertyAssignment => {
                        let initializer = member.initializer.ok_or_else(|| {
                            super::super::internal("discriminant property has no initializer")
                        })?;
                        if !self.source.possibly_discriminant_value(initializer)? {
                            continue;
                        }
                        initializer
                    }
                    K::ShorthandPropertyAssignment => member.name.ok_or_else(|| {
                        super::super::internal("shorthand discriminant has no name")
                    })?,
                    _ => continue,
                };
                let symbol = self.register_reference_symbol(symbol, consumer)?;
                let name =
                    self.reference_symbol_key(ReferenceSymbol::from_property(symbol), consumer)?;
                if sequence.read(
                    self.is_discriminant_property_by_key_completion(contextual, &name, consumer),
                )? {
                    items.push((name, Some(value)));
                }
            }
            for property in
                sequence.read(self.properties_of_type_completion(contextual, consumer))?
            {
                let symbol = self.property_symbol(&property, consumer)?;
                if !self.property_symbol_flags(symbol)?.optional {
                    continue;
                }
                let key =
                    self.reference_symbol_key(ReferenceSymbol::from_property(symbol), consumer)?;
                let object = self
                    .reference_raw_symbol_of_declaration(recipe.declaration)?
                    .ok_or_else(|| super::super::internal("object literal has no raw symbol"))?;
                let present = match self.reference_raw_symbol_table(object, false)? {
                    Some(table) => self
                        .reference_raw_table_entries(table, consumer)?
                        .iter()
                        .any(|(name, _)| *name == key),
                    None => false,
                };
                if present {
                    continue;
                }
                let name =
                    self.reference_symbol_key(ReferenceSymbol::from_property(symbol), consumer)?;
                if sequence.read(
                    self.is_discriminant_property_by_key_completion(contextual, &name, consumer),
                )? {
                    items.push((name, None));
                }
            }
            sequence.read(self.discriminate_by_items_completion(
                contextual,
                items.len(),
                |_, index| Ok(items[index].0.clone()),
                |checker, index| match items[index].1 {
                    Some(root) => checker.context_free_expression_type_completion(root),
                    None => Ok(NativeCompletion::clean(
                        checker.model.special(SpecialName::Undefined),
                    )),
                },
                &mut |checker, source, target| {
                    checker.type_related_completion(
                        source,
                        target,
                        RelationKind::Assignable,
                        consumer,
                    )
                },
                consumer,
            ))
        })();
        sequence.finish(result)
    }
}

fn empty_members() -> MembersView {
    MembersView {
        properties: Arc::from([]),
        call_signatures: Arc::from([]),
        construct_signatures: Arc::from([]),
        index_signatures: Arc::from([]),
    }
}
