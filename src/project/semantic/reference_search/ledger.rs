//! Request-local reference result storage. Native symbols establish group identity;
//! exact public Location identity establishes occurrence identity. No display key
//! replaces either. Checked maps own collisions; this owner owns retained payloads,
//! scalar buffers and the existing public output order.
use super::super::gaps::{Gaps, sequence_order};
use super::*;
use crate::references::metadata::{
    copy_location, copy_text, location_hash, location_order, text_order,
};
use crate::resources::{CheckedKey, CheckedMap, Control};
use std::cmp::Ordering;
use std::collections::HashMap;

pub(super) struct Ledger {
    identities: HashMap<NativeSymbolIdentity, usize>,
    groups: Vec<Group>,
    gaps: Gaps,
}
struct Group {
    declarations: Vec<NamedLocation>,
    positions: CheckedMap<OccurrenceKey, usize>,
    occurrences: Vec<Occurrence>,
}
struct OccurrenceKey {
    location: Location,
    fingerprint: u64,
}
impl CheckedKey for OccurrenceKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        location_order(&self.location, &other.location) == Ordering::Equal
    }
}
impl Ledger {
    pub(super) fn new(control: Option<Control>) -> Self {
        Self {
            identities: HashMap::new(),
            groups: Vec::new(),
            gaps: Gaps::new(control),
        }
    }
    /// Seed-only groups use precisely the same cache and backing as occurrence groups.
    pub(super) fn ensure_group(
        &mut self,
        identity: NativeSymbolIdentity,
        declarations: Vec<NamedLocation>,
    ) {
        if !self.identities.contains_key(&identity) {
            self.install_group(identity, declarations);
        }
    }
    fn install_group(
        &mut self,
        identity: NativeSymbolIdentity,
        declarations: Vec<NamedLocation>,
    ) -> usize {
        let index = self.groups.len();
        let group = Group {
            declarations,
            positions: CheckedMap::new(),
            occurrences: Vec::new(),
        };
        (self.groups).push(group);
        self.identities.insert(identity, index);
        index
    }
    pub(super) fn reference(
        &mut self,
        evaluator: &mut Evaluator<'_, '_>,
        symbol: ReferenceSymbol,
        site: &ReferenceSite,
    ) -> Result<(), DemandError> {
        let identity = evaluator.reference_symbol_identity(symbol, &site.selection)?;
        let index = if let Some(index) = self.identities.get(&identity) {
            *index
        } else {
            let declarations =
                evaluator.reference_symbol_named_declarations(symbol, &site.selection)?;
            self.install_group(identity, declarations)
        };

        let group = &mut self.groups[index];
        let occurrence_index = {
            let fingerprint = location_hash(&site.occurrence);
            let found = group
                .positions
                .get_by(fingerprint, |key| {
                    location_order(&key.location, &site.occurrence) == Ordering::Equal
                })
                .copied();
            if let Some(index) = found {
                index
            } else {
                let index = group.occurrences.len();
                let (key, occurrence) = {
                    (
                        OccurrenceKey {
                            location: copy_location(&site.occurrence),
                            fingerprint,
                        },
                        Occurrence {
                            name: copy_text(&site.key),
                            location: copy_location(&site.occurrence),
                            roles: Vec::new(),
                        },
                    )
                };
                (group.occurrences).push(occurrence);
                group.positions.insert(key, index);
                index
            }
        };
        let occurrence = &mut group.occurrences[occurrence_index];
        for role in references::role_list(site.roles) {
            (occurrence.roles).push(role);
        }
        // Four native role bits bound this list; duplicate occurrences add at most
        // another four before the immediately following sort/dedup.
        occurrence.roles.sort();
        occurrence.roles.dedup();
        Ok(())
    }
    pub(super) fn gap(
        &mut self,
        location: Location,
        failure: DemandError,
    ) -> Result<(), AnalysisError> {
        self.gaps.record(location, failure)
    }

    pub(super) fn finish(self) -> (Vec<ReferenceGroup>, Vec<ReferenceGap>) {
        let mut groups = Vec::new();
        for mut group in self.groups {
            (group.occurrences).sort_by(|a, b| location_order(&a.location, &b.location));
            (groups).push(ReferenceGroup {
                declarations: group.declarations,
                occurrences: group.occurrences,
            });
        }
        (groups).sort_by(|a, b| {
            sequence_order(&a.declarations, &b.declarations, |a, b| {
                location_order(&a.location, &b.location).then_with(|| text_order(&a.name, &b.name))
            })
            .then_with(|| {
                sequence_order(&a.occurrences, &b.occurrences, |a, b| {
                    location_order(&a.location, &b.location)
                })
            })
        });
        (groups, self.gaps.finish())
    }
}
