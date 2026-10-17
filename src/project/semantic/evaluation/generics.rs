//! Generic identities and specialized child questions within one Evaluator.

use super::super::model::{
    Alias, AliasKey, DemandError, DemandId, Index, IntersectionFlags, Mapper, MapperTarget,
    MembersOfType, MembersView, NativeCompletion, NativeResult, NativeSequence, ObjectFlags,
    ParameterData, ParameterValue, Property, PropertyDisplayDemand, Signature, SignatureData,
    SpecialName, TargetHeader, TargetKind, TypeDemand, TypeFlags, TypeId, TypeView,
};
use super::super::model::{KeyBuilder, same_alias};
use super::super::source::{
    AliasRecipe, DeferredReferenceRecipe, OuterParameterRecipe, ParameterConstraintRecipe,
    ThisTypeContext, TypeExpression, TypeParameterRecipe,
};
use super::structure::MembersState;
use super::{DemandRecipe, Evaluator, TypeDemandState, TypeState, resolution};
use crate::Span;
use crate::analysis::SemanticFailureReason;
use crate::project::binding::flow::Site;
use crate::project::binding::{
    BoundClassKind, BoundClassOrInterface, BoundDeclaration, BoundDeclaredType, BoundSymbol,
    TypeBindingFailure,
};
use crate::project::sources::FileId;
use crate::references::Location;
use crate::resources::{CheckedKey, CheckedMap};
use crate::syntax_kind::SyntaxKind;
use std::collections::HashMap;

use super::super::model::MapperId;

#[derive(Clone)]
struct TargetAssociation {
    symbol: super::ReferenceSymbol,
    bound: BoundClassOrInterface,
    declaration: BoundDeclaration,
}

enum BaseTypesState {
    Resolving(Vec<TypeId>),
    Resolved(NativeCompletion<Vec<TypeId>>),
    Failed(DemandError),
}

struct ActiveMapper {
    mapper: MapperId,
    cache: CheckedMap<ActiveKey, NativeCompletion<TypeId>>,
}

/// `instantiateTypeWithAlias`: actual input identity and the explicitly
/// supplied alias. The mapper belongs to the enclosing frame, not this key.
struct ActiveKey {
    ty: TypeId,
    alias: Option<AliasKey>,
    fingerprint: u64,
}
impl ActiveKey {
    fn new(ty: TypeId, alias: Option<&Alias>) -> Self {
        let alias = AliasKey::of(alias);

        let mut hash = KeyBuilder::new();
        hash.fixed(&ty);
        hash.alias(alias.as_ref());
        Self {
            ty,
            alias,
            fingerprint: hash.finish(),
        }
    }
}
impl CheckedKey for ActiveKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        self.ty == other.ty && same_alias(self.alias.as_ref(), other.alias.as_ref())
    }
}

/// Request-local caches of actual native questions. Declaration associations,
/// structured inventory and mapper substitutions are different questions;
/// none is keyed by a printed name or a display origin.
pub(super) struct GenericsState {
    reference_nodes: crate::resources::FixedMap<(FileId, Site), NativeResult<TypeId>>,
    reference_symbols:
        crate::resources::FixedMap<(FileId, Site), NativeResult<super::ReferenceSymbol>>,
    targets: crate::resources::FixedMap<TypeId, TargetAssociation>,
    declared_members: crate::resources::FixedMap<TypeId, MembersState>,
    bases: HashMap<TypeId, BaseTypesState>,
    base_constructors: crate::resources::FixedMap<TypeId, TypeState>,
    equivalent_bases: crate::resources::FixedMap<TypeId, NativeResult<Option<TypeId>>>,
    /// Native TypeParameter.mapper belongs to the fresh parameter identity.
    parameter_mappers: crate::resources::FixedMap<TypeId, MapperId>,
    instantiated_demands: crate::resources::FixedMap<(DemandId, MapperId), TypeDemand>,
    active_mappers: Vec<ActiveMapper>,
    instantiation_depth: usize,
    instantiation_count: usize,
}

impl GenericsState {
    /// Request-local backing for the existing native questions. Keys, values
    /// and provisional publication remain with their individual operations.
    pub(super) fn new() -> Self {
        Self {
            reference_nodes: crate::resources::FixedMap::new(),
            reference_symbols: crate::resources::FixedMap::new(),
            targets: crate::resources::FixedMap::new(),
            declared_members: crate::resources::FixedMap::new(),
            bases: Default::default(),
            base_constructors: crate::resources::FixedMap::new(),
            equivalent_bases: crate::resources::FixedMap::new(),
            parameter_mappers: crate::resources::FixedMap::new(),
            instantiated_demands: crate::resources::FixedMap::new(),
            active_mappers: Default::default(),
            instantiation_depth: Default::default(),
            instantiation_count: Default::default(),
        }
    }
    pub(super) fn has_base_constructor(&self, ty: TypeId) -> bool {
        self.base_constructors
            .get(&ty)
            .is_some_and(TypeState::has_type)
    }
    /// Native baseTypesResolved is distinct from an in-progress partial list.
    pub(super) fn has_base_types(&self, ty: TypeId) -> bool {
        matches!(self.bases.get(&ty), Some(BaseTypesState::Resolved(_)))
    }
}

#[derive(Clone)]
pub(super) enum GenericDemand {
    MappedOperand {
        ty: TypeId,
        operand: super::mapped::MappedOperand,
        location: Location,
    },
    ConditionalBranch {
        ty: TypeId,
        true_branch: bool,
        location: Location,
    },
    RawParameterConstraint(ParameterConstraintRecipe),
    ParameterConstraint {
        symbol: super::ReferenceSymbol,
        location: Location,
    },
    ParameterDefault(Box<TypeExpression>),
    ClonedParameterConstraint {
        clone: TypeId,
        location: Location,
    },
    Instantiated {
        original: TypeDemand,
        mapper: MapperId,
        location: Location,
    },
}

impl GenericDemand {
    fn location(&self) -> &Location {
        match self {
            Self::ConditionalBranch { location, .. } | Self::MappedOperand { location, .. } => {
                location
            }
            Self::RawParameterConstraint(recipe) => recipe.location(),
            Self::ParameterConstraint { location, .. } => location,
            Self::ParameterDefault(expression) => &expression.location,
            Self::ClonedParameterConstraint { location, .. } => location,
            Self::Instantiated { location, .. } => location,
        }
    }
}

impl Evaluator<'_, '_> {
    /// `getTypeOfPrototypeProperty` uses the real parent class and supplies
    /// Any for every regular type parameter, excluding the synthetic this.
    pub(super) fn prototype_property_type(
        &mut self,
        parent: BoundSymbol,
    ) -> Result<TypeId, DemandError> {
        let declaration = self
            .source
            .bindings()
            .declarations(parent)
            .into_iter()
            .next()
            .ok_or_else(|| super::super::internal("prototype parent lacks a class declaration"))?;
        let consumer = super::super::location(self.inputs, declaration.file, declaration.anchor);
        self.reference_prototype_property_type(parent.into(), &consumer)
    }

    pub(super) fn reference_prototype_property_type(
        &mut self,
        parent: super::ReferenceSymbol,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        let class_type = self.class_or_interface_type(parent, consumer)?;
        let target = self.model.declared_target(class_type)?;
        let count = target.outer_parameters.len() + target.local_parameters.len();
        if count == 0 {
            return Ok(class_type);
        }
        let any = self.model.special(SpecialName::Any);
        self.model.reference(class_type, vec![any; count])
    }

    /// `getTypeOfFuncClassEnumModuleWorker` / `getBaseTypeVariableOfClass`.
    /// The value cache belongs to the caller; this raw anonymous allocation
    /// must not become a successful value during circular base acquisition.
    pub(super) fn class_value_type(
        &mut self,
        symbol: BoundSymbol,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        let static_type = self.new_class_static_type(symbol, consumer)?;
        match self.base_type_variable_of_class(symbol.into(), consumer)? {
            Some(base) => self.intersection_type(vec![static_type, base], consumer),
            None => Ok(static_type),
        }
    }

    /// `getBaseTypeVariableOfClass` is shared by class value construction and
    /// native symbol display. The first TypeVariable constituent wins.
    pub(super) fn base_type_variable_of_class(
        &mut self,
        symbol: super::ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        let class_type = self.declared_reference_type(symbol, consumer)?;
        let base = self.base_constructor_type(class_type, consumer)?;
        Ok(
            if self.model.has_type_flags(base, TypeFlags::TYPE_VARIABLE) {
                Some(base)
            } else if let Some(types) = self.model.intersection_constituents(base) {
                types
                    .iter()
                    .copied()
                    .find(|&ty| self.model.has_type_flags(ty, TypeFlags::TYPE_VARIABLE))
            } else {
                None
            },
        )
    }

    /// `getBaseConstructorTypeOfClass`: this installed property is distinct
    /// from baseTypes and member resolution. Forced inventory precedes pop;
    /// constructor validation follows pop, preserving native re-entry order.
    pub(super) fn base_constructor_type(
        &mut self,
        class_type: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.base_constructor_type_completion(class_type, consumer)?
            .admit()
    }

    pub(super) fn base_constructor_type_completion(
        &mut self,
        class_type: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        match self.generics.base_constructors.get(&class_type) {
            Some(TypeState::Installed(value)) => return Ok(value.clone()),
            Some(TypeState::Failed(error)) => return Err(error.clone()),
            Some(TypeState::Resolving) | None => {}
        }
        let symbol = self
            .reference_symbol_of_type(class_type, consumer)?
            .ok_or_else(|| super::super::internal("class target lacks its actual symbol"))?;
        let recipe = self.reference_class_recipe(symbol, consumer)?;
        let Some(base) = recipe.base else {
            let value = NativeCompletion::clean(self.model.special(SpecialName::Undefined));
            self.generics
                .base_constructors
                .insert(class_type, TypeState::Installed(value.clone()));
            return Ok(value);
        };
        let inputs = self.inputs;
        let cycle = || {
            super::super::invalid_at_location(
                inputs,
                consumer,
                SemanticFailureReason::ErrorType,
                "class base-constructor cycle consumes native errorType recovery",
            )
            .into()
        };
        if !self.push_resolution(resolution::Key::BaseConstructorType(class_type)) {
            return NativeCompletion::recovered(self.model.native_error_type(), cycle());
        }
        let result = self
            .check_expression_completion(&base.expression, super::contextual::CheckMode::NORMAL)
            .and_then(|value| {
                value.and_then(|ty| {
                    if self
                        .model
                        .has_type_flags(ty, TypeFlags::OBJECT | TypeFlags::INTERSECTION)
                    {
                        return self
                            .structured_type_members_completion(ty, &base.expression.location)
                            .map(|members| members.map(|_| ty));
                    }
                    Ok(NativeCompletion::clean(ty))
                })
            });
        let resolved = self
            .resolution
            .pop()
            .ok_or_else(|| super::super::internal("base-constructor frame is missing"))?;
        let result = result.and_then(|value| {
            value.and_then(|ty| {
                if !resolved {
                    return self
                        .diagnostic_symbol_display_completion(symbol, consumer)?
                        .and_then(|_| {
                            NativeCompletion::recovered(self.model.native_error_type(), cycle())
                        });
                }
                if !self.model.has_type_flags(ty, TypeFlags::ANY)
                    && ty != self.model.null_initializer()
                {
                    return self
                        .is_constructor_type_completion(ty, &base.expression.location)?
                        .and_then(|valid| {
                            if !valid {
                                self.invalid_base_constructor_completion(
                                    ty,
                                    &base.expression.location,
                                )
                            } else {
                                Ok(NativeCompletion::clean(ty))
                            }
                        });
                }
                Ok(NativeCompletion::clean(ty))
            })
        });
        let result = result.map(
            |value| match self.generics.base_constructors.get(&class_type) {
                Some(TypeState::Installed(inner)) => value.map(|_| *inner.value()),
                _ => value,
            },
        );
        if !self
            .generics
            .base_constructors
            .get(&class_type)
            .is_some_and(TypeState::has_type)
        {
            self.generics.base_constructors.insert(
                class_type,
                match &result {
                    Ok(value) => TypeState::Installed(value.clone()),
                    Err(error) => TypeState::Failed(error.clone()),
                },
            );
        }
        result
    }

    /// `getBaseConstructorTypeOfClass` checks each diagnostic argument before
    /// installing errorType, including the TypeParameter related-info branch.
    fn invalid_base_constructor_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.check_diagnostic_type_display_completion(ty, consumer)?
            .and_then(|_| {
                let report = if self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER) {
                    let ctor_return =
                        self.raw_constraint_completion(ty)?.and_then(|constraint| {
                            let Some(constraint) = constraint else {
                                return Ok(NativeCompletion::clean(
                                    self.model.special(SpecialName::Unknown),
                                ));
                            };
                            self.construct_signatures_completion(constraint, consumer)?
                                .and_then(|signatures| {
                                    if let Some(signature) = signatures.first() {
                                        self.get_return_type_of_signature_completion(
                                            signature, consumer,
                                        )
                                    } else {
                                        Ok(NativeCompletion::clean(
                                            self.model.special(SpecialName::Unknown),
                                        ))
                                    }
                                })
                        })?;
                    ctor_return.and_then(|ctor_return| {
                        let symbol =
                            self.reference_symbol_of_type(ty, consumer)?
                                .ok_or_else(|| {
                                    super::super::internal(
                                        "base TypeParameter has no native symbol",
                                    )
                                })?;
                        if self
                            .reference_symbol_declarations(symbol, consumer)?
                            .is_empty()
                        {
                            return Ok(NativeCompletion::clean(()));
                        }
                        self.diagnostic_symbol_display_completion(symbol, consumer)?
                            .and_then(|_| {
                                self.check_diagnostic_type_display_completion(ctor_return, consumer)
                            })
                    })?
                } else {
                    NativeCompletion::clean(())
                };
                report.and_then(|_| {
                    NativeCompletion::recovered(
                        self.model.native_error_type(),
                        super::super::invalid_at_location(
                            self.inputs,
                            consumer,
                            SemanticFailureReason::ErrorType,
                            "class base is not a native constructor type",
                        )
                        .into(),
                    )
                })
            })
    }

    /// `isConstructorType` / `isMixinConstructorType` preserve the single
    /// signature, nongeneric, one-rest-parameter and exact Any element tests.
    pub(super) fn is_constructor_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if !sequence
                .read(self.construct_signatures_completion(ty, consumer))?
                .is_empty()
            {
                return Ok(true);
            }
            if !self.model.has_type_flags(ty, TypeFlags::TYPE_VARIABLE) {
                return Ok(false);
            }
            let Some(constraint) = sequence.read(self.base_constraint_completion(ty, consumer))?
            else {
                return Ok(false);
            };
            sequence.read(self.is_mixin_constructor_type_completion(constraint, consumer))
        })();
        sequence.finish(result)
    }

    /// `getDefaultConstructSignatures` owns argument acquisition and fresh
    /// instantiation selection; the shared signature owner owns each factory.
    pub(super) fn default_construct_signatures_completion(
        &mut self,
        class_type: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Signature>, DemandError> {
            let base =
                sequence.read(self.base_constructor_type_completion(class_type, consumer))?;
            let signatures = sequence.read(self.construct_signatures_completion(base, consumer))?;
            let symbol = self
                .reference_symbol_of_type(class_type, consumer)?
                .ok_or_else(|| {
                    super::super::internal("class default constructor lacks its symbol")
                })?;
            let recipe = self.reference_class_recipe(symbol, consumer)?;
            if signatures.is_empty() {
                return Ok(vec![self.fresh_default_constructor(
                    class_type,
                    recipe.abstract_,
                    consumer,
                )?]);
            }
            let base = recipe
                .base
                .ok_or_else(|| super::super::internal("base signatures lack an extends node"))?;
            let arguments = base
                .type_arguments
                .iter()
                .map(|argument| sequence.read(self.check_type_completion(argument)))
                .collect::<Result<Vec<_>, _>>()?;
            let mut result = Vec::new();
            for signature in signatures {
                let parameters = signature.type_parameter_types();
                let minimum = self.minimum_type_argument_count(parameters)?;
                if arguments.len() >= minimum && arguments.len() <= parameters.len() {
                    let arguments = if parameters.is_empty() {
                        None
                    } else {
                        Some(sequence.read(self.fill_missing_type_arguments_completion(
                            arguments.clone(),
                            parameters,
                            consumer,
                        ))?)
                    };
                    result.push(sequence.read(self.derive_default_constructor_completion(
                        &signature,
                        arguments,
                        class_type,
                        recipe.abstract_,
                        consumer,
                    ))?);
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `resolveBaseTypesOfClass` selects the direct class-reference path only
    /// with its actual constructor symbol and applied outer parameters. The
    /// class-like path filters constructors before acquiring written arguments.
    fn class_base_type_completion(
        &mut self,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Option<TypeId>, DemandError> {
            let constructor =
                sequence.read(self.base_constructor_type_completion(target, consumer))?;
            let constructor =
                sequence.read(self.apparent_type_completion(constructor, consumer))?;
            if !self.model.has_type_flags(
                constructor,
                TypeFlags::OBJECT | TypeFlags::INTERSECTION | TypeFlags::ANY,
            ) {
                return Ok(None);
            }
            let symbol = self
                .reference_symbol_of_type(target, consumer)?
                .ok_or_else(|| super::super::internal("class base target lacks its symbol"))?;
            let base = self
                .reference_class_recipe(symbol, consumer)?
                .base
                .ok_or_else(|| {
                    super::super::internal("class base constructor has no extends node")
                })?;
            let class_symbol = self
                .type_symbol_flags(constructor)?
                .is_some_and(|flags| flags.class);
            let original = if class_symbol {
                let symbol = self
                    .reference_symbol_of_type(constructor, &base.expression.location)?
                    .ok_or_else(|| {
                        super::super::internal("class constructor lost its actual symbol")
                    })?;
                Some(self.class_or_interface_type(symbol, &base.expression.location)?)
            } else {
                None
            };
            // Non-declared symbols (TypeLiteral/Function) make the native ignored
            // getDeclaredTypeOfSymbol question return errorType without child work.
            let direct = if let Some(original) = original {
                let view = self.model.declared_target(original)?;
                if let Some(&last) = view.outer_parameters.last() {
                    let TypeView::Reference(reference) = self.model.get(original)? else {
                        return Err(super::super::internal(
                            "generic class target lacks native arguments",
                        )
                        .into());
                    };
                    let arguments =
                        sequence.read(self.get_type_arguments_completion(reference.ty))?;
                    self.type_symbol_identity(last)?
                        != self.type_symbol_identity(arguments[view.outer_parameters.len() - 1])?
                } else {
                    true
                }
            } else {
                false
            };
            let base_type = if direct {
                sequence.read(self.class_reference_type_completion(
                    original.expect("direct class has its declared type"),
                    &base.type_arguments,
                    None,
                    &base.reference.location,
                ))?
            } else if self.model.has_type_flags(constructor, TypeFlags::ANY) {
                constructor
            } else {
                let instantiated = sequence.read(
                    self.instantiated_constructors_for_type_arguments_completion(
                        constructor,
                        &base.type_arguments,
                        &base.reference.location,
                    ),
                )?;
                let Some(signature) = instantiated.first() else {
                    return Ok(None);
                };
                sequence.read(self.get_return_type_of_signature_completion(
                    signature,
                    &base.expression.location,
                ))?
            };
            if self.model.is_error_type(base_type) {
                return Ok(None);
            }
            let reduced = sequence
                .read(self.reduced_type_completion(base_type, &base.expression.location))?;
            if !sequence
                .read(self.valid_base_type_completion(reduced, &base.expression.location))?
            {
                sequence.read(self.elaborate_never_intersection_completion(
                    base_type,
                    &base.expression.location,
                ))?;
                sequence.read(self.check_diagnostic_type_display_completion(
                    reduced,
                    &base.expression.location,
                ))?;
                return Ok(None);
            }
            if reduced == target
                || sequence.read(self.has_base_type_completion(
                    reduced,
                    Some(target),
                    &base.expression.location,
                ))?
            {
                let declaration = self
                    .reference_symbol_value_declaration(symbol, consumer)?
                    .ok_or_else(|| {
                        super::super::internal(
                            "class base diagnostic lacks its native ValueDeclaration",
                        )
                    })?;
                let location = self.reference_declaration_location(declaration)?;
                sequence.read(self.check_diagnostic_type_display_completion(target, &location))?;
                return Ok(None);
            }
            Ok(Some(reduced))
        })();
        sequence.finish(result)
    }

    /// `getConstructorsForTypeArguments` filters the native inventory by the
    /// actual minimum and maximum type-argument counts without instantiation.
    pub(super) fn constructors_for_type_arguments_completion(
        &mut self,
        ty: TypeId,
        count: usize,
        consumer: &Location,
    ) -> NativeResult<Vec<Signature>> {
        self.construct_signatures_completion(ty, consumer)?
            .and_then_value(|signatures| {
                let mut candidates = Vec::new();
                for signature in signatures {
                    if count
                        >= self.minimum_type_argument_count(signature.type_parameter_types())?
                        && count <= signature.type_parameter_types().len()
                    {
                        candidates.push(signature);
                    }
                }
                Ok(candidates)
            })
    }

    /// `getInstantiatedConstructorsForTypeArguments` first filters the native
    /// construct inventory, then acquire every written argument even if no
    /// constructor matched. Each surviving generic signature is instantiated
    /// through the shared cached signature factory.
    pub(super) fn instantiated_constructors_for_type_arguments_completion(
        &mut self,
        ty: TypeId,
        nodes: &[TypeExpression],
        consumer: &Location,
    ) -> NativeResult<Vec<Signature>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let candidates = sequence.read(self.constructors_for_type_arguments_completion(
                ty,
                nodes.len(),
                consumer,
            ))?;
            let arguments = nodes
                .iter()
                .map(|node| sequence.read(self.check_type_completion(node)))
                .collect::<Result<Vec<_>, _>>()?;
            let mut instantiated = Vec::with_capacity(candidates.len());
            for signature in candidates {
                instantiated.push(if signature.type_parameter_types().is_empty() {
                    signature
                } else {
                    sequence.read(self.get_signature_instantiation_completion(
                        &signature,
                        arguments.clone(),
                        consumer,
                    ))?
                });
            }
            Ok(instantiated)
        })();
        sequence.finish(result)
    }

    /// `getMinTypeArgumentCount` asks only actual default presence. It must
    /// neither demand the default nor consume an unused invalid default.
    pub(super) fn minimum_type_argument_count(
        &self,
        parameters: &[TypeId],
    ) -> Result<usize, DemandError> {
        let mut minimum = 0;
        for (index, &parameter) in parameters.iter().enumerate() {
            let TypeView::TypeParameter(parameter) = self.model.get(parameter)? else {
                return Err(super::super::internal(
                    "generic argument count has a non-parameter identity",
                )
                .into());
            };
            if parameter.default.is_none() {
                minimum = index + 1;
            }
        }
        Ok(minimum)
    }

    /// `getSingleBaseForNonAugmentingSubtype` installs the calculated fact
    /// before asking bases. Complete table presence includes reserved members;
    /// empty named-property output alone cannot establish nonaugmentation.
    pub(super) fn non_augmenting_reference_base_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let flags = self.model.object_flags(ty)?;
        if !flags.contains(ObjectFlags::REFERENCE) {
            return Ok(NativeCompletion::clean(None));
        }
        let Some(target) = self.model.reference_target(ty) else {
            return Ok(NativeCompletion::clean(None));
        };
        if !self
            .model
            .object_flags(target)?
            .intersects(ObjectFlags::CLASS | ObjectFlags::INTERFACE)
        {
            return Ok(NativeCompletion::clean(None));
        }
        if let Some(value) = self.generics.equivalent_bases.get(&ty) {
            return value.clone();
        }
        self.model.mark_identical_base_calculated(ty)?;
        self.generics
            .equivalent_bases
            .insert(ty, Ok(NativeCompletion::clean(None)));
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let association = self
                .generics
                .targets
                .get(&target)
                .ok_or_else(|| {
                    super::super::internal("reference base question has no target association")
                })?
                .clone();
            if association.bound.kind == BoundClassKind::Class {
                let class = self.reference_class_recipe(association.symbol, consumer)?;
                if class.base.is_some_and(|base| !base.simple_entity) {
                    return Ok(None);
                }
            }
            let bases = sequence.read(self.base_types_completion(target, consumer))?;
            let [base] = bases.as_slice() else {
                return Ok(None);
            };
            if sequence
                .read(self.reference_members_of_symbol_completion(association.symbol, consumer))?
                .is_some_and(|table| !table.is_empty())
            {
                return Ok(None);
            }
            let target = self.model.declared_target(target)?;
            let TypeView::Reference(_) = self.model.get(ty)? else {
                return Err(super::super::internal(
                    "native reference changed its established view",
                )
                .into());
            };
            let mut parameters = target.outer_parameters.to_vec();
            parameters.extend_from_slice(&target.local_parameters);
            let mut base = *base;
            if !parameters.is_empty() {
                let reference_arguments = sequence.read(self.get_type_arguments_completion(ty))?;
                let arguments = reference_arguments.get(..parameters.len()).ok_or_else(|| {
                    super::super::internal("reference lacks regular arguments for equivalent base")
                })?;
                let mapper = self.model.mapper(parameters.clone(), arguments.to_vec())?;
                base = sequence.read(self.instantiate_type_completion(base, mapper, consumer))?;
            }
            // Native reacquires arguments after instantiation, then again
            // only for the trailing this argument. Cache evidence follows
            // those separate reads; it is not a borrowed syntax vector.
            if sequence.read(self.get_type_arguments_completion(ty))?.len() > parameters.len() {
                let this_argument = sequence
                    .read(self.get_type_arguments_completion(ty))?
                    .last()
                    .copied();
                base = sequence.read(self.get_type_with_this_argument_completion(
                    base,
                    this_argument,
                    false,
                    consumer,
                ))?;
            }
            Ok(Some(base))
        })();
        let result = sequence.finish(result);
        self.generics.equivalent_bases.insert(ty, result.clone());
        result
    }

    /// `getBaseTypes` owns the native resolution key and its partial ordered
    /// list. A rejected push returns that list; diagnostics for invalid or
    /// recursive bases do not manufacture an error-type dependency.
    pub(super) fn base_types(
        &mut self,
        target: TypeId,
        consumer: &Location,
    ) -> Result<Vec<TypeId>, DemandError> {
        self.base_types_completion(target, consumer)?.admit()
    }

    pub(super) fn base_types_completion(
        &mut self,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        if !self
            .model
            .object_flags(target)?
            .intersects(ObjectFlags::CLASS | ObjectFlags::INTERFACE | ObjectFlags::TUPLE)
        {
            return Ok(NativeCompletion::clean(Vec::new()));
        }
        match self.generics.bases.get(&target) {
            Some(BaseTypesState::Resolved(bases)) => return Ok(bases.clone()),
            Some(BaseTypesState::Failed(error)) => return Err(error.clone()),
            Some(BaseTypesState::Resolving(_)) | None => {}
        }
        if !self.push_resolution(resolution::Key::BaseTypes(target)) {
            return Ok(NativeCompletion::clean(
                match self.generics.bases.get(&target) {
                    Some(BaseTypesState::Resolving(bases)) => bases.clone(),
                    _ => Vec::new(),
                },
            ));
        }
        self.generics
            .bases
            .entry(target)
            .or_insert_with(|| BaseTypesState::Resolving(Vec::new()));
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<(), DemandError> {
            if self.model.is_tuple_type(target) {
                let base = sequence.read(self.tuple_base_type_completion(target, consumer))?;
                self.generics
                    .bases
                    .insert(target, BaseTypesState::Resolving(vec![base]));
                return Ok(());
            }
            let association = self
                .generics
                .targets
                .get(&target)
                .ok_or_else(|| {
                    super::super::internal("base target has no canonical declaration association")
                })?
                .clone();
            if association.bound.kind == BoundClassKind::Class {
                if let Some(base) =
                    sequence.read(self.class_base_type_completion(target, consumer))?
                {
                    let Some(BaseTypesState::Resolving(bases)) =
                        self.generics.bases.get_mut(&target)
                    else {
                        return Err(super::super::internal(
                            "class base list lost its active operation",
                        )
                        .into());
                    };
                    *bases = vec![base];
                }
            }
            if association.bound.has_interface {
                for &declaration in &association.bound.declarations {
                    let Some(bases) = self.source.interface_bases(declaration)? else {
                        continue;
                    };
                    for base in bases {
                        let ty = sequence.read(self.check_type_completion(&base.expression))?;
                        let ty = sequence
                            .read(self.reduced_type_completion(ty, &base.expression.location))?;
                        // resolveBaseTypesOfInterface skips the actual errorType
                        // value while retaining this invocation's acquisitions.
                        if self.model.is_error_type(ty) {
                            continue;
                        }
                        if sequence
                            .read(self.valid_base_type_completion(ty, &base.expression.location))?
                        {
                            if ty != target
                                && !sequence.read(self.has_base_type_completion(
                                    ty,
                                    Some(target),
                                    &base.expression.location,
                                ))?
                            {
                                let Some(BaseTypesState::Resolving(bases)) =
                                    self.generics.bases.get_mut(&target)
                                else {
                                    return Err(super::super::internal(
                                        "base list lost its active native operation",
                                    )
                                    .into());
                                };
                                bases.push(ty);
                            } else {
                                let declaration = self.source.declaration_identity(declaration)?;
                                sequence.read(
                                    self.report_circular_base_type_completion(target, declaration),
                                )?;
                            }
                        }
                    }
                }
            }
            Ok(())
        })();
        let completed = self
            .resolution
            .pop()
            .ok_or_else(|| super::super::internal("base type resolution stack is unbalanced"))?;
        let result = (|| {
            // An interrupted invocation still unwinds its resolution frame,
            // but cannot acquire diagnostics or publish a completed base list.
            result?;
            if !completed && let Some(symbol) = self.reference_symbol_of_type(target, consumer)? {
                for declaration in self.reference_symbol_declarations(symbol, consumer)? {
                    let super::ReferenceDeclaration::Captured(declaration) = declaration else {
                        continue;
                    };
                    if matches!(
                        self.source.declaration_kind(declaration)?,
                        SyntaxKind::ClassDeclaration | SyntaxKind::InterfaceDeclaration
                    ) {
                        sequence
                            .read(self.report_circular_base_type_completion(target, declaration))?;
                    }
                }
            }
            // getBaseTypes clears members only after the false-pop diagnostic
            // pass. Display can reenter and complete the same partial list.
            self.model.clear_members_resolved(target)?;
            match self.generics.bases.get(&target) {
                Some(BaseTypesState::Resolving(bases)) => Ok(bases.clone()),
                Some(BaseTypesState::Resolved(bases)) => Ok(bases.value().clone()),
                _ => {
                    Err(super::super::internal("base list lost its active resolution state").into())
                }
            }
        })();
        let result = sequence.finish(result);
        self.generics.bases.insert(
            target,
            match &result {
                Ok(bases) => BaseTypesState::Resolved(bases.clone()),
                Err(error) => BaseTypesState::Failed(error.clone()),
            },
        );
        result
    }

    /// `reportCircularBaseType` eagerly acquires typeToStringEx with exactly
    /// WriteArrayAsGenericType and nil enclosing declaration. The inner class
    /// base diagnostic uses TypeToString's distinct default flags instead.
    fn report_circular_base_type_completion(
        &mut self,
        target: TypeId,
        declaration: super::super::source::DeclarationIdentity,
    ) -> NativeResult<()> {
        let consumer = self.source.declaration_location(declaration)?;
        Ok(self
            .diagnostic_type_display_completion(
                target,
                None,
                super::native_display::DiagnosticTypeFormat::WRITE_ARRAY_AS_GENERIC_TYPE,
                &consumer,
            )?
            .map(|_| ()))
    }

    /// `isValidBaseType`: constraints precede the object/Any/intersection
    /// tests. A diagnosed scalar base is skipped, not an error-type result.
    pub(super) fn valid_base_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            if self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER)
                && let Some(constraint) =
                    sequence.read(self.base_constraint_completion(ty, consumer))?
            {
                return sequence.read(self.valid_base_type_completion(constraint, consumer));
            }
            if self.model.has_type_flags(
                ty,
                TypeFlags::OBJECT | TypeFlags::NON_PRIMITIVE | TypeFlags::ANY,
            ) {
                return Ok(!sequence.read(self.is_generic_mapped_type_completion(ty, consumer))?);
            }
            if let Some(types) = self.model.intersection_constituents(ty) {
                for &ty in types.iter() {
                    if !sequence.read(self.valid_base_type_completion(ty, consumer))? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `hasBaseType` traverses canonical targets and intersection constituents
    /// through the shared base operation, including its native cycle window.
    pub(super) fn has_base_type(
        &mut self,
        ty: TypeId,
        check: Option<TypeId>,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.has_base_type_completion(ty, check, consumer)?.admit()
    }

    pub(super) fn has_base_type_completion(
        &mut self,
        ty: TypeId,
        check: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result =
            (|| -> Result<bool, DemandError> {
                if self.model.object_flags(ty)?.intersects(
                    ObjectFlags::CLASS | ObjectFlags::INTERFACE | ObjectFlags::REFERENCE,
                ) {
                    let target = self.model.reference_target(ty).unwrap_or(ty);
                    if Some(target) == check {
                        return Ok(true);
                    }
                    for base in sequence.read(self.base_types_completion(target, consumer))? {
                        if sequence.read(self.has_base_type_completion(base, check, consumer))? {
                            return Ok(true);
                        }
                    }
                } else if let Some(types) = self.model.intersection_constituents(ty) {
                    for &ty in types.iter() {
                        if sequence.read(self.has_base_type_completion(ty, check, consumer))? {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            })();
        sequence.finish(result)
    }

    /// `getTypeReferenceType` / `getTypeFromClassOrInterfaceReference` /
    /// `getMinTypeArgumentCount` / `fillMissingTypeArguments`: the declared
    /// target is acquired first, count uses default-presence facts without
    /// demanding defaults, and only a valid count checks arguments/defaults.
    /// Canonical parameter identities establish every substitution mapper.
    pub(super) fn type_reference_completion(
        &mut self,
        expression: &TypeExpression,
        site: Site,
        anchor: Option<Span>,
        arguments: &[TypeExpression],
        alias: Option<&AliasRecipe>,
        deferred: Option<&DeferredReferenceRecipe>,
    ) -> NativeResult<TypeId> {
        let key = (expression.file, site);
        if let Some(result) = self.generics.reference_nodes.get(&key) {
            return result.clone();
        }
        let result =
            self.type_reference_worker(expression, site, anchor, arguments, alias, deferred);
        self.generics.reference_nodes.insert(key, result.clone());
        result
    }

    /// `getSymbolFromTypeReference` has its own SymbolNodeLinks property, which
    /// survives a later interrupted type-argument acquisition.
    pub(super) fn symbol_from_type_reference(
        &mut self,
        expression: &TypeExpression,
        site: Site,
        anchor: Option<Span>,
    ) -> NativeResult<super::ReferenceSymbol> {
        let key = (expression.file, site);
        if let Some(result) = self.generics.reference_symbols.get(&key) {
            return result.clone();
        }
        let Some(anchor) = anchor else {
            // resolveTypeReferenceName returns unknownSymbol for native nil.
            // Symbol-only callers have not acquired its errorType yet.
            let result = Ok(NativeCompletion::clean(self.unknown_reference_symbol()));
            self.generics.reference_symbols.insert(key, result.clone());
            return result;
        };
        let result = self
            .reference_entity_at_completion(
                expression.file,
                anchor,
                true,
                crate::project::binding::NativeLookupMeaning::TYPE,
                false,
                &expression.location,
            )
            .and_then(|completion| {
                completion.and_then(|(resolved, missing)| {
                    if let Some(symbol) = resolved {
                        if self.is_unknown_reference_symbol(symbol) {
                            return NativeCompletion::recovered(
                                symbol,
                                self.generic_binding_failure(
                                    crate::project::binding::TypeBindingFailure::ErrorType(
                                        "native type alias target is unknown",
                                    ),
                                    &expression.location,
                                ),
                            );
                        }
                        return Ok(NativeCompletion::clean(symbol));
                    }
                    let error = missing.ok_or_else(|| {
                        super::super::internal("entity lookup missed without a failed component")
                    })?;
                    let root = super::super::source::ExpressionRoot {
                        file: expression.file,
                        site,
                    };
                    let symbol = match self.source.type_reference_name(root)? {
                        Some(name) => self.unresolved_symbol_for_entity_name(name)?,
                        None => self.unknown_reference_symbol(),
                    };
                    NativeCompletion::recovered(symbol, error)
                })
            });
        self.generics.reference_symbols.insert(key, result.clone());
        result
    }

    /// `getResolvedSymbolOrNil` observes only the installed native slot. It
    /// neither resolves the reference nor replays evidence from its acquisition.
    /// A stopped product operation has not installed a native symbol property.
    pub(super) fn resolved_type_reference_symbol(
        &self,
        root: super::super::source::ExpressionRoot,
    ) -> Option<super::ReferenceSymbol> {
        match self.generics.reference_symbols.get(&(root.file, root.site)) {
            Some(Ok(symbol)) => Some(*symbol.value()),
            Some(Err(_)) | None => None,
        }
    }

    fn type_reference_worker(
        &mut self,
        expression: &TypeExpression,
        site: Site,
        anchor: Option<Span>,
        arguments: &[TypeExpression],
        alias: Option<&AliasRecipe>,
        deferred: Option<&DeferredReferenceRecipe>,
    ) -> NativeResult<TypeId> {
        let consumer = &expression.location;
        self.symbol_from_type_reference(expression, site, anchor)?
            .and_then(|symbol| {
                if self.is_unknown_reference_symbol(symbol) {
                    if anchor.is_none() {
                        // getTypeReferenceType consumes unknownSymbol as errorType.
                        // Keep this evidence in the type cache, not the independent
                        // getSymbolFromTypeReference cache used by symbol probes.
                        return NativeCompletion::recovered(
                            self.model.native_error_type(),
                            super::super::invalid_at_location(
                                self.inputs,
                                consumer,
                                SemanticFailureReason::ErrorType,
                                "nonentity type reference consumes native errorType recovery",
                            )
                            .into(),
                        );
                    }
                    return Ok(NativeCompletion::clean(self.model.native_error_type()));
                }
                if let Some(property) = symbol.property()
                    && self.unresolved_alias_declared_type(property).is_some()
                {
                    return self
                        .type_arguments_from_nodes_completion(arguments)?
                        .and_then_value(|arguments| {
                            let alias =
                                self.unresolved_type_alias(property, arguments, consumer)?;
                            Ok(self.model.unresolved_alias_type(alias))
                        });
                }
                let declaration = self.declared_reference_family(symbol, consumer)?;
                self.declared_type_completion(symbol, declaration)?
                    .and_then(|ty| {
                        if matches!(declaration, BoundDeclaredType::Alias(_)) {
                            let anchor = anchor.ok_or_else(|| {
                                super::super::internal(
                                    "resolved type alias lacks native entity-name syntax",
                                )
                            })?;
                            return self.type_alias_reference_completion(
                                symbol,
                                ty,
                                expression,
                                anchor,
                                arguments,
                                alias,
                                site.kind == SyntaxKind::TypeReference,
                            );
                        }
                        if !matches!(declaration, BoundDeclaredType::ClassOrInterface(_)) {
                            return self.without_type_arguments_completion(ty, arguments, consumer);
                        }
                        self.class_reference_type_completion(ty, arguments, deferred, consumer)
                    })
            })
    }

    /// Native `core.Map(typeArguments, getTypeFromTypeNode)` checks every child
    /// after a completed recovery and stops only on an interrupted operation.
    pub(super) fn type_arguments_from_nodes_completion(
        &mut self,
        arguments: &[TypeExpression],
    ) -> NativeResult<Vec<TypeId>> {
        let mut result = NativeCompletion::clean(Vec::with_capacity(arguments.len()));
        for argument in arguments {
            result = result.and_then(|mut types| {
                Ok(self.check_type_completion(argument)?.map(|ty| {
                    types.push(ty);
                    types
                }))
            })?;
        }
        Ok(result)
    }

    /// `getTypeFromClassOrInterfaceReference` applies actual class/interface
    /// identity. An extends value alias need not have a source TYPE meaning.
    fn class_reference_type_completion(
        &mut self,
        ty: TypeId,
        arguments: &[TypeExpression],
        deferred: Option<&DeferredReferenceRecipe>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let TypeView::Reference(reference) = self.model.get(ty)? else {
            return self.without_type_arguments_completion(ty, arguments, consumer);
        };
        let target = self.model.declared_target(reference.target)?;
        if target.local_parameters.is_empty() {
            return self.without_type_arguments_completion(ty, arguments, consumer);
        }
        let minimum = self.minimum_type_argument_count(&target.local_parameters)?;
        if arguments.len() < minimum || arguments.len() > target.local_parameters.len() {
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                super::super::invalid_at_location(
                    self.inputs,
                    consumer,
                    SemanticFailureReason::ErrorType,
                    "generic type-reference argument count consumes native errorType recovery",
                )
                .into(),
            );
        }
        if let Some(recipe) = deferred
            && self.is_deferred_type_reference_node(
                recipe,
                arguments.len() != target.local_parameters.len(),
            )?
        {
            return self.create_deferred_reference_completion(
                target.target,
                recipe.clone(),
                arguments.to_vec(),
                true,
                None,
                None,
                consumer,
            );
        }
        self.type_arguments_from_nodes_completion(arguments)?
            .and_then(|actual| {
                self.fill_missing_type_arguments_completion(
                    actual,
                    &target.local_parameters,
                    consumer,
                )?
                .and_then_value(|actual| {
                    let mut all_arguments = target.outer_parameters.to_vec();
                    all_arguments.extend(actual);
                    self.model.reference_ex(
                        target.target,
                        all_arguments,
                        ObjectFlags::FROM_TYPE_NODE,
                    )
                })
            })
    }

    /// `fillMissingTypeArguments`: initialize every missing slot to native
    /// errorType before resolving defaults in order. Fully supplied arguments
    /// never consume defaults. TS-only input admission owns the absent JS
    /// implicit-any branch; default absence is native unknownType.
    pub(super) fn fill_missing_type_arguments_completion(
        &mut self,
        actual: Vec<TypeId>,
        parameters: &[TypeId],
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        if parameters.is_empty() {
            return Ok(NativeCompletion::clean(Vec::new()));
        }
        if actual.len() >= parameters.len() {
            return Ok(NativeCompletion::clean(actual));
        }
        let start = actual.len();
        let mut completed = NativeCompletion::clean(actual);
        for &parameter in &parameters[start..] {
            completed = completed.and_then(|mut actual| {
                let TypeView::TypeParameter(parameter) = self.model.get(parameter)? else {
                    return Err(super::super::internal(
                        "missing argument default has a non-parameter identity",
                    )
                    .into());
                };
                let default = if let Some(default) = parameter.default {
                    let default_consumer = match default {
                        TypeDemand::Known(_) => consumer.clone(),
                        TypeDemand::Deferred(id) => self.demands[id.index()]
                            .location
                            .clone()
                            .unwrap_or_else(|| consumer.clone()),
                    };
                    let default = self.demand_completion(default)?;
                    default.and_then(|default| {
                        let mut targets: Vec<_> =
                            actual.iter().copied().map(MapperTarget::Known).collect();
                        targets.resize(parameters.len(), MapperTarget::ErrorType);
                        let mapper = self.model.mapper_targets(parameters.to_vec(), targets)?;
                        self.instantiate_type_completion(default, mapper, &default_consumer)
                    })?
                } else {
                    // TS-only Project inputs: the JS implicit-any branch is owned
                    // by input admission and cannot silently change this fallback.
                    NativeCompletion::clean(self.model.special(SpecialName::Unknown))
                };
                Ok(default.map(|default| {
                    actual.push(default);
                    actual
                }))
            })?;
        }
        Ok(completed)
    }

    /// `getDefaultFromTypeParameter` consumes the canonical resolved-default
    /// question. Product failures remain failures rather than absent defaults.
    pub(super) fn default_from_type_parameter(
        &mut self,
        ty: TypeId,
    ) -> Result<Option<TypeId>, DemandError> {
        self.default_from_type_parameter_completion(ty)?.admit()
    }

    pub(super) fn default_from_type_parameter_completion(
        &mut self,
        ty: TypeId,
    ) -> NativeResult<Option<TypeId>> {
        let TypeView::TypeParameter(parameter) = self.model.get(ty)? else {
            return Err(
                super::super::internal("default question has a non-parameter identity").into(),
            );
        };
        match parameter.default {
            Some(default) => Ok(self.demand_completion(default)?.map(Some)),
            None => Ok(NativeCompletion::clean(None)),
        }
    }

    /// `checkNoTypeArguments` refuses count before any supplied child demand.
    pub(super) fn without_type_arguments_completion(
        &self,
        ty: TypeId,
        arguments: &[TypeExpression],
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if arguments.is_empty() {
            Ok(NativeCompletion::clean(ty))
        } else {
            NativeCompletion::recovered(
                self.model.native_error_type(),
                super::super::invalid_at_location(
                    self.inputs,
                    consumer,
                    SemanticFailureReason::ErrorType,
                    "nongeneric type reference consumes native errorType recovery",
                )
                .into(),
            )
        }
    }

    /// `getOuterTypeParameters`: ordered enclosing declarations share one
    /// acquisition owner. Ordinary declared parameters append uniquely;
    /// synthetic this parameters are appended at their native class frame.
    pub(super) fn outer_type_parameters(
        &mut self,
        declaration: impl Into<super::super::source::DeclarationReference>,
        include_this_types: bool,
        consumer: &Location,
    ) -> Result<Vec<TypeId>, DemandError> {
        self.outer_type_parameters_completion(declaration, include_this_types, consumer)?
            .admit()
    }

    pub(super) fn outer_type_parameters_completion(
        &mut self,
        declaration: impl Into<super::super::source::DeclarationReference>,
        include_this_types: bool,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut parameters = Vec::new();
            for recipe in self
                .source
                .outer_type_parameters(declaration.into(), include_this_types)?
            {
                let declaration = match recipe {
                    OuterParameterRecipe::Parameter(declaration) => {
                        parameters = sequence.read(
                            self.append_type_parameters_completion(parameters, &[declaration]),
                        )?;
                        continue;
                    }
                    OuterParameterRecipe::This(declaration) => declaration,
                    OuterParameterRecipe::Conditional(root) => {
                        parameters.extend(
                            sequence.read(self.infer_type_parameters_completion(root, consumer))?,
                        );
                        continue;
                    }
                    OuterParameterRecipe::Contextual {
                        declaration,
                        root,
                        parameters: declared,
                    } => {
                        if self.source.expression_is_context_sensitive(root)? {
                            let symbol = sequence.read(
                                self.reference_symbol_of_declaration_completion(declaration),
                            )?;
                            let ty = sequence
                                .read(self.value_reference_type_completion(symbol, consumer))?;
                            if let Some(signature) = sequence
                                .read(self.call_signatures_completion(ty, consumer))?
                                .first()
                                && !signature.type_parameter_types().is_empty()
                            {
                                parameters.extend_from_slice(signature.type_parameter_types());
                                continue;
                            }
                        }
                        parameters = sequence
                            .read(self.append_type_parameters_completion(parameters, &declared))?;
                        continue;
                    }
                };
                // `getDeclaredTypeOfClassOrInterface(getSymbolOfDeclaration(node)).thisType`.
                let symbol =
                    sequence.read(self.reference_symbol_of_declaration_completion(declaration))?;
                let ty =
                    sequence.read(self.declared_reference_type_completion(symbol, consumer))?;
                let target = self.model.reference_target(ty).ok_or_else(|| {
                    super::super::internal("outer this owner has no native target")
                })?;
                if let Some(this) = self.model.target_this_parameter(target)? {
                    parameters.push(this);
                }
            }
            Ok(parameters)
        })();
        sequence.finish(result)
    }

    /// `getDeclaredTypeOfClassOrInterface` publishes the actual target before
    /// acquiring local parameter identities. A later interruption is retained
    /// by the escaped model target, preserving the already published native
    /// identity/barrier without inventing the target's unfinished fields.
    pub(super) fn class_or_interface_type(
        &mut self,
        symbol: impl Into<super::ReferenceSymbol>,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        let symbol = symbol.into();
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        match self.declared_types.get(&identity) {
            Some(TypeState::Installed(value)) => return value.clone().admit(),
            Some(TypeState::Failed(error)) => {
                return Err(error.clone());
            }
            Some(TypeState::Resolving) | None => {}
        }
        let bound = self.class_or_interface_declaration_recipe(symbol, consumer)?;
        // `getDeclaredTypeOfClassOrInterface` publishes before the native
        // outer-parameter owner selects ValueDeclaration. This first retained
        // association supplies only diagnostic provenance, never that choice.
        let declaration = *bound.declarations.first().ok_or_else(|| {
            super::super::internal("canonical class/interface has no own declaration")
        })?;
        let kind = bound.kind;
        let consumer = super::super::location(self.inputs, declaration.file, declaration.anchor);
        let native_symbol = self.type_symbol_of_reference(symbol, &consumer)?;
        let name = self.symbol_name_as_written(symbol, &consumer)?;
        let target = self.model.begin_declared_target(TargetHeader {
            symbol: native_symbol,
            kind: match kind {
                BoundClassKind::Class => TargetKind::Class,
                BoundClassKind::Interface => TargetKind::Interface,
            },
            name,
        });
        self.declared_types.insert(
            identity,
            TypeState::Installed(NativeCompletion::clean(target)),
        );
        self.generics.targets.insert(
            target,
            TargetAssociation {
                symbol,
                bound,
                declaration,
            },
        );
        let result: Result<TypeId, DemandError> = (|| {
            let outer = self.outer_type_parameters_of_symbol(symbol, &consumer)?;
            let mut local = self
                .local_type_parameters_of_symbol_completion(symbol, &consumer)?
                .admit()?;
            local.retain(|parameter| !outer.contains(parameter));
            let bound = self
                .generics
                .targets
                .get(&target)
                .expect("registered generic target")
                .bound
                .clone();
            let needs_this = !outer.is_empty()
                || !local.is_empty()
                || kind == BoundClassKind::Class
                || !self.is_thisless_interface(&bound, &consumer)?;
            let completed = self
                .model
                .complete_declared_target(target, outer, local, needs_this)?;
            if let Some(this) = completed.this_parameter {
                self.constraints
                    .register_parameter(this, symbol, None, consumer.clone());
            }
            Ok(target)
        })();
        if let Err(error) = &result {
            self.model.fail_declared_target(target, error.clone());
        }
        result
    }

    /// `isThislessInterface`: complete ordered own interface flags are tested
    /// before each declaration's bases. The ignoreErrors lookup recognizes
    /// native unresolved absence, while unavailable retained facts still fail.
    fn is_thisless_interface(
        &mut self,
        bound: &BoundClassOrInterface,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        for &declaration in &bound.declarations {
            if self.inputs.bindings.declaration_syntax_kind(declaration)
                != crate::syntax_kind::SyntaxKind::InterfaceDeclaration
            {
                continue;
            }
            let contains_this = declaration.contains_this.ok_or_else(|| {
                super::super::internal(
                    "canonical interface has no finalized ContainsThis attachment",
                )
            })?;
            if contains_this {
                return Ok(false);
            }
            let bases = self.source.interface_bases(declaration)?.ok_or_else(|| {
                super::super::internal("interface base recipe changed its declaration family")
            })?;
            for base in bases {
                let Some(anchor) = base.entity_anchor else {
                    continue;
                };
                let (symbol, _) = self
                    .reference_entity_at_completion(
                        declaration.file,
                        anchor,
                        true,
                        crate::project::binding::NativeLookupMeaning::TYPE,
                        false,
                        &base.expression.location,
                    )?
                    .admit()?;
                let Some(symbol) = symbol else {
                    return Ok(false);
                };
                if !self.reference_symbol_flags(symbol, consumer)?.interface {
                    return Ok(false);
                }
                let ty = self.declared_reference_type(symbol, consumer)?;
                if self.model.target_this_parameter(ty)?.is_some() {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// `getTypeFromThisTypeNode` / `getThisType`: actual nearest-container
    /// facts decide eligibility. The selected named parent uses its canonical
    /// declared cache and real synthetic parameter; invalid contexts consume
    /// native errorType instead of a healthy any or a fresh parameter identity.
    pub(super) fn this_type(
        &mut self,
        expression: &TypeExpression,
        context: &ThisTypeContext,
    ) -> Result<TypeId, DemandError> {
        let inputs = self.inputs;
        let invalid = || {
            super::super::invalid_at_location(
                inputs,
                &expression.location,
                SemanticFailureReason::ErrorType,
                "this type consumes native unavailable-context errorType",
            )
            .into()
        };
        let Some(target) = context.target.filter(|_| {
            !context.static_member && (!context.constructor || context.in_constructor_body)
        }) else {
            return Err(invalid());
        };
        let symbol = self.reference_symbol_of_declaration(
            super::super::source::DeclarationIdentity::new(expression.file, target),
        )?;
        let flags = self
            .reference_symbol_native_flags(symbol, &expression.location)?
            .type_flags();
        if !(flags.class || flags.interface) {
            return Err(super::super::internal(
                "this parent symbol is not a canonical class/interface",
            )
            .into());
        }
        let ty = self
            .declared_reference_type_completion(symbol, &expression.location)?
            .admit()?;
        match self.model.get(ty)? {
            TypeView::Reference(reference) => self
                .model
                .declared_target(reference.target)?
                .this_parameter
                .ok_or_else(invalid),
            // The known ordinary interface compatibility representation has
            // proved no synthetic this; native Coalesce returns errorType.
            TypeView::Object(_) => Err(invalid()),
            _ => Err(super::super::internal("this declared parent has another type family").into()),
        }
    }

    pub(super) fn generic_binding_failure(
        &self,
        error: TypeBindingFailure,
        consumer: &Location,
    ) -> DemandError {
        super::super::binding_failure_at(self.inputs, consumer, error)
    }

    pub(in crate::project::semantic) fn members_at(
        &mut self,
        demand: MembersOfType,
        consumer: &Location,
    ) -> Result<MembersView, DemandError> {
        self.members_completion_at(demand, consumer)?.admit()
    }

    pub(in crate::project::semantic) fn members_completion_at(
        &mut self,
        demand: MembersOfType,
        consumer: &Location,
    ) -> NativeResult<MembersView> {
        let ty = demand.type_id();
        match self.structure.members.get(&ty) {
            Some(MembersState::Resolved(members))
                if self
                    .model
                    .object_flags(ty)?
                    .contains(ObjectFlags::MEMBERS_RESOLVED) =>
            {
                return Ok(members.clone());
            }
            Some(MembersState::Failed(error)) => return Err(error.clone()),
            // `resolveStructuredTypeMembers` reenters until MembersResolved.
            // Late-name checking can already observe the separately published
            // early symbol table while this type's inventory remains unset.
            Some(MembersState::Resolving(_) | MembersState::Resolved(_)) | None => {}
        }
        let view = self.model.get(ty)?;
        let previous = match self.structure.members.remove(&ty) {
            Some(MembersState::Resolved(previous)) => Some(previous),
            Some(MembersState::Resolving(previous)) => previous,
            _ => None,
        };
        self.structure
            .members
            .insert(ty, MembersState::Resolving(previous));
        let result = match view {
            TypeView::LazyObject(_) => {
                if self.model.object_flags(ty)?.intersects(ObjectFlags::MAPPED) {
                    self.mapped_members_completion(ty)
                } else {
                    self.anonymous_members_completion(ty, consumer)
                }
            }
            TypeView::Reference(reference) => {
                self.reference_members_completion(ty, reference.target, reference.is_reference)
            }
            _ => Err(super::super::internal(
                "deferred member question has no reference/anonymous target",
            )
            .into()),
        };
        // A stopped suffix cannot erase a table already published by its owner.
        if !matches!(
            self.structure.members.get(&ty),
            Some(MembersState::Resolved(_))
        ) {
            let previous = match self.structure.members.remove(&ty) {
                Some(MembersState::Resolving(previous)) => previous,
                _ => None,
            };
            self.structure.members.insert(
                ty,
                match (&result, previous) {
                    (Ok(members), _) => MembersState::Resolved(members.clone()),
                    (Err(_), Some(previous)) => MembersState::Resolved(previous),
                    (Err(error), None) => MembersState::Failed(error.clone()),
                },
            );
        }
        result
    }

    /// `resolveTypeReferenceMembers` acquires actual arguments before entering
    /// `resolveObjectTypeMembers`; a declared class/interface enters directly.
    fn reference_members_completion(
        &mut self,
        ty: TypeId,
        target_id: TypeId,
        is_reference: bool,
    ) -> NativeResult<MembersView> {
        let mut sequence = NativeSequence::new();
        let prepared = (|| {
            let target = self.model.declared_target(target_id)?;
            let mut arguments = if is_reference {
                sequence
                    .read(self.get_type_arguments_completion(ty))?
                    .to_vec()
            } else {
                Vec::new()
            };
            if is_reference
                && target.this_parameter.is_some()
                && arguments.len() + 1 == target.all_parameters.len()
            {
                arguments.push(ty);
            }
            let declared = sequence.read(self.declared_target_members_completion(target.target))?;
            let consumer = self.member_consumer(ty)?;
            let mapper = if !is_reference || target.all_parameters.as_ref() == arguments.as_slice()
            {
                None
            } else {
                Some(
                    self.model
                        .mapper(target.all_parameters.to_vec(), arguments.clone())?,
                )
            };
            let members = match mapper {
                Some(mapper) => sequence
                    .read(self.instantiate_members_completion(declared, mapper, &consumer))?,
                None => declared,
            };
            let table_present = if self.model.is_tuple_type(target.target) {
                true
            } else {
                let symbol = self
                    .generics
                    .targets
                    .get(&target.target)
                    .expect("established generic target")
                    .symbol;
                let table = sequence
                    .read(self.reference_members_of_symbol_completion(symbol, &consumer))?;
                if mapper.is_some() {
                    table.is_some_and(|table| !table.is_empty())
                } else {
                    table.is_some()
                }
            };
            let bases = sequence.read(self.base_types_completion(target.target, &consumer))?;
            Ok((members, arguments, mapper, table_present, bases, consumer))
        })();
        let prepared = sequence.finish(prepared)?;
        let (_, arguments, mapper, mut table_present, bases, consumer) = prepared.value().clone();
        let mut members = prepared.map(|(members, ..)| members);
        if !bases.is_empty() {
            let table = table_present.then(|| members.value().properties.clone());
            members = self.set_structured_type_members_completion(ty, table, members, &consumer)?;
            let table_shared = table_present;
            self.model.set_unresolved_members(ty, true)?;
            let result = (|| -> NativeResult<MembersView> {
                for base in bases {
                    // Keep the full invocation completion through each base.
                    // The next raw-table mutation occurs after all earlier base
                    // signature/index acquisitions, even before final fields.
                    let prepared = members.and_then(|own| {
                        let mut sequence = NativeSequence::new();
                        let result = (|| {
                            let mut base = base;
                            if let Some(this) = arguments.last().copied() {
                                if let Some(mapper) = mapper {
                                    base = sequence.read(
                                        self.instantiate_type_completion(base, mapper, &consumer),
                                    )?;
                                }
                                base =
                                    sequence.read(self.get_type_with_this_argument_completion(
                                        base,
                                        Some(this),
                                        false,
                                        &consumer,
                                    ))?;
                            }
                            Ok((own, base))
                        })();
                        sequence.finish(result)
                    })?;
                    let base = prepared.value().1;
                    members = self.inherit_members_completion(
                        table_shared.then_some(ty),
                        prepared.map(|(own, _)| own),
                        base,
                        &consumer,
                    )?;
                    table_present |= !members.value().properties.is_empty();
                }
                Ok(members)
            })();
            self.model.set_unresolved_members(ty, false)?;
            members = result?;
        }
        let table = table_present.then(|| members.value().properties.clone());
        self.set_structured_type_members_completion(ty, table, members, &consumer)
    }

    /// `resolveObjectTypeMembers` publishes a shared inherited raw table
    /// after properties, retaining the invocation prefix before signatures.
    fn inherit_members_completion(
        &mut self,
        shared_table: Option<TypeId>,
        own: NativeCompletion<MembersView>,
        base: TypeId,
        consumer: &Location,
    ) -> NativeResult<MembersView> {
        let properties = own.and_then(|mut own| {
            self.properties_of_type_completion(base, consumer)?
                .and_then_value(|properties| {
                    own.properties = self
                        .inherit_properties(own.properties.to_vec(), &properties, consumer)?
                        .into();
                    Ok(own)
                })
        })?;
        if let Some(ty) = shared_table {
            let prior = match self.structure.members.get(&ty) {
                Some(MembersState::Resolved(installed)) => installed.value().clone(),
                _ => {
                    return Err(super::super::internal(
                        "shared inherited table lacks its installed inventory",
                    )
                    .into());
                }
            };
            self.structure.member_tables.insert(
                ty,
                Some(super::structure::MemberTable::Projected(
                    properties.value().properties.clone(),
                )),
            );
            self.structure.members.insert(
                ty,
                MembersState::Resolved(properties.clone().map(|_| prior)),
            );
        }
        properties.and_then(|mut own| {
            let mut sequence = NativeSequence::new();
            let result = (|| {
                let calls =
                    sequence.read(self.signatures_of_type_completion(base, false, consumer))?;
                own.call_signatures = concatenate_signatures(own.call_signatures, calls);
                let constructs =
                    sequence.read(self.signatures_of_type_completion(base, true, consumer))?;
                own.construct_signatures =
                    concatenate_signatures(own.construct_signatures, constructs);
                let mut indexes = own.index_signatures.to_vec();
                let own_keys = indexes
                    .iter()
                    .map(|index| self.model.established_type(index.key_type))
                    .collect::<Result<Vec<_>, _>>()?;
                let inherited_indexes = if base == self.model.special(SpecialName::Any) {
                    vec![self.model.any_base_index()]
                } else {
                    sequence.read(self.index_infos_of_type_completion(base, consumer))?
                };
                for index in &inherited_indexes {
                    let key = self.model.established_type(index.key_type)?;
                    if !own_keys.contains(&key) {
                        indexes.push(index.clone());
                    }
                }
                own.index_signatures = indexes.into();
                Ok(own)
            })();
            sequence.finish(result)
        })
    }

    /// `addInheritedMembers`: static raw and projected instance tables share
    /// private-static exclusion, exact keys and replacement of non-Value slots.
    /// Nil remains nil until an inherited value actually allocates the table.
    pub(super) fn inherit_member_table(
        &mut self,
        mut table: Option<
            Vec<(
                crate::project::binding::NativePropertyKey,
                super::super::model::PropertySymbolId,
            )>,
        >,
        inherited: &[Property],
        consumer: &Location,
    ) -> Result<
        Option<
            Vec<(
                crate::project::binding::NativePropertyKey,
                super::super::model::PropertySymbolId,
            )>,
        >,
        DemandError,
    > {
        for property in inherited {
            let symbol = property.symbol.ok_or_else(|| {
                super::super::internal("inherited native property lacks its symbol")
            })?;
            if self.is_static_private_identifier_property(symbol, consumer)? {
                continue;
            }
            let key =
                self.reference_symbol_key(super::ReferenceSymbol::from_property(symbol), consumer)?;
            let existing = table
                .as_ref()
                .and_then(|table| table.iter().position(|(own, _)| own == &key));
            if let Some(index) = existing {
                let table = table.as_mut().expect("entry belongs to a present table");
                if !self
                    .reference_symbol_flags(
                        super::ReferenceSymbol::from_property(table[index].1),
                        consumer,
                    )?
                    .value
                {
                    table[index].1 = symbol;
                }
            } else {
                table.get_or_insert_with(Vec::new).push((key, symbol));
            }
        }
        Ok(table)
    }

    fn inherit_properties(
        &mut self,
        properties: Vec<Property>,
        inherited: &[Property],
        consumer: &Location,
    ) -> Result<Vec<Property>, DemandError> {
        let mut table = Vec::new();
        for property in &properties {
            let symbol = property
                .symbol
                .ok_or_else(|| super::super::internal("own native property lacks its symbol"))?;
            table.push((
                self.reference_symbol_key(super::ReferenceSymbol::from_property(symbol), consumer)?,
                symbol,
            ));
        }
        let table = self
            .inherit_member_table(Some(table), inherited, consumer)?
            .expect("present own table remains present");
        table
            .into_iter()
            .map(|(_, symbol)| {
                properties
                    .iter()
                    .chain(inherited)
                    .find(|property| property.symbol == Some(symbol))
                    .cloned()
                    .ok_or_else(|| {
                        super::super::internal(
                            "inherited table symbol lacks its property descriptor",
                        )
                        .into()
                    })
            })
            .collect()
    }

    fn declared_target_members_completion(&mut self, target: TypeId) -> NativeResult<MembersView> {
        if let Some(tuple) = self.tuples.associations.get(&target) {
            return Ok(NativeCompletion::clean(tuple.members.clone()));
        }
        match self.generics.declared_members.get(&target) {
            Some(MembersState::Resolved(members)) => return Ok(members.clone()),
            Some(MembersState::Failed(error)) => return Err(error.clone()),
            Some(MembersState::Resolving(_)) => {
                return self
                    .pending_members(
                        target,
                        "recursive native declared-member inventory is pending",
                    )
                    .map(NativeCompletion::clean);
            }
            None => {}
        }
        let association = self.generics.targets.get(&target).cloned().ok_or_else(|| {
            super::super::internal("declared target has no canonical binding association")
        })?;
        let consumer = super::super::location(
            self.inputs,
            association.declaration.file,
            association.declaration.anchor,
        );
        let result = self
            .symbol_type_members_completion(association.symbol, Some(target), &consumer)
            .map(|value| value.map(|(_, members)| members));
        if !matches!(
            self.generics.declared_members.get(&target),
            Some(MembersState::Resolved(_))
        ) {
            if let Err(error) = &result {
                self.generics
                    .declared_members
                    .insert(target, MembersState::Failed(error.clone()));
            }
        }
        result
    }

    /// `resolveDeclaredMembers` and TypeLiteral `resolveAnonymousTypeMembers`
    /// share table, signature and index acquisitions. Only declared targets
    /// publish their declaredMembers slot before acquiring signatures.
    pub(super) fn symbol_type_members_completion(
        &mut self,
        symbol: super::ReferenceSymbol,
        target: Option<TypeId>,
        consumer: &Location,
    ) -> NativeResult<(
        Option<super::reference_symbols::ReferenceSymbolTable>,
        MembersView,
    )> {
        let table = self.reference_members_of_symbol_completion(symbol, consumer)?;
        let raw = table.value().clone();
        let mut members = table.and_then(|table| {
            let mut sequence = NativeSequence::new();
            let result = (|| {
                let mut properties = Vec::new();
                if let Some(table) = table {
                    for (key, symbol) in table.iter() {
                        if sequence.read(
                            self.reference_is_named_member_completion(*symbol, key, consumer),
                        )? {
                            let symbol = self.register_reference_symbol(*symbol, consumer)?;
                            properties.push(self.project_property_symbol(symbol, consumer)?);
                        }
                    }
                }
                Ok(MembersView {
                    properties: properties.into(),
                    call_signatures: std::sync::Arc::from([]),
                    construct_signatures: std::sync::Arc::from([]),
                    index_signatures: std::sync::Arc::from([]),
                })
            })();
            sequence.finish(result)
        })?;
        if let Some(target) = target {
            self.generics
                .declared_members
                .insert(target, MembersState::Resolved(members.clone()));
        }
        for construct in [false, true] {
            let key: &[u8] = if construct { b"\xfenew" } else { b"\xfecall" };
            let signature = raw.as_ref().and_then(|table| {
                table
                    .iter()
                    .find(|(name, _)| name.fixed_bytes() == Some(key))
                    .map(|(_, symbol)| *symbol)
            });
            members = members.and_then(|mut members| {
                Ok(self
                    .signatures_of_symbol_completion(signature, consumer)?
                    .map(|signatures| {
                        if construct {
                            members.construct_signatures = signatures.into();
                        } else {
                            members.call_signatures = signatures.into();
                        }
                        members
                    }))
            })?;
            if let Some(target) = target {
                self.generics
                    .declared_members
                    .insert(target, MembersState::Resolved(members.clone()));
            }
        }
        members = members.and_then(|mut members| {
            Ok(self
                .index_infos_of_symbol_completion(symbol, consumer)?
                .map(|indexes| {
                    members.index_signatures = indexes.into();
                    members
                }))
        })?;
        if let Some(target) = target {
            self.generics
                .declared_members
                .insert(target, MembersState::Resolved(members.clone()));
        }
        Ok(members.map(|members| (raw, members)))
    }

    pub(super) fn member_consumer(&self, ty: TypeId) -> Result<Location, DemandError> {
        let reference = match self.model.get(ty)? {
            TypeView::LazyObject(_) => return self.anonymous_consumer(ty),
            TypeView::Reference(reference) => reference,
            _ => {
                return Err(super::super::internal(
                    "native inventory has no reference/anonymous attachment",
                )
                .into());
            }
        };
        if let Some(tuple) = self.tuples.associations.get(&reference.target) {
            return Ok(tuple.consumer.clone());
        }
        let association = self
            .generics
            .targets
            .get(&reference.target)
            .ok_or_else(|| {
                super::super::internal("native inventory target has no binding association")
            })?;
        Ok(super::super::location(
            self.inputs,
            association.declaration.file,
            association.declaration.anchor,
        ))
    }

    fn pending_members(
        &self,
        ty: TypeId,
        message: &'static str,
    ) -> Result<MembersView, DemandError> {
        Err(super::super::unsupported_demand_at_location(
            self.inputs,
            &self.member_consumer(ty)?,
            SemanticFailureReason::UnsupportedSemanticForm,
            message,
        ))
    }

    /// Explicit declaration branch of `getIndexInfosOfIndexSymbol`: acquire
    /// value before key, flatten keys in order, then validate and deduplicate.
    /// Late-bound declarations are handled by the source index-symbol producer.
    pub(super) fn index_infos_from_recipes_completion(
        &mut self,
        recipes: &[super::super::source::IndexSignatureRecipe],
    ) -> NativeResult<Vec<Index>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut indexes = Vec::new();
            let mut keys = Vec::new();
            for index in recipes {
                let value = match &index.value_type {
                    Some(value) => sequence.read(self.check_type_completion(value))?,
                    None => self.model.special(SpecialName::Any),
                };
                let key = sequence.read(self.check_type_completion(&index.key_type))?;
                let mut pending = vec![key];
                while let Some(key) = pending.pop() {
                    if let TypeView::Union(types) = self.model.get(key)? {
                        pending.extend(types.iter().rev().copied());
                        continue;
                    }
                    if !sequence.read(
                        self.is_valid_index_key_type_completion(key, &index.key_type.location),
                    )? || keys.contains(&key)
                    {
                        continue;
                    }
                    keys.push(key);
                    indexes.push(self.model.index_info(
                        key,
                        value,
                        index.readonly,
                        Some((index.declaration, index.parameter_name.clone())),
                        None,
                    ));
                }
            }
            Ok(indexes)
        })();
        sequence.finish(result)
    }

    pub(super) fn instantiate_members_completion(
        &mut self,
        mut members: MembersView,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<MembersView> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<MembersView, DemandError> {
            let properties = sequence.read(self.instantiate_properties_completion(
                members.properties.to_vec(),
                mapper,
                consumer,
            ))?;
            members.call_signatures = members
                .call_signatures
                .iter()
                .map(|signature| {
                    sequence
                        .read(self.instantiate_signature_completion(signature, mapper, consumer))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into();
            members.construct_signatures = members
                .construct_signatures
                .iter()
                .map(|signature| {
                    sequence
                        .read(self.instantiate_signature_completion(signature, mapper, consumer))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into();
            let indexes = members
                .index_signatures
                .iter()
                .map(|index| {
                    sequence.read(self.instantiate_index_info_completion(index, mapper, consumer))
                })
                .collect::<Result<Vec<_>, _>>()?;
            members.properties = properties.into();
            members.index_signatures = indexes.into();
            Ok(members)
        })();
        sequence.finish(result)
    }

    /// `createInstantiatedSymbolTable` maps actual property symbols in native
    /// inventory order without acquiring their value types.
    pub(super) fn instantiate_properties_completion(
        &mut self,
        properties: Vec<Property>,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<Vec<Property>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut properties = properties;
            for property in &mut properties {
                if let Some(original) = property.symbol {
                    let symbol = sequence.read(
                        self.instantiate_property_symbol_completion(original, mapper, consumer),
                    )?;
                    property.symbol = Some(symbol);
                    if symbol == original {
                        continue;
                    }
                    property.read_type =
                        self.register_property_symbol_demand(symbol, false, consumer);
                    property.write_type = property
                        .write_type
                        .map(|_| self.register_property_symbol_demand(symbol, true, consumer));
                    if let PropertyDisplayDemand::Native(_) = property.display {
                        property.display = PropertyDisplayDemand::Native(
                            self.register_property_serialization(symbol, consumer),
                        );
                    }
                    continue;
                }
                property.read_type = sequence.read(self.instantiate_demand_completion(
                    property.read_type,
                    mapper,
                    consumer,
                ))?;
                match property.display {
                    PropertyDisplayDemand::Value(value) => {
                        property.write_type = property
                            .write_type
                            .map(|write| {
                                sequence.read(
                                    self.instantiate_demand_completion(write, mapper, consumer),
                                )
                            })
                            .transpose()?;
                        property.display =
                            PropertyDisplayDemand::Value(sequence.read(
                                self.instantiate_demand_completion(value, mapper, consumer),
                            )?);
                    }
                    PropertyDisplayDemand::Native(_) => {
                        return Err(super::super::internal(
                            "native property display lacks its actual symbol",
                        )
                        .into());
                    }
                };
            }
            Ok(properties)
        })();
        sequence.finish(result)
    }

    /// `instantiateIndexInfo` shares the original record when substitution
    /// leaves its value unchanged; otherwise only the value changes in the
    /// newly allocated record. Keys and actual declarations are never mapped.
    pub(super) fn instantiate_index_info_completion(
        &mut self,
        index: &Index,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<Index> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Index, DemandError> {
            let old_value = self.model.established_type(index.value_type)?;
            let value =
                sequence.read(self.instantiate_type_completion(old_value, mapper, consumer))?;
            if value == old_value {
                return Ok(index.clone());
            }
            Ok(self.model.index_info(
                self.model.established_type(index.key_type)?,
                value,
                index.readonly,
                index
                    .declaration
                    .map(|declaration| (declaration, index.parameter_name.clone())),
                index.components.clone(),
            ))
        })();
        sequence.finish(result)
    }

    /// `instantiateSignatureEx` allocates all local parameter identities before
    /// composing their mapper. Every invocation allocates a new signature;
    /// repeated member inventories share their resolved result, not an invented
    /// signature interner. Constraint/default/return types remain lazy native
    /// questions on the fresh identities.
    pub(super) fn instantiate_signature(
        &mut self,
        signature: &Signature,
        outer: MapperId,
        consumer: &Location,
    ) -> Result<Signature, DemandError> {
        self.instantiate_signature_completion(signature, outer, consumer)?
            .admit()
    }

    pub(super) fn instantiate_signature_completion(
        &mut self,
        signature: &Signature,
        outer: MapperId,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        // Native instantiateSignature erases only for this actual singleton
        // mapper; equal mapped results do not establish the same policy.
        let erase = outer == self.model.permissive_mapper();
        self.instantiate_signature_ex_completion(signature, outer, erase, consumer)
    }

    /// `getTypeParametersForMapper` applies each actual parameter's own
    /// mapper. Instantiated signature origins do not identify its parameters.
    pub(super) fn get_type_parameters_for_mapper_completion(
        &mut self,
        signature: &Signature,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = signature
            .type_parameter_types()
            .iter()
            .map(
                |&parameter| match self.generics.parameter_mappers.get(&parameter).copied() {
                    Some(mapper) => {
                        sequence.read(self.instantiate_type_completion(parameter, mapper, consumer))
                    }
                    None => Ok(parameter),
                },
            )
            .collect();
        sequence.finish(result)
    }

    /// `cloneTypeParameter` retains its target and mapper. Constraint/default
    /// acquisition remains lazy for both signature locals and mapped keys.
    pub(super) fn initialize_parameter_clone(
        &mut self,
        clone: TypeId,
        target: TypeId,
        mapper: MapperId,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        self.generics.parameter_mappers.insert(clone, mapper);
        let TypeView::TypeParameter(parameter) = self.model.get(target)? else {
            return Err(super::super::internal("signature local has no parameter view").into());
        };
        let constraint = parameter.constraint.map(|constraint| {
            let location = self.demand_location(constraint, consumer);
            self.register_generic_demand(GenericDemand::ClonedParameterConstraint {
                clone,
                location,
            })
        });
        let default = parameter
            .default
            .map(|default| self.fresh_instantiated_demand(default, mapper, consumer));
        self.register_parameter_clone(clone, target, Some(mapper))?;
        self.model
            .complete_parameter_clone(clone, constraint, default)?;
        Ok(())
    }

    /// `instantiateSignatureEx` keeps erasure independent of the mapper:
    /// member mapping clones locals, while call instantiation erases them.
    /// Every result remains a fresh actual SignatureId with its own return
    /// property and inherited native declaration association.
    pub(super) fn instantiate_signature_ex_completion(
        &mut self,
        signature: &Signature,
        outer: MapperId,
        erase: bool,
        consumer: &Location,
    ) -> NativeResult<Signature> {
        let mut clones = Vec::with_capacity(signature.type_parameter_types().len());
        let mut sequence = NativeSequence::new();
        let result: Result<Signature, DemandError> = (|| {
            if !erase {
                for &parameter in signature.type_parameter_types().iter() {
                    clones.push(self.model.clone_parameter(parameter)?);
                }
            }
            let mapper = if clones.is_empty() {
                outer
            } else {
                let local = self
                    .model
                    .mapper(signature.type_parameter_types().to_vec(), clones.clone())?;
                self.model.combine_mappers(local, outer)
            };
            for (&clone, &target) in clones.iter().zip(signature.type_parameter_types().iter()) {
                self.initialize_parameter_clone(clone, target, mapper, consumer)?;
            }
            let type_parameters = clones
                .iter()
                .map(|&clone| match self.model.get(clone)? {
                    TypeView::TypeParameter(parameter) => Ok(parameter),
                    _ => Err(super::super::internal(
                        "fresh signature local has another type family",
                    )
                    .into()),
                })
                .collect::<Result<Vec<_>, DemandError>>()?;
            let this_parameter = match signature.this_parameter() {
                Some(parameter) => Some(sequence.read(
                    self.instantiate_parameter_value_completion(parameter, mapper, consumer),
                )?),
                None => None,
            };
            let mut parameters = Vec::with_capacity(signature.parameters.len());
            for parameter in signature.parameters.iter() {
                parameters.push(sequence.read(
                    self.instantiate_parameter_value_completion(parameter, mapper, consumer),
                )?);
            }
            let mut predicate = signature.predicate.clone();
            if let Some(predicate) = &mut predicate {
                predicate.value_type = predicate
                    .value_type
                    .map(|asserted| self.fresh_instantiated_demand(asserted, mapper, consumer));
            }
            let return_type =
                self.fresh_instantiated_demand(signature.return_type, mapper, consumer);
            let instantiated = self.model.signature(SignatureData {
                type_parameter_types: clones.clone().into(),
                type_parameters,
                this_parameter,
                parameters: parameters.into(),
                return_type,
                predicate,
                origins: signature.origins.clone().into(),
                minimum_arguments: signature.minimum_arguments,
                flags: signature.flags.propagating(),
            })?;
            self.attach_signature_instantiation(&instantiated, signature, mapper)?;
            // A derived computation needs its current mapper-operation context;
            // this does not locate the original startup recovery. That failure
            // remains unbound through the generic demand and cache publication.
            let location = self
                .signature_return_location(signature.id)?
                .cloned()
                .unwrap_or_else(|| consumer.clone());
            self.attach_signature_return(&instantiated, location)?;
            Ok(instantiated)
        })();
        let result = sequence.finish(result);
        if let Err(error) = &result {
            for clone in clones {
                self.model.fail_parameter_clone(clone, error.clone())?;
            }
        }
        result
    }

    /// `getTypeWithThisArgument`: only a reference with exactly the real
    /// outer/local argument count gains the actual this argument. Scalar and
    /// union types stay unchanged; apparent conversion is a separate native
    /// question delegated to its existing owner.
    pub(super) fn get_type_with_this_argument(
        &mut self,
        ty: TypeId,
        this_argument: Option<TypeId>,
        need_apparent: bool,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.get_type_with_this_argument_completion(ty, this_argument, need_apparent, consumer)?
            .admit()
    }

    pub(super) fn get_type_with_this_argument_completion(
        &mut self,
        ty: TypeId,
        this_argument: Option<TypeId>,
        need_apparent: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            if let Some(types) = self.model.intersection_constituents(ty) {
                let mapped = types
                    .iter()
                    .map(|&ty| {
                        sequence.read(self.get_type_with_this_argument_completion(
                            ty,
                            this_argument,
                            need_apparent,
                            consumer,
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                // `core.SameMap` preserves the original intersection (including
                // its alias/origin) when no child changed. Reconstructing even
                // an equal list would ask normalization questions native skips.
                if mapped.as_slice() == types.as_ref() {
                    return Ok(ty);
                }
                return sequence.read(self.intersection_type_completion(mapped, consumer));
            }
            if let TypeView::Reference(reference) = self.model.get(ty)? {
                if !reference.is_reference {
                    return if need_apparent {
                        sequence.read(self.apparent_type_completion(ty, consumer))
                    } else {
                        Ok(ty)
                    };
                }
                let target = self.model.declared_target(reference.target)?;
                let count = target.outer_parameters.len() + target.local_parameters.len();
                let arguments = sequence.read(self.get_type_arguments_completion(ty))?;
                if arguments.len() == count {
                    let this_argument =
                        this_argument.or(target.this_parameter).ok_or_else(|| {
                            super::super::internal("reference target has no native this parameter")
                        })?;
                    let mut arguments = arguments.to_vec();
                    arguments.push(this_argument);
                    return self.model.reference(reference.target, arguments);
                }
                return Ok(ty);
            }
            if need_apparent {
                sequence.read(self.apparent_type_completion(ty, consumer))
            } else {
                Ok(ty)
            }
        })();
        sequence.finish(result)
    }

    fn instantiate_parameter_value_completion(
        &mut self,
        parameter: &ParameterValue,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<ParameterValue> {
        Ok(self
            .instantiate_property_symbol_completion(parameter.symbol, mapper, consumer)?
            .map(|symbol| {
                if symbol == parameter.symbol {
                    parameter.clone()
                } else {
                    self.parameter_value_with_symbol(parameter, symbol, consumer)
                }
            }))
    }

    fn demand_location(&self, demand: TypeDemand, consumer: &Location) -> Location {
        match demand {
            TypeDemand::Known(_) => consumer.clone(),
            TypeDemand::Deferred(id) => self.demands[id.index()]
                .location
                .clone()
                .unwrap_or_else(|| consumer.clone()),
        }
    }

    /// Signature result properties belong to the actual new SignatureId. This
    /// fresh lazy question intentionally bypasses the scalar substitution cache.
    fn fresh_instantiated_demand(
        &mut self,
        original: TypeDemand,
        mapper: MapperId,
        consumer: &Location,
    ) -> TypeDemand {
        let location = self.demand_location(original, consumer);
        self.register_generic_demand(GenericDemand::Instantiated {
            original,
            mapper,
            location,
        })
    }

    pub(super) fn instantiate_demand_completion(
        &mut self,
        original: TypeDemand,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<TypeDemand> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeDemand, DemandError> {
            match original {
                TypeDemand::Known(ty) => Ok(TypeDemand::Known(
                    sequence.read(self.instantiate_type_completion(ty, mapper, consumer))?,
                )),
                TypeDemand::Deferred(demand) => {
                    if let Some(&instantiated) =
                        self.generics.instantiated_demands.get(&(demand, mapper))
                    {
                        return Ok(instantiated);
                    }
                    let location = self.demand_location(original, consumer);
                    let instantiated = self.register_generic_demand(GenericDemand::Instantiated {
                        original,
                        mapper,
                        location,
                    });
                    self.generics
                        .instantiated_demands
                        .insert((demand, mapper), instantiated);
                    Ok(instantiated)
                }
            }
        })();
        sequence.finish(result)
    }

    /// `instantiateTypeWithAlias` owns containment, native budgets and active
    /// mapper frames. The common operation has no explicit alias override.
    pub(super) fn instantiate_type(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.instantiate_type_completion(ty, mapper, consumer)?
            .admit()
    }

    pub(super) fn instantiate_type_completion(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.instantiate_type_with_alias_completion(ty, mapper, None, consumer)
    }

    /// `instantiateTypeWithAlias`: containment belongs to this invocation's
    /// prefix. An active-mapper cache stores only the worker completion, so a
    /// hit cannot replay a previous containment acquisition as a new one.
    pub(super) fn instantiate_type_with_alias_completion(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let contains = (|| {
            if sequence.read(self.could_contain_type_variables_completion(ty, consumer))? {
                return Ok(true);
            }
            let old_alias = self.model.metadata(ty)?.alias;
            if let Some(arguments) = old_alias.and_then(|alias| alias.arguments.clone()) {
                for &argument in arguments.iter() {
                    if sequence
                        .read(self.could_contain_type_variables_completion(argument, consumer))?
                    {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(contains)?.and_then(|contains| {
            if !contains { return Ok(NativeCompletion::clean(ty)); }
            if self.generics.instantiation_depth == 100 || self.generics.instantiation_count >= 5_000_000 {
                return NativeCompletion::recovered(self.model.native_error_type(), super::super::invalid_at_location(
                    self.inputs, consumer, SemanticFailureReason::InstantiationLimit,
                    "native type instantiation depth or per-expression count limit was consumed",
                ).into());
            }
            let existing = self.generics.active_mappers.iter().rposition(|frame| {

                frame.mapper == mapper
            });
            let index = match existing {
                Some(index) => index,
                None => {
                    let index = self.generics.active_mappers.len();
                    self.generics.active_mappers.push(ActiveMapper { mapper, cache: CheckedMap::new() });
                    index
                }
            };
            let key = ActiveKey::new(ty, alias.as_ref());
            if let Some(cached) = self.generics.active_mappers[index].cache.get(&key) {
                return Ok(cached.clone());
            }
            self.generics.instantiation_count += 1;
            self.generics.instantiation_depth += 1;
            let result = self.instantiate_type_worker_completion(ty, mapper, alias, consumer);
            if existing.is_none() {
                self.pop_active_mapper();
            } else if let Ok(result) = &result {
                self.generics.active_mappers[index].cache.insert(key, result.clone());
            }
            self.generics.instantiation_depth -= 1;
            result
        })
    }

    /// `popActiveMapper` clears the retiring frame before releasing it. These
    /// checks belong to normal execution; request-abort unwinding never charges.
    fn pop_active_mapper(&mut self) {
        self.generics
            .active_mappers
            .last_mut()
            .expect("active mapper frame")
            .cache
            .clear();
        self.generics.active_mappers.pop();
    }

    /// `clearActiveMapperCaches` invalidates every live frame after an
    /// inference slot resolves. Committed ordinary factory caches survive.
    pub(super) fn clear_active_mapper_caches(&mut self) {
        for frame in &mut self.generics.active_mappers {
            frame.cache.clear();
        }
    }

    /// `checkExpressionEx` and the native statement/deferred-node entries reset
    /// the count without restoring it on exit; nested expression work shares
    /// the current counter while instantiation depth remains independent.
    pub(super) fn reset_instantiation_count(&mut self) {
        self.generics.instantiation_count = 0;
    }

    fn instantiate_type_worker_completion(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            // `instantiateTypeWorker` maps parameter headers without describing
            // them. Fresh locals can still be Allocating during mapper composition.
            if self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER) {
                return sequence.read(self.map_parameter_completion(ty, mapper, consumer));
            }
            match self.model.get(ty)? {
                TypeView::Special(_)
                | TypeView::Primitive(_)
                | TypeView::Enum
                | TypeView::UniqueSymbol(_)
                | TypeView::Literal { .. }
                | TypeView::TypeParameter(_) => Ok(ty),
                TypeView::Conditional(view) => {
                    let mapper = match view.mapper {
                        Some(first) => self.model.combine_mappers(first, mapper),
                        None => mapper,
                    };
                    sequence.read(
                        self.conditional_instantiation_completion(
                            ty, mapper, false, alias, consumer,
                        ),
                    )
                }
                TypeView::Substitution(_) => {
                    sequence.read(self.instantiate_substitution_completion(ty, mapper, consumer))
                }
                TypeView::Reference(reference) => {
                    if self.model.deferred_reference(ty).is_some() {
                        return sequence.read(
                            self.instantiate_object_type_completion(ty, mapper, alias, consumer),
                        );
                    }
                    let original = sequence.read(self.get_type_arguments_completion(ty))?;
                    let arguments = original
                        .iter()
                        .map(|&argument| {
                            sequence
                                .read(self.instantiate_type_completion(argument, mapper, consumer))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if arguments.as_slice() == original.as_ref() {
                        return Ok(ty);
                    }
                    sequence.read(self.create_normalized_reference_completion(
                        reference.target,
                        arguments,
                        consumer,
                    ))
                }
                TypeView::StringMapping(mapping) => {
                    let target = sequence.read(self.instantiate_type_completion(
                        mapping.target,
                        mapper,
                        consumer,
                    ))?;
                    sequence.read(self.string_mapping_type_completion(
                        mapping.symbol,
                        mapping.kind,
                        target,
                        consumer,
                    ))
                }
                TypeView::TemplateLiteral(template) => {
                    let types = template
                        .types
                        .iter()
                        .map(|&ty| {
                            sequence.read(self.instantiate_type_completion(ty, mapper, consumer))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    sequence.read(self.template_literal_type_completion(
                        template.texts,
                        types,
                        consumer,
                    ))
                }
                TypeView::Index(index) => {
                    let target = sequence.read(self.instantiate_type_completion(
                        index.target,
                        mapper,
                        consumer,
                    ))?;
                    sequence.read(self.index_type_ex_completion(
                        target,
                        super::super::model::IndexFlags::NONE,
                        consumer,
                    ))
                }
                TypeView::IndexedAccess(access) => {
                    let alias = match alias {
                        Some(alias) => Some(alias),
                        None => sequence.read(
                            self.instantiate_type_alias_completion(
                                self.model
                                    .metadata(ty)?
                                    .alias
                                    .map(|alias| alias.as_ref().clone()),
                                mapper,
                                consumer,
                            ),
                        )?,
                    };
                    let object = sequence.read(self.instantiate_type_completion(
                        access.object_type,
                        mapper,
                        consumer,
                    ))?;
                    let index = sequence.read(self.instantiate_type_completion(
                        access.index_type,
                        mapper,
                        consumer,
                    ))?;
                    sequence.read(self.indexed_access_type_ex_completion(
                        object,
                        index,
                        access.access_flags,
                        alias,
                        consumer,
                    ))
                }
                TypeView::Union(_) | TypeView::Intersection(_) => {
                    let metadata = self.model.metadata(ty)?;
                    let source = metadata.summary_origin.unwrap_or(ty);
                    let intersection = self.model.has_type_flags(source, TypeFlags::INTERSECTION);
                    let types = self
                        .model
                        .union_constituents(source)
                        .or_else(|| self.model.intersection_constituents(source))
                        .ok_or_else(|| {
                            super::super::internal("composite instantiation has no constituents")
                        })?;
                    let instantiated = types
                        .iter()
                        .map(|&member| {
                            sequence
                                .read(self.instantiate_type_completion(member, mapper, consumer))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if instantiated.as_slice() == types.as_ref()
                        && alias.as_ref().map(|alias| alias.symbol)
                            == metadata.alias.as_ref().map(|alias| alias.symbol)
                    {
                        return Ok(ty);
                    }
                    let alias = match alias {
                        Some(alias) => Some(alias),
                        None => sequence.read(self.instantiate_type_alias_completion(
                            metadata.alias.map(|alias| alias.as_ref().clone()),
                            mapper,
                            consumer,
                        ))?,
                    };
                    if intersection {
                        sequence.read(self.intersection_type_ex_completion(
                            instantiated,
                            IntersectionFlags::NONE,
                            alias,
                            consumer,
                        ))
                    } else {
                        sequence.read(self.union_type_ex_completion(
                            instantiated,
                            alias,
                            None,
                            consumer,
                        ))
                    }
                }
                TypeView::Object(_) | TypeView::LazyObject(_) => {
                    // `instantiateTypeWorker` dispatches by native object flags,
                    // independently from whether our inventory is eager or lazy.
                    let flags = self.model.object_flags(ty)?;
                    if !flags.intersects(
                        ObjectFlags::REFERENCE | ObjectFlags::ANONYMOUS | ObjectFlags::MAPPED,
                    ) {
                        Ok(ty)
                    } else if flags.intersects(ObjectFlags::REVERSE_MAPPED) {
                        sequence.read(
                            self.instantiate_reverse_mapped_type_completion(ty, mapper, consumer),
                        )
                    } else {
                        sequence.read(
                            self.instantiate_object_type_completion(ty, mapper, alias, consumer),
                        )
                    }
                }
            }
        })();
        sequence.finish(result)
    }

    /// Native `TypeMapper.Map` and its Simple/Array/Deferred/ArrayToSingle,
    /// Merged and Composite implementations. Charge only visited comparisons;
    /// a short-circuit never scans or charges an unvisited parameter tail.
    pub(super) fn map_parameter_completion(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let control = self.control.clone();
        let _depth = control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let matches_source = |source: &TypeId| *source == ty;
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<TypeId, DemandError> {
            // Borrow the stored table: Map performs no source/target copy.
            match self.model.mapper_data(mapper) {
                Mapper::Permissive => Ok(
                    if self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER) {
                        self.model.wildcard_type()
                    } else {
                        ty
                    },
                ),
                Mapper::Restrictive => self.restrictive_parameter(ty),
                Mapper::Deferred { id, sources } => {
                    let id = *id;
                    match sources.iter().position(matches_source) {
                        Some(index) => {
                            sequence.read(self.deferred_infer_argument_completion(id, index))
                        }
                        None => Ok(ty),
                    }
                }
                Mapper::UniqueLiteral => Ok(
                    if self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER) {
                        self.model.unique_literal_type()
                    } else {
                        ty
                    },
                ),
                Mapper::Simple { sources, targets } => {
                    match sources.iter().position(matches_source).map(|index| targets[index]) {
                        Some(MapperTarget::Known(target)) => Ok(target),
                        Some(MapperTarget::ErrorType) => sequence.read(NativeCompletion::recovered(
                            self.model.native_error_type(),
                            super::super::invalid_at_location(
                                self.inputs, consumer, SemanticFailureReason::ErrorType,
                                "default substitution consumes an unsupplied parameter's native errorType recovery",
                            ).into(),
                        )),
                        None => Ok(ty),
                    }
                }
                Mapper::Composite { first, second } => {
                    let (first, second) = (*first, *second);
                    let mapped =
                        sequence.read(self.map_parameter_completion(ty, first, consumer))?;
                    if mapped != ty {
                        sequence.read(self.instantiate_type_completion(mapped, second, consumer))
                    } else {
                        sequence.read(self.map_parameter_completion(ty, second, consumer))
                    }
                }
                Mapper::Merged { first, second } => {
                    let (first, second) = (*first, *second);
                    let mapped =
                        sequence.read(self.map_parameter_completion(ty, first, consumer))?;
                    sequence.read(self.map_parameter_completion(mapped, second, consumer))
                }
                Mapper::ArrayToSingle { sources, target } => {
                    Ok(if sources.iter().any(matches_source) {
                        *target
                    } else {
                        ty
                    })
                }
                Mapper::Inference {
                    context, fixing, ..
                } => {
                    let (context, fixing) = (*context, *fixing);
                    sequence.read(
                        self.map_inference_parameter_completion(ty, context, fixing, consumer),
                    )
                }
                Mapper::ReportReliability { unmeasurable } => {
                    let unmeasurable = *unmeasurable;
                    self.report_variance_reliability(ty, unmeasurable);
                    Ok(ty)
                }
            }
        })();
        sequence.finish(result)
    }

    /// `getDeclaredTypeOfTypeParameter` installs one identity through the
    /// parent's canonical declared-symbol cache. Neither constraint nor default
    /// resolution participates in establishing that identity.
    pub(super) fn parameter_type(
        &mut self,
        symbol: impl Into<super::ReferenceSymbol>,
        first: BoundDeclaration,
    ) -> Result<TypeId, DemandError> {
        let symbol = symbol.into();
        let consumer = super::super::location(self.inputs, first.file, first.anchor);
        let mut declarations = Vec::new();
        for declaration in self.reference_symbol_declarations(symbol, &consumer)? {
            if let super::ReferenceDeclaration::Captured(declaration) = declaration
                && self.source.declaration_kind(declaration)?
                    == crate::syntax_kind::SyntaxKind::TypeParameter
            {
                declarations.push(declaration);
            }
        }
        let TypeParameterRecipe {
            name,
            constraint,
            default,
            variance,
            constant,
        } = self.source.type_parameter(&declarations, first)?;
        let display_name = self.symbol_name_as_written(
            symbol,
            &super::super::location(self.inputs, first.file, first.anchor),
        )?;
        let inferred = matches!(constraint, Some(ParameterConstraintRecipe::Inferred { .. }));
        let constraint = constraint
            .filter(|recipe| matches!(recipe, ParameterConstraintRecipe::Annotation { .. }));
        let constraint_location = constraint.as_ref().map(|recipe| recipe.location().clone());
        let raw_constraint = constraint.map(|recipe| {
            self.register_generic_demand(GenericDemand::RawParameterConstraint(recipe))
        });
        let constraint = constraint_location.as_ref().map(|location| {
            self.register_generic_demand(GenericDemand::ParameterConstraint {
                symbol,
                location: location.clone(),
            })
        });
        let default = default.map(|expression| {
            self.register_generic_demand(GenericDemand::ParameterDefault(Box::new(expression)))
        });
        let native_symbol = self.type_symbol_of_reference(symbol, &consumer)?;
        let ty = self.model.type_parameter(
            native_symbol,
            ParameterData {
                name,
                display_name,
                constraint,
                default,
                variance,
                constant,
                is_this: false,
            },
        );
        self.constraints.register_parameter(
            ty,
            symbol,
            raw_constraint,
            constraint_location
                .unwrap_or_else(|| super::super::location(self.inputs, first.file, first.anchor)),
        );
        if inferred {
            self.constraints.register_inferred(ty);
        }
        Ok(ty)
    }

    // The native canonical parameter factory runs once, so each specialized
    // child gets one DemandId in the existing request-local question cache.
    // There is no second constraint/default cache keyed by source spelling.
    pub(super) fn register_generic_demand(&mut self, recipe: GenericDemand) -> TypeDemand {
        let demand = self.register_demand(TypeDemandState {
            location: Some(recipe.location().clone()),
            recipe: DemandRecipe::Generic(recipe),
            state: None,
        });
        TypeDemand::Deferred(demand)
    }

    pub(super) fn check_generic_demand_completion(
        &mut self,
        recipe: &GenericDemand,
    ) -> NativeResult<TypeId> {
        match recipe {
            GenericDemand::Instantiated {
                original,
                mapper,
                location,
            } => self
                .demand_completion(*original)?
                .and_then(|ty| self.instantiate_type_completion(ty, *mapper, location)),
            GenericDemand::ParameterDefault(expression) => self.check_type_completion(expression),
            GenericDemand::RawParameterConstraint(recipe) => {
                self.raw_parameter_constraint_completion(recipe)
            }
            GenericDemand::ParameterConstraint { symbol, location } => {
                let declaration = self.declared_reference_family(*symbol, location)?;
                self.declared_type_completion(*symbol, declaration)?
                    .and_then(|ty| {
                        let constraint = self.parameter_constraint_completion(ty, location)?;
                        constraint_type_demand(
                            constraint,
                            "declared constraint demand has no raw constraint",
                        )
                    })
            }
            GenericDemand::ClonedParameterConstraint { clone, location } => {
                let constraint = self.parameter_constraint_completion(*clone, location)?;
                constraint_type_demand(
                    constraint,
                    "cloned guarded constraint has no raw constraint",
                )
            }
            GenericDemand::ConditionalBranch {
                ty,
                true_branch,
                location,
            } => {
                if *true_branch {
                    self.conditional_true_type_completion(*ty, location)
                } else {
                    self.conditional_false_type_completion(*ty, location)
                }
            }
            _ => self
                .check_generic_demand_worker(recipe)
                .map(NativeCompletion::clean),
        }
    }

    fn check_generic_demand_worker(
        &mut self,
        recipe: &GenericDemand,
    ) -> Result<TypeId, DemandError> {
        match recipe {
            GenericDemand::MappedOperand {
                ty,
                operand,
                location,
            } => self.mapped_operand(*ty, *operand, location),
            GenericDemand::ConditionalBranch { .. }
            | GenericDemand::ParameterDefault(_)
            | GenericDemand::Instantiated { .. }
            | GenericDemand::RawParameterConstraint(_)
            | GenericDemand::ParameterConstraint { .. }
            | GenericDemand::ClonedParameterConstraint { .. } => {
                unreachable!("completion-bearing generic demand dispatched before legacy owner")
            }
        }
    }

    /// `getConstraintFromTypeParameter` is the raw native worker, distinct
    /// from the base-resolution guard on the public constraint question.
    /// Genuine any normalizes to unknown; an alias-bearing error Any retains
    /// its actual identity and recovery. Mapped keys ask a distinct union.
    fn raw_parameter_constraint_completion(
        &mut self,
        recipe: &ParameterConstraintRecipe,
    ) -> NativeResult<TypeId> {
        let ParameterConstraintRecipe::Annotation {
            expression,
            mapped_key,
        } = recipe
        else {
            return Err(super::super::unsupported_demand_at_location(
                self.inputs,
                recipe.location(),
                SemanticFailureReason::UnsupportedSemanticForm,
                "native inferred type-parameter constraint construction is pending",
            ));
        };
        self.check_type_completion(expression)?
            .and_then_value(|constraint| {
                let constraint = if self
                    .model
                    .has_type_flags(constraint, super::super::model::TypeFlags::ANY)
                    && !self.model.is_error_type(constraint)
                {
                    if *mapped_key {
                        return Ok(self.model.string_number_symbol_type());
                    }
                    self.model.special(SpecialName::Unknown)
                } else {
                    constraint
                };
                Ok(constraint)
            })
    }
}

/// A retained TypeDemand promises a type rather than native nil. Circular
/// guarded absence propagates its recovery instead of replacing that cause
/// with an internal missing-type error at this representational boundary.
fn constraint_type_demand(
    constraint: NativeCompletion<Option<TypeId>>,
    missing: &'static str,
) -> NativeResult<TypeId> {
    if let Some(ty) = *constraint.value() {
        return Ok(constraint.map(|_| ty));
    }
    constraint.admit()?;
    Err(super::super::internal(missing).into())
}

/// Native core.Concatenate preserves the surviving inventory identity when
/// either side is empty; getUnionSignatures observes shared signature slices.
fn concatenate_signatures(
    left: std::sync::Arc<[Signature]>,
    right: std::sync::Arc<[Signature]>,
) -> std::sync::Arc<[Signature]> {
    if left.is_empty() {
        return right;
    }
    if right.is_empty() {
        return left;
    }
    let mut result = left.to_vec();
    result.extend_from_slice(&right);
    result.into()
}
