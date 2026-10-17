# Retained JSON facts review

This is a bounded read-only Ousterhout review of the new private JSON facts
owner and its existing reference consumers, for one captured Project generation.
It does not establish native imported-JSON type semantics or complete Q3 groups.

No blocking defect was found in `src/project/binding/json.rs` or the public
compatibility tests in `tests/reference_json_facts.rs`. Exact `JsonString`
equality preserves decoded key identity: raw and escaped equivalent names share
a key, paired astral values share their decoded string, and unpaired UTF-16 units
remain exact rather than merging through replacement characters. Every raw key
occurrence retains its own child ID and the final duplicate's child ID; the
first-occurrence index is only an accelerator for the existing lookup operation.
That operation still chooses the final value, so there is one owner for duplicate
selection rather than another rule in the retained representation.

The breadth-first value queue gives each pending object or array child one ID.
Its assignment accounts for already queued children and retains array elements
and overwritten duplicate values. Existing apparent-interface behavior for
scalar and array reference lookup remains separate from future imported-JSON
widening and optional-slot decisions. Keeping that distinction avoids turning a
representation improvement into an unproved semantic change.

The public compatibility gate is
`vrun-54cdc85d-c242-4322-a337-f991490b5bb6`. Three new private tests inspect facts
through the final Project, covering duplicate/decoded keys, all raw occurrence
origins, UTF-16 identities, line-break/BOM spans and a wide shallow queue. Their
full Rust checkpoint `vrun-a7afe484-7a17-4fd9-9f4c-5eb6c8896366` subsequently
passed all three, the existing JSON reference tests and the three public
compatibility tests. That overall run later failed an unrelated service protocol
test, so it is not a full compatibility GREEN. Broader corpus cost and native
JSON Q4 remain open.

Reviewed SHA-256 values:

| File | SHA-256 |
| --- | --- |
| `src/project/binding/json.rs` | `f56118cf2dae37e78aedbe23a7ec61f2129f1c5da7b396469d3f4a7fccb2918e` |
| `tests/reference_json_facts.rs` | `9bf4cb64a3f6e023bfde554de29d243ab8864a66495d7ec888f7eb86e73f129a` |
