# Fresh TypeScript 7.0.2 oracle evidence

This records the bounded oracle implementation for checkpoint 1 of
[types-design.md](types-design.md). **Checkpoint 1 remains open.** These fixtures
prove selected compiler operations and histories; they do not prove the complete
T/Q3/Q4 contract, the representation gate, or production semantic behavior.

## Executable owners and version gates

`tools/type-oracle.mjs` exports `typeOracle`, `memberOracle`, and
`compilerHookOracle`. A position is a project-relative file and one-based UTF-16
line/column. Every public request creates and closes a new native API session.
The existing `loadSemanticApi` owner requires both native compiler and API package
version 7.0.2; absence and version mismatch fail without emitting an answer.
The CLI accepts a request JSON file for public type/member observations.

The hook additionally requires the pristine Go release source at
`typescript/v7.0.2`, commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.
Tracked changes and untracked source inputs fail before execution. It runs
`go test -overlay` with two jobs, adding the three files in
`fixtures/types/compiler-hook` and replacing uniquely matched counter/search
anchors. It never edits that checkout. Missing anchors, Go, build, timeout,
output, and JSON errors remain failures. Go 1.27.1 was used for these measurements.

The hook freezes all native API `readFile` inputs, not only program sources:
configuration ancestors and package manifests affect resolution. It captures
realpath mappings and the pinned API's canonical filename case policy. Only
`ENOENT`/`ENOTDIR` mean absence; other I/O errors fail. Non-UTF-8 file input is
explicitly unsupported by this replay adapter rather than decoded with loss.
Captured UTF-8 input retains a leading BOM as exact replay bytes. The existing
native `decodeBytes` source-text owner in `tools/navigation-oracle.mjs` removes
that mark for API parsing and span translation, as the Go virtual filesystem
does. API `SourceFile.text` supplies only otherwise uncaptured virtual sources;
it cannot overwrite captured disk input with its decoded form. Navigation and
JSON diagnostic position probes share that source-text owner.
`inputFiles` and sorted `inputHashes:{file,sha256}[]` describe that same frozen
replay map, without a later disk read or extra checker operation. Disk-byte
equality is asserted for the leaf/inherited configurations, conditional package
manifest and BOM source. Virtual library input hashes describe captured decoded
text; compiler/package/library identities remain separate provenance obligations.
These fields apply to new hook requests and do not repair historical corpus
captures whose metadata bytes were not hashed.
The replay rejects configuration, program, syntax and binding diagnostics,
different resolved compiler options, different non-library program sources, or
a missing selected syntax kind/span. Complete filesystem replay equivalence,
including every case/symlink/glob/package-resolution variant, remains unproved.

## Checking histories and private operations

`typeOracle` establishes one of three explicit histories before its final demand:
`baseline` resolves enclosing call/new/tagged-template signatures outermost
first; `cold` makes no prelude; `diagnostics` asks for semantic diagnostics first.
Diagnostics after the final demand are opt-in through `diagnostics:true` and are
additional work, not part of the baseline or its timing.

The public member API records raw/combined origins and sorted candidate demands,
then reports LS reference observations. It explicitly returns
`public_reference_api_uses_separate_checker`: the pinned public reference handler
acquires a language-server checker, so that API cannot prove shared request state.

The Go hook supplies one checker to seed prelude, raw lookup, the exact private
`getResolvedMembersOrExportsOfSymbol` normalization, and the real directional
`getReferencedSymbolsForSymbol` search. Its insertion runs candidate preludes and
symbol demands immediately after the real `populateSearchSymbolSet`/`createSearch`
owner constructs the initial search set. Candidate order comes unchanged from the
JS owner's exact normalized path/start/end/role sort. Raw compiler groups/roots
are retained separately from deterministically sorted normalized observations.
`searchSetCount` and `candidateDemands` report the callback operations that
actually executed; cold mode reports no executed candidate schedule. The raw LS
search still receives all compiler program files, so raw groups may include
sites outside this bounded prelude inventory. This is a compiler observation,
not proof of complete product Q3 candidate accounting.
The hook supports baseline/cold histories. Diagnostics-first equivalence in this
same-checker harness is unproved and is rejected explicitly; unknown histories
fail. Selection preserves alias and shorthand value-symbol overrides.

Q4 selection follows both renamed alias-name positions, asks a value question
for a `typeof` operand, and asks about the real containing access/call/new node
at useful closing punctuation or `new`. Synthetic punctuation is never sent as
an unrelated symbol RPC argument. Parameter-property names and first modifiers
reuse the Q3 role owner; the type hook reports the actual selected question,
symbol flags and role, rather than inferring a role from equal type summaries.
Error classification delegates to native `isErrorType`, including its unresolved
alias case. Explicit `question` overrides remain compiler experiments outside
the automatic product selector contract.

The decoded candidate owner now accounts for broad same-name identifiers,
property/method/accessor/enum names, object declarations and shorthand dual roles,
parameter-property names and their first producing modifier, computed literal
and element/indexed-access keys, JSON keys, private names and JSX names. Library
and external files remain outside this bounded inventory. A computed expression
identifier remains its own lexical name; a constant value never relabels it as
the property's key. Native meaning and related-symbol decisions run separately
on every scheduled candidate rather than being guessed from its syntactic role.

Each candidate records native raw-position validity, raw text-scan admission,
resolved symbol flags/origins, shorthand lexical origins, and directional related
origins. Escaped identifiers and strings, hexadecimal numeric keys and empty
keys can have native related origins while the pinned raw text scan misses them.
An empty search name exits the raw scanner immediately; its source also records
the Unicode-escape TODO. These paired observations prove candidate accounting
and native relation decisions for the tested forms, not a completed decoded
reference search or canonical product groups.

Parameter names and first modifiers select distinct native local/member symbols
through `GetSymbolsOfParameterPropertyDeclaration`; their declarations can have
identical locations, so origin-set equality does not identify a role. Public API
requests reject unavailable pair roles explicitly. Namespaced JSX attributes
keep the full `ns:foo` name and exclude their child `foo` from that shorter-name
inventory. An initial assumed nil-symbol blocker was disproved by a fresh final
hook observation: the declaration-name branch returns a `JsxAttribute` symbol,
and native `getRelatedSymbol` finds the property despite both raw scan predicates
rejecting the composite name. Product JSX typing remains outside the current
profile. Full candidate completeness and grouping identity remain open.

## Measured outcomes

| Fixture/demand | Fresh pinned outcome |
| --- | --- |
| Rejected-overload callback parameter | Cold/diagnostics-first `any`; outermost call prelude `string`; a later result query cannot warm the next fresh cold request. |
| Rejected-overload callback `shout` member | Baseline `() => string`, origin at the interface member; same-checker directional group includes the declaration and callback use. |
| Late computed/named `value` class member | Named raw seed has one origin; exact resolved member normalization has both computed/named origins. Named declaration and use yield equal same-checker reference groups. |
| Class declaration/use; `a.n`; key in `a['n']`; final `Box<string>` type name | Declared `Widget`; constructor value `typeof Widget`; `number`; literal `"n"`; containing `Box<string>`. |
| Recursive `Box<string>` alias | First-level properties are `value:string` and `next:Box<string> \| undefined`; this is not a proof of the full graph backreference contract. |
| Annotated callback outside calls | Parameter `string`, empty prelude. |
| Invalid `satisfies` callback outside calls | Parameter `number` remains inspectable with root `isErrorType=false`; opt-in diagnostics include TS2322. An unrelated/outer diagnostic alone does not invalidate this demand. |
| Inherited NodeNext config and conditional package export | Native baseline selects `"import"`; replay must select the same branch and preserve resolved options/source inventory. Review originally exposed `any` from the incomplete replay. |
| UTF-8 BOM source in that inherited/package project | Native and replay type demands select `"import"`; member origins and scheduled candidate locations use the same decoded positions. |

Direct-call observations promote the selected member to its access expression,
then walk outward through the pinned outer-expression wrappers before invoking
`IsCallOrNewExpressionTarget`/`IsTaggedTemplateTag`. The pinned helper's own
`SkipOuterExpressions` descends; it does not walk from a name up to its wrapper.
Parenthesized, non-null, angle/assertion, `as`, `satisfies`, optional-call,
constructor-without-parentheses and tagged-template targets are measured true.
Argument reads, plain reads, `fn` as receiver of `fn.call`, namespace receivers,
and implicit `@fn`
decoration are measured false; explicit `@fn()` is true. Namespace-member/receiver
controls establish only the selected member as a call target. The complete
token/dual-role matrix remains a separate obligation.

## Compiler boundaries

Generated exact sources live in `fixtures/types/compiler-limits/generate.mjs`.
The hook records maximum operation counters immediately before each pinned guard
and distinct guard events. Values below are compiler work units, not source depth.

| Family | Under | At | Over |
| --- | --- | --- | --- |
| Conditional tail `Build<N>` | N=999: tail 999, valid tuple, no failure | N=1000: tail 1000, error root, `conditional_tail_limit` | N=1001: tail 1000, same failure |
| Instantiation count, distributive variable-N plus identity aliases | Two identities: guard observation 4999999, valid numeric union | Three identities: guard observation 5000000, error root, `instantiation_limit` | Four identities: capped observation 5000000, same error root/event |
| Cross-product union | 3×33333=99999, valid union | 100×1000=100000, error root, `union_complexity_limit` | 11×9091=100001, same failure |
| Head-recursive tuple through `Outer<N>` | N=47: depth 99, count 1614, valid tuple | N=48: depth 100, recovered tuple root, `instantiation_limit` | N=49: depth 100, same failure |
| Literal relation depth | 99 wrappers: stacks 99/99, `true` | 100 wrappers: stacks 100/100, recovered `false`, `relation_depth_limit` | 101 wrappers: same stack guard and recovery |
| Subtype removal estimate, 1250 annotated objects | 43 redundant types: estimate 991250, 1207 valid constituents | 42: estimate 1000000, 1208 valid constituents | 41: estimate 1007500, `subtype_complexity_limit`; array element is error |
| Natural reversed-union relation allowance | 1998 terms: last uncached guard has 4 remaining; comparison finishes at 1 | 1999: zero at guard, `relation_complexity_limit`, finishes at -2 | 2000: same guard and recovery |

The exact depth 99 predecessor uses
`Outer<N> = Head<N> extends infer U ? U : never` and query `Outer<47>`.
Its fresh pinned output is a 47-item tuple, depth 99, count 1614, and no guard event.
The same-family 47/48/49 registered boundary regression is green. The initial
unwrapped depth 98 family was insufficient for an exact under/at/over proof.

The count family distributes `Build<V,V>` over
`900|901|902|903|904|905|906|907|908|909|910|885|26|2` and nests identity aliases
around `Expand<...>`. All work is natural compiler evaluation; no counter is
seeded or reset by the fixture. Identity aliases are declared before Build/Expand.
The counter is observed immediately before the pinned guard, not after its
increment; two identities may finish at internal count 5000000 without another
guard observation. The third identity reaches the guard at that exact count.

Head 48/49 produce a tuple whose **root is not the error type**, while this fresh
operation history hits the instantiation guard. Root pointer/error-flag checking
cannot prove that no reached semantic branch failed. The hook now emits a cloned
`beforeSummary` counter/event snapshot after the final type demand and before
native `TypeToString`; aggregate observations after presentation remain separate.
The registered regression proves both Head depth and instantiation-count guard
events already occurred before summary printing. Even a before-summary event is
operation history: causal attribution separating final demanded recovery from
rejected overload candidate work remains a required production/hook refinement.

The relation/subtype generators byte-match the 13 independently frozen natural
inputs in [types-relation-limit-evidence.md](types-relation-limit-evidence.md).
No compiler counter, threshold, cache branch or reset changes. Counters use the
native signed integer domain, with explicit lossless conversions for the bounded
`uint32` instantiation fields. `relationAllowanceUsed` is measured from the fresh
two-million maximum, not a claim that every relation starts with that allowance.
The native formula accounts for existing cache size.

Only first per-site witnesses, maxima and the completed-comparison count are
retained. `relationFirstFinish` is the first completed comparison with a nonempty
cache; it is not inferred to be an arbitrary selected root's comparison. In the
natural allowance family it records 1999999/2000002/2000002 cache entries and
signed allowance 1/-2/-2. Those completion values differ from guard entry:
successful unwinding commits three more entries below the boundary, while the
overflow case still commits two. Native overflow cache bits remain untouched.

Already accumulated checker diagnostic codes are read in the phase snapshot;
this does not invoke whole-file `Checker.GetDiagnostics`. Relation depth overflow
produces TS2321 and a non-error `false` root, while a different leaf at depth 99
produces ordinary `false` without overflow or diagnostic. Subtype overflow
produces TS2590 and a non-error `any[]` root. The opt-in private
`inspectArrayElement:true` observation checks its native type argument after the
phase snapshot and proves that child is an error through the same `isErrorType`
owner; under/at children are valid unions.

An active-key Maybe at stacks 1/1 can finish as admitted Maybe (`true`) or
ordinary False after a conflicting leaf. Deep nesting at pushed stacks 3/3
finishes as admitted Maybe (`true`). These controls have no failure event or
diagnostic. They establish those concrete protocols, not complete cache/reset,
variance or Maybe behavior. First witnesses and even before-summary events
remain operation history, not general consumed-failure provenance.

The allowance baseline matches independent fresh public API controls. The pinned
whole-program CLI reports TS2859 even for the 1998 under case, so these request
results do not claim whole-program acceptance. The specific history-dependent
cause remains open.

## Validation and remaining gates

Registered single-file command:

```sh
cctl validate run harness-test --queue-if-busy --require-match -- tools/type-oracle.test.mjs
```

The first 12-test slice passed with zero skips in
`vrun-4b0d5dfd-0834-4a64-99a9-eacc465a6b89`.
Review-driven red `vrun-2cb567ea-c52f-4da4-950b-662e7ee0a188` confirmed the unwanted
implicit diagnostics and package replay mismatch through final oracle outcomes.
Its replay/diagnostics fix passed 13/13 in
`vrun-4ff279b4-7a53-4dc6-8f67-79ce25bffc59`.
Red `vrun-8edb6c74-8e03-4384-8431-b89daa3595eb` pinned the missing observed schedule
trace and depth 98 predecessor. Green
`vrun-0e302170-6c67-4577-9b38-b816f4de1492` passed all 15 tests with zero skips,
including exact depth 99/100/100, namespace call controls, outside-root and
astral/BMP filename ordering, and explicit unsupported hook histories. The
independent review suite passed six tests with zero skips in
`vrun-60ca2a21-44a1-423f-b00a-73e6b7a8f49e`.
Final guard/UTF-16 source-span verification passed 15/15 with zero skips in
`vrun-6d5de296-0624-4fff-949b-2947e1a6f77b`. Independent review reproduced the
percent-escaped overlay path failure in a checkout containing spaces, non-ASCII
characters and `%`; `fileURLToPath` fixed it, with seven independent tests green
and zero skips in `vrun-954c43f1-09d2-4ca7-afbf-e55ff7e0d659`.

Phase/count red `vrun-64b10fcb-fd97-4179-96f9-ca3a27ff95c9` failed precisely on
missing before-summary snapshots; its natural under-boundary counter already
measured 4999999. Final green `vrun-fd9885db-7854-454f-9278-57cead88017e` passed
all **16 tests with zero skips**, proving Head depth 99/100/100 and natural
count 4999999/5000000/5000000 before printing, with exact valid numeric union under
the count boundary and error-root `any` at/over it. The
[independent follow-up](types-oracle-review.md) inspected cloned phase snapshots,
the natural generator, current manifest hashes and those registered outcomes.

The candidate-ledger behavior first failed precisely on missing output in
`vrun-d91a7d5e-f8af-4359-904d-65233001930c`; its 17-test green was
`vrun-4d9c9973-6ab2-4a07-a6fb-5d0f8887d98b`. Selection provenance then failed in
`vrun-d455cda1-acb2-4e45-b81f-71dd507fa7cb`, unresolved seed handling in
`vrun-a89b1087-10d7-473d-9602-9f0e9d7a3ffa`, and exact namespaced JSX inventory
in `vrun-31e16bca-73ae-428f-be83-da3881006488`. The next run,
`vrun-d12933a7-0126-4ed9-9f02-e8dce6d80691`, passed 20/21 tests; its sole failure
was the subsequently disproved expected JSX nil-symbol blocker. A fresh final
hook result identified the actual `JsxAttribute` symbol and related property,
so the expectation was corrected from compiler evidence.

The independent [Q4 selector review](types-selection-oracle-review.md) reproduced
renamed type-only alias, type-query operand and parameter modifier selection
errors. Its expanded run `vrun-60abb32a-d878-4efb-9124-924ca19f049e` additionally
proved three synthetic punctuation-token RPC failures and an unresolved alias
reported as non-error by pointer equality in the Go hook. The fixes reuse the
real selected expression node and a checker-local shim to native `isErrorType`;
the latter includes compiler-manufactured unresolved alias types. Additional
question/flags/role observations are pure wiring and issue no extra checker
operation. With these observations exposed, staged review
`vrun-524cc654-dfb1-4e6e-ba26-4a98aac8d104` passed 17/18 tests and precisely
proved the parameter-name member/local role mismatch. Both roles now use the
same existing native pair owner. Expanded main green
`vrun-bae36d21-f12c-46ba-84ae-615604140cb1` passed **21/21 with zero skips**,
including the corrected namespaced JSX native relation and every original limit
regression. Independent selector green
`vrun-eb9d186c-c7c2-4077-8e25-d7240986b3c8` passed 20/20 with zero skips.

Relation/subtype desired red
`vrun-498e2d8c-a033-4892-9be4-7c12f2afb2e5` passed the original 21 tests and failed
all four new tests on missing-observation assertions. An initial run used an
unchecked dereference for the missing Maybe field; that expectation was repaired
and the red rerun above confirmed its assertion. The first implementation run
hit a Go observation-helper type mismatch, corrected with explicit conversions.
Final green `vrun-11671a51-5307-423c-9693-3ad4d3e7fcee` passed **25/25 with zero
skips**, including every under/at/over and ordinary-False/Maybe control. The
independent selector review's relation follow-up inspected guard ordering,
snapshot copies, signed completion values, diagnostic collection and child
error ownership and confirmed the bounded run without a blocking finding.

Metadata regression `vrun-e11df82b-6a72-4106-a47b-89b83ca1e5ab` first reproduced
a BOM replay anchor failure. The custom API filesystem had preserved the BOM in
parsed text while pinned `vfstest.ReadFile` removed it. The source-text owner fix
made final type, member-origin and scheduled candidate-position assertions pass;
rerun `vrun-931996d5-9211-4840-ba33-7cb0097b9d50` passed 24/25 and failed solely
on the new missing-input-hash assertion. Hash wiring derives each digest from the
already frozen replay map rather than source text that has undergone decoding.
Final metadata/BOM green `vrun-179b33a5-0398-47fc-996b-0057ee79a445` passed
**25/25 with zero skips**, including disk-byte equality for every asserted input.

Property-name selection red `vrun-26af97b8-c5ec-4fc3-a818-9598e1793b5c`
passed those 25 tests and failed two new final-operation assertions: an ordinary
object assignment name asked the expression question instead of the value
question, and a module specifier was accepted as an ordinary Q4 typed anchor.
The selector now includes `PropertyAssignment` in declaration-name value
selection and rejects import/export/import-equals module specifiers before
ordinary expression selection. The computed property's inner literal retains
its native expression role; it is not the declaration-name wrapper. Final green
`vrun-f28c1e49-764e-41d1-b936-959d299db52a` passed **27/27 with zero skips**,
covering identifier, quoted, numeric, hexadecimal, empty and escaped assignment
names with both fresh API and same-checker hook outcomes. An explicit raw
compiler question override remains an oracle experiment, not a product anchor.

TypeQuery operand regression `vrun-bce10f5a-43c8-435f-bd94-253d5990344c`
passed 27 tests and failed the new site-read assertion: `typeof current` after
native narrowing returned the declared `string | number` through
GetTypeOfSymbol instead of `string` through GetTypeAtLocation. The selector now
retains a private location-demand mode while keeping the public Value question;
ordinary declaration/alias symbol reads and explicit raw Value controls retain
their own operation. Final `vrun-2ad6b2fa-9bb7-409a-8c9d-edc17c14b746`
passes **28/28 with zero skips**, including narrowed, assigned and guarded-object
operands in the fresh API and same-checker hook. This corrects the oracle
question; it does not establish the complete production flow admission rule.

Exact path order was then corrected independently. The pinned native
CompareStringsCaseSensitive operation uses UTF-8 byte comparison, agreeing with
Rust Location Ord and placing BMP U+E000 before astral U+10000. The earlier
JavaScript comparator and its test assumption used UTF-16 code-unit order.
Changing the final candidate-schedule expectation to the measured native order
gave real RED `vrun-db9d0cee-9c0c-4a86-a845-820ffa4bdad0` (27 pass, one
failure). Shared exact text/location comparison now owns UTF-8 ordering and
refuses malformed unpaired identities instead of replacing their values.
Final `vrun-31732fbe-c9af-4d38-b96d-68d5713ea77d` passes **28/28 with zero
skips**. Coordinates remain one-based UTF-16 positions; this correction is the
separate path-string ordering question. Raw compiler groups retain their raw
order. The fresh comparer observation and historical executable are preserved
under `fixtures/types/path-order`.

The later synthetic TypeQuery-keyword regression passed its same-checker
positive controls but failed the public final operation in actual RED
`vrun-4ce35d57-b107-411c-99b2-f163aad23106` (28 pass, one assertion failure).
The selector had already chosen the real containing TypeQuery, while an unused
symbol observation still asked about its synthetic `typeof` token. Type and
expression observations now use the selected real node; declaration/value
questions keep their own selected-name operation. Final
`vrun-c91e8e2c-6a28-4e1e-a322-595e6a13dc55` passes **29/29 with zero
skips**, including fresh narrowed and primitive TypeQuery keyword answers.
This corrects oracle acquisition; native flow and TypeQuery delivery remain
separate implementation obligations.

The last frozen `fixtures/types/oracle-manifest.json` records 82 source/tooling file hashes, 258
exact marker anchors, 23 effective fixture configurations, compiler
package/binary/library identity, Go module locks and 22 exact generated-input
hashes. It includes all seven executable oracle suites and excludes historical
`command-center`, `representation`, `relation-research` and `captured-tooling`
subtrees, whose independent provenance owns those captures. Its reproducible
generator is data-only wiring, so it has no separate behavior test. Run it with the compiler
environment used by `scripts/validate/common.sh`. The concrete corpus manifest
is recorded separately under `fixtures/types/command-center`. A final direct
data verification matched every inventory digest, regenerated source and marker
position; this does not substitute for the registered behavior gates.
Later native families and oracle fixes require a new complete freeze before a
final checkpoint; the earlier inventory is a point-in-time record.

Checkpoint 1 still requires the complete Q3 candidate/relationship/root matrix,
normalization for all eligible seeds, Q4 selector/graph/predicate/write-type and
output traversal/budget matrix, all supported non-call contextual forms,
counter reset/cache/Maybe behavior, and the concrete corpus journeys. The tested
natural subtype/depth/allowance boundaries and active/deep Maybe samples do not
prove all operation histories or consumed-failure provenance. The same-checker
hook removes the public API barrier
for this bounded slice; it does not waive any of those obligations. Complete
native semantic delivery still requires the full baseline and checkpoint-2
representation decision. Bounded native implementation increments close neither.

The option classification and its separate configuration verification are in
[types-options.md](types-options.md) and
[types-configuration-review.md](types-configuration-review.md); this oracle slice
does not substitute for their tests.
