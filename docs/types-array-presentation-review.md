# Bounded Array graph admission review

This independent read-only review uses the Ousterhout skill. It assumes one
request-local Evaluator over captured Project inputs and a finite graph with
depth, node and serialized-type byte limits. It covers Array skeletons,
required-child replacement, deferred member admission and flattened metadata
accounting in the current presentation stage. It does not establish complete
generic/reference semantics or resource calibration.

No concrete new defect was found in this slice. The presenter consumes an
established immutable ReferenceView and one complete MembersOfType operation;
it does not inspect source AST or reconstruct parameter/property substitution.
This keeps the semantic decision with the evaluator and finite representation
with presentation.

At depth zero, the graph has its real Array kind, readonly fact and an explicit
omitted element descriptor. Deferred member fields remain absent and listed as
depth omissions. Neither the skeleton nor the bounded Array summary demands
the member inventory. The summary asks only its established element question.

Above that boundary, `required_child` replaces the descriptor already reserved
in the Array record. Its lower-bound check includes the descriptor-size change
and common omission change before admitting a child. A later failure to fit
rolls back graph nodes, IDs, queue and byte accounting; it retains encountered
semantic issues and the evaluator's actual demand result.

`reference_members` first admits the true minimum complete success header: four
known empty collection fields. Only then does it demand the deferred inventory.
Actual collection counts and child records are admitted after establishment.
An unsupported inventory records the original MembersOfType issue before
checking whether its blocked-collection metadata fits. If the metadata does not
fit, the graph remains omitted and truncated while the original issue and
partial result remain. Invalid recovery, resource and internal failures still
fail the whole operation. The known-member branch starts with its actual
reserved skeleton and does not need this unknown-inventory acquisition step.

`member_metadata_delta` accounts for flattened fields' leading comma and the
common node omission record. Its before/after field lists preserve unrelated
element and type-argument omissions. Successful prefix admission retains actual
omitted counts; unsupported inventory supplies no guessed count.

This source review ran no additional validation and proves no new byte
threshold boundary. The implementation owners' registered final Project
controls remain authoritative. Ordinary Reference callable-shape expansion is
still explicitly pending. Wider member/mapper/invalid causality, precise
under/at/over presentation cases, outer result/issue size limits and the complete
semantic/resource gates remain open.
