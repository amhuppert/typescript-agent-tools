//! Native type-member source checks are distinct from lazy member inventories.
use super::super::super::model::{DemandError, NativeResult, NativeSequence, TypeId};
use super::super::super::source::{DeclarationIdentity, ExpressionRoot, TypeExpression};
use super::super::{Evaluator, relations::RelationKind};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    /// `checkTypeLiteral`: children precede inventory, index compatibility,
    /// duplicate index declarations, and the object-member duplicate rescan.
    pub(super) fn check_type_literal_completion(
        &mut self,
        members: &[ExpressionRoot],
        node: &TypeExpression,
        declaration: DeclarationIdentity,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for &member in members {
                sequence.read(self.check_type_source_element_completion(member))?;
            }
            let ty = sequence.read(self.check_type_completion(node))?;
            let symbol = self.source.raw_symbol_of_declaration(declaration)?;
            sequence.read(self.check_index_constraints_completion(
                ty,
                symbol,
                false,
                &node.location,
            ))?;
            sequence.read(
                self.check_type_for_duplicate_index_signatures_completion(symbol, &node.location),
            )?;
            sequence.read(
                self.check_object_type_for_duplicate_declarations_completion(
                    declaration,
                    members,
                    false,
                    &node.location,
                ),
            )
        })();
        sequence.finish(result)
    }

    /// `checkPropertySignature` / `checkPropertyDeclaration`: grammar's
    /// eligible dynamic-name acquisition precedes the variable-like worker.
    pub(super) fn check_property_signature_completion(
        &mut self,
        root: ExpressionRoot,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let facts = self.source.type_member_check(root)?;
            if let Some(name) = facts.grammar_name {
                sequence.read(self.literal_type_of_property_name_completion(root.file, name))?;
            }
            sequence
                .read(self.check_variable_like_declaration_completion(facts.declaration))
                .map(|_| ())
        })();
        sequence.finish(result)
    }

    /// `checkVariableLikeDeclaration` keeps binding-element checks, nested
    /// source checks and initializer reporting in their native order. Pattern
    /// declarations leave before the canonical-symbol path.
    pub(super) fn check_variable_like_declaration_completion(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let facts = self.source.variable_like_check(declaration)?;
            let consumer = self.source.declaration_location(declaration)?;
            let root = self.source.declaration_expression_root(declaration)?;
            if let Some(decorator) = facts.decorator {
                return Err(super::super::super::unsupported_demand_at_location(
                    self.inputs,
                    &super::super::super::location(
                        self.inputs,
                        decorator.file,
                        decorator.site.span,
                    ),
                    crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
                    "native declaration decorator checking is pending",
                ));
            }
            if facts.name.is_none() {
                return Ok(());
            }
            if let Some(annotation) = facts.annotation {
                sequence.read(self.check_type_source_element_completion(annotation))?;
            }
            if let Some(name) = facts.computed_name {
                sequence
                    .read(self.check_computed_property_name_completion(declaration.file, name))?;
                if let Some(initializer) = facts.initializer {
                    sequence.read(
                        self.check_expression_cached_completion(initializer.file, initializer.site),
                    )?;
                }
            }
            if let Some(binding) = &facts.binding {
                if binding.renamed_in_signature {
                    return Ok(());
                }
                if binding.object
                    && binding.rest
                    && !self.inputs.semantic_options().target_supports_object_rest()
                {
                    self.check_external_emit_helpers(
                        declaration,
                        super::super::emit_helpers::ExternalEmitHelpers::REST,
                        &consumer,
                    )?;
                }
                if let Some(name) = binding.computed_property {
                    sequence
                        .read(self.check_computed_property_name_completion(name.file, name.site))?;
                }
                let mode = if binding.rest {
                    super::super::contextual::CheckMode::REST_BINDING_ELEMENT
                } else {
                    super::super::contextual::CheckMode::NORMAL
                };
                if let Some(parent) =
                    sequence.read(self.binding_parent_type_completion(binding.parent, mode))?
                    && let Some(key) = binding.key
                {
                    let key = sequence
                        .read(self.literal_type_of_property_name_completion(key.file, key.site))?;
                    if let Some(key) = self.property_key_from_type(key)?
                        && let Some(property) = sequence.read(
                            self.property_of_type_by_key_completion(parent, &key, &consumer),
                        )?
                    {
                        let symbol = property.symbol.ok_or_else(|| {
                            super::super::super::internal(
                                "binding accessibility has no property symbol",
                            )
                        })?;
                        // markPropertyAsReferenced(prop,nil,false) updates only
                        // unused-reference bookkeeping; it has no type reads.
                        sequence.read(self.check_property_accessibility_completion(
                            root,
                            binding.super_initializer,
                            false,
                            parent,
                            symbol,
                            &consumer,
                        ))?;
                    }
                }
            }
            if let Some(pattern) = &facts.pattern {
                for &element in &pattern.elements {
                    sequence.read(self.check_type_source_element_completion(element))?;
                }
            }
            if facts.initializer.is_some() && facts.parameter_without_body {
                return Ok(());
            }
            if let Some(pattern) = &facts.pattern {
                if pattern.ambient {
                    return Ok(());
                }
                let check_initializer = facts.initializer.is_some() && !facts.for_in;
                if check_initializer || pattern.empty {
                    let widened = sequence
                        .read(self.widened_variable_like_type_ex_completion(declaration, false))?;
                    if check_initializer {
                        let initializer = facts.initializer.expect("initializer gate");
                        let initial = sequence.read(self.check_expression_cached_completion(
                            initializer.file,
                            initializer.site,
                        ))?;
                        if self.inputs.semantic_options().strict_null_checks && pattern.empty {
                            sequence.read(
                                self.check_non_null_non_void_type_completion(initial, &consumer),
                            )?;
                        } else {
                            let widened = sequence.read(
                                self.widened_variable_like_type_ex_completion(declaration, false),
                            )?;
                            sequence.read(
                                self.check_declaration_initializer_assignability_completion(
                                    initial,
                                    widened,
                                    root,
                                    initializer,
                                    &consumer,
                                ),
                            )?;
                        }
                    }
                    if pattern.empty {
                        if pattern.array {
                            sequence.read(self.checked_iteration_type_completion(
                                widened,
                                super::super::iteration::IterationUse::Destructuring,
                                Some(root.into()),
                                &consumer,
                            ))?;
                        } else if self.inputs.semantic_options().strict_null_checks {
                            sequence.read(
                                self.check_non_null_non_void_type_completion(widened, &consumer),
                            )?;
                        }
                    }
                }
                return Ok(());
            }
            let symbol =
                sequence.read(self.reference_symbol_of_declaration_completion(declaration))?;
            let raw = sequence.read(self.value_reference_type_completion(symbol, &consumer))?;
            let ty = self.convert_auto_to_any(raw, &consumer)?;
            let primary = matches!(self.reference_symbol_value_declaration(symbol, &consumer)?,
                Some(super::super::ReferenceDeclaration::Captured(value)) if value == declaration);
            if primary {
                if let Some(initializer) = facts.initializer.filter(|_| !facts.for_in) {
                    let initial = sequence.read(
                        self.check_expression_cached_completion(initializer.file, initializer.site),
                    )?;
                    sequence.read(self.check_declaration_initializer_assignability_completion(
                        initial,
                        ty,
                        root,
                        initializer,
                        &consumer,
                    ))?;
                    sequence.read(self.check_using_initializer_completion(
                        declaration,
                        initial,
                        initializer,
                        &consumer,
                    ))?;
                }
            } else {
                let local = sequence
                    .read(self.widened_variable_like_type_ex_completion(declaration, false))?;
                let local = self.convert_auto_to_any(local, &consumer)?;
                if !self.model.is_error_type(ty)
                    && !self.model.is_error_type(local)
                    && !sequence.read(self.type_related_completion(
                        ty,
                        local,
                        RelationKind::Identity,
                        &consumer,
                    ))?
                    && !self.reference_symbol_flags(symbol, &consumer)?.assignment
                {
                    self.source.diagnostic_written_name(declaration)?;
                    sequence.read(self.check_diagnostic_type_display_completion(ty, &consumer))?;
                    sequence
                        .read(self.check_diagnostic_type_display_completion(local, &consumer))?;
                }
                if let Some(initializer) = facts.initializer {
                    let initial = sequence.read(
                        self.check_expression_cached_completion(initializer.file, initializer.site),
                    )?;
                    sequence.read(self.check_declaration_initializer_assignability_completion(
                        initial,
                        local,
                        root,
                        initializer,
                        &consumer,
                    ))?;
                }
            }
            // Modifier agreement and name collisions acquire no types. Alias
            // declaration-space lookup in the merged-export tail remains a
            // separate symbol dependency, never an inferred successful check.
            if !matches!(
                self.source.declaration_kind(declaration)?,
                K::PropertyDeclaration | K::PropertySignature
            ) {
                self.check_variable_merged_export_dependencies(declaration, symbol, &consumer)?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// Semantic dependency boundary of `checkExportsOnMergedDeclarations`.
    /// Ordinary declaration spaces are retained by the binder; alias spaces
    /// require the checker's resolveAlias traversal when that family is reached.
    pub(super) fn check_variable_merged_export_dependencies(
        &mut self,
        declaration: DeclarationIdentity,
        symbol: super::super::ReferenceSymbol,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        let local = self.reference_declaration_local_symbol(declaration)?;
        let symbol = match local {
            Some(local) => local,
            None if self.reference_export_symbol(symbol, consumer)?.is_some() => symbol,
            None => return Ok(()),
        };
        let kind = self.source.declaration_kind(declaration)?;
        let declarations = self.reference_symbol_declarations(symbol, consumer)?;
        for &item in &declarations {
            if self.reference_declaration_kind(item)? == kind {
                if item != super::super::ReferenceDeclaration::Captured(declaration) {
                    return Ok(());
                }
                break;
            }
        }
        for item in declarations {
            if matches!(
                self.reference_declaration_kind(item)?,
                K::ImportEqualsDeclaration
                    | K::NamespaceImport
                    | K::ImportClause
                    | K::ExportAssignment
                    | K::BinaryExpression
            ) {
                return Err(super::super::super::unsupported_demand_at_location(
                    self.inputs,
                    consumer,
                    crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
                    "native merged-export alias declaration-space checking is pending",
                ));
            }
        }
        Ok(())
    }

    /// `checkTypeAssignableToAndOptionallyElaborate` at a declaration keeps
    /// its diagnostic node distinct from the initializer expression.
    fn check_declaration_initializer_assignability_completion(
        &mut self,
        initial: TypeId,
        target: TypeId,
        declaration: ExpressionRoot,
        initializer: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.check_type_related_to_and_optionally_elaborate_completion(
            initial,
            target,
            RelationKind::Assignable,
            super::super::relations::RelationReporting::Report.assignment_at(declaration),
            Some(initializer),
            consumer,
        )
    }

    /// The using branches of `checkVariableLikeDeclaration` test the actual
    /// checked global sentinels before constructing the nullable target.
    fn check_using_initializer_completion(
        &mut self,
        declaration: DeclarationIdentity,
        initial: TypeId,
        initializer: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<()> {
        use crate::compiler::node::{AWAIT_USING, CONST, LET, USING};
        let flags = self.source.constant_declaration_flags(declaration)? & (LET | CONST | USING);
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut types = Vec::new();
            if flags == AWAIT_USING {
                types.push(
                    sequence.read(self.global_disposable_identity_completion(true, consumer))?,
                );
                types.push(
                    sequence.read(self.global_disposable_identity_completion(false, consumer))?,
                );
            } else if flags == USING {
                types.push(
                    sequence.read(self.global_disposable_identity_completion(false, consumer))?,
                );
            } else {
                return Ok(());
            }
            if types.iter().any(|&ty| ty == self.model.empty_object_type()) {
                return Ok(());
            }
            types.push(
                self.model
                    .special(super::super::super::model::SpecialName::Null),
            );
            types.push(
                self.model
                    .special(super::super::super::model::SpecialName::Undefined),
            );
            let optional =
                sequence.read(self.union_type_ex_completion(types, None, None, consumer))?;
            let source =
                sequence.read(self.widen_type_for_variable_like_declaration_completion(
                    Some(initial),
                    declaration,
                    false,
                ))?;
            sequence.read(
                self.check_type_related_to_completion(
                    source,
                    optional,
                    RelationKind::Assignable,
                    super::super::relations::RelationReporting::Report
                        .using_initializer_at(initializer, flags == AWAIT_USING),
                    consumer,
                ),
            )?;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkMethodDeclaration` / `checkFunctionOrMethodDeclaration`: the
    /// computed-name and overload questions follow signature source checks,
    /// and the return annotation is acquired even for a bodyless method.
    pub(super) fn check_method_signature_completion(
        &mut self,
        root: ExpressionRoot,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let facts = self.source.type_member_check(root)?;
            if let Some(name) = facts.grammar_name {
                sequence.read(self.literal_type_of_property_name_completion(root.file, name))?;
            }
            let signature = facts
                .signature
                .ok_or_else(|| super::super::super::internal("method check lacks signature"))?;
            sequence.read(self.check_signature_declaration_completion(signature))?;
            if let Some(name) = facts.computed_name {
                sequence.read(self.literal_type_of_property_name_completion(root.file, name))?;
            }
            let consumer = self.source.declaration_location(facts.declaration)?;
            if self.has_bindable_declaration_name(facts.declaration)? {
                let symbol = sequence
                    .read(self.reference_symbol_of_declaration_completion(facts.declaration))?;
                self.check_type_member_overload_symbol(symbol, &consumer)?;
                if self.reference_symbol_parent(symbol, &consumer)?.is_some() {
                    self.check_type_member_overload_symbol(symbol, &consumer)?;
                }
            }
            let recipe = self.source.signature_from_root(signature)?;
            sequence.read(self.return_type_from_annotation_completion(&recipe.return_type))?;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkFunctionOrConstructorSymbol` installs its checked bit before
    /// entering the declaration worker. Consumed failures remain attached to
    /// that same question instead of becoming a later healthy cache hit.
    pub(super) fn check_type_member_overload_symbol(
        &mut self,
        symbol: super::super::ReferenceSymbol,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        if let Some(result) = self.type_checks.functions.get(&identity) {
            return result.clone();
        }
        self.type_checks.functions.insert(identity, Ok(()));
        let result = (|| {
            let declarations = self
                .reference_symbol_declarations(symbol, consumer)?
                .into_iter()
                .filter_map(|declaration| match declaration {
                    super::super::ReferenceDeclaration::Captured(declaration) => Some(declaration),
                    super::super::ReferenceDeclaration::Json { .. } => None,
                })
                .collect::<Vec<_>>();
            if self
                .source
                .type_member_overload_has_implementation(&declarations)?
            {
                return Err(super::super::super::unsupported_demand_at_location(
                    self.inputs,
                    consumer,
                    crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
                    "native overload implementation compatibility requires its function declaration source-check owner",
                ));
            }
            Ok(())
        })();
        self.type_checks.functions.insert(identity, result.clone());
        result
    }

    /// `checkAccessorDeclaration` acquires the canonical accessor-pair type
    /// after checking this declaration's signature and computed name.
    pub(super) fn check_accessor_declaration_completion(
        &mut self,
        root: ExpressionRoot,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let facts = self.source.type_member_check(root)?;
            sequence.read(self.check_signature_declaration_completion(
                facts.signature.ok_or_else(|| {
                    super::super::super::internal("accessor check lacks signature")
                })?,
            ))?;
            if let Some(name) = facts.computed_name {
                sequence.read(self.literal_type_of_property_name_completion(root.file, name))?;
            }
            let consumer = self.source.declaration_location(facts.declaration)?;
            let symbol = sequence
                .read(self.reference_symbol_of_declaration_completion(facts.declaration))?;
            sequence.read(self.value_reference_type_completion(symbol, &consumer))?;
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkObjectTypeForDuplicateDeclarations` preserves static/instance and
    /// private-name tables, constructor parameter properties and report rescans.
    pub(super) fn check_object_type_for_duplicate_declarations_completion(
        &mut self,
        owner: DeclarationIdentity,
        members: &[ExpressionRoot],
        check_private: bool,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let ambient = self.source.declaration_is_ambient(owner)?;
            let mut states = std::collections::HashMap::new();
            let mut private_names = std::collections::HashMap::new();
            for &root in members {
                let declaration = self.source.type_node_declaration(root)?;
                let (constructor, entries) = self.duplicate_member_declarations(declaration)?;
                for declaration in entries {
                    let Some(symbol) = sequence
                        .read(self.reference_node_declaration_symbol_completion(declaration))?
                    else {
                        continue;
                    };
                    let static_ = !constructor
                        && self
                            .source
                            .declaration_has_modifier(declaration, K::StaticKeyword)?;
                    let key = self.reference_symbol_key(symbol, consumer)?;
                    if !constructor
                        && !ambient
                        && static_
                        && key.fixed_bytes() == Some(b"prototype".as_slice())
                    {
                        let class = sequence
                            .read(self.reference_symbol_of_declaration_completion(owner))?;
                        sequence
                            .read(self.diagnostic_symbol_display_completion(class, consumer))?;
                    }
                    let kind = match self.source.declaration_kind(declaration)? {
                        K::Parameter | K::PropertySignature => Some(1),
                        K::PropertyDeclaration => Some(
                            if self
                                .source
                                .declaration_has_modifier(declaration, K::AccessorKeyword)?
                            {
                                2
                            } else {
                                1
                            },
                        ),
                        K::GetAccessor | K::SetAccessor => Some(2),
                        _ => None,
                    };
                    if let Some(kind) = kind
                        && self.reference_symbol_declarations(symbol, consumer)?.len() > 1
                    {
                        let state = states.entry((static_, key.clone())).or_insert(0);
                        if *state == 0 {
                            *state = kind;
                        } else if *state == 1 || *state == 2 && kind != 2 {
                            sequence.read(self.report_duplicate_member_dependencies_completion(
                                members,
                                &key,
                                Some(static_),
                                consumer,
                            ))?;
                            *state = 3;
                        }
                    }
                    if !constructor
                        && check_private
                        && self
                            .source
                            .declaration_name_site(declaration)?
                            .is_some_and(|(_, name)| name.kind == K::PrivateIdentifier)
                    {
                        let state = private_names.entry(key.clone()).or_insert(0);
                        if *state != 3 {
                            *state |= if static_ { 2 } else { 1 };
                            if *state == 3 {
                                sequence.read(
                                    self.report_duplicate_member_dependencies_completion(
                                        members, &key, None, consumer,
                                    ),
                                )?;
                            }
                        }
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `reportDuplicateMemberErrors` rescans all native member symbols before
    /// reporting matching names; symbolToString remains an eager dependency.
    fn report_duplicate_member_dependencies_completion(
        &mut self,
        members: &[ExpressionRoot],
        name: &crate::project::binding::NativePropertyKey,
        static_: Option<bool>,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for &root in members {
                let declaration = self.source.type_node_declaration(root)?;
                let (constructor, declarations) =
                    self.duplicate_member_declarations(declaration)?;
                for declaration in declarations {
                    if let Some(symbol) = sequence
                        .read(self.reference_node_declaration_symbol_completion(declaration))?
                        && self.reference_symbol_key(symbol, consumer)? == *name
                    {
                        let matches_side = if constructor {
                            true
                        } else {
                            match static_ {
                                Some(expected) => {
                                    self.source
                                        .declaration_has_modifier(declaration, K::StaticKeyword)?
                                        == expected
                                }
                                None => true,
                            }
                        };
                        if matches_side {
                            sequence.read(
                                self.diagnostic_symbol_display_completion(symbol, consumer),
                            )?;
                        }
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkObjectTypeForDuplicateDeclarations` and its reporting rescan
    /// both exclude binding-pattern parameter properties. The override checker
    /// deliberately consumes the complete parameter-property inventory.
    fn duplicate_member_declarations(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<(bool, Vec<DeclarationIdentity>), DemandError> {
        if self.source.declaration_kind(declaration)? != K::Constructor {
            return Ok((false, vec![declaration]));
        }
        let mut named = Vec::new();
        for parameter in self.source.constructor_parameter_properties(declaration)? {
            if !self
                .source
                .declaration_name_site(parameter)?
                .is_some_and(|(_, name)| {
                    matches!(name.kind, K::ObjectBindingPattern | K::ArrayBindingPattern)
                })
            {
                named.push(parameter);
            }
        }
        Ok((true, named))
    }
}
