//! LiteralType.value is an exact native string, including reserved-name bytes
//! and identity components. Factories retain one category and one interner;
//! consumers explicitly request scalar text only where the native operation
//! actually needs it.

use crate::project::binding::{NativePropertyKey, NativeStringValue, NativeSymbolIdentity};
pub(in crate::project::semantic) use crate::project::binding::{
    NativeStringError as LiteralError, NativeStringPart as LiteralPart,
    NativeStringPosition as LiteralPosition, NativeStringSegment as LiteralSegment,
};
use std::cmp::Ordering;

/// One located boundary for native string observations, shared by evaluator
/// consumers and source-free factories once their failure reaches a consumer.
pub(in crate::project::semantic) fn literal_value_failure(
    inputs: crate::project::semantic::inputs::CheckerInputs<'_>,
    consumer: &crate::references::Location,
    error: LiteralError,
) -> super::DemandError {
    match error {
        LiteralError::IdentitySpelling => super::super::unsupported_demand_at_location(
            inputs,
            consumer,
            crate::analysis::SemanticFailureReason::UnrepresentableIdentity,
            error.message(),
        ),
        LiteralError::NonUnicodeBytes => super::super::unsupported_demand_at_location(
            inputs,
            consumer,
            crate::analysis::SemanticFailureReason::UnsupportedSemanticForm,
            error.message(),
        ),
        LiteralError::InvalidBoundary(message) => super::super::internal(message).into(),
    }
}

#[cfg(test)]
mod ordering_tests {
    use super::*;
    use crate::project::binding::IdentityOrder;

    fn atom(index: usize) -> LiteralPart {
        LiteralPart::Identity(NativeSymbolIdentity::Transient(index))
    }

    fn bytes(value: &[u8]) -> LiteralPart {
        LiteralPart::Bytes(value.into())
    }

    #[test]
    fn literal_order_observes_only_a_decisive_native_byte() {
        let identity = LiteralValue::from_parts([atom(0)], None);
        for byte in [0, b'/', b'0', b':', b'x', 0xfe, 0xff] {
            let fixed = LiteralValue::from_bytes(vec![byte], None);
            let expected = if byte < b'1' {
                Ordering::Greater
            } else {
                Ordering::Less
            };
            assert_eq!(identity.compare_value(&fixed, None), Ok(expected));
            assert_eq!(fixed.compare_value(&identity, None), Ok(expected.reverse()));
        }
        for byte in b'1'..=b'9' {
            assert_eq!(
                identity.compare_value(&LiteralValue::from_bytes(vec![byte], None), None),
                Err(LiteralError::IdentitySpelling)
            );
        }
        let other = LiteralValue::from_parts([atom(1)], None);
        assert_eq!(
            identity.compare_value(&other, None),
            Err(LiteralError::IdentitySpelling)
        );
        assert_eq!(identity.compare_value(&identity, None), Ok(Ordering::Equal));
        let empty = LiteralValue::from_text("", None);
        assert_eq!(empty.compare_value(&identity, None), Ok(Ordering::Less));
        assert_eq!(identity.compare_value(&empty, None), Ok(Ordering::Greater));
        let left = LiteralValue::from_parts([bytes(b"pre"), atom(0), bytes(b":a")], None);
        let right = LiteralValue::from_parts([bytes(b"pre"), atom(0), bytes(b":b")], None);
        assert_eq!(left.compare_value(&right, None), Ok(Ordering::Less));
        let prefix = LiteralValue::from_parts([bytes(b"pre"), atom(0)], None);
        assert_eq!(prefix.compare_value(&left, None), Ok(Ordering::Less));
        let undecidable = LiteralValue::from_parts([bytes(b"pre"), atom(1), bytes(b":z")], None);
        assert_eq!(
            left.compare_value(&undecidable, None),
            Err(LiteralError::IdentitySpelling)
        );
        let partitioned = LiteralValue::from_parts([atom(0), atom(1)], None);
        assert_eq!(
            partitioned.compare_value(&LiteralValue::from_parts([atom(1), atom(0)], None), None),
            Err(LiteralError::IdentitySpelling)
        );
        assert_eq!(
            LiteralValue::from_bytes(b"\xfe".as_slice(), None)
                .compare_value(&LiteralValue::from_text("�", None), None),
            Ok(Ordering::Greater)
        );
    }

    #[test]
    fn generated_key_policy_cannot_leak_into_literal_order() {
        let a = NativePropertyKey::unique(
            NativeSymbolIdentity::Transient(0),
            NativePropertyKey::fixed(b"a".as_slice(), None),
            None,
        );
        let b = NativePropertyKey::unique(
            NativeSymbolIdentity::Transient(1),
            NativePropertyKey::fixed(b"a0".as_slice(), None),
            None,
        );
        let c = LiteralValue::from_parts([bytes(b"\xfe@a9"), atom(2)], None);
        // B's underlying-name rule gives a < b. Using that result for literal
        // values while comparing composed c by bytes would create a cycle.
        assert_eq!(
            IdentityOrder::default()
                .compare_property_keys(&a, &b, None)
                .unwrap(),
            Ordering::Less
        );
        let a = LiteralValue::from_property_key(&a);
        let b = LiteralValue::from_property_key(&b);
        assert_eq!(b.compare_value(&c, None), Ok(Ordering::Less));
        assert_eq!(c.compare_value(&a, None), Ok(Ordering::Less));
        assert_eq!(b.compare_value(&a, None), Ok(Ordering::Less));
        assert_eq!(a.compare_value(&b, None), Ok(Ordering::Greater));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) struct LiteralValue(NativeStringValue);

impl LiteralValue {
    pub(in crate::project::semantic) fn from_text(
        text: impl AsRef<str>,
        control: Option<&crate::resources::Control>,
    ) -> Self {
        Self::from_bytes(text.as_ref().as_bytes(), control)
    }

    pub(in crate::project::semantic) fn from_bytes(
        bytes: impl crate::project::binding::NativeByteInput,
        control: Option<&crate::resources::Control>,
    ) -> Self {
        Self(NativeStringValue::from_bytes(bytes, control))
    }

    /// Canonical interner identity; semantic string comparison may ask the
    /// separate, fallible question of an opaque identity's decimal spelling.
    pub(super) fn canonical_equal(&self, other: &Self) -> bool {
        self.0.canonical_equal(&other.0)
    }

    pub(super) fn fingerprint_into<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.fingerprint_into(state);
    }

    pub(in crate::project::semantic) fn from_parts(
        parts: impl IntoIterator<Item = LiteralPart>,
        control: Option<&crate::resources::Control>,
    ) -> Self {
        Self(NativeStringValue::from_parts(parts, control))
    }

    pub(in crate::project::semantic) fn from_property_key(key: &NativePropertyKey) -> Self {
        Self(key.string_value().clone())
    }

    pub(in crate::project::semantic) fn to_property_key(&self) -> NativePropertyKey {
        NativePropertyKey::from_string_value(self.0.clone())
    }

    pub(in crate::project::semantic) fn scalar_text(&self) -> Result<&str, LiteralError> {
        self.0.scalar_text()
    }
    pub(in crate::project::semantic) fn fixed_bytes(&self) -> Result<&[u8], LiteralError> {
        self.0.fixed_bytes()
    }
    pub(in crate::project::semantic) fn equals_text(&self, text: &str) -> bool {
        self.0.equals_text(text)
    }
    pub(in crate::project::semantic) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(in crate::project::semantic) fn display_length(
        &self,
        control: Option<&crate::resources::Control>,
    ) -> usize {
        self.0.display_length(control)
    }
    pub(in crate::project::semantic) fn entire_identity(&self) -> Option<NativeSymbolIdentity> {
        self.0.entire_identity()
    }
    pub(in crate::project::semantic) fn parts(&self) -> impl Iterator<Item = LiteralPart> + '_ {
        self.0.parts()
    }
    pub(in crate::project::semantic) fn concat<'a>(
        parts: impl IntoIterator<Item = &'a Self>,
        control: Option<&crate::resources::Control>,
    ) -> Self {
        Self::from_parts(parts.into_iter().flat_map(Self::parts), control)
    }

    /// Native `CombineSurrogatePairs` cannot combine across an identity atom:
    /// every possible spelling of that atom contains at least one digit.
    pub(in crate::project::semantic) fn combine_surrogate_pairs(
        &self,
        control: Option<&crate::resources::Control>,
    ) -> Self {
        Self::from_parts(
            self.parts().map(|part| match part {
                LiteralPart::Bytes(bytes) => LiteralPart::Bytes(std::sync::Arc::from(
                    crate::strings::combine_surrogate_pairs(&bytes, control),
                )),
                LiteralPart::Identity(identity) => LiteralPart::Identity(identity),
            }),
            control,
        )
    }
    pub(in crate::project::semantic) fn start(&self) -> LiteralPosition {
        self.0.start()
    }
    pub(in crate::project::semantic) fn end(
        &self,
        control: Option<&crate::resources::Control>,
    ) -> LiteralPosition {
        self.0.end(control)
    }
    pub(in crate::project::semantic) fn segment_at(
        &self,
        position: LiteralPosition,
        control: Option<&crate::resources::Control>,
    ) -> Result<LiteralSegment<'_>, LiteralError> {
        self.0.segment_at(position, control)
    }
    pub(in crate::project::semantic) fn previous_segment(
        &self,
        position: LiteralPosition,
        control: Option<&crate::resources::Control>,
    ) -> Result<(LiteralPosition, LiteralSegment<'_>), LiteralError> {
        self.0.previous_segment(position, control)
    }
    pub(in crate::project::semantic) fn advance_bytes(
        &self,
        position: LiteralPosition,
        count: usize,
        control: Option<&crate::resources::Control>,
    ) -> Result<LiteralPosition, LiteralError> {
        self.0.advance_bytes(position, count, control)
    }
    pub(in crate::project::semantic) fn retreat_bytes(
        &self,
        position: LiteralPosition,
        count: usize,
        control: Option<&crate::resources::Control>,
    ) -> Result<LiteralPosition, LiteralError> {
        self.0.retreat_bytes(position, count, control)
    }
    pub(in crate::project::semantic) fn after_segment(
        &self,
        position: LiteralPosition,
        control: Option<&crate::resources::Control>,
    ) -> Result<LiteralPosition, LiteralError> {
        self.0.after_segment(position, control)
    }
    pub(in crate::project::semantic) fn slice(
        &self,
        start: LiteralPosition,
        end: LiteralPosition,
        control: Option<&crate::resources::Control>,
    ) -> Result<Self, LiteralError> {
        self.0.slice(start, end, control).map(Self)
    }

    /// `CompareTypes` compares string values and template spans with native
    /// byte ordering. Equal whole atoms cancel without observing their width;
    /// a first differing byte decides immediately. An opaque positive decimal
    /// starts in 1..=9, but its actual digits and distinct-atom order are unknown.
    /// Generated property-key ordering is a different, explicitly adopted rule.
    pub(in crate::project::semantic) fn compare_value(
        &self,
        other: &Self,
        control: Option<&crate::resources::Control>,
    ) -> Result<Ordering, LiteralError> {
        let (mut left, mut right) = (self.start(), other.start());
        loop {
            match (
                self.segment_at(left, control)?,
                other.segment_at(right, control)?,
            ) {
                (LiteralSegment::End, LiteralSegment::End) => return Ok(Ordering::Equal),
                (LiteralSegment::End, _) => return Ok(Ordering::Less),
                (_, LiteralSegment::End) => return Ok(Ordering::Greater),
                (LiteralSegment::Bytes(a), LiteralSegment::Bytes(b)) => {
                    let count = a.len().min(b.len());
                    let order = a[..count].cmp(&b[..count]);
                    if order != Ordering::Equal {
                        return Ok(order);
                    }
                    left = self.advance_bytes(left, count, control)?;
                    right = other.advance_bytes(right, count, control)?;
                }
                (LiteralSegment::Identity(a), LiteralSegment::Identity(b)) if a == b => {
                    left = self.after_segment(left, control)?;
                    right = other.after_segment(right, control)?;
                }
                (LiteralSegment::Identity(_), LiteralSegment::Bytes(bytes)) if bytes[0] < b'1' => {
                    return Ok(Ordering::Greater);
                }
                (LiteralSegment::Identity(_), LiteralSegment::Bytes(bytes)) if bytes[0] > b'9' => {
                    return Ok(Ordering::Less);
                }
                (LiteralSegment::Bytes(bytes), LiteralSegment::Identity(_)) if bytes[0] < b'1' => {
                    return Ok(Ordering::Less);
                }
                (LiteralSegment::Bytes(bytes), LiteralSegment::Identity(_)) if bytes[0] > b'9' => {
                    return Ok(Ordering::Greater);
                }
                _ => return Err(LiteralError::IdentitySpelling),
            }
        }
    }

    /// Native LiteralType.value equality is a string observation, separately
    /// from canonical interner identity. Equal atoms cancel; distinct whole
    /// atoms are unequal because GetSymbolId is injective. Adjacent atoms must
    /// not be zipped as digits: differently partitioned spellings can coincide.
    pub(in crate::project::semantic) fn semantic_equal(
        &self,
        other: &Self,
        control: Option<&crate::resources::Control>,
    ) -> Result<bool, LiteralError> {
        if self.0.canonical_equal(&other.0) {
            return Ok(true);
        }
        let collect = |value: &Self| {
            let mut parts = Vec::new();
            for part in value.parts() {
                parts.push(part);
            }
            parts
        };
        let left = collect(self);
        let right = collect(other);
        let mut start = 0;
        while start < left.len().min(right.len()) && left[start].canonical_equal(&right[start]) {
            start += 1;
        }
        let (mut left_end, mut right_end) = (left.len(), right.len());
        while left_end > start
            && right_end > start
            && left[left_end - 1].canonical_equal(&right[right_end - 1])
        {
            left_end -= 1;
            right_end -= 1;
        }
        let has_non_decimal_part = |rest: &[LiteralPart]| {
            rest.iter().any(|part| match part {
                LiteralPart::Bytes(bytes) => bytes.iter().any(|byte| !byte.is_ascii_digit()),
                LiteralPart::Identity(_) => false,
            })
        };
        match (&left[start..left_end], &right[start..right_end]) {
            ([LiteralPart::Identity(a)], [LiteralPart::Identity(b)]) => return Ok(a == b),
            ([LiteralPart::Identity(_)], rest) | (rest, [LiteralPart::Identity(_)])
                if has_non_decimal_part(rest) =>
            {
                return Ok(false);
            }
            _ => {}
        }
        match self.match_prefix_at(self.start(), other, control) {
            Ok(Some(end)) => Ok(end == self.end(control)),
            Ok(None) => Ok(false),
            // A fixed suffix can independently prove inequality without
            // observing the digits that made the prefix inconclusive.
            Err(LiteralError::IdentitySpelling) => match self.match_suffix(other, control) {
                Ok(None) => Ok(false),
                Ok(Some(_)) | Err(LiteralError::IdentitySpelling) => {
                    Err(LiteralError::IdentitySpelling)
                }
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        }
    }

    /// Native `strings.HasPrefix`, including whole equal identity atoms.
    /// The result is an established boundary, never an inferred digit count.
    pub(in crate::project::semantic) fn match_prefix_at(
        &self,
        position: LiteralPosition,
        prefix: &Self,
        control: Option<&crate::resources::Control>,
    ) -> Result<Option<LiteralPosition>, LiteralError> {
        self.prefix_match(position, prefix, false, control)
    }

    /// `templateLiteralTypesDefinitelyUnrelated` compares the shorter prefix.
    pub(in crate::project::semantic) fn compatible_prefix(
        &self,
        other: &Self,
        control: Option<&crate::resources::Control>,
    ) -> Result<bool, LiteralError> {
        self.prefix_match(self.start(), other, true, control)
            .map(|end| end.is_some())
    }

    fn prefix_match(
        &self,
        mut position: LiteralPosition,
        prefix: &Self,
        allow_shorter: bool,
        control: Option<&crate::resources::Control>,
    ) -> Result<Option<LiteralPosition>, LiteralError> {
        let mut pattern = prefix.start();
        loop {
            match (
                self.segment_at(position, control)?,
                prefix.segment_at(pattern, control)?,
            ) {
                (_, LiteralSegment::End) => return Ok(Some(position)),
                (LiteralSegment::End, _) => return Ok(allow_shorter.then_some(position)),
                (LiteralSegment::Bytes(bytes), LiteralSegment::Bytes(wanted)) => {
                    let count = bytes.len().min(wanted.len());
                    if bytes[..count] != wanted[..count] {
                        return Ok(None);
                    }
                    position = self.advance_bytes(position, count, control)?;
                    pattern = prefix.advance_bytes(pattern, count, control)?;
                }
                (LiteralSegment::Identity(a), LiteralSegment::Identity(b)) if a == b => {
                    position = self.after_segment(position, control)?;
                    pattern = prefix.after_segment(pattern, control)?;
                }
                (LiteralSegment::Identity(_), LiteralSegment::Bytes(bytes))
                | (LiteralSegment::Bytes(bytes), LiteralSegment::Identity(_))
                    if !bytes[0].is_ascii_digit() =>
                {
                    return Ok(None);
                }
                _ => return Err(LiteralError::IdentitySpelling),
            }
        }
    }

    /// Native `strings.HasSuffix` establishes the start boundary by walking
    /// backward over bytes and whole equal atoms, without subtracting widths.
    pub(in crate::project::semantic) fn match_suffix(
        &self,
        suffix: &Self,
        control: Option<&crate::resources::Control>,
    ) -> Result<Option<LiteralPosition>, LiteralError> {
        self.suffix_match(suffix, false, control)
    }

    /// `templateLiteralTypesDefinitelyUnrelated` compares the shorter suffix.
    pub(in crate::project::semantic) fn compatible_suffix(
        &self,
        other: &Self,
        control: Option<&crate::resources::Control>,
    ) -> Result<bool, LiteralError> {
        self.suffix_match(other, true, control)
            .map(|start| start.is_some())
    }

    fn suffix_match(
        &self,
        suffix: &Self,
        allow_shorter: bool,
        control: Option<&crate::resources::Control>,
    ) -> Result<Option<LiteralPosition>, LiteralError> {
        let mut position = self.end(control);
        let mut pattern = suffix.end(control);
        loop {
            let (before, segment) = self.previous_segment(position, control)?;
            let (pattern_before, wanted) = suffix.previous_segment(pattern, control)?;
            match (segment, wanted) {
                (_, LiteralSegment::End) => return Ok(Some(position)),
                (LiteralSegment::End, _) => return Ok(allow_shorter.then_some(position)),
                (LiteralSegment::Bytes(bytes), LiteralSegment::Bytes(wanted)) => {
                    let count = bytes.len().min(wanted.len());
                    if bytes[bytes.len() - count..] != wanted[wanted.len() - count..] {
                        return Ok(None);
                    }
                    position = self.retreat_bytes(position, count, control)?;
                    pattern = suffix.retreat_bytes(pattern, count, control)?;
                }
                (LiteralSegment::Identity(a), LiteralSegment::Identity(b)) if a == b => {
                    position = before;
                    pattern = pattern_before;
                }
                (LiteralSegment::Identity(_), LiteralSegment::Bytes(bytes))
                | (LiteralSegment::Bytes(bytes), LiteralSegment::Identity(_))
                    if !bytes[bytes.len() - 1].is_ascii_digit() =>
                {
                    return Ok(None);
                }
                _ => return Err(LiteralError::IdentitySpelling),
            }
        }
    }

    /// Native `strings.Index` searches exact bytes. A nondigit delimiter skips
    /// whole atoms; a candidate depending on hidden digits stays a failure.
    pub(in crate::project::semantic) fn index_from(
        &self,
        mut position: LiteralPosition,
        delimiter: &Self,
        control: Option<&crate::resources::Control>,
    ) -> Result<Option<(LiteralPosition, LiteralPosition)>, LiteralError> {
        self.segment_at(position, control)?;
        if delimiter.is_empty() {
            return Ok(Some((position, position)));
        }
        loop {
            match self.segment_at(position, control)? {
                LiteralSegment::Bytes(bytes) => {
                    let offset = match delimiter.segment_at(delimiter.start(), control)? {
                        LiteralSegment::Bytes(wanted) => {
                            bytes.iter().position(|&byte| byte == wanted[0])
                        }
                        LiteralSegment::Identity(_) => {
                            bytes.iter().position(|byte| byte.is_ascii_digit())
                        }
                        LiteralSegment::End => {
                            return Err(LiteralError::InvalidBoundary(
                                "nonempty native delimiter has no first segment",
                            ));
                        }
                    };
                    let Some(offset) = offset else {
                        position = self.after_segment(position, control)?;
                        continue;
                    };
                    position = self.advance_bytes(position, offset, control)?;
                    if let Some(after) = self.match_prefix_at(position, delimiter, control)? {
                        return Ok(Some((position, after)));
                    }
                    position = self.advance_bytes(position, 1, control)?;
                }
                LiteralSegment::Identity(_) => {
                    if let Some(after) = self.match_prefix_at(position, delimiter, control)? {
                        return Ok(Some((position, after)));
                    }
                    position = self.after_segment(position, control)?;
                }
                LiteralSegment::End => return Ok(None),
            }
        }
    }

    /// Native `DecodeJSStringRune` chooses only the width for an empty
    /// delimiter. The caller's slice retains the original bytes, including FE.
    pub(in crate::project::semantic) fn next_rune(
        &self,
        position: LiteralPosition,
        control: Option<&crate::resources::Control>,
    ) -> Result<Option<LiteralPosition>, LiteralError> {
        match self.segment_at(position, control)? {
            LiteralSegment::Bytes(bytes) => {
                let (_, count) = crate::strings::decode_js_string_rune(bytes);
                self.advance_bytes(position, count, control).map(Some)
            }
            LiteralSegment::Identity(_) => Err(LiteralError::IdentitySpelling),
            LiteralSegment::End => Ok(None),
        }
    }
}

/// `getStringLiteralType` / `getNumberLiteralType` / `getBigIntLiteralType`
/// share regular identity by category and exact canonical value. Native enum
/// literal caches additionally require the actual enum symbol's identity.
pub(super) struct LiteralKey {
    base: crate::types::PrimitiveName,
    enumeration: Option<NativeSymbolIdentity>,
    value: LiteralValue,
    fingerprint: u64,
}

impl LiteralKey {
    pub(super) fn new(
        base: crate::types::PrimitiveName,
        enumeration: Option<NativeSymbolIdentity>,
        value: LiteralValue,
    ) -> Self {
        use std::hash::{DefaultHasher, Hash, Hasher};

        let mut hash = DefaultHasher::new();
        (base, enumeration).hash(&mut hash);
        {
            value.fingerprint_into(&mut hash);
        }
        Self {
            base,
            enumeration,
            value,
            fingerprint: hash.finish(),
        }
    }
}

impl crate::resources::CheckedKey for LiteralKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn equivalent(&self, other: &Self) -> bool {
        self.base == other.base
            && self.enumeration == other.enumeration
            && self.value.canonical_equal(&other.value)
    }
}
