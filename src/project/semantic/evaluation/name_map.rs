//! Request-local native-name tables. Canonical name policy stays in binding;
//! checked buckets own collision traversal and growth, and clones share bytes.
use crate::project::binding::NativePropertyKey;
use crate::project::semantic::model::KeyBuilder;
use crate::resources::{CheckedKey, CheckedMap, Control};

pub(super) struct NameMap<V> {
    entries: CheckedMap<NameKey, V>,
    control: Option<Control>,
}
struct NameKey {
    name: NativePropertyKey,
    fingerprint: u64,
}
impl NameKey {
    fn hash(name: &NativePropertyKey, _control: Option<&Control>) -> u64 {
        let mut hash = KeyBuilder::new();
        hash.native_string(name.string_value());
        hash.finish()
    }
    fn matches(&self, name: &NativePropertyKey) -> bool {
        self.name
            .string_value()
            .canonical_equal(name.string_value())
    }
}
impl CheckedKey for NameKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    fn equivalent(&self, other: &Self) -> bool {
        self.matches(&other.name)
    }
}
impl<V> NameMap<V> {
    /// Native SymbolTable / map[string] operations use the complete canonical
    /// name, including opaque identity, with no extra escaping or case policy.
    pub(super) fn new(control: Option<Control>) -> Self {
        Self {
            entries: CheckedMap::new(),
            control,
        }
    }
    pub(super) fn get(&self, name: &NativePropertyKey) -> Option<&V> {
        self.entries
            .get_by(NameKey::hash(name, self.control.as_ref()), |key| {
                key.matches(name)
            })
    }
    pub(super) fn insert(&mut self, name: NativePropertyKey, value: V) {
        let fingerprint = NameKey::hash(&name, self.control.as_ref());

        self.entries.insert(NameKey { name, fingerprint }, value);
    }
}
