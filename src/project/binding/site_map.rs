//! Frozen exact-syntax-site lookup. Capture appends in binder order; freezing
//! retains the last attachment at an identical kind and span, as HashMap did.

use crate::Span;
use crate::syntax_kind::SyntaxKind;

type Key = (SyntaxKind, Span);

/// Lossless file-local packing: `unpack` returns the value `pack` was given.
pub(super) trait PackedValue: Copy {
    type Packed: Copy + std::fmt::Debug;

    fn pack(self) -> Self::Packed;
    fn unpack(value: Self::Packed) -> Self;
}

#[derive(Debug)]
pub(super) struct SiteMap<V: PackedValue> {
    entries: Box<[(CompactSite, V::Packed)]>,
}

#[derive(Debug, Clone, Copy)]
struct CompactSite {
    start: u32,
    end: u32,
    kind: SyntaxKind,
}

/// A span's offsets as `u32`: the one packing rule of binding's frozen
/// facts. Both fit, since loading refuses a source longer than `u32::MAX`
/// bytes (`program::capture::check_length`, the owner of that limit).
pub(super) fn pack_span(span: Span) -> [u32; 2] {
    debug_assert!(
        span.start <= u32::MAX as usize && span.end <= u32::MAX as usize,
        "a source's offsets fit u32: {span:?}"
    );
    [span.start as u32, span.end as u32]
}

impl CompactSite {
    fn pack((kind, span): Key) -> Self {
        let [start, end] = pack_span(span);
        Self { start, end, kind }
    }

    fn key(self) -> (usize, usize, u32) {
        (self.start as usize, self.end as usize, self.kind.number())
    }
}

fn key((kind, span): Key) -> (usize, usize, u32) {
    (span.start, span.end, kind.number())
}

/// Sort `writes` by site, stably, and keep one write per site: the first,
/// taking the value of each later write that `replaces` it, in order. The
/// one write rule of both constructors.
fn resolve<T>(
    writes: &mut Vec<T>,
    site: impl Fn(&T) -> Key,
    replaces: impl Fn(&T) -> bool,
    take: impl Fn(&mut T, &T),
) {
    writes.sort_by_key(|write| key(site(write)));
    // dedup_by passes the later write first: update the kept one before
    // removing the later slot.
    writes.dedup_by(|later, kept| {
        if site(later) != site(kept) {
            return false;
        }
        if replaces(later) {
            take(kept, later);
        }
        true
    });
}

impl<V: PackedValue> Default for SiteMap<V> {
    fn default() -> Self {
        Self {
            entries: Box::default(),
        }
    }
}

/// How a write treats a value already written at its site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Write {
    Replace,
    KeepFirst,
}

impl<V: PackedValue> SiteMap<V> {
    /// The map of `entries` in write order, each replacing any earlier
    /// value at its site.
    pub(super) fn from_entries(mut entries: Vec<(Key, V)>) -> Self {
        resolve(
            &mut entries,
            |&(site, _)| site,
            |_| true,
            |kept, later| kept.1 = later.1,
        );
        Self::sorted(entries)
    }

    /// The map of `writes` in write order: each replaces any earlier value
    /// at its site or keeps it, as its `Write` says.
    pub(super) fn from_writes(mut writes: Vec<(Key, V, Write)>) -> Self {
        resolve(
            &mut writes,
            |&(site, _, _)| site,
            |&(_, _, write)| write == Write::Replace,
            |kept, later| kept.1 = later.1,
        );
        Self::sorted(
            writes
                .into_iter()
                .map(|(site, value, _)| (site, value))
                .collect(),
        )
    }

    /// Entries sorted by `key`, one per site.
    fn sorted(entries: Vec<(Key, V)>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(site, value)| (CompactSite::pack(site), value.pack()))
                .collect(),
        }
    }

    pub(super) fn get(&self, site: Key) -> Option<V> {
        self.entries
            .binary_search_by_key(&key(site), |&(site, _)| site.key())
            .ok()
            .map(|index| V::unpack(self.entries[index].1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Value(usize);

    impl PackedValue for Value {
        type Packed = u32;

        fn pack(self) -> Self::Packed {
            u32::try_from(self.0).expect("a test value fits u32")
        }

        fn unpack(value: Self::Packed) -> Self {
            Self(value as usize)
        }
    }

    #[test]
    fn frozen_sites_preserve_last_write_and_exact_syntax_identity() {
        let identifier = (SyntaxKind::Identifier, Span { start: 2, end: 5 });
        let same_span = (SyntaxKind::StringLiteral, identifier.1);
        let outer = (SyntaxKind::Identifier, Span { start: 2, end: 6 });
        let map = SiteMap::from_entries(vec![
            (outer, Value(1)),
            (identifier, Value(2)),
            (same_span, Value(3)),
            (identifier, Value(4)),
            (identifier, Value(5)),
        ]);
        assert_eq!(map.get(identifier), Some(Value(5)));
        assert_eq!(map.get(same_span), Some(Value(3)));
        assert_eq!(map.get(outer), Some(Value(1)));
        assert_eq!(map.get((SyntaxKind::NumericLiteral, identifier.1)), None);
        assert_eq!(map.get((identifier.0, Span { start: 1, end: 5 })), None);
    }

    #[test]
    fn sites_and_values_up_to_u32_max_are_not_truncated() {
        for offset in [u32::MAX as usize - 1, u32::MAX as usize] {
            let site = (
                SyntaxKind::Identifier,
                Span {
                    start: offset,
                    end: offset,
                },
            );
            let map = SiteMap::from_entries(vec![(site, Value(offset))]);
            assert_eq!(map.get(site), Some(Value(offset)));
            assert_eq!(map.get((site.0, Span { start: 0, end: 0 })), None);
        }
    }
}
