//! Unique types are cached by actual checker symbol, never by source spelling.

use super::super::model::{DemandError, NativeCompletion, NativeResult, NativeSequence, TypeId};
use super::super::source::{CallRecipe, DeclarationIdentity, ExpressionRoot};
use super::Evaluator;
use crate::project::binding::{NativePropertyKey, NativePropertyKeyKind, NativeSymbolIdentity};
use crate::references::Location;
use crate::resources::FixedMap;
use crate::types::PrimitiveName;

pub(super) struct UniqueSymbolsState {
    symbols: FixedMap<NativeSymbolIdentity, TypeId>,
    nodes: FixedMap<ExpressionRoot, NativeCompletion<TypeId>>,
}

impl UniqueSymbolsState {
    pub(super) fn new() -> Self {
        Self {
            symbols: FixedMap::new(),
            nodes: FixedMap::new(),
        }
    }
}

impl Evaluator<'_, '_> {
    pub(in crate::project::semantic) fn unique_symbol_origins(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Vec<Location>, DemandError> {
        self.type_symbol_origins(ty, consumer)
    }

    /// Public origins of a native type symbol; enum and unique-symbol graphs
    /// ask this same metadata question, without acquiring the symbol type.
    pub(in crate::project::semantic) fn type_symbol_origins(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Vec<Location>, DemandError> {
        let symbol = self
            .reference_symbol_of_type(ty, consumer)?
            .ok_or_else(|| super::super::internal("named type has no actual symbol"))?;
        self.reference_symbol_origins(symbol, consumer)
    }

    /// Owned scalar key projection preserves the existing exact UTF-8
    /// admission rule. Copying and indivisible validation use this request's
    /// controller; invalid native bytes remain Unsupported, never replacement.
    fn property_key_text(
        &self,
        bytes: &[u8],
        consumer: &Location,
        message: &'static str,
    ) -> Result<String, DemandError> {
        let bytes = { bytes.to_vec() };

        let text = String::from_utf8(bytes);
        if let Some(control) = &self.control {
            crate::resources::complete_or_abort(control.check());
        }
        text.map_err(|_| {
            super::super::unsupported_demand_at_location(
                self.inputs,
                consumer,
                crate::analysis::SemanticFailureReason::UnrepresentableIdentity,
                message,
            )
        })
    }

    /// Public key metadata is derived from its actual symbol. Display text is
    /// never used to look up the key or to merge distinct property identities.
    pub(super) fn project_property_key(
        &mut self,
        key: &NativePropertyKey,
        name_type: Option<TypeId>,
        origins: &[Location],
        consumer: &Location,
    ) -> Result<crate::types::TypeKey, DemandError> {
        use crate::types::TypeKey;
        match key.kind(self.control.as_ref()) {
            NativePropertyKeyKind::Fixed(bytes) => Ok(TypeKey::String {
                name: self.property_key_text(
                    bytes,
                    consumer,
                    "property projection requires an exact scalar name",
                )?,
            }),
            NativePropertyKeyKind::Unique { symbol, .. } => {
                let ty = name_type
                    .filter(|ty| {
                        self.model
                            .has_type_flags(*ty, super::super::model::TypeFlags::UNIQUE_SYMBOL)
                    })
                    .or_else(|| self.unique_symbols.symbols.get(&symbol).copied())
                    .ok_or_else(|| {
                        super::super::internal(
                            "unique property key lacks its established unique type",
                        )
                    })?;
                let symbol = self
                    .reference_symbol_of_type(ty, consumer)?
                    .ok_or_else(|| {
                        super::super::internal("unique property type lacks actual symbol")
                    })?;
                let name = self.symbol_name_as_written(symbol, consumer)?;
                Ok(TypeKey::Symbol {
                    name,
                    origins: self.unique_symbol_origins(ty, consumer)?,
                })
            }
            NativePropertyKeyKind::Private { description, .. } => Ok(TypeKey::Private {
                name: self.property_key_text(
                    description,
                    consumer,
                    "private property projection requires an exact scalar description",
                )?,
                origins: { super::super::model::Model::copy_origins(origins) },
            }),
            NativePropertyKeyKind::Composed => Err(super::super::unsupported_demand_at_location(
                self.inputs,
                consumer,
                crate::analysis::SemanticFailureReason::UnrepresentableIdentity,
                "composed native property identity requires a symbolic projection",
            )),
        }
    }

    /// `getESSymbolLikeTypeForNode`: validity precedes optional node-symbol
    /// acquisition. The symbol cache, not the type-node cache, owns uniqueness.
    pub(super) fn es_symbol_like_type_for_node(
        &mut self,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        if self.source.valid_es_symbol_declaration(declaration)?
            && let Some(symbol) = self.reference_node_declaration_symbol(declaration)?
        {
            let identity = self.reference_symbol_identity(symbol, consumer)?;
            if let Some(&ty) = self.unique_symbols.symbols.get(&identity) {
                return Ok(ty);
            }
            let underlying = self.reference_symbol_key(symbol, consumer)?;
            // ast.GetSymbolId is acquired at generated-name construction. The
            // documented request-local ordering policy observes that same point.
            self.model.observe_symbol_identity(identity);
            let key = NativePropertyKey::unique(identity, underlying, self.control.as_ref());
            let symbol = self.type_symbol_of_reference(symbol, consumer)?;
            let ty = self.model.new_unique_es_symbol_type(symbol, key);
            self.unique_symbols.symbols.insert(identity, ty);
            return Ok(ty);
        }
        Ok(self.model.primitive(PrimitiveName::Symbol))
    }

    /// The UniqueKeyword branch of `getTypeFromTypeOperatorNode`. An invalid
    /// immediate operand installs native errorType without visiting that child.
    pub(super) fn unique_type_node_completion(
        &mut self,
        root: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(value) = self.unique_symbols.nodes.get(&root) {
            return Ok(value.clone());
        }
        let completed = if self.source.unique_type_operand_is_symbol(root)? {
            let ty = match self.source.es_symbol_parent(root, true)? {
                Some(parent) => self.es_symbol_like_type_for_node(parent, consumer)?,
                None => self.model.primitive(PrimitiveName::Symbol),
            };
            NativeCompletion::clean(ty)
        } else {
            NativeCompletion::recovered(
                self.model.native_error_type(),
                super::super::invalid_at_location(
                    self.inputs,
                    consumer,
                    crate::analysis::SemanticFailureReason::ErrorType,
                    "native unique type operator requires an immediate symbol keyword",
                )
                .into(),
            )?
        };
        self.unique_symbols.nodes.insert(root, completed.clone());
        Ok(completed)
    }

    /// `isSymbolOrSymbolForCall` resolves the raw Value name after the quiet
    /// global constructor lookup. Alias targets and coincident spelling do not
    /// replace the actual identity comparison.
    pub(super) fn is_symbol_or_symbol_for_call_completion(
        &mut self,
        recipe: &CallRecipe,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(root) = recipe.symbol_candidate else {
                return Ok(false);
            };
            let Some(global) = self.global_value_symbol("Symbol", &recipe.location)? else {
                return Ok(false);
            };
            let local =
                sequence.read(self.reference_value_name_completion(root, &recipe.location))?;
            let Some(local) = local else {
                return Ok(false);
            };
            Ok(self.reference_symbol_identity(global, &recipe.location)?
                == self.reference_symbol_identity(local, &recipe.location)?)
        })();
        sequence.finish(result)
    }
}
