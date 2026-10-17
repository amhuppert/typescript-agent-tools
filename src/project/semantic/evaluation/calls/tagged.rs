//! Tagged calls share signature selection, but acquire different arguments.

use super::{
    CallArgument, CallInput, CallRecipe, CheckMode, Evaluator, PreparedCall, ResolvedSignature,
    SignaturePreparation,
};
use crate::project::semantic::model::{NativeResult, NativeSequence, TypeId};
use crate::project::semantic::source::Expression;

impl Evaluator<'_, '_> {
    /// `checkTaggedTemplateExpression` always resolves in Normal mode, then
    /// performs deprecation checking before acquiring the selected return.
    pub(super) fn check_tagged_template_expression_completion(
        &mut self,
        recipe: &CallRecipe,
    ) -> NativeResult<TypeId> {
        self.resolved_signature_completion(recipe, CheckMode::NORMAL)?
            .and_then(|resolved| match resolved {
                ResolvedSignature::Signature(signature) => self
                    .check_deprecated_signature_completion(&signature, &recipe.location)?
                    .and_then(|()| {
                        self.get_return_type_of_signature_completion(&signature, &recipe.location)
                    }),
                ResolvedSignature::Deferred => Err(super::super::super::internal(
                    "Normal tagged-template resolution returned resolvingSignature",
                )
                .into()),
            })
    }

    /// `resolveTaggedTemplateExpression` does not perform the ordinary call's
    /// nonnull operation or generic-function skipping. Its array-parent
    /// diagnostic branch also avoids the invocation display dependencies.
    pub(super) fn resolve_tagged_template_expression_completion(
        &mut self,
        recipe: &CallRecipe,
        mode: CheckMode,
    ) -> NativeResult<ResolvedSignature> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let tag_type = sequence
                .read(self.check_expression_completion(&recipe.callee, CheckMode::NORMAL))?;
            let apparent = self.apparent_type(tag_type, &recipe.callee.location)?;
            if self.model.is_error_type(apparent) {
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(
                        sequence.read(self.resolve_error_call_completion(recipe))?,
                    ),
                ));
            }
            let calls = self.call_signatures(apparent, &recipe.location)?;
            let constructs = self.construct_signatures(apparent, &recipe.location)?;
            if sequence.read(self.is_untyped_function_call_completion(
                tag_type,
                apparent,
                calls.len(),
                constructs.len(),
                &recipe.location,
            ))? {
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(
                        sequence.read(self.resolve_untyped_call_completion(recipe))?,
                    ),
                ));
            }
            if calls.is_empty() {
                if !self.source.call_is_array_element(recipe.root)? {
                    sequence.read(self.invocation_error_dependencies_completion(
                        apparent,
                        false,
                        &recipe.callee.location,
                    ))?;
                }
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(
                        sequence.read(self.resolve_error_call_completion(recipe))?,
                    ),
                ));
            }
            Ok(SignaturePreparation::Call(PreparedCall {
                signatures: calls,
                chain: None,
                mode,
                construct_via_call: false,
            }))
        })();
        let prepared = sequence.finish(result)?;
        self.resolve_prepared_signature_completion(recipe, prepared)
    }

    /// `getEffectiveCallArguments` creates a fresh synthetic expression whose
    /// parent and location are the actual template, followed by captured
    /// substitution expressions. It never expands these substitutions as
    /// ordinary call spread arguments. A recovered global empty-object value
    /// still constructs these arguments so signature checking can continue.
    pub(super) fn effective_tagged_template_arguments_completion<'a>(
        &mut self,
        recipe: &'a CallRecipe,
        template: &Expression,
    ) -> NativeResult<CallInput<'a>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let root = template.root.ok_or_else(|| {
                super::super::super::internal("tagged template has no captured expression root")
            })?;
            let ty = sequence.read(
                self.global_object_type_completion("TemplateStringsArray", &template.location),
            )?;
            let first = CallArgument::Synthetic {
                ty,
                node: self.new_synthetic_context_node(root),
                location: template.location.clone(),
                spread: false,
                label: None,
            };
            let mut arguments = vec![first];
            arguments.extend(
                self.source
                    .tagged_template_substitutions(root)?
                    .into_iter()
                    .map(CallArgument::Captured),
            );
            Ok(CallInput { recipe, arguments })
        })();
        sequence.finish(result)
    }
}
