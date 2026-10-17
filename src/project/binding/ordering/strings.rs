//! Exact native string identity shared by property keys and literal values.
//! Byte runs are canonical; symbol-ID components never acquire a spelling or
//! an ordering rank merely because a string is constructed or compared equal.

use super::NativeSymbolIdentity;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Concrete spelling and invalid owner boundaries are different failures.
/// Callers choose the response without interpreting diagnostic text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::project) enum NativeStringError {
    IdentitySpelling,
    NonUnicodeBytes,
    InvalidBoundary(&'static str),
}

impl NativeStringError {
    pub(in crate::project) fn message(self) -> &'static str {
        match self {
            Self::IdentitySpelling => {
                "native string operation requires the opaque symbol identity's decimal spelling"
            }
            Self::NonUnicodeBytes => {
                "native string operation requires exact non-UTF-8 byte support"
            }
            Self::InvalidBoundary(message) => message,
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::project) enum NativeStringPart {
    Bytes(Arc<[u8]>),
    /// The native spelling is a positive decimal GetSymbolId. Its digits and
    /// byte length are deliberately unavailable under the identity policy.
    Identity(NativeSymbolIdentity),
}

#[derive(Debug, Clone, Hash)]
enum Repr {
    Bytes(Arc<[u8]>),
    Parts(Arc<[NativeStringPart]>),
}

#[derive(Debug, Clone)]
pub(in crate::project) struct NativeStringValue(Repr);

impl Hash for NativeStringPart {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Canonical equality compares the variant and its exact payload.
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Bytes(bytes) => bytes.hash(state),
            Self::Identity(identity) => identity.hash(state),
        }
    }
}

impl Hash for NativeStringValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for NativeStringPart {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_equal(other)
    }
}
impl Eq for NativeStringPart {}

impl PartialEq for NativeStringValue {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_equal(other)
    }
}
impl Eq for NativeStringValue {}

/// A boundary in one value, not a native string byte offset. Opaque atoms
/// occupy one internal slot solely to make whole-atom slices representable.
/// Only this owner creates positions; advancing native bytes cannot cross an
/// atom whose native length is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::project) struct NativeStringPosition(usize);

pub(in crate::project) enum NativeStringSegment<'a> {
    Bytes(&'a [u8]),
    Identity(NativeSymbolIdentity),
    End,
}

impl NativeStringPart {
    /// Equality of canonical representation parts, separate from string-value
    /// equality across different opaque-ID partitions.
    pub(in crate::project) fn canonical_equal(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::Identity(a), Self::Identity(b)) => a == b,
            (Self::Bytes(_), Self::Identity(_)) | (Self::Identity(_), Self::Bytes(_)) => false,
        }
    }
}

pub(in crate::project) trait NativeByteInput {
    fn into_native_bytes(self, control: Option<&crate::resources::Control>) -> Arc<[u8]>;
}
impl NativeByteInput for &[u8] {
    fn into_native_bytes(self, _control: Option<&crate::resources::Control>) -> Arc<[u8]> {
        std::sync::Arc::from(self)
    }
}
impl NativeByteInput for Vec<u8> {
    fn into_native_bytes(self, _control: Option<&crate::resources::Control>) -> Arc<[u8]> {
        std::sync::Arc::from(self)
    }
}
impl NativeByteInput for Arc<[u8]> {
    fn into_native_bytes(self, _control: Option<&crate::resources::Control>) -> Arc<[u8]> {
        self
    }
}

/// The existing from_parts coalescing decision, reusable by native factories
/// supplying borrowed byte runs. Formatting policy stays with those factories.
pub(super) struct NativeStringBuilder<'a> {
    parts: Vec<NativeStringPart>,
    bytes: Vec<u8>,
    control: Option<&'a crate::resources::Control>,
}
impl<'a> NativeStringBuilder<'a> {
    pub(super) fn new(control: Option<&'a crate::resources::Control>) -> Self {
        Self {
            parts: Vec::new(),
            bytes: Vec::new(),
            control,
        }
    }
    pub(super) fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    pub(super) fn identity(&mut self, identity: NativeSymbolIdentity) {
        self.flush();
        self.parts.push(NativeStringPart::Identity(identity));
    }
    pub(super) fn value(&mut self, value: &NativeStringValue) {
        for part in value.parts() {
            match part {
                NativeStringPart::Bytes(bytes) => self.bytes(&bytes),
                NativeStringPart::Identity(identity) => self.identity(identity),
            }
        }
    }
    fn flush(&mut self) {
        if !self.bytes.is_empty() {
            let bytes = std::sync::Arc::from(std::mem::take(&mut self.bytes));
            self.parts.push(NativeStringPart::Bytes(bytes));
        }
    }
    pub(super) fn finish(mut self) -> NativeStringValue {
        if self.parts.is_empty() {
            return NativeStringValue::from_bytes(std::sync::Arc::from(self.bytes), self.control);
        }
        self.flush();
        NativeStringValue(Repr::Parts(std::sync::Arc::from(self.parts)))
    }
}

impl NativeStringValue {
    /// Exact retained bytes: borrowed/owned inputs pay for a new payload;
    /// already shared input stays shared without an intermediate conversion.
    pub(in crate::project) fn from_bytes(
        bytes: impl NativeByteInput,
        control: Option<&crate::resources::Control>,
    ) -> Self {
        Self(Repr::Bytes(bytes.into_native_bytes(control)))
    }

    /// Concatenation retains exact native bytes and actual identity atoms.
    /// Coalescing is representational only; CombineSurrogatePairs and casing
    /// are separate semantic string operations and must not run here.
    pub(in crate::project) fn from_parts(
        parts: impl IntoIterator<Item = NativeStringPart>,
        control: Option<&crate::resources::Control>,
    ) -> Self {
        let mut result = NativeStringBuilder::new(control);
        for part in parts {
            match part {
                NativeStringPart::Bytes(bytes) => result.bytes(&bytes),
                NativeStringPart::Identity(identity) => result.identity(identity),
            }
        }
        result.finish()
    }

    /// Byte-run concatenation shares from_parts' canonical construction owner
    /// without requiring callers to allocate temporary Arc parts first.
    pub(in crate::project) fn from_byte_parts<'a>(
        parts: impl IntoIterator<Item = &'a [u8]>,
        control: Option<&crate::resources::Control>,
    ) -> Self {
        let mut result = NativeStringBuilder::new(control);
        for bytes in parts {
            result.bytes(bytes);
        }
        result.finish()
    }

    pub(in crate::project) fn canonical_equal(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Repr::Bytes(a), Repr::Bytes(b)) => a == b,
            (Repr::Parts(a), Repr::Parts(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.canonical_equal(b))
            }
            (Repr::Bytes(_), Repr::Parts(_)) | (Repr::Parts(_), Repr::Bytes(_)) => false,
        }
    }

    /// Candidate-filter hashing for request-owned literal caches. This does
    /// not assign a native spelling or ordering rank to opaque identity atoms;
    /// canonical_equal remains the decisive identity question on collisions.
    pub(in crate::project) fn fingerprint_into<H: Hasher>(&self, state: &mut H) {
        use crate::resources::hash_bytes;

        std::mem::discriminant(&self.0).hash(state);
        match &self.0 {
            Repr::Bytes(bytes) => hash_bytes(bytes, state),
            Repr::Parts(parts) => {
                parts.len().hash(state);
                for part in parts.iter() {
                    std::mem::discriminant(part).hash(state);
                    match part {
                        NativeStringPart::Bytes(bytes) => hash_bytes(bytes, state),
                        NativeStringPart::Identity(identity) => identity.hash(state),
                    }
                }
            }
        }
    }

    pub(in crate::project) fn parts(&self) -> impl Iterator<Item = NativeStringPart> + '_ {
        let (bytes, parts): (Option<&Arc<[u8]>>, &[NativeStringPart]) = match &self.0 {
            Repr::Bytes(bytes) => ((!bytes.is_empty()).then_some(bytes), &[]),
            Repr::Parts(parts) => (None, parts),
        };
        bytes
            .into_iter()
            .map(|bytes| NativeStringPart::Bytes(bytes.clone()))
            .chain(parts.iter().cloned())
    }

    pub(in crate::project) fn fixed_bytes(&self) -> Result<&[u8], NativeStringError> {
        self.fixed_part().ok_or(NativeStringError::IdentitySpelling)
    }

    /// A byte-only representation is an identity fact, independently from
    /// whether a caller's concrete-text operation admits the whole value.
    pub(super) fn fixed_part(&self) -> Option<&[u8]> {
        match &self.0 {
            Repr::Bytes(bytes) => Some(bytes),
            Repr::Parts(_) => None,
        }
    }

    /// Symbol.Name retains this exact fixed payload; publishing a header must
    /// not copy a byte run already shared by its canonical property key.
    pub(super) fn shared_fixed_part(&self) -> Option<Arc<[u8]>> {
        match &self.0 {
            Repr::Bytes(bytes) => Some(bytes.clone()),
            Repr::Parts(_) => None,
        }
    }

    pub(in crate::project) fn scalar_text(&self) -> Result<&str, NativeStringError> {
        std::str::from_utf8(self.fixed_bytes()?).map_err(|_| NativeStringError::NonUnicodeBytes)
    }

    pub(in crate::project) fn equals_text(&self, text: &str) -> bool {
        matches!(&self.0, Repr::Bytes(bytes) if bytes.as_ref() == text.as_bytes())
    }

    pub(in crate::project) fn is_empty(&self) -> bool {
        matches!(&self.0, Repr::Bytes(bytes) if bytes.is_empty())
    }

    pub(in crate::project) fn display_length(
        &self,
        _control: Option<&crate::resources::Control>,
    ) -> usize {
        match &self.0 {
            Repr::Bytes(bytes) => bytes.len(),
            Repr::Parts(parts) => parts
                .iter()
                .map(|part| match part {
                    NativeStringPart::Bytes(bytes) => bytes.len(),
                    NativeStringPart::Identity(_) => 1,
                })
                .sum(),
        }
    }

    /// An entire opaque ID is known to be positive decimal text. This fact
    /// does not reveal a digit, authorize splitting, or produce a number.
    pub(in crate::project) fn entire_identity(&self) -> Option<NativeSymbolIdentity> {
        match &self.0 {
            Repr::Parts(parts) => match parts.as_ref() {
                [NativeStringPart::Identity(identity)] => Some(*identity),
                _ => None,
            },
            Repr::Bytes(_) => None,
        }
    }

    pub(in crate::project) fn start(&self) -> NativeStringPosition {
        NativeStringPosition(0)
    }
    pub(in crate::project) fn end(
        &self,
        control: Option<&crate::resources::Control>,
    ) -> NativeStringPosition {
        NativeStringPosition(self.display_length(control))
    }

    pub(in crate::project) fn segment_at(
        &self,
        position: NativeStringPosition,
        _control: Option<&crate::resources::Control>,
    ) -> Result<NativeStringSegment<'_>, NativeStringError> {
        let mut offset = position.0;
        match &self.0 {
            Repr::Bytes(bytes) => {
                if offset < bytes.len() {
                    return Ok(NativeStringSegment::Bytes(&bytes[offset..]));
                }
                if offset == bytes.len() {
                    return Ok(NativeStringSegment::End);
                }
            }
            Repr::Parts(parts) => {
                for part in parts.iter() {
                    match part {
                        NativeStringPart::Bytes(bytes) => {
                            if offset < bytes.len() {
                                return Ok(NativeStringSegment::Bytes(&bytes[offset..]));
                            }
                            offset -= bytes.len();
                        }
                        NativeStringPart::Identity(identity) => {
                            if offset == 0 {
                                return Ok(NativeStringSegment::Identity(*identity));
                            }
                            offset -= 1;
                        }
                    }
                }
                if offset == 0 {
                    return Ok(NativeStringSegment::End);
                }
            }
        }
        Err(NativeStringError::InvalidBoundary(
            "native string slice position is outside its value",
        ))
    }

    pub(in crate::project) fn advance_bytes(
        &self,
        position: NativeStringPosition,
        count: usize,
        control: Option<&crate::resources::Control>,
    ) -> Result<NativeStringPosition, NativeStringError> {
        match self.segment_at(position, control)? {
            NativeStringSegment::Bytes(bytes) if count <= bytes.len() => {
                Ok(NativeStringPosition(position.0 + count))
            }
            _ if count == 0 => Ok(position),
            NativeStringSegment::Identity(_) => Err(NativeStringError::IdentitySpelling),
            _ => Err(NativeStringError::InvalidBoundary(
                "native string byte advancement exceeds its fixed byte run",
            )),
        }
    }

    /// Skip one established run or a whole opaque atom. This is not native
    /// rune advancement: callers must not use it to consume an ID's first digit.
    pub(in crate::project) fn after_segment(
        &self,
        position: NativeStringPosition,
        control: Option<&crate::resources::Control>,
    ) -> Result<NativeStringPosition, NativeStringError> {
        Ok(match self.segment_at(position, control)? {
            NativeStringSegment::Bytes(bytes) => NativeStringPosition(position.0 + bytes.len()),
            NativeStringSegment::Identity(_) => NativeStringPosition(position.0 + 1),
            NativeStringSegment::End => position,
        })
    }

    /// The run immediately before a boundary, truncated at that boundary.
    /// The returned position starts that run; an atom is returned whole.
    pub(in crate::project) fn previous_segment(
        &self,
        position: NativeStringPosition,
        control: Option<&crate::resources::Control>,
    ) -> Result<(NativeStringPosition, NativeStringSegment<'_>), NativeStringError> {
        if position == self.start() {
            return Ok((position, NativeStringSegment::End));
        }
        if position > self.end(control) {
            return Err(NativeStringError::InvalidBoundary(
                "native string slice position is outside its value",
            ));
        }
        match &self.0 {
            Repr::Bytes(bytes) => Ok((
                self.start(),
                NativeStringSegment::Bytes(&bytes[..position.0]),
            )),
            Repr::Parts(parts) => {
                let mut offset = 0;
                for part in parts.iter() {
                    match part {
                        NativeStringPart::Bytes(bytes) => {
                            if position.0 <= offset + bytes.len() {
                                return Ok((
                                    NativeStringPosition(offset),
                                    NativeStringSegment::Bytes(&bytes[..position.0 - offset]),
                                ));
                            }
                            offset += bytes.len();
                        }
                        NativeStringPart::Identity(identity) => {
                            if position.0 == offset + 1 {
                                return Ok((
                                    NativeStringPosition(offset),
                                    NativeStringSegment::Identity(*identity),
                                ));
                            }
                            offset += 1;
                        }
                    }
                }
                Err(NativeStringError::InvalidBoundary(
                    "native string slice position is outside its value",
                ))
            }
        }
    }

    pub(in crate::project) fn retreat_bytes(
        &self,
        position: NativeStringPosition,
        count: usize,
        control: Option<&crate::resources::Control>,
    ) -> Result<NativeStringPosition, NativeStringError> {
        match self.previous_segment(position, control)?.1 {
            NativeStringSegment::Bytes(bytes) if count <= bytes.len() => {
                Ok(NativeStringPosition(position.0 - count))
            }
            _ if count == 0 => Ok(position),
            NativeStringSegment::Identity(_) => Err(NativeStringError::IdentitySpelling),
            _ => Err(NativeStringError::InvalidBoundary(
                "native string byte retreat exceeds its fixed byte run",
            )),
        }
    }

    pub(in crate::project) fn slice(
        &self,
        start: NativeStringPosition,
        end: NativeStringPosition,
        control: Option<&crate::resources::Control>,
    ) -> Result<Self, NativeStringError> {
        if start > end || end > self.end(control) {
            return Err(NativeStringError::InvalidBoundary(
                "native string slice has invalid boundaries",
            ));
        }
        let mut parts = Vec::new();
        let mut offset = 0;
        for part in self.parts() {
            let length = match &part {
                NativeStringPart::Bytes(bytes) => bytes.len(),
                NativeStringPart::Identity(_) => 1,
            };
            let left = start.0.max(offset);
            let right = end.0.min(offset + length);
            if left < right {
                let part = match part {
                    NativeStringPart::Bytes(bytes) => {
                        let mut copy = Vec::new();
                        copy.extend_from_slice(&bytes[left - offset..right - offset]);
                        NativeStringPart::Bytes(std::sync::Arc::from(copy))
                    }
                    NativeStringPart::Identity(identity) => NativeStringPart::Identity(identity),
                };
                parts.push(part);
            }
            offset += length;
            if offset >= end.0 {
                break;
            }
        }
        Ok(Self::from_parts(parts, control))
    }

    pub(super) fn byte_prefix(&self, prefix: &[u8]) -> bool {
        match &self.0 {
            Repr::Bytes(bytes) => bytes.starts_with(prefix),
            Repr::Parts(parts) => {
                matches!(parts.first(), Some(NativeStringPart::Bytes(bytes)) if bytes.starts_with(prefix))
            }
        }
    }

    pub(super) fn private_parts(&self) -> Option<(NativeSymbolIdentity, &[u8])> {
        let Repr::Parts(parts) = &self.0 else {
            return None;
        };
        match parts.as_ref() {
            [
                NativeStringPart::Bytes(prefix),
                NativeStringPart::Identity(class),
                NativeStringPart::Bytes(tail),
            ] if prefix.as_ref() == b"\xfe#" && tail.first() == Some(&b'@') => {
                Some((*class, &tail[1..]))
            }
            _ => None,
        }
    }

    pub(super) fn unique_parts(
        &self,
        control: Option<&crate::resources::Control>,
    ) -> Option<(NativeSymbolIdentity, Self)> {
        let Repr::Parts(parts) = &self.0 else {
            return None;
        };
        if !self.byte_prefix(b"\xfe@") {
            return None;
        }
        let Some(NativeStringPart::Identity(symbol)) = parts.last() else {
            return None;
        };
        let Some(NativeStringPart::Bytes(tail)) = parts.get(parts.len().checked_sub(2)?) else {
            return None;
        };
        if !tail.ends_with(b"@") {
            return None;
        }
        // Prefix, delimiter and final whole atom are established components;
        // these positions never depend on the atom's hidden decimal width.
        let start = NativeStringPosition(2);
        let end = NativeStringPosition(self.display_length(control) - 2);
        // getUniqueESSymbolType's leading FE@ and trailing @ are distinct,
        // including when the underlying name is empty.
        if end < start {
            return None;
        }
        Some((
            *symbol,
            self.slice(start, end, control)
                .expect("recognized unique-name boundaries"),
        ))
    }
}
