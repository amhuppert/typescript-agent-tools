# Native heritage implementation plan

Pinned TypeScript Go 7.0.2 commit 2bd066d87f5bafd315be9f40889d0a60b9e58e0b.
This is the staged implementation design. The canonical property-symbol consumer and failure-provenance work are established; heritage implementation and full-profile delivery remain open. The operating envelope is one request-local evaluator over an immutable accepted Project, with the pinned compiler demand order and recovery provenance defining query behavior.

## Operations and boundaries

- Source owns the exact selected class declaration and ordered interface declarations, actual extends nodes, entity-name eligibility, expression and written type-argument recipes. Binding retains canonical symbol flags (Class and Interface independently), actual type/value references, and complete own member table presence. No spelling-based base binding.
- getDeclaredTypeOfClassOrInterface (checker17235) installs actual target before local parameters and isThislessInterface (17269). Model target_this_parameter must observe native nil while target allocation is incomplete without exposing a fake completed TypeView.
- isThislessInterface checks ContainsThis before each interface declaration's ordered entity-name bases; unresolved entity name returns false for the native ignoreErrors lookup, not a generic swallowed binding failure. A non-interface base also returns false. NotDemanded/NotRetained and other unavailable association states remain failures. Non-entity expressions are ignored only by this syntax-specific predicate.
- getBaseTypes (19072) owns a real ResolvedBaseTypes resolution-stack key, ordered partial native base list, and baseTypesResolved installed fact. It resolves class bases before interface bases on a merged symbol, resets MembersResolved after completion, and handles native recursive work separately from mere in-progress cache markers.
- resolveBaseTypesOfInterface (19403) consumes ordered reduced base types, validates base kind (19445), rejects a recursive target through hasBaseType (19460), and preserves inherited base order.
- resolveObjectTypeMembers (19011) acquires declared members, instantiates properties before call/construct/index signatures if needed, then always gets bases. With bases, installs own inventory before inherited traversal, marks UnresolvedMembers, instantiates each base with the same mapper then rebinds its this argument, merges properties and signatures, applies exact key-TypeId index precedence, clears UnresolvedMembers and installs final inventory.
- addInheritedMembers (19503) preserves the actual PropertySymbolId; it skips static private identifiers, and existing own value members win by native semantic key. TypeKey display origins never supply private identity.
- getNamedMembers (21954) partitions actual ValueDeclaration containment by the current class/interface's declaration ranges, then compareSymbols sorts both own and inherited partitions. Reuse the shared sort facts algebra is adding, rather than infer order from append position or display origins.
- getSingleBaseForNonAugmentingSubtype (27916) marks IdenticalBaseTypeCalculated before checking, admits simple class base names only, gets native base list before checking the complete own member table, instantiates regular parameters and explicit this arguments, and caches its real optional result.

## Class branch dependencies in the same full scope

- Class anonymous static value factory getTypeOfFuncClassEnumModuleWorker (16825), actual class symbol association, and native intersection with getBaseTypeVariableOfClass (16850).
- getBaseConstructorTypeOfClass (16870) has a distinct BaseConstructorType stack key/cache. Evaluate the actual extends expression, resolve object/intersection static inventory to detect cycles, check native constructor/mixin validity, preserve null/any and recovery distinctions.
- resolveBaseTypesOfClass (19125): direct canonical Class constructor path applies type arguments to its declared instance; other class-like constructors use first native matching construct signature return; reduce, validate, and detect native base cycles.
- resolveAnonymousTypeMembers class branch (20587) installs own export inventory first, inherits static members from actual base constructor excluding static private names, then indexes and explicit/default constructor signatures. Namespace/class merges retain complete native exports.
- Explicit/default constructor signature factories belong to signature agent; coordinated calls must preserve fresh signature identity, target/mapper and actual declarations. Class instance canonical member table remains distinct from static export table.
- Parameter properties, inherited private identifiers, class/interface merges and dynamic/nonentity extends retain exact source/binder associations; no guessed members or partial-factory healthy completion.

## Compiler controls already measured

Scratch .cc/temp/types-heritage-forced-controls/answers.json contains eight fresh public/native pairs, strict true, noLib. Every final call returns literal chosen. Baseline noLib codes2318 are independent of these cases.

A/B each contain marker:number and extend Shared; Shared has self:this. Their own marker prevents getSingleBaseForNonAugmentingSubtype from collapsing both references directly to Shared.

- latent:Missing: no unresolved-name diagnostic; inherited property is preserved by MapsThisOnly/isThisless, and equal actual symbol skips type demand.
- latent:(Missing), latent:Missing|never: unresolved-name diagnostic is emitted in selected call's native trace; no thisless identity shortcut, original missing property read is consumed.
- latent():Missing, latent(x:Missing):void, get latent():Missing: no unresolved-name diagnostic, safe syntax identity shortcuts.
- latent():(Missing): no unresolved-name diagnostic despite fresh property, because native method/signature identity paths can still avoid return-type demand. Do not assert missing failure from syntax alone.
- Independent A/B declarations with latent:Missing: unresolved-name diagnostic; their distinct property symbols cannot shortcut.
- Empty A/B controls all skip Missing even parenthesized, due equivalent-base normalization; retain as separate nonaugmenting-base control, not evidence for instantiateSymbol.

## Final RED and implementation order after current regression checkpoint

1. Author tests/type_heritage.rs with Shared controls. Selected overload final literal must be chosen for safe forms; visited Missing variants assert exact InvalidSemantic UnresolvedSymbol declaration location. Test strict false/true and exact optional true based fresh native observations.
2. Add graph final controls for base property/signature/index precedence, multiple/diamond/merged bases, own-before-inherited order, recursive this, actual inherited origins, nested generic specialization, and native no-this bases. Native control first for each distinction.
3. Implement interface/base-target machinery and shared member merge, then extend class static/instance branches and corresponding native controls. These are coordinated stages, not partial-profile delivery claims.
4. Add mapped/transient getter/read-warm/setter cases and full class/private controls; peer review Source metadata and cache/re-entry protocol before root registered scoped checks. Full T profile remains delivery requirement.

## Open evidence questions

- Native circular/invalid base paths often diagnose and leave an empty base list instead of returning errorType. Measure selected-demand recovery effects before deciding product InvalidSemantic versus healthy non-consumption; do not map all diagnostics to failure or all empty lists to absence.
- Full inherited private key representation must be semantic identity separate from public TypeKey origins and must integrate root property lookup and relation target-property enumeration.
- Member ordering containment uses actual native node positions/ranges (including native containment behavior); token anchors are not interchangeable.

## Maintained controls and observation correction

The combined source in `fixtures/types/native-heritage/cases.ts` is retained by both `tools/type-native-heritage-oracle.test.mjs` and `tests/type_heritage.rs`. Every A/B in the symbol-identity controls has an own marker, while the separate empty-parent control deliberately exercises equivalent-base normalization. Strict, loose-null and exact-optional configurations are all covered.

The first maintained baseline `vrun-b98ca746-0b81-4955-9a7a-1267050ae2d1` failed because it assumed unresolved names always emit diagnostic 2304. A six-input native diagnostic probe isolated original names, renamed names, blank-line padding, preceding unrelated declarations, renamed plus preceding declarations, and combined source. Renaming `Missing` to `MissingParenthesized` selected diagnostic 2552 (spelling suggestion); the relation observations and consumed annotation were unchanged. This was an expectation error, not a source-order/cache behavior difference.

The compiler hook now observes the actual return of `getNonMissingTypeOfSymbol`, recording its first symbol name and instantiated flag only when native `isErrorType` holds. It performs no extra type query and allocates no symbol identity. The exact native function body is guarded by `replaceExactly`, alongside the existing pinned source/version guards. Pointer equality with `c.errorType` alone was insufficient: checker.go `isErrorType` also recognizes alias-bearing Any manufactured for unresolved references. The failed pointer-only control is `vrun-cb81431f-914c-4b65-bad0-f386a5b45fee`; the corrected baseline is GREEN: `vrun-26bcdc79-25b9-4cd0-a4dd-f8eb3be93435`, two tests executed, two passed, zero skipped (27 case/profile observations).

Native recovery must stay causal: the selected root remains the valid literal `"chosen"`, but the visited target `latent` dependency returns native error recovery in parenthesized, union and independent-symbol variants. Safe thisless and nonaugmenting variants must not consume that dependency. The product's `InvalidSemantic/UnresolvedSymbol` expectation is justified by that dependency, not the unrelated noLib diagnostics or root-only error status.

## Identity owner follow-through

Binding's native declaration groups may identify one actual symbol across distinct lexical `BoundSymbol` values. Algebra will expose a typed equality/hash `NativeSymbolIdentity` from the existing retained native group owner. The property arena and recursion identity must consume that identity, without numeric ordering, display names, origins, or serialization IDs. Registration becomes fallible when actual native association is unavailable. Compatible-role final controls must establish this distinction before claiming full native identity support.

For member order, inherited transient properties retain their original canonical symbol declaration/name facts. `isDeclarationContainedBy` consumes the actual ValueDeclaration span and container declaration spans; the native operation compares ranges without a source-file guard. Source/binding provide this same bounded observation, and shared compareSymbols owns ordering within the own/inherited partitions.

## First interface implementation checkpoint

The final-query tests established genuine RED `vrun-04ef7e88-e5d8-4b18-b41e-60b23bbc25f8`: three tests failed at the unimplemented isThislessInterface base dependency. The staged implementation now installs targets before thisless analysis, records optional target thisType during allocation, resolves interface bases through a native BaseTypes resolution key/partial list, resets MembersResolved after base completion, and always acquires bases before publishing the final member inventory. Inherited property tokens survive unchanged; own declarations sort ahead of inherited declarations using actual native containment and the shared compareSymbols owner. Nonaugmenting subtype normalization checks the full own table, including reserved signature members. Product failures retain their causal error, while native diagnosed scalar/recursive base skips preserve the valid partial list.

The property arena and recursion identities now use `NativeSymbolIdentity`, including the explicit empty-TypeLiteral synthetic symbol. Registration is fallible; unavailable identity cannot become a fresh invented symbol. Type.symbol equality does not acquire declaration/name ordering facts. Actual object-literal factory symbols now answer couldContainTypeVariables through Model's retained bound association. Existing lexical-role recursion facades were removed.

Root adversarial review of checker17235/19011/19072/19403/27916 found no blocker for admitted interfaces. Class constructor/base-expression acquisition, reduced-intersection bases, the Any-base index sentinel, semantic private keys, and late/compound inventory scheduling remain explicit next dependencies. The first implementation checkpoint is not full heritage or full T-profile delivery. Post-change Rust verification is pending the coordinated freeze.

## Next class factory stage

Keep the native operation boundaries already present: the target/base owner
adds the class branch and its distinct base-constructor cache; the anonymous
owner adds the actual class static inventory. A second class-shaped object
builder would duplicate member instantiation, symbol identity and signature
ordering, so it is rejected. Binding projects the actual exports table
separately from the instance members table, and Source supplies the selected
class declaration and extends expression/type-argument recipes. The existing
getTypeOfSymbol value dispatcher admits the class family through this owner.

`getTypeOfFuncClassEnumModuleWorker` creates the actual anonymous class type
before asking `getBaseTypeVariableOfClass`; its caller installs the final
value type only when that operation completes. Do not install the raw
anonymous value early to conceal a base-expression cycle. A type-variable
base contributes to the native intersection result rather than becoming an
invented instance member table.

`getBaseConstructorTypeOfClass` distinguishes no extends (undefined), extends
null, Any, actual constructor type and consumed error recovery. Its own
resolution-stack property ends cycle search only when installed. It checks
the extends value expression, forces structured members for object or
intersection results, pops the frame, and only then checks constructor/mixin
validity. Native recovery already installed by recursive work wins over an
outer assignment. Product dependency failure must retain its original cause.

The static anonymous inventory installs own exports before following the
base constructor. Inherited properties are merged through addInheritedMembers
and preserve their actual property tokens. Indexes are established before
explicit/default constructor signatures, so a qualified typeof in a signature
can observe the installed members. Native temporary empty signature lists
are not a generic completed-inventory shortcut.

The signature owner will add eager Known(classType) return associations and
fresh default constructor factories. A class with no base signatures gets a
synthetic declaration-less signature using its actual local type parameter
IDs. With base signatures, getDefaultConstructSignatures acquires written
arguments only after the nonempty-signature branch, filters by native minimum
and maximum argument counts, and calls fresh createSignatureInstantiation or
cloneSignature. The result preserves its actual declaration/target/mapper,
replaces local type parameters and return type, and sets or clears Abstract
from the derived class. The cached getSignatureInstantiation operation is
not interchangeable with this fresh factory.

Controls for this stage must distinguish instance versus static inheritance,
explicit versus default constructors, generic inherited constructor arguments,
abstract versus concrete defaults, actual Class versus class-like constructor
bases, no extends versus null versus Any, and merged class/interface flags.
Missing/invalid/circular bases need actual consumed recovery observations;
diagnostic presence and a healthy displayed result alone cannot decide the
product outcome. Private-name inheritance and namespace/class exports also
remain required controls once their exact factory inputs are admitted.
