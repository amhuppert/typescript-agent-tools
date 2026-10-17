//! Native tuple target facts shared by factories and their consumers.

use super::DemandError;
use crate::project::binding::NativeNodeIdentity;
use std::cmp::Ordering;
use std::ops::BitOr;
use std::sync::Arc;

/// TypeScript `ElementFlags`; syntax classification and normalization use
/// these same bits, while target identity retains the actual label node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) struct TupleElementFlags(u8);

impl TupleElementFlags {
    pub(in crate::project::semantic) const NONE: Self = Self(0);
    pub(in crate::project::semantic) const REQUIRED: Self = Self(1);
    pub(in crate::project::semantic) const OPTIONAL: Self = Self(2);
    pub(in crate::project::semantic) const REST: Self = Self(4);
    pub(in crate::project::semantic) const VARIADIC: Self = Self(8);
    pub(in crate::project::semantic) const FIXED: Self = Self(1 | 2);
    pub(in crate::project::semantic) const VARIABLE: Self = Self(4 | 8);
    pub(in crate::project::semantic) const NON_REQUIRED: Self = Self(2 | 4 | 8);
    pub(in crate::project::semantic) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl BitOr for TupleElementFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// The label's node identity participates in getTupleKey, whereas its exact
/// name is acquired only by compareElementLabels or tuple serialization.
#[derive(Clone)]
pub(in crate::project::semantic) struct TupleElementLabel {
    pub(in crate::project::semantic) declaration: NativeNodeIdentity,
    pub(in crate::project::semantic) name: Result<Arc<str>, DemandError>,
}

#[derive(Clone)]
pub(in crate::project::semantic) struct TupleElementInfo {
    pub(in crate::project::semantic) flags: TupleElementFlags,
    pub(in crate::project::semantic) label: Option<TupleElementLabel>,
}

/// `createTupleTargetType`'s complete established target metadata. None of
/// these observations acquires a reference's deferred element arguments.
#[derive(Clone)]
pub(in crate::project::semantic) struct TupleTarget {
    pub(in crate::project::semantic) elements: Arc<[TupleElementInfo]>,
    pub(in crate::project::semantic) minimum_length: usize,
    pub(in crate::project::semantic) fixed_length: usize,
    pub(in crate::project::semantic) combined_flags: TupleElementFlags,
    pub(in crate::project::semantic) readonly: bool,
}

impl TupleTarget {
    /// Native `getStartElementCount` visits through the first nonmatching flag.
    pub(in crate::project::semantic) fn start_element_count(
        &self,
        flags: TupleElementFlags,
    ) -> usize {
        self.matching_edge(self.elements.iter(), flags)
    }

    /// Native `getEndElementCount` asks the same question from the final element.
    pub(in crate::project::semantic) fn end_element_count(
        &self,
        flags: TupleElementFlags,
    ) -> usize {
        self.matching_edge(self.elements.iter().rev(), flags)
    }

    fn matching_edge<'a>(
        &self,
        elements: impl Iterator<Item = &'a TupleElementInfo>,
        flags: TupleElementFlags,
    ) -> usize {
        elements
            .take_while(|element| element.flags.intersects(flags))
            .count()
    }

    /// Native `getTotalFixedElementCount` includes the suffix after a rest.
    pub(in crate::project::semantic) fn total_fixed_element_count(&self) -> usize {
        self.fixed_length + self.end_element_count(TupleElementFlags::FIXED)
    }
}

/// `compareTupleTypes` / `compareElementLabels`: target identity uses actual
/// label nodes, while ordering compares their names only after all flag bits.
pub(super) fn compare_tuple_targets(
    left: &TupleTarget,
    right: &TupleTarget,
) -> Result<Ordering, DemandError> {
    let order = left
        .readonly
        .cmp(&right.readonly)
        .then_with(|| left.elements.len().cmp(&right.elements.len()));
    if order != Ordering::Equal {
        return Ok(order);
    }
    for (left, right) in left.elements.iter().zip(right.elements.iter()) {
        let order = left.flags.0.cmp(&right.flags.0);
        if order != Ordering::Equal {
            return Ok(order);
        }
    }
    for (left, right) in left.elements.iter().zip(right.elements.iter()) {
        let order = match (&left.label, &right.label) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(left), Some(right)) if left.declaration == right.declaration => Ordering::Equal,
            (Some(left), Some(right)) => left
                .name
                .as_ref()
                .map_err(Clone::clone)?
                .cmp(right.name.as_ref().map_err(Clone::clone)?),
        };
        if order != Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}
