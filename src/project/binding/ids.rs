//! Distinct identities for the binder's private tables and captured facts.
//! Each link names the kind of record it refers to; indexing requires an
//! explicit conversion.

/// Each id stores its index plus one, so that an absent id (`Option`) takes
/// no more room than a present one; ids compare and debug-print by index.
macro_rules! ids {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub(crate) struct $name(std::num::NonZeroU32);
        impl $name {
            pub(crate) fn new(index: usize) -> Self {
                Self(
                    u32::try_from(index)
                        .ok()
                        .and_then(|index| index.checked_add(1))
                        .and_then(std::num::NonZeroU32::new)
                        .expect("semantic record count fits in u32"),
                )
            }
            pub(crate) fn index(self) -> usize { (self.0.get() - 1) as usize }
        }
        impl std::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.debug_tuple(stringify!($name)).field(&self.index()).finish()
            }
        }
    )+};
}

ids!(
    ScopeId,
    DeclId,
    ModuleDeclarationId,
    RefId,
    AliasId,
    NameId,
    RequestId,
    AmbientId,
    TableId,
    ModuleId,
    OccurrenceId,
    StringId,
    SymbolId,
    NativeHeaderId,
    NativeTableId,
    TypeParameterListId,
    TextId
);

impl TableId {
    pub(super) const fn new_const(index: u32) -> Self {
        match index.checked_add(1) {
            Some(stored) => match std::num::NonZeroU32::new(stored) {
                Some(stored) => Self(stored),
                None => unreachable!(),
            },
            None => panic!("semantic record count fits in u32"),
        }
    }
}

impl super::site_map::PackedValue for ScopeId {
    type Packed = u32;

    fn pack(self) -> Self::Packed {
        self.0.get()
    }

    fn unpack(value: Self::Packed) -> Self {
        Self(std::num::NonZeroU32::new(value).expect("a packed scope id is stored nonzero"))
    }
}
