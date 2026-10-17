//! Native identity-list and alias-key operations used by type factory caches.
use super::{Alias, AliasKey, IntersectionFlags, IntersectionKey, TypeId, UnionIdentity, UnionKey};
use crate::resources::{CheckedKey, CheckedMap, Control};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

/// `keyBuilder.writeTypes` / `getTypeListKey`: ordered actual identities, never
/// structural equality or spelling. Each fixed-size hash operation is checked.
pub(in crate::project::semantic) struct KeyBuilder {
    hash: DefaultHasher,
}

impl KeyBuilder {
    pub(in crate::project::semantic) fn new() -> Self {
        Self {
            hash: DefaultHasher::new(),
        }
    }

    // Only fixed-size identities/tags enter here; variable lists use `types`.
    pub(in crate::project::semantic) fn fixed<T: Hash>(&mut self, value: &T) {
        value.hash(&mut self.hash);
    }

    pub(in crate::project::semantic) fn types(&mut self, types: &[TypeId]) {
        self.fixed(&types.len());
        for ty in types {
            self.fixed(ty);
        }
    }

    /// `keyBuilder.writeAlias`: actual symbol and ordered instantiated arguments.
    pub(in crate::project::semantic) fn alias(&mut self, alias: Option<&AliasKey>) {
        self.fixed(&alias.map(|alias| alias.symbol));
        if let Some(alias) = alias {
            self.types(&alias.arguments);
        }
    }

    /// Combine an exact native name through its existing canonical owner.
    /// This preserves byte components and opaque identities without spelling them.
    pub(in crate::project::semantic) fn native_string(
        &mut self,
        value: &crate::project::binding::NativeStringValue,
    ) {
        value.fingerprint_into(&mut self.hash);
    }

    pub(in crate::project::semantic) fn finish(self) -> u64 {
        self.hash.finish()
    }
}

/// Exact collision check for `keyBuilder.writeTypes`. The fingerprint is only
/// a bucket selector; identity remains the complete ordered TypeId sequence.
pub(in crate::project::semantic) fn same_types(left: &[TypeId], right: &[TypeId]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    for (left, right) in left.iter().zip(right) {
        if left != right {
            return false;
        }
    }
    true
}

pub(in crate::project::semantic) fn same_alias(
    left: Option<&AliasKey>,
    right: Option<&AliasKey>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.symbol == right.symbol && same_types(&left.arguments, &right.arguments)
        }
        _ => false,
    }
}

impl IntersectionKey {
    /// `getIntersectionKey`: only NoConstraintReduction replaces the actual
    /// alias with `*`; NoSupertypeReduction does not partition the cache.
    pub(super) fn new(
        types: Arc<[TypeId]>,
        flags: IntersectionFlags,
        alias: Option<&Alias>,
    ) -> Self {
        let no_constraint_reduction = flags.contains(IntersectionFlags::NO_CONSTRAINT_REDUCTION);
        let alias = if no_constraint_reduction {
            None
        } else {
            AliasKey::of(alias)
        };
        let mut hash = KeyBuilder::new();
        hash.types(&types);
        hash.fixed(&no_constraint_reduction);
        hash.alias(alias.as_ref());
        Self {
            types,
            no_constraint_reduction,
            alias,
            fingerprint: hash.finish(),
        }
    }
}

impl CheckedKey for IntersectionKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn equivalent(&self, other: &Self) -> bool {
        self.no_constraint_reduction == other.no_constraint_reduction
            && same_types(&self.types, &other.types)
            && same_alias(self.alias.as_ref(), other.alias.as_ref())
    }
}

impl UnionKey {
    /// `getUnionKey`: ordinary constituents and the three origin domains have
    /// distinct markers. Index origins retain both their actual identity and
    /// the current alternatives. Every domain then appends the actual alias.
    pub(super) fn new(identity: UnionIdentity, alias: Option<&Alias>) -> Self {
        let alias = AliasKey::of(alias);
        let mut hash = KeyBuilder::new();
        match &identity {
            UnionIdentity::Types(types) => {
                hash.fixed(&0u8);
                hash.types(types);
            }
            UnionIdentity::UnionOrigin(types) => {
                hash.fixed(&1u8);
                hash.types(types);
            }
            UnionIdentity::IntersectionOrigin(types) => {
                hash.fixed(&2u8);
                hash.types(types);
            }
            UnionIdentity::IndexOrigin { origin, types } => {
                hash.fixed(&3u8);
                hash.fixed(origin);
                hash.types(types);
            }
        }
        hash.alias(alias.as_ref());
        Self {
            identity,
            alias,
            fingerprint: hash.finish(),
        }
    }
}

impl CheckedKey for UnionKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn equivalent(&self, other: &Self) -> bool {
        let same_identity = match (&self.identity, &other.identity) {
            (UnionIdentity::Types(left), UnionIdentity::Types(right))
            | (UnionIdentity::UnionOrigin(left), UnionIdentity::UnionOrigin(right))
            | (UnionIdentity::IntersectionOrigin(left), UnionIdentity::IntersectionOrigin(right)) => {
                same_types(left, right)
            }
            (
                UnionIdentity::IndexOrigin {
                    origin: left_origin,
                    types: left,
                },
                UnionIdentity::IndexOrigin {
                    origin: right_origin,
                    types: right,
                },
            ) => left_origin == right_origin && same_types(left, right),
            _ => false,
        };
        same_identity && same_alias(self.alias.as_ref(), other.alias.as_ref())
    }
}

/// `getAliasKey` for the native errorTypes cache. Other factory keys retain
/// their own operand domains and reuse the same alias serialization below.
pub(super) struct ErrorAliasKey {
    alias: AliasKey,
    fingerprint: u64,
}

impl ErrorAliasKey {
    pub(super) fn new(alias: &Alias) -> Self {
        let alias = AliasKey::of(Some(alias)).expect("present unresolved alias");
        let mut hash = KeyBuilder::new();
        hash.alias(Some(&alias));
        Self {
            alias,
            fingerprint: hash.finish(),
        }
    }
}

impl CheckedKey for ErrorAliasKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn equivalent(&self, other: &Self) -> bool {
        same_alias(Some(&self.alias), Some(&other.alias))
    }
}

/// A cache whose complete native question is `getTypeListKey`: an ordered list
/// of actual identities. It owns borrowed probing and retained key copying;
/// payload construction stays with the native operation that produces it.
pub(in crate::project::semantic) struct TypeListCache<V> {
    entries: CheckedMap<TypeListKey, V>,
    control: Option<Control>,
}

struct TypeListKey {
    types: Vec<TypeId>,
    fingerprint: u64,
}

impl TypeListKey {
    fn hash(types: &[TypeId], _control: Option<&Control>) -> u64 {
        let mut hash = KeyBuilder::new();
        hash.types(types);
        hash.finish()
    }
    fn matches(&self, types: &[TypeId]) -> bool {
        same_types(&self.types, types)
    }
}

impl CheckedKey for TypeListKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        self.matches(&other.types)
    }
}

impl<V> TypeListCache<V> {
    pub(in crate::project::semantic) fn new(control: Option<Control>) -> Self {
        Self {
            entries: CheckedMap::new(),
            control,
        }
    }
    pub(in crate::project::semantic) fn get(&self, types: &[TypeId]) -> Option<&V> {
        self.entries
            .get_by(TypeListKey::hash(types, self.control.as_ref()), |key| {
                key.matches(types)
            })
    }
    pub(in crate::project::semantic) fn insert(&mut self, types: &[TypeId], value: V) {
        let fingerprint = TypeListKey::hash(types, self.control.as_ref());
        let types = { types.to_vec() };
        self.entries
            .insert(TypeListKey { types, fingerprint }, value);
    }
}
impl<V> Default for TypeListCache<V> {
    fn default() -> Self {
        Self::new(None)
    }
}
