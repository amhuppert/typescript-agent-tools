//! Native reference marking retains its acquisition schedule even when the
//! query does not emit JavaScript. Alias marking can resolve another alias.
use super::*;
use crate::project::binding::NativeSymbolIdentity;
use crate::project::semantic::evaluation::{ReferenceDeclaration, ReferenceSymbol};
use crate::project::semantic::model::{NativeResult, NativeSequence};
use std::collections::HashSet;

#[derive(Default)]
pub(in crate::project::semantic::evaluation) struct AccessReferencesState {
    aliases: HashSet<NativeSymbolIdentity>,
    properties: HashSet<NativeSymbolIdentity>,
}

impl Evaluator<'_, '_> {
    /// `markLinkedReferences(Property)` with the supplied native parent type.
    /// The nil-parent-type replay is a different caller and is not substituted
    /// here: checked access already acquired its receiver and chosen property.
    pub(super) fn mark_linked_property_references_completion(
        &mut self,
        root: ExpressionRoot,
        property: Option<PropertySymbolId>,
        parent_type: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.inputs.semantic_options().verbatim_module_syntax {
                return Ok(());
            }
            let Some(context) = self.source.property_alias_reference(root)? else {
                return Ok(());
            };
            let parent = sequence.read(
                self.reference_resolved_identifier_symbol_completion(context.left, consumer),
            )?;
            if self.is_unknown_reference_symbol(parent) {
                return Ok(());
            }
            if self.inputs.semantic_options().isolated_modules
                || self.inputs.semantic_options().preserve_const_enums
                    && self.source.is_export_or_export_expression(root)?
            {
                return sequence
                    .read(self.mark_alias_referenced_completion(parent, root, consumer));
            }
            if self.model.has_type_flags(parent_type, TypeFlags::ANY)
                || parent_type == self.model.silent_never()
            {
                return sequence
                    .read(self.mark_alias_referenced_completion(parent, root, consumer));
            }
            if let Some(property) = property {
                let symbol = ReferenceSymbol::from_property(property);
                if self.is_const_enum_or_const_enum_only_module(symbol, consumer)?
                    || self.reference_symbol_flags(symbol, consumer)?.enum_member
                        && context.enum_member_parent
                {
                    return Ok(());
                }
            }
            sequence.read(self.mark_alias_referenced_completion(parent, root, consumer))
        })();
        sequence.finish(result)
    }

    /// `markAliasReferenced`: Value is the native exclusion mask here, unlike
    /// resolveSymbol's wider Value|Type|Namespace non-local-alias question.
    fn mark_alias_referenced_completion(
        &mut self,
        symbol: ReferenceSymbol,
        location: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.inputs.semantic_options().verbatim_module_syntax {
                return Ok(());
            }
            let flags = self.reference_symbol_flags(symbol, consumer)?;
            if !flags.alias
                || !flags.assignment && flags.value
                || self.source.reference_is_in_type_query(location)?
            {
                return Ok(());
            }
            let target = sequence
                .read(self.resolve_alias_target_completion(symbol, consumer))?
                .ok_or_else(|| {
                    super::super::super::internal("referenced alias resolved native nil")
                })?;
            let flags = sequence
                .read(self.reference_symbol_flags_ex_completion(symbol, true, false, consumer))?;
            if flags.value || flags.export_value {
                let mark = if self.inputs.semantic_options().isolated_modules
                    || self.inputs.semantic_options().preserve_const_enums
                        && self.source.is_export_or_export_expression(location)?
                {
                    true
                } else {
                    let target =
                        self.reference_export_symbol_of_value_if_exported(target, consumer)?;
                    !self.is_const_enum_or_const_enum_only_module(target, consumer)?
                };
                if mark {
                    sequence
                        .read(self.mark_alias_symbol_as_referenced_completion(symbol, consumer))?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `markAliasSymbolAsReferenced` installs the referenced bit before it
    /// follows an internal import-equals chain, preserving the recursion guard.
    fn mark_alias_symbol_as_referenced_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let identity = self.reference_symbol_identity(symbol, consumer)?;
            if !self.access_references.aliases.insert(identity) {
                return Ok(());
            }
            let declaration = self
                .reference_alias_declaration(symbol, consumer)?
                .ok_or_else(|| {
                    super::super::super::internal("referenced alias has no native declaration")
                })?;
            let ReferenceDeclaration::Captured(declaration) = declaration else {
                return Ok(());
            };
            let Some(first) = self
                .source
                .internal_alias_reference_identifier(declaration)?
            else {
                return Ok(());
            };
            let target = sequence
                .read(self.resolve_reference_symbol_completion(symbol, consumer))?
                .ok_or_else(|| super::super::super::internal("referenced alias target is nil"))?;
            if sequence
                .read(self.reference_combined_symbol_flags_completion(target, consumer))?
                .value
            {
                let parent = sequence
                    .read(self.reference_resolved_identifier_symbol_completion(first, consumer))?;
                if !self.is_unknown_reference_symbol(parent)
                    && !self.source.reference_is_in_type_query(first)?
                {
                    sequence
                        .read(self.mark_alias_referenced_completion(parent, first, consumer))?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `isSelfTypeAccess` intentionally may resolve the first identifier again;
    /// it does not derive self access from the receiver type's symbol.
    pub(super) fn is_self_type_access_completion(
        &mut self,
        left: ExpressionRoot,
        parent: Option<ReferenceSymbol>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if left.site.kind == SyntaxKind::ThisKeyword {
                return Ok(true);
            }
            let Some(parent) = parent else {
                return Ok(false);
            };
            let Some(first) = self.source.self_type_access_identifier(left)? else {
                return Ok(false);
            };
            let resolved = sequence
                .read(self.reference_resolved_identifier_symbol_completion(first, consumer))?;
            Ok(self.reference_symbol_identity(parent, consumer)?
                == self.reference_symbol_identity(resolved, consumer)?)
        })();
        sequence.finish(result)
    }

    /// `markPropertyAsReferenced`: source predicates precede target-link access;
    /// recursive self method references do not mark their own declaration.
    pub(super) fn mark_property_as_referenced(
        &mut self,
        property: PropertySymbolId,
        recipe: Option<&AccessRecipe>,
        self_access: bool,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        let flags = self.property_symbol_flags(property)?;
        if !(flags.property || flags.method || flags.get_accessor || flags.set_accessor) {
            return Ok(());
        }
        let Some(ReferenceDeclaration::Captured(declaration)) = self
            .reference_symbol_value_declaration(
                ReferenceSymbol::from_property(property),
                consumer,
            )?
        else {
            return Ok(());
        };
        let private_modifier = self
            .source
            .declaration_has_modifier(declaration, SyntaxKind::PrivateKeyword)?;
        let private_name = self
            .source
            .private_identifier_declaration_name(declaration)?
            .is_some();
        if !private_modifier && !private_name {
            return Ok(());
        }
        if recipe.is_some_and(|recipe| recipe.access_kind == crate::syntax::AccessKind::Write)
            && !flags.set_accessor
        {
            return Ok(());
        }
        if self_access
            && let Some(recipe) = recipe
            && let Some(function) = self
                .source
                .property_reference_containing_function(recipe.root)?
            && let Some(symbol) = self.reference_raw_symbol_of_declaration(function)?
            && self.reference_symbol_identity(symbol, consumer)?
                == self
                    .reference_symbol_identity(ReferenceSymbol::from_property(property), consumer)?
        {
            return Ok(());
        }
        let target = self.property_target_symbol(property)?;
        let identity =
            self.reference_symbol_identity(ReferenceSymbol::from_property(target), consumer)?;
        self.access_references.properties.insert(identity);
        Ok(())
    }
}
