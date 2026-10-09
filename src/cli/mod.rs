//! Command-line requests: one parsed-request boundary (`request`), the
//! execution of queries in file, direct, and service entry (`query`), one
//! delivery policy for their outcomes (`delivery`), navigation's text and
//! JSON forms (`navigation`), and editing commands (`editing`).

pub(crate) mod delivery;
pub(crate) mod editing;
pub(crate) mod navigation;
pub(crate) mod query;
pub(crate) mod request;

/// A failed command: its exit code and stderr diagnostic. An empty message
/// writes nothing, for outcomes already delivered as a JSON document.
pub(crate) type Exit = (u8, String);
