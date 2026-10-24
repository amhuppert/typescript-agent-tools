//! Enum declared identities and source constant slots share native publication.
use super::super::{
    constants::{ConstantResult, ConstantValue},
    model::{DemandError, NativeCompletion, NativeResult, NativeSequence, TypeFlags, TypeId},
    source::{DeclarationIdentity, ExpressionRoot},
};
use super::{
    Evaluator, ReferenceDeclaration, ReferenceSymbol, TypeState,
    contextual::CheckMode,
    relations::{RelationKind, RelationReporting},
};
use crate::{
    project::binding::BoundDeclaration, references::Location, syntax_kind::SyntaxKind as K,
    types::PrimitiveName,
};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct EnumsState {
    computed: HashSet<DeclarationIdentity>,
    values: HashMap<DeclarationIdentity, NativeCompletion<ConstantResult>>,
    checked: HashSet<crate::project::binding::NativeSymbolIdentity>,
}

impl Evaluator<'_, '_> {
    /// `isConstEnumOrConstEnumOnlyModule`: the second flag belongs to the
    /// native symbol header, including its binder/merge state transitions.
    pub(super) fn is_const_enum_or_const_enum_only_module(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        if self.reference_symbol_flags(symbol, consumer)?.const_enum {
            return Ok(true);
        }
        self.reference_symbol_const_enum_only_module(symbol, consumer)
    }
    /// `isTypeSubsetOf` / `isTypeSubsetOfUnion`: exact constituent identity,
    /// including the actual declared parent of a non-union enum member.
    pub(super) fn type_subset_of_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if source == target || self.model.has_type_flags(source, TypeFlags::NEVER) {
            return Ok(NativeCompletion::clean(true));
        }
        let Some(targets) = self.model.union_constituents(target) else {
            return Ok(NativeCompletion::clean(false));
        };
        if let Some(sources) = self.model.union_constituents(source) {
            return Ok(NativeCompletion::clean(
                sources.iter().all(|source| targets.contains(source)),
            ));
        }
        if self.is_enum_like(source) {
            return Ok(self
                .base_type_of_enum_like_completion(source, consumer)?
                .map(|base| base == target || targets.contains(&source)));
        }
        Ok(NativeCompletion::clean(targets.contains(&source)))
    }
    /// `isConstEnumObjectType`: exact anonymous object and symbol flag gate.
    pub(super) fn is_const_enum_object_type(
        &self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        if !self
            .model
            .object_flags(ty)?
            .intersects(super::super::model::ObjectFlags::ANONYMOUS)
        {
            return Ok(false);
        }
        Ok(match self.reference_symbol_of_type(ty, consumer)? {
            Some(symbol) => self.reference_symbol_flags(symbol, consumer)?.const_enum,
            None => false,
        })
    }
    /// `checkEnumDeclaration` checks every member before constant evaluation.
    /// Its grammar/export/merged-const diagnostics consume declaration facts
    /// only; the separate value operation retains its native computed flag.
    pub(super) fn check_enum_declaration_completion(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> NativeResult<()> {
        let members = self.source.enum_declaration_recipe(declaration)?.members;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for &member in &members {
                let root = self.source.declaration_expression_root(member)?;
                sequence.read(self.check_type_source_element_completion(root))?;
            }
            if let Some(&member) = members.first() {
                // `computeEnumMemberValues` does not read the member slot on
                // a warm computed flag. The cold getter invokes that whole
                // worker and projects the already-installed first value.
                if !self.enums.computed.contains(&declaration) {
                    sequence.read(self.enum_member_value_completion(member))?;
                }
            } else {
                self.enums.computed.insert(declaration);
            }
            // `getSymbolOfDeclaration`: enums declared in several global
            // scripts share one merged symbol, enumChecked and declaration list.
            let symbol =
                sequence.read(self.reference_symbol_of_declaration_completion(declaration))?;
            let consumer = self.source.declaration_location(declaration)?;
            let identity = self.reference_symbol_identity(symbol, &consumer)?;
            // Native publishes enumChecked before examining merged const
            // modifiers and the first initializer of each declaration.
            if self.enums.checked.insert(identity) {
                let declarations = self.reference_symbol_declarations(symbol, &consumer)?;
                if declarations.len() > 1 {
                    self.source.enum_declaration_recipe(declaration)?;
                    for node in &declarations {
                        if let ReferenceDeclaration::Captured(node) = node
                            && self.source.declaration_kind(*node)? == K::EnumDeclaration
                        {
                            self.source.enum_declaration_recipe(*node)?;
                        }
                    }
                }
                for node in declarations {
                    if let ReferenceDeclaration::Captured(node) = node
                        && self.source.declaration_kind(node)? == K::EnumDeclaration
                        && let Some(&first) =
                            self.source.enum_declaration_recipe(node)?.members.first()
                    {
                        self.source.enum_member_recipe(first)?;
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkEnumMember` always checks an initializer, including ambient and
    /// const members whose constant-value operation deliberately skips it.
    pub(super) fn check_enum_member_completion(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> NativeResult<()> {
        let Some(initializer) = self.source.enum_member_recipe(declaration)?.initializer else {
            return Ok(NativeCompletion::clean(()));
        };
        Ok(self
            .check_expression_completion(&initializer, CheckMode::NORMAL)?
            .map(|_| ()))
    }
    /// `formatUnionTypes` coalesces complete boolean/enum groups and moves
    /// canonical nullable types to the end. Enum parent reads are semantic
    /// acquisitions, so display and Q4 share this evaluator owner.
    pub(in crate::project::semantic) fn formatted_union_types_completion(
        &mut self,
        types: &[TypeId],
        expanding_enum: bool,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut result = Vec::new();
            let mut flags = TypeFlags::NONE;
            let mut index = 0;
            while index < types.len() {
                let ty = types[index];
                let current = self.model.type_flags(ty);
                flags = flags | current;
                if !current.intersects(TypeFlags::NULLABLE) {
                    if current.intersects(TypeFlags::BOOLEAN_LITERAL)
                        || !expanding_enum && current.intersects(TypeFlags::ENUM_LIKE)
                    {
                        let base = if current.intersects(TypeFlags::BOOLEAN_LITERAL) {
                            self.model.primitive(PrimitiveName::Boolean)
                        } else {
                            sequence.read(self.base_type_of_enum_like_completion(ty, consumer))?
                        };
                        if let Some(parts) = self.model.union_constituents(base) {
                            let count = parts.len();
                            if index + count <= types.len()
                                && self.model.regular_literal(types[index + count - 1])
                                    == self.model.regular_literal(parts[count - 1])
                            {
                                result.push(base);
                                index += count;
                                continue;
                            }
                        }
                    }
                    result.push(ty);
                }
                index += 1;
            }
            if flags.intersects(TypeFlags::NULL) {
                result.push(self.model.special(super::super::model::SpecialName::Null));
            }
            if flags.intersects(TypeFlags::UNDEFINED) {
                result.push(
                    self.model
                        .special(super::super::model::SpecialName::Undefined),
                );
            }
            Ok(result)
        })();
        sequence.finish(result)
    }
    pub(in crate::project::semantic) fn is_enum_like(&self, ty: TypeId) -> bool {
        self.model.has_type_flags(ty, TypeFlags::ENUM_LIKE)
    }
    pub(in crate::project::semantic) fn enum_display_summary(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Vec<u8>, DemandError> {
        Ok(self
            .diagnostic_type_display_completion(
                ty,
                None,
                super::native_display::DiagnosticTypeFormat::DEFAULT,
                consumer,
            )?
            .admit()?
            .into_bytes())
    }
    /// `getDeclaredTypeOfEnum`: each member's declared slot is published before
    /// the final enum union, using actual member and enum symbol identities.
    pub(super) fn declared_enum_type_completion(
        &mut self,
        symbol: ReferenceSymbol,
        first: BoundDeclaration,
    ) -> NativeResult<TypeId> {
        let consumer = super::super::location(self.inputs, first.file, first.anchor);
        let identity = self.reference_symbol_identity(symbol, &consumer)?;
        if let Some(TypeState::Installed(ty)) = self.declared_types.get(&identity) {
            return Ok(ty.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut types = Vec::new();
            let declarations = self.reference_symbol_declarations(symbol, &consumer)?;
            for declaration in declarations {
                let ReferenceDeclaration::Captured(declaration) = declaration else {
                    continue;
                };
                if self.source.declaration_kind(declaration)? != K::EnumDeclaration {
                    continue;
                }
                for member in self.source.enum_declaration_recipe(declaration)?.members {
                    if !sequence.read(self.has_bindable_declaration_name_completion(member))? {
                        continue;
                    }
                    let member_symbol =
                        sequence.read(self.reference_symbol_of_declaration_completion(member))?;
                    let member_identity =
                        self.reference_symbol_identity(member_symbol, &consumer)?;
                    let value = self.enum_member_value_completion(member)?;
                    let type_symbol = self.type_symbol_of_reference(member_symbol, &consumer)?;
                    let member_type = match &value.value().value {
                        Some(ConstantValue::Number(value)) => self.model.enum_literal_type(
                            identity,
                            type_symbol,
                            PrimitiveName::Number,
                            super::super::model::LiteralValue::from_text(
                                crate::numbers::number_to_string(*value),
                            ),
                        ),
                        Some(ConstantValue::String(value)) => self.model.enum_literal_type(
                            identity,
                            type_symbol,
                            PrimitiveName::String,
                            value.clone(),
                        ),
                        None => self.model.computed_enum_type(type_symbol),
                    };
                    let fresh = self.model.fresh_literal(member_type);
                    // This slot copies the native result of this actual value
                    // acquisition, never the preceding members' evidence.
                    self.declared_types.insert(
                        member_identity,
                        TypeState::Installed(value.clone().map(|_| fresh)),
                    );
                    sequence.read(Ok(value))?;
                    types.push(self.model.regular_literal(member_type));
                }
            }
            let type_symbol = self.type_symbol_of_reference(symbol, &consumer)?;
            let ty = if types.is_empty() {
                self.model.computed_enum_type(type_symbol.clone())
            } else {
                let alias = self.establish_alias_symbol(symbol, &consumer)?;
                sequence.read(self.union_type_ex_completion(types, Some(alias), None, &consumer))?
            };
            self.model.attach_enum_union_symbol(ty, type_symbol);
            Ok(ty)
        })();
        let result = sequence.finish(result)?;
        self.declared_types
            .insert(identity, TypeState::Installed(result.clone()));
        Ok(result)
    }

    /// `getDeclaredTypeOfEnumMember` preserves a member slot installed by the
    /// parent enumeration instead of replaying that slot after the cold call.
    pub(super) fn declared_enum_member_type_completion(
        &mut self,
        symbol: ReferenceSymbol,
        first: BoundDeclaration,
    ) -> NativeResult<TypeId> {
        let consumer = super::super::location(self.inputs, first.file, first.anchor);
        let identity = self.reference_symbol_identity(symbol, &consumer)?;
        if let Some(TypeState::Installed(ty)) = self.declared_types.get(&identity) {
            return Ok(ty.clone());
        }
        let parent = self
            .reference_parent_of_symbol(symbol, &consumer)?
            .ok_or_else(|| super::super::internal("enum member has no actual parent symbol"))?;
        let value = self.declared_reference_type_completion(parent, &consumer)?;
        if let Some(TypeState::Installed(existing)) = self.declared_types.get(&identity) {
            return Ok(value.map(|_| *existing.value()));
        }
        self.declared_types
            .insert(identity, TypeState::Installed(value.clone()));
        Ok(value)
    }

    /// `getEnumMemberValue` / `computeEnumMemberValues` publish the computed
    /// flag before visiting children. Reentry reads the actual zero-initialized
    /// result slot; an interrupted calculation cannot refund that publication.
    pub(super) fn enum_member_value_completion(
        &mut self,
        member: DeclarationIdentity,
    ) -> NativeResult<ConstantResult> {
        let recipe = self.source.enum_member_recipe(member)?;
        if !self.enums.computed.insert(recipe.parent) {
            return Ok(self
                .enums
                .values
                .get(&member)
                .cloned()
                .unwrap_or_else(|| NativeCompletion::clean(ConstantResult::default())));
        }
        let declaration = self.source.enum_declaration_recipe(recipe.parent)?;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut next = Some(0.0);
            let mut previous = None;
            for child in declaration.members {
                let value = self.compute_enum_member_value_completion(
                    child,
                    declaration.ambient,
                    declaration.constant,
                    next,
                    previous,
                )?;
                self.enums.values.insert(child, value.clone());
                let value = sequence.read(Ok(value))?;
                next = match value.value {
                    Some(ConstantValue::Number(number)) => Some(number + 1.0),
                    _ => None,
                };
                previous = Some(child);
            }
            Ok(self
                .enums
                .values
                .get(&member)
                .map(|value| value.value().clone())
                .unwrap_or_default())
        })();
        sequence.finish(result)
    }

    /// `computeEnumMemberValue` and `computeConstantEnumMemberValue` retain
    /// ordinary diagnostics as diagnostics: only acquired semantic dependencies
    /// can taint the constant result. A computed member has native nil value.
    fn compute_enum_member_value_completion(
        &mut self,
        member: DeclarationIdentity,
        ambient: bool,
        constant: bool,
        auto: Option<f64>,
        previous: Option<DeclarationIdentity>,
    ) -> NativeResult<ConstantResult> {
        let recipe = self.source.enum_member_recipe(member)?;
        let consumer = self.source.declaration_location(member)?;
        if let Some(initializer) = recipe.initializer {
            let root = initializer
                .root
                .ok_or_else(|| super::super::internal("enum initializer lacks actual node"))?;
            let location = self.source.declaration_expression_root(member)?;
            return self
                .evaluate_constant_completion(root, Some(location), &consumer)?
                .and_then(|value| {
                    if value.value.is_none() && !constant && !ambient {
                        return self
                            .check_expression_completion(&initializer, CheckMode::NORMAL)?
                            .and_then(|ty| {
                                let number = self.model.primitive(PrimitiveName::Number);
                                Ok(self
                                    .check_type_related_to_completion(
                                        ty,
                                        number,
                                        RelationKind::Assignable,
                                        RelationReporting::Report.computed_enum_at(root),
                                        &initializer.location,
                                    )?
                                    .map(|_| value))
                            });
                    }
                    Ok(NativeCompletion::clean(value))
                });
        }
        if ambient && !constant || auto.is_none() {
            return Ok(NativeCompletion::clean(ConstantResult::default()));
        }
        let result = ConstantResult {
            value: auto.map(ConstantValue::Number),
            ..Default::default()
        };
        if self.inputs.semantic_options().isolated_modules
            && let Some(previous) = previous
            && self
                .source
                .enum_member_recipe(previous)?
                .initializer
                .is_some()
        {
            return Ok(self.enum_member_value_completion(previous)?.map(|_| result));
        }
        Ok(NativeCompletion::clean(result))
    }

    /// `evaluateEnumMember` distinguishes self-reference, a later declaration
    /// (native numeric zero), and a successfully acquired constant slot.
    pub(super) fn evaluate_enum_member_completion(
        &mut self,
        symbol: ReferenceSymbol,
        location: Option<ExpressionRoot>,
        consumer: &Location,
    ) -> NativeResult<ConstantResult> {
        let declaration = self.reference_symbol_value_declaration(symbol, consumer)?;
        let Some(ReferenceDeclaration::Captured(declaration)) = declaration else {
            return Ok(self
                .diagnostic_symbol_display_completion(symbol, consumer)?
                .map(|_| ConstantResult::default()));
        };
        if let Some(location) = location {
            if self.source.declaration_expression_root(declaration)? == location {
                return Ok(self
                    .diagnostic_symbol_display_completion(symbol, consumer)?
                    .map(|_| ConstantResult::default()));
            }
        }
        let before_use = match location {
            Some(location) => self.block_scoped_name_declared_before_use_completion(
                declaration,
                location,
                consumer,
            )?,
            None => NativeCompletion::clean(true),
        };
        before_use.and_then(|before| {
            if !before {
                return Ok(NativeCompletion::clean(ConstantResult {
                    value: Some(ConstantValue::Number(0.0)),
                    ..Default::default()
                }));
            }
            self.enum_member_value_completion(declaration)?
                .and_then_value(|mut result| {
                    if let Some(location) = location {
                        let location = self.source.declaration_from_expression_root(location)?;
                        result.external_references |= self.source.declaration_parent(location)?
                            != self.source.declaration_parent(declaration)?;
                    }
                    Ok(result)
                })
        })
    }

    /// `getBaseTypeOfEnumLikeType` asks the actual member's parent declared
    /// type, keeping enum identity distinct from primitive literal widening.
    pub(super) fn base_type_of_enum_like_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self.model.has_type_flags(ty, TypeFlags::ENUM_LIKE) {
            let symbol = self
                .reference_symbol_of_type(ty, consumer)?
                .ok_or_else(|| super::super::internal("enum type has no symbol"))?;
            if self.reference_symbol_flags(symbol, consumer)?.enum_member {
                let parent = self
                    .reference_parent_of_symbol(symbol, consumer)?
                    .ok_or_else(|| super::super::internal("enum member has no parent"))?;
                return self.declared_reference_type_completion(parent, consumer);
            }
        }
        Ok(NativeCompletion::clean(ty))
    }
}
