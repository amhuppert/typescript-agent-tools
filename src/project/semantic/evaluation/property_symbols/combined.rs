//! A named import can combine a value property with a supplemental exported
//! type. The resulting symbol has its own identity and lazy declaration worker.
use super::*;

impl Evaluator<'_, '_> {
    /// `combineValueAndTypeSymbols`: shortcuts preserve the actual input
    /// identity. The fresh case copies type Members and value Exports, keeps
    /// the value declaration, and only removes adjacent duplicate declarations.
    pub(in crate::project::semantic::evaluation) fn combine_value_and_type_symbols(
        &mut self,
        value: ReferenceSymbol,
        type_: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<ReferenceSymbol, DemandError> {
        if self.is_unknown_reference_symbol(value) && self.is_unknown_reference_symbol(type_) {
            return Ok(self.unknown_reference_symbol());
        }
        if self.reference_symbol_flags(type_, consumer)?.value {
            return Ok(type_);
        }
        if self
            .reference_symbol_flags(value, consumer)?
            .type_or_namespace
        {
            return Ok(value);
        }
        let flags = self
            .reference_symbol_native_flags(value, consumer)?
            .union(self.reference_symbol_native_flags(type_, consumer)?);
        let key = self.reference_symbol_key(value, consumer)?;
        let identity = self.model.new_symbol_identity();
        let mut declarations = self.reference_symbol_declarations(value, consumer)?;
        declarations.extend(self.reference_symbol_declarations(type_, consumer)?);
        declarations.dedup();
        let first = declarations
            .first()
            .copied()
            .ok_or_else(|| super::super::super::internal("combined symbol has no declarations"))?;
        let ordering = self.reference_declaration_ordering(first)?;
        let parent = match self.reference_symbol_parent(value, consumer)? {
            Some(parent) => Some(parent),
            None => self.reference_symbol_parent(type_, consumer)?,
        };
        let value_declaration = self.reference_symbol_value_declaration(value, consumer)?;
        let raw_members = self
            .reference_raw_symbol_table(type_, false)?
            .map(|table| self.clone_reference_raw_table(table, consumer))
            .transpose()?;
        let raw_exports = self
            .reference_raw_symbol_table(value, true)?
            .map(|table| self.clone_reference_raw_table(table, consumer))
            .transpose()?;
        let symbol = self.install_created_property(CreatedProperty {
            native_flags: Some(flags),
            flags: PropertySymbolFlags::default(),
            checks: 0,
            declarations: declarations.into_iter().map(Into::into).collect(),
            value_declaration: value_declaration.map(Into::into),
            parent,
            raw_members,
            raw_exports,
            name_type: None,
            header: NativeSymbol::created_with_key(identity, key, Some(ordering)),
            mapper: None,
            target: None,
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: CreatedPropertyValue::Declaration,
        });
        // Installation is private until return. Reuse the native flag owner;
        // no caller can observe a partial header or an invented value cache.
        self.set_merge_symbol_flags(symbol, flags)?;
        Ok(ReferenceSymbol::from_property(symbol))
    }
}
