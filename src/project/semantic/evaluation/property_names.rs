//! Property names and lookups consume established checker questions.

use super::super::model::{
    DemandError, Index, NativeCompletion, NativeResult, NativeSequence, Property, PropertySymbolId,
    SpecialName, TypeFlags, TypeId, TypeView,
};
use super::super::source::PropertyNameForm;
use super::Evaluator;
use super::relations::RelationKind;
use crate::project::binding::NativePropertyKey;
use crate::project::binding::flow::Site;
use crate::project::sources::FileId;
use crate::references::Location;
use crate::types::PrimitiveName;

impl Evaluator<'_, '_> {
    /// `getEffectivePropertyNameForPropertyNameNode`: fixed syntax names
    /// precede the dynamic expression's native type/name question.
    pub(super) fn effective_property_name_completion(
        &mut self,
        name: super::super::source::ExpressionRoot,
    ) -> NativeResult<Option<NativePropertyKey>> {
        match self.source.effective_property_name_syntax(name)? {
            super::super::source::EffectivePropertyName::Text(text) => Ok(NativeCompletion::clean(
                Some(NativePropertyKey::fixed(text.as_bytes())),
            )),
            super::super::source::EffectivePropertyName::Dynamic(root) => {
                let expression = self.source.expression_at(root.file, root.site)?;
                self.type_of_source_expression_completion(&expression)?
                    .and_then_value(|ty| self.property_key_from_type(ty))
            }
        }
    }
    /// `getPropertyNameForKnownSymbolName` resolves the actual global Symbol
    /// value before considering the reserved fallback name. A declaration may
    /// supply a different usable literal/unique key; callers never guess it.
    pub(super) fn property_key_for_known_symbol_completion(
        &mut self,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<NativePropertyKey> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some(constructor) = self.global_value_symbol("Symbol", consumer)? {
                let ty =
                    sequence.read(self.value_reference_type_completion(constructor, consumer))?;
                if let Some(ty) =
                    sequence.read(self.type_of_property_of_type_completion(ty, name, consumer))?
                {
                    if let Some(key) = self.property_key_from_type(ty)? {
                        return Ok(key);
                    }
                }
            }
            Ok(NativePropertyKey::from_string_value(
                crate::project::binding::NativeStringValue::from_byte_parts([
                    b"\xfe@".as_slice(),
                    name.as_bytes(),
                ]),
            ))
        })();
        sequence.finish(result)
    }

    /// `isSymbolWithComputedName`, and the declaration observation shared by
    /// `isSymbolWithSymbolName`/`isSymbolWithNumericName`. Only the first actual
    /// declaration participates; JSON's retained property name is always a
    /// string literal, established by the JSON grammar owner.
    fn first_index_property_name(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> Result<(Option<super::super::source::ExpressionRoot>, bool), DemandError> {
        let declarations = self.property_reference_declarations(symbol, consumer)?;
        match declarations.first().copied() {
            Some(super::ReferenceDeclaration::Captured(declaration)) => {
                match self.source.declaration_name_site(declaration)? {
                    Some((file, site)) => self
                        .source
                        .index_property_name_syntax(super::super::source::ExpressionRoot {
                            file,
                            site,
                        })
                        .map_err(Into::into),
                    None => Ok((None, false)),
                }
            }
            Some(super::ReferenceDeclaration::Json { .. }) | None => Ok((None, false)),
        }
    }

    /// `isSymbolWithSymbolName`: known-symbol identity precedes the first
    /// declaration's computed-name type and assignability question.
    pub(super) fn is_symbol_with_symbol_name_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let key =
            self.reference_symbol_key(super::ReferenceSymbol::from_property(symbol), consumer)?;
        if key.is_known_symbol() {
            return Ok(NativeCompletion::clean(true));
        }
        if let (Some(root), _) = self.first_index_property_name(symbol, consumer)? {
            let expression = self.source.expression_at(root.file, root.site)?;
            let ty = self.computed_property_name_type_completion(
                &expression,
                false,
                &expression.location,
            )?;
            return ty.and_then(|ty| {
                self.type_assignable_to_kind_completion(ty, TypeFlags::SYMBOL, false, consumer)
            });
        }
        Ok(NativeCompletion::clean(false))
    }

    /// `isSymbolWithNumericName` / `isNumericName` / `isNumericComputedName`:
    /// native Any counts as a numeric computed name through the shared kind
    /// relation, even though it has no literal property key.
    pub(super) fn is_symbol_with_numeric_name_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let key =
            self.reference_symbol_key(super::ReferenceSymbol::from_property(symbol), consumer)?;
        if let Some(bytes) = key.fixed_bytes()
            && let Ok(name) = std::str::from_utf8(bytes)
            && crate::numbers::numeric_literal_name_value(name).is_some()
        {
            return Ok(NativeCompletion::clean(true));
        }
        let (computed, numeric) = self.first_index_property_name(symbol, consumer)?;
        if let Some(root) = computed {
            let expression = self.source.expression_at(root.file, root.site)?;
            let ty = self.computed_property_name_type_completion(
                &expression,
                false,
                &expression.location,
            )?;
            return ty.and_then(|ty| {
                self.type_assignable_to_kind_completion(ty, TypeFlags::NUMBER_LIKE, false, consumer)
            });
        }
        Ok(NativeCompletion::clean(numeric))
    }

    /// `getObjectLiteralIndexInfo`: short-circuit name classification precedes
    /// every raw type read; computed declarations are retained as components
    /// only after that read. The Subtype union is shared with index factories.
    pub(super) fn object_literal_index_info(
        &mut self,
        readonly: bool,
        properties: &[PropertySymbolId],
        key: TypeId,
        consumer: &Location,
    ) -> Result<Index, DemandError> {
        self.object_literal_index_info_completion(readonly, properties, key, consumer)?
            .admit()
    }
    pub(super) fn object_literal_index_info_completion(
        &mut self,
        readonly: bool,
        properties: &[PropertySymbolId],
        key: TypeId,
        consumer: &Location,
    ) -> NativeResult<Index> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut types = Vec::new();
            let mut components = Vec::new();
            let string = self.model.primitive(PrimitiveName::String);
            let number = self.model.primitive(PrimitiveName::Number);
            let symbol = self.model.primitive(PrimitiveName::Symbol);
            for &property in properties {
                if key == string
                    && !sequence
                        .read(self.is_symbol_with_symbol_name_completion(property, consumer))?
                    || key == number
                        && sequence
                            .read(self.is_symbol_with_numeric_name_completion(property, consumer))?
                    || key == symbol
                        && sequence
                            .read(self.is_symbol_with_symbol_name_completion(property, consumer))?
                {
                    types.push(
                        sequence.read(self.property_raw_type_completion(property, consumer))?,
                    );
                    if self
                        .first_index_property_name(property, consumer)?
                        .0
                        .is_some()
                        && let Some(super::ReferenceDeclaration::Captured(declaration)) = self
                            .property_reference_declarations(property, consumer)?
                            .first()
                    {
                        components.push(*declaration);
                    }
                }
            }
            let value = if types.is_empty() {
                self.model.special(SpecialName::Undefined)
            } else {
                sequence.read(self.union_type_reduced_completion(
                    types,
                    super::super::model::UnionReduction::Subtype,
                    None,
                    None,
                    consumer,
                ))?
            };
            Ok(self.model.index_info(
                key,
                value,
                readonly,
                None,
                (!components.is_empty()).then(|| components.into()),
            ))
        })();
        sequence.finish(result)
    }

    /// `hasLateBindableIndexSignature` / `isLateBindableIndexSignature`.
    /// This asks assignability to the index-key union, a different question
    /// from the literal-only `isLateBindableName` gate.
    pub(super) fn has_late_bindable_index_signature(
        &mut self,
        declaration: super::super::source::DeclarationIdentity,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.has_late_bindable_index_signature_completion(declaration, consumer)?
            .admit()
    }

    pub(super) fn has_late_bindable_index_signature_completion(
        &mut self,
        declaration: super::super::source::DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let Some(name) = self.source.pseudo_child(declaration, "name")? else {
            return Ok(super::super::model::NativeCompletion::clean(false));
        };
        let Some((expression, computed)) = self.source.late_bindable_name_expression(name)? else {
            return Ok(super::super::model::NativeCompletion::clean(false));
        };
        let ty = if computed {
            let root = self.source.pseudo_node_expression(name)?;
            self.check_computed_property_name_completion(name.file, root.site)?
        } else {
            self.check_expression_cached_completion(expression.file, expression.site)?
        };
        ty.and_then(|ty| {
            self.type_related_completion(
                ty,
                self.model.string_number_symbol_type(),
                RelationKind::Assignable,
                consumer,
            )
        })
    }
    /// `hasBindableName` / `isLateBindableName`: syntax-only early names
    /// return immediately, while an eligible entity-name expression consumes
    /// the shared computed-name cache before checking property-name flags.
    pub(super) fn has_bindable_declaration_name(
        &mut self,
        declaration: super::super::source::DeclarationIdentity,
    ) -> Result<bool, DemandError> {
        self.has_bindable_declaration_name_completion(declaration)?
            .admit()
    }

    pub(super) fn has_bindable_declaration_name_completion(
        &mut self,
        declaration: super::super::source::DeclarationIdentity,
    ) -> NativeResult<bool> {
        let facts = self.source.declaration_name_binding(declaration)?;
        if !facts.dynamic {
            return Ok(super::super::model::NativeCompletion::clean(true));
        }
        if !facts.late_bindable {
            return Ok(super::super::model::NativeCompletion::clean(false));
        }
        let name = self
            .source
            .pseudo_child(declaration, "name")?
            .ok_or_else(|| super::super::internal("late-bindable declaration has no name"))?;
        self.is_late_bindable_name_completion(name)
    }

    /// `isLateBindableName` owns the syntax gate, distinct computed/cached
    /// expression questions, and exact usable-property-name flags.
    pub(super) fn is_late_bindable_name_completion(
        &mut self,
        name: super::super::source::DeclarationIdentity,
    ) -> NativeResult<bool> {
        let Some((expression, computed)) = self.source.late_bindable_name_expression(name)? else {
            return Ok(super::super::model::NativeCompletion::clean(false));
        };
        let ty = if computed {
            let name_root = self.source.pseudo_node_expression(name)?;
            let recipe = self
                .source
                .property_name_recipe(name.file, name_root.site)?;
            let PropertyNameForm::Expression {
                expression,
                forbidden_in,
                ..
            } = recipe.form
            else {
                return Err(
                    super::super::internal("late computed name has no expression recipe").into(),
                );
            };
            self.computed_property_name_type_completion(
                &expression,
                forbidden_in,
                &recipe.location,
            )?
        } else {
            self.check_expression_cached_completion(expression.file, expression.site)?
        };
        Ok(ty.map(|ty| {
            self.model.has_type_flags(
                ty,
                TypeFlags::STRING_LITERAL | TypeFlags::NUMBER_LITERAL | TypeFlags::UNIQUE_SYMBOL,
            )
        }))
    }
    /// `getLiteralTypeFromProperty` observes native symbol links and the actual
    /// ValueDeclaration name. No displayed graph key can supply this identity.
    pub(super) fn literal_type_from_property(
        &mut self,
        symbol: PropertySymbolId,
        include: TypeFlags,
        include_non_public: bool,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.literal_type_from_property_completion(symbol, include, include_non_public, consumer)?
            .admit()
    }

    pub(super) fn literal_type_from_property_completion(
        &mut self,
        symbol: PropertySymbolId,
        include: TypeFlags,
        include_non_public: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if !include_non_public
                && self.property_declaration_modifiers(symbol, false, consumer)?
                    & (crate::syntax::modifiers::PRIVATE | crate::syntax::modifiers::PROTECTED)
                    != 0
            {
                return Ok(self.model.special(SpecialName::Never));
            }
            let facts = self.property_symbol_name_facts(symbol, consumer)?;
            let ty = if let Some(ty) = facts.name_type {
                Some(ty)
            } else if facts.key.fixed_bytes() == Some(b"default".as_slice()) {
                Some(self.model.literal(PrimitiveName::String, "default".into()))
            } else {
                let name = match self.property_reference_value_declaration(symbol, consumer)? {
                    Some(declaration) => sequence
                        .read(self.reference_declaration_literal_name_completion(declaration))?,
                    None => None,
                };
                if let Some(name) = name {
                    Some(name)
                } else if !facts.known_symbol {
                    Some(self.model.string_literal_for_key(&facts.key))
                } else {
                    None
                }
            };
            Ok(match ty {
                Some(ty) if self.model.has_type_flags(ty, include) => ty,
                _ => self.model.special(SpecialName::Never),
            })
        })();
        sequence.finish(result)
    }

    /// `getLiteralTypeFromPropertyName`: numeric and computed expressions
    /// keep their different checking/cache operations before regularization.
    pub(super) fn literal_type_of_property_name(
        &mut self,
        file: FileId,
        name: Site,
    ) -> Result<TypeId, DemandError> {
        self.literal_type_of_property_name_completion(file, name)?
            .admit()
    }

    pub(super) fn literal_type_of_property_name_completion(
        &mut self,
        file: FileId,
        name: Site,
    ) -> super::super::model::NativeResult<TypeId> {
        use super::super::model::NativeCompletion;
        let recipe = self.source.property_name_recipe(file, name)?;
        let ty = match recipe.form {
            PropertyNameForm::String(value) => {
                NativeCompletion::clean(self.model.literal(PrimitiveName::String, value))
            }
            PropertyNameForm::Expression {
                expression,
                computed,
                forbidden_in,
            } => {
                if computed {
                    self.computed_property_name_type_completion(
                        &expression,
                        forbidden_in,
                        &recipe.location,
                    )?
                } else {
                    self.check_expression_completion(
                        &expression,
                        super::contextual::CheckMode::NORMAL,
                    )?
                }
            }
            PropertyNameForm::Never => {
                NativeCompletion::clean(self.model.special(SpecialName::Never))
            }
        };
        Ok(ty.map(|ty| self.model.regular_literal(ty)))
    }

    /// `checkComputedPropertyName` for source-check callers starts from the
    /// same captured name recipe as the literal-name operation, without its
    /// separate regularization step.
    pub(super) fn check_computed_property_name_completion(
        &mut self,
        file: FileId,
        name: Site,
    ) -> NativeResult<TypeId> {
        let recipe = self.source.property_name_recipe(file, name)?;
        let PropertyNameForm::Expression {
            expression,
            computed: true,
            forbidden_in,
        } = recipe.form
        else {
            return Err(super::super::internal("computed-name check has another name kind").into());
        };
        self.computed_property_name_type_completion(&expression, forbidden_in, &recipe.location)
    }

    /// `isTypeUsableAsPropertyName` / `getPropertyNameFromType`: only actual
    /// string/number literals or native unique-symbol identity supplies a key.
    pub(super) fn property_name_from_type(
        &self,
        ty: TypeId,
    ) -> Result<Option<String>, DemandError> {
        self.property_native_name_from_type(ty)?
            .map(|name| {
                String::from_utf8(name).map_err(|_| {
                    super::super::internal(
                        "native generated property key requires its exact byte consumer",
                    )
                    .into()
                })
            })
            .transpose()
    }

    /// `getPropertyNameFromType` keeps the generated identity in its native
    /// key domain. Only consumers specifically requiring fixed bytes use this
    /// projection; property-table decisions use property_key_from_type.
    pub(super) fn property_native_name_from_type(
        &self,
        ty: TypeId,
    ) -> Result<Option<Vec<u8>>, DemandError> {
        self.property_key_from_type(ty)?
            .map(|key| {
                key.fixed_bytes().map(<[u8]>::to_vec).ok_or_else(|| {
                    super::super::internal(
                        "generated property key requires its structured consumer",
                    )
                    .into()
                })
            })
            .transpose()
    }

    /// `isTypeUsableAsPropertyName` / `getPropertyNameFromType` is the sole
    /// type-to-table-key decision; generated names retain actual symbol identity.
    pub(super) fn property_key_from_type(
        &self,
        ty: TypeId,
    ) -> Result<Option<NativePropertyKey>, DemandError> {
        Ok(match self.model.get(ty)? {
            TypeView::UniqueSymbol(key) => Some(key),
            TypeView::Literal {
                base: PrimitiveName::String | PrimitiveName::Number,
                value,
            } => Some(value.to_property_key()),
            _ => None,
        })
    }

    /// `getPropertyOfTypeEx` with the ordinary augmentation policy. The
    /// established inventory owns property identity; checking that a property
    /// exists deliberately does not demand its value type.
    pub(super) fn property_of_type(
        &mut self,
        ty: TypeId,
        name: &str,
        consumer: &Location,
    ) -> Result<Option<Property>, DemandError> {
        self.property_of_type_completion(ty, name, consumer)?
            .admit()
    }

    pub(super) fn property_of_type_completion(
        &mut self,
        ty: TypeId,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        self.property_of_type_by_name_completion(ty, name.as_bytes(), consumer)
    }

    /// Native property lookup retains byte-domain names, including internal
    /// and generated names, until an operation actually requires literal text.
    pub(super) fn property_of_type_by_name_completion(
        &mut self,
        ty: TypeId,
        name: &[u8],
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        self.property_of_type_by_name_ex_completion(ty, name, false, consumer)
    }

    /// `getPropertyOfTypeEx` keeps object augmentation separate from the
    /// intersection's mandatory first lookup without augmentation. Composite
    /// caches retain both policies, and only normal lookup hides ReadPartial.
    pub(super) fn property_of_type_ex_completion(
        &mut self,
        ty: TypeId,
        name: &str,
        skip_augmentation: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        self.property_of_type_by_name_ex_completion(
            ty,
            name.as_bytes(),
            skip_augmentation,
            consumer,
        )
    }

    pub(super) fn property_of_type_by_name_ex_completion(
        &mut self,
        ty: TypeId,
        name: &[u8],
        skip_augmentation: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        self.property_of_type_by_key_ex_completion(
            ty,
            &NativePropertyKey::fixed(name),
            skip_augmentation,
            consumer,
        )
    }

    pub(super) fn property_of_type_by_key(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<Option<Property>, DemandError> {
        self.property_of_type_by_key_completion(ty, name, consumer)?
            .admit()
    }

    pub(super) fn property_of_type_by_key_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        self.property_of_type_by_key_ex_completion(ty, name, false, consumer)
    }

    pub(super) fn property_of_type_by_key_ex_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        skip_augmentation: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        self.property_of_type_by_key_flags_completion(ty, name, skip_augmentation, false, consumer)
    }

    /// Complete `getPropertyOfTypeEx` policy; only QualifiedName callers admit
    /// values reached through a type-only alias or export-star view.
    pub(super) fn property_of_type_by_key_flags_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        skip_augmentation: bool,
        include_type_only: bool,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Property>, DemandError> {
            let ty = sequence.read(self.reduced_apparent_type_completion(ty, consumer))?;
            if self.model.has_type_flags(ty, TypeFlags::INTERSECTION) {
                if let Some(property) = sequence.read(
                    self.property_of_composite_type_by_key_completion(ty, name, true, consumer),
                )? {
                    return Ok(Some(property));
                }
                return if skip_augmentation {
                    Ok(None)
                } else {
                    sequence.read(
                        self.property_of_composite_type_by_key_completion(
                            ty, name, false, consumer,
                        ),
                    )
                };
            }
            if self.model.has_type_flags(ty, TypeFlags::UNION) {
                return sequence.read(self.property_of_composite_type_by_key_completion(
                    ty,
                    name,
                    skip_augmentation,
                    consumer,
                ));
            }
            if !self.model.has_type_flags(ty, TypeFlags::OBJECT) {
                return Ok(None);
            }
            let members = sequence
                .read(self.structured_type_members_completion(ty, consumer))?
                .ok_or_else(|| {
                    super::super::internal("native object lacks a structured inventory")
                })?;
            match sequence.read(self.property_in_raw_members_completion(
                ty,
                &members,
                name,
                !include_type_only,
                include_type_only,
                consumer,
            ))? {
                super::structure::RawPropertyLookup::Found(property) => return Ok(Some(property)),
                super::structure::RawPropertyLookup::Hidden => return Ok(None),
                super::structure::RawPropertyLookup::Absent => {}
            }
            if skip_augmentation {
                return Ok(None);
            }
            // The actual anyFunctionType sentinel has its own Function branch
            // when that factory is admitted. Ordinary callable objects ask the
            // call inventory before the construct inventory, then Object.
            let function = if ty == self.model.any_function_type() {
                Some(sequence.read(self.global_object_type_completion("Function", consumer))?)
            } else if !members.call_signatures.is_empty() {
                Some(sequence.read(self.global_strict_function_type_completion(false, consumer))?)
            } else if !members.construct_signatures.is_empty() {
                Some(sequence.read(self.global_strict_function_type_completion(true, consumer))?)
            } else {
                None
            };
            if let Some(function) = function {
                if let Some(property) = sequence.read(
                    self.property_of_object_type_by_key_completion(function, name, consumer),
                )? {
                    return Ok(Some(property));
                }
            }
            let object = sequence.read(self.global_object_type_completion("Object", consumer))?;
            sequence.read(self.property_of_object_type_by_key_completion(object, name, consumer))
        })();
        sequence.finish(result)
    }

    pub(super) fn property_of_object_type_by_key(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<Option<Property>, DemandError> {
        self.property_of_object_type_by_key_completion(ty, name, consumer)?
            .admit()
    }

    pub(super) fn property_of_object_type_by_key_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Property>, DemandError> {
            if !self.model.has_type_flags(ty, TypeFlags::OBJECT) {
                return Ok(None);
            }
            let members = sequence
                .read(self.structured_type_members_completion(ty, consumer))?
                .ok_or_else(|| {
                    super::super::internal("object type lacks a structured inventory")
                })?;
            Ok(
                match sequence.read(self.property_in_raw_members_completion(
                    ty, &members, name, false, false, consumer,
                ))? {
                    super::structure::RawPropertyLookup::Found(property) => Some(property),
                    super::structure::RawPropertyLookup::Absent => None,
                    super::structure::RawPropertyLookup::Hidden => {
                        unreachable!("narrow object lookup does not filter export-star views")
                    }
                },
            )
        })();
        sequence.finish(result)
    }

    /// `getTypeOfPropertyOfType`: symbol lookup precedes the actual read.
    pub(super) fn type_of_property_of_type_completion(
        &mut self,
        ty: TypeId,
        name: &str,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            match sequence.read(self.property_of_type_completion(ty, name, consumer))? {
                Some(property) => sequence
                    .read(self.demand_completion(property.read_type))
                    .map(Some),
                None => Ok(None),
            }
        })();
        sequence.finish(result)
    }

    pub(super) fn type_of_property_of_type_by_key(
        &mut self,
        ty: TypeId,
        key: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.type_of_property_of_type_by_key_completion(ty, key, consumer)?
            .admit()
    }

    pub(super) fn type_of_property_of_type_by_key_completion(
        &mut self,
        ty: TypeId,
        key: &NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            match sequence.read(self.property_of_type_by_key_completion(ty, key, consumer))? {
                Some(property) => sequence
                    .read(self.demand_completion(property.read_type))
                    .map(Some),
                None => Ok(None),
            }
        })();
        sequence.finish(result)
    }

    /// `getTypeOfPropertyOrIndexSignatureOfType` makes index fallback optional
    /// independently of noUncheckedIndexedAccess and only after property miss.
    pub(super) fn type_of_property_or_index_signature_by_key_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            if let Some(property) =
                sequence.read(self.property_of_type_by_key_completion(ty, name, consumer))?
            {
                return sequence
                    .read(self.demand_completion(property.read_type))
                    .map(Some);
            }
            match sequence
                .read(self.applicable_index_info_for_key_completion(ty, name, consumer))?
            {
                Some(index) => {
                    let ty = sequence.read(self.demand_completion(index.value_type))?;
                    sequence
                        .read(self.add_optionality_completion(ty, true, true, consumer))
                        .map(Some)
                }
                None => Ok(None),
            }
        })();
        sequence.finish(result)
    }

    pub(super) fn applicable_index_info_for_key(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<Option<Index>, DemandError> {
        self.applicable_index_info_for_key_completion(ty, name, consumer)?
            .admit()
    }

    /// `getApplicableIndexInfoForName` asks ESSymbol for a late-bound key,
    /// and the exact string literal for every other native name.
    pub(super) fn applicable_index_info_for_key_completion(
        &mut self,
        ty: TypeId,
        name: &NativePropertyKey,
        consumer: &Location,
    ) -> NativeResult<Option<Index>> {
        let mut sequence = NativeSequence::new();
        let result = {
            let key = if name.is_known_symbol() {
                self.model.primitive(PrimitiveName::Symbol)
            } else {
                self.model.string_literal_for_key(name)
            };
            sequence.read(self.applicable_index_info_completion(ty, key, consumer))
        };
        sequence.finish(result)
    }

    /// `getApplicableIndexInfo` / `findApplicableIndexInfo`: retain a string
    /// fallback while testing every other index in native inventory order.
    /// Multiple matches combine their values and readonly flags; no consumer
    /// needs to reconstruct this ordering or distinguish the synthetic result.
    pub(super) fn applicable_index_info(
        &mut self,
        ty: TypeId,
        key: TypeId,
        consumer: &Location,
    ) -> Result<Option<Index>, DemandError> {
        self.applicable_index_info_completion(ty, key, consumer)?
            .admit()
    }

    pub(super) fn applicable_index_info_completion(
        &mut self,
        ty: TypeId,
        key: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<Index>> {
        self.index_infos_of_type_completion(ty, consumer)?
            .and_then(|indices| self.find_applicable_index_info_completion(&indices, key, consumer))
    }

    /// `findApplicableIndexInfo` consumes its established ordered inventory;
    /// callers choose their native apparent or structured inventory question.
    pub(super) fn find_applicable_index_info_completion(
        &mut self,
        indices: &[Index],
        key: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<Index>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let string = self.model.primitive(PrimitiveName::String);
            let mut string_index = None;
            let mut applicable = Vec::new();
            for index in indices.iter() {
                let index_key = sequence.read(self.demand_completion(index.key_type))?;
                if index_key == string {
                    string_index = Some(index);
                } else if sequence
                    .read(self.is_applicable_index_type_completion(key, index_key, consumer))?
                {
                    applicable.push(index);
                }
            }
            match applicable.len() {
                0 => match string_index {
                    Some(index)
                        if sequence.read(
                            self.is_applicable_index_type_completion(key, string, consumer),
                        )? =>
                    {
                        Ok(Some(index.clone()))
                    }
                    _ => Ok(None),
                },
                1 => Ok(Some(applicable[0].clone())),
                _ => {
                    let mut values = Vec::with_capacity(applicable.len());
                    let mut readonly = true;
                    for index in applicable {
                        values.push(sequence.read(self.demand_completion(index.value_type))?);
                        readonly &= index.readonly;
                    }
                    let value =
                        sequence.read(self.intersection_type_completion(values, consumer))?;
                    let key = self.model.special(SpecialName::Unknown);
                    Ok(Some(
                        self.model.index_info(key, value, readonly, None, None),
                    ))
                }
            }
        })();
        sequence.finish(result)
    }

    /// `isValidIndexKeyType`: pattern literals and nongeneric intersections
    /// with a valid constituent share the primitive index-key admission rule.
    pub(super) fn is_valid_index_key_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_valid_index_key_type_completion(ty, consumer)?
            .admit()
    }

    pub(super) fn is_valid_index_key_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if self.model.has_type_flags(
                ty,
                TypeFlags::STRING | TypeFlags::NUMBER | TypeFlags::SYMBOL,
            ) || self.is_pattern_literal_type(ty)?
            {
                return Ok(true);
            }
            if let Some(types) = self.model.intersection_constituents(ty) {
                if !sequence.read(self.is_generic_type_completion(ty, consumer))? {
                    for &ty in types.iter() {
                        if sequence.read(self.is_valid_index_key_type_completion(ty, consumer))? {
                            return Ok(true);
                        }
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isApplicableIndexType`: assignability precedes the extra string-index
    /// number case and the number-index canonical numeric string case.
    pub(super) fn is_applicable_index_type(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_applicable_index_type_completion(source, target, consumer)?
            .admit()
    }

    pub(super) fn is_applicable_index_type_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if sequence.read(self.type_related_completion(
                source,
                target,
                RelationKind::Assignable,
                consumer,
            ))? {
                return Ok(true);
            }
            let number = self.model.primitive(PrimitiveName::Number);
            let string = self.model.primitive(PrimitiveName::String);
            if target == string
                && sequence.read(self.type_related_completion(
                    source,
                    number,
                    RelationKind::Assignable,
                    consumer,
                ))?
            {
                return Ok(true);
            }
            if target == number {
                if source == self.model.numeric_string_type() {
                    return Ok(true);
                }
                if let TypeView::Literal {
                    base: PrimitiveName::String,
                    value,
                } = self.model.get(source)?
                {
                    return self.literal_is_numeric_name(&value, consumer);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `getIndexInfoOfType` / `findIndexInfo` compares the established exact
    /// key identity. This does not ask the broader applicability relation.
    pub(super) fn index_info_of_type(
        &mut self,
        ty: TypeId,
        key: TypeId,
        consumer: &Location,
    ) -> Result<Option<Index>, DemandError> {
        self.index_info_of_type_completion(ty, key, consumer)?
            .admit()
    }

    pub(super) fn index_info_of_type_completion(
        &mut self,
        ty: TypeId,
        key: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<Index>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<Index>, DemandError> {
            let indices = sequence.read(self.index_infos_of_type_completion(ty, consumer))?;
            for index in indices.iter() {
                if sequence.read(self.demand_completion(index.key_type))? == key {
                    return Ok(Some(index.clone()));
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    /// `getIndexTypeOfType` consumes only the value of the selected index.
    pub(super) fn index_type_of_type(
        &mut self,
        ty: TypeId,
        key: TypeId,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.index_type_of_type_completion(ty, key, consumer)?
            .admit()
    }

    pub(super) fn index_type_of_type_completion(
        &mut self,
        ty: TypeId,
        key: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            match sequence.read(self.index_info_of_type_completion(ty, key, consumer))? {
                Some(index) => sequence
                    .read(self.demand_completion(index.value_type))
                    .map(Some),
                None => Ok(None),
            }
        })();
        sequence.finish(result)
    }

    /// `includeUndefinedInIndexSignature` uses missingType independently of
    /// exactOptionalPropertyTypes. Every index/destructuring consumer shares
    /// this option policy and the native union factory.
    pub(super) fn include_undefined_in_index_signature_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if !self.inputs.semantic_options().no_unchecked_indexed_access {
            return Ok(super::super::model::NativeCompletion::clean(ty));
        }
        let missing = self.model.missing_type();
        self.union_type_ex_completion(vec![ty, missing], None, None, consumer)
    }

    /// `isTupleLikeType` recognizes actual tuples before property lookup. A
    /// present '0' does not consume its read; the length read occurs only after
    /// the separate array-like question succeeds.
    pub(super) fn is_tuple_like_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.is_tuple_like_type_completion(ty, consumer)?.admit()
    }

    pub(super) fn is_tuple_like_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if self.model.is_tuple_type(ty)
                || sequence
                    .read(self.property_of_type_completion(ty, "0", consumer))?
                    .is_some()
            {
                return Ok(true);
            }
            if sequence.read(self.is_array_like_type_completion(ty, consumer))? {
                if let Some(length) = sequence
                    .read(self.type_of_property_of_type_completion(ty, "length", consumer))?
                {
                    let subjects = self.model.union_constituents(length);
                    return Ok(subjects
                        .as_deref()
                        .unwrap_or(&[length])
                        .iter()
                        .all(|&ty| self.model.has_type_flags(ty, TypeFlags::NUMBER_LITERAL)));
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `getTupleElementType`: ordinary numeric property lookup precedes the
    /// all-tuples tail operation. Its unchecked-index marker is undefinedType,
    /// distinct from includeUndefinedInIndexSignature's missingType.
    pub(super) fn tuple_element_type_completion(
        &mut self,
        ty: TypeId,
        index: usize,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        self.type_of_property_of_type_completion(ty, &index.to_string(), consumer)?
            .and_then(|property| {
                if property.is_some() {
                    return Ok(NativeCompletion::clean(property));
                }
                let types = self.model.union_constituents(ty);
                if types
                    .as_deref()
                    .unwrap_or(&[ty])
                    .iter()
                    .all(|&ty| self.model.is_tuple_type(ty))
                {
                    let undefined = self
                        .inputs
                        .semantic_options()
                        .no_unchecked_indexed_access
                        .then(|| self.model.special(SpecialName::Undefined));
                    return Ok(self
                        .tuple_element_type_out_of_start_count_completion(
                            ty,
                            index as f64,
                            undefined,
                            consumer,
                        )?
                        .map(Some));
                }
                Ok(NativeCompletion::clean(None))
            })
    }
}
