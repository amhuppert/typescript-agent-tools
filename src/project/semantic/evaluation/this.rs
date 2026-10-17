//! Native this-expression acquisition. Its ordinary nil result means implicit
//! Any; a failed signature/class/flow dependency must remain a failure.

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, ObjectFlags, SpecialName, TypeId,
    TypeView,
};
use super::super::source::{DeclarationIdentity, ExpressionRoot};
use super::Evaluator;
use super::reads::FlowReferenceQuestion;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;

impl Evaluator<'_, '_> {
    /// `checkThisExpression`, including the semantic reads performed only to
    /// report constructor/noImplicitThis diagnostics. Diagnostic text itself
    /// does not replace the compiler's ordinary result with errorType.
    pub(super) fn check_this_expression_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let context = self.source.this_expression_context(root)?;
            if let Some(class) = context.before_super_class {
                // checkThisBeforeSuper -> classDeclarationExtendsNull reads
                // both declared class and base constructor before flow-order
                // diagnostics. Their failures cannot become a skipped report.
                sequence.read(self.class_declaration_extends_null_completion(class, consumer))?;
            }
            let ty = sequence.read(self.try_this_type_at_completion(
                root,
                true,
                Some(context.container),
                consumer,
            ))?;
            if self.inputs.semantic_options().no_implicit_this {
                let global = self.global_this_reference_symbol();
                sequence.read(self.value_reference_type_completion(global, consumer))?;
                if ty.is_none()
                    && self.source.declaration_kind(context.container)? != SyntaxKind::SourceFile
                {
                    let outside = self.source.declaration_expression_root(context.container)?;
                    sequence
                        .read(self.try_this_type_at_completion(outside, true, None, consumer))?;
                }
            }
            Ok(ty.unwrap_or_else(|| self.model.special(SpecialName::Any)))
        })();
        sequence.finish(result)
    }

    /// `tryGetThisTypeAtEx`: signature-this precedes contextual-this, then
    /// class instance/static state, then module/global source-file state.
    /// FullSignature is a JS-only branch outside the supported source profile.
    pub(super) fn try_this_type_at_completion(
        &mut self,
        root: ExpressionRoot,
        include_global: bool,
        container: Option<DeclarationIdentity>,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let context = self.source.this_type_context(root, container)?;
            if context.function {
                let recipe = self.source.signature_from_declaration(context.container)?;
                let signature = sequence.read(self.signature_completion(&recipe))?;
                let mut ty = sequence.read(self.signature_this_type_completion(&signature))?;
                if ty.is_none() {
                    let function = self.source.declaration_expression_root(context.container)?;
                    ty = sequence
                        .read(self.contextual_this_parameter_type_completion(function, consumer))?;
                }
                if let Some(ty) = ty {
                    return self.this_reference_flow_type(root, ty, consumer).map(Some);
                }
            }
            if let Some((class, static_)) = context.class {
                let symbol = self.reference_symbol_of_declaration(class)?;
                let ty = if static_ {
                    sequence.read(self.value_reference_type_completion(symbol, consumer))?
                } else {
                    let declared =
                        sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
                    self.model.target_this_parameter(declared)?.ok_or_else(|| {
                        super::super::internal("class lacks its native this parameter")
                    })?
                };
                return self.this_reference_flow_type(root, ty, consumer).map(Some);
            }
            if context.source_file {
                if context.external_module {
                    return Ok(Some(self.model.special(SpecialName::Undefined)));
                }
                if include_global {
                    let global = self.global_this_reference_symbol();
                    return sequence
                        .read(self.value_reference_type_completion(global, consumer))
                        .map(Some);
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `getThisTypeOfObjectLiteralFromContextualType` walks only directly
    /// nested property literals while a contextual type remains available.
    pub(super) fn contextual_object_this_completion(
        &mut self,
        mut literal: ExpressionRoot,
        mut contextual: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            while let Some(ty) = contextual {
                if let Some(ty) =
                    sequence.read(self.this_type_from_context_completion(ty, consumer))?
                {
                    return Ok(Some(ty));
                }
                let Some(outer) = self.source.outer_this_object_literal(literal)? else {
                    break;
                };
                literal = outer;
                contextual = sequence.read(self.apparent_contextual_type_completion(
                    literal,
                    super::contextual::ContextFlags::NONE,
                    consumer,
                ))?;
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `getThisTypeFromContextualType` uses mapType, including its Never,
    /// origin-union and nil-mapping behavior. Intersections select the first
    /// actual ThisType target, not a similarly spelled alias or property.
    fn this_type_from_context_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.map_type_optional_completion(ty, false, consumer, |checker, child| {
            let mut sequence = NativeSequence::new();
            let result = (|| {
                if let TypeView::Intersection(types) = checker.model.get(child)? {
                    for &ty in types.iter() {
                        if let Some(ty) =
                            sequence.read(checker.this_type_argument_completion(ty, consumer))?
                        {
                            return Ok(Some(ty));
                        }
                    }
                    Ok(None)
                } else {
                    sequence.read(checker.this_type_argument_completion(child, consumer))
                }
            })();
            sequence.finish(result)
        })
    }

    /// `getThisTypeArgument` observes reference target identity before
    /// acquiring arguments. Missing/wrong-kind quiet ThisType globals cannot
    /// match any native reference and do not themselves introduce recovery.
    fn this_type_argument_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        if !self
            .model
            .object_flags(ty)?
            .contains(ObjectFlags::REFERENCE)
        {
            return Ok(NativeCompletion::clean(None));
        }
        let TypeView::Reference(reference) = self.model.get(ty)? else {
            return Err(super::super::internal("native Reference lacks reference view").into());
        };
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let global = self
                .fixed_global_type_identity(super::globals::FixedGlobalType::ThisType, consumer)?;
            if global.is_none_or(|(_, ty)| ty != reference.target) {
                return Ok(None);
            }
            let arguments = sequence.read(self.get_type_arguments_completion(ty))?;
            arguments.first().copied().map(Some).ok_or_else(|| {
                super::super::internal("ThisType reference lacks its required argument").into()
            })
        })();
        sequence.finish(result)
    }

    fn this_reference_flow_type(
        &mut self,
        root: ExpressionRoot,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.flow_type_of_reference(
            FlowReferenceQuestion {
                file: root.file,
                reference: root.into(),
                declared: ty,
                initial: ty,
                admitted: ty,
                container: None,
                flow: None,
            },
            consumer,
        )
    }
}
