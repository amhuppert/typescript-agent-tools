# Bounded member value review

This read-only Ousterhout review covers the request-local native accessor and
method operations in `evaluation/members.rs`, `evaluation/anonymous.rs` and
`source/members.rs`. It assumes one immutable accepted Project. It is a source
review during the coupled member stage, not a replacement for final registered
Project tests or a full member/mapper gate.

The separate raw read, write, nonmissing and serialization questions are
justified. `getTypeOfAccessors` preserves getter annotation, setter annotation,
auto-accessor annotation and getter body precedence. Source projects each
annotation without checking it; an unsupported sibling type remains a recipe
until the actual native question selects it. A rejected Type push does not
install recovery, while a marked participant pop installs its own recovery.
`getWriteTypeOfAccessors` uses its distinct WriteType frame, then pops before
falling back to raw accessor read. Installed Resolved/Recovered properties are
preserved. Ordinary Failed product state is not a native installed-property
barrier.

The method's established anonymous identity precedes demanded signatures and
uses the canonical symbol value cache. Mapper keys use actual target and
effective argument identities, independently of names and origins. The
ordinary parent parameter filtering follows canonical references; unsupported
qualified/type-query/conditional decisions remain explicit. The local-parent
parameter projection is narrower than native recursive getOuterTypeParameters,
but the admitted named target's existing outer-context refusal prevents
generalizing it to unsupported outer generic/contextual scopes. Those complete
owners remain required.

One concrete source-backed serialization dependency was reported. Native
`addPropertyToElementList` decides accessor clauses from the **instantiated**
raw read/write identities. An interface getter of T paired with a number setter
has distinct original identities; after T maps to number they can become equal,
changing Accessors to Plain. Transforming the original serialized clauses in
`Serialization::Instantiated` preserves the old mode and cannot establish this
decision. Current nominal reference summaries may avoid that path, and no
final public false-result reproduction is claimed here. Before that path is
admitted, the complete owner must consume actual instantiated member/read/write
questions and choose the native mode, or retain a located required dependency.
Mapping original text/clauses is insufficient evidence. The implementation
owner subsequently replaced clause transformation with a shared
`serialization_question` acquisition and actual instantiated read/write/shape
decision in `serialize_property`. The original source defect is therefore
closed on reread. General mapper families and final variant proof remain
required; this source correction alone is not a full mapper gate.

A setup mismatch was reported before validation: two serialization arms used
`object.members` while TypeView::Object still carried MembersView directly.
The implementation owner corrected that DTO wiring before the final run. This is compile-stage
cleanup, not a behavioral RED or a correctness test result.

Pinned source is TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`: checker.go
`getTypeOfAccessors`/`getWriteTypeOfAccessors` (18424/18479),
`getObjectTypeInstantiation`/`isTypeParameterPossiblyReferenced` (22210/22309)
and `getOuterTypeParameters` (23664). No additional incorrect successful answer
was found in the currently admitted ordinary source slice. Inline canonical
members, general outer contexts, complete mapper shape and resource gates
remain required; explicit Pending does not complete them.

The corrected own/mapped serializer was reread after the nominal member gate.
It now acquires the original raw symbol read, applies the actual mapper, then
removes Missing for the display question. Accessor write is acquired and mapped
independently; optional Property write instead removes Missing from the actual
mapped raw read. Method own-property inventory precedes readonly and Undefined
filtering. The shared decision therefore compares current semantic identities
rather than mapping an earlier Plain/Methods/Accessors choice. No further
incorrect successful answer was found in this admitted slice. A mapped optional
Property counterexample, general anonymous instantiation, overloaded class
implementation filtering and inline canonical member binding still need their
final Project proof.

Final nominal member suite `tests/type_member_values.rs` passed eight tests,
zero skips, in `vrun-c90be8fb-4a6f-431f-b42a-106c90537c75`. This also proves the
native function-type parentheses in an optional Method union; the first
compiled checkpoint had exposed the missing parentheses. The presenter now
uses the single native `GetTypeNodePrecedence` question for Union and Array
children. The depth-zero expectation was corrected to the existing whole-field
omission contract, and loose plus exact-optional rejection remains TS5052.
Neither expectation correction is a product behavior fix. The independent
inline four-test assertion RED remains open until canonical anonymous binding
and final serialization tests pass.
