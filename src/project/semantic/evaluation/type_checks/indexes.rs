//! Native index declaration checks consume actual IndexInfo identities.
use super::super::super::model::{
    Index, NativeCompletion, NativeResult, NativeSequence, ObjectFlags, PropertySymbolId,
    TypeFlags, TypeId, TypeView,
};
use super::super::super::source::{DeclarationIdentity, SignatureRoot};
use super::super::{Evaluator, relations::RelationKind};
use super::super::{ReferenceDeclaration, ReferenceSymbol};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    /// `checkGrammarIndexSignatureParameters`: syntax gates precede the key;
    /// literal/generic rejection precedes the every-union-key classification.
    pub(super) fn check_index_signature_grammar_completion(
        &mut self,
        root: SignatureRoot,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(key) = self.source.index_signature_grammar_key(root)? else {
                return Ok(());
            };
            let ty = sequence.read(self.check_type_completion(&key))?;
            let parts = match self.model.get(ty)? {
                TypeView::Union(parts) => parts.to_vec(),
                _ => vec![ty],
            };
            if parts.iter().any(|&part| {
                self.model.has_type_flags(
                    part,
                    TypeFlags::STRING_LITERAL
                        | TypeFlags::NUMBER_LITERAL
                        | TypeFlags::UNIQUE_SYMBOL,
                )
            }) || self.is_generic_type(ty, consumer)?
            {
                return Ok(());
            }
            for part in parts {
                if !self.is_valid_index_key_type(part, consumer)? {
                    break;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `getApplicableIndexInfos` is an ordered filter, not the singular
    /// findApplicableIndexInfo operation that combines overlapping values.
    fn applicable_index_infos_completion(
        &mut self,
        ty: TypeId,
        key: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<Index>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let indexes = self.index_infos_of_type(ty, consumer)?;
            let mut result = Vec::new();
            for info in indexes.iter() {
                let index_key = sequence.read(self.demand_completion(info.key_type))?;
                if self.is_applicable_index_type(key, index_key, consumer)? {
                    result.push(info.clone());
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `checkIndexConstraints`: no indexes means no property value reads.
    /// Literal names precede nonmissing values, including private names whose
    /// helper later returns. Static Prototype is the sole early property skip.
    pub(super) fn check_index_constraints_completion(
        &mut self,
        ty: TypeId,
        symbol: impl Into<ReferenceSymbol>,
        is_static: bool,
        consumer: &Location,
    ) -> NativeResult<()> {
        let symbol = symbol.into();
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let indexes = self.index_infos_of_type(ty, consumer)?;
            if indexes.is_empty() {
                return Ok(());
            }
            for property in self.properties_of_object_type(ty, consumer)?.iter() {
                let property = self.property_symbol(property, consumer)?;
                if is_static && self.property_symbol_flags(property)?.prototype {
                    continue;
                }
                let name = self.literal_type_from_property(
                    property,
                    TypeFlags::STRING_LITERAL
                        | TypeFlags::NUMBER_LITERAL
                        | TypeFlags::UNIQUE_SYMBOL,
                    true,
                    consumer,
                )?;
                let value =
                    sequence.read(self.non_missing_property_type_completion(property, consumer))?;
                sequence.read(self.check_index_constraint_for_property_completion(
                    ty, symbol, property, name, value, consumer,
                ))?;
            }
            if let Some(ReferenceDeclaration::Captured(declaration)) =
                self.reference_symbol_value_declaration(symbol, consumer)?
            {
                if matches!(
                    self.source.declaration_kind(declaration)?,
                    K::ClassDeclaration | K::ClassExpression
                ) {
                    for member in self.source.declaration_members(declaration)? {
                        let declaration = self.source.type_node_declaration(member)?;
                        if (self.source.declaration_modifiers(declaration)?
                            & crate::syntax::modifiers::STATIC
                            != 0)
                            != is_static
                            || self.has_bindable_declaration_name(declaration)?
                        {
                            continue;
                        }
                        let member_symbol = sequence
                            .read(self.reference_symbol_of_declaration_completion(declaration))?;
                        let property = self.register_reference_symbol(member_symbol, consumer)?;
                        let name = self
                            .source
                            .declaration_name_binding(declaration)?
                            .name
                            .ok_or_else(|| {
                                super::super::super::internal("unbindable class member has no name")
                            })?;
                        let recipe = self.source.property_name_recipe(member.file, name)?;
                        let super::super::super::source::PropertyNameForm::Expression {
                            expression,
                            ..
                        } = recipe.form
                        else {
                            return Err(super::super::super::internal(
                                "unbindable class member lacks computed expression",
                            )
                            .into());
                        };
                        let name = sequence.read(self.check_expression_completion(
                            &expression,
                            super::super::contextual::CheckMode::NORMAL,
                        ))?;
                        let value = sequence
                            .read(self.non_missing_property_type_completion(property, consumer))?;
                        sequence.read(self.check_index_constraint_for_property_completion(
                            ty, symbol, property, name, value, consumer,
                        ))?;
                    }
                }
            }
            if indexes.len() > 1 {
                for index in indexes.iter() {
                    sequence.read(self.check_index_constraint_for_index_completion(
                        ty, symbol, index, consumer,
                    ))?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    fn index_declaration_is_local_completion(
        &mut self,
        declaration: Option<DeclarationIdentity>,
        container: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let Some(declaration) = declaration else {
            return Ok(NativeCompletion::clean(false));
        };
        self.reference_symbol_of_declaration_completion(declaration)?
            .and_then(|symbol| self.index_symbol_parent_is_completion(symbol, container, consumer))
    }

    fn index_symbol_parent_is_completion(
        &mut self,
        symbol: ReferenceSymbol,
        container: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.reference_parent_of_symbol_completion(symbol, consumer)?
            .and_then_value(|parent| match parent {
                Some(parent) => Ok(self.reference_symbol_identity(parent, consumer)?
                    == self.reference_symbol_identity(container, consumer)?),
                None => Ok(false),
            })
    }

    /// `checkIndexConstraintForProperty`: local declaration precedence avoids
    /// base queries; only Interface invokes the separate-inherited-base gate.
    fn check_index_constraint_for_property_completion(
        &mut self,
        ty: TypeId,
        container: ReferenceSymbol,
        property: PropertySymbolId,
        name: TypeId,
        value: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let declaration = self.property_reference_value_declaration(property, consumer)?;
            if let Some(declaration) = declaration
                && self.reference_declaration_name_is_private(declaration)?
            {
                return Ok(());
            }
            let indexes =
                sequence.read(self.applicable_index_infos_completion(ty, name, consumer))?;
            if indexes.is_empty() {
                return Ok(());
            }
            let interface = self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::INTERFACE);
            let local_property = declaration.is_some()
                && sequence.read(self.index_symbol_parent_is_completion(
                    ReferenceSymbol::from_property(property),
                    container,
                    consumer,
                ))?;
            for info in indexes {
                // Native acquires this local index question even if the property
                // already provided an error node.
                let local_index = sequence.read(self.index_declaration_is_local_completion(
                    info.declaration,
                    container,
                    consumer,
                ))?;
                let mut check = local_property || local_index;
                if !check && interface {
                    let facts = self.property_symbol_name_facts(property, consumer)?;
                    let name = facts.key;
                    check = true;
                    for base in self.base_types(ty, consumer)?.iter().copied() {
                        if self
                            .property_of_object_type_by_key(base, &name, consumer)?
                            .is_some()
                        {
                            let key = sequence.read(self.demand_completion(info.key_type))?;
                            if self.index_type_of_type(base, key, consumer)?.is_some() {
                                check = false;
                                break;
                            }
                        }
                    }
                }
                if check {
                    let target = sequence.read(self.demand_completion(info.value_type))?;
                    if !sequence.read(self.type_related_completion(
                        value,
                        target,
                        RelationKind::Assignable,
                        consumer,
                    ))? {
                        sequence
                            .read(self.check_diagnostic_type_display_completion(value, consumer))?;
                        let key = sequence.read(self.demand_completion(info.key_type))?;
                        sequence
                            .read(self.check_diagnostic_type_display_completion(key, consumer))?;
                        sequence.read(
                            self.check_diagnostic_type_display_completion(target, consumer),
                        )?;
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkIndexConstraintForIndexSignature` skips only actual IndexInfo
    /// identity; equal key/value records still participate as distinct infos.
    fn check_index_constraint_for_index_completion(
        &mut self,
        ty: TypeId,
        container: ReferenceSymbol,
        check_info: &Index,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let key = sequence.read(self.demand_completion(check_info.key_type))?;
            let indexes =
                sequence.read(self.applicable_index_infos_completion(ty, key, consumer))?;
            if indexes.is_empty() {
                return Ok(());
            }
            let interface = self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::INTERFACE);
            let local_check = sequence.read(self.index_declaration_is_local_completion(
                check_info.declaration,
                container,
                consumer,
            ))?;
            for info in indexes {
                if info.same_identity(check_info) {
                    continue;
                }
                let local_index = sequence.read(self.index_declaration_is_local_completion(
                    info.declaration,
                    container,
                    consumer,
                ))?;
                let mut check = local_check || local_index;
                if !check && interface {
                    check = true;
                    for base in self.base_types(ty, consumer)?.iter().copied() {
                        if self.index_info_of_type(base, key, consumer)?.is_some() {
                            let index_key = sequence.read(self.demand_completion(info.key_type))?;
                            if self
                                .index_type_of_type(base, index_key, consumer)?
                                .is_some()
                            {
                                check = false;
                                break;
                            }
                        }
                    }
                }
                if check {
                    let value = sequence.read(self.demand_completion(check_info.value_type))?;
                    let target = sequence.read(self.demand_completion(info.value_type))?;
                    if !sequence.read(self.type_related_completion(
                        value,
                        target,
                        RelationKind::Assignable,
                        consumer,
                    ))? {
                        sequence
                            .read(self.check_diagnostic_type_display_completion(key, consumer))?;
                        sequence
                            .read(self.check_diagnostic_type_display_completion(value, consumer))?;
                        let index_key = sequence.read(self.demand_completion(info.key_type))?;
                        sequence.read(
                            self.check_diagnostic_type_display_completion(index_key, consumer),
                        )?;
                        sequence.read(
                            self.check_diagnostic_type_display_completion(target, consumer),
                        )?;
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkTypeForDuplicateIndexSignatures` uses the original reserved
    /// index symbol's declaration list, never the deduplicated member inventory.
    pub(super) fn check_type_for_duplicate_index_signatures_completion(
        &mut self,
        symbol: impl Into<ReferenceSymbol>,
        consumer: &Location,
    ) -> NativeResult<()> {
        let symbol = symbol.into();
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(symbol) = sequence.read(self.index_symbol_completion(symbol, consumer))?
            else {
                return Ok(());
            };
            let declarations = self
                .reference_symbol_declarations(symbol, consumer)?
                .into_iter()
                .filter_map(|declaration| match declaration {
                    super::super::ReferenceDeclaration::Captured(node) => Some(node),
                    super::super::ReferenceDeclaration::Json { .. } => None,
                })
                .collect::<Vec<_>>();
            if declarations.len() <= 1 {
                return Ok(());
            }
            let mut keys: Vec<(TypeId, usize)> = Vec::new();
            for declaration in declarations {
                let Some(node) = self.source.duplicate_index_key(declaration)? else {
                    continue;
                };
                let key = sequence.read(self.check_type_completion(&node))?;
                let parts = match self.model.get(key)? {
                    TypeView::Union(parts) => parts.to_vec(),
                    _ => vec![key],
                };
                for part in parts {
                    if let Some((_, count)) = keys.iter_mut().find(|(key, _)| *key == part) {
                        *count += 1;
                    } else {
                        keys.push((part, 1));
                    }
                }
            }
            for (key, count) in keys {
                if count > 1 {
                    for _ in 0..count {
                        sequence
                            .read(self.check_diagnostic_type_display_completion(key, consumer))?;
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }
}
