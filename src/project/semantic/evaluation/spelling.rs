//! Candidate-name and identity ordering rules shared by property suggestions.
use super::super::model::{NativeResult, NativeSequence, PropertySymbolId};
use super::{Evaluator, ReferenceSymbol};
use crate::references::Location;

#[derive(Clone, Copy)]
pub(super) enum SuggestionMeaning {
    Value,
    ModuleMember,
    ClassMember,
}

impl Evaluator<'_, '_> {
    /// `getSpellingSuggestionForName(..., SymbolFlagsValue)` uses SymbolName,
    /// raw value flags, and only then tryResolveAlias. Candidate order remains
    /// the native CompareSymbols order supplied to the spelling algorithm.
    pub(super) fn spelling_suggestion_for_properties_completion(
        &mut self,
        name: &str,
        symbols: Vec<PropertySymbolId>,
        consumer: &Location,
    ) -> NativeResult<Option<PropertySymbolId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            sequence
                .read(
                    self.spelling_suggestion_for_symbols_completion(
                        name,
                        symbols
                            .into_iter()
                            .map(ReferenceSymbol::from_property)
                            .collect(),
                        SuggestionMeaning::Value,
                        consumer,
                    ),
                )?
                .map(|symbol| {
                    symbol.property().ok_or_else(|| {
                        super::super::internal("property suggestion lost its input identity").into()
                    })
                })
                .transpose()
        })();
        sequence.finish(result)
    }

    /// `getSpellingSuggestionForName`: the raw requested meaning precedes
    /// tryResolveAlias. ModuleMember itself includes Alias, unlike Value.
    pub(super) fn spelling_suggestion_for_symbols_completion(
        &mut self,
        name: &str,
        symbols: Vec<ReferenceSymbol>,
        meaning: SuggestionMeaning,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        let mut sequence = NativeSequence::new();
        let result = crate::spelling::try_suggestion_by(
            self,
            name,
            symbols,
            |checker, symbol| {
                let name = checker.reference_symbol_display_name_bytes(symbol, consumer)?;
                if name.is_empty() || matches!(name.first(), Some(b'"' | 0xfe)) {
                    return Ok(String::new());
                }
                let flags = checker.reference_symbol_flags(symbol, consumer)?;
                let matches = match meaning {
                    SuggestionMeaning::Value => flags.value,
                    SuggestionMeaning::ModuleMember => checker
                        .reference_symbol_native_flags(symbol, consumer)?
                        .has_module_member(),
                    SuggestionMeaning::ClassMember => {
                        let symbol = checker.register_reference_symbol(symbol, consumer)?;
                        checker.property_symbol_flags(symbol)?.class_member()
                    }
                };
                let accepted = if matches {
                    true
                } else if flags.alias {
                    match sequence
                        .read(checker.try_resolve_alias_target_completion(symbol, consumer))?
                    {
                        Some(target) => match meaning {
                            SuggestionMeaning::Value => {
                                checker.reference_symbol_flags(target, consumer)?.value
                            }
                            SuggestionMeaning::ModuleMember => checker
                                .reference_symbol_native_flags(target, consumer)?
                                .has_module_member(),
                            SuggestionMeaning::ClassMember => {
                                let symbol = checker.register_reference_symbol(target, consumer)?;
                                checker.property_symbol_flags(symbol)?.class_member()
                            }
                        },
                        None => false,
                    }
                } else {
                    false
                };
                if !accepted {
                    return Ok(String::new());
                }
                String::from_utf8(name).map_err(|_| {
                    super::super::unsupported_demand_at_location(
                        checker.inputs,
                        consumer,
                        crate::analysis::SemanticFailureReason::UnrepresentableIdentity,
                        "spelling candidate contains non-scalar native text",
                    )
                })
            },
            |checker, left, right| {
                let left = checker.register_reference_symbol(left, consumer)?;
                let right = checker.register_reference_symbol(right, consumer)?;
                let left = checker.property_native_symbol(left, consumer)?;
                let right = checker.property_native_symbol(right, consumer)?;
                checker
                    .model
                    .compare_native_symbols(Some(&left), Some(&right))
                    .map_err(|error| checker.generic_binding_failure(error, consumer))
            },
        );
        sequence.finish(result)
    }
}
