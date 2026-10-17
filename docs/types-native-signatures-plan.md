# Native signature construction

Final modifier and identity coverage is also registered: compiler control
`vrun-63ec92a1-497f-40d0-9ca9-a4775ff6b36c` executed/passed twelve tests,
and final Project run `vrun-9ea600d1-61e1-43cf-859b-3d8c708b3c7a`
passed the fourteen-test signature file with zero skips. These additions pin
in/out/in-out and const-before-variance printing, and separate canonical types
for same-named T parameters on unrelated overload declarations. The behavior
was already implemented, so these coverage additions had no artificial RED.

This increment extends the complete public T/Q4 operation. It does not narrow
the required design profile or claim native call resolution, overload inference,
flow admission or the final semantic/resource gates are complete. The same
captured Project generation, canonical retained binder and request-local
Evaluator supply every operation. TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b` owns expected behavior.

## Operating envelope and compiler owners

The immediate consumer is an own FunctionDeclaration name or a renamed import
whose canonical value is a function, and function/constructor type nodes inside
annotated declarations. The complete lane includes required, optional, rest,
`this`, defaulted and generic parameters; annotated and inferred returns;
explicit predicates; and ordered call/construct signatures. Existing support
for required property signatures continues. Method/accessor/class construction,
contextual function expressions and complete call/new/tag checking remain
mandatory neighboring operations, with explicit pending results until their
relevant baseline and final Project tests exist.

The pinned owners are:

- `getTypeOfSymbol` / `getTypeOfFuncClassEnumModuleWorker` (checker.go 16817): a
  function has a canonical anonymous object type; its source name is not an
  interface nominal display label or type-alias label.
- `getSignaturesOfSymbol` (19711): enumerate own declarations in native
  bind/merge order and exclude an adjacent overload implementation by its real
  parent/kind/pos/end relationship. A sorted lexical origin list cannot replace
  this operation.
- `getSignatureFromDeclaration` (19741): capture the actual parameter symbols,
  separate a first `this` parameter, retain declared generic identities and
  distinguish call/construct/abstract/rest flags. Parameters do not become
  properties through their spelling or the reference index.
- `getTypeForVariableLikeDeclaration`, `getTypeOfParameter` and
  `isOptionalDeclaration`: annotation precedes initializer/context demand;
  optional read type and optional argument position are different questions.
  `isOptionalParameter` makes a defaulted parameter optional only when its raw
  syntax index reaches the signature's strong minimum argument count; a later
  required parameter can prevent this. A first this parameter participates in
  that raw index while remaining outside the ordinary parameter list.
  `serializeTypeForDeclaration` and `requiresAddingImplicitUndefinedWorker`
  use a separate summary question: a strict required initialized parameter can
  display undefined while its actual getTypeOfSymbol graph type remains the
  declared type. Rest parameters need actual established array/tuple facts,
  including tuple-derived required count, rather than a syntactic shortcut.
- `getReturnTypeOfSignature` / `getReturnTypeFromAnnotation` (19906/19963): an
  explicit return annotation consumes its own type dependencies without
  checking a function body. A bodyless unannotated return is genuine any;
  body inference uses the actual body/reachability operation and remains pending
  until implemented. Circular return recovery is not successful any.
- `getTypePredicateOfSignature` / `createTypePredicateFromTypePredicateNode`
  (relater.go 2016/2084): explicit predicate subject uses the native parameter
  index or this; inferred predicates remain excluded by the designed profile.
  A predicate return has its own boolean/void return question and a distinct
  asserted-type child demand.

## Designing twice

One candidate would put parameter types, return values and rendering rows into
Source's output and pass generic environments/caches through every helper.
That makes captured syntax responsible for semantic checking order and couples
function construction to display limits. The chosen alternative keeps owned
SignatureRecipe syntax descriptions and exact locations in Source, while the
existing deep Evaluator owns native signature construction and demand caches.
Presentation consumes established complete ordered descriptors; it does not
infer semantic facts from text or optional fields.

For the model factory, a callable-only wrapper could silently invent empty
properties/indexes and grow another constructor for each future family. The
chosen boundary is a complete structured-member constructor accepting the
established MembersView and metadata. Existing ordinary objects can delegate to
it after supplying their genuinely known collections; call/construct signatures
and library members cannot be guessed empty. Callable display is derived from
these descriptors, not a function-name alias. A nominal interface keeps its
separate label and can avoid undemanded signature children in summary/depth-zero
queries just as it avoids undemanded properties.

A third choice is whether every signature child reuses MemberId or a string-keyed
cache. Neither describes the compiler question. SignatureRoot has an exact own
file/raw anchor/final role; a signature question key selects parameter index,
this, return, constraint/default or predicate type. Distinct DemandIds cache
those questions and their complete located failures. Actual generic TypeId and
canonical BoundSymbol identity remain separate from SignatureRoot, MemberId,
DemandId, graph IDs and display origins. Instantiated/composite signatures will
need their native target/mapper/constituent identities, not origin equality.

## Integration and order

The completed Store now retains typed own declaration associations in native
bind/merge order. The binder's single value-family dispatch will return either
an ordinary variable declaration or ordered function declarations after its
supported-family check. Namespace/function merges and other value families
must not be collapsed into a plain function. SelectedSeed holds this typed
result, and parent Evaluator dispatches inside its existing canonical value
cache; selection does not type-check candidates.

Source attaches each retained declaration to its final FunctionDeclaration
name anchor and captures the signature syntax once using the cached AST/parent
owner. FunctionType, ConstructorType, call and construct member syntax route
through the same signature recipe operation. Exact cooked names and raw origins
stay distinct; missing canonical attachments are internal errors.

Evaluator establishes descriptor collections first and registers child recipes
only where native signature construction is lazy. The graph summary can then
demand required children in native display order; an unsupported valid child is
located Partial, while consumed invalid recovery stays a whole-operation error.
A named interface summary does not force its lazy members. Required roots and
collection acquisition cannot be made partial by hiding a failed native
construction behind an empty vector.

Canonical generic parameters use the existing retained declaration/reference
associations and the shared declared-type owner. Their complete canonical own parameter declaration lists preserve native order.
`getConstraintDeclaration` and `getResolvedTypeParameterDefault` select the first
nonnull constraint and default separately; a merged parameter cannot use only
its first declaration. `getDeclaredTypeOfTypeParameter` allocates the identity
before either dependency is demanded. Constraints/defaults, variance and substitution depend on actual typed parameter identity. The global
Array lane consumes these same parameter operations; no name-based `Array` or
utility shortcut will be introduced. The initial nongeneric stage avoids
claiming array/rest/generic success before these operations are established.

For ordinary generic signatures, Source captures only each actual own
TypeParameter name anchor. One alternative was to construct a signature-local
name environment; it would duplicate the canonical parameter decision and
could give parameter reads, returns and merged own declarations different
identities. The chosen `getTypeParametersFromDeclaration` operation resolves
these sites through the existing retained declaration association and canonical
declared-type cache, then deduplicates actual TypeIds in source order. It returns
owned Parameter descriptors carrying the same lazy constraint/default handles.
Acquiring the required identity collection is fallible; a failed collection
cannot become a known empty signature or a partial root. Constraint/default
checking remains a separate demand.

The fresh twelve-test compiler baseline passed with zero skips in
`vrun-f35ee481-a4b1-421b-b255-cdb93873e761`. It pins ordinary, const and escaped
parameter spelling, genuine-any constraint normalization to unknown, generic
call/constructor syntax, named interfaces with undemanded invalid children,
and distinct overload parameter identities/order through a renamed import.
The final thirteen-test native RED
`vrun-18fa4965-5dd4-4fdc-a8ac-78bb4e70b12a` compiled and executed all tests:
nine established cases passed and four new cases failed precisely on the
pending generic signature operation, with zero skips. Final thirteen-test
GREEN `vrun-d152990f-faa8-4e12-8836-8bb4c442ae23` then passed the explicit
single file with zero skips. The admitted ordinary generic signature family
now preserves its canonical parameter/read/return identities, const and exact
escaped display, overload order and lazy consumed-invalid dependency behavior.
Generic reference arity/default/mapper operations, instantiated signatures,
contextual/JS and JSDoc full-signature questions remain separate obligations.

## Behavior-level slices and methodology

During the root's sole Rust checkpoint this agent may create compiler fixtures,
controls and this plan, but does not edit or build Rust. For each new native
behavior, first execute its fresh pinned compiler controls through registered
`harness-test` scoped to one file. Then add and execute final public
`Project::type_of` tests in `tests/type_signatures.rs`; verify the actual missing
behavior assertion before production changes. Pure DTO/enum scaffolding can
precede that RED so absent imports never masquerade as behavioral evidence.

1. Own and renamed ordinary annotated functions: required/optional/default/this
   parameter descriptors, strict-null option differences, exact origins and
   return type. A deliberately unresolved return expression demonstrates that
   an explicit return annotation does not consume the body. Unresolved demanded
   annotations still fail with their exact cause.
2. FunctionType/ConstructorType and ordered call/construct interface members:
   same signature operation and complete graph fields, alias labels distinct
   from function symbol names, depth boundaries retaining collection totals.
3. Ordered overload declarations: real adjacent implementation exclusion and
   own origins/order, plus same-named unrelated functions and namespace merges
   that remain distinct or explicitly pending. No guessed best overload.
4. Canonical generic parameters/constraints/defaults/variance and exact reused
   reference identities; native Array rest parameters after the shared global
   and substitution owners exist. Wrong/missing global recovery remains invalid
   when consumed.
5. Explicit `is`, `asserts is`, bare `asserts`, and this predicates with native
   parameter indices; predicate child failure/provenance and strict-null
   states. Inferred predicates are never silently substituted.

The first nongeneric native signature family reached genuine four-test RED in
`vrun-99dd5c3c-f242-4d84-a887-b03eae2ca66f` and four-test GREEN in
`vrun-09608720-3a22-42ee-9ee1-2fed984b3c4d`, with zero skips. The default-before-
required counterexample then reached genuine final optional-flag RED in
`vrun-b4e69451-4b26-438c-bc51-99448c90ed0b` and five-test GREEN in
`vrun-fe1e209d-fd98-4e9b-9b11-0e587b908813`, with zero skips. The fresh compiler
seven-test baseline passed in `vrun-48b3ad42-784a-4306-924b-35cd51a22263`.

Ordinary summary parameters now use a distinct display TypeDemand only where
the admitted native serializer needs one. Graph parameters retain the read
TypeDemand; adding display undefined does not replace that graph type. Question
tokens, strict/loose state, required void, raw this index, inferred defaults,
and already-contained undefined are native distinctions. Literal-alias source
spelling does not become a label: the fresh Word="ready" control has no alias
symbol and formats its parameter/return as the literal type.

Initialized signatures with a rest parameter remain explicitly pending because
their exact optionality asks getMinArgumentCountEx, which demands the rest type
and can derive tuple required elements. Parameter modifiers/properties,
generic references/instantiation, inferred bodies and contextual
IIFE/JS questions remain mandatory subsequent operations. No known bool/empty
collection is substituted for those pending decisions. Broader expression
inference, contextual state,
call candidate selection, relation/cache/limit schedules, flow and complete
resource/representation proof remain required for delivery.

Ordered nongeneric overload collection reached final seven-test RED in
`vrun-77e055f4-f8ae-4bb8-b053-29a7fb7e2ea6` and GREEN in
`vrun-2688203f-fdc1-4fb3-b2f9-6d772436c1a3`. The owner uses actual declaration
parent, kind and raw adjacency; comments preserve adjacency, while intervening
declarations and separate merged namespace bodies preserve signatures.

Explicit predicates reached final nine-test RED in
`vrun-6eec9a82-ef40-41be-8714-254ad94d3fde` and GREEN in
`vrun-2cafe3f6-6da6-417e-8788-c48a1615e0b2`, with zero skips. The fresh pinned
nine-test signature baseline passed in
`vrun-2362a328-3ad0-446e-9985-817fc4939db9`. Parameter-index and this subjects,
`is`, `asserts is`, and bare `asserts` are admitted in the ordinary nongeneric
family. Boolean/void return construction and asserted-type child demand remain
different questions. A named predicate-bearing interface can be established
without consuming an omitted invalid asserted type; expansion consumes it and
fails at the real source cause. Unbound predicate subjects, abstract
constructors, instantiated generic signatures, inferred returns and contextual/JS questions
still require their own complete operations; no inferred predicate is supplied.
