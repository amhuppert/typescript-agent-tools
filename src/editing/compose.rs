//! Source change composition: the changes every edit to one captured text
//! makes, in that text's coordinates, checked for conflicts and applied in
//! one deterministic order, with the map between the original and the
//! result. It knows nothing of syntax or files.

use crate::Span;

/// One change to a captured text: `span`, in its byte offsets, becomes
/// `text`. An insertion has an empty span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Change {
    pub(super) span: Span,
    pub(super) text: String,
    /// The request the change belongs to.
    pub(super) edit: usize,
}

/// Two requests whose changes cannot be composed without choosing an order
/// or a winner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Conflict {
    pub(super) first: usize,
    pub(super) second: usize,
    pub(super) at: Span,
    pub(super) reason: &'static str,
}

/// A text with its changes made, and how it relates to the original.
#[derive(Debug)]
pub(super) struct Composition {
    text: String,
    /// The changes in original order, each with where its text lies in the
    /// result.
    regions: Vec<Region>,
}

#[derive(Debug, Clone)]
pub(super) struct Region {
    pub(super) change: Change,
    /// Where the change's text lies in the composed text.
    pub(super) composed: Span,
}

/// Which side of an insertion at an offset a position belongs to: the end
/// of what precedes it, or the start of what follows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Before,
    After,
}

impl Composition {
    /// Compose `changes` to `original`. Overlapping or nested changes,
    /// duplicates, two insertions at one offset, and an insertion at the
    /// start of a nonempty replacement conflict: each would need an ordering
    /// rule this delivery does not define. An insertion at the end of a
    /// replacement follows it, as an insertion after a statement follows a
    /// body that ends the statement; adjacent nonempty replacements compose.
    /// Every span lies on character boundaries of `original`.
    pub(super) fn new(original: &str, mut changes: Vec<Change>) -> Result<Self, Conflict> {
        changes.sort_by_key(|change| (change.span.start, change.span.end, change.edit));
        for pair in changes.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let conflict = |reason| Conflict {
                first: a.edit.min(b.edit),
                second: a.edit.max(b.edit),
                at: b.span,
                reason,
            };
            let empty = |change: &Change| change.span.start == change.span.end;
            if b.span.start < a.span.end {
                return Err(conflict("their changes overlap"));
            }
            if empty(a) && empty(b) && a.span.start == b.span.start {
                return Err(conflict("both insert at the same place"));
            }
            if b.span.start == a.span.end && empty(a) && !empty(b) {
                return Err(conflict(
                    "one inserts at the start of the other's replacement",
                ));
            }
        }
        let mut text = String::with_capacity(original.len());
        let mut regions = Vec::with_capacity(changes.len());
        let mut at = 0;
        for change in changes {
            debug_assert!(
                original.is_char_boundary(change.span.start)
                    && original.is_char_boundary(change.span.end)
            );
            text.push_str(&original[at..change.span.start]);
            let start = text.len();
            text.push_str(&change.text);
            at = change.span.end;
            regions.push(Region {
                composed: Span {
                    start,
                    end: text.len(),
                },
                change,
            });
        }
        text.push_str(&original[at..]);
        Ok(Self { text, regions })
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn regions(&self) -> &[Region] {
        &self.regions
    }

    /// Where `change`, one of the composed changes, lies in the composed
    /// text.
    pub(super) fn region_of(&self, change: &Change) -> &Region {
        self.regions
            .iter()
            .find(|region| region.change.span == change.span && region.change.edit == change.edit)
            .expect("a composed change")
    }

    /// Where an original offset that no change replaces lies in the
    /// composed text: past an insertion at it for `Side::After`, before it
    /// for `Side::Before`. The end of a replaced span maps to the end of its
    /// text and its start to the start of its text; an offset strictly
    /// inside one maps nowhere.
    pub(super) fn map(&self, offset: usize, side: Side) -> Option<usize> {
        let mut shift: isize = 0;
        for region in &self.regions {
            let span = region.change.span;
            let delta = region.change.text.len() as isize - (span.end - span.start) as isize;
            if span.end < offset
                || span.end == offset && span.start < span.end
                || span.start == offset && span.end == offset && side == Side::After
            {
                shift += delta;
            } else if span.start < offset {
                return None;
            } else {
                break;
            }
        }
        Some((offset as isize + shift) as usize)
    }

    /// The original offset of a composed offset outside every change's
    /// text, or the change whose text holds it.
    pub(super) fn origin(&self, offset: usize) -> Origin<'_> {
        let mut shift: isize = 0;
        for region in &self.regions {
            if offset < region.composed.start {
                break;
            }
            if offset < region.composed.end
                || offset == region.composed.end && region.change.text.is_empty()
            {
                return Origin::Changed { region };
            }
            let span = region.change.span;
            shift += region.change.text.len() as isize - (span.end - span.start) as isize;
        }
        Origin::Preserved((offset as isize - shift) as usize)
    }
}

/// Where a composed offset came from.
#[derive(Debug, Clone, Copy)]
pub(super) enum Origin<'a> {
    Preserved(usize),
    Changed { region: &'a Region },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(start: usize, end: usize, text: &str, edit: usize) -> Change {
        Change {
            span: Span { start, end },
            text: text.into(),
            edit,
        }
    }

    #[test]
    fn composes_in_original_order_and_maps_both_sides_of_an_insertion() {
        let composed =
            Composition::new("abcdef", vec![change(4, 4, "XY", 1), change(1, 3, "z", 0)]).unwrap();
        assert_eq!(composed.text(), "azdXYef");
        assert_eq!(composed.map(0, Side::After), Some(0));
        assert_eq!(composed.map(1, Side::After), Some(1));
        assert_eq!(composed.map(2, Side::After), None);
        assert_eq!(composed.map(3, Side::Before), Some(2));
        assert_eq!(composed.map(4, Side::Before), Some(3));
        assert_eq!(composed.map(4, Side::After), Some(5));
        assert_eq!(composed.map(6, Side::After), Some(7));
        assert!(matches!(composed.origin(4), Origin::Changed { .. }));
        assert!(matches!(composed.origin(5), Origin::Preserved(4)));
        assert!(matches!(composed.origin(2), Origin::Preserved(3)));
    }

    #[test]
    fn conflicts_need_an_order_and_adjacent_replacements_do_not() {
        let conflict = |changes| Composition::new("abcdef", changes).unwrap_err().reason;
        assert_eq!(
            conflict(vec![change(1, 4, "", 0), change(3, 5, "", 1)]),
            "their changes overlap"
        );
        assert_eq!(
            conflict(vec![change(1, 4, "", 0), change(1, 4, "", 1)]),
            "their changes overlap"
        );
        assert_eq!(
            conflict(vec![change(2, 2, "a", 0), change(2, 2, "b", 1)]),
            "both insert at the same place"
        );
        assert_eq!(
            conflict(vec![change(1, 4, "x", 0), change(1, 1, "b", 1)]),
            "one inserts at the start of the other's replacement"
        );
        // An insertion at a replacement's end follows it.
        let composed =
            Composition::new("abcdef", vec![change(4, 4, "b", 1), change(1, 4, "x", 0)]).unwrap();
        assert_eq!(composed.text(), "axbef");
        let composed =
            Composition::new("abcdef", vec![change(1, 3, "X", 0), change(3, 5, "Y", 1)]).unwrap();
        assert_eq!(composed.text(), "aXYf");
    }
}
