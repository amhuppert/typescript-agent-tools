//! Shared native signature source checks, independent of function bodies.
use super::super::super::model::{NativeResult, NativeSequence, TypeId};
use super::super::super::source::{ExpressionRoot, SignatureRoot};
use super::super::Evaluator;
use super::super::relations::RelationKind;
use crate::analysis::SemanticFailureReason;

impl Evaluator<'_, '_> {
    /// `checkTypePredicate` acquires the signature predicate before checking
    /// its asserted child. Only a bound, non-rest parameter predicate with an
    /// asserted type asks the raw parameter symbol type and assignability.
    pub(in crate::project::semantic::evaluation) fn check_type_predicate_completion(
        &mut self,
        root: ExpressionRoot,
        parent: Option<SignatureRoot>,
        asserted_node: Option<ExpressionRoot>,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            // An invalid parent emits a grammar diagnostic without acquiring
            // the signature or the asserted type.
            let Some(parent) = parent else {
                return Ok(());
            };
            let consumer = super::super::super::location(self.inputs, root.file, root.site.span);
            let recipe = self.source.signature_from_root(parent)?;
            let signature = sequence.read(self.signature_completion(&recipe))?;
            let Some(predicate) = sequence
                .read(self.type_predicate_of_signature_completion(&signature, &consumer))?
            else {
                return Ok(());
            };
            if let Some(asserted_node) = asserted_node {
                sequence.read(self.check_type_source_element_completion(asserted_node))?;
            }
            if let crate::types::TypePredicateSubject::ParameterIndex(index) = predicate.subject
                && !(signature.flags.has_rest() && index + 1 == signature.parameters.len())
                && let Some(asserted) = predicate.value_type
            {
                let asserted_node = asserted_node.ok_or_else(|| {
                    super::super::super::internal("asserted predicate type has no source child")
                })?;
                let parameter = signature.parameters.get(index).ok_or_else(|| {
                    super::super::super::internal("predicate parameter index exceeds its signature")
                })?;
                let asserted = sequence.read(self.demand_completion(asserted))?;
                let parameter = sequence.read(self.demand_completion(parameter.value_type))?;
                let error =
                    super::super::relations::RelationReporting::Report.assignment_at(asserted_node);
                // Native wraps the resulting diagnostic; a false relation is
                // still a completed check and supplies no recovery type.
                sequence.read(self.check_type_related_to_completion(
                    asserted,
                    parameter,
                    RelationKind::Assignable,
                    error,
                    &consumer,
                ))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkSignatureDeclaration` preserves semantic child checks even
    /// though function bodies are deferred. Grammar-only diagnostics do not
    /// become invented type dependencies.
    pub(in crate::project::semantic::evaluation) fn check_signature_declaration_completion(
        &mut self,
        root: SignatureRoot,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let facts = self.source.function_expression_facts(
                super::super::super::source::ExpressionRoot {
                    file: root.file,
                    site: root.site(),
                },
            )?;
            let consumer = &super::super::super::location(self.inputs, root.file, root.span());
            sequence.read(self.check_index_signature_grammar_completion(root, consumer))?;
            sequence.read(self.check_type_parameters_completion(&facts.type_parameter_nodes))?;
            for &declaration in &facts.parameters {
                self.reset_instantiation_count();
                sequence.read(self.check_variable_like_declaration_completion(declaration))?;
                let facts = self.source.variable_like_check(declaration)?;
                let recipe = self.source.parameter_recipe(declaration)?;
                if recipe.rest && facts.pattern.is_none() {
                    let symbol = self
                        .reference_raw_symbol_of_declaration(declaration)?
                        .ok_or_else(|| {
                            super::super::super::internal("rest parameter has no actual symbol")
                        })?;
                    let raw = sequence
                        .read(self.value_reference_type_completion(symbol, &recipe.location))?;
                    sequence
                        .read(self.check_rest_parameter_type_completion(raw, &recipe.location))?;
                }
            }
            if let Some(root) = facts.annotation_node {
                sequence.read(self.check_type_source_element_completion(root))?;
                if (!facts.flags.is_invalid() && facts.flags.is_generator())
                    || (!facts.flags.is_generator() && facts.flags.is_async())
                {
                    return Err(super::super::super::unsupported_demand_at_location(
                        self.inputs,
                        consumer,
                        SemanticFailureReason::UnsupportedSemanticForm,
                        "native async/generator signature annotation checks are pending",
                    ));
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkTypeParameter` owns source children, canonical constraints,
    /// defaults and the diagnostic-only default compatibility comparison.
    pub(in crate::project::semantic::evaluation) fn check_type_parameter_declaration_completion(
        &mut self,
        root: super::super::super::source::ExpressionRoot,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let consumer = &super::super::super::location(self.inputs, root.file, root.site.span);
            let (constraint, default_node, _anchor) =
                self.source.type_parameter_check_children(root)?;
            if let Some(root) = constraint {
                sequence.read(self.check_type_source_element_completion(root))?;
            }
            if let Some(root) = default_node {
                sequence.read(self.check_type_source_element_completion(root))?;
            }
            let declaration_node = self.source.type_node_declaration(root)?;
            let symbol =
                sequence.read(self.reference_symbol_of_declaration_completion(declaration_node))?;
            let ty = sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
            sequence.read(self.base_constraint_completion(ty, consumer))?;
            sequence.read(self.default_from_type_parameter_completion(ty))?;
            let constraint = sequence.read(self.parameter_constraint_completion(ty, consumer))?;
            let default = sequence.read(self.default_from_type_parameter_completion(ty))?;
            if let (Some(constraint), Some(default)) = (constraint, default) {
                let mapper = self.model.mapper(vec![ty], vec![default])?;
                let constraint = self.instantiate_type(constraint, mapper, consumer)?;
                let constraint =
                    self.get_type_with_this_argument(constraint, Some(default), false, consumer)?;
                let error = default_node.and_then(|node| {
                    super::super::relations::RelationReporting::Report.constraint_at(node)
                });
                sequence.read(self.check_type_related_to_completion(
                    default,
                    constraint,
                    RelationKind::Assignable,
                    error,
                    consumer,
                ))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkTypeParameters` owns the list-only default-reference walk after
    /// each single declaration check. Its reference acquisition bypasses the
    /// conditional-flow wrapper exactly as getTypeFromTypeReference does.
    pub(in crate::project::semantic::evaluation) fn check_type_parameters_completion(
        &mut self,
        parameters: &[super::super::super::source::ExpressionRoot],
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for &root in parameters {
                sequence.read(self.check_type_parameter_declaration_completion(root))?;
                let (_, default, _) = self.source.type_parameter_check_children(root)?;
                if let Some(default) = default {
                    for reference in self.source.type_parameter_default_references(default)? {
                        sequence.read(self.check_type_worker_completion(&reference))?;
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkParameter` asks the reduced raw symbol type and the initialized
    /// anyReadonlyArrayType only after checkVariableLikeDeclaration completes.
    fn check_rest_parameter_type_completion(
        &mut self,
        raw: TypeId,
        consumer: &crate::references::Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let ty = sequence.read(self.reduced_type_completion(raw, consumer))?;
            let array = self.installed_any_readonly_array_type()?;
            sequence.read(self.type_related_completion(
                ty,
                array,
                RelationKind::Assignable,
                consumer,
            ))?;
            Ok(())
        })();
        sequence.finish(result)
    }
}
