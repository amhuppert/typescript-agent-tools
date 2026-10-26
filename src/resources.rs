//! Request-local recursion protection. Process memory and elapsed-time limits
//! belong to the CLI watchdog; native checker cycles retain their own owners.
use std::cell::Cell;
use std::hash::Hash;
use std::rc::Rc;

mod checked_map;
pub(crate) use checked_map::{CheckedKey, CheckedMap};

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
    /// Another thread no longer wants the request's answer (`Control::new`).
    Superseded,
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
    superseded: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}
#[derive(Clone)]
pub(crate) struct Control(Rc<State>);
impl Control {
    /// A request within `limits`, which stops at its next check once
    /// `superseded` is set.
    pub(crate) fn new(
        limits: Limits,
        superseded: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Self {
        Self(Rc::new(State {
            limits,
            usage: Cell::new(Usage::default()),
            stopped: Cell::new(None),
            superseded,
        }))
    }
    pub(crate) fn check(&self) -> Result<(), Stop> {
        if let Some(reason) = self.0.stopped.get() {
            return Err(reason);
        }
        if self
            .0
            .superseded
            .as_ref()
            .is_some_and(|superseded| superseded.load(std::sync::atomic::Ordering::Relaxed))
        {
            self.0.stopped.set(Some(Stop::Superseded));
            return Err(Stop::Superseded);
        }
        Ok(())
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
    state.write(bytes);
}
