//! Whole native switch binding keeps case grouping and flow restoration local.

use super::{Builder, FlowId, Node, Site, Work as BindWork};
use crate::syntax_kind::SyntaxKind;
use crate::{NodeId, syntax as fields};

pub(super) struct Frame {
    statement: NodeId,
    clauses: Vec<NodeId>,
    pre: FlowId,
    after: FlowId,
    saved_break: Option<FlowId>,
    fallthrough: FlowId,
    index: usize,
    group_start: usize,
}

pub(super) enum Work {
    Expression { statement: NodeId, after: FlowId },
    Empty(Frame),
    Clause(Frame),
}

impl Builder<'_, '_> {
    /// `bindSwitchStatement`: the expression precedes the new break target.
    /// Every active switch owns its exact pre-case flow in its transient frame.
    pub(super) fn bind_switch(&mut self, statement: NodeId) {
        let after = self.label(false);
        self.work
            .push(BindWork::Switch(Work::Expression { statement, after }));
        self.work.push(BindWork::Bind(
            fields::id(self.ast.node(statement), "expression")
                .expect("accepted switch has its expression"),
        ));
    }

    pub(super) fn switch_step(&mut self, work: Work) {
        match work {
            Work::Expression { statement, after } => {
                let block = fields::id(self.ast.node(statement), "caseBlock")
                    .expect("accepted switch has its case block");
                let frame = Frame {
                    statement,
                    clauses: fields::ids(self.ast.node(block), "clauses").to_vec(),
                    pre: self.context.current,
                    after,
                    saved_break: self.context.break_target,
                    fallthrough: FlowId::new(0),
                    index: 0,
                    group_start: 0,
                };
                self.context.break_target = Some(after);
                self.switch_group(frame);
            }
            Work::Empty(mut frame) => {
                frame.index += 1;
                self.switch_empty_group(frame);
            }
            Work::Clause(mut frame) => {
                frame.fallthrough = self.context.current;
                frame.index += 1;
                self.switch_group(frame);
            }
        }
    }

    /// `bindCaseBlock`: empty clauses are bound before the group's entry label,
    /// then pre-case and fallthrough paths are added in that native order.
    fn switch_group(&mut self, mut frame: Frame) {
        if frame.index == frame.clauses.len() {
            self.add(frame.after, self.context.current);
            if !frame
                .clauses
                .iter()
                .any(|&clause| self.ast.node(clause).syntax_kind == SyntaxKind::DefaultClause)
            {
                let bypass = self.switch_clause(&frame, 0, 0);
                self.add(frame.after, bypass);
            }
            self.context.break_target = frame.saved_break;
            self.context.current = self.finish(frame.after);
            return;
        }
        frame.group_start = frame.index;
        self.switch_empty_group(frame);
    }

    fn switch_empty_group(&mut self, frame: Frame) {
        let clause = frame.clauses[frame.index];
        if fields::ids(self.ast.node(clause), "statements").is_empty()
            && frame.index + 1 < frame.clauses.len()
        {
            if self.unreachable(frame.fallthrough) {
                self.context.current = frame.pre;
            }
            let pre = frame.pre;
            self.work.push(BindWork::Switch(Work::Empty(frame)));
            self.bind_switch_clause(clause, pre);
            return;
        }
        let expression = fields::id(self.ast.node(frame.statement), "expression")
            .expect("accepted switch has its expression");
        let entry = self.label(false);
        let pre = if self.ast.node(expression).syntax_kind == SyntaxKind::TrueKeyword
            || self.facts.narrowing(expression)
        {
            self.switch_clause(&frame, frame.group_start, frame.index + 1)
        } else {
            frame.pre
        };
        self.add(entry, pre);
        self.add(entry, frame.fallthrough);
        self.context.current = self.finish(entry);
        let pre = frame.pre;
        self.work.push(BindWork::Switch(Work::Clause(frame)));
        self.bind_switch_clause(clause, pre);
    }

    /// `bindCaseOrDefaultClause`: case expression effects are bound at the
    /// pre-switch flow and restored before the clause statements execute.
    fn bind_switch_clause(&mut self, clause: NodeId, pre: FlowId) {
        self.work.extend(
            fields::ids(self.ast.node(clause), "statements")
                .iter()
                .rev()
                .copied()
                .map(BindWork::Bind),
        );
        if let Some(expression) = fields::id(self.ast.node(clause), "expression") {
            let saved = self.context.current;
            self.work.push(BindWork::SetCurrent(saved));
            self.work.push(BindWork::Bind(expression));
            self.work.push(BindWork::SetCurrent(pre));
        }
    }

    /// `createFlowSwitchClause` sets only native reference state; it is not a
    /// mutation and does not create an exception edge or hasFlowEffects bit.
    fn switch_clause(&mut self, frame: &Frame, start: usize, end: usize) -> FlowId {
        self.reference(frame.pre);
        self.push(Node::SwitchClause {
            statement: Site::new(self.ast, frame.statement),
            start,
            end,
            antecedent: frame.pre,
        })
    }
}
