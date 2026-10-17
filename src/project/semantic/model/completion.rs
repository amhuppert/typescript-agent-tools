//! Native completion and product admission are different questions. A native
//! checker operation can return a real value after consuming errorType; callers
//! must use that value for subsequent native work and still reject the query.

use super::DemandError;

pub(in crate::project::semantic) type NativeResult<T> = Result<NativeCompletion<T>, DemandError>;

/// Ordered completion for one imperative native invocation. This is local
/// control flow, never evaluator/session state: every failed `read` must be
/// propagated immediately, and `finish` runs after the invocation's cleanup.
/// Both operations delegate to NativeCompletion's single sequencing policy.
pub(in crate::project::semantic) struct NativeSequence {
    completed: Option<NativeCompletion<()>>,
}

impl NativeSequence {
    pub(in crate::project::semantic) fn new() -> Self {
        Self {
            completed: Some(NativeCompletion::clean(())),
        }
    }

    pub(in crate::project::semantic) fn read<T>(
        &mut self,
        next: NativeResult<T>,
    ) -> Result<T, DemandError> {
        let prior = self.completed.take().ok_or_else(|| {
            super::super::internal("native sequence continued after interruption")
        })?;
        let NativeCompletion { value, recovery } = prior.and_then(|()| next)?;
        self.completed = Some(NativeCompletion {
            value: (),
            recovery,
        });
        Ok(value)
    }

    pub(in crate::project::semantic) fn finish<T>(
        self,
        result: Result<T, DemandError>,
    ) -> NativeResult<T> {
        match self.completed {
            Some(prior) => prior.and_then_value(|()| result),
            // `read` already composed all prior evidence into this Err. A
            // second append here would duplicate every preceding acquisition.
            None => match result {
                Err(failure) => Err(failure),
                Ok(_) => {
                    Err(super::super::internal("native sequence discarded an interruption").into())
                }
            },
        }
    }
}

/// The completed native value plus the failures consumed in producing it.
/// Only an operation that establishes a native value may construct recovery.
/// An unsupported dependency, interruption, or absent value remains outer Err;
/// this abstraction never supplies a fallback or changes native short circuits.
#[derive(Clone)]
pub(in crate::project::semantic) struct NativeCompletion<T> {
    value: T,
    recovery: Option<DemandError>,
}

impl<T> NativeCompletion<T> {
    pub(in crate::project::semantic) fn clean(value: T) -> Self {
        Self {
            value,
            recovery: None,
        }
    }

    /// Observe an installed native property without consuming it. This is for
    /// native cache-presence/identity predicates and final nil checks; native
    /// acquisitions use sequencing so their evidence accompanies the value.
    pub(in crate::project::semantic) fn value(&self) -> &T {
        &self.value
    }

    /// A nonforcing preview may expose known child values only when doing so
    /// cannot bypass recovery that their actual acquisition must consume.
    pub(in crate::project::semantic) fn clean_value(&self) -> Option<&T> {
        self.recovery.is_none().then_some(&self.value)
    }

    /// The native producer supplies its actual recovery value. Whole failures
    /// cannot be attached to a completed result, even at a recovery producer.
    pub(in crate::project::semantic) fn recovered(
        value: T,
        recovery: DemandError,
    ) -> NativeResult<T> {
        if recovery.is_whole_operation_failure() {
            return Err(recovery);
        }
        Ok(Self {
            value,
            recovery: Some(recovery),
        })
    }

    pub(in crate::project::semantic) fn map<U>(
        self,
        operation: impl FnOnce(T) -> U,
    ) -> NativeCompletion<U> {
        NativeCompletion {
            value: operation(self.value),
            recovery: self.recovery,
        }
    }

    /// Run the next native operation and retain acquisition order through both
    /// its completion and interruption paths. The outer Result prevents later
    /// callbacks after an interruption; whole failures replace local evidence.
    pub(in crate::project::semantic) fn and_then<U>(
        self,
        operation: impl FnOnce(T) -> NativeResult<U>,
    ) -> NativeResult<U> {
        match operation(self.value) {
            Ok(next) => {
                let recovery = match (self.recovery, next.recovery) {
                    (Some(first), Some(next)) => Some(first.append_failure(next)),
                    (first, next) => first.or(next),
                };
                if let Some(failure) = recovery.as_ref()
                    && failure.is_whole_operation_failure()
                {
                    return Err(recovery.expect("whole failure just observed"));
                }
                Ok(NativeCompletion {
                    value: next.value,
                    recovery,
                })
            }
            Err(next) => Err(match self.recovery {
                Some(first) => first.append_failure(next),
                None => next,
            }),
        }
    }

    /// Transitional boundary for native owners that cannot yet return a
    /// recovered completion. Their Err stays an interruption, never a value.
    pub(in crate::project::semantic) fn and_then_value<U>(
        self,
        operation: impl FnOnce(T) -> Result<U, DemandError>,
    ) -> NativeResult<U> {
        self.and_then(|value| operation(value).map(NativeCompletion::clean))
    }

    /// Observe a returned copy, leaving any cached source-free marker intact.
    pub(in crate::project::semantic) fn observe(
        mut self,
        operation: impl FnOnce(DemandError) -> DemandError,
    ) -> Self {
        self.recovery = self.recovery.map(operation);
        self
    }

    pub(in crate::project::semantic) fn admit(self) -> Result<T, DemandError> {
        match self.recovery {
            Some(failure) => Err(failure),
            None => Ok(self.value),
        }
    }
}
