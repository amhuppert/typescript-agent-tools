//! Exact established reference metadata. No decoding or path acquisition.
use super::Location;
use std::cmp::Ordering;
use std::hash::{DefaultHasher, Hash, Hasher};
pub(crate) fn copy_text(text: &str) -> String {
    text.to_owned()
}
pub(crate) fn copy_location(location: &Location) -> Location {
    location.clone()
}
pub(crate) fn location_order(a: &Location, b: &Location) -> Ordering {
    a.cmp(b)
}
pub(crate) fn text_order(a: &str, b: &str) -> Ordering {
    a.cmp(b)
}
pub(crate) fn option_location_order(a: Option<&Location>, b: Option<&Location>) -> Ordering {
    a.cmp(&b)
}
pub(crate) fn location_hash(location: &Location) -> u64 {
    let mut hash = DefaultHasher::new();
    crate::resources::hash_bytes(location.file.as_bytes(), &mut hash);
    location.start.hash(&mut hash);
    location.end.hash(&mut hash);
    hash.finish()
}
