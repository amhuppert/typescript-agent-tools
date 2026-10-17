//! Native body-predicate eligibility; changed flow answers retain the F gate.

use super::super::super::model::{
    DemandError, NativeResult, NativeSequence, Predicate, TypeDemand, TypeFlags,
};
use super::super::super::source::SignatureRoot;
use super::super::Evaluator;
use super::super::reads::{FlowNodeRef, FlowReferenceQuestion};
use crate::references::Location;

impl Evaluator<'_, '_> {
    /// `getTypePredicateFromBody` / `checkIfExpressionRefinesAnyParameter` use
    /// exact body eligibility and ordered raw symbol reads. The shared flow
    /// owner admits unchanged types; an inferred refinement remains F-required.
    pub(super) fn type_predicate_from_body_completion(
        &mut self,
        root: SignatureRoot,
        consumer: &Location,
    ) -> NativeResult<Option<Predicate>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Predicate>, DemandError> {
            let Some(body) = self.source.predicate_body(root)? else {
                return Ok(None);
            };
            if body.block
                && sequence.read(self.function_has_implicit_return_completion(root, consumer))?
            {
                return Ok(None);
            }
            let returned = sequence
                .read(self.check_expression_cached_completion(root.file, body.expression))?;
            if !self.model.has_type_flags(returned, TypeFlags::BOOLEAN) {
                return Ok(None);
            }
            for (index, parameter) in body.parameters.into_iter().enumerate() {
                let symbol = self.source.raw_symbol_of_declaration(parameter)?;
                let family = self
                    .source
                    .bindings()
                    .value_type(symbol)
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                let initial = sequence.read(self.value_type_completion(symbol, &family))?;
                if self.model.has_type_flags(initial, TypeFlags::BOOLEAN) {
                    continue;
                }
                let Some(name) = self.source.predicate_parameter_name(parameter)? else {
                    continue;
                };
                if sequence.read(self.is_symbol_assigned_completion(
                    symbol.into(),
                    parameter,
                    consumer,
                ))? || self.source.predicate_parameter_is_rest(parameter)?
                {
                    continue;
                }
                let mut antecedent = self
                    .inputs
                    .bindings
                    .flow_node_of_node(root.file, body.expression)
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                if antecedent.is_none()
                    && let Some(parent) = body.parent_return
                {
                    antecedent = self
                        .inputs
                        .bindings
                        .flow_node_of_node(root.file, parent)
                        .map_err(|error| self.generic_binding_failure(error, consumer))?;
                }
                let antecedent = match antecedent {
                    Some(antecedent) => FlowNodeRef::Bound(antecedent),
                    None => self.new_flow_start(),
                };
                let condition = self.new_flow_condition(body.expression, true, antecedent);
                let container = self
                    .inputs
                    .bindings
                    .flow_graph(root.file)
                    .map_err(|error| self.generic_binding_failure(error, consumer))?
                    .container_id_at(root.site())
                    .map_err(super::super::super::internal)?;
                let true_type = sequence.read(self.flow_type_of_reference_completion(
                    FlowReferenceQuestion {
                        file: root.file,
                        reference: (root.file, name).into(),
                        declared: initial,
                        initial,
                        admitted: initial,
                        container: Some(container),
                        flow: Some(condition),
                    },
                    consumer,
                ))?;
                // Ordinary changed flow still obeys the shared F admission. A
                // completed recovery value can differ from the initial identity,
                // and native then acquires the False path and reduction too.
                if true_type == initial {
                    continue;
                }
                let condition = self.new_flow_condition(body.expression, false, antecedent);
                let false_type = sequence.read(self.flow_type_of_reference_completion(
                    FlowReferenceQuestion {
                        file: root.file,
                        reference: (root.file, name).into(),
                        declared: initial,
                        initial: true_type,
                        admitted: initial,
                        container: Some(container),
                        flow: Some(condition),
                    },
                    consumer,
                ))?;
                let false_type =
                    sequence.read(self.reduced_type_completion(false_type, consumer))?;
                if self.model.has_type_flags(false_type, TypeFlags::NEVER) {
                    return Ok(Some(Predicate {
                        kind: crate::types::TypePredicateKind::Is,
                        subject: crate::types::TypePredicateSubject::ParameterIndex(index),
                        parameter_name: Some(
                            self.source
                                .try_property_name_text(
                                    super::super::super::source::ExpressionRoot {
                                        file: root.file,
                                        site: name,
                                    },
                                )?
                                .ok_or_else(|| {
                                    super::super::super::internal(
                                        "predicate identifier lacks its actual name",
                                    )
                                })?
                                .into(),
                        ),
                        value_type: Some(TypeDemand::Known(true_type)),
                    }));
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }
}
