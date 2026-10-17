# Native non-null assertion checking

TypeScript 7.0.2 checkNonNullAssertion/checkNonNullChain (checker10598) is distinct
from the nullable-use diagnostic helper checkNonNullExpression. This matrix
covers ordinary removal, strict Unknown/Any, optional-chain outer/inner/new-root
propagation, parenthesis termination, optional call, and actual recovered operand
continuation. Filter-origin fixtures separately cover Never and scalar removal.

Every root uses ordinary API and checker-hook parity; no explicit question override
changes the final anchor. Dependency order derives operand-first checking and the
comma operation, not the native sorted diagnostic array. Maintained comparison
`vrun-8365ab14-f22c-4bf5-af2a-362cd8da9854` passed all 33 roots across strict,
loose, and exact profiles, with zero skips. Final Project assertions are staged
in `tests/type_non_null_assertions.rs`; their coordinated Rust run is pending.
