# Tagged-template global fallback and later annotation

Pinned TypeScript 7.0.2 baseline type selection performs the enclosing tagged
call's checking prelude before answering the selected literal's own question.
The healthy tsconfig.json supplies globals.d.ts; missing-globals.json uses the
same positions.ts and overrides files to omit all globals. The source contains
an astral character before the annotation, so Go byte offsets and API UTF-16
positions must be converted explicitly.

Registered proof vrun-7a19fed5-3833-497c-bcfa-75fff9c8f770 executes seven separate
baseline native/API pairs, zero skips: all six healthy enclosing-call examples
and the missing tagged-call control. Both tagged literals return the healthy
expression type "tagged". The actual TemplateStringsArray getter returns its
real target under healthy inputs and the existing emptyObjectType sentinel when
missing; both returned values have errorType=false. The getter has arity=0 and
reportErrors=true, as initializeClosures checker.go1075 specifies. The other five
preludes never execute that getter. Missing lookup records TS2318, then signature
checking acquires the missing TemplateStringsArray parameter annotation and
records TS2304. The three error exits are one getTypeFromTypeNode acquisition and
two getTypeOfSymbol observations; they do not represent three distinct leaves.

The isolated overlay changes only getGlobalType's return binding and adds a
defer that reads the already-produced pointer, existing sentinel pointers,
flags, ID and the pure isErrorType predicate. It neither asks a checker question
nor generates an ID. Existing AgentOracleObservation stores the bounded first
witness. Shared tools/type-oracle.mjs, compiler-hook fixtures and pinned compiler
source were unchanged. captured-tooling contains the immutable runtime import
closure used by the isolated driver and the three unchanged hook files;
captured-tooling-hashes.json hashes those bytes. original-input-hashes.json
preserves the original isolated-copy inventory, and isolated-oracle-sha256.json
records the instrumented oracle. observations.json preserves actual source/input
hashes, getter values, preludes, diagnostics and later acquisitions.

This evidence establishes native continuation after the global fallback. The
product's global recovery remains an invalid dependency even when the selected
literal itself is healthy. Its complete tagged-argument producer must retain the
actual empty object, then allow the later annotation acquisition before product
admission. It does not justify deleting recovery or treating diagnostics alone
as native errorType. At capture time the public missing-control test covered only the global cause.
The strengthened assertion requires the later exact annotation cause after that
global cause; its subsequent implementation proof is recorded below.

## Public repair checkpoint

Registered public RED vrun-8fd55be7-f4a1-4d84-a8b6-b3d112e9c050 compiled the
connected initializer, passed the four healthy constant-position cases, then
failed the strengthened fifth case at its later annotation assertion. The error
retained only the global TemplateStringsArray cause, proving that the argument
producer admitted recovery before signature checking could continue.

The bounded repair changes effective_tagged_template_arguments to its completed
native operation. It reads global_object_type_completion with the actual
template location, retains the real recovered empty-object value, then builds
the existing synthetic context node and captured substitutions. Its caller
forwards the full completion. NativeSequence preserves the global primary and
later error order; ordinary interruptions still stop the operation. No fallback,
source location, checking mode, signature policy or global getter changed.

Public GREEN vrun-facbabb3-6744-4ad7-9cba-88d4c20ada88 passes all five
constant-position tests, including the exact later annotation cause after the
source-free global cause, repeated compact/expanded requests and an unrelated
healthy literal. Generated literal ordering also passes in that run. The run's
separate JSON initialization failures remain owned by the unfinished initializer
adapter and are not covered by this bounded tagged repair. Independent adversarial source review using the Ousterhout skill returned
SOURCE PASS with no substantiated finding for the two bounded producer/caller
hunks and the zero-demand overlay. review.md preserves that verdict and exact
reviewed hashes; it does not claim whole-initializer acceptance.
