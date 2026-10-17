# Canonical parameter presentation review

This bounded read-only review applies the Ousterhout reviewing route to the
captured-Project, request-local native type operation. TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b` owns semantic expectations. The review
does not establish the complete generic, graph-budget or semantic delivery gate.

No blocking defect was found in the admitted ordinary parameter presentation
slice. `Model::type_parameter` allocates an identity; the Evaluator's canonical
declared-symbol cache decides reuse. Neither the parameter name nor constraint
origins are an interning key. The owned view retains independent constraint and
default handles. Summary reads the exact name without demanding either child,
and the graph skeleton records only fields whose handles actually exist.

`Graph::optional_child` checks the serialized optional-field metadata and
omission adjustment before demand. An unresolved handle previews the shortest
possible reference, rather than treating a DemandId as a future graph ID. After
actual demand, the operation checks the real field size and any newly reserved
child nodes. Rollback restores nodes, graph identities, queue, bytes and
truncation state. It deliberately retains semantic demand results and already
encountered issues. Unsupported children can therefore remain Partial even if
their displayed edge is rolled back; invalid, resource and internal errors
remain whole-operation failures. This preserves the design's separation between
semantic demand and display admission without exposing graph storage to callers.

The registered final Project parameter tests passed in
`vrun-136a2036-da03-47fa-ad85-a3a4f91a684b` (three tests, one matched file, zero
skips). They establish independent first-nonnil merged constraint/default
selection, lazy summary/depth-zero behavior, consumed missing-type failures and
ordinary any-to-unknown constraint normalization. This review inspected that
evidence and implementation; it did not repeat the run.

Residual obligations remain explicit. There is no parameter-specific final
byte-boundary regression in this slice; the shared admission mechanism has its
own existing graph controls. Declared variance and const modifiers remain
separate semantic facts, with no invented public graph fields. Generic reference
defaults must implement the native resolving/circular-default sentinel protocol
before those references are admitted. Parameter-chain base constraints remain
pending until the actual guarded resolution operation exists. Current plain
default type-node demand must not become a universal default resolver merely
because ordinary literal defaults pass.


## Stage 28: native rest-tuple expansion

The summary and diagnostic signature owners now acquire `getExpandedParameters`
through one completion operation before entering the declaration scope. Native
`signatureToSignatureDeclarationHelper` selects the original list only when an
expanded rest parameter is not last. The Q4 graph continues to expose the actual
signature parameter inventory. Its summary admission uses punctuation as the
safe lower bound, because a rest tuple may expand to zero parameters.

The expansion factory preserves argument acquisition before label selection and
fresh symbol creation before optional array construction. Each new parameter
slot stores its own value computation; the incoming argument/name prefix belongs
to the invocation. The native local uniqueness rule in nodebuilderimpl.go1906
preserves the first duplicate, unlike the related global checker helper.

Native capture `vrun-baca068a-556c-457b-a077-664bafeb2c02` acquired all 16 roots in
strict and loose modes, with pinned hook/API agreement and no diagnostics. Its
only failures were the two absent expected files; those exact measured files
are now installed. `tests/type_expanded_parameters.rs` asserts those summaries
in compact and expanded queries and retains original graph parameter names.
The maintained native rerun is root-owned; no Rust validation was run in this
source window.
