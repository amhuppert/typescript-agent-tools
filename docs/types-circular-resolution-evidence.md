# Native type-resolution increment

The operating envelope is one request-local native checker over an immutable
captured Project. No concurrency mechanism or persistent checker cache is
needed. The full T/Q3/Q4 design remains authorized and in progress.

## Compiler baseline

`tools/type-circular-alias-oracle.test.mjs` passed two registered controls with
zero skips in `vrun-4c0c0aa9-40cd-4df9-8769-204eec562e1f`. Each question uses a
fresh pinned TypeScript 7.0.2 API session. Under strict and loose null profiles,
self and mutual aliases, an outer alias, and a variable annotation consuming
the cycle return error types. A deferred recursive object alias is healthy.
These observations establish expected behavior; they do not prove the native
implementation or infer causality from whole-program diagnostics.

## Ownership decision

Two approaches were considered using the Ousterhout design route. Replacing
every `Resolving` cache hit with a circular error is a small local patch, but
would leak a false universal protocol into interfaces, member reads, calls and
relations. A request-local resolution stack hides the pinned target/property
matching and participant marking in one owner. The complete alias operation
uses that stack, while interfaces retain their early construction and lazy
members. Value-type resolution uses the distinct `Type` property.

The chosen stack ports `pushTypeResolution`, `findResolutionCycleStartIndex`,
`typeResolutionHasProperty` and `popTypeResolution` from pinned
`internal/checker/checker.go` around lines 18663–18716. Matching keys include the
canonical target and property. An established intervening property stops the
search, and a cycle marks all participants from its first frame through the
current boundary. `getDeclaredTypeOfTypeAlias` around line 23745 owns alias
recovery at each marked participant's declaration. An outer, unmarked frame
preserves the consumed alias failure. Legal deferred recursion uses the
already established canonical type and never becomes an error merely because
its output graph contains a cycle.

The admitted symbol properties are value `Type` and alias `DeclaredType`.
Other target families and properties will be added with their actual native
operations, rather than represented by member-demand or output-node identities.
The resolving-signature sentinel and temporary resolution window remain owned
by the future complete `getResolvedSignature` operation; this increment does
not claim that checking-order behavior.

## Native proof

The four final-boundary tests in `tests/type_circular.rs` had actual assertion
RED `vrun-503028c0-8bf6-42a3-8733-12627591e3b2`: compilation succeeded,
the three invalid-cycle tests failed on the existing pending failure, and the
deferred recursive alias passed. Native recovery made all four pass in
`vrun-f7838129-5445-4d87-8c3a-b3621267a38d`. Extended recursive variable
coverage passed in `vrun-89c6f1c8-b905-47e6-b54b-62892c06f091`. These tests
exercise strict/loose profiles, both display modes for invalid roots and
depths 0/1/3/32 for valid recursive declared and variable roots, including the
exact native participant or consumed-child declaration position.

Adversarial Ousterhout review found no blocker for the admitted alias slice,
and identified two protocol distinctions to preserve before reentrant
function/generic operations grow. The parent now pushes `Type` only for the
variable/parameter/property family, not anonymous function construction.
An explicit `Recovered` cache state marks installed alias recovery; an
arbitrary `InvalidSemantic` dependency does not establish a native property.
This was green protocol cleanup, not another behavior RED claim. The four
circular tests passed afterward in
`vrun-86bb8c96-c178-45b7-ac5a-d128d3f21073`, and all four lazy-member
compatibility tests passed in `vrun-ce939ebe-70e1-4453-9d84-9afe02230081`.
All native runs above have zero skips.

Two earlier attempts failed during shared API setup/compilation and are not
behavior RED: `vrun-ff166fce-12d0-4144-877d-2bc71f1b9c0f` and
`vrun-c2af0f6a-220b-4d89-8dac-01e512f7331b`. The temporary signature window,
early generic interface construction, and variable initializer circularity
recovery remain required separate operations; these receipts do not prove them.
