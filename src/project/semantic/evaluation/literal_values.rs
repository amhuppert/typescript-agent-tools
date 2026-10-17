//! Literal text observations retain the exact payload until the actual consumer
//! needs a spelling. A failed observation is never an absent value or mismatch.

use super::super::model::{DemandError, LiteralError, LiteralPart, LiteralValue};
use super::Evaluator;
use crate::references::Location;

impl Evaluator<'_, '_> {
    /// Native string observations distinguish unavailable generated spelling,
    /// unfinished byte consumers, and an invalid internal slicing operation.
    pub(in crate::project::semantic) fn literal_value_failure(
        &self,
        error: LiteralError,
        consumer: &Location,
    ) -> DemandError {
        super::super::model::literal_value_failure(self.inputs, consumer, error)
    }

    /// A reached native string operation that still requires scalar text must
    /// report its dependency at its consumer. Identity and flag-only operations
    /// do not call this projection.
    pub(in crate::project::semantic) fn literal_scalar_text<'a>(
        &self,
        value: &'a LiteralValue,
        consumer: &Location,
    ) -> Result<&'a str, DemandError> {
        value
            .scalar_text()
            .map_err(|error| self.literal_value_failure(error, consumer))
    }

    /// `printer.EscapeString` consumes exact fixed bytes, independently from
    /// their identity. Opaque native IDs have no permitted numeric spelling.
    pub(in crate::project::semantic) fn quoted_literal_value(
        &self,
        value: &LiteralValue,
        quote: char,
        consumer: &Location,
    ) -> Result<String, DemandError> {
        let bytes = value
            .fixed_bytes()
            .map_err(|error| self.literal_value_failure(error, consumer))?;
        Ok(crate::printer::quoted_bytes(bytes, quote))
    }

    /// `isNumericLiteralName` requires the actual binary64 round-trip value.
    /// A known invalid character can reject a symbolic string first; otherwise
    /// an opaque ID's numeric spelling remains a required observation.
    pub(super) fn literal_is_numeric_name(
        &self,
        value: &LiteralValue,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        Ok(self
            .literal_numeric_text(value, consumer)?
            .is_some_and(crate::numbers::is_numeric_literal_name))
    }

    /// `isValidNumberString`: an entire GetSymbolId is positive Uint64 decimal,
    /// hence finite as binary64. Canonical round-trip inference still needs its
    /// digits: Uint64 values need not be exactly representable as binary64.
    pub(super) fn literal_is_valid_number_string(
        &self,
        value: &LiteralValue,
        round_trip_only: bool,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        if !round_trip_only && value.entire_identity().is_some() {
            return Ok(true);
        }
        Ok(self
            .literal_numeric_text(value, consumer)?
            .is_some_and(|text| crate::numbers::is_valid_number_string(text, round_trip_only)))
    }

    /// `isValidBigIntString`: GetSymbolId's positive decimal spelling is exact
    /// and canonical in the integer domain, independently from binary64.
    pub(super) fn literal_is_valid_bigint_string(
        &self,
        value: &LiteralValue,
        round_trip_only: bool,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        if value.entire_identity().is_some() {
            return Ok(true);
        }
        Ok(self
            .literal_numeric_text(value, consumer)?
            .is_some_and(|text| {
                crate::numbers::bigint_from_string(text, round_trip_only).is_some()
            }))
    }

    /// None means a proven grammar rejection. It never stands for unavailable
    /// decimal spelling or a failed Unicode projection.
    pub(super) fn literal_numeric_text<'a>(
        &self,
        value: &'a LiteralValue,
        consumer: &Location,
    ) -> Result<Option<&'a str>, DemandError> {
        for part in value.parts() {
            if let LiteralPart::Bytes(bytes) = part
                && crate::numbers::has_non_numeric_string_character(&bytes)
            {
                return Ok(None);
            }
        }
        self.literal_scalar_text(value, consumer).map(Some)
    }
}
