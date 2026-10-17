# T, Q3, Q4 design evidence

Recorded 2026-10-09 at repository `ea23d8b`. These are design probes of the pinned
TypeScript **7.0.2** native semantic API and language server, not tests of a new
native checker. The design is [types-design.md](types-design.md).

## Repository observations

* `src/project/mod.rs`: `load_here` builds a `Program`, then a binding `Index`,
  moves sources/file lookup/navigation into the Project, and drops other facts.
  Project queries read captured sources; Session supplies freshness.
* `src/project/binding/index.rs`: the index stores symbol declaration sites,
  token targets, roles, name candidates, and occurrences. It lacks declaration
  type syntax, expression dependencies, inference context and member types.
* `src/project/binding/collect.rs`, `mod.rs`, `store.rs`, and `link.rs`: enum
  names and selected apparent-interface names support existing linking/JSON
  decisions. `JsonShape` stores primitive categories and object-key paths, without the
  recursive array element facts and complete key-occurrence provenance T/Q3
  need. Scalar values are not needed for the selected imported-module question.
* `src/project/program/config.rs`: effective options currently support loading
  and binding. Semantic options such as strict null checks are not retained as
  a typed checker configuration. They need the existing conversion/inheritance
  owner, not a parallel reader.
* `src/project/session.rs`: changed inputs discard the previous generation
  before rebuilding; failure never answers from the discarded snapshot.
* `src/cli/query.rs`: direct and service execution share `ProjectSession` and
  one delivery policy. No new semantic service or second freshness check is
  needed to add a Project request.
* `tools/semantic-oracle.mjs`: member/property queries are explicitly normalized
  to `UnsupportedTarget`. Existing lexical conformance is not member conformance.

These observations are anchored to code, not line-count arguments. The design
addresses the missing owned facts and rule duplication they would otherwise
invite, rather than introducing modules simply to split large source files.

## Probe setup and scope

The local compiler cache supplied the version-checked native binary and the
7.0.2 package's `dist/api/sync/api.js`. The probe opened one strict ESNext
TypeScript project, collected types/symbol declarations at 27 marked positions,
and asked 12 `textDocument/references` questions with declarations included.
It reused the repository's `loadSemanticApi` and `DocumentSymbolServer` harness
owners; all snapshots and processes were closed. The compiler reported zero
semantic diagnostics for the probe source.

The disposable script and raw output are under
`.cc/temp/type-design-probes.mjs` and
`.cc/temp/type-design-probes/results.json` in the authoring worktree. Re-run with:

```sh
bash -c 'source scripts/validate/common.sh; find_semantic_compilers; node .cc/temp/type-design-probes.mjs'
```

The source cases and relevant results below are retained here so the design's
evidence does not depend on keeping temporary files. Numbers assigned to
compiler symbols are snapshot-local and were not treated as durable identities.

## Reference identity is not reference relatedness

The essential input was:

```ts
interface Host { observe(x: string): number }
const annotated: Host = { observe(x) { return x.length; } };
const satisfiesHost = { observe(x) { return x.length; } } satisfies Host;
const unrelated = { observe(x: string) { return x.length; } };
annotated.observe('a');
satisfiesHost.observe('b');
unrelated.observe('c');
const alias = annotated.observe; alias('d');
annotated['observe']('e');
interface Other { observe(x: string): number }
declare const union: Host | Other;
union.observe('g');
```

All four declarations of `observe` have the displayed type
`(x: string) => number`, but the API reports distinct declaration origins for
the interface, contextual object, `satisfies` object, and unrelated object.
`annotated.observe` resolves to the interface declaration. The other object uses
resolve to their own method declarations. The union access has both interface
declarations as origins. Widening/instantiation can create fresh compiler symbol
objects with the same declaration origins.

The reference searches demonstrate why an equivalence class is insufficient:

| Selected site | Observed references of interest |
| --- | --- |
| `Host.observe` | Interface declaration; annotated and `satisfies` object method declarations; `annotated.observe` call/read; literal element access; union call. **Not** `satisfiesHost.observe('b')`. |
| Annotated object's method declaration | Same set as the interface seed in this case. |
| `satisfies` object's method declaration | The interface-seed set **plus** `satisfiesHost.observe('b')`. |
| Unrelated object's method | Only its own declaration and `unrelated.observe('c')`. |
| `Other.observe` | Its own declaration and the union call. |
| Union call's method | The two interfaces' related sets, including their contextual declaration sites; no unrelated object method. |
| `alias` in `alias('d')` | The `alias` variable declaration and its call, not the interface method family. |

Do not infer additional behavior from this one case. The pinned native source
provides the mechanism to investigate and port:

* `internal/ls/findallreferences.go`: `populateSearchSymbolSet`,
  `forEachRelatedSymbol`, `getRelatedSymbol` and property symbols from base types.
  Relatedness depends on location, contextual properties, root symbols, base
  types and static/instance distinctions. Shorthand and parameter properties
  have explicit dual-symbol handling.
* `internal/checker/checker.go`: `getPropertyOfTypeEx`,
  `createUnionOrIntersectionProperty`, `instantiateType` and declared/value type
  operations. These belong to type computation, not reference result filtering.

The source was read from the local pinned `typescript-go-7.0.2` checkout. The
probe directly contradicts merging contextual and interface method identities,
or using structural type compatibility as reference identity.

## Inheritance and generic provenance

```ts
class Base { observe(x: string) { return x.length; } }
class Derived extends Base {
  override observe(x: string) { return super.observe(x); }
}
new Derived().observe('f');
function makeHost<T>(initial: T) {
  return { observe(x: T) { return x; }, initial };
}
const made = makeHost('start');
made.observe('end');
```

Base and override are distinct declarations. Selecting either method in the
language server returned the base declaration, override declaration,
`super.observe` and the derived call. The design preserves explicit heritage
relationships instead of assigning all equal signatures one identity.

The factory method displays `(x: T) => T` at its declaration and
`(x: string) => string` at the instantiated call. Both point to the same method
declaration origin, and both reference searches returned those two sites. Type
instantiation therefore needs its own key while retaining member provenance.

## Types and expansion

| Input/query | 7.0.2 result |
| --- | --- |
| `const literal = 'one'`, at declaration | `"one"` |
| `let wide = 'one'`, at declaration | `string` |
| `const object = { status: 'ok' }` | `{ status: string; }` |
| Same initializer `as const` | `{ readonly status: "ok"; }` |
| `declare const optional: { key?: number }; const read = optional.key` | `number | undefined` for `read` under `strict` |
| `type Box<T> = { value: T; next?: Box<T> }; type StringBox = Box<string>` | Summary `StringBox`; properties `value: string` and `next: Box<string> | undefined`. |
| `type Mapped = { [K in 'a' | 'b']?: K }` | Summary `Mapped`; `a: "a" | undefined` and `b: "b" | undefined`. |
| `type Conditional = ReturnType<typeof makeHost<string>>` | `{ observe(x: string): string; initial: string; }`. |
| String token inside `annotated['observe']` | Type `"observe"`, while its symbol is the interface member. Type-target and reference-target selection ask different questions. |
| `declare const anyValue: any; anyValue.observe()` | Member expression displays `any`, is not an error type, has no resolved property symbol; member reference search returned an empty set. |
| A copied `unknown` value | `unknown`, not the error type. |

For `function narrow(v: string | number) { if (typeof v === 'string') return v;
return ''; }`, the read in the guarded return is `string`. Returning the declared
union from a Q4 use query would be wrong. Until F supplies flow, the design must
identify this demand as unsupported, including when inference reaches it
indirectly through an inferred return.

The installed API exposes `getTypeAtLocation`/`getTypeAtPosition`,
`getDeclaredTypeOfSymbol`, `getTypeOfSymbol`, `getContextualType`,
`getResolvedSignature`, `isTypeAssignableTo`, `getPropertiesOfType`,
`getIndexInfosOfType`, `getTypeArguments`, `typeToString`, and
`getReferencedSymbolsForNode`. Availability is evidence for building differential
oracles and for the backend alternative, not a promise that arbitrary production
integration or every presentation operation has already been tested.

## What remains unproved

The [Astra xhigh review](types-design-review.md) adds independent 7.0.2
counterexamples for late-bound computed members, annotated-initializer flow,
inferred predicates, flow through `typeof` annotations, and imported interfaces
without value types. Its source cases and results are retained in that document;
the revised design incorporates them. The [response](types-review-response.md)
maps each finding to the new contract and required implementation evidence.

There is no native semantic implementation, memory measurement for retained
semantic terms, query-latency measurement, broad conditional/inference corpus,
option matrix, or unchanged-Command-Center semantic coverage measurement in this
design work. The proposed display limits are product choices. Flow admission,
candidate completeness, reference-group normalization and the semantic resource
limits need the behavior-level proof in the design before implementation can be
called complete. These probes must not be reported as a successful delivery of T.

## Round-two verification and corrections (2026-10-10)

The [additional review](types-design-review-2.md) was an incomplete review run;
its verification labels are preserved. For the response, three bounded
investigations independently read the Ousterhout skill, repository owners,
pinned 7.0.2 sources and retained review probes. New probes below test the
uncertain remedies as well as the original findings. No Rust implementation,
whole-project coverage measurement or fresh full-design approval is claimed.

### Contextual checking needs an explicit prelude

`chooseOverload`, `contextuallyCheckFunctionExpressionOrObjectLiteralMethod`
and `assignParameterType` share call/node contextual state across rejected
candidates. Candidate-specific inference state does not imply rollback of that
context. The new probe used these zero-diagnostic cases:

```ts
declare function m(cb: (x: string) => number): 'first';
declare function m(cb: (x: any) => string): 'second';
const result = m(x => x.toUpperCase());
interface Shout { shout(): string }
declare function q(cb: (x: Shout) => number): 'first';
declare function q(cb: (x: any) => string): 'second';
const qr = q(x => x.shout());
```

Thirteen fresh compiler sessions compared cold subnode-first queries, explicit
prelude queries in forward/reverse/singleton order, and diagnostics-first
queries. The prelude uses AST parent links, calls `getResolvedSignature` on
containing call/new/tag nodes outermost-first, then `getTypeAtLocation` or the
appropriate declaration-symbol type operation. The callback parameter becomes
`string`, the member `shout` becomes `() => string` with the Shout declaration
origin, and both result declarations have type `"second"` in every tested
prelude ordering. Cold subnode-first queries instead return `any` for the first
parameter/member inspected.

**Correction to “settled” wording in review/probe notes:** requesting
`program.getSemanticDiagnostics()` alone did not normalize the sync API checker
in this experiment. Earlier declaration-first queries supplied the effective
warmup. The design therefore states a concrete fresh-request prelude, and does
not identify it with a full diagnostics/LS history. This evidence establishes
that one vertical slice, not every alias/limit/reference-search history effect.

Scripts/results: `.cc/temp/response-review2-core/prelude.mjs` and
`prelude-results.json`. Additional cases in `probe.mjs` / `results.json`
confirm the generic winner is `string`; the incompatible two-overload case
reports TS2769 with a recovery display of `never`; rejected `satisfies` retains
its expression type and contextual callback `number`; strict-unset preserves
`undefined`; index-signature access has unchanged `number` despite TS4111;
NoInfer, Awaited and variadic tuples have their compiler-defined forms.
Those intentional-diagnostic cases must not be described as zero-diagnostic.

### Flow and reachability: refine two review remedies

The new flow probe ran under `strict: true` and under the same configuration
with `strictNullChecks: false`; both had zero diagnostics.

| Input | 7.0.2 answer in both variants |
| --- | --- |
| `function f() { return fail(); }`, with `fail(): never` | `() => never` |
| `function f() { fail(); }` | `() => void` |
| Class method with explicit `return fail()` / trailing `fail()` | `() => never` / `() => void` |
| Object method with either form | `m(): never` |
| `(x: {}) => typeof x === 'string'`, also through a local boolean alias | `(x: {}) => x is string` |
| `<T>(x: T) => { const yes = typeof x === 'string'; return yes; }` | `<T>(x: T) => x is T & string` |
| Nonunion object callbacks returning `x.active`, `x.id === 'x'`, or `x.id.startsWith('x')` | Boolean return, no predicate |
| Filtering that object array by `x.id === 'x'` | Original element array |

Thus `mayReturnNever` does not prohibit all function/class never inference, and
parameter category or textual-reference checks cannot replace the compiler's
condition/alias dispatch. Binding supplies one graph; Evaluation interprets
supported read setup, identity cases and reachability, refusing excluded
refinements. Existing review probes for defaults, constraints, optional-chain
roots, union initialization, exhaustive switches and typed call effects were
inspected rather than all rerun.

Script: `.cc/temp/response-review2-flow-probe.mjs`; results:
`.cc/temp/review2/flow/response-check/results.json`.

### Other source-backed corrections

- `instantiateTypeWithAlias`, `getConditionalType`, `checkCrossProductUnion`,
  `removeSubtypes` and `recursiveTypeRelatedTo` own distinct limits. Some return
  error/recovery; relation overflow can return False, while active/deep relation
  pairs can return Maybe. A product resource budget cannot replace any of them
  or be guaranteed greater than all possible valid project work.
- Alias-bearing instantiation keys, `CompareTypes`, union insertion and
  intersection order affect reduction/origins. `getPropertiesOfType` preserves
  operation-specific member order; sorting its final output alphabetically or
  applying one global `sortSymbols` is not equivalent.
- `GetRootSymbols` includes mapped `syntheticOrigin` and special spread links;
  inline union seed substitution occurs before the three previously named
  search relationship functions. Candidate syntax excludes arbitrary computed
  key expressions. Existing cold/warm late-member probe outputs are retained
  as observations, not a reason to warm the oracle opportunistically.
- `JsonFacts` needs recursive array structure, every exact key occurrence and
  last-value links. Module/property types widen literals. **Direct JSON value
  positions can still have literal types**, and an earlier duplicate key can
  have a different type from the imported last value; the revised Q4 profile
  excludes positions inside JSON rather than retaining unused value payloads.
- `GetStrictOptionValue` defaults the strict family on when unset. Intrinsic
  aliases are the four string mappings, NoInfer and BuiltinIteratorReturn;
  Awaited is a separate global alias hook. Per-file module facts remain inputs
  from the existing resolver, not a checker-side second detector.

Pinned sources are `internal/checker/{checker,flow,relater,services,utilities}.go`,
`internal/binder/binder.go`, `internal/ls/findallreferences.go`,
`internal/core/compileroptions.go`, and the constant evaluator package in the
cached `typescript-go-7.0.2` checkout. The second response maps each finding to
its revised contract and required end-to-end proof.

### Reproducible corpus, remaining evidence

The current clean Command Center checkout was read at
`92fe6ab06fbea2c4b6a305689f8cf336173eef81`. It contains
`SpecWorkflowCleanupPort.observe`, `createAgentGatewayHandlers`,
`CollapsibleTextProps.maxCollapsedHeight`, and `agentPermittedModes`' filter;
these replace the absent RunHost as real-project checkpoint seeds. The revised
design requires a hashed query manifest and concrete successful journeys,
coverage, latency, queue and compatibility outcomes. We did not recompute the
review's broad call/JSX syntax counts or measure those semantic journeys.

Eager versus reprojected semantic source, complete Q3 normalization, all compiler
limit boundaries, semantic worker stack/budgets, and real-project utility remain
implementation feasibility gates. The new 30-second semantic deadline and graph
limits are proposed product choices, not compiler facts or measured guarantees.
