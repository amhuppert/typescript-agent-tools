//! File batch application: check every target, stage every replacement,
//! check again, install in a deterministic order, and on an ordinary
//! failure restore what was installed. This module knows which files are
//! staged, installed, or restored, and derives the batch's filesystem
//! effect from their outcomes; each file's resources and identity checks
//! are `file::Replacement`'s. It knows nothing of TypeScript.

use super::file::{Mismatch, Replacement, Unstaged};
use super::{
    Applied, ApplyFailure, ApplyFailureKind, Effect, FileOutcome, FileState, Leftover,
    PreparedFile, Step,
};
use std::io;

/// Apply the changing files of a batch, calling `hook` before each step it
/// names, whose error stands for that step's own failure.
pub(super) fn apply(
    files: &[PreparedFile],
    hook: &mut dyn FnMut(Step) -> io::Result<()>,
) -> Applied {
    let mut order: Vec<&PreparedFile> = files.iter().filter(|file| file.changes()).collect();
    order.sort_by(|a, b| a.captured.referent.cmp(&b.captured.referent));
    let mut outcome = Outcome {
        files,
        states: vec![FileState::Unchanged; files.len()],
        leftovers: Vec::new(),
    };
    if order.is_empty() {
        return outcome.finish(None);
    }
    for file in &order {
        if let Err(mismatch) = file.captured.current() {
            return outcome.finish(Some(failure(file, mismatch)));
        }
    }
    let mut staged: Vec<Replacement<'_>> = Vec::with_capacity(order.len());
    for (index, file) in order.iter().enumerate() {
        let result = match hook(Step::Stage(index)) {
            Ok(()) => Replacement::stage(&file.captured, &file.proposed, &mut || {
                hook(Step::Link(index))
            }),
            Err(error) => Err(Unstaged {
                mismatch: Mismatch::Io(error),
                partial: None,
            }),
        };
        match result {
            Ok(replacement) => staged.push(replacement),
            Err(unstaged) => {
                staged.extend(unstaged.partial);
                outcome.release(staged, hook);
                return outcome.finish(Some(failure(file, unstaged.mismatch)));
            }
        }
    }
    // Staging took time; the first check does not cover it.
    for (replacement, file) in staged.iter().zip(&order) {
        if let Err(mismatch) = replacement.recheck() {
            let failure = failure(file, mismatch);
            outcome.release(staged, hook);
            return outcome.finish(Some(failure));
        }
    }
    for index in 0..staged.len() {
        if let Err(error) = hook(Step::Install(index)).and_then(|()| staged[index].install()) {
            let failure = failure(order[index], Mismatch::Io(error));
            for restored in (0..index).rev() {
                let position = outcome.position(order[restored]);
                outcome.states[position] =
                    match staged[restored].restore(hook(Step::Restore(restored))) {
                        Ok(()) => FileState::Restored,
                        Err(unrestored) => FileState::RecoveryRequired {
                            observed: unrestored.observed,
                            original: unrestored.original,
                        },
                    };
            }
            outcome.release(staged, hook);
            return outcome.finish(Some(failure));
        }
        let position = outcome.position(order[index]);
        outcome.states[position] = FileState::Replaced;
    }
    outcome.release(staged, hook);
    outcome.finish(None)
}

/// The failure a file's mismatch or I/O error reports.
fn failure(file: &PreparedFile, mismatch: Mismatch) -> ApplyFailure {
    let (kind, message) = match mismatch {
        Mismatch::Stale(reason) => (
            ApplyFailureKind::Stale,
            format!("the file changed since the batch was prepared: {reason}; prepare it again"),
        ),
        Mismatch::Io(error) => (ApplyFailureKind::Io, error.to_string()),
    };
    ApplyFailure {
        kind,
        message,
        path: file.captured.path.clone(),
    }
}

/// The per-file outcomes of one application, in batch order.
struct Outcome<'b> {
    files: &'b [PreparedFile],
    states: Vec<FileState>,
    leftovers: Vec<Leftover>,
}

impl Outcome<'_> {
    fn position(&self, file: &PreparedFile) -> usize {
        self.files
            .iter()
            .position(|candidate| std::ptr::eq(candidate, file))
            .expect("a file of this batch")
    }

    /// Release every replacement's remaining resources, recording what
    /// could not be removed.
    fn release(
        &mut self,
        staged: Vec<Replacement<'_>>,
        hook: &mut dyn FnMut(Step) -> io::Result<()>,
    ) {
        for (index, replacement) in staged.into_iter().enumerate() {
            let fault = hook(Step::Cleanup(index));
            self.leftovers
                .extend(
                    replacement
                        .release(fault)
                        .into_iter()
                        .map(|leftover| Leftover {
                            path: leftover.path,
                            message: leftover.error.to_string(),
                        }),
                );
        }
    }

    /// The outcome of the batch: its effect derived from the files' states
    /// and the failure, as the design's per-file outcomes decide it.
    fn finish(self, failure: Option<ApplyFailure>) -> Applied {
        let any = |wanted: fn(&FileState) -> bool| self.states.iter().any(wanted);
        let effect = if any(|state| matches!(state, FileState::RecoveryRequired { .. })) {
            Effect::RecoveryRequired
        } else if any(|state| *state == FileState::Restored) {
            Effect::RolledBack
        } else if any(|state| *state == FileState::Replaced) {
            Effect::Applied
        } else if failure.is_some() {
            Effect::Unchanged
        } else {
            Effect::NoChange
        };
        Applied {
            effect,
            failure,
            files: self
                .files
                .iter()
                .zip(self.states)
                .map(|(file, state)| FileOutcome {
                    path: file.captured.path.clone(),
                    display: file.proposal.display.clone(),
                    state,
                })
                .collect(),
            leftovers: self.leftovers,
        }
    }
}
