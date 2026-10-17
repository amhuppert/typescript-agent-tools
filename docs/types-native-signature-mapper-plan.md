# Canonical signature factories and mapper substitution

The operating envelope is one immutable accepted TypeScript/TSX Project and one
request-local Evaluator. TypeScript 7.0.2 at commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b` defines native identity, factory
order and failures. This increment closes the callable-reference dependency
and supplies the same real signature identities to call evaluation. It does
not claim complete overload resolution, generic inference, all anonymous
instantiation families, or the resource gates.

## Alternatives and chosen owner

Copying the current displayed signature and substituting its parameter/return
handles would admit nongeneric `Callable<string>`, but it loses native local
TypeParameter identity. `<U extends T>(input: U): U` requires a fresh U for each
instantiated signature, with the original canonical symbol and a composed
mapper for its raw constraint/default questions. Names, origins, identical
return demands and descriptor equality cannot identify that operation.

The chosen design retains an actual private SignatureId allocated by the Model
`newSignature` factory. The Evaluator's signature child owns the complete
`getSignatureFromDeclaration` cache, keyed by its actual SignatureRoot. A
descriptor retains ordered canonical local parameter TypeIds alongside the
immutable display views, its native minimum argument count and propagating
flag facts, and its existing lazy this/parameter/return/predicate handles.
Ordinary callers request one complete signature; they do not assemble its
identity or reconstruct native optionality.

The generics child owns `instantiateSignatureEx`, using the established target
SignatureId and actual mapper identity. Every invocation allocates a new native
SignatureId; that operation has no target/mapper interner.
It retains the same own declaration/origins while establishing distinct native
local parameter and instantiated read identities. Caching inventory resolution
retains the result; no extra original-signature identity is inferred from output
locations. A later native `getSignatureInstantiation` type-argument cache is a
separate question, with its native key. It must not be conflated with arbitrary
mapper instantiation or with inventory resolution.

## Private API

`Model::signature(SignatureData) -> Result<Signature, DemandError>` allocates a SignatureId and
returns an immutable descriptor. `SignatureData` contains required facts rather
than an optional fake ID. `Signature.id` is the actual identity used by later
call, arity and return caches. `Signature.type_parameter_types` retains actual
TypeIds; its display `type_parameters` collection remains an owned view of the
same identities. The factory checks cardinality rather than permitting the two
collections to drift.

`Evaluator::signature(&SignatureRecipe) -> Result<Signature, DemandError>`
owns declaration acquisition and cached success/failure. Existing
SignatureRoot/SignatureQuestion demand registration remains the sole source
parameter, display, return and predicate question owner. The call lane adds a
distinct `getTypeOfParameter` read where native optionality differs from the
displayed signature's `isOptionalParameter`; mapper substitution preserves all
these separate handles.

`Evaluator::instantiate_signature(&Signature, MapperId, consumer) -> Result<Signature,
DemandError>` owns the native non-erasing operation used by admitted reference
member inventories. The erasing signature and explicit type-argument cache
remain separate required callers; neither is guessed from this operation.
Ordinary member inventories
delegate to it for both call and construct collections. It hides fresh local
identity allocation, mapper composition, deferred child substitution and
failure propagation. Parent dispatch and presentation do not manage these
steps.

The canonical parameter clone operation allocates real TypeIds with the
original target identity and canonical symbol association. The constraints
child owns registering and resolving cloned raw constraint questions, following
`getConstraintFromTypeParameter(target)`'s guarded target demand and mapper.
The default owner separately follows `getResolvedTypeParameterDefault`'s
target/default/sentinel protocol. A public guarded constraint handle is not
copied as if it were the raw native clone question. An absent result is proved
absence; pending, circular, error recovery and attachment failures remain
distinct.

## Native sequence and composition

`checker.go:20524` clones every local TypeParameter first, then combines the
old-to-fresh mapper with the incoming outer mapper. It stores that composed
mapper on each clone before constructing the signature's instantiated this and
parameter symbols. Return type and predicate are deliberately unresolved at
this point (`GH#17600`). Native flags propagate through the single mask defined
in `types.go:1270`; optional-chain flags are excluded from instantiated flags.

`combineTypeMappers` and `CompositeTypeMapper.Map` are different from plain
merged lookup. Composition first maps through the clone mapper; a changed
result is instantiated through the outer mapper, while an unchanged source is
mapped through the outer mapper directly. The evaluator therefore retains
typed mapper variants and the complete operation. Concatenating parameter
names or two flat lookup arrays would lose recursive substitution and shadowing.

`getTypeOfInstantiatedSymbol`, `getReturnTypeOfSignature` and
`getTypePredicateOfSignature` preserve their existing demand order while
delegating actual type substitution to this owner. Index instantiation still
maps only its established value, retaining exact key/readonly/name facts.
Unaliased unions map every actual alternative and retain an unchanged identity
or invoke the shared Literal union reduction. Aliased/origin union construction
and anonymous structured instantiation remain explicit pending dependencies;
this increment does not treat them as identity.

## Compiler and final proof

`fixtures/types/native-signature-mapper/cases.ts` and
`tools/type-signature-mapper-oracle.test.mjs` use fresh pinned checkers without a
semantic-diagnostic prelude. Registered native four-test GREEN
`vrun-d1d65aa9-92c9-4f9e-944a-7cec2f828ba0` pins ordinary this/parameter/return
substitution, distinct fresh local TypeIds for string versus number target
instantiations, retained canonical symbols, repeated inventory identity,
parameter-chain composition, same-name shadowing and missing-child laziness.
It observes actual guarded constraints. A separately frozen disposable hook
control now calls the actual `GetDefaultFromTypeParameter` on native cloned
signature parameters in twelve fresh strict/loose sessions. The captured
`evidence/answers.json` pins string and number defaults, V's exact fresh-U
default identity, proved absence and the missing-default native errorType.
Initialization TS2318 diagnostics are retained separately; the missing default
adds TS2304 only after its demand. `evidence/provenance.json` hashes the exact
fourteen captured input/tool files and observations. These bounded specialized
controls are not full default/circular or semantic-guard proof.

The ordinary Reference presentation suite has a real final assertion RED for
callable/constructable substitution. The dedicated public mapper suite's four
tests were genuine assertion RED `vrun-75d1f401-6c3c-4714-bad5-f48fc82581a0`,
then GREEN `vrun-3c3407fa-413c-4fd6-b2d1-ef2aa0a4e4ee` with zero skips. They
pin this/parameter/construct/return substitution, canonical local graph
identities, mapped constraints/defaults, parameter chains, same-name shadowing
and lazy missing children at `Project::type_of`.

The fifth native control is GREEN
`vrun-e98f5506-73de-48f6-aefb-49c0e996a27b`, five executed/passed with zero
skips. It pins strict optional `string | undefined` versus loose `string`,
explicit union substitution/reduction, and read/return identity. Actual union
collection order is independently observed: `[undefined, string]`, while the
serializer prints `string | undefined`. The fifth public test genuinely failed
on Unsupported child substitution in
`vrun-d6688416-eef8-430d-a105-ad87cfea5cad` (four pass/one fail); its first
post-fix run exposed a test-only summary/collection-order assumption, corrected
from the fresh native `getTypes` observation before the final retry. Private
ID/cache wiring follows the mechanical exception; its behavioral evidence is
these actual consumer tests, not a helper-only assertion.

Final public five-test GREEN is
`vrun-450f464c-437e-4d5e-a6a8-faae10729054`, one matched file with zero skips.

The signature child also owns the actual `getReturnTypeOfSignature` frame,
keyed by SignatureId. Original and instantiated return demands are attached to
different identities; scalar substitution caching cannot merge their native
properties. A rejected push installs no property. A marked pop caches each
participant's own circular recovery as InvalidSemantic and retains the native
installed-property barrier, independent of noImplicitAny diagnostics. Agent1's
cycle suite genuinely failed in `vrun-b0643be8-86c4-4963-bb67-68e27829115d`;
the post-ID receipt `vrun-267245da-48b7-4fec-95b2-4a1730761ccc` passes both
mutual-recursion/provenance tests across options/depths. Its third test passes
the healthy direct-self Never control but still fails a separately owned
recursive-function summary spelling. That receipt is not a full cycle-suite
GREEN or a general return/inference completion claim. Independent Ousterhout
review remains part of the final checkpoint.

The review identified one whole-owner branch to retain before claiming the
return protocol: native `getReturnTypeOfSignature` checks the installed property
again after popping its frame. Nested call-resolution windows can establish it
during outer work; an already installed Resolved/Recovered property must win.
That exact final guard is present. Current admitted no-argument controls do not
isolate the nested-window case because broader contextual call argument work
remains pending, so this source-backed completeness correction is not described
as a separately reproduced behavioral fix.
