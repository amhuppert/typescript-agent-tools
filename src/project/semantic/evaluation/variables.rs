//! The raw variable-like type question is distinct from symbol widening and
//! checked assignment patterns. All consumers share its native precedence.

use super::super::model::{
    AccessFlags, DemandError, IndexFlags, MembersView, Metadata, NativeCompletion, NativeResult,
    NativeSequence, ObjectFlags, ObjectKind, SpecialName, TupleElementFlags, TupleElementInfo,
    TypeFlags, TypeId, TypeSymbol, UnionReduction,
};
use super::super::source::variables::{
    AssignmentPatternRecipe, PropertyInference, VariableEarly, VariableFacts,
};
use super::super::source::{DeclarationIdentity, ExpressionRoot};
use super::contextual::CheckMode;
use super::{Evaluator, ReferenceDeclaration};
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;
use crate::types::PrimitiveName;

#[derive(Default)]
pub(super) struct VariablesState {
    contextual_patterns: Vec<DeclarationIdentity>,
    patterns: std::collections::HashMap<TypeId, DeclarationIdentity>,
}

impl Evaluator<'_, '_> {
    /// Native `patternForType` is an actual factory association, not a flag.
    pub(super) fn has_binding_pattern_type(&self, ty: TypeId) -> bool {
        self.variables.patterns.contains_key(&ty)
    }

    /// `patternForType` is shared by binding and assignment-pattern factories.
    pub(super) fn record_pattern_type(&mut self, ty: TypeId, pattern: DeclarationIdentity) {
        self.variables.patterns.insert(ty, pattern);
    }

    pub(super) fn object_context_has_pattern(&mut self, ty: TypeId) -> Result<bool, DemandError> {
        match self.variables.patterns.get(&ty).copied() {
            Some(pattern) => Ok(matches!(
                self.source.declaration_kind(pattern)?,
                K::ObjectBindingPattern | K::ObjectLiteralExpression
            )),
            None => Ok(false),
        }
    }

    /// `checkIdentifier`'s contextual-binding recursion guard precedes any
    /// narrowed/symbol type demand and uses the actual containing pattern.
    pub(super) fn is_contextual_binding_reference(
        &mut self,
        declaration: DeclarationIdentity,
        root: ExpressionRoot,
    ) -> Result<bool, DemandError> {
        if self.variables.contextual_patterns.is_empty() {
            return Ok(false);
        }
        if self.source.declaration_kind(declaration)? != K::BindingElement {
            return Ok(false);
        }
        let pattern = self.source.binding_element_facts(declaration)?.pattern;
        if !self.variables.contextual_patterns.contains(&pattern) {
            return Ok(false);
        }
        let pattern = self.source.declaration_expression_root(pattern)?;
        self.source
            .expression_is_descendant(root, pattern)
            .map_err(Into::into)
    }
    /// `getContextualTypeForInitializerExpression` asks declaration context
    /// before constructing the optional implied binding-pattern context.
    pub(super) fn contextual_variable_initializer_completion(
        &mut self,
        declaration: DeclarationIdentity,
        flags: super::contextual::ContextFlags,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(ty) =
                sequence.read(self.contextual_variable_type_completion(declaration, flags))?
            {
                return Ok(Some(ty));
            }
            if !flags.contains(super::contextual::ContextFlags::SKIP_BINDING_PATTERNS) {
                if let Some(pattern) = self.source.variable_facts(declaration)?.pattern {
                    if !self.source.pattern_elements(pattern)?.is_empty() {
                        return sequence
                            .read(self.type_from_binding_pattern_completion(pattern, true, false))
                            .map(Some);
                    }
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `getContextualTypeForVariableLikeDeclaration` is a separate question
    /// from the declaration's own inferred type and never reads its value slot.
    fn contextual_variable_type_completion(
        &mut self,
        declaration: DeclarationIdentity,
        flags: super::contextual::ContextFlags,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let facts = self.source.variable_facts(declaration)?;
            if let Some(annotation) = &facts.annotation {
                return sequence
                    .read(self.check_type_completion(annotation))
                    .map(Some);
            }
            if facts.kind == K::Parameter {
                let parameter = self.source.variable_parameter_recipe(declaration)?;
                return sequence.read(self.contextually_typed_parameter_completion(&parameter));
            }
            if facts.kind == K::BindingElement {
                let binding = self.source.binding_element_facts(declaration)?;
                if !self.source.contextual_binding_name_eligible(binding.name)? {
                    return Ok(None);
                }
                let mut parent = sequence
                    .read(self.contextual_variable_type_completion(binding.parent, flags))?;
                if parent.is_none() {
                    let parent_facts = self.source.variable_facts(binding.parent)?;
                    if parent_facts.kind != K::BindingElement && parent_facts.initializer.is_some()
                    {
                        parent =
                            Some(sequence.read(self.declaration_initializer_type_completion(
                                &parent_facts,
                                if facts.rest {
                                    CheckMode::REST_BINDING_ELEMENT
                                } else {
                                    CheckMode::NORMAL
                                },
                                None,
                            ))?);
                    }
                }
                let Some(parent) = parent else {
                    return Ok(None);
                };
                if !binding.object {
                    return sequence.read(self.contextual_element_type_completion(
                        Some(parent),
                        binding.index,
                        None,
                        None,
                        None,
                        &facts.location,
                    ));
                }
                let ty = sequence.read(self.literal_type_of_property_name_completion(
                    binding.name.file,
                    binding.name.site,
                ))?;
                return match self.property_key_from_type(ty)? {
                    Some(name) => sequence.read(self.type_of_property_of_type_by_key_completion(
                        parent,
                        &name,
                        &facts.location,
                    )),
                    None => Ok(None),
                };
            }
            if facts.kind == K::PropertyDeclaration
                && self
                    .source
                    .declaration_has_modifier(declaration, K::StaticKeyword)?
            {
                if let Some(parent) = facts.parent {
                    if let Some(root) = self.source.declaration_expression(parent)? {
                        if let Some(contextual) =
                            sequence.read(self.contextual_type_completion(root, flags))?
                        {
                            // getContextualTypeForStaticPropertyDeclaration uses
                            // the exact Symbol.Name, including generated keys.
                            let symbol = self.reference_symbol_of_declaration(declaration)?;
                            let name = self.reference_symbol_key(symbol, &facts.location)?;
                            return sequence.read(self.contextual_property_type_by_key_completion(
                                contextual,
                                &name,
                                None,
                                &facts.location,
                            ));
                        }
                    }
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `checkDeclarationInitializer` for existing callers with an initializer
    /// recipe. Its actual parent supplies pattern-padding and root facts.
    pub(super) fn check_declaration_initializer_completion(
        &mut self,
        expression: &super::super::source::Expression,
    ) -> NativeResult<TypeId> {
        let Some(root) = expression.root else {
            return self.check_expression_completion(expression, CheckMode::NORMAL);
        };
        let declaration = self.source.initializer_declaration(root)?;
        let facts = self.source.variable_facts(declaration)?;
        self.declaration_initializer_type_completion(&facts, CheckMode::NORMAL, None)
    }
    /// `getDestructuringPropertyName` / `getLiteralPropertyNameText` share
    /// exact literal acquisition between flow and synthetic destructuring.
    pub(super) fn destructuring_property_name_completion(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> NativeResult<Option<String>> {
        use super::super::source::variables::DestructuringKey;
        match self.source.destructuring_key(declaration)? {
            None => Ok(NativeCompletion::clean(None)),
            Some(DestructuringKey::Index(name)) => Ok(NativeCompletion::clean(Some(name))),
            Some(DestructuringKey::Name(name)) => self
                .literal_type_of_property_name_completion(name.file, name.site)?
                .and_then_value(|ty| {
                    if !self
                        .model
                        .has_type_flags(ty, TypeFlags::STRING_LITERAL | TypeFlags::NUMBER_LITERAL)
                    {
                        return Ok(None);
                    }
                    let name = self.property_name_from_type(ty)?.ok_or_else(|| {
                        super::super::internal("destructuring literal has no exact key")
                    })?;
                    Ok(Some(name))
                }),
        }
    }

    /// `getSyntheticElementAccess` resolves its parent before reading the key.
    /// Fresh reference nodes retain actual ancestry and the copied FlowNode.
    fn destructuring_reference(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<Option<super::reads::ReferenceNode>, DemandError> {
        use super::super::source::variables::DestructuringParent;
        let parent = match self.source.destructuring_parent(declaration)? {
            DestructuringParent::None => return Ok(None),
            DestructuringParent::Expression(root) => Some(root.into()),
            DestructuringParent::Nested(parent) => self.destructuring_reference(parent)?,
        };
        let Some(parent) = parent else {
            return Ok(None);
        };
        let consumer = self.source.declaration_location(declaration)?;
        let Some(flow) = self.reference_flow_node(parent, &consumer)? else {
            return Ok(None);
        };
        let Some(name) = self
            .destructuring_property_name_completion(declaration)?
            .admit()?
        else {
            return Ok(None);
        };
        let parenthesized = if let Some(site) = parent.captured_site() {
            !self
                .source
                .destructuring_expression_is_left_hand_side(ExpressionRoot {
                    file: parent.file,
                    site,
                })?
        } else {
            false
        };
        let root = self.source.declaration_expression_root(declaration)?;
        Ok(Some(self.new_destructuring_reference(
            parent,
            name,
            root,
            flow,
            parenthesized,
        )))
    }

    /// `getFlowTypeOfDestructuring` applies the ordinary flow interpreter and
    /// its final identity admission to the actual synthetic element reference.
    fn flow_type_of_destructuring(
        &mut self,
        declaration: DeclarationIdentity,
        declared: TypeId,
    ) -> Result<TypeId, DemandError> {
        let Some(reference) = self.destructuring_reference(declaration)? else {
            return Ok(declared);
        };
        let consumer = self.source.declaration_location(declaration)?;
        self.flow_type_of_reference(
            super::reads::FlowReferenceQuestion {
                file: declaration.file,
                reference,
                declared,
                initial: declared,
                admitted: declared,
                container: None,
                flow: None,
            },
            &consumer,
        )
    }

    /// `checkDestructuringAssignment` preserves default checking before the
    /// recursive object/array/reference dispatch.
    pub(super) fn checked_destructuring_assignment(
        &mut self,
        node: ExpressionRoot,
        mut source: TypeId,
        mode: CheckMode,
        right_is_this: bool,
    ) -> Result<TypeId, DemandError> {
        let facts = self.source.destructuring_target(node)?;
        let consumer = self.source.declaration_location(facts.declaration)?;
        let mut target = node;
        if facts.kind == K::ShorthandPropertyAssignment {
            if let Some(initializer) = facts.initializer {
                if self.inputs.semantic_options().strict_null_checks {
                    let expression = self
                        .source
                        .expression_at(initializer.file, initializer.site)?;
                    let ty = self.check_expression(&expression)?;
                    if self.type_facts(ty, super::facts::TypeFacts::ONLY_UNDEFINED, &consumer)?
                        == super::facts::TypeFacts::NONE
                    {
                        let _ = self.type_with_facts(
                            source,
                            super::facts::TypeFacts::NOT_UNDEFINED,
                            &consumer,
                        )?;
                    }
                }
                return Err(self.variable_dependency(
                    &consumer,
                    "native shorthand default binary assignment checking is pending",
                ));
            }
            target = facts
                .name
                .ok_or_else(|| super::super::internal("shorthand assignment has no target"))?;
        }
        let facts = self.source.destructuring_target(target)?;
        if let Some(left) = facts.left {
            self.check_binary_expression(target, mode, &consumer)?;
            target = left;
            if self.inputs.semantic_options().strict_null_checks {
                source = self.type_with_facts(
                    source,
                    super::facts::TypeFacts::NOT_UNDEFINED,
                    &consumer,
                )?;
            }
        }
        match target.site.kind {
            K::ObjectLiteralExpression => {
                let pattern = self.source.destructuring_target(target)?.declaration;
                let count = self.source.pattern_elements(pattern)?.len();
                if count == 0 && self.inputs.semantic_options().strict_null_checks {
                    return self.check_non_null_type(source, &consumer);
                }
                for index in 0..count {
                    self.checked_destructuring_property(target, source, index, right_is_this)?;
                }
                Ok(source)
            }
            K::ArrayLiteralExpression => {
                let pattern = self.source.destructuring_target(target)?.declaration;
                let elements = self.source.pattern_elements(pattern)?;
                let out_of_bounds = self
                    .binding_iteration_type_completion(source, true, target.into(), &consumer)?
                    .admit()?;
                let mut in_bounds = (!self.inputs.semantic_options().no_unchecked_indexed_access)
                    .then_some(out_of_bounds);
                for (index, element) in elements.iter().enumerate() {
                    let ty = if element.rest {
                        match in_bounds {
                            Some(ty) => ty,
                            None => {
                                let ty = self
                                    .binding_iteration_type_completion(
                                        source,
                                        false,
                                        target.into(),
                                        &consumer,
                                    )?
                                    .admit()?;
                                in_bounds = Some(ty);
                                ty
                            }
                        }
                    } else {
                        out_of_bounds
                    };
                    self.checked_destructuring_element(target, source, index, ty, mode)?;
                }
                Ok(source)
            }
            _ => {
                let expression = self.source.expression_at(target.file, target.site)?;
                let target_type = self.check_expression_ex(&expression, mode)?;
                if self.source.assignment_reference_is_valid(target)? {
                    self.check_type_related_to_and_optionally_elaborate(
                        source,
                        target_type,
                        super::relations::RelationKind::Assignable,
                        super::relations::RelationReporting::Report.assignment_at(target),
                        Some(target),
                        &consumer,
                    )?;
                }
                Ok(source)
            }
        }
    }

    /// `checkObjectLiteralDestructuringPropertyAssignment`: key lookup and
    /// accessibility precede indexed selection and recursive assignment.
    fn checked_destructuring_property(
        &mut self,
        pattern: ExpressionRoot,
        source: TypeId,
        index: usize,
        _right_is_this: bool,
    ) -> Result<Option<TypeId>, DemandError> {
        let declaration = self.source.destructuring_target(pattern)?.declaration;
        let elements = self.source.pattern_elements(declaration)?;
        let element = elements
            .get(index)
            .ok_or_else(|| super::super::internal("destructuring property index is absent"))?;
        let consumer = self.source.declaration_location(element.declaration)?;
        if matches!(
            element.kind,
            K::PropertyAssignment | K::ShorthandPropertyAssignment
        ) {
            let name = element
                .name
                .ok_or_else(|| super::super::internal("destructuring property has no name"))?;
            let key = self.literal_type_of_property_name(name.file, name.site)?;
            if let Some(key) = self.property_key_from_type(key)? {
                if let Some(property) = self.property_of_type_by_key(source, &key, &consumer)? {
                    let symbol = self.property_symbol(&property, &consumer)?;
                    let flags = self.property_declaration_modifiers(symbol, true, &consumer)?;
                    if flags
                        & (crate::syntax::modifiers::PRIVATE | crate::syntax::modifiers::PROTECTED)
                        != 0
                    {
                        return Err(self.variable_dependency(&consumer, "native destructuring private/protected property accessibility is pending"));
                    }
                }
            }
            let flags = AccessFlags::EXPRESSION_POSITION
                | if element.default_value {
                    AccessFlags::ALLOW_MISSING
                } else {
                    AccessFlags::NONE
                };
            let ty = self.indexed_access_at_node(source, key, flags, name.into(), &consumer)?;
            let ty = self.flow_type_of_destructuring(element.declaration, ty)?;
            let target = if element.kind == K::PropertyAssignment {
                element.initializer.ok_or_else(|| {
                    super::super::internal("property assignment has no initializer")
                })?
            } else {
                self.source
                    .declaration_expression_root(element.declaration)?
            };
            return self
                .checked_destructuring_assignment(target, ty, CheckMode::NORMAL, false)
                .map(Some);
        }
        if element.kind == K::SpreadAssignment && index + 1 == elements.len() {
            if !self.inputs.semantic_options().target_supports_object_rest() {
                self.check_external_emit_helpers(
                    element.declaration,
                    super::emit_helpers::ExternalEmitHelpers::REST,
                    &consumer,
                )?;
            }
            let names = elements
                .iter()
                .filter(|element| element.kind != K::SpreadAssignment)
                .map(|element| {
                    element.name.ok_or_else(|| {
                        super::super::internal("destructuring property lacks its actual name")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let ty = self.rest_type(source, &names, self.model.type_symbol(source), &consumer)?;
            let target = element
                .operand
                .ok_or_else(|| super::super::internal("rest assignment lacks its operand"))?;
            return self
                .checked_destructuring_assignment(target, ty, CheckMode::NORMAL, false)
                .map(Some);
        }
        // Syntax-only diagnostics return native nil; enclosing loops continue.
        Ok(None)
    }

    /// `checkArrayLiteralDestructuringElementAssignment` creates the numeric
    /// index before the array-like question and preserves rest slice identity.
    fn checked_destructuring_element(
        &mut self,
        pattern: ExpressionRoot,
        source: TypeId,
        index: usize,
        element_type: TypeId,
        mode: CheckMode,
    ) -> Result<Option<TypeId>, DemandError> {
        let declaration = self.source.destructuring_target(pattern)?.declaration;
        let elements = self.source.pattern_elements(declaration)?;
        let element = elements
            .get(index)
            .ok_or_else(|| super::super::internal("destructuring element index is absent"))?;
        let consumer = self.source.declaration_location(element.declaration)?;
        if element.kind == K::OmittedExpression {
            return Ok(None);
        }
        let root = self
            .source
            .declaration_expression_root(element.declaration)?;
        if element.kind != K::SpreadElement {
            let key = self
                .model
                .regular_literal_type(PrimitiveName::Number, index.to_string());
            let ty = if self.is_array_like_type(source, &consumer)? {
                let access_node = self.new_synthetic_context_node(root);
                let ty = self.indexed_access_at_node(
                    source,
                    key,
                    AccessFlags::EXPRESSION_POSITION
                        | if element.default_value {
                            AccessFlags::ALLOW_MISSING
                        } else {
                            AccessFlags::NONE
                        },
                    access_node,
                    &consumer,
                )?;
                let ty = if element.default_value {
                    self.type_with_facts(ty, super::facts::TypeFacts::NOT_UNDEFINED, &consumer)?
                } else {
                    ty
                };
                self.flow_type_of_destructuring(element.declaration, ty)?
            } else {
                element_type
            };
            return self
                .checked_destructuring_assignment(root, ty, mode, false)
                .map(Some);
        }
        if index + 1 != elements.len() {
            return Ok(None);
        }
        let operand = element
            .operand
            .ok_or_else(|| super::super::internal("destructuring spread has no operand"))?;
        if self.source.destructuring_target(operand)?.left.is_some() {
            return Ok(None);
        }
        let ty = if self.every_tuple_type(source)? {
            self.map_type(source, &consumer, |checker, ty| {
                checker.slice_tuple_type(ty, index, 0, &consumer)
            })?
        } else {
            self.array_type(element_type, &consumer)?
        };
        self.checked_destructuring_assignment(operand, ty, mode, false)
            .map(Some)
    }
    /// `padObjectLiteralType` / `padTupleType` apply only to parameter-root
    /// initializers and preserve the original identity if nothing is missing.
    fn pad_binding_initializer(
        &mut self,
        ty: TypeId,
        pattern: DeclarationIdentity,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        let kind = self.source.declaration_kind(pattern)?;
        if kind == K::ObjectBindingPattern
            && self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::OBJECT_LITERAL)
        {
            let elements = self.source.pattern_elements(pattern)?;
            let mut missing = Vec::new();
            for element in elements {
                if element.initializer.is_none() {
                    continue;
                }
                let name = element
                    .name
                    .ok_or_else(|| super::super::internal("binding element has no key"))?;
                let key = self.literal_type_of_property_name(name.file, name.site)?;
                if let Some(key) = self.property_key_from_type(key)?
                    && self.property_of_type_by_key(ty, &key, consumer)?.is_none()
                {
                    missing.push((key, element.declaration));
                }
            }
            if missing.is_empty() {
                return Ok(ty);
            }
            let mut properties = self.properties_of_object_type(ty, consumer)?;
            for (name, declaration) in missing {
                let value = self.type_from_binding_element(declaration, false, false)?;
                let symbol = self.create_property_symbol(name, true, false, Some(value));
                properties.push(self.project_property_symbol(symbol, consumer)?);
            }
            let properties = self.named_member_properties(properties, None, consumer)?;
            let indices = self.index_infos_of_type(ty, consumer)?;
            return Ok(self.model.structured_with_flags(
                self.model.type_symbol(ty),
                MembersView {
                    properties: properties.into(),
                    call_signatures: Vec::new().into(),
                    construct_signatures: Vec::new().into(),
                    index_signatures: indices.into(),
                },
                Metadata::default(),
                ObjectKind::Anonymous,
                self.model.object_flags(ty)?,
            ));
        }
        if kind == K::ArrayBindingPattern
            && let Some(tuple) = self.model.tuple_target(ty)?
        {
            if tuple.combined_flags.intersects(TupleElementFlags::VARIABLE) {
                return Ok(ty);
            }
            let elements = self.source.pattern_elements(pattern)?;
            let count = self.type_reference_arity(ty)?;
            if count >= elements.len() {
                return Ok(ty);
            }
            let mut types = self.get_element_types(ty)?.to_vec();
            let mut infos = tuple.elements.to_vec();
            for (index, element) in elements.iter().enumerate().skip(count) {
                if index + 1 == elements.len() && element.kind == K::BindingElement && element.rest
                {
                    continue;
                }
                let value = if element.kind != K::OmittedExpression && element.default_value {
                    self.type_from_binding_element(element.declaration, false, false)?
                } else {
                    self.model.special(SpecialName::Any)
                };
                types.push(value);
                infos.push(TupleElementInfo {
                    flags: TupleElementFlags::OPTIONAL,
                    label: None,
                });
                if element.kind != K::OmittedExpression && !element.default_value {
                    let any = self.model.special(SpecialName::Any);
                    self.check_diagnostic_type_display(any, consumer)?;
                }
            }
            return self.create_tuple_type_ex(types, infos, tuple.readonly, consumer);
        }
        Ok(ty)
    }

    /// `getTypeFromBindingPattern` uses an invocation stack for contextual
    /// recursion, restoring it even when a child interrupts checking.
    pub(super) fn type_from_binding_pattern_completion(
        &mut self,
        pattern: DeclarationIdentity,
        include_pattern: bool,
        report_errors: bool,
    ) -> NativeResult<TypeId> {
        if include_pattern {
            self.variables.contextual_patterns.push(pattern);
        }
        let result =
            self.binding_pattern_worker_completion(pattern, include_pattern, report_errors);
        if include_pattern {
            self.variables.contextual_patterns.pop();
        }
        result
    }

    fn binding_pattern_worker_completion(
        &mut self,
        pattern: DeclarationIdentity,
        include_pattern: bool,
        report_errors: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let consumer = self.source.declaration_location(pattern)?;
            let elements = self.source.pattern_elements(pattern)?;
            let result = if self.source.declaration_kind(pattern)? == K::ObjectBindingPattern {
                let mut properties: Vec<(
                    crate::project::binding::NativePropertyKey,
                    super::super::model::PropertySymbolId,
                )> = Vec::new();
                let mut index = None;
                let mut flags =
                    ObjectFlags::OBJECT_LITERAL | ObjectFlags::CONTAINS_OBJECT_OR_ARRAY_LITERAL;
                for element in elements {
                    if element.rest {
                        let string = self.model.primitive(PrimitiveName::String);
                        let any = self.model.special(SpecialName::Any);
                        index = Some(self.model.index_info(string, any, false, None, None));
                        continue;
                    }
                    let name = element.name.ok_or_else(|| {
                        super::super::internal("object binding element has no property name")
                    })?;
                    let ty = sequence.read(
                        self.literal_type_of_property_name_completion(name.file, name.site),
                    )?;
                    let Some(name) = self.property_key_from_type(ty)? else {
                        flags =
                            flags | ObjectFlags::OBJECT_LITERAL_PATTERN_WITH_COMPUTED_PROPERTIES;
                        continue;
                    };
                    // Native allocates before getTypeFromBindingElement. The
                    // fresh slot receives the actual resulting type only;
                    // child acquisition evidence belongs to this invocation.
                    let symbol = self.create_property_symbol(
                        name.clone(),
                        element.initializer.is_some(),
                        false,
                        None,
                    );
                    let value = sequence.read(self.type_from_binding_element_completion(
                        element.declaration,
                        include_pattern,
                        report_errors,
                    ))?;
                    self.set_property_value_state(
                        symbol,
                        false,
                        super::TypeState::Installed(NativeCompletion::clean(value)),
                    )?;
                    if let Some(entry) = properties.iter_mut().find(|entry| entry.0 == name) {
                        entry.1 = symbol;
                    } else {
                        properties.push((name, symbol));
                    }
                }
                self.new_resolved_anonymous_type(
                    TypeSymbol::Absent,
                    Some(super::structure::MemberTable::Symbols(properties.into())),
                    MembersView {
                        properties: Vec::new().into(),
                        call_signatures: Vec::new().into(),
                        construct_signatures: Vec::new().into(),
                        index_signatures: index.into_iter().collect::<Vec<_>>().into(),
                    },
                    flags,
                    &consumer,
                )?
            } else {
                let rest = elements
                    .last()
                    .is_some_and(|element| element.kind == K::BindingElement && element.rest);
                if elements.is_empty() || elements.len() == 1 && rest {
                    let any = self.model.special(SpecialName::Any);
                    return sequence.read(self.binding_pattern_iterable_completion(any, &consumer));
                }
                let minimum = elements
                    .iter()
                    .rposition(|element| {
                        !element.rest && element.name.is_some() && !element.default_value
                    })
                    .map_or(0, |index| index + 1);
                let mut types = Vec::with_capacity(elements.len());
                let mut infos = Vec::with_capacity(elements.len());
                for (index, element) in elements.iter().enumerate() {
                    types.push(if element.name.is_none() {
                        self.model.special(SpecialName::Any)
                    } else {
                        sequence.read(self.type_from_binding_element_completion(
                            element.declaration,
                            include_pattern,
                            report_errors,
                        ))?
                    });
                    infos.push(TupleElementInfo {
                        flags: if index + 1 == elements.len() && rest {
                            TupleElementFlags::REST
                        } else if index >= minimum {
                            TupleElementFlags::OPTIONAL
                        } else {
                            TupleElementFlags::REQUIRED
                        },
                        label: None,
                    });
                }
                let ty = sequence
                    .read(self.create_tuple_type_ex_completion(types, infos, false, &consumer))?;
                if include_pattern {
                    self.model.binding_pattern_reference(ty)?
                } else {
                    ty
                }
            };
            if include_pattern {
                self.record_pattern_type(result, pattern);
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `getTypeFromBindingElement` constructs the implied contextual type
    /// independently of its parent's inferred or annotated type.
    fn type_from_binding_element(
        &mut self,
        declaration: DeclarationIdentity,
        include_pattern: bool,
        report_errors: bool,
    ) -> Result<TypeId, DemandError> {
        self.type_from_binding_element_completion(declaration, include_pattern, report_errors)?
            .admit()
    }

    fn type_from_binding_element_completion(
        &mut self,
        declaration: DeclarationIdentity,
        include_pattern: bool,
        report_errors: bool,
    ) -> NativeResult<TypeId> {
        let facts = self.source.variable_facts(declaration)?;
        if facts.initializer.is_some() {
            let context = if let Some(pattern) = facts.pattern {
                self.type_from_binding_pattern_completion(pattern, true, false)?
            } else {
                NativeCompletion::clean(self.model.special(SpecialName::Unknown))
            };
            return context.and_then(|context| {
                self.declaration_initializer_type_completion(
                    &facts,
                    CheckMode::NORMAL,
                    Some(context),
                )?
                .and_then(|ty| {
                    self.widened_literal_for_initializer_completion(&facts, ty)?
                        .and_then(|ty| {
                            self.add_optionality_completion(ty, false, true, &facts.location)
                        })
                })
            });
        }
        if let Some(pattern) = facts.pattern {
            return self.type_from_binding_pattern_completion(
                pattern,
                include_pattern,
                report_errors,
            );
        }
        let report = if report_errors && !self.source.declaration_is_private_ambient(declaration)? {
            let any = self.model.special(SpecialName::Any);
            self.report_implicit_any_completion(declaration, any, &facts.location)?
        } else {
            NativeCompletion::clean(())
        };
        Ok(report.map(|()| {
            if include_pattern {
                self.model.non_inferrable_any_type()
            } else {
                self.model.special(SpecialName::Any)
            }
        }))
    }

    /// `assignBindingElementTypes` performs depth-first native assignment.
    /// Each actual identifier symbol is installed before the next sibling;
    /// nested patterns have no invented symbol or independent value cache.
    pub(super) fn assign_binding_element_types_completion(
        &mut self,
        pattern: DeclarationIdentity,
        parent: TypeId,
    ) -> NativeResult<()> {
        enum Work {
            Pattern(DeclarationIdentity, TypeId),
            Element(DeclarationIdentity, Option<DeclarationIdentity>, TypeId),
        }
        let mut work = vec![Work::Pattern(pattern, parent)];
        let mut result = NativeCompletion::clean(());
        while let Some(next) = work.pop() {
            result = result.and_then(|()| match next {
                Work::Pattern(pattern, parent) => {
                    for (element, nested) in self
                        .source
                        .binding_pattern_assignments(pattern)?
                        .into_iter()
                        .rev()
                    {
                        work.push(Work::Element(element, nested, parent));
                    }
                    Ok(NativeCompletion::clean(()))
                }
                Work::Element(element, nested, parent) => self
                    .binding_element_from_parent_completion(element, parent, false)?
                    .and_then_value(|ty| {
                        if let Some(nested) = nested {
                            work.push(Work::Pattern(nested, ty));
                        } else {
                            let symbol = self.source.raw_symbol_of_declaration(element)?;
                            let symbol = self.register_property_symbol(symbol)?;
                            self.set_property_value_state(
                                symbol,
                                false,
                                super::TypeState::Installed(NativeCompletion::clean(ty)),
                            )?;
                        }
                        Ok(())
                    }),
            })?;
        }
        Ok(result)
    }

    /// `createIterableType` consumes the checked global and supplies the
    /// actual empty-object recovery when the generic target is unavailable.
    fn binding_pattern_iterable_completion(
        &mut self,
        element: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if !self.inputs.semantic_options().target_supports_iterables() {
            return self.array_type_completion(element, consumer);
        }
        self.global_iterable_type_checked_completion(false, consumer)?
            .and_then_value(|lookup| match lookup {
                super::globals::GlobalTypeLookup::Target { ty: target, .. } => {
                    let void = self.model.special(SpecialName::Void);
                    let undefined = self.model.special(SpecialName::Undefined);
                    self.model.reference(target, vec![element, void, undefined])
                }
                super::globals::GlobalTypeLookup::Recovery(_) => Ok(self.model.empty_object_type()),
            })
    }

    /// `getTypeForBindingElement` asks the optional parent before the
    /// complete `getBindingElementTypeFromParentType` operation.
    fn binding_element_type_completion(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> NativeResult<Option<TypeId>> {
        let facts = self.source.variable_facts(declaration)?;
        let binding = self.source.binding_element_facts(declaration)?;
        let mode = if facts.rest {
            CheckMode::REST_BINDING_ELEMENT
        } else {
            CheckMode::NORMAL
        };
        self.binding_parent_type_completion(binding.parent, mode)?
            .and_then(|parent| match parent {
                Some(parent) => self
                    .binding_element_from_parent_completion(declaration, parent, false)
                    .map(|value| value.map(Some)),
                None => Ok(NativeCompletion::clean(None)),
            })
    }

    /// `getBindingElementTypeFromParentType`: the ordinary inference and
    /// correlated-narrowing callers differ only in native tuple bounds policy.
    pub(super) fn binding_element_from_parent_completion(
        &mut self,
        declaration: DeclarationIdentity,
        parent: TypeId,
        no_tuple_bounds_check: bool,
    ) -> NativeResult<TypeId> {
        let facts = self.source.variable_facts(declaration)?;
        let binding = self.source.binding_element_facts(declaration)?;
        if self.model.has_type_flags(parent, TypeFlags::ANY) {
            return Ok(NativeCompletion::clean(parent));
        }
        let strict = self.inputs.semantic_options().strict_null_checks;
        let parent = if strict && binding.ambient_parameter {
            self.non_nullable_type_completion(parent, &facts.location)?
        } else if strict {
            let parent_facts = self.source.variable_facts(binding.parent)?;
            if let Some(initializer) = parent_facts.initializer {
                let initial = match initializer.root {
                    Some(root) => self.type_of_initializer_completion(root.file, root.site)?,
                    None => self.check_expression_completion(&initializer, CheckMode::NORMAL)?,
                };
                initial.and_then(|ty| {
                    self.type_facts_completion(
                        ty,
                        super::facts::TypeFacts::EQUAL_UNDEFINED,
                        &facts.location,
                    )?
                    .and_then(|facts_| {
                        if facts_ == super::facts::TypeFacts::NONE {
                            self.type_with_facts_completion(
                                parent,
                                super::facts::TypeFacts::NOT_UNDEFINED,
                                &facts.location,
                            )
                        } else {
                            Ok(NativeCompletion::clean(parent))
                        }
                    })
                })?
            } else {
                NativeCompletion::clean(parent)
            }
        } else {
            NativeCompletion::clean(parent)
        };
        parent.and_then(|parent| {
        let flags = AccessFlags::EXPRESSION_POSITION
            | if no_tuple_bounds_check || facts.initializer.is_some() {
                AccessFlags::ALLOW_MISSING
            } else {
                AccessFlags::NONE
            };
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
        let ty = if binding.object {
            if facts.rest {
                let parent = sequence.read(self.reduced_type_completion(parent, &facts.location))?;
                if self.model.has_type_flags(parent, TypeFlags::UNKNOWN)
                    || !sequence.read(self.is_valid_spread_type_completion(parent, &facts.location))?
                {
                    return sequence.read(NativeCompletion::recovered(self.model.native_error_type(),
                        super::super::invalid_at_location(
                            self.inputs, &facts.location,
                            crate::analysis::SemanticFailureReason::ErrorType,
                            "native object rest from a non-object source consumes errorType",
                        ).into(),
                    )).map(|ty| (ty, true));
                }
                let names = self
                    .source
                    .pattern_elements(binding.pattern)?
                    .into_iter()
                    .filter(|element| !element.rest)
                    .map(|element| {
                        element.name.ok_or_else(|| {
                            super::super::internal("binding element lacks its actual name")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let symbol = match self.reference_raw_symbol_of_declaration(declaration)? {
                    Some(symbol) => self.type_symbol_of_reference(symbol, &facts.location)?,
                    None => TypeSymbol::Absent,
                };
                sequence.read(self.rest_type_completion(parent, &names, symbol, &facts.location))?
            } else {
                let index =
                    sequence.read(self.literal_type_of_property_name_completion(binding.name.file, binding.name.site))?;
                let ty = sequence.read(self.indexed_access_at_node_completion(
                    parent,
                    index,
                    flags,
                    binding.name.into(),
                    &facts.location,
                ))?;
                self.flow_type_of_destructuring(declaration, ty)?
            }
        } else {
            let error = self.source.declaration_expression_root(binding.pattern)?.into();
            let iteration_location = self.source.declaration_location(binding.pattern)?;
            let element = sequence.read(self.binding_iteration_type_completion(parent, !facts.rest, error, &iteration_location))?;
            if facts.rest {
                let base = sequence.read(self.map_type_completion(parent, &facts.location, |checker, ty| {
                    if checker
                        .model
                        .has_type_flags(ty, TypeFlags::INSTANTIABLE_NON_PRIMITIVE)
                    {
                        Ok(checker.base_constraint_completion(ty, &facts.location)?.map(|base| base.unwrap_or(ty)))
                    } else {
                        Ok(NativeCompletion::clean(ty))
                    }
                }))?;
                if self.every_tuple_type(base)? {
                    sequence.read(self.map_type_completion(base, &facts.location, |checker, ty| {
                        checker.slice_tuple_type_completion(ty, binding.index, 0, &facts.location)
                    }))?
                } else {
                    sequence.read(self.array_type_completion(element, &facts.location))?
                }
            } else if sequence.read(self.is_array_like_type_completion(parent, &facts.location))? {
                let key = self
                    .model
                    .regular_literal_type(PrimitiveName::Number, binding.index.to_string());
                let ty = sequence.read(self.indexed_access_at_node_completion(
                    parent,
                    key,
                    flags,
                    binding.name.into(),
                    &facts.location,
                ))?;
                self.flow_type_of_destructuring(declaration, ty)?
            } else {
                element
            }
        };
        Ok((ty, false))
        })();
        sequence.finish(result)?.and_then(|(ty, recovered_rest)| {
        if recovered_rest { return Ok(NativeCompletion::clean(ty)); }
        if facts.initializer.is_none() { return Ok(NativeCompletion::clean(ty)); }
        if binding.root_annotated {
            if !strict { return Ok(NativeCompletion::clean(ty)); }
            return self.declaration_initializer_type_completion(&facts, CheckMode::NORMAL, None)?.and_then(|initial| {
                self.type_facts_completion(initial, super::facts::TypeFacts::ONLY_UNDEFINED, &facts.location)?.and_then(|facts_| {
                    if facts_ == super::facts::TypeFacts::NONE { self.non_undefined_assignment_type_completion(ty, &facts.location) }
                    else { Ok(NativeCompletion::clean(ty)) }
                })
            });
        }
        self.non_undefined_assignment_type_completion(ty, &facts.location)?.and_then(|without| {
            self.declaration_initializer_type_completion(&facts, CheckMode::NORMAL, None)?.and_then(|initial| {
                self.union_type_reduced_completion(vec![without, initial], UnionReduction::Subtype, None, None, &facts.location)
                    ?.and_then(|ty| self.widened_literal_for_initializer_completion(&facts, ty))
            })
        })
        })
        })
    }

    pub(super) fn binding_parent_type_completion(
        &mut self,
        declaration: DeclarationIdentity,
        mode: CheckMode,
    ) -> NativeResult<Option<TypeId>> {
        let facts = self.source.variable_facts(declaration)?;
        // bindVariableDeclarationOrBindingElement has no symbol for an outer
        // binding pattern. Pattern parameters have a distinct anonymous one.
        if mode == CheckMode::NORMAL
            && !(facts.pattern.is_some()
                && matches!(facts.kind, K::VariableDeclaration | K::BindingElement))
        {
            let symbol = self.source.raw_symbol_of_declaration(declaration)?;
            match self.value_types.get(&symbol) {
                Some(super::TypeState::Failed(error)) => {
                    return Err(error.clone());
                }
                Some(super::TypeState::Installed(value))
                    if !(self.inputs.semantic_options().strict_null_checks && facts.optional) =>
                {
                    return Ok(value.clone().map(Some));
                }
                _ => {}
            }
        }
        self.variable_like_declaration_type_completion(declaration, false, mode)
    }

    fn every_tuple_type(&self, ty: TypeId) -> Result<bool, DemandError> {
        let types = self
            .model
            .union_constituents(ty)
            .unwrap_or_else(|| vec![ty].into());
        Ok(types.iter().all(|&ty| self.model.is_tuple_type(ty)))
    }

    /// `getTypeForVariableLikeDeclaration`, including native nil. This raw
    /// result must not be widened by GetTypeOfNode or binding-parent callers.
    pub(super) fn variable_like_declaration_type_completion(
        &mut self,
        declaration: DeclarationIdentity,
        include_optionality: bool,
        mode: CheckMode,
    ) -> NativeResult<Option<TypeId>> {
        let facts = self.source.variable_facts(declaration)?;
        match &facts.early {
            VariableEarly::ForIn(expression) => {
                return self
                    .check_expression_completion(expression, mode)?
                    .and_then(|ty| {
                        self.non_nullable_type_if_needed_completion(ty, &facts.location)
                    })?
                    .and_then(|ty| {
                        self.index_type_ex_completion(ty, IndexFlags::NONE, &facts.location)
                    })?
                    .and_then(|index| {
                        if self
                            .model
                            .has_type_flags(index, TypeFlags::TYPE_PARAMETER | TypeFlags::INDEX)
                        {
                            return self
                                .extract_string_type_completion(index, &facts.location)
                                .map(|value| value.map(Some));
                        }
                        Ok(NativeCompletion::clean(Some(
                            self.model.primitive(PrimitiveName::String),
                        )))
                    });
            }
            VariableEarly::ForOf {
                expression,
                asynchronous,
            } => {
                return self
                    .assignment_for_of_type_completion(
                        expression.file,
                        expression.site,
                        *asynchronous,
                    )
                    .map(|value| value.map(Some));
            }
            VariableEarly::BindingElement => {
                return self.binding_element_type_completion(declaration);
            }
            VariableEarly::Ordinary => {}
        }
        let optional = include_optionality && facts.optional;
        let annotated = match &facts.annotation {
            Some(annotation) => self.check_type_completion(annotation)?.map(Some),
            None => NativeCompletion::clean(None),
        };
        annotated.and_then(|annotated| {
            if facts.catch {
                return match annotated {
                    Some(ty)
                        if self
                            .model
                            .has_type_flags(ty, TypeFlags::ANY | TypeFlags::UNKNOWN) =>
                    {
                        Ok(NativeCompletion::clean(Some(ty)))
                    }
                    Some(_) => NativeCompletion::recovered(
                        Some(self.model.native_error_type()),
                        self.variable_recovery(
                            &facts.location,
                            "native catch annotation consumes errorType unless Any or Unknown",
                        ),
                    ),
                    None => Ok(NativeCompletion::clean(Some(
                        self.model.special(
                            if self
                                .inputs
                                .semantic_options()
                                .use_unknown_in_catch_variables
                            {
                                SpecialName::Unknown
                            } else {
                                SpecialName::Any
                            },
                        ),
                    ))),
                };
            }
            if let Some(ty) = annotated {
                return self
                    .add_optionality_completion(ty, facts.property, optional, &facts.location)
                    .map(|value| value.map(Some));
            }
            let automatic = if self.inputs.semantic_options().no_implicit_any
                && facts.automatic_eligible
                && !facts.constant
            {
                self.variable_initializer_is_null_or_undefined_completion(
                    declaration,
                    &facts.location,
                )?
            } else {
                NativeCompletion::clean(false)
            };
            automatic.and_then(|automatic| {
                if automatic {
                    return Ok(NativeCompletion::clean(Some(
                        self.model.automatic_variable(),
                    )));
                }
                if self.inputs.semantic_options().no_implicit_any
                    && facts.automatic_eligible
                    && facts.empty_array
                {
                    return self
                        .automatic_array_type(&facts.location)
                        .map(|ty| NativeCompletion::clean(Some(ty)));
                }
                let contextual = if facts.kind == K::Parameter {
                    let parameter = self.source.variable_parameter_recipe(declaration)?;
                    if parameter.function_kind == K::SetAccessor {
                        let bound = self
                            .source
                            .raw_symbol_of_declaration(parameter.declaration)?;
                        let symbol = self.register_property_symbol(bound)?;
                        let name = self.property_lookup_key(symbol, &parameter.location)?;
                        let value = self.setter_parameter_from_getter_completion(&facts, &name)?;
                        if value.value().is_some() {
                            return Ok(value);
                        }
                        // Native nil is not an erroneous TypeId. This owner only
                        // returns nil without acquiring a recovered type.
                    }
                    self.contextually_typed_parameter_completion(&parameter)?
                } else {
                    NativeCompletion::clean(None)
                };
                contextual.and_then(|contextual| {
                    if let Some(ty) = contextual {
                        return self
                            .add_optionality_completion(ty, false, optional, &facts.location)
                            .map(|value| value.map(Some));
                    }
                    if facts.initializer.is_some() {
                        return self
                            .declaration_initializer_type_completion(&facts, mode, None)?
                            .and_then(|ty| {
                                self.widened_literal_for_initializer_completion(&facts, ty)?
                                    .and_then(|ty| {
                                        self.add_optionality_completion(
                                            ty,
                                            facts.property,
                                            optional,
                                            &facts.location,
                                        )
                                        .map(|value| value.map(Some))
                                    })
                            });
                    }
                    if self.inputs.semantic_options().no_implicit_any
                        && facts.kind == K::PropertyDeclaration
                    {
                        match self.source.property_inference(declaration)? {
                            PropertyInference::None => {}
                            PropertyInference::AutomaticFlow => {
                                return Err(self.flow_required(
                                    &facts.location,
                                    "automatic constructor/static-block property inference requires excluded flow analysis",
                                ));
                            }
                            PropertyInference::BaseClass => {
                                // Native passes declaration.Symbol directly;
                                // this is not a late/merged declaration lookup.
                                let symbol = self.reference_raw_symbol_of_declaration(declaration)?
                                    .ok_or_else(|| super::super::internal("property inference lacks its raw symbol"))?;
                                let symbol = self.register_reference_symbol(symbol, &facts.location)?;
                                return self.type_of_property_in_base_class_completion(symbol, &facts.location)?
                                    .and_then(|ty| match ty {
                                        Some(ty) => self.add_optionality_completion(ty, true, optional, &facts.location)
                                            .map(|value| value.map(Some)),
                                        None => Ok(NativeCompletion::clean(None)),
                                    });
                            }
                        }
                    }
                    if facts.kind == K::JsxAttribute {
                        return Ok(NativeCompletion::clean(Some(
                            self.model
                                .regular_literal_type(PrimitiveName::Boolean, "true"),
                        )));
                    }
                    if let Some(pattern) = facts.pattern {
                        return self
                            .type_from_binding_pattern_completion(pattern, false, true)
                            .map(|value| value.map(Some));
                    }
                    Ok(NativeCompletion::clean(None))
                })
            })
        })
    }

    /// `isNullOrUndefined`: only the actual undefinedSymbol satisfies an
    /// identifier initializer, after parentheses have been stripped.
    fn variable_initializer_is_null_or_undefined_completion(
        &mut self,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<bool> {
        use super::super::source::variables::NullishInitializer;
        match self.source.nullish_initializer(declaration)? {
            NullishInitializer::AbsentOrNull => Ok(NativeCompletion::clean(true)),
            NullishInitializer::Other => Ok(NativeCompletion::clean(false)),
            NullishInitializer::Identifier(root) => self
                .reference_resolved_identifier_symbol_completion(root, consumer)?
                .and_then_value(|symbol| {
                    let Some(undefined) = self.initialization.undefined_symbol() else {
                        return Ok(false);
                    };
                    Ok(self.reference_symbol_identity_at(symbol, None)?
                        == self.reference_symbol_identity_at(undefined, None)?)
                }),
        }
    }

    /// `getExtractStringType` consumes the actual global alias when present;
    /// an absent/wrong-arity alias has the native String result.
    fn extract_string_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let string = self.model.primitive(PrimitiveName::String);
            let Some(symbol) =
                sequence.read(self.global_type_alias_completion("Extract", 2, consumer))?
            else {
                return Ok(string);
            };
            let declared =
                sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
            sequence.read(self.type_alias_instantiation_completion(
                symbol,
                declared,
                vec![ty, string],
                None,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// The setter-specific branch of `getTypeForVariableLikeDeclaration`
    /// acquires the getter signature before distinguishing this/value params.
    fn setter_parameter_from_getter_completion(
        &mut self,
        facts: &VariableFacts,
        name: &crate::project::binding::NativePropertyKey,
    ) -> NativeResult<Option<TypeId>> {
        let parent = facts
            .parent
            .ok_or_else(|| super::super::internal("setter parameter has no accessor"))?;
        if !self.has_bindable_declaration_name(parent)? {
            return Ok(NativeCompletion::clean(None));
        }
        let symbol = self.reference_symbol_of_declaration(parent)?;
        for declaration in self.reference_symbol_declarations(symbol, &facts.location)? {
            let ReferenceDeclaration::Captured(declaration) = declaration else {
                continue;
            };
            if self.source.declaration_kind(declaration)? != K::GetAccessor {
                continue;
            }
            let recipe = self.source.signature_from_declaration(declaration)?;
            let signature = self.signature(&recipe)?;
            if name.fixed_bytes() == Some(b"this".as_slice()) {
                let parameter = signature.this_parameter().as_ref().ok_or_else(|| {
                    self.variable_recovery(
                        &facts.location,
                        "native getter this parameter is absent",
                    )
                })?;
                return self
                    .demand_completion(parameter.value_type)
                    .map(|value| value.map(Some));
            }
            return self
                .get_return_type_of_signature_completion(&signature, &facts.location)
                .map(|value| value.map(Some));
        }
        Ok(NativeCompletion::clean(None))
    }

    /// `checkDeclarationInitializer`: quick type first, then contextual/cached
    /// checking, followed by parameter binding-pattern padding.
    pub(super) fn declaration_initializer_type_completion(
        &mut self,
        facts: &VariableFacts,
        mode: CheckMode,
        contextual: Option<TypeId>,
    ) -> NativeResult<TypeId> {
        let expression = facts
            .initializer
            .as_ref()
            .ok_or_else(|| super::super::internal("initializer question has no initializer"))?;
        let value = self
            .quick_type_of_expression_completion(expression)?
            .and_then(|quick| {
                if let Some(ty) = quick {
                    return Ok(NativeCompletion::clean(ty));
                }
                if let Some(contextual) = contextual {
                    self.check_expression_with_contextual_type_completion(
                        expression, contextual, None, mode,
                    )
                } else if expression.root.is_some() {
                    self.check_expression_cached_ex_completion(expression, mode)
                } else {
                    self.check_expression_completion(expression, mode)
                }
            })?;
        value.and_then_value(|ty| {
            if facts.root_parameter
                && let Some(pattern) = facts.pattern
            {
                return self.pad_binding_initializer(ty, pattern, &facts.location);
            }
            Ok(ty)
        })
    }

    /// `getWidenedLiteralTypeForInitializer`: const/readonly preserve literal
    /// freshness. Whole structural widening belongs to the symbol wrapper.
    fn widened_literal_for_initializer_completion(
        &mut self,
        facts: &VariableFacts,
        ty: TypeId,
    ) -> NativeResult<TypeId> {
        if facts.constant || facts.readonly {
            Ok(NativeCompletion::clean(ty))
        } else {
            self.widened_literal_type_completion(ty, &facts.location)
        }
    }

    /// `widenTypeForVariableLikeDeclaration` is shared by every declaration
    /// symbol read, separately from the raw optional result above.
    pub(super) fn widened_variable_like_type_completion(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> NativeResult<TypeId> {
        self.widened_variable_like_type_ex_completion(declaration, true)
    }

    /// `getWidenedTypeForVariableLikeDeclaration` keeps reportErrors separate
    /// from includeOptionality, which is always true at this native entry.
    pub(super) fn widened_variable_like_type_ex_completion(
        &mut self,
        declaration: DeclarationIdentity,
        report_errors: bool,
    ) -> NativeResult<TypeId> {
        self.variable_like_declaration_type_completion(declaration, true, CheckMode::NORMAL)?
            .and_then(|raw| {
                self.widen_type_for_variable_like_declaration_completion(
                    raw,
                    declaration,
                    report_errors,
                )
            })
    }

    /// `widenTypeForVariableLikeDeclaration` also accepts already acquired
    /// initializer types; using-declaration checks must not read them again.
    pub(super) fn widen_type_for_variable_like_declaration_completion(
        &mut self,
        raw: Option<TypeId>,
        declaration: DeclarationIdentity,
        report_errors: bool,
    ) -> NativeResult<TypeId> {
        let facts = self.source.variable_facts(declaration)?;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(mut ty) = raw {
                if self.model.has_type_flags(ty, TypeFlags::SYMBOL) {
                    let parent = facts
                        .parent
                        .map(|parent| self.reference_node_declaration_symbol(parent))
                        .transpose()?
                        .flatten();
                    let global = sequence.read(
                        self.global_type_symbol_completion("SymbolConstructor", &facts.location),
                    )?;
                    if let (Some(parent), Some(global)) = (parent, global) {
                        let parent = self.reference_symbol_identity(parent, &facts.location)?;
                        let global = self.reference_symbol_identity(global, &facts.location)?;
                        if parent == global {
                            ty = self.es_symbol_like_type_for_node(declaration, &facts.location)?;
                        }
                    }
                }
                if report_errors {
                    sequence.read(self.report_variable_widening_completion(
                        declaration,
                        ty,
                        &facts.location,
                    ))?;
                }
                if self.model.has_type_flags(ty, TypeFlags::UNIQUE_SYMBOL)
                    && (facts.kind == K::BindingElement || facts.annotation.is_none())
                {
                    let symbol = self.reference_symbol_of_declaration(declaration)?;
                    let symbol = self.reference_symbol_identity(symbol, &facts.location)?;
                    if self.type_symbol_identity(ty)? != Some(symbol) {
                        ty = self.model.primitive(PrimitiveName::Symbol);
                    }
                }
                sequence.read(self.widened_type_completion(ty, &facts.location))
            } else {
                let any = self.model.special(SpecialName::Any);
                let ty = if facts.kind == K::Parameter && facts.rest {
                    self.installed_any_array_type()?
                } else {
                    any
                };
                if report_errors && !self.source.declaration_is_private_ambient(declaration)? {
                    sequence.read(self.report_implicit_any_completion(
                        declaration,
                        ty,
                        &facts.location,
                    ))?;
                }
                Ok(ty)
            }
        })();
        sequence.finish(result)
    }

    /// `getTypeOfAssignmentPattern` acquires the parent source and then asks
    /// checked destructuring, not the unrelated initial/assigned flow recipe.
    pub(super) fn type_of_assignment_pattern(
        &mut self,
        pattern: ExpressionRoot,
    ) -> Result<TypeId, DemandError> {
        let consumer = super::super::location(self.inputs, pattern.file, pattern.site.span);
        match self.source.assignment_pattern_recipe(pattern)? {
            AssignmentPatternRecipe::ForOf {
                expression,
                asynchronous,
            } => {
                let source =
                    self.assignment_for_of_type(expression.file, expression.site, asynchronous)?;
                self.checked_destructuring_assignment(pattern, source, CheckMode::NORMAL, false)
            }
            AssignmentPatternRecipe::Binary(right) => {
                let source = self.type_of_expression(right.file, right.site)?;
                self.checked_destructuring_assignment(pattern, source, CheckMode::NORMAL, false)
            }
            AssignmentPatternRecipe::Object {
                pattern: parent,
                index,
            } => {
                let source = self.type_of_assignment_pattern(parent)?;
                self.checked_destructuring_property(parent, source, index, false)?
                    .ok_or_else(|| {
                        self.variable_recovery(
                            &consumer,
                            "native assignment-pattern property has no type",
                        )
                    })
            }
            AssignmentPatternRecipe::Array {
                pattern: parent,
                index,
            } => {
                let source = self.type_of_assignment_pattern(parent)?;
                let element = self
                    .binding_iteration_type_completion(source, false, parent.into(), &consumer)?
                    .admit()?;
                self.checked_destructuring_element(
                    parent,
                    source,
                    index,
                    element,
                    CheckMode::NORMAL,
                )?
                .ok_or_else(|| {
                    self.variable_recovery(
                        &consumer,
                        "native assignment-pattern element has no type",
                    )
                })
            }
        }
    }

    fn variable_dependency(&self, consumer: &Location, message: &str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            consumer,
            crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }
    fn variable_recovery(&self, consumer: &Location, message: &str) -> DemandError {
        super::super::invalid_at_location(
            self.inputs,
            consumer,
            crate::analysis::SemanticFailureReason::UnresolvedSymbol,
            message,
        )
        .into()
    }
}
