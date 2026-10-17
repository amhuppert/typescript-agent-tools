//! Native mapped symbols use the shared checker-created symbol arena and Type
//! slot. Declaration links and synthetic origins never substitute for identity.
use super::*;
use crate::project::semantic::evaluation::resolution;

impl Evaluator<'_, '_> {
    /// Native `mappedSymbolLinks.syntheticOrigin`, without the broader spread
    /// and final-target traversal of `getImmediateRootSymbols`.
    pub(in crate::project::semantic::evaluation) fn property_mapped_origin(
        &self,
        symbol: PropertySymbolId,
    ) -> Result<Option<PropertySymbolId>, DemandError> {
        Ok(self.created_property(symbol)?.synthetic_origin)
    }

    /// `resolveMappedTypeMembers` creates one Property symbol for each usable
    /// name, retaining key/name types and modifier declaration links separately.
    #[expect(
        clippy::too_many_arguments,
        reason = "carries each resolveMappedTypeMembers symbol input separately"
    )]
    pub(in crate::project::semantic::evaluation) fn create_mapped_symbol(
        &mut self,
        name: crate::project::binding::NativePropertyKey,
        name_type: TypeId,
        key_type: TypeId,
        containing: TypeId,
        optional: bool,
        readonly: bool,
        strip_optional: bool,
        modifier: Option<PropertySymbolId>,
        link_declarations: bool,
        consumer: &Location,
    ) -> Result<PropertySymbolId, DemandError> {
        let late = modifier
            .map(|symbol| self.property_check_flags(symbol))
            .transpose()?
            .unwrap_or(0)
            & checks::LATE;
        let identity = self.model.new_symbol_identity();
        let declarations = if link_declarations {
            match modifier {
                Some(symbol) => self.property_reference_declarations(symbol, consumer)?,
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let declaration = declarations
            .first()
            .copied()
            .map(|declaration| self.reference_declaration_ordering(declaration))
            .transpose()?;
        let header = NativeSymbol::created_with_key(identity, name, declaration);
        Ok(self.install_created_property(CreatedProperty {
            native_flags: None,
            flags: PropertySymbolFlags {
                property: true,
                optional,
                ..Default::default()
            },
            checks: late
                | checks::MAPPED
                | if readonly { checks::READONLY } else { 0 }
                | if strip_optional {
                    checks::STRIP_OPTIONAL
                } else {
                    0
                },
            declarations: declarations.into_iter().map(Into::into).collect(),
            value_declaration: None,
            parent: None,
            raw_members: None,
            raw_exports: None,
            name_type: Some(name_type),
            header,
            mapper: None,
            target: None,
            synthetic_origin: modifier,
            spread: None,
            export_type: None,
            value: CreatedPropertyValue::Mapped {
                containing,
                key_type,
            },
        }))
    }

    /// Native repeated remapped names update nameType before keyType. The
    /// actual symbol header remains unchanged; any failed second union cannot
    /// undo the already installed first link.
    pub(in crate::project::semantic::evaluation) fn merge_mapped_symbol_key_completion(
        &mut self,
        symbol: PropertySymbolId,
        name: TypeId,
        key: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let control = self.control.clone();
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            let data = self.created_property(symbol)?;
            let previous_name = data.name_type.ok_or_else(|| {
                crate::project::semantic::internal("mapped property has no nameType")
            })?;
            let CreatedPropertyValue::Mapped {
                key_type: previous_key,
                ..
            } = &data.value
            else {
                return Err(crate::project::semantic::internal(
                    "mapped key merge has another symbol recipe",
                )
                .into());
            };
            let previous_key = *previous_key;
            drop(data);
            let name = sequence.read(self.union_type_ex_completion(
                vec![previous_name, name],
                None,
                None,
                consumer,
            ))?;
            let Association::Created(data) =
                &mut self.native_members.symbols.entries[symbol.index()].association
            else {
                unreachable!("created mapped association");
            };
            CreatedProperty::make_mut(data, control.as_ref()).name_type = Some(name);
            let key = sequence.read(self.union_type_ex_completion(
                vec![previous_key, key],
                None,
                None,
                consumer,
            ))?;
            let Association::Created(data) =
                &mut self.native_members.symbols.entries[symbol.index()].association
            else {
                unreachable!("created mapped association");
            };
            let CreatedPropertyValue::Mapped { key_type, .. } =
                &mut CreatedProperty::make_mut(data, control.as_ref()).value
            else {
                unreachable!("mapped value recipe");
            };
            *key_type = key;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `getTypeOfMappedSymbol` owns Type resolution, optional markers and the
    /// final installed-slot check. A local consumed failure wins over reentry.
    pub(super) fn mapped_property_type_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.mapped_property_type_worker(symbol, consumer)
            .map(|value| {
                value.observe(|failure| failure.at_property(PropertyValueQuestion::Read(symbol)))
            })
            .map_err(|failure| failure.at_property(PropertyValueQuestion::Read(symbol)))
    }

    fn mapped_property_type_worker(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(result) = self.property_value_completion(symbol, false)? {
            return result;
        }
        let data = self.created_property(symbol)?;
        let CreatedPropertyValue::Mapped {
            containing,
            key_type,
        } = &data.value
        else {
            unreachable!("mapped value dispatch");
        };
        let (containing, key_type) = (*containing, *key_type);
        let inputs = self.inputs;
        let circular = || {
            crate::project::semantic::invalid_at_location(
                inputs,
                consumer,
                crate::analysis::SemanticFailureReason::CircularType,
                "property type circularly references itself in a mapped type",
            )
            .into()
        };
        if !self.push_resolution(resolution::Key::PropertySymbol {
            target: symbol,
            property: resolution::Property::Type,
        }) {
            self.mark_mapped_contains_error(containing);
            return NativeCompletion::recovered(self.model.native_error_type(), circular());
        }
        let result: NativeResult<TypeId> = (|| {
            let (target, mapper) = self.object_target_mapper(containing)?;
            let template = self.mapped_template(target, consumer)?;
            let parameter = self.mapped_parameter(containing, consumer)?;
            let mapper = self.append_type_mapping(mapper, parameter, key_type)?;
            let ty = self.instantiate_type(template, mapper, consumer)?;
            if self.inputs.semantic_options().strict_null_checks
                && data.flags.optional
                && !self
                    .model
                    .maybe_type_of_kind(ty, TypeFlags::UNDEFINED | TypeFlags::VOID)
            {
                return self.optional_type(ty, true, consumer);
            }
            if data.checks & checks::STRIP_OPTIONAL != 0 {
                return self.remove_missing_or_undefined_type(ty, consumer);
            }
            Ok(ty)
        })()
        .map(NativeCompletion::clean);
        let resolved = self.resolution.pop().ok_or_else(|| {
            crate::project::semantic::internal("mapped type resolution has no accepted frame")
        })?;
        let result = result.and_then(|value| {
            if resolved {
                if let Some(TypeState::Installed(installed)) =
                    self.property_value_state(symbol, false)?
                {
                    return Ok(value.map(|_| *installed.value()));
                }
                self.set_property_value_state(symbol, false, TypeState::Installed(value.clone()))?;
                return Ok(value);
            }
            let ty = match self.property_value_state(symbol, false)? {
                Some(TypeState::Installed(installed)) => *installed.value(),
                _ => self.model.native_error_type(),
            };
            let value = value.and_then(|_| NativeCompletion::recovered(ty, circular()))?;
            if !self.property_has_value(symbol, false) {
                self.set_property_value_state(symbol, false, TypeState::Installed(value.clone()))?;
            }
            // The actual Type slot precedes both diagnostic arguments. An
            // interrupted child never starts this native reporting branch.
            value.and_then(|ty| {
                self.diagnostic_symbol_display_completion(
                    ReferenceSymbol::from_property(symbol),
                    consumer,
                )?
                .and_then(|_| self.check_diagnostic_type_display_completion(containing, consumer))
                .map(|report| report.map(|_| ty))
            })
        });
        if let Err(error) = &result
            && !self.property_has_value(symbol, false)
        {
            self.set_property_value_state(symbol, false, TypeState::Failed(error.clone()))?;
        }
        result
    }
}
