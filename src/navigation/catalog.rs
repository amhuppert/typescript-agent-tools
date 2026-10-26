//! The declaration catalog of one captured source: projected records, the
//! statement sites beside them, the selection rules over both, and their
//! public form. Outline and show use the same records; selection reads only
//! keys, containment, targets, and statements' first tokens, never syntax. A selection index owns name selection and the facts that
//! describe a keyed site, for a snapshot's captured index and a catalog's
//! records alike. A catalog holds the source its records describe, so no
//! caller can pair it with other text, such as another generation's.

use super::projection::{self, Detail, Record, StatementSite};
use super::{Candidate, Declaration, Found, Kind, Selector, ShowOutcome, SourceRange, Specifier};
use crate::analysis::Parser;
use crate::source::{PositionError, SourceText};
use crate::{Ast, Diagnostic, NodeId};
use smallvec::SmallVec;
use std::num::NonZeroU32;
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct Catalog {
    records: Vec<Record>,
    /// The statement sites, ordered by first token: a collection apart from
    /// `records`, so the outline, record ordinals, and `index` are the
    /// records' alone.
    statements: Vec<StatementSite>,
    /// The selection index of `records`, which owns name selection and the
    /// key, selector, and candidate of each keyed record, as it does for a
    /// snapshot's captured index; it holds the text `records` describe.
    index: Index,
}

impl Catalog {
    /// The catalog of `ast`, which was parsed from `source`.
    pub(crate) fn new(ast: &Ast<'_>, source: Arc<SourceText>) -> Self {
        debug_assert!(
            std::ptr::eq(ast.source(), source.as_str()),
            "a catalog describes the text its syntax tree was parsed from"
        );
        let (records, statements) = projection::complete(ast);
        let index = Index::of(&records, source);
        Self {
            records,
            statements,
            index,
        }
    }

    /// What a name or position selects. A name selects records only. A
    /// position selects the records anchored at it or with a target that
    /// contains it, in source order; only when there are none, the
    /// statement whose first token contains it, from the token's first
    /// character up to but not including its end. A position outside the
    /// text is an error; one that names no site selects no record.
    pub(crate) fn select(&self, selector: &Selector) -> Result<Selection, PositionError> {
        Ok(match selector {
            Selector::Name(name) => Selection::Records(
                self.index
                    .select_name(name)
                    .into_iter()
                    .map(|site| self.index.record(site))
                    .collect(),
            ),
            Selector::At(position) => {
                let offset = self.index.source.offset(*position)?;
                let records: Vec<usize> = (0..self.records.len())
                    .filter(|&index| {
                        let record = &self.records[index];
                        record.anchor == offset
                            || record
                                .targets
                                .iter()
                                .any(|target| target.start <= offset && offset < target.end)
                    })
                    .collect();
                if records.is_empty()
                    && let Some(statement) = self.statement_at(offset)
                {
                    Selection::Statement(statement)
                } else {
                    Selection::Records(records)
                }
            }
        })
    }

    /// The statement site whose first token contains `offset`. First tokens
    /// are ordered and never overlap, since a token is the first token of at
    /// most one statement.
    fn statement_at(&self, offset: usize) -> Option<usize> {
        let at = self
            .statements
            .partition_point(|statement| statement.first.end <= offset);
        self.statements
            .get(at)
            .filter(|statement| statement.first.start <= offset)
            .map(|_| at)
    }

    /// The import and export specifiers that spell `name` without
    /// declaring it, in source order: the records whose `unkeyed_alias`,
    /// which the projection gives only such specifiers, is exactly `name`.
    pub(crate) fn specifiers(&self, name: &str) -> Vec<Specifier> {
        self.records
            .iter()
            .filter(|record| record.unkeyed_alias.as_deref() == Some(name))
            .map(|record| Specifier {
                kind: record.kind,
                anchor: self.index.source.position(record.anchor),
            })
            .collect()
    }

    /// Every declaration, in source order.
    pub(crate) fn declarations(&self) -> Vec<Declaration> {
        (0..self.records.len())
            .map(|index| self.declaration(index))
            .collect()
    }

    /// The selected site with its exact source, in `file`.
    pub(crate) fn selected(&self, site: SelectedSite, file: &str) -> Found {
        match site {
            SelectedSite::Record(index) => self.found(index, file),
            SelectedSite::Statement(statement) => self.found_statement(statement, file),
        }
    }

    /// The selected declaration with its exact source, in `file`.
    pub(crate) fn found(&self, index: usize, file: &str) -> Found {
        let range = self.records[index].range;
        Found {
            file: file.to_owned(),
            declaration: self.declaration(index),
            source: self.index.source.as_str()[range.start..range.end].to_owned(),
        }
    }

    /// The selected statement site with its exact source, in `file`: a
    /// declaration of kind `Statement`, without a key, selector, or
    /// signature.
    pub(crate) fn found_statement(&self, statement: usize, file: &str) -> Found {
        let source = &self.index.source;
        let statement = &self.statements[statement];
        let range = statement.range;
        Found {
            file: file.to_owned(),
            declaration: Declaration {
                label: statement.label.clone(),
                key: None,
                selector: None,
                kind: Kind::Statement,
                anchor: source.position(statement.first.start),
                range: SourceRange {
                    start: source.position(range.start),
                    end: source.position(range.end),
                },
                signature: None,
                depth: statement.depth,
            },
            source: source.as_str()[range.start..range.end].to_owned(),
        }
    }

    fn declaration(&self, index: usize) -> Declaration {
        let source = &self.index.source;
        let record = &self.records[index];
        let site = self.index.site_of(index);
        Declaration {
            label: record.label.clone(),
            key: site.map(|site| self.index.key(site).to_owned()),
            selector: site.and_then(|site| self.index.selector(site)),
            kind: record.kind,
            anchor: source.position(record.anchor),
            range: SourceRange {
                start: source.position(record.range.start),
                end: source.position(record.range.end),
            },
            signature: record.signature.clone(),
            depth: record.depth,
        }
    }

    /// One of several matches, in `file`: a keyed record as its index
    /// describes it, which is also how a project-wide name search gives it.
    pub(crate) fn candidate(&self, index: usize, file: &str) -> Candidate {
        if let Some(site) = self.index.site_of(index) {
            return self.index.candidate(site, file);
        }
        // Only a position selects a site without a key.
        let record = &self.records[index];
        Candidate {
            file: file.to_owned(),
            label: record.label.clone(),
            kind: record.kind,
            key: None,
            selector: None,
            anchor: self.index.source.position(record.anchor),
        }
    }
}

/// A catalog with the syntax tree its records were projected from: the one
/// way to learn which syntax a selected site is, lent only while the tree
/// lives. Selection, candidates, and excerpts are the catalog's, so an
/// operation on a selected site, such as an edit, selects exactly as
/// `show` does and decides afterwards what the site supports.
pub(crate) struct Syntax<'t, 'src> {
    ast: &'t Ast<'src>,
    catalog: Catalog,
}

/// What a selector selects in one catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Selection {
    /// Records, by ordinal, in source order: one is found, several are
    /// candidates, and none is a missing selection.
    Records(Vec<usize>),
    /// A statement site, by its place among the catalog's statement sites,
    /// which is selected only alone.
    Statement(usize),
}

impl Selection {
    /// The one site selected, if exactly one is.
    pub(crate) fn single(&self) -> Option<SelectedSite> {
        match self {
            Self::Records(records) => match records[..] {
                [record] => Some(SelectedSite::Record(record)),
                _ => None,
            },
            Self::Statement(statement) => Some(SelectedSite::Statement(*statement)),
        }
    }
}

/// One selected site of a catalog: a record, by ordinal, or a statement
/// site, by its place among the statement sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectedSite {
    Record(usize),
    Statement(usize),
}

/// One selected site's syntax in a `Syntax`'s tree.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SiteSyntax {
    /// The site's own node, such as a variable declaration or a method.
    pub(crate) node: NodeId,
    /// What the site designates as a whole, which `show` excerpts: a
    /// binding's variable statement, a specifier's import or export
    /// declaration, and otherwise the site's node (`projection::Record`),
    /// which for a statement site is the statement.
    pub(crate) designation: NodeId,
    pub(crate) kind: Kind,
}

impl<'t, 'src> Syntax<'t, 'src> {
    /// The catalog of `ast`, which was parsed from `source`, with its tree.
    pub(crate) fn new(ast: &'t Ast<'src>, source: Arc<SourceText>) -> Self {
        Self {
            ast,
            catalog: Catalog::new(ast, source),
        }
    }

    pub(crate) fn ast(&self) -> &'t Ast<'src> {
        self.ast
    }

    pub(crate) fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// The syntax of a selected site.
    pub(crate) fn site(&self, site: SelectedSite) -> SiteSyntax {
        match site {
            SelectedSite::Record(index) => {
                let record = &self.catalog.records[index];
                SiteSyntax {
                    node: record.node,
                    designation: record.designation,
                    kind: record.kind,
                }
            }
            SelectedSite::Statement(statement) => {
                let node = self.catalog.statements[statement].node;
                SiteSyntax {
                    node,
                    designation: node,
                    kind: Kind::Statement,
                }
            }
        }
    }
}

/// Key segments, almost always one.
type Segments<'a> = SmallVec<[&'a str; 1]>;

/// A compact selection index of one captured source: the keyed sites,
/// each with its key, nearest keyed container, kind, and anchor, enough to
/// select by name and to describe candidates. It is the one owner of those
/// facts: a catalog derives its keys, selectors, and candidates from the
/// index of its own records. Ranges, signatures, unkeyed sites, and
/// statement sites, which only a position selects, are left to the complete
/// catalog, which projecting the same text again gives, with each site at
/// its `record` ordinal.
#[derive(Debug)]
pub(crate) struct Index {
    sites: Vec<Site>,
    /// The labels that differ from their site's key, such as quoted string
    /// names, by site.
    labels: Vec<(u32, Box<str>)>,
    source: Arc<SourceText>,
}

#[derive(Debug)]
struct Site {
    /// The site's ordinal among every projected record of the file.
    record: u32,
    /// The nearest keyed container, as an index into `sites` plus one.
    parent: Option<NonZeroU32>,
    anchor: u32,
    kind: Kind,
    /// Whether `key` joins several segments with `.`, as a dotted
    /// namespace's does; other keys are one segment that may hold a `.`.
    dotted: bool,
    key: Box<str>,
}

impl Index {
    /// The index of `ast`, which was parsed from `source`, projected
    /// without the labels and signatures only a catalog shows.
    pub(crate) fn new(ast: &Ast<'_>, source: Arc<SourceText>) -> Self {
        debug_assert!(
            std::ptr::eq(ast.source(), source.as_str()),
            "an index describes the text its syntax tree was parsed from"
        );
        Self::of(&projection::project(ast, Detail::Selection), source)
    }

    /// The index of a projection's `records` of `source`.
    fn of(records: &[Record], source: Arc<SourceText>) -> Self {
        // Each record's nearest keyed container or itself, as a site.
        let mut keyed: Vec<Option<u32>> = Vec::with_capacity(records.len());
        let mut sites = Vec::new();
        let mut labels = Vec::new();
        for (ordinal, record) in records.iter().enumerate() {
            let container = record.parent.and_then(|parent| keyed[parent]);
            if record.key.is_empty() {
                keyed.push(container);
                continue;
            }
            let site = narrow(sites.len());
            let key = record.key.join(".");
            if record.label != key {
                labels.push((site, record.label.as_str().into()));
            }
            sites.push(Site {
                record: narrow(ordinal),
                parent: container.map(|parent| {
                    parent
                        .checked_add(1)
                        .and_then(NonZeroU32::new)
                        .expect("a site number fits in u32")
                }),
                anchor: narrow(record.anchor),
                kind: record.kind,
                dotted: record.key.len() > 1,
                key: key.into_boxed_str(),
            });
            keyed.push(Some(site));
        }
        sites.shrink_to_fit();
        labels.shrink_to_fit();
        Self {
            sites,
            labels,
            source,
        }
    }

    fn segments(&self, site: usize) -> Segments<'_> {
        let site = &self.sites[site];
        if site.dotted {
            site.key.split('.').collect()
        } else {
            smallvec::smallvec![&*site.key]
        }
    }

    fn parent(&self, site: usize) -> Option<usize> {
        self.sites[site]
            .parent
            .map(|parent| parent.get() as usize - 1)
    }

    /// The sites a name selects, in source order: each site whose key ends
    /// the name, and whose nearest keyed ancestors' keys are the name's
    /// earlier segments, in order. Ancestors without a key are skipped;
    /// ancestors beyond the name are unconstrained.
    pub(crate) fn select_name(&self, name: &str) -> Vec<usize> {
        let segments: Vec<&str> = name.split('.').collect();
        if !segments.iter().all(|segment| spellable(segment)) {
            return Vec::new();
        }
        (0..self.sites.len())
            .filter(|&site| self.matches(site, &segments))
            .collect()
    }

    /// The existing catalog's member declarations with this exact decoded key.
    /// Product advisory only: no semantic resolution, inferred members, or
    /// fabricated selectors for unspellable keys/ancestors. Multiple syntax
    /// sites may intentionally report the same display selector. This is a
    /// different question from `select_name`: an empty key is a valid member
    /// identity even though the selector language cannot spell it.
    pub(crate) fn member_candidates(&self, name: &str, file: &str) -> Vec<Candidate> {
        self.sites
            .iter()
            .enumerate()
            .filter(|(_, site)| {
                site.key.as_ref() == name
                    && matches!(
                        site.kind,
                        Kind::Method | Kind::Accessor | Kind::Property | Kind::EnumMember
                    )
            })
            .map(|(site, _)| self.candidate(site, file))
            .collect()
    }

    fn matches(&self, site: usize, segments: &[&str]) -> bool {
        let mut remaining = segments;
        let mut current = Some(site);
        while let Some(at) = current {
            for segment in self.segments(at).iter().rev() {
                let Some((last, rest)) = remaining.split_last() else {
                    return true;
                };
                if last != segment {
                    return false;
                }
                remaining = rest;
            }
            if remaining.is_empty() {
                return true;
            }
            current = self.parent(at);
        }
        false
    }

    /// A site's ordinal among the file's complete catalog records.
    pub(crate) fn record(&self, site: usize) -> usize {
        self.sites[site].record as usize
    }

    /// The site of the record at `ordinal`, if it has a key.
    fn site_of(&self, ordinal: usize) -> Option<usize> {
        self.sites
            .binary_search_by_key(&ordinal, |site| site.record as usize)
            .ok()
    }

    /// A site's key: its name's exact text, or a dotted namespace's
    /// segments joined with `.`.
    fn key(&self, site: usize) -> &str {
        &self.sites[site].key
    }

    /// One of several matches, in `file`.
    pub(crate) fn candidate(&self, site: usize, file: &str) -> Candidate {
        let entry = &self.sites[site];
        let label = match self
            .labels
            .binary_search_by_key(&(site as u32), |(site, _)| *site)
        {
            Ok(at) => self.labels[at].1.to_string(),
            Err(_) => entry.key.to_string(),
        };
        Candidate {
            file: file.to_owned(),
            label,
            kind: entry.kind,
            key: Some(entry.key.to_string()),
            selector: self.selector(site),
            anchor: self.source.position(entry.anchor as usize),
        }
    }

    /// A site's qualified selector: the keys of its keyed ancestors and its
    /// own, joined with `.`, when the selector language can spell every
    /// segment (`spellable`).
    fn selector(&self, site: usize) -> Option<String> {
        let mut segments: Vec<&str> = Vec::new();
        let mut current = Some(site);
        while let Some(at) = current {
            for segment in self.segments(at).into_iter().rev() {
                if !spellable(segment) {
                    return None;
                }
                segments.push(segment);
            }
            current = self.parent(at);
        }
        segments.reverse();
        Some(segments.join("."))
    }

    /// An index of `source` with no sites, whatever the text: a test's
    /// stand-in for captured text that projecting again fails to parse,
    /// which loading never captures.
    #[cfg(test)]
    pub(crate) fn without_sites(source: Arc<SourceText>) -> Self {
        Self {
            sites: Vec::new(),
            labels: Vec::new(),
            source,
        }
    }

    /// The captured text the index describes.
    pub(crate) fn source(&self) -> &SourceText {
        &self.source
    }

    /// The complete catalog of the indexed text, projected again from the
    /// same text under the file name it was parsed with, so its records are
    /// this index's sites at their `record` ordinals.
    pub(crate) fn catalog(&self, parser: &Parser, file_name: &str) -> Result<Catalog, Diagnostic> {
        let ast = parser.parse(self.source.as_str(), file_name)?;
        Ok(Catalog::new(&ast, Arc::clone(&self.source)))
    }
}

/// A record ordinal, site number, or offset of one file, stored in 32 bits
/// as the binder stores its record identities.
fn narrow(value: usize) -> u32 {
    u32::try_from(value).expect("a file's records and offsets fit in u32")
}

/// The outcome of a selection in `catalog`, whose file results name
/// `file`: one site, a record or a statement site, is found, several
/// records are candidates, and none is not found, listing no specifier: a
/// show that misses a name fills them (`Catalog::specifiers`), and an
/// edit's selection lists none.
pub(crate) fn outcome(file: &str, catalog: &Catalog, selection: &Selection) -> ShowOutcome {
    match selection {
        Selection::Records(records) if records.len() > 1 => ShowOutcome::Ambiguous(
            records
                .iter()
                .map(|&index| catalog.candidate(index, file))
                .collect(),
        ),
        selection => match selection.single() {
            Some(site) => ShowOutcome::Found(catalog.selected(site, file)),
            None => ShowOutcome::NotFound {
                specifiers: Vec::new(),
            },
        },
    }
}

/// The selector language: a selector is key segments joined with `.`, so
/// it can spell exactly the non-empty segments that contain no `.`. This one
/// rule decides which selectors match (`settings..e` matches nothing) and
/// which entries report a `selector` (`settings.display-name` and
/// `@scope/pkg.X` do; a key `"a.b"` does not). A product rule: TypeScript
/// has no selector language.
fn spellable(segment: &str) -> bool {
    !segment.is_empty() && !segment.contains('.')
}
