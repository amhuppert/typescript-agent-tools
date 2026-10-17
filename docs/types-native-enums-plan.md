# Native enum owner

This work uses the pinned TypeScript 7.0.2 compiler and the existing TS-only,
request-local evaluator. Enum symbols are actual binder identities. No new
cross-request cache, rendered-name identity, or source-free declaration is needed.

## Representation choice

Two representations were compared. A second enum-literal view would duplicate
the established scalar literal operations. Instead, literal payloads retain an
enum flag and the actual member symbol; their interning key includes the actual
enum identity. Fresh/regular pairing is keyed by the regular TypeId, so creating
the fresh form cannot accidentally intern the payload as a primitive literal.
Computed enum types have a separate payload-free representation, corresponding
to `createComputedEnumType` (checker.go23825).

The shared factory retains signed-zero equality and canonical numeric values.
NaN is keyed by enum identity, matching the native separate NaN cache. Equal
values from different enums remain distinct; equal values within one enum reuse
the first member's type and symbol. `CompareTypes` observes enum symbols before
literal payloads (utilities.go392). Ordinary literals use the same fresh/regular
pair owner without acquiring enum metadata.

## Compiled foundation

The stage28 foundation compiled in
`vrun-fb84acd1-77bc-4cd5-bfb1-fd4e50c0f905`. This is compiler evidence, not full enum
behavior closure. Both peer reviews passed their bounded binder, cache, identity,
and widening slices.

- Binder `EnumMember` uses the actual declaration/member table owner, enum
  exclusions, and Value/Type declaration spaces. The existing enum lexical
  branch observes actual exports. Raw semantic lookup returns the member symbol;
  navigation keeps its existing non-reference projection.
- `getDeclaredTypeOfEnum` and `getDeclaredTypeOfEnumMember` own member and final
  declared slots. Enum value types use the shared function/class/enum/module
  anonymous inventory, including its enum number-index branch.
- `computeEnumMemberValues` publishes its computed flag before the member loop.
  The value slots hold each member's own completed calculation. The cold getter
  returns the full actual computation prefix and projects the selected native
  value without reacquiring its stored evidence. A warm/reentrant getter reads
  the actual member slot, including its native zero-initialized absent value.
  Already published state survives interruption.
- Constant evaluation carries completion through unary/binary/template/entity
  recursion. Const/ambient absent constants skip expression checking; ordinary
  computed values check the initializer and the numeric assignment relation.
  Ordinary compiler diagnostics do not invent an errorType value.
- `getWidenedLiteralType` is now an evaluator operation. Fresh enum members
  acquire the actual parent declared type; regular members retain their
  identity. Union children use the existing completion-aware map operation.
  All eight former direct model callers consume this whole operation.

## Stage29 semantic continuation

The staged display owner follows nodebuilderimpl.go3232: parent name first,
parent declared-type equality second, exact member name third. Identifier text
appends a type reference; other member names convert the actual TypeReference
or ImportType to typeof indexed access. The current diagnostic context has
maxExpansionDepth=0, so enum unions retain their native names. Q4 uses the same
name operation; graph payloads/origins remain separate observations.

`isEnumTypeRelatedTo` now owns directional symbol-keyed completion entries.
Parent normalization and exact name/RegularEnum gates precede lookup. A quiet
failed hit replays its installed comparison; reporting failed hits rerun without
replaying the old evidence. Target value acquisition precedes source inventory,
then source/target member values and diagnostic displays follow native order.
The entry is published only after a completed comparison, including the display
work native schedules before failure installation. Structural cache/session
metadata remains separate.

`formatUnionTypes` moved from Model into one evaluator completion operation:
boolean/enum group coalescing and nullable order now retain actual enum-parent
acquisition. Diagnostic and Q4 consumers share it. `isTypeSubsetOfUnion` likewise
moved whole to the evaluator; all five flow/excess callers use its actual parent
question. Existing Result flow boundaries admit explicitly.

Other staged consumers include Enum/EnumMember selection, distinct enum source
checks, mapped numeric-enum index normalization, template inference preference,
containment's EnumLiteral-union skip, const assertion qualified Value resolution,
keyof's enum-number-index identity exclusion, and own-enum unchecked indexing.
The private access lane consumes the shared exact const-enum-object predicate.
The native header lane owns ConstEnumOnlyModule flags, paired symbols, and actual
raw headers; no declaration approximation is admitted here.

Final proof is pending the coordinated Rust gate. The source checker’s ordinary
grammar/export diagnostic infrastructure and broader existing Result-only
subowners remain their own transport obligations; enum value/type production
never fabricates successful values for an interrupted prerequisite. Obsolete
binder enum-name lists can be removed only under the header owner's coordination.

## Native controls

`fixtures/types/native-enum-types` and
`tools/type-native-enum-types-oracle.test.mjs` contain 38 fresh-checker queries in
strict, loose, and isolatedModules profiles. They compare the hook result with
the ordinary pinned API before recording maintained expected rows. The matrix
covers automatic/string/computed/ambient/const values, duplicate and special
numeric values, local/self/later/foreign references, const/ambient checking
skips, two ordered bad computed initializers, declaration merges, value-object
inventory, fresh/widened/contextual forms, and numeric assignability controls.

Diagnostic lists establish membership, not acquisition order. Order assertions
must come from the native operation schedule or an observer that reads only
already-acquired values. Repeated independent Project requests are isolation
controls, not cache-hit evidence. Add same-checker enum value/declaration/relation
cache controls before claiming warm or reentrant behavior proven.


The stage28 capture `vrun-5492ef75-3f64-4a09-8e05-8d720c34212c` matched all
114 hook/API rows; its three expected files are installed verbatim.
`tests/type_enum_types.rs` checks healthy summaries in both expansion modes,
actual unresolved initializer locations and later-member continuation, and the
ordinary overload recovery distinction. The separate
`native-enum-relations` fixture has 32 roots across strict/loose/unchecked
profiles. It covers directional same-name compatibility, values/opaque/const
flags, repeated relation successes/failures and reporting, plus enum indexing,
keyof, mapped names and template consumers. Each marker starts a fresh query;
only repeated expressions inside one marker are warm-checker evidence.

The maintained enum114 matrix passed all three profiles in
`vrun-0682bad1-55c2-45b1-a2d4-dbd7a3552885`. The same run matched all 96
relation/consumer hook results with the ordinary API; those three captured
baseline files are installed verbatim. `tests/type_enum_relations.rs` retains
the distinction between the quick single-signature declaration initializer
(`reportValues`, `reportMissing`, `reportOpaque`) and Normal checking through a
comma expression (`reportTwice`). The former return a healthy literal without
applicability; the latter consumes two separate failed calls. Native diagnostic
codes corroborate that difference but are not used as an acquisition trace.

The final relation test also distinguishes same-enum reverse indexing from a
different enum with the same member value under `noUncheckedIndexedAccess`,
const-enum string-literal syntax from a string-valued assertion, and repeated
success/failure comparisons inside one query. Normal missing-member and opaque
reporting controls still need an additional native capture; the quick controls
must remain as healthy twins. Rust validation of this semantic continuation
and its new final tests is pending the coordinated arena checkpoint.

## Stage32 measured follow-up

The first arena build passed (`vrun-eb7ae72f-d24b-4205-8c6b-95e13e536a27`).
The following query checkpoint passed const-index recovery and repeated enum
reporting, but the ordinary relation matrix stopped at `ownReverse`. Its cause
was the existing blanket qualified-constant interruption in
`getAccessedPropertyName`, not enum relation or binder resolution. The whole
`tryGetNameFromEntityNameExpression` worker now uses the existing Value resolver
for both identifiers and qualified names, tests constant-variable/EnumMember
eligibility, reads annotation before the initializer branch, and preserves the
before-use and binding-element exclusions. An enum member without an initializer
uses `TryGetTextOfPropertyName` at that branch; consulting its enum numeric value
would answer a different native question. The worker returns completion and
the existing flow admission wrapper remains available during flow migration.

Two relation entry gates still used the narrower Literal flag. Both now use the
existing `isFreshLiteralType` model operation, including computed enums, before
identity/cache work. This changes no enum factory or interning rule. The expanded
46-root native fixture adds Normal missing/value/opaque reporters alongside
their healthy quick twins, computed fresh/regular identity and argument controls,
and qualified/automatic key variants. Capture
`vrun-50dd78c4-f82f-4994-b7f7-f923dd6fce93` matched all 138 hook/API queries;
the only failures compared the expanded rows with the old 32-row files. All
three captured baselines are installed verbatim. Computed fresh/regular identity
is `"same"` in both directions, the distinct computed enum is `"different"`,
and the overload controls select `"computed"` versus `"fallback"`. Normal
reporters preserve native `0` while each actual call reports 2345; the final
Project test requires one OverloadRecovery event per actual call, without
additional causes or omissions. The existing 114 enum and 96 relation controls
passed the stage32 Rust checkpoint. The expanded final test and maintained
138-query rerun remain pending; the earlier controls do not prove the additions.
