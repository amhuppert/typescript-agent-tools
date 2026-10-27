//! One request owns semantic syntax, factories and canonical question caches.
//! Operations consume that state; presentation never supplies checker context.

mod access;
mod alias_diagnostics;
mod aliases;
mod anonymous;
mod arrays;
mod calls;
mod conditional;
mod constants;
mod constraints;
pub(super) mod constructs;
mod contextual;
mod declared;
mod discriminants;
mod effects;
pub(super) mod emit_helpers;
mod enums;
mod expression;
mod facts;
mod functions;
mod generics;
mod globals;
mod import_types;
pub(super) use globals::ReferenceDisplay;
mod indexed_access;
mod inference;
mod instantiation_expressions;
mod intersections;
mod iteration;
mod javascript;
mod json;
mod keyof;
mod literal_values;
pub(super) mod mapped;
mod members;
mod named_members;
mod narrowing;
mod native_display;
pub(super) use native_display::property_key_name;
mod declarations;
pub(super) mod initialization;
mod module_exports;
mod name_map;
mod names;
mod node_types;
mod object_literals;
mod operators;
mod parameter_references;
mod promises;
mod property_names;
mod property_symbols;
mod reads;
mod reference_relationships;
mod reference_symbols;
mod symbol_accessibility;
mod symbol_aliases;
mod symbol_tables;
mod symbol_type_parameters;
pub(super) use property_symbols::SymbolParent;
pub(super) use reference_symbols::{ReferenceDeclaration, ReferenceSymbol};
pub(super) use symbol_accessibility::SymbolMeaning;
pub(super) mod relations;
mod resolution;
mod rest;
mod returns;
mod signatures;
mod spelling;
mod spread;
mod structure;
mod super_expression;
mod templates;
mod this;
mod tuples;
mod type_checks;
mod type_diagnostics;
mod unions;
mod unique_symbols;
mod variables;
mod widening;

use super::super::binding::{BoundMember, BoundMemberKind, BoundSymbol, BoundValueType};
use super::inputs::CheckerInputs;
use super::model::{
    DemandError, DemandId, MetadataView, Model, NativeCompletion, NativeResult, TypeDemand, TypeId,
    TypeView,
};
use super::selection::{Selected, SelectedSeed};
use super::source::{
    SignatureQuestion, SignatureReturnRecipe, SignatureRoot, Source, TypeExpression,
};
use crate::analysis::{AnalysisError, Parser, SemanticFailureReason};
use crate::references::Location;
use std::collections::HashMap;

pub(super) struct Evaluator<'p, 'w> {
    control: Option<crate::resources::Control>,
    inputs: CheckerInputs<'p>,
    source: Source<'p, 'w>,
    model: Model,
    value_types: HashMap<BoundSymbol, TypeState>,
    declared_types: HashMap<crate::project::binding::NativeSymbolIdentity, TypeState>,
    resolution: resolution::TypeResolution,
    effects: effects::EffectsCache,
    enums: enums::EnumsState,
    emit_helpers: emit_helpers::EmitHelpersState,
    calls: calls::CallsState,
    relations: relations::RelationsState,
    discriminants: discriminants::DiscriminantsState,
    inference: inference::InferenceContexts,
    intersections: intersections::IntersectionsState,
    unions: unions::UnionsState,
    indexed_access: indexed_access::IndexedAccessState,
    keyof: keyof::KeyofState,
    mapped: mapped::MappedState,
    tuples: tuples::TuplesState,
    templates: templates::TemplatesState,
    unique_symbols: unique_symbols::UniqueSymbolsState,
    structure: structure::StructureState,
    signatures: signatures::SignaturesState,
    generics: generics::GenericsState,
    constraints: constraints::ConstraintsState,
    conditional: conditional::ConditionalState,
    globals: globals::GlobalsState,
    initialization: initialization::Initialization,
    widening: widening::WideningState,
    variables: variables::VariablesState,
    promises: promises::PromisesState,
    iteration: iteration::IterationState,
    contextual: contextual::ContextualState,
    reads: reads::ReadCache,
    reference_symbols: reference_symbols::ReferenceSymbolsState,
    symbol_tables: symbol_tables::SymbolTables,
    access_references: access::AccessReferencesState,
    node_types: node_types::NodeTypesState,
    missing_properties: access::missing::MissingPropertiesState,
    object_literals: object_literals::ObjectLiteralsState,
    symbol_aliases: symbol_aliases::SymbolAliasesState,
    module_exports: module_exports::ModuleExports,
    expressions: expression::ExpressionCache,
    functions: functions::FunctionsState,
    type_checks: type_checks::TypeChecksState,
    native_display: native_display::NativeDisplayState,
    symbol_accessibility: symbol_accessibility::State,
    native_members: members::MemberValuesState,
    anonymous: anonymous::AnonymousState,
    aliases: aliases::AliasesState,
    import_types: import_types::ImportTypesState,
    signature_demands: HashMap<(SignatureRoot, SignatureQuestion), DemandId>,
    demands: Vec<TypeDemandState>,
}

/// Display metadata carries the argument question, not prematurely acquired
/// child types. Only the evaluator can resolve the whole native vector.
pub(super) struct GraphTypeArguments {
    length: usize,
    recipe: GraphArgumentRecipe,
}
enum GraphArgumentRecipe {
    Known(std::sync::Arc<[super::model::TypeDemand]>),
    Reference(TypeId),
}
impl GraphTypeArguments {
    pub(super) fn len(&self) -> usize {
        self.length
    }
    pub(super) fn known_arguments(&self) -> Option<&[super::model::TypeDemand]> {
        match &self.recipe {
            GraphArgumentRecipe::Known(arguments) => Some(arguments),
            GraphArgumentRecipe::Reference(_) => None,
        }
    }
}

#[derive(Clone)]
enum TypeState {
    Resolving,
    /// The actual native property, including source-free recovery evidence.
    /// A later interrupted invocation cannot erase an installed property.
    Installed(super::model::NativeCompletion<TypeId>),
    Failed(DemandError),
}

impl TypeState {
    /// `typeResolutionHasProperty` sees explicitly installed native recovery
    /// types too. A product failure's category alone cannot establish a cache
    /// property: its operation may have stopped before native installation.
    fn has_type(&self) -> bool {
        match self {
            Self::Installed(_) => true,
            Self::Resolving | Self::Failed(_) => false,
        }
    }
}

struct TypeDemandState {
    recipe: DemandRecipe,
    // None only for an already installed source-free native recovery.
    location: Option<Location>,
    state: Option<TypeState>,
}

#[derive(Clone)]
enum DemandRecipe {
    Type(Box<TypeExpression>),
    PropertySymbolValue {
        symbol: super::model::PropertySymbolId,
        writing: bool,
        consumer: Location,
    },
    ParameterDisplay {
        read_type: TypeDemand,
    },
    ParameterCall {
        read_type: TypeDemand,
    },
    Return {
        root: SignatureRoot,
        recipe: SignatureReturnRecipe,
    },
    /// A fresh native signature return whose target/composite/declaration
    /// recipe is selected by its reverse-attached actual SignatureId.
    SignatureReturn,
    Generic(generics::GenericDemand),
}

impl<'p, 'w> Evaluator<'p, 'w> {
    pub(super) fn new(inputs: CheckerInputs<'p>, parser: &'w Parser) -> Self {
        let control = inputs.control.cloned();
        Self {
            control: control.clone(),
            inputs,
            source: Source::new(inputs, parser),
            model: Model::new(
                inputs.semantic_options().strict_null_checks,
                inputs.semantic_options().exact_optional_property_types,
                control.clone(),
            ),
            value_types: HashMap::new(),
            declared_types: HashMap::new(),
            resolution: resolution::TypeResolution::default(),
            effects: effects::EffectsCache::default(),
            enums: enums::EnumsState::default(),
            emit_helpers: emit_helpers::EmitHelpersState::new(),
            calls: calls::CallsState::new(),
            relations: relations::RelationsState::new(),
            discriminants: discriminants::DiscriminantsState::default(),
            inference: inference::InferenceContexts::default(),
            intersections: intersections::IntersectionsState::default(),
            unions: unions::UnionsState::new(),
            indexed_access: indexed_access::IndexedAccessState::new(),
            keyof: keyof::KeyofState::new(),
            mapped: mapped::MappedState::new(),
            tuples: tuples::TuplesState::new(),
            templates: templates::TemplatesState::default(),
            unique_symbols: unique_symbols::UniqueSymbolsState::new(),
            structure: structure::StructureState::new(),
            signatures: signatures::SignaturesState::new(),
            generics: generics::GenericsState::new(),
            constraints: constraints::ConstraintsState::new(),
            conditional: conditional::ConditionalState::new(),
            globals: globals::GlobalsState::default(),
            initialization: initialization::Initialization::default(),
            widening: widening::WideningState::new(),
            variables: variables::VariablesState::default(),
            promises: promises::PromisesState::new(),
            iteration: iteration::IterationState::default(),
            contextual: contextual::ContextualState::default(),
            reads: reads::ReadCache::default(),
            reference_symbols: reference_symbols::ReferenceSymbolsState::default(),
            symbol_tables: symbol_tables::SymbolTables::new(),
            access_references: access::AccessReferencesState::default(),
            node_types: node_types::NodeTypesState::default(),
            missing_properties: access::missing::MissingPropertiesState::default(),
            object_literals: object_literals::ObjectLiteralsState::default(),
            symbol_aliases: symbol_aliases::SymbolAliasesState::default(),
            module_exports: module_exports::ModuleExports::default(),
            expressions: expression::ExpressionCache::default(),
            functions: functions::FunctionsState::default(),
            type_checks: type_checks::TypeChecksState::default(),
            native_display: native_display::NativeDisplayState::default(),
            symbol_accessibility: symbol_accessibility::State::default(),
            native_members: members::MemberValuesState::new(),
            anonymous: anonymous::AnonymousState::new(),
            aliases: aliases::AliasesState::new(),
            import_types: import_types::ImportTypesState::default(),
            signature_demands: HashMap::new(),
            demands: Vec::new(),
        }
    }

    pub(super) fn current_native_facts(&self) -> impl crate::project::binding::NativeFacts + '_ {
        names::CurrentFacts { checker: self }
    }

    pub(super) fn checker_inputs(&self) -> CheckerInputs<'p> {
        self.inputs
    }

    pub(super) fn source(&mut self) -> &mut Source<'p, 'w> {
        &mut self.source
    }

    pub(super) fn describe(&self, ty: TypeId) -> Result<TypeView, DemandError> {
        self.model.get(ty)
    }

    pub(super) fn metadata(&self, ty: TypeId) -> Result<MetadataView, DemandError> {
        self.model.metadata(ty)
    }

    pub(super) fn signature_origins_snapshot(
        &self,
        signature: &super::model::Signature,
    ) -> Vec<Location> {
        Model::copy_origins(&signature.origins)
    }

    /// Q4 labels use the actual alias before an ordinary nominal Type.symbol.
    /// `getAliasSymbolForTypeNode` and native declaration order already own
    /// those facts; display must not infer a declaring symbol from a type's
    /// members, source-use site, or printed spelling. This asks no child type.
    /// The caller's consumer locates metadata failures only. A nominal enum or
    /// class this parameter need not have an anonymous-object association.
    pub(super) fn graph_label_origin(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<Option<Location>, DemandError> {
        let metadata = self.model.metadata(ty)?;
        if let Some(alias) = metadata.alias {
            return Ok(alias.provenance.origins.first().cloned());
        }
        let nominal = match self.model.get(ty)? {
            TypeView::Reference(reference) => {
                self.reference_display(reference.ty)? == ReferenceDisplay::Ordinary
            }
            _ => {
                self.type_symbol_flags(ty)?
                    .is_some_and(|flags| flags.class || flags.enum_ || flags.value_module)
                    || metadata.nominal_name.is_some()
                    || matches!(
                        metadata.function_symbol,
                        Some(super::model::FunctionSymbolSerialization::NonLocal)
                    )
            }
        };
        if !nominal {
            return Ok(None);
        }
        if matches!(
            self.model.type_symbol(ty),
            super::model::TypeSymbol::Absent | super::model::TypeSymbol::Synthetic { .. }
        ) {
            return Ok(None);
        }
        let Some(symbol) = self.reference_symbol_of_type(ty, consumer)? else {
            return Ok(None);
        };
        self.reference_symbol_first_declaration(symbol, consumer)?
            .map(|declaration| self.reference_declaration_location(declaration))
            .transpose()
    }

    /// Publish a request-local demand in its arena.
    /// Native question caches attach the returned handle after this completes.
    fn register_demand(&mut self, entry: TypeDemandState) -> DemandId {
        let id = DemandId::new(self.demands.len());
        self.demands.push(entry);
        id
    }

    /// Cache one native signature question at its actual declaration root.
    /// Parameter and return reads are separate from member-read identity and
    /// are not keyed by output origins or by their eventual type values.
    fn register_signature_demand(
        &mut self,
        root: SignatureRoot,
        question: SignatureQuestion,
        recipe: DemandRecipe,
    ) -> TypeDemand {
        if let Some(&demand) = self.signature_demands.get(&(root, question)) {
            return TypeDemand::Deferred(demand);
        }
        let demand = self.register_demand(TypeDemandState {
            recipe,
            location: Some(super::location(self.inputs, root.file, root.span())),
            state: None,
        });
        self.signature_demands.insert((root, question), demand);
        TypeDemand::Deferred(demand)
    }

    /// Graph argument cardinality is established metadata. Alias arguments
    /// take precedence, including a present empty list; unresolved reference
    /// arguments remain one native collection question until display admission.
    pub(super) fn graph_type_arguments(
        &self,
        ty: TypeId,
    ) -> Result<Option<GraphTypeArguments>, DemandError> {
        let metadata = self.model.metadata(ty)?;
        if let Some(arguments) = metadata
            .alias
            .as_ref()
            .and_then(|alias| alias.arguments.as_ref())
        {
            return Ok((!arguments.is_empty()).then(|| GraphTypeArguments {
                length: arguments.len(),
                recipe: GraphArgumentRecipe::Known(
                    arguments
                        .iter()
                        .copied()
                        .map(super::model::TypeDemand::Known)
                        .collect(),
                ),
            }));
        }
        if let Some(arguments) = self.model.reference_arguments_for_graph(ty)? {
            return Ok((arguments.length != 0).then(|| GraphTypeArguments {
                length: arguments.length,
                recipe: match arguments.established {
                    Some(arguments) => GraphArgumentRecipe::Known(
                        arguments
                            .iter()
                            .copied()
                            .map(super::model::TypeDemand::Known)
                            .collect(),
                    ),
                    None => GraphArgumentRecipe::Reference(ty),
                },
            }));
        }
        Ok(metadata
            .type_arguments
            .filter(|arguments| !arguments.is_empty())
            .map(|arguments| GraphTypeArguments {
                length: arguments.len(),
                recipe: GraphArgumentRecipe::Known(arguments),
            }))
    }

    /// Tuple row previews observe the actual reference argument slot, without
    /// alias substitution or consuming a deferred argument dependency.
    pub(super) fn reference_arguments_for_graph(
        &self,
        ty: TypeId,
    ) -> Result<Option<super::model::ReferenceArgumentsForGraph>, DemandError> {
        self.model.reference_arguments_for_graph(ty)
    }

    pub(super) fn resolve_graph_type_arguments(
        &mut self,
        arguments: &GraphTypeArguments,
    ) -> Result<std::sync::Arc<[super::model::TypeDemand]>, DemandError> {
        match &arguments.recipe {
            GraphArgumentRecipe::Known(arguments) => Ok(arguments.clone()),
            GraphArgumentRecipe::Reference(ty) => Ok(self
                .get_type_arguments(*ty)?
                .iter()
                .copied()
                .map(super::model::TypeDemand::Known)
                .collect()),
        }
    }

    /// Product admission is separate from the native demand/cache owner.
    pub(super) fn demand(&mut self, demand: TypeDemand) -> Result<TypeId, DemandError> {
        self.demand_completion(demand)?.admit()
    }

    /// Resolve the native question once, preserving its actual installed value
    /// and source-free evidence. Cache replay never mutates stored evidence.
    pub(super) fn demand_completion(&mut self, demand: TypeDemand) -> NativeResult<TypeId> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let TypeDemand::Deferred(id) = demand else {
            let TypeDemand::Known(ty) = demand else {
                unreachable!()
            };
            return Ok(NativeCompletion::clean(ty));
        };
        if let Some(signature) = self.signatures.return_signature(id) {
            return self.resolve_signature_return_completion(signature);
        }
        let entry = self.demands.get(id.index()).ok_or_else(|| {
            DemandError::from(super::internal("type demand has no request-local recipe"))
        })?;
        // A demand handle names the actual symbol property; it is not another
        // resolvedType cache. In particular, a diagnostic interruption after
        // native installation must not leave a shadow Failed wrapper entry.
        if let DemandRecipe::PropertySymbolValue {
            symbol,
            writing,
            consumer,
        } = &entry.recipe
        {
            let (symbol, writing, consumer) = (*symbol, *writing, consumer.clone());
            return if writing {
                self.property_write_type_completion(symbol, &consumer)
            } else {
                self.property_raw_type_completion(symbol, &consumer)
            };
        }
        match &entry.state {
            Some(TypeState::Installed(value)) => {
                return Ok(value.clone());
            }
            Some(TypeState::Failed(error)) => return Err(error.clone()),
            Some(TypeState::Resolving) => {
                return Err(super::unsupported_demand_at_location(
                    self.inputs,
                    entry
                        .location
                        .as_ref()
                        .ok_or_else(|| super::internal("unresolved demand has no source"))?,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "native circular member-read recovery is pending",
                ));
            }
            None => {}
        }
        let recipe = entry.recipe.clone();
        let location = entry
            .location
            .clone()
            .ok_or_else(|| super::internal("unresolved demand has no source"))?;
        self.demands[id.index()].state = Some(TypeState::Resolving);
        let result = match &recipe {
            DemandRecipe::Type(annotation) => self.check_type_completion(annotation),
            DemandRecipe::PropertySymbolValue { .. } => {
                unreachable!("canonical symbol question dispatched before wrapper cache")
            }
            DemandRecipe::ParameterDisplay { read_type }
            | DemandRecipe::ParameterCall { read_type } => {
                self.optional_parameter_read_completion(*read_type, &location)
            }
            DemandRecipe::Return { root, recipe } => {
                self.check_signature_return_completion(*root, recipe)
            }
            DemandRecipe::SignatureReturn => Err(super::internal(
                "synthetic signature return demand lacks its signature association",
            )
            .into()),
            DemandRecipe::Generic(recipe) => self.check_generic_demand_completion(recipe),
        };
        let result = result.map(|value| {
            if let Some(TypeState::Installed(inner)) = &self.demands[id.index()].state {
                // Native final nil selection is not another acquisition.
                return value.map(|_| *inner.value());
            }
            value
        });
        if !self.demands[id.index()]
            .state
            .as_ref()
            .is_some_and(TypeState::has_type)
        {
            self.demands[id.index()].state = Some(match &result {
                Ok(value) => TypeState::Installed(value.clone()),
                Err(error) => TypeState::Failed(error.clone()),
            });
        }
        result
    }

    pub(super) fn establish(&mut self, selected: &Selected) -> Result<TypeId, AnalysisError> {
        // Alias selection precedes the checking prelude. Its evidence must
        // accompany later work even when that prelude interrupts the demand.
        let selected_prefix = match &selected.seed {
            SelectedSeed::ResolvedSymbol { symbol } => symbol.clone().map(|_| ()),
            _ => NativeCompletion::clean(()),
        };
        let result = selected_prefix.and_then_value(|_| {
            self.check_site_prelude(&selected.checking_prelude)?;
            match &selected.seed {
                SelectedSeed::Type(expression) => self.check_type(expression),
                SelectedSeed::PendingValue {
                    root,
                    reason,
                    message,
                    ..
                } => Err(super::unsupported_demand_at_location(
                    self.inputs,
                    &super::location(self.inputs, root.file, root.span()),
                    *reason,
                    message.clone(),
                )),
                SelectedSeed::Expression(expression) => self.regular_type_of_expression(expression),
                // `GetTypeOfSymbolAtLocation` of an actual typeof operand asks
                // its expression/read operation, not the declaration-value cache.
                SelectedSeed::ValueAt(expression) => self.type_of_source_expression(expression),
                SelectedSeed::ResolvedSymbol { symbol } => match selected.target.kind {
                    crate::types::TypeQuestion::Value => {
                        self.value_reference_type(*symbol.value(), &selected.target.location)
                    }
                    crate::types::TypeQuestion::Declared => {
                        self.declared_reference_type(*symbol.value(), &selected.target.location)
                    }
                    _ => Err(
                        super::internal("resolved alias seed has no declaration question").into(),
                    ),
                },
            }
        });
        result
            .and_then(NativeCompletion::admit)
            .map_err(AnalysisError::from)
    }

    /// `getTypeOfSymbol` / `getTypeOfVariableOrParameterOrProperty` retain one
    /// value result per canonical symbol, separate from its declared type.
    /// Syntax attachment and annotation-before-initializer precedence remain
    /// Source's existing getTypeForVariableLikeDeclaration operation.
    pub(super) fn value_type_completion(
        &mut self,
        symbol: BoundSymbol,
        family: &BoundValueType,
    ) -> NativeResult<TypeId> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()));
        let symbol = self
            .source
            .bindings()
            .value_type_symbol(symbol)
            .map_err(|error| self.symbol_association_failure(symbol, error))?;
        let property = if matches!(
            family,
            BoundValueType::Member(_) | BoundValueType::Prototype { .. }
        ) {
            Some(self.register_property_symbol(symbol)?)
        } else {
            None
        };
        let result = self.value_type_worker(symbol, family);
        if let Some(property) = property {
            let question = super::model::PropertyValueQuestion::Read(property);
            result
                .map(|value| value.observe(|failure| failure.at_property(question)))
                .map_err(|failure| failure.at_property(question))
        } else {
            result
        }
    }

    fn value_type_worker(
        &mut self,
        symbol: BoundSymbol,
        family: &BoundValueType,
    ) -> NativeResult<TypeId> {
        match self.value_types.get(&symbol) {
            Some(TypeState::Installed(value)) => {
                return Ok(value.clone());
            }
            Some(TypeState::Failed(error)) => return Err(error.clone()),
            Some(TypeState::Resolving) | None => {}
        }
        let result = match family {
            BoundValueType::Undefined => {
                Ok(NativeCompletion::clean(self.model.undefined_initializer()))
            }
            BoundValueType::EnumMember(declaration) => self.declared_type_completion(
                symbol,
                crate::project::binding::BoundDeclaredType::EnumMember(*declaration),
            ),
            BoundValueType::Module => self.module_value_type(symbol).map(NativeCompletion::clean),
            BoundValueType::Json { .. }
            | BoundValueType::Variable(_)
            | BoundValueType::Member(BoundMember {
                kind: BoundMemberKind::Property,
                ..
            }) => self.bound_variable_or_property_type_completion(symbol, family),
            BoundValueType::Class(declaration) => {
                let consumer = super::location(self.inputs, declaration.file, declaration.anchor);
                self.class_value_type(symbol, &consumer)
                    .map(NativeCompletion::clean)
            }
            BoundValueType::Prototype { parent } => self
                .prototype_property_type(*parent)
                .map(NativeCompletion::clean),
            BoundValueType::Function(declarations) => {
                if matches!(self.value_types.get(&symbol), Some(TypeState::Resolving)) {
                    let first = declarations
                        .first()
                        .ok_or_else(|| super::internal("canonical function has no declarations"))?;
                    return Err(super::unsupported_demand_at_location(
                        self.inputs,
                        &super::location(self.inputs, first.file, first.anchor),
                        SemanticFailureReason::UnsupportedSemanticForm,
                        "native early function member construction for this re-entry is pending",
                    ));
                }
                self.value_types.insert(symbol, TypeState::Resolving);
                self.function_value_type(symbol, declarations)
                    .map(NativeCompletion::clean)
            }
            BoundValueType::Member(member) => match member.kind {
                BoundMemberKind::Accessor => self.accessor_read_type_completion(symbol, member),
                BoundMemberKind::Method => self
                    .method_value_type(symbol, member)
                    .map(NativeCompletion::clean),
                BoundMemberKind::Property => {
                    unreachable!("property has the variable-like native owner")
                }
            },
        };
        // The native variable wrapper returns its local result, while a prior
        // contextual installation remains the value for subsequent cache hits.
        // Other families own their own final nil checks before this boundary.
        if self
            .value_types
            .get(&symbol)
            .is_some_and(TypeState::has_type)
        {
            return result;
        }
        let result = result.and_then(|value| {
            value.and_then(|ty| {
                if let BoundValueType::Variable(declaration) = family {
                    let consumer =
                        super::location(self.inputs, declaration.file, declaration.anchor);
                    if self.is_parameter_of_context_sensitive_signature(symbol.into(), &consumer)? {
                        return Ok(NativeCompletion::clean((ty, false)));
                    }
                }
                Ok(NativeCompletion::clean((ty, true)))
            })
        });
        match result {
            Ok(value) => {
                let install = value.value().1;
                let value = value.map(|(ty, _)| ty);
                if install {
                    self.value_types
                        .insert(symbol, TypeState::Installed(value.clone()));
                } else {
                    self.value_types.remove(&symbol);
                }
                Ok(value)
            }
            Err(error) => {
                self.value_types
                    .insert(symbol, TypeState::Failed(error.clone()));
                Err(error)
            }
        }
    }
}
