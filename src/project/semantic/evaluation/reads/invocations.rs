//! Native flow operation frames and the exact completions owned by their slots.
use super::FlowId;
use crate::project::semantic::model::{
    DemandError, NativeCompletion, NativeResult, NativeSequence, TypeId,
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct FlowType {
    pub(super) ty: TypeId,
    pub(super) incomplete: bool,
}
impl FlowType {
    pub(super) fn complete(ty: TypeId) -> Self {
        Self {
            ty,
            incomplete: false,
        }
    }
}

pub(super) struct ReadInvocation {
    pub(super) depth: usize,
    pub(super) entered: bool,
    parent: Option<usize>,
    last_shared: Option<FlowId>,
    before_shared: Option<NativeCompletion<()>>,
    progress: Option<NativeSequence>,
    result: Option<NativeCompletion<FlowType>>,
}
impl ReadInvocation {
    fn new(depth: usize, parent: Option<usize>, entered: bool) -> Self {
        Self {
            depth,
            parent,
            entered,
            last_shared: None,
            before_shared: Some(NativeCompletion::clean(())),
            progress: Some(NativeSequence::new()),
            result: None,
        }
    }

    fn finish_progress<T>(&mut self, result: Result<T, DemandError>) -> NativeResult<T> {
        let progress = self.progress.take().ok_or_else(|| {
            super::super::super::internal("native flow operation completed twice")
        })?;
        let suffix = progress.finish(result);
        let prefix = self.before_shared.take().ok_or_else(|| {
            super::super::super::internal("native flow operation lost its prefix")
        })?;
        prefix.and_then(|()| suffix)
    }
}

/// Frames, Shared results and reduce labels of one reference question.
pub(super) struct ReadInvocations {
    pub(super) frames: Vec<ReadInvocation>,
    pub(super) reductions: Vec<FlowId>,

    // `sharedFlowStart`: entries belong to this reference question only.
    shared: HashMap<FlowId, NativeCompletion<FlowType>>,
    pub(super) admitted_literal_base: Option<TypeId>,
    pub(super) reference_key: Option<Option<super::loops::FlowCacheKey>>,
}
impl ReadInvocations {
    pub(super) fn new() -> Self {
        let frames = Vec::from([ReadInvocation::new(0, None, false)]);
        Self {
            frames,
            reductions: Vec::new(),

            shared: HashMap::new(),
            admitted_literal_base: None,
            reference_key: None,
        }
    }

    /// A recursive `getTypeAtFlowNode` adds depth; tail traversal does not.
    pub(super) fn child(&mut self, parent: usize) -> usize {
        let frame = self.frames.len();
        let invocation = ReadInvocation::new(self.frames[parent].depth + 1, Some(parent), false);
        self.push_frame(invocation);
        frame
    }

    fn push_frame(&mut self, invocation: ReadInvocation) {
        self.frames.push(invocation);
    }

    /// `getTypeAtFlowNode`'s ReduceLabel scope, popped by its continuation.
    pub(super) fn push_reduction(&mut self, flow: FlowId) {
        self.reductions.push(flow);
    }

    /// `getTypeAtFlowLoopLabel` is an actual operation with its own cache
    /// publication point, but is not another getTypeAtFlowNode invocation.
    pub(super) fn loop_operation(&mut self, parent: usize) -> usize {
        let frame = self.frames.len();
        let invocation = ReadInvocation::new(self.frames[parent].depth, Some(parent), true);
        self.push_frame(invocation);
        frame
    }

    pub(super) fn read<T>(
        &mut self,
        frame: usize,
        next: NativeResult<T>,
    ) -> Result<T, DemandError> {
        self.frames[frame]
            .progress
            .as_mut()
            .ok_or_else(|| {
                super::super::super::internal(
                    "completed native flow operation acquired another dependency",
                )
            })?
            .read(next)
    }

    pub(super) fn result(&self, frame: usize) -> Result<NativeCompletion<FlowType>, DemandError> {
        self.frames[frame].result.clone().ok_or_else(|| {
            super::super::super::internal("native flow operation did not finish").into()
        })
    }

    pub(super) fn read_result(
        &mut self,
        parent: usize,
        child: usize,
    ) -> Result<FlowType, DemandError> {
        self.read(parent, self.result(child))
    }

    /// `getTypeAtFlowNode` checks Shared before the node worker. A hit returns
    /// without publishing an earlier tail candidate. On a miss, only the new
    /// suffix will belong to this selected Shared slot.
    pub(super) fn visit_shared(&mut self, frame: usize, flow: FlowId) -> Result<bool, DemandError> {
        if let Some(completion) = self.shared.get(&flow).cloned() {
            let ty = self.read(frame, Ok(completion))?;
            let completion = self.frames[frame].finish_progress(Ok(ty))?;
            self.frames[frame].result = Some(completion);
            return Ok(true);
        }
        let invocation = &mut self.frames[frame];
        let prefix = invocation.finish_progress(Ok(()))?;
        invocation.before_shared = Some(prefix);
        invocation.progress = Some(NativeSequence::new());
        invocation.last_shared = Some(flow);
        Ok(false)
    }

    /// Publish the native result and only the selected Shared suffix. The
    /// returned invocation completion also retains its earlier tail prefix.
    pub(super) fn finish(&mut self, frame: usize, ty: FlowType) -> Result<(), DemandError> {
        let invocation = &mut self.frames[frame];
        let progress = invocation.progress.take().ok_or_else(|| {
            super::super::super::internal("native flow operation completed twice")
        })?;
        let suffix = progress.finish(Ok(ty));
        let prefix = invocation.before_shared.take().ok_or_else(|| {
            super::super::super::internal("native flow operation lost its prefix")
        })?;
        let suffix = match suffix {
            Ok(suffix) => suffix,
            Err(error) => return prefix.and_then_value::<()>(|()| Err(error)).map(|_| ()),
        };
        if let Some(flow) = invocation.last_shared
            && !self.shared.contains_key(&flow)
        {
            self.shared.insert(flow, suffix.clone());
        }
        invocation.result = Some(prefix.and_then(|()| Ok(suffix))?);
        Ok(())
    }

    pub(super) fn finish_with(
        &mut self,
        frame: usize,
        completion: NativeResult<FlowType>,
    ) -> Result<(), DemandError> {
        let ty = self.read(frame, completion)?;
        self.finish(frame, ty)
    }

    /// On interruption, unfinished native calls unwind in actual nesting
    /// order. A failed `read` already owns its suffix; NativeSequence prevents
    /// duplicate composition. Completed slots remain installed and untouched.
    pub(super) fn fail(&mut self, mut frame: usize, mut failure: DemandError) -> DemandError {
        loop {
            let invocation = &mut self.frames[frame];
            if invocation.progress.is_some() {
                failure = match invocation.finish_progress::<()>(Err(failure)) {
                    Err(failure) => failure,
                    Ok(_) => unreachable!("an interrupted flow cannot complete"),
                };
            }
            match invocation.parent {
                Some(parent) => frame = parent,
                None => return failure,
            }
        }
    }
}
