//! Native checker questions used by directed reference relationships. These
//! acquire actual symbols/types; the language-service traversal owns direction,
//! callback order, scope, and output grouping.

use super::super::model::{DemandError, TypeId};
use super::super::source::{DeclarationIdentity, ExpressionRoot, ReferenceSite};
use super::SymbolParent;
use super::contextual::ContextFlags;
use super::relations::RelationKind;
use super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use crate::project::binding::ReadOrigin;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use std::collections::HashSet;

impl Evaluator<'_, '_> {
    /// `forEachRelatedSymbol`'s export branch uses the first declaration of
    /// ExportSpecifier kind, then asks the checker's local target operation.
    pub(in crate::project::semantic) fn reference_export_specifier_target(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            if let ReferenceDeclaration::Captured(declaration) = declaration
                && self.source.declaration_kind(declaration)? == SyntaxKind::ExportSpecifier
            {
                return self.reference_export_specifier_local_target(declaration, consumer);
            }
        }
        Ok(None)
    }

    /// The inline-union branch of `skipPastExportOrImportSpecifierOrUnion`.
    /// It takes the first successful property query in declaration order; an
    /// absent property on one union does not suppress a later declaration.
    pub(in crate::project::semantic) fn reference_inline_union_symbol(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let name = self.reference_symbol_key(symbol, consumer)?;
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            let ReferenceDeclaration::Captured(declaration) = declaration else {
                continue;
            };
            if let Some(union) = self.source.reference_inline_union(declaration)? {
                let ty = self.check_type(&union)?;
                if let Some(property) = self
                    .reference_property_symbol_by_key_completion(ty, &name, consumer)?
                    .admit()?
                {
                    return Ok(Some(property));
                }
            }
        }
        Ok(None)
    }

    pub(in crate::project::semantic) fn reference_shorthand_value(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let Some(declaration) = self.source.reference_shorthand_declaration(site)? else {
            return Ok(None);
        };
        self.shorthand_assignment_value_symbol(declaration, &site.selection)
    }

    /// `GetShorthandAssignmentValueSymbol` asks resolveEntityName in Value |
    /// Alias space with ignoreErrors. The raw shorthand property symbol is a
    /// different question; expression checking would demand an unwanted type.
    fn shorthand_assignment_value_symbol(
        &mut self,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        if self.source.declaration_kind(declaration)? != SyntaxKind::ShorthandPropertyAssignment {
            return Ok(None);
        }
        let Some((file, name)) = self.source.declaration_name_site(declaration)? else {
            return Ok(None);
        };
        self.reference_name_completion(
            file,
            name.span,
            crate::project::binding::NativeLookupMeaning::VALUE.with_alias(),
            consumer,
        )?
        .map(|(symbol, _)| symbol)
        .admit()
    }

    pub(in crate::project::semantic) fn reference_contextual_properties(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Vec<ReferenceSymbol>, DemandError> {
        let Some((element, object)) = self.source.reference_object_literal_element(site)? else {
            return Ok(Vec::new());
        };
        let Some(context) = self.contextual_type(object, ContextFlags::NONE)? else {
            return Ok(Vec::new());
        };
        self.property_symbols_from_contextual_type(element, object, context, true, &site.selection)
    }

    /// `GetPropertySymbolsFromContextualType`. Filtering must inspect every
    /// constituent before property acquisition. The union-property fallback
    /// compares with the original union length, before identity deduplication.
    fn property_symbols_from_contextual_type(
        &mut self,
        element: DeclarationIdentity,
        object: ExpressionRoot,
        context: TypeId,
        union_symbol_ok: bool,
        consumer: &Location,
    ) -> Result<Vec<ReferenceSymbol>, DemandError> {
        let Some(name) = self.source.reference_property_text(element)? else {
            return Ok(Vec::new());
        };
        if name.is_empty() {
            return Ok(Vec::new());
        }
        let Some(types) = self.model.union_constituents(context) else {
            return Ok(self
                .reference_property_symbol(context, &name, consumer)?
                .into_iter()
                .collect());
        };
        let mut filtered = Vec::with_capacity(types.len());
        for &ty in types.iter() {
            if !self.type_invalid_due_to_union_discriminant(ty, object, consumer)? {
                filtered.push(ty);
            }
        }
        let mut properties = Vec::new();
        for &ty in &filtered {
            if let Some(property) = self.reference_property_symbol(ty, &name, consumer)? {
                properties.push(property);
            }
        }
        if union_symbol_ok && (properties.is_empty() || properties.len() == types.len()) {
            if let Some(property) = self.reference_property_symbol(context, &name, consumer)? {
                return Ok(vec![property]);
            }
        }
        if filtered.is_empty() && properties.is_empty() {
            for &ty in types.iter() {
                if let Some(property) = self.reference_property_symbol(ty, &name, consumer)? {
                    properties.push(property);
                }
            }
            // The native bad-discriminant retry returns MapNonNil directly.
            return Ok(properties);
        }
        let mut seen = HashSet::new();
        let mut unique = Vec::with_capacity(properties.len());
        for property in properties {
            if seen.insert(self.reference_symbol_identity(property, consumer)?) {
                unique.push(property);
            }
        }
        Ok(unique)
    }

    /// `IsTypeInvalidDueToUnionDiscriminant`: expected literal types consume
    /// GetTypeOfNode on each actual property, including accessors and methods.
    /// Initializer-only checking would change both value types and failures.
    fn type_invalid_due_to_union_discriminant(
        &mut self,
        context: TypeId,
        object: ExpressionRoot,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        for property in self.source.reference_object_properties(object)? {
            let Some((file, name)) = self.source.declaration_name_site(property)? else {
                continue;
            };
            let name_type = if name.kind == SyntaxKind::JsxNamespacedName {
                let name = self
                    .source
                    .reference_property_text(property)?
                    .ok_or_else(|| super::super::internal("JSX property name lost its text"))?;
                self.model
                    .literal(crate::types::PrimitiveName::String, name)
            } else {
                self.literal_type_of_property_name(file, name)?
            };
            let Some(name) = self.property_key_from_type(name_type)? else {
                continue;
            };
            if name.fixed_bytes() == Some(b"".as_slice()) {
                continue;
            }
            let Some(expected) = self.type_of_property_of_type_by_key(context, &name, consumer)?
            else {
                continue;
            };
            if self.model.is_literal_type(expected) {
                let root = self.source.declaration_expression_root(property)?;
                let actual = self.type_of_node(root)?;
                if !self.type_related(actual, expected, RelationKind::Assignable, consumer)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// `GetPropertySymbolOfDestructuringAssignment` retains its independent
    /// assignment-pattern checking question and exact immediate-parent gate.
    pub(in crate::project::semantic) fn reference_destructuring_property(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let Some(pattern) = self.source.reference_destructuring_pattern(site)? else {
            return Ok(None);
        };
        let ty = self.type_of_assignment_pattern(pattern)?;
        self.reference_property_symbol(ty, &site.key, &site.selection)
    }

    pub(in crate::project::semantic) fn reference_binding_element_property(
        &mut self,
        site: &ReferenceSite,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let Some(pattern) = self.source.reference_binding_pattern(site)? else {
            return Ok(None);
        };
        let ty = self.type_of_node(pattern)?;
        self.reference_property_symbol(ty, &site.key, &site.selection)
    }

    /// `getPropertySymbolsFromBaseTypes`: actual Type.symbol identity guards
    /// cycles. Property roots are visited before recursing into that base's
    /// declarations, preserving early callback termination and demand order.
    pub(in crate::project::semantic) fn reference_base_properties(
        &mut self,
        parent: SymbolParent,
        name: &crate::project::binding::NativePropertyKey,
        consumer: &Location,
        callback: &mut impl FnMut(
            &mut Self,
            ReferenceSymbol,
        ) -> Result<Option<ReferenceSymbol>, DemandError>,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let symbol = self.reference_module_parent_symbol(parent);
        enum Work {
            Symbol(ReferenceSymbol),
            Declaration(ReferenceDeclaration),
            Heritage(ExpressionRoot),
        }
        let mut work = vec![Work::Symbol(symbol)];
        let mut seen = HashSet::new();
        while let Some(next) = work.pop() {
            match next {
                Work::Symbol(symbol) => {
                    let flags = self.reference_symbol_flags(symbol, consumer)?;
                    if !(flags.class || flags.interface)
                        || !seen.insert(self.reference_symbol_identity(symbol, consumer)?)
                    {
                        continue;
                    }
                    work.extend(
                        self.reference_symbol_declarations(symbol, consumer)?
                            .into_iter()
                            .rev()
                            .map(Work::Declaration),
                    );
                }
                Work::Declaration(ReferenceDeclaration::Captured(declaration)) => {
                    work.extend(
                        self.source
                            .reference_super_type_nodes(declaration)?
                            .into_iter()
                            .rev()
                            .map(Work::Heritage),
                    );
                }
                Work::Declaration(ReferenceDeclaration::Json { .. }) => {}
                Work::Heritage(heritage) => {
                    let ty = self.type_of_node(heritage)?;
                    if !self.model.has_type_symbol(ty)? {
                        continue;
                    }
                    if let Some(property) = self
                        .reference_property_symbol_by_key_completion(ty, name, consumer)?
                        .admit()?
                    {
                        for root in self.reference_root_symbols(property, consumer)? {
                            if let Some(result) = callback(self, root)? {
                                return Ok(Some(result));
                            }
                        }
                    }
                    if let Some(base) = self.reference_base_type_symbol(ty, consumer)? {
                        work.push(Work::Symbol(base));
                    }
                }
            }
        }
        Ok(None)
    }

    /// The recursion gate asks Class | Interface after the heritage property
    /// callback. A real non-class synthetic symbol is not a missing symbol.
    fn reference_base_type_symbol(
        &self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        use super::super::model::TypeSymbol;
        Ok(match self.model.type_symbol(ty) {
            TypeSymbol::Bound { symbol, .. } => {
                Some(ReferenceSymbol::from_origin(ReadOrigin::Direct(symbol)))
            }
            TypeSymbol::Property { symbol, .. } => Some(ReferenceSymbol::from_property(symbol)),
            TypeSymbol::Absent => None,
            TypeSymbol::Synthetic { flags, .. } if !flags.class && !flags.interface => None,
            TypeSymbol::Synthetic { .. } => {
                return Err(self.access_pending(
                    consumer,
                    "native synthetic class/interface declaration association is unavailable",
                ));
            }
        })
    }

    pub(in crate::project::semantic) fn reference_parameter_property_other(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let Some(ReferenceDeclaration::Captured(declaration)) =
            self.reference_symbol_value_declaration(symbol, consumer)?
        else {
            return Ok(None);
        };
        if !self.source.reference_parameter_property(declaration)? {
            return Ok(None);
        }
        let (parameter, property) =
            self.reference_parameter_property_symbols(declaration, consumer)?;
        Ok(Some(
            if self
                .reference_symbol_flags(symbol, consumer)?
                .function_scoped_variable
            {
                property
            } else {
                parameter
            },
        ))
    }

    /// `GetSymbolsOfParameterPropertyDeclaration` performs both own-table
    /// lookups, including complete late member resolution on the class side:
    /// the constructor's Locals and `getMembersOfSymbol` of the class node's
    /// own `Symbol()` (`parameter.Parent.Parent`), each through `getSymbol`
    /// with the Value meaning. A constructor local has no symbol parent.
    fn reference_parameter_property_symbols(
        &mut self,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> Result<(ReferenceSymbol, ReferenceSymbol), DemandError> {
        let raw = self.reference_symbol_of_declaration(declaration)?;
        let name = self.reference_symbol_name(raw, consumer)?;
        let name = std::str::from_utf8(&name).map_err(|_| {
            self.access_pending(
                consumer,
                "parameter-property name has no exact scalar identity",
            )
        })?;
        let ordering = self.source.declaration_ordering(declaration)?;
        let table = self
            .source
            .bindings()
            .parameter_property_table(declaration.file, ordering.span)
            .map_err(|failure| self.generic_binding_failure(failure, consumer))?
            .ok_or_else(|| super::super::internal("parameter property lacks constructor locals"))?;
        let raw = self
            .reference_raw_table_lookup(
                super::reference_symbols::ReferenceRawSymbolTable::Retained(table),
                &crate::project::binding::NativePropertyKey::fixed(name.as_bytes()),
                consumer,
            )?
            .ok_or_else(|| {
                super::super::internal("parameter property lacks constructor local symbol")
            })?;
        let parameter = self
            .select_reference_symbol_completion(
                raw,
                crate::project::binding::NativeLookupMeaning::VALUE,
                false,
                consumer,
            )?
            .admit()?
            .ok_or_else(|| {
                super::super::internal("parameter property local lacks value meaning")
            })?;
        let class = self
            .source
            .declaration_parent(declaration)?
            .map(|constructor| self.source.declaration_parent(constructor))
            .transpose()?
            .flatten()
            .ok_or_else(|| super::super::internal("parameter property lacks its class node"))?;
        let class = self
            .reference_raw_symbol_of_declaration(class)?
            .ok_or_else(|| super::super::internal("parameter property class lacks its symbol"))?;
        let key = crate::project::binding::NativePropertyKey::fixed(name.as_bytes());
        let member = self
            .reference_members_of_symbol(class, consumer)?
            .and_then(|table| {
                table
                    .iter()
                    .find(|(candidate, _)| *candidate == key)
                    .map(|(_, symbol)| *symbol)
            })
            .ok_or_else(|| super::super::internal("parameter property lacks own class member"))?;
        let property = self
            .select_reference_symbol_completion(
                member,
                crate::project::binding::NativeLookupMeaning::VALUE,
                false,
                consumer,
            )?
            .admit()?
            .ok_or_else(|| {
                super::super::internal("parameter property member lacks value meaning")
            })?;
        Ok((parameter, property))
    }

    /// `getReferenceForShorthandProperty` checks Transient, not Synthetic.
    /// A found use can add its own shorthand declaration-name occurrence.
    pub(in crate::project::semantic) fn reference_shorthand_fallback(
        &mut self,
        inventory: &crate::project::binding::Bindings,
        symbol: ReferenceSymbol,
        site: &ReferenceSite,
    ) -> Result<Option<(ReferenceSymbol, ReferenceSite)>, DemandError> {
        if self
            .reference_symbol_flags(symbol, &site.selection)?
            .transient
        {
            return Ok(None);
        }
        let Some(ReferenceDeclaration::Captured(declaration)) =
            self.reference_symbol_value_declaration(symbol, &site.selection)?
        else {
            return Ok(None);
        };
        let Some(value) = self.shorthand_assignment_value_symbol(declaration, &site.selection)?
        else {
            return Ok(None);
        };
        Ok(self
            .source
            .reference_declaration_name(inventory, declaration)?
            .map(|site| (value, site)))
    }
}
