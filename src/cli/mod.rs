//! Command-line requests: the vocabulary of operations and flags
//! (`registry`) and the help derived from it (`help`), one parsed-request
//! boundary (`request`), the execution of queries in file, direct, and
//! service entry (`query`), one delivery policy for their outcomes
//! (`delivery`), the typed failure of a command that leaves none
//! (`failure`), navigation's text and JSON forms (`navigation`), a
//! reference answer's (`references`), editing commands (`editing`), the
//! report of what project queries depend on (`doctor`), and printed
//! follow-up commands (`command_line`).

pub(crate) mod command_line;
pub(crate) mod constructs;
pub(crate) mod delivery;
pub(crate) mod doctor;
pub(crate) mod editing;
pub(crate) mod failure;
pub(crate) mod help;
pub(crate) mod navigation;
pub(crate) mod query;
pub(crate) mod references;
pub(crate) mod registry;
pub(crate) mod request;
pub(crate) mod types;

pub(crate) use failure::{Failure, Issue};

/// This program's identity, as `--version` and the doctor print it: its
/// package version and the complete build stamp `build.rs` compiles in,
/// which keys the project's service and which a plan records. The stamp is
/// identity text, so it is never shortened (decisions.md, step 5).
pub(crate) fn version() -> String {
    format!(
        "tsparse {} (build {})",
        env!("CARGO_PKG_VERSION"),
        env!("TSPARSE_SERVICE_BUILD")
    )
}
