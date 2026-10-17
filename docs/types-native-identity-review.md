# Native identity review

Status: alternative B adopted in the evaluation baseline of `types-design.md`
and the proposed differences in `references.md`. Equality-only presentation
bookkeeping and eager retained display metadata explicitly do not allocate
ordering identities. The remaining sections preserve the evidence and proposal;
adoption is not an implementation or final-query verification claim.

This review uses the Ousterhout software-design skill. The operating envelope is an immutable local Project with independent, deterministic semantic requests and request-local scratch state. The current contract selects a fresh compiler checker and a fixed call prelude; it does not define the lifetime or thread schedule of the compiler process that allocated AST symbol and node IDs.

## Finding: a fresh checker does not define one native answer

Pinned TypeScript Go 7.0.2 stores `nextSymbolId` and `nextNodeId` in process-global atomics (`internal/ast/utilities.go`, `GetSymbolId` / `GetNodeId`). A new checker does not reset them. `GetSymbolId` assigns on first use, not on symbol construction. Concurrent compare-and-swap can also consume unused numbers.

The maintained control is `tools/type-symbol-id-collision-oracle.test.mjs`, with source in `fixtures/types/native-symbol-id-collision/cases.ts`. Registered run **vrun-969941b9-9e68-43e7-99f3-05c206668c19** passed all three tests with zero skips. The earlier inequality probe passed as **vrun-ca455c2c-e6e3-4d55-a95d-e8768ee8618f**.

The fixture assigns a number to a literal property and a string to a stable dynamic index, both before and inside a loop. Its selected result reads the literal property first and the dynamic index second:

```ts
obj["@\u0002\u0000\u0000\u0000\u0000\u0000\u0000\u0000"] = 0;
obj[key] = "value";
// The same two assignments occur in the loop.
const answer = {
    warm: obj["@\u0002\u0000\u0000\u0000\u0000\u0000\u0000\u0000"],
    result: obj[key]
};
```

The test-only oracle allocates zero, one, or two unrelated native symbols before constructing the same fresh program/checker. It uses the same serial binding setting and call prelude each time. The maintained assertions compare every captured input hash and the compiler options across the three executions. The hook observes the ID inside the existing `keyBuilder.writeSymbol` call; it does not make an extra ID request.

| Prior unrelated IDs | Actual `key` ID | Final native summary |
| --- | --- | --- |
| 0 | 2 | `{ warm: number; result: number; }` |
| 1 | 3 | `{ warm: number; result: string; }` |
| 2 | 4 | `{ warm: number; result: string; }` |

Every result has `errorType == false` and no acquired demand failure. The only permitted diagnostics are the expected missing-global-library diagnostics from `noLib`. Replacing the literal key with ordinary decimal text `"@2"` returns `result: string` under both tested histories.

This is a final type difference, not merely a changed private cache key. It demonstrates that the current fresh-checker wording alone leaves an observable process-history dimension unspecified. It does not show that every native ID assignment is semantically observable.

## Mechanism and concurrent binding evidence

`checker.writeFlowCacheKey` (`internal/checker/flow.go`) writes a literal access as its object key followed by `'.'` and raw property-name bytes. A stable identifier index writes the same object key followed by `".@"` and `keyBuilder.writeSymbol`. The latter writes **eight little-endian binary bytes**, through `hashWrite64`; it does not write decimal digits. When the symbol ID is 2, the two byte sequences in the fixture are identical and reuse the same loop-flow cache entry.

Native type IDs use four little-endian bytes. Node IDs use the same eight-byte numeric representation as symbol IDs. The decision therefore concerns all identity-bearing key components, not only this one dynamic-index branch.

Binding also affects the process epoch. `binder.GetSymbolNameForPrivateIdentifier` embeds `GetSymbolId(containingClassSymbol)` in `FE#<decimal ID>@<description>`. `Program.BindSourceFiles` queues each source file through `core.NewWorkGroup`. The default work group starts goroutines. The single-thread work group pops from the end of its queue: it binds files in **reverse** program order.

The third maintained test creates 12 independent private-field classes, reads only their already-bound private-name suffixes, and queries the same unrelated scalar result. Reading the suffixes requests no new ID or type. Two serial fresh runs assigned classes C0 through C11 the IDs `[12,11,10,9,8,7,6,5,4,3,2,1]`. Four parallel fresh runs recorded these different permutations:

```text
[2,3,4,6,8,1,5,7,10,11,9,12]
[2,3,4,5,7,1,6,8,11,9,10,12]
[2,3,4,6,7,1,5,8,10,9,11,12]
[2,3,4,5,1,6,11,12,8,7,9,10]
```

The test asserts serial ordering and that every parallel observation is a permutation of the actual allocated IDs. It deliberately does not require a particular parallel permutation, or require two runs to differ. The captured artifact is `.cc/temp/type-symbol-id-bind-observations.json`. All six scalar results were `0`; this concurrency control demonstrates variable allocation order, not a second final-answer difference.

## Two complete alternatives

**A. Define and reproduce a native process epoch.** Extend the contract to a fresh native process, explicit `SingleThreaded` binding with its real LIFO order, no semantic/API warmup beyond the defined prelude, and the precise intrinsic initialization and later ID-request schedule. Retain one hidden identity epoch, including binder-private allocations before checker operations. Symbol comparison, generated private/unique names, flow keys, and other native ID consumers share it. This preserves the proven binary-key collision for that one canonical native epoch. It requires reproducing allocation events from binding, checking, and relevant formatting; a counter that starts at one when the evaluator first encounters a symbol is insufficient.

**B. Specify opaque identity as a deliberate tool difference.** Preserve actual canonical symbol, type, and node identities, but do not serialize their incidental process numbers into the same domain as user-name bytes. Define identity-derived internal names structurally as well. This makes Project results independent of unrelated process allocations and compiler thread scheduling. It changes the proven fixture's result to `string` and changes ordering where decimal IDs previously participated in generated names. It must be recorded as an observable exception to native behavior, with maintained native and final Project controls.

For the stated local Project envelope, **B is recommended**. It confines identity decisions to one owner instead of making unrelated native process history a hidden input to every query. This is a contract choice, not a behavior-preserving refactor or permission to replace unsupported semantic types with approximations. A literal promise to reproduce all native process histories would require A plus an explicit history input; one deterministic answer cannot equal both proven native answers.

## Proposed boundary for B

Binding continues to own canonical `NativeSymbolIdentity`, including role-specific aliases and merged symbols. It never substitutes a Store index for native lazy ordering. Model owns actual type identity. The captured AST owns actual node identity. Neither presentation strings nor source locations reconstructed from display establish identity.

One request-local identity-order owner records first observations where an admitted semantic operation consumes an identity-bearing key or compares identity atoms. Comparisons retain native identity/nil, declaration, and name branches before requesting their final lazy tie. Left is observed before right at that tie. Do not allocate ordering merely because a symbol was constructed, and do not create separate ordering maps for the comparator and flow engine. Binding-produced generated names retain symbolic components only; their first semantic key acquisition records the owner identity in this request's order. Thus immutable binding and arbitrary parallel binder scheduling do not preseed the request's order. This lazy acquisition is part of B's deliberate difference, not a claim to reproduce native private-name allocation time.

The comparator's observation schedule is tool-defined where final identity ties are reached. The implementation retains its stable fallible merge sort, rather than claiming the same comparator-call sequence as native Go `slices.SortFunc` (pdqsort). Native declaration and exact-name branches remain unchanged. The sort and all flow-key writes share Model's one request-local order; `RefCell` borrows cover only the leaf rank-map operation and never a Source, evaluator, or recursive comparator demand.

The implementation must distinguish retained facts from an actual ordering observation. For example, native nodebuilder `getNameOfSymbolAsWritten` acquires a symbol ID for its remapping cache, while our current Source operation of that name eagerly retains display metadata in factories. Wiring an observation into that Source read would wrongly make symbol construction count as a display demand. Record an included presentation observation when the presenter actually uses the name, or explicitly exclude equality-only presentation bookkeeping from the adopted ordering policy. The proposed minimum policy includes flow-key symbol writes, identity-derived key acquisition, and comparison ties; any additional native ID-based semantic owner must reuse the same state when it is introduced. Eager metadata acquisition must never silently enlarge that set.

The flow-key interface should be a builder with four private component kinds:

```text
Symbol(actual canonical symbol identity)
Type(actual Model type identity)
Node(actual captured node identity)
Bytes(exact bytes)
```

`write_byte` and `write_string` coalesce adjacent byte components. Thus native named-dot concatenation remains unchanged: this proposal does not distinguish `obj["a.b"]` from `obj.a.b` solely by introducing property-path segments. `write_symbol` also records the shared lazy identity observation even though cache equality uses the actual symbol identity. Callers cannot manufacture identity components from strings or convert them back to decimal/binary numbers. Equality checks component kind and identity, not just a hash.

There is a second required owner boundary for generated names. Native private names contain a decimal class ID; unique-symbol names contain `FE@<actual symbol name>@<decimal symbol ID>` (`getESSymbolLikeTypeForNode`). Decimal lexical comparison changes at boundaries such as IDs 9/10 versus 10/11 even when relative allocation order is the same. Replacing process IDs with decimal request-local ranks would retain that incidental spelling rule and would not constitute opaque identity semantics.

Retain generated keys as typed data:

```text
Fixed(exact native bytes)
Private(actual declaring-class identity, exact private description)
Unique(actual symbol identity, actual underlying native symbol name)
```

Equality includes the actual identity and exact name components. Proposed ordering within `Private` uses the shared lazy owner order, then exact description; within `Unique` it uses the underlying native name, then shared lazy owner order. Preserve the native reserved family prefix ordering (`FE#` before `FE@` before fixed `FE`-letter names). Ordinary user names and fixed reserved names retain their exact byte comparisons. This explicitly changes identity-generated-name ordering; it does not normalize, case-fold, or decode exact user names lossily. Private and unique factories are currently absent, so their whole producer/lookup/order implementation must adopt this boundary when introduced; the current precise missing-factory failures must not become guessed names.

The underlying name inside `Unique` must itself retain this structured representation: for example, the symbol can be a private static readonly unique-symbol property. It cannot be forced into a byte string by a second encoder. Generated ordering differences also include prefix/punctuation cases, not merely decimal boundaries. Native bytes `FE@a@9` sort after `FE@a@!@10` because `!` precedes `9`; structural comparison of the underlying names `a` and `a@!` orders the former first. Future generated-factory controls must pin both this case and decimal 9/10 versus 10/11. All valid scalar UTF-8 user names remain below the invalid FE prefix, and fixed reserved names retain exact native bytes.

The existing comparator split also needs a concrete migration. Model's `CompareTypes`, inherited-member sorting, and Binding's current `named_symbol_members` sort all call `compare_symbols`. Binding has no request-local ordering state. Have Binding return the retained table inventory, and make the evaluator's complete `getNamedMembers` operation own both partitioning and sorting through the same stateful symbol-comparison owner. Do not leave a pure fallback in Binding and invent a second tie-breaker in Model.

## Proposed normative wording for review

> Semantic identities are opaque and retain their actual symbol, type, and node associations. Compiler-process numeric IDs are not inputs to Project query results. Where native internal keys combine identity numbers with user-name bytes, this tool uses distinct identity components and exact byte components; adjacent name bytes still concatenate according to the native operation. Consequently, a user property name cannot collide with an encoded symbol, type, or node ID. Identity-generated private and unique names retain their actual identity and exact textual components rather than decimal process-ID spellings. Their identity component ordering uses one deterministic request-local lazy ordering owner; ordinary names, fixed reserved names, and native declaration ordering remain exact. These identity-key and generated-name ordering rules are deliberate observable differences from TypeScript's process-global ID serialization.

This wording must be adopted in the design's evaluation baseline and the documented-differences owner before the changed behavior is claimed complete. Validation should retain the native prefix controls above, a final Project regression for the selected loop result, unchanged named-dot concatenation controls, and generated private/unique equality and ordering controls at the relevant factories. No absolute compiler IDs become part of the public result.

## Generated-name length refinement during implementation

Native nodebuilder's `addPropertyToElementList` charges
`len(ast.SymbolName(propertySymbol)) + 1` to approximate display length.
`getESSymbolLikeTypeForNode` embeds the decimal `GetSymbolId` in a unique name;
that process-global atomic is lazy and compare-and-swap can consume unused
numbers. Consequently generated-name length, truncation and later dependency
acquisition can vary with unrelated process history even when key equality
and ordering are already opaque. A fresh checker does not fix this width.

The adopted B contract now counts each opaque identity atom as **one logical
unit**; exact prefix, separator and textual byte components retain their native
lengths, recursively for generated underlying names. Length reads do not
allocate ordering identities. Native `SymbolName`'s actual private-declaration
special case precedes this operation. One typed-key owner supplies the measure
to all consumers; no consumer renders an identity to decimal text.

This is an explicit observable exception for generated-name cutoffs and their
dependency acquisition, not a claim of exact native display scheduling. Fixed
names retain native lengths. Request-rank decimal width was rejected because it
reintroduces incidental spelling and makes unrelated identity observations
affect cutoffs. Reproducing a process epoch was rejected for the same operating
envelope reasons as alternative A. Conservative width intervals would add a
second partial scheduling policy and still need an answer where intervals
straddle a cutoff. A one-unit opaque atom makes the decision complete within
the existing identity owner.

The implementation agent's adversarial Ousterhout review accepted this boundary
with required proof: native width-only cutoff controls; final Project controls
below, at and above the deterministic cutoff; unrelated-allocation invariance;
and unchanged fixed-name cutoffs. These are obligations, not yet completed
validation claims.

## Remaining proof and scope limits

- The prefix collision and native binding allocation observations are proven by the maintained three-test run above.
- The typed-key implementation and final `tests/type_flow_identity.rs` passed the coordinated three-test Rust gate, **vrun-ca5a9507-8ad7-4965-be7c-88dcea648595**, after a controlled revert of the stable-index branch produced genuine assertion RED **vrun-522222d0-abe7-4800-a32e-9e6153093a61** (two failures, named-dot control still passes). The restored source was byte-identical to the saved implementation. The maintained strict/loose native control `tools/type-flow-identity-oracle.test.mjs` passed four tests, zero skips, in **vrun-239632bd-3d5d-4538-9f65-8c675a5cefe0**. It covers stable parameter/const/unassigned-let indexes, assigned-let ineligibility, nested suffixes, unchanged named-dot collision, and the binary-ID prefix collision. The final binary control warms an unchanged union, then requires the distinct dynamic read to report its exact `FlowRequired` dependency under the current F profile; a silently cached union and an unrelated pending-key refusal both fail the assertion.
- A final comparator tie requires distinct actual symbols with equal declaration/name facts. `tools/type-symbol-order-identity-oracle.test.mjs` compares two actual binder-created class prototype symbols, each with no declaration and the same native name. It passed **vrun-ee70b04b-fa4e-4b81-ad0f-570a863436f4**, one test, zero skips, across prefix 0/8 and fresh/prior-right observations. The matching IdentityOrder tests also check that exact declaration/name decisions do not allocate lazy ranks. This is limited owner proof: no final Project producer reaching that tie has yet been established, and this control is not presented as that missing final-query proof.
- Private/unique generated-name ordering is a specified proposal supported by the native formulas, not a final public-query control yet. It needs native controls at decimal-boundary prefixes before those factories claim completion.
- This review does not redefine type allocation order, native object structure, constraint/mapper semantics, demand failures or product resource guards. Generated-name approximate display length is the explicit formatting exception above. Actual type identities and their factory facts remain required.
- The exception covers identity-number serialization, ordering and generated-name approximate display length. Removing ordinary named-dot concatenation collisions would be an additional behavior change requiring its own explicit decision and controls.
