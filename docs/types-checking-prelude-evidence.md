# Executable checking prelude

This Ousterhout checkpoint extends the existing immutable Project and native
request-local Evaluator. Expected behavior comes from TypeScript 7.0.2 source
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. It implements the designed
outermost-to-innermost signature prelude for established call families. Full
contextual, overload/inference, New/Tag and other semantic obligations remain
mandatory; this is not a declaration that the entire baseline has closed.

## Actual identities and complete operation

Source's existing containment operation now returns opaque CallRoots carrying
the accepted file, syntax kind and raw span. Selection retains these identities
for every role that it has selected, without evaluating a candidate. The
evaluator's `check_site_prelude` acquires each actual recipe and calls the same
`getResolvedSignature` owner used by checked expressions. It stops on the first
failure, then asks the selected seed's own question. Public positions, printed
origins and declaration names are not converted back into a guessed call.

This operation deliberately does not call `checkCallExpression` or acquire the
chosen return just because a subnode is selected. That distinction matters for
`hiddenReturn(/*selected*/3)` where the function's return annotation is
MissingReturn: resolving the signature and typing 3 is healthy; a separate
request that demands the signature return consumes its own missing annotation.
Nested resolution uses the existing actual call cache and native resolution
window. An outer arity rejection precedes the inner site's own question.

The design comparison in the call implementation plan (archived in Git history)
rejects reverse-mapping public Locations and reusing the checked-call return
question. Both would push missing scheduling/identity decisions onto callers.
The chosen complete prelude hides those decisions without adding a checker,
whole-file diagnostics pass or alternative symbol inventory.

## Native and final Project proof

`tools/type-checking-prelude-oracle.test.mjs` final three-test GREEN is
`vrun-4632d5ef-e6d5-4d66-bf4b-8098109f8cc7`, zero skips. It pins:

- 16 actual root/role cases in both option profiles, with exact outer/inner
  locations. Expression, value, declared and type questions retain their own
  native questions; generic type arguments, New and Tag are represented too.
- Seven before/after-summary schedules in both profiles. Prelude-only
  MissingReturn stays undemanded; required MissingParameter is checked. Wrong
  arity, this and scalar argument rejection have their own diagnostic schedule,
  and null applicability follows effective strictNullChecks.
- A separate function declaration summary with no prelude consumes the return
  annotation after establishing a healthy function type.

Initial native run `vrun-2c103c0b-76af-438d-b90e-9989f60d54b1` passed the
schedule test but exposed an incorrect test expectation: selecting the template
literal after a tag still asks its literal type, not the tag's return. Native
observation corrected it. Expanded run
`vrun-7548ceb4-6145-4b0a-b7c0-a050604f6e20` passed both original tests but
the added test asked for a prelude field the type-hook output does not expose.
The final test uses a separate fresh public observation for that field. Neither
intermediate run is a product defect or behavioral RED claim.

`tests/type_checking_prelude.rs` genuine compiled four-test RED is
`vrun-3cd019b7-c3a0-4d64-ad38-da13aca60a9b`: all assertions failed at the
temporary enclosing-call refusal. Four-test GREEN is
`vrun-42b95fec-d9f6-4dd5-b1d1-5fa08622e1a8`, one matched file, zero skips.
It asserts final literal graphs and locations, renamed cross-file callees,
nested calls, hidden return health, consumed parameter/return failures, native
outer arity/this/argument invalidity and strict/loose null outcomes.

After this proof, the two temporary-refusal test files were migrated. Ordinary
literal arguments now assert native Found answers; remaining constructors and
context-sensitive expressions report their actual required construction site.
`tests/type_constant_positions.rs` passed in
`vrun-dd51e74c-c4d4-4c58-8e25-179c39d3077d`; `tests/type_pending_values.rs`
passed in `vrun-a132cfab-5c30-4f32-a4e4-70dffbfe88da`, both zero skips.
The latter keeps the exact unpaired-name failures outside a call while showing
that the actual contextual object dependency precedes a selected own name.

## Invocation acquisition correction

The first compatibility gate
`vrun-2c8a4133-d891-45a9-9604-afb6cc294741` compiled and passed three tests
but reproduced an Internal error for tagged source acquisition. A tag stores
its invoked expression in `tag`, not `expression`; the newly executable
prelude reached that previously unexercised acquisition path.

The complete native `GetInvokedExpression` owner now lives in syntax and covers
Tag, JSX names, Binary right operand, JSX fragment and ordinary Expression
fields. Call recipes and lexical call-target inspection both use it. The
lexical inspection retains its existing Call/New/Tag kind guard and its own
narrower called-name question. The compatibility GREEN above proves the tag
now reaches its explicit semantic dependency rather than inventing an internal
attachment failure. Full checked Tag construction remains required.
The existing lexical filter file also passed with zero skips in
`vrun-7e9bc23b-4e17-4d61-a408-52649b97da1d` after the owner move.

## Snapshot and remaining obligations

These hashes were read together after the scoped gates. Other owners' later
changes are outside this snapshot; no global source-stability claim is made.

| Artifact | SHA-256 |
| --- | --- |
| definitions.ts | `7e4f19df7f53bfae9a16f65d5da7b082cb28e23e00b49178bf71bb104048105b` |
| sites.ts | `79eb3d419d44f7659fd317936e5f7399ab7904350d86b0f9e781e63f70a3a094` |
| tsconfig.json | `bd26f8791a2f17c179270a013e65690a429b7d18ad665e2ffbc9b9a3f15aabce` |
| loose.json | `4da10c0c1db4785b8b9fdb174282a6345e106ccdbf745bfc440a9275b62c9a7b` |
| native oracle | `9041beb3f7f9e5eeb67c55ae208522d20acc6e1cdab1f3ec9f5007b6956014d1` |
| final Project tests | `58c9d06dfa593ab292876a69cffff3210c7f32571224d2170088787b49bd9099` |
| call-source owner | `9db63bb40a5a8eb0e3d1b5741cbf5f8bbb6c7cc355e1f68b68954d29604a6eb8` |
| call evaluation owner | `f12576fd9780a919a114b793ffe4b4cda17f6575adc780323519d4c85e20dfed` |

The native baseline observes callback declarations/types and generic/New/Tag
roles more broadly than this production increment can construct. Their native
operations, actual inference/context state, receiver/rest/tuple/spread
questions, argument relations and overload reorder/retry remain required.
Unselected or unimplemented selection roles and Q3's full candidate schedule
are separate open obligations. Representation/work/resource calibration and
all compiler limits also remain open; these receipts establish no cost gate.
