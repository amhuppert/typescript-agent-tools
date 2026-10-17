# Native template and intrinsic string types

Operating envelope: the existing native, local, request-scoped checker. These
operations use captured syntax and established semantic identities; no runtime
compiler or new shared cache is introduced.

The native owner is `getTemplateLiteralType`, with separate Model interning and
source capture. Keeping templates as display strings would make substitution,
pattern relations and union reduction reconstruct type information. Instead,
Model retains exact normalized segments plus actual placeholder TypeIds;
evaluation owns distribution, the cross-product limit, flattening and pattern
classification. Instantiation reuses that operation. Source fragments retain
UTF-16 until concatenation; each surviving segment must then be representable
without replacement characters. Candidate segments remain UTF-16 through the
whole normalization walk: a later invalid placeholder can widen the result to
String and discard an earlier malformed segment. Placeholder type acquisition
precedes this walk, so an unresolved placeholder still wins over malformed
text in a later segment. Unsupported literal producers remain explicit
when their own exact value cannot be retained.

`getStringMappingType` is reached only through the actual intrinsic marker and
canonical alias symbol, with the native one-argument gate. An ordinary alias
with the same name is inert. The factory key is the actual symbol and target,
with separate TypeView metadata. Native marker, numeric-string and template
constraint startup identities are preserved. `BuiltinIteratorReturn` is replaced
after the alias's parameter-cache seed; `NoInfer` uses the shared Substitution
owner. The generic string operations share template pattern classification.

Casing is one policy in `strings`: TypeScript 7.0.2's Unicode 15.1 tables, full
multi-code-point mappings and context-sensitive Final_Sigma. The generator
retains the source hash. Host Unicode libraries would create a hidden version
dependency. Capitalize and Uncapitalize consume a code point, as the pinned
compiler does, rather than half of a supplementary character.

Union construction resumes its pure Model factory after the shared evaluator
answers pattern matching, in the native order: redundant literals, string
literals matched by patterns, constrained variables, subtypes. Pattern matching
and inference share one owner and retain the active relation comparer.

Proof is in progress. The template and mapping matrices have 213 asserted
fresh compiler observations across strict/loose/exact, including surrogate
joining, later widening, Unicode casing, products, substitution, direct union
constituents and consumed error dependencies. The full matrix and its diagnostic
and demand-failure observations pass in
`vrun-9f17b49d-5d7d-41cc-9675-1e0bfe9484a2` (six tests, zero skips).
Diagnostic-only results remain distinct from consumed error dependencies.
Matching and constrained inference have 216 exact native final answers
(`vrun-e03f108f-f108-4f32-bf20-5306a98c2e5e`). The shared index-key rule has 136
native answers across four profiles
(`vrun-2a0753c0-bd5e-4b8a-bd26-48450ae1766b`), including duplicate and invalid key
value acquisition. Corresponding final public Rust queries and graph-edge
assertions are staged. Rust validation and native-limit controls remain required
before this document can claim delivery.

Final template, mapping, matching and index-query suites now pass:
`vrun-caadfce2-a114-4bfb-ac47-415dfcd39b6b`,
`vrun-509f7f03-ebbd-4ccc-8115-5c441227934d`,
`vrun-c2077c46-2147-4770-a3a3-e0b31be92d4a` and
`vrun-9d5550b3-a328-4b4a-ac1c-8023173b3b27`. Expanded queries check actual
placeholder and nested mapping edges as well as exact summaries.

Two controlled regressions prove the reviewed text boundaries. Validating a
segment at its first retained placeholder fails `lateInvalid`
(`vrun-831e1a25-f228-481c-afb1-8ed386986c10`). Restoring that operation and then
validating source fragments independently fails the joined-surrogate query
(`vrun-301d739c-1367-48b8-82cc-22d7ed31a121`). All changed sources were restored
byte for byte; the full template suite passes again in
`vrun-cf5b3ee3-5523-418b-8457-0268f6a3b79c`. Template/string intersection
normalization is staged and its 72 native observations pass in
`vrun-f7dbd5c2-f804-46c4-a37b-e7a4efc6dd6f`. The strengthened 75-question matrix
also passes (`vrun-92eb2754-de99-4416-a0e7-1f7bc1e9cdd3`), proving the reduced
union's actual members with an unaliased consumer. Final queries reach the
qualified raw-alias binding boundary for same-named namespace mappings; that
shared operation is being completed before claiming these controls pass.

The exact 99,999/100,000/100,001 template product boundary and 10,001 flat
placeholder control pass in both compilers
(`vrun-68847370-56f6-4de1-baf1-ced0192d74b1`) and final Rust
(`vrun-cd9dc4dd-86e4-4d1b-90cd-ee5213c0ac20`). Review found the separate signed
overflow branch: native returns MaxInt immediately, before examining a later
Never constituent. Saturating multiplication followed by a later zero was wrong.
The shared product-size owner now stops in native order. Safe final controls
cover 62/63/64 two-way factors without Never, overflowing factors before Never,
Never before those factors, and a small product before Never. A just-under-
overflow product followed by Never is deliberately not executed: the native
template factory would enumerate its enormous empty product. This safety limit
does not change the production rule or substitute a successful answer.

Remaining relation/display controls, signed-overflow final Rust proof and
controlled reduction/casing regressions are still required closure work.
