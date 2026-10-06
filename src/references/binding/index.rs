//! The frozen semantic index served to queries.

use super::ids::{OccurrenceId, StringId, SymbolId};
use crate::Span;
use crate::references::source::FileId;
use smallvec::SmallVec;
use std::collections::HashMap;

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
    Unresolved(StringId),
}

/// An identifier token in a searched file.
#[derive(Debug)]
pub(crate) struct Occurrence {
    pub(crate) span: Span,
    /// The decoded name, as an `Index::text` string.
    pub(crate) name: StringId,
    /// `collect` role bits.
    pub(crate) roles: u8,
    pub(crate) target: Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OccurrenceRef {
    pub(crate) file: FileId,
    pub(crate) id: OccurrenceId,
}

#[derive(Debug, Default)]
pub(crate) struct Index {
    sites: HashMap<SymbolId, Vec<Site>>,
    /// Occurrences of each file, indexed by `FileId` and ordered by start.
    files: Vec<Vec<Occurrence>>,
    by_symbol: HashMap<SymbolId, Vec<OccurrenceRef>>,
    by_name: HashMap<String, Vec<OccurrenceRef>>,
    /// Names and reasons shared by many occurrences.
    strings: Vec<Box<str>>,
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
        &self.files[reference.file.index()][reference.id.index()]
    }

    /// Declaration, local import, and exported alias names spelled `name`.
    pub(crate) fn candidates(&self, name: &str) -> &[OccurrenceRef] {
        self.by_name.get(name).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn occurrences_of(&self, symbol: SymbolId) -> &[OccurrenceRef] {
        self.by_symbol.get(&symbol).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn text(&self, id: StringId) -> &str {
        &self.strings[id.index()]
    }

    pub(crate) fn sites(&self, symbol: SymbolId) -> &[Site] {
        self.sites.get(&symbol).map_or(&[], Vec::as_slice)
    }
}

/// An identifier token to index.
pub(crate) struct Token<'a> {
    pub(crate) span: Span,
    pub(crate) name: &'a str,
    /// `collect` role bits.
    pub(crate) roles: u8,
    /// A declaration, local import, or exported alias name, which name
    /// queries select.
    pub(crate) candidate: bool,
    pub(crate) target: Target,
}

/// Builds an `Index` one file at a time, in `FileId` order.
#[derive(Debug, Default)]
pub(crate) struct Builder {
    index: Index,
    string_ids: HashMap<Box<str>, StringId>,
}

impl Builder {
    /// Store `text` once, as an `Index::text` string.
    pub(crate) fn intern(&mut self, text: &str) -> StringId {
        if let Some(&id) = self.string_ids.get(text) {
            return id;
        }
        let id = StringId::new(self.index.strings.len());
        self.index.strings.push(text.into());
        self.string_ids.insert(text.into(), id);
        id
    }

    /// Add the next file's tokens, in any order. A file that is not searched
    /// adds none.
    pub(crate) fn add_file(&mut self, file: FileId, mut tokens: Vec<Token<'_>>) {
        assert_eq!(
            file.index(),
            self.index.files.len(),
            "files are added in order"
        );
        tokens.sort_by_key(|token| token.span.start);
        let mut occurrences = Vec::with_capacity(tokens.len());
        for (position, token) in tokens.into_iter().enumerate() {
            let reference = OccurrenceRef {
                file,
                id: OccurrenceId::new(position),
            };
            if let Target::Symbols(symbols) = &token.target {
                for &symbol in symbols {
                    self.index
                        .by_symbol
                        .entry(symbol)
                        .or_default()
                        .push(reference);
                }
            }
            if token.candidate {
                self.index
                    .by_name
                    .entry(token.name.to_string())
                    .or_default()
                    .push(reference);
            }
            occurrences.push(Occurrence {
                span: token.span,
                name: self.intern(token.name),
                roles: token.roles,
                target: token.target,
            });
        }
        self.index.files.push(occurrences);
    }

    /// Freeze the index with the declarations of every symbol that an
    /// occurrence denotes.
    pub(crate) fn finish(mut self, sites: impl Fn(SymbolId) -> Vec<Site>) -> Index {
        self.index.sites = self
            .index
            .by_symbol
            .keys()
            .map(|&symbol| (symbol, sites(symbol)))
            .collect();
        self.index
    }
}
