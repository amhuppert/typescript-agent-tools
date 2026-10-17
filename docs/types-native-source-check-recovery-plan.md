# Checked type-node continuation

This is the next proof and dependency closure for existing TS/TSX type-node
checking. It uses the current request-local completion, source projection,
JSDoc parser and resource owners. It adds no diagnostic output surface or
second symbol resolver.

The unresolved-alias acquisition matrix is maintained GREEN in
`vrun-08796d03-36b0-4072-bd2b-823b4b0a0c43` (27 roots in each of strict, loose
and exact profiles). That matrix proves `getTypeFromTypeNode` argument and
default acquisition. It does not prove `checkSourceElement`'s additional
children, diagnostic relations, or declaration checks. The stage-19 source
scheduler and constraint completion changes are staged for the shared Rust
checkpoint; they are not yet validated by the new matrix below.

## Checked source proof

`fixtures/types/native-source-check-recovery` and
`tools/type-native-source-check-recovery-oracle.test.mjs` contain 25 isolated
queries in three profiles. They use actual untyped call, constructor and tag
checking to consume type arguments before runtime arguments. The compiler
hook is unchanged. Initial capture
`vrun-9771c430-62c5-4659-9f2c-077b39180c9c` passed all three profiles; the 25
rows are identical across the profiles. All three exact expected files are
retained and the harness comparison is now unconditional. It records
before-summary diagnostics, existing recovery observations and the public
result. The maintained rerun passed all three profiles with zero skips in
`vrun-b2e2dd4e-0e3c-4738-9611-1d9d41d0a90a`. Final Rust validation remains
pending.

The variants follow native checker 2978–3023 and each selected source worker:

- A missing argument followed by another argument and a runtime property read.
- A nested missing reference: child source checks precede the outer reference,
  while subsequent type acquisition can read the already-established child.
- A discarded alias argument still receives its source check.
- Incorrect arity still checks written children under `checkTypeReferenceNode`;
  the paired type-acquisition-only alias skips them before arity recovery.
- Union/intersection, parenthesized readonly array, named optional tuple,
  variadic tuple prepass, and grammar early-stop followed by child checks.
- Both conditional result branches, infer constraints, signature constraints,
  defaults, parameters and return, constructors, literal members, indexed
  access, template substitutions, mapped name/template, and type queries.
- A first constraint comparison returns False, yet the later parameter
  constraint is still acquired before the next source argument.
- New/tag invocation variants, genuine healthy Any, and a healthy request after
  all invalid requests.

`tests/type_source_check_recovery.rs` asserts the original primary site and
ordered first occurrences of distinct later failure sites. Every retained
cause must match an expected reason and actual declaration location; no omitted
causes are permitted in these bounded controls. Repeated cache evidence is
allowed for compound workers, so this is not a proof of cache-read multiplicity.
The simple list/new/tag and acquisition-only arity cases also assert the exact
full cause count. The test does not infer validity from final Any or errorType.
Native
sorted diagnostic codes establish which diagnostics occurred; their array
order is not a trace. Expected cause order must also follow the pinned worker
schedule, including actual cache acquisitions. Fresh forward/reverse request
orders and compact/expanded queries cover request isolation and presentation
independence. The arity pair distinguishes source-check child acquisition from
type-acquisition arity recovery; the variadic tuple's first failure precedes
the written head, and a rejected constraint still acquires its following
constraint. These orders come from the pinned workers, not the captured code
arrays. A controlled reversion of the completion fold must fail at least
the first/last/runtime-argument assertion before this behavior is called proven.

## Deprecated reference dependency

Pinned `checkTypeReferenceOrImport` (checker 2995) has a distinct operation
from `checkDeprecatedSignature`:

1. Acquire the node type. An actual error type skips constraints and deprecation.
2. With written arguments, acquire type parameters and run argument constraints.
   The constraints loop keeps acquiring constraints after its relation is False.
3. Observe `getResolvedSymbolOrNil(node)` (13827). This reads the existing
   SymbolNodeLinks slot; it does not resolve the symbol or consume its prior
   recovery a second time.
4. In declaration order, `Some(IsTypeDeclaration && IsDeprecatedDeclaration)`.
   Reuse `syntax::is_type_declaration` and `Source::deprecated_declaration`.
5. On the first positive confirmation, `addDeprecatedSuggestionWorker` (13968)
   rescans all original declarations for the first `GetJSDocDeprecatedTag`.
   The confirmation's ancestor and this original declaration are different
   questions. Reuse `Source::jsdoc_deprecated_tag` and its existing cache.

There is no TypeToString or signature display here. A hidden-property Missing
control is therefore not evidence that deprecated reference checking ran.
Suggestion name/range formatting does not acquire semantic types.

The staged implementation is a nonforcing observation over the existing
`GenericsState.reference_symbols` native slot, plus one source-check helper
using actual `ReferenceDeclaration` identities. An installed completion exposes
only its symbol value for this native observation; a product failure is not an
installed native symbol. No additional cache or binder lookup is justified.
The source helper projects the existing exact `IsTypeDeclaration` rule rather
than duplicating a list of kinds. JSON declarations cannot be TS type
declarations and must remain distinct in the shared declaration sum.

The source worker now uses a constraint-only conditional: zero arguments skip
constraints, not deprecation.
The final deprecation step follows a completed False/recovered constraint check;
whole interruptions stop it. Actual error-type recovery retains the early exit.

Proof should use simple `type Old = number` declarations with lazy deprecated
JSDoc. Compare quick `0 as Old` with checked `(0 as Old) satisfies number`,
zero-argument and generic references, merged interface declaration ordering,
negative cheap hints, and error-type/arity gates. Both ordinary summaries may
remain `number`. A future coordinated zero-demand hook can observe the actual
`IsDeprecatedDeclaration` return and the existing suggestion-worker tag hit;
it must not call GetSuggestionDiagnostics or parse an extra tag itself.

The staged final resource test uses the existing controlled semantic worker
with short/long JSDoc and same-file quick/checked roots, for both nongeneric and
generic references. Its discriminator adds the short pair's measured checking
overhead to the long quick query's work. The long checked query must exceed
that cutoff; at/under/over of its full work and subsequent healthy requests are
also checked. This is unrun. The cutoff still needs an exact omission
counterfactual: ordinary source-check overhead alone must not make the test
pass. Until that discriminating resource witness is established,
healthy summary comparisons plus native observations prove the schedule only,
not final product behavior of this otherwise diagnostic-only branch.

## Remaining optional constraint bridge

The stage-19 constraint cache preserves native noConstraint/circular sentinel
values with recovery and keeps interruptions outside the cache value. Its
formerly nonoptional `ClonedRawParameterConstraint` TypeDemand bridge is now
removed: the existing constraint association stores either an annotation demand
or the actual cloned target/mapper. The latter asks the guarded target
constraint, instantiates only Some, and installs completed None with its
recovery in the same raw cache. Reporting clones keep their distinct nil raw
slot. No extra cache or guessed Unknown/Circular type is introduced.

Bounded peer review against checker16984–17018 passed the target, absence and
cache branches. `native-cloned-constraint-recovery` adds ten standalone native
queries: typed/inferred circular calls, mutual constraints, Missing constraints,
ordinary absence, substituted bounds, later argument failure and healthy
controls. Initial native capture passed `vrun-2d617796-9fd4-4266-85fb-63b3baa61906`.
All three pinned baselines now have unconditional comparison; the maintained
combined deprecated/clone run passed `vrun-a53d1a73-4c1c-4184-8fcd-cc2677930df6`
(six profiles, zero skips). `tests/type_cloned_constraint_recovery.rs` is staged
for final compact/expanded queries in forward/reverse request order; it has not
run yet. Its mutual-cycle order follows sentinel acquisition (recursive U,
then V), independently of the later diagnostic emission order.


## Stage-21 tuple and indexed acquisition continuation

The stage-20 source25 run `vrun-2b579e31-3aa8-4a60-896d-df17a944103f`
passed three final groups and failed two. `tupleChildren` stopped in named
optional tuple acquisition after MissingTupleFirst; `indexedChildren` stopped
in a property's raw type acquisition after MissingIndexedValue. The source
scheduler was already sequencing completions; these child owners admitted
recovery before returning their native values.

The repair keeps the existing node caches and changes their entries to
NativeResult. Array/tuple/named/optional/readonly acquisition carries all checked
children through normalization and optionality. The same normalizer retains
its native false/errorType and variadic missing-element decisions. Shared
mapType accepts completion callbacks while preserving Never, union origin,
unchanged identity, and optional nil filtering. Indexed access sequences object,
index, alias, selected property/index values, unions/intersections and applicable
index comparisons; native absence remains optional until getIndexedAccessTypeEx
chooses Unknown for nil access nodes or errorType for actual nodes. The deferred
index source check retains its ordered diagnostic display work and returned
errorType. Existing constraints consume the new tuple/index completion APIs.

The source25 final regression remains unchanged. The indexed simplification,
structured inventory, contextual property and flow owners still have explicit
legacy admission boundaries; this repair does not claim to migrate those
separate native operations or omitted index diagnostics. No fallback type is
created for an interruption. Integrated validation and a controlled omission
counterfactual remain required after the repair compiles.
