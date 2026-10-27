//! Immediate class inheritance checks. Diagnostic-only incompatibility keeps
//! checking; acquired symbol/type recovery remains in the native completion.
use super::super::super::model::{
    DemandError, MembersView, NativeResult, NativeSequence, ObjectFlags, PropertySymbolId,
    TypeFlags, TypeId, TypeView,
};
use super::super::super::source::{
    DeclarationIdentity, ExpressionRoot, TypeExpressionForm, class_checks::ClassCheckRecipe,
};
use super::super::{
    Evaluator, ReferenceDeclaration, ReferenceSymbol,
    property_symbols::checks,
    relations::{RelationKind, RelationReporting},
    spelling::SuggestionMeaning,
};
use crate::references::Location;
use crate::syntax::{self, modifiers};
use crate::syntax_kind::SyntaxKind as K;

impl Evaluator<'_, '_> {
    /// `checkClassLikeDeclaration`: extends, override modifiers, then implements.
    /// Instance incompatibility suppresses only the static-side relation.
    pub(super) fn check_class_heritage_completion(
        &mut self,
        recipe: &ClassCheckRecipe,
        ty: TypeId,
        with_this: TypeId,
        static_type: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let this = self.model.target_this_parameter(ty)?;
            let extends = self.source.class_extends_recipe(recipe.declaration)?;
            if let Some(base) = &extends {
                for argument in &base.type_arguments {
                    let root = argument.root.ok_or_else(|| {
                        super::super::super::internal("heritage argument has no native node")
                    })?;
                    sequence.read(self.check_type_source_element_completion(root))?;
                }
                let bases = sequence.read(self.base_types_completion(ty, consumer))?;
                if let Some(&base_type) = bases.first() {
                    let constructor =
                        sequence.read(self.base_constructor_type_completion(ty, consumer))?;
                    let static_base =
                        sequence.read(self.apparent_type_completion(constructor, consumer))?;
                    sequence.read(self.check_base_type_accessibility_completion(
                        static_base,
                        &base.reference,
                        consumer,
                    ))?;
                    // `checkSourceElement(base.Expression())` has no expression
                    // arm in checkSourceElementWorker. The base constructor
                    // owner has already performed its separate expression read.
                    if !base.type_arguments.is_empty() {
                        for argument in &base.type_arguments {
                            sequence.read(self.check_type_source_element_completion(
                                argument.root.ok_or_else(|| {
                                    super::super::super::internal(
                                        "heritage argument has no native node",
                                    )
                                })?,
                            ))?;
                        }
                        for signature in
                            sequence.read(self.constructors_for_type_arguments_completion(
                                static_base,
                                base.type_arguments.len(),
                                consumer,
                            ))?
                        {
                            if !sequence.read(
                                self.check_supplied_type_argument_constraints_completion(
                                    &base.reference,
                                    &base.type_arguments,
                                    signature.type_parameter_types(),
                                ),
                            )? {
                                break;
                            }
                        }
                    }
                    let base_with_this =
                        sequence.read(self.get_type_with_this_argument_completion(
                            base_type, this, false, consumer,
                        ))?;
                    if !sequence.read(self.check_type_related_to_completion(
                        with_this,
                        base_with_this,
                        RelationKind::Assignable,
                        None,
                        consumer,
                    ))? {
                        sequence.read(self.issue_class_member_specific_error_completion(
                            recipe,
                            with_this,
                            base_with_this,
                            None,
                            consumer,
                        ))?;
                    } else {
                        let target = sequence
                            .read(self.type_without_signatures_completion(static_base, consumer))?;
                        let node = self.class_check_error_node(recipe.declaration)?;
                        sequence.read(self.check_type_related_to_completion(
                            static_type,
                            target,
                            RelationKind::Assignable,
                            RelationReporting::Report.class_heritage_at(node, None, true),
                            consumer,
                        ))?;
                    }
                    let variable = self
                        .model
                        .has_type_flags(constructor, TypeFlags::TYPE_VARIABLE);
                    if variable
                        && sequence.read(
                            self.is_mixin_constructor_type_completion(static_type, consumer),
                        )?
                    {
                        // Abstractness diagnostics only inspect signature flags.
                        sequence
                            .read(self.construct_signatures_completion(constructor, consumer))?;
                    }
                    let class = self
                        .reference_symbol_of_type(static_base, consumer)?
                        .map(|symbol| {
                            self.reference_symbol_flags(symbol, consumer)
                                .map(|flags| flags.class)
                        })
                        .transpose()?
                        .unwrap_or(false);
                    if !class && !variable {
                        for signature in sequence.read(
                            self.instantiated_constructors_for_type_arguments_completion(
                                static_base,
                                &base.type_arguments,
                                self.inputs.in_javascript_file(base.reference.file),
                                consumer,
                            ),
                        )? {
                            let returned = sequence.read(
                                self.get_return_type_of_signature_completion(&signature, consumer),
                            )?;
                            if !sequence.read(self.type_related_completion(
                                returned,
                                base_type,
                                RelationKind::Identity,
                                consumer,
                            ))? {
                                break;
                            }
                        }
                    }
                    sequence.read(self.check_kinds_of_property_member_overrides_completion(
                        ty, base_type, consumer,
                    ))?;
                }
            }
            sequence.read(self.check_members_for_override_modifier_completion(
                recipe,
                ty,
                with_this,
                static_type,
                extends.is_some(),
                consumer,
            ))?;
            for implemented in &recipe.implements {
                // Invalid entity/optional-chain syntax emits grammar diagnostics
                // before these same type-reference checks, without type reads.
                if let TypeExpressionForm::Reference { arguments, .. } = &implemented.form {
                    for argument in arguments {
                        sequence.read(self.check_type_source_element_completion(
                            argument.root.ok_or_else(|| {
                                super::super::super::internal(
                                    "implements argument has no native node",
                                )
                            })?,
                        ))?;
                    }
                }
                sequence.read(self.check_type_reference_or_import_completion(implemented))?;
                let target = sequence.read(self.check_type_completion(implemented))?;
                let target = sequence.read(self.reduced_type_completion(target, consumer))?;
                if !self.model.is_error_type(target)
                    && sequence.read(self.valid_base_type_completion(target, consumer))?
                {
                    let class = self
                        .reference_symbol_of_type(target, consumer)?
                        .map(|symbol| {
                            self.reference_symbol_flags(symbol, consumer)
                                .map(|flags| flags.class)
                        })
                        .transpose()?
                        .unwrap_or(false);
                    let target = sequence.read(
                        self.get_type_with_this_argument_completion(target, this, false, consumer),
                    )?;
                    if !sequence.read(self.check_type_related_to_completion(
                        with_this,
                        target,
                        RelationKind::Assignable,
                        None,
                        consumer,
                    ))? {
                        sequence.read(self.issue_class_member_specific_error_completion(
                            recipe,
                            with_this,
                            target,
                            Some(class),
                            consumer,
                        ))?;
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    fn class_check_error_node(
        &mut self,
        declaration: DeclarationIdentity,
    ) -> Result<ExpressionRoot, DemandError> {
        let node = self
            .source
            .pseudo_child(declaration, "name")?
            .unwrap_or(declaration);
        self.source.pseudo_node_expression(node)
    }

    /// `checkBaseTypeAccessibility` asks only the first raw constructor's
    /// private modifier, then class ancestry and fully qualified symbol display.
    fn check_base_type_accessibility_completion(
        &mut self,
        ty: TypeId,
        base: &super::super::super::source::TypeExpression,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let signatures = sequence.read(self.construct_signatures_completion(ty, consumer))?;
            let Some(first) = signatures.first() else {
                return Ok(());
            };
            let Some(root) = self.signature_declaration_root(first.id)? else {
                return Ok(());
            };
            if self.source.signature_declaration_modifiers(root)? & modifiers::PRIVATE == 0 {
                return Ok(());
            }
            let symbol = self
                .reference_symbol_of_type(ty, consumer)?
                .ok_or_else(|| {
                    super::super::super::internal(
                        "private base constructor has no native class symbol",
                    )
                })?;
            let class = self.reference_class_declaration(symbol, consumer)?;
            let node = base.root.ok_or_else(|| {
                super::super::super::internal("base reference has no native node")
            })?;
            let enclosing = self.source.enclosing_classes(node)?;
            if !class.is_some_and(|class| enclosing.contains(&class)) {
                sequence
                    .read(self.fully_qualified_symbol_display_completion(symbol, None, consumer))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `getTypeWithoutSignatures` retains the original raw members and
    /// properties, but no indexes or signatures, in a fresh resolved object.
    fn type_without_signatures_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.model.has_type_flags(ty, TypeFlags::OBJECT) {
                let members = sequence
                    .read(self.structured_type_members_completion(ty, consumer))?
                    .ok_or_else(|| {
                        super::super::super::internal("object has no structured member inventory")
                    })?;
                if !members.call_signatures.is_empty() || !members.construct_signatures.is_empty() {
                    let table = self.structured_member_table(ty, &members);
                    return sequence.read(self.new_resolved_anonymous_type_completion(
                        self.model.type_symbol(ty),
                        table,
                        MembersView {
                            properties: members.properties,
                            call_signatures: [].into(),
                            construct_signatures: [].into(),
                            index_signatures: [].into(),
                        },
                        ObjectFlags::NONE,
                        consumer,
                    ));
                }
            } else if let TypeView::Intersection(types) = self.model.get(ty)? {
                let mut stripped = Vec::new();
                for child in types.iter().copied() {
                    stripped.push(
                        sequence.read(self.type_without_signatures_completion(child, consumer))?,
                    );
                }
                return sequence.read(self.intersection_type_completion(stripped, consumer));
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    /// `issueMemberSpecificError` performs member relations in source order,
    /// then repeats the broad relation with reporting only if none failed.
    fn issue_class_member_specific_error_completion(
        &mut self,
        recipe: &ClassCheckRecipe,
        ty: TypeId,
        base: TypeId,
        implemented: Option<bool>,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut issued = false;
            for &member in &recipe.members {
                let declaration = self.source.type_node_declaration(member)?;
                if self.source.declaration_is_static(declaration)? {
                    continue;
                }
                let Some(symbol) = sequence
                    .read(self.reference_node_declaration_symbol_completion(declaration))?
                else {
                    continue;
                };
                let key = self.reference_symbol_key(symbol, consumer)?;
                if key.fixed_bytes() == Some(b"\xfecomputed") {
                    continue;
                }
                let property =
                    sequence.read(self.property_of_type_by_key_completion(ty, &key, consumer))?;
                let base_property =
                    sequence.read(self.property_of_type_by_key_completion(base, &key, consumer))?;
                if let (Some(property), Some(base_property)) = (property, base_property) {
                    let property = self.property_symbol(&property, consumer)?;
                    let base_property = self.property_symbol(&base_property, consumer)?;
                    let source = sequence.read(self.value_reference_type_completion(
                        ReferenceSymbol::from_property(property),
                        consumer,
                    ))?;
                    let target = sequence.read(self.value_reference_type_completion(
                        ReferenceSymbol::from_property(base_property),
                        consumer,
                    ))?;
                    let node = self.class_check_error_node(declaration)?;
                    if !sequence.read(self.check_type_related_to_completion(
                        source,
                        target,
                        RelationKind::Assignable,
                        RelationReporting::Report.assignment_at(node),
                        consumer,
                    ))? {
                        sequence
                            .read(self.diagnostic_symbol_display_completion(symbol, consumer))?;
                        sequence
                            .read(self.check_diagnostic_type_display_completion(ty, consumer))?;
                        sequence
                            .read(self.check_diagnostic_type_display_completion(base, consumer))?;
                        issued = true;
                    }
                }
            }
            if !issued {
                let node = self.class_check_error_node(recipe.declaration)?;
                sequence.read(self.check_type_related_to_completion(
                    ty,
                    base,
                    RelationKind::Assignable,
                    RelationReporting::Report.class_heritage_at(node, implemented, false),
                    consumer,
                ))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `arePropertiesAbstractOrInterface`: synthetic intersection properties
    /// require any eligible declaration; ordinary symbols require every one.
    fn properties_abstract_or_interface(
        &mut self,
        property: PropertySymbolId,
        modifiers: u32,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        let any = self.property_check_flags(property)? & checks::SYNTHETIC != 0;
        for declaration in self.property_reference_declarations(property, consumer)? {
            let matches = match declaration {
                ReferenceDeclaration::Captured(declaration) => {
                    let parent = self.source.declaration_parent(declaration)?;
                    let interface = parent
                        .map(|parent| self.source.declaration_kind(parent))
                        .transpose()?
                        == Some(K::InterfaceDeclaration);
                    interface
                        || modifiers & modifiers::ABSTRACT != 0
                            && (self.source.declaration_kind(declaration)?
                                != K::PropertyDeclaration
                                || self
                                    .source
                                    .pseudo_child(declaration, "initializer")?
                                    .is_none())
                }
                ReferenceDeclaration::Json { .. } => false,
            };
            if matches == any {
                return Ok(any);
            }
        }
        Ok(!any)
    }

    /// `checkKindsOfPropertyMemberOverrides` uses target-symbol identity and
    /// raw declaration flags. It does not check deferred member bodies.
    fn check_kinds_of_property_member_overrides_completion(
        &mut self,
        ty: TypeId,
        base_type: TypeId,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            'property: for property in sequence
                .read(self.properties_of_type_completion(base_type, consumer))?
                .iter()
            {
                let base_property = self.property_symbol(property, consumer)?;
                let base = self.property_target_symbol(base_property)?;
                let base_flags = self.property_symbol_flags(base)?;
                if base_flags.prototype {
                    continue;
                }
                let key =
                    self.reference_symbol_key(ReferenceSymbol::from_property(base), consumer)?;
                let Some(derived) = sequence
                    .read(self.property_of_object_type_by_key_completion(ty, &key, consumer))?
                else {
                    continue;
                };
                let derived = self.property_symbol(&derived, consumer)?;
                let derived = self.property_target_symbol(derived)?;
                let base_modifiers = self.property_declaration_modifiers(base, false, consumer)?;
                if self.reference_symbol_identity(ReferenceSymbol::from_property(base), consumer)?
                    == self.reference_symbol_identity(
                        ReferenceSymbol::from_property(derived),
                        consumer,
                    )?
                {
                    if base_modifiers & modifiers::ABSTRACT != 0 {
                        let symbol =
                            self.reference_symbol_of_type(ty, consumer)?
                                .ok_or_else(|| {
                                    super::super::super::internal("class type has no native symbol")
                                })?;
                        let declaration = self.reference_class_declaration(symbol, consumer)?;
                        if declaration
                            .map(|declaration| {
                                self.source.class_declaration_is_abstract(declaration)
                            })
                            .transpose()?
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        for other in sequence.read(self.base_types_completion(ty, consumer))? {
                            if other == base_type {
                                continue;
                            }
                            if let Some(other) =
                                sequence.read(self.property_of_object_type_by_key_completion(
                                    other, &key, consumer,
                                ))?
                            {
                                let other = self.property_symbol(&other, consumer)?;
                                let other = self.property_target_symbol(other)?;
                                if self.reference_symbol_identity(
                                    ReferenceSymbol::from_property(base),
                                    consumer,
                                )? != self.reference_symbol_identity(
                                    ReferenceSymbol::from_property(other),
                                    consumer,
                                )? {
                                    continue 'property;
                                }
                            }
                        }
                        sequence.read(
                            self.check_diagnostic_type_display_completion(base_type, consumer),
                        )?;
                        sequence
                            .read(self.check_diagnostic_type_display_completion(ty, consumer))?;
                        sequence.read(self.diagnostic_symbol_display_completion(
                            ReferenceSymbol::from_property(base_property),
                            consumer,
                        ))?;
                    }
                    continue;
                }
                let derived_modifiers =
                    self.property_declaration_modifiers(derived, false, consumer)?;
                if (base_modifiers | derived_modifiers) & modifiers::PRIVATE != 0 {
                    continue;
                }
                let derived_flags = self.property_symbol_flags(derived)?;
                let base_pa =
                    base_flags.property || base_flags.get_accessor || base_flags.set_accessor;
                let derived_pa = derived_flags.property
                    || derived_flags.get_accessor
                    || derived_flags.set_accessor;
                if base_pa && derived_pa {
                    if self.property_check_flags(base)? & checks::MAPPED != 0 {
                        continue;
                    }
                    let assignment = self
                        .property_reference_value_declaration(derived, consumer)?
                        .map(|declaration| self.reference_declaration_kind(declaration))
                        .transpose()?
                        == Some(K::BinaryExpression);
                    if assignment
                        || self.properties_abstract_or_interface(base, base_modifiers, consumer)?
                    {
                        continue;
                    }
                    let base_property_only =
                        base_flags.property && !base_flags.get_accessor && !base_flags.set_accessor;
                    let derived_property_only = derived_flags.property
                        && !derived_flags.get_accessor
                        && !derived_flags.set_accessor;
                    if base_property_only != derived_property_only {
                        sequence.read(self.diagnostic_symbol_display_completion(
                            ReferenceSymbol::from_property(base),
                            consumer,
                        ))?;
                        sequence.read(
                            self.check_diagnostic_type_display_completion(base_type, consumer),
                        )?;
                        sequence
                            .read(self.check_diagnostic_type_display_completion(ty, consumer))?;
                    } else if self.inputs.semantic_options().use_define_for_class_fields {
                        let declarations =
                            self.property_reference_declarations(derived, consumer)?;
                        let mut uninitialized = None;
                        for &declaration in &declarations {
                            if self.reference_declaration_kind(declaration)?
                                == K::PropertyDeclaration
                            {
                                let super::super::ReferenceDeclaration::Captured(declaration) =
                                    declaration
                                else {
                                    return Err(super::super::super::internal(
                                        "JSON grammar produced a class field",
                                    )
                                    .into());
                                };
                                if self
                                    .source
                                    .pseudo_child(declaration, "initializer")?
                                    .is_none()
                                {
                                    uninitialized = Some(declaration);
                                    break;
                                }
                            }
                        }
                        if let Some(uninitialized) = uninitialized
                            && !self.property_is_transient(derived)?
                            && (base_modifiers | derived_modifiers) & modifiers::ABSTRACT == 0
                        {
                            let mut ambient = false;
                            for &declaration in &declarations {
                                if self.reference_declaration_is_ambient(declaration)? {
                                    ambient = true;
                                    break;
                                }
                            }
                            if ambient {
                                continue;
                            }
                            let symbol =
                                self.reference_symbol_of_type(ty, consumer)?
                                    .ok_or_else(|| {
                                        super::super::super::internal(
                                            "class type has no native symbol",
                                        )
                                    })?;
                            let class = self.reference_class_declaration(symbol, consumer)?;
                            let constructor = if let Some(class) = class {
                                let root = self.source.pseudo_node_expression(class)?;
                                self.source.class_check_recipe(root)?.constructor
                            } else {
                                None
                            };
                            let name = self
                                .source
                                .pseudo_child(uninitialized, "name")?
                                .ok_or_else(|| {
                                    super::super::super::internal(
                                        "uninitialized property has no name",
                                    )
                                })?;
                            let definite = self
                                .source
                                .pseudo_child(uninitialized, "exclamationToken")?
                                .is_some();
                            let overwrite = match constructor {
                                Some(constructor)
                                    if !definite
                                        && self.source.declaration_kind(name)? == K::Identifier
                                        && self.inputs.semantic_options().strict_null_checks =>
                                {
                                    let name = self.source.pseudo_node_expression(name)?;
                                    !sequence.read(
                                        self.property_initialized_in_constructor_completion(
                                            name,
                                            None,
                                            ty,
                                            constructor,
                                            consumer,
                                        ),
                                    )?
                                }
                                _ => true,
                            };
                            if overwrite {
                                sequence.read(self.diagnostic_symbol_display_completion(
                                    ReferenceSymbol::from_property(base),
                                    consumer,
                                ))?;
                                sequence.read(self.check_diagnostic_type_display_completion(
                                    base_type, consumer,
                                ))?;
                            }
                        }
                    }
                    continue;
                }
                if self.is_prototype_property(base)?
                    && (self.is_prototype_property(derived)? || derived_flags.property)
                {
                    continue;
                }
                sequence
                    .read(self.check_diagnostic_type_display_completion(base_type, consumer))?;
                sequence.read(self.diagnostic_symbol_display_completion(
                    ReferenceSymbol::from_property(base),
                    consumer,
                ))?;
                sequence.read(self.check_diagnostic_type_display_completion(ty, consumer))?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkMembersForOverrideModifier` reacquires bases and always asks the
    /// base constructor, then visits nonambient members/parameter properties.
    fn check_members_for_override_modifier_completion(
        &mut self,
        recipe: &ClassCheckRecipe,
        ty: TypeId,
        with_this: TypeId,
        static_type: TypeId,
        extends: bool,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let base = if extends {
                let bases = sequence.read(self.base_types_completion(ty, consumer))?;
                match bases.first().copied() {
                    Some(base) => {
                        Some(sequence.read(self.get_type_with_this_argument_completion(
                            base,
                            self.model.target_this_parameter(ty)?,
                            false,
                            consumer,
                        ))?)
                    }
                    None => None,
                }
            } else {
                None
            };
            let static_base = sequence.read(self.base_constructor_type_completion(ty, consumer))?;
            for &member in &recipe.members {
                let declaration = self.source.type_node_declaration(member)?;
                if self
                    .source
                    .declaration_has_modifier(declaration, K::DeclareKeyword)?
                {
                    continue;
                }
                let members = if self.source.declaration_kind(declaration)? == K::Constructor {
                    self.source.constructor_parameter_properties(declaration)?
                } else {
                    vec![declaration]
                };
                for declaration in members {
                    sequence.read(self.check_member_override_modifier_completion(
                        declaration,
                        ty,
                        with_this,
                        static_type,
                        base,
                        static_base,
                        recipe.ambient,
                        consumer,
                    ))?;
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }

    /// `checkMemberForOverrideModifier`: dynamic names are rejected before
    /// completed symbol acquisition; noImplicitOverride is a separate gate.
    #[expect(
        clippy::too_many_arguments,
        reason = "ports checkMemberForOverrideModifier's parameters plus the request consumer"
    )]
    fn check_member_override_modifier_completion(
        &mut self,
        member: DeclarationIdentity,
        ty: TypeId,
        with_this: TypeId,
        static_type: TypeId,
        base: Option<TypeId>,
        static_base: TypeId,
        ambient: bool,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let override_ = self
                .source
                .declaration_has_modifier(member, K::OverrideKeyword)?;
            let Some(base) = base else {
                if override_ {
                    sequence.read(self.check_diagnostic_type_display_completion(ty, consumer))?;
                }
                return Ok(());
            };
            if override_
                && let Some(raw) = self.reference_raw_symbol_of_declaration(member)?
                && self
                    .reference_symbol_value_declaration(raw, consumer)?
                    .is_some()
                && syntax::is_class_element(self.source.declaration_kind(member)?)
                && self.source.declaration_name_binding(member)?.name.is_some()
                && !sequence.read(self.has_bindable_declaration_name_completion(member))?
            {
                return Ok(());
            }
            if !override_ && !self.inputs.semantic_options().no_implicit_override {
                return Ok(());
            }
            let Some(symbol) =
                sequence.read(self.reference_node_declaration_symbol_completion(member))?
            else {
                return Ok(());
            };
            let static_ = self.source.declaration_is_static(member)?;
            let key = self.reference_symbol_key(symbol, consumer)?;
            if sequence
                .read(self.property_of_type_by_key_completion(
                    if static_ { static_type } else { with_this },
                    &key,
                    consumer,
                ))?
                .is_none()
            {
                return Ok(());
            }
            let base_type = if static_ { static_base } else { base };
            let base_property = sequence
                .read(self.property_of_type_by_key_completion(base_type, &key, consumer))?;
            if base_property.is_none() && override_ {
                let name = self.reference_symbol_display_name_bytes(symbol, consumer)?;
                let name = String::from_utf8(name).map_err(|_| {
                    super::super::super::unsupported_demand_at_location(
                        self.inputs,
                        consumer,
                        crate::analysis::SemanticFailureReason::UnrepresentableIdentity,
                        "override suggestion name contains non-scalar native text",
                    )
                })?;
                let properties =
                    sequence.read(self.properties_of_type_completion(base_type, consumer))?;
                let symbols = properties
                    .iter()
                    .map(|property| {
                        self.property_symbol(property, consumer)
                            .map(ReferenceSymbol::from_property)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let suggestion = sequence.read(self.spelling_suggestion_for_symbols_completion(
                    &name,
                    symbols,
                    SuggestionMeaning::ClassMember,
                    consumer,
                ))?;
                sequence.read(self.check_diagnostic_type_display_completion(base, consumer))?;
                if let Some(suggestion) = suggestion {
                    sequence
                        .read(self.diagnostic_symbol_display_completion(suggestion, consumer))?;
                }
            } else if let Some(base_property) = base_property
                && !override_
                && self.inputs.semantic_options().no_implicit_override
                && !ambient
            {
                let property = self.property_symbol(&base_property, consumer)?;
                let declarations = self.property_reference_declarations(property, consumer)?;
                if !declarations.is_empty() {
                    let mut abstract_ = false;
                    for declaration in declarations {
                        if self
                            .reference_declaration_has_modifier(declaration, K::AbstractKeyword)?
                        {
                            abstract_ = true;
                            break;
                        }
                    }
                    if !abstract_
                        || self
                            .source
                            .declaration_has_modifier(member, K::AbstractKeyword)?
                    {
                        sequence
                            .read(self.check_diagnostic_type_display_completion(base, consumer))?;
                    }
                }
            }
            Ok(())
        })();
        sequence.finish(result)
    }
}
