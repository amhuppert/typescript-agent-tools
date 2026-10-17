//! Native overload failure still constructs a real signature, then reports.
//! Diagnostic-only dependencies run after the call owner publishes that value.

use super::super::ReferenceDeclaration;
use super::*;

impl Evaluator<'_, '_> {
    /// `getCandidateForOverloadFailure` / `pickLongestCandidateSignature`.
    /// checkNodeDeferred enqueues later file checking; it does not eagerly
    /// acquire unmatched arguments in this query. Generic recovery uses a
    /// fresh signature factory, independently of ordinary instantiation caches.
    pub(super) fn candidate_for_overload_failure_completion(
        &mut self,
        input: &CallInput<'_>,
        candidates: &mut [Signature],
        mode: CheckMode,
    ) -> NativeResult<Signature> {
        if candidates.len() != 1
            && candidates
                .iter()
                .all(|signature| signature.type_parameter_types().is_empty())
        {
            return self.create_union_of_signatures_for_overload_failure_completion(
                candidates,
                &input.recipe.location,
            );
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let index = sequence.read(self.longest_candidate_index_completion(
                candidates,
                input.arguments.len(),
                &input.recipe.location,
            ))?;
            let candidate = &candidates[index];
            if candidate.type_parameter_types().is_empty() {
                return Ok(candidate.clone());
            }
            let arguments = if input.recipe.type_arguments.is_empty() {
                let context = self.new_call_inference_context(candidate);
                sequence.read(self.infer_call_type_arguments_completion(
                    input,
                    candidate,
                    context,
                    mode | CheckMode::SKIP_CONTEXT_SENSITIVE | CheckMode::SKIP_GENERIC_FUNCTIONS,
                ))?
            } else {
                sequence.read(self.overload_failure_type_arguments_completion(
                    &input.recipe.type_arguments,
                    candidate.type_parameter_types(),
                    &input.recipe.location,
                ))?
            };
            let instantiated = sequence.read(self.create_signature_instantiation_completion(
                candidate,
                arguments,
                &input.recipe.location,
            ))?;
            candidates[index] = instantiated.clone();
            Ok(instantiated)
        })();
        sequence.finish(result)
    }

    /// `getLongestCandidateIndex`: effective count is acquired before its
    /// rest predicate; the first accepting candidate or first strict maximum
    /// wins. Raw signature parameter counts answer a different question.
    fn longest_candidate_index_completion(
        &mut self,
        candidates: &[Signature],
        arguments: usize,
        consumer: &Location,
    ) -> NativeResult<usize> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut longest = None;
            for (index, candidate) in candidates.iter().enumerate() {
                let count = sequence
                    .read(self.signature_parameter_count_completion(candidate, consumer))?;
                if sequence
                    .read(self.signature_has_effective_rest_completion(candidate, consumer))?
                    || count >= arguments
                {
                    return Ok(index);
                }
                if longest.is_none_or(|(_, previous)| count > previous) {
                    longest = Some((index, count));
                }
            }
            longest.map(|(index, _)| index).ok_or_else(|| {
                super::super::super::internal("overload recovery has no candidates").into()
            })
        })();
        sequence.finish(result)
    }

    /// `getTypeArgumentsFromNodes`: truncate before acquisition, then fill
    /// missing positions with default, guarded constraint, or native Unknown.
    /// This deliberately does not use fillMissingTypeArguments or its mapper.
    fn overload_failure_type_arguments_completion(
        &mut self,
        nodes: &[CallTypeArgument],
        parameters: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut arguments = Vec::with_capacity(parameters.len());
            for argument in nodes.iter().take(parameters.len()) {
                arguments.push(sequence.read(self.check_type_completion(&argument.expression))?);
            }
            for &parameter in &parameters[arguments.len()..] {
                let default =
                    sequence.read(self.default_from_type_parameter_completion(parameter))?;
                let argument = match default {
                    Some(default) => default,
                    None => sequence
                        .read(self.parameter_constraint_completion(parameter, consumer))?
                        .unwrap_or_else(|| self.model.special(SpecialName::Unknown)),
                };
                arguments.push(argument);
            }
            Ok(arguments)
        })();
        sequence.finish(result)
    }

    /// `reportCallResolutionErrors` preserves the last chooseOverload pass's
    /// three error slots. Fallback arity filtering observes the original
    /// signature inventory, not reordered or recovery-instantiated candidates.
    pub(super) fn report_call_resolution_errors_completion(
        &mut self,
        input: &CallInput<'_>,
        selection: &OverloadSelection,
        originals: &[Signature],
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(last) = selection.errors.arguments.last() {
                let checked = sequence.read(self.signature_applicability_completion(
                    input,
                    last,
                    RelationKind::Assignable,
                    CheckMode::NORMAL,
                    RelationReporting::Report,
                ))?;
                for _ in 0..checked.diagnostics {
                    sequence.read(
                        self.implementation_success_elaboration_completion(input, selection, last),
                    )?;
                }
            } else if let Some(candidate) = &selection.errors.argument_arity {
                sequence.read(
                    self.argument_arity_error_completion(input, std::slice::from_ref(candidate)),
                )?;
            } else if let Some(candidate) = &selection.errors.type_arguments {
                sequence.read(self.check_type_arguments_completion(
                    candidate,
                    &input.recipe.type_arguments,
                    RelationReporting::Report,
                    &input.recipe.location,
                ))?;
            } else {
                let mut signatures = Vec::new();
                for signature in originals {
                    if self
                        .correct_type_argument_arity(signature, input.recipe.type_arguments.len())?
                    {
                        signatures.push(signature.clone());
                    }
                }
                if signatures.is_empty() {
                    // getTypeArgumentArityError only asks default presence;
                    // its diagnostic range and message add no type demands.
                    for signature in originals {
                        self.minimum_type_argument_count(signature.type_parameter_types())?;
                    }
                } else {
                    sequence.read(self.argument_arity_error_completion(input, &signatures))?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `addImplementationSuccessElaboration` reads the raw declaration
    /// Symbol, chooses the first actual body, then performs a fresh overload
    /// selection once per emitted diagnostic. An unsuccessful relation alone
    /// does not establish that a diagnostic was emitted.
    fn implementation_success_elaboration_completion(
        &mut self,
        input: &CallInput<'_>,
        selection: &OverloadSelection,
        failed: &Signature,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(root) = self.signature_declaration_root(failed.id)? else {
                return Ok(());
            };
            let declaration = self.source.signature_declaration_facts(root)?.declaration;
            let Some(symbol) = self.reference_raw_symbol_of_declaration(declaration)? else {
                return Ok(());
            };
            let declarations =
                self.reference_symbol_declarations(symbol, &input.recipe.location)?;
            if declarations.len() <= 1 {
                return Ok(());
            }
            for declaration in declarations {
                let ReferenceDeclaration::Captured(declaration) = declaration else {
                    continue;
                };
                if !self.source.function_declaration_has_body(declaration)? {
                    continue;
                }
                let recipe = self.source.signature_from_declaration(declaration)?;
                let candidate = self.signature(&recipe)?;
                let mut local = OverloadSelection {
                    single_nongeneric: candidate.type_parameter_types().is_empty(),
                    candidates: vec![candidate],
                    argument_mode: selection.argument_mode,
                    errors: OverloadErrors::default(),
                };
                sequence.read(self.choose_overload_completion(
                    input,
                    &mut local,
                    RelationKind::Assignable,
                ))?;
                break;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `getArgumentArityError`: a spread short-circuits all signature reads.
    /// Otherwise every minimum/count is acquired before Some(effective rest).
    fn argument_arity_error_completion(
        &mut self,
        input: &CallInput<'_>,
        signatures: &[Signature],
    ) -> NativeResult<()> {
        if input.arguments.iter().any(CallArgument::is_spread) {
            return Ok(NativeCompletion::clean(()));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut minimum = usize::MAX;
            let mut maximum = 0;
            for signature in signatures {
                minimum = minimum.min(sequence.read(
                    self.minimum_argument_count_completion(signature, &input.recipe.location),
                )?);
                maximum = maximum.max(sequence.read(
                    self.signature_parameter_count_completion(signature, &input.recipe.location),
                )?);
            }
            let mut rest = false;
            for signature in signatures {
                if sequence.read(
                    self.signature_has_effective_rest_completion(signature, &input.recipe.location),
                )? {
                    rest = true;
                    break;
                }
            }
            if !rest && minimum == 1 && maximum == 1 && input.arguments.is_empty() {
                sequence.read(self.promise_resolve_arity_error_completion(input.recipe))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `isPromiseResolveArityError` compares actual raw symbol identities;
    /// a same-spelled local constructor is not the global Promise value.
    fn promise_resolve_arity_error_completion(
        &mut self,
        recipe: &CallRecipe,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if recipe.kind != SyntaxKind::CallExpression {
                return Ok(false);
            }
            let Some(root) = recipe
                .callee
                .root
                .filter(|root| root.site.kind == SyntaxKind::Identifier)
            else {
                return Ok(false);
            };
            let Some(symbol) =
                sequence.read(self.reference_value_name_completion(root, &recipe.location))?
            else {
                return Ok(false);
            };
            let Some(ReferenceDeclaration::Captured(declaration)) =
                self.reference_symbol_value_declaration(symbol, &recipe.location)?
            else {
                return Ok(false);
            };
            let Some(constructor) = self.source.promise_resolve_constructor(declaration)? else {
                return Ok(false);
            };
            let Some(global) = self.global_value_symbol("Promise", &recipe.location)? else {
                return Ok(false);
            };
            let constructor = sequence.read(
                self.reference_resolved_identifier_symbol_completion(constructor, &recipe.location),
            )?;
            Ok(self.reference_symbol_identity(global, &recipe.location)?
                == self.reference_symbol_identity(constructor, &recipe.location)?)
        })();
        sequence.finish(result)
    }
}
