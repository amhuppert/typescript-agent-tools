# Restrictive-cache recovery candidate

The selected property is the healthy literal `41`. `MissingMarker` occurs only
on an unused sibling. Cold native queries in strict and noCheck return `41`
with one TS2304 diagnostic and no erroneous returned type acquisitions. A cold
query of `healthy` returns `45` with no diagnostics.

The existing optional nonforcing restrictive-cache observer sees, in order:

1. Object input: miss, same result, installs its own cache entry.
2. Type-parameter input: miss, same result, installs its own cache entry.
3. The same object input: cache hit returning itself.
4. The same type parameter: cache hit returning itself.

None of these four calls emits a fresh diagnostic or erroneous type acquisition.
The harness compares identities only within each request, never across checkers.

This establishes the native cache path and removes the old candidate's final
property error acquisition. It does not alone prove the product preserves a
recovered completion on the self-key hit. The exact product replay ledger and a
marker-only counterfactual are separate evidence obligations; do not label a
minimum cause count as marker proof.
