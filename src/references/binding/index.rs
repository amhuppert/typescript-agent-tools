//! The frozen semantic index served to queries.

use super::ids::{OccurrenceId, StringId, SymbolId};
use crate::Span;
use crate::references::SourcePosition;
use crate::references::source::{FileId, Sources};
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

/// A result location captured once, without an owned copy of its filename.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IndexedLocation {
    pub(crate) file: FileId,
    pub(crate) start: SourcePosition,
    pub(crate) end: SourcePosition,
}

impl IndexedLocation {
    fn new(sources: &Sources, file: FileId, span: Span) -> Self {
        let text = &sources.get(file).text;
        Self {
            file,
            start: text.position(span.start),
            end: text.position(span.end),
        }
    }

    /// The public `Location` order, using its display name rather than the
    /// file's discovery order. No filenames are allocated for comparisons.
    fn key(self, sources: &Sources) -> (&str, SourcePosition, SourcePosition) {
        (&sources.get(self.file).display, self.start, self.end)
    }
}

#[derive(Debug)]
pub(crate) struct IndexedSite {
    pub(crate) name: String,
    pub(crate) location: IndexedLocation,
}

#[derive(Debug)]
pub(crate) struct IndexedOccurrence {
    pub(crate) name: StringId,
    pub(crate) location: IndexedLocation,
    pub(crate) roles: u8,
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
    sites: HashMap<SymbolId, Vec<IndexedSite>>,
    /// Occurrences of each file, indexed by `FileId` and ordered by start.
    files: Vec<Vec<Occurrence>>,
    /// Public result order, with duplicate locations' roles combined. This
    /// immutable work is shared by position and name queries.
    by_symbol: HashMap<SymbolId, Vec<IndexedOccurrence>>,
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

    pub(crate) fn occurrences_of(&self, symbol: SymbolId) -> &[IndexedOccurrence] {
        self.by_symbol.get(&symbol).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn text(&self, id: StringId) -> &str {
        &self.strings[id.index()]
    }

    pub(crate) fn sites(&self, symbol: SymbolId) -> &[IndexedSite] {
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
    by_symbol: HashMap<SymbolId, Vec<OccurrenceRef>>,
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
                    self.by_symbol.entry(symbol).or_default().push(reference);
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
    pub(crate) fn finish(
        mut self,
        sources: &Sources,
        sites: impl Fn(SymbolId) -> Vec<Site>,
    ) -> Index {
        for (symbol, references) in self.by_symbol {
            let mut declarations: Vec<_> = sites(symbol)
                .into_iter()
                .map(|site| IndexedSite {
                    name: site.name,
                    location: IndexedLocation::new(sources, site.file, site.span),
                })
                .collect();
            declarations.sort_by_key(|site| site.location.key(sources));
            declarations.dedup_by(|a, b| {
                a.location.key(sources) == b.location.key(sources) && a.name == b.name
            });
            self.index.sites.insert(symbol, declarations);

            let mut occurrences: Vec<_> = references
                .into_iter()
                .map(|reference| {
                    let occurrence = self.index.occurrence(reference);
                    IndexedOccurrence {
                        name: occurrence.name,
                        location: IndexedLocation::new(sources, reference.file, occurrence.span),
                        roles: occurrence.roles,
                    }
                })
                .collect();
            // Stable sorting keeps the first token's spelling at a shared
            // location, as the former query-time map did. Every role survives.
            occurrences.sort_by_key(|occurrence| occurrence.location.key(sources));
            occurrences.dedup_by(|later, first| {
                if later.location.key(sources) != first.location.key(sources) {
                    return false;
                }
                first.roles |= later.roles;
                true
            });
            self.index.by_symbol.insert(symbol, occurrences);
        }
        self.index
    }
}
