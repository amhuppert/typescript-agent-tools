//! Native exception targets and finally reductions share the binder's graph.
//! Scheduling frames are transient; published reductions retain only FlowIds.

use super::{Builder, FlowId, Node, Work as BindWork};
use crate::{NodeId, syntax as fields};

pub(super) struct TryFrame {
    normal: FlowId,
    returns: FlowId,
    exceptions: FlowId,
    saved_return: Option<FlowId>,
    saved_exception: Option<FlowId>,
    catch: Option<NodeId>,
    finally: Option<NodeId>,
}

pub(super) enum Work {
    Try(TryFrame),
    Catch(TryFrame),
    Finally { frame: TryFrame, label: FlowId },
}

impl Builder<'_, '_> {
    /// `bindTryStatement`: exceptions begin before mutations, catch has its
    /// own exception target, and finally sees normal, exception and return
    /// paths in that native order. Reduced exits preserve each caller's path.
    pub(super) fn bind_try(&mut self, id: NodeId) {
        let node = self.ast.node(id);
        let frame = TryFrame {
            normal: self.label(false),
            returns: self.label(false),
            exceptions: self.label(false),
            saved_return: self.context.return_target,
            saved_exception: self.context.exception_target,
            catch: fields::id(node, "catchClause"),
            finally: fields::id(node, "finallyBlock"),
        };
        if frame.finally.is_some() {
            self.context.return_target = Some(frame.returns);
        }
        self.add(frame.exceptions, self.context.current);
        self.context.exception_target = Some(frame.exceptions);
        self.work.push(BindWork::Try(Work::Try(frame)));
        self.work.push(BindWork::Bind(
            fields::id(node, "tryBlock").expect("try has its block"),
        ));
    }

    pub(super) fn try_step(&mut self, work: Work) {
        match work {
            Work::Try(mut frame) => {
                self.add(frame.normal, self.context.current);
                if let Some(catch) = frame.catch {
                    self.context.current = self.finish(frame.exceptions);
                    frame.exceptions = self.label(false);
                    self.add(frame.exceptions, self.context.current);
                    self.context.exception_target = Some(frame.exceptions);
                    self.work.push(BindWork::Try(Work::Catch(frame)));
                    self.work.push(BindWork::Bind(catch));
                } else {
                    self.before_finally(frame);
                }
            }
            Work::Catch(frame) => {
                self.add(frame.normal, self.context.current);
                self.before_finally(frame);
            }
            Work::Finally { frame, label } => {
                if self.unreachable(self.context.current) {
                    return;
                }
                if let Some(target) = self.context.return_target
                    && !self.label_antecedents(frame.returns).is_empty()
                {
                    let reduced = self.reduce_label(label, frame.returns);
                    self.add(target, reduced);
                }
                if let Some(target) = self.context.exception_target
                    && !self.label_antecedents(frame.exceptions).is_empty()
                {
                    let reduced = self.reduce_label(label, frame.exceptions);
                    self.add(target, reduced);
                }
                self.context.current = if self.label_antecedents(frame.normal).is_empty() {
                    FlowId::new(0)
                } else {
                    self.reduce_label(label, frame.normal)
                };
            }
        }
    }

    fn before_finally(&mut self, frame: TryFrame) {
        self.context.return_target = frame.saved_return;
        self.context.exception_target = frame.saved_exception;
        if let Some(finally) = frame.finally {
            let mut antecedents = self.label_antecedents(frame.normal).to_vec();
            antecedents.extend_from_slice(self.label_antecedents(frame.exceptions));
            antecedents.extend_from_slice(self.label_antecedents(frame.returns));
            // `combineFlowLists` concatenates, without the ordinary label's
            // deduplication or extra setFlowNodeReferenced calls.
            let label = self.push(Node::Label {
                looping: false,
                antecedents,
            });
            self.context.current = label;
            self.work
                .push(BindWork::Try(Work::Finally { frame, label }));
            self.work.push(BindWork::Bind(finally));
        } else {
            self.context.current = self.finish(frame.normal);
        }
    }

    fn label_antecedents(&self, flow: FlowId) -> &[FlowId] {
        let Node::Label { antecedents, .. } = self.graph.node(flow) else {
            unreachable!("try path target is a native label")
        };
        antecedents
    }

    /// `createReduceLabel`: retain a copied subset, without mutating the
    /// finally label or inventing a referenced bit on its final antecedent.
    fn reduce_label(&mut self, target: FlowId, subset: FlowId) -> FlowId {
        self.push(Node::ReduceLabel {
            target,
            antecedents: self.label_antecedents(subset).to_vec(),
            antecedent: self.context.current,
        })
    }
}
