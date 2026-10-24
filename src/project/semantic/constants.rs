//! Compiler constant payloads, separate from checking expression result types.

/// Native evaluator `Result.Value` admits numeric and string values from the
/// source evaluator. Strings retain the scanner's exact native bytes through
/// concatenation; `NewEvaluator` does not combine surrogate sentinels.
#[derive(Clone, PartialEq)]
pub(super) enum ConstantValue {
    Number(f64),
    String(super::model::LiteralValue),
}
impl ConstantValue {
    /// `AnyToString` delegates numeric formatting to the shared jsnum owner.
    pub(super) fn into_text(self) -> super::model::LiteralValue {
        match self {
            Self::Number(number) => {
                super::model::LiteralValue::from_text(crate::numbers::number_to_string(number))
            }
            Self::String(text) => text,
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct ConstantResult {
    pub(super) value: Option<ConstantValue>,
    pub(super) syntactically_string: bool,
    pub(super) other_files: bool,
    pub(super) external_references: bool,
}

/// `getNumberLiteralType` / `getBigIntLiteralType` identify both signed zeros
/// as the same literal. Direct unary minus consumes an already normalized
/// literal payload; it does not repeat scanner radix or float conversion.
pub(super) fn negate_literal(value: &str) -> String {
    if value == "0" {
        "0".into()
    } else {
        format!("-{value}")
    }
}
