# Native declaration serialization and syntax reuse

This continues the stage24 diagnostic-node cache. The operating envelope is the
existing request-local checker over captured TypeScript files, with a nil outer
symbol tracker and no hover expansion. It does not add a second parser or a
declaration-emission service. The ordinary inferred-property path is required
by diagnostic type display, including the measured mapped `limited` failure.
Q4 preview/admission remains outside semantic serialization.

## Ownership choice

Routing every inferred declaration straight to `diagnostic_type_node` is a
smaller implementation but changes checker acquisitions, native spelling and
cache publication. Retaining source text as the output is also incorrect:
`reuseTypeNode` qualifies names, visits children, tracks symbols and can reject
reuse after acquiring semantic evidence. The chosen boundary is the complete
native declaration serializer with two private collaborators:

* `source/pseudo_types.rs` ports `PseudoChecker`'s pure grammar from
  `internal/pseudochecker/lookup.go` and `type.go`, over the existing captured
  AST. It returns unnormalized recipes carrying actual declaration, expression
  and type-node handles. Resource interruption remains a failure.
* `native_display/pseudo.rs` owns `pseudoTypeToType`,
  `pseudoTypeEquivalentToType`, and `pseudoTypeToNode` from
  `pseudotypenodebuilder.go`. It uses existing checker completion owners and
  produces private output nodes.
* `native_display/reuse.rs` owns `reuseNode`, `reuseName`, `reuseTypeNode`, and
  `tryReuseExistingNodeHelper` from `nodecopy.go`. Annotation reuse in signature
  and type-parameter display must use this same operation.

`native_display/declarations.rs` remains the sole
`serializeTypeForDeclaration` orchestrator. Its complete private inputs are
optional actual declaration, optional supplied type, optional actual symbol,
and tryReuse. Callers do not select pseudochecker variants or manage temporary
symbol maps. The existing property entry supplies its already acquired type;
recursive inferred/no-result serialization can omit it at native sites.

## Source recipe contract

The recipe is deliberately not a TypeId. It retains native `Direct`, `Inferred`,
`NoResult`, `MaybeConstLocation`, union, primitive/literal, tuple, object and
single-call-signature alternatives. Inferred carries the actual expression,
signature-return bit and ordered error-node handles. Structural alternatives
retain each member's actual name/declaration and parameter/type-parameter order.
Unsupported producer metadata never becomes `NoResult` or `Inferred`; those
variants are native grammar results only.

Port the complete admitted TS grammar rather than a property-only subset:
declarations, optional/default parameters, paired accessors, one-return bodies,
assertions, const ancestry, primitive prefix expressions, arrays and object
members. The pure `isContextuallyTyped` and `IsInConstContext` predicates differ
from checker contextual/const operations and keep separate native-named owners.
The syntactic identifier `undefined` rule does not resolve the identifier.
Variable raw declaration counts and accessor pairing use actual raw declaration
metadata, not a merged symbol or a newly discovered set of members. Existing
`symbol_accessor_recipe` answers a different checker question and is not a
substitute for the pseudochecker's raw pairing operation.

Object eligibility retains source-ordered parse-error, shorthand/spread,
private-name and computed-name gates. Array eligibility retains first-spread,
const-context and contextual-typing gates. Parameter generation computes the
last required parameter before adding native strict-null undefined. Literal
recipes retain original nodes so output can preserve written numeric/string
syntax; identity text continues to use exact decoding owners.

## Semantic schedule

The declaration serializer follows nodebuilderimpl.go2167–2270:

1. Select the actual declaration and symbol using native nil rules. A missing
   supplied type first checks the temporary enclosing-symbol-type map, then
   performs the setter write or symbol read/widening/mapper operation. Preserve
   actual native errorType only at the native fallback branch.
2. Run `requiresAddingImplicitUndefined`, including the actual property read
   even when a type was supplied. Apply optionality only at its parameter gate.
   Save flags and apply the actual same-symbol/same-file unique allowance.
3. At the inferred/accessor, nonnil-scope, non-widening reuse gate, temporarily
   install the supplied symbol type, then obtain the pure recipe. Check
   equivalence with native optional-annotation rules. ErrorType equality returns
   true before any pseudo semantic acquisition.
4. Realize the recipe only where native asks. `Direct` acquires the annotation;
   `Inferred` acquires the actual expression/return and widening; `NoResult` and
   structural recipes return actual nil. A union stops at its first actual nil,
   with the distinct strict-null elision/empty/singleton rules. Semantic failure
   with a native value continues through `NativeCompletion`; interruption stops.
5. Preserve native pointer/regular-literal/union identity comparison order,
   then structural property/signature/tuple comparison. Setter comparison uses
   write type. Count native declarations and preserve actual node/name identity;
   do not equate members by printed names.
6. Perform the actual repeated realization required by undefined retry; it is
   not a memoized pseudo operation. On an equivalent recipe serialize it using
   checker fallback only at native top-level Inferred/JS-annotation gates.
   Otherwise run the ordinary type builder with the native nested-report
   suppression state. Restore the symbol map and flags on all exits.

The three pseudo operations remain distinct. Pure primitive pseudo output does
not increment approximateLength like ordinary `typeToTypeNode`. Pseudo object
output asks the checker's const context, then sets InObjectTypeLiteral and
enters signature scopes where native does. Pseudo function output expands
parameters before entering the shared scope. `MaybeConstLocation` asks the
checker const predicate first and asks contextual type only after the pure
const predicate admits that branch. Top-level Inferred fallback serializes the
supplied checker type, preserving instantiated types rather than reacquiring
the original initializer's type.

## Reuse boundary and failures

`nodecopy.go98–232` requires a private nested recovery boundary within Context.
It snapshots tracker, tracked-symbol list, encounteredError and approximate
length; installs a wrapping tracker; transforms the existing node; restores the
snapshots; flushes deferred reports; and replays tracked symbols only if native
reuse succeeded. Successful reuse adds the original node's span length. Failed
reuse returns native nil, after which `reuseTypeNode` reports inference fallback,
acquires the original type node and runs ordinary type serialization.

Retain two different tracking vectors: `recoveryBoundary.trackedSymbols`
receives wrapping-tracker calls, while `ctx.trackedSymbols` is the node-builder
cache's current list. Native `startRecoveryScope`/`endRecoveryScope` truncates
only the latter and deferred reports; successful boundary finalization replays
the former. One generic rollback vector would silently change that schedule.

The existing nil-tracker boolean cannot represent the wrapping tracker's
deferred-report behavior by itself. Introduce typed private tracker mode/state
at the shared report/track owner, and route existing reports through it. Preserve
the special immediate reports (inference fallback, nonlocal augmentation and
truncation) separately from deferred cyclic/this/unique/nonserializable/import
reports. Native rollback truncates its report/tracking metadata; it never rolls
back completion evidence or checker caches already acquired. Scope rollback
must therefore remain independent from the operation-local `NativeSequence`.
Whole resource/I/O failures restore state and stop immediately.

The tree visitor must cover admitted type syntax as a whole: references and
queries, operators/indexed access, import types, literals, signatures and
parameters, mapped/infer/conditional syntax and their actual name visits. Reuse
of a source node is not ordinary semantic re-rendering. Shared qualification,
module-specifier, synthetic-scope and unique-key prerequisites remain explicit
at their actual branches until those owners are complete; they cannot be
replaced by a successful source-text copy.

## Delivery and proof

The initial cache checkpoint is independently compile-ready. Implementation of
this continuation begins only after the coordinated reopen. The first coherent
next checkpoint is pure recipe generation plus semantic realization/equivalence
and the full declaration orchestration, with reuse routed through its actual
owner. A concrete reached missing reuse prerequisite remains a failure; it is
not grounds to mark the mapped regression complete. The following checkpoint
closes the reuse visitor and shared tracker boundary, then expands qualified
and synthetic-scope proofs. The entire producer remains required scope.

Retain measured mapped `limited` as OverloadRecovery without an Unsupported
suffix, and its no-extra-property healthy twin. Add final Project queries for
cold/warm/different declarations, hidden missing annotations, ordinary versus
const literals, optional/default parameters before required parameters,
getter/setter annotation order, one/nested/multiple returns, contextual arrays,
and annotation reuse that consumes a later missing type. Compare strict, loose
and exact optional-property behavior where each native rule branches.

Native proof uses natural final queries in one checker, with the existing
zero-demand cache observer. Fixed names such as reportedCold are not evidence:
assert an actual reportedDiagnostic/notInstalled event from a valid producer.
Use a valid polymorphic `this` value inside a method for that candidate, and
pin it before asserting the product result. Depth-elision proof similarly must
observe the actual >10 gate, with under/at/over controls. Equal-byte name retry
must retain its observed left/right contextual pair followed by the nil-context
FullyQualified pair. Final tests assert observable outcomes and consumed causes;
helper tests or repeated independent Project calls do not establish cache proof.

## Stage25 coherent implementation checkpoint

The ordinary source recipe, realization/equivalence, declaration orchestration,
output construction and recovery-boundary owners are staged. The visitor now
preserves native `tryVisitSimpleTypeNode`: a failed reference/index/keyof attempt
returns nil to the containing type, while other syntax gets its own visitor
recovery. Wrapping-tracker observations remain separate from the context list;
rollback never removes acquired completion evidence. Peer review covered these
boundaries and the method-versus-function recipe order.

The dedicated cache query tests are in `tests/type_diagnostic_cache.rs`; they
retain the native captured expectations. This checkpoint has not yet compiled
or passed those tests. The required next visitor branches are conditional,
mapped, template and import syntax, failed-name `serializeTypeName`, `this`
entity resolution, and computed-name tracking. Each currently stops at its
actual unavailable operation. They are implementation obligations, not a
redefinition of the ordinary input envelope. Predicate getter completion and
omitted-Any signature output also remain explicit shared producer seams.
