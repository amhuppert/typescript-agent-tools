# Native modifier accessibility and access-flow continuation

This is the next required member slice, planned while the stage30 compiler
checkpoint is frozen. The operating envelope remains one captured Project and
one request-local evaluator. TypeScript 7.0.2 defines operations; the documented
T profile still refuses a final flow-sensitive result with `flow_required`.
This plan does not change that profile or call a diagnostic-only denial an
error-type recovery.

## One accessibility owner

Extend `checkPropertyAccessibilityAtLocation` behind the current reporting and
suggestion predicates, carrying the actual containing TypeId and optional
actual diagnostic node. The current worker discards containingType, which is
required by the protected receiver test and diagnostic fallback. A second
suggestion-only traversal would duplicate protected/base decisions; a caller
assembled list of checks would expose ordering and error-node rules. One deep
operation returning `NativeResult<bool>` avoids both.

Preserve the native order (checker11755–11875):

1. Writing-sensitive declaration modifiers; abstract-super and instance-field
   super exclusions; abstract nonmethod access during initialization.
2. Public returns true. Private obtains the late/merged Parent, finds its first
   actual ClassLike declaration, and tests enclosing class identity. On denial,
   reporting alone acquires symbol display then declaring-class type/display.
3. Protected super returns true. Otherwise walk enclosing classes inner to
   outer, acquiring each actual declared class type and testing all protected
   constituent declarations. Stop at the first class deriving from each.
4. With no enclosing class match, acquire the explicit this-parameter annotation
   (then its constraint if a parameter), or the existing contextual-this owner
   only when that explicit result is absent. Keep the resulting reference target
   identity. Static properties cannot use this fallback, but the fallback reads
   still occur before the static/no-class rejection.
5. A static property then succeeds. For instance access, an actual this type
   uses getConstraintOfTypeParameter; another parameter uses getBaseConstraint.
   Nil means false. Otherwise hasBaseType(containing,enclosing) decides. Reporting
   acquires symbol, enclosing-class and containing-type displays in that order;
   a nil containing type does not display.

False is ordinary data. Checked access ignores the bool and proceeds to
readonly/value/flow as native does. Completion-suggestion filtering consumes it
as eligibility and must perform no reporting-only displays. Source owns class,
this-container, declaration-name/error-node and ancestry syntax, using existing
`GetContainingClass`, `GetThisContainer`, and first-class-declaration operations.
No class spelling or lexical merge-group ancestry can replace actual identity.

## Shared operations and coordination

- Add `for_each_property_completion(symbol, consumer, callback)` returning
  NativeResult<bool>, with callback of the same return family. Synthetic
  constituent lookup and recursive callback preserve first-true short circuit.
  Keep the admission adapter; route accessibility and relation override callers
  through the complete owner.
- Add `property_declaring_class_completion(symbol, consumer)` returning
  NativeResult<Option<TypeId>>. Observe raw Parent and its Class flag first;
  only then acquire getParentOfSymbol (late then merged) and declared type.
  Existing property_declaring_class becomes its admission adapter.
- Reuse Source::first_class_declaration and the existing
  calls/construct.rs::reference_class_declaration adapter after making that
  same-question operation available; do not create another declaration scan.
- `isClassDerivedFromDeclaringClasses` is distinct from the relation question
  `isPropertyInClassDerivedFrom`: the former requires every protected target
  constituent, the latter searches any source constituent. Both reuse the
  complete forEachProperty/declaring-class/hasBaseType owners.
- Algebra owns actual header declarations, raw Parent and late/merged Parent
  facade migration. The current guarded legacy bridge is not evidence that
  those producers are complete. Structured released the two exact relation
  override/derived helper regions for this migration.
- Existing contextual_this_parameter_type_completion, parameter constraint,
  base constraint, declared type and has_base_type_completion are already
  available. No additional contextual inference implementation is planned here.

## Access-flow owner

Complete getFlowTypeOfAccessExpression at its existing owner rather than
leaving a blanket automatic-property rejection before native setup:

- Definite assignment removes missing first. Nonvariable/nonproperty/nonaccessor
  symbols and nonoptional methods return before flow.
- An actual autoType calls getFlowTypeOfProperty: start at undefined; only with
  ValueDeclaration and (!isAutoTypedProperty || ambient) acquire the first base
  property type; then enter the shared native flow interpreter with autoType
  declared and that exact initial type. Assignment-created property metadata
  remains the actual binder prerequisite, never an invented class association.
- Ordinary values pass the shared getNarrowableTypeForReference, strict
  initialization setup, optionality, flow-reference operation and final
  base-literal operation. NoInfer now has an actual Substitution producer: only
  isNoInferType unwraps its base; another substitution is not blanket-refused.
- A used-before-assignment result follows the existing documented admission
  contract. It cannot become a healthy declared type solely because native emits
  a diagnostic and later returns that type. Conversely, exact Start/absent-flow
  and final identity cases must not be refused before setup.
- Complete checkPropertyNotUsedBeforeDeclaration and its static-initialization
  pseudo-reference acquisitions through the same read owner, preserving native
  declaration/type/flow order before access metadata installation.

The current read interpreter is Result-only and stores FlowType without evidence
in invocation and Shared caches; loops store clean TypeIds separately. A clean
wrapper would lose recovered child reads. The coherent transport is
NativeCompletion<FlowType> on actual completed invocation/Shared/loop slots,
keeping each key's own acquired prefix. Active loop TypeId candidates remain
copied native values; an assumed reentry does not replay unfinished evidence.
Key construction stays before lookup. Depth-2000 checks stay before Shared
lookup; loop/expression scopes restore on every interruption. This is a
continuation migration of the existing interpreter, not a session journal and
not a second flow engine. Its breadth must be included when scheduling the
source window; access cannot claim completion while an admitted child drops
recovery at this boundary.

The same tail audit also found isAssignmentToReadonlyEntity currently misses
the raw receiver-symbol/ModuleExports early exemption and final NamespaceImport
restriction. Complete this existing decision with its source receiver and alias
questions; readonly symbol, constructor exemption and namespace import are
separate native branches, not one property flag.

## Proof gates

Keep native fixtures below30 roots per profile. Proposed independent groups:

- Modifier access: private own/peer/nested/outside/derived/static; protected
  derived-vs-base receivers, nested class, static, super, explicit and contextual
  this, generic/this constraints, composite constituents, getter/read versus
  setter/write modifier selection; abstract super/field/constructor controls.
- Reporting continuation: inaccessible property with a lazy missing annotation,
  generic class display dependencies, first enclosing-class failure before a
  later successful class, first-failing protected constituent and first-success
  base short circuits; a misspelled access exercises no-error suggestion filtering.
- Flow: absent/Start identity, readonly constructor versus outside/namespace
  import assignment, strict initialization on/off, annotated/uninitialized,
  inherited base initial type, auto constructor/method controls, changed branch
  versus rejoined identity, NoInfer and generic-constraint context positions,
  recovered child followed by a later native read, and whole-failure cleanup.

Expected summaries/diagnostic membership come from both ordinary native API and
hook. Ordered dependencies come from the pinned producer trace, with zero-demand
observers only if needed. Accessibility denial normally keeps the property type;
only actual fallback producers get recovery markers. Final Project tests must
assert healthy summaries, exact failure category/location/ordered causes, and
explicit FlowRequired where the established profile requires it. Reverse query
order proves isolation only; warm-cache evidence needs two real acquisitions in
one checker query. No validators or frozen inputs have been modified for this
planning step.
