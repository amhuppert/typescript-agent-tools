# T/Q3/Q4 implementation: adversarial plan audit

2026-10-10. Read-only audit of implementation base `ea23d8bbbb34d5609aa9ba7057b479805c61ca7b`
and the current [type design](types-design.md), [compiler evidence](types-design-evidence.md),
and both [review responses](types-review-response.md) [round two](types-review-response-2.md).
This audit proposes implementation checks; it does not certify checker behavior or
replace the selected T/Q3/Q4 scope.

Applied the `posd:ousterhout-software-design` skill and its reviewing route.
Operating envelope: the existing native Rust CLI/library, immutable disk-derived
Project snapshots, request-local semantic state, and one serialized local Unix
service. The user authorized implementation. No production compiler dependency,
durable semantic database, concurrent checker, F implementation, or additional
ticket capability is assumed.

## Findings, in priority order

### P0 — The proposed semantic baseline still needs an executable oracle

**Evidence.** [types-design.md](types-design.md#evaluation-order-baseline) explicitly
requires one fresh semantic request, outermost-first call resolution, seed and
directed-search establishment, and candidate resolution in exact file/span/role
order. The retained evidence covers thirteen contextual-call sessions, not that
whole reference schedule. `tools/semantic-oracle.mjs` deliberately classifies
property/method symbols as unsupported and its `SemanticSession.collect` queries
all searched tokens in one snapshot before requesting optional diagnostics. It
cannot validate the new baseline merely by adding members to `MEMBER_FLAGS`.

The pinned API exposes `getResolvedSignature`, `getReferencedSymbolsForNode`,
`getPropertiesOfType`, and public type operations in
`dist/api/sync/api.d.ts`; it does not expose `GetRootSymbols`, combined late-member
resolution, flow-node dispatch, relation Maybe/overflow state, or instantiation
counter reset instrumentation. Those missing observations matter because the
design intentionally normalizes cold late-member answers and rejects particular
recovery paths that can print ordinary-looking types.

**Consequence.** A native implementation could agree with a convenient warmed
language-server history while disagreeing with the promised fresh-query semantics.
Root-origin matching alone could also conceal an incorrect directed search set.
Compiler-limit tests could silently test a product budget instead of a compiler
outcome. This is a correctness gate, not a preference about harness organization.

**Remedy.** Complete checkpoint 1 before relying on the semantic contract:

1. Give member/type requests their own oracle entry, retaining the lexical oracle
   and its intentional product differences unchanged.
2. Open/dispose a fresh checker for each semantic request. Retain the exact
   operation trace, raw answer, and explicit normalization records together.
3. Build the Q3 candidate schedule independently from pinned compiler syntax and
   meaning operations, with every eligible candidate represented. Do not derive
   completeness or expected gaps from Rust's supported cases.
4. Use a pinned test-only compiler hook where public APIs cannot observe combined
   roots, recovery causes, operation counters, or reset histories. Version/source
   hashes and final query outcomes must be assertions. Keep hooks outside the
   production runtime.
5. Add non-call contextual forms, recursive aliases, reversed/singleton requests,
   and language-limit final outcomes to the retained baseline. A missing hook or
   unexplained mismatch is a failed checkpoint, not a skipped success.

The design already demands this gate. The implementation plan should name its
artifact and command explicitly so later checker tests cannot bypass it.
This pulls checking history downward into one oracle/evaluation owner (P10/P13).

### P1 — Configuration's existing behavior contradicts a stated design premise

**Evidence.** The design's compiler-option section says to preserve the config
owner's rejection of disabled `esModuleInterop` and
`allowSyntheticDefaultImports`. At the audited base,
`src/project/program/config.rs::convert` reads neither option. The test
`supported_combinations_and_irrelevant_options_load` in
`src/project/program/tests.rs` deliberately loads a configuration containing
`esModuleInterop: false` alongside ignored options. The pinned
`internal/compiler/program.go` rejects false `alwaysStrict`, `esModuleInterop`,
and `allowSyntheticDefaultImports` through `createRemovedOptionDiagnostic`.
`core/compileroptions.go::GetStrictOptionValue` returns true for an unset strict
family option unless `strict` is explicitly false.

Configuration has strong existing conversion/inheritance ownership in
`Reader`, `Chain::option`, `own_setting`, and `Kind`. However, `Options` retains
only loading/binding inputs, and the returned `Program` retains only
`preserve_const_enums` from the effective settings. Effective semantic options
and resolver per-file format therefore disappear before Project queries.

**Consequence.** Assuming these rejections already exist leaves removed false
modes accepted and risks different answers after reparsing configuration in the
checker. Treating unset strict as false would change nullability, contextual
typing, and several intrinsic hooks at once.

**Remedy.** Correct the design's starting-state wording and extend the existing
configuration owner. Freeze a classification for every pinned accepted option:
semantic, diagnostic-only, emit-only, or demanded deferred semantics. Preserve
typed effective semantic settings and per-file module facts in the snapshot;
consumers must not redetect them. Replace the obsolete ignored-option assertion
with final `Project::load` tests for pinned rejected values. Include inherited
false/true/null/unset overrides, repeated keys, and malformed occurrences that
lose inheritance precedence. An option being irrelevant to these queries does
not justify skipping the pinned validation of its value.

This is one-owner repair (P08/P10), not a separate option parser. Tightening an
old intentionally ignored option is a deliberate compatibility correction that
should be recorded in the user documentation.

### P1 — The binding migration must freeze the existing owner, not build another binder

**Evidence.** `Project::load_here` receives only `binding::Index`. `Linker::link`
creates and freezes Store tables but then returns `resolve_and_index()`;
canonical symbols, scope tables, declaration-to-symbol mappings, alias states,
resolved references, module exports, and augmentation decisions are discarded.
`Index::Target` is a deliberately smaller result containing lexical symbols,
NotReference, or Unresolved. `collect::Facts` has scope-aware lexical bindings;
the Q2 catalog intentionally omits parameter/type/heritage syntax and retains
arena-local `NodeId`s only for transient reprojection.

`Linker::value_property` returns `Option<Resolution>` for its narrow JSON/library
question. `None` can mean no directly known JSON key, no available apparent
interface fact, or no present member; inherited possibilities are deliberately
represented separately. `JsonModule::new` discards array children and all earlier
duplicate-key values/spans while preserving the first key position and last value.

**Consequence.** Reconstructing semantics from Q2 signatures, spelling, or a new
scope walk would duplicate declaration merging, aliases, namespace meanings,
and module/global augmentation. Reinterpreting existing `None`, NotReference,
and unresolved paths as tolerant semantic Unsupported could change old lexical
answers and turn a hard load failure into an apparent successful query.

**Remedy.** Make the existing linker publish an immutable BoundProgram and
project the old Index once from those canonical identities. Before editing it,
write a migration table for each old projection: symbols by meaning; merged
declaration origins; aliases/type-only routes; lexical uses; narrow value paths;
NotReference; unresolved; tolerated unsupported dependencies; hard failures.
Test the final old Project outcomes for each row. Semantic source attaches frozen
site/role handles to reprojection of captured text; it never binds names again.
Extend the shared JSON facts with recursive array children, every exact key span,
and last-value links, preserving old narrow property lookup as a separate named
operation with present/absent/deferred/failure distinctions.

No permanent source substring copies per term, exposed binder tables, universal
checker context, or forwarding trait per design row is needed. Measure both
representations before choosing eager terms. Cohesive algorithms and a small
complete Project operation are justified; arbitrary file splitting is not a
substitute for ownership (P04/P09/P10).

### P1 — Three different cycle protocols must not collapse into one evaluator state

**Evidence.** The design names type-resolution target/property cycle marking,
resolving signatures, early late-bound members, and recursive relation
True/False/Maybe as separate protocols. Current lexical `Linker::force` correctly
has its own alias-cycle stack and fill-edge cuts; its general resolution shape
does not define semantic recursive type behavior.

**Consequence.** A shared `Evaluating => circular_type` rule would reject valid
recursive aliases and member re-entry. Returning true from every active relation
pair would instead accept invalid assignments or overloads. Candidate-local
rollback would erase callback parameter types established by rejected overloads.

**Remedy.** Keep each protocol inside its complete operation, with typed outcomes
and operation-specific cache keys. One shared request work/budget ledger is
appropriate; one shared semantic cycle meaning is not. Tests must observe final
Project types/member groups for valid recursion and explicit final recovery
errors for invalid cycles, including a call whose rejected candidate supplies
the winning candidate's callback context. Apply source/argument order and
counter reset rules before memo reuse. This hides the compiler's differing
questions instead of imposing them on Q3 and the presenter (P10/P11).

### P1 — Useful positive journeys must be checked before broad refusal coverage can count as delivery

**Evidence.** The profile requires inference, mapped/conditional operators,
generic factories, overloads, and flow infrastructure; it explicitly rejects an
annotations-only subset. The real-project acceptance requires successful
inspection of the annotated member and inferred factory journeys plus an
established cross-file member use. JSX and predicate-heavy cases may be partial,
but their refusal counts must be reported separately.

**Consequence.** It is easy to ship a Q4 schema that prints annotations and an
honest gap ledger that refuses most of the motivating program. That can be a
useful intermediate commit, but does not fulfill selected T/Q3/Q4. Source-backed
design facts also do not calibrate native request work, stack, or queue bounds.

**Remedy.** Keep the fixed Command Center journeys in the manifest from checkpoint
1 and run them at each core checkpoint. Preserve all selected operator/inference
families in the implementation coverage matrix. Mark incomplete slices as
incomplete rather than revising the design to authorize a smaller deliverable.
Record first/repeated latency, work, established/gap/error counts, output,
peak/retained memory, and old load/refresh cost. Budget constants must admit
isolated compiler-boundary fixtures and successful representative queries.
Deadline, work, memory, stack, teardown, and queued lexical checks close delivery;
increasing the client timeout does not address them (P02/P03/P16).

### P2 — Delivery currently infers result-versus-diagnostic from the exit code

**Evidence.** `src/cli/delivery.rs::try_deliver` sends every nonzero text outcome
to stderr and leaves `-o` untouched. `Delivery` documents zero as a result and
nonzero as a failure. Q4 partial text must instead be delivered with exit 1;
Q3 partial/unselected documents also have nonzero successful delivery.

**Consequence.** Wiring new result types without changing this decision would
turn `PARTIAL TYPE` into a diagnostic and violate the output-file contract.

**Remedy.** Represent result-versus-diagnostic explicitly at the existing delivery
owner; preserve transport/write failures as exit-2 stderr-only failures. Test
exact stdout/stderr/exit and preexisting `-o` contents through direct and service
paths. Do not copy semantic delivery into another command handler (P10/P13).

## Concrete behavior-level test slices

These slices are an implementation sequence, not a reduction of scope. Each
starts with a final Project/Session or CLI behavior assertion, run through the
registered validation command scoped to that single test file. Add only the
minimal compiling module skeleton before a test if otherwise it would fail from
an absent import. Expected language facts come from the pinned oracle; product
selection/filter/display facts come from the frozen contract.

| Slice | Cases that must travel together | Final assertions and negative control |
| --- | --- | --- |
| 1. Frozen baseline | Shared rejected-overload callback context; generic winning candidate; all-failed overload; nested call/new/tag preludes; object contextual methods outside calls; cold/reversed/singleton requests; combined computed-plus-static member roots. | Retained raw and normalized types/roots/groups plus operation trace. A deliberately subnode-first request differs where expected; diagnostics-first is not accepted as the prelude. Compiler/source/manifest mismatches and missing hooks fail. |
| 2. Options and captured inputs | Strict unset/true/false, each per-option override, inherited null/reset and repeated keys; removed false interop modes; exact optional and unchecked index reads; `BuiltinIteratorReturn`; diagnostic-only TS4111/skipLibCheck/noCheck. | `Project::load` rejects invalid configuration and final type results match pinned effective settings. Disk configuration changed after direct load cannot change that snapshot; Session refresh changes it. No checker-side config reader. |
| 3. BoundProgram/site roles | Same-name type/value, interface/class merge, overload family, namespace A.B.C, type-only and renamed import/export forms, parameter property, shorthand, augmentation, JSON/library paths. | Old lexical/Q1/Q2/E outcomes remain equal. New name-selection candidates collapse by symbol/question and every supplied anchor replays through the same operation's `At`. Node IDs from one arena cannot serve as frozen identity. |
| 4. Read admission and return inference | `const host: Host = createHost()`; widened boolean and annotated union initializer; guards through alias/destructuring; unrelated assignment; `typeof` annotation; defaults/constraints/optional-chain interior; annotated callable with unsupported body; explicit/trailing never call across function/class/object/arrow forms; switch exhaustiveness and infinite loops. | Positive final types stay useful; demands requiring refinement return located `flow_required`. Explicit annotations bypass body validation only where the compiler does. Null-sensitive outcomes run with strictNullChecks on/off. Refusal is not an unnarrowed successful type. |
| 5. Full type core | Generic factory inferred object methods; contextual callbacks; aliases of equal structure with different identities; recursive alias plus growing `Grow<T[]>`; mapped optional/readonly/key-remapping; distributive conditional/infer; `keyof`/indexed access; all string mappings, NoInfer, Awaited, BuiltinIteratorReturn; rest/optional/variadic tuples; unique symbols/enums and constant forms. | Final semantic type/graph facts, alias/type-argument/provenance/order preservation, and positive inferred-factory journey. No utility-name shortcut substitutes for operator semantics. Include ordinary any, unknown, unresolved/error recovery, and rejected final satisfies with a valid unchanged expression type. |
| 6. Calls/relations/re-entry | Rejected candidates sharing contextual node state, ordered inference, signatures with this/predicates/rest, variance/private/protected origins, active-pair Maybe, invalid recursive cycles, compiler instantiation/conditional/union/subtype/relation boundaries. | Final chosen signature/member type or exact semantic failure. Just-under/at/over language limits produce pinned operation outcomes; relation False can reject one candidate without poisoning a valid later result. Unsupported/resource outcomes never mean inapplicable. |
| 7. Member Q3 | Interface/annotated/satisfies/unrelated method direction; base/override/implements; Box<string>/Box<number> same roots; Partial/Readonly/Pick/Omit; ordinary versus special spreads; inline-literal union versus interface union; private/static/instance, accessors, computed origins; JSON repeated/escaped keys; genuine any, unknown and recovery receivers; JSX props gaps and ordinary member tags. | Exact directed groups and final declaration/use/reversed/cross-file equality under normalization. Every eligible filtered candidate is an established occurrence, proven non-reference, or located gap. No arbitrary host[key]/[key] wildcard candidate. Unrelated equal signatures never merge. |
| 8. Lexical call/test filters | Call/optional/new/tag, parentheses/non-null/assertion/as/satisfies/instantiation wrappers; explicit @factory(); ns.run; observe.call; alias read then alias call; JSX/decorator/instanceof; test directory/basename/case/slash variants. | Lexical refs remain binding/syntax-only. Member token, not receiver, gets direct call role; implicit uses and later value aliases do not. Filters preserve seed metadata, may remove filtered-only related groups, and do not remove type dependencies. |
| 9. Selection and public delivery | Every table anchor including class constructor side through aliases, enum declaration/member, namespace meanings, accessors, final type names, call/new/tag result punctuation, element literal versus ], JSON Q4 rejection, whitespace and unsupported catalog sites. | Final questions and replay anchors, typed not_found/ambiguous documents, member-only bare-name advisory, compatibility of legacy ReferenceQuery::Name. Partial text/JSON with exit 1 is delivered; analysis failures preserve -o; usage/transport/write errors never emit a second document. |
| 10. Q4 bounded graph | Primitive at depth 0; object at depth 0; shared child reached at unequal depths; alias/argument edges; distinct accessor writes; missing vs undefined; wide property/signature/union lists; exact long keys; Unicode summary tokens; recursive sharing versus unbounded generic growth; unsupported child and collection enumeration. | Depth/node/UTF-8 byte/summary boundaries just under/at/over; real root kinds, exact omitted counts and stable order, document-local references, minimum-depth expansion. Display elision remains found; unsupported children remain partial; invalid/resource/root failures remain errors. Pretty/text share one admitted graph. |
| 11. Snapshot/service containment | Fresh/repeated/reversed requests; source/config/dependency/library edits; failed refresh; semantic panic; work/memory/depth/deadline exhaustion; queued lexical request behind the worst semantic request; teardown and join. | Direct/service result equality, no stale fallback, following lexical query succeeds after an unwinding panic, old S/C/E journeys remain usable. Queue completion includes worker teardown/join and stays inside the semantic deadline plus measured lexical/load/delivery/slack bound. |

AGENTS.md's variants apply to the final outcomes above: BOM, CR/LF/CRLF/U+2028/
U+2029, whitespace classes where the pinned rule distinguishes them, exact versus
folded case, slash variants, decoded identifiers/numeric names, empty names,
unpaired surrogates, UTF-16 columns, ties, and all option states. For structural
limits also include a large shallow/flat input so speculative parser depth is
not mistaken for final-tree depth. Tests may share fixtures, but must not silently
discard a variant after it produces a different recovery or failure class.

Suggested test ownership is one integration file per cohesive slice, with
separate oracle/harness tests under `tools/`. Names are an implementation choice;
the checkpoint matrix should link each slice to its exact registered command and
retained result. Do not use a directory path in the TDD loop.

## Checkpoint 1 exit checklist

- A callable fresh-request member/type oracle plus test-only hooks for otherwise
  unobservable promises; version and source identity checks fail closed.
- Raw and normalized answer records with reasons; no expected-answer derivation
  from the new Rust output.
- Whole Q3 scheduling, non-call context, recursive/re-entry and compiler-limit
  reset fixtures, not only the already demonstrated contextual-call slice.
- Frozen selection/result/graph examples, rejection classes, overload pass order,
  and accepted-option classification. Correct the false-interop config premise.
- Hashed Command Center manifest at the required revision/config, with actual
  source anchors, dependency/library lock identity, generated-input inventory,
  and synthetic positive/unsupported controls. RunHost remains illustrative.
- Recorded known oracle limitations and prospective product normalizations in
  references.md. A limitation on a required promise blocks that promise's
  checkpoint until its oracle or design is corrected.

## Audit limits

This pass inspected live Rust ownership, the current design/evidence/responses,
retained probe descriptions, the pinned API declarations, and the pinned option
and checker source locations. It did not rerun compiler probes, implement a
semantic checker, execute Rust/harness tests, or measure representation/latency/
memory. New documentation has no runtime behavior for a red-green cycle.
Predicted implementation costs above are explicitly tied to contract and owner
evidence; they are not newly measured semantic counterexamples.
