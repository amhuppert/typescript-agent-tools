//! End reachability is a separate native question from read narrowing.
//! Explicit continuations preserve its caches and temporary reduced labels.

use crate::project::semantic::model::{NativeCompletion, NativeResult, NativeSequence};
use crate::resources::complete_or_abort;

use super::{
    DemandError, Evaluator, FileId, FlowId, Location, Node, TypeFlags, TypePredicateKind,
    TypePredicateSubject, branch_antecedents,
};

enum Work {
    Visit {
        frame: usize,
        flow: FlowId,
        no_cache_check: bool,
    },
    Cache {
        frame: usize,
        antecedent: usize,
        flow: FlowId,
    },
    Branch {
        frame: usize,
        antecedent: usize,
        flow: FlowId,
        index: usize,
    },
    Reduce {
        frame: usize,
        antecedent: usize,
        reduction: FlowId,
    },
}

/// Each native worker owns its own ordered acquisitions. Shared publication
/// copies only its nested worker result; an outer tail prefix stays outside
/// that slot. NativeSequence remains the single causal-composition owner.
struct Invocation {
    parent: Option<usize>,
    progress: Option<NativeSequence>,
    result: Option<NativeCompletion<bool>>,
}
impl Invocation {
    fn new(parent: Option<usize>) -> Self {
        Self {
            parent,
            progress: Some(NativeSequence::new()),
            result: None,
        }
    }
}
struct Invocations {
    frames: Vec<Invocation>,
}
impl Invocations {
    fn new() -> Self {
        let frames = vec![Invocation::new(None)];
        Self { frames }
    }
    fn child(&mut self, parent: usize) -> usize {
        let frame = self.frames.len();
        self.frames.push(Invocation::new(Some(parent)));
        frame
    }
    fn result(&self, frame: usize) -> NativeResult<bool> {
        self.frames[frame].result.clone().ok_or_else(|| {
            super::super::super::internal("native reachability invocation did not finish").into()
        })
    }
    fn read<T>(&mut self, frame: usize, next: NativeResult<T>) -> Result<T, DemandError> {
        self.frames[frame]
            .progress
            .as_mut()
            .ok_or_else(|| {
                super::super::super::internal(
                    "completed reachability worker acquired another dependency",
                )
            })?
            .read(next)
    }
    fn finish(&mut self, frame: usize, result: bool) -> Result<(), DemandError> {
        let progress = self.frames[frame].progress.take().ok_or_else(|| {
            super::super::super::internal("native reachability worker completed twice")
        })?;
        self.frames[frame].result = Some(progress.finish(Ok(result))?);
        Ok(())
    }
    fn read_result(&mut self, parent: usize, child: usize) -> Result<bool, DemandError> {
        self.read(parent, self.result(child))
    }
    fn finish_with(&mut self, frame: usize, result: NativeResult<bool>) -> Result<(), DemandError> {
        let result = self.read(frame, result)?;
        self.finish(frame, result)
    }
    fn fail(&mut self, mut frame: usize, mut failure: DemandError) -> DemandError {
        loop {
            if let Some(progress) = self.frames[frame].progress.take() {
                failure = match progress.finish::<()>(Err(failure)) {
                    Err(failure) => failure,
                    Ok(_) => unreachable!("interrupted reachability cannot complete"),
                };
            }
            match self.frames[frame].parent {
                Some(parent) => frame = parent,
                None => return failure,
            }
        }
    }
}
impl Work {
    fn frame(&self) -> usize {
        match self {
            Self::Visit { frame, .. }
            | Self::Cache { frame, .. }
            | Self::Branch { frame, .. }
            | Self::Reduce { frame, .. } => *frame,
        }
    }
}

impl Evaluator<'_, '_> {
    /// `isReachableFlowNode` / `isReachableFlowNodeWorker`: native Shared and
    /// last-node caches, ordered branch exits, loop entry and call effects.
    /// ReduceLabel clears only last-node history, as the compiler does.
    /// Native traversal stays on an explicit stack; recursive reentry shares
    /// the request depth guard.
    pub(in crate::project::semantic::evaluation) fn reachable_flow_completion(
        &mut self,
        file: FileId,
        root: FlowId,
        location: &Location,
    ) -> NativeResult<bool> {
        // The graph walk is iterative; effects-signature callbacks can reenter
        // reachability through another function's return inference.
        let _depth = self
            .control
            .as_ref()
            .map(|control| complete_or_abort(control.enter()));
        let graph = self
            .inputs
            .bindings
            .flow_graph(file)
            .map_err(|error| self.read_binding_failure(location, error))?;
        let mut invocations = Invocations::new();
        let mut reductions = Vec::new();
        let mut work = Vec::new();
        work.push(Work::Visit {
            frame: 0,
            flow: root,
            no_cache_check: false,
        });
        let mut active_frame = 0;
        let execution = (|| -> NativeResult<bool> {
            while let Some(task) = work.pop() {
                active_frame = task.frame();
                match task {
                    Work::Cache {
                        frame,
                        antecedent,
                        flow,
                    } => {
                        let reachable = invocations.result(antecedent)?;
                        if !self.reads.reachable.contains_key(&(file, flow)) {
                            self.reads.reachable.reserve(1);
                        }
                        self.reads.reachable.insert((file, flow), reachable.clone());
                        invocations.finish_with(frame, Ok(reachable))?;
                    }
                    Work::Branch {
                        frame,
                        antecedent,
                        flow,
                        index,
                    } => {
                        if invocations.read_result(frame, antecedent)? {
                            invocations.finish(frame, true)?;
                            continue;
                        }
                        let antecedents = branch_antecedents(graph, flow, &reductions)?;
                        if let Some(&next) = antecedents.get(index + 1) {
                            let antecedent = invocations.child(frame);
                            work.push(Work::Branch {
                                frame,
                                antecedent,
                                flow,
                                index: index + 1,
                            });
                            work.push(Work::Visit {
                                frame: antecedent,
                                flow: next,
                                no_cache_check: false,
                            });
                        } else {
                            invocations.finish(frame, false)?;
                        }
                    }
                    Work::Reduce {
                        frame,
                        antecedent,
                        reduction,
                    } => {
                        if reductions.pop() != Some(reduction) {
                            return Err(super::super::super::internal(
                                "native reachability reduction has no matching scope",
                            )
                            .into());
                        }
                        let result = invocations.read_result(frame, antecedent)?;
                        invocations.finish(frame, result)?;
                    }
                    Work::Visit {
                        frame,
                        flow,
                        mut no_cache_check,
                    } => {
                        if let Some((last_file, last_flow, reachable)) =
                            self.reads.last_reachable.clone()
                            && last_file == file
                            && last_flow == flow
                        {
                            invocations.finish_with(frame, Ok(reachable))?;
                            continue;
                        }
                        if graph.shared(flow) {
                            if !no_cache_check {
                                if let Some(reachable) =
                                    self.reads.reachable.get(&(file, flow)).cloned()
                                {
                                    invocations.finish_with(frame, Ok(reachable))?;
                                } else {
                                    let antecedent = invocations.child(frame);
                                    work.push(Work::Cache {
                                        frame,
                                        antecedent,
                                        flow,
                                    });
                                    work.push(Work::Visit {
                                        frame: antecedent,
                                        flow,
                                        no_cache_check: true,
                                    });
                                }
                                continue;
                            }
                            no_cache_check = false;
                        }
                        match graph.node(flow) {
                            Node::Unreachable => invocations.finish(frame, false)?,
                            Node::Start { .. } => invocations.finish(frame, true)?,
                            Node::Assignment { antecedent, .. }
                            | Node::Condition { antecedent, .. }
                            | Node::ArrayMutation { antecedent, .. } => {
                                work.push(Work::Visit {
                                    frame,
                                    flow: *antecedent,
                                    no_cache_check,
                                });
                            }
                            Node::Label {
                                looping: true,
                                antecedents,
                            } => {
                                if let Some(&entry) = antecedents.first() {
                                    work.push(Work::Visit {
                                        frame,
                                        flow: entry,
                                        no_cache_check,
                                    });
                                } else {
                                    invocations.finish(frame, false)?;
                                }
                            }
                            Node::Label { looping: false, .. } => {
                                if let Some(&first) =
                                    branch_antecedents(graph, flow, &reductions)?.first()
                                {
                                    let antecedent = invocations.child(frame);
                                    work.push(Work::Branch {
                                        frame,
                                        antecedent,
                                        flow,
                                        index: 0,
                                    });
                                    work.push(Work::Visit {
                                        frame: antecedent,
                                        flow: first,
                                        no_cache_check: false,
                                    });
                                } else {
                                    invocations.finish(frame, false)?;
                                }
                            }
                            Node::Call {
                                expression,
                                antecedent,
                            } => {
                                if let Some(signature) = invocations.read(
                                    frame,
                                    self.effects_signature_completion(file, *expression),
                                )? {
                                    if let Some(predicate) = invocations.read(
                                        frame,
                                        self.type_predicate_of_signature_completion(
                                            &signature, location,
                                        ),
                                    )? && predicate.kind == TypePredicateKind::Asserts
                                        && let TypePredicateSubject::ParameterIndex(index) =
                                            predicate.subject
                                        && let Some(argument) =
                                            self.source.call_argument(file, *expression, index)?
                                        && self.source.false_expression(file, argument)?
                                    {
                                        invocations.finish(frame, false)?;
                                        continue;
                                    }
                                    let call_location = super::super::super::location(
                                        self.inputs,
                                        file,
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
                                        invocations.finish(frame, false)?;
                                        continue;
                                    }
                                }
                                work.push(Work::Visit {
                                    frame,
                                    flow: *antecedent,
                                    no_cache_check,
                                });
                            }
                            Node::ReduceLabel { antecedent, .. } => {
                                self.reads.last_reachable = None;
                                reductions.push(flow);
                                let child = invocations.child(frame);
                                work.push(Work::Reduce {
                                    frame,
                                    antecedent: child,
                                    reduction: flow,
                                });
                                work.push(Work::Visit {
                                    frame: child,
                                    flow: *antecedent,
                                    no_cache_check: false,
                                });
                            }
                            Node::Pending { site, message } => {
                                let cause =
                                    super::super::super::location(self.inputs, file, site.span);
                                return Err(self.flow_required(&cause, message));
                            }
                            Node::SwitchClause {
                                statement,
                                start,
                                end,
                                antecedent,
                            } => {
                                if start == end
                                    && invocations.read(
                                        frame,
                                        self.exhaustive_switch_completion(
                                            file, *statement, location,
                                        ),
                                    )?
                                {
                                    invocations.finish(frame, false)?;
                                } else {
                                    work.push(Work::Visit {
                                        frame,
                                        flow: *antecedent,
                                        no_cache_check,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            invocations.result(0)
        })();
        let reachable = match execution {
            Ok(reachable) => reachable,
            Err(failure) => return Err(invocations.fail(active_frame, failure)),
        };
        self.reads.last_reachable = Some((file, root, reachable.clone()));
        Ok(reachable)
    }
}
