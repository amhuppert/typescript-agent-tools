//! The frozen lexical projection served to reference queries.

use super::ids::{NameId, OccurrenceId, StringId, SymbolId};
use super::reference_tokens::{
    CollectedReferenceToken, ReferenceSyntax, ReferenceToken, ReferenceTokenEntry,
};
use crate::Span;
use crate::hash::FastMap;
use crate::project::sources::{FileId, Sources};
use crate::source::SourcePosition;
use smallvec::SmallVec;

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

    /// The public `Location` order, by its file's display name rather than
    /// the file's discovery order (`DisplayOrder`).
    fn key(self, order: &DisplayOrder) -> (u32, SourcePosition, SourcePosition) {
        (order.of(self.file), self.start, self.end)
    }
}

/// Each file's place among the display names, which order public locations:
/// ordering by place is ordering by name, without comparing names. Files
/// with the same display name share a place.
struct DisplayOrder(Vec<u32>);

impl DisplayOrder {
    fn new(sources: &Sources) -> Self {
        let mut files: Vec<FileId> = sources.ids().collect();
        files.sort_by(|left, right| sources.get(*left).display.cmp(&sources.get(*right).display));
        let mut places = vec![0; files.len()];
        let mut place = 0;
        for (position, &file) in files.iter().enumerate() {
            if position > 0 && sources.get(file).display != sources.get(files[position - 1]).display
            {
                place += 1;
            }
            places[file.index()] = place;
        }
        Self(places)
    }

    fn of(&self, file: FileId) -> u32 {
        self.0[file.index()]
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
    pub(crate) call: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Symbols(SmallVec<[SymbolId; 2]>),
    /// Captured dependency token; lexical binding was not requested. Such
    /// tokens never enter lexical selectors or result indexes.
    Unsearched,
    NotReference(&'static str),
    /// The reason, as an `Index::text` string.
    Unresolved(StringId),
}

/// A captured name token, shared by semantic and lexical projections.
#[derive(Debug)]
pub(crate) struct Occurrence {
    pub(crate) span: Span,
    /// The decoded name, as an `Index::text` string.
    pub(crate) name: StringId,
    /// `collect` role bits.
    pub(crate) roles: u8,
    pub(crate) call: bool,
    pub(crate) target: Target,
    pub(super) reference: Option<ReferenceTokenData>,
}

#[derive(Debug)]
pub(super) struct ReferenceTokenData {
    pub(super) key: Result<StringId, String>,
    pub(super) syntax: ReferenceSyntax,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OccurrenceRef {
    pub(crate) file: FileId,
    pub(crate) id: OccurrenceId,
}

#[derive(Debug, Default)]
pub(crate) struct Index {
    /// Its maps are only looked up; nothing iterates them.
    sites: FastMap<SymbolId, Vec<IndexedSite>>,
    /// Occurrences of each file, indexed by `FileId` and ordered by start.
    files: Vec<Vec<Occurrence>>,
    /// Derived selectors over the same entries; full JSX names can overlap
    /// legacy identifier children without changing lexical position queries.
    lexical_positions: Vec<Vec<OccurrenceId>>,
    reference_positions: Vec<Vec<OccurrenceId>>,
    /// Public result order, with duplicate locations' roles combined. This
    /// immutable work is shared by position and name queries.
    by_symbol: FastMap<SymbolId, Vec<IndexedOccurrence>>,
    by_name: FastMap<String, Vec<OccurrenceRef>>,
    by_reference_name: FastMap<String, Box<[ReferenceToken]>>,
    /// Names and reasons shared by many occurrences.
    strings: Vec<Box<str>>,
}

impl Index {
    /// The identifier token containing `offset`, start inclusive.
    pub(crate) fn occurrence_at(&self, file: FileId, offset: usize) -> Option<&Occurrence> {
        let occurrences = self.files.get(file.index())?;
        let positions = self.lexical_positions.get(file.index())?;
        let index = positions.partition_point(|id| occurrences[id.index()].span.start <= offset);
        let occurrence = &occurrences[positions.get(index.checked_sub(1)?)?.index()];
        (offset < occurrence.span.end).then_some(occurrence)
    }

    pub(crate) fn occurrence(&self, reference: OccurrenceRef) -> &Occurrence {
        &self.files[reference.file.index()][reference.id.index()]
    }

    /// Declaration, local import, and exported alias names spelled `name`.
    pub(crate) fn candidates(&self, name: &str) -> &[OccurrenceRef] {
        self.by_name.get(name).map_or(&[], Vec::as_slice)
    }

    pub(super) fn reference_tokens(
        &self,
        name: &str,
        control: Option<&crate::resources::Control>,
    ) -> &[ReferenceToken] {
        // getPossibleSymbolReferencePositions' decoded-key accelerator. Debit
        // the exact query bytes before the indivisible standard-table lookup;
        // this neither copies the key nor changes its hash/equality policy.

        let tokens: &[ReferenceToken] = self
            .by_reference_name
            .get(name)
            .map_or(&[], |tokens| tokens);
        if let Some(control) = control {
            crate::resources::complete_or_abort(control.check());
        }
        tokens
    }

    pub(super) fn reference_token_at(&self, file: FileId, offset: usize) -> Option<ReferenceToken> {
        let occurrences = self.files.get(file.index())?;
        let positions = self.reference_positions.get(file.index())?;
        let index = positions.partition_point(|id| occurrences[id.index()].span.start <= offset);
        let &id = positions.get(index.checked_sub(1)?)?;
        (offset < occurrences[id.index()].span.end).then_some(ReferenceToken {
            file,
            entry: ReferenceTokenEntry::Syntax(id),
        })
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
    /// Its original file-local collected name, before position sorting.
    pub(crate) source: NameId,
    pub(crate) span: Span,
    pub(crate) name: &'a str,
    /// `collect` role bits.
    pub(crate) roles: u8,
    pub(crate) call: bool,
    /// A declaration, local import, or exported alias name, which name
    /// queries select.
    pub(crate) candidate: bool,
    pub(crate) target: Target,
    pub(super) lexical: bool,
    pub(super) reference: Option<&'a CollectedReferenceToken>,
}

/// Builds an `Index` one file at a time, in `FileId` order.
#[derive(Debug, Default)]
pub(crate) struct Builder {
    index: Index,
    string_ids: FastMap<Box<str>, StringId>,
    /// `finish` sorts each entry, so its order is not the hasher's.
    by_symbol: FastMap<SymbolId, Vec<OccurrenceRef>>,
    by_reference_name: FastMap<String, Vec<(ReferenceToken, Span)>>,
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

    /// Add the next captured file. Only searched files enter the lexical
    /// selectors and result indexes; all files retain reference syntax.
    pub(crate) fn add_file(
        &mut self,
        file: FileId,
        searched: bool,
        mut tokens: Vec<Token<'_>>,
    ) -> Vec<OccurrenceId> {
        assert_eq!(
            file.index(),
            self.index.files.len(),
            "files are added in order"
        );
        tokens.sort_by_key(|token| token.span.start);
        let mut names = vec![OccurrenceId::new(0); tokens.len()];
        let mut occurrences = Vec::with_capacity(tokens.len());
        let mut lexical_positions = Vec::new();
        let mut reference_positions = Vec::new();
        for (position, token) in tokens.into_iter().enumerate() {
            let reference = OccurrenceRef {
                file,
                id: OccurrenceId::new(position),
            };
            names[token.source.index()] = reference.id;
            if searched && token.lexical {
                lexical_positions.push(reference.id);
            }
            let reference_data = token.reference.map(|data| {
                reference_positions.push(reference.id);
                let key = match &data.key {
                    Ok(key) => {
                        self.by_reference_name
                            .entry(key.clone())
                            .or_default()
                            .push((
                                ReferenceToken {
                                    file,
                                    entry: ReferenceTokenEntry::Syntax(reference.id),
                                },
                                token.span,
                            ));
                        Ok(self.intern(key))
                    }
                    Err(error) => Err(error.clone()),
                };
                ReferenceTokenData {
                    key,
                    syntax: data.syntax,
                }
            });
            if searched && let Target::Symbols(symbols) = &token.target {
                for &symbol in symbols {
                    self.by_symbol.entry(symbol).or_default().push(reference);
                }
            }
            if searched && token.candidate {
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
                call: token.call,
                target: token.target,
                reference: reference_data,
            });
        }
        self.index.files.push(occurrences);
        self.index.lexical_positions.push(lexical_positions);
        self.index.reference_positions.push(reference_positions);
        names
    }

    pub(super) fn add_json(&mut self, file: FileId, json: &super::JsonFacts) {
        for key in json.reference_keys() {
            if let Some(name) = key.key.as_str() {
                self.by_reference_name
                    .entry(name.to_owned())
                    .or_default()
                    .push((
                        ReferenceToken {
                            file,
                            entry: ReferenceTokenEntry::Json(key.id),
                        },
                        key.span,
                    ));
            }
        }
    }

    /// Freeze the index with the declarations of every symbol that an
    /// occurrence denotes.
    pub(crate) fn finish(
        mut self,
        sources: &Sources,
        sites: impl Fn(SymbolId) -> Vec<Site>,
    ) -> Index {
        let order = DisplayOrder::new(sources);
        for (name, mut references) in self.by_reference_name {
            references
                .sort_by_key(|(reference, span)| (order.of(reference.file), span.start, span.end));
            // Collected at its exact length: mapping in place would keep the
            // larger pairs' allocation and the slack of their growth.
            let mut tokens = Vec::with_capacity(references.len());
            tokens.extend(references.into_iter().map(|(reference, _)| reference));
            self.index
                .by_reference_name
                .insert(name, tokens.into_boxed_slice());
        }
        for (symbol, references) in self.by_symbol {
            let mut declarations: Vec<_> = sites(symbol)
                .into_iter()
                .map(|site| IndexedSite {
                    name: site.name,
                    location: IndexedLocation::new(sources, site.file, site.span),
                })
                .collect();
            declarations.sort_by_key(|site| site.location.key(&order));
            declarations.dedup_by(|a, b| {
                a.location.key(&order) == b.location.key(&order) && a.name == b.name
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
                        call: occurrence.call,
                    }
                })
                .collect();
            // Stable sorting keeps the first token's spelling at a shared
            // location, as the former query-time map did. Every role survives.
            occurrences.sort_by_key(|occurrence| occurrence.location.key(&order));
            occurrences.dedup_by(|later, first| {
                if later.location.key(&order) != first.location.key(&order) {
                    return false;
                }
                first.roles |= later.roles;
                first.call |= later.call;
                true
            });
            self.index.by_symbol.insert(symbol, occurrences);
        }
        self.index
    }
}
