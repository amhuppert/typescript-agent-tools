//! One exact table observation policy serves binder and checker symbol values.

use super::{NativePropertyKey, TypeBindingFailure};
use std::collections::HashMap;

/// Optional owner fields retain nil; this value is always an allocated table.
/// Copying it is not a semantic inventory read. Failed keys and unfinished
/// binder publication survive a copy without becoming an absent name.
#[derive(Debug, Clone)]
pub(in crate::project) struct NativeSymbolTable<S> {
    entries: HashMap<NativePropertyKey, S>,
    pending_names: usize,
    unavailable: Option<TypeBindingFailure>,
}

impl<S> Default for NativeSymbolTable<S> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            pending_names: 0,
            unavailable: None,
        }
    }
}

impl<S: Copy> NativeSymbolTable<S> {
    /// `SymbolTable[name]` observes a known entry before a retained failed key.
    /// Unpublished binder names still prevent every lookup.
    pub(in crate::project) fn lookup(
        &self,
        key: &NativePropertyKey,
    ) -> Result<Option<S>, TypeBindingFailure> {
        self.require_names()?;
        if let Some(&symbol) = self.entries.get(key) {
            return Ok(Some(symbol));
        }
        if let Some(failure) = &self.unavailable {
            return Err(failure.clone());
        }
        Ok(None)
    }

    /// Native table iteration consumes all keys, unlike one keyed lookup.
    pub(in crate::project) fn entries(
        &self,
    ) -> Result<&HashMap<NativePropertyKey, S>, TypeBindingFailure> {
        self.require_names()?;
        if let Some(failure) = &self.unavailable {
            return Err(failure.clone());
        }
        Ok(&self.entries)
    }

    /// Binder placement and checker `mergeSymbolTable` publish one entry at a
    /// time into the already-installed table. Reentrant lookup sees completed
    /// prior entries even while a later merge operation is still running.
    pub(in crate::project) fn insert(
        &mut self,
        key: Result<NativePropertyKey, TypeBindingFailure>,
        symbol: S,
    ) {
        match key {
            Ok(key) => {
                self.entries.insert(key, symbol);
            }
            Err(failure) => {
                self.unavailable.get_or_insert(failure);
            }
        }
    }

    pub(in crate::project::binding) fn hold_name(&mut self) {
        self.pending_names += 1;
    }
    pub(in crate::project::binding) fn publish_name(&mut self) {
        self.pending_names -= 1;
    }

    /// `declareSymbol` checks an existing known key while incomplete identity
    /// observations remain attached to the table for later checker reads.
    pub(in crate::project::binding) fn declaration_entry(
        &self,
        key: &NativePropertyKey,
    ) -> Result<Option<S>, TypeBindingFailure> {
        self.require_names()?;
        Ok(self.entries.get(key).copied())
    }

    fn require_names(&self) -> Result<(), TypeBindingFailure> {
        if self.pending_names != 0 {
            return Err(TypeBindingFailure::Pending(
                "native binder table name publication is incomplete",
            ));
        }
        Ok(())
    }

    /// Checker `cloneSymbol` shallow-copies table storage without observing
    /// unavailable names. Name registrations belong to the original binder
    /// table and cannot complete a checker copy, so cloning still requires
    /// completed binder publication.
    pub(in crate::project) fn checker_copy<T: Copy>(
        &self,
        mut symbol: impl FnMut(S) -> T,
    ) -> Result<NativeSymbolTable<T>, TypeBindingFailure> {
        if self.pending_names != 0 {
            return Err(TypeBindingFailure::Pending(
                "checker clone requires completed binder table names",
            ));
        }
        Ok(NativeSymbolTable {
            entries: self
                .entries
                .iter()
                .map(|(key, value)| (key.clone(), symbol(*value)))
                .collect(),
            pending_names: 0,
            unavailable: self.unavailable.clone(),
        })
    }
}
