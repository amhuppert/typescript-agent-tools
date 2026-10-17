# Native typeof type queries

This increment extends the designed public T/Q4 operation over one captured
Project and its retained canonical binder. The operating envelope is unchanged:
request-local native checking, exact captured sources, fixed fresh-demand
prelude, and no production compiler subprocess. The complete semantic profile
and its remaining resource/relationship gates remain required.

TypeScript 7.0.2 commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`
supplies the contracts. `getTypeFromTypeQueryNode` (checker.go 24010) demands
`checkExpressionWithTypeArguments`, then `getWidenedType` and
`getRegularTypeOfLiteralType`. A const literal is regularized, not replaced by
its primitive base. TypeQuery aliases do not create an object display alias.
The operator keyword selects a Type question and the operand selects a Value
question at its actual location. That Value question cannot use a declaration's
`getTypeOfSymbol` result and bypass flow. `GetTypeOfSymbolAtLocation` (16367)
routes an actual identifier expression to `getTypeOfExpression`; it also has
separate hypothetical-location and property-write branches that are not
approximated by this first identifier family.

`checkIdentifier` (11014) continues to own variable/alias flow setup.
`IsInTypeQuery` (utilities.go 146) walks only Identifier/QualifiedName ancestors,
stopping on any other kind. For nonautomatic types, this fact participates in
the native assumeInitialized disjunction; automatic types still require their
evolution question. Union narrowing, closure capture, assignments and call
effects remain real dependencies. No type-query flag skips those operations.
The binder's existing TypeQuery collection captures VALUE references with the
actual type-context bit. Native `resolveNameHelper` emits TS1361/TS1362 for
runtime use through a type-only alias but still returns the eligible alias;
`getTypeOfAlias` supplies its healthy value type. This diagnostic does not create
errorType recovery or change the query into runtime authorization. Semantic
acquisition must retain that raw alias instead of reusing the legacy lexical
runtime projection's missing/outer fallback target.

## Designing twice

One option would resolve a typeof operand's spelling into the declaration and
return its cached value type. It makes ordinary examples work, but erases the
actual flow reference and exposes a wrong contract for narrowed unions,
automatic variables and separate raw aliases. A separate bespoke typeof reader
would preserve the location but duplicate checkIdentifier policy.

The chosen boundary extends the existing completed-reference acquisition owner
with a distinct type-query operation and extends the existing Source flow syntax
fact. The same checkIdentifier owner chooses the appropriate complete binder
operation from that fact and performs normal flow traversal. Source captures a
TypeQuery recipe and a separate Value-at-location seed; Evaluator owns the
whole type-node operation, including widening and regularization. Callers do not
select lookup meanings, remember initialization suppression, or invoke flow
manually.

No parser AST or raw name becomes a semantic identity. Exact span/role attaches
the captured operand to the existing reference association. Runtime and
type-query acquisition share mechanical reference matching. Both use the raw
eligible alias's completed VALUE target for its actual type question, while
context still selects the correct captured reference and initialization setup.
Lexical reporting keeps its deliberate type-only restriction. Raw alias versus
direct symbol identity and alias-own flow stay unchanged. Unvisited or failed
aliases remain failures; the operation never repeats lookup or forces an alias.

## Initial behavior and evidence obligations

Compiler controls distinguish operator Type and operand Value for ordinary
const/mutable variables, objects, runtime and type-only renamed imports,
nonautomatic uninitialized variables, automatic variables, narrowed unions,
unresolved values, instantiation arguments and qualified names. Every demand
uses a fresh pinned checker and the existing checking prelude. A disposable
fresh API control is recorded in `.cc/temp/types-type-query-probe.json`; it is
research evidence, not a registered shipping gate. The first registered compiler
run `vrun-e103df0d-12a0-4273-a22d-4682d0c2f113` exposed the shared oracle's
synthetic-keyword symbol-acquisition bug before the selected demand; it is not
a baseline pass or a native behavior RED. The shared owner confirmed a
behavior-level doesNotReject regression in main suite RED
`vrun-4ce35d57-b107-411c-99b2-f163aad23106` before fixing that operation;
its final 29-test suite GREEN is
`vrun-c91e8e2c-6a28-4e1e-a322-595e6a13dc55` with zero skips.

Registered compiler run `vrun-5f3f3ac5-2c49-4a70-bca6-d2b9ab208b04`
executed/passed four controls with zero skips. It includes same-checker runtime
type-only diagnostic 1361 with healthy literal/object types, and typeof operand
controls without 1361/2454. Native selected-operation RED
`vrun-2a1f5b80-0542-4f84-b42b-330d306d5db1` compiled and executed three
tests, failing all three selected-Type/consumed-recovery assertions because the
keyword was unselected. The independent runtime alias counterexample was then
correctly pinned to native Found; RED
`vrun-45324dfc-5c8c-4916-bd22-775ff4347ccb` executed four tests with three
passes and the alias's false UnresolvedSymbol failure. The complete acquisition
owner was corrected and final Project run
`vrun-3e99c901-4f3b-4080-af31-9ef17e37cf78` passed the four-test file with
zero skips. Extended compiler coverage
`vrun-f1ebd852-251b-4e2a-a8a3-12b5e6359341` also passed four tests with
zero skips: specifier/export-type routes, same-named outer global values, and
the separate this-query question. It does not claim a healthy native this
implementation; the product preserves the exact pending operation.
The extended final Project file passed with zero skips in
`vrun-f8331648-4a69-4963-9712-d80d9acb99b6`. It checks the same six
clause/specifier/export-type alias operands through Type and Value questions,
their separate runtime Expression questions, and paired same-named outer
globals. A preceding retry `vrun-078ef029-a546-43ab-aa9a-d3c585d9668d`
failed compiling the concurrently staged effects owner's `demand_type` calls;
it is a setup failure, not a behavior RED.

The disposable API probe predates the extended fixture. Its raw artifact is
point-in-time research evidence; the registered extended compiler run is the
current fixture's expectation evidence. The final assertions prove selected
question, outcome and summary for this family, rather than every neighboring
graph or source-location contract.

The first implementation admits plain identifier operands without type arguments
through the existing complete value/flow operations. Qualified names, this
queries and TypeQuery instantiation arguments retain explicit located native
construction obligations until their property/checkThis/instantiation owners
are admitted. Operand questions and whole-node questions remain separate:
`typeof generic<string>` and its `generic` operand have different native results.
Automatic evolution and demanded union narrowing retain FlowRequired when the
designed admission policy excludes them. Consumed unresolved operands remain
InvalidSemantic/ErrorType; unrelated diagnostics do not invalidate a healthy
query. These are staged boundaries, not reductions of the mandatory profile.
