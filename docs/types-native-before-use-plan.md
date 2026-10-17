# Native declaration-before-use operation

The immutable Project and request-local evaluator already retain each actual
declaration and usage node. The shared operation must answer TypeScript 7.0.2
`isBlockScopedNameDeclaredBeforeUse` (checker.go1908), both for computed constant
keys and for class property access. A native false answer is ordinary data;
source acquisition, property-type and flow failures remain failures.

The current `Source::key_declared_before_use` position approximation will be
removed. Its caller must use the same operation as class access. The property
context's `before_declaration_ancestor_candidate` is only a later ancestor-base
condition; it cannot replace this preceding before-use question.

Two boundaries were considered. A Source boolean with a special static-property
exception would hide a semantic dependency or require callers to reconstruct
the rest of the rule. Moving the entire operation to evaluation would expose
syntax ancestry, node positions and option interpretation. Instead Source owns
the complete syntax decision and returns an ordered plan: static-initialization
questions, the result when a question succeeds, and the result when none does.
Evaluation executes those questions in order. This is the one semantic branch
of the native traversal, rather than a general syntax interpreter.

Each static-initialization question retains the actual property declaration,
its name node, the actual class/static-block nodes, and native start/end
positions. Evaluation first acquires `getTypeOfSymbol(getSymbolOfDeclaration)`
even when there are no eligible static blocks, then delegates the actual
`isPropertyInitializedInStaticBlocks` flow question to the access-flow owner.
It must not skip an earlier question merely because a later syntax branch
would establish deferred use. Within the native ancestor walk, every successful
static-initialization question returns the same deferred-use result, so the
ordered plan does not require an evaluator copy of the syntax decision.

The syntax owner covers different-file early success; type-query and ambient
or type-node deferral; native declaration Pos comparisons; binding-element
siblings and containing-variable recursion; immediate variable initializers;
class computed names and modern versus legacy decorators; property self-use;
parameter properties and standard class-field emit; export specifiers and
export-equals; function versus IIFE deferral; static blocks; static and instance
property initializers; and decorator ancestry. Existing function flags,
parameter-property, this-property, type-query and IIFE owners are reused.
GetEnclosingBlockScopeContainer/IsBlockScope, GetContainingClass,
isSameScopeDescendentOf and isPropertyImmediatelyReferencedWithinDeclaration
receive native-named owners where none already exists. JavaScript/JSDoc
assignment bindings remain outside the documented input profile.

Implementation starts only after maintained compiler controls and genuine
final-query RED. Computed-key access-flow controls distinguish earlier and later
constants, deferred functions, direct IIFEs, async/generator IIFEs, and shadowed
declarations. Class controls distinguish computed names, property self-use,
parameter properties, static initialization and each relevant compiler option.
Where final display conceals a consumed invalid static-property dependency,
record an actual native recovery observation before asserting product failure.
All exact property-type and flow questions must retain their original error
location and failure identity.

## Integration facts established during the native checkpoint

The flow reference will use a typed `Captured(Site)` / `Created(id)` identity.
The request owns fresh `This` and `PropertyAccess` nodes for each eligible static
block, with the actual property name node and actual block parent. A reference
syntax adapter feeds the existing matching, loop-key and narrowing-context
operations; it must not invent a captured span or copy those decisions into a
synthetic-only checker. Created node identity occupies a separate domain from
captured file/node identity. The normal static-block parent proves its context
through the same parent-role rules as captured references.

The binder currently retains `EndFlowNode` before completing a return target.
Native `bindContainer` (binder.go1571–1575) installs a distinct `ReturnFlowNode`
after the return-label merge for constructors and static blocks. Retain both
fields and acquire the latter for initialization questions; using the former
would erase return/finally paths and unreachable distinctions.

A synthetic reference uses declared `propType`, initial
`getOptionalType(propType, false)`, and that actual initial identity as the F
admission baseline. This admits an unchanged optional result (a healthy false
initialization answer), and Any/Unknown when optionality preserves identity.
An assigned result that differs from initial still requires F even if it is the
declared property type. Already containing undefined does not exempt a later
refinement. Native absent-flow and unreachable-never postprocessing keep their
existing semantics in the shared interpreter.

Initial final tests are staged in `tests/type_before_use.rs`; the shared
syntax/flow implementation is now staged and awaits coordinated validation. Combined-source compiler controls are
retained because preceding static blocks may participate in the actual flow
question. Exact declaration/use and property-name/block observations must
separate nested acquisition from the selected query before pinning outcomes.

The keyed run `vrun-22b0575b-933a-4a83-bf57-58249744bfd0` establishes
strict/legacy selected-query observations independently from nested uses. Empty
blocks preserve the optional initial type. Both assignment blocks and the
combined-source AlreadyOptional case return a different flow identity. In that
combined source, an earlier `this.later` assignment remains visible to native
matching at a later class's static block; keep that case rather than isolating
away the demand. Standard class-field semantics still produce a false final
before-use answer after consuming the static-flow question, while legacy emit
can return true. The product's F gate applies to the consumed flow either way.

Loose null checks reach a native invariant failure at the actual
getOptionalType request from isPropertyInitializedInStaticBlocks. Both the
version-guarded source hook and the shipped public 7.0.2 API panic with
`Debug failure` at getOptionalType for EmptyBlock.later; this is not a hook-only
assertion or a semantic errorType. The product reports
UnsupportedSemantic/UnsupportedSemanticForm at that precise request, rather
than inventing a healthy optional type, InvalidSemantic recovery, or FlowRequired
answer. Loose queries that never ask that optional-type question remain ordinary
healthy controls. The maintained harness separately checks the remaining static
block variants against both native routes.


The staged owner now routes computed constant keys and property/class access
through the same ordered operation. `ReferenceNode` owns fresh identities for
created This and PropertyAccess nodes; only the outer access is returned by the
factory. Matching and loop-key traversal can inspect its private This child,
but contextual and narrowing questions receive the outer native reference.
`ReferencePosition` sends actual captured parent facts through the same Source
parent-role decision without a fabricated source span or contextual-frame hit.
Internal flow questions use exact initial-identity admission; existing selected
value questions retain their established literal-base proof separately.

The sparse binder attachment owner now also retains ReturnStatement entry flow
at native bindChildren's statement attachment point, before binding the return
expression. It is distinct from a function/static-block ReturnFlowNode. Fresh
predicate Condition/Start nodes reuse the same interpreter; actual predicate
inference remains excluded by the first changed-flow admission.

`tests/type_before_use_syntax.rs` covers nineteen final access queries across
strict, legacy field/decorator emit, and loose profiles, including ordinary and
shadowed keys, deferred functions and arrows, ordinary/async/generator IIFEs,
computed class names, self-reference, parameter properties, static deferral,
optional/asserted properties, nested access, inheritance and modern/legacy
decorators. Its maintained native harness also records the actual before-use
questions without issuing additional checker calls. Validation receipts remain
required before claiming these staged changes complete.

The syntax native baseline is GREEN
`vrun-a0754c71-9056-4a70-b1d6-d320cf568d68` (57 final questions,
1 registered harness test, 0 skipped). The source changes and corresponding
Rust final tests remain uncompiled until the coordinated checkpoint. Earlier
static-flow/invariant native proof remains
`vrun-f2e9bd2e-d127-4a27-8ad7-d184f665edc7`.
