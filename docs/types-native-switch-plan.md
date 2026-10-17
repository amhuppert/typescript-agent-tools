# Native switch graph, admission and end reachability

The operating envelope is one accepted immutable Project and one isolated
request checker. Switches supply required T read admission and end
reachability; changed value narrowing still belongs to excluded F. The full
T/Q3/Q4 design remains required.

The Ousterhout alternatives were ordinary condition branches versus native
SwitchClause nodes. Condition branches erase grouped empty clauses, case
expression visitation, fallthrough and the conditional bypass demand. Native
nodes preserve those facts behind the existing graph operation. Binder's
transient scheduler owns the active pre-switch flow and break target; published
nodes retain only exact switch sites, clause ranges and graph identities.
Nested switches therefore need no evaluator reconstruction or second binder.

Source exposes the actual expression and ordered clause sites. Typeof
witnesses preserve complete cooked UTF-16 units, so decoded escapes, duplicates,
empty strings and unpaired surrogates keep exact identity. Witness acquisition
is syntax-only and stops at a nonliteral case. The separately demanded
case-type vector checks all case expressions and regularizes their types;
default contributes ordinary Never. Its actual unrepresentable string type
remains a failure, even where the selected default would otherwise be unchanged.

Evaluation owns the complete switch operation and native exhaustive-state
cache. Reads consume the antecedent in a child invocation, then distinguish
matching discriminants, typeof operands, true switches, optional containment
and discriminant properties. Relation questions use the shared native relation
owner; type-facts classification cannot stand in for subtype or compatibility.
Native intermediate clause types remain available to ordered branch joins.
The completed reference read admits only its established identity: a changed
final read reports `flow_required`, while a failed demanded dependency retains
its own category. The first implementation checkpoint exposed why admission
cannot sit inside an individual clause: the direct and typeof joins restore
Number after their intermediate clauses produce a literal or Never.

Branch labels defer only their first empty switch bypass. Other paths can
return the declared/initial identity before the bypass is demanded. A distinct
non-Never bypass contribution asks exhaustiveness; an already collected type
does not. End reachability asks empty-switch exhaustiveness directly, then
follows its antecedent if necessary. These callers share the same exhaustive
question without sharing their traversal or cache policy.

Native exhaustive Computing reentry installs False. The outer calculation
must preserve that installed result. Typeof exhaustiveness acquires syntax
witnesses before checking its operand/base constraint. Ordinary exhaustiveness
checks the actual expression/base constraint, then literal eligibility, then
the complete case-type vector. No eager checking or universal circular-failure
fallback is introduced. Cached expression and flow-only expression questions
keep their native separate caches and checking windows.

Fresh compiler controls pass with zero skips in
`vrun-ea10d016-bd65-4273-8280-1343a9ea626d`: 43 markers across strict and
loose options, including literal/typeof identities, duplicates, grouped/default
clauses, fallthrough, nil witnesses, exact names and exhaustive returns.
Final Project seven-test RED is
`vrun-dd1611a1-b7f3-4081-a656-8dcce5806349`: six missing-behavior assertions
fail and the existing changed-read refusal control passes. The earlier
`vrun-2a673be4-c386-4ae1-bf7d-6c16b07b9bb4` is a compile/setup failure
from a mechanical global-target DTO cleanup, not a behavior RED.

[Independent source research](types-switch-flow-review.md) records native
operation boundaries and demand order. Full typed relations, operators,
loop/incomplete caches, capture setup and resource/corpus proofs remain
mandatory; these controls cannot close the complete checker gates.

The first compiled implementation checkpoint,
`vrun-95672a3f-9bff-4d96-b714-0968a84a0dfb`, passed four tests and failed
three behavior assertions. It distinguished the final-reference admission
ownership defect from the ordinary canonical Parameter dependency in typeof
exhaustiveness. The earlier `vrun-1edf8212-1c47-4439-8dce-559259aab11a`
was a compile/setup omission in an opaque graph node constructor. Neither is
a GREEN receipt.


## Completed read and compatibility checkpoint

The first compiled increment passed four of seven final tests in
`vrun-95672a3f-9bff-4d96-b714-0968a84a0dfb`. Two healthy after-switch
joins exposed premature intermediate identity admission; a third required the
native ordinary-parameter value owner. Clause typing now preserves its exact
intermediate TypeId for native joins, and the canonical parameter operation is
shared by read and signature questions. All seven passed in
`vrun-a76f05d2-3297-433e-9151-8a629ff5f183`.

Moving admission to the completed read then exposed one existing literal-base
conversion in the 22-test read compatibility file, genuine assertion failure
`vrun-2157bdf0-4078-443e-a501-21de42ad44da`. The complete read now records
only the actual consumed base result when its input equals the admitted
baseline. It admits that exact resulting TypeId; it does not admit a guessed
base alternative or suppress a changed switch result. Read compatibility
passed in `vrun-32eadc63-e173-4ae0-a819-609a606db456`, and all seven switch
controls passed again in `vrun-84d10b83-f62d-4fc7-aa2e-9a3b0b468017`,
zero skips. Independent native-source review accepted that bounded policy.

The formal fresh compiler replay
`vrun-75bc6c27-e290-48db-aaad-a833c35170f8` pins 86 strict/loose selected
roots with exact frozen witness/tool inputs. These scoped receipts do not close
full structured, automatic, optional and constrained switch families, general
F analysis, compiler/product resource or representation obligations.
