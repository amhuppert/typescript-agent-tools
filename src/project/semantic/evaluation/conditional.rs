//! Conditional roots own native instantiation and lazy branch caches. Mapper
//! callbacks, inference candidates and relation sessions remain shared owners.
use super::super::model::{
    Alias, AliasKey, ConditionalRootId, ConditionalView, DeferredMapperId, DemandError, MapperId,
    NativeCompletion, NativeResult, NativeSequence, SpecialName, TypeFlags, TypeId, TypeView,
};
use super::super::model::{KeyBuilder, same_alias, same_types};
use super::super::source::{
    ExpressionRoot, TypeExpression,
    conditional::{ConditionalFlowStep, ConditionalRecipe, InferredConstraint},
};
use super::{Evaluator, relations::RelationKind};
use crate::analysis::SemanticFailureReason;
use crate::references::Location;
use crate::resources::{CheckedKey, CheckedMap};
use std::collections::HashMap;
use std::sync::Arc;

struct Root {
    data: Arc<RootData>,
    instantiations: CheckedMap<InstantiationKey, NativeResult<TypeId>>,
}

struct RootData {
    recipe: ConditionalRecipe,
    check_type: TypeId,
    extends_type: TypeId,
    distributive: bool,
    infer_parameters: Vec<TypeId>,
    outer_parameters: Vec<TypeId>,
    alias: Option<Alias>,
}

impl std::ops::Deref for Root {
    type Target = RootData;
    fn deref(&self) -> &RootData {
        &self.data
    }
}
/// `getConditionalTypeKey`: each native root owns its table. Effective outer
/// arguments, actual alias and forConstraint remain separate identity domains.
struct InstantiationKey {
    arguments: Vec<TypeId>,
    alias: Option<AliasKey>,
    for_constraint: bool,
    fingerprint: u64,
}

impl InstantiationKey {
    fn new(arguments: &[TypeId], alias: Option<&Alias>, for_constraint: bool) -> Self {
        let alias = AliasKey::of(alias);
        let mut hash = KeyBuilder::new();
        hash.types(arguments);
        hash.alias(alias.as_ref());
        hash.fixed(&for_constraint);
        Self {
            arguments: arguments.to_vec(),
            alias,
            for_constraint,
            fingerprint: hash.finish(),
        }
    }
}

impl CheckedKey for InstantiationKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        self.for_constraint == other.for_constraint
            && same_types(&self.arguments, &other.arguments)
            && same_alias(self.alias.as_ref(), other.alias.as_ref())
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Branch {
    True,
    False,
    InferredTrue,
    DefaultConstraint,
}
// Four fixed native questions, independent of either branch's type graph.

pub(super) struct ConditionalState {
    nodes: HashMap<ExpressionRoot, NativeResult<TypeId>>,
    roots: Vec<Root>,
    branches: HashMap<(TypeId, Branch), NativeResult<TypeId>>,
    branch_demands: HashMap<(TypeId, bool), super::super::model::TypeDemand>,
    distributive_constraints: HashMap<TypeId, NativeResult<Option<TypeId>>>,
    permissive: HashMap<TypeId, NativeResult<TypeId>>,
    restrictive: HashMap<TypeId, NativeResult<TypeId>>,
    restrictive_parameters: HashMap<TypeId, TypeId>,
    deferred_arguments: Vec<(TypeExpression, Vec<TypeId>)>,
    no_infer_consumers: HashMap<TypeId, Location>,
    checked_infer_parameters: HashMap<
        crate::project::binding::NativeSymbolIdentity,
        super::super::model::NativeResult<()>,
    >,
}

impl Evaluator<'_, '_> {
    /// Lazy public operand handles preserve native true/false branch caches
    /// without acquiring either branch during graph preview or skeleton work.
    pub(in crate::project::semantic) fn conditional_branch_demand(
        &mut self,
        ty: TypeId,
        true_branch: bool,
    ) -> Result<super::super::model::TypeDemand, DemandError> {
        if let Some(&demand) = self.conditional.branch_demands.get(&(ty, true_branch)) {
            return Ok(demand);
        }
        let TypeView::Conditional(view) = self.model.get(ty)? else {
            return Err(super::super::internal("conditional display lacks header").into());
        };
        let root = &self.conditional.roots[view.root.0];
        let location = if true_branch {
            root.recipe.true_type.location.clone()
        } else {
            root.recipe.false_type.location.clone()
        };
        let demand =
            self.register_generic_demand(super::generics::GenericDemand::ConditionalBranch {
                ty,
                true_branch,
                location,
            });
        self.conditional
            .branch_demands
            .insert((ty, true_branch), demand);
        Ok(demand)
    }

    /// `checkInferType` performs the shared parameter check before the one-time
    /// canonical declaration constraint/default compatibility check.
    pub(super) fn check_infer_type_completion(
        &mut self,
        node: ExpressionRoot,
    ) -> super::super::model::NativeResult<()> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let parameter = self.source.infer_parameter_declaration(node)?;
            let root = self.source.declaration_expression_root(parameter)?;
            sequence.read(self.check_type_source_element_completion(root))?;
            let symbol =
                sequence.read(self.reference_symbol_of_declaration_completion(parameter))?;
            let consumer = &super::super::location(self.inputs, node.file, node.site.span);
            let declarations = self.reference_symbol_declarations(symbol, consumer)?;
            if declarations.len() > 1 {
                let identity = self.reference_symbol_identity(symbol, consumer)?;
                if let Some(result) = self.conditional.checked_infer_parameters.get(&identity) {
                    return sequence.read(result.clone());
                }
                self.conditional.checked_infer_parameters.insert(
                    identity,
                    Ok(super::super::model::NativeCompletion::clean(())),
                );
                let mut local = super::super::model::NativeSequence::new();
                let result = (|| {
                    let ty =
                        local.read(self.declared_reference_type_completion(symbol, consumer))?;
                    self.minimum_type_argument_count(&[ty])?;
                    let target_name = match self.model.get(ty)? {
                        TypeView::TypeParameter(parameter) => parameter.copy_name(),
                        _ => {
                            return Err(super::super::internal(
                                "infer declared family is not parameter",
                            )
                            .into());
                        }
                    };
                    for declaration in declarations {
                        let super::ReferenceDeclaration::Captured(declaration) = declaration else {
                            return Err(super::super::internal(
                                "a JSON declaration cannot be an infer parameter",
                            )
                            .into());
                        };
                        if self.source.declaration_kind(declaration)?
                            != crate::syntax_kind::SyntaxKind::TypeParameter
                        {
                            continue;
                        }
                        let (name, constraint, default) =
                            self.source.parameter_declaration_children(declaration)?;
                        if name != target_name {
                            break;
                        }
                        let target_constraint =
                            local.read(self.parameter_constraint_completion(ty, consumer))?;
                        if let (Some(constraint), Some(target)) = (constraint, target_constraint) {
                            let source = local.read(self.check_type_completion(&constraint))?;
                            if !local.read(self.type_related_completion(
                                source,
                                target,
                                RelationKind::Identity,
                                consumer,
                            ))? {
                                break;
                            }
                        }
                        let target_default =
                            local.read(self.default_from_type_parameter_completion(ty))?;
                        if let (Some(default), Some(target)) = (default, target_default) {
                            let source = local.read(self.check_type_completion(&default))?;
                            if !local.read(self.type_related_completion(
                                source,
                                target,
                                RelationKind::Identity,
                                consumer,
                            ))? {
                                break;
                            }
                        }
                    }
                    Ok(())
                })();
                let result = local.finish(result);
                self.conditional
                    .checked_infer_parameters
                    .insert(identity, result.clone());
                sequence.read(result)?;
            }
            Ok(())
        })();
        sequence.finish(result)
    }
    pub(in crate::project::semantic) fn conditional_parameters(
        &self,
        ty: TypeId,
    ) -> Result<Vec<TypeId>, DemandError> {
        let TypeView::Conditional(view) = self.model.get(ty)? else {
            return Err(
                super::super::internal("conditional parameter question lacks header").into(),
            );
        };
        Ok(self.conditional.roots[view.root.0].infer_parameters.clone())
    }
    pub(super) fn conditional_is_distributive(&self, ty: TypeId) -> Result<bool, DemandError> {
        let TypeView::Conditional(view) = self.model.get(ty)? else {
            return Err(
                super::super::internal("conditional distribution question lacks header").into(),
            );
        };
        Ok(self.conditional.roots[view.root.0].distributive)
    }
    /// Actual root check type, distinct from an instantiated check operand.
    pub(super) fn conditional_root_check(&self, root: ConditionalRootId) -> TypeId {
        self.conditional.roots[root.0].check_type
    }

    pub(super) fn conditional_root_node(
        &self,
        root: ConditionalRootId,
    ) -> crate::project::binding::NativeNodeIdentity {
        self.conditional.roots[root.0].recipe.declaration
    }
    /// `isDistributionDependent` observes true before false and only asks
    /// syntax reference containment for an actual distributive root.
    pub(super) fn conditional_distribution_dependent_completion(
        &mut self,
        ty: TypeId,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<bool, DemandError> {
            let TypeView::Conditional(view) = self.model.get(ty)? else {
                return Err(super::super::internal(
                    "conditional distribution question lacks header",
                )
                .into());
            };
            let root = self.conditional.roots[view.root.0].data.clone();
            if !root.distributive {
                return Ok(false);
            }
            for branch in [&root.recipe.true_type, &root.recipe.false_type] {
                let node = branch.root.ok_or_else(|| {
                    super::super::internal("conditional branch lacks node identity")
                })?;
                let declaration = self.source.type_node_declaration(node)?;
                if sequence.read(self.parameter_possibly_referenced_completion(
                    &[declaration.into()],
                    root.check_type,
                    &branch.location,
                ))? {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }
    /// `getInferredTypeParameterConstraint` visits canonical declarations in
    /// native order. Deferred reference mappings ask each effective argument
    /// only when instantiation maps that source parameter.
    pub(super) fn inferred_parameter_constraint(
        &mut self,
        ty: TypeId,
        symbol: impl Into<super::ReferenceSymbol>,
        omit_type_references: bool,
        consumer: &Location,
    ) -> Result<Option<TypeId>, DemandError> {
        self.inferred_parameter_constraint_completion(ty, symbol, omit_type_references, consumer)?
            .admit()
    }

    pub(super) fn inferred_parameter_constraint_completion(
        &mut self,
        ty: TypeId,
        symbol: impl Into<super::ReferenceSymbol>,
        omit_type_references: bool,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let declarations = self.reference_symbol_declarations(symbol.into(), consumer)?;
            let mut constraints = Vec::new();
            for declaration in declarations {
                let super::ReferenceDeclaration::Captured(declaration) = declaration else {
                    continue;
                };
                let Some(recipe) = self
                    .source
                    .inferred_constraint(declaration, omit_type_references)?
                else {
                    continue;
                };
                match recipe {
                    InferredConstraint::Reference { node, index } => {
                        let parameters =
                            sequence.read(self.type_reference_parameters_completion(&node))?;
                        if let Some(&parameter) = parameters.get(index)
                            && let Some(constraint) = sequence.read(
                                self.parameter_constraint_completion(parameter, &node.location),
                            )?
                        {
                            let id = DeferredMapperId(self.conditional.deferred_arguments.len());
                            self.conditional
                                .deferred_arguments
                                .push(((*node).clone(), parameters.clone()));
                            let mapper = self.model.deferred_type_mapper(id, parameters);
                            let constraint = sequence.read(self.instantiate_type_completion(
                                constraint,
                                mapper,
                                &node.location,
                            ))?;
                            if constraint != ty {
                                constraints.push(constraint);
                            }
                        }
                    }
                    InferredConstraint::Rest => {
                        let unknown = self.model.special(SpecialName::Unknown);
                        constraints
                            .push(sequence.read(self.array_type_completion(unknown, consumer))?);
                    }
                    InferredConstraint::String => {
                        constraints.push(self.model.primitive(crate::types::PrimitiveName::String))
                    }
                    InferredConstraint::Keys => {
                        constraints.push(self.model.string_number_symbol_type())
                    }
                    InferredConstraint::Mapped {
                        template,
                        parameter,
                        constraint,
                    } => {
                        let template = sequence.read(self.check_type_completion(&template))?;
                        let symbol = sequence
                            .read(self.reference_symbol_of_declaration_completion(parameter))?;
                        let parameter = sequence
                            .read(self.declared_reference_type_completion(symbol, consumer))?;
                        let constraint = match constraint {
                            Some(recipe) => sequence.read(self.check_type_completion(&recipe))?,
                            None => self.model.string_number_symbol_type(),
                        };
                        let mapper = self.model.mapper(vec![parameter], vec![constraint])?;
                        constraints.push(
                            sequence.read(
                                self.instantiate_type_completion(template, mapper, consumer),
                            )?,
                        );
                    }
                }
            }
            if constraints.is_empty() {
                Ok(None)
            } else {
                sequence
                    .read(self.intersection_type_completion(constraints, consumer))
                    .map(Some)
            }
        })();
        sequence.finish(result)
    }

    pub(super) fn deferred_infer_argument_completion(
        &mut self,
        id: DeferredMapperId,
        index: usize,
    ) -> NativeResult<TypeId> {
        let (node, parameters) = self.conditional.deferred_arguments[id.0].clone();
        self.effective_type_argument_at_completion(&node, &parameters, index)
    }

    /// `getNoInferType` / `isNoInferTargetType` preserve potentially generic
    /// types without traversing object members or wrapping literal patterns.
    pub(super) fn no_infer_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if sequence.read(self.is_no_infer_target_type_completion(ty, consumer))? {
                let unknown = self.model.special(SpecialName::Unknown);
                let result = self.model.substitution_type_unchecked(ty, unknown);
                self.conditional
                    .no_infer_consumers
                    .entry(result)
                    .or_insert_with(|| consumer.clone());
                Ok(result)
            } else {
                Ok(ty)
            }
        })();
        sequence.finish(result)
    }

    /// Native node-builder `Substitution` display uses the actual global alias
    /// only when that optional alias exists. The factory retains the consumer
    /// solely for failures of this later acquired global-symbol question.
    pub(in crate::project::semantic) fn display_no_infer_alias(
        &mut self,
        ty: TypeId,
    ) -> Result<bool, DemandError> {
        if !self.model.is_no_infer_type(ty) {
            return Ok(false);
        }
        let consumer = self
            .conditional
            .no_infer_consumers
            .get(&ty)
            .cloned()
            .ok_or_else(|| super::super::internal("NoInfer display lacks its factory consumer"))?;
        Ok(self.global_type_alias("NoInfer", 1, &consumer)?.is_some())
    }

    /// Native node-builder infer declaration constraint omission. This semantic
    /// question is reached only after the display owner admits the infer clause.
    pub(in crate::project::semantic) fn infer_constraint_for_display(
        &mut self,
        ty: TypeId,
    ) -> Result<Option<TypeId>, DemandError> {
        let consumer = self.parameter_constraint_location(ty)?.clone();
        let Some(constraint) = self.parameter_constraint(ty, &consumer)? else {
            return Ok(None);
        };
        let symbol = self
            .reference_symbol_of_type(ty, &consumer)?
            .ok_or_else(|| super::super::internal("inferred parameter lacks its symbol"))?;
        if let Some(inferred) = self.inferred_parameter_constraint(ty, symbol, true, &consumer)? {
            if self.type_related(constraint, inferred, RelationKind::Identity, &consumer)? {
                return Ok(None);
            }
        }
        Ok(Some(constraint))
    }

    fn is_no_infer_target_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| match self.model.get(ty)? {
            TypeView::Union(types) | TypeView::Intersection(types) => {
                for &part in types.iter() {
                    if sequence.read(self.is_no_infer_target_type_completion(part, consumer))? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            TypeView::Substitution(view) => Ok(!self.model.is_no_infer_type(ty)
                && sequence.read(self.is_no_infer_target_type_completion(view.base, consumer))?),
            _ if self.model.has_type_flags(ty, TypeFlags::OBJECT) => {
                Ok(!sequence.read(self.is_empty_anonymous_object_type_completion(ty, consumer))?)
            }
            _ => Ok(self.model.has_type_flags(ty, TypeFlags::INSTANTIABLE)
                && !self.is_pattern_literal_type(ty)?),
        })();
        sequence.finish(result)
    }

    /// `instantiateTypeWorker(Substitution)` acquires base first and skips
    /// constraint for NoInfer. Ordinary intersections retain [constraint, base].
    pub(super) fn instantiate_substitution_completion(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let TypeView::Substitution(view) = self.model.get(ty)? else {
                return Err(
                    super::super::internal("substitution instantiation lacks header").into(),
                );
            };
            let base =
                sequence.read(self.instantiate_type_completion(view.base, mapper, consumer))?;
            if self.model.is_no_infer_type(ty) {
                return sequence.read(self.no_infer_type_completion(base, consumer));
            }
            let constraint = sequence.read(self.instantiate_type_completion(
                view.constraint,
                mapper,
                consumer,
            ))?;
            let variable = self.model.has_type_flags(base, TypeFlags::TYPE_VARIABLE);
            if variable && sequence.read(self.is_generic_type_completion(constraint, consumer))? {
                return Ok(self.model.substitution_type(base, constraint));
            }
            if self
                .model
                .has_type_flags(constraint, TypeFlags::ANY | TypeFlags::UNKNOWN)
            {
                return Ok(base);
            }
            let source =
                sequence.read(self.restrictive_instantiation_completion(base, consumer))?;
            let target =
                sequence.read(self.restrictive_instantiation_completion(constraint, consumer))?;
            if sequence.read(self.type_related_completion(
                source,
                target,
                RelationKind::Assignable,
                consumer,
            ))? {
                return Ok(base);
            }
            if variable {
                Ok(self.model.substitution_type(base, constraint))
            } else {
                sequence.read(self.intersection_type_completion(vec![constraint, base], consumer))
            }
        })();
        sequence.finish(result)
    }

    /// `getSubstitutionIntersection` exposes the native structural view.
    pub(super) fn substitution_intersection_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let TypeView::Substitution(view) = self.model.get(ty)? else {
                return Err(
                    super::super::internal("substitution intersection lacks header").into(),
                );
            };
            if self.model.is_no_infer_type(ty) {
                return Ok(view.base);
            }
            sequence
                .read(self.intersection_type_completion(vec![view.constraint, view.base], consumer))
        })();
        sequence.finish(result)
    }

    /// `getInferTypeParameters` preserves actual binding-table symbols. The
    /// table may be empty; an unavailable retained table is a failed question.
    pub(super) fn infer_type_parameters(
        &mut self,
        node: ExpressionRoot,
        consumer: &Location,
    ) -> Result<Vec<TypeId>, DemandError> {
        self.infer_type_parameters_completion(node, consumer)?
            .admit()
    }

    pub(super) fn infer_type_parameters_completion(
        &mut self,
        node: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<Vec<TypeId>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let declaration = self.source.type_node_declaration(node)?;
            let native = self.source.declaration_ordering(declaration)?;
            let symbols = self
                .source
                .bindings()
                .conditional_parameters(native.file, native.span)
                .map_err(|error| self.generic_binding_failure(error, consumer))?;
            symbols
                .into_iter()
                .map(|symbol| {
                    let declaration = self
                        .source
                        .bindings()
                        .declared_type(symbol)
                        .map_err(|error| self.generic_binding_failure(error, consumer))?;
                    sequence.read(self.declared_type_completion(symbol, declaration))
                })
                .collect()
        })();
        sequence.finish(result)
    }

    /// `getTypeFromInferTypeNode` shares the canonical declared parameter; its
    /// type-node slot is separate from that symbol's raw constraint slot.
    pub(super) fn infer_type_node_completion(
        &mut self,
        node: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(result) = self.conditional.nodes.get(&node) {
            return result.clone();
        }
        let result = (|| {
            let declaration = self.source.infer_parameter_declaration(node)?;
            self.reference_symbol_of_declaration_completion(declaration)?
                .and_then(|symbol| self.declared_reference_type_completion(symbol, consumer))
        })();
        self.conditional.nodes.insert(node, result.clone());
        result
    }

    /// `getTypeFromConditionalTypeNode`: check, alias and filtered outer
    /// parameters precede extends and infer locals. Branches remain lazy.
    pub(super) fn conditional_type_node_completion(
        &mut self,
        node: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(result) = self.conditional.nodes.get(&node) {
            return result.clone();
        }
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let recipe = self.source.conditional_recipe(node)?;
            let check_type = sequence.read(self.check_type_completion(&recipe.check))?;
            let alias = recipe
                .alias
                .as_ref()
                .map(|alias| sequence.read(self.establish_alias_completion(alias)))
                .transpose()?;
            let all = sequence.read(self.outer_type_parameters_completion(
                recipe.declaration,
                true,
                consumer,
            ))?;
            let mut outer_parameters = Vec::new();
            if alias
                .as_ref()
                .and_then(|alias| alias.arguments.as_ref())
                .is_some_and(|arguments| !arguments.is_empty())
            {
                outer_parameters = all;
            } else {
                for parameter in all {
                    let referenced =
                        sequence.read(self.parameter_possibly_referenced_completion(
                            &[recipe.declaration.into()],
                            parameter,
                            consumer,
                        ))?;
                    if referenced {
                        outer_parameters.push(parameter);
                    }
                }
            }
            let extends_type = sequence.read(self.check_type_completion(&recipe.extends))?;
            let infer_parameters =
                sequence.read(self.infer_type_parameters_completion(node, consumer))?;
            let id = ConditionalRootId(self.conditional.roots.len());
            self.conditional.roots.push(Root {
                data: Arc::new(RootData {
                    recipe,
                    check_type,
                    extends_type,
                    distributive: self
                        .model
                        .has_type_flags(check_type, TypeFlags::TYPE_PARAMETER),
                    infer_parameters,
                    outer_parameters: outer_parameters.clone(),
                    alias,
                }),
                instantiations: CheckedMap::new(),
            });
            let result = self.conditional_type_completion(id, None, false, None, consumer);
            if !outer_parameters.is_empty() {
                self.conditional.roots[id.0].instantiations.insert(
                    InstantiationKey::new(&outer_parameters, None, false),
                    result.clone(),
                );
            }
            sequence.read(result)
        })();
        let result = sequence.finish(result);
        self.conditional.nodes.insert(node, result.clone());
        result
    }

    /// `getConditionalType` uses native permissive/restrictive comparisons
    /// and iterates immediate/tail conditionals. Its 1000 limit counts only
    /// aliased tail transitions; it is distinct from instantiation depth.
    fn conditional_type_completion(
        &mut self,
        mut id: ConditionalRootId,
        mut mapper: Option<MapperId>,
        for_constraint: bool,
        mut alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut extra = Vec::new();
            let mut tails = 0;
            let result = loop {
                if tails == 1000 {
                    // getConditionalType returns errorType immediately, before
                    // reducing any accumulated extraTypes. Keep the recovered
                    // value so enclosing operations can acquire later operands.
                    return sequence.read(NativeCompletion::recovered(
                        self.model.native_error_type(),
                        super::super::invalid_at_location(
                            self.inputs,
                            consumer,
                            SemanticFailureReason::ConditionalTailLimit,
                            "conditional type exceeds the native tail-instantiation limit",
                        )
                        .into(),
                    ));
                }
                let root = self.conditional.roots[id.0].data.clone();
                let actual_check = sequence
                    .read(self.actual_type_variable_completion(root.check_type, consumer))?;
                let check = sequence.read(self.instantiate_optional_completion(
                    actual_check,
                    mapper,
                    consumer,
                ))?;
                let extends = sequence.read(self.instantiate_optional_completion(
                    root.extends_type,
                    mapper,
                    consumer,
                ))?;
                if check == self.model.native_error_type()
                    || extends == self.model.native_error_type()
                {
                    return Ok(self.model.native_error_type());
                }
                if self.model.is_wildcard_type(check) || self.model.is_wildcard_type(extends) {
                    return Ok(self.model.wildcard_type());
                }
                let deferred = sequence.read(self.conditional_deferred_completion(
                    check,
                    root.recipe.check_tuples,
                    consumer,
                ))?;
                let combined = if root.infer_parameters.is_empty() {
                    None
                } else {
                    Some(sequence.read(self.conditional_inference_mapper_completion(
                        root.infer_parameters.clone(),
                        (!deferred).then_some(check),
                        extends,
                        mapper,
                        None,
                        consumer,
                    ))?)
                };
                let inferred_extends = match combined {
                    Some(combined) => sequence.read(self.instantiate_type_completion(
                        root.extends_type,
                        combined,
                        consumer,
                    ))?,
                    None => extends,
                };
                if !deferred
                    && !sequence.read(self.conditional_deferred_completion(
                        inferred_extends,
                        root.recipe.check_tuples,
                        consumer,
                    ))?
                {
                    let extends_top = self
                        .model
                        .has_type_flags(inferred_extends, TypeFlags::ANY | TypeFlags::UNKNOWN);
                    let check_any = self.model.has_type_flags(check, TypeFlags::ANY);
                    let definitely_false = if extends_top {
                        false
                    } else if check_any {
                        true
                    } else {
                        let source = sequence
                            .read(self.permissive_instantiation_completion(check, consumer))?;
                        let target = sequence.read(
                            self.permissive_instantiation_completion(inferred_extends, consumer),
                        )?;
                        !sequence.read(self.type_related_completion(
                            source,
                            target,
                            RelationKind::Assignable,
                            consumer,
                        ))?
                    };
                    if definitely_false {
                        let mut include_true = check_any;
                        if !include_true
                            && for_constraint
                            && !self
                                .model
                                .has_type_flags(inferred_extends, TypeFlags::NEVER)
                        {
                            let permissive =
                                sequence.read(self.permissive_instantiation_completion(
                                    inferred_extends,
                                    consumer,
                                ))?;
                            let types = self
                                .model
                                .union_constituents(permissive)
                                .unwrap_or_else(|| Arc::from([permissive]));
                            for &candidate in types.iter() {
                                let target = sequence.read(
                                    self.permissive_instantiation_completion(check, consumer),
                                )?;
                                if sequence.read(self.type_related_completion(
                                    candidate,
                                    target,
                                    RelationKind::Assignable,
                                    consumer,
                                ))? {
                                    include_true = true;
                                    break;
                                }
                            }
                        }
                        if include_true {
                            let ty = sequence
                                .read(self.check_type_completion(&root.recipe.true_type))?;
                            extra.push(sequence.read(self.instantiate_optional_completion(
                                ty,
                                combined.or(mapper),
                                consumer,
                            ))?);
                        }
                        let ty =
                            sequence.read(self.check_type_completion(&root.recipe.false_type))?;
                        if let TypeView::Conditional(view) = self.model.get(ty)? {
                            let next = &self.conditional.roots[view.root.0];
                            if next.recipe.parent == Some(root.recipe.declaration)
                                && (!next.distributive || next.check_type == root.check_type)
                            {
                                id = view.root;
                                continue;
                            }
                            if let Some((next, next_mapper)) = sequence
                                .read(self.tail_recursion_root_completion(view, mapper, consumer))?
                            {
                                id = next;
                                mapper = Some(next_mapper);
                                alias = None;
                                tails +=
                                    usize::from(self.conditional.roots[next.0].alias.is_some());
                                continue;
                            }
                        }
                        break sequence
                            .read(self.instantiate_optional_completion(ty, mapper, consumer))?;
                    }
                    let definitely_true = if extends_top {
                        true
                    } else {
                        let source = sequence
                            .read(self.restrictive_instantiation_completion(check, consumer))?;
                        let target = sequence.read(
                            self.restrictive_instantiation_completion(inferred_extends, consumer),
                        )?;
                        sequence.read(self.type_related_completion(
                            source,
                            target,
                            RelationKind::Assignable,
                            consumer,
                        ))?
                    };
                    if definitely_true {
                        let ty =
                            sequence.read(self.check_type_completion(&root.recipe.true_type))?;
                        let true_mapper = combined.or(mapper);
                        if let TypeView::Conditional(view) = self.model.get(ty)?
                            && let Some((next, next_mapper)) = sequence.read(
                                self.tail_recursion_root_completion(view, true_mapper, consumer),
                            )?
                        {
                            id = next;
                            mapper = Some(next_mapper);
                            alias = None;
                            tails += usize::from(self.conditional.roots[next.0].alias.is_some());
                            continue;
                        }
                        break sequence.read(self.instantiate_optional_completion(
                            ty,
                            true_mapper,
                            consumer,
                        ))?;
                    }
                }
                // newConditionalType re-instantiates the root header, rather than
                // copying getActualTypeVariable's temporary check above.
                let check_type = sequence.read(self.instantiate_optional_completion(
                    root.check_type,
                    mapper,
                    consumer,
                ))?;
                let extends_type = sequence.read(self.instantiate_optional_completion(
                    root.extends_type,
                    mapper,
                    consumer,
                ))?;
                let alias = match alias {
                    Some(alias) => Some(alias),
                    None => match mapper {
                        Some(mapper) => sequence.read(self.instantiate_type_alias_completion(
                            root.alias.clone(),
                            mapper,
                            consumer,
                        ))?,
                        None => root.alias.clone(),
                    },
                };
                break self.model.conditional_type(
                    ConditionalView {
                        root: id,
                        declaration: root.recipe.ordering,
                        check_type,
                        extends_type,
                        mapper,
                        combined_mapper: combined,
                    },
                    alias,
                );
            };
            if extra.is_empty() {
                Ok(result)
            } else {
                extra.push(result);
                sequence.read(self.union_type_ex_completion(extra, None, None, consumer))
            }
        })();
        sequence.finish(result)
    }

    pub(super) fn instantiate_optional_completion(
        &mut self,
        ty: TypeId,
        mapper: Option<MapperId>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = match mapper {
            Some(mapper) => sequence.read(self.instantiate_type_completion(ty, mapper, consumer)),
            None => Ok(ty),
        };
        sequence.finish(result)
    }

    /// `prependTypeMapping` applies two Map operations. The incoming target
    /// must not be recursively instantiated through the existing mapper.
    pub(super) fn prepend_type_mapping(
        &mut self,
        source: TypeId,
        target: TypeId,
        mapper: Option<MapperId>,
    ) -> Result<MapperId, DemandError> {
        let mapping = self.model.mapper(vec![source], vec![target])?;
        Ok(match mapper {
            Some(mapper) => self.model.merge_mappers(mapping, mapper),
            None => mapping,
        })
    }

    fn conditional_deferred_completion(
        &mut self,
        ty: TypeId,
        check_tuples: bool,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if sequence.read(self.is_generic_type_completion(ty, consumer))? {
                return Ok(true);
            }
            if check_tuples && self.model.tuple_target(ty)?.is_some() {
                for &element in sequence.read(self.get_element_types_completion(ty))?.iter() {
                    if sequence.read(self.is_generic_type_completion(element, consumer))? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    fn tail_recursion_root_completion(
        &mut self,
        view: ConditionalView,
        mapper: Option<MapperId>,
        consumer: &Location,
    ) -> NativeResult<Option<(ConditionalRootId, MapperId)>> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let Some(mapper) = mapper else {
                return Ok(None);
            };
            let root = self.conditional.roots[view.root.0].data.clone();
            if root.outer_parameters.is_empty() {
                return Ok(None);
            }
            let mapper = match view.mapper {
                Some(first) => self.model.combine_mappers(first, mapper),
                None => mapper,
            };
            let arguments = root
                .outer_parameters
                .iter()
                .map(|&parameter| {
                    sequence.read(self.map_parameter_completion(parameter, mapper, consumer))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let next = self
                .model
                .mapper(root.outer_parameters.clone(), arguments)?;
            if !root.distributive {
                return Ok(Some((view.root, next)));
            }
            let check =
                sequence.read(self.map_parameter_completion(root.check_type, next, consumer))?;
            Ok((check == root.check_type
                || !self
                    .model
                    .has_type_flags(check, TypeFlags::UNION | TypeFlags::NEVER))
            .then_some((view.root, next)))
        })();
        sequence.finish(result)
    }

    /// `getConditionalTypeInstantiation`: one root cache partitions actual
    /// arguments, alias and the constraint mode before native distribution.
    pub(super) fn conditional_instantiation_completion(
        &mut self,
        ty: TypeId,
        mapper: MapperId,
        for_constraint: bool,
        alias: Option<Alias>,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut prefix = NativeSequence::new();
        let result = (|| {
            let TypeView::Conditional(view) = self.model.get(ty)? else {
                return Err(super::super::internal(
                    "conditional instantiation lacks conditional header",
                )
                .into());
            };
            let root = self.conditional.roots[view.root.0].data.clone();
            if root.outer_parameters.is_empty() {
                return Ok(ty);
            }
            // Native maps the outer arguments on every invocation, before a
            // cache hit. The cache below owns only the subsequent worker.
            let mut arguments = Vec::with_capacity(root.outer_parameters.len());
            for &parameter in &root.outer_parameters {
                arguments
                    .push(prefix.read(self.map_parameter_completion(parameter, mapper, consumer))?);
            }
            let key = InstantiationKey::new(&arguments, alias.as_ref(), for_constraint);
            if let Some(result) = self.conditional.roots[view.root.0].instantiations.get(&key) {
                return prefix.read(result.clone());
            }
            let mut sequence = NativeSequence::new();
            let result = (|| {
                let mapper = self
                    .model
                    .mapper(root.outer_parameters.clone(), arguments)?;
                let distribution = if root.distributive {
                    let check = sequence.read(self.map_parameter_completion(
                        root.check_type,
                        mapper,
                        consumer,
                    ))?;
                    Some(sequence.read(self.reduced_type_completion(check, consumer))?)
                } else {
                    None
                };
                if let Some(distribution) = distribution
                    && distribution != root.check_type
                    && self
                        .model
                        .has_type_flags(distribution, TypeFlags::UNION | TypeFlags::NEVER)
                {
                    sequence.read(self.map_type_with_alias_completion(
                        distribution,
                        alias,
                        consumer,
                        |checker, constituent| {
                            let mapper = checker.prepend_type_mapping(
                                root.check_type,
                                constituent,
                                Some(mapper),
                            )?;
                            checker.conditional_type_completion(
                                view.root,
                                Some(mapper),
                                for_constraint,
                                None,
                                consumer,
                            )
                        },
                    ))
                } else {
                    sequence.read(self.conditional_type_completion(
                        view.root,
                        Some(mapper),
                        for_constraint,
                        alias,
                        consumer,
                    ))
                }
            })();
            let result = sequence.finish(result);
            self.conditional.roots[view.root.0]
                .instantiations
                .insert(key, result.clone());
            prefix.read(result)
        })();
        prefix.finish(result)
    }

    pub(super) fn conditional_true_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.conditional_branch_completion(ty, Branch::True, consumer)
    }

    pub(super) fn conditional_false_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.conditional_branch_completion(ty, Branch::False, consumer)
    }

    pub(super) fn conditional_inferred_true_type_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.conditional_branch_completion(ty, Branch::InferredTrue, consumer)
    }

    pub(super) fn default_conditional_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.conditional_branch_completion(ty, Branch::DefaultConstraint, consumer)
    }

    /// Native getTrue/False/InferredTrueTypeFromConditionalType and
    /// getDefaultConstraintOfConditionalType share lazy branch storage here.
    /// A cached value retains the acquisitions that established that branch.
    fn conditional_branch_completion(
        &mut self,
        ty: TypeId,
        branch: Branch,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if let Some(result) = self.conditional.branches.get(&(ty, branch)) {
            return result.clone();
        }
        let TypeView::Conditional(view) = self.model.get(ty)? else {
            return Err(
                super::super::internal("conditional branch lacks conditional header").into(),
            );
        };
        let root = self.conditional.roots[view.root.0].data.clone();
        let mut sequence = NativeSequence::new();
        let result = (|| match branch {
            Branch::True | Branch::False => {
                let value =
                    sequence.read(self.check_type_completion(if branch == Branch::True {
                        &root.recipe.true_type
                    } else {
                        &root.recipe.false_type
                    }))?;
                sequence.read(self.instantiate_optional_completion(value, view.mapper, consumer))
            }
            Branch::InferredTrue => match view.combined_mapper {
                Some(mapper) => {
                    let value =
                        sequence.read(self.check_type_completion(&root.recipe.true_type))?;
                    sequence.read(self.instantiate_type_completion(value, mapper, consumer))
                }
                None => sequence.read(self.conditional_true_type_completion(ty, consumer)),
            },
            Branch::DefaultConstraint => {
                let true_type =
                    sequence.read(self.conditional_inferred_true_type_completion(ty, consumer))?;
                let false_type =
                    sequence.read(self.conditional_false_type_completion(ty, consumer))?;
                if self.model.has_type_flags(true_type, TypeFlags::ANY) {
                    Ok(false_type)
                } else if self.model.has_type_flags(false_type, TypeFlags::ANY) {
                    Ok(true_type)
                } else {
                    sequence.read(self.union_type_ex_completion(
                        vec![true_type, false_type],
                        None,
                        None,
                        consumer,
                    ))
                }
            }
        })();
        let result = sequence.finish(result);
        self.conditional
            .branches
            .insert((ty, branch), result.clone());
        result
    }

    pub(super) fn conditional_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| match sequence
            .read(self.distributive_conditional_constraint_completion(ty, consumer))?
        {
            Some(ty) => Ok(ty),
            None => sequence.read(self.default_conditional_constraint_completion(ty, consumer)),
        })();
        sequence.finish(result)
    }
    pub(super) fn distributive_conditional_constraint_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<Option<TypeId>> {
        if let Some(result) = self.conditional.distributive_constraints.get(&ty) {
            return result.clone();
        }
        let TypeView::Conditional(view) = self.model.get(ty)? else {
            return Err(super::super::internal("conditional constraint lacks header").into());
        };
        let mut sequence = NativeSequence::new();
        let result = (|| {
            if self.conditional.roots[view.root.0].distributive
                && match self.conditional.restrictive.get(&ty) {
                    // Native observes the installed identity without acquiring
                    // the instantiation again. A failed question is not nil.
                    Some(Ok(result)) => *result.value() != ty,
                    Some(Err(error)) => return Err(error.clone()),
                    None => true,
                }
            {
                let simplified = sequence.read(self.simplified_type_completion(
                    view.check_type,
                    false,
                    consumer,
                ))?;
                let constraint = if simplified == view.check_type {
                    sequence.read(self.constraint_of_type_completion(simplified, consumer))?
                } else {
                    Some(simplified)
                };
                if let Some(constraint) = constraint
                    && constraint != view.check_type
                {
                    let root_check = self.conditional.roots[view.root.0].check_type;
                    let mapper = self.prepend_type_mapping(root_check, constraint, view.mapper)?;
                    let instantiated = sequence.read(
                        self.conditional_instantiation_completion(ty, mapper, true, None, consumer),
                    )?;
                    if !self.model.has_type_flags(instantiated, TypeFlags::NEVER) {
                        return Ok(Some(instantiated));
                    }
                }
            }
            Ok(None)
        })();
        let result = sequence.finish(result);
        self.conditional
            .distributive_constraints
            .insert(ty, result.clone());
        result
    }

    pub(super) fn permissive_instantiation_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self.model.has_type_flags(
            ty,
            TypeFlags::PRIMITIVE | TypeFlags::ANY | TypeFlags::UNKNOWN | TypeFlags::NEVER,
        ) {
            return Ok(NativeCompletion::clean(ty));
        }
        if let Some(result) = self.conditional.permissive.get(&ty) {
            return result.clone();
        }
        let result = self.instantiate_type_completion(ty, self.model.permissive_mapper(), consumer);
        self.conditional.permissive.insert(ty, result.clone());
        result
    }
    pub(super) fn restrictive_instantiation_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        if self.model.has_type_flags(
            ty,
            TypeFlags::PRIMITIVE | TypeFlags::ANY | TypeFlags::UNKNOWN | TypeFlags::NEVER,
        ) {
            return Ok(NativeCompletion::clean(ty));
        }
        if let Some(result) = self.conditional.restrictive.get(&ty) {
            return result.clone();
        }
        let result =
            self.instantiate_type_completion(ty, self.model.restrictive_mapper(), consumer);
        self.conditional.restrictive.insert(ty, result.clone());
        if let Ok(completed) = &result {
            let restricted = *completed.value();
            // The native self marker acquires nothing. If both keys coincide,
            // retain the invocation's evidence instead of overwriting it.
            if restricted != ty {
                self.conditional
                    .restrictive
                    .insert(restricted, Ok(NativeCompletion::clean(restricted)));
            }
        }
        result
    }
    pub(super) fn restrictive_parameter(&mut self, ty: TypeId) -> Result<TypeId, DemandError> {
        if !self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER) {
            return Ok(ty);
        }
        if self.parameter_has_no_declared_constraint(ty)? {
            return Ok(ty);
        }
        if let Some(&result) = self.conditional.restrictive_parameters.get(&ty) {
            return Ok(result);
        }
        let TypeView::TypeParameter(mut parameter) = self.model.get(ty)? else {
            unreachable!()
        };
        let symbol = self.model.type_symbol(ty);
        parameter.constraint = None;
        parameter.default = None;
        parameter.is_this = false;
        let result = self.model.type_parameter(symbol, parameter);
        self.register_restrictive_parameter(result, ty)?;
        self.conditional.restrictive_parameters.insert(ty, result);
        Ok(result)
    }

    pub(super) fn actual_type_variable(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.actual_type_variable_completion(ty, consumer)?.admit()
    }

    pub(super) fn actual_type_variable_completion(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| match self.model.get(ty)? {
            TypeView::Substitution(view) => {
                sequence.read(self.actual_type_variable_completion(view.base, consumer))
            }
            TypeView::IndexedAccess(view)
                if self
                    .model
                    .has_type_flags(view.object_type, TypeFlags::SUBSTITUTION)
                    || self
                        .model
                        .has_type_flags(view.index_type, TypeFlags::SUBSTITUTION) =>
            {
                let object = sequence
                    .read(self.actual_type_variable_completion(view.object_type, consumer))?;
                let index = sequence
                    .read(self.actual_type_variable_completion(view.index_type, consumer))?;
                sequence.read(self.indexed_access_type_completion(object, index, consumer))
            }
            _ => Ok(ty),
        })();
        sequence.finish(result)
    }

    /// `getSimplifiedConditionalType` acquires both branches before testing
    /// the two identity-preserving filter forms. Writing is forwarded only
    /// when the selected branch itself is simplified.
    pub(super) fn simplified_conditional_type_completion(
        &mut self,
        ty: TypeId,
        writing: bool,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let TypeView::Conditional(view) = self.model.get(ty)? else {
                return Err(
                    super::super::internal("conditional simplification lacks header").into(),
                );
            };
            let true_type = sequence.read(self.conditional_true_type_completion(ty, consumer))?;
            let false_type = sequence.read(self.conditional_false_type_completion(ty, consumer))?;
            if self.model.has_type_flags(false_type, TypeFlags::NEVER)
                && sequence.read(self.actual_type_variable_completion(true_type, consumer))?
                    == sequence
                        .read(self.actual_type_variable_completion(view.check_type, consumer))?
            {
                if self.model.has_type_flags(view.check_type, TypeFlags::ANY)
                    || sequence
                        .read(self.restrictive_conditional_related_completion(view, consumer))?
                {
                    return sequence
                        .read(self.simplified_type_completion(true_type, writing, consumer));
                }
                if sequence.read(self.is_intersection_empty_completion(
                    view.check_type,
                    view.extends_type,
                    consumer,
                ))? {
                    return Ok(self.model.special(SpecialName::Never));
                }
            } else if self.model.has_type_flags(true_type, TypeFlags::NEVER)
                && sequence.read(self.actual_type_variable_completion(false_type, consumer))?
                    == sequence
                        .read(self.actual_type_variable_completion(view.check_type, consumer))?
            {
                if !self.model.has_type_flags(view.check_type, TypeFlags::ANY)
                    && sequence
                        .read(self.restrictive_conditional_related_completion(view, consumer))?
                {
                    return Ok(self.model.special(SpecialName::Never));
                }
                if self.model.has_type_flags(view.check_type, TypeFlags::ANY)
                    || sequence.read(self.is_intersection_empty_completion(
                        view.check_type,
                        view.extends_type,
                        consumer,
                    ))?
                {
                    return sequence
                        .read(self.simplified_type_completion(false_type, writing, consumer));
                }
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    fn restrictive_conditional_related_completion(
        &mut self,
        view: ConditionalView,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let source = sequence
                .read(self.restrictive_instantiation_completion(view.check_type, consumer))?;
            let target = sequence
                .read(self.restrictive_instantiation_completion(view.extends_type, consumer))?;
            sequence.read(self.type_related_completion(
                source,
                target,
                RelationKind::Assignable,
                consumer,
            ))
        })();
        sequence.finish(result)
    }

    /// `isIntersectionEmpty` deliberately invokes union reduction after the
    /// intersection, since a raw intersection need not yet reduce to Never.
    pub(super) fn is_intersection_empty_completion(
        &mut self,
        source: TypeId,
        target: TypeId,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let intersection =
                sequence.read(self.intersection_type_completion(vec![source, target], consumer))?;
            let never = self.model.special(SpecialName::Never);
            let reduced = sequence.read(self.union_type_ex_completion(
                vec![intersection, never],
                None,
                None,
                consumer,
            ))?;
            Ok(self.model.has_type_flags(reduced, TypeFlags::NEVER))
        })();
        sequence.finish(result)
    }

    pub(super) fn conditional_flow_type_completion(
        &mut self,
        ty: TypeId,
        node: ExpressionRoot,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            let mut constraints = Vec::new();
            for step in self.source.conditional_flow_steps(node)? {
                match step {
                    ConditionalFlowStep::Constraint {
                        covariant,
                        check,
                        extends,
                    } if covariant || self.model.has_type_flags(ty, TypeFlags::TYPE_VARIABLE) => {
                        let check = sequence.read(self.check_type_completion(&check))?;
                        let actual_check =
                            sequence.read(self.actual_type_variable_completion(check, consumer))?;
                        if actual_check
                            == sequence.read(self.actual_type_variable_completion(ty, consumer))?
                        {
                            constraints.push(sequence.read(self.check_type_completion(&extends))?);
                        }
                    }
                    ConditionalFlowStep::Mapped { declaration }
                        if self.model.has_type_flags(ty, TypeFlags::TYPE_PARAMETER) =>
                    {
                        let mapped = sequence
                            .read(self.mapped_type_node_completion(declaration, consumer))?;
                        let parameter =
                            sequence.read(self.mapped_parameter_completion(mapped, consumer))?;
                        if parameter
                            == sequence.read(self.actual_type_variable_completion(ty, consumer))?
                        {
                            if let Some(variable) = sequence
                                .read(self.homomorphic_type_variable_completion(mapped, consumer))?
                            {
                                if let Some(constraint) = sequence.read(
                                    self.parameter_constraint_completion(variable, consumer),
                                )? {
                                    let types = self
                                        .model
                                        .union_constituents(constraint)
                                        .unwrap_or_else(|| vec![constraint].into());
                                    let mut all = true;
                                    for &ty in types.iter() {
                                        if !sequence
                                            .read(self.is_array_or_tuple_type_completion(ty))?
                                        {
                                            all = false;
                                            break;
                                        }
                                    }
                                    if all {
                                        let number = self
                                            .model
                                            .primitive(crate::types::PrimitiveName::Number);
                                        let numeric = self.model.numeric_string_type();
                                        constraints.push(sequence.read(
                                            self.union_type_ex_completion(
                                                vec![number, numeric],
                                                None,
                                                None,
                                                consumer,
                                            ),
                                        )?);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            if constraints.is_empty() {
                Ok(ty)
            } else {
                let constraint =
                    sequence.read(self.intersection_type_completion(constraints, consumer))?;
                Ok(self.model.substitution_type(ty, constraint))
            }
        })();
        sequence.finish(result)
    }
}

impl ConditionalState {
    /// Request-local backing for the existing native questions. Keys, values
    /// and provisional publication remain with their individual operations.
    pub(super) fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            roots: Default::default(),
            branches: HashMap::new(),
            branch_demands: HashMap::new(),
            distributive_constraints: HashMap::new(),
            permissive: HashMap::new(),
            restrictive: HashMap::new(),
            restrictive_parameters: HashMap::new(),
            deferred_arguments: Default::default(),
            no_infer_consumers: Default::default(),
            checked_infer_parameters: HashMap::new(),
        }
    }
}
