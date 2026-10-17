//! One native modifier policy serves checked access and suggestion filtering.
use super::super::super::model::{
    NativeCompletion, NativeResult, NativeSequence, ObjectFlags, PropertySymbolId, TypeFlags,
    TypeId, TypeView,
};
use super::super::super::source::ExpressionRoot;
use super::super::{Evaluator, ReferenceSymbol};
use crate::references::Location;

impl Evaluator<'_, '_> {
    /// `checkPropertyAccessibility` preserves the reporting form; callers
    /// selecting suggestion candidates use the boolean no-error operation.
    pub(in crate::project::semantic::evaluation) fn check_property_accessibility_completion(
        &mut self,
        root: ExpressionRoot,
        is_super: bool,
        writing: bool,
        containing_type: TypeId,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<()> {
        Ok(self
            .property_accessibility_at_location_completion(
                root,
                is_super,
                writing,
                containing_type,
                symbol,
                true,
                consumer,
            )?
            .map(|_| ()))
    }

    /// `isPropertyAccessible` has an Any shortcut and a private-identifier
    /// lexical descendant gate before the ordinary modifier policy.
    pub(in crate::project::semantic::evaluation) fn is_property_accessible_completion(
        &mut self,
        root: ExpressionRoot,
        is_super: bool,
        writing: bool,
        containing_type: TypeId,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if self.model.has_type_flags(containing_type, TypeFlags::ANY) {
            return Ok(NativeCompletion::clean(true));
        }
        if let Some(super::super::ReferenceDeclaration::Captured(declaration)) = self
            .reference_symbol_value_declaration(
                super::super::ReferenceSymbol::from_property(symbol),
                consumer,
            )?
            && self
                .source
                .private_identifier_declaration_name(declaration)?
                .is_some()
        {
            let class = self
                .source
                .containing_class(declaration)?
                .ok_or_else(|| super::super::super::internal("private property has no class"))?;
            let node = self.source.declaration_from_expression_root(root)?;
            return Ok(NativeCompletion::clean(
                !self.source.node_is_optional_chain(node)?
                    && self.source.declaration_is_descendant(node, class)?,
            ));
        }
        self.property_accessibility_at_location_completion(
            root,
            is_super,
            writing,
            containing_type,
            symbol,
            false,
            consumer,
        )
    }

    /// `checkPropertyAccessibilityAtLocation` shares boolean decisions while
    /// guarding diagnostic-only displays with the actual error-node presence.
    #[expect(
        clippy::too_many_arguments,
        reason = "ports checkPropertyAccessibilityAtLocation's parameters plus the request consumer"
    )]
    fn property_accessibility_at_location_completion(
        &mut self,
        root: ExpressionRoot,
        is_super: bool,
        writing: bool,
        containing_type: TypeId,
        symbol: PropertySymbolId,
        report: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        use crate::syntax::modifiers::{ABSTRACT, PRIVATE, PROTECTED, STATIC};
        let mut sequence = super::super::super::model::NativeSequence::new();
        let result = (|| {
            let modifiers = self.property_declaration_modifiers(symbol, writing, consumer)?;
            if is_super {
                if modifiers & ABSTRACT != 0 {
                    if report {
                        sequence.read(self.diagnostic_symbol_display_completion(
                            super::super::ReferenceSymbol::from_property(symbol),
                            consumer,
                        ))?;
                        let class = sequence
                            .read(self.property_declaring_class_completion(symbol, consumer))?
                            .ok_or_else(|| {
                                self.access_pending(
                                    consumer,
                                    "native abstract super access has no declaring class type",
                                )
                            })?;
                        sequence
                            .read(self.check_diagnostic_type_display_completion(class, consumer))?;
                    }
                    return Ok(false);
                }
                if modifiers & STATIC == 0 {
                    for declaration in self.property_reference_declarations(symbol, consumer)? {
                        if let super::super::ReferenceDeclaration::Captured(declaration) =
                            declaration
                            && self.source.class_instance_property(declaration)?
                        {
                            if report {
                                sequence.read(self.diagnostic_symbol_display_completion(
                                    super::super::ReferenceSymbol::from_property(symbol),
                                    consumer,
                                ))?;
                            }
                            return Ok(false);
                        }
                    }
                }
            }
            if modifiers & ABSTRACT != 0
                && sequence.read(self.for_each_property_completion(
                    symbol,
                    consumer,
                    &mut |this, property| {
                        Ok(NativeCompletion::clean(
                            !this.property_symbol_flags(property)?.method,
                        ))
                    },
                ))?
                && self.source.accessibility_this_initializer(root)?
                && let Some(parent) = self.reference_parent_of_symbol(
                    super::super::ReferenceSymbol::from_property(symbol),
                    consumer,
                )?
                && self.reference_symbol_flags(parent, consumer)?.class
                && self.source.node_used_during_class_initialization(root)?
            {
                if report {
                    sequence.read(self.diagnostic_symbol_display_completion(
                        super::super::ReferenceSymbol::from_property(symbol),
                        consumer,
                    ))?;
                    sequence.read(self.diagnostic_symbol_display_completion(parent, consumer))?;
                }
                return Ok(false);
            }
            if modifiers & (PRIVATE | PROTECTED) == 0 {
                return Ok(true);
            }
            let node = self.source.declaration_from_expression_root(root)?;
            if modifiers & PRIVATE != 0 {
                let parent = self
                    .reference_parent_of_symbol(ReferenceSymbol::from_property(symbol), consumer)?;
                let declaring = match parent {
                    Some(parent) => self.reference_class_declaration(parent, consumer)?,
                    None => None,
                };
                let mut containing = self.source.containing_class(node)?;
                while let Some(class) = containing {
                    if Some(class) == declaring {
                        return Ok(true);
                    }
                    containing = self.source.containing_class(class)?;
                }
                if report {
                    sequence.read(self.diagnostic_symbol_display_completion(
                        ReferenceSymbol::from_property(symbol),
                        consumer,
                    ))?;
                    let class = sequence
                        .read(self.property_declaring_class_completion(symbol, consumer))?
                        .ok_or_else(|| {
                            super::super::super::internal(
                                "private property has no declaring class type",
                            )
                        })?;
                    sequence
                        .read(self.check_diagnostic_type_display_completion(class, consumer))?;
                }
                return Ok(false);
            }
            if is_super {
                return Ok(true);
            }
            let mut enclosing = None;
            let mut container = self.source.containing_class(node)?;
            while let Some(declaration) = container {
                let class_symbol = self.reference_symbol_of_declaration(declaration)?;
                let class = sequence
                    .read(self.declared_reference_type_completion(class_symbol, consumer))?;
                if sequence.read(self.class_derived_from_declaring_classes_completion(
                    class, symbol, writing, consumer,
                ))? {
                    enclosing = Some(class);
                    break;
                }
                container = self.source.containing_class(declaration)?;
            }
            if enclosing.is_none() {
                if let Some(class) = sequence
                    .read(self.enclosing_class_from_this_parameter_completion(root, consumer))?
                    && sequence.read(self.class_derived_from_declaring_classes_completion(
                        class, symbol, writing, consumer,
                    ))?
                {
                    enclosing = Some(class);
                }
                if modifiers & STATIC != 0 || enclosing.is_none() {
                    if report {
                        let class = sequence
                            .read(self.property_declaring_class_completion(symbol, consumer))?
                            .unwrap_or(containing_type);
                        sequence.read(self.diagnostic_symbol_display_completion(
                            ReferenceSymbol::from_property(symbol),
                            consumer,
                        ))?;
                        sequence
                            .read(self.check_diagnostic_type_display_completion(class, consumer))?;
                    }
                    return Ok(false);
                }
            }
            if modifiers & STATIC != 0 {
                return Ok(true);
            }
            let containing = match self.model.get(containing_type)? {
                TypeView::TypeParameter(parameter) if parameter.is_this => sequence
                    .read(self.parameter_constraint_completion(containing_type, consumer))?,
                TypeView::TypeParameter(_) => {
                    sequence.read(self.base_constraint_completion(containing_type, consumer))?
                }
                _ => Some(containing_type),
            };
            if let Some(containing) = containing {
                if sequence.read(self.has_base_type_completion(containing, enclosing, consumer))? {
                    return Ok(true);
                }
                if report {
                    sequence.read(self.diagnostic_symbol_display_completion(
                        ReferenceSymbol::from_property(symbol),
                        consumer,
                    ))?;
                    sequence.read(self.check_diagnostic_type_display_completion(
                        enclosing.expect("protected enclosing class was established"),
                        consumer,
                    ))?;
                    sequence.read(
                        self.check_diagnostic_type_display_completion(containing, consumer),
                    )?;
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `isClassDerivedFromDeclaringClasses`: protected constituents are
    /// checked in native order; the first failed base relation stops the walk.
    fn class_derived_from_declaring_classes_completion(
        &mut self,
        class: TypeId,
        property: PropertySymbolId,
        writing: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        Ok(self
            .for_each_property_completion(property, consumer, &mut |this, property| {
                if this.property_declaration_modifiers(property, writing, consumer)?
                    & crate::syntax::modifiers::PROTECTED
                    == 0
                {
                    return Ok(NativeCompletion::clean(false));
                }
                this.property_declaring_class_completion(property, consumer)?
                    .and_then(|base| {
                        Ok(this
                            .has_base_type_completion(class, base, consumer)?
                            .map(|derived| !derived))
                    })
            })?
            .map(|failed| !failed))
    }

    /// `getEnclosingClassFromThisParameter`: an explicit this parameter,
    /// including its constraint, takes precedence over contextual this.
    fn enclosing_class_from_this_parameter_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let Some((function, parameter)) = self.source.this_parameter_function_context(root)? else {
            return Ok(NativeCompletion::clean(None));
        };
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut ty = match parameter {
                Some(parameter) => match self.source.declaration_type_annotation(parameter)? {
                    Some(annotation) => {
                        Some(sequence.read(self.check_type_completion(&annotation))?)
                    }
                    None => None,
                },
                None => None,
            };
            if let Some(explicit) = ty {
                if self
                    .model
                    .has_type_flags(explicit, TypeFlags::TYPE_PARAMETER)
                {
                    ty = sequence.read(self.parameter_constraint_completion(explicit, consumer))?;
                }
            } else {
                ty = sequence
                    .read(self.contextual_this_parameter_type_completion(function, consumer))?;
            }
            match ty {
                Some(ty)
                    if self.model.object_flags(ty)?.intersects(
                        ObjectFlags::CLASS | ObjectFlags::INTERFACE | ObjectFlags::REFERENCE,
                    ) =>
                {
                    Ok(Some(self.model.reference_target(ty).unwrap_or(ty)))
                }
                _ => Ok(None),
            }
        })();
        sequence.finish(result)
    }
}
