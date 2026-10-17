//! Native widening owns sibling contexts and semantic property acquisition.
//! Model factories receive complete facts; they never recurse into references.

use super::super::model::{
    DemandError, MembersView, Metadata, NativeCompletion, NativeResult, NativeSequence,
    ObjectFlags, ObjectKind, Property, PropertyDisplayDemand, SpecialName, TypeDemand, TypeFlags,
    TypeId, UnionReduction,
};
use super::{Evaluator, name_map::NameMap};
use crate::project::binding::NativePropertyKey;
use crate::references::Location;
use crate::resources::Control;
use crate::types::TypePropertyKind;
use std::collections::HashMap;

pub(super) struct WideningState {
    roots: HashMap<TypeId, NativeCompletion<TypeId>>,
    undefined_properties: NameMap<Property>,
}

impl WideningState {
    pub(super) fn new(control: Option<Control>) -> Self {
        Self {
            roots: HashMap::new(),
            undefined_properties: NameMap::new(control),
        }
    }
}

struct Context {
    parent: Option<usize>,
    property: Option<NativePropertyKey>,
    siblings: Option<NativeCompletion<Vec<TypeId>>>,
    properties: Option<NativeCompletion<Vec<Property>>>,
    children: NameMap<usize>,
    widened: HashMap<TypeId, NativeCompletion<TypeId>>,
}

impl Context {
    fn new(control: Option<Control>) -> Self {
        Self {
            parent: None,
            property: None,
            siblings: None,
            properties: None,
            children: NameMap::new(control),
            widened: HashMap::new(),
        }
    }
}

impl Evaluator<'_, '_> {
    /// `reportImplicitAny`: TypeScript inputs first serialize the widened
    /// type, then perform the parameter-name lookup when syntax requires it.
    /// The remaining branches only select diagnostic wording. Reporting a
    /// diagnostic does not itself create a semantic recovery type.
    pub(super) fn report_implicit_any_completion(
        &mut self,
        declaration: super::super::source::DeclarationIdentity,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let widened = sequence.read(self.widened_type_completion(ty, consumer))?;
            sequence.read(self.check_diagnostic_type_display_completion(widened, consumer))?;
            if let Some(span) = self
                .source
                .implicit_any_parameter_type_lookup(declaration)?
            {
                sequence.read(self.reference_parameter_type_name_completion(
                    declaration.file,
                    span,
                    consumer,
                ))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `reportErrorsFromWidening(..., Normal)` has no contextual-signature
    /// gate. It shares the complete child-report and implicit-any operations
    /// with the function-return branch below.
    pub(super) fn report_variable_widening_completion(
        &mut self,
        declaration: super::super::source::DeclarationIdentity,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        if !self.inputs.semantic_options().no_implicit_any
            || !self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::CONTAINS_WIDENING_TYPE)
        {
            return Ok(NativeCompletion::clean(()));
        }
        self.report_widening_errors_in_type_completion(ty, consumer)?
            .and_then(|reported| {
                if reported {
                    Ok(NativeCompletion::clean(()))
                } else {
                    self.report_implicit_any_completion(declaration, ty, consumer)
                }
            })
    }

    /// `reportErrorsFromWidening(..., FunctionReturn)` and
    /// `shouldReportErrorsFromWideningWithContextualSignature`: these
    /// diagnostic-only demands precede contextual literal and whole widening.
    pub(super) fn report_return_widening_completion(
        &mut self,
        body: &super::super::source::ReturnBodyRecipe,
        ty: TypeId,
    ) -> NativeResult<()> {
        if !self.inputs.semantic_options().no_implicit_any
            || !self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::CONTAINS_WIDENING_TYPE)
        {
            return Ok(NativeCompletion::clean(()));
        }
        let consumer = &body.location;
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(signature) = sequence
                .read(self.contextual_signature_for_function_completion(body.root, consumer))?
            {
                let mut returned = sequence
                    .read(self.get_return_type_of_signature_completion(&signature, consumer))?;
                if body.flags.is_generator() {
                    return Err(self.widening_dependency(
                        consumer,
                        "native generator contextual widening report is pending",
                    ));
                }
                if body.flags.is_async() {
                    returned = sequence
                        .read(self.awaited_type_no_alias_completion(returned, consumer))?
                        .unwrap_or(returned);
                }
                if !sequence.read(self.is_generic_type_completion(returned, consumer))? {
                    return Ok(());
                }
            }
            if !sequence.read(self.report_widening_errors_in_type_completion(ty, consumer))? {
                let declaration = self.source.declaration_from_expression_root(
                    super::super::source::ExpressionRoot {
                        file: body.root.file,
                        site: body.root.site(),
                    },
                )?;
                sequence.read(self.report_implicit_any_completion(declaration, ty, consumer))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `reportWideningErrorsInType`: union/array child reports short-circuit
    /// after true, but each object property has its own assignment to the
    /// reported flag. Actual declaration identity locates normalized members.
    fn report_widening_errors_in_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if !self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::CONTAINS_WIDENING_TYPE)
            {
                return Ok(false);
            }
            let mut reported = false;
            if let Some(types) = self.model.union_constituents(ty) {
                for &child in types.iter() {
                    if sequence.read(self.is_empty_object_type_completion(child, consumer))? {
                        return Ok(true);
                    }
                }
                for &child in types.iter() {
                    if !reported {
                        reported = sequence.read(
                            self.report_widening_errors_in_type_completion(child, consumer),
                        )?;
                    }
                }
            } else if sequence.read(self.is_array_or_tuple_type_completion(ty))? {
                for &child in sequence
                    .read(self.get_type_arguments_completion(ty))?
                    .iter()
                {
                    if !reported {
                        reported = sequence.read(
                            self.report_widening_errors_in_type_completion(child, consumer),
                        )?;
                    }
                }
            } else if self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::OBJECT_LITERAL)
            {
                for property in
                    sequence.read(self.properties_of_object_type_completion(ty, consumer))?
                {
                    let child = sequence.read(self.widening_property_type(&property, consumer))?;
                    if self
                        .model
                        .object_flags(child)?
                        .intersects(ObjectFlags::CONTAINS_WIDENING_TYPE)
                    {
                        reported = sequence.read(
                            self.report_widening_errors_in_type_completion(child, consumer),
                        )?;
                        if !reported {
                            let symbol = property.symbol.ok_or_else(|| {
                                self.widening_dependency(consumer,
                                "native widening diagnostic requires its actual property symbol")
                            })?;
                            let mut found = false;
                            for declaration in
                                self.property_reference_declarations(symbol, consumer)?
                            {
                                let super::ReferenceDeclaration::Captured(declaration) =
                                    declaration
                                else {
                                    return Err(self.widening_dependency(consumer, "native widening diagnostic requires a captured declaration"));
                                };
                                let raw = self
                                    .reference_raw_symbol_of_declaration(declaration)?
                                    .ok_or_else(|| {
                                        super::super::internal(
                                            "widening property declaration has no raw symbol",
                                        )
                                    })?;
                                if let Some(super::ReferenceDeclaration::Captured(value)) =
                                    self.reference_symbol_value_declaration(raw, consumer)?
                                {
                                    let parent = self.source.declaration_parent(value)?;
                                    if parent.map(super::ReferenceDeclaration::Captured)
                                        == self.type_symbol_value_declaration(ty, consumer)?
                                    {
                                        found = true;
                                        break;
                                    }
                                }
                            }
                            if found {
                                sequence.read(self.diagnostic_symbol_display_completion(
                                    super::ReferenceSymbol::from_property(symbol),
                                    consumer,
                                ))?;
                                let widened =
                                    sequence.read(self.widened_type_completion(child, consumer))?;
                                sequence.read(
                                    self.check_diagnostic_type_display_completion(
                                        widened, consumer,
                                    ),
                                )?;
                                reported = true;
                            }
                        }
                    }
                }
            }
            Ok(reported)
        })();
        sequence.finish(result)
    }

    /// `getWidenedType` / `getWidenedTypeWithContext`: only root questions use
    /// the checker cache; union sibling contexts and their child caches live
    /// for this operation. Nullable union children deliberately remain intact.
    pub(super) fn widened_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.widened_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn widened_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.widened_type_with_context(ty, None, &mut Vec::new(), consumer)
    }

    fn widened_type_with_context(
        &mut self,
        ty: TypeId,
        context: Option<usize>,
        contexts: &mut Vec<Context>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if !self
            .model
            .object_flags(ty)?
            .intersects(ObjectFlags::REQUIRES_WIDENING)
        {
            return Ok(NativeCompletion::clean(ty));
        }
        if context.is_none()
            && let Some(cached) = self.widening.roots.get(&ty)
        {
            return Ok(cached.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let flags = self.model.type_flags(ty);
            let result = if flags.intersects(TypeFlags::ANY | TypeFlags::NULLABLE) {
                Some(self.model.special(SpecialName::Any))
            } else if self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::OBJECT_LITERAL)
            {
                Some(sequence.read(self.widened_object_literal(ty, context, contexts, consumer))?)
            } else if let Some(types) = self.model.union_constituents(ty) {
                let union_context = match context {
                    Some(context) => context,
                    None => {
                        let id = contexts.len();
                        contexts.push(Context {
                            siblings: Some(NativeCompletion::clean(types.to_vec())),
                            ..Context::new(self.control.clone())
                        });
                        id
                    }
                };
                let mut widened = Vec::with_capacity(types.len());
                for &child in types.iter() {
                    widened.push(if self.model.has_type_flags(child, TypeFlags::NULLABLE) {
                        child
                    } else {
                        sequence.read(self.widened_type_with_context(
                            child,
                            Some(union_context),
                            contexts,
                            consumer,
                        ))?
                    });
                }
                let mut empty = false;
                for &child in &widened {
                    if sequence.read(self.is_empty_object_type_completion(child, consumer))? {
                        empty = true;
                        break;
                    }
                }
                Some(sequence.read(self.union_type_reduced_completion(
                    widened,
                    if empty {
                        UnionReduction::Subtype
                    } else {
                        UnionReduction::Literal
                    },
                    None,
                    None,
                    consumer,
                ))?)
            } else if let Some(types) = self.model.intersection_constituents(ty) {
                let mut widened = Vec::with_capacity(types.len());
                for &child in types.iter() {
                    widened.push(sequence.read(self.widened_type_completion(child, consumer))?);
                }
                Some(sequence.read(self.intersection_type_completion(widened, consumer))?)
            } else if sequence.read(self.is_array_or_tuple_type_completion(ty))? {
                let target = self
                    .model
                    .reference_target(ty)
                    .ok_or_else(|| super::super::internal("array/tuple widening lacks target"))?;
                let mut arguments = Vec::new();
                for &argument in sequence
                    .read(self.get_type_arguments_completion(ty))?
                    .iter()
                {
                    arguments
                        .push(sequence.read(self.widened_type_completion(argument, consumer))?);
                }
                Some(sequence.read(
                    self.create_normalized_reference_completion(target, arguments, consumer),
                )?)
            } else {
                None
            };
            Ok(result)
        })();
        let result = sequence.finish(result)?;
        if context.is_none()
            && let Some(value) = *result.value()
        {
            self.widening
                .roots
                .insert(ty, result.clone().map(|_| value));
        }
        Ok(result.map(|result| result.unwrap_or(ty)))
    }

    /// `getWidenedTypeOfObjectLiteral`: own properties precede missing sibling
    /// properties and indexes. IndexInfo is newly allocated even for an
    /// unchanged value, preserving its actual declaration and components.
    fn widened_object_literal(
        &mut self,
        ty: TypeId,
        context: Option<usize>,
        contexts: &mut Vec<Context>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(context) = context
            && let Some(cached) = contexts[context].widened.get(&ty)
        {
            return Ok(cached.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let mut properties = Vec::new();
            let mut names = NameMap::new(self.control.clone());
            for property in
                sequence.read(self.properties_of_object_type_completion(ty, consumer))?
            {
                let name = self.property_key(&property, consumer)?;
                let property =
                    sequence.read(self.widened_property(property, context, contexts, consumer))?;
                names.insert(name, ());
                properties.push(property);
            }
            let mut added = false;
            if let Some(context) = context {
                for property in sequence
                    .read(self.properties_of_widening_context(context, contexts, consumer))?
                {
                    let name = self.property_key(&property, consumer)?;
                    if names.get(&name).is_none() {
                        let property = self.undefined_property(property, &name, consumer)?;
                        names.insert(name, ());
                        properties.push(property);
                        added = true;
                    }
                }
            }
            let mut indexes = Vec::new();
            for index in sequence.read(self.index_infos_of_type_completion(ty, consumer))? {
                let key = sequence.read(self.demand_completion(index.key_type))?;
                let value = sequence.read(self.demand_completion(index.value_type))?;
                let value = sequence.read(self.widened_type_completion(value, consumer))?;
                indexes.push(
                    self.model.index_info(
                        key,
                        value,
                        index.readonly,
                        index
                            .declaration
                            .map(|declaration| (declaration, index.parameter_name.clone())),
                        index.components.clone(),
                    ),
                );
            }
            // Copied declarations preserve an existing inventory's order. Adding
            // sibling declarations requires the actual native symbol comparator.
            if added {
                if properties.iter().any(|property| property.symbol.is_none()) {
                    return Err(self.widening_dependency(consumer, "native object-literal property symbols are required for sibling widening order"));
                }
                properties = self.named_member_properties(properties, None, consumer)?;
            }
            let symbol = self.model.type_symbol(ty);
            let flags = self.model.object_flags(ty)?
                & (ObjectFlags::JS_LITERAL | ObjectFlags::NON_INFERRABLE_TYPE);
            let result = self.model.structured_with_flags(
                symbol,
                MembersView {
                    properties: properties.into(),
                    call_signatures: Vec::new().into(),
                    construct_signatures: Vec::new().into(),
                    index_signatures: indexes.into(),
                },
                Metadata::default(),
                ObjectKind::Anonymous,
                flags,
            );
            Ok(result)
        })();
        let result = sequence.finish(result)?;
        if let Some(context) = context
            && contexts[context].parent.is_some()
        {
            contexts[context].widened.insert(ty, result.clone());
        }
        Ok(result)
    }

    /// `getWidenedProperty` only widens actual Property symbols. Accessor and
    /// method values keep their original symbol identity.
    fn widened_property(
        &mut self,
        property: Property,
        context: Option<usize>,
        contexts: &mut Vec<Context>,
        consumer: &Location,
    ) -> NativeResult<Property> {
        let is_property = match property.symbol {
            Some(symbol) => self.property_symbol_flags(symbol)?.property,
            None => property.kind == TypePropertyKind::Property,
        };
        if !is_property {
            return Ok(NativeCompletion::clean(property));
        }
        self.widening_property_type(&property, consumer)?
            .and_then(|original| {
                let child_context = if let Some(parent) = context {
                    let name = self.property_key(&property, consumer)?;
                    Some(if let Some(&child) = contexts[parent].children.get(&name) {
                        child
                    } else {
                        let child = contexts.len();
                        contexts.push(Context {
                            parent: Some(parent),
                            property: Some(name.clone()),
                            ..Context::new(self.control.clone())
                        });
                        contexts[parent].children.insert(name, child);
                        child
                    })
                } else {
                    None
                };
                self.widened_type_with_context(original, child_context, contexts, consumer)?
                    .and_then_value(|widened| {
                        if widened == original {
                            return Ok(property);
                        }
                        self.property_with_widened_type(property, widened, false, consumer)
                    })
            })
    }

    /// `getUndefinedProperty` shares its first clone by exact native name.
    fn undefined_property(
        &mut self,
        property: Property,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<Property, DemandError> {
        if let Some(property) = self.widening.undefined_properties.get(name) {
            return Ok(property.clone());
        }
        let ty = if self.inputs.semantic_options().exact_optional_property_types {
            self.model.missing_type()
        } else {
            self.model.special(SpecialName::Undefined)
        };
        let property = self.property_with_widened_type(property, ty, true, consumer)?;
        self.widening
            .undefined_properties
            .insert(name.clone(), property.clone());
        Ok(property)
    }

    fn property_with_widened_type(
        &mut self,
        mut property: Property,
        ty: TypeId,
        optional: bool,
        consumer: &Location,
    ) -> Result<Property, DemandError> {
        if let Some(symbol) = property.symbol {
            let symbol = self.create_property_with_type(symbol, Some(ty), optional, consumer)?;
            return self.project_property_symbol(symbol, consumer);
        }
        // Eager literal/JSON factories already expose complete value facts but
        // have no actual symbol producer. Preserve that absent association;
        // later native symbol questions still fail through their shared owner.
        property.read_type = TypeDemand::Known(ty);
        property.display = PropertyDisplayDemand::Value(property.read_type);
        property.optional |= optional;
        Ok(property)
    }

    fn widening_property_type(
        &mut self,
        property: &Property,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        match property.symbol {
            Some(symbol) => self.property_raw_type_completion(symbol, consumer),
            None => self.demand_completion(property.read_type),
        }
    }

    /// `getPropertiesOfContext`: last sibling wins each name's value while
    /// preserving the first insertion position. Spread literals are excluded.
    fn properties_of_widening_context(
        &mut self,
        context: usize,
        contexts: &mut Vec<Context>,
        consumer: &Location,
    ) -> NativeResult<Vec<Property>> {
        if let Some(properties) = &contexts[context].properties {
            return Ok(properties.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Property>, DemandError> {
            let mut properties = Vec::<Property>::new();
            let mut positions = NameMap::new(self.control.clone());
            for ty in
                sequence.read(self.siblings_of_widening_context(context, contexts, consumer))?
            {
                let flags = self.model.object_flags(ty)?;
                if flags.intersects(ObjectFlags::OBJECT_LITERAL)
                    && !flags.intersects(ObjectFlags::CONTAINS_SPREAD)
                {
                    for property in
                        sequence.read(self.properties_of_type_completion(ty, consumer))?
                    {
                        let name = self.property_key(&property, consumer)?;
                        if let Some(&index) = positions.get(&name) {
                            properties[index] = property;
                        } else {
                            positions.insert(name, properties.len());
                            properties.push(property);
                        }
                    }
                }
            }
            Ok(properties)
        })();
        let result = sequence.finish(result)?;
        // slices.Collect over an empty native OrderedMap returns nil, so an
        // empty projected list remains a cache miss on the next acquisition.
        if !result.value().is_empty() {
            contexts[context].properties = Some(result.clone());
        }
        Ok(result)
    }

    /// `getSiblingsOfContext`: child siblings use the parent's matching raw
    /// property type, distributed over unions in its established order.
    fn siblings_of_widening_context(
        &mut self,
        context: usize,
        contexts: &mut Vec<Context>,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        if let Some(siblings) = &contexts[context].siblings {
            return Ok(siblings.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<TypeId>, DemandError> {
            let parent = contexts[context]
                .parent
                .ok_or_else(|| super::super::internal("widening root lacks siblings"))?;
            let name = contexts[context]
                .property
                .clone()
                .ok_or_else(|| super::super::internal("widening child lacks its property key"))?;
            let mut siblings = Vec::new();
            for ty in
                sequence.read(self.siblings_of_widening_context(parent, contexts, consumer))?
            {
                if self
                    .model
                    .object_flags(ty)?
                    .intersects(ObjectFlags::OBJECT_LITERAL)
                {
                    if let Some(property) = sequence
                        .read(self.property_of_object_type_by_key_completion(ty, &name, consumer))?
                    {
                        let ty = sequence.read(self.widening_property_type(&property, consumer))?;
                        if let Some(types) = self.model.union_constituents(ty) {
                            siblings.extend(types.iter().copied());
                        } else {
                            siblings.push(ty);
                        }
                    }
                }
            }
            Ok(siblings)
        })();
        let result = sequence.finish(result)?;
        contexts[context].siblings = Some(result.clone());
        Ok(result)
    }

    fn widening_dependency(&self, consumer: &Location, message: &'static str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            consumer,
            crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }
}
