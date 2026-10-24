//! Unresolved TypeAlias symbols retain native identity, Parent and declaredType.

use super::*;
use crate::project::semantic::model::{Alias, AliasSymbol};
use crate::project::semantic::source::ExpressionRoot;

impl Evaluator<'_, '_> {
    /// `getUnresolvedSymbolForEntityName`: intern by the complete native symbol
    /// path in this checker. The terminal empty-name case selects unknownSymbol;
    /// new symbols publish before Parent and declaredType are installed.
    pub(in crate::project::semantic::evaluation) fn unresolved_symbol_for_entity_name(
        &mut self,
        root: ExpressionRoot,
    ) -> Result<ReferenceSymbol, DemandError> {
        let names = self.source.unresolved_entity_name(root)?;
        let mut parent = None;
        let mut path = String::new();
        for name in names {
            if name.is_empty() {
                let symbol = self.unknown_reference_symbol();
                parent = symbol.property();
                path.clear();
                path.push_str("unknown");
                continue;
            }
            {
                if parent.is_some() {
                    path.push('.');
                }
                path.push_str(&name);
            }
            let symbol = match self.native_members.symbols.unresolved_names.get(&path) {
                Some(&symbol) => symbol,
                None => {
                    let name_bytes = { std::sync::Arc::from(name.as_bytes()) };
                    let header =
                        NativeSymbol::created(self.model.new_symbol_identity(), name_bytes, None);
                    let symbol = self.install_created_property(CreatedProperty {
                        native_flags: None,
                        flags: PropertySymbolFlags {
                            types: TypeSymbolFlags {
                                type_: true,
                                type_or_namespace: true,
                                type_alias: true,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        checks: checks::UNRESOLVED,
                        declarations: Vec::new(),
                        value_declaration: None,
                        parent: None,
                        raw_members: None,
                        raw_exports: None,
                        name_type: None,
                        header,
                        mapper: None,
                        target: None,
                        synthetic_origin: None,
                        spread: None,
                        export_type: None,
                        value: CreatedPropertyValue::UnresolvedAlias,
                    });
                    self.native_members
                        .symbols
                        .unresolved_names
                        .insert(path.clone(), symbol);
                    let Association::Created(data) =
                        &mut self.native_members.symbols.entries[symbol.index()].association
                    else {
                        return Err(super::super::super::internal(
                            "unresolved symbol lost its created header",
                        )
                        .into());
                    };
                    CreatedProperty::make_mut(data).parent = parent.map(SymbolParent::Created);
                    self.native_members
                        .symbols
                        .unresolved_declared
                        .insert(symbol, self.model.unresolved_type());
                    symbol
                }
            };
            parent = Some(symbol);
        }
        let symbol = parent.ok_or_else(|| {
            super::super::super::internal("entity name supplied no terminal identifier")
        })?;
        Ok(ReferenceSymbol::from_property(symbol))
    }

    /// Actual typeAliasLinks.declaredType from the unresolved-symbol factory.
    pub(in crate::project::semantic::evaluation) fn unresolved_alias_declared_type(
        &self,
        symbol: PropertySymbolId,
    ) -> Option<TypeId> {
        self.native_members
            .symbols
            .unresolved_declared
            .get(&symbol)
            .copied()
    }

    pub(in crate::project::semantic::evaluation) fn created_alias_parameters_completion(
        &mut self,
        symbol: PropertySymbolId,
        consumer: &Location,
    ) -> crate::project::semantic::model::NativeResult<Vec<TypeId>> {
        if self.unresolved_alias_declared_type(symbol).is_some() {
            // Native installs only declaredType, leaving typeParameters nil.
            return Ok(crate::project::semantic::model::NativeCompletion::clean(
                Vec::new(),
            ));
        }
        self.local_type_parameters_of_symbol_completion(
            ReferenceSymbol::from_property(symbol),
            consumer,
        )
    }

    /// The unresolved alias has no source declaration or display origin. Its
    /// actual symbol and argument identities still participate in native caches.
    pub(in crate::project::semantic::evaluation) fn unresolved_type_alias(
        &self,
        symbol: PropertySymbolId,
        arguments: Vec<TypeId>,
        consumer: &Location,
    ) -> Result<Alias, DemandError> {
        let header = self.property_native_symbol(symbol, consumer)?;
        let name = header
            .name()
            .map_err(|error| self.generic_binding_failure(error, consumer))?;
        let name = String::from_utf8(name).map_err(|_| {
            self.access_pending(consumer, "unresolved symbol spelling is not scalar text")
        })?;
        Ok(Alias {
            native_symbol: TypeSymbol::Property {
                symbol,
                native: header,
            },
            symbol: AliasSymbol::Created(symbol),
            provenance: crate::types::TypeAlias {
                name: name.clone(),
                origins: Vec::new(),
            },
            display_name: name,
            arguments: Some(arguments.into()),
        })
    }
}
