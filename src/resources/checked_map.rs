//! Immutable native keys: fingerprints select buckets; exact equality decides identity.
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, DefaultHasher};

/// Equivalent keys must have equal fingerprints. Exact comparison is decisive.
pub(crate) trait CheckedKey {
    fn fingerprint(&self) -> u64;
    fn equivalent(&self, other: &Self) -> bool;
}

type Bucket<K, V> = smallvec::SmallVec<[(K, V); 1]>;

/// Only fixed-size fingerprints enter the standard table, so resizing cannot
/// rehash retained key payloads. A lone entry is inline; deliberate collisions
/// retain exact identity and storage. Bucket order is private.
pub(crate) struct CheckedMap<K, V> {
    buckets: HashMap<u64, Bucket<K, V>, BuildHasherDefault<DefaultHasher>>,
    length: usize,
}

impl<K: CheckedKey, V> CheckedMap<K, V> {
    pub(crate) fn new() -> Self {
        Self {
            buckets: HashMap::default(),
            length: 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.length
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.get_by(key.fingerprint(), |candidate| candidate.equivalent(key))
    }

    /// Probe a borrowed key without allocating retained payload on a hit.
    /// The key owner supplies the same fingerprint and exact comparison
    /// as its stored key. Collision traversal and capacity remain table-owned.
    pub(crate) fn get_by(
        &self,
        fingerprint: u64,
        mut equivalent: impl FnMut(&K) -> bool,
    ) -> Option<&V> {
        self.buckets
            .get(&fingerprint)?
            .iter()
            .find_map(|(candidate, value)| equivalent(candidate).then_some(value))
    }

    /// Membership uses the same exact comparison as lookup.
    pub(crate) fn contains(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> Option<V> {
        let fingerprint = key.fingerprint();
        if let Some(bucket) = self.buckets.get_mut(&fingerprint) {
            for (candidate, installed) in bucket.iter_mut() {
                if candidate.equivalent(&key) {
                    return Some(std::mem::replace(installed, value));
                }
            }
            bucket.push((key, value));
        } else {
            self.buckets.reserve(1);
            let mut bucket = Bucket::new();
            bucket.push((key, value));
            self.buckets.insert(fingerprint, bucket);
        }
        self.length += 1;
        None
    }

    /// Clear entries while retaining table capacity.
    pub(crate) fn clear(&mut self) {
        self.buckets.clear();
        self.length = 0;
    }

    pub(crate) fn remove(&mut self, key: &K) -> Option<V> {
        let bucket = self.buckets.get_mut(&key.fingerprint())?;
        let index = bucket
            .iter()
            .position(|(candidate, _)| candidate.equivalent(key))?;
        // Bucket order is private. Swap removal keeps movement bounded even
        // for deliberately colliding fingerprints, without changing key identity.
        let (_, value) = bucket.swap_remove(index);
        if bucket.is_empty() {
            self.buckets.remove(&key.fingerprint());
        }
        self.length -= 1;
        Some(value)
    }
}

impl<K: CheckedKey, V> Default for CheckedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
