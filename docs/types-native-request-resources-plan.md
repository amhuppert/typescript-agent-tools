> Superseded for initial single-user use on 2026-10-16. The user authorized
> process memory/timeout protection and removal of granular work/allocation
> accounting. See [the current design](types-design.md) and
> [completion checkpoint](types-completion-correctness-2026-10-13.md).
> The historical plan below is retained as context, not a current release gate.

# Evaluator and model resource boundary audit

Accepted Ousterhout design, under implementation. Storage/transport and pure
union, cached filtering/mapping and recursive kind-predicate accounting have
scoped final-query proof. Their dated checkpoints state the exact coverage.
Whole-evaluator accounting, production calibration and teardown remain open. Historical audit
observations below describe the starting point; later checkpoints record changes.
The envelope is an immutable captured Project, one synchronous scoped semantic
worker per query, one request-local evaluator and model, and a serialized service.
CheckerInputs is shared by loading and requests. Mutable resource state belongs
to the request, not to Project. The location-copy increment below refines how
the ephemeral Copy input view borrows that request capability.

### Implemented refinement: borrow the existing capability for source locations

The source-location factory is shared by more than eighty semantic call sites.
Passing a separate optional controller through every caller would make each one
responsible for remembering the same allocation rule. Instead, the ephemeral
`CheckerInputs` view borrows `Option<&Control>` alongside its captured facts.
Both request constructors supply the existing controller; the loading constructor
supplies None. Nested consumers reuse the evaluator's view rather than rebuilding
one from Project. Evaluator and Source derive their owned controller handles from
this borrowed capability, removing their redundant constructor argument.

This supersedes the original proposal below to keep Control entirely separate
from CheckerInputs. The view owns no ledger, and neither Project, retained binder
products nor symbol/type identities acquire a controller. Location construction
charges the exact filename copy at its existing owner; source positioning retains
its bounded UTF-16 checkpoint implementation. Loading still creates no request
budget. ASCII and all UTF-8 scalar widths, native final locations, exact resource
boundaries, isolated controller omission and selection compatibility have scoped
proof recorded in the completion checkpoint. Other Location/error copies remain
separate accounting obligations.

## Finding and chosen ownership

`resources::Control` already owns checked work, cumulative requested allocation
payload, balanced checked depth, a sticky first interruption, and a 30-second
deadline. `semantic/mod.rs::worker_with_limits` constructs one handle, checks at
entry, and checks a successful result after request-state destruction. However,
`Evaluator::new` moves the handle only into Source. Model and evaluator state have
no handle. The deterministic production work/bytes/depth maxima are all None;
their calibration is explicitly pending in `types-resource-source-checkpoint.md`.

Consequently, model-only normalization, repeated cached checker work, relation
and inference loops, and arena construction can run between Source operations
without any deadline or work observation. The worker's final check can reject a
late success but cannot enforce a liveness bound while that work runs. Existing
native instantiation/relation/cross-product limits answer different questions
and do not bound all product work, memory, Rust stack, or teardown.

Three alternatives:

1. Charge only public demand/Source entry points. This is small but leaves pure
   model loops, mapper recursion, warm-cache loops, and allocations unbounded;
   every caller would have to know hidden work performed below it.
2. Introduce a general evaluator VM/scheduler or a second checker-wide budget
   service. This would rewrite compiler control flow and split the existing
   sticky-failure/accounting policy without a concurrency or process requirement.
3. Extend the same Control into existing operation and allocation owners.
   This is the smallest coherent extension. Each recursive native operation
   guards its own invocation; each iterative owner charges its next bounded
   unit; each storage owner reserves before growth. No semantic rule, native
   counter, or presentation budget moves into resources.

Choose 3. Clone the same Control into Evaluator, Model, and Source for a
controlled request. Never create a fresh ledger inside a nested evaluator/model/
helper. Keep parser and ordinary nonsemantic entrypoint behavior unchanged.
There is no need for locks, thread cancellation, an allocator backend trait, or
a public resource tuning API. The future loader uses the existing optional-
Control convention; absence means no request product budget, not a fresh
30-second ledger. Control is separate from immutable CheckerInputs.

## Failure transport: compared alternatives and accepted choice

Parent review accepted the narrow typed request-abort transport described below.
This is a transport decision for a whole abandoned request, not a new semantic
failure family. Source/parser keep their existing explicit Result transport.
Both paths use the same Control, Stop variants and sticky first interruption.

**Explicit fallible factories.** Model::push would return Result<TypeId, Stop>
or a semantic error, making every allocating factory and its callers propagate
that result. This is sound and keeps normal Rust control flow visible. Existing
manual cleanup continues to run before an Err propagates. It is appropriate if
callers can recover and reuse the checker after resource failure. They cannot
in this operating contract: every product interruption aborts the whole query.

A source inventory finds 41 model methods directly growing the type or mapper
arenas across model.rs and model submodules; 35 currently return plain values.
This excludes indirect allocating wrappers, property/signature/demand arenas,
collection copies, literal byte operations and pure recursive predicates. For
example, regular_literal_type has 16 external model call sites across eight
files, filter_type seven across six files, combine_mappers twelve across seven,
and maybe_type_of_kind 28 across eighteen. Widening the latter introduces error
handling into boolean callbacks as well as constructors. A naive migration of
primitive/special would also touch 107/206 calls, but that cost is avoidable:
startup initializes those finite intrinsics, so dedicated startup allocation
and established read access can preserve their plain accessors. These are
change-amplification observations, not a design-quality score or an exact patch
estimate. The substantive burden is propagating an unhandleable request abort
through compiler interfaces and adapting UnionFailure's retry/dependency domain.

**Private typed request abort.** Keep infallible model factories infallible to
their ordinary callers. At their existing scheduling/allocation owners, reuse
checked Control operations and reserve helpers; on Stop, a single private
resource transport resumes unwinding with a private Stop payload. It produces
no placeholder TypeId, false predicate, unknown type, cache hit or normal return.
The semantic worker recognizes only that private payload and calls its existing
resource_failure conversion. All other unwinding panics still become Internal.
Use resume_unwind rather than panic_any: resource interruption must not invoke
the process panic hook. No global hook, thread-local budget, cancellation flag,
new public option, or alternate model implementation is needed.

Choose the typed abort for the controlled evaluator/model region. It puts the
unrecoverable request-discard decision at its real boundary and avoids exposing
it through dozens of otherwise useful compiler interfaces. It does not reduce
the accounting work: every unbounded loop, recursion edge and allocation owner
below still needs coverage. It also does not catch allocator abort, stack
overflow, or a panic during unwinding.

### Audited unwind, speculation and state boundaries

- `semantic/mod.rs::worker_with_limits` is the sole production catch_unwind.
  Today it maps every payload to Internal. Change that one dispatch to recognize
  the private resource payload first, retaining the ordinary-panic policy.
  Keep the catch outside construction, selected work and destruction of the
  evaluator. The entry/final checks retain their existing Result behavior.
- `analysis.rs::on_parsing_thread` joins and resumes a thread panic; it does not
  classify or tolerate one. The typed request abort must be consumed inside the
  semantic thread. Cargo has no panic=abort profile; supporting such a build
  would require revisiting this explicit dependency.
- There are no other production catches in src. Parser matched/attempt/probe/
  look_ahead and angle speculation classify Result/Option, not panic payloads.
  Their current Source-controlled Stop Results stay failures. A typed abort
  from checker work bypasses semantic overload/relation/inference Result
  alternatives, rather than becoming a mismatch, false result or later branch.
  Any future internal catch must resume this private payload unchanged; add an
  architecture check/review obligation at new catches, not ad hoc fallback.
- QueryBindings owns Rc<RefCell<RequestState>> created per request. Resolver's
  Request branch mutates only that sparse state; retained Bindings are immutable.
  Model's IdentityOrder and presentation's identity registry are also local.
  Signature Arc<OnceLock> slots are created in the evaluator and stay within
  that evaluator under the accepted fresh-initialization design. No such
  mutable checker state is published to Project or another request.
- Native manual cleanup (active mapper pop/depth decrement, relation session
  restoration, inference priority/context restoration, resolver journal/cuts)
  is skipped by abort. That is safe only because no code resumes or reuses the
  abandoned evaluator/resolver and their entire owners drop. This is different
  from ordinary Result interruptions, whose existing cleanup remains required.
  Resource DepthGuard and Rust borrow guards do Drop during unwinding; no
  cleanup may acquire more budget. The only production custom Drop found is
  DepthGuard; there is no model/evaluator destructor consulting arena alignment
  or reentering semantic work. Standard container teardown is still measured.
- Do not let the request payload escape into a later catch that treats all
  panics as absence. Keep it private and created only from a denied checked
  operation. The payload carries the Stop already selected by Control, never a
  second deadline check or a guessed reason. An internal panic merely occurring
  after Control stopped remains an internal panic; classify the payload, not
  the ledger bit alone.

The proposed emergency-allocation policy is a fixed, input-independent boundary
allowance for one private Stop payload and one existing pathless resource error
envelope, including its fixed message. Keep that allowance outside the exhausted
request payload ledger, derive its size from the concrete payload/error types
and static message bytes, and never call Control during abort or reporting.
No native causes, source paths, diagnostic display or model allocation enters
this allowance. Preconstructing all four public errors for every request would
add startup allocations without removing the unwinder's own possible allocator
use; the simple bounded allowance is preferred. This is an explicit logical
budget policy, not a claim of OS/allocator-OOM recovery: stable Box/String
allocation and the unwind runtime can still abort after a real try_reserve
failure. The existing Source Result error path has that same reporting limit.
Production acceptance needs measured headroom for the envelope and teardown,
not a statement that a successful byte-budget check reserves physical memory.

### Exact loading policy

Current Project::load has no product Control or request deadline. Parser
loading uses its existing grammar budget. `Project::on_loading_thread` maps
thread-start IO and otherwise resumes panics through the common thread helper.
InputErrorKind::ResourceLimit and docs/types-design.md explicitly describe a
parse limit; converting a checker Stop to that kind would change the contract.

Preserve that policy when enabling shared checker initialization: loading passes
no request product Control, while Q3/Q4 pass the same live request ledger through
initialization and selected work. Existing resource reserve helpers already
accept Option<&Control>, so reuse that convention at the shared ownership seam.
Do not create a default Control inside the shared initializer. No request-abort
catch or resource-to-InputError mapping is needed on the absent-Control load
path. Existing load grammar, admission, IO and ordinary panic outcomes remain.

If a finite load-time product budget is later authorized, its discard boundary
is the entire loading invocation in on_loading_thread, enclosing initialization
and any projected Linker writes. Catching a single merge and continuing with a
partially mutated projection is forbidden. The catch would recognize only the
same private payload, resume all other panics unchanged, and require an explicit
load error contract before conversion. This audit does not invent that policy.

## First coherent implementation checkpoint

The first increment should cover evaluator scheduling plus model construction
and the semantic algorithms exercised by the selected proof fixtures. It must
not be advertised as complete resource safety while other families remain open.

1. Pass the controlled-request capability into Model before initialization.
   Startup allocates intrinsics, primitive unions, mapper records and empty
   objects, so wrapping a completed model misses the zero/near-zero byte
   boundary. Keep Model::new's ordinary return type; denied controlled startup
   aborts to the already installed semantic boundary. Established intrinsic
   accessors need no invented semantic failure.
2. Keep Model::push as the type-arena growth owner. It appends four aligned
   vectors (types, symbols, object_flags, type_mappers). Reserve all required
   capacity before mutating any length or publishing a TypeId. Even though the
   abort path discards the whole model, preserve this local publication invariant
   rather than making Drop-safety the sole argument for a half-installed type.
   A denied later reservation may leave spare capacity; there is no refund.
3. Apply the same rule to mapper allocation, property-symbol insertion,
   signature records, demand registration and cache/interner growth. Existing
   allocation-returning TypeId/MapperId APIs retain their ordinary signatures;
   the denied path cannot return any ID. Charge variable payload preparation
   before copying and reserve destination tables before publication.
4. Add checked invocation guards at actual recursive scheduling owners, before
   a child or cache replay can bypass accounting. Demand-only checks cannot
   cover direct checker calls. These guards clone Control, never borrow &mut
   Evaluator across recursion. Stop aborts the whole controlled request; native
   and ordinary semantic failures retain their existing NativeResult transport.
5. Charge iterative work and scratch growth inside the owners listed below.
   Explicit traversal stacks consume work/allocation per item, not depth
   proportional to a flat list. Product depth measures actual guarded nesting;
   native logical depth/tail counts remain independent.
6. Keep resumable model errors such as UnionFailure in their current semantic
   domain. Resource abort bypasses retry handlers completely. Source/parser
   Stop Results still go through resource_failure; neither route may be
   converted to a dependency request, ordering failure or clean recovery.

`resources::reserve`, reserve_map/reserve_set/reserve_string and checked push
already implement growth policy. Extend those mechanisms for genuinely missing
container operations rather than creating evaluator-specific growth formulas.
The owner of a semantic operation still controls when it publishes native state.

## Concrete scheduling and allocation hotspots

This inventory traces current code, rather than inferring cost from file size.
Line numbers are approximate because the context migration is in flight.

| Owner / location | Work or allocation requiring a boundary |
| --- | --- |
| evaluation.rs demand_completion (~434), register_signature_demand (~340), demands/signature_demands | Cached acquisitions still perform work; new demand/recipe/cache entries grow vectors/maps. Property demands dispatch directly to their actual symbol owner, so a demand-only guard cannot cover other direct acquisitions. |
| declared.rs check_type_completion; expression.rs check_expression_completion/cached owners | Recursive type/expression entry, ordinary cache replay, and child dispatch. Preserve native expression count resets. No generic resource layer should move those resets. |
| generics.rs instantiate_type_with_alias_completion (~2155), map_parameter_completion (~2358) | Containment prefix, active mapper search/cache, recursive mapper composition, parameter scans and instantiated argument vectors. Native depth100/count5,000,000 gates retain exact order and reasons; product limits are independent. Composite/Merged mapper chains can recurse independently of source AST nesting. |
| conditional.rs conditional_type_completion loop (~642), conditional_instantiation_completion | Tail transitions and distribution loops can iterate after cached type-node reads; charge each transition/constituent. Tail1000 remains native ConditionalTailLimit, not product depth. Retain normal frame/conditional-depth cleanup for ordinary Results; product abort abandons the whole evaluator. |
| model.rs union_with_reductions (~2900), insert_union_type, compare_types/compare_type_lists (~3164/3433) | Work-stack flattening, binary ordered insertion, nested constituent comparison, constrained-variable scans, named-origin membership checks and Vec movement. The comparator's recursive type-list descent and exact literal comparisons require accounting even with no evaluator demand. Keep the current semantic order and identity observation owner. |
| model.rs maybe_type_of_kind (~3893), filter_type/TypeFilter, propagating_flags | Nested composite predicates and full constituent walks can be pure model work. TypeFilter owns retained/origin-filter scratch allocations; its plain accept/finish API needs controlled growth internally, without asking every caller to precharge. |
| evaluation/intersections.rs intersection_type_ex_completion; unions.rs union factory/reduction loops | Flattening, ordered set insertion, nested subtype/literal scans and cross-product enumeration. Native >=100,000 product and subtype comparison/estimate rules remain compiler outcomes; charge product work before each enumeration step and scratch capacity before materialization. |
| templates.rs template_literal_type_completion; model/templates.rs and model/literals.rs | Union distribution, explicit nested-template stack, exact-byte concatenation/casing/surrogate normalization, delimiter search and symbolic part walks. A single work tick before a large byte scan does not bound it. Reuse exact existing string owners; never duplicate decoding or approximate opaque identity spelling for accounting. |
| relations.rs related_in_session/recursive_relation/structured_relation and pairwise loops | Relation cache lookup/commit, maybe-key and source/target stacks, union alternatives, property/signature/index pairs, variance marker comparisons. Preserve native Maybe/overflow and active-key publication. Charge an existing pair/step, not just the first relation invocation. |
| inference.rs infer_from_types, infer_types_once, inference candidates/contexts | Mutual recursion, candidate insertion/scanning, constituent matching, object/signature inference, priority and visited-state stacks. Ordinary Result failures retain priority/expansion/context/mapper cleanup; product abort discards the complete request state and is never an inference rejection. |
| reads.rs read_flow_inner_completion (~437), narrow_assertion_completion (~1061) | These already use explicit work stacks, but their ReadWork/AssertionWork pushes and visit loop have no Control. Native logical flow depth2000 stays an errorType producer; broad flat flow uses work/bytes rather than Rust-depth units. |
| property_symbols.rs PropertySymbols/created symbol factories; model.rs signature (~2524); evaluation/signatures.rs | Symbol arena/declaration/origin vectors, signature parameter/type-parameter payloads, lazy completion/cache attachments. Allocation identity/order is native semantic state: reserve before publishing, and never issue a placeholder identity after a denied allocation. |
| symbol_tables.rs, module_exports.rs, alias and accessibility caches | Request-local maps/tables and traversal scratch can grow without model types. Retained immutable input bytes are not newly allocated; copied vectors/maps are. The shared module traversal must remain one decision owner; charging belongs in its actual iteration/client allocation points. |
| native_display and presentation graph/output construction | Diagnostic rendering can demand semantics and allocate strings; product graph budgets constrain output records but do not bound prior semantic work or all scratch. Their work uses the same request control while retaining their separate native/display limit policy. |

The initial Source/parser checkpoint is also not a claim that every Source recipe
walk is controlled. Current Source::syntax and several scanner/JSDoc/attachment
paths charge work, while many captured-AST recipe loops remain ordinary Vec,
String and traversal operations. Extend those same syntax owners as their
unbounded units are reached; do not add evaluator-side guesses about their cost.

## Allocation and latency contracts that must remain explicit

The current byte ledger counts cumulative requested payload capacity, including
replacement capacity, not allocator overhead or peak RSS. Hash table bucket
rounding/control bytes and allocator fragmentation are outside that unit. Do not
change this silently while adding model records; freeze/document units before
using calibrated constants.

Charging only an arena record undercounts separately allocated Vec/String/Arc
payloads. Existing `clone`, collect, with_capacity, Arc::new/from, Box::new and
format paths need an ownership audit. Cheap Arc clones do not allocate payload;
deep Vec/String/DemandError clones can. Reserve/charge before an untrusted-sized
copy, preserving one accounting owner to avoid accidentally double charging the
same reused allocation. Cumulative replacement allocations are intentionally
charged even if old storage is later freed.

Stable infallible Arc/Box construction is not made allocator-fallible merely by
calling Control::allocation first. Resolve unbounded payload construction at its
existing storage owner (fallible backing construction or a bounded measured
allocation unit); do not promise that catch_unwind catches allocation aborts.
Fixed startup/error-reporting allocations also need an explicit bounded policy.
The 256 MiB parsing/semantic worker stack is a current configuration, not proof
of headroom for arbitrary checker recursion or destructor chains.

Long exact-byte hashing/equality, hash-table rehash, Vec insertion/movement,
sorting comparisons, numeric parsing/formatting and deallocation are indivisible
standard-library work unless split. A before/after deadline check does not prove
a maximum unchecked unit. Measure their allowed size under calibrated budgets;
split a unit that can violate the liveness envelope. Presentation omission or
cache eviction cannot retroactively refund already performed work.

The worker cannot abandon its scoped thread. Stop must unwind through RAII
cleanup and drop request-local state; teardown and join are part of the measured
bound. Model TypeId/MapperId links already avoid recursively owning most
semantic graphs, but nested recipes/aliases/error payloads and large collection
destruction still need measured headroom and latency. No checks that request new
budget are allowed in cleanup or Drop.

## Failure and publication invariants

- Keep Stop::{Work,Memory,Depth,Deadline} distinct through the existing
  semantic::resource_failure conversion. They remain whole-operation failures
  with no fabricated source location, native errorType, Q3 gap, or Q4 partial node.
- NativeCompletion::recovered already rejects whole-operation failures, and
  append/sequence semantics make them outrank local recovery. Preserve that
  boundary; do not catch Stop to try a later overload/branch or cache a clean hit.
- The ledger's first Stop is sticky and speculative work is not refunded.
  Cache hits consume actual lookup/replay work but do not reallocate/recharge the
  original arena payload. An interrupted child cannot install a new successful
  cache entry; an earlier genuine native publication is retained according to
  that owner's existing rule, while the whole request fails.
- Recursion guards live through the real recursive work and drop on normal
  return or abort. Manual/native cleanup still precedes ordinary Result return;
  product abort relies only on RAII and whole-state discard. They must not borrow
  &mut Evaluator across a recursive call; Control's same-thread shared guard already solves this.
- Charging/reserving before mutation must not add new native observations or
  reorder symbol/type creation. Resource units are product mechanics, never a
  second source of compiler limits or ordering ranks.

## Final-query proof matrix

Use the existing private injected worker Limits and public query results in
semantic/resources_tests.rs. Do not invent public tuning options or validate
only a counter/helper. For every newly covered family:

1. Capture pinned native final answers for the healthy fixture and ensure the
   query reaches the intended semantic operation. Isolated compiler-limit
   fixtures must still reach their native error reason under production limits.
2. Put a cheap and expensive semantic demand in the same source/project so
   Source parsing/capture costs are shared in shape. Use already captured syntax
   with different semantic fanout/depth (generic mapping, conditional distribution,
   union/relation pairs, inference, template concatenation and wide flow).
   A budget that admits the cheap query must reject the semantic-heavy query;
   omission of the new accounting must fail this discriminator.
3. Measure one complete successful request, then assert just below, at and above
   its exact work, requested-payload and checked-depth usage. Assert final healthy
   type/graph or the exact whole resource kind, not an internal intermediate.
   Wide flat lists must not acquire checked depth proportional to their length.
4. Include warm-cache acquisitions inside one request, not merely repeated
   Project queries (which create fresh checkers). Confirm reuse does not recharge
   original payload while repeated lookup/continuation work remains bounded.
5. Force interruption after native state publication and during a measured
   growth/loop boundary. Check no fallback answer, no partial Q3/Q4 success, no
   replaced failure category, and healthy subsequent semantic and lexical
   requests over the immutable Project.
6. Exercise elapsed deadline during actual semantic work as well as duration0
   at entry. Freeze/check the largest unchecked unit and measured scheduling
   interval; tests of duration0 alone do not prove mid-loop cooperation.
7. Prove transport at public boundaries: each Stop maps to its exact resource
   kind, ordinary injected panic remains Internal, and speculative overload/
   branch work cannot recover from the private abort. In an isolated test
   process, install a panic hook and verify typed abort does not invoke it;
   ordinary injected panic retains the existing hook/Internal behavior. Acquire
   a native recovery first, then exhaust resources and require the whole resource
   error with no prior native causes, partial graph or Q3 gap. Interrupt after a native
   provisional publication and while QueryBindings has an active borrow; then
   query the same immutable Project again. Include unchanged Project::load
   outcomes when shared initialization is enabled with no request Control.
8. Measure peak memory/stack, failed and successful teardown, thread join, and a
   lexical request queued behind the worst admitted semantic request. The total
   must satisfy deadline plus baseline lexical/load/delivery and measured slack.

Suggested staging order is constructor/arena + common scheduler first; model
normalization/mapper/conditional loops next; relation/inference/flow and property
inventories next; diagnostic/presentation and residual Source/string units next;
then measured calibration and full corpus/service proof. Each checkpoint records
its remaining uncharged families. No finite deterministic resource maximum is
chosen in this audit: that requires the representative native boundary corpus and
real-project measurements mandated by the design.

## First storage and transport increment (staged)

The same request Control now reaches Model before intrinsic initialization and
Evaluator alongside Source. A private resources::RequestAbort payload carries
only the selected Stop to the existing worker catch; resume_unwind bypasses the
process panic hook, while an unrelated panic still maps to Internal.

Model::push reserves its four aligned vectors before changing lengths. All
mapper factories and startup mapper entries use one push_mapper owner. All
five demand factories use register_demand; the three property entry factories
use register_property_entry. Signature and created-property Arc factories charge
their fixed payloads before allocation; their Vec/String contents were already
prepared and remain separate work. Selected demand/property lookup maps reserve
before insertion. Demand, value-type, type-node, and expression invocations
charge balanced guards before their early returns and cache hits.

This is an incomplete accounting increment. It does not yet cover all model
interners, payload copies, identity-order maps, member/request caches, direct
recursive checker/model operations, or inner loops. A no-source absent-name
query isolates startup accounting at the public result boundary. An isolated
child-process test checks all four typed interruptions without panic-hook calls
and verifies the ordinary-panic policy. These tests and the prior final-query
exact-boundary tests await the next coordinated Rust checkpoint.


## Pure union normalization checkpoint

The next staged increment extends accounting through the existing pure model
union owner: flattening, native adjacent-identity suppression, ordered insertion,
recursive type/mapper comparisons, reductions and named-origin scans, scratch
copies and growth, and union key/payload/interner storage. Resuming a factory
charges its repeated pure work; NativeSequence still retains semantic acquisitions
once. A resource abort bypasses UnionFailure's dependency retry protocol.

The model factory now borrows its input identity list. The evaluator previously
cloned it on every resumption, and the model immediately cloned it again to keep
original inputs for the named-origin pass. Both passes can borrow the same
immutable input; their separate traversal stacks remain locally owned and
charged. A union key's freshly copied immutable constituent array is reused for
the type payload when it is exactly the same list. Native TypeId allocation,
ordering, alias/origin decisions and acquisition order are unchanged.

Alternative: guard only the evaluator union entry. That misses repeated visits
to cached types inside one invocation and still permits an unchecked successful
number/any/existing-union result. A new normalization scheduler or memoized
comparison layer would change control flow and identity observation unnecessarily.
The existing operation and allocation owners are sufficient.

Native evidence is `vrun-0e8a9414-7eac-407a-bf7d-806d5b765670`: sixteen cold
hook/API queries across strict and noCheck. Same-file grouped and alternating
alias lists have identical multiplicities but flatten 32/128 or 512/16,384
cached numeric leaves; all final types are number. Ascending, descending and
adjacent-duplicate lists preserve the full native 0..15 order. The product test
uses those exact inputs and checks work/bytes/depth boundaries, a grouped versus
alternating work-budget discriminator, flat depth and healthy subsequent requests.
Rust validation and an accounting-disabled counterfactual remain pending.

This increment does not complete long literal-byte/string-symbol comparisons,
wide key hashing/rehashing or movement latency, TypeFilter, mapper evaluation,
relation/inference loops, presentation, or the other resource audit owners.
Before/after work units bound the logical cost of an indivisible slice/Arc copy
or Vec movement; they do not prove its maximum unchecked wall-clock duration.
The existing production calibration and teardown gates remain open.

The union slice also leaves deep alias display/provenance copies and completion/
error cache clones to their payload owners. Control::work accepts collection
sizes directly and owns the checked usize-to-u64 conversion; the previous
parser movement cast now uses that same owner. Arithmetic overflow remains a
Work interruption with sticky first-failure semantics.


Accounting-disabled final-query RED is established by
`vrun-4504b322-55f3-4caf-b5e8-daa28fdb99e1`: grouped and alternating small
controls both consume exactly 90,418 work and 1,287,192 bytes at depth 10,
failing the work discriminator. The experiment restores only the previous
Model union/worker source, retaining earlier resource owners and the new test.
Reviewed union source is restored; its GREEN validation remains pending.


Restored-source resource GREEN is `vrun-5ed2d043-c541-4616-861d-741a7f13562d`:
all nine resource library tests pass, including the new exact boundary and
cached-union discriminator. The same full run reports 140 library passes, one
ignored, and only the preexisting JSON initialization probe group failure.
All targets compile. This closes the bounded union increment, not the remaining
payload, algorithm, calibration, teardown or corpus obligations above.


## Cached filtering increment (under implementation)

`TypeFilter` is the existing shared owner of native `filterType` for pure model
predicates and semantic completion predicates. Its controller spans both the
normalized-constituent pass and, only after a change, the denormalized-origin
pass. Its retained scratch vectors currently grow without the request ledger.
An unchanged wide union can execute repeated full filters without creating a
new type, bypassing arena-based accounting.

Extend this controller with the same optional Control and a guard acquired
before begin's shortcuts or singleton allocation. Keep the guard through the
predicate callbacks and finish reconstruction; field destruction balances it
on normal return, ordinary predicate failure and whole-request resource abort.
Charge each visited normalized/origin entry and check vector growth before
retaining it. The origin pass's nested union entries bypass the predicate, so
the controller must charge and store them through the same retained-entry
operation. Existing Arc clones share already-owned payload; only a new singleton
constituent slice is newly charged. Loading still supplies no request ledger.

The private finish operation currently scans the whole retained and original
lists for equality. Both actual controller clients pass the exact next identity
back to accept, preserving an ordered subset. Equal lengths therefore establish
an unchanged result without repeating that scan. Keep every native origin
choice: unchanged normalized list skips origins, nested origin unions are
retained without a predicate, and removal counts decide reuse/rebuild/discard.
Existing `type_filter_origins` tests cover these final projection outcomes.

Alternatives considered: charging each caller would duplicate traversal and
leave the direct nested-origin retention unprotected; a separate filtering
scheduler adds no ownership benefit. Instrumenting the existing controller
keeps its interface and the native predicate acquisition order unchanged.
Bounded adversarial plan review passes these decisions.

Native strict/noCheck evidence `vrun-bc3c3e81-d0ac-4e01-be9d-5fe2bb45a121`
passes ten cold hook/API queries with no diagnostics, errorType acquisitions,
skips or instantiations. Each same-file pair first acquires the same 16- or
256-member numeric union and scalar, then repeats eight scalar or union non-null
assertions before returning number. The resource test compares the paired
work/storage deltas across widths, checks exact under/at/over limits, flat depth,
and healthy subsequent semantic/lexical queries. Its pre-change RED is running.

This increment does not account for the separate `mapType` pass performed after
non-null filtering, nor prove `maybeTypeOfKind` recursive accounting. Those
owners, the previously listed algorithms/payloads and final calibration remain
required; this is not full resource safety.


Cached-filter RED is confirmed by
`vrun-5ecd0de8-bfa2-4648-a296-2c72082d8831`: increasing the cached union from
16 to 256 members leaves the heavy-minus-cheap work delta exactly 6. Large
heavy/cheap usage is respectively 48,462/48,456 work and 774,472/774,304 bytes,
both at depth 12. The new final-query growth assertion fails without the
filter accounting. All targets compile; 140 other library tests pass, one is
ignored, and the known 18-of-926 JSON group remains the only other failure.
Integration tests stop after the library failures. The TypeFilter implementation
is ready for the next source checkpoint; no accounting-disabled code is installed.


## Cached mapping follow-up (native baseline, not implemented)

`mapTypeEx` and the distinct alias-present branch of `mapTypeWithAlias` own
constituent traversal and temporary mapped lists. They currently allocate those
lists directly and can return an unchanged cached type after unchecked work.
Guard the worker before Never/scalar shortcuts; keep that guard across callbacks,
origin recursion and final union reconstruction. Charge each actual visited
entry before invoking its callback and reserve the scratch vector with the
existing resource owner before allocation. The alias branch requires its own
entry/iteration/storage checks because native maps normalized constituents
there, without the ordinary origin substitution. Neither branch changes native
nil, unchanged-identity, no-reduction or alias rules.

Do not add a separate traversal controller: unlike filtering, mapping has only
one evaluator callback family, and the recursion and reconstructed results
already belong together. Model/resource helpers remain the accounting owners;
callers do not need to predict constituent counts or pass a budget separately.
The existing Arc constituent views share payload and are not charged as copies.

Native `vrun-8611a752-dc6b-48b1-9c4f-b39656e3da32` passes ten cold hook/API
queries in `native-map-resources`. Both requests warm the same union and scalar,
then differ only in scalar versus cached-union values in eight object literal
properties. Mutable-location widening invokes two unchanged map operations per
union initializer. Final number, no diagnostics/errors/instantiations, and
strict/noCheck are pinned. Reuse the final-query paired resource discriminator
and exact boundary test shape; establish its RED before accounting is enabled.
This follow-up is still a plan. It does not close other object-literal work,
mapper instantiation, maybeTypeOfKind recursion, cache payloads or calibration.


Bounded adversarial mapping-plan review passes after confirming that only the
alias-present branch adds its own guard; its ordinary delegate is already
guarded by the worker. Every entry is charged even for a nil callback or a nested
origin result. Checked initial reservation covers the known maximum of one
retained TypeId per entry, so no second traversal or accounting controller is
needed. Native `mapTypeWithAlias` retains direct normalized-child callbacks and
unconditional aliased reconstruction, independent of the ordinary origin and
unchanged-identity rules. Production and final resource RED remain pending.


## Witness correction before acceptance

The first filter and map fixtures used void-wrapped comma operands. Pinned
checkVoidExpression schedules those operands through checkNodeDeferred, so cold
type queries returned number without executing the intended work. Library
`vrun-0912278e-014b-4fb9-a51e-234f45dfd512` confirms both work deltas remained
six despite installed filter accounting. These are fixture failures, not valid
accounting-omission evidence. Earlier RED and native traversal claims above are
superseded; originals are preserved under .cc/temp/stage67-deferred-void-*.
Production Void behavior remains correct, independently pinned by native12
`vrun-92ef24e8-9d32-4f04-a6f6-7712d8729f71`.

Corrected eager comma operands retain native diagnostic-only TS2695 results.
The filter gate now checks exactly sixteen wide unchanged filter invocations,
from eight flow-parent checks plus eight assertion transformations, and no wide
filter in the cheap twin. Native10 strict/noCheck PASS is
`vrun-e8372beb-7f53-4d68-8147-d25b8ea3af12`. The map gate asserts all eight
object-literal checkExpressionEx exits and native10 passes in
`vrun-defc4c98-8d98-45b7-97d2-44b503dcf6bb` (the map half of that run).
A scratch accounting patch is ready; production mapping is unchanged until the
corrected final-query discriminator has its RED. Filter's omission counterfactual
must also be rerun on these corrected inputs before claiming its resource GREEN.


## Isolated cached-filter proof and recursive kind predicate (2026-10-12)

Independent bounded snapshot review is
`memory-bank/agent-runs/next-evaluator-resource-increment-review-2026-10-12.md`.
It identifies a real witness defect: installed `mapTypeEx` charges can satisfy
both growth assertions in the non-null filter fixture. Removing only all
TypeFilter accounting passes library `vrun-e6968de9-e741-4abb-92aa-3e191cec099d`.
The non-null fixture remains combined filter/map compatibility coverage.

The new `native-filter-only-resources` fixture uses eight eager inequality
branches with a read in the true arm. Its numeric `NOT_UNDEFINED` result has no
undefined fact, so nullable replacement returns before mapping. This also
avoids the `NOT_NULLABLE`/`TRUTHY` mapping continuation; unchanged constituents
remain the original union and the final type is number. Pinned strict/noCheck
native/API controls pass `vrun-b43f85b1-c54a-4852-b346-c46ab9d8d954`, asserting
eight actual wide unchanged filters, eighteen diagnostic-only TS2695 codes and
no errorType acquisitions or native instantiations. With only TypeFilter
accounting removed, its final-query test fails in
`vrun-0c6dc982-1540-452a-9362-7129d340844b`: heavy-minus-cheap work remains
168 for both widths. Old non-null and mapping tests still pass. The unrelated
10,000-config loading guard also times out in that run; it is not resource RED.
Separate work-only and scratch-only omission proof, memory discriminators and
restored-source GREEN are still required.

`Model::maybe_type_of_kind` is the sole `maybeTypeOfKind` owner. Native applies
actual flags first, then short-circuits through actual union/intersection
constituents; type-parameter constraints and union origins are different
questions. Install the existing optional request depth guard before that first
flag check, so each visited invocation receives one work unit and balanced depth
without precharging unvisited tails. The shared Arc views allocate no new
constituent payload and need no scratch charge. Loading supplies no Control.
No fallible public predicate interface or caller precharge is needed.

The `native-kind-resources` witness warms early-symbol and late-branded-symbol
unions identically, then performs a symbol-disallowed relational expression in
an eager comma initializer ending in number. Native symbol rejection emits
TS2469 and skips later relational filtering/mapping. Direct symbol sorts before
numeric literal flags; the branded intersection sorts after them. Strict/noCheck
native/API gate `vrun-8875a8f2-ed5a-48d5-85e0-59237c46b753` passes sixteen pairs:
early union visits two predicate invocations regardless of width, late visits
20/260 with three actual recursive levels, the numeric miss is replayed through
the separate constraint wrapper before the right symbol short-circuits, and a
direct symbol needs one invocation. Final results are number with no errorType
acquisitions or instantiations. An opt-in zero-demand overlay records only actual
recursive entries, existing flags/IDs, visited children and returned booleans.
Final-query work RED/GREEN and an owner-specific depth discriminator remain
required; compiler nesting alone does not prove this predicate controls the
whole request's peak. Whole-evaluator accounting and calibration remain open.


The restored-source kind and filter tests pass in library
`vrun-18c771e6-9330-486d-b9fb-68b202ee3021` (all thirteen resource tests pass;
the unchanged loading timing guard fails separately). Independent filter
traversal/retention/completion work omission preserves the entry guard and
checked scratch, and fails its work-growth assertion in
`vrun-03ea5ffc-a93e-4650-8ff9-1245893f1d44`. Independent scratch omission
preserves work and fails its byte-growth assertion in
`vrun-c5c54b5e-b4d2-4e6d-9734-3ec681afb732`. The source is restored after each
experiment. The paired byte cutoff now complements the work cutoff in the
shared public-result helper. Combined final cold native gate
`vrun-d8f54d77-4a1a-46ec-8a04-883a87a036ff` passes four tests and 26 pairs
without skips. Independent frozen-source review
`memory-bank/agent-runs/final-evaluator-resource-increment-review-2026-10-12.md`
recommends retaining the guard/controller. The source acceptance is bounded:
this does not prove origin-pass resource coverage, predicate-specific whole
request depth dominance, shortcut-only debit mutation, production constants,
teardown or whole-evaluator completeness.

## Flow continuation stack and read invocation storage (2026-10-12)

`getTypeAtFlowNode`'s explicit continuation stack in `evaluation/reads.rs` now
runs through `ReadStack`, an aborting wrapper over the new generic
`resources::WorkStack`: every popped task (each native visit, tail hop and
continuation) is one work unit, and every push reserves charged capacity before
growth. `ReadInvocations` charges frame, Shared-result and reduce-label growth
before mutation through the same request Control, and branch, bypass and loop
`types` scratch is pushed through `resources::push`. A denied charge abandons
the whole request through the existing typed abort; native traversal order,
Shared publication, reduce-label pairing and the native 2000 invocation-depth
error are unchanged, and the explicit stack spends no checked depth.

The `native-flow-resources` witness reads the same `let value: number = 0`
before (cheap) and after (heavy) w copies of `if (condition) {}` at widths 16
and 256. The declared non-union type keeps the flow type equal to the declared
type, so this checker's read admission accepts it (a union-narrowing variant is
refused as `FlowRequired`, flow narrowing being outside T). An opt-in
zero-demand overlay on the pinned Go `getTypeAtFlowNode`
(`inspectFlowNodes` in `tools/type-oracle.mjs`) counts loop visits, Shared hits
and records, entries and maximum depth per `getFlowTypeOfReferenceEx`
invocation. Strict and noCheck controls pass in
`vrun-3f054761-3a59-432b-b4a0-44122646e61e` (two tests, zero skips): every final
type is number with no diagnostics; cheap reads visit one node; heavy reads make
exactly 4w+1 entries, 4w+2 visits, native depth 2w+1, w Shared hits and 3w
Shared records.

Before this change the final-query test failed only on payload growth: heavy
minus cheap requested bytes were 0 at both widths. The registered storage
omission (stack, frame, Shared, reduce-label and `types` charges removed, pop
work kept) fails exactly that assertion in
`vrun-a90f82da-2d70-43de-9258-e3d3fadeab71`; the other 145 library tests pass.
The restored source passes the full library root, including
`request_resource_flow_work_stack_width_stays_controlled` with exact
under/at/over work, payload and depth boundaries, flat checked depth and
healthy successors.

Limitation: the work dimension is covered but not owner-isolated by this
witness. Condition narrowing already charges about twenty units per branch, so
heavy-minus-cheap work grew (326 to 5,126 at widths 16 to 256, local
diagnostic) before the pop charge, which adds 8w+1 units. Reachability,
assignment-destructuring and assertion work stacks, loop caches
(`reads.loops`), and Source/syntax costs remain uncharged by this increment.
