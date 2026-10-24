//! TypeScript's JavaScript default casing, pinned independently of Rust's
//! Unicode tables. Native strings may also contain lone-surrogate sentinels
//! and invalid UTF-8 bytes; each operation preserves the native byte semantics.
mod js_case_tables;

/// `stringutil.DecodeJSStringRune`: decode one native JS-string rune, including
/// the three-byte surrogate sentinel. A stray invalid byte consumes one byte
/// and yields RuneError; an empty input consumes none. This is a text operation,
/// never a decoder for stored identity: callers retain the original bytes.
pub(crate) fn decode_js_string_rune(bytes: &[u8]) -> (u32, usize) {
    if bytes.len() >= 3
        && bytes[0] == 0xed
        && (0xa0..=0xbf).contains(&bytes[1])
        && (0x80..=0xbf).contains(&bytes[2])
    {
        return (
            0xd000 | (u32::from(bytes[1] & 0x3f) << 6) | u32::from(bytes[2] & 0x3f),
            3,
        );
    }
    let Some(&first) = bytes.first() else {
        return (0xfffd, 0);
    };
    let width = match first {
        0..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return (0xfffd, 1),
    };
    let Some(prefix) = bytes.get(..width) else {
        return (0xfffd, 1);
    };
    match std::str::from_utf8(prefix) {
        Ok(text) => (
            text.chars().next().expect("nonempty decoded rune") as u32,
            width,
        ),
        Err(_) => (0xfffd, 1),
    }
}

/// `stringutil.EncodeJSStringRune` writes lone UTF-16 halves as the native
/// three-byte sentinel; other runes use Go's ordinary string(rune) encoding.
pub(crate) fn encode_js_string_rune(rune: u32, output: &mut impl Extend<u8>) {
    if (0xd800..=0xdfff).contains(&rune) {
        output.extend([
            0xed,
            0x80 | ((rune >> 6) & 0x3f) as u8,
            0x80 | (rune & 0x3f) as u8,
        ]);
    } else {
        let rune = char::from_u32(rune).unwrap_or('\u{fffd}');
        output.extend(rune.encode_utf8(&mut [0; 4]).bytes());
    }
}

/// `stringutil.CombineSurrogatePairs` normalizes values after concatenation.
/// Unrelated bytes, including invalid UTF-8, remain exact; decoding only finds
/// adjacent high/low sentinels and their widths.
pub(crate) fn combine_surrogate_pairs(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len());
    let mut offset = 0;
    while offset < bytes.len() {
        let (high, width) = decode_js_string_rune(&bytes[offset..]);
        if (0xd800..=0xdbff).contains(&high) {
            let (low, low_width) = decode_js_string_rune(&bytes[offset + width..]);
            if (0xdc00..=0xdfff).contains(&low) {
                let mut encoded = smallvec::SmallVec::<[u8; 4]>::new();
                encode_js_string_rune(
                    0x10000 + ((high - 0xd800) << 10) + low - 0xdc00,
                    &mut encoded,
                );
                output.extend_from_slice(&encoded);
                offset += width + low_width;
                continue;
            }
        }
        output.extend_from_slice(&bytes[offset..offset + width]);
        offset += width;
    }
    output
}

/// TypeScript `stringutil.StripQuotes` removes a matching ASCII quote pair;
/// it does not decode escapes or remove unmatched quotes.
pub(crate) fn strip_quotes(text: &str) -> &str {
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && matches!(bytes[0], b'\'' | b'"' | b'`') && bytes.first() == bytes.last()
    {
        &text[1..text.len() - 1]
    } else {
        text
    }
}

/// `stringutil.UnquoteString` preserves native's deliberately limited regexp
/// replacement: remove each backslash followed by a non-LF character. It is
/// not JavaScript string-literal decoding.
pub(crate) fn unquote_string(text: &str) -> String {
    let mut chars = strip_quotes(text).chars().peekable();
    let mut result = String::new();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek().is_some_and(|c| *c != '\n') {
            result.push(chars.next().expect("peeked character"));
        } else {
            result.push(c);
        }
    }
    result
}

/// `stringutil.ToUpperJS` uses the pinned full mappings, including expansions
/// such as sharp-s to SS. Lone surrogates are unchanged; other invalid bytes
/// deliberately become RuneError under the native casing operation.
pub(crate) fn to_upper_js(text: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(text.len());
    let mut offset = 0;
    while offset < text.len() {
        let (rune, width) = decode_js_string_rune(&text[offset..]);
        if let Ok(index) = js_case_tables::MAPPINGS.binary_search_by_key(&rune, |row| row.0) {
            result.extend_from_slice(js_case_tables::MAPPINGS[index].2.as_bytes());
        } else {
            let mut encoded = smallvec::SmallVec::<[u8; 4]>::new();
            encode_js_string_rune(rune, &mut encoded);
            result.extend_from_slice(&encoded);
        }
        offset += width;
    }
    result
}

/// `stringutil.ToLowerJS` / `isFinalSigmaContext` implement the native
/// context rule using the same pinned Cased and Case_Ignorable properties.
pub(crate) fn to_lower_js(text: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(text.len());
    let mut cased_before = false;
    let mut offset = 0;
    while offset < text.len() {
        let (rune, width) = decode_js_string_rune(&text[offset..]);
        offset += width;
        if let Ok(mapping) = js_case_tables::MAPPINGS.binary_search_by_key(&rune, |row| row.0) {
            let (_, lower, _, conditional) = js_case_tables::MAPPINGS[mapping];
            let final_sigma =
                !conditional.is_empty() && cased_before && !has_cased_after(&text[offset..]);
            result.extend_from_slice(if final_sigma { conditional } else { lower }.as_bytes());
        } else {
            let mut encoded = smallvec::SmallVec::<[u8; 4]>::new();
            encode_js_string_rune(rune, &mut encoded);
            result.extend_from_slice(&encoded);
        }
        if !in_ranges(rune, js_case_tables::CASE_IGNORABLE) {
            cased_before = in_ranges(rune, js_case_tables::CASED);
        }
    }
    result
}

fn has_cased_after(mut text: &[u8]) -> bool {
    while !text.is_empty() {
        let (rune, width) = decode_js_string_rune(text);
        text = &text[width..];
        if !in_ranges(rune, js_case_tables::CASE_IGNORABLE) {
            return in_ranges(rune, js_case_tables::CASED);
        }
    }
    false
}

fn in_ranges(c: u32, ranges: &[(u32, u32, u32)]) -> bool {
    let index = ranges.partition_point(|&(lo, _, _)| lo <= c);
    index > 0 && {
        let (lo, hi, stride) = ranges[index - 1];
        c <= hi && (c - lo) % stride == 0
    }
}
