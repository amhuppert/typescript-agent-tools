//! Native binder flow over the final tree. NodeIds are temporary construction
//! handles; published graph nodes retain exact sites and file-local identities.

mod switch_flow;
mod syntax;
mod try_flow;

use super::site_map::{PackedValue, SiteMap};
use crate::analysis::{InputError, InputErrorKind};
use crate::syntax as fields;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Span};
use std::collections::HashMap;
use std::num::NonZeroUsize;

/// A final-tree site keeps its native role as well as its exact token range.
/// Several different syntax nodes can have the same range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct Site {
    pub(in crate::project) span: Span,
    pub(in crate::project) kind: SyntaxKind,
}
// Exact final-tree kind plus two scalar offsets, with no syntax traversal.

impl Site {
    pub(in crate::project) fn new(ast: &Ast<'_>, id: NodeId) -> Self {
        Self {
            span: ast.token_span(id),
            kind: ast.node(id).syntax_kind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub(in crate::project) struct FlowId(NonZeroUsize);

impl FlowId {
    fn new(index: usize) -> Self {
        Self(encode_index(index))
    }

    fn index(self) -> usize {
        self.0.get() - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub(in crate::project) struct ContainerId(NonZeroUsize);

impl ContainerId {
    fn new(index: usize) -> Self {
        Self(encode_index(index))
    }

    fn index(self) -> usize {
        self.0.get() - 1
    }
}

// Both identities index nonzero-sized Vec elements, so usize::MAX can never
// name an allocated slot. Reserving zero preserves their optional niches,
// including Option<Read> through its always-present container identity.
fn encode_index(index: usize) -> NonZeroUsize {
    index
        .checked_add(1)
        .and_then(NonZeroUsize::new)
        .expect("native flow arena index fits a nonzero identity")
}

/// Refuse a file whose flow graph has more than `u32::MAX` nodes or
/// containers, as `Unsupported`: a limit of tsparse, not a TypeScript rule.
/// It is the one owner of that limit; a frozen read packs both identities as
/// `u32` on the strength of it (`Read::pack`). The source-length limit does
/// not imply it: `a&&a;` allocates six flow nodes in five bytes.
fn check_arenas(nodes: usize, containers: usize) -> Result<(), InputError> {
    if nodes <= u32::MAX as usize && containers <= u32::MAX as usize {
        return Ok(());
    }
    Err(InputError {
        kind: InputErrorKind::Unsupported,
        message: format!(
            "control flow has more than {} nodes or containers; tsparse supports up to u32::MAX of each per file",
            u32::MAX
        ),
        path: None,
        position: None,
    })
}

#[derive(Debug, Clone, Copy)]
pub(in crate::project) struct Read {
    pub(in crate::project) flow: Option<FlowId>,
    pub(in crate::project) container: ContainerId,
}

// A zero encoded flow means an explicitly absent native attachment. Flow 0
// itself is reachable as an identity (the Unreachable node), so present ids
// are shifted by one. Both fit, since capture refuses a graph with more than
// `u32::MAX` nodes or containers (`check_arenas`, the owner of that limit).
impl PackedValue for Read {
    type Packed = [u32; 2];

    fn pack(self) -> Self::Packed {
        debug_assert!(
            self.flow
                .is_none_or(|flow| flow.index() < u32::MAX as usize)
                && self.container.index() <= u32::MAX as usize,
            "a file's flow identities fit u32: {self:?}"
        );
        [
            self.flow.map_or(0, |flow| flow.index() as u32 + 1),
            self.container.index() as u32,
        ]
    }

    fn unpack([flow, container]: Self::Packed) -> Self {
        Self {
            flow: flow.checked_sub(1).map(|flow| FlowId::new(flow as usize)),
            container: ContainerId::new(container as usize),
        }
    }
}

#[derive(Debug)]
pub(in crate::project) enum Node {
    Unreachable,
    Start {
        container: Option<ContainerId>,
    },
    Assignment {
        target: Site,
        antecedent: FlowId,
    },
    Condition {
        expression: Site,
        truthy: bool,
        antecedent: FlowId,
    },
    Label {
        looping: bool,
        antecedents: Vec<FlowId>,
    },
    Call {
        expression: Site,
        antecedent: FlowId,
    },
    ArrayMutation {
        expression: Site,
        antecedent: FlowId,
    },
    ReduceLabel {
        target: FlowId,
        antecedents: Vec<FlowId>,
        antecedent: FlowId,
    },
    /// A deferred native construction retains its actual cause site. It has
    /// no successful graph interpretation or fabricated predecessor behavior.
    Pending {
        site: Site,
        message: &'static str,
    },
    SwitchClause {
        statement: Site,
        start: usize,
        end: usize,
        antecedent: FlowId,
    },
}

#[derive(Debug)]
struct Entry {
    node: Node,
    referenced: bool,
    shared: bool,
}

#[derive(Debug)]
pub(in crate::project) struct Container {
    pub(in crate::project) site: Span,
    pub(in crate::project) kind: SyntaxKind,
    parent_kind: Option<SyntaxKind>,
    pub(in crate::project) outer: Option<Read>,
    pub(in crate::project) end: Option<FlowId>,
    /// Native ReturnFlowNode after finishFlowLabel merges explicit returns.
    pub(in crate::project) return_flow: Option<FlowId>,
    pub(in crate::project) explicit_return: bool,
    pub(in crate::project) contains_this: bool,
}

#[derive(Debug, Default)]
pub(in crate::project) struct Graph {
    nodes: Vec<Entry>,
    containers: Vec<Container>,
    reads: SiteMap<Read>,
}

pub(super) struct Capture {
    pub(super) graph: Graph,
    contains_this: HashMap<(usize, usize), bool>,
}

impl Capture {
    pub(super) fn read(&self, span: Span) -> Option<Read> {
        self.graph.read(Site {
            span,
            kind: SyntaxKind::Identifier,
        })
    }

    pub(super) fn interface_contains_this(&self, name: Span) -> Option<bool> {
        self.contains_this.get(&(name.start, name.end)).copied()
    }
}

impl Graph {
    /// `getFlowNodeOfNode` observes the attachment retained by bindWorker.
    /// A recorded absent flow is distinct from a site which was not captured.
    pub(in crate::project) fn read(&self, site: Site) -> Option<Read> {
        self.reads.get((site.kind, site.span))
    }
    pub(in crate::project) fn node(&self, id: FlowId) -> &Node {
        &self.nodes[id.index()].node
    }
    pub(in crate::project) fn shared(&self, id: FlowId) -> bool {
        self.nodes[id.index()].shared
    }
    pub(in crate::project) fn container(&self, id: ContainerId) -> &Container {
        &self.containers[id.index()]
    }

    /// `checkIdentifier`'s capture-container eligibility. Native syntax
    /// predicates own the class-expression versus class-declaration decision;
    /// the actual saved attachment supplies the eligible outer container.
    pub(in crate::project) fn capture_extension(&self, id: ContainerId) -> Option<Read> {
        let container = self.container(id);
        (crate::syntax::is_function_expression_or_arrow_function(container.kind)
            || crate::syntax::is_object_literal_or_class_expression_method_or_accessor(
                container.kind,
                container.parent_kind,
            ))
        .then_some(container.outer)
        .flatten()
    }

    /// Acquire the native control-flow container at its actual final syntax
    /// role. A declaration name's outer read attachment is a different
    /// question. Missing or ambiguous replay is an invariant failure, while a
    /// found container may legitimately have no reachable EndFlowNode.
    pub(in crate::project) fn container_at(&self, site: Site) -> Result<&Container, &'static str> {
        self.container_id_at(site).map(|id| self.container(id))
    }

    pub(in crate::project) fn container_id_at(
        &self,
        site: Site,
    ) -> Result<ContainerId, &'static str> {
        let mut matches =
            self.containers.iter().enumerate().filter(|(_, container)| {
                container.site == site.span && container.kind == site.kind
            });
        let (index, _) = matches
            .next()
            .ok_or("native body has no retained control-flow container")?;
        if matches.next().is_some() {
            return Err("native body control-flow container attachment is ambiguous");
        }
        Ok(ContainerId::new(index))
    }

    /// `bindSourceFile` / `bindContainer` / `bindChildren`: construct native
    /// ordered flow with an explicit scheduling stack, before collection's AST
    /// arena is released. The retained site inventory is the flow-attachment
    /// owner: every visited node records its optional native field, so an
    /// explicit nil attachment differs from a node that was not retained.
    pub(super) fn capture(ast: &Ast<'_>) -> Result<Capture, InputError> {
        Builder::new(ast).run()
    }
}

#[derive(Clone, Copy)]
struct Context {
    current: FlowId,
    container: ContainerId,
    break_target: Option<FlowId>,
    continue_target: Option<FlowId>,
    return_target: Option<FlowId>,
    exception_target: Option<FlowId>,
    explicit_return: bool,
}

struct ActiveLabel {
    node: NodeId,
    name: NodeId,
    break_target: FlowId,
    continue_target: Option<FlowId>,
}

enum Work {
    Bind(NodeId),
    Try(try_flow::Work),
    Switch(switch_flow::Work),
    EndSeenThis {
        saved: bool,
        id: NodeId,
        propagates: bool,
    },
    EndContainer {
        saved: Context,
        labels: Vec<ActiveLabel>,
        id: NodeId,
        immediate: bool,
    },
    EndLabel(FlowId),
    EndExpression {
        saved_flow: FlowId,
        saved_effects: bool,
        after: FlowId,
    },
    Initializer(NodeId),
    EndInitializer(FlowId),
    InitializedVariable(NodeId),
    Assignment(NodeId),
    AfterBinary(NodeId),
    AfterCall(NodeId),
    MaybeCall(NodeId),
    OptionalRest {
        node: NodeId,
        yes: FlowId,
        no: FlowId,
    },
    Condition {
        expression: Option<NodeId>,
        yes: FlowId,
        no: FlowId,
        after_bind: bool,
    },
    SetBranches(Option<(FlowId, FlowId)>),
    SetAssignmentPattern(bool),
    SetLabel(FlowId),
    SetCurrent(FlowId),
    AddAntecedent(FlowId),
    SetTargets {
        break_target: Option<FlowId>,
        continue_target: Option<FlowId>,
    },
    Return,
    Throw,
    Jump {
        label: Option<FlowId>,
    },
}

struct Builder<'a, 's> {
    ast: &'a Ast<'s>,
    facts: syntax::Facts,
    graph: Graph,
    reads: Vec<((SyntaxKind, Span), Read)>,
    contains_this: HashMap<(usize, usize), bool>,
    seen_this: bool,
    has_effects: bool,
    branches: Option<(FlowId, FlowId)>,
    in_assignment_pattern: bool,
    context: Context,
    labels: Vec<ActiveLabel>,
    work: Vec<Work>,
}

impl<'a, 's> Builder<'a, 's> {
    fn new(ast: &'a Ast<'s>) -> Self {
        let mut graph = Graph::default();
        graph.nodes.push(Entry {
            node: Node::Unreachable,
            referenced: false,
            shared: false,
        });
        graph.nodes.push(Entry {
            node: Node::Start { container: None },
            referenced: false,
            shared: false,
        });
        graph.containers.push(Container {
            site: ast.token_span(ast.root_id()),
            kind: SyntaxKind::SourceFile,
            parent_kind: None,
            outer: None,
            end: None,
            return_flow: None,
            explicit_return: false,
            contains_this: false,
        });
        Self {
            ast,
            facts: syntax::Facts::new(ast),
            graph,
            reads: Vec::new(),
            contains_this: HashMap::new(),
            seen_this: false,
            has_effects: false,
            branches: None,
            in_assignment_pattern: false,
            context: Context {
                current: FlowId::new(1),
                container: ContainerId::new(0),
                break_target: None,
                continue_target: None,
                return_target: None,
                exception_target: None,
                explicit_return: false,
            },
            work: vec![Work::Bind(ast.root_id())],
            labels: Vec::new(),
        }
    }

    fn run(mut self) -> Result<Capture, InputError> {
        while let Some(work) = self.work.pop() {
            match work {
                Work::Bind(id) => self.bind(id),
                Work::Try(work) => self.try_step(work),
                Work::Switch(work) => self.switch_step(work),
                Work::EndSeenThis {
                    saved,
                    id,
                    propagates,
                } => {
                    if self.ast.node(id).syntax_kind == SyntaxKind::InterfaceDeclaration {
                        if let Some(name) = fields::id(self.ast.node(id), "name") {
                            let span = self.ast.token_span(name);
                            self.contains_this
                                .insert((span.start, span.end), self.seen_this);
                        }
                    }
                    self.seen_this = saved || (propagates && self.seen_this);
                }
                Work::EndContainer {
                    saved,
                    labels,
                    id,
                    immediate,
                } => {
                    let reachable = !self.unreachable(self.context.current);
                    let has_body = fields::id(self.ast.node(id), "body").is_some();
                    let container = &mut self.graph.containers[self.context.container.index()];
                    container.end = (reachable && has_body).then_some(self.context.current);
                    container.explicit_return = self.context.explicit_return;
                    container.contains_this = self.seen_this;
                    if let Some(label) = self.context.return_target {
                        self.add(label, self.context.current);
                        self.context.current = self.finish(label);
                    }
                    if matches!(
                        self.ast.node(id).syntax_kind,
                        SyntaxKind::Constructor | SyntaxKind::ClassStaticBlockDeclaration
                    ) {
                        self.graph.containers[self.context.container.index()].return_flow =
                            Some(self.context.current);
                    }
                    let current = self.context.current;
                    self.context = saved;
                    self.labels = labels;
                    if immediate {
                        self.context.current = current;
                    }
                }
                Work::EndLabel(after) => {
                    self.labels
                        .pop()
                        .expect("active native label has a completion");
                    self.add(after, self.context.current);
                    self.context.current = self.finish(after);
                }
                Work::EndExpression {
                    saved_flow,
                    saved_effects,
                    after,
                } => {
                    self.context.current = if self.has_effects {
                        self.finish(after)
                    } else {
                        saved_flow
                    };
                    self.has_effects |= saved_effects;
                }
                Work::Initializer(id) => {
                    self.work.push(Work::EndInitializer(self.context.current));
                    self.work.push(Work::Bind(id));
                }
                Work::EndInitializer(entry) => {
                    // `bindInitializer`: a default can remain unevaluated;
                    // only a real changed flow adds a join with the entry.
                    if !self.unreachable(entry) && entry != self.context.current {
                        let exit = self.label(false);
                        self.add(exit, entry);
                        self.add(exit, self.context.current);
                        self.context.current = self.finish(exit);
                    }
                }
                Work::InitializedVariable(id) => self.initialized_variable(id),
                Work::Assignment(id) => self.assignment(id),
                Work::AfterBinary(id) => self.after_binary(id),
                Work::AfterCall(id) => self.after_call(id),
                Work::MaybeCall(id) => self.maybe_call(id),
                Work::OptionalRest { node, yes, no } => self.optional_rest(node, yes, no),
                Work::Condition {
                    expression,
                    yes,
                    no,
                    after_bind,
                } => {
                    // `bindCondition` adds final conditions only when the
                    // expression did not already route its own logical or
                    // outermost optional-chain branches.
                    if after_bind
                        && expression.is_some_and(|id| {
                            self.facts.logical_assignment(self.ast, id)
                                || self.facts.logical_expression(self.ast, id)
                                || fields::is_optional_chain(self.ast, id)
                                    && self.facts.outermost_optional(self.ast, id)
                        })
                    {
                        continue;
                    }
                    let yes_flow = self.condition(expression, true);
                    self.add(yes, yes_flow);
                    let no_flow = self.condition(expression, false);
                    self.add(no, no_flow);
                }
                Work::SetBranches(branches) => self.branches = branches,
                Work::SetAssignmentPattern(value) => self.in_assignment_pattern = value,
                Work::SetLabel(label) => self.context.current = self.finish(label),
                Work::SetCurrent(flow) => self.context.current = flow,
                Work::AddAntecedent(label) => self.add(label, self.context.current),
                Work::SetTargets {
                    break_target,
                    continue_target,
                } => {
                    self.context.break_target = break_target;
                    self.context.continue_target = continue_target;
                }
                Work::Return => {
                    if let Some(label) = self.context.return_target {
                        self.add(label, self.context.current);
                    }
                    self.context.current = FlowId::new(0);
                    self.context.explicit_return = true;
                    self.has_effects = true;
                }
                Work::Throw => {
                    self.context.current = FlowId::new(0);
                    self.has_effects = true;
                }
                Work::Jump { label } => {
                    if let Some(label) = label {
                        self.add(label, self.context.current);
                        self.context.current = FlowId::new(0);
                        self.has_effects = true;
                    }
                }
            }
        }
        self.graph.containers[0].end = Some(self.context.current);
        check_arenas(self.graph.nodes.len(), self.graph.containers.len())?;
        self.graph.reads = SiteMap::from_entries(self.reads);
        // The captured graph never grows: release what its growth left.
        self.graph.nodes.shrink_to_fit();
        self.graph.containers.shrink_to_fit();
        Ok(Capture {
            graph: self.graph,
            contains_this: self.contains_this,
        })
    }

    fn bind(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        // `bindChildren` carries assignment-pattern state only through the
        // native pattern-capable kinds, then restores the caller's state.
        let saved_pattern = self.in_assignment_pattern;
        self.in_assignment_pattern = false;
        self.work.push(Work::SetAssignmentPattern(saved_pattern));
        // `bindWorker`, `bindFunctionExpression`,
        // `bindPropertyOrMethodOrAccessor` and `bindChildren` assign the
        // optional FlowNode field before entering a child flow container.
        // Retain nil for other visited nodes too: getFlowNodeOfNode accepts
        // declaration/container nodes, while a missing captured site is an
        // invariant failure rather than an assumed nil field.
        let assigned_flow = match node.syntax_kind {
            SyntaxKind::Identifier
            | SyntaxKind::ThisKeyword
            | SyntaxKind::SuperKeyword
            | SyntaxKind::MetaProperty
            | SyntaxKind::BindingElement
            | SyntaxKind::FunctionExpression
            | SyntaxKind::ArrowFunction => true,
            SyntaxKind::QualifiedName => {
                crate::syntax::is_in_type_query(self.ast, |id| self.facts.parent(id), id)
            }
            SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression => {
                self.facts.narrowable(id)
            }
            kind => {
                (SyntaxKind::VariableStatement.number()..=SyntaxKind::DebuggerStatement.number())
                    .contains(&kind.number())
                    || crate::syntax::is_object_literal_or_class_expression_method_or_accessor(
                        kind,
                        self.facts
                            .parent(id)
                            .map(|parent| self.ast.node(parent).syntax_kind),
                    )
            }
        };
        self.reads.push((
            (node.syntax_kind, self.ast.token_span(id)),
            Read {
                flow: assigned_flow.then_some(self.context.current),
                container: self.context.container,
            },
        ));
        // `bindWorker` records only ThisKeyword/ThisType, before bindContainer
        // resets state. Property names that spell "this" are not this facts.
        if matches!(
            node.syntax_kind,
            SyntaxKind::ThisKeyword | SyntaxKind::ThisType
        ) {
            self.seen_this = true;
        }
        let flags = self.facts.container_flags(self.ast, id);
        if flags.control_flow() || flags.interface() {
            let saved = self.seen_this;
            self.seen_this = false;
            self.work.push(Work::EndSeenThis {
                saved,
                id,
                propagates: flags.propagates_this(),
            });
        }
        if flags.interface() {
            self.clear_unreachable_flow_field(id);
            self.children(id);
            return;
        }
        if id != self.ast.root_id() && flags.control_flow() {
            let saved = self.context;
            let container = ContainerId::new(self.graph.containers.len());
            self.graph.containers.push(Container {
                site: self.ast.token_span(id),
                kind: node.syntax_kind,
                parent_kind: self
                    .facts
                    .parent(id)
                    .map(|parent| self.ast.node(parent).syntax_kind),
                outer: Some(Read {
                    flow: Some(saved.current),
                    container: saved.container,
                }),
                end: None,
                return_flow: None,
                explicit_return: false,
                contains_this: false,
            });
            let immediate = self.facts.immediate(self.ast, id);
            let captures_outer = flags.captures_outer_flow();
            let current = if immediate {
                saved.current
            } else {
                self.push(Node::Start {
                    container: captures_outer.then_some(container),
                })
            };
            let return_target = (immediate
                || matches!(
                    node.syntax_kind,
                    SyntaxKind::Constructor | SyntaxKind::ClassStaticBlockDeclaration
                ))
            .then(|| self.label(false));
            self.context = Context {
                current,
                container,
                break_target: None,
                continue_target: None,
                return_target,
                exception_target: None,
                explicit_return: false,
            };
            self.work.push(Work::EndContainer {
                saved,
                labels: std::mem::take(&mut self.labels),
                id,
                immediate,
            });
            self.clear_unreachable_flow_field(id);
            if node.syntax_kind == SyntaxKind::ModuleBlock {
                self.statements(id);
            } else {
                self.children(id);
            }
            return;
        }
        if self.unreachable(self.context.current) {
            self.clear_unreachable_flow_field(id);
            self.children(id);
            return;
        }
        match node.syntax_kind {
            SyntaxKind::SourceFile | SyntaxKind::Block | SyntaxKind::ModuleBlock => {
                self.statements(id)
            }
            SyntaxKind::VariableDeclaration => {
                // `bindVariableDeclarationFlow` initializes loop variables
                // even when their declaration has no ordinary initializer.
                let in_iteration = self
                    .facts
                    .parent(id)
                    .and_then(|parent| self.facts.parent(parent))
                    .is_some_and(|parent| {
                        matches!(
                            self.ast.node(parent).syntax_kind,
                            SyntaxKind::ForInStatement | SyntaxKind::ForOfStatement
                        )
                    });
                if fields::id(node, "initializer").is_some() || in_iteration {
                    self.work.push(Work::InitializedVariable(id));
                }
                self.children(id);
            }
            SyntaxKind::BindingElement | SyntaxKind::Parameter => {
                self.bind_binding_or_parameter(id)
            }
            SyntaxKind::ExpressionStatement => {
                if let Some(expression) = fields::id(node, "expression") {
                    self.work.push(Work::MaybeCall(expression));
                    self.work.push(Work::Bind(expression));
                }
            }
            SyntaxKind::BinaryExpression => {
                if fields::is_destructuring_assignment(self.ast, id) {
                    self.bind_destructuring_assignment(id, saved_pattern);
                    return;
                }
                if fields::id(node, "operatorToken")
                    .is_some_and(|id| syntax::is_logical_operator(self.ast.node(id).syntax_kind))
                {
                    self.bind_logical(id);
                    return;
                }
                if fields::id(node, "operatorToken")
                    .is_some_and(|id| self.ast.node(id).syntax_kind == SyntaxKind::CommaToken)
                {
                    // `bindBinaryExpressionFlow`: calls in each comma operand
                    // take effect before the next operand is visited.
                    if let Some(right) = fields::id(node, "right") {
                        self.work.push(Work::MaybeCall(right));
                        self.work.push(Work::Bind(right));
                    }
                    if let Some(operator) = fields::id(node, "operatorToken") {
                        self.work.push(Work::Bind(operator));
                    }
                    if let Some(left) = fields::id(node, "left") {
                        self.work.push(Work::MaybeCall(left));
                        self.work.push(Work::Bind(left));
                    }
                    return;
                }
                self.work.push(Work::AfterBinary(id));
                self.children(id);
            }
            SyntaxKind::PrefixUnaryExpression | SyntaxKind::PostfixUnaryExpression => {
                if node.syntax_kind == SyntaxKind::PrefixUnaryExpression
                    && fields::number(node, "operator")
                        == Some(SyntaxKind::ExclamationToken.number())
                {
                    // `bindPrefixUnaryExpressionFlow` reverses only the
                    // conditional targets while binding logical negation.
                    self.work.push(Work::SetBranches(self.branches));
                    self.children(id);
                    self.work
                        .push(Work::SetBranches(self.branches.map(|(yes, no)| (no, yes))));
                    return;
                }
                if matches!(fields::number(node, "operator"), Some(value) if value == SyntaxKind::PlusPlusToken.number() || value == SyntaxKind::MinusMinusToken.number())
                {
                    if let Some(operand) = fields::id(node, "operand") {
                        self.work.push(Work::Assignment(operand));
                    }
                }
                self.children(id);
            }
            SyntaxKind::IfStatement => self.bind_if(id),
            SyntaxKind::ConditionalExpression => self.bind_conditional(id),
            SyntaxKind::WhileStatement => self.bind_while(id),
            SyntaxKind::DoStatement => self.bind_do(id),
            SyntaxKind::ForStatement => self.bind_for(id),
            SyntaxKind::ForInStatement | SyntaxKind::ForOfStatement => self.bind_for_in_or_of(id),
            SyntaxKind::ReturnStatement => {
                self.work.push(Work::Return);
                self.children(id);
            }
            SyntaxKind::ThrowStatement => {
                self.work.push(Work::Throw);
                self.children(id);
            }
            SyntaxKind::BreakStatement | SyntaxKind::ContinueStatement => {
                let target = if let Some(name) = fields::id(node, "label") {
                    // `findActiveLabel` compares native decoded identifiers,
                    // so an escaped label and its plain use share identity.
                    self.labels
                        .iter()
                        .rev()
                        .find(|label| {
                            self.ast.identifier_name(label.name) == self.ast.identifier_name(name)
                        })
                        .and_then(|label| {
                            if node.syntax_kind == SyntaxKind::BreakStatement {
                                Some(label.break_target)
                            } else {
                                label.continue_target
                            }
                        })
                } else {
                    if node.syntax_kind == SyntaxKind::BreakStatement {
                        self.context.break_target
                    } else {
                        self.context.continue_target
                    }
                };
                self.work.push(Work::Jump { label: target });
                self.children(id);
            }
            SyntaxKind::LabeledStatement => self.bind_labeled(id),
            SyntaxKind::TryStatement => self.bind_try(id),
            SyntaxKind::SwitchStatement => self.bind_switch(id),
            SyntaxKind::DeleteExpression => {
                // `bindDeleteExpressionFlow` mutates only an immediate
                // property access. Element and parenthesized accesses ask
                // a different native branch even with the same property.
                if let Some(expression) = fields::id(node, "expression")
                    && self.ast.node(expression).syntax_kind == SyntaxKind::PropertyAccessExpression
                {
                    self.work.push(Work::Assignment(expression));
                }
                self.children(id);
            }
            SyntaxKind::CallExpression => self.bind_call(id),
            SyntaxKind::ObjectLiteralExpression
            | SyntaxKind::ArrayLiteralExpression
            | SyntaxKind::PropertyAssignment
            | SyntaxKind::SpreadElement => {
                self.in_assignment_pattern = saved_pattern;
                self.children(id);
            }
            SyntaxKind::PropertyAccessExpression
            | SyntaxKind::ElementAccessExpression
            | SyntaxKind::NonNullExpression
                if fields::is_optional_chain(self.ast, id) =>
            {
                self.bind_optional(id);
            }
            _ => self.children(id),
        }
    }

    fn children(&mut self, id: NodeId) {
        let children: Vec<_> = self.ast.children(id).collect();
        self.work.extend(children.into_iter().rev().map(Work::Bind));
    }

    /// `bindBindingElementFlow` / `bindParameterFlow`: default expressions
    /// precede the name (which can itself be a nested binding pattern), and
    /// bindInitializer owns their unevaluated alternative.
    fn bind_binding_or_parameter(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        if let Some(name) = fields::id(node, "name") {
            self.work.push(Work::Bind(name));
        }
        if let Some(initializer) = fields::id(node, "initializer") {
            self.work.push(Work::Initializer(initializer));
        }
        if node.syntax_kind == SyntaxKind::Parameter {
            for field in ["type", "questionToken", "dotDotDotToken"] {
                if let Some(child) = fields::id(node, field) {
                    self.work.push(Work::Bind(child));
                }
            }
            self.work.extend(
                fields::ids(node, "modifiers")
                    .iter()
                    .rev()
                    .copied()
                    .map(Work::Bind),
            );
        } else {
            for field in ["propertyName", "dotDotDotToken"] {
                if let Some(child) = fields::id(node, field) {
                    self.work.push(Work::Bind(child));
                }
            }
        }
    }

    /// `bindDestructuringAssignmentFlow`: a nested pattern initializer binds
    /// its RHS first; an ordinary assignment binds its pattern first. The
    /// state changes stay local to this native binder operation.
    fn bind_destructuring_assignment(&mut self, id: NodeId, in_pattern: bool) {
        let node = self.ast.node(id);
        let left = fields::id(node, "left").expect("destructuring assignment has a left");
        let right = fields::id(node, "right").expect("destructuring assignment has a right");
        let operator =
            fields::id(node, "operatorToken").expect("destructuring assignment has an operator");
        self.work.push(Work::Assignment(left));
        if in_pattern {
            if let Some(ty) = fields::id(node, "type") {
                self.work.push(Work::Bind(ty));
            }
            self.work.push(Work::Bind(left));
            self.work.push(Work::SetAssignmentPattern(true));
            self.work.push(Work::Bind(right));
            self.work.push(Work::Bind(operator));
            self.work.push(Work::SetAssignmentPattern(false));
        } else {
            self.work.push(Work::Bind(right));
            self.work.push(Work::Bind(operator));
            self.work.push(Work::SetAssignmentPattern(false));
            if let Some(ty) = fields::id(node, "type") {
                self.work.push(Work::Bind(ty));
            }
            self.work.push(Work::Bind(left));
            self.work.push(Work::SetAssignmentPattern(true));
        }
    }

    /// `bindCallExpressionFlow`: IIFE arguments execute before its function
    /// body acquires the incoming flow. Other callees retain normal child
    /// order; super and array effects follow complete optional-chain binding.
    fn bind_call(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        self.work.push(Work::AfterCall(id));
        if fields::is_optional_chain(self.ast, id) {
            self.bind_optional(id);
            return;
        }
        let callee = fields::id(node, "expression").expect("call expression has a target");
        if matches!(
            self.ast
                .node(fields::skip_parentheses(self.ast, callee))
                .syntax_kind,
            SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction
        ) {
            self.work.push(Work::Bind(callee));
            self.work.extend(
                fields::ids(node, "arguments")
                    .iter()
                    .rev()
                    .copied()
                    .map(Work::Bind),
            );
            self.work.extend(
                fields::ids(node, "typeArguments")
                    .iter()
                    .rev()
                    .copied()
                    .map(Work::Bind),
            );
        } else {
            self.children(id);
        }
    }

    fn after_call(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        let callee = fields::id(node, "expression").expect("call expression has a target");
        if !fields::is_optional_chain(self.ast, id)
            && self.ast.node(callee).syntax_kind == SyntaxKind::SuperKeyword
        {
            self.mutation(Node::Call {
                expression: Site::new(self.ast, id),
                antecedent: self.context.current,
            });
        }
        let access = self.ast.node(callee);
        if access.syntax_kind == SyntaxKind::PropertyAccessExpression
            && fields::id(access, "expression").is_some_and(|receiver| self.facts.operand(receiver))
            && fields::id(access, "name")
                .is_some_and(|name| fields::is_push_or_unshift_identifier(self.ast, name))
        {
            self.mutation(Node::ArrayMutation {
                expression: Site::new(self.ast, id),
                antecedent: self.context.current,
            });
        }
    }

    /// `bindEachStatementFunctionsFirst`: function bodies have their own graph
    /// and are visited before ordinary statements of SourceFile/ModuleBlock/Block.
    /// Case/default clauses deliberately use native source-order bindEach.
    fn statements(&mut self, id: NodeId) {
        let statements = fields::ids(self.ast.node(id), "statements");
        self.work.extend(
            crate::syntax::statement_bind_order(self.ast, statements)
                .rev()
                .map(Work::Bind),
        );
    }

    /// `bindIfStatement` and `bindCondition` preserve both ordered branches and
    /// native label reduction, rather than flattening guards into a site flag.
    fn bind_if(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        let yes = self.label(false);
        let no = self.label(false);
        let after = self.label(false);
        self.work.push(Work::SetLabel(after));
        self.work.push(Work::AddAntecedent(after));
        if let Some(statement) = fields::id(node, "elseStatement") {
            self.work.push(Work::Bind(statement));
        }
        self.work.push(Work::SetLabel(no));
        self.work.push(Work::AddAntecedent(after));
        if let Some(statement) = fields::id(node, "thenStatement") {
            self.work.push(Work::Bind(statement));
        }
        self.work.push(Work::SetLabel(yes));
        self.schedule_condition(fields::id(node, "expression"), yes, no);
    }

    /// `bindConditionalExpressionFlow`: branch traversal keeps source order,
    /// but a conditional with no actual mutation/call/jump effects restores
    /// its original current flow. Its condition alone is not a lasting guard.
    fn bind_conditional(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        let yes = self.label(false);
        let no = self.label(false);
        let after = self.label(false);
        self.work.push(Work::EndExpression {
            saved_flow: self.context.current,
            saved_effects: self.has_effects,
            after,
        });
        self.has_effects = false;
        self.work.push(Work::AddAntecedent(after));
        if let Some(expression) = fields::id(node, "whenFalse") {
            self.work.push(Work::Bind(expression));
        }
        if let Some(token) = fields::id(node, "colonToken") {
            self.work.push(Work::Bind(token));
        }
        self.work.push(Work::SetLabel(no));
        self.work.push(Work::AddAntecedent(after));
        if let Some(expression) = fields::id(node, "whenTrue") {
            self.work.push(Work::Bind(expression));
        }
        if let Some(token) = fields::id(node, "questionToken") {
            self.work.push(Work::Bind(token));
        }
        self.work.push(Work::SetLabel(yes));
        self.schedule_condition(fields::id(node, "condition"), yes, no);
    }

    /// `bindBinaryExpressionFlow` / `bindLogicalLikeExpression`: short-circuit
    /// and nullish branches share the actual enclosing conditional targets.
    /// Top-level expressions retain a join only when traversal has effects.
    fn bind_logical(&mut self, id: NodeId) {
        let (yes, no) = if self.facts.top_level_logical(self.ast, id) {
            let after = self.label(false);
            self.work.push(Work::EndExpression {
                saved_flow: self.context.current,
                saved_effects: self.has_effects,
                after,
            });
            self.has_effects = false;
            (after, after)
        } else if let Some(branches) = self.branches {
            branches
        } else {
            // An intermediate unsupported enclosing optional operation has
            // not yet established its native branch targets. Keep that real
            // construction obligation instead of inventing targets or flow.
            self.mutation(Node::Pending {
                site: Site::new(self.ast, id),
                message: "native enclosing conditional branch targets are pending",
            });
            self.children(id);
            return;
        };
        let node = self.ast.node(id);
        let left = fields::id(node, "left").expect("logical binary has its left operand");
        let right = fields::id(node, "right").expect("logical binary has its right operand");
        let operator = fields::id(node, "operatorToken").expect("logical binary has its operator");
        let before_right = self.label(false);
        if self.facts.logical_assignment(self.ast, id) {
            self.work.push(Work::Condition {
                expression: Some(id),
                yes,
                no,
                after_bind: false,
            });
            self.work.push(Work::Assignment(left));
            self.schedule_in_branches(right, yes, no);
        } else {
            self.schedule_condition(Some(right), yes, no);
        }
        self.work.push(Work::Bind(operator));
        self.work.push(Work::SetLabel(before_right));
        if matches!(
            self.ast.node(operator).syntax_kind,
            SyntaxKind::AmpersandAmpersandToken | SyntaxKind::AmpersandAmpersandEqualsToken
        ) {
            self.schedule_condition(Some(left), before_right, no);
        } else {
            self.schedule_condition(Some(left), yes, before_right);
        }
    }

    /// `bindOptionalChainFlow` / `bindOptionalChain`: each root routes its
    /// receiver before the chain rest. Unlike logical-expression binding, the
    /// native top-level operation does not reset the incoming effects flag.
    fn bind_optional(&mut self, id: NodeId) {
        let (yes, no) = if self.facts.top_level_logical(self.ast, id) {
            let after = self.label(false);
            self.work.push(Work::EndExpression {
                saved_flow: self.context.current,
                saved_effects: self.has_effects,
                after,
            });
            (after, after)
        } else {
            self.branches
                .expect("a nested native optional chain has conditional targets")
        };
        if self.facts.outermost_optional(self.ast, id) {
            self.work.push(Work::Condition {
                expression: Some(id),
                yes,
                no,
                after_bind: false,
            });
        }
        self.work.push(Work::OptionalRest { node: id, yes, no });
        let before_rest = self
            .facts
            .optional_root(self.ast, id)
            .then(|| self.label(false));
        if let Some(label) = before_rest {
            self.work.push(Work::SetLabel(label));
        }
        let receiver = fields::id(self.ast.node(id), "expression")
            .expect("a native optional chain has a receiver");
        // `bindOptionalExpression` adds conditions even when a logical
        // receiver already routed its branches. Only an inner chain skips them.
        if !fields::is_optional_chain(self.ast, receiver)
            || self.facts.outermost_optional(self.ast, receiver)
        {
            self.work.push(Work::Condition {
                expression: Some(receiver),
                yes: before_rest.unwrap_or(yes),
                no,
                after_bind: false,
            });
        }
        self.schedule_in_branches(receiver, before_rest.unwrap_or(yes), no);
    }

    /// `bindOptionalChainRest`: preserve actual question-dot/name, element
    /// argument and call type-argument/argument order under the enclosing pair.
    /// NonNull has no rest; its receiver was visited by bindOptionalChain.
    fn optional_rest(&mut self, id: NodeId, yes: FlowId, no: FlowId) {
        let node = self.ast.node(id);
        self.work.push(Work::SetBranches(self.branches));
        match node.syntax_kind {
            SyntaxKind::PropertyAccessExpression => {
                if let Some(name) = fields::id(node, "name") {
                    self.work.push(Work::Bind(name));
                }
            }
            SyntaxKind::ElementAccessExpression => {
                if let Some(argument) = fields::id(node, "argumentExpression") {
                    self.work.push(Work::Bind(argument));
                }
            }
            SyntaxKind::CallExpression => {
                self.work.extend(
                    fields::ids(node, "arguments")
                        .iter()
                        .rev()
                        .copied()
                        .map(Work::Bind),
                );
                self.work.extend(
                    fields::ids(node, "typeArguments")
                        .iter()
                        .rev()
                        .copied()
                        .map(Work::Bind),
                );
            }
            SyntaxKind::NonNullExpression => {}
            _ => unreachable!("native optional-chain kind has a rest operation"),
        }
        if let Some(token) = fields::id(node, "questionDotToken") {
            self.work.push(Work::Bind(token));
        }
        self.work.push(Work::SetBranches(Some((yes, no))));
    }

    /// `bindWhileStatement`: the loop label retains its actual back edge;
    /// unreachable edges are excluded by addAntecedent, not textual screening.
    fn bind_while(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        let before = self.label(true);
        let body = self.label(false);
        let after = self.label(false);
        self.set_continue_target(id, before);
        self.add(before, self.context.current);
        let saved_break = self.context.break_target;
        let saved_continue = self.context.continue_target;
        self.context.current = before;
        self.work.push(Work::SetTargets {
            break_target: saved_break,
            continue_target: saved_continue,
        });
        self.work.push(Work::SetLabel(after));
        self.work.push(Work::AddAntecedent(before));
        if let Some(statement) = fields::id(node, "statement") {
            self.work.push(Work::Bind(statement));
        }
        self.work.push(Work::SetTargets {
            break_target: Some(after),
            continue_target: Some(before),
        });
        self.work.push(Work::SetLabel(body));
        self.schedule_condition(fields::id(node, "expression"), body, after);
    }

    /// `bindDoStatement`: continue edges enter the condition, while the loop
    /// header's first antecedent remains the actual non-looping entry path.
    fn bind_do(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        let before = self.label(true);
        let condition = self.label(false);
        let after = self.label(false);
        self.set_continue_target(id, condition);
        self.add(before, self.context.current);
        let saved_break = self.context.break_target;
        let saved_continue = self.context.continue_target;
        self.context.current = before;
        self.work.push(Work::SetLabel(after));
        self.schedule_condition(fields::id(node, "expression"), before, after);
        self.work.push(Work::SetLabel(condition));
        self.work.push(Work::AddAntecedent(condition));
        self.work.push(Work::SetTargets {
            break_target: saved_break,
            continue_target: saved_continue,
        });
        if let Some(statement) = fields::id(node, "statement") {
            self.work.push(Work::Bind(statement));
        }
        self.work.push(Work::SetTargets {
            break_target: Some(after),
            continue_target: Some(condition),
        });
    }

    /// `bindForStatement`: initializer, condition, body and incrementor have
    /// their compiler execution order; continue targets the incrementor.
    fn bind_for(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        let before = self.label(true);
        let body = self.label(false);
        let incrementor = self.label(false);
        let after = self.label(false);
        self.set_continue_target(id, before);
        let saved_break = self.context.break_target;
        let saved_continue = self.context.continue_target;
        self.work.push(Work::SetLabel(after));
        self.work.push(Work::AddAntecedent(before));
        if let Some(expression) = fields::id(node, "incrementor") {
            self.work.push(Work::Bind(expression));
        }
        self.work.push(Work::SetLabel(incrementor));
        self.work.push(Work::AddAntecedent(incrementor));
        self.work.push(Work::SetTargets {
            break_target: saved_break,
            continue_target: saved_continue,
        });
        if let Some(statement) = fields::id(node, "statement") {
            self.work.push(Work::Bind(statement));
        }
        self.work.push(Work::SetTargets {
            break_target: Some(after),
            continue_target: Some(incrementor),
        });
        self.work.push(Work::SetLabel(body));
        self.schedule_condition(fields::id(node, "condition"), body, after);
        self.work.push(Work::SetCurrent(before));
        self.work.push(Work::AddAntecedent(before));
        if let Some(initializer) = fields::id(node, "initializer") {
            self.work.push(Work::Bind(initializer));
        }
    }

    /// `bindForInOrForOfStatement`: iterable evaluation precedes the loop,
    /// the exit edge precedes the initializer, and the initializer's own
    /// assignment follows its child traversal.
    fn bind_for_in_or_of(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        let before = self.label(true);
        let after = self.label(false);
        self.set_continue_target(id, before);
        let saved_break = self.context.break_target;
        let saved_continue = self.context.continue_target;
        self.work.push(Work::SetLabel(after));
        self.work.push(Work::AddAntecedent(before));
        self.work.push(Work::SetTargets {
            break_target: saved_break,
            continue_target: saved_continue,
        });
        if let Some(statement) = fields::id(node, "statement") {
            self.work.push(Work::Bind(statement));
        }
        self.work.push(Work::SetTargets {
            break_target: Some(after),
            continue_target: Some(before),
        });
        if let Some(initializer) = fields::id(node, "initializer") {
            if self.ast.node(initializer).syntax_kind != SyntaxKind::VariableDeclarationList {
                self.work.push(Work::Assignment(initializer));
            }
            self.work.push(Work::Bind(initializer));
        }
        self.work.push(Work::AddAntecedent(after));
        if let Some(modifier) = fields::id(node, "awaitModifier") {
            self.work.push(Work::Bind(modifier));
        }
        self.work.push(Work::SetCurrent(before));
        self.work.push(Work::AddAntecedent(before));
        if let Some(expression) = fields::id(node, "expression") {
            self.work.push(Work::Bind(expression));
        }
    }

    /// `setContinueTarget`: only labels directly wrapping this loop inherit
    /// its native target. Container entry clears this transient label stack.
    fn set_continue_target(&mut self, mut node: NodeId, target: FlowId) {
        for label in self.labels.iter_mut().rev() {
            let Some(parent) = self.facts.parent(node) else {
                break;
            };
            if self.ast.node(parent).syntax_kind != SyntaxKind::LabeledStatement
                || label.node != parent
            {
                break;
            }
            label.continue_target = Some(target);
            node = parent;
        }
    }

    /// `bindLabeledStatement` owns active target lifetime and the exact
    /// post-statement join. Unused-label reporting does not affect query flow.
    fn bind_labeled(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        let name = fields::id(node, "label").expect("a label statement has its identifier");
        let after = self.label(false);
        self.labels.push(ActiveLabel {
            node: id,
            name,
            break_target: after,
            continue_target: None,
        });
        self.work.push(Work::EndLabel(after));
        if let Some(statement) = fields::id(node, "statement") {
            self.work.push(Work::Bind(statement));
        }
        self.work.push(Work::Bind(name));
    }

    fn schedule_condition(&mut self, expression: Option<NodeId>, yes: FlowId, no: FlowId) {
        self.work.push(Work::Condition {
            expression,
            yes,
            no,
            after_bind: true,
        });
        if let Some(expression) = expression {
            self.schedule_in_branches(expression, yes, no);
        }
    }

    /// `doWithConditionalBranches`: restore the actual caller's target pair
    /// after child traversal. This state never becomes a retained site index.
    fn schedule_in_branches(&mut self, id: NodeId, yes: FlowId, no: FlowId) {
        self.work.push(Work::SetBranches(self.branches));
        self.work.push(Work::Bind(id));
        self.work.push(Work::SetBranches(Some((yes, no))));
    }

    /// `createFlowCondition`: literal impossibility is native only outside
    /// optional-root/nullish contexts; non-narrowing expressions add no node.
    fn condition(&mut self, expression: Option<NodeId>, truthy: bool) -> FlowId {
        if self.unreachable(self.context.current) {
            return self.context.current;
        }
        let Some(expression) = expression else {
            return if truthy {
                self.context.current
            } else {
                FlowId::new(0)
            };
        };
        let kind = self.ast.node(expression).syntax_kind;
        if (kind == SyntaxKind::TrueKeyword && !truthy
            || kind == SyntaxKind::FalseKeyword && truthy)
            && !fields::is_expression_of_optional_chain_root(
                self.ast,
                expression,
                self.facts.parent(expression),
            )
            && !self.facts.nullish_parent(self.ast, expression)
        {
            return FlowId::new(0);
        }
        if !self.facts.narrowing(expression) {
            return self.context.current;
        }
        self.reference(self.context.current);
        self.push(Node::Condition {
            expression: Site::new(self.ast, expression),
            truthy,
            antecedent: self.context.current,
        })
    }

    /// `bindInitializedVariableFlow`: every actual binding leaf gets its own
    /// assignment node, in native pattern order, with its declaration role.
    fn initialized_variable(&mut self, root: NodeId) {
        let mut work = vec![root];
        while let Some(id) = work.pop() {
            let name = match self.ast.node(id).syntax_kind {
                SyntaxKind::VariableDeclaration | SyntaxKind::BindingElement => {
                    fields::id(self.ast.node(id), "name")
                }
                _ => None,
            };
            if let Some(name) = name
                && matches!(
                    self.ast.node(name).syntax_kind,
                    SyntaxKind::ObjectBindingPattern | SyntaxKind::ArrayBindingPattern
                )
            {
                work.extend(
                    fields::ids(self.ast.node(name), "elements")
                        .iter()
                        .rev()
                        .copied(),
                );
            } else {
                self.mutation(Node::Assignment {
                    target: Site::new(self.ast, id),
                    antecedent: self.context.current,
                });
            }
        }
    }

    /// `bindAssignmentTargetFlow` / `bindDestructuringTargetFlow`: destructure
    /// the actual assignment syntax; a default's left is its target, and the
    /// array/object container is never substituted for all of its leaves.
    fn assignment(&mut self, root: NodeId) {
        let mut work = vec![root];
        while let Some(id) = work.pop() {
            let node = self.ast.node(id);
            match node.syntax_kind {
                SyntaxKind::ArrayLiteralExpression => {
                    for &element in fields::ids(node, "elements").iter().rev() {
                        let element_node = self.ast.node(element);
                        let target = if element_node.syntax_kind == SyntaxKind::SpreadElement {
                            fields::id(element_node, "expression")
                                .expect("spread assignment has an expression")
                        } else {
                            self.destructuring_target(element)
                        };
                        work.push(target);
                    }
                }
                SyntaxKind::ObjectLiteralExpression => {
                    for &property in fields::ids(node, "properties").iter().rev() {
                        let property_node = self.ast.node(property);
                        let target = match property_node.syntax_kind {
                            SyntaxKind::PropertyAssignment => Some(
                                self.destructuring_target(
                                    fields::id(property_node, "initializer")
                                        .expect("property assignment has an initializer"),
                                ),
                            ),
                            SyntaxKind::ShorthandPropertyAssignment => {
                                fields::id(property_node, "name")
                            }
                            SyntaxKind::SpreadAssignment => fields::id(property_node, "expression"),
                            _ => None,
                        };
                        if let Some(target) = target {
                            work.push(target);
                        }
                    }
                }
                _ if self.facts.narrowable(id) => self.mutation(Node::Assignment {
                    target: Site::new(self.ast, id),
                    antecedent: self.context.current,
                }),
                _ => {}
            }
        }
    }

    fn destructuring_target(&self, id: NodeId) -> NodeId {
        let node = self.ast.node(id);
        if node.syntax_kind == SyntaxKind::BinaryExpression
            && fields::id(node, "operatorToken").is_some_and(|operator| {
                self.ast.node(operator).syntax_kind == SyntaxKind::EqualsToken
            })
        {
            fields::id(node, "left").expect("destructuring default has a left")
        } else {
            id
        }
    }

    fn after_binary(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        let Some(operator) = fields::id(node, "operatorToken") else {
            return;
        };
        let operator = self.ast.node(operator).syntax_kind;
        if fields::is_assignment_operator(operator)
            && self.facts.assignment_target(self.ast, id).is_none()
        {
            if let Some(left) = fields::id(node, "left") {
                self.assignment(left);
                let access = self.ast.node(left);
                if operator == SyntaxKind::EqualsToken
                    && access.syntax_kind == SyntaxKind::ElementAccessExpression
                    && fields::id(access, "expression")
                        .is_some_and(|receiver| self.facts.operand(receiver))
                {
                    self.mutation(Node::ArrayMutation {
                        expression: Site::new(self.ast, id),
                        antecedent: self.context.current,
                    });
                }
            }
        }
    }

    /// `maybeBindExpressionFlowIfCall`: the code's actual dotted-name rule
    /// includes zero-argument calls too, despite the narrower old comment.
    fn maybe_call(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        if node.syntax_kind == SyntaxKind::CallExpression {
            if let Some(callee) = fields::id(node, "expression") {
                if self.ast.node(callee).syntax_kind != SyntaxKind::SuperKeyword
                    && self.facts.dotted(callee)
                {
                    self.mutation(Node::Call {
                        expression: Site::new(self.ast, id),
                        antecedent: self.context.current,
                    });
                }
            }
        }
    }

    fn push(&mut self, node: Node) -> FlowId {
        let id = FlowId::new(self.graph.nodes.len());
        self.graph.nodes.push(Entry {
            node,
            referenced: false,
            shared: false,
        });
        id
    }

    fn label(&mut self, looping: bool) -> FlowId {
        self.push(Node::Label {
            looping,
            antecedents: Vec::new(),
        })
    }
    fn unreachable(&self, id: FlowId) -> bool {
        matches!(self.graph.node(id), Node::Unreachable)
    }

    /// `bindChildren` clears the preassigned FlowNode field only after
    /// `bindContainer` has established its current flow. A function expression
    /// entered from unreachable code keeps its original unreachable field when
    /// its body starts a new flow; an IIFE sharing that flow clears it.
    fn clear_unreachable_flow_field(&mut self, id: NodeId) {
        if self.unreachable(self.context.current) {
            // This runs within bind, after its attachment was appended and
            // before any child is dispatched. Clear that exact last write;
            // freezing later resolves any duplicate sites in binder order.
            let (site, read) = self
                .reads
                .last_mut()
                .expect("bind appended its flow attachment");
            debug_assert_eq!(
                *site,
                (self.ast.node(id).syntax_kind, self.ast.token_span(id))
            );
            read.flow = None;
        }
    }

    /// `setFlowNodeReferenced` preserves the first/shared distinction.
    fn reference(&mut self, id: FlowId) {
        let entry = &mut self.graph.nodes[id.index()];
        if entry.referenced {
            entry.shared = true;
        } else {
            entry.referenced = true;
        }
    }

    fn mutation(&mut self, node: Node) {
        self.reference(self.context.current);
        self.has_effects = true;
        let exception_mutation =
            matches!(node, Node::Assignment { .. } | Node::ArrayMutation { .. });
        self.context.current = self.push(node);
        // `createFlowMutation` adds assignment and array mutations to the
        // current exception target. `createFlowCall` deliberately does not.
        if exception_mutation && let Some(target) = self.context.exception_target {
            self.add(target, self.context.current);
        }
    }

    /// `addAntecedent`: ignore unreachable and duplicate identities while
    /// retaining native insertion order, including loop back edges.
    fn add(&mut self, label: FlowId, antecedent: FlowId) {
        if self.unreachable(antecedent) {
            return;
        }
        let Node::Label { antecedents, .. } = &mut self.graph.nodes[label.index()].node else {
            unreachable!("antecedent target is a native label")
        };
        if antecedents.contains(&antecedent) {
            return;
        }
        antecedents.push(antecedent);
        self.reference(antecedent);
    }

    /// `finishFlowLabel`: no antecedent is unreachable; one reuses its exact
    /// identity; multiple keep the label and its ordered alternatives.
    fn finish(&self, label: FlowId) -> FlowId {
        let Node::Label { antecedents, .. } = self.graph.node(label) else {
            unreachable!("finished target is a native label")
        };
        match antecedents.as_slice() {
            [] => FlowId::new(0),
            [only] => *only,
            _ => label,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_flow_attachments_keep_their_identity_niches() {
        assert_eq!(
            std::mem::size_of::<Option<FlowId>>(),
            std::mem::size_of::<usize>()
        );
        assert_eq!(
            std::mem::size_of::<Read>(),
            2 * std::mem::size_of::<usize>()
        );
        assert_eq!(
            std::mem::size_of::<Option<Read>>(),
            std::mem::size_of::<Read>()
        );
        for index in [0, 1, u32::MAX as usize - 1, usize::MAX - 1] {
            assert_eq!(FlowId::new(index).index(), index);
            assert_eq!(ContainerId::new(index).index(), index);
        }
    }

    #[test]
    fn frozen_reads_preserve_nil_unreachable_and_u32_identities() {
        let site = Site {
            span: Span { start: 2, end: 4 },
            kind: SyntaxKind::Identifier,
        };
        for flow in [
            None,
            Some(FlowId::new(0)),
            Some(FlowId::new(u32::MAX as usize - 1)),
        ] {
            for container in [
                ContainerId::new(0),
                ContainerId::new((u32::MAX as usize).min(usize::MAX - 1)),
            ] {
                let graph = Graph {
                    reads: SiteMap::from_entries(vec![(
                        (site.kind, site.span),
                        Read { flow, container },
                    )]),
                    ..Graph::default()
                };
                let read = graph
                    .read(site)
                    .expect("the exact captured site is retained");
                assert_eq!((read.flow, read.container), (flow, container));
                assert!(
                    graph
                        .read(Site {
                            kind: SyntaxKind::StringLiteral,
                            ..site
                        })
                        .is_none()
                );
            }
        }
    }

    /// The arena limit at and just over `u32::MAX` entries, for each arena.
    /// No end-to-end variant runs: a file with four billion flow nodes needs
    /// hundreds of gigabytes to parse and bind, so the rule takes the counts
    /// as values, and `Builder::run` applies it to every file's graph.
    #[test]
    fn a_flow_graph_over_u32_max_nodes_or_containers_is_unsupported() {
        let max = u32::MAX as usize;
        assert!(check_arenas(max, max).is_ok());
        for (nodes, containers) in [(max + 1, 1), (2, max + 1)] {
            let error = check_arenas(nodes, containers)
                .expect_err("a flow arena one entry over the limit is refused");
            assert_eq!(error.kind, InputErrorKind::Unsupported);
            assert_eq!(error.path, None);
            assert_eq!(error.position, None);
            assert_eq!(
                error.message,
                "control flow has more than 4294967295 nodes or containers; \
                 tsparse supports up to u32::MAX of each per file"
            );
        }
    }
}
