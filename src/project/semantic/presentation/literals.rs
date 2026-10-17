//! Lossless public literal projection. These IDs belong to one description,
//! never to native symbol ordering or semantic identity. Summary recursion and
//! graph expansion share this registry. Only retained output claims references;
//! summary and graph admission roll back entries from omitted work.

use super::super::evaluation::Evaluator;
use super::super::model::{LiteralError, LiteralPart, LiteralValue};
use crate::analysis::AnalysisError;
use crate::project::binding::NativeSymbolIdentity;
use crate::references::Location;
use crate::types::{TypeLiteralPart, TypeLiteralValue};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Default)]
pub(super) struct LiteralPresentation(Rc<RefCell<HashMap<NativeSymbolIdentity, usize>>>);

impl LiteralPresentation {
    fn reference(&self, identity: NativeSymbolIdentity) -> String {
        let mut ids = self.0.borrow_mut();
        let next = ids.len();
        format!("s{}", ids.entry(identity).or_insert(next))
    }

    pub(super) fn checkpoint(&self) -> usize {
        self.0.borrow().len()
    }

    pub(super) fn rollback(&self, length: usize) {
        self.0.borrow_mut().retain(|_, index| *index < length);
    }

    /// Fixed bytes retain the native quoted form, including its deliberate
    /// invalid-byte escapes. Only opaque identities need symbolic constructors,
    /// so quoted user text cannot imitate a generated string's parts.
    pub(super) fn summary(&self, value: &LiteralValue) -> Result<String, AnalysisError> {
        match value.fixed_bytes() {
            Ok(bytes) => return Ok(crate::printer::quoted_bytes(bytes, '"')),
            Err(LiteralError::IdentitySpelling) => {}
            Err(LiteralError::NonUnicodeBytes) => {
                return Err(super::super::internal(
                    "fixed literal bytes unexpectedly require Unicode",
                ));
            }
            Err(LiteralError::InvalidBoundary(message)) => {
                return Err(super::super::internal(message));
            }
        }
        let parts = value
            .parts()
            .map(|part| match part {
                LiteralPart::Bytes(bytes) => match std::str::from_utf8(&bytes) {
                    Ok(text) => crate::printer::quoted_text(text, '"'),
                    Err(_) => format!("bytes(\"{}\")", hex(&bytes)),
                },
                LiteralPart::Identity(identity) => format!("symbol({})", self.reference(identity)),
            })
            .collect::<Vec<_>>();
        Ok(format!("string({})", parts.join(", ")))
    }

    /// Exact graph metadata is acquired only when an admitted node needs its
    /// value. The caller charges the complete result to its ordinary byte budget.
    pub(super) fn value(
        &self,
        value: &LiteralValue,
        evaluator: &mut Evaluator<'_, '_>,
        consumer: &Location,
    ) -> Result<TypeLiteralValue, AnalysisError> {
        match value.scalar_text() {
            Ok(text) => return Ok(TypeLiteralValue::Text(text.into())),
            Err(LiteralError::IdentitySpelling | LiteralError::NonUnicodeBytes) => {}
            Err(LiteralError::InvalidBoundary(message)) => {
                return Err(super::super::internal(message));
            }
        }
        let mut parts = Vec::new();
        for part in value.parts() {
            parts.push(match part {
                LiteralPart::Bytes(bytes) => match std::str::from_utf8(&bytes) {
                    Ok(text) => TypeLiteralPart::Text { value: text.into() },
                    Err(_) => TypeLiteralPart::Bytes { value: hex(&bytes) },
                },
                LiteralPart::Identity(identity) => TypeLiteralPart::Symbol {
                    r#ref: self.reference(identity),
                    origins: evaluator
                        .symbol_identity_origins(identity, consumer)
                        .map_err(AnalysisError::from)?,
                },
            });
        }
        Ok(TypeLiteralValue::Symbolic { parts })
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        text.push(DIGITS[usize::from(byte >> 4)] as char);
        text.push(DIGITS[usize::from(byte & 15)] as char);
    }
    text
}
