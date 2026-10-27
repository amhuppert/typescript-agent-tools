# Construction sites and omitted properties (Q5): technical design

Status: implemented, revision 3, 2026-10-24, from repository revision
`7c2b42c`, for **Q5** in `typescript-agent-tools#1`. Revision 2 responded to
the Astra xhigh design review; revision 3 records the measurements, the
user's decisions on the examined universe and its cost, and what changed in
implementation: no call prelude, independent argument candidates, released
syntax, and a capped gap list (dispositions at the end). T, Q3 and Q4
([types design](types-design.md)) supply the checker, selection, gap
vocabulary and presentation this builds on.

## Problem

An agent adds a property to an interface and must find every object literal
that builds a value of that type. A required property shows up as one compiler
error per site; an optional one shows up nowhere. The motivating retrospective
asked for `constructs RunView --missing`: "every literal typed as RunView, and
which props it lacks (optional ones too)". The later edit E3
(`add-prop --where-missing`) will consume the same sites, with stronger
evidence of its own before it writes (see Presence).

## Operating envelope

[ENVELOPE.md](../ENVELOPE.md) holds: one author's agents, a per-worktree
service that serializes queries, a fresh checker per request, very large
projects (4,000–8,000 inputs), warm queries of 100–200 ms for navigation, about
2 GB retained per service, a 60 s query deadline and 6 GiB memory threshold as
abort ceilings, and failure cost measured in the author's time. Q5 adds no
cache, durable state, concurrency or input kind. Semantic searches already
exceed the navigation latency (member searches take 12–16 s on Command
Center). Q5's latency was measured before its universe was fixed, and the
user chose the default universe and its budget (Measurement gate): a
narrower universe is named in every result, never presented as all literals.
A wrong `complete` misleads an agent edit, so an undecidable site is a
located gap, never a silent non-site.

## Contract

```sh
tsparse --constructs --project tsconfig.json --name RunView
tsparse --constructs --project tsconfig.json --name RunView src/runs/view.ts --missing lane
tsparse --constructs --project tsconfig.json --at src/runs/view.ts:12:18 --no-tests --json
tsparse --constructs --project tsconfig.json --name Options --all-literals --json -o sites.json
```

Selection is Q4's: `--name` (Q2 selectors, optional FILE) or `--at` on the
declaration. It must be a **declared** question for an interface, a class
(instance side) or a type alias; anything else, including a type reference in
an annotation, is `UnsupportedTarget` naming what to select. Further target
refusals are under Construction.

`--no-tests` and `--file FILE` are Q3's filters (one owner,
`project::references::Filters`), applied to sites before semantic work.
`--missing KEY` keeps the sites with a constructed alternative that **declares**
the string key `KEY` (compared exactly; the empty string is a key) and does
not certainly provide it. A `KEY` that no target alternative declares as a
string key fails before the search as a property-specific query failure
(`UnknownProperty`, exit 1) naming the target, the key and the nearest key by
the existing spelling owner. `--all-literals` examines every literal instead
of the default universe (Examined literals). `--limit N|all` caps the
bounded lists; `-o`, `--json`, `--pretty`, `--direct` behave as for
`--references`.

### Result

```json
{
  "operation": "constructs",
  "outcome": "complete",
  "project": "/work/tsconfig.json",
  "selection": {"Name": {"name": "RunView", "file": null}},
  "target": {
    "summary": "RunView", "summaryTruncated": false,
    "location": {"file": "src/runs/view.ts", "start": {"line": 12, "column": 18}, "end": {"line": 12, "column": 25}},
    "alternatives": [{"id": "a0", "summary": "RunView", "summaryTruncated": false, "origin": {"file": "src/runs/view.ts", "start": {"line": 12, "column": 1}, "end": {"line": 20, "column": 2}}}]
  },
  "filters": {"excludeTests": false, "file": null, "missing": null, "allLiterals": false},
  "sites": [{
    "location": {"file": "src/runs/project.ts", "start": {"line": 40, "column": 10}, "end": {"line": 44, "column": 4}},
    "contextualType": {"summary": "RunView", "summaryTruncated": false},
    "constructs": [{
      "alternative": "a0",
      "summary": "RunView", "summaryTruncated": false,
      "lacks": [
        {"key": {"kind": "string", "name": "lane"}, "optional": true, "presence": "absent"},
        {"key": {"kind": "string", "name": "status"}, "optional": false, "presence": "unknown"}
      ]
    }]
  }],
  "coverage": {"status": "complete", "universe": "target_keys", "literals": 9120, "examined": 412, "sites": 1, "gaps": []}
}
```

* A **site** is an object literal and the target alternatives it constructs
  (only those; other alternatives of its context appear in `contextualType`).
  Its location is the literal's whole span, so E3 can edit it.
* `target.alternatives` gives each alternative a query-local `id`, a bounded
  summary and the origin of its declaration, so equal or truncated summaries
  stay distinguishable. `constructs[].alternative` refers to it; the
  per-site summary is the instantiated alternative (`Box<string>`).
* `lacks` lists, per constructed alternative and in its property order, each
  property not certainly present: `optional` is the alternative's own
  requirement (Optional or Partial, as `getUnmatchedProperties` tests);
  `presence` is `absent` or `unknown`. Keys use Q4's vocabulary: `string`,
  `private` and `symbol` (with origins). An empty list means every property is
  provided.
* `coverage.universe` names the examined universe (`target_keys` or `all`),
  `literals` counts the literals of the files searched, `examined` those whose
  construction was asked, and `sites` all sites before display cuts. Text
  output follows `--references`: a header (`10537 of 110159 literals
  examined`), one line per site (`FILE:LINE:COLUMN lacks lane?, status
  (unknown)`), gaps, a hint-tier line counting the literals not examined and
  naming `--all-literals`, and the cut lines.
* Display budgets (registry bounded lists, one `--limit`): 200 sites, 20
  `lacks` per shown construction (a cut construction records
  `omittedLacks`), and 50 gaps. A cut is `display` in JSON and the last text
  lines, never a change to `outcome` or `coverage`; the library result keeps
  everything.
* `outcome`/`coverage.status` is `complete` when every examined literal was
  decided, else `partial`, exit 1. Gaps use Q3's record and closed reason
  vocabulary. Unselected selections use Q4's `not_found`/`ambiguous` shape
  with `operation: "constructs"`.

## Semantics

TypeScript 7.0.2 defines every type below; product rules are marked.

### Examined literals

The literals are every `ObjectLiteralExpression` of each searched file
admitted by the filters, in exact display-path/position order, except JSON
modules, whose literals have no contextual type. Each file's syntax is
captured while its literals are examined and released afterwards unless an
earlier demand held it; handles stay valid because a reparse yields the same
nodes.

**Universe** (user decision, revision 3). By default a literal is examined
when one of its *named* members (not a spread, and with no dynamic name,
`isDynamicName`) has a key that some target alternative declares, or when it
has no named member (`{}`, or only spreads and late-bound names). A member's
key is the one `checkObjectLiteral` gives its property
(`object_member_key`): the binder's name, or for a computed literal name the
key of its literal type (`[+100]` names `100` and `[-0]` names `0`, though
the binder spells them `+100` and `-0`) when the member is a property
assignment, shorthand or method; an accessor keeps the binder's name
(`get [+100]` names `+100`), as `checkObjectLiteral` renames only the members
it checks as values. Only such a name is typed, never an
initializer; the presence fragment below shares the predicate and the key. A target alternative against which any key is valid makes its
universe every literal: one with an index signature, or one that
`hasExcessProperties` skips (an empty object type, or the global `Object`
type), against which `{meaningful: 1}` is a valid construction. `--all-literals` examines every
literal. The result names the universe and both counts.

Outside the default universe are literals whose named members all name other
keys. They construct a target alternative, for example, under a type
assertion, which relates comparability without excess-property checks
(`{other: 1} as Options`), beside a union member those keys choose, in a
union context whose other member accepts any key (`View | {}`, `View |
object`, `View | Record<string, unknown>`), or with an excess-property error
(TS2353), which a spread or late-bound name beside them does not prevent
(`{...base, other: 1}`; checked with the pinned `tsc`). On the measured targets (below) the default universe found every
site the exhaustive run found.

Two syntactic exclusions are sound and precede semantic work:

* a literal that is an assignment target (`({a} = value)`, `for ({a} of xs)`),
  which is a destructuring pattern, not a construction (`isAssignmentTarget`);
* a **spread contribution** (product rule): a literal whose contextual type
  TypeScript forwards from an enclosing literal's spread operand is part of
  that literal's construction, not a site: `{id, ...(lane ? {lane} : {})}` is
  one site. The walk follows the existing `ContextRecipe` dispatch
  (`reference_context`): parentheses, `!`, `as const`, conditional arms,
  `await` operands, and binary operands exactly when
  `getContextualTypeForBinaryOperand` forwards the binary's own context. It
  stops at any independent context (annotation, assertion, `satisfies`,
  return, argument, element, property, initializer). A spread recipe variant
  replaces today's anonymous `Forward` for spreads so the dispatch has one
  owner.

### Construction

For each examined literal, compute `I`, the instantiated contextual type;
`C`, its apparent form before object-literal discrimination (each
constituent mapped through `getApparentType` unless it is a mapped type);
and `D`, after discrimination (`getApparentTypeOfContextualType` split at
each step; one owner composes them). Absent context means no site. There is no checking prelude: `getContextualType`
is asked on demand, and an argument context resolves its own call. Removing
revision 2's outermost-first prelude changed no pinned compiler answer and
turned 84 `ApiError` gaps on the construction benchmark project into sites.
When no constituent of `I`, or of the apparent forms tried below, is an
instance of a target alternative, the literal is not a site before `D` is
asked: discrimination keeps a subset of the constituents, so it
cannot add one, and asking it could only fail (a narrowed discriminant read
was a spurious gap).

**Independent argument candidate.** An argument's context is
`getTypeAtPosition(getResolvedSignature(call), index)`. When a call or `new`
has exactly one candidate signature, without type parameters, explicit type
arguments, `super`, `import()`, an invoked function expression, or a
construct-via-call, both `chooseOverload` (success) and
`pickLongestCandidateSignature` (failure) return that candidate unchanged,
so the parameter type is known after the call's preparation (callee typing
and candidate inventory) without checking any argument. The evaluator then
answers from the candidate: it marks the call resolving and starts a fresh
type-resolution window while preparing, as `getResolvedSignature` does,
shares that preparation with a later resolution, and keeps and returns a
signature that nested checking installed meanwhile. Results computed while
flow loops are active are not cached. Q3 and Q4 share the path, so a
literal argument of a failing single-signature call is decided rather than
an `overload_recovery` gap there too.

Target alternatives: the declared type's union constituents (or the type
itself) that are object types or intersections of object types and have no
call or construct signature, since no object literal's type has one. A
target with none, or whose alternative is a generic object type
(`isGenericObjectType`), is `UnsupportedTarget`.

Identity is decided on each constituent `X` of `I`, not of `C`:
`getApparentType` rebuilds an intersection with its `this` argument
(`getTypeWithThisArgument`), so the apparent form of `type PointExtra = Point &
{…}`, `type Props = Box<string> & Plain` or `PE<number>` for `type PE<T> =
Box<T> & {…}` is a new type without the alias, and the apparent form of a
type parameter constrained by a class rebuilds the class reference. A
constituent that is not an instance is tried through its base constraint
(`getBaseConstraintOfType`) when it is instantiable, so a type parameter's
constraint, and a class's `this` type, stands for it; any other constituent
is tried through its apparent forms (a primitive's wrapper). An alternative
reached twice is one construction. `X` is an instance of alternative `Y` by
the target's form:

* **Non-generic target** (no own type parameters): `X == Y`. TypeScript
  interns references and alias instantiations, so `Box<number>` behind
  `type Fixed = Box<number>` is one identity; `Box<string>` is another.
* **Generic interface or class**: `X` is a reference whose target is the
  declaration's target type.
* **Generic type alias**: `X` carries this alias's own symbol (its alias
  instantiation), or `X` is an anonymous instantiation whose target is `Y`
  (alternatives of an instantiated union alias lose the alias). An alternative
  with neither provenance (for example an intersection inside a generic union
  alias) makes the target `UnsupportedTarget` rather than a silent non-match.

The literal constructs `Y` when the apparent form of a matching `X` is also a
constituent of `D`:
TypeScript's discrimination removes alternatives it rules out, and an
undiscriminated union keeps every matching alternative. Unions widened with
`undefined` match by constituent. A literal typed by one alternative of a
union target alone (`const f: First = …` for `type Event = First | Second`)
constructs that alternative. Base types, intersection parts, and mapped
derivations (`Partial<T>`) are **not** constructions of `T` (product rule);
query the derived type. `ThisType<T>` contributes method `this` only. The
site's `contextualType` is `I`, or `D` when discrimination removed a member,
which it always does for `undefined` and `null`: a literal annotated with an
alias shows the alias unless its union lost a member.

The identity question is extracted from the native comparison that
`best_matching_type` already makes (`findMatchingTypeReferenceOrTypeAliasReference`),
with Q5's stronger per-form policy kept separate from that reporting rule.

### Presence (compiler model)

Presence is read from TypeScript's **checked literal type** `S`
(`checkObjectLiteral`): it is a statement about the compiler's named-property
model, not about the runtime object. For a property `p` of the constructed
alternative (skipping static private names as `getUnmatchedPropertiesWorker`
does):

* `S` is `any`: `unknown` for every property;
* `S`'s constituents (one, or a union's) each have `p` as a required named
  property: present; none has it: `absent` unless an index signature applicable
  to `p`'s name (`getApplicableIndexInfoForName`) exists in some constituent,
  which makes it `unknown`; otherwise `unknown`.

`S` demands every initializer. Where `S` is not needed for exactness, a
cheaper fragment computes the same answer, used only where it is proved
equivalent to `S` by the oracle: a literal with **no spreads and no late-bound
computed names** (none of its members has a dynamic name, `isDynamicName`)
owns exactly its members' keys (`object_member_key`) as required named
properties and no index signature, so its presence needs no initializer
type; any other key resolves, as `getPropertyOfType` resolves it, through
the global `Object` type's own named members and its augmentations (a
required `toString` is present, an augmented optional member `unknown`;
`Object`'s signatures and index signatures are not the literal's). Every other literal computes `S` with the native owner, and an
initializer failure is a located gap. Private-member suppression by later
spreads and generic spread reduction (an intersection that reduces to
`never`) are exactly why spreads take the full type.

`absent` therefore means "the compiler's type of this literal has no such named
property". A spread of `{}`-typed value can still carry it at runtime; E3 must
establish its own evidence before inserting.

## Ownership

| Knowledge | Owner |
| --- | --- |
| Public request/result types; `--missing` key shape | new `src/constructs.rs` (peer of `types.rs`, `references.rs`) |
| Target selection; declared-kind check | `semantic::selection` (existing), `semantic::constructs` |
| Retained object-literal spans per file | binding (accessor over existing `Decl`s) |
| Filters, test paths | `project::references::Filters` (existing) |
| Assignment-target test | `syntax` (`isAssignmentTarget`) |
| Spread recipe, provenance walk | `source::narrowing::reference_context`; `evaluation::contextual` |
| Binary-operand forwarding decision | `evaluation::contextual`, shared by `contextual_binary_operand_completion` and the walk |
| `C`/`D` split | `evaluation::contextual` (`apparent_contextual_type_completion` composes them) |
| Reference/alias/instantiation identity | extracted from `evaluation::discriminants::best_matching_type`; Q5 form policy in `evaluation::constructs` |
| Target property inventory and optional test | `evaluation::relations`, shared with `getUnmatchedPropertiesWorker` |
| Literal type `S`; written-key fragment | `evaluation::object_literals` (`check_object_literal`), `source::object_literals` |
| Applicable index info | `evaluation` (existing `getApplicableIndexInfoForName` port) |
| Candidate failure → located gap | `semantic::gaps`, extracted from Q3's ledger and shared |
| Default universe; named-member predicate | `evaluation::constructs` (`in_default_universe`, `named_member`, shared with `written_keys`) |
| Independent argument candidate; shared call preparation | `evaluation::calls` (`independent_candidate_completion`, `prepared_signature_completion`) |
| Object literals of a file; syntax hold/release | `source` (`object_literals`, `holds_syntax`, `release_syntax`) |
| Order, result assembly | `semantic::constructs` |
| Rendering, caps, the universe hint | `cli::constructs`; registry `SITES`, `LACKS` and `GAPS` bounds |

Nothing outside `semantic` sees a `TypeId`; results own their data.

## Measurement gate

The construction benchmark project (4,275 searched files, 110,159 object
literals), macOS Intel i9, release build, load average 3–4 during the runs.
Direct times include about 7 s of loading; peak is the process's physical
footprint, the loaded project included.

| Run | Examined | Time | Peak | Sites | Gaps |
| --- | ---: | ---: | ---: | ---: | ---: |
| Revision 2 (prelude, no release), every literal, `ApiError` | 110,159 | killed at 50 s | >6 GiB | | |
| Reparse every file, no semantic work | 110,159 | 3.2 s | 3.3 GB | | |
| Every literal, `ApiError`, one checker, before the last two changes | 110,159 | 62 s | 5.9 GB | 150 | 14,373 |
| Every literal, 2–4 threads, a fresh checker per 4,000–8,000 literals | 110,159 | 77–114 s | 2.9–4.6 GB | 150 | ~14,375 |
| `--all-literals`, `ApiError`, direct | 110,159 | 74 s | 6.1 GB | 150 | 14,246 |
| `--all-literals`, `BackendModelInfo`, direct | 110,159 | 74 s | 6.1 GB | 9 | 14,246 |
| `--all-literals`, `ApiError`, default limits | | 60 s deadline | 5.9 GB | | |
| Default, `ApiError`, direct | 10,537 | 27.6 s | 3.1 GB | 150 | 1,563 |
| Default, `BackendModelInfo`, direct | 15,139 | 30.2 s | 3.5 GB | 9 | 1,553 |
| Default, `PortableMcpConfig`, direct | 6,121 | 20.9 s | 2.7 GB | 99 | 883 |
| Default, `SpecRevision`, direct | 15,695 | 30.8 s | 3.5 GB | 73 | 1,719 |
| Default, warm service, the same four targets | | 19.5, 22.4, 13.8, 23.4 s | | | |

After the implementation review's fixes, the four direct runs found the
same sites and gaps in 21.7, 23.0, 16.3 and 23.4 s at load average 2. After
those service queries the service's footprint was 1.7 GB (peak 3.3 GB, the
rest reclaimable). Call arguments dominate: in the one-checker
exhaustive profile 52,299 argument literals took 39.3 s of the query, array
elements 5.6 s, forwarded contexts 5.4 s, returns 3.3 s, initializers 2.8 s,
nested property values 1.4 s. Memory grows 35–58 KB per examined literal in
the checker's append-only arenas. Gaps, about 13–15% of examined literals,
are mostly `flow_required` (the checker has no flow narrowing),
`unsupported_semantic_form` and `jsx_typing_required`, so a real project's
answer is `partial`.

Decisions. Exhaustive examination did not fit the deadline. The user first
accepted 10–20 s per query as Q5's budget and asked for memory work first.
Two independent investigations (Fable xhigh and Astra xhigh) then put the
reachable exhaustive floor at about 28–45 s query time with fresh checkers,
after several days of behaviour-preserving work. A wider default, which
also examined literals with a spread or late-bound name beside other named
keys, examined 17,606 literals for `ApiError` in 35 s and found no
additional site. The user chose the written-key universe as the default and
exhaustive examination as the opt-in `--all-literals`.

Against that budget: the default universe met it on two of the four measured
targets warm (13.8 and 19.5 s); the other two took 22.4 and 23.4 s, and every
direct run took 21–31 s with loading, more under a heavier load (a
reviewer's concurrent `ApiError` run took 43.5 s with identical counts).

## Alternatives considered

1. **Key-seeded universe** (revision 1): only literals naming a target key.
   It misses `{}`, spread-only and computed-key literals, which are valid
   constructions an optional-property migration needs. Revision 3's default
   adds every literal with no named member and names the universe in the
   result.
2. **Exhaustive by default.** About 60–75 s and 6 GB on the construction
   benchmark project, past the deadline; kept as `--all-literals`.
3. **Parallel or chunked checkers.** Each checker rebuilds the same demanded
   closure, so throughput per thread fell 2–3× and chunking repeats the
   warm-up; slower than one checker at every setting measured.
4. **Module-graph pruning** (files outside the target's reverse import
   closure). The closure held about 73% of files and 87% of literals for
   `ApiError`.
5. **A file prefilter on target-key tokens.** `{}` and spread-only literals
   occur in nearly every file, so it would skip few parses (3.2 s for all).
6. **A checker kept across requests.** Retained memory would exceed the
   envelope.
7. **Follow references to the type.** A value-flow analysis that misses
   generic and inferred contexts; F's work.
8. **Fold spreads without `S`** (revision 1). Not equivalent: later private
   spreads suppress earlier keys, generic spreads reduce to `never`.
9. **Include derived types** (bases, intersection parts). Deferred; it widens
   matching and proof.

## Failures and limits

Gaps are located at the literal with `Ledger::gap`'s rules, extracted: only
unsupported/invalid semantic failures become gaps; resource, I/O and internal
failures abort. Expected gap sources: `flow_required` reads in a call's other
arguments or a spread literal's initializers, JSX attribute contexts,
assignments to property targets, invalid spreads (`unknown`, `never`, falsy),
and cross-product limits. Process limits are the existing ones.

## Proof

* The pinned compiler hook operation `constructs` enumerates **every** object
  literal in searched files independently of the product, marks those the
  default universe examines (from the binder's member names and the
  alternatives' properties and index signatures), applies the two
  exclusions, and reports `C`, `D`, matches and presence from the full `S`.
  Fixture rows are frozen as `expected-<profile>.json`; Rust tests assert the
  final `Project` result for both universes and both profiles, and CLI
  documents, against them, so a missed literal fails.
* Families: annotations, returns (sync, async, arrow bodies), arguments
  (overloads, explicit and inferred generics, `new`), `satisfies`, `as`, array
  elements, nested property values, defaults, assignments, discriminated and
  undiscriminated unions, `| undefined`, non-generic aliases of instantiations
  (`Fixed = Box<number>`, `PA = Partial<A>`), generic interfaces, aliases,
  union aliases and an unsupported generic union of intersections,
  intersection aliases, classes, methods and accessors, `{}`, spread-only,
  spreads (required, optional, union, `any`, generic and its `never` reduction,
  private suppression, invalid), conditional and `await` spread contributions,
  computed literal and late-bound keys, dynamic computed keys (index
  signature), assignment patterns, strict and loose null profiles, and each
  gap source.
* Universe families: literals naming only other keys (excess property, a
  spread beside them, a late-bound name beside them, a union member chosen
  by them, a type assertion), a literal with only a spread and a late-bound
  name, an index-signature target, an empty target, a hexadecimal numeric
  key, and a signed computed name (`[-1]`).
* Identity families: aliases of intersections with a class, a generic
  instantiation, a `this`-typed interface and an interface extending a class;
  a generic alias of such an intersection as an annotation, argument and
  array element; a union alternative that is one; and a type parameter's
  constraint. The oracle asserts an independent floor for every row: a
  literal whose instantiated context is the declared type, an alternative,
  or carries the declaration's alias, is a construction unless
  discrimination removed it.
* Key and presence families: a private name and a unique symbol (compared
  by kind and public name), string keys that are not identifiers (`lane?`,
  a newline, the empty string), and the global `Object` type with standard
  and augmented members (`constructs-object`).
* Argument families for the independent candidate: nested calls, a call in
  a spread operand, rest elements, too many and too few arguments, an
  argument past the parameters, overloaded constructors, an IIFE, an
  inferred IIFE, a getter in the callee's initializer calling the same
  method (TypeScript defers its body), and a union context
  without the target whose discrimination would read a narrowed value.
* The Measurement gate's corpus table, summarized in `docs/constructs.md`.

## Review dispositions (Astra xhigh, revision 1)

| # | Finding | Disposition |
| --- | --- | --- |
| 1 | Key seeding cannot support `complete` | Accepted: exhaustive examined universe; keys may order work; independent all-literal oracle. |
| 2 | Spread fold not equivalent to `checkObjectLiteral` | Accepted: `S` is the reference; only the no-spread, no-late-bound fragment is syntactic, proved by the oracle. |
| 3 | Matching overmatches and misses | Accepted: per-form identity (non-generic identity; generic provenance); unprovable alternatives refuse the target. |
| 4 | Compiler absence is not runtime absence | Accepted: named as the compiler model; applicable index signatures make `unknown`; E3 owns stronger evidence. |
| 5 | Exclusion misses `await` | Accepted: provenance walk over `ContextRecipe`, including `await`, stopping at independent contexts. |
| 6 | Consolidate owners | Accepted: identity extracted from `best_matching_type`; one recipe dispatch; computed names stay in `property_names`; gap extraction kept narrow. |
| 7 | Measurement must gate the architecture | Accepted: Measurement gate before fixing the universe; any latency exception goes to the user. |
| 8 | Complete the result contract | Accepted: `--missing` over alternatives that declare the key; constructed alternatives only; alternative ids and origins; key kinds; aggregate `lacks` budget. |
| 9 | Unknown-key failure kind | Accepted: property-specific `UnknownProperty` query failure, exit policy unchanged. |

## Revision 3 dispositions (performance investigations and decisions)

| Item | Disposition |
| --- | --- |
| Exhaustive examination misses the deadline | Measured; user chose the written-key default universe, named in `coverage`, and `--all-literals`. |
| Call prelude | Removed: identical compiler answers, fewer gaps. |
| Argument contexts dominate | Independent candidate for one non-generic signature; Fable and Astra both found it sound. Astra's request for pinned nested, rest, arity and constructor cases is met by the argument families. Keeping a signature installed during preparation now follows `getResolvedSignature`; no input that installs one was found (a circular callee makes both resolutions untyped), so that rule is unpinned. |
| Unmatched contexts are discriminated | Rejected before discrimination (Astra). |
| Syntax for every file held at once | Released per file unless held earlier (~0.8 GB). A bounded syntax cache (both reviewers) is deferred until measured. |
| Generic naked-parameter, explicit-type-argument and agreeing-overload argument rules (Fable 3.1–3.3) | Deferred: behaviour-preserving speedups for `--all-literals`, each needing its own oracle families. |
| Compact consumer locations in demands, a declaration index for `node.Symbol` lookups, allocation hygiene (Fable 3.4, 3.5, 3.7) | Deferred: shared checker work that benefits every semantic query; measure separately. |
| Parallel checkers | Rejected: duplicated closures, not contention. |

## Implementation review dispositions (Fable xhigh and Astra xhigh)

| Finding | Disposition |
| --- | --- |
| Intersection aliases with a class, generic instantiation or `this`-typed interface lose every site, `complete` (Fable blocking, Astra major) | Fixed: identity on the instantiated contextual type, apparent forms only as fallback and for discrimination; oracle changed likewise, with an independent floor and the identity families. |
| The written-key fragment reports global `Object` members absent (Astra major) | Fixed: unwritten keys resolve through the global `Object` type; `constructs-object` fixture. |
| `[-1]` treated as late-bound; two owners of the dynamic-name rule (Fable) | Fixed: members carry `syntax::is_dynamic_name`; the oracle uses `ast.IsDynamicName`. |
| A literal naming other keys validly constructs an empty target (Astra) | Fixed in the universe: an alternative `hasExcessProperties` skips (empty, global `Object`) examines every literal, as an index signature does; docs list the remaining valid omissions. |
| Text keys ambiguous (`lane?`, newline, empty) (Astra) | Fixed: keys quoted by `--type`'s owner, with `private`/`symbol` kinds. |
| `--missing ''` rejected (Astra); `--missing '#hidden'` message (Fable) | Fixed: any exact string key; the message says string-keyed property. |
| Private and symbol keys unpinned (Fable) | Fixed: the oracle spells them by kind and public name. |
| COMMANDS.md lacks `--all-literals`, `--limit` caps, design example casing (Fable) | Fixed. |
| Budget not summarized (Fable, Astra) | Summarized under Measurement gate. |
| `Filters::resolve` takes Q3's `calls`; a summary failure makes a decided site a gap; the spread walk's forwarding list is kept with the typing path by hand (Fable notes) | Kept: no defect; recorded for a later cleanup. |

## Follow-up review dispositions (Fable xhigh and Astra xhigh)

Both reviews confirmed every first-round finding fixed or, for oracle
independence, partly fixed, and found:

| Finding | Disposition |
| --- | --- |
| `[+N]` and `[-0]` keys reported absent; the binder spells them `+N`, `-0` (both, major; a regression of the `[-1]` fix) | Fixed: one owner of a member's key, `object_member_key` (a computed literal name's type key, as `checkObjectLiteral`), for the universe and the fragment; the oracle uses `checkComputedPropertyName` likewise. |
| A type parameter constrained by a class, an alias of an instantiation, an intersection or a `this`-typed interface, and a class's `this`, lose their sites (both, major) | Fixed: identity on the base constraint for instantiable constituents; the oracle's independent floor now covers base constraints. |
| The global-`Object` fallback reaches `Function` members through `Object`'s signatures and treats its index signature as the literal's (both) | Fixed: only `Object`'s own named members; `constructs-object-index` fixture. |
| An alternative listed twice (`T \| Plain`) (Fable) | Fixed: merged. |
| A function-typed alternative "constructed" (Fable) | Fixed: alternatives with signatures are excluded; a target with none left is refused. |
| A context union with an every-key member is another valid omission; the every-key list omitted the global `Object` (Fable) | Documented. |
| `--missing` echoed unquoted in the header (both) | Fixed: `key_text`. |
| `contextualType` loses `\| undefined`; the design example (Fable) | Documented; the example corrected. |
| Budget sentence miscounted the warm samples (Astra) | Corrected: two of four met it. |

## Round-3 verification dispositions (Fable xhigh and Astra xhigh)

Both verifications found every follow-up finding fixed and one new defect:
`object_member_key` renamed a getter or setter with a `[+N]` or `[-0]` name,
which TypeScript names by the binder (`get [+100]` is `+100`). Fixed: the
member kinds `checkObjectLiteral` renames are one predicate
(`renamed_member`), shared by the checked-literal path and the key owner;
the oracle's `AgentOracleMemberKey` follows the same kinds; accessor, data
and getter-only rows pin it in both universes.
