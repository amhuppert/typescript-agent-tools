//! Native late and merge factories. Actual flags/declarations and fresh cache
//! identity are independent from the declaration-backed lazy value recipe.
use super::super::reference_symbols::ReferenceRawSymbolTable;
use super::*;
use crate::project::binding::NativeSymbolFlags;

impl Evaluator<'_, '_> {
    pub(in crate::project::semantic::evaluation) fn reference_symbol_native_flags(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<NativeSymbolFlags, DemandError> {
        self.reference_symbol_native_flags_at(symbol, Some(consumer))
    }
    pub(in crate::project::semantic::evaluation) fn reference_symbol_native_flags_at(
        &self,
        symbol: ReferenceSymbol,
        _consumer: Option<&Location>,
    ) -> Result<NativeSymbolFlags, DemandError> {
        if let Some(origin) = self.reference_raw_origin(symbol)? {
            return Ok(self.source.bindings().native_symbol_flags(origin.symbol()));
        }
        let property = symbol
            .property()
            .ok_or_else(|| super::super::super::internal("symbol has no native header"))?;
        let target = self.property_target_symbol(property)?;
        if let Association::Own(bound) = self.property_association(target)? {
            return Ok(self.source.bindings().native_symbol_flags(bound));
        }
        let data = self.created_property(target)?;
        Ok(data.native_flags.unwrap_or_else(|| {
            NativeSymbolFlags::from_factory(data.flags.type_flags(), data.flags.native_flags())
        }))
    }

    fn merge_property_mut(
        &mut self,
        symbol: PropertySymbolId,
    ) -> Result<&mut CreatedProperty, DemandError> {
        let entry = self
            .native_members
            .symbols
            .entries
            .get_mut(symbol.index())
            .ok_or_else(|| super::super::super::internal("merge symbol is absent"))?;
        match &mut entry.association {
            Association::Created(data) => Ok(CreatedProperty::make_mut(data)),
            _ => Err(super::super::super::internal(
                "retained symbol cannot be mutated by checker merge",
            )
            .into()),
        }
    }

    /// `newSymbolEx(None, name, Late)`: no target, Parent, declaration or
    /// value is invented at allocation. lateBindMember installs them in order.
    pub(in crate::project::semantic::evaluation) fn new_late_symbol(
        &mut self,
        key: NativePropertyKey,
    ) -> PropertySymbolId {
        let header = NativeSymbol::created_with_key(self.model.new_symbol_identity(), key, None);
        self.install_created_property(CreatedProperty {
            native_flags: Some(NativeSymbolFlags::default()),
            flags: PropertySymbolFlags::default(),
            checks: checks::LATE,
            declarations: Vec::new(),
            value_declaration: None,
            parent: None,
            raw_members: None,
            raw_exports: None,
            name_type: None,
            header,
            mapper: None,
            target: None,
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: CreatedPropertyValue::Declaration,
        })
    }

    /// Native `initializeTypeCaches` intrinsic headers are declarationless and
    /// transient; their actual value links are installed by initializeChecker.
    pub(in crate::project::semantic::evaluation) fn new_intrinsic_reference_symbol(
        &mut self,
        name: &str,
        flags: NativeSymbolFlags,
        readonly: bool,
    ) -> Result<ReferenceSymbol, DemandError> {
        let symbol = self.new_late_symbol(NativePropertyKey::fixed(name.as_bytes()));
        let property = flags.property_flags();
        let data = self.merge_property_mut(symbol)?;
        data.native_flags = Some(flags);
        data.flags = PropertySymbolFlags {
            types: flags.type_flags(),
            variable: property.variable,
            property: property.property,
            method: property.method,
            get_accessor: property.getter,
            set_accessor: property.setter,
            optional: property.optional,
            prototype: property.prototype,
            partial: false,
        };
        data.checks = if readonly { checks::READONLY } else { 0 };
        Ok(ReferenceSymbol::from_property(symbol))
    }

    /// `cloneSymbol` copies only native header fields, not SymbolLinks.target
    /// or cached types. Each nonnil raw table gets a separate stable slot;
    /// later merges mutate that clone's slot and publish each completed key.
    pub(in crate::project::semantic::evaluation) fn clone_reference_symbol(
        &mut self,
        source: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<PropertySymbolId, DemandError> {
        let registered = self.register_reference_symbol(source, consumer)?;
        let flags = self.property_symbol_flags(registered)?;
        let native_flags = self.reference_symbol_native_flags(source, consumer)?;
        let header = self
            .property_native_symbol(registered, consumer)?
            .with_identity(self.model.new_symbol_identity());
        let declarations = self.reference_header_declarations(source, Some(consumer))?;
        let value_declaration = self.reference_header_value_declaration(source, Some(consumer))?;
        let parent = self.reference_symbol_parent(source, consumer)?;
        let raw_members = self
            .reference_raw_symbol_table(source, false)?
            .map(|table| self.clone_reference_raw_table(table, consumer))
            .transpose()?;
        let raw_exports = self
            .reference_raw_symbol_table(source, true)?
            .map(|table| self.clone_reference_raw_table(table, consumer))
            .transpose()?;
        let symbol = self.install_created_property(CreatedProperty {
            native_flags: Some(native_flags),
            flags,
            checks: 0,
            declarations,
            value_declaration,
            parent,
            raw_members,
            raw_exports,
            name_type: None,
            header,
            mapper: None,
            target: None,
            synthetic_origin: None,
            spread: None,
            export_type: None,
            value: CreatedPropertyValue::Declaration,
        });
        let projection = self.reference_symbol_projection(source)?;
        let source = self.reference_symbol_identity(source, consumer)?;
        let identity =
            self.reference_symbol_identity(ReferenceSymbol::from_property(symbol), consumer)?;
        if let Some(projection) = projection {
            self.symbol_tables.projections.insert(identity, projection);
        }
        self.symbol_tables
            .merged
            .insert(source, ReferenceSymbol::from_property(symbol));
        Ok(symbol)
    }

    /// `mergeSymbol` mutates an already transient target in place. Retained
    /// transient headers acquire a request-local copy with the same identity.
    pub(in crate::project::semantic::evaluation) fn mutable_reference_symbol(
        &mut self,
        source: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<PropertySymbolId, DemandError> {
        let registered = self.register_reference_symbol(source, consumer)?;
        if matches!(
            self.property_association(registered)?,
            Association::Created(_)
        ) {
            return Ok(registered);
        }
        let flags = self.property_symbol_flags(registered)?;
        let native_flags = self.reference_symbol_native_flags(source, consumer)?;
        let header = self.property_native_symbol(registered, consumer)?;
        let declarations = self.reference_header_declarations(source, Some(consumer))?;
        let value_declaration = self.reference_header_value_declaration(source, Some(consumer))?;
        let parent = self.reference_symbol_parent(source, consumer)?;
        let raw_members = self.reference_raw_symbol_table(source, false)?;
        let raw_exports = self.reference_raw_symbol_table(source, true)?;
        let identity = header.identity();
        if let Some(origin) = self.reference_raw_origin(source)? {
            let bound = origin.symbol();
            let entry = &mut self.native_members.symbols.entries[registered.index()];
            entry.read = self.value_types.get(&bound).cloned();
            entry.write = self.native_members.writes.get(&bound).cloned();
        }
        self.native_members.symbols.entries[registered.index()].association =
            Association::Created(Arc::new(CreatedProperty {
                native_flags: Some(native_flags),
                flags,
                checks: 0,
                declarations,
                value_declaration,
                parent,
                raw_members,
                raw_exports,
                name_type: None,
                header,
                mapper: None,
                target: None,
                synthetic_origin: None,
                spread: None,
                export_type: None,
                value: CreatedPropertyValue::Declaration,
            }));
        self.symbol_tables.headers.insert(identity, registered);
        Ok(registered)
    }

    pub(in crate::project::semantic::evaluation) fn set_merge_symbol_flags(
        &mut self,
        symbol: PropertySymbolId,
        flags: NativeSymbolFlags,
    ) -> Result<(), DemandError> {
        let property = flags.property_flags();
        let data = self.merge_property_mut(symbol)?;
        data.native_flags = Some(flags);
        data.flags = PropertySymbolFlags {
            types: flags.type_flags(),
            variable: property.variable,
            property: property.property,
            method: property.method,
            get_accessor: property.getter,
            set_accessor: property.setter,
            optional: property.optional,
            prototype: property.prototype,
            partial: false,
        };
        Ok(())
    }

    pub(in crate::project::semantic::evaluation) fn mark_symbol_late(
        &mut self,
        symbol: PropertySymbolId,
    ) -> Result<(), DemandError> {
        self.merge_property_mut(symbol)?.checks |= checks::LATE;
        Ok(())
    }
    pub(in crate::project::semantic::evaluation) fn set_late_symbol_name_type(
        &mut self,
        symbol: PropertySymbolId,
        ty: TypeId,
    ) -> Result<(), DemandError> {
        self.merge_property_mut(symbol)?.name_type = Some(ty);
        Ok(())
    }
    pub(in crate::project::semantic::evaluation) fn set_merge_symbol_parent(
        &mut self,
        symbol: PropertySymbolId,
        parent: ReferenceSymbol,
    ) -> Result<(), DemandError> {
        let parent =
            match self.reference_raw_origin(parent)? {
                Some(origin) => self
                    .source
                    .bindings()
                    .checker_symbol_parent(origin.symbol())
                    .into(),
                None => SymbolParent::Created(parent.property().ok_or_else(|| {
                    super::super::super::internal("parent lacks an actual symbol")
                })?),
            };
        self.merge_property_mut(symbol)?.parent = Some(parent);
        Ok(())
    }

    /// `SetValueDeclaration` uses the shared binder/checker precedence rule,
    /// but retains this fresh symbol's own actual declaration reference.
    pub(in crate::project::semantic::evaluation) fn set_merge_value_declaration(
        &mut self,
        symbol: PropertySymbolId,
        next: impl Into<super::super::declarations::DeclarationAssociation>,
        _consumer: &Location,
    ) -> Result<(), DemandError> {
        let next = next.into();
        let old = self
            .property_header_value_declaration(symbol, None)?
            .map(|old| self.header_declaration_kind(old))
            .transpose()?;
        let next_kind = self.header_declaration_kind(next)?;
        if crate::project::binding::should_replace_value_declaration(old, next_kind) {
            self.merge_property_mut(symbol)?.value_declaration = Some(next);
        }
        Ok(())
    }

    /// `addDeclarationToLateBoundSymbol` / `mergeSymbol` retain the next native declaration.
    pub(in crate::project::semantic::evaluation) fn append_merge_declaration(
        &mut self,
        symbol: PropertySymbolId,
        declaration: impl Into<super::super::declarations::DeclarationAssociation>,
    ) -> Result<(), DemandError> {
        let declaration = declaration.into();
        let first = self.created_property(symbol)?.declarations.is_empty();
        let ordering = if first {
            Some(match declaration {
                super::super::declarations::DeclarationAssociation::Raw(raw) => self
                    .source
                    .bindings()
                    .raw_declaration_facts(raw)
                    .map(|facts| {
                        Some(crate::project::binding::NativeDeclaration {
                            file: facts.file,
                            span: facts.span,
                        })
                    }),
                super::super::declarations::DeclarationAssociation::Attached(
                    ReferenceDeclaration::Captured(node),
                ) => {
                    let (_, span) = self.source.observed_declaration_site(node)?;
                    Ok(Some(crate::project::binding::NativeDeclaration {
                        file: node.file,
                        span,
                    }))
                }
                super::super::declarations::DeclarationAssociation::Attached(
                    ReferenceDeclaration::Json { .. },
                ) => Ok(None),
            })
        } else {
            None
        };

        let data = self.merge_property_mut(symbol)?;
        if let Some(ordering) = ordering {
            data.header = data.header.with_declaration(ordering);
        }

        data.declarations.push(declaration);
        Ok(())
    }

    pub(in crate::project::semantic::evaluation) fn set_merge_raw_table(
        &mut self,
        symbol: PropertySymbolId,
        exports: bool,
        table: Option<ReferenceRawSymbolTable>,
    ) -> Result<(), DemandError> {
        let data = self.merge_property_mut(symbol)?;
        if exports {
            data.raw_exports = table;
        } else {
            data.raw_members = table;
        }
        Ok(())
    }
}
