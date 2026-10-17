//! Lexical private identifiers retain their actual containing-class identity.
//! Receiver lookup and diagnostic display are separate native operations.
use super::*;
use crate::project::binding::NativePropertyKey;
use crate::project::semantic::evaluation::{ReferenceDeclaration, ReferenceSymbol};
use crate::project::semantic::model::{NativeSequence, Property};
use crate::project::semantic::source::DeclarationIdentity;

impl Evaluator<'_, '_> {
    /// `checkPrivateIdentifierExpression` checks the lexical private symbol
    /// and marks it referenced, but its type is Any even for native nil.
    /// Grammar diagnostics do not acquire another type or symbol.
    pub(in crate::project::semantic::evaluation) fn check_private_identifier_expression_completion(
        &mut self,
        root: super::super::super::source::ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.reference_symbol_at_node_completion(root, consumer)?
            .and_then_value(|symbol| {
                if let Some(symbol) = symbol {
                    let property = self.register_reference_symbol(symbol, consumer)?;
                    self.mark_property_as_referenced(property, None, false, consumer)?;
                }
                Ok(self
                    .model
                    .special(super::super::super::model::SpecialName::Any))
            })
    }

    /// `binder.GetSymbolNameForPrivateIdentifier`: the caller's actual class
    /// symbol owns the generated key. GetSymbolId is an acquisition event;
    /// equality-only key observations elsewhere do not allocate ordering ranks.
    pub(in crate::project::semantic::evaluation) fn private_identifier_key(
        &mut self,
        class: ReferenceSymbol,
        description: &str,
        consumer: &Location,
    ) -> Result<NativePropertyKey, DemandError> {
        let identity = self.reference_symbol_identity(class, consumer)?;
        self.model.observe_symbol_identity(identity);
        Ok(NativePropertyKey::private(
            identity,
            description,
            self.control.as_ref(),
        ))
    }

    /// `lookupSymbolForPrivateIdentifierDeclaration` walks lexical classes
    /// lazily and asks Members before Exports. These are raw keyed table reads,
    /// without late binding, merging, or acquisition of unrelated entries.
    pub(in crate::project::semantic::evaluation) fn lookup_private_identifier_declaration(
        &mut self,
        description: &str,
        location: DeclarationIdentity,
        consumer: &Location,
    ) -> Result<Option<ReferenceSymbol>, DemandError> {
        let mut class = self.source.private_identifier_containing_class(location)?;
        while let Some(current) = class {
            let symbol = self
                .reference_raw_symbol_of_declaration(current)?
                .ok_or_else(|| {
                    super::super::super::internal("containing class lacks its native raw symbol")
                })?;
            let key = self.private_identifier_key(symbol, description, consumer)?;
            if let Some(table) = self.reference_raw_symbol_table(symbol, false)?
                && let Some(property) = self.reference_raw_table_lookup(table, &key, consumer)?
            {
                return Ok(Some(property));
            }
            if let Some(table) = self.reference_raw_symbol_table(symbol, true)?
                && let Some(property) = self.reference_raw_table_lookup(table, &key, consumer)?
            {
                return Ok(Some(property));
            }
            class = self.source.containing_class(current)?;
        }
        Ok(None)
    }

    /// `getPrivateIdentifierPropertyOfType` uses the lexical symbol's exact
    /// key against the original left type, independently of its apparent type.
    pub(super) fn private_identifier_property_of_type_completion(
        &mut self,
        left: TypeId,
        lexical: ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<Option<Property>> {
        let key = self.reference_symbol_key(lexical, consumer)?;
        self.property_of_type_by_key_completion(left, &key, consumer)
    }

    /// `checkPrivateIdentifierPropertyAccess` only reports the private-specific
    /// shadow/outside-class cases. A genuine absence continues the ordinary
    /// nonexistent-property reporter at the outer access owner.
    pub(super) fn check_private_identifier_property_access_completion(
        &mut self,
        left: TypeId,
        description: &str,
        lexical: Option<ReferenceSymbol>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut found = None;
            for property in sequence.read(self.properties_of_type_completion(left, consumer))? {
                let property = property.symbol.ok_or_else(|| {
                    super::super::super::internal(
                        "native property inventory lost its canonical symbol",
                    )
                })?;
                if let Some(ReferenceDeclaration::Captured(declaration)) = self
                    .reference_symbol_value_declaration(
                        ReferenceSymbol::from_property(property),
                        consumer,
                    )?
                    && self
                        .source
                        .private_identifier_declaration_name(declaration)?
                        .as_deref()
                        == Some(description)
                {
                    found = Some(declaration);
                    break;
                }
            }
            let Some(declaration) = found else {
                return Ok(false);
            };
            let type_class = self.source.containing_class(declaration)?.ok_or_else(|| {
                super::super::super::internal("private property lacks its containing class")
            })?;
            if let Some(lexical) = lexical
                && let Some(ReferenceDeclaration::Captured(declaration)) =
                    self.reference_symbol_value_declaration(lexical, consumer)?
                && let Some(lexical_class) = self.source.containing_class(declaration)?
                && self
                    .source
                    .declaration_is_descendant(lexical_class, type_class)?
            {
                sequence.read(self.check_diagnostic_type_display_completion(left, consumer))?;
                return Ok(true);
            }
            let class_symbol = self
                .reference_raw_symbol_of_declaration(type_class)?
                .ok_or_else(|| {
                    super::super::super::internal("private diagnostic class lacks its raw symbol")
                })?;
            sequence.read(self.diagnostic_symbol_display_completion(class_symbol, consumer))?;
            Ok(true)
        })();
        sequence.finish(result)
    }
}
