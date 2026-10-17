//! Named inventory order is a semantic operation over actual symbol headers;
//! binding retains tables without assigning request-local identity order.

use super::{Evaluator, structure::MemberEntry};
use crate::project::binding::{BoundSymbol, try_sort_by};
use crate::project::semantic::model::{DemandError, NativeResult, NativeSequence, Property};
use crate::references::Location;

impl Evaluator<'_, '_> {
    /// `getNamedMembers` partitions Class/Interface ValueDeclarations before
    /// inherited symbols, then sorts each partition with the shared native
    /// comparator. Actual source containment intentionally ignores file identity.
    pub(super) fn named_member_properties(
        &mut self,
        properties: Vec<Property>,
        container: Option<BoundSymbol>,
        consumer: &Location,
    ) -> Result<Vec<Property>, DemandError> {
        self.named_member_entries(
            properties.into_iter().map(MemberEntry::Projected).collect(),
            container,
            consumer,
        )
    }

    pub(super) fn named_member_entries(
        &mut self,
        entries: Vec<MemberEntry>,
        container: Option<BoundSymbol>,
        consumer: &Location,
    ) -> Result<Vec<Property>, DemandError> {
        self.named_member_entries_completion(entries, container, consumer)?
            .admit()
    }

    pub(super) fn named_member_entries_completion(
        &mut self,
        entries: Vec<MemberEntry>,
        container: Option<BoundSymbol>,
        consumer: &Location,
    ) -> NativeResult<Vec<Property>> {
        let mut sequence = NativeSequence::new();
        let result = (|| -> Result<Vec<Property>, DemandError> {
            if entries.is_empty() {
                return Ok(Vec::new());
            }
            let partition = if let Some(container) = container {
                let flags = self
                    .source
                    .bindings()
                    .type_symbol_flags(container)
                    .map_err(|error| self.generic_binding_failure(error, consumer))?;
                (flags.class || flags.interface).then_some(container)
            } else {
                None
            };
            let mut contained = Vec::new();
            let mut inherited = Vec::new();
            let passes: &[bool] = if partition.is_some() {
                &[true, false]
            } else {
                &[false]
            };
            for &want_contained in passes {
                for entry in &entries {
                    let (symbol, key) = match entry {
                        MemberEntry::Projected(property) => {
                            let symbol = property.symbol.ok_or_else(|| {
                                super::super::internal("named native property lacks its symbol")
                            })?;
                            (
                                symbol,
                                self.reference_symbol_key(
                                    super::ReferenceSymbol::from_property(symbol),
                                    consumer,
                                )?,
                            )
                        }
                        MemberEntry::Symbol { key, symbol } => (*symbol, key.clone()),
                    };
                    if !sequence.read(self.reference_is_named_member_completion(
                        super::ReferenceSymbol::from_property(symbol),
                        &key,
                        consumer,
                    ))? {
                        continue;
                    }
                    let (own, header) =
                        self.property_ordering_facts(symbol, partition, consumer)?;
                    if own != want_contained {
                        continue;
                    }
                    let item = (header, entry.clone());
                    if own {
                        contained.push(item);
                    } else {
                        inherited.push(item);
                    }
                }
            }
            for entries in [&mut contained, &mut inherited] {
                try_sort_by(entries, &|left, right| {
                    self.model
                        .compare_native_symbols(Some(&left.0), Some(&right.0))
                        .map_err(|error| self.generic_binding_failure(error, consumer))
                })?;
            }
            contained.extend(inherited);
            contained
                .into_iter()
                .map(|(_, entry)| match entry {
                    MemberEntry::Projected(property) => Ok(property),
                    MemberEntry::Symbol { symbol, .. } => {
                        self.project_property_symbol(symbol, consumer)
                    }
                })
                .collect()
        })();
        sequence.finish(result)
    }
}
