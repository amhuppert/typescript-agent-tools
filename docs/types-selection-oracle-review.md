# Q4 selection oracle review

Independent bounded review, 2026-10-10, using the Ousterhout software-design
skill's reviewing route. The envelope is the repository's local test-only
TypeScript 7.0.2 oracle: one captured disk project and one fresh checker per
request. This assesses selection and observations, not the native Rust checker,
Q4 production integration, type graph, binding migration, or a completed proof
gate.

## Findings and disposition

1. **Renamed alias source sides bypassed the Q4 alias question.**
   `selectType` recognized only `parent.name`, so the source name in
   `import type { Widget as Local }` asked `expression` while the local name
   asked `declared`. Ordinary renamed imports also returned the correct-looking
   constructor summary under the wrong question. Both renamed sides belong to
   the Q4 resolved-target meaning table; Q3 retains its separate lexical roles.
   The shared owner now recognizes both `name` and `propertyName`, follows the
   alias, and applies the same clause/specifier type-only decision. The corrected
   matrix covers eleven renamed import/export pairs, including type-only
   functions and variables, plus default, namespace and import-equals forms.

2. **Type-query operands inherited a generic type-syntax question.**
   The simple `typeof Local` operand selected `type`; a qualified operand could
   be refused as an earlier qualified type-reference segment. A matching
   `typeof Local` summary concealed the wrong question. The existing selection
   owner now recognizes type-query operands before the narrower type-reference
   rule. Type-reference substitution and earlier-segment refusal stay distinct.

3. **The parameter-property modifier had no Q4 member anchor.**
   Its first `public` modifier was rejected as an untyped keyword, contrary to
   the table's explicit member anchor. The owner now uses the existing candidate
   role operation and the Go hook's native parameter-property symbol pair.
   A parameter property's local and member symbols can share a type and origin,
   so those observations do not establish role identity. After observation wiring,
   a separate RED proved the name still chose member flags `4` and an empty mode.
   The shared selector now attaches both modes through the same role operation;
   the Go demand uses the existing native pair owner. Final controls observe
   local flags `1`, member flags `4`, the value question, and the exact role mode
   for public/private/protected/readonly/override/mixed first modifiers. A
   secondary modifier is unsupported. The public compiler API explicitly rejects
   both unavailable pair roles; it does not substitute one symbol for the other.

4. **Supported punctuation anchors failed an unused symbol operation.**
   Closing `]`, closing `)`, and `new` without parentheses selected the correct
   containing expression, then the public oracle called `getSymbolAtLocation`
   on a synthetic punctuation token. The API rejected it with
   `getNodeId requires a RemoteNode`. Raw compiler requests on the actual access,
   call and new-expression nodes, and the same-checker Go replay, returned
   `number`, `"result"`, and `Local`, all with a valid root. The remedy belongs
   to the complete selection/demand operation: use the selected real expression
   node when a symbol observation is needed. The shared owner made that change;
   all three final requests pass. Callers do not need to know which tokens the
   API synthesizes.

5. **The Go hook masked unresolved alias error types.**
   `oracle-test.go` compared the result with `GetErrorType()` by identity. For
   the containing `Missing` type-reference node it reported
   `summary: "Missing", errorType: false`; the public native API reported true.
   Pinned `Checker.isErrorType` also recognizes manufactured unresolved alias
   types (`checker.go`, function `isErrorType`), and the public API documents
   the same cases. Expose that existing private predicate through the
   checker-local observation shim. The shared owner now does so; the final
   unresolved alias control records true. It adds no second error-type rule and
   does not interpret the displayed summary as validity.

6. **Type-hook output could not prove the executed question and role.**
   The output exposed only summary and singleton-root validity. A class's
   constructor type can appear under an incorrectly selected expression
   question; parameter and member types can be identical. The shared owner
   exposes the selected question returned after the Go demand, the actual selected
   symbol flags and the role mode without adding checker demands. The native
   question switch is retained in the hook source; expression and type both use
   `GetTypeAtLocation`. This is evidence wiring, not a new production API.

These remedies pull complete decisions into their existing owners (Ousterhout
P10), keep failures visible (P11), and expose the distinctions that affect proof
(P13/P16). A second selector or copied error predicate would increase change
amplification. Keeping one selector does not mean collapsing Q3 lexical roles,
Q4 inspection meaning, qualified type-query lookup and qualified type-reference
selection into the same question.

## Compiler facts and product rules

The expected questions come from the design's table. Native explicit-question
controls establish the actual declared/value types; auto-selection must use
that question. Explicit experimental question overrides bypass auto-selection
and therefore are unsuitable controls for followed-alias origin sets. The
tests instead require both auto-selected alias sides to retain equal followed
target origins in `exports.ts`.

Class declarations ask for the instance type; ordinary imported classes ask
for the constructor type; type-only aliases to both meanings ask for the
declared type. Type-only aliases to value-only functions and variables still
expose their value type. External namespace aliases ask for the module's value
type, including a type-only module with no value properties. A noninstantiated
internal namespace has no supported value anchor.

The final name of `Active.Box<string>` deliberately selects the containing type
node, preserving substitution; an earlier namespace segment is unsupported.
The `typeof` operand uses its value question. Literal element keys retain their
literal type while `]` selects the access result. A callee retains its callable
type while `)` selects the call result. A tagged template's final backtick and
`new` select their containing expressions. Constructors, signature catalog
positions, module blocks and JSON positions remain explicit unsupported targets.

One initially expected result was wrong: raw `GetTypeAtLocation` on the numeric
operand of `export default 42` yields an error type displayed as `any` in both
the native API and Go hook. Pinned `IsInExpressionContext` omits
`ExportAssignment`, and `getTypeOfNode` falls through to the error result.
The test preserves that invalid observation; it does not invent an expression
checking normalization or treat `any` as a positive answer. The design's
export-default wording does not by itself prove every literal operand is a
valid native demand.

## Reproduction and coverage

All test iterations use the registered single-file command:

```sh
cctl validate run harness-test --queue-if-busy -- tools/type-selection-oracle-review.test.mjs
```

- Initial three behavior failures:
  `vrun-e9b69da3-6ce2-4209-b1a1-4671992251e3` (3 executed, 3 failed, 0 skipped).
- Broader initial matrix:
  `vrun-3d29be01-a60b-460b-a631-c85413d34c78` (9 executed, 7 failed, 0 skipped).
  This run included the subsequently corrected test assumptions described above.
- Separately pinned punctuation failures after the first owner fixes:
  `vrun-388f5e12-fa9b-476c-b449-a66aa6e103aa` (16 executed, 6 failed, 0 skipped).
- Precise remaining failures, with corrected expectations:
  `vrun-60abb32a-d878-4efb-9124-924ca19f049e` (18 executed, 13 passed, 5 failed,
  0 skipped): three synthetic punctuation demands, missing question/role
  observations, and unresolved alias invalidity masked in the Go hook.
- Actual role failure after pure observation wiring and the other owner fixes:
  `vrun-524cc654-dfb1-4e6e-ba26-4a98aac8d104` (18 executed, 17 passed, 1 failed,
  0 skipped). The parameter name had the value question and correct-looking
  `string` type, but actual member flags `4` instead of local flags `1`.
- Final expanded suite after the role fix:
  `vrun-eb9d186c-c7c2-4077-8e25-d7240986b3c8` (one file matched, 20 executed,
  20 passed, 0 skipped), with `--require-match`.

The matrix also covers positive/negative root flags, genuine `any` in fresh
cold requests, getters/setters, shorthand precedence, enum declaration/member
and value use, nested enclosing calls in outermost-first baseline order, and
cold/diagnostics-first histories. A repeated cold query after a result demand
still begins fresh. Thirty generated source variants combine five compiler
line-break forms, space/tab/NBSP, and leading BOM absence/presence; final Q4
results preserve UTF-16 locations after astral text and reject exact whitespace
and comment positions.

The isolated native control is `.cc/temp/type-selection-probe.mjs`, with output
`.cc/temp/type-selection-probe.json`. It walks native AST nodes and queries the
actual containing expression for the punctuation controls. Those throwaway
probes are distinct from registered test evidence; they do not prove the full
oracle or native implementation.

## Current verification boundary

All six scoped findings are addressed and the final registered suite is GREEN.
The cases demonstrate the table against compiler-backed test
requests. They do not prove graph completeness, recovery provenance below a
root, full dependency admission, Q3 coverage, service behavior, refresh, the
native checker, or migration/resource gates. The public API's role-pair
limitation and diagnostics-first Go equivalence remain explicit boundaries.

## Bounded relation instrumentation follow-up

Independent read-only review of the later relation instrumentation found no
blocking defect. The reviewed owners are `compilerHookOracle`'s exact overlay
anchors, `checker-hook.go`'s observation records and `oracle-test.go`'s snapshots.
The operating envelope remains fresh compiler research processes, not a
production cache/provenance implementation.

The overlay inserts observations at the existing native sites. It leaves
thresholds, resets, branches and cache operations unchanged. In particular,
relation cache/previous-overflow handling still precedes the allowance guard;
active-key checks precede the depth guard; stack pushes precede the deeply
nested Maybe observation. The subtype estimate retains native integer division
and strict `> 1000000`. Counter-only conversion of native unsigned instantiation
metrics is explicit; relation allowances and finish records use signed native
`int`, so final unwinding allowance `-2` is preserved.

Guard-entry maxima and first-finish records answer different questions. A first
finish is the first observed completed comparison with a nonempty relation
cache, not a guaranteed arbitrary selected root's owner. The reviewed tests
separately assert guard entry, cache commits/finish allowance, ordinary False,
overflow False and admitted Maybe. These bounded witnesses do not prove every
cache/Maybe history or relate a global event causally to every returned type.

`beforeSummary` copies the counter map, failure slice and each witness map after
the selected demand. Witness values are scalar integers/booleans in the reviewed
hooks, so subsequent observations cannot mutate that snapshot. Optional array
element inspection happens afterward through the returned type's native type
arguments and uses the existing `c.isErrorType` owner. The child flag is separate
from the array root flag. Already-demanded diagnostics come from the checker's
diagnostic collection; the hook does not invoke whole-file `GetDiagnostics`.
These observation operations are explicitly outside the selected-demand
snapshot and are not presented as zero-cost production instrumentation.

The corrected desired RED was
`vrun-498e2d8c-a033-4892-9be4-7c12f2afb2e5`: 21 passed and four missing
observation assertions failed. The owner reported one intermediate compile
failure from unsigned metric types; it is not behavior-level RED evidence.
Final registered single-file run
`vrun-11671a51-5307-423c-9693-3ad4d3e7fcee` executed 25 tests, all passed,
with zero skips. This reviewer independently confirmed its durable terminal
status. No production Rust or shared oracle file was edited by this review.

## Captured-input and BOM follow-up

Independent Ousterhout review of the later metadata/BOM change found no blocking
defect within the same fixed-fixture, fresh-process research envelope.
`captureFileSystem.readFile` uses fatal UTF-8 decoding with BOM preservation,
records the raw string under the read and canonical paths, and returns the
existing `navigation-oracle.compilerText` operation. Missing paths are tolerated
only for ENOENT/ENOTDIR; decoding, permission and other failures still fail.

The replay builder copies those captured strings first. Program text fills only
uncaptured virtual sources, so a parsed SourceFile cannot replace a captured
file's BOM bytes. Selection byte offsets use compiler text; returned Go byte
spans go through the same BOM owner before UTF-16 translation. This removes a
second BOM decision without conflating raw input identity and parsed positions.
The existing pinned bundled-library fallback remains distinct from filesystem
input capture; this review does not claim that virtual source strings were disk
bytes.

Sorted `inputHashes` are calculated from the exact frozen `input.files` strings
that the Go request replays, without a later filesystem read or checker demand.
For valid UTF-8 filesystem inputs, decoding and re-encoding preserves the bytes,
including the retained BOM. The test compares hashes to actual bytes for the
leaf config, inherited base, conditional package manifest and BOM source, and
compares final native/replay Q3 origins and candidate demand spans.

The owner preserved its missing-hash desired RED separately from the prior BOM
anchor defect. Final registered run
`vrun-179b33a5-0398-47fc-996b-0057ee79a445` executed and passed all 25 tests
with zero skips; this reviewer independently read its durable passed status.
The final JSON controls additionally assert six local raw-input byte hashes.
This does not retrospectively freeze the historical corpus's 327 unhashed
metadata inputs, prove concurrent-write replay, or close production semantic
provenance/recovery gates. No shared oracle source was edited by this review.
