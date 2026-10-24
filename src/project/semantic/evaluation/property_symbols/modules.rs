//! Native synthetic-default and module-clone symbol factories. Raw symbol
//! tables are copied independently from the resolved anonymous type's table.

use super::*;
use crate::project::semantic::evaluation::structure::MemberTable;
use crate::project::semantic::model::MembersView;
use crate::project::semantic::model::{NativeResult, NativeSequence};
use crate::project::semantic::source::ExpressionRoot;

impl Evaluator<'_, '_> {
    /// The fresh TypeLiteral symbol in getTypeWithSyntheticDefaultImportType.
    /// Its resolvedType is installed by that worker before publication.
    pub(in crate::project::semantic::evaluation) fn create_synthetic_module_type_symbol(
        &mut self,
    ) -> PropertySymbolId {
        let header = NativeSymbol::created(
            self.model.new_symbol_identity(),
            b"\xfetype".as_slice(),
            None,
        );
        self.install_created_property(CreatedProperty {
            native_flags: None,
            flags: PropertySymbolFlags {
                types: TypeSymbolFlags {
                    type_literal: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            checks: 0,
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
            value: CreatedPropertyValue::Known,
        })
    }

    /// `createDefaultPropertyWrapperForModule`: a declarationless Alias owns
    /// its preinstalled target; the new type owns the one-entry member table.
    pub(in crate::project::semantic::evaluation) fn create_default_property_wrapper_for_module_completion(
        &mut self,
        symbol: ReferenceSymbol,
        parent: Option<SymbolParent>,
        anonymous_symbol: Option<PropertySymbolId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let header = NativeSymbol::created(
                self.model.new_symbol_identity(),
                b"default".as_slice(),
                None,
            );
            let name_type = self
                .model
                .regular_literal_type(crate::types::PrimitiveName::String, "default");
            let property = self.install_created_property(CreatedProperty {
                native_flags: None,
                flags: PropertySymbolFlags {
                    types: TypeSymbolFlags {
                        alias: true,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                checks: 0,
                declarations: Vec::new(),
                value_declaration: None,
                parent,
                raw_members: None,
                raw_exports: None,
                name_type: Some(name_type),
                header,
                mapper: None,
                target: None,
                synthetic_origin: None,
                spread: None,
                export_type: None,
                value: CreatedPropertyValue::Alias(None),
            });
            let target = self
                .resolve_reference_symbol_completion(symbol, consumer)?
                .and_then_value(|target| {
                    target.ok_or_else(|| {
                        super::super::super::internal(
                            "resolveSymbol of a module wrapper's nonnil symbol returned nil",
                        )
                        .into()
                    })
                })?;
            self.install_alias_target(
                ReferenceSymbol::from_property(property),
                target.clone(),
                consumer,
            )?;
            sequence.read(Ok(target))?;
            let type_symbol = match anonymous_symbol {
                Some(symbol) => {
                    self.type_symbol_of_reference(ReferenceSymbol::from_property(symbol), consumer)?
                }
                None => TypeSymbol::Absent,
            };
            sequence.read(self.new_resolved_anonymous_type_completion(
                type_symbol,
                Some(MemberTable::Symbols(Arc::from([(
                    crate::project::binding::NativePropertyKey::fixed(b"default".as_slice()),
                    property,
                )]))),
                MembersView {
                    properties: Arc::from([]),
                    call_signatures: Arc::from([]),
                    construct_signatures: Arc::from([]),
                    index_signatures: Arc::from([]),
                },
                ObjectFlags::default(),
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `cloneTypeAsModuleType` copies raw symbol metadata before resolving
    /// moduleType, then installs an anonymous read with no call/construct list.
    pub(in crate::project::semantic::evaluation) fn clone_type_as_module_type_completion(
        &mut self,
        source: ReferenceSymbol,
        module_type: TypeId,
        originating_import: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<ReferenceSymbol> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let source_property = self.register_reference_symbol(source, consumer)?;
            let header = self
                .property_native_symbol(source_property, consumer)?
                .with_identity(self.model.new_symbol_identity());
            let mut flags = self.property_symbol_flags(source_property)?;
            flags.partial = false;
            let declarations = self.reference_symbol_declarations(source, consumer)?;
            let value_declaration = self.reference_symbol_value_declaration(source, consumer)?;
            let raw_members = self
                .reference_raw_symbol_table(source, false)?
                .map(|table| self.clone_reference_raw_table(table, consumer))
                .transpose()?;
            let raw_exports = self
                .reference_raw_symbol_table(source, true)?
                .map(|table| self.clone_reference_raw_table(table, consumer))
                .transpose()?;
            let parent = self.reference_symbol_parent(source, consumer)?;
            let symbol = self.install_created_property(CreatedProperty {
                native_flags: None,
                flags,
                checks: 0,
                declarations: declarations.into_iter().map(Into::into).collect(),
                value_declaration: value_declaration.map(Into::into),
                parent,
                raw_members,
                raw_exports,
                name_type: None,
                header,
                mapper: None,
                target: None,
                synthetic_origin: None,
                spread: None,
                export_type: Some(ExportTypeLinks {
                    originating_import,
                    target: source,
                }),
                value: CreatedPropertyValue::Known,
            });
            let resolved = sequence
                .read(self.structured_type_members_completion(module_type, consumer))?
                .ok_or_else(|| {
                    super::super::super::internal(
                        "native module clone input lacks structured members",
                    )
                })?;
            let table = self.structured_member_table(module_type, &resolved);
            let native_symbol =
                self.type_symbol_of_reference(ReferenceSymbol::from_property(symbol), consumer)?;
            let ty = sequence.read(self.new_resolved_anonymous_type_completion(
                native_symbol,
                table,
                MembersView {
                    properties: Arc::from([]),
                    call_signatures: Arc::from([]),
                    construct_signatures: Arc::from([]),
                    index_signatures: resolved.index_signatures,
                },
                ObjectFlags::default(),
                consumer,
            ))?;
            Ok((symbol, ty))
        })();
        let completed = sequence.finish(result)?;
        let (symbol, ty) = *completed.value();
        self.set_property_value_state(
            symbol,
            false,
            TypeState::Installed(completed.clone().map(|_| ty)),
        )?;
        Ok(completed.map(|_| ReferenceSymbol::from_property(symbol)))
    }
}
