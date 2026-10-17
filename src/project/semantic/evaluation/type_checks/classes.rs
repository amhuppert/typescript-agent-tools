//! Class expressions acquire their immediate check dependencies in native order.
//! Deferred member checking belongs to whole-file checking, not this query phase.
use super::super::super::model::{
    NativeCompletion, NativeResult, NativeSequence, TypeFlags, TypeId,
};
use super::super::super::source::{
    DeclarationIdentity, ExpressionRoot, class_checks::ClassCheckRecipe,
};
use super::super::{Evaluator, emit_helpers::ExternalEmitHelpers, reads::FlowReferenceQuestion};
use crate::analysis::SemanticFailureReason;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    /// `checkClassExpression`: immediate class checks, deferred scheduling,
    /// external helpers, then completed declaration symbol and value type.
    pub(in crate::project::semantic::evaluation) fn check_class_expression_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.class_check_recipe(root)?;
            sequence.read(self.check_class_like_declaration_completion(&recipe, consumer))?;
            // `checkNodeDeferred` only queues work for checkSourceFile. Query
            // evaluation never drains that queue, as for function expressions.
            if let Some((site, computed)) =
                self.source.class_static_helper_site(recipe.declaration)?
            {
                let location = self.source.declaration_location(site)?;
                sequence.read(self.check_external_emit_helpers_completion(
                    site,
                    ExternalEmitHelpers::SET_FUNCTION_NAME,
                    &location,
                ))?;
                if computed {
                    sequence.read(self.check_external_emit_helpers_completion(
                        site,
                        ExternalEmitHelpers::PROP_KEY,
                        &location,
                    ))?;
                }
            }
            let symbol = sequence
                .read(self.reference_symbol_of_declaration_completion(recipe.declaration))?;
            sequence.read(self.value_reference_type_completion(symbol, consumer))
        })();
        sequence.finish(result)
    }

    /// `checkClassLikeDeclaration` owns the complete immediate schedule. The
    /// excluded decorator checking interrupts at its native position; a class
    /// factory cannot bypass the required immediate heritage operations.
    fn check_class_like_declaration_completion(
        &mut self,
        recipe: &ClassCheckRecipe,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            // Grammar and declaration-name collisions have diagnostic-only
            // effects here. Eligible decorators have semantic dependencies.
            sequence.read(self.check_class_expression_decorators_completion(recipe))?;
            for &parameter in &recipe.parameters {
                sequence.read(self.check_type_source_element_completion(parameter))?;
            }
            let symbol = sequence
                .read(self.reference_symbol_of_declaration_completion(recipe.declaration))?;
            self.check_variable_merged_export_dependencies(recipe.declaration, symbol, consumer)?;
            let class_type =
                sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
            let type_with_this = sequence.read(
                self.get_type_with_this_argument_completion(class_type, None, false, consumer),
            )?;
            let static_type =
                sequence.read(self.value_reference_type_completion(symbol, consumer))?;
            // `getClassOrInterfaceDeclarationsOfSymbol` excludes expressions.
            // Multiple declaration lists still require the full agreement check.
            let declarations = self.reference_symbol_declarations(symbol, consumer)?;
            if declarations.len() != 1 {
                let mut count = 0;
                for declaration in declarations {
                    if matches!(
                        self.reference_declaration_kind(declaration)?,
                        K::ClassDeclaration | K::InterfaceDeclaration
                    ) {
                        count += 1;
                    }
                }
                if count > 1 {
                    return Err(super::super::super::unsupported_demand_at_location(
                        self.inputs,
                        consumer,
                        SemanticFailureReason::UnsupportedSemanticForm,
                        "native merged class type-parameter agreement checking is pending",
                    ));
                }
            }
            self.check_type_member_overload_symbol(symbol, consumer)?;
            sequence.read(
                self.check_object_type_for_duplicate_declarations_completion(
                    recipe.declaration,
                    &recipe.members,
                    true,
                    consumer,
                ),
            )?;
            if !recipe.ambient && !self.inputs.semantic_options().use_define_for_class_fields {
                for &member in &recipe.members {
                    let declaration = self.source.type_node_declaration(member)?;
                    if !self.source.declaration_is_static(declaration)? {
                        continue;
                    }
                    if let Some((file, site)) = self.source.declaration_name_site(declaration)?
                        && let Some(key) = sequence.read(
                            self.effective_property_name_completion(ExpressionRoot { file, site }),
                        )?
                        && matches!(
                            key.fixed_bytes(),
                            Some(b"name" | b"length" | b"caller" | b"arguments")
                        )
                    {
                        let class = sequence.read(
                            self.reference_symbol_of_declaration_completion(recipe.declaration),
                        )?;
                        sequence
                            .read(self.diagnostic_symbol_display_completion(class, consumer))?;
                    }
                }
            }
            sequence.read(self.check_class_heritage_completion(
                recipe,
                class_type,
                type_with_this,
                static_type,
                consumer,
            ))?;
            sequence.read(
                self.check_index_constraints_completion(class_type, symbol, false, consumer),
            )?;
            sequence.read(self.check_index_constraints_completion(
                static_type,
                symbol,
                true,
                consumer,
            ))?;
            sequence.read(
                self.check_class_duplicate_indexes_completion(recipe.declaration, consumer),
            )?;
            sequence.read(self.check_class_property_initialization_completion(recipe, consumer))?;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkDecorators` / `markLinkedReferences(Decorator)` retain the native
    /// prefix before the profile's excluded `checkDecorator` operation.
    /// ClassExpression has no metadata type-node arm in
    /// `markDecoratorAliasReferenced`; its Metadata helper still precedes the
    /// boundary. NodeCanBeDecorated has already excluded legacy expressions.
    fn check_class_expression_decorators_completion(
        &mut self,
        recipe: &ClassCheckRecipe,
    ) -> NativeResult<()> {
        let Some(&decorator) = recipe.decorators.first() else {
            return Ok(NativeCompletion::clean(()));
        };
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let declaration = self.source.type_node_declaration(decorator)?;
            let location = self.source.declaration_location(declaration)?;
            if self
                .inputs
                .semantic_options()
                .decorators_need_emit_helpers()
            {
                sequence.read(self.check_external_emit_helpers_completion(
                    declaration,
                    ExternalEmitHelpers::DECORATE,
                    &location,
                ))?;
            }
            let options = self.inputs.semantic_options();
            if !options.verbatim_module_syntax && !recipe.ambient && options.emit_decorator_metadata
            {
                sequence.read(self.check_external_emit_helpers_completion(
                    declaration,
                    ExternalEmitHelpers::METADATA,
                    &location,
                ))?;
            }
            Err(super::super::super::unsupported_demand_at_location(
                self.inputs,
                &location,
                SemanticFailureReason::DecoratorTypingRequired,
                "native class decorator checking is outside the supported type profile",
            ))
        })();
        sequence.finish(result)
    }

    /// `checkClassOrInterfaceForDuplicateIndexSignatures` sets its native
    /// once-only bit before entering the index checker, preserving reentry.
    fn check_class_duplicate_indexes_completion(
        &mut self,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<()> {
        self.reference_symbol_of_declaration_completion(declaration)?
            .and_then(|symbol| {
                let identity = self.reference_symbol_identity(symbol, consumer)?;
                if let Some(result) = self.type_checks.class_indexes.get(&identity) {
                    return result.clone();
                }
                self.type_checks
                    .class_indexes
                    .insert(identity, Ok(NativeCompletion::clean(())));
                let result = self
                    .reference_symbol_of_declaration_completion(declaration)?
                    .and_then(|symbol| {
                        self.check_type_for_duplicate_index_signatures_completion(symbol, consumer)
                    });
                self.type_checks
                    .class_indexes
                    .insert(identity, result.clone());
                result
            })
    }

    /// `checkPropertyInitialization` / `isPropertyInitializedInConstructor`:
    /// only eligible properties demand types, and constructor flow starts at
    /// the native optional type with the constructor's actual ReturnFlowNode.
    fn check_class_property_initialization_completion(
        &mut self,
        recipe: &ClassCheckRecipe,
        consumer: &Location,
    ) -> NativeResult<()> {
        let options = self.inputs.semantic_options();
        if !options.strict_null_checks || !options.strict_property_initialization || recipe.ambient
        {
            return Ok(NativeCompletion::clean(()));
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for property in self.source.initialization_properties(recipe.declaration)? {
                let symbol = sequence
                    .read(self.reference_symbol_of_declaration_completion(property.declaration))?;
                let ty = sequence.read(self.value_reference_type_completion(symbol, consumer))?;
                if self
                    .model
                    .has_type_flags(ty, TypeFlags::ANY | TypeFlags::UNKNOWN)
                    || self.model.contains_undefined(ty)
                {
                    continue;
                }
                if let Some(constructor) = recipe.constructor {
                    sequence.read(self.property_initialized_in_constructor_completion(
                        property.name,
                        property.argument,
                        ty,
                        constructor,
                        consumer,
                    ))?;
                }
                // The uninitialized-property diagnostic formats written name
                // syntax only, and therefore adds no semantic dependency.
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `isPropertyInitializedInConstructor` is shared by strict initialization
    /// and define-field override checks. The latter deliberately supplies the
    /// class type as propType, exactly as the native caller does.
    pub(super) fn property_initialized_in_constructor_completion(
        &mut self,
        name: ExpressionRoot,
        argument: Option<ExpressionRoot>,
        ty: TypeId,
        constructor: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let flow = self
                .inputs
                .bindings
                .flow_graph(constructor.file)
                .map_err(|error| self.generic_binding_failure(error, consumer))?
                .container_at(constructor.site)
                .map_err(super::super::super::internal)?
                .return_flow;
            let reference =
                self.new_this_initialization_reference(name, argument, constructor, flow);
            let initial = sequence.read(self.optional_type_completion(ty, false, consumer))?;
            let ty = sequence.read(self.flow_type_of_reference_completion(
                FlowReferenceQuestion {
                    file: constructor.file,
                    reference,
                    declared: ty,
                    initial,
                    admitted: initial,
                    container: None,
                    flow: None,
                },
                consumer,
            ))?;
            Ok(!self.model.contains_undefined(ty))
        })();
        sequence.finish(result)
    }
}
