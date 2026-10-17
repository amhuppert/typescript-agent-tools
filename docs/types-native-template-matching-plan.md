# Native template matching and inference

The operating envelope remains the captured immutable Project and request-local
Evaluator. This continuation implements the TypeScript Go 7.0.2 template relation
and inference decisions alongside the parent-owned template and StringMapping
factories. It is not a release boundary for the full type profile.

`inferTypesFromTemplateLiteralType` and `inferFromLiteralPartsToTemplateLiteral`
(relater.go2345–2468) share one matching owner. Retained exact text spans and
placeholder types are the input; no printed summary is parsed. Identical span
lists acquire each source and target base constraint before their independent
Assignable question. Differing spans match prefix/suffix and then the first
delimiter within source text spans. Empty delimiters consume one native Unicode
code point, including a supplementary scalar as one point. Captures crossing a
placeholder go through the actual template factory.

One alternative was to put separate matchers into relation, inference and union
normalization. That duplicates delimiter semantics and makes future fixes change
three successful answers independently. The chosen owner returns actual inferred
types and accepts the active comparer only for placeholder validation. Relation
callbacks retain their current session and native Both/None recursion policy;
inference and literal absorption use an ordinary Assignable question. Capturing
a relation kind alone would silently lose the current recursion and reliability
state. Matching never converts a failed semantic dependency into an unmatched
string.

`isValidTypeForTemplateLiteralPlaceholder` preserves intersection traversal,
the exact emptyTypeLiteral exemption, comparison before literal parsing, and
recursive template/StringMapping membership. `isMemberOfStringMapping` applies
inner mappings before outer mappings through the factory, compares actual type
identities, then checks the unmapped inner domain. Identity relations compare
exact span lists and corresponding placeholders; Comparable templates use only
the native overlapping prefix/suffix test. Ordinary template relations report
unreliable mapper observations before matching. Source constraints remain at
their native subsequent position.

`inferToTemplateLiteralType` (inference.go532–650) consumes these matches. A failed
all-placeholder extraction infers Never for each position. Constraint-based
literal conversion folds the actual ordered union constituents with the native
preference order; it does not sort candidates or select the first assignable
type. Number/BigInt alternatives are filtered by their separate round-trip rule
before the fold. StringMapping text application uses the factory's shared text
owner. Enum factories remain a separate unimplemented producer dependency and
must add their documented position in this fold when introduced.

Numeric interpretation belongs to the existing shared numbers module.
`jsnum.FromString` owns decimal/radix conversion and its narrower ECMAScript
string whitespace; it intentionally excludes scanner-only U+0085/U+200B.
`isValidNumberString` rejects empty and nonfinite results and optionally requires
canonical spelling. `isValidBigIntString` delegates lexical decisions to the
existing scanner with an appended n, forbids trivia and separators, and shares
exact magnitude conversion with ordinary bigint literals. Existing numeric
property-name round-trip queries consume this same StringToNumber owner.

The initial 87-query matrix passed in
`vrun-6f5148eb-adfd-40e9-8f5b-fda7f0ea1ff8`. Its expanded grammar matrix is GREEN in
`vrun-e03f108f-f108-4f32-bf20-5306a98c2e5e`: 216 fresh final queries across strict,
loose and exact-optional profiles, zero skips. It distinguishes canonical versus
permissive numeric spelling, whitespace boundaries, bigint lexical rejections,
the first delimiter, Unicode scalar splitting, nested template captures, and
nested StringMapping membership. The grammar additions cover signed radix
rejection, binary/octal input, huge radix rounding and overflow, decimal
underflow, all native string-whitespace categories, and bigint leading-zero,
fraction, exponent, sign-adjacency and trivia boundaries. Numeric peer review
found no concrete mismatch against the pinned conversion/scanner owners.
`tests/type_template_matching.rs` asserts the
same answers through summary and depth-zero graph requests. Rust validation and
a controlled semantic-gate counterfactual remain required after the coordinated
source checkpoint. An earlier harness run failed only project setup because its
copied file list named absent files; it is not semantic RED evidence.

The staged template factory now retains raw UTF-16 fragments through complete
normalization, including separately unrepresentable halves that join into a
surrogate pair and fragments discarded by later widening. See
[the template factory plan](types-native-template-plan.md) for pinned joined and
nested controls and the remaining final-query proof. This is a staged fix, not
an outstanding known gap for joined segments. A surviving unpaired normalized
segment still preserves the explicit exact-representation failure.

Intersection reduction for a string literal combined with a template or string
mapping remains a required next-window closure in `getIntersectionTypeEx`.
The existing explicit Pending branch must route to the shared matching owner
before this family can be claimed complete.
