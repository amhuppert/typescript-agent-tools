# Member roots and directional relationships

Bounded compiler-baseline capture, 2026-10-10, using the Ousterhout ownership and
error-semantics principles. This fixture tests the pinned operations behind Q3;
it is not a native implementation, product canonicalization proof, or completed
checkpoint. The envelope is a local captured fixture, one fresh checker per
request, and the design's explicit enclosing-call/candidate schedule.

The standalone exploratory capture queried 43 declaration/use markers in
[member-relations/cases.ts](../fixtures/types/member-relations/cases.ts) through
the test-only TypeScript 7.0.2 hook at source commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. The pinned source tree and compiler
limit constants were unchanged; a test-only overlay added observations and the
specified demand schedule. Full raw answers, candidate observations and exact historical tooling
are retained with [provenance](../fixtures/types/member-relations/provenance.json).
The current executable assertions are
[tools/type-member-relations.test.mjs](../tools/type-member-relations.test.mjs).
These data/fixture additions have no production behavior to implement; there is
no claim of a native red-green cycle for them.

## Observed distinctions

| Family | Observed pinned behavior |
| --- | --- |
| `Box<string>` / `Box<number>` | Different instantiated value types keep the same member root. Declaration and both uses produce the same group. |
| Partial / Readonly / Pick / Omit | Homomorphic member uses retain the Box declaration root and group. |
| Remapped `new_value` | The synthetic selected symbol has no own declarations; `GetRootSymbols` returns Box.value. Its search occurrence is the remapped name, while a Box.value seed does not search that different key. |
| Ordinary and optional-overlap spread of the same Box origin | The uses retain that Box root. This does not distinguish two distinct spread origins; that matrix remains open. |
| Getter-only, setter-only and readonly-changing spreads | The observed spread uses retain their respective source property/accessor roots. Root retention does not prove read/write type semantics. |
| Annotated contextual object method | The declaration search set includes its own method and the Host interface method; the returned group uses the interface origin. |
| `satisfies` object method | Declaration selection relates the contextual Host group and the use's own group. Selection at the use instead returns the object's own declaration/use group. |
| Unrelated equal method signature | It retains its own declaration/use group and gains no Host relationship. |
| Base / override | Both declaration seeds and use controls relate the Base/Derived groups. |
| Implements | Interface and implementing class relate their groups; unrelated same-signature Base/Derived groups remain separate. |
| Inline union versus interface union | Inline property seeds substitute to the shared union search and return both constituent declaration groups plus the combined use group. An interface constituent seed returns only its own declaration group and the combined use; it does not search the other interface declaration. |
| Static / instance / private | Distinct groups retain exact declaration spans, including static and instance declarations on one line and private names with equal spelling in different classes. |
| Getter / setter pair | Both declaration sites and the use retain one group with both distinct accessor origins. |

These results demonstrate why roots, reference relationships, exact keys,
selection roles and assignability are separate questions. Joining symbols by
spelling or structural type equality would destroy measured directional facts.
Calling one generic origin lookup for every question would lose remapped search
keys, contextual selection-site effects and dual roles. The native owners must
hide those complete operations while preserving their distinctions.

## Verification and remaining work

The registered single-file check is:

```sh
cctl validate run harness-test --queue-if-busy --require-match -- tools/type-member-relations.test.mjs
```

The first six-test registered run passed in
`vrun-b010ac19-16c0-4207-bb79-9541e99540a3`: six executed, six passed, zero skipped.
It checks final reference groups
and roots, with exact positions for shared-line/private/accessor identities;
counters alone are not the assertions. Expected facts came from the pinned
compiler capture, not Rust output. Some additional use controls are derived
from the same declared identities and checked independently in that run.

Raw native groups and deterministically sorted observations are retained
separately. Full product normalization and candidate accounting remain open,
including repeated synthetic roots, parameter-property identities sharing an
origin, decoded raw scan misses, JSON duplicate keys, multiple distinct optional
spread origins, missing union members, genuine any/unknown/recovery receivers,
all contextual forms, and fresh/repeated/reversed cross-file schedules. This
fixture does not classify unsupported dependencies, prove native inference,
flow admission or overload behavior, or measure service/resource guarantees.
