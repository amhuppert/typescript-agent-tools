//! The frozen lexical projection served to reference queries.
//!
//! Occurrences are stored compactly, as many as a project has names: a
//! record of small codes per occurrence, its spans in a per-file table, and
//! the rare wide values (several symbols, reasons, unavailable keys) out of
//! line. Readers see them through `Occurrence`, a view assembled on demand.
//! Result orders are built from byte offsets; positions are computed only
//! for the locations a query returns.

use super::ids::{NameId, OccurrenceId, StringId, SymbolId};
use super::json::JsonPropertyId;
use super::reference_tokens::{
    CollectedReferenceToken, ReferenceSyntax, ReferenceToken, ReferenceTokenEntry,
    ReferenceTokenRole,
};
use crate::Span;
use crate::hash::FastMap;
use crate::project::sources::{FileId, Sources};
use crate::source::SourcePosition;
use crate::syntax_kind::SyntaxKind;
use smallvec::SmallVec;

/// One declaration of a symbol: a name token, or the anchor of a synthetic
/// declaration such as an anonymous default export or a module.
#[derive(Debug, Clone)]
pub(crate) struct Site {
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) name: String,
}

/// Each file's place among the display names, which order public locations:
/// ordering by place is ordering by name, without comparing names. Files
/// with the same display name share a place.
struct DisplayOrder {
    places: Vec<u32>,
    /// By place: whether files share it.
    shared: Vec<bool>,
}

/// Where a location falls in public `Location` order within its place.
/// Within one file, byte offsets order locations as their positions do,
/// and only equal offsets have equal positions; files that share a place
/// interleave, so their locations compare by position, as public locations
/// do. Every location of one place uses the same variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Within {
    Offsets(usize, usize),
    Positions(SourcePosition, SourcePosition),
}

impl DisplayOrder {
    fn new(sources: &Sources) -> Self {
        let mut files: Vec<FileId> = sources.ids().collect();
        files.sort_by(|left, right| sources.get(*left).display.cmp(&sources.get(*right).display));
        let mut places = vec![0; files.len()];
        let mut shared = Vec::new();
        let mut place = 0;
        for (position, &file) in files.iter().enumerate() {
            if position == 0 {
                shared.push(false);
            } else if sources.get(file).display != sources.get(files[position - 1]).display {
                place += 1;
                shared.push(false);
            } else {
                shared[place as usize] = true;
            }
            places[file.index()] = place;
        }
        Self { places, shared }
    }

    fn of(&self, file: FileId) -> u32 {
        self.places[file.index()]
    }

    /// The public `Location` order of `span` in `file`, as a key.
    fn key(&self, sources: &Sources, file: FileId, span: Span) -> (u32, Within) {
        let place = self.of(file);
        let within = if self.shared[place as usize] {
            let text = &sources.get(file).text;
            Within::Positions(text.position(span.start), text.position(span.end))
        } else {
            Within::Offsets(span.start, span.end)
        };
        (place, within)
    }
}

#[derive(Debug)]
pub(crate) struct IndexedSite {
    pub(crate) name: String,
    pub(crate) file: FileId,
    pub(crate) span: Span,
}

/// One public occurrence of a symbol: the first token at its location, whose
/// spelling it shows, with the roles of every token there.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IndexedOccurrence {
    pub(crate) occurrence: OccurrenceRef,
    pub(crate) roles: u8,
    pub(crate) call: bool,
}

/// What a token denotes, as linking hands it to the builder.
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

/// What an occurrence denotes, as a query reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OccurrenceTarget<'a> {
    Symbols(&'a [SymbolId]),
    Unsearched,
    NotReference(&'a str),
    Unresolved(&'a str),
}

/// A captured name token, shared by semantic and lexical projections.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Occurrence<'a> {
    pub(crate) span: Span,
    /// The decoded name, as an `Index::text` string.
    pub(crate) name: StringId,
    pub(crate) call: bool,
    pub(crate) target: OccurrenceTarget<'a>,
}

/// An occurrence's reference-token inventory facts.
#[derive(Debug, Clone, Copy)]
pub(super) struct ReferenceData<'a> {
    /// The exact decoded key, or why it is unavailable.
    pub(super) key: Result<&'a str, &'a str>,
    pub(super) syntax: ReferenceSyntax,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OccurrenceRef {
    pub(crate) file: FileId,
    pub(crate) id: OccurrenceId,
}

/// `Target`, stored: one symbol inline, several in `Index::several`, and
/// reasons as `Index::text` strings.
#[derive(Debug, Clone, Copy)]
enum StoredTarget {
    Symbol(SymbolId),
    Several(u32),
    Unsearched,
    NotReference(StringId),
    Unresolved(StringId),
}

/// One occurrence's facts besides its spans (`FileOccurrences::spans`).
#[derive(Debug, Clone, Copy)]
struct Record {
    name: StringId,
    target: StoredTarget,
    /// The reference token's exact key or the reason it has none, as
    /// `Index::text` strings; absent for an occurrence without one.
    key: Option<Result<StringId, StringId>>,
    roles: u8,
    call: bool,
    /// The reference token's syntax, meaningful only with a `key`.
    kind: SyntaxKind,
    role: ReferenceTokenRole,
    reference_roles: u8,
    traversal_header: bool,
}

/// Spans packed to `u32` offsets where all of them fit, as `SiteMap` packs
/// sites: never a truncated offset or a new input restriction.
#[derive(Debug)]
enum Spans {
    Narrow(Box<[[u32; 2]]>),
    Wide(Box<[Span]>),
}

impl Spans {
    fn new(spans: Vec<Span>) -> Self {
        let narrow: Option<Vec<[u32; 2]>> = spans
            .iter()
            .map(|span| {
                Some([
                    u32::try_from(span.start).ok()?,
                    u32::try_from(span.end).ok()?,
                ])
            })
            .collect();
        match narrow {
            Some(narrow) => Spans::Narrow(narrow.into_boxed_slice()),
            None => Spans::Wide(spans.into_boxed_slice()),
        }
    }

    fn get(&self, index: usize) -> Span {
        match self {
            Spans::Narrow(spans) => {
                let [start, end] = spans[index];
                Span {
                    start: start as usize,
                    end: end as usize,
                }
            }
            Spans::Wide(spans) => spans[index],
        }
    }
}

/// The spans of each occurrence, in this order.
const SELECTION: usize = 0;
const NATIVE: usize = 1;
const OCCURRENCE: usize = 2;
const SPANS: usize = 3;

/// One file's occurrences, ordered by start.
#[derive(Debug)]
struct FileOccurrences {
    records: Box<[Record]>,
    /// `SPANS` per occurrence: its selection span, then its reference token's
    /// native and occurrence spans (empty without a token).
    spans: Spans,
}

impl FileOccurrences {
    fn start(&self, id: OccurrenceId) -> usize {
        self.spans.get(id.index() * SPANS + SELECTION).start
    }

    fn span(&self, id: OccurrenceId, which: usize) -> Span {
        self.spans.get(id.index() * SPANS + which)
    }
}

/// A stored reference token: `ReferenceToken` with its JSON key's identity
/// out of line (`Index::json_keys`).
#[derive(Debug, Clone, Copy)]
struct StoredToken {
    file: FileId,
    entry: StoredEntry,
}

#[derive(Debug, Clone, Copy)]
enum StoredEntry {
    Syntax(OccurrenceId),
    Json(u32),
}

/// The reference tokens of one decoded key, in public path/start/end order.
#[derive(Debug, Clone, Copy)]
pub(in crate::project) struct ReferenceTokens<'a> {
    tokens: &'a [StoredToken],
    json_keys: &'a [JsonPropertyId],
}

impl<'a> ReferenceTokens<'a> {
    pub(in crate::project) fn iter(self) -> impl Iterator<Item = ReferenceToken> + 'a {
        let json_keys = self.json_keys;
        self.tokens.iter().map(move |token| ReferenceToken {
            file: token.file,
            entry: match token.entry {
                StoredEntry::Syntax(id) => ReferenceTokenEntry::Syntax(id),
                StoredEntry::Json(key) => ReferenceTokenEntry::Json(json_keys[key as usize]),
            },
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct Index {
    /// Its maps are only looked up; nothing iterates them.
    sites: FastMap<SymbolId, Box<[IndexedSite]>>,
    /// Occurrences of each file, indexed by `FileId`.
    files: Vec<FileOccurrences>,
    /// Derived selectors over the same entries; full JSX names can overlap
    /// legacy identifier children without changing lexical position queries.
    lexical_positions: Vec<Box<[OccurrenceId]>>,
    reference_positions: Vec<Box<[OccurrenceId]>>,
    /// Public result order, with duplicate locations' roles combined. This
    /// immutable work is shared by position and name queries.
    by_symbol: FastMap<SymbolId, Box<[IndexedOccurrence]>>,
    by_name: FastMap<String, Vec<OccurrenceRef>>,
    by_reference_name: FastMap<String, Box<[StoredToken]>>,
    /// The symbols of each occurrence that denotes more than one.
    several: Vec<Box<[SymbolId]>>,
    /// The JSON keys among reference tokens.
    json_keys: Vec<JsonPropertyId>,
    /// Names and reasons shared by many occurrences.
    strings: Vec<Box<str>>,
}

impl Index {
    /// The identifier token containing `offset`, start inclusive.
    pub(crate) fn occurrence_at(&self, file: FileId, offset: usize) -> Option<Occurrence<'_>> {
        let occurrences = self.files.get(file.index())?;
        let positions = self.lexical_positions.get(file.index())?;
        let index = positions.partition_point(|&id| occurrences.start(id) <= offset);
        let id = *positions.get(index.checked_sub(1)?)?;
        let occurrence = self.occurrence(OccurrenceRef { file, id });
        (offset < occurrence.span.end).then_some(occurrence)
    }

    pub(crate) fn occurrence(&self, reference: OccurrenceRef) -> Occurrence<'_> {
        let file = &self.files[reference.file.index()];
        let record = &file.records[reference.id.index()];
        Occurrence {
            span: file.span(reference.id, SELECTION),
            name: record.name,
            call: record.call,
            target: match &record.target {
                StoredTarget::Symbol(symbol) => {
                    OccurrenceTarget::Symbols(std::slice::from_ref(symbol))
                }
                StoredTarget::Several(index) => {
                    OccurrenceTarget::Symbols(&self.several[*index as usize])
                }
                StoredTarget::Unsearched => OccurrenceTarget::Unsearched,
                StoredTarget::NotReference(why) => OccurrenceTarget::NotReference(self.text(*why)),
                StoredTarget::Unresolved(why) => OccurrenceTarget::Unresolved(self.text(*why)),
            },
        }
    }

    /// The occurrence's reference token, where it has one.
    pub(super) fn reference_data(&self, reference: OccurrenceRef) -> Option<ReferenceData<'_>> {
        let file = &self.files[reference.file.index()];
        let record = &file.records[reference.id.index()];
        let key = record.key?;
        Some(ReferenceData {
            key: match key {
                Ok(key) => Ok(self.text(key)),
                Err(why) => Err(self.text(why)),
            },
            syntax: ReferenceSyntax {
                kind: record.kind,
                native_span: file.span(reference.id, NATIVE),
                occurrence_span: file.span(reference.id, OCCURRENCE),
                role: record.role,
                roles: record.reference_roles,
                traversal_header: record.traversal_header,
            },
        })
    }

    /// Declaration, local import, and exported alias names spelled `name`.
    pub(crate) fn candidates(&self, name: &str) -> &[OccurrenceRef] {
        self.by_name.get(name).map_or(&[], Vec::as_slice)
    }

    pub(super) fn reference_tokens(
        &self,
        name: &str,
        control: Option<&crate::resources::Control>,
    ) -> ReferenceTokens<'_> {
        // getPossibleSymbolReferencePositions' decoded-key accelerator. Debit
        // the exact query bytes before the indivisible standard-table lookup;
        // this neither copies the key nor changes its hash/equality policy.

        let tokens: &[StoredToken] = self
            .by_reference_name
            .get(name)
            .map_or(&[], |tokens| tokens);
        if let Some(control) = control {
            crate::resources::complete_or_abort(control.check());
        }
        ReferenceTokens {
            tokens,
            json_keys: &self.json_keys,
        }
    }

    pub(super) fn reference_token_at(&self, file: FileId, offset: usize) -> Option<ReferenceToken> {
        let occurrences = self.files.get(file.index())?;
        let positions = self.reference_positions.get(file.index())?;
        let index = positions.partition_point(|&id| occurrences.start(id) <= offset);
        let &id = positions.get(index.checked_sub(1)?)?;
        (offset < occurrences.span(id, SELECTION).end).then_some(ReferenceToken {
            file,
            entry: ReferenceTokenEntry::Syntax(id),
        })
    }

    pub(crate) fn occurrences_of(&self, symbol: SymbolId) -> &[IndexedOccurrence] {
        self.by_symbol
            .get(&symbol)
            .map_or(&[], |occurrences| occurrences)
    }

    pub(crate) fn text(&self, id: StringId) -> &str {
        &self.strings[id.index()]
    }

    pub(crate) fn sites(&self, symbol: SymbolId) -> &[IndexedSite] {
        self.sites.get(&symbol).map_or(&[], |sites| sites)
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

/// Append `value` to `key`'s list, allocating the key only for a new list.
fn push_keyed<V>(map: &mut FastMap<String, Vec<V>>, key: &str, value: V) {
    match map.get_mut(key) {
        Some(values) => values.push(value),
        None => {
            map.insert(key.to_owned(), vec![value]);
        }
    }
}

/// Builds an `Index` one file at a time, in `FileId` order.
#[derive(Debug, Default)]
pub(crate) struct Builder {
    index: Index,
    string_ids: FastMap<Box<str>, StringId>,
    /// `finish` sorts each entry, so its order is not the hasher's.
    by_symbol: FastMap<SymbolId, Vec<OccurrenceRef>>,
    by_reference_name: FastMap<String, Vec<(StoredToken, Span)>>,
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
        let mut records = Vec::with_capacity(tokens.len());
        let mut spans = Vec::with_capacity(tokens.len() * SPANS);
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
            let empty = Span {
                start: token.span.start,
                end: token.span.start,
            };
            let syntax = token.reference.map(|data| data.syntax);
            let key = token.reference.map(|data| {
                reference_positions.push(reference.id);
                match &data.key {
                    Ok(key) => {
                        push_keyed(
                            &mut self.by_reference_name,
                            key,
                            (
                                StoredToken {
                                    file,
                                    entry: StoredEntry::Syntax(reference.id),
                                },
                                token.span,
                            ),
                        );
                        Ok(self.intern(key))
                    }
                    Err(why) => Err(self.intern(why)),
                }
            });
            if searched && let Target::Symbols(symbols) = &token.target {
                for &symbol in symbols {
                    self.by_symbol.entry(symbol).or_default().push(reference);
                }
            }
            if searched && token.candidate {
                push_keyed(&mut self.index.by_name, token.name, reference);
            }
            let target = match token.target {
                Target::Symbols(symbols) if symbols.len() == 1 => StoredTarget::Symbol(symbols[0]),
                Target::Symbols(symbols) => {
                    let index = u32::try_from(self.index.several.len())
                        .expect("semantic record count fits in u32");
                    self.index
                        .several
                        .push(symbols.into_vec().into_boxed_slice());
                    StoredTarget::Several(index)
                }
                Target::Unsearched => StoredTarget::Unsearched,
                Target::NotReference(why) => StoredTarget::NotReference(self.intern(why)),
                Target::Unresolved(why) => StoredTarget::Unresolved(why),
            };
            spans.push(token.span);
            spans.push(syntax.map_or(empty, |syntax| syntax.native_span));
            spans.push(syntax.map_or(empty, |syntax| syntax.occurrence_span));
            records.push(Record {
                name: self.intern(token.name),
                target,
                key,
                roles: token.roles,
                call: token.call,
                kind: syntax.map_or(SyntaxKind::Unknown, |syntax| syntax.kind),
                role: syntax.map_or(ReferenceTokenRole::Name, |syntax| syntax.role),
                reference_roles: syntax.map_or(0, |syntax| syntax.roles),
                traversal_header: syntax.is_some_and(|syntax| syntax.traversal_header),
            });
        }
        self.index.files.push(FileOccurrences {
            records: records.into_boxed_slice(),
            spans: Spans::new(spans),
        });
        self.index
            .lexical_positions
            .push(lexical_positions.into_boxed_slice());
        self.index
            .reference_positions
            .push(reference_positions.into_boxed_slice());
        names
    }

    pub(super) fn add_json(&mut self, file: FileId, json: &super::JsonFacts) {
        for key in json.reference_keys() {
            if let Some(name) = key.key.as_str() {
                let index = u32::try_from(self.index.json_keys.len())
                    .expect("semantic record count fits in u32");
                self.index.json_keys.push(key.id);
                push_keyed(
                    &mut self.by_reference_name,
                    name,
                    (
                        StoredToken {
                            file,
                            entry: StoredEntry::Json(index),
                        },
                        key.span,
                    ),
                );
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
        let files = &self.index.files;
        let mut all_sites = FastMap::default();
        let mut by_symbol = FastMap::default();
        for (symbol, references) in self.by_symbol {
            let mut declarations: Vec<_> = sites(symbol)
                .into_iter()
                .map(|site| IndexedSite {
                    name: site.name,
                    file: site.file,
                    span: site.span,
                })
                .collect();
            let site_key = |site: &IndexedSite| order.key(sources, site.file, site.span);
            declarations.sort_by_key(site_key);
            declarations.dedup_by(|a, b| site_key(a) == site_key(b) && a.name == b.name);
            all_sites.insert(symbol, declarations.into_boxed_slice());

            let key = |occurrence: &IndexedOccurrence| {
                let reference = occurrence.occurrence;
                let span = files[reference.file.index()].span(reference.id, SELECTION);
                order.key(sources, reference.file, span)
            };
            let mut occurrences: Vec<_> = references
                .into_iter()
                .map(|reference| {
                    let record = &files[reference.file.index()].records[reference.id.index()];
                    IndexedOccurrence {
                        occurrence: reference,
                        roles: record.roles,
                        call: record.call,
                    }
                })
                .collect();
            // Stable sorting keeps the first token's spelling at a shared
            // location, as the former query-time map did. Every role survives.
            occurrences.sort_by_key(key);
            occurrences.dedup_by(|later, first| {
                if key(later) != key(first) {
                    return false;
                }
                first.roles |= later.roles;
                first.call |= later.call;
                true
            });
            by_symbol.insert(symbol, occurrences.into_boxed_slice());
        }
        self.index.sites = all_sites;
        self.index.by_symbol = by_symbol;
        self.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::sources::SourceFile;
    use crate::source::SourceText;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Every project name has an occurrence: its record stays a few words,
    /// with spans packed beside it and wide values out of line.
    #[test]
    fn occurrence_records_stay_compact() {
        assert!(std::mem::size_of::<Record>() <= 28);
        assert!(std::mem::size_of::<IndexedOccurrence>() <= 12);
        assert!(std::mem::size_of::<StoredToken>() <= 12);
    }

    /// Offsets that do not fit `u32` keep their full width, for the file.
    #[test]
    fn spans_pack_only_what_fits() {
        let spans = |offsets: &[(usize, usize)]| {
            offsets
                .iter()
                .map(|&(start, end)| Span { start, end })
                .collect::<Vec<_>>()
        };
        let narrow = Spans::new(spans(&[(0, 1), (7, u32::MAX as usize)]));
        assert!(matches!(narrow, Spans::Narrow(_)));
        assert_eq!(
            narrow.get(1),
            Span {
                start: 7,
                end: u32::MAX as usize
            }
        );
        if let Some(wide_end) = (u32::MAX as usize).checked_add(1) {
            let wide = Spans::new(spans(&[(0, 1), (7, wide_end)]));
            assert!(matches!(wide, Spans::Wide(_)));
            assert_eq!(wide.get(0), Span { start: 0, end: 1 });
            assert_eq!(
                wide.get(1),
                Span {
                    start: 7,
                    end: wide_end
                }
            );
        }
    }

    fn token(span: Span, target: Target) -> Token<'static> {
        Token {
            source: NameId::new(0),
            span,
            name: "value",
            roles: crate::project::binding::collect::USE,
            call: false,
            candidate: false,
            target,
            lexical: true,
            reference: None,
        }
    }

    /// Public locations order by display name, then position. Within a file
    /// byte order is position order; files that share a display name
    /// interleave by position, and a position both share is one location.
    #[test]
    fn locations_order_by_position_where_files_share_a_display_name() {
        let symbol = SymbolId::new(0);
        // `b` is at offset 8, on the third line, of the first file, and at
        // offset 11, on the second line, of the second, which shares its
        // display name: the second's is earlier by position, later by offset.
        let texts = ["\n\n      b", "          \nb", "\n\n      b"];
        let displays = ["same.ts", "same.ts", "other.ts"];
        let mut sources = Sources::default();
        for (index, (text, display)) in texts.iter().zip(displays).enumerate() {
            let id = sources.reserve();
            sources.capture(
                id,
                SourceFile {
                    path: PathBuf::from(format!("/p/{index}.ts")),
                    display: display.into(),
                    text: Arc::new(SourceText::new((*text).into())),
                },
            );
        }
        let mut builder = Builder::default();
        let at = |text: &str| {
            let start = text.find('b').unwrap();
            Span {
                start,
                end: start + 1,
            }
        };
        let ids: Vec<FileId> = sources.ids().collect();
        for (&id, text) in ids.iter().zip(texts) {
            let target = Target::Symbols(SmallVec::from_elem(symbol, 1));
            builder.add_file(id, true, vec![token(at(text), target)]);
        }
        let index = builder.finish(&sources, |_| Vec::new());
        let order: Vec<(usize, usize)> = index
            .occurrences_of(symbol)
            .iter()
            .map(|indexed| {
                (
                    indexed.occurrence.file.index(),
                    index.occurrence(indexed.occurrence).span.start,
                )
            })
            .collect();
        // other.ts first; then same.ts's two files by position.
        assert_eq!(order, [(2, 8), (1, 11), (0, 8)]);

        // One position in two files of one display name is one location.
        let mut sources = Sources::default();
        for index in 0..2 {
            let id = sources.reserve();
            sources.capture(
                id,
                SourceFile {
                    path: PathBuf::from(format!("/p/{index}.ts")),
                    display: "same.ts".into(),
                    text: Arc::new(SourceText::new("b".into())),
                },
            );
        }
        let mut builder = Builder::default();
        for id in sources.ids() {
            let target = Target::Symbols(SmallVec::from_elem(symbol, 1));
            builder.add_file(id, true, vec![token(Span { start: 0, end: 1 }, target)]);
        }
        let index = builder.finish(&sources, |_| Vec::new());
        assert_eq!(index.occurrences_of(symbol).len(), 1);
    }
}
