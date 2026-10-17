//! Native signature presentation parameters are checker-created symbols.

use super::*;
use std::collections::HashMap;

impl Evaluator<'_, '_> {
    /// Actual signature.mapper; composite factories and instantiated targets
    /// install the same native field through their respective associations.
    pub(in crate::project::semantic::evaluation) fn signature_mapper(
        &self,
        signature: &Signature,
    ) -> Option<MapperId> {
        self.signatures
            .instantiated
            .get(&signature.id)
            .map(|(_, mapper)| *mapper)
            .or_else(|| {
                self.signatures
                    .composites
                    .get(&signature.id)
                    .and_then(|composite| composite.mapper)
            })
    }

    /// `getTupleElementLabel` uses the actual label declaration, then rest
    /// parameter binding syntax, then the native synthesized name.
    pub(super) fn tuple_parameter_element_name(
        &mut self,
        info: &TupleElementInfo,
        rest: Option<&ParameterValue>,
        index: usize,
    ) -> Result<String, DemandError> {
        if let Some(label) = &info.label {
            return label.name.clone().map(|name| name.to_string());
        }
        if let Some(declaration) = rest.and_then(|rest| rest.declaration)
            && let Some(name) = self.source.tuple_element_label_from_binding_element(
                declaration,
                index,
                info.flags,
            )?
        {
            return Ok(name.to_string());
        }
        Ok(format!(
            "{}_{}",
            rest.map_or("arg", |rest| rest.name.as_str()),
            index
        ))
    }

    /// `getExpandedParameters` preserves raw rest-symbol reads, optional union
    /// expansion, argument-before-label demand and fresh synthetic parameters.
    pub(in crate::project::semantic) fn expanded_parameters_completion(
        &mut self,
        signature: &Signature,
        skip_union_expanding: bool,
        consumer: &Location,
    ) -> NativeResult<Vec<Vec<ParameterValue>>> {
        if signature.flags.has_rest() {
            let rest_index =
                signature.parameters.len().checked_sub(1).ok_or_else(|| {
                    super::super::super::internal("rest signature has no parameter")
                })?;
            let rest = &signature.parameters[rest_index];
            return self.demand_completion(rest.value_type)?.and_then(|ty| {
                if self.model.is_tuple_type(ty) {
                    return Ok(self
                        .expand_tuple_parameters_completion(signature, rest_index, ty, consumer)?
                        .map(|parameters| vec![parameters]));
                }
                if !skip_union_expanding
                    && let Some(types) = self.model.union_constituents(ty)
                    && types.iter().all(|&ty| self.model.is_tuple_type(ty))
                {
                    let mut expanded = NativeCompletion::clean(Vec::with_capacity(types.len()));
                    for &ty in types.iter() {
                        expanded = expanded.and_then(|mut expanded| {
                            Ok(self
                                .expand_tuple_parameters_completion(
                                    signature, rest_index, ty, consumer,
                                )?
                                .map(|parameters| {
                                    expanded.push(parameters);
                                    expanded
                                }))
                        })?;
                    }
                    return Ok(expanded);
                }
                Ok(NativeCompletion::clean(vec![
                    self.model.copy_parameter_values(&signature.parameters),
                ]))
            });
        }
        Ok(NativeCompletion::clean(vec![
            self.model.copy_parameter_values(&signature.parameters),
        ]))
    }

    /// `signatureToSignatureDeclarationHelper`: an interior variadic parameter
    /// requires the original syntax, even though expansion already ran.
    pub(in crate::project::semantic) fn signature_display_parameters<'a>(
        signature: &'a Signature,
        expanded: &'a [ParameterValue],
    ) -> &'a [ParameterValue] {
        if expanded
            .iter()
            .take(expanded.len().saturating_sub(1))
            .any(|parameter| parameter.rest)
        {
            &signature.parameters
        } else {
            expanded
        }
    }

    fn expand_tuple_parameters_completion(
        &mut self,
        signature: &Signature,
        rest_index: usize,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<ParameterValue>> {
        self.get_type_arguments_completion(ty)?.and_then(|types| {
            let tuple = self.model.tuple_target(ty)?.ok_or_else(|| {
                super::super::super::internal("expanded rest type is not a tuple")
            })?;
            let rest = &signature.parameters[rest_index];
            let mut names = Vec::with_capacity(tuple.elements.len());
            for (index, info) in tuple.elements.iter().enumerate() {
                names.push(self.tuple_parameter_element_name(info, Some(rest), index)?);
            }
            let mut unique = HashSet::new();
            let mut duplicates = Vec::new();
            for (index, name) in names.iter().enumerate() {
                if !unique.insert(name.clone()) {
                    duplicates.push(index);
                }
            }
            let mut counters = HashMap::new();
            for index in duplicates {
                let mut counter = counters.get(&names[index]).copied().unwrap_or(1);
                let name = loop {
                    let candidate = format!("{}_{}", names[index], counter);
                    if unique.insert(candidate.clone()) {
                        break candidate;
                    }
                    counter += 1;
                };
                names[index] = name;
                // Native stores under the newly assigned name, not the old root.
                counters.insert(names[index].clone(), counter + 1);
            }
            let mut sequence = NativeSequence::new();
            let result = (|| {
                let mut parameters = self
                    .model
                    .copy_parameter_values(&signature.parameters[..rest_index]);
                for (index, &ty) in types.iter().enumerate() {
                    let flags = tuple.elements[index].flags;
                    let rest = flags.intersects(TupleElementFlags::VARIABLE);
                    let optional = !rest && flags.intersects(TupleElementFlags::OPTIONAL);
                    let symbol = self.create_parameter_symbol(&names[index], optional, rest, None);
                    let value = if flags.intersects(TupleElementFlags::REST) {
                        self.array_type_completion(ty, consumer)?
                    } else {
                        NativeCompletion::clean(ty)
                    };
                    // Each fresh symbol installs only its own array/type computation;
                    // the argument/name prefix belongs to this expansion invocation.
                    self.set_property_value_state(
                        symbol,
                        false,
                        super::super::TypeState::Installed(value.clone()),
                    )?;
                    sequence.read(Ok(value))?;
                    parameters.push(self.synthetic_parameter_value(
                        symbol,
                        names[index].clone(),
                        optional,
                        rest,
                        consumer,
                    ));
                }
                Ok(parameters)
            })();
            sequence.finish(result)
        })
    }
}
