//! Imported JSON checking within the request-owned evaluator. Every original
//! initializer is demanded before the actually widened module is read.

use super::super::model::{NativeResult, NativeSequence, TypeId, UnionReduction};
use super::Evaluator;
use crate::analysis::SemanticFailureReason;
use crate::project::binding::{BoundJson, JsonDeclaration, JsonFacts, JsonShape, JsonValueId};
use crate::project::sources::FileId;
use crate::types::PrimitiveName;
use std::collections::HashMap;

impl Evaluator<'_, '_> {
    /// `getTypeOfVariableOrParameterOrPropertyWorker`'s JSON SourceFile
    /// branch checks the full root before widening; raw PropertyAssignment
    /// symbols check their own original initializer, including duplicates.
    pub(super) fn json_declaration_type_completion(
        &mut self,
        file: FileId,
        declaration: JsonDeclaration,
    ) -> NativeResult<TypeId> {
        let selected = BoundJson {
            file,
            value: JsonFacts::ROOT,
        };
        let facts = self.inputs.bindings.json_facts(selected);
        let value = match declaration {
            JsonDeclaration::SourceFile => JsonFacts::ROOT,
            JsonDeclaration::Property(property) => facts.property_initializer(property),
            JsonDeclaration::Value(_) => {
                return Err(super::super::internal(
                    "object-literal symbol has no raw getTypeOfSymbol value family",
                )
                .into());
            }
        };
        let ty = self.construct_json(selected, facts, value)?;
        if matches!(declaration, JsonDeclaration::SourceFile) {
            let location = super::super::location(
                self.inputs,
                file,
                facts
                    .describe(value)
                    .span
                    .unwrap_or(crate::Span { start: 0, end: 0 }),
            );
            ty.and_then(|ty| self.widened_type_completion(ty, &location))
        } else {
            Ok(ty)
        }
    }

    /// JSON `checkExpression` / `checkObjectLiteral` / `checkArrayLiteral`.
    /// Source-order initializer demand precedes the unique final property
    /// table. Native subtype/nullable/global factories stay shared owners.
    fn construct_json(
        &mut self,
        selected: BoundJson,
        facts: &JsonFacts,
        value: JsonValueId,
    ) -> NativeResult<TypeId> {
        let description = facts.describe(value);
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let ty = match description.shape {
                JsonShape::String => self.model.primitive(PrimitiveName::String),
                JsonShape::Number => self.model.primitive(PrimitiveName::Number),
                JsonShape::Boolean => self.model.primitive(PrimitiveName::Boolean),
                JsonShape::Null => self.model.null_initializer(),
                JsonShape::Empty => self.model.empty_object_type(),
                JsonShape::Object(object) => {
                    let mut checked = HashMap::new();
                    let mut checked_values = Vec::new();
                    for initializer in object.initializers(value) {
                        let location =
                            super::super::location(self.inputs, selected.file, initializer.span);
                        if initializer.key.as_str().is_none() {
                            return Err(super::super::unsupported_demand_at_location(
                                self.inputs,
                                &location,
                                SemanticFailureReason::UnrepresentableIdentity,
                                "JSON property identity contains an unpaired UTF-16 surrogate",
                            ));
                        }
                        if initializer.optional {
                            return Err(super::super::unsupported_demand_at_location(
                                self.inputs,
                                &location,
                                SemanticFailureReason::UnsupportedSemanticForm,
                                "native optional JSON property construction is pending",
                            ));
                        }
                        let raw = self
                            .source
                            .bindings()
                            .json_node_symbol(
                                selected.file,
                                JsonDeclaration::Property(initializer.id),
                            )
                            .map_err(|error| self.generic_binding_failure(error, &location))?;
                        let raw = self.register_property_symbol(raw)?;
                        let ty = self.construct_json(selected, facts, initializer.value)?;
                        checked_values.push(sequence.read(Ok(ty.clone()))?);
                        // The object consumes every child in order, while each
                        // property's installed Type slot retains only the actual
                        // initializer that created that transient property.
                        let symbol = self.create_object_literal_property(
                            raw,
                            ty,
                            super::property_symbols::ObjectLiteralPropertyOptions::default(),
                            &location,
                        )?;
                        checked.insert(initializer.value, symbol);
                    }
                    let location = super::super::location(
                        self.inputs,
                        selected.file,
                        description.span.expect("JSON object has a captured span"),
                    );
                    let mut properties = Vec::new();
                    for value in object.final_values() {
                        let symbol = checked.get(&value).copied().ok_or_else(|| {
                            super::super::internal("JSON final property was not checked")
                        })?;
                        properties.push(self.project_property_symbol(symbol, &location)?);
                    }
                    let bound = self
                        .source
                        .bindings()
                        .json_node_symbol(selected.file, JsonDeclaration::Value(value))
                        .map_err(|error| self.generic_binding_failure(error, &location))?;
                    let symbol = self.type_symbol(bound, &location);
                    self.model.object(symbol, properties, &checked_values)?
                }
                JsonShape::Array(elements) => {
                    let location = super::super::location(
                        self.inputs,
                        selected.file,
                        description.span.expect("JSON arrays have a captured span"),
                    );
                    // Native checkArrayLiteral establishes each child, then the
                    // subtype-reduced element, before consuming its Array target.
                    let mut types = Vec::new();
                    for &element in elements.iter() {
                        types.push(sequence.read(self.construct_json(selected, facts, element))?);
                    }
                    let element = if types.is_empty() {
                        self.model.empty_array_element()
                    } else {
                        sequence.read(self.union_type_reduced_completion(
                            types,
                            UnionReduction::Subtype,
                            None,
                            None,
                            &location,
                        ))?
                    };
                    let array = sequence.read(self.array_type_completion(element, &location))?;
                    self.model.array_literal_type(array)?
                }
            };
            Ok(ty)
        })();
        sequence.finish(result)
    }
}
