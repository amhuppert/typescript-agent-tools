# Canonical Array dependency and JSON array core

This next required increment follows the Ousterhout designing route. Its
operating envelope is a request over one immutable successfully loaded Project.
It builds on the scalar/object and pending null lane, preserving full array,
contextual/object relation, presentation, evaluator and resource obligations.
No library or member view is inferred from an option or a summary string.

## The dependency the factory actually consumes

Pinned `initializeTypeCaches` eagerly asks `getGlobalType("Array", 1, true)`.
`getGlobalType` resolves a global Type-meaning symbol, requires class/interface
flags and exactly one declared type parameter, and otherwise reports a precise
missing/kind/arity error before returning `emptyGenericType`.
`createArrayTypeEx` consumes that target; `createTypeFromGenericGlobalType`
returns `emptyObjectType` for the fallback identity. Native noLib `{}` with a
false root error flag is therefore consumed recovery, not ordinary product
admission. The exact source chain and same-checker already-accumulated diagnostic
control are archived in `json-native-arrays/evidence`.

The decision must use the canonical retained global table. noLib does not decide
presence: user scripts or declare-global blocks may establish Array. Type/value
meaning separation, external module boundaries and canonical merges remain the
existing binding owner's questions. Target lookup must distinguish missing,
wrong class/interface kind, wrong arity and a genuine target; when consumed,
recovery outcomes become InvalidSemantic with the original cause. It must not
substitute Unsupported, ordinary `{}`, genuine any or a whole-file diagnostic
pass. Unproved target construction remains an explicit pending semantic failure.

## Proposed owner boundary

The retained binder exposes a complete typed global-Type operation over its
existing Store table and canonical declarations. Class/interface eligibility
comes from the existing symbol meanings/flags decision. Local type parameters
come from the compiler `getLocalTypeParametersOfClassOrInterfaceOrTypeAlias`
question: union canonical parameter identities from own class/interface/type
alias declaration lists, preserving native deduplication across merged lists.
The existing linker parameter-table read is reused or moved whole into a shared
Store operation; no second name lookup or guessed first-declaration arity is
added. Mechanical visibility/wiring remains coordinated with the binder owner.

Implementation now retains each own typed declaration association on the Store
symbol through its actual bind/merge operations. A captured-file-order scan
failed the final compiler-backed script-before-augmentation cause test and was
removed; lexical sites are not that association/order question. One complete
private operation serves declared/value/global acquisition with separate kind
decisions. Storage cost and measured proof limits are recorded in
`types-json-native-evidence.md`.

The source owner supplies the genuine declared target/member recipe. The shared
Evaluator owns establishing its actual members and type-argument substitution.
The shared Model consumes an established canonical Array target, element type,
readonly flag and genuine member recipe/view. It owns type identity/interning,
union reduction and widening. JSON calls these operations; it does not duplicate
assignability, union ordering, optional slot creation or generic instantiation.
The presenter consumes established/lazy views through the same evaluator rather
than demanding JSON syntax or assuming missing member inventory is empty.

## Behavior-first sequence

1. Pin fresh native controls for missing global Array, wrong kind, zero/two
   parameters, actual noLib user Array declarations and valid standard library
   target. Include value-only and module-local Array names, merged declarations
   and declare-global variants according to the native lookup rule. Preserve
   every option state and distinguish raw input-diagnostic failures from admitted
   semantic questions.
2. Add genuine final `Project::type_of` REDs to `tests/type_json.rs`: consumed
   missing/kind/arity recovery must return InvalidSemantic at its canonical
   dependency with exact cause. A selected scalar beside arrays must demand the
   entire module, including discarded duplicate initializers.
3. Establish target facts through the retained binding product and source owner,
   then route all actual factory callers through the one Model/Evaluator target
   operation. No compiler process, disk reread or JSON-only global check is used.
4. Admit primitive-element arrays through complete native union/subtype and
   contextual widening operations, then expand to recursive/unlike-object
   arrays and missing optional slots. Empty arrays use native strict-null
   undefined-widening/implicit-never input, not an arbitrary any fallback.
5. Exercise actual declared noLib and standard-library member facts. A fixture
   with an actually empty valid Array interface can pin a genuinely empty view;
   it cannot stand in for standard library members or arbitrary user generics.
   Full standard-library members must remain lazily established by the general
   evaluator. Resource/graph omissions remain explicit and independently tested.

The first noLib raw array controls and default-library controls establish the
classification difference, not this implementation. The Model Array interface,
canonical lookup and shared member/evaluator operations are coordinated with
agents 1/2 before source edits. The representation gate, complete JSON typing
and full semantic/resource gates remain open until their final journeys pass.

The consumed-recovery increment has genuine public RED/GREEN receipts and a
seventh adversarial ordering RED/GREEN, recorded in
`types-json-native-evidence.md`. It does not instantiate genuine Array targets.
The next shared prerequisite is the canonical declared TypeParameter operation
and a general generic Class/Interface recipe, including merged declarations,
actual substitution and lazy members. That same owner serves Array globals and
ordinary generic/signature questions. A specially guessed empty Array view or
an Array-local spelling map would bypass those obligations and is not planned.
