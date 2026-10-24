//! Native applicable-index symbols share the actual IndexInfo identity.
use super::*;

impl Evaluator<'_, '_> {
    /// `getApplicableIndexSymbol` preserves nil and the any-base sentinel,
    /// gathers applicable declarations only for a synthetic combined index,
    /// then installs one Property/IndexSymbol on that actual IndexInfo.
    pub(in crate::project::semantic::evaluation) fn applicable_index_symbol_completion(
        &mut self,
        ty: TypeId,
        key: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<PropertySymbolId>> {
        use crate::resources::complete_or_abort;
        let _depth = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(info) =
                sequence.read(self.applicable_index_info_completion(ty, key, consumer))?
            else {
                return Ok(None);
            };
            if info.same_identity(&self.model.any_base_index()) {
                return Ok(None);
            }
            if let Some(&symbol) = self.native_members.symbols.index_symbols.get(&info) {
                return Ok(Some(symbol));
            }
            let mut declarations = Vec::new();
            if let Some(declaration) = info.declaration {
                declarations.push(ReferenceDeclaration::Captured(declaration));
            } else {
                let indices = sequence.read(self.index_infos_of_type_completion(ty, consumer))?;
                for index in indices {
                    if let Some(declaration) = index.declaration {
                        let index_key = sequence.read(self.demand_completion(index.key_type))?;
                        if sequence.read(
                            self.is_applicable_index_type_completion(key, index_key, consumer),
                        )? {
                            declarations.push(ReferenceDeclaration::Captured(declaration));
                        }
                    }
                }
            }
            let Some(&first) = declarations.first() else {
                return Ok(None);
            };
            let header = NativeSymbol::created_with_key(
                self.model.new_symbol_identity(),
                NativePropertyKey::fixed(b"\xfeindex".as_slice()),
                Some(self.reference_declaration_ordering(first)?),
            );
            let parent = self
                .reference_symbol_of_type(ty, consumer)?
                .map(|symbol| self.reference_as_parent(symbol))
                .transpose()?;
            let value = self.model.established_type(info.value_type)?;

            let symbol = self.install_created_property(CreatedProperty {
                native_flags: None,
                flags: PropertySymbolFlags {
                    property: true,
                    ..Default::default()
                },
                checks: checks::INDEX_SYMBOL,
                declarations: declarations.into_iter().map(Into::into).collect(),
                value_declaration: Some(first.into()),
                parent,
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
            });
            self.native_members.symbols.entries[symbol.index()].read =
                Some(TypeState::Installed(NativeCompletion::clean(value)));
            self.native_members
                .symbols
                .index_symbols
                .insert(info, symbol);
            Ok(Some(symbol))
        })();
        sequence.finish(result)
    }
}
