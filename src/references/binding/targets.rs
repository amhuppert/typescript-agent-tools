//! Resolved meanings and their access through aliases. A name can combine
//! independent targets; a type-only route restricts only the target it adds.

use super::{SymbolId, meaning};

/// Each single meaning, in slot order.
const EACH: [u8; 3] = [meaning::VALUE, meaning::TYPE, meaning::NAMESPACE];

/// The single meanings in `mask`.
pub(super) fn each(mask: u8) -> impl Iterator<Item = u8> {
    EACH.into_iter().filter(move |bit| mask & bit != 0)
}

fn slot(meaning: u8) -> usize {
    debug_assert!(EACH.contains(&meaning), "one meaning");
    meaning.trailing_zeros() as usize
}

/// At most one symbol per meaning: the declarations a table entry holds, or
/// the targets a name resolves to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Slots([Option<SymbolId>; 3]);

impl Slots {
    /// `symbol` in every meaning of `mask`.
    pub(super) fn only(mask: u8, symbol: SymbolId) -> Self {
        let mut slots = Self::default();
        for meaning in each(mask) {
            slots.set(meaning, symbol);
        }
        slots
    }

    /// The symbol of one meaning.
    pub(super) fn get(self, meaning: u8) -> Option<SymbolId> {
        self.0[slot(meaning)]
    }

    pub(super) fn set(&mut self, meaning: u8, symbol: SymbolId) {
        self.0[slot(meaning)] = Some(symbol);
    }

    pub(super) fn value(self) -> Option<SymbolId> {
        self.get(meaning::VALUE)
    }

    /// The symbol whose exports a qualified name or `export =` reaches: the
    /// namespace target, or otherwise the value.
    pub(super) fn container(self) -> Option<SymbolId> {
        self.get(meaning::NAMESPACE).or(self.value())
    }

    pub(super) fn is_empty(self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    /// The symbols of occupied meanings, in slot order. A symbol with several
    /// meanings repeats.
    pub(super) fn symbols(self) -> impl Iterator<Item = SymbolId> {
        self.0.into_iter().flatten()
    }

    pub(super) fn map(self, mut f: impl FnMut(SymbolId) -> SymbolId) -> Self {
        Self(self.0.map(|symbol| symbol.map(&mut f)))
    }
}

/// How a member is reached through the container that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Reach {
    /// An export of a module whose `export =` target is the container.
    ExportAssignment,
    /// The member of a qualified name, such as `ns.member`.
    Qualified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Meanings {
    targets: Slots,
    /// Type-only provenance per occupied meaning, retained for re-exports and
    /// type queries. Only a runtime value selection rejects this access.
    restricted: u8,
}

impl Meanings {
    /// Targets reached without a type-only route.
    pub(super) fn new(targets: Slots) -> Self {
        Self {
            targets,
            restricted: 0,
        }
    }

    pub(super) fn targets(self) -> Slots {
        self.targets
    }

    pub(super) fn container(self) -> Option<SymbolId> {
        self.targets.container()
    }

    pub(super) fn is_empty(self) -> bool {
        self.targets.is_empty()
    }

    pub(super) fn needs(self, mask: u8) -> bool {
        each(mask).any(|meaning| self.targets.get(meaning).is_none())
    }

    fn clear(&mut self, meaning: u8) {
        self.targets.0[slot(meaning)] = None;
        self.restricted &= !meaning;
    }

    /// The meanings in `mask`. Only a runtime value selection, which needs
    /// nothing but a value outside a type, drops a type-only value. Any other
    /// selection keeps its identity and access for re-exports, aliases, and
    /// type queries to pass on.
    pub(super) fn select(mut self, mask: u8, type_context: bool) -> Self {
        let runtime_value = mask == meaning::VALUE && !type_context;
        for meaning in EACH {
            if mask & meaning == 0 || runtime_value && self.restricted & meaning != 0 {
                self.clear(meaning);
            }
        }
        self
    }

    pub(super) fn without_value(mut self) -> Self {
        self.clear(meaning::VALUE);
        self
    }

    /// Fill absent meanings from an alias without changing existing locals.
    pub(super) fn fill_missing(&mut self, alias: Self, mask: u8, type_context: bool) {
        let alias = alias.select(mask, type_context);
        for meaning in EACH {
            if let (None, Some(symbol)) = (self.targets.get(meaning), alias.targets.get(meaning)) {
                self.targets.set(meaning, symbol);
                self.restricted |= alias.restricted & meaning;
            }
        }
    }

    /// Mark every target as reached through a type-only route.
    pub(super) fn restrict(mut self, type_only: bool) -> Self {
        if type_only {
            for meaning in EACH {
                if self.targets.get(meaning).is_some() {
                    self.restricted |= meaning;
                }
            }
        }
        self
    }

    /// Whether a member reached through this container inherits the
    /// container's type-only access. The exports a module exposes through
    /// `export =` do, as through any alias: TypeScript rejects a value
    /// imported from a module whose `export =` names a type-only import. A
    /// qualified name's member keeps only its own access: TypeScript resolves
    /// `new x()` after `import x = ns.C` with a type-only `ns`, and a value
    /// path's qualifier, selected as a runtime value, has already rejected a
    /// type-only one.
    pub(super) fn members_type_only(self, reach: Reach) -> bool {
        match reach {
            Reach::ExportAssignment => {
                let container = if self.targets.get(meaning::NAMESPACE).is_some() {
                    meaning::NAMESPACE
                } else {
                    meaning::VALUE
                };
                self.restricted & container != 0
            }
            Reach::Qualified => false,
        }
    }

    /// Alternative routes to the same targets grant the less restricted
    /// access independently for each meaning.
    pub(super) fn merge_routes(&mut self, other: Self) {
        debug_assert_eq!(self.targets, other.targets);
        self.restricted &= other.restricted;
    }

    /// Fill absent meanings from every meaning each target has, with the
    /// access of the meaning it was found in.
    pub(super) fn expanded(mut self, meanings: impl Fn(SymbolId) -> u8) -> Self {
        let found = self;
        for from in EACH {
            let Some(symbol) = found.targets.get(from) else {
                continue;
            };
            for meaning in each(meanings(symbol)) {
                if self.targets.get(meaning).is_none() {
                    self.targets.set(meaning, symbol);
                    if found.restricted & from != 0 {
                        self.restricted |= meaning;
                    }
                }
            }
        }
        self
    }
}
