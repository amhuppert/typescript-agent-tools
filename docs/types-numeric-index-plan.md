# Numeric names and ordered index applicability

The operating envelope remains one immutable Project and a request-local native
checker. Property names are exact identity text. The current index-name consumer
stops before a nonempty index inventory's applicability operation; its new final
test must first record an assertion failure at that missing operation.

The shared property lookup owner will port `getApplicableIndexInfoForName`,
`findApplicableIndexInfo` and `isApplicableIndexType` from TypeScript 7.0.2
checker.go:18917–18965. String indexes are retained as fallback while every
non-string index is tested in inventory order. Multiple applicable indexes
produce the native intersection, Unknown key, conjunction of readonly flags,
and the declaration-free display parameter `x`. Exact index identity remains a
separate operation. Relations, destructuring and future indexed access consume
these complete operations rather than classify names themselves.

`isNumericLiteralName` is the equality predicate
`jsnum.FromString(name).String() == name`. A float parser alone admits too much:
`-0`, `01`, radix prefixes, whitespace and noncanonical exponents are not numeric
names, while `NaN` and both infinities are. The predicate can parse only the
canonical output alphabet because all other strings necessarily fail equality.
This is not a substitute implementation of the broader StringToNumber operation.

The parser already makes the Number.String decision inside `numeric_text`,
using local decimal formatting. Keeping a second semantic formatter would make
numeric AST keys and string index names disagree. The chosen owner is a private
numeric module shared by those existing operations. It delegates shortest
binary64 digits to pinned Ryu 1.0.20, already present in Cargo.lock, then owns
TypeScript's zero/nonfinite spelling and decimal/exponent thresholds. The
alternative ryu-js dependency carries the same f64 digit algorithm with a
different pretty-printing layer; source comparison found the d2s implementation
identical. Reusing Ryu avoids another copy of those tables and algorithm.
Scanning, radix conversion and bigint payloads retain their separate questions.

The independent adversarial review compared the Ryu tie-to-even step with the
pinned Go JSON/strconv path used by `jsnum.Number.String`. Final controls must
cover threshold boundaries, shortest-decimal ties, signed zero, roundoff aliases,
exponent signs/zeros, all represented whitespace classes and exact case. Native
lookup controls `vrun-798823c8-4a2e-48e0-9ec7-e9e375f38bbd` pass 50 names in
three profiles with zero skips. Under unchecked indexed access the final read
keeps its declared union; the other profiles distinguish actual number-index
narrowing from string-index identity. That expectation was obtained from the
compiler, not assumed from the helper rule. Product, parser and tie controls
remain required before this increment is complete.

The expanded native controls pass 58 names in three profiles and 14 numeric
literal spellings (`vrun-d421cc13-c212-4d5b-9260-6d63e929145d`, four tests,
zero skips). The public compiler API cannot marshal the nonfinite numeric
payload for Infinity; that one case uses the existing same-checker native hook
and transfers the exact summary instead. This is an oracle transport limit,
not evidence that the language operation failed.

The final Project lookup test first failed at the missing applicability
operation (`vrun-ec0274d2-04dd-474c-b8c8-58b9fb35edce`). The final AST test
then reproduced a shortest-decimal tie mismatch: native
`2.9802322387695312e-8`, old parser `2.9802322387695313e-8`
(`vrun-9aa21cda-046c-4cc6-9a38-c93e62b2b6c2`). The implementation now routes
parser spelling and exact-name classification through `numbers`, with ordered
applicability in the shared property owner. Independent source review found no
mismatch in the established branches. Post-change registered checks pass:
`vrun-14981a57-fe10-4a04-bdec-b9368ad11f11` (six final AST tests) and
`vrun-c4181473-cbaf-45bd-91f9-8ac58aa005ba` (all 58 names across three option
profiles and both expansion modes). Both have zero skips.
The numeric-template sentinel, native reserved symbol names and multiple
overlapping template indexes need their real factories and final controls;
they are not approximated by text prefixes or helper-only tests.
