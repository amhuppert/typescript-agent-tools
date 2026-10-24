//! `scanString` / `scanTemplateAndSetTokenValue` share the native cooked-byte
//! decoder. The public AST's UTF-16-compatible value is a projection of those
//! bytes, never the source of identity or checker literal values.

use crate::resources::Stop;
use crate::{
    Value,
    compiler::token as flags,
    lexer::{Kind, Token},
};
use std::borrow::Cow;

pub(super) struct DecodedLiteral<'src> {
    pub(super) bytes: Cow<'src, [u8]>,
    pub(super) flags: u32,
    pub(super) error: Option<&'static str>,
}

/// `scanString` / `scanTemplateAndSetTokenValue` retain all bytes after the
/// opening delimiter when unterminated. JSDoc recovery admits those tokens;
/// ordinary source rejects them before literal publication. Only successfully
/// terminated head/middle tokens lose `${`; other terminated tokens lose one
/// closing delimiter. The scanner's result, not the last byte, decides this.
pub(super) fn token_body<'src>(token: Token<'src>) -> &'src str {
    let end = if token.error.is_some() {
        0
    } else if matches!(token.kind, Kind::TemplateHead | Kind::TemplateMiddle) {
        2
    } else {
        1
    };
    &token.raw[1..token.raw.len() - end]
}

impl<'src> DecodedLiteral<'src> {
    pub(super) fn into_public_text(self) -> Result<(Value<'src>, Option<Vec<u8>>), Stop> {
        match self.bytes {
            Cow::Borrowed(bytes) => Ok((
                Value::Text(Cow::Borrowed(
                    std::str::from_utf8(bytes).expect("borrowed scanner bytes are source UTF-8"),
                )),
                None,
            )),
            Cow::Owned(bytes) => match String::from_utf8(bytes) {
                Ok(text) => Ok((Value::Text(Cow::Owned(text)), None)),
                Err(error) => Ok((public_utf16(error.as_bytes())?, Some(error.into_bytes()))),
            },
        }
    }
}

fn public_utf16<'src>(bytes: &[u8]) -> Result<Value<'src>, Stop> {
    let units = utf16(bytes)?;
    let mut text = String::new();
    let mut scalar = true;
    for rune in char::decode_utf16(units.iter().copied()) {
        let Ok(rune) = rune else {
            scalar = false;
            break;
        };
        text.push(rune);
    }
    Ok(if scalar {
        text.into()
    } else {
        Value::Utf16(units)
    })
}

/// UTF-16 projection for the public AST.
/// Decoding native bytes is intentionally distinct from combining adjacent
/// sentinels in a checker string operation.
fn utf16(bytes: &[u8]) -> Result<Vec<u16>, Stop> {
    let mut units = Vec::with_capacity(bytes.len());
    let mut offset = 0;
    while offset < bytes.len() {
        let (rune, width) = crate::strings::decode_js_string_rune(&bytes[offset..]);
        offset += width;
        if rune <= 0xffff {
            units.push(rune as u16);
        } else {
            units.push(0xd800 + ((rune - 0x10000) >> 10) as u16);
            units.push(0xdc00 + ((rune - 0x10000) & 0x3ff) as u16);
        }
    }
    Ok(units)
}

pub(super) fn decode<'s>(raw: &'s str) -> Result<DecodedLiteral<'s>, Stop> {
    if raw.find(['\\', '\r']).is_none() {
        return Ok(DecodedLiteral {
            bytes: Cow::Borrowed(raw.as_bytes()),
            flags: 0,
            error: None,
        });
    }
    let mut bytes = Vec::with_capacity(raw.len());
    let mut offset = 0;
    let mut token_flags = 0;
    let mut first_error = None;
    while offset < raw.len() {
        let start = offset;
        let c = raw[offset..].chars().next().expect("remaining character");
        offset += c.len_utf8();
        if c == '\r' {
            if raw.as_bytes().get(offset) == Some(&b'\n') {
                offset += 1;
            }
            bytes.push(b'\n');
            continue;
        }
        if c != '\\' {
            bytes.extend_from_slice(&raw.as_bytes()[start..offset]);
            continue;
        }
        let mut error = None;
        if let Some(c) = raw[offset..].chars().next() {
            offset += c.len_utf8();
            match c {
                'n' => bytes.push(b'\n'),
                'r' => bytes.push(b'\r'),
                't' => bytes.push(b'\t'),
                'b' => bytes.push(8),
                'f' => bytes.push(12),
                'v' => bytes.push(11),
                '0' if !raw.as_bytes().get(offset).is_some_and(u8::is_ascii_digit) => bytes.push(0),
                '0'..='9' => error = Some("Octal and decimal escape sequences are not allowed"),
                '\n' | '\u{2028}' | '\u{2029}' => {}
                '\r' => {
                    if raw.as_bytes().get(offset) == Some(&b'\n') {
                        offset += 1;
                    }
                }
                'x' | 'u' => {
                    let (mut rune, escape_flags, escape_error) = hex_escape(raw, &mut offset, c)?;
                    error = escape_error;
                    if error.is_none() {
                        token_flags |= escape_flags;
                        // `scanEscapeSequence` combines only an immediately
                        // adjacent Unicode escape. A removed continuation is
                        // still a boundary; concatenation cannot recreate it.
                        if c == 'u'
                            && (0xd800..=0xdbff).contains(&rune)
                            && raw.as_bytes().get(offset..offset + 2) == Some(b"\\u")
                        {
                            let mut next = offset + 2;
                            let (low, low_flags, low_error) = hex_escape(raw, &mut next, 'u')?;
                            if low_error.is_none() && (0xdc00..=0xdfff).contains(&low) {
                                offset = next;
                                token_flags |= low_flags;
                                rune = 0x10000 + ((rune - 0xd800) << 10) + low - 0xdc00;
                            }
                        }
                        crate::strings::encode_js_string_rune(rune, &mut bytes);
                    }
                }
                _ => crate::strings::encode_js_string_rune(c as u32, &mut bytes),
            }
        } else {
            error = Some("Incomplete escape sequence");
        }
        if let Some(error) = error {
            first_error.get_or_insert(error);
            token_flags |= flags::CONTAINS_INVALID_ESCAPE;
            bytes.extend_from_slice(&raw.as_bytes()[start..offset]);
        }
    }
    Ok(DecodedLiteral {
        bytes: Cow::Owned(bytes),
        flags: token_flags,
        error: first_error,
    })
}

/// `scanEscapeSequence`'s fixed/braced hexadecimal arm. A speculative low
/// surrogate read owns its offset and flags.
fn hex_escape(
    raw: &str,
    offset: &mut usize,
    kind: char,
) -> Result<(u32, u32, Option<&'static str>), Stop> {
    let braced = kind == 'u' && raw.as_bytes().get(*offset) == Some(&b'{');
    if braced {
        *offset += 1;
    }
    let digits_start = *offset;
    let count = if kind == 'x' { 2 } else { 4 };
    let mut number = 0u32;
    while braced || *offset - digits_start < count {
        let Some(byte) = raw
            .as_bytes()
            .get(*offset)
            .filter(|b| b.is_ascii_hexdigit())
        else {
            break;
        };
        number = number
            .saturating_mul(16)
            .saturating_add((*byte as char).to_digit(16).expect("hex digit"));
        *offset += 1;
    }
    let mut error = None;
    if braced {
        if *offset == digits_start {
            error = Some("Invalid hexadecimal escape");
        } else if number > 0x10ffff {
            error = Some("Unicode escape exceeds maximum code point");
        }
        if raw.as_bytes().get(*offset) == Some(&b'}') {
            *offset += 1;
        } else {
            error.get_or_insert("Unterminated Unicode escape");
        }
    } else if *offset - digits_start != count {
        error = Some(if *offset == raw.len() {
            "Incomplete hexadecimal escape"
        } else {
            "Invalid hexadecimal escape"
        });
    }
    let flags = if braced {
        flags::EXTENDED_UNICODE_ESCAPE
    } else if kind == 'u' {
        flags::UNICODE_ESCAPE
    } else {
        flags::HEX_ESCAPE
    };
    Ok((number, flags, error))
}
