# Native generic alias lifecycle

The operating envelope is the existing immutable Project and request-local
Evaluator. The remaining ordering test consumes actual alias arguments, so
alias construction and substitution belong in the required ordering scope.

TypeScript 7.0.2 owns these decisions in getDeclaredTypeOfTypeAlias,
getTypeFromTypeAliasReference, getTypeAliasInstantiation,
getObjectTypeInstantiation and instantiateTypeWithAlias. Evaluation owns their
lazy questions; Model retains established alias keys and mapper identities;
Source and Binding retain actual declaration context and parameter identities.

An alias-specific substitution engine would duplicate reference, composite,
anonymous and inference scheduling. Instead the existing instantiate_type
operation gains an explicit-alias entry, while the common entry supplies nil.
Its active-mapper cache key becomes the native type-plus-alias key. The new
aliases module owns declaration parameter lists and alias instantiation caches.
Anonymous instantiation owns effective outer arguments, explicit alias override,
target alias preservation, and actual flag propagation in one operation.

Declaration evaluation pops the DeclaredType resolution frame before acquiring
local parameter identities and seeding the raw-parameter cache entry. Alias
references acquire that declared type before arity checking. Arity failure
precedes child argument evaluation. Valid references retain native enclosing
alias context and imported alias identity; cache lookup precedes missing-default
acquisition. Fully supplied arguments never acquire defaults. Acquired invalid
defaults preserve their original dependency failure even when the compiler's
eventual raw root discards the argument. Unsupplied errorType mapper slots are
separate identity-only placeholders until actually substituted.

getOuterTypeParameters is shared by anonymous methods and TypeLiterals. It
walks actual enclosing declaration nodes from outer to inner, appends canonical
own parameters, then acquires a class/interface this parameter. Contextual
function, mapped and conditional contexts remain required distinct operations;
until their actual factories exist they retain precise dependencies. The
possibly-referenced filter uses the same Source owner for methods and literals;
generic alias RHS literals skip that filter exactly when alias arguments exist.
Anonymous cache keys include effective arguments, actual alias key and native
SingleSignature state. Effective outer arguments use combinedMapper.Map, not
arbitrary recursive instantiation. Existing member and property-symbol owners
then lazily acquire instantiated children through that retained mapper.

Alias summaries serialize actual argument identities through the established
bounded summary traversal. No printed name, public location or origin supplies
an identity or cache key.

Native controls: generic alias lifecycle/default/import/recursive/lazy property
tests passed twice with zero skips, vrun-be20c3cf-cfcd-4a77-80bb-54aca9aa5ff1.
Final Project tests are genuine RED at
vrun-42ad1dab-1006-48af-8318-5863790c7035. Order6 currently has five GREEN and
one generic-alias RED, vrun-ef9aed32-2bbf-4f29-9208-83508a09cdcd. Exports has
passed its default-name cases and reaches a parameter selection-role RED,
vrun-cd940a69-3d2e-4fcb-85a5-389bc573dcc6. Fresh hook controls establish actual
default recovery acquisition separately from raw root health before final
expectations are adjusted.

The bounded hook control is GREEN at
vrun-d1bcb749-d233-4dbe-a98a-9f0b9cd621f0 (six strict/loose cases, one test,
zero skips). Ignore with an omitted invalid default acquires recovery even
though its raw result is number; an explicitly supplied argument skips that
default. The final Project expectation therefore requires InvalidSemantic for
the omitted case, under the existing acquired-dependency contract. The ordinary
dependent default remains healthy. This correction changes no implementation
boundary and does not infer health from a raw root summary.

Pre-validation peer review confirmed the admitted raw-key/default/mapper
schedule, and identified two required corrections for the next source phase.
getObjectTypeInstantiation must use the actual symbol's first declaration and
all its declarations for filtering. A class/interface parameter can share a
native symbol with a same-named Method across disjoint TYPE/VALUE meanings;
the parameter's constraint/default can participate in the reference scan even
when no method signature mentions it. BoundMember's value declaration and
member-only list are insufficient observations. Use Model's real bound-symbol
association and Binding's native ordered declaration list.

Those corrections are now staged. CapturedSyntax attaches native declaration
entries through the existing binder declaration-anchor owner, including
anonymous TypeLiteral, FunctionType and ConstructorType roles. Anonymous outer
parameter acquisition reads Model's actual symbol and the complete native
declaration list before applying its family-specific reference filter.
The fresh native context control is GREEN at
vrun-d0022b4e-81d2-40e2-ac13-9c4f8c5551c3, proving the merged parameter/method
symbol, its declaration order, distinct method instantiation identities and
unnamed callable/constructor alias summaries. The two Carrier references keep
the merged B parameter identical and vary only A; this isolates the reference
inside B's constraint from B's own multiple-declaration filter. Final contexts2 awaits its
coordinated baseline and completed-implementation runs. The previous alias4
checkpoint vrun-50ee9418-fe95-46cb-976d-212abba52828 compiled and had one pass,
three failures at the anonymous declaration attachment; it supplies the
actual-syntax regression baseline.

getTypeFromTypeReference now owns a captured file/Site cache for both
TypeReference and ExpressionWithTypeArguments, matching the native shared
dispatch. Imported aliases acquire argument nodes in two native steps; neither
an alias factory cache nor a universal invented check_type cache substitutes
for that node question. Completed failures retain their original provenance.
The native cache hook is GREEN at
vrun-612c0ba0-fbf6-4b5c-b10e-0be2c6f8e343: three tests, zero skips. Three nested
imported references produce exactly three cache misses and two hits before
summary in both baseline and cold histories. The same run revalidates the
acquired-default recovery control after making its synthetic-parameter symbol
observation nil-safe. Final Project coverage includes the nested imported root;
the hook establishes the native acquisition counts, not a claim that the public
Project API exposes internal counters.

The stage 6 repair extends `resolveTypeReferenceName(...Alias, true)` as its
own symbol question. TYPE lookup cannot answer it: a direct inner type does
not stop Alias-only lookup, and an export entry can carry an alias beside a
declaration meaning. The existing Linker scope/table traversal now serves both
questions. Export, namespace-member and star-export operations carry the actual
selected raw alias alongside their resolved meanings, including memoized results.
When equivalent star routes meet, the first selected export symbol remains the
raw identity even if a later route has a different alias declaration. Navigation
continues to project the resolved target.

Two alternatives were considered: replaying a separate export traversal in the
semantic facade, or retaining the raw symbol in the existing lookup result.
Retention keeps export precedence, cycles and ambiguity in one owner. Raw lookup
does not resolve the selected alias. A final semantic-retention phase completes
its target through the existing force/cycle owner, after navigation projection;
errors remain retained failures for the later evaluator question. Source alias
declaration facts retain the actual owning syntax kind and full native span,
separately from the name/diagnostic anchor, for raw-symbol ancestry observations.

Pinned controls are GREEN at
`vrun-3d72ad5b-27fc-469c-a4ad-199e196979b0` (39 observations across strict,
loose and exact-optional profiles, one test, zero skips). A zero-demand hook
observes the actual Alias-only result without another checker call. The controls
cover direct, named and type-only exports, nested namespace exports, duplicate
star-route order, a star cycle, export assignment, namespace import-equals,
lexical TYPE shadowing and an external alias target chain. The shadow control
serializes the outer imported alias while retaining the inner type's properties.
The missing outer alias target is selected and resolved even though native
returns the healthy inner alias type; the product must preserve that consumed
lookup failure. Stage 6 existing template-intersection tests are GREEN at
`vrun-2eeab6d8-b8ca-4e56-81f1-d79d2f2fb86e`. New final qualified-alias tests and
independent counterfactuals are awaiting their coordinated verdict.

Remaining proof obligations are explicit: a non-TypeAlias JSON value selected
by the raw Alias question is a native negative, not an unsupported type factory;
and an unsearched dependency can contain the independent raw-alias question
inside a deferred property type. Those controls must prove the complete retained
lookup boundary. A defaulted generic reexport is the planned final discriminator
for the raw empty alias-argument list, independently of lexical shadowing.


The pending-resolution boundary is now a shared operation rather than another
Linker copy. `binding/resolution.rs` owns the complete existing task stack,
fill-edge cycle cuts, lexical table/meaning gates, exports, star precedence,
qualified members and JSON value lookup. Its immutable inputs adapt the original
collected facts and the retained binding product. Load keeps its dense caches and
merge-invalidation journal. A semantic request keeps a sparse overlay over the
completed answers; unknown tasks alone enter the same task machine. Source owns
a cloneable request view whose handles share one cache. No query mutates the
Store, the completed load states or the navigation index.

The alternatives were eager completion of every dependency reference at load,
and a semantic-only lexical/export replay. The former violates the retained
NotDemanded contract and makes unrelated source cost depend on unused code. The
latter creates two owners for cycles, star precedence and meaning selection.
The chosen extraction adds no new binding or parsing phase. The temporary
load-only `complete_type_reference_aliases` phase is removed: Alias-only lookup
retains the actual selected symbol, and its target resolves only when the native
alias question consumes it. The semantic failure enum no longer exposes
NotDemanded, since a queried retained task now resolves; the immutable load
product continues to distinguish NotDemanded from unresolved.

Native boundaries are GREEN at
`vrun-3b14610e-2658-47c3-a853-2a0be2907971` (24 observations, three profiles,
one test, zero skips). Direct generic references fill defaults in their alias
arguments, while actual raw import aliases retain only supplied arguments:
`Defaulted<string>` versus `Defaulted`, and `Pair<boolean, number>` versus
`Pair<boolean>`. A selected JSON value alias is a known non-TypeAlias negative.
An unsearched interface's member resolves the actual inner generic type while
retaining the outer imported alias for display. A missing sibling member stays
unconsumed at depth zero and produces an actual error type when expanded.
Final baseline `vrun-95896001-8315-4c52-9551-0850882c4b98` is genuine RED:
one default-argument test passed; JSON-negative and two unsearched-member tests
failed. Final tests also compare navigation before and after semantic queries.
The retained-load test now asks an unrelated final type question before proving
that the unused dependency reference and alias remain NotDemanded.

For raw TypeQuery declaration ancestry, getResolvedSymbol is a symbol question,
not a value-type question, but native alias eligibility can resolve a target:
checker13814 passes Value|ExportValue to resolveName; createNameResolver1466 uses
getSymbol2162; a matching direct flag returns immediately, otherwise
getSymbolFlags16276 follows aliases and maps unknown target flags to All.
TypeQuery's captured first identifier uses exactly VALUE in `Path::VALUE`, so
this shared resolver never asks extra TYPE/Namespace meanings for that question.
Failed target eligibility preserves the actual raw Alias and its declarations;
only a later value-type consumer turns its target failure into an invalid result.
Peer review found no blocker in sparse-cache override/nil-origin handling,
non-Done reset, load invalidation, raw/value separation or Source borrowing.
Full navigation/loading compatibility and source-cost measurements remain
required after the coordinated compile; no behavioral success is inferred from
this uncompiled stage.
