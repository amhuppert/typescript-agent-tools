use super::*;

impl Evaluator<'_, '_> {
    /// `checkDeprecatedSignature` / `addDeprecatedSuggestionWorker`. The
    /// suggestion's syntax/name formatting has no checker dependencies. Its
    /// signature serialization and subsequent lookup on the original
    /// declaration do, in that order, even when confirmation used an ancestor.
    pub(in crate::project::semantic::evaluation) fn check_deprecated_signature_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<()> {
        if signature.flags.is_overload_failure_candidate() {
            return Ok(NativeCompletion::clean(()));
        }
        let Some(root) = self.signature_declaration_root(signature.id)? else {
            return Ok(NativeCompletion::clean(()));
        };
        let declaration = self.source.signature_declaration_facts(root)?.declaration;
        if self.source.deprecated_declaration(declaration)? {
            return self
                .diagnostic_signature_display_completion(signature, consumer)?
                .and_then_value(|_| {
                    self.add_deprecated_suggestion_declarations(&[
                        super::super::ReferenceDeclaration::Captured(declaration),
                    ])
                });
        }
        Ok(NativeCompletion::clean(()))
    }

    /// `addDeprecatedSuggestionWorker` scans the original declarations again
    /// for the first attached tag. Confirmation may have used an ancestor and
    /// cannot substitute for this distinct native read. Message/range formatting
    /// does not acquire checker types.
    pub(in crate::project::semantic::evaluation) fn add_deprecated_suggestion_declarations(
        &mut self,
        declarations: &[super::super::ReferenceDeclaration],
    ) -> Result<(), DemandError> {
        for &declaration in declarations {
            match declaration {
                super::super::ReferenceDeclaration::Captured(declaration) => {
                    if self.source.jsdoc_deprecated_tag(declaration)?.is_some() {
                        break;
                    }
                }
                super::super::ReferenceDeclaration::Json { .. } => {}
            }
        }
        Ok(())
    }
}
