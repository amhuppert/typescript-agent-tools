//! Native numeric spelling shared by syntax and semantic property identities.

/// `jsnum.Number.String`: shortest round-trip binary64 digits, with the native
/// decimal/exponent windows and zero/nonfinite spellings. Ryu owns digit
/// generation (including ties); this owner supplies the JavaScript layout.
pub(crate) fn number_to_string(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".into()
        } else {
            "Infinity".into()
        };
    }

    let mut buffer = ryu::Buffer::new();
    let raw = buffer.format_finite(value.abs());
    let (mantissa, exponent) = match raw.split_once('e') {
        Some((mantissa, exponent)) => (
            mantissa,
            exponent
                .parse::<i32>()
                .expect("Ryu emits a decimal exponent"),
        ),
        None => (raw, 0),
    };
    let mut point = mantissa.find('.').unwrap_or(mantissa.len()) as i32 + exponent;
    let digits = mantissa.replace('.', "");
    let leading = digits.bytes().take_while(|byte| *byte == b'0').count();
    point -= leading as i32;
    let digits = digits[leading..].trim_end_matches('0');
    let mut result = String::new();
    if value.is_sign_negative() {
        result.push('-');
    }
    if point > 0 && point <= 21 {
        let point = point as usize;
        if point >= digits.len() {
            result.push_str(digits);
            result.extend(std::iter::repeat_n('0', point - digits.len()));
        } else {
            result.push_str(&digits[..point]);
            result.push('.');
            result.push_str(&digits[point..]);
        }
    } else if point <= 0 && point > -6 {
        result.push_str("0.");
        result.extend(std::iter::repeat_n('0', (-point) as usize));
        result.push_str(digits);
    } else {
        result.push_str(&digits[..1]);
        if digits.len() > 1 {
            result.push('.');
            result.push_str(&digits[1..]);
        }
        result.push('e');
        let exponent = point - 1;
        if exponent >= 0 {
            result.push('+');
        }
        result.push_str(&exponent.to_string());
    }
    result
}

/// `isNumericLiteralName` asks whether StringToNumber followed by Number.String
/// preserves the exact name, including the native nonfinite spellings.
pub(crate) fn is_numeric_literal_name(name: &str) -> bool {
    numeric_literal_name_value(name).is_some()
}

/// `isNumericLiteralName` followed by `jsnum.FromString` preserves the same
/// exact round-trip classification when the caller also needs its value.
pub(crate) fn numeric_literal_name_value(name: &str) -> Option<f64> {
    let value = number_from_string(name);
    (number_to_string(value) == name).then_some(value)
}

/// Native `jsnum.FromString` / StringToNumber, distinct from the scanner's
/// literal grammar. Radix forms have no sign; decimal forms accept a sign,
/// leading zeros, and surrounding ECMAScript string whitespace.
pub(crate) fn number_from_string(value: &str) -> f64 {
    if has_non_numeric_string_character(value.as_bytes()) {
        return f64::NAN;
    }
    let value = value.trim_matches(string_number_whitespace);
    match value {
        "" => return 0.0,
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let radix = match value.get(..2) {
        Some("0x" | "0X") => Some(16),
        Some("0o" | "0O") => Some(8),
        Some("0b" | "0B") => Some(2),
        _ => None,
    };
    if let Some(radix) = radix {
        let digits = &value[2..];
        if digits.is_empty()
            || !digits
                .chars()
                .all(|digit| digit.is_ascii() && digit.is_digit(radix))
        {
            return f64::NAN;
        }
        let integer = num_bigint::BigUint::parse_bytes(digits.as_bytes(), radix)
            .expect("validated radix digits parse as an integer");
        return integer
            .to_str_radix(10)
            .parse()
            .expect("decimal integer converts to binary64");
    }
    // Rust accepts non-JavaScript spellings such as inf. Restrict characters
    // first, then let its correctly rounded decimal conversion own the value.
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        return f64::NAN;
    }
    value.parse().unwrap_or(f64::NAN)
}

/// Native `jsnum.isStrWhiteSpace` intentionally differs from scanner trivia:
/// neither U+0085 nor U+200B belongs to StringToNumber's whitespace grammar.
fn string_number_whitespace(value: char) -> bool {
    matches!(
        value,
        '\t' | '\u{b}' | '\u{c}' | '\n' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// A necessary character condition shared by `jsnum.FromString` and
/// `isValidBigIntString`. Their decimal/radix/nonfinite grammars use only ASCII
/// letters and digits, sign/point punctuation and native string whitespace.
/// This operation can reject a fixed fragment around an opaque decimal ID
/// without inspecting that ID. Passing it is not a successful numeric parse;
/// the complete parser still owns grammar, range and canonical spelling.
pub(crate) fn has_non_numeric_string_character(mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        let (rune, width) = crate::strings::decode_js_string_rune(bytes);
        let Some(character) = char::from_u32(rune) else {
            return true;
        };
        if !character.is_ascii_alphanumeric()
            && !matches!(character, '+' | '-' | '.')
            && !string_number_whitespace(character)
        {
            return true;
        }
        bytes = &bytes[width..];
    }
    false
}

/// `isValidNumberString` excludes empty and nonfinite values; constrained
/// inference additionally requires the native round-trip spelling.
pub(crate) fn is_valid_number_string(value: &str, round_trip_only: bool) -> bool {
    if value.is_empty() {
        return false;
    }
    let number = number_from_string(value);
    number.is_finite() && (!round_trip_only || number_to_string(number) == value)
}

/// `isValidBigIntString` uses the existing scanner, with trivia forbidden and
/// an appended n. It accepts a single leading minus, rejects separators and
/// scanner errors, and optionally compares the canonical decimal magnitude.
pub(crate) fn bigint_from_string(value: &str, round_trip_only: bool) -> Option<String> {
    if value.is_empty() || has_non_numeric_string_character(value.as_bytes()) {
        return None;
    }
    let text = format!("{value}n");
    let mut scanner = crate::lexer::Lexer::new(&text, 0);
    let mut token = scanner.next();
    if token.start != 0 || token.error.is_some() {
        return None;
    }
    let negative = token.raw == "-";
    if negative {
        token = scanner.next();
        if token.start != 1 {
            return None;
        }
    }
    if token.kind != crate::lexer::Kind::Number
        || token.error.is_some()
        || token.end != text.len()
        || !token.raw.ends_with('n')
    {
        return None;
    }
    let magnitude = bigint_literal_value(token.raw)?;
    let canonical = if negative && magnitude != "0" {
        format!("-{magnitude}")
    } else {
        magnitude
    };
    (!round_trip_only || canonical == value).then_some(canonical)
}

/// `jsnum.ParsePseudoBigInt` / `PseudoBigInt.String`: scanner-normalized
/// unsigned literal payloads convert exactly to decimal without a trailing n.
/// Validation of arbitrary input strings is a separate scanner question above.
pub(crate) fn bigint_literal_value(text: &str) -> Option<String> {
    let text = text.strip_suffix('n').unwrap_or(text);
    let (digits, radix) = match text.get(..2) {
        Some("0b" | "0B") => (&text[2..], 2),
        Some("0o" | "0O") => (&text[2..], 8),
        Some("0x" | "0X") => (&text[2..], 16),
        _ => (text, 10),
    };
    num_bigint::BigUint::parse_bytes(digits.as_bytes(), radix)
        .map(|integer| integer.to_str_radix(10))
}

/// `jsnum.Number.toInt32` / `toUint32`: truncate finite numbers, reduce modulo
/// 2^32, and interpret the retained bits; Rust's saturating float cast alone
/// would change out-of-range bitwise operands.
pub(crate) fn number_to_int32(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    (value.trunc() % 4294967296.0) as i64 as i32
}

/// `jsnum.Number.Exponentiate` retains native non-finite exceptions and the
/// exact integer-power path, including the intermediate 256-bit float rounding.
pub(crate) fn number_exponentiate(base: f64, exponent: f64) -> f64 {
    if (base == 1.0 || base == -1.0) && exponent.is_infinite() || base == 1.0 && exponent.is_nan() {
        return f64::NAN;
    }
    let magnitude = exponent * base.abs().log2();
    if base >= i64::MIN as f64
        && base <= i64::MAX as f64
        && base == base.trunc()
        && exponent >= 0.0
        && exponent <= i64::MAX as f64
        && exponent == exponent.trunc()
        && exponent.is_finite()
        && magnitude > 53.0
        && magnitude <= f64::MAX.log2()
    {
        use num_bigint::{BigInt, BigUint, Sign};
        // Go's boundary conversion of float64(2^63) has the sign bit set.
        let integer = if base == 9223372036854775808.0 {
            i64::MIN
        } else {
            base as i64
        };
        let exact = BigInt::from(integer).pow(exponent as u32);
        let (sign, bytes) = exact.to_bytes_be();
        let mut value = BigUint::from_bytes_be(&bytes);
        if value.bits() > 256 {
            let shift = (value.bits() - 256) as usize;
            let mut high = &value >> shift;
            let low = &value - (&high << shift);
            let half = BigUint::from(1_u8) << (shift - 1);
            if low > half || low == half && high.bit(0) {
                high += 1_u8;
            }
            value = high << shift;
        }
        let value = value
            .to_str_radix(10)
            .parse::<f64>()
            .expect("a generated decimal integer is a valid numeric payload");
        return if sign == Sign::Minus { -value } else { value };
    }
    base.powf(exponent)
}
