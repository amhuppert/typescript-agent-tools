//! `getIndexInfosOfSymbol` / `getIndexInfosOfIndexSymbol`: explicit and
//! late computed declarations share one ordered index producer.
use super::super::super::model::Index;
use super::super::relations::RelationKind;
use super::*;
use crate::types::PrimitiveName;

impl Evaluator<'_, '_> {
    pub(in crate::project::semantic::evaluation) fn index_symbol_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<ReferenceSymbol>> {
        Ok(self
            .reference_members_of_symbol_completion(symbol, consumer)?
            .map(|table| {
                lookup(
                    table.as_deref(),
                    &NativePropertyKey::fixed(b"\xfeindex".as_slice(), self.control.as_ref()),
                    self.control.as_ref(),
                )
            }))
    }

    pub(in crate::project::semantic::evaluation) fn index_infos_of_symbol_completion(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Vec<Index>> {
        self.index_symbol_completion(symbol, consumer)?
            .and_then(|index| {
                let Some(index) = index else {
                    return Ok(NativeCompletion::clean(Vec::new()));
                };
                self.reference_members_of_symbol_completion(symbol, consumer)?
                    .and_then(|siblings| {
                        self.index_infos_of_index_symbol_completion(
                            index,
                            siblings.as_ref(),
                            consumer,
                        )
                    })
            })
    }

    pub(in crate::project::semantic::evaluation) fn index_infos_of_index_symbol_completion(
        &mut self,
        index_symbol: ReferenceSymbol,
        siblings: Option<&ReferenceSymbolTable>,
        consumer: &Location,
    ) -> NativeResult<Vec<Index>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut indexes = Vec::new();
            let mut keys = Vec::new();
            let mut computed = [false; 3];
            let mut readonly = [true; 3];
            let mut properties = Vec::new();
            let string = self.model.primitive(PrimitiveName::String);
            let number = self.model.primitive(PrimitiveName::Number);
            let symbol = self.model.primitive(PrimitiveName::Symbol);
            for declaration in self.reference_symbol_declarations(index_symbol, consumer)? {
                let ReferenceDeclaration::Captured(declaration) = declaration else {
                    continue;
                };
                if self.source.declaration_kind(declaration)? == K::IndexSignature {
                    let Some(recipe) = self.source.index_signature_of_declaration(declaration)?
                    else {
                        continue;
                    };
                    for index in sequence.read(
                        self.index_infos_from_recipes_completion(std::slice::from_ref(&recipe)),
                    )? {
                        let key = self.model.established_type(index.key_type)?;
                        if !keys.contains(&key) {
                            keys.push(key);
                            indexes.push(index);
                        }
                    }
                } else if sequence.read(
                    self.has_late_bindable_index_signature_completion(declaration, consumer),
                )? {
                    let key =
                        sequence.read(self.late_declaration_name_type_completion(declaration))?;
                    if keys.contains(&key) {
                        continue;
                    }
                    if sequence.read(self.type_related_completion(
                        key,
                        self.model.string_number_symbol_type(),
                        RelationKind::Assignable,
                        consumer,
                    ))? {
                        let kind = if sequence.read(self.type_related_completion(
                            key,
                            number,
                            RelationKind::Assignable,
                            consumer,
                        ))? {
                            1
                        } else if sequence.read(self.type_related_completion(
                            key,
                            symbol,
                            RelationKind::Assignable,
                            consumer,
                        ))? {
                            2
                        } else {
                            0
                        };
                        computed[kind] = true;
                        readonly[kind] &= self
                            .source
                            .declaration_has_modifier(declaration, K::ReadonlyKeyword)?;
                        let raw = self
                            .reference_raw_symbol_of_declaration(declaration)?
                            .ok_or_else(|| {
                                super::super::super::internal(
                                    "computed index declaration has no raw symbol",
                                )
                            })?;
                        properties.push(self.register_reference_symbol(raw, consumer)?);
                    }
                }
            }
            if computed.iter().any(|&present| present) {
                let index_id = self.reference_symbol_identity(index_symbol, consumer)?;
                if let Some(siblings) = siblings {
                    for (_, sibling) in siblings.iter() {
                        if self.reference_symbol_identity(*sibling, consumer)? != index_id {
                            properties.push(self.register_reference_symbol(*sibling, consumer)?);
                        }
                    }
                }
                for (kind, key) in [string, number, symbol].into_iter().enumerate() {
                    if computed[kind] && !keys.contains(&key) {
                        indexes.push(sequence.read(self.object_literal_index_info_completion(
                            readonly[kind],
                            &properties,
                            key,
                            consumer,
                        ))?);
                    }
                }
            }
            Ok(indexes)
        })();
        sequence.finish(result)
    }
}
