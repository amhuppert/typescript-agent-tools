//! Frozen exact-syntax-site lookup. Capture appends in binder order; freezing
//! retains the last attachment at an identical kind and span, as HashMap did.

use crate::Span;
use crate::syntax_kind::SyntaxKind;

type Key = (SyntaxKind, Span);

/// Lossless file-local packing. A value which cannot fit selects full-width
/// storage for the file, never a truncated value or a new input restriction.
pub(super) trait PackedValue: Copy {
    type Packed: Copy + std::fmt::Debug;

    fn pack(self) -> Option<Self::Packed>;
    fn unpack(value: Self::Packed) -> Self;
}

#[derive(Debug)]
pub(super) struct SiteMap<V: PackedValue> {
    entries: Entries<V>,
}

#[derive(Debug)]
enum Entries<V: PackedValue> {
    Compact(Box<[(CompactSite, V::Packed)]>),
    Wide(Box<[(Key, V)]>),
}

#[derive(Debug, Clone, Copy)]
struct CompactSite {
    start: u32,
    end: u32,
    kind: SyntaxKind,
}

/// A span's offsets as `u32`, where both fit: the one packing rule of
/// binding's frozen facts, whose owners keep full width where it fails.
pub(super) fn pack_span(span: Span) -> Option<[u32; 2]> {
    Some([
        u32::try_from(span.start).ok()?,
        u32::try_from(span.end).ok()?,
    ])
}

impl CompactSite {
    fn pack((kind, span): Key) -> Option<Self> {
        let [start, end] = pack_span(span)?;
        Some(Self { start, end, kind })
    }

    fn key(self) -> (usize, usize, u32) {
        (self.start as usize, self.end as usize, self.kind.number())
    }
}

fn key((kind, span): Key) -> (usize, usize, u32) {
    (span.start, span.end, kind.number())
}

impl<V: PackedValue> Default for SiteMap<V> {
    fn default() -> Self {
        Self {
            entries: Entries::Compact(Box::default()),
        }
    }
}

impl<V: PackedValue> SiteMap<V> {
    pub(super) fn from_entries(mut entries: Vec<(Key, V)>) -> Self {
        // Stable sorting preserves binder write order within each exact site.
        // dedup_by passes the later entry first, so replace the surviving
        // earlier value before removing the later slot.
        entries.sort_by_key(|&(site, _)| key(site));
        entries.dedup_by(|later, earlier| {
            if later.0 == earlier.0 {
                earlier.1 = later.1;
                true
            } else {
                false
            }
        });
        let compact = (|| {
            let mut compact = Vec::with_capacity(entries.len());
            for &(site, value) in &entries {
                compact.push((CompactSite::pack(site)?, value.pack()?));
            }
            Some(compact.into_boxed_slice())
        })();
        Self {
            entries: match compact {
                Some(entries) => Entries::Compact(entries),
                None => Entries::Wide(entries.into_boxed_slice()),
            },
        }
    }

    pub(super) fn get(&self, site: Key) -> Option<V> {
        let sought = key(site);
        match &self.entries {
            Entries::Compact(entries) => entries
                .binary_search_by_key(&sought, |&(site, _)| site.key())
                .ok()
                .map(|index| V::unpack(entries[index].1)),
            Entries::Wide(entries) => entries
                .binary_search_by_key(&sought, |&(site, _)| key(site))
                .ok()
                .map(|index| entries[index].1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Value(usize);

    impl PackedValue for Value {
        type Packed = u32;

        fn pack(self) -> Option<Self::Packed> {
            u32::try_from(self.0).ok()
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
    fn full_width_sites_and_values_are_not_truncated() {
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
        if let Some(wide) = (u32::MAX as usize).checked_add(1) {
            let low = (SyntaxKind::Identifier, Span { start: 0, end: 0 });
            let high = (
                low.0,
                Span {
                    start: wide,
                    end: wide,
                },
            );
            for entries in [
                vec![(low, Value(1)), (high, Value(2))],
                vec![(low, Value(wide)), (high, Value(2))],
                vec![(low, Value(wide))],
            ] {
                let expected = entries.clone();
                let map = SiteMap::from_entries(entries);
                for (site, value) in expected {
                    assert_eq!(map.get(site), Some(value));
                }
            }
        }
    }
}
