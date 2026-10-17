# T, Q3, Q4 design review

Reviewed 2026-10-09 against repository `ea23d8bbbb34d5609aa9ba7057b479805c61ca7b`.
This is a review of the proposed design, not an implementation review.

Reviewed artifacts:

| File | SHA-256 |
| --- | --- |
| `docs/types-design.md` | `bd0f7ae46e8b0f530510703a17e6185ccdc8e22e17ed362100ba6711eb856e25` |
| `docs/types-design-evidence.md` | `6fcb145dc906f905089b22c695f4c9fabb417c8b02b88d1ce69229aeeb45e856` |

The review follows the Ousterhout skill's **reviewing** route, particularly
information hiding within a module, complete operations, and error semantics.
The assumed operating envelope is the one established by the repository and
design: an unpublished native Rust CLI/library over one captured disk project,
with the existing serialized local service. T is a substantial native compiler
investment; F, I, production compiler integration, and new service machinery
remain outside scope. No numerical complexity targets are assumed.

## Findings

### 1. High — Final member identity cannot all be frozen before type evaluation

**Location:** `types-design.md`, “Hierarchical ownership,” lines 358–364, and
“Preserve and extend existing owners,” especially lines 385–389 and 402–414.

The proposed frozen bound program is a sound replacement for the current lossy
index, but the requirement that “Merging still happens once in binding; the
checker never re-merges symbols by name” is too broad for the supported member
profile. TypeScript deliberately completes some member binding only after it
has evaluated a computed name's type. This is required for literal and unique
symbol names, merged interface members, and computed accessors already admitted
by this design.

The current implementation supports the narrower rule it actually needs:
`src/project/binding/collect.rs:186` retains lexical declarations, enum names,
and selected apparent-interface names; `src/project/binding/link.rs:449` runs
augmentation merging before producing the final reference index.
`src/project/mod.rs:89` then discards the binding program. Retaining more of that
product is necessary, but does not make every future member symbol a precomputed
lexical symbol.

Pinned compiler evidence is explicit. In
`typescript-go-7.0.2/internal/checker/checker.go`,
`getLateBoundSymbol` at line 14334 forces member-table resolution;
`getResolvedMembersOrExportsOfSymbol` at line 15843 obtains early members and
adds late members; and `lateBindMember` at line 15918 type-checks the computed
name, derives its exact member key, and combines declarations. The source
specifically names merging computed accessors as a reason for the late symbol.

A zero-diagnostic 7.0.2 probe used:

```ts
const key: "value" = "value";
interface Late { [key]: string }
interface Late { value: string }
declare const late: Late;
late.value;
class Accessors {
  get [key](): number { return 1; }
  set [key](v: number) {}
}
new Accessors().value;
```

The symbol at the named interface declaration has that declaration alone; the
symbol at `late.value` has both interface declarations. The accessor-use symbol
has both getter and setter declarations. The language server also distinguishes
the named declaration seed from the late member-use seed in its reference
results. Globally canonicalizing all these symbols prematurely would erase
observable reference behavior; moving type evaluation into initial binding
would instead make old queries depend on newly demanded semantics.

**Remedy:** Freeze lexical identities and member declaration recipes, including
pending computed names. Give one member-table operation ownership of completing
those recipes after typed keys are available, returning member identity and
origins to both the evaluator and reference search. Keep the lexical bound
program immutable; retain early, late, and synthetic member distinctions where
the compiler does. Reuse the applicable declaration-combination rules through
their owner instead of duplicating a miniature binder in the checker. Explicitly
limit “merging once” to each applicable operation, rather than requiring all
member construction to finish during project loading.

This is an ownership correction, not a request for a new subsystem or mutable
global symbol registry. It preserves the intended information hiding while
removing a dependency cycle the current contract would otherwise force.

### 2. High — The flow admission examples promise more than a syntactic screen proves

**Location:** `types-design.md`, “Proposed semantic profile,” lines 64–87, and
the “Semantic source” ownership row at line 359.

The strict `flow_required` boundary is appropriate. However, the first admission
rule treats straight-line immutable initializers and explicit annotations as
safe categories and says that explicit annotations remain queryable in regions
containing flow. Those categories are not themselves flow-free. A query can
acquire a flow dependency through an annotation or a derived signature even
when its selected syntax contains no branch or mutation.

The review's zero-diagnostic 7.0.2 probe establishes three counterexamples:

```ts
const annotated: string | number = 1;
const copy = annotated; // number, not string | number

function nonNull(x: string | undefined) {
  return x !== undefined;
} // (x: string | undefined) => x is string, not => boolean

function narrow(x: string | number) {
  if (typeof x === "string") {
    const y: typeof x = x; // the annotation itself denotes string
    return y;
  }
}
```

The first consumes assignment refinement without later writes or a guard. The
second requires inferred-predicate analysis, explicitly excluded by the profile,
despite having one simple return expression. The third defeats the unconditional
annotation exception. In the pinned compiler, `getTypeFromTypeQueryNode`
(`checker.go:24010`) checks an expression; it does not simply copy the declared
type. `getTypePredicateFromBody` (`checker.go:20440`) ultimately invokes flow
analysis through `checkIfExpressionRefinesParameter`.

There is also an internal boundary consequence: deciding that a call is an
assertion call is not generally a syntax-only operation. A semantic-source
projection that promises a definitive answer must either conservatively reject
all potentially relevant calls or consult signature semantics. Leaving that
choice implicit encourages admission decisions to spread into inference and
query handlers.

**Remedy:** Make admission a property of semantic demands and all their
dependencies. The source owner can capture conservative syntactic facts and
obvious disqualifiers; the evaluation owner must reject a demand that reaches
an unsupported flow operation, including through `typeof`, initializers,
contextual signatures, and inferred predicates. Qualify the annotation exception
to annotations whose own dependencies are supported. State a conservative
admission rule for annotated-initializer reads and possibly inferred predicates;
do not silently return their declared union or a boolean signature.

Add these examples to the expected-outcome boundary matrix before implementation.
This does not require implementing F. Honest refusal is sufficient, provided
the advertised supported subset and refusal explanation agree. The fixed
Command Center query set should expose the resulting refusal cost, as the
design already requires.

### 3. Medium — Import declarations cannot uniformly select a value type

**Location:** `types-design.md`, “Selection,” lines 173–177.

The selection contract says that imports denote their value types. A valid
import of an interface or type alias can have no value type. Q2 can select a
renamed import specifier, so this is reachable through both supported selection
forms, not an obscure internal state.

For `export interface Imported { x: number }` and
`import type { Imported as Renamed } from './exports'`, the 7.0.2 API reports
`Imported` at the import declaration. `getDeclaredTypeOfSymbol` also returns
`Imported`, while `getTypeOfSymbol` returns the error type displayed as `any`.
The native source explains this exact distinction in `getTypeOfAlias`
(`checker.go:18511`). Under the proposed blanket rule, a valid imported type
would therefore become an invalid semantic query.

**Remedy:** Resolve the alias and choose the question from its available
type/value meanings and the selection context. Specify the declaration-anchor
policy for a type-only target and for a target with both meanings; an ordinary
imported class may deliberately expose its constructor value, but that should
not imply that an imported interface has one. Include a renamed imported
interface/type alias and an imported class in the selection examples. Keep
`--name` and declaration-anchor `--at` consistent.

This needs a small contract correction in the existing selection owner, not a
new public selector language.

## Structures that should be retained

The separation between declaration identity, instantiated member provenance,
and seed-dependent reference relationships is justified by the compiler source
and the author's probes. In particular, an equivalence-class merge would lose
the contextual asymmetry already demonstrated in the evidence. The reference
owner's use of `populateSearchSymbolSet`, `forEachRelatedSymbol`, and
`getRelatedSymbol` is a useful boundary; it should not move into assignability.

The proposed hierarchy also goes beyond a shallow top-level facade in useful
ways: calls/inference owns candidate transactions; relations own direction and
recursive-relation rules; type operations own lookup and substitution; and
presentation owns bounded traversal without performing inference. These are
meaningful secrets rather than arbitrary file boundaries. Exact Rust layout can
remain open. The findings above correct specific conflicts within that
hierarchy, rather than calling for additional general-purpose abstractions.

Request-local caches, the existing snapshot/session boundary, and no second
service or invalidation protocol fit the local operating envelope. The complete
candidate ledger, explicit partial results, and hard resource failures are also
sound contracts. Their implementation proof is still substantial, but the
absence of that implementation is not a design defect by itself.

## Remaining proof and integration risks

These are implementation obligations or local clarifications, not additional
reasons to expand the scope:

* Retained semantic terms, whole-candidate Q3 demand, and request-local caches
  have no measured cost yet. Keep the proposed real-project memory, latency,
  and refusal checkpoints before accepting the chosen representation and
  caching policy. Compiler API probes do not prove native feasibility.
* Define how the legacy `find_references` result carries partial coverage. The
  current `ReferenceResult` (`src/references.rs:38`) has only `project` and
  `groups`; a convenience wrapper must not discard the new coverage field.
  Concrete public response examples would make this and truncated Q4 results
  easier to review.
* Freeze the actual 7.0.2 semantic option matrix. The speculative
  `keyofStringsOnly` entry should be removed if absent from the pinned catalog,
  and supported strict-family and property-access options need explicit owners
  and demand-level expectations. Configuration inheritance must continue to
  use `src/project/program/config.rs`.
* Preserve dual-role token behavior while extending member selection. Existing
  lexical shorthand and parameter-property targets need an explicit policy;
  the semantic reference search must still preserve the selected site.
* Distinguish failed semantic operations from the compiler's displayable
  recovery types. Not every diagnostic recovery is detected by an
  `isErrorType` flag. Failed overload applicability and assignability-sensitive
  `satisfies` need final query assertions, while unrelated diagnostics and
  unvalidated annotated bodies must not turn this into a full-project checker.
* Q4 display limits and semantic work limits must stay independent. A legal
  recursive graph is not a failed inference cycle; non-repeating generic growth
  is not a cycle; omitted output must not hide a demanded unsupported operation.
  The design already states the right distinctions and requires their proof.

## Evidence and limits of this review

Read the current project load, binding collection/linking/index, configuration,
and reference-query paths; navigation, reference, and editing contracts; the
active charter and AGENTS.md; and the relevant pinned checker/language-service
source. The original design/evidence claims were assessed as compiler probes,
not native implementation results.

Additional disposable probes are in `.cc/temp/review-types-probes.mjs` and
`.cc/temp/review-types-probes/results.json`. They use the repository's existing
semantic API and language-server harnesses, report TypeScript 7.0.2, and produce
zero semantic diagnostics. The substantive source cases and outcomes are
retained above so the review does not depend on those temporary files. Re-run:

```sh
bash -c 'source scripts/validate/common.sh; find_semantic_compilers; node .cc/temp/review-types-probes.mjs'
```

A mapped-key-renaming probe did **not** establish a missing cross-name reference
relationship: the compiler kept the original property and renamed synthetic
property searches separate. No finding is asserted from that suspicion.
Production code and the reviewed design/evidence files were not modified.

## Verdict

**Revise before implementation.** Findings 1 and 2 conflict with required
semantic behavior and the proposed ownership/admission contracts. Finding 3 is
a small but observable selection correction. After those are resolved, the
remaining native-checker breadth, coverage, and performance risks can be handled
by the design's implementation checkpoints; this review does not require a
production backend change, F, I, or a generic checker framework.

## Follow-up review — 2026-10-09

This focused follow-up uses the same Ousterhout reviewing route and operating
envelope. The original assessment and hashes above are preserved. Reviewed the
revised design and response, and the evidence document's new review references,
against the compiler observations already established in the original review.
No native implementation or new conformance claim is being assessed.

| Revised artifact | SHA-256 |
| --- | --- |
| `docs/types-design.md` | `595bfd26a25805528df43c847966d5f9650d912b9ed152729c94f7a4f8ad0fcd` |
| `docs/types-design-evidence.md` | `4c57a52fb499417cc9e608a4cb9b2ab610418eee831b9befc1138cf69329dd2d` |
| `docs/types-review-response.md` | `ce1075825af27d33d99fcf5465b6a37c0138dd7c82d58eccb1fece18691f38b7` |

### Disposition of the original findings

1. **Resolved in the design.** “Hierarchical ownership” and “Preserve and extend
   existing owners” now distinguish frozen lexical binding from type-dependent
   member completion (`types-design.md:505`, `:534`, `:544`). The member-table
   operation owns early/late declaration combination, preserves the different
   declaration and lookup views, and receives computed-key demands through the
   evaluator. Owner/side/substitution memo keys are explicit (`:606`). This
   removes the original requirement to complete every member identity before
   typing without introducing a mutable global symbol registry or duplicating
   reference relatedness. The early/late interface and accessor examples are
   also explicit implementation fixtures (`:751`).

2. **Resolved in the design.** Flow admission now belongs to the evaluation
   owner and follows dependencies through annotations, contextual types,
   initializers, computed keys and signatures (`types-design.md:78`). Syntax
   capture supplies conservative facts rather than deciding semantic admission.
   The three counterexamples have explicit refusal outcomes, and the contract
   no longer treats an annotation or immutable initializer as a general
   exception (`:88`–`:123`). Possible assertion/never-return effects require
   signature knowledge or conservative refusal. This is a coherent boundary
   for T without implementing F; its practical refusal rate remains an
   implementation measurement, not an exactness claim.

3. **Resolved in the design.** Alias-anchor selection distinguishes targets
   with only a type meaning, targets with both meanings, and value-only targets
   (`types-design.md:235`). The imported-interface failure is removed. Ordinary
   and type-only imported classes have explicit product selection rules, and
   name/anchor consistency and access restrictions remain obligations (`:258`,
   `:733`). The distinction between inspecting a value's type and authorizing a
   runtime use is clear.

### Adjacent clarification check

The extended `ReferenceResult` now carries coverage and filters through both
library entry points (`types-design.md:198`); no adapter silently strips partial
coverage. The public examples distinguish a partial search from a complete empty
search, preserve filtered-out declaration metadata, and distinguish display
elision from semantic failure (`:405`). Document-local graph references remain
separate from compiler and internal identities.

The deliberate lexical precedence for shorthand and parameter-property tokens
is now documented (`types-design.md:219`). It preserves the existing query
question while member-use seeds retain the compiler's related-symbol behavior.
The option discussion removes the speculative legacy option, identifies the
additional strict/property-access settings, and keeps configuration conversion
with its existing owner (`:130`). Finally, demand-local operation failures are
explicitly independent of the compiler's error-type sentinel (`:155`), while
unrelated diagnostics and unvalidated annotated bodies remain outside the
query's validity claim. These clarifications introduce no substantiated new
contradiction in the reviewed scope.

### Revised verdict

**Ready to guide implementation; no remaining design blocker was found in this
focused follow-up.** This is a technical design-readiness verdict, not a claim
that T/Q3/Q4 is implemented, proven, or newly authorized by this review.

The remaining risks are the ones explicitly assigned to implementation:
native semantic breadth, sound conservative admission, candidate completeness,
recursive/contextual inference, resource accounting, and memory/latency and
refusal rates on the motivating project. Keep the compiler-derived behavior
tests and real-project checkpoints; do not report these resolved document
findings as production fixes or successful delivery evidence.
