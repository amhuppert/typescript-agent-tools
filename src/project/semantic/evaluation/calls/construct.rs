//! Native checked construction shares overload resolution with ordinary calls.
//! Accessibility and abstractness precede argument acquisition; call-only
//! signatures have their separate checkCallExpression result policy.
use super::super::super::model::ObjectFlags;
use super::super::super::source::DeclarationIdentity;
use super::super::native_display::DiagnosticTypeFormat;
use super::super::{ReferenceDeclaration, ReferenceSymbol};
use super::*;

impl Evaluator<'_, '_> {
    /// `resolveNewExpression`: checked construction deliberately does not use
    /// getQuickTypeOfExpression's single-signature return shortcut.
    pub(super) fn resolve_new_expression_completion(
        &mut self,
        recipe: &CallRecipe,
        mode: CheckMode,
    ) -> NativeResult<ResolvedSignature> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let expression = sequence
                .read(self.check_expression_completion(&recipe.callee, CheckMode::NORMAL))?;
            let expression = self.check_non_null_type(expression, &recipe.callee.location)?;
            if self.model.is_silent_never(expression) {
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(self.silent_never_signature()?),
                ));
            }
            let expression = self.apparent_type(expression, &recipe.callee.location)?;
            if self.model.is_error_type(expression) {
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(
                        sequence.read(self.resolve_error_call_completion(recipe))?,
                    ),
                ));
            }
            if self.model.has_type_flags(expression, TypeFlags::ANY) {
                return Ok(SignaturePreparation::Resolved(
                    ResolvedSignature::Signature(
                        sequence.read(self.resolve_untyped_call_completion(recipe))?,
                    ),
                ));
            }
            let constructors = self.construct_signatures(expression, &recipe.location)?;
            if let Some(first) = constructors.first() {
                if !sequence.read(self.constructor_accessible_completion(recipe, first))?
                    || self.some_abstract_signature(&constructors)
                {
                    return Ok(SignaturePreparation::Resolved(
                        ResolvedSignature::Signature(
                            sequence.read(self.resolve_error_call_completion(recipe))?,
                        ),
                    ));
                }
                if let Some(symbol) = self.reference_symbol_of_type(expression, &recipe.location)?
                    && let Some(declaration) =
                        self.reference_class_declaration(symbol, &recipe.location)?
                    && self.source.class_declaration_is_abstract(declaration)?
                {
                    return Ok(SignaturePreparation::Resolved(
                        ResolvedSignature::Signature(
                            sequence.read(self.resolve_error_call_completion(recipe))?,
                        ),
                    ));
                }
                return Ok(SignaturePreparation::Call(PreparedCall {
                    signatures: constructors,
                    chain: None,
                    mode,
                    construct_via_call: false,
                }));
            }
            let calls = self.call_signatures(expression, &recipe.location)?;
            if !calls.is_empty() {
                return Ok(SignaturePreparation::Call(PreparedCall {
                    signatures: calls,
                    chain: None,
                    mode,
                    construct_via_call: true,
                }));
            }
            sequence.read(self.invocation_error_dependencies_completion(
                expression,
                true,
                &recipe.callee.location,
            ))?;
            Ok(SignaturePreparation::Resolved(
                ResolvedSignature::Signature(
                    sequence.read(self.resolve_error_call_completion(recipe))?,
                ),
            ))
        })();
        let prepared = sequence.finish(result)?;
        self.resolve_prepared_signature_completion(recipe, prepared)
    }

    /// resolveNewExpression's call-signature fallback runs after resolveCall
    /// has installed and reported any overload recovery. These later reads do
    /// not belong to that earlier resolved-signature cache entry.
    pub(super) fn finish_new_call_signature_completion(
        &mut self,
        recipe: &CallRecipe,
        signature: Signature,
    ) -> NativeResult<Signature> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self.inputs.semantic_options().no_implicit_any {
                if self.signature_declaration_root(signature.id)?.is_some() {
                    sequence.read(
                        self.get_return_type_of_signature_completion(&signature, &recipe.location),
                    )?;
                }
                if let Some(this) = signature.this_parameter() {
                    sequence.read(self.demand_completion(this.value_type))?;
                }
            }
            Ok(signature)
        })();
        sequence.finish(result)
    }

    pub(in crate::project::semantic::evaluation) fn reference_class_declaration(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let declarations = self.reference_symbol_declarations(symbol, consumer)?;
        let captured = declarations
            .into_iter()
            .filter_map(|declaration| match declaration {
                ReferenceDeclaration::Captured(declaration) => Some(
                    super::super::super::source::DeclarationReference::Captured(declaration),
                ),
                ReferenceDeclaration::Json { .. } => None,
            });
        match self.source.first_class_declaration(captured)? {
            Some(super::super::super::source::DeclarationReference::Captured(declaration)) => {
                Ok(Some(declaration))
            }
            None => Ok(None),
            Some(
                super::super::super::source::DeclarationReference::Bound(_)
                | super::super::super::source::DeclarationReference::Native(_),
            ) => Err(super::super::super::internal(
                "captured class list returned a bound declaration",
            )
            .into()),
        }
    }

    /// `isConstructorAccessible` acquires the declaring class before checking
    /// ancestry and uses only the nearest containing class for protected bases.
    fn constructor_accessible_completion(
        &mut self,
        recipe: &CallRecipe,
        signature: &Signature,
    ) -> NativeResult<bool> {
        let Some(root) = self.signature_declaration_root(signature.id)? else {
            return Ok(NativeCompletion::clean(true));
        };
        let Some(access) = self.source.constructor_access(root)? else {
            return Ok(NativeCompletion::clean(true));
        };
        let symbol = self.reference_symbol_of_declaration(access.class)?;
        let declaration = self.reference_class_declaration(symbol, &recipe.location)?;
        let declaring_type = self.declared_reference_type(symbol, &recipe.location)?;
        let enclosing =
            self.source
                .enclosing_classes(super::super::super::source::ExpressionRoot {
                    file: recipe.root.file,
                    site: recipe.root.site(),
                })?;
        if declaration.is_some_and(|declaration| enclosing.contains(&declaration)) {
            return Ok(NativeCompletion::clean(true));
        }
        if access.protected
            && let Some(&containing) = enclosing.first()
        {
            let containing_symbol = self.reference_symbol_of_declaration(containing)?;
            let containing_type =
                self.declared_reference_type(containing_symbol, &recipe.location)?;
            if self.type_has_protected_accessible_base(symbol, containing_type, &recipe.location)? {
                return Ok(NativeCompletion::clean(true));
            }
        }
        // Both diagnostics independently format the declaring type. Keeping
        // this acquisition preserves hidden annotation dependencies.
        let mut checked = NativeCompletion::clean(());
        if access.private {
            checked = self
                .diagnostic_type_display_completion(
                    declaring_type,
                    None,
                    DiagnosticTypeFormat::DEFAULT,
                    &recipe.location,
                )?
                .map(|_| ());
        }
        if access.protected {
            checked = checked.and_then(|()| {
                Ok(self
                    .diagnostic_type_display_completion(
                        declaring_type,
                        None,
                        DiagnosticTypeFormat::DEFAULT,
                        &recipe.location,
                    )?
                    .map(|_| ()))
            })?;
        }
        Ok(checked.map(|()| false))
    }

    /// `typeHasProtectedAccessibleBase` follows first bases in depth-first
    /// order. Intersection bases classify all mixins before visiting only
    /// non-mixin class/interface constituents; ordinary hasBaseType differs.
    fn type_has_protected_accessible_base(
        &mut self,
        target: ReferenceSymbol,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        enum Work {
            Descend(TypeId),
            CompareThenDescend(TypeId),
        }
        let identity = self.reference_symbol_identity(target, consumer)?;
        let mut pending = vec![Work::Descend(ty)];
        while let Some(work) = pending.pop() {
            let ty = match work {
                Work::Descend(ty) => ty,
                Work::CompareThenDescend(ty) => {
                    if self.type_symbol_identity(ty)? == Some(identity) {
                        return Ok(true);
                    }
                    ty
                }
            };
            let target_type = self.model.reference_target(ty).unwrap_or(ty);
            let Some(&first) = self.base_types(target_type, consumer)?.first() else {
                continue;
            };
            if let Some(types) = self
                .model
                .intersection_constituents(first)
                .map(|types| types.to_vec())
            {
                let (mixins, _) = self.find_mixins(&types, consumer)?;
                for (&base, mixin) in types.iter().zip(mixins).rev() {
                    if !mixin
                        && self
                            .model
                            .object_flags(base)?
                            .intersects(ObjectFlags::CLASS | ObjectFlags::INTERFACE)
                    {
                        pending.push(Work::CompareThenDescend(base));
                    }
                }
            } else {
                pending.push(Work::CompareThenDescend(first));
            }
        }
        Ok(false)
    }

    /// `resolveErrorCall` still checks actual argument/type-argument children;
    /// only consuming unknownSignature's return acquires its error type.
    pub(super) fn resolve_error_call_completion(
        &mut self,
        recipe: &CallRecipe,
    ) -> NativeResult<Signature> {
        self.resolve_untyped_call_completion(recipe)?
            .and_then_value(|_| self.unknown_signature())
    }

    pub(super) fn silent_never_signature(&mut self) -> Result<Signature, DemandError> {
        if let Some(signature) = &self.calls.silent_never_signature {
            return Ok(signature.clone());
        }
        let ty = self.model.silent_never();
        let signature = self.model.signature(SignatureData {
            type_parameter_types: Vec::new().into(),
            type_parameters: Vec::new(),
            this_parameter: None,
            parameters: Vec::new().into(),
            return_type: super::super::super::model::TypeDemand::Known(ty),
            predicate: None,
            origins: Vec::new().into(),
            minimum_arguments: 0,
            flags: SignatureFlags::declared(false, false, false, false),
        })?;
        self.attach_signature_declaration(signature.id, None)?;
        self.calls.silent_never_signature = Some(signature.clone());
        Ok(signature)
    }
}
