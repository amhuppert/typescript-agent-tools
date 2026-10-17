# Native type-literal source checks

Status: immutable IndexInfo and the computed-expression attachment repair are
implemented and tested. The final source-check failure-order matrix is GREEN;
the healthy matrix reaches the remaining late-member producer dependency.
Full source-check closure is still pending. This uses the Ousterhout skill
for the existing local, immutable Project and request-local evaluator; it adds
no compiler service or cross-request cache.

## Operation boundary

The missing operation is `checkTypeLiteral` (pinned checker.go3120), not a generic
walk over member types. Its sequence is member source checks in source order,
actual anonymous type acquisition, `checkIndexConstraints`,
`checkTypeForDuplicateIndexSignatures`, then
`checkObjectTypeForDuplicateDeclarations`. The enclosing source-check dispatcher
continues to reset the instantiation counter for every source element. There is
no new type-literal checked sentinel: native does not install one here.

A children-only patch would return healthy answers without raw property checks,
secondary-declaration Identity relations, index compatibility, or duplicate
symbol acquisition. A separate property-specific copy of the parameter checker
would also duplicate `checkVariableLikeDeclaration`. Instead extend the current
source-check owner with typed declaration recipes and extract its existing
parameter logic into one shared variable-like operation.

## Required producer facts first

`Index` now wraps an immutable `Arc<IndexInfo>` created by Model's `newIndexInfo`
port. It retains actual optional declaration and component declarations as well
as key/value/readonly/display-name. Source `IndexSignatureRecipe` carries the
actual declaration identity. All existing factories now use this owner. Ordinary
cloning preserves identity. `instantiateIndexInfo` preserves the original record
when its value is unchanged and creates a fresh record when changed, retaining
key/readonly/declaration/components. Inherited records remain shared. Combined
applicable indexes have a fresh identity and no invented declaration. The native
Any-base index is one startup record shared by static and instance inheritance.
`appendIndexInfo` owns both union and intersection value/readonly policies.

Compared with a numeric identity beside mutable fields, the immutable handle
prevents callers from changing an established value while preserving identity.
Read-only field access keeps existing consumers simple; pointer equality answers
the actual native identity question without an interner or parallel cache. This
choice was independently reviewed using Ousterhout's information-hiding and
caller-complexity criteria. Its source-check identity consumers and end-to-end
proof remain required; the staged representation alone does not establish them.

Source recipes retain actual NodeIds for member order, annotations, initializers,
computed names, actual containing declaration and grammar short-circuit facts.
They return facts, not eagerly acquired types. The binder's canonical ordered
native declarations and actual symbol Parent/ValueDeclaration are already owned
and should be observed through those APIs. Do not infer them from origin spans
or printer metadata.

## Member source checking

1. `checkPropertySignature` calls the property declaration checker. Preserve
   modifier/property grammar short-circuits before variable-like checking.
   Grammar is not uniformly syntax-only: `checkGrammarForInvalidDynamicName`
   calls `isNonBindableDynamicName` and `isLateBindableName`; eligible computed
   names acquire their expression type before the annotation is source-checked.
   Later `checkVariableLikeDeclaration` source-checks the annotation, checks all
   computed names again through their existing cache, then any computed-name
   initializer. The native comment saying this exits is stale; code continues.
2. Extract current parameter source checking from
   `evaluation/type_checks/signatures.rs` into shared
   `check_variable_like_declaration(actual_declaration, consumer)`. Reuse canonical
   value/property acquisition, `convert_auto_to_any`, declaration-local widening,
   cached initializer checking, and Identity/Assignable relation owners. Compare
   the actual declaration with ValueDeclaration. Secondary initializer checking
   uses its declaration-local type even after a diagnostic-only Identity failure.
   The bodyless parameter initializer return stays before raw symbol acquisition;
   its enclosing parameter worker still performs the separate rest check.
   Type-literal properties skip the native non-property export/collision tail.
   Existing binding-pattern/class-initializer producer boundaries remain explicit
   at their real branches; they are not healthy no-ops or proof of those families.
3. Route MethodSignature through the native method wrapper, not directly through
   signature checking. It also checks computed names, canonical overload symbols
   with their own checked state, and acquires the return annotation before the
   bodyless return-path check can exit. Bodyless declaration groups do not need
   implementation-body compatibility, but that decision uses actual family facts.
4. Route GetAccessor/SetAccessor through the accessor wrapper. Signature checking
   alone is insufficient: the canonical pair's accessor type is acquired even in
   a bodyless type literal. Reuse the current getTypeOfAccessors/value owner.
5. Extend signature grammar preflight for IndexSignature. Native parameter-count,
   trailing-comma/rest/modifier/question/initializer/missing-type gates precede
   semantic key acquisition. After those gates it acquires the key, rejects
   literal-or-unique or generic keys, then checks every union constituent with
   the shared `is_valid_index_key_type`; afterward normal parameter and return
   source checks still run even when grammar emitted a diagnostic. Do not
   substitute index inventory construction: its value-before-key schedule is a
   different question.

## Index and duplicate checks

`check_index_constraints(ty, actual_symbol, is_static, consumer)` owns the complete
native checker.go4769 operation. Acquire index infos first and return immediately
when empty, without property-value demands. Otherwise visit ordered object
properties; skip Prototype only for static checking. Acquire each property's
literal name with includeNonPublic=true before its nonmissing value, then invoke
the property helper. A private name returns only inside that helper, after those
two acquisitions. Class-only unbindable computed-member handling stays under its
actual class/static gates; the new TypeLiteral entry does not invent a class.

Add plural `applicable_index_infos` beside the existing pair predicate. It is an
ordered filter over every info using `is_applicable_index_type`, unlike the
existing singular operation which combines several matches. For each property
or index pair, preserve native local Parent/declaration gates, then the Interface
separate-base fallback, and only then Assignable. Diagnostic-only false continues
with later infos. Index-pair checks skip only actual IndexInfo identity. Existing
base, exact index, own-property, literal-name and nonmissing-value owners supply
these questions. Preserve any demanded invalid/resource failure even if native
would subsequently recover for diagnostics.

`check_type_for_duplicate_index_signatures` observes the canonical reserved index
symbol and its complete native declaration list, not the already deduplicated
index inventory. With more than one declaration, visit actual IndexSignatures
having one annotated parameter, acquire keys in native declaration order,
distribute unions, and group by actual TypeId. Late-bound declarations are skipped
by their real syntax kind. The class/interface checked-state wrapper is separate
from this direct TypeLiteral operation. No helper-level cache replaces that gate.

The final object-duplicate worker must still acquire the symbol for each actual
member. Even when emitted diagnostics do not affect the query result, late-bound
symbol acquisition can be semantic. `reportDuplicateMemberErrors` also rescans
all members and acquires their symbols when the first duplicate report is
triggered, before the outer loop reaches those later members. Preserve this
rescan and the actual property/accessor kind state machine; do not omit the worker merely because ordinary identifier cases
only produce diagnostics. Type-to-string diagnostic demands need separate pinned
controls before deciding they are irrelevant to the acquired-dependency contract.

## Implementation ownership and proof

One implementation lane should own Source/type_checks, evaluation/type_checks
and the small shared variable-like extraction. The Model owner publishes the
IndexInfo factory before consumer migration; current index factory sites migrate
together. Binding adds only missing actual-declaration observations. This avoids
source-check and member-inventory code each owning index-key classification or
variable-like value selection.

The adjacent index-key inventory bug already has maintained native proof:
`vrun-2a0753c0-bd5e-4b8a-bd26-48450ae1766b`, 34 queries in each of strict, loose,
exact-optional and unchecked profiles, zero skips. The fixture is
`fixtures/types/native-template-indexes`, with final consumers in
`tests/type_template_indexes.rs`. Pattern/mapping/branded keys, generic rejection,
union distribution, overlapping and duplicate keys, value-before-key and
value-before-duplicate failures are covered. That is factory proof, not proof of
the complete source-check schedule.

Next source-check controls should use explicit generic call type arguments and
checked assertion/satisfies expressions, paired with direct quick assertions.
They must distinguish:

- Property annotation source constraint versus raw body acquisition; first member
  failure versus later member; diagnostic-only duplicate mismatch followed by a
  later consumed Missing dependency.
- Early duplicate reports that acquire a later invalid computed member during
  their report-time rescan, before intervening outer-loop work.
- Computed-name grammar acquisition before annotation, versus syntax grammar
  short-circuit followed by annotation first; ordinary names as a control.
- Index grammar key/parameter-before-return acquisition versus factory
  value-before-key; invalid key with healthy and invalid return types, and
  bodyless initialized parameters with their separate rest checks.
- Zero indexes skipping property compatibility demands; matching versus
  nonmatching pattern indexes; string/number/symbol and branded keys; optional
  properties under strict/loose/exact; explicit Undefined versus Missing.
- Same inherited IndexInfo versus newly instantiated value; local property,
  local index, and separate inherited-base interface paths. Avoid imposing these
  interface-only gates on the type-literal caller.
- Duplicate index declarations hidden by inventory deduplication, union keys,
  native declaration order, and late-bound declaration exclusion when that
  producer is admitted.
- Method overload and getter/setter pair source checks with consumed failures in
  parameters/return annotations, plus independent healthy roots after failure.

Capture fresh native summaries, before-summary diagnostics and meaningful
ordered dependency traces before final expected outcomes. Final Rust assertions
must retain original invalid/resource identities and exact locations. After
GREEN, counterfactually remove each whole behavioral gate that the controls are
intended to prove; restoring only old Pending is insufficient for omitted-demand
branches. No additional surface family is part of this closure.

## Current evidence and attachment repair

Native source checks cover 45 roots in each of strict, loose and exact-optional
mode. The combined source-check and diagnostic-display asserted run is GREEN
`vrun-87c053a8-6460-46e1-a9cb-cc5bd0fb2165` (135 source-check plus 54 display
queries). The final diagnostic-display boundary/options test is GREEN
`vrun-c6fde60d-d8e2-4e44-a6ad-ab65d43823b4`. In the same final run, both
TypeLiteral tests reached Internal failures at computed-name operands:
`computedStable` and `computedBeforeAnnotation`. Earlier loop cases passing is
not proof of later cases that were never reached.

The collector carried its enclosing type context into runtime expressions.
`value_reference` correctly asked for a runtime attachment, so it rejected the
mistagged attachment rather than performing a second lookup. The staged repair
resets that context at the common collector expression entry, using the existing
native `IsExpression` owner. TypeReference, TypeQuery and ImportType names are
captured directly by their entity-name branches and preserve their explicit
context. Binding declaration names also keep their own capture owner. This
covers computed binding keys, declaration/parameter initializers and nested
expression trees without a new parent map or a caller-maintained reset list.

`fixtures/types/native-reference-contexts` and
`tests/type_reference_contexts.rs` select the actual operands, independently of
late-member synthesis. Native capture GREEN
`vrun-344ea222-8759-49a4-9904-c34783fb44d8`, followed by fully asserted GREEN
`vrun-896f8236-1594-412f-aad8-a42ca252ca2d`, establishes 15 exact outcomes:
property/method/interface/class/object computed names, binding-pattern keys,
property and parameter initializers, ordinary/nested TypeQuery, type-only import
expression/query roles, and missing computed/initializer names. The final tests
require healthy literal results or the original located UnresolvedSymbol,
never the former Internal attachment failure. A bounded counterfactual removing
only the common context reset remains to be recorded.
Final reference-context tests are now GREEN
`vrun-e14e787c-9314-46dc-8660-865f725ce529` (both tests). Stage8 diagnostic run
`vrun-83708015-4823-4337-b8df-f13b54346ac5` passes the diagnostic-display test
and the entire TypeLiteral original-failure-order test. The healthy TypeLiteral
matrix now reaches the explicit late-member table dependency at `computedStable`
instead of the former Internal attachment failure; later healthy loop cases
have not been reached in that run.

The rendered diagnostic result, reporting initializer relation, late symbols
and annotation-reuse dependencies remain required owner work; their explicit
failures are not accepted final behavior. See the
[diagnostic display plan](types-native-diagnostic-display-plan.md) for its exact
byte/result and global-identity migration contracts.
