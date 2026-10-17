# Next native increment: imported object value and members

This is a bounded implementation recommendation using the
`ousterhout-software-design` designing route. It preserves the complete selected
native Rust T/Q3/Q4 scope. It does not close the semantic baseline,
representation, execution or delivery gates, and it proposes no compiler
process in production.

## Decision and operating envelope

Implement the native value type of an ordinary import/export alias whose
resolved target is a variable initialized by an ordinary object literal. Follow
the existing canonical binding across files, construct actual inferred literal
and object types, and demand the object's member types through the same native
type operations. Compare eager capture and captured-source reprojection against
this one consumer. An import alias is the first final Project question because
the design explicitly asks its target's value type; it does not depend on
claiming arbitrary expression flow/admission complete.

The minimal useful case is:

```ts
// model.ts
export const current = { label: "ready", count: 1 };
// barrel.ts
export { current as chosen } from "./model";
// consumer.ts
import { chosen } from "./barrel";
```

Selecting the consumer alias asks `getTypeOfSymbol` of the existing resolved
value target. The answer has `label: string` and `count: number`, with property
origins in `model.ts`. A `const` variable does not make its ordinary object
properties readonly literals. A separate `const text = "ready"` control does
retain the literal variable type. The evaluator must establish these results
from source recipes and native type rules; navigation signatures and flattened
lexical reference groups cannot supply the type.

Each request describes one immutable Project generation, uses captured text and
options, performs no disk reads/rebinding, and owns its temporary description
and checker state. Existing hard load failures remain load failures. No unused
alias or dependency work is forced during load. Complete ProjectSession refresh,
worker/resource and broad semantic journeys remain delivery obligations.

## Sequencing: relevant proof first, complete proof before delivery

Two alternatives were considered:

| Alternative | Benefit | Cost and risk |
| --- | --- | --- |
| Finish every Q3/Q4 baseline before writing any native leaf | One large proof boundary. | Unrelated graph truncation, JSX selection and relationship cases block an independently specified value operation. It delays testing representation with a real consumer and allows oracle work to substitute for implementation progress. |
| Freeze each increment's relevant compiler expectations, implement behind the intended owners, retain the complete baseline gate before public semantic delivery | Produces native behavior and measures real dependencies early; failures and owner boundaries are exercised as they are built. | Requires precise partial status and prevents incidental leaf successes from becoming a conformance/resource claim. |

Choose the second. The first import-alias/object increment depends on effective
options, alias meaning, canonical declaration identity, final-tree source replay,
literal freshness/widening, ordinary object member creation and exact origins.
It does not depend on recursive graph budgets or contextual Q3 relationship
direction. Those unrelated obligations remain mandatory for delivery.

Add a deliberate sequencing clarification to the design: compiler-pinned native
increments may proceed alongside unfinished baseline extensions after their own
relevant expectations are frozen. Checkpoint 1 remains the full baseline gate;
checkpoint 2 remains the complete representation/migration/execution gate before
the full checker is claimed. No expectation is weakened to current Rust output.

## Existing and needed evidence

`tools/type-algebra-oracle.test.mjs` already pins an ordinary fresh object with
widened `string`/`number` members and the separate `as const` readonly literal
case. The current selection baseline pins alias question choice. The completed
binding-retention tests pin sparse canonical lookup, per-meaning/type-only
provenance, directed own versus projected origins, exact source attachments and
publication after all checks. They do not independently pin the three-file
native inference journey above.

Before native behavior tests, add a small hashed compiler fixture for the
cross-file cases and ask the fresh pinned type oracle at the alias anchor.
Assert the root question/type, member types, property order and exact declaration
origins, keeping formatting after the semantic demand. Pin direct import and
renamed-barrel routes to the same target separately. The dependency-origin case
must remain outside Searched while its type/member origins are included. Any
uncallable expectation or unexplained answer mismatch blocks that increment.

The relevant pinned source is TypeScript 7.0.2, commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`, under `internal/checker/checker.go`:

| Decision | Compiler owner to port |
| --- | --- |
| Variable/property dispatch and re-entry | `getTypeOfSymbol`, `getTypeOfVariableOrParameterOrProperty`, its worker and `pushTypeResolution`/`popTypeResolution` |
| Initializer type and const/let treatment | `getTypeForVariableLikeDeclaration`, `checkDeclarationInitializer`, `widenTypeInferredFromInitializer`, `getWidenedLiteralTypeForInitializer`, `widenTypeForVariableLikeDeclaration` |
| Fresh object and property creation | `checkObjectLiteral`, `checkPropertyAssignment` |
| Mutable property expression typing | `checkExpressionForMutableLocation`, `getWidenedLiteralLikeTypeForContextualType`; the no-context case must be deliberate, not a generic widening shortcut |
| Literal freshness versus regular/base type | `getFreshTypeOfLiteralType`, `getRegularTypeOfLiteralType`, `getWidenedLiteralType` |
| Object widening and retained member origins | `getWidenedType`, `getWidenedTypeOfObjectLiteral`, `getWidenedProperty`, `createSymbolWithType` |
| Property demand | `getPropertyOfType`, `getTypeOfPropertyOfType`, `getTypeOfSymbol` |

Search each owner name and existing copies before adding a rule. This table
identifies the real path, not a requirement to create one module per function.
Contextual typing, assertions, spreads, methods and computed keys require their
own pinned branches before admission. In particular, Q2's computed-name selector
policy in `navigation::names` is not the semantic member-key question.

## Abstraction before machinery

The evaluator consumes typed binding/source handles. It must not inspect
Store's parent arrays, string maps, declaration catalogs or parser layout.
Binding owns operations that answer the relevant meaning of an existing alias
or reference and identify the canonical target's own value declaration. Source
owns acquiring that declaration's context and initializer description.

Conceptually, the consumer is one operation:

```text
value_type(alias_seed, request_state) -> admitted native type or exact failure
member_type(native_type, exact_key, request_state) -> type plus member origins
```

The alias seed already contains its typed site/role/question. It is not a plain
name string or a public reference-result group. The binding answer preserves
type-only provenance and distinguishes NotDemanded, Unresolved, GlobalThis,
ValueTarget and Found. No pending demand is treated as a missing type or `any`.
This first family uses aliases the existing searched-load schedule already
settled and literal initializers needing no new lexical lookup. Extracting the
read-only pending-alias/name resolver later must preserve the original owners;
the discarded Linker task stack cannot simply be resumed.

Current Store own sites contain file/span/name, while declaration associations
retain `(FileId, DeclId)`. Select the canonical value declaration through those
existing associations and compiler value-declaration rules. Do not infer identity
from a display name, sort Q2 candidates to choose a declaration, or equate a
directed Index origin with Store's own declaration. A small derived declaration
access path may be necessary; it must remain binding-owned and must not become
a second lookup/canonicalization decision.

Semantic source returns owned compact descriptions with stable source-site and
role links. Final-tree preorder identity, named AST fields, exact payloads and
existing `(FileId, DeclId/RefId)` attachments are separate facts. Parser NodeId
and a naked span are insufficient for general identity: final module/await
reparses and dual roles can distinguish nodes sharing a physical span. The
increment pins exact variable/property replay now, then extends the same owner
for later role families. Object property recipes/origins do not create another
searched lexical token inventory.

The constructed object retains fresh/regular distinctions and ordered member
records. Literal types are not just summary strings. The member operation
preserves source origins as widening creates replacement symbols/types. Future
Q3 roots consume this same member ownership; the first Q4 property success does
not prove relationship normalization or all candidate coverage.

## Design the source representation twice

Both alternatives use the same final traversal, semantic description contract,
canonical handles, native type constructors and evaluator. Avoid a generic
backend trait or plugin framework. A temporary experiment switch may select the
acquisition strategy without entering the eventual public API.

| Representation | What is retained | Request work |
| --- | --- | --- |
| Eager compact recipes | During final-tree capture, descriptions for the supported declaration/initializer/member family, with exact payloads and binding/source links. No permanent substrings or full parser arena. | Consume the retained recipe, construct native types, demand members. |
| Captured-source reprojection | Existing captured SourceText and canonical bindings, plus the stable attachments required to verify replay. | Parse through the existing analysis policy, reconstruct exactly the same descriptions on first file demand, reuse them within the request, then release them. |

Prefer reprojection provisionally. The earlier primitive-annotation spike could
not measure canonical binding or actual inference/member demands. This consumer
does. Eager capture wins only if measured semantic latency warrants its added
load, refresh and retained cost on the same captured inputs. A small object case
alone cannot choose the complete representation; imported factories, contextual
calls, recursive types and the fixed corpus journeys remain required.

Acquire both descriptions through the operations callers already use, with
`analysis::Parser`, `Ast::walk`, `Ast::children`, `Ast::token_span` and exact name
owners. Reprojection uses captured canonical filename/per-file facts and the
current deep parse thread policy; it never silently falls back on parser depth
or I/O failure. Shared syntax acquisition should not escape as mandatory
caller-managed setup ordering.

## Concrete test slices and methodology path

Use the registered single integration-test file, such as
`tests/type_vertical.rs`, for genuine new native behavior. The canonical
`scripts/validate/test-changed.sh` supports only `tests/<target>.rs` today.
Registered wrappers resolve from the canonical project root, so an unmerged
local wrapper extension cannot establish a registered private-unit RED. Do not
edit the canonical worktree, run Cargo inside a Node test as a workaround, or
duplicate the crate to expose its private fields.

Two honest paths remain:

1. A bounded private measurement consumer may remain explicitly disposable
   prototype code, using the throwaway-spike test-first exception and registered
   full Project unit controls. This establishes representation feasibility; it
   is not production native checker implementation.
2. For production progress, scaffold the **complete designed**
   `Project::type_of` request/result contract first, then use supported explicit
   integration files for RED/GREEN. Scaffolding has no behavior to pin. After it
   compiles, the cross-file final-result assertion must fail on the missing
   semantic behavior, then pass with the minimum native implementation.
   Unimplemented branches remain explicit failures and status remains partial;
   no testing API or reduced public deliverable is introduced.

Choose path 2 for the native implementation increment. Alias inspection is the
first supported behavior behind the full contract. A bounded path-1 comparison
may run alongside it using the same evaluator, with its limitations explicit.

| Variant | Final expected result or invariant |
| --- | --- |
| Direct import and renamed re-export/import | Same inferred ordinary object members and source origins; selection location remains the selected alias. |
| `const` primitive versus `let` primitive; ordinary `const` object | Primitive const retains its literal, let widens; ordinary object mutable members widen. No annotation substitution. |
| Identifier and exactly representable quoted/escaped key branches once pinned | Exact decoded key and original raw origin; no lossy identity conversion. Start with identifier keys if the additional key branch is not yet frozen. |
| Model reached as an external dependency | Type/member origin retained outside Searched; no dependency occurrence added to lexical results. |
| Unused unrelated alias/reference in the dependency | Existing NotDemanded remains after load and after a request that does not need it. |
| Absent property on the constructed type | Internal property lookup returns absence, not manufactured any. Public invalid member demand must use its defined semantic outcome when that target form is implemented. |
| Unimplemented spread, computed key, contextual initializer, call or recursive cycle | Exact unsupported/recovery failure at the demanded site; never silently omit a member, guess its type, or fall through to a lexical answer. Valid controls remain admitted. |
| Model file changes after Project load | Same answer and origins from captured source. New generation after refresh may change; old Project stays immutable. |
| Final module/await tree; BOM/astral/line-break controls | Existing binding handles attach to the final syntax description and exact UTF-16 origins in both representations. |

Run one explicitly scoped registered file per TDD iteration. Cover an increment's
rule-distinguished variants before widening compatibility once. For limits
introduced by the native owner, pin just-under/at/over final outcomes and a long
flat shallow control. Do not invent reduced compiler thresholds or label an
aggregate global event as the selected type's recovery cause.

## Retained binding cost: compact it twice

The completed ownership stage preserves a fat builder product. The measured
retained requested heap grows by **146.3%** on the 300-module noLib corpus and
**282.6%** on the same roots with default libraries. Peak load changes little
because those allocations already existed transiently. This cost cannot be
hidden inside a claim that eager semantic terms are inexpensive.

Two meaningfully different compact designs are available:

| Design | Durable facts | Source-derived facts and tradeoff |
| --- | --- | --- |
| Compact frozen binding/semantic records | Project durable Decl/Scope/Alias/Ref/Store records after validation, preserving sparse IDs, canonical lookup, meanings, origins, tables and explicit pending state. Share exact name payloads where it is the same identity question; replace duplicated own-site data with typed declaration links where synthetic/alias sites allow it. | Eager descriptions carry initializer/type/body facts. Consumed validation lists and duplicated names/default metadata can disappear only after field-by-field ownership proof. More load retention, simpler request acquisition. |
| Minimal durable binding plus request-local semantic descriptions | Preserve canonical symbol/merge/table/module and alias/reference associations and exact replay handles. Binding decisions that require merges or resolved dependencies remain frozen. | Reproject initializer/type/body and validated source metadata on demand. Cannot drop type-parameter symbol/list links, type-only routes or diagnostic policy merely because syntax exists; source replay is not rebinding. Less retained source-derived data, added request work. |

Choose the second as the starting semantic-source shape, while compacting the
mechanically retained binding product independently. A source recipe strategy
does not justify retaining two copies of validation-only builder records.
Audit actual consumers before removing `Decl.hoisted`, list/conflict metadata,
copied enum/interface names and type-parameter name/default tuples. Some can be
discarded after completed checks; type-parameter order/default syntax and
canonical symbol links still have semantic owners. Store's own sites and
directed Index display projections ask different questions and cannot be
collapsed indiscriminately.

Exact strings currently occur in declaration records, Store sites, table keys
and Index strings. If consolidating them, move the complete binding identity
interner/lookup operation and route all relevant uses through it; do not add a
checker interner or duplicate key decoder. Preserve empty/escaped/exact names,
alias/synthetic origins, sparse IDs, and all existing failures. Compacting maps
or dropping frozen writer-only metadata is a mechanical change only where its
read ownership is proven; any new behavior begins with a final-boundary RED.

## Measurement and actual corpus feasibility

Measure both acquisition candidates with the same native type/member answers:
load and changed-generation refresh; first/repeated native requests; lexical/
navigation/edit compatibility; peak and retained requested heap; description/
checker teardown; and work/depth/stack observations. Keep allocator-instrumented
memory capture separate from elapsed time, alternate order, pin source and
binary hashes, and record failed/unsupported inputs separately. Full semantic
budget constants and service deadlines cannot be frozen from this small leaf.

The current pinned Command Center manifest has **7,639** program sources and
**76,174,045** source bytes: 5,325 searched inputs/53,456,861 bytes, 2,225
non-default external-library inputs/19,745,474 bytes, and 89 default libraries/
2,971,710 bytes. It records revision
`92fe6ab06fbea2c4b6a305689f8cf336173eef81` and effective strict/JSX/path/library
options. Older navigation performance snapshots use different input captures;
their retained totals cannot be called this generation's baseline.

As a deliberately uncalibrated risk scenario, applying the small noLib corpus's
added-byte/source-byte ratio to 76.17 MB would add roughly 1.81 GB of retained
requested heap. This is **not an estimate, bound or admission prediction**:
declaration density, scopes, dependency policy, merges and libraries differ.
The actual corpus must be loaded and measured with the same captured inputs
before deciding its feasibility. Record resource failure as failure. A full
comparison should use the fixed actual native journeys once their evaluator
branches exist, rather than extrapolating a small prototype or benchmarking
refusals as successful semantic latency.

The next decisive implementation result is a compiler-pinned final Project
alias answer produced by native cross-file inference, plus both acquisition
candidates returning exactly the same member types/origins. That result advances
the checker and gives the representation comparison a real consumer. It leaves
the full factory/contextual/flow/member-relation/JSON/recursive/presentation and
worker/resource obligations explicitly pending.
