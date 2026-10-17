# T, Q3, Q4 design review, round 2

Reviewed 2026-10-10 against repository `ea23d8b`. This reviews the proposed
design, not an implementation. It follows the reviewing route of the Ousterhout
software design skill and was requested for correctness and maintainable
architecture, especially whether each decision has one owner.

| Reviewed artifact | SHA-256 |
| --- | --- |
| `docs/types-design.md` | `595bfd26a25805528df43c847966d5f9650d912b9ed152729c94f7a4f8ad0fcd` |
| `docs/types-design-evidence.md` | `4c57a52fb499417cc9e608a4cb9b2ab610418eee831b9befc1138cf69329dd2d` |

**Operating envelope assumed:** an unpublished native Rust CLI and library that
agents use locally over one explicit tsconfig. A serialized local service shares
immutable snapshots and fully rebuilds when any input changes. There are no
external Rust consumers, editor buffers or network callers. The user chose a
native T; F (flow), I (incremental updates) and a production compiler backend
are out of scope.

## Method and limits

Nine reviewers each took one dimension: flow, Q3 semantics, checker core,
representation, ownership, public contract, error semantics, Q4 presentation,
and internal consistency. They started from seeded hypotheses, confirmed or
refuted each against the pinned TypeScript 7.0.2 Go source and the repository,
and ran zero-diagnostic compiler probes. The probes are under
`.cc/temp/review2/`. Three independent verifiers then checked each finding:
**evidence** (facts and probes), **design text** (whether the design already
answers it) and **significance** (whether it matters within the envelope). A
finding survives with at least two of three votes.

The run was stopped before it finished, so the review is incomplete in four
ways:

* Checker-core and internal-consistency findings were never verified. They are
  included below only where a verified finding from another dimension
  independently reached the same conclusion. Otherwise they are marked
  *unverified*.
* Error-semantics findings received only the design and significance votes; the
  evidence verifier did not run.
* The completeness rounds and the final merge did not run. I deduplicated and
  ranked the findings by hand.
* Verifiers corrected several overstated details. The corrected version is what
  appears here.

None of the three findings resolved in round 1 is re-raised.

## Summary

The core structure from round 1 still holds: frozen lexical binding, member
tables completed after typing, directed reference relationships, demand-driven
evaluation, request-local caches, and explicit partial results. The problems are
at the next level down. In five places the design states a rule TypeScript
7.0.2 does not follow, or leaves a compiler decision with no owner, inside the
profile it claims to support:

1. **Overload resolution.** A rejected candidate permanently fixes callback
   parameter types. The design forbids exactly that.
2. **Reachability.** Function-end reachability decides every inferred
   block-body return, and no owner is assigned to it.
3. **Flow facts.** In TypeScript the binder's flow graph decides which syntax
   can affect a read. The design substitutes an unported syntax screen.
4. **Compiler limits.** These are semantics in TypeScript. The design treats
   them as measured product budgets.
5. **Q3 candidates.** The inventory goes beyond the compiler's text-based
   candidate search, which contradicts the design's own completeness baseline.

Several cross-cutting decisions also lack a single owner:

* target selection;
* union and intersection order, and alias identity;
* the rule that classifies an answer as invalid;
* JSON facts;
* the candidate token inventory.

The public contracts for Q3 outcomes and the Q4 graph also have gaps that agents
would hit immediately.

**Verdict: revise before implementation.** None of these findings requires
changing scope. Each needs a contract correction, a named compiler owner, or a
fixture.

## High-severity findings

### H1. Overload resolution: rejected candidates fix callback parameter types (3/3, 2/2, unverified duplicate)

**Design:** `types-design.md:615-618`, `:633`. Calls and inference own the
candidate transaction, and "failed candidates cannot publish their inferred
parameter types or poison the winning candidate's cache". Memo keys include
contextual expectations.

**Compiler:** `chooseOverload` (`checker.go:9001-9078`) first checks candidates
with context-sensitive arguments skipped. The first candidate that passes switches
the shared call state to `CheckModeNormal` and keeps that mode for every later
candidate (comment at `:9056-9061`). In that normal-mode check,
`contextuallyCheckFunctionExpressionOrObjectLiteralMethod` (`:10128-10178`) sets
`NodeCheckFlagsContextChecked` once per node, assigns parameter types from that
candidate (`assignContextualParameterTypes`) and caches the return type.
`assignParameterType` (`:10380-10394`) ignores later assignments.

**Probes** (strict mode, zero diagnostics unless stated):

| Source | 7.0.2 | Port that follows the design |
| --- | --- | --- |
| `m(cb: (x: string) => number): 'first'; m(cb: (x: any) => string): 'second'; m(x => x.toUpperCase())` | `'second'`; `x: string` | `x: any` |
| `n(cb: (x: string) => number): 'first'; n<T>(cb: (x: T) => T): T; n(x => x)` | `string` | `unknown` |
| `f(cb: (x: string) => number): 1; f(cb: (x: number) => string): 2; f(x => "s")` | TS2769, `never` | succeeds with `2` |
| `m(x => x.shout())`, with first overload `(x: Shout) => number` | references from `s.shout()` include `x.shout()` | reference dropped, answer still marked complete (an `any` receiver counts as a proven non-reference) |

**Mechanism:** the design moves a compiler decision ("contextual assignment is
once per node") into the wrong owner and invents isolation that 7.0.2 does not
have.

**Remedy:**

* In Calls and inference, port `chooseOverload`'s change of `argCheckMode` and
  the one-time contextual assignment, as a per-call-site record that later
  candidates see.
* Keep inference contexts and speculative diagnostics per candidate.
* Key contextual parameter assignment by node, not by expectation.
* Add fixtures for both declaration orders, single-overload controls, and a
  valid-call variant.

A cheaper exact alternative: report the call as unsupported whenever the first
fully checked candidate with a context-sensitive argument is rejected, and count
those refusals.

**Verifier caveat:** 7.0.2's own on-demand answer for such a parameter depends
on query order. On a fresh checker, the first `getTypeAtPosition` returns the
winner's type, because `getTypeOfVariableOrParameterOrProperty` does not cache
context-sensitive parameters (`checker.go:16457-16476`). Fixtures must state the
baseline: the settled checker state that `tsc` diagnostics use. The oracle must
reach that state before comparing.

**Related, also 2/2 (errors-6):** nothing says what an unsupported,
`flow_required` or invalid-dependency result inside a candidate does. The phrase
"Speculative errors remain local to the owner…" (`:627-628`) invites treating
these results as inapplicability, which turns them into "No overload matches" or
lets a later overload succeed. State that a candidate is rejected only by an
established TypeScript inapplicability, reached in TypeScript's own argument
order and pass order. Any other blocker fails the whole call.

### H2. Function-end reachability has no rule or owner (3/3)

**Design:** the Bodies row supports "inferred returns whose dependencies do not
require flow analysis" (`:68`). Reachability is in neither the Flow row's
exclusions (`:69`) nor the admission rules (`:88-112`).

**Compiler:** `checkAndAggregateReturnExpressionTypes` (`checker.go:20164`)
starts from `functionHasImplicitReturn` (`:20212`), which is `isReachableFlowNode`
(`flow.go:2483`) applied to the binder's `EndFlowNode`. That walk consults:

* `getEffectsSignature`, at calls to never-returning or asserting functions;
* `isExhaustiveSwitchStatement` (`flow.go:1912`), which needs the discriminant's
  type.

Two things depend on the result. `mayReturnNever` makes function expressions,
arrows and object-literal methods with no return infer `never`. Under
`strictNullChecks` it also decides whether `undefined` joins the return union.

**Probes** (TypeScript 7.0.2):

| Source | Inferred type |
| --- | --- |
| `function kind(x: "a" \| "b") { switch (x) { case "a": return 1; case "b": return 2; } }` | `1 \| 2` (with a third case: `1 \| 2 \| undefined`) |
| `createRunHost(deps: Deps) { return { observe(x) { deps.log(x); }, stop() { deps.die("stop"); } }; }` | `{ observe(x: string): void; stop(): never }` |
| `const loop = () => { while (true) {} }` versus `function loopDecl() { while (true) {} }` | `() => never` versus `() => void` |
| `items.map(i => { fail(i); })`, where `fail` returns `never` | `never[]` |

**Consequence:** an implementer must invent the rule. If reachability is
computed syntactically from the binder alone, the tool returns wrong types
inside the profile, the motivating factory shape among them. If it is refused,
most block-bodied callbacks and factories become Q4 errors and Q3 gaps, a cost
the design does not disclose.

**Remedy:**

* Name function-end reachability as an evaluation operation.
* Port `functionHasImplicitReturn`, `mayReturnNever`, `isReachableFlowNode`
  (including its cache), `getEffectsSignature`/`getTypeOfDottedName` and
  `isExhaustiveSwitchStatement` over the flow graph described in H3.
* Send the discriminant read through normal admission.
* State that function declarations and class methods never infer `never`.
* Add the fixtures above, with `strictNullChecks` on and off.

### H3. Flow-fact ownership: the syntax screen duplicates the binder's decision (3/3), and the read rules are imprecise (3/3 each)

**Design:** the semantic-source owner captures "conservative syntax facts"
(`:79-81`, `:506`) with no TypeScript function behind them. The change scenario
for F says a new flow construct is supported by extending admission "using
captured source facts" (`:661`).

**Compiler:** one owner decides which syntax can affect a read: the binder's
flow-graph construction (`binder.go`: `createFlowCondition` guarded by
`isNarrowingExpression`, `bindAssignmentTargetFlow`,
`maybeBindExpressionFlowIfCall`, the bind functions for loops, `try` and
`switch`, `EndFlowNode` at `:1561`). The flow nodes are attached to identifiers
and type-query names. That construction is about 1,040 non-blank lines. The
checker interprets the graph through `isMatchingReference`. T already needs the
graph in this delivery for H2.

Relevance is not syntactic. Probes show narrowing through an aliased condition
(`const isA = a.kind === "A"; if (isA) …`), through destructured siblings, and
inside an optional-chain root. A region-based screen either misses these, giving
wrong answers, or refuses broadly.

**Read rules that pass every enumerated rejection yet change the type** (flow-3):

| Rule (`checker.go`) | Example | 7.0.2 type of the read |
| --- | --- | --- |
| `getNarrowableTypeForReference` (`:31248`) substitutes union constraints at constraint positions | `x.toString()` with `x: T extends string \| number` | `string \| number`; `const c = x` gives `T` |
| Optional chain narrows its own root | inner `chain` in `chain?.b.c(chain.d)` | object type without `undefined` |
| `removeOptionalityFromDeclaredType` (`:11183`) for defaulted parameters | `x: string \| undefined = "a"` | `string` |

The design's "initialized annotated binding" rule refuses the defaulted
parameter, but only once "initial type" is defined as `checkIdentifier`'s
`initialType`.

**Initializer refinement is keyed on the wrong property** (flow-4).
`getTypeAtFlowAssignment` refines a read by its own initializer only when the
declared type, annotated or widened, is a union (`flow.go:246-248`):

* `let flag = false; const c = flag` gives `false`.
* `let mode = E.A` gives `E.A`.
* With overloads on `true`/`false`/`boolean`, `pick(done)` chooses `"f"`.
* An annotated non-union type such as `const host: RunHost = createRunHost()` is
  never refined.

The design keys the rule on "annotated" (`:97-99`). Verifiers note that the
general rule at `:95-96` makes a faithful implementer refuse these reads, so the
practical defect is unnecessary refusal of the motivating dependency-injection
shape plus an under-specified rule. It is not a guaranteed wrong answer.

**The blanket predicate refusal disables Array `filter`/`find`/`findLast`/`every`**
(flow-5). These methods' first overloads take `value is S`, so inference and the
applicability check call `getTypePredicateOfSignature` on the callback
(`inference.go:863-865`, `relater.go:1599-1601`). That call runs predicate
inference (`checker.go:20470`, `:20499`). Under `:110-111`, every such call
becomes `flow_required`. Command Center has about 5,000 such calls, and roughly
1,300–1,600 of the relevant callbacks are in non-test source. The design does
not disclose this cost.

A narrower rule is sound: answer "no predicate" when the parameter's declared
type is not a union, `unknown`, `any` or a constrained type parameter, and the
return expression contains no condition matching the parameter.

**Remedy (one owner):**

* Binding ports the flow graph. Its doc comments name the bind and flow
  functions listed above.
* Replace "conservative syntax facts" with flow-node handles.
* Admission walks from the read's flow node and refuses at the first node that
  can concern the reference.
* An admitted read returns `checkIdentifier`'s pre-flow type,
  `getNarrowableTypeForReference(getNarrowedTypeOfSymbol(…))`, with
  `initialType` chosen as the compiler does.
* Restate initializer refinement as the union test.
* Rewrite the change scenario at `:661`.
* Add fixtures for each branch above.

This is not F: no narrowing algorithm is ported, only the graph and the "no
matching node" walk. (flow-6, low, is subsumed: the call-effect blocker should
be keyed on `FlowCall` nodes and explicitly typed names, as
`getEffectsSignature` does.)

### H4. Compiler limits are semantics, not measured budgets (2/2, unverified duplicates)

**Design:** "Finite computation is bounded by semantic resource limits" (`:66`).
Budgets are set "from fixture and real-project measurements" (`:641`), the
constants are implementation choices (`:777`), a resource failure aborts a Q3
search (`:340-341`) and is "never ordinary candidate rejection" (`:629`).

**Compiler:** limits are fixed constants inside specific operations, and they
decide answers:

| Limit (`checker.go` unless noted) | Effect when reached |
| --- | --- |
| `instantiationDepth == 100 \|\| instantiationCount >= 5_000_000` (`:22016`) | `errorType` plus TS2589 |
| Conditional type `tailCount == 1000` (`:24218`) | error |
| `checkCrossProductUnion` (`:26519`) at 100,000 | error |
| `removeSubtypes` complexity | error |
| `recursiveTypeRelatedTo`: maybe-stack (`relater.go`) | returns *Maybe* for pairs already on the stack |
| `recursiveTypeRelatedTo`: `isDeeplyNestedType` (`relater.go:768`) | treats sufficiently nested types as related |
| `recursiveTypeRelatedTo`: depth overflow | false relation, which can reject an overload candidate |

The maybe-stack behavior directly contradicts `:625` ("do not return true for
every pair already on a stack"). Whether a limit is hit can depend on what was
evaluated earlier, and instantiation counts reset per statement or expression.

**Consequence:** if budgets are above TypeScript's constants, Q4 returns types
where 7.0.2 gives the error type. If they are below, valid answers fail. If
TypeScript's limits are folded into `ResourceLimit`, one deep candidate aborts a
whole Q3 search or overload resolution, where TypeScript would reject one
candidate.

**Remedy:**

* Port each limit inside its operation, naming the function, with its constants,
  reset points and outcome.
* Classify reaching a limit as an invalid semantic operation with its own reason
  code: a Q4 error or one Q3 gap.
* Keep the product work budget separate and strictly above the work needed to
  reach TypeScript's limits. Its exhaustion stays a resource failure.
* State the evaluation-history baseline used for comparison.
* Fix `:625`.
* Add just-under, at and over fixtures for each limit.

### H5. Q3 candidates exceed the compiler's reference operation (3/3; owner gap 3/3)

**Design:** a separate list of "computed or unresolved names that cannot safely
be excluded", and gaps for "every unresolved-name site that might designate it"
(`:333-348`). The same section says completeness "is relative to the compiler's
static reference operation" (`:350-352`).

**Compiler:** `getPossibleSymbolReferencePositions`
(`findallreferences.go:1538-1580`) is an identifier-bounded `strings.Index`
search of the raw text, followed by symbol resolution. Two consequences:

* `host[key]` and declarations like `[key]` are never member candidates. A
  computed site whose key token spells the name resolves to the variable, not
  the member.
* The round-1 probe confirms it: references from `late.value` omit the computed
  `[key]` declaration, and a computed-getter use returns no references.

**Consequence:** where 7.0.2 is complete (exit 0), the design either reports
extra occurrences, failing checkpoint 6's comparison, or gaps (exit 1). Every
member search must also type every non-literal computed site in the project.
Separately (coherence-1, unverified), undotted lexical answers are labelled
`complete` although the compiler's operation also reports same-symbol uses
through receivers and binding keys. The design has two baselines for
"complete" and declares neither.

**Owner gap** (owners-5, repr-6): the inventory is captured at load, but only
Reference search names "candidate inventory traversal" (`:513`). The syntax set
is given by example. "Exact member keys" is claimed by both Type model and
Member tables, and the inventory's matching key is a third, unassigned question.
The binder's per-file token table already classifies identifiers, so member
`--at` would have to consult two tables to apply lexical precedence.

**Remedy:**

* Port the compiler's candidate rule into one owner:
  `getPossibleSymbolReferencePositions`, `isValidReferencePosition` and
  `getMeaningFromLocation`.
* Extend binding's existing token table to record every name-bearing token once,
  with its lexical target, syntactic role and a member key from one key owner.
  That covers identifiers, private names, string, template and numeric literal
  names, element-access keys, and binding-property and JSX attribute names.
* Delete the computed list and the "might designate" gaps.
* State the baseline of each search kind.
* Record deliberate differences in `docs/references.md` (refs-6, low).
  7.0.2 searches raw text and misses escaped identifiers such as `observe`
  and numeric spellings such as `0x1`, while exact decoded identity finds them.

## Medium-severity findings

### M1. Q3 relationships missing from the three named functions (3/3 each)

* **Mapped origins** (refs-2). `GetRootSymbols` (`services.go:392-440`) maps
  homomorphic mapped properties (`Partial`, `Readonly`, `Pick`, `Omit`) to
  `syntheticOrigin`. The design names only instantiation and union/intersection
  roots (`:272-276`, `:289-291`). Verifier correction: ordinary spreads reuse the
  original symbol (`getSpreadSymbol`), so only readonly-changing, set-only and
  optional-overlap spreads need `leftSpread`/`rightSpread` links. Name
  `GetRootSymbols` as the root operation, require those links in the type model,
  and add fixtures for each form seeded at both declaration and use.
* **Union-type-literal seed substitution** (refs-4).
  `skipPastExportOrImportSpecifierOrUnion` in `getReferencedSymbolsForSymbol`
  replaces a seed declared in an inline type-literal union constituent with the
  union property, but only when every constituent has the property. A port of
  only the three named functions misses sibling constituents. Its interface-union
  twin legitimately behaves differently, so implementers cannot infer it.
* **JSX attributes** (refs-3). The compiler relates attribute names to props
  members through the attributes' contextual type. The design defers JSX typing,
  so these sites can only become gaps. Command Center has about 19,600 attribute
  names on component tags, about 13,000 outside tests. Disclose this with a JSX
  reason code and add a props-member query to checkpoint 6. Property-access tag
  names (`<host.Panel/>`) resolve normally and are not gaps.

### M2. Late and early member seeds pin a 7.0.2 evaluation-order artifact (3/3; unverified duplicate)

**Design:** separate early named-declaration and combined late-bound views
(`:547-548`, `:555-559`), compared per seed against the compiler (`:751-753`).

**Probes:** a cold 7.0.2 server's named-declaration seed returns only the
declaration, or nothing when a use precedes the declarations or they are in
another file. A use seed returns both. Uses of members declared only through
computed keys return no references at all. Requesting diagnostics first does
not change this, because the semantic API uses a different checker.

**Consequence:** checkpoint 4 has no well-defined expected outcome, and the
separate early view's only observable effect is this artifact.

**Remedy:**

* Define Q3 identity as the combined member, which is the steady state after
  `getResolvedMembersOrExportsOfSymbol`.
* Have the oracle query a use before the declaration seed.
* Record the cold-checker and computed-use behaviors as deliberate differences.

### M3. Type identity, alias attachment and member order have the wrong owners (3/3, 2/2, unverified duplicate)

**Design:** a type key is "semantic kind and actual substitutions", with alias
as "associated provenance" (`:600-603`). Storage may intern equal structures
(`:609`). The presenter owns aliases and "a deterministic semantic order for
alternatives" (`:374-376`, `:514`).

**Compiler:**

* Each aliased instantiation is a separate cached type, keyed by alias symbol
  and alias type arguments.
* `getAliasSymbolForTypeNode` attaches the alias when a type is built from a
  node.
* Unions are kept in `CompareTypes` order, which sorts by flags, alias or
  declared name, alias type arguments, literal values and declaration order.
* Intersections keep insertion order.
* Members follow `sortSymbols`: declaration position, then name.

These orders are deterministic, contrary to the design's premise for inventing
its own, and they are semantic. Union order decides which mutually subtype
constituent survives reduction, and therefore which declaration a member access
resolves to: Q3 groups and Q4 origins. Interning `Obj<string>` without its alias
collapses `A1 | A2` into one constituent.

**Remedy:**

* Put alias attachment, alias identity and union origins in type construction.
* Port `CompareTypes`, `addTypeToUnion` and `addTypeToIntersection` as the one
  canonical order.
* Have the presenter preserve the model's order.
* Use 7.0.2's member order instead of exact-name order, or record the
  difference with a rationale (present-9).

### M4. The invalid-operation rule contradicts itself (unverified, corroborated by refs-10 at 2/2)

**Design:** `:156-157` makes a rejected `satisfies` relation invalid "even if
TypeScript continues with an ordinary-looking recovery type". `:158-160` exempts
diagnostics outside the "queried dependency path".

**Compiler:** `checkSatisfiesExpression` computes the expression type before the
relation and returns it unchanged. There is no recovery type and no data
dependency. Q3's contextual relation through a failed `satisfies` is also
unaffected in 7.0.2. "Dependency path" is never defined.

**Remedy:** adopt one principle with one owner. An answer is invalid exactly
when a value it depends on, by data dependency, came from a compiler recovery
path:

* error-type returns;
* `getCandidateForOverloadFailure` and its helpers;
* unresolved symbols;
* limit outcomes (H4).

Either drop `satisfies` from the rule or keep it as a declared product
difference.

### M5. Target selection has no owner, and its rules break at the edges (3/3, 3/3, 2/3, 3/3)

**Ownership** (owners-2, repr-3). The table has no selection row, although
selection covers:

* Q2-catalog names and dotted selectors;
* member token targeting;
* dual-role precedence;
* alias meaning;
* the `(FileId, semantic site)` bridge;
* smallest-node anchors;
* the question kind;
* name/position agreement.

The diagram places it inside both the reference node and the type node.
"Semantic site" is undefined. Q2 anchors and bound declaration sites differ: a
parameter-property token anchors two bound declarations, and dotted namespaces
anchor the first segment.

**Anchors do not round-trip** (contract-4). Pasting Q2's suggested anchor into
reference `--at` can select a different symbol. Lexical precedence takes the
parameter, not the property, and the namespace's first segment rather than the
named member.

**Anchor kinds are unspecified or depart from the compiler** (contract-5,
contract-6). The Q4 rules omit enums, enum members, namespaces, accessors,
constructors, signatures and module blocks, though Q2 can select all of them. A
name inside a type reference has the error type in 7.0.2's position typing,
while the design's general rule implies the referenced type. Tagged-template
results have no anchor. The alias table departs from `getTypeOfNode` in several
forms (import-equals of a namespace, export type, default) without recording
them. Verifiers rejected one sub-claim: the class-declaration versus
ordinary-import asymmetry is TypeScript's own.

**Remedy:**

* Add a Target selection owner: one operation maps a name or position to a typed
  target plus question kind, consuming the navigation catalog and binding's token
  targets.
* Define the semantic site as declaration node identity plus role.
* Render ambiguity anchors that its own `--at` resolves to the same seed.
* Add a table from anchor form and target meaning to question, naming
  `getTypeOfNode` and `isTypeDeclaration` where it ports them.

### M6. Q3 outcome contract (3/3 each)

* **Exit 1 has two meanings** (contract-1). A partial answer writes a document
  and exits 1, while existing reference failures write a stderr diagnostic and
  leave `-o` untouched. `coverage.status` is a discriminator, but nothing makes
  "says prominently" concrete. Add `outcome: "complete" | "partial"` near the top
  of the document and specify the text form.
* **Ambiguous or unmatched dotted selectors have no result** (contract-2).
  `Result<ReferenceAnswer, AnalysisError>` cannot carry candidates or not-found,
  and "the existing error document" does not exist for references. Today a
  dotted `--name` is a usage error, exit 2. Mirror editing's
  `Selected | Unselected(Ambiguous/NotFound)` with exit 1.
* **A member-only bare name returns an empty `complete` answer** (contract-3).
  The same `--name` finds the method under `--show` and `--type`. Keep the
  lexical semantics, but disclose matching member selectors from the catalog,
  for example `unsearched_members: {count, selectors}`.
* **Sites of one symbol reported as ambiguous** (contract-7 and refs-11, low).
  Overload signatures and merged declarations are separate catalog entries, but
  they seed the same search and give the same Q4 answer. Collapse candidates
  that denote one symbol before reporting ambiguity.

### M7. Q4 graph schema cannot carry facts the compiler keeps (3/3 each unless noted)

| Finding | Gap | Remedy |
| --- | --- | --- |
| present-1 | One `type` per property. Divergent accessor write types and the `exactOptionalPropertyTypes` missing/`undefined` distinction cannot be shown, yet checkpoint 5 promises "optional read/write distinctions". | Optional `writeType`, emitted only when it differs. |
| present-3 | Signatures lack declared `x is T`/`asserts` predicates, `this` parameters and optional/rest flags, so distinct function types look identical. | Add the fields. |
| present-4 | No type-argument or array-element edges. `RunEvent` inside `Promise<RunEvent>` is reachable only through library members, though callers "never parse display text". | `typeArguments` and `element` edges; state a default for library generics. |
| present-5 | Summaries have no length or elision rule. An anonymous type's summary inlines every nested type: a 3,000-entry JSON table renders about 212 KB as one summary, and each node summary repeats its subtree. | Bounded summaries that elide at token boundaries (`… N more …`, `{ … }`) with a flag; inside a graph, node summaries name children rather than inlining them. |
| present-6 | Elision works only on whole nodes, so a wide member list is all or nothing. | `omitted_members: N` with `truncated: true`. |
| present-7 | Shared nodes plus depth "from the root" have no traversal rule. A depth-first walk hides members within the requested depth. | Breadth-first, with a stable child order. |
| present-8 (2/3) | One unsupported member fails the whole Q4 answer, even the summary, while Q3 returns partial results. | Typed `unsupported` leaves with reason and site, and a non-success outcome. |
| present-10/11/12 (low) | The depth-zero example erases the node kind and sets `truncated` when nothing is hidden. `name` cannot tell symbol, private and string keys apart. The text output and the units of the limits are unspecified. | Fix the example and name the kinds and units. |

### M8. Representation chosen without the measurement the codebase uses (3/3); migration undecided (3/3)

**Representation** (repr-1). Retained terms must encode nearly the whole
expression, statement and type grammar with parents and spans. Contextual
typing, Q4 smallest-node selection, Q3 roles and reachability all need that.
The result is in effect a second syntax tree that every session keeps and every
refresh rebuilds. The repository already reprojects captured text per request
for Q2 and editing, after navigation measured eager retention at +58% retained
heap. The design does leave layout open (`:570-571`), but `:706` rejects
reparsing captured text without data. That differs from reparsing from disk,
which `:569` already forbids.

Remedy: make term production a hidden decision of the semantic-source owner.
The evaluator asks for descriptions by `(FileId, site)` and holds no arenas. At
checkpoint 2, measure eager terms against on-demand reprojection from captured
text, as navigation did, including the load and refresh cost for old
operations.

**Migration** (repr-2). Three decisions are left open: which name uses
`BoundProgram` resolves, how the current lossy projections are replaced, and
what "duplicated facts" means. Decide them, or name them as checkpoint-2 exit
criteria.

### M9. JSON facts are the wrong ones and lack an owner (3/3 each)

**Wrong facts** (repr-5, also coherence-9). `checker.go:16502` types a JSON
module as `getWidenedType(getWidenedLiteralType(…))`. No literal value appears
in any JSON module or property type, so "JSON leaf values must be retained for
typing" (`:70`, `:576`) is wrong. What T and Q3 do need is each value's kind,
object keys with the span of **every** occurrence, and recursive array element
structure. TypeScript reports both declarations of a repeated key, and the last
one's value type wins. `null` and `[]` depend on `strictNullChecks`. The design
also never says how JSON files, which a separate reader parses, feed member
tables or the Q3 inventory.

**No owner** (repr-4, owners-4). The linker's name-only JSON property lookup
(`value_property`; the `json_property` the design cites does not exist) already
decides load-time binding errors. It cannot wait for type operations, yet the
design also forbids two `getPropertyOfType` implementations. Give JSON facts one
owner used by both layers, and document binding's lookup as a deliberately
narrow port with an explicit "undecided" result.

### M10. Shared-service failure semantics (2/2 each, evidence lens not run)

* **Cost is not tied to the client's wait** (errors-4). The serialized service
  was justified by sub-second lexical queries, and its client waits at most
  300 s. A Q3 call can approach a whole-program check with request-local caches,
  and concurrent agents' lexical, navigation and editing calls queue behind it.
  Calibrate the deterministic work budget so it is exhausted well inside the
  client wait. Add lexical latency under a concurrent semantic query to
  checkpoint 6.
* **Internal failures can take the service down** (errors-5, repr-7).
  "Internal" failures have no representation. A checker panic or stack overflow
  kills the process every agent in the worktree shares: `on_parsing_thread`
  re-raises panics (`src/analysis.rs:71-75`), and no thread or stack is
  specified for semantic recursion. Run semantic work on a named thread with a
  measured stack, and catch panics at the query boundary as a typed `Internal`
  failure.
* **The resource error kind is overloaded** (repr-8, errors-7, low). The design
  keeps checker limits under `ResourceLimit`, which today means an input's parse
  budget. Name the semantic variants, with H4's limit classification.

### M11. Core ownership and diagram (3/3; others unverified)

* **The diagram and the table disagree** (owners-3). The diagram is acyclic and
  merges rows. Type operations owns "declared/member types" while Evaluation
  owns "types of declarations", although 7.0.2 computes both in
  `getTypeOfSymbol` by checking initializers. Assign `getTypeOfSymbol` to
  Evaluation and `getDeclaredTypeOfSymbol`, apparent types and lookup to Type
  operations. Draw the back-edges the text requires. Verifiers note that
  `:516-520` does state the cycles; the diagram is incomplete rather than hiding
  them.
* **Re-entrance is per operation** (checker-6 and owners-6, the latter 3/3).
  One "evaluating" memo state cannot express it:
  * member resolution returns the early members during late binding;
  * call resolution uses a resolving-signature sentinel and a resolution window;
  * type-resolution cycles mark every participant (`pushTypeResolution`,
    `findResolutionCycleStartIndex`).

  Specify each and name the ported function.
* **Constant evaluation has no owner** (checker-5, unverified). It defines
  template-expression literal types, enum member values and computed member
  keys. Add an owner porting the evaluator and `computeEnumMemberValues`. The
  phrase "no function-body execution" invites leaving it out.
* **Options** (checker-7, unverified). 7.0.2 turns the strict family on when
  `strict` is unset, but the current config owner reads no strict option.
  `noPropertyAccessFromIndexSignature` is diagnostic-only (TS4111), yet listed as
  must-honor. Default and namespace import types depend on per-file module
  format, which the resolver keeps private. Add a table classifying each
  accepted option as type-affecting, with the operation that consults it, or
  diagnostic-only.

## Low-severity findings (verified unless marked)

* **Filters change the group set** (refs-8). Filtering before resolution means
  related groups whose only occurrences are filtered out disappear. Non-test
  references cannot change (`populateSearchSymbolSet` seeds only from the
  selection), but say so.
* **`--calls` owner** (refs-9, owners-8). The outer-expression owner is
  unnamed; 7.0.2 has `skipOuterExpressions` and `isCallOrNewExpressionTarget`,
  and the repository already ports a narrower form. The call-like set should
  decide decorators and JSX tags explicitly. `instanceof` cannot drop a member.
* **Vocabulary** (contract-8). Reason codes, Q4 error kinds and node kinds,
  which agents branch on, are open sets spelled inconsistently. Close them.
* **Conflicting rule sets** (contract-9). "Follow existing project-query rules"
  (`:176`) points at two incompatible rule sets, references and navigation.
* **Profile omissions** (checker-8, unverified). Variadic tuple elements and two
  of 7.0.2's six intrinsic aliases (`NoInfer`, `Awaited` hooks) are missing.
* **Checkpoint 6 is not reproducible** (coherence-11, unverified). `RunHost`
  does not exist in Command Center at its current revision (`92fe6ab06`), and
  the checkpoint has no acceptance criterion.
* **Determinism** (coherence-12/13 and errors-9, unverified). Undefined terms
  such as "useful aliases" and "eligible", missing orders for gaps and origins,
  and no direct-versus-service proof for the new outputs.

## Structures examined and found sound

These held up under refutation and should not be changed in response to this
review:

* **"Not a generic query engine"** is consistent with evaluator demand
  scheduling. 7.0.2 itself uses a closed, typed resolution stack.
* **Name-to-binding attachment** has one owner in substance: binding already
  resolves every identifier token. Resource-budget charging at allocation and
  scheduling points is one ledger, not two.
* **`Project::references` and `Project::type_of` are deep** operations. The
  legacy `find_references` uses the same owner.
* **Extending `ReferenceResult` additively** is sound. In-repository consumers
  read only `groups` and errors, and CLI/service build parity is enforced by
  `TSPARSE_SERVICE_BUILD`.
* **Envelope-justified mechanisms:** document-local graph IDs, causal chains,
  the coverage ledger, explicit work stacks and memo states each serve a
  condition inside the envelope.
* **Display limits cannot hide a semantic failure** (`:397-401`). Long semantic
  queries do not affect the service idle timeout.
* **Correct as stated:** `typeof x` in type queries; `strictPropertyInitialization`
  as diagnostic-only for read types; the punctuation anchors as consequences of
  the smallest-node rule; the class-declaration versus import asymmetry (it is
  TypeScript's own); the `any`-receiver non-reference claim; optional-property
  read types under `exactOptionalPropertyTypes`.

## Suggested revision order

1. **Compiler-rule corrections:** H1, H2, H3, H4 and M3–M4. They change what
   counts as a correct answer, so fixtures in checkpoint 1 depend on them.
2. **Owner assignments:** the flow graph in binding; one token inventory; target
   selection; JSON facts; type construction order and aliases; constant
   evaluation; per-operation re-entrance.
3. **Public contracts:** the Q3 outcome and selection results (M6) and the Q4
   graph schema (M7).
4. **Measurement and envelope:** checkpoints M8, M10 and the disclosed refusal
   costs (reachability, predicates, JSX).

Probe sources and results for every compiler claim above are under
`.cc/temp/review2/<dimension>/` and `.cc/temp/review2/verify-<id>-<lens>/`. Run
them with:

```sh
bash -c 'source scripts/validate/common.sh; find_semantic_compilers; node <probe>.mjs'
```
