//! Native checker-created references and flow nodes have fresh request-local
//! identities. Their syntax and graph edges feed the ordinary flow interpreter.
use super::super::super::model::{DemandError, NativeCompletion, NativeResult, TypeId};
use super::super::super::source::{ExpressionRoot, FlowSyntax};
use super::super::Evaluator;
use super::{FlowType, ReadInvocations, ReadStack, ReadState, ReadWork};
use crate::project::binding::flow::{ContainerId, FlowId, Site};
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic::evaluation) struct CreatedReferenceId(usize);
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ReferenceOrigin {
    Captured(Site),
    Created(CreatedReferenceId),
}
/// A syntax identity includes its source file. Flow graph identity is carried
/// separately: alias-condition inlining can cross files without changing the
/// original reference or the graph being interpreted.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic::evaluation) struct ReferenceNode {
    pub(in crate::project::semantic::evaluation) file: FileId,
    pub(in crate::project::semantic::evaluation) kind: SyntaxKind,
    origin: ReferenceOrigin,
}
impl From<ExpressionRoot> for ReferenceNode {
    fn from(root: ExpressionRoot) -> Self {
        Self {
            file: root.file,
            kind: root.site.kind,
            origin: ReferenceOrigin::Captured(root.site),
        }
    }
}
impl From<(FileId, Site)> for ReferenceNode {
    fn from((file, site): (FileId, Site)) -> Self {
        ExpressionRoot { file, site }.into()
    }
}
impl ReferenceNode {
    pub(in crate::project::semantic::evaluation) fn captured(self) -> Result<Site, DemandError> {
        match self.origin {
            ReferenceOrigin::Captured(site) => Ok(site),
            ReferenceOrigin::Created(_) => Err(super::super::super::internal(
                "created reference has no captured source site",
            )
            .into()),
        }
    }
    pub(in crate::project::semantic::evaluation) fn captured_site(self) -> Option<Site> {
        match self.origin {
            ReferenceOrigin::Captured(site) => Some(site),
            ReferenceOrigin::Created(_) => None,
        }
    }
    pub(in crate::project::semantic::evaluation) fn created_id(self) -> Option<CreatedReferenceId> {
        match self.origin {
            ReferenceOrigin::Created(id) => Some(id),
            _ => None,
        }
    }
}
struct CreatedReference {
    parent: ReferenceNode,
    expression: Option<ReferenceNode>,
    name: Option<ExpressionRoot>,
    argument: Option<ReferenceNode>,
    property_name: Option<String>,
    flow: Option<FlowId>,
}
#[derive(Clone, Copy)]
pub(in crate::project::semantic::evaluation) struct CreatedFlowId(usize);
#[derive(Clone, Copy)]
pub(in crate::project::semantic::evaluation) enum FlowNodeRef {
    Bound(FlowId),
    Created(CreatedFlowId),
}
impl From<FlowId> for FlowNodeRef {
    fn from(flow: FlowId) -> Self {
        Self::Bound(flow)
    }
}
#[derive(Clone, Copy)]
pub(super) enum CreatedFlow {
    Start,
    Condition {
        expression: Site,
        truthy: bool,
        antecedent: FlowNodeRef,
    },
}
#[derive(Default)]
pub(super) struct ReferenceState {
    nodes: Vec<CreatedReference>,
    flows: Vec<CreatedFlow>,
}

pub(in crate::project::semantic::evaluation) struct FlowReferenceQuestion {
    pub(in crate::project::semantic::evaluation) file: FileId,
    pub(in crate::project::semantic::evaluation) reference: ReferenceNode,
    pub(in crate::project::semantic::evaluation) declared: TypeId,
    pub(in crate::project::semantic::evaluation) initial: TypeId,
    pub(in crate::project::semantic::evaluation) admitted: TypeId,
    pub(in crate::project::semantic::evaluation) container: Option<ContainerId>,
    /// None asks the reference's own FlowNode attachment, as native does.
    pub(in crate::project::semantic::evaluation) flow: Option<FlowNodeRef>,
}
impl ReadWork {
    pub(super) fn visit_reference(frame: usize, flow: FlowNodeRef) -> Self {
        match flow {
            FlowNodeRef::Bound(flow) => Self::Visit { frame, flow },
            FlowNodeRef::Created(flow) => Self::VisitCreated { frame, flow },
        }
    }
}
impl Evaluator<'_, '_> {
    /// `getSyntheticElementAccess` creates distinct literal/access nodes and
    /// an actual parenthesized receiver when needed. The outer node copies the
    /// parent's FlowNode but retains the destructuring declaration as parent.
    pub(in crate::project::semantic::evaluation) fn new_destructuring_reference(
        &mut self,
        expression: ReferenceNode,
        name: String,
        parent: ExpressionRoot,
        flow: FlowId,
        parenthesized: bool,
    ) -> ReferenceNode {
        let start = self.reads.references.nodes.len();
        let literal = ReferenceNode {
            file: parent.file,
            kind: SyntaxKind::StringLiteral,
            origin: ReferenceOrigin::Created(CreatedReferenceId(start)),
        };
        let access = ReferenceNode {
            file: parent.file,
            kind: SyntaxKind::ElementAccessExpression,
            origin: ReferenceOrigin::Created(CreatedReferenceId(start + 1)),
        };
        let receiver = if parenthesized {
            ReferenceNode {
                file: expression.file,
                kind: SyntaxKind::ParenthesizedExpression,
                origin: ReferenceOrigin::Created(CreatedReferenceId(start + 2)),
            }
        } else {
            expression
        };
        self.reads.references.nodes.push(CreatedReference {
            parent: access,
            expression: None,
            name: None,
            argument: None,
            property_name: Some(name.clone()),
            flow: None,
        });
        self.reads.references.nodes.push(CreatedReference {
            parent: parent.into(),
            expression: Some(receiver),
            name: None,
            argument: Some(literal),
            property_name: Some(name),
            flow: Some(flow),
        });
        if parenthesized {
            self.reads.references.nodes.push(CreatedReference {
                parent: access,
                expression: Some(expression),
                name: None,
                argument: None,
                property_name: None,
                flow: None,
            });
        }
        access
    }

    /// `getFlowNodeOfNode` reads the actual captured or created attachment.
    pub(in crate::project::semantic::evaluation) fn reference_flow_node(
        &self,
        reference: ReferenceNode,
        consumer: &Location,
    ) -> Result<Option<FlowId>, DemandError> {
        match reference.origin {
            ReferenceOrigin::Captured(site) => self
                .inputs
                .bindings
                .flow_node_of_node(reference.file, site)
                .map_err(|failure| self.read_binding_failure(consumer, failure)),
            ReferenceOrigin::Created(id) => Ok(self.reads.references.nodes[id.0].flow),
        }
    }
    /// `getTypeAtFlowNode`'s Start branch is shared by retained and fresh nodes.
    pub(super) fn schedule_flow_start(
        &mut self,
        state: &ReadState<'_>,
        graph: &crate::project::binding::flow::Graph,
        frame: usize,
        container: Option<ContainerId>,
        invocations: &mut ReadInvocations,
        work: &mut ReadStack,
    ) -> Result<(), DemandError> {
        if let Some(container) = container
            && Some(container) != state.flow_container
            && !matches!(
                state.reference.kind,
                SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
            )
            && !(state.reference.kind == SyntaxKind::ThisKeyword
                && graph.container(container).kind != SyntaxKind::ArrowFunction)
        {
            let outer = graph.container(container).outer.ok_or_else(|| {
                super::super::super::internal("captured Start has no outer attachment")
            })?;
            let flow = outer
                .flow
                .ok_or_else(|| super::super::super::internal("captured Start has no outer flow"))?;
            work.push(ReadWork::Visit { frame, flow });
        } else {
            invocations.finish(frame, FlowType::complete(state.initial))?;
        }
        Ok(())
    }
    /// `getTypeAtFlowCondition` recursively acquires the antecedent before
    /// the common Condition continuation performs narrowing and Never handling.
    pub(super) fn schedule_flow_condition(
        &mut self,
        frame: usize,
        expression: Site,
        truthy: bool,
        antecedent: FlowNodeRef,
        invocations: &mut ReadInvocations,
        work: &mut ReadStack,
    ) {
        let child = invocations.child(frame);
        work.push(ReadWork::Condition {
            frame,
            antecedent: child,
            expression,
            truthy,
        });
        work.push(ReadWork::visit_reference(child, antecedent));
    }
    /// `NewPropertyAccessExpression(NewKeywordExpression(ThisKeyword), name)`
    /// retains the original name node, two fresh nodes and their actual parents.
    pub(super) fn new_this_property_reference(
        &mut self,
        name: ExpressionRoot,
        parent: ExpressionRoot,
        flow: Option<FlowId>,
    ) -> ReferenceNode {
        self.new_this_access_reference(Some(name), None, parent, flow)
    }

    /// `isPropertyInitializedInConstructor` uses an ElementAccessExpression
    /// for computed names, retaining the original expression node.
    pub(in crate::project::semantic::evaluation) fn new_this_initialization_reference(
        &mut self,
        name: ExpressionRoot,
        argument: Option<ExpressionRoot>,
        parent: ExpressionRoot,
        flow: Option<FlowId>,
    ) -> ReferenceNode {
        self.new_this_access_reference(
            if argument.is_some() { None } else { Some(name) },
            argument,
            parent,
            flow,
        )
    }

    fn new_this_access_reference(
        &mut self,
        name: Option<ExpressionRoot>,
        argument: Option<ExpressionRoot>,
        parent: ExpressionRoot,
        flow: Option<FlowId>,
    ) -> ReferenceNode {
        let this = ReferenceNode {
            file: parent.file,
            kind: SyntaxKind::ThisKeyword,
            origin: ReferenceOrigin::Created(CreatedReferenceId(self.reads.references.nodes.len())),
        };
        let access = ReferenceNode {
            file: parent.file,
            kind: if argument.is_some() {
                SyntaxKind::ElementAccessExpression
            } else {
                SyntaxKind::PropertyAccessExpression
            },
            origin: ReferenceOrigin::Created(CreatedReferenceId(
                self.reads.references.nodes.len() + 1,
            )),
        };
        self.reads.references.nodes.push(CreatedReference {
            parent: access,
            expression: None,
            name: None,
            argument: None,
            property_name: None,
            flow: None,
        });
        self.reads.references.nodes.push(CreatedReference {
            parent: parent.into(),
            expression: Some(this),
            name,
            argument: argument.map(Into::into),
            property_name: None,
            flow,
        });
        access
    }
    pub(in crate::project::semantic::evaluation) fn new_flow_start(&mut self) -> FlowNodeRef {
        let id = CreatedFlowId(self.reads.references.flows.len());
        self.reads.references.flows.push(CreatedFlow::Start);
        FlowNodeRef::Created(id)
    }
    /// Native raw FlowNode allocation does not mark its antecedent Shared.
    pub(in crate::project::semantic::evaluation) fn new_flow_condition(
        &mut self,
        expression: Site,
        truthy: bool,
        antecedent: FlowNodeRef,
    ) -> FlowNodeRef {
        let id = CreatedFlowId(self.reads.references.flows.len());
        self.reads.references.flows.push(CreatedFlow::Condition {
            expression,
            truthy,
            antecedent,
        });
        FlowNodeRef::Created(id)
    }
    pub(super) fn created_flow(&self, id: CreatedFlowId) -> CreatedFlow {
        self.reads.references.flows[id.0]
    }

    /// `getFlowTypeOfReferenceEx`: explicit flow overrides attachment lookup;
    /// absent flow returns declared before any interpretation/admission work.
    pub(in crate::project::semantic::evaluation) fn flow_type_of_reference(
        &mut self,
        question: FlowReferenceQuestion,
        consumer: &Location,
    ) -> Result<TypeId, DemandError> {
        self.flow_type_of_reference_completion(question, consumer)?
            .admit()
    }

    pub(in crate::project::semantic::evaluation) fn flow_type_of_reference_completion(
        &mut self,
        question: FlowReferenceQuestion,
        consumer: &Location,
    ) -> NativeResult<TypeId> {
        self.flow_type_of_reference_with_admission_completion(question, consumer, false)
    }

    /// Existing source-value questions can establish a native literal-base
    /// admission proof. Internal initialization/predicate questions require
    /// their exact supplied initial identity and cannot borrow that proof.
    pub(super) fn flow_type_of_reference_with_admission_completion(
        &mut self,
        question: FlowReferenceQuestion,
        consumer: &Location,
        literal_base_admission: bool,
    ) -> NativeResult<TypeId> {
        if self.reads.analysis_disabled {
            return NativeCompletion::recovered(
                self.model.native_error_type(),
                self.native_flow_depth_error(consumer),
            );
        }
        let flow = match question.flow {
            Some(flow) => Some(flow),
            None => self
                .reference_flow_node(question.reference, consumer)?
                .map(Into::into),
        };
        let Some(flow) = flow else {
            return Ok(NativeCompletion::clean(question.declared));
        };
        let state = ReadState {
            file: question.file,
            flow_container: question.container,
            reference: question.reference,
            declared: question.declared,
            initial: question.initial,
            admitted: question.admitted,
            regular_expression: false,
            literal_base_admission,
            location: consumer,
        };
        self.read_flow_completion(&state, flow)
    }

    // Both created factories expose their outer access as the acquired root.
    // Their private receiver/literal children serve matching and cache keys;
    // narrowing and contextual questions retain the outer captured parent.
    pub(in crate::project::semantic::evaluation) fn reference_position(
        &self,
        reference: ReferenceNode,
    ) -> Result<super::super::super::source::ReferencePosition, DemandError> {
        use super::super::super::source::ReferencePosition;
        Ok(match reference.origin {
            ReferenceOrigin::Captured(site) => ReferencePosition::Captured(site),
            ReferenceOrigin::Created(id) => ReferencePosition::Created {
                kind: reference.kind,
                parent: self.reads.references.nodes[id.0].parent.captured()?,
            },
        })
    }

    pub(super) fn reference_syntax(
        &mut self,
        reference: ReferenceNode,
    ) -> Result<FlowSyntax<ReferenceNode>, DemandError> {
        if let ReferenceOrigin::Captured(site) = reference.origin {
            let s = self.source.flow_syntax(reference.file, site)?;
            let child = |site| (reference.file, site).into();
            return Ok(FlowSyntax {
                kind: s.kind,
                expression: s.expression.map(child),
                argument: s.argument.map(child),
                operand: s.operand.map(child),
                left: s.left.map(child),
                right: s.right.map(child),
                name: s.name.map(child),
                reference_name: s.reference_name,
                keyword_token: s.keyword_token,
                operator: s.operator,
                unary_operator: s.unary_operator,
                parent: s.parent.map(child),
                assignment_target: s.assignment_target.map(child),
                simple_assignment: s.simple_assignment,
                assignment_expression: s.assignment_expression,
                in_type_query: s.in_type_query,
                this_identifier: s.this_identifier,
                optional_chain: s.optional_chain,
                optionality_condition: s.optionality_condition,
            });
        }
        let id = reference.created_id().expect("created reference identity");
        let node = &self.reads.references.nodes[id.0];
        Ok(FlowSyntax {
            kind: reference.kind,
            expression: node.expression,
            argument: node.argument,
            operand: None,
            left: None,
            right: None,
            name: node.name.map(Into::into),
            reference_name: node.property_name.clone(),
            keyword_token: None,
            operator: None,
            unary_operator: None,
            parent: Some(node.parent),
            assignment_target: None,
            simple_assignment: false,
            assignment_expression: false,
            in_type_query: false,
            this_identifier: false,
            optional_chain: false,
            optionality_condition: false,
        })
    }
    pub(super) fn reference_property_name_completion(
        &mut self,
        reference: ReferenceNode,
        consumer: &Location,
    ) -> NativeResult<Option<crate::project::binding::NativePropertyKey>> {
        match reference.origin {
            ReferenceOrigin::Captured(site) => {
                self.accessed_property_name_completion(reference.file, site, consumer)
            }
            ReferenceOrigin::Created(id)
                if self.reads.references.nodes[id.0].property_name.is_some() =>
            {
                Ok(NativeCompletion::clean(
                    self.reads.references.nodes[id.0]
                        .property_name
                        .as_ref()
                        .map(|name| {
                            crate::project::binding::NativePropertyKey::fixed(
                                name.as_bytes(),
                                self.control.as_ref(),
                            )
                        }),
                ))
            }
            ReferenceOrigin::Created(id) => match self.reads.references.nodes[id.0].name {
                Some(name) => self
                    .source
                    .access_name_text(name)
                    .map(|name| {
                        NativeCompletion::clean(Some(
                            crate::project::binding::NativePropertyKey::fixed(
                                name.as_bytes(),
                                self.control.as_ref(),
                            ),
                        ))
                    })
                    .map_err(Into::into),
                None => Ok(NativeCompletion::clean(None)),
            },
        }
    }
}
