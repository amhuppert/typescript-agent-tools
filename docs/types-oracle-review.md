# Adversarial compiler-oracle implementation review

Reviewed 2026-10-10 using `posd:ousterhout-software-design`, its reviewing
route, and `AGENTS.md`. The operating envelope is an evidence harness for the
selected native Rust T/Q3/Q4 implementation. The compiler overlay remains test
instrumentation; this review does not introduce a production compiler backend
or reduce the selected semantic scope.

The reviewed slice is `tools/type-oracle.mjs`, its primary and independently
owned review tests, `fixtures/types/compiler-hook/*.go`, the compiler-limit
generator, and `docs/types-oracle-evidence.md`. Pinned private owners were
checked against TypeScript 7.0.2 source commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.

**Verdict:** the bounded same-checker observations are useful evidence after
the concrete fixes below. Checkpoint 1 remains open. These observations do
not prove complete member candidate accounting, demanded-recovery causality,
the full Q4 result contract, or the representation gate. In particular,
representative positive answers must not be presented as complete T/Q3/Q4
implementation evidence.

## Findings and dispositions

### P1, resolved: replay omitted semantic inputs

The first hook implementation copied program source text and the leaf config
into a virtual filesystem. It did not replay ancestor configuration or package
manifests. Its serialized compiler options were not used to enforce equivalent
configuration. A native NodeNext import whose conditional package export has
type `"import"` became `any` in the hook. This was a successful answer from a
different program, so the discrepancy could have corrupted later reference or
limit conclusions.

The capture owner now freezes the native API's actual file reads, including
config ancestors and package manifests, and realpath mappings. Only
ENOENT/ENOTDIR mean absence; other I/O failures propagate. The Go replay checks
resolved compiler options, the exact non-library source inventory, and config,
program, syntax, and binding diagnostics before a demand. It fails when those
premises differ. Case policy comes from the pinned API's canonical filename
operation rather than a host-platform guess.

The independent tests assert final native and hook type outcomes for inherited
`strict:false` and conditional package exports. The package assertion failed
with `any` versus `"import"` in registered RED
`vrun-ad04891e-eaa7-4c96-8b0c-cf32f1311e6a`. Both cases pass in the six-test
review GREEN recorded below. Filesystem replay equivalence for the complete
case/symlink/glob/package-resolution matrix remains a separate obligation.

### P1, resolved: shorthand selection changed the question

The JS selection owner correctly used `getShorthandAssignmentValueSymbol`,
but the Go adapter reacquired `GetSymbolAtLocation` at the same token. In
`const value = "fixed"; const object = { value }`, the value symbol's type is
`"fixed"`; the object property type is `string`. The hook therefore answered
a related but different question.

Selection now transports the explicit alias/shorthand symbol mode, and Go
uses the corresponding compiler operation. A missing selected kind/span is
also fatal instead of falling back to a nearby token. This keeps syntax
selection and compiler question ownership together rather than requiring
callers to recognize special token roles. The independent shorthand assertion
confirmed RED `vrun-a9bb7655-9e52-4757-8765-36192a81aec0`, then passes in the
review GREEN.

### P1, resolved: candidate order and schedule provenance were overstated

Go originally re-sorted candidate nodes by absolute UTF-8 filenames. The
design orders exact normalized project-relative paths and UTF-16 spans; an
outside-root `../zzz.ts` and an inside-root file demonstrate a different
order. Astral and BMP filename ordering can also differ between UTF-8 and
UTF-16 comparison. The adapter now preserves the candidate owner's supplied
order. The main regression pins both dimensions, and the independent review
pins the resulting outer-call prelude order.

The result also initially claimed relationship-before-candidates from a
configured history rather than actual callback execution. Cold mode skipped
the inserted callback. Current output records the actual search-set callback
count and exact demanded node sequence; the baseline claim depends on a
callback having executed. The insertion is immediately after the pinned
`populateSearchSymbolSet`/`createSearch` operation, using `state.checker` from
the real directional reference owner. Cold observations make no baseline
claim. Diagnostics-first hook history is explicitly rejected until equivalent
same-checker execution is demonstrated; unknown histories fail.

The implementer's registered RED `vrun-8edb6c74` contained the intended
missing-trace and exact-depth assertions, and its main 15-test GREEN
`vrun-0e302170-6c67-4577-9b38-b816f4de1492` executed every test with zero
skips. Public `memberOracle` continues to identify its separate LS checker as
unproved shared state. Neither public nor hook results silently substitute
whole-file semantic diagnostics for the baseline; diagnostics after the demand
are opt-in additional work.

The exported hook also initially treated an unknown operation as a type
request. Registered RED `vrun-72e511fa-d189-4c9b-a5de-079864299877` passed
14 tests and failed the expected rejection assertion. A single operation
dispatch guard now rejects that input before checking. Final main GREEN
`vrun-6d5de296-0624-4fff-949b-2947e1a6f77b` passed all 15 tests with zero
skips, including this guard, fixed fixture paths, and an exact non-ASCII
UTF-16 source-span bridge.

### P2, resolved: a commit pin admitted untracked compiler code

Checking HEAD/tag and tracked diff did not exclude untracked Go source that
`go test` compiles. The current gate rejects all nonignored untracked files
and relevant ignored Go build inputs before execution. The independent test
uses a disposable shared clone with an extra checker source and a Go marker
shim, asserting rejection before Go is invoked. The cached pinned source is
never modified.

The original exploratory clone invocation timed out during compilation; that
is not a valid RED proof of this source-validation behavior. The final marker
regression passed in the registered review GREEN. Missing source/tag, tracked
changes, missing unique overlay anchors, compiler version/setup failures,
process errors, missing output, and malformed JSON remain failures in the
reviewed code. Existing CLI tests assert no answer on missing compiler and
wrong API version.

### P2, resolved: fixture URLs were used as filesystem paths

Hook attachments used a file URL's `pathname`, which retains percent encoding.
A valid checkout name containing spaces, non-ASCII text, or a literal percent
therefore supplies nonexistent filenames to the Go overlay. The independently
owned relocated-module regression checks a native positive control and the
final hook answer from `oracle fixtures é %`.

Registered RED `vrun-ad10d7e7-0163-4f2c-b87c-503c377f71b6` passed the first
six tests and the relocated native positive control, then failed the final
hook-answer assertion on Go's ENOENT for percent-encoded fixture filenames.
The three attachments now use Node's existing `fileURLToPath` operation.
Registered GREEN `vrun-954c43f1-09d2-4ca7-afbf-e55ff7e0d659` executed and
passed all seven tests with zero skips, including the final relocated answer.

## Remaining required evidence

The current eligibility inventory includes named property/method/accessor and
enum declarations, property-access names, and direct literal element keys.
It excludes libraries and external search files. It does not cover the full
parameter-property, shorthand/dual-role, computed-literal, JSON, JSX,
escaped-key, private-key, and unique-symbol matrix. The real compiler reference
owner then scans the complete program, which can produce raw groups or later
guard work at sites outside the custom scheduled inventory. Those raw groups
are compiler observations; they do not establish the product's complete
candidate schedule or coverage contract. Keep raw groups/search roots separate
from normalized comparisons, then expand the shared eligibility owner and
prove the full directed seed/candidate matrix before closing this gate.

Counter maxima and deduplicated guard events describe the fresh process's
operation history. They do not identify which final root, child, candidate, or
rejected overload consumed recovery. The head-recursive tuple fixture is
particularly useful: its demanded child reaches depth100 while the final
tuple root is not the compiler error type. Root error flags alone therefore
cannot establish atomicity. Add causal demand evidence before mapping these
events to product invalid results or located candidate gaps; do not treat an
ordinary false relation or an unrelated rejected candidate as fatal.
The initial snapshot followed `TypeToString` and included summary formatting.
The final harness now copies the counter map and failure slice immediately
after the selected type operation and before `TypeToString`, returning that
`beforeSummary` phase separately from aggregate observations afterward. The
copies cannot change when later work updates the global observations. The
Head47/48/49 regression proves depth99/100/100 and the corresponding empty or
instantiation-failure event before summary construction; Head48/49 still have
non-error tuple roots. That rules out summary construction as the source of
these particular guard events. It does not establish arbitrary root/candidate
causality or distinguish unrelated rejected-overload work within the earlier
phase.

Exact depth99/100 boundaries, conditional tail999/1000, and union
cross-product99999/100000 are now represented. Subtype estimate>1,000,000,
relation depth100/allowance, active-key and
deep-nesting Maybe/cache behavior remain unproved. They need exact
under/at/over operations and final outcomes, including legitimate shallow
inputs that perform extensive compiler work. A new broad recursion limit
would not satisfy these distinct compiler decisions.

The natural count fixture distributes `Build<V,V>` across
`900|901|902|903|904|905|906|907|908|909|910|885|26|2`, with two, three, or
four surrounding identity aliases. The generator neither seeds nor resets
checker state. Two identities observe a maximum guard-entry count4999999,
no event, and the exact ordered numeric union. Three and four observe
count5000000, an instantiation-limit event, and an error root displayed as
`any`. All these counter/events also occur before summary construction.
The counter is instrumented before the guard: a successful operation may
increment to internal count5000000 without another guarded entry, so the
under case must not be described as a proven final internal count4999999.
The over case remains capped at the guard, as the compiler rule requires.
This proves the natural guard boundary for this family, not count reset
placement, cache-hit behavior, or arbitrary request/candidate sharing.

The hook's private late-member normalization and directional search prove
selected computed/named and contextual-overload cases. They do not prove all
static/instance, union/intersection, generic provenance, inheritance,
shorthand, parameter-property, JSON, and JSX roots. Q4 selector/name forms,
finite graph/backreference behavior, predicates, read/write distinctions,
traversal budgets, supported option journeys, and concrete corpus journeys
also remain checkpoint obligations.

## Validation record and design assessment

The independent initial six-test suite passed with **six executed, six passed,
zero skips** in `vrun-60ca2a21-44a1-423f-b00a-73e6b7a8f49e`. After the
path regression and fix, the final suite passed with **seven executed, seven
passed, zero skips** in `vrun-954c43f1-09d2-4ca7-afbf-e55ff7e0d659`:

```sh
HARNESS_REQUIRE_COMPILERS=1 cctl validate run harness-test --queue-if-busy --require-match -- tools/type-oracle-review.test.mjs
```

This is bounded compiler evidence, not a full repository test-pass claim. A
transient Go compile error during an in-flight edit and the exploratory timeout
are not counted as behavior-level RED evidence.

The phase/count follow-up confirmed desired RED
`vrun-64b10fcb-fd97-4179-96f9-ca3a27ff95c9` on missing before-summary fields.
Final registered GREEN `vrun-fd9885db-7854-454f-9278-57cead88017e` was read
through `cctl validate status`: **16 executed, 16 passed, zero skips**, one
test file matched. The reviewer inspected the phase copies, count source,
assertions, and unchanged guard instrumentation without repeating the suite.
All 24 manifest inventory hashes match their current files, all nine generated
sources match regeneration, all 37 marker anchors and source hashes match,
and the two pinned Go module lock records and library provenance match. The
reviewed current hashes are recorded in
`.cc/temp/types-oracle-review-hashes.json`.

The current boundary is appropriately small: one native API selection owner,
one captured replay input, and a test-only overlay of named compiler
operations. Actual roots/groups, demand ordering, and work counters are
inspectable; the adapter does not invent semantic equivalence by display
names. The major design risk remains interpreting an incomplete observation
as a complete result. Preserve explicit histories, raw evidence, fatal premise
checks, and the open checkpoint gate while extending this harness. Do not add
a generalized compiler backend abstraction or weaken the selected native
contract to avoid the remaining work.
