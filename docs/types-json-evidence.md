# Imported JSON type and origin baseline

This is a bounded TypeScript 7.0.2 compiler baseline, researched with the
`ousterhout-software-design` skill. It adds fixtures and registered oracle tests,
not a native checker, JsonFacts implementation or production query. The
operating envelope is a fresh local compiler request over one fixed TypeScript
consumer and its four imported JSON files. Native T/Q3/Q4 and the semantic gates remain
open.

## Compiler questions and expectations

The selected Q4 positions are TypeScript declarations initialized from the JSON
import. Each asks the native `GetTypeOfSymbol` question in a fresh checker, with
no enclosing-call prelude. Scalar literals widen: `"literal"` becomes `string`,
`42` becomes `number`, and `true` becomes `boolean`. Duplicate property semantics
use the last value, while decoded key identity preserves every source origin.

| Fixture property | Imported value type under strict-null checking | Exact key origins |
| --- | --- | --- |
| Top-level `dup`, first string then number | `number` | Both keys on lines 11 and 12 |
| Nested `dup`, first `{old: 1}` then `{last: "s"}` | `{ last: string; }` | Both keys on line 10, with distinct columns |
| `a` followed by `"\\u0061"` | `boolean` | Lines 13 and 14, preserving different raw span widths |
| Empty key | `number` | Line 15, columns 3–5 |
| `"01"` and `"1"` | `string` and `boolean`, respectively | Distinct exact keys on lines 16 and 17 |
| Astral key `😀` | `boolean` | Line 18, columns 3–7 in UTF-16 |

These are origin controls, not a display-name join or a general canonical
relationship algorithm. Q3 raw JSON key requests and TypeScript property uses
both retain those exact PropertyAssignment origins. Direct JSON keys have native
Property flags `4`; the widened TypeScript use has Property plus Transient flags
`33554436`. Matching origins do not require erasing this native role distinction.

There is a deliberate compiler distinction behind the direct-Q4 JSON exclusion.
A raw native symbol selected at either top-level duplicate JSON key has both
declarations but its direct symbol type is the first declaration's `string`.
The imported property type is the last value's `number`. The nested duplicate
similarly yields `{old: number}` at raw JSON keys and `{last: string}` through
the import. These direct native symbol observations are research controls, not
a new Q4 JSON-position capability.

Synthetic default import denotes the whole JSON root even when the file has a
literal `"default"` key. Reading `data.default` still returns that key's `string`
value. The native namespace import has a synthetic `default` property of the
whole root type. Named import of `dup` yields the widened last-value `number`.

## Recursive arrays and option-dependent construction

The JSON fixture includes empty, numeric, mixed, unlike-object and nested arrays.
Pinned strict-null baseline answers are `never[]`, `number[]`,
`(string | number | null)[]`,
`({ a: number; b?: undefined; } | { a?: undefined; b: string; })[]`, and
`(number[] | (string | null)[])[]`, respectively. The recursive structure must
survive the future fact-owner transfer; an undifferentiated Array shape cannot
provide these types.

Separate modules with root values `null`, `[]` and `42` are accepted by all four
pre-demand diagnostic families. Their strict-null imported types are `null`,
`never[]` and `number`; root null/empty types become genuine `any`/`any[]` when
strict-null checking is off. These root controls are TypeScript import queries,
not Q4 positions in the JSON files.

All eight explicit combinations of strict-null, exact-optional and unchecked
index options were examined through fresh raw native API requests. Six are
accepted by the oracle's existing program-diagnostic policy. The two with
`exactOptionalPropertyTypes: true` and `strictNullChecks: false` report native
Program TS5052 and are rejected before the oracle demand. Their raw checker
answers are recorded as controls, not accepted profiles or Rust Project loading
requirements.

| Question | Strict-null off | Strict-null on, unchecked off | Strict-null on, unchecked on |
| --- | --- | --- | --- |
| Imported null value | Genuine `any` | `null` | `null` |
| Empty array | Genuine `any[]` | `never[]` | `never[]` |
| Empty array index read | Genuine `any` | `never` | `undefined` |
| Numeric array index read | `number` | `number` | `number | undefined` |
| Mixed array index read | `string | number` | `string | number | null` | `string | number | null | undefined` |
| Unlike-object array's `a` read | `number` | `number | undefined` | `number | undefined` |

Exact-optional construction changes the unlike-object array's absent slots from
`a?: undefined`/`b?: undefined` to `a?: never`/`b?: never`. Reading an optional
member still includes `undefined` under strict-null checking. This preserves
missing versus explicitly undefined construction; a shared spelling alone
cannot represent that distinction.

## Recovery observations

A missing JSON property returns native `any` with `isErrorType == true`.
An explicitly declared `unknown` value returns `unknown` with a false flag;
reading its property returns erroneous `any`. A property read on genuine `any`
returns `any` with a false flag. Under non-strict null checking, imported null
and empty-array reads also produce genuine `any`, not the manufactured missing
property error type.

The registered controls compare the direct flags before requesting whole-file
diagnostics. A separate diagnostic pass observes TS2339 for the missing JSON
property and TS18046 for the unknown receiver. That pass establishes additional
checking history and cannot retroactively prove arbitrary recovery causality or
semantic admission from a root flag.

Another raw control observes the unlike-object array's `a` read returning
`number | undefined` with a false root error flag in both unchecked profiles.
Only unchecked-index-on produces TS2532 at that read in the later semantic pass.
A non-error root therefore does not prove a fully admitted read journey. Native
consumed-recovery provenance remains implementation work.

## Ownership consequence

The existing JSON reader preserves raw properties. `JsonModule::new` deliberately
retains each distinct exact object key and the last reachable value, while
`JsonShape::Array` retains no element structure. This is sufficient for its
current narrow binding question, not the full type/origin baseline above.

Extend that existing fact product for the design's JsonFacts: recursive array
elements, every exact key occurrence and last-value links. Keep scalar payloads
out of the imported widened-type question unless a separate admitted operation
needs them. Keep `Linker::value_property`'s narrow `getPropertyOfType` operation
distinct from a native receiver-type evaluator. Retention alone does not
implement its future known-present/known-absent/deferred contract. This review
made no change to those production owners.

The subsequent bounded mechanical retention change is recorded separately in
the historical JSON-retention document (archived in Git history). Its
fact-structure proof and public compatibility controls do not turn this compiler
baseline into a native JSON type evaluator.

## Evidence and limits

The throwaway raw expectation probes are
`.cc/temp/json-type-native-probe.mjs` and
`.cc/temp/json-options-native-probe.mjs`; their JSON outputs record native
questions independently of the shared oracle's selection/result code. They use
the version-checked TypeScript 7.0.2 native API, and dispose each checker after
its one selected demand. The option probe's explicit later diagnostic controls
are separately labeled. Fixture/probe archives and hashes are recorded beside
the committed fixture under `fixtures/types/json-types` after validation settles.

Registered single-file validation uses:

```sh
cctl validate run harness-test --queue-if-busy --require-match -- tools/type-json-oracle.test.mjs
```

Initial runs each executed eight tests with seven passes and zero skips. Their
one failure was a test observation expectation: first the schedule field path,
then an overly narrow flags value that omitted the widened symbol's Transient
bit. Fresh raw compiler controls justified both corrections. They are not
reported as production defects or behavior-fix RED evidence. The corrected
eight-test baseline passed in `vrun-2637f2f0-d2c4-4b17-8221-ec47ba27b095`.
The ninth diagnostic-history control passed in
`vrun-c1d65b6c-e1b3-4a89-971b-dc612dec1edd`, with nine executed and zero skips.
The final expanded root-module profiles, alias selection controls and exact
frozen-input hash assertions passed in
`vrun-493c9f31-e064-4b24-9b43-641029310824`: one matched file, nine executed,
nine passed and zero skipped. Its 23 fixture/test/tool/hook source hashes were
verified unchanged after validation. The manifest and seven raw/native/Go
control artifacts are in `fixtures/types/json-types/evidence`; eight exact
tool/hook copies are in `fixtures/types/json-types/captured-tooling`.

The main raw probe records 36 fresh demands. The option probe records 72 option
demands, ten raw JSON-key controls, four TypeScript-use flags controls and two
explicit later-diagnostic controls. The five final Go answers contain the exact
frozen replay input hashes, with byte equality assertions for all six local
inputs. Archived probe scripts retain their original `.cc/temp` relative import
and output context; the archive README explains that they are inspection copies,
not standalone replay commands.

This baseline does not prove arbitrary JSON formatting/recovery, unpaired UTF-16
keys, all import/resolution modes, full candidate relationships, canonical Q3
group normalization, dependency admission, flow, graph/materialization, service
refresh or representation/resource limits. The public member API uses a separate
reference checker; only the bounded Go controls exercise the shared checker
schedule. No latency or retained-memory claim is inferred from harness duration.
