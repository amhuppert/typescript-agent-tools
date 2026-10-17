//! Native keyof identities distinguish fresh display origins from cached
//! generic index types. Neither factory resolves the target's members.
use super::{Model, Type, TypeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::project::semantic) struct IndexFlags(u8);
impl IndexFlags {
    pub(in crate::project::semantic) const NONE: Self = Self(0);
    pub(in crate::project::semantic) const STRINGS_ONLY: Self = Self(1);
    pub(in crate::project::semantic) const NO_INDEX_SIGNATURES: Self = Self(2);
    pub(in crate::project::semantic) const NO_REDUCIBLE_CHECK: Self = Self(4);
    pub(in crate::project::semantic) fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }
}
impl std::ops::BitOr for IndexFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Copy)]
pub(in crate::project::semantic) struct IndexView {
    pub(in crate::project::semantic) target: TypeId,
    pub(in crate::project::semantic) index_flags: IndexFlags,
}
impl Model {
    /// `newIndexType` always allocates an actual Index Type with no symbol.
    pub(in crate::project::semantic) fn new_index_type(
        &mut self,
        target: TypeId,
        index_flags: IndexFlags,
    ) -> TypeId {
        self.push(Type::Index(IndexView {
            target,
            index_flags,
        }))
    }

    /// `getIndexTypeForGenericType` keys only StringsOnly and persists only
    /// that bit; NoIndexSignatures/NoReducibleCheck govern the caller operation.
    pub(in crate::project::semantic) fn index_type_for_generic(
        &mut self,
        target: TypeId,
        flags: IndexFlags,
    ) -> TypeId {
        let strings = flags.contains(IndexFlags::STRINGS_ONLY);
        if let Some(&ty) = self.index_types.get(&(target, strings)) {
            return ty;
        }
        let ty = self.new_index_type(
            target,
            if strings {
                IndexFlags::STRINGS_ONLY
            } else {
                IndexFlags::NONE
            },
        );
        self.index_types.insert((target, strings), ty);
        ty
    }
}
