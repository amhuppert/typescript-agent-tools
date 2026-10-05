//! Decode literal text and escape metadata together. Invalid escapes retain
//! their scanned spelling so tagged templates can preserve compiler token text;
//! strict callers reject the first error instead.

use crate::{Value, compiler::token as flags};

pub(super) struct DecodedLiteral<'src> {
    pub(super) text: Value<'src>,
    pub(super) flags: u32,
    pub(super) error: Option<&'static str>,
}

pub(super) fn decode(raw: &str) -> DecodedLiteral<'_> {
    if !raw.contains(['\\', '\r']) {
        return DecodedLiteral {
            text: raw.into(),
            flags: 0,
            error: None,
        };
    }
    let mut text = Vec::with_capacity(raw.len());
    let mut offset = 0;
    let mut token_flags = 0;
    let mut first_error = None;
    while offset < raw.len() {
        let start = offset;
        let c = raw[offset..].chars().next().unwrap();
        offset += c.len_utf8();
        if c == '\r' {
            if raw.as_bytes().get(offset) == Some(&b'\n') {
                offset += 1;
            }
            text.push(10);
            continue;
        }
        if c != '\\' {
            push(&mut text, c);
            continue;
        }
        let mut error = None;
        if let Some(c) = raw[offset..].chars().next() {
            offset += c.len_utf8();
            match c {
                'n' => text.push(10),
                'r' => text.push(13),
                't' => text.push(9),
                'b' => text.push(8),
                'f' => text.push(12),
                'v' => text.push(11),
                '0' if !raw.as_bytes().get(offset).is_some_and(u8::is_ascii_digit) => {
                    text.push(0);
                }
                '0'..='9' => error = Some("Octal and decimal escape sequences are not allowed"),
                '\n' | '\u{2028}' | '\u{2029}' => {}
                '\r' => {
                    if raw.as_bytes().get(offset) == Some(&b'\n') {
                        offset += 1;
                    }
                }
                'x' | 'u' => {
                    let braced = c == 'u' && raw.as_bytes().get(offset) == Some(&b'{');
                    if braced {
                        offset += 1;
                    }
                    let digits_start = offset;
                    let count = if c == 'x' { 2 } else { 4 };
                    let mut number = 0u32;
                    while braced || offset - digits_start < count {
                        let Some(byte) =
                            raw.as_bytes().get(offset).filter(|b| b.is_ascii_hexdigit())
                        else {
                            break;
                        };
                        number = number
                            .saturating_mul(16)
                            .saturating_add((*byte as char).to_digit(16).unwrap());
                        offset += 1;
                    }
                    if braced {
                        if offset == digits_start {
                            error = Some("Invalid hexadecimal escape");
                        } else if number > 0x10FFFF {
                            error = Some("Unicode escape exceeds maximum code point");
                        }
                        if raw.as_bytes().get(offset) == Some(&b'}') {
                            offset += 1;
                        } else {
                            error.get_or_insert("Unterminated Unicode escape");
                        }
                    } else if offset - digits_start != count {
                        error = Some(if offset == raw.len() {
                            "Incomplete hexadecimal escape"
                        } else {
                            "Invalid hexadecimal escape"
                        });
                    }
                    if error.is_none() {
                        token_flags |= if braced {
                            flags::EXTENDED_UNICODE_ESCAPE
                        } else if c == 'u' {
                            flags::UNICODE_ESCAPE
                        } else {
                            flags::HEX_ESCAPE
                        };
                        if number <= 0xFFFF {
                            text.push(number as u16);
                        } else {
                            push(&mut text, char::from_u32(number).unwrap());
                        }
                    }
                }
                _ => push(&mut text, c),
            }
        } else {
            error = Some("Incomplete escape sequence");
        }
        if let Some(error) = error {
            first_error.get_or_insert(error);
            token_flags |= flags::CONTAINS_INVALID_ESCAPE;
            text.extend(raw[start..offset].encode_utf16());
        }
    }
    DecodedLiteral {
        text: match String::from_utf16(&text) {
            Ok(text) => text.into(),
            Err(_) => Value::Utf16(text),
        },
        flags: token_flags,
        error: first_error,
    }
}

fn push(text: &mut Vec<u16>, c: char) {
    let mut units = [0; 2];
    text.extend_from_slice(c.encode_utf16(&mut units));
}
