//! Reverse-mapped symbols keep native property/template/constraint links in
//! the same checker symbol arena; their Type slot is the ordinary read cache.
use super::*;
use crate::project::semantic::model::{SpecialName, TypeFlags, TypeView};

impl Evaluator<'_, '_> {
    /// The symbol factory in `resolveReverseMappedTypeMembers`: allocate after
    /// readonly, copy declarations/nameType, read source, then simplify links.
    pub(in crate::project::semantic::evaluation) fn create_reverse_mapped_symbol(
        &mut self,
        source: PropertySymbolId,
        mapped_type: TypeId,
        constraint_type: TypeId,
        readonly_mask: bool,
        optional_mask: bool,
        consumer: &Location,
    ) -> Result<PropertySymbolId, DemandError> {
        let readonly = readonly_mask && self.property_is_readonly(source, consumer)?;
        let flags = self.property_symbol_flags(source)?;
        let identity = self.model.new_symbol_identity();
        let header = self
            .property_native_symbol(source, consumer)?
            .with_identity(identity);
        let declarations = self.property_reference_declarations(source, consumer)?;
        let name_type = self.property_name_type(source)?;
        let property_type = self.property_raw_type(source, consumer)?;
        let TypeView::Index(constraint) = self.model.get(constraint_type)? else {
            return Err(crate::project::semantic::internal(
                "reverse mapped property constraint is not an Index",
            )
            .into());
        };
        let (mapped_type, constraint_type) = if let TypeView::IndexedAccess(access) =
            self.model.get(constraint.target)?
            && self
                .model
                .has_type_flags(access.object_type, TypeFlags::TYPE_PARAMETER)
            && self
                .model
                .has_type_flags(access.index_type, TypeFlags::TYPE_PARAMETER)
        {
            let mapped = self.replace_indexed_access(
                mapped_type,
                constraint.target,
                access.object_type,
                consumer,
            )?;
            let index = self.index_type_ex(
                access.object_type,
                crate::project::semantic::model::IndexFlags::NONE,
                consumer,
            )?;
            (mapped, index)
        } else {
            (mapped_type, constraint_type)
        };
        Ok(self.install_created_property(CreatedProperty {
            native_flags: None,
            flags: PropertySymbolFlags {
                property: true,
                optional: optional_mask && flags.optional,
                ..Default::default()
            },
            checks: checks::REVERSE_MAPPED | if readonly { checks::READONLY } else { 0 },
            declarations: declarations.into_iter().map(Into::into).collect(),
            value_declaration: None,
            parent: None,
            raw_members: None,
            raw_exports: None,
            name_type,
            header,
            mapper: None,
            target: None,
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: CreatedPropertyValue::ReverseMapped {
                property_type,
                mapped_type,
                constraint_type,
            },
        }))
    }

    /// `getTypeOfReverseMappedSymbol` has no separate resolution frame. Native
    /// nil inference becomes Unknown only here; retained failures stay failures.
    pub(super) fn reverse_mapped_property_type_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(value) = self.property_value_completion(symbol, false)? {
            return value;
        }
        let data = self.created_property(symbol)?;
        let CreatedPropertyValue::ReverseMapped {
            property_type,
            mapped_type,
            constraint_type,
        } = &data.value
        else {
            unreachable!("reverse mapped dispatch uses actual recipe");
        };
        let result = self
            .infer_reverse_mapped_type(*property_type, *mapped_type, *constraint_type, consumer)
            .map(|ty| ty.unwrap_or_else(|| self.model.special(SpecialName::Unknown)))
            .map(NativeCompletion::clean)
            .map_err(|error| error.at_property(PropertyValueQuestion::Read(symbol)));
        self.set_property_value_state(
            symbol,
            false,
            match &result {
                Ok(value) => TypeState::Installed(value.clone()),
                Err(error) => TypeState::Failed(error.clone()),
            },
        )?;
        result
    }
}

impl Evaluator<'_, '_> {
    pub(in crate::project::semantic) fn is_reverse_mapped_property(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<bool, DemandError> {
        Ok(self.property_check_flags(symbol)? & checks::REVERSE_MAPPED != 0)
    }
    pub(in crate::project::semantic) fn is_reverse_mapped_type(
        &self,
        ty: TypeId,
    ) -> Result<bool, DemandError> {
        Ok(self
            .model
            .object_flags(ty)?
            .intersects(crate::project::semantic::model::ObjectFlags::REVERSE_MAPPED))
    }
    /// `shouldUsePlaceholderForProperty` shares exact stack decisions between
    /// native diagnostics and bounded summaries. The native loop inspects four
    /// entries once depth reaches three (its break is `i > 3`, not `>= 3`).
    pub(in crate::project::semantic) fn reverse_mapped_property_placeholder(
        &self,
        symbol: PropertySymbolId,
        stack: &[PropertySymbolId],
    ) -> Result<bool, DemandError> {
        if !self.is_reverse_mapped_property(symbol)? {
            return Ok(false);
        }
        if stack.contains(&symbol) {
            return Ok(true);
        }
        if let Some(&last) = stack.last()
            && let Some((property_type, _)) = self.reverse_property_links(last)?
            && !self
                .model
                .object_flags(property_type)?
                .intersects(crate::project::semantic::model::ObjectFlags::ANONYMOUS)
        {
            return Ok(true);
        }
        if stack.len() < 3 {
            return Ok(false);
        }
        let Some((_, mapped)) = self.reverse_property_links(symbol)? else {
            return Ok(false);
        };
        let symbol = self.model.type_symbol(mapped);
        let Some(symbol) = symbol.native()? else {
            return Ok(false);
        };
        for &other in stack.iter().rev().take(4) {
            if let Some((_, mapped)) = self.reverse_property_links(other)?
                && let Some(other) = self.model.type_symbol(mapped).native()?
                && other.same_symbol(symbol)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(in crate::project::semantic::evaluation) fn reverse_property_links(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<Option<(TypeId, TypeId)>, DemandError> {
        Ok(match self.property_association(symbol)? {
            Association::Created(data) => match &data.value {
                CreatedPropertyValue::ReverseMapped {
                    property_type,
                    mapped_type,
                    ..
                } => Some((*property_type, *mapped_type)),
                _ => None,
            },
            // Native Has(symbol) does not follow an instantiated target.
            _ => None,
        })
    }
}
