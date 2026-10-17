//! Request-local recursion protection. Process memory and elapsed-time limits
//! belong to the CLI watchdog; native checker cycles retain their own owners.
use std::cell::Cell;
use std::hash::Hash;
use std::rc::Rc;

mod checked_map;
pub(crate) use checked_map::{CheckedKey, CheckedMap};
pub(crate) use std::collections::HashMap as FixedMap;

struct RequestAbort(Stop);
pub(crate) fn complete_or_abort<T>(result: Result<T, Stop>) -> T {
    match result {
        Ok(value) => value,
        Err(stop) => std::panic::resume_unwind(Box::new(RequestAbort(stop))),
    }
}
pub(crate) fn request_interruption(payload: &(dyn std::any::Any + Send)) -> Option<Stop> {
    payload.downcast_ref::<RequestAbort>().map(|abort| abort.0)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    Depth,
}
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) depth: Option<usize>,
}
impl Default for Limits {
    fn default() -> Self {
        Self { depth: Some(1024) }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Usage {
    pub(crate) depth: usize,
    pub(crate) peak_depth: usize,
}
struct State {
    limits: Limits,
    usage: Cell<Usage>,
    stopped: Cell<Option<Stop>>,
}
#[derive(Clone)]
pub(crate) struct Control(Rc<State>);
impl Control {
    pub(crate) fn new(limits: Limits) -> Self {
        Self(Rc::new(State {
            limits,
            usage: Cell::new(Usage::default()),
            stopped: Cell::new(None),
        }))
    }
    pub(crate) fn check(&self) -> Result<(), Stop> {
        match self.0.stopped.get() {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }
    pub(crate) fn poll(&self) -> Result<(), Stop> {
        self.check()
    }
    pub(crate) fn enter(&self) -> Result<DepthGuard, Stop> {
        self.check()?;
        let mut usage = self.0.usage.get();
        let depth = usage
            .depth
            .checked_add(1)
            .filter(|depth| self.0.limits.depth.is_none_or(|limit| *depth <= limit));
        let Some(depth) = depth else {
            self.0.stopped.set(Some(Stop::Depth));
            return Err(Stop::Depth);
        };
        usage.depth = depth;
        usage.peak_depth = usage.peak_depth.max(depth);
        self.0.usage.set(usage);
        Ok(DepthGuard(self.clone()))
    }
    #[cfg(test)]
    pub(crate) fn usage(&self) -> Usage {
        self.0.usage.get()
    }
}
pub(crate) struct DepthGuard(Control);
impl Drop for DepthGuard {
    fn drop(&mut self) {
        let mut usage = self.0.0.usage.get();
        usage.depth -= 1;
        self.0.0.usage.set(usage);
    }
}

/// Exact bytes with a length prefix separating adjacent identity fields.
pub(crate) fn hash_bytes<H: std::hash::Hasher>(bytes: &[u8], state: &mut H) {
    bytes.len().hash(state);
    for chunk in bytes.chunks(1024) {
        state.write(chunk);
    }
}
