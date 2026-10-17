//! Directional enum compatibility and its diagnostic-sensitive native cache.
use super::super::super::{constants::ConstantValue, source::DeclarationIdentity};
use super::super::{ReferenceDeclaration, ReferenceSymbol, native_display::DiagnosticTypeFormat};
use super::*;

impl Evaluator<'_, '_> {
    /// `isEnumTypeRelatedTo`: successful/quiet-failed hits replay their own
    /// acquired result; a failed reporting comparison reruns without replaying
    /// its old evidence. Cache publication follows all actual diagnostic reads.
    pub(super) fn enum_type_related_completion(
        &mut self,
        source: ReferenceSymbol,
        target: ReferenceSymbol,
        reporter: Option<RelationInvocation>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let source_is_member = self.reference_symbol_flags(source, consumer)?.enum_member;
        // Go evaluates both `core.IfElse` value arguments: each parent read
        // occurs even when the input is already its enum's symbol.
        let source_parent = self.reference_parent_of_symbol(source, consumer)?;
        let source = if source_is_member {
            source_parent
                .ok_or_else(|| super::super::super::internal("enum member has no parent"))?
        } else {
            source
        };
        let target_is_member = self.reference_symbol_flags(target, consumer)?.enum_member;
        let target_parent = self.reference_parent_of_symbol(target, consumer)?;
        let target = if target_is_member {
            target_parent
                .ok_or_else(|| super::super::super::internal("enum member has no parent"))?
        } else {
            target
        };
        let key = (
            self.reference_symbol_identity(source, consumer)?,
            self.reference_symbol_identity(target, consumer)?,
        );
        if key.0 == key.1 {
            return Ok(NativeCompletion::clean(true));
        }
        if self.reference_symbol_key(source, consumer)?
            != self.reference_symbol_key(target, consumer)?
            || !self.reference_symbol_flags(source, consumer)?.regular_enum
            || !self.reference_symbol_flags(target, consumer)?.regular_enum
        {
            return Ok(NativeCompletion::clean(false));
        }
        if let Some(cached) = self.relations.enum_relations.get(&key)
            && (*cached.value() || reporter.is_none())
        {
            return Ok(cached.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let target_type =
                sequence.read(self.value_reference_type_completion(target, consumer))?;
            let source_type =
                sequence.read(self.value_reference_type_completion(source, consumer))?;
            for property in
                sequence.read(self.properties_of_type_completion(source_type, consumer))?
            {
                let source_property =
                    ReferenceSymbol::from_property(self.property_symbol(&property, consumer)?);
                if !self
                    .reference_symbol_flags(source_property, consumer)?
                    .enum_member
                {
                    continue;
                }
                let name = self.reference_symbol_key(source_property, consumer)?;
                let target_property = sequence.read(self.property_of_type_by_key_completion(
                    target_type,
                    &name,
                    consumer,
                ))?;
                let target_property = target_property
                    .as_ref()
                    .map(|property| self.property_symbol(property, consumer))
                    .transpose()?
                    .map(ReferenceSymbol::from_property);
                let target_property = match target_property {
                    Some(symbol) if self.reference_symbol_flags(symbol, consumer)?.enum_member => {
                        symbol
                    }
                    _ => {
                        if let Some(session) = reporter {
                            let name = sequence
                                .read(self.raw_diagnostic_symbol_display_completion(
                                    source_property,
                                    consumer,
                                ))?
                                .into_bytes();
                            let declared = sequence
                                .read(self.declared_reference_type_completion(target, consumer))?;
                            let target_name = sequence
                                .read(self.raw_diagnostic_type_display_completion(
                                    declared,
                                    None,
                                    DiagnosticTypeFormat::FULLY_QUALIFIED,
                                    consumer,
                                ))?
                                .into_bytes();
                            self.relations.session_mut(session).error.report(
                                Message::EnumMissingProperty,
                                vec![Argument::Text(name), Argument::Text(target_name)],
                            );
                        }
                        return Ok(false);
                    }
                };
                let source_declaration = self.enum_member_declaration(source_property, consumer)?;
                let source_value = sequence
                    .read(self.enum_member_value_completion(source_declaration))?
                    .value;
                let target_declaration = self.enum_member_declaration(target_property, consumer)?;
                let target_value = sequence
                    .read(self.enum_member_value_completion(target_declaration))?
                    .value;
                if source_value != target_value {
                    let both_known = source_value.is_some() && target_value.is_some();
                    let string = matches!(source_value, Some(ConstantValue::String(_)))
                        || matches!(target_value, Some(ConstantValue::String(_)));
                    if both_known || string {
                        if let Some(session) = reporter {
                            let target_name = sequence
                                .read(
                                    self.raw_diagnostic_symbol_display_completion(target, consumer),
                                )?
                                .into_bytes();
                            let member_name = sequence
                                .read(self.raw_diagnostic_symbol_display_completion(
                                    target_property,
                                    consumer,
                                ))?
                                .into_bytes();
                            let mut arguments =
                                vec![Argument::Text(target_name), Argument::Text(member_name)];
                            let message = if both_known {
                                arguments.push(Argument::Text(self.enum_value_text(
                                    target_value.as_ref().expect("known value"),
                                    consumer,
                                )?));
                                arguments.push(Argument::Text(self.enum_value_text(
                                    source_value.as_ref().expect("known value"),
                                    consumer,
                                )?));
                                Message::EnumDifferentValues
                            } else {
                                arguments.push(Argument::Text(
                                    self.enum_value_text(
                                        source_value
                                            .as_ref()
                                            .or(target_value.as_ref())
                                            .expect("known string value"),
                                        consumer,
                                    )?,
                                ));
                                Message::EnumUnknownNumericValue
                            };
                            self.relations
                                .session_mut(session)
                                .error
                                .report(message, arguments);
                        }
                        return Ok(false);
                    }
                }
            }
            Ok(true)
        })();
        let result = sequence.finish(result)?;
        self.relations.enum_relations.insert(key, result.clone());
        Ok(result)
    }

    /// `GetDeclarationOfKind(symbol, EnumMember)` observes the declaration
    /// list, not ValueDeclaration (a distinct native metadata question).
    fn enum_member_declaration(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<DeclarationIdentity, DemandError> {
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            if let ReferenceDeclaration::Captured(declaration) = declaration
                && self.source.declaration_kind(declaration)? == SyntaxKind::EnumMember
            {
                return Ok(declaration);
            }
        }
        Err(super::super::super::internal(
            "native enum member symbol has no EnumMember declaration",
        )
        .into())
    }

    /// `ValueToString` quotes the exact native constant bytes, including lone
    /// surrogate sentinels. Source constants never contain generated atoms.
    fn enum_value_text(
        &self,
        value: &ConstantValue,
        consumer: &Location,
    ) -> Result<Vec<u8>, DemandError> {
        Ok(match value {
            ConstantValue::Number(number) => crate::numbers::number_to_string(*number).into_bytes(),
            ConstantValue::String(value) => {
                let bytes = value
                    .fixed_bytes()
                    .map_err(|error| self.literal_value_failure(error, consumer))?;
                crate::printer::quoted_bytes(bytes, '"').into_bytes()
            }
        })
    }
}
