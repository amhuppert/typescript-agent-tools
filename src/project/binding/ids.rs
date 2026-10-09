//! Distinct identities for the binder's private tables and captured facts.
//! Each link names the kind of record it refers to; indexing requires an
//! explicit conversion.

macro_rules! ids {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub(crate) struct $name(u32);
        impl $name {
            pub(crate) fn new(index: usize) -> Self {
                Self(u32::try_from(index).expect("semantic record count fits in u32"))
            }
            pub(crate) fn index(self) -> usize { self.0 as usize }
        }
    )+};
}

ids!(
    ScopeId,
    DeclId,
    RefId,
    AliasId,
    NameId,
    RequestId,
    AmbientId,
    TableId,
    ModuleId,
    OccurrenceId,
    StringId,
    SymbolId
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TypeParameterListId(pub(super) usize);

impl TableId {
    pub(super) const fn new_const(index: u32) -> Self {
        Self(index)
    }
}
