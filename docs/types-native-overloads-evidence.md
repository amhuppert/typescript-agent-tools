# Native overload selection evidence

This increment extends the existing request-local `resolveCall` operation.
It establishes ordinary nongeneric overload ordering and scalar applicability
through the public `Project::type_of` contract. Full contextual/generic calls,
recovery construction, new/tag/super/optional calls and relation/resource
gates remain required work. It is not a complete native-call delivery claim.

The expected behavior comes from TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. The relevant owners are
`resolveCall`, `reorderCandidates`, `chooseOverload`,
`hasCorrectTypeArgumentArity`, `hasCorrectArity`, `isSignatureApplicable`,
`getCandidateForOverloadFailure`, and `getSymbolOfDeclaration` /
`getLateBoundSymbol` in `internal/checker/checker.go`.

Two designs were compared using literal-specialized signatures, ties and
reversed script-file groups. Reconstructing identity from display origins
would lose actual declaration Parent groups and instantiated-signature
provenance. The chosen complete operation retains the actual `SignatureId`
factory attachment, follows real instantiated targets, and distinguishes an
explicit synthetic signature from a missing invariant. Source acquires actual
declaration Parent and anchor facts; the existing binder owns the canonical
symbol and late-bound dependency. No argument/return demand is added merely
to reorder signatures.

`reorderCandidates` preserves native parent-group order and cutoff, then inserts
literal-specialized signatures before ordinary signatures. `chooseOverload`
checks type-argument arity, argument arity and applicability in native order.
Multiple candidates use Subtype before Assignable. False continues selection;
an actually consumed failure stops it. Explicit this is checked before
positional arguments. Void minimum counts consume the distinct parameter-call
question, while unused optional annotations and excess arguments are skipped
where the native owner skips them.

Anonymous function/constructor-type symbols are not native nil:
`bindFunctionOrConstructorType` creates both the Signature symbol and an
anonymous TypeLiteral symbol. The current singleton nongeneric case can omit
identity bookkeeping only because it cannot change candidate order and Source
proves this native symbol is not ClassMember, so no late-bound type demand is
skipped. Multiple anonymous candidates remain explicit pending until their
canonical binder identity is consumed. Dynamic members require the complete
`getLateBoundSymbol` / `hasLateBindableName` question; class-parent syntax or
display names do not establish that identity.

The frozen compiler control is
`tools/type-native-overloads-oracle.test.mjs`, with
`fixtures/types/native-overloads`. Registered native run
`vrun-07a446ae-9099-4b9a-af2a-d11a04f6fd44` passed two tests with zero skips:
56 fresh healthy final roots across strict/loose and normal/reversed file-order
profiles, plus 12 same-checker recovery/scheduling observations. They pin exact
literal specialization including escaped text, Any's Subtype-versus-Assignable
choice, ordinary/Void arity, explicit this, same-parent ties, reserved call
signatures and later script-parent priority. Missing argument/type-input
controls distinguish visited failures from skipped excess arguments.

The behavior RED in `tests/type_overloads.rs` is
`vrun-a526af36-00c9-4d09-88a2-fecc77bb384b`: five compiled tests failed on
missing actual reorder/selection behavior, with zero skips. Final GREEN is
`vrun-22e2b3e9-8cd3-477d-a287-c0404cc5301e`: the same explicit file passed
with zero skips. The public tests assert selected graph literals, actual
file-order outcomes, invalid overload-recovery locations, and original
UnresolvedSymbol locations for genuinely visited inputs. Concurrent member
stage/Hash-coupling runs that failed compilation are setup evidence only:
`vrun-101ab4a0-922d-454a-8433-81203bace15f`,
`vrun-30b7ced3-74b8-40e6-a1b0-36b5e0715f6c`, and
`vrun-3e73cc76-a207-4729-903f-6a5f29c95c72`.

The independent Ousterhout review reread actual symbol/Parent cutoff,
specialized insertion and singleton anonymous-symbol premises, and found no
additional incorrect successful scalar answer. Its bounded verdict does not
establish context-sensitive retry, generic inference or general structured
relations. Those remain within the selected full scope.

When every candidate fails, the current product reports consumed
`overload_recovery` instead of publishing native recovery's healthy-looking
return. It preserves failures already visited during type arguments, minimum
counts, this and arguments. This boundary does not yet port the whole
`getCandidateForOverloadFailure`: deferred checking, longest/generic candidate
instantiation, combined parameter types and return intersections, early
installation before diagnostic reentry, and their actual child-failure/cache
schedule remain mandatory. In particular, a recovery candidate is distinct
from the resolving-signature sentinel and from an ordinary product failure;
an arbitrary failed operation cannot establish a native resolved-signature
property barrier.

The existing quick-type/initializer distinction is preserved: a sole
nongeneric signature can supply a quick return without applicability checking.
After quick-type returns None, `checkDeclarationInitializer` now routes its
captured source root through the existing normal expression cache. This was
same-question mechanical producer wiring, not a separate behavior fix. Fresh
object/contextual cache outcomes and active-loop cache suppression still need
their own final native controls.
