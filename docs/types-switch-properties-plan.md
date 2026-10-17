# Native switch property discrimination

The operating envelope is one accepted immutable TypeScript Project and a
request-local evaluator. Native behavior is TypeScript 7.0.2 at pinned commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.

The operation is `narrowTypeBySwitchOnDiscriminantProperty`
(`internal/checker/flow.go:1212`), called from `getTypeAtSwitchClause` after
optional-chain containment (`flow.go:1043`). The previous Rust admission helper
interrupted after computing the matching access. The staged whole operation
keeps changed intermediate flow types: the existing completed-reference
boundary decides whether the final read needs the documented FlowRequired
outcome.

The selected design adds one whole worker in `reads/switches.rs`, with the
existing state, actual access Site, statement Site, and clause start/end. It
returns `NativeResult<TypeId>`. The alternative of translating a case group
into equality conditions would duplicate key-map policy, reverse dependency
order, lose the full-case-vector cache question, and mishandle default groups.
No new Source recipe, cache, or generic callback abstraction is needed.

The worker's exact schedule is:

1. Only when start < end and the current type is a Union, acquire the accessed
   property name. A missing or empty name skips key-map acquisition.
2. Acquire `union_key_property_name_completion` and compare the exact key.
   This is name-before-key, unlike the separate equality optimization.
3. On a match, acquire the whole `switch_clause_types_completion` vector and
   then select the current group. Every clause in the statement is acquired
   even if the group has one literal case. The shared cache owns this demand.
4. Map each group clause with raw `union_constituent_for_key`. Absent and native
   unknown entries both produce the actual Unknown singleton. Construct the
   candidate through the ordinary `getUnionType` completion (Literal reduction,
   nil alias/origin). Return it only when its identity differs from Unknown.
5. Otherwise call the existing `narrow_discriminant_completion`, passing a
   callback to `switch_discriminant_completion` for the actual statement/group.
   That preserves a second name acquisition, actual optionality adjustment,
   property read, full-vector acquisition, and final property/index comparisons.
   It must not replace the callback with condition narrowing.

The caller replaces its `admit_discriminant_completion` use with the actual
access result and the whole worker. The now-unused admission helper is removed
from reads.rs under the parent's narrow lease. The caller must use the type
already produced by optional-chain containment, and preserve the antecedent's
incomplete flag via the existing ReadInvocations owner.

Each invocation uses NativeCompletion sequencing. The candidate union factory
can consume recovery; an Unknown result continues into the fallback. A second
clause-vector getter consumes its installed completion as an actual cache hit,
without recomputing source expressions or discarding evidence. No new slot is
installed by this worker, and no evidence is moved into a key-map or clause
cache that the corresponding producer did not install. An outer interruption
stops subsequent work. The whole callback's evidence survives unchanged-type
answers and final FlowRequired admission.

## Proof inputs prepared while production remains frozen

`native-switch-properties` has 40 selected reference roots under strict,
loose-null, and exact-optional profiles. The unchanged shared hook observes
actual selected flow identity and whether marked erroneous case references
were acquired. Those per-range observations overwrite and establish presence,
not acquisition order or counts; diagnostic-code lists are source-sorted.
Ordinary API/hook equality is checked before captured baselines are compared.
No expected outputs or final Rust expectations are taken from this tool.

The variants are ordinary two-member fallback; grouped/all/default/implicit
and joined branches; 9/10/11 constituent feasibility; successful/absent/duplicate
and union-literal key entries; a different earlier unit property selected as
key; dot, bracket, decoded escaped and constant names; empty name; optional
receiver branches; an already narrowed single constituent; and a generic
property whose discriminator eligibility is false. Unrelated references and
dynamic names must skip erroneous cases.

Noisy 9/10/11 unions put an unresolved nonunit property before the unit key.
The native threshold predicts the 9-member query skips that key-search demand,
while 10/11 acquire it before clause types. An empty-name ten-member control
must skip key-map acquisition even though the threshold is met. These compact
queries intentionally avoid graph expansion, which is a separate legitimate
consumer of the latent erroneous properties.

The recovery cases place two independently unresolved case references inside
and outside the selected group, in the default path, and after the key-search
recovery. Native source determines acquisition order; capture establishes
presence and actual final flow identity. Final Project tests will require the
original ordered failures even when the native result is unchanged or a later
admission rejects a refinement. Exact repeated-cache evidence assertions need
the relevant installed-completion schedule, not deduplication by source site.

The maintained native run passed all 120 queries in
`vrun-2a93c7ff-c859-4436-ac44-c444048f06fa`. The complete operation is staged in
`reads/switches.rs`; its now-unused admission helper was removed from
`reads.rs`. A bounded peer review passed the native acquisition and completion
schedule. Rust validation remains pending with the parent.

## Captured outcomes and final assertion draft

Native capture `vrun-6eed5b8c-e972-48b0-ac62-d7284c610491` compared all 120
hook/API results successfully. The only bootstrap failures were missing
expectation files. `.cc/temp/native-switch-properties-{tsconfig,loose,exact}.json`
were copied verbatim into the fixture; all inputs and baselines are frozen.
The installation itself changed no production or Rust tests. After the parent
released the next implementation window, `tests/type_switch_properties.rs`
was staged with all 40 rows, compact/expanded controls, exact final failure
sites, and the ordered clause-cache replay assertion described below.

A key measured distinction is that `tenAll` and `elevenAll` do not retain the
original declared identity, even though every constituent is covered. Native's
optimized ordinary union drops the original union alias identity; bracket,
escaped-name, and constant-name equivalents do the same. These are final
FlowRequired controls, not healthy answers inferred from equal constituent
sets. `nineAll`, `tenGroupedDefault`, and `otherKey` retain declared identity.
The optional-undefined branch retains Ten only with loose null checks.

The final tests derive healthy versus changed identity directly from the
native selected-reference observation. Recovery rows separately require the
original Missing annotations before any actual final FlowRequired suffix;
a changed result cannot hide those acquisitions. The earlier planning data
at `.cc/temp/switch-properties-final-assertion-plan.json` is not a product
output baseline.
Strict/exact native rows also contain two startup diagnostic 2318 entries from
the minimal global declarations. These are retained verbatim and are not treated
as consumed query failures; loose rows have no such startup entries.

The five erroneous-case rows distinguish one case-vector acquisition
(`caseRecovery`'s successful map, the small-union fallback, and empty-name
fallback) from native Unknown-candidate fallback (`groupRecovery` and
`defaultRecovery`). The latter acquires the installed vector again. Final tests
can normalize the two independent annotation sites to first/second and compare
the fallback's ordered Missing evidence to two copies of the successful-map
control. This preserves repeated acquisitions and avoids guessing each leaf
getter's own evidence multiplicity. A final FlowRequired cause belongs only to
the rows whose measured native identity changed, at the actual selected read.

`noiseTen` and `noiseEleven` require MissingKeyNoise0; the map candidate search
reads the first declaration's noise property before selecting kind. The
`keyBeforeCases` row requires that site before MissingKeyCaseFirst and
MissingKeyCaseSecond. The 9-member noisy control and ten-member empty-name
control remain healthy compact queries. Their deeper graph expansions would
separately consume latent property types, so this flow proof does not claim
that those expansions are healthy.
