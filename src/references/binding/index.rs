//! The frozen semantic index served to queries.

use crate::Span;
use crate::references::source::FileId;
use smallvec::SmallVec;
use std::collections::HashMap;

/// A project-local symbol identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct SymbolId(pub(crate) u32);

/// One declaration of a symbol: a name token, or the anchor of a synthetic
/// declaration such as an anonymous default export or a module.
#[derive(Debug, Clone)]
pub(crate) struct Site {
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Symbols(SmallVec<[SymbolId; 2]>),
    NotReference(&'static str),
    /// The reason, as an `Index::text` string.
    Unresolved(u32),
}

/// An identifier token in a searched file.
#[derive(Debug)]
pub(crate) struct Occurrence {
    pub(crate) span: Span,
    /// The decoded name, as an `Index::text` string.
    pub(crate) name: u32,
    /// `collect` role bits.
    pub(crate) roles: u8,
    pub(crate) target: Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OccurrenceRef {
    pub(crate) file: FileId,
    pub(crate) index: u32,
}

#[derive(Debug, Default)]
pub(crate) struct Index {
    pub(crate) sites: HashMap<SymbolId, Vec<Site>>,
    /// Occurrences of each searched file, ordered by start.
    pub(crate) files: Vec<Vec<Occurrence>>,
    pub(crate) by_symbol: HashMap<SymbolId, Vec<OccurrenceRef>>,
    pub(crate) by_name: HashMap<String, Vec<OccurrenceRef>>,
    /// Names and reasons shared by many occurrences.
    pub(crate) strings: Vec<Box<str>>,
}

/// Builds `Index::strings`, storing each distinct text once.
#[derive(Debug, Default)]
pub(crate) struct Strings {
    texts: Vec<Box<str>>,
    ids: HashMap<Box<str>, u32>,
}

impl Strings {
    pub(crate) fn intern(&mut self, text: &str) -> u32 {
        if let Some(&id) = self.ids.get(text) {
            return id;
        }
        let id = self.texts.len() as u32;
        self.texts.push(text.into());
        self.ids.insert(text.into(), id);
        id
    }

    pub(crate) fn finish(self) -> Vec<Box<str>> {
        self.texts
    }
}

impl Index {
    /// The identifier token containing `offset`, start inclusive.
    pub(crate) fn occurrence_at(&self, file: FileId, offset: usize) -> Option<&Occurrence> {
        let occurrences = self.files.get(file.index())?;
        let index = occurrences.partition_point(|occurrence| occurrence.span.start <= offset);
        let occurrence = occurrences.get(index.checked_sub(1)?)?;
        (offset < occurrence.span.end).then_some(occurrence)
    }

    pub(crate) fn occurrence(&self, reference: OccurrenceRef) -> &Occurrence {
        &self.files[reference.file.index()][reference.index as usize]
    }

    /// Declaration, local import, and exported alias names spelled `name`.
    pub(crate) fn candidates(&self, name: &str) -> &[OccurrenceRef] {
        self.by_name.get(name).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn occurrences_of(&self, symbol: SymbolId) -> &[OccurrenceRef] {
        self.by_symbol.get(&symbol).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn text(&self, id: u32) -> &str {
        &self.strings[id as usize]
    }

    pub(crate) fn sites(&self, symbol: SymbolId) -> &[Site] {
        self.sites.get(&symbol).map_or(&[], Vec::as_slice)
    }
}
