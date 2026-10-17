use super::super::model::DemandError;
use super::{DeclarationIdentity, Source};

impl Source<'_, '_> {
    /// `checkExternalEmitHelpers`'s IsEffectiveExternalModule and raw Ambient
    /// node gates precede the helper-module question. Module detection remains
    /// the captured program fact; only the node's own flag comes from syntax.
    pub(in crate::project::semantic) fn emit_helpers_eligible(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        // TypeScript inputs have no CommonJSModuleIndicator. IsolatedModules
        // creates the program's helper request but does not make this true.
        if !self.inputs.bindings.file_is_module(declaration.file) {
            return Ok(false);
        }
        let captured = self.syntax(declaration.file)?;
        Ok(captured.ast.node(declaration.node).flags & crate::compiler::node::AMBIENT == 0)
    }
}
