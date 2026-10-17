# Native diagnostic display acquisition

The TypeLiteral controls establish a semantic dependency, not a request for more
product output. Native run `vrun-04ed0764-0413-45a0-a0c9-0b466424ac94`
checks 45 roots in strict, loose and exact-optional modes. Given
`type DisplayBox = { payload: { deep: MissingDiagnosticDisplay } }`, a checked
type argument containing `value: DisplayBox["payload"]` consumes no missing-name
diagnostic by itself. A matching incompatible index or secondary property
declaration causes native `TypeToString` to acquire that missing type. A
nonmatching pattern index remains healthy.

The shared interface is
`check_diagnostic_type_display(TypeId, &Location) -> Result<(), DemandError>`.
It performs the native type-to-node acquisition schedule without publishing the
diagnostic text. TypeLiteral compatibility and instantiation-expression error
selection invoke this same operation. A false relation remains diagnostic-only;
any type failure consumed while constructing its diagnostic remains a failure.

This request-local owner differs from public summary rendering. Public summaries
have product byte/depth limits and convert unsupported child failures into
partial output. Neither policy belongs in `Checker.TypeToString`. Calling the
public renderer or walking every reachable type would both change healthy
answers. The owner belongs in `evaluation/native_display.rs`, alongside the
other native operations. It reuses property serialization, signature metadata,
tuple display arguments, conditional binders, symbol metadata and resolution
operations rather than creating parallel semantic factories.

The initial entry is exactly `printer.go` `TypeToString`: nil enclosing
declaration, AllowUniqueESSymbolType, UseAliasDefinedOutsideCurrentScope and
IgnoreErrors. `serializationLevel >= 2` returns without another acquisition.
Each invocation has fresh node-builder context and restores the level on every
exit. The native `noErrorTruncation` option is retained by the existing config
owner; it selects the native maximum of one million rather than 160. These
length limits are native approximate-length gates, not encoded output budgets.

The traversal preserves these decisions from `nodebuilderimpl.go`:

- Reduce the type before primitive/alias dispatch. Primitive branches have
  their actual length contributions. Alias arguments precede symbol spelling;
  a displayed alias never expands the aliased structure.
- `mapToTypeNodes` has its own initial truncation branch, its ordered per-item
  check and its last-item continuation. Non-bare and bare lists differ. An
  already-truncated bare list with more than two items still visits first and
  last. Tuple argument acquisition precedes optional-marker removal and the
  list gate; labels are acquired only for retained tuple positions.
- `createTypeNodeFromObjectType` acquires the resolved inventory before its
  empty/single-signature/abstract-constructor decisions. General object output
  checks truncation before calls, constructs and indexes; properties have the
  native per-item gate and last-property continuation. Property raw values are
  acquired before name/readonly/accessor/method serialization decisions.
- Signatures enter their native scope, expand rest parameters, then acquire
  type-parameter declarations, ordinary parameters, this parameter and return
  annotation/predicate in native order. Existing signature owners supply
  optional/rest transformations and inferred defaults/constraints.
- Conditional serialization checks truncation, serializes the check type,
  installs only the current conditional's infer parameters for the extends
  type, restores them, then asks true and false branches. Inferred constraints
  use the shared native comparison/constraint operation.
- `visitAndTransformType` tracks actual type recursion and the native composite
  symbol/node identity with its `depth > 10` gate. Equality-only native display
  bookkeeping does not allocate deterministic identity-order ranks. Nil
  enclosing-declaration visits do not read the native serialized-node cache;
  signature scopes must retain any actual non-nil context before claiming the
  cache is unreachable. Failures are never memoized as a successful truncation.

Name-rendering work can itself acquire types (`isStringNamed` for computed
declarations, symbol qualification/type arguments). Those operations must use
the actual symbol/declaration facts; public origins and graph labels cannot
substitute. New generated-name or mapped producers extend this owner at their
real native branch, rather than silently falling through to an ordinary object.

Proof will retain paired cold/no-diagnostic, mismatch/diagnostic and healthy-after
queries; native default/noErrorTruncation boundaries; first/middle/last hidden
failures in flat objects and union lists; alias suppression; ordinary and tuple
reference arguments; signature parameter/default/return failures; recursive
types and repeated calls in one request. Compiler expectations are captured
before final Rust assertions. Counterfactual removal of the display invocation
must turn an original invalid dependency into a wrong healthy answer; merely
restoring an old Pending worker is insufficient proof.

## Current checkpoint and remaining native decisions

The effect-only entry is staged in `evaluation/native_display.rs`; the initial
native matrix is GREEN `vrun-e1beba25-e340-448d-be03-0c97529d9a1f`
(54 fresh requests). It establishes the strict `> 160` boundary, byte lengths
for ASCII and multibyte identifier names, first/middle/last property demand,
alias suppression, and default/true/false/inherited/overridden/null-reset
`noErrorTruncation`. Native first-property contributions are actual SymbolName
UTF-8 bytes plus one, followed by the property's type contribution. The maximum
one-million boundary, list-specific truncation and depth/serialization-level
reentry controls are still required. The complete final Project boundary and
option test passed in `vrun-c6fde60d-d8e2-4e44-a6ad-ab65d43823b4`. The separate
TypeLiteral suite in that run exposed computed-expression binding context
errors; it did not establish all TypeLiteral source-check outcomes.

`formatUnionTypes` now has one Model owner shared by product and diagnostic
display; it preserves canonical nullable identities and boolean-group folding.
The new serializer's frame restoration applies on both success and DemandError.
It does not install a successful diagnostic cache after a failed child.

The full renderer remains required. Current explicit dependent refusals cover
colliding identifier-reference qualification, nondefault outer type-argument
symbol chains, matching type-parameter annotation reuse, and instantiation
expression node reuse. These are incomplete native branches, not accepted
product differences. The reference iterator-default-elision branch also needs
completion before claiming the reference serializer complete. Primitive, alias,
list, conditional and ordinary member processing do not license a generic walk
through other unimplemented producers.

The original claim that nil enclosing context excludes all annotation reuse
was too broad. It excludes the serialized-node cache lookup and
`serializeTypeForDeclaration`'s pseudochecker branch. In contrast,
`typeParameterToDeclaration` invokes
`typeToTypeNodeHelperWithPossibleReusableTypeNode`: it first asks the actual
annotation type under the current mapper, and if identical tries syntax reuse.
That exact operation remains a dependency at its branch.

Reporting relations need a rendered result, in addition to effects. The planned
shared entry is `diagnostic_type_display(ty, DiagnosticTypeFormat, consumer)`
returning an opaque `RenderedTypeName` with exact native bytes/equality. Its
options must retain the actual optional enclosing DeclarationIdentity and native
format flags, including FullyQualified. `check_diagnostic_type_display` may
then discard this result. `getTypeNamesForErrorDisplay` compares the two initial
strings and may rerun both with fully-qualified names; reporting-chain argument
equality consumes those exact strings. Product graph qualification and bounded
summary rendering cannot supply this decision.

## Rendered-result owner and migration

The operating envelope is one captured Project and one request-local evaluator,
with reentrant checker serialization. This is an internal semantic operation:
the product does not publish diagnostics, and its summary/graph budgets do not
govern native reporting. No cross-request cache, printer service or parser
round trip is needed.

Two implementations were considered. Adding string concatenation to each
effect worker is superficially smaller, but it spreads precedence, quoting,
parenthesization and node-reuse rules across semantic acquisition branches.
It also cannot represent `mapToTypeNodes` replacing an earlier colliding
reference with a newly qualified node. The chosen direction is for the current
native effect traversal to build private diagnostic syntax nodes, followed by a
pure native printer over those nodes. Those nodes retain only output syntax,
plus the actual type/symbol identities needed by node-builder transformations;
they are not a second semantic type model. Annotation reuse must import actual
captured syntax through its native reuse operation, rather than printing a
source slice or acquiring every type mentioned in it.

The public-to-evaluator interface is planned as:

```rust
fn diagnostic_type_display(
    &mut self,
    ty: TypeId,
    enclosing: Option<DeclarationIdentity>,
    flags: DiagnosticTypeFormat,
    consumer: &Location,
) -> Result<RenderedTypeName, DemandError>;
```

`DiagnosticTypeFormat` retains native bits, beginning with the existing default
(`AllowUniqueESSymbolType | UseAliasDefinedOutsideCurrentScope`),
`UseFullyQualifiedType`, `NoTypeReduction` and `NoTruncation`. The operation owns
the native node-builder flag mask and IgnoreErrors addition. Signature and
symbol serialization expose their own native format inputs through this same
display owner; callers must not pretend signature or symbol formatting is a
type-format question. The existing effect entry becomes a convenience that
discards the rendered result.

Flags are exact caller input: `FULLY_QUALIFIED` alone does not implicitly add
`DEFAULT`. Native `getTypeNamesForErrorDisplay` first uses the default flags,
then its equal-name retry passes only `UseFullyQualifiedType`. The convenience
effect entry explicitly passes `DEFAULT`.

`RenderedTypeName` exposes exact byte equality, `as_bytes()` and `into_bytes()`.
It must **not** promise a Rust `str`: pinned `printer.go` cuts the final Go string
at byte `maxLength - len("...")` when `len(result) >= maxLength`. The native
320-byte (or two-million-byte) cutoff can split a multibyte identifier. JSON or
user-facing message encoding may replace invalid UTF-8, but relation-chain
equality must see the original bytes. Reporting text arguments and ASCII
prefix/dotted-name concatenation therefore retain these same exact bytes.
The raw-byte compiler control is now GREEN
`vrun-09dc73d9-c103-4075-861d-f9ac79191265`: the maintained
`native-diagnostic-bytes` fixture asserts 20 results across default and
`noErrorTruncation=true`. It distinguishes 319/320/321 ASCII bytes, whole
multibyte boundaries and each possible split within two-, three- and four-byte
identifiers. Five default results are invalid UTF-8; the hook encodes the
already-produced Go string as `[]byte` (base64 in JSON) without a second
serialization. This proves native printer bytes, not the still-pending final
reporting consumer of equal rendered names.

The migration replaces `diagnostic_type_node`'s effect-only return with an
owned syntax result, without moving or repeating semantic reads. The private
printer owns native operator precedence, punctuation, whitespace, string and
identifier escaping, `MultilineObjectLiterals`, and final byte truncation.
Serialization level restoration remains outside the builder, as native does;
printing performs no semantic queries. Native builder context keeps actual
enclosing declaration, mapper, flags, approximate byte length, depth, visited
identities and native node-reuse cache state. Failed semantic reads remain
`DemandError`, never a truncated successful result.

The current effect-only header reconstruction in
`diagnostic_identifier_reference` will disappear. Collision detection will
inspect the actual produced node shape, as `isIdentifierTypeReference` does,
and ask `typesAreSameReference` using the actual retained input type. A
FullyQualified retry builds a replacement node with the native saved/restored
flags; it cannot reuse Q4's file-location suffix or public metadata origin.

Required native controls include equal initial names with distinct canonical
symbols, shared alias/reference identity, ordinary versus FullyQualified text,
actual enclosing declaration selection, escaped names, 319/320/321-byte final
cutoffs, a multibyte split at the cutoff, and equality of two truncated strings.
Final reporting controls must show the branch-dependent semantic acquisition
caused by those equalities. Existing 160-byte approximate-length controls stay
separate: approximate node-builder length and final printed length are different
native decisions.

## Global identities and reference display

Before completing ordinary generic reference display, move the whole
`getGlobalType` decision out of binding into the existing evaluator globals
owner. Binding supplies `getGlobalSymbol` identity and native symbol metadata.
The evaluator checks the Class/Interface flag, acquires `getDeclaredTypeOfSymbol`,
then checks the established target's complete TypeParameters length. Recovery
identity is distinct from consuming a missing global as an ordinary factory;
declared-type failures retain their original cause in either case.

This migration cannot make `generics::reference_classification` semantically
acquire globals during class/interface target construction. Native
`getDeclaredTypeOfClassOrInterface` does not ask whether its result is Array.
Retaining the current eager classification and adding richer global reads
would introduce new dependencies and recursion. Instead, remove the redundant
Array/ReadonlyArray classification from Model reference headers. Model keeps
the actual target and Tuple flag/metadata. Evaluator operations own
`isArrayType`, `isReadonlyArrayType`, `isArrayOrTupleType` and
`isMutableArrayOrTuple` at their actual consumers. ReadonlyArray fallback aliases
Array, so a target can satisfy both mutable-name display and native readonly
identity; do not collapse those distinct questions into one Boolean.

Migration inventory: target construction and Model header propagation;
`structure::array_element_type`; tuple/array predicates; iteration; relation and
inference array branches; const-variable and type-source checks; union derived
type checks; evaluator widening and the obsolete nested Model widening branch;
diagnostic display; and product reference summary/kind/precedence/graph labels.
Each consumer must preserve its native operand acquisition order. Q4 may use a
single acquired presentation classification for its own repeated formatting,
but that operation belongs above semantic globals and must preserve failures.
No caller may retain an old header fallback after the migration.

Native startup establishes Array and ReadonlyArray identities before ordinary
queries, while Iterable-family getters are lazy. The migration must establish
the fixed-global identity contract with native controls; it must not copy lazy
iterator getters onto startup globals or force every unrelated global merely
to construct a user interface. Existing identity-only recovery behavior remains
until this operation is coherently replaced and proven.

After that move, `typeReferenceToTypeNode` consumes whole type arguments first,
then array/tuple branches, then outer parameter groups. For ordinary references
with nonempty arguments and a nonnil native type-parameter list it observes
Iterable, IterableIterator, AsyncIterable and AsyncIterableIterator global
getters in native short-circuit order, even when the reference name differs.
A matching target asks actual retained reference syntax for written argument
presence/count, then compares defaults from right to left with native Identity.
Only after that does it map retained arguments and serialize the symbol chain.
Absent and present-empty parameter/argument lists cannot be conflated where
native tests nil. One Source recipe supplies those actual node facts.

Control obligations include missing/wrong-kind/wrong-arity/correct globals;
ReadonlyArray fallback aliasing Array; an unrelated generic reference observing
iterator globals; each iterator family match and short-circuit; written versus
deferred arguments; nil/identical/nonidentical trailing defaults; and changed
outer groups. A claimed wrong-arity hidden failure must be witnessed through a
real declared-type dependency. Type-parameter constraints alone are insufficient:
`getDeclaredTypeOfClassOrInterface` does not acquire them just to count parameters.


## Current globals and rendered-result stage

The global/classification migration is staged, without Rust validation yet.
`global_type_lookup` now owns `getGlobalType`'s flags, declared-type acquisition,
and total outer/local parameter count, in that order. The binding layer retains
lookup and declaration facts only. Model reference headers no longer classify
Array/ReadonlyArray; semantic consumers ask the shared evaluator predicates at
their actual native sites. `reference_display_argument_count` owns all four
iterator getters and right-to-left default/Identity elision, shared by diagnostic
and Q4 reference display.

Native global-display15 is GREEN
`vrun-e9ffd68d-afb5-484e-9b23-0d381eaa7775`. Displaying `Box<number>` acquires a
missing imported base of a zero-parameter global Iterable before rejecting its
arity. Array and nongeneric display skip this; actual local parameters or
ContainsThis skip the interface-base question. The corresponding final matrix
is `tests/type_global_display.rs` (unrun). The resolver now retains original
module/alias dependency provenance, so `ignoreErrors` suppresses only an absent
terminal entity, not that acquired module failure.

The shared widening migration also remains uncompiled. Its maintained native24
controls are GREEN `vrun-5ffcb4a7-7fca-42ba-a34b-a94dae05096d`; final
`tests/type_widening_context.rs` includes nested sibling contexts, last-value /
first-name order, readonly tuples and exact missing versus undefined. These
controls intentionally include object literals whose complete property-symbol
producer is still required; they are not narrowed to avoid that dependency.

The existing effect traversal now constructs private output syntax in
`native_display/syntax.rs`. Type, signature, predicate and property-symbol
entries return `RenderedTypeName` bytes; the type entry alone owns the native
serialization-level gate and final 320/two-million byte cutoff. Source predicate
names are retained at their factory rather than recovered from a signature at
render time. Literal-name quoting and native type-position/extends rules reuse
`crate::printer`; Q4 still owns its independent admission, truncation and issues.

This is a coherent implementation checkpoint, not completed native rendering.
Qualification still requires the whole accessible-symbol-chain and container
operations over the existing resolver's actual scope tables. Enclosing-node
serialized caches, signature fake scopes and syntax/pseudochecker annotation
reuse remain precise acquired dependencies. The current entry must not emit a
healthy approximation where one of those operations is required. No product
graph qualification is substituted for native diagnostic names. Reporting's
end-to-end assertions and byte-equality decision controls are still required.

## Raw default symbol display checkpoint

`native_display/symbols.rs` now owns the default `SymbolToString` operation over
the actual `ReferenceSymbol`. Its one-element, nil-enclosing chain preserves raw
aliases; name acquisition does not ask for the symbol's value type. The same
`getNameOfSymbolAsWritten` decision now supplies established type headers and
property diagnostics, replacing the prior Source-only copy. Computed-name
serialization consumes actual nameType/Late facts; generated unique names remain
the real unique-symbol producer dependency.

`getSpecifierForModuleSymbol` still performs its source-file/equivalent export=
container search before the nil-enclosing fast return. The shared same-reference
operation owns merge, alias resolution, final merge and actual symbol equality.
The asserted native run `vrun-5c7e1cdf-cb67-4e12-807c-f579e223c249` established 19 raw
symbol cases: quoted and escaped names, computed and private names, default and
raw alias names, a 1080-byte untruncated name, and three module export= cases.
Displaying `module-alias-failure` adds TS2304 from its missing export target;
displaying `module-alias-healthy` skips the missing annotation on its exported
value. Both native test files passed, with zero skips. No helper setup failure
is treated as compiler behavior evidence.

The existing final symbol-written-name test adds a computed-name/type-parameter
merge witness; its ten strict/loose native controls passed in that same run.
Final alias false-pop and module diagnostic consumers still require coordinated
product validation. Raw symbol controls alone do not prove those consumers or
the unfinished qualification and annotation-reuse operations above.

## Next qualified-name increment

The operating envelope remains one request-local evaluator over an immutable
captured Project. Qualification can reenter type checking and display, so an
output string or a spelling-indexed map cannot be its semantic state. Two
options were compared: add a local qualifier search in each diagnostic name
consumer, or port the shared native accessibility operations and let the node
builder consume their actual symbol chains. The latter is the smallest
coherent choice: type, expression, unique-key, default-symbol and fully-qualified
retry consumers ask the same chain questions, with different builder flags.

`evaluation/symbol_accessibility.rs` will own `getAccessibleSymbolChain`,
`needsQualification`, `getContainersOfSymbol`, their alternative-container and
alias-in-container operations, and the corresponding native caches. Source owns
the per-node `someSymbolTableInScope` facts and actual ancestry. Binding supplies
raw table contents without resolving aliases or applying lookup meanings.
Collector Signature and Body scopes are an implementation split; they must not
be exposed as separate native function Locals. Binding's logical raw-table
observation joins that retained representation according to the actual native
node, preserving symbol identity. Nil versus present-empty comes from native
container factory facts, not whether the linker happened to allocate a table.
Tables keep distinct typed identities: Locals(actual captured/synthetic node),
RawExports(symbol), Members(symbol), Globals, and ResolvedExports(symbol).
First-relevant-scope cache keys include the actual ScopeLocation, meaning and
external-alias policy. A computed nil chain is an installed answer; a failed
acquisition is never installed as nil. Per-search visited pairs are removed on
unwind. Alias-list caches apply only to the native table families that cache
them. Raw class members are filtered for type parameters before any late-member
operation; class-expression name tables retain the actual class symbol.

The member lane supplies the shared raw locals/globals/table observation, exact
Type and Namespace flags, and complete `getParentOfSymbol` (late then merged),
`getMembersOfSymbol`, and `getExportsOfSymbol` dispatch. The current raw parent
operation is not a substitute. Alternative module containers reuse captured
Source.Imports and the existing resolving module request owner, then the native
program-source fallback; they do not scan syntax for a second import inventory.
The static-unique witness also needs the real unique-symbol/nameType and late
property producers. Canonical native parameter symbols belong to their existing
factories, including cloned and expanded parameters.

The algebra lane owns native fake-scope allocation/reuse and all context-map
lifetime. ScopeLocation distinguishes captured nodes from synthetic Blocks;
their identity survives temporary locals replacement. Fixed enclosingFile stays
separate from mutable enclosingDeclaration. Cloned/restored maps include
remapped symbols, generated names and the native typeParameterSymbolList scoped
set. Signature and mapped consumers enter one closure-based scope operation;
their native optional arguments and original/expanded parameter identity remain
distinct. No consumer reconstructs locals from displayed parameter names.
The scope-owned naming state includes TypeId-to-name, used exact names and
next suffix by raw text, in addition to actual-symbol typeParameterSymbolList.
Its Source binding-pattern walk preserves native early return after the first
omitted/BindingElement in each pattern. Mapped GenerateNames wrappers need a
real declaration-less TypeParameter symbol and Type factory, separately from
ordinary parameter-value symbols.

The builder then ports `lookupSymbolChain`, `createAccessFromSymbolChain`,
`symbolToExpression`, and both qualified type-parameter lookup operations.
It preserves root-to-leaf acquisition order, export-name same-reference lookup,
symbol sorting, indexed access versus dotted access, and nil versus supplied
type arguments. The shared pure output syntax retains entity/expression/type
node distinctions; it does not call the checker. `getNameOfSymbolAsWritten`
is extended in its existing owner for remapping, initial-name/default-binding
context and unique nameType handling. Type-parameter declarations clear
WriteTypeParametersInQualifiedName while acquiring defaults and restore it on
every exit.

Non-nil entry contexts also require the complete native
`modulespecifiers.GetModuleSpecifiers` owner. The current resolver answers input
resolution, which is a different question from choosing emitted specifiers.
The new operation must consume retained program/package/module-format facts,
the native mode-aware cache, project-relative preference and ending policy;
it cannot guess a relative path. Until that operation lands, only its exact
branch remains a dependency. Nil-entry property/fake scopes still keep
enclosingFile nil and therefore use the established specifier fast path.

Final proof starts with the already measured deprecated static-unique/default
witness and its ordinary-call control. Add actual local shadowing, named class
expressions, raw versus resolved exports, competing reexport spellings and tie
order, default/export= aliases, namespace/value meaning differences, repeated
fake-scope reuse, and missing dependencies behind the selected versus skipped
alias. Module specifier controls distinguish nil/non-nil context, Node modes,
package exports and resolution-mode attributes. Failure controls require the
original consumed range and unchanged healthy controls. The planned recovery
completion migration must preserve raw typed failure evidence through these
caches and scope restoration; this owner will not add a separate journal or
convert failed chains into empty results.

### Stage17 qualification foundation

The shared accessibility module now owns the native scope-table walk, accessible
chain and alias-list caches, visited symbol/table pairs, `needsQualification`,
container discovery, alias-in-container lookup and both alternative-module cache
policies. The builder owns parent-chain selection and native best-specifier
sorting. The public/current rendering entry still reports the precise required
qualified-access syntax operation until the full entity/expression builder and
qualified type-parameter acquisitions are installed. This checkpoint does not
complete qualified display or the deprecated static-unique/default witness.

Raw tables and raw Parent must preserve the binder's stored identity until the
native caller explicitly merges it. A missing raw premerge snapshot is a
required producer dependency; a canonical symbol is not an equivalent answer.
JSON supplies another distinct fact: it has a SourceFile module symbol, but
`getExternalModuleIndicator` returns nil for JSON and binder creates its module
in a separate branch. `hasNonGlobalAugmentationExternalModuleSymbol` and
`getExternalModuleContainer` therefore cannot treat JSON's module factory fact
as an external-module indicator.

The next builder increment must retain `typeParameterSymbolList` insertion even
when `lookupTypeParameterNodes` returns nil, distinguish that rule from expression
chain lookup, and consume `getTargetSymbol` rather than resolve aliases for the
parameter-declaration gate. Class/interface outer/local parameter acquisition
must be extracted from the existing canonical owners, preserving declaration
order and unique TypeIds. `symbolToTypeNode` uses the current enclosing node's
source file for resolution-mode attributes, while `getSpecifierForModuleSymbol`
uses the fixed entry `enclosingFile` for its cache and module-specifier choice.
These are distinct native questions.

## Stage24 serialized diagnostic-node cache

The implemented owner is `native_display/cache.rs::diagnostic_visit`, porting
`visitAndTransformType` (nodebuilderimpl.go3089–3157). Its operating envelope is
one request-local checker with the existing printer entry points: a nil inner
symbol tracker, InternalFlagsNone and no hover expansion. The cache belongs to
the persistent node builder, while each display invocation owns fresh context.
Q4 byte/depth admission remains outside this operation.

Two alternatives were considered. A cache around the finished type string
would hide the actual enclosing scope, flags, tracker callbacks and incremental
length, and would replay checker work the native cache skips. The chosen cache
stores immutable private output nodes at the existing transform boundary. The
key is actual enclosing identity, TypeId, full native flags and internal flags.
The constructor/symbol or originating-node depth identity is acquired before
lookup but is not an extra cache-key component. Scope identities include actual
captured nodes and the reusable synthetic scope IDs; the fixed enclosing file
remains separate context state.

A nonnil scope permits a hit. The hit replays recorded TrackSymbol calls, adds
native approximate length, propagates truncation and clones the output node,
without entering the depth/visited transform. Nil-enclosing transformations
still install entries, but native never reads those entries. On a miss, the
`depth > 10` gate precedes visited/depth installation. The worker runs with an
isolated tracked-symbol list. A completed worker installs only while both
reportedDiagnostic and encounteredError remain false, then restores visited,
depth and the previous tracked list. Misses restore the outer list without
appending the child's records; hits replay into the current list. Interruptions
restore all local state and install no node.

The entry stores the worker's raw `NativeCompletion<Node>`. Completed semantic
recovery is distinct from a native builder diagnostic and does not itself veto
publication. Cache replay copies the stored completion; only observation at the
outer display boundary binds source-free recovery locations. Captured failure
locations remain unchanged. The tracker reads raw symbol flags, skips type
parameters, and records exact symbol/scope/meaning; it does not perform alias
eligibility or symbol-type queries. Existing printer entries have no external
tracker callback. Reporter status is sticky even with that nil inner tracker;
for example, This in an object type can report while IgnoreErrors preserves the
returned node. ReportInferenceFallback is the native exception and does not
set reportedDiagnostic.

WriteTypeParametersInQualifiedName is native ordinary flag bit9. Its former
parallel context boolean was removed so signature/predicate entry, parameter
flag clearing and the serialized-node key observe one state. The general object
member loop now owns InObjectTypeLiteral save/set/restore, after the native
empty and single-signature early branches.

The same audit exposed the separate `serializeTypeForDeclaration` operation
(nodebuilderimpl.go2167–2225). Ordinary property serialization now enters that
owner with the already acquired property type. Its property implicit-undefined
question deliberately re-reads the raw property type before the Optional,
question-token, ReverseMapped and non-missing-undefined gates, matching
emitresolver.go581. That second read is a different native operation from the
single property preparation read; it must retain any actual replayed evidence.
Unique-symbol allowance saves/restores flags and compares the actual symbol and
fixed enclosing file.

A nonnil enclosing declaration with an actual inferred/accessor declaration
still requires native pseudochecker acquisition, equivalence checking and syntax
reuse. That whole producer is not present yet, so the owner reports its precise
required dependency instead of substituting an ordinary type traversal. The
mapped `limited` excess-property regression reaches this branch. Its measured
OverloadRecovery expectation remains unchanged and the extra Unsupported cause
remains a required failing gate until the pseudochecker owner closes.

The isolated `native-diagnostic-cache` matrix and opt-in observer record natural
final-query executions: repeated calls in one checker, distinct scopes,
recovered hidden members, equal-byte names, truncation controls and recursion.
The hook observes already acquired raw fields and pointer identities; it asks
for no additional type, symbol or native identity. Ordered node-builder events
prove cache acquisition order; diagnostic-code lists are source sorted and
cannot. No repeated Project request is treated as a warm-cache test.

The 14-root matrix passed all three profiles in
`vrun-b378df78-f55a-4fb1-805c-485f363d1429`. Each final hooked summary/errorType
also matched the unmodified native API. Transition assertions cover actual
installation-before-hit, nonnil lookup, unchanged key and payload, exact length
addition, truncation propagation, TrackSymbol replay, and cleanup. The natural
warm comma query has three hits versus one for its cold call; distinct object
declarations retain distinct scope identities. Recovered hidden annotations
still permit native node installation. The `thisCold`/`thisWarm` family retains
one tracked symbol per entry and replays it on every hit. Long-name entries
retain addedLength 193 with truncation in ordinary/loose profiles, versus 448
without truncation under noErrorTruncation. The equal-byte inequality performs
four visits: left/right with their actual nonnil contexts, then the same two
types with nil enclosing and FullyQualified alone. Its compatible twin performs
no display work.

Two proof limits remain explicit. `recursive` has nil-context installation and
no hit, but does not reach the depth-elision boundary. The fixture labels
`reportedCold`/`reportedWarm` currently contain a native-invalid nested `this`
annotation (TS2526); those executions do not set reportedDiagnostic and cannot
prove the publication veto. A valid polymorphic-this producer and a naturally
expanding recursion witness are still required. The full declaration/reuse
continuation and its final-query gates are planned in
[types-native-pseudochecker-plan.md](types-native-pseudochecker-plan.md).
