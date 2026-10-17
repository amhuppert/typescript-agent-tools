//! Native expression access composes receiver, key, symbol and flow operations.
//! Names locate members; only the canonical property arena supplies identity.

mod accessibility;
pub(super) mod missing;
mod private;
mod references;
pub(super) use references::AccessReferencesState;

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, PropertySymbolId, TypeFlags, TypeId, TypeView,
};
use super::super::source::{AccessKey, AccessRecipe, ExpressionRoot};
use super::contextual::CheckMode;
use super::{Evaluator, reads::AssignmentKind};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::NativePropertyKey;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;

impl Evaluator<'_, '_> {
    /// `getOptionalExpressionType`: only an optional root removes nullable
    /// types; a continuing chain removes its private optional sentinel.
    pub(super) fn optional_expression_type(
        &mut self,
        ty: TypeId,
        root: bool,
        chain: bool,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.optional_expression_type_completion(ty, root, chain, consumer)?
            .admit()
    }

    pub(super) fn optional_expression_type_completion(
        &mut self,
        ty: TypeId,
        root: bool,
        chain: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if root {
            self.non_nullable_type_completion(ty, consumer)
        } else if chain {
            Ok(NativeCompletion::clean(self.remove_optional_marker(ty)))
        } else {
            Ok(NativeCompletion::clean(ty))
        }
    }

    /// `addOptionalTypeMarker` is distinct from adding ordinary undefined.
    pub(super) fn add_optional_marker(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.add_optional_marker_completion(ty, consumer)?.admit()
    }

    pub(super) fn add_optional_marker_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if !self.inputs.semantic_options().strict_null_checks {
            return Ok(NativeCompletion::clean(ty));
        }
        let optional = self.model.optional_marker_type();
        self.union_type_ex_completion(vec![ty, optional], None, None, consumer)
    }

    /// `propagateOptionalTypeMarker` materializes undefined only at the
    /// outermost chain. Calls and access expressions share this operation.
    pub(super) fn propagate_optional_marker(
        &mut self,
        ty: TypeId,
        changed: bool,
        outermost: bool,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.propagate_optional_marker_completion(ty, changed, outermost, consumer)?
            .admit()
    }

    pub(super) fn propagate_optional_marker_completion(
        &mut self,
        ty: TypeId,
        changed: bool,
        outermost: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if !changed {
            Ok(NativeCompletion::clean(ty))
        } else if outermost {
            self.optional_type_completion(ty, false, consumer)
        } else {
            self.add_optional_marker_completion(ty, consumer)
        }
    }

    /// `checkPropertyAccessExpression` / `checkIndexedAccess` preserve their
    /// distinct native receiver/key order and optional-chain propagation.
    pub(super) fn check_access_expression_completion(
        &mut self,
        root: ExpressionRoot,
        mode: CheckMode,
        write_only: bool,
    ) -> NativeResult<TypeId> {
        let recipe = self.source.access_recipe(root)?;
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let receiver = sequence
                .read(self.check_expression_completion(&recipe.receiver, CheckMode::NORMAL))?;
            let non_optional = if recipe.optional_chain {
                sequence.read(self.optional_expression_type_completion(
                    receiver,
                    recipe.receiver_optional_root,
                    recipe.receiver_optional_chain,
                    &recipe.receiver.location,
                ))?
            } else {
                receiver
            };
            let receiver_type = sequence.read(
                self.check_non_null_type_completion(non_optional, &recipe.receiver.location),
            )?;
            let assignment = self.assignment_kind(root.file, recipe.assignment_target)?;
            let value = match &recipe.key {
                AccessKey::Name { .. } => sequence.read(self.dot_access_completion(
                    &recipe,
                    receiver_type,
                    assignment,
                    mode,
                    write_only && !recipe.optional_chain,
                ))?,
                AccessKey::Element(key) => {
                    let receiver_type = if assignment != AssignmentKind::None || recipe.method_call
                    {
                        sequence.read(
                            self.widened_type_completion(receiver_type, &recipe.receiver.location),
                        )?
                    } else {
                        receiver_type
                    };
                    let index =
                        sequence.read(self.check_expression_completion(key, CheckMode::NORMAL))?;
                    sequence.read(self.element_access_completion(
                        &recipe,
                        receiver_type,
                        index,
                        assignment,
                        mode,
                    ))?
                }
            };
            sequence.read(self.propagate_optional_marker_completion(
                value,
                recipe.optional_chain && non_optional != receiver,
                recipe.outermost_chain,
                &recipe.location,
            ))
        })();
        sequence.finish(result)
    }

    /// `checkPropertyAccessExpressionOrQualifiedName` retains the original
    /// left type through widening/apparent lookup. Private lexical lookup,
    /// marking, metadata installation, accessibility, and value acquisition
    /// are stages of this same owner for ordinary and private access alike.
    fn dot_access_completion(
        &mut self,
        recipe: &AccessRecipe,
        left_type: TypeId,
        assignment: AssignmentKind,
        mode: CheckMode,
        write_only: bool,
    ) -> NativeResult<TypeId> {
        use super::emit_helpers::ExternalEmitHelpers;
        use super::{ReferenceDeclaration, ReferenceSymbol};
        use crate::project::binding::NativePropertyKey;
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let AccessKey::Name {
                name,
                private,
                declaration,
                location: name_location,
            } = &recipe.key
            else {
                unreachable!()
            };
            let left = recipe.receiver.root.ok_or_else(|| {
                super::super::internal("access receiver lacks actual syntax identity")
            })?;
            let parent_symbol = self.reference_resolved_symbol_if_any(left)?;
            let widened = if assignment != AssignmentKind::None || recipe.method_call {
                sequence.read(self.widened_type_completion(left_type, &recipe.receiver.location))?
            } else {
                left_type
            };
            let apparent =
                sequence.read(self.apparent_type_completion(widened, &recipe.location))?;
            let any_like = self.model.has_type_flags(apparent, TypeFlags::ANY)
                || apparent == self.model.silent_never();
            let property = if *private {
                if self
                    .inputs
                    .semantic_options()
                    .private_access_needs_emit_helpers()
                {
                    let node = self.source.declaration_from_expression_root(recipe.root)?;
                    if assignment != AssignmentKind::None {
                        sequence.read(self.check_external_emit_helpers_completion(
                            node,
                            ExternalEmitHelpers::PRIVATE_SET,
                            &recipe.location,
                        ))?;
                    }
                    if assignment != AssignmentKind::Definite {
                        sequence.read(self.check_external_emit_helpers_completion(
                            node,
                            ExternalEmitHelpers::PRIVATE_GET,
                            &recipe.location,
                        ))?;
                    }
                }
                let lexical =
                    self.lookup_private_identifier_declaration(name, *declaration, name_location)?;
                if assignment != AssignmentKind::None
                    && let Some(lexical) = lexical
                    && let Some(value) =
                        self.reference_symbol_value_declaration(lexical, name_location)?
                {
                    // The private-method write grammar diagnostic changes no
                    // type, but observes the actual declaration at this point.
                    let _method_write =
                        self.reference_declaration_kind(value)? == SyntaxKind::MethodDeclaration;
                }
                if any_like {
                    if lexical.is_some() {
                        return Ok(if self.model.is_error_type(apparent) {
                            self.model.native_error_type()
                        } else {
                            apparent
                        });
                    }
                    if self
                        .source
                        .private_identifier_containing_class(*declaration)?
                        .is_none()
                    {
                        // Native reports the grammar error and returns its
                        // ordinary anyType, distinct from errorType recovery.
                        return Ok(self
                            .model
                            .special(crate::project::semantic::model::SpecialName::Any));
                    }
                }
                let property = match lexical {
                    Some(lexical) => {
                        sequence.read(self.private_identifier_property_of_type_completion(
                            left_type,
                            lexical,
                            name_location,
                        ))?
                    }
                    None => None,
                };
                if property.is_none() {
                    if sequence.read(self.check_private_identifier_property_access_completion(
                        left_type,
                        name,
                        lexical,
                        name_location,
                    ))? {
                        return sequence.read(NativeCompletion::recovered(
                            self.model.native_error_type(),
                            self.access_error(
                                name_location,
                                "private property access consumes native error recovery",
                            ),
                        ));
                    }
                    // Plain-JS private-declaration diagnostics are outside the
                    // admitted TypeScript source profile and add no TS demand.
                } else if let Some(property) = &property {
                    let symbol = self.property_symbol(property, name_location)?;
                    let flags = self.property_symbol_flags(symbol)?;
                    let _set_only_read = flags.set_accessor
                        && !flags.get_accessor
                        && assignment != AssignmentKind::Definite;
                }
                property
            } else {
                if any_like {
                    if left.site.kind == SyntaxKind::Identifier && parent_symbol.is_some() {
                        sequence.read(self.mark_linked_property_references_completion(
                            recipe.root,
                            None,
                            left_type,
                            &recipe.location,
                        ))?;
                    }
                    return Ok(if self.model.is_error_type(apparent) {
                        self.model.native_error_type()
                    } else {
                        apparent
                    });
                }
                let skip_augmentation = self.is_const_enum_object_type(apparent, name_location)?;
                sequence.read(self.property_of_type_by_key_flags_completion(
                    apparent,
                    &NativePropertyKey::fixed(name.as_bytes(), self.control.as_ref()),
                    skip_augmentation,
                    recipe.root.site.kind == SyntaxKind::QualifiedName,
                    name_location,
                ))?
            };
            let symbol = property
                .as_ref()
                .map(|property| self.property_symbol(property, name_location))
                .transpose()?;
            sequence.read(self.mark_linked_property_references_completion(
                recipe.root,
                symbol,
                left_type,
                &recipe.location,
            ))?;
            let ty = if let Some(symbol) = symbol {
                let native_symbol = ReferenceSymbol::from_property(symbol);
                let target = sequence.read(
                    self.alias_with_deprecation_check_completion(native_symbol, name_location),
                )?;
                if self.is_deprecated_symbol(target, name_location)?
                    && self.is_uncalled_function_reference(recipe.root, target, name_location)?
                {
                    let declarations = self.reference_symbol_declarations(target, name_location)?;
                    if !declarations.is_empty() {
                        self.add_deprecated_symbol_suggestion(&declarations)?;
                    }
                }
                sequence.read(self.check_property_not_used_before_declaration_completion(
                    symbol,
                    recipe.root,
                    &recipe.location,
                ))?;
                let self_access = sequence.read(self.is_self_type_access_completion(
                    left,
                    parent_symbol,
                    &recipe.location,
                ))?;
                self.mark_property_as_referenced(
                    symbol,
                    Some(recipe),
                    self_access,
                    &recipe.location,
                )?;
                // Native installs resolvedSymbol before any accessibility
                // displays or readonly result; interrupted suffixes retain it.
                self.record_access_symbol(recipe.root, symbol);
                sequence.read(self.check_property_accessibility_completion(
                    recipe.root,
                    left.site.kind == SyntaxKind::SuperKeyword,
                    recipe.access_kind != crate::syntax::AccessKind::Read,
                    apparent,
                    symbol,
                    &recipe.location,
                ))?;
                if sequence.read(
                    self.assignment_to_readonly_property_completion(recipe, symbol, assignment),
                )? {
                    return sequence.read(NativeCompletion::recovered(
                        self.model.native_error_type(),
                        self.access_error(
                            name_location,
                            "assignment consumed a readonly property error type",
                        ),
                    ));
                }
                if self.property_access_auto_type(symbol, recipe.root, &recipe.location)? {
                    let automatic = self.model.automatic_variable();
                    return sequence.read(self.flow_type_of_access_completion(
                        recipe,
                        Some(symbol),
                        automatic,
                        assignment,
                        mode,
                    ));
                }
                if write_only || recipe.access_kind == crate::syntax::AccessKind::Write {
                    sequence.read(self.property_write_type_completion(symbol, name_location))?
                } else {
                    sequence.read(self.property_raw_type_completion(symbol, name_location))?
                }
            } else {
                let this_parameter = matches!(self.model.get(left_type)?, TypeView::TypeParameter(parameter) if parameter.is_this);
                let index_allowed = !*private
                    && (assignment == AssignmentKind::None
                        || !sequence.read(
                            self.is_generic_object_type_completion(left_type, name_location),
                        )?
                        || this_parameter);
                let index = if index_allowed {
                    sequence.read(self.applicable_index_info_for_key_completion(
                        apparent,
                        &NativePropertyKey::fixed(name.as_bytes(), self.control.as_ref()),
                        name_location,
                    ))?
                } else {
                    None
                };
                let Some(index) = index else {
                    if let Some(type_symbol) =
                        self.reference_symbol_of_type(left_type, name_location)?
                    {
                        let global = self.global_this_reference_symbol();
                        if self.reference_symbol_identity(type_symbol, name_location)?
                            == self.reference_symbol_identity(global, name_location)?
                        {
                            let property = match self.reference_raw_symbol_table(global, true)? {
                                Some(table) => self.reference_raw_table_lookup(
                                    table,
                                    &NativePropertyKey::fixed(
                                        name.as_bytes(),
                                        self.control.as_ref(),
                                    ),
                                    name_location,
                                )?,
                                None => None,
                            };
                            if property
                                .map(|property| {
                                    self.reference_symbol_flags(property, name_location)
                                        .map(|flags| flags.is_block_scoped())
                                })
                                .transpose()?
                                .unwrap_or(false)
                                || self.inputs.semantic_options().no_implicit_any
                            {
                                sequence.read(self.check_diagnostic_type_display_completion(
                                    left_type,
                                    name_location,
                                ))?;
                            }
                            return sequence.read(NativeCompletion::recovered(
                                self.model
                                    .special(crate::project::semantic::model::SpecialName::Any),
                                self.access_error(
                                    name_location,
                                    "globalThis property access consumes native any fallback",
                                ),
                            ));
                        }
                    }
                    if !name.is_empty()
                        && !sequence.read(
                            self.check_and_report_error_for_extending_interface_completion(
                                recipe.root,
                                name_location,
                            ),
                        )?
                    {
                        sequence.read(self.report_nonexistent_property_completion(
                            *declaration,
                            if this_parameter { apparent } else { left_type },
                            false,
                            name_location,
                        ))?;
                    }
                    return sequence.read(NativeCompletion::recovered(
                        self.model.native_error_type(),
                        self.access_error(
                            name_location,
                            "property and applicable index signature are absent",
                        ),
                    ));
                };
                if index.readonly
                    && (assignment != AssignmentKind::None
                        || self.source.access_is_delete_target(recipe.root)?)
                {
                    sequence.read(
                        self.check_diagnostic_type_display_completion(apparent, name_location),
                    )?;
                }
                let ty = sequence.read(self.demand_completion(index.value_type))?;
                let ty = if assignment != AssignmentKind::Definite {
                    sequence.read(
                        self.include_undefined_in_index_signature_completion(ty, name_location),
                    )?
                } else {
                    ty
                };
                if let Some(declaration) = index.declaration {
                    if self.source.deprecated_declaration(declaration)? {
                        self.add_deprecated_symbol_suggestion(&[ReferenceDeclaration::Captured(
                            declaration,
                        )])?;
                    }
                }
                ty
            };
            sequence.read(self.flow_type_of_access_completion(recipe, symbol, ty, assignment, mode))
        })();
        sequence.finish(result)
    }

    /// `checkElementAccessExpression`: checked key precedes the error and
    /// const-enum gates. The private-name dot lookup is never used here.
    fn element_access_completion(
        &mut self,
        recipe: &AccessRecipe,
        receiver: TypeId,
        index: TypeId,
        assignment: AssignmentKind,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        if self.model.is_error_type(receiver) || receiver == self.model.silent_never() {
            return Ok(NativeCompletion::clean(receiver));
        }
        let AccessKey::Element(key) = &recipe.key else {
            unreachable!()
        };
        let index_root = key
            .root
            .ok_or_else(|| super::super::internal("element index lacks actual syntax identity"))?;
        if self.is_const_enum_object_type(receiver, &recipe.location)?
            && !matches!(
                index_root.site.kind,
                SyntaxKind::StringLiteral | SyntaxKind::NoSubstitutionTemplateLiteral
            )
        {
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                self.access_error(
                    &key.location,
                    "const enum element access consumes a non-string-literal error type",
                ),
            );
        }
        self.indexed_element_value_completion(recipe, receiver, index, assignment)?
            .and_then(|value| {
                self.flow_type_of_access_completion(
                    recipe,
                    value.symbol,
                    value.ty,
                    assignment,
                    mode,
                )?
                .and_then(|ty| {
                    self.check_indexed_access_index_type_completion(ty, &recipe.location)
                })
            })
    }

    /// `isAssignmentToReadonlyEntity`: the canonical symbol decides readonly;
    /// a same-class this assignment in its constructor has a syntax exemption.
    pub(super) fn assignment_to_readonly_property_completion(
        &mut self,
        recipe: &AccessRecipe,
        symbol: PropertySymbolId,
        assignment: AssignmentKind,
    ) -> NativeResult<bool> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            if assignment == AssignmentKind::None {
                return Ok(false);
            }
            let receiver = self
                .source
                .readonly_access_receiver_identifier(recipe.root)?;
            if let Some(receiver) = receiver {
                let symbol =
                    sequence.read(self.reference_resolved_identifier_symbol_completion(
                        receiver,
                        &recipe.location,
                    ))?;
                if self
                    .reference_symbol_flags(symbol, &recipe.location)?
                    .module_exports
                {
                    return Ok(false);
                }
            }
            if sequence.read(self.property_is_readonly_completion(symbol, &recipe.location))? {
                if self.property_symbol_flags(symbol)?.property {
                    if let Some(declaration) =
                        self.property_reference_value_declaration(symbol, &recipe.location)?
                    {
                        if self
                            .reference_property_access_context(declaration, recipe.root)?
                            .readonly_constructor_write
                        {
                            return Ok(false);
                        }
                    }
                }
                return Ok(true);
            }
            if let Some(receiver) = receiver {
                let symbol =
                    sequence.read(self.reference_resolved_identifier_symbol_completion(
                        receiver,
                        &recipe.location,
                    ))?;
                if self.reference_symbol_flags(symbol, &recipe.location)?.alias
                    && let Some(declaration) =
                        self.reference_alias_declaration(symbol, &recipe.location)?
                {
                    return Ok(self.reference_declaration_kind(declaration)?
                        == SyntaxKind::NamespaceImport);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `getFlowTypeOfAccessExpression`: symbol flags, optional writes and
    /// strict property initialization feed the shared native reference walk.
    fn flow_type_of_access_completion(
        &mut self,
        recipe: &AccessRecipe,
        symbol: Option<PropertySymbolId>,
        mut ty: TypeId,
        assignment: AssignmentKind,
        mode: CheckMode,
    ) -> NativeResult<TypeId> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let flags = symbol
                .map(|symbol| self.property_symbol_flags(symbol))
                .transpose()?;
            if assignment == AssignmentKind::Definite {
                return self
                    .model
                    .remove_missing_type(ty, flags.is_some_and(|flags| flags.optional))
                    .map_err(|error| error.at(self.inputs, &recipe.location));
            }
            if flags.is_some_and(|flags| {
                !(flags.variable
                    || flags.property
                    || flags.get_accessor
                    || flags.set_accessor
                    || flags.method && self.model.has_type_flags(ty, TypeFlags::UNION))
            }) {
                return Ok(ty);
            }
            if self.model.is_automatic(ty) {
                return sequence.read(self.flow_type_of_property_completion(
                    recipe.root,
                    symbol,
                    &recipe.location,
                ));
            }
            ty = sequence.read(self.narrowable_type_for_reference_completion(
                ty,
                recipe.root,
                mode,
                &recipe.location,
            ))?;
            let mut initial = ty;
            let mut assume_uninitialized = false;
            if self.inputs.semantic_options().strict_null_checks {
                if let Some(symbol) = symbol {
                    if let Some(declaration) =
                        self.property_reference_value_declaration(symbol, &recipe.location)?
                    {
                        assume_uninitialized = self
                            .reference_property_access_context(declaration, recipe.root)?
                            .assume_uninitialized;
                        if assume_uninitialized {
                            initial = sequence.read(self.optional_type_completion(
                                ty,
                                false,
                                &recipe.location,
                            ))?;
                        }
                    }
                }
            }
            let flow = sequence.read(self.access_reference_flow_completion(
                recipe.root,
                ty,
                initial,
                &recipe.location,
            ))?;
            if assume_uninitialized
                && !self.model.contains_undefined(ty)
                && self.model.contains_undefined(flow)
            {
                if let Some(symbol) = symbol {
                    sequence.read(self.diagnostic_symbol_display_completion(
                        super::ReferenceSymbol::from_property(symbol),
                        &recipe.location,
                    ))?;
                }
                return Err(self.flow_required(
                &recipe.location,
                "property used-before-assignment consumes excluded definite-assignment analysis",
            ));
            }
            if assignment != AssignmentKind::None {
                sequence.read(self.base_type_of_literal_completion(flow, &recipe.location))
            } else {
                Ok(flow)
            }
        })();
        sequence.finish(result)
    }

    /// `getFlowTypeOfProperty` acquires an inherited initial type before the
    /// shared automatic-flow admission; ambient uses own ModifierFlags().
    fn flow_type_of_property_completion(
        &mut self,
        root: ExpressionRoot,
        property: Option<PropertySymbolId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let mut initial = self
                .model
                .special(super::super::model::SpecialName::Undefined);
            if let Some(property) = property
                && let Some(declaration) =
                    self.property_reference_value_declaration(property, consumer)?
                && (!self.reference_property_is_auto_typed(declaration)?
                    || self.reference_declaration_own_modifier_flags(declaration)?
                        & crate::syntax::modifiers::AMBIENT
                        != 0)
            {
                if let Some(base) = sequence
                    .read(self.type_of_property_in_base_class_completion(property, consumer))?
                {
                    initial = base;
                }
            }
            let automatic = self.model.automatic_variable();
            sequence.read(self.access_reference_flow_completion(root, automatic, initial, consumer))
        })();
        sequence.finish(result)
    }

    /// `getTypeOfPropertyInBaseClass` only asks the first native base type.
    pub(super) fn type_of_property_in_base_class_completion(
        &mut self,
        property: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.property_declaring_class_completion(property, consumer)?
            .and_then(|class| {
                let Some(class) = class else {
                    return Ok(NativeCompletion::clean(None));
                };
                self.base_types_completion(class, consumer)?
                    .and_then(|bases| {
                        let Some(&base) = bases.first() else {
                            return Ok(NativeCompletion::clean(None));
                        };
                        let name = self.property_symbol_name_facts(property, consumer)?.key;
                        self.type_of_property_of_type_by_key_completion(base, &name, consumer)
                    })
            })
    }

    /// `removeOptionalTypeMarker` removes only the native chain sentinel.
    pub(super) fn remove_optional_marker(&mut self, ty: TypeId) -> TypeId {
        if !self.inputs.semantic_options().strict_null_checks {
            return ty;
        }
        let marker = self.model.optional_marker_type();
        self.model.remove_type(ty, marker)
    }

    /// `getAccessedPropertyName` / `tryGetElementAccessExpressionName` ask
    /// literal syntax first, then the exact entity-name constant operation.
    pub(super) fn accessed_property_name_completion(
        &mut self,
        file: FileId,
        site: Site,
        consumer: &Location,
    ) -> NativeResult<Option<NativePropertyKey>> {
        if site.kind == SyntaxKind::BindingElement {
            let declaration = self
                .source
                .declaration_from_expression_root(ExpressionRoot { file, site })?;
            return Ok(self
                .destructuring_property_name_completion(declaration)?
                .map(|name| {
                    name.map(|name| {
                        NativePropertyKey::fixed(name.as_bytes(), self.control.as_ref())
                    })
                }));
        }
        if site.kind == SyntaxKind::Parameter {
            return Ok(NativeCompletion::clean(Some(NativePropertyKey::fixed(
                self.source
                    .parameter_access_index(ExpressionRoot { file, site })?
                    .as_bytes(),
                self.control.as_ref(),
            ))));
        }
        if !matches!(
            site.kind,
            SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
        ) {
            return Ok(NativeCompletion::clean(None));
        }
        let recipe = self.source.access_recipe(ExpressionRoot { file, site })?;
        match recipe.key {
            AccessKey::Name { name, .. } => Ok(NativeCompletion::clean(Some(
                NativePropertyKey::fixed(name.as_bytes(), self.control.as_ref()),
            ))),
            AccessKey::Element(expression) => {
                let root = expression.root.ok_or_else(|| {
                    super::super::internal("element key has no exact source role")
                })?;
                match self.source.element_access_name_syntax(root)? {
                    super::super::source::ElementAccessName::Text(text) => {
                        Ok(NativeCompletion::clean(Some(NativePropertyKey::fixed(
                            text.as_bytes(),
                            self.control.as_ref(),
                        ))))
                    }
                    super::super::source::ElementAccessName::Entity => {
                        self.name_from_entity_expression_completion(root, consumer)
                    }
                    super::super::source::ElementAccessName::None => {
                        Ok(NativeCompletion::clean(None))
                    }
                }
            }
        }
    }

    /// `tryGetNameFromEntityNameExpression`: the same Value-only resolver
    /// serves identifier and qualified syntax. Annotation precedes the native
    /// initializer/before-use branch; enum auto-values use name text here,
    /// deliberately not the different getEnumMemberValue operation.
    fn name_from_entity_expression_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<Option<NativePropertyKey>> {
        use super::ReferenceDeclaration;
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| -> Result<Option<NativePropertyKey>, DemandError> {
            let Some(symbol) =
                sequence.read(self.resolve_value_entity_name_completion(root, consumer))?
            else {
                return Ok(None);
            };
            if !self.is_constant_variable_symbol(symbol, consumer)?
                && !self.reference_symbol_flags(symbol, consumer)?.enum_member
            {
                return Ok(None);
            }
            let Some(declaration) = self.reference_symbol_value_declaration(symbol, consumer)?
            else {
                return Ok(None);
            };
            let ReferenceDeclaration::Captured(declaration) = declaration else {
                return Err(super::super::internal(
                    "constant variable or enum member has a non-syntax value declaration",
                )
                .into());
            };
            if let Some(annotation) = self.source.declaration_type_annotation(declaration)? {
                let ty = sequence.read(self.check_type_completion(&annotation))?;
                if let Some(name) = self.property_key_from_type(ty)? {
                    return Ok(Some(name));
                }
            }
            let Some(key) = self.source.constant_key_recipe(declaration)? else {
                return Ok(None);
            };
            if sequence.read(self.block_scoped_name_declared_before_use_completion(
                declaration,
                root,
                consumer,
            ))? {
                if let Some(initializer) = key.initializer {
                    let ty =
                        sequence.read(self.type_of_source_expression_completion(&initializer))?;
                    return self.property_key_from_type(ty);
                }
                if let Some(name) = key.enum_name {
                    return Ok(self.source.try_property_name_text(name)?.map(|name| {
                        NativePropertyKey::fixed(name.as_bytes(), self.control.as_ref())
                    }));
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    pub(super) fn access_pending(&self, consumer: &Location, message: &str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::UnsupportedSemanticForm,
            message,
        )
    }

    pub(super) fn access_error(&self, consumer: &Location, message: &str) -> DemandError {
        super::super::invalid_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::ErrorType,
            message,
        )
        .into()
    }
}
