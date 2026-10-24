//! Native signature construction uses captured declaration recipes and the
//! request's shared question cache. No body is checked merely to read an annotation.

use super::super::model::MapperId;
use super::super::model::{
    DemandError, DemandId, NativeCompletion, NativeResult, NativeSequence, ObjectFlags, Parameter,
    ParameterValue, Predicate, Signature, SignatureData, SignatureFlags, SignatureId, SpecialName,
    TupleElementFlags, TupleElementInfo, TupleElementLabel, TypeDemand, TypeFlags, TypeId,
    TypeView,
};
use super::super::model::{KeyBuilder, same_types};
use super::super::source::{
    SignatureParameterRecipe, SignatureQuestion, SignatureRecipe, SignatureReturnRecipe,
    SignatureRoot,
};
use super::{DemandRecipe, Evaluator, TypeDemandState, TypeState, resolution};
use crate::analysis::SemanticFailureReason;
use crate::project::binding::{BoundDeclaration, BoundSymbol};
use crate::references::Location;
use crate::resources::{CheckedKey, CheckedMap};
use std::collections::{HashMap, HashSet};

mod composite;
mod deprecated;
mod expanded;
mod predicates;

/// `signatureLinks.resolvedSignature` stores the actual declaration's factory
/// result. Constraint/default types are distinct lazy questions; output
/// origins and the signature's eventual return type never identify this cache.
pub(super) struct SignaturesState {
    declared: HashMap<SignatureRoot, NativeResult<Signature>>,
    declarations: HashMap<SignatureId, SignatureDeclaration>,
    returns: HashMap<SignatureId, (TypeDemand, Option<Location>)>,
    return_signatures: HashMap<DemandId, SignatureId>,
    minimum_arguments: HashMap<SignatureId, NativeResult<usize>>,
    instantiations: CheckedMap<InstantiationKey, NativeCompletion<Signature>>,
    instantiated: HashMap<SignatureId, (Signature, MapperId)>,
    erased: HashMap<SignatureId, NativeCompletion<Signature>>,
    base: HashMap<SignatureId, NativeCompletion<Signature>>,
    canonical: HashMap<SignatureId, NativeCompletion<Signature>>,
    predicates: HashMap<SignatureId, NativeCompletion<Option<Predicate>>>,
    isolated_types: HashMap<SignatureId, TypeId>,
    composites: HashMap<SignatureId, composite::CompositeSignature>,
    unknown: Option<Signature>,
    optional: HashMap<(SignatureId, super::super::model::CallChain), Signature>,
    optional_returns: HashMap<SignatureId, super::super::model::CallChain>,
}

/// `CachedSignatureKey` / `getTypeListKey`: the actual signature plus ordered
/// filled arguments. Return spelling and declaration origins are not identity.
struct InstantiationKey {
    signature: SignatureId,
    arguments: Vec<TypeId>,
    fingerprint: u64,
}

impl InstantiationKey {
    fn new(signature: SignatureId, arguments: &[TypeId]) -> Self {
        let fingerprint = {
            let mut hash = KeyBuilder::new();
            hash.fixed(&signature);
            hash.types(arguments);
            hash.finish()
        };
        let arguments = { arguments.to_vec() };
        Self {
            signature,
            arguments,
            fingerprint,
        }
    }
}

impl CheckedKey for InstantiationKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        self.signature == other.signature && same_types(&self.arguments, &other.arguments)
    }
}

#[derive(Clone, Copy)]
enum SignatureDeclaration {
    Own(SignatureRoot),
    Instantiated(SignatureId),
    Synthetic,
}

/// `typePredicateKindsMatch` compares native assertion/ordinary kind and the
/// This/parameter index, independently of the public assertion vocabulary.
pub(super) fn predicate_kinds_match(source: &Predicate, target: &Predicate) -> bool {
    (source.kind == crate::types::TypePredicateKind::Is)
        == (target.kind == crate::types::TypePredicateKind::Is)
        && source.subject == target.subject
}

impl SignaturesState {
    pub(super) fn new() -> Self {
        Self {
            declared: HashMap::new(),
            declarations: HashMap::new(),
            returns: HashMap::new(),
            return_signatures: HashMap::new(),
            minimum_arguments: HashMap::new(),
            instantiations: CheckedMap::new(),
            instantiated: HashMap::new(),
            erased: HashMap::new(),
            base: HashMap::new(),
            canonical: HashMap::new(),
            predicates: HashMap::new(),
            isolated_types: HashMap::new(),
            composites: HashMap::new(),
            unknown: None,
            optional: HashMap::new(),
            optional_returns: HashMap::new(),
        }
    }

    pub(super) fn has_return_property(
        &self,
        signature: SignatureId,
        demands: &[TypeDemandState],
    ) -> bool {
        match self.returns.get(&signature) {
            Some((TypeDemand::Known(_), _)) => true,
            Some((TypeDemand::Deferred(demand), _)) => demands
                .get(demand.index())
                .and_then(|demand| demand.state.as_ref())
                .is_some_and(TypeState::has_type),
            None => false,
        }
    }

    pub(super) fn return_signature(&self, demand: DemandId) -> Option<SignatureId> {
        self.return_signatures.get(&demand).copied()
    }
}

impl Evaluator<'_, '_> {
    /// Native getNonRestParameterCount (utilities1245), not getParameterCount.
    pub(super) fn signature_non_rest_parameter_count(signature: &Signature) -> usize {
        signature.parameters.len() - usize::from(signature.flags.has_rest())
    }

    /// Native tryGetRestTypeOfSignature (checker9611). A fixed rest tuple can
    /// yield nil, and the tuple tail is followed by a numeric index question;
    /// neither effective-rest expansion nor getRestTypeAtPosition is equivalent.
    pub(super) fn try_rest_type_of_signature_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            if !signature.flags.has_rest() {
                return Ok(None);
            }
            let mut rest =
                sequence.read(self.signature_rest_parameter_type_completion(signature))?;
            if self.model.tuple_target(rest)?.is_some() {
                let Some(tail) =
                    sequence.read(self.get_rest_type_of_tuple_type_completion(rest, consumer))?
                else {
                    return Ok(None);
                };
                rest = tail;
            }
            let number = self.model.primitive(crate::types::PrimitiveName::Number);
            sequence.read(self.index_type_of_type_completion(rest, number, consumer))
        })();
        sequence.finish(result)
    }
    /// `getOptionalCallSignature` caches fresh cloneSignature identities by
    /// the actual input and chain kind. Ordinary clones drop chain flags; the
    /// optional factory alone installs this return transformation.
    pub(super) fn optional_call_signature(
        &mut self,
        signature: &Signature,
        chain: super::super::model::CallChain,
        consumer: &Location,
    ) -> Result<Signature, DemandError> {
        if signature.flags.call_chain() == Some(chain) {
            return Ok(signature.clone());
        }
        if let Some(cached) = self.signatures.optional.get(&(signature.id, chain)) {
            return Ok(cached.clone());
        }
        let mut result = self.clone_signature(signature, consumer)?;
        result.flags = result.flags.with_call_chain(chain);
        self.signatures.optional_returns.insert(result.id, chain);
        self.signatures
            .optional
            .insert((signature.id, chain), result.clone());
        Ok(result)
    }
    /// `contextuallyCheckFunctionExpressionOrObjectLiteralMethod` installs
    /// resolvedReturnType on the existing canonical signature. Its lazy
    /// descriptor and reverse identity remain unchanged for every handle.
    pub(super) fn assign_contextual_signature_return(
        &mut self,
        signature: &Signature,
        ty: NativeCompletion<TypeId>,
    ) -> Result<(), DemandError> {
        let (demand, _) =
            self.signatures.returns.get(&signature.id).ok_or_else(|| {
                super::super::internal("contextual signature has no return property")
            })?;
        if let TypeDemand::Deferred(id) = demand {
            match &self.demands[id.index()].state {
                Some(TypeState::Installed(_)) => {}
                Some(TypeState::Failed(error)) => return Err(error.clone()),
                Some(TypeState::Resolving) | None => {
                    self.demands[id.index()].state = Some(TypeState::Installed(ty))
                }
            }
        }
        Ok(())
    }

    /// `getOrCreateTypeFromSignature` has a per-signature identity and actual
    /// declaration symbol. Constructors and declarationless defaults expose
    /// a construct inventory; every other signature exposes a call inventory.
    pub(super) fn type_from_signature(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        if let Some(&ty) = self.signatures.isolated_types.get(&signature.id) {
            return Ok(ty);
        }
        let root = self.signature_declaration_root(signature.id)?;
        let construct = root.is_none_or(|root| {
            matches!(
                root.site().kind,
                crate::syntax_kind::SyntaxKind::Constructor
                    | crate::syntax_kind::SyntaxKind::ConstructSignature
                    | crate::syntax_kind::SyntaxKind::ConstructorType
            )
        });
        let symbol = if let Some(root) = root {
            let facts = self.source.signature_declaration_facts(root)?;
            match self.reference_raw_symbol_of_declaration(facts.declaration)? {
                Some(symbol) => self.type_symbol_of_reference(symbol, consumer)?,
                None => super::super::model::TypeSymbol::Absent,
            }
        } else {
            super::super::model::TypeSymbol::Absent
        };
        let signatures: std::sync::Arc<[Signature]> = vec![signature.clone()].into();
        let empty: std::sync::Arc<[Signature]> = Vec::new().into();
        let members = super::super::model::MembersView {
            properties: Vec::new().into(),
            call_signatures: if construct {
                empty.clone()
            } else {
                signatures.clone()
            },
            construct_signatures: if construct { signatures } else { empty },
            index_signatures: Vec::new().into(),
        };
        let ty = self.model.structured_with_flags(
            symbol,
            members,
            super::super::model::Metadata::default(),
            super::super::model::ObjectKind::Anonymous,
            ObjectFlags::SINGLE_SIGNATURE_TYPE,
        );
        self.signatures.isolated_types.insert(signature.id, ty);
        Ok(ty)
    }
    /// `getDefaultConstructSignatures` creates a declaration-less signature
    /// when the class has no base constructors. Its class return is already
    /// installed, independently of any later signature mapper.
    pub(super) fn fresh_default_constructor(
        &mut self,
        class_type: TypeId,
        is_abstract: bool,
        consumer: &Location,
    ) -> Result<Signature, DemandError> {
        let (type_parameter_types, type_parameters) =
            self.class_signature_parameters(class_type)?;
        let signature = self.model.signature(SignatureData {
            type_parameter_types,
            type_parameters,
            this_parameter: None,
            parameters: Vec::new().into(),
            return_type: TypeDemand::Known(class_type),
            predicate: None,
            origins: Vec::new().into(),
            minimum_arguments: 0,
            flags: SignatureFlags::declared(false, false, true, is_abstract),
        })?;
        self.attach_signature_declaration(signature.id, None)?;
        self.attach_signature_return(&signature, consumer.clone())?;
        Ok(signature)
    }

    /// `getDefaultConstructSignatures` uses a fresh createSignatureInstantiation
    /// for generic bases and cloneSignature otherwise. The class owner has
    /// already filtered arity and filled arguments. No cached signature is
    /// mutated when local parameters, resolvedReturnType and Abstract change.
    pub(super) fn derive_default_constructor_completion(
        &mut self,
        base: &Signature,
        filled_arguments: Option<Vec<TypeId>>,
        class_type: TypeId,
        is_abstract: bool,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Signature, DemandError> {
            let mut signature = if let Some(arguments) = filled_arguments {
                let parameters = sequence
                    .read(self.get_type_parameters_for_mapper_completion(base, consumer))?;
                let mapper = self.model.mapper(parameters, arguments)?;
                sequence
                    .read(self.instantiate_signature_ex_completion(base, mapper, true, consumer))?
            } else {
                self.clone_signature(base, consumer)?
            };
            let (types, parameters) = self.class_signature_parameters(class_type)?;
            signature.replace_factory_type_parameters(types, parameters)?;
            signature.flags = signature.flags.with_abstract(is_abstract);
            signature.predicate = None;
            self.install_signature_return(&mut signature, class_type, consumer)?;
            Ok(signature)
        })();
        sequence.finish(result)
    }

    fn class_signature_parameters(
        &self,
        class_type: TypeId,
    ) -> Result<(std::sync::Arc<[TypeId]>, Vec<Parameter>), DemandError> {
        let types = self.model.declared_target(class_type)?.local_parameters;
        let parameters = types
            .iter()
            .map(|&ty| match self.model.get(ty)? {
                TypeView::TypeParameter(parameter) => Ok(parameter),
                _ => Err(
                    super::super::internal("class signature local has another type family").into(),
                ),
            })
            .collect::<Result<Vec<_>, DemandError>>()?;
        Ok((types, parameters))
    }

    /// `getParameterCount` observes the actual rest type before its tuple
    /// fixed-length metadata. Ordinary syntax length alone cannot answer rest.
    pub(super) fn signature_parameter_count_completion(
        &mut self,
        signature: &Signature,
        _consumer: &Location,
    ) -> NativeResult<usize> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<usize, DemandError> {
            let length = signature.parameters.len();
            if signature.flags.has_rest() {
                let rest =
                    sequence.read(self.signature_rest_parameter_type_completion(signature))?;
                if let Some(tuple) = self.model.tuple_target(rest)? {
                    return Ok(length + tuple.fixed_length
                        - usize::from(
                            !tuple.combined_flags.intersects(TupleElementFlags::VARIABLE),
                        ));
                }
            }
            Ok(length)
        })();
        sequence.finish(result)
    }

    fn signature_rest_parameter_type_completion(
        &mut self,
        signature: &Signature,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let parameter = signature
                .parameters
                .last()
                .ok_or_else(|| super::super::internal("rest signature has no parameter"))?;
            sequence.read(self.demand_completion(parameter.value_type))
        })();
        sequence.finish(result)
    }

    /// `hasEffectiveRestParameter`: an actual non-tuple rest is effective;
    /// tuple rest requires its retained native Variable flags.
    pub(super) fn signature_has_effective_rest_completion(
        &mut self,
        signature: &Signature,
        _consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if !signature.flags.has_rest() {
                return Ok(false);
            }
            let rest = sequence.read(self.signature_rest_parameter_type_completion(signature))?;
            Ok(self
                .model
                .tuple_target(rest)?
                .is_none_or(|tuple| tuple.combined_flags.intersects(TupleElementFlags::VARIABLE)))
        })();
        sequence.finish(result)
    }

    /// `getNonArrayRestType` uses getEffectiveRestType's Any-to-anyArray
    /// conversion before deciding whether a rest position needs tuple work.
    pub(super) fn signature_non_array_rest_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let Some(rest) =
                sequence.read(self.signature_effective_rest_completion(signature, consumer))?
            else {
                return Ok(None);
            };
            Ok(if self.array_element_type(rest)?.is_some() {
                None
            } else {
                Some(rest)
            })
        })();
        sequence.finish(result)
    }

    /// `getEffectiveRestType` reads the raw rest symbol type, converting Any
    /// through the actual global Array factory before any element question.
    pub(super) fn signature_effective_rest_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            if !signature.flags.has_rest() {
                return Ok(None);
            }
            let rest = sequence.read(self.signature_rest_parameter_type_completion(signature))?;
            if let Some(tuple) = self.model.tuple_target(rest)? {
                return if tuple.combined_flags.intersects(TupleElementFlags::VARIABLE) {
                    self.slice_tuple_type(rest, tuple.fixed_length, 0, consumer)
                        .map(Some)
                } else {
                    Ok(None)
                };
            }
            let rest = if self.model.has_type_flags(rest, TypeFlags::ANY) {
                self.array_type(rest, consumer)?
            } else {
                rest
            };
            Ok(Some(rest))
        })();
        sequence.finish(result)
    }

    /// `getTypeAtPosition` uses Any only for an established absent position.
    /// A failed positional or rest-index demand remains the original failure.
    pub(super) fn signature_type_at_position_or_any_completion(
        &mut self,
        signature: &Signature,
        position: usize,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            Ok(sequence
                .read(self.signature_type_at_position_completion(signature, position, consumer))?
                .unwrap_or_else(|| self.model.special(SpecialName::Any)))
        })();
        sequence.finish(result)
    }

    /// `tryGetTypeAtPosition` reads both existing positional types even when
    /// the counterpart is absent. Rest indexing remains its distinct native
    /// indexed-access question rather than a direct element approximation.
    pub(super) fn signature_type_at_position_completion(
        &mut self,
        signature: &Signature,
        position: usize,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let ordinary = Self::signature_non_rest_parameter_count(signature);
            if position < ordinary {
                return sequence
                    .read(self.demand_completion(signature.parameters[position].call_type))
                    .map(Some);
            }
            if signature.flags.has_rest() {
                let rest =
                    sequence.read(self.signature_rest_parameter_type_completion(signature))?;
                let index = position - ordinary;
                if self.model.tuple_target(rest)?.is_none_or(|tuple| {
                    tuple.combined_flags.intersects(TupleElementFlags::VARIABLE)
                        || index < tuple.fixed_length
                }) {
                    let key = self
                        .model
                        .literal(crate::types::PrimitiveName::Number, index.to_string());
                    let key = self.model.regular_literal(key);
                    return self.indexed_access_type(rest, key, consumer).map(Some);
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    pub(super) fn signature_this_type_completion(
        &mut self,
        signature: &Signature,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = signature
            .this_parameter()
            .as_ref()
            .map(|parameter| sequence.read(self.demand_completion(parameter.value_type)))
            .transpose();
        sequence.finish(result)
    }

    /// `isTopSignature`: this observation precedes the parameter-count guard;
    /// qualifying rest parameters then precede the return observation.
    pub(super) fn is_top_signature_completion(
        &mut self,
        signature: &Signature,
        _consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if !signature.type_parameter_types().is_empty() {
                return Ok(false);
            }
            if let Some(parameter) = signature.this_parameter() {
                let this = sequence.read(self.demand_completion(parameter.call_type))?;
                if !self.model.has_type_flags(this, TypeFlags::ANY) {
                    return Ok(false);
                }
            }
            if signature.parameters.len() != 1 || !signature.flags.has_rest() {
                return Ok(false);
            }
            let parameter =
                sequence.read(self.demand_completion(signature.parameters[0].call_type))?;
            let rest = self.array_element_type(parameter)?.unwrap_or(parameter);
            if !self
                .model
                .has_type_flags(rest, TypeFlags::ANY | TypeFlags::NEVER)
            {
                return Ok(false);
            }
            let returned =
                sequence.read(self.raw_return_type_of_signature_completion(signature))?;
            Ok(self
                .model
                .has_type_flags(returned, TypeFlags::ANY | TypeFlags::UNKNOWN))
        })();
        sequence.finish(result)
    }

    /// `isInstantiatedGenericParameter` asks the actual target's positional
    /// type. Instantiated declaration origins do not imply generic parameters.
    pub(super) fn is_instantiated_generic_parameter_completion(
        &mut self,
        signature: &Signature,
        position: usize,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let Some((target, _)) = self.signatures.instantiated.get(&signature.id).cloned() else {
                return Ok(false);
            };
            let Some(ty) = sequence
                .read(self.signature_type_at_position_completion(&target, position, consumer))?
            else {
                return Ok(false);
            };
            self.is_generic_type(ty, consumer)
        })();
        sequence.finish(result)
    }

    /// `getSignatureInstantiation` fills missing arguments before the cached
    /// operation on every invocation. Its acquired default evidence is not part
    /// of the cached fresh-factory result. Existing callers with no inferred
    /// type-parameter list use the empty-list entry to the same whole owner.
    pub(super) fn get_signature_instantiation_completion(
        &mut self,
        signature: &Signature,
        arguments: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        self.get_signature_instantiation_with_inferred_completion(
            signature,
            arguments,
            &[],
            consumer,
        )
    }

    pub(super) fn get_signature_instantiation_with_inferred_completion(
        &mut self,
        signature: &Signature,
        arguments: Vec<TypeId>,
        inferred_parameters: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<Signature> {
        self.fill_missing_type_arguments_completion(
            arguments,
            signature.type_parameter_types(),
            consumer,
        )?
        .and_then(|arguments| {
            self.get_signature_instantiation_without_filling_completion(
                signature, arguments, consumer,
            )
        })?
        .and_then(|instantiated| {
            if inferred_parameters.is_empty() {
                return Ok(NativeCompletion::clean(instantiated));
            }
            self.raw_return_type_of_signature_completion(&instantiated)?
                .and_then_value(|returned| {
                    let returned_signature =
                        match self.single_signature(returned, false, false, consumer)? {
                            Some(signature) => Some(signature),
                            None => self.single_signature(returned, true, false, consumer)?,
                        };
                    let Some(returned_signature) = returned_signature else {
                        return Ok(instantiated);
                    };
                    let mut returned_signature =
                        self.clone_signature(&returned_signature, consumer)?;
                    let parameters = inferred_parameters
                        .iter()
                        .map(|&parameter| match self.model.get(parameter)? {
                            TypeView::TypeParameter(parameter) => Ok(parameter),
                            _ => Err(super::super::internal(
                                "inferred signature parameter has another type family",
                            )
                            .into()),
                        })
                        .collect::<Result<Vec<_>, DemandError>>()?;
                    returned_signature.replace_factory_type_parameters(
                        inferred_parameters.to_vec().into(),
                        parameters,
                    )?;
                    let returned_type = self.type_from_signature(&returned_signature, consumer)?;
                    if let Some(mapper) = self.signature_mapper(&instantiated) {
                        self.model.install_object_mapper(returned_type, mapper)?;
                    }
                    let mut result = self.clone_signature(&instantiated, consumer)?;
                    self.install_signature_return(&mut result, returned_type, consumer)?;
                    Ok(result)
                })
        })
    }

    /// `getSignatureInstantiationWithoutFillingInTypeArguments` owns the one
    /// native signature/argument-list cache. An interrupted factory installs
    /// nothing; completed recovery retains its actual fresh signature.
    pub(super) fn get_signature_instantiation_without_filling_completion(
        &mut self,
        signature: &Signature,
        arguments: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        let key = InstantiationKey::new(signature.id, &arguments);
        if let Some(instantiation) = self.signatures.instantiations.get(&key) {
            return Ok(instantiation.clone());
        }
        let instantiation =
            self.create_signature_instantiation_completion(signature, arguments, consumer)?;
        self.signatures
            .instantiations
            .insert(key, instantiation.clone());
        Ok(instantiation)
    }

    /// `createSignatureInstantiation` and `createSignatureTypeMapper`: a
    /// fresh signature, with no argument filling and no cache lookup. Native
    /// overload recovery deliberately calls this rather than the cached owner.
    pub(super) fn create_signature_instantiation_completion(
        &mut self,
        signature: &Signature,
        arguments: Vec<TypeId>,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        self.get_type_parameters_for_mapper_completion(signature, consumer)?
            .and_then(|parameters| {
                let mapper = self.model.mapper(parameters, arguments)?;
                self.instantiate_signature_ex_completion(signature, mapper, true, consumer)
            })
    }

    /// `getCanonicalSignature` substitutes an actual clone's unconstrained
    /// target identity, leaving constrained clones distinct across generations.
    pub(super) fn canonical_signature_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        if signature.type_parameter_types().is_empty() {
            return Ok(NativeCompletion::clean(signature.clone()));
        }
        if let Some(canonical) = self.signatures.canonical.get(&signature.id) {
            return Ok(canonical.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut arguments = Vec::new();
            for &parameter in signature.type_parameter_types().iter() {
                let argument = if let Some(target) = self.model.parameter_target(parameter) {
                    if sequence
                        .read(self.parameter_constraint_completion(target, consumer))?
                        .is_none()
                    {
                        target
                    } else {
                        parameter
                    }
                } else {
                    parameter
                };
                arguments.push(argument);
            }
            sequence
                .read(self.get_signature_instantiation_completion(signature, arguments, consumer))
        })();
        let canonical = sequence.finish(result)?;
        self.signatures
            .canonical
            .insert(signature.id, canonical.clone());
        Ok(canonical)
    }

    /// `getMinArgumentCountEx` caches the question on the actual signature,
    /// including instantiated identities. Native someType(Void) examines
    /// union constituents; Undefined/Any are different optionality questions.
    pub(super) fn minimum_argument_count_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<usize> {
        self.minimum_argument_count_ex_completion(signature, false, consumer)
    }

    /// The syntactic-arity mode of `getMinArgumentCountEx` ignores both the
    /// resolved minimum cache and Void optionality, and does not publish it.
    /// StrongArityForUntypedJS has no additional branch in accepted TS input.
    pub(super) fn minimum_argument_count_ex_completion(
        &mut self,
        signature: &Signature,
        void_is_non_optional: bool,
        consumer: &Location,
    ) -> NativeResult<usize> {
        if !void_is_non_optional
            && let Some(value) = self.signatures.minimum_arguments.get(&signature.id)
        {
            return value.clone();
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut minimum = signature.minimum_arguments;
            if signature.flags.has_rest() {
                let rest =
                    sequence.read(self.signature_rest_parameter_type_completion(signature))?;
                if let Some(tuple) = self.model.tuple_target(rest)? {
                    let required = tuple
                        .elements
                        .iter()
                        .position(|element| !element.flags.intersects(TupleElementFlags::REQUIRED))
                        .unwrap_or(tuple.fixed_length);
                    if required > 0 {
                        minimum = signature.parameters.len() - 1 + required;
                    }
                }
            }
            if void_is_non_optional {
                return Ok(minimum);
            }
            for index in (0..minimum).rev() {
                let ty = sequence.read(
                    self.signature_type_at_position_or_any_completion(signature, index, consumer),
                )?;
                if !self.contains_void(ty) {
                    break;
                }
                minimum = index;
            }
            Ok(minimum)
        })();
        let result = sequence.finish(result);
        if !void_is_non_optional {
            self.signatures
                .minimum_arguments
                .insert(signature.id, result.clone());
        }
        result
    }

    pub(super) fn contains_void(&self, ty: TypeId) -> bool {
        self.model.some_type_of_kind(ty, TypeFlags::VOID)
    }

    /// Native Signature.declaration is an actual factory association. Missing
    /// association is an invariant failure; only a proven synthetic factory
    /// can establish the absence of a declaration.
    pub(super) fn attach_signature_declaration(
        &mut self,
        signature: SignatureId,
        declaration: Option<SignatureRoot>,
    ) -> Result<(), DemandError> {
        if self.signatures.declarations.contains_key(&signature) {
            return Err(super::super::internal(
                "signature declaration association was installed twice",
            )
            .into());
        }
        self.signatures.declarations.insert(
            signature,
            match declaration {
                Some(root) => SignatureDeclaration::Own(root),
                None => SignatureDeclaration::Synthetic,
            },
        );
        Ok(())
    }

    /// `instantiateSignatureEx` inherits its actual target's declaration.
    /// Origins, return demands and descriptor equality do not answer it.
    pub(super) fn attach_signature_instantiation(
        &mut self,
        signature: &Signature,
        target: &Signature,
        mapper: MapperId,
    ) -> Result<(), DemandError> {
        if self.signatures.declarations.contains_key(&signature.id)
            || !self.signatures.declarations.contains_key(&target.id)
        {
            return Err(super::super::internal(
                "instantiated signature declaration has an invalid factory association",
            )
            .into());
        }
        self.signatures
            .declarations
            .insert(signature.id, SignatureDeclaration::Instantiated(target.id));
        self.signatures
            .instantiated
            .insert(signature.id, (target.clone(), mapper));
        Ok(())
    }

    /// `getBaseSignature` substitutes each immediate constraint N times,
    /// then erases residual cycles before publishing one cached signature.
    pub(super) fn base_signature_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        let parameters = signature.type_parameter_types();
        if parameters.is_empty() {
            return Ok(NativeCompletion::clean(signature.clone()));
        }
        if let Some(base) = self.signatures.base.get(&signature.id) {
            return Ok(base.clone());
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let unknown = self.model.special(SpecialName::Unknown);
            let mut constraints = Vec::with_capacity(parameters.len());
            for &parameter in parameters.iter() {
                constraints.push(
                    sequence
                        .read(self.parameter_constraint_completion(parameter, consumer))?
                        .unwrap_or(unknown),
                );
            }
            let mapper = self.model.mapper(parameters.to_vec(), constraints)?;
            let mut constraints = parameters.to_vec();
            for _ in 0..parameters.len() {
                for ty in &mut constraints {
                    *ty = sequence.read(self.instantiate_type_completion(*ty, mapper, consumer))?;
                }
            }
            let any = self.model.special(SpecialName::Any);
            let eraser = self.model.array_to_single_mapper(parameters.to_vec(), any);
            for ty in &mut constraints {
                *ty = sequence.read(self.instantiate_type_completion(*ty, eraser, consumer))?;
            }
            let mapper = self.model.mapper(parameters.to_vec(), constraints)?;
            sequence
                .read(self.instantiate_signature_ex_completion(signature, mapper, true, consumer))
        })();
        let base = sequence.finish(result)?;
        self.signatures.base.insert(signature.id, base.clone());
        Ok(base)
    }

    /// `getErasedSignature` maps the actual local parameters to Any without
    /// reading their constraints, caching only a completed factory result.
    pub(super) fn erased_signature_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        if signature.type_parameter_types().is_empty() {
            return Ok(NativeCompletion::clean(signature.clone()));
        }
        if let Some(erased) = self.signatures.erased.get(&signature.id) {
            return Ok(erased.clone());
        }
        let any = self.model.special(SpecialName::Any);
        let mapper = self
            .model
            .array_to_single_mapper(signature.type_parameter_types().to_vec(), any);
        let erased = self.instantiate_signature_ex_completion(signature, mapper, true, consumer)?;
        self.signatures.erased.insert(signature.id, erased.clone());
        Ok(erased)
    }

    /// `getNonCircularReturnTypeOfSignature` deliberately uses a transient Any
    /// for an in-progress native return. This read-only observation does not
    /// push a frame, install recovery or manufacture a circularity failure.
    pub(super) fn non_circular_signature_return_completion(
        &mut self,
        signature: &Signature,
        _consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if self.is_resolving_signature_return(signature) {
                return Ok(self.model.special(SpecialName::Any));
            }
            sequence.read(self.raw_return_type_of_signature_completion(signature))
        })();
        sequence.finish(result)
    }

    /// `getTypePredicateOfSignature` resolves the actual target predicate
    /// before mapping it. Explicit predicate types are demanded separately
    /// from their Boolean/Void return annotation and cached per signature.
    pub(in crate::project::semantic) fn type_predicate_of_signature(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> Result<Option<Predicate>, DemandError> {
        self.type_predicate_of_signature_completion(signature, consumer)?
            .admit()
    }

    pub(in crate::project::semantic) fn type_predicate_of_signature_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<Option<Predicate>> {
        self.resolve_signature_predicate_completion(signature, consumer)
            .map(|value| {
                value.observe(|failure| {
                    super::super::observe_recovery(self.inputs, failure, consumer)
                })
            })
            .map_err(|failure| super::super::observe_recovery(self.inputs, failure, consumer))
    }

    // Raw cached predicates keep source-free evidence. Only outward actual
    // observations attach a location, independently for every returned cause.
    fn resolve_signature_predicate_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<Option<Predicate>> {
        if let Some(predicate) = self.signatures.predicates.get(&signature.id) {
            return Ok(predicate.clone());
        }
        enum Next {
            Value(Option<Predicate>),
            Body(SignatureRoot),
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if let Some((target, mapper)) = self.signatures.instantiated.get(&signature.id).cloned()
            {
                let Some(mut predicate) = sequence
                    .read(self.resolve_signature_predicate_completion(&target, consumer))?
                else {
                    return Ok(Next::Value(None));
                };
                if let Some(ty) = predicate.value_type {
                    let ty = sequence.read(self.demand_completion(ty))?;
                    predicate.value_type = Some(TypeDemand::Known(
                        sequence.read(self.instantiate_type_completion(ty, mapper, consumer))?,
                    ));
                }
                return Ok(Next::Value(Some(predicate)));
            }
            if let Some(composite) = self.signatures.composites.get(&signature.id).cloned() {
                return sequence
                    .read(self.composite_signature_predicate_completion(&composite, consumer))
                    .map(Next::Value);
            }
            // A signature clone retains its declaration's predicate metadata.
            // Reading that metadata is not another signature-factory demand.
            let declaration_predicate = if signature.predicate.is_none() {
                match self.signature_declaration_root(signature.id)? {
                    Some(root) => match self.signatures.declared.get(&root) {
                        Some(Ok(value)) => value.value().predicate.clone(),
                        Some(Err(error)) => return Err(error.clone()),
                        None => None,
                    },
                    None => None,
                }
            } else {
                None
            };
            if let Some(mut predicate) = signature.predicate.clone().or(declaration_predicate) {
                if let Some(ty) = predicate.value_type {
                    predicate.value_type = Some(TypeDemand::Known(
                        sequence.read(self.demand_completion(ty))?,
                    ));
                }
                return Ok(Next::Value(Some(predicate)));
            }
            if self.signature_return_is_annotated(signature)? {
                return Ok(Next::Value(None));
            }
            if let Some(root) = self.signature_declaration_root(signature.id)?
                && crate::syntax::is_function_like_declaration_kind(root.site().kind)
            {
                // Native reads resolvedReturnType's flags without acquiring
                // that property again. An unfinished failed operation has no
                // native value to use for this eligibility decision.
                let can_infer = match signature.return_type {
                    TypeDemand::Known(ty) => self.model.has_type_flags(ty, TypeFlags::BOOLEAN),
                    TypeDemand::Deferred(id) => match self.demands[id.index()].state.as_ref() {
                        Some(TypeState::Installed(value)) => self
                            .model
                            .has_type_flags(*value.value(), TypeFlags::BOOLEAN),
                        Some(TypeState::Failed(error)) => return Err(error.clone()),
                        Some(TypeState::Resolving) | None => true,
                    },
                };
                if can_infer
                    && sequence
                        .read(self.signature_parameter_count_completion(signature, consumer))?
                        > 0
                {
                    return Ok(Next::Body(root));
                }
            }
            Ok(Next::Value(None))
        })();
        let prepared = sequence.finish(result)?;
        let result = match prepared.value() {
            Next::Body(root) => {
                let root = *root;
                let prefix = prepared.map(|_| None);
                // noTypePredicate breaks reentry before body checking. Keep
                // this actual installation when the body is interrupted.
                self.signatures
                    .predicates
                    .insert(signature.id, prefix.clone());
                prefix.and_then(|_| self.type_predicate_from_body_completion(root, consumer))
            }
            Next::Value(_) => Ok(prepared.map(|next| match next {
                Next::Value(value) => value,
                Next::Body(_) => unreachable!("body handled above"),
            })),
        };
        if let Ok(value) = &result {
            self.signatures
                .predicates
                .insert(signature.id, value.clone());
        }
        result
    }

    /// `hasTypePredicateOrNeverReturnType` asks the actual declaration's return
    /// annotation independently from the established return cache property.
    pub(super) fn signature_return_is_annotated(
        &mut self,
        signature: &Signature,
    ) -> Result<bool, DemandError> {
        let Some(root) = self.signature_declaration_root(signature.id)? else {
            return Ok(false);
        };
        self.source
            .signature_return_has_annotation(root)
            .map_err(Into::into)
    }

    /// `getRestTypeAtPosition` creates the native tuple of remaining parameter
    /// types, preserving actual nameable declarations independently of names.
    pub(super) fn signature_rest_type_at_position_completion(
        &mut self,
        signature: &Signature,
        position: usize,
        readonly: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let count =
                sequence.read(self.signature_parameter_count_completion(signature, consumer))?;
            let minimum =
                sequence.read(self.minimum_argument_count_completion(signature, consumer))?;
            let rest =
                sequence.read(self.signature_effective_rest_completion(signature, consumer))?;
            if let Some(rest) = rest
                && position >= count.saturating_sub(1)
            {
                if position == count.saturating_sub(1) {
                    return Ok(rest);
                }
                let number = self.model.primitive(crate::types::PrimitiveName::Number);
                let element = self.indexed_access_type(rest, number, consumer)?;
                return self.array_type(element, consumer);
            }
            let length = count.saturating_sub(position);
            let mut types = Vec::with_capacity(length);
            let mut infos = Vec::with_capacity(length);
            for index in 0..length {
                let (ty, flags) = if let Some(rest) = rest
                    && index + 1 == length
                {
                    (rest, TupleElementFlags::VARIADIC)
                } else {
                    (
                        sequence.read(self.signature_type_at_position_or_any_completion(
                            signature,
                            position + index,
                            consumer,
                        ))?,
                        if position + index < minimum {
                            TupleElementFlags::REQUIRED
                        } else {
                            TupleElementFlags::OPTIONAL
                        },
                    )
                };
                types.push(ty);
                infos.push(TupleElementInfo {
                    flags,
                    label: sequence.read(
                        self.signature_nameable_label_completion(signature, position + index),
                    )?,
                });
            }
            self.create_tuple_type_ex(types, infos, readonly, consumer)
        })();
        sequence.finish(result)
    }

    /// `getRestOrAnyTypeAtPosition` deliberately treats an any-element array
    /// as Any; required tuple elements otherwise reject that ordinary array.
    pub(super) fn signature_rest_or_any_at_position_completion(
        &mut self,
        signature: &Signature,
        position: usize,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            let rest = sequence.read(self.signature_rest_type_at_position_completion(
                signature, position, false, consumer,
            ))?;
            if let Some(element) = self.array_element_type(rest)?
                && self.model.has_type_flags(element, TypeFlags::ANY)
            {
                return Ok(self.model.special(SpecialName::Any));
            }
            Ok(rest)
        })();
        sequence.finish(result)
    }

    /// `getNameableDeclarationAtPosition` keeps the rest type's tuple label
    /// nodes, or the original valid parameter declaration for ordinary rests.
    fn signature_nameable_label_completion(
        &mut self,
        signature: &Signature,
        position: usize,
    ) -> NativeResult<Option<TupleElementLabel>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TupleElementLabel>, DemandError> {
            let ordinary = Self::signature_non_rest_parameter_count(signature);
            if position < ordinary {
                return match signature.parameters[position].declaration {
                    Some(declaration) => self.source.nameable_tuple_label(declaration),
                    None => Ok(None),
                };
            }
            if signature.flags.has_rest() {
                let parameter = &signature.parameters[ordinary];
                let rest = sequence.read(self.demand_completion(parameter.value_type))?;
                if let Some(tuple) = self.model.tuple_target(rest)? {
                    return Ok(tuple
                        .elements
                        .get(position - ordinary)
                        .and_then(|info| info.label.clone()));
                }
                if let Some(declaration) = parameter.declaration {
                    return self.source.nameable_tuple_label(declaration);
                }
            }
            Ok(None)
        })();
        sequence.finish(result)
    }

    pub(super) fn signature_declaration_root(
        &self,
        mut signature: SignatureId,
    ) -> Result<Option<SignatureRoot>, DemandError> {
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(signature) {
                return Err(super::super::internal(
                    "signature declaration target associations are circular",
                )
                .into());
            }
            match self.signatures.declarations.get(&signature) {
                Some(SignatureDeclaration::Own(root)) => return Ok(Some(*root)),
                Some(SignatureDeclaration::Synthetic) => return Ok(None),
                Some(SignatureDeclaration::Instantiated(target)) => signature = *target,
                None => {
                    return Err(super::super::internal(
                        "signature identity has no declaration association",
                    )
                    .into());
                }
            }
        }
    }

    /// `getTypeOfFuncClassEnumModuleWorker` / `resolveAnonymousTypeMembers`
    /// establish a lazy anonymous value. Merged module/class flags share the
    /// ordinary symbol inventory; the canonical value cache owns its identity.
    pub(super) fn function_value_type(
        &mut self,
        symbol: BoundSymbol,
        declarations: &[BoundDeclaration],
    ) -> Result<TypeId, DemandError> {
        let serialization = if self.source.function_symbol_is_nonlocal(declarations)? {
            super::super::model::FunctionSymbolSerialization::NonLocal
        } else {
            super::super::model::FunctionSymbolSerialization::Local
        };
        let first = declarations
            .first()
            .ok_or_else(|| super::super::internal("canonical function has no own declarations"))?;
        let consumer = super::super::location(self.inputs, first.file, first.anchor);
        let native = self.type_symbol(symbol, &consumer);
        let ty = self.model.anonymous_type(native, Default::default());
        self.model.attach_function_symbol(ty, serialization)?;
        self.register_function_type_consumer(ty, symbol, consumer);
        Ok(ty)
    }

    /// `getSignatureFromDeclaration` constructs an established descriptor
    /// without demanding parameter or return types. Those are distinct cached
    /// questions, preserving annotation-before-initializer and lazy returns.
    pub(super) fn signature(&mut self, recipe: &SignatureRecipe) -> Result<Signature, DemandError> {
        self.signature_completion(recipe)?.admit()
    }

    pub(super) fn signature_completion(
        &mut self,
        recipe: &SignatureRecipe,
    ) -> NativeResult<Signature> {
        if let Some(signature) = self.signatures.declared.get(&recipe.root) {
            return signature.clone();
        }
        let result = self.signature_from_declaration_completion(recipe);
        self.signatures.declared.insert(recipe.root, result.clone());
        result
    }

    fn signature_from_declaration_completion(
        &mut self,
        recipe: &SignatureRecipe,
    ) -> NativeResult<Signature> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Signature, DemandError> {
            // `getSignatureFromDeclaration` records the last nonoptional argument.
            // For this admitted typed, non-IIFE family, initialized+rest recipes
            // stay pending at acquisition until tuple-sensitive arity exists.
            let minimum_arguments = recipe
                .parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    !parameter.question_mark && parameter.initializer.is_none() && !parameter.rest
                })
                .map(|(index, _)| index + 1)
                .next_back()
                .unwrap_or(0);
            let this_parameter = recipe
                .this_parameter
                .as_ref()
                .map(|parameter| {
                    self.signature_parameter_value(
                        recipe,
                        parameter,
                        SignatureQuestion::This,
                        SignatureQuestion::ThisDisplay,
                        0,
                        minimum_arguments,
                    )
                })
                .transpose()?;
            let raw_offset = usize::from(recipe.this_parameter.is_some());
            let parameters = recipe
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    self.signature_parameter_value(
                        recipe,
                        parameter,
                        SignatureQuestion::Parameter(index),
                        SignatureQuestion::ParameterDisplay(index),
                        index + raw_offset,
                        minimum_arguments,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (type_parameter_types, type_parameters) = match &recipe.return_type {
                SignatureReturnRecipe::Constructor { class } => {
                    let class_type =
                        sequence.read(self.constructor_class_type_completion(*class))?;
                    let (types, parameters) = self.class_signature_parameters(class_type)?;
                    (types.to_vec(), parameters)
                }
                _ => sequence
                    .read(self.signature_type_parameters_completion(&recipe.type_parameters))?,
            };
            // `getTypePredicateOfSignature`'s explicit-node family has a separate
            // asserted-type demand. Reading the return annotation alone asks for
            // boolean/void and does not consume that asserted child.
            let predicate = self.signature_predicate_from_recipe(recipe, &parameters)?;
            let return_type = self.register_signature_demand(
                recipe.root,
                SignatureQuestion::Return,
                DemandRecipe::Return {
                    root: recipe.root,
                    recipe: recipe.return_type.clone(),
                },
            );
            let signature = self.model.signature(SignatureData {
                type_parameter_types: type_parameter_types.into(),
                type_parameters,
                this_parameter,
                parameters: parameters.into(),
                return_type,
                predicate,
                origins: vec![recipe.origin.clone()].into(),
                minimum_arguments,
                flags: recipe.flags,
            })?;
            self.attach_signature_declaration(signature.id, Some(recipe.root))?;
            let return_location = match &recipe.return_type {
                SignatureReturnRecipe::Annotation(annotation) => annotation.location.clone(),
                SignatureReturnRecipe::Bodyless
                | SignatureReturnRecipe::Constructor { .. }
                | SignatureReturnRecipe::Infer
                | SignatureReturnRecipe::GetterAccessor { .. } => recipe.origin.clone(),
            };
            self.attach_signature_return(&signature, return_location)?;
            Ok(signature)
        })();
        sequence.finish(result)
    }

    /// An actual SignatureId owns its native resolvedReturnType property.
    /// Source roots identify original factory inputs, never instantiated
    /// return properties. The reverse attachment admits re-entry at the same
    /// native question even when the lazy wrapper is already resolving.
    pub(super) fn attach_signature_return(
        &mut self,
        signature: &Signature,
        location: impl Into<Option<Location>>,
    ) -> Result<(), DemandError> {
        if self.signatures.returns.contains_key(&signature.id) {
            return Err(super::super::internal(
                "signature return attachment was reused by another native identity",
            )
            .into());
        }
        if let TypeDemand::Deferred(demand) = signature.return_type {
            if self.signatures.return_signatures.contains_key(&demand) {
                return Err(
                    super::super::internal("lazy return belongs to another signature").into(),
                );
            }
            self.signatures
                .return_signatures
                .insert(demand, signature.id);
        }
        self.signatures
            .returns
            .insert(signature.id, (signature.return_type, location.into()));
        Ok(())
    }

    pub(in crate::project::semantic) fn signature_return_location(
        &self,
        signature: SignatureId,
    ) -> Result<Option<&Location>, DemandError> {
        self.signatures
            .returns
            .get(&signature)
            .map(|(_, location)| location.as_ref())
            .ok_or_else(|| {
                super::super::internal("native signature has no return property association").into()
            })
    }

    /// Observe a returned copy. The raw native cache remains source-free.
    pub(in crate::project::semantic) fn get_return_type_of_signature(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.get_return_type_of_signature_completion(signature, consumer)?
            .admit()
    }

    pub(in crate::project::semantic) fn get_return_type_of_signature_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.raw_return_type_of_signature_completion(signature)
            .map(|value| {
                value.observe(|failure| {
                    super::super::observe_recovery(self.inputs, failure, consumer)
                })
            })
            .map_err(|failure| super::super::observe_recovery(self.inputs, failure, consumer))
    }

    /// Derived signatures retain source-free evidence in their native slot.
    pub(super) fn raw_return_type_of_signature_completion(
        &mut self,
        signature: &Signature,
    ) -> NativeResult<TypeId> {
        self.demand_completion(signature.return_type)
    }

    /// `getReturnTypeOfSignature`: a rejected push returns errorType without
    /// installing it. A false pop selects anyType, then the native final nil
    /// check preserves a reentrant value. Invocation failure and installation
    /// are distinct; selecting an installed value is not a second acquisition.
    pub(super) fn resolve_signature_return_completion(
        &mut self,
        signature: SignatureId,
    ) -> NativeResult<TypeId> {
        let (demand, location) = self
            .signatures
            .returns
            .get(&signature)
            .cloned()
            .ok_or_else(|| super::super::internal("signature return has no lazy property"))?;
        let demand = match demand {
            TypeDemand::Known(ty) => return Ok(NativeCompletion::clean(ty)),
            TypeDemand::Deferred(demand) => demand,
        };
        let entry = &self.demands[demand.index()];
        match &entry.state {
            Some(TypeState::Installed(value)) => {
                return Ok(value.clone());
            }
            Some(TypeState::Failed(error)) => return Err(error.clone()),
            Some(TypeState::Resolving) | None => {}
        }
        let recipe = entry.recipe.clone();
        let location = location
            .ok_or_else(|| super::super::internal("unresolved signature return has no source"))?;
        let inputs = self.inputs;
        let circular = || {
            super::super::invalid_at_location(
                inputs,
                &location,
                SemanticFailureReason::CircularType,
                "signature return consumes native circular type-resolution recovery",
            )
            .into()
        };
        if !self.push_resolution(resolution::Key::SignatureReturn(signature)) {
            return NativeCompletion::recovered(self.model.native_error_type(), circular());
        }
        self.demands[demand.index()].state = Some(TypeState::Resolving);
        let result = match &recipe {
            DemandRecipe::Return { root, recipe } => {
                self.check_signature_return_completion(*root, recipe)
            }
            DemandRecipe::SignatureReturn => {
                self.synthetic_signature_return_completion(signature, &location)
            }
            DemandRecipe::Generic(super::generics::GenericDemand::Instantiated { .. }) => {
                let DemandRecipe::Generic(recipe) = &recipe else {
                    unreachable!()
                };
                self.check_generic_demand_completion(recipe)
            }
            _ => Err(super::super::internal(
                "signature return attachment has another question role",
            )
            .into()),
        };
        let result = result.and_then(|value| {
            value.and_then_value(|ty| {
                match self.signatures.optional_returns.get(&signature).copied() {
                    Some(super::super::model::CallChain::Inner) => {
                        self.add_optional_marker(ty, &location)
                    }
                    Some(super::super::model::CallChain::Outer) => {
                        self.optional_type(ty, false, &location)
                    }
                    None => Ok(ty),
                }
            })
        });
        let resolved = self.resolution.pop().ok_or_else(|| {
            super::super::internal("signature return has no native resolution frame")
        })?;
        let result = result.and_then(|value| {
            let value = if resolved {
                value
            } else {
                value.and_then(|_| {
                    NativeCompletion::recovered(self.model.special(SpecialName::Any), circular())
                })?
            };
            if let Some(TypeState::Installed(inner)) = &self.demands[demand.index()].state {
                return Ok(value.map(|_| *inner.value()));
            }
            Ok(value)
        });
        if !self.demands[demand.index()]
            .state
            .as_ref()
            .is_some_and(TypeState::has_type)
        {
            self.demands[demand.index()].state = Some(match &result {
                Ok(value) => TypeState::Installed(value.clone()),
                Err(error) => TypeState::Failed(error.clone()),
            });
        }
        result
    }

    /// `getTypeParametersFromDeclaration` establishes required identity
    /// collection in source order through the canonical declared-symbol cache.
    /// Constraint/default demands remain on the established Parameter view;
    /// collecting a signature neither checks them nor resolves by name.
    fn signature_type_parameters_completion(
        &mut self,
        declarations: &[super::super::source::DeclarationIdentity],
    ) -> NativeResult<(Vec<TypeId>, Vec<Parameter>)> {
        self.append_type_parameters_completion(Vec::new(), declarations)?.and_then_value(|types| {
            let mut parameters = Vec::with_capacity(types.len());
            for &ty in &types {
                let TypeView::TypeParameter(parameter) = self.model.get(ty)? else {
                    return Err(super::super::internal("signature parameter declaration did not establish its canonical type-parameter family").into());
                };
                parameters.push(parameter);
            }
            Ok((types, parameters))
        })
    }

    /// `createTypePredicateFromTypePredicateNode` addresses the signature's
    /// actual parameter symbols, including binder-created pattern names.
    fn signature_predicate_from_recipe(
        &mut self,
        recipe: &SignatureRecipe,
        parameters: &[ParameterValue],
    ) -> Result<Option<Predicate>, DemandError> {
        use super::super::source::SignaturePredicateSubject;
        use crate::types::{TypePredicateSubject, TypeThis};
        let Some(predicate) = &recipe.predicate else {
            return Ok(None);
        };
        let (subject, parameter_name) = match &predicate.subject {
            SignaturePredicateSubject::This => (TypePredicateSubject::This(TypeThis::This), None),
            SignaturePredicateSubject::Parameter(name) => {
                let index = parameters
                    .iter()
                    .position(|parameter| parameter.name == name.as_ref())
                    .ok_or_else(|| {
                        super::super::unsupported_demand_at_location(
                            self.inputs,
                            &recipe.origin,
                            SemanticFailureReason::UnsupportedSemanticForm,
                            "native unbound predicate subject representation is pending",
                        )
                    })?;
                (
                    TypePredicateSubject::ParameterIndex(index),
                    Some(name.clone()),
                )
            }
        };
        let value_type = predicate.value_type.as_ref().map(|value| {
            self.register_signature_demand(
                recipe.root,
                SignatureQuestion::Predicate,
                DemandRecipe::Type(Box::new(value.clone())),
            )
        });
        Ok(Some(Predicate {
            kind: predicate.kind,
            subject,
            parameter_name,
            value_type,
        }))
    }

    fn signature_parameter_value(
        &mut self,
        recipe: &SignatureRecipe,
        parameter: &SignatureParameterRecipe,
        read_question: SignatureQuestion,
        display_question: SignatureQuestion,
        raw_index: usize,
        minimum_arguments: usize,
    ) -> Result<ParameterValue, DemandError> {
        let optional = is_optional_parameter(parameter, raw_index, minimum_arguments);
        let bound = self
            .source
            .raw_symbol_of_declaration(parameter.declaration)?;
        let symbol = self.register_property_symbol(bound)?;
        let name = self.property_lookup_key(symbol, &parameter.location)?;
        let name = self
            .literal_scalar_text(
                &super::super::model::LiteralValue::from_property_key(&name),
                &parameter.location,
            )?
            .to_owned();
        let value_type = self.register_property_symbol_demand(symbol, false, &parameter.location);
        let transforms = super::super::model::ParameterTypeTransforms {
            display_optional: requires_adding_implicit_undefined(
                parameter,
                optional,
                self.inputs.semantic_options().strict_null_checks,
            ),
            call_optional: self.inputs.semantic_options().strict_null_checks
                && (parameter.initializer.is_some() || parameter.question_mark),
        };
        let display_type = if transforms.display_optional {
            self.register_signature_demand(
                recipe.root,
                display_question,
                DemandRecipe::ParameterDisplay {
                    read_type: value_type,
                },
            )
        } else {
            value_type
        };
        let call_type = if transforms.call_optional {
            let question = match read_question {
                SignatureQuestion::This => SignatureQuestion::ThisCall,
                SignatureQuestion::Parameter(index) => SignatureQuestion::ParameterCall(index),
                _ => {
                    unreachable!("parameter constructor received a different native read question")
                }
            };
            self.register_signature_demand(
                recipe.root,
                question,
                DemandRecipe::ParameterCall {
                    read_type: value_type,
                },
            )
        } else {
            value_type
        };
        Ok(ParameterValue {
            symbol,
            name,
            declaration: Some(parameter.declaration),
            value_type,
            display_type,
            call_type,
            optional,
            rest: parameter.rest,
            transforms,
        })
    }

    /// `addOptionalityEx`'s strict getOptionalType(type,false) operation after
    /// the canonical symbol read. Both admitted producers install their own
    /// policy-selected question, then share this complete native operation.
    pub(super) fn optional_parameter_read_completion(
        &mut self,
        read_type: TypeDemand,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.demand_completion(read_type)?
            .and_then_value(|ty| self.optional_type(ty, false, consumer))
    }

    /// `getReturnTypeFromAnnotation` is independent of a signature's mapped
    /// resolved return. Effects ask the actual declaration annotation, while
    /// ordinary return resolution maps it through its own factory operation.
    pub(super) fn signature_annotated_return_completion(
        &mut self,
        signature: &Signature,
    ) -> NativeResult<Option<TypeId>> {
        let Some((_, recipe)) = self.signature_return_recipe(signature.id)? else {
            return Ok(NativeCompletion::clean(None));
        };
        self.return_type_from_annotation_completion(&recipe)
    }

    pub(super) fn return_type_from_annotation_completion(
        &mut self,
        recipe: &SignatureReturnRecipe,
    ) -> NativeResult<Option<TypeId>> {
        match recipe {
            SignatureReturnRecipe::Constructor { class } => self
                .constructor_class_type_completion(*class)
                .map(|value| value.map(Some)),
            SignatureReturnRecipe::Annotation(annotation) => self
                .check_type_completion(annotation)
                .map(|value| value.map(Some)),
            SignatureReturnRecipe::Bodyless | SignatureReturnRecipe::Infer => {
                Ok(NativeCompletion::clean(None))
            }
            // Only a bindable getter consults the setter of its late-bound,
            // merged symbol; otherwise the native full-signature fallback is
            // nil outside JavaScript.
            &SignatureReturnRecipe::GetterAccessor { declaration, .. } => {
                let mut sequence = NativeSequence::new();
                let result = (|| {
                    if !sequence.read(self.has_bindable_declaration_name_completion(declaration))? {
                        return Ok(None);
                    }
                    let symbol = sequence
                        .read(self.reference_symbol_of_declaration_completion(declaration))?;
                    let location = self.source.declaration_location(declaration)?;

                    let declarations = (self
                        .reference_symbol_declarations(symbol, &location)?
                        .into_iter()
                        .map(|declaration| -> Result<_, DemandError> {
                            match declaration {
                                super::ReferenceDeclaration::Captured(declaration) => {
                                    Ok(super::super::source::DeclarationReference::Captured(
                                        declaration,
                                    ))
                                }
                                super::ReferenceDeclaration::Json { .. } => {
                                    Err(super::super::internal(
                                        "a JSON declaration cannot be an accessor",
                                    )
                                    .into())
                                }
                            }
                        }))
                    .collect::<Result<Vec<_>, _>>()?;
                    let recipe = self.source.accessor_recipe(&declarations)?;
                    match recipe.setter.and_then(|setter| setter.annotation) {
                        Some(annotation) => Ok(Some(
                            sequence.read(self.check_type_completion(&annotation))?,
                        )),
                        None => Ok(None),
                    }
                })();
                sequence.finish(result)
            }
        }
    }

    /// `getReturnTypeOfSignature` acquires the annotation before the body.
    pub(super) fn check_signature_return_completion(
        &mut self,
        root: SignatureRoot,
        recipe: &SignatureReturnRecipe,
    ) -> NativeResult<TypeId> {
        self.return_type_from_annotation_completion(recipe)?
            .and_then(|annotation| {
                if let Some(ty) = annotation {
                    return Ok(NativeCompletion::clean(ty));
                }
                match recipe {
                    SignatureReturnRecipe::Infer
                    | SignatureReturnRecipe::GetterAccessor { body: true, .. } => {
                        let body = self.source.return_body(root)?;
                        self.return_body_type_completion(&body)
                    }
                    SignatureReturnRecipe::Bodyless
                    | SignatureReturnRecipe::GetterAccessor { body: false, .. } => Ok(
                        NativeCompletion::clean(self.model.special(SpecialName::Any)),
                    ),
                    SignatureReturnRecipe::Annotation(_)
                    | SignatureReturnRecipe::Constructor { .. } => Err(super::super::internal(
                        "explicit return annotation produced no type",
                    )
                    .into()),
                }
            })
    }

    /// `getSignatureFromDeclaration` and `getReturnTypeFromAnnotation` ask
    /// the same declared class question at separate native demand points.
    fn constructor_class_type_completion(
        &mut self,
        class: super::super::source::DeclarationIdentity,
    ) -> NativeResult<TypeId> {
        let consumer = self.source.declaration_location(class)?;
        // Both native callers ask getMergedSymbol(declaration.Parent.Symbol()).
        // The anonymous class keyword is syntax provenance, not symbol identity.
        let raw = self
            .reference_raw_symbol_of_declaration(class)?
            .ok_or_else(|| {
                super::super::internal("constructor parent class lacks its raw symbol")
            })?;
        let symbol = self.get_merged_reference_symbol(raw, &consumer)?;
        self.declared_reference_type_completion(symbol, &consumer)
    }
}

/// `isOptionalParameter`: a question token is optional independently of a
/// later required parameter. A default asks strong arity with VoidIsNonOptional
/// and compares the raw syntax index (including a first this parameter).
/// Contextual IIFE/JS and initialized tuple-rest decisions are not admitted by
/// Source's current signature recipe operation.
fn is_optional_parameter(
    parameter: &SignatureParameterRecipe,
    raw_index: usize,
    minimum_arguments: usize,
) -> bool {
    parameter.question_mark || parameter.initializer.is_some() && raw_index >= minimum_arguments
}

/// `requiresAddingImplicitUndefinedWorker` / `isRequiredInitializedParameter`
/// for an ordinary parse-tree parameter. Parameter-property modifiers remain
/// a separate pending source family. Annotation-contained undefined is
/// preserved by the display operation's complete native union normalization.
fn requires_adding_implicit_undefined(
    parameter: &SignatureParameterRecipe,
    optional: bool,
    strict_null_checks: bool,
) -> bool {
    strict_null_checks && !optional && parameter.initializer.is_some()
}
