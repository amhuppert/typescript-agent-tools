//! Native type-alias declarations and raw-argument instantiation caches.
//! Substitution stays with the shared mapper and anonymous factory owners.

use super::super::model::{
    Alias, AliasKey, AliasSymbol, DemandError, NativeCompletion, NativeResult, NativeSequence,
    TypeId,
};
use super::super::model::{KeyBuilder, same_alias, same_types};
use super::super::source::{AliasRecipe, TypeExpression};
use super::{Evaluator, ReferenceSymbol};
use crate::Span;
use crate::analysis::SemanticFailureReason;
use crate::project::binding::NativeSymbolIdentity;
use crate::references::Location;
use crate::resources::{CheckedKey, CheckedMap};
use std::collections::HashMap;

pub(super) struct AliasesState {
    parameters: HashMap<NativeSymbolIdentity, NativeResult<Vec<TypeId>>>,
    instantiations: CheckedMap<InstantiationKey, NativeResult<TypeId>>,
}

impl AliasesState {
    pub(super) fn new() -> Self {
        Self {
            parameters: HashMap::new(),
            instantiations: CheckedMap::new(),
        }
    }
}

/// `getTypeAliasInstantiationKey`: actual symbol plus raw supplied arguments
/// and enclosing alias. Defaults are filled only after this exact cache lookup.
struct InstantiationKey {
    symbol: NativeSymbolIdentity,
    arguments: Vec<TypeId>,
    alias: Option<AliasKey>,
    fingerprint: u64,
}

impl InstantiationKey {
    fn new(symbol: NativeSymbolIdentity, arguments: &[TypeId], alias: Option<&Alias>) -> Self {
        let alias = AliasKey::of(alias);
        let mut hash = KeyBuilder::new();
        hash.fixed(&symbol);
        hash.types(arguments);
        hash.alias(alias.as_ref());
        Self {
            symbol,
            arguments: arguments.to_vec(),
            alias,
            fingerprint: hash.finish(),
        }
    }
}

impl CheckedKey for InstantiationKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        self.symbol == other.symbol
            && same_types(&self.arguments, &other.arguments)
            && same_alias(self.alias.as_ref(), other.alias.as_ref())
    }
}

impl Evaluator<'_, '_> {
    /// `isLocalTypeAlias` is a declaration ancestry question. The retained
    /// declaration supplies syntax without replacing the actual alias cache.
    fn is_local_reference_type_alias(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        for declaration in self.reference_symbol_declarations(symbol, consumer)? {
            if let super::ReferenceDeclaration::Captured(declaration) = declaration
                && self.source.declaration_kind(declaration)?
                    == crate::syntax_kind::SyntaxKind::TypeAliasDeclaration
            {
                return self.source.is_local_type_alias(declaration);
            }
        }
        Ok(false)
    }

    /// getTypeArgumentsForAliasSymbol reuses canonical local parameters. It
    /// may run during body construction before declaredType is installed.
    pub(super) fn alias_parameters_completion(
        &mut self,
        symbol: impl Into<AliasSymbol>,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        let symbol = match symbol.into() {
            AliasSymbol::Bound(symbol) => ReferenceSymbol::from(symbol),
            AliasSymbol::Created(symbol) => ReferenceSymbol::from_property(symbol),
        };
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        if let Some(parameters) = self.aliases.parameters.get(&identity) {
            return parameters.clone();
        }
        let parameters = match symbol.property() {
            Some(property) if self.reference_raw_origin(symbol)?.is_none() => {
                self.created_alias_parameters_completion(property, consumer)
            }
            _ => self.local_type_parameters_of_symbol_completion(symbol, consumer),
        };
        self.aliases.parameters.insert(identity, parameters.clone());
        parameters
    }

    /// `typeAliasLinks.Get(symbol).typeParameters` after getDeclaredTypeOfSymbol.
    /// This observes the raw selected symbol's slot; it must not compute a
    /// second family's parameters or follow an accepted import alias.
    pub(super) fn installed_alias_parameter_count(
        &self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<usize, DemandError> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        match self.aliases.parameters.get(&identity) {
            Some(Ok(completed)) => Ok(completed.value().len()),
            Some(Err(error)) => Err(error.clone()),
            None => Ok(0),
        }
    }

    /// getDeclaredTypeOfTypeAlias seeds the raw-parameter instantiation after
    /// its resolution frame has popped, before publishing declaredType.
    pub(super) fn finish_alias_declaration_completion(
        &mut self,
        symbol: ReferenceSymbol,
        value: NativeCompletion<TypeId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        let alias_symbol = self.reference_alias_symbol(symbol, consumer)?;
        let completed = value.and_then(|ty| {
            self.alias_parameters_completion(alias_symbol, consumer)
                .map(|parameters| parameters.map(|parameters| (ty, parameters)))
        })?;
        let (ty, parameters) = completed.value();
        let ty = *ty;
        let key =
            (!parameters.is_empty()).then(|| InstantiationKey::new(identity, parameters, None));
        let completed = completed.map(|(ty, _)| ty);
        if let Some(key) = key {
            self.aliases
                .instantiations
                .insert(key, Ok(completed.clone()));
        }
        if self.model.is_intrinsic_marker(ty)
            && self.reference_symbol_name(symbol, consumer)? == b"BuiltinIteratorReturn"
        {
            return Ok(completed.map(|_| {
                self.model.special(
                    if self
                        .inputs
                        .semantic_options()
                        .strict_builtin_iterator_return
                    {
                        super::super::model::SpecialName::Undefined
                    } else {
                        super::super::model::SpecialName::Any
                    },
                )
            }));
        }
        Ok(completed)
    }

    /// getTypeFromTypeAliasReference acquires declaredType before arity and
    /// asks no child argument until that count is valid. Enclosing alias and
    /// imported alias contexts are separate native symbol questions.
    pub(super) fn type_alias_reference_completion(
        &mut self,
        symbol: impl Into<ReferenceSymbol>,
        ty: TypeId,
        expression: &TypeExpression,
        imported_alias: Option<Span>,
        arguments: &[TypeExpression],
        context: Option<&AliasRecipe>,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let symbol = symbol.into();
            let consumer = &expression.location;
            let alias_symbol = self.reference_alias_symbol(symbol, consumer)?;
            let parameters =
                sequence.read(self.alias_parameters_completion(alias_symbol, consumer))?;
            if parameters.is_empty() {
                return sequence
                    .read(self.without_type_arguments_completion(ty, arguments, consumer));
            }
            let minimum = self.minimum_type_argument_count(&parameters)?;
            if arguments.len() < minimum || arguments.len() > parameters.len() {
                return sequence.read(NativeCompletion::recovered(
                    self.model.native_error_type(),
                    super::super::invalid_at_location(
                        self.inputs,
                        consumer,
                        SemanticFailureReason::ErrorType,
                        "generic type-alias argument count consumes native errorType recovery",
                    )
                    .into(),
                ));
            }
            let mut alias = None;
            if let Some(recipe) = context {
                let candidate = sequence
                    .read(self.reference_symbol_of_declaration_completion(recipe.declaration))?;
                if self.is_local_reference_type_alias(symbol, consumer)?
                    || !self.is_local_reference_type_alias(candidate, consumer)?
                {
                    alias = Some(
                        sequence
                            .read(self.establish_alias_symbol_completion(candidate, consumer))?,
                    );
                }
            }
            let imported = if alias.is_none()
                && let Some(anchor) = imported_alias
            {
                sequence
                    .read(self.reference_entity_at_completion(
                        expression.file,
                        anchor,
                        true,
                        crate::project::binding::NativeLookupMeaning::ALIAS,
                        true,
                        consumer,
                    ))?
                    .0
            } else {
                None
            };
            // Native import-alias arguments precede the ordinary raw argument
            // list below. These are separate acquisitions even when cached.
            if let Some(imported) =
                imported.filter(|symbol| !self.is_unknown_reference_symbol(*symbol))
                && let Some(resolved) =
                    sequence.read(self.resolve_alias_target_completion(imported, consumer))?
                && self
                    .reference_symbol_native_flags(resolved, consumer)?
                    .type_flags()
                    .type_alias
            {
                let mut imported_alias =
                    sequence.read(self.establish_alias_symbol_completion(resolved, consumer))?;
                imported_alias.arguments = Some(
                    sequence
                        .read(self.type_arguments_from_nodes_completion(arguments))?
                        .into(),
                );
                alias = Some(imported_alias);
            }
            let actual = sequence.read(self.type_arguments_from_nodes_completion(arguments))?;
            sequence
                .read(self.type_alias_instantiation_completion(symbol, ty, actual, alias, consumer))
        })();
        sequence.finish(result)
    }

    /// getTypeAliasInstantiation keys raw supplied arguments before resolving
    /// missing defaults. Only a cache miss creates the mapper and substitutes.
    pub(super) fn type_alias_instantiation_completion(
        &mut self,
        symbol: impl Into<ReferenceSymbol>,
        ty: TypeId,
        actual: Vec<TypeId>,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let symbol = symbol.into();
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        let alias_symbol = self.reference_alias_symbol(symbol, consumer)?;
        if self.model.is_intrinsic_marker(ty) && actual.len() == 1 {
            let name = self.reference_symbol_name(symbol, consumer)?;
            if name == b"NoInfer" {
                return self.no_infer_type_completion(actual[0], consumer);
            }
            if let Some(kind) = super::super::model::StringMappingKind::from_name(&name) {
                return self.string_mapping_type_completion(
                    alias_symbol,
                    kind,
                    actual[0],
                    consumer,
                );
            }
        }
        let key = InstantiationKey::new(identity, &actual, alias.as_ref());
        if let Some(result) = self.aliases.instantiations.get(&key) {
            return result.clone();
        }
        let result = self
            .alias_parameters_completion(alias_symbol, consumer)?
            .and_then(|parameters| {
                self.fill_missing_type_arguments_completion(actual, &parameters, false, consumer)?
                    .and_then(|actual| {
                        let mapper = self.model.mapper(parameters, actual)?;
                        self.instantiate_type_with_alias_completion(ty, mapper, alias, consumer)
                    })
            });
        if result.is_ok() {
            self.aliases.instantiations.insert(key, result.clone());
        }
        result
    }

    /// instantiateTypeAlias retains the canonical symbol while substituting
    /// only its actual argument identities through the same mapper owner.
    pub(super) fn instantiate_type_alias_completion(
        &mut self,
        alias: Option<Alias>,
        mapper: super::super::model::MapperId,
        consumer: &Location,
    ) -> NativeResult<Option<Alias>> {
        let Some(mut alias) = alias else {
            return Ok(NativeCompletion::clean(None));
        };
        let Some(arguments) = alias.arguments.take() else {
            return Ok(NativeCompletion::clean(Some(alias)));
        };
        let mut mapped = NativeCompletion::clean(Vec::with_capacity(arguments.len()));
        for &argument in arguments.iter() {
            mapped = mapped.and_then(|mut mapped| {
                Ok(self
                    .instantiate_type_completion(argument, mapper, consumer)?
                    .map(|argument| {
                        mapped.push(argument);
                        mapped
                    }))
            })?;
        }
        Ok(mapped.map(|arguments| {
            alias.arguments = Some(arguments.into());
            Some(alias)
        }))
    }
}
