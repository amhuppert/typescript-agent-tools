//! Native spread symbols share ordinary symbol headers and Type slots.
use super::*;
use crate::project::semantic::model::{NativeResult, NativeSequence, SpecialName, UnionReduction};

impl Evaluator<'_, '_> {
    /// The optional-property factory in tryMergeUnionOfObjectTypeAndEmptyObject.
    /// It always allocates, even when getSpreadSymbol would reuse the source.
    pub(in crate::project::semantic::evaluation) fn create_optional_spread_symbol_completion(
        &mut self,
        source: PropertySymbolId,
        readonly: bool,
        consumer: &Location,
    ) -> NativeResult<PropertySymbolId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let flags = self.property_symbol_flags(source)?;
            let checks = self.property_check_flags(source)? & checks::LATE
                | if readonly { checks::READONLY } else { 0 };
            let identity = self.model.new_symbol_identity();
            let ty = if flags.set_accessor && !flags.get_accessor {
                self.model.special(SpecialName::Undefined)
            } else {
                let ty = sequence.read(self.property_raw_type_completion(source, consumer))?;
                sequence.read(self.add_optionality_completion(ty, true, true, consumer))?
            };
            let declarations = self.property_reference_declarations(source, consumer)?;
            let name_type = self.property_name_type(source)?;
            let header = self
                .property_native_symbol(source, consumer)?
                .with_identity(identity);
            let symbol = self.install_created_property(CreatedProperty {
                native_flags: None,
                flags: PropertySymbolFlags {
                    property: true,
                    optional: true,
                    ..Default::default()
                },
                checks,
                declarations: declarations.into_iter().map(Into::into).collect(),
                value_declaration: None,
                parent: None,
                raw_members: None,
                raw_exports: None,
                name_type,
                header,
                mapper: None,
                target: None,
                synthetic_origin: Some(source),
                spread: None,
                export_type: None,
                value: CreatedPropertyValue::Known,
            });
            Ok((symbol, ty))
        })();
        let completed = sequence.finish(result)?;
        let (symbol, ty) = *completed.value();
        self.set_property_value_state(
            symbol,
            false,
            TypeState::Installed(completed.clone().map(|_| ty)),
        )?;
        Ok(completed.map(|_| symbol))
    }

    /// The overlapping optional-property factory in getSpreadType. The caller
    /// already acquired rightType before testing the optional bit.
    pub(in crate::project::semantic::evaluation) fn create_overlapping_spread_symbol_completion(
        &mut self,
        left: PropertySymbolId,
        right: PropertySymbolId,
        right_type: TypeId,
        consumer: &Location,
    ) -> NativeResult<PropertySymbolId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut declarations = self.property_reference_declarations(left, consumer)?;
            declarations.extend(self.property_reference_declarations(right, consumer)?);
            let flags = self.property_symbol_flags(left)?;
            let identity = self.model.new_symbol_identity();
            let left_type = sequence.read(self.property_raw_type_completion(left, consumer))?;
            let left_without_undefined = sequence
                .read(self.remove_missing_or_undefined_type_completion(left_type, consumer))?;
            let right_without_undefined = sequence
                .read(self.remove_missing_or_undefined_type_completion(right_type, consumer))?;
            let ty = if left_without_undefined == right_without_undefined {
                left_type
            } else {
                sequence.read(self.union_type_reduced_completion(
                    vec![left_type, right_without_undefined],
                    UnionReduction::Subtype,
                    None,
                    None,
                    consumer,
                ))?
            };
            let name_type = self.property_name_type(left)?;
            let header = self
                .property_native_symbol(left, consumer)?
                .with_identity(identity);
            let symbol = self.install_created_property(CreatedProperty {
                native_flags: None,
                flags: PropertySymbolFlags {
                    property: true,
                    optional: flags.optional,
                    ..Default::default()
                },
                checks: 0,
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
                spread: Some((left, right)),
                export_type: None,
                value: CreatedPropertyValue::Known,
            });
            Ok((symbol, ty))
        })();
        let completed = sequence.finish(result)?;
        let (symbol, ty) = *completed.value();
        self.set_property_value_state(
            symbol,
            false,
            TypeState::Installed(completed.clone().map(|_| ty)),
        )?;
        Ok(completed.map(|_| symbol))
    }
}
