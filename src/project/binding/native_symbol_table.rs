//! One exact table observation policy serves binder and checker symbol values.

use super::{NativePropertyKey, TypeBindingFailure};
use crate::hash::FastMap;

/// Optional owner fields retain nil; this value is always an allocated table.
/// Copying it is not a semantic inventory read. Failed keys and unfinished
/// binder publication survive a copy without becoming an absent name.
#[derive(Debug, Clone)]
pub(in crate::project) struct NativeSymbolTable<S> {
    entries: Stored<S>,
    pending_names: usize,
    unavailable: Option<Box<TypeBindingFailure>>,
}

/// Most tables hold a few entries (a project has hundreds of thousands of
/// them, most with one to four): those are a short list, searched in order,
/// and only a larger table is a map. Either way a key appears once.
#[derive(Debug, Clone)]
enum Stored<S> {
    Few(Vec<(NativePropertyKey, S)>),
    Many(Box<FastMap<NativePropertyKey, S>>),
}

/// The most entries a table keeps as a list.
const FEW: usize = 8;

impl<S> Default for NativeSymbolTable<S> {
    fn default() -> Self {
        Self {
            entries: Stored::Few(Vec::new()),
            pending_names: 0,
            unavailable: None,
        }
    }
}

impl<S> Stored<S> {
    fn get(&self, key: &NativePropertyKey) -> Option<&S> {
        match self {
            Stored::Few(entries) => entries
                .iter()
                .find(|(present, _)| present == key)
                .map(|(_, symbol)| symbol),
            Stored::Many(entries) => entries.get(key),
        }
    }

    /// Add or replace `key`'s entry, as a map's insert does.
    fn insert(&mut self, key: NativePropertyKey, symbol: S) {
        match self {
            Stored::Few(entries) => {
                if let Some(entry) = entries.iter_mut().find(|(present, _)| *present == key) {
                    entry.1 = symbol;
                } else if entries.len() < FEW {
                    entries.push((key, symbol));
                } else {
                    let mut many: FastMap<_, _> = std::mem::take(entries).into_iter().collect();
                    many.insert(key, symbol);
                    *self = Stored::Many(Box::new(many));
                }
            }
            Stored::Many(entries) => {
                entries.insert(key, symbol);
            }
        }
    }
}

/// A table's entries. Their order is no answer: TypeScript's SymbolTable is
/// a Go map, whose iteration order is unspecified.
#[derive(Debug, Clone, Copy)]
pub(in crate::project) struct Entries<'a, S>(&'a Stored<S>);

impl<'a, S> Entries<'a, S> {
    pub(in crate::project) fn iter(self) -> EntriesIter<'a, S> {
        match self.0 {
            Stored::Few(entries) => EntriesIter::Few(entries.iter()),
            Stored::Many(entries) => EntriesIter::Many(entries.iter()),
        }
    }

    pub(in crate::project) fn values(self) -> impl Iterator<Item = &'a S> {
        self.iter().map(|(_, symbol)| symbol)
    }

    #[cfg(test)]
    pub(in crate::project) fn is_empty(self) -> bool {
        match self.0 {
            Stored::Few(entries) => entries.is_empty(),
            Stored::Many(entries) => entries.is_empty(),
        }
    }
}

impl<'a, S> IntoIterator for Entries<'a, S> {
    type Item = (&'a NativePropertyKey, &'a S);
    type IntoIter = EntriesIter<'a, S>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub(in crate::project) enum EntriesIter<'a, S> {
    Few(std::slice::Iter<'a, (NativePropertyKey, S)>),
    Many(std::collections::hash_map::Iter<'a, NativePropertyKey, S>),
}

impl<'a, S> Iterator for EntriesIter<'a, S> {
    type Item = (&'a NativePropertyKey, &'a S);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EntriesIter::Few(entries) => entries.next().map(|(key, symbol)| (key, symbol)),
            EntriesIter::Many(entries) => entries.next(),
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
            return Err((**failure).clone());
        }
        Ok(None)
    }

    /// Native table iteration consumes all keys, unlike one keyed lookup.
    pub(in crate::project) fn entries(&self) -> Result<Entries<'_, S>, TypeBindingFailure> {
        self.require_names()?;
        if let Some(failure) = &self.unavailable {
            return Err((**failure).clone());
        }
        Ok(Entries(&self.entries))
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
            Ok(key) => self.entries.insert(key, symbol),
            Err(failure) => {
                self.unavailable.get_or_insert_with(|| Box::new(failure));
            }
        }
    }

    /// A published table never grows: release the capacity its growth left.
    pub(in crate::project::binding) fn freeze(&mut self) {
        match &mut self.entries {
            Stored::Few(entries) => entries.shrink_to_fit(),
            Stored::Many(entries) => entries.shrink_to_fit(),
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
        let entries = match &self.entries {
            Stored::Few(entries) => Stored::Few(
                entries
                    .iter()
                    .map(|(key, value)| (key.clone(), symbol(*value)))
                    .collect(),
            ),
            Stored::Many(entries) => Stored::Many(Box::new(
                entries
                    .iter()
                    .map(|(key, value)| (key.clone(), symbol(*value)))
                    .collect(),
            )),
        };
        Ok(NativeSymbolTable {
            entries,
            pending_names: 0,
            unavailable: self.unavailable.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(index: usize) -> NativePropertyKey {
        NativePropertyKey::fixed(format!("k{index}").into_bytes().as_slice(), None)
    }

    /// A table answers as a map does at every size, across the change from
    /// a list to a map: each key once, the latest symbol for it.
    #[test]
    fn tables_answer_as_maps_at_every_size() {
        for size in [0, 1, FEW - 1, FEW, FEW + 1, 3 * FEW] {
            let mut table = NativeSymbolTable::<usize>::default();
            for index in 0..size {
                table.insert(Ok(key(index)), index);
            }
            // Replacing every entry keeps one entry per key.
            for index in 0..size {
                table.insert(Ok(key(index)), index + 100);
            }
            table.freeze();
            for index in 0..size {
                assert_eq!(
                    table.lookup(&key(index)).unwrap(),
                    Some(index + 100),
                    "{size}"
                );
            }
            assert_eq!(table.lookup(&key(size)).unwrap(), None, "{size}");
            let mut entries: Vec<(NativePropertyKey, usize)> = table
                .entries()
                .unwrap()
                .iter()
                .map(|(key, &value)| (key.clone(), value))
                .collect();
            entries.sort_by_key(|&(_, value)| value);
            let expected: Vec<_> = (0..size).map(|index| (key(index), index + 100)).collect();
            assert_eq!(entries, expected, "{size}");
            assert_eq!(table.entries().unwrap().is_empty(), size == 0);
            let copy = table.checker_copy(|value| value * 2).unwrap();
            assert_eq!(copy.lookup(&key(0)).unwrap(), (size > 0).then_some(200));
        }
    }

    /// Hundreds of thousands of tables: each stays a few words.
    #[test]
    fn tables_stay_compact() {
        let size =
            std::mem::size_of::<NativeSymbolTable<crate::project::binding::ids::NativeHeaderId>>();
        assert!(size <= 40, "{size}");
    }

    /// A failed key is observed after known entries, at every size.
    #[test]
    fn a_failed_key_follows_known_entries() {
        for size in [1, FEW + 1] {
            let mut table = NativeSymbolTable::<usize>::default();
            for index in 0..size {
                table.insert(Ok(key(index)), index);
            }
            table.insert(Err(TypeBindingFailure::Pending("failed key")), 0);
            assert_eq!(table.lookup(&key(0)).unwrap(), Some(0));
            assert!(table.lookup(&key(size)).is_err());
            assert!(table.entries().is_err());
        }
    }
}
