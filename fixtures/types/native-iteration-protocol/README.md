# Native iteration protocol

Pinned source: TypeScript 7.0.2, typescript-go commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.

The maintained harness selects actual binding names and for-of declarations.
Each query uses a fresh compiler. The three main profiles cover strict null
checking, unchecked indexed access and exact optionality independently. The
legacy profiles distinguish missing, wrong-kind and wrong-arity global Iterable
identities. Those quiet identity probes legitimately choose the legacy array
operation; they do not themselves consume a global recovery.

The protocol file gives the actual global Symbol value literal-valued iterator
properties. Native `getPropertyNameForKnownSymbolName` therefore chooses `iter`
and `asyncIter` as the actual keys. This avoids conflating iterable-protocol
coverage with the separate computed unique-symbol/late-binding producer. It is
not an implementation convention: the compiler performs the same Symbol value
and property acquisitions for these declarations.

The zero-demand observations inspect only already-computed native values:

- `checkedIteration:*` distinguishes genuine Any from completed nil followed by
  the native `checkIteratedTypeOrElementType` Any fallback. A public summary can
  be healthy even after this fallback, especially a tuple's later indexed read.
- `iteratorMethod:*` records the actual optional yield/return/next fields after
  native `getUnionType`. In particular a singleton nil return remains nil.
- `asyncFromSyncIteration` distinguishes absent fields from rejected nonnil
  awaited operands and records the resulting Any fields.
- `iteratorMissingValue:*` records the actual missing-value Any producer.
- `iterationAwaitedRecovery:*` records the nil awaited result before its
  native Any fallback. A nonnil input rejected by awaiting is distinct from
  an absent yield/return field.

Expected acquisition order comes from checker6074–6705, not from diagnostic
arrays (which are sorted by source position). Normalized iterator constituents
precede origin-filter repeats; iterator methods visit next, return, then throw;
each method visits each parameter before its return. Yield done-filtering and
value acquisition precede the independent return done-filtering pass. Native
deferred diagnostic callbacks are not run just to produce a queried type.

The implementation uses the existing property, signature, relation, awaited
type and global factories. A separate syntax-based array shortcut was rejected:
it would erase Symbol-key demands, method overload ordering, async adoption,
optional nil fields and checked recovery. One protocol operation owns those
decisions, with invocation prefixes outside its installed cache and local
NativeCompletion sequencing inside it.

The `asyncOnlyReturn` row is a pinned native invariant failure. Its synchronous
iterator's `next(): IteratorReturnResult<number>` produces a nil yield field.
`getAsyncFromSyncIterationTypes` passes that nil into `getAwaitedTypeEx`, which
reaches `isAwaitedTypeInstantiation` and panics. Both the instrumented checker
and the ordinary public API reproduce the same frames; the harness asserts
the panic rather than skipping the query or accepting an invented Any result.
Project returns `UnsupportedSemanticForm` at the actual for-await RHS for this
case. That boundary is separate from resource exhaustion, missing producers,
and nonnil awaited operands that legitimately recover to Any.

The control uses `cases.ts` SHA-256
`63520de4e5bbaaa0285a89c9cb867bf6c1a11fd8c8e63998b52f53e616b8a86b`.
The pinned unmodified `internal/checker/checker.go` SHA-256 is
`32384dde486c33c59c5695505f4c951c3009c6d7e50589cb5b18d0ea92cbf161`.
The public API's relevant frames are checker.go:31142
(`isAwaitedTypeInstantiation`), :31032 (`getAwaitedTypeNoAliasEx`), :31015
(`getAwaitedTypeEx`), and :6425 (`getAsyncFromSyncIterationTypes`). The hook
has shifted line numbers because its observations are added through an overlay;
it does not modify the pinned source tree.

`asyncBadThen` distinguishes a second invariant: async-from-sync adoption
passes a nil diagnostic message to `getAwaitedTypeEx`. For an invalid thenable
with a nonnil error node, native `NewDiagnosticChainForNode` dereferences that
nil message. The control asserts that panic as well; Project preserves any
required rejected-this displays and then reports `UnsupportedSemanticForm`.
The ordinary async method resolver supplies a diagnostic message and can
legitimately consume nil→Any (`asyncBadReturn`). Those two caller policies
remain distinct in the shared awaited-type owner.
