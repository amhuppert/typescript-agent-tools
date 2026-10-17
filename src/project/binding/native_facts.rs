//! Read-only current checker facts borrowed only during lexical load work.
//! No alias, type, export worker, parser, cache or current table is retained.

use super::{BoundSymbol, NativeSymbolFlags, SymbolId, TypeBindingFailure};
use crate::analysis::AnalysisError;

#[derive(Debug, Clone)]
pub(in crate::project) enum LookupFailure {
    Binding(TypeBindingFailure),
    Native(AnalysisError),
}
impl From<TypeBindingFailure> for LookupFailure {
    fn from(failure: TypeBindingFailure) -> Self {
        Self::Binding(failure)
    }
}
impl From<AnalysisError> for LookupFailure {
    fn from(failure: AnalysisError) -> Self {
        Self::Native(failure)
    }
}

pub(in crate::project) enum ScopeExportRead {
    NamedDefault {
        projection: SymbolId,
        flags: NativeSymbolFlags,
    },
    Table(bool),
    Skip,
}

/// The checker supplies completed header observations, never projected flags
/// reconstructed from lexical Slots. Host/scope selection remains in binding.
pub(in crate::project) trait NativeFacts {
    fn scope_exports(
        &self,
        symbol: BoundSymbol,
        external: bool,
        name: &str,
        meaning: u32,
    ) -> Result<ScopeExportRead, LookupFailure>;
    fn flags(&self, symbol: BoundSymbol) -> Result<NativeSymbolFlags, LookupFailure>;
}
