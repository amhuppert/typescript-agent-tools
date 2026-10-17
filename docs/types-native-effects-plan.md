# Native call-effect admission

The operating envelope remains one immutable captured Project and one
request-local evaluator. Ordinary read admission and function-end reachability
consume the same native FlowCall nodes; F still owns value narrowing. Full
T/Q3/Q4 semantics and resource gates remain required.

The Ousterhout design comparison considered letting every reader inspect a
callee's syntax against a list of safe functions versus one complete cached
`getEffectsSignature` operation. The former duplicates alias resolution,
explicit annotation eligibility, overload selection and recovery ordering in
reads and reachability. The latter hides those decisions behind an actual call
site and preserves its native signature dependencies. No printed name or origin
becomes a semantic key. The selected operation owns request-local effect cache
states and returns an established signature or known absence of one; failures
remain distinct.

Pinned TypeScript 7.0.2 `getEffectsSignature` first acquires an explicitly typed
dotted target for an expression-statement call, or the ordinary checked target
for other calls. `getExplicitTypeOfSymbol` deliberately returns no type for an
ordinary variable without an annotation, without checking its initializer.
Unknown raw symbols likewise establish no explicit target. Single nongeneric
signatures are selected directly; other signatures require `getResolvedSignature`
only when `hasTypePredicateOrNeverReturnType` finds a possible effect. Signature
predicate questions and annotated return questions use their actual deferred
operation identities. Missing annotations do not force an unrelated return
inference merely to ask whether an annotated never return exists. Required
predicate-body inference, apparent/global types, overload resolution, property
and alias families remain actual operations, not guessed no-effect answers.

A known no-effect call follows its antecedent. Assertion effects require the
native reference/argument/predicate operation; never effects have their distinct
unreachable result and end-reachability behavior. Ordinary type predicates are
not statement assertions. Effect acquisition consumes recovery dependencies,
including predicate/annotation children, and must propagate invalid/resource
failures even if an eventual flag check would say the call cannot narrow.

Final read18 RED `vrun-83b3da70-9443-4ca7-b430-a6c62424468f` had seventeen
passing controls and one assertion failure at a typed ordinary call. Compiler15
GREEN `vrun-b25e9b3f-5e2e-4a7c-a305-60b739f82f2b` executed fifteen controls
without skips. The new cases include generic and overloaded no-effect signatures,
annotated variables, unannotated invalid initializers, unknown call names, comma
calls and unrelated invalid arguments/bodies. Positive baselines do not prove
native implementation or the complete effect/end/inference/resource matrix.

Read18 GREEN `vrun-b0645bac-a837-48a4-b001-2cf4d79d90fe` passed eighteen
final Project controls with zero skips. Read19 RED
`vrun-eda782d6-4742-4121-b6c0-afb9c0cb5ec5` failed the ordinary Is predicate
statement read with eighteen other controls passing. GREEN
`vrun-81a2a426-4d2c-4d64-b866-d798b93b8ecc` passed nineteen with zero skips.
The model now distinguishes native unreachableNever from ordinary never.
Call interpretation returns that sentinel before acquiring an antecedent;
the final reference operation restores its declared type. Ordinary Is
predicates acquire their actual predicate dependency but do not assert a
statement's argument. Assertion narrowing and complete end inference remain
required operations.

Adversarial review supplied a named JSON property call with no annotation.
Final Project RED `vrun-93e43136-cd7a-48bc-adac-d471506f2d76` incorrectly
consumed apparent primitive call signatures. GREEN
`vrun-867f48a7-a66d-442f-b6a0-5ff5227855cb` passed its reproduction with no
skips after actual JSON module acquisition was preserved and the property's
absent explicit annotation established. Default/root alias flavor remains
an actual pending dependency; no healthy JSON type substitutes for that fact.

Compiler16 GREEN `vrun-d4af6694-7753-4614-96b0-d6580957fd36` executed all
sixteen controls without skips, adding false/parenthesized/AND/OR assertion
reachability and true controls. A read20 launch during a coupled This-type
stage failed setup, so it is not a behavior RED or native proof.
