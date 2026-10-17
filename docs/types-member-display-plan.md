# Native inline member serialization

This Ousterhout design assumes one immutable accepted Project and one
request-local evaluator. Canonical symbols and mapper identities already own
member questions; the finite presenter owns only text/graph budgets and
formatting. Full native member, mapper and contextual families remain required.

Two interfaces were considered. Adding `display_type` to each property hides
Missing removal but leaves the presenter reconstructing accessor equality,
class-parent rules, method filtering and actual overload choice. Returning a
complete typed serialization decision hides those rules once in the evaluator.
The latter follows native `addPropertyToElementList` and avoids inspecting graph
kinds or string equality to reconstruct semantic choices.

Model `Property.display` carries either an established ordinary value demand or
an opaque `PropertySerializationId`. The evaluator retains the actual canonical
symbol, complete native metadata and mapper behind that identity. Its
`serialize_property` operation returns one of:

- Plain property with its actual nonmissing display-value demand.
- Method clauses with actual ordered signature descriptors.
- Getter/setter clauses with their actual signature descriptors.

Raw `Property.read_type` remains the native symbol read. An independent write
question may resolve to the same actual TypeId; the graph emits `writeType`
only for a different identity. Readonly suppresses that writable dependency.
The presenter admits the mandatory read-only row lower bound before asking the
write question. Semantic issues survive later graph rollback; invalid/resource
failures remain whole failures. This preserves a fitting row when its write
turns out identical and avoids using display refs as semantic identity.

Native `nodebuilderimpl.go:2465–2585` first obtains the nonmissing read. For an
accessor it asks write, compares actual TypeIds, and uses real declarations and
class-parent/property-declaration facts to choose signature syntax. For a method
it checks actual own property inventory, readonly, removes Undefined and asks
call signatures before its fallback. These decisions belong to the evaluator.
Nominal summaries bypass the anonymous member printer, so descriptor creation
must not eagerly consume those display questions.

Original and instantiated method values establish real anonymous TypeIds before
overload inventory. `AnonymousView` exposes its original target when present and
the actual deferred `MembersOfType` question. It does not assert an empty
collection. Model owns native flags/header facts; the evaluator's complete
`IsEmptyAnonymousObjectType` operation resolves inventory when that question
requires it. Kind metadata and anonymous text can require that inventory at
depth zero, while optional displayed signature children remain lazy.

The formal compiler suite `tools/type-member-display-oracle.test.mjs` passed
four tests, zero skips, in `vrun-06824310-0f87-4728-aa41-94cfbc8ccf10`.
Three option states distinguish raw optional property/method reads from printed
clauses. An inline getter string/setter Missing root remains healthy with
summary `{ value: string; }`, but the same-checker printer phase adds TS2304;
the actual setter parameter demand is ErrorType. Aggregate limit hooks do not
claim ordinary recovered-dependency causality. The pinned source's explicit
write demand and the direct child control establish this consumed dependency.

Final Project suite `tests/type_member_display.rs` has genuine RED
`vrun-e6b78d99-9c62-4f7f-a39c-fa53b78df496`: four compiled assertion failures,
zero skips, at the existing optional/method/accessor source Pending branches.
It asks summary-only, depth-zero and expanded questions; bad setter consumption
must remain InvalidSemantic at every depth. Production is being implemented
under that gate and the independent raw member-value gate. Byte/node boundary,
class accessor, method overload/mapper and complete serializer recovery controls
remain required; the compiler GREEN is not product proof.

# Setter return question refinement

A fresh same-checker scratch control also calls the actual
`GetReturnTypeOfSignature` for three setter declarations under strict and
loose options. Bodyless TypeLiteral setters, including a paired setter whose
parameter refers to `Missing`, return genuine `any` with no new diagnostic;
an implemented class setter with an empty body returns `void`. The setter
clause still consumes this actual return question even though the AST printer
omits its return text. It cannot replace bodyless return acquisition with an
assumed `void`, nor treat return acquisition as having checked its parameter.
The six observations and exact scratch overlay are retained in
`fixtures/types/native-member-display/evidence/setter-return/`; this is native
research evidence, not a registered production GREEN.
