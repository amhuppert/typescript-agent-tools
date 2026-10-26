//! `resolveEntityName` selects from checker-owned tables. The retained scope
//! traversal remains shared with loading; only getSymbol and member demands
//! enter the evaluator, where aliases and created identities are available.
use super::*;
use crate::Span;
use crate::project::binding::{NativeLookupMeaning, NativeSelectionFailure};
use crate::project::sources::FileId;

impl Evaluator<'_, '_> {
    /// `getSymbol`: one merge observation, direct flags, then getSymbolFlags.
    /// Eligibility can follow aliases while the result keeps the selected alias.
    pub(in crate::project::semantic::evaluation) fn select_reference_symbol_completion(
        &mut self,
        symbol: ReferenceSymbol,
        meaning: NativeLookupMeaning,
        direct: bool,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        self.select_reference_symbol_at_completion(symbol, meaning, direct, Some(consumer))
    }
    /// `getSymbol` can observe nil/direct meanings without a source node. Only
    /// its actual alias acquisition obtains an own real declaration consumer.
    pub(in crate::project::semantic::evaluation) fn select_reference_symbol_at_completion(
        &mut self,
        symbol: ReferenceSymbol,
        meaning: NativeLookupMeaning,
        direct: bool,
        consumer: Option<&Location>,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        if direct {
            return Ok(NativeCompletion::clean(Some(symbol)));
        }
        let symbol = self.get_merged_reference_symbol_at(symbol, consumer)?;
        let flags = self.reference_symbol_native_flags_at(symbol, consumer)?;
        if meaning.matches(flags) {
            return Ok(NativeCompletion::clean(Some(symbol)));
        }
        if !flags.type_flags().alias {
            return Ok(NativeCompletion::clean(None));
        }
        let consumer = self
            .reference_symbol_consumer(symbol, consumer)?
            .ok_or_else(|| {
                super::super::super::internal("alias lookup lacks its actual declaration")
            })?;
        Ok(self
            .reference_symbol_flags_ex_completion(symbol, false, false, &consumer)?
            .map(|flags| meaning.matches(flags.native).then_some(symbol)))
    }

    pub(in crate::project::semantic::evaluation) fn reference_selection_failure(
        &self,
        error: NativeSelectionFailure<DemandError>,
        consumer: &Location,
    ) -> DemandError {
        match error {
            NativeSelectionFailure::Binding(error) => self.generic_binding_failure(error, consumer),
            NativeSelectionFailure::Selection(error) => error,
        }
    }

    /// `resolveEntityName` follows Alias only while its requested meaning is
    /// absent. Unrelated local meanings cannot stop this distinct operation.
    fn entity_symbol_completion(
        &mut self,
        mut symbol: ReferenceSymbol,
        meaning: NativeLookupMeaning,
        dont_resolve_alias: bool,
        consumer: &Location,
    ) -> NativeResult<ReferenceSymbol> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            loop {
                let flags = self.reference_symbol_flags(symbol, consumer)?;
                if dont_resolve_alias || meaning.matches(flags.native) || !flags.alias {
                    return Ok(symbol);
                }
                let target = sequence
                    .read(self.resolve_alias_target_completion(symbol, consumer))?
                    .ok_or_else(|| {
                        super::super::super::internal("resolveAlias returned native nil")
                    })?;
                symbol = target;
            }
        })();
        sequence.finish(result)
    }

    /// `resolveQualifiedName` probes the complete own exports with the requested
    /// meaning, then retries an alias target only when that first probe misses.
    fn entity_export_completion(
        &mut self,
        parent: ReferenceSymbol,
        name: &str,
        meaning: NativeLookupMeaning,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        self.reference_exports_of_symbol_completion(parent, consumer)?
            .and_then(|table| {
                let symbol = lookup(table.as_deref(), &NativePropertyKey::fixed(name.as_bytes()));
                match symbol {
                    Some(symbol) => {
                        self.select_reference_symbol_completion(symbol, meaning, false, consumer)
                    }
                    None => Ok(NativeCompletion::clean(None)),
                }
            })
    }

    /// Complete entity lookup, retaining the exact failed component for the
    /// caller's diagnostic and every recovered computed-table acquisition.
    /// `resolveEntityName` resolves the head at that identifier;
    /// `resolveQualifiedName` reports a missing export at its right identifier.
    /// A quiet caller may discard the located miss without acquiring recovery.
    pub(in crate::project::semantic::evaluation) fn reference_entity_at_completion(
        &mut self,
        file: FileId,
        anchor: Span,
        type_reference: bool,
        meaning: NativeLookupMeaning,
        dont_resolve_alias: bool,
        consumer: &Location,
    ) -> NativeResult<(Option<ReferenceSymbol>, Option<DemandError>)> {
        let bindings = self.source.bindings();
        let recipe = bindings
            .entity_name_recipe(file, anchor, type_reference)
            .map_err(|error| self.generic_binding_failure(error, consumer))?;
        let head_meaning = if recipe.members.is_empty() {
            meaning
        } else {
            NativeLookupMeaning::NAMESPACE
        };
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let head = sequence.read(self.reference_entity_head_completion(
                &recipe,
                head_meaning,
                consumer,
            ))?;
            let Some(mut symbol) = head else {
                let location = super::super::super::location(self.inputs, file, recipe.head.span);
                return Ok((
                    None,
                    Some(self.generic_binding_failure(recipe.head.missing, &location)),
                ));
            };
            // resolveEntityName observes getMergedSymbol again after resolveName.
            symbol = self.get_merged_reference_symbol(symbol, consumer)?;
            for (index, member) in recipe.members.iter().enumerate() {
                let parent = sequence.read(self.entity_symbol_completion(
                    symbol,
                    NativeLookupMeaning::NAMESPACE,
                    false,
                    consumer,
                ))?;
                if self.is_unknown_reference_symbol(parent) {
                    return Ok((Some(parent), None));
                }
                let member_meaning = if index + 1 == recipe.members.len() {
                    meaning
                } else {
                    NativeLookupMeaning::NAMESPACE
                };
                let mut found = sequence.read(self.entity_export_completion(
                    parent,
                    &member.name,
                    member_meaning,
                    consumer,
                ))?;
                if found.is_none() && self.reference_symbol_flags(parent, consumer)?.alias {
                    let target = sequence
                        .read(self.resolve_alias_target_completion(parent, consumer))?
                        .ok_or_else(|| {
                            super::super::super::internal("resolveAlias returned native nil")
                        })?;
                    if !self.is_unknown_reference_symbol(target) {
                        found = sequence.read(self.entity_export_completion(
                            target,
                            &member.name,
                            member_meaning,
                            consumer,
                        ))?;
                    }
                }
                let Some(found) = found else {
                    let location = super::super::super::location(self.inputs, file, member.span);
                    return Ok((
                        None,
                        Some(self.generic_binding_failure(member.missing.clone(), &location)),
                    ));
                };
                symbol = self.get_merged_reference_symbol(found, consumer)?;
            }
            Ok((
                Some(sequence.read(self.entity_symbol_completion(
                    symbol,
                    meaning,
                    dont_resolve_alias,
                    consumer,
                ))?),
                None,
            ))
        })();
        sequence.finish(result)
    }
}
