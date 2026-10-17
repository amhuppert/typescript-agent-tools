//! Native indexed-access identity stores established operands without resolving
//! them. Incoming access flags key the cache; only Persistent flags survive on
//! the actual operator, as in getIndexedAccessTypeOrUndefined.

use super::keys::{KeyBuilder, same_alias};
use super::{Alias, AliasKey, Model, Type, TypeId};
use crate::resources::CheckedKey;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) struct AccessFlags(u32);

impl AccessFlags {
    pub(in crate::project::semantic) const NONE: Self = Self(0);
    pub(in crate::project::semantic) const INCLUDE_UNDEFINED: Self = Self(1 << 0);
    pub(in crate::project::semantic) const NO_INDEX_SIGNATURES: Self = Self(1 << 1);
    pub(in crate::project::semantic) const WRITING: Self = Self(1 << 2);
    pub(in crate::project::semantic) const CACHE_SYMBOL: Self = Self(1 << 3);
    pub(in crate::project::semantic) const ALLOW_MISSING: Self = Self(1 << 4);
    pub(in crate::project::semantic) const EXPRESSION_POSITION: Self = Self(1 << 5);
    pub(in crate::project::semantic) const REPORT_DEPRECATED: Self = Self(1 << 6);
    pub(in crate::project::semantic) const SUPPRESS_NO_IMPLICIT_ANY: Self = Self(1 << 7);
    pub(in crate::project::semantic) const CONTEXTUAL: Self = Self(1 << 8);

    pub(in crate::project::semantic) fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }

    fn persistent(self) -> Self {
        Self(self.0 & Self::INCLUDE_UNDEFINED.0)
    }
}

impl std::ops::BitOr for AccessFlags {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl std::ops::BitOrAssign for AccessFlags {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

#[derive(Clone, Copy)]
pub(in crate::project::semantic) struct IndexedAccessView {
    pub(in crate::project::semantic) object_type: TypeId,
    pub(in crate::project::semantic) index_type: TypeId,
    pub(in crate::project::semantic) access_flags: AccessFlags,
}

pub(super) struct IndexedAccessKey {
    object_type: TypeId,
    index_type: TypeId,
    access_flags: AccessFlags,
    alias: Option<AliasKey>,
    fingerprint: u64,
}

impl IndexedAccessKey {
    /// `getIndexedAccessKey`: actual operands, all incoming access flags and
    /// the complete alias identity participate, even when stored flags differ.
    fn new(
        object_type: TypeId,
        index_type: TypeId,
        access_flags: AccessFlags,
        alias: Option<&Alias>,
    ) -> Self {
        let alias = AliasKey::of(alias);
        let mut hash = KeyBuilder::new();
        hash.fixed(&object_type);
        hash.fixed(&index_type);
        hash.fixed(&access_flags);
        hash.alias(alias.as_ref());
        Self {
            object_type,
            index_type,
            access_flags,
            alias,
            fingerprint: hash.finish(),
        }
    }
}

impl CheckedKey for IndexedAccessKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn equivalent(&self, other: &Self) -> bool {
        self.object_type == other.object_type
            && self.index_type == other.index_type
            && self.access_flags == other.access_flags
            && same_alias(self.alias.as_ref(), other.alias.as_ref())
    }
}

impl Model {
    /// `newIndexedAccessType` / `getIndexedAccessKey`: inputs are established
    /// types. The evaluator has already selected deferral and adjusted flags.
    pub(in crate::project::semantic) fn indexed_access_type(
        &mut self,
        object_type: TypeId,
        index_type: TypeId,
        access_flags: AccessFlags,
        alias: Option<Arc<Alias>>,
    ) -> TypeId {
        let key = IndexedAccessKey::new(object_type, index_type, access_flags, alias.as_deref());
        if let Some(&id) = self.indexed_accesses.get(&key) {
            return id;
        }
        let id = self.push(Type::IndexedAccess {
            data: IndexedAccessView {
                object_type,
                index_type,
                access_flags: access_flags.persistent(),
            },
            alias,
        });
        self.indexed_accesses.insert(key, id);
        id
    }
}
