//! Identifier checking and restricted native flow interpretation. Exact
//! lookup, read setup, type interpretation and reachability remain distinct
//! questions over the same immutable graph.

mod assignments;
mod invocations;
use invocations::{FlowType, ReadInvocations};
mod before_use;
mod deprecated;
mod equality;
mod narrowed;
mod reference;
pub(super) use reference::{FlowNodeRef, FlowReferenceQuestion, ReferenceNode};
mod capture;
mod constant;
mod loops;
mod reachability;
mod switches;

use super::super::model::{
    DemandError, NativeCompletion, NativeResult, Predicate, TypeFlags, TypeId, UnionReduction,
};
use super::super::source::{Expression, ExpressionRoot, VariableReadFacts};
use super::Evaluator;
use super::facts::TypeFacts;
use crate::Span;
use crate::analysis::SemanticFailureReason;
use crate::project::binding::TypeBindingFailure;
use crate::project::binding::flow::{ContainerId, FlowId, Graph, Node, Site};
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use crate::types::{TypePredicateKind, TypePredicateSubject};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AssignmentKind {
    None,
    Definite,
    Compound,
}

#[derive(Default)]
pub(super) struct ReadCache {
    analysis_disabled: bool,
    inline_level: u8,
    narrowed: narrowed::NarrowedState,
    pub(super) invocations: usize,
    switches: switches::SwitchCache,
    capture: capture::CaptureCache,
    assignments: HashMap<(TypeId, TypeId), NativeCompletion<TypeId>>,
    pub(super) loops: loops::LoopCache,
    references: reference::ReferenceState,
    reachable: HashMap<(FileId, FlowId), NativeCompletion<bool>>,
    last_reachable: Option<(FileId, FlowId, NativeCompletion<bool>)>,
}

impl ReadCache {
    pub(super) fn has_initializer_undefined(
        &self,
        declaration: super::super::source::DeclarationIdentity,
    ) -> bool {
        self.narrowed.has_initializer_undefined(declaration)
    }
}

enum ReadWork {
    VisitCreated {
        frame: usize,
        flow: reference::CreatedFlowId,
    },
    Visit {
        frame: usize,
        flow: FlowId,
    },
    Reduce {
        frame: usize,
        antecedent: usize,
        reduction: FlowId,
    },
    Compound {
        frame: usize,
        antecedent: usize,
    },
    Condition {
        frame: usize,
        antecedent: usize,
        expression: Site,
        truthy: bool,
    },
    Loop(loops::LoopWork),
    Switch {
        frame: usize,
        antecedent: usize,
        statement: Site,
        start: usize,
        end: usize,
    },
    Branch {
        frame: usize,
        flow: FlowId,
        index: usize,
        antecedent: usize,
        types: Vec<TypeId>,
        subtype_reduction: bool,
        bypass: Option<FlowId>,
        incomplete: bool,
    },
    Bypass {
        frame: usize,
        antecedent: usize,
        statement: Site,
        types: Vec<TypeId>,
        subtype_reduction: bool,
        incomplete: bool,
    },
    Assertion {
        frame: usize,
        antecedent: usize,
        expression: Site,
        predicate: Predicate,
    },
}

enum AssertionWork {
    Visit { ty: TypeId, site: Site },
    AndRight(Site),
    OrRight { ty: TypeId, site: Site },
    Union(TypeId),
}

/// The explicit `getTypeAtFlowNode` continuation stack.
struct ReadStack(Vec<ReadWork>);

impl ReadStack {
    fn new() -> Self {
        Self(Vec::new())
    }
    fn push(&mut self, task: ReadWork) {
        self.0.push(task);
    }
    fn pop(&mut self) -> Option<ReadWork> {
        self.0.pop()
    }
}

impl ReadWork {
    fn frame(&self) -> usize {
        match self {
            Self::VisitCreated { frame, .. }
            | Self::Visit { frame, .. }
            | Self::Reduce { frame, .. }
            | Self::Compound { frame, .. }
            | Self::Condition { frame, .. }
            | Self::Switch { frame, .. }
            | Self::Branch { frame, .. }
            | Self::Bypass { frame, .. }
            | Self::Assertion { frame, .. } => *frame,
            Self::Loop(task) => task.frame(),
        }
    }
}

struct ReadState<'a> {
    file: FileId,
    flow_container: Option<ContainerId>,
    reference: ReferenceNode,
    declared: TypeId,
    initial: TypeId,
    admitted: TypeId,
    regular_expression: bool,
    literal_base_admission: bool,
    location: &'a Location,
}

impl Evaluator<'_, '_> {
    /// `checkIdentifier`: acquire the actual raw symbol and its value type,
    /// then the variable/alias read operation. Functions use the native
    /// non-variable early return, while aliases retain their own flow question.
    pub(super) fn check_identifier_completion(
        &mut self,
        expression: &Expression,
        file: FileId,
        anchor: Span,
        mode: super::contextual::CheckMode,
    ) -> NativeResult<TypeId> {
        let inputs = self.inputs;
        let root = super::super::source::ExpressionRoot {
            file,
            site: Site {
                span: anchor,
                kind: SyntaxKind::Identifier,
            },
        };
        let syntax = self.source.flow_syntax(file, root.site)?;
        if syntax.this_identifier && syntax.in_type_query {
            return self.check_this_expression_completion(root, &expression.location);
        }
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let raw_symbol = sequence.read(
                self.reference_resolved_identifier_symbol_completion(root, &expression.location),
            )?;
            if self.is_unknown_reference_symbol(raw_symbol) {
                return Ok(self.model.native_error_type());
            }
            let symbol = self
                .reference_export_symbol_of_value_if_exported(raw_symbol, &expression.location)?;
            sequence.read(self.check_identifier_deprecation(symbol, root, &expression.location))?;
            let immediate =
                self.reference_symbol_value_declaration(symbol, &expression.location)?;
            let immediate = match immediate {
                Some(super::ReferenceDeclaration::Captured(declaration)) => Some(declaration),
                Some(super::ReferenceDeclaration::Json { .. }) | None => None,
            };
            if let Some(declaration) = immediate
                && self.is_contextual_binding_reference(declaration, root)?
            {
                return Ok(self.model.non_inferrable_any_type());
            }
            let mut declared = sequence.read(self.narrowed_symbol_type_completion(
                symbol,
                root,
                &expression.location,
            ))?;
            let assignment = self.assignment_kind(file, syntax.assignment_target)?;
            let flags = self.reference_symbol_flags(symbol, &expression.location)?;
            if assignment != AssignmentKind::None {
                let readonly = if flags.variable {
                    let property = self.register_reference_symbol(symbol, &expression.location)?;
                    sequence.read(
                        self.property_is_readonly_completion(property, &expression.location),
                    )?
                } else {
                    false
                };
                if !flags.variable || readonly {
                    sequence.read(
                        self.diagnostic_symbol_display_completion(raw_symbol, &expression.location),
                    )?;
                    return sequence.read(NativeCompletion::recovered(
                        self.model.native_error_type(),
                        self.assignment_error(
                            &expression.location,
                            "native identifier assignment consumes errorType",
                        ),
                    ));
                }
            }
            let declaration = if flags.variable {
                if assignment == AssignmentKind::Definite {
                    return if self.is_in_compound_like_assignment(file, syntax.assignment_target)? {
                        sequence.read(
                            self.base_type_of_literal_completion(declared, &expression.location),
                        )
                    } else {
                        Ok(declared)
                    };
                }
                immediate
            } else if flags.alias {
                // Native uses the original identifier's Alias declaration,
                // including mixed aliases; it never takes the value target's.
                match self.reference_alias_declaration(raw_symbol, &expression.location)? {
                    Some(super::ReferenceDeclaration::Captured(declaration)) => Some(declaration),
                    Some(super::ReferenceDeclaration::Json { .. }) => {
                        return Err(super::super::internal(
                            "native alias declaration cannot be a JSON declaration",
                        )
                        .into());
                    }
                    None => None,
                }
            } else {
                return Ok(declared);
            };
            if declaration.is_none() {
                return Ok(declared);
            }
            let read_flow = self
                .source
                .bindings()
                .value_reference_flow(file, anchor, syntax.in_type_query)
                .map_err(|failure| self.read_binding_failure(&expression.location, failure))?;
            let mut flow_container = read_flow.container;
            let mut parameter = false;
            let setup = if flags.variable {
                // ValueDeclaration is syntax provenance even when merging
                // created this symbol. Its actual node carries the binder's
                // flow container; no retained symbol substitutes its identity.
                let declaration = immediate.expect("variable declaration selected above");
                let setup = self.source.variable_declaration_read_facts(declaration)?;
                parameter = setup.root_kind == SyntaxKind::Parameter;
                declared = sequence.read(self.narrowable_type_for_reference_completion(
                    declared,
                    root,
                    mode,
                    &expression.location,
                ))?;
                let declaration_root = self.source.declaration_expression_root(declaration)?;
                let declaration_read = inputs
                    .bindings
                    .flow_attachment(declaration_root.file, declaration_root.site)
                    .map_err(|failure| self.read_binding_failure(&expression.location, failure))?;
                let outer =
                    file != declaration.file || read_flow.container != declaration_read.container;
                let graph = inputs
                    .bindings
                    .flow_graph(file)
                    .map_err(|failure| self.read_binding_failure(&expression.location, failure))?;
                while file != declaration.file || flow_container != declaration_read.container {
                    let Some(extension) = graph.capture_extension(flow_container) else {
                        break;
                    };
                    let eligible = self
                        .is_constant_variable_symbol(symbol, &expression.location)?
                        && !self.model.is_automatic_array(declared)
                        || is_parameter_or_mutable_local_variable(&setup)
                            && sequence.read(self.is_past_last_assignment_completion(
                                symbol,
                                immediate.expect("variable declaration selected above"),
                                file,
                                root.site,
                                &expression.location,
                            ))?;
                    if !eligible {
                        break;
                    }
                    flow_container = extension.container;
                }
                let never_initialized = setup.declaration_kind == SyntaxKind::VariableDeclaration
                    && !setup.for_in_or_of
                    && !setup.initialized
                    && !setup.definite
                    && is_mutable_local_variable_declaration(&setup)
                    && !sequence.read(self.is_symbol_assigned_definitely_completion(
                        symbol,
                        immediate.expect("variable declaration selected above"),
                        &expression.location,
                    ))?;
                parameter
                    || setup.declaration_flags & crate::compiler::node::AMBIENT != 0
                    || setup.definite
                    || outer && !never_initialized
            } else {
                declared = sequence.read(self.narrowable_type_for_reference_completion(
                    declared,
                    root,
                    mode,
                    &expression.location,
                ))?;
                true
            };
            let (same_scope, ambient_type, spread_target) = self
                .source
                .identifier_initialization_context(root, declaration)?;
            let automatic = self.model.is_automatic(declared);
            let direct_nonnull = syntax
                .parent
                .is_some_and(|parent| parent.kind == SyntaxKind::NonNullExpression);
            let automatic_in_nonnull = automatic && direct_nonnull;
            // checkIdentifier's isAlias contributes independently even when
            // its earlier Variable branch selected ValueDeclaration.
            let assume_initialized = setup
                || flags.alias
                || same_scope
                || spread_target
                || direct_nonnull
                || !automatic
                    && (!inputs.semantic_options().strict_null_checks
                        || syntax.in_type_query
                        || ambient_type
                        || syntax
                            .parent
                            .is_some_and(|parent| parent.kind == SyntaxKind::ExportSpecifier)
                        || self.model.has_type_flags(
                            declared,
                            TypeFlags::ANY | TypeFlags::UNKNOWN | TypeFlags::VOID,
                        ));
            let initial = if automatic_in_nonnull {
                self.model
                    .special(super::super::model::SpecialName::Undefined)
            } else if assume_initialized && parameter {
                sequence.read(self.remove_parameter_optionality_completion(
                    declared,
                    declaration.ok_or_else(|| {
                        super::super::internal("parameter read has no declaration")
                    })?,
                    &expression.location,
                ))?
            } else if assume_initialized {
                declared
            } else if automatic {
                self.model
                    .special(super::super::model::SpecialName::Undefined)
            } else {
                sequence.read(self.optional_type_completion(
                    declared,
                    false,
                    &expression.location,
                ))?
            };
            if self.reads.analysis_disabled {
                return sequence.read(NativeCompletion::recovered(
                    self.model.native_error_type(),
                    self.native_flow_depth_error(&expression.location),
                ));
            }
            let mut flow_type = if let Some(flow) = read_flow.flow {
                let state = ReadState {
                    file,
                    flow_container: Some(flow_container),
                    reference: root.into(),
                    declared,
                    initial,
                    admitted: if assume_initialized {
                        initial
                    } else {
                        declared
                    },
                    regular_expression: self.is_regular_expression_reference(file, anchor),
                    literal_base_admission: true,
                    location: &expression.location,
                };
                sequence.read(self.read_flow_completion(&state, flow))?
            } else {
                declared
            };
            if automatic_in_nonnull {
                flow_type = sequence
                    .read(self.non_nullable_type_completion(flow_type, &expression.location))?;
            }
            if automatic {
                // Evolving-array operation targets and auto conversion are a
                // separate native flow producer, still excluded by admission.
                return Err(self.flow_required(
                    &expression.location,
                    "automatic use-site evolution requires excluded flow analysis",
                ));
            }
            if !assume_initialized
                && !self.model.contains_undefined(declared)
                && self.model.contains_undefined(flow_type)
            {
                sequence.read(
                    self.diagnostic_symbol_display_completion(raw_symbol, &expression.location),
                )?;
                return Ok(declared);
            }
            if assignment != AssignmentKind::None {
                sequence.read(self.base_type_of_literal_completion(flow_type, &expression.location))
            } else {
                Ok(flow_type)
            }
        })();
        sequence.finish(result)
    }

    /// `getFlowTypeOfReferenceEx` for an access: its native flowContainer is
    /// nil, and its own bindWorker attachment decides whether traversal runs.
    pub(super) fn access_reference_flow_completion(
        &mut self,
        root: super::super::source::ExpressionRoot,
        declared: TypeId,
        initial: TypeId,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.flow_type_of_reference_with_admission_completion(
            FlowReferenceQuestion {
                file: root.file,
                reference: root.into(),
                declared,
                initial,
                admitted: declared,
                container: None,
                flow: None,
            },
            consumer,
            true,
        )
    }

    /// `getAssignmentTargetKind`: target syntax comes from the shared
    /// `GetAssignmentTarget` owner; native definite and compound kinds remain
    /// separate from whether the read's graph contains a matching assignment.
    pub(super) fn assignment_kind(
        &mut self,
        file: FileId,
        target: Option<Site>,
    ) -> Result<AssignmentKind, DemandError> {
        let Some(target) = target else {
            return Ok(AssignmentKind::None);
        };
        let syntax = self.source.flow_syntax(file, target)?;
        match syntax.kind {
            SyntaxKind::BinaryExpression => {
                let operator = syntax
                    .operator
                    .ok_or_else(|| super::super::internal("assignment target has no operator"))?;
                Ok(
                    if matches!(
                        operator,
                        SyntaxKind::EqualsToken
                            | SyntaxKind::BarBarEqualsToken
                            | SyntaxKind::AmpersandAmpersandEqualsToken
                            | SyntaxKind::QuestionQuestionEqualsToken
                    ) {
                        AssignmentKind::Definite
                    } else {
                        AssignmentKind::Compound
                    },
                )
            }
            SyntaxKind::PrefixUnaryExpression | SyntaxKind::PostfixUnaryExpression => {
                Ok(AssignmentKind::Compound)
            }
            SyntaxKind::ForInStatement | SyntaxKind::ForOfStatement => Ok(AssignmentKind::Definite),
            _ => Err(
                super::super::internal("GetAssignmentTarget returned a non-assignment role").into(),
            ),
        }
    }

    /// `isInCompoundLikeAssignment` / `isCompoundLikeAssignment` /
    /// `isShiftOperatorOrHigher`. Only a simple assignment whose parenthesis-
    /// skipped RHS has this native arithmetic precedence widens its LHS type.
    fn is_in_compound_like_assignment(
        &mut self,
        file: FileId,
        target: Option<Site>,
    ) -> Result<bool, DemandError> {
        let Some(target) = target else {
            return Ok(false);
        };
        let syntax = self.source.flow_syntax(file, target)?;
        if !syntax.simple_assignment {
            return Ok(false);
        }
        let mut right = required(syntax.right, "simple assignment has no right")?;
        loop {
            let syntax = self.source.flow_syntax(file, right)?;
            if syntax.kind == SyntaxKind::ParenthesizedExpression {
                right = required(syntax.expression, "compound-like wrapper has no expression")?;
                continue;
            }
            return Ok(syntax.kind == SyntaxKind::BinaryExpression
                && syntax.operator.is_some_and(|operator| {
                    matches!(
                        operator,
                        SyntaxKind::LessThanLessThanToken
                            | SyntaxKind::GreaterThanGreaterThanToken
                            | SyntaxKind::GreaterThanGreaterThanGreaterThanToken
                            | SyntaxKind::PlusToken
                            | SyntaxKind::MinusToken
                            | SyntaxKind::AsteriskToken
                            | SyntaxKind::SlashToken
                            | SyntaxKind::PercentToken
                            | SyntaxKind::AsteriskAsteriskToken
                    )
                }));
        }
    }

    pub(super) fn base_literal_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.base_type_of_literal(ty, consumer)
    }

    fn assignment_error(&self, consumer: &Location, message: &str) -> DemandError {
        super::super::invalid_at_location(
            self.inputs,
            consumer,
            SemanticFailureReason::ErrorType,
            message,
        )
        .into()
    }

    /// `getFlowTypeOfReferenceEx` / `getTypeAtFlowNode`: exact identity
    /// admission with native invocation depth, tail traversal and Shared cache.
    /// Intermediate assignment/loop types can restore exact final identity;
    /// annotations do not bypass native setup, traversal or admission.
    fn read_flow_completion(
        &mut self,
        state: &ReadState<'_>,
        root: impl Into<FlowNodeRef>,
    ) -> NativeResult<TypeId> {
        let checkpoint = self.loop_scope_checkpoint();
        let result = self.read_flow_inner_completion(state, root.into());
        let balanced = self.restore_loop_scopes(checkpoint);
        if result.is_ok() && !balanced {
            return Err(super::super::internal(
                "native read completed with an unfinished loop scope",
            )
            .into());
        }
        result
    }

    fn read_flow_inner_completion(
        &mut self,
        state: &ReadState<'_>,
        root: FlowNodeRef,
    ) -> NativeResult<TypeId> {
        let inputs = self.inputs;
        let graph = inputs
            .bindings
            .flow_graph(state.file)
            .map_err(|error| self.read_binding_failure(state.location, error))?;
        self.reads.invocations += 1;
        let mut invocations = ReadInvocations::new();
        let mut work = ReadStack::new();
        work.push(ReadWork::visit_reference(0, root));
        let mut active_frame = 0;
        let execution = (|| -> Result<NativeCompletion<FlowType>, DemandError> {
            while let Some(task) = work.pop() {
                active_frame = task.frame();
                match task {
                    ReadWork::VisitCreated { frame, flow } => {
                        if !invocations.frames[frame].entered {
                            if invocations.frames[frame].depth == 2000 {
                                invocations.finish_with(
                                    frame,
                                    NativeCompletion::recovered(
                                        FlowType::complete(self.model.native_error_type()),
                                        self.native_flow_depth_error(state.location),
                                    ),
                                )?;
                                continue;
                            }
                            invocations.frames[frame].entered = true;
                        }
                        match self.created_flow(flow) {
                            reference::CreatedFlow::Start => self.schedule_flow_start(
                                state,
                                graph,
                                frame,
                                None,
                                &mut invocations,
                                &mut work,
                            )?,
                            reference::CreatedFlow::Condition {
                                expression,
                                truthy,
                                antecedent,
                            } => {
                                self.schedule_flow_condition(
                                    frame,
                                    expression,
                                    truthy,
                                    antecedent,
                                    &mut invocations,
                                    &mut work,
                                );
                            }
                        }
                    }
                    ReadWork::Reduce {
                        frame,
                        antecedent,
                        reduction,
                    } => {
                        if invocations.reductions.pop() != Some(reduction) {
                            return Err(super::super::internal(
                                "native flow reduction has no matching scope",
                            )
                            .into());
                        }
                        let result = invocations.read_result(frame, antecedent)?;
                        invocations.finish(frame, result)?;
                    }
                    ReadWork::Compound { frame, antecedent } => {
                        let antecedent = invocations.read_result(frame, antecedent)?;
                        let ty = invocations.read(
                            frame,
                            self.base_type_of_literal_completion(antecedent.ty, state.location),
                        )?;
                        if antecedent.ty == state.admitted {
                            invocations.admitted_literal_base = Some(ty);
                        }
                        let result = self.new_flow_type(ty, antecedent.incomplete);
                        invocations.finish(frame, result)?;
                    }
                    ReadWork::Condition {
                        frame,
                        antecedent,
                        expression,
                        truthy,
                    } => {
                        let ty = invocations.read_result(frame, antecedent)?;
                        // `getTypeAtFlowCondition` returns Never before acquiring
                        // or narrowing the actual condition.
                        if self.model.has_type_flags(ty.ty, TypeFlags::NEVER) {
                            invocations.finish(frame, ty)?;
                            continue;
                        }
                        let non_evolving =
                            self.finalize_evolving_array_type(ty.ty, state.location)?;
                        let narrowed = invocations.read(
                            frame,
                            self.narrow_condition_completion(
                                state,
                                non_evolving,
                                ExpressionRoot {
                                    file: state.file,
                                    site: expression,
                                },
                                truthy,
                            ),
                        )?;
                        let result = if narrowed == non_evolving {
                            ty
                        } else {
                            self.new_flow_type(narrowed, ty.incomplete)
                        };
                        invocations.finish(frame, result)?;
                    }
                    ReadWork::Assertion {
                        frame,
                        antecedent,
                        expression,
                        predicate,
                    } => {
                        let antecedent = invocations.read_result(frame, antecedent)?;
                        let ty =
                            self.finalize_evolving_array_type(antecedent.ty, state.location)?;
                        let narrowed = if predicate.value_type.is_some() {
                            return Err(self.flow_required(
                                state.location,
                                "native assertion type-predicate relation admission is pending",
                            ));
                        } else if let TypePredicateSubject::ParameterIndex(index) =
                            predicate.subject
                            && let Some(argument) =
                                self.source.call_argument(state.file, expression, index)?
                        {
                            invocations.read(
                                frame,
                                self.narrow_assertion_completion(state, ty, argument),
                            )?
                        } else {
                            ty
                        };
                        let result = if narrowed == ty {
                            antecedent
                        } else {
                            self.new_flow_type(narrowed, antecedent.incomplete)
                        };
                        invocations.finish(frame, result)?;
                    }
                    ReadWork::Loop(task) => {
                        self.resume_flow_loop(state, graph, task, &mut invocations, &mut work)?;
                    }
                    ReadWork::Switch {
                        frame,
                        antecedent,
                        statement,
                        start,
                        end,
                    } => {
                        let antecedent = invocations.read_result(frame, antecedent)?;
                        let ty = invocations.read(
                            frame,
                            self.read_switch_clause_completion(
                                state,
                                antecedent.ty,
                                statement,
                                start,
                                end,
                            ),
                        )?;
                        let result = self.new_flow_type(ty, antecedent.incomplete);
                        invocations.finish(frame, result)?;
                    }
                    ReadWork::Bypass {
                        frame,
                        antecedent,
                        statement,
                        mut types,
                        mut subtype_reduction,
                        mut incomplete,
                    } => {
                        let antecedent = invocations.read_result(frame, antecedent)?;
                        let ty = antecedent.ty;
                        if !self.model.has_type_flags(ty, TypeFlags::NEVER)
                            && !types.contains(&ty)
                            && !invocations.read(
                                frame,
                                self.exhaustive_switch_completion(
                                    state.file,
                                    statement,
                                    state.location,
                                ),
                            )?
                        {
                            if ty == state.declared && state.initial == state.declared {
                                invocations.finish(frame, FlowType::complete(ty))?;
                                continue;
                            }
                            types.push(ty);
                            subtype_reduction |= !invocations.read(
                                frame,
                                self.type_subset_of_completion(ty, state.initial, state.location),
                            )?;
                            incomplete |= antecedent.incomplete;
                        }
                        let ty = invocations.read(
                            frame,
                            self.flow_branch_union_completion(state, types, subtype_reduction),
                        )?;
                        let result = self.new_flow_type(ty, incomplete);
                        invocations.finish(frame, result)?;
                    }
                    ReadWork::Branch {
                        frame,
                        flow,
                        index,
                        antecedent,
                        mut types,
                        mut subtype_reduction,
                        mut bypass,
                        mut incomplete,
                    } => {
                        let antecedent_result = invocations.read_result(frame, antecedent)?;
                        let ty = antecedent_result.ty;
                        // Native declared/initial early return precedes any later
                        // path, including the deferred switch bypass/exhaustiveness.
                        if ty == state.declared && state.initial == state.declared {
                            invocations.finish(frame, FlowType::complete(ty))?;
                            continue;
                        }
                        if !types.contains(&ty) {
                            types.push(ty);
                        }
                        subtype_reduction |= !invocations.read(
                            frame,
                            self.type_subset_of_completion(ty, state.initial, state.location),
                        )?;
                        incomplete |= antecedent_result.incomplete;
                        let antecedents = branch_antecedents(graph, flow, &invocations.reductions)?;
                        if let Some((index, next)) =
                            next_branch_path(graph, antecedents, index + 1, &mut bypass)
                        {
                            let antecedent = invocations.child(frame);
                            work.push(ReadWork::Branch {
                                frame,
                                flow,
                                index,
                                antecedent,
                                types,
                                subtype_reduction,
                                bypass,
                                incomplete,
                            });
                            work.push(ReadWork::Visit {
                                frame: antecedent,
                                flow: next,
                            });
                        } else if let Some(bypass) = bypass {
                            let Node::SwitchClause { statement, .. } = graph.node(bypass) else {
                                unreachable!("native deferred bypass is a switch clause")
                            };
                            let antecedent = invocations.child(frame);
                            work.push(ReadWork::Bypass {
                                frame,
                                antecedent,
                                statement: *statement,
                                types,
                                subtype_reduction,
                                incomplete,
                            });
                            work.push(ReadWork::Visit {
                                frame: antecedent,
                                flow: bypass,
                            });
                        } else {
                            let ty = invocations.read(
                                frame,
                                self.flow_branch_union_completion(state, types, subtype_reduction),
                            )?;
                            let result = self.new_flow_type(ty, incomplete);
                            invocations.finish(frame, result)?;
                        }
                    }
                    ReadWork::Visit { frame, flow } => {
                        // Native depth checks precede Shared lookup, once at
                        // invocation entry. Tail hops do not increment or recheck.
                        if !invocations.frames[frame].entered {
                            if invocations.frames[frame].depth == 2000 {
                                invocations.finish_with(
                                    frame,
                                    NativeCompletion::recovered(
                                        FlowType::complete(self.model.native_error_type()),
                                        self.native_flow_depth_error(state.location),
                                    ),
                                )?;
                                continue;
                            }
                            invocations.frames[frame].entered = true;
                        }
                        if graph.shared(flow) && invocations.visit_shared(frame, flow)? {
                            continue;
                        }
                        match graph.node(flow) {
                            Node::Unreachable => {
                                invocations.finish(frame, FlowType::complete(state.declared))?
                            }
                            Node::Start { container } => {
                                self.schedule_flow_start(
                                    state,
                                    graph,
                                    frame,
                                    *container,
                                    &mut invocations,
                                    &mut work,
                                )?;
                            }
                            Node::Assignment { target, antecedent } => {
                                if !invocations.read(
                                    frame,
                                    self.matching_reference_completion(
                                        state.reference,
                                        (state.file, *target),
                                        state.location,
                                    ),
                                )? {
                                    work.push(ReadWork::Visit {
                                        frame,
                                        flow: *antecedent,
                                    });
                                    continue;
                                }
                                if !invocations.read(
                                    frame,
                                    self.reachable_flow_completion(
                                        state.file,
                                        flow,
                                        state.location,
                                    ),
                                )? {
                                    invocations.finish(
                                        frame,
                                        FlowType::complete(self.model.unreachable_never()),
                                    )?;
                                    continue;
                                }
                                let target_syntax = self.source.flow_syntax(state.file, *target)?;
                                if self
                                    .assignment_kind(state.file, target_syntax.assignment_target)?
                                    == AssignmentKind::Compound
                                {
                                    let child = invocations.child(frame);
                                    work.push(ReadWork::Compound {
                                        frame,
                                        antecedent: child,
                                    });
                                    work.push(ReadWork::Visit {
                                        frame: child,
                                        flow: *antecedent,
                                    });
                                    continue;
                                }
                                if self.model.is_automatic(state.declared) {
                                    return Err(self.flow_required(
                                    state.location,
                                    "matching automatic assignment requires excluded flow analysis",
                                ));
                                }
                                let ty = if self.is_in_compound_like_assignment(
                                    state.file,
                                    target_syntax.assignment_target,
                                )? {
                                    let ty = invocations.read(
                                        frame,
                                        self.base_type_of_literal_completion(
                                            state.declared,
                                            state.location,
                                        ),
                                    )?;
                                    if state.declared == state.admitted {
                                        invocations.admitted_literal_base = Some(ty);
                                    }
                                    ty
                                } else {
                                    state.declared
                                };
                                let ty = if self.model.has_type_flags(ty, TypeFlags::UNION) {
                                    let assigned = invocations.read(
                                        frame,
                                        self.initial_or_assigned_type_completion(
                                            state.file,
                                            *target,
                                            state.reference,
                                            state.location,
                                        ),
                                    )?;
                                    invocations.read(
                                        frame,
                                        self.assignment_reduced_type_completion(
                                            ty,
                                            assigned,
                                            state.location,
                                        ),
                                    )?
                                } else {
                                    ty
                                };
                                invocations.finish(frame, FlowType::complete(ty))?;
                            }
                            Node::Condition {
                                expression,
                                truthy,
                                antecedent,
                            } => {
                                self.schedule_flow_condition(
                                    frame,
                                    *expression,
                                    *truthy,
                                    (*antecedent).into(),
                                    &mut invocations,
                                    &mut work,
                                );
                            }
                            Node::Label {
                                looping,
                                antecedents,
                            } => {
                                let antecedents = if *looping {
                                    antecedents.as_slice()
                                } else {
                                    branch_antecedents(graph, flow, &invocations.reductions)?
                                };
                                let Some(&first) = antecedents.first() else {
                                    return Err(super::super::internal(
                                        "retained reachable flow label has no antecedents",
                                    )
                                    .into());
                                };
                                if antecedents.len() == 1 {
                                    work.push(ReadWork::Visit { frame, flow: first });
                                } else {
                                    if *looping {
                                        self.begin_flow_loop(
                                            state,
                                            flow,
                                            first,
                                            frame,
                                            &mut invocations,
                                            &mut work,
                                        )?;
                                    } else {
                                        let child = invocations.child(frame);
                                        let mut bypass = None;
                                        let Some((index, first)) =
                                            next_branch_path(graph, antecedents, 0, &mut bypass)
                                        else {
                                            return Err(super::super::internal(
                                                "native branch has no ordinary path",
                                            )
                                            .into());
                                        };
                                        work.push(ReadWork::Branch {
                                            frame,
                                            flow,
                                            index,
                                            antecedent: child,
                                            types: Vec::new(),
                                            subtype_reduction: false,
                                            bypass,
                                            incomplete: false,
                                        });
                                        work.push(ReadWork::Visit {
                                            frame: child,
                                            flow: first,
                                        });
                                        continue;
                                    }
                                }
                            }
                            Node::Call {
                                expression,
                                antecedent,
                            } => {
                                if let Some(signature) = invocations.read(
                                    frame,
                                    self.effects_signature_completion(state.file, *expression),
                                )? {
                                    if let Some(predicate) = invocations.read(
                                        frame,
                                        self.type_predicate_of_signature_completion(
                                            &signature,
                                            state.location,
                                        ),
                                    )? && matches!(
                                        predicate.kind,
                                        TypePredicateKind::Asserts | TypePredicateKind::AssertsIs
                                    ) {
                                        let child = invocations.child(frame);
                                        work.push(ReadWork::Assertion {
                                            frame,
                                            antecedent: child,
                                            expression: *expression,
                                            predicate,
                                        });
                                        work.push(ReadWork::Visit {
                                            frame: child,
                                            flow: *antecedent,
                                        });
                                        continue;
                                    }
                                    let call_location = super::super::location(
                                        self.inputs,
                                        state.file,
                                        expression.span,
                                    );
                                    let returned = invocations.read(
                                        frame,
                                        self.get_return_type_of_signature_completion(
                                            &signature,
                                            &call_location,
                                        ),
                                    )?;
                                    if self.model.has_type_flags(returned, TypeFlags::NEVER) {
                                        invocations.finish(
                                            frame,
                                            FlowType::complete(self.model.unreachable_never()),
                                        )?;
                                        continue;
                                    }
                                }
                                work.push(ReadWork::Visit {
                                    frame,
                                    flow: *antecedent,
                                });
                            }
                            Node::ArrayMutation {
                                expression,
                                antecedent,
                            } => {
                                // `getTypeAtFlowArrayMutation` only acquires its
                                // receiver for the actual automatic families.
                                if self.model.is_automatic(state.declared) {
                                    let mutation =
                                        self.source.flow_syntax(state.file, *expression)?;
                                    let access = match mutation.kind {
                                        SyntaxKind::CallExpression => required(
                                            mutation.expression,
                                            "array call mutation has no target",
                                        )?,
                                        SyntaxKind::BinaryExpression => required(
                                            mutation.left,
                                            "array element mutation has no left",
                                        )?,
                                        _ => {
                                            return Err(super::super::internal(
                                                "array mutation has an invalid syntax role",
                                            )
                                            .into());
                                        }
                                    };
                                    let access = self.source.flow_syntax(state.file, access)?;
                                    let receiver = self.reference_candidate(ExpressionRoot {
                                        file: state.file,
                                        site: required(
                                            access.expression,
                                            "array mutation access has no receiver",
                                        )?,
                                    })?;
                                    if invocations.read(
                                        frame,
                                        self.matching_reference_completion(
                                            state.reference,
                                            receiver,
                                            state.location,
                                        ),
                                    )? {
                                        return Err(self.flow_required(state.location,
                                        "native evolving-array mutation consumes excluded flow analysis"));
                                    }
                                }
                                work.push(ReadWork::Visit {
                                    frame,
                                    flow: *antecedent,
                                });
                            }
                            Node::ReduceLabel { antecedent, .. } => {
                                invocations.push_reduction(flow);
                                let child = invocations.child(frame);
                                work.push(ReadWork::Reduce {
                                    frame,
                                    antecedent: child,
                                    reduction: flow,
                                });
                                work.push(ReadWork::Visit {
                                    frame: child,
                                    flow: *antecedent,
                                });
                            }
                            Node::Pending { site, message } => {
                                let cause = super::super::location(inputs, state.file, site.span);
                                return Err(self.flow_required(&cause, message));
                            }
                            Node::SwitchClause {
                                statement,
                                start,
                                end,
                                antecedent,
                            } => {
                                let child = invocations.child(frame);
                                work.push(ReadWork::Switch {
                                    frame,
                                    antecedent: child,
                                    statement: *statement,
                                    start: *start,
                                    end: *end,
                                });
                                work.push(ReadWork::Visit {
                                    frame: child,
                                    flow: *antecedent,
                                });
                            }
                        }
                    }
                }
            }
            invocations.result(0)
        })();
        let completion = match execution {
            Ok(completion) => completion,
            Err(failure) => return Err(invocations.fail(active_frame, failure)),
        };
        completion.and_then_value(|flow| {
            let ty = flow.ty;
            if self.model.is_error_type(ty) {
                return Ok(ty);
            }
            // `getFlowTypeOfReferenceEx` normalizes this distinct sentinel, not
            // ordinary Never. Admission then asks the completed read question:
            // native intermediate clause types may have been restored by a join.
            if self.model.is_unreachable_never(ty) {
                return Ok(state.declared);
            }
            if ty == state.admitted
                || state.literal_base_admission && Some(ty) == invocations.admitted_literal_base
            {
                Ok(ty)
            } else if state.regular_expression
                && self.model.regular_literal(ty) == self.model.regular_literal(state.admitted)
            {
                // `getRegularTypeOfExpression` consumes this exact root after
                // getFlowTypeOfReferenceEx; raw symbol-at-location/dependency
                // questions cannot borrow its later normalization.
                Ok(ty)
            } else {
                Err(self.flow_required(
                    state.location,
                    "native reference flow changes this read and requires excluded flow analysis",
                ))
            }
        })
    }

    /// `getUnionOrEvolvingArrayType`: one final branch union operation,
    /// including native reduction, unknown recombination and declared identity.
    fn flow_branch_union_completion(
        &mut self,
        state: &ReadState<'_>,
        types: Vec<TypeId>,
        subtype: bool,
    ) -> NativeResult<TypeId> {
        if types.iter().any(|&ty| self.model.is_automatic(ty)) {
            return Err(self.flow_required(
                state.location,
                "native branch evolving-array union consumes excluded flow analysis",
            ));
        }
        let completion = self.union_type_reduced_completion(
            types,
            if subtype {
                UnionReduction::Subtype
            } else {
                UnionReduction::Literal
            },
            None,
            None,
            state.location,
        )?;
        Ok(completion.map(|ty| {
            let ty = self.model.recombine_unknown(ty);

            if ty != state.declared
                && self.model.has_type_flags(ty, TypeFlags::UNION)
                && self.model.has_type_flags(state.declared, TypeFlags::UNION)
                && self.model.union_constituents(ty)
                    == self.model.union_constituents(state.declared)
            {
                state.declared
            } else {
                ty
            }
        }))
    }

    /// `finalizeEvolvingArrayType`: only the actual evolving factory requires
    /// final-array inference; native Auto/AutoArray sentinel identities answer
    /// a different question. All flow consumers share this dependency gate.
    fn finalize_evolving_array_type(
        &mut self,
        ty: TypeId,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        if self
            .model
            .object_flags(ty)?
            .intersects(super::super::model::ObjectFlags::EVOLVING_ARRAY)
        {
            return Err(self.flow_required(
                consumer,
                "native evolving-array finalization requires excluded flow analysis",
            ));
        }
        Ok(ty)
    }

    /// `getTypeAtFlowNode` / `getFlowTypeOfReferenceEx`: the compiler's
    /// recursion guard installs errorType and disables subsequent analysis.
    /// This is an invalid native dependency, distinct from product resources.
    fn native_flow_depth_error(&mut self, location: &Location) -> DemandError {
        self.reads.analysis_disabled = true;
        super::super::invalid_at_location(self.inputs, location,
            SemanticFailureReason::ErrorType,
            "native control-flow recursion reached 2000; the containing function or module body is too large for control flow analysis").into()
    }

    fn narrow_assertion_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        site: Site,
    ) -> NativeResult<TypeId> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let mut work = vec![AssertionWork::Visit { ty, site }];
            let mut result = ty;
            while let Some(task) = work.pop() {
                match task {
                    AssertionWork::Visit { ty, mut site } => {
                        let syntax = loop {
                            let syntax = self.source.flow_syntax(state.file, site)?;
                            if syntax.kind == SyntaxKind::ParenthesizedExpression {
                                site = required(
                                    syntax.expression,
                                    "assertion wrapper has no expression",
                                )?;
                            } else {
                                break syntax;
                            }
                        };
                        match syntax.kind {
                            SyntaxKind::FalseKeyword => result = self.model.unreachable_never(),
                            SyntaxKind::BinaryExpression
                                if syntax.operator == Some(SyntaxKind::AmpersandAmpersandToken) =>
                            {
                                work.push(AssertionWork::AndRight(required(
                                    syntax.right,
                                    "assertion AND has no right",
                                )?));
                                work.push(AssertionWork::Visit {
                                    ty,
                                    site: required(syntax.left, "assertion AND has no left")?,
                                });
                            }
                            SyntaxKind::BinaryExpression
                                if syntax.operator == Some(SyntaxKind::BarBarToken) =>
                            {
                                work.push(AssertionWork::OrRight {
                                    ty,
                                    site: required(syntax.right, "assertion OR has no right")?,
                                });
                                work.push(AssertionWork::Visit {
                                    ty,
                                    site: required(syntax.left, "assertion OR has no left")?,
                                });
                            }
                            _ => {
                                result = sequence.read(self.narrow_condition_completion(
                                    state,
                                    ty,
                                    ExpressionRoot {
                                        file: state.file,
                                        site,
                                    },
                                    true,
                                ))?;
                            }
                        }
                    }
                    AssertionWork::AndRight(site) => {
                        work.push(AssertionWork::Visit { ty: result, site })
                    }
                    AssertionWork::OrRight { ty, site } => {
                        work.push(AssertionWork::Union(result));
                        work.push(AssertionWork::Visit { ty, site });
                    }
                    AssertionWork::Union(left) => {
                        result = sequence.read(self.union_type_ex_completion(
                            vec![left, result],
                            None,
                            None,
                            state.location,
                        ))?;
                    }
                }
            }
            Ok(result)
        })();
        sequence.finish(result)
    }

    /// `isMatchingReference` for the admitted Identifier source. Alias
    /// symbols compare their own handles; declarations compare export/value
    /// canonical identity. Exact text is never a substitute for either.
    fn matches_reference_completion(
        &mut self,
        state: &ReadState<'_>,
        target: ExpressionRoot,
    ) -> NativeResult<bool> {
        self.matching_reference_completion(state.reference, target, state.location)
    }

    /// `isMatchingReference` compares exact reference structure and native raw
    /// symbol identity. Access keys use their own native name acquisition.
    fn matching_reference_completion(
        &mut self,
        source: impl Into<ReferenceNode>,
        target: impl Into<ReferenceNode>,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let mut source = source.into();
            let mut target = target.into();
            loop {
                let target_syntax = self.reference_syntax(target)?;
                match target.kind {
                    SyntaxKind::ParenthesizedExpression | SyntaxKind::NonNullExpression => {
                        target = required(
                            target_syntax.expression,
                            "matching wrapper has no expression",
                        )?;
                        continue;
                    }
                    SyntaxKind::BinaryExpression => {
                        target = if target_syntax.assignment_expression {
                            required(target_syntax.left, "matching assignment has no left")?
                        } else if target_syntax.operator == Some(SyntaxKind::CommaToken) {
                            required(target_syntax.right, "matching comma has no right")?
                        } else {
                            return Ok(false);
                        };
                        continue;
                    }
                    _ => {}
                }
                let syntax = self.reference_syntax(source)?;
                match source.kind {
                    SyntaxKind::Identifier | SyntaxKind::PrivateIdentifier => {
                        if syntax.this_identifier && syntax.in_type_query {
                            return Ok(target.kind == SyntaxKind::ThisKeyword);
                        }
                        if !matches!(
                            target.kind,
                            SyntaxKind::Identifier
                                | SyntaxKind::VariableDeclaration
                                | SyntaxKind::BindingElement
                        ) {
                            return Ok(false);
                        }
                        let selected =
                            sequence.read(self.reference_resolved_identifier_symbol_completion(
                                ExpressionRoot {
                                    file: source.file,
                                    site: source.captured()?,
                                },
                                consumer,
                            ))?;
                        if target.kind == SyntaxKind::Identifier {
                            let other = sequence.read(
                                self.reference_resolved_identifier_symbol_completion(
                                    ExpressionRoot {
                                        file: target.file,
                                        site: target.captured()?,
                                    },
                                    consumer,
                                ),
                            )?;
                            return Ok(self.reference_symbol_identity(selected, consumer)?
                                == self.reference_symbol_identity(other, consumer)?);
                        }
                        if matches!(
                            target.kind,
                            SyntaxKind::VariableDeclaration | SyntaxKind::BindingElement
                        ) {
                            let selected = if self.is_unknown_reference_symbol(selected) {
                                None
                            } else {
                                Some(self.reference_export_symbol_of_value_if_exported(
                                    selected, consumer,
                                )?)
                            };
                            let declaration = self.source.declaration_from_expression_root(
                                super::super::source::ExpressionRoot {
                                    file: target.file,
                                    site: target.captured()?,
                                },
                            )?;
                            let symbol = sequence.read(
                                self.reference_node_declaration_symbol_completion(declaration),
                            )?;
                            return match (selected, symbol) {
                                (Some(selected), Some(symbol)) => Ok(self
                                    .reference_symbol_identity(selected, consumer)?
                                    == self.reference_symbol_identity(symbol, consumer)?),
                                _ => Ok(false),
                            };
                        }
                        return Ok(false);
                    }
                    SyntaxKind::ThisKeyword | SyntaxKind::SuperKeyword => {
                        return Ok(source.kind == target.kind);
                    }
                    SyntaxKind::NonNullExpression
                    | SyntaxKind::ParenthesizedExpression
                    | SyntaxKind::SatisfiesExpression => {
                        source = required(
                            syntax.expression,
                            "matching source wrapper has no expression",
                        )?
                    }
                    SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression => {
                        if let Some(name) = sequence
                            .read(self.reference_property_name_completion(source, consumer))?
                        {
                            if matches!(
                                target.kind,
                                SyntaxKind::PropertyAccessExpression
                                    | SyntaxKind::ElementAccessExpression
                            ) {
                                if let Some(other) = sequence.read(
                                    self.reference_property_name_completion(target, consumer),
                                )? {
                                    if name != other {
                                        return Ok(false);
                                    }
                                    source = required(
                                        syntax.expression,
                                        "matching access has no receiver",
                                    )?;
                                    target = required(
                                        target_syntax.expression,
                                        "matching target access has no receiver",
                                    )?;
                                    continue;
                                }
                            }
                        }
                        if source.kind == SyntaxKind::ElementAccessExpression
                            && target.kind == SyntaxKind::ElementAccessExpression
                        {
                            let left =
                                required(syntax.argument, "matching element has no argument")?;
                            let right = required(
                                target_syntax.argument,
                                "matching element target has no argument",
                            )?;
                            if left.kind == SyntaxKind::Identifier
                                && right.kind == SyntaxKind::Identifier
                            {
                                let a = sequence.read(
                                    self.reference_resolved_identifier_symbol_completion(
                                        ExpressionRoot {
                                            file: left.file,
                                            site: left.captured()?,
                                        },
                                        consumer,
                                    ),
                                )?;
                                let b = sequence.read(
                                    self.reference_resolved_identifier_symbol_completion(
                                        ExpressionRoot {
                                            file: right.file,
                                            site: right.captured()?,
                                        },
                                        consumer,
                                    ),
                                )?;
                                if self.reference_symbol_identity(a, consumer)?
                                    == self.reference_symbol_identity(b, consumer)?
                                    && sequence
                                        .read(self.stable_element_key_completion(a, consumer))?
                                {
                                    source = required(
                                        syntax.expression,
                                        "matching access has no receiver",
                                    )?;
                                    target = required(
                                        target_syntax.expression,
                                        "matching target access has no receiver",
                                    )?;
                                    continue;
                                }
                            }
                        }
                        return Ok(false);
                    }
                    SyntaxKind::BinaryExpression
                        if syntax.operator == Some(SyntaxKind::CommaToken) =>
                    {
                        source = required(syntax.right, "matching source comma has no right")?
                    }
                    SyntaxKind::QualifiedName => {
                        if matches!(
                            target.kind,
                            SyntaxKind::PropertyAccessExpression
                                | SyntaxKind::ElementAccessExpression
                        ) && sequence
                            .read(self.reference_property_name_completion(target, consumer))?
                            == syntax.reference_name.as_ref().map(|name| {
                                crate::project::binding::NativePropertyKey::fixed(
                                    name.as_bytes(),
                                    self.control.as_ref(),
                                )
                            })
                        {
                            source = required(syntax.left, "qualified reference has no left")?;
                            target = required(
                                target_syntax.expression,
                                "matching access has no receiver",
                            )?;
                            continue;
                        }
                        return Ok(false);
                    }
                    SyntaxKind::MetaProperty => {
                        return Ok(target.kind == SyntaxKind::MetaProperty
                            && syntax.keyword_token == target_syntax.keyword_token
                            && syntax.reference_name == target_syntax.reference_name);
                    }
                    _ => return Ok(false),
                }
            }
        })();
        sequence.finish(result)
    }

    /// `isMatchingReference` / `writeFlowCacheKey` share this exact stable
    /// identifier-key eligibility, not the broader constant capture predicate.
    fn stable_element_key_completion(
        &mut self,
        symbol: super::ReferenceSymbol,
        consumer: &Location,
    ) -> NativeResult<bool> {
        if self.is_constant_variable_symbol(symbol, consumer)? {
            return Ok(NativeCompletion::clean(true));
        }
        match self.mutable_reference_declaration(symbol, consumer)? {
            Some(declaration) => self
                .is_symbol_assigned_completion(symbol, declaration, consumer)
                .map(|value| value.map(|assigned| !assigned)),
            None => Ok(NativeCompletion::clean(false)),
        }
    }

    /// `getReferenceCandidate`: parentheses and assignment/comma expressions
    /// select their native candidate. NonNull is deliberately not skipped.
    fn reference_candidate(
        &mut self,
        mut root: ExpressionRoot,
    ) -> Result<ExpressionRoot, DemandError> {
        loop {
            let syntax = self.source.flow_syntax(root.file, root.site)?;
            match syntax.kind {
                SyntaxKind::ParenthesizedExpression => {
                    root.site = required(syntax.expression, "reference wrapper has no expression")?
                }
                SyntaxKind::BinaryExpression => match syntax.operator {
                    Some(
                        SyntaxKind::EqualsToken
                        | SyntaxKind::BarBarEqualsToken
                        | SyntaxKind::AmpersandAmpersandEqualsToken
                        | SyntaxKind::QuestionQuestionEqualsToken,
                    ) => root.site = required(syntax.left, "reference assignment has no left")?,
                    Some(SyntaxKind::CommaToken) => {
                        root.site = required(syntax.right, "reference comma has no right")?
                    }
                    _ => return Ok(root),
                },
                _ => return Ok(root),
            }
        }
    }

    /// `narrowType` / `narrowTypeByBinaryExpression` / `narrowTypeByTypeof`.
    /// Exact native fact reductions can be intermediate inputs to a later
    /// identity join. Source supplies syntax; completed read admission decides
    /// whether the result would expose an excluded flow-sensitive answer.
    fn narrow_condition_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        mut root: ExpressionRoot,
        mut truthy: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            loop {
                let syntax = self.source.flow_syntax(root.file, root.site)?;
                let child = |site| ExpressionRoot {
                    file: root.file,
                    site,
                };
                // `narrowType` asks optionality before its syntax-kind dispatch:
                // an optional root receiver and a nullish left operand are not
                // ordinary truthiness conditions.
                if syntax.optionality_condition {
                    if sequence.read(self.matches_reference_completion(state, root))? {
                        return sequence.read(self.adjusted_type_with_facts_completion(
                            ty,
                            if truthy {
                                TypeFacts::NOT_NULLABLE
                            } else {
                                TypeFacts::NULLABLE
                            },
                            state.location,
                        ));
                    }
                    return sequence.read(self.narrow_discriminant_facts_completion(
                        state,
                        ty,
                        root,
                        if truthy {
                            TypeFacts::NOT_NULLABLE
                        } else {
                            TypeFacts::NULLABLE
                        },
                    ));
                }
                match syntax.kind {
                    SyntaxKind::ParenthesizedExpression
                    | SyntaxKind::NonNullExpression
                    | SyntaxKind::SatisfiesExpression => {
                        root.site =
                            required(syntax.expression, "condition wrapper has no expression")?
                    }
                    SyntaxKind::PrefixUnaryExpression
                        if syntax.unary_operator == Some(SyntaxKind::ExclamationToken.number()) =>
                    {
                        root.site = required(syntax.operand, "condition negation has no operand")?;
                        truthy = !truthy;
                    }
                    SyntaxKind::BinaryExpression => match syntax.operator {
                        Some(
                            SyntaxKind::EqualsEqualsToken
                            | SyntaxKind::ExclamationEqualsToken
                            | SyntaxKind::EqualsEqualsEqualsToken
                            | SyntaxKind::ExclamationEqualsEqualsToken,
                        ) => {
                            let left =
                                child(required(syntax.left, "binary condition has no left")?);
                            let right =
                                child(required(syntax.right, "binary condition has no right")?);
                            return sequence.read(self.narrow_equality_condition_completion(
                                state,
                                ty,
                                left,
                                right,
                                required(syntax.operator, "equality condition has no operator")?,
                                truthy,
                            ));
                        }
                        Some(SyntaxKind::CommaToken) => {
                            root.site = required(syntax.right, "comma condition has no right")?
                        }
                        Some(
                            SyntaxKind::EqualsToken
                            | SyntaxKind::BarBarEqualsToken
                            | SyntaxKind::AmpersandAmpersandEqualsToken
                            | SyntaxKind::QuestionQuestionEqualsToken,
                        ) => {
                            // `narrowTypeByBinaryExpression` checks the right
                            // condition first, then left truthiness. Neither may
                            // be skipped merely because this is an assignment.
                            let ty = sequence.read(self.narrow_condition_completion(
                                state,
                                ty,
                                child(required(syntax.right, "assignment condition has no right")?),
                                truthy,
                            ))?;
                            return sequence.read(self.narrow_truthiness_completion(
                                state,
                                ty,
                                child(required(syntax.left, "assignment condition has no left")?),
                                truthy,
                            ));
                        }
                        Some(SyntaxKind::AmpersandAmpersandToken | SyntaxKind::BarBarToken) => {
                            let left = sequence.read(self.narrow_condition_completion(
                                state,
                                ty,
                                child(required(syntax.left, "logical condition has no left")?),
                                truthy,
                            ))?;
                            let sequential = (syntax.operator
                                == Some(SyntaxKind::AmpersandAmpersandToken))
                                == truthy;
                            let right = sequence.read(self.narrow_condition_completion(
                                state,
                                if sequential { left } else { ty },
                                child(required(syntax.right, "logical condition has no right")?),
                                truthy,
                            ))?;
                            return if sequential {
                                Ok(right)
                            } else {
                                sequence.read(self.union_type_ex_completion(
                                    vec![left, right],
                                    None,
                                    None,
                                    state.location,
                                ))
                            };
                        }
                        Some(SyntaxKind::InstanceOfKeyword | SyntaxKind::InKeyword) => {
                            return Err(self.flow_required(
                                state.location,
                                "native condition narrowing admission is pending",
                            ));
                        }
                        _ => return Ok(ty),
                    },
                    SyntaxKind::Identifier => {
                        if !sequence.read(self.matches_reference_completion(state, root))?
                            && self.reads.inline_level < 5
                        {
                            if let Some(initializer) = sequence
                                .read(self.alias_condition_initializer_completion(state, root))?
                            {
                                self.reads.inline_level += 1;
                                let narrowed = self.narrow_condition_completion(
                                    state,
                                    ty,
                                    initializer,
                                    truthy,
                                );
                                // A product interruption must unwind the same dynamic
                                // inline scope as an ordinary native return.
                                self.reads.inline_level -= 1;
                                return sequence.read(narrowed);
                            }
                        }
                        return sequence
                            .read(self.narrow_truthiness_completion(state, ty, root, truthy));
                    }
                    SyntaxKind::ThisKeyword
                    | SyntaxKind::SuperKeyword
                    | SyntaxKind::PropertyAccessExpression
                    | SyntaxKind::ElementAccessExpression => {
                        return sequence
                            .read(self.narrow_truthiness_completion(state, ty, root, truthy));
                    }
                    SyntaxKind::CallExpression => {
                        return sequence
                            .read(self.narrow_call_condition_completion(state, ty, root, truthy));
                    }
                    _ => return Ok(ty),
                }
            }
        })();
        sequence.finish(result)
    }

    /// `narrowType`'s Identifier alias gate uses the raw resolved symbol,
    /// actual unannotated variable initializer, and original reference's
    /// constant status. The caller owns the checker-wide inline-level scope.
    fn alias_condition_initializer_completion(
        &mut self,
        state: &ReadState<'_>,
        root: ExpressionRoot,
    ) -> NativeResult<Option<ExpressionRoot>> {
        self.reference_resolved_identifier_symbol_completion(root, state.location)?
            .and_then(|symbol| {
                if !self.is_constant_variable_symbol(symbol, state.location)? {
                    return Ok(NativeCompletion::clean(None));
                }
                let Some(super::ReferenceDeclaration::Captured(declaration)) =
                    self.reference_symbol_value_declaration(symbol, state.location)?
                else {
                    return Ok(NativeCompletion::clean(None));
                };
                let Some(initializer) = self.source.alias_condition_initializer(declaration)?
                else {
                    return Ok(NativeCompletion::clean(None));
                };
                Ok(self
                    .constant_reference_completion(state.reference, state.location)?
                    .map(|constant| constant.then_some(initializer)))
            })
    }

    /// `narrowTypeByTruthiness` preserves exact intermediate fact reductions.
    /// Only the completed read decides whether these changes cancel at a join
    /// or require excluded flow-sensitive output. Discriminant work retains
    /// its own admission boundary and consumes the post-optionality type.
    fn narrow_truthiness_completion(
        &mut self,
        state: &ReadState<'_>,
        mut ty: TypeId,
        root: ExpressionRoot,
        truthy: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            if sequence.read(self.matches_reference_completion(state, root))? {
                return sequence.read(self.adjusted_type_with_facts_completion(
                    ty,
                    if truthy {
                        TypeFacts::TRUTHY
                    } else {
                        TypeFacts::FALSY
                    },
                    state.location,
                ));
            }
            if self.inputs.semantic_options().strict_null_checks
                && truthy
                && sequence.read(self.optional_chain_contains_completion(state, root))?
            {
                ty = sequence.read(self.adjusted_type_with_facts_completion(
                    ty,
                    TypeFacts::NOT_NULLABLE,
                    state.location,
                ))?;
            }
            sequence.read(self.narrow_discriminant_facts_completion(
                state,
                ty,
                root,
                if truthy {
                    TypeFacts::TRUTHY
                } else {
                    TypeFacts::FALSY
                },
            ))
        })();
        sequence.finish(result)
    }

    /// `optionalChainContainsReference`: ask matching at each actual receiver,
    /// without treating parentheses or arbitrary children as chain continuations.
    fn optional_chain_contains_completion(
        &mut self,
        state: &ReadState<'_>,
        mut root: ExpressionRoot,
    ) -> NativeResult<bool> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            loop {
                let syntax = self.source.flow_syntax(root.file, root.site)?;
                if !syntax.optional_chain {
                    return Ok(false);
                }
                root.site = required(syntax.expression, "optional chain has no receiver")?;
                if sequence.read(self.matching_reference_completion(
                    root,
                    state.reference,
                    state.location,
                ))? {
                    return Ok(true);
                }
            }
        })();
        sequence.finish(result)
    }

    /// `getDiscriminantPropertyAccess` preserves the native union, candidate,
    /// accessed-name, subset and synthetic-discriminant questions in order.
    fn discriminant_property_access_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        root: ExpressionRoot,
    ) -> NativeResult<Option<ExpressionRoot>> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            if !self.model.has_type_flags(state.declared, TypeFlags::UNION)
                && !self.model.has_type_flags(ty, TypeFlags::UNION)
            {
                return Ok(None);
            }
            let Some(access) = sequence
                .read(self.candidate_discriminant_property_access_completion(state, root))?
            else {
                return Ok(None);
            };
            let Some(name) = sequence.read(self.accessed_property_name_completion(
                access.file,
                access.site,
                state.location,
            ))?
            else {
                return Ok(None);
            };
            let candidate_type = if self.model.has_type_flags(state.declared, TypeFlags::UNION)
                && sequence.read(self.type_subset_of_completion(
                    ty,
                    state.declared,
                    state.location,
                ))? {
                state.declared
            } else {
                ty
            };
            Ok(
                if sequence.read(self.is_discriminant_property_by_key_completion(
                    candidate_type,
                    &name,
                    state.location,
                ))? {
                    Some(access)
                } else {
                    None
                },
            )
        })();
        sequence.finish(result)
    }

    /// `narrowTypeByDiscriminant` with the native optionality/truthiness fact
    /// callback. A changed intermediate constituent set is valid work; the
    /// completed read still owns flow-sensitive result admission.
    fn narrow_discriminant_facts_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        root: ExpressionRoot,
        facts: TypeFacts,
    ) -> NativeResult<TypeId> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let Some(access) =
                sequence.read(self.discriminant_property_access_completion(state, ty, root))?
            else {
                return Ok(ty);
            };
            sequence.read(self.narrow_discriminant_completion(
                state,
                ty,
                access,
                |checker, property| {
                    checker.type_with_facts_completion(property, facts, state.location)
                },
            ))
        })();
        sequence.finish(result)
    }

    /// `hasMatchingArgument` / `isOrContainsMatchingReference`: receiver
    /// containment and optional chains use the same matching owner as reads.
    fn has_matching_argument(
        &mut self,
        file: FileId,
        call: Site,
        reference: ReferenceNode,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        self.has_matching_argument_completion(file, call, reference, consumer)?
            .admit()
    }

    fn has_matching_argument_completion(
        &mut self,
        file: FileId,
        call: Site,
        reference: ReferenceNode,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            for mut argument in self.source.call_arguments(file, call)? {
                if sequence.read(self.is_or_contains_matching_reference_completion(
                    file, reference, argument, consumer,
                ))? {
                    return Ok(true);
                }
                loop {
                    let syntax = self.source.flow_syntax(file, argument)?;
                    if !syntax.optional_chain {
                        break;
                    }
                    argument = required(syntax.expression, "optional argument has no receiver")?;
                    // Native optionalChainContainsReference matches the chain's
                    // receiver as source and the original reference as target.
                    if sequence.read(self.matching_reference_completion(
                        (file, argument),
                        reference,
                        consumer,
                    ))? {
                        return Ok(true);
                    }
                }
            }
            let syntax = self.source.flow_syntax(file, call)?;
            let callee = required(syntax.expression, "call has no expression")?;
            if callee.kind == SyntaxKind::PropertyAccessExpression {
                let receiver = required(
                    self.source.flow_syntax(file, callee)?.expression,
                    "call access has no receiver",
                )?;
                return sequence.read(self.is_or_contains_matching_reference_completion(
                    file, reference, receiver, consumer,
                ));
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    fn is_or_contains_matching_reference_completion(
        &mut self,
        file: FileId,
        mut source: ReferenceNode,
        target: Site,
        consumer: &Location,
    ) -> NativeResult<bool> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            if sequence.read(self.matching_reference_completion(
                source,
                (file, target),
                consumer,
            ))? {
                return Ok(true);
            }
            while matches!(
                source.kind,
                SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
            ) {
                source = required(
                    self.reference_syntax(source)?.expression,
                    "matching access has no receiver",
                )?;
                if sequence.read(self.matching_reference_completion(
                    source,
                    (file, target),
                    consumer,
                ))? {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        sequence.finish(result)
    }

    /// `narrowTypeByCallExpression`: matching arguments precede effects and
    /// predicates; the separate hasOwnProperty branch consumes only its native
    /// access/name gates and preserves the resulting missing-type facts.
    fn narrow_call_condition_completion(
        &mut self,
        state: &ReadState<'_>,
        ty: TypeId,
        root: ExpressionRoot,
        truthy: bool,
    ) -> NativeResult<TypeId> {
        let mut sequence = super::super::model::NativeSequence::new();
        let result = (|| {
            let matching = sequence.read(self.has_matching_argument_completion(
                root.file,
                root.site,
                state.reference,
                state.location,
            ))?;
            let syntax = self.source.flow_syntax(root.file, root.site)?;
            if matching
                && (truthy || !syntax.optional_chain)
                && let Some(signature) =
                    sequence.read(self.effects_signature_completion(root.file, root.site))?
                && sequence
                    .read(self.type_predicate_of_signature_completion(&signature, state.location))?
                    .is_some_and(|predicate| predicate.kind == TypePredicateKind::Is)
            {
                return Err(self.flow_required(
                    state.location,
                    "native call type-predicate relation admission is pending",
                ));
            }
            if self.model.contains_missing_type(ty)
                && matches!(
                    state.reference.kind,
                    SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
                )
            {
                let callee = required(syntax.expression, "call condition has no expression")?;
                if callee.kind == SyntaxKind::PropertyAccessExpression {
                    let access = self.source.flow_syntax(root.file, callee)?;
                    let receiver = required(
                        self.reference_syntax(state.reference)?.expression,
                        "condition reference access has no receiver",
                    )?;
                    let candidate = self.reference_candidate(ExpressionRoot {
                        file: root.file,
                        site: required(access.expression, "condition call access has no receiver")?,
                    })?;
                    if sequence.read(self.matching_reference_completion(
                        receiver,
                        candidate,
                        state.location,
                    ))? {
                        let name = required(access.name, "condition call access has no name")?;
                        if name.kind == SyntaxKind::Identifier
                            && self.source.access_name_text(ExpressionRoot {
                                file: root.file,
                                site: name,
                            })? == "hasOwnProperty"
                        {
                            let arguments = self.source.call_arguments(root.file, root.site)?;
                            if arguments.len() == 1
                                && let Some(name) =
                                    sequence.read(self.reference_property_name_completion(
                                        state.reference,
                                        state.location,
                                    ))?
                                && matches!(
                                    arguments[0].kind,
                                    SyntaxKind::StringLiteral
                                        | SyntaxKind::NoSubstitutionTemplateLiteral
                                )
                                && name
                                    == self.source.string_literal_key(root.file, arguments[0])?
                            {
                                return sequence.read(self.type_with_facts_completion(
                                    ty,
                                    if truthy {
                                        TypeFacts::NOT_UNDEFINED
                                    } else {
                                        TypeFacts::ONLY_UNDEFINED
                                    },
                                    state.location,
                                ));
                            }
                        }
                    }
                }
            }
            Ok(ty)
        })();
        sequence.finish(result)
    }

    pub(super) fn flow_required(&self, location: &Location, message: &str) -> DemandError {
        super::super::unsupported_demand_at_location(
            self.inputs,
            location,
            SemanticFailureReason::FlowRequired,
            message,
        )
    }

    fn read_binding_failure(&self, location: &Location, error: TypeBindingFailure) -> DemandError {
        match error {
            TypeBindingFailure::ErrorType(_) | TypeBindingFailure::Dependency { .. } => {
                super::super::binding_failure_at(self.inputs, location, error)
            }
            TypeBindingFailure::Unresolved(message) => super::super::invalid_at_location(
                self.inputs,
                location,
                SemanticFailureReason::UnresolvedSymbol,
                message,
            )
            .into(),
            TypeBindingFailure::NotRetained => {
                super::super::internal("selected read has no final retained binding attachment")
                    .into()
            }
            TypeBindingFailure::Pending(message) => super::super::unsupported_demand_at_location(
                self.inputs,
                location,
                SemanticFailureReason::UnsupportedSemanticForm,
                message,
            ),
        }
    }
}

/// `isMutableLocalVariableDeclaration`: a non-exported/non-global `let` is
/// mutable local. Its syntax facts come from the ordinary read operation.
fn is_mutable_local_variable_declaration(facts: &VariableReadFacts) -> bool {
    facts.parent_flags & crate::compiler::node::LET != 0
        && facts.combined_modifiers & crate::syntax::modifiers::EXPORT == 0
        && !facts.at_global_source
}

/// `isParameterOrMutableLocalVariable`: native root declaration and catch
/// context are distinct from the narrower non-exported/non-global let rule.
fn is_parameter_or_mutable_local_variable(facts: &VariableReadFacts) -> bool {
    facts.root_kind == SyntaxKind::Parameter
        || facts.root_kind == SyntaxKind::VariableDeclaration
            && (facts.parent_kind == SyntaxKind::CatchClause
                || is_mutable_local_variable_declaration(facts))
}

fn required<T>(site: Option<T>, message: &'static str) -> Result<T, DemandError> {
    site.ok_or_else(|| super::super::internal(message).into())
}

/// `getBranchLabelAntecedents`: the nearest active reduction substitutes its
/// exact ordered subset. Labels remain immutable and looping labels use their
/// separate native entry operation, not this branch question.
fn branch_antecedents<'g>(
    graph: &'g Graph,
    label: FlowId,
    reductions: &[FlowId],
) -> Result<&'g [FlowId], DemandError> {
    for &reduction in reductions.iter().rev() {
        let Node::ReduceLabel {
            target,
            antecedents,
            ..
        } = graph.node(reduction)
        else {
            return Err(super::super::internal(
                "native reduction history has a non-reduction node",
            )
            .into());
        };
        if *target == label {
            return Ok(antecedents);
        }
    }
    let Node::Label {
        looping: false,
        antecedents,
    } = graph.node(label)
    else {
        return Err(super::super::internal("native branch question has no branch label").into());
    };
    Ok(antecedents)
}

/// `getTypeAtFlowBranchLabel`: defer only the first empty switch path, retaining
/// exact indices and the source order of every other antecedent.
fn next_branch_path(
    graph: &Graph,
    paths: &[FlowId],
    start: usize,
    bypass: &mut Option<FlowId>,
) -> Option<(usize, FlowId)> {
    for (index, &flow) in paths.iter().enumerate().skip(start) {
        if bypass.is_none()
            && matches!(graph.node(flow), Node::SwitchClause { start, end, .. } if start == end)
        {
            *bypass = Some(flow);
        } else {
            return Some((index, flow));
        }
    }
    None
}
