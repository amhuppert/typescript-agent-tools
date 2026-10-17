# Exact ordinary native strings

The source fixture separates scanner token cooking from later native string
normalization. Adjacent high/low Unicode escapes cover all fixed/braced
combinations. Removing a line continuation must retain two sentinel encodings:
LF, CR, CRLF, U+2028 and U+2029 are distinct source-byte cases, checked by the
harness before running the compiler. Conditional equality against the scalar
pair exposes that distinction even if a display format could hide it.

Declaration, expression, literal-type and no-substitution-template queries
cover lone high/low, reversed halves, a paired scalar and U+FFFD. Constant
template concatenation keeps separate halves; template-type construction
combines them. All four string mappings, expansion and Final_Sigma controls
consume lone/sentinel values. Enum member values and template interpolation
exercise the constant evaluator and enum factory separately.

The diagnostic fixture compares equal and unequal same-named enums, including
separately concatenated halves versus a paired scalar. Each marked argument
query runs the actual call prelude. Strict/loose and noCheck profiles distinguish
semantic value acquisition from the reporting-only quoted enum value operation.
Native hook/API parity, diagnostic codes, actual declaration metadata and
display acquisitions are captured before any Rust implementation is changed.

Exact property/module names remain a separate admission rule. Supporting these
literal values must not allow a scalar conversion to merge distinct names.
